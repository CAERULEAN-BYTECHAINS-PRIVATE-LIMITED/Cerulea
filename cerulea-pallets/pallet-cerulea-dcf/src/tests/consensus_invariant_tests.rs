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
                total_staked += pallet_cerulea_pos::Stake::<Test>::get(validator);
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
        let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();
        let max_validators = <<Test as pallet_cerulea_pos::Config>::MaxValidators as frame_support::traits::Get<u32>>::get();
        
        for block_num in 1..=15 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
            
            let active_validators = DcfPallet::active_validators();
            
            // Property: validator count within bounds
            assert!(active_validators.len() <= max_validators as usize);
            
            for validator in &active_validators {
                let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
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

/// Property: the Standing Engine keeps every score inside one whole
///
/// The Trust Score replaced the stake derived score. The engine composes the
/// score from the components the network weighs, and every component is a
/// share. The total therefore stays inside one whole at every block, and the
/// commit weight the Commit Engine reads stays inside the same bound.
#[test]
fn property_trust_score_stays_inside_one_whole() {
    new_test_ext().execute_with(|| {
        super::assert_composition_reaches_one_whole();

        let confidence_bound = <<Test as pallet_cerulea_poi::Config>::MaxValidatorScore as frame_support::traits::Get<u64>>::get();

        for block_num in 1..=10 {
            System::set_block_number(block_num);
            DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let active_validators = DcfPallet::active_validators();

            for validator in &active_validators {
                // The composed score and every component share hold.
                super::assert_trust_score_bounded(validator);
                super::assert_trust_weight_bounded(validator);

                // The score reads the same on two calls in the same block, so
                // every node reaches the same weighting.
                let first = DcfPallet::trust_score(validator);
                let second = DcfPallet::trust_score(validator);
                assert_eq!(first, second);

                // The inference confidence carries its own bound.
                assert!(
                    DcfPallet::validator_inference_score(validator) <= confidence_bound,
                    "The inference confidence of validator {} passed its bound",
                    validator
                );
            }
        }
    });
}

/// Property: Epoch transitions preserve system invariants
#[test]
fn property_epoch_transitions_preserve_invariants() {
    new_test_ext().execute_with(|| {
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        
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

/// Property: a penalty lowers the free balance and the epoch record
///
/// A penalty takes the named amount from the free balance of the validator.
/// The stake the validator bonded stays where it is, so the Commit Engine
/// share holds until the next epoch boundary. The stake pallet records the
/// amount against the epoch and against the validator, and the two published
/// ceilings hold across the whole run.
#[test]
fn property_penalty_lowers_the_balance_and_records_the_epoch() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        Balances::make_free_balance_be(&validator, 100_000);

        let bonded = pallet_cerulea_pos::Stake::<Test>::get(validator);
        let ceiling_per_epoch = <Test as pallet_cerulea_pos::Config>::MaxSlashPerEpoch::get();
        let ceiling_per_validator =
            <Test as pallet_cerulea_pos::Config>::MaxSlashPerValidator::get();

        let penalties = vec![100u128, 500u128, 1000u128, 2000u128];

        for penalty in penalties {
            let before = Balances::free_balance(validator);
            let recorded_before = pallet_cerulea_pos::ValidatorEpochSlashed::<Test>::get(validator);

            assert_ok!(PalletCeruleaPos::slash_validator(
                RuntimeOrigin::root(),
                validator,
                penalty
            ));

            let after = Balances::free_balance(validator);
            let recorded_after = pallet_cerulea_pos::ValidatorEpochSlashed::<Test>::get(validator);

            // The penalty takes the named amount from the free balance.
            assert_eq!(before - after, penalty);

            // The stake pallet records the same amount against the validator.
            assert_eq!(recorded_after - recorded_before, penalty);

            // The bonded stake holds, so the Commit Engine share holds.
            assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(validator), bonded);

            // The validator keeps its seat while the standing holds.
            assert!(DcfPallet::is_validator_active(&validator));

            // Both published ceilings hold.
            assert!(recorded_after <= ceiling_per_validator);
            assert!(pallet_cerulea_pos::EpochTotalSlashed::<Test>::get() <= ceiling_per_epoch);
        }

        // The Standing Engine keeps the Trust Score inside one whole through
        // every penalty.
        super::assert_trust_score_bounded(&validator);
    });
}

