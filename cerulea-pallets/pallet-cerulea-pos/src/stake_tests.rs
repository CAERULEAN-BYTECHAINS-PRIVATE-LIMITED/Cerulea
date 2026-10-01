//! Further tests of the stake module.
//!
//! These tests drive every call, every hook and every public routine of the
//! stake module against the mock runtime. Each refusal test asserts the
//! exact error. Each success test reads the stored state and the recorded
//! events afterwards.

use crate::{
    mock::*, CurrentEpoch, Error, Event, SlashReason, SlashingCount, SlashingRecord, Stake,
    ValidatorEpochRewarded, ValidatorEpochSlashed, ValidatorHandler, ValidatorScores,
    ValidatorSlashingHistory, Validators,
};
use crate::{EpochTotalRewarded, EpochTotalSlashed};
use frame_support::{
    assert_noop, assert_ok,
    traits::{Currency, Hooks, ReservableCurrency},
    weights::Weight,
    BoundedVec,
};
use sp_runtime::{BuildStorage, DispatchError};

type Pos = PalletCeruleaPos;

/// Start a test runtime at block one, so every event lands in the record.
fn ext() -> sp_io::TestExternalities {
    let mut held = new_test_ext();
    held.execute_with(|| System::set_block_number(1));
    held
}

fn register(who: u64) {
    assert_ok!(Pos::register_validator(RuntimeOrigin::signed(who)));
}

fn admit(who: u64) {
    assert_ok!(<TestAdmission as ValidatorHandler<u64, u64>>::on_joined(
        &who, 1_000
    ));
}

fn score(who: u64, value: u32) {
    assert_ok!(Pos::submit_score(RuntimeOrigin::signed(9), who, value));
}

fn recorded(expected: Event<Test>) -> bool {
    System::events()
        .iter()
        .any(|held| held.event == RuntimeEvent::PalletCeruleaPos(expected.clone()))
}

fn free(who: u64) -> u64 {
    <Balances as Currency<u64>>::free_balance(&who)
}

fn reserved(who: u64) -> u64 {
    <Balances as ReservableCurrency<u64>>::reserved_balance(&who)
}

#[test]
fn registration_records_the_validator_and_its_event() {
    ext().execute_with(|| {
        register(1);
        assert_eq!(Validators::<Test>::get(1), Some(true));
        assert!(recorded(Event::ValidatorRegistered { validator: 1 }));

        score(1, 75);
        assert!(recorded(Event::ScoreSubmitted {
            validator: 1,
            score: 75
        }));
        assert_noop!(
            Pos::submit_score(RuntimeOrigin::none(), 1, 75),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn bonding_needs_the_minimum_stake_and_a_registration() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::bond_stake(RuntimeOrigin::signed(1), 999),
            Error::<Test>::InsufficientStake
        );
        assert_noop!(
            Pos::bond_stake(RuntimeOrigin::signed(1), 1_000),
            Error::<Test>::ValidatorNotRegistered
        );
        assert_noop!(
            Pos::bond_stake(RuntimeOrigin::none(), 1_000),
            DispatchError::BadOrigin
        );

        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 1_500));
        assert_eq!(Stake::<Test>::get(1), 1_500);
        assert!(recorded(Event::StakeBonded {
            validator: 1,
            amount: 1_500
        }));
        assert!(recorded(Event::StakeUpdated {
            validator: 1,
            old_stake: 0,
            new_stake: 1_500
        }));

        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 1_000));
        assert_eq!(Stake::<Test>::get(1), 2_500);
    });
}

#[test]
fn bonding_past_the_largest_balance_declines() {
    ext().execute_with(|| {
        register(1);
        Stake::<Test>::insert(1, u64::MAX - 10);
        assert_noop!(
            Pos::bond_stake(RuntimeOrigin::signed(1), 1_000),
            Error::<Test>::InvalidStakeAmount
        );
    });
}

