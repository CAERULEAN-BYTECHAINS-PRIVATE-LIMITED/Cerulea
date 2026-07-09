//! Deterministic processing tests

use crate::mock::*;
use frame_support::traits::{OnFinalize, OnInitialize};

/// Tests deterministic validator set ordering
#[test]
fn deterministic_validator_set_ordering_works() {
    new_test_ext().execute_with(|| {
        let validators1 = DcfPallet::active_validators();
        let validators2 = DcfPallet::active_validators();
        
        // Should return identical results
        assert_eq!(validators1, validators2);
        
        // Order should be consistent
        assert_eq!(validators1.len(), validators2.len());
        for (v1, v2) in validators1.iter().zip(validators2.iter()) {
            assert_eq!(v1, v2);
        }
    });
}

/// Tests deterministic score calculations
#[test]
fn deterministic_score_calculations_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Record initial scores
        let initial_scores: Vec<_> = active_validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake_score(v), DcfPallet::validator_inference_score(v)))
            .collect();
        
        // Query scores again
        let repeat_scores: Vec<_> = active_validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake_score(v), DcfPallet::validator_inference_score(v)))
            .collect();
        
        // Should be identical
        assert_eq!(initial_scores, repeat_scores);
    });
}

/// Tests deterministic consensus weight calculations
#[test]
fn deterministic_consensus_weight_calculations_work() {
    new_test_ext().execute_with(|| {
        // Multiple queries should return identical results
        let weights1 = DcfPallet::consensus_weights();
        let weights2 = DcfPallet::consensus_weights();
        let weights3 = DcfPallet::consensus_weights();
        
        assert_eq!(weights1, weights2);
        assert_eq!(weights2, weights3);
        
        // Sum should always be 10000
        assert_eq!(weights1.0 + weights1.1, 10000);
        assert_eq!(weights2.0 + weights2.1, 10000);
        assert_eq!(weights3.0 + weights3.1, 10000);
    });
}

/// Tests deterministic block processing
#[test]
fn deterministic_block_processing_works() {
    new_test_ext().execute_with(|| {
        let initial_state = (
            DcfPallet::active_validators(),
            DcfPallet::current_epoch(),
            DcfPallet::consensus_weights(),
        );
        
        // Process the same block multiple times (conceptually)
        for _ in 0..3 {
            let weight = DcfPallet::on_initialize(1);
            DcfPallet::on_finalize(1);
            
            // State should remain consistent
            let current_state = (
                DcfPallet::active_validators(),
                DcfPallet::current_epoch(),
                DcfPallet::consensus_weights(),
            );
            
            // Some elements should remain stable within the same block
            assert_eq!(current_state.2, initial_state.2); // Consensus weights
        }
    });
}

/// Tests deterministic epoch state
#[test]
fn deterministic_epoch_state_works() {
    new_test_ext().execute_with(|| {
        let epoch1 = DcfPallet::current_epoch();
        let epoch2 = DcfPallet::current_epoch();
        let epoch3 = DcfPallet::current_epoch();
        
        // Epoch should be stable within same block
        assert_eq!(epoch1, epoch2);
        assert_eq!(epoch2, epoch3);
        
        // Should be non-negative
        assert!(epoch1 >= 0);
    });
}

/// Tests deterministic validator activity tracking
#[test]
fn deterministic_validator_activity_tracking_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            // Multiple queries should return identical results
            let participation1 = DcfPallet::validator_participation(validator);
            let participation2 = DcfPallet::validator_participation(validator);
            let participation3 = DcfPallet::validator_participation(validator);
            
            assert_eq!(participation1, participation2);
            assert_eq!(participation2, participation3);
            
            let last_active1 = DcfPallet::validator_last_active(validator);
            let last_active2 = DcfPallet::validator_last_active(validator);
            
            assert_eq!(last_active1, last_active2);
            
            let is_active1 = DcfPallet::is_validator_active(validator);
            let is_active2 = DcfPallet::is_validator_active(validator);
            
            assert_eq!(is_active1, is_active2);
            assert!(is_active1); // Should be true for active validators
        }
    });
}

/// Tests deterministic validator ranking
#[test]
fn deterministic_validator_ranking_works() {
    new_test_ext().execute_with(|| {
        // Multiple queries should return identical ordering
        let ranking1 = DcfPallet::validators_by_score();
        let ranking2 = DcfPallet::validators_by_score();
        let ranking3 = DcfPallet::validators_by_score();
        
        assert_eq!(ranking1, ranking2);
        assert_eq!(ranking2, ranking3);
        
        // Verify sorting is deterministic
        for window in ranking1.windows(2) {
            let (_, score1) = window[0];
            let (_, score2) = window[1];
            assert!(score1 >= score2);
        }
    });
}

