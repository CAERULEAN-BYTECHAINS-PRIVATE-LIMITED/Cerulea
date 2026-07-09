//! Runtime-level integration tests for the CBC system
//! 
//! These tests verify that all pallets work together correctly within the runtime
//! and that the runtime configuration is properly set up.

#[cfg(test)]
mod tests {
    use crate::mock::*;
    use frame_support::{assert_ok, assert_noop, traits::Currency};
    use sp_runtime::traits::Zero;

    #[test]
    fn test_runtime_pallet_integration() {
        new_test_ext().execute_with(|| {
            // Test that all pallets are properly integrated
            
            // System pallet
            assert_eq!(System::block_number(), 1);
            assert!(System::account_exists(&1));
            
            // Balances pallet
            assert!(Balances::free_balance(&1) > Zero::zero());
            
            // Timestamp pallet
            let _ = pallet_timestamp::Pallet::<Runtime>::now();
            
            // Sudo pallet
            assert!(pallet_sudo::Pallet::<Runtime>::key().is_some());
            
            // DCF pallet
            let validators = DcfPallet::validator_set();
            assert!(!validators.is_empty());
            
            // PoS pallet integration
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(1)));
            
            // PoI pallet integration
            assert_ok!(PalletCbcPoi::submit_inference(RuntimeOrigin::signed(1), 42, 80));
        });
    }

    #[test]
    fn test_runtime_constants() {
        new_test_ext().execute_with(|| {
            // Test that runtime constants are properly configured
            
            // System constants
            assert_eq!(System::block_hash_count(), 2400);
            
            // DCF constants
            let max_validators = pallet_cbc_dcf::DcfMaxValidators::<Runtime>::get();
            assert_eq!(max_validators, 100);
            
            let min_active = pallet_cbc_dcf::MinActiveValidators::<Runtime>::get();
            assert_eq!(min_active, 3);
            
            let epoch_length = pallet_cbc_dcf::EpochLength::<Runtime>::get();
            assert_eq!(epoch_length, 2400);
            
            // PoS constants
            let min_validator_score = pallet_cbc_pos::MinValidatorScore::<Runtime>::get();
            assert_eq!(min_validator_score, 50);
            
            // PoI constants
            let min_confidence = pallet_cbc_poi::MinInferenceConfidence::<Runtime>::get();
            assert_eq!(min_confidence, 50);
            
            let challenge_window = pallet_cbc_poi::ChallengeWindow::<Runtime>::get();
            assert_eq!(challenge_window, 5);
        });
    }

    #[test]
    fn test_runtime_weights() {
        new_test_ext().execute_with(|| {
            // Test that runtime weights are properly configured
            use crate::configs::weights::*;
            
            // Verify weights are non-zero
            assert!(CBC_DCF_WEIGHT.ref_time() > 0);
            assert!(CBC_POS_WEIGHT.ref_time() > 0);
            assert!(CBC_POI_WEIGHT.ref_time() > 0);
            
            // Verify operation-specific weights
            assert!(DCF_EPOCH_TRANSITION_WEIGHT.ref_time() > DCF_JOIN_VALIDATORS_WEIGHT.ref_time());
            assert!(DCF_EXECUTE_PROPOSAL_WEIGHT.ref_time() > DCF_PROPOSE_REWARD_WEIGHT.ref_time());
            
            // Verify PoI operations are appropriately weighted
            assert!(POI_CHALLENGE_INFERENCE_WEIGHT.ref_time() > POI_SUBMIT_INFERENCE_WEIGHT.ref_time());
        });
    }

    #[test]
    fn test_runtime_genesis_configuration() {
        new_test_ext().execute_with(|| {
            // Test that genesis configuration is applied correctly
            
            // Verify validators were set up
            let validators = DcfPallet::validator_set();
            assert_eq!(validators.len(), 4);
            assert!(validators.contains(&1));
            assert!(validators.contains(&2));
            assert!(validators.contains(&3));
            assert!(validators.contains(&4));
            
            // Verify validator states
            for validator in validators.iter() {
                let state = DcfPallet::validator_states(validator);
                assert!(state.is_some());
                
                let state = state.unwrap();
                assert_eq!(state.current.epoch, 0);
                assert!(state.current.final_score > 0);
            }
            
            // Verify stakes
            for validator in validators.iter() {
                let stake = DcfPallet::validator_stake(validator);
                assert_eq!(stake, 10000);
            }
            
            // Verify balances
            for i in 1..=5 {
                let balance = Balances::free_balance(&i);
                assert!(balance >= 90000); // Should have balance minus reserved stake
            }
        });
    }

    #[test]
    fn test_runtime_consensus_weights() {
        new_test_ext().execute_with(|| {
            // Test consensus weight configuration
            let pos_weight = DcfPallet::pos_weight();
            let poi_weight = DcfPallet::poi_weight();
            
            assert_eq!(pos_weight, 60);
            assert_eq!(poi_weight, 40);
            assert_eq!(pos_weight + poi_weight, 100);
            
            // Test weight updates
            assert_ok!(DcfPallet::update_consensus_weights(
                RuntimeOrigin::root(),
                70,
                30
            ));
            
            assert_eq!(DcfPallet::pos_weight(), 70);
            assert_eq!(DcfPallet::poi_weight(), 30);
        });
    }

    #[test]
    fn test_runtime_governance_integration() {
        new_test_ext_for_governance().execute_with(|| {
            let proposer = 1u64;
            let target = 2u64;
            
            // Test governance proposal lifecycle
            assert_ok!(DcfPallet::propose_reward_validator(
                RuntimeOrigin::root(),
                proposer,
                target,
                1000u128
            ));
            
            let proposal_id = 0u32;
            
            // Test voting
            assert_ok!(DcfPallet::vote_proposal(
                RuntimeOrigin::signed(proposer),
                proposal_id,
                true
            ));
            
            assert_ok!(DcfPallet::vote_proposal(
                RuntimeOrigin::signed(target),
                proposal_id,
                true
            ));
            
            // Test execution
            assert_ok!(DcfPallet::execute_proposal(
                RuntimeOrigin::root(),
                proposal_id
            ));
            
            // Verify proposal was executed
            let proposal = pallet_cbc_dcf::Proposals::<Runtime>::get(proposal_id).unwrap();
            assert_eq!(proposal.status, pallet_cbc_dcf::ProposalStatus::Executed);
        });
    }

    #[test]
    fn test_runtime_epoch_management() {
        new_test_ext_for_epochs().execute_with(|| {
            let initial_epoch = DcfPallet::current_epoch();
            
            // Test epoch configuration
            let epoch_config = pallet_cbc_dcf::EpochConfigStorage::<Runtime>::get();
            assert_eq!(epoch_config.blocks_per_epoch, 100);
            assert_eq!(epoch_config.min_stake, 1000);
            assert_eq!(epoch_config.max_validators, 100);
            
            // Test epoch transition
            advance_to_next_epoch();
            
            let new_epoch = DcfPallet::current_epoch();
            assert!(new_epoch > initial_epoch);
            
            // Verify validators are still active after epoch transition
            let validators = DcfPallet::validator_set();
            assert!(!validators.is_empty());
        });
    }

    #[test]
    fn test_runtime_performance_with_many_validators() {
        new_test_ext_with_validators(50).execute_with(|| {
            let validators = DcfPallet::validator_set();
            assert_eq!(validators.len(), 50);
            
            use std::time::Instant;
            
            // Test performance of operations with many validators
            let start = Instant::now();
            
            // Update scores for multiple validators
            for validator in validators.iter().take(20) {
                let _ = DcfPallet::update_validator_stake_score(
                    RuntimeOrigin::signed(*validator),
                    *validator
                );
            }
            
            let duration = start.elapsed();
            assert!(duration.as_millis() < 1000);
            
            // Test epoch transition performance
            let start = Instant::now();
            advance_to_next_epoch();
            let duration = start.elapsed();
            
            // Should complete within reasonable time
            assert!(duration.as_millis() < 5000);
        });
    }

    #[test]
    fn test_runtime_error_handling() {
        new_test_ext().execute_with(|| {
            let invalid_validator = 999u64;
            
            // Test that runtime properly handles errors from all pallets
            
            // DCF errors
            assert_noop!(
                DcfPallet::leave_validators(RuntimeOrigin::signed(invalid_validator)),
                pallet_cbc_dcf::Error::<Runtime>::ValidatorNotFound
            );
            
            // PoS errors
            assert_noop!(
                PalletCbcPos::submit_score(RuntimeOrigin::signed(1), invalid_validator, 75),
                pallet_cbc_pos::Error::<Runtime>::ValidatorNotRegistered
            );
            
            // PoI errors
            assert_noop!(
                PalletCbcPoi::challenge_inference(RuntimeOrigin::signed(1), invalid_validator, 42),
                pallet_cbc_poi::Error::<Runtime>::InferenceNotFound
            );
            
            // System errors
            assert_noop!(
                System::remark(RuntimeOrigin::none(), vec![]),
                sp_runtime::DispatchError::BadOrigin
            );
        });
    }

    #[test]
    fn test_runtime_economic_model() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let initial_balance = Balances::free_balance(&validator);
            
            // Test economic interactions across pallets
            
            // Stake in DCF should reserve balance
            let stake_amount = 5000u128;
            setup_validator_with_stake(validator, stake_amount);
            
            let free_balance = Balances::free_balance(&validator);
            let reserved_balance = Balances::reserved_balance(&validator);
            
            assert_eq!(reserved_balance, stake_amount);
            assert_eq!(free_balance + reserved_balance, initial_balance);
            
            // Slashing should reduce reserved balance
            let slash_amount = 1000u128;
            assert_ok!(DcfPallet::slash_validator(
                RuntimeOrigin::root(),
                validator,
                slash_amount
            ));
            
            let new_stake = DcfPallet::validator_stake(&validator);
            assert_eq!(new_stake, stake_amount - slash_amount);
            
            // Test that economic model is consistent
            let new_reserved = Balances::reserved_balance(&validator);
            assert_eq!(new_reserved, new_stake);
        });
    }

    #[test]
    fn test_runtime_api_consistency() {
        new_test_ext().execute_with(|| {
            // Test that runtime APIs return consistent data
            
            let validators = DcfPallet::validator_set();
            let active_validators = DcfPallet::active_validators();
            
            // Active validators should be subset of all validators
            for active in active_validators.iter() {
                assert!(validators.contains(active));
            }
            
            // Test validator profiles
            for validator in validators.iter() {
                let profile = DcfPallet::get_validator_profile(*validator);
                assert!(profile.is_some());
                
                if let Some((combined_score, pos_score, poi_score, trust_score, uptime, inference_count, participation_rate, missed_blocks)) = profile {
                    // Scores should be reasonable
                    assert!(combined_score <= pallet_cbc_dcf::MaxValidatorScore::<Runtime>::get());
                    assert!(pos_score <= pallet_cbc_dcf::MaxValidatorScore::<Runtime>::get());
                    assert!(poi_score <= pallet_cbc_dcf::MaxValidatorScore::<Runtime>::get());
                    assert!(trust_score <= pallet_cbc_dcf::MaxTrustScore::<Runtime>::get());
                    
                    // Participation rate should be valid percentage
                    assert!(participation_rate <= 100);
                    
                    // Counts should be non-negative
                    assert!(uptime <= 100);
                    assert!(inference_count >= 0);
                    assert!(missed_blocks >= 0);
                }
            }
        });
    }

    #[test]
    fn test_runtime_storage_consistency() {
        new_test_ext().execute_with(|| {
            // Test that storage across pallets is consistent
            
            let validators = DcfPallet::validator_set();
            
            for validator in validators.iter() {
                // DCF storage
                let dcf_state = DcfPallet::validator_states(validator);
                assert!(dcf_state.is_some());
                
                let dcf_stake = DcfPallet::validator_stake(validator);
                assert!(dcf_stake > 0);
                
                // Balance storage should match DCF stake
                let reserved_balance = Balances::reserved_balance(validator);
                assert_eq!(reserved_balance, dcf_stake);
                
                // Total balance should be consistent
                let free_balance = Balances::free_balance(validator);
                let total_balance = free_balance + reserved_balance;
                assert!(total_balance >= dcf_stake);
            }
        });
    }

    #[test]
    fn test_runtime_upgrade_simulation() {
        new_test_ext().execute_with(|| {
            // Simulate runtime upgrade by checking state consistency
            
            let initial_validators = DcfPallet::validator_set();
            let initial_epoch = DcfPallet::current_epoch();
            let initial_balances: Vec<_> = initial_validators
                .iter()
                .map(|v| (*v, Balances::free_balance(v), Balances::reserved_balance(v)))
                .collect();
            
            // Advance several blocks to simulate runtime activity
            advance_blocks(50);
            
            // Verify state consistency after activity
            let current_validators = DcfPallet::validator_set();
            assert_eq!(current_validators.len(), initial_validators.len());
            
            // Verify balances are still consistent
            for (validator, initial_free, initial_reserved) in initial_balances {
                let current_free = Balances::free_balance(&validator);
                let current_reserved = Balances::reserved_balance(&validator);
                
                // Total balance should not change (ignoring rewards/slashing for this test)
                let initial_total = initial_free + initial_reserved;
                let current_total = current_free + current_reserved;
                
                // Allow for small differences due to operations
                let diff = if current_total > initial_total {
                    current_total - initial_total
                } else {
                    initial_total - current_total
                };
                assert!(diff <= 1000); // Small tolerance for test operations
            }
        });
    }

    #[test]
    fn test_runtime_event_emission() {
        new_test_ext().execute_with(|| {
            // Test that runtime properly emits events from all pallets
            
            let validator = 1u64;
            
            // Clear any existing events
            System::reset_events();
            
            // Perform operations that should emit events
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            assert_ok!(PalletCbcPoi::submit_inference(RuntimeOrigin::signed(validator), 42, 80));
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(validator), None));
            
            // Verify events were emitted
            let events = System::events();
            assert!(!events.is_empty());
            
            // Events should include events from multiple pallets
            let event_pallets: std::collections::HashSet<_> = events
                .iter()
                .map(|record| {
                    match &record.event {
                        RuntimeEvent::PalletCbcPos(_) => "PoS",
                        RuntimeEvent::PalletCbcPoi(_) => "PoI", 
                        RuntimeEvent::DcfPallet(_) => "DCF",
                        RuntimeEvent::System(_) => "System",
                        RuntimeEvent::Balances(_) => "Balances",
                        _ => "Other",
                    }
                })
                .collect();
            
            // Should have events from multiple pallets
            assert!(event_pallets.len() >= 2);
        });
    }

    #[test]
    fn test_runtime_transaction_fees() {
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            let initial_balance = Balances::free_balance(&validator);
            
            // Test that transactions consume appropriate fees
            // Note: In test environment, fees might be zero or minimal
            
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            
            let balance_after_tx = Balances::free_balance(&validator);
            
            // Balance should be same or slightly less (due to fees)
            assert!(balance_after_tx <= initial_balance);
            
            // Difference should be reasonable (fees shouldn't be excessive)
            let fee_paid = initial_balance - balance_after_tx;
            assert!(fee_paid <= 1000); // Reasonable fee limit
        });
    }

    #[test]
    fn test_runtime_security_model() {
        new_test_ext().execute_with(|| {
            let attacker = 999u64;
            create_funded_account(attacker, 100000);
            
            // Test that unauthorized operations are properly rejected
            
            // Should not be able to execute privileged operations
            assert_noop!(
                DcfPallet::set_governance_mode(RuntimeOrigin::signed(attacker), true),
                sp_runtime::DispatchError::BadOrigin
            );
            
            assert_noop!(
                DcfPallet::slash_validator(RuntimeOrigin::signed(attacker), 1, 1000),
                sp_runtime::DispatchError::BadOrigin
            );
            
            // Should not be able to manipulate other validators' data directly
            assert_noop!(
                DcfPallet::update_validator_activity(
                    RuntimeOrigin::signed(attacker),
                    1, // target validator
                    100, // block number
                    0 // missed blocks
                ),
                pallet_cbc_dcf::Error::<Runtime>::Unauthorized
            );
        });
    }

    #[test]
    fn test_runtime_scalability() {
        new_test_ext_for_scenario(TestScenario::StressTest).execute_with(|| {
            let validators = DcfPallet::validator_set();
            assert_eq!(validators.len(), 50);
            
            use std::time::Instant;
            
            // Test that runtime can handle many validators efficiently
            let start = Instant::now();
            
            // Register all validators in PoS
            for validator in validators.iter().take(25) {
                let _ = PalletCbcPos::register_validator(RuntimeOrigin::signed(*validator));
            }
            
            let registration_time = start.elapsed();
            assert!(registration_time.as_millis() < 2000);
            
            // Test inference submissions
            let start = Instant::now();
            
            for (i, validator) in validators.iter().take(25).enumerate() {
                let _ = PalletCbcPoi::submit_inference(
                    RuntimeOrigin::signed(*validator),
                    (i + 1) as u32,
                    80
                );
            }
            
            let inference_time = start.elapsed();
            assert!(inference_time.as_millis() < 2000);
            
            // Test score updates
            let start = Instant::now();
            
            for validator in validators.iter().take(25) {
                let _ = DcfPallet::update_validator_stake_score(
                    RuntimeOrigin::signed(*validator),
                    *validator
                );
            }
            
            let score_update_time = start.elapsed();
            assert!(score_update_time.as_millis() < 2000);
        });
    }
}