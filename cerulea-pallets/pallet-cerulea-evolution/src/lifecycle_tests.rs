//! The tests of every branch of the version transition.
//!
//! Every test drives a real registry and a real record store. The land
//! parcel register carries the transition. Each test reads the stored plan,
//! the events, the registry and the audit log after the calls.

use crate::{
    bridge::is_applied, mock::*, plan_breaks, plan_hash, step_breaks, ActiveVersion, Error, Event,
    NextPlan, PlanStatus, Plans, SchemaBridge, SchemaMigrations,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::Expr,
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, RecordDef, TypeDef, TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationStep},
    primitives::{name_of, DeploymentId, RecordKey},
};
use pallet_cerulea_state::RecordStore;
use sp_runtime::DispatchError;

extern crate alloc;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const DAY: u64 = 86_400_000;

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

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
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
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

fn parcel(survey: &str, holder: &str, area: i128) -> Value {
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

fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.to_string());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

fn add_field_plan(field: &str, default: &str) -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register adds one field to each parcel.").with(
        MigrationStep::AddField {
            field: FieldName::new(field),
            ty: TypeExpr::Text,
            default: Value::Text(default.to_string()),
            summary: "One further detail of the parcel.".to_string(),
        },
    )
}

fn add_ward_plan() -> MigrationPlan {
    add_field_plan("ward", "unassigned")
}

fn install() {
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
        bounded(parcel("12/3", "Anitha", 400).canonical_bytes())
    ));
}

fn plan(body: &MigrationPlan, window: u64) -> u64 {
    let id = NextPlan::<Test>::get(DEPLOYMENT);
    assert_ok!(Evolution::plan_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(body.encode()),
        window
    ));
    id
}

fn dry_run(id: u64) {
    assert_ok!(Evolution::dry_run(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        id
    ));
}

fn apply(id: u64) {
    assert_ok!(Evolution::apply(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        id
    ));
}

fn applied(body: &MigrationPlan) -> u64 {
    let id = plan(body, DAY);
    dry_run(id);
    apply(id);
    id
}

fn status(id: u64) -> PlanStatus {
    Plans::<Test>::get(DEPLOYMENT, id)
        .expect("the registry holds the plan")
        .status
}

fn live_version() -> Option<u32> {
    <SchemaBridge<Test> as SchemaMigrations>::collection_version(
        &DEPLOYMENT,
        &CollectionId::new("parcel"),
    )
}

fn stored_field(name: &str) -> Option<Value> {
    let record = State::read(
        &DEPLOYMENT,
        &CollectionId::new("parcel"),
        &record_key("12/3"),
    )
    .expect("the store holds the record");
    pallet_cerulea_state::value_at(&record, &FieldPath::parse(name))
}

