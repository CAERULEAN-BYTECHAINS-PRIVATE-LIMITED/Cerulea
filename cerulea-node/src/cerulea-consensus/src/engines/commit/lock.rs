//! The Commit Engine lock.
//!
//! The Commit Engine commits a checkpoint when validators holding at least
//! sixty seven percent of the Trust Score weight vote for one block in one
//! round. A round is one pass of voting. The chain opens a new round when a
//! checkpoint commits or when the round timeout runs out. A vote for a block
//! also covers every ancestor of that block, since committing a block commits
//! the chain beneath it.
//!
//! # The two rules
//!
//! 1. **One vote per round.** A validator signs at most one vote in each
//!    round. A second request in the same round repeats the vote already
//!    signed, byte for byte, whatever block the node now sees. The node
//!    records every vote on disk before it signs, so a restart or a
//!    reorganisation never produces a second, different vote.
//! 2. **The lock.** A validator that voted for block X at height H is locked
//!    on X. In every later round it votes only for X or for a block on the
//!    same chain as X. It moves to a block off that chain only while it holds
//!    a round change certificate for the round of its lock. The certificate is
//!    a set of signed round timeout messages for that round and that height,
//!    from validators holding more than one third of the Trust Score weight,
//!    each stating that the signer saw no commitment and naming the block the
//!    signer was locked on at that height as of that round, where that block
//!    is something other than X or nothing at all. Without
//!    the certificate the validator repeats its locked vote. With it, the
//!    validator votes for the block the most declared weight stands behind,
//!    so a split round gathers behind one block in the next round.
//!
//! # Why two conflicting blocks never both gather their threshold
//!
//! Suppose block X gathers the threshold at height H in round r. Validators
//! holding at least sixty seven percent of the weight signed a vote for X in
//! round r, and each is locked on X.
//!
//! 1. **Round r.** A certificate against X for round r needs more than one
//!    third of the weight declaring a lock other than X as of round r. Every
//!    validator that voted X in round r is locked on X as of round r, and a
//!    declaration names the lock the ledger of its signer holds, so at most
//!    thirty three percent of the weight declares otherwise. No certificate
//!    against X for round r exists.
//! 2. **Every later round.** A validator locked on X leaves that lock only
//!    with a certificate against X for the round of its lock, which is round r
//!    or later. Take the earliest round after r in which any member of the
//!    locked set left X. Up to that round every member still declares X,
//!    whether it voted in a round or stayed silent, since the declaration
//!    names the lock rather than the vote. So no round in that span carries a
//!    certificate against X, and no member could have left. By induction the
//!    locked set never leaves the chain of X.
//! 3. **The conflicting block.** A block Y off the chain of X can therefore
//!    gather at most the thirty three percent outside the locked set in any
//!    round. The threshold asks for sixty seven. Y never commits, and neither
//!    does any block built on Y.
//!
//! A round timeout statement is a vote too, in the sense that matters here.
//! A validator that states it cast no vote in a round records that statement
//! in its ledger before it signs, and never votes in that round afterwards,
//! even after a reorganisation onto a fork where that round is still open.
//!
//! A validator that breaks either rule signs two statements that contradict
//! each other. Two votes from one validator in one round are equivocation
//! evidence. The node keeps the pair and reports it to the chain, and the
//! chain returns the standing of that validator to zero and records the
//! fault. The argument holds while the weight of validators that sign
//! contradicting statements stays below the gap between the two counts in the
//! first step, and every such signature leaves evidence behind.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::{Path, PathBuf};

use codec::{Decode, Encode};

/// The number of past rounds a node states its vote for.
///
/// A locked validator needs the statements of the round of its lock. Every
/// node therefore sends its statements again for the rounds since the last
/// commitment it saw, up to this many rounds back.
pub const STATEMENT_WINDOW_ROUNDS: u32 = 64;

/// The domain tag every round timeout signature carries.
///
/// The tag keeps a timeout signature apart from a vote signature, so neither
/// passes for the other.
pub const ROUND_TIMEOUT_DOMAIN: &[u8] = b"cerulea.commit.round-timeout.v1";

/// One vote this validator signed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct CastVote {
    /// The round the vote belongs to.
    pub round: u32,
    /// The checkpoint height the vote names.
    pub height: u32,
    /// The block the vote names.
    pub hash: [u8; 32],
}

