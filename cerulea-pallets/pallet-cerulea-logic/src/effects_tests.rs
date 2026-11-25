//! Further tests of the logic module.
//!
//! These tests cover every effect kind, the stage checks of an action, the
//! refusal branches of the calls, the view passes the earlier tests leave
//! out, the genesis installation and the plain sentence a compensation
//! writes.
//!
//! A few tests stage a Blueprint straight into storage after a valid
//! installation. Installation refuses a Blueprint with a dangling
//! reference, while the interpreter still guards against one. Staging
//! reaches those guards.

use crate::{
    execute::ExecutionOutcome,
    mock::*,
    tests::{base_blueprint, bounded, install, parcel, parcel_collection},
    tests_support::{grant_everything, grant_over, name, ACTOR},
    Bindings, Error, Event, Installed, LogicInterpreter, Pallet,
};
use cerulea_blueprint::{
    actions::{Action, Effect, Param},
    collections::KeySelector,
    compose::Block,
    events::EventDef,
    expression::{BinaryOp, Expr},
    rules::{Invariant, Precondition},
    types::{
        CollectionId, Decimal, EventId, FieldName, FieldPath, ParamName, RoleId, RuleId, StateId,
        TypeExpr, TypeId, Value, WorkflowId,
    },
    views::{Aggregate, AggregateFunction, Join, JoinKind, Projection, SortKey, View},
    Blueprint,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::BuildGenesisConfig, BoundedVec};
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use pallet_cerulea_state::RecordStore;
use sp_runtime::DispatchError;

fn parcel_id() -> CollectionId {
    CollectionId::new("parcel")
}

fn exact(survey: &str) -> KeySelector {
    KeySelector::Exact(Expr::text(survey))
}

fn record_param() -> Param {
    Param::new(
        "parcel",
        TypeExpr::Record(TypeId::new("parcel")),
        "The parcel the action names.",
    )
}

fn arguments(record: &Value) -> Bindings {
    vec![(ParamName::new("parcel"), record.clone())]
}

fn insert_effect() -> Effect {
    Effect::Insert {
        collection: parcel_id(),
        key: KeySelector::Exact(Expr::field(Expr::bound("parcel"), "survey_number")),
        record: Expr::bound("parcel"),
    }
}

fn recorded_event() -> EventDef {
    EventDef::new(
        "parcel_recorded",
        "parcel",
        "The registry recorded one parcel.",
    )
}

fn emit_effect(payload: Expr) -> Effect {
    Effect::Emit {
        event: EventId::new("parcel_recorded"),
        payload,
    }
}

fn action(id: &str, effects: Vec<Effect>) -> Action {
    let mut held = Action::new(id, "One action of the land registry.");
    held.effects = effects;
    held
}

/// Install the base Blueprint, then stage the given Blueprint over it.
fn stage(blueprint: Blueprint) -> DeploymentId {
    let deployment = install(&base_blueprint());
    Installed::<Test>::insert(deployment, blueprint);
    deployment
}

fn staged_with(actions: Vec<Action>) -> DeploymentId {
    let mut blueprint = base_blueprint();
    blueprint.events.push(recorded_event());
    blueprint.actions = actions;
    let deployment = stage(blueprint);
    grant_everything(deployment);
    deployment
}

fn land(deployment: DeploymentId, survey: &str, holder: &str, area: i128) {
    assert_ok!(<State as RecordStore>::insert(
        &deployment,
        &parcel_id(),
        &ACTOR,
        &parcel(survey, holder, area)
    ));
}

fn run(
    deployment: DeploymentId,
    action: &str,
    record: &Value,
) -> Result<ExecutionOutcome, DispatchError> {
    Pallet::<Test>::run_action(&deployment, &ACTOR, action, &arguments(record))
}

fn unreadable_name() -> BoundedName {
    BoundedVec::try_from(vec![0xff, 0xfe]).expect("the name fits")
}

