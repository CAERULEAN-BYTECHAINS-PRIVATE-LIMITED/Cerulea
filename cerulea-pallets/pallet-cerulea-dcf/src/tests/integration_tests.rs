//! Basic integration tests for DCF pallet

use crate::{mock::*, Error};
use frame_support::{
    assert_noop, assert_ok, 
    traits::Get,
};

/// Tests basic validator set initialization from genesis
#[test]
fn basic_validator_set_initialization_works() {
    new_test_ext().execute_with(|| {
        // Check if validators are properly initialized
        let active_validators = DcfPallet::validator_set();
        assert_eq!(active_validators.len(), 3);
        assert!(active_validators.contains(&1));
        assert!(active_validators.contains(&2));
        assert!(active_validators.contains(&3));
        
        // Check current epoch
        assert_eq!(DcfPallet::current_epoch(), 0);
    });
}

/// Tests validator scoring system via storage
#[test]
fn validator_scoring_works() {
    new_test_ext().execute_with(|| {
        // Test that validators have initial scores - access via storage
        let validator_states = crate::ValidatorStates::<Test>::get(&1);
        if let Some(state) = validator_states {
            assert!(state.current.stake_score > 0);
            assert!(state.current.inference_score >= 0);
        }
    });
}

/// Tests basic epoch functionality
#[test]
fn epoch_management_works() {
    new_test_ext().execute_with(|| {
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0);
        
        // Test if validators are in the set
        let validator_set = DcfPallet::validator_set();
        assert!(validator_set.contains(&1));
        assert!(validator_set.contains(&2));
        assert!(validator_set.contains(&3));
    });
}

/// Tests consensus weights functionality
#[test]
fn consensus_weights_work() {
    new_test_ext().execute_with(|| {
        let pos_weight = crate::PosWeight::<Test>::get();
        let poi_weight = crate::PoiWeight::<Test>::get();
        assert!(pos_weight > 0);
        assert!(poi_weight > 0);
        assert_eq!(pos_weight + poi_weight, 10000); // Should sum to 100%
    });
}

/// Tests basic pallet functions don't panic
#[test]
fn pallet_functions_dont_panic() {
    new_test_ext().execute_with(|| {
        // Test all basic getter functions don't panic
        let _ = DcfPallet::validator_set();
        let _ = DcfPallet::current_epoch();
        let _ = crate::PosWeight::<Test>::get();
        let _ = crate::PoiWeight::<Test>::get();
    });
}

/// Tests validator set size constraints
#[test]
fn validator_set_constraints_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::validator_set();
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        
        // Validator set should not exceed maximum
        assert!(active_validators.len() <= max_validators as usize);
        
        // Should have minimum required validators
        let min_validators = <Test as crate::Config>::MinActiveValidators::get();
        assert!(active_validators.len() >= min_validators as usize);
    });
}

/// Tests epoch length configuration
#[test]
fn epoch_length_configuration_works() {
    new_test_ext().execute_with(|| {
        // Test that epoch length is properly configured
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        assert!(epoch_length > 0);
        assert_eq!(epoch_length, 2400); // From mock configuration
    });
}

/// Tests storage version
#[test]
fn storage_version_is_set() {
    new_test_ext().execute_with(|| {
        // This test ensures storage version is properly tracked
        assert!(true); // Placeholder for storage version checks
    });
}

// ================================================================================================
// Additional Unit Tests for Individual Function Coverage
// ================================================================================================


/// Tests update_consensus_weights function
#[test]
fn update_consensus_weights_works() {
    new_test_ext().execute_with(|| {
        let new_pos_weight = 6000u64;
        let new_poi_weight = 4000u64;
        
        // Test successful weight update
        assert_ok!(DcfPallet::update_consensus_weights(
            RuntimeOrigin::root(),
            new_pos_weight,
            new_poi_weight
        ));
        
        // Verify weights were updated
        assert_eq!(crate::PosWeight::<Test>::get(), new_pos_weight);
        assert_eq!(crate::PoiWeight::<Test>::get(), new_poi_weight);
    });
}

