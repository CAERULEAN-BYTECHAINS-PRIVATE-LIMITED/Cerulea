//! Further tests of the audit module.
//!
//! These tests drive every call, every read and every refusal of the log.
//! Each refusal test also reads the fix sentence the error carries in the
//! runtime metadata and checks the sentence word for word.

use crate::{
    contract::{AuditRequest, AuditSink, Sentence, WorkClass},
    mock::*,
    ActorIndex, DefinitionNames, Definitions, Entries, Error, Event, EventIndex, LogDigest,
    NextSequence, Pallet, Subscription, Subscriptions,
};
use alloc::{string::String, vec, vec::Vec};
use cerulea_blueprint::actions::TransactionClass;
use codec::{Decode, Encode};
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use scale_info::{TypeDef, TypeInfo};
use sp_runtime::DispatchError;

const DEPLOYMENT: DeploymentId = [9u8; 32];
const OTHER: DeploymentId = [4u8; 32];
const REGISTRAR: u64 = 1;
const CLERK: u64 = 2;
const READER: u64 = 3;
const AUDITOR: u64 = 4;

fn event(name: &str) -> BoundedName {
    name_of(name).expect("the event name fits")
}

fn summary(text: &str) -> BoundedVec<u8, ConstU32<256>> {
    BoundedVec::try_from(text.as_bytes().to_vec()).expect("the summary fits")
}

fn reason(text: &str) -> Sentence {
    BoundedVec::try_from(text.as_bytes().to_vec()).expect("the reason fits")
}

fn payload(bytes: &[u8]) -> BoundedVec<u8, ConstU32<4_096>> {
    BoundedVec::try_from(bytes.to_vec()).expect("the payload fits")
}

fn events(names: &[&str]) -> BoundedVec<BoundedName, ConstU32<16>> {
    BoundedVec::try_from(names.iter().map(|name| event(name)).collect::<Vec<_>>())
        .expect("the event list fits")
}

fn classes(held: &[WorkClass]) -> BoundedVec<WorkClass, ConstU32<4>> {
    BoundedVec::try_from(held.to_vec()).expect("the class list fits")
}

fn define(name: &str, class: WorkClass) {
    assert_ok!(Audit::define_event(
        RuntimeOrigin::signed(REGISTRAR),
        DEPLOYMENT,
        event(name),
        class,
        summary("The register recorded one change.")
    ));
}

fn emit(actor: u64, name: &str, bytes: &[u8]) {
    assert_ok!(Audit::emit(
        RuntimeOrigin::signed(actor),
        DEPLOYMENT,
        event(name),
        reason("The clerk recorded the parcel."),
        payload(bytes)
    ));
}

