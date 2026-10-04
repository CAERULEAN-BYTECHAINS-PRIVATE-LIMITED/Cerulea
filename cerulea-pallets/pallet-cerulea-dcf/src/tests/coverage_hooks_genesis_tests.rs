//! Coverage tests for the block hooks, the rate limits, the metrics and genesis.
//!
//! The block hooks drive the framework pallet with no person acting. The rate
//! limits guard every proposal and every vote. The metrics give monitoring a
//! single read of the chain state. Genesis seats the first validators. Every
//! test here drives the real pallet through its public calls and hooks and
//! reads the effect back from storage and events.

use crate::engines::assurance::TransactionClass;
use crate::engines::standing::{accumulated, decayed, ServiceRecord};
use crate::mock::*;
use crate::{
    AccountOperationHistory, BlockOperationCounts, DispatchableType, EpochConfig, Error, Event,
    LastOperationBlock, RateLimitConfig, RateLimitConfigStorage,
};
use frame_support::traits::{
    BuildGenesisConfig, ConstU32, Hooks, LockableCurrency, ReservableCurrency, WithdrawReasons,
};
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok, assert_storage_noop, BoundedVec};
use sp_runtime::{DispatchError, DispatchResult};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Run the block initialisation hook for one block.
fn initialise(block: u64) -> Weight {
    System::set_block_number(block);
    <DcfPallet as Hooks<u64>>::on_initialize(block)
}

/// Every framework pallet event recorded so far.
fn dcf_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// Submit one proposal from one account.
fn propose(who: u64) -> DispatchResult {
    DcfPallet::submit_proposal(
        RuntimeOrigin::signed(who),
        crate::ProposalAction::<Test>::AddValidator { validator: 99 },
        None,
    )
}

/// Cast one vote from one account.
fn vote(who: u64, proposal_id: u32) -> DispatchResult {
    DcfPallet::vote_proposal(RuntimeOrigin::signed(who), proposal_id, true)
}

/// A genesis record with the epoch shape the mock runtime expects.
fn genesis_of(
    validators: Vec<u64>,
    stakes: Vec<u128>,
    scores: Vec<u32>,
    names: Vec<Option<Vec<u8>>>,
    blocks_per_epoch: u32,
    strict: bool,
) -> crate::GenesisConfig<Test> {
    crate::GenesisConfig::<Test> {
        validators,
        validator_scores: scores,
        validator_stakes: stakes,
        current_epoch: 0,
        epoch_config: EpochConfig {
            blocks_per_epoch,
            min_stake: 1000,
            max_validators: 100,
        },
        validator_names: names,
        strict_validation: strict,
    }
}

/// A genesis record that passes every comprehensive check.
fn sound_genesis() -> crate::GenesisConfig<Test> {
    genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 100, false)
}

/// Hold the governance epoch length at one hundred blocks.
///
/// The mock genesis runs ten blocks per epoch, the floor of the mock range.
/// Tests that compare figures read one fixed epoch length through this helper.
fn lift_governance_epoch_length() {
    crate::GovernanceConfigStorage::<Test>::mutate(|held| held.epoch_length.current = 100);
}

/// A test externalities with the offchain worker extension registered.
fn ext_with_offchain() -> sp_io::TestExternalities {
    let mut ext = new_test_ext();
    let (offchain, _state) = sp_core::offchain::testing::TestOffchainExt::new();
    ext.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));
    ext
}

// ---------------------------------------------------------------------------
// Runtime upgrade hook
// ---------------------------------------------------------------------------

#[test]
fn upgrade_from_version_zero_migrates_to_the_current_version() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        lift_governance_epoch_length();
        crate::pallet::StorageVersion::<Test>::put(0);

        let weight = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();

        assert_eq!(
            crate::pallet::StorageVersion::<Test>::get(),
            crate::CURRENT_STORAGE_VERSION
        );
        System::assert_has_event(
            Event::<Test>::StorageMigrationCompleted {
                from_version: 0,
                to_version: crate::CURRENT_STORAGE_VERSION,
            }
            .into(),
        );
        // The hook weight, the migration weight and three integrity reads.
        assert_eq!(weight.ref_time(), 50_000);
    });
}

#[test]
fn upgrade_at_the_current_version_runs_the_integrity_check_only() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        lift_governance_epoch_length();
        crate::pallet::StorageVersion::<Test>::put(crate::CURRENT_STORAGE_VERSION);

        let weight = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();

        assert_eq!(weight.ref_time(), 10_000);
        assert_eq!(
            crate::pallet::StorageVersion::<Test>::get(),
            crate::CURRENT_STORAGE_VERSION
        );
        assert!(!dcf_events().iter().any(|event| matches!(
            event,
            Event::StorageMigrationCompleted { .. } | Event::StorageMigrationFailed { .. }
        )));
    });
}

#[test]
fn upgrade_at_the_current_version_tolerates_a_failed_integrity_check() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // A held epoch length above its own range fails the integrity check,
        // and the hook at the current version records a warning only.
        crate::GovernanceConfigStorage::<Test>::mutate(|held| {
            held.epoch_length.current = held.epoch_length.max + 1
        });
        assert!(DcfPallet::validate_storage_integrity().is_err());
        crate::pallet::StorageVersion::<Test>::put(crate::CURRENT_STORAGE_VERSION);

        let weight = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();

        assert_eq!(weight.ref_time(), 10_000);
        assert_eq!(
            crate::pallet::StorageVersion::<Test>::get(),
            crate::CURRENT_STORAGE_VERSION
        );
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, Event::StorageMigrationFailed { .. })));
    });
}

#[test]
#[should_panic(expected = "DCF: Critical storage migration failure - runtime upgrade aborted")]
fn upgrade_from_an_unknown_version_aborts() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        lift_governance_epoch_length();
        crate::pallet::StorageVersion::<Test>::put(5);
        let _ = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();
    });
}

#[test]
#[should_panic(expected = "DCF: Critical storage migration failure - runtime upgrade aborted")]
fn upgrade_from_version_zero_aborts_when_the_governance_range_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // A held epoch length above its own range fails the governance check.
        crate::GovernanceConfigStorage::<Test>::mutate(|held| {
            held.epoch_length.current = held.epoch_length.max + 1
        });
        crate::pallet::StorageVersion::<Test>::put(0);
        let _ = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();
    });
}

#[test]
#[should_panic(expected = "DCF: Critical storage migration failure - runtime upgrade aborted")]
fn upgrade_from_version_zero_aborts_on_a_zero_epoch_length() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        lift_governance_epoch_length();
        crate::EpochConfigStorage::<Test>::mutate(|held| held.blocks_per_epoch = 0);
        crate::pallet::StorageVersion::<Test>::put(0);
        let _ = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();
    });
}

// ---------------------------------------------------------------------------
// Block initialisation hook: interval work
// ---------------------------------------------------------------------------

#[test]
fn a_plain_block_costs_the_hook_and_the_counter_reset() {
    new_test_ext().execute_with(|| {
        let weight = initialise(1);
        assert_eq!(weight, Weight::from_parts(20_000, 0));
    });
}