#[test]
fn unbonding_keeps_the_minimum_stake_in_place() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::unbond_stake(RuntimeOrigin::signed(1), 100),
            Error::<Test>::ValidatorNotRegistered
        );
        assert_noop!(
            Pos::unbond_stake(RuntimeOrigin::none(), 100),
            DispatchError::BadOrigin
        );

        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 3_000));
        assert_noop!(
            Pos::unbond_stake(RuntimeOrigin::signed(1), 4_000),
            Error::<Test>::InsufficientStake
        );
        assert_noop!(
            Pos::unbond_stake(RuntimeOrigin::signed(1), 2_500),
            Error::<Test>::InsufficientStake
        );

        assert_ok!(Pos::unbond_stake(RuntimeOrigin::signed(1), 2_000));
        assert_eq!(Stake::<Test>::get(1), 1_000);
        assert!(recorded(Event::StakeUnbonded {
            validator: 1,
            amount: 2_000
        }));
        assert!(recorded(Event::StakeUpdated {
            validator: 1,
            old_stake: 3_000,
            new_stake: 1_000
        }));
    });
}

#[test]
fn the_root_slash_counts_up_and_removes_at_the_limit() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::slash_validator_call(RuntimeOrigin::root(), 1),
            Error::<Test>::ValidatorNotRegistered
        );

        register(1);
        score(1, 70);
        // Another validator holds no power to record an offence.
        assert_noop!(
            Pos::slash_validator_call(RuntimeOrigin::signed(2), 1),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Pos::slash_validator_call(RuntimeOrigin::signed(1), 1),
            DispatchError::BadOrigin
        );
        assert_ok!(Pos::slash_validator_call(RuntimeOrigin::root(), 1));
        assert_ok!(Pos::slash_validator_call(RuntimeOrigin::root(), 1));
        assert_eq!(SlashingCount::<Test>::get(1), Some(2));
        assert!(recorded(Event::ValidatorSlashed {
            validator: 1,
            slashing_count: 2
        }));

        assert_ok!(Pos::slash_validator_call(RuntimeOrigin::root(), 1));
        assert_eq!(Validators::<Test>::get(1), None);
        assert_eq!(ValidatorScores::<Test>::get(1), None);
        assert_eq!(SlashingCount::<Test>::get(1), None);
        assert!(recorded(Event::ValidatorRemoved {
            validator: 1,
            reason: b"Max slashing count reached".to_vec()
        }));
        assert!(recorded(Event::ValidatorSlashed {
            validator: 1,
            slashing_count: 3
        }));
    });
}

#[test]
fn a_score_boost_and_a_score_slash_record_both_figures() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::boost_score(RuntimeOrigin::signed(2), 1, 5),
            Error::<Test>::ValidatorNotRegistered
        );
        assert_noop!(
            Pos::slash_score(RuntimeOrigin::signed(2), 1, 5),
            Error::<Test>::ValidatorNotRegistered
        );
        assert_noop!(
            Pos::boost_score(RuntimeOrigin::none(), 1, 5),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Pos::slash_score(RuntimeOrigin::none(), 1, 5),
            DispatchError::BadOrigin
        );

        register(1);
        score(1, 60);
        assert_ok!(Pos::boost_score(RuntimeOrigin::signed(2), 1, 15));
        assert_eq!(ValidatorScores::<Test>::get(1), Some(75));
        assert!(recorded(Event::ScoreBoosted {
            validator: 1,
            old_score: 60,
            new_score: 75,
            boost_amount: 15
        }));

        assert_ok!(Pos::slash_score(RuntimeOrigin::signed(2), 1, 30));
        assert_eq!(ValidatorScores::<Test>::get(1), Some(45));
        assert!(recorded(Event::ScoreSlashed {
            validator: 1,
            old_score: 75,
            new_score: 45,
            slash_amount: 30
        }));

        // A validator without a score starts from zero.
        register(2);
        assert_ok!(Pos::boost_score(RuntimeOrigin::signed(3), 2, 7));
        assert_eq!(ValidatorScores::<Test>::get(2), Some(7));

        // The boost stops at the largest score.
        score(1, u32::MAX - 10);
        assert_ok!(Pos::boost_score(RuntimeOrigin::signed(2), 1, 20));
        assert_eq!(ValidatorScores::<Test>::get(1), Some(u32::MAX));
    });
}

#[test]
fn a_stake_increase_reserves_the_funds() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::increase_validator_stake(RuntimeOrigin::signed(1), 3_000),
            Error::<Test>::ValidatorNotInSet
        );
        assert_noop!(
            Pos::increase_validator_stake(RuntimeOrigin::root(), 3_000),
            DispatchError::BadOrigin
        );

        register(1);
        assert_noop!(
            Pos::increase_validator_stake(RuntimeOrigin::signed(1), 20_000),
            Error::<Test>::InsufficientStake
        );

        assert_ok!(Pos::increase_validator_stake(
            RuntimeOrigin::signed(1),
            3_000
        ));
        assert_eq!(Stake::<Test>::get(1), 3_000);
        assert_eq!(reserved(1), 3_000);
        assert_eq!(free(1), 7_000);
        assert!(recorded(Event::ValidatorStakeReserved {
            validator: 1,
            amount: 3_000
        }));
        assert!(recorded(Event::ValidatorStakeIncreased {
            validator: 1,
            amount: 3_000
        }));
    });
}

