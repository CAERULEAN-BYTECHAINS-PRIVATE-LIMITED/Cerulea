//! The tests of the audit module.

use crate::{
    contract::{AuditRequest, AuditSink, LogRecord, Sentence, WorkClass},
    mock::*,
    Definitions, Entries, Error, NextSequence, Pallet,
};
use codec::Decode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const REGISTRAR: u64 = 1;
const CLERK: u64 = 2;
const READER: u64 = 3;

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

fn define(name: &str, class: WorkClass) {
    assert_ok!(Audit::define_event(
        RuntimeOrigin::signed(REGISTRAR),
        DEPLOYMENT,
        event(name),
        class,
        summary("The register recorded one change.")
    ));
}

#[test]
fn a_definition_registers_and_reads_back() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        let held = Definitions::<Test>::get(DEPLOYMENT, event("parcel_registered"))
            .expect("the registry holds the definition");
        assert_eq!(held.class, WorkClass::Routine);
        assert_eq!(held.module, event("audit"));
    });
}

#[test]
fn a_definition_under_another_class_declines() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        assert_noop!(
            Audit::define_event(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                event("parcel_registered"),
                WorkClass::Structural,
                summary("The register recorded one change.")
            ),
            Error::<Test>::ClassMismatch
        );
    });
}

#[test]
fn an_emitted_entry_carries_its_actor_and_its_time() {
    new_test_ext().execute_with(|| {
        set_clock(1_700_000_000_000);
        define("parcel_registered", WorkClass::Routine);
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[7, 7, 7])
        ));

        let entry = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the log holds the entry");
        assert_eq!(entry.actor, CLERK);
        assert_eq!(entry.at_millis, 1_700_000_000_000);
        assert_eq!(entry.class, WorkClass::Routine);
        assert_eq!(entry.payload.as_slice(), &[7, 7, 7]);
        assert_eq!(entry.corrects, None);
    });
}

#[test]
fn every_recorded_event_appears_in_the_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        for (module, name, actor) in [
            ("workflow", "instance_started", REGISTRAR),
            ("rules", "precondition_refused", CLERK),
            ("governance", "proposal_opened", READER),
        ] {
            assert_ok!(<Pallet<Test> as AuditSink>::record(AuditRequest::new(
                DEPLOYMENT,
                module,
                name,
                &actor,
                "The module recorded its work."
            )));
        }

        assert_eq!(Pallet::<Test>::length(&DEPLOYMENT), 3);
        let held = Pallet::<Test>::query(&DEPLOYMENT, 0, 16);
        assert_eq!(held.len(), 3);
        assert_eq!(held[0].actor, REGISTRAR);
        assert_eq!(held[0].module, "workflow");
        assert_eq!(held[1].actor, CLERK);
        assert_eq!(held[1].module, "rules");
        assert_eq!(held[2].actor, READER);
        assert_eq!(held[2].module, "governance");
    });
}

#[test]
fn a_module_record_registers_the_event_it_names() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Pallet<Test> as AuditSink>::record(
            AuditRequest::new(
                DEPLOYMENT,
                "evolution",
                "migration_applied",
                &REGISTRAR,
                "The steward applied the migration."
            )
            .classed(WorkClass::Structural)
        ));
        let held = Definitions::<Test>::get(DEPLOYMENT, event("migration_applied"))
            .expect("the record registered its event");
        assert_eq!(held.class, WorkClass::Structural);
        assert_eq!(held.module, event("evolution"));
    });
}

#[test]
fn an_unknown_event_declines_the_emit() {
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
    });
}

#[test]
fn a_correction_appends_and_leaves_the_original_whole() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the area as 400 square metres."),
            payload(&[1])
        ));
        let original = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the log holds the entry");

        assert_ok!(Audit::correct(
            RuntimeOrigin::signed(REGISTRAR),
            DEPLOYMENT,
            0,
            reason("The survey gives the area as 420 square metres."),
            payload(&[2])
        ));

        assert_eq!(Pallet::<Test>::length(&DEPLOYMENT), 2);
        let kept = Pallet::<Test>::entry(&DEPLOYMENT, 0).expect("the original stays");
        assert_eq!(kept.body_bytes(), original.body_bytes());
        assert_eq!(kept.digest, original.digest);
        let correction = Pallet::<Test>::entry(&DEPLOYMENT, 1).expect("the correction lands");
        assert_eq!(correction.corrects, Some(0));
        assert_eq!(correction.event, original.event);
        assert_eq!(correction.actor, REGISTRAR);
    });
}

#[test]
fn a_correction_of_a_correction_declines() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[1])
        ));
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
    });
}

#[test]
fn a_correction_of_an_absent_entry_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Audit::correct(
                RuntimeOrigin::signed(REGISTRAR),
                DEPLOYMENT,
                4,
                reason("The survey gives another area."),
                payload(&[2])
            ),
            Error::<Test>::EntryAbsent
        );
    });
}

#[test]
fn the_digest_chain_verifies_over_the_whole_log() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for step in 0..5u8 {
            assert_ok!(Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[step])
            ));
        }
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));
        assert!(Pallet::<Test>::verify_chain(&DEPLOYMENT, 3, 16));
    });
}

