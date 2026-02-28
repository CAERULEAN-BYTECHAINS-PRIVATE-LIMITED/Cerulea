use codec::{Decode, Encode};
use sc_network_gossip::{ValidatorContext, ValidationResult, MessageIntent};
use sp_runtime::traits::{Block as BlockT, NumberFor};
use sp_core::ed25519;
use std::collections::{HashMap, HashSet};
use parking_lot::RwLock;
use std::sync::Arc;
use sc_network::PeerId;
use log::{debug, warn};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;

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

    /// Prunes votes older than the finalized round.
    pub fn prune_older_rounds(&self, finalized_round: u32) {
        let mut votes = self.votes.write();
        votes.retain(|&(round, _), _| round >= finalized_round);

        let mut participation = self.participation.write();
        participation.retain(|&round, _| round >= finalized_round);
    }
}

/// Validator for DVF gossip messages.
/// Filters, validates, and manages routing of `/cbc/dvf/1` messages across peers.
pub struct DvfGossipValidator<B: BlockT, C, AccountId> {
    pool: Arc<DvfVotePool<B::Hash, AccountId>>,
    known_messages: RwLock<HashSet<B::Hash>>,
    client: Arc<C>,
}

impl<B, C, AccountId> DvfGossipValidator<B, C, AccountId>
where
    B: BlockT,
    AccountId: std::cmp::Eq + std::hash::Hash + Clone + Encode + Decode + std::fmt::Debug,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    /// Creates a new DVF Gossip Validator.
    pub fn new(pool: Arc<DvfVotePool<B::Hash, AccountId>>, client: Arc<C>) -> Self {
        Self {
            pool,
            known_messages: RwLock::new(HashSet::new()),
            client,
        }
    }

    /// Internal validation core.
    fn validate_core(&self, message_data: &[u8]) -> Result<DvfVoteMessage<B::Hash, AccountId>, ()> {
        let message = DvfVoteMessage::<B::Hash, AccountId>::decode(&mut &message_data[..]).map_err(|_| {
            warn!("Failed to decode incoming DVF Vote Message");
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
             warn!("Invalid signature on incoming DVF Vote from {:?}", message.validator_account_id);
             return Err(());
        }

        // 2. Fetch Runtime APIs and verify active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        match api.get_active_validators(best_hash) {
            Ok(active_validators) => {
                if !active_validators.contains(&message.validator_account_id) {
                    warn!("DVF Vote from inactive validator {:?}", message.validator_account_id);
                    return Err(());
                }
            },
            Err(e) => {
                warn!("Failed to fetch active validators: {:?}", e);
                return Err(());
            }
        }
        
        // 3. Verify Epoch ID
        match api.get_current_epoch(best_hash) {
            Ok(current_epoch) => {
                if message.epoch_id != current_epoch {
                    warn!("DVF Vote epoch mismatch. Expected {}, got {}", current_epoch, message.epoch_id);
                    return Err(());
                }
            },
            Err(_) => {
                warn!("Failed to fetch current epoch");
                return Err(());
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
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
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
                debug!("DVF Gossip Validator: Accepted vote for block {}", msg.block_number);
                
                // Track internally and pass to pool
                let inserted = self.pool.insert_vote(msg);
                if inserted {
                    ValidationResult::ProcessAndKeep(msg_hash_b) // Broadcast to others
                } else {
                    ValidationResult::ProcessAndDiscard(msg_hash_b) // Double vote detected, discard
                }
            }
            Err(_) => ValidationResult::Discard, // Malformed or invalid signature
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
