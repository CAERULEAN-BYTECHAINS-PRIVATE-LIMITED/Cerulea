//! Behavioural tests for workflows.
//!
//! Each test starts from a clean workflow, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    actions::{Action, ActionRegistry},
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    expression::{BinaryOp, Builtin, Expr},
    identities::{Identities, RoleDef},
    permissions::ROW_BINDING,
    types::{
        ActionId, CollectionId, EnumerationDef, EventId, FieldDef, FieldPath, RecordDef, RoleId,
        StateId, TimerId, TransitionId, TypeDef, TypeExpr, TypeId, TypeRegistry, VariantName,
        WorkflowId,
    },
    workflows::{
        validate_workflows, CyclePolicy, Deadline, Timer, TimerAction, Transition, Workflow,
        WorkflowRegistry, WorkflowState,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("case"),
            fields: vec![
                FieldDef::new("number", TypeExpr::Text, "The case number."),
                FieldDef::new(
                    "stage",
                    TypeExpr::Enumeration(TypeId::new("stage")),
                    "The stage.",
                ),
                FieldDef::new("filed_at", TypeExpr::Timestamp, "The filing time."),
                FieldDef::new("score", TypeExpr::Integer, "The score."),
            ],
            summary: "One case.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("stage"),
            variants: vec![
                VariantName::new("draft"),
                VariantName::new("review"),
                VariantName::new("closed"),
            ],
            summary: "The stage of a case.".to_string(),
        }),
    ]
}

fn collections() -> Vec<Collection> {
    let mut case = Collection::new("case", "case", "number", "The cases.");
    case.purpose = PurposeDeclaration::new("The register records cases.", &["casework"]);
    vec![case]
}

fn identities() -> Identities {
    Identities {
        roles: vec![RoleDef::new(
            "clerk",
            None,
            "Clerk",
            "The clerk files cases.",
        )],
        ..Identities::default()
    }
}

fn actions() -> Vec<Action> {
    vec![Action::new("approve", "The action approves a case.")]
}

fn row(field: &str) -> Expr {
    Expr::field(Expr::bound(ROW_BINDING), field)
}

fn clean() -> Workflow {
    Workflow {
        id: WorkflowId::new("case_flow"),
        collection: CollectionId::new("case"),
        state_field: FieldPath::parse("stage"),
        states: vec![
            WorkflowState::passing("draft", "Draft", "The case waits."),
            WorkflowState::passing("review", "Review", "The case sits in review."),
            WorkflowState::resting("closed", "Closed", "The case rests."),
        ],
        initial: StateId::new("draft"),
        transitions: vec![
            Transition::new("submit", "draft", "review")
                .guarded_by(Expr::binary(
                    BinaryOp::Greater,
                    row("score"),
                    Expr::integer(0),
                ))
                .carried_by("approve")
                .run_by(&["clerk"]),
            Transition::new("close", "review", "closed"),
        ],
        timers: vec![
            Timer {
                id: TimerId::new("remind"),
                state: StateId::new("review"),
                after_millis: 1_000,
                on_expiry: TimerAction::Emit(EventId::new("reminded")),
            },
            Timer {
                id: TimerId::new("escalate"),
                state: StateId::new("review"),
                after_millis: 2_000,
                on_expiry: TimerAction::Run(ActionId::new("approve")),
            },
        ],
        deadlines: vec![Deadline {
            id: TimerId::new("lapse"),
            state: StateId::new("draft"),
            due: Expr::Call {
                function: Builtin::TimestampPlus,
                args: vec![row("filed_at"), Expr::integer(86_400_000)],
            },
            on_miss: TimerAction::MoveTo(StateId::new("closed")),
        }],
        cycles: CyclePolicy::Forbid,
        summary: "The case moves from draft to closed.".to_string(),
    }
}

