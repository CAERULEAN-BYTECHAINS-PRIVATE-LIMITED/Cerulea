//! Coverage tests for the governance calls of the framework pallet.
//!
//! Each section drives one dispatchable call through the branches it holds.
//! Every call runs inside a storage layer. A refused call therefore leaves
//! storage exactly as it found it, which `assert_noop!` checks.

use crate::mock::*;
use crate::{
    AccountOperationHistory, ActiveValidators, BlockOperationCounts, CurrentEpoch,
    DispatchableType, EjectionReason, EpochConfig, EpochStats, Error, Event,
    GovernanceConfigStorage, GovernanceModeEnabled, GovernanceProposal, LastOperationBlock,
    NextProposalId, ParameterType, PoiWeight, PosWeight, ProposalAction, ProposalStatus,
    ProposalVotes, Proposals, RateLimitConfig, RateLimitConfigStorage, TrustScoreBounds,
    TrustScoreConfigStorage, ValidatorSet, ValidatorStates,
};
use codec::Encode;
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU32, Currency, Get, ReservableCurrency},
    BoundedVec,
};
use sp_runtime::{DispatchError, DispatchResult};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Read every event the framework pallet deposited so far.
fn dcf_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// Move the chain one block forward and clear the per-block counters.
///
/// The pallet clears the counters in its block hook. The tests skip the hook,
/// so this helper clears them the same way.
fn next_block() {
    let now = System::block_number();
    System::set_block_number(now + 1);
    let _ = BlockOperationCounts::<Test>::clear(u32::MAX, None);
}

/// Encode one parameter value the way the parameter call reads it.
fn bounded_value<V: Encode>(value: V) -> BoundedVec<u8, ConstU32<64>> {
    BoundedVec::truncate_from(value.encode())
}

/// Update one governance parameter from root.
fn set_parameter(parameter: ParameterType, value: BoundedVec<u8, ConstU32<64>>) -> DispatchResult {
    DcfPallet::update_dcf_parameter(RuntimeOrigin::root(), parameter, value)
}

/// The proposal record a fresh submission stores.
fn pending(proposer: u64, action: ProposalAction<Test>) -> GovernanceProposal<Test> {
    GovernanceProposal {
        proposer,
        action,
        status: ProposalStatus::Pending,
        votes_for: 0,
        votes_against: 0,
    }
}

/// A small reward action the rate limit tests submit.
fn reward_action() -> ProposalAction<Test> {
    ProposalAction::Reward {
        validator: 2,
        amount: 100,
    }
}

/// Open one reward proposal from root and answer its identifier.
fn open_reward_proposal() -> u32 {
    let proposal_id = NextProposalId::<Test>::get();
    assert_ok!(DcfPallet::propose_reward_validator(
        RuntimeOrigin::root(),
        1,
        2,
        100
    ));
    proposal_id
}

/// Read the status of one proposal.
fn status_of(proposal_id: u32) -> Option<ProposalStatus> {
    Proposals::<Test>::get(proposal_id).map(|proposal| proposal.status)
}

/// Carry one proposal to approval and execute it from root.
fn approve_and_execute(proposal_id: u32) {
    assert_ok!(mock_approve_proposal(proposal_id));
    assert_eq!(status_of(proposal_id), Some(ProposalStatus::Approved));
    assert_ok!(DcfPallet::execute_proposal(
        RuntimeOrigin::root(),
        proposal_id
    ));
    assert_eq!(status_of(proposal_id), Some(ProposalStatus::Executed));
    assert!(dcf_events().contains(&Event::ProposalExecuted {
        proposal_id,
        status: ProposalStatus::Executed,
    }));
}

/// Carry one proposal to approval and ask root to execute it.
///
/// The action inside the proposal fails. A failed action leaves the proposal
/// recorded as something other than executed, and no executed announcement
/// names it.
fn approve_and_attempt_failing_execution(proposal_id: u32) {
    assert_ok!(mock_approve_proposal(proposal_id));
    assert_eq!(status_of(proposal_id), Some(ProposalStatus::Approved));

    let outcome = DcfPallet::execute_proposal(RuntimeOrigin::root(), proposal_id);

    assert_ne!(
        status_of(proposal_id),
        Some(ProposalStatus::Executed),
        "A proposal whose action failed is recorded as failed. Outcome: {:?}",
        outcome
    );
    assert!(proposal_id_absent_from_executed_events(proposal_id));
}

/// Hold that no executed announcement names one proposal.
fn proposal_id_absent_from_executed_events(proposal_id: u32) -> bool {
    !dcf_events().contains(&Event::ProposalExecuted {
        proposal_id,
        status: ProposalStatus::Executed,
    })
}

fn total_balance(who: u64) -> u128 {
    <Balances as Currency<u64>>::total_balance(&who)
}

fn free_balance(who: u64) -> u128 {
    <Balances as Currency<u64>>::free_balance(&who)
}

fn reserved_balance(who: u64) -> u128 {
    <Balances as ReservableCurrency<u64>>::reserved_balance(&who)
}

/// The epoch record the mock runtime accepts at genesis.
fn epoch(blocks_per_epoch: u32) -> EpochConfig {
    EpochConfig {
        blocks_per_epoch,
        min_stake: 1000,
        max_validators: 100,
    }
}

/// Hold that both genesis checks refuse one genesis record.
fn assert_genesis_refused(validators: Vec<u64>, stakes: Vec<u128>, epoch_record: EpochConfig) {
    assert_noop!(
        DcfPallet::validate_genesis_configuration(
            RuntimeOrigin::root(),
            validators.clone(),
            stakes.clone(),
            epoch_record.clone()
        ),
        Error::<Test>::GenesisValidationFailed
    );
    assert_noop!(
        DcfPallet::dry_run_genesis_configuration(
            RuntimeOrigin::root(),
            validators,
            stakes,
            epoch_record
        ),
        Error::<Test>::GenesisValidationFailed
    );
}

// ---------------------------------------------------------------------------
// update_validator_stake_score
// ---------------------------------------------------------------------------

#[test]
fn stake_score_update_records_the_stake_and_caps_the_final_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(4),
            1
        ));

        let state = ValidatorStates::<Test>::get(&1).expect("Validator 1 holds a state record.");
        let stake = pallet_cerulea_pos::Stake::<Test>::get(1u64);
        assert_eq!(state.current.stake_score, stake as u64);
        assert_eq!(
            state.current.inference_score,
            DcfPallet::inference_contribution(&1)
        );

        // The final score weighs both shares and holds at the score ceiling.
        let ceiling = <Test as pallet_cerulea_poi::Config>::MaxValidatorScore::get();
        let weighted = (state.current.stake_score * PosWeight::<Test>::get()
            + state.current.inference_score * PoiWeight::<Test>::get())
            / 10_000;
        assert_eq!(state.current.final_score, weighted.min(ceiling));

        assert!(dcf_events().contains(&Event::ValidatorScoreUpdated {
            validator: 1,
            stake_score: state.current.stake_score,
            inference_score: state.current.inference_score,
            final_score: state.current.final_score,
        }));
    });
}

#[test]
fn stake_score_update_announces_the_previous_scores() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let before = ValidatorStates::<Test>::get(&1).expect("Validator 1 holds a state record.");

        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(4),
            1
        ));

        // The detailed event carries the scores held before the update.
        let announced = dcf_events()
            .into_iter()
            .find_map(|event| match event {
                Event::ValidatorScoreUpdatedDetailed {
                    validator: 1,
                    old_inference_score,
                    old_final_score,
                    ..
                } => Some((old_inference_score, old_final_score)),
                _ => None,
            })
            .expect("The detailed score event was deposited.");
        assert_eq!(
            announced,
            (before.current.inference_score, before.current.final_score),
            "The detailed event reports the scores held before the update."
        );
    });
}