#[test]
fn a_stake_decrease_releases_the_reserved_funds() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::decrease_validator_stake(RuntimeOrigin::signed(1), 1_000),
            Error::<Test>::ValidatorNotInSet
        );
        assert_noop!(
            Pos::decrease_validator_stake(RuntimeOrigin::root(), 1_000),
            DispatchError::BadOrigin
        );

        register(1);
        assert_ok!(Pos::increase_validator_stake(
            RuntimeOrigin::signed(1),
            3_000
        ));
        assert_noop!(
            Pos::decrease_validator_stake(RuntimeOrigin::signed(1), 2_500),
            Error::<Test>::InsufficientStake
        );

        assert_ok!(Pos::decrease_validator_stake(
            RuntimeOrigin::signed(1),
            1_000
        ));
        assert_eq!(Stake::<Test>::get(1), 2_000);
        assert_eq!(reserved(1), 2_000);
        assert_eq!(free(1), 8_000);
        assert!(recorded(Event::ValidatorStakeUnreserved {
            validator: 1,
            amount: 1_000
        }));
        assert!(recorded(Event::ValidatorStakeDecreased {
            validator: 1,
            amount: 1_000
        }));
    });
}

#[test]
fn a_stake_decrease_moves_only_what_the_currency_released() {
    ext().execute_with(|| {
        // A bonded stake is a record. The currency holds nothing in reserve
        // behind the record, so a decrease releases nothing.
        register(2);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(2), 5_000));
        assert_eq!(reserved(2), 0);

        assert_ok!(Pos::decrease_validator_stake(
            RuntimeOrigin::signed(2),
            1_000
        ));
        assert_eq!(Stake::<Test>::get(2), 5_000);
        assert_eq!(free(2), 10_000);
        assert!(recorded(Event::ValidatorStakeDecreased {
            validator: 2,
            amount: 0
        }));
    });
}

#[test]
fn a_root_slash_takes_the_default_share_or_the_named_amount() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::signed(1), 1, 100),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::root(), 1, 100),
            Error::<Test>::ValidatorNotInSet
        );

        register(1);
        // A zero amount takes the default share of ten percent of the free
        // balance.
        assert_ok!(Pos::slash_validator(RuntimeOrigin::root(), 1, 0));
        assert_eq!(free(1), 9_000);
        assert_eq!(EpochTotalSlashed::<Test>::get(), 1_000);
        assert_eq!(ValidatorEpochSlashed::<Test>::get(1), 1_000);

        assert_ok!(Pos::slash_validator(RuntimeOrigin::root(), 1, 4_000));
        assert_eq!(free(1), 5_000);
        assert_eq!(ValidatorEpochSlashed::<Test>::get(1), 5_000);

        // The validator bound of twenty thousand holds.
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::root(), 1, 15_001),
            Error::<Test>::SlashingBoundsExceeded
        );
    });
}

#[test]
fn a_root_slash_respects_the_epoch_bound_and_the_arithmetic_limits() {
    ext().execute_with(|| {
        register(1);
        register(2);
        EpochTotalSlashed::<Test>::put(49_500);
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::root(), 2, 1_000),
            Error::<Test>::SlashingBoundsExceeded
        );

        EpochTotalSlashed::<Test>::put(u64::MAX);
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::root(), 2, 1),
            Error::<Test>::ArithmeticOverflow
        );

        EpochTotalSlashed::<Test>::kill();
        ValidatorEpochSlashed::<Test>::insert(1, u64::MAX);
        assert_noop!(
            Pos::slash_validator(RuntimeOrigin::root(), 1, 1),
            Error::<Test>::ArithmeticOverflow
        );
    });
}

#[test]
fn a_slash_past_the_balance_records_only_what_the_currency_took() {
    ext().execute_with(|| {
        register(3);
        assert_ok!(Pos::execute_slash_validator_with_reason(
            &3,
            15_000,
            SlashReason::Downtime
        ));
        assert_eq!(free(3), 0);
        assert_eq!(ValidatorEpochSlashed::<Test>::get(3), 10_000);
        assert_eq!(EpochTotalSlashed::<Test>::get(), 10_000);
    });
}

