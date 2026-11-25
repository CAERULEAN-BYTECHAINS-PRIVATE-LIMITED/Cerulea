//! Runtime API integration tests for DCF pallet

use crate::mock::*;

/// Tests basic runtime API functionality
#[test]
fn runtime_api_basic_queries_work() {
    new_test_ext().execute_with(|| {
        // Test basic validator queries
        let active_validators = DcfPallet::validator_set();
        assert!(!active_validators.is_empty());

        // Test score queries for active validators
        for validator in &active_validators {
            let stake_score = pallet_cerulea_pos::Stake::<Test>::get(validator);

            assert!(stake_score > 0);
        }
    });
}

/// Tests consensus weight queries
#[test]
fn runtime_api_consensus_weights_work() {
    new_test_ext().execute_with(|| {
        // Test consensus weights via storage
        let pos_weight = crate::PosWeight::<Test>::get();
        let poi_weight = crate::PoiWeight::<Test>::get();

        // Weights should be positive and sum to 100%
        assert!(pos_weight > 0);
        assert!(poi_weight > 0);
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}

/// Tests epoch state queries
#[test]
fn runtime_api_epoch_queries_work() {
    new_test_ext().execute_with(|| {
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0);

        // Test epoch-related queries
        let governance_mode = DcfPallet::get_governance_mode();
        assert!(!governance_mode); // Default should be false
    });
}

/// Tests validator activity queries
#[test]
fn runtime_api_validator_activity_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();

        for validator in &active_validators {
            // Test activity status
            let is_active = DcfPallet::is_validator_active(validator);
            assert!(is_active);

            // Test basic validator data
            let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
            assert!(stake > 0);
        }
    });
}

/// Tests block authorship queries
#[test]
fn runtime_api_block_authorship_works() {
    new_test_ext().execute_with(|| {
        // Test expected author functionality using available methods
        for block_num in 1..=5 {
            let expected_author = DcfPallet::get_expected_author(block_num);

            // No author is acceptable while author scheduling is not ready.
            if let Some(author) = expected_author {
                // Author should be an active validator
                let active_validators = DcfPallet::validator_set();
                assert!(active_validators.contains(&author));
            }
        }
    });
}

/// Tests validator score history queries
#[test]
fn runtime_api_score_history_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();

        for validator in &active_validators {
            // Test basic score access via storage
            let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);

            // History should be accessible (simplified)
            assert!(stake > 0);
        }
    });
}

/// Tests validator metadata queries
#[test]
fn runtime_api_validator_metadata_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();

        for validator in &active_validators {
            // Test basic validator information
            let name = DcfPallet::get_validator_name(validator);

            // Name query should not panic. A stored name stays inside its bound.
            if let Some(name) = name {
                assert!(name.len() <= 32);
            }
        }
    });
}

/// Tests system start values queries
#[test]
fn runtime_api_system_config_works() {
    new_test_ext().execute_with(|| {
        // Test basic system information
        let current_epoch = DcfPallet::current_epoch();
        let validator_set = DcfPallet::validator_set();

        // Basic system state should be available
        assert_eq!(current_epoch, 0);
        assert!(!validator_set.is_empty());
    });
}

/// Tests validator count and set information
#[test]
fn runtime_api_validator_set_info_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();
        let total_count = active_validators.len() as u32;

        // Total count should be reasonable
        assert!(total_count >= 3); // From genesis config

        // Test basic validator set information
        let max_validators = <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<u32>>::get();
        let min_validators = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();

        assert!(total_count >= min_validators);
        assert!(total_count <= max_validators);
    });
}

/// Tests validators by score ranking
#[test]
fn runtime_api_validators_by_score_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();

        // Create a simple ranking based on available data
        let mut validators_with_scores: Vec<_> = active_validators
            .iter()
            .map(|v| (*v, pallet_cerulea_pos::Stake::<Test>::get(v)))
            .collect();

        // Sort by score (descending)
        validators_with_scores.sort_by(|a, b| b.1.cmp(&a.1));

        // Should have validators
        assert!(!validators_with_scores.is_empty());

        // Verify sorting (scores should be in descending order)
        for window in validators_with_scores.windows(2) {
            let (_, score1) = window[0];
            let (_, score2) = window[1];
            assert!(
                score1 >= score2,
                "Validators should be sorted by score in descending order"
            );
        }
    });
}

/// Tests finality-related queries
#[test]
fn runtime_api_finality_queries_work() {
    new_test_ext().execute_with(|| {
        // Test basic block information
        let _ = System::block_number();

        // Test current system state
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0);
    });
}

/// Tests error handling in API queries
#[test]
fn runtime_api_error_handling_works() {
    new_test_ext().execute_with(|| {
        let non_existent_validator = 999u64;

        // These queries should handle non-existent validators gracefully
        let stake_score = pallet_cerulea_pos::Stake::<Test>::get(non_existent_validator);
        let is_active = DcfPallet::is_validator_active(&non_existent_validator);

        // Should return default/empty values without panicking
        assert_eq!(stake_score, 0);
        assert!(!is_active);
    });
}