/// The fix sentence one error carries in the runtime metadata.
///
/// The metadata splits a long doc comment into lines. The helper joins the
/// lines back into the sentence a reader sees.
fn fix_sentence(error: Error<Test>) -> String {
    let wanted: &str = match DispatchError::from(error) {
        DispatchError::Module(module) => module.message.expect("the error names its variant"),
        other => panic!("the error maps to a module error, found {other:?}"),
    };
    let TypeDef::Variant(variants) = Error::<Test>::type_info().type_def else {
        panic!("the error type lists variants");
    };
    let variant = variants
        .variants
        .iter()
        .find(|variant| variant.name == wanted)
        .expect("the metadata holds the variant");
    variant
        .docs
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

fn recorded(expected: Event<Test>) -> bool {
    System::events()
        .iter()
        .any(|held| held.event == RuntimeEvent::Audit(expected.clone()))
}

#[test]
fn every_error_names_its_fix_in_plain_words() {
    let expected: [(Error<Test>, &str); 11] = [
        (
            Error::NameTooLong,
            "The name runs past 128 bytes. Shorten the event name.",
        ),
        (
            Error::EventUnknown,
            "The registry lacks that event. Define the event before emitting it.",
        ),
        (
            Error::ClassMismatch,
            "The registry already holds that event under another class. \
             Emit the event under its registered class.",
        ),
        (
            Error::DefinitionLimitReached,
            "The deployment holds its full count of event definitions. \
             Raise the definition limit in the runtime.",
        ),
        (
            Error::ReasonTooLong,
            "The reason runs past 256 bytes. Shorten the sentence.",
        ),
        (
            Error::SummaryTooLong,
            "The summary runs past 256 bytes. Shorten the sentence.",
        ),
        (
            Error::PayloadTooLarge,
            "The payload runs past the payload limit. Shorten the payload, \
             or raise the limit in the runtime.",
        ),
        (
            Error::EntryAbsent,
            "The log holds no entry under that sequence number. \
             Query the log for the sequence number of the entry.",
        ),
        (
            Error::CorrectionChained,
            "A correction names an entry that is itself a correction. \
             Correct the original entry.",
        ),
        (
            Error::SubscriptionAbsent,
            "The deployment holds no subscription for that account. \
             Open a subscription before closing one.",
        ),
        (
            Error::LogFull,
            "The log reached its last sequence number. Start a new deployment log.",
        ),
    ];
    for (error, sentence) in expected {
        assert_eq!(fix_sentence(error), sentence);
    }
}

#[test]
fn every_call_refuses_an_unsigned_origin() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        emit(CLERK, "parcel_registered", &[1]);
        assert_noop!(
            Audit::define_event(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                event("parcel_retired"),
                WorkClass::Routine,
                summary("The register retired one parcel.")
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Audit::emit(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[])
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Audit::correct(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                0,
                reason("The survey gives another area."),
                payload(&[])
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Audit::subscribe(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                BoundedVec::default(),
                BoundedVec::default()
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Audit::unsubscribe(RuntimeOrigin::none(), DEPLOYMENT),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_repeated_definition_under_its_class_keeps_the_first_registration() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Financial);
        let first = Definitions::<Test>::get(DEPLOYMENT, event("parcel_registered"))
            .expect("the registry holds the definition");
        assert_eq!(first.registered_at, 1);
        assert_eq!(
            first.summary.as_slice(),
            b"The register recorded one change."
        );
        assert!(recorded(Event::EventDefined {
            deployment: DEPLOYMENT,
            event: event("parcel_registered"),
            class: WorkClass::Financial,
        }));

        System::set_block_number(5);
        System::reset_events();
        assert_ok!(Audit::define_event(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            WorkClass::Financial,
            summary("A second summary for the same event.")
        ));

        let kept = Definitions::<Test>::get(DEPLOYMENT, event("parcel_registered"))
            .expect("the registry keeps the definition");
        assert_eq!(kept, first);
        assert_eq!(
            DefinitionNames::<Test>::get(DEPLOYMENT).into_inner(),
            vec![event("parcel_registered")]
        );
        assert!(System::events().is_empty());
    });
}

#[test]
fn a_class_mismatch_names_its_fix() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        assert_noop!(
            Audit::define_event(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                event("parcel_registered"),
                WorkClass::Constitutional,
                summary("The register recorded one change.")
            ),
            Error::<Test>::ClassMismatch
        );
        assert_eq!(
            fix_sentence(Error::<Test>::ClassMismatch),
            "The registry already holds that event under another class. \
             Emit the event under its registered class."
        );
    });
}

#[test]
fn the_registry_declines_a_definition_past_its_limit() {
    new_test_ext().execute_with(|| {
        for step in 0..64u32 {
            let name = alloc::format!("event_{step}");
            define(&name, WorkClass::Routine);
        }
        assert_eq!(DefinitionNames::<Test>::get(DEPLOYMENT).len(), 64);
        assert_noop!(
            Audit::define_event(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                event("event_64"),
                WorkClass::Routine,
                summary("The register recorded one change.")
            ),
            Error::<Test>::DefinitionLimitReached
        );
        assert_eq!(
            fix_sentence(Error::<Test>::DefinitionLimitReached),
            "The deployment holds its full count of event definitions. \
             Raise the definition limit in the runtime."
        );

        // The limit belongs to one deployment. Another deployment still
        // registers its first definition.
        assert_ok!(Audit::define_event(
            RuntimeOrigin::signed(REGISTRAR),
            OTHER,
            event("event_64"),
            WorkClass::Routine,
            summary("The register recorded one change.")
        ));
        assert_eq!(DefinitionNames::<Test>::get(OTHER).len(), 1);
    });
}

