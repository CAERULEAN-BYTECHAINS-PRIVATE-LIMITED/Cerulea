//! Line coverage for the engine calls and the public engine helpers.
//!
//! These tests drive the dispatchable calls that wrap the Assurance Engine,
//! the Standing Engine and the Amendment Engine. They also drive the validator
//! record calls, the checkpoint calls and the public helpers that follow the
//! call block. Each test reads the effect out of pallet storage or out of the
//! event record.
//!
//! A failed dispatchable call runs inside a storage layer. The tests therefore
//! hold every failed call with `assert_noop!`, which also proves that storage
//! stayed unchanged.

use crate::engines::amendment::{
    Amendment, AmendmentError, AmendmentState, Authority, CandidatePoint, ConfirmationPreference,
    Declarations, ErrorCost, OperatorCount, PartitionChoice, PolicyKind, PolicySet, RetentionNeed,
    SplitBehaviour,
};
use crate::engines::assurance::{DelayWindow, ScheduledChange, ThresholdError, TransactionClass};
use crate::engines::standing::{
    ChainKind, DecaySettings, Misconduct, ResetRecord, SampleOutcome, ServiceEvidence,
    ServiceRecord,
};
use crate::mock::*;
use crate::{
    AmendmentApprovals, Amendments, AnchoredCheckpoints, AssuranceClassTable, AttestationShares,
    CustodyShares, DeploymentChainKind, Error, Event, GovernanceConfigStorage, LatestCheckpoint,
    MisbehaviorReports, NextAmendmentId, NextScheduledChangeId, ParameterRange, ParameterType,
    PerformanceRecord, PerformanceShares, Policies, RoleShares, SamplingRecords, ScheduledChanges,
    ServiceRecords, StandingDecay, StandingRampEpochs, StandingResets, StandingShares,
    ValidatorBlocksAuthored, ValidatorBlocksMissed, ValidatorJoinTime, ValidatorLastSeen,
    ValidatorMetadata, ValidatorNames, ValidatorPerformanceHistory, ValidatorSet, ValidatorStates,
};
use cerulea_primitives::checkpoint::Checkpoint;
use cerulea_primitives::equivocation::{
    EquivocationProof, SealClaimBytes, SealedHead, DRAW_KEY_OFFSET, RANK_OFFSET, SEAL_CLAIM_BYTES,
    SEAL_KEY_OFFSET, SLOT_OFFSET,
};
use codec::Encode;
use frame_support::traits::ConstU32;
use frame_support::{assert_err, assert_noop, assert_ok, BoundedVec};
use sp_core::ed25519::Pair;
use sp_core::Pair as PairTrait;
use sp_core::H256;
use sp_runtime::{DispatchError, Perbill};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// The evidence type the misbehaviour report carries.
type Evidence = BoundedVec<u8, <Test as crate::Config>::MaxEvidenceLength>;

/// The payload type a scheduled change carries.
type Payload = BoundedVec<u8, ConstU32<512>>;

/// Build test externalities that answer the offchain clock.
///
/// The validator record calls read a timestamp. The clock extension lets those
/// calls run here so the remaining logic is readable.
fn clocked_ext(millis: u64) -> sp_io::TestExternalities {
    let mut ext = new_test_ext();
    let (offchain, state) = sp_core::offchain::testing::TestOffchainExt::new();
    state.write().timestamp = sp_core::offchain::Timestamp::from_unix_millis(millis);
    ext.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));
    ext
}

/// Report whether the event record holds one pallet event.
fn has_event(event: Event<Test>) -> bool {
    System::events()
        .iter()
        .any(|record| record.event == RuntimeEvent::DcfPallet(event.clone()))
}

/// Every pallet event in the record, in order.
fn dcf_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(event) => Some(event),
            _ => None,
        })
        .collect()
}

/// Report whether the mock registry holds one validator in the active set.
fn seated(who: u64) -> bool {
    type Registry = TestValidatorRegistry;
    <Registry as crate::traits::ValidatorRegistryProvider<u64, u128, u64>>::is_validator_active(
        &who,
    )
}

fn short_name(bytes: &[u8]) -> BoundedVec<u8, ConstU32<32>> {
    BoundedVec::try_from(bytes.to_vec()).expect("the name fits thirty two bytes")
}

fn evidence(byte: u8) -> Evidence {
    BoundedVec::try_from(vec![byte; 16]).expect("sixteen bytes fit the evidence bound")
}

fn payload(byte: u8) -> Payload {
    BoundedVec::try_from(vec![byte; 8]).expect("eight bytes fit the payload bound")
}

fn every_condition() -> ServiceEvidence {
    ServiceEvidence {
        custody_held: true,
        participated: true,
        sampling_accurate: true,
    }
}

fn served_record(epochs_served: u32, last_served_epoch: u32) -> ServiceRecord {
    ServiceRecord {
        epochs_served,
        last_served_epoch,
        joined_epoch: 0,
    }
}

fn valid_point() -> CandidatePoint {
    CandidatePoint {
        validator_count: 10,
        tolerated_failures: 3,
        routine_percent: 51,
        constitutional_percent: 90,
        partition_choice: PartitionChoice::HaltToStayConsistent,
        partition_choice_stated: true,
    }
}

fn checkpoint(index: u32, block: u32, byte: u8) -> Checkpoint {
    Checkpoint::new(index, block, [byte; 32], [byte.wrapping_add(1); 32])
}

// ---------------------------------------------------------------------------
// Validator display name
// ---------------------------------------------------------------------------

#[test]
fn a_seated_validator_records_a_display_name() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_validator_name(
            RuntimeOrigin::signed(1),
            b"north".to_vec()
        ));
        assert_eq!(ValidatorNames::<Test>::get(&1), Some(short_name(b"north")));
        assert!(has_event(Event::ValidatorNameRegistered {
            validator: 1,
            name: short_name(b"north"),
        }));
    });
}

#[test]
fn a_name_of_thirty_two_bytes_fits() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_validator_name(
            RuntimeOrigin::signed(2),
            vec![b'a'; 32]
        ));
        assert_eq!(
            ValidatorNames::<Test>::get(&2).map(|held| held.len()),
            Some(32)
        );
    });
}

#[test]
fn a_name_past_thirty_two_bytes_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_validator_name(RuntimeOrigin::signed(1), vec![b'a'; 33]),
            Error::<Test>::InvalidEpochConfig
        );
    });
}

#[test]
fn a_name_from_an_account_without_a_seat_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_validator_name(RuntimeOrigin::signed(9), b"west".to_vec()),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn a_name_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_validator_name(RuntimeOrigin::root(), b"west".to_vec()),
            DispatchError::BadOrigin
        );
    });
}

// ---------------------------------------------------------------------------
// Validator metadata
// ---------------------------------------------------------------------------

#[test]
fn metadata_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::root(),
                b"north".to_vec(),
                None,
                None,
                None,
                None,
                None,
                None
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn metadata_from_an_account_without_a_seat_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(9),
                b"north".to_vec(),
                None,
                None,
                None,
                None,
                None,
                None
            ),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn every_oversized_metadata_field_is_refused() {
    new_test_ext().execute_with(|| {
        // The name holds 32 bytes at most.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                vec![b'n'; 33],
                None,
                None,
                None,
                None,
                None,
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
        // The website holds 64 bytes at most.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                b"north".to_vec(),
                Some(vec![b'w'; 65]),
                None,
                None,
                None,
                None,
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
        // The contact holds 64 bytes at most.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                b"north".to_vec(),
                None,
                Some(vec![b'c'; 65]),
                None,
                None,
                None,
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
        // The description holds 128 bytes at most.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                b"north".to_vec(),
                None,
                None,
                Some(vec![b'd'; 129]),
                None,
                None,
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
        // The location holds 32 bytes at most.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                b"north".to_vec(),
                None,
                None,
                None,
                Some(vec![b'l'; 33]),
                None,
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
    });
}

#[test]
fn a_commission_above_the_ceiling_is_refused() {
    new_test_ext().execute_with(|| {
        // The mock ceiling is 10000.
        assert_noop!(
            DcfPallet::set_validator_metadata(
                RuntimeOrigin::signed(1),
                b"north".to_vec(),
                None,
                None,
                None,
                None,
                Some(10_001),
                None
            ),
            Error::<Test>::InvalidEpochConfig
        );
    });
}