/// What the lock allows the node to sign.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoteChoice {
    /// No lock stands in the way. The node votes for its candidate.
    Fresh {
        /// The checkpoint height.
        height: u32,
        /// The candidate block.
        hash: [u8; 32],
    },
    /// The node already signed a vote in this round. It repeats that vote.
    Repeat(CastVote),
    /// A lock without a certificate stands in the way. The node votes again
    /// for its locked block, at the height of the lock.
    Locked {
        /// The height of the lock.
        height: u32,
        /// The locked block.
        hash: [u8; 32],
    },
    /// A round change certificate released every lock in the way. The node
    /// votes for the block the certificate stands behind, or its candidate
    /// where the certificate names none.
    Released {
        /// The checkpoint height.
        height: u32,
        /// The block the node moves to.
        hash: [u8; 32],
    },
    /// The node already stated that it cast no vote in this round. It signs
    /// nothing.
    Silent,
}

impl VoteChoice {
    /// The height and the block the node signs for, if it signs at all.
    pub fn target(&self) -> Option<(u32, [u8; 32])> {
        match self {
            VoteChoice::Fresh { height, hash }
            | VoteChoice::Locked { height, hash }
            | VoteChoice::Released { height, hash } => Some((*height, *hash)),
            VoteChoice::Repeat(vote) => Some((vote.height, vote.hash)),
            VoteChoice::Silent => None,
        }
    }

    /// The vote the choice asks the node to sign in one round, if any.
    pub fn as_vote(&self, round: u32) -> Option<CastVote> {
        match self {
            VoteChoice::Repeat(vote) => Some(*vote),
            other => other.target().map(|(height, hash)| CastVote {
                round,
                height,
                hash,
            }),
        }
    }
}

/// A problem the ledger reports.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LedgerError {
    /// The ledger states that the node cast no vote in this round.
    AbstainedRound {
        /// The round.
        round: u32,
    },
    /// The ledger already holds a different vote for this round.
    ConflictingVote {
        /// The vote the ledger holds.
        held: CastVote,
        /// The vote the node asked to record.
        asked: CastVote,
    },
    /// The ledger file failed to read or write.
    Storage(String),
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::ConflictingVote { held, asked } => write!(
                f,
                "The node already signed a vote for #{} in round {} and was asked to sign one for #{}. Fix: repeat the vote already signed.",
                held.height, held.round, asked.height
            ),
            LedgerError::AbstainedRound { round } => write!(
                f,
                "The node stated that it cast no vote in round {}. Fix: vote in a later round.",
                round
            ),
            LedgerError::Storage(detail) => write!(
                f,
                "The vote ledger failed to read or write ({}). Fix: check that the node base path is writable.",
                detail
            ),
        }
    }
}

/// Every vote this validator signed, by round.
///
/// The ledger also records the locks a round change certificate released. A
/// released lock at one height covers every vote there up to its round.
#[derive(Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct VoteLedger {
    votes: BTreeMap<u32, CastVote>,
    released: BTreeMap<u32, u32>,
    abstained: BTreeSet<u32>,
    /// The highest round the ledger dropped a record for. The node never
    /// votes at or below this round again, so a pruned record never hides an
    /// earlier vote.
    floor: Option<u32>,
}

impl VoteLedger {
    /// Start an empty ledger.
    pub fn new() -> Self {
        VoteLedger::default()
    }

    /// The vote signed in one round, if any.
    pub fn vote_in_round(&self, round: u32) -> Option<CastVote> {
        self.votes.get(&round).copied()
    }

    /// Every vote the ledger holds, in round order.
    pub fn votes(&self) -> impl Iterator<Item = &CastVote> {
        self.votes.values()
    }

    /// The lock at every height, as the vote with the highest round there.
    ///
    /// A vote a certificate released holds no lock.
    pub fn locks(&self) -> Vec<CastVote> {
        let mut by_height: BTreeMap<u32, CastVote> = BTreeMap::new();
        for vote in self.votes.values() {
            if self
                .released
                .get(&vote.height)
                .map(|round| vote.round <= *round)
                .unwrap_or(false)
            {
                continue;
            }
            let keep = by_height
                .get(&vote.height)
                .map(|held| vote.round > held.round)
                .unwrap_or(true);
            if keep {
                by_height.insert(vote.height, *vote);
            }
        }
        by_height.into_values().collect()
    }