#[test]
fn an_unknown_event_names_its_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[])
            ),
            Error::<Test>::EventUnknown
        );
        assert_eq!(
            fix_sentence(Error::<Test>::EventUnknown),
            "The registry lacks that event. Define the event before emitting it."
        );
    });
}

#[test]
fn an_emit_writes_the_entry_the_indexes_and_the_running_digest() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Financial);
        System::reset_events();
        emit(CLERK, "parcel_registered", &[5, 6]);

        let entry = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the log holds the entry");
        assert_eq!(entry.sequence, 0);
        assert_eq!(entry.module, event("audit"));
        assert_eq!(entry.class, WorkClass::Financial);
        assert_eq!(entry.block, 1);
        assert_eq!(entry.at_millis, 1_000_000);
        assert_eq!(entry.reason.as_slice(), b"The clerk recorded the parcel.");

        // The first digest folds the genesis digest of the deployment with
        // the body of the entry.
        let mut folded = Pallet::<Test>::genesis_digest(&DEPLOYMENT).to_vec();
        folded.extend_from_slice(&entry.body_bytes());
        assert_eq!(entry.digest, sp_crypto_hashing::blake2_256(&folded));
        assert_eq!(LogDigest::<Test>::get(DEPLOYMENT), Some(entry.digest));

        assert!(ActorIndex::<Test>::contains_key((DEPLOYMENT, CLERK, 0u64)));
        assert!(EventIndex::<Test>::contains_key((
            DEPLOYMENT,
            event("parcel_registered"),
            0u64
        )));
        assert_eq!(NextSequence::<Test>::get(DEPLOYMENT), 1);
        assert!(recorded(Event::EntryRecorded {
            deployment: DEPLOYMENT,
            sequence: 0,
            event: event("parcel_registered"),
            module: event("audit"),
            class: WorkClass::Financial,
            actor: CLERK,
        }));

        // The second digest folds the first one.
        emit(READER, "parcel_registered", &[7]);
        let second = Pallet::<Test>::entry(&DEPLOYMENT, 1).expect("the log holds the entry");
        let mut folded = entry.digest.to_vec();
        folded.extend_from_slice(&second.body_bytes());
        assert_eq!(second.digest, sp_crypto_hashing::blake2_256(&folded));
        assert_eq!(LogDigest::<Test>::get(DEPLOYMENT), Some(second.digest));
    });
}

#[test]
fn two_deployments_start_from_different_genesis_digests() {
    assert_ne!(
        Pallet::<Test>::genesis_digest(&DEPLOYMENT),
        Pallet::<Test>::genesis_digest(&OTHER)
    );
    assert_eq!(
        Pallet::<Test>::genesis_digest(&DEPLOYMENT),
        sp_crypto_hashing::blake2_256(&(crate::AUDIT_DOMAIN, DEPLOYMENT).encode())
    );
}

#[test]
fn a_correction_records_its_link_and_its_event() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Structural);
        emit(CLERK, "parcel_registered", &[1]);
        System::reset_events();

        assert_ok!(Audit::correct(
            RuntimeOrigin::signed(AUDITOR),
            DEPLOYMENT,
            0,
            reason("The survey gives the area as 420 square metres."),
            payload(&[2])
        ));

        let correction = Pallet::<Test>::entry(&DEPLOYMENT, 1).expect("the correction lands");
        assert_eq!(correction.class, WorkClass::Structural);
        assert_eq!(correction.module, event("audit"));
        assert_eq!(correction.actor, AUDITOR);
        assert_eq!(correction.payload.as_slice(), &[2]);
        assert_eq!(
            correction.reason.as_slice(),
            b"The survey gives the area as 420 square metres."
        );
        assert!(recorded(Event::EntryCorrected {
            deployment: DEPLOYMENT,
            original: 0,
            correction: 1,
        }));
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));
    });
}

