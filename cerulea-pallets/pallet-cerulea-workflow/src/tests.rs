//! The tests of the workflow module.
//!
//! Every test drives a real record store, a real registry and a real audit
//! log. The land parcel register carries the process. A parcel passes from
//! submitted to surveyed, from surveyed to approved and from approved to
//! registered.

use crate::{
    mock::*, CompensationQueue, Error, Instance, InstanceStatus, Instances, Pallet, ProcessView,
    Schedule,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    types::{
        EnumerationDef, FieldDef, FieldName, FieldPath, RecordDef, StateId, Timestamp, TypeDef,
        TypeExpr, TypeId, Value, VariantName, WorkflowId,
    },
    workflows::{
        CyclePolicy, Deadline, Timer, TimerAction, Transition, Workflow as WorkflowDef,
        WorkflowState,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName, DeploymentId, RecordKey};

extern crate alloc;
use alloc::{string::ToString, vec, vec::Vec};

pub(crate) const DEPLOYMENT: DeploymentId = [9u8; 32];
pub(crate) const STEWARD: u64 = 1;
pub(crate) const SURVEYOR: u64 = 2;

pub(crate) fn parcel_state_type() -> TypeDef {
    TypeDef::Enumeration(EnumerationDef {
        id: TypeId::new("parcel_state"),
        variants: vec![
            VariantName::new("submitted"),
            VariantName::new("surveyed"),
            VariantName::new("approved"),
            VariantName::new("registered"),
            VariantName::new("withdrawn"),
        ],
        summary: "The stages a parcel passes through in the register.".to_string(),
    })
}

pub(crate) fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
            FieldDef::new(
                "due_at",
                TypeExpr::Timestamp,
                "The moment the survey falls due.",
            ),
            FieldDef::new(
                "state",
                TypeExpr::Enumeration(TypeId::new("parcel_state")),
                "The stage the parcel stands at.",
            ),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

pub(crate) fn parcel_collection() -> Collection {
    let mut collection = Collection::new(
        "parcel",
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records who holds each land parcel.",
        &["registration"],
    );
    collection
}

pub(crate) fn parcel(survey: &str, area: i128, due_at: u64, state: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text("Anitha".to_string())),
            (FieldName::new("area"), Value::Integer(area)),
            (
                FieldName::new("due_at"),
                Value::Timestamp(Timestamp::from_millis(due_at)),
            ),
            (
                FieldName::new("state"),
                Value::enumeration(TypeId::new("parcel_state"), VariantName::new(state)),
            ),
        ],
    );
    value.canonicalise();
    value
}

pub(crate) fn machine() -> WorkflowDef {
    WorkflowDef {
        id: WorkflowId::new("parcel_registration"),
        collection: cerulea_blueprint::types::CollectionId::new("parcel"),
        state_field: FieldPath::parse("state"),
        states: vec![
            WorkflowState::passing(
                "submitted",
                "Submitted",
                "The holder submitted the parcel for registration.",
            ),
            WorkflowState::passing("surveyed", "Surveyed", "The surveyor measured the parcel."),
            WorkflowState::passing("approved", "Approved", "The registrar approved the parcel."),
            WorkflowState::resting("registered", "Registered", "The register holds the parcel."),
            WorkflowState::resting("withdrawn", "Withdrawn", "The parcel left the register."),
        ],
        initial: StateId::new("submitted"),
        transitions: vec![
            Transition::new("survey", "submitted", "surveyed").run_by(&["surveyor"]),
            Transition::new("approve", "surveyed", "approved").guarded_by(Expr::binary(
                BinaryOp::Greater,
                Expr::field(Expr::bound("record"), "area"),
                Expr::integer(0),
            )),
            Transition::new("register", "approved", "registered"),
            Transition::new("withdraw", "submitted", "withdrawn"),
        ],
        timers: vec![Timer {
            id: cerulea_blueprint::types::TimerId::new("reminder"),
            state: StateId::new("submitted"),
            after_millis: 10_000,
            on_expiry: TimerAction::Emit(cerulea_blueprint::types::EventId::new("reminder_sent")),
        }],
        deadlines: vec![Deadline {
            id: cerulea_blueprint::types::TimerId::new("survey_due"),
            state: StateId::new("surveyed"),
            due: Expr::field(Expr::bound("record"), "due_at"),
            on_miss: TimerAction::MoveTo(StateId::new("withdrawn")),
        }],
        cycles: CyclePolicy::Forbid,
        summary: "The registration of one land parcel.".to_string(),
    }
}