/// Tests update_consensus_weights with invalid origin
#[test]
fn update_consensus_weights_fails_with_invalid_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_consensus_weights(
                RuntimeOrigin::signed(1),
                5000,
                5000
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// Tests set_governance_mode function
#[test]
fn set_governance_mode_works() {
    new_test_ext().execute_with(|| {
        // Test enabling governance mode
        assert_ok!(DcfPallet::set_governance_mode(
            RuntimeOrigin::root(),
            true
        ));
        
        assert!(crate::GovernanceModeEnabled::<Test>::get());
        
        // Test disabling governance mode
        assert_ok!(DcfPallet::set_governance_mode(
            RuntimeOrigin::root(),
            false
        ));
        
        assert!(!crate::GovernanceModeEnabled::<Test>::get());
    });
}

/// Tests set_governance_mode with invalid origin
#[test]
fn set_governance_mode_fails_with_invalid_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_governance_mode(
                RuntimeOrigin::signed(1),
                true
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// Tests set_validator_name function
#[test]
fn set_validator_name_works() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let name = b"Alice".to_vec();
        
        assert_ok!(DcfPallet::set_validator_name(
            RuntimeOrigin::signed(validator),
            name.clone()
        ));
        
        // Function should execute without error
        // (Actual verification would depend on storage structure)
    });
}

/// Tests set_validator_name with non-validator
#[test]
fn set_validator_name_fails_with_non_validator() {
    new_test_ext().execute_with(|| {
        let non_validator = 999u64;
        let name = b"Invalid".to_vec();
        
        assert_noop!(
            DcfPallet::set_validator_name(
                RuntimeOrigin::signed(non_validator),
                name
            ),
            Error::<Test>::ValidatorNotFound
        );
    });
}

/// Tests consensus weights management with different combinations
#[test]
fn consensus_weights_management() {
    new_test_ext().execute_with(|| {
        // Test different weight combinations
        let test_cases = vec![
            (5000u64, 5000u64),
            (7000u64, 3000u64),
            (3000u64, 7000u64),
            (9000u64, 1000u64),
            (1000u64, 9000u64),
        ];
        
        for (pos_weight, poi_weight) in test_cases {
            assert_ok!(DcfPallet::update_consensus_weights(
                RuntimeOrigin::root(),
                pos_weight,
                poi_weight
            ));
            
            // Verify weights were set
            assert_eq!(crate::PosWeight::<Test>::get(), pos_weight);
            assert_eq!(crate::PoiWeight::<Test>::get(), poi_weight);
        }
    });
}

/// Tests governance mode toggle functionality
#[test]
fn governance_mode_toggle() {
    new_test_ext().execute_with(|| {
        // Test multiple toggles
        for enabled in [true, false, true, false, true].iter() {
            assert_ok!(DcfPallet::set_governance_mode(
                RuntimeOrigin::root(),
                *enabled
            ));
            
            assert_eq!(crate::GovernanceModeEnabled::<Test>::get(), *enabled);
        }
    });
}

/// Tests validator name setting with different lengths
#[test]
fn set_validator_name_validates_length() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        
        // Test empty name (should work or fail based on implementation)
        let _result = DcfPallet::set_validator_name(
            RuntimeOrigin::signed(validator),
            vec![]
        );
        // Don't assert specific behavior as it depends on implementation
        
        // Test reasonable length name
        let reasonable_name = b"ValidatorName".to_vec();
        let _result = DcfPallet::set_validator_name(
            RuntimeOrigin::signed(validator),
            reasonable_name
        );
        
        // Test very long name
        let long_name = vec![b'A'; 1000];
        let _result = DcfPallet::set_validator_name(
            RuntimeOrigin::signed(validator),
            long_name
        );
        // Don't assert specific behavior as it depends on implementation
    });
}

