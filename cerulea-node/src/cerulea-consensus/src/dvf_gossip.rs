//! Commit Engine vote gossip.
//!
//! Validators exchange two kinds of message on the commit protocol. A vote
//! names one block at one checkpoint height in one round. A round timeout
//! states that the signer saw no commitment in one round at one height and
//! names the block the signer was locked on at that height as of that round. The round timeouts of more than one
//! third of the Trust Score weight form the round change certificate the
//! Commit Engine lock reads. See [`crate::engines::commit::lock`].
//!
//! The pool holds one vote per validator per round. A second vote from the
//! same validator in the same round is refused. Where the second vote names a
//! different block, the pool keeps both votes as equivocation evidence, stops
//! counting that validator in that round and hands the pair to the aggregator,
//! which reports it to the chain.

use crate::engines::commit::gossip::MessageSet;
use crate::engines::commit::lock::round_timeout_payload;
use crate::metrics::DvfMetrics;
use codec::{Decode, Encode};
use log::{debug, info, warn};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use pallet_cerulea_dvf::DvfApi as RuntimeDvfApi;
use parking_lot::RwLock;
use sc_network::PeerId;
use sc_network_gossip::{MessageIntent, ValidationResult, ValidatorContext};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::ed25519;
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The protocol ID for Commit Engine Vote Gossiping.
pub const DVF_PROTOCOL_NAME: &str = "/cerulea/commit/2";

/// The structure of a Commit Engine Vote gossiped over the P2P network.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct DvfVoteMessage<Hash, AccountId> {
    /// Epoch ID
    pub epoch_id: u32,
    /// Validator set ID
    pub validator_set_id: u32,
    /// Round number
    pub round_number: u32,
    /// Block number
    pub block_number: u32,
    /// Block hash
    pub block_hash: Hash,
    /// Validator account ID
    pub validator_account_id: AccountId,
    /// Validator public key
    pub validator_public_key: ed25519::Public,
    /// Signature
    pub signature: ed25519::Signature,
}

/// One round timeout statement.
///
/// The signer states that it saw no commitment at one height in one round and
/// names the block it was locked on at that height as of that round, if any.
/// The signature
/// covers every field under its own domain tag, so a timeout never passes for
/// a vote.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct RoundTimeoutMessage<Hash, AccountId> {
    /// Epoch ID
    pub epoch_id: u32,
    /// Validator set ID
    pub validator_set_id: u32,
    /// The round that closed with no commitment the signer saw.
    pub round_number: u32,
    /// The checkpoint height the statement covers.
    pub block_number: u32,
    /// The block the signer was locked on at that height as of that round.
    pub voted: Option<Hash>,
    /// The signer account.
    pub validator_account_id: AccountId,
    /// The signer seal key.
    pub validator_public_key: ed25519::Public,
    /// The signature over the statement.
    pub signature: ed25519::Signature,
}

impl<Hash: Encode, AccountId: Encode> RoundTimeoutMessage<Hash, AccountId> {
    /// The bytes the signature covers.
    pub fn payload(&self) -> Vec<u8> {
        let voted = self.voted.as_ref().map(hash_bytes);
        round_timeout_payload(
            self.epoch_id,
            self.validator_set_id,
            self.round_number,
            self.block_number,
            &voted,
            &self.validator_account_id.encode(),
        )
    }
}

/// Read a block hash as thirty two bytes.
///
/// Every Cerulea block hash encodes as thirty two bytes. A shorter encoding
/// fills the tail with zeros.
pub fn hash_bytes<Hash: Encode>(hash: &Hash) -> [u8; 32] {
    let encoded = hash.encode();
    let mut bytes = [0u8; 32];
    let length = encoded.len().min(32);
    bytes[..length].copy_from_slice(&encoded[..length]);
    bytes
}

/// The messages the commit protocol carries.
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub enum CommitMessage<Hash, AccountId> {
    /// One vote.
    Vote(DvfVoteMessage<Hash, AccountId>),
    /// One round timeout statement.
    RoundTimeout(RoundTimeoutMessage<Hash, AccountId>),
}

/// The rounds a vote or a round timeout statement stays in the gossip store.
///
/// The lock reads statements as far back as its statement window, so the
/// store keeps messages that long. Older messages leave the store, and so do
/// messages at or below the committed head.
pub const GOSSIP_KEEP_ROUNDS: u32 = crate::engines::commit::lock::STATEMENT_WINDOW_ROUNDS;

/// The rounds a message is sent again to peers that already hold it.
///
/// The gossip engine repeats its store to every peer three times every two
/// seconds. Only the messages of the current and the previous round are
/// repeated, so a message lost in transit arrives again while the rest of the
/// store costs nothing.
pub const GOSSIP_REPEAT_ROUNDS: u32 = 1;

/// Report whether one message still belongs in the gossip store.
///
/// `committed` is the committed head and `round` the current round.
pub fn message_live<Hash, AccountId>(
    message: &CommitMessage<Hash, AccountId>,
    committed: u32,
    round: u32,
) -> bool {
    match message {
        CommitMessage::Vote(vote) => {
            vote.block_number > committed
                && vote.round_number.saturating_add(GOSSIP_KEEP_ROUNDS) >= round
        }
        CommitMessage::RoundTimeout(statement) => {
            statement.block_number > committed
                && statement.round_number.saturating_add(GOSSIP_KEEP_ROUNDS) >= round
        }
    }
}

/// Report whether the gossip store drops one encoded message.
///
/// A message the node cannot read leaves the store at once.
pub fn message_expired_bytes<Hash: Decode, AccountId: Decode>(
    data: &[u8],
    committed: u32,
    round: u32,
) -> bool {
    match CommitMessage::<Hash, AccountId>::decode(&mut &data[..]) {
        Ok(message) => !message_live(&message, committed, round),
        Err(_) => true,
    }
}

/// Report whether the gossip engine sends one encoded message to a peer.
///
/// A peer lacking the message always receives it. The periodic repeat to a
/// peer that already holds it covers the current and the previous round
/// alone.
pub fn message_allowed_bytes<Hash: Decode, AccountId: Decode>(
    intent: MessageIntent,
    data: &[u8],
    committed: u32,
    round: u32,
) -> bool {
    match intent {
        MessageIntent::Broadcast | MessageIntent::ForcedBroadcast => true,
        MessageIntent::PeriodicRebroadcast => {
            match CommitMessage::<Hash, AccountId>::decode(&mut &data[..]) {
                Ok(message) => message_repeated(&message, committed, round),
                Err(_) => false,
            }
        }
    }
}

/// Report whether one message is sent again to a peer that already holds it.
pub fn message_repeated<Hash, AccountId>(
    message: &CommitMessage<Hash, AccountId>,
    committed: u32,
    round: u32,
) -> bool {
    match message {
        CommitMessage::Vote(vote) => {
            vote.block_number > committed
                && vote.round_number.saturating_add(GOSSIP_REPEAT_ROUNDS) >= round
        }
        CommitMessage::RoundTimeout(statement) => {
            statement.block_number > committed
                && statement.round_number.saturating_add(GOSSIP_REPEAT_ROUNDS) >= round
        }
    }
}

/// The structure of a Commit Engine Justification containing a threshold of votes
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct DvfJustification<Hash, AccountId> {
    /// Round number for this justification
    pub round_number: u32,
    /// Block hash being justified
    pub block_hash: Hash,
    /// Collection of votes that justify this block
    pub votes: Vec<DvfVoteMessage<Hash, AccountId>>,
}

/// What the pool did with one offered vote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteOffer {
    /// The pool took the vote.
    Inserted,
    /// The pool already holds this exact vote. The repeat is refused.
    Repeated,
    /// The pool holds a different vote from the same validator in the same
    /// round. The pool kept both as equivocation evidence.
    Conflicting,
}

/// The answer gossip gives for one offered vote.
///
/// `offer` is what the pool did with the vote, or nothing where the checks
/// refused it: a stale round, an epoch outside the current range, a signer
/// outside the active set. A refused vote is discarded. The gossip engine
/// changes the standing of a peer for a kept or processed message, which
/// earns standing, and for the same bytes sent twice by one peer. A discarded
/// vote therefore costs the sending peer nothing. A peer that relays old or
/// repeated votes during a stall keeps its standing and its connection.
pub fn vote_verdict<H>(offer: Option<VoteOffer>, topic: H) -> ValidationResult<H> {
    match offer {
        Some(VoteOffer::Inserted) => ValidationResult::ProcessAndKeep(topic),
        Some(VoteOffer::Repeated) | Some(VoteOffer::Conflicting) | None => {
            ValidationResult::Discard
        }
    }
}

/// Two votes one validator signed in one round, naming different blocks.
pub type VoteEvidence<Hash, AccountId> = (
    DvfVoteMessage<Hash, AccountId>,
    DvfVoteMessage<Hash, AccountId>,
);

