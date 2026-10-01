//! The Commit Engine lock under an adversarial scheduler.
//!
//! The harness runs a network of validators through the vote path the node
//! runs: the vote ledger, the lock, the round timeout statements and the
//! round change certificate, all from `engines::commit::lock`. A scheduler
//! delays every message by up to several slots, loses some, reorders them, reorganises
//! the best chain of every validator at every height and restarts validators
//! in the middle of a round. A restarted validator keeps only its vote ledger,
//! the way the node keeps the ledger file on disk.
//!
//! Each schedule checks two properties.
//!
//! 1. **Safety.** No two different blocks commit at one height, on any node,
//!    at any moment.
//! 2. **Progress.** Once the delays end, every validator commits every height.
//!
//! Votes carry the epoch they were signed in. The wall clock keeps moving the
//! epoch while a stall holds, so a schedule crosses epoch boundaries before the
//! delays end. A vote stays current from the epoch of the committed head
//! onward. Progress is measured in rounds: once the delays end, every height
//! commits within a bounded number of rounds.
//!
//! Further tests replay the two delay exercises recorded on the nine validator
//! network: the first, where six validators signed two votes in round 142 and
//! two groups of nodes committed two different blocks at height 1440, and the
//! second, where one machine of three ran a second behind for three minutes
//! and finality stalled across an epoch boundary.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cerulea_consensus::engines::commit::lock::{
    blocking_locks, certificate_release, certificate_undecided, choose_vote, CastVote,
    TimeoutStatement, VoteChoice, VoteLedger, STATEMENT_WINDOW_ROUNDS,
};
use codec::{Decode, Encode};

/// The commit threshold, in percent of the total weight.
const THRESHOLD_PERCENT: u128 = 67;

/// The share of messages between peers lost while the fault holds, per
/// thousand.
const LOSS_PER_THOUSAND: u64 = 100;

/// A small deterministic generator, so every schedule replays exactly.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            0
        } else {
            self.next() % bound
        }
    }

    fn chance(&mut self, per_thousand: u64) -> bool {
        self.below(1000) < per_thousand
    }
}

/// The block one fork carries at one height.
fn block(height: u32, fork: u8) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes[0] = fork + 1;
    bytes[1..5].copy_from_slice(&height.to_le_bytes());
    bytes
}

/// The fork one block belongs to.
fn fork_of(hash: &[u8; 32]) -> u8 {
    hash[0] - 1
}

#[derive(Clone, Debug)]
enum Message {
    Vote {
        from: usize,
        round: u32,
        height: u32,
        hash: [u8; 32],
    },
    Timeout {
        from: usize,
        round: u32,
        height: u32,
        voted: Option<[u8; 32]>,
    },
    Commit {
        height: u32,
        hash: [u8; 32],
    },
}

/// The round timeout statement of each validator in one round.
type TimeoutBook = BTreeMap<usize, Option<[u8; 32]>>;

/// One validator of the harness network.
struct Node {
    /// The ledger bytes on disk.
    disk: Vec<u8>,
    ledger: VoteLedger,
    round: u32,
    round_started: u64,
    fork: u8,
    committed: BTreeMap<u32, [u8; 32]>,
    /// The first vote seen from each validator in each round.
    votes: HashMap<(u32, usize), (u32, [u8; 32])>,
    timeouts: HashMap<(u32, u32), TimeoutBook>,
    seen_round: BTreeMap<usize, u32>,
    down_until: u64,
    sent_timeouts: BTreeSet<(u32, u32)>,
    /// The round the node reached when it last committed.
    commit_round: u32,
    /// The epoch the node ran when it last committed.
    commit_epoch: u64,
}

impl Node {
    fn new(fork: u8) -> Self {
        Node {
            disk: VoteLedger::new().encode(),
            ledger: VoteLedger::new(),
            round: 0,
            round_started: 0,
            fork,
            committed: BTreeMap::new(),
            votes: HashMap::new(),
            timeouts: HashMap::new(),
            seen_round: BTreeMap::new(),
            down_until: 0,
            sent_timeouts: BTreeSet::new(),
            commit_round: 0,
            commit_epoch: 0,
        }
    }

