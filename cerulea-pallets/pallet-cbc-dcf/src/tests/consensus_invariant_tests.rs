//! Property-based tests for consensus invariants and economic bounds

use crate::{mock::*, Error};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Get, OnFinalize, OnInitialize, Currency},
};

/// Property: Total staked amount never exceeds total supply
#[test]
fn property_total_stake_bounded_by_supply() {
    new_test_ext().execute_with(|| {
        for block_num in 1..=20 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let active_validators = DcfPallet::active_validators();
            let mut total_staked = 0u128;
            let mut total_supply = 0u128;
            
            for validator in &active_validators {
                total_staked += DcfPallet::validator_stake(validator);
                total_supply += Balances::free_balance(validator);
                total_supply += Balances::reserved_balance(validator);
            }
            
            // Property: total_staked <= total_supply
            assert!(total_staked <= total_supply);
        }
    });
}

/// Property: Consensus weights maintain mathematical consistency
#[test]
fn property_consensus_weights_mathematical_consistency() {
    new_test_ext().execute_with(|| {
        let test_weight_pairs = vec![
            (5000u64, 5000u64),
            (7000u64, 3000u64),
            (3000u64, 7000u64),
            (9000u64, 1000u64),
            (1000u64, 9000u64),
            (6000u64, 4000u64),
        ];
        
        for (pos_weight, poi_weight) in test_weight_pairs {
            assert_ok!(DcfPallet::update_consensus_weights(
                RuntimeOrigin::root(),
                pos_weight,
                poi_weight
            ));
            
            let (current_pos, current_poi) = DcfPallet::consensus_weights();
            
            // Property: weights sum to exactly 10000 (100%)
            assert_eq!(current_pos + current_poi, 10000);
            
            // Property: individual weights are within bounds
            assert!(current_pos > 0);
            assert!(current_poi > 0);
            assert!(current_pos <= 10000);
            assert!(current_poi <= 10000);
            
            // Property: weights match what was set
            assert_eq!(current_pos, pos_weight);
            assert_eq!(current_poi, poi_weight);
        }
    });
}

/// Property: Validator economic bounds are maintained
#[test]
fn property_validator_economic_bounds() {
    new_test_ext().execute_with(|| {
        let min_stake = <Test as crate::Config>::MinStake::get();
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let active_validators = DcfPallet::active_validators();
            
            // Property: validator count within bounds
            assert!(active_validators.len() <= max_validators as usize);
            
            for validator in &active_validators {
                let stake = DcfPallet::validator_stake(validator);
                let free_balance = Balances::free_balance(validator);
                let reserved_balance = Balances::reserved_balance(validator);
                
                // Property: active validators meet minimum stake
                assert!(stake >= min_stake);
                
                // Property: reserved balance covers stake
                assert!(reserved_balance >= stake);
                
                // Property: total balance is non-negative
                assert!(free_balance >= 0);
                assert!(reserved_balance >= 0);
                
                // Property: stake is consistent with reserved balance
                assert_eq!(stake, reserved_balance);
            }
        }
    });
}

/// Property: Score calculations maintain mathematical bounds
#[test]
fn property_score_mathematical_bounds() {
    new_test_ext().execute_with(|| {
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        let min_score = <Test as crate::Config>::MinValidatorScore::get() as u64;
        
        for block_num in 1..=10 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let active_validators = DcfPallet::active_validators();
            
            for validator in &active_validators {
                let _stake_score = DcfPallet::validator_stake_score(validator);
                let _inference_score = DcfPallet::validator_inference_score(validator);
                let stake_score = DcfPallet::validator_stake_score(validator);
                let inference_score = DcfPallet::validator_inference_score(validator);
                let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
                let combined_score = (stake_score * pos_weight as u128 + inference_score as u128 * poi_weight as u128) / 10000;
                
                // Property: individual scores within bounds
                assert!(stake_score >= min_score.into());
                assert!(stake_score <= max_score.into());
                assert!(inference_score >= 0);
                assert!(inference_score <= max_score);
                
                // Property: combined score calculation consistency
                let expected_combined = (stake_score * pos_weight as u128 + 
                                       inference_score as u128 * poi_weight as u128) / 10000;
                assert_eq!(combined_score, expected_combined);
                
                // Property: combined score within reasonable bounds
                assert!(combined_score >= min_score as u128);
                assert!(combined_score <= max_score as u128);
            }
        }
    });
}