#[test]
fn stake_score_update_appends_one_history_entry() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let before = ValidatorStates::<Test>::get(&1).expect("Validator 1 holds a state record.");
        assert_eq!(before.history.len(), 1);

        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::signed(4),
            1
        ));

        // The history holds ten entries. One update adds one entry.
        let after = ValidatorStates::<Test>::get(&1).expect("Validator 1 holds a state record.");
        assert_eq!(
            after.history.len(),
            2,
            "The history keeps the genesis entry beside the new entry."
        );
        assert_eq!(
            after.history.last(),
            Some(&EpochStats {
                epoch: 0,
                stake_score: after.current.stake_score,
                inference_score: after.current.inference_score,
                final_score: after.current.final_score,
                authored_blocks: 0,
                missed_blocks: 0,
            })
        );
    });
}

#[test]
fn stake_score_update_refuses_an_unknown_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            DcfPallet::update_validator_stake_score(RuntimeOrigin::signed(1), 99),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn stake_score_update_outside_governance_mode_asks_for_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert!(!GovernanceModeEnabled::<Test>::get());
        assert_noop!(
            DcfPallet::update_validator_stake_score(RuntimeOrigin::root(), 1),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::update_validator_stake_score(RuntimeOrigin::none(), 1),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn stake_score_update_in_governance_mode_asks_for_root() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));

        assert_noop!(
            DcfPallet::update_validator_stake_score(RuntimeOrigin::signed(1), 1),
            DispatchError::BadOrigin
        );
        assert_ok!(DcfPallet::update_validator_stake_score(
            RuntimeOrigin::root(),
            1
        ));
        let state = ValidatorStates::<Test>::get(&1).expect("Validator 1 holds a state record.");
        assert_eq!(
            state.current.stake_score,
            pallet_cerulea_pos::Stake::<Test>::get(1u64) as u64
        );
    });
}

// ---------------------------------------------------------------------------
// update_consensus_weights
// ---------------------------------------------------------------------------

#[test]
fn consensus_weights_update_stores_both_shares_and_announces_them() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::update_consensus_weights(
            RuntimeOrigin::root(),
            7_000,
            3_000
        ));
        assert_eq!(PosWeight::<Test>::get(), 7_000);
        assert_eq!(PoiWeight::<Test>::get(), 3_000);
        assert!(dcf_events().contains(&Event::ConsensusWeightsUpdated {
            pos_weight: 7_000,
            poi_weight: 3_000,
        }));
    });
}

#[test]
fn consensus_weights_update_refuses_a_pair_off_the_precision() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 6_000, 3_000),
            Error::<Test>::InvalidWeight
        );
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), 6_000, 4_001),
            Error::<Test>::InvalidWeight
        );
    });
}

#[test]
fn consensus_weights_update_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::signed(1), 7_000, 3_000),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn consensus_weights_update_refuses_an_overflowing_pair() {
    new_test_ext().execute_with(|| {
        // The sum of the two shares passes the integer bound. The call answers
        // with the weight error and leaves storage untouched.
        assert_noop!(
            DcfPallet::update_consensus_weights(RuntimeOrigin::root(), u64::MAX, 1),
            Error::<Test>::InvalidWeight
        );
    });
}

// ---------------------------------------------------------------------------
// update_dcf_parameter
// ---------------------------------------------------------------------------

#[test]
fn parameter_update_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_dcf_parameter(
                RuntimeOrigin::signed(1),
                ParameterType::EpochLength,
                bounded_value(200u32)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn epoch_length_update_stores_and_announces_old_and_new() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let old = GovernanceConfigStorage::<Test>::get().epoch_length.current;

        assert_ok!(set_parameter(
            ParameterType::EpochLength,
            bounded_value(200u32)
        ));

        assert_eq!(
            GovernanceConfigStorage::<Test>::get().epoch_length.current,
            200
        );
        assert!(dcf_events().contains(&Event::DcfParameterUpdated {
            parameter: ParameterType::EpochLength,
            old_value: bounded_value(old),
            new_value: bounded_value(200u32),
        }));
    });
}

#[test]
fn epoch_length_update_refuses_values_outside_the_range() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            set_parameter(ParameterType::EpochLength, bounded_value(9u32)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(ParameterType::EpochLength, bounded_value(14_401u32)),
            Error::<Test>::ParameterOutOfRange
        );
    });
}

#[test]
fn parameter_update_refuses_an_undecodable_value() {
    new_test_ext().execute_with(|| {
        // One byte decodes into no four byte integer.
        assert_noop!(
            set_parameter(
                ParameterType::EpochLength,
                BoundedVec::truncate_from(vec![7u8])
            ),
            Error::<Test>::InvalidParameterType
        );
        // Three bytes decode into no sixteen byte balance.
        assert_noop!(
            set_parameter(
                ParameterType::MinStake,
                BoundedVec::truncate_from(vec![1u8, 2, 3])
            ),
            Error::<Test>::InvalidParameterType
        );
    });
}

#[test]
fn min_stake_and_max_validators_updates_hold_their_ranges() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::MinStake,
            bounded_value(200_000u128)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get().min_stake.current,
            200_000
        );
        assert_noop!(
            set_parameter(ParameterType::MinStake, bounded_value(1_000u128)),
            Error::<Test>::ParameterOutOfRange
        );

        assert_ok!(set_parameter(
            ParameterType::MaxValidators,
            bounded_value(50u32)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get()
                .max_validators
                .current,
            50
        );
        assert_noop!(
            set_parameter(ParameterType::MaxValidators, bounded_value(0u32)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(ParameterType::MaxValidators, bounded_value(1_001u32)),
            Error::<Test>::ParameterOutOfRange
        );
    });
}

#[test]
fn stake_share_update_needs_a_matching_inference_share() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            set_parameter(ParameterType::PosWeight, bounded_value(500u64)),
            Error::<Test>::ParameterOutOfRange
        );
        // The inference share holds 4000, so 7000 misses the precision.
        assert_noop!(
            set_parameter(ParameterType::PosWeight, bounded_value(7_000u64)),
            Error::<Test>::InvalidWeight
        );

        GovernanceConfigStorage::<Test>::mutate(|ranges| ranges.poi_weight.current = 3_000);
        assert_ok!(set_parameter(
            ParameterType::PosWeight,
            bounded_value(7_000u64)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get().pos_weight.current,
            7_000
        );
        assert_eq!(PosWeight::<Test>::get(), 7_000);
    });
}

#[test]
fn inference_share_update_needs_a_matching_stake_share() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            set_parameter(ParameterType::PoiWeight, bounded_value(9_500u64)),
            Error::<Test>::ParameterOutOfRange
        );
        // The stake share holds 6000, so 3000 misses the precision.
        assert_noop!(
            set_parameter(ParameterType::PoiWeight, bounded_value(3_000u64)),
            Error::<Test>::InvalidWeight
        );

        GovernanceConfigStorage::<Test>::mutate(|ranges| ranges.pos_weight.current = 7_000);
        assert_ok!(set_parameter(
            ParameterType::PoiWeight,
            bounded_value(3_000u64)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get().poi_weight.current,
            3_000
        );
        assert_eq!(PoiWeight::<Test>::get(), 3_000);
    });
}

