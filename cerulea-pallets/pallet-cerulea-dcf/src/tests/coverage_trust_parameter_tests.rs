//! Line coverage for the Trust Score helpers, the parameter path, the block
//! hook helpers and the trait implementations at the end of the framework
//! pallet.
//!
//! Each test reaches one branch through a public call, the block
//! initialisation hook or a trait implementation. Each test then reads the
//! state the branch writes or the event it deposits.
//!
//! A private helper stays invisible from this module. The tests therefore
//! reach every private helper through the public call or the hook that runs
//! it. The block initialisation hook runs the regular path at block 0, 10, 11
//! and 50 and the epoch path at block 100.

use crate::mock::*;
use crate::ParameterType as Param;
use codec::Encode;
use frame_support::{
    assert_noop, assert_ok,
    traits::{ConstU32, OnInitialize},
    weights::Weight,
    BoundedVec,
};
use sp_runtime::{DigestItem, DispatchError, DispatchResult};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// The encoded value the parameter call carries.
type ParamValue = BoundedVec<u8, ConstU32<64>>;

/// Encode one value the way the parameter call reads it.
fn value<E: Encode>(raw: E) -> ParamValue {
    BoundedVec::truncate_from(raw.encode())
}

/// Carry raw bytes into the parameter call.
fn raw_value(bytes: Vec<u8>) -> ParamValue {
    BoundedVec::truncate_from(bytes)
}

/// Submit one parameter change through the root origin.
fn update(parameter: Param, new_value: ParamValue) -> DispatchResult {
    DcfPallet::update_dcf_parameter(RuntimeOrigin::root(), parameter, new_value)
}

/// Submit one change and hold the event the Amendment Engine records.
fn assert_updated(parameter: Param, old_value: ParamValue, new_value: ParamValue) {
    assert_ok!(update(parameter.clone(), new_value.clone()));
    System::assert_last_event(RuntimeEvent::DcfPallet(crate::Event::DcfParameterUpdated {
        parameter,
        old_value,
        new_value,
    }));
}

/// Submit one change the Amendment Engine refuses and hold storage unchanged.
fn assert_refused(parameter: Param, new_value: ParamValue, error: crate::Error<Test>) {
    assert_noop!(update(parameter, new_value), error);
}

/// One epoch record with a stake score of one and no inference result.
fn stats(epoch: u32, final_score: u64) -> crate::EpochStats {
    crate::EpochStats {
        epoch,
        stake_score: 1,
        inference_score: 0,
        final_score,
        authored_blocks: 0,
        missed_blocks: 0,
    }
}

/// A validator record with one epoch of service and one history entry.
fn state_with(final_score: u64, last_active_epoch: u32) -> crate::ValidatorState<Test> {
    crate::ValidatorState {
        last_active_epoch,
        current: stats(last_active_epoch, final_score),
        history: BoundedVec::truncate_from(vec![stats(last_active_epoch, final_score)]),
        uptime: 1,
        inference_success_count: 0,
        participation_rate: 100,
        inference_count: 0,
        last_active_block: 1,
        name: None,
        trust_score: 5_000,
    }
}

/// Write one validator record into the registry.
///
/// The record starts with no stored Trust Score. The Standing Engine
/// therefore reads the next calculation as the first score, whatever score
/// genesis stored for the account.
fn seed(who: u64, state: crate::ValidatorState<Test>) {
    crate::ValidatorStates::<Test>::insert(&who, state);
    crate::ValidatorTrustScores::<Test>::remove(who);
}

/// Read one validator record from the registry.
fn state_of(who: u64) -> crate::ValidatorState<Test> {
    crate::ValidatorStates::<Test>::get(&who)
        .expect("The registry holds a record for this validator.")
}

/// Change one validator record in place.
fn edit(who: u64, change: impl FnOnce(&mut crate::ValidatorState<Test>)) {
    let mut held = state_of(who);
    change(&mut held);
    crate::ValidatorStates::<Test>::insert(&who, held);
}

/// Replace the active set the Admission Engine holds.
fn set_active(list: Vec<u64>) {
    crate::ActiveValidators::<Test>::put(BoundedVec::truncate_from(list));
}

/// Replace the registered set the Admission Engine holds.
fn set_registered(list: Vec<u64>) {
    crate::ValidatorSet::<Test>::put(BoundedVec::truncate_from(list));
}

/// The active set the Admission Engine holds.
fn active() -> Vec<u64> {
    crate::ActiveValidators::<Test>::get().into_inner()
}

/// The registered set the Admission Engine holds.
fn registered() -> Vec<u64> {
    crate::ValidatorSet::<Test>::get().into_inner()
}

/// Every framework pallet event in the record, in order.
fn dcf_events() -> Vec<crate::Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// Report whether the record holds one framework pallet event.
fn has_event(event: crate::Event<Test>) -> bool {
    dcf_events().contains(&event)
}

/// Run the block initialisation hook once and return its weight.
fn hook_weight(block: u64) -> Weight {
    <DcfPallet as OnInitialize<u64>>::on_initialize(block)
}

/// Run the block initialisation hook once.
fn run_hook(block: u64) {
    let _ = hook_weight(block);
}

/// The reason text an automatic proposal carries.
fn reason(text: &[u8]) -> BoundedVec<u8, ConstU32<64>> {
    BoundedVec::truncate_from(text.to_vec())
}

/// Trust Score bounds with the last update at epoch zero.
fn bounds(
    min_score: u64,
    max_score: u64,
    max_growth_rate: u32,
    max_decay_rate: u32,
    stability_factor: u32,
) -> crate::TrustScoreBoundsData {
    crate::TrustScoreBoundsData {
        min_score,
        max_score,
        max_growth_rate,
        max_decay_rate,
        stability_factor,
        last_updated_epoch: 0,
    }
}

/// A Trust Score history of (epoch, score) entries.
fn history(entries: Vec<(u32, u64)>) -> BoundedVec<(u32, u64), ConstU32<100>> {
    BoundedVec::truncate_from(entries)
}

/// The encoded author record the Authoring Engine places in a digest item.
fn author_bytes(author: u64, len: usize) -> Vec<u8> {
    let mut bytes = author.encode();
    bytes.resize(len, 0);
    bytes
}

/// Name one author for every slot of one epoch.
///
/// The genesis profile runs ten blocks per epoch, so block 11 reads the
/// second slot of epoch one.
fn fix_author_for_epoch(epoch: u32, author: u64) {
    let sequence: BoundedVec<u64, ConstU32<1000>> = BoundedVec::truncate_from(vec![author; 10]);
    crate::EpochAuthorSequences::<Test>::insert(epoch, sequence);
}

/// Prepare block 11 with validator 2 as the expected author.
fn authorship_block() {
    System::set_block_number(11);
    seed(2, state_with(60, 0));
    seed(3, state_with(60, 0));
    fix_author_for_epoch(1, 2);
}

/// One governance proposal from validator 1.
fn proposal(
    action: crate::ProposalAction<Test>,
    status: crate::ProposalStatus,
) -> crate::GovernanceProposal<Test> {
    crate::GovernanceProposal {
        proposer: 1,
        action,
        status,
        votes_for: 0,
        votes_against: 0,
    }
}

/// The status one stored proposal carries.
fn status_of(id: u32) -> Option<crate::ProposalStatus> {
    crate::Proposals::<Test>::get(id).map(|held| held.status)
}

fn handler_joined(who: u64, stake: u128) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_joined(&who, stake)
}

fn handler_leave_requested(who: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_leave_requested(&who)
}

fn handler_left(who: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_left(&who)
}

fn handler_stake_increased(who: u64, amount: u128) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_stake_increased(&who, amount)
}

fn handler_stake_decreased(who: u64, amount: u128) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_stake_decreased(&who, amount)
}

fn handler_slashed(who: u64, amount: u128, penalty: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_slashed(
        &who, amount, penalty,
    )
}

fn handler_rewarded(who: u64, amount: u128, boost: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::on_rewarded(&who, amount, boost)
}

fn handler_score(who: u64) -> u64 {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::get_validator_score(&who)
}

fn handler_active() -> Vec<u64> {
    <DcfPallet as pallet_cerulea_pos::ValidatorHandler<u64, u128>>::get_active_validators()
}

fn interface_record(who: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_poi::DcfInterface<u64>>::record_inference_activity(&who)
}

fn interface_eject(who: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_poi::DcfInterface<u64>>::eject_validator(
        &who,
        "score below the floor",
    )
}

fn interface_final_score(who: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_poi::DcfInterface<u64>>::update_final_score(&who)
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: epoch and validator set parameters
// ---------------------------------------------------------------------------

#[test]
fn epoch_length_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::EpochLength, value(10u32), value(200u32));
        assert_eq!(DcfPallet::governance_config().epoch_length.current, 200);
        // The running epoch keeps its length until the epoch path reads the record.
        assert_eq!(
            crate::EpochConfigStorage::<Test>::get().blocks_per_epoch,
            10
        );
    });
}

