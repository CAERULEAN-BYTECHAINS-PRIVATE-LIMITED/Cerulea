//! Further tests of the workflow module.
//!
//! These tests cover every refusal of the machine check, the refusal
//! branches of the calls, the plain statements a refused transition
//! carries, the scheduled steps that fail, the compensation limits and
//! the helpers that read and write the state field.

use crate::{
    mock::*, tests::*, ActionRequest, ActionRunner, CompensationQueue, DueActions, Error, Event,
    GuardEngine, InstanceStatus, Instances, Machines, MeteredGuards, NextDueAction, Pallet,
    ProcessView, QueuedActions,
};
use cerulea_blueprint::{
    expression::{BinaryOp, Expr},
    types::{
        ActionId, CollectionId, EventId, FieldPath, StateId, TimerId, TypeExpr, TypeId, Value,
        VariantName, WorkflowId,
    },
    workflows::{CyclePolicy, TimerAction, Transition, Workflow as WorkflowDef},
    Path,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchResult};
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName};

extern crate alloc;
use alloc::{string::ToString, vec, vec::Vec};

fn named(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn variant(id: &str) -> WorkflowDef {
    let mut held = machine();
    held.id = WorkflowId::new(id);
    held
}

fn install_variant(held: &WorkflowDef) -> DispatchResult {
    Workflow::install_machine(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(held.encode()),
    )
}

/// Place a machine straight into storage.
///
/// The machine check refuses the shape these tests need. The module still
/// guards against a stored machine of that shape, so the tests place it
/// directly and read the guard.
fn place_machine(held: &WorkflowDef) -> BoundedName {
    let name = named(held.id.as_str());
    Machines::<Test>::insert(DEPLOYMENT, &name, bounded(held.encode()));
    name
}

fn start_on(workflow: &BoundedName) {
    assert_ok!(Workflow::start_instance(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        workflow.clone(),
        record_key("12/3")
    ));
}

fn take(workflow: &BoundedName, id: u64, who: u64, transition: &str) -> DispatchResult {
    Workflow::attempt_transition(
        RuntimeOrigin::signed(who),
        DEPLOYMENT,
        workflow.clone(),
        id,
        named(transition),
    )
}

fn workflow_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Workflow(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

fn dividing_guard() -> Expr {
    Expr::binary(
        BinaryOp::Greater,
        Expr::binary(
            BinaryOp::Divide,
            Expr::field(Expr::bound("record"), "area"),
            Expr::integer(0),
        ),
        Expr::integer(0),
    )
}

#[test]
fn the_machine_check_refuses_every_malformed_shape() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut cases: Vec<(WorkflowDef, Error<Test>)> = Vec::new();

        let mut held = variant("empty");
        held.states.clear();
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("no_initial");
        held.initial = StateId::new("archived");
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("foreign_state");
        held.states
            .push(cerulea_blueprint::workflows::WorkflowState::passing(
                "archived",
                "Archived",
                "The register archived the parcel.",
            ));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("twin_state");
        let twin = held.states[0].clone();
        held.states.push(twin);
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("untitled");
        held.states[0].title = alloc::string::String::new();
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("twin_transition");
        let twin = held.transitions[0].clone();
        held.transitions.push(twin);
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("bad_from");
        held.transitions
            .push(Transition::new("leap", "archived", "registered"));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("bad_to");
        held.transitions
            .push(Transition::new("leap", "submitted", "archived"));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("number_guard");
        held.transitions[2] =
            Transition::new("register", "approved", "registered").guarded_by(Expr::integer(1));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("unknown_field_guard");
        held.transitions[2] = Transition::new("register", "approved", "registered")
            .guarded_by(Expr::field(Expr::bound("record"), "ward"));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("twin_timer");
        let twin = held.timers[0].clone();
        held.timers.push(twin);
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("timer_state");
        held.timers[0].state = StateId::new("archived");
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("instant_timer");
        held.timers[0].after_millis = 0;
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("timer_target");
        held.timers[0].on_expiry = TimerAction::MoveTo(StateId::new("archived"));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("deadline_twin");
        held.deadlines[0].id = TimerId::new("reminder");
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("deadline_state");
        held.deadlines[0].state = StateId::new("archived");
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("deadline_number");
        held.deadlines[0].due = Expr::field(Expr::bound("record"), "area");
        cases.push((held, Error::<Test>::DeadlineInvalid));

        let mut held = variant("deadline_unknown");
        held.deadlines[0].due = Expr::field(Expr::bound("record"), "ward");
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("deadline_target");
        held.deadlines[0].on_miss = TimerAction::MoveTo(StateId::new("archived"));
        cases.push((held, Error::<Test>::MachineInvalid));

        let mut held = variant("text_state_field");
        held.state_field = FieldPath::parse("holder");
        cases.push((held, Error::<Test>::StateFieldInvalid));

        for (held, error) in cases {
            assert_noop!(install_variant(&held), error);
        }
    });
}