fn report(workflow: &Workflow) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root().field("collections"));
    let action_defs = actions();
    let actions = ActionRegistry::build(&action_defs);
    workflow.validate(
        &registry,
        &collections,
        &identities(),
        &actions,
        &Path::root().field("workflows").index(0),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_workflow_passes_and_lists_its_references() {
    let workflow = clean();
    let found = report(&workflow);
    assert!(found.is_clean(), "{found}");
    assert!(workflow.has_state(&StateId::new("review")));
    assert!(!workflow.has_state(&StateId::new("archived")));
    assert_eq!(workflow.state_names(), ["draft", "review", "closed"]);
    let path = Path::root().field("workflows").index(0);
    let actions = workflow.action_refs(&path);
    let listed: Vec<(&str, &str)> = actions
        .iter()
        .map(|(action, location)| (action.as_str(), location.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("approve", "workflows[0].transitions[0].action"),
            ("approve", "workflows[0].timers[1].on_expiry"),
        ]
    );
    let events = workflow.event_refs(&path);
    let listed: Vec<(&str, &str)> = events
        .iter()
        .map(|(event, location)| (event.as_str(), location.as_str()))
        .collect();
    assert_eq!(listed, [("reminded", "workflows[0].timers[0].on_expiry")]);
}

#[test]
fn the_deadline_references_join_the_timer_references() {
    let mut workflow = clean();
    workflow.deadlines.push(Deadline {
        id: TimerId::new("chase"),
        state: StateId::new("review"),
        due: row("filed_at"),
        on_miss: TimerAction::Run(ActionId::new("approve")),
    });
    workflow.deadlines.push(Deadline {
        id: TimerId::new("notice"),
        state: StateId::new("review"),
        due: row("filed_at"),
        on_miss: TimerAction::Emit(EventId::new("overdue")),
    });
    let path = Path::root();
    let actions = workflow.action_refs(&path);
    assert_eq!(actions.len(), 3);
    assert_eq!(actions[2].1.as_str(), "deadlines[1].on_miss");
    let events = workflow.event_refs(&path);
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].0, EventId::new("overdue"));
    assert_eq!(events[1].1.as_str(), "deadlines[2].on_miss");
    assert!(report(&workflow).is_clean());
}

#[test]
fn reachability_follows_transitions_timers_and_deadlines() {
    let mut workflow = clean();
    workflow.states.push(WorkflowState::passing(
        "appeal",
        "Appeal",
        "The case waits for an appeal.",
    ));
    workflow.states.push(WorkflowState::passing(
        "hearing",
        "Hearing",
        "The case waits for a hearing.",
    ));
    workflow.timers.push(Timer {
        id: TimerId::new("to_appeal"),
        state: StateId::new("closed"),
        after_millis: 5,
        on_expiry: TimerAction::MoveTo(StateId::new("appeal")),
    });
    workflow.deadlines.push(Deadline {
        id: TimerId::new("to_hearing"),
        state: StateId::new("appeal"),
        due: row("filed_at"),
        on_miss: TimerAction::MoveTo(StateId::new("hearing")),
    });
    let reached: Vec<String> = workflow.reachable().into_iter().collect();
    assert_eq!(reached, ["appeal", "closed", "draft", "hearing", "review"]);

    let mut lost = clean();
    lost.initial = StateId::new("nowhere");
    assert!(lost.reachable().is_empty());
}

#[test]
fn a_state_no_transition_reaches_is_reported() {
    let mut workflow = clean();
    workflow.transitions.truncate(1);
    workflow.deadlines[0].on_miss = TimerAction::Emit(EventId::new("lapsed"));
    let found = report(&workflow);
    assert_eq!(found.len(), 1);
    let stranded = at(&found, "workflows[0].states[2].id");
    assert_eq!(stranded.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        stranded.problem,
        "A record reaches the state `closed` through no transition."
    );
    assert_eq!(
        stranded.fix,
        "Add a transition into `closed`, or remove the state."
    );
}