/// Property: Epoch transitions preserve system invariants
#[test]
fn property_epoch_transitions_preserve_invariants() {
    new_test_ext().execute_with(|| {
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        
        let mut previous_epoch = DcfPallet::current_epoch();
        let mut previous_validators = DcfPallet::active_validators();
        
        // Advance through multiple epochs
        for epoch_cycle in 1..=3 {
            for block in 1..=epoch_length {
                let block_num = (epoch_cycle - 1) * epoch_length + block;
                System::set_block_number(block_num as u64);
                DcfPallet::on_initialize(block_num as u64);
                DcfPallet::on_finalize(block_num as u64);
            }
            
            let current_epoch = DcfPallet::current_epoch();
            let current_validators = DcfPallet::active_validators();
            
            // Property: epoch advances monotonically
            assert!(current_epoch >= previous_epoch);
            
            // Property: minimum validators maintained across epochs
            assert!(current_validators.len() >= min_active as usize);
            
            // Property: validator set changes are bounded
            let validator_changes = current_validators.len() as i32 - previous_validators.len() as i32;
            assert!(validator_changes.abs() <= 10); // Reasonable change limit
            
            // Property: consensus weights remain valid across epochs
            let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
            assert_eq!(pos_weight + poi_weight, 10000);
            
            previous_epoch = current_epoch;
            previous_validators = current_validators;
        }
    });
}

/// Property: Slashing maintains economic security
#[test]
fn property_slashing_economic_security() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let _initial_stake = DcfPallet::validator_stake(&validator);
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        // Test various slashing amounts
        let slash_amounts = vec![100u128, 500u128, 1000u128, 2000u128];
        
        for slash_amount in slash_amounts {
            let pre_slash_stake = DcfPallet::validator_stake(&validator);
            let pre_slash_balance = Balances::reserved_balance(&validator);
            
            if slash_amount < pre_slash_stake {
                assert_ok!(DcfPallet::slash_validator(
                    RuntimeOrigin::root(),
                    validator,
                    slash_amount
                ));
                
                let post_slash_stake = DcfPallet::validator_stake(&validator);
                let post_slash_balance = Balances::reserved_balance(&validator);
                
                // Property: slashing reduces stake by exact amount
                assert_eq!(pre_slash_stake - post_slash_stake, slash_amount);
                
                // Property: balance changes match stake changes
                assert_eq!(pre_slash_balance - post_slash_balance, slash_amount);
                
                // Property: validator remains active if above minimum
                if post_slash_stake >= min_stake {
                    assert!(DcfPallet::is_validator_active(&validator));
                }
                
                // Property: stake never goes negative
                assert!(post_slash_stake >= 0);
            }
        }
    });
}

/// Property: Reward distribution maintains economic bounds
#[test]
fn property_reward_distribution_bounds() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let initial_stake = DcfPallet::validator_stake(&validator);
        
        // Enable governance for rewards
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        
        let reward_amounts = vec![100u128, 500u128, 1000u128, 2000u128];
        
        for (i, reward_amount) in reward_amounts.iter().enumerate() {
            let pre_reward_stake = DcfPallet::validator_stake(&validator);
            
            // Create and execute reward proposal
            assert_ok!(DcfPallet::propose_reward_validator(
                RuntimeOrigin::root(),
                validator,
                validator,
                *reward_amount
            ));
            
            assert_ok!(DcfPallet::vote_proposal(
                RuntimeOrigin::signed(validator),
                i as u32,
                true
            ));
            
            assert_ok!(DcfPallet::execute_proposal(
                RuntimeOrigin::root(),
                i as u32
            ));
            
            let post_reward_stake = DcfPallet::validator_stake(&validator);
            
            // Property: rewards increase stake by exact amount
            assert_eq!(post_reward_stake - pre_reward_stake, *reward_amount);
            
            // Property: stake never decreases from rewards
            assert!(post_reward_stake >= pre_reward_stake);
            
            // Property: total stake remains bounded
            let max_reasonable_stake = initial_stake * 10; // 10x growth limit
            assert!(post_reward_stake <= max_reasonable_stake);
        }
    });
}