#[test]
fn metadata_reaches_storage_with_every_field() {
    clocked_ext(1_000).execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_validator_metadata(
            RuntimeOrigin::signed(1),
            b"north".to_vec(),
            Some(b"https://north.example".to_vec()),
            Some(b"ops@north.example".to_vec()),
            Some(b"A validator in the north region.".to_vec()),
            Some(b"Hyderabad".to_vec()),
            Some(10_000),
            Some(2_000u128)
        ));

        let held = ValidatorMetadata::<Test>::get(&1).expect("the chain holds the metadata");
        assert_eq!(held.name, short_name(b"north"));
        assert_eq!(
            held.website.map(|value| value.to_vec()),
            Some(b"https://north.example".to_vec())
        );
        assert_eq!(
            held.contact.map(|value| value.to_vec()),
            Some(b"ops@north.example".to_vec())
        );
        assert_eq!(
            held.description.map(|value| value.to_vec()),
            Some(b"A validator in the north region.".to_vec())
        );
        assert_eq!(
            held.location.map(|value| value.to_vec()),
            Some(b"Hyderabad".to_vec())
        );
        assert_eq!(held.commission_rate, Some(10_000));
        assert_eq!(held.min_stake_required, Some(2_000u128));
        assert!(held.updated_at >= held.created_at);
        assert_eq!(ValidatorNames::<Test>::get(&1), Some(short_name(b"north")));

        // The registry seeded a join time of zero, so the call keeps it.
        assert_eq!(ValidatorJoinTime::<Test>::get(&1), Some(0));

        let events = dcf_events();
        let named = events.iter().position(|event| {
            *event
                == Event::ValidatorNameRegistered {
                    validator: 1,
                    name: short_name(b"north"),
                }
        });
        let updated = events.iter().position(|event| {
            *event
                == Event::ValidatorMetadataUpdated {
                    validator: 1,
                    name: short_name(b"north"),
                }
        });
        assert!(named.is_some(), "the chain announces the name");
        assert!(updated.is_some(), "the chain announces the metadata");
        assert!(named < updated, "the name event comes first");
    });
}

#[test]
fn metadata_records_a_missing_join_time() {
    clocked_ext(2_000).execute_with(|| {
        ValidatorJoinTime::<Test>::remove(&2);
        assert_eq!(ValidatorJoinTime::<Test>::get(&2), None);

        System::set_block_number(7);
        assert_ok!(DcfPallet::set_validator_metadata(
            RuntimeOrigin::signed(2),
            b"south".to_vec(),
            None,
            None,
            None,
            None,
            None,
            None
        ));
        assert_eq!(ValidatorJoinTime::<Test>::get(&2), Some(7));
    });
}

#[test]
fn a_second_metadata_update_keeps_the_creation_time() {
    let mut ext = new_test_ext();
    let (offchain, state) = sp_core::offchain::testing::TestOffchainExt::new();
    state.write().timestamp = sp_core::offchain::Timestamp::from_unix_millis(1_000);
    ext.register_extension(sp_core::offchain::OffchainWorkerExt::new(offchain));

    ext.execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_validator_metadata(
            RuntimeOrigin::signed(3),
            b"east".to_vec(),
            Some(b"https://east.example".to_vec()),
            None,
            None,
            None,
            Some(250),
            None
        ));
        let first = ValidatorMetadata::<Test>::get(&3).expect("the first record exists");

        state.write().timestamp = sp_core::offchain::Timestamp::from_unix_millis(4_000);
        System::set_block_number(3);
        assert_ok!(DcfPallet::set_validator_metadata(
            RuntimeOrigin::signed(3),
            b"east two".to_vec(),
            None,
            None,
            None,
            None,
            None,
            None
        ));
        let second = ValidatorMetadata::<Test>::get(&3).expect("the second record exists");

        assert_eq!(second.created_at, first.created_at);
        assert!(second.updated_at >= second.created_at);
        assert_eq!(second.name, short_name(b"east two"));
        assert_eq!(second.website, None);
        assert_eq!(second.commission_rate, None);
        assert_eq!(second.min_stake_required, None);
    });
}

/// A dispatchable call runs on chain, where no offchain clock exists.
///
/// The metadata call reads the offchain clock today, which halts the call on a
/// running chain. This test holds the correct behaviour.
#[test]
fn metadata_reaches_storage_without_an_offchain_clock() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_validator_metadata(
            RuntimeOrigin::signed(1),
            b"north".to_vec(),
            None,
            None,
            None,
            None,
            None,
            None
        ));
        assert!(ValidatorMetadata::<Test>::get(&1).is_some());
    });
}

// ---------------------------------------------------------------------------
// Validator activity
// ---------------------------------------------------------------------------

#[test]
fn activity_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::update_validator_activity(RuntimeOrigin::signed(1), 1, 5, 2),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn activity_updates_counters_history_and_the_event() {
    clocked_ext(5_000).execute_with(|| {
        System::set_block_number(10);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            5,
            2
        ));

        assert_eq!(ValidatorLastSeen::<Test>::get(&1), 10);
        assert_eq!(ValidatorBlocksAuthored::<Test>::get(&1), 5);
        assert_eq!(ValidatorBlocksMissed::<Test>::get(&1), 2);

        let state = ValidatorStates::<Test>::get(&1).expect("genesis seeded the state");
        let history = ValidatorPerformanceHistory::<Test>::get(&1);
        assert_eq!(history.len(), 1);
        let record = &history[0];
        assert_eq!(record.epoch, DcfPallet::current_epoch());
        assert_eq!(record.blocks_authored, 5);
        assert_eq!(record.blocks_missed, 2);
        assert_eq!(record.inference_score, state.current.inference_score);
        assert_eq!(record.stake_score, state.current.stake_score);
        assert_eq!(record.final_score, state.current.final_score);
        assert_eq!(record.participation_rate, state.participation_rate);

        // The validator joined at block zero and had never been seen before
        // this report at block ten, so the uptime reads zero.
        assert_eq!(record.uptime_percentage, 0);
        assert!(has_event(Event::ValidatorActivityUpdated {
            validator: 1,
            blocks_authored: 5,
            blocks_missed: 2,
            uptime_percentage: 0,
        }));
    });
}

#[test]
fn activity_accumulates_across_calls() {
    clocked_ext(5_000).execute_with(|| {
        System::set_block_number(4);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            2,
            5,
            1
        ));
        System::set_block_number(6);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            2,
            3,
            2
        ));

        assert_eq!(ValidatorBlocksAuthored::<Test>::get(&2), 8);
        assert_eq!(ValidatorBlocksMissed::<Test>::get(&2), 3);
        assert_eq!(ValidatorLastSeen::<Test>::get(&2), 6);
        assert_eq!(ValidatorPerformanceHistory::<Test>::get(&2).len(), 2);
    });
}

#[test]
fn activity_for_an_account_without_state_moves_the_counters_alone() {
    clocked_ext(5_000).execute_with(|| {
        System::set_block_number(3);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            9,
            4,
            1
        ));

        assert_eq!(ValidatorLastSeen::<Test>::get(&9), 3);
        assert_eq!(ValidatorBlocksAuthored::<Test>::get(&9), 4);
        assert_eq!(ValidatorBlocksMissed::<Test>::get(&9), 1);
        assert!(ValidatorPerformanceHistory::<Test>::get(&9).is_empty());
        let announced = dcf_events()
            .iter()
            .any(|event| matches!(event, Event::ValidatorActivityUpdated { .. }));
        assert!(
            !announced,
            "an account without state raises no activity event"
        );
    });
}

