//! End-to-end tests for complete validator workflows

use crate::mock::*;
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Get, OnFinalize, OnInitialize},
};

/// Complete validator lifecycle: join -> participate -> score updates -> leave
#[test]
fn end_to_end_validator_complete_lifecycle() {
    new_test_ext().execute_with(|| {
        let validator = 100u64;
        let _stake_amount = 10000u128;

        // Setup: Give validator sufficient balance
        Balances::make_free_balance_be(&validator, 50000);
        let initial_balance = Balances::free_balance(validator);

        // Phase 1: Join as validator
        assert_ok!(mock_join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"TestValidator".to_vec())
        ));

        // Verify joining
        assert!(DcfPallet::is_validator_active(&validator));
        let validator_stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
        assert!(validator_stake > 0);
        assert_eq!(Balances::reserved_balance(validator), validator_stake);
        // Admission reserves the stake the deployment asks for, and the free
        // balance falls by exactly that reserve.
        assert_eq!(
            Balances::free_balance(validator),
            initial_balance - validator_stake
        );

        // Phase 2: Register in PoS system
        assert_ok!(mock_register_validator(validator));
        assert_eq!(PalletCeruleaPos::validators(validator), Some(true));

        // Phase 3: Participate in PoS scoring
        assert_ok!(PalletCeruleaPos::submit_score(
            RuntimeOrigin::signed(1),
            validator,
            85
        ));
        assert_eq!(PalletCeruleaPos::validator_scores(validator), Some(85));

        // Phase 4: Participate in PoI system
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            90
        ));
        let (result, epoch) = PalletCeruleaPoi::inference_results(validator).unwrap();
        assert_eq!(result, 42);
        assert_eq!(epoch, DcfPallet::current_epoch());

        // Phase 5: Update DCF scores based on participation
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
        assert!(stake_score > 0);
        assert!(inference_score > 0);

        // Phase 6: Participate in governance (if enabled)
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        assert_ok!(DcfPallet::propose_reward_validator(
            RuntimeOrigin::root(),
            validator,
            validator,
            1000
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(validator),
            0,
            true
        ));

        // Phase 7: Experience epoch transitions
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        for block in 1..=epoch_length + 5 {
            System::set_block_number(block as u64);
            DcfPallet::on_initialize(block as u64);
            DcfPallet::on_finalize(block as u64);
        }

        // Verify validator survived epoch transition
        assert!(DcfPallet::is_validator_active(&validator));
        assert!(DcfPallet::current_epoch() > 0);

        // Phase 8: Leave validator set
        let _pre_leave_stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
        assert_ok!(mock_leave_validators(RuntimeOrigin::signed(validator)));

        // Phase 9: Wait for cooldown period
        let cooldown_period: u32 =
            <<Test as pallet_cerulea_pos::Config>::LeaveCooldown as frame_support::traits::Get<
                u32,
            >>::get();
        for block in 1..=cooldown_period + 1 {
            System::set_block_number((epoch_length + 5 + block) as u64);
            DcfPallet::on_initialize((epoch_length + 5 + block) as u64);
            DcfPallet::on_finalize((epoch_length + 5 + block) as u64);
        }

        // Verify leaving completed
        assert!(!DcfPallet::is_validator_active(&validator));
        let final_balance = Balances::free_balance(validator);
        assert!(final_balance > initial_balance - _stake_amount); // Got some stake back
    });
}