#[test]
fn performance_score_floor_stays_under_the_high_mark() {
    new_test_ext().execute_with(|| {
        let high = GovernanceConfigStorage::<Test>::get()
            .high_performance_score
            .current;

        assert_noop!(
            set_parameter(ParameterType::MinPerformanceScore, bounded_value(5u64)),
            Error::<Test>::ParameterOutOfRange
        );
        // A floor equal to the high mark breaks the order of the two marks.
        assert_noop!(
            set_parameter(ParameterType::MinPerformanceScore, bounded_value(high)),
            Error::<Test>::InvalidGovernanceConfig
        );

        let floor = high - 1;
        assert_ok!(set_parameter(
            ParameterType::MinPerformanceScore,
            bounded_value(floor)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get()
                .min_performance_score
                .current,
            floor
        );
    });
}

#[test]
fn high_performance_score_accepts_a_value_under_the_score_ceiling() {
    new_test_ext().execute_with(|| {
        // The score scale ends at the score ceiling. A high mark between the
        // floor and the ceiling is a valid high mark.
        let ceiling = <Test as pallet_cerulea_poi::Config>::MaxValidatorScore::get();
        let floor = GovernanceConfigStorage::<Test>::get()
            .min_performance_score
            .current;
        let mark = 90u64;
        assert!(mark > floor && mark <= ceiling);

        assert_ok!(set_parameter(
            ParameterType::HighPerformanceScore,
            bounded_value(mark)
        ));
        assert_eq!(
            GovernanceConfigStorage::<Test>::get()
                .high_performance_score
                .current,
            mark
        );
    });
}

#[test]
fn participation_rates_keep_their_order() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            set_parameter(ParameterType::MinParticipationRate, bounded_value(5u32)),
            Error::<Test>::ParameterOutOfRange
        );
        // The high rate holds 90. A floor of 90 breaks the order.
        assert_noop!(
            set_parameter(ParameterType::MinParticipationRate, bounded_value(90u32)),
            Error::<Test>::InvalidGovernanceConfig
        );
        assert_ok!(set_parameter(
            ParameterType::MinParticipationRate,
            bounded_value(60u32)
        ));

        assert_noop!(
            set_parameter(ParameterType::HighParticipationRate, bounded_value(101u32)),
            Error::<Test>::ParameterOutOfRange
        );
        // The floor now holds 60. A high rate of 60 breaks the order.
        assert_noop!(
            set_parameter(ParameterType::HighParticipationRate, bounded_value(60u32)),
            Error::<Test>::InvalidGovernanceConfig
        );
        assert_ok!(set_parameter(
            ParameterType::HighParticipationRate,
            bounded_value(95u32)
        ));

        let ranges = GovernanceConfigStorage::<Test>::get();
        assert_eq!(ranges.min_participation_rate.current, 60);
        assert_eq!(ranges.high_participation_rate.current, 95);
    });
}

#[test]
fn economic_parameters_hold_their_ranges() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::ValidatorReward,
            bounded_value(20_000u128)
        ));
        assert_noop!(
            set_parameter(ParameterType::ValidatorReward, bounded_value(500u128)),
            Error::<Test>::ParameterOutOfRange
        );

        assert_ok!(set_parameter(
            ParameterType::SlashPercent,
            bounded_value(20u32)
        ));
        assert_noop!(
            set_parameter(ParameterType::SlashPercent, bounded_value(0u32)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(ParameterType::SlashPercent, bounded_value(51u32)),
            Error::<Test>::ParameterOutOfRange
        );

        assert_ok!(set_parameter(
            ParameterType::LeaveCooldown,
            bounded_value(500u64)
        ));
        assert_noop!(
            set_parameter(ParameterType::LeaveCooldown, bounded_value(99u64)),
            Error::<Test>::ParameterOutOfRange
        );

        let ranges = GovernanceConfigStorage::<Test>::get();
        assert_eq!(ranges.validator_reward.current, 20_000);
        assert_eq!(ranges.slash_percent.current, 20);
        assert_eq!(ranges.leave_cooldown.current, 500);
    });
}

#[test]
fn trust_score_weights_reach_the_trust_score_record() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::TrustScoreUptimeWeight,
            bounded_value(50u32)
        ));
        assert_ok!(set_parameter(
            ParameterType::TrustScoreInferenceWeight,
            bounded_value(30u32)
        ));
        assert_ok!(set_parameter(
            ParameterType::TrustScoreSlashingWeight,
            bounded_value(20u32)
        ));

        let record = TrustScoreConfigStorage::<Test>::get();
        assert_eq!(record.uptime_weight, 50);
        assert_eq!(record.inference_weight, 30);
        assert_eq!(record.slashing_weight, 20);

        let ranges = GovernanceConfigStorage::<Test>::get();
        assert_eq!(ranges.trust_score_uptime_weight.current, 50);
        assert_eq!(ranges.trust_score_inference_weight.current, 30);
        assert_eq!(ranges.trust_score_slashing_weight.current, 20);

        assert_noop!(
            set_parameter(ParameterType::TrustScoreUptimeWeight, bounded_value(81u32)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(
                ParameterType::TrustScoreInferenceWeight,
                bounded_value(9u32)
            ),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(
                ParameterType::TrustScoreSlashingWeight,
                bounded_value(51u32)
            ),
            Error::<Test>::ParameterOutOfRange
        );
    });
}

#[test]
fn trust_score_rates_reach_the_trust_score_bounds() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::TrustScoreMaxGrowthRate,
            bounded_value(1_000u32)
        ));
        assert_ok!(set_parameter(
            ParameterType::TrustScoreMaxDecayRate,
            bounded_value(300u32)
        ));
        assert_ok!(set_parameter(
            ParameterType::TrustScoreStabilityFactor,
            bounded_value(9_000u32)
        ));

        let record = TrustScoreConfigStorage::<Test>::get();
        assert_eq!(record.max_growth_rate, 1_000);
        assert_eq!(record.max_decay_rate, 300);
        assert_eq!(record.stability_factor, 9_000);

        let bounds = TrustScoreBounds::<Test>::get();
        assert_eq!(bounds.max_growth_rate, 1_000);
        assert_eq!(bounds.max_decay_rate, 300);
        assert_eq!(bounds.stability_factor, 9_000);
        assert_eq!(bounds.last_updated_epoch, DcfPallet::current_epoch());

        assert_noop!(
            set_parameter(ParameterType::TrustScoreMaxGrowthRate, bounded_value(99u32)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(
                ParameterType::TrustScoreMaxDecayRate,
                bounded_value(1_001u32)
            ),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(
                ParameterType::TrustScoreStabilityFactor,
                bounded_value(9_501u32)
            ),
            Error::<Test>::ParameterOutOfRange
        );
    });
}

#[test]
fn trust_score_value_bounds_reach_the_trust_score_bounds() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::TrustScoreMinValue,
            bounded_value(2_000u64)
        ));
        assert_ok!(set_parameter(
            ParameterType::TrustScoreMaxValue,
            bounded_value(12_000u64)
        ));

        let record = TrustScoreConfigStorage::<Test>::get();
        assert_eq!(record.min_trust_score, 2_000);
        assert_eq!(record.max_trust_score, 12_000);

        let bounds = TrustScoreBounds::<Test>::get();
        assert_eq!(bounds.min_score, 2_000);
        assert_eq!(bounds.max_score, 12_000);

        assert_noop!(
            set_parameter(ParameterType::TrustScoreMinValue, bounded_value(499u64)),
            Error::<Test>::ParameterOutOfRange
        );
        assert_noop!(
            set_parameter(ParameterType::TrustScoreMaxValue, bounded_value(15_001u64)),
            Error::<Test>::ParameterOutOfRange
        );
    });
}