#[test]
fn a_percentage_slash_takes_its_share_of_the_stake() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::slash_validator_percentage(RuntimeOrigin::signed(1), 1, 10),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Pos::slash_validator_percentage(RuntimeOrigin::root(), 1, 101),
            Error::<Test>::InvalidStakeAmount
        );

        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 5_000));
        assert_ok!(Pos::slash_validator_percentage(
            RuntimeOrigin::root(),
            1,
            20
        ));
        assert_eq!(free(1), 9_000);
        assert_eq!(ValidatorEpochSlashed::<Test>::get(1), 1_000);
    });
}

#[test]
fn a_zero_percentage_slash_takes_nothing() {
    ext().execute_with(|| {
        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 5_000));
        assert_ok!(Pos::slash_validator_percentage(RuntimeOrigin::root(), 1, 0));
        assert_eq!(free(1), 10_000);
        assert_eq!(EpochTotalSlashed::<Test>::get(), 0);

        // A validator without a stake loses nothing under any percentage.
        register(2);
        assert_ok!(Pos::slash_validator_percentage(
            RuntimeOrigin::root(),
            2,
            50
        ));
        assert_eq!(free(2), 10_000);
        assert_eq!(ValidatorEpochSlashed::<Test>::get(2), 0);

        // An account outside the set still receives the membership error.
        assert_noop!(
            Pos::slash_validator_percentage(RuntimeOrigin::root(), 3, 50),
            Error::<Test>::ValidatorNotInSet
        );
    });
}

#[test]
fn a_batch_slash_passes_over_an_account_outside_the_set() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::slash_multiple_validators(RuntimeOrigin::signed(1), vec![1], 1_000),
            DispatchError::BadOrigin
        );
        register(1);
        register(2);
        assert_ok!(Pos::slash_multiple_validators(
            RuntimeOrigin::root(),
            vec![1, 2, 99],
            1_000
        ));
        assert_eq!(free(1), 9_000);
        assert_eq!(free(2), 9_000);
        assert_eq!(EpochTotalSlashed::<Test>::get(), 2_000);
    });
}

#[test]
fn a_root_reward_pays_the_default_or_the_named_amount_within_bounds() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::signed(1), 1, 100),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::root(), 1, 100),
            Error::<Test>::ValidatorNotInSet
        );

        register(1);
        assert_ok!(Pos::reward_validator_call(RuntimeOrigin::root(), 1, 0));
        assert_eq!(free(1), 20_000);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 10_000);
        assert_eq!(ValidatorEpochRewarded::<Test>::get(1), 10_000);

        // The validator bound of ten thousand holds.
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::root(), 1, 1),
            Error::<Test>::RewardBoundsExceeded
        );

        // The epoch bound of thirty thousand holds.
        register(2);
        register(3);
        register(4);
        assert_ok!(Pos::reward_validator_call(RuntimeOrigin::root(), 2, 10_000));
        assert_ok!(Pos::reward_validator_call(RuntimeOrigin::root(), 3, 10_000));
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::root(), 4, 1),
            Error::<Test>::RewardBoundsExceeded
        );
    });
}

#[test]
fn a_root_reward_declines_on_arithmetic_overflow() {
    ext().execute_with(|| {
        register(1);
        EpochTotalRewarded::<Test>::put(u64::MAX);
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::root(), 1, 1),
            Error::<Test>::ArithmeticOverflow
        );

        EpochTotalRewarded::<Test>::kill();
        ValidatorEpochRewarded::<Test>::insert(1, u64::MAX);
        assert_noop!(
            Pos::reward_validator_call(RuntimeOrigin::root(), 1, 1),
            Error::<Test>::ArithmeticOverflow
        );
    });
}

#[test]
fn a_batch_reward_passes_over_an_account_outside_the_set() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::reward_multiple_validators(RuntimeOrigin::signed(1), vec![1], 700),
            DispatchError::BadOrigin
        );
        register(1);
        assert_ok!(Pos::reward_multiple_validators(
            RuntimeOrigin::root(),
            vec![1, 99],
            700
        ));
        assert_eq!(free(1), 10_700);
        assert_eq!(free(99), 0);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 700);
    });
}

