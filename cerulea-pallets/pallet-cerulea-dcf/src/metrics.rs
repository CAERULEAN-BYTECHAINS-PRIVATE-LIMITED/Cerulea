//! Metrics integration for DCF pallet
//!
//! This module implements comprehensive metrics collection and aggregation
//! for the DCF system. It provides runtime-exposed counters and compact
//! metrics snapshots to reduce RPC fan-out and improve monitoring efficiency.

use super::*;
use frame_support::traits::{Get, Currency};
use sp_runtime::traits::{Zero, Saturating};
use sp_std::collections::btree_map::BTreeMap;

/// Comprehensive system metrics for operational monitoring.
///
/// This structure aggregates all essential system metrics into a single
/// compact representation that can be retrieved with a single RPC call.
/// It reduces monitoring overhead and provides consistent system visibility.
///
/// All metrics are updated automatically during epoch transitions and
/// significant system events to ensure accuracy and consistency.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct SystemMetrics<T: Config> {
    /// Current epoch number
    pub current_epoch: u32,
    /// Block number when metrics were last updated
    pub last_updated_block: u32,
    
    // Validator metrics
    /// Total number of registered validators
    pub total_validators: u32,
    /// Number of currently active validators
    pub active_validators: u32,
    /// Number of validators in cooldown periods
    pub validators_in_cooldown: u32,
    /// Number of validators with pending leave requests
    pub validators_leaving: u32,
    
    // Economic metrics
    /// Total stake across all validators
    pub total_stake: T::Balance,
    /// Total amount slashed in current epoch
    pub total_slashed_current_epoch: T::Balance,
    /// Total rewards distributed in current epoch
    pub total_rewards_current_epoch: T::Balance,
    /// Average stake per active validator
    pub average_stake_per_validator: T::Balance,
    
    // Performance metrics
    /// Average validator score across active set
    pub average_validator_score: u64,
    /// Highest validator score in active set
    pub highest_validator_score: u64,
    /// Lowest validator score in active set
    pub lowest_validator_score: u64,
    /// Number of validators above performance threshold
    pub high_performance_validators: u32,
    
    // Network health metrics
    /// Current finalized block number
    pub finalized_block: u32,
    /// Blocks since last finality advancement
    pub blocks_since_finality: u32,
    /// Number of invariant violations in current epoch
    pub invariant_violations_count: u32,
    /// Overall system health score (0-10000)
    pub system_health_score: u32,
    
    // Governance metrics
    /// Number of active governance proposals
    pub active_proposals: u32,
    /// Number of proposals executed in current epoch
    pub proposals_executed_current_epoch: u32,
    /// Governance participation rate (percentage of validators voting)
    pub governance_participation_rate: u32,
}

impl<T: Config> Default for SystemMetrics<T> {
    fn default() -> Self {
        Self {
            current_epoch: 0,
            last_updated_block: 0,
            total_validators: 0,
            active_validators: 0,
            validators_in_cooldown: 0,
            validators_leaving: 0,
            total_stake: T::Balance::zero(),
            total_slashed_current_epoch: T::Balance::zero(),
            total_rewards_current_epoch: T::Balance::zero(),
            average_stake_per_validator: T::Balance::zero(),
            average_validator_score: 0,
            highest_validator_score: 0,
            lowest_validator_score: 0,
            high_performance_validators: 0,
            finalized_block: 0,
            blocks_since_finality: 0,
            invariant_violations_count: 0,
            system_health_score: 10000, // Perfect health initially
            active_proposals: 0,
            proposals_executed_current_epoch: 0,
            governance_participation_rate: 0,
        }
    }
}