    fn target(&self) -> u32 {
        (1..)
            .find(|height| !self.committed.contains_key(height))
            .unwrap_or(u32::MAX)
    }

    /// Restart from the ledger on disk. Every message the node held is gone.
    fn restart(&mut self, now: u64) {
        self.ledger = VoteLedger::decode(&mut &self.disk[..]).expect("the ledger reads back");
        // The chain round never falls below a round this node recorded.
        self.round = self
            .ledger
            .highest_round()
            .map(|round| round + 1)
            .unwrap_or(0)
            .max(self.round);
        self.round_started = now;
        self.votes.clear();
        self.timeouts.clear();
        self.seen_round.clear();
        self.sent_timeouts.clear();
        self.commit_round = self.round.saturating_sub(STATEMENT_WINDOW_ROUNDS);
    }
}

/// The shape of one schedule.
#[derive(Clone, Copy)]
struct Shape {
    validators: usize,
    heights: u32,
    fault_ticks: u64,
    max_delay: u64,
    round_timeout: u64,
    reorg_per_thousand: u64,
    restart_per_thousand: u64,
    unequal_weights: bool,
    /// The ticks one epoch of wall clock lasts.
    epoch_ticks: u64,
    /// One machine of validators, by index range, that every message to or
    /// from runs late by the stated ticks while the fault holds.
    slow_machine: Option<(usize, usize, u64)>,
}

struct Network {
    nodes: Vec<Node>,
    weights: Vec<u128>,
    total: u128,
    queue: BTreeMap<u64, Vec<(usize, Message, u64)>>,
    now: u64,
    /// Every commitment any node made, by height.
    commitments: BTreeMap<u32, BTreeSet<[u8; 32]>>,
    shape: Shape,
    healed: bool,
    seed: u64,
}

impl Network {
    fn new(shape: Shape, rng: &mut Rng) -> Self {
        let weights: Vec<u128> = (0..shape.validators)
            .map(|_| {
                if shape.unequal_weights {
                    1 + rng.below(5) as u128
                } else {
                    3555
                }
            })
            .collect();
        let total = weights.iter().sum();
        let nodes = (0..shape.validators)
            .map(|_| Node::new(rng.below(2) as u8))
            .collect();
        Network {
            nodes,
            weights,
            total,
            queue: BTreeMap::new(),
            now: 0,
            commitments: BTreeMap::new(),
            shape,
            healed: false,
            seed: 0,
        }
    }

    fn crosses(&self, signed: u128) -> bool {
        signed.saturating_mul(100) >= self.total.saturating_mul(THRESHOLD_PERCENT)
    }

    fn epoch_now(&self) -> u64 {
        self.now / self.shape.epoch_ticks.max(1)
    }

    fn send(&mut self, from: usize, message: Message, rng: &mut Rng) {
        let epoch = self.epoch_now();
        for to in 0..self.nodes.len() {
            let mut delay = if to == from {
                0
            } else if self.healed {
                rng.below(2)
            } else {
                rng.below(self.shape.max_delay + 1)
            };
            if let (Some((low, high, late)), false) = (self.shape.slow_machine, self.healed) {
                let slow = |index: usize| index >= low && index < high;
                if to != from && slow(to) != slow(from) {
                    delay += late;
                }
            }
            // While the fault holds, a share of the messages between peers
            // is lost. Once it ends, every message arrives.
            if to != from && !self.healed && rng.chance(LOSS_PER_THOUSAND) {
                continue;
            }
            // The queue of the current tick already ran, so the earliest
            // arrival is the next tick.
            self.queue
                .entry(self.now + delay.max(1))
                .or_default()
                .push((to, message.clone(), epoch));
        }
    }

    /// Report whether a node takes a message signed in one epoch.
    ///
    /// A vote stays current from the epoch of the committed head onward, the
    /// rule `dvf_gossip::epoch_is_current` applies.
    fn epoch_current(&self, node: usize, epoch: u64) -> bool {
        let current = self.epoch_now();
        let committed = self.nodes[node].commit_epoch.min(current);
        epoch >= committed && epoch <= current + 1
    }