fn evolution_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Evolution(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// The fix sentence one error carries in the runtime metadata.
fn error_text(name: &str) -> String {
    use scale_info::{TypeDef as Shape, TypeInfo};
    let info = <Error<Test> as TypeInfo>::type_info();
    let variants = match info.type_def {
        Shape::Variant(found) => found.variants,
        _ => panic!("the error type is an enumeration"),
    };
    let variant = variants
        .into_iter()
        .find(|held| held.name == name)
        .expect("the error type names the variant");
    variant
        .docs
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn every_step_that_changes_an_existing_field_breaks_an_integration() {
    let rename = MigrationStep::RenameField {
        from: FieldName::new("holder"),
        to: FieldName::new("owner"),
    };
    let retype = MigrationStep::RetypeField {
        field: FieldName::new("area"),
        ty: TypeExpr::Text,
        convert: Expr::Literal(Value::Text("unknown".to_string())),
    };
    let remove = MigrationStep::RemoveField {
        field: FieldName::new("holder"),
    };
    assert!(step_breaks(&rename));
    assert!(step_breaks(&retype));
    assert!(step_breaks(&remove));
    assert!(!plan_breaks(&add_ward_plan()));
    assert!(plan_breaks(&add_ward_plan().with(rename)));
    assert_ne!(
        plan_hash(&add_ward_plan().encode()),
        plan_hash(&add_field_plan("district", "unassigned").encode())
    );
}

#[test]
fn a_lower_version_records_and_leaves_the_active_version_in_place() {
    new_test_ext().execute_with(|| {
        for (version, blueprint) in [(3u32, [3u8; 32]), (2, [2u8; 32])] {
            assert_ok!(Evolution::record_version(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                version,
                blueprint,
                version,
                bounded(b"One published Blueprint.".to_vec())
            ));
        }
        assert_eq!(ActiveVersion::<Test>::get(DEPLOYMENT), 3);
        assert!(evolution_events().contains(&Event::VersionRecorded {
            deployment: DEPLOYMENT,
            version: 2,
            blueprint: [2u8; 32]
        }));
        let recorded = Audit::query_by_event(&DEPLOYMENT, "version_recorded", 0, 64);
        assert_eq!(recorded.len(), 2);
        assert_eq!(recorded[0].class, WorkClass::Structural);
        assert_eq!(
            recorded[0].reason,
            "The steward recorded one version of the deployment."
        );
        assert_eq!(
            error_text("VersionPresent"),
            "The registry already holds that version. Record the next version number."
        );
    });
}

#[test]
fn a_plan_that_fails_to_decode_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(Vec::new()),
                DAY
            ),
            Error::<Test>::PlanUnreadable
        );
        assert_eq!(
            error_text("PlanUnreadable"),
            "The plan failed to decode. Submit the canonical encoding the Studio produces."
        );
    });
}

#[test]
fn a_plan_naming_a_collection_past_the_name_limit_declines() {
    new_test_ext().execute_with(|| {
        install();
        let long = "p".repeat(200);
        let body = MigrationPlan::new(&long, 1, "The register adds one field.");
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(body.encode()),
                DAY
            ),
            Error::<Test>::NameTooLong
        );
        assert_eq!(
            error_text("NameTooLong"),
            "The name runs past 128 bytes. Shorten the collection name."
        );
    });
}

#[test]
fn a_full_plan_register_declines_a_further_plan() {
    new_test_ext().execute_with(|| {
        install();
        NextPlan::<Test>::insert(DEPLOYMENT, u64::MAX);
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(add_ward_plan().encode()),
                DAY
            ),
            Error::<Test>::RegisterFull
        );
    });
}

#[test]
fn a_stored_plan_carries_its_hash_and_emits_its_event() {
    new_test_ext().execute_with(|| {
        install();
        let body = add_ward_plan();
        let id = plan(&body, DAY);
        let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the registry holds the plan");
        assert_eq!(held.plan_hash, plan_hash(&body.encode()));
        assert_eq!(held.from_version, 1);
        assert_eq!(held.to_version, 2);
        assert_eq!(held.planner, STEWARD);
        assert_eq!(held.planned_at, START);
        assert_eq!(held.status, PlanStatus::Planned);
        assert!(evolution_events().contains(&Event::MigrationPlanned {
            deployment: DEPLOYMENT,
            plan: id,
            collection: name_of("parcel").expect("the name fits"),
            breaking: false
        }));
        let planned = Audit::query_by_event(&DEPLOYMENT, "migration_planned", 0, 64);
        assert_eq!(
            planned[0].reason,
            "The steward planned one migration of a collection."
        );
        assert_eq!(
            Evolution::plan_collection(&DEPLOYMENT, id).expect("the plan names a collection"),
            CollectionId::new("parcel")
        );
        assert_eq!(
            Evolution::plan_body(&DEPLOYMENT, id).expect("the plan decodes"),
            body
        );
    });
}