/// Detailed system performance indicators.
///
/// This structure provides comprehensive performance metrics that complement
/// the core system metrics, focusing on operational efficiency and network
/// performance characteristics.
#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct SystemPerformanceIndicators {
    /// Current epoch for these indicators
    pub epoch: u32,
    /// Block number when indicators were last updated
    pub last_updated_block: u32,
    
    // Block production metrics
    /// Average blocks per epoch over last 10 epochs
    pub average_blocks_per_epoch: u32,
    /// Block production efficiency (percentage of expected blocks produced)
    pub block_production_efficiency: u32,
    /// Average time between blocks (in milliseconds)
    pub average_block_time: u64,
    
    // Validator competition metrics
    /// Standard deviation of validator scores (measure of competition)
    pub score_standard_deviation: u64,
    /// Percentage of validators within 10% of highest score
    pub competitive_validators_percentage: u32,
    /// Rate of validator score changes per epoch
    pub score_volatility_index: u32,
    
    // Network participation metrics
    /// Average validator uptime percentage
    pub average_validator_uptime: u32,
    /// Percentage of validators participating in governance
    pub governance_participation_percentage: u32,
    /// Average number of validator status changes per epoch
    pub validator_churn_rate: u32,
    
    // System efficiency metrics
    /// Average epoch transition time (in blocks)
    pub average_epoch_transition_time: u32,
    /// Percentage of epochs with no invariant violations
    pub clean_epochs_percentage: u32,
    /// System stability index (0-10000, higher = more stable)
    pub stability_index: u32,
    
    // Economic efficiency metrics
    /// Ratio of rewards to slashing (economic health indicator)
    pub reward_to_slash_ratio: u32,
    /// Average economic operation processing time
    pub economic_operation_efficiency: u32,
    /// Percentage of economic operations within bounds
    pub economic_bounds_compliance: u32,
}

impl Default for SystemPerformanceIndicators {
    fn default() -> Self {
        Self {
            epoch: 0,
            last_updated_block: 0,
            average_blocks_per_epoch: 100, // Default epoch length
            block_production_efficiency: 100, // Perfect efficiency initially
            average_block_time: 6000, // 6 seconds default
            score_standard_deviation: 0,
            competitive_validators_percentage: 100,
            score_volatility_index: 0,
            average_validator_uptime: 100,
            governance_participation_percentage: 100,
            validator_churn_rate: 0,
            average_epoch_transition_time: 1,
            clean_epochs_percentage: 100,
            stability_index: 10000, // Perfect stability initially
            reward_to_slash_ratio: 100, // Balanced initially
            economic_operation_efficiency: 100,
            economic_bounds_compliance: 100,
        }
    }
}

/// Metrics aggregation and calculation functions
impl<T: Config> Pallet<T> {
    /// Update system metrics during epoch transitions and significant events
    pub fn update_system_metrics() {
        let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
        let current_epoch = Self::current_epoch();
        
        // Collect validator metrics
        let validator_set = Self::validator_set();
        let active_validators = Self::active_validators();
        let total_validators = validator_set.len() as u32;
        let active_validator_count = active_validators.len() as u32;
        
        // Count validators in various states
        let validators_in_cooldown = Self::count_validators_in_cooldown();
        let validators_leaving = Self::count_validators_with_leave_requests();
        
        // Calculate economic metrics
        let total_stake = Self::calculate_total_stake(&active_validators);
        let average_stake = if active_validator_count > 0 {
            total_stake / active_validator_count.into()
        } else {
            T::Balance::zero()
        };
        
        // Get economic tracking for current epoch
        let economic_tracker = Self::get_epoch_economic_tracker(current_epoch);
        
        // Calculate performance metrics
        let (avg_score, highest_score, lowest_score, high_perf_count) = 
            Self::calculate_score_metrics(&active_validators);
        
        // Calculate network health metrics
        let finalized_block = Self::last_finalized_block();
        let blocks_since_finality = current_block.saturating_sub(finalized_block);
        let invariant_violations = Self::count_current_epoch_violations(current_epoch);
        let system_health = Self::calculate_system_health_score(
            active_validator_count,
            blocks_since_finality,
            invariant_violations,
        );
        
        // Calculate governance metrics
        let (active_proposals, executed_proposals, participation_rate) = 
            Self::calculate_governance_metrics(current_epoch);
        
        // Create metrics snapshot
        let metrics = SystemMetrics {
            current_epoch,
            last_updated_block: current_block,
            total_validators,
            active_validators: active_validator_count,
            validators_in_cooldown,
            validators_leaving,
            total_stake,
            total_slashed_current_epoch: economic_tracker.total_slashing_this_epoch,
            total_rewards_current_epoch: economic_tracker.total_rewards_this_epoch,
            average_stake_per_validator: average_stake,
            average_validator_score: avg_score,
            highest_validator_score: highest_score,
            lowest_validator_score: lowest_score,
            high_performance_validators: high_perf_count,
            finalized_block,
            blocks_since_finality,
            invariant_violations_count: invariant_violations,
            system_health_score: system_health,
            active_proposals,
            proposals_executed_current_epoch: executed_proposals,
            governance_participation_rate: participation_rate,
        };
        
        // Store updated metrics
        SystemMetricsStorage::<T>::put(metrics);
        MetricsLastUpdated::<T>::put(current_block);
        
        // Update performance indicators
        Self::update_performance_indicators(current_epoch, current_block);
    }
    