#[test]
fn epoch_length_outside_its_range_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::EpochLength,
            value(49u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::EpochLength,
            value(14_401u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn minimum_stake_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::MinStake, value(1_000u128), value(200_000u128));
        assert_eq!(DcfPallet::governance_config().min_stake.current, 200_000);
        assert_refused(
            Param::MinStake,
            value(99_999u128),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MinStake,
            value(1_000_000_001u128),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn maximum_validators_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::MaxValidators, value(100u32), value(50u32));
        assert_eq!(DcfPallet::governance_config().max_validators.current, 50);
        assert_refused(
            Param::MaxValidators,
            value(0u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MaxValidators,
            value(1_001u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: score composition shares
// ---------------------------------------------------------------------------

#[test]
fn stake_share_change_reaches_the_running_share() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // The stake share and the inference share reach the whole together.
        crate::GovernanceConfigStorage::<Test>::mutate(|record| record.poi_weight.current = 3_000);
        assert_updated(Param::PosWeight, value(6_000u64), value(7_000u64));
        assert_eq!(DcfPallet::governance_config().pos_weight.current, 7_000);
        assert_eq!(DcfPallet::pos_weight(), 7_000);
    });
}

#[test]
fn stake_share_outside_its_range_or_its_pair_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::PosWeight,
            value(500u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::PosWeight,
            value(9_500u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        // 5000 sits inside the range. With the inference share at 4000 the
        // pair falls short of the whole.
        assert_refused(
            Param::PosWeight,
            value(5_000u64),
            crate::Error::<Test>::InvalidWeight,
        );
    });
}

#[test]
fn inference_share_change_reaches_the_running_share() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceConfigStorage::<Test>::mutate(|record| record.pos_weight.current = 7_000);
        assert_updated(Param::PoiWeight, value(4_000u64), value(3_000u64));
        assert_eq!(DcfPallet::governance_config().poi_weight.current, 3_000);
        assert_eq!(DcfPallet::poi_weight(), 3_000);
    });
}

#[test]
fn inference_share_outside_its_range_or_its_pair_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::PoiWeight,
            value(999u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::PoiWeight,
            value(9_500u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::PoiWeight,
            value(5_000u64),
            crate::Error::<Test>::InvalidWeight,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: performance thresholds
// ---------------------------------------------------------------------------

#[test]
fn minimum_performance_score_change_lands_under_the_high_threshold() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::MinPerformanceScore, value(30u64), value(40u64));
        assert_eq!(
            DcfPallet::governance_config().min_performance_score.current,
            40
        );
    });
}

#[test]
fn minimum_performance_score_outside_its_range_or_above_the_high_threshold_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::MinPerformanceScore,
            value(5u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MinPerformanceScore,
            value(5_001u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        // 90 sits inside the range and above the high threshold of 80.
        assert_refused(
            Param::MinPerformanceScore,
            value(90u64),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

/// The genesis record bounds the high threshold between one point above the
/// starting minimum score and the score ceiling of 100. A threshold of 90
/// sits inside that range and above the floor of 30, so the Amendment Engine
/// accepts it.
#[test]
fn high_performance_score_accepts_a_value_under_the_score_ceiling() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::HighPerformanceScore, value(80u64), value(90u64));
        assert_eq!(
            DcfPallet::governance_config()
                .high_performance_score
                .current,
            90
        );
    });
}

#[test]
fn high_performance_score_change_lands_inside_a_widened_range() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceConfigStorage::<Test>::mutate(|record| {
            record.high_performance_score.min = 50;
            record.high_performance_score.max = 100;
        });
        assert_updated(Param::HighPerformanceScore, value(80u64), value(95u64));
        assert_eq!(
            DcfPallet::governance_config()
                .high_performance_score
                .current,
            95
        );
    });
}

#[test]
fn high_performance_score_above_the_ceiling_or_under_the_floor_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::HighPerformanceScore,
            value(101u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        crate::GovernanceConfigStorage::<Test>::mutate(|record| {
            record.high_performance_score.min = 10;
        });
        // 20 sits inside the widened range and under the floor threshold of 30.
        assert_refused(
            Param::HighPerformanceScore,
            value(20u64),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

#[test]
fn minimum_participation_rate_change_lands_under_the_high_rate() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::MinParticipationRate, value(50u32), value(60u32));
        assert_eq!(
            DcfPallet::governance_config()
                .min_participation_rate
                .current,
            60
        );
    });
}

#[test]
fn minimum_participation_rate_outside_its_range_or_at_the_high_rate_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::MinParticipationRate,
            value(5u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MinParticipationRate,
            value(95u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MinParticipationRate,
            value(90u32),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

#[test]
fn high_participation_rate_change_lands_above_the_minimum_rate() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::HighParticipationRate, value(90u32), value(95u32));
        assert_eq!(
            DcfPallet::governance_config()
                .high_participation_rate
                .current,
            95
        );
    });
}