#[test]
fn a_reward_for_the_active_set_reaches_only_admitted_validators() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::reward_all_active_validators(RuntimeOrigin::signed(1), 400),
            DispatchError::BadOrigin
        );
        for who in 1..=3u64 {
            register(who);
        }
        admit(1);
        admit(2);
        assert_ok!(Pos::reward_all_active_validators(
            RuntimeOrigin::root(),
            400
        ));
        assert_eq!(free(1), 10_400);
        assert_eq!(free(2), 10_400);
        assert_eq!(free(3), 10_000);
    });
}

#[test]
fn an_epoch_distribution_with_no_active_validator_pays_nothing() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::distribute_epoch_rewards(RuntimeOrigin::signed(1), 10_000),
            DispatchError::BadOrigin
        );
        register(1);
        assert_ok!(Pos::distribute_epoch_rewards(RuntimeOrigin::root(), 10_000));
        assert_eq!(free(1), 10_000);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 0);
        assert!(!System::events().iter().any(|held| matches!(
            held.event,
            RuntimeEvent::PalletCeruleaPos(Event::EpochRewardsDistributed { .. })
        )));
    });
}

#[test]
fn an_epoch_distribution_splits_the_base_the_performance_and_the_top_pools() {
    ext().execute_with(|| {
        let scores = [(1u64, 95u32), (2, 85), (3, 60), (4, 55), (5, 50)];
        for (who, value) in scores {
            register(who);
            score(who, value);
            admit(who);
        }
        CurrentEpoch::<Test>::put(7);

        // Sixty percent of the pool splits five ways. Twenty five percent
        // splits between the two validators at or above a score of 80.
        // Fifteen percent goes to the top fifth, which is one validator.
        assert_ok!(Pos::distribute_epoch_rewards(RuntimeOrigin::root(), 10_000));
        assert_eq!(free(1), 10_000 + 1_200 + 1_250 + 1_500);
        assert_eq!(free(2), 10_000 + 1_200 + 1_250);
        assert_eq!(free(3), 10_000 + 1_200);
        assert_eq!(free(4), 1_200);
        assert_eq!(free(5), 1_200);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 10_000);
        assert!(recorded(Event::EpochRewardsDistributed {
            epoch: 7,
            total_distributed: 10_000
        }));
    });
}

#[test]
fn an_epoch_distribution_reports_only_what_it_paid() {
    ext().execute_with(|| {
        for (who, value) in [(1u64, 90u32), (2, 85), (3, 50)] {
            register(who);
            score(who, value);
            admit(who);
        }
        // Three validators leave the top fifth empty. The top pool of 1_500
        // stays unpaid, so the record reports 8_500.
        assert_ok!(Pos::distribute_epoch_rewards(RuntimeOrigin::root(), 10_000));
        assert_eq!(free(1), 13_250);
        assert_eq!(free(2), 13_250);
        assert_eq!(free(3), 12_000);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 8_500);
        assert!(recorded(Event::EpochRewardsDistributed {
            epoch: 0,
            total_distributed: 8_500
        }));
    });
}

#[test]
fn an_epoch_distribution_of_a_tiny_pool_pays_no_default_reward() {
    ext().execute_with(|| {
        for (who, value) in [(1u64, 90u32), (2, 85), (3, 50)] {
            register(who);
            score(who, value);
            admit(who);
        }
        // Every share of a pool of four rounds to zero. A zero share asks
        // the reward routine for its default reward of ten thousand, so the
        // distribution holds each zero share back.
        assert_ok!(Pos::distribute_epoch_rewards(RuntimeOrigin::root(), 4));
        assert_eq!(free(1), 10_000);
        assert_eq!(free(2), 10_000);
        assert_eq!(free(3), 10_000);
        assert_eq!(EpochTotalRewarded::<Test>::get(), 0);
        assert!(recorded(Event::EpochRewardsDistributed {
            epoch: 0,
            total_distributed: 0
        }));
    });
}

#[test]
fn an_epoch_distribution_past_the_validator_bound_declines_whole() {
    ext().execute_with(|| {
        for (who, value) in [(1u64, 90u32), (2, 85), (3, 50)] {
            register(who);
            score(who, value);
            admit(who);
        }
        assert_noop!(
            Pos::distribute_epoch_rewards(RuntimeOrigin::root(), 60_000),
            Error::<Test>::RewardBoundsExceeded
        );
    });
}

