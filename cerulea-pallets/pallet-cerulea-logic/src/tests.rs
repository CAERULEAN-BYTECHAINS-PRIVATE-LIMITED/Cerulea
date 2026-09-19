//! The tests of the logic module.

use crate::{mock::*, Bindings, Error, Installed, LogicInterpreter, Pallet, StateView};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    fold::CollectionView,
    types::{
        CollectionId, FieldDef, FieldName, ParamName, RecordDef, TypeDef, TypeExpr, TypeId, Value,
    },
    Blueprint, BlueprintMeta,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::{primitives::DeploymentId, SchemaRegistry};

pub const STEWARD: u64 = 1;

pub fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

pub fn parcel_type() -> TypeDef {
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
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

pub fn parcel_collection() -> Collection {
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

pub fn parcel(survey: &str, holder: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

pub fn base_blueprint() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "district land registry",
        "The registry records land parcels and their holders.",
        "Kollam District Administration",
    ));
    blueprint.types.push(parcel_type());
    blueprint.collections.push(parcel_collection());
    blueprint
        .identities
        .roles
        .push(cerulea_blueprint::identities::RoleDef::new(
            "registrar",
            None,
            "Registrar",
            "The registrar records land transfers.",
        ));
    blueprint
}

pub fn install(blueprint: &Blueprint) -> DeploymentId {
    assert_ok!(Logic::install_blueprint(
        RuntimeOrigin::signed(STEWARD),
        bounded(blueprint.encode())
    ));
    blueprint.id().0
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the traits stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_interpreter<L: LogicInterpreter>() {}
    fn requires_store<S: pallet_cerulea_state::RecordStore>() {}
    fn requires_gate<G: pallet_cerulea_schema::access::PermissionGate>() {}
    fn requires_matrix<M: pallet_cerulea_permission::PermissionMatrix>() {}
    fn requires_identity<I: pallet_cerulea_identity::IdentityRegistry>() {}
    requires_interpreter::<Pallet<Test>>();
    requires_store::<State>();
    requires_gate::<Permission>();
    requires_matrix::<Permission>();
    requires_identity::<Identity>();
}

#[test]
fn an_installed_blueprint_registers_its_types_and_collections() {
    new_test_ext().execute_with(|| {
        let blueprint = base_blueprint();
        let deployment = install(&blueprint);
        assert!(<Pallet<Test> as LogicInterpreter>::is_installed(
            &deployment
        ));
        assert_eq!(
            <Schema as SchemaRegistry>::collection(&deployment, &CollectionId::new("parcel")),
            Some(parcel_collection())
        );
        assert_eq!(<Schema as SchemaRegistry>::type_defs(&deployment).len(), 1);
        let held = <Pallet<Test> as LogicInterpreter>::blueprint(&deployment)
            .expect("the deployment runs a Blueprint");
        assert_eq!(held, blueprint);
    });
}

#[test]
fn the_deployment_identifier_is_the_content_address() {
    new_test_ext().execute_with(|| {
        let blueprint = base_blueprint();
        let deployment = install(&blueprint);
        assert_eq!(deployment, blueprint.id().0);
        assert!(Installed::<Test>::contains_key(deployment));
    });
}

#[test]
fn a_repeated_installation_is_declined() {
    new_test_ext().execute_with(|| {
        let blueprint = base_blueprint();
        install(&blueprint);
        assert_noop!(
            Logic::install_blueprint(RuntimeOrigin::signed(STEWARD), bounded(blueprint.encode())),
            Error::<Test>::BlueprintPresent
        );
    });
}

#[test]
fn a_blueprint_with_a_dangling_reference_is_declined() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        blueprint.types.clear();
        assert_noop!(
            Logic::install_blueprint(RuntimeOrigin::signed(STEWARD), bounded(blueprint.encode())),
            Error::<Test>::BlueprintInvalid
        );
    });
}

#[test]
fn a_payload_that_fails_to_decode_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Logic::install_blueprint(
                RuntimeOrigin::signed(STEWARD),
                bounded(vec![255u8, 255, 255, 255])
            ),
            Error::<Test>::PayloadUnreadable
        );
    });
}