#[test]
fn block_and_inference_boosts_hold_their_ranges() {
    new_test_ext().execute_with(|| {
        assert_ok!(set_parameter(
            ParameterType::BlockAuthorshipBoost,
            bounded_value(20u64)
        ));
        assert_ok!(set_parameter(
            ParameterType::MissedBlockPenalty,
            bounded_value(10u64)
        ));
        assert_ok!(set_parameter(
            ParameterType::InferenceBoostLow,
            bounded_value(3u64)
        ));
        assert_ok!(set_parameter(
            ParameterType::InferenceBoostMedium,
            bounded_value(10u64)
        ));
        assert_ok!(set_parameter(
            ParameterType::InferenceBoostHigh,
            bounded_value(20u64)
        ));

        let ranges = GovernanceConfigStorage::<Test>::get();
        assert_eq!(ranges.block_authorship_boost.current, 20);
        assert_eq!(ranges.missed_block_penalty.current, 10);
        assert_eq!(ranges.inference_boost_low.current, 3);
        assert_eq!(ranges.inference_boost_medium.current, 10);
        assert_eq!(ranges.inference_boost_high.current, 20);

        for (parameter, value) in [
            (ParameterType::BlockAuthorshipBoost, 0u64),
            (ParameterType::MissedBlockPenalty, 51u64),
            (ParameterType::InferenceBoostLow, 21u64),
            (ParameterType::InferenceBoostMedium, 1u64),
            (ParameterType::InferenceBoostHigh, 101u64),
        ] {
            assert_noop!(
                set_parameter(parameter, bounded_value(value)),
                Error::<Test>::ParameterOutOfRange
            );
        }
    });
}

// ---------------------------------------------------------------------------
// update_rate_limit_config
// ---------------------------------------------------------------------------

#[test]
fn rate_limit_update_stores_three_limits_and_keeps_the_rest() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            10,
            5,
            4
        ));

        let stored = RateLimitConfigStorage::<Test>::get();
        assert_eq!(
            stored,
            RateLimitConfig {
                max_proposals_per_block: 10,
                max_joins_per_block: 5,
                max_leaves_per_block: 4,
                ..RateLimitConfig::default()
            }
        );
        assert!(dcf_events().contains(&Event::RateLimitConfigUpdated {
            max_proposals_per_block: 10,
            max_joins_per_block: 5,
            max_leaves_per_block: 4,
        }));
    });
}

#[test]
fn rate_limit_update_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::signed(1), 10, 5, 4),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn rate_limit_update_holds_the_per_block_bounds() {
    new_test_ext().execute_with(|| {
        for (proposals, joins, leaves) in [
            (0u32, 3u32, 3u32),
            (101, 3, 3),
            (5, 0, 3),
            (5, 51, 3),
            (5, 3, 0),
            (5, 3, 51),
        ] {
            assert_noop!(
                DcfPallet::update_rate_limit_config(
                    RuntimeOrigin::root(),
                    proposals,
                    joins,
                    leaves
                ),
                Error::<Test>::InvalidRateLimitConfig
            );
        }

        // The upper bound of each limit is accepted.
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            100,
            50,
            50
        ));
        assert_eq!(
            RateLimitConfigStorage::<Test>::get().max_proposals_per_block,
            100
        );
    });
}

#[test]
fn rate_limit_update_refuses_an_invalid_stored_record() {
    new_test_ext().execute_with(|| {
        let base = RateLimitConfig::default();
        let refused = vec![
            RateLimitConfig {
                max_votes_per_block: 0,
                ..base.clone()
            },
            RateLimitConfig {
                max_votes_per_block: 1_001,
                ..base.clone()
            },
            RateLimitConfig {
                max_proposals_per_account: 0,
                ..base.clone()
            },
            RateLimitConfig {
                max_status_changes_per_account: 21,
                ..base.clone()
            },
            RateLimitConfig {
                proposal_rate_window: 0,
                ..base.clone()
            },
            RateLimitConfig {
                status_change_rate_window: 100_001,
                ..base.clone()
            },
            RateLimitConfig {
                min_validator_status_interval: 10_001,
                ..base.clone()
            },
            RateLimitConfig {
                min_proposal_interval: 1_001,
                ..base.clone()
            },
            RateLimitConfig {
                min_vote_interval: 101,
                ..base.clone()
            },
            RateLimitConfig {
                max_validator_iteration_weight: 0,
                ..base.clone()
            },
            RateLimitConfig {
                max_proposal_processing_weight: 0,
                ..base.clone()
            },
            RateLimitConfig {
                max_loop_iterations: 0,
                ..base.clone()
            },
            RateLimitConfig {
                max_loop_iterations: 10_001,
                ..base.clone()
            },
        ];

        // The call keeps every stored limit beside the three it carries. A
        // stored limit outside its bound refuses the whole update.
        for stored in refused {
            RateLimitConfigStorage::<Test>::put(stored.clone());
            assert_noop!(
                DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 3, 3),
                Error::<Test>::InvalidRateLimitConfig
            );
            assert_eq!(RateLimitConfigStorage::<Test>::get(), stored);
        }
    });
}

// ---------------------------------------------------------------------------
// set_governance_mode and sudo_advance_epoch
// ---------------------------------------------------------------------------

#[test]
fn governance_mode_toggle_stores_the_flag_and_announces_it() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);

        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        assert!(GovernanceModeEnabled::<Test>::get());
        assert!(dcf_events().contains(&Event::GovernanceModeToggled { enabled: true }));

        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), false));
        assert!(!GovernanceModeEnabled::<Test>::get());
        assert!(dcf_events().contains(&Event::GovernanceModeToggled { enabled: false }));
    });
}

#[test]
fn governance_mode_toggle_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_governance_mode(RuntimeOrigin::signed(1), true),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn manual_epoch_advance_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        assert_noop!(
            DcfPallet::sudo_advance_epoch(RuntimeOrigin::signed(1)),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn manual_epoch_advance_needs_governance_mode() {
    new_test_ext().execute_with(|| {
        assert!(!GovernanceModeEnabled::<Test>::get());
        assert_noop!(
            DcfPallet::sudo_advance_epoch(RuntimeOrigin::root()),
            Error::<Test>::NotAllowedInGovernanceMode
        );
    });
}

#[test]
fn manual_epoch_advance_moves_the_epoch_forward() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));
        let before = CurrentEpoch::<Test>::get();

        assert_ok!(DcfPallet::sudo_advance_epoch(RuntimeOrigin::root()));

        assert_eq!(CurrentEpoch::<Test>::get(), before + 1);
        assert!(dcf_events()
            .iter()
            .any(|event| matches!(event, Event::EpochStarted { epoch: started, .. } if *started == before + 1)));
    });
}

// ---------------------------------------------------------------------------
// submit_proposal
// ---------------------------------------------------------------------------

