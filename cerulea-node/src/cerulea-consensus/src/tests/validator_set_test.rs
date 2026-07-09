//! Tests for the ValidatorSet module

#[cfg(test)]
mod tests {
    use crate::validator_set::*;
    use crate::mock::*;
    use sp_core::{ed25519::{Pair, Public}, Pair as PairTrait};

    use std::collections::HashSet;

    fn create_test_validator_id(seed: u8) -> u64 {
        seed as u64
    }

    fn create_test_authority_id(seed: u8) -> u64 {
        seed as u64 
    }

    #[test]
    fn test_validator_set_creation() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            assert!(!validators.is_empty());
            assert_eq!(validators.len(), 4);
            
            // Verify all validators have valid IDs
            for validator in validators.iter() {
                assert!(*validator > 0);
            }
        });
    }

    #[test]
    fn test_validator_set_uniqueness() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            let validator_set: HashSet<_> = validators.iter().collect();
            
            // All validators should be unique
            assert_eq!(validators.len(), validator_set.len());
        });
    }

    #[test]
    fn test_active_validator_management() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let active_validators = pallet_cbc_dcf::ActiveValidators::<Test>::get();
            let validator_set = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            
            // Active validators should be a subset of validator set
            for active_validator in active_validators.iter() {
                assert!(validator_set.contains(active_validator));
            }
            
            // Initially, all validators should be active
            assert_eq!(active_validators.len(), validator_set.len());
        });
    }

    #[test]
    fn test_validator_state_consistency() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            
            for validator in validators.iter() {
                // Each validator should have a state
                let state = pallet_cbc_dcf::ValidatorStates::<Test>::get(validator);
                assert!(state.is_some());
                
                let state = state.unwrap();
                
                // State should have valid values
                assert!(state.current.final_score >= 0);
                assert_eq!(state.current.epoch, 0);
                assert!(state.participation_rate <= 100);
                
                // Each validator should have a stake
                let stake = pallet_cbc_dcf::ValidatorStake::<Test>::get(validator);
                assert!(stake > 0);
            }
        });
    }

    #[test]
    fn test_validator_scoring_system() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            
            for validator in validators.iter() {
                let state = pallet_cbc_dcf::ValidatorStates::<Test>::get(validator).unwrap();
                
                // Test score components
                assert!(state.current.stake_score > 0);
                assert!(state.current.inference_score >= 0);
                assert!(state.current.final_score > 0);
                
                // Test score bounds
                let max_score = <Test as pallet_cbc_dcf::Config>::MaxValidatorScore::get();
                assert!(state.current.final_score <= max_score);
                
                // Test participation rate bounds
                assert!(state.participation_rate <= 100);
            }
        });
    }

    #[test]
    fn test_validator_activity_metrics() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validator = 1u64;
            let mut state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
            
            // Test activity updates
            state.current.authored_blocks = 10;
            state.current.missed_blocks = 2;
            state.last_active_block = 150;
            state.uptime = 95;
            
            pallet_cbc_dcf::ValidatorStates::<Test>::insert(&validator, &state);
            
            // Verify updates
            let updated_state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
            assert_eq!(updated_state.current.authored_blocks, 10);
            assert_eq!(updated_state.current.missed_blocks, 2);
            assert_eq!(updated_state.last_active_block, 150);
            assert_eq!(updated_state.uptime, 95);
        });
    }

    #[test]
    fn test_validator_performance_calculation() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validator = 1u64;
            let mut state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
            
            // Set up performance metrics
            state.current.authored_blocks = 20;
            state.current.missed_blocks = 1;
            state.participation_rate = 95;
            state.inference_success_count = 18;
            state.inference_count = 20;
            
            pallet_cbc_dcf::ValidatorStates::<Test>::insert(&validator, &state);
            
            // Verify performance metrics are reasonable
            let updated_state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
            
            // Success rate should be high
            if updated_state.inference_count > 0 {
                let success_rate = (updated_state.inference_success_count * 100) / (updated_state.inference_count as u32);
                assert!(success_rate >= 80); // At least 80% success rate
            }
            
            // Participation should be high
            assert!(updated_state.participation_rate >= 90);
            
            // Block production should be good (low miss rate)
            let total_blocks = updated_state.current.authored_blocks + updated_state.current.missed_blocks;
            if total_blocks > 0 {
                let miss_rate = (updated_state.current.missed_blocks * 100) / total_blocks;
                assert!(miss_rate <= 10); // At most 10% miss rate
            }
        });
    }

    #[test]
    fn test_validator_set_rotation() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let initial_validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            let initial_count = initial_validators.len();
            
            // Test adding a new validator
            let new_validator = 10u64;
            let mut updated_validators = initial_validators.clone();
            
            if updated_validators.try_push(new_validator).is_ok() {
                pallet_cbc_dcf::ValidatorSet::<Test>::put(&updated_validators);
                
                // Create state for new validator
                let new_state = pallet_cbc_dcf::ValidatorState {
                    last_active_epoch: 0,
                    current: pallet_cbc_dcf::EpochStats {
                        epoch: 0,
                        stake_score: 1000,
                        inference_score: 800,
                        final_score: 900,
                        authored_blocks: 0,
                        missed_blocks: 0,
                    },
                    history: frame_support::BoundedVec::default(),
                    uptime: 0,
                    inference_success_count: 0,
                    participation_rate: 100,
                    inference_count: 0,
                    last_active_block: 0,
                    name: None,
                    trust_score: 0,
                };
                
                pallet_cbc_dcf::ValidatorStates::<Test>::insert(&new_validator, new_state);
                pallet_cbc_dcf::ValidatorStake::<Test>::insert(&new_validator, 1000u128);
                
                // Verify validator was added
                let current_validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
                assert_eq!(current_validators.len(), initial_count + 1);
                assert!(current_validators.contains(&new_validator));
            }
        });
    }

    #[test]
    fn test_validator_removal() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let initial_validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            let validator_to_remove = initial_validators[0];
            
            // Remove validator from set
            let mut updated_validators = initial_validators.clone();
            if let Some(pos) = updated_validators.iter().position(|v| *v == validator_to_remove) {
                updated_validators.remove(pos);
                pallet_cbc_dcf::ValidatorSet::<Test>::put(&updated_validators);
                
                // Remove from active validators too
                let mut active_validators = pallet_cbc_dcf::ActiveValidators::<Test>::get();
                if let Some(pos) = active_validators.iter().position(|v| *v == validator_to_remove) {
                    active_validators.remove(pos);
                    pallet_cbc_dcf::ActiveValidators::<Test>::put(&active_validators);
                }
                
                // Verify validator was removed
                let current_validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
                assert!(!current_validators.contains(&validator_to_remove));
                
                let current_active = pallet_cbc_dcf::ActiveValidators::<Test>::get();
                assert!(!current_active.contains(&validator_to_remove));
            }
        });
    }

    #[test]
    fn test_validator_set_limits() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let max_validators = 100u32; // DcfMaxValidators constant value
            let current_validators = pallet_cbc_dcf::ValidatorSet::<Test>::get();
            
            // Current validator count should not exceed maximum
            assert!(current_validators.len() <= max_validators as usize);
            
            // Test minimum active validators
            let min_active = 3u32; // MinActiveValidators constant value
            let active_validators = pallet_cbc_dcf::ActiveValidators::<Test>::get();
            
            // Should have at least minimum active validators
            assert!(active_validators.len() >= min_active as usize);
        });
    }

    #[test]
    fn test_validator_epoch_history() {
        setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
            let validator = 1u64;
            let mut state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
            
            // Test adding epoch history
            let epoch_stats = pallet_cbc_dcf::EpochStats {
                epoch: 1,
                stake_score: 1100,
                inference_score: 850,
                final_score: 950,
                authored_blocks: 5,
                missed_blocks: 0,
            };
            
            // Add to history (if there's space)
            if state.history.try_push(epoch_stats.clone()).is_ok() {
                pallet_cbc_dcf::ValidatorStates::<Test>::insert(&validator, &state);
                
                // Verify history was added
                let updated_state = pallet_cbc_dcf::ValidatorStates::<Test>::get(&validator).unwrap();
                assert!(!updated_state.history.is_empty());
                
                if let Some(last_epoch) = updated_state.history.last() {
                    assert_eq!(last_epoch.epoch, 1);
                    assert_eq!(last_epoch.final_score, 950);
                }
            }
        });
    }
}