#[test]
fn an_expression_evaluates_under_its_bindings() {
    new_test_ext().execute_with(|| {
        let deployment = install(&base_blueprint());
        let expression = Expr::binary(BinaryOp::Add, Expr::bound("left"), Expr::integer(5));
        let bindings: Bindings = vec![(ParamName::new("left"), Value::Integer(7))];
        let (held, steps) =
            Pallet::<Test>::run_expression(&deployment, &STEWARD, &expression, &bindings)
                .expect("the expression evaluates");
        assert_eq!(held, Value::Integer(12));
        assert!(steps > 0);
    });
}

#[test]
fn an_expression_spending_its_budget_aborts() {
    new_test_ext().execute_with(|| {
        let deployment = install(&base_blueprint());
        // A fold declaring a maximum of ten elements over a list of two
        // thousand runs past its declared bound.
        let mut items = Vec::new();
        for held in 0..2_000i128 {
            items.push(Value::Integer(held));
        }
        let fold = cerulea_blueprint::fold::FoldExpr::over_list(
            Expr::Literal(Value::list(TypeExpr::Integer, items)),
            "item",
            "total",
            Expr::integer(0),
            Expr::binary(BinaryOp::Add, Expr::bound("total"), Expr::bound("item")),
            10,
        );
        let expression = Expr::Fold(Box::new(fold));
        assert_noop!(
            Pallet::<Test>::run_expression(&deployment, &STEWARD, &expression, &Vec::new()),
            Error::<Test>::BudgetSpent
        );
    });
}

#[test]
fn an_expression_call_writes_its_result_digest() {
    new_test_ext().execute_with(|| {
        let deployment = install(&base_blueprint());
        let expression = Expr::integer(11);
        assert_ok!(Logic::evaluate_expression(
            RuntimeOrigin::signed(STEWARD),
            deployment,
            bounded(expression.encode()),
            bounded(Bindings::new().encode())
        ));
        let found = System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::Logic(crate::Event::ExpressionEvaluated { digest, .. }) => {
                    Some(digest)
                }
                _ => None,
            })
            .last()
            .expect("the call wrote an event");
        assert_eq!(found, Pallet::<Test>::digest_of(&Value::Integer(11)));
    });
}

#[test]
fn an_expression_over_an_absent_deployment_still_evaluates_a_constant() {
    new_test_ext().execute_with(|| {
        let held =
            Pallet::<Test>::run_expression(&[0u8; 32], &STEWARD, &Expr::integer(3), &Vec::new())
                .expect("a constant needs no Blueprint");
        assert_eq!(held.0, Value::Integer(3));
    });
}

#[test]
fn a_state_view_answers_with_the_records_a_range_reaches() {
    new_test_ext().execute_with(|| {
        let deployment = install(&base_blueprint());
        crate::tests_support::grant_everything(deployment);
        for survey in ["12/3", "44/9"] {
            assert_ok!(<State as pallet_cerulea_state::RecordStore>::insert(
                &deployment,
                &CollectionId::new("parcel"),
                &STEWARD,
                &parcel(survey, "Anitha", 100)
            ));
        }
        let view = StateView::<Test>::new(deployment, 100);
        let held = view.records(
            &CollectionId::new("parcel"),
            &cerulea_blueprint::collections::KeyRange::Full,
        );
        assert_eq!(held.len(), 2);
    });
}

#[test]
fn an_absent_blueprint_names_the_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Pallet::<Test>::installed(&[1u8; 32]),
            Error::<Test>::BlueprintAbsent
        );
    });
}

fn transfer_action() -> cerulea_blueprint::actions::Action {
    let mut action = cerulea_blueprint::actions::Action::new(
        "record_transfer",
        "The registrar records a transfer of one parcel.",
    );
    action.params = vec![cerulea_blueprint::actions::Param::new(
        "parcel",
        TypeExpr::Record(TypeId::new("parcel")),
        "The parcel the registrar records.",
    )];
    action.effects = vec![cerulea_blueprint::actions::Effect::Insert {
        collection: CollectionId::new("parcel"),
        key: cerulea_blueprint::collections::KeySelector::Exact(Expr::field(
            Expr::bound("parcel"),
            "survey_number",
        )),
        record: Expr::bound("parcel"),
    }];
    action
        .roles
        .push(cerulea_blueprint::types::RoleId::new("registrar"));
    action
}