#[test]
fn an_update_effect_writes_the_named_field_of_every_record_in_range() {
    new_test_ext().execute_with(|| {
        let mut rename = action(
            "rename_holder",
            vec![Effect::Update {
                collection: parcel_id(),
                key: exact("12/3"),
                field: FieldPath::parse("holder"),
                value: Expr::text("Benny"),
            }],
        );
        rename.params = vec![record_param()];
        let deployment = staged_with(vec![rename]);
        land(deployment, "12/3", "Anitha", 400);
        land(deployment, "44/9", "Chandra", 100);

        let outcome = run(deployment, "rename_holder", &parcel("12/3", "Anitha", 400))
            .expect("the update runs");
        assert_eq!(outcome.effects, 1);
        assert_eq!(outcome.touched.len(), 1);
        let key = outcome.touched[0].1.clone();
        assert_eq!(
            <State as RecordStore>::read(&deployment, &parcel_id(), &key),
            Some(parcel("12/3", "Benny", 400))
        );
    });
}

#[test]
fn an_update_naming_an_absent_field_stops_the_action() {
    new_test_ext().execute_with(|| {
        let mut rename = action(
            "rename_ward",
            vec![Effect::Update {
                collection: parcel_id(),
                key: exact("12/3"),
                field: FieldPath::parse("ward"),
                value: Expr::text("north"),
            }],
        );
        rename.params = vec![record_param()];
        let deployment = staged_with(vec![rename]);
        land(deployment, "12/3", "Anitha", 400);
        assert_noop!(
            run(deployment, "rename_ward", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::RecordInvalid
        );
    });
}

#[test]
fn a_delete_effect_removes_the_records_its_key_reaches() {
    new_test_ext().execute_with(|| {
        let mut retire = action(
            "retire_parcel",
            vec![Effect::Delete {
                collection: parcel_id(),
                key: exact("12/3"),
            }],
        );
        retire.params = vec![record_param()];
        let mut sweep = action(
            "sweep_prefix",
            vec![Effect::Delete {
                collection: parcel_id(),
                key: KeySelector::Prefix(Expr::text("44/9")),
            }],
        );
        sweep.params = vec![record_param()];
        let deployment = staged_with(vec![retire, sweep]);
        land(deployment, "12/3", "Anitha", 400);
        land(deployment, "44/9", "Benny", 100);
        land(deployment, "55/5", "Chandra", 900);

        assert_ok!(run(
            deployment,
            "retire_parcel",
            &parcel("12/3", "Anitha", 400)
        ));
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 2);
        assert_ok!(run(
            deployment,
            "sweep_prefix",
            &parcel("12/3", "Anitha", 400)
        ));
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 1);
    });
}

#[test]
fn a_read_effect_binds_the_record_for_a_later_emit() {
    new_test_ext().execute_with(|| {
        let mut announce = action(
            "announce_parcel",
            vec![
                Effect::Read {
                    collection: parcel_id(),
                    key: exact("12/3"),
                    binding: ParamName::new("entry"),
                    element: TypeExpr::Record(TypeId::new("parcel")),
                },
                emit_effect(Expr::bound("entry")),
            ],
        );
        announce.params = vec![record_param()];
        let deployment = staged_with(vec![announce]);
        land(deployment, "12/3", "Anitha", 400);

        let outcome = run(deployment, "announce_parcel", &parcel("99/9", "Nobody", 1))
            .expect("the action runs");
        assert_eq!(
            outcome.events,
            vec![(
                EventId::new("parcel_recorded"),
                parcel("12/3", "Anitha", 400)
            )]
        );
        System::assert_has_event(RuntimeEvent::Logic(Event::EventEmitted {
            deployment,
            event: name("parcel_recorded"),
            actor: ACTOR,
            digest: Pallet::<Test>::digest_of(&parcel("12/3", "Anitha", 400)),
        }));
    });
}

