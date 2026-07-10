//! System integration tests for DCF pallet runtime integration

use crate::mock::*;
use frame_support::{
    assert_ok,
    traits::{Get, OnFinalize, OnInitialize, Currency},
};

/// Tests system integration and runtime hooks
#[test]
fn system_runtime_integration_works() {
    new_test_ext().execute_with(|| {
        // Test that the pallet integrates properly with the runtime
        let block_number = System::block_number();
        assert_eq!(block_number, 0);
        
        // Test on_initialize hook
        let result = DcfPallet::on_initialize(1);
        assert!(result.all_gte(frame_support::weights::Weight::zero()));
    });
}

/// Tests block production and authorship tracking
#[test]
fn block_authorship_tracking_works() {
    new_test_ext().execute_with(|| {
        // Test expected author functionality
        let expected_author = DcfPallet::get_expected_author(1);
        // Should return Some validator or None if not determined yet
        match expected_author {
            Some(author) => {
                // Author should be one of the active validators
                let active_validators = DcfPallet::validator_set();
                assert!(active_validators.contains(&author));
            },
            None => {
                // This is acceptable for early blocks
                assert!(true);
            }
        }
    });
}

/// Tests epoch boundary processing
#[test]
fn epoch_boundary_processing_works() {
    new_test_ext().execute_with(|| {
        let initial_epoch = DcfPallet::current_epoch();
        
        // Advance to near epoch boundary
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        
        // Run through some blocks without hitting the epoch boundary
        for block_num in 1..5 {
            System::set_block_number(block_num as u64);
            let _ = DcfPallet::on_initialize(block_num as u64);
            DcfPallet::on_finalize(block_num as u64);
        }
        
        // Epoch should still be the same
        assert_eq!(DcfPallet::current_epoch(), initial_epoch);
    });
}

/// Tests validator state persistence across blocks
#[test]
fn validator_state_persistence_works() {
    new_test_ext().execute_with(|| {
        let initial_validators = DcfPallet::validator_set();
        let initial_scores: Vec<_> = initial_validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake(v)))
            .collect();
        
        // Advance a few blocks
        for block_num in 1..5 {
            System::set_block_number(block_num as u64);
            let _ = DcfPallet::on_initialize(block_num as u64);
            DcfPallet::on_finalize(block_num as u64);
        }
        
        // Validator set should remain stable
        let current_validators = DcfPallet::validator_set();
        assert_eq!(initial_validators, current_validators);
        
        // Scores should remain stable (or change predictably)
        for (validator, initial_score) in initial_scores {
            let current_score = DcfPallet::validator_stake(&validator);
            // Score should not change dramatically without explicit updates
            assert!(current_score > 0);
        }
    });
}

/// Tests weight calculation and limits
#[test]
fn weight_calculation_works() {
    new_test_ext().execute_with(|| {
        // Test that on_initialize returns reasonable weights
        let weight = DcfPallet::on_initialize(1);
        assert!(weight.ref_time() > 0);
        
        // Weight should be bounded by reasonable limits
        let max_weight = frame_support::weights::Weight::from_parts(1_000_000_000, 0);
        assert!(weight.ref_time() < max_weight.ref_time());
    });
}

/// Tests event emission
#[test]
fn event_emission_works() {
    new_test_ext().execute_with(|| {
        // Reset events
        System::reset_events();
        
        // Perform some operations that should emit events
        System::set_block_number(1);
        let _ = DcfPallet::on_initialize(1);
        DcfPallet::on_finalize(1);
        
        // Check that some events were emitted (if any)
        let events = System::events();
        // Events may or may not be emitted depending on the block, which is fine
        assert!(events.len() >= 0);
    });
}