#[test]
fn a_chained_correction_and_an_absent_entry_name_their_fixes() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        emit(CLERK, "parcel_registered", &[1]);
        assert_ok!(Audit::correct(
            RuntimeOrigin::signed(REGISTRAR),
            DEPLOYMENT,
            0,
            reason("The survey gives another area."),
            payload(&[2])
        ));
        assert_noop!(
            Audit::correct(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                1,
                reason("The survey gives a third area."),
                payload(&[3])
            ),
            Error::<Test>::CorrectionChained
        );
        assert_eq!(
            fix_sentence(Error::<Test>::CorrectionChained),
            "A correction names an entry that is itself a correction. \
             Correct the original entry."
        );
        assert_noop!(
            Audit::correct(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                7,
                reason("The survey gives another area."),
                payload(&[3])
            ),
            Error::<Test>::EntryAbsent
        );
        assert_eq!(
            fix_sentence(Error::<Test>::EntryAbsent),
            "The log holds no entry under that sequence number. \
             Query the log for the sequence number of the entry."
        );
    });
}

#[test]
fn a_full_log_declines_every_append() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        NextSequence::<Test>::insert(DEPLOYMENT, u64::MAX);
        assert_noop!(
            Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[])
            ),
            Error::<Test>::LogFull
        );
        assert_eq!(
            <Pallet<Test> as AuditSink>::record(AuditRequest::new(
                DEPLOYMENT,
                "workflow",
                "parcel_registered",
                &CLERK,
                "The workflow recorded the parcel."
            )),
            Err(Error::<Test>::LogFull.into())
        );
        assert_eq!(
            fix_sentence(Error::<Test>::LogFull),
            "The log reached its last sequence number. Start a new deployment log."
        );
    });
}

#[test]
fn a_module_record_declines_an_oversized_field() {
    new_test_ext().execute_with(|| {
        let long_name = "n".repeat(129);
        let long_reason = "r".repeat(257);

        assert_eq!(
            <Pallet<Test> as AuditSink>::record(AuditRequest::new(
                DEPLOYMENT,
                "workflow",
                &long_name,
                &CLERK,
                "The workflow recorded its work."
            )),
            Err(Error::<Test>::NameTooLong.into())
        );
        assert_eq!(
            <Pallet<Test> as AuditSink>::record(AuditRequest::new(
                DEPLOYMENT,
                &long_name,
                "instance_started",
                &CLERK,
                "The workflow recorded its work."
            )),
            Err(Error::<Test>::NameTooLong.into())
        );
        assert_eq!(
            fix_sentence(Error::<Test>::NameTooLong),
            "The name runs past 128 bytes. Shorten the event name."
        );

        assert_eq!(
            <Pallet<Test> as AuditSink>::record(AuditRequest::new(
                DEPLOYMENT,
                "workflow",
                "instance_started",
                &CLERK,
                &long_reason
            )),
            Err(Error::<Test>::ReasonTooLong.into())
        );
        assert_eq!(
            fix_sentence(Error::<Test>::ReasonTooLong),
            "The reason runs past 256 bytes. Shorten the sentence."
        );

        assert_eq!(
            <Pallet<Test> as AuditSink>::record(
                AuditRequest::new(
                    DEPLOYMENT,
                    "workflow",
                    "instance_started",
                    &CLERK,
                    "The workflow recorded its work."
                )
                .carrying(vec![0u8; 4_097])
            ),
            Err(Error::<Test>::PayloadTooLarge.into())
        );
        assert_eq!(
            fix_sentence(Error::<Test>::PayloadTooLarge),
            "The payload runs past the payload limit. Shorten the payload, \
             or raise the limit in the runtime."
        );

        // A declined record leaves the registry and the log untouched.
        assert!(DefinitionNames::<Test>::get(DEPLOYMENT).is_empty());
        assert_eq!(Pallet::<Test>::length(&DEPLOYMENT), 0);
        assert_eq!(LogDigest::<Test>::get(DEPLOYMENT), None);
    });
}

