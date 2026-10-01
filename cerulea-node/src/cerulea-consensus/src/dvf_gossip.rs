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

/// The most evidence pairs the pool holds before the aggregator reports them.
pub const MAX_HELD_EVIDENCE: usize = 1024;

/// Validator for Commit Engine gossip messages.
/// Filters, validates, and manages routing of commit protocol messages across peers.
pub struct DvfGossipValidator<B: BlockT, C, AccountId> {
    pool: Arc<DvfVotePool<B::Hash, AccountId>>,
    /// The messages this node has already seen.
    ///
    /// The set carries a capacity limit with least recently used eviction,
    /// so node memory stays inside a stated ceiling whatever the peers send.
    known_messages: RwLock<MessageSet<B::Hash>>,
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
            client,
            metrics: None,
        }
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

        // Allow the current epoch or the next one, the grace period at an
        // epoch boundary.
        match <C::Api as RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>>::get_current_epoch(
            &api, best_hash,
        ) {
            Ok(current_epoch) => {
                if epoch_id != current_epoch && epoch_id != current_epoch.saturating_add(1) {
                    warn!(
                        "Commit Engine Gossip: epoch mismatch. Expected {} (or {}), got {}",
                        current_epoch,
                        current_epoch.saturating_add(1),
                        epoch_id
                    );
                    return Err(self.reject("epoch_mismatch"));
                }
            }
            Err(_) => return Err(self.reject("runtime_api_error")),
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
            Err(_) => return Err(self.reject("runtime_api_error")),
        }
        Ok(())
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
        // Hash the bytes natively for lookups (to prevent spam re-broadcasting)
        let msg_hash = sp_core::hashing::blake2_256(data);
        let msg_hash_b = B::Hash::decode(&mut &msg_hash[..]).unwrap_or_default();

        {
            let mut known = self.known_messages.write();
            if known.record(msg_hash_b) {
                return ValidationResult::ProcessAndDiscard(msg_hash_b);
            }
        }

        match self.validate_core(data) {
            Ok(CommitMessage::Vote(msg)) => {
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_received(msg.round_number);
                }
                match self.pool.offer_vote(msg.clone()) {
                    VoteOffer::Inserted => {
                        info!(
                            "Commit Engine Gossip Validator: Accepted vote for block #{} ({:?}) from validator {:?} (round: {})",
                            msg.block_number, msg.block_hash, msg.validator_account_id, msg.round_number
                        );
                        ValidationResult::ProcessAndKeep(msg_hash_b)
                    }
                    VoteOffer::Repeated => {
                        debug!(
                            "Commit Engine Gossip Validator: Repeated vote from validator {:?} in round {} refused",
                            msg.validator_account_id, msg.round_number
                        );
                        self.reject("repeated_vote");
                        ValidationResult::Discard
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
                        ValidationResult::Discard
                    }
                }
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
                warn!(
                    "Commit Engine Gossip Validator: message rejected - reason: {}",
                    reason
                );
                ValidationResult::Discard
            }
        }
    }

    fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
        Box::new(move |_hash, _data| false)
    }

    fn message_allowed<'a>(
        &'a self,
    ) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
        Box::new(move |_who, _intent, _hash, _data| true)
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

    /// A signer key must match the account it names.
    #[test]
    fn a_vote_signed_for_another_account_is_refused() {
        let pair = ed25519::Pair::from_seed(&[4u8; 32]);
        assert!(signer_matches(&pair.public().0, &pair.public()));
        assert!(!signer_matches(&[9u8; 32], &pair.public()));
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