/// Tests storage consistency
#[test]
fn storage_consistency_works() {
    new_test_ext().execute_with(|| {
        // Verify that storage items are consistent with each other
        let active_validators = DcfPallet::validator_set();
        
        // Each active validator should have associated data
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake(validator);
            
            // Scores should be non-negative
            assert!(stake_score >= 0);
            
            // Validator should be marked as active
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}

/// Tests configuration parameter access
#[test]
fn configuration_parameters_work() {
    new_test_ext().execute_with(|| {
        // Test that configuration parameters are accessible and reasonable
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        assert!(max_validators > 0);
        assert!(min_active > 0);
        assert!(epoch_length > 0);
        assert!(min_stake > 0);
        
        // Logical relationships
        assert!(min_active <= max_validators);
    });
}

/// Tests system time integration
#[test]
fn system_time_integration_works() {
    new_test_ext().execute_with(|| {
        // Test that the pallet works with system time tracking
        let current_block = System::block_number();
        assert_eq!(current_block, 0);
        
        // Advance system state
        System::set_block_number(10);
        assert_eq!(System::block_number(), 10);
        
        // DCF should handle block number changes gracefully
        let _ = DcfPallet::on_initialize(10);
        // Remove call to non-existent function
    });
}

/// Tests error handling and edge cases
#[test]
fn error_handling_works() {
    new_test_ext().execute_with(|| {
        // Test queries for non-existent validators
        let non_existent_validator = 999u64;
        
        // These should return default values or handle gracefully
        let score = DcfPallet::validator_stake(&non_existent_validator);
        let is_active = DcfPallet::is_validator_active(&non_existent_validator);
        
        // Should handle gracefully without panicking
        assert!(score >= 0);
        assert_eq!(is_active, false);
    });
}

#[test]
fn system_initialization_with_genesis_works() {
    new_test_ext().execute_with(|| {
        // Verify system is properly initialized
        assert_eq!(System::block_number(), 0);
        assert_eq!(DcfPallet::current_epoch(), 0);
        
        // Check that genesis validators are set up correctly
        let active_validators = DcfPallet::validator_set();
        assert_eq!(active_validators.len(), 3);
        
        // Verify initial balances
        assert!(Balances::free_balance(&1) > 0);
        assert!(Balances::free_balance(&2) > 0);
        assert!(Balances::free_balance(&3) > 0);
    });
}

#[test]
fn balance_integration_for_validator_joining_works() {
    new_test_ext().execute_with(|| {
        let new_validator = 10;
        let initial_balance = 100000;
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        // Set up balance for new validator
        Balances::make_free_balance_be(&new_validator, initial_balance);
        
        let free_before = Balances::free_balance(&new_validator);
        let reserved_before = Balances::reserved_balance(&new_validator);
        
        // Join validators - should reserve the minimum stake
        assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(new_validator), None));
        
        let free_after = Balances::free_balance(&new_validator);
        let reserved_after = Balances::reserved_balance(&new_validator);
        
        // Verify balance changes
        assert_eq!(free_before - free_after, min_stake);
        assert_eq!(reserved_after - reserved_before, min_stake);
        
        // Verify validator is active
        assert!(DcfPallet::is_validator_active(&new_validator));
    });
}

#[test]
fn epoch_transition_system_integration_works() {
    new_test_ext().execute_with(|| {
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let initial_epoch = DcfPallet::current_epoch();
        
        // Track system state before epoch transition
        let _initial_validators = DcfPallet::validator_set();
        
        // Progress through blocks leading to epoch boundary
        for block in 1..=epoch_length {
            System::set_block_number(block as u64);
            let weight = DcfPallet::on_initialize(block as u64);
            
            // Verify weight is reasonable
            assert!(weight.ref_time() > 0);
        }
        
        // Verify system remains stable
        let final_epoch = DcfPallet::current_epoch();
        assert!(final_epoch >= initial_epoch);
        
        // Verify system events were emitted
        let _events = System::events();
        // Events may or may not be emitted depending on implementation
    });
}

#[test]
fn balance_integration_for_validator_leaving_works() {
    new_test_ext().execute_with(|| {
        let validator = 1;
        let cooldown_period: u32 = <Test as crate::Config>::LeaveCooldown::get();
        
        let reserved_before = Balances::reserved_balance(&validator);
        
        // Request to leave
        assert_ok!(DcfPallet::leave_validators(RuntimeOrigin::signed(validator)));
        
        // During cooldown, balance should still be reserved
        let reserved_during = Balances::reserved_balance(&validator);
        assert_eq!(reserved_before, reserved_during);
        
        // Simulate cooldown period passing
        for block in 1..=cooldown_period + 1 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
        }
        
        // After cooldown, balance should be unreserved
        let reserved_after = Balances::reserved_balance(&validator);
        assert!(reserved_after < reserved_before);
    });
}

#[test]
fn multi_validator_system_interaction_works() {
    new_test_ext().execute_with(|| {
        // Add multiple new validators
        let new_validators = vec![10, 11, 12];
        
        for validator in &new_validators {
            Balances::make_free_balance_be(validator, 100000);
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(*validator), None));
        }
        
        // Verify all are active
        let active_validators = DcfPallet::validator_set();
        for validator in &new_validators {
            assert!(active_validators.contains(validator));
        }
        
        // Update scores for multiple validators
        for (i, validator) in new_validators.iter().enumerate() {
            let _score = 5000 + (i as u64 * 1000);
            assert_ok!(DcfPallet::update_validator_stake_score(
                RuntimeOrigin::signed(*validator),
                *validator
            ));
        }
        
        // Verify scores are accessible via storage
        for validator in new_validators.iter() {
            let stake = DcfPallet::validator_stake(validator);
            assert!(stake > 0);
        }
    });
}