#[test]
fn the_hook_resets_the_block_counters_and_announces_it() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        assert_eq!(
            BlockOperationCounts::<Test>::get(DispatchableType::SubmitProposal),
            1
        );

        initialise(2);

        assert_eq!(
            BlockOperationCounts::<Test>::get(DispatchableType::SubmitProposal),
            0
        );
        System::assert_has_event(
            Event::<Test>::BlockRateLimitCountersReset { block_number: 2 }.into(),
        );
    });
}

#[test]
fn a_score_decay_block_charges_one_unit_per_validator() {
    new_test_ext().execute_with(|| {
        // The score decay interval is ten blocks. Three validators sit in the set.
        let weight = initialise(10);
        assert_eq!(weight, Weight::from_parts(50_000, 0));
    });
}

#[test]
fn block_zero_runs_every_interval_task() {
    new_test_ext().execute_with(|| {
        // Block zero is a multiple of every interval. Score decay and the
        // participation update each charge one unit per validator.
        let weight = <DcfPallet as Hooks<u64>>::on_initialize(0);
        assert_eq!(weight, Weight::from_parts(80_000, 0));
        assert_eq!(DcfPallet::current_epoch(), 0);
        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 2, 3]);
    });
}

#[test]
fn the_snapshot_block_freezes_scores_for_the_next_epoch() {
    new_test_ext().execute_with(|| {
        // Ten blocks per epoch put the snapshot five blocks before block ten.
        assert!(crate::EpochScoreSnapshot::<Test>::get(1).is_none());

        let weight = initialise(5);

        let snapshot = crate::EpochScoreSnapshot::<Test>::get(1).expect("the snapshot was taken");
        assert_eq!(
            snapshot.into_inner(),
            vec![(1u64, 2400u64), (2, 2800), (3, 3200)]
        );
        assert_eq!(weight, Weight::from_parts(70_000, 0));
    });
}

#[test]
fn a_second_pass_over_the_snapshot_block_keeps_the_first_snapshot() {
    new_test_ext().execute_with(|| {
        initialise(5);
        let first = crate::EpochScoreSnapshot::<Test>::get(1).expect("the snapshot was taken");

        crate::ValidatorStates::<Test>::mutate(&1, |held| {
            if let Some(state) = held.as_mut() {
                state.current.final_score = 9_999;
            }
        });
        let weight = initialise(5);

        assert_eq!(crate::EpochScoreSnapshot::<Test>::get(1), Some(first));
        assert_eq!(weight, Weight::from_parts(20_000, 0));
    });
}

#[test]
fn the_underperformance_check_ejects_a_validator_below_the_score_floor() {
    new_test_ext().execute_with(|| {
        crate::ValidatorStates::<Test>::mutate(&3, |held| {
            if let Some(state) = held.as_mut() {
                state.current.stake_score = 0;
                state.current.inference_score = 0;
                state.current.final_score = 10;
            }
        });

        // Block forty runs score decay only. The validator keeps its seat.
        initialise(40);
        assert!(DcfPallet::active_validators().contains(&3));

        // Block fifty runs the underperformance check.
        initialise(50);
        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 2]);
        System::assert_has_event(
            Event::<Test>::ValidatorEjected {
                validator: 3,
                reason: crate::EjectionReason::ScoreBelowThreshold,
            }
            .into(),
        );
    });
}

#[test]
fn the_underperformance_check_ejects_a_long_idle_validator() {
    new_test_ext().execute_with(|| {
        // Six epochs of absence pass the limit of five.
        crate::CurrentEpoch::<Test>::put(6);
        for validator in [1u64, 3] {
            crate::ValidatorStates::<Test>::mutate(&validator, |held| {
                if let Some(state) = held.as_mut() {
                    state.last_active_epoch = 6;
                }
            });
        }

        initialise(50);

        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 3]);
    });
}

#[test]
fn a_matured_scheduled_change_is_released_by_the_hook() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_assurance_class(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            95,
            10
        ));
        let payload =
            BoundedVec::<u8, ConstU32<512>>::try_from(vec![9u8; 8]).expect("eight bytes fit");
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload
        ));

        let weight = initialise(11);

        assert!(crate::ScheduledChanges::<Test>::get(0).is_none());
        System::assert_has_event(
            Event::<Test>::ScheduledChangeReleased {
                id: 0,
                class: TransactionClass::Constitutional,
                released_at: 11,
            }
            .into(),
        );
        // The hook, one release and the counter reset.
        assert_eq!(weight, Weight::from_parts(70_000, 0));
    });
}

/// Depends on the zero epoch length fix in `get_expected_author`.
///
/// A held epoch length of zero leaves the expected author lookup without a
/// divisor. The hook keeps running and charges the plain block cost.
#[test]
fn a_zero_epoch_length_leaves_the_block_hook_running() {
    new_test_ext().execute_with(|| {
        crate::EpochConfigStorage::<Test>::mutate(|held| held.blocks_per_epoch = 0);

        let weight = initialise(1);
        let _ = DcfPallet::get_expected_author(1);

        assert_eq!(weight, Weight::from_parts(20_000, 0));
        System::assert_has_event(
            Event::<Test>::BlockRateLimitCountersReset { block_number: 1 }.into(),
        );
    });
}

#[test]
fn the_finalisation_hook_leaves_storage_unchanged() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_storage_noop!(<DcfPallet as Hooks<u64>>::on_finalize(5));
        assert_storage_noop!(<DcfPallet as Hooks<u64>>::on_finalize(100));
    });
}

// ---------------------------------------------------------------------------
// Block initialisation hook: epoch boundary
// ---------------------------------------------------------------------------

#[test]
fn the_epoch_boundary_block_announces_the_boundary_and_advances_the_epoch() {
    new_test_ext().execute_with(|| {
        initialise(99);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, Event::EpochBoundaryDetected { .. })));
        assert_eq!(DcfPallet::current_epoch(), 0);

        initialise(100);

        System::assert_has_event(
            Event::<Test>::EpochBoundaryDetected {
                block_number: 100,
                epoch: 0,
                governance_mode: false,
            }
            .into(),
        );
        assert_eq!(DcfPallet::current_epoch(), 1);
        let sequence =
            crate::EpochAuthorSequences::<Test>::get(1).expect("the next sequence was cached");
        assert_eq!(sequence.len(), 10);
        assert!(crate::InvariantReports::<Test>::get(1).is_some());
        assert_eq!(DcfPallet::get_metrics_last_updated(), 100);
    });
}

#[test]
fn the_epoch_boundary_event_carries_the_governance_mode() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_governance_mode(RuntimeOrigin::root(), true));

        initialise(100);

        System::assert_has_event(
            Event::<Test>::EpochBoundaryDetected {
                block_number: 100,
                epoch: 0,
                governance_mode: true,
            }
            .into(),
        );
    });
}

#[test]
fn the_epoch_boundary_lowers_the_standing_of_an_idle_validator() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(10);
        let record = ServiceRecord {
            epochs_served: 90,
            last_served_epoch: 2,
            joined_epoch: 0,
        };
        crate::ServiceRecords::<Test>::insert(77u64, record);
        let base = accumulated(&record, DcfPallet::ramp_epochs());
        let expected = decayed(base, 8, DcfPallet::decay_settings());
        assert!(expected < base);

        initialise(100);

        assert_eq!(crate::StandingShares::<Test>::get(77u64), Some(expected));
        System::assert_has_event(
            Event::<Test>::StandingDecayed {
                validator: 77,
                idle_epochs: 8,
                standing: expected,
            }
            .into(),
        );
    });
}

