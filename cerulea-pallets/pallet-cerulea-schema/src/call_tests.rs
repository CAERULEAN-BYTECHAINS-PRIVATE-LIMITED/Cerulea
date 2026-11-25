//! The tests of every declining branch of the schema module calls.
//!
//! Each error carries a plain fix in its documentation. A test here drives
//! the call into the branch and checks the error the call returns, and the
//! registry the branch leaves behind.

use crate::{
    migrate::{MigrationPlan, MigrationState, MigrationStep},
    mock::*,
    primitives::{name_of, DeploymentId},
    text_of, Error, Event, MigrationHook, Pallet, SchemaRegistrar, SchemaRegistry,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::Expr,
    types::{CollectionId, FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
    Blueprint, BlueprintMeta,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};

const DEPLOYMENT: DeploymentId = [8u8; 32];
const STEWARD: u64 = 1;
const OTHER: u64 = 2;

fn deed_type(extra: bool) -> TypeDef {
    let mut fields = vec![
        FieldDef::new("number", TypeExpr::Text, "The deed number."),
        FieldDef::new("holder", TypeExpr::Text, "The holder of the deed."),
    ];
    if extra {
        fields.push(FieldDef::new("area", TypeExpr::Integer, "The area."));
    }
    TypeDef::Record(RecordDef {
        id: TypeId::new("deed"),
        fields,
        summary: "One deed.".to_string(),
    })
}

fn deeds() -> Collection {
    let mut held = Collection::new("deed", "deed", "number", "The register of deeds.");
    held.purpose = PurposeDeclaration::new("The register records deeds.", &["registration"]);
    held
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn register() {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(deed_type(false).encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(deeds().encode())
    ));
}

fn deed_name() -> crate::primitives::BoundedName {
    name_of("deed").expect("the name fits")
}

fn add_area(from: u32) -> MigrationPlan {
    MigrationPlan::new("deed", from, "The register adds the area.").with(MigrationStep::AddField {
        field: FieldName::new("area"),
        ty: TypeExpr::Integer,
        default: Value::Integer(0),
        summary: "The area.".to_string(),
    })
}

fn propose(plan: &MigrationPlan) -> frame_support::pallet_prelude::DispatchResult {
    Schema::propose_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(plan.encode()),
    )
}

fn apply(from: u32) -> frame_support::pallet_prelude::DispatchResult {
    Schema::apply_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        deed_name(),
        from,
    )
}

// Payloads the registry cannot read.

#[test]
fn unreadable_payloads_are_declined_on_every_registration_call() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![0xff, 0xff])
            ),
            Error::<Test>::DefinitionUnreadable
        );
        assert_noop!(
            Schema::register_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![0xff])
            ),
            Error::<Test>::DefinitionUnreadable
        );
        register();
        assert_noop!(
            Schema::propose_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![0xff])
            ),
            Error::<Test>::DefinitionUnreadable
        );
    });
}

#[test]
fn a_type_failing_validation_is_declined_and_the_check_names_the_fix() {
    new_test_ext().execute_with(|| {
        let empty = TypeDef::Record(RecordDef {
            id: TypeId::new("deed"),
            fields: Vec::new(),
            summary: "One deed.".to_string(),
        });
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(empty.encode())
            ),
            Error::<Test>::DefinitionInvalid
        );
        let report = Pallet::<Test>::check_type(&DEPLOYMENT, &empty);
        assert!(!report.is_clean());
        assert!(report
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.fix.is_empty()));
    });
}

// Stewardship.

#[test]
fn a_call_on_a_deployment_without_a_steward_is_declined() {
    new_test_ext().execute_with(|| {
        assert_noop!(propose(&add_area(1)), Error::<Test>::NotSteward);
        assert_noop!(apply(1), Error::<Test>::NotSteward);
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name(), 1),
            Error::<Test>::NotSteward
        );
        assert_noop!(
            Schema::retire_collection(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name()),
            Error::<Test>::NotSteward
        );
    });
}