#[test]
fn the_stake_readers_answer_from_the_bonded_stake() {
    ext().execute_with(|| {
        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 5_000));
        assert_eq!(Pos::calculate_slash_amount(&1), Ok(500));
        assert_eq!(Pos::validator_stake_score(&1), 5_000);
        assert_eq!(Pos::stake(1), 5_000);
        assert_eq!(Pos::calculate_slash_amount(&2), Ok(0));
        assert_eq!(Pos::validator_stake_score(&2), 0);
    });
}

#[test]
fn the_slashing_history_reads_back_in_order() {
    ext().execute_with(|| {
        assert!(Pos::get_slashing_history(1).is_empty());
        let records = vec![
            SlashingRecord {
                block_number: 3u64,
                amount: 200u64,
                reason: SlashReason::Downtime,
                epoch: 1,
            },
            SlashingRecord {
                block_number: 9u64,
                amount: 700u64,
                reason: SlashReason::InvalidBlock,
                epoch: 2,
            },
        ];
        ValidatorSlashingHistory::<Test>::insert(
            1,
            BoundedVec::try_from(records.clone()).expect("the history fits"),
        );
        assert_eq!(Pos::get_slashing_history(1), records);
        assert_eq!(Pos::validator_slashing_history(1).into_inner(), records);
    });
}

#[test]
fn the_epoch_totals_report_and_reset() {
    ext().execute_with(|| {
        register(1);
        register(2);
        assert_ok!(Pos::reward_validator_call(RuntimeOrigin::root(), 1, 2_000));
        assert_ok!(Pos::slash_validator(RuntimeOrigin::root(), 2, 1_000));
        assert_eq!(Pos::calculate_epoch_economic_impact(0), (2_000, 1_000));
        assert_eq!(Pos::epoch_total_rewarded(), 2_000);
        assert_eq!(Pos::epoch_total_slashed(), 1_000);
        assert_eq!(Pos::validator_epoch_rewarded(1), 2_000);
        assert_eq!(Pos::validator_epoch_slashed(2), 1_000);

        Pos::reset_epoch_totals();
        assert_eq!(Pos::calculate_epoch_economic_impact(0), (0, 0));
        assert!(!ValidatorEpochRewarded::<Test>::contains_key(1));
        assert!(!ValidatorEpochSlashed::<Test>::contains_key(2));

        // A fresh epoch admits a full reward again.
        assert_ok!(Pos::reward_validator_call(RuntimeOrigin::root(), 1, 10_000));
    });
}

#[test]
fn a_stake_slash_lowers_the_stake_and_counts_toward_removal() {
    ext().execute_with(|| {
        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 5_000));
        assert_ok!(Pos::slash_validator_stake(&1, 1_200));
        assert_eq!(Stake::<Test>::get(1), 3_800);
        assert!(recorded(Event::ValidatorStakeSlashed {
            validator: 1,
            old_stake: 5_000,
            new_stake: 3_800,
            slashed_amount: 1_200
        }));
        assert!(recorded(Event::ValidatorSlashed {
            validator: 1,
            slashing_count: 1
        }));

        // A slash larger than the stake floors the stake at zero.
        assert_ok!(Pos::slash_validator_stake(&1, 9_000));
        assert_eq!(Stake::<Test>::get(1), 0);

        assert_ok!(Pos::slash_validator_stake(&1, 1));
        assert_eq!(Validators::<Test>::get(1), None);
        assert!(recorded(Event::ValidatorRemoved {
            validator: 1,
            reason: b"Max slashing count reached".to_vec()
        }));
    });
}

#[test]
fn a_stake_reward_raises_the_stake_and_records_both_events() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::reward_validator(&1, 500),
            Error::<Test>::ValidatorNotRegistered
        );
        register(1);
        assert_ok!(Pos::bond_stake(RuntimeOrigin::signed(1), 1_000));
        assert_ok!(Pos::reward_validator(&1, 500));
        assert_eq!(Stake::<Test>::get(1), 1_500);
        assert!(recorded(Event::StakeBonded {
            validator: 1,
            amount: 500
        }));
        assert!(recorded(Event::StakeUpdated {
            validator: 1,
            old_stake: 1_000,
            new_stake: 1_500
        }));
    });
}

