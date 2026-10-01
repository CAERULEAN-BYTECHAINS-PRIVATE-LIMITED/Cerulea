//! Vote Aggregator Service
//!
//! This module implements the Vote Aggregator Service which continuously monitors
//! vote accumulation in the vote pool and triggers justification construction when
//! the finality threshold is reached.

use codec::{Decode, Encode};
use log::{debug, info, warn};
use sc_client_api::{
    backend::{Finalizer, LockImportRun},
    HeaderBackend,
};
use sp_api::ProvideRuntimeApi;
use sp_runtime::traits::{Block as BlockT, Header, NumberFor, SaturatedConversion};
use std::sync::Arc;
use std::time::Duration;

use crate::{
    dvf_gossip::DvfVotePool, finality::FinalityNotifier,
    justification_builder::JustificationBuilder, metrics::DvfMetrics, CERULEA_COMMIT_ENGINE_ID,
};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use pallet_cerulea_dvf::DvfApi;

/// One vote in the form the runtime call takes.
type ChainVote = pallet_cerulea_dvf::DvfVote<
    sp_core::H256,
    cerulea_runtime::AccountId,
    sp_runtime::MultiSignature,
>;

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

impl<Block, Backend, Client, AccountId, TP>
    VoteAggregatorService<Block, Backend, Client, AccountId, TP>