#[test]
fn a_full_history_drops_the_oldest_record() {
    clocked_ext(5_000).execute_with(|| {
        let mut full: BoundedVec<PerformanceRecord, ConstU32<100>> = BoundedVec::default();
        for epoch in 0..100u32 {
            full.try_push(PerformanceRecord {
                epoch,
                blocks_authored: 1,
                blocks_missed: 0,
                uptime_percentage: 10_000,
                inference_score: 0,
                stake_score: 0,
                final_score: 0,
                participation_rate: 100,
                timestamp: 0,
            })
            .expect("one hundred records fit");
        }
        ValidatorPerformanceHistory::<Test>::insert(&1, full);

        System::set_block_number(2);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            7,
            0
        ));

        let history = ValidatorPerformanceHistory::<Test>::get(&1);
        assert_eq!(history.len(), 100);
        assert_eq!(history[0].epoch, 1, "the record of epoch zero left first");
        assert_eq!(
            history[99].blocks_authored, 7,
            "the newest record sits last"
        );
    });
}

/// The counters saturate at the ceiling of the type.
///
/// The call adds with a plain addition today. A large report therefore
/// overflows. This test holds the correct behaviour.
#[test]
fn activity_counters_saturate_at_the_ceiling() {
    clocked_ext(5_000).execute_with(|| {
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            u32::MAX,
            u32::MAX
        ));
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            1,
            1
        ));
        assert_eq!(ValidatorBlocksAuthored::<Test>::get(&1), u32::MAX);
        assert_eq!(ValidatorBlocksMissed::<Test>::get(&1), u32::MAX);
    });
}

/// A dispatchable call runs on chain, where no offchain clock exists.
///
/// The activity call reads the offchain clock today, which halts the call on a
/// running chain. This test holds the correct behaviour.
#[test]
fn activity_reaches_storage_without_an_offchain_clock() {
    new_test_ext().execute_with(|| {
        System::set_block_number(2);
        assert_ok!(DcfPallet::update_validator_activity(
            RuntimeOrigin::root(),
            1,
            2,
            0
        ));
        assert_eq!(ValidatorBlocksAuthored::<Test>::get(&1), 2);
        assert_eq!(ValidatorPerformanceHistory::<Test>::get(&1).len(), 1);
    });
}

// ---------------------------------------------------------------------------
// Misbehaviour reports
// ---------------------------------------------------------------------------

#[test]
fn a_report_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::root(), 1, evidence(1)),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_reporter_outside_the_set_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(9), 1, evidence(1)),
            Error::<Test>::ValidatorNotInSet
        );
    });
}

#[test]
fn a_report_against_an_account_outside_the_set_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(2), 9, evidence(1)),
            Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn a_validator_reporting_itself_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(1), 1, evidence(1)),
            Error::<Test>::InvalidScore
        );
    });
}

#[test]
fn one_reporter_reports_one_validator_once() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            1,
            evidence(1)
        ));
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(2), 1, evidence(2)),
            Error::<Test>::InvalidScore
        );
    });
}

#[test]
fn a_report_records_the_evidence_and_the_count() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            1,
            evidence(4)
        ));
        assert_eq!(
            MisbehaviorReports::<Test>::get(1u64, 2u64),
            Some(evidence(4))
        );
        assert!(has_event(Event::ValidatorMisbehaviorReported {
            reported_validator: 1,
            reporter: 2,
            evidence: evidence(4),
            total_reports: 1,
        }));

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(3),
            1,
            evidence(5)
        ));
        assert!(has_event(Event::ValidatorMisbehaviorReported {
            reported_validator: 1,
            reporter: 3,
            evidence: evidence(5),
            total_reports: 2,
        }));

        // Two reports sit below the threshold of three.
        let slashed = dcf_events()
            .iter()
            .any(|event| matches!(event, Event::ValidatorAutoSlashed { .. }));
        assert!(!slashed, "two reports leave the stake untouched");
        assert_eq!(MisbehaviorReports::<Test>::iter_prefix(1u64).count(), 2);
    });
}

#[test]
fn the_third_report_slashes_and_clears_the_reports() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        ValidatorSet::<Test>::mutate(|set| {
            let _ = set.try_push(4);
        });

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            1,
            evidence(1)
        ));
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(3),
            1,
            evidence(2)
        ));

        // The slash takes ten percent of the free balance. The stake pallet in
        // the mock runtime sets ten percent.
        let free = <Balances as frame_support::traits::Currency<u64>>::free_balance(&1);
        let expected = free / 100 * 10;

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(4),
            1,
            evidence(3)
        ));
        assert!(has_event(Event::ValidatorMisbehaviorReported {
            reported_validator: 1,
            reporter: 4,
            evidence: evidence(3),
            total_reports: 3,
        }));
        assert!(has_event(Event::ValidatorAutoSlashed {
            validator: 1,
            amount: expected,
            report_count: 3,
        }));
        assert_eq!(
            MisbehaviorReports::<Test>::iter_prefix(1u64).count(),
            0,
            "the slash clears every report against the validator"
        );
    });
}

#[test]
fn the_third_report_against_a_validator_without_state_rolls_back() {
    new_test_ext().execute_with(|| {
        ValidatorSet::<Test>::mutate(|set| {
            let _ = set.try_push(4);
        });
        assert!(!ValidatorStates::<Test>::contains_key(&4));

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(1),
            4,
            evidence(1)
        ));
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            4,
            evidence(2)
        ));
        // The slash amount reads the state, so the third report fails whole.
        assert_noop!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(3), 4, evidence(3)),
            Error::<Test>::ValidatorNotFound
        );
        assert_eq!(MisbehaviorReports::<Test>::iter_prefix(4u64).count(), 2);
    });
}

// ---------------------------------------------------------------------------
// Assurance Engine class table
// ---------------------------------------------------------------------------

#[test]
fn the_class_table_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_assurance_class(
                RuntimeOrigin::signed(1),
                TransactionClass::Financial,
                75,
                0
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_share_above_ninety_nine_percent_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_assurance_class(
                RuntimeOrigin::root(),
                TransactionClass::Constitutional,
                100,
                100
            ),
            Error::<Test>::AssuranceTableInvalid
        );
    });
}

#[test]
fn a_share_below_a_lighter_class_is_refused() {
    new_test_ext().execute_with(|| {
        // The financial class holds 67 percent, so a structural share of 60
        // breaks the order.
        assert_noop!(
            DcfPallet::set_assurance_class(
                RuntimeOrigin::root(),
                TransactionClass::Structural,
                60,
                0
            ),
            Error::<Test>::AssuranceTableInvalid
        );
    });
}

#[test]
fn a_longer_delay_window_reaches_the_table() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::set_assurance_class(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            90,
            250
        ));
        let table = AssuranceClassTable::<Test>::get();
        assert_eq!(table.delay_blocks(TransactionClass::Constitutional), 250);
        assert_eq!(
            table.threshold_percent(TransactionClass::Constitutional),
            90
        );
        assert!(has_event(Event::AssuranceClassTableAmended {
            class: TransactionClass::Constitutional,
            previous_percent: 90,
            current_percent: 90,
            delay_blocks: 250,
        }));
    });
}

// ---------------------------------------------------------------------------
// Scheduled changes
// ---------------------------------------------------------------------------

#[test]
fn a_scheduled_change_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::schedule_class_change(
                RuntimeOrigin::signed(1),
                TransactionClass::Routine,
                payload(1)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn scheduling_records_the_change_and_raises_the_opened_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload(6)
        ));

        let change = ScheduledChanges::<Test>::get(0).expect("the change is scheduled");
        assert_eq!(change.payload, payload(6));
        assert!(!change.released);
        assert_eq!(NextScheduledChangeId::<Test>::get(), 1);
        assert!(has_event(Event::ScheduledChangeOpened {
            id: 0,
            class: TransactionClass::Constitutional,
            committed_at: 4,
            effective_at: 104,
        }));
    });
}

#[test]
fn a_full_identifier_space_refuses_a_new_change() {
    new_test_ext().execute_with(|| {
        NextScheduledChangeId::<Test>::put(u64::MAX);
        assert_noop!(
            DcfPallet::schedule_class_change(
                RuntimeOrigin::root(),
                TransactionClass::Routine,
                payload(1)
            ),
            Error::<Test>::ScheduledChangeQueueFull
        );
        assert_err!(
            DcfPallet::open_scheduled_change(TransactionClass::Routine, payload(2)),
            Error::<Test>::ScheduledChangeQueueFull
        );
        assert!(ScheduledChanges::<Test>::get(u64::MAX).is_none());
    });
}