#[test]
fn an_emit_naming_an_undeclared_event_stops_the_action() {
    new_test_ext().execute_with(|| {
        let mut shout = action(
            "shout",
            vec![Effect::Emit {
                event: EventId::new("parcel_shouted"),
                payload: Expr::bound("parcel"),
            }],
        );
        shout.params = vec![record_param()];
        let deployment = staged_with(vec![shout]);
        assert_noop!(
            run(deployment, "shout", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::EventUnknown
        );
    });
}

#[test]
fn a_transition_without_the_workflow_module_names_the_fix() {
    new_test_ext().execute_with(|| {
        let mut approve = action(
            "approve_parcel",
            vec![Effect::Transition {
                workflow: WorkflowId::new("approval"),
                collection: parcel_id(),
                key: exact("12/3"),
                to: StateId::new("approved"),
            }],
        );
        approve.params = vec![record_param()];
        let deployment = staged_with(vec![approve]);

        // A transition over an absent record stops before the sink.
        assert_noop!(
            run(deployment, "approve_parcel", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::RecordInvalid
        );
        land(deployment, "12/3", "Anitha", 400);
        assert_noop!(
            run(deployment, "approve_parcel", &parcel("12/3", "Anitha", 400)),
            DispatchError::Other("Add the workflow module to the runtime to run a transition.")
        );
    });
}

#[test]
fn a_block_runs_its_effects_with_the_arguments_it_receives() {
    new_test_ext().execute_with(|| {
        let mut block = Block::new("file_parcel", "The block files one parcel.");
        block.inputs = vec![Param::new(
            "item",
            TypeExpr::Record(TypeId::new("parcel")),
            "The parcel the block files.",
        )];
        block.effects = vec![Effect::Insert {
            collection: parcel_id(),
            key: KeySelector::Exact(Expr::field(Expr::bound("item"), "survey_number")),
            record: Expr::bound("item"),
        }];
        let mut file = action(
            "file",
            vec![Effect::RunBlock {
                block: cerulea_blueprint::types::BlockId::new("file_parcel"),
                arguments: vec![(ParamName::new("item"), Expr::bound("parcel"))],
            }],
        );
        file.params = vec![record_param()];
        let mut stray = action(
            "stray",
            vec![Effect::RunBlock {
                block: cerulea_blueprint::types::BlockId::new("absent_block"),
                arguments: Vec::new(),
            }],
        );
        stray.params = vec![record_param()];

        let mut blueprint = base_blueprint();
        blueprint.blocks.push(block);
        blueprint.actions = vec![file, stray];
        let deployment = stage(blueprint);
        grant_everything(deployment);

        let outcome =
            run(deployment, "file", &parcel("12/3", "Anitha", 400)).expect("the block runs");
        // The run block effect and the insert inside it both count.
        assert_eq!(outcome.effects, 2);
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 1);

        // The manifest derivation reads the block library first, so an
        // absent block stops the action at the manifest.
        assert_noop!(
            run(deployment, "stray", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::ManifestBreach
        );
    });
}

#[test]
fn an_action_past_its_effect_limit_stops() {
    new_test_ext().execute_with(|| {
        let effects: Vec<Effect> = (0..65)
            .map(|_| emit_effect(Expr::bound("parcel")))
            .collect();
        let mut chatty = action("chatty", effects);
        chatty.params = vec![record_param()];
        let deployment = staged_with(vec![chatty]);
        assert_noop!(
            run(deployment, "chatty", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::EffectLimitReached
        );
    });
}

fn positive_area_precondition() -> Precondition {
    Precondition {
        id: RuleId::new("positive_area"),
        inputs: Vec::new(),
        holds: Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound("parcel"), "area"),
            Expr::integer(0),
        ),
        problem: "The parcel carries no area.".to_string(),
        fix: "Record the surveyed area of the parcel.".to_string(),
    }
}

#[test]
fn a_precondition_declining_the_arguments_stops_the_action() {
    new_test_ext().execute_with(|| {
        let mut record = action("record_parcel", vec![insert_effect()]);
        record.params = vec![record_param()];
        record.preconditions = vec![RuleId::new("positive_area")];
        let mut orphan = action("record_orphan", vec![insert_effect()]);
        orphan.params = vec![record_param()];
        orphan.preconditions = vec![RuleId::new("absent_rule")];
        let mut odd = action("record_odd", vec![insert_effect()]);
        odd.params = vec![record_param()];
        odd.preconditions = vec![RuleId::new("text_rule")];

        let mut blueprint = base_blueprint();
        blueprint
            .rules
            .preconditions
            .push(positive_area_precondition());
        blueprint.rules.preconditions.push(Precondition {
            id: RuleId::new("text_rule"),
            inputs: Vec::new(),
            holds: Expr::text("yes"),
            problem: "The rule answers with text.".to_string(),
            fix: "Give the rule a truth value.".to_string(),
        });
        blueprint.actions = vec![record, orphan, odd];
        let deployment = stage(blueprint);
        grant_everything(deployment);

        assert_noop!(
            run(deployment, "record_parcel", &parcel("12/3", "Anitha", 0)),
            Error::<Test>::PreconditionFailed
        );
        assert_noop!(
            run(deployment, "record_orphan", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::PreconditionFailed
        );
        assert_noop!(
            run(deployment, "record_odd", &parcel("12/3", "Anitha", 400)),
            Error::<Test>::BudgetSpent
        );
        assert_ok!(run(
            deployment,
            "record_parcel",
            &parcel("12/3", "Anitha", 400)
        ));
    });
}

#[test]
fn an_invariant_over_a_touched_collection_rolls_the_effects_back() {
    new_test_ext().execute_with(|| {
        let mut record = action("record_parcel", vec![insert_effect()]);
        record.params = vec![record_param()];
        let mut blueprint = base_blueprint();
        blueprint.rules.invariants.push(Invariant {
            id: RuleId::new("small_parcels"),
            collection: parcel_id(),
            holds: Expr::binary(
                BinaryOp::Less,
                Expr::field(Expr::bound("record"), "area"),
                Expr::integer(1_000),
            ),
            problem: "The parcel runs past the area limit.".to_string(),
            fix: "Split the parcel before recording it.".to_string(),
        });
        blueprint.actions = vec![record];
        let deployment = stage(blueprint);
        grant_everything(deployment);

        assert_noop!(
            run(
                deployment,
                "record_parcel",
                &parcel("12/3", "Anitha", 5_000)
            ),
            Error::<Test>::PreconditionFailed
        );
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 0);
        assert_ok!(run(
            deployment,
            "record_parcel",
            &parcel("12/3", "Anitha", 400)
        ));
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 1);
    });
}