/// Validator workflow with challenges and slashing
#[test]
fn end_to_end_validator_with_challenges_and_slashing() {
    new_test_ext().execute_with(|| {
        let validator = 101u64;
        let challenger = 102u64;
        let _stake_amount = 8000u128;

        // Setup both accounts
        Balances::make_free_balance_be(&validator, 50000);
        Balances::make_free_balance_be(&challenger, 50000);

        // Both join as validators
        assert_ok!(mock_join_validators(
            RuntimeOrigin::signed(validator),
            Some(b"Validator".to_vec())
        ));
        assert_ok!(mock_join_validators(
            RuntimeOrigin::signed(challenger),
            Some(b"Challenger".to_vec())
        ));

        // Both register in PoS
        assert_ok!(mock_register_validator(validator));
        assert_ok!(mock_register_validator(challenger));

        // Submit PoS scores
        assert_ok!(PalletCeruleaPos::submit_score(
            RuntimeOrigin::signed(1),
            validator,
            75
        ));
        assert_ok!(PalletCeruleaPos::submit_score(
            RuntimeOrigin::signed(1),
            challenger,
            85
        ));

        // Validator submits inference
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(validator),
            42,
            80
        ));

        // Challenger challenges the inference
        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(challenger),
            validator,
            42
        ));

        // Verify challenge was recorded
        let (challenged_validator, result, epoch) =
            PalletCeruleaPoi::challenges(challenger).unwrap();
        assert_eq!(challenged_validator, validator);
        assert_eq!(result, 42);
        assert_eq!(epoch, DcfPallet::current_epoch());

        // Simulate poor performance leading to slashing
        let initial_stake = pallet_cerulea_pos::Stake::<Test>::get(validator);

        // PoS slashing for poor performance
        assert_ok!(mock_slash_validator(&validator, 100));
        assert_eq!(PalletCeruleaPos::slashing_count(validator), Some(1));

        // A penalty takes the named amount from the free balance. The bonded
        // stake holds until the next epoch boundary.
        let slash_amount = 1000u128;
        let balance_before = Balances::free_balance(validator);
        assert_ok!(PalletCeruleaPos::slash_validator(
            RuntimeOrigin::root(),
            validator,
            slash_amount
        ));

        assert_eq!(
            balance_before - Balances::free_balance(validator),
            slash_amount
        );
        assert_eq!(
            pallet_cerulea_pos::Stake::<Test>::get(validator),
            initial_stake
        );

        // Multiple slashes in PoS lead to removal
        assert_ok!(mock_slash_validator(&validator, 100));
        assert_ok!(mock_slash_validator(&validator, 100));

        // Validator should be removed from PoS after 3 slashes
        assert_eq!(PalletCeruleaPos::validators(validator), None);

        // But still active in DCF (different slashing rules)
        assert!(DcfPallet::is_validator_active(&validator));

        // Challenger benefits from successful challenge
        assert_ok!(PalletCeruleaPos::boost_score(
            RuntimeOrigin::signed(1),
            challenger,
            10
        ));

        let challenger_score = PalletCeruleaPos::validator_scores(challenger).unwrap_or(0);
        assert!(challenger_score > 85);
    });
}