#[test]
fn high_participation_rate_outside_its_range_or_at_the_minimum_rate_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::HighParticipationRate,
            value(40u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::HighParticipationRate,
            value(101u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::HighParticipationRate,
            value(50u32),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: economic parameters
// ---------------------------------------------------------------------------

#[test]
fn validator_reward_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::ValidatorReward, value(10_000u128), value(20_000u128));
        assert_eq!(
            DcfPallet::governance_config().validator_reward.current,
            20_000
        );
        assert_refused(
            Param::ValidatorReward,
            value(999u128),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::ValidatorReward,
            value(10_000_001u128),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn slash_percent_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::SlashPercent, value(10u32), value(25u32));
        assert_eq!(DcfPallet::governance_config().slash_percent.current, 25);
        assert_refused(
            Param::SlashPercent,
            value(0u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::SlashPercent,
            value(51u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn slash_percent_above_the_whole_stake_is_refused_inside_a_widened_range() {
    new_test_ext().execute_with(|| {
        crate::GovernanceConfigStorage::<Test>::mutate(|record| record.slash_percent.max = 200);
        // 150 sits inside the widened range and above one hundred percent.
        assert_refused(
            Param::SlashPercent,
            value(150u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn leave_cooldown_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::LeaveCooldown, value(1_000u64), value(5_000u64));
        assert_eq!(DcfPallet::governance_config().leave_cooldown.current, 5_000);
        assert_refused(
            Param::LeaveCooldown,
            value(99u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::LeaveCooldown,
            value(100_001u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: Standing Engine weights and bounds
// ---------------------------------------------------------------------------

#[test]
fn uptime_weight_change_reaches_the_trust_score_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::TrustScoreUptimeWeight, value(4_000u32), value(50u32));
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_uptime_weight
                .current,
            50
        );
        assert_eq!(DcfPallet::trust_score_config().uptime_weight, 50);
        assert_refused(
            Param::TrustScoreUptimeWeight,
            value(9u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreUptimeWeight,
            value(81u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn inference_weight_change_reaches_the_trust_score_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(
            Param::TrustScoreInferenceWeight,
            value(4_000u32),
            value(30u32),
        );
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_inference_weight
                .current,
            30
        );
        assert_eq!(DcfPallet::trust_score_config().inference_weight, 30);
        assert_refused(
            Param::TrustScoreInferenceWeight,
            value(5u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreInferenceWeight,
            value(81u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn slashing_weight_change_reaches_the_trust_score_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(
            Param::TrustScoreSlashingWeight,
            value(2_000u32),
            value(20u32),
        );
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_slashing_weight
                .current,
            20
        );
        assert_eq!(DcfPallet::trust_score_config().slashing_weight, 20);
        assert_refused(
            Param::TrustScoreSlashingWeight,
            value(4u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreSlashingWeight,
            value(51u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn growth_rate_change_reaches_the_record_and_the_bounds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(3u32);
        assert_updated(Param::TrustScoreMaxGrowthRate, value(500u32), value(800u32));
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_max_growth_rate
                .current,
            800
        );
        assert_eq!(DcfPallet::trust_score_config().max_growth_rate, 800);
        let held = DcfPallet::trust_score_bounds();
        assert_eq!(held.max_growth_rate, 800);
        assert_eq!(held.last_updated_epoch, 3);
        assert_refused(
            Param::TrustScoreMaxGrowthRate,
            value(99u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreMaxGrowthRate,
            value(2_001u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn decay_rate_change_reaches_the_record_and_the_bounds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(3u32);
        assert_updated(Param::TrustScoreMaxDecayRate, value(200u32), value(300u32));
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_max_decay_rate
                .current,
            300
        );
        assert_eq!(DcfPallet::trust_score_config().max_decay_rate, 300);
        let held = DcfPallet::trust_score_bounds();
        assert_eq!(held.max_decay_rate, 300);
        assert_eq!(held.last_updated_epoch, 3);
        assert_refused(
            Param::TrustScoreMaxDecayRate,
            value(49u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreMaxDecayRate,
            value(1_001u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn trust_score_floor_change_reaches_the_record_and_the_bounds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(3u32);
        assert_updated(Param::TrustScoreMinValue, value(1_000u64), value(2_000u64));
        assert_eq!(
            DcfPallet::governance_config().trust_score_min_value.current,
            2_000
        );
        assert_eq!(DcfPallet::trust_score_config().min_trust_score, 2_000);
        let held = DcfPallet::trust_score_bounds();
        assert_eq!(held.min_score, 2_000);
        assert_eq!(held.last_updated_epoch, 3);
    });
}

#[test]
fn trust_score_floor_outside_its_range_or_above_the_ceiling_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::TrustScoreMinValue,
            value(499u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreMinValue,
            value(3_001u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        crate::GovernanceConfigStorage::<Test>::mutate(|record| {
            record.trust_score_max_value.current = 2_500;
        });
        // 3000 sits inside the floor range and above the running ceiling.
        assert_refused(
            Param::TrustScoreMinValue,
            value(3_000u64),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

#[test]
fn trust_score_ceiling_change_reaches_the_record_and_the_bounds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(3u32);
        assert_updated(
            Param::TrustScoreMaxValue,
            value(10_000u64),
            value(12_000u64),
        );
        assert_eq!(
            DcfPallet::governance_config().trust_score_max_value.current,
            12_000
        );
        assert_eq!(DcfPallet::trust_score_config().max_trust_score, 12_000);
        let held = DcfPallet::trust_score_bounds();
        assert_eq!(held.max_score, 12_000);
        assert_eq!(held.last_updated_epoch, 3);
    });
}

#[test]
fn trust_score_ceiling_outside_its_range_or_under_the_floor_is_refused() {
    new_test_ext().execute_with(|| {
        assert_refused(
            Param::TrustScoreMaxValue,
            value(7_999u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreMaxValue,
            value(15_001u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        crate::GovernanceConfigStorage::<Test>::mutate(|record| {
            record.trust_score_min_value.current = 9_000;
        });
        // 8500 sits inside the ceiling range and under the running floor.
        assert_refused(
            Param::TrustScoreMaxValue,
            value(8_500u64),
            crate::Error::<Test>::InvalidGovernanceConfig,
        );
    });
}

#[test]
fn stability_factor_change_reaches_the_record_and_the_bounds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(3u32);
        assert_updated(
            Param::TrustScoreStabilityFactor,
            value(8_000u32),
            value(9_000u32),
        );
        assert_eq!(
            DcfPallet::governance_config()
                .trust_score_stability_factor
                .current,
            9_000
        );
        assert_eq!(DcfPallet::trust_score_config().stability_factor, 9_000);
        let held = DcfPallet::trust_score_bounds();
        assert_eq!(held.stability_factor, 9_000);
        assert_eq!(held.last_updated_epoch, 3);
        assert_refused(
            Param::TrustScoreStabilityFactor,
            value(4_999u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::TrustScoreStabilityFactor,
            value(9_501u32),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: block and inference scoring
// ---------------------------------------------------------------------------

#[test]
fn block_authorship_boost_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::BlockAuthorshipBoost, value(10u64), value(20u64));
        assert_eq!(
            DcfPallet::governance_config()
                .block_authorship_boost
                .current,
            20
        );
        assert_refused(
            Param::BlockAuthorshipBoost,
            value(0u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::BlockAuthorshipBoost,
            value(101u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn missed_block_penalty_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::MissedBlockPenalty, value(5u64), value(7u64));
        assert_eq!(
            DcfPallet::governance_config().missed_block_penalty.current,
            7
        );
        assert_refused(
            Param::MissedBlockPenalty,
            value(0u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::MissedBlockPenalty,
            value(51u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn low_inference_boost_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::InferenceBoostLow, value(2u64), value(3u64));
        assert_eq!(
            DcfPallet::governance_config().inference_boost_low.current,
            3
        );
        assert_refused(
            Param::InferenceBoostLow,
            value(0u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::InferenceBoostLow,
            value(21u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn medium_inference_boost_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::InferenceBoostMedium, value(5u64), value(8u64));
        assert_eq!(
            DcfPallet::governance_config()
                .inference_boost_medium
                .current,
            8
        );
        assert_refused(
            Param::InferenceBoostMedium,
            value(1u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::InferenceBoostMedium,
            value(51u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

#[test]
fn high_inference_boost_change_lands_in_the_governance_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_updated(Param::InferenceBoostHigh, value(10u64), value(15u64));
        assert_eq!(
            DcfPallet::governance_config().inference_boost_high.current,
            15
        );
        assert_refused(
            Param::InferenceBoostHigh,
            value(4u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
        assert_refused(
            Param::InferenceBoostHigh,
            value(101u64),
            crate::Error::<Test>::ParameterOutOfRange,
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine parameter path: malformed values and origin
// ---------------------------------------------------------------------------

#[test]
fn every_parameter_refuses_empty_value_bytes() {
    new_test_ext().execute_with(|| {
        let every_parameter = [
            Param::EpochLength,
            Param::MinStake,
            Param::MaxValidators,
            Param::PosWeight,
            Param::PoiWeight,
            Param::MinPerformanceScore,
            Param::HighPerformanceScore,
            Param::MinParticipationRate,
            Param::HighParticipationRate,
            Param::ValidatorReward,
            Param::SlashPercent,
            Param::LeaveCooldown,
            Param::TrustScoreUptimeWeight,
            Param::TrustScoreInferenceWeight,
            Param::TrustScoreSlashingWeight,
            Param::TrustScoreMaxGrowthRate,
            Param::TrustScoreMaxDecayRate,
            Param::TrustScoreMinValue,
            Param::TrustScoreMaxValue,
            Param::TrustScoreStabilityFactor,
            Param::BlockAuthorshipBoost,
            Param::MissedBlockPenalty,
            Param::InferenceBoostLow,
            Param::InferenceBoostMedium,
            Param::InferenceBoostHigh,
        ];
        assert_eq!(every_parameter.len(), 25);
        for parameter in every_parameter.iter().cloned() {
            assert_refused(
                parameter,
                raw_value(Vec::new()),
                crate::Error::<Test>::InvalidParameterType,
            );
        }
    });
}

#[test]
fn short_value_bytes_are_refused_for_each_value_width() {
    new_test_ext().execute_with(|| {
        // A 32 bit value needs four bytes.
        assert_refused(
            Param::EpochLength,
            raw_value(vec![1, 2]),
            crate::Error::<Test>::InvalidParameterType,
        );
        // A block number needs eight bytes.
        assert_refused(
            Param::LeaveCooldown,
            value(5_000u32),
            crate::Error::<Test>::InvalidParameterType,
        );
        // A balance needs sixteen bytes.
        assert_refused(
            Param::MinStake,
            raw_value(vec![0u8; 8]),
            crate::Error::<Test>::InvalidParameterType,
        );
        assert_refused(
            Param::ValidatorReward,
            raw_value(vec![0u8; 8]),
            crate::Error::<Test>::InvalidParameterType,
        );
    });
}

#[test]
fn signed_origin_is_refused_a_parameter_change() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_dcf_parameter(
                RuntimeOrigin::signed(1),
                Param::EpochLength,
                value(200u32),
            ),
            DispatchError::BadOrigin
        );
    });
}

// ---------------------------------------------------------------------------
// Standing Engine: bounded growth, decay and smoothing
// ---------------------------------------------------------------------------

#[test]
fn first_trust_score_clamps_into_the_bounds() {
    new_test_ext().execute_with(|| {
        let held = bounds(1_000, 10_000, 500, 200, 8_000);
        assert_eq!(DcfPallet::apply_trust_score_bounds(500, 0, &held), 1_000);
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(20_000, 0, &held),
            10_000
        );
        assert_eq!(DcfPallet::apply_trust_score_bounds(4_321, 0, &held), 4_321);
    });
}

#[test]
fn rising_trust_score_moves_by_the_growth_rate_and_smooths() {
    new_test_ext().execute_with(|| {
        let held = bounds(1_000, 10_000, 500, 200, 8_000);
        // Growth caps the rise at 250. Smoothing keeps 80 percent of 5000.
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(9_000, 5_000, &held),
            5_050
        );
    });
}

#[test]
fn falling_trust_score_moves_by_the_decay_rate_and_smooths() {
    new_test_ext().execute_with(|| {
        let held = bounds(1_000, 10_000, 500, 200, 8_000);
        // Decay caps the fall at 100. Smoothing keeps 80 percent of 5000.
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(1_000, 5_000, &held),
            4_980
        );
    });
}

#[test]
fn small_trust_score_moves_by_at_least_one_point() {
    new_test_ext().execute_with(|| {
        let held = bounds(0, 100, 500, 200, 0);
        assert_eq!(DcfPallet::apply_trust_score_bounds(20, 10, &held), 11);
        assert_eq!(DcfPallet::apply_trust_score_bounds(1, 10, &held), 9);
    });
}

#[test]
fn steady_trust_score_holds_its_value() {
    new_test_ext().execute_with(|| {
        let held = bounds(1_000, 10_000, 500, 200, 8_000);
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(5_000, 5_000, &held),
            5_000
        );
    });
}

#[test]
fn smoothed_trust_score_clamps_at_both_bounds() {
    new_test_ext().execute_with(|| {
        let ceiling = bounds(0, 5_100, 500, 200, 0);
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(9_000, 5_000, &ceiling),
            5_100
        );
        let floor = bounds(4_950, 10_000, 500, 200, 0);
        assert_eq!(
            DcfPallet::apply_trust_score_bounds(1_000, 5_000, &floor),
            4_950
        );
    });
}

// ---------------------------------------------------------------------------
// Standing Engine: Trust Score calculation
// ---------------------------------------------------------------------------

#[test]
fn trust_score_of_an_unknown_validator_reads_the_floor() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::calculate_trust_score(&99), 1_000);
    });
}

#[test]
fn trust_score_with_both_weights_at_zero_reads_the_floor() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        crate::TrustScoreConfigStorage::<Test>::mutate(|weights| {
            weights.uptime_weight = 0;
            weights.inference_weight = 0;
        });
        assert_eq!(DcfPallet::calculate_trust_score(&1), 1_000);
    });
}

#[test]
fn trust_score_at_genesis_weighs_full_uptime_and_neutral_inference() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        // Uptime reads 10000 and inference reads 5000. The record weighs them
        // 40 to 35, which gives 7666.
        assert_eq!(DcfPallet::calculate_trust_score(&1), 7_666);
    });
}

#[test]
fn trust_score_takes_the_high_penalty_under_a_quarter_of_the_ceiling() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(10, 0));
        // The penalty reads 2500 and the slashing weight takes 25 percent.
        assert_eq!(DcfPallet::calculate_trust_score(&1), 7_041);
    });
}

#[test]
fn trust_score_takes_the_medium_penalty_under_half_of_the_ceiling() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(30, 0));
        assert_eq!(DcfPallet::calculate_trust_score(&1), 7_354);
    });
}

#[test]
fn trust_score_in_a_later_epoch_reads_uptime_blocks_and_inference() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(4u32);
        seed(1, state_with(2_400, 4));
        edit(1, |held| {
            held.uptime = 2;
            held.inference_count = 8;
            held.inference_success_count = 6;
        });
        crate::ValidatorBlocksAuthored::<Test>::insert(&1, 3);
        crate::ValidatorBlocksMissed::<Test>::insert(&1, 1);
        // Uptime 5750 and inference 4580, weighed 40 to 35.
        assert_eq!(DcfPallet::calculate_trust_score(&1), 5_204);
    });
}

#[test]
fn trust_score_with_a_previous_score_moves_inside_the_bounds() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        crate::ValidatorTrustScores::<Test>::insert(1u64, 5_000u64);
        assert_eq!(DcfPallet::calculate_trust_score(&1), 5_050);
    });
}