pub(crate) fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

pub(crate) fn workflow_name() -> BoundedName {
    name_of("parcel_registration").expect("the name fits")
}

pub(crate) fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.to_string());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

pub(crate) fn reason(text: &str) -> BoundedVec<u8, ConstU32<256>> {
    BoundedVec::try_from(text.as_bytes().to_vec()).expect("the reason fits")
}

/// Register the types, the collection, the roles and one parcel record.
pub(crate) fn install(area: i128, due_at: u64) {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_state_type().encode())
    ));
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_type().encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_collection().encode())
    ));
    assert_ok!(State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name_of("parcel").expect("the name fits"),
        bounded(parcel("12/3", area, due_at, "submitted").canonical_bytes())
    ));
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(STEWARD),
        SURVEYOR,
        [2u8; 32]
    ));
    assert_ok!(Identity::create_unit(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name_of("register").expect("the name fits"),
        None,
        bounded(b"Land register".to_vec()),
        bounded(b"The land register office.".to_vec())
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name_of("surveyor").expect("the name fits"),
        Some(name_of("register").expect("the name fits")),
        BoundedVec::default(),
        bounded(b"Surveyor".to_vec())
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        SURVEYOR,
        name_of("surveyor").expect("the name fits"),
        None
    ));
    assert_ok!(Workflow::install_machine(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(machine().encode())
    ));
}

pub(crate) fn start() {
    assert_ok!(Workflow::start_instance(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        workflow_name(),
        record_key("12/3")
    ));
}

pub(crate) fn held() -> Instance<u64, u64, ConstU32<64>, ConstU32<8>> {
    Pallet::<Test>::instance(&DEPLOYMENT, &workflow_name(), 0).expect("the process is running")
}

pub(crate) fn stored_state() -> alloc::string::String {
    let record = <Test as crate::Config>::Records::read(
        &DEPLOYMENT,
        &cerulea_blueprint::types::CollectionId::new("parcel"),
        &record_key("12/3"),
    )
    .expect("the store holds the record");
    match crate::value_at(&record, &FieldPath::parse("state")) {
        Some(Value::Enumeration(found)) => found.variant.as_str().to_string(),
        _ => panic!("the record carries an enumeration at the state field"),
    }
}

use pallet_cerulea_state::RecordStore;

#[test]
fn a_machine_installs_and_reads_back() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let stored = Pallet::<Test>::machine(&DEPLOYMENT, &workflow_name())
            .expect("the registry holds the machine");
        assert_eq!(stored.states.len(), 5);
        assert_eq!(stored.initial.as_str(), "submitted");
    });
}

#[test]
fn a_machine_naming_an_unknown_collection_declines() {
    new_test_ext().execute_with(|| {
        let mut broken = machine();
        broken.collection = cerulea_blueprint::types::CollectionId::new("absent");
        assert_noop!(
            Workflow::install_machine(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(broken.encode())
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_machine_closing_a_forbidden_loop_declines() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut looping = machine();
        looping.id = WorkflowId::new("looping_registration");
        looping
            .transitions
            .push(Transition::new("reopen", "approved", "submitted"));
        assert_noop!(
            Workflow::install_machine(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(looping.encode())
            ),
            Error::<Test>::MachineInvalid
        );
    });
}

#[test]
fn a_started_process_takes_the_initial_state() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        let instance = held();
        assert_eq!(name_text(&instance.state), "submitted");
        assert_eq!(instance.status, InstanceStatus::Running);
        assert_eq!(stored_state(), "submitted");
        assert_eq!(
            Pallet::<Test>::instance_state(&DEPLOYMENT, &WorkflowId::new("parcel_registration"), 0),
            Some(StateId::new("submitted"))
        );
    });
}