#[test]
fn a_machine_allowing_loops_installs_with_a_loop() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("review_cycle");
        held.transitions
            .push(Transition::new("reopen", "surveyed", "submitted"));
        held.cycles = CyclePolicy::Forbid;
        assert_noop!(install_variant(&held), Error::<Test>::MachineInvalid);
        held.cycles = CyclePolicy::Allow;
        assert_ok!(install_variant(&held));
    });
}

#[test]
fn an_install_declines_an_unreadable_or_long_machine() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            Workflow::install_machine(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![0xff, 0x01])
            ),
            Error::<Test>::MachineUnreadable
        );
        assert_noop!(
            install_variant(&variant(&"w".repeat(200))),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn a_deployment_past_its_machine_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        for place in 0..31 {
            let mut id = alloc::string::String::from("machine_");
            id.push_str(&place.to_string());
            assert_ok!(install_variant(&variant(&id)));
        }
        assert_noop!(
            install_variant(&variant("machine_extra")),
            Error::<Test>::MachineLimitReached
        );
    });
}

#[test]
fn a_retirement_of_an_absent_machine_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            Workflow::retire_machine(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, named("absent")),
            Error::<Test>::MachineUnknown
        );
        assert_ok!(Workflow::retire_machine(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name()
        ));
        System::assert_last_event(RuntimeEvent::Workflow(Event::MachineRetired {
            deployment: DEPLOYMENT,
            workflow: workflow_name(),
        }));
    });
}

#[test]
fn a_start_the_gate_refuses_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        refuse("parcel");
        assert_noop!(
            Workflow::start_instance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                record_key("12/3")
            ),
            Error::<Test>::PermissionDenied
        );
        admit_every_collection();
        start();
    });
}

#[test]
fn a_process_starting_in_a_resting_state_settles_at_once() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("already_registered");
        held.initial = StateId::new("registered");
        assert_ok!(install_variant(&held));
        let name = named("already_registered");
        start_on(&name);
        let id = WorkflowId::new("already_registered");
        assert!(Pallet::<Test>::is_settled(&DEPLOYMENT, &id, 0));
        assert_eq!(
            Pallet::<Test>::instance_state(&DEPLOYMENT, &id, 0),
            Some(StateId::new("registered"))
        );
        assert_eq!(stored_state(), "registered");
    });
}

#[test]
fn a_transition_on_an_absent_process_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            take(&workflow_name(), 7, SURVEYOR, "survey"),
            Error::<Test>::InstanceUnknown
        );
        assert!(Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            7,
            &named("survey"),
            &SURVEYOR
        )
        .is_none());
        assert!(Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &named("absent"),
            0,
            &named("survey"),
            &SURVEYOR
        )
        .is_none());
    });
}