fn action_blueprint() -> Blueprint {
    let mut blueprint = base_blueprint();
    blueprint.actions.push(transfer_action());
    blueprint
}

fn arguments(record: &Value) -> Bindings {
    vec![(ParamName::new("parcel"), record.clone())]
}

#[test]
fn an_action_runs_its_effect_sequence_through_the_state_module() {
    new_test_ext().execute_with(|| {
        let blueprint = action_blueprint();
        let deployment = install(&blueprint);
        crate::tests_support::grant_everything(deployment);
        let record = parcel("12/3", "Anitha", 400);
        let outcome = Pallet::<Test>::run_action(
            &deployment,
            &crate::tests_support::ACTOR,
            "record_transfer",
            &arguments(&record),
        )
        .expect("the action runs");
        assert_eq!(outcome.effects, 1);
        assert!(outcome.succeeded());
        assert_eq!(
            <State as pallet_cerulea_state::RecordStore>::count(
                &deployment,
                &CollectionId::new("parcel")
            ),
            1
        );
    });
}

#[test]
fn an_action_call_writes_the_execution_event() {
    new_test_ext().execute_with(|| {
        let deployment = install(&action_blueprint());
        crate::tests_support::grant_everything(deployment);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(Logic::execute_action(
            RuntimeOrigin::signed(crate::tests_support::ACTOR),
            deployment,
            crate::tests_support::name("record_transfer"),
            bounded(arguments(&record).encode())
        ));
        let found = System::events()
            .into_iter()
            .filter_map(|held| match held.event {
                RuntimeEvent::Logic(crate::Event::ActionExecuted { effects, .. }) => Some(effects),
                _ => None,
            })
            .last()
            .expect("the call wrote an event");
        assert_eq!(found, 1);
    });
}

#[test]
fn an_action_missing_a_parameter_is_declined() {
    new_test_ext().execute_with(|| {
        let deployment = install(&action_blueprint());
        crate::tests_support::grant_everything(deployment);
        assert_noop!(
            Pallet::<Test>::run_action(
                &deployment,
                &crate::tests_support::ACTOR,
                "record_transfer",
                &Vec::new()
            ),
            Error::<Test>::ParameterMissing
        );
    });
}

#[test]
fn an_absent_action_names_the_fix() {
    new_test_ext().execute_with(|| {
        let deployment = install(&action_blueprint());
        crate::tests_support::grant_everything(deployment);
        assert_noop!(
            Pallet::<Test>::run_action(
                &deployment,
                &crate::tests_support::ACTOR,
                "retire_parcel",
                &Vec::new()
            ),
            Error::<Test>::ActionUnknown
        );
    });
}

#[test]
fn an_action_without_the_declared_role_is_declined() {
    new_test_ext().execute_with(|| {
        let deployment = install(&action_blueprint());
        let record = parcel("12/3", "Anitha", 400);
        // The account registers and holds no role.
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(crate::tests_support::ACTOR),
            crate::tests_support::ACTOR,
            [7u8; 32]
        ));
        assert_noop!(
            Pallet::<Test>::run_action(
                &deployment,
                &crate::tests_support::ACTOR,
                "record_transfer",
                &arguments(&record)
            ),
            Error::<Test>::PermissionDenied
        );
    });
}

#[test]
fn every_effect_of_an_action_sits_inside_its_manifest() {
    new_test_ext().execute_with(|| {
        let mut blueprint = action_blueprint();
        blueprint.actions[0]
            .effects
            .push(cerulea_blueprint::actions::Effect::Delete {
                collection: CollectionId::new("parcel"),
                key: cerulea_blueprint::collections::KeySelector::Full,
            });
        let deployment = install(&blueprint);
        crate::tests_support::grant_everything(deployment);
        let record = parcel("12/3", "Anitha", 400);
        // The manifest derives from the effects themselves, so the full
        // range the second effect names sits inside it.
        let outcome = Pallet::<Test>::run_action(
            &deployment,
            &crate::tests_support::ACTOR,
            "record_transfer",
            &arguments(&record),
        )
        .expect("the action runs");
        assert_eq!(outcome.effects, 2);
        assert_eq!(
            <State as pallet_cerulea_state::RecordStore>::count(
                &deployment,
                &CollectionId::new("parcel")
            ),
            0
        );
    });
}

