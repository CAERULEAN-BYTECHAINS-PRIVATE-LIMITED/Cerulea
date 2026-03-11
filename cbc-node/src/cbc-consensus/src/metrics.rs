//! Consensus metrics implementation
//! 
//! This module handles tracking and reporting of consensus-related metrics
//! including validator economics, rewards, and slashing data.

use sp_runtime::traits::Block as BlockTrait;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;

use std::collections::HashMap;
use std::time::Instant;
use std::sync::Arc;
use parking_lot::RwLock;
use prometheus::{
    Registry, Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, HistogramOpts, Opts,
    register_counter_with_registry, register_counter_vec_with_registry, register_gauge_with_registry, 
    register_histogram_with_registry, register_gauge_vec_with_registry, register_histogram_vec_with_registry,
};
use pallet_cbc_dcf::DcfApi;
// Remove unused imports - we'll use the runtime types directly in the trait bound

/// Validator economics metrics for Prometheus
#[derive(Clone)]
pub struct ValidatorEconomicsMetrics {
    /// Number of active validators
    pub active_validators: Gauge,
    /// Total stake across all validators
    pub total_stake: Gauge,
    /// Total rewards distributed
    pub total_rewards_distributed: Counter,
    /// Total amount slashed
    pub total_slashed_amount: Counter,
    /// Average validator score
    pub average_validator_score: Gauge,
    /// Epoch duration histogram
    pub epoch_duration: Histogram,
    /// Rewards per epoch
    pub rewards_per_epoch: Histogram,
    /// Slashing events counter
    pub slashing_events: Counter,
    /// Validator participation rate
    pub validator_participation_rate: Gauge,
    /// Total inference results processed
    pub total_inference_results: Counter,
}

impl ValidatorEconomicsMetrics {
    /// Create new validator economics metrics
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            active_validators: register_gauge_with_registry!(
                Opts::new("cbc_active_validators", "Number of active validators"),
                registry
            )?,
            total_stake: register_gauge_with_registry!(
                Opts::new("cbc_total_stake", "Total stake across all validators"),
                registry
            )?,
            total_rewards_distributed: register_counter_with_registry!(
                Opts::new("cbc_total_rewards_distributed", "Total rewards distributed to validators"),
                registry
            )?,
            total_slashed_amount: register_counter_with_registry!(
                Opts::new("cbc_total_slashed_amount", "Total amount slashed from validators"),
                registry
            )?,
            average_validator_score: register_gauge_with_registry!(
                Opts::new("cbc_average_validator_score", "Average validator performance score"),
                registry
            )?,
            epoch_duration: register_histogram_with_registry!(
                HistogramOpts::new("cbc_epoch_duration_seconds", "Duration of epochs in seconds"),
                registry
            )?,
            rewards_per_epoch: register_histogram_with_registry!(
                HistogramOpts::new("cbc_rewards_per_epoch", "Rewards distributed per epoch"),
                registry
            )?,
            slashing_events: register_counter_with_registry!(
                Opts::new("cbc_slashing_events", "Number of slashing events"),
                registry
            )?,
            validator_participation_rate: register_gauge_with_registry!(
                Opts::new("cbc_validator_participation_rate", "Average validator participation rate"),
                registry
            )?,
            total_inference_results: register_counter_with_registry!(
                Opts::new("cbc_total_inference_results", "Total inference results processed"),
                registry
            )?,
        })
    }

    /// Update active validators count
    pub fn update_active_validators(&self, count: u64) {
        self.active_validators.set(count as f64);
    }

    /// Update total stake
    pub fn update_total_stake(&self, stake: u128) {
        self.total_stake.set(stake as f64);
    }

    /// Record reward distribution
    pub fn record_reward_distribution(&self, amount: u128) {
        self.total_rewards_distributed.inc_by(amount as f64);
    }

    /// Record slashing event
    pub fn record_slashing(&self, amount: u128) {
        self.total_slashed_amount.inc_by(amount as f64);
        self.slashing_events.inc();
    }

    /// Update average validator score
    pub fn update_average_score(&self, score: f64) {
        self.average_validator_score.set(score);
    }

    /// Record epoch duration
    pub fn record_epoch_duration(&self, duration_secs: f64) {
        self.epoch_duration.observe(duration_secs);
    }

    /// Record rewards per epoch
    pub fn record_rewards_per_epoch(&self, rewards: f64) {
        self.rewards_per_epoch.observe(rewards);
    }

    /// Update validator participation rate
    pub fn update_participation_rate(&self, rate: f64) {
        self.validator_participation_rate.set(rate);
    }

    /// Record inference result
    pub fn record_inference_result(&self) {
        self.total_inference_results.inc();
    }
}