#[test]
fn a_validator_inside_the_grace_span_keeps_its_standing() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(10);
        crate::ServiceRecords::<Test>::insert(
            77u64,
            ServiceRecord {
                epochs_served: 90,
                last_served_epoch: 9,
                joined_epoch: 0,
            },
        );

        initialise(100);

        assert_eq!(crate::StandingShares::<Test>::get(77u64), None);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, Event::StandingDecayed { .. })));
    });
}

#[test]
fn a_standing_already_at_the_decayed_share_raises_no_event() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(10);
        let record = ServiceRecord {
            epochs_served: 90,
            last_served_epoch: 2,
            joined_epoch: 0,
        };
        crate::ServiceRecords::<Test>::insert(77u64, record);
        let expected = decayed(
            accumulated(&record, DcfPallet::ramp_epochs()),
            8,
            DcfPallet::decay_settings(),
        );
        crate::StandingShares::<Test>::insert(77u64, expected);

        initialise(100);

        assert_eq!(crate::StandingShares::<Test>::get(77u64), Some(expected));
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, Event::StandingDecayed { .. })));
    });
}

#[test]
fn standing_decay_waits_for_the_epoch_boundary_block() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(10);
        crate::ServiceRecords::<Test>::insert(
            77u64,
            ServiceRecord {
                epochs_served: 90,
                last_served_epoch: 2,
                joined_epoch: 0,
            },
        );

        initialise(99);

        assert_eq!(crate::StandingShares::<Test>::get(77u64), None);
    });
}

// ---------------------------------------------------------------------------
// Rate limits on proposals
// ---------------------------------------------------------------------------

#[test]
fn the_sixth_proposal_in_one_block_hits_the_block_limit() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        for who in 10..15u64 {
            assert_ok!(propose(who));
        }
        assert_eq!(
            BlockOperationCounts::<Test>::get(DispatchableType::SubmitProposal),
            5
        );

        assert_noop!(propose(15), Error::<Test>::PerBlockRateLimitExceeded);
    });
}

#[test]
fn the_block_limit_lifts_once_the_hook_resets_the_counter() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        for who in 10..15u64 {
            assert_ok!(propose(who));
        }
        assert_noop!(propose(15), Error::<Test>::PerBlockRateLimitExceeded);

        initialise(2);

        assert_ok!(propose(15));
        assert_eq!(DcfPallet::next_proposal_id(), 6);
    });
}

#[test]
fn the_fourth_proposal_inside_the_window_hits_the_account_limit() {
    new_test_ext().execute_with(|| {
        for block in [1u64, 21, 41] {
            System::set_block_number(block);
            assert_ok!(propose(10));
        }

        System::set_block_number(61);
        assert_noop!(propose(10), Error::<Test>::PerAccountRateLimitExceeded);
    });
}

#[test]
fn the_account_window_slides_and_prunes_old_entries() {
    new_test_ext().execute_with(|| {
        for block in [1u64, 21, 41] {
            System::set_block_number(block);
            assert_ok!(propose(10));
        }

        // At block 102 the window opens at block 2, so block 1 falls out.
        System::set_block_number(102);
        assert_ok!(propose(10));

        assert_eq!(
            AccountOperationHistory::<Test>::get(10u64, DispatchableType::SubmitProposal)
                .into_inner(),
            vec![21, 41, 102]
        );
        assert_eq!(
            LastOperationBlock::<Test>::get(10u64, DispatchableType::SubmitProposal),
            Some(102)
        );
    });
}

#[test]
fn a_proposal_inside_the_minimum_interval_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));

        System::set_block_number(20);
        assert_noop!(propose(10), Error::<Test>::MinimumIntervalViolation);

        // Twenty blocks after the first proposal the interval has passed.
        System::set_block_number(21);
        assert_ok!(propose(10));
    });
}

#[test]
fn a_proposal_above_the_weight_bound_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        // A proposal weighs 20,000 units in the mock runtime.
        RateLimitConfigStorage::<Test>::mutate(|held| held.max_proposal_processing_weight = 19_999);
        assert_noop!(propose(10), Error::<Test>::WeightLimitExceeded);

        RateLimitConfigStorage::<Test>::mutate(|held| held.max_proposal_processing_weight = 20_000);
        assert_ok!(propose(10));
    });
}

#[test]
fn a_refused_proposal_records_no_operation() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        System::set_block_number(5);
        assert_noop!(propose(10), Error::<Test>::MinimumIntervalViolation);

        assert_eq!(
            AccountOperationHistory::<Test>::get(10u64, DispatchableType::SubmitProposal)
                .into_inner(),
            vec![1]
        );
        assert_eq!(
            LastOperationBlock::<Test>::get(10u64, DispatchableType::SubmitProposal),
            Some(1)
        );
    });
}

// ---------------------------------------------------------------------------
// Rate limits on votes
// ---------------------------------------------------------------------------

#[test]
fn the_vote_block_limit_refuses_a_second_vote_and_lifts_next_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        RateLimitConfigStorage::<Test>::mutate(|held| held.max_votes_per_block = 1);

        assert_ok!(vote(1, 0));
        assert_noop!(vote(2, 0), Error::<Test>::PerBlockRateLimitExceeded);

        initialise(2);
        assert_ok!(vote(2, 0));

        let proposal = crate::Proposals::<Test>::get(0).expect("proposal zero exists");
        assert_eq!(proposal.votes_for, 2);
        assert!(matches!(proposal.status, crate::ProposalStatus::Approved));
    });
}

#[test]
fn a_second_vote_in_one_block_breaks_the_vote_interval() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        assert_ok!(propose(11));

        assert_ok!(vote(1, 0));
        assert_noop!(vote(1, 1), Error::<Test>::MinimumIntervalViolation);

        System::set_block_number(2);
        assert_ok!(vote(1, 1));
    });
}

#[test]
fn a_voter_with_a_full_window_hits_the_account_limit() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        let full = BoundedVec::<u32, ConstU32<100>>::try_from(vec![1u32; 100])
            .expect("one hundred entries fit");
        AccountOperationHistory::<Test>::insert(1u64, DispatchableType::VoteProposal, full);

        assert_noop!(vote(1, 0), Error::<Test>::PerAccountRateLimitExceeded);
    });
}

#[test]
fn a_vote_above_the_weight_bound_is_refused() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        // A vote weighs 15,000 units in the mock runtime.
        RateLimitConfigStorage::<Test>::mutate(|held| held.max_proposal_processing_weight = 14_999);
        assert_noop!(vote(1, 0), Error::<Test>::WeightLimitExceeded);

        RateLimitConfigStorage::<Test>::mutate(|held| held.max_proposal_processing_weight = 15_000);
        assert_ok!(vote(1, 0));
    });
}