#[test]
fn header_and_state_field_faults_each_raise_a_diagnostic() {
    let mut workflow = clean();
    workflow.id = WorkflowId::new("Bad");
    workflow.collection = CollectionId::new("ghost");
    let found = report(&workflow);
    assert_eq!(
        at(&found, "workflows[0].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "workflows[0].collection").fix,
        "Point the reference at one of these collections: case."
    );
    assert!(found.find("workflows[0].state_field").is_none());

    let mut empty_field = clean();
    empty_field.state_field = FieldPath::default();
    let found = report(&empty_field);
    assert_eq!(
        at(&found, "workflows[0].state_field").code,
        DiagnosticCode::MissingValue
    );

    let mut missing_field = clean();
    missing_field.state_field = FieldPath::parse("phase");
    let found = report(&missing_field);
    assert_eq!(
        at(&found, "workflows[0].state_field").fix,
        "Name a field declared on the record type of `case`."
    );

    let mut number_field = clean();
    number_field.state_field = FieldPath::parse("score");
    let found = report(&number_field);
    let wrong = at(&found, "workflows[0].state_field");
    assert_eq!(wrong.code, DiagnosticCode::TypeMismatch);
    assert_eq!(wrong.problem, "The state field carries a whole number.");
    assert_eq!(
        wrong.fix,
        "Point the state field at a field carrying an enumeration."
    );
}

#[test]
fn state_faults_each_raise_a_diagnostic() {
    let mut workflow = clean();
    workflow
        .states
        .push(WorkflowState::passing("archived", "", "The case is kept."));
    workflow.states.push(WorkflowState::passing(
        "draft",
        "Again",
        "A repeated state.",
    ));
    workflow
        .states
        .push(WorkflowState::passing("Bad", "Bad", "A malformed state."));
    let found = report(&workflow);
    let outside = at(&found, "workflows[0].states[3].id");
    assert_eq!(outside.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        outside.fix,
        "Add `archived` to the enumeration `stage`, or rename the state."
    );
    assert_eq!(
        at(&found, "workflows[0].states[3].title").fix,
        "Give the state the name a person reads on screen."
    );
    let repeated: Vec<&Diagnostic> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "workflows[0].states[4].id")
        .collect();
    assert_eq!(repeated.len(), 1);
    assert_eq!(repeated[0].fix, "Rename one of the two `draft` states.");
    let malformed: Vec<DiagnosticCode> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "workflows[0].states[5].id")
        .map(|d| d.code)
        .collect();
    assert!(malformed.contains(&DiagnosticCode::InvalidIdentifier));
}

#[test]
fn a_workflow_without_states_names_both_faults() {
    let mut workflow = clean();
    workflow.states.clear();
    workflow.transitions.clear();
    workflow.timers.clear();
    workflow.deadlines.clear();
    let found = report(&workflow);
    assert_eq!(
        at(&found, "workflows[0].states").fix,
        "Declare at least one state and name it as the initial state."
    );
    assert_eq!(
        at(&found, "workflows[0].initial").fix,
        "Declare a state named `draft` on the workflow `case_flow`."
    );

    let mut wrong_initial = clean();
    wrong_initial.initial = StateId::new("archived");
    let found = report(&wrong_initial);
    let initial = at(&found, "workflows[0].initial");
    assert_eq!(
        initial.problem,
        "The entry names the state `archived`, which `case_flow` lacks."
    );
    assert_eq!(
        initial.fix,
        "Point the entry at one of these states: draft, review, closed."
    );
}

#[test]
fn transition_faults_each_raise_a_diagnostic() {
    let mut workflow = clean();
    workflow.transitions.push(
        Transition::new("submit", "limbo", "nowhere")
            .guarded_by(row("score"))
            .carried_by("ghost_action")
            .run_by(&["Clerk", "auditor"]),
    );
    workflow
        .transitions
        .push(Transition::new("Bad", "closed", "review"));
    workflow
        .transitions
        .push(Transition::new("broken", "draft", "review").guarded_by(Expr::bound("absent")));
    workflow.cycles = CyclePolicy::Allow;
    let found = report(&workflow);
    let prefix = "workflows[0].transitions";
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].id")).fix,
        "Rename one of the two `submit` transitions."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].from")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].to")).problem,
        "The entry names the state `nowhere`, which `case_flow` lacks."
    );
    let guard = at(&found, &alloc::format!("{prefix}[2].guard"));
    assert_eq!(guard.problem, "The guard carries a whole number.");
    assert_eq!(guard.fix, "Give the guard a truth value.");
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].action")).fix,
        "Point the entry at one of these actions: approve."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].roles[0]")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].roles[1]")).fix,
        "Point the entry at one of these roles: clerk."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[3].id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let leaves_final = at(&found, &alloc::format!("{prefix}[3].from"));
    assert_eq!(leaves_final.code, DiagnosticCode::CycleDetected);
    assert_eq!(
        leaves_final.fix,
        "Clear the final mark on `closed`, or remove the transition."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[4].guard")).code,
        DiagnosticCode::DanglingReference
    );
}

#[test]
fn a_loop_is_reported_unless_the_workflow_allows_one() {
    let mut workflow = clean();
    workflow
        .transitions
        .push(Transition::new("reopen", "review", "draft"));
    let found = report(&workflow);
    assert_eq!(found.len(), 1);
    let cycle = at(&found, "workflows[0].transitions[0].to");
    assert_eq!(cycle.code, DiagnosticCode::CycleDetected);
    assert_eq!(
        cycle.problem,
        "The workflow `case_flow` closes a loop: draft -> review -> draft."
    );
    assert_eq!(
        cycle.fix,
        "Remove one transition on the loop, or set the workflow to allow a loop."
    );
    workflow.cycles = CyclePolicy::Allow;
    assert!(report(&workflow).is_clean());
}