#[test]
fn the_first_registration_names_the_steward_and_another_account_is_declined() {
    new_test_ext().execute_with(|| {
        register();
        assert!(System::events().iter().any(|record| record.event
            == RuntimeEvent::Schema(Event::StewardSet {
                deployment: DEPLOYMENT,
                steward: STEWARD,
            })));
        assert_noop!(
            Schema::retire_collection(RuntimeOrigin::signed(OTHER), DEPLOYMENT, deed_name()),
            Error::<Test>::NotSteward
        );
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(OTHER),
                DEPLOYMENT,
                bounded(deed_type(false).encode())
            ),
            Error::<Test>::NotSteward
        );
    });
}

// Collections the registry lacks or retired.

#[test]
fn a_plan_or_a_retirement_over_an_absent_collection_is_declined() {
    new_test_ext().execute_with(|| {
        register();
        let plan = MigrationPlan::new("licence", 1, "The register adds a field.").with(
            MigrationStep::RemoveField {
                field: FieldName::new("holder"),
            },
        );
        assert_noop!(propose(&plan), Error::<Test>::CollectionUnknown);
        assert_noop!(
            Schema::retire_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name_of("licence").expect("the name fits")
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_retired_collection_declines_plans_applies_and_a_second_retirement() {
    new_test_ext().execute_with(|| {
        register();
        assert_ok!(propose(&add_area(1)));
        assert_ok!(Schema::retire_collection(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            deed_name()
        ));
        assert!(<Pallet<Test> as SchemaRegistry>::is_retired(
            &DEPLOYMENT,
            &CollectionId::new("deed")
        ));
        assert_noop!(propose(&add_area(1)), Error::<Test>::CollectionRetired);
        assert_noop!(apply(1), Error::<Test>::CollectionRetired);
        assert_noop!(
            Schema::retire_collection(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name()),
            Error::<Test>::CollectionRetired
        );
    });
}

#[test]
fn a_plan_failing_its_check_is_declined_at_the_proposal() {
    new_test_ext().execute_with(|| {
        register();
        let plan = MigrationPlan::new("deed", 1, "The register removes an absent field.").with(
            MigrationStep::RemoveField {
                field: FieldName::new("district"),
            },
        );
        assert_noop!(propose(&plan), Error::<Test>::DefinitionInvalid);
    });
}

// Migrations the registry lacks or holds in another state.

#[test]
fn an_apply_or_a_revert_of_an_absent_plan_is_declined() {
    new_test_ext().execute_with(|| {
        register();
        assert_noop!(apply(1), Error::<Test>::MigrationUnknown);
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name(), 1),
            Error::<Test>::MigrationUnknown
        );
    });
}

#[test]
fn a_revert_of_a_plan_that_has_yet_to_run_is_declined() {
    new_test_ext().execute_with(|| {
        register();
        assert_ok!(propose(&add_area(1)));
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name(), 1),
            Error::<Test>::MigrationNotApplied
        );
    });
}

#[test]
fn a_stored_plan_that_stopped_matching_the_registry_is_declined_at_the_apply() {
    new_test_ext().execute_with(|| {
        register();
        assert_ok!(propose(&add_area(1)));
        // A Blueprint installation moves the collection to version two, so
        // the stored plan starting from version one no longer fits.
        let mut changed = deeds();
        changed.summary = "The register of deeds, kept by the district.".to_string();
        assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
            &DEPLOYMENT,
            &blueprint_with(changed)
        ));
        assert_noop!(apply(1), Error::<Test>::DefinitionInvalid);
    });
}