#[test]
fn a_dry_run_of_a_plan_the_registry_declines_fails() {
    new_test_ext().execute_with(|| {
        install();
        // The record type carries no field named nickname.
        let body = MigrationPlan::new("parcel", 1, "The register drops the nickname.").with(
            MigrationStep::RemoveField {
                field: FieldName::new("nickname"),
            },
        );
        let id = plan(&body, DAY);
        assert_noop!(
            Evolution::dry_run(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::DryRunFailed
        );
        assert_eq!(status(id), PlanStatus::Planned);
        assert_eq!(
            error_text("DryRunFailed"),
            "The dry run found a problem in the plan. Correct the plan in the Studio and plan the migration again."
        );
    });
}

#[test]
fn a_dry_run_repeats_and_records_its_event() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        dry_run(id);
        dry_run(id);
        assert_eq!(status(id), PlanStatus::Checked);
        assert!(evolution_events().contains(&Event::DryRunPassed {
            deployment: DEPLOYMENT,
            plan: id,
            reversible: true
        }));
        let runs = Audit::query_by_event(&DEPLOYMENT, "migration_dry_run", 0, 64);
        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs[0].reason,
            "The steward ran one migration plan without changing anything."
        );
    });
}

#[test]
fn an_applied_plan_takes_no_further_dry_run_or_apply() {
    new_test_ext().execute_with(|| {
        install();
        let id = applied(&add_ward_plan());
        assert_noop!(
            Evolution::dry_run(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::PlanClosed
        );
        assert_noop!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::DryRunPending
        );
        assert_eq!(
            error_text("PlanClosed"),
            "The plan already ran. Plan a new migration for a further change."
        );
        assert_eq!(
            error_text("DryRunPending"),
            "The plan waits for its dry run. Run the dry run before the apply call."
        );
    });
}

#[test]
fn an_apply_stamps_the_plan_and_emits_its_event() {
    new_test_ext().execute_with(|| {
        install();
        System::set_block_number(4);
        let id = applied(&add_ward_plan());
        let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the registry holds the plan");
        assert_eq!(held.applied_at, Some(START));
        assert_eq!(held.applied_block, Some(4));
        assert_eq!(held.records_rewritten, 1);
        assert!(evolution_events().contains(&Event::MigrationApplied {
            deployment: DEPLOYMENT,
            plan: id,
            to_version: 2,
            records: 1
        }));
        let log = Evolution::applied_log(&DEPLOYMENT);
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].plan, id);
        assert_eq!(log[0].block, 4);
        assert_eq!(log[0].at_millis, START);
        assert_eq!(log[0].collection, name_of("parcel").expect("the name fits"));
        let entries = Audit::query_by_event(&DEPLOYMENT, "migration_applied", 0, 64);
        assert_eq!(
            entries[0].reason,
            "The steward applied one migration to a collection."
        );
    });
}

#[test]
fn a_verify_of_an_unapplied_or_unknown_plan_declines() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        dry_run(id);
        assert_noop!(
            Evolution::verify(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::PlanClosed
        );
        assert_noop!(
            Evolution::verify(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 9),
            Error::<Test>::PlanUnknown
        );
        assert_noop!(
            Evolution::roll_back(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 9),
            Error::<Test>::PlanUnknown
        );
        assert_eq!(
            error_text("PlanUnknown"),
            "The registry holds no plan under that identifier. Plan the migration before running it."
        );
    });
}

#[test]
fn a_verify_after_the_registry_moved_the_collection_back_fails() {
    new_test_ext().execute_with(|| {
        install();
        let id = applied(&add_ward_plan());
        // The registry reverts the migration outside the evolution module,
        // so the live version returns to one while the plan reads applied.
        assert_ok!(Schema::revert_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
            1
        ));
        assert_eq!(live_version(), Some(1));
        assert_noop!(
            Evolution::verify(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::VerificationFailed
        );
        assert_eq!(status(id), PlanStatus::Applied);
        assert_eq!(
            error_text("VerificationFailed"),
            "The registry reports a shape other than the one the plan produced. Roll the migration back inside its window."
        );
    });
}