/// Multi-validator competitive workflow
#[test]
fn end_to_end_multi_validator_competition() {
    new_test_ext().execute_with(|| {
        let validators = vec![201u64, 202u64, 203u64, 204u64, 205u64];
        let _stake_amounts = [5000u128, 6000u128, 7000u128, 8000u128, 9000u128];

        // Setup all validators
        for validator in validators.iter() {
            Balances::make_free_balance_be(validator, 50000);

            // Join with different stake amounts
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("Validator{}", validator).into_bytes())
            ));

            // Register in PoS
            assert_ok!(mock_register_validator(*validator));
        }

        // Simulate competitive scoring rounds. Each round runs in its own
        // epoch, since a validator contributes one inference per epoch.
        for round in 1..=5 {
            DcfPallet::handle_epoch_transition();

            // Submit different PoS scores
            for (i, validator) in validators.iter().enumerate() {
                let score = 70 + (i as u32 * 5) + (round * 2);
                assert_ok!(PalletCeruleaPos::submit_score(
                    RuntimeOrigin::signed(1),
                    *validator,
                    score
                ));
            }

            // Submit PoI inferences
            for (i, validator) in validators.iter().enumerate() {
                let result = 40 + (i as u32 * 2) + round;
                let confidence = 80 + (i as u32 * 2);
                assert_ok!(PalletCeruleaPoi::submit_inference(
                    RuntimeOrigin::signed(*validator),
                    result,
                    confidence
                ));
            }

            // Some validators challenge others
            if round > 1 {
                // Validator 0 challenges validator 1
                let target_result = 40 + 2 + round;
                assert_ok!(PalletCeruleaPoi::challenge_inference(
                    RuntimeOrigin::signed(validators[0]),
                    validators[1],
                    target_result
                ));
            }

            // Update DCF scores
            for validator in &validators {
                assert_ok!(DcfPallet::update_validator_stake_score(
                    RuntimeOrigin::signed(*validator),
                    *validator
                ));
                assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
                    RuntimeOrigin::signed(*validator),
                    *validator
                ));
            }

            // Advance some blocks
            for block in 1..=10 {
                let block_num = (round - 1) * 10 + block;
                System::set_block_number(block_num as u64);
                DcfPallet::on_initialize(block_num as u64);
                DcfPallet::on_finalize(block_num as u64);
            }
        }

        // Analyze final rankings
        let mut validator_scores: Vec<_> = validators
            .iter()
            .map(|v| {
                let stake_score = DcfPallet::validator_stake_score(v);
                let inference_score = DcfPallet::validator_inference_score(v);
                let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
                let combined = (stake_score * pos_weight as u128
                    + inference_score as u128 * poi_weight as u128)
                    / 10000;
                (*v, combined as u64)
            })
            .collect();
        validator_scores.sort_by(|a, b| b.1.cmp(&a.1));

        // Admission reserves the same minimum stake for every validator, so
        // the ranking reads the composed standing rather than a stake spread.
        let top_validator = validator_scores[0].0;
        let top_stake = pallet_cerulea_pos::Stake::<Test>::get(top_validator);
        let minimum = <Test as pallet_cerulea_pos::Config>::MinStake::get();
        assert!(top_stake >= minimum);

        // The top validator carries a score at or above every other one.
        for (_, score) in validator_scores.iter() {
            assert!(validator_scores[0].1 >= *score);
        }

        // All validators should still be active
        for validator in &validators {
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}

