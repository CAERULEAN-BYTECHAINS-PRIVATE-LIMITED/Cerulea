//! Vote Creator Service
//!
//! This module implements the Vote Creator Service which monitors imported checkpoint blocks
//! and creates signed DVF votes for them when the node is an active validator.

use codec::{Decode, Encode};
use futures::StreamExt;
use log::{debug, info, warn};
use sc_client_api::{BlockchainEvents, HeaderBackend};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderMetadata;
use sp_core::ed25519;
use sp_keystore::{Keystore, KeystorePtr};
use sp_runtime::traits::{Block as BlockT, Header, NumberFor};
use std::sync::{Arc, Mutex};

use crate::dvf_gossip::{DvfVoteMessage, DvfVotePool};
use crate::metrics::DvfMetrics;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi as RuntimeDvfApi;

/// Vote Creator Service
///
/// Monitors imported blocks and creates votes for checkpoint blocks when the node
/// is an active validator.
pub struct VoteCreatorService<Block, Client, AccountId>
where
    Block: BlockT,
{
    client: Arc<Client>,
    keystore: KeystorePtr,
    gossip_engine: Arc<Mutex<sc_network_gossip::GossipEngine<Block>>>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    validator_account: AccountId,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<Block, Client, AccountId> VoteCreatorService<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + BlockchainEvents<Block>
        + HeaderMetadata<Block, Error = sp_blockchain::Error>
        + Send
        + Sync
        + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>> + RuntimeDvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::hash::Hash + std::fmt::Debug + Send + Sync + 'static,
{
    /// Creates a new Vote Creator Service
    pub fn new(
        client: Arc<Client>,
        keystore: KeystorePtr,
        gossip_engine: Arc<Mutex<sc_network_gossip::GossipEngine<Block>>>,
        vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
        validator_account: AccountId,
    ) -> Self {
        info!("DVF Vote Creator: Initializing service for validator {:?}", validator_account);
        Self {
            client,
            keystore,
            gossip_engine,
            vote_pool,
            validator_account,
            metrics: None,
        }
    }

    /// Sets the metrics for this service
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Starts the service as a background task
    pub async fn run(self) {
        info!("DVF Vote Creator: Starting service");

        // Subscribe to block import notifications
        let mut import_notifications = self.client.import_notification_stream();

        while let Some(notification) = import_notifications.next().await {
            let block_number = *notification.header.number();
            let block_hash = notification.hash;

            debug!(
                "DVF Vote Creator: Processing imported block #{} ({:?})",
                block_number, block_hash
            );

            // Process the block
            if let Err(e) = self.process_block(block_number, block_hash).await {
                warn!(
                    "DVF Vote Creator: Failed to process block #{}: {:?}",
                    block_number, e
                );
            }
        }

        warn!("DVF Vote Creator: Import notification stream ended");
    }

    /// Processes a single imported block
    async fn process_block(
        &self,
        block_number: NumberFor<Block>,
        block_hash: Block::Hash,
    ) -> Result<(), String> {
        // Check if this is a checkpoint block
        if !self.is_checkpoint_block(block_number)? {
            debug!(
                "DVF Vote Creator: Block #{} is not a checkpoint, skipping",
                block_number
            );
            return Ok(());
        }

        debug!(
            "DVF Vote Creator: Block #{} is a checkpoint block",
            block_number
        );

        // Check if block is already finalized
        if self.is_already_finalized(block_number)? {
            debug!(
                "DVF Vote Creator: Block #{} is already finalized, skipping",
                block_number
            );
            return Ok(());
        }

        // Check if node is an active validator
        if !self.is_active_validator()? {
            debug!("DVF Vote Creator: Node is not an active validator, skipping");
            return Ok(());
        }

        // Create and broadcast vote
        self.create_and_broadcast_vote(block_number, block_hash)
            .await?;

        Ok(())
    }

    /// Checks if a block is a checkpoint block
    fn is_checkpoint_block(&self, block_number: NumberFor<Block>) -> Result<bool, String> {
        // Read FinalityCheckpointInterval from the DVF pallet runtime API so that
        // the Vote_Creator and the DVF_Pallet always use the same value.
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let checkpoint_interval: u32 = api
            .get_finality_checkpoint_interval(best_hash)
            .map_err(|e| format!("Failed to get FinalityCheckpointInterval: {:?}", e))?
            .try_into()
            .map_err(|_| "FinalityCheckpointInterval conversion failed")?;

        // Convert block_number to u32 for modulo operation
        let block_num_u32: u32 = block_number
            .try_into()
            .map_err(|_| "Block number conversion failed")?;

        // Check if block number is a multiple of checkpoint interval
        Ok(checkpoint_interval > 0 && block_num_u32 % checkpoint_interval == 0)
    }

    /// Checks if a block is already finalized
    fn is_already_finalized(&self, block_number: NumberFor<Block>) -> Result<bool, String> {
        Ok(crate::types::utils::is_block_finalized(&*self.client, block_number))
    }

    /// Checks if the node is an active validator
    fn is_active_validator(&self) -> Result<bool, String> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api
            .get_active_validators(best_hash)
            .map_err(|e| format!("Failed to get active validators: {:?}", e))?;

        Ok(active_validators.contains(&self.validator_account))
    }

    /// Creates and broadcasts a vote for a checkpoint block
    async fn create_and_broadcast_vote(
        &self,
        block_number: NumberFor<Block>,
        block_hash: Block::Hash,
    ) -> Result<(), String> {
        info!(
            "DVF Vote Creator: Creating vote for block #{} ({:?})",
            block_number, block_hash
        );

        // Get runtime state
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let epoch_id = <Client::Api as RuntimeDvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>>::get_current_epoch(&api, best_hash)
            .map_err(|e| format!("Failed to get current epoch: {:?}", e))?;

        // Get current validator set ID from runtime
        let validator_set_id = api
            .get_validator_set_id(best_hash)
            .map_err(|e| format!("Failed to get validator set ID: {:?}", e))?;

        // Get current round number from runtime
        let round_number = api
            .get_current_round(best_hash)
            .map_err(|e| format!("Failed to get current round: {:?}", e))?;

        // Get validator public key from keystore
        let public_key = self.get_validator_public_key()?;

        // Convert block_number to u32
        let block_num_u32: u32 = block_number
            .try_into()
            .map_err(|_| "Block number conversion failed")?;

        // Construct vote message with placeholder signature
        let mut vote = DvfVoteMessage {
            epoch_id,
            validator_set_id,
            round_number,
            block_number: block_num_u32,
            block_hash,
            validator_account_id: self.validator_account.clone(),
            validator_public_key: public_key,
            signature: ed25519::Signature::from_raw([0u8; 64]),
        };

        // Sign the vote with validator's private key
        vote.signature = self.sign_vote(&vote)?;

        // Broadcast the vote
        self.broadcast_vote(vote).await?;

        Ok(())
    }

    /// Gets the validator's public key from the keystore
    fn get_validator_public_key(&self) -> Result<ed25519::Public, String> {
        // Get all ed25519 public keys from keystore using the CBC DVF key type
        let public_keys = Keystore::ed25519_public_keys(&*self.keystore, crate::CBC_DVF_KEY_TYPE);

        if public_keys.is_empty() {
            return Err("No CBC DVF ed25519 keys found in keystore. Ensure the node started with --validator and key auto-generation ran.".to_string());
        }

        // Use the first key (in production, this should match validator_account)
        Ok(public_keys[0])
    }

    /// Signs a vote using the validator's private key
    fn sign_vote(&self, vote: &DvfVoteMessage<Block::Hash, AccountId>) -> Result<ed25519::Signature, String> {
        // Construct the signed payload
        let mut payload = Vec::new();
        vote.epoch_id.encode_to(&mut payload);
        vote.validator_set_id.encode_to(&mut payload);
        vote.round_number.encode_to(&mut payload);
        vote.block_number.encode_to(&mut payload);
        vote.block_hash.encode_to(&mut payload);
        vote.validator_account_id.encode_to(&mut payload);

        debug!(
            "DVF Vote Creator: Signing payload of {} bytes for block #{}",
            payload.len(),
            vote.block_number
        );

        // Sign using keystore
        let signature = Keystore::ed25519_sign(
                &*self.keystore,
                crate::CBC_DVF_KEY_TYPE,
                &vote.validator_public_key,
                &payload,
            )
            .map_err(|e| format!("Failed to sign vote: {:?}", e))?
            .ok_or_else(|| "Keystore returned no signature".to_string())?;

        Ok(signature)
    }

    /// Broadcasts a vote via the gossip protocol
    async fn broadcast_vote(&self, vote: DvfVoteMessage<Block::Hash, AccountId>) -> Result<(), String> {
        debug!(
            "DVF Vote Creator: Broadcasting vote for block #{} to gossip network",
            vote.block_number
        );

        // Insert vote into local pool FIRST before broadcasting
        // This ensures our own vote is counted even if gossip network has issues
        let inserted = self.vote_pool.insert_vote(vote.clone());
        if inserted {
            info!(
                "DVF Vote Creator: Vote inserted into local pool successfully"
            );
        } else {
            warn!(
                "DVF Vote Creator: Failed to insert vote into local pool (possible double vote)"
            );
        }

        // Encode the vote
        let encoded_vote = vote.encode();

        // Create a topic hash for the message (same pattern as gossip validator)
        let msg_hash = sp_core::hashing::blake2_256(&encoded_vote);
        let topic = Block::Hash::decode(&mut &msg_hash[..])
            .map_err(|e| format!("Failed to decode message hash: {:?}", e))?;

        // Broadcast via gossip engine
        let mut gossip_engine = self.gossip_engine.lock()
            .map_err(|e| format!("Failed to lock gossip engine: {:?}", e))?;
        gossip_engine.gossip_message(
            topic,
            encoded_vote,
            false, // Don't force send
        );

        info!(
            "DVF Vote Creator: Successfully broadcast vote for block #{} (epoch: {}, round: {}, validator_set: {}, validator: {:?})",
            vote.block_number, vote.epoch_id, vote.round_number, vote.validator_set_id, vote.validator_account_id
        );

        Ok(())
    }
}