#[test]
fn the_helper_returns_the_identifier_it_opened() {
    new_test_ext().execute_with(|| {
        System::set_block_number(2);
        assert_eq!(
            DcfPallet::open_scheduled_change(TransactionClass::Financial, payload(3)),
            Ok(0)
        );
        assert_eq!(
            DcfPallet::open_scheduled_change(TransactionClass::Structural, payload(4)),
            Ok(1)
        );
        let second = ScheduledChanges::<Test>::get(1).expect("the second change exists");
        assert_eq!(second.class, TransactionClass::Structural);
        assert_eq!(second.window.effective_at, 2);
    });
}

#[test]
fn a_release_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::release_scheduled_change(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_change_marked_released_releases_no_second_time() {
    new_test_ext().execute_with(|| {
        ScheduledChanges::<Test>::insert(
            3,
            ScheduledChange {
                class: TransactionClass::Routine,
                window: DelayWindow::open(0, 0),
                payload: payload(1),
                released: true,
            },
        );
        assert_noop!(
            DcfPallet::release_scheduled_change(RuntimeOrigin::signed(1), 3),
            Error::<Test>::ScheduledChangeAlreadyReleased
        );
        assert_err!(
            DcfPallet::release_change(3),
            Error::<Test>::ScheduledChangeAlreadyReleased
        );
    });
}

#[test]
fn the_helper_refuses_an_absent_change_and_an_open_window() {
    new_test_ext().execute_with(|| {
        assert_err!(
            DcfPallet::release_change(77),
            Error::<Test>::ScheduledChangeNotFound
        );

        System::set_block_number(1);
        assert_ok!(DcfPallet::open_scheduled_change(
            TransactionClass::Constitutional,
            payload(5)
        ));
        assert_err!(DcfPallet::release_change(0), Error::<Test>::DelayWindowOpen);
        assert!(ScheduledChanges::<Test>::get(0).is_some());
    });
}

#[test]
fn the_chain_releases_every_matured_change_at_once() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        for class in [
            TransactionClass::Routine,
            TransactionClass::Financial,
            TransactionClass::Constitutional,
        ] {
            assert_ok!(DcfPallet::open_scheduled_change(class, payload(1)));
        }

        assert_eq!(DcfPallet::release_matured_changes(1), 2);
        assert!(ScheduledChanges::<Test>::get(0).is_none());
        assert!(ScheduledChanges::<Test>::get(1).is_none());
        assert!(ScheduledChanges::<Test>::get(2).is_some());

        assert_eq!(DcfPallet::release_matured_changes(50), 0);
        assert!(ScheduledChanges::<Test>::get(2).is_some());

        System::set_block_number(101);
        assert_eq!(DcfPallet::release_matured_changes(101), 1);
        assert!(ScheduledChanges::<Test>::get(2).is_none());
        assert!(has_event(Event::ScheduledChangeReleased {
            id: 2,
            class: TransactionClass::Constitutional,
            released_at: 101,
        }));
    });
}

#[test]
fn a_matured_sweep_passes_over_a_change_marked_released() {
    new_test_ext().execute_with(|| {
        ScheduledChanges::<Test>::insert(
            5,
            ScheduledChange {
                class: TransactionClass::Financial,
                window: DelayWindow::open(0, 0),
                payload: payload(2),
                released: true,
            },
        );
        assert_eq!(DcfPallet::release_matured_changes(10), 0);
        assert!(ScheduledChanges::<Test>::get(5).is_some());
    });
}

// ---------------------------------------------------------------------------
// Standing Engine calls
// ---------------------------------------------------------------------------

#[test]
fn a_service_epoch_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::record_service_epoch(RuntimeOrigin::signed(1), 1, 1, every_condition()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_epoch_missing_a_condition_is_refused() {
    new_test_ext().execute_with(|| {
        let missing = ServiceEvidence {
            custody_held: true,
            participated: false,
            sampling_accurate: true,
        };
        assert_noop!(
            DcfPallet::record_service_epoch(RuntimeOrigin::root(), 1, 1, missing),
            Error::<Test>::ServiceConditionsUnmet
        );
    });
}

#[test]
fn a_service_epoch_counts_once() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::record_service_epoch(
            RuntimeOrigin::root(),
            1,
            3,
            every_condition()
        ));
        assert_noop!(
            DcfPallet::record_service_epoch(RuntimeOrigin::root(), 1, 3, every_condition()),
            Error::<Test>::ServiceEpochAlreadyCounted
        );
        assert_noop!(
            DcfPallet::record_service_epoch(RuntimeOrigin::root(), 1, 2, every_condition()),
            Error::<Test>::ServiceEpochAlreadyCounted
        );
    });
}

#[test]
fn a_served_epoch_raises_standing_along_the_ramp() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        StandingRampEpochs::<Test>::put(4);

        assert_ok!(DcfPallet::record_service_epoch(
            RuntimeOrigin::root(),
            2,
            1,
            every_condition()
        ));
        assert_eq!(
            StandingShares::<Test>::get(2),
            Some(Perbill::from_percent(25))
        );

        assert_ok!(DcfPallet::record_service_epoch(
            RuntimeOrigin::root(),
            2,
            2,
            every_condition()
        ));
        let record = ServiceRecords::<Test>::get(2);
        assert_eq!(record.epochs_served, 2);
        assert_eq!(record.last_served_epoch, 2);
        assert_eq!(
            StandingShares::<Test>::get(2),
            Some(Perbill::from_percent(50))
        );
        assert!(has_event(Event::ServiceEpochCounted {
            validator: 2,
            epoch: 2,
            epochs_served: 2,
            standing: Perbill::from_percent(50),
        }));
    });
}

#[test]
fn a_sampling_report_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::report_sampling_outcome(RuntimeOrigin::signed(1), 1, SampleOutcome::Agreed),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn sampling_reports_move_the_share() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::report_sampling_outcome(
            RuntimeOrigin::root(),
            3,
            SampleOutcome::Agreed
        ));
        assert!(has_event(Event::SamplingReportSettled {
            validator: 3,
            outcome: SampleOutcome::Agreed,
            sampling: Perbill::from_parts(520_000_000),
        }));

        assert_ok!(DcfPallet::report_sampling_outcome(
            RuntimeOrigin::root(),
            3,
            SampleOutcome::FaultCaught
        ));
        assert_eq!(
            DcfPallet::sampling_share(&3),
            Perbill::from_parts(620_000_000)
        );

        assert_ok!(DcfPallet::report_sampling_outcome(
            RuntimeOrigin::root(),
            3,
            SampleOutcome::FalseReport
        ));
        assert_eq!(
            DcfPallet::sampling_share(&3),
            Perbill::from_parts(420_000_000)
        );

        let record = SamplingRecords::<Test>::get(3);
        assert_eq!(record.agreements, 1);
        assert_eq!(record.faults_caught, 1);
        assert_eq!(record.false_reports, 1);
    });
}

#[test]
fn proven_misconduct_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::prove_misconduct(RuntimeOrigin::signed(2), 1, Misconduct::FalseCustodyProof),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn proven_misconduct_returns_standing_to_zero_and_keeps_the_seat() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(7);
        ServiceRecords::<Test>::insert(1, served_record(12, 6));
        StandingShares::<Test>::insert(1, Perbill::from_percent(60));

        assert_ok!(DcfPallet::prove_misconduct(
            RuntimeOrigin::root(),
            1,
            Misconduct::FalseCustodyProof
        ));

        assert_eq!(StandingShares::<Test>::get(1), Some(Perbill::zero()));
        let record = ServiceRecords::<Test>::get(1);
        assert_eq!(record.epochs_served, 0);
        assert_eq!(record.last_served_epoch, 0);
        assert_eq!(record.joined_epoch, 7);
        assert_eq!(
            StandingResets::<Test>::get(1),
            Some(ResetRecord {
                misconduct: Misconduct::FalseCustodyProof,
                epoch: 7,
                epochs_destroyed: 12,
            })
        );
        assert!(has_event(Event::StandingReset {
            validator: 1,
            misconduct: Misconduct::FalseCustodyProof,
            epochs_destroyed: 12,
        }));
        // A false custody proof takes the seat at the epoch boundary.
        assert!(seated(1));
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine: opening
// ---------------------------------------------------------------------------