#[test]
fn a_failing_action_runs_its_compensating_steps_in_reverse() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        let mut ledger = parcel_collection();
        ledger.id = CollectionId::new("ledger");
        blueprint.collections.push(ledger);

        let mut action = transfer_action();
        // The second effect reads a record the collection lacks, so the
        // run fails part way through.
        action
            .effects
            .push(cerulea_blueprint::actions::Effect::Read {
                collection: CollectionId::new("ledger"),
                key: cerulea_blueprint::collections::KeySelector::Exact(Expr::text("absent")),
                binding: ParamName::new("entry"),
                element: TypeExpr::Record(TypeId::new("parcel")),
            });
        // The compensating step writes a marker into the ledger.
        action.compensation = vec![cerulea_blueprint::actions::Effect::Insert {
            collection: CollectionId::new("ledger"),
            key: cerulea_blueprint::collections::KeySelector::Exact(Expr::field(
                Expr::bound("parcel"),
                "survey_number",
            )),
            record: Expr::bound("parcel"),
        }];
        blueprint.actions.push(action);

        let deployment = install(&blueprint);
        crate::tests_support::grant_over(deployment, &["parcel", "ledger"], &[]);
        let record = parcel("12/3", "Anitha", 400);
        let outcome = Pallet::<Test>::run_action(
            &deployment,
            &crate::tests_support::ACTOR,
            "record_transfer",
            &arguments(&record),
        )
        .expect("the compensating steps run");
        assert!(!outcome.succeeded());
        assert!(!outcome
            .failure
            .expect("the run names the failure")
            .is_empty());
        // The effects rolled back and the compensation landed.
        assert_eq!(
            <State as pallet_cerulea_state::RecordStore>::count(
                &deployment,
                &CollectionId::new("parcel")
            ),
            0
        );
        assert_eq!(
            <State as pallet_cerulea_state::RecordStore>::count(
                &deployment,
                &CollectionId::new("ledger")
            ),
            1
        );
    });
}

#[test]
fn a_failing_action_without_compensation_aborts_the_transaction() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        let mut action = transfer_action();
        action
            .effects
            .push(cerulea_blueprint::actions::Effect::Read {
                collection: CollectionId::new("parcel"),
                key: cerulea_blueprint::collections::KeySelector::Exact(Expr::text("absent")),
                binding: ParamName::new("entry"),
                element: TypeExpr::Record(TypeId::new("parcel")),
            });
        blueprint.actions.push(action);
        let deployment = install(&blueprint);
        crate::tests_support::grant_everything(deployment);
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(
            Pallet::<Test>::run_action(
                &deployment,
                &crate::tests_support::ACTOR,
                "record_transfer",
                &arguments(&record)
            ),
            Error::<Test>::RecordInvalid
        );
        assert_eq!(
            <State as pallet_cerulea_state::RecordStore>::count(
                &deployment,
                &CollectionId::new("parcel")
            ),
            0
        );
    });
}

#[test]
fn a_field_path_writes_the_value_it_names() {
    let record = parcel("12/3", "Anitha", 400);
    let changed = crate::execute::set_at(
        &record,
        &cerulea_blueprint::types::FieldPath::parse("holder"),
        Value::Text("Benny".to_string()),
    )
    .expect("the field takes the value");
    match changed {
        Value::Record(held) => {
            assert_eq!(
                held.field(&FieldName::new("holder")),
                Some(&Value::Text("Benny".to_string()))
            );
        }
        other => unreachable!("the write produced {:?}", other),
    }
    assert!(crate::execute::set_at(
        &record,
        &cerulea_blueprint::types::FieldPath::parse("ward"),
        Value::Text("north".to_string())
    )
    .is_none());
}