#[test]
fn timer_and_deadline_faults_each_raise_a_diagnostic() {
    let mut workflow = clean();
    workflow.timers.push(Timer {
        id: TimerId::new("remind"),
        state: StateId::new("limbo"),
        after_millis: 0,
        on_expiry: TimerAction::MoveTo(StateId::new("nowhere")),
    });
    workflow.timers.push(Timer {
        id: TimerId::new("Bad"),
        state: StateId::new("review"),
        after_millis: 5,
        on_expiry: TimerAction::Run(ActionId::new("ghost_action")),
    });
    workflow.deadlines.push(Deadline {
        id: TimerId::new("escalate"),
        state: StateId::new("limbo"),
        due: row("score"),
        on_miss: TimerAction::Run(ActionId::new("ghost_action")),
    });
    workflow.deadlines.push(Deadline {
        id: TimerId::new("Bad2"),
        state: StateId::new("draft"),
        due: Expr::bound("absent"),
        on_miss: TimerAction::MoveTo(StateId::new("nowhere")),
    });
    let found = report(&workflow);
    let timers = "workflows[0].timers";
    assert_eq!(
        at(&found, &alloc::format!("{timers}[2].id")).fix,
        "Rename one of the two `remind` timers."
    );
    assert_eq!(
        at(&found, &alloc::format!("{timers}[2].state")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{timers}[2].after_millis")).fix,
        "Set a span above zero."
    );
    assert_eq!(
        at(&found, &alloc::format!("{timers}[2].on_expiry")).problem,
        "The entry names the state `nowhere`, which `case_flow` lacks."
    );
    assert_eq!(
        at(&found, &alloc::format!("{timers}[3].id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{timers}[3].on_expiry")).fix,
        "Point the entry at one of these actions: approve."
    );
    let deadlines = "workflows[0].deadlines";
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[1].id")).fix,
        "Rename one of the two `escalate` deadlines."
    );
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[1].state")).code,
        DiagnosticCode::DanglingReference
    );
    let due = at(&found, &alloc::format!("{deadlines}[1].due"));
    assert_eq!(due.problem, "The deadline carries a whole number.");
    assert_eq!(due.fix, "Give the deadline a timestamp.");
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[1].on_miss")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[2].id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[2].due")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{deadlines}[2].on_miss")).fix,
        "Point the entry at one of these states: draft, review, closed."
    );
}

#[test]
fn the_registry_answers_lookups_and_names_the_workflows_on_offer() {
    let mut second = clean();
    second.id = WorkflowId::new("appeal_flow");
    let workflows = vec![clean(), second];
    let registry = WorkflowRegistry::build(&workflows);
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert!(registry.contains(&WorkflowId::new("appeal_flow")));
    assert_eq!(
        registry
            .get(&WorkflowId::new("case_flow"))
            .map(|found| found.states.len()),
        Some(3)
    );
    assert_eq!(registry.names(), ["appeal_flow", "case_flow"]);
    assert_eq!(
        registry
            .missing(&WorkflowId::new("ghost"), Location::default())
            .fix,
        "Point the entry at one of these workflows: appeal_flow, case_flow."
    );
    let none: Vec<Workflow> = Vec::new();
    let empty = WorkflowRegistry::build(&none);
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&WorkflowId::new("ghost"), Location::default())
            .fix,
        "Declare a workflow named `ghost` in the workflows section."
    );
    let bare = Workflow {
        states: Vec::new(),
        ..clean()
    };
    assert_eq!(
        bare.missing_state(&StateId::new("draft"), Location::default())
            .fix,
        "Declare a state named `draft` on the workflow `case_flow`."
    );
    assert_eq!(TransitionId::new("submit"), clean().transitions[0].id);
    assert_eq!(clean().transitions[0].roles, [RoleId::new("clerk")]);
}

#[test]
fn a_repeated_workflow_name_is_reported_by_the_section_pass() {
    let workflows = vec![clean(), clean()];
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let action_defs = actions();
    let actions = ActionRegistry::build(&action_defs);
    let found = validate_workflows(
        &workflows,
        &registry,
        &collections,
        &identities(),
        &actions,
        &Path::root().field("workflows"),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        at(&found, "workflows[1].id").fix,
        "Rename one of the two workflows away from `case_flow`."
    );
}