#[test]
fn a_full_vote_history_drops_its_oldest_entry_and_prunes_the_window() {
    new_test_ext().execute_with(|| {
        System::set_block_number(200);
        assert_ok!(propose(10));
        // One hundred entries at block zero fill the history. All sit outside
        // the vote window of one hundred blocks.
        let stale = BoundedVec::<u32, ConstU32<100>>::try_from(vec![0u32; 100])
            .expect("one hundred entries fit");
        AccountOperationHistory::<Test>::insert(1u64, DispatchableType::VoteProposal, stale);

        assert_ok!(vote(1, 0));

        assert_eq!(
            AccountOperationHistory::<Test>::get(1u64, DispatchableType::VoteProposal).into_inner(),
            vec![200]
        );
        assert_eq!(
            LastOperationBlock::<Test>::get(1u64, DispatchableType::VoteProposal),
            Some(200)
        );
    });
}

// ---------------------------------------------------------------------------
// Rate limit rules under root authority
// ---------------------------------------------------------------------------

/// Store one altered rule set and hold that the root update refuses it.
///
/// The root call changes three per-block limits and keeps every other rule as
/// stored. A broken stored rule therefore fails the validation on update.
fn assert_stored_rule_refused(alter: impl FnOnce(&mut RateLimitConfig)) {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let mut held = RateLimitConfigStorage::<Test>::get();
        alter(&mut held);
        RateLimitConfigStorage::<Test>::put(held);
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 3, 3),
            Error::<Test>::InvalidRateLimitConfig
        );
    });
}

#[test]
fn root_updates_the_block_limits_and_announces_them() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            10,
            5,
            4
        ));

        let held = DcfPallet::rate_limit_config();
        assert_eq!(held.max_proposals_per_block, 10);
        assert_eq!(held.max_joins_per_block, 5);
        assert_eq!(held.max_leaves_per_block, 4);
        assert_eq!(
            held.max_votes_per_block,
            RateLimitConfig::default().max_votes_per_block
        );
        System::assert_has_event(
            Event::<Test>::RateLimitConfigUpdated {
                max_proposals_per_block: 10,
                max_joins_per_block: 5,
                max_leaves_per_block: 4,
            }
            .into(),
        );
    });
}

#[test]
fn a_signed_account_cannot_change_the_rate_limits() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::signed(1), 10, 5, 5),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn the_proposal_block_limit_must_sit_between_one_and_one_hundred() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 0, 3, 3),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 101, 3, 3),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            100,
            3,
            3
        ));
    });
}

#[test]
fn the_join_block_limit_must_sit_between_one_and_fifty() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 0, 3),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 51, 3),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            5,
            50,
            3
        ));
    });
}

#[test]
fn the_leave_block_limit_must_sit_between_one_and_fifty() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 3, 0),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_noop!(
            DcfPallet::update_rate_limit_config(RuntimeOrigin::root(), 5, 3, 51),
            Error::<Test>::InvalidRateLimitConfig
        );
        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            5,
            3,
            50
        ));
    });
}

#[test]
fn a_stored_vote_block_limit_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.max_votes_per_block = 0);
    assert_stored_rule_refused(|held| held.max_votes_per_block = 1001);
}

#[test]
fn a_stored_proposal_account_limit_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.max_proposals_per_account = 0);
    assert_stored_rule_refused(|held| held.max_proposals_per_account = 101);
}

#[test]
fn a_stored_status_change_limit_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.max_status_changes_per_account = 0);
    assert_stored_rule_refused(|held| held.max_status_changes_per_account = 21);
}

#[test]
fn a_stored_proposal_window_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.proposal_rate_window = 0);
    assert_stored_rule_refused(|held| held.proposal_rate_window = 10_001);
}

#[test]
fn a_stored_status_change_window_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.status_change_rate_window = 0);
    assert_stored_rule_refused(|held| held.status_change_rate_window = 100_001);
}

#[test]
fn stored_minimum_intervals_above_their_ceilings_are_refused() {
    assert_stored_rule_refused(|held| held.min_validator_status_interval = 10_001);
    assert_stored_rule_refused(|held| held.min_proposal_interval = 1001);
    assert_stored_rule_refused(|held| held.min_vote_interval = 101);
}

#[test]
fn stored_zero_weight_bounds_are_refused() {
    assert_stored_rule_refused(|held| held.max_validator_iteration_weight = 0);
    assert_stored_rule_refused(|held| held.max_proposal_processing_weight = 0);
}

#[test]
fn a_stored_loop_ceiling_outside_its_range_is_refused() {
    assert_stored_rule_refused(|held| held.max_loop_iterations = 0);
    assert_stored_rule_refused(|held| held.max_loop_iterations = 10_001);
}

#[test]
fn every_rule_at_its_edge_is_accepted() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let edges = RateLimitConfig {
            max_proposals_per_block: 1,
            max_joins_per_block: 1,
            max_leaves_per_block: 1,
            max_votes_per_block: 1000,
            max_proposals_per_account: 100,
            proposal_rate_window: 10_000,
            max_status_changes_per_account: 20,
            status_change_rate_window: 100_000,
            min_validator_status_interval: 10_000,
            min_proposal_interval: 1000,
            min_vote_interval: 100,
            max_validator_iteration_weight: 1,
            max_proposal_processing_weight: 1,
            max_loop_iterations: 10_000,
        };
        RateLimitConfigStorage::<Test>::put(edges.clone());

        assert_ok!(DcfPallet::update_rate_limit_config(
            RuntimeOrigin::root(),
            100,
            50,
            50
        ));

        let held = DcfPallet::rate_limit_config();
        assert_eq!(held.max_proposals_per_block, 100);
        assert_eq!(held.max_joins_per_block, 50);
        assert_eq!(held.max_leaves_per_block, 50);
        assert_eq!(held.max_votes_per_block, edges.max_votes_per_block);
        assert_eq!(held.max_loop_iterations, edges.max_loop_iterations);
        assert_eq!(held.min_vote_interval, edges.min_vote_interval);
    });
}

// ---------------------------------------------------------------------------
// Metrics
// ---------------------------------------------------------------------------

#[test]
fn genesis_records_the_first_metrics() {
    new_test_ext().execute_with(|| {
        let metrics = DcfPallet::get_system_metrics();
        assert_eq!(metrics.active_validator_count, 3);
        assert_eq!(metrics.total_validator_count, 3);
        assert_eq!(metrics.validator_set_capacity, 100);
        assert_eq!(metrics.total_reserved, 3000);
        assert_eq!(metrics.average_stake, 1000);
        assert_eq!(metrics.blocks_in_current_epoch, 0);
        assert_eq!(metrics.invariant_health, crate::InvariantHealth::Healthy);
        assert_eq!(DcfPallet::get_metrics_last_updated(), 0);

        let indicators = DcfPallet::get_performance_indicators();
        assert_eq!(indicators.top_performer_score, 3200);
        assert_eq!(indicators.lowest_performer_score, 2400);
    });
}

#[test]
fn a_metrics_update_reads_the_validator_set_and_the_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 4000u64);
        crate::ValidatorTrustScores::<Test>::insert(2u64, 5000u64);
        crate::ValidatorTrustScores::<Test>::insert(3u64, 6000u64);

        DcfPallet::update_system_metrics();

        let metrics = DcfPallet::get_system_metrics();
        assert_eq!(metrics.active_validator_count, 3);
        assert_eq!(metrics.total_validator_count, 3);
        assert_eq!(metrics.validator_set_capacity, 100);
        assert_eq!(metrics.total_reserved, 3000);
        assert_eq!(metrics.average_stake, 1000);
        assert_eq!(metrics.total_slashed, 0);
        assert_eq!(metrics.total_rewards, 0);
        assert_eq!(metrics.current_epoch, 0);
        assert_eq!(metrics.blocks_in_current_epoch, 7);
        assert_eq!(metrics.epoch_progress_percentage, 70);
        assert_eq!(metrics.average_trust_score, 5000);
        assert_eq!(metrics.invariant_health, crate::InvariantHealth::Healthy);
        assert_eq!(metrics.missed_blocks_current_epoch, 0);
        assert_eq!(DcfPallet::get_metrics_last_updated(), 7);
    });
}