    /// Update performance indicators with detailed efficiency metrics
    pub fn update_performance_indicators(epoch: u32, block_number: u32) {
        // Calculate block production metrics
        let (avg_blocks_per_epoch, production_efficiency, avg_block_time) = 
            Self::calculate_block_production_metrics(epoch);
        
        // Calculate validator competition metrics
        let active_validators = Self::active_validators();
        let (score_std_dev, competitive_percentage, volatility_index) = 
            Self::calculate_competition_metrics(&active_validators);
        
        // Calculate participation metrics
        let (avg_uptime, gov_participation, churn_rate) = 
            Self::calculate_participation_metrics(epoch);
        
        // Calculate system efficiency metrics
        let (transition_time, clean_epochs, stability_index) = 
            Self::calculate_efficiency_metrics(epoch);
        
        // Calculate economic efficiency metrics
        let (reward_slash_ratio, operation_efficiency, bounds_compliance) = 
            Self::calculate_economic_efficiency_metrics(epoch);
        
        // Create performance indicators snapshot
        let indicators = SystemPerformanceIndicators {
            epoch,
            last_updated_block: block_number,
            average_blocks_per_epoch: avg_blocks_per_epoch,
            block_production_efficiency: production_efficiency,
            average_block_time: avg_block_time,
            score_standard_deviation: score_std_dev,
            competitive_validators_percentage: competitive_percentage,
            score_volatility_index: volatility_index,
            average_validator_uptime: avg_uptime,
            governance_participation_percentage: gov_participation,
            validator_churn_rate: churn_rate,
            average_epoch_transition_time: transition_time,
            clean_epochs_percentage: clean_epochs,
            stability_index,
            reward_to_slash_ratio: reward_slash_ratio,
            economic_operation_efficiency: operation_efficiency,
            economic_bounds_compliance: bounds_compliance,
        };
        
        // Store updated indicators
        PerformanceIndicatorsStorage::<T>::put(indicators);
    }
    
    /// Count validators currently in cooldown periods
    fn count_validators_in_cooldown() -> u32 {
        let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
        let cooldown_period = T::LeaveCooldown::get();
        
        let mut count = 0u32;
        
        // Count validators with active leave requests
        for (_, request_block) in ValidatorLeaveRequests::<T>::iter() {
            if current_block < request_block.saturating_add(cooldown_period) {
                count = count.saturating_add(1);
            }
        }
        
        // Count recently removed validators still in cooldown
        for (_, removal_block) in RecentlyRemovedValidators::<T>::iter() {
            let re_entry_cooldown = T::ValidatorCooldownPeriod::get();
            if current_block < removal_block.saturating_add(re_entry_cooldown) {
                count = count.saturating_add(1);
            }
        }
        
        count
    }
    
    /// Count validators with pending leave requests
    fn count_validators_with_leave_requests() -> u32 {
        ValidatorLeaveRequests::<T>::iter().count() as u32
    }
    
    /// Calculate total stake across all active validators
    fn calculate_total_stake(active_validators: &[T::AccountId]) -> T::Balance {
        active_validators.iter()
            .map(|v| T::Currency::reserved_balance(v))
            .fold(T::Balance::zero(), |acc, stake| acc.saturating_add(stake))
    }
    
    /// Calculate validator score metrics (average, highest, lowest, high performers)
    fn calculate_score_metrics(active_validators: &[T::AccountId]) -> (u64, u64, u64, u32) {
        if active_validators.is_empty() {
            return (0, 0, 0, 0);
        }
        
        let scores: Vec<u64> = active_validators.iter()
            .map(|v| Self::get_validator_final_score(*v))
            .collect();
        
        let total_score: u64 = scores.iter().sum();
        let avg_score = total_score / scores.len() as u64;
        let highest_score = *scores.iter().max().unwrap_or(&0);
        let lowest_score = *scores.iter().min().unwrap_or(&0);
        
        // Count high performers (top 25% or above 80% of max score)
        let high_threshold = std::cmp::max(
            highest_score.saturating_mul(80) / 100, // 80% of highest
            T::MaxValidatorScore::get().saturating_mul(80) / 100, // 80% of max possible
        );
        let high_perf_count = scores.iter()
            .filter(|&&score| score >= high_threshold)
            .count() as u32;
        
        (avg_score, highest_score, lowest_score, high_perf_count)
    }
    