/// A thread-safe, in-memory pool storing Commit Engine votes.
pub struct DvfVotePool<Hash, AccountId> {
    votes: RwLock<VoteIndex<Hash, AccountId>>,
    participation: RwLock<HashMap<u32, HashSet<AccountId>>>,
    by_voter: RwLock<HashMap<(u32, AccountId), DvfVoteMessage<Hash, AccountId>>>,
    equivocators: RwLock<HashSet<(u32, AccountId)>>,
    evidence: RwLock<Vec<VoteEvidence<Hash, AccountId>>>,
    timeouts: RwLock<TimeoutIndex<Hash, AccountId>>,
}

/// Votes held for one round and one candidate block.
type VoteIndex<Hash, AccountId> = HashMap<(u32, Hash), Vec<DvfVoteMessage<Hash, AccountId>>>;

/// Round timeout statements held for one round and one height, by signer.
type TimeoutIndex<Hash, AccountId> =
    HashMap<(u32, u32), HashMap<AccountId, RoundTimeoutMessage<Hash, AccountId>>>;

impl<
        Hash: std::cmp::Eq + std::hash::Hash + Clone,
        AccountId: std::cmp::Eq + std::hash::Hash + Clone,
    > DvfVotePool<Hash, AccountId>
{
    /// Creates a new Commit Engine Vote Pool.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            votes: RwLock::new(HashMap::new()),
            participation: RwLock::new(HashMap::new()),
            by_voter: RwLock::new(HashMap::new()),
            equivocators: RwLock::new(HashSet::new()),
            evidence: RwLock::new(Vec::new()),
            timeouts: RwLock::new(HashMap::new()),
        })
    }

    /// Offer one vote to the pool.
    ///
    /// One validator holds one vote per round. A repeat of the held vote is
    /// refused. A different vote in the same round is equivocation. The pool
    /// keeps both votes as evidence and stops counting that validator in that
    /// round.
    pub fn offer_vote(&self, vote: DvfVoteMessage<Hash, AccountId>) -> VoteOffer {
        let key = (vote.round_number, vote.validator_account_id.clone());
        let mut by_voter = self.by_voter.write();
        if let Some(held) = by_voter.get(&key) {
            if held.block_number == vote.block_number && held.block_hash == vote.block_hash {
                return VoteOffer::Repeated;
            }
            let pair = (held.clone(), vote);
            drop(by_voter);
            self.equivocators.write().insert(key);
            let mut evidence = self.evidence.write();
            if evidence.len() < MAX_HELD_EVIDENCE {
                evidence.push(pair);
            }
            return VoteOffer::Conflicting;
        }
        by_voter.insert(key, vote.clone());
        drop(by_voter);

        self.participation
            .write()
            .entry(vote.round_number)
            .or_default()
            .insert(vote.validator_account_id.clone());
        self.votes
            .write()
            .entry((vote.round_number, vote.block_hash.clone()))
            .or_default()
            .push(vote);
        VoteOffer::Inserted
    }

    /// Inserts a vote into the pool.
    ///
    /// The answer reads true where the pool took the vote.
    pub fn insert_vote(&self, vote: DvfVoteMessage<Hash, AccountId>) -> bool {
        matches!(self.offer_vote(vote), VoteOffer::Inserted)
    }

    /// The vote one validator cast in one round, as the pool holds it.
    pub fn vote_of(
        &self,
        round: u32,
        validator: &AccountId,
    ) -> Option<DvfVoteMessage<Hash, AccountId>> {
        self.by_voter
            .read()
            .get(&(round, validator.clone()))
            .cloned()
    }

    /// Report whether one validator equivocated in one round.
    pub fn equivocated(&self, round: u32, validator: &AccountId) -> bool {
        self.equivocators
            .read()
            .contains(&(round, validator.clone()))
    }

    /// Take every evidence pair the pool holds.
    pub fn take_evidence(&self) -> Vec<VoteEvidence<Hash, AccountId>> {
        std::mem::take(&mut *self.evidence.write())
    }

    /// Offer one round timeout statement.
    ///
    /// The pool refuses a statement whose declared vote contradicts the vote
    /// it holds from the same signer in the same round at the same height, so
    /// a false declaration never counts toward a certificate.
    pub fn offer_timeout(&self, statement: RoundTimeoutMessage<Hash, AccountId>) -> bool {
        let key = (
            statement.round_number,
            statement.validator_account_id.clone(),
        );
        if self.equivocators.read().contains(&key) {
            return false;
        }
        if let Some(held) = self.by_voter.read().get(&key) {
            // A vote at the same height in the same round is the lock as of
            // that round, so the statement must name it.
            let declared_matches = held.block_number != statement.block_number
                || statement.voted.as_ref() == Some(&held.block_hash);
            if !declared_matches {
                return false;
            }
        }
        let mut timeouts = self.timeouts.write();
        let held = timeouts
            .entry((statement.round_number, statement.block_number))
            .or_default();
        if held.contains_key(&statement.validator_account_id) {
            return false;
        }
        held.insert(statement.validator_account_id.clone(), statement);
        true
    }

    /// The round timeout statements for one round at one height.
    pub fn timeouts_for(
        &self,
        round: u32,
        height: u32,
    ) -> Vec<RoundTimeoutMessage<Hash, AccountId>> {
        self.timeouts
            .read()
            .get(&(round, height))
            .map(|held| held.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Gets votes for a specific round and block hash.
    ///
    /// A validator the pool holds equivocation evidence against in that round
    /// counts for nothing.
    pub fn get_votes(&self, round: u32, block_hash: &Hash) -> Vec<DvfVoteMessage<Hash, AccountId>> {
        let equivocators = self.equivocators.read();
        let votes = self.votes.read();
        let mut seen: HashSet<AccountId> = HashSet::new();
        votes
            .get(&(round, block_hash.clone()))
            .map(|held| {
                held.iter()
                    .filter(|vote| {
                        !equivocators.contains(&(round, vote.validator_account_id.clone()))
                            && seen.insert(vote.validator_account_id.clone())
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Gets all candidate blocks (block hashes) for a specific round.
    pub fn get_candidate_blocks(&self, round: u32) -> Vec<Hash> {
        let votes = self.votes.read();
        votes
            .keys()
            .filter(|(r, _)| *r == round)
            .map(|(_, hash)| hash.clone())
            .collect()
    }

    /// Prunes the records of rounds older than the given round.
    ///
    /// The argument is a round number. A block number belongs in
    /// [`Self::prune_by_finalized_block`].
    pub fn prune_older_rounds(&self, oldest_kept_round: u32) {
        self.votes
            .write()
            .retain(|&(round, _), _| round >= oldest_kept_round);
        self.participation
            .write()
            .retain(|&round, _| round >= oldest_kept_round);
        self.by_voter
            .write()
            .retain(|(round, _), _| *round >= oldest_kept_round);
        self.equivocators
            .write()
            .retain(|(round, _)| *round >= oldest_kept_round);
        self.timeouts
            .write()
            .retain(|(round, _), _| *round >= oldest_kept_round);
    }

    /// Prunes votes for rounds older than (current_round - retention_rounds).
    ///
    /// This implements round-based pruning to prevent unbounded memory growth.
    /// Removes votes, the one-vote-per-round record and the timeout statements
    /// for old rounds.
    pub fn prune_by_round_age(&self, current_round: u32, retention_rounds: u32) -> usize {
        let cutoff_round = current_round.saturating_sub(retention_rounds);
        let initial_count = self.votes.read().len();
        self.prune_older_rounds(cutoff_round);
        initial_count - self.votes.read().len()
    }

    /// Prunes votes for blocks older than the finalized block number.
    ///
    /// The one-vote-per-round record stays until the round leaves the
    /// retention window, so a second vote in a round the pool pruned here is
    /// still refused.
    pub fn prune_by_finalized_block(&self, finalized_block_number: u32) -> usize {
        let mut votes = self.votes.write();
        let initial_count = votes.len();

        // Votes for the finalized checkpoint itself stay, so the aggregator can
        // still act on them during this pruning cycle.
        votes.retain(|_, vote_list| {
            vote_list.retain(|vote| vote.block_number >= finalized_block_number);
            !vote_list.is_empty()
        });
        self.timeouts
            .write()
            .retain(|(_, height), _| *height >= finalized_block_number);

        initial_count - votes.len()
    }

    /// Enforces maximum pool size by pruning oldest votes first.
    pub fn enforce_max_size(&self, max_size: usize) -> usize {
        let mut votes = self.votes.write();

        if votes.len() <= max_size {
            return 0;
        }

        let mut entries: Vec<_> = votes
            .iter()
            .map(|((round, hash), _)| (*round, hash.clone()))
            .collect();
        entries.sort_by_key(|(round, _)| *round);
        let to_remove = votes.len() - max_size;
        for (round, hash) in entries.iter().take(to_remove) {
            votes.remove(&(*round, hash.clone()));
        }

        to_remove
    }

    /// Gets the current size of the vote pool (number of vote entries).
    pub fn size(&self) -> usize {
        self.votes.read().len()
    }

    /// Gets the total number of votes across all entries.
    pub fn total_votes(&self) -> usize {
        self.votes.read().values().map(|v| v.len()).sum()
    }

    /// Clears all votes and participation tracking (used on validator set changes).
    pub fn clear(&self) {
        self.votes.write().clear();
        self.participation.write().clear();
        self.by_voter.write().clear();
        self.equivocators.write().clear();
        self.timeouts.write().clear();
    }
}

/// Check the length of one commit vote message before any decoding.
///
/// The message crosses two bounds. The propagation layer bound covers every
/// message on the path. The commit vote bound covers the one vote or round
/// timeout statement the message carries. A message longer than either bound
/// is refused on the length alone.
pub fn admit_vote_message(bytes: &[u8]) -> Result<(), crate::engines::EngineError> {
    crate::engines::bounds::admit(
        crate::engines::bounds::MessagePath::GossipMessage,
        bytes.len(),
    )?;
    crate::engines::bounds::admit(crate::engines::bounds::MessagePath::CommitVote, bytes.len())?;
    Ok(())
}

/// Report whether a vote of one epoch is still current.
///
/// A vote belongs to a height that has yet to commit, and the validator set
/// that holds that height stays valid for it until it commits. The vote
/// therefore stays current from the epoch of the committed head up to one
/// epoch past the epoch the node runs now, whatever epoch the wall clock has
/// reached in between.
pub fn epoch_is_current(vote_epoch: u32, committed_epoch: u32, current_epoch: u32) -> bool {
    vote_epoch >= committed_epoch.min(current_epoch)
        && vote_epoch <= current_epoch.saturating_add(1)
}

/// The most evidence pairs the pool holds before the aggregator reports them.
pub const MAX_HELD_EVIDENCE: usize = 1024;

/// The refusal reasons that hold a message for a later try.
///
/// Each one says the node lacks something it needs to decide: the block a
/// vote names, or an answer from the runtime. Neither says anything against
/// the message itself, so the message waits for the next block import.
pub const HOLD_REASONS: [&str; 2] = ["target_not_imported", "runtime_api_error"];

/// The most messages the gossip validator holds for a later try.
pub const MAX_HELD_MESSAGES: usize = 1024;

/// Messages held until the node can decide on them, oldest first.
#[derive(Debug, Default)]
pub struct HeldMessages {
    queue: std::collections::VecDeque<Vec<u8>>,
}

impl HeldMessages {
    /// An empty hold.
    pub fn new() -> Self {
        Self::default()
    }

    /// Hold one message and report whether it was new to the hold.
    ///
    /// A message already held stays once. At capacity the oldest message
    /// leaves first, so the hold stays inside a stated ceiling.
    pub fn hold(&mut self, bytes: Vec<u8>) -> bool {
        if self.queue.iter().any(|held| *held == bytes) {
            return false;
        }
        if self.queue.len() >= MAX_HELD_MESSAGES {
            self.queue.pop_front();
        }
        self.queue.push_back(bytes);
        true
    }

    /// Take every held message, oldest first, and leave the hold empty.
    pub fn take_all(&mut self) -> Vec<Vec<u8>> {
        self.queue.drain(..).collect()
    }

    /// The number of messages held.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Report whether the hold is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

/// Validator for Commit Engine gossip messages.
/// Filters, validates, and manages routing of commit protocol messages across peers.
pub struct DvfGossipValidator<B: BlockT, C, AccountId> {
    pool: Arc<DvfVotePool<B::Hash, AccountId>>,
    /// The messages this node has already seen.
    ///
    /// The set carries a capacity limit with least recently used eviction,
    /// so node memory stays inside a stated ceiling whatever the peers send.
    known_messages: RwLock<MessageSet<B::Hash>>,
    /// Messages the node could not decide on yet, tried again on import.
    held: RwLock<HeldMessages>,
    /// The committed head and the current round, which decide what the
    /// gossip store keeps and repeats.
    window: RwLock<(u32, u32)>,
    client: Arc<C>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<B, C, AccountId> DvfGossipValidator<B, C, AccountId>
where
    B: BlockT,
    AccountId: std::cmp::Eq + std::hash::Hash + Clone + Encode + Decode + std::fmt::Debug,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>
        + RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>,
{
    /// Creates a new Commit Engine Gossip Validator.
    pub fn new(pool: Arc<DvfVotePool<B::Hash, AccountId>>, client: Arc<C>) -> Self {
        Self {
            pool,
            known_messages: RwLock::new(MessageSet::new()),
            held: RwLock::new(HeldMessages::new()),
            window: RwLock::new((0, 0)),
            client,
            metrics: None,
        }
    }

    /// Record the committed head and the current round.
    ///
    /// The node calls this after each import and each commitment. The gossip
    /// store then drops what fell below the window and repeats only the
    /// current round.
    pub fn set_window(&self, committed: u32, round: u32) {
        *self.window.write() = (committed, round);
    }

    /// The committed head and the current round the store reads.
    pub fn window(&self) -> (u32, u32) {
        *self.window.read()
    }

    /// The number of messages held for a later try.
    pub fn held_count(&self) -> usize {
        self.held.read().len()
    }

    /// The epoch range a vote must fall in: the epoch of the committed head
    /// and the epoch at the best block.
    ///
    /// A runtime API instance answers for one block only, so each block gets
    /// its own instance. Where the committed head is the best block, as at
    /// genesis, one answer serves both. Where the committed head cannot answer,
    /// the range falls back to the epoch at the best block, the narrower rule.
    fn epoch_window(&self, best_hash: B::Hash) -> Result<(u32, u32), String> {
        let current =
            <C::Api as RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>>::get_current_epoch(
                &self.client.runtime_api(),
                best_hash,
            )
            .map_err(|reason| format!("the epoch at the best block: {:?}", reason))?;
        let finalized_hash = self.client.info().finalized_hash;
        if finalized_hash == best_hash {
            return Ok((current, current));
        }
        let committed =
            match <C::Api as RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>>::get_current_epoch(
                &self.client.runtime_api(),
                finalized_hash,
            ) {
                Ok(epoch) => epoch,
                Err(reason) => {
                    debug!(
                    "Commit Engine Gossip: the epoch at the committed head is unavailable ({:?}); the epoch at the best block decides",
                    reason
                );
                    current
                }
            };
        Ok((committed, current))
    }

    /// Sets the metrics for this validator
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    fn reject(&self, reason: &'static str) -> &'static str {
        if let Some(ref metrics) = self.metrics {
            metrics.record_vote_rejection(reason);
        }
        reason
    }

    /// Check that one signer belongs to the active set in the stated epoch and
    /// validator set.
    fn check_membership(
        &self,
        epoch_id: u32,
        validator_set_id: u32,
        validator: &AccountId,
    ) -> Result<(), &'static str> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        match api.get_active_validators(best_hash) {
            Ok(active_validators) => {
                if !active_validators.contains(validator) {
                    warn!(
                        "Commit Engine Gossip: message from inactive validator {:?}",
                        validator
                    );
                    return Err(self.reject("inactive_validator"));
                }
            }
            Err(e) => {
                warn!(
                    "Commit Engine Gossip: Failed to fetch active validators: {:?}",
                    e
                );
                return Err(self.reject("runtime_api_error"));
            }
        }

        // A vote stays current from the epoch of the committed head onward,
        // so a stall that crosses an epoch boundary keeps its votes.
        match self.epoch_window(best_hash) {
            Ok((committed_epoch, current_epoch)) => {
                if !epoch_is_current(epoch_id, committed_epoch, current_epoch) {
                    debug!(
                        "Commit Engine Gossip: epoch {} sits outside {} to {}",
                        epoch_id,
                        committed_epoch,
                        current_epoch.saturating_add(1)
                    );
                    return Err(self.reject("epoch_mismatch"));
                }
            }
            Err(reason) => {
                warn!(
                    "Commit Engine Gossip: the epoch check could not run: {}. The message is held for the next import.",
                    reason
                );
                return Err(self.reject("runtime_api_error"));
            }
        }

        match api.get_validator_set_id(best_hash) {
            Ok(current_validator_set_id) => {
                if validator_set_id != current_validator_set_id {
                    let is_grace_period =
                        if validator_set_id == current_validator_set_id.saturating_sub(1) {
                            match api.get_validator_set_id_changed_at(best_hash) {
                                Ok(Some(changed_at)) => {
                                    let current_block = self.client.info().best_number;
                                    current_block <= changed_at + 1u32.into()
                                }
                                _ => false,
                            }
                        } else {
                            false
                        };
                    if !is_grace_period {
                        warn!(
                            "Commit Engine Gossip: validator set mismatch. Expected {}, got {} from validator {:?}",
                            current_validator_set_id, validator_set_id, validator
                        );
                        return Err(self.reject("validator_set_mismatch"));
                    }
                    debug!(
                        "Commit Engine Gossip: accepted previous validator set {} during grace period",
                        validator_set_id
                    );
                }
            }
            Err(reason) => {
                warn!(
                    "Commit Engine Gossip: the validator set check could not run: {:?}. The message is held for the next import.",
                    reason
                );
                return Err(self.reject("runtime_api_error"));
            }
        }
        Ok(())
    }

    /// Check that the node holds the block one vote names.
    ///
    /// A vote can arrive before its block. Such a vote is held and tried again
    /// once the block imports. A vote for an unknown block at or below the
    /// committed head names a fork the chain left behind, and is refused.
    fn check_target(&self, height: u32, block: B::Hash) -> Result<(), &'static str> {
        use sp_runtime::SaturatedConversion;
        match self.client.header(block) {
            Ok(Some(_)) => Ok(()),
            Ok(None) => {
                let committed: u32 = self.client.info().finalized_number.saturated_into();
                if height <= committed {
                    Err(self.reject("stale_target"))
                } else {
                    Err(self.reject("target_not_imported"))
                }
            }
            Err(reason) => {
                warn!(
                    "Commit Engine Gossip: the block a vote names could not be read: {:?}. The vote is held for the next import.",
                    reason
                );
                Err(self.reject("target_not_imported"))
            }
        }
    }

    /// Internal validation core.
    fn validate_core(
        &self,
        message_data: &[u8],
    ) -> Result<CommitMessage<B::Hash, AccountId>, &'static str> {
        // The size bounds of the propagation path and the vote path run before
        // the decoding, so an oversized message costs two length comparisons.
        if let Err(reason) = admit_vote_message(message_data) {
            warn!("Commit Engine Gossip: {}", reason);
            return Err(self.reject("above_bound"));
        }

        let message =
            CommitMessage::<B::Hash, AccountId>::decode(&mut &message_data[..]).map_err(|_| {
                warn!("Commit Engine Gossip: Failed to decode incoming commit protocol message");
                self.reject("invalid_encoding")
            })?;

        match &message {
            CommitMessage::Vote(vote) => {
                let mut encoded_payload = Vec::new();
                vote.epoch_id.encode_to(&mut encoded_payload);
                vote.validator_set_id.encode_to(&mut encoded_payload);
                vote.round_number.encode_to(&mut encoded_payload);
                vote.block_number.encode_to(&mut encoded_payload);
                vote.block_hash.encode_to(&mut encoded_payload);
                vote.validator_account_id.encode_to(&mut encoded_payload);
                if !signer_matches(&vote.validator_account_id, &vote.validator_public_key)
                    || !sp_io::crypto::ed25519_verify(
                        &vote.signature,
                        &encoded_payload,
                        &vote.validator_public_key,
                    )
                {
                    warn!(
                        "Commit Engine Gossip: Invalid signature on vote from {:?}",
                        vote.validator_account_id
                    );
                    return Err(self.reject("invalid_signature"));
                }
                self.check_membership(
                    vote.epoch_id,
                    vote.validator_set_id,
                    &vote.validator_account_id,
                )?;
                self.check_target(vote.block_number, vote.block_hash)?;
            }
            CommitMessage::RoundTimeout(statement) => {
                if !signer_matches(
                    &statement.validator_account_id,
                    &statement.validator_public_key,
                ) || !sp_io::crypto::ed25519_verify(
                    &statement.signature,
                    &statement.payload(),
                    &statement.validator_public_key,
                ) {
                    warn!(
                        "Commit Engine Gossip: Invalid signature on round timeout from {:?}",
                        statement.validator_account_id
                    );
                    return Err(self.reject("invalid_signature"));
                }
                self.check_membership(
                    statement.epoch_id,
                    statement.validator_set_id,
                    &statement.validator_account_id,
                )?;
            }
        }
        Ok(message)
    }
}

/// Report whether a signer key belongs to the account it names.
///
/// A Cerulea validator account is the thirty two bytes of its seal key. A
/// message naming one account and carrying the key of another is refused, so
/// one validator never signs in the name of another.
pub fn signer_matches<AccountId: Encode>(account: &AccountId, key: &ed25519::Public) -> bool {
    account.encode() == key.0.to_vec()
}

impl<B, C, AccountId> sc_network_gossip::Validator<B> for DvfGossipValidator<B, C, AccountId>
where
    B: BlockT + sp_runtime::traits::Block,
    AccountId: std::cmp::Eq
        + std::hash::Hash
        + Clone
        + Encode
        + Decode
        + Send
        + Sync
        + std::fmt::Debug
        + 'static,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>
        + RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>,
{
    fn validate(
        &self,
        _context: &mut dyn ValidatorContext<B>,
        _sender: &PeerId,
        data: &[u8],
    ) -> ValidationResult<B::Hash> {
        let msg_hash_b = message_hash::<B>(data);
        {
            let mut known = self.known_messages.write();
            if known.holds(&msg_hash_b) {
                known.record(msg_hash_b);
                return ValidationResult::ProcessAndDiscard(msg_hash_b);
            }
        }
        // A held message waits for the next import, and costs the peer nothing.
        self.admit(data, msg_hash_b)
            .unwrap_or(ValidationResult::Discard)
    }

    /// A message leaves the gossip store once it falls below the committed
    /// head or out of the round window, so the store stays small however long
    /// the chain runs.
    fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
        let (committed, round) = self.window();
        Box::new(move |_hash, data| {
            message_expired_bytes::<B::Hash, AccountId>(data, committed, round)
        })
    }

    /// A peer receives every message it lacks. A message a peer already holds
    /// goes out again only while it belongs to the current or the previous
    /// round, so the periodic repeat of the gossip engine never resends the
    /// whole store.
    fn message_allowed<'a>(
        &'a self,
    ) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
        let (committed, round) = self.window();
        Box::new(move |_who, intent, _hash, data| {
            message_allowed_bytes::<B::Hash, AccountId>(intent, data, committed, round)
        })
    }
}

/// The identifier the validator keeps for one message: the hash of its bytes.
fn message_hash<B: BlockT>(data: &[u8]) -> B::Hash {
    let hash = sp_core::hashing::blake2_256(data);
    B::Hash::decode(&mut &hash[..]).unwrap_or_default()
}

impl<B, C, AccountId> DvfGossipValidator<B, C, AccountId>
where
    B: BlockT + sp_runtime::traits::Block,
    AccountId: std::cmp::Eq
        + std::hash::Hash
        + Clone
        + Encode
        + Decode
        + Send
        + Sync
        + std::fmt::Debug
        + 'static,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>
        + RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>,
{
    /// Try every held message again.
    ///
    /// The node calls this after each block import. A vote that arrived before
    /// its block, or while the runtime could not answer, then counts once the
    /// node can decide on it. A message that still waits stays held. The answer
    /// is the number of messages decided.
    pub fn retry_held(&self) -> usize {
        let waiting = self.held.write().take_all();
        let mut decided = 0;
        for bytes in waiting {
            let hash = message_hash::<B>(&bytes);
            if self.known_messages.read().holds(&hash) {
                continue;
            }
            if self.admit(&bytes, hash).is_some() {
                decided += 1;
            }
        }
        decided
    }

    /// Check one message and hand it to the pool.
    ///
    /// The answer is the gossip verdict, or nothing where the message is held
    /// for a later try.
    fn admit(&self, data: &[u8], msg_hash_b: B::Hash) -> Option<ValidationResult<B::Hash>> {
        let checked = self.validate_core(data);
        if let Err(reason) = checked {
            if HOLD_REASONS.contains(&reason) {
                if self.held.write().hold(data.to_vec()) {
                    debug!(
                        "Commit Engine Gossip Validator: message held for the next import, reason: {}",
                        reason
                    );
                }
                return None;
            }
        }
        self.known_messages.write().record(msg_hash_b);
        Some(match checked {
            Ok(CommitMessage::Vote(msg)) => {
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_received(msg.round_number);
                }
                let offer = self.pool.offer_vote(msg.clone());
                match offer {
                    VoteOffer::Inserted => {
                        info!(
                            "Commit Engine Gossip Validator: Accepted vote for block #{} ({:?}) from validator {:?} (round: {})",
                            msg.block_number, msg.block_hash, msg.validator_account_id, msg.round_number
                        );
                    }
                    VoteOffer::Repeated => {
                        debug!(
                            "Commit Engine Gossip Validator: Repeated vote from validator {:?} in round {} refused",
                            msg.validator_account_id, msg.round_number
                        );
                        self.reject("repeated_vote");
                    }
                    VoteOffer::Conflicting => {
                        warn!(
                            "Commit Engine Gossip Validator: validator {:?} signed two votes in round {}. The second vote is refused and the pair is kept as equivocation evidence.",
                            msg.validator_account_id, msg.round_number
                        );
                        if let Some(ref metrics) = self.metrics {
                            metrics.record_double_vote_detection();
                        }
                        self.reject("double_vote");
                    }
                }
                vote_verdict(Some(offer), msg_hash_b)
            }
            Ok(CommitMessage::RoundTimeout(statement)) => {
                if self.pool.offer_timeout(statement) {
                    ValidationResult::ProcessAndKeep(msg_hash_b)
                } else {
                    self.reject("round_timeout_refused");
                    ValidationResult::Discard
                }
            }
            Err(reason) => {
                // A stale or out of range vote is ordinary during a stall, so
                // the refusal stays at debug level and costs the peer nothing.
                debug!(
                    "Commit Engine Gossip Validator: message refused, reason: {}",
                    reason
                );
                vote_verdict(None, msg_hash_b)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::{Pair, H256};

    type Pool = DvfVotePool<H256, [u8; 32]>;

    fn vote(signer: u8, round: u32, height: u32, block: u8) -> DvfVoteMessage<H256, [u8; 32]> {
        DvfVoteMessage {
            epoch_id: 1,
            validator_set_id: 0,
            round_number: round,
            block_number: height,
            block_hash: H256::repeat_byte(block),
            validator_account_id: [signer; 32],
            validator_public_key: ed25519::Public::from_raw([signer; 32]),
            signature: ed25519::Signature::from_raw([0u8; 64]),
        }
    }

    fn timeout(
        signer: u8,
        round: u32,
        height: u32,
        voted: Option<u8>,
    ) -> RoundTimeoutMessage<H256, [u8; 32]> {
        RoundTimeoutMessage {
            epoch_id: 1,
            validator_set_id: 0,
            round_number: round,
            block_number: height,
            voted: voted.map(H256::repeat_byte),
            validator_account_id: [signer; 32],
            validator_public_key: ed25519::Public::from_raw([signer; 32]),
            signature: ed25519::Signature::from_raw([0u8; 64]),
        }
    }

    /// The gossip store keeps live messages and repeats only the current round.
    ///
    /// The engine repeats its store to every peer three times every two
    /// seconds. A store that kept and repeated every message since genesis
    /// grew the traffic of each node without end. The store now drops what
    /// fell below the committed head or out of the round window, and repeats
    /// the current and the previous round alone.
    #[test]
    fn the_gossip_store_keeps_live_messages_and_repeats_the_current_round() {
        type Message = CommitMessage<H256, [u8; 32]>;
        let committed = 930;
        let round = 120;
        let fresh: Message = CommitMessage::Vote(vote(1, 120, 940, 7));
        let last_round: Message = CommitMessage::Vote(vote(1, 119, 940, 7));
        let older: Message = CommitMessage::Vote(vote(1, 100, 940, 7));
        let ancient: Message = CommitMessage::Vote(vote(1, 40, 940, 7));
        let committed_height: Message = CommitMessage::Vote(vote(1, 120, 930, 7));
        let statement: Message = CommitMessage::RoundTimeout(timeout(2, 119, 940, Some(7)));

        assert!(message_live(&fresh, committed, round));
        assert!(message_live(&older, committed, round));
        assert!(!message_live(&ancient, committed, round));
        assert!(!message_live(&committed_height, committed, round));
        assert!(message_live(&statement, committed, round));

        assert!(message_repeated(&fresh, committed, round));
        assert!(message_repeated(&last_round, committed, round));
        assert!(message_repeated(&statement, committed, round));
        assert!(!message_repeated(&older, committed, round));
        assert!(!message_repeated(&committed_height, committed, round));

        // The byte forms the gossip engine hands the validator.
        let bytes = fresh.encode();
        assert!(!message_expired_bytes::<H256, [u8; 32]>(
            &bytes, committed, round
        ));
        assert!(message_expired_bytes::<H256, [u8; 32]>(
            &ancient.encode(),
            committed,
            round
        ));
        assert!(message_expired_bytes::<H256, [u8; 32]>(
            &[9, 9, 9],
            committed,
            round
        ));
        assert!(message_allowed_bytes::<H256, [u8; 32]>(
            MessageIntent::Broadcast,
            &older.encode(),
            committed,
            round
        ));
        assert!(!message_allowed_bytes::<H256, [u8; 32]>(
            MessageIntent::PeriodicRebroadcast,
            &older.encode(),
            committed,
            round
        ));
        assert!(message_allowed_bytes::<H256, [u8; 32]>(
            MessageIntent::PeriodicRebroadcast,
            &bytes,
            committed,
            round
        ));
    }

    /// The traffic one node sends, run through the gossip engine algorithm.
    ///
    /// The model follows the engine: a new message goes to every peer lacking
    /// it, the store drops expired messages, and every 750 milliseconds the
    /// store goes out again to every peer, each message passing the allowed
    /// check first. One node with eight peers runs 300 commitments. Each
    /// commitment takes one round, and every fifth takes two, with a round
    /// timeout statement from every validator. The answer is the largest store
    /// and the sends to one peer per second, for the first and the last fifty
    /// commitments.
    fn gossip_traffic(
        expired: impl Fn(&[u8], u32, u32) -> bool,
        allowed: impl Fn(MessageIntent, &[u8], u32, u32) -> bool,
    ) -> (usize, f64, f64) {
        const VALIDATORS: u8 = 9;
        const PEERS: usize = 8;
        const INTERVAL: u32 = 10;
        const COMMITMENTS: u32 = 300;
        const TICKS_PER_ROUND: u32 = 8;
        let mut store: Vec<(H256, Vec<u8>)> = Vec::new();
        let mut known: Vec<HashSet<H256>> = vec![HashSet::new(); PEERS];
        let mut sends_per_commitment: Vec<u64> = Vec::new();
        let mut largest = 0usize;
        let mut round = 0u32;
        for index in 0..COMMITMENTS {
            let committed = index * INTERVAL;
            let target = committed + INTERVAL;
            let rounds = if index % 5 == 4 { 2 } else { 1 };
            let mut sends = 0u64;
            for step in 0..rounds {
                round += 1;
                let mut fresh: Vec<Vec<u8>> = (1..=VALIDATORS)
                    .map(|signer| CommitMessage::Vote(vote(signer, round, target, 7)).encode())
                    .collect();
                if step > 0 {
                    fresh.extend((1..=VALIDATORS).map(|signer| {
                        CommitMessage::RoundTimeout(timeout(signer, round - 1, target, Some(7)))
                            .encode()
                    }));
                }
                for bytes in fresh {
                    let hash = H256(sp_core::hashing::blake2_256(&bytes));
                    for peer in known.iter_mut() {
                        if allowed(MessageIntent::Broadcast, &bytes, committed, round) {
                            peer.insert(hash);
                            sends += 1;
                        }
                    }
                    store.push((hash, bytes));
                }
                for _ in 0..TICKS_PER_ROUND {
                    store.retain(|(_, bytes)| !expired(bytes, committed, round));
                    let held: HashSet<H256> = store.iter().map(|(hash, _)| *hash).collect();
                    for peer in known.iter_mut() {
                        peer.retain(|hash| held.contains(hash));
                        for (hash, bytes) in &store {
                            let intent = if peer.contains(hash) {
                                MessageIntent::PeriodicRebroadcast
                            } else {
                                MessageIntent::Broadcast
                            };
                            if allowed(intent, bytes, committed, round) {
                                peer.insert(*hash);
                                sends += 1;
                            }
                        }
                    }
                    largest = largest.max(store.len());
                }
            }
            sends_per_commitment.push(sends);
        }
        // Sends to one peer per second: each tick is 750 milliseconds.
        let rate = |slice: &[u64], rounds: f64| -> f64 {
            let total: u64 = slice.iter().sum();
            total as f64 / PEERS as f64 / (rounds * TICKS_PER_ROUND as f64 * 0.75)
        };
        let rounds_per_fifty = 60.0;
        let first = rate(&sends_per_commitment[..50], rounds_per_fifty);
        let last = rate(&sends_per_commitment[250..], rounds_per_fifty);
        (largest, first, last)
    }

    /// The gossip store and the send rate stay bounded over 300 commitments.
    ///
    /// Run with the expiry and the repeat rule of the validator, the store
    /// never holds more than two rounds of votes and statements, and the
    /// sends to one peer per second stay the same from the first fifty
    /// commitments to the last. Run with the earlier rule, which kept every
    /// message and repeated all of them, both grow with the length of the
    /// chain.
    #[test]
    fn the_gossip_store_and_the_send_rate_stay_bounded_over_three_hundred_commitments() {
        let (largest, first, last) = gossip_traffic(
            |bytes, committed, round| {
                message_expired_bytes::<H256, [u8; 32]>(bytes, committed, round)
            },
            |intent, bytes, committed, round| {
                message_allowed_bytes::<H256, [u8; 32]>(intent, bytes, committed, round)
            },
        );
        // Two rounds of nine votes and nine statements at the open height.
        assert!(largest <= 27, "the store held {largest} messages");
        // About two votes a second per peer, and no growth.
        assert!(
            last <= first * 1.05 + 0.01,
            "rate grew from {first:.2} to {last:.2}"
        );
        assert!(last < 25.0, "rate {last:.2} sends per peer per second");

        let (largest_before, first_before, last_before) =
            gossip_traffic(|_, _, _| false, |_, _, _, _| true);
        assert!(largest_before > 3_000, "{largest_before}");
        assert!(
            last_before > first_before * 4.0,
            "{first_before:.2} to {last_before:.2}"
        );
    }

    /// A message the node cannot decide on yet waits once, within a ceiling.
    #[test]
    fn held_messages_wait_once_within_a_ceiling() {
        let mut held = HeldMessages::new();
        assert!(held.hold(vec![1]));
        assert!(!held.hold(vec![1]));
        assert!(held.hold(vec![2]));
        assert_eq!(held.len(), 2);
        assert_eq!(held.take_all(), vec![vec![1], vec![2]]);
        assert!(held.is_empty());
        for index in 0..=MAX_HELD_MESSAGES {
            held.hold((index as u32).to_le_bytes().to_vec());
        }
        assert_eq!(held.len(), MAX_HELD_MESSAGES);
        // The oldest message left first.
        assert_eq!(held.take_all()[0], 1u32.to_le_bytes().to_vec());
        // Only a missing block or a missing runtime answer holds a message.
        assert!(HOLD_REASONS.contains(&"target_not_imported"));
        assert!(HOLD_REASONS.contains(&"runtime_api_error"));
        assert!(!HOLD_REASONS.contains(&"epoch_mismatch"));
        assert!(!HOLD_REASONS.contains(&"stale_target"));
    }

    /// A stale, repeated or conflicting vote is discarded and never kept.
    ///
    /// The gossip engine charges a peer only for the same bytes sent twice by
    /// that peer, and a discarded message changes no standing. A peer relaying
    /// the votes of a stall therefore keeps its standing.
    #[test]
    fn refused_votes_cost_the_relaying_peer_nothing() {
        let pool = Pool::new();
        let topic = H256::repeat_byte(3);
        let first = pool.offer_vote(vote(1, 9, 1440, 7));
        assert!(matches!(
            vote_verdict(Some(first), topic),
            ValidationResult::ProcessAndKeep(kept) if kept == topic
        ));
        let repeat = pool.offer_vote(vote(1, 9, 1440, 7));
        assert_eq!(repeat, VoteOffer::Repeated);
        assert!(matches!(
            vote_verdict(Some(repeat), topic),
            ValidationResult::Discard
        ));
        let conflicting = pool.offer_vote(vote(1, 9, 1440, 8));
        assert_eq!(conflicting, VoteOffer::Conflicting);
        assert!(matches!(
            vote_verdict(Some(conflicting), topic),
            ValidationResult::Discard
        ));
        // A vote the checks refused, such as one from an epoch below the
        // committed head.
        assert!(matches!(
            vote_verdict(None, topic),
            ValidationResult::Discard
        ));
    }

    /// P-05. A repeated vote from one voter is refused.
    #[test]
    fn a_repeated_vote_is_refused() {
        let pool = Pool::new();
        assert_eq!(pool.offer_vote(vote(1, 142, 1440, 7)), VoteOffer::Inserted);
        assert_eq!(pool.offer_vote(vote(1, 142, 1440, 7)), VoteOffer::Repeated);
        assert!(!pool.insert_vote(vote(1, 142, 1440, 7)));
        assert_eq!(pool.get_votes(142, &H256::repeat_byte(7)).len(), 1);
    }

    /// A second vote in one round for a different block is evidence.
    #[test]
    fn a_conflicting_vote_is_refused_and_kept_as_evidence() {
        let pool = Pool::new();
        assert_eq!(pool.offer_vote(vote(1, 142, 1440, 7)), VoteOffer::Inserted);
        assert_eq!(pool.offer_vote(vote(2, 142, 1440, 7)), VoteOffer::Inserted);
        assert_eq!(
            pool.offer_vote(vote(1, 142, 1440, 9)),
            VoteOffer::Conflicting
        );
        assert!(pool.equivocated(142, &[1u8; 32]));

        // The aggregator never counts the equivocator in that round.
        let counted = pool.get_votes(142, &H256::repeat_byte(7));
        assert_eq!(counted.len(), 1);
        assert_eq!(counted[0].validator_account_id, [2u8; 32]);
        assert!(pool.get_votes(142, &H256::repeat_byte(9)).is_empty());

        let evidence = pool.take_evidence();
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].0.block_hash, H256::repeat_byte(7));
        assert_eq!(evidence[0].1.block_hash, H256::repeat_byte(9));
        assert!(pool.take_evidence().is_empty());
    }

    /// Pruning finalized heights keeps the one-vote-per-round record.
    #[test]
    fn pruning_by_height_keeps_the_round_record() {
        let pool = Pool::new();
        pool.offer_vote(vote(1, 142, 1440, 7));
        pool.prune_by_finalized_block(1450);
        assert_eq!(pool.total_votes(), 0);
        assert_eq!(
            pool.offer_vote(vote(1, 142, 1440, 9)),
            VoteOffer::Conflicting
        );
        pool.prune_by_round_age(200, 20);
        assert_eq!(pool.offer_vote(vote(1, 142, 1440, 9)), VoteOffer::Inserted);
    }

    /// A timeout that contradicts the vote the pool holds is refused.
    #[test]
    fn a_false_declaration_never_counts() {
        let pool = Pool::new();
        pool.offer_vote(vote(1, 5, 50, 7));
        assert!(!pool.offer_timeout(timeout(1, 5, 50, Some(9))));
        assert!(!pool.offer_timeout(timeout(1, 5, 50, None)));
        assert!(pool.offer_timeout(timeout(1, 5, 50, Some(7))));
        assert!(!pool.offer_timeout(timeout(1, 5, 50, Some(7))));
        assert!(pool.offer_timeout(timeout(2, 5, 50, None)));
        assert_eq!(pool.timeouts_for(5, 50).len(), 2);
    }

    /// A vote for an uncommitted height stays current across epochs.
    #[test]
    fn a_vote_stays_current_until_its_height_commits() {
        // The committed head sits in epoch 14 and the node runs epoch 16.
        assert!(epoch_is_current(14, 14, 16));
        assert!(epoch_is_current(15, 14, 16));
        assert!(epoch_is_current(17, 14, 16));
        assert!(!epoch_is_current(13, 14, 16));
        assert!(!epoch_is_current(18, 14, 16));
        // The rule the stall met: current or next epoch alone.
        let legacy = |vote: u32, current: u32| vote == current || vote == current + 1;
        assert!(!legacy(14, 16));
    }

    /// A signer key must match the account it names.
    #[test]
    fn a_vote_signed_for_another_account_is_refused() {
        let pair = ed25519::Pair::from_seed(&[4u8; 32]);
        assert!(signer_matches(&pair.public().0, &pair.public()));
        assert!(!signer_matches(&[9u8; 32], &pair.public()));
    }

    /// Pruning, size limits and the pool counters.
    #[test]
    fn the_pool_bounds_its_memory() {
        let pool = Pool::new();
        for round in 0..5u32 {
            pool.offer_vote(vote(1, round, 10, round as u8 + 1));
        }
        pool.offer_timeout(timeout(2, 1, 10, None));
        assert_eq!(pool.size(), 5);
        assert_eq!(pool.total_votes(), 5);
        assert_eq!(pool.get_candidate_blocks(3), vec![H256::repeat_byte(4)]);
        assert!(pool.get_candidate_blocks(9).is_empty());

        // The two oldest rounds leave first.
        assert_eq!(pool.enforce_max_size(3), 2);
        assert!(pool.get_votes(0, &H256::repeat_byte(1)).is_empty());
        assert!(pool.get_votes(1, &H256::repeat_byte(2)).is_empty());
        assert_eq!(pool.enforce_max_size(3), 0);

        pool.prune_older_rounds(4);
        assert_eq!(pool.size(), 1);
        assert!(pool.timeouts_for(1, 10).is_empty());
        assert!(pool.vote_of(3, &[1u8; 32]).is_none());
        assert!(pool.vote_of(4, &[1u8; 32]).is_some());

        pool.clear();
        assert_eq!(pool.size(), 0);
        assert!(pool.vote_of(4, &[1u8; 32]).is_none());
    }

    /// Evidence beyond the held ceiling is dropped, and the pair counts once.
    #[test]
    fn the_evidence_store_holds_inside_its_ceiling() {
        let pool = Pool::new();
        for signer in 0..=MAX_HELD_EVIDENCE as u32 {
            let mut first = vote(1, 7, 70, 1);
            first.validator_account_id = signer.to_le_bytes().repeat(8).try_into().unwrap();
            let mut second = first.clone();
            second.block_hash = H256::repeat_byte(2);
            pool.offer_vote(first);
            assert_eq!(pool.offer_vote(second), VoteOffer::Conflicting);
        }
        assert_eq!(pool.take_evidence().len(), MAX_HELD_EVIDENCE);
    }

    /// An equivocator states nothing in the round it equivocated in.
    #[test]
    fn an_equivocator_states_no_timeout_in_that_round() {
        let pool = Pool::new();
        pool.offer_vote(vote(1, 5, 50, 7));
        pool.offer_vote(vote(1, 5, 50, 8));
        assert!(!pool.offer_timeout(timeout(1, 5, 50, Some(7))));
        // A statement at another height in the same round stands apart from
        // the vote, as long as the signer did not equivocate.
        pool.offer_vote(vote(2, 6, 60, 7));
        assert!(pool.offer_timeout(timeout(2, 6, 70, None)));
    }

    /// A message the size bounds refuse never reaches the decoder.
    #[test]
    fn the_vote_message_bound_runs_before_decoding() {
        assert!(admit_vote_message(&[0u8; 256]).is_ok());
        let refusal = admit_vote_message(&[0u8; 257]).expect_err("the vote bound refuses it");
        assert!(refusal.to_string().contains("Fix:"), "{}", refusal);
        assert!(admit_vote_message(&vec![0u8; 9 * 1024]).is_err());
    }

    /// A short hash fills its tail with zeros.
    #[test]
    fn a_hash_reads_as_thirty_two_bytes() {
        assert_eq!(hash_bytes(&H256::repeat_byte(5)), [5u8; 32]);
        let mut expected = [0u8; 32];
        expected[..4].copy_from_slice(&7u32.to_le_bytes());
        assert_eq!(hash_bytes(&7u32), expected);
        assert_eq!(DVF_PROTOCOL_NAME, "/cerulea/commit/2");
    }

    /// A timeout signature never passes for a vote signature.
    #[test]
    fn the_timeout_payload_differs_from_the_vote_payload() {
        let statement = timeout(1, 5, 50, Some(7));
        let mut vote_payload = Vec::new();
        statement.epoch_id.encode_to(&mut vote_payload);
        statement.validator_set_id.encode_to(&mut vote_payload);
        statement.round_number.encode_to(&mut vote_payload);
        statement.block_number.encode_to(&mut vote_payload);
        H256::repeat_byte(7).encode_to(&mut vote_payload);
        statement.validator_account_id.encode_to(&mut vote_payload);
        assert_ne!(statement.payload(), vote_payload);
    }
}

#[cfg(test)]
mod validator_tests {
    //! The gossip validator, run against the test chain.
    //!
    //! The chain commits block two and holds block twelve as its best block.
    //! The runtime runs epoch five with validator set three. Two of three
    //! validators sit in the active set.

    use super::*;
    use crate::test_support::{capture_logs, AccountId, Block, RecordingContext, TestChain, Voter};
    use sc_network_gossip::Validator as _;
    use sp_core::{Pair as _, H256};

    type Pool = DvfVotePool<H256, AccountId>;
    type GossipCheck = DvfGossipValidator<Block, TestChain, AccountId>;

    struct Scene {
        chain: Arc<TestChain>,
        pool: Arc<Pool>,
        metrics: Arc<DvfMetrics>,
        validator: GossipCheck,
        active: [Voter; 2],
        outsider: Voter,
    }

    fn scene_on(chain: Arc<TestChain>) -> Scene {
        let active = [Voter::from_seed(1), Voter::from_seed(2)];
        let outsider = Voter::from_seed(3);
        chain.answers.set(
            "DcfApi_get_active_validators",
            (),
            vec![active[0].account.clone(), active[1].account.clone()],
        );
        chain.answers.set("DvfApi_get_current_epoch", (), 5u32);
        chain.answers.set("DvfApi_get_validator_set_id", (), 3u32);
        chain
            .answers
            .set("DvfApi_get_validator_set_id_changed_at", (), Some(12u32));
        let registry = prometheus::Registry::new();
        let metrics = Arc::new(DvfMetrics::new(&registry).expect("the metrics register"));
        let pool = Pool::new();
        let validator = GossipCheck::new(pool.clone(), chain.clone()).with_metrics(metrics.clone());
        Scene {
            chain,
            pool,
            metrics,
            validator,
            active,
            outsider,
        }
    }

    fn scene() -> Scene {
        scene_on(TestChain::build(2, 12))
    }

    fn offer(scene: &Scene, data: &[u8]) -> ValidationResult<H256> {
        let mut context = RecordingContext::default();
        let verdict = scene
            .validator
            .validate(&mut context, &PeerId::random(), data);
        assert!(
            context.requests.is_empty(),
            "the validator sends nothing itself"
        );
        verdict
    }

    fn encoded_vote(vote: DvfVoteMessage<H256, AccountId>) -> Vec<u8> {
        CommitMessage::Vote(vote).encode()
    }

    fn rejections(scene: &Scene, reason: &str) -> f64 {
        scene
            .metrics
            .dvf_vote_rejections_total
            .with_label_values(&[reason])
            .get()
    }

    fn kept(verdict: &ValidationResult<H256>, data: &[u8]) -> bool {
        matches!(verdict, ValidationResult::ProcessAndKeep(topic) if *topic == message_hash::<Block>(data))
    }

    fn discarded(verdict: &ValidationResult<H256>) -> bool {
        matches!(verdict, ValidationResult::Discard)
    }

    #[test]
    fn a_checked_vote_reaches_the_pool_once() {
        let scene = scene();
        let vote = scene.active[0].vote(5, 3, 7, 10, scene.chain.hash_at(10));
        let data = encoded_vote(vote.clone());

        assert!(kept(&offer(&scene, &data), &data));
        assert_eq!(scene.pool.vote_of(7, &scene.active[0].account), Some(vote));
        assert_eq!(
            scene
                .metrics
                .dvf_votes_received_total
                .with_label_values(&["7"])
                .get(),
            1.0
        );

        // The same bytes again count as seen.
        assert!(matches!(
            offer(&scene, &data),
            ValidationResult::ProcessAndDiscard(topic) if topic == message_hash::<Block>(&data)
        ));
        assert_eq!(scene.pool.total_votes(), 1);
    }

    #[test]
    fn a_repeat_of_a_held_vote_under_new_bytes_is_refused() {
        let scene = scene();
        let block = scene.chain.hash_at(10);
        let first = encoded_vote(scene.active[0].vote(5, 3, 7, 10, block));
        // The next epoch is still current, so the repeat passes every check.
        let repeat = encoded_vote(scene.active[0].vote(6, 3, 7, 10, block));

        assert!(kept(&offer(&scene, &first), &first));
        assert!(discarded(&offer(&scene, &repeat)));
        assert_eq!(rejections(&scene, "repeated_vote"), 1.0);
        assert_eq!(scene.pool.total_votes(), 1);
    }

    #[test]
    fn a_second_block_in_one_round_is_kept_as_evidence() {
        let scene = scene();
        let first = encoded_vote(scene.active[0].vote(5, 3, 7, 10, scene.chain.hash_at(10)));
        let second = encoded_vote(scene.active[0].vote(5, 3, 7, 11, scene.chain.hash_at(11)));
        let logs = capture_logs();

        assert!(kept(&offer(&scene, &first), &first));
        assert!(discarded(&offer(&scene, &second)));

        assert_eq!(rejections(&scene, "double_vote"), 1.0);
        assert_eq!(scene.metrics.dvf_double_votes_detected_total.get(), 1.0);
        assert!(scene.pool.equivocated(7, &scene.active[0].account));
        assert_eq!(scene.pool.take_evidence().len(), 1);
        assert!(logs.has("signed two votes in round 7"));
    }

    #[test]
    fn unreadable_and_oversized_messages_are_refused_on_sight() {
        let scene = scene();
        assert!(discarded(&offer(&scene, &[9u8, 1, 2])));
        assert_eq!(rejections(&scene, "invalid_encoding"), 1.0);
        assert!(discarded(&offer(&scene, &[0u8; 300])));
        assert_eq!(rejections(&scene, "above_bound"), 1.0);
        assert_eq!(scene.validator.held_count(), 0);
    }

    #[test]
    fn a_forged_or_borrowed_signature_is_refused() {
        let scene = scene();
        let block = scene.chain.hash_at(10);
        let mut forged = scene.active[0].vote(5, 3, 7, 10, block);
        forged.signature = scene.active[0].pair.sign(b"another payload");
        assert!(discarded(&offer(&scene, &encoded_vote(forged))));

        // The second validator signs in the name of the first.
        let mut borrowed = scene.active[1].vote(5, 3, 8, 10, block);
        borrowed.validator_account_id = scene.active[0].account.clone();
        borrowed.signature = scene.active[1]
            .pair
            .sign(&crate::vote_creator::vote_payload(&borrowed));
        assert!(discarded(&offer(&scene, &encoded_vote(borrowed))));

        assert_eq!(rejections(&scene, "invalid_signature"), 2.0);
        assert_eq!(scene.pool.total_votes(), 0);
    }

    #[test]
    fn a_vote_from_outside_the_active_set_is_refused() {
        let scene = scene();
        let data = encoded_vote(scene.outsider.vote(5, 3, 7, 10, scene.chain.hash_at(10)));
        assert!(discarded(&offer(&scene, &data)));
        assert_eq!(rejections(&scene, "inactive_validator"), 1.0);
    }

    #[test]
    fn a_vote_outside_the_current_epochs_is_refused() {
        let scene = scene();
        let block = scene.chain.hash_at(10);
        let stale = encoded_vote(scene.active[0].vote(4, 3, 7, 10, block));
        let early = encoded_vote(scene.active[0].vote(7, 3, 8, 10, block));
        assert!(discarded(&offer(&scene, &stale)));
        assert!(discarded(&offer(&scene, &early)));
        assert_eq!(rejections(&scene, "epoch_mismatch"), 2.0);
        assert_eq!(scene.validator.held_count(), 0);
    }

    #[test]
    fn a_lower_committed_epoch_keeps_older_votes_current() {
        let scene = scene();
        let committed = scene.chain.info().finalized_hash;
        scene
            .chain
            .answers
            .set_at("DvfApi_get_current_epoch", committed, (), 3u32);
        let block = scene.chain.hash_at(10);

        let older = encoded_vote(scene.active[0].vote(3, 3, 7, 10, block));
        assert!(kept(&offer(&scene, &older), &older));
        let oldest = encoded_vote(scene.active[1].vote(2, 3, 7, 10, block));
        assert!(discarded(&offer(&scene, &oldest)));
    }

    #[test]
    fn a_committed_head_without_an_epoch_answer_leaves_the_best_block_to_decide() {
        let scene = scene();
        let best = scene.chain.info().best_hash;
        scene.chain.answers.forget("DvfApi_get_current_epoch");
        scene
            .chain
            .answers
            .set_at("DvfApi_get_current_epoch", best, (), 5u32);
        let logs = capture_logs();

        let data = encoded_vote(scene.active[0].vote(5, 3, 7, 10, scene.chain.hash_at(10)));
        assert!(kept(&offer(&scene, &data), &data));
        assert!(logs.has("the epoch at the committed head is unavailable"));
    }

    #[test]
    fn at_genesis_one_epoch_answer_serves_both_ends_of_the_window() {
        let scene = scene_on(TestChain::build(0, 0));
        let genesis = scene.chain.hash_at(0);
        scene.chain.answers.forget("DvfApi_get_current_epoch");
        scene
            .chain
            .answers
            .set_at("DvfApi_get_current_epoch", genesis, (), 5u32);
        let data = encoded_vote(scene.active[0].vote(5, 3, 0, 0, genesis));
        assert!(kept(&offer(&scene, &data), &data));
    }

    #[test]
    fn the_previous_validator_set_passes_inside_its_grace_block() {
        let scene = scene();
        let block = scene.chain.hash_at(10);

        let inside = encoded_vote(scene.active[0].vote(5, 2, 20, 10, block));
        assert!(kept(&offer(&scene, &inside), &inside));

        scene
            .chain
            .answers
            .set("DvfApi_get_validator_set_id_changed_at", (), Some(5u32));
        let late = encoded_vote(scene.active[0].vote(5, 2, 21, 10, block));
        assert!(discarded(&offer(&scene, &late)));

        scene
            .chain
            .answers
            .set("DvfApi_get_validator_set_id_changed_at", (), None::<u32>);
        let unrecorded = encoded_vote(scene.active[0].vote(5, 2, 22, 10, block));
        assert!(discarded(&offer(&scene, &unrecorded)));

        let older = encoded_vote(scene.active[0].vote(5, 1, 23, 10, block));
        assert!(discarded(&offer(&scene, &older)));

        assert_eq!(rejections(&scene, "validator_set_mismatch"), 3.0);
    }

    #[test]
    fn a_vote_for_an_abandoned_fork_below_the_committed_head_is_refused() {
        let scene = scene();
        let fork = H256::repeat_byte(0x5a);
        let data = encoded_vote(scene.active[0].vote(5, 3, 7, 1, fork));
        assert!(discarded(&offer(&scene, &data)));
        assert_eq!(rejections(&scene, "stale_target"), 1.0);
        assert_eq!(scene.validator.held_count(), 0);
    }

    #[test]
    fn a_vote_ahead_of_its_block_waits_for_the_import() {
        let scene = scene();
        let next = scene.chain.next_header();
        let data = encoded_vote(scene.active[0].vote(5, 3, 7, 13, next.hash()));

        assert!(discarded(&offer(&scene, &data)));
        assert_eq!(scene.validator.held_count(), 1);
        assert_eq!(rejections(&scene, "target_not_imported"), 1.0);
        // A retry before the import keeps the message waiting.
        assert_eq!(scene.validator.retry_held(), 0);
        assert_eq!(scene.validator.held_count(), 1);

        scene.chain.import(next);
        assert_eq!(scene.validator.retry_held(), 1);
        assert_eq!(scene.validator.held_count(), 0);
        assert!(scene.pool.vote_of(7, &scene.active[0].account).is_some());
    }

    #[test]
    fn a_held_message_that_arrived_again_is_decided_once() {
        let scene = scene();
        let next = scene.chain.next_header();
        let data = encoded_vote(scene.active[0].vote(5, 3, 7, 13, next.hash()));
        assert!(discarded(&offer(&scene, &data)));
        scene.chain.import(next);

        assert!(kept(&offer(&scene, &data), &data));
        assert_eq!(scene.validator.retry_held(), 0);
        assert_eq!(scene.validator.held_count(), 0);
        assert_eq!(scene.pool.total_votes(), 1);
    }

    #[test]
    fn a_silent_runtime_holds_the_message_for_the_next_import() {
        for call in [
            "DcfApi_get_active_validators",
            "DvfApi_get_current_epoch",
            "DvfApi_get_validator_set_id",
        ] {
            let scene = scene();
            let data = encoded_vote(scene.active[0].vote(5, 3, 7, 10, scene.chain.hash_at(10)));
            scene.chain.answers.forget(call);

            assert!(discarded(&offer(&scene, &data)), "{call}");
            assert_eq!(scene.validator.held_count(), 1, "{call}");
            assert_eq!(rejections(&scene, "runtime_api_error"), 1.0, "{call}");
        }
    }

    #[test]
    fn a_held_message_counts_once_the_runtime_answers_again() {
        let scene = scene();
        let data = encoded_vote(scene.active[0].vote(5, 3, 7, 10, scene.chain.hash_at(10)));
        scene.chain.answers.forget("DcfApi_get_active_validators");
        assert!(discarded(&offer(&scene, &data)));

        scene.chain.answers.set(
            "DcfApi_get_active_validators",
            (),
            vec![scene.active[0].account.clone()],
        );
        assert_eq!(scene.validator.retry_held(), 1);
        assert!(scene.pool.vote_of(7, &scene.active[0].account).is_some());
    }

    #[test]
    fn a_round_timeout_statement_reaches_the_pool_once() {
        let scene = scene();
        let block = scene.chain.hash_at(10);
        let statement = scene.active[0].timeout(5, 3, 7, 10, Some(block));
        let data = CommitMessage::RoundTimeout(statement.clone()).encode();
        assert!(kept(&offer(&scene, &data), &data));
        assert_eq!(scene.pool.timeouts_for(7, 10), vec![statement]);

        // A second statement from the same signer for that round and height.
        let changed =
            CommitMessage::RoundTimeout(scene.active[0].timeout(5, 3, 7, 10, None)).encode();
        assert!(discarded(&offer(&scene, &changed)));
        assert_eq!(rejections(&scene, "round_timeout_refused"), 1.0);
    }

    #[test]
    fn a_round_timeout_statement_passes_the_same_checks_as_a_vote() {
        let scene = scene();
        let mut forged = scene.active[0].timeout(5, 3, 7, 10, None);
        forged.signature = scene.active[0].pair.sign(b"another payload");
        assert!(discarded(&offer(
            &scene,
            &CommitMessage::RoundTimeout(forged).encode()
        )));
        assert_eq!(rejections(&scene, "invalid_signature"), 1.0);

        let outsider = scene.outsider.timeout(5, 3, 7, 10, None);
        assert!(discarded(&offer(
            &scene,
            &CommitMessage::RoundTimeout(outsider).encode()
        )));
        assert_eq!(rejections(&scene, "inactive_validator"), 1.0);
        assert!(scene.pool.timeouts_for(7, 10).is_empty());
    }

    #[test]
    fn an_unreadable_message_leaves_the_store_at_once() {
        // Since P1-022 the store keeps only messages it can read and place
        // inside the commit window. Three stray bytes decode as no message.
        let scene = scene();
        let mut expired = scene.validator.message_expired();
        assert!(expired(H256::zero(), &[1, 2, 3]));
    }
}