where
    Block: BlockT,
    Backend: sc_client_api::backend::Backend<Block>,
    Client: ProvideRuntimeApi<Block>
        + HeaderBackend<Block>
        + Finalizer<Block, Backend>
        + LockImportRun<Block, Backend>
        + Send
        + Sync
        + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
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
            "Commit Engine Vote Aggregator: Initializing service with check interval {:?}",
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
        info!("Commit Engine Vote Aggregator: Starting service");

        // Create a cancellation token for graceful shutdown
        let mut interval = tokio::time::interval(self.check_interval);

        loop {
            // Wait for the next tick
            interval.tick().await;

            // Check for validator set changes and clear pool if needed
            if let Err(e) = self.check_validator_set_changes() {
                warn!(
                    "Commit Engine Vote Aggregator: Error checking validator set changes: {:?}",
                    e
                );
            }

            // Check vote accumulation
            if let Err(e) = self.check_vote_accumulation().await {
                warn!(
                    "Commit Engine Vote Aggregator: Error checking vote accumulation: {:?}",
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
        // starving Commit Engine finality. Round-based and block-based pruning handle cleanup.
        let mut last_id = self.last_validator_set_id.write().unwrap();
        if *last_id != 0 && *last_id != current_validator_set_id {
            info!(
                "Commit Engine Vote Aggregator: Validator set ID changed from {} to {} (pool retained for in-flight votes).",
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

        // Every conflicting pair of votes the pool caught goes to the chain.
        self.report_evidence(best_hash).await;

        // Get all candidate blocks from the vote pool
        let mut candidate_blocks = self.vote_pool.get_candidate_blocks(current_round);

        // Filter out blocks we have already triggered justification for locally
        let last_triggered = *self.last_triggered_block.read().unwrap();

        // Also query the actual pallet finalized head to avoid re-justifying what others already finalized
        let dvf_finalized = api
            .get_dvf_finalized_block(best_hash)
            .map_err(|e| format!("Failed to get Commit Engine finalized block: {:?}", e))?
            .saturated_into::<u32>();

        candidate_blocks.retain(|hash| {
            if let Ok(Some(header)) = self.client.header(*hash) {
                let num = (*header.number()).saturated_into::<u32>();
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
            "Commit Engine Vote Aggregator: ===== Checking vote accumulation for round {} =====",
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
            "Commit Engine Vote Aggregator: Total weight: {}, Finality threshold: {} ({:?} of total)",
            total_weight, finality_threshold, finality_threshold_perbill
        );

        info!(
            "Commit Engine Vote Aggregator: Found {} candidate blocks for round {}",
            candidate_blocks.len(),
            current_round
        );

        // Log vote pool statistics
        info!(
            "Commit Engine Vote Aggregator: Vote pool size: {} votes total",
            self.vote_pool.total_votes()
        );

        // Track the best candidate (highest weight that meets threshold)
        let mut best_candidate: Option<Block::Hash> = None;
        let mut best_weight: u128 = 0;

        // Check each candidate block
        for block_hash in candidate_blocks {
            let accumulated_weight =
                self.calculate_accumulated_weight(current_round, &block_hash)?;

            info!(
                "Commit Engine Vote Aggregator: Block {:?} has accumulated weight {} (threshold: {}, reached: {})",
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
            // Check if this block is already committed to avoid re-finalization attempts.
            // We use the Commit Engine finalized block number (not Substrate client finalized) because
            // DCF progressive finality advances the Substrate finalized head before Commit Engine acts,
            // which would cause the aggregator to skip blocks that Commit Engine hasn't finalized yet.
            let block_number: NumberFor<Block> = *self
                .client
                .header(block_hash)
                .map_err(|e| format!("Failed to get header: {:?}", e))?
                .ok_or_else(|| "Block header not found".to_string())?
                .number();

            let dvf_finalized: NumberFor<Block> = {
                let api = self.client.runtime_api();
                let bh = self.client.info().best_hash;
                api.get_dvf_finalized_block(bh)
                    .unwrap_or_else(|_| 0u32.into())
            };

            let block_num_u32 = block_number.saturated_into::<u32>();
            let last_triggered = *self.last_triggered_block.read().unwrap();

            if block_num_u32 <= last_triggered {
                debug!(
                    "Commit Engine Vote Aggregator: Block #{} already had justification triggered by this node recently, skipping",
                    block_number
                );
                return Ok(());
            }

            if block_number <= dvf_finalized {
                debug!(
                    "Commit Engine Vote Aggregator: Block #{} ({:?}) is already committed (commit finalized: #{}), skipping",
                    block_number, block_hash, dvf_finalized
                );
                return Ok(());
            }

            // Record that we triggered it to prevent duplicates
            if let Ok(mut triggered) = self.last_triggered_block.write() {
                *triggered = block_num_u32.max(*triggered);
            }

            info!(
                "Commit Engine Vote Aggregator: Block {:?} reached finality threshold! Weight: {}, Threshold: {}",
                block_hash, best_weight, finality_threshold
            );

            // Record checkpoint time for latency measurement
            let checkpoint_time = std::time::Instant::now();
            if let Ok(mut times) = self.checkpoint_times.write() {
                times.insert(current_round, checkpoint_time);
            }

            // Trigger justification construction with retry logic
            match self
                .trigger_justification_construction(current_round, block_hash, best_weight)
                .await
            {
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
                                "Commit Engine Vote Aggregator: Finality latency for round {}: {:.2}s",
                                current_round, latency
                            );
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Commit Engine Vote Aggregator: Failed to trigger justification construction: {:?}. Will retry on next check.",
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
                    "Commit Engine Vote Aggregator: Finality stalled! No blocks finalized for {} rounds (current: {}, last finalized: {})",
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
            "Commit Engine Vote Aggregator: Triggering justification construction for block {:?} (round: {}, weight: {})",
            block_hash, round, accumulated_weight
        );

        // Construct justification using the justification builder
        let justification = self
            .justification_builder
            .construct_justification(round, block_hash)
            .map_err(|e| format!("Failed to construct justification: {}", e))?;

        info!(
            "Commit Engine Vote Aggregator: Successfully constructed justification with {} votes",
            justification.votes.len()
        );

        // Get the block number from the justification
        let block_number: NumberFor<Block> = if !justification.votes.is_empty() {
            justification.votes[0].block_number.into()
        } else {
            return Err("Justification has no votes".to_string());
        };

        info!(
            "Commit Engine Vote Aggregator: Justification constructed for block #{} ({:?}) with {} votes, weight: {}",
            block_number, block_hash, justification.votes.len(), accumulated_weight
        );

        // Encode the justification for attachment to the block
        let encoded_justification = justification.encode();

        info!(
            "Commit Engine Vote Aggregator: Encoded justification ({} bytes) for block #{} ({:?})",
            encoded_justification.len(),
            block_number,
            block_hash
        );

        // Finalize the block with the justification attached
        // This will:
        // 1. Mark the block as finalized in the client
        // 2. Attach the justification to the block
        // 3. Trigger finality notifications automatically
        let justification_data = Some((CERULEA_COMMIT_ENGINE_ID, encoded_justification));

        let result = self.client.lock_import_and_run(|import_op| {
            self.client
                .apply_finality(import_op, block_hash, justification_data, true)
        });

        match result {
            Ok(()) => {
                info!(
                    "Commit Engine Vote Aggregator: Successfully finalized block #{} ({:?}) with justification",
                    block_number, block_hash
                );

                // Update Commit Engine pallet's FinalizedBlockNumber so get_dvf_finalized_block()
                // returns the correct value and DCF progressive finality is properly capped.
                // Convert local DvfJustification<Hash, AccountId> to the pallet's
                // DvfJustification<Hash, AccountId, MultiSignature>.
                let _ = self.client.runtime_api();
                let best_hash = self.client.info().best_hash;
                let pallet_votes: Vec<
                    pallet_cerulea_dvf::DvfVote<Block::Hash, AccountId, sp_runtime::MultiSignature>,
                > = justification
                    .votes
                    .iter()
                    .map(|v| pallet_cerulea_dvf::DvfVote {
                        epoch_id: v.epoch_id,
                        validator_set_id: v.validator_set_id,
                        round_id: v.round_number,
                        block_number: v.block_number,
                        block_hash: v.block_hash,
                        validator_account: v.validator_account_id.clone(),
                        signature: sp_runtime::MultiSignature::Ed25519(v.signature),
                    })
                    .collect();
                let pallet_justification = pallet_cerulea_dvf::DvfJustification {
                    round_number: justification.round_number,
                    block_hash: justification.block_hash,
                    votes: pallet_votes,
                };
                // Convert generic justification to concrete runtime justification via SCALE codec
                let enc_justification = pallet_justification.encode();
                let concrete_justification: pallet_cerulea_dvf::DvfJustification<
                    sp_core::H256,
                    cerulea_runtime::AccountId,
                    sp_runtime::MultiSignature,
                > = Decode::decode(&mut &enc_justification[..])
                    .expect("Failed to decode generic justification to concrete type");

                // Construct the unsigned extrinsic call
                // The runtime carries one call variant per pallet. The conversion
                // reaches that variant without naming it here.
                let encoded_call = cerulea_runtime::RuntimeCall::from(pallet_cerulea_dvf::Call::<
                    cerulea_runtime::Runtime,
                >::submit_justification {
                    justification: concrete_justification,
                });

                let tx = cerulea_runtime::UncheckedExtrinsic::new_bare(encoded_call);
                let encoded_tx = tx.encode();

                match <Block as BlockT>::Extrinsic::decode(&mut &encoded_tx[..]) {
                    Ok(ext) => {
                        let tp = self.transaction_pool.clone();
                        // Submit the transaction to the local pool
                        match tp
                            .submit_one(
                                best_hash,
                                sc_transaction_pool_api::TransactionSource::Local,
                                ext,
                            )
                            .await
                        {
                            Ok(hash) => {
                                info!(
                                    "Commit Engine Vote Aggregator: Submitted justification extrinsic for block #{}, tx hash: {:?}",
                                    block_number, hash
                                );
                            }
                            Err(e) => {
                                let err_str = format!("{:?}", e);
                                // Silence expected noise in a multi-validator environment
                                if err_str.contains("AlreadyImported")
                                    || err_str.contains("TemporarilyBanned")
                                    || err_str.contains("TooLowPriority")
                                {
                                    debug!(
                                        "Commit Engine Vote Aggregator: Justification for block #{} was already submitted by another node.",
                                        block_number
                                    );
                                } else {
                                    warn!(
                                        "Commit Engine Vote Aggregator: Failed to submit justification extrinsic for block #{}: {}",
                                        block_number, err_str
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Commit Engine Vote Aggregator: Failed to decode Extrinsic for block #{}: {:?}",
                            block_number, e
                        );
                    }
                }
            }
            Err(e) => {
                warn!(
                    "Commit Engine Vote Aggregator: Failed to finalize block #{} ({:?}): {:?}",
                    block_number, block_hash, e
                );
                return Err(format!("Failed to finalize block: {:?}", e));
            }
        }

        Ok(())
    }

    /// Report every conflicting pair of votes the pool caught.
    ///
    /// Two votes one validator signed in one round for two different blocks
    /// prove an equivocation. The report travels as an unsigned transaction,
    /// since the chain checks both signatures itself.
    async fn report_evidence(&self, best_hash: Block::Hash) {
        for (first, second) in self.vote_pool.take_evidence() {
            let to_chain = |vote: &crate::dvf_gossip::DvfVoteMessage<Block::Hash, AccountId>| {
                pallet_cerulea_dvf::DvfVote {
                    epoch_id: vote.epoch_id,
                    validator_set_id: vote.validator_set_id,
                    round_id: vote.round_number,
                    block_number: vote.block_number,
                    block_hash: vote.block_hash,
                    validator_account: vote.validator_account_id.clone(),
                    signature: sp_runtime::MultiSignature::Ed25519(vote.signature),
                }
            };
            let pair = (to_chain(&first), to_chain(&second));
            let encoded = pair.encode();
            let decoded: Result<(ChainVote, ChainVote), _> = Decode::decode(&mut &encoded[..]);
            let Ok((first_vote, second_vote)) = decoded else {
                warn!("Commit Engine Vote Aggregator: an evidence pair failed to encode for the chain");
                continue;
            };
            warn!(
                "Commit Engine Vote Aggregator: validator {:?} signed two votes in round {}. Reporting the pair to the chain.",
                first.validator_account_id, first.round_number
            );
            let call = cerulea_runtime::RuntimeCall::from(pallet_cerulea_dvf::Call::<
                cerulea_runtime::Runtime,
            >::report_vote_equivocation {
                first: first_vote,
                second: second_vote,
            });
            let encoded_tx = cerulea_runtime::UncheckedExtrinsic::new_bare(call).encode();
            match <Block as BlockT>::Extrinsic::decode(&mut &encoded_tx[..]) {
                Ok(ext) => {
                    if let Err(e) = self
                        .transaction_pool
                        .submit_one(
                            best_hash,
                            sc_transaction_pool_api::TransactionSource::Local,
                            ext,
                        )
                        .await
                    {
                        debug!(
                            "Commit Engine Vote Aggregator: the evidence report was not taken: {:?}",
                            e
                        );
                    }
                }
                Err(e) => warn!(
                    "Commit Engine Vote Aggregator: the evidence report failed to decode: {:?}",
                    e
                ),
            }
        }
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
            "Commit Engine Vote Aggregator: Found {} votes for block {:?} in round {}",
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
        let weight_map: std::collections::HashMap<_, _> = validator_weights.into_iter().collect();

        // Calculate accumulated weight
        let mut accumulated_weight = 0u128;
        for vote in &votes {
            if let Some(weight) = weight_map.get(&vote.validator_account_id) {
                accumulated_weight = accumulated_weight.saturating_add(*weight);
                info!(
                    "Commit Engine Vote Aggregator:   - Validator {:?} voted with weight {}",
                    vote.validator_account_id, weight
                );
            } else {
                warn!(
                    "Commit Engine Vote Aggregator:   - Validator {:?} has no weight (not in validator set?)",
                    vote.validator_account_id
                );
            }
        }

        info!(
            "Commit Engine Vote Aggregator: Total accumulated weight for block {:?}: {} (from {} votes)",
            block_hash,
            accumulated_weight,
            votes.len()
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
    AccountId: Clone
        + Encode
        + codec::Codec
        + PartialEq
        + Eq
        + std::fmt::Debug
        + Send
        + Sync
        + std::hash::Hash
        + 'static,
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
        info!("Commit Engine Vote Pool Pruning Service: Initializing");
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
        info!("Commit Engine Vote Pool Pruning Service: Starting service");

        // Subscribe to finality notifications
        let mut finality_receiver = self.finality_notifier.subscribe().await;

        loop {
            // Wait for finality notification
            match finality_receiver.recv().await {
                Some(notification) => {
                    debug!(
                        "Commit Engine Vote Pool Pruning Service: Received finality notification for block #{}",
                        notification.block_number
                    );

                    // Trigger pruning
                    self.prune_after_finalization(
                        notification.block_number.saturated_into::<u32>(),
                        notification.round_id,
                    );
                }
                None => {
                    warn!("Commit Engine Vote Pool Pruning Service: Finality notification channel closed");
                    break;
                }
            }
        }

        info!("Commit Engine Vote Pool Pruning Service: Service stopped");
    }

    /// Prunes the vote pool after finalization
    ///
    /// Only executes when `finalized_block_number` is a checkpoint boundary
    /// (divisible by `FinalityCheckpointInterval`). Votes for the finalized
    /// checkpoint block itself are retained until the next pruning cycle so
    /// the aggregator can still act on them.
    ///
    /// # Arguments
    /// * `finalized_block_number` - The committed block number
    /// * `current_round` - The current round number
    fn prune_after_finalization(&self, finalized_block_number: u32, current_round: u32) {
        // Get checkpoint interval from runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let checkpoint_interval: u32 = match api.get_finality_checkpoint_interval(best_hash) {
            Ok(interval) => interval.saturated_into::<u32>(),
            Err(e) => {
                warn!(
                    "Commit Engine Vote Pool Pruning Service: Failed to get FinalityCheckpointInterval: {:?}. Using default of 10.",
                    e
                );
                10
            }
        };

        // Guard: only prune at checkpoint boundaries
        if checkpoint_interval == 0 || finalized_block_number % checkpoint_interval != 0 {
            debug!(
                "Commit Engine Vote Pool Pruning Service: Skipping pruning — block {} is not a checkpoint boundary (interval: {})",
                finalized_block_number, checkpoint_interval
            );
            return;
        }

        debug!(
            "Commit Engine Vote Pool Pruning Service: Pruning vote pool at checkpoint boundary (finalized block: {}, current round: {})",
            finalized_block_number, current_round
        );

        let initial_size = self.vote_pool.size();
        let initial_votes = self.vote_pool.total_votes();

        let retention_rounds = match api.get_vote_retention_rounds(best_hash) {
            Ok(rounds) => rounds,
            Err(e) => {
                warn!(
                    "Commit Engine Vote Pool Pruning Service: Failed to get VoteRetentionRounds: {:?}. Using default of 20.",
                    e
                );
                20
            }
        };

        // 1. Round-based pruning
        let rounds_removed = self
            .vote_pool
            .prune_by_round_age(current_round, retention_rounds);
        if rounds_removed > 0 {
            debug!(
                "Commit Engine Vote Pool Pruning Service: Pruned {} vote entries by round age (retention: {} rounds)",
                rounds_removed, retention_rounds
            );
        }

        // 2. Finalized block pruning — retain votes for the finalized checkpoint block
        //    itself (block_number >= finalized_block_number) so the aggregator can still
        //    act on them during this cycle. Only votes strictly below are removed.
        let finalized_removed = self
            .vote_pool
            .prune_by_finalized_block(finalized_block_number);
        if finalized_removed > 0 {
            debug!(
                "Commit Engine Vote Pool Pruning Service: Pruned {} vote entries for blocks strictly below checkpoint {}",
                finalized_removed, finalized_block_number
            );
        }

        // 3. Max size enforcement (max 10,000 vote entries)
        const MAX_VOTE_POOL_SIZE: usize = 10_000;
        let size_removed = self.vote_pool.enforce_max_size(MAX_VOTE_POOL_SIZE);
        if size_removed > 0 {
            warn!(
                "Commit Engine Vote Pool Pruning Service: Vote pool exceeded max size. Pruned {} oldest entries.",
                size_removed
            );
        }

        let final_size = self.vote_pool.size();
        let final_votes = self.vote_pool.total_votes();

        debug!(
            "Commit Engine Vote Pool Pruning Service: Pruning complete. Entries: {} -> {} (removed {}), Total votes: {} -> {} (removed {})",
            initial_size, final_size, initial_size - final_size,
            initial_votes, final_votes, initial_votes - final_votes
        );
    }
}

#[cfg(test)]
mod coverage_tests {
    //! The aggregator and the pruning service, run against the test chain.
    //!
    //! The test chain keeps real headers in the in-memory backend. The
    //! aggregator writes its justification there and hands its extrinsics to
    //! a local pool that keeps them, so each test reads back what the service
    //! did.

    use super::*;
    use crate::dvf_gossip::{DvfJustification, DvfVoteMessage};
    use crate::finality::FinalityNotification;
    use crate::test_support::{
        capture_logs, AccountId, Block, InMemBackend, PoolError, SubmissionLog, TestChain, Voter,
    };
    use sp_core::H256;
    use sp_runtime::{MultiSignature, Perbill};

    type Pool = DvfVotePool<H256, AccountId>;
    type Builder = JustificationBuilder<Block, TestChain, AccountId>;
    type Aggregator =
        VoteAggregatorService<Block, InMemBackend, TestChain, AccountId, SubmissionLog>;
    type Pruner = VotePoolPruningService<Block, TestChain, AccountId>;

    /// One chain, one vote pool, one local transaction pool and three
    /// validators holding five, three and two tenths of the weight.
    struct Scene {
        chain: Arc<TestChain>,
        pool: Arc<Pool>,
        submissions: Arc<SubmissionLog>,
        metrics: Arc<DvfMetrics>,
        voters: [Voter; 3],
    }

    fn scene() -> Scene {
        let chain = TestChain::with_blocks(12);
        let voters = [
            Voter::from_seed(1),
            Voter::from_seed(2),
            Voter::from_seed(3),
        ];
        let answers = &chain.answers;
        answers.set("DvfApi_get_validator_set_id", (), 1u32);
        answers.set("DvfApi_get_current_round", (), 5u32);
        answers.set("DvfApi_get_dvf_finalized_block", (), 0u32);
        answers.set(
            "DvfApi_get_finality_threshold_perbill",
            (),
            Perbill::from_percent(67),
        );
        answers.set(
            "DvfApi_get_validator_weights",
            (),
            vec![
                (voters[0].account.clone(), 500u128),
                (voters[1].account.clone(), 300u128),
                (voters[2].account.clone(), 200u128),
            ],
        );
        answers.set(
            "DvfApi_get_validator_set",
            (),
            voters
                .iter()
                .map(|voter| voter.account.clone())
                .collect::<Vec<_>>(),
        );
        let registry = prometheus::Registry::new();
        let metrics = Arc::new(DvfMetrics::new(&registry).expect("the metrics register"));
        Scene {
            chain,
            pool: Pool::new(),
            submissions: Arc::new(SubmissionLog::default()),
            metrics,
            voters,
        }
    }

    fn aggregator(scene: &Scene) -> Aggregator {
        let builder = Arc::new(Builder::new(scene.chain.clone(), scene.pool.clone()));
        Aggregator::new(
            scene.chain.clone(),
            scene.pool.clone(),
            builder,
            scene.submissions.clone(),
            Duration::from_millis(5),
        )
        .with_metrics(scene.metrics.clone())
    }

    fn chain_vote(vote: &DvfVoteMessage<H256, AccountId>) -> ChainVote {
        pallet_cerulea_dvf::DvfVote {
            epoch_id: vote.epoch_id,
            validator_set_id: vote.validator_set_id,
            round_id: vote.round_number,
            block_number: vote.block_number,
            block_hash: vote.block_hash,
            validator_account: vote.validator_account_id.clone(),
            signature: MultiSignature::Ed25519(vote.signature),
        }
    }

    /// The bytes of the unsigned extrinsic that submits one justification.
    fn submission_bytes(justification: &DvfJustification<H256, AccountId>) -> Vec<u8> {
        let chain_justification = pallet_cerulea_dvf::DvfJustification {
            round_number: justification.round_number,
            block_hash: justification.block_hash,
            votes: justification.votes.iter().map(chain_vote).collect(),
        };
        let call = cerulea_runtime::RuntimeCall::from(pallet_cerulea_dvf::Call::<
            cerulea_runtime::Runtime,
        >::submit_justification {
            justification: chain_justification,
        });
        cerulea_runtime::UncheckedExtrinsic::new_bare(call).encode()
    }

    fn weight_label(hash: &H256) -> String {
        format!("{:?}", hash)
    }

    #[tokio::test]
    async fn a_checkpoint_past_the_threshold_commits_with_its_justification() {
        let scene = scene();
        let target = scene.chain.hash_at(10);
        assert!(scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target)));
        assert!(scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target)));
        let service = aggregator(&scene);

        service
            .check_vote_accumulation()
            .await
            .expect("the pass completes");

        // The backend marks the block final and keeps the justification.
        assert_eq!(scene.chain.info().finalized_hash, target);
        let stored = scene
            .chain
            .commit_justification(target)
            .expect("the block carries the Commit Engine justification");
        let justification = DvfJustification::<H256, AccountId>::decode(&mut &stored[..])
            .expect("the stored justification decodes");
        assert_eq!(justification.round_number, 5);
        assert_eq!(justification.block_hash, target);
        let signers: Vec<AccountId> = justification
            .votes
            .iter()
            .map(|vote| vote.validator_account_id.clone())
            .collect();
        assert_eq!(
            signers,
            vec![
                scene.voters[0].account.clone(),
                scene.voters[1].account.clone()
            ]
        );

        // The chain hears of the commitment through one unsigned extrinsic.
        let taken = scene.submissions.taken();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].encode(), submission_bytes(&justification));

        assert_eq!(*service.last_triggered_block.read().unwrap(), 10);
        assert_eq!(*service.last_finalized_round.read().unwrap(), 5);
        assert!(service.checkpoint_times.read().unwrap().contains_key(&5));
        assert_eq!(
            scene
                .metrics
                .dvf_accumulated_weight
                .with_label_values(&[&weight_label(&target)])
                .get(),
            800.0
        );
        assert_eq!(
            scene
                .metrics
                .dvf_finality_latency_seconds
                .get_sample_count(),
            1
        );

        // A second pass finds the block handled and submits nothing more.
        service.check_vote_accumulation().await.unwrap();
        assert_eq!(scene.submissions.taken().len(), 1);
    }

    #[tokio::test]
    async fn a_checkpoint_below_the_threshold_stays_open() {
        let scene = scene();
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[2].vote(1, 1, 5, 10, target));
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        assert_eq!(scene.chain.info().finalized_hash, scene.chain.hash_at(0));
        assert!(scene.chain.commit_justification(target).is_none());
        assert!(scene.submissions.taken().is_empty());
        assert_eq!(*service.last_triggered_block.read().unwrap(), 0);
        assert_eq!(
            scene
                .metrics
                .dvf_accumulated_weight
                .with_label_values(&[&weight_label(&target)])
                .get(),
            200.0
        );
        assert!(logs.has("has accumulated weight 200 (threshold: 670, reached: false)"));
    }

    #[tokio::test]
    async fn the_heaviest_candidate_past_the_threshold_wins() {
        let scene = scene();
        scene.chain.answers.set(
            "DvfApi_get_finality_threshold_perbill",
            (),
            Perbill::from_percent(20),
        );
        let ten = scene.chain.hash_at(10);
        let eleven = scene.chain.hash_at(11);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, ten));
        scene
            .pool
            .insert_vote(scene.voters[2].vote(1, 1, 5, 11, eleven));
        let service = aggregator(&scene);

        service.check_vote_accumulation().await.unwrap();

        assert_eq!(scene.chain.info().finalized_hash, ten);
        assert!(scene.chain.commit_justification(eleven).is_none());
        assert_eq!(*service.last_triggered_block.read().unwrap(), 10);
    }

    #[tokio::test]
    async fn a_vote_for_an_unknown_block_costs_no_runtime_query() {
        let scene = scene();
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, H256::repeat_byte(0x77)));
        // The threshold query fails, and the pass never reaches it.
        scene
            .chain
            .answers
            .forget("DvfApi_get_finality_threshold_perbill");
        let service = aggregator(&scene);

        assert_eq!(service.check_vote_accumulation().await, Ok(()));
        assert!(scene.submissions.taken().is_empty());
    }

    #[tokio::test]
    async fn a_vote_at_or_below_the_committed_height_is_left_alone() {
        let scene = scene();
        scene
            .chain
            .answers
            .set("DvfApi_get_dvf_finalized_block", (), 10u32);
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target));
        let service = aggregator(&scene);

        service.check_vote_accumulation().await.unwrap();

        assert!(scene.chain.commit_justification(target).is_none());
        assert!(scene.submissions.taken().is_empty());
    }

    #[tokio::test]
    async fn every_missing_runtime_answer_names_its_query() {
        let cases = [
            ("DvfApi_get_current_round", "Failed to get current round: "),
            (
                "DvfApi_get_dvf_finalized_block",
                "Failed to get Commit Engine finalized block: ",
            ),
            (
                "DvfApi_get_finality_threshold_perbill",
                "Failed to get finality threshold: ",
            ),
            (
                "DvfApi_get_validator_weights",
                "Failed to get validator weights: ",
            ),
        ];
        for (call, opening) in cases {
            let scene = scene();
            let target = scene.chain.hash_at(10);
            scene
                .pool
                .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
            scene.chain.answers.forget(call);
            let reason = aggregator(&scene)
                .check_vote_accumulation()
                .await
                .expect_err("the pass stops at the missing answer");
            assert!(reason.starts_with(opening), "{call}: {reason}");
            assert!(reason.contains(call), "{call}: {reason}");
        }
    }

    #[tokio::test]
    async fn a_failed_construction_leaves_the_block_open_for_the_next_pass() {
        let scene = scene();
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target));
        // The size check reads the validator set, which the runtime withholds.
        scene.chain.answers.forget("DvfApi_get_validator_set");
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        assert!(scene.chain.commit_justification(target).is_none());
        assert!(scene.submissions.taken().is_empty());
        assert_eq!(*service.last_finalized_round.read().unwrap(), 0);
        assert!(logs.has("Failed to trigger justification construction"));
        assert!(logs.has("Failed to get validator set"));
    }

    #[tokio::test]
    async fn a_submission_another_node_made_first_raises_no_warning() {
        let scene = scene();
        scene
            .submissions
            .refuse_with(|| PoolError::AlreadyImported(Box::new(())));
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target));
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        // The block commits locally whatever the pool answers.
        assert_eq!(scene.chain.info().finalized_hash, target);
        assert!(scene.submissions.taken().is_empty());
        assert!(logs.has("was already submitted by another node"));
        assert!(logs.lines_at(log::Level::Warn).is_empty());
    }

    #[tokio::test]
    async fn a_refused_submission_raises_a_warning() {
        let scene = scene();
        scene
            .submissions
            .refuse_with(|| PoolError::InvalidBlockId("the pool lags".to_string()));
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target));
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        assert_eq!(scene.chain.info().finalized_hash, target);
        let warnings = logs.lines_at(log::Level::Warn);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0]
            .contains("Failed to submit justification extrinsic for block #10: InvalidBlockId"));
    }

    #[tokio::test]
    async fn two_votes_from_one_validator_in_one_round_reach_the_chain_as_evidence() {
        let scene = scene();
        let ten = scene.chain.hash_at(10);
        let eleven = scene.chain.hash_at(11);
        let first = scene.voters[0].vote(1, 1, 5, 10, ten);
        let second = scene.voters[0].vote(1, 1, 5, 11, eleven);
        scene.pool.insert_vote(first.clone());
        assert_eq!(
            scene.pool.offer_vote(second.clone()),
            crate::dvf_gossip::VoteOffer::Conflicting
        );
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        let call = cerulea_runtime::RuntimeCall::from(pallet_cerulea_dvf::Call::<
            cerulea_runtime::Runtime,
        >::report_vote_equivocation {
            first: chain_vote(&first),
            second: chain_vote(&second),
        });
        let expected = cerulea_runtime::UncheckedExtrinsic::new_bare(call).encode();
        let taken = scene.submissions.taken();
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].encode(), expected);
        assert!(logs.has("signed two votes in round 5. Reporting the pair to the chain."));
        // The equivocator counts for nothing, so the checkpoint stays open.
        assert!(scene.chain.commit_justification(ten).is_none());
        assert!(scene.pool.take_evidence().is_empty());
    }

    #[tokio::test]
    async fn an_evidence_report_the_pool_refuses_is_dropped_quietly() {
        let scene = scene();
        scene
            .submissions
            .refuse_with(|| PoolError::TemporarilyBanned);
        let ten = scene.chain.hash_at(10);
        let eleven = scene.chain.hash_at(11);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, ten));
        scene
            .pool
            .offer_vote(scene.voters[0].vote(1, 1, 5, 11, eleven));
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_vote_accumulation().await.unwrap();

        assert!(scene.submissions.taken().is_empty());
        assert!(logs.has("the evidence report was not taken: TemporarilyBanned"));
    }

    #[test]
    fn a_change_of_validator_set_keeps_the_pool() {
        let scene = scene();
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_validator_set_changes().unwrap();
        assert_eq!(*service.last_validator_set_id.read().unwrap(), 1);
        assert!(!logs.has("Validator set ID changed"));

        scene
            .chain
            .answers
            .set("DvfApi_get_validator_set_id", (), 2u32);
        service.check_validator_set_changes().unwrap();
        assert_eq!(*service.last_validator_set_id.read().unwrap(), 2);
        assert!(
            logs.has("Validator set ID changed from 1 to 2 (pool retained for in-flight votes).")
        );
        assert_eq!(scene.pool.total_votes(), 1);

        scene.chain.answers.forget("DvfApi_get_validator_set_id");
        let reason = service
            .check_validator_set_changes()
            .expect_err("the set identifier is unavailable");
        assert!(reason.starts_with("Failed to get validator set ID: "));
    }

    #[test]
    fn ten_rounds_without_a_commitment_raise_the_stall_warning() {
        let scene = scene();
        let service = aggregator(&scene);
        let logs = capture_logs();

        service.check_finality_stall(9);
        assert!(!logs.has("Finality stalled"));

        service.check_finality_stall(15);
        assert!(logs.has(
            "Finality stalled! No blocks finalized for 15 rounds (current: 15, last finalized: 0)"
        ));
    }

    #[tokio::test]
    async fn the_running_service_commits_a_checkpoint_on_its_own() {
        let scene = scene();
        let target = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 5, 10, target));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 5, 10, target));
        // Each tick also checks the set identifier, which the runtime withholds.
        scene.chain.answers.forget("DvfApi_get_validator_set_id");
        let service = aggregator(&scene);
        let logs = capture_logs();

        let stopped = tokio::time::timeout(Duration::from_millis(200), service.run()).await;

        assert!(stopped.is_err(), "the service runs until the runtime stops");
        assert_eq!(scene.chain.info().finalized_hash, target);
        assert_eq!(scene.submissions.taken().len(), 1);
        assert!(logs.has("Error checking validator set changes"));
    }

    #[tokio::test]
    async fn the_running_service_reports_a_failed_pass_and_keeps_going() {
        let scene = scene();
        scene.chain.answers.forget("DvfApi_get_current_round");
        let service = aggregator(&scene);
        let logs = capture_logs();

        let stopped = tokio::time::timeout(Duration::from_millis(60), service.run()).await;

        assert!(stopped.is_err());
        assert!(logs.count("Error checking vote accumulation") >= 2);
    }

    fn pruning_scene() -> (Scene, Pruner, Arc<FinalityNotifier<Block>>) {
        let scene = scene();
        scene
            .chain
            .answers
            .set("DvfApi_get_finality_checkpoint_interval", (), 10u32);
        scene
            .chain
            .answers
            .set("DvfApi_get_vote_retention_rounds", (), 2u32);
        let notifier = Arc::new(FinalityNotifier::new());
        let pruner = Pruner::new(scene.chain.clone(), scene.pool.clone(), notifier.clone());
        (scene, pruner, notifier)
    }

    /// Three entries: an old round, a height below the checkpoint, and the
    /// checkpoint itself.
    fn fill_for_pruning(scene: &Scene) {
        let ten = scene.chain.hash_at(10);
        scene
            .pool
            .insert_vote(scene.voters[0].vote(1, 1, 3, 10, ten));
        scene
            .pool
            .insert_vote(scene.voters[1].vote(1, 1, 9, 10, ten));
        scene
            .pool
            .insert_vote(scene.voters[2].vote(1, 1, 9, 20, H256::repeat_byte(0x20)));
        assert_eq!(scene.pool.size(), 3);
    }

    #[test]
    fn pruning_waits_for_a_checkpoint_boundary() {
        let (scene, pruner, _notifier) = pruning_scene();
        fill_for_pruning(&scene);
        let logs = capture_logs();

        pruner.prune_after_finalization(15, 10);

        assert_eq!(scene.pool.size(), 3);
        assert!(logs.has("block 15 is not a checkpoint boundary (interval: 10)"));
    }

    #[test]
    fn pruning_at_a_checkpoint_keeps_the_checkpoint_votes() {
        let (scene, pruner, _notifier) = pruning_scene();
        fill_for_pruning(&scene);

        pruner.prune_after_finalization(20, 10);

        assert_eq!(scene.pool.size(), 1);
        assert_eq!(scene.pool.total_votes(), 1);
        assert_eq!(
            scene
                .pool
                .get_votes(9, &H256::repeat_byte(0x20))
                .first()
                .map(|vote| vote.validator_account_id.clone()),
            Some(scene.voters[2].account.clone())
        );
        // The one vote per round record outlives the height pruning.
        assert!(scene.pool.vote_of(9, &scene.voters[1].account).is_some());
        assert!(scene.pool.vote_of(3, &scene.voters[0].account).is_none());
    }

    #[test]
    fn pruning_falls_back_to_the_starting_profile_when_the_runtime_is_silent() {
        let (scene, pruner, _notifier) = pruning_scene();
        scene
            .chain
            .answers
            .forget("DvfApi_get_finality_checkpoint_interval");
        scene
            .chain
            .answers
            .forget("DvfApi_get_vote_retention_rounds");
        fill_for_pruning(&scene);
        let logs = capture_logs();

        pruner.prune_after_finalization(20, 25);

        assert!(logs.has("Failed to get FinalityCheckpointInterval"));
        assert!(logs.has("Using default of 10."));
        assert!(logs.has("Failed to get VoteRetentionRounds"));
        assert!(logs.has("Using default of 20."));
        // A retention of twenty rounds from round 25 drops round three and
        // keeps round nine. The height rule removes the entry below the
        // checkpoint. A retention of two rounds would have dropped all three.
        assert_eq!(scene.pool.size(), 1);
        assert_eq!(scene.pool.get_votes(9, &H256::repeat_byte(0x20)).len(), 1);
    }

    #[test]
    fn an_interval_of_zero_prunes_nothing() {
        let (scene, pruner, _notifier) = pruning_scene();
        scene
            .chain
            .answers
            .set("DvfApi_get_finality_checkpoint_interval", (), 0u32);
        fill_for_pruning(&scene);

        pruner.prune_after_finalization(20, 10);

        assert_eq!(scene.pool.size(), 3);
    }

    #[test]
    fn a_pool_above_ten_thousand_entries_drops_its_oldest_rounds() {
        let (scene, pruner, _notifier) = pruning_scene();
        scene
            .chain
            .answers
            .set("DvfApi_get_vote_retention_rounds", (), 100_000u32);
        for index in 0..10_005u32 {
            let mut block = [0u8; 32];
            block[..4].copy_from_slice(&index.to_le_bytes());
            scene.pool.insert_vote(DvfVoteMessage {
                epoch_id: 1,
                validator_set_id: 1,
                round_number: index,
                block_number: 30,
                block_hash: H256::from(block),
                validator_account_id: scene.voters[0].account.clone(),
                validator_public_key: scene.voters[0].public(),
                signature: sp_core::ed25519::Signature::from_raw([0u8; 64]),
            });
        }
        let logs = capture_logs();

        pruner.prune_after_finalization(20, 10_004);

        assert_eq!(scene.pool.size(), 10_000);
        assert!(scene.pool.get_votes(0, &H256::zero()).is_empty());
        assert!(logs.has("Vote pool exceeded max size. Pruned 5 oldest entries."));
    }

    #[tokio::test]
    async fn the_running_pruner_acts_on_each_commitment_message() {
        let (scene, pruner, notifier) = pruning_scene();
        fill_for_pruning(&scene);
        let target = scene.chain.hash_at(10);
        let driver = async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            notifier
                .notify(FinalityNotification {
                    block_number: 20,
                    block_hash: target,
                    round_id: 10,
                })
                .await;
            tokio::time::sleep(Duration::from_millis(20)).await;
        };

        let (stopped, ()) = tokio::join!(
            tokio::time::timeout(Duration::from_millis(120), pruner.run()),
            driver
        );

        assert!(
            stopped.is_err(),
            "the pruner listens until the runtime stops"
        );
        assert_eq!(scene.pool.size(), 1);
    }
}
