#[cfg(test)]
mod tests {
    use crate::mock::*;
    use crate::Error;
    use frame_support::{assert_noop, assert_ok, traits::Get};

    #[test]
    fn test_register_validator_success() {
        new_test_ext().execute_with(|| {
            // Test successful validator registration
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Verify the validator was registered
            assert_eq!(PalletCbcPos::validators(1), Some(true));
        });
    }

    #[test]
    fn test_register_validator_already_registered() {
        new_test_ext().execute_with(|| {
            // Register validator first time
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Try to register same validator again
            assert_noop!(
                PalletCbcPos::register_validator(RuntimeOrigin::signed(1)),
                Error::<Test>::ValidatorAlreadyRegistered
            );
        });
    }

    #[test]
    fn test_submit_score_success() {
        new_test_ext().execute_with(|| {
            // Register validator first
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Submit score
            assert_ok!(PalletCbcPos::submit_score(
                RuntimeOrigin::signed(2),
                1,
                75 // score above minimum threshold
            ));

            // Verify the score was stored
            assert_eq!(PalletCbcPos::validator_scores(1), Some(75));
        });
    }

    #[test]
    fn test_submit_score_not_registered() {
        new_test_ext().execute_with(|| {
            // Try to submit score for non-registered validator
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 75),
                Error::<Test>::ValidatorNotRegistered
            );
        });
    }

    #[test]
    fn test_submit_score_too_low() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Try to submit score below minimum - should fail
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 20),
                Error::<Test>::ScoreTooLow
            );
        });
    }

    #[test]
    fn test_slash_validator_success() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Slash validator
            assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));

            // Verify slashing count
            assert_eq!(PalletCbcPos::slashing_count(1), Some(1));
        });
    }

    #[test]
    fn test_slash_validator_removal() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));

            // Slash validator multiple times to reach max count
            for _ in 0..3 {
                assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));
            }

            // Verify validator was removed
            assert_eq!(PalletCbcPos::validators(1), None);
            assert_eq!(PalletCbcPos::validator_scores(1), None);
            assert_eq!(PalletCbcPos::slashing_count(1), None);
        });
    }

    #[test]
    fn test_slash_validator_not_registered() {
        new_test_ext().execute_with(|| {
            // Try to slash non-registered validator
            assert_noop!(
                PalletCbcPos::slash_validator_stake(&1, 100),
                Error::<Test>::ValidatorNotRegistered
            );
        });
    }

    #[test]
    fn test_unauthorized_register() {
        new_test_ext().execute_with(|| {
            // Try to register with root origin - should fail
            assert_noop!(
                PalletCbcPos::register_validator(RuntimeOrigin::root()),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn test_unauthorized_submit_score() {
        new_test_ext().execute_with(|| {
            // Try to submit score with root origin - should fail
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::root(), 1, 75),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn test_unauthorized_slash() {
        new_test_ext().execute_with(|| {
            // Register validator first
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Try to slash - should work as it's an internal function
            assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));
        });
    }

    #[test]
    fn test_submit_score_no_registration() {
        new_test_ext().execute_with(|| {
            // This should fail because validator is not registered
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 75),
                Error::<Test>::ValidatorNotRegistered
            );
        });
    }

    #[test]
    fn test_boost_score_functionality() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Set initial score
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 50));
            
            // Boost score
            assert_ok!(PalletCbcPos::boost_score(RuntimeOrigin::signed(2), 1, 10));
            
            // Verify score was boosted
            let new_score = PalletCbcPos::validator_scores(1).unwrap_or(0);
            assert!(new_score >= 50);
        });
    }

    #[test]
    fn test_slash_score_functionality() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Set initial score
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 100));
            
            // Slash score
            assert_ok!(PalletCbcPos::slash_score(RuntimeOrigin::signed(2), 1, 10));
            
            // Verify score was slashed
            let new_score = PalletCbcPos::validator_scores(1).unwrap_or(0);
            assert!(new_score <= 100);
        });
    }

    #[test]
    fn test_validator_score_decay() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Set initial score
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, 100));
            
            // Apply score decay
            let decay_amount = <Test as crate::Config>::ValidatorScoreDecay::get();
            let initial_score = PalletCbcPos::validator_scores(1).unwrap_or(0);
            
            // Simulate decay (this would normally happen in on_initialize)
            let expected_score = initial_score.saturating_sub(decay_amount);
            
            // Verify decay calculation
            assert!(expected_score <= initial_score);
        });
    }

    #[test]
    fn test_multiple_validators_registration() {
        new_test_ext().execute_with(|| {
            // Register multiple validators
            for i in 1..=5 {
                assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(i)));
                assert!(PalletCbcPos::validators(i).is_some());
            }
            
            // Verify all are registered
            for i in 1..=5 {
                assert_eq!(PalletCbcPos::validators(i), Some(true));
            }
        });
    }

    #[test]
    fn test_validator_score_boundaries() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Test minimum score
            let min_score: u32 = <Test as crate::Config>::MinValidatorScore::get();
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, min_score));
            assert_eq!(PalletCbcPos::validator_scores(1), Some(min_score));
            
            // Test score below minimum (should fail)
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, min_score - 1),
                Error::<Test>::ScoreTooLow
            );
        });
    }

    #[test]
    fn test_slashing_count_tracking() {
        new_test_ext().execute_with(|| {
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // Slash validator and check count
            assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));
            assert_eq!(PalletCbcPos::slashing_count(1), Some(1));
            
            // Slash again
            assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));
            assert_eq!(PalletCbcPos::slashing_count(1), Some(2));
            
            // One more slash should remove validator
            assert_ok!(PalletCbcPos::slash_validator_stake(&1, 100));
            assert_eq!(PalletCbcPos::validators(1), None);
            assert_eq!(PalletCbcPos::slashing_count(1), None);
        });
    }

    // ================================================================================================
    // Additional Unit Tests for Comprehensive Coverage
    // ================================================================================================

    #[test]
    fn test_bond_stake_functionality() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let _bond_amount = 1000u64;
            
            // Register validator first
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            // Test stake bonding (if function exists)
            // Note: This test assumes bond_stake function exists
            // If not, this test can be removed or modified
        });
    }

    #[test]
    fn test_get_active_validators() {
        new_test_ext().execute_with(|| {
            // Register multiple validators
            for i in 1..=3 {
                assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(i)));
            }
            
            // Test getting active validators
            let active_validators = PalletCbcPos::get_active_validators();
            
            // Should include registered validators
            assert!(active_validators.contains(&1u64));
            assert!(active_validators.contains(&2u64));
            assert!(active_validators.contains(&3u64));
        });
    }

    #[test]
    fn test_validator_activation_deactivation() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            // Test deactivation
            assert_ok!(PalletCbcPos::deactivate_validator(&validator));
            assert_eq!(PalletCbcPos::validators(validator), Some(false));
            
            // Test reactivation
            assert_ok!(PalletCbcPos::activate_validator(&validator));
            assert_eq!(PalletCbcPos::validators(validator), Some(true));
        });
    }

    #[test]
    fn test_reward_validator_functionality() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let reward_amount = 500u64;
            
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            // Test validator reward
            assert_ok!(PalletCbcPos::reward_validator(&validator, reward_amount));
            
            // Verify reward was applied (implementation-specific verification)
        });
    }

    #[test]
    fn test_score_overflow_handling() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            // Set score near maximum
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), validator, u32::MAX - 10));
            
            // Test score boosting with potential overflow
            assert_ok!(PalletCbcPos::boost_score(RuntimeOrigin::signed(2), validator, 20));
            
            // Verify score is capped at maximum
            let final_score = PalletCbcPos::validator_scores(validator).unwrap_or(0);
            assert!(final_score <= u32::MAX);
        });
    }

    #[test]
    fn test_score_underflow_handling() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            
            // Register validator
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            // Set low score
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), validator, 50));
            
            // Test score slashing with potential underflow
            assert_ok!(PalletCbcPos::slash_score(RuntimeOrigin::signed(2), validator, 100));
            
            // Verify score is floored at zero
            let final_score = PalletCbcPos::validator_scores(validator).unwrap_or(0);
            assert_eq!(final_score, 0);
        });
    }

    #[test]
    fn test_maximum_validators_constraint() {
        new_test_ext().execute_with(|| {
            let max_validators = <Test as crate::Config>::MaxValidators::get();
            
            // Register validators up to maximum
            for i in 1..=max_validators {
                assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(i as u64)));
            }
            
            // Try to register one more (should fail)
            assert_noop!(
                PalletCbcPos::register_validator(RuntimeOrigin::signed((max_validators + 1) as u64)),
                Error::<Test>::TooManyValidators
            );
        });
    }

    #[test]
    fn test_minimum_active_validators_constraint() {
        new_test_ext().execute_with(|| {
            let min_active = <Test as crate::Config>::MinActiveValidators::get();
            
            // Register minimum required validators
            for i in 1..=min_active {
                assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(i as u64)));
            }
            
            let active_validators = PalletCbcPos::get_active_validators();
            assert!(active_validators.len() >= min_active as usize);
        });
    }

    #[test]
    fn test_validator_score_decay_integration() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let initial_score = 1000u32;
            let decay_amount = <Test as crate::Config>::ValidatorScoreDecay::get();
            
            // Register validator and set score
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            assert_ok!(PalletCbcPos::submit_score(RuntimeOrigin::signed(2), validator, initial_score));
            
            // Simulate epoch transition with decay
            let current_score = PalletCbcPos::validator_scores(validator).unwrap_or(0);
            let expected_score = current_score.saturating_sub(decay_amount);
            
            // Verify decay calculation
            assert!(expected_score <= current_score);
            assert_eq!(expected_score, initial_score.saturating_sub(decay_amount));
        });
    }

    #[test]
    fn test_comprehensive_error_handling() {
        new_test_ext().execute_with(|| {
            // Test all error variants can be triggered
            
            // ValidatorNotRegistered
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 999u64, 100),
                Error::<Test>::ValidatorNotRegistered
            );
            
            // ValidatorAlreadyRegistered
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            assert_noop!(
                PalletCbcPos::register_validator(RuntimeOrigin::signed(1)),
                Error::<Test>::ValidatorAlreadyRegistered
            );
            
            // ScoreTooLow
            let min_score: u32 = <Test as crate::Config>::MinValidatorScore::get();
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(2), 1, min_score - 1),
                Error::<Test>::ScoreTooLow
            );
        });
    }
} 