#[test]
fn a_module_record_carries_its_payload_and_its_correction_link() {
    new_test_ext().execute_with(|| {
        let first = <Pallet<Test> as AuditSink>::record(
            AuditRequest::new(
                DEPLOYMENT,
                "tier",
                "fee_settled",
                &CLERK,
                "The tier module settled the fee.",
            )
            .classed(WorkClass::Financial)
            .carrying(vec![1, 2, 3]),
        )
        .expect("the log takes the record");
        assert_eq!(first, 0);

        let second = <Pallet<Test> as AuditSink>::record(
            AuditRequest::new(
                DEPLOYMENT,
                "tier",
                "fee_settled",
                &AUDITOR,
                "The auditor corrected the settled fee.",
            )
            .classed(WorkClass::Financial)
            .carrying(vec![4])
            .correcting(first),
        )
        .expect("the log takes the correction");
        assert_eq!(second, 1);

        let held = Pallet::<Test>::query(&DEPLOYMENT, 0, 16);
        assert_eq!(held.len(), 2);
        assert_eq!(held[0].payload, vec![1, 2, 3]);
        assert_eq!(held[0].corrects, None);
        assert_eq!(held[0].class, WorkClass::Financial);
        assert_eq!(held[1].payload, vec![4]);
        assert_eq!(held[1].corrects, Some(0));
        assert_eq!(held[1].reason, "The auditor corrected the settled fee.");

        // The first record registered the event once, under the module that
        // named it, with the reason as its summary.
        let definition = Definitions::<Test>::get(DEPLOYMENT, event("fee_settled"))
            .expect("the record registered its event");
        assert_eq!(definition.module, event("tier"));
        assert_eq!(
            definition.summary.as_slice(),
            b"The tier module settled the fee."
        );
        assert_eq!(DefinitionNames::<Test>::get(DEPLOYMENT).len(), 1);
    });
}

#[test]
fn a_record_request_starts_as_routine_work_and_takes_each_setting() {
    let actor = CLERK;
    let plain = AuditRequest::new(
        DEPLOYMENT,
        "rules",
        "precondition_refused",
        &actor,
        "The rule refused the action.",
    );
    assert_eq!(plain.class, WorkClass::Routine);
    assert!(plain.payload.is_empty());
    assert_eq!(plain.corrects, None);
    assert_eq!(plain.module, "rules");
    assert_eq!(plain.event, "precondition_refused");
    assert_eq!(plain.reason, "The rule refused the action.");

    let shaped = plain
        .clone()
        .classed(WorkClass::Constitutional)
        .carrying(vec![9, 9])
        .correcting(12);
    assert_eq!(shaped.class, WorkClass::Constitutional);
    assert_eq!(shaped.payload, vec![9, 9]);
    assert_eq!(shaped.corrects, Some(12));
    assert_ne!(shaped, plain);
}

#[test]
fn a_work_class_names_itself_and_matches_the_blueprint_class() {
    let pairs = [
        (WorkClass::Routine, TransactionClass::Routine, "routine"),
        (
            WorkClass::Financial,
            TransactionClass::Financial,
            "financial",
        ),
        (
            WorkClass::Structural,
            TransactionClass::Structural,
            "structural",
        ),
        (
            WorkClass::Constitutional,
            TransactionClass::Constitutional,
            "constitutional",
        ),
    ];
    for (work, blueprint, label) in pairs {
        assert_eq!(work.label(), label);
        assert_eq!(WorkClass::from(blueprint), work);
        assert_eq!(TransactionClass::from(work), blueprint);
        assert_eq!(work.holds_delay(), work == WorkClass::Constitutional);
    }
    assert_eq!(WorkClass::default(), WorkClass::Routine);
    assert!(WorkClass::Routine < WorkClass::Financial);
    assert!(WorkClass::Structural < WorkClass::Constitutional);
    let encoded = WorkClass::Structural.encode();
    assert_eq!(
        WorkClass::decode(&mut encoded.as_slice()).expect("the class decodes"),
        WorkClass::Structural
    );
}

