//! Parameter governance tests for DCF pallet

use crate::mock::*;
use frame_support::traits::Get;

/// Tests basic governance mode functionality
#[test]
fn governance_mode_basic_functionality_works() {
    new_test_ext().execute_with(|| {
        // Check initial governance mode
        let initial_mode = DcfPallet::governance_mode();
        assert!(!initial_mode); // Should be false by default
        
        // Test governance mode query consistency
        let mode_check = DcfPallet::governance_mode();
        assert_eq!(initial_mode, mode_check);
    });
}

/// Tests consensus weight parameter queries
#[test]
fn consensus_weight_parameters_work() {
    new_test_ext().execute_with(|| {
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        
        // Weights should be properly configured
        assert!(pos_weight > 0);
        assert!(poi_weight > 0);
        assert_eq!(pos_weight + poi_weight, 10000); // Should sum to 100%
        
        // Check against configured defaults
        let default_pos = <Test as crate::Config>::DefaultPosWeight::get();
        let default_poi = <Test as crate::Config>::DefaultPoiWeight::get();
        
        // Should match defaults initially
        assert_eq!(pos_weight, default_pos);
        assert_eq!(poi_weight, default_poi);
    });
}

/// Tests validator score parameter bounds
#[test]
fn validator_score_parameter_bounds_work() {
    new_test_ext().execute_with(|| {
        let min_score = <Test as crate::Config>::MinValidatorScore::get() as u64;
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        
        // Bounds should be logical
        assert!(min_score < max_score);
        assert!(min_score > 0);
        assert!(max_score > 100); // Should allow reasonable scoring range
        
        let active_validators = DcfPallet::active_validators();
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake_score(validator);
            let inference_score = DcfPallet::validator_inference_score(validator);
            
            // Scores should be within bounds
            assert!(stake_score >= min_score.into());
            assert!(stake_score <= max_score.into());
            assert!(inference_score >= 0); // Inference score can start at 0
            assert!(inference_score <= max_score);
        }
    });
}

/// Tests epoch configuration parameters
#[test]
fn epoch_configuration_parameters_work() {
    new_test_ext().execute_with(|| {
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let current_epoch = DcfPallet::current_epoch();
        
        // Epoch configuration should be reasonable
        assert!(epoch_length > 0);
        assert!(epoch_length < 1_000_000); // Not too large
        assert!(current_epoch >= 0);
        
        // Should match mock configuration
        assert_eq!(epoch_length, 2400);
    });
}

/// Tests validator set size parameters
#[test]
fn validator_set_size_parameters_work() {
    new_test_ext().execute_with(|| {
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        
        // Size parameters should be logical
        assert!(min_active > 0);
        assert!(max_validators >= min_active);
        assert!(max_validators <= 1000); // Reasonable upper bound
        
        let active_validators = DcfPallet::active_validators();
        assert!(active_validators.len() >= min_active as usize);
        assert!(active_validators.len() <= max_validators as usize);
    });
}

/// Tests economic parameters
#[test]
fn economic_parameters_work() {
    new_test_ext().execute_with(|| {
        let min_stake = <Test as crate::Config>::MinStake::get();
        let validator_reward: u128 = <Test as crate::Config>::ValidatorReward::get();
        let slash_percent: u32 = <Test as crate::Config>::SlashPercent::get();
        
        // Economic parameters should be reasonable
        assert!(min_stake > 0);
        assert!(validator_reward > 0);
        assert!(slash_percent > 0);
        assert!(slash_percent <= 100); // Can't slash more than 100%
        
        // Check against active validator stakes
        let active_validators = DcfPallet::active_validators();
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            assert!(stake >= min_stake);
        }
    });
}

/// Tests trust score configuration parameters
#[test]
fn trust_score_configuration_parameters_work() {
    new_test_ext().execute_with(|| {
        let uptime_weight: u64 = <Test as crate::Config>::TrustScoreUptimeWeight::get();
        let inference_weight: u64 = <Test as crate::Config>::TrustScoreInferenceWeight::get();
        let slashing_weight: u64 = <Test as crate::Config>::TrustScoreSlashingWeight::get();
        let max_trust_score: u64 = <Test as crate::Config>::MaxTrustScore::get();
        
        // Trust score weights should be configured
        assert!(uptime_weight > 0);
        assert!(inference_weight > 0);
        assert!(slashing_weight > 0);
        assert!(max_trust_score > 0);
        
        // Weights should sum to a reasonable total
        let total_weight = uptime_weight + inference_weight + slashing_weight;
        assert!(total_weight <= 10000); // Should not exceed 100%
    });
}