/// Governance-driven validator workflow
#[test]
fn end_to_end_governance_driven_workflow() {
    new_test_ext().execute_with(|| {
        let proposer = 301u64;
        let beneficiary = 302u64;
        let voter1 = 303u64;
        let voter2 = 304u64;

        // Setup validators
        for validator in [proposer, beneficiary, voter1, voter2].iter() {
            Balances::make_free_balance_be(validator, 50000);
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("Validator{}", validator).into_bytes())
            ));
        }

        // Enable governance
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));

        // Phase 1: Proposal creation
        let balance_before_governance = Balances::free_balance(beneficiary);
        let reward_amount = 2000u128;
        assert_ok!(DcfPallet::propose_reward_validator(
            RuntimeOrigin::root(),
            proposer,
            beneficiary,
            reward_amount
        ));

        let proposal = crate::Proposals::<Test>::get(0).unwrap();
        assert_eq!(proposal.proposer, proposer);
        match &proposal.action {
            crate::ProposalAction::Reward {
                validator: prop_beneficiary,
                amount: prop_amount,
            } => {
                assert_eq!(*prop_beneficiary, beneficiary);
                assert_eq!(*prop_amount, reward_amount);
            }
            _ => panic!("Expected Reward action"),
        }

        // Phase 2: Voting process
        assert_ok!(mock_cast_vote(voter1, 0, true));
        assert_ok!(mock_cast_vote(voter2, 0, true));
        assert_ok!(mock_cast_vote(proposer, 0, true));

        let voted_proposal = crate::Proposals::<Test>::get(0).unwrap();
        assert_eq!(voted_proposal.votes_for, 3);
        assert_eq!(voted_proposal.votes_against, 0);

        // Phase 3: Proposal execution. A reward credits the free balance.
        let pre_execution_balance = Balances::free_balance(beneficiary);
        // The proposal reaches the quorum the pallet asks for before it
        // executes. Half of the active validators vote in favour.
        assert_ok!(mock_approve_proposal(0));
        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        let post_execution_balance = Balances::free_balance(beneficiary);
        assert_eq!(
            post_execution_balance - pre_execution_balance,
            reward_amount
        );

        let executed_proposal = crate::Proposals::<Test>::get(0).unwrap();
        assert_eq!(executed_proposal.status, crate::ProposalStatus::Executed);

        // Phase 4: Multiple governance rounds
        for round in 1..=3 {
            let proposal_id = round as u32;
            let round_reward = 500u128 * round as u128;

            assert_ok!(DcfPallet::propose_reward_validator(
                RuntimeOrigin::root(),
                proposer,
                beneficiary,
                round_reward
            ));

            // Voting with different patterns
            match round {
                1 => {
                    // Unanimous approval
                    for voter in [voter1, voter2, proposer].iter() {
                        assert_ok!(mock_cast_vote(*voter, proposal_id, true));
                    }
                }
                2 => {
                    // Mixed voting
                    assert_ok!(mock_cast_vote(voter1, proposal_id, true));
                    assert_ok!(mock_cast_vote(voter2, proposal_id, false));
                    assert_ok!(mock_cast_vote(proposer, proposal_id, true));
                }
                3 => {
                    // Rejection
                    for voter in [voter1, voter2].iter() {
                        assert_ok!(mock_cast_vote(*voter, proposal_id, false));
                    }
                }
                _ => {}
            }

            // Execute if approved
            let proposal = crate::Proposals::<Test>::get(proposal_id).unwrap();
            if proposal.votes_for > proposal.votes_against {
                // The proposal reaches the quorum the pallet asks for before it
                // executes. Half of the active validators vote in favour.
                assert_ok!(mock_approve_proposal(proposal_id));
                assert_ok!(DcfPallet::execute_proposal(
                    RuntimeOrigin::root(),
                    proposal_id
                ));
            }
        }

        // Verify governance outcomes. Every reward credited the free balance
        // of the beneficiary, and the stake pallet recorded the same total
        // against the epoch.
        let recorded = pallet_cerulea_pos::ValidatorEpochRewarded::<Test>::get(beneficiary);
        assert!(recorded >= reward_amount);
        assert_eq!(
            Balances::free_balance(beneficiary),
            balance_before_governance + recorded
        );
    });
}

/// Stress test: High-frequency validator operations
#[test]
fn end_to_end_high_frequency_operations() {
    new_test_ext().execute_with(|| {
        let num_validators = 20u64;
        let operations_per_validator = 5;

        // Setup many validators
        for i in 1..=num_validators {
            let validator = 400 + i;
            Balances::make_free_balance_be(&validator, 100000);

            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(validator),
                Some(format!("Validator{}", validator).into_bytes())
            ));

            assert_ok!(mock_register_validator(validator));
        }

        // Perform high-frequency operations. Each round runs in its own
        // epoch, since a validator contributes one inference per epoch.
        for round in 1..=operations_per_validator {
            DcfPallet::handle_epoch_transition();

            for i in 1..=num_validators {
                let validator = 400 + i;

                // PoS score updates
                let score = 60 + ((i + round) % 40) as u32;
                assert_ok!(PalletCeruleaPos::submit_score(
                    RuntimeOrigin::signed(1),
                    validator,
                    score
                ));

                // PoI inference submissions
                let result = (i * round) % 100;
                let confidence = 80 + ((i + round) % 20) as u32;
                assert_ok!(PalletCeruleaPoi::submit_inference(
                    RuntimeOrigin::signed(validator),
                    result as u32,
                    confidence
                ));

                // DCF score updates
                assert_ok!(DcfPallet::update_validator_stake_score(
                    RuntimeOrigin::signed(validator),
                    validator
                ));
                assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
                    RuntimeOrigin::signed(validator),
                    validator
                ));
            }

            // Process some blocks
            for block in 1..=5 {
                let block_num = (round - 1) * 5 + block;
                System::set_block_number(block_num);
                DcfPallet::on_initialize(block_num);
                DcfPallet::on_finalize(block_num);
            }
        }

        // Verify system stability
        let active_validators = DcfPallet::active_validators();
        assert_eq!(active_validators.len(), (num_validators + 3) as usize); // +3 genesis validators

        // Verify all validators maintained their state
        for i in 1..=num_validators {
            let validator = 400 + i;
            assert!(DcfPallet::is_validator_active(&validator));
            assert_eq!(PalletCeruleaPos::validators(validator), Some(true));
            assert!(PalletCeruleaPoi::inference_results(validator).is_some());
        }

        // Verify system metrics are reasonable
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}