#[test]
fn opening_an_amendment_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::open_amendment(RuntimeOrigin::root(), PolicyKind::Assurance),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_account_without_a_seat_opens_no_amendment() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::open_amendment(RuntimeOrigin::signed(9), PolicyKind::Assurance),
            Error::<Test>::AmendmentAuthorityAbsent
        );
    });
}

#[test]
fn a_seated_validator_opens_an_amendment() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_ok!(DcfPallet::open_amendment(
            RuntimeOrigin::signed(1),
            PolicyKind::Assurance
        ));

        assert_eq!(NextAmendmentId::<Test>::get(), 1);
        let held = Amendments::<Test>::get(0).expect("the amendment is open");
        assert_eq!(held.policy, PolicyKind::Assurance);
        assert_eq!(held.opened_at, 5);
        assert_eq!(held.voting_closes_at, 105);
        assert_eq!(held.effective_at, 205);
        assert_eq!(held.body_size, 3);
        assert_eq!(held.quorum, Perbill::from_percent(67));
        assert_eq!(held.approvals, 0);
        assert_eq!(held.state, AmendmentState::Collecting);

        // Two thirds of a body of three rounds upward to three approvals.
        assert!(has_event(Event::AmendmentOpened {
            id: 0,
            policy: PolicyKind::Assurance,
            approvals_required: 3,
            effective_at: 205,
        }));
    });
}

#[test]
fn an_operator_authority_refuses_a_seated_validator() {
    new_test_ext().execute_with(|| {
        Policies::<Test>::mutate(|held| held.amendment.authority = Authority::Operator);
        assert_noop!(
            DcfPallet::open_amendment(RuntimeOrigin::signed(1), PolicyKind::Reward),
            Error::<Test>::AmendmentAuthorityAbsent
        );
    });
}

#[test]
fn a_governing_body_authority_admits_a_seated_validator() {
    new_test_ext().execute_with(|| {
        Policies::<Test>::mutate(|held| held.amendment.authority = Authority::GoverningBody);
        assert_ok!(DcfPallet::open_amendment(
            RuntimeOrigin::signed(2),
            PolicyKind::Penalty
        ));
        assert_eq!(
            Amendments::<Test>::get(0).map(|held| held.policy),
            Some(PolicyKind::Penalty)
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine: approval
// ---------------------------------------------------------------------------

fn open_assurance_amendment_at(block: u64) {
    System::set_block_number(block);
    assert_ok!(DcfPallet::open_amendment(
        RuntimeOrigin::signed(1),
        PolicyKind::Assurance
    ));
}

#[test]
fn approving_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_account_without_a_seat_approves_nothing() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(9), 0),
            Error::<Test>::AmendmentAuthorityAbsent
        );
    });
}

#[test]
fn approving_an_absent_amendment_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 42),
            Error::<Test>::AmendmentNotFound
        );
    });
}

#[test]
fn one_account_approves_one_amendment_once() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        assert_ok!(DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 0));
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 0),
            Error::<Test>::AmendmentAlreadyApproved
        );
    });
}

#[test]
fn approvals_reach_the_quorum_and_the_amendment_waits() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        System::set_block_number(6);
        for (count, approver) in [1u64, 2, 3].iter().enumerate() {
            assert_ok!(DcfPallet::approve_amendment(
                RuntimeOrigin::signed(*approver),
                0
            ));
            assert!(AmendmentApprovals::<Test>::get(0u64, *approver));
            assert!(has_event(Event::AmendmentApproved {
                id: 0,
                approver: *approver,
                approvals: count as u32 + 1,
            }));
        }
        let held = Amendments::<Test>::get(0).expect("the amendment exists");
        assert_eq!(held.approvals, 3);
        assert_eq!(held.state, AmendmentState::Waiting);
    });
}

#[test]
fn an_approval_after_the_voting_window_fails() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        System::set_block_number(106);
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(2), 0),
            Error::<Test>::AmendmentVotingWindowClosed
        );
        assert_eq!(
            Amendments::<Test>::get(0).map(|held| held.state),
            Some(AmendmentState::Collecting)
        );
    });
}

#[test]
fn an_approval_on_an_enacted_amendment_fails() {
    new_test_ext().execute_with(|| {
        Amendments::<Test>::insert(
            9,
            Amendment {
                voting_closes_at: 1_000,
                state: AmendmentState::Enacted,
                ..Default::default()
            },
        );
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 9),
            Error::<Test>::AmendmentAlreadyEnacted
        );
    });
}

#[test]
fn an_approval_on_a_lapsed_amendment_fails() {
    new_test_ext().execute_with(|| {
        Amendments::<Test>::insert(
            8,
            Amendment {
                voting_closes_at: 1_000,
                state: AmendmentState::Lapsed,
                ..Default::default()
            },
        );
        assert_noop!(
            DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 8),
            Error::<Test>::AmendmentVotingWindowClosed
        );
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine: enactment
// ---------------------------------------------------------------------------

#[test]
fn enacting_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::root(), 0),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn enacting_an_absent_amendment_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(1), 3),
            Error::<Test>::AmendmentNotFound
        );
    });
}

#[test]
fn an_amendment_short_of_the_quorum_fails_to_enact() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        assert_ok!(DcfPallet::approve_amendment(RuntimeOrigin::signed(1), 0));
        System::set_block_number(50);
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(1), 0),
            Error::<Test>::AmendmentQuorumShort
        );
    });
}

#[test]
fn an_amendment_past_its_window_without_quorum_fails_to_enact() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        System::set_block_number(106);
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(1), 0),
            Error::<Test>::AmendmentVotingWindowClosed
        );
        assert_eq!(
            Amendments::<Test>::get(0).map(|held| held.state),
            Some(AmendmentState::Collecting)
        );
    });
}

#[test]
fn an_amendment_enacts_once_after_the_delay() {
    new_test_ext().execute_with(|| {
        open_assurance_amendment_at(5);
        System::set_block_number(6);
        for approver in [1u64, 2, 3] {
            assert_ok!(DcfPallet::approve_amendment(
                RuntimeOrigin::signed(approver),
                0
            ));
        }

        System::set_block_number(204);
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(2), 0),
            Error::<Test>::AmendmentDelayWindowOpen
        );

        System::set_block_number(205);
        assert_ok!(DcfPallet::enact_amendment(RuntimeOrigin::signed(2), 0));
        assert_eq!(
            Amendments::<Test>::get(0).map(|held| held.state),
            Some(AmendmentState::Enacted)
        );
        assert!(has_event(Event::AmendmentEnacted {
            id: 0,
            policy: PolicyKind::Assurance,
        }));

        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(2), 0),
            Error::<Test>::AmendmentAlreadyEnacted
        );
    });
}

#[test]
fn a_lapsed_amendment_fails_to_enact() {
    new_test_ext().execute_with(|| {
        Amendments::<Test>::insert(
            4,
            Amendment {
                state: AmendmentState::Lapsed,
                ..Default::default()
            },
        );
        assert_noop!(
            DcfPallet::enact_amendment(RuntimeOrigin::signed(1), 4),
            Error::<Test>::AmendmentVotingWindowClosed
        );
    });
}