#[test]
fn a_verify_records_its_event_and_its_reason() {
    new_test_ext().execute_with(|| {
        install();
        let id = applied(&add_ward_plan());
        assert_ok!(Evolution::verify(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(evolution_events().contains(&Event::MigrationVerified {
            deployment: DEPLOYMENT,
            plan: id
        }));
        let verified = Audit::query_by_event(&DEPLOYMENT, "migration_verified", 0, 64);
        assert_eq!(
            verified[0].reason,
            "The steward confirmed the shape one migration produced."
        );
        // A verified plan still rolls back inside its window.
        assert!(Evolution::rollback_available(&DEPLOYMENT, id));
    });
}

#[test]
fn a_rollback_of_an_unapplied_plan_declines() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert!(!Evolution::rollback_available(&DEPLOYMENT, id));
        assert!(!Evolution::rollback_available(&DEPLOYMENT, 9));
        assert_noop!(
            Evolution::roll_back(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::PlanClosed
        );
        assert_eq!(
            error_text("WindowClosed"),
            "The rollback window closed. Plan a further migration to change the shape again."
        );
        assert_eq!(
            error_text("Irreversible"),
            "The plan carries no reverse. Plan a further migration to change the shape again."
        );
    });
}

#[test]
fn a_rollback_records_its_event_and_its_reason() {
    new_test_ext().execute_with(|| {
        install();
        let id = applied(&add_ward_plan());
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(evolution_events().contains(&Event::MigrationRolledBack {
            deployment: DEPLOYMENT,
            plan: id,
            records: 1
        }));
        let undone = Audit::query_by_event(&DEPLOYMENT, "migration_rolled_back", 0, 64);
        assert_eq!(
            undone[0].reason,
            "The steward undid one migration inside its rollback window."
        );
        assert!(!Evolution::rollback_available(&DEPLOYMENT, id));
    });
}

#[test]
fn a_stored_plan_that_fails_to_decode_declines_the_dry_run() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        Plans::<Test>::mutate(DEPLOYMENT, id, |slot| {
            if let Some(held) = slot.as_mut() {
                held.plan = BoundedVec::default();
            }
        });
        assert_noop!(
            Evolution::dry_run(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::PlanUnreadable
        );
        assert_eq!(
            Evolution::plan_body(&DEPLOYMENT, 9),
            Err(Error::<Test>::PlanUnknown.into())
        );
        assert_eq!(
            Evolution::plan_collection(&DEPLOYMENT, 9),
            Err(Error::<Test>::PlanUnknown.into())
        );
    });
}

#[test]
fn a_plan_planned_again_after_a_rollback_applies() {
    new_test_ext().execute_with(|| {
        install();
        let first = applied(&add_ward_plan());
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            first
        ));
        assert_eq!(live_version(), Some(1));

        // The rollback error names a further plan as the fix. The further
        // plan starts from the same version and applies.
        let second = applied(&add_ward_plan());
        assert_eq!(status(second), PlanStatus::Applied);
        assert_eq!(live_version(), Some(2));
        assert_eq!(
            stored_field("ward"),
            Some(Value::Text("unassigned".to_string()))
        );
    });
}

#[test]
fn a_plan_the_registry_already_holds_as_proposed_applies_once() {
    new_test_ext().execute_with(|| {
        install();
        let body = add_ward_plan();
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(body.encode())
        ));
        applied(&body);
        assert_eq!(live_version(), Some(2));
        assert!(is_applied::<Test>(&DEPLOYMENT, &body));
    });
}

#[test]
fn the_registry_applies_the_plan_the_evolution_module_ordered() {
    new_test_ext().execute_with(|| {
        install();
        // The registry holds a different proposed plan from the same
        // version. The apply call carries the plan the steward checked.
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(add_field_plan("district", "central").encode())
        ));
        applied(&add_ward_plan());
        assert_eq!(
            stored_field("ward"),
            Some(Value::Text("unassigned".to_string()))
        );
        assert!(stored_field("district").is_none());
    });
}

#[test]
fn the_bridge_reports_whether_the_registry_applied_a_plan() {
    new_test_ext().execute_with(|| {
        install();
        let body = add_ward_plan();
        assert!(!is_applied::<Test>(&DEPLOYMENT, &body));
        let id = applied(&body);
        assert!(is_applied::<Test>(&DEPLOYMENT, &body));
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(!is_applied::<Test>(&DEPLOYMENT, &body));
    });
}