/// Enhanced ConsensusMetrics struct for comprehensive monitoring
#[derive(Clone)]
pub struct ConsensusMetrics {
    /// Number of active validators
    pub active_validators: Gauge,
    /// Total reserved stake across all validators
    pub total_reserved_stake: Gauge,
    /// Current epoch number
    pub current_epoch: Gauge,
    /// Total rewards distributed to validators
    pub total_rewards_distributed: Counter,
    /// Total amount slashed from validators
    pub total_slashed_amount: Counter,
    /// Author mismatch counter (Task 8 requirement)
    pub author_mismatch_total: Counter,
    /// Epoch transitions counter (Task 8 requirement)
    pub epoch_transitions_total: Counter,
    /// Validator score gauge vector (Task 8 requirement)
    pub validator_score_gauge: GaugeVec,
    /// Block production time histogram (Task 8 requirement)
    pub block_production_time: Histogram,
    /// RPC request duration histogram vector (Task 8 requirement)
    pub rpc_request_duration: HistogramVec,
}

impl ConsensusMetrics {
    /// Create new ConsensusMetrics with Prometheus registry
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            active_validators: register_gauge_with_registry!(
                Opts::new("cbc_consensus_active_validators", "Number of active validators in the consensus"),
                registry
            )?,
            total_reserved_stake: register_gauge_with_registry!(
                Opts::new("cbc_consensus_total_reserved_stake", "Total reserved stake across all validators"),
                registry
            )?,
            current_epoch: register_gauge_with_registry!(
                Opts::new("cbc_consensus_current_epoch", "Current epoch number"),
                registry
            )?,
            total_rewards_distributed: register_counter_with_registry!(
                Opts::new("cbc_consensus_total_rewards_distributed", "Total rewards distributed to validators"),
                registry
            )?,
            total_slashed_amount: register_counter_with_registry!(
                Opts::new("cbc_consensus_total_slashed_amount", "Total amount slashed from validators"),
                registry
            )?,
            // Task 8 requirement: Register author_mismatch_total counter
            author_mismatch_total: register_counter_with_registry!(
                Opts::new("cbc_author_mismatch_total", "Total number of author mismatch events"),
                registry
            )?,
            // Task 8 requirement: Register epoch_transitions_total counter
            epoch_transitions_total: register_counter_with_registry!(
                Opts::new("cbc_epoch_transitions_total", "Total number of epoch transitions"),
                registry
            )?,
            // Task 8 requirement: Register validator_score_gauge gauge vector
            validator_score_gauge: register_gauge_vec_with_registry!(
                Opts::new("cbc_validator_score", "Current validator trust scores"),
                &["validator_id"],
                registry
            )?,
            // Task 8 requirement: Register block_production_time histogram
            block_production_time: register_histogram_with_registry!(
                HistogramOpts::new("cbc_block_production_time_seconds", "Time taken to produce blocks")
                    .buckets(vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]),
                registry
            )?,
            // Task 8 requirement: Register rpc_request_duration histogram vector
            rpc_request_duration: register_histogram_vec_with_registry!(
                HistogramOpts::new("cbc_rpc_request_duration_seconds", "Duration of RPC requests")
                    .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0]),
                &["method"],
                registry
            )?,
        })
    }

    /// Update metrics from runtime state
    pub fn update_from_runtime<B, C>(&self, client: &Arc<C>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        B: BlockTrait,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: DcfApi<B, sp_runtime::AccountId32, u128, u32>,
    {
        let best_hash = client.info().best_hash;
        let api = client.runtime_api();

        // Update active validators count
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| format!("Failed to get active validators: {:?}", e))?;
        self.active_validators.set(active_validators.len() as f64);

        // Update current epoch
        let current_epoch = api.get_current_epoch(best_hash)
            .map_err(|e| format!("Failed to get current epoch: {:?}", e))?;
        self.current_epoch.set(current_epoch as f64);

        // Calculate total reserved stake
        let mut total_stake: u128 = 0;
        for validator in &active_validators {
            let stake = api.get_validator_stake(best_hash, validator.clone())
                .map_err(|e| format!("Failed to get validator stake: {:?}", e))?;
            total_stake = total_stake.saturating_add(stake);
        }
        self.total_reserved_stake.set(total_stake as f64);

        // Task 8 requirement: Update validator scores
        if let Err(e) = self.update_validator_scores(client) {
            log::warn!("Failed to update validator score metrics: {}", e);
        }

        Ok(())
    }

    /// Record reward distribution
    pub fn record_reward_distribution(&self, amount: u128) {
        self.total_rewards_distributed.inc_by(amount as f64);
    }

    /// Record slashing event
    pub fn record_slashing(&self, amount: u128) {
        self.total_slashed_amount.inc_by(amount as f64);
    }

    /// Update active validators count manually
    pub fn update_active_validators(&self, count: u64) {
        self.active_validators.set(count as f64);
    }

    /// Update total reserved stake manually
    pub fn update_total_reserved_stake(&self, stake: u128) {
        self.total_reserved_stake.set(stake as f64);
    }

    /// Update current epoch manually
    pub fn update_current_epoch(&self, epoch: u32) {
        self.current_epoch.set(epoch as f64);
    }

    // Task 8 requirement: Methods to update new metrics on relevant events

    /// Record author mismatch event
    pub fn record_author_mismatch(&self) {
        self.author_mismatch_total.inc();
    }

    /// Record epoch transition event
    pub fn record_epoch_transition(&self) {
        self.epoch_transitions_total.inc();
    }

    /// Update validator score gauge
    pub fn update_validator_score(&self, validator_id: &str, score: f64) {
        self.validator_score_gauge
            .with_label_values(&[validator_id])
            .set(score);
    }

    /// Update validator scores for all validators
    pub fn update_validator_scores<B, C>(&self, client: &Arc<C>) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
    where
        B: BlockTrait,
        C: ProvideRuntimeApi<B> + HeaderBackend<B>,
        C::Api: DcfApi<B, sp_runtime::AccountId32, u128, u32>,
    {
        let best_hash = client.info().best_hash;
        let api = client.runtime_api();

        // Get validator scores from runtime
        let validator_scores = api.get_validator_scores(best_hash)
            .map_err(|e| format!("Failed to get validator scores: {:?}", e))?;

        // Update gauge for each validator
        for (validator, score) in validator_scores {
            let validator_id = format!("{:?}", validator);
            self.update_validator_score(&validator_id, score as f64);
        }

        Ok(())
    }

    /// Record block production time
    pub fn record_block_production_time(&self, duration_seconds: f64) {
        self.block_production_time.observe(duration_seconds);
    }

    /// Record RPC request duration
    pub fn record_rpc_request_duration(&self, method: &str, duration_seconds: f64) {
        self.rpc_request_duration
            .with_label_values(&[method])
            .observe(duration_seconds);
    }
}