#[test]
fn a_transition_moves_the_record_and_the_process() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        assert_eq!(name_text(&held().state), "surveyed");
        assert_eq!(stored_state(), "surveyed");
        assert_eq!(held().history.len(), 1);
    });
}

#[test]
fn a_transition_the_present_state_lacks_declines() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("register").expect("the name fits")
            ),
            Error::<Test>::TransitionUnavailable
        );
    });
}

#[test]
fn a_caller_lacking_the_role_declines() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("survey").expect("the name fits")
            ),
            Error::<Test>::RoleMissing
        );
    });
}

#[test]
fn a_guard_answering_false_leaves_the_record_where_it_was() {
    new_test_ext().execute_with(|| {
        install(0, START + 50_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("approve").expect("the name fits")
            ),
            Error::<Test>::GuardRefused
        );
        assert_eq!(stored_state(), "surveyed");
    });
}

#[test]
fn a_refused_transition_keeps_a_plain_statement() {
    new_test_ext().execute_with(|| {
        install(0, START + 50_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        let refusal = Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &name_of("approve").expect("the name fits"),
            &STEWARD,
        )
        .expect("the module states the reason");
        assert_eq!(
            refusal.problem_text(),
            "The guard of the transition answered false."
        );
        assert!(refusal.fix_text().starts_with("Meet the condition"));

        // A transition the present state lacks carries its own statement.
        let absent = Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &name_of("withdraw").expect("the name fits"),
            &STEWARD,
        )
        .expect("the module states the reason");
        assert_eq!(
            absent.problem_text(),
            "The present state offers no transition of that name."
        );
    });
}

#[test]
fn a_permission_refusal_stops_the_transition() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        refuse("parcel");
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(SURVEYOR),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("survey").expect("the name fits")
            ),
            Error::<Test>::PermissionDenied
        );
        admit_every_collection();
    });
}

#[test]
fn a_final_state_settles_the_process() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        run_to_registered();
        let instance = held();
        assert_eq!(instance.status, InstanceStatus::Settled);
        assert!(Pallet::<Test>::is_settled(
            &DEPLOYMENT,
            &WorkflowId::new("parcel_registration"),
            0
        ));
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("register").expect("the name fits")
            ),
            Error::<Test>::InstanceClosed
        );
    });
}

pub(crate) fn run_to_registered() {
    assert_ok!(Workflow::attempt_transition(
        RuntimeOrigin::signed(SURVEYOR),
        DEPLOYMENT,
        workflow_name(),
        0,
        name_of("survey").expect("the name fits")
    ));
    assert_ok!(Workflow::attempt_transition(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        workflow_name(),
        0,
        name_of("approve").expect("the name fits")
    ));
    assert_ok!(Workflow::attempt_transition(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        workflow_name(),
        0,
        name_of("register").expect("the name fits")
    ));
}

#[test]
fn a_timer_of_the_initial_state_enters_the_schedule() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_eq!(
            Pallet::<Test>::scheduled_due(
                &DEPLOYMENT,
                &workflow_name(),
                0,
                &name_of("reminder").expect("the name fits")
            ),
            Some(START + 10_000)
        );
        assert_eq!(Pallet::<Test>::scheduled_count(), 1);
    });
}

#[test]
fn a_timer_fires_once_its_span_passes() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        roll_one_block();
        assert_eq!(Pallet::<Test>::scheduled_count(), 1);

        advance_clock(11_000);
        roll_one_block();
        assert_eq!(Pallet::<Test>::scheduled_count(), 0);

        let fired = Audit::query_by_event(&DEPLOYMENT, "timer_fired", 0, 64);
        assert_eq!(fired.len(), 1);
        let emitted = Audit::query_by_event(&DEPLOYMENT, "reminder_sent", 0, 64);
        assert_eq!(emitted.len(), 1);
        assert_eq!(emitted[0].actor, STEWARD);
    });
}