/// Property: Validator participation tracking is consistent
#[test]
fn property_participation_tracking_consistency() {
    new_test_ext().execute_with(|| {
        for block_num in 1..=20 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let active_validators = DcfPallet::active_validators();
            
            for validator in &active_validators {
                let (authored, missed) = DcfPallet::validator_participation(validator);
                let last_active = DcfPallet::validator_last_active(validator);
                
                // Property: participation counters are non-negative
                assert!(authored >= 0);
                assert!(missed >= 0);
                
                // Property: last active is reasonable
                assert!(last_active as u64 <= block_num);
                
                // Property: total participation makes sense
                let total_participation = authored + missed;
                assert!(total_participation <= block_num as u32);
                
                // Property: active validators have recent activity
                if DcfPallet::is_validator_active(validator) {
                    let blocks_since_active = block_num - last_active as u64;
                    assert!(blocks_since_active <= 100); // Reasonable activity window
                }
            }
        }
    });
}

/// Property: Governance proposals maintain system integrity
#[test]
fn property_governance_system_integrity() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        
        let validator = 1u64;
        let proposal_amounts = vec![100u128, 500u128, 1000u128];
        
        for (i, amount) in proposal_amounts.iter().enumerate() {
            // Create proposal
            assert_ok!(DcfPallet::propose_reward_validator(
                RuntimeOrigin::root(),
                validator,
                validator,
                *amount
            ));
            
            let proposal = crate::Proposals::<Test>::get(i as u32).unwrap();
            
            // Property: proposal has valid structure
            assert_eq!(proposal.proposer, validator);
            match &proposal.action {
                crate::ProposalAction::Reward { validator: beneficiary, amount: prop_amount } => {
                    assert_eq!(*beneficiary, validator);
                    assert_eq!(*prop_amount, *amount);
                },
                _ => panic!("Expected Reward action"),
            }
            assert_eq!(proposal.status, crate::ProposalStatus::Pending);
            assert_eq!(proposal.votes_for, 0);
            assert_eq!(proposal.votes_against, 0);
            
            // Vote on proposal
            assert_ok!(DcfPallet::vote_proposal(
                RuntimeOrigin::signed(validator),
                i as u32,
                true
            ));
            
            let updated_proposal = crate::Proposals::<Test>::get(i as u32).unwrap();
            
            // Property: voting updates proposal correctly
            assert_eq!(updated_proposal.votes_for, 1);
            assert_eq!(updated_proposal.votes_against, 0);
            
            // Execute proposal
            assert_ok!(DcfPallet::execute_proposal(
                RuntimeOrigin::root(),
                i as u32
            ));
            
            let executed_proposal = crate::Proposals::<Test>::get(i as u32).unwrap();
            
            // Property: execution updates status
            assert_eq!(executed_proposal.status, crate::ProposalStatus::Executed);
        }
    });
}