#[test]
fn a_delegation_carrying_the_action_admits_a_holder_without_the_role() {
    new_test_ext().execute_with(|| {
        let mut announce = action("announce", vec![emit_effect(Expr::bound("parcel"))]);
        announce.params = vec![record_param()];
        announce.roles = vec![RoleId::new("registrar")];
        let deployment = staged_with(vec![announce]);
        let clerk: u64 = 2;
        let stranger: u64 = 3;
        for (who, identity) in [(clerk, 2u8), (stranger, 3u8)] {
            assert_ok!(Identity::register_account(
                RuntimeOrigin::signed(ACTOR),
                who,
                [identity; 32]
            ));
        }
        assert_ok!(Identity::delegate(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            clerk,
            name("registrar"),
            BoundedVec::truncate_from(vec![name("announce")]),
            START + 10_000,
            None
        ));
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(Pallet::<Test>::run_action(
            &deployment,
            &clerk,
            "announce",
            &arguments(&record)
        ));
        assert_noop!(
            Pallet::<Test>::run_action(&deployment, &stranger, "announce", &arguments(&record)),
            Error::<Test>::PermissionDenied
        );
    });
}

#[test]
fn a_compensation_writes_its_events_and_the_plain_reason() {
    new_test_ext().execute_with(|| {
        let mut approve = action(
            "approve_parcel",
            vec![
                insert_effect(),
                Effect::Transition {
                    workflow: WorkflowId::new("approval"),
                    collection: parcel_id(),
                    key: KeySelector::Exact(Expr::field(Expr::bound("parcel"), "survey_number")),
                    to: StateId::new("approved"),
                },
            ],
        );
        approve.params = vec![record_param()];
        approve.compensation = vec![emit_effect(Expr::bound("parcel"))];
        let deployment = staged_with(vec![approve]);

        let record = parcel("12/3", "Anitha", 400);
        let outcome = run(deployment, "approve_parcel", &record).expect("the compensation runs");
        let reason = "The action stopped at the step reporting Add the workflow module to the runtime to run a transition. The compensating steps ran in reverse.";
        assert_eq!(outcome.failure.as_deref(), Some(reason));
        assert_eq!(<State as RecordStore>::count(&deployment, &parcel_id()), 0);
        System::assert_has_event(RuntimeEvent::Logic(Event::EventEmitted {
            deployment,
            event: name("parcel_recorded"),
            actor: ACTOR,
            digest: Pallet::<Test>::digest_of(&record),
        }));
        System::assert_last_event(RuntimeEvent::Logic(Event::ActionCompensated {
            deployment,
            action: name("approve_parcel"),
            actor: ACTOR,
            reason: BoundedVec::truncate_from(reason.as_bytes().to_vec()),
        }));
    });
}