#[test]
fn a_metrics_update_computes_the_score_spread() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        DcfPallet::update_system_metrics();

        // Genesis scores 2400, 2800 and 3200 sit 400 either side of 2800.
        let indicators = DcfPallet::get_performance_indicators();
        assert_eq!(indicators.top_performer_score, 3200);
        assert_eq!(indicators.lowest_performer_score, 2400);
        assert_eq!(indicators.score_distribution_variance, 106_666);
        assert_eq!(indicators.consensus_participation_rate, 0);
        assert_eq!(indicators.governance_activity_level, 0);
        assert_eq!(indicators.average_block_time, 6000);
        assert_eq!(indicators.block_production_rate, 10);
        assert_eq!(indicators.epoch_transition_efficiency, 100);
        assert_eq!(indicators.validator_churn_rate, 0);
    });
}

#[test]
fn a_metrics_update_counts_authoring_validators_and_proposals() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(propose(10));
        crate::ValidatorStates::<Test>::mutate(&1, |held| {
            if let Some(state) = held.as_mut() {
                state.current.authored_blocks = 4;
            }
        });

        DcfPallet::update_system_metrics();

        let indicators = DcfPallet::get_performance_indicators();
        assert_eq!(indicators.consensus_participation_rate, 33);
        assert_eq!(indicators.governance_activity_level, 1);
    });
}

#[test]
fn missed_blocks_from_the_hook_reach_the_metrics() {
    new_test_ext().execute_with(|| {
        // No block carries an author digest, so every expected author misses.
        for block in 1..=4u64 {
            initialise(block);
        }

        DcfPallet::update_system_metrics();

        let metrics = DcfPallet::get_system_metrics();
        assert_eq!(metrics.missed_blocks_current_epoch, 4);
        assert_eq!(metrics.blocks_in_current_epoch, 4);
        assert_eq!(metrics.epoch_progress_percentage, 40);
    });
}

#[test]
fn a_zero_epoch_length_reports_no_epoch_progress() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        crate::EpochConfigStorage::<Test>::mutate(|held| held.blocks_per_epoch = 0);

        DcfPallet::update_system_metrics();

        let metrics = DcfPallet::get_system_metrics();
        assert_eq!(metrics.blocks_in_current_epoch, 0);
        assert_eq!(metrics.epoch_progress_percentage, 0);
    });
}

#[test]
fn an_understaked_validator_marks_the_metrics_critical() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert!(!DcfPallet::has_invariant_violations());

        let left = <Balances as ReservableCurrency<u64>>::unreserve(&1, 1000);
        assert_eq!(left, 0);
        assert!(DcfPallet::has_invariant_violations());

        DcfPallet::update_system_metrics();
        assert_eq!(
            DcfPallet::get_system_metrics().invariant_health,
            crate::InvariantHealth::Critical
        );
    });
}

#[test]
fn an_invariant_report_counts_only_inside_the_last_three_epochs() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(10);
        let violations = DcfPallet::check_temporal_invariants(5, 3);
        assert_eq!(violations.len(), 1);
        let report = crate::InvariantReport::<Test> {
            epoch: 6,
            block_number: 1,
            violations: BoundedVec::truncate_from(violations),
            severity: crate::InvariantSeverity::Medium,
            timestamp: 0,
        };

        // Epoch six sits outside the window from epoch seven to epoch ten.
        crate::InvariantReports::<Test>::insert(6, report.clone());
        assert!(!DcfPallet::has_invariant_violations());

        crate::InvariantReports::<Test>::insert(7, report);
        assert!(DcfPallet::has_invariant_violations());
    });
}

#[test]
fn an_invariant_report_without_violations_keeps_the_chain_healthy() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(2);
        crate::InvariantReports::<Test>::insert(
            2,
            crate::InvariantReport::<Test> {
                epoch: 2,
                block_number: 20,
                violations: BoundedVec::default(),
                severity: crate::InvariantSeverity::Low,
                timestamp: 0,
            },
        );
        assert!(!DcfPallet::has_invariant_violations());
    });
}

#[test]
fn an_empty_validator_set_yields_zero_metrics() {
    let empty = genesis_of(vec![], vec![], vec![], vec![], 100, false);
    new_test_ext_with_genesis(empty).execute_with(|| {
        let metrics = DcfPallet::get_system_metrics();
        assert_eq!(metrics.active_validator_count, 0);
        assert_eq!(metrics.total_validator_count, 0);
        assert_eq!(metrics.total_reserved, 0);
        assert_eq!(metrics.average_stake, 0);
        assert_eq!(metrics.average_trust_score, 0);

        let indicators = DcfPallet::get_performance_indicators();
        assert_eq!(indicators.top_performer_score, 0);
        assert_eq!(indicators.lowest_performer_score, 0);
        assert_eq!(indicators.score_distribution_variance, 0);
        assert_eq!(indicators.consensus_participation_rate, 0);

        // An empty set receives no author sequence for epoch zero.
        assert!(crate::EpochAuthorSequences::<Test>::get(0).is_none());
        assert!(DcfPallet::active_validators().is_empty());
    });
}

// ---------------------------------------------------------------------------
// Activity reports, uptime and the timestamp source
// ---------------------------------------------------------------------------

#[test]
fn an_activity_report_reads_the_uptime_from_the_earlier_sighting() {
    ext_with_offchain().execute_with(|| {
        // The first report at block 20 follows no earlier sighting, so the
        // uptime reads zero.
        System::set_block_number(20);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            3,
            1
        ));
        let history = DcfPallet::validator_performance_history(&1);
        let record = history.last().expect("one record was written");
        assert_eq!(record.uptime_percentage, 0);
        assert_eq!(record.blocks_authored, 3);
        assert_eq!(record.blocks_missed, 1);
        assert_eq!(DcfPallet::validator_last_seen(&1), 20);

        // The second report at block 30 follows a sighting at block 20. The
        // validator joined at block zero, so 20 of 30 blocks read as up.
        System::set_block_number(30);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            1,
            0
        ));
        let history = DcfPallet::validator_performance_history(&1);
        let record = history.last().expect("a second record was written");
        assert_eq!(record.uptime_percentage, 6_666);
        assert_eq!(DcfPallet::validator_last_seen(&1), 30);
        System::assert_has_event(
            Event::<Test>::ValidatorActivityUpdated {
                validator: 1,
                blocks_authored: 1,
                blocks_missed: 0,
                uptime_percentage: 6_666,
            }
            .into(),
        );
    });
}

#[test]
fn a_validator_joining_after_the_current_block_reports_full_uptime() {
    ext_with_offchain().execute_with(|| {
        System::set_block_number(20);
        crate::ValidatorJoinTime::<Test>::insert(&2, 50);

        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            2,
            0,
            0
        ));

        let history = DcfPallet::validator_performance_history(&2);
        assert_eq!(
            history
                .last()
                .expect("one record was written")
                .uptime_percentage,
            10_000
        );
    });
}