    /// The lock at one height.
    pub fn lock_at(&self, height: u32) -> Option<CastVote> {
        self.locks().into_iter().find(|lock| lock.height == height)
    }

    /// Record one vote.
    ///
    /// The ledger refuses a vote that differs from the one already recorded
    /// for the same round. Recording the same vote twice changes nothing.
    pub fn record(&mut self, vote: CastVote) -> Result<(), LedgerError> {
        if self.abstained.contains(&vote.round) || self.below_floor(vote.round) {
            return Err(LedgerError::AbstainedRound { round: vote.round });
        }
        if let Some(held) = self.votes.get(&vote.round) {
            if *held != vote {
                return Err(LedgerError::ConflictingVote {
                    held: *held,
                    asked: vote,
                });
            }
            return Ok(());
        }
        self.votes.insert(vote.round, vote);
        Ok(())
    }

    /// Record that the node cast no vote in one round.
    ///
    /// The node records this before it signs a round timeout that names no
    /// vote, and never votes in that round afterwards.
    pub fn abstain(&mut self, round: u32) -> Result<(), LedgerError> {
        if self.below_floor(round) {
            return Ok(());
        }
        if let Some(held) = self.votes.get(&round) {
            return Err(LedgerError::ConflictingVote {
                held: *held,
                asked: *held,
            });
        }
        self.abstained.insert(round);
        Ok(())
    }

    /// Report whether the node stated that it cast no vote in one round.
    pub fn abstained_in(&self, round: u32) -> bool {
        self.abstained.contains(&round)
    }

    /// Report whether a round sits at or below the highest dropped record.
    pub fn below_floor(&self, round: u32) -> bool {
        self.floor.map(|floor| round <= floor).unwrap_or(false)
    }

    /// The highest round the ledger holds any record of, dropped or kept.
    pub fn highest_round(&self) -> Option<u32> {
        let kept = self.votes.keys().next_back().copied();
        let abstained = self.abstained.iter().next_back().copied();
        [kept, abstained, self.floor].into_iter().flatten().max()
    }

    fn raise_floor(&mut self, round: u32) {
        self.floor = Some(self.floor.map(|held| held.max(round)).unwrap_or(round));
    }

    /// The statement the node makes for one round at one height.
    ///
    /// The statement names the block the node was locked on at that height as
    /// of that round: its latest vote there in that round or any earlier one.
    /// A node that voted nowhere at that height states nothing. The statement
    /// names the lock rather than the vote, so a locked node that stayed
    /// silent in a round still counts for its lock.
    pub fn statement_for(&self, round: u32, height: u32) -> Option<[u8; 32]> {
        self.votes
            .range(..=round)
            .rev()
            .map(|(_, vote)| vote)
            .find(|vote| vote.height == height)
            .map(|vote| vote.hash)
    }

    /// Release one lock a round change certificate answered.
    pub fn release(&mut self, lock: &CastVote) {
        let round = self.released.entry(lock.height).or_insert(lock.round);
        if lock.round > *round {
            *round = lock.round;
        }
    }

    /// Drop every vote at or below a committed height.
    ///
    /// A committed height needs no lock, since the finalized head only rises.
    pub fn prune_through(&mut self, committed_height: u32) {
        let dropped = self
            .votes
            .values()
            .filter(|vote| vote.height <= committed_height)
            .map(|vote| vote.round)
            .max();
        if let Some(round) = dropped {
            self.raise_floor(round);
        }
        self.votes.retain(|_, vote| vote.height > committed_height);
        self.released.retain(|height, _| *height > committed_height);
        // The ledger keeps the most recent abstained rounds, four statement
        // windows of them, so the file stays small.
        let keep = (STATEMENT_WINDOW_ROUNDS as usize) * 4;
        while self.abstained.len() > keep {
            if let Some(oldest) = self.abstained.iter().next().copied() {
                self.abstained.remove(&oldest);
                self.raise_floor(oldest);
            }
        }
    }

    /// The number of votes the ledger holds.
    pub fn len(&self) -> usize {
        self.votes.len()
    }

    /// Report whether the ledger holds no vote.
    pub fn is_empty(&self) -> bool {
        self.votes.is_empty()
    }
}