/// Tests performance threshold parameters
#[test]
fn performance_threshold_parameters_work() {
    new_test_ext().execute_with(|| {
        let min_perf_score: u64 = <Test as crate::Config>::MinPerformanceScore::get();
        let high_perf_score: u64 = <Test as crate::Config>::HighPerformanceScore::get();
        let min_participation: u32 = <Test as crate::Config>::MinParticipationRate::get();
        let high_participation: u32 = <Test as crate::Config>::HighParticipationRate::get();
        
        // Performance thresholds should be logical
        assert!(min_perf_score < high_perf_score);
        assert!(min_participation < high_participation);
        assert!(min_participation <= 100);
        assert!(high_participation <= 100);
        
        // Should be within reasonable ranges
        assert!(min_perf_score > 0);
        assert!(high_perf_score < 100);
        assert!(min_participation > 0);
    });
}

/// Tests cooldown period parameters
#[test]
fn cooldown_period_parameters_work() {
    new_test_ext().execute_with(|| {
        let leave_cooldown: u32 = <Test as crate::Config>::LeaveCooldown::get();
        let max_inactive_epochs: u32 = <Test as crate::Config>::MaxInactiveEpochs::get();
        
        // Cooldown parameters should be reasonable
        assert!(leave_cooldown > 0);
        assert!(leave_cooldown < 1_000_000); // Not too long
        assert!(max_inactive_epochs > 0);
        assert!(max_inactive_epochs < 100); // Reasonable inactivity limit
    });
}

/// Tests reward distribution parameters
#[test]
fn reward_distribution_parameters_work() {
    new_test_ext().execute_with(|| {
        let base_reward_pct: u32 = <Test as crate::Config>::BaseRewardPercentage::get();
        let performance_reward_pct: u32 = <Test as crate::Config>::PerformanceRewardPercentage::get();
        let top_performer_reward_pct: u32 = <Test as crate::Config>::TopPerformerRewardPercentage::get();
        
        // Reward percentages should be logical
        assert!(base_reward_pct > 0);
        assert!(performance_reward_pct > 0);
        assert!(top_performer_reward_pct > 0);
        
        // Should sum to 100% or less
        let total_pct = base_reward_pct + performance_reward_pct + top_performer_reward_pct;
        assert!(total_pct <= 100);
    });
}

/// Tests interval and timing parameters
#[test]
fn interval_timing_parameters_work() {
    new_test_ext().execute_with(|| {
        let score_decay_interval: u32 = <Test as crate::Config>::ScoreDecayInterval::get();
        let participation_update_interval: u32 = <Test as crate::Config>::ParticipationUpdateInterval::get();
        let health_metrics_interval: u32 = <Test as crate::Config>::HealthMetricsInterval::get();
        
        // Intervals should be positive
        assert!(score_decay_interval > 0);
        assert!(participation_update_interval > 0);
        assert!(health_metrics_interval > 0);
        
        // Should be reasonable relative to each other
        assert!(score_decay_interval <= participation_update_interval);
        assert!(participation_update_interval <= health_metrics_interval);
    });
}

/// Tests parameter consistency across configuration
#[test]
fn parameter_consistency_across_configuration_works() {
    new_test_ext().execute_with(|| {
        // Test that related parameters are consistent
        let min_stake = <Test as crate::Config>::MinStake::get();
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        
        // Logical relationships should hold
        assert!(min_active <= max_validators);
        assert!(min_stake > 0);
        assert!(epoch_length > 0);
        
        // Check against current state
        let current_validators = DcfPallet::active_validators();
        assert!(current_validators.len() >= min_active as usize);
        assert!(current_validators.len() <= max_validators as usize);
    });
}

/// Tests parameter bounds and limits
#[test]
fn parameter_bounds_and_limits_work() {
    new_test_ext().execute_with(|| {
        // Test various parameter bounds
        let percentage_precision: u32 = <Test as crate::Config>::PercentagePrecision::get();
        let max_validator_name_length: u32 = <Test as crate::Config>::MaxValidatorNameLength::get();
        let max_evidence_length: u32 = <Test as crate::Config>::MaxEvidenceLength::get();
        
        // Bounds should be reasonable
        assert!(percentage_precision > 0);
        assert!(percentage_precision >= 100); // Should allow percentage precision
        assert!(max_validator_name_length > 0);
        assert!(max_validator_name_length <= 1000); // Reasonable name length
        assert!(max_evidence_length > 0);
        assert!(max_evidence_length <= 10000); // Reasonable evidence size
    });
}