fn holder_view() -> cerulea_blueprint::views::View {
    let mut view = cerulea_blueprint::views::View::new(
        "holders",
        "parcel",
        "The view lists the holder of every parcel.",
    );
    view.purposes = vec![cerulea_blueprint::types::PurposeTag::new("registration")];
    view.projection = vec![
        cerulea_blueprint::views::Projection::computed(
            "survey_number",
            Expr::field(Expr::bound("record"), "survey_number"),
        ),
        cerulea_blueprint::views::Projection::computed(
            "holder",
            Expr::field(Expr::bound("record"), "holder"),
        ),
        cerulea_blueprint::views::Projection::computed(
            "area",
            Expr::field(Expr::bound("record"), "area"),
        ),
    ];
    view
}

fn view_blueprint(view: cerulea_blueprint::views::View) -> Blueprint {
    let mut blueprint = base_blueprint();
    blueprint.views.push(view);
    blueprint
}

fn land(deployment: pallet_cerulea_schema::primitives::DeploymentId, rows: &[(&str, &str, i128)]) {
    for (survey, holder, area) in rows.iter() {
        assert_ok!(<State as pallet_cerulea_state::RecordStore>::insert(
            &deployment,
            &CollectionId::new("parcel"),
            &crate::tests_support::ACTOR,
            &parcel(survey, holder, *area)
        ));
    }
}

fn value_at(row: &crate::Row, name: &str) -> Value {
    crate::view::field_of(row, &FieldName::new(name))
        .cloned()
        .expect("the row carries the field")
}

#[test]
fn a_view_projects_the_fields_it_names() {
    new_test_ext().execute_with(|| {
        let deployment = install(&view_blueprint(holder_view()));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[("12/3", "Anitha", 400), ("44/9", "Benny", 100)],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "holders")
            .expect("the view runs");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 3);
    });
}

#[test]
fn a_view_filter_keeps_the_rows_it_admits() {
    new_test_ext().execute_with(|| {
        let mut view = holder_view();
        view.filter = Some(Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound("record"), "area"),
            Expr::integer(200),
        ));
        let deployment = install(&view_blueprint(view));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[("12/3", "Anitha", 400), ("44/9", "Benny", 100)],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "holders")
            .expect("the view runs");
        assert_eq!(rows.len(), 1);
        assert_eq!(value_at(&rows[0], "holder"), Value::Text("Anitha".into()));
    });
}

#[test]
fn a_view_sorts_by_the_keys_it_names() {
    new_test_ext().execute_with(|| {
        let mut view = holder_view();
        view.sort = vec![cerulea_blueprint::views::SortKey::descending("area")];
        let deployment = install(&view_blueprint(view));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[
                ("12/3", "Anitha", 400),
                ("44/9", "Benny", 100),
                ("55/5", "Chandra", 900),
            ],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "holders")
            .expect("the view runs");
        assert_eq!(value_at(&rows[0], "area"), Value::Integer(900));
        assert_eq!(value_at(&rows[2], "area"), Value::Integer(100));
    });
}

#[test]
fn a_view_stops_at_its_limit() {
    new_test_ext().execute_with(|| {
        let mut view = holder_view();
        view.sort = vec![cerulea_blueprint::views::SortKey::ascending("area")];
        view.limit = Some(2);
        let deployment = install(&view_blueprint(view));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[
                ("12/3", "Anitha", 400),
                ("44/9", "Benny", 100),
                ("55/5", "Chandra", 900),
            ],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "holders")
            .expect("the view runs");
        assert_eq!(rows.len(), 2);
        assert_eq!(value_at(&rows[0], "area"), Value::Integer(100));
    });
}