#[test]
fn a_guard_that_faults_stops_the_transition_with_its_diagnostic() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("dividing");
        held.transitions[0] =
            Transition::new("survey", "submitted", "surveyed").guarded_by(dividing_guard());
        assert_ok!(install_variant(&held));
        let name = named("dividing");
        start_on(&name);
        assert_noop!(
            take(&name, 0, STEWARD, "survey"),
            Error::<Test>::GuardFaulted
        );

        // The statement carries the diagnostic the evaluator raised.
        let record = parcel("12/3", 400, START + 50_000, "submitted");
        let env = Pallet::<Test>::environment(&STEWARD, START, &record);
        let path = Path::root().field("workflows").key("dividing");
        let diagnostic = MeteredGuards::evaluate(&dividing_guard(), &env, 10_000, &path)
            .expect_err("the division faults");
        let refusal =
            Pallet::<Test>::explain_refusal(&DEPLOYMENT, &name, 0, &named("survey"), &STEWARD)
                .expect("the module states the reason");
        assert_eq!(refusal.problem_text(), diagnostic.problem);
        assert_eq!(refusal.fix_text(), diagnostic.fix);
    });
}

#[test]
fn a_stored_guard_answering_a_number_stops_the_transition() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("numbered");
        held.transitions[0] =
            Transition::new("survey", "submitted", "surveyed").guarded_by(Expr::integer(1));
        let name = place_machine(&held);
        start_on(&name);
        assert_noop!(
            take(&name, 0, STEWARD, "survey"),
            Error::<Test>::GuardFaulted
        );
        let refusal =
            Pallet::<Test>::explain_refusal(&DEPLOYMENT, &name, 0, &named("survey"), &STEWARD)
                .expect("the module states the reason");
        assert_eq!(
            refusal.problem_text(),
            "The guard of the transition answered with a value other than a truth value."
        );
        assert_eq!(
            refusal.fix_text(),
            "Give the guard a truth value in the Studio."
        );
    });
}

#[test]
fn every_refusal_statement_names_the_problem_and_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();

        let missing_role = Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("survey"),
            &STEWARD,
        )
        .expect("the module states the reason");
        assert_eq!(
            missing_role.problem_text(),
            "The caller holds none of the roles the transition names."
        );
        assert_eq!(
            missing_role.fix_text(),
            "Ask a holder of one of those roles to take the transition."
        );
        assert_eq!(missing_role.transition, named("survey"));
        assert_eq!(missing_role.instance, 0);

        // A transition the caller may take carries no refusal.
        assert!(Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("survey"),
            &SURVEYOR
        )
        .is_none());
        assert!(Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("withdraw"),
            &STEWARD
        )
        .is_none());

        assert_ok!(take(&workflow_name(), 0, SURVEYOR, "survey"));
        // A guard answering true carries no refusal.
        assert!(Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("approve"),
            &STEWARD
        )
        .is_none());
        let absent = Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("register"),
            &STEWARD,
        )
        .expect("the module states the reason");
        assert_eq!(
            absent.fix_text(),
            "Name a transition that leaves the present state."
        );

        assert_ok!(take(&workflow_name(), 0, STEWARD, "approve"));
        assert_ok!(take(&workflow_name(), 0, STEWARD, "register"));
        let settled = Pallet::<Test>::explain_refusal(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("register"),
            &STEWARD,
        )
        .expect("the module states the reason");
        assert_eq!(
            settled.problem_text(),
            "The process left its running state."
        );
        assert_eq!(settled.fix_text(), "Start a new process over the record.");
    });
}

#[test]
fn a_transition_carried_by_an_action_queues_the_action() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("carried");
        held.transitions[3] =
            Transition::new("withdraw", "submitted", "withdrawn").carried_by("notify_holder");
        assert_ok!(install_variant(&held));
        let name = named("carried");
        start_on(&name);
        assert_ok!(take(&name, 0, STEWARD, "withdraw"));
        System::assert_has_event(RuntimeEvent::Workflow(Event::InstanceSettled {
            deployment: DEPLOYMENT,
            workflow: name.clone(),
            instance: 0,
            state: named("withdrawn"),
        }));
        let queued = DueActions::<Test>::get(0).expect("the action joined the queue");
        assert_eq!(queued.action, named("notify_holder"));
        assert_eq!(queued.collection, named("parcel"));
        assert_eq!(queued.record, record_key("12/3"));
        assert_eq!(queued.raised_at, 1);
    });
}

