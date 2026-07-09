use codec::{Decode, Encode};
use sc_network_gossip::{ValidatorContext, ValidationResult, MessageIntent};
use sp_runtime::traits::{Block as BlockT, NumberFor};
use sp_core::ed25519;
use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use std::sync::Arc;
use sc_network::PeerId;
use log::{debug, info, warn};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi as RuntimeDvfApi;
use crate::metrics::DvfMetrics;

/// The protocol ID for DVF Vote Gossiping.
pub const DVF_PROTOCOL_NAME: &str = "/cbc/dvf/1";

/// The structure of a DVF Vote gossiped over the P2P network.
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


/// The structure of a DVF Justification containing a threshold of votes
#[derive(Debug, Clone, Encode, Decode, PartialEq, Eq)]
pub struct DvfJustification<Hash, AccountId> {
    /// Round number for this justification
    pub round_number: u32,
    /// Block hash being justified
    pub block_hash: Hash,
    /// Collection of votes that justify this block
    pub votes: Vec<DvfVoteMessage<Hash, AccountId>>,
}

/// A thread-safe, in-memory pool storing DVF votes.
pub struct DvfVotePool<Hash, AccountId> {
    votes: RwLock<HashMap<(u32, Hash), Vec<DvfVoteMessage<Hash, AccountId>>>>,
    participation: RwLock<HashMap<u32, HashSet<AccountId>>>,
}

impl<Hash: std::cmp::Eq + std::hash::Hash + Clone, AccountId: std::cmp::Eq + std::hash::Hash + Clone> DvfVotePool<Hash, AccountId> {
    /// Creates a new DVF Vote Pool.
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            votes: RwLock::new(HashMap::new()),
            participation: RwLock::new(HashMap::new()),
        })
    }

    /// Inserts a vote into the pool.
    pub fn insert_vote(&self, vote: DvfVoteMessage<Hash, AccountId>) -> bool {
        let mut participation = self.participation.write();
        let round_voters = participation.entry(vote.round_number).or_insert_with(HashSet::new);
        
        if !round_voters.insert(vote.validator_account_id.clone()) {
            return false;
        }
        
        let mut votes = self.votes.write();
        let block_votes = votes.entry((vote.round_number, vote.block_hash.clone())).or_insert_with(Vec::new);
        block_votes.push(vote);
        
        true
    }

    /// Gets votes for a specific round and block hash.
    pub fn get_votes(&self, round: u32, block_hash: &Hash) -> Vec<DvfVoteMessage<Hash, AccountId>> {
        let votes = self.votes.read();
        votes.get(&(round, block_hash.clone())).cloned().unwrap_or_default()
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

    /// Prunes votes older than the finalized round.
    pub fn prune_older_rounds(&self, finalized_round: u32) {
        let mut votes = self.votes.write();
        votes.retain(|&(round, _), _| round >= finalized_round);

        let mut participation = self.participation.write();
        participation.retain(|&round, _| round >= finalized_round);
    }

    /// Prunes votes for rounds older than (current_round - retention_rounds).
    ///
    /// This implements round-based pruning to prevent unbounded memory growth.
    /// Removes both votes and participation tracking for old rounds.
    ///
    /// # Arguments
    /// * `current_round` - The current round number
    /// * `retention_rounds` - Number of rounds to retain
    pub fn prune_by_round_age(&self, current_round: u32, retention_rounds: u32) -> usize {
        let cutoff_round = current_round.saturating_sub(retention_rounds);
        
        let mut votes = self.votes.write();
        let initial_count = votes.len();
        votes.retain(|&(round, _), _| round >= cutoff_round);
        let votes_removed = initial_count - votes.len();

        let mut participation = self.participation.write();
        participation.retain(|&round, _| round >= cutoff_round);
        
        votes_removed
    }

    /// Prunes votes for blocks older than the finalized block number.
    ///
    /// This removes votes for blocks that have already been finalized,
    /// as they are no longer needed for consensus.
    ///
    /// # Arguments
    /// * `finalized_block_number` - The finalized block number
    ///
    /// # Returns
    /// The number of vote entries removed
    pub fn prune_by_finalized_block(&self, finalized_block_number: u32) -> usize {
        let mut votes = self.votes.write();
        let initial_count = votes.len();
        
        // Remove votes for blocks strictly below the finalized checkpoint.
        // Votes for the finalized checkpoint block itself (block_number == finalized_block_number)
        // are retained so the aggregator can still act on them during this pruning cycle.
        votes.retain(|_, vote_list| {
            vote_list.retain(|vote| vote.block_number >= finalized_block_number);
            !vote_list.is_empty()
        });
        
        let entries_removed = initial_count - votes.len();
        entries_removed
    }

    /// Enforces maximum pool size by pruning oldest votes first.
    ///
    /// This prevents unbounded memory growth by removing the oldest
    /// vote entries when the pool exceeds the maximum size.
    ///
    /// # Arguments
    /// * `max_size` - Maximum number of vote entries to keep
    ///
    /// # Returns
    /// The number of vote entries removed
    pub fn enforce_max_size(&self, max_size: usize) -> usize {
        let mut votes = self.votes.write();
        
        if votes.len() <= max_size {
            return 0;
        }
        
        // Collect all entries with their rounds (for sorting by age)
        let mut entries: Vec<_> = votes.iter().map(|((round, hash), _)| (*round, hash.clone())).collect();
        
        // Sort by round (oldest first)
        entries.sort_by_key(|(round, _)| *round);
        
        // Calculate how many to remove
        let to_remove = votes.len() - max_size;
        
        // Remove oldest entries
        for (round, hash) in entries.iter().take(to_remove) {
            votes.remove(&(*round, hash.clone()));
        }
        
        to_remove
    }

    /// Gets the current size of the vote pool (number of vote entries).
    pub fn size(&self) -> usize {
        let votes = self.votes.read();
        votes.len()
    }

    /// Gets the total number of votes across all entries.
    pub fn total_votes(&self) -> usize {
        let votes = self.votes.read();
        votes.values().map(|v| v.len()).sum()
    }

    /// Clears all votes and participation tracking (used on validator set changes).
    pub fn clear(&self) {
        let mut votes = self.votes.write();
        votes.clear();

        let mut participation = self.participation.write();
        participation.clear();
    }

}