#[test]
fn every_failure_reads_back_as_one_plain_sentence() {
    let module: DispatchError = Error::<Test>::RecordInvalid.into();
    assert_eq!(
        Pallet::<Test>::failure_sentence(&module),
        "The action stopped at the step reporting RecordInvalid. The compensating steps ran in reverse."
    );
    let unnamed = DispatchError::Module(sp_runtime::ModuleError {
        index: 5,
        error: [0; 4],
        message: None,
    });
    assert_eq!(
        Pallet::<Test>::failure_sentence(&unnamed),
        "The action stopped at one of its steps. The compensating steps ran in reverse."
    );
    assert_eq!(
        Pallet::<Test>::failure_sentence(&DispatchError::Other("the ledger refused the write")),
        "The action stopped at the step reporting the ledger refused the write. The compensating steps ran in reverse."
    );
    assert_eq!(
        Pallet::<Test>::failure_sentence(&DispatchError::BadOrigin),
        "The action stopped at one of its steps. The compensating steps ran in reverse."
    );
}

#[test]
fn an_empty_outcome_reads_as_a_success() {
    let held = ExecutionOutcome::default();
    assert!(held.succeeded());
    assert_eq!(held, ExecutionOutcome::new());
    assert_eq!(held.effects, 0);
}

#[test]
fn the_calls_decline_payloads_and_names_they_cannot_read() {
    new_test_ext().execute_with(|| {
        let deployment = install(&base_blueprint());
        assert_noop!(
            Logic::execute_action(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("record_transfer"),
                bounded(vec![0xff])
            ),
            Error::<Test>::PayloadUnreadable
        );
        assert_noop!(
            Logic::execute_action(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                unreadable_name(),
                bounded(Bindings::new().encode())
            ),
            Error::<Test>::ActionUnknown
        );
        assert_noop!(
            Logic::evaluate_view_call(RuntimeOrigin::signed(ACTOR), deployment, unreadable_name()),
            Error::<Test>::ViewUnknown
        );
        assert_noop!(
            Logic::evaluate_expression(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                bounded(vec![0xff]),
                bounded(Bindings::new().encode())
            ),
            Error::<Test>::PayloadUnreadable
        );
        assert_noop!(
            Logic::evaluate_expression(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                bounded(Expr::integer(1).encode()),
                bounded(vec![0xff])
            ),
            Error::<Test>::PayloadUnreadable
        );
        assert_noop!(
            Pallet::<Test>::bounded(&"a".repeat(200)),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn the_interpreter_answers_what_a_deployment_holds() {
    new_test_ext().execute_with(|| {
        let blueprint = base_blueprint();
        let deployment = install(&blueprint);
        assert!(<Pallet<Test> as LogicInterpreter>::is_installed(
            &deployment
        ));
        assert!(!<Pallet<Test> as LogicInterpreter>::is_installed(
            &[0u8; 32]
        ));
        assert_eq!(
            <Pallet<Test> as LogicInterpreter>::blueprint(&deployment),
            Some(blueprint)
        );
        let bindings: Bindings = vec![(ParamName::new("width"), Value::Integer(20))];
        assert_eq!(
            <Pallet<Test> as LogicInterpreter>::evaluate(
                &deployment,
                &ACTOR,
                &Expr::binary(BinaryOp::Multiply, Expr::bound("width"), Expr::integer(3)),
                &bindings
            ),
            Ok(Value::Integer(60))
        );
        assert_eq!(Pallet::<Test>::identity_of(&ACTOR).0, [0u8; 32]);
    });
}

#[test]
fn a_genesis_blueprint_installs_once() {
    new_test_ext().execute_with(|| {
        let blueprint = base_blueprint();
        let bytes = blueprint.encode();
        let genesis = crate::GenesisConfig::<Test> {
            blueprints: vec![bytes.clone(), bytes],
            marker: Default::default(),
        };
        genesis.build();
        let deployment = blueprint.id().0;
        assert_eq!(Installed::<Test>::get(deployment), Some(blueprint));
        assert_eq!(crate::InstalledAt::<Test>::get(deployment), Some(1));
        assert!(Pallet::<Test>::installed(&deployment).is_ok());
    });
}

fn source_view(id: &str) -> View {
    let mut view = View::new(id, "parcel", "One view over the parcel register.");
    view.projection = vec![
        Projection::computed("holder", Expr::field(Expr::bound("record"), "holder")),
        Projection::computed("area", Expr::field(Expr::bound("record"), "area")),
    ];
    view
}

fn viewing() -> DeploymentId {
    let deployment = install(&base_blueprint());
    grant_everything(deployment);
    land(deployment, "12/3", "Anitha", 400);
    land(deployment, "44/9", "Benny", 100);
    deployment
}

#[test]
fn a_view_naming_roles_admits_their_holders_alone() {
    new_test_ext().execute_with(|| {
        let deployment = viewing();
        let mut view = source_view("registrar_view");
        view.roles = vec![RoleId::new("registrar")];
        let rows = Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &view)
            .expect("the registrar reads the view");
        assert_eq!(rows.len(), 2);
        assert_noop!(
            Pallet::<Test>::evaluate_view(&deployment, &9u64, &view),
            Error::<Test>::PermissionDenied
        );
    });
}

#[test]
fn an_aggregate_over_values_it_cannot_add_reports_a_mismatch() {
    new_test_ext().execute_with(|| {
        let deployment = viewing();
        let mut text_total = source_view("text_total");
        text_total.aggregates = vec![Aggregate::total(
            "holders",
            Expr::field(Expr::bound("record"), "holder"),
        )];
        assert_noop!(
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &text_total),
            Error::<Test>::ResultMismatch
        );
        let mut empty_total = source_view("empty_total");
        empty_total.aggregates = vec![Aggregate {
            name: FieldName::new("total"),
            function: AggregateFunction::Largest,
            value: None,
        }];
        assert_noop!(
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &empty_total),
            Error::<Test>::ResultMismatch
        );
    });
}

#[test]
fn an_ungrouped_aggregate_folds_every_row_into_one() {
    new_test_ext().execute_with(|| {
        let deployment = viewing();
        let mut view = source_view("everything");
        view.group_by = vec![FieldName::new("absent")];
        view.aggregates = vec![
            Aggregate::count("parcels"),
            Aggregate {
                name: FieldName::new("smallest"),
                function: AggregateFunction::Smallest,
                value: Some(Expr::field(Expr::bound("record"), "area")),
            },
        ];
        let rows =
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &view).expect("the view runs");
        assert_eq!(
            rows,
            vec![vec![
                (FieldName::new("absent"), Value::Bool(false)),
                (FieldName::new("parcels"), Value::Integer(2)),
                (FieldName::new("smallest"), Value::Integer(100)),
            ]]
        );
    });
}