#[test]
fn every_amendment_failure_maps_to_its_own_error() {
    new_test_ext().execute_with(|| {
        let cases: [(AmendmentError, DispatchError); 6] = [
            (
                AmendmentError::AuthorityAbsent,
                Error::<Test>::AmendmentAuthorityAbsent.into(),
            ),
            (
                AmendmentError::QuorumShort,
                Error::<Test>::AmendmentQuorumShort.into(),
            ),
            (
                AmendmentError::DelayWindowOpen,
                Error::<Test>::AmendmentDelayWindowOpen.into(),
            ),
            (
                AmendmentError::AlreadyEnacted,
                Error::<Test>::AmendmentAlreadyEnacted.into(),
            ),
            (
                AmendmentError::VotingWindowClosed,
                Error::<Test>::AmendmentVotingWindowClosed.into(),
            ),
            (
                AmendmentError::AlreadyApproved,
                Error::<Test>::AmendmentAlreadyApproved.into(),
            ),
        ];
        for (failure, expected) in cases {
            assert_eq!(
                DcfPallet::amendment_error(failure),
                expected,
                "the failure {:?} maps to its own pallet error",
                failure
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Amendment Engine: policy profiles
// ---------------------------------------------------------------------------

#[test]
fn adopting_a_profile_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::adopt_policy_set(
                RuntimeOrigin::signed(1),
                PolicySet::starting_profile(),
                valid_point()
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_profile_crossing_a_fixed_limit_is_refused() {
    new_test_ext().execute_with(|| {
        let mut point = valid_point();
        // Ten validators tolerate three failures at most.
        point.tolerated_failures = 4;
        let mut policies = PolicySet::starting_profile();
        policies.amendment.delay_blocks = 300;
        assert_noop!(
            DcfPallet::adopt_policy_set(RuntimeOrigin::root(), policies, point),
            Error::<Test>::ProfileCrossesFixedLimit
        );
    });
}

#[test]
fn a_valid_profile_replaces_the_policies() {
    new_test_ext().execute_with(|| {
        let mut policies = PolicySet::starting_profile();
        policies.amendment.delay_blocks = 300;
        policies.rotation.set_size = 21;
        assert_ok!(DcfPallet::adopt_policy_set(
            RuntimeOrigin::root(),
            policies,
            valid_point()
        ));
        assert_eq!(Policies::<Test>::get(), policies);
    });
}

#[test]
fn the_solver_answers_through_the_pallet() {
    new_test_ext().execute_with(|| {
        let declarations = Declarations {
            confirmation: ConfirmationPreference::MaximumCertainty,
            operators: OperatorCount::ManyOrganisations,
            error_cost: ErrorCost::Irreversible,
            split: SplitBehaviour::KeepServing,
            retention: RetentionNeed::SeveralYears,
        };
        let profiles = DcfPallet::solve_profiles(declarations);
        assert_eq!(
            profiles,
            crate::engines::amendment::solve(declarations),
            "the pallet returns the profiles the engine solves"
        );
        assert!(profiles.len() >= crate::engines::amendment::MIN_PROFILES);
        assert!(profiles.len() <= crate::engines::amendment::MAX_PROFILES);
        for profile in profiles.iter() {
            assert_eq!(crate::engines::amendment::inspect_profile(profile), Ok(()));
        }
    });
}

// ---------------------------------------------------------------------------
// Equivocation evidence
// ---------------------------------------------------------------------------

const OFFENDER: u64 = 1;
const DRAW_KEY: [u8; 32] = [7u8; 32];

fn author_pair() -> Pair {
    Pair::from_seed(&[42u8; 32])
}

fn claim_bytes(slot: u64, rank: u8, seal_key: [u8; 32]) -> SealClaimBytes {
    let mut claim = [0u8; SEAL_CLAIM_BYTES];
    claim[SLOT_OFFSET..SLOT_OFFSET + 8].copy_from_slice(&slot.to_le_bytes());
    claim[RANK_OFFSET] = rank;
    claim[SEAL_KEY_OFFSET..SEAL_KEY_OFFSET + 32].copy_from_slice(&seal_key);
    claim[DRAW_KEY_OFFSET..DRAW_KEY_OFFSET + 32].copy_from_slice(&DRAW_KEY);
    claim
}

fn sealed_head(pair: &Pair, pre_seal_hash: [u8; 32], slot: u64, rank: u8) -> SealedHead {
    let mut sealed = SealedHead::new(
        pre_seal_hash,
        claim_bytes(slot, rank, pair.public().0),
        [0u8; 64],
    );
    sealed.signature = pair.sign(&sealed.payload()).0;
    sealed
}

fn equivocation(pair: &Pair, slot: u64, rank: u8) -> EquivocationProof {
    EquivocationProof::new(
        sealed_head(pair, [1u8; 32], slot, rank),
        sealed_head(pair, [2u8; 32], slot, rank),
    )
}

#[test]
fn equivocation_evidence_needs_a_signed_origin() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        registry_record_keys(OFFENDER, DRAW_KEY, pair.public().0);
        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::none(),
                Box::new(equivocation(&pair, 9, 1))
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_draw_key_without_a_recorded_seal_key_carries_no_penalty() {
    new_test_ext().execute_with(|| {
        let pair = author_pair();
        // The mock registry stores the draw key owner under this raw key. The
        // seal key stays absent, so the second registry read finds nothing.
        let mut key = b"registry/draw_owner/".to_vec();
        key.extend_from_slice(&DRAW_KEY);
        sp_io::storage::set(&key, &OFFENDER.encode());

        assert_noop!(
            DcfPallet::report_equivocation(
                RuntimeOrigin::signed(2),
                Box::new(equivocation(&pair, 9, 1))
            ),
            Error::<Test>::EquivocationAuthorUnregistered
        );
    });
}

#[test]
fn proven_equivocation_takes_the_seat_and_announces_the_reset() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        let pair = author_pair();
        registry_record_keys(OFFENDER, DRAW_KEY, pair.public().0);
        ServiceRecords::<Test>::insert(OFFENDER, served_record(6, 6));
        assert!(seated(OFFENDER));

        assert_ok!(DcfPallet::report_equivocation(
            RuntimeOrigin::signed(2),
            Box::new(equivocation(&pair, 11, 2))
        ));

        assert!(
            !seated(OFFENDER),
            "two blocks at one slot take the seat at once"
        );
        assert!(has_event(Event::StandingReset {
            validator: OFFENDER,
            misconduct: Misconduct::TwoBlocksAtOneSlot,
            epochs_destroyed: 6,
        }));
        assert!(has_event(Event::EquivocationProven {
            validator: OFFENDER,
            slot: 11,
            rank: 2,
            epochs_destroyed: 6,
        }));
    });
}

// ---------------------------------------------------------------------------
// Checkpoints
// ---------------------------------------------------------------------------

#[test]
fn anchoring_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::signed(1), checkpoint(0, 900, 1)),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn the_checkpoint_interval_needs_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::set_checkpoint_interval(RuntimeOrigin::signed(1), 40),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_empty_anchored_set_settles_block_zero() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::anchored_checkpoints().is_empty());
        assert_eq!(DcfPallet::highest_anchored_block(), 0);
        assert!(LatestCheckpoint::<Test>::get().is_none());
    });
}

#[test]
fn the_anchored_list_sorts_by_index() {
    new_test_ext().execute_with(|| {
        // Storage iterates in hash order, so the helper sorts the entries.
        for index in [3u32, 0, 2, 1] {
            AnchoredCheckpoints::<Test>::insert(index, checkpoint(index, (index + 1) * 100, 1));
        }
        let held = DcfPallet::anchored_checkpoints();
        let order: Vec<u32> = held.iter().map(|entry| entry.index).collect();
        assert_eq!(order, vec![0, 1, 2, 3]);
    });
}

// ---------------------------------------------------------------------------
// Standing Engine helpers
// ---------------------------------------------------------------------------

#[test]
fn the_ramp_reads_the_starting_profile_until_set() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::ramp_epochs(),
            crate::engines::standing::DEFAULT_RAMP_EPOCHS
        );
        StandingRampEpochs::<Test>::put(12);
        assert_eq!(DcfPallet::ramp_epochs(), 12);
    });
}

#[test]
fn the_decay_reads_the_starting_profile_until_set() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::decay_settings(),
            DecaySettings::starting_profile()
        );

        let custom = DecaySettings {
            decay_percent: 10,
            grace_epochs: 1,
        };
        StandingDecay::<Test>::put(custom);
        assert_eq!(DcfPallet::decay_settings(), custom);

        // One field above zero marks the record as set.
        let grace_only = DecaySettings {
            decay_percent: 0,
            grace_epochs: 3,
        };
        StandingDecay::<Test>::put(grace_only);
        assert_eq!(DcfPallet::decay_settings(), grace_only);
    });
}

#[test]
fn the_standing_share_reads_the_stored_share_first() {
    new_test_ext().execute_with(|| {
        ServiceRecords::<Test>::insert(9, served_record(90, 90));
        // Ninety epochs over the ramp of 180 reach half the share.
        assert_eq!(DcfPallet::standing_share(&9), Perbill::from_percent(50));

        StandingShares::<Test>::insert(9, Perbill::from_percent(20));
        assert_eq!(DcfPallet::standing_share(&9), Perbill::from_percent(20));
    });
}