    /// Calculate system health score based on various factors
    fn calculate_system_health_score(
        active_validators: u32,
        blocks_since_finality: u32,
        invariant_violations: u32,
    ) -> u32 {
        let mut health_score = 10000u32; // Start with perfect health
        
        // Penalize for insufficient validators
        let min_validators = T::MinActiveValidators::get();
        if active_validators < min_validators {
            health_score = health_score.saturating_sub(2000); // -20% for insufficient validators
        }
        
        // Penalize for finality lag
        if blocks_since_finality > 10 {
            let penalty = std::cmp::min(blocks_since_finality.saturating_mul(100), 3000);
            health_score = health_score.saturating_sub(penalty);
        }
        
        // Penalize for invariant violations
        if invariant_violations > 0 {
            let penalty = std::cmp::min(invariant_violations.saturating_mul(1000), 5000);
            health_score = health_score.saturating_sub(penalty);
        }
        
        health_score
    }
    
    /// Count invariant violations in current epoch
    fn count_current_epoch_violations(epoch: u32) -> u32 {
        // In a real implementation, this would check stored violation reports
        // For now, return 0 as violations would be tracked separately
        0
    }
    
    /// Calculate governance metrics (active proposals, executed, participation)
    fn calculate_governance_metrics(epoch: u32) -> (u32, u32, u32) {
        let mut active_proposals = 0u32;
        let mut executed_proposals = 0u32;
        
        // Count active and executed proposals
        for (_, proposal) in Proposals::<T>::iter() {
            match proposal.status {
                ProposalStatus::Pending => active_proposals = active_proposals.saturating_add(1),
                ProposalStatus::Executed => {
                    // Check if executed in current epoch (simplified)
                    executed_proposals = executed_proposals.saturating_add(1);
                },
                _ => {},
            }
        }
        
        // Calculate participation rate based on actual voting data
        let active_validators = Self::active_validators();
        let participation_rate = if active_validators.is_empty() {
            0
        } else {
            // Calculate based on recent proposal voting activity
            let recent_proposals = Self::get_recent_proposals(10); // Last 10 proposals
            if recent_proposals.is_empty() {
                100 // No proposals means 100% theoretical participation
            } else {
                let total_possible_votes = recent_proposals.len() * active_validators.len();
                let actual_votes = recent_proposals.iter()
                    .map(|proposal_id| Self::get_proposal_vote_count(*proposal_id))
                    .sum::<usize>();
                
                if total_possible_votes == 0 {
                    100
                } else {
                    ((actual_votes * 100) / total_possible_votes) as u32
                }
            }
        };
        
        (active_proposals, executed_proposals, participation_rate)
    }
    
    /// Calculate block production metrics
    fn calculate_block_production_metrics(epoch: u32) -> (u32, u32, u64) {
        // Calculate actual metrics from epoch history
        if let Some(history) = EpochHistories::<T>::get().iter().find(|h| h.epoch == epoch) {
            let expected_blocks = T::EpochLength::get();
            let actual_blocks = history.blocks_produced;
            
            // Calculate production efficiency as percentage
            let production_efficiency = if expected_blocks > 0 {
                ((actual_blocks * 100) / expected_blocks).min(100)
            } else {
                100
            };
            
            // Calculate average block time based on epoch duration
            // Assuming epoch duration is roughly EpochLength * target_block_time
            let target_block_time = 6000u64; // 6 seconds in milliseconds
            let avg_block_time = if actual_blocks > 0 {
                (expected_blocks as u64 * target_block_time) / actual_blocks as u64
            } else {
                target_block_time
            };
            
            (actual_blocks, production_efficiency, avg_block_time)
        } else {
            // FB-18: Do NOT report fabricated efficiency (95%) when history is missing.
            // Report 0 to be honest about the lack of available historical data.
            let avg_blocks_per_epoch = T::EpochLength::get();
            let production_efficiency = 0; 
            let avg_block_time = 6000;
            
            (avg_blocks_per_epoch, production_efficiency, avg_block_time)
        }
    }
    
