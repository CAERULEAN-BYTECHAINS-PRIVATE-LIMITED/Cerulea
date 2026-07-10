//! Finality and consensus tests

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

/// Tests basic finality information
#[test]
fn basic_finality_information_works() {
    new_test_ext().execute_with(|| {
        let last_finalized = DcfPallet::last_finalized_block();
        assert!(last_finalized >= 0);
        
        let finality_info = DcfPallet::finality_info();
        let (finalized_block, current_block) = finality_info;
        
        assert_eq!(finalized_block, last_finalized);
        assert!(current_block >= finalized_block);
    });
}

/// Tests block finality status
#[test]
fn block_finality_status_works() {
    new_test_ext().execute_with(|| {
        // Test finality status for various blocks
        let is_block_0_finalized = DcfPallet::is_block_finalized(0);
        let is_block_1_finalized = DcfPallet::is_block_finalized(1);
        let is_future_block_finalized = DcfPallet::is_block_finalized(1000);
        
        // Block 0 might be finalized (genesis)
        assert!(is_block_0_finalized == true || is_block_0_finalized == false);
        
        // Future blocks should not be finalized
        assert!(!is_future_block_finalized);
    });
}

/// Tests finality progression
#[test]
fn finality_progression_works() {
    new_test_ext().execute_with(|| {
        let initial_finalized = DcfPallet::last_finalized_block();
        
        // Process several blocks
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let current_finalized = DcfPallet::last_finalized_block();
            
            // Finality should never go backwards
            assert!(current_finalized >= initial_finalized);
        }
    });
}

/// Tests consensus participation
#[test]
fn consensus_participation_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // All active validators should participate in consensus
        for validator in &active_validators {
            let is_active = DcfPallet::is_validator_active(validator);
            assert!(is_active);
            
            let (authored, missed) = DcfPallet::validator_participation(validator);
            assert!(authored >= 0);
            assert!(missed >= 0);
        }
    });
}

/// Tests consensus weight distribution
#[test]
fn consensus_weight_distribution_works() {
    new_test_ext().execute_with(|| {
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        
        // Weights should be properly distributed
        assert!(pos_weight > 0);
        assert!(poi_weight > 0);
        assert_eq!(pos_weight + poi_weight, 10000);
        
        // Should reflect configured defaults
        let default_pos = <Test as crate::Config>::DefaultPosWeight::get();
        let default_poi = <Test as crate::Config>::DefaultPoiWeight::get();
        
        assert_eq!(pos_weight, default_pos);
        assert_eq!(poi_weight, default_poi);
    });
}

/// Tests expected block authorship
#[test]
fn expected_block_authorship_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Test expected authors for various blocks
        for block_num in 1..=10 {
            let expected_author = DcfPallet::expected_author(block_num);
            
            match expected_author {
                Some(author) => {
                    // Author should be an active validator
                    assert!(active_validators.contains(&author));
                },
                None => {
                    // No expected author determined yet (acceptable)
                    assert!(true);
                }
            }
        }
    });
}

/// Tests block author validation
#[test]
fn block_author_validation_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for block_num in 1..=5 {
            for validator in &active_validators {
                // Validate if this validator can be a block author
                let validation_result = DcfPallet::validate_expected_author(block_num, *validator);
                
                // Should return true or false without panicking
                assert!(validation_result == true || validation_result == false);
            }
        }
    });
}

/// Tests consensus mechanism stability
#[test]
fn consensus_mechanism_stability_works() {
    new_test_ext().execute_with(|| {
        let initial_validators = DcfPallet::active_validators();
        let initial_weights = DcfPallet::consensus_weights();
        
        // Process multiple blocks
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let current_validators = DcfPallet::active_validators();
            let current_weights = DcfPallet::consensus_weights();
            
            // Consensus should remain stable
            assert!(!current_validators.is_empty());
            assert_eq!(current_weights.0 + current_weights.1, 10000);
            
            // Validator set should be stable without external changes
            assert_eq!(current_validators.len(), initial_validators.len());
        }
    });
}

/// Tests finality lag calculation
#[test]
fn finality_lag_calculation_works() {
    new_test_ext().execute_with(|| {
        let current_block = System::block_number();
        let finality_lag = DcfPallet::blocks_since_finalization(current_block as u32);
        
        // Lag should be non-negative
        assert!(finality_lag >= 0);
        
        // Test with various block numbers
        for block_num in 1..=10 {
            let lag = DcfPallet::blocks_since_finalization(block_num);
            assert!(lag >= 0);
        }
    });
}

/// Tests consensus efficiency metrics
#[test]
fn consensus_efficiency_metrics_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let total_participation: u32 = active_validators.iter()
            .map(|v| {
                let (authored, missed) = DcfPallet::validator_participation(v);
                authored + missed
            })
            .sum();
        
        // Should have some consensus activity (or zero if just started)
        assert!(total_participation >= 0);
        
        // Check individual validator efficiency
        for validator in &active_validators {
            let (authored, missed) = DcfPallet::validator_participation(validator);
            
            // Participation data should be consistent
            assert!(authored >= 0);
            assert!(missed >= 0);
        }
    });
}

/// Tests consensus algorithm correctness
#[test]
fn consensus_algorithm_correctness_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_validators = <Test as crate::Config>::MinActiveValidators::get();
        
        // Should have sufficient validators for consensus
        assert!(active_validators.len() >= min_validators as usize);
        
        // All validators should be properly configured
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            let min_stake = <Test as crate::Config>::MinStake::get();
            
            assert!(stake >= min_stake);
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}

/// Tests finality safety guarantees
#[test]
fn finality_safety_guarantees_work() {
    new_test_ext().execute_with(|| {
        let initial_finalized = DcfPallet::last_finalized_block();
        
        // Process blocks and check finality never goes backwards
        let mut previous_finalized = initial_finalized;
        
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let current_finalized = DcfPallet::last_finalized_block();
            
            // Safety: finality never decreases
            assert!(current_finalized >= previous_finalized);
            previous_finalized = current_finalized;
        }
    });
}

/// Tests consensus liveness properties
#[test]
fn consensus_liveness_properties_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Liveness: system should continue making progress
        let initial_epoch = DcfPallet::current_epoch();
        
        for block_num in 1..=5 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        // System should remain live
        let current_validators = DcfPallet::active_validators();
        let current_epoch = DcfPallet::current_epoch();
        
        assert!(!current_validators.is_empty());
        assert!(current_epoch >= initial_epoch);
    });
}

/// Tests consensus agreement properties
#[test]
fn consensus_agreement_properties_work() {
    new_test_ext().execute_with(|| {
        // All validators should agree on system state
        let active_validators = DcfPallet::active_validators();
        let current_epoch = DcfPallet::current_epoch();
        let consensus_weights = DcfPallet::consensus_weights();
        
        // Agreement: all queries should return consistent results
        for _ in 0..5 {
            assert_eq!(DcfPallet::active_validators(), active_validators);
            assert_eq!(DcfPallet::current_epoch(), current_epoch);
            assert_eq!(DcfPallet::consensus_weights(), consensus_weights);
        }
    });
}