#[test]
fn a_transition_drops_the_timers_of_the_state_it_leaves() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_eq!(Pallet::<Test>::scheduled_count(), 1);
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        // The reminder of `submitted` leaves. The deadline of `surveyed`
        // enters.
        assert_eq!(Pallet::<Test>::scheduled_count(), 1);
        assert_eq!(
            Pallet::<Test>::scheduled_due(
                &DEPLOYMENT,
                &workflow_name(),
                0,
                &name_of("survey_due").expect("the name fits")
            ),
            Some(START + 50_000)
        );
    });
}

#[test]
fn a_deadline_escalates_once_its_moment_passes() {
    new_test_ext().execute_with(|| {
        install(400, START + 20_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        advance_clock(25_000);
        roll_one_block();

        assert_eq!(stored_state(), "withdrawn");
        assert_eq!(held().status, InstanceStatus::Settled);
        let escalations = Audit::query_by_event(&DEPLOYMENT, "deadline_escalated", 0, 64);
        assert_eq!(escalations.len(), 1);
    });
}

#[test]
fn a_deadline_escalated_by_hand_needs_its_moment_to_pass() {
    new_test_ext().execute_with(|| {
        install(400, START + 20_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        assert_noop!(
            Workflow::escalate_deadline(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("survey_due").expect("the name fits")
            ),
            Error::<Test>::TimerPending
        );
        advance_clock(25_000);
        assert_ok!(Workflow::escalate_deadline(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey_due").expect("the name fits")
        ));
        assert_eq!(stored_state(), "withdrawn");
    });
}

#[test]
fn one_block_runs_a_bounded_count_of_scheduled_steps() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        // Six processes over the same record, each with one reminder.
        for _ in 0..6 {
            start();
        }
        assert_eq!(Pallet::<Test>::scheduled_count(), 6);
        advance_clock(11_000);
        roll_one_block();
        // The block limit is four, so two steps wait.
        assert_eq!(Pallet::<Test>::scheduled_count(), 2);
        roll_one_block();
        assert_eq!(Pallet::<Test>::scheduled_count(), 0);
    });
}

#[test]
fn a_failed_process_runs_its_compensating_steps_in_reverse() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        run_to_registered();
        assert_eq!(stored_state(), "registered");

        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The survey of the parcel failed verification.")
        ));
        assert_eq!(held().status, InstanceStatus::Compensating);
        assert_eq!(held().cursor, 3);

        // The block limit is two compensating steps, so the work spans two
        // blocks and nothing stays locked across them.
        roll_one_block();
        assert_eq!(held().cursor, 1);
        assert_eq!(stored_state(), "surveyed");
        assert_eq!(Pallet::<Test>::compensating_count(), 1);

        roll_one_block();
        let instance = held();
        assert_eq!(instance.status, InstanceStatus::Compensated);
        assert_eq!(instance.cursor, 0);
        assert_eq!(stored_state(), "submitted");
        assert_eq!(Pallet::<Test>::compensating_count(), 0);

        let undone: Vec<alloc::string::String> =
            Pallet::<Test>::undone_steps(&DEPLOYMENT, &workflow_name(), 0)
                .iter()
                .map(name_text)
                .collect();
        assert_eq!(undone, vec!["register", "approve", "survey"]);
    });
}

#[test]
fn every_compensating_step_lands_in_the_audit_log() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        run_to_registered();
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The survey of the parcel failed verification.")
        ));
        roll_blocks(2);

        let steps = Audit::query_by_event(&DEPLOYMENT, "step_compensated", 0, 64);
        assert_eq!(steps.len(), 3);
        assert!(steps.iter().all(|entry| entry.actor == STEWARD));
        let completed = Audit::query_by_event(&DEPLOYMENT, "compensation_completed", 0, 64);
        assert_eq!(completed.len(), 1);
    });
}