// ---------------------------------------------------------------------------
// Standing Engine: Trust Score update and history
// ---------------------------------------------------------------------------

#[test]
fn first_trust_score_update_writes_the_score_the_history_and_the_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        assert_ok!(DcfPallet::update_trust_score(&1));
        assert_eq!(crate::ValidatorTrustScores::<Test>::get(1u64), 7_666);
        assert_eq!(state_of(1).trust_score, 7_666);
        assert_eq!(
            crate::TrustScoreHistory::<Test>::get(1u64).into_inner(),
            vec![(0, 7_666)]
        );
        assert!(has_event(crate::Event::TrustScoreUpdated {
            validator: 1,
            old_score: 0,
            new_score: 7_666,
            uptime_component: 10_000,
            inference_component: 5_000,
            slashing_component: 0,
        }));
    });
}

#[test]
fn repeated_trust_score_update_with_no_movement_records_nothing_new() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        assert_ok!(DcfPallet::update_trust_score(&1));
        assert_ok!(DcfPallet::update_trust_score(&1));
        assert_eq!(crate::ValidatorTrustScores::<Test>::get(1u64), 7_666);
        assert_eq!(crate::TrustScoreHistory::<Test>::get(1u64).len(), 1);
        let updates = dcf_events()
            .iter()
            .filter(|event| matches!(event, crate::Event::TrustScoreUpdated { .. }))
            .count();
        assert_eq!(updates, 1);
    });
}

#[test]
fn small_trust_score_movement_stays_out_of_the_history() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        crate::ValidatorTrustScores::<Test>::insert(1u64, 7_660u64);
        assert_ok!(DcfPallet::update_trust_score(&1));
        // The score moves one point. The history records moves above 100.
        assert_eq!(crate::ValidatorTrustScores::<Test>::get(1u64), 7_661);
        assert!(crate::TrustScoreHistory::<Test>::get(1u64).is_empty());
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::TrustScoreUpdated { .. })));
    });
}

#[test]
fn trust_score_update_reports_the_high_slashing_component() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(4u32);
        seed(1, state_with(10, 4));
        edit(1, |held| {
            held.uptime = 2;
            held.inference_count = 8;
            held.inference_success_count = 6;
        });
        crate::ValidatorBlocksAuthored::<Test>::insert(&1, 3);
        crate::ValidatorBlocksMissed::<Test>::insert(&1, 1);
        assert_ok!(DcfPallet::update_trust_score(&1));
        assert!(has_event(crate::Event::TrustScoreUpdated {
            validator: 1,
            old_score: 0,
            new_score: 4_579,
            uptime_component: 5_000,
            inference_component: 7_500,
            slashing_component: 2_500,
        }));
        assert_eq!(
            crate::TrustScoreHistory::<Test>::get(1u64).into_inner(),
            vec![(4, 4_579)]
        );
    });
}

#[test]
fn trust_score_update_reports_the_medium_slashing_component() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(30, 0));
        assert_ok!(DcfPallet::update_trust_score(&1));
        assert!(has_event(crate::Event::TrustScoreUpdated {
            validator: 1,
            old_score: 0,
            new_score: 7_354,
            uptime_component: 10_000,
            inference_component: 5_000,
            slashing_component: 1_250,
        }));
    });
}

#[test]
fn full_trust_score_history_drops_the_oldest_entry() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        let entries: Vec<(u32, u64)> = (0..100u32).map(|step| (step, step as u64)).collect();
        crate::TrustScoreHistory::<Test>::insert(1u64, history(entries));
        assert_ok!(DcfPallet::update_trust_score(&1));
        let held = crate::TrustScoreHistory::<Test>::get(1u64);
        assert_eq!(held.len(), 100);
        assert_eq!(held.first().copied(), Some((1, 1)));
        assert_eq!(held.last().copied(), Some((0, 7_666)));
    });
}

/// The history holds up to one hundred entries. Two significant movements in
/// two epochs therefore leave two entries.
#[test]
fn trust_score_history_grows_with_each_significant_movement() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        assert_ok!(DcfPallet::update_trust_score(&1));

        crate::CurrentEpoch::<Test>::put(1u32);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 0u64);
        assert_ok!(DcfPallet::update_trust_score(&1));

        assert_eq!(
            crate::TrustScoreHistory::<Test>::get(1u64).into_inner(),
            vec![(0, 7_666), (1, 6_733)],
            "The Trust Score history kept one entry. Compare the length with the bound instead of the vector capacity."
        );
    });
}

/// A validator with no record holds no Trust Score. The update therefore
/// fails and leaves storage untouched.
#[test]
fn trust_score_update_for_an_unknown_validator_writes_nothing() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_trust_score(&99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

// ---------------------------------------------------------------------------
// Standing Engine: batch update and stability metrics
// ---------------------------------------------------------------------------

#[test]
fn batch_update_scores_every_active_validator_and_writes_the_metrics() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        seed(2, state_with(2_800, 0));
        seed(3, state_with(3_200, 0));
        let _ = DcfPallet::update_all_trust_scores();
        for who in [1u64, 2, 3] {
            assert_eq!(crate::ValidatorTrustScores::<Test>::get(who), 7_666);
        }
        assert_eq!(
            DcfPallet::trust_score_stability_metrics(),
            crate::TrustScoreStabilityMetricsData {
                epoch: 0,
                avg_score_change: 0,
                max_score_change: 0,
                validators_at_min_bound: 0,
                validators_at_max_bound: 0,
                score_standard_deviation: 0,
                score_median: 7_666,
                high_performers_count: 1,
                low_performers_count: 0,
                stability_index: 10_000,
            }
        );
    });
}

#[test]
fn batch_update_continues_past_a_validator_without_a_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        set_active(vec![9, 1]);
        crate::CurrentEpoch::<Test>::put(2u32);
        let _ = DcfPallet::update_all_trust_scores();
        assert!(crate::ValidatorTrustScores::<Test>::get(1u64) > 0);
        assert_eq!(
            state_of(1).trust_score,
            crate::ValidatorTrustScores::<Test>::get(1u64)
        );
        assert_eq!(DcfPallet::trust_score_stability_metrics().epoch, 2);
    });
}

#[test]
fn stability_metrics_read_changes_bounds_spread_and_median() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(5u32);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 1_000u64);
        crate::ValidatorTrustScores::<Test>::insert(2u64, 5_000u64);
        crate::ValidatorTrustScores::<Test>::insert(3u64, 10_000u64);
        // Validators 1 and 2 carry an entry from the previous epoch. Validator
        // 3 carries an older entry, which the change count leaves aside.
        crate::TrustScoreHistory::<Test>::insert(1u64, history(vec![(4, 1_200)]));
        crate::TrustScoreHistory::<Test>::insert(2u64, history(vec![(4, 4_000)]));
        crate::TrustScoreHistory::<Test>::insert(3u64, history(vec![(2, 9_000)]));
        DcfPallet::update_trust_score_stability_metrics();
        assert_eq!(
            DcfPallet::trust_score_stability_metrics(),
            crate::TrustScoreStabilityMetricsData {
                epoch: 5,
                avg_score_change: 600,
                max_score_change: 1_000,
                validators_at_min_bound: 1,
                validators_at_max_bound: 1,
                score_standard_deviation: 3_681,
                score_median: 5_000,
                high_performers_count: 1,
                low_performers_count: 0,
                stability_index: 9_400,
            }
        );
    });
}

#[test]
fn stability_metrics_take_the_middle_pair_for_an_even_count() {
    new_test_ext().execute_with(|| {
        set_active(vec![1, 2, 3, 4]);
        for (who, score) in [(1u64, 2_000u64), (2, 4_000), (3, 6_000), (4, 8_000)] {
            crate::ValidatorTrustScores::<Test>::insert(who, score);
        }
        DcfPallet::update_trust_score_stability_metrics();
        assert_eq!(
            DcfPallet::trust_score_stability_metrics(),
            crate::TrustScoreStabilityMetricsData {
                epoch: 0,
                avg_score_change: 0,
                max_score_change: 0,
                validators_at_min_bound: 0,
                validators_at_max_bound: 0,
                score_standard_deviation: 2_236,
                score_median: 5_000,
                high_performers_count: 1,
                low_performers_count: 0,
                stability_index: 10_000,
            }
        );
    });
}

