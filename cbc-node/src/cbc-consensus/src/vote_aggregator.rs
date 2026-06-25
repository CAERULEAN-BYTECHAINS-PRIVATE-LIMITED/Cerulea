//! Vote Aggregator Service
//!
//! This module implements the Vote Aggregator Service which continuously monitors
//! vote accumulation in the vote pool and triggers justification construction when
//! the finality threshold is reached.

use codec::{Encode, Decode};
use log::{debug, info, warn};
use sc_client_api::{HeaderBackend, backend::{Finalizer, LockImportRun}};
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::{Block as BlockT, Header, NumberFor, SaturatedConversion};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    dvf_gossip::DvfVotePool,
    finality::FinalityNotifier,
    DVF_ENGINE_ID,
    justification_builder::JustificationBuilder,
    metrics::DvfMetrics,
};
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi;

/// Vote Aggregator Service
///
/// Monitors vote accumulation in the vote pool and triggers justification construction
/// when the finality threshold is reached.
pub struct VoteAggregatorService<Block, Backend, Client, AccountId, TP>
where
    Block: BlockT,
    Backend: sc_client_api::backend::Backend<Block>,
    TP: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
{
    client: Arc<Client>,
    vote_pool: Arc<DvfVotePool<Block::Hash, AccountId>>,
    justification_builder: Arc<JustificationBuilder<Block, Client, AccountId>>,
    check_interval: Duration,
    last_finalized_round: std::sync::RwLock<u32>,
    stall_warning_threshold: u32,
    last_validator_set_id: std::sync::RwLock<u32>,
    last_triggered_block: std::sync::RwLock<u32>,
    metrics: Option<Arc<DvfMetrics>>,
    checkpoint_times: Arc<std::sync::RwLock<std::collections::HashMap<u32, std::time::Instant>>>,
    transaction_pool: Arc<TP>,
    _phantom: std::marker::PhantomData<Backend>,
}