#[test]
fn the_performance_share_starts_at_the_whole_share() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::performance_share(&9), Perbill::one());
        PerformanceShares::<Test>::insert(9, Perbill::from_percent(30));
        assert_eq!(DcfPallet::performance_share(&9), Perbill::from_percent(30));
    });
}

#[test]
fn the_sampling_share_starts_at_half() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::sampling_share(&9), Perbill::from_percent(50));
    });
}

#[test]
fn the_public_access_score_weighs_four_components() {
    new_test_ext().execute_with(|| {
        DeploymentChainKind::<Test>::put(ChainKind::PublicAccess);
        StandingShares::<Test>::insert(9, Perbill::one());
        PerformanceShares::<Test>::insert(9, Perbill::one());
        CustodyShares::<Test>::insert(9, 0u32);
        RoleShares::<Test>::insert(9, Perbill::one());

        // Standing 40, performance 30, custody 0 and half of sampling 10.
        let score = DcfPallet::trust_score(&9);
        assert_eq!(score.total, Perbill::from_percent(75));
        assert_eq!(score.standing, Perbill::one());
        assert_eq!(score.sampling, Perbill::from_percent(50));
        assert_eq!(
            score.role,
            Perbill::zero(),
            "the public chain leaves role out"
        );
        assert_eq!(score.attestation, Perbill::zero());
        assert_eq!(DcfPallet::trust_weight(&9), 750_000_000u128);
    });
}

#[test]
fn the_private_score_weighs_role_and_attestation() {
    new_test_ext().execute_with(|| {
        DeploymentChainKind::<Test>::put(ChainKind::Private);
        StandingShares::<Test>::insert(9, Perbill::one());
        PerformanceShares::<Test>::insert(9, Perbill::one());
        CustodyShares::<Test>::insert(9, 0u32);
        RoleShares::<Test>::insert(9, Perbill::one());
        AttestationShares::<Test>::insert(9, Perbill::from_percent(40));

        // Role 35, a share of 40 percent of attestation 25 and performance 25.
        let score = DcfPallet::trust_score(&9);
        assert_eq!(score.total, Perbill::from_percent(70));
        assert_eq!(score.role, Perbill::one());
        assert_eq!(score.attestation, Perbill::from_percent(40));
        assert_eq!(
            score.standing,
            Perbill::zero(),
            "a private chain leaves standing out"
        );
        assert_eq!(score.sampling, Perbill::zero());
        assert_eq!(DcfPallet::trust_weight(&9), 700_000_000u128);
    });
}

/// The Custody Engine reports the share in parts per million.
///
/// The Trust Score reads every component in parts per billion. The whole
/// custody share therefore reaches the score as the whole share. This test
/// holds the correct conversion.
#[test]
fn the_custody_share_reaches_the_score_as_a_share_of_one() {
    new_test_ext().execute_with(|| {
        CustodyShares::<Test>::insert(9, 1_000_000u32);
        assert_eq!(DcfPallet::trust_score(&9).custody, Perbill::one());

        CustodyShares::<Test>::remove(9);
        assert_eq!(
            DcfPallet::custody_share(&9),
            pallet_cerulea_custody::score::STARTING_SHARE
        );
        assert_eq!(
            DcfPallet::trust_score(&9).custody,
            Perbill::from_percent(50)
        );
    });
}

#[test]
fn a_vote_equivocation_returns_standing_to_zero_and_takes_the_seat() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::CurrentEpoch::<Test>::put(4);
        ServiceRecords::<Test>::insert(2, served_record(33, 4));
        StandingShares::<Test>::insert(2, Perbill::from_percent(18));
        assert!(seated(2));

        assert_eq!(DcfPallet::penalise_vote_equivocation(&2), 33);

        assert_eq!(StandingShares::<Test>::get(2), Some(Perbill::zero()));
        assert_eq!(ServiceRecords::<Test>::get(2).epochs_served, 0);
        assert_eq!(
            StandingResets::<Test>::get(2),
            Some(ResetRecord {
                misconduct: Misconduct::TwoResultsAtOneHeight,
                epoch: 4,
                epochs_destroyed: 33,
            })
        );
        assert!(
            !seated(2),
            "two results at one height take the seat at once"
        );
        assert!(has_event(Event::StandingReset {
            validator: 2,
            misconduct: Misconduct::TwoResultsAtOneHeight,
            epochs_destroyed: 33,
        }));
    });
}

#[test]
fn decay_lowers_only_validators_past_the_grace_span() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let _ = ServiceRecords::<Test>::clear(u32::MAX, None);
        let _ = StandingShares::<Test>::clear(u32::MAX, None);

        // Ninety of 180 epochs reach half the share. Three idle epochs pass the
        // grace span of two by one, so one step of five percent applies.
        ServiceRecords::<Test>::insert(1, served_record(90, 10));
        // One idle epoch sits inside the grace span.
        ServiceRecords::<Test>::insert(2, served_record(90, 12));
        // The stored share already matches the decayed share.
        ServiceRecords::<Test>::insert(3, served_record(90, 10));
        StandingShares::<Test>::insert(3, Perbill::from_parts(475_000_000));

        assert_eq!(DcfPallet::apply_standing_decay(13), 1);

        assert_eq!(
            StandingShares::<Test>::get(1),
            Some(Perbill::from_parts(475_000_000))
        );
        assert_eq!(StandingShares::<Test>::get(2), None);
        assert_eq!(
            StandingShares::<Test>::get(3),
            Some(Perbill::from_parts(475_000_000))
        );
        assert!(has_event(Event::StandingDecayed {
            validator: 1,
            idle_epochs: 3,
            standing: Perbill::from_parts(475_000_000),
        }));
        let decayed = dcf_events()
            .iter()
            .filter(|event| matches!(event, Event::StandingDecayed { .. }))
            .count();
        assert_eq!(decayed, 1, "one validator moved");
    });
}

#[test]
fn the_sample_draw_reads_the_parent_hash() {
    new_test_ext().execute_with(|| {
        let parent = H256::repeat_byte(9);
        System::set_parent_hash(parent);

        let drawn = DcfPallet::draw_sampling_positions(4, 100);
        assert_eq!(
            drawn,
            crate::engines::standing::draw_sample(
                parent,
                4,
                100,
                crate::engines::standing::DEFAULT_SAMPLE_SIZE
            )
        );
        assert_eq!(
            drawn.len(),
            crate::engines::standing::DEFAULT_SAMPLE_SIZE as usize
        );
        assert!(drawn.iter().all(|position| *position < 100));
        assert!(DcfPallet::draw_sampling_positions(4, 0).is_empty());
        assert_eq!(DcfPallet::draw_sampling_positions(4, 3).len(), 3);

        let wide = DcfPallet::draw_sampling_positions(4, 1_000);
        System::set_parent_hash(H256::repeat_byte(10));
        assert_ne!(
            DcfPallet::draw_sampling_positions(4, 1_000),
            wide,
            "a new parent hash draws a new sample"
        );
    });
}

// ---------------------------------------------------------------------------
// Assurance Engine helpers
// ---------------------------------------------------------------------------

#[test]
fn an_empty_or_weightless_domain_resolves_to_a_named_failure() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::domain_assurance(Vec::new(), 100),
            Err(ThresholdError::DomainEmpty)
        );
        assert_eq!(
            DcfPallet::domain_assurance(vec![TransactionClass::Routine], 0),
            Err(ThresholdError::TotalWeightZero)
        );
        assert!(!DcfPallet::domain_reaches_threshold(Vec::new(), 100, 100));
        assert!(!DcfPallet::domain_reaches_threshold(
            vec![TransactionClass::Routine],
            0,
            0
        ));
        assert!(DcfPallet::domain_reaches_threshold(
            vec![TransactionClass::Financial],
            100,
            67
        ));
        assert!(!DcfPallet::domain_reaches_threshold(
            vec![TransactionClass::Financial],
            100,
            66
        ));
    });
}

#[test]
fn the_compatible_event_path_deposits_the_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let event = Event::<Test>::StandingDecayed {
            validator: 3,
            idle_epochs: 4,
            standing: Perbill::from_percent(10),
        };
        DcfPallet::deposit_event_with_evm_compat(event.clone());
        assert!(has_event(event));
    });
}

