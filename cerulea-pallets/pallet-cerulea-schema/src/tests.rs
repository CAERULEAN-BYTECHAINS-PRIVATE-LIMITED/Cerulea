//! The tests of the schema module.

use crate::{
    migrate::{inverse_plan, migrate_record, MigrationPlan, MigrationState, MigrationStep},
    mock::*,
    primitives::{name_of, CollectionKey, DeploymentId},
    CollectionDefs, Error, MigrationHook, Pallet, SchemaRegistrar, SchemaRegistry, SchemaSnapshot,
};
use cerulea_blueprint::{
    collections::{Collection, Index, PurposeDeclaration},
    expression::{BinaryOp, Env, Expr, Meter},
    types::{
        FieldDef, FieldName, FieldPath, Identity, IndexName, RecordDef, Timestamp, TypeDef,
        TypeExpr, TypeId, Value,
    },
    Blueprint, BlueprintMeta, Path,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const OTHER: u64 = 2;

fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

fn parcel_collection() -> Collection {
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

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn register_parcel() {
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
}

fn parcel_record(survey: &str, holder: &str) -> Value {
    Value::record(
        TypeId::new("parcel"),
        vec![
            (FieldName::new("survey_number"), Value::Text(survey.into())),
            (FieldName::new("holder"), Value::Text(holder.into())),
        ],
    )
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the trait stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_registry<S: SchemaRegistry>() {}
    fn requires_registrar<S: SchemaRegistrar>() {}
    fn requires_hook<H: MigrationHook>() {}
    requires_registry::<Pallet<Test>>();
    requires_registrar::<Pallet<Test>>();
    requires_hook::<()>();
}

#[test]
fn a_registered_type_resolves_by_name_and_by_hash() {
    new_test_ext().execute_with(|| {
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(parcel_type().encode())
        ));
        let found = <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("parcel"))
            .expect("the registry holds the type");
        assert_eq!(found, parcel_type());
        assert_eq!(Pallet::<Test>::type_defs(&DEPLOYMENT).len(), 1);
    });
}

#[test]
fn a_repeated_type_registration_keeps_one_entry() {
    new_test_ext().execute_with(|| {
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(parcel_type().encode())
        ));
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(parcel_type().encode())
        ));
        assert_eq!(Pallet::<Test>::type_names(&DEPLOYMENT).len(), 1);
    });
}

#[test]
fn a_changed_type_under_one_name_reports_a_conflict() {
    new_test_ext().execute_with(|| {
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(parcel_type().encode())
        ));
        let mut changed = match parcel_type() {
            TypeDef::Record(record) => record,
            _ => unreachable!("the sample carries a record type"),
        };
        changed
            .fields
            .push(FieldDef::new("area", TypeExpr::Integer, "The area."));
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(TypeDef::Record(changed).encode())
            ),
            Error::<Test>::DefinitionConflict
        );
    });
}

#[test]
fn a_collection_naming_an_absent_record_type_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Schema::register_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(parcel_collection().encode())
            ),
            Error::<Test>::DefinitionInvalid
        );
    });
}

#[test]
fn a_registered_collection_starts_at_version_one() {
    new_test_ext().execute_with(|| {
        register_parcel();
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(1)
        );
        assert_eq!(
            Pallet::<Test>::collection_at(&DEPLOYMENT, &id, 1),
            Some(parcel_collection())
        );
        assert!(<Pallet<Test> as SchemaRegistry>::is_installed(&DEPLOYMENT));
    });
}

#[test]
fn an_account_without_stewardship_is_declined() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(OTHER),
                DEPLOYMENT,
                bounded(parcel_type().encode())
            ),
            Error::<Test>::NotSteward
        );
    });
}

fn add_area_plan() -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register gains the parcel area.")
        .with(MigrationStep::AddField {
            field: FieldName::new("area"),
            ty: TypeExpr::Integer,
            default: Value::Integer(0),
            summary: "The area of the parcel in square metres.".to_string(),
        })
        .within(100)
}

#[test]
fn a_proposed_plan_waits_for_the_apply_call() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_area_plan().encode())
        ));
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        let entry = Pallet::<Test>::migration_entry(&DEPLOYMENT, &id, 1)
            .expect("the registry holds the plan");
        assert_eq!(entry.state, MigrationState::Proposed);
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(1)
        );
    });
}

#[test]
fn an_applied_plan_raises_the_version_and_the_record_type() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_area_plan().encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name,
            1
        ));
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(2)
        );
        let found = <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("parcel"))
            .expect("the registry holds the type");
        match found {
            TypeDef::Record(record) => {
                assert_eq!(record.fields.len(), 3);
                assert_eq!(record.fields[2].name, FieldName::new("area"));
            }
            _ => unreachable!("the registry holds a record type"),
        }
    });
}

#[test]
fn an_applied_plan_runs_once() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_area_plan().encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name.clone(),
            1
        ));
        assert_noop!(
            Schema::apply_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, name, 1),
            Error::<Test>::MigrationAlreadyApplied
        );
    });
}

#[test]
fn a_revert_inside_the_window_restores_the_earlier_version() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_area_plan().encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name.clone(),
            1
        ));
        System::set_block_number(20);
        assert_ok!(Schema::revert_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name,
            1
        ));
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(1)
        );
        let found = <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("parcel"))
            .expect("the registry holds the type");
        assert_eq!(found, parcel_type());
    });
}

#[test]
fn a_revert_past_the_window_is_declined() {
    new_test_ext().execute_with(|| {
        register_parcel();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_area_plan().encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name.clone(),
            1
        ));
        System::set_block_number(500);
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, name, 1),
            Error::<Test>::RevertWindowClosed
        );
    });
}

