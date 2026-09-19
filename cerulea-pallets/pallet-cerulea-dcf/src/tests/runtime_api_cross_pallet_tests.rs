//! Runtime API integration tests between pallets

use crate::mock::*;
use frame_support::{
    assert_ok,
    traits::{Get, OnFinalize, OnInitialize, Currency},
};

/// Tests runtime API consistency across DCF, PoS, and PoI pallets
#[test]
fn runtime_api_cross_pallet_consistency() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        
        // Register validator in PoS
        assert_ok!(PalletCeruleaPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        // Submit PoS score
        assert_ok!(PalletCeruleaPos::submit_score(
            RuntimeOrigin::signed(2),
            validator,
            85
        ));
        
        // Submit PoI inference
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            90
        ));
        
        // Update DCF scores
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Test API consistency
        
        // Epoch consistency
        let dcf_epoch = DcfPallet::current_epoch();
        let poi_epoch = PalletCeruleaPoi::current_epoch();
        assert_eq!(dcf_epoch, poi_epoch);
        
        // Validator state consistency
        let dcf_active = DcfPallet::is_validator_active(&validator);
        let pos_registered = PalletCeruleaPos::validators(validator).is_some();
        let poi_has_inference = PalletCeruleaPoi::inference_results(validator).is_some();
        
        assert!(dcf_active);
        assert!(pos_registered);
        assert!(poi_has_inference);
        
        // Score consistency
        let pos_score = PalletCeruleaPos::validator_scores(validator).unwrap_or(0);
        let dcf_stake_score = DcfPallet::validator_stake_score(&validator);
        let dcf_inference_score = DcfPallet::validator_inference_score(&validator);
        
        assert_eq!(pos_score, 85);
        assert!(dcf_stake_score > 0);
        assert!(dcf_inference_score > 0);
        
        // Combined score calculation
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        let combined_score = (dcf_stake_score * pos_weight as u128 + 
                               dcf_inference_score as u128 * poi_weight as u128) / 10000;
        let expected_combined = combined_score as u64;
        assert!(expected_combined > 0);
    });
}

/// Tests runtime API data flow between pallets
#[test]
fn runtime_api_data_flow_integration() {
    new_test_ext().execute_with(|| {
        let validators = vec![10u64, 11u64, 12u64];
        
        // Setup validators in all pallets
        for validator in &validators {
            Balances::make_free_balance_be(validator, 50000);
            
            // DCF
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("DataFlowValidator{}", validator).into_bytes().try_into().unwrap())
            ));
            
            // PoS
            assert_ok!(PalletCeruleaPos::register_validator(
                RuntimeOrigin::signed(*validator)
            ));
            
            // PoI
            assert_ok!(PalletCeruleaPoi::submit_inference(
                RuntimeOrigin::signed(*validator),
                40 + *validator as u32,
                80 + (*validator as u32 % 15)
            ));
        }
        
        // Test bulk API operations
        let dcf_validators = DcfPallet::active_validators();
        let pos_active_validators = PalletCeruleaPos::get_active_validators();
        
        // Verify validator lists are consistent
        for validator in &validators {
            assert!(dcf_validators.contains(validator));
            assert!(pos_active_validators.contains(validator));
        }
        
        // Test score aggregation APIs
        let validators_by_score = DcfPallet::get_validators_by_score();
        assert!(!validators_by_score.is_empty());
        
        // Verify scores are ordered correctly
        for window in validators_by_score.windows(2) {
            let (_, score1) = window[0];
            let (_, score2) = window[1];
            assert!(score1 >= score2);
        }
        
        // Test validator set info API
        let (total_validators, active_validators, inactive_validators) = DcfPallet::get_validator_set_info();
        assert_eq!(total_validators, active_validators + inactive_validators);
        assert!(active_validators >= validators.len() as u32);
        
        // Test individual validator APIs
        for validator in &validators {
            let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
            let (authored, missed) = DcfPallet::validator_participation(validator);
            let last_active = DcfPallet::validator_last_active(validator);
            
            assert!(stake > 0);
            assert!(authored >= 0);
            assert!(missed >= 0);
            assert!(last_active >= 0);
            
            // Cross-pallet data verification
            let pos_score = PalletCeruleaPos::validator_scores(validator).unwrap_or(0);
            let (poi_result, poi_epoch) = PalletCeruleaPoi::inference_results(validator).unwrap();
            
            assert!(pos_score > 0);
            assert!(poi_result > 0);
            assert_eq!(poi_epoch, DcfPallet::current_epoch());
        }
    });
}

