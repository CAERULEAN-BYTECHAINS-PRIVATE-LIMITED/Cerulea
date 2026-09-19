//! DCF (Dynamic Consensus Framework) implementation
//!
//! This module provides the core consensus engine implementation for the Cerulea chain.
//! It handles block production, validation, and author selection.

use crate::{
    error::{ConsensusError, ConsensusResult},
    metrics::ConsensusMetrics,
    proposer_factory::ProposerFactory,
    types::{ConsensusParams, ValidatorMetrics},
};
use cerulea_runtime::AccountId;
use codec::Encode;
use log::{debug, error, info, warn};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use sc_consensus::{BlockImport, BlockImportParams, ImportResult};
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::ed25519::Public;
use sp_core::Pair;
use sp_runtime::traits::{Block as BlockTrait, Header as HeaderT, NumberFor, SaturatedConversion};
use std::{sync::Arc, time::Duration};
use tokio::time::sleep;
// Import lifecycle tracer
use crate::lifecycle_tracer::{LifecycleTracer, TraceMetadata};

/// DCF consensus engine implementation
pub struct DcfConsensus<B, C, P, TP>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + sp_block_builder::BlockBuilder<B>,
    P: Pair,
    TP: TransactionPool<Block = B> + 'static,
{
    client: Arc<C>,
    proposer_factory: ProposerFactory<B, C, TP>,
    block_import: Arc<dyn BlockImport<B, Error = sp_consensus::Error> + Send + Sync>,
    params: ConsensusParams,
    metrics: ValidatorMetrics,
    consensus_metrics: Option<ConsensusMetrics>,
    last_block_time: Duration,
    current_slot: u64,
    last_metrics_update_slot: u64,
    last_score_refresh_slot: u64,
    _phantom: std::marker::PhantomData<(B, P, TP)>,
}