#[test]
fn a_view_counts_and_totals_one_group() {
    new_test_ext().execute_with(|| {
        let mut view = cerulea_blueprint::views::View::new(
            "by_holder",
            "parcel",
            "The view totals the area each holder holds.",
        );
        view.purposes = vec![cerulea_blueprint::types::PurposeTag::new("registration")];
        view.projection = vec![cerulea_blueprint::views::Projection::computed(
            "holder",
            Expr::field(Expr::bound("record"), "holder"),
        )];
        view.group_by = vec![FieldName::new("holder")];
        view.aggregates = vec![
            cerulea_blueprint::views::Aggregate::count("parcels"),
            cerulea_blueprint::views::Aggregate::total(
                "total_area",
                Expr::field(Expr::bound("record"), "area"),
            ),
        ];
        view.sort = vec![cerulea_blueprint::views::SortKey::ascending("holder")];
        let deployment = install(&view_blueprint(view));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[
                ("12/3", "Anitha", 400),
                ("44/9", "Anitha", 100),
                ("55/5", "Benny", 900),
            ],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "by_holder")
            .expect("the view runs");
        assert_eq!(rows.len(), 2);
        assert_eq!(value_at(&rows[0], "holder"), Value::Text("Anitha".into()));
        assert_eq!(value_at(&rows[0], "parcels"), Value::Integer(2));
        assert_eq!(value_at(&rows[0], "total_area"), Value::Integer(500));
        assert_eq!(value_at(&rows[1], "parcels"), Value::Integer(1));
        assert_eq!(value_at(&rows[1], "total_area"), Value::Integer(900));
    });
}

#[test]
fn a_view_takes_the_ends_of_a_range() {
    new_test_ext().execute_with(|| {
        let mut view = cerulea_blueprint::views::View::new(
            "extremes",
            "parcel",
            "The view reports the smallest and the largest parcel.",
        );
        view.purposes = vec![cerulea_blueprint::types::PurposeTag::new("registration")];
        view.projection = vec![cerulea_blueprint::views::Projection::computed(
            "register",
            Expr::text("parcel"),
        )];
        view.group_by = vec![FieldName::new("register")];
        view.aggregates = vec![
            cerulea_blueprint::views::Aggregate {
                name: FieldName::new("smallest"),
                function: cerulea_blueprint::views::AggregateFunction::Smallest,
                value: Some(Expr::field(Expr::bound("record"), "area")),
            },
            cerulea_blueprint::views::Aggregate {
                name: FieldName::new("largest"),
                function: cerulea_blueprint::views::AggregateFunction::Largest,
                value: Some(Expr::field(Expr::bound("record"), "area")),
            },
        ];
        let deployment = install(&view_blueprint(view));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[
                ("12/3", "Anitha", 400),
                ("44/9", "Benny", 100),
                ("55/5", "Chandra", 900),
            ],
        );
        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "extremes")
            .expect("the view runs");
        assert_eq!(rows.len(), 1);
        assert_eq!(value_at(&rows[0], "smallest"), Value::Integer(100));
        assert_eq!(value_at(&rows[0], "largest"), Value::Integer(900));
    });
}

#[test]
fn a_matched_join_keeps_the_rows_that_match() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        let mut ledger = parcel_collection();
        ledger.id = CollectionId::new("ledger");
        blueprint.collections.push(ledger);

        let mut view = cerulea_blueprint::views::View::new(
            "joined",
            "parcel",
            "The view joins each parcel to its ledger entry.",
        );
        view.purposes = vec![cerulea_blueprint::types::PurposeTag::new("registration")];
        view.joins = vec![cerulea_blueprint::views::Join {
            collection: CollectionId::new("ledger"),
            alias: ParamName::new("entry"),
            left: cerulea_blueprint::types::FieldPath::parse("survey_number"),
            right: cerulea_blueprint::types::FieldPath::parse("survey_number"),
            kind: cerulea_blueprint::views::JoinKind::Matched,
        }];
        view.projection = vec![
            cerulea_blueprint::views::Projection::computed(
                "survey_number",
                Expr::field(Expr::bound("record"), "survey_number"),
            ),
            cerulea_blueprint::views::Projection::computed(
                "entry_holder",
                Expr::field(Expr::bound("entry"), "holder"),
            ),
        ];
        blueprint.views.push(view);

        let deployment = install(&blueprint);
        crate::tests_support::grant_over(deployment, &["parcel", "ledger"], &[]);
        land(
            deployment,
            &[("12/3", "Anitha", 400), ("44/9", "Benny", 100)],
        );
        assert_ok!(<State as pallet_cerulea_state::RecordStore>::insert(
            &deployment,
            &CollectionId::new("ledger"),
            &crate::tests_support::ACTOR,
            &parcel("12/3", "Ledger Anitha", 400)
        ));

        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "joined")
            .expect("the view runs");
        assert_eq!(rows.len(), 1);
        assert_eq!(
            value_at(&rows[0], "entry_holder"),
            Value::Text("Ledger Anitha".into())
        );
    });
}

