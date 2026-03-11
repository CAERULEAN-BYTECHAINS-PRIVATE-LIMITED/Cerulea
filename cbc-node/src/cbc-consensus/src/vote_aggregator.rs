//! Vote Aggregator Service
//!
//! This module implements the Vote Aggregator Service which continuously monitors
//! vote accumulation in the vote pool and triggers justification construction when
//! the finality threshold is reached.

use codec::Encode;
use log::{debug, info, warn};
use sc_client_api::{HeaderBackend, backend::{Finalizer, LockImportRun}};
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::{Block as BlockT, Header, NumberFor, SaturatedConversion};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    dvf_gossip::DvfVotePool,
    dvf_block_import::{FinalityNotifier, DVF_ENGINE_ID},
    justification_builder::JustificationBuilder,
    metrics::DvfMetrics,
};
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi;

/// Vote Aggregator Service
///
/// Monitors vote accumulation in the vote pool and triggers justification construction
/// when the finality threshold is reached.
pub struct VoteAggregatorService<Block, Backend, Client, AccountId>
where
    Block: BlockT,
    Backend: sc_client_api::backend::Backend<Block>,
{
    client: Arc<Client>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    justification_builder: Arc<JustificationBuilder<Block, Client, AccountId>>,
    check_interval: Duration,
    last_finalized_round: std::sync::RwLock<u32>,
    stall_warning_threshold: u32,
    last_validator_set_id: std::sync::RwLock<u32>,
    metrics: Option<Arc<DvfMetrics>>,
    checkpoint_times: Arc<std::sync::RwLock<std::collections::HashMap<u32, std::time::Instant>>>,
    _phantom: std::marker::PhantomData<Backend>,
}