#[test]
fn submitted_proposal_is_stored_pending_and_announced() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let description: BoundedVec<u8, ConstU32<128>> =
            BoundedVec::truncate_from(b"Reward validator 2 for steady service".to_vec());

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            reward_action(),
            Some(description.clone())
        ));

        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, reward_action()))
        );
        assert_eq!(NextProposalId::<Test>::get(), 1);

        let events = dcf_events();
        assert!(events.contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action: reward_action(),
        }));
        assert!(events.contains(&Event::ProposalCreated {
            proposal_id: 0,
            proposer: 1,
            action: reward_action(),
            description,
        }));
    });
}

#[test]
fn submitted_proposal_without_text_carries_the_default_description() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            reward_action(),
            None
        ));
        assert!(dcf_events().contains(&Event::ProposalCreated {
            proposal_id: 0,
            proposer: 1,
            action: reward_action(),
            description: BoundedVec::truncate_from(b"No description provided".to_vec()),
        }));
    });
}

#[test]
fn submitted_proposal_records_the_operation_for_rate_limiting() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            reward_action(),
            None
        ));

        assert_eq!(
            BlockOperationCounts::<Test>::get(DispatchableType::SubmitProposal),
            1
        );
        assert_eq!(
            LastOperationBlock::<Test>::get(1u64, DispatchableType::SubmitProposal),
            Some(1)
        );
        assert_eq!(
            AccountOperationHistory::<Test>::get(1u64, DispatchableType::SubmitProposal).to_vec(),
            vec![1u32]
        );
    });
}

#[test]
fn submitted_proposal_keeps_every_action_kind() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let actions: Vec<ProposalAction<Test>> = vec![
            ProposalAction::Slash {
                validator: 2,
                amount: 500,
            },
            ProposalAction::Reward {
                validator: 2,
                amount: 700,
            },
            ProposalAction::Eject {
                validator: 3,
                reason: EjectionReason::RepeatedMisbehavior,
            },
            ProposalAction::AddValidator { validator: 10 },
            ProposalAction::RemoveValidator { validator: 3 },
            ProposalAction::RewardMultiple {
                validators: BoundedVec::truncate_from(vec![1u64, 2u64]),
                amount: 900,
            },
        ];

        for (index, action) in actions.into_iter().enumerate() {
            next_block();
            let proposer = 10 + index as u64;
            assert_ok!(DcfPallet::submit_proposal(
                RuntimeOrigin::signed(proposer),
                action.clone(),
                None
            ));
            assert_eq!(
                Proposals::<Test>::get(index as u32),
                Some(pending(proposer, action))
            );
        }
        assert_eq!(NextProposalId::<Test>::get(), 6);
    });
}

#[test]
fn proposal_submission_refuses_root_and_unsigned_origins() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::root(), reward_action(), None),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::none(), reward_action(), None),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn proposal_submission_refuses_a_full_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let limit = RateLimitConfigStorage::<Test>::get().max_proposals_per_block;
        BlockOperationCounts::<Test>::insert(DispatchableType::SubmitProposal, limit);

        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::signed(1), reward_action(), None),
            Error::<Test>::PerBlockRateLimitExceeded
        );
    });
}

#[test]
fn proposal_submission_refuses_an_account_over_its_window_limit() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let limit = RateLimitConfigStorage::<Test>::get().max_proposals_per_account;
        let history: BoundedVec<u32, ConstU32<100>> =
            BoundedVec::truncate_from(vec![1u32; limit as usize]);
        AccountOperationHistory::<Test>::insert(1u64, DispatchableType::SubmitProposal, history);

        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::signed(1), reward_action(), None),
            Error::<Test>::PerAccountRateLimitExceeded
        );
    });
}

#[test]
fn proposal_submission_holds_the_minimum_interval() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let interval = RateLimitConfigStorage::<Test>::get().min_proposal_interval as u64;

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            reward_action(),
            None
        ));
        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::signed(1), reward_action(), None),
            Error::<Test>::MinimumIntervalViolation
        );

        // One block short of the interval still refuses the proposal.
        System::set_block_number(interval);
        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::signed(1), reward_action(), None),
            Error::<Test>::MinimumIntervalViolation
        );

        // The full interval admits the next proposal.
        System::set_block_number(1 + interval);
        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            reward_action(),
            None
        ));
        assert_eq!(NextProposalId::<Test>::get(), 2);
    });
}

#[test]
fn proposal_submission_refuses_a_call_over_the_weight_bound() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        RateLimitConfigStorage::<Test>::put(RateLimitConfig {
            max_proposal_processing_weight: 1_000,
            ..RateLimitConfig::default()
        });

        assert_noop!(
            DcfPallet::submit_proposal(RuntimeOrigin::signed(1), reward_action(), None),
            Error::<Test>::WeightLimitExceeded
        );
    });
}

#[test]
fn private_chain_refuses_a_proposer_outside_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode(vec![1, 2, 3], false));

        assert_noop!(
            DcfPallet::submit_proposal(
                RuntimeOrigin::signed(10),
                ProposalAction::AddValidator { validator: 1 },
                None
            ),
            Error::<Test>::NotInAllowlist
        );
    });
}

#[test]
fn private_chain_refuses_a_slash_or_reward_target_outside_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode(vec![1, 2, 3], false));

        assert_noop!(
            DcfPallet::submit_proposal(
                RuntimeOrigin::signed(1),
                ProposalAction::Slash {
                    validator: 50,
                    amount: 100
                },
                None
            ),
            Error::<Test>::TargetNotInAllowlist
        );
        assert_noop!(
            DcfPallet::submit_proposal(
                RuntimeOrigin::signed(1),
                ProposalAction::Reward {
                    validator: 50,
                    amount: 100
                },
                None
            ),
            Error::<Test>::TargetNotInAllowlist
        );
    });
}

#[test]
fn private_chain_refuses_an_eject_target_outside_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode(vec![1, 2, 3], false));

        assert_noop!(
            DcfPallet::submit_proposal(
                RuntimeOrigin::signed(1),
                ProposalAction::Eject {
                    validator: 50,
                    reason: EjectionReason::ManualEjection
                },
                None
            ),
            Error::<Test>::TargetNotInAllowlist
        );
    });
}

#[test]
fn private_chain_accepts_proposals_inside_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode(vec![1, 2, 3], false));

        // Both sides of a slash sit on the allowlist.
        let slash = ProposalAction::Slash {
            validator: 2,
            amount: 100,
        };
        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            slash.clone(),
            None
        ));

        // An action without a target checks the proposer alone.
        let add = ProposalAction::AddValidator { validator: 50 };
        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(2),
            add.clone(),
            None
        ));

        assert_eq!(Proposals::<Test>::get(0u32), Some(pending(1, slash)));
        assert_eq!(Proposals::<Test>::get(1u32), Some(pending(2, add)));
    });
}

// ---------------------------------------------------------------------------
// vote_proposal
// ---------------------------------------------------------------------------

#[test]
fn first_vote_below_quorum_leaves_the_proposal_pending() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();

        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            true
        ));

        let proposal = Proposals::<Test>::get(proposal_id).expect("The proposal is stored.");
        assert_eq!(proposal.votes_for, 1);
        assert_eq!(proposal.votes_against, 0);
        assert_eq!(proposal.status, ProposalStatus::Pending);
        assert_eq!(ProposalVotes::<Test>::get(proposal_id, 1u64), Some(true));
        assert_eq!(
            LastOperationBlock::<Test>::get(1u64, DispatchableType::VoteProposal),
            Some(1)
        );
        assert!(dcf_events().contains(&Event::ProposalVoted {
            proposal_id,
            voter: 1,
            approve: true,
        }));
    });
}