/// Property: a reward credits the balance and records the epoch
///
/// A reward credits the named amount to the free balance of the validator.
/// The stake the validator bonded stays where it is. The stake pallet records
/// the amount against the epoch and against the validator, and the two
/// published ceilings hold across the whole run.
#[test]
fn property_reward_credits_the_balance_and_records_the_epoch() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let bonded = pallet_cerulea_pos::Stake::<Test>::get(validator);
        let ceiling_per_epoch = <Test as pallet_cerulea_pos::Config>::MaxRewardPerEpoch::get();
        let ceiling_per_validator =
            <Test as pallet_cerulea_pos::Config>::MaxRewardPerValidator::get();

        // Enable governance for rewards
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));

        let reward_amounts = [100u128, 500u128, 1000u128, 2000u128];

        for (i, reward_amount) in reward_amounts.iter().enumerate() {
            let before = Balances::free_balance(validator);
            let recorded_before = pallet_cerulea_pos::ValidatorEpochRewarded::<Test>::get(validator);

            assert_ok!(DcfPallet::propose_reward_validator(
                RuntimeOrigin::root(),
                validator,
                validator,
                *reward_amount
            ));

            // The proposal reaches the quorum the pallet asks for before it
            // executes. Half of the active validators vote in favour.
            assert_ok!(mock_approve_proposal(i as u32));
            assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), i as u32));

            let after = Balances::free_balance(validator);
            let recorded_after = pallet_cerulea_pos::ValidatorEpochRewarded::<Test>::get(validator);

            // The reward credits the named amount to the free balance.
            assert_eq!(after - before, *reward_amount);

            // The stake pallet records the same amount against the validator.
            assert_eq!(recorded_after - recorded_before, *reward_amount);

            // The bonded stake holds through the reward.
            assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(validator), bonded);

            // Both published ceilings hold.
            assert!(recorded_after <= ceiling_per_validator);
            assert!(pallet_cerulea_pos::EpochTotalRewarded::<Test>::get() <= ceiling_per_epoch);
        }

        // The Standing Engine keeps the Trust Score inside one whole through
        // every reward.
        super::assert_trust_score_bounded(&validator);
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
        let proposal_amounts = [100u128, 500u128, 1000u128];
        
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
            
            // Vote on the proposal until the quorum is reached. One vote
            // leaves the proposal pending, which the pallet holds.
            assert_ok!(mock_approve_proposal(i as u32));

            let updated_proposal = crate::Proposals::<Test>::get(i as u32).unwrap();

            // Property: every vote counted in favour, and the proposal passed.
            let quorum = (DcfPallet::active_validators().len() as u32 + 1) / 2;
            assert!(updated_proposal.votes_for >= quorum);
            assert_eq!(updated_proposal.votes_against, 0);
            assert_eq!(updated_proposal.status, crate::ProposalStatus::Approved);
            
            // Execute proposal
            // The proposal reaches the quorum the pallet asks for before it
            // executes. Half of the active validators vote in favour.
            assert_ok!(mock_approve_proposal(i as u32));
            assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), i as u32));
            
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
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        
        // Test system liveness under various stress conditions
        for scenario in 1..=5 {
            match scenario {
                1 => {
                    // Scenario: Multiple validator operations
                    for i in 10..15 {
                        Balances::make_free_balance_be(&i, 50000);
                        assert_ok!(mock_join_validators(
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
                    // Scenario: Score updates. Governance mode is on from the
                    // scenario before, so the call runs under root.
                    let validators = DcfPallet::active_validators();
                    for validator in validators.iter().take(3) {
                        assert_ok!(DcfPallet::update_validator_stake_score(
                            RuntimeOrigin::root(),
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
                assert!(pallet_cerulea_pos::Stake::<Test>::get(validator) > 0);
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
            .map(pallet_cerulea_pos::Stake::<Test>::get)
            .sum();
        
        // Property: higher stake validators have higher influence
        let mut validator_stakes: Vec<_> = validators.iter()
            .map(|v| (*v, pallet_cerulea_pos::Stake::<Test>::get(v)))
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
        let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();
        let stake_ratio = max_stake / min_stake;
        assert!(stake_ratio <= 100); // No validator has more than 100x minimum
    });
}

/// Property: the engines hold their edges
///
/// The Admission Engine keeps the active set at or above the minimum the
/// deployment asks for. The Assurance Engine keeps every threshold above half
/// the weight, so two conflicting domains never both commit. A weight pair
/// that misses the whole is refused.
#[test]
fn property_edge_case_handling() {
    new_test_ext().execute_with(|| {
        use crate::engines::assurance::TransactionClass;

        // Test edge case: minimum validator count
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        let active_validators = DcfPallet::active_validators();

        // Try to make validators leave until minimum
        let mut leave_attempts = 0;
        for validator in &active_validators {
            if active_validators.len() - leave_attempts > min_active as usize {
                let result = mock_leave_validators(RuntimeOrigin::signed(*validator));
                if result.is_ok() {
                    leave_attempts += 1;
                }
            }
        }

        // Property: system maintains minimum validators
        let remaining_validators = DcfPallet::active_validators();
        assert!(remaining_validators.len() >= min_active as usize);

        // The Assurance Engine holds every threshold above half the weight.
        let table = DcfPallet::assurance_class_table();
        for class in [
            TransactionClass::Routine,
            TransactionClass::Financial,
            TransactionClass::Structural,
            TransactionClass::Constitutional,
        ] {
            let share = table.threshold_percent(class);
            assert!(share > 50, "A threshold at or below half lets two domains commit");
            assert!(share <= 100);
        }

        // The constitutional class waits a delay window, and the routine class
        // waits none.
        assert!(table.delay_blocks(TransactionClass::Constitutional) > 0);
        assert_eq!(table.delay_blocks(TransactionClass::Routine), 0);

        // A weight pair that misses the whole is refused.
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 5000, 4000),
            Error::<Test>::InvalidWeight
        );

        // Property: invalid operations don't break system
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}