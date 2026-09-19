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