/// Defect check. The activity report runs inside block execution.
///
/// Block execution carries no offchain worker extension. The timestamp source
/// of the metrics block reads the offchain clock, so the call stops the block.
/// The report succeeds once the timestamp source reads on-chain state.
#[test]
fn an_activity_report_succeeds_inside_block_execution() {
    new_test_ext().execute_with(|| {
        System::set_block_number(20);
        pallet_timestamp::Now::<Test>::put(120_000u64);

        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            3,
            1
        ));
        assert_eq!(DcfPallet::validator_performance_history(&1).len(), 1);
    });
}

// ---------------------------------------------------------------------------
// Genesis build: accepted records
// ---------------------------------------------------------------------------

#[test]
fn genesis_seats_validators_with_stakes_scores_and_names() {
    let record = genesis_of(
        vec![10, 11, 12],
        vec![1000, 1500, 2000],
        vec![50, 60, 70],
        vec![Some(b"alpha".to_vec()), None, Some(b"gamma".to_vec())],
        20,
        true,
    );
    new_test_ext_with_genesis(record).execute_with(|| {
        assert_eq!(DcfPallet::active_validators().to_vec(), vec![10, 11, 12]);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(12u64), 2000);
        assert_eq!(Balances::reserved_balance(12u64), 2000);

        let state = DcfPallet::validator_states(&12).expect("validator 12 holds a state");
        assert_eq!(state.current.stake_score, 2);
        assert_eq!(state.current.inference_score, 70);
        // (2 x 6000 + 70 x 4000) / 10000 rounds down to 29.
        assert_eq!(state.current.final_score, 29);
        assert_eq!(state.trust_score, 5000);
        assert_eq!(state.participation_rate, 100);
        assert_eq!(state.uptime, 1);
        assert_eq!(state.last_active_block, 1);
        assert_eq!(state.history.len(), 1);
        assert_eq!(
            state.name.map(|name| name.into_inner()),
            Some(b"gamma".to_vec())
        );

        let unnamed = DcfPallet::validator_states(&11).expect("validator 11 holds a state");
        assert_eq!(unnamed.name, None);
    });
}

#[test]
fn genesis_fills_empty_stakes_scores_and_names_with_defaults() {
    let mut record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 20, false);
    record.current_epoch = 7;
    new_test_ext_with_genesis(record).execute_with(|| {
        assert_eq!(DcfPallet::current_epoch(), 7);
        for validator in [10u64, 11, 12] {
            assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(validator), 1000);
            let state = DcfPallet::validator_states(&validator).expect("a state exists");
            assert_eq!(state.current.inference_score, 0);
            assert_eq!(state.current.final_score, 0);
            assert_eq!(state.current.epoch, 7);
            assert_eq!(state.last_active_epoch, 7);
            assert_eq!(state.history[0].epoch, 7);
            assert_eq!(state.name, None);
        }
    });
}

#[test]
fn genesis_truncates_an_overlong_name_outside_strict_mode() {
    let long_name = vec![b'n'; 40];
    let record = genesis_of(
        vec![10, 11, 12],
        vec![],
        vec![],
        vec![Some(long_name.clone()), None, None],
        20,
        false,
    );
    new_test_ext_with_genesis(record).execute_with(|| {
        let state = DcfPallet::validator_states(&10).expect("validator 10 holds a state");
        let held = state.name.expect("the name was kept").into_inner();
        assert_eq!(held.len(), 32);
        assert_eq!(held, long_name[..32].to_vec());
    });
}

#[test]
fn genesis_writes_the_settings_every_engine_reads() {
    let mut record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 20, false);
    record.epoch_config.max_validators = 50;
    new_test_ext_with_genesis(record).execute_with(|| {
        let epoch = DcfPallet::epoch_config();
        assert_eq!(epoch.blocks_per_epoch, 20);
        assert_eq!(epoch.max_validators, 50);
        assert_eq!(crate::PosWeight::<Test>::get(), 6000);
        assert_eq!(crate::PoiWeight::<Test>::get(), 4000);
        assert_eq!(DcfPallet::rate_limit_config(), RateLimitConfig::default());

        let governance = crate::GovernanceConfigStorage::<Test>::get();
        assert_eq!(governance.epoch_length.current, 20);
        assert_eq!(governance.max_validators.current, 50);
        assert_eq!(governance.min_stake.current, 1000);
        assert_eq!(governance.pos_weight.current, 6000);
        assert_eq!(governance.poi_weight.current, 4000);

        let trust = crate::TrustScoreConfigStorage::<Test>::get();
        assert_eq!(trust.uptime_weight, 40);
        assert_eq!(trust.inference_weight, 35);
        assert_eq!(trust.slashing_weight, 25);
        assert_eq!(trust.min_trust_score, 1000);
        assert_eq!(trust.max_trust_score, 10_000);

        let bounds = crate::TrustScoreBounds::<Test>::get();
        assert_eq!(bounds.max_growth_rate, 500);
        assert_eq!(bounds.max_decay_rate, 200);
        assert_eq!(bounds.stability_factor, 8000);
        assert_eq!(bounds.last_updated_epoch, 0);
    });
}

/// Depends on the genesis Trust Score fix.
///
/// Genesis fills the Trust Score of every seated validator. Every invariant
/// therefore holds from the first block.
#[test]
fn genesis_leaves_every_invariant_satisfied() {
    new_test_ext().execute_with(|| {
        for validator in [1u64, 2, 3] {
            let held = crate::ValidatorTrustScores::<Test>::get(validator);
            assert!(
                (1000..=10_000).contains(&held),
                "The Trust Score of genesis validator {} sits outside its bounds.",
                validator
            );
        }
        assert_eq!(DcfPallet::validate_current_invariants(), Ok(()));
    });
}

#[test]
fn genesis_draws_one_author_per_slot_of_epoch_zero() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 20, false);
    new_test_ext_with_genesis(record).execute_with(|| {
        let sequence =
            crate::EpochAuthorSequences::<Test>::get(0).expect("epoch zero holds a sequence");
        assert_eq!(sequence.len(), 20);
        assert!(sequence
            .iter()
            .all(|author| [10u64, 11, 12].contains(author)));
    });
}

#[test]
fn genesis_skips_a_validator_without_the_balance_for_its_stake() {
    new_test_ext().execute_with(|| {
        // Account 30 holds no balance in the mock runtime.
        let record = genesis_of(vec![10, 30], vec![1000, 1000], vec![], vec![], 20, false);
        record.build();

        assert_eq!(Balances::reserved_balance(10u64), 1000);
        assert!(DcfPallet::validator_states(&10).is_some());
        assert_eq!(Balances::reserved_balance(30u64), 0);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(30u64), 0);
        assert!(DcfPallet::validator_states(&30).is_none());
    });
}

/// Defect check. A validator genesis skips holds no seat.
///
/// Genesis skips a validator whose balance falls short of its stake. The
/// skipped validator reserves nothing and holds no state, so it stays outside
/// the active set and the validator set.
#[test]
fn a_skipped_genesis_validator_stays_outside_the_active_set() {
    new_test_ext().execute_with(|| {
        let record = genesis_of(vec![10, 30], vec![1000, 1000], vec![], vec![], 20, false);
        record.build();

        assert!(DcfPallet::active_validators().contains(&10));
        assert!(!DcfPallet::active_validators().contains(&30));
        assert!(!DcfPallet::validator_set().contains(&30));
    });
}