#[test]
fn stability_metrics_of_ten_equal_scores_count_both_percentile_tails() {
    new_test_ext().execute_with(|| {
        let ten: Vec<u64> = (1..=10).collect();
        set_active(ten.clone());
        for who in ten {
            crate::ValidatorTrustScores::<Test>::insert(who, 5_000u64);
        }
        DcfPallet::update_trust_score_stability_metrics();
        let held = DcfPallet::trust_score_stability_metrics();
        assert_eq!(held.score_standard_deviation, 0);
        assert_eq!(held.score_median, 5_000);
        assert_eq!(held.high_performers_count, 1);
        assert_eq!(held.low_performers_count, 1);
    });
}

#[test]
fn stability_metrics_hold_the_previous_record_for_an_empty_active_set() {
    new_test_ext().execute_with(|| {
        let sentinel = crate::TrustScoreStabilityMetricsData {
            epoch: 77,
            ..Default::default()
        };
        crate::TrustScoreStabilityMetrics::<Test>::put(sentinel.clone());
        set_active(vec![]);
        DcfPallet::update_trust_score_stability_metrics();
        assert_eq!(DcfPallet::trust_score_stability_metrics(), sentinel);
    });
}

#[test]
fn stability_metrics_with_a_zero_ceiling_read_full_stability() {
    new_test_ext().execute_with(|| {
        crate::TrustScoreBounds::<Test>::put(bounds(0, 0, 500, 200, 8_000));
        crate::CurrentEpoch::<Test>::put(2u32);
        set_active(vec![1]);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 3_000u64);
        crate::TrustScoreHistory::<Test>::insert(1u64, history(vec![(1, 1_000)]));
        DcfPallet::update_trust_score_stability_metrics();
        assert_eq!(
            DcfPallet::trust_score_stability_metrics(),
            crate::TrustScoreStabilityMetricsData {
                epoch: 2,
                avg_score_change: 2_000,
                max_score_change: 2_000,
                validators_at_min_bound: 0,
                validators_at_max_bound: 1,
                score_standard_deviation: 0,
                score_median: 3_000,
                high_performers_count: 0,
                low_performers_count: 0,
                stability_index: 10_000,
            }
        );
    });
}

// ---------------------------------------------------------------------------
// Authoring Engine: block authorship read from the digest at block 11
// ---------------------------------------------------------------------------

#[test]
fn expected_author_in_the_pre_runtime_item_earns_the_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::PreRuntime(*b"ceru", author_bytes(2, 64)));
        run_hook(11);
        let held = state_of(2);
        assert_eq!(held.current.authored_blocks, 1);
        assert_eq!(held.current.missed_blocks, 0);
        assert_eq!(held.last_active_block, 11);
        assert_eq!(crate::ValidatorBlocksAuthored::<Test>::get(&2), 1);
        assert!(has_event(crate::Event::ValidatorScoreBoosted {
            validator: 2,
            old_score: 60,
            new_score: 70,
            reason: crate::ScoreBoostReason::ValidBlockAuthored,
        }));
    });
}

#[test]
fn expected_author_in_the_seal_item_earns_the_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::Seal(*b"ceru", author_bytes(2, 64)));
        run_hook(11);
        assert_eq!(state_of(2).current.authored_blocks, 1);
        assert_eq!(state_of(2).current.missed_blocks, 0);
    });
}

#[test]
fn expected_author_in_the_consensus_item_earns_the_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::Consensus(*b"ceru", 2u64.encode()));
        run_hook(11);
        assert_eq!(state_of(2).current.authored_blocks, 1);
        assert_eq!(state_of(2).current.missed_blocks, 0);
    });
}

#[test]
fn another_author_in_the_digest_costs_the_expected_author_a_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::PreRuntime(*b"ceru", author_bytes(3, 64)));
        run_hook(11);
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(3).current.authored_blocks, 0);
        assert!(has_event(crate::Event::InvalidAuthor {
            block_number: 11,
            author: 3,
        }));
    });
}

#[test]
fn empty_digest_costs_the_expected_author_a_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        run_hook(11);
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(2).current.authored_blocks, 0);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::InvalidAuthor { .. })));
    });
}

#[test]
fn digest_items_from_another_engine_count_as_a_missed_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::PreRuntime(*b"othr", author_bytes(2, 64)));
        System::deposit_log(DigestItem::Seal(*b"othr", author_bytes(2, 64)));
        System::deposit_log(DigestItem::Consensus(*b"othr", 2u64.encode()));
        run_hook(11);
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(2).current.authored_blocks, 0);
    });
}

#[test]
fn short_author_records_count_as_a_missed_block() {
    new_test_ext().execute_with(|| {
        authorship_block();
        System::deposit_log(DigestItem::PreRuntime(*b"ceru", author_bytes(2, 16)));
        System::deposit_log(DigestItem::Seal(*b"ceru", author_bytes(2, 31)));
        System::deposit_log(DigestItem::Consensus(*b"ceru", vec![2, 0, 0]));
        System::deposit_log(DigestItem::Other(vec![1, 2, 3]));
        run_hook(11);
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(2).current.authored_blocks, 0);
    });
}

#[test]
fn empty_active_set_leaves_every_authorship_count_unchanged() {
    new_test_ext().execute_with(|| {
        authorship_block();
        set_active(vec![]);
        run_hook(11);
        for who in [2u64, 3] {
            assert_eq!(state_of(who).current.missed_blocks, 0);
            assert_eq!(state_of(who).current.authored_blocks, 0);
        }
    });
}

// ---------------------------------------------------------------------------
// Admission Engine: underperformance check at block 50
// ---------------------------------------------------------------------------

#[test]
fn score_under_the_floor_takes_the_seat_at_the_check() {
    new_test_ext().execute_with(|| {
        System::set_block_number(50);
        seed(1, state_with(2_000, 0));
        seed(2, state_with(2_000, 0));
        seed(3, state_with(10, 0));
        run_hook(50);
        assert_eq!(active(), vec![1, 2]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 3,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

#[test]
fn long_inactivity_takes_the_seat_at_the_check() {
    new_test_ext().execute_with(|| {
        System::set_block_number(50);
        crate::CurrentEpoch::<Test>::put(10u32);
        seed(1, state_with(2_000, 10));
        seed(2, state_with(2_000, 0));
        seed(3, state_with(2_000, 10));
        run_hook(50);
        // Validator 2 sat out ten epochs, past the limit of five.
        assert_eq!(active(), vec![1, 3]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 2,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

#[test]
fn healthy_validators_and_validators_without_a_record_keep_their_seats() {
    new_test_ext().execute_with(|| {
        System::set_block_number(50);
        for who in [1u64, 2, 3] {
            seed(who, state_with(2_000, 0));
        }
        set_active(vec![1, 2, 3, 9]);
        run_hook(50);
        assert_eq!(active(), vec![1, 2, 3, 9]);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorEjected { .. })));
    });
}

// ---------------------------------------------------------------------------
// Standing Engine: score decay at block 10 and participation at block 0
// ---------------------------------------------------------------------------

#[test]
fn inactive_validator_decays_at_the_decay_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        crate::CurrentEpoch::<Test>::put(3u32);
        seed(1, state_with(2_400, 0));
        seed(2, state_with(2_000, 3));
        seed(3, state_with(2_000, 3));
        set_registered(vec![1, 2, 3, 9]);
        run_hook(10);
        // The decay takes 2 points and the score ceiling of 100 holds the rest.
        assert_eq!(state_of(1).current.final_score, 100);
        assert!(has_event(crate::Event::ValidatorScoreDecayed {
            validator: 1,
            old_score: 2_400,
            new_score: 100,
        }));
        assert_eq!(state_of(2).current.final_score, 2_000);
        let decayed = dcf_events()
            .iter()
            .filter(|event| matches!(event, crate::Event::ValidatorScoreDecayed { .. }))
            .count();
        assert_eq!(decayed, 1);
        assert!(!crate::ValidatorStates::<Test>::contains_key(&9));
    });
}

#[test]
fn decay_under_the_floor_takes_the_seat() {
    new_test_ext().execute_with(|| {
        System::set_block_number(10);
        crate::CurrentEpoch::<Test>::put(3u32);
        seed(1, state_with(40, 0));
        seed(2, state_with(2_000, 3));
        seed(3, state_with(2_000, 3));
        run_hook(10);
        assert_eq!(active(), vec![2, 3]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 1,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

#[test]
fn participation_rate_follows_authored_and_missed_blocks() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        for who in [1u64, 2, 3] {
            seed(who, state_with(2_000, 0));
        }
        edit(1, |held| {
            held.current.authored_blocks = 1;
            held.current.missed_blocks = 3;
        });
        // Validator 2 owns the slot of block 0 and misses it.
        fix_author_for_epoch(0, 2);
        run_hook(0);
        assert_eq!(state_of(1).participation_rate, 2_500);
        assert_eq!(state_of(2).participation_rate, 0);
        assert_eq!(state_of(3).participation_rate, 100);
    });
}

#[test]
fn block_zero_runs_every_interval_with_a_weight_per_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // Base 10000, counter reset 10000, decay 3 x 10000 and participation
        // 3 x 10000.
        assert_eq!(hook_weight(0), Weight::from_parts(80_000, 0));
    });
}

#[test]
fn block_zero_with_no_validators_reads_the_base_weight() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_registered(vec![]);
        set_active(vec![]);
        assert_eq!(hook_weight(0), Weight::from_parts(20_000, 0));
    });
}