#[test]
fn an_optional_join_keeps_the_rows_that_match_nothing() {
    new_test_ext().execute_with(|| {
        let mut blueprint = base_blueprint();
        let mut ledger = parcel_collection();
        ledger.id = CollectionId::new("ledger");
        blueprint.collections.push(ledger);

        let mut view = cerulea_blueprint::views::View::new(
            "joined",
            "parcel",
            "The view joins each parcel to its ledger entry where one sits.",
        );
        view.purposes = vec![cerulea_blueprint::types::PurposeTag::new("registration")];
        view.joins = vec![cerulea_blueprint::views::Join {
            collection: CollectionId::new("ledger"),
            alias: ParamName::new("entry"),
            left: cerulea_blueprint::types::FieldPath::parse("survey_number"),
            right: cerulea_blueprint::types::FieldPath::parse("survey_number"),
            kind: cerulea_blueprint::views::JoinKind::Optional,
        }];
        view.projection = vec![cerulea_blueprint::views::Projection::computed(
            "survey_number",
            Expr::field(Expr::bound("record"), "survey_number"),
        )];
        blueprint.views.push(view);

        let deployment = install(&blueprint);
        crate::tests_support::grant_over(deployment, &["parcel", "ledger"], &[]);
        land(
            deployment,
            &[("12/3", "Anitha", 400), ("44/9", "Benny", 100)],
        );
        assert_ok!(<State as pallet_cerulea_state::RecordStore>::insert(
            &deployment,
            &CollectionId::new("ledger"),
            &crate::tests_support::ACTOR,
            &parcel("12/3", "Ledger Anitha", 400)
        ));

        let rows = Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "joined")
            .expect("the view runs");
        assert_eq!(rows.len(), 2);
    });
}

#[test]
fn a_view_reads_the_rows_the_caller_reaches() {
    new_test_ext().execute_with(|| {
        let deployment = install(&view_blueprint(holder_view()));
        crate::tests_support::grant_everything(deployment);
        land(deployment, &[("12/3", "Anitha", 400)]);
        // A second account holds no role, so the view returns nothing.
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(crate::tests_support::ACTOR),
            9u64,
            [9u8; 32]
        ));
        let rows = Pallet::<Test>::run_view(&deployment, &9u64, "holders").expect("the view runs");
        assert!(rows.is_empty());
    });
}

#[test]
fn an_absent_view_names_the_fix() {
    new_test_ext().execute_with(|| {
        let deployment = install(&view_blueprint(holder_view()));
        crate::tests_support::grant_everything(deployment);
        assert_noop!(
            Pallet::<Test>::run_view(&deployment, &crate::tests_support::ACTOR, "ledger"),
            Error::<Test>::ViewUnknown
        );
    });
}

#[test]
fn a_view_call_writes_its_row_count_and_digest() {
    new_test_ext().execute_with(|| {
        let deployment = install(&view_blueprint(holder_view()));
        crate::tests_support::grant_everything(deployment);
        land(
            deployment,
            &[("12/3", "Anitha", 400), ("44/9", "Benny", 100)],
        );
        assert_ok!(Logic::evaluate_view_call(
            RuntimeOrigin::signed(crate::tests_support::ACTOR),
            deployment,
            crate::tests_support::name("holders")
        ));
        let found = System::events()
            .into_iter()
            .filter_map(|held| match held.event {
                RuntimeEvent::Logic(crate::Event::ViewEvaluated { rows, .. }) => Some(rows),
                _ => None,
            })
            .last()
            .expect("the call wrote an event");
        assert_eq!(found, 2);
    });
}

#[test]
fn two_numeric_values_add_and_a_mixed_pair_declines() {
    assert_eq!(
        crate::view::add_values(&Value::Integer(2), &Value::Integer(3)),
        Some(Value::Integer(5))
    );
    assert_eq!(
        crate::view::add_values(&Value::Integer(2), &Value::Text("three".into())),
        None
    );
    assert_eq!(
        crate::view::zero_like(&Value::Integer(9)),
        Some(Value::Integer(0))
    );
    assert_eq!(crate::view::zero_like(&Value::Bool(true)), None);
}