impl<B, C, P, TP> DcfConsensus<B, C, P, TP>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + sp_block_builder::BlockBuilder<B>,
    P: Pair,
    TP: TransactionPool<Block = B> + 'static,
{
    /// Create a new DCF consensus engine instance
    pub fn new(
        client: Arc<C>,
        transaction_pool: Arc<TP>,
        block_import: Arc<dyn BlockImport<B, Error = sp_consensus::Error> + Send + Sync>,
        params: ConsensusParams,
    ) -> Self {
        // Initialize last_block_time to 0 so the first block can be produced immediately
        let last_block_time = Duration::from_secs(0);

        let proposer_factory = ProposerFactory::new(
            client.clone(),
            transaction_pool.clone(),
            Duration::from_millis(params.min_block_time),
            params.max_transactions_per_block as usize,
        );

        Self {
            client,
            proposer_factory,
            block_import,
            params,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: None,
            last_block_time,
            current_slot: 0,
            last_metrics_update_slot: 0,
            last_score_refresh_slot: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new DCF consensus engine instance with metrics
    pub fn new_with_metrics(
        client: Arc<C>,
        transaction_pool: Arc<TP>,
        block_import: Arc<dyn BlockImport<B, Error = sp_consensus::Error> + Send + Sync>,
        params: ConsensusParams,
        consensus_metrics: ConsensusMetrics,
    ) -> Self {
        // Initialize last_block_time to 0 so the first block can be produced immediately
        let last_block_time = Duration::from_secs(0);

        let proposer_factory = ProposerFactory::new(
            client.clone(),
            transaction_pool.clone(),
            Duration::from_millis(params.min_block_time),
            params.max_transactions_per_block as usize,
        );

        Self {
            client,
            proposer_factory,
            block_import,
            params,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: Some(consensus_metrics),
            last_block_time,
            current_slot: 0,
            last_metrics_update_slot: 0,
            last_score_refresh_slot: 0,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Start the DCF consensus engine
    pub async fn run(&mut self) {
        loop {
            // Check if we should produce a block
            if self.should_produce_block() {
                // Get current runtime state
                let best_hash = self.client.info().best_hash;
                let _best_number = self.client.info().best_number;
                let block_number = self.client.info().best_number.saturated_into::<u32>() + 1;

                // STEP 40: Consensus loop iteration started
                let mut metadata = TraceMetadata::new();
                metadata.block_number = Some(block_number);
                LifecycleTracer::global().trace_step(
                    40,
                    "dcf.rs::run",
                    &format!(
                        "Consensus loop iteration started for block {}",
                        block_number
                    ),
                    Some(metadata),
                );

                // Get active validators from runtime
                let active_validators = {
                    let api = self.client.runtime_api();
                    api.get_active_validators(best_hash)
                };

                match active_validators {
                    Ok(validators) => {
                        // STEP 41: Active validators retrieved
                        let mut metadata = TraceMetadata::new();
                        metadata.block_number = Some(block_number);
                        metadata
                            .custom
                            .insert("validator_count".to_string(), validators.len().to_string());
                        LifecycleTracer::global().trace_step(
                            41,
                            "dcf.rs::run",
                            &format!(
                                "Active validators retrieved: {} validators",
                                validators.len()
                            ),
                            Some(metadata),
                        );

                        if validators.is_empty() {
                            error!("DCF: No active validators available for block production");
                        } else {
                            // STEP 42: Author selection started
                            let mut metadata = TraceMetadata::new();
                            metadata.block_number = Some(block_number);
                            LifecycleTracer::global().trace_step(
                                42,
                                "dcf.rs::run",
                                &format!("Author selection for block {} started", block_number),
                                Some(metadata),
                            );

                            // Select next author using runtime logic
                            match self.select_next_author_from_runtime(&validators) {
                                Ok(author) => {
                                    // STEP 43: Expected author selected
                                    let author_account: AccountId = author.into();
                                    let mut metadata = TraceMetadata::new();
                                    metadata.block_number = Some(block_number);
                                    metadata.author = Some(format!("{:?}", author_account));
                                    metadata.custom.insert(
                                        "selection_method".to_string(),
                                        "runtime_deterministic".to_string(),
                                    );
                                    LifecycleTracer::global().trace_step(
                                        43,
                                        "dcf.rs::run",
                                        &format!("Expected author selected: {:?} (method: runtime_deterministic)", author_account),
                                        Some(metadata),
                                    );

                                    // STEP 44: Block production triggered
                                    let mut metadata = TraceMetadata::new();
                                    metadata.block_number = Some(block_number);
                                    metadata.author = Some(format!("{:?}", author_account));
                                    LifecycleTracer::global().trace_step(
                                        44,
                                        "dcf.rs::run",
                                        &format!(
                                            "Block production triggered for block {}",
                                            block_number
                                        ),
                                        Some(metadata),
                                    );

                                    // Requirement 11: Double block production guard.
                                    // Re-read best_number immediately before producing to catch any
                                    // block that a peer may have imported since we entered this branch.
                                    let current_best: u32 =
                                        self.client.info().best_number.saturated_into::<u32>();
                                    if current_best >= block_number {
                                        debug!(
                                            "DCF: Block {} already produced by peer, skipping",
                                            block_number
                                        );
                                        // Advance timing so we don't spin-produce on the next loop tick.
                                        self.last_block_time = std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            // FB-13: Zero would immediately re-trigger production;
                                            // log loudly if the system clock is before UNIX epoch.
                                            .unwrap_or_else(|e| {
                                                error!("DCF: System clock before UNIX epoch ({:?}). Using zero — may cause immediate block production.", e);
                                                Duration::ZERO
                                            });
                                    } else {
                                        match self.produce_block_with_validation(&author).await {
                                            Ok(()) => {
                                                debug!(
                                                    "Block production successful for author {:?}",
                                                    author
                                                );
                                            }
                                            Err(e) => {
                                                LifecycleTracer::global().trace_error(
                                                    44,
                                                    "dcf.rs::run",
                                                    &e,
                                                    "Block production failed",
                                                );
                                                debug!(
                                                    "Block production failed for author {:?}: {:?}",
                                                    author, e
                                                );
                                                // Update consensus state to handle the failure
                                                let author_account: AccountId = author.into();
                                                if let Err(state_err) = self
                                                    .handle_block_production_failure(
                                                        &author_account,
                                                    )
                                                    .await
                                                {
                                                    debug!("Failed to handle block production failure: {:?}", state_err);
                                                }
                                            }
                                        }
                                    } // end else (double block production guard)
                                } // end Ok(author)
                                Err(e) => debug!("Failed to select next author: {:?}", e),
                            }
                        }
                    }
                    Err(e) => error!("DCF: Failed to get active validators: {:?}", e),
                }

                // Epoch transitions are handled exclusively by on_initialize; do not trigger from the consensus loop
            }

            // Update validator metrics periodically
            if self.current_slot % self.params.metrics_update_interval == 0
                && self.last_metrics_update_slot != self.current_slot
            {
                self.update_validator_metrics().await;
                self.last_metrics_update_slot = self.current_slot;
            }

            // Refresh validator scores periodically to ensure fresh PoS/PoI data
            if self.current_slot % self.params.score_refresh_interval == 0
                && self.last_score_refresh_slot != self.current_slot
            {
                if let Err(e) = self.refresh_validator_scores().await {
                    debug!("Failed to refresh validator scores: {:?}", e);
                }
                self.last_score_refresh_slot = self.current_slot;
            }

            sleep(Duration::from_millis(self.params.consensus_loop_interval)).await;
        }
    }

    /// Check if it's time to produce a new block
    fn should_produce_block(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            // FB-13: Zero here means time_since_last will be large, triggering immediate production.
            // Log loudly if the system clock is anomalous.
            .unwrap_or_else(|e| {
                error!("DCF: System clock before UNIX epoch ({:?}). Block production timer may fire immediately.", e);
                Duration::ZERO
            });

        let block_interval = Duration::from_secs(self.params.block_time);
        let time_since_last = now.saturating_sub(self.last_block_time);

        let should_produce = time_since_last >= block_interval;

        if should_produce {
            debug!(
                "Time to produce block - interval: {:?}, time since last: {:?}",
                block_interval, time_since_last
            );
        }

        should_produce
    }

    /// Update validator metrics from runtime with fresh PoS and PoI scores
    async fn update_validator_metrics(&mut self) {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Update consensus metrics manually since we can't use the generic update_from_runtime
        // due to trait bound constraints. The service layer will handle periodic updates.
        if let Some(ref _metrics) = self.consensus_metrics {
            debug!("DCF: Consensus metrics available, updates handled by service layer");
        }

        // FB-09: Do not fabricate weights if the API fails — different nodes could get
        // different API errors and silently use different weights during an upgrade/migration.
        // Skip this metrics cycle entirely instead.
        // Note: the contribution part holds at zero until the optional module lands.
        let (pos_weight, poi_weight) = match api.get_consensus_weights(best_hash) {
            Ok(weights) => weights,
            Err(e) => {
                warn!(
                    "DCF: get_consensus_weights API error: {:?} — skipping metrics update cycle.",
                    e
                );
                return;
            }
        };

        // Get all validator scores and update metrics
        if let Ok(scores) = api.get_validator_scores(best_hash) {
            for (account_id, _final_score) in scores {
                let public_key = Public::from_raw(*account_id.as_ref());

                // Get detailed validator information with fresh scores
                if let Ok(Some(profile)) = api.get_validator_profile(best_hash, account_id.clone())
                {
                    let combined_score = profile.final_score;
                    let poi_score = profile.poi_score as u64;
                    let _trust_score = profile.trust_score;
                    let inference_count = profile.inference_count;
                    // Get actual metrics from PoS pallet and validator state
                    let pos_score = self.get_pos_score(&account_id);
                    let uptime = self.calculate_validator_uptime(&account_id);
                    let (participation_rate, missed_blocks) =
                        self.get_validator_participation_metrics(&account_id);

                    self.metrics.update_validator_score(
                        public_key,
                        uptime,
                        inference_count.try_into().unwrap_or(0),
                        combined_score.try_into().unwrap_or(0),
                    );

                    // Log detailed metrics periodically
                    if self.current_slot % self.params.detailed_logging_interval == 0 {
                        info!("DCF: Validator {:?} - Combined: {} (PoS: {} @{}%, PoI: {} @{}%), Participation: {}%, Missed: {}", 
                              account_id, combined_score, pos_score, pos_weight, poi_score, poi_weight, participation_rate, missed_blocks);
                    }

                    // Check for score imbalances and log warnings
                    if self.current_slot % 500 == 0 {
                        // Check every 500 slots
                        let total_weighted = pos_score.saturating_mul(pos_weight)
                            + poi_score.saturating_mul(poi_weight);
                        if total_weighted > 0u64 {
                            let pos_contribution =
                                (pos_score.saturating_mul(pos_weight) * 100) / total_weighted;
                            let poi_contribution =
                                (poi_score.saturating_mul(poi_weight) * 100) / total_weighted;

                            if pos_contribution > 85u64 {
                                debug!("DCF: Validator {:?} PoS-heavy ({}%) - consider increasing PoI activity", 
                                      account_id, pos_contribution);
                            } else if poi_contribution > 85u64 {
                                debug!("DCF: Validator {:?} PoI-heavy ({}%) - consider increasing stake", 
                                      account_id, poi_contribution);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Record reward distribution in consensus metrics
    pub fn record_reward_distribution(&self, amount: u128) {
        if let Some(ref metrics) = self.consensus_metrics {
            metrics.record_reward_distribution(amount);
            debug!("DCF: Recorded reward distribution of {}", amount);
        }
    }

    /// Record slashing event in consensus metrics
    pub fn record_slashing_event(&self, amount: u128) {
        if let Some(ref metrics) = self.consensus_metrics {
            metrics.record_slashing(amount);
            debug!("DCF: Recorded slashing event of {}", amount);
        }
    }

    /// Get PoS score for a validator from the PoS pallet
    fn get_pos_score(&self, validator: &AccountId) -> u64 {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        match api.get_validator_stake_score(best_hash, validator.clone()) {
            Ok(score) => score,
            Err(e) => {
                debug!(
                    "Failed to get PoS score for validator {:?}: {:?}",
                    validator, e
                );
                0u64
            }
        }
    }

    /// Calculate validator uptime based on historical data
    fn calculate_validator_uptime(&self, validator: &AccountId) -> u32 {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Get validator uptime from DCF pallet
        match api.get_validator_uptime(best_hash, validator.clone()) {
            Ok(Some(uptime_stats)) => {
                // Return the participation rate as uptime percentage
                uptime_stats.participation_rate
            }
            Ok(None) => {
                // FB-11: New validator — no uptime data yet. This is not an error.
                0u32
            }
            Err(e) => {
                // FB-11: Distinguish API errors from "no data yet" so we don't penalise
                // a validator whose uptime record simply cannot be fetched right now.
                warn!("DCF: Uptime API error for {:?}: {:?}", validator, e);
                0u32
            }
        }
    }

    /// Get validator participation rate and missed blocks
    fn get_validator_participation_metrics(&self, validator: &AccountId) -> (u32, u32) {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        match api.get_validator_participation(best_hash, validator.clone()) {
            Ok((authored, missed)) => {
                let total_blocks = authored + missed;
                let participation_rate = if total_blocks > 0 {
                    ((authored * 100) / total_blocks).min(100)
                } else {
                    // FB-14: Grace period for brand-new validators (zero authored AND zero missed).
                    // Starting at 100% prevents false penalisation before their first assigned slot.
                    // This only applies when total_blocks == 0, i.e., the validator has never
                    // produced or missed any block yet.
                    100
                };
                (participation_rate, missed)
            }
            Err(e) => {
                debug!(
                    "Failed to get participation metrics for validator {:?}: {:?}",
                    validator, e
                );
                // FB-10: Return u32::MAX as a sentinel meaning "data unavailable".
                // Callers that use this value for health checks or scoring should treat
                // u32::MAX as "skip / no data" rather than 0% participation.
                // TODO: refactor return type to Option<(u32,u32)> in the metrics refactor.
                (u32::MAX, u32::MAX)
            }
        }
    }

    /// Select the next block author from active validators using runtime logic
    fn select_next_author_from_runtime(
        &mut self,
        active_validators: &[AccountId],
    ) -> ConsensusResult<Public> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let block_number = self.client.info().best_number.saturated_into::<u32>() + 1;

        // Try to get expected author from runtime
        match api.get_expected_author(best_hash, block_number) {
            Ok(Some(account_id)) => {
                // Verify the author is in active validators
                if active_validators.contains(&account_id) {
                    Ok(Public::from_raw(*account_id.as_ref()))
                } else {
                    error!(
                        "DCF: Expected author {:?} is not in active validator set",
                        account_id
                    );
                    self.fallback_author_selection(active_validators)
                }
            }
            Ok(None) => {
                // FB-05/06: Runtime has no queued author for this slot.
                // Do NOT elect an author locally — two nodes with different API outcomes
                // would pick different authors, breaking global agreement.
                // Correct behaviour is to skip production and wait for the next slot.
                warn!(
                    "DCF: No expected author scheduled for block {} — skipping slot.",
                    block_number
                );
                Err(ConsensusError::AuthorSelection(
                    "No author scheduled for this slot".into(),
                ))
            }
            Err(e) => {
                // FB-05: Runtime API error — skip slot rather than elect locally.
                error!("DCF: Runtime API error getting expected author for block {}: {:?}. Skipping slot.", block_number, e);
                Err(ConsensusError::AuthorSelection(format!(
                    "Runtime API error: {:?}",
                    e
                )))
            }
        }
    }

    /// Fallback author selection using round-robin
    fn fallback_author_selection(
        &self,
        active_validators: &[AccountId],
    ) -> ConsensusResult<Public> {
        if active_validators.is_empty() {
            return Err(ConsensusError::AuthorSelection(
                "No active validators available".into(),
            ));
        }

        // FB-07: use chain-agreed best_number as seed, not the local unsynchronised
        // current_slot counter. Two nodes that restart at different times will have
        // different current_slot values and would pick different round-robin indices.
        let best_number = self.client.info().best_number.saturated_into::<u64>();
        let index = (best_number as usize) % active_validators.len();
        let selected_validator = &active_validators[index];

        debug!(
            "Using fallback selection (best_number={}) — validator {} of {}",
            best_number,
            index + 1,
            active_validators.len()
        );
        Ok(Public::from_raw(*selected_validator.as_ref()))
    }

    /// Produce a new block with DCF validation
    async fn produce_block_with_validation(&mut self, author: &Public) -> ConsensusResult<()> {
        // Task 8 requirement: Record block production time
        let production_start = std::time::Instant::now();

        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let best_number = self.client.info().best_number;
        let author_account_id: AccountId = (*author).into();
        let block_number = (best_number + 1u32.into()).saturated_into::<u32>();

        // Only log every 10th block to reduce noise
        if block_number % 10 == 1 {
            info!(
                "Producing block #{} with author {:?}",
                block_number, author_account_id
            );
        } else {
            log::trace!(
                "Producing block #{} with author {:?}",
                block_number,
                author_account_id
            );
        }

        // Validate block authorship through runtime
        api.validate_block_author(best_hash, block_number, author_account_id.clone())
            .map_err(|e| {
                ConsensusError::BlockProduction(format!("Author validation failed: {:?}", e))
            })?;

        // Get and update validator metrics
        if let Ok(scores) = api.get_validator_scores(best_hash) {
            if let Some((_, final_score)) = scores.iter().find(|(a, _)| a == &author_account_id) {
                self.metrics.update_validator_score(
                    *author,
                    0,
                    0,
                    (*final_score).try_into().unwrap_or(0),
                );
                debug!(
                    "DCF: Author {:?} has final score: {}",
                    author_account_id, final_score
                );
            }
        }

        // Get validator profile for additional metrics
        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, author_account_id.clone()) {
            let combined_score = profile.final_score;
            let pos_score = self.get_pos_score(&author_account_id);
            let poi_score = profile.poi_score as u64;
            let trust_score = profile.trust_score;
            let uptime = self.calculate_validator_uptime(&author_account_id);
            let inference_count = profile.inference_count;
            let (participation_rate, _) =
                self.get_validator_participation_metrics(&author_account_id);
            let (_, missed_blocks) = self.get_validator_participation_metrics(&author_account_id);
            debug!("DCF: Validator profile - Combined: {}, PoS: {}, PoI: {}, Trust: {}, Uptime: {}, Inferences: {}, Participation: {}%, Missed: {}", 
                  combined_score, pos_score, poi_score, trust_score, uptime, inference_count, participation_rate, missed_blocks);

            // Used to have Trace 7 here, removed to avoid duplicate since traces 7 and 8 are handled dynamically in the pallet.
        }

        // 1. Create block proposal with transactions from the pool (includes signing)
        let signed_block = self.create_block_proposal(author, block_number).await?;

        // 2. Import the block through the consensus pipeline
        self.import_consensus_block(signed_block).await?;

        // 3. Update consensus state after successful block production
        self.update_consensus_state(block_number, &author_account_id)
            .await?;

        // if let Ok(Some(profile)) = self.client.runtime_api().get_validator_profile(best_hash, author_account_id.clone()) {
        //     let combined_score = profile.final_score;
        //     let pos_score = self.get_pos_score(&author_account_id);
        //     let poi_score = profile.poi_score as u64;
        // }

        // Task 8 requirement: Record block production time metric
        let production_duration = production_start.elapsed();
        if let Some(ref metrics) = self.consensus_metrics {
            metrics.record_block_production_time(production_duration.as_secs_f64());
        }

        debug!(
            "Block #{} produced by {:?} in {:?}",
            block_number, author_account_id, production_duration
        );
        Ok(())
    }

    /// Create block proposal with transactions from the pool
    async fn create_block_proposal(
        &mut self,
        author: &Public,
        block_number: u32,
    ) -> ConsensusResult<B> {
        debug!(
            "Creating block proposal #{} for author {:?}",
            block_number, author
        );

        let parent_hash = self.client.info().best_hash;
        let parent_number = self.client.info().best_number;
        debug!(
            "Using parent hash {:?} (block #{})",
            parent_hash, parent_number
        );

        // Create the PreRuntime digest item with our author identity
        let digest_item =
            sp_runtime::generic::DigestItem::PreRuntime(crate::CERULEA_ENGINE_ID, author.encode());

        // Use the comprehensive block creation method with proper error handling
        debug!("DCF: Attempting to create block with comprehensive method");
        let (block, expected_author) = match self
            .proposer_factory
            .create_complete_block(
                parent_hash,
                block_number as u64,
                Some(digest_item), // Inject the digest item so other nodes know the author
                Some(*author),     // Force the author we selected
            )
            .await
        {
            Ok(result) => {
                debug!("DCF: Successfully created block with comprehensive method");
                result
            }
            Err(e) => {
                error!("DCF: Comprehensive block creation failed: {:?}", e);
                warn!("DCF: Attempting emergency block creation (skipping Level-2 to preserve author identity)");

                // FB-02: Level-2 (create_block_with_transactions) is intentionally skipped here.
                // It does not inject the author digest or force the selected author, so any block
                // produced through it would arrive at peers without an identity seal. We go directly
                // to the emergency block which re-injects the author correctly.
                self.proposer_factory.create_emergency_block(
                    parent_hash,
                    block_number as u64,
                    *author
                ).await.map_err(|emergency_error| {
                    error!("DCF: Emergency block creation also failed: {:?}", emergency_error);
                    ConsensusError::BlockProduction(format!(
                        "Both comprehensive and emergency block creation failed. Original: {:?}, Emergency: {:?}",
                        e, emergency_error
                    ))
                })?
            }
        };

        // Verify the expected author matches our selected author — hard reject on mismatch (FB-01)
        let author_account: AccountId = (*author).into();
        let expected_account: AccountId = expected_author.into();
        if author_account != expected_account {
            error!("DCF: Author mismatch — proposer returned {:?}, selected {:?}. Aborting block creation.",
                   expected_account, author_account);
            return Err(ConsensusError::AuthorMismatch {
                block_number,
                expected: Some(expected_account),
                actual: author_account,
            });
        }

        // Validate the created block
        self.validate_created_block(&block, block_number)?;

        debug!(
            "Created block #{} with {} extrinsics, parent: {:?}",
            block_number,
            block.extrinsics().len(),
            block.header().parent_hash()
        );
        debug!(
            "Block state root: {:?}, extrinsics root: {:?}",
            block.header().state_root(),
            block.header().extrinsics_root()
        );

        Ok(block)
    }

    /// Validate a created block before using it
    fn validate_created_block(&self, block: &B, expected_block_number: u32) -> ConsensusResult<()> {
        let header = block.header();

        // Check block number
        let actual_block_number = (*header.number()).saturated_into::<u32>();
        if actual_block_number != expected_block_number {
            return Err(ConsensusError::BlockProduction(format!(
                "Block number mismatch: expected {}, got {}",
                expected_block_number, actual_block_number
            )));
        }

        // Check that state root is not zero (the main fix for Issue #1)
        let zero_hash = Default::default();
        if header.state_root() == &zero_hash {
            return Err(ConsensusError::BlockProduction(
                "Created block has zero state root - this will cause import failures".into(),
            ));
        }

        // Check extrinsics root for non-empty blocks
        if !block.extrinsics().is_empty() && header.extrinsics_root() == &zero_hash {
            return Err(ConsensusError::BlockProduction(
                "Created block has zero extrinsics root but contains extrinsics".into(),
            ));
        }

        debug!(
            "DCF: Block validation passed - state root: {:?}, extrinsics root: {:?}",
            header.state_root(),
            header.extrinsics_root()
        );

        Ok(())
    }

    /// Import consensus block through pipeline
    async fn import_consensus_block(&self, block: B) -> ConsensusResult<()> {
        let block_hash = block.header().hash();
        let block_number = *block.header().number();
        let parent_hash = *block.header().parent_hash();

        debug!(
            "Importing consensus block #{} ({:?}) with parent {:?}",
            block_number, block_hash, parent_hash
        );

        // Create proper block import parameters
        let mut import_params =
            BlockImportParams::new(sp_consensus::BlockOrigin::Own, block.header().clone());
        import_params.body = Some(block.extrinsics().to_vec());
        import_params.finalized = false;
        import_params.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);

        // Import through our block import
        match self.block_import.import_block(import_params).await {
            Ok(ImportResult::Imported(_)) => {
                debug!("Block #{} successfully imported", block_number);
                Ok(())
            }
            Ok(ImportResult::AlreadyInChain) => {
                debug!("Block #{} already in chain", block_number);
                Ok(())
            }
            Ok(other) => {
                debug!("Block #{} import result: {:?}", block_number, other);
                Ok(())
            }
            Err(e) => {
                error!("Failed to import block #{}: {:?}", block_number, e);
                Err(ConsensusError::BlockImport(format!(
                    "Import failed: {:?}",
                    e
                )))
            }
        }
    }

    /// Update consensus state after successful block production
    async fn update_consensus_state(
        &mut self,
        block_number: u32,
        author: &AccountId,
    ) -> ConsensusResult<()> {
        // Update timing and slot
        self.last_block_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            // FB-13: Log clock anomalies; Duration::ZERO would cause immediate next-slot production.
            .unwrap_or_else(|e| {
                error!(
                    "DCF: System clock before UNIX epoch ({:?}). Block timing may be incorrect.",
                    e
                );
                Duration::ZERO
            });

        // Update metrics
        self.metrics.total_blocks = self.metrics.total_blocks.saturating_add(1);

        // Update validator-specific metrics
        let author_public = Public::from_raw(*author.as_ref());
        self.metrics.update_validator_score(author_public, 0, 0, 1); // Increment block count

        // Record successful block authorship in runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Record block authorship through runtime API
        if let Err(e) =
            api.report_successful_block_authorship(best_hash, block_number, author.clone())
        {
            warn!(
                "Failed to record block authorship for validator {:?} at block #{}: {:?}",
                author, block_number, e
            );
        } else {
            debug!(
                "Recorded successful block authorship for validator {:?} at block #{}",
                author, block_number
            );
        }

        // STEP 53: Consensus state updated after block production
        let mut metadata = TraceMetadata::new();
        metadata.block_number = Some(block_number);
        metadata.author = Some(format!("{:?}", author));
        metadata
            .custom
            .insert("slot".to_string(), self.current_slot.to_string());
        metadata.custom.insert(
            "total_blocks".to_string(),
            self.metrics.total_blocks.to_string(),
        );
        LifecycleTracer::global().trace_step(
            53,
            "dcf.rs::update_consensus_state",
            &format!(
                "Consensus state updated after block production (block: {}, slot: {}, total: {})",
                block_number, self.current_slot, self.metrics.total_blocks
            ),
            Some(metadata),
        );

        // Update validator performance in runtime
        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, author.clone()) {
            let combined_score = profile.final_score;
            let pos_score = self.get_pos_score(author);
            let poi_score = profile.poi_score as u64;
            let trust_score = profile.trust_score;
            let uptime = self.calculate_validator_uptime(author);
            let inference_count = profile.inference_count;
            let (participation_rate, missed_blocks) =
                self.get_validator_participation_metrics(author);

            // STEP 54: Author scores updated
            let mut metadata = TraceMetadata::new();
            metadata.block_number = Some(block_number);
            metadata.author = Some(format!("{:?}", author));
            metadata
                .custom
                .insert("pos_score".to_string(), pos_score.to_string());
            metadata
                .custom
                .insert("poi_score".to_string(), poi_score.to_string());
            metadata
                .custom
                .insert("combined_score".to_string(), combined_score.to_string());
            metadata
                .custom
                .insert("trust_score".to_string(), trust_score.to_string());
            metadata
                .custom
                .insert("uptime".to_string(), uptime.to_string());
            metadata
                .custom
                .insert("inference_count".to_string(), inference_count.to_string());
            metadata.custom.insert(
                "participation_rate".to_string(),
                participation_rate.to_string(),
            );
            metadata
                .custom
                .insert("missed_blocks".to_string(), missed_blocks.to_string());
            LifecycleTracer::global().trace_step(
                54,
                "dcf.rs::update_consensus_state",
                &format!("Author scores updated - PoS: {}, PoI: {}, Combined: {}, Trust: {}, Uptime: {}, Inferences: {}, Participation: {}%, Missed: {}", 
                         pos_score, poi_score, combined_score, trust_score, uptime, inference_count, participation_rate, missed_blocks),
                Some(metadata),
            );

            // Log the successful block production
            debug!(
                "Block #{} produced successfully by {:?}",
                block_number, author
            );
            debug!("Validator stats - Combined: {}, PoS: {}, PoI: {}, Trust: {}, Uptime: {}, Inferences: {}, Participation: {}%, Missed: {}", 
                  combined_score, pos_score, poi_score, trust_score, uptime, inference_count, participation_rate, missed_blocks);

            // Update local metrics with current runtime state
            self.metrics.update_validator_score(
                author_public,
                uptime,
                inference_count.try_into().unwrap_or(0),
                combined_score.try_into().unwrap_or(0),
            );
        }

        // Log consensus state update
        debug!(
            "Updated consensus state - Block: {}, Slot: {}, Total Blocks: {}, Author: {:?}",
            block_number, self.current_slot, self.metrics.total_blocks, author
        );

        // Periodic state health check
        if block_number % self.params.health_check_interval == 0 {
            self.log_consensus_health().await?;
        }

        Ok(())
    }

    /// Refresh validator scores in the runtime by triggering score updates
    async fn refresh_validator_scores(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Get all active validators and their fresh scores via runtime API
        if let Ok(active_validators) = api.get_active_validators(best_hash) {
            let mut updated_count = 0;

            for validator in active_validators.iter() {
                // Get fresh validator profile which includes updated PoS and PoI scores
                if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                    let combined_score = profile.final_score;
                    let pos_score = self.get_pos_score(validator);
                    let poi_score = profile.poi_score as u64;

                    // Log the fresh scores
                    debug!(
                        "DCF: Refreshed scores for {:?} - Combined: {}, PoS: {}, PoI: {}",
                        validator, combined_score, pos_score, poi_score
                    );
                    updated_count += 1;
                }
            }

            info!("DCF: Refreshed scores for {} validators", updated_count);
        }

        Ok(())
    }

    /// Log consensus health metrics
    async fn log_consensus_health(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let current_block = self.client.info().best_number.saturated_into::<u32>();

        // Get active validators count
        if let Ok(active_validators) = api.get_active_validators(best_hash) {
            let validator_count = active_validators.len();

            // Calculate average validator score
            let mut total_score = 0u64;
            let mut healthy_validators = 0;

            for validator in active_validators
                .iter()
                .take(self.params.health_check_sample_size as usize)
            {
                // Sample validators for health check
                if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                    let combined_score = profile.final_score;
                    let _trust_score = profile.trust_score;
                    let _inference_count = profile.inference_count;
                    let (participation_rate, missed_blocks) =
                        self.get_validator_participation_metrics(validator);
                    total_score += combined_score;
                    if combined_score >= self.params.healthy_validator_score
                        && participation_rate >= (self.params.healthy_participation_rate - 10)
                        && missed_blocks <= self.params.max_missed_blocks
                    {
                        healthy_validators += 1;
                    }
                }
            }

            let avg_score = if validator_count > 0 {
                total_score
                    / validator_count.min(self.params.health_check_sample_size as usize) as u64
            } else {
                0
            };

            info!("PoS+PoI: Consensus Health - Block: {}, Slot: {}, Validators: {}, Healthy: {}, Avg Score: {}", 
                  current_block, self.current_slot, validator_count, healthy_validators, avg_score);

            // Check if we need to trigger epoch transition
            if let Ok(current_epoch) = api.get_current_epoch(best_hash) {
                debug!(
                    "PoS+PoI: Current epoch: {}, Total blocks produced: {}",
                    current_epoch, self.metrics.total_blocks
                );
            }
        }

        Ok(())
    }

    /// Handle block production failure and update consensus state
    async fn handle_block_production_failure(&mut self, author: &AccountId) -> ConsensusResult<()> {
        let current_block = self.client.info().best_number.saturated_into::<u32>() + 1;

        // FB-12: derive slot from chain state (best_number) instead of a local
        // unsynchronised counter. This prevents current_slot from diverging between
        // nodes that restart at different times, which would corrupt the FB-07 seed.
        self.current_slot = self
            .client
            .info()
            .best_number
            .saturated_into::<u64>()
            .saturating_add(1);

        // Update timing to prevent immediate retry
        self.last_block_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            // FB-13: Log clock anomalies; Duration::ZERO would cause immediate retry.
            .unwrap_or_else(|e| {
                error!("DCF: System clock before UNIX epoch ({:?}). Block timing on failure may be incorrect.", e);
                Duration::ZERO
            });

        // Update metrics to track the failure
        self.metrics.failed_blocks = self.metrics.failed_blocks.saturating_add(1);

        // Record missed block in runtime
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        if let Err(e) = api.report_missed_block(best_hash, current_block, author.clone()) {
            warn!(
                "Failed to record missed block for validator {:?} at block #{}: {:?}",
                author, current_block, e
            );
        } else {
            debug!(
                "Recorded missed block for validator {:?} at block #{}",
                author, current_block
            );
        }

        // Log the failure for monitoring
        info!(
            "PoS+PoI: Block production failed - Slot: {}, Failed blocks: {}, Author: {:?}",
            self.current_slot, self.metrics.failed_blocks, author
        );

        // Get updated validator metrics after recording the missed block
        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, author.clone()) {
            let combined_score = profile.final_score;
            let pos_score = self.get_pos_score(author);
            let poi_score = profile.poi_score as u64;
            let (participation_rate, missed_blocks) =
                self.get_validator_participation_metrics(author);

            info!("PoS+PoI: Validator {:?} missed block - Combined: {}, PoS: {}, PoI: {}, Participation: {}%, Total Missed: {}", 
                  author, combined_score, pos_score, poi_score, participation_rate, missed_blocks);
        }

        Ok(())
    }
}

/// Block import implementation that actually imports blocks to chain state
pub struct RealBlockImport<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    client: Arc<C>,
    _phantom: std::marker::PhantomData<B>,
}

impl<B, C> RealBlockImport<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    /// Create a new RealBlockImport instance
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _phantom: std::marker::PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<B, C> BlockImport<B> for RealBlockImport<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + BlockImport<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    type Error = sp_consensus::Error;

    async fn check_block(
        &self,
        block: sc_consensus::BlockCheckParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = block.number.saturated_into::<u32>();

        debug!("Checking block #{}", block_number);

        // Basic validation - check if we have active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e))
        })?;

        if active_validators.is_empty() {
            error!(
                "DCF: Block check failed for block #{}: No active validators",
                block_number
            );
            // FB-03: hard reject — imported(false) is NOT a rejection, it silently accepts the block.
            return Err(sp_consensus::Error::ClientImport(format!(
                "No active validators at block #{}",
                block_number
            )));
        }

        debug!("Block #{} check passed", block_number);
        Ok(ImportResult::imported(true))
    }

    async fn import_block(
        &self,
        block: BlockImportParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();

        debug!("Importing block #{} ({:?})", block_number, block_hash);

        // Validate that we have active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e))
        })?;

        if active_validators.is_empty() {
            error!(
                "DCF: Block import failed for block #{}: No active validators",
                block_number
            );
            // FB-03: hard reject — imported(false) is NOT a rejection, it silently accepts the block.
            return Err(sp_consensus::Error::ClientImport(format!(
                "No active validators at block #{}",
                block_number
            )));
        }

        // Extract block author from digest before importing
        let block_author = self.extract_block_author_from_digest(&block.header);

        // Validate block authorship if we have an author
        if let Some(author) = &block_author {
            // Validate that the author is an active validator
            if !active_validators.contains(author) {
                warn!(
                    "Block #{} authored by inactive validator {:?}",
                    block_number, author
                );
            }

            // Check if this matches the expected author — hard reject on mismatch (FB-04)
            if let Ok(Some(expected_author)) = api.get_expected_author(best_hash, block_number) {
                if *author != expected_author {
                    error!(
                        "DCF: Block #{} author mismatch: expected {:?}, got {:?}. Rejecting block.",
                        block_number, expected_author, author
                    );
                    // Report the mismatch and record missed block for expected author
                    let _ = api.report_author_mismatch(
                        best_hash,
                        block_number,
                        Some(expected_author.clone()),
                        author.clone(),
                    );
                    let _ = api.report_missed_block(best_hash, block_number, expected_author);
                    // Hard reject — do not import a block from the wrong author
                    return Err(sp_consensus::Error::InvalidAuthoritiesSet);
                } else {
                    // Correct author, record successful authorship
                    let _ = api.report_successful_block_authorship(
                        best_hash,
                        block_number,
                        author.clone(),
                    );
                }
            } else {
                // No expected author, still record the authorship
                let _ =
                    api.report_successful_block_authorship(best_hash, block_number, author.clone());
            }
        } else {
            // No author found in block, check if we expected one
            if let Ok(Some(expected_author)) = api.get_expected_author(best_hash, block_number) {
                warn!(
                    "Block #{} has no author but expected {:?}",
                    block_number, expected_author
                );
                let _ = api.report_missed_block(best_hash, block_number, expected_author);
            }
        }

        // Actually import the block using the client's import functionality
        let import_result = self.client.import_block(block).await.map_err(|e| {
            sp_consensus::Error::ClientImport(format!("Client import failed: {:?}", e))
        })?;

        // Log periodic statistics
        if block_number % 10u32 == 0 {
            let current_epoch = api.get_current_epoch(best_hash).unwrap_or(0);
            info!(
                "Block #{} processed - {} active validators, epoch {}",
                block_number,
                active_validators.len(),
                current_epoch
            );
        }

        debug!(
            "Block #{} ({:?}) successfully imported to chain",
            block_number, block_hash
        );

        Ok(import_result)
    }
}

