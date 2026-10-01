//! Epoch management implementation
//!
//! This module provides epoch transition logic and validator set management
//! that works in conjunction with the DCF runtime pallet.

use crate::{
    error::{ConsensusError, ConsensusResult},
    types::{EpochConfig, ValidatorInfo},
};
use cerulea_runtime::AccountId;
use log::{debug, error, info, warn};
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::NumberFor;
use sp_runtime::traits::{Block as BlockTrait, SaturatedConversion};
use std::sync::Arc;

/// Policy constants from the runtime
#[derive(Debug, Clone)]
pub struct EpochManagerRuntimeConfig {
    /// Minimum score for good performance
    pub min_performance_score: u64,
    /// Score for high performance rewards
    pub high_performance_score: u64,
    /// Minimum participation rate percentage
    pub min_participation_rate: u32,
    /// High participation rate percentage
    pub high_participation_rate: u32,
    /// Maximum missed blocks before penalty
    pub max_missed_blocks: u32,
    /// Maximum missed blocks for high performers
    pub max_missed_blocks_high: u32,
    /// Score threshold for healthy validator
    pub healthy_validator_score: u64,
    /// Participation rate for healthy validator
    pub healthy_participation_rate: u32,
    /// Max missed blocks for healthy validator
    pub healthy_missed_blocks_max: u32,
    /// Cooldown period in blocks after leaving
    pub leave_cooldown: u32,
    /// Number of top validators to display in logs
    pub top_validators_display_count: u32,
}

/// One proposal the epoch boundary raises about one validator.
///
/// The Admission Engine reads these proposals at the epoch boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatorManagementProposal {
    /// The validator the proposal names.
    pub validator: AccountId,
    /// One sentence stating why the proposal was raised.
    pub reason: String,
    /// The combined standing score of the validator.
    pub combined_score: u64,
    /// The stake part of the score.
    pub stake_score: u64,
    /// The measured contribution part of the score.
    pub contribution_score: u64,
}

/// Epoch manager for handling epoch transitions and validator set updates
pub struct EpochManager<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    client: Arc<C>,
    epoch_config: EpochConfig,
    _phantom: std::marker::PhantomData<B>,
}