/// Validator for DVF gossip messages.
/// Filters, validates, and manages routing of `/cbc/dvf/1` messages across peers.
pub struct DvfGossipValidator<B: BlockT, C, AccountId> {
    pool: Arc<DvfVotePool<B::Hash, AccountId>>,
    known_messages: RwLock<HashSet<B::Hash>>,
    client: Arc<C>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<B, C, AccountId> DvfGossipValidator<B, C, AccountId>
where
    B: BlockT,
    AccountId: std::cmp::Eq + std::hash::Hash + Clone + Encode + Decode + std::fmt::Debug,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>,
{
    /// Creates a new DVF Gossip Validator.
    pub fn new(pool: Arc<DvfVotePool<B::Hash, AccountId>>, client: Arc<C>) -> Self {
        Self {
            pool,
            known_messages: RwLock::new(HashSet::new()),
            client,
            metrics: None,
        }
    }

    /// Sets the metrics for this validator
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Internal validation core.
    fn validate_core(&self, message_data: &[u8]) -> Result<DvfVoteMessage<B::Hash, AccountId>, &'static str> {
        let message = DvfVoteMessage::<B::Hash, AccountId>::decode(&mut &message_data[..]).map_err(|_| {
            warn!("DVF Gossip: Failed to decode incoming DVF Vote Message");
            if let Some(ref metrics) = self.metrics {
                metrics.record_vote_rejection("invalid_encoding");
            }
            "invalid_encoding"
        })?;

        // 1. Verify Cryptographic Integrity
        let mut encoded_payload = Vec::new();
        message.epoch_id.encode_to(&mut encoded_payload);
        message.validator_set_id.encode_to(&mut encoded_payload);
        message.round_number.encode_to(&mut encoded_payload);
        message.block_number.encode_to(&mut encoded_payload);
        message.block_hash.encode_to(&mut encoded_payload);
        message.validator_account_id.encode_to(&mut encoded_payload);

        // The SCALE decode process already validates the type for `ed25519::Public` bytes.
        
        // For Ed25519 standard Substrate:
        if !sp_io::crypto::ed25519_verify(&message.signature, &encoded_payload, &message.validator_public_key) {
             warn!("DVF Gossip: Invalid signature on incoming DVF Vote from {:?}", message.validator_account_id);
             if let Some(ref metrics) = self.metrics {
                 metrics.record_vote_rejection("invalid_signature");
             }
             return Err("invalid_signature");
        }

        // 2. Fetch Runtime APIs and verify active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        match api.get_active_validators(best_hash) {
            Ok(active_validators) => {
                if !active_validators.contains(&message.validator_account_id) {
                    warn!("DVF Gossip: DVF Vote from inactive validator {:?}", message.validator_account_id);
                    if let Some(ref metrics) = self.metrics {
                        metrics.record_vote_rejection("inactive_validator");
                    }
                    return Err("inactive_validator");
                }
            },
            Err(e) => {
                warn!("DVF Gossip: Failed to fetch active validators: {:?}", e);
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_rejection("runtime_api_error");
                }
                return Err("runtime_api_error");
            }
        }
        
        // 3. Verify Epoch ID - allow current epoch or current_epoch+1 (grace period at epoch boundary)
        match <C::Api as RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>>::get_current_epoch(&api, best_hash) {
            Ok(current_epoch) => {
                if message.epoch_id != current_epoch && message.epoch_id != current_epoch.saturating_add(1) {
                    warn!("DVF Gossip: DVF Vote epoch mismatch. Expected {} (or {}), got {}", current_epoch, current_epoch.saturating_add(1), message.epoch_id);
                    if let Some(ref metrics) = self.metrics {
                        metrics.record_vote_rejection("epoch_mismatch");
                    }
                    return Err("epoch_mismatch");
                }
            },
            Err(_) => {
                warn!("DVF Gossip: Failed to fetch current epoch");
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_rejection("runtime_api_error");
                }
                return Err("runtime_api_error");
            }
        }
        
        // 4. Verify Validator Set ID (with grace period)
        match api.get_validator_set_id(best_hash) {
            Ok(current_validator_set_id) => {
                if message.validator_set_id != current_validator_set_id {
                    // Check if this is within the grace period (previous validator set ID)
                    let is_grace_period = if message.validator_set_id == current_validator_set_id.saturating_sub(1) {
                        // Check if we're within one block of the validator set change
                        match api.get_validator_set_id_changed_at(best_hash) {
                            Ok(Some(changed_at)) => {
                                // Get current block number
                                let current_block = self.client.info().best_number;
                                // Allow if within one block of the change
                                current_block <= changed_at + 1u32.into()
                            },
                            _ => false,
                        }
                    } else {
                        false
                    };
                    
                    if !is_grace_period {
                        warn!("DVF Gossip: DVF Vote validator set ID mismatch. Expected {}, got {} from validator {:?}", 
                            current_validator_set_id, message.validator_set_id, message.validator_account_id);
                        if let Some(ref metrics) = self.metrics {
                            metrics.record_vote_rejection("validator_set_mismatch");
                        }
                        return Err("validator_set_mismatch");
                    } else {
                        debug!("DVF Gossip: DVF Vote accepted with previous validator set ID {} during grace period", message.validator_set_id);
                    }
                }
            },
            Err(_) => {
                warn!("DVF Gossip: Failed to fetch current validator set ID");
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_rejection("runtime_api_error");
                }
                return Err("runtime_api_error");
            }
        }

        Ok(message)
    }
}

