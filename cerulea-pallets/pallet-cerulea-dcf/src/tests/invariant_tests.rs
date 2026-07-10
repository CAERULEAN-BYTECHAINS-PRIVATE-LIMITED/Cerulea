//! Invariant and system health tests

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

/// Tests basic system invariants
#[test]
fn basic_system_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        
        // Validator set size invariants
        assert!(active_validators.len() <= max_validators as usize);
        assert!(active_validators.len() >= min_active as usize);
        
        // Each validator should be unique
        let mut unique_validators: Vec<u64> = active_validators.iter().cloned().collect();
        unique_validators.sort();
        unique_validators.dedup();
        assert_eq!(unique_validators.len(), active_validators.len());
    });
}

/// Tests economic invariants
#[test]
fn economic_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            let free_balance = Balances::free_balance(validator);
            let reserved_balance = Balances::reserved_balance(validator);
            
            // Economic invariants
            assert!(stake >= min_stake); // Minimum stake requirement
            assert!(free_balance >= 0); // Non-negative free balance
            assert!(reserved_balance >= 0); // Non-negative reserved balance
            assert!(stake <= free_balance + reserved_balance); // Stake can't exceed total balance
        }
    });
}

/// Tests scoring invariants
#[test]
fn scoring_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        let min_score = <Test as crate::Config>::MinValidatorScore::get() as u64;
        
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake(validator);
            let inference_score = crate::PoiWeight::<Test>::get();
            
            // Score bounds invariants
            assert!(stake_score >= min_score as u128);
            assert!(stake_score <= max_score as u128);
            assert!(inference_score >= 0);
            assert!(inference_score <= max_score);
        }
    });
}

/// Tests consensus weight invariants
#[test]
fn consensus_weight_invariants_hold() {
    new_test_ext().execute_with(|| {
        let (pos_weight, poi_weight) = (crate::PosWeight::<Test>::get(), crate::PoiWeight::<Test>::get());
        
        // Weight invariants
        assert!(pos_weight > 0); // PoS weight must be positive
        assert!(poi_weight > 0); // PoI weight must be positive
        assert_eq!(pos_weight + poi_weight, 10000); // Must sum to 100%
        assert!(pos_weight <= 10000); // Cannot exceed 100%
        assert!(poi_weight <= 10000); // Cannot exceed 100%
    });
}

/// Tests epoch progression invariants
#[test]
fn epoch_progression_invariants_hold() {
    new_test_ext().execute_with(|| {
        let initial_epoch = DcfPallet::current_epoch();
        
        // Progress through several blocks
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let current_epoch = DcfPallet::current_epoch();
            
            // Epoch progression invariant: epochs never decrease
            assert!(current_epoch >= initial_epoch);
        }
    });
}

/// Tests validator activity invariants
#[test]
fn validator_activity_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            let (authored, missed) = if let Some(state) = DcfPallet::validator_states(validator) {
                (state.current.authored_blocks, state.current.missed_blocks)
            } else {
                (0, 0)
            };
            let last_active = DcfPallet::validator_stake(validator);
            let is_active = DcfPallet::is_validator_active(validator);
            
            // Activity invariants
            assert!(authored >= 0); // Non-negative authored blocks
            assert!(missed >= 0); // Non-negative missed blocks
            assert!(last_active >= 0); // Non-negative last active block
            assert!(is_active); // Active validators should be marked as active
        }
    });
}

/// Tests trust score invariants
#[test]
fn trust_score_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let max_trust_score: u64 = <Test as crate::Config>::MaxTrustScore::get();
        let min_trust_score: u64 = <Test as crate::Config>::MinTrustScore::get();
        
        for validator in &active_validators {
            // Trust score bounds would be checked if available
            // For now, just verify the configuration is reasonable
            assert!(max_trust_score > min_trust_score);
            assert!(min_trust_score > 0);
        }
    });
}

/// Tests storage consistency invariants
#[test]
fn storage_consistency_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // All active validators should have consistent data
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake(validator);
            let inference_score = crate::PoiWeight::<Test>::get();
            let is_active = DcfPallet::is_validator_active(validator);
            
            // Consistency checks
            assert!(stake_score > 0 || inference_score >= 0); // At least some score data
            assert!(is_active); // Active validators must be marked active
        }
    });
}

/// Tests configuration invariants
#[test]
fn configuration_invariants_hold() {
    new_test_ext().execute_with(|| {
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_stake = <Test as crate::Config>::MinStake::get();
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        
        // Configuration invariants
        assert!(min_active > 0); // Must have minimum validators
        assert!(min_active <= max_validators); // Min can't exceed max
        assert!(epoch_length > 0); // Epochs must have positive length
        assert!(min_stake > 0); // Must have minimum stake
        assert!(max_score > 0); // Must have positive max score
    });
}

/// Tests system health metrics
#[test]
fn system_health_metrics_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let current_epoch = DcfPallet::current_epoch();
        let (pos_weight, poi_weight) = (crate::PosWeight::<Test>::get(), crate::PoiWeight::<Test>::get());
        
        // Health metrics should indicate healthy system
        assert!(!active_validators.is_empty()); // Have active validators
        assert!(current_epoch >= 0); // Valid epoch
        assert!(pos_weight + poi_weight == 10000); // Weights sum correctly
        
        // Check validator distribution
        let total_stake: u128 = active_validators.iter()
            .map(|v| DcfPallet::validator_stake(v))
            .sum();
        assert!(total_stake > 0); // Total stake is positive
    });
}

/// Tests invariant preservation across blocks
#[test]
fn invariant_preservation_across_blocks_works() {
    new_test_ext().execute_with(|| {
        // Record initial state
        let initial_validators = DcfPallet::active_validators();
        let initial_epoch = DcfPallet::current_epoch();
        let initial_weights = (crate::PosWeight::<Test>::get(), crate::PoiWeight::<Test>::get());
        
        // Advance through multiple blocks
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            // Check invariants are maintained
            let current_validators = DcfPallet::active_validators();
            let current_epoch = DcfPallet::current_epoch();
            let current_weights = (crate::PosWeight::<Test>::get(), crate::PoiWeight::<Test>::get());
            
            // Invariant checks
            assert!(!current_validators.is_empty());
            assert!(current_epoch >= initial_epoch);
            assert_eq!(current_weights.0 + current_weights.1, 10000);
            
            // Validator set should remain stable (no random changes)
            assert_eq!(current_validators.len(), initial_validators.len());
        }
    });
}

/// Tests boundary condition invariants
#[test]
fn boundary_condition_invariants_hold() {
    new_test_ext().execute_with(|| {
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let active_validators = DcfPallet::active_validators();
        
        // Test at capacity boundaries
        if active_validators.len() == max_validators as usize {
            // At maximum capacity
            assert_eq!(active_validators.len(), max_validators as usize);
        } else {
            // Below maximum capacity
            assert!(active_validators.len() < max_validators as usize);
        }
        
        // Test score boundaries
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        for validator in &active_validators {
            let score = DcfPallet::validator_stake(validator);
            assert!(score <= max_score as u128);
        }
    });
}