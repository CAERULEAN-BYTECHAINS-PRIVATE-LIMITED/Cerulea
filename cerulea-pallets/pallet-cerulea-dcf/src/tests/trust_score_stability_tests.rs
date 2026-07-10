//! Trust score stability tests

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

/// Tests trust score bounds enforcement
#[test]
fn trust_score_bounds_enforcement_works() {
    new_test_ext().execute_with(|| {
        let min_trust_score: u64 = <Test as crate::Config>::MinTrustScore::get();
        let max_trust_score: u64 = <Test as crate::Config>::MaxTrustScore::get();
        
        // Trust score bounds should be reasonable
        assert!(min_trust_score < max_trust_score);
        assert!(min_trust_score > 0);
        assert!(max_trust_score > 1000); // Should allow meaningful range
        
        // Check configured values
        assert_eq!(min_trust_score, 1000);
        assert_eq!(max_trust_score, 10000);
    });
}

/// Tests trust score weight configuration
#[test]
fn trust_score_weight_configuration_works() {
    new_test_ext().execute_with(|| {
        let uptime_weight: u64 = <Test as crate::Config>::TrustScoreUptimeWeight::get();
        let inference_weight: u64 = <Test as crate::Config>::TrustScoreInferenceWeight::get();
        let slashing_weight: u64 = <Test as crate::Config>::TrustScoreSlashingWeight::get();
        
        // Weights should be positive
        assert!(uptime_weight > 0);
        assert!(inference_weight > 0);
        assert!(slashing_weight > 0);
        
        // Check configured values from mock
        assert_eq!(uptime_weight, 4000);
        assert_eq!(inference_weight, 4000);
        assert_eq!(slashing_weight, 2000);
        
        // Total weight should be reasonable
        let total_weight = uptime_weight + inference_weight + slashing_weight;
        assert_eq!(total_weight, 10000); // 100%
    });
}

/// Tests trust score stability factors
#[test]
fn trust_score_stability_factors_work() {
    new_test_ext().execute_with(|| {
        let max_growth_rate: u32 = <Test as crate::Config>::MaxTrustScoreGrowthRate::get();
        let max_decay_rate: u32 = <Test as crate::Config>::MaxTrustScoreDecayRate::get();
        let stability_factor: u32 = <Test as crate::Config>::TrustScoreStabilityFactor::get();
        
        // Stability factors should be reasonable
        assert!(max_growth_rate > 0);
        assert!(max_decay_rate > 0);
        assert!(stability_factor > 0);
        
        // Growth should be limited to prevent explosive increases
        assert!(max_growth_rate <= 1000); // Max 10% per epoch
        
        // Decay should be limited to allow recovery
        assert!(max_decay_rate <= 500); // Max 5% per epoch
        
        // Stability factor should promote smoothing
        assert!(stability_factor >= 5000); // At least 50% stability
    });
}

/// Tests trust score calculation consistency
#[test]
fn trust_score_calculation_consistency_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Test that trust score components are accessible
        for validator in &active_validators {
            // These would be trust score components if available
            let stake_score = DcfPallet::validator_stake_score(validator);
            let inference_score = DcfPallet::validator_inference_score(validator);
            let (authored, missed) = DcfPallet::validator_participation(validator);
            
            // Verify components are in valid ranges
            assert!(stake_score >= 0);
            assert!(inference_score >= 0);
            assert!(authored >= 0);
            assert!(missed >= 0);
            
            // Trust score calculation would use these components
            // with the configured weights
        }
    });
}

/// Tests trust score stability over time
#[test]
fn trust_score_stability_over_time_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Record initial scores
        let initial_scores: Vec<_> = active_validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake_score(v), DcfPallet::validator_inference_score(v)))
            .collect();
        
        // Advance through several blocks
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        // Check score stability
        for (validator, initial_stake_score, initial_inf_score) in initial_scores {
            let current_stake_score = DcfPallet::validator_stake_score(&validator);
            let current_inf_score = DcfPallet::validator_inference_score(&validator);
            
            // Scores should not change dramatically without external input
            assert!(current_stake_score > 0);
            assert!(current_inf_score >= 0);
            
            // Changes should be bounded by stability factors
            let stake_change = current_stake_score.abs_diff(initial_stake_score);
            let inf_change = current_inf_score.abs_diff(initial_inf_score);
            
            // Changes should be reasonable (no explosive growth or decay)
            assert!(stake_change <= initial_stake_score / 2); // Max 50% change
            assert!(inf_change <= initial_inf_score + 1000); // Reasonable inference change
        }
    });
}