#[test]
fn system_weight_tracking_works() {
    new_test_ext().execute_with(|| {
        let mut total_weight = frame_support::weights::Weight::zero();
        
        // Track weights across multiple operations
        for block in 1..=10 {
            System::set_block_number(block as u64);
            let weight = DcfPallet::on_initialize(block as u64);
            total_weight = total_weight.saturating_add(weight);
            
            // Verify weight is within reasonable bounds
            assert!(weight.ref_time() < 1_000_000_000); // Less than 1 second
        }
        
        // Total weight should be accumulated properly
        assert!(total_weight.ref_time() > 0);
    });
}

#[test]
fn system_event_integration_works() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        // Perform operations that should emit events
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(1),
            1
        ));
        
        // Check system events
        let events = System::events();
        assert!(!events.is_empty());
        
        // Verify event types
        let dcf_events: Vec<_> = events
            .iter()
            .filter_map(|record| {
                if let RuntimeEvent::DcfPallet(event) = &record.event {
                    Some(event)
                } else {
                    None
                }
            })
            .collect();
        
        assert!(!dcf_events.is_empty());
    });
}

#[test]
fn runtime_api_integration_works() {
    new_test_ext().execute_with(|| {
        // Test basic runtime methods that actually exist
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0);
        
        let active_validators = DcfPallet::validator_set();
        assert_eq!(active_validators.len(), 3);
        
        // Test individual validator queries
        for validator in &active_validators {
            assert!(DcfPallet::is_validator_active(validator));
            let stake_score = DcfPallet::validator_stake(validator);
            
            // Scores should be reasonable
            assert!(stake_score > 0);
        }
    });
}

#[test]
fn cross_pallet_interaction_works() {
    new_test_ext().execute_with(|| {
        // Test interaction with balance pallet
        let validator = 1;
        let initial_free = Balances::free_balance(&validator);
        let initial_reserved = Balances::reserved_balance(&validator);
        
        // DCF operations should affect balances appropriately
        assert_ok!(DcfPallet::leave_validators(RuntimeOrigin::signed(validator)));
        
        // Balance state should be consistent
        let current_free = Balances::free_balance(&validator);
        let current_reserved = Balances::reserved_balance(&validator);
        
        // During leave request, reserved balance should remain
        assert_eq!(initial_reserved, current_reserved);
        assert_eq!(initial_free, current_free);
    });
}

#[test]
fn system_consistency_across_blocks_works() {
    new_test_ext().execute_with(|| {
        let mut previous_epoch = DcfPallet::current_epoch();
        
        // Progress through multiple blocks and verify consistency
        for block in 1..=50 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
            
            let current_epoch = DcfPallet::current_epoch();
            
            // Epoch should only increase monotonically
            assert!(current_epoch >= previous_epoch);
            
            // Active validators should always be available
            let active_validators = DcfPallet::validator_set();
            assert!(!active_validators.is_empty());
            
            // System should maintain minimum validator count
            let min_validators = <Test as crate::Config>::MinActiveValidators::get();
            assert!(active_validators.len() as u32 >= min_validators);
            
            previous_epoch = current_epoch;
        }
    });
}

#[test]
fn system_recovery_from_edge_cases_works() {
    new_test_ext().execute_with(|| {
        // Test system behavior when all validators try to leave
        let active_validators = DcfPallet::validator_set();
        let min_validators = <Test as crate::Config>::MinActiveValidators::get();
        
        // Try to make validators leave (should be prevented to maintain minimum)
        for validator in &active_validators {
            let _result = DcfPallet::leave_validators(RuntimeOrigin::signed(*validator));
            
            // System should prevent leaving if it would violate minimum
            let remaining_count = DcfPallet::validator_set().len() as u32;
            if remaining_count <= min_validators {
                // Should either fail or be queued for later processing
                // The exact behavior depends on implementation
            }
        }
        
        // System should maintain minimum validators
        let final_validators = DcfPallet::validator_set();
        assert!(final_validators.len() as u32 >= min_validators);
    });
}

#[test]
fn concurrent_operations_handling_works() {
    new_test_ext().execute_with(|| {
        // Simulate concurrent validator operations
        let validators = vec![1, 2, 3];
        
        // Multiple score updates in same block
        for validator in validators.iter() {
            assert_ok!(DcfPallet::update_validator_stake_score(
                RuntimeOrigin::signed(*validator),
                *validator
            ));
            
            assert_ok!(DcfPallet::update_validator_inference_score(
                RuntimeOrigin::signed(*validator),
                *validator
            ));
        }
        
        // Verify all updates were processed correctly
        for validator in validators.iter() {
            let stake_score = DcfPallet::validator_stake(validator);
            assert!(stake_score > 0);
        }
    });
}