#[test]
fn the_body_encoding_leaves_the_digest_out() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        emit(CLERK, "parcel_registered", &[1]);
        let entry = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the log holds the entry");

        let mut reshaped = entry.clone();
        reshaped.digest = [0u8; 32];
        assert_eq!(reshaped.body_bytes(), entry.body_bytes());

        reshaped.corrects = Some(3);
        assert_ne!(reshaped.body_bytes(), entry.body_bytes());
    });
}

#[test]
fn a_subscription_follows_by_start_by_event_and_by_class() {
    let open: Subscription<ConstU32<16>> = Subscription {
        events: BoundedVec::default(),
        classes: BoundedVec::default(),
        since: 3,
    };
    assert!(!open.follows(&event("parcel_registered"), WorkClass::Routine, 2));
    assert!(open.follows(&event("parcel_registered"), WorkClass::Routine, 3));
    assert!(open.follows(&event("anything"), WorkClass::Constitutional, 40));

    let narrow: Subscription<ConstU32<16>> = Subscription {
        events: events(&["parcel_retired", "parcel_registered"]),
        classes: classes(&[WorkClass::Structural]),
        since: 0,
    };
    assert!(narrow.follows(&event("parcel_registered"), WorkClass::Structural, 0));
    assert!(!narrow.follows(&event("parcel_registered"), WorkClass::Routine, 0));
    assert!(!narrow.follows(&event("parcel_split"), WorkClass::Structural, 0));
}

#[test]
fn a_subscriber_reads_only_what_arrives_after_it_subscribes() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        define("fee_settled", WorkClass::Financial);
        emit(CLERK, "parcel_registered", &[0]);
        emit(CLERK, "fee_settled", &[1]);

        System::reset_events();
        assert_ok!(Audit::subscribe(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            BoundedVec::default(),
            classes(&[WorkClass::Financial])
        ));
        assert!(recorded(Event::SubscriptionOpened {
            deployment: DEPLOYMENT,
            subscriber: READER,
        }));
        let held = Subscriptions::<Test>::get(DEPLOYMENT, READER).expect("the subscription lands");
        assert_eq!(held.since, 2);

        emit(CLERK, "parcel_registered", &[2]);
        emit(CLERK, "fee_settled", &[3]);
        emit(CLERK, "fee_settled", &[4]);

        let read = Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &READER, 0, 16);
        assert_eq!(
            read.iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![3, 4]
        );
        assert!(read
            .iter()
            .all(|record| record.class == WorkClass::Financial));

        // A later start and a batch limit both narrow the read.
        let later = Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &READER, 4, 16);
        assert_eq!(later.len(), 1);
        assert_eq!(later[0].sequence, 4);
        assert_eq!(
            Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &READER, 0, 1).len(),
            1
        );

        // An account without a subscription reads nothing.
        assert!(Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &AUDITOR, 0, 16).is_empty());
    });
}