#[test]
fn activation_and_deactivation_record_only_a_change() {
    ext().execute_with(|| {
        assert_noop!(
            Pos::activate_validator(&1),
            Error::<Test>::ValidatorNotRegistered
        );
        assert_noop!(
            Pos::deactivate_validator(&1),
            Error::<Test>::ValidatorNotRegistered
        );

        register(1);
        System::reset_events();
        assert_ok!(Pos::activate_validator(&1));
        assert!(System::events().is_empty());

        assert_ok!(Pos::deactivate_validator(&1));
        assert_eq!(Validators::<Test>::get(1), Some(false));
        assert!(recorded(Event::ValidatorDeactivated { validator: 1 }));

        System::reset_events();
        assert_ok!(Pos::deactivate_validator(&1));
        assert!(System::events().is_empty());

        assert_ok!(Pos::activate_validator(&1));
        assert_eq!(Validators::<Test>::get(1), Some(true));
        assert!(recorded(Event::ValidatorActivated { validator: 1 }));
    });
}

#[test]
fn a_forced_ejection_releases_the_stake_and_leaves_the_active_set() {
    ext().execute_with(|| {
        register(1);
        admit(1);
        assert_ok!(Pos::increase_validator_stake(
            RuntimeOrigin::signed(1),
            3_000
        ));

        assert_ok!(Pos::force_eject_validator(&1));
        assert_eq!(reserved(1), 0);
        assert_eq!(free(1), 10_000);
        assert_eq!(Validators::<Test>::get(1), None);
        assert!(!Stake::<Test>::contains_key(1));
        assert!(!Pos::get_active_validators().contains(&1));
        assert!(recorded(Event::ValidatorStakeUnreserved {
            validator: 1,
            amount: 3_000
        }));
        assert!(recorded(Event::ValidatorLeft { validator: 1 }));

        // An ejection without a stake records the exit alone.
        register(2);
        System::reset_events();
        assert_ok!(Pos::force_eject_validator(&2));
        assert_eq!(Validators::<Test>::get(2), None);
        assert!(!System::events().iter().any(|held| matches!(
            held.event,
            RuntimeEvent::PalletCeruleaPos(Event::ValidatorStakeUnreserved { .. })
        )));
        assert!(recorded(Event::ValidatorLeft { validator: 2 }));
    });
}

#[test]
fn the_genesis_build_writes_validators_scores_epoch_and_counts() {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("the system genesis builds");
    crate::GenesisConfig::<Test> {
        validators: vec![7, 8],
        validator_scores: vec![60, 70],
        current_epoch: 4,
        slashing_count: vec![(7, 2)],
    }
    .assimilate_storage(&mut storage)
    .expect("the stake genesis builds");

    sp_io::TestExternalities::new(storage).execute_with(|| {
        assert_eq!(Validators::<Test>::get(7), Some(true));
        assert_eq!(Validators::<Test>::get(8), Some(true));
        assert_eq!(ValidatorScores::<Test>::get(7), Some(60));
        assert_eq!(ValidatorScores::<Test>::get(8), Some(70));
        assert_eq!(Pos::current_epoch(), 4);
        assert_eq!(SlashingCount::<Test>::get(7), Some(2));
        assert_eq!(SlashingCount::<Test>::get(8), None);
    });
}

#[test]
fn the_block_hook_charges_no_weight() {
    ext().execute_with(|| {
        assert_eq!(<Pos as Hooks<u64>>::on_initialize(5), Weight::zero());
    });
}

#[test]
fn the_empty_handler_accepts_every_notice_and_reports_an_empty_set() {
    type Empty = ();
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_joined(&1, 1_000));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_leave_requested(
        &1
    ));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_left(&1));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_stake_increased(
        &1, 10
    ));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_stake_decreased(
        &1, 10
    ));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_slashed(&1, 10, 1));
    assert_ok!(<Empty as ValidatorHandler<u64, u64>>::on_rewarded(
        &1, 10, 1
    ));
    assert_eq!(
        <Empty as ValidatorHandler<u64, u64>>::get_validator_score(&1),
        0
    );
    assert!(<Empty as ValidatorHandler<u64, u64>>::get_active_validators().is_empty());
}

#[test]
fn the_mock_admission_engine_ignores_a_repeated_admission() {
    ext().execute_with(|| {
        admit(1);
        admit(1);
        assert_eq!(TestAdmission::admitted(), vec![1]);
        assert_ok!(<TestAdmission as ValidatorHandler<u64, u64>>::on_leave_requested(&1));
        assert_eq!(TestAdmission::admitted(), vec![1]);
    });
}