// ---------------------------------------------------------------------------
// Admission Engine: automatic proposals at block 0
// ---------------------------------------------------------------------------

#[test]
fn high_scoring_candidate_receives_an_automatic_join() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_registered(vec![1, 2, 3, 10]);
        seed(10, state_with(80, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 1_000u128);
        run_hook(0);
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&10),
            Some(crate::ValidatorAction::Join)
        );
        assert!(has_event(crate::Event::ValidatorJoined {
            validator: 10,
            stake_amount: 1_000,
        }));
        assert!(has_event(crate::Event::AutomaticValidatorProposal {
            validator: 10,
            action: crate::ValidatorAction::Join,
            score: 80,
            reason: reason(b"High score automatic addition"),
        }));
    });
}

#[test]
fn candidate_filters_hold_back_low_pending_removed_and_unrecorded_validators() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_registered(vec![1, 2, 3, 11, 12, 13, 14]);
        seed(11, state_with(40, 0));
        seed(12, state_with(90, 0));
        crate::PendingValidatorActions::<Test>::insert(&12, crate::ValidatorAction::Join);
        seed(13, state_with(90, 0));
        crate::RecentlyRemovedValidators::<Test>::insert(&13, 1);
        run_hook(0);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&11), None);
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&12),
            Some(crate::ValidatorAction::Join)
        );
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&13), None);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&14), None);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::AutomaticValidatorProposal { .. })));
    });
}

#[test]
fn automatic_proposals_wait_for_governance_mode() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_registered(vec![1, 2, 3, 10]);
        seed(10, state_with(80, 0));
        run_hook(0);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&10), None);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::AutomaticValidatorProposal { .. })));
    });
}

#[test]
fn automatic_joins_stop_at_the_free_seats() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_active((1..=99).collect());
        set_registered(vec![1, 2, 3, 200, 201]);
        seed(200, state_with(90, 0));
        seed(201, state_with(70, 0));
        run_hook(0);
        // One seat stays free, so the higher scoring candidate takes it.
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&200),
            Some(crate::ValidatorAction::Join)
        );
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&201), None);
    });
}

#[test]
fn full_set_swaps_the_lowest_seat_for_a_stronger_candidate() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_active((1..=100).collect());
        set_registered(vec![1, 2, 3, 200]);
        seed(200, state_with(1_200, 0));
        run_hook(0);
        // Validator 4 holds the lowest score of zero among the seated set.
        assert_eq!(active().len(), 99);
        assert!(!active().contains(&4));
        assert_eq!(
            crate::PendingValidatorActions::<Test>::get(&200),
            Some(crate::ValidatorAction::Join)
        );
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 4,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
        assert!(has_event(crate::Event::AutomaticValidatorProposal {
            validator: 4,
            action: crate::ValidatorAction::Leave,
            score: 0,
            reason: reason(b"Low score automatic ejection"),
        }));
        assert!(has_event(crate::Event::AutomaticValidatorProposal {
            validator: 200,
            action: crate::ValidatorAction::Join,
            score: 1_200,
            reason: reason(b"High score automatic addition"),
        }));
    });
}

#[test]
fn full_set_keeps_its_seats_for_a_gain_under_the_fixed_threshold() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_active((1..=100).collect());
        set_registered(vec![1, 2, 3, 200]);
        seed(200, state_with(900, 0));
        run_hook(0);
        assert_eq!(active().len(), 100);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&200), None);
    });
}

#[test]
fn full_set_keeps_its_seats_for_a_gain_under_the_percentage_threshold() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::GovernanceModeEnabled::<Test>::put(true);
        let seated: Vec<u64> = (1..=100).collect();
        set_active(seated.clone());
        for who in seated {
            seed(who, state_with(20_000, 0));
        }
        set_registered(vec![200]);
        // The gain of 1500 clears the fixed 1000 and falls short of the
        // 2000 that ten percent of 20000 asks for.
        seed(200, state_with(21_500, 0));
        run_hook(0);
        assert_eq!(active().len(), 100);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&200), None);
    });
}

// ---------------------------------------------------------------------------
// Admission Engine: recent removal check
// ---------------------------------------------------------------------------

#[test]
fn removal_inside_the_cooldown_reads_as_recent() {
    new_test_ext().execute_with(|| {
        crate::RecentlyRemovedValidators::<Test>::insert(&10, 1);
        System::set_block_number(500);
        assert!(DcfPallet::was_recently_ejected(&10));
    });
}

#[test]
fn removal_past_the_cooldown_without_a_record_reads_as_settled() {
    new_test_ext().execute_with(|| {
        crate::RecentlyRemovedValidators::<Test>::insert(&10, 1);
        System::set_block_number(2_000);
        assert!(!DcfPallet::was_recently_ejected(&10));
    });
}

#[test]
fn zero_score_inside_the_grace_epochs_reads_as_recent() {
    new_test_ext().execute_with(|| {
        crate::RecentlyRemovedValidators::<Test>::insert(&10, 1);
        System::set_block_number(2_000);
        seed(10, state_with(0, 0));
        assert!(DcfPallet::was_recently_ejected(&10));
        // Five epochs sit at the edge of the grace window.
        crate::CurrentEpoch::<Test>::put(5u32);
        assert!(DcfPallet::was_recently_ejected(&10));
    });
}

#[test]
fn zero_score_past_the_grace_epochs_reads_as_settled() {
    new_test_ext().execute_with(|| {
        seed(10, state_with(0, 0));
        crate::CurrentEpoch::<Test>::put(6u32);
        assert!(!DcfPallet::was_recently_ejected(&10));
    });
}

#[test]
fn scored_validator_and_unknown_account_read_as_settled() {
    new_test_ext().execute_with(|| {
        seed(10, state_with(80, 0));
        assert!(!DcfPallet::was_recently_ejected(&10));
        assert!(!DcfPallet::was_recently_ejected(&11));
    });
}

// ---------------------------------------------------------------------------
// Epoch boundary at block 100
// ---------------------------------------------------------------------------

#[test]
fn epoch_boundary_scores_stake_as_a_share_of_the_total() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 2_000u128);
        run_hook(100);
        assert!(has_event(crate::Event::EpochBoundaryDetected {
            block_number: 100,
            epoch: 0,
            governance_mode: false,
        }));
        // The total stake reads 4000 and the scale reads 32000.
        assert_eq!(state_of(1).current.stake_score, 8_000);
        assert_eq!(state_of(2).current.stake_score, 16_000);
        assert_eq!(state_of(3).current.stake_score, 8_000);
        for who in [1u64, 2, 3] {
            assert_eq!(state_of(who).current.final_score, 100);
        }
    });
}

#[test]
fn epoch_boundary_scores_reach_the_high_middle_and_low_tiers() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 1_000_000u128);
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 4_000u128);
        pallet_cerulea_pos::Stake::<Test>::insert(3u64, 0u128);
        run_hook(100);
        assert_eq!(state_of(1).current.stake_score, 31_872);
        assert_eq!(state_of(2).current.stake_score, 127);
        assert_eq!(state_of(3).current.stake_score, 0);
        assert_eq!(state_of(1).current.final_score, 100);
        assert_eq!(state_of(2).current.final_score, 76);
        assert_eq!(state_of(3).current.final_score, 0);
    });
}

#[test]
fn epoch_boundary_with_no_stake_scores_zero_and_skips_unrecorded_validators() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        for who in [1u64, 2, 3] {
            pallet_cerulea_pos::Stake::<Test>::insert(who, 0u128);
        }
        set_registered(vec![1, 2, 3, 9]);
        run_hook(100);
        for who in [1u64, 2, 3] {
            assert_eq!(state_of(who).current.stake_score, 0);
            assert_eq!(state_of(who).current.final_score, 0);
        }
        assert!(!crate::ValidatorStates::<Test>::contains_key(&9));
    });
}

#[test]
fn epoch_boundary_after_epoch_zero_closes_the_previous_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        crate::CurrentEpoch::<Test>::put(4u32);
        run_hook(100);
        assert_eq!(DcfPallet::current_epoch(), 5);
        assert!(has_event(crate::Event::EpochBoundaryDetected {
            block_number: 100,
            epoch: 4,
            governance_mode: false,
        }));
        assert!(has_event(crate::Event::EpochEnded { epoch: 4 }));
    });
}

#[test]
fn first_epoch_boundary_records_no_closed_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        run_hook(100);
        assert_eq!(DcfPallet::current_epoch(), 1);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::EpochEnded { .. })));
    });
}

#[test]
fn epoch_boundary_counts_uptime_and_participation_for_active_validators() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        for who in [1u64, 2, 3] {
            seed(who, state_with(2_000, 0));
        }
        edit(1, |held| {
            held.current.authored_blocks = 3;
            held.current.missed_blocks = 1;
        });
        run_hook(100);
        assert_eq!(state_of(1).uptime, 2);
        assert_eq!(state_of(1).participation_rate, 7_500);
        assert_eq!(state_of(2).uptime, 2);
        assert_eq!(state_of(2).participation_rate, 100);
    });
}

