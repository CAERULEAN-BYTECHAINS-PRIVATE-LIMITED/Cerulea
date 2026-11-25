//! Property-based and fuzz testing

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

/// Property: Validator set size never exceeds maximum
#[test]
fn property_validator_set_size_bounded() {
    new_test_ext().execute_with(|| {
        let max_validators =
            <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<
                u32,
            >>::get();

        // Test property across multiple blocks
        for block_num in 1..=20 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            // Property: |validators| <= max_validators
            assert!(active_validators.len() <= max_validators as usize);
        }
    });
}

/// Property: Consensus weights always sum to 100%
#[test]
fn property_consensus_weights_sum_to_hundred_percent() {
    new_test_ext().execute_with(|| {
        // Test property across multiple blocks
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let (pos_weight, poi_weight) = DcfPallet::consensus_weights();

            // Property: pos_weight + poi_weight = 10000 (100%)
            assert_eq!(pos_weight + poi_weight, 10000);
        }
    });
}

/// Property: Active validators always have positive stakes
#[test]
fn property_active_validators_have_positive_stakes() {
    new_test_ext().execute_with(|| {
        let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();

        // Test property across multiple states
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            for validator in &active_validators {
                let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);

                // Property: stake >= min_stake for all active validators
                assert!(stake >= min_stake);
            }
        }
    });
}

/// Property: Validator scores are within bounds
#[test]
fn property_validator_scores_within_bounds() {
    new_test_ext().execute_with(|| {
        super::assert_composition_reaches_one_whole();

        for block_num in 1..=8 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            for validator in &active_validators {
                // Property: the Trust Score stays inside one whole at every
                // block, and so does every component share.
                super::assert_trust_score_bounded(validator);
                super::assert_trust_weight_bounded(validator);
            }
        }
    });
}

/// Property: Epoch never decreases
#[test]
fn property_epoch_never_decreases() {
    new_test_ext().execute_with(|| {
        let mut previous_epoch = DcfPallet::current_epoch();

        for block_num in 1..=25 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let current_epoch = DcfPallet::current_epoch();

            // Property: epoch never decreases
            assert!(current_epoch >= previous_epoch);
            previous_epoch = current_epoch;
        }
    });
}

/// Property: System maintains minimum validators
#[test]
fn property_system_maintains_minimum_validators() {
    new_test_ext().execute_with(|| {
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();

        for block_num in 1..=12 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            // Property: |active_validators| >= min_active
            assert!(active_validators.len() >= min_active as usize);
        }
    });
}

/// Property: Balances are non-negative
#[test]
fn property_balances_non_negative() {
    new_test_ext().execute_with(|| {
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            for validator in &active_validators {
                let free_balance = Balances::free_balance(validator);
                let reserved_balance = Balances::reserved_balance(validator);

                // Property: every active validator holds a balance
                assert!(free_balance > 0 || reserved_balance > 0);
            }
        }
    });
}

/// Property: Validator participation counters are non-negative
#[test]
fn property_participation_counters_non_negative() {
    new_test_ext().execute_with(|| {
        for block_num in 1..=8 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            for validator in &active_validators {
                let (authored, missed) = DcfPallet::validator_participation(validator);

                // Property: participation counters stay within the blocks produced
                assert!(authored + missed <= block_num as u32);
            }
        }
    });
}

/// Fuzz test: Random validator queries
#[test]
fn fuzz_test_random_validator_queries() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();

        // Test with various validator IDs including invalid ones
        let test_validators = vec![1, 2, 3, 999, 1000, u64::MAX, 0, 42, 123, 456];

        for validator_id in test_validators {
            // All queries should handle invalid validators gracefully
            let stake_score = DcfPallet::validator_stake_score(&validator_id);
            let _ = DcfPallet::validator_inference_score(&validator_id);
            let is_active = DcfPallet::is_validator_active(&validator_id);
            let _ = DcfPallet::validator_participation(&validator_id);
            let _ = DcfPallet::validator_last_active(&validator_id);

            // Should not panic, should return reasonable defaults
            if active_validators.contains(&validator_id) {
                assert!(is_active);
                assert!(stake_score > 0);
            } else {
                assert!(!is_active);
            }
        }
    });
}

/// Fuzz test: Random block numbers
#[test]
fn fuzz_test_random_block_numbers() {
    new_test_ext().execute_with(|| {
        let test_blocks = vec![0, 1, 42, 100, 999, 1000, 9999, u32::MAX, u32::MAX / 2];

        for block_num in test_blocks {
            // Expected author queries should handle any block number
            let expected_author = DcfPallet::expected_author(block_num);
            let threshold = DcfPallet::assurance_class_table()
                .threshold_percent(crate::engines::assurance::TransactionClass::Routine);

            // Should not panic. No author determined is fine.
            if let Some(author) = expected_author {
                let active_validators = DcfPallet::active_validators();
                // If author is returned, should be valid
                assert!(active_validators.contains(&author) || active_validators.is_empty());
            }

            // The Assurance Engine holds the routine share above half.
            assert!(threshold > 50);
        }
    });
}

/// Property: System state consistency after operations
#[test]
fn property_system_state_consistency() {
    new_test_ext().execute_with(|| {
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            // Capture system state
            let active_validators = DcfPallet::active_validators();
            let consensus_weights = DcfPallet::consensus_weights();

            // Property: State should be internally consistent
            assert!(!active_validators.is_empty()); // Always have validators
            assert_eq!(consensus_weights.0 + consensus_weights.1, 10000); // Weights sum correctly

            // Validator consistency
            for validator in &active_validators {
                assert!(DcfPallet::is_validator_active(validator)); // Active validators marked as active
            }
        }
    });
}

/// Property: Deterministic behavior
#[test]
fn property_deterministic_behavior() {
    new_test_ext().execute_with(|| {
        // Same queries should return same results
        for _ in 0..10 {
            let validators1 = DcfPallet::active_validators();
            let validators2 = DcfPallet::active_validators();
            assert_eq!(validators1, validators2);

            let epoch1 = DcfPallet::current_epoch();
            let epoch2 = DcfPallet::current_epoch();
            assert_eq!(epoch1, epoch2);

            let weights1 = DcfPallet::consensus_weights();
            let weights2 = DcfPallet::consensus_weights();
            assert_eq!(weights1, weights2);
        }
    });
}

/// Property: Resource bounds respected
#[test]
fn property_resource_bounds_respected() {
    new_test_ext().execute_with(|| {
        let max_validators =
            <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<
                u32,
            >>::get();
        let max_history: u32 = <Test as crate::Config>::MaxValidatorHistorySize::get();

        for block_num in 1..=5 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            // Property: Resource limits respected
            assert!(active_validators.len() <= max_validators as usize);

            for validator in &active_validators {
                let history = DcfPallet::validator_score_history(validator);
                assert!(history.len() <= max_history as usize);
            }
        }
    });
}