#[test]
fn votes_reaching_quorum_in_favour_approve_the_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();

        // Three active validators set the quorum at two votes.
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            true
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(2),
            proposal_id,
            true
        ));

        assert_eq!(status_of(proposal_id), Some(ProposalStatus::Approved));
        assert!(dcf_events().contains(&Event::ProposalPassed { proposal_id }));
    });
}

#[test]
fn votes_reaching_quorum_against_reject_the_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();

        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            false
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(2),
            proposal_id,
            false
        ));

        let proposal = Proposals::<Test>::get(proposal_id).expect("The proposal is stored.");
        assert_eq!(proposal.votes_against, 2);
        assert_eq!(proposal.status, ProposalStatus::Rejected);
        assert_eq!(ProposalVotes::<Test>::get(proposal_id, 2u64), Some(false));
        assert!(dcf_events().contains(&Event::ProposalRejected { proposal_id }));
    });
}

#[test]
fn tied_votes_at_quorum_reject_the_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();

        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            true
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(2),
            proposal_id,
            false
        ));

        assert_eq!(status_of(proposal_id), Some(ProposalStatus::Rejected));
        assert!(dcf_events().contains(&Event::ProposalRejected { proposal_id }));
    });
}

#[test]
fn vote_refuses_an_unknown_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), 7, true),
            Error::<Test>::ProposalNotApproved
        );
    });
}

#[test]
fn vote_refuses_a_closed_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            true
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(2),
            proposal_id,
            true
        ));

        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(3), proposal_id, true),
            Error::<Test>::ProposalAlreadyExecuted
        );
    });
}

#[test]
fn vote_refuses_a_second_vote_from_one_account() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            true
        ));

        // The next block clears the vote interval. The double vote stays refused.
        next_block();
        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), proposal_id, false),
            Error::<Test>::AlreadyVoted
        );
    });
}

#[test]
fn vote_refuses_root_origin() {
    new_test_ext().execute_with(|| {
        let proposal_id = open_reward_proposal();
        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::root(), proposal_id, true),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn vote_refuses_a_full_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        let limit = RateLimitConfigStorage::<Test>::get().max_votes_per_block;
        BlockOperationCounts::<Test>::insert(DispatchableType::VoteProposal, limit);

        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), proposal_id, true),
            Error::<Test>::PerBlockRateLimitExceeded
        );
    });
}

#[test]
fn vote_refuses_an_account_over_its_window_limit() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        // The pallet holds one hundred votes per account in one window.
        let history: BoundedVec<u32, ConstU32<100>> = BoundedVec::truncate_from(vec![1u32; 100]);
        AccountOperationHistory::<Test>::insert(1u64, DispatchableType::VoteProposal, history);

        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), proposal_id, true),
            Error::<Test>::PerAccountRateLimitExceeded
        );
    });
}

#[test]
fn vote_holds_the_minimum_interval() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let first = open_reward_proposal();
        let second = open_reward_proposal();

        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            first,
            true
        ));
        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), second, true),
            Error::<Test>::MinimumIntervalViolation
        );

        next_block();
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            second,
            true
        ));
        assert_eq!(ProposalVotes::<Test>::get(second, 1u64), Some(true));
    });
}

#[test]
fn vote_refuses_a_call_over_the_weight_bound() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        RateLimitConfigStorage::<Test>::put(RateLimitConfig {
            max_proposal_processing_weight: 1_000,
            ..RateLimitConfig::default()
        });

        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(1), proposal_id, true),
            Error::<Test>::WeightLimitExceeded
        );
    });
}

#[test]
fn vote_from_an_account_outside_the_active_set_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        assert!(!ActiveValidators::<Test>::get().contains(&10));

        // The quorum counts the active validators. A vote from outside that
        // set counts toward no quorum, so the pallet refuses it.
        assert_noop!(
            DcfPallet::vote_proposal(RuntimeOrigin::signed(10), proposal_id, true),
            Error::<Test>::NotValidator
        );
    });
}

// ---------------------------------------------------------------------------
// execute_proposal
// ---------------------------------------------------------------------------

#[test]
fn execute_refuses_a_signed_caller() {
    new_test_ext().execute_with(|| {
        let proposal_id = open_reward_proposal();
        assert_noop!(
            DcfPallet::execute_proposal(RuntimeOrigin::signed(1), proposal_id),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn execute_refuses_an_unknown_or_pending_proposal() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::execute_proposal(RuntimeOrigin::root(), 5),
            Error::<Test>::ProposalNotApproved
        );

        let proposal_id = open_reward_proposal();
        assert_noop!(
            DcfPallet::execute_proposal(RuntimeOrigin::root(), proposal_id),
            Error::<Test>::ProposalNotApproved
        );
    });
}

#[test]
fn execute_refuses_a_rejected_proposal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(1),
            proposal_id,
            false
        ));
        assert_ok!(DcfPallet::vote_proposal(
            RuntimeOrigin::signed(2),
            proposal_id,
            false
        ));

        assert_noop!(
            DcfPallet::execute_proposal(RuntimeOrigin::root(), proposal_id),
            Error::<Test>::ProposalNotApproved
        );
    });
}

#[test]
fn executed_proposal_refuses_a_second_execution() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let proposal_id = open_reward_proposal();
        approve_and_execute(proposal_id);

        assert_noop!(
            DcfPallet::execute_proposal(RuntimeOrigin::root(), proposal_id),
            Error::<Test>::ProposalNotApproved
        );
    });
}

#[test]
fn executed_slash_takes_the_amount_from_the_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let before = total_balance(2);
        assert_ok!(DcfPallet::propose_slash_validator(
            RuntimeOrigin::root(),
            1,
            2,
            500
        ));

        approve_and_execute(0);

        assert_eq!(total_balance(2), before - 500);
    });
}

#[test]
fn executed_slash_against_an_outside_account_is_recorded_as_failed() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::propose_slash_validator(
            RuntimeOrigin::root(),
            1,
            50,
            500
        ));

        // The stake pallet refuses the slash. The proposal records the failure.
        approve_and_attempt_failing_execution(0);
        assert_eq!(total_balance(50), 0);
    });
}

#[test]
fn executed_reward_credits_the_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let before = free_balance(3);
        assert_ok!(DcfPallet::propose_reward_validator(
            RuntimeOrigin::root(),
            1,
            3,
            2_000
        ));

        approve_and_execute(0);

        assert_eq!(free_balance(3), before + 2_000);
    });
}

#[test]
fn executed_multiple_reward_credits_every_named_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let reward: u128 = <Test as pallet_cerulea_pos::Config>::ValidatorReward::get();
        let before: Vec<u128> = [1u64, 2, 3].iter().map(|who| free_balance(*who)).collect();

        assert_ok!(DcfPallet::propose_default_reward_multiple_validators(
            RuntimeOrigin::root(),
            1,
            vec![1, 2, 3]
        ));
        approve_and_execute(0);

        for (index, who) in [1u64, 2, 3].iter().enumerate() {
            assert_eq!(free_balance(*who), before[index] + reward);
        }
    });
}

#[test]
fn executed_eject_releases_the_stake_and_announces_it() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(reserved_balance(3), 1_000);
        assert_ok!(DcfPallet::propose_eject_validator(
            RuntimeOrigin::root(),
            1,
            3,
            EjectionReason::ManualEjection
        ));

        approve_and_execute(0);

        assert_eq!(reserved_balance(3), 0);
        assert!(!pallet_cerulea_pos::Validators::<Test>::contains_key(3u64));
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(3u64), 0);
        assert!(dcf_events().contains(&Event::ValidatorEjected {
            validator: 3,
            reason: EjectionReason::ManualEjection,
        }));
    });
}