/// One round timeout statement, as the certificate check reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimeoutStatement {
    /// The signer, as its account bytes.
    pub signer: [u8; 32],
    /// The Trust Score weight of the signer.
    pub weight: u128,
    /// The vote the signer cast at that height in that round, if any.
    pub voted: Option<[u8; 32]>,
}

/// Check a round change certificate against one locked block.
///
/// The statements belong to the round and the height of the lock. Each signer
/// counts once. The certificate holds where the signers declaring a vote other
/// than the locked block carry more than one third of the total weight. The
/// answer then names the block the most declared weight stands behind, the
/// locked block included, or nothing where no statement names a block.
pub fn certificate_release(
    locked: [u8; 32],
    statements: &[TimeoutStatement],
    total_weight: u128,
) -> Option<Option<[u8; 32]>> {
    if total_weight == 0 {
        return None;
    }
    let mut counted: BTreeMap<[u8; 32], (u128, Option<[u8; 32]>)> = BTreeMap::new();
    for statement in statements {
        counted
            .entry(statement.signer)
            .or_insert((statement.weight, statement.voted));
    }
    let against: u128 = counted
        .values()
        .filter(|(_, voted)| *voted != Some(locked))
        .map(|(weight, _)| *weight)
        .fold(0u128, |sum, weight| sum.saturating_add(weight));
    if against.saturating_mul(3) <= total_weight {
        return None;
    }
    let mut behind: BTreeMap<[u8; 32], u128> = BTreeMap::new();
    for (weight, voted) in counted.values() {
        if let Some(block) = voted {
            let held = behind.entry(*block).or_default();
            *held = held.saturating_add(*weight);
        }
    }
    let best = behind
        .into_iter()
        .max_by(|left, right| left.1.cmp(&right.1).then(right.0.cmp(&left.0)))
        .map(|(block, _)| block);
    Some(best)
}

/// Report whether a round change certificate against one locked block holds.
pub fn certificate_holds(
    locked: [u8; 32],
    statements: &[TimeoutStatement],
    total_weight: u128,
) -> bool {
    certificate_release(locked, statements, total_weight).is_some()
}

/// The locks that stand between the node and its candidate.
pub fn blocking_locks(ledger: &VoteLedger, related: impl Fn(&CastVote) -> bool) -> Vec<CastVote> {
    ledger
        .locks()
        .into_iter()
        .filter(|lock| !related(lock))
        .collect()
}

/// Decide what the node signs in one round.
///
/// * `target` and `candidate` name the checkpoint the node would vote for on
///   its best chain.
/// * `related` answers whether one lock lies on the same chain as the
///   candidate, as an ancestor, the block itself or a descendant.
/// * `certificate` answers the round change certificate against one lock:
///   nothing where it fails, and the block it stands behind where it holds.
pub fn choose_vote(
    ledger: &VoteLedger,
    round: u32,
    target: u32,
    candidate: [u8; 32],
    related: impl Fn(&CastVote) -> bool,
    certificate: impl Fn(&CastVote) -> Option<Option<[u8; 32]>>,
) -> VoteChoice {
    if let Some(held) = ledger.vote_in_round(round) {
        return VoteChoice::Repeat(held);
    }
    if ledger.abstained_in(round) || ledger.below_floor(round) {
        return VoteChoice::Silent;
    }
    let locks = ledger.locks();
    let mut held_back: Option<CastVote> = None;
    let mut blocked = false;
    let mut pointed: Option<(u32, u32, [u8; 32])> = None;
    for lock in &locks {
        let in_the_way = !related(lock);
        let answer = if lock.round > round {
            None
        } else {
            certificate(lock)
        };
        match answer {
            None if in_the_way => {
                if held_back
                    .map(|held| lock.round > held.round)
                    .unwrap_or(true)
                {
                    held_back = Some(*lock);
                }
            }
            None => {}
            Some(best) => {
                blocked |= in_the_way;
                // A certificate that stands behind another block moves the
                // node there, even where its lock sits on its own chain. A
                // split round then gathers behind one block.
                if let Some(block) = best.filter(|block| *block != lock.hash) {
                    if pointed
                        .map(|(held, _, _)| lock.round > held)
                        .unwrap_or(true)
                    {
                        pointed = Some((lock.round, lock.height, block));
                    }
                }
            }
        }
    }
    if let Some(lock) = held_back {
        return VoteChoice::Locked {
            height: lock.height,
            hash: lock.hash,
        };
    }
    match pointed {
        Some((_, height, block)) => VoteChoice::Released {
            height,
            hash: block,
        },
        None if blocked => VoteChoice::Released {
            height: target,
            hash: candidate,
        },
        None => VoteChoice::Fresh {
            height: target,
            hash: candidate,
        },
    }
}