#[test]
fn the_process_view_reads_state_settlement_and_steps() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        let id = WorkflowId::new("parcel_registration");
        assert_eq!(
            Pallet::<Test>::instance_state(&DEPLOYMENT, &id, 0),
            Some(StateId::new("submitted"))
        );
        assert!(!Pallet::<Test>::is_settled(&DEPLOYMENT, &id, 0));
        assert!(Pallet::<Test>::instance_state(&DEPLOYMENT, &id, 4).is_none());
        let long = WorkflowId::new(&"w".repeat(200));
        assert!(Pallet::<Test>::instance_state(&DEPLOYMENT, &long, 0).is_none());
        assert!(!Pallet::<Test>::is_settled(&DEPLOYMENT, &long, 0));
        assert_eq!(Pallet::<Test>::step_count(&DEPLOYMENT, &long, 0), 0);
    });
}

#[test]
fn a_timer_fired_by_hand_records_its_event() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_noop!(
            Workflow::fire_timer(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                3,
                named("reminder")
            ),
            Error::<Test>::InstanceUnknown
        );
        assert_noop!(
            Workflow::fire_timer(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                named("absent")
            ),
            Error::<Test>::TimerUnknown
        );
        assert_eq!(
            Pallet::<Test>::scheduled_due(&DEPLOYMENT, &workflow_name(), 0, &named("reminder")),
            Some(START + 10_000)
        );
        advance_clock(10_000);
        // The reminder is a timer, so an escalation by name declines.
        assert_noop!(
            Workflow::escalate_deadline(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                named("reminder")
            ),
            Error::<Test>::TimerUnknown
        );
        assert_ok!(Workflow::fire_timer(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            named("reminder")
        ));
        System::assert_last_event(RuntimeEvent::Workflow(Event::TimerFired {
            deployment: DEPLOYMENT,
            workflow: workflow_name(),
            instance: 0,
            timer: named("reminder"),
        }));
        assert_eq!(
            Audit::query_by_event(&DEPLOYMENT, "reminder_sent", 0, 64).len(),
            1
        );
        assert!(Pallet::<Test>::scheduled_due(
            &DEPLOYMENT,
            &workflow_name(),
            0,
            &named("reminder")
        )
        .is_none());
        assert!(Pallet::<Test>::scheduled_due(
            &DEPLOYMENT,
            &workflow_name(),
            5,
            &named("reminder")
        )
        .is_none());
    });
}

#[test]
fn a_scheduled_step_over_a_retired_machine_reports_the_failure() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(Workflow::retire_machine(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name()
        ));
        advance_clock(11_000);
        roll_one_block();
        System::assert_has_event(RuntimeEvent::Workflow(Event::ScheduledStepFailed {
            deployment: DEPLOYMENT,
            workflow: workflow_name(),
            instance: 0,
            timer: named("reminder"),
        }));
        assert_eq!(Pallet::<Test>::scheduled_count(), 0);
    });
}

#[test]
fn a_scheduled_step_over_a_closed_process_reports_the_failure() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        Instances::<Test>::mutate((DEPLOYMENT, workflow_name(), 0), |held| {
            if let Some(instance) = held.as_mut() {
                instance.status = InstanceStatus::Settled;
            }
        });
        advance_clock(11_000);
        roll_one_block();
        assert!(workflow_events()
            .iter()
            .any(|held| matches!(held, Event::ScheduledStepFailed { .. })));
    });
}

#[test]
fn a_timer_moving_the_record_schedules_the_next_state() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("self_surveying");
        held.timers[0].on_expiry = TimerAction::MoveTo(StateId::new("surveyed"));
        assert_ok!(install_variant(&held));
        let name = named("self_surveying");
        start_on(&name);
        advance_clock(10_000);
        roll_one_block();
        assert_eq!(stored_state(), "surveyed");
        let id = WorkflowId::new("self_surveying");
        assert_eq!(
            Pallet::<Test>::instance_state(&DEPLOYMENT, &id, 0),
            Some(StateId::new("surveyed"))
        );
        assert_eq!(Pallet::<Test>::step_count(&DEPLOYMENT, &id, 0), 1);
        // The surveyed state carries the deadline the record names.
        assert_eq!(
            Pallet::<Test>::scheduled_due(&DEPLOYMENT, &name, 0, &named("survey_due")),
            Some(START + 50_000)
        );
    });
}

