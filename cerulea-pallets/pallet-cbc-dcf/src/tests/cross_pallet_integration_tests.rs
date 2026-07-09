//! Cross-pallet integration tests for DCF, PoS, and PoI interactions

use crate::{mock::*, Error};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Get, OnFinalize, OnInitialize, Currency},
};

/// Tests DCF and PoS pallet integration
#[test]
fn dcf_pos_integration_validator_lifecycle() {
    new_test_ext().execute_with(|| {
        let validator = 10u64;
        let _stake_amount = 5000u128;
        
        // Setup validator with balance
        Balances::make_free_balance_be(&validator, 50000);
        
        // Test DCF validator joining
        assert_ok!(DcfPallet::join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"TestValidator".to_vec().try_into().unwrap())
        ));
        
        // Verify DCF state
        assert!(DcfPallet::is_validator_active(&validator));
        let validator_stake = DcfPallet::validator_stake(&validator);
        assert!(validator_stake > 0);
        
        // Test PoS registration integration
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        // Verify PoS state
        assert_eq!(PalletCbcPos::validators(&validator), Some(true));
        
        // Test score submission affects both pallets
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(1),
            validator,
            85
        ));
        
        // Verify score is recorded in PoS
        assert_eq!(PalletCbcPos::validator_scores(&validator), Some(85));
        
        // Test DCF score update integration
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Verify DCF reflects the update
        let updated_stake = DcfPallet::validator_stake(&validator);
        assert!(updated_stake > 0);
    });
}

/// Tests DCF and PoI pallet integration
#[test]
fn dcf_poi_integration_inference_tracking() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let inference_result = 42u32;
        let confidence = 85u32;
        
        // Verify validator is active in DCF
        assert!(DcfPallet::is_validator_active(&validator));
        
        // Test PoI inference submission
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            inference_result,
            confidence
        ));
        
        // Verify inference is recorded in PoI
        let (result, epoch) = PalletCbcPoi::inference_results(&validator).unwrap();
        assert_eq!(result, inference_result);
        assert_eq!(epoch, DcfPallet::current_epoch());
        
        // Test DCF inference score update
        assert_ok!(DcfPallet::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Verify DCF tracks inference activity
        let inference_score = DcfPallet::validator_inference_score(&validator);
        assert!(inference_score > 0);
    });
}

/// Tests three-way integration between DCF, PoS, and PoI
#[test]
fn three_way_pallet_integration() {
    new_test_ext().execute_with(|| {
        let validator = 11u64;
        let challenger = 12u64;
        let _stake_amount = 8000u128;
        
        // Setup validators with balances
        Balances::make_free_balance_be(&validator, 50000);
        Balances::make_free_balance_be(&challenger, 50000);
        
        // 1. Join DCF as validator
        assert_ok!(DcfPallet::join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"Validator".to_vec().try_into().unwrap())
        ));
        assert_ok!(DcfPallet::join_validators(
            RuntimeOrigin::signed(challenger),
            Some(b"Challenger".to_vec().try_into().unwrap())
        ));
        
        // 2. Register in PoS
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(challenger)
        ));
        
        // 3. Submit PoS scores
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(1),
            validator,
            90
        ));
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(1),
            challenger,
            85
        ));
        
        // 4. Submit PoI inference
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            88
        ));
        
        // 5. Challenge inference
        assert_ok!(PalletCbcPoi::challenge_inference(
            RuntimeOrigin::signed(challenger),
            validator,
            42
        ));
        
        // 6. Update DCF scores based on PoS and PoI activity
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        assert_ok!(DcfPallet::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Verify all pallets have consistent state
        assert!(DcfPallet::is_validator_active(&validator));
        assert!(DcfPallet::is_validator_active(&challenger));
        assert_eq!(PalletCbcPos::validators(&validator), Some(true));
        assert_eq!(PalletCbcPos::validators(&challenger), Some(true));
        assert!(PalletCbcPoi::inference_results(&validator).is_some());
        assert!(PalletCbcPoi::challenges(&challenger).is_some());
    });
}

/// Tests validator slashing across pallets
#[test]
fn cross_pallet_validator_slashing() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let initial_stake = DcfPallet::validator_stake(&validator);
        
        // Register in PoS
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        // Submit initial score
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(2),
            validator,
            75
        ));
        
        // Test PoS slashing
        assert_ok!(PalletCbcPos::slash_validator(&validator, 100));
        
        // Verify slashing count increased
        assert_eq!(PalletCbcPos::slashing_count(&validator), Some(1));
        
        // Test DCF slashing integration
        let slash_amount = 1000u128;
        assert_ok!(DcfPallet::slash_validator(
            RuntimeOrigin::root(),
            validator,
            slash_amount
        ));
        
        // Verify DCF stake was reduced
        let new_stake = DcfPallet::validator_stake(&validator);
        assert!(new_stake < initial_stake);
        
        // Test that excessive slashing removes validator from PoS
        assert_ok!(PalletCbcPos::slash_validator(&validator, 100));
        assert_ok!(PalletCbcPos::slash_validator(&validator, 100));
        
        // After 3 slashes, validator should be removed from PoS
        assert_eq!(PalletCbcPos::validators(&validator), None);
    });
}