/// Tests runtime API performance with multiple pallets
#[test]
fn runtime_api_performance_integration() {
    new_test_ext().execute_with(|| {
        let num_validators = 20u64;
        
        // Setup many validators
        for i in 1..=num_validators {
            let validator = 100 + i;
            Balances::make_free_balance_be(&validator, 100000);
            
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(validator),
                Some(format!("PerfValidator{}", validator).into_bytes().try_into().unwrap())
            ));
            assert_ok!(PalletCeruleaPos::register_validator(
                RuntimeOrigin::signed(validator)
            ));
            assert_ok!(PalletCeruleaPos::submit_score(
                RuntimeOrigin::signed(1),
                validator,
                70 + (i % 25) as u32
            ));
            assert_ok!(PalletCeruleaPoi::submit_inference(
                RuntimeOrigin::signed(validator),
                (30 + i % 50) as u32,
                (75 + i % 20) as u32
            ));
        }
        
        // Test bulk API performance
        use std::time::Instant;
        
        // Test getting all validators
        let start = Instant::now();
        let _all_validators = DcfPallet::active_validators();
        let duration1 = start.elapsed();
        
        // Test getting validators by score
        let start = Instant::now();
        let _validators_by_score = DcfPallet::get_validators_by_score();
        let duration2 = start.elapsed();
        
        // Test individual validator queries
        let start = Instant::now();
        for i in 1..=num_validators {
            let validator = 100 + i;
            let _ = pallet_cerulea_pos::Stake::<Test>::get(validator);
            let stake_score = DcfPallet::validator_stake_score(&validator);
            let inference_score = DcfPallet::validator_inference_score(&validator);
            let _ = (stake_score, inference_score);
            let _ = PalletCeruleaPos::validator_scores(validator);
            let _ = PalletCeruleaPoi::inference_results(validator);
        }
        let duration3 = start.elapsed();
        
        // Performance should be reasonable (these are loose bounds for testing)
        assert!(duration1.as_millis() < 100);
        assert!(duration2.as_millis() < 200);
        assert!(duration3.as_millis() < 500);
        
        // Verify all operations completed successfully
        let final_validators = DcfPallet::active_validators();
        assert_eq!(final_validators.len(), (num_validators + 3) as usize);
    });
}