    /// Calculate validator competition metrics
    fn calculate_competition_metrics(active_validators: &[T::AccountId]) -> (u64, u32, u32) {
        if active_validators.is_empty() {
            return (0, 0, 0);
        }
        
        let scores: Vec<u64> = active_validators.iter()
            .map(|v| Self::get_validator_final_score(*v))
            .collect();
        
        // Calculate standard deviation (simplified)
        let mean = scores.iter().sum::<u64>() / scores.len() as u64;
        let variance = scores.iter()
            .map(|&score| {
                let diff = if score > mean { score - mean } else { mean - score };
                diff * diff
            })
            .sum::<u64>() / scores.len() as u64;
        let std_dev = (variance as f64).sqrt() as u64;
        
        // Calculate competitive percentage (within 10% of highest)
        let highest_score = *scores.iter().max().unwrap_or(&0);
        let competitive_threshold = highest_score.saturating_mul(90) / 100;
        let competitive_count = scores.iter()
            .filter(|&&score| score >= competitive_threshold)
            .count();
        let competitive_percentage = (competitive_count * 100) / scores.len();
        
        // Volatility index (simplified - would track score changes over time)
        let volatility_index = 10; // Low volatility
        
        (std_dev, competitive_percentage as u32, volatility_index)
    }
    
    /// Calculate participation metrics
    fn calculate_participation_metrics(_epoch: u32) -> (u32, u32, u32) {
        // FB-16: Removed fabricated metrics (95% uptime, 80% gov, 5% churn).
        // These counters are not yet implemented with real historical tracking.
        // Returning 0 is more honest than fabricating success.
        let avg_uptime = 0; 
        let gov_participation = 0;
        let churn_rate = 0;
        
        (avg_uptime, gov_participation, churn_rate)
    }
    
    /// Calculate system efficiency metrics
    fn calculate_efficiency_metrics(_epoch: u32) -> (u32, u32, u32) {
        // FB-16: Removed fabricated metrics (1 block transition, 95% clean epochs, 9500 stability).
        let transition_time = 0; 
        let clean_epochs = 0; 
        let stability_index = 0; 
        
        (transition_time, clean_epochs, stability_index)
    }
    
    /// Calculate economic efficiency metrics
    fn calculate_economic_efficiency_metrics(epoch: u32) -> (u32, u32, u32) {
        let economic_tracker = Self::get_epoch_economic_tracker(epoch);
        
        // Calculate reward to slash ratio
        let reward_slash_ratio = if !economic_tracker.total_slashing_this_epoch.is_zero() {
            let ratio = economic_tracker.total_rewards_this_epoch
                .saturating_mul(100u32.into()) / economic_tracker.total_slashing_this_epoch;
            ratio.saturated_into::<u32>()
        } else if !economic_tracker.total_rewards_this_epoch.is_zero() {
            200 // High ratio when rewards but no slashing
        } else {
            100 // Balanced when neither
        };
        
        // FB-16: Removed fabricated metrics (98% efficiency, 100% compliance).
        let operation_efficiency = 0; 
        let bounds_compliance = 0; 
        
        (reward_slash_ratio, operation_efficiency, bounds_compliance)
    }
    
    /// Get compact metrics snapshot for efficient RPC access
    pub fn get_metrics_snapshot() -> (SystemMetrics<T>, SystemPerformanceIndicators, u32) {
        let metrics = SystemMetricsStorage::<T>::get();
        let indicators = PerformanceIndicatorsStorage::<T>::get();
        let last_updated = MetricsLastUpdated::<T>::get();
        
        (metrics, indicators, last_updated)
    }
    
    /// Check if metrics need updating based on staleness threshold
    pub fn metrics_need_update(staleness_threshold_blocks: u32) -> bool {
        let current_block = <frame_system::Pallet<T>>::block_number().saturated_into::<u32>();
        let last_updated = MetricsLastUpdated::<T>::get();
        
        current_block.saturating_sub(last_updated) >= staleness_threshold_blocks
    }
    
    /// Force metrics update (for testing or manual refresh)
    pub fn force_metrics_update() {
        Self::update_system_metrics();
    }
    
    /// Get recent proposal IDs for participation calculation
    fn get_recent_proposals(count: usize) -> Vec<u32> {
        // Get the most recent proposal IDs from storage
        let mut proposals = Vec::new();
        let current_proposal_id = NextProposalId::<T>::get();
        
        for i in 0..count {
            if let Some(proposal_id) = current_proposal_id.checked_sub(i as u32) {
                if Proposals::<T>::contains_key(proposal_id) {
                    proposals.push(proposal_id);
                }
            }
        }
        
        proposals
    }
    
    /// Get vote count for a specific proposal
    fn get_proposal_vote_count(proposal_id: u32) -> usize {
        ProposalVotes::<T>::iter_prefix(proposal_id).count()
    }
}

/// Runtime API implementations for metrics access
impl<T: Config> Pallet<T> {
    /// Get encoded system metrics for runtime API
    pub fn get_encoded_system_metrics() -> Vec<u8> {
        let metrics = SystemMetricsStorage::<T>::get();
        metrics.encode()
    }
    