#[test]
fn a_compensating_process_takes_no_further_transition() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The survey of the parcel failed verification.")
        ));
        assert_noop!(
            Workflow::attempt_transition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                name_of("approve").expect("the name fits")
            ),
            Error::<Test>::InstanceClosed
        );
        // The schedule holds nothing for a compensating process.
        assert_eq!(Pallet::<Test>::scheduled_count(), 0);
    });
}

#[test]
fn a_process_holding_no_step_compensates_at_once() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The holder withdrew the submission.")
        ));
        assert_eq!(held().status, InstanceStatus::Compensated);
        assert!(CompensationQueue::<Test>::get().is_empty());
    });
}

#[test]
fn every_workflow_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            0,
            name_of("survey").expect("the name fits")
        ));

        let installed = Audit::query_by_event(&DEPLOYMENT, "machine_installed", 0, 64);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].actor, STEWARD);
        assert_eq!(installed[0].module, "workflow");

        let started = Audit::query_by_event(&DEPLOYMENT, "instance_started", 0, 64);
        assert_eq!(started.len(), 1);
        assert_eq!(started[0].actor, STEWARD);

        let taken = Audit::query_by_event(&DEPLOYMENT, "transition_taken", 0, 64);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].actor, SURVEYOR);
    });
}

#[test]
fn a_retired_machine_takes_no_further_process() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_ok!(Workflow::retire_machine(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name()
        ));
        assert_noop!(
            Workflow::start_instance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                record_key("12/3")
            ),
            Error::<Test>::MachineUnknown
        );
    });
}

#[test]
fn a_machine_installed_twice_declines() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            Workflow::install_machine(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(machine().encode())
            ),
            Error::<Test>::MachinePresent
        );
    });
}

#[test]
fn a_process_over_an_absent_record_declines() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            Workflow::start_instance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                record_key("99/9")
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

#[test]
fn a_timer_action_naming_an_action_joins_the_queue() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut running = machine();
        running.id = WorkflowId::new("parcel_survey_run");
        running.timers[0].on_expiry =
            TimerAction::Run(cerulea_blueprint::types::ActionId::new("send_reminder"));
        assert_ok!(Workflow::install_machine(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(running.encode())
        ));
        assert_ok!(Workflow::start_instance(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel_survey_run").expect("the name fits"),
            record_key("12/3")
        ));
        advance_clock(11_000);
        roll_one_block();

        assert_eq!(Pallet::<Test>::due_action_count(), 1);
        let taken = Pallet::<Test>::take_due_actions(8);
        assert_eq!(taken.len(), 1);
        assert_eq!(name_text(&taken[0].1.action), "send_reminder");
        assert_eq!(taken[0].1.actor, STEWARD);
        assert_eq!(Pallet::<Test>::due_action_count(), 0);
    });
}

#[test]
fn the_schedule_holds_nothing_once_a_process_settles() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        run_to_registered();
        assert_eq!(Pallet::<Test>::scheduled_count(), 0);
        assert!(Schedule::<Test>::iter().next().is_none());
    });
}

#[test]
fn two_processes_over_one_machine_stay_apart() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        start();
        assert!(Instances::<Test>::get((DEPLOYMENT, workflow_name(), 0)).is_some());
        assert!(Instances::<Test>::get((DEPLOYMENT, workflow_name(), 1)).is_some());
        assert_ok!(Workflow::attempt_transition(
            RuntimeOrigin::signed(SURVEYOR),
            DEPLOYMENT,
            workflow_name(),
            1,
            name_of("survey").expect("the name fits")
        ));
        assert_eq!(
            Pallet::<Test>::step_count(&DEPLOYMENT, &WorkflowId::new("parcel_registration"), 0),
            0
        );
        assert_eq!(
            Pallet::<Test>::step_count(&DEPLOYMENT, &WorkflowId::new("parcel_registration"), 1),
            1
        );
    });
}