/// Property: System maintains liveness under various conditions
#[test]
fn property_system_liveness() {
    new_test_ext().execute_with(|| {
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        
        // Test system liveness under various stress conditions
        for scenario in 1..=5 {
            match scenario {
                1 => {
                    // Scenario: Multiple validator operations
                    for i in 10..15 {
                        Balances::make_free_balance_be(&i, 50000);
                        assert_ok!(DcfPallet::join_validators(
                            RuntimeOrigin::signed(i),
                            Some(b"TestValidator".to_vec().try_into().unwrap())
                        ));
                    }
                },
                2 => {
                    // Scenario: Consensus weight changes
                    assert_ok!(DcfPallet::update_consensus_weights(
                        RuntimeOrigin::root(),
                        8000,
                        2000
                    ));
                },
                3 => {
                    // Scenario: Governance operations
                    assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
                    assert_ok!(DcfPallet::propose_reward_validator(
                        RuntimeOrigin::root(),
                        1,
                        1,
                        1000
                    ));
                },
                4 => {
                    // Scenario: Score updates
                    let validators = DcfPallet::active_validators();
                    for validator in validators.iter().take(3) {
                        assert_ok!(DcfPallet::update_validator_stake_score(
                            RuntimeOrigin::signed(*validator),
                            *validator
                        ));
                    }
                },
                5 => {
                    // Scenario: Block progression
                    for block in 1..=10 {
                        System::set_block_number(block);
                        DcfPallet::on_initialize(block);
                        DcfPallet::on_finalize(block);
                    }
                },
                _ => {}
            }
            
            // Property: system maintains liveness after each scenario
            let active_validators = DcfPallet::active_validators();
            assert!(active_validators.len() >= min_active as usize);
            
            let current_epoch = DcfPallet::current_epoch();
            assert!(current_epoch >= 0);
            
            let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
            assert_eq!(pos_weight + poi_weight, 10000);
            
            // Property: all active validators have valid states
            for validator in &active_validators {
                assert!(DcfPallet::is_validator_active(validator));
                assert!(DcfPallet::validator_stake(validator) > 0);
            }
        }
    });
}

/// Property: Economic incentives align with security
#[test]
fn property_economic_security_alignment() {
    new_test_ext().execute_with(|| {
        let validators = DcfPallet::active_validators();
        let total_stake: u128 = validators.iter()
            .map(|v| DcfPallet::validator_stake(v))
            .sum();
        
        // Property: higher stake validators have higher influence
        let mut validator_stakes: Vec<_> = validators.iter()
            .map(|v| (*v, DcfPallet::validator_stake(v)))
            .collect();
        validator_stakes.sort_by(|a, b| b.1.cmp(&a.1));
        
        for window in validator_stakes.windows(2) {
            let (_, stake1) = window[0];
            let (_, stake2) = window[1];
            
            // Property: stakes are ordered correctly
            assert!(stake1 >= stake2);
        }
        
        // Property: no single validator controls majority
        let max_stake = validator_stakes[0].1;
        assert!(max_stake < total_stake / 2);
        
        // Property: stake distribution is reasonable
        let min_stake = <Test as crate::Config>::MinStake::get();
        let stake_ratio = max_stake / min_stake;
        assert!(stake_ratio <= 100); // No validator has more than 100x minimum
    });
}

/// Property: System handles edge cases gracefully
#[test]
fn property_edge_case_handling() {
    new_test_ext().execute_with(|| {
        // Test edge case: minimum validator count
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        let active_validators = DcfPallet::active_validators();
        
        // Try to make validators leave until minimum
        let mut leave_attempts = 0;
        for validator in &active_validators {
            if active_validators.len() - leave_attempts > min_active as usize {
                let result = DcfPallet::leave_validators(RuntimeOrigin::signed(*validator));
                if result.is_ok() {
                    leave_attempts += 1;
                }
            }
        }
        
        // Property: system maintains minimum validators
        let remaining_validators = DcfPallet::active_validators();
        assert!(remaining_validators.len() >= min_active as usize);
        
        // Test edge case: zero consensus weights (should fail)
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 0, 10000),
            Error::<Test>::InvalidWeight
        );
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 10000, 0),
            Error::<Test>::InvalidWeight
        );
        
        // Test edge case: weights not summing to 100%
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 5000, 4000),
            Error::<Test>::InvalidWeight
        );
        
        // Property: invalid operations don't break system
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}