impl<B, C> RealBlockImport<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    /// Extract block author from block header digest
    fn extract_block_author_from_digest(&self, header: &B::Header) -> Option<AccountId> {
        use sp_core::Decode;
        use sp_runtime::DigestItem;

        // Look for consensus digest items that might contain author information
        for log in header.digest().logs() {
            match log {
                DigestItem::Seal(engine_id, data) => {
                    // Check if this is a Cerulea consensus seal
                    if engine_id == b"ceru" {
                        // Try to extract author from seal data
                        // The seal should contain author information
                        if data.len() >= 32 {
                            // First 32 bytes should be the author's public key
                            if let Ok(author) = AccountId::decode(&mut &data[0..32]) {
                                return Some(author);
                            }
                        }
                    }
                }
                DigestItem::PreRuntime(engine_id, data) => {
                    // Check for pre-runtime digest with author info
                    if engine_id == b"ceru" {
                        if let Ok(author) = AccountId::decode(&mut &data[..]) {
                            return Some(author);
                        }
                    }
                }
                DigestItem::Consensus(engine_id, data) => {
                    // Check for consensus digest with author info
                    if engine_id == b"ceru" {
                        if let Ok(author) = AccountId::decode(&mut &data[..]) {
                            return Some(author);
                        }
                    }
                }
                _ => {}
            }
        }

        None
    }

    /// Update validator scores based on block authorship
    pub fn update_block_authorship_scores(&self, block_number: u32) {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Get the expected author for this block
        if let Ok(Some(expected_author)) = api.get_expected_author(best_hash, block_number) {
            // Record successful block authorship
            debug!(
                "Recording block authorship for validator {:?} at block #{}",
                expected_author, block_number
            );

            // Update validator metrics
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, expected_author.clone())
            {
                let combined_score = profile.final_score;
                let pos_score = api
                    .get_validator_stake_score(best_hash, expected_author.clone())
                    .unwrap_or(0);
                let poi_score = profile.poi_score as u64;
                let trust_score = profile.trust_score;
                let uptime = api
                    .get_validator_uptime(best_hash, expected_author.clone())
                    .unwrap_or(None)
                    .map(|stats| stats.participation_rate)
                    .unwrap_or(0);
                let inference_count = profile.inference_count;

                // Log validator metrics
                debug!("Validator {:?} metrics - Combined: {}, PoS: {}, PoI: {}, Trust: {}, Uptime: {}, Inferences: {}", 
                      expected_author, combined_score, pos_score, poi_score, trust_score, uptime, inference_count);

                // Log successful block production
                debug!(
                    "Block #{} successfully produced by validator {:?} (combined score: {})",
                    block_number, expected_author, combined_score
                );
            }
        }
    }
}

/// Start the DCF consensus engine
pub async fn start_dcf_consensus<B, C, TP, BE>(
    client: Arc<C>,
    transaction_pool: Arc<TP>,
    params: ConsensusParams,
) where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + sp_block_builder::BlockBuilder<B>,
    TP: TransactionPool<Block = B> + 'static,
    BE: sc_client_api::Backend<B> + 'static,
{
    // Create the DCF block import queue for consensus validation
    let block_import = Arc::new(crate::import_queue::DcfImportQueue::<B, C, BE>::new(
        client.clone(),
    ));
    let mut consensus: DcfConsensus<B, C, sp_core::ed25519::Pair, TP> =
        DcfConsensus::new(client, transaction_pool, block_import, params);
    consensus.run().await;
}