#[test]
fn genesis_skips_a_validator_whose_stake_cannot_be_reserved() {
    new_test_ext().execute_with(|| {
        // A lock over the whole balance blocks the reserve.
        <Balances as LockableCurrency<u64>>::set_lock(
            *b"dcf/lock",
            &4,
            10_000,
            WithdrawReasons::all(),
        );
        let record = genesis_of(vec![4], vec![1000], vec![], vec![], 20, false);
        record.build();

        assert_eq!(Balances::reserved_balance(4u64), 0);
        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(4u64), 0);
        assert!(DcfPallet::validator_states(&4).is_none());
    });
}

// ---------------------------------------------------------------------------
// Genesis build: refused records
// ---------------------------------------------------------------------------

/// Depends on the genesis zero epoch length fix.
///
/// The expected panic text reuses the wording both validation routines
/// already carry. Align it with the fix if the fix words it differently.
#[test]
#[should_panic(expected = "Epoch length cannot be zero")]
fn genesis_refuses_a_zero_epoch_length_outside_strict_mode() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 0, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "Epoch length 9 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
)]
fn genesis_refuses_an_epoch_below_the_governance_range() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 9, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "Epoch length 9 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
)]
fn strict_genesis_refuses_an_epoch_below_the_governance_range() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 9, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "Epoch length 14401 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
)]
fn genesis_refuses_an_epoch_above_the_governance_range() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 14_401, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "Epoch length 14401 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
)]
fn strict_genesis_refuses_an_epoch_above_the_governance_range() {
    let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], 14_401, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
fn genesis_admits_both_ends_of_the_governance_range() {
    for length in [10u32, 14_400] {
        let record = genesis_of(vec![10, 11, 12], vec![], vec![], vec![], length, false);
        new_test_ext_with_genesis(record).execute_with(|| {
            assert_eq!(
                crate::EpochConfigStorage::<Test>::get().blocks_per_epoch,
                length
            );
            assert_eq!(
                crate::GovernanceConfigStorage::<Test>::get()
                    .epoch_length
                    .current,
                length
            );
        });
    }
}

#[test]
fn genesis_writes_the_current_storage_version() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            crate::pallet::StorageVersion::<Test>::get(),
            crate::CURRENT_STORAGE_VERSION
        );
    });
}

#[test]
fn a_new_chain_runs_no_migration_on_its_first_upgrade() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let _ = <DcfPallet as Hooks<u64>>::on_runtime_upgrade();
        assert_eq!(
            crate::pallet::StorageVersion::<Test>::get(),
            crate::CURRENT_STORAGE_VERSION
        );
        assert!(!dcf_events().iter().any(|event| matches!(
            event,
            Event::StorageMigrationCompleted { .. } | Event::StorageMigrationFailed { .. }
        )));
    });
}

#[test]
fn the_genesis_governance_record_passes_its_own_check() {
    new_test_ext().execute_with(|| {
        let held = crate::GovernanceConfigStorage::<Test>::get();
        assert!(DcfPallet::validate_governance_config(&held).is_ok());
        assert!(DcfPallet::validate_storage_integrity().is_ok());
        // The range comes from the runtime constants of the mock chain.
        assert_eq!(held.epoch_length.min, 10);
        assert_eq!(held.epoch_length.max, 14_400);
        assert_eq!(held.epoch_length.current, 10);
    });
}

#[test]
#[should_panic(expected = "Genesis validators (101) exceed MaxValidators (100)")]
fn genesis_refuses_more_validators_than_the_ceiling() {
    let record = genesis_of((100u64..=200).collect(), vec![], vec![], vec![], 20, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Genesis validators (101) exceed MaxValidators (100)")]
fn strict_genesis_refuses_more_validators_than_the_ceiling() {
    let record = genesis_of((100u64..=200).collect(), vec![], vec![], vec![], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Duplicate validator found in genesis config: 10")]
fn genesis_refuses_a_duplicate_validator() {
    new_test_ext().execute_with(|| {
        let record = genesis_of(vec![10, 11, 10], vec![], vec![], vec![], 20, false);
        record.build();
    });
}

#[test]
#[should_panic(
    expected = "validator_stakes length (1) must match validators length (2) or be empty"
)]
fn genesis_refuses_a_short_stake_list() {
    let record = genesis_of(vec![10, 11], vec![1000], vec![], vec![], 20, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Genesis validator 1 stake (999) below MinStake (1000)")]
fn genesis_refuses_a_stake_below_the_minimum() {
    let record = genesis_of(vec![10, 11], vec![1000, 999], vec![], vec![], 20, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "validator_scores length (1) must match validators length (2) or be empty"
)]
fn genesis_refuses_a_short_score_list() {
    let record = genesis_of(vec![10, 11], vec![], vec![50], vec![], 20, false);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(
    expected = "validator_names length (1) must match validators length (2) or be empty"
)]
fn genesis_refuses_a_short_name_list() {
    let record = genesis_of(
        vec![10, 11],
        vec![],
        vec![],
        vec![Some(b"alpha".to_vec())],
        20,
        false,
    );
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Genesis validator 30 has insufficient balance (0) for stake (1000)")]
fn strict_genesis_refuses_a_validator_without_the_balance_for_its_stake() {
    new_test_ext().execute_with(|| {
        let record = genesis_of(vec![30], vec![1000], vec![], vec![], 20, true);
        record.build();
    });
}

#[test]
#[should_panic(expected = "Failed to reserve stake for genesis validator 4")]
fn strict_genesis_refuses_a_stake_that_cannot_be_reserved() {
    new_test_ext().execute_with(|| {
        <Balances as LockableCurrency<u64>>::set_lock(
            *b"dcf/lock",
            &4,
            10_000,
            WithdrawReasons::all(),
        );
        let record = genesis_of(vec![4], vec![1000], vec![], vec![], 20, true);
        record.build();
    });
}

#[test]
#[should_panic(expected = "Genesis must have at least one validator")]
fn strict_genesis_refuses_an_empty_validator_list() {
    let record = genesis_of(vec![], vec![], vec![], vec![], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "validator_scores length must match validators length or be empty")]
fn strict_genesis_refuses_a_short_score_list() {
    let record = genesis_of(vec![10, 11], vec![], vec![50], vec![], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "validator_stakes length must match validators length or be empty")]
fn strict_genesis_refuses_a_short_stake_list() {
    let record = genesis_of(vec![10, 11], vec![1000], vec![], vec![], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "validator_names length must match validators length or be empty")]
fn strict_genesis_refuses_a_short_name_list() {
    let record = genesis_of(vec![10, 11], vec![], vec![], vec![None], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Genesis validator 0 stake below minimum: 500 < 1000")]
fn strict_genesis_refuses_a_stake_below_the_minimum() {
    let record = genesis_of(vec![10], vec![500], vec![], vec![], 20, true);
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Genesis validator 0 name too long: 33 > 32")]
fn strict_genesis_refuses_an_overlong_name() {
    let record = genesis_of(
        vec![10],
        vec![],
        vec![],
        vec![Some(vec![b'n'; 33])],
        20,
        true,
    );
    let _ = new_test_ext_with_genesis(record);
}

#[test]
#[should_panic(expected = "Epoch length cannot be zero")]
fn strict_genesis_refuses_a_zero_epoch_length() {
    let record = genesis_of(vec![10], vec![], vec![], vec![], 0, true);
    let _ = new_test_ext_with_genesis(record);
}

// ---------------------------------------------------------------------------
// Genesis validation without a build
// ---------------------------------------------------------------------------

#[test]
fn a_sound_genesis_record_passes_comprehensive_validation() {
    new_test_ext().execute_with(|| {
        assert_eq!(sound_genesis().validate_genesis_comprehensive(), Ok(()));
        assert_eq!(DcfPallet::validate_genesis_config(&sound_genesis()), Ok(()));
    });
}

#[test]
fn comprehensive_validation_refuses_an_empty_validator_list() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validators = vec![];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis settings must contain at least one validator"
        );
    });
}

#[test]
fn comprehensive_validation_names_the_index_of_a_duplicate() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validators = vec![10, 11, 10];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Duplicate validator found at index 2: 10"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_more_validators_than_the_ceiling() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validators = (100u64..=200).collect();
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis validator count (101) exceeds MaxValidators (100)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_fewer_validators_than_the_floor() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validators = vec![10, 11];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis validator count (2) below MinActiveValidators (3)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_short_stake_list() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_stakes = vec![1000, 1000];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "validator_stakes length (2) must match validators length (3) or be empty"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_stake_below_the_minimum() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_stakes = vec![1000, 1000, 999];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis validator 2 stake (999) below MinStake (1000)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_short_score_list() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_scores = vec![10, 20];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "validator_scores length (2) must match validators length (3) or be empty"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_score_above_the_ceiling() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_scores = vec![100, 101, 0];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis validator 1 score (101) exceeds MaxValidatorScore (100)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_short_name_list() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_names = vec![None];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "validator_names length (1) must match validators length (3) or be empty"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_an_overlong_name() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_names = vec![Some(vec![b'n'; 33]), None, Some(b"ok".to_vec())];
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Genesis validator 0 name length (33) exceeds MaxValidatorNameSize (32)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_zero_epoch_length() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.epoch_config.blocks_per_epoch = 0;
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Epoch length cannot be zero"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_an_epoch_outside_the_governance_range() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.epoch_config.blocks_per_epoch = 14_401;
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Epoch length 14401 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
        );
        record.epoch_config.blocks_per_epoch = 9;
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Epoch length 9 sits outside the governance range of 10 to 14400 blocks. Set blocks_per_epoch inside that range."
        );
        // Both ends of the range pass this check.
        record.epoch_config.blocks_per_epoch = 10;
        assert!(record.validate_genesis_comprehensive().is_ok());
        record.epoch_config.blocks_per_epoch = 14_400;
        assert!(record.validate_genesis_comprehensive().is_ok());
    });
}