impl<B, C, AccountId> sc_network_gossip::Validator<B> for DvfGossipValidator<B, C, AccountId>
where
    B: BlockT + sp_runtime::traits::Block,
    AccountId: std::cmp::Eq + std::hash::Hash + Clone + Encode + Decode + Send + Sync + std::fmt::Debug + 'static,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + RuntimeDvfApi<B, NumberFor<B>, AccountId, B::Hash>,
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

        let mut known = self.known_messages.write();
        if !known.insert(msg_hash_b.clone()) {
            return ValidationResult::ProcessAndDiscard(msg_hash_b); // Seen this message already
        }

        match self.validate_core(data) {
            Ok(msg) => {
                info!(
                    "DVF Gossip Validator: Accepted vote for block #{} ({:?}) from validator {:?} (round: {}, epoch: {}, validator_set: {})",
                    msg.block_number, msg.block_hash, msg.validator_account_id, msg.round_number, msg.epoch_id, msg.validator_set_id
                );
                
                // Record vote reception metrics
                if let Some(ref metrics) = self.metrics {
                    metrics.record_vote_received(msg.round_number);
                }
                
                // Track internally and pass to pool
                let inserted = self.pool.insert_vote(msg.clone());
                if inserted {
                    info!(
                        "DVF Gossip Validator: Vote inserted into pool successfully"
                    );
                    ValidationResult::ProcessAndKeep(msg_hash_b) // Broadcast to others
                } else {
                    // Double vote detected
                    warn!(
                        "DVF Gossip Validator: Double vote detected from validator {:?} in round {} - REJECTED",
                        msg.validator_account_id, msg.round_number
                    );
                    if let Some(ref metrics) = self.metrics {
                        metrics.record_double_vote_detection();
                        metrics.record_vote_rejection("double_vote");
                    }
                    ValidationResult::ProcessAndDiscard(msg_hash_b)
                }
            }
            Err(reason) => {
                warn!("DVF Gossip Validator: Vote rejected - reason: {}", reason);
                ValidationResult::Discard
            }
        }
    }

    fn message_expired<'a>(&'a self) -> Box<dyn FnMut(B::Hash, &[u8]) -> bool + 'a> {
        // Here we could parse the vote and see if the block is older than `finalized_head`
        // For simplicity now, we rely on `prune_older_rounds` directly.
        Box::new(move |_hash, _data| false)
    }

    fn message_allowed<'a>(
        &'a self,
    ) -> Box<dyn FnMut(&PeerId, MessageIntent, &B::Hash, &[u8]) -> bool + 'a> {
        // For strict topologies, determine if we should send this specific peer this hash
        Box::new(move |_who, _intent, _hash, _data| true)
    }
}