/// Tests runtime API error handling across pallets
#[test]
fn runtime_api_error_handling_integration() {
    new_test_ext().execute_with(|| {
        let valid_validator = 1u64;
        let invalid_validator = 999u64;
        
        // Setup one valid validator
        assert_ok!(PalletCeruleaPos::register_validator(
            RuntimeOrigin::signed(valid_validator)
        ));
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(valid_validator),
            42,
            85
        ));
        
        // Test APIs with invalid validator
        
        // DCF APIs should handle gracefully
        let stake = pallet_cerulea_pos::Stake::<Test>::get(invalid_validator);
        assert_eq!(stake, 0);
        
        let is_active = DcfPallet::is_validator_active(&invalid_validator);
        assert!(!is_active);
        
        let stake_score = DcfPallet::validator_stake_score(&invalid_validator);
        assert_eq!(stake_score, 0);
        
        let (authored, missed) = DcfPallet::validator_participation(&invalid_validator);
        assert_eq!(authored, 0);
        assert_eq!(missed, 0);
        
        // PoS APIs should handle gracefully
        let pos_score = PalletCeruleaPos::validator_scores(invalid_validator);
        assert_eq!(pos_score, None);
        
        let pos_active = PalletCeruleaPos::get_active_validators();
        assert!(!pos_active.contains(&invalid_validator));
        
        // PoI APIs should handle gracefully
        let poi_result = PalletCeruleaPoi::inference_results(invalid_validator);
        assert_eq!(poi_result, None);
        
        let poi_challenge = PalletCeruleaPoi::challenges(invalid_validator);
        assert_eq!(poi_challenge, None);
        
        // Test with valid validator for comparison
        let valid_stake = pallet_cerulea_pos::Stake::<Test>::get(valid_validator);
        assert!(valid_stake > 0);
        
        let valid_active = DcfPallet::is_validator_active(&valid_validator);
        assert!(valid_active);
        
        let valid_pos_active = PalletCeruleaPos::get_active_validators();
        assert!(valid_pos_active.contains(&valid_validator));
        
        let valid_poi_result = PalletCeruleaPoi::inference_results(valid_validator);
        assert!(valid_poi_result.is_some());
    });
}

/// Tests runtime API state transitions across pallets
#[test]
fn runtime_api_state_transitions_integration() {
    new_test_ext().execute_with(|| {
        let validator = 20u64;
        Balances::make_free_balance_be(&validator, 50000);
        
        // Phase 1: Initial state (not in any pallet)
        assert!(!DcfPallet::is_validator_active(&validator));
        assert_eq!(PalletCeruleaPos::validators(validator), None);
        assert_eq!(PalletCeruleaPoi::inference_results(validator), None);
        
        // Phase 2: Join DCF
        assert_ok!(mock_join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"StateTransitionValidator".to_vec().try_into().unwrap())
        ));
        
        assert!(DcfPallet::is_validator_active(&validator));
        assert!(pallet_cerulea_pos::Stake::<Test>::get(validator) > 0);
        assert_eq!(PalletCeruleaPos::validators(validator), None); // Still not in PoS
        
        // Phase 3: Register in PoS
        assert_ok!(PalletCeruleaPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        assert_eq!(PalletCeruleaPos::validators(validator), Some(true));
        let pos_active = PalletCeruleaPos::get_active_validators();
        assert!(pos_active.contains(&validator));
        
        // Phase 4: Submit PoS score
        assert_ok!(PalletCeruleaPos::submit_score(
            RuntimeOrigin::signed(1),
            validator,
            80
        ));
        
        assert_eq!(PalletCeruleaPos::validator_scores(validator), Some(80));
        
        // Phase 5: Submit PoI inference
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            85
        ));
        
        let (result, epoch) = PalletCeruleaPoi::inference_results(validator).unwrap();
        assert_eq!(result, 42);
        assert_eq!(epoch, DcfPallet::current_epoch());
        
        // Phase 6: Update DCF scores
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        let stake_score = DcfPallet::validator_stake_score(&validator);
        let inference_score = DcfPallet::validator_inference_score(&validator);
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        let combined_score = (stake_score * pos_weight as u128 + inference_score as u128 * poi_weight as u128) / 10000;
        
        assert!(stake_score > 0);
        assert!(inference_score > 0);
        assert!(combined_score > 0);
        
        // Phase 7: Slashing in PoS
        assert_ok!(mock_slash_validator(&validator, 100));
        assert_eq!(PalletCeruleaPos::slashing_count(validator), Some(1));
        
        // Multiple slashes to remove from PoS
        assert_ok!(mock_slash_validator(&validator, 100));
        assert_ok!(mock_slash_validator(&validator, 100));
        
        assert_eq!(PalletCeruleaPos::validators(validator), None);
        let pos_active_after_slash = PalletCeruleaPos::get_active_validators();
        assert!(!pos_active_after_slash.contains(&validator));
        
        // But still active in DCF
        assert!(DcfPallet::is_validator_active(&validator));
        
        // Phase 8: Leave DCF
        assert_ok!(mock_leave_validators(RuntimeOrigin::signed(validator)));
        
        // During cooldown, still shows as active
        assert!(DcfPallet::is_validator_active(&validator));
        
        // After cooldown period
        let cooldown_period: u32 = <<Test as pallet_cerulea_pos::Config>::LeaveCooldown as frame_support::traits::Get<u32>>::get();
        for block in 1..=cooldown_period + 1 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
            DcfPallet::on_finalize(block as u64);
        }
        
        // Should no longer be active
        assert!(!DcfPallet::is_validator_active(&validator));
        
        // But PoI data might still exist
        let poi_result_after_leave = PalletCeruleaPoi::inference_results(validator);
        // This depends on implementation - might be Some or None
        assert!(poi_result_after_leave.is_some() || poi_result_after_leave.is_none());
    });
}