#[test]
fn a_compensation_the_gate_refuses_or_the_registry_lacks_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        assert_noop!(
            Workflow::compensate(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                reason("The survey failed.")
            ),
            Error::<Test>::InstanceUnknown
        );
        start();
        refuse("parcel");
        assert_noop!(
            Workflow::compensate(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                reason("The survey failed.")
            ),
            Error::<Test>::PermissionDenied
        );
        admit_every_collection();
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The survey failed.")
        ));
        assert_eq!(held().status, InstanceStatus::Compensated);
        assert_noop!(
            Workflow::compensate(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                0,
                reason("The survey failed again.")
            ),
            Error::<Test>::InstanceClosed
        );
    });
}

#[test]
fn a_full_compensation_queue_names_the_fix() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        for id in 0..17u64 {
            start();
            assert_ok!(take(&workflow_name(), id, STEWARD, "withdraw"));
        }
        for id in 0..16u64 {
            assert_ok!(Workflow::compensate(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                id,
                reason("The register withdrew the batch.")
            ));
        }
        assert_eq!(Pallet::<Test>::compensating_count(), 16);
        assert_noop!(
            Workflow::compensate(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                workflow_name(),
                16,
                reason("The register withdrew the batch.")
            ),
            Error::<Test>::CompensationQueueFull
        );
    });
}

#[test]
fn a_compensation_runs_the_action_of_the_way_back() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        let mut held = variant("reviewed");
        held.cycles = CyclePolicy::Allow;
        held.transitions
            .push(Transition::new("unsurvey", "surveyed", "submitted").carried_by("void_survey"));
        assert_ok!(install_variant(&held));
        let name = named("reviewed");
        start_on(&name);
        assert_ok!(take(&name, 0, SURVEYOR, "survey"));
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name.clone(),
            0,
            reason("The survey failed verification.")
        ));
        roll_one_block();
        assert_eq!(stored_state(), "submitted");
        let taken = Pallet::<Test>::take_due_actions(8);
        assert_eq!(taken.len(), 1);
        assert_eq!(name_text(&taken[0].1.action), "void_survey");
        System::assert_has_event(RuntimeEvent::Workflow(Event::CompensationCompleted {
            deployment: DEPLOYMENT,
            workflow: name,
            instance: 0,
        }));
    });
}

#[test]
fn a_compensation_over_a_retired_machine_leaves_the_queue() {
    new_test_ext().execute_with(|| {
        install(400, START + 50_000);
        start();
        assert_ok!(take(&workflow_name(), 0, SURVEYOR, "survey"));
        assert_ok!(Workflow::compensate(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name(),
            0,
            reason("The survey failed verification.")
        ));
        assert_eq!(Pallet::<Test>::compensating_count(), 1);
        assert_ok!(Workflow::retire_machine(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            workflow_name()
        ));
        roll_one_block();
        assert_eq!(Pallet::<Test>::compensating_count(), 0);
        assert!(CompensationQueue::<Test>::get().is_empty());
        assert_eq!(held().status, InstanceStatus::Compensating);
        assert!(Pallet::<Test>::undone_steps(&DEPLOYMENT, &workflow_name(), 0).is_empty());
        assert!(Pallet::<Test>::undone_steps(&DEPLOYMENT, &workflow_name(), 9).is_empty());
    });
}