#[test]
fn the_bridge_names_the_problem_of_a_plan_it_declines() {
    new_test_ext().execute_with(|| {
        install();
        let absent = MigrationPlan::new("holder", 1, "The register changes the holder list.");
        assert_eq!(
            <SchemaBridge<Test> as SchemaMigrations>::dry_run(&DEPLOYMENT, &absent),
            Err(DispatchError::Other(
                "The migration plan carries a problem the registry named."
            ))
        );
        let body = add_ward_plan();
        let outcome = <SchemaBridge<Test> as SchemaMigrations>::dry_run(&DEPLOYMENT, &body)
            .expect("the plan applies");
        assert!(outcome.rewrites_records);
        assert!(outcome.reversible);
        assert!(!<SchemaBridge<Test> as SchemaMigrations>::has_collection(
            &DEPLOYMENT,
            &CollectionId::new("holder")
        ));
    });
}

#[test]
fn the_bridge_declines_a_collection_name_past_the_name_limit() {
    new_test_ext().execute_with(|| {
        install();
        let long = "p".repeat(200);
        let body = MigrationPlan::new(&long, 1, "The register adds one field.");
        let named = DispatchError::Other("The collection name runs past 128 bytes.");
        assert_eq!(
            <SchemaBridge<Test> as SchemaMigrations>::apply(&STEWARD, &DEPLOYMENT, &body),
            Err(named)
        );
        assert_eq!(
            <SchemaBridge<Test> as SchemaMigrations>::revert(&STEWARD, &DEPLOYMENT, &body),
            Err(named)
        );
    });
}

#[test]
fn the_bridge_declines_a_plan_past_the_plan_size_limit() {
    new_test_ext().execute_with(|| {
        install();
        let summary = "s".repeat(70_000);
        let body = MigrationPlan::new("parcel", 1, &summary);
        assert_eq!(
            <SchemaBridge<Test> as SchemaMigrations>::apply(&STEWARD, &DEPLOYMENT, &body),
            Err(DispatchError::Other(
                "The migration plan runs past the plan size limit."
            ))
        );
        assert_eq!(live_version(), Some(1));
    });
}

#[test]
fn the_bridge_reverts_no_plan_the_registry_never_applied() {
    new_test_ext().execute_with(|| {
        install();
        assert_eq!(
            <SchemaBridge<Test> as SchemaMigrations>::revert(
                &STEWARD,
                &DEPLOYMENT,
                &add_ward_plan()
            ),
            Err(pallet_cerulea_schema::Error::<Test>::MigrationUnknown.into())
        );
    });
}

#[test]
fn the_platform_version_call_records_the_reason_it_carries() {
    new_test_ext().execute_with(|| {
        assert_ok!(Evolution::set_platform_version(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            5,
            bounded(b"The operator pushed the platform update.".to_vec())
        ));
        assert!(evolution_events().contains(&Event::PlatformVersionSet {
            deployment: DEPLOYMENT,
            version: 5
        }));
        let moved = Audit::query_by_event(&DEPLOYMENT, "platform_version_set", 0, 64);
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].actor, STEWARD);
        assert_eq!(moved[0].class, WorkClass::Structural);
        assert_eq!(moved[0].reason, "The operator pushed the platform update.");
    });
}

#[test]
fn a_breaking_plan_names_the_proposal_it_needs() {
    assert_eq!(
        error_text("ApprovalMissing"),
        "The plan breaks an existing integration and carries no approval. Open a schema change proposal and reach the structural threshold of 80 percent first."
    );
    assert_eq!(
        error_text("WindowTooLong"),
        "The requested window runs past the largest window. Shorten the window, or raise the limit in the runtime."
    );
    assert_eq!(
        error_text("CollectionUnknown"),
        "The registry lacks the collection the plan names. Register the collection before planning a migration."
    );
    assert_eq!(
        error_text("VersionMismatch"),
        "The plan starts from a version the collection left. Plan the migration from the live version of the collection."
    );
}