#[test]
fn a_plan_removing_a_field_carries_no_reverse() {
    new_test_ext().execute_with(|| {
        register_parcel();
        let plan = MigrationPlan::new("parcel", 1, "The register drops the holder field.").with(
            MigrationStep::RemoveField {
                field: FieldName::new("holder"),
            },
        );
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(plan.encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name.clone(),
            1
        ));
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, name, 1),
            Error::<Test>::MigrationNotReversible
        );
    });
}

#[test]
fn a_retired_collection_declines_a_further_registration() {
    new_test_ext().execute_with(|| {
        register_parcel();
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::retire_collection(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name
        ));
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        assert!(<Pallet<Test> as SchemaRegistry>::is_retired(
            &DEPLOYMENT,
            &id
        ));
        assert_noop!(
            Schema::register_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(parcel_collection().encode())
            ),
            Error::<Test>::CollectionRetired
        );
    });
}

#[test]
fn an_index_step_changes_the_collection_definition() {
    new_test_ext().execute_with(|| {
        register_parcel();
        let plan = MigrationPlan::new("parcel", 1, "The register gains an index over the holder.")
            .with(MigrationStep::AddIndex {
                index: Index {
                    name: IndexName::new("by_holder"),
                    fields: vec![FieldPath::parse("holder")],
                    unique: false,
                },
            })
            .within(0);
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(plan.encode())
        ));
        let name = name_of("parcel").expect("the name fits");
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name,
            1
        ));
        let id = cerulea_blueprint::types::CollectionId::new("parcel");
        let found = <Pallet<Test> as SchemaRegistry>::collection(&DEPLOYMENT, &id)
            .expect("the registry holds the collection");
        assert_eq!(found.indexes.len(), 1);
        assert_eq!(found.indexes[0].name, IndexName::new("by_holder"));
    });
}

#[test]
fn a_record_rewrite_adds_renames_and_retypes() {
    let plan = MigrationPlan::new("parcel", 1, "The register reshapes the parcel record.")
        .with(MigrationStep::AddField {
            field: FieldName::new("area"),
            ty: TypeExpr::Integer,
            default: Value::Integer(0),
            summary: "The area in square metres.".to_string(),
        })
        .with(MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .with(MigrationStep::RetypeField {
            field: FieldName::new("area"),
            ty: TypeExpr::Integer,
            convert: Expr::binary(
                BinaryOp::Add,
                Expr::bound(crate::migrate::OLD_VALUE_BINDING),
                Expr::integer(5),
            ),
        });

    let env = Env::new(Identity([0u8; 32]), Timestamp::from_millis(0));
    let mut meter = Meter::new(10_000);
    let produced = migrate_record(
        &plan,
        &parcel_record("12/3", "Anitha"),
        &env,
        &mut meter,
        &Path::root().field("record"),
    )
    .expect("the rewrite runs");

    match produced {
        Value::Record(record) => {
            assert_eq!(
                record.field(&FieldName::new("owner")),
                Some(&Value::Text("Anitha".to_string()))
            );
            assert_eq!(
                record.field(&FieldName::new("area")),
                Some(&Value::Integer(5))
            );
            assert!(record.field(&FieldName::new("holder")).is_none());
        }
        other => unreachable!("the rewrite produced {:?}", other),
    }
}

#[test]
fn an_inverse_plan_swaps_every_reversible_step() {
    let plan = add_area_plan();
    let inverse = inverse_plan(
        &plan,
        &parcel_collection(),
        &Path::root().field("migration"),
    )
    .expect("the plan reverses");
    assert_eq!(inverse.from_version, 2);
    assert_eq!(inverse.to_version, 1);
    assert_eq!(inverse.steps.len(), 1);
    assert_eq!(
        inverse.steps[0],
        MigrationStep::RemoveField {
            field: FieldName::new("area")
        }
    );
}

#[test]
fn a_blueprint_installs_every_type_and_collection() {
    new_test_ext().execute_with(|| {
        let mut blueprint = Blueprint::new(BlueprintMeta::new(
            "district land registry",
            "The registry records land parcels and their holders.",
            "Kollam District Administration",
        ));
        blueprint.types.push(parcel_type());
        blueprint.collections.push(parcel_collection());
        assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
            &DEPLOYMENT,
            &blueprint
        ));
        let snapshot = SchemaSnapshot::load::<Pallet<Test>>(&DEPLOYMENT);
        assert_eq!(snapshot.types.len(), 1);
        assert_eq!(snapshot.collections.len(), 1);
        let types = snapshot.type_registry();
        assert!(types.record(&TypeId::new("parcel")).is_some());
        let collections = snapshot.collection_registry();
        assert!(collections.contains(&cerulea_blueprint::types::CollectionId::new("parcel")));
    });
}

#[test]
fn a_collection_key_carries_the_deployment_and_the_collection() {
    let key = CollectionKey::from_text(DEPLOYMENT, "parcel").expect("the name fits");
    assert_eq!(key.deployment, DEPLOYMENT);
    assert_eq!(key.name_text(), "parcel");
    let name = name_of("parcel").expect("the name fits");
    assert_eq!(key.name, name);
}

#[test]
fn the_stored_entry_carries_the_registration_block() {
    new_test_ext().execute_with(|| {
        register_parcel();
        let name = name_of("parcel").expect("the name fits");
        let entry =
            CollectionDefs::<Test>::get(DEPLOYMENT, &name).expect("the registry holds the entry");
        assert_eq!(entry.registered_at, 1);
        assert!(!entry.retired);
    });
}