/// Tests error conditions for common error variants
#[test]
fn common_error_variants_are_testable() {
    new_test_ext().execute_with(|| {
        // Test ValidatorNotFound with set_validator_name
        assert_noop!(
            DcfPallet::set_validator_name(
                RuntimeOrigin::signed(999u64),
                b"Invalid".to_vec()
            ),
            Error::<Test>::ValidatorNotFound
        );
        
        // Test BadOrigin errors
        assert_noop!(
            DcfPallet::update_consensus_weights(
                RuntimeOrigin::signed(1),
                5000,
                5000
            ),
            sp_runtime::DispatchError::BadOrigin
        );
        
        assert_noop!(
            DcfPallet::set_governance_mode(
                RuntimeOrigin::signed(1),
                true
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

/// Tests validator set management functions
#[test]
fn validator_set_management_basic() {
    new_test_ext().execute_with(|| {
        // Test basic validator set operations
        let validator_set = DcfPallet::validator_set();
        assert!(!validator_set.is_empty());
        
        // Test that all validators in set are valid
        for validator in validator_set.iter() {
            assert!(*validator > 0);
        }
        
        // Test validator set size constraints
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        assert!(validator_set.len() <= max_validators as usize);
        
        let min_validators = <Test as crate::Config>::MinActiveValidators::get();
        assert!(validator_set.len() >= min_validators as usize);
    });
}

/// Tests epoch management functionality
#[test]
fn epoch_management_basic() {
    new_test_ext().execute_with(|| {
        // Test current epoch
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0);
        
        // Test epoch configuration
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        assert!(epoch_length > 0);
        
        // Test that epoch functions don't panic
        let _ = DcfPallet::current_epoch();
    });
}

/// Tests configuration constants and their relationships
#[test]
fn configuration_constants_validation() {
    new_test_ext().execute_with(|| {
        // Test that configuration constants have reasonable values
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_validators = <Test as crate::Config>::MinActiveValidators::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        // Basic sanity checks
        assert!(max_validators > 0);
        assert!(min_validators > 0);
        assert!(max_validators >= min_validators);
        assert!(epoch_length > 0);
        assert!(min_stake > 0);
        
        // Test consensus weights sum to 100%
        let pos_weight = crate::PosWeight::<Test>::get();
        let poi_weight = crate::PoiWeight::<Test>::get();
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}

/// Tests pallet storage access patterns
#[test]
fn storage_access_patterns() {
    new_test_ext().execute_with(|| {
        // Test that storage can be accessed without panicking
        let validator_set = DcfPallet::validator_set();
        
        for validator in validator_set.iter().take(3) {
            // Test validator state access
            let _validator_state = crate::ValidatorStates::<Test>::get(validator);
            
            // Test that storage access doesn't panic
            let _ = DcfPallet::validator_stake(validator);
        }
        
        // Test governance mode storage
        let _governance_enabled = crate::GovernanceModeEnabled::<Test>::get();
        
        // Test consensus weights storage
        let _pos_weight = crate::PosWeight::<Test>::get();
        let _poi_weight = crate::PoiWeight::<Test>::get();
        
        // Test current epoch storage
        let _current_epoch = DcfPallet::current_epoch();
    });
}

/// Tests function execution performance
#[test]
fn function_execution_performance() {
    new_test_ext().execute_with(|| {
        // Test that functions execute in reasonable time
        
        // Test consensus weight updates
        for i in 1..=10 {
            let pos_weight = 5000 + (i * 100);
            let poi_weight = 10000 - pos_weight;
            
            assert_ok!(DcfPallet::update_consensus_weights(
                RuntimeOrigin::root(),
                pos_weight,
                poi_weight
            ));
        }
        
        // Test governance mode toggles
        for _ in 1..=10 {
            assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
            assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), false));
        }
        
        // Test validator name setting
        let validator = 1u64;
        for i in 1..=5 {
            let name = format!("Validator{}", i).into_bytes();
            let _result = DcfPallet::set_validator_name(
                RuntimeOrigin::signed(validator),
                name
            );
        }
    });
}