/// The payload one round timeout signature covers.
///
/// `signer` is the encoded account of the signer.
pub fn round_timeout_payload(
    epoch_id: u32,
    validator_set_id: u32,
    round: u32,
    height: u32,
    voted: &Option<[u8; 32]>,
    signer: &[u8],
) -> Vec<u8> {
    let mut payload = ROUND_TIMEOUT_DOMAIN.to_vec();
    epoch_id.encode_to(&mut payload);
    validator_set_id.encode_to(&mut payload);
    round.encode_to(&mut payload);
    height.encode_to(&mut payload);
    voted.encode_to(&mut payload);
    payload.extend_from_slice(signer);
    payload
}

/// The ledger file the node keeps under its base path.
///
/// Every write lands in a temporary file first, reaches the disk, and then
/// replaces the ledger in one rename. A crash therefore leaves either the old
/// ledger or the new one, never a torn file.
#[derive(Clone, Debug)]
pub struct LedgerStore {
    path: PathBuf,
}

impl LedgerStore {
    /// The ledger file name under the node base path.
    pub const FILE_NAME: &'static str = "commit-engine-votes.scale";

    /// Open the store at one file path.
    pub fn at(path: impl Into<PathBuf>) -> Self {
        LedgerStore { path: path.into() }
    }

    /// Open the store under a node base path.
    pub fn under(base: &Path) -> Self {
        LedgerStore::at(base.join(Self::FILE_NAME))
    }