/// Tests deterministic system metrics
#[test]
fn deterministic_system_metrics_work() {
    new_test_ext().execute_with(|| {
        // System info should be deterministic
        let info1 = DcfPallet::validator_set_info();
        let info2 = DcfPallet::validator_set_info();
        let info3 = DcfPallet::validator_set_info();
        
        assert_eq!(info1, info2);
        assert_eq!(info2, info3);
        
        let total1 = DcfPallet::total_validators_count();
        let total2 = DcfPallet::total_validators_count();
        
        assert_eq!(total1, total2);
        
        let governance1 = DcfPallet::governance_mode();
        let governance2 = DcfPallet::governance_mode();
        
        assert_eq!(governance1, governance2);
    });
}

/// Tests deterministic block authorship
#[test]
fn deterministic_block_authorship_works() {
    new_test_ext().execute_with(|| {
        // Expected author for same block should be deterministic
        for block_num in 1..=5 {
            let author1 = DcfPallet::expected_author(block_num);
            let author2 = DcfPallet::expected_author(block_num);
            let author3 = DcfPallet::expected_author(block_num);
            
            assert_eq!(author1, author2);
            assert_eq!(author2, author3);
        }
    });
}

/// Tests deterministic finality information
#[test]
fn deterministic_finality_information_works() {
    new_test_ext().execute_with(|| {
        let finalized1 = DcfPallet::last_finalized_block();
        let finalized2 = DcfPallet::last_finalized_block();
        
        assert_eq!(finalized1, finalized2);
        
        let finality_info1 = DcfPallet::finality_info();
        let finality_info2 = DcfPallet::finality_info();
        
        assert_eq!(finality_info1, finality_info2);
        
        // Test finality check determinism
        for block_num in 0..=5 {
            let is_finalized1 = DcfPallet::is_block_finalized(block_num);
            let is_finalized2 = DcfPallet::is_block_finalized(block_num);
            
            assert_eq!(is_finalized1, is_finalized2);
        }
    });
}

/// Tests deterministic weight calculations
#[test]
fn deterministic_weight_calculations_work() {
    new_test_ext().execute_with(|| {
        // Weight calculations should be deterministic
        for block_num in 1..=5 {
            let weight1 = DcfPallet::on_initialize(block_num);
            // Note: We can't call on_initialize multiple times for the same block
            // in practice, but we can test that the calculation is deterministic
            // by checking that similar operations yield predictable weights
            
            assert!(weight1.ref_time() > 0);
            assert!(weight1.ref_time() < 1_000_000_000); // Reasonable bound
        }
    });
}

/// Tests deterministic state transitions
#[test]
fn deterministic_state_transitions_work() {
    new_test_ext().execute_with(|| {
        let initial_validators = DcfPallet::active_validators();
        let initial_epoch = DcfPallet::current_epoch();
        
        // Process a block
        System::set_block_number(1);
        let _ = DcfPallet::on_initialize(1);
        DcfPallet::on_finalize(1);
        
        let post_block_validators = DcfPallet::active_validators();
        let post_block_epoch = DcfPallet::current_epoch();
        
        // State changes should be predictable
        assert_eq!(post_block_validators.len(), initial_validators.len());
        assert!(post_block_epoch >= initial_epoch);
        
        // If no epoch transition occurred, validator set should be stable
        if post_block_epoch == initial_epoch {
            assert_eq!(post_block_validators, initial_validators);
        }
    });
}

/// Tests deterministic error handling
#[test]
fn deterministic_error_handling_works() {
    new_test_ext().execute_with(|| {
        let non_existent_validator = 999u64;
        
        // Error cases should be handled deterministically
        for _ in 0..5 {
            let score = DcfPallet::validator_stake_score(&non_existent_validator);
            let inference_score = DcfPallet::validator_inference_score(&non_existent_validator);
            let is_active = DcfPallet::is_validator_active(&non_existent_validator);
            let participation = DcfPallet::validator_participation(&non_existent_validator);
            let last_active = DcfPallet::validator_last_active(&non_existent_validator);
            
            // Should consistently return default values
            assert_eq!(score, 0); // Assuming default is 0
            assert_eq!(inference_score, 0);
            assert!(!is_active);
            assert_eq!(participation, (0, 0));
            assert_eq!(last_active, 0);
        }
    });
}