/// Tests validator rewards across pallets
#[test]
fn cross_pallet_validator_rewards() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let initial_stake = DcfPallet::validator_stake(&validator);
        
        // Register in PoS
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        // Test PoS reward
        assert_ok!(PalletCbcPos::reward_validator(&validator, 500));
        
        // Submit high-quality inference in PoI
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            95 // High confidence
        ));
        
        // Test DCF reward integration
        let reward_amount = 2000u128;
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        assert_ok!(DcfPallet::propose_reward_validator(
            RuntimeOrigin::root(),
            validator,
            validator,
            reward_amount
        ));
        
        // Vote and execute proposal
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(validator),
            0,
            true
        ));
        assert_ok!(DcfPallet::execute_proposal(
            RuntimeOrigin::root(),
            0
        ));
        
        // Verify reward was applied
        let new_stake = DcfPallet::validator_stake(&validator);
        assert!(new_stake > initial_stake);
    });
}

/// Tests epoch transitions across all pallets
#[test]
fn cross_pallet_epoch_transitions() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        
        // Record initial states
        let initial_dcf_epoch = DcfPallet::current_epoch();
        let _initial_poi_epoch = PalletCbcPoi::current_epoch();
        
        // Register validator in PoS
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        
        // Submit inference in PoI
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            85
        ));
        
        // Advance through epoch boundary
        for block in 1..=epoch_length + 1 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
            DcfPallet::on_finalize(block as u64);
        }
        
        // Verify epoch advanced in all pallets
        let new_dcf_epoch = DcfPallet::current_epoch();
        let new_poi_epoch = PalletCbcPoi::current_epoch();
        
        assert!(new_dcf_epoch > initial_dcf_epoch);
        assert_eq!(new_dcf_epoch, new_poi_epoch); // Epochs should be synchronized
        
        // Verify validator states persist across epoch
        assert!(DcfPallet::is_validator_active(&validator));
        assert_eq!(PalletCbcPos::validators(&validator), Some(true));
    });
}

/// Tests consensus weight updates affecting all pallets
#[test]
fn cross_pallet_consensus_weight_updates() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        
        // Record initial consensus weights
        let (initial_pos_weight, initial_poi_weight) = DcfPallet::consensus_weights();
        
        // Register validator and submit scores
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(2),
            validator,
            80
        ));
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            90
        ));
        
        // Update consensus weights
        let new_pos_weight = 7000u64;
        let new_poi_weight = 3000u64;
        
        assert_ok!(DcfPallet::update_consensus_weights(
            RuntimeOrigin::root(),
            new_pos_weight,
            new_poi_weight
        ));
        
        // Verify weights updated
        let (current_pos_weight, current_poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(current_pos_weight, new_pos_weight);
        assert_eq!(current_poi_weight, new_poi_weight);
        assert_ne!(current_pos_weight, initial_pos_weight);
        assert_ne!(current_poi_weight, initial_poi_weight);
        
        // Update validator scores to reflect new weights
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        assert_ok!(DcfPallet::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Verify scores reflect new weight distribution
        let stake_score = DcfPallet::validator_stake_score(&validator);
        let inference_score = DcfPallet::validator_inference_score(&validator);
        
        assert!(stake_score > 0);
        assert!(inference_score > 0);
    });
}

/// Tests error propagation across pallets
#[test]
fn cross_pallet_error_propagation() {
    new_test_ext().execute_with(|| {
        let non_validator = 999u64;
        
        // Test that PoS errors don't affect DCF
        assert_noop!(
            PalletCbcPos::submit_score(
                RuntimeOrigin::signed(1),
                non_validator,
                75
            ),
            pallet_cbc_pos::Error::<Test>::ValidatorNotRegistered
        );
        
        // DCF should still function normally
        let active_validators = DcfPallet::active_validators();
        assert!(!active_validators.is_empty());
        
        // Test that PoI errors don't affect other pallets
        assert_noop!(
            PalletCbcPoi::challenge_inference(
                RuntimeOrigin::signed(1),
                non_validator,
                42
            ),
            pallet_cbc_poi::Error::<Test>::InferenceNotFound
        );
        
        // Other pallets should still function
        assert!(DcfPallet::is_validator_active(&1));
        
        // Test DCF error handling
        assert_noop!(
            DcfPallet::leave_validators(RuntimeOrigin::signed(non_validator)),
            Error::<Test>::ValidatorNotFound
        );
        
        // PoS and PoI should still function
        let validator = 1u64;
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            85
        ));
    });
}