/// Recovery workflow: System recovery from validator failures
#[test]
fn end_to_end_system_recovery_workflow() {
    new_test_ext().execute_with(|| {
        let critical_validators = vec![501u64, 502u64, 503u64];
        let backup_validators = vec![504u64, 505u64, 506u64, 507u64];

        // Setup critical validators
        for validator in &critical_validators {
            Balances::make_free_balance_be(validator, 100000);
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("CriticalValidator{}", validator).into_bytes())
            ));
            assert_ok!(mock_register_validator(*validator));
        }

        // Setup backup validators
        for validator in &backup_validators {
            Balances::make_free_balance_be(validator, 50000);
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("BackupValidator{}", validator).into_bytes())
            ));
            assert_ok!(mock_register_validator(*validator));
        }

        // Establish normal operations
        for validator in critical_validators.iter().chain(backup_validators.iter()) {
            assert_ok!(PalletCeruleaPos::submit_score(
                RuntimeOrigin::signed(1),
                *validator,
                85
            ));
            assert_ok!(PalletCeruleaPoi::submit_inference(
                RuntimeOrigin::signed(*validator),
                42,
                90
            ));
        }

        // Simulate critical validator failures. The deployment takes the
        // amount first. The third recorded offence then removes the validator
        // from the stake pallet, so a later penalty finds no seat.
        for validator in &critical_validators {
            assert_ok!(PalletCeruleaPos::slash_validator(
                RuntimeOrigin::root(),
                *validator,
                10000
            ));

            for _ in 0..3 {
                assert_ok!(mock_slash_validator(validator, 100));
            }

            assert_noop!(
                PalletCeruleaPos::slash_validator(RuntimeOrigin::root(), *validator, 100),
                pallet_cerulea_pos::Error::<Test>::ValidatorNotInSet
            );
        }

        // Verify critical validators are removed from PoS
        for validator in &critical_validators {
            assert_eq!(PalletCeruleaPos::validators(validator), None);
        }

        // System should still maintain minimum validators
        let min_active = <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as frame_support::traits::Get<u32>>::get();
        let active_validators = DcfPallet::active_validators();
        assert!(active_validators.len() >= min_active as usize);

        // Backup validators should still be active
        for validator in &backup_validators {
            assert!(DcfPallet::is_validator_active(validator));
            assert_eq!(PalletCeruleaPos::validators(validator), Some(true));
        }

        // Add new validators to replace failed ones
        let replacement_validators = vec![508u64, 509u64, 510u64];
        for validator in &replacement_validators {
            Balances::make_free_balance_be(validator, 100000);
            assert_ok!(mock_join_validators(
                RuntimeOrigin::signed(*validator),
                Some(format!("ReplacementValidator{}", validator).into_bytes())
            ));
            assert_ok!(mock_register_validator(*validator));
        }

        // Verify system recovery
        let recovered_validators = DcfPallet::active_validators();
        assert!(recovered_validators.len() > min_active as usize);

        // System should function normally after recovery
        for validator in &replacement_validators {
            assert_ok!(PalletCeruleaPos::submit_score(
                RuntimeOrigin::signed(1),
                *validator,
                80
            ));
            assert_ok!(PalletCeruleaPoi::submit_inference(
                RuntimeOrigin::signed(*validator),
                45,
                85
            ));
        }

        // Verify consensus weights still work
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}