    /// The file path of the ledger.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Read the ledger. A missing file reads as an empty ledger.
    pub fn load(&self) -> Result<VoteLedger, LedgerError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => VoteLedger::decode(&mut &bytes[..])
                .map_err(|reason| LedgerError::Storage(format!("{:?}", reason))),
            Err(reason) if reason.kind() == std::io::ErrorKind::NotFound => Ok(VoteLedger::new()),
            Err(reason) => Err(LedgerError::Storage(reason.to_string())),
        }
    }

    /// Write the ledger to disk and wait for the disk to hold it.
    pub fn save(&self, ledger: &VoteLedger) -> Result<(), LedgerError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|reason| LedgerError::Storage(reason.to_string()))?;
        }
        let staging = self.path.with_extension("scale.next");
        let mut file = std::fs::File::create(&staging)
            .map_err(|reason| LedgerError::Storage(reason.to_string()))?;
        file.write_all(&ledger.encode())
            .map_err(|reason| LedgerError::Storage(reason.to_string()))?;
        file.sync_all()
            .map_err(|reason| LedgerError::Storage(reason.to_string()))?;
        drop(file);
        std::fs::rename(&staging, &self.path)
            .map_err(|reason| LedgerError::Storage(reason.to_string()))?;
        Ok(())
    }

    /// Record one vote and write the ledger before the node signs.
    ///
    /// The call reads the ledger from disk, refuses a conflicting vote, and
    /// writes the ledger back. The signature follows only after the write
    /// returns.
    pub fn record_before_signing(&self, vote: CastVote) -> Result<VoteLedger, LedgerError> {
        let mut ledger = self.load()?;
        ledger.record(vote)?;
        self.save(&ledger)?;
        Ok(ledger)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const X: [u8; 32] = [1u8; 32];
    const Y: [u8; 32] = [2u8; 32];

    fn vote(round: u32, height: u32, hash: [u8; 32]) -> CastVote {
        CastVote {
            round,
            height,
            hash,
        }
    }

    fn statement(index: u8, weight: u128, voted: Option<[u8; 32]>) -> TimeoutStatement {
        TimeoutStatement {
            signer: [index; 32],
            weight,
            voted,
        }
    }

    fn same_hash(candidate: [u8; 32]) -> impl Fn(&CastVote) -> bool {
        move |lock: &CastVote| lock.hash == candidate
    }

    #[test]
    fn a_second_request_in_one_round_repeats_the_first_vote() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(142, 1440, X)).unwrap();
        let choice = choose_vote(&ledger, 142, 1440, Y, same_hash(Y), |_| Some(None));
        assert_eq!(choice, VoteChoice::Repeat(vote(142, 1440, X)));
        assert_eq!(choice.target(), Some((1440, X)));
        assert_eq!(choice.as_vote(142), Some(vote(142, 1440, X)));
    }

    #[test]
    fn the_ledger_refuses_a_conflicting_vote_in_one_round() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(3, 30, X)).unwrap();
        ledger.record(vote(3, 30, X)).unwrap();
        assert!(matches!(
            ledger.record(vote(3, 30, Y)),
            Err(LedgerError::ConflictingVote { .. })
        ));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn a_locked_validator_repeats_its_block_without_a_certificate() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(4, 40, X)).unwrap();
        assert_eq!(
            choose_vote(&ledger, 5, 40, Y, same_hash(Y), |_| None),
            VoteChoice::Locked {
                height: 40,
                hash: X
            }
        );
    }

    #[test]
    fn a_certificate_releases_the_lock() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(4, 40, X)).unwrap();
        let choice = choose_vote(&ledger, 5, 40, Y, same_hash(Y), |lock| {
            assert_eq!(*lock, vote(4, 40, X));
            Some(None)
        });
        assert_eq!(
            choice,
            VoteChoice::Released {
                height: 40,
                hash: Y
            }
        );

        // A certificate that stands behind a block moves the node there.
        let choice = choose_vote(&ledger, 5, 40, Y, same_hash(Y), |_| Some(Some([7u8; 32])));
        assert_eq!(
            choice,
            VoteChoice::Released {
                height: 40,
                hash: [7u8; 32]
            }
        );
    }

    #[test]
    fn a_descendant_of_the_lock_needs_no_certificate() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(4, 40, X)).unwrap();
        let choice = choose_vote(&ledger, 5, 50, Y, |_| true, |_| None);
        assert_eq!(
            choice,
            VoteChoice::Fresh {
                height: 50,
                hash: Y
            }
        );
    }

    #[test]
    fn a_block_off_the_locked_chain_holds_the_lock() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(4, 40, X)).unwrap();
        ledger.record(vote(5, 50, Y)).unwrap();
        // The candidate at fifty sits off both locked chains. The latest lock
        // holds.
        let choice = choose_vote(&ledger, 6, 50, [7u8; 32], |_| false, |_| None);
        assert_eq!(
            choice,
            VoteChoice::Locked {
                height: 50,
                hash: Y
            }
        );
    }

    #[test]
    fn a_vote_in_an_earlier_round_than_the_lock_repeats_the_lock() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(9, 90, X)).unwrap();
        assert_eq!(
            choose_vote(&ledger, 8, 90, Y, same_hash(Y), |_| Some(None)),
            VoteChoice::Locked {
                height: 90,
                hash: X
            }
        );
    }

    #[test]
    fn the_certificate_needs_more_than_one_third_against_the_lock() {
        // Nine validators of equal weight, a total of nine.
        let three: Vec<_> = (0..3).map(|index| statement(index, 1, Some(Y))).collect();
        assert!(!certificate_holds(X, &three, 9));
        let four: Vec<_> = (0..4).map(|index| statement(index, 1, Some(Y))).collect();
        assert!(certificate_holds(X, &four, 9));
        let four_silent: Vec<_> = (0..4).map(|index| statement(index, 1, None)).collect();
        assert!(certificate_holds(X, &four_silent, 9));
    }

    #[test]
    fn a_release_stands_behind_the_block_with_the_most_declared_weight() {
        let mut statements: Vec<_> = (0..6).map(|index| statement(index, 1, Some(X))).collect();
        statements.extend((6..9).map(|index| statement(index, 1, Some(Y))));
        // Against Y: six of nine declared otherwise. The certificate points to X.
        assert_eq!(certificate_release(Y, &statements, 9), Some(Some(X)));
        // Against X: three of nine is one third exactly, short of the rule.
        assert_eq!(certificate_release(X, &statements, 9), None);
    }

    #[test]
    fn a_declaration_for_the_locked_block_counts_for_the_lock() {
        let mut statements: Vec<_> = (0..6).map(|index| statement(index, 1, Some(X))).collect();
        statements.extend((6..9).map(|index| statement(index, 1, None)));
        assert!(!certificate_holds(X, &statements, 9));
    }

    #[test]
    fn a_signer_counts_once_in_a_certificate() {
        let statements: Vec<_> = (0..6).map(|_| statement(1, 1, Some(Y))).collect();
        assert!(!certificate_holds(X, &statements, 9));
    }

    #[test]
    fn the_ledger_survives_a_restart() {
        let folder = tempfile::tempdir().unwrap();
        let store = LedgerStore::under(folder.path());
        assert!(store.load().unwrap().is_empty());
        store.record_before_signing(vote(142, 1440, X)).unwrap();
        let reopened = LedgerStore::under(folder.path());
        let ledger = reopened.load().unwrap();
        assert_eq!(ledger.vote_in_round(142), Some(vote(142, 1440, X)));
        assert!(matches!(
            reopened.record_before_signing(vote(142, 1440, Y)),
            Err(LedgerError::ConflictingVote { .. })
        ));
        assert_eq!(
            choose_vote(
                &reopened.load().unwrap(),
                142,
                1440,
                Y,
                same_hash(Y),
                |_| Some(None)
            ),
            VoteChoice::Repeat(vote(142, 1440, X))
        );
    }

    #[test]
    fn a_released_lock_stays_released_across_rounds() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(3, 40, X)).unwrap();
        ledger.record(vote(4, 40, X)).unwrap();
        let blocking = blocking_locks(&ledger, same_hash(Y));
        assert_eq!(blocking, vec![vote(4, 40, X)]);
        for lock in &blocking {
            ledger.release(lock);
        }
        assert!(blocking_locks(&ledger, same_hash(Y)).is_empty());
        ledger.record(vote(5, 40, Y)).unwrap();
        assert_eq!(ledger.lock_at(40), Some(vote(5, 40, Y)));
        assert_eq!(
            choose_vote(&ledger, 6, 40, X, same_hash(X), |_| None),
            VoteChoice::Locked {
                height: 40,
                hash: Y
            }
        );
    }

    #[test]
    fn a_round_stated_as_silent_takes_no_vote() {
        let mut ledger = VoteLedger::new();
        ledger.abstain(8).unwrap();
        assert_eq!(
            choose_vote(&ledger, 8, 80, X, same_hash(X), |_| None),
            VoteChoice::Silent
        );
        assert_eq!(VoteChoice::Silent.as_vote(8), None);
        assert!(matches!(
            ledger.record(vote(8, 80, X)),
            Err(LedgerError::AbstainedRound { round: 8 })
        ));
        ledger.record(vote(9, 80, X)).unwrap();
        assert!(ledger.abstain(9).is_err());
        assert_eq!(ledger.statement_for(9, 80), Some(X));
        assert_eq!(ledger.statement_for(9, 90), None);
        // A silent round after the vote still states the lock.
        assert_eq!(ledger.statement_for(12, 80), Some(X));
        assert_eq!(ledger.statement_for(8, 80), None);
    }

    #[test]
    fn pruning_drops_committed_heights() {
        let mut ledger = VoteLedger::new();
        ledger.record(vote(1, 10, X)).unwrap();
        ledger.record(vote(2, 20, Y)).unwrap();
        ledger.prune_through(10);
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.lock_at(20), Some(vote(2, 20, Y)));
        assert_eq!(ledger.lock_at(10), None);

        // A dropped round never takes a vote again.
        assert!(ledger.below_floor(1));
        assert_eq!(ledger.highest_round(), Some(2));
        assert_eq!(
            choose_vote(&ledger, 1, 30, X, same_hash(X), |_| None),
            VoteChoice::Silent
        );
        assert!(ledger.record(vote(1, 30, X)).is_err());
    }

    #[test]
    fn the_timeout_payload_carries_its_own_domain() {
        let payload = round_timeout_payload(1, 2, 3, 4, &Some(X), &[9u8; 32]);
        assert!(payload.starts_with(ROUND_TIMEOUT_DOMAIN));
        assert_ne!(
            payload,
            round_timeout_payload(1, 2, 3, 4, &None, &[9u8; 32])
        );
    }
}