#[test]
fn epoch_boundary_executes_set_changes_and_keeps_manual_proposals_pending() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        pallet_cerulea_pos::Stake::<Test>::insert(20u64, 1_000u128);
        let rewarded: BoundedVec<u64, _> = BoundedVec::truncate_from(vec![1u64]);
        crate::Proposals::<Test>::insert(
            0u32,
            proposal(
                crate::ProposalAction::AddValidator { validator: 20 },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            1u32,
            proposal(
                crate::ProposalAction::RemoveValidator { validator: 3 },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            2u32,
            proposal(
                crate::ProposalAction::Eject {
                    validator: 2,
                    reason: crate::EjectionReason::ExcessValidators,
                },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            3u32,
            proposal(
                crate::ProposalAction::Eject {
                    validator: 1,
                    reason: crate::EjectionReason::ManualEjection,
                },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            4u32,
            proposal(
                crate::ProposalAction::Slash {
                    validator: 1,
                    amount: 100,
                },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            5u32,
            proposal(
                crate::ProposalAction::Reward {
                    validator: 1,
                    amount: 100,
                },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            6u32,
            proposal(
                crate::ProposalAction::RewardMultiple {
                    validators: rewarded,
                    amount: 100,
                },
                crate::ProposalStatus::Pending,
            ),
        );
        crate::Proposals::<Test>::insert(
            7u32,
            proposal(
                crate::ProposalAction::AddValidator { validator: 21 },
                crate::ProposalStatus::Approved,
            ),
        );

        run_hook(100);

        for id in [0u32, 1, 2] {
            assert_eq!(status_of(id), Some(crate::ProposalStatus::Executed));
        }
        for id in [3u32, 4, 5, 6] {
            assert_eq!(status_of(id), Some(crate::ProposalStatus::Pending));
        }
        assert_eq!(status_of(7), Some(crate::ProposalStatus::Approved));

        // The stake pallet released the removed and the ejected validator.
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(3u64), 0);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(2u64), 0);
        assert!(has_event(crate::Event::ValidatorRemoved { validator: 3 }));
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 2,
            reason: crate::EjectionReason::ExcessValidators,
        }));
    });
}

/// An approved addition seats a validator that holds the minimum stake. The
/// registered set holds three of one hundred seats, so the validator joins.
#[test]
fn epoch_boundary_addition_registers_the_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        pallet_cerulea_pos::Stake::<Test>::insert(20u64, 1_000u128);
        crate::Proposals::<Test>::insert(
            0u32,
            proposal(
                crate::ProposalAction::AddValidator { validator: 20 },
                crate::ProposalStatus::Pending,
            ),
        );
        run_hook(100);
        assert!(
            registered().contains(&20),
            "The addition left validator 20 outside the set. Compare the set length with the bound instead of the vector capacity."
        );
        assert!(crate::ValidatorStates::<Test>::contains_key(&20));
        assert!(has_event(crate::Event::ValidatorAdded { validator: 20 }));
    });
}

#[test]
fn epoch_boundary_proposes_a_join_from_the_refreshed_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_registered(vec![1, 2, 3, 10]);
        seed(10, state_with(80, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 1_000u128);
        run_hook(100);
        // The refreshed score reads 4800 and the score ceiling holds it at 100.
        assert!(has_event(crate::Event::AutomaticValidatorProposal {
            validator: 10,
            action: crate::ValidatorAction::Join,
            score: 100,
            reason: reason(b"High score automatic addition"),
        }));
        assert!(has_event(crate::Event::EpochBoundaryDetected {
            block_number: 100,
            epoch: 0,
            governance_mode: true,
        }));
    });
}

/// A scheduled join lands at the epoch boundary when a seat stays free. The
/// active set holds three of one hundred seats.
#[test]
fn scheduled_join_takes_a_free_seat_at_the_epoch_boundary() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        crate::GovernanceModeEnabled::<Test>::put(true);
        set_registered(vec![1, 2, 3, 10]);
        seed(10, state_with(80, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 1_000u128);
        run_hook(100);
        assert!(
            active().contains(&10),
            "The scheduled join found no free seat. Count free seats from the bound instead of the vector capacity."
        );
    });
}

// ---------------------------------------------------------------------------
// Inference module interface
// ---------------------------------------------------------------------------

#[test]
fn inference_activity_counts_one_result_and_marks_the_validator_active() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        crate::CurrentEpoch::<Test>::put(2u32);
        seed(1, state_with(2_400, 0));
        assert_ok!(interface_record(1));
        let held = state_of(1);
        assert_eq!(held.inference_count, 1);
        assert_eq!(held.last_active_block, 7);
        assert_eq!(held.last_active_epoch, 2);
    });
}

#[test]
fn inference_activity_for_an_unknown_validator_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            interface_record(99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn interface_ejection_takes_the_seat_and_records_the_reason() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert_ok!(interface_eject(1));
        assert_eq!(active(), vec![2, 3]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 1,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

/// The status record names the status the validator held before the
/// ejection. Validator 1 held an active seat.
#[test]
fn interface_ejection_records_the_active_status_it_ends() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert_ok!(interface_eject(1));
        assert!(
            has_event(crate::Event::ValidatorStatusChanged {
                validator: 1,
                old_status: crate::ValidatorStatus::Active,
                new_status: crate::ValidatorStatus::Ejected,
                block_number: 3,
            }),
            "The ejection named the earlier status as inactive. Read the status before the seat goes."
        );
    });
}

#[test]
fn interface_ejection_of_an_inactive_validator_records_the_inactive_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert_ok!(interface_eject(9));
        assert_eq!(active(), vec![1, 2, 3]);
        assert!(has_event(crate::Event::ValidatorStatusChanged {
            validator: 9,
            old_status: crate::ValidatorStatus::Inactive,
            new_status: crate::ValidatorStatus::Ejected,
            block_number: 3,
        }));
    });
}

#[test]
fn interface_final_score_recomputes_from_the_stake_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        edit(1, |held| held.current.stake_score = 150);
        assert_ok!(interface_final_score(1));
        // 150 at the stake share of 6000 parts in 10000 reads 90.
        assert_eq!(state_of(1).current.final_score, 90);
        assert!(has_event(crate::Event::ValidatorScoreUpdated {
            validator: 1,
            stake_score: 150,
            inference_score: 0,
            final_score: 90,
        }));
    });
}

#[test]
fn interface_final_score_restores_the_missing_shares() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        crate::PosWeight::<Test>::kill();
        crate::PoiWeight::<Test>::kill();
        assert_ok!(interface_final_score(1));
        assert!(crate::PosWeight::<Test>::exists());
        assert!(crate::PoiWeight::<Test>::exists());
        assert_eq!(DcfPallet::pos_weight(), 6_000);
        assert_eq!(DcfPallet::poi_weight(), 4_000);
    });
}

/// The score history holds up to ten epochs. One recomputation over one
/// stored entry therefore leaves two entries.
#[test]
fn interface_final_score_grows_the_score_history() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        assert_ok!(interface_final_score(1));
        assert_eq!(
            state_of(1).history.len(),
            2,
            "The score history kept one entry. Compare the length with the bound instead of the vector capacity."
        );
    });
}

#[test]
fn interface_final_score_for_an_unknown_validator_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            interface_final_score(99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

// ---------------------------------------------------------------------------
// Admission Engine handler: joins and departures
// ---------------------------------------------------------------------------

#[test]
fn joined_validator_takes_a_place_and_an_opening_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        crate::CurrentEpoch::<Test>::put(2u32);
        assert_ok!(handler_joined(10, 100));
        assert!(registered().contains(&10));
        let expected = crate::ValidatorState::<Test> {
            last_active_epoch: 2,
            current: crate::EpochStats {
                epoch: 2,
                stake_score: 100,
                inference_score: 0,
                final_score: 60,
                authored_blocks: 0,
                missed_blocks: 0,
            },
            history: BoundedVec::new(),
            uptime: 0,
            inference_success_count: 0,
            participation_rate: 0,
            inference_count: 0,
            last_active_block: 5,
            name: None,
            trust_score: 5_000,
        };
        // The record carries no equality, so the test compares its encoding.
        assert_eq!(state_of(10).encode(), expected.encode());
    });
}

#[test]
fn joined_validator_score_holds_at_the_ceiling() {
    new_test_ext().execute_with(|| {
        assert_ok!(handler_joined(10, 1_000));
        // 1000 at the stake share reads 600 and the ceiling holds it at 100.
        assert_eq!(state_of(10).current.final_score, 100);
    });
}

#[test]
fn joined_validator_restores_the_missing_shares() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::kill();
        crate::PoiWeight::<Test>::kill();
        assert_ok!(handler_joined(10, 100));
        assert_eq!(DcfPallet::pos_weight(), 6_000);
        assert_eq!(DcfPallet::poi_weight(), 4_000);
        assert_eq!(state_of(10).current.final_score, 60);
    });
}

#[test]
fn joined_member_keeps_its_place_and_its_record() {
    new_test_ext().execute_with(|| {
        let before = state_of(1);
        assert_ok!(handler_joined(1, 5_000));
        assert_eq!(registered(), vec![1, 2, 3]);
        assert_eq!(state_of(1).encode(), before.encode());
    });
}

#[test]
fn joined_validator_with_a_record_keeps_the_record() {
    new_test_ext().execute_with(|| {
        seed(10, state_with(77, 0));
        assert_ok!(handler_joined(10, 100));
        assert!(registered().contains(&10));
        assert_eq!(state_of(10).current.final_score, 77);
    });
}

#[test]
fn join_into_a_full_set_is_refused() {
    new_test_ext().execute_with(|| {
        set_registered((1..=100).collect());
        assert_noop!(
            handler_joined(500, 1_000),
            crate::Error::<Test>::NotEnoughValidators
        );
    });
}