impl<B, C> EpochManager<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    /// Create a new epoch manager
    pub fn new(client: Arc<C>, epoch_config: EpochConfig) -> Self {
        Self {
            client,
            epoch_config,
            _phantom: std::marker::PhantomData,
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
            _ => {
                debug!("Failed to calculate uptime for validator {:?}", validator);
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
                    100 // New validators get 100% participation initially
                };
                (participation_rate, missed)
            }
            Err(e) => {
                debug!(
                    "Failed to get participation metrics for validator {:?}: {:?}",
                    validator, e
                );
                (0u32, 0u32)
            }
        }
    }

    /// Get runtime policy constants
    fn get_runtime_config(&self) -> ConsensusResult<EpochManagerRuntimeConfig> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let config_tuple = api.get_epoch_manager_config(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get epoch manager config: {:?}", e))
        })?;

        Ok(EpochManagerRuntimeConfig {
            min_performance_score: config_tuple.0,
            high_performance_score: config_tuple.1,
            min_participation_rate: config_tuple.2,
            high_participation_rate: config_tuple.3,
            max_missed_blocks: config_tuple.4,
            max_missed_blocks_high: config_tuple.5,
            healthy_validator_score: config_tuple.6,
            healthy_participation_rate: config_tuple.7,
            healthy_missed_blocks_max: config_tuple.8,
            leave_cooldown: config_tuple.9,
            top_validators_display_count: config_tuple.10,
        })
    }

    /// Check if an epoch transition should occur
    pub fn should_transition_epoch(&self, current_block: u32) -> ConsensusResult<bool> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let current_epoch = api.get_current_epoch(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e))
        })?;

        // Get epoch length from runtime policy (T::EpochLength)
        let epoch_length = api.get_epoch_length(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get epoch length: {:?}", e))
        })?;

        let epoch_start_block = current_epoch.saturating_mul(epoch_length);
        let should_transition = current_block >= epoch_start_block + epoch_length;

        if should_transition {
            info!("DCF EpochManager: Epoch transition needed at block {} (epoch {} -> {}, length: {})", 
                  current_block, current_epoch, current_epoch + 1, epoch_length);
        }

        Ok(should_transition)
    }

    /// Handle epoch transition logic
    pub fn handle_epoch_transition(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let current_epoch = api.get_current_epoch(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e))
        })?;

        // 1. Apply validator score decay for inactive validators
        self.apply_validator_score_decay(current_epoch)?;

        // 2. Update active validator set based on scores and stake
        self.update_active_validator_set()?;

        // 3. Handle validator join/leave requests
        self.process_validator_set_changes()?;

        // 4. Update validator participation rates
        self.update_validator_participation_rates()?;

        // 5. Check for underperforming validators
        self.handle_underperforming_validators()?;

        info!("DCF EpochManager: Epoch transition completed successfully");
        Ok(())
    }

    /// Apply score decay to inactive validators
    fn apply_validator_score_decay(&self, current_epoch: u32) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let all_validators = api.get_validator_scores(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e))
        })?;

        let mut decayed_count = 0;

        for (validator, _score) in all_validators {
            let last_active = api
                .get_validator_last_active(best_hash, validator.clone())
                .map_err(|e| {
                    ConsensusError::EpochTransition(format!(
                        "Failed to get validator last active: {:?}",
                        e
                    ))
                })?;

            let inactive_epochs = current_epoch.saturating_sub(last_active);

            if inactive_epochs > 0 {
                // Score decay is handled by the runtime pallet automatically
                info!(
                    "DCF EpochManager: Validator {:?} inactive for {} epochs",
                    validator, inactive_epochs
                );
                decayed_count += 1;
            }
        }

        if decayed_count > 0 {
            info!(
                "DCF EpochManager: Applied score decay to {} inactive validators",
                decayed_count
            );
        }

        Ok(())
    }

    /// Update the active validator set based on current scores and stake
    fn update_active_validator_set(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let current_active = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        let all_scores = api.get_validator_scores(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e))
        })?;

        // Filter validators that meet minimum requirements
        let mut eligible_validators: Vec<(AccountId, u64)> = Vec::new();

        // Get current block number for cooldown check
        let current_block = self.client.info().best_number.saturated_into::<u32>();

        // Get runtime policy
        let config = self.get_runtime_config()?;

        for (validator, score) in all_scores {
            // Check minimum score requirement
            if score >= config.healthy_validator_score {
                // Check if validator is in cooldown period after leaving
                match api.get_validator_leave_request(best_hash, validator.clone()) {
                    Ok(Some(leave_block)) => {
                        let blocks_passed = current_block.saturating_sub(leave_block);
                        let cooldown_period = config.leave_cooldown;

                        if blocks_passed < cooldown_period {
                            // Validator is still in cooldown period, skip
                            debug!("DCF EpochManager: Validator {:?} still in cooldown period ({} blocks remaining)", 
                                   validator, cooldown_period.saturating_sub(blocks_passed));
                            continue;
                        }
                    }
                    Ok(None) => {
                        // No leave request, validator is eligible
                    }
                    Err(e) => {
                        warn!("DCF EpochManager: Failed to check leave request for validator {:?}: {:?}", validator, e);
                        // Continue with validator to avoid blocking the epoch transition
                    }
                }

                // Check if validator has sufficient stake (this was checked via PoS pallet)
                eligible_validators.push((validator, score));
            }
        }

        // Sort by score (descending) and take top validators up to max limit
        eligible_validators.sort_by(|a, b| b.1.cmp(&a.1));

        let max_validators = self.epoch_config.max_validators as usize;
        if eligible_validators.len() > max_validators {
            eligible_validators.truncate(max_validators);
        }

        let new_active_count = eligible_validators.len();
        let old_active_count = current_active.len();

        info!(
            "DCF EpochManager: Validator set update - {} -> {} active validators",
            old_active_count, new_active_count
        );
        Ok(())
    }

    /// Process validator join/leave requests
    fn process_validator_set_changes(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        // Validate minimum validator count
        if active_validators.len() < self.epoch_config.min_validators as usize {
            warn!(
                "DCF EpochManager: Active validator count ({}) below minimum ({})",
                active_validators.len(),
                self.epoch_config.min_validators
            );
        }

        // Validate maximum validator count
        if active_validators.len() > self.epoch_config.max_validators as usize {
            error!(
                "DCF EpochManager: Active validator count ({}) exceeds maximum ({})",
                active_validators.len(),
                self.epoch_config.max_validators
            );
        }

        info!(
            "DCF EpochManager: Processed validator set changes - {} active validators",
            active_validators.len()
        );

        Ok(())
    }

    /// Update validator participation rates
    fn update_validator_participation_rates(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        let mut total_participation = 0u32;
        let mut validator_count = 0u32;

        for validator in active_validators {
            if let Ok((authored, missed)) =
                api.get_validator_participation(best_hash, validator.clone())
            {
                let total_blocks = authored + missed;
                let participation_rate = if total_blocks > 0 {
                    (authored * 100) / total_blocks
                } else {
                    100 // New validators get full participation initially
                };

                total_participation += participation_rate;
                validator_count += 1;

                if participation_rate < config.healthy_participation_rate {
                    warn!(
                        "DCF EpochManager: Low participation rate for validator {:?}: {}%",
                        validator, participation_rate
                    );
                }
            }
        }

        let average_participation = if validator_count > 0 {
            total_participation / validator_count
        } else {
            0
        };

        info!(
            "DCF EpochManager: Average validator participation rate: {}%",
            average_participation
        );

        Ok(())
    }

    /// Handle underperforming validators
    fn handle_underperforming_validators(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        let mut underperforming_count = 0;

        for validator in active_validators {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let _trust_score = profile.trust_score;
                let _inference_count = profile.inference_count;
                let (participation_rate, missed_blocks) =
                    self.get_validator_participation_metrics(&validator);

                // Check for underperformance criteria
                let is_underperforming = combined_score < config.min_performance_score
                    || participation_rate < config.min_participation_rate
                    || missed_blocks > config.max_missed_blocks;

                if is_underperforming {
                    warn!("DCF EpochManager: Underperforming validator detected: {:?} - Combined Score: {}, Participation: {}%, Missed: {}", 
                          validator, combined_score, participation_rate, missed_blocks);
                    underperforming_count += 1;

                    // The runtime pallet will handle automatic ejection if score falls below threshold
                }
            }
        }

        if underperforming_count > 0 {
            info!(
                "DCF EpochManager: Found {} underperforming validators",
                underperforming_count
            );
        }

        Ok(())
    }

    /// Get current epoch information
    pub fn get_current_epoch_info(&self) -> ConsensusResult<(u32, Vec<AccountId>)> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        let current_epoch = api.get_current_epoch(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e))
        })?;

        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        Ok((current_epoch, active_validators))
    }

    /// Generate validator management proposals from the combined standing score
    pub fn generate_validator_management_proposals(
        &self,
    ) -> ConsensusResult<Vec<ValidatorManagementProposal>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;
        let mut proposals = Vec::new();

        // Get active validators and their scores
        let active_validators = api.get_active_validators(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e))
        })?;

        let (_pos_weight, _poi_weight) = api.get_consensus_weights(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get consensus weights: {:?}", e))
        })?;

        for validator in active_validators.iter() {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let pos_score = self.get_pos_score(validator);
                let poi_score = profile.poi_score as u64;
                let _inference_count = profile.inference_count;
                let (participation_rate, missed_blocks) =
                    self.get_validator_participation_metrics(validator);

                let mut proposal_reason = String::new();

                // Check for underperformance
                if combined_score < config.min_performance_score {
                    proposal_reason = format!("Low combined score: {}", combined_score);
                } else if participation_rate < config.min_participation_rate {
                    proposal_reason = format!("Low participation: {}%", participation_rate);
                } else if missed_blocks > config.max_missed_blocks_high {
                    proposal_reason = format!("Too many missed blocks: {}", missed_blocks);
                } else if pos_score == 0u64 && poi_score == 0u64 {
                    proposal_reason = "No PoS or PoI contribution".to_string();
                } else if combined_score >= config.high_performance_score {
                    proposal_reason =
                        format!("High performance reward candidate: {}", combined_score);
                }

                if !proposal_reason.is_empty() {
                    proposals.push(ValidatorManagementProposal {
                        validator: validator.clone(),
                        reason: proposal_reason,
                        combined_score,
                        stake_score: pos_score,
                        contribution_score: poi_score,
                    });
                }
            }
        }

        info!(
            "EpochManager: Generated {} validator management proposals",
            proposals.len()
        );
        Ok(proposals)
    }

    /// Update validator set based on combined PoS and PoI scores
    pub fn update_validator_set_by_combined_scores(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        // Get all validators and their combined scores
        let all_validators = api.get_validator_scores(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e))
        })?;

        let (pos_weight, poi_weight) = api.get_consensus_weights(best_hash).map_err(|e| {
            ConsensusError::EpochTransition(format!("Failed to get consensus weights: {:?}", e))
        })?;

        let mut validator_performance: Vec<(AccountId, u64, u64, u64)> = Vec::new(); // (validator, combined, pos, poi)

        for (validator, _stored_score) in all_validators {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let pos_score = self.get_pos_score(&validator);
                let poi_score = profile.poi_score as u64;
                validator_performance.push((validator, combined_score, pos_score, poi_score));
            }
        }

        // Sort by combined score (descending)
        validator_performance.sort_by(|a, b| b.1.cmp(&a.1));

        // Get runtime policy for display count
        let config = self.get_runtime_config()?;

        // Log top performers
        info!("EpochManager: Top validators by combined PoS/contribution score:");
        for (i, (validator, combined, pos, poi)) in validator_performance
            .iter()
            .take(config.top_validators_display_count as usize)
            .enumerate()
        {
            info!(
                "  {}. {:?} - Combined: {}, PoS: {} ({}%), PoI: {} ({}%)",
                i + 1,
                validator,
                combined,
                pos,
                pos_weight,
                poi,
                poi_weight
            );
        }

        Ok(())
    }

    /// Get validator information for a specific validator
    pub fn get_validator_info(
        &self,
        validator: &AccountId,
    ) -> ConsensusResult<Option<ValidatorInfo>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;

        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
            let combined_score = profile.final_score;
            let pos_score = self.get_pos_score(validator);
            let uptime = self.calculate_validator_uptime(validator);
            let (_, missed_blocks) = self.get_validator_participation_metrics(validator);

            let stake_score = pos_score; // Use the fresh PoS score from the profile

            let validator_info = ValidatorInfo {
                account_id: sp_core::ed25519::Public::from_raw(*validator.as_ref()),
                stake: stake_score as u128,
                performance_score: combined_score as u32,
                blocks_produced: uptime, // Using uptime as blocks produced approximation
                blocks_missed: missed_blocks,
            };

            Ok(Some(validator_info))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod coverage_tests {
    //! The epoch manager, run against the test chain.
    //!
    //! The epoch manager reads the framework interface and reports what it
    //! finds through its answers and its log lines. Each test fills the
    //! runtime answers, runs one call and reads both back.

    use super::*;
    use crate::test_support::{capture_logs, Block, TestChain, Voter};
    use pallet_cerulea_dcf::{UptimeStats, ValidatorProfile, ValidatorStatus};

    type Manager = EpochManager<Block, TestChain>;

    /// The policy tuple the runtime answers.
    ///
    /// Minimum score 100, high score 900, minimum participation 50 percent,
    /// high participation 90 percent, five missed blocks, three missed blocks
    /// for high performers, healthy score 300, healthy participation 80
    /// percent, four missed blocks for a healthy validator, a cooldown of ten
    /// blocks and two validators in the top list.
    const POLICY: (u64, u64, u32, u32, u32, u32, u64, u32, u32, u32, u32) =
        (100, 900, 50, 90, 5, 3, 300, 80, 4, 10, 2);

    fn account(seed: u8) -> AccountId {
        Voter::from_seed(seed).account
    }

    fn profile(final_score: u64, contribution: u32) -> ValidatorProfile<AccountId, u128, u32> {
        ValidatorProfile {
            stake: 1_000,
            poi_score: contribution,
            final_score,
            trust_score: 0,
            status: ValidatorStatus::Active,
            inference_count: 0,
            name: None,
            last_active_block: 0,
            _phantom: Default::default(),
        }
    }

    fn manager(chain: &Arc<TestChain>, min_validators: u32, max_validators: u32) -> Manager {
        Manager::new(
            chain.clone(),
            EpochConfig {
                epoch_length: 10,
                min_validators,
                max_validators,
                min_stake: 0,
            },
        )
    }

    /// Five validators at epoch four on a chain twelve blocks high.
    fn transition_chain() -> (Arc<TestChain>, [AccountId; 5]) {
        let chain = TestChain::with_blocks(12);
        let set = [
            account(11),
            account(12),
            account(13),
            account(14),
            account(15),
        ];
        let [one, two, three, four, five] = set.clone();
        let answers = &chain.answers;
        answers.set("DcfApi_get_current_epoch", (), 4u32);
        answers.set("DcfApi_get_epoch_length", (), 10u32);
        answers.set(
            "DcfApi_get_validator_scores",
            (),
            vec![
                (one.clone(), 950u64),
                (two.clone(), 400u64),
                (three.clone(), 200u64),
                (four.clone(), 600u64),
                (five.clone(), 500u64),
            ],
        );
        for (validator, last_active) in
            [(&one, 4u32), (&two, 2), (&three, 1), (&four, 4), (&five, 4)]
        {
            answers.set(
                "DcfApi_get_validator_last_active",
                validator.clone(),
                last_active,
            );
        }
        answers.set("DcfApi_get_active_validators", (), set.to_vec());
        answers.set("DcfApi_get_epoch_manager_config", (), POLICY);
        // The first validator has no leave request answer at all.
        answers.set(
            "DcfApi_get_validator_leave_request",
            two.clone(),
            Some(8u32),
        );
        answers.set(
            "DcfApi_get_validator_leave_request",
            four.clone(),
            Some(1u32),
        );
        answers.set(
            "DcfApi_get_validator_leave_request",
            five.clone(),
            None::<u32>,
        );
        answers.set(
            "DcfApi_get_validator_participation",
            one.clone(),
            (95u32, 5u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            two.clone(),
            (0u32, 0u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            three.clone(),
            (30u32, 70u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            five.clone(),
            (80u32, 20u32),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            one.clone(),
            Some(profile(950, 1)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            two.clone(),
            Some(profile(400, 1)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            three.clone(),
            Some(profile(50, 1)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            five.clone(),
            Some(profile(500, 1)),
        );
        (chain, set)
    }

    #[test]
    fn an_epoch_transition_reads_every_validator_and_reports_each_finding() {
        let (chain, [_one, two, three, _four, five]) = transition_chain();
        let logs = capture_logs();

        manager(&chain, 6, 2)
            .handle_epoch_transition()
            .expect("the transition completes");

        assert!(logs.has(&format!("Validator {:?} inactive for 2 epochs", two)));
        assert!(logs.has(&format!("Validator {:?} inactive for 3 epochs", three)));
        assert!(logs.has("Applied score decay to 2 inactive validators"));
        assert!(logs.has("Failed to check leave request for validator"));
        assert!(logs.has("still in cooldown period (6 blocks remaining)"));
        assert!(logs.has("Validator set update - 5 -> 2 active validators"));
        assert!(logs.has("Active validator count (5) below minimum (6)"));
        assert!(logs.has("Active validator count (5) exceeds maximum (2)"));
        assert!(logs.has(&format!(
            "Low participation rate for validator {:?}: 30%",
            three
        )));
        assert!(logs.has("Average validator participation rate: 76%"));
        assert!(logs.has(&format!("Underperforming validator detected: {:?}", three)));
        assert!(logs.has(&format!("Underperforming validator detected: {:?}", five)));
        assert!(logs.has("Found 2 underperforming validators"));
        assert!(logs.has("Epoch transition completed successfully"));
    }

    #[test]
    fn a_set_inside_its_bounds_raises_no_size_warning() {
        let (chain, _) = transition_chain();
        let logs = capture_logs();

        manager(&chain, 1, 10)
            .handle_epoch_transition()
            .expect("the transition completes");

        assert!(!logs.has("below minimum"));
        assert!(!logs.has("exceeds maximum"));
        assert!(logs.has("Validator set update - 5 -> 3 active validators"));
    }

    #[test]
    fn an_epoch_transition_names_each_missing_runtime_answer() {
        let cases = [
            ("DcfApi_get_current_epoch", "Failed to get current epoch: "),
            (
                "DcfApi_get_validator_scores",
                "Failed to get validator scores: ",
            ),
            (
                "DcfApi_get_validator_last_active",
                "Failed to get validator last active: ",
            ),
            (
                "DcfApi_get_active_validators",
                "Failed to get active validators: ",
            ),
            (
                "DcfApi_get_epoch_manager_config",
                "Failed to get epoch manager config: ",
            ),
        ];
        for (call, opening) in cases {
            let (chain, _) = transition_chain();
            chain.answers.forget(call);
            match manager(&chain, 1, 10).handle_epoch_transition() {
                Err(ConsensusError::EpochTransition(reason)) => {
                    assert!(reason.starts_with(opening), "{call}: {reason}")
                }
                other => panic!("{call}: the transition answered {:?}", other),
            }
        }
    }

    #[test]
    fn a_transition_falls_due_once_the_epoch_runs_out() {
        let (chain, _) = transition_chain();
        let logs = capture_logs();
        let manager = manager(&chain, 1, 10);

        assert!(!manager.should_transition_epoch(49).unwrap());
        assert!(!logs.has("Epoch transition needed"));
        assert!(manager.should_transition_epoch(50).unwrap());
        assert!(logs.has("Epoch transition needed at block 50 (epoch 4 -> 5, length: 10)"));

        chain.answers.forget("DcfApi_get_epoch_length");
        let reason = manager
            .should_transition_epoch(50)
            .expect_err("the epoch length is unavailable")
            .to_string();
        assert!(reason.starts_with("Epoch transition failed: Failed to get epoch length: "));

        chain.answers.forget("DcfApi_get_current_epoch");
        let reason = manager
            .should_transition_epoch(50)
            .expect_err("the epoch is unavailable")
            .to_string();
        assert!(reason.starts_with("Epoch transition failed: Failed to get current epoch: "));
    }

    #[test]
    fn the_epoch_information_carries_the_active_set() {
        let (chain, set) = transition_chain();
        let manager = manager(&chain, 1, 10);

        assert_eq!(manager.get_current_epoch_info().unwrap(), (4, set.to_vec()));

        chain.answers.forget("DcfApi_get_active_validators");
        let reason = manager
            .get_current_epoch_info()
            .expect_err("the active set is unavailable")
            .to_string();
        assert!(reason.starts_with("Epoch transition failed: Failed to get active validators: "));

        chain.answers.forget("DcfApi_get_current_epoch");
        let reason = manager
            .get_current_epoch_info()
            .expect_err("the epoch is unavailable")
            .to_string();
        assert!(reason.starts_with("Epoch transition failed: Failed to get current epoch: "));
    }

    /// Seven validators, one for each reason the proposal list knows.
    fn proposal_chain() -> (Arc<TestChain>, [AccountId; 7]) {
        let chain = TestChain::with_blocks(12);
        let set = [
            account(21),
            account(22),
            account(23),
            account(24),
            account(25),
            account(26),
            account(27),
        ];
        let [low, absent, missing, idle, strong, steady, unknown] = set.clone();
        let answers = &chain.answers;
        answers.set("DcfApi_get_epoch_manager_config", (), POLICY);
        answers.set("DcfApi_get_active_validators", (), set.to_vec());
        answers.set("DcfApi_get_consensus_weights", (), (40u64, 60u64));
        answers.set(
            "DcfApi_get_validator_scores",
            (),
            vec![
                (steady.clone(), 1u64),
                (low.clone(), 1u64),
                (strong.clone(), 1u64),
                (unknown.clone(), 1u64),
            ],
        );
        answers.set(
            "DcfApi_get_validator_profile",
            low.clone(),
            Some(profile(50, 0)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            absent.clone(),
            Some(profile(500, 3)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            missing.clone(),
            Some(profile(500, 4)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            idle.clone(),
            Some(profile(500, 0)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            strong.clone(),
            Some(profile(950, 7)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            steady.clone(),
            Some(profile(500, 2)),
        );
        answers.set(
            "DcfApi_get_validator_profile",
            unknown.clone(),
            None::<ValidatorProfile<AccountId, u128, u32>>,
        );
        answers.set(
            "DcfApi_get_validator_participation",
            absent.clone(),
            (40u32, 60u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            missing.clone(),
            (96u32, 4u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            idle.clone(),
            (100u32, 0u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            strong.clone(),
            (100u32, 0u32),
        );
        answers.set(
            "DcfApi_get_validator_participation",
            steady.clone(),
            (100u32, 0u32),
        );
        answers.set("DcfApi_get_validator_stake_score", idle.clone(), 0u64);
        answers.set("DcfApi_get_validator_stake_score", strong.clone(), 10u64);
        answers.set("DcfApi_get_validator_stake_score", steady.clone(), 10u64);
        (chain, set)
    }

    #[test]
    fn each_proposal_names_the_reason_the_scores_give() {
        let (chain, [low, absent, missing, idle, strong, _steady, _unknown]) = proposal_chain();

        let proposals = manager(&chain, 1, 10)
            .generate_validator_management_proposals()
            .expect("the proposals build");

        let expected = vec![
            ValidatorManagementProposal {
                validator: low,
                reason: "Low combined score: 50".to_string(),
                combined_score: 50,
                stake_score: 0,
                contribution_score: 0,
            },
            ValidatorManagementProposal {
                validator: absent,
                reason: "Low participation: 40%".to_string(),
                combined_score: 500,
                stake_score: 0,
                contribution_score: 3,
            },
            ValidatorManagementProposal {
                validator: missing,
                reason: "Too many missed blocks: 4".to_string(),
                combined_score: 500,
                stake_score: 0,
                contribution_score: 4,
            },
            ValidatorManagementProposal {
                validator: idle,
                reason: "No PoS or PoI contribution".to_string(),
                combined_score: 500,
                stake_score: 0,
                contribution_score: 0,
            },
            ValidatorManagementProposal {
                validator: strong,
                reason: "High performance reward candidate: 950".to_string(),
                combined_score: 950,
                stake_score: 10,
                contribution_score: 7,
            },
        ];
        assert_eq!(proposals, expected);
    }

    #[test]
    fn the_proposal_list_names_each_missing_runtime_answer() {
        let cases = [
            (
                "DcfApi_get_epoch_manager_config",
                "Failed to get epoch manager config: ",
            ),
            (
                "DcfApi_get_active_validators",
                "Failed to get active validators: ",
            ),
            (
                "DcfApi_get_consensus_weights",
                "Failed to get consensus weights: ",
            ),
        ];
        for (call, opening) in cases {
            let (chain, _) = proposal_chain();
            chain.answers.forget(call);
            match manager(&chain, 1, 10).generate_validator_management_proposals() {
                Err(ConsensusError::EpochTransition(reason)) => {
                    assert!(reason.starts_with(opening), "{call}: {reason}")
                }
                other => panic!("{call}: the proposals answered {:?}", other),
            }
        }
    }

    #[test]
    fn the_top_list_shows_the_strongest_validators_first() {
        let (chain, [_low, _absent, _missing, _idle, strong, steady, _unknown]) = proposal_chain();
        let logs = capture_logs();

        manager(&chain, 1, 10)
            .update_validator_set_by_combined_scores()
            .expect("the list builds");

        let lines = logs.lines();
        let first = format!(
            "  1. {:?} - Combined: 950, PoS: 10 (40%), PoI: 7 (60%)",
            strong
        );
        let second = format!(
            "  2. {:?} - Combined: 500, PoS: 10 (40%), PoI: 2 (60%)",
            steady
        );
        let first_at = lines.iter().position(|line| *line == first);
        let second_at = lines.iter().position(|line| *line == second);
        assert!(first_at.is_some(), "{:?}", lines);
        assert!(first_at < second_at);
        // The policy shows two validators.
        assert!(!lines.iter().any(|line| line.starts_with("  3. ")));
    }

    #[test]
    fn the_top_list_names_each_missing_runtime_answer() {
        let cases = [
            (
                "DcfApi_get_validator_scores",
                "Failed to get validator scores: ",
            ),
            (
                "DcfApi_get_consensus_weights",
                "Failed to get consensus weights: ",
            ),
            (
                "DcfApi_get_epoch_manager_config",
                "Failed to get epoch manager config: ",
            ),
        ];
        for (call, opening) in cases {
            let (chain, _) = proposal_chain();
            chain.answers.forget(call);
            match manager(&chain, 1, 10).update_validator_set_by_combined_scores() {
                Err(ConsensusError::EpochTransition(reason)) => {
                    assert!(reason.starts_with(opening), "{call}: {reason}")
                }
                other => panic!("{call}: the list answered {:?}", other),
            }
        }
    }

    #[test]
    fn validator_information_joins_profile_stake_uptime_and_missed_blocks() {
        let (chain, [low, _absent, _missing, _idle, strong, _steady, unknown]) = proposal_chain();
        chain.answers.set(
            "DcfApi_get_validator_uptime",
            strong.clone(),
            Some(UptimeStats {
                total_epochs: 3,
                blocks_authored: 30,
                blocks_missed: 2,
                uptime_percentage: 9_000,
                participation_rate: 9_500,
            }),
        );
        chain.answers.set(
            "DcfApi_get_validator_participation",
            strong.clone(),
            (90u32, 6u32),
        );
        let manager = manager(&chain, 1, 10);

        let info = manager
            .get_validator_info(&strong)
            .unwrap()
            .expect("the validator carries a profile");
        let strong_bytes: [u8; 32] = *strong.as_ref();
        assert_eq!(
            info.account_id,
            sp_core::ed25519::Public::from_raw(strong_bytes)
        );
        assert_eq!(info.stake, 10);
        assert_eq!(info.performance_score, 950);
        assert_eq!(info.blocks_produced, 9_500);
        assert_eq!(info.blocks_missed, 6);

        // A validator without uptime or participation answers reads as zero.
        let quiet = manager
            .get_validator_info(&low)
            .unwrap()
            .expect("the validator carries a profile");
        assert_eq!(quiet.stake, 0);
        assert_eq!(quiet.performance_score, 50);
        assert_eq!(quiet.blocks_produced, 0);
        assert_eq!(quiet.blocks_missed, 0);

        assert!(manager.get_validator_info(&unknown).unwrap().is_none());
        assert!(manager.get_validator_info(&account(99)).unwrap().is_none());
    }

    #[test]
    fn the_runtime_policy_reads_back_field_by_field() {
        let (chain, _) = proposal_chain();
        let policy = manager(&chain, 1, 10)
            .get_runtime_config()
            .expect("the policy reads");
        assert_eq!(policy.min_performance_score, 100);
        assert_eq!(policy.high_performance_score, 900);
        assert_eq!(policy.min_participation_rate, 50);
        assert_eq!(policy.high_participation_rate, 90);
        assert_eq!(policy.max_missed_blocks, 5);
        assert_eq!(policy.max_missed_blocks_high, 3);
        assert_eq!(policy.healthy_validator_score, 300);
        assert_eq!(policy.healthy_participation_rate, 80);
        assert_eq!(policy.healthy_missed_blocks_max, 4);
        assert_eq!(policy.leave_cooldown, 10);
        assert_eq!(policy.top_validators_display_count, 2);
    }
}