#[test]
fn a_join_past_the_row_limit_stops_the_view() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        let mut ledger = parcel_collection();
        ledger.id = CollectionId::new("ledger");
        blueprint.collections.push(ledger);
        let deployment = install(&blueprint);
        grant_over(deployment, &["parcel", "ledger"], &[]);
        // Seventeen parcels meet seventeen ledger entries on one holder, so
        // the join produces 289 rows against a limit of 256.
        for place in 0..17 {
            let survey = alloc_survey(place);
            land(deployment, &survey, "Anitha", 100);
            assert_ok!(<State as RecordStore>::insert(
                &deployment,
                &CollectionId::new("ledger"),
                &ACTOR,
                &parcel(&survey, "Anitha", 100)
            ));
        }
        let mut view = source_view("crowded");
        view.joins = vec![Join {
            collection: CollectionId::new("ledger"),
            alias: ParamName::new("entry"),
            left: FieldPath::parse("holder"),
            right: FieldPath::parse("holder"),
            kind: JoinKind::Matched,
        }];
        assert_noop!(
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &view),
            Error::<Test>::EffectLimitReached
        );
    });
}

fn alloc_survey(place: u32) -> String {
    let mut held = String::from("survey_");
    held.push_str(&place.to_string());
    held
}

#[test]
fn a_join_on_a_field_the_source_lacks_keeps_optional_rows_alone() {
    new_test_ext().execute_with(|| {
        let deployment = viewing();
        let mut matched = source_view("matched");
        matched.joins = vec![Join {
            collection: parcel_id(),
            alias: ParamName::new("twin"),
            left: FieldPath::parse("ward"),
            right: FieldPath::parse("ward"),
            kind: JoinKind::Matched,
        }];
        assert!(Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &matched)
            .expect("the view runs")
            .is_empty());
        let mut optional = matched.clone();
        optional.joins[0].kind = JoinKind::Optional;
        assert_eq!(
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &optional)
                .expect("the view runs")
                .len(),
            2
        );
    });
}