#[test]
fn leave_request_releases_the_active_seat_and_keeps_the_place() {
    new_test_ext().execute_with(|| {
        assert_ok!(handler_leave_requested(1));
        assert_eq!(active(), vec![2, 3]);
        assert!(registered().contains(&1));
        assert_ok!(handler_leave_requested(9));
        assert_eq!(active(), vec![2, 3]);
    });
}

#[test]
fn departure_clears_every_record_the_validator_held() {
    new_test_ext().execute_with(|| {
        crate::ValidatorLastSeen::<Test>::insert(&1, 5);
        crate::ValidatorBlocksAuthored::<Test>::insert(&1, 4);
        crate::ValidatorBlocksMissed::<Test>::insert(&1, 2);
        crate::PendingValidatorActions::<Test>::insert(&1, crate::ValidatorAction::Leave);
        pallet_cerulea_poi::ValidatorInferenceCount::<Test>::insert(1u64, 3u64);
        assert_eq!(crate::ValidatorLastSeen::<Test>::get(&1), 5);

        assert_ok!(handler_left(1));

        assert_eq!(registered(), vec![2, 3]);
        assert_eq!(active(), vec![2, 3]);
        assert!(!crate::ValidatorStates::<Test>::contains_key(&1));
        assert_eq!(crate::ValidatorLastSeen::<Test>::get(&1), 0);
        assert_eq!(crate::ValidatorBlocksAuthored::<Test>::get(&1), 0);
        assert_eq!(crate::ValidatorBlocksMissed::<Test>::get(&1), 0);
        assert_eq!(crate::ValidatorUptime::<Test>::get(&1), 0);
        assert_eq!(crate::PendingValidatorActions::<Test>::get(&1), None);
        assert!(!pallet_cerulea_poi::ValidatorInferenceCount::<Test>::contains_key(1u64));
    });
}

#[test]
fn departure_of_an_unknown_account_leaves_the_sets_whole() {
    new_test_ext().execute_with(|| {
        assert_ok!(handler_left(9));
        assert_eq!(registered(), vec![1, 2, 3]);
        assert_eq!(active(), vec![1, 2, 3]);
    });
}

// ---------------------------------------------------------------------------
// Admission Engine handler: stake movements
// ---------------------------------------------------------------------------

#[test]
fn stake_increase_writes_the_new_stake_score() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 150u128);
        assert_ok!(handler_stake_increased(1, 50));
        assert_eq!(state_of(1).current.stake_score, 150);
    });
}

/// A stake movement recomputes the final score from the new stake score.
#[test]
fn stake_increase_recomputes_the_final_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 150u128);
        assert_ok!(handler_stake_increased(1, 50));
        assert_eq!(
            state_of(1).current.final_score,
            90,
            "The final score kept its earlier value. Recompute it after the stake score write closes."
        );
        assert!(has_event(crate::Event::ValidatorScoreUpdated {
            validator: 1,
            stake_score: 150,
            inference_score: 0,
            final_score: 90,
        }));
    });
}

#[test]
fn stake_decrease_writes_the_new_stake_score() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 100u128);
        assert_ok!(handler_stake_decreased(1, 900));
        assert_eq!(state_of(1).current.stake_score, 100);
    });
}

/// A stake movement recomputes the final score from the new stake score.
#[test]
fn stake_decrease_recomputes_the_final_score() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 100u128);
        assert_ok!(handler_stake_decreased(1, 900));
        assert_eq!(
            state_of(1).current.final_score,
            60,
            "The final score kept its earlier value. Recompute it after the stake score write closes."
        );
    });
}

#[test]
fn stake_movement_for_an_unknown_account_opens_no_record() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_pos::Stake::<Test>::insert(99u64, 500u128);
        assert_ok!(handler_stake_increased(99, 500));
        assert_ok!(handler_stake_decreased(99, 100));
        assert!(!crate::ValidatorStates::<Test>::contains_key(&99));
    });
}

// ---------------------------------------------------------------------------
// Admission Engine handler: penalties and rewards
// ---------------------------------------------------------------------------

#[test]
fn slash_lowers_the_score_and_records_the_epoch() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(3u32);
        seed(1, state_with(2_400, 0));
        assert_ok!(handler_slashed(1, 100, 400));
        let held = state_of(1);
        assert_eq!(held.current.final_score, 2_000);
        assert_eq!(held.last_active_epoch, 3);
        assert_eq!(held.current.stake_score, 1_000);
        assert_eq!(
            held.history.last().cloned(),
            Some(crate::EpochStats {
                epoch: 3,
                stake_score: 1_000,
                inference_score: 0,
                final_score: 2_000,
                authored_blocks: 0,
                missed_blocks: 0,
            })
        );
        assert!(active().contains(&1));
    });
}

#[test]
fn slash_under_the_floor_takes_the_seat() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        seed(1, state_with(2_400, 0));
        assert_ok!(handler_slashed(1, 100, 2_380));
        assert_eq!(state_of(1).current.final_score, 20);
        assert!(!active().contains(&1));
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 1,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

#[test]
fn slash_larger_than_the_score_settles_at_zero() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        assert_ok!(handler_slashed(1, 100, u64::MAX));
        assert_eq!(state_of(1).current.final_score, 0);
    });
}

/// The score history holds up to ten epochs. One slash over one stored entry
/// therefore leaves two entries.
#[test]
fn slash_grows_the_score_history() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        assert_ok!(handler_slashed(1, 100, 400));
        assert_eq!(
            state_of(1).history.len(),
            2,
            "The score history kept one entry. Compare the length with the bound instead of the vector capacity."
        );
    });
}

/// A validator the handler admitted starts with an empty score history. A
/// slash on that validator writes the first entry.
#[test]
fn slash_on_an_empty_history_writes_the_first_entry() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        edit(1, |held| held.history = BoundedVec::new());
        assert_ok!(handler_slashed(1, 100, 400));
        assert_eq!(state_of(1).history.len(), 1);
    });
}

#[test]
fn slash_on_a_full_history_drops_the_oldest_entry() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(3u32);
        seed(1, state_with(2_400, 0));
        let ten: Vec<crate::EpochStats> = (0..10u32).map(|epoch| stats(epoch, 100)).collect();
        edit(1, |held| held.history = BoundedVec::truncate_from(ten));
        assert_ok!(handler_slashed(1, 100, 400));
        let held = state_of(1);
        assert_eq!(held.history.len(), 10);
        assert_eq!(held.history.first().map(|entry| entry.epoch), Some(1));
        assert_eq!(
            held.history.last().map(|entry| entry.final_score),
            Some(2_000)
        );
    });
}

#[test]
fn slash_for_an_unknown_validator_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            handler_slashed(99, 100, 10),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn reward_raises_the_score_and_records_the_epoch() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(2u32);
        seed(1, state_with(40, 0));
        assert_ok!(handler_rewarded(1, 10, 30));
        let held = state_of(1);
        assert_eq!(held.current.final_score, 70);
        assert_eq!(held.last_active_epoch, 2);
        assert_eq!(held.current.stake_score, 1_000);
        assert_eq!(held.history.last().map(|entry| entry.final_score), Some(70));
    });
}

#[test]
fn reward_holds_the_score_at_the_ceiling() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(90, 0));
        assert_ok!(handler_rewarded(1, 10, 30));
        assert_eq!(state_of(1).current.final_score, 100);
    });
}

#[test]
fn reward_for_an_unknown_validator_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            handler_rewarded(99, 10, 30),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn handler_reads_the_final_score_and_the_active_set() {
    new_test_ext().execute_with(|| {
        seed(1, state_with(2_400, 0));
        assert_eq!(handler_score(1), 2_400);
        assert_eq!(handler_score(99), 0);
        assert_eq!(handler_active(), vec![1, 2, 3]);
        set_active(vec![2]);
        assert_eq!(handler_active(), vec![2]);
    });
}

// ---------------------------------------------------------------------------
// Custody Engine reports into the Standing Engine
// ---------------------------------------------------------------------------

#[test]
fn custody_share_starts_at_the_starting_share_and_follows_each_report() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(
            DcfPallet::custody_share(&1),
            pallet_cerulea_custody::score::STARTING_SHARE
        );
        <DcfPallet as pallet_cerulea_custody::CustodyStanding<u64>>::report_custody_share(
            &1, 750_000,
        );
        assert_eq!(crate::CustodyShares::<Test>::get(1u64), Some(750_000));
        assert_eq!(DcfPallet::custody_share(&1), 750_000);
        assert!(has_event(crate::Event::CustodyShareRecorded {
            validator: 1,
            share: 750_000,
        }));

        <DcfPallet as pallet_cerulea_custody::CustodyStanding<u64>>::report_custody_share(
            &1, 250_000,
        );
        assert_eq!(DcfPallet::custody_share(&1), 250_000);
    });
}

#[test]
fn custody_faults_start_at_zero_and_follow_each_report() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_eq!(DcfPallet::custody_faults(&1), 0);
        <DcfPallet as pallet_cerulea_custody::CustodyStanding<u64>>::report_custody_fault(&1, 2);
        assert_eq!(crate::CustodyFaults::<Test>::get(1u64), 2);
        assert_eq!(DcfPallet::custody_faults(&1), 2);
        assert!(has_event(crate::Event::CustodyFaultRecorded {
            validator: 1,
            faults: 2,
        }));
        // A report from a second validator leaves the first record whole.
        <DcfPallet as pallet_cerulea_custody::CustodyStanding<u64>>::report_custody_fault(&2, 5);
        assert_eq!(DcfPallet::custody_faults(&1), 2);
        assert_eq!(DcfPallet::custody_faults(&2), 5);
    });
}