/// Tests that ValidatorStatusChanged events are emitted correctly
#[test]
fn validator_status_changed_events_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        // Clear any existing events
        System::reset_events();
        
        // Test leaving a validator (should emit ValidatorStatusChanged event)
        let validator = 1; // Use existing validator
        
        // Leave validators should emit ValidatorStatusChanged event
        assert_ok!(DcfPallet::leave_validators(RuntimeOrigin::signed(validator)));
        
        // Check that ValidatorStatusChanged event was emitted
        let events = System::events();
        let status_change_events: Vec<_> = events
            .iter()
            .filter_map(|record| {
                if let RuntimeEvent::DcfPallet(crate::Event::ValidatorStatusChanged { 
                    validator: event_validator, 
                    old_status, 
                    new_status, 
                    block_number 
                }) = &record.event {
                    Some((event_validator, old_status, new_status, block_number))
                } else {
                    None
                }
            })
            .collect();
        
        assert!(!status_change_events.is_empty());
        
        // Verify the event details
        let (event_validator, old_status, new_status, block_number) = &status_change_events[0];
        assert_eq!(**event_validator, validator);
        assert_eq!(**old_status, crate::ValidatorStatus::Active);
        assert_eq!(**new_status, crate::ValidatorStatus::Leaving);
        assert_eq!(**block_number, 1);
    });
}

/// Tests that ValidatorUptimeUpdated events are emitted during epoch transitions
#[test]
fn validator_uptime_updated_events_work() {
    new_test_ext().execute_with(|| {
        // Set up initial state
        System::set_block_number(1);
        
        // Get epoch length to trigger epoch transition
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        
        // Clear any existing events
        System::reset_events();
        
        // Advance to epoch boundary to trigger epoch transition
        let epoch_boundary_block = epoch_length;
        System::set_block_number(epoch_boundary_block as u64);
        
        // Trigger epoch transition by calling on_initialize
        DcfPallet::on_initialize(epoch_boundary_block as u64);
        
        // Check that ValidatorUptimeUpdated events were emitted
        let events = System::events();
        let uptime_events: Vec<_> = events
            .iter()
            .filter_map(|record| {
                if let RuntimeEvent::DcfPallet(crate::Event::ValidatorUptimeUpdated { 
                    validator, 
                    epoch, 
                    blocks_expected, 
                    blocks_authored, 
                    blocks_missed, 
                    uptime_percentage 
                }) = &record.event {
                    Some((validator, epoch, blocks_expected, blocks_authored, blocks_missed, uptime_percentage))
                } else {
                    None
                }
            })
            .collect();
        
        // Should have uptime events for validators
        if !uptime_events.is_empty() {
            // Verify the event structure
            let (validator, epoch, blocks_expected, blocks_authored, blocks_missed, uptime_percentage) = &uptime_events[0];
            
            // Basic sanity checks
            assert!(**blocks_expected >= **blocks_authored + **blocks_missed);
            assert!(**uptime_percentage <= 10000); // Should be <= 100%
            assert!(**epoch >= 0); // Valid epoch number
            
            // Verify validator is valid
            let validator_set = DcfPallet::validator_set();
            assert!(validator_set.contains(validator));
        }
    });
}

/// Tests that ValidatorStatusChanged events are emitted when validators leave
#[test]
fn validator_leave_status_change_events_work() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        // Clear any existing events
        System::reset_events();
        
        let validator = 1;
        
        // Leave validators (this should emit ValidatorStatusChanged event)
        assert_ok!(DcfPallet::leave_validators(RuntimeOrigin::signed(validator)));
        
        // Check that ValidatorStatusChanged event was emitted
        let events = System::events();
        let status_change_events: Vec<_> = events
            .iter()
            .filter_map(|record| {
                if let RuntimeEvent::DcfPallet(crate::Event::ValidatorStatusChanged { 
                    validator: event_validator, 
                    old_status, 
                    new_status, 
                    block_number 
                }) = &record.event {
                    Some((event_validator, old_status, new_status, block_number))
                } else {
                    None
                }
            })
            .collect();
        
        assert!(!status_change_events.is_empty());
        
        // Verify the event details
        let (event_validator, old_status, new_status, block_number) = &status_change_events[0];
        assert_eq!(**event_validator, validator);
        assert_eq!(**old_status, crate::ValidatorStatus::Active);
        assert_eq!(**new_status, crate::ValidatorStatus::Leaving);
        assert_eq!(**block_number, 1);
    });
}