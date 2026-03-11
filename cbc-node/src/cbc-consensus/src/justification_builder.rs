//! Justification Builder
//!
//! This module implements the JustificationBuilder component that constructs and verifies
//! DVF justifications containing threshold-reaching votes.

use codec::Encode;
use log::{debug, info};
use sc_client_api::HeaderBackend;
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::{Block as BlockT, NumberFor};
use std::collections::HashSet;
use std::sync::Arc;

use crate::dvf_gossip::{DvfJustification, DvfVoteMessage, DvfVotePool};
use crate::metrics::DvfMetrics;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi;

/// Maximum encoded size for a justification (1 MB)
const MAX_JUSTIFICATION_SIZE: usize = 1_048_576;

/// Justification Builder
///
/// Constructs and verifies DVF justifications containing threshold-reaching votes.
pub struct JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
{
    client: Arc<Client>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Creates a new justification builder
    ///
    /// # Arguments
    /// * `client` - The blockchain client for querying runtime state
    /// * `vote_pool` - The vote pool containing accumulated votes
    pub fn new(
        client: Arc<Client>,
        vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    ) -> Self {
        info!("DVF Justification Builder: Initializing");
        Self { 
            client, 
            vote_pool,
            metrics: None,
        }
    }

    /// Sets the metrics for this builder
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Selects minimum votes needed to reach threshold
    ///
    /// This method retrieves all votes for the target block from the vote pool,
    /// queries validator weights from the runtime, sorts votes by validator weight
    /// descending, and selects the minimum votes needed to reach the threshold.
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash to select votes for
    ///
    /// # Returns
    /// A vector of selected votes sorted by weight (descending)
    fn select_votes(
        &self,
        round: u32,
        block_hash: &Block::Hash,
    ) -> Result<Vec<DvfVoteMessage<Block::Hash, AccountId>>, String> {
        debug!(
            "DVF Justification Builder: Selecting votes for block {:?} in round {}",
            block_hash, round
        );

        // 1. Retrieve all votes for target block from vote pool
        let votes = self.vote_pool.get_votes(round, block_hash);

        if votes.is_empty() {
            return Err("No votes found for block".to_string());
        }

        debug!(
            "DVF Justification Builder: Found {} votes for block",
            votes.len()
        );

        // 2. Query validator weights from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        // Create a map for quick weight lookup
        let weight_map: std::collections::HashMap<_, _> =
            validator_weights.into_iter().collect();

        // 3. Create weighted votes (vote, weight) pairs
        let mut weighted_votes: Vec<(DvfVoteMessage<Block::Hash, AccountId>, u128)> = votes
            .into_iter()
            .filter_map(|vote| {
                weight_map
                    .get(&vote.validator_account_id)
                    .map(|&weight| (vote, weight))
            })
            .collect();

        if weighted_votes.is_empty() {
            return Err("No valid votes with weights found".to_string());
        }

        // 4. Sort votes by validator weight descending
        weighted_votes.sort_by(|a, b| b.1.cmp(&a.1));

        debug!(
            "DVF Justification Builder: Sorted {} weighted votes",
            weighted_votes.len()
        );

        // 5. Query finality threshold
        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        debug!(
            "DVF Justification Builder: Total weight: {}, Threshold: {}",
            total_weight, threshold
        );

        // 6. Select minimum votes needed to reach threshold
        let mut selected_votes = Vec::new();
        let mut accumulated_weight = 0u128;

        for (vote, weight) in weighted_votes {
            selected_votes.push(vote);
            accumulated_weight = accumulated_weight.saturating_add(weight);

            if accumulated_weight >= threshold {
                break;
            }
        }

        info!(
            "DVF Justification Builder: Selected {} votes with accumulated weight {} (threshold: {})",
            selected_votes.len(),
            accumulated_weight,
            threshold
        );

        Ok(selected_votes)
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Constructs a justification for a block
    ///
    /// This method constructs a DvfJustification with the round number, block hash,
    /// and selected votes. It verifies no duplicate validators and ensures accumulated
    /// weight meets the threshold.
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash to construct justification for
    ///
    /// # Returns
    /// A verified DvfJustification
    pub fn construct_justification(
        &self,
        round: u32,
        block_hash: Block::Hash,
    ) -> Result<DvfJustification<Block::Hash, AccountId>, String> {
        info!(
            "DVF Justification Builder: Constructing justification for block {:?} in round {}",
            block_hash, round
        );

        // 1. Select votes using the vote selection algorithm
        let selected_votes = self.select_votes(round, &block_hash)?;

        // 2. Verify no duplicate validators
        let mut seen_validators = HashSet::new();
        for vote in &selected_votes {
            if !seen_validators.insert(vote.validator_account_id.clone()) {
                return Err(format!(
                    "Duplicate validator {:?} in selected votes",
                    vote.validator_account_id
                ));
            }
        }

        debug!(
            "DVF Justification Builder: Verified no duplicate validators ({} unique)",
            seen_validators.len()
        );

        // 3. Construct justification
        let justification = DvfJustification {
            round_number: round,
            block_hash: block_hash.clone(),
            votes: selected_votes,
        };

        // 4. Verify accumulated weight meets threshold
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        let weight_map: std::collections::HashMap<_, _> =
            validator_weights.into_iter().collect();

        let mut accumulated_weight = 0u128;
        for vote in &justification.votes {
            if let Some(&weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(weight);
            }
        }

        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        if accumulated_weight < threshold {
            return Err(format!(
                "Accumulated weight {} does not meet threshold {}",
                accumulated_weight, threshold
            ));
        }

        info!(
            "DVF Justification Builder: Constructed justification with {} votes, accumulated weight {} (threshold: {})",
            justification.votes.len(),
            accumulated_weight,
            threshold
        );

        // Log justification construction at info level
        info!(
            "DVF Justification Builder: Justification constructed for block {:?}, round {}, {} votes, weight {}",
            block_hash, round, justification.votes.len(), accumulated_weight
        );

        // 5. Enforce size limits
        self.enforce_size_limits(&justification)?;

        Ok(justification)
    }

    /// Enforces size limits on a justification
    ///
    /// This method checks that the encoded size doesn't exceed 1 MB and that
    /// the number of votes doesn't exceed the number of active validators.
    ///
    /// # Arguments
    /// * `justification` - The justification to check
    ///
    /// # Returns
    /// Ok if size limits are satisfied, Err otherwise
    fn enforce_size_limits(
        &self,
        justification: &DvfJustification<Block::Hash, AccountId>,
    ) -> Result<(), String> {
        // 1. Check encoded size doesn't exceed 1 MB
        let encoded = justification.encode();
        let encoded_size = encoded.len();

        if encoded_size > MAX_JUSTIFICATION_SIZE {
            return Err(format!(
                "Justification size {} exceeds maximum {} bytes",
                encoded_size, MAX_JUSTIFICATION_SIZE
            ));
        }

        debug!(
            "DVF Justification Builder: Justification encoded size: {} bytes",
            encoded_size
        );

        // 2. Limit votes to number of active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api
            .get_validator_set(best_hash)
            .map_err(|e| format!("Failed to get validator set: {:?}", e))?;

        if justification.votes.len() > active_validators.len() {
            return Err(format!(
                "Justification has {} votes but only {} active validators",
                justification.votes.len(),
                active_validators.len()
            ));
        }

        debug!(
            "DVF Justification Builder: Size limits satisfied ({} votes, {} bytes)",
            justification.votes.len(),
            encoded_size
        );

        Ok(())
    }
}

impl<Block, Client, AccountId> JustificationBuilder<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Verifies a justification is valid
    ///
    /// This method verifies all vote signatures, checks that all votes are from active
    /// validators, verifies accumulated weight meets threshold, and ensures all votes
    /// have matching validator_set_id. Logs verification at info level.
    ///
    /// # Arguments
    /// * `justification` - The justification to verify
    ///
    /// # Returns
    /// Ok if justification is valid, Err with reason otherwise
    pub fn verify_justification(
        &self,
        justification: &DvfJustification<Block::Hash, AccountId>,
    ) -> Result<(), String> {
        info!(
            "DVF Justification Builder: Verifying justification for block {:?} in round {}",
            justification.block_hash, justification.round_number
        );

        // 1. Check vote count
        if justification.votes.is_empty() {
            return Err("Justification has no votes".to_string());
        }

        // 2. Query runtime state
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api
            .get_validator_set(best_hash)
            .map_err(|e| format!("Failed to get validator set: {:?}", e))?;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        let weight_map: std::collections::HashMap<_, _> =
            validator_weights.into_iter().collect();

        // 3. Verify all votes
        let mut seen_validators = HashSet::new();
        let mut accumulated_weight = 0u128;
        let mut expected_validator_set_id: Option<u32> = None;

        for vote in &justification.votes {
            // Check for duplicate validators
            if !seen_validators.insert(vote.validator_account_id.clone()) {
                return Err(format!(
                    "Duplicate validator {:?} in justification",
                    vote.validator_account_id
                ));
            }

            // Verify signature
            let mut encoded_payload = Vec::new();
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_number.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account_id.encode_to(&mut encoded_payload);

            if !sp_io::crypto::ed25519_verify(
                &vote.signature,
                &encoded_payload,
                &vote.validator_public_key,
            ) {
                return Err(format!(
                    "Invalid signature for validator {:?}",
                    vote.validator_account_id
                ));
            }

            // Check validator set ID consistency
            if let Some(expected_id) = expected_validator_set_id {
                if vote.validator_set_id != expected_id {
                    return Err(format!(
                        "Validator set ID mismatch: expected {}, got {}",
                        expected_id, vote.validator_set_id
                    ));
                }
            } else {
                expected_validator_set_id = Some(vote.validator_set_id);
            }

            // Check round consistency
            if vote.round_number != justification.round_number {
                return Err(format!(
                    "Round mismatch: expected {}, got {}",
                    justification.round_number, vote.round_number
                ));
            }

            // Check block hash consistency
            if vote.block_hash != justification.block_hash {
                return Err(format!(
                    "Block hash mismatch in vote from validator {:?}",
                    vote.validator_account_id
                ));
            }

            // Verify validator is active
            if !active_validators.contains(&vote.validator_account_id) {
                return Err(format!(
                    "Validator {:?} is not in active set",
                    vote.validator_account_id
                ));
            }

            // Accumulate weight
            if let Some(&weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(weight);
            } else {
                return Err(format!(
                    "Validator {:?} has no weight",
                    vote.validator_account_id
                ));
            }
        }

        // 4. Verify accumulated weight meets threshold
        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        let total_weight: u128 = weight_map.values().sum();
        let threshold = finality_threshold_perbill * total_weight;

        if accumulated_weight < threshold {
            return Err(format!(
                "Accumulated weight {} does not meet threshold {}",
                accumulated_weight, threshold
            ));
        }

        info!(
            "DVF Justification Builder: Justification verified successfully - {} votes, accumulated weight {} (threshold: {})",
            justification.votes.len(),
            accumulated_weight,
            threshold
        );

        // Log justification verification at info level
        info!(
            "DVF Justification Builder: Justification verified for block {:?}, round {}, {} votes, weight {}",
            justification.block_hash, justification.round_number, justification.votes.len(), accumulated_weight
        );

        Ok(())
    }
}