#[test]
fn a_collection_past_its_version_bound_declines_a_further_apply() {
    new_test_ext().execute_with(|| {
        register();
        // The mock keeps thirty two versions per collection.
        for from in 1..32u32 {
            let plan = MigrationPlan::new("deed", from, "The register adds one field.").with(
                MigrationStep::AddField {
                    field: FieldName::new(&format!("field_{from}")),
                    ty: TypeExpr::Integer,
                    default: Value::Integer(0),
                    summary: "One more field.".to_string(),
                },
            );
            assert_ok!(propose(&plan));
            assert_ok!(apply(from));
        }
        let last = MigrationPlan::new("deed", 32, "The register adds one field too many.").with(
            MigrationStep::AddField {
                field: FieldName::new("field_32"),
                ty: TypeExpr::Integer,
                default: Value::Integer(0),
                summary: "One more field.".to_string(),
            },
        );
        assert_ok!(propose(&last));
        assert_noop!(apply(32), Error::<Test>::VersionLimitReached);
    });
}

// The applied state and its records.

#[test]
fn an_applied_plan_records_its_state_block_and_count() {
    new_test_ext().execute_with(|| {
        register();
        assert_ok!(propose(&add_area(1).within(20)));
        let id = CollectionId::new("deed");
        let proposed = Pallet::<Test>::migration_entry(&DEPLOYMENT, &id, 1)
            .expect("the registry holds the plan");
        assert_eq!(proposed.state, MigrationState::Proposed);
        assert_eq!(proposed.proposer, STEWARD);
        assert_eq!(proposed.applied_at, None);

        System::set_block_number(5);
        assert_ok!(apply(1));
        let applied = Pallet::<Test>::migration_entry(&DEPLOYMENT, &id, 1)
            .expect("the registry holds the plan");
        assert_eq!(applied.state, MigrationState::Applied);
        assert_eq!(applied.applied_at, Some(5));
        assert_eq!(applied.records_rewritten, 0);
        assert_eq!(
            Pallet::<Test>::collection_at(&DEPLOYMENT, &id, 1),
            Some(deeds())
        );
        assert!(Pallet::<Test>::collection_at(&DEPLOYMENT, &id, 2).is_some());
        assert!(Pallet::<Test>::collection_at(&DEPLOYMENT, &id, 3).is_none());
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("deed")),
            Some(deed_type(true))
        );

        System::set_block_number(9);
        assert_ok!(Schema::revert_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            deed_name(),
            1
        ));
        let reverted = Pallet::<Test>::migration_entry(&DEPLOYMENT, &id, 1)
            .expect("the registry holds the plan");
        assert_eq!(reverted.state, MigrationState::Reverted);
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("deed")),
            Some(deed_type(false))
        );
        assert!(System::events().iter().any(|record| record.event
            == RuntimeEvent::Schema(Event::MigrationReverted {
                deployment: DEPLOYMENT,
                name: deed_name(),
                version: 1,
                records: 0,
            })));
        assert_noop!(
            Schema::revert_migration(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, deed_name(), 1),
            Error::<Test>::MigrationNotApplied
        );
    });
}

// The read side of the registry.

#[test]
fn the_registry_lists_its_names_and_reports_installation() {
    new_test_ext().execute_with(|| {
        assert!(!<Pallet<Test> as SchemaRegistry>::is_installed(&DEPLOYMENT));
        assert!(<Pallet<Test> as SchemaRegistry>::collections(&DEPLOYMENT).is_empty());
        assert!(!<Pallet<Test> as SchemaRegistry>::is_retired(
            &DEPLOYMENT,
            &CollectionId::new("deed")
        ));
        register();
        assert!(<Pallet<Test> as SchemaRegistry>::is_installed(&DEPLOYMENT));
        assert_eq!(Pallet::<Test>::type_names(&DEPLOYMENT), vec![deed_name()]);
        assert_eq!(
            Pallet::<Test>::collection_names(&DEPLOYMENT),
            vec![deed_name()]
        );
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collections(&DEPLOYMENT),
            vec![deeds()]
        );
        assert_eq!(text_of(&deed_name()), "deed");
        assert!(
            <Pallet<Test> as SchemaRegistry>::type_def(&DEPLOYMENT, &TypeId::new("seal")).is_none()
        );
        let long = CollectionId::new(&"d".repeat(200));
        assert!(Pallet::<Test>::collection_entry(&DEPLOYMENT, &long).is_none());
        assert!(Pallet::<Test>::collection_at(&DEPLOYMENT, &long, 1).is_none());
        assert!(Pallet::<Test>::migration_entry(&DEPLOYMENT, &long, 1).is_none());
        assert!(<Pallet<Test> as SchemaRegistry>::type_def(
            &DEPLOYMENT,
            &TypeId::new(&"t".repeat(200))
        )
        .is_none());
    });
}