impl<Block, Backend, Client, AccountId> VoteAggregatorService<Block, Backend, Client, AccountId>
where
    Block: BlockT,
    Backend: sc_client_api::backend::Backend<Block>,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Finalizer<Block, Backend> + LockImportRun<Block, Backend> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Creates a new Vote Aggregator Service
    ///
    /// # Arguments
    /// * `client` - The blockchain client for querying runtime state
    /// * `vote_pool` - The vote pool containing accumulated votes
    /// * `justification_builder` - The justification builder for constructing justifications
    /// * `check_interval` - How often to check vote accumulation (default: 1 second)
    pub fn new(
        client: Arc<Client>,
        vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
        justification_builder: Arc<JustificationBuilder<Block, Client, AccountId>>,
        check_interval: Duration,
    ) -> Self {
        info!(
            "DVF Vote Aggregator: Initializing service with check interval {:?}",
            check_interval
        );
        Self {
            client,
            vote_pool,
            justification_builder,
            check_interval,
            last_finalized_round: std::sync::RwLock::new(0),
            stall_warning_threshold: 10, // Warn if no finality for 10 rounds
            last_validator_set_id: std::sync::RwLock::new(0),
            metrics: None,
            checkpoint_times: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Sets the metrics for this service
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Starts the service as a background task
    ///
    /// This method runs continuously, checking vote accumulation at regular intervals
    /// and triggering justification construction when the finality threshold is reached.
    /// The task will run until the tokio runtime is shut down.
    pub async fn run(self) {
        info!("DVF Vote Aggregator: Starting service");

        // Create a cancellation token for graceful shutdown
        let mut interval = tokio::time::interval(self.check_interval);
        
        loop {
            // Wait for the next tick
            interval.tick().await;
            
            // Check for validator set changes and clear pool if needed
            if let Err(e) = self.check_validator_set_changes() {
                warn!(
                    "DVF Vote Aggregator: Error checking validator set changes: {:?}",
                    e
                );
            }
            
            // Check vote accumulation
            if let Err(e) = self.check_vote_accumulation().await {
                warn!(
                    "DVF Vote Aggregator: Error checking vote accumulation: {:?}",
                    e
                );
            }
        }
    }
    
    /// Checks for validator set changes and clears the vote pool if the set has changed
    fn check_validator_set_changes(&self) -> Result<(), String> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get current validator set ID
        let current_validator_set_id = api
            .get_validator_set_id(best_hash)
            .map_err(|e| format!("Failed to get validator set ID: {:?}", e))?;
        
        // Check if it has changed
        let mut last_id = self.last_validator_set_id.write().unwrap();
        if *last_id != 0 && *last_id != current_validator_set_id {
            info!(
                "DVF Vote Aggregator: Validator set changed from {} to {}. Clearing vote pool.",
                *last_id, current_validator_set_id
            );
            self.vote_pool.clear();
        }
        
        // Update last known validator set ID
        *last_id = current_validator_set_id;
        
        Ok(())
    }

    /// Checks vote accumulation for the current round
    ///
    /// This method queries the runtime for the current round and finality threshold,
    /// then checks if any candidate blocks have accumulated enough weight to be finalized.
    async fn check_vote_accumulation(&self) -> Result<(), String> {
        // Get runtime state
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Query current round
        let current_round = api
            .get_current_round(best_hash)
            .map_err(|e| format!("Failed to get current round: {:?}", e))?;

        info!(
            "DVF Vote Aggregator: ===== Checking vote accumulation for round {} =====",
            current_round
        );

        // Check for finality stall
        self.check_finality_stall(current_round);

        // Query finality threshold
        let finality_threshold_perbill = api
            .get_finality_threshold_perbill(best_hash)
            .map_err(|e| format!("Failed to get finality threshold: {:?}", e))?;

        // Query validator weights to calculate total weight
        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        let total_weight: u128 = validator_weights.iter().map(|(_, weight)| weight).sum();
        let finality_threshold = finality_threshold_perbill * total_weight;

        info!(
            "DVF Vote Aggregator: Total weight: {}, Finality threshold: {} ({:?} of total)",
            total_weight, finality_threshold, finality_threshold_perbill
        );

        // Get all candidate blocks from the vote pool
        let candidate_blocks = self.vote_pool.get_candidate_blocks(current_round);

        info!(
            "DVF Vote Aggregator: Found {} candidate blocks for round {}",
            candidate_blocks.len(),
            current_round
        );
        
        // Log vote pool statistics
        info!(
            "DVF Vote Aggregator: Vote pool size: {} votes total",
            self.vote_pool.total_votes()
        );

        // Track the best candidate (highest weight that meets threshold)
        let mut best_candidate: Option<Block::Hash> = None;
        let mut best_weight: u128 = 0;

        // Check each candidate block
        for block_hash in candidate_blocks {
            let accumulated_weight = self.calculate_accumulated_weight(current_round, &block_hash)?;

            info!(
                "DVF Vote Aggregator: Block {:?} has accumulated weight {} (threshold: {}, reached: {})",
                block_hash, accumulated_weight, finality_threshold, accumulated_weight >= finality_threshold
            );

            // Update accumulated weight metric
            if let Some(ref metrics) = self.metrics {
                metrics.update_accumulated_weight(&format!("{:?}", block_hash), accumulated_weight);
            }

            // Check if threshold is reached
            if accumulated_weight >= finality_threshold {
                // Select block with highest weight if multiple reach threshold
                if accumulated_weight > best_weight {
                    best_candidate = Some(block_hash);
                    best_weight = accumulated_weight;
                }
            }
        }

        // If we found a candidate that reached threshold, trigger justification
        if let Some(block_hash) = best_candidate {
            // First check if this block is already finalized to avoid re-finalization attempts
            let block_number: NumberFor<Block> = self.client
                .header(block_hash)
                .map_err(|e| format!("Failed to get header: {:?}", e))?
                .ok_or_else(|| "Block header not found".to_string())?
                .number()
                .clone();
            
            if crate::types::utils::is_block_finalized(&*self.client, block_number) {
                let client_finalized_number = self.client.info().finalized_number;
                debug!(
                    "DVF Vote Aggregator: Block #{} ({:?}) is already finalized (client finalized: #{}), skipping",
                    block_number, block_hash, client_finalized_number
                );
                return Ok(());
            }
            
            info!(
                "DVF Vote Aggregator: Block {:?} reached finality threshold! Weight: {}, Threshold: {}",
                block_hash, best_weight, finality_threshold
            );

            // Record checkpoint time for latency measurement
            let checkpoint_time = std::time::Instant::now();
            if let Ok(mut times) = self.checkpoint_times.write() {
                times.insert(current_round, checkpoint_time);
            }

            // Trigger justification construction with retry logic
            match self.trigger_justification_construction(current_round, block_hash, best_weight).await {
                Ok(_) => {
                    // Update last finalized round
                    if let Ok(mut last_round) = self.last_finalized_round.write() {
                        *last_round = current_round;
                    }

                    // Calculate and record finality latency
                    if let Ok(times) = self.checkpoint_times.read() {
                        if let Some(start_time) = times.get(&current_round) {
                            let latency = start_time.elapsed().as_secs_f64();
                            if let Some(ref metrics) = self.metrics {
                                metrics.record_finality_latency(latency);
                            }
                            info!(
                                "DVF Vote Aggregator: Finality latency for round {}: {:.2}s",
                                current_round, latency
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "DVF Vote Aggregator: Failed to trigger justification construction: {:?}. Will retry on next check.",
                        e
                    );
                    // Don't return error - we'll retry on the next interval
                }
            }
        }

        Ok(())
    }

    /// Checks if finality has stalled and logs a warning if needed
    fn check_finality_stall(&self, current_round: u32) {
        if let Ok(last_finalized) = self.last_finalized_round.read() {
            let rounds_since_finality = current_round.saturating_sub(*last_finalized);
            
            if rounds_since_finality >= self.stall_warning_threshold {
                warn!(
                    "DVF Vote Aggregator: Finality stalled! No blocks finalized for {} rounds (current: {}, last finalized: {})",
                    rounds_since_finality, current_round, *last_finalized
                );
            }
        }
    }

    /// Triggers justification construction for a block that reached the finality threshold
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash that reached threshold
    /// * `accumulated_weight` - The accumulated weight for the block
    async fn trigger_justification_construction(
        &self,
        round: u32,
        block_hash: Block::Hash,
        accumulated_weight: u128,
    ) -> Result<(), String> {
        info!(
            "DVF Vote Aggregator: Triggering justification construction for block {:?} (round: {}, weight: {})",
            block_hash, round, accumulated_weight
        );

        // Construct justification using the justification builder
        let justification = self.justification_builder
            .construct_justification(round, block_hash)
            .map_err(|e| format!("Failed to construct justification: {}", e))?;

        info!(
            "DVF Vote Aggregator: Successfully constructed justification with {} votes",
            justification.votes.len()
        );

        // Get the block number from the justification
        let block_number: NumberFor<Block> = if !justification.votes.is_empty() {
            justification.votes[0].block_number.into()
        } else {
            return Err("Justification has no votes".to_string());
        };

        info!(
            "DVF Vote Aggregator: Justification constructed for block #{} ({:?}) with {} votes, weight: {}",
            block_number, block_hash, justification.votes.len(), accumulated_weight
        );

        // Encode the justification for attachment to the block
        let encoded_justification = justification.encode();
        
        info!(
            "DVF Vote Aggregator: Encoded justification ({} bytes) for block #{} ({:?})",
            encoded_justification.len(), block_number, block_hash
        );
        
        // Finalize the block with the justification attached
        // This will:
        // 1. Mark the block as finalized in the client
        // 2. Attach the justification to the block
        // 3. Trigger finality notifications automatically
        let justification_data = Some((DVF_ENGINE_ID, encoded_justification));
        
        let result = self.client.lock_import_and_run(|import_op| {
            self.client.apply_finality(import_op, block_hash, justification_data, true)
        });
        
        match result {
            Ok(()) => {
                info!(
                    "DVF Vote Aggregator: Successfully finalized block #{} ({:?}) with justification",
                    block_number, block_hash
                );
            }
            Err(e) => {
                warn!(
                    "DVF Vote Aggregator: Failed to finalize block #{} ({:?}): {:?}",
                    block_number, block_hash, e
                );
                return Err(format!("Failed to finalize block: {:?}", e));
            }
        }

        Ok(())
    }

    /// Calculates accumulated weight for a specific block hash
    ///
    /// # Arguments
    /// * `round` - The round number
    /// * `block_hash` - The block hash to calculate weight for
    ///
    /// # Returns
    /// The total accumulated weight for the block
    fn calculate_accumulated_weight(
        &self,
        round: u32,
        block_hash: &Block::Hash,
    ) -> Result<u128, String> {
        // Get all votes for this block
        let votes = self.vote_pool.get_votes(round, block_hash);

        info!(
            "DVF Vote Aggregator: Found {} votes for block {:?} in round {}",
            votes.len(),
            block_hash,
            round
        );

        // Query validator weights from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let validator_weights = api
            .get_validator_weights(best_hash)
            .map_err(|e| format!("Failed to get validator weights: {:?}", e))?;

        // Create a map for quick weight lookup
        let weight_map: std::collections::HashMap<_, _> =
            validator_weights.into_iter().collect();

        // Calculate accumulated weight
        let mut accumulated_weight = 0u128;
        for vote in &votes {
            if let Some(weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(*weight);
                info!(
                    "DVF Vote Aggregator:   - Validator {:?} voted with weight {}",
                    vote.validator_account_id, weight
                );
            } else {
                warn!(
                    "DVF Vote Aggregator:   - Validator {:?} has no weight (not in validator set?)",
                    vote.validator_account_id
                );
            }
        }

        info!(
            "DVF Vote Aggregator: Total accumulated weight for block {:?}: {} (from {} votes)",
            block_hash, accumulated_weight, votes.len()
        );

        Ok(accumulated_weight)
    }
}

/// Vote Pool Pruning Service
///
/// This service subscribes to finality notifications and triggers vote pool pruning
/// after each block finalization. It runs as a separate background task to avoid
/// blocking the vote aggregator's main loop.
pub struct VotePoolPruningService<Block, Client, AccountId>
where
    Block: BlockT,
{
    client: Arc<Client>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    finality_notifier: Arc<FinalityNotifier<Block>>,
}

impl<Block, Client, AccountId> VotePoolPruningService<Block, Client, AccountId>
where
    Block: BlockT,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Creates a new Vote Pool Pruning Service
    ///
    /// # Arguments
    /// * `client` - The blockchain client for querying runtime state
    /// * `vote_pool` - The vote pool to prune
    /// * `finality_notifier` - The finality notifier to subscribe to
    pub fn new(
        client: Arc<Client>,
        vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
        finality_notifier: Arc<FinalityNotifier<Block>>,
    ) -> Self {
        info!("DVF Vote Pool Pruning Service: Initializing");
        Self {
            client,
            vote_pool,
            finality_notifier,
        }
    }

    /// Starts the service as a background task
    ///
    /// This method subscribes to finality notifications and triggers pruning
    /// after each block finalization. The task will run until the tokio runtime
    /// is shut down.
    pub async fn run(self) {
        info!("DVF Vote Pool Pruning Service: Starting service");

        // Subscribe to finality notifications
        let mut finality_receiver = self.finality_notifier.subscribe().await;

        loop {
            // Wait for finality notification
            match finality_receiver.recv().await {
                Some(notification) => {
                    debug!(
                        "DVF Vote Pool Pruning Service: Received finality notification for block #{}",
                        notification.block_number
                    );

                    // Trigger pruning
                    self.prune_after_finalization(
                        notification.block_number.saturated_into::<u32>(),
                        notification.round_id,
                    );
                }
                None => {
                    warn!("DVF Vote Pool Pruning Service: Finality notification channel closed");
                    break;
                }
            }
        }

        info!("DVF Vote Pool Pruning Service: Service stopped");
    }

    /// Prunes the vote pool after finalization
    ///
    /// This method is called after a block is finalized to remove old votes
    /// and prevent unbounded memory growth.
    ///
    /// # Arguments
    /// * `finalized_block_number` - The finalized block number
    /// * `current_round` - The current round number
    fn prune_after_finalization(&self, finalized_block_number: u32, current_round: u32) {
        debug!(
            "DVF Vote Pool Pruning Service: Pruning vote pool (finalized block: {}, current round: {})",
            finalized_block_number, current_round
        );

        let initial_size = self.vote_pool.size();
        let initial_votes = self.vote_pool.total_votes();

        // Get VoteRetentionRounds from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let retention_rounds = match api.get_vote_retention_rounds(best_hash) {
            Ok(rounds) => rounds,
            Err(e) => {
                warn!(
                    "DVF Vote Pool Pruning Service: Failed to get VoteRetentionRounds: {:?}. Using default of 20.",
                    e
                );
                20 // Default fallback
            }
        };

        // 1. Round-based pruning
        let rounds_removed = self.vote_pool.prune_by_round_age(current_round, retention_rounds);
        if rounds_removed > 0 {
            debug!(
                "DVF Vote Pool Pruning Service: Pruned {} vote entries by round age (retention: {} rounds)",
                rounds_removed, retention_rounds
            );
        }

        // 2. Finalized block pruning
        let finalized_removed = self.vote_pool.prune_by_finalized_block(finalized_block_number);
        if finalized_removed > 0 {
            debug!(
                "DVF Vote Pool Pruning Service: Pruned {} vote entries for finalized blocks",
                finalized_removed
            );
        }

        // 3. Max size enforcement (max 10,000 vote entries)
        const MAX_VOTE_POOL_SIZE: usize = 10_000;
        let size_removed = self.vote_pool.enforce_max_size(MAX_VOTE_POOL_SIZE);
        if size_removed > 0 {
            warn!(
                "DVF Vote Pool Pruning Service: Vote pool exceeded max size. Pruned {} oldest entries.",
                size_removed
            );
        }

        let final_size = self.vote_pool.size();
        let final_votes = self.vote_pool.total_votes();

        debug!(
            "DVF Vote Pool Pruning Service: Pruning complete. Entries: {} -> {} (removed {}), Total votes: {} -> {} (removed {})",
            initial_size, final_size, initial_size - final_size,
            initial_votes, final_votes, initial_votes - final_votes
        );
    }
}