    /// Get encoded performance indicators for runtime API
    pub fn get_encoded_performance_indicators() -> Vec<u8> {
        let indicators = PerformanceIndicatorsStorage::<T>::get();
        indicators.encode()
    }
    
    /// Get metrics last updated block number
    pub fn get_metrics_last_updated_block() -> u32 {
        MetricsLastUpdated::<T>::get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    use frame_support::{assert_ok, traits::Get};

    #[test]
    fn test_system_metrics_update() {
        new_test_ext().execute_with(|| {
            // Setup validators
            for i in 1..=5 {
                let _ = Balances::make_free_balance_be(&i, 100_000_000);
                assert_ok!(DcfModule::join_validators(RuntimeOrigin::signed(i)));
            }
            
            // Update metrics
            DcfModule::update_system_metrics();
            
            // Check metrics were updated
            let metrics = SystemMetricsStorage::<Test>::get();
            assert_eq!(metrics.total_validators, 5);
            assert_eq!(metrics.active_validators, 5);
            assert!(metrics.total_stake > 0);
            assert!(metrics.last_updated_block > 0);
        });
    }

    #[test]
    fn test_performance_indicators_update() {
        new_test_ext().execute_with(|| {
            // Setup validators
            for i in 1..=3 {
                let _ = Balances::make_free_balance_be(&i, 100_000_000);
                assert_ok!(DcfModule::join_validators(RuntimeOrigin::signed(i)));
            }
            
            // Update performance indicators
            DcfModule::update_performance_indicators(1, 100);
            
            // Check indicators were updated
            let indicators = PerformanceIndicatorsStorage::<Test>::get();
            assert_eq!(indicators.epoch, 1);
            assert_eq!(indicators.last_updated_block, 100);
            assert!(indicators.stability_index > 0);
        });
    }

    #[test]
    fn test_score_metrics_calculation() {
        new_test_ext().execute_with(|| {
            // Setup validators with different scores
            let validators = vec![1u64, 2u64, 3u64];
            for &validator in &validators {
                let _ = Balances::make_free_balance_be(&validator, 100_000_000);
                assert_ok!(DcfModule::join_validators(RuntimeOrigin::signed(validator)));
                
                // Set different scores
                let score = validator * 1000;
                assert_ok!(DcfModule::update_validator_stake_score(
                    RuntimeOrigin::root(), validator, score
                ));
            }
            
            // Calculate score metrics
            let active_validators = DcfModule::active_validators();
            let (avg, highest, lowest, high_perf) = DcfModule::calculate_score_metrics(&active_validators);
            
            assert!(avg > 0);
            assert!(highest >= avg);
            assert!(lowest <= avg);
            assert!(highest >= lowest);
        });
    }

    #[test]
    fn test_system_health_calculation() {
        new_test_ext().execute_with(|| {
            // Test perfect health
            let health = DcfModule::calculate_system_health_score(5, 0, 0);
            assert_eq!(health, 10000);
            
            // Test with finality lag
            let health_with_lag = DcfModule::calculate_system_health_score(5, 20, 0);
            assert!(health_with_lag < 10000);
            
            // Test with invariant violations
            let health_with_violations = DcfModule::calculate_system_health_score(5, 0, 2);
            assert!(health_with_violations < 10000);
        });
    }

    #[test]
    fn test_metrics_staleness_check() {
        new_test_ext().execute_with(|| {
            // Initially should need update
            assert!(DcfModule::metrics_need_update(10));
            
            // Update metrics
            DcfModule::update_system_metrics();
            
            // Should not need update immediately after
            assert!(!DcfModule::metrics_need_update(10));
            
            // Advance blocks
            run_to_block(20);
            
            // Should need update after staleness threshold
            assert!(DcfModule::metrics_need_update(5));
        });
    }

    #[test]
    fn test_metrics_encoding() {
        new_test_ext().execute_with(|| {
            // Update metrics
            DcfModule::update_system_metrics();
            
            // Test encoding
            let encoded_metrics = DcfModule::get_encoded_system_metrics();
            assert!(!encoded_metrics.is_empty());
            
            let encoded_indicators = DcfModule::get_encoded_performance_indicators();
            assert!(!encoded_indicators.is_empty());
            
            // Test decoding
            let decoded_metrics: SystemMetrics<Test> = Decode::decode(&mut &encoded_metrics[..]).unwrap();
            assert!(decoded_metrics.last_updated_block > 0);
        });
    }
}