#[test]
fn the_subscriber_list_follows_the_event_and_the_class() {
    new_test_ext().execute_with(|| {
        assert_ok!(Audit::subscribe(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            events(&["fee_settled"]),
            BoundedVec::default()
        ));
        assert_ok!(Audit::subscribe(
            RuntimeOrigin::signed(AUDITOR),
            DEPLOYMENT,
            BoundedVec::default(),
            classes(&[WorkClass::Constitutional])
        ));

        let fee =
            Pallet::<Test>::subscribers_for(&DEPLOYMENT, "fee_settled", WorkClass::Financial, 16);
        assert_eq!(fee, vec![READER]);

        let mut amendment = Pallet::<Test>::subscribers_for(
            &DEPLOYMENT,
            "fee_settled",
            WorkClass::Constitutional,
            16,
        );
        amendment.sort();
        assert_eq!(amendment, vec![READER, AUDITOR]);

        assert_eq!(
            Pallet::<Test>::subscribers_for(
                &DEPLOYMENT,
                "fee_settled",
                WorkClass::Constitutional,
                1
            )
            .len(),
            1
        );
        assert!(Pallet::<Test>::subscribers_for(
            &DEPLOYMENT,
            "parcel_registered",
            WorkClass::Routine,
            16
        )
        .is_empty());
        assert!(Pallet::<Test>::subscribers_for(
            &DEPLOYMENT,
            &"n".repeat(129),
            WorkClass::Routine,
            16
        )
        .is_empty());
        assert!(
            Pallet::<Test>::subscribers_for(&OTHER, "fee_settled", WorkClass::Financial, 16)
                .is_empty()
        );
    });
}

#[test]
fn closing_a_subscription_records_the_close_and_names_the_fix_for_a_second_close() {
    new_test_ext().execute_with(|| {
        assert_ok!(Audit::subscribe(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            BoundedVec::default(),
            BoundedVec::default()
        ));
        assert_ok!(Audit::unsubscribe(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT
        ));
        assert!(recorded(Event::SubscriptionClosed {
            deployment: DEPLOYMENT,
            subscriber: READER,
        }));
        assert!(!Subscriptions::<Test>::contains_key(DEPLOYMENT, READER));
        assert_noop!(
            Audit::unsubscribe(RuntimeOrigin::signed(READER), DEPLOYMENT),
            Error::<Test>::SubscriptionAbsent
        );
        assert_eq!(
            fix_sentence(Error::<Test>::SubscriptionAbsent),
            "The deployment holds no subscription for that account. \
             Open a subscription before closing one."
        );
    });
}

#[test]
fn a_read_returns_the_entry_in_plain_text() {
    new_test_ext().execute_with(|| {
        set_clock(1_750_000_000_123);
        define("parcel_registered", WorkClass::Routine);
        emit(CLERK, "parcel_registered", &[8]);

        let record = Pallet::<Test>::read(&DEPLOYMENT, 0).expect("the reader finds the entry");
        let stored = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the log holds the entry");
        assert_eq!(record.sequence, 0);
        assert_eq!(record.event, "parcel_registered");
        assert_eq!(record.module, "audit");
        assert_eq!(record.reason, "The clerk recorded the parcel.");
        assert_eq!(record.at_millis, 1_750_000_000_123);
        assert_eq!(record.block, 1);
        assert_eq!(record.actor, CLERK);
        assert_eq!(record.payload, vec![8]);
        assert_eq!(record.digest, stored.digest);

        assert_eq!(Pallet::<Test>::read(&DEPLOYMENT, 1), None);
        assert_eq!(Pallet::<Test>::read(&OTHER, 0), None);
    });
}

#[test]
fn every_query_skips_a_missing_entry_and_honours_its_bounds() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        define("parcel_retired", WorkClass::Structural);
        for step in 0..4u8 {
            emit(CLERK, "parcel_registered", &[step]);
        }
        emit(READER, "parcel_retired", &[4]);

        // Remove one stored entry and leave its index rows in place.
        Entries::<Test>::remove(DEPLOYMENT, 1);

        let span = Pallet::<Test>::query(&DEPLOYMENT, 0, 16);
        assert_eq!(
            span.iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![0, 2, 3, 4]
        );

        let by_actor = Pallet::<Test>::query_by_actor(&DEPLOYMENT, &CLERK, 0, 16);
        assert_eq!(
            by_actor
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
        assert_eq!(
            Pallet::<Test>::query_by_actor(&DEPLOYMENT, &CLERK, 2, 1)
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![2]
        );
        assert!(Pallet::<Test>::query_by_actor(&DEPLOYMENT, &AUDITOR, 0, 16).is_empty());

        let by_event = Pallet::<Test>::query_by_event(&DEPLOYMENT, "parcel_registered", 0, 16);
        assert_eq!(
            by_event
                .iter()
                .map(|record| record.sequence)
                .collect::<Vec<_>>(),
            vec![0, 2, 3]
        );
        assert_eq!(
            Pallet::<Test>::query_by_event(&DEPLOYMENT, "parcel_registered", 0, 2).len(),
            2
        );
        assert!(Pallet::<Test>::query_by_event(&DEPLOYMENT, &"n".repeat(129), 0, 16).is_empty());
        assert!(Pallet::<Test>::query_by_event(&DEPLOYMENT, "parcel_split", 0, 16).is_empty());

        // A start past the end of the log reads nothing.
        assert!(Pallet::<Test>::query(&DEPLOYMENT, 9, 16).is_empty());
    });
}