#[test]
fn executed_add_validator_proposal_seats_a_staked_account() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 2_000u128);
        assert!(!ValidatorSet::<Test>::get().contains(&10));

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            ProposalAction::AddValidator { validator: 10 },
            None
        ));
        approve_and_execute(0);

        // The set holds three of one hundred seats, so the staked account
        // takes a seat and opens a state record.
        assert!(
            ValidatorSet::<Test>::get().contains(&10),
            "An executed add proposal seats the staked account."
        );
        let state =
            ValidatorStates::<Test>::get(&10).expect("The new validator holds a state record.");
        assert_eq!(state.current.stake_score, 2_000);
        assert!(dcf_events().contains(&Event::ValidatorAdded { validator: 10 }));
    });
}

#[test]
fn executed_add_validator_for_a_seated_account_changes_nothing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let seated = ValidatorSet::<Test>::get().to_vec();

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            ProposalAction::AddValidator { validator: 2 },
            None
        ));
        approve_and_attempt_failing_execution(0);

        assert_eq!(ValidatorSet::<Test>::get().to_vec(), seated);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, Event::ValidatorAdded { .. })));
    });
}

#[test]
fn executed_add_validator_without_stake_seats_nobody() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(11u64), 0);

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            ProposalAction::AddValidator { validator: 11 },
            None
        ));
        approve_and_attempt_failing_execution(0);

        assert!(!ValidatorSet::<Test>::get().contains(&11));
        assert!(!ValidatorStates::<Test>::contains_key(&11));
    });
}

#[test]
fn executed_remove_validator_releases_the_stake_and_announces_it() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(reserved_balance(3), 1_000);

        assert_ok!(DcfPallet::submit_proposal(
            RuntimeOrigin::signed(1),
            ProposalAction::RemoveValidator { validator: 3 },
            None
        ));
        approve_and_execute(0);

        assert_eq!(reserved_balance(3), 0);
        assert!(!pallet_cerulea_pos::Validators::<Test>::contains_key(3u64));
        assert!(dcf_events().contains(&Event::ValidatorRemoved { validator: 3 }));
    });
}

// ---------------------------------------------------------------------------
// Root proposal calls
// ---------------------------------------------------------------------------

#[test]
fn root_proposal_calls_refuse_a_signed_caller() {
    new_test_ext().execute_with(|| {
        let signed = || RuntimeOrigin::signed(1);
        assert_noop!(
            DcfPallet::propose_slash_validator(signed(), 1, 2, 500),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_reward_validator(signed(), 1, 2, 500),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_default_reward_validator(signed(), 1, 2),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(signed(), 1, vec![1, 2], 500),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_default_reward_multiple_validators(signed(), 1, vec![1, 2]),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_reward_all_active_validators(signed(), 1),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::propose_eject_validator(signed(), 1, 2, EjectionReason::ManualEjection),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn slash_proposal_is_stored_and_announced() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::propose_slash_validator(
            RuntimeOrigin::root(),
            1,
            2,
            500
        ));

        let action = ProposalAction::Slash {
            validator: 2,
            amount: 500,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert_eq!(NextProposalId::<Test>::get(), 1);
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn reward_proposal_is_stored_and_announced() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::propose_reward_validator(
            RuntimeOrigin::root(),
            1,
            3,
            750
        ));

        let action = ProposalAction::Reward {
            validator: 3,
            amount: 750,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn default_reward_proposal_uses_the_stake_pallet_reward() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let reward: u128 = <Test as pallet_cerulea_pos::Config>::ValidatorReward::get();

        assert_ok!(DcfPallet::propose_default_reward_validator(
            RuntimeOrigin::root(),
            1,
            2
        ));

        let action = ProposalAction::Reward {
            validator: 2,
            amount: reward,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn multiple_reward_proposal_is_stored_with_the_named_validators() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::propose_reward_multiple_validators(
            RuntimeOrigin::root(),
            1,
            vec![1, 2],
            700
        ));

        let action = ProposalAction::RewardMultiple {
            validators: BoundedVec::truncate_from(vec![1u64, 2u64]),
            amount: 700,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn multiple_reward_proposal_refuses_an_empty_or_oversized_list() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(RuntimeOrigin::root(), 1, vec![], 700),
            Error::<Test>::NotEnoughValidators
        );
        let oversized: Vec<u64> = (1_000u64..1_101).collect();
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(RuntimeOrigin::root(), 1, oversized, 700),
            Error::<Test>::NotEnoughValidators
        );
    });
}

#[test]
fn multiple_reward_proposal_refuses_a_list_over_the_weight_bound() {
    new_test_ext().execute_with(|| {
        // Each named validator costs ten thousand units of weight.
        RateLimitConfigStorage::<Test>::put(RateLimitConfig {
            max_proposal_processing_weight: 15_000,
            ..RateLimitConfig::default()
        });
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(
                RuntimeOrigin::root(),
                1,
                vec![1, 2],
                700
            ),
            Error::<Test>::WeightLimitExceeded
        );
    });
}

#[test]
fn multiple_reward_proposal_refuses_a_list_over_the_loop_bound() {
    new_test_ext().execute_with(|| {
        RateLimitConfigStorage::<Test>::put(RateLimitConfig {
            max_loop_iterations: 1,
            ..RateLimitConfig::default()
        });
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(
                RuntimeOrigin::root(),
                1,
                vec![1, 2],
                700
            ),
            Error::<Test>::WeightLimitExceeded
        );
    });
}

#[test]
fn multiple_reward_proposal_refuses_an_unknown_validator() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::propose_reward_multiple_validators(
                RuntimeOrigin::root(),
                1,
                vec![1, 99],
                700
            ),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn default_multiple_reward_proposal_uses_the_stake_pallet_reward() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let reward: u128 = <Test as pallet_cerulea_pos::Config>::ValidatorReward::get();

        assert_ok!(DcfPallet::propose_default_reward_multiple_validators(
            RuntimeOrigin::root(),
            1,
            vec![2, 3]
        ));

        let action = ProposalAction::RewardMultiple {
            validators: BoundedVec::truncate_from(vec![2u64, 3u64]),
            amount: reward,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn default_multiple_reward_proposal_refuses_bad_lists() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::propose_default_reward_multiple_validators(RuntimeOrigin::root(), 1, vec![]),
            Error::<Test>::NotEnoughValidators
        );
        let oversized: Vec<u64> = (1_000u64..1_101).collect();
        assert_noop!(
            DcfPallet::propose_default_reward_multiple_validators(
                RuntimeOrigin::root(),
                1,
                oversized
            ),
            Error::<Test>::NotEnoughValidators
        );
        assert_noop!(
            DcfPallet::propose_default_reward_multiple_validators(
                RuntimeOrigin::root(),
                1,
                vec![1, 99]
            ),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn reward_all_active_proposal_names_every_active_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let reward: u128 = <Test as pallet_cerulea_pos::Config>::ValidatorReward::get();
        let active = ActiveValidators::<Test>::get().to_vec();
        assert_eq!(active.len(), 3);

        assert_ok!(DcfPallet::propose_reward_all_active_validators(
            RuntimeOrigin::root(),
            1
        ));

        let action = ProposalAction::RewardMultiple {
            validators: BoundedVec::truncate_from(active),
            amount: reward,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn reward_all_active_proposal_refuses_an_empty_active_set() {
    new_test_ext().execute_with(|| {
        ActiveValidators::<Test>::put(Default::default());
        assert!(ActiveValidators::<Test>::get().is_empty());

        assert_noop!(
            DcfPallet::propose_reward_all_active_validators(RuntimeOrigin::root(), 1),
            Error::<Test>::NotEnoughValidators
        );
    });
}

#[test]
fn eject_proposal_is_stored_and_announced() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::propose_eject_validator(
            RuntimeOrigin::root(),
            1,
            3,
            EjectionReason::ScoreBelowThreshold
        ));

        let action = ProposalAction::Eject {
            validator: 3,
            reason: EjectionReason::ScoreBelowThreshold,
        };
        assert_eq!(
            Proposals::<Test>::get(0u32),
            Some(pending(1, action.clone()))
        );
        assert!(dcf_events().contains(&Event::ProposalSubmitted {
            proposal_id: 0,
            proposer: 1,
            action,
        }));
    });
}