// ---------------------------------------------------------------------------
// Parameter read and validation helpers
// ---------------------------------------------------------------------------

/// Write known ranges for every parameter the helpers read.
fn seed_governance() {
    GovernanceConfigStorage::<Test>::mutate(|held| {
        held.epoch_length = ParameterRange {
            min: 50,
            max: 500,
            current: 100,
        };
        held.min_stake = ParameterRange {
            min: 1_000u128,
            max: 9_000,
            current: 2_000,
        };
        held.max_validators = ParameterRange {
            min: 4,
            max: 200,
            current: 21,
        };
        held.min_performance_score = ParameterRange {
            min: 10,
            max: 90,
            current: 30,
        };
        held.slash_percent = ParameterRange {
            min: 1,
            max: 50,
            current: 10,
        };
        held.validator_reward = ParameterRange {
            min: 100u128,
            max: 50_000,
            current: 10_000,
        };
        held.leave_cooldown = ParameterRange {
            min: 10u64,
            max: 5_000,
            current: 1_000,
        };
        held.pos_weight = ParameterRange {
            min: 1_000,
            max: 9_000,
            current: 6_000,
        };
        held.poi_weight = ParameterRange {
            min: 1_000,
            max: 9_000,
            current: 4_000,
        };
        held.block_authorship_boost = ParameterRange {
            min: 1,
            max: 100,
            current: 10,
        };
        held.missed_block_penalty = ParameterRange {
            min: 2,
            max: 50,
            current: 5,
        };
        held.inference_boost_low = ParameterRange {
            min: 1,
            max: 20,
            current: 2,
        };
        held.inference_boost_medium = ParameterRange {
            min: 3,
            max: 40,
            current: 7,
        };
        held.inference_boost_high = ParameterRange {
            min: 6,
            max: 80,
            current: 11,
        };
    });
}

/// The parameters the helpers leave unread.
fn unsupported_parameters() -> Vec<ParameterType> {
    vec![
        ParameterType::HighPerformanceScore,
        ParameterType::MinParticipationRate,
        ParameterType::HighParticipationRate,
        ParameterType::TrustScoreUptimeWeight,
        ParameterType::TrustScoreInferenceWeight,
        ParameterType::TrustScoreSlashingWeight,
        ParameterType::TrustScoreMaxGrowthRate,
        ParameterType::TrustScoreMaxDecayRate,
        ParameterType::TrustScoreMinValue,
        ParameterType::TrustScoreMaxValue,
        ParameterType::TrustScoreStabilityFactor,
    ]
}

/// Hold one parameter inside its range and outside it.
fn assert_range(kind: ParameterType, inside: &[Vec<u8>], outside: &[Vec<u8>]) {
    let key = kind.encode();
    for value in inside {
        assert!(
            DcfPallet::validate_parameter_value_encoded(key.clone(), value.clone()),
            "the value {:?} sits inside the range of {:?}",
            value,
            kind
        );
    }
    for value in outside {
        assert!(
            !DcfPallet::validate_parameter_value_encoded(key.clone(), value.clone()),
            "the value {:?} sits outside the range of {:?}",
            value,
            kind
        );
    }
    assert!(
        !DcfPallet::validate_parameter_value_encoded(key, Vec::new()),
        "an empty value for {:?} reads as invalid",
        kind
    );
}

#[test]
fn every_supported_parameter_reads_back_its_current_value() {
    new_test_ext().execute_with(|| {
        seed_governance();
        let cases: Vec<(ParameterType, Vec<u8>)> = vec![
            (ParameterType::EpochLength, 100u32.encode()),
            (ParameterType::MinStake, 2_000u128.encode()),
            (ParameterType::MaxValidators, 21u32.encode()),
            (ParameterType::MinPerformanceScore, 30u64.encode()),
            (ParameterType::SlashPercent, 10u32.encode()),
            (ParameterType::ValidatorReward, 10_000u128.encode()),
            (ParameterType::LeaveCooldown, 1_000u64.encode()),
            (ParameterType::PosWeight, 6_000u64.encode()),
            (ParameterType::PoiWeight, 4_000u64.encode()),
            (ParameterType::BlockAuthorshipBoost, 10u64.encode()),
            (ParameterType::MissedBlockPenalty, 5u64.encode()),
            (ParameterType::InferenceBoostLow, 2u64.encode()),
            (ParameterType::InferenceBoostMedium, 7u64.encode()),
            (ParameterType::InferenceBoostHigh, 11u64.encode()),
        ];
        for (kind, expected) in cases {
            assert_eq!(
                DcfPallet::get_parameter_value_encoded(kind.encode()),
                Some(expected),
                "the parameter {:?} reads back its current value",
                kind
            );
        }
    });
}

#[test]
fn an_unsupported_or_unreadable_parameter_reads_back_nothing() {
    new_test_ext().execute_with(|| {
        seed_governance();
        for kind in unsupported_parameters() {
            assert_eq!(
                DcfPallet::get_parameter_value_encoded(kind.encode()),
                None,
                "the parameter {:?} reads back nothing",
                kind
            );
        }
        assert_eq!(DcfPallet::get_parameter_value_encoded(Vec::new()), None);
        assert_eq!(DcfPallet::get_parameter_value_encoded(vec![250u8]), None);
    });
}

#[test]
fn the_four_byte_parameters_validate_inside_their_ranges() {
    new_test_ext().execute_with(|| {
        seed_governance();
        assert_range(
            ParameterType::EpochLength,
            &[50u32.encode(), 500u32.encode()],
            &[49u32.encode(), 501u32.encode()],
        );
        assert_range(
            ParameterType::MaxValidators,
            &[4u32.encode(), 200u32.encode()],
            &[3u32.encode(), 201u32.encode()],
        );
        assert_range(
            ParameterType::SlashPercent,
            &[1u32.encode(), 50u32.encode()],
            &[0u32.encode(), 51u32.encode()],
        );
    });
}

#[test]
fn the_eight_byte_parameters_validate_inside_their_ranges() {
    new_test_ext().execute_with(|| {
        seed_governance();
        let ranges: [(ParameterType, u64, u64); 9] = [
            (ParameterType::MinPerformanceScore, 10, 90),
            (ParameterType::LeaveCooldown, 10, 5_000),
            (ParameterType::PosWeight, 1_000, 9_000),
            (ParameterType::PoiWeight, 1_000, 9_000),
            (ParameterType::BlockAuthorshipBoost, 1, 100),
            (ParameterType::MissedBlockPenalty, 2, 50),
            (ParameterType::InferenceBoostLow, 1, 20),
            (ParameterType::InferenceBoostMedium, 3, 40),
            (ParameterType::InferenceBoostHigh, 6, 80),
        ];
        for (kind, min, max) in ranges {
            assert_range(
                kind,
                &[min.encode(), max.encode()],
                // A four byte value falls short of the eight bytes the type reads.
                &[(min - 1).encode(), (max + 1).encode(), 50u32.encode()],
            );
        }
    });
}

#[test]
fn the_balance_parameters_validate_inside_their_ranges() {
    new_test_ext().execute_with(|| {
        seed_governance();
        assert_range(
            ParameterType::MinStake,
            &[1_000u128.encode(), 9_000u128.encode()],
            &[999u128.encode(), 9_001u128.encode(), 2_000u64.encode()],
        );
        assert_range(
            ParameterType::ValidatorReward,
            &[100u128.encode(), 50_000u128.encode()],
            &[99u128.encode(), 50_001u128.encode()],
        );
    });
}

#[test]
fn an_unsupported_or_unreadable_parameter_never_validates() {
    new_test_ext().execute_with(|| {
        seed_governance();
        for kind in unsupported_parameters() {
            assert!(
                !DcfPallet::validate_parameter_value_encoded(kind.encode(), 50u64.encode()),
                "the parameter {:?} never validates",
                kind
            );
        }
        assert!(!DcfPallet::validate_parameter_value_encoded(
            Vec::new(),
            100u32.encode()
        ));
        assert!(!DcfPallet::validate_parameter_value_encoded(
            vec![250u8],
            100u32.encode()
        ));
    });
}