/// DVF (Dynamic Validator Finality) metrics for Prometheus
#[derive(Clone)]
pub struct DvfMetrics {
    /// Current finalized block number
    pub dvf_finalized_block_number: Gauge,
    /// Total votes received per round
    pub dvf_votes_received_total: CounterVec,
    /// Accumulated weight per block hash
    pub dvf_accumulated_weight: GaugeVec,
    /// Vote rejections by reason
    pub dvf_vote_rejections_total: CounterVec,
    /// Finality latency from checkpoint to finalization
    pub dvf_finality_latency_seconds: Histogram,
    /// Double vote detections
    pub dvf_double_votes_detected_total: Counter,
}

impl DvfMetrics {
    /// Create new DVF metrics
    pub fn new(registry: &Registry) -> Result<Self, prometheus::Error> {
        Ok(Self {
            dvf_finalized_block_number: register_gauge_with_registry!(
                Opts::new("dvf_finalized_block_number", "Current finalized block number in DVF"),
                registry
            )?,
            dvf_votes_received_total: register_counter_vec_with_registry!(
                Opts::new("dvf_votes_received_total", "Total DVF votes received per round"),
                &["round"],
                registry
            )?,
            dvf_accumulated_weight: register_gauge_vec_with_registry!(
                Opts::new("dvf_accumulated_weight", "Accumulated voting weight per block hash"),
                &["block_hash"],
                registry
            )?,
            dvf_vote_rejections_total: register_counter_vec_with_registry!(
                Opts::new("dvf_vote_rejections_total", "Total DVF vote rejections by reason"),
                &["reason"],
                registry
            )?,
            dvf_finality_latency_seconds: register_histogram_with_registry!(
                HistogramOpts::new("dvf_finality_latency_seconds", "Time from checkpoint to finalization")
                    .buckets(vec![1.0, 2.0, 5.0, 10.0, 20.0, 30.0, 60.0, 120.0]),
                registry
            )?,
            dvf_double_votes_detected_total: register_counter_with_registry!(
                Opts::new("dvf_double_votes_detected_total", "Total double vote detections"),
                registry
            )?,
        })
    }

    /// Update finalized block number
    pub fn update_finalized_block_number(&self, block_number: u32) {
        self.dvf_finalized_block_number.set(block_number as f64);
    }

    /// Record vote reception for a round
    pub fn record_vote_received(&self, round: u32) {
        self.dvf_votes_received_total
            .with_label_values(&[&round.to_string()])
            .inc();
    }

    /// Update accumulated weight for a block hash
    pub fn update_accumulated_weight(&self, block_hash: &str, weight: u128) {
        self.dvf_accumulated_weight
            .with_label_values(&[block_hash])
            .set(weight as f64);
    }