#[test]
fn comprehensive_validation_refuses_a_mismatched_epoch_minimum_stake() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.epoch_config.min_stake = 1500;
        assert_eq!(
            record.validate_genesis_comprehensive().unwrap_err(),
            "Epoch config min_stake (1500) does not match pallet MinStake (1000)"
        );
    });
}

#[test]
fn comprehensive_validation_refuses_a_mismatched_epoch_ceiling() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.epoch_config.max_validators = 50;
        assert_eq!(
            DcfPallet::validate_genesis_config(&record).unwrap_err(),
            "Epoch config max_validators (50) does not match pallet MaxValidators (100)"
        );
    });
}

// ---------------------------------------------------------------------------
// Genesis dry run
// ---------------------------------------------------------------------------

#[test]
fn a_dry_run_of_equal_stakes_reports_every_invariant_and_no_warning() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_stakes = vec![2000, 2000, 2000];

        let report = record.dry_run_genesis_build().expect("the dry run passes");

        assert!(report.validation_passed);
        assert_eq!(report.validator_count, 3);
        assert_eq!(report.total_stake, 6000);
        assert_eq!(report.average_stake, 2000);
        assert_eq!(report.min_stake_validator, Some((10, 2000)));
        assert_eq!(report.max_stake_validator, Some((10, 2000)));
        assert_eq!(report.epoch_config, record.epoch_config);
        assert_eq!(
            report.invariant_checks,
            vec![
                "Economic: All validator stakes meet minimum requirements".to_string(),
                "Validator Set: Active validator count (3) within limits (min: 3, max: 100)"
                    .to_string(),
                "Temporal: Epoch length (100) is within reasonable bounds".to_string(),
                "Trust Score: Initial trust score (5000) within bounds (1000 to 10000)".to_string(),
                "Consensus Weights: PoS (6000) + PoI (4000) = 10000 > 0".to_string(),
            ]
        );
        assert!(report.warnings.is_empty());
    });
}

#[test]
fn a_dry_run_finds_the_extreme_stakes_and_warns_on_inequality_and_short_epochs() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_stakes = vec![2000, 1000, 30_000];
        record.epoch_config.blocks_per_epoch = 50;

        let report = DcfPallet::dry_run_genesis_build(&record).expect("the dry run passes");

        assert_eq!(report.total_stake, 33_000);
        assert_eq!(report.average_stake, 11_000);
        assert_eq!(report.min_stake_validator, Some((11, 1000)));
        assert_eq!(report.max_stake_validator, Some((12, 30_000)));
        assert_eq!(
            report.warnings,
            vec![
                "High stake inequality: Max stake (30000) is >10x min stake (1000)".to_string(),
                "Short epoch length: 50 blocks may cause frequent validator set changes"
                    .to_string(),
            ]
        );
    });
}

#[test]
fn a_dry_run_fills_empty_stakes_and_warns_on_long_epochs() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.epoch_config.blocks_per_epoch = 12_000;

        let report = record.dry_run_genesis_build().expect("the dry run passes");

        assert_eq!(report.total_stake, 3000);
        assert_eq!(report.average_stake, 1000);
        assert_eq!(report.min_stake_validator, Some((10, 1000)));
        assert_eq!(report.max_stake_validator, Some((10, 1000)));
        assert_eq!(
            report.warnings,
            vec!["Long epoch length: 12000 blocks may delay validator set updates".to_string()]
        );
    });
}

#[test]
fn a_dry_run_holds_a_tenfold_stake_gap_below_the_inequality_warning() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validator_stakes = vec![1000, 10_000, 1000];

        let report = record.dry_run_genesis_build().expect("the dry run passes");

        assert_eq!(report.max_stake_validator, Some((11, 10_000)));
        assert!(report.warnings.is_empty());
    });
}

#[test]
fn a_dry_run_returns_the_validation_error_unchanged() {
    new_test_ext().execute_with(|| {
        let mut record = sound_genesis();
        record.validators = vec![10, 11, 10];
        assert_eq!(
            record.dry_run_genesis_build().unwrap_err(),
            "Duplicate validator found at index 2: 10"
        );
        assert_eq!(
            DcfPallet::dry_run_genesis_build(&record).unwrap_err(),
            record.validate_genesis_comprehensive().unwrap_err()
        );
    });
}