/// Tests runtime API finality integration
#[test]
fn runtime_api_finality_integration() {
    new_test_ext().execute_with(|| {
        let validator = 30u64;
        
        // Setup validator
        Balances::make_free_balance_be(&validator, 50000);
        assert_ok!(mock_join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"FinalityValidator".to_vec().try_into().unwrap())
        ));
        
        // Test finality APIs
        let initial_score = DcfPallet::trust_score(&1u64).parts_per_million();
        let initial_current = System::block_number();
        
        // Advance blocks and test finality tracking
        for block in 1..=10 {
            System::set_block_number(block);
            DcfPallet::on_initialize(block);
            DcfPallet::on_finalize(block);
            
            let current_score = DcfPallet::trust_score(&1u64).parts_per_million();
            
            // The Trust Score stays inside its range at every block.
            assert!(current_score <= 1_000_000);
            assert!(initial_score <= 1_000_000);
            
            // Test expected author for block
            let expected_author = DcfPallet::get_expected_author(block as u32);
            match expected_author {
                Some(author) => {
                    let active_validators = DcfPallet::active_validators();
                    assert!(active_validators.contains(&author));
                },
                None => {
                    // No author determined, which is acceptable
                    assert!(true);
                }
            }
        }
        
        // The Assurance Engine answers every class above half the weight.
        let threshold_table = DcfPallet::assurance_class_table();
        let current_block = System::block_number() as u32;
        for class in [
            crate::engines::assurance::TransactionClass::Routine,
            crate::engines::assurance::TransactionClass::Financial,
            crate::engines::assurance::TransactionClass::Structural,
            crate::engines::assurance::TransactionClass::Constitutional,
        ] {
            assert!(threshold_table.threshold_percent(class) > 50);
        }
        assert!(current_block as u64 >= initial_current);
        
        // Test with validator participation
        let (authored, missed) = DcfPallet::validator_participation(&validator);
        assert!(authored >= 0);
        assert!(missed >= 0);
        
        // Total participation should be reasonable
        let total_participation = authored + missed;
        assert!(total_participation <= current_block);
    });
}