    /// Record vote rejection with reason
    pub fn record_vote_rejection(&self, reason: &str) {
        self.dvf_vote_rejections_total
            .with_label_values(&[reason])
            .inc();
    }

    /// Record finality latency
    pub fn record_finality_latency(&self, latency_seconds: f64) {
        self.dvf_finality_latency_seconds.observe(latency_seconds);
    }

    /// Record double vote detection
    pub fn record_double_vote_detection(&self) {
        self.dvf_double_votes_detected_total.inc();
    }
}

/// Legacy metrics for tracking consensus performance (kept for backward compatibility)
pub struct LegacyConsensusMetrics<B: BlockTrait> {
    /// Total number of blocks produced
    pub total_blocks: u32,
    /// Total number of transactions processed
    pub total_transactions: u32,
    /// Average time between blocks
    pub average_block_time: u64,
    /// Per-validator metrics
    pub validator_metrics: HashMap<B::Hash, u32>,
    /// Start time of the current epoch
    pub epoch_start_time: Instant,
    /// Validator economics metrics
    pub economics_metrics: Option<ValidatorEconomicsMetrics>,
    /// Epoch rewards tracking
    pub epoch_rewards: Arc<RwLock<u128>>,
}

impl<B: BlockTrait> LegacyConsensusMetrics<B> {
    /// Create a new metrics tracker
    pub fn new() -> Self {
        Self {
            total_blocks: 0,
            total_transactions: 0,
            average_block_time: 0,
            validator_metrics: HashMap::new(),
            epoch_start_time: Instant::now(),
            economics_metrics: None,
            epoch_rewards: Arc::new(RwLock::new(0)),
        }
    }

    /// Create a new metrics tracker with Prometheus registry
    pub fn new_with_registry(registry: &Registry) -> Result<Self, prometheus::Error> {
        let economics_metrics = ValidatorEconomicsMetrics::new(registry)?;
        
        Ok(Self {
            total_blocks: 0,
            total_transactions: 0,
            average_block_time: 0,
            validator_metrics: HashMap::new(),
            epoch_start_time: Instant::now(),
            economics_metrics: Some(economics_metrics),
            epoch_rewards: Arc::new(RwLock::new(0)),
        })
    }

    /// Update block metrics with new block time and transaction count
    pub fn update_block_metrics(&mut self, block_time: u64, tx_count: u32) {
        self.total_blocks = self.total_blocks.saturating_add(1);
        self.total_transactions = self.total_transactions.saturating_add(tx_count);
        self.average_block_time = (self.average_block_time + block_time) / 2;
    }

    /// Update validator metrics with new score
    pub fn update_validator_metrics(&mut self, validator: B::Hash, score: u32) {
        self.validator_metrics.insert(validator, score);
    }

    /// Get the duration of the current epoch
    pub fn get_epoch_duration(&self) -> u64 {
        self.epoch_start_time.elapsed().as_secs()
    }

    /// Update validator economics metrics
    pub fn update_validator_economics(&self, 
        active_count: u64, 
        total_stake: u128, 
        avg_score: f64,
        participation_rate: f64
    ) {
        if let Some(ref metrics) = self.economics_metrics {
            metrics.update_active_validators(active_count);
            metrics.update_total_stake(total_stake);
            metrics.update_average_score(avg_score);
            metrics.update_participation_rate(participation_rate);
        }
    }

    /// Record reward distribution
    pub fn record_reward(&self, amount: u128) {
        if let Some(ref metrics) = self.economics_metrics {
            metrics.record_reward_distribution(amount);
        }
        
        // Track epoch rewards
        let mut epoch_rewards = self.epoch_rewards.write();
        *epoch_rewards = epoch_rewards.saturating_add(amount);
    }

    /// Record slashing event
    pub fn record_slashing(&self, amount: u128) {
        if let Some(ref metrics) = self.economics_metrics {
            metrics.record_slashing(amount);
        }
    }

    /// Record inference result
    pub fn record_inference(&self) {
        if let Some(ref metrics) = self.economics_metrics {
            metrics.record_inference_result();
        }
    }

    /// Start new epoch (reset epoch-specific metrics)
    pub fn start_new_epoch(&mut self) {
        let epoch_duration = self.get_epoch_duration() as f64;
        let epoch_rewards = {
            let mut rewards = self.epoch_rewards.write();
            let current_rewards = *rewards;
            *rewards = 0; // Reset for new epoch
            current_rewards
        };

        if let Some(ref metrics) = self.economics_metrics {
            metrics.record_epoch_duration(epoch_duration);
            metrics.record_rewards_per_epoch(epoch_rewards as f64);
        }

        self.epoch_start_time = Instant::now();
    }
}