impl<Block, Backend, Client, AccountId, TP> VoteAggregatorService<Block, Backend, Client, AccountId, TP>
where
    Block: BlockT,
    Backend: sc_client_api::backend::Backend<Block>,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Finalizer<Block, Backend> + LockImportRun<Block, Backend> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
    TP: sc_transaction_pool_api::TransactionPool<Block = Block> + 'static,
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
        transaction_pool: Arc<TP>,
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
            last_triggered_block: std::sync::RwLock::new(0),
            metrics: None,
            checkpoint_times: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
            transaction_pool,
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
        
        // Update last known validator set ID without clearing the pool.
        // The validator_set_id increments at every epoch transition (every 100 blocks),
        // but checkpoint blocks occur every 10 blocks. Clearing the pool on every epoch
        // transition wipes votes that were just cast for the current checkpoint block,
        // starving DVF finality. Round-based and block-based pruning handle cleanup.
        let mut last_id = self.last_validator_set_id.write().unwrap();
        if *last_id != 0 && *last_id != current_validator_set_id {
            info!(
                "DVF Vote Aggregator: Validator set ID changed from {} to {} (pool retained for in-flight votes).",
                *last_id, current_validator_set_id
            );
            // Do NOT clear the pool here — votes for the current checkpoint block
            // may have just been inserted and must survive until the aggregator acts.
        }
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

        // Check for finality stall (important to run even if pool is empty)
        self.check_finality_stall(current_round);

        // Get all candidate blocks from the vote pool
        let mut candidate_blocks = self.vote_pool.get_candidate_blocks(current_round);

        // Filter out blocks we have already triggered justification for locally
        let last_triggered = *self.last_triggered_block.read().unwrap();
        
        // Also query the actual pallet finalized head to avoid re-justifying what others already finalized
        let dvf_finalized = api
            .get_dvf_finalized_block(best_hash)
            .map_err(|e| format!("Failed to get DVF finalized block: {:?}", e))?
            .saturated_into::<u32>();

        candidate_blocks.retain(|hash| {
            if let Ok(Some(header)) = self.client.header(*hash) {
                let num = header.number().clone().saturated_into::<u32>();
                num > last_triggered && num > dvf_finalized
            } else {
                false
            }
        });

        // Fast-path: If there are no candidate blocks (or all are already triggered/finalized), 
        // skip the heavy runtime API queries and log spam.
        if candidate_blocks.is_empty() {
            return Ok(());
        }

        info!(
            "DVF Vote Aggregator: ===== Checking vote accumulation for round {} =====",
            current_round
        );

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
            // Check if this block is already DVF-finalized to avoid re-finalization attempts.
            // We use the DVF finalized block number (not Substrate client finalized) because
            // DCF progressive finality advances the Substrate finalized head before DVF acts,
            // which would cause the aggregator to skip blocks that DVF hasn't finalized yet.
            let block_number: NumberFor<Block> = self.client
                .header(block_hash)
                .map_err(|e| format!("Failed to get header: {:?}", e))?
                .ok_or_else(|| "Block header not found".to_string())?
                .number()
                .clone();

            let dvf_finalized: NumberFor<Block> = {
                let api = self.client.runtime_api();
                let bh = self.client.info().best_hash;
                api.get_dvf_finalized_block(bh).unwrap_or_else(|_| 0u32.into())
            };

            let block_num_u32 = block_number.clone().saturated_into::<u32>();
            let last_triggered = *self.last_triggered_block.read().unwrap();
            
            if block_num_u32 <= last_triggered {
                debug!(
                    "DVF Vote Aggregator: Block #{} already had justification triggered by this node recently, skipping",
                    block_number
                );
                return Ok(());
            }

            if block_number <= dvf_finalized {
                debug!(
                    "DVF Vote Aggregator: Block #{} ({:?}) is already DVF-finalized (dvf finalized: #{}), skipping",
                    block_number, block_hash, dvf_finalized
                );
                return Ok(());
            }
            
            // Record that we triggered it to prevent duplicates
            if let Ok(mut triggered) = self.last_triggered_block.write() {
                *triggered = block_num_u32.max(*triggered);
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

                // Update DVF pallet's FinalizedBlockNumber so get_dvf_finalized_block()
                // returns the correct value and DCF progressive finality is properly capped.
                // Convert local DvfJustification<Hash, AccountId> to the pallet's
                // DvfJustification<Hash, AccountId, MultiSignature>.
                let _ = self.client.runtime_api();
                let best_hash = self.client.info().best_hash;
                let pallet_votes: Vec<pallet_cbc_dvf::DvfVote<Block::Hash, AccountId, sp_runtime::MultiSignature>> =
                    justification.votes.iter().map(|v| pallet_cbc_dvf::DvfVote {
                        epoch_id: v.epoch_id,
                        validator_set_id: v.validator_set_id,
                        round_id: v.round_number,
                        block_number: v.block_number,
                        block_hash: v.block_hash.clone(),
                        validator_account: v.validator_account_id.clone(),
                        signature: sp_runtime::MultiSignature::Ed25519(v.signature),
                    }).collect();
                let pallet_justification = pallet_cbc_dvf::DvfJustification {
                    round_number: justification.round_number,
                    block_hash: justification.block_hash.clone(),
                    votes: pallet_votes,
                };
                // Convert generic justification to concrete runtime justification via SCALE codec
                let enc_justification = pallet_justification.encode();
                let concrete_justification: pallet_cbc_dvf::DvfJustification<
                    sp_core::H256,
                    cbc_runtime::AccountId,
                    sp_runtime::MultiSignature
                > = Decode::decode(&mut &enc_justification[..])
                    .expect("Failed to decode generic justification to concrete type");

                // Construct the unsigned extrinsic call
                let encoded_call = cbc_runtime::RuntimeCall::Dvf(
                    pallet_cbc_dvf::Call::submit_justification {
                        justification: concrete_justification,
                    }
                );
                
                let tx = cbc_runtime::UncheckedExtrinsic::new_bare(encoded_call);
                let encoded_tx = tx.encode();
                
                match <Block as BlockT>::Extrinsic::decode(&mut &encoded_tx[..]) {
                    Ok(ext) => {
                        let tp = self.transaction_pool.clone();
                        // Submit the transaction to the local pool
                        match tp.submit_one(best_hash, sc_transaction_pool_api::TransactionSource::Local, ext).await {
                            Ok(hash) => {
                                info!(
                                    "DVF Vote Aggregator: Submitted justification extrinsic for block #{}, tx hash: {:?}",
                                    block_number, hash
                                );
                            }
                            Err(e) => {
                                let err_str = format!("{:?}", e);
                                // Silence expected noise in a multi-validator environment
                                if err_str.contains("AlreadyImported") || err_str.contains("TemporarilyBanned") || err_str.contains("TooLowPriority") {
                                    debug!(
                                        "DVF Vote Aggregator: Justification for block #{} was already submitted by another node.",
                                        block_number
                                    );
                                } else {
                                    warn!(
                                        "DVF Vote Aggregator: Failed to submit justification extrinsic for block #{}: {}",
                                        block_number, err_str
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("DVF Vote Aggregator: Failed to decode Extrinsic for block #{}: {:?}", block_number, e);
                    }
                }
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
    /// Only executes when `finalized_block_number` is a checkpoint boundary
    /// (divisible by `FinalityCheckpointInterval`). Votes for the finalized
    /// checkpoint block itself are retained until the next pruning cycle so
    /// the aggregator can still act on them.
    ///
    /// # Arguments
    /// * `finalized_block_number` - The DVF-finalized block number
    /// * `current_round` - The current round number
    fn prune_after_finalization(&self, finalized_block_number: u32, current_round: u32) {
        // Get checkpoint interval from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let checkpoint_interval: u32 = match api.get_finality_checkpoint_interval(best_hash) {
            Ok(interval) => interval.saturated_into::<u32>(),
            Err(e) => {
                warn!(
                    "DVF Vote Pool Pruning Service: Failed to get FinalityCheckpointInterval: {:?}. Using default of 10.",
                    e
                );
                10
            }
        };

        // Guard: only prune at checkpoint boundaries
        if checkpoint_interval == 0 || finalized_block_number % checkpoint_interval != 0 {
            debug!(
                "DVF Vote Pool Pruning Service: Skipping pruning — block {} is not a checkpoint boundary (interval: {})",
                finalized_block_number, checkpoint_interval
            );
            return;
        }

        debug!(
            "DVF Vote Pool Pruning Service: Pruning vote pool at checkpoint boundary (finalized block: {}, current round: {})",
            finalized_block_number, current_round
        );

        let initial_size = self.vote_pool.size();
        let initial_votes = self.vote_pool.total_votes();

        let retention_rounds = match api.get_vote_retention_rounds(best_hash) {
            Ok(rounds) => rounds,
            Err(e) => {
                warn!(
                    "DVF Vote Pool Pruning Service: Failed to get VoteRetentionRounds: {:?}. Using default of 20.",
                    e
                );
                20
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

        // 2. Finalized block pruning — retain votes for the finalized checkpoint block
        //    itself (block_number >= finalized_block_number) so the aggregator can still
        //    act on them during this cycle. Only votes strictly below are removed.
        let finalized_removed = self.vote_pool.prune_by_finalized_block(finalized_block_number);
        if finalized_removed > 0 {
            debug!(
                "DVF Vote Pool Pruning Service: Pruned {} vote entries for blocks strictly below checkpoint {}",
                finalized_removed, finalized_block_number
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