#[test]
fn proposal_ids_count_up_across_proposal_kinds() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::propose_slash_validator(
            RuntimeOrigin::root(),
            1,
            2,
            500
        ));
        assert_ok!(DcfPallet::propose_default_reward_validator(
            RuntimeOrigin::root(),
            1,
            2
        ));
        assert_ok!(DcfPallet::propose_eject_validator(
            RuntimeOrigin::root(),
            1,
            3,
            EjectionReason::ManualEjection
        ));
        assert_ok!(DcfPallet::propose_reward_all_active_validators(
            RuntimeOrigin::root(),
            1
        ));

        assert_eq!(NextProposalId::<Test>::get(), 4);
        for proposal_id in 0..4u32 {
            assert_eq!(status_of(proposal_id), Some(ProposalStatus::Pending));
        }
    });
}

// ---------------------------------------------------------------------------
// validate_genesis_configuration and dry_run_genesis_configuration
// ---------------------------------------------------------------------------

#[test]
fn genesis_check_announces_count_and_total_stake() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let seated = ValidatorSet::<Test>::get().to_vec();

        assert_ok!(DcfPallet::validate_genesis_configuration(
            RuntimeOrigin::root(),
            vec![10, 11, 12],
            vec![1_000, 2_000, 3_000],
            epoch(100)
        ));

        assert!(dcf_events().contains(&Event::GenesisValidationSucceeded {
            validator_count: 3,
            total_stake: 6_000,
        }));
        // The check reads the record and seats nobody.
        assert_eq!(ValidatorSet::<Test>::get().to_vec(), seated);
    });
}

#[test]
fn genesis_checks_refuse_a_signed_caller() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::validate_genesis_configuration(
                RuntimeOrigin::signed(1),
                vec![10, 11, 12],
                vec![],
                epoch(100)
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            DcfPallet::dry_run_genesis_configuration(
                RuntimeOrigin::signed(1),
                vec![10, 11, 12],
                vec![],
                epoch(100)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn genesis_checks_refuse_empty_duplicate_and_short_sets() {
    new_test_ext().execute_with(|| {
        assert_genesis_refused(vec![], vec![], epoch(100));
        assert_genesis_refused(vec![10, 10, 11], vec![], epoch(100));
        // The mock runtime asks for three active validators.
        assert_genesis_refused(vec![10, 11], vec![], epoch(100));
    });
}

#[test]
fn genesis_checks_refuse_an_oversized_set() {
    new_test_ext().execute_with(|| {
        let oversized: Vec<u64> = (1_000u64..1_101).collect();
        assert_genesis_refused(oversized, vec![], epoch(100));
    });
}

#[test]
fn genesis_checks_refuse_stake_faults() {
    new_test_ext().execute_with(|| {
        // Two stakes for three validators.
        assert_genesis_refused(vec![10, 11, 12], vec![1_000, 1_000], epoch(100));
        // One stake under the minimum stake.
        assert_genesis_refused(vec![10, 11, 12], vec![1_000, 999, 1_000], epoch(100));
    });
}

#[test]
fn genesis_checks_refuse_epoch_faults() {
    new_test_ext().execute_with(|| {
        assert_genesis_refused(vec![10, 11, 12], vec![], epoch(0));
        assert_genesis_refused(vec![10, 11, 12], vec![], epoch(100_001));
        assert_genesis_refused(
            vec![10, 11, 12],
            vec![],
            EpochConfig {
                blocks_per_epoch: 100,
                min_stake: 2_000,
                max_validators: 100,
            },
        );
        assert_genesis_refused(
            vec![10, 11, 12],
            vec![],
            EpochConfig {
                blocks_per_epoch: 100,
                min_stake: 1_000,
                max_validators: 50,
            },
        );
    });
}

#[test]
fn dry_run_genesis_reports_checks_and_no_warnings() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let epoch_before = CurrentEpoch::<Test>::get();
        let next_id_before = NextProposalId::<Test>::get();

        assert_ok!(DcfPallet::dry_run_genesis_configuration(
            RuntimeOrigin::root(),
            vec![10, 11, 12],
            vec![1_000, 2_000, 3_000],
            epoch(100)
        ));

        // Five invariant checks pass. The record raises no warning.
        assert!(dcf_events().contains(&Event::GenesisDryRunCompleted {
            validator_count: 3,
            total_stake: 6_000,
            invariant_checks_passed: 5,
            warnings_count: 0,
        }));
        assert_eq!(CurrentEpoch::<Test>::get(), epoch_before);
        assert_eq!(NextProposalId::<Test>::get(), next_id_before);
        assert!(!ValidatorSet::<Test>::get().contains(&10));
    });
}

#[test]
fn dry_run_genesis_counts_inequality_and_short_epoch_warnings() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // The largest stake passes ten times the smallest. The epoch runs
        // under one hundred blocks.
        assert_ok!(DcfPallet::dry_run_genesis_configuration(
            RuntimeOrigin::root(),
            vec![10, 11, 12],
            vec![1_000, 1_000, 20_000],
            epoch(50)
        ));

        assert!(dcf_events().contains(&Event::GenesisDryRunCompleted {
            validator_count: 3,
            total_stake: 22_000,
            invariant_checks_passed: 5,
            warnings_count: 2,
        }));
    });
}

#[test]
fn dry_run_genesis_warns_on_a_long_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::dry_run_genesis_configuration(
            RuntimeOrigin::root(),
            vec![10, 11, 12],
            vec![1_000, 1_000, 1_000],
            epoch(12_000)
        ));

        assert!(dcf_events().contains(&Event::GenesisDryRunCompleted {
            validator_count: 3,
            total_stake: 3_000,
            invariant_checks_passed: 5,
            warnings_count: 1,
        }));
    });
}

#[test]
fn dry_run_genesis_fills_empty_stakes_with_the_minimum() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let min_stake: u128 = <Test as pallet_cerulea_pos::Config>::MinStake::get();

        assert_ok!(DcfPallet::dry_run_genesis_configuration(
            RuntimeOrigin::root(),
            vec![10, 11, 12],
            vec![],
            epoch(100)
        ));

        assert!(dcf_events().contains(&Event::GenesisDryRunCompleted {
            validator_count: 3,
            total_stake: min_stake * 3,
            invariant_checks_passed: 5,
            warnings_count: 0,
        }));
    });
}