#[test]
fn a_preview_over_an_absent_collection_names_the_registration_fix() {
    new_test_ext().execute_with(|| {
        let report = Pallet::<Test>::preview_migration(&DEPLOYMENT, &add_area(1))
            .expect_err("the registry lacks the collection");
        assert_eq!(report.len(), 1);
        assert_eq!(
            report.diagnostics[0].fix,
            "Register the collection before planning a migration."
        );
        assert_eq!(
            report.diagnostics[0].location.as_str(),
            "migration.collection"
        );
    });
}

// The value rewrite the record store calls.

#[test]
fn a_value_rewrite_applies_the_plan_or_reports_the_spent_budget() {
    let record = Value::record(
        TypeId::new("deed"),
        vec![
            (FieldName::new("number"), Value::Text("D-1".to_string())),
            (FieldName::new("holder"), Value::Text("Anitha".to_string())),
        ],
    );
    let produced = Pallet::<Test>::migrate_value(&add_area(1), &record).expect("the rewrite runs");
    match produced {
        Value::Record(held) => {
            assert_eq!(
                held.field(&FieldName::new("area")),
                Some(&Value::Integer(0))
            )
        }
        other => panic!("the rewrite yields a record, and this reads {other:?}"),
    }

    // A membership test over a list longer than the step budget of the
    // mock charges one step per element and runs past the budget.
    let long = Value::list(
        TypeExpr::Integer,
        (0..100_001).map(Value::Integer).collect(),
    );
    let plan = MigrationPlan::new("deed", 1, "The register marks listed holders.").with(
        MigrationStep::RetypeField {
            field: FieldName::new("holder"),
            ty: TypeExpr::Bool,
            convert: Expr::Contains {
                list: Box::new(Expr::Literal(long)),
                item: Box::new(Expr::integer(1)),
            },
        },
    );
    assert_eq!(
        Pallet::<Test>::migrate_value(&plan, &record),
        Err(Error::<Test>::MigrationBudgetSpent.into())
    );
    let (env, meter) = Pallet::<Test>::migration_env();
    assert_eq!(meter.limit(), 100_000);
    assert_eq!(env.now().millis(), 0);
}

#[test]
fn the_empty_record_store_rewrites_nothing() {
    let collection = deeds();
    let produced = match deed_type(true) {
        TypeDef::Record(held) => held,
        _ => unreachable!("the deed type is a record"),
    };
    assert_eq!(
        <() as MigrationHook>::migrate_records(&DEPLOYMENT, &add_area(1), &produced, &collection),
        Ok(0)
    );
    assert_eq!(
        <() as MigrationHook>::revert_records(&DEPLOYMENT, &add_area(1), &produced, &collection),
        Ok(0)
    );
}

// Blueprint installation.

fn blueprint_with(collection: Collection) -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "deed registry",
        "The registry records deeds.",
        "Kollam District Administration",
    ));
    blueprint.types.push(deed_type(false));
    blueprint.collections.push(collection);
    blueprint
}

#[test]
fn a_repeated_blueprint_keeps_the_version_and_a_changed_one_raises_it() {
    new_test_ext().execute_with(|| {
        let id = CollectionId::new("deed");
        assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
            &DEPLOYMENT,
            &blueprint_with(deeds())
        ));
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(1)
        );
        assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
            &DEPLOYMENT,
            &blueprint_with(deeds())
        ));
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(1)
        );
        let mut changed = deeds();
        changed.summary = "The register of deeds, kept by the district.".to_string();
        assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
            &DEPLOYMENT,
            &blueprint_with(changed.clone())
        ));
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(2)
        );
        assert_eq!(
            Pallet::<Test>::collection_at(&DEPLOYMENT, &id, 2),
            Some(changed)
        );
    });
}