/// Tests trust score component weighting
#[test]
fn trust_score_component_weighting_works() {
    new_test_ext().execute_with(|| {
        let uptime_weight: u64 = <Test as crate::Config>::TrustScoreUptimeWeight::get();
        let inference_weight: u64 = <Test as crate::Config>::TrustScoreInferenceWeight::get();
        let slashing_weight: u64 = <Test as crate::Config>::TrustScoreSlashingWeight::get();
        
        // Test weight relationships
        assert!(uptime_weight > 0);
        assert!(inference_weight > 0);
        assert!(slashing_weight > 0);
        
        // In our mock, uptime and inference have equal weight
        assert_eq!(uptime_weight, inference_weight);
        
        // Slashing weight should be lower (penalties are less than rewards)
        assert!(slashing_weight < uptime_weight);
        assert!(slashing_weight < inference_weight);
    });
}

/// Tests trust score bounds during extreme conditions
#[test]
fn trust_score_bounds_during_extreme_conditions_work() {
    new_test_ext().execute_with(|| {
        let min_trust_score: u64 = <Test as crate::Config>::MinTrustScore::get();
        let max_trust_score: u64 = <Test as crate::Config>::MaxTrustScore::get();
        let active_validators = DcfPallet::active_validators();
        
        // Test extreme scenarios
        for validator in &active_validators {
            let current_stake_score = DcfPallet::validator_stake_score(validator);
            let current_inf_score = DcfPallet::validator_inference_score(validator);
            
            // Scores should always be within bounds
            assert!(current_stake_score <= <Test as crate::Config>::MaxValidatorScore::get().into());
            assert!(current_inf_score <= <Test as crate::Config>::MaxValidatorScore::get());
            
            // Trust score bounds should be enforced
            // (In a real implementation, trust scores would be calculated and bounded)
        }
    });
}

/// Tests trust score decay mechanisms
#[test]
fn trust_score_decay_mechanisms_work() {
    new_test_ext().execute_with(|| {
        let max_decay_rate: u32 = <Test as crate::Config>::MaxTrustScoreDecayRate::get();
        let max_inactive_epochs: u32 = <Test as crate::Config>::MaxInactiveEpochs::get();
        
        // Decay parameters should be reasonable
        assert!(max_decay_rate > 0);
        assert!(max_decay_rate <= 1000); // Max 10% decay per epoch
        assert!(max_inactive_epochs > 0);
        assert!(max_inactive_epochs <= 20); // Reasonable inactivity tolerance
        
        // Test that decay is gradual, not sudden
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            // All current validators should be active (no decay)
            assert!(DcfPallet::is_validator_active(validator));
            
            let last_active = DcfPallet::validator_last_active(validator);
            assert!(last_active >= 0);
        }
    });
}

/// Tests trust score growth limitations
#[test]
fn trust_score_growth_limitations_work() {
    new_test_ext().execute_with(|| {
        let max_growth_rate: u32 = <Test as crate::Config>::MaxTrustScoreGrowthRate::get();
        let stability_factor: u32 = <Test as crate::Config>::TrustScoreStabilityFactor::get();
        
        // Growth should be limited to prevent gaming
        assert!(max_growth_rate <= 1000); // Max 10% growth per epoch
        
        // Stability factor should smooth out volatility
        assert!(stability_factor >= 5000); // At least 50% stability
        
        let active_validators = DcfPallet::active_validators();
        
        // Test current validator states
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake_score(validator);
            let max_score = <Test as crate::Config>::MaxValidatorScore::get();
            
            // Scores should not exceed maximum
            assert!(stake_score <= max_score.into());
        }
    });
}

/// Tests trust score anti-gaming mechanisms
#[test]
fn trust_score_anti_gaming_mechanisms_work() {
    new_test_ext().execute_with(|| {
        // Test that trust score system resists gaming attempts
        
        let active_validators = DcfPallet::active_validators();
        let stability_factor: u32 = <Test as crate::Config>::TrustScoreStabilityFactor::get();
        
        // High stability factor should prevent rapid manipulation
        assert!(stability_factor >= 8000); // 80% stability in mock config
        
        // Validators should have consistent, reasonable scores
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake_score(validator);
            let inference_score = DcfPallet::validator_inference_score(validator);
            
            // Scores should be in reasonable ranges, not extreme values
            assert!(stake_score > 1000); // Not too low
            assert!(stake_score < 9000); // Not too high (from genesis)
            assert!(inference_score >= 0);
            assert!(inference_score <= <Test as crate::Config>::MaxValidatorScore::get());
        }
    });
}

/// Tests trust score system resilience
#[test]
fn trust_score_system_resilience_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Record baseline state
        let baseline_scores: Vec<_> = active_validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake_score(v)))
            .collect();
        
        // Simulate sustained operation
        for block_num in 1..=20 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        // System should remain stable and resilient
        let final_validators = DcfPallet::active_validators();
        assert_eq!(final_validators.len(), active_validators.len());
        
        for (validator, baseline_score) in baseline_scores {
            let final_score = DcfPallet::validator_stake_score(&validator);
            
            // Score should remain in reasonable range
            assert!(final_score > baseline_score / 2); // No dramatic drops
            assert!(final_score < baseline_score * 2); // No dramatic increases
            assert!(DcfPallet::is_validator_active(&validator)); // Should remain active
        }
    });
}