    fn record_commit(&mut self, node: usize, height: u32, hash: [u8; 32], rng: &mut Rng) {
        if self.nodes[node].committed.contains_key(&height) {
            return;
        }
        self.nodes[node].committed.insert(height, hash);
        let held = self.commitments.entry(height).or_default();
        held.insert(hash);
        assert_eq!(
            held.len(),
            1,
            "seed {}: two blocks committed at height {}: {:?}",
            self.seed,
            height,
            held
        );
        let now = self.now;
        let state = &mut self.nodes[node];
        state.fork = fork_of(&hash);
        state.ledger.prune_through(height);
        state.disk = state.ledger.encode();
        state.round = state.round.saturating_add(1);
        state.round_started = now;
        state.commit_round = state.round;
        state.commit_epoch = now / self.shape.epoch_ticks.max(1);
        self.send(node, Message::Commit { height, hash }, rng);
    }

    fn deliver(&mut self, to: usize, message: Message, epoch: u64, rng: &mut Rng) {
        if self.nodes[to].down_until > self.now {
            return;
        }
        if !matches!(message, Message::Commit { .. }) && !self.epoch_current(to, epoch) {
            return;
        }
        match message {
            Message::Vote {
                from,
                round,
                height,
                hash,
            } => {
                let state = &mut self.nodes[to];
                let seen = state.seen_round.entry(from).or_insert(0);
                *seen = (*seen).max(round);
                if let Some(held) = state.votes.get(&(round, from)) {
                    assert_eq!(
                        *held,
                        (height, hash),
                        "validator {} signed two votes in round {}",
                        from,
                        round
                    );
                    return;
                }
                state.votes.insert((round, from), (height, hash));
                self.check_commit(to, round, height, hash, rng);
            }
            Message::Timeout {
                from,
                round,
                height,
                voted,
            } => {
                let state = &mut self.nodes[to];
                let seen = state.seen_round.entry(from).or_insert(0);
                *seen = (*seen).max(round.saturating_add(1));
                state
                    .timeouts
                    .entry((round, height))
                    .or_default()
                    .entry(from)
                    .or_insert(voted);
            }
            Message::Commit { height, hash } => {
                self.record_commit(to, height, hash, rng);
            }
        }
    }

    fn check_commit(
        &mut self,
        node: usize,
        round: u32,
        height: u32,
        hash: [u8; 32],
        rng: &mut Rng,
    ) {
        if self.nodes[node].committed.contains_key(&height) {
            return;
        }
        let signed: u128 = self.nodes[node]
            .votes
            .iter()
            .filter(|((held_round, _), (held_height, held_hash))| {
                *held_round == round && *held_height == height && *held_hash == hash
            })
            .map(|((_, from), _)| self.weights[*from])
            .sum();
        if self.crosses(signed) {
            self.record_commit(node, height, hash, rng);
        }
    }