#[test]
fn the_action_queue_refuses_long_names_and_a_spent_counter() {
    new_test_ext().execute_with(|| {
        let long = CollectionId::new(&"c".repeat(200));
        let parcel_id = CollectionId::new("parcel");
        let key = record_key("12/3");
        let action = ActionId::new("notify_holder");
        let long_action = ActionId::new(&"a".repeat(200));
        assert_noop!(
            QueuedActions::<Test>::run(ActionRequest {
                deployment: DEPLOYMENT,
                collection: &long,
                record: &key,
                action: &action,
                actor: &STEWARD,
            }),
            Error::<Test>::NameTooLong
        );
        assert_noop!(
            QueuedActions::<Test>::run(ActionRequest {
                deployment: DEPLOYMENT,
                collection: &parcel_id,
                record: &key,
                action: &long_action,
                actor: &STEWARD,
            }),
            Error::<Test>::NameTooLong
        );
        NextDueAction::<Test>::put(u64::MAX);
        assert_noop!(
            QueuedActions::<Test>::run(ActionRequest {
                deployment: DEPLOYMENT,
                collection: &parcel_id,
                record: &key,
                action: &action,
                actor: &STEWARD,
            }),
            Error::<Test>::QueueFull
        );
        NextDueAction::<Test>::put(7);
        assert_eq!(
            QueuedActions::<Test>::run(ActionRequest {
                deployment: DEPLOYMENT,
                collection: &parcel_id,
                record: &key,
                action: &action,
                actor: &STEWARD,
            }),
            Ok(7)
        );
        assert_eq!(Pallet::<Test>::due_action_count(), 1);
    });
}

#[test]
fn the_state_field_helpers_reach_through_an_optional_record() {
    let inner = parcel("12/3", 400, START, "submitted");
    let wrapped = Value::some(TypeExpr::Record(TypeId::new("parcel")), inner.clone());
    let empty = Value::none(TypeExpr::Record(TypeId::new("parcel")));
    let holder = FieldPath::parse("holder");

    assert_eq!(
        crate::value_at(&wrapped, &holder),
        Some(Value::Text("Anitha".to_string()))
    );
    assert_eq!(crate::value_at(&empty, &holder), None);
    assert_eq!(crate::value_at(&Value::Integer(3), &holder), None);
    let wrapped_number = Value::some(TypeExpr::Integer, Value::Integer(3));
    assert_eq!(crate::value_at(&wrapped_number, &holder), None);

    let mut changed = wrapped.clone();
    assert!(crate::set_value_at(
        &mut changed,
        &holder,
        Value::Text("Benny".to_string())
    ));
    assert_eq!(
        crate::value_at(&changed, &holder),
        Some(Value::Text("Benny".to_string()))
    );
    let mut nothing = empty.clone();
    assert!(!crate::set_value_at(
        &mut nothing,
        &holder,
        Value::Integer(1)
    ));
    let mut number = Value::Integer(3);
    assert!(!crate::set_value_at(
        &mut number,
        &holder,
        Value::Integer(1)
    ));
    let mut wrapped_number = wrapped_number;
    assert!(!crate::set_value_at(
        &mut wrapped_number,
        &holder,
        Value::Integer(1)
    ));
    let mut record = inner.clone();
    assert!(!crate::set_value_at(
        &mut record,
        &FieldPath(Vec::new()),
        Value::Integer(1)
    ));
    assert!(!crate::set_value_at(
        &mut record,
        &FieldPath::parse("ward"),
        Value::Integer(1)
    ));

    assert_eq!(
        crate::state_of(&inner, &FieldPath::parse("state")),
        Some((TypeId::new("parcel_state"), VariantName::new("submitted")))
    );
    assert_eq!(crate::state_of(&inner, &holder), None);
}

#[test]
fn every_timer_action_carries_a_label_and_a_state_lists_its_timers() {
    assert_eq!(
        crate::timer_action_label(&TimerAction::MoveTo(StateId::new("withdrawn"))),
        "move the record"
    );
    assert_eq!(
        crate::timer_action_label(&TimerAction::Run(ActionId::new("notify_holder"))),
        "run the action"
    );
    assert_eq!(
        crate::timer_action_label(&TimerAction::Emit(EventId::new("reminder_sent"))),
        "record the event"
    );
    let held = machine();
    let found = crate::timers_of(&held, "submitted");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].id, TimerId::new("reminder"));
    assert!(crate::timers_of(&held, "registered").is_empty());
    assert_eq!(
        crate::timer::missing_machine::<Test>(),
        Error::<Test>::MachineUnknown.into()
    );
    assert_eq!(crate::timer::bucket_of(258), [0, 0, 0, 0, 0, 0, 1, 2]);
}