#[test]
fn a_blueprint_past_the_version_bound_is_declined() {
    new_test_ext().execute_with(|| {
        for round in 0..32u32 {
            let mut changed = deeds();
            changed.summary = format!("The register of deeds, revision {round}.");
            assert_ok!(<Pallet<Test> as SchemaRegistrar>::register_blueprint(
                &DEPLOYMENT,
                &blueprint_with(changed)
            ));
        }
        let mut changed = deeds();
        changed.summary = "The register of deeds, one revision too many.".to_string();
        assert_eq!(
            <Pallet<Test> as SchemaRegistrar>::register_blueprint(
                &DEPLOYMENT,
                &blueprint_with(changed)
            ),
            Err(Error::<Test>::VersionLimitReached.into())
        );
    });
}

// The bounds on the registry.

fn numbered_type(number: u32) -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new(&format!("kind_{number:02}")),
        fields: vec![FieldDef::new("number", TypeExpr::Text, "The number.")],
        summary: "One numbered kind.".to_string(),
    })
}

fn numbered_collection(number: u32) -> Collection {
    let mut held = Collection::new(
        &format!("shelf_{number:02}"),
        "deed",
        "number",
        "One numbered shelf of deeds.",
    );
    held.purpose = PurposeDeclaration::new("The shelf records deeds.", &["registration"]);
    held
}

#[test]
fn a_repeated_collection_registration_keeps_version_one() {
    new_test_ext().execute_with(|| {
        register();
        System::reset_events();
        assert_ok!(Schema::register_collection(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(deeds().encode())
        ));
        assert_eq!(
            <Pallet<Test> as SchemaRegistry>::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("deed")
            ),
            Some(1)
        );
        assert!(System::events().iter().any(|record| record.event
            == RuntimeEvent::Schema(Event::CollectionRegistered {
                deployment: DEPLOYMENT,
                name: deed_name(),
                version: 1,
            })));
    });
}

#[test]
fn a_deployment_holding_its_full_count_of_types_declines_one_more() {
    new_test_ext().execute_with(|| {
        // The mock admits sixty four types per deployment.
        for number in 0..64 {
            assert_ok!(Schema::register_type(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(numbered_type(number).encode())
            ));
        }
        assert_noop!(
            Schema::register_type(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(numbered_type(64).encode())
            ),
            Error::<Test>::TypeLimitReached
        );
        let mut blueprint = Blueprint::new(BlueprintMeta::new(
            "kind registry",
            "The registry records kinds.",
            "Kollam District Administration",
        ));
        blueprint.types.push(numbered_type(65));
        assert_eq!(
            <Pallet<Test> as SchemaRegistrar>::register_blueprint(&DEPLOYMENT, &blueprint),
            Err(Error::<Test>::TypeLimitReached.into())
        );
    });
}

#[test]
fn a_deployment_holding_its_full_count_of_collections_declines_one_more() {
    new_test_ext().execute_with(|| {
        register();
        // The mock admits sixty four collections per deployment, and the
        // deed collection already takes one place.
        for number in 0..63 {
            assert_ok!(Schema::register_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(numbered_collection(number).encode())
            ));
        }
        assert_noop!(
            Schema::register_collection(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(numbered_collection(63).encode())
            ),
            Error::<Test>::CollectionLimitReached
        );
        let mut blueprint = blueprint_with(numbered_collection(64));
        blueprint.types.clear();
        assert_eq!(
            <Pallet<Test> as SchemaRegistrar>::register_blueprint(&DEPLOYMENT, &blueprint),
            Err(Error::<Test>::CollectionLimitReached.into())
        );
    });
}