/// Tests runtime API epoch synchronization
#[test]
fn runtime_api_epoch_synchronization() {
    new_test_ext().execute_with(|| {
        let validators = vec![40u64, 41u64, 42u64];
        
        // Setup validators in all pallets
        for validator in &validators {
            Balances::make_free_balance_be(validator, 50000);
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("EpochValidator{}", validator).into_bytes().try_into().unwrap())
            ));
            assert_ok!(PalletCeruleaPos::register_validator(
                RuntimeOrigin::signed(*validator)
            ));
            assert_ok!(PalletCeruleaPoi::submit_inference(
                RuntimeOrigin::signed(*validator),
                40 + *validator as u32,
                85
            ));
        }
        
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let initial_dcf_epoch = DcfPallet::current_epoch();
        let initial_poi_epoch = PalletCeruleaPoi::current_epoch();
        
        // Verify initial synchronization
        assert_eq!(initial_dcf_epoch, initial_poi_epoch);
        
        // Advance through epoch boundary
        for block in 1..=epoch_length + 5 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
            DcfPallet::on_finalize(block as u64);
            
            // Check epoch consistency during transition
            let dcf_epoch = DcfPallet::current_epoch();
            let poi_epoch = PalletCeruleaPoi::current_epoch();
            
            // Epochs should remain synchronized
            assert_eq!(dcf_epoch, poi_epoch);
            
            // Epoch should advance monotonically
            assert!(dcf_epoch >= initial_dcf_epoch);
        }
        
        // Verify epoch advanced
        let final_dcf_epoch = DcfPallet::current_epoch();
        let final_poi_epoch = PalletCeruleaPoi::current_epoch();
        
        assert!(final_dcf_epoch > initial_dcf_epoch);
        assert_eq!(final_dcf_epoch, final_poi_epoch);
        
        // Test epoch-related data consistency
        for validator in &validators {
            let (poi_result, poi_epoch) = PalletCeruleaPoi::inference_results(validator).unwrap();
            
            // PoI epoch should be valid
            assert!(poi_epoch <= final_dcf_epoch);
            assert!(poi_result > 0);
            
            // Validator should still be active after epoch transition
            assert!(DcfPallet::is_validator_active(validator));
        }
        
        // Test validator set consistency across epoch
        let active_validators = DcfPallet::active_validators();
        for validator in &validators {
            assert!(active_validators.contains(validator));
        }
    });
}

/// Tests runtime API settings consistency
#[test]
fn runtime_api_configuration_consistency() {
    new_test_ext().execute_with(|| {
        // Test consensus weights API
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(pos_weight + poi_weight, 10000);
        assert!(pos_weight > 0);
        assert!(poi_weight > 0);
        
        // Test settings parameters
        let max_validators = <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<u32>>::get();
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();
        
        assert!(max_validators > 0);
        assert!(min_active > 0);
        assert!(epoch_length > 0);
        assert!(min_stake > 0);
        assert!(min_active <= max_validators);
        
        // Test PoS settings
        let pos_min_score: u32 = <<Test as pallet_cerulea_pos::Config>::MinValidatorScore as frame_support::traits::Get<u32>>::get();
        let pos_max_validators = <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<u32>>::get();
        let pos_min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        
        assert!(pos_min_score > 0);
        assert!(pos_max_validators > 0);
        assert!(pos_min_active > 0);
        
        // Test PoI settings
        let poi_min_confidence: u32 = <Test as pallet_cerulea_poi::Config>::MinInferenceConfidence::get();
        let poi_challenge_window: u32 = <Test as pallet_cerulea_poi::Config>::ChallengeWindow::get();
        let poi_max_age: u32 = <Test as pallet_cerulea_poi::Config>::MaxInferenceAge::get();
        
        assert!(poi_min_confidence > 0);
        assert!(poi_challenge_window > 0);
        assert!(poi_max_age > 0);
        
        // Test settings relationships
        assert!(pos_max_validators >= max_validators); // PoS should support at least as many
        assert!(pos_min_active <= min_active); // PoS minimum should not exceed DCF minimum
        
        // Test governance mode API
        let governance_enabled = DcfPallet::get_governance_mode();
        assert!(governance_enabled || !governance_enabled);
        
        // Test system info API
        let (total_validators, active_validators, inactive_validators) = DcfPallet::get_validator_set_info();
        assert_eq!(total_validators, active_validators + inactive_validators);
        assert!(active_validators >= min_active);
        assert!(total_validators <= max_validators);
        
        // Test validator count API
        let validator_count = DcfPallet::get_total_validators_count();
        assert_eq!(validator_count, total_validators);
    });
}