#[test]
fn a_read_never_exceeds_the_runtime_batch_limit() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for step in 0..260u32 {
            emit(CLERK, "parcel_registered", &step.to_le_bytes());
        }
        assert_eq!(Pallet::<Test>::query(&DEPLOYMENT, 0, u32::MAX).len(), 256);
        assert_eq!(
            Pallet::<Test>::query_by_actor(&DEPLOYMENT, &CLERK, 0, u32::MAX).len(),
            256
        );
        assert_eq!(Pallet::<Test>::query(&DEPLOYMENT, 256, u32::MAX).len(), 4);
    });
}

#[test]
fn an_export_of_an_empty_span_decodes_to_an_empty_list() {
    new_test_ext().execute_with(|| {
        let bytes = Pallet::<Test>::export(&DEPLOYMENT, 0, 16);
        let held: Vec<crate::LogRecord<u64, u64>> =
            Decode::decode(&mut bytes.as_slice()).expect("the export decodes");
        assert!(held.is_empty());

        define("parcel_registered", WorkClass::Routine);
        for step in 0..3u8 {
            emit(CLERK, "parcel_registered", &[step]);
        }
        let bytes = Pallet::<Test>::export(&DEPLOYMENT, 1, 1);
        let held: Vec<crate::LogRecord<u64, u64>> =
            Decode::decode(&mut bytes.as_slice()).expect("the export decodes");
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].sequence, 1);
        assert_eq!(held[0].payload, vec![1]);
    });
}

#[test]
fn the_chain_check_covers_its_edges() {
    new_test_ext().execute_with(|| {
        // An empty log verifies.
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));

        define("parcel_registered", WorkClass::Routine);
        for step in 0..5u8 {
            emit(CLERK, "parcel_registered", &[step]);
        }
        // A start at the end of the log verifies. A short span verifies.
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 5, 16));
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 2, 2));

        // A changed entry past the checked span leaves the span whole.
        let mut changed = Pallet::<Test>::entry(&DEPLOYMENT, 4).expect("the log holds the entry");
        changed.payload = payload(&[99]);
        Entries::<Test>::insert(DEPLOYMENT, 4, changed);
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 4));
        assert!(!Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));
        assert!(!Pallet::<Test>::verify_chain(&DEPLOYMENT, 4, 16));

        // A removed entry inside the span breaks the check. A removed entry
        // just before the span breaks the check too, since the span loses
        // its anchor digest.
        Entries::<Test>::remove(DEPLOYMENT, 2);
        assert!(!Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));
        assert!(!Pallet::<Test>::verify_chain(&DEPLOYMENT, 3, 1));
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 2));
    });
}

#[test]
fn the_test_clock_starts_at_its_default_and_follows_each_change() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        emit(CLERK, "parcel_registered", &[0]);
        set_clock(2_000_000);
        emit(CLERK, "parcel_registered", &[1]);
        let held = Pallet::<Test>::query(&DEPLOYMENT, 0, 16);
        assert_eq!(held[0].at_millis, 1_000_000);
        assert_eq!(held[1].at_millis, 2_000_000);
    });
}