#[test]
fn a_changed_entry_breaks_the_digest_chain() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for step in 0..3u8 {
            assert_ok!(Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[step])
            ));
        }
        let mut entry = Pallet::<Test>::entry(&DEPLOYMENT, 1).expect("the log holds the entry");
        entry.payload = payload(&[99]);
        Entries::<Test>::insert(DEPLOYMENT, 1, entry);
        assert!(!Pallet::<Test>::verify_chain(&DEPLOYMENT, 0, 16));
    });
}

#[test]
fn a_query_by_actor_returns_the_entries_of_that_actor() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for actor in [CLERK, REGISTRAR, CLERK] {
            assert_ok!(Audit::emit(
                RuntimeOrigin::signed(actor),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The actor recorded the parcel."),
                payload(&[])
            ));
        }
        let held = Pallet::<Test>::query_by_actor(&DEPLOYMENT, &CLERK, 0, 16);
        assert_eq!(held.len(), 2);
        assert!(held.iter().all(|record| record.actor == CLERK));
        assert_eq!(held[0].sequence, 0);
        assert_eq!(held[1].sequence, 2);
    });
}

#[test]
fn a_query_by_event_returns_the_entries_of_that_event() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        define("parcel_retired", WorkClass::Structural);
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[])
        ));
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_retired"),
            reason("The clerk retired the parcel."),
            payload(&[])
        ));
        let held = Pallet::<Test>::query_by_event(&DEPLOYMENT, "parcel_retired", 0, 16);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].event, "parcel_retired");
        assert_eq!(held[0].class, WorkClass::Structural);
    });
}

#[test]
fn a_subscription_follows_the_events_it_names() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        define("parcel_retired", WorkClass::Structural);
        let followed: BoundedVec<BoundedName, ConstU32<16>> =
            BoundedVec::try_from(alloc::vec![event("parcel_retired")]).expect("the list fits");
        assert_ok!(Audit::subscribe(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            followed,
            BoundedVec::default()
        ));

        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[])
        ));
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_retired"),
            reason("The clerk retired the parcel."),
            payload(&[])
        ));

        let held = Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &READER, 0, 16);
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].event, "parcel_retired");

        let followers = Pallet::<Test>::subscribers_for(
            &DEPLOYMENT,
            "parcel_retired",
            WorkClass::Structural,
            8,
        );
        assert_eq!(followers, alloc::vec![READER]);
    });
}

#[test]
fn a_closed_subscription_follows_nothing() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
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
        assert_noop!(
            Audit::unsubscribe(RuntimeOrigin::signed(READER), DEPLOYMENT),
            Error::<Test>::SubscriptionAbsent
        );
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[])
        ));
        assert!(Pallet::<Test>::query_for_subscriber(&DEPLOYMENT, &READER, 0, 16).is_empty());
    });
}

#[test]
fn an_export_carries_the_entries_in_sequence_order() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for step in 0..4u8 {
            assert_ok!(Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[step])
            ));
        }
        let bytes = Pallet::<Test>::export(&DEPLOYMENT, 0, 16);
        let held: alloc::vec::Vec<LogRecord<u64, u64>> =
            Decode::decode(&mut bytes.as_slice()).expect("the export decodes");
        assert_eq!(held.len(), 4);
        for (position, record) in held.iter().enumerate() {
            assert_eq!(record.sequence, position as u64);
            assert_eq!(record.event, "parcel_registered");
            assert_eq!(record.actor, CLERK);
        }
    });
}

#[test]
fn a_read_stops_at_the_batch_limit() {
    new_test_ext().execute_with(|| {
        define("parcel_registered", WorkClass::Routine);
        for step in 0..6u8 {
            assert_ok!(Audit::emit(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                event("parcel_registered"),
                reason("The clerk recorded the parcel."),
                payload(&[step])
            ));
        }
        assert_eq!(Pallet::<Test>::query(&DEPLOYMENT, 0, 2).len(), 2);
        assert_eq!(Pallet::<Test>::query(&DEPLOYMENT, 4, 10).len(), 2);
    });
}

#[test]
fn two_deployments_hold_separate_logs() {
    new_test_ext().execute_with(|| {
        let other: DeploymentId = [4u8; 32];
        define("parcel_registered", WorkClass::Routine);
        assert_ok!(Audit::emit(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            event("parcel_registered"),
            reason("The clerk recorded the parcel."),
            payload(&[])
        ));
        assert_eq!(NextSequence::<Test>::get(DEPLOYMENT), 1);
        assert_eq!(NextSequence::<Test>::get(other), 0);
        assert!(Pallet::<Test>::query(&other, 0, 16).is_empty());
    });
}

#[test]
fn a_work_class_carries_its_threshold() {
    assert_eq!(WorkClass::Routine.threshold_percent(), 51);
    assert_eq!(WorkClass::Financial.threshold_percent(), 67);
    assert_eq!(WorkClass::Structural.threshold_percent(), 80);
    assert_eq!(WorkClass::Constitutional.threshold_percent(), 90);
    assert!(WorkClass::Constitutional.holds_delay());
    assert!(!WorkClass::Structural.holds_delay());
}

extern crate alloc;