    fn statements(&self, node: usize, round: u32, height: u32) -> Vec<TimeoutStatement> {
        self.nodes[node]
            .timeouts
            .get(&(round, height))
            .map(|held| {
                held.iter()
                    .map(|(from, voted)| TimeoutStatement {
                        signer: [*from as u8; 32],
                        weight: self.weights[*from],
                        voted: *voted,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Send the round timeout statements a node owes.
    ///
    /// The node states every closed round since its last commitment, inside
    /// the statement window. A round without a vote is recorded as abstained
    /// before the statement leaves. `force` sends every statement again.
    fn state_rounds(&mut self, node: usize, force: bool, rng: &mut Rng) {
        let target = self.nodes[node].target();
        let round = self.nodes[node].round;
        let first = self.nodes[node]
            .commit_round
            .saturating_sub(1)
            .max(round.saturating_sub(STATEMENT_WINDOW_ROUNDS));
        for past in first..round {
            let fresh = self.nodes[node].sent_timeouts.insert((past, target));
            if !fresh && !force {
                continue;
            }
            let state = &mut self.nodes[node];
            if state.ledger.vote_in_round(past).is_none() && !state.ledger.abstained_in(past) {
                state
                    .ledger
                    .abstain(past)
                    .expect("a round without a vote abstains");
                state.disk = state.ledger.encode();
            }
            let voted = state.ledger.statement_for(past, target);
            self.send(
                node,
                Message::Timeout {
                    from: node,
                    round: past,
                    height: target,
                    voted,
                },
                rng,
            );
        }
    }

    /// One tick of one validator.
    fn act(&mut self, node: usize, rng: &mut Rng) {
        if self.nodes[node].down_until > self.now {
            return;
        }
        if self.now % 4 == 0 {
            // A node that missed a commitment while it was down learns it
            // again, the way block sync carries the commitment record.
            let held: Vec<(u32, [u8; 32])> = self.nodes[node]
                .committed
                .iter()
                .map(|(height, hash)| (*height, *hash))
                .collect();
            for (height, hash) in held {
                self.send(node, Message::Commit { height, hash }, rng);
            }
        }
        let target = self.nodes[node].target();
        if target > self.shape.heights {
            return;
        }

        // Catch up with a later round that more than one third of the weight
        // already reached. The statements of the skipped rounds follow.
        let mut rounds: Vec<(u32, u128)> = self.nodes[node]
            .seen_round
            .iter()
            .map(|(from, round)| (*round, self.weights[*from]))
            .collect();
        rounds.sort_by(|left, right| right.0.cmp(&left.0));
        let mut weight = 0u128;
        let mut reached = 0u32;
        for (round, held) in rounds {
            weight += held;
            if weight.saturating_mul(3) > self.total {
                reached = round;
                break;
            }
        }
        if self.healed {
            // Once the delays end every node follows one chain, and the chain
            // carries one round. A node reads the highest round it has seen.
            let highest = self.nodes[node]
                .seen_round
                .values()
                .copied()
                .max()
                .unwrap_or(0);
            reached = reached.max(highest);
        }
        if reached > self.nodes[node].round {
            self.nodes[node].round = reached;
            self.nodes[node].round_started = self.now;
        }

        // The round timeout.
        if self.now.saturating_sub(self.nodes[node].round_started) >= self.shape.round_timeout {
            self.nodes[node].round += 1;
            self.nodes[node].round_started = self.now;
        }

        // The statements of every closed round, and every few ticks all of
        // them again for peers that restarted.
        let resend = self.now % 4 == 0;
        self.state_rounds(node, resend, rng);

        // The vote of this round, under the lock.
        let round = self.nodes[node].round;
        if let Some(held) = self.nodes[node].ledger.vote_in_round(round) {
            if resend {
                self.send(
                    node,
                    Message::Vote {
                        from: node,
                        round,
                        height: held.height,
                        hash: held.hash,
                    },
                    rng,
                );
            }
            return;
        }
        let fork = self.nodes[node].fork;
        let candidate = block(target, fork);
        let related = |lock: &CastVote| lock.hash == block(lock.height, fork);
        let certificate = |lock: &CastVote| {
            certificate_release(
                lock.hash,
                &self.statements(node, lock.round, lock.height),
                self.total,
            )
        };
        let choice = choose_vote(
            &self.nodes[node].ledger,
            round,
            target,
            candidate,
            related,
            certificate,
        );
        if let Some(lock) = self.nodes[node].ledger.lock_at(target) {
            // A locked node waits for the statements of the round of its lock
            // while they leave the answer of the certificate open, up to the
            // end of the round, the way the node waits.
            let statements = self.statements(node, lock.round, lock.height);
            let elapsed = self.now.saturating_sub(self.nodes[node].round_started);
            if certificate_undecided(lock.hash, &statements, self.total)
                && elapsed + 1 < self.shape.round_timeout
            {
                return;
            }
        }
        if let VoteChoice::Released { .. } = choice {
            for lock in blocking_locks(&self.nodes[node].ledger, related) {
                self.nodes[node].ledger.release(&lock);
            }
        }
        let Some(cast) = choice.as_vote(round) else {
            return;
        };
        self.nodes[node]
            .ledger
            .record(cast)
            .expect("the lock never asks for a second vote in one round");
        // The ledger reaches the disk before the vote leaves the node.
        let bytes = self.nodes[node].ledger.encode();
        self.nodes[node].disk = bytes;
        self.send(
            node,
            Message::Vote {
                from: node,
                round,
                height: cast.height,
                hash: cast.hash,
            },
            rng,
        );
    }

    fn tick(&mut self, rng: &mut Rng) {
        if let Some(arrivals) = self.queue.remove(&self.now) {
            let mut arrivals = arrivals;
            // Arrivals in one tick land in a scrambled order.
            for index in (1..arrivals.len()).rev() {
                let other = rng.below(index as u64 + 1) as usize;
                arrivals.swap(index, other);
            }
            for (to, message, epoch) in arrivals {
                self.deliver(to, message, epoch, rng);
            }
        }
        if !self.healed {
            for node in 0..self.nodes.len() {
                if rng.chance(self.shape.reorg_per_thousand) {
                    self.nodes[node].fork ^= 1;
                }
                if self.nodes[node].down_until <= self.now
                    && rng.chance(self.shape.restart_per_thousand)
                {
                    let down = 1 + rng.below(self.shape.round_timeout * 2);
                    self.nodes[node].down_until = self.now + down;
                    self.nodes[node].restart(self.now + down);
                }
            }
        }
        let mut order: Vec<usize> = (0..self.nodes.len()).collect();
        for index in (1..order.len()).rev() {
            let other = rng.below(index as u64 + 1) as usize;
            order.swap(index, other);
        }
        for node in order {
            self.act(node, rng);
        }
        self.now += 1;
    }

    /// End the fault. Every validator comes back and the best chains agree.
    fn heal(&mut self) {
        self.healed = true;
        let now = self.now;
        for node in &mut self.nodes {
            if node.down_until > now {
                node.down_until = now;
            }
            node.fork = 0;
        }
    }

    fn all_committed(&self) -> bool {
        self.nodes
            .iter()
            .all(|node| (1..=self.shape.heights).all(|height| node.committed.contains_key(&height)))
    }
}

/// The rounds each height needs once the delays end, at the most.
const ROUNDS_TO_RECOVER_PER_HEIGHT: u32 = 2;

/// The rounds the network needs to gather after the delays end, before the
/// first height commits.
const ROUNDS_TO_GATHER: u32 = 2;

/// What one schedule reached.
struct Outcome {
    /// The rounds the network needed after the delays ended.
    rounds_after_heal: u32,
    /// The epochs the stall crossed before the delays ended.
    epochs_crossed: u64,
}

fn run_schedule(seed: u64, shape: Shape) -> Outcome {
    let mut rng = Rng::new(seed);
    let mut network = Network::new(shape, &mut rng);
    network.seed = seed;
    while network.now < shape.fault_ticks {
        network.tick(&mut rng);
    }
    let epochs_crossed = network.epoch_now();
    let round_at_heal = network
        .nodes
        .iter()
        .map(|node| node.round)
        .max()
        .unwrap_or(0);
    network.heal();
    let limit = network.now + 60 * shape.round_timeout * shape.heights as u64 + 200;
    while !network.all_committed() {
        if network.now + 1 >= limit {
            for (index, node) in network.nodes.iter().enumerate() {
                let round = node.round.saturating_sub(1);
                eprintln!(
                    "node {} weight {} fork {} round {} started {} now {} locks {:?} statements {:?} abstained {}",
                    index,
                    network.weights[index],
                    node.fork,
                    node.round,
                    node.round_started,
                    network.now,
                    node.ledger.locks().iter().map(|l| (l.round, l.hash[0])).collect::<Vec<_>>(),
                    node.timeouts.get(&(round, node.target())),
                    node.ledger.abstained_in(node.round)
                );
            }
        }
        assert!(
            network.now < limit,
            "seed {}: progress stalled after the delays ended at tick {}; commitments {:?}; nodes {:?}",
            seed,
            shape.fault_ticks,
            network.commitments,
            network
                .nodes
                .iter()
                .map(|node| (node.target(), node.round, node.ledger.locks()))
                .collect::<Vec<_>>()
        );
        network.tick(&mut rng);
    }
    let round_done = network
        .nodes
        .iter()
        .map(|node| node.round)
        .max()
        .unwrap_or(0);
    let rounds_after_heal = round_done.saturating_sub(round_at_heal);
    assert!(
        rounds_after_heal <= ROUNDS_TO_RECOVER_PER_HEIGHT * shape.heights + ROUNDS_TO_GATHER,
        "seed {}: recovery took {} rounds after the delays ended",
        seed,
        rounds_after_heal
    );
    Outcome {
        rounds_after_heal,
        epochs_crossed,
    }
}

/// No two conflicting commitments under thousands of adversarial schedules,
/// and progress resumes once the delays end.
#[test]
fn no_conflicting_commitment_and_progress_after_delays_end() {
    let mut schedules = 0u32;
    let mut crossed = 0u64;
    for seed in 1..=5_000u64 {
        let mut pick = Rng::new(seed ^ 0xC0FFEE);
        let round_timeout = 4 + pick.below(6);
        let shape = Shape {
            validators: 4 + pick.below(7) as usize,
            heights: 1 + pick.below(3) as u32,
            fault_ticks: 20 + pick.below(120),
            max_delay: 1 + pick.below(round_timeout * 3),
            round_timeout,
            reorg_per_thousand: pick.below(250),
            restart_per_thousand: pick.below(40),
            unequal_weights: pick.chance(500),
            epoch_ticks: 10 + pick.below(50),
            slow_machine: None,
        };
        let outcome = run_schedule(seed, shape);
        crossed += u64::from(outcome.epochs_crossed > 0);
        schedules += 1;
    }
    // Most schedules carry the stall across at least one epoch boundary.
    assert!(
        crossed * 2 > u64::from(schedules),
        "{} of {}",
        crossed,
        schedules
    );
    assert_eq!(schedules, 5_000);
}

/// The nine validator delay exercise, replayed.
///
/// Nine validators of equal weight: 31995 in total and 21437 to commit, so
/// seven votes commit and six do not. In round 142 at height 1440, validators
/// one to five and eight first see block e88e, then reorganise to block a3ef.
/// Validators six and seven vote for e88e. Validator nine votes for a3ef.
/// Nodes that hear the e88e votes first and nodes that hear the a3ef votes
/// first then face different vote sets.
struct Replay {
    first: [u8; 32],
    second: [u8; 32],
}

impl Replay {
    fn new() -> Self {
        let mut first = [0u8; 32];
        first[0] = 0xe8;
        first[1] = 0x8e;
        let mut second = [0u8; 32];
        second[0] = 0xa3;
        second[1] = 0xef;
        Replay { first, second }
    }

    /// The votes each validator signs in round 142, in signing order.
    fn signed(&self, locked: bool) -> Vec<Vec<[u8; 32]>> {
        let switchers = [0usize, 1, 2, 3, 4, 7];
        (0..9)
            .map(|validator| {
                if switchers.contains(&validator) {
                    let mut ledger = VoteLedger::new();
                    let mut signed = Vec::new();
                    for candidate in [self.first, self.second] {
                        if locked {
                            let choice = choose_vote(
                                &ledger,
                                142,
                                1440,
                                candidate,
                                |lock: &CastVote| lock.hash == candidate,
                                |_| None,
                            );
                            let cast = choice.as_vote(142).expect("the node votes");
                            ledger.record(cast).expect("one vote per round");
                            if !signed.contains(&cast.hash) {
                                signed.push(cast.hash);
                            }
                        } else {
                            // The earlier vote creator signed on every import.
                            signed.push(candidate);
                        }
                    }
                    signed
                } else if validator == 8 {
                    vec![self.second]
                } else {
                    vec![self.first]
                }
            })
            .collect()
    }

    /// The commitments two nodes reach when one hears the first block first
    /// and the other hears the second block first.
    fn commitments(&self, locked: bool) -> BTreeSet<[u8; 32]> {
        let signed = self.signed(locked);
        let mut committed = BTreeSet::new();
        for prefer in [self.first, self.second] {
            // A node keeps the first vote it hears from each validator in the
            // round, the way the vote pool does.
            let mut heard: BTreeMap<usize, [u8; 32]> = BTreeMap::new();
            for (validator, votes) in signed.iter().enumerate() {
                let pick = votes
                    .iter()
                    .find(|vote| **vote == prefer)
                    .or_else(|| votes.first())
                    .copied()
                    .expect("every validator signs");
                heard.insert(validator, pick);
            }
            for block in [self.first, self.second] {
                let signed_weight =
                    heard.values().filter(|vote| **vote == block).count() as u128 * 3555;
                if signed_weight * 100 >= 31995 * THRESHOLD_PERCENT {
                    committed.insert(block);
                }
            }
        }
        committed
    }
}

#[test]
fn the_recorded_delay_exercise_commits_one_block() {
    let replay = Replay::new();

    // The earlier vote path: six validators sign both blocks in round 142,
    // and the two node groups commit two different blocks.
    let before = replay.commitments(false);
    assert_eq!(before.len(), 2, "the replay reproduces the recorded fault");

    // The lock: each validator signs one vote in round 142.
    for votes in replay.signed(true) {
        assert_eq!(votes.len(), 1, "one vote per validator per round");
    }
    let after = replay.commitments(true);
    assert_eq!(after.len(), 1, "one block commits at height 1440");
    assert!(after.contains(&replay.first));
}

/// A restart in the middle of round 142 leaves the vote of that round in place.
#[test]
fn a_restart_mid_round_repeats_the_vote_on_disk() {
    let replay = Replay::new();
    let mut ledger = VoteLedger::new();
    let first = choose_vote(&ledger, 142, 1440, replay.first, |_| true, |_| None)
        .as_vote(142)
        .expect("the node votes");
    ledger.record(first).unwrap();
    let disk = ledger.encode();

    // The node restarts and reorganises to the other block.
    let reopened = VoteLedger::decode(&mut &disk[..]).unwrap();
    let choice = choose_vote(
        &reopened,
        142,
        1440,
        replay.second,
        |lock: &CastVote| lock.hash == replay.second,
        |_| Some(None),
    );
    assert_eq!(choice, VoteChoice::Repeat(first));

    // In round 143 the lock holds without a certificate.
    let choice = choose_vote(
        &reopened,
        143,
        1440,
        replay.second,
        |lock: &CastVote| lock.hash == replay.second,
        |_| None,
    );
    assert_eq!(
        choice,
        VoteChoice::Locked {
            height: 1440,
            hash: replay.first
        }
    );
}

/// The second delay exercise, replayed.
///
/// Nine validators of equal weight run on three machines of three. One tick
/// stands for 250 milliseconds. For three minutes every message to or from
/// the second machine runs 500 milliseconds late one way, a second round trip,
/// and the best chains of its validators fork from the others. The stall
/// crosses an epoch boundary. Once the delay ends, every height commits within
/// the round bound and no two blocks commit at one height.
fn recorded_delay_shape() -> Shape {
    Shape {
        validators: 9,
        heights: 6,
        fault_ticks: 720,
        max_delay: 1,
        round_timeout: 24,
        reorg_per_thousand: 60,
        restart_per_thousand: 0,
        unequal_weights: false,
        epoch_ticks: 400,
        slow_machine: Some((3, 6, 2)),
    }
}

#[test]
fn the_recorded_one_second_delay_recovers_after_an_epoch_boundary() {
    for seed in 1..=64u64 {
        let outcome = run_schedule(seed, recorded_delay_shape());
        assert!(outcome.epochs_crossed >= 1, "seed {seed}");
        assert!(
            outcome.rounds_after_heal <= ROUNDS_TO_RECOVER_PER_HEIGHT * 6 + ROUNDS_TO_GATHER,
            "seed {seed}"
        );
    }
}