#[test]
fn rows_order_with_the_missing_field_first() {
    let with = vec![(FieldName::new("area"), Value::Integer(5))];
    let without: crate::Row = Vec::new();
    let area = FieldName::new("area");
    assert_eq!(
        crate::view::compare_rows(&with, &without, &area),
        core::cmp::Ordering::Greater
    );
    assert_eq!(
        crate::view::compare_rows(&without, &with, &area),
        core::cmp::Ordering::Less
    );
    assert_eq!(
        crate::view::compare_rows(&without, &without, &area),
        core::cmp::Ordering::Equal
    );
}

#[test]
fn two_decimal_values_add_at_one_scale() {
    let one = Value::Decimal(Decimal::new(150, 2));
    let other = Value::Decimal(Decimal::new(250, 2));
    assert_eq!(
        crate::view::add_values(&one, &other),
        Some(Value::Decimal(Decimal::new(400, 2)))
    );
    assert_eq!(
        crate::view::zero_like(&one),
        Some(Value::Decimal(Decimal::zero(2)))
    );
    assert_eq!(
        crate::view::add_values(&Value::Integer(i128::MAX), &Value::Integer(1)),
        None
    );
}

#[test]
fn a_descending_sort_puts_the_later_name_first() {
    new_test_ext().execute_with(|| {
        let deployment = viewing();
        let mut view = source_view("sorted");
        view.sort = vec![SortKey::descending("holder")];
        let rows =
            Pallet::<Test>::evaluate_view(&deployment, &ACTOR, &view).expect("the view runs");
        assert_eq!(
            crate::view::field_of(&rows[0], &FieldName::new("holder")),
            Some(&Value::Text("Benny".into()))
        );
    });
}