/// Tests concurrent operations across pallets
#[test]
fn cross_pallet_concurrent_operations() {
    new_test_ext().execute_with(|| {
        let validators = vec![1u64, 2u64, 3u64];
        
        // Perform concurrent operations across all pallets
        for validator in &validators {
            // DCF operations
            assert_ok!(DcfPallet::update_validator_stake_score(
                RuntimeOrigin::signed(*validator),
                *validator
            ));
            
            // PoS operations
            assert_ok!(PalletCbcPos::register_validator(
                RuntimeOrigin::signed(*validator)
            ));
            assert_ok!(PalletCbcPos::submit_score(
                RuntimeOrigin::signed(1),
                *validator,
                75 + (*validator as u32 * 5)
            ));
            
            // PoI operations
            assert_ok!(PalletCbcPoi::submit_inference(
                RuntimeOrigin::signed(*validator),
                40 + *validator as u32,
                80 + (*validator as u32 * 2)
            ));
        }
        
        // Verify all operations completed successfully
        for validator in &validators {
            assert!(DcfPallet::is_validator_active(validator));
            assert_eq!(PalletCbcPos::validators(validator), Some(true));
            assert!(PalletCbcPoi::inference_results(validator).is_some());
        }
        
        // Test cross-pallet challenges
        assert_ok!(PalletCbcPoi::challenge_inference(
            RuntimeOrigin::signed(validators[1]),
            validators[0],
            40 + validators[0] as u32
        ));
        
        // Verify challenge was recorded
        assert!(PalletCbcPoi::challenges(&validators[1]).is_some());
    });
}

/// Tests data consistency across pallet boundaries
#[test]
fn cross_pallet_data_consistency() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        
        // Verify initial consistency
        let dcf_active = DcfPallet::is_validator_active(&validator);
        let dcf_epoch = DcfPallet::current_epoch();
        let poi_epoch = PalletCbcPoi::current_epoch();
        
        assert_eq!(dcf_epoch, poi_epoch);
        
        // Register in PoS and submit data
        assert_ok!(PalletCbcPos::register_validator(
            RuntimeOrigin::signed(validator)
        ));
        assert_ok!(PalletCbcPos::submit_score(
            RuntimeOrigin::signed(2),
            validator,
            85
        ));
        assert_ok!(PalletCbcPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            90
        ));
        
        // Update DCF scores
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        assert_ok!(DcfPallet::update_validator_inference_score(
            RuntimeOrigin::signed(validator),
            validator
        ));
        
        // Verify data consistency
        let pos_score = PalletCbcPos::validator_scores(&validator).unwrap_or(0);
        let (poi_result, poi_epoch) = PalletCbcPoi::inference_results(&validator).unwrap();
        let dcf_stake_score = DcfPallet::validator_stake_score(&validator);
        let dcf_inference_score = DcfPallet::validator_inference_score(&validator);
        
        // Verify relationships
        assert!(pos_score > 0);
        assert!(poi_result > 0);
        assert!(dcf_stake_score > 0);
        assert!(dcf_inference_score > 0);
        assert_eq!(poi_epoch, DcfPallet::current_epoch());
        
        // Verify validator is consistently active
        if dcf_active {
            assert_eq!(PalletCbcPos::validators(&validator), Some(true));
        }
    });
}

/// Tests pallet integration under stress conditions
#[test]
fn cross_pallet_stress_test() {
    new_test_ext().execute_with(|| {
        let num_validators = 10u64;
        
        // Setup multiple validators
        for i in 1..=num_validators {
            let validator = i;
            Balances::make_free_balance_be(&validator, 100000);
            
            // Join DCF
            assert_ok!(DcfPallet::join_validators(
                RuntimeOrigin::signed(validator),
                Some(format!("Validator{}", validator).into_bytes().try_into().unwrap())
            ));
            
            // Register in PoS
            assert_ok!(PalletCbcPos::register_validator(
                RuntimeOrigin::signed(validator)
            ));
            
            // Submit PoS score
            assert_ok!(PalletCbcPos::submit_score(
                RuntimeOrigin::signed(1),
                validator,
                70 + (i as u32 * 2)
            ));
            
            // Submit PoI inference
            assert_ok!(PalletCbcPoi::submit_inference(
                RuntimeOrigin::signed(validator),
                30 + i as u32,
                75 + (i as u32)
            ));
        }
        
        // Perform bulk operations
        for i in 1..=num_validators {
            let validator = i;
            
            // Update DCF scores
            assert_ok!(DcfPallet::update_validator_stake_score(
                RuntimeOrigin::signed(validator),
                validator
            ));
            assert_ok!(DcfPallet::update_validator_inference_score(
                RuntimeOrigin::signed(validator),
                validator
            ));
        }
        
        // Verify system stability
        let active_validators = DcfPallet::active_validators();
        assert_eq!(active_validators.len(), (num_validators + 3) as usize); // +3 for genesis validators
        
        // Verify all pallets maintain consistency
        for validator in &active_validators {
            if *validator <= num_validators {
                assert_eq!(PalletCbcPos::validators(validator), Some(true));
                assert!(PalletCbcPoi::inference_results(validator).is_some());
            }
        }
    });
}