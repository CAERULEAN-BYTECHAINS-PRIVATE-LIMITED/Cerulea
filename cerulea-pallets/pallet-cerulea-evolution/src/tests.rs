//! The tests of the evolution module.
//!
//! Every test drives a real registry and a real record store, so a
//! migration rewrites real records. The land parcel register carries the
//! transition. One plan adds a ward field. One plan removes the holder
//! field, which breaks an existing integration.

use crate::{
    mock::*, AppliedCount, Error, PlanStatus, Plans, PlatformVersions, SchemaMigrations, Versions,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, RecordDef, TypeDef, TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_governance::{ProposalKind, ProposalStatus};
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationStep},
    primitives::{name_of, DeploymentId, RecordKey},
};
use pallet_cerulea_state::RecordStore;

extern crate alloc;
use alloc::{string::ToString, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const VALIDATORS: [u64; 4] = [11, 12, 13, 14];
const DAY: u64 = 86_400_000;

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

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.to_string());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

fn add_ward_plan() -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register adds the ward of each parcel.").with(
        MigrationStep::AddField {
            field: FieldName::new("ward"),
            ty: TypeExpr::Text,
            default: Value::Text("unassigned".to_string()),
            summary: "The ward the parcel sits in.".to_string(),
        },
    )
}

fn remove_holder_plan() -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register drops the holder name.").with(
        MigrationStep::RemoveField {
            field: FieldName::new("holder"),
        },
    )
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
    for (survey, holder, area) in [("12/3", "Anitha", 400), ("14/1", "Benny", 250)] {
        assert_ok!(State::insert(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
            bounded(parcel(survey, holder, area).canonical_bytes())
        ));
    }
}

fn plan(body: &MigrationPlan, window: u64) -> u64 {
    let id = crate::NextPlan::<Test>::get(DEPLOYMENT);
    assert_ok!(Evolution::plan_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(body.encode()),
        window
    ));
    id
}

fn seat_the_electorate() {
    for voter in VALIDATORS {
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            voter,
            1
        ));
    }
}

/// Carry one schema change through the electorate at the structural
/// threshold of 80 percent.
fn approve_schema_change(payload: Vec<u8>) {
    seat_the_electorate();
    let id = pallet_cerulea_governance::NextProposal::<Test>::get(DEPLOYMENT);
    assert_ok!(Governance::propose(
        RuntimeOrigin::signed(VALIDATORS[0]),
        DEPLOYMENT,
        ProposalKind::SchemaChange,
        bounded(payload),
        clock() + DAY
    ));
    for voter in VALIDATORS {
        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(voter),
            DEPLOYMENT,
            id,
            true
        ));
    }
    assert_ok!(Governance::tally(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        id
    ));
    assert_eq!(
        Governance::proposal(&DEPLOYMENT, id)
            .expect("the register holds the proposal")
            .status,
        ProposalStatus::Approved
    );
    assert_ok!(Governance::execute(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        id
    ));
}

fn stored(survey: &str) -> Value {
    <Test as crate::Config>::Schema::collection_version(&DEPLOYMENT, &CollectionId::new("parcel"))
        .expect("the registry holds the collection");
    State::read(
        &DEPLOYMENT,
        &CollectionId::new("parcel"),
        &record_key(survey),
    )
    .expect("the store holds the record")
}

fn field(record: &Value, name: &str) -> Option<Value> {
    pallet_cerulea_state::value_at(record, &FieldPath::parse(name))
}

#[test]
fn a_version_records_and_becomes_active() {
    new_test_ext().execute_with(|| {
        assert_ok!(Evolution::record_version(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            1,
            [7u8; 32],
            1,
            bounded(b"The first published Blueprint.".to_vec())
        ));
        assert_eq!(crate::ActiveVersion::<Test>::get(DEPLOYMENT), 1);
        let held = Versions::<Test>::get(DEPLOYMENT, 1).expect("the registry holds the version");
        assert_eq!(held.blueprint, [7u8; 32]);
        assert_eq!(held.recorded_by, STEWARD);
    });
}

#[test]
fn one_version_number_records_once() {
    new_test_ext().execute_with(|| {
        assert_ok!(Evolution::record_version(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            1,
            [7u8; 32],
            1,
            bounded(b"The first published Blueprint.".to_vec())
        ));
        assert_noop!(
            Evolution::record_version(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                1,
                [8u8; 32],
                2,
                bounded(b"A second Blueprint under one number.".to_vec())
            ),
            Error::<Test>::VersionPresent
        );
    });
}

#[test]
fn a_plan_records_whether_it_breaks_an_integration() {
    new_test_ext().execute_with(|| {
        install();
        let open = plan(&add_ward_plan(), DAY);
        let breaking = plan(&remove_holder_plan(), DAY);
        assert!(
            !Plans::<Test>::get(DEPLOYMENT, open)
                .expect("the registry holds the plan")
                .breaking
        );
        assert!(
            Plans::<Test>::get(DEPLOYMENT, breaking)
                .expect("the registry holds the plan")
                .breaking
        );
    });
}

#[test]
fn a_plan_naming_an_unknown_collection_declines() {
    new_test_ext().execute_with(|| {
        install();
        let absent = MigrationPlan::new("holder", 1, "The register changes the holder list.").with(
            MigrationStep::AddField {
                field: FieldName::new("ward"),
                ty: TypeExpr::Text,
                default: Value::Text("unassigned".to_string()),
                summary: "The ward.".to_string(),
            },
        );
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(absent.encode()),
                DAY
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_plan_starting_from_another_version_declines() {
    new_test_ext().execute_with(|| {
        install();
        let mut ahead = add_ward_plan();
        ahead.from_version = 4;
        ahead.to_version = 5;
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(ahead.encode()),
                DAY
            ),
            Error::<Test>::VersionMismatch
        );
    });
}

#[test]
fn a_window_past_the_limit_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Evolution::plan_migration(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(add_ward_plan().encode()),
                DAY * 30
            ),
            Error::<Test>::WindowTooLong
        );
    });
}

#[test]
fn a_dry_run_reports_the_reverse_and_changes_nothing() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the registry holds the plan");
        assert_eq!(held.status, PlanStatus::Checked);
        assert!(held.reversible);

        // The registry still reports version one and the record keeps its
        // earlier shape.
        assert_eq!(
            <Test as crate::Config>::Schema::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("parcel")
            ),
            Some(1)
        );
        assert!(field(&stored("12/3"), "ward").is_none());
    });
}

#[test]
fn an_apply_before_the_dry_run_declines() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_noop!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::DryRunPending
        );
    });
}

#[test]
fn an_open_plan_applies_and_verifies() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the registry holds the plan");
        assert_eq!(held.status, PlanStatus::Applied);
        assert_eq!(held.records_rewritten, 2);
        assert_eq!(held.rollback_deadline, Some(clock() + DAY));
        assert_eq!(
            <Test as crate::Config>::Schema::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("parcel")
            ),
            Some(2)
        );

        assert_ok!(Evolution::verify(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Plans::<Test>::get(DEPLOYMENT, id)
                .expect("the registry holds the plan")
                .status,
            PlanStatus::Verified
        );
        assert_eq!(AppliedCount::<Test>::get(DEPLOYMENT), 2);
    });
}

#[test]
fn an_existing_integration_keeps_working_after_a_version_transition() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        // Every field an existing reader knows still reads back with its
        // earlier value. The new field carries the declared value.
        for (survey, holder, area) in [("12/3", "Anitha", 400i128), ("14/1", "Benny", 250)] {
            let record = stored(survey);
            assert_eq!(
                field(&record, "survey_number"),
                Some(Value::Text(survey.to_string()))
            );
            assert_eq!(
                field(&record, "holder"),
                Some(Value::Text(holder.to_string()))
            );
            assert_eq!(field(&record, "area"), Some(Value::Integer(area)));
            assert_eq!(
                field(&record, "ward"),
                Some(Value::Text("unassigned".to_string()))
            );
        }
    });
}

#[test]
fn an_applied_plan_rolls_back_cleanly_inside_its_window() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(Evolution::rollback_available(&DEPLOYMENT, id));

        advance_clock(DAY / 2);
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        assert_eq!(
            Plans::<Test>::get(DEPLOYMENT, id)
                .expect("the registry holds the plan")
                .status,
            PlanStatus::RolledBack
        );
        assert_eq!(
            <Test as crate::Config>::Schema::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("parcel")
            ),
            Some(1)
        );
        let record = stored("12/3");
        assert!(field(&record, "ward").is_none());
        assert_eq!(
            field(&record, "holder"),
            Some(Value::Text("Anitha".to_string()))
        );
    });
}

#[test]
fn a_rollback_past_the_window_declines() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        advance_clock(DAY + 1);
        assert!(!Evolution::rollback_available(&DEPLOYMENT, id));
        assert_noop!(
            Evolution::roll_back(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::WindowClosed
        );
    });
}

#[test]
fn a_breaking_plan_without_approval_declines() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&remove_holder_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_noop!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::ApprovalMissing
        );
        assert_eq!(
            <Test as crate::Config>::Schema::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("parcel")
            ),
            Some(1)
        );
    });
}

#[test]
fn a_breaking_plan_applies_once_it_reaches_the_structural_threshold() {
    new_test_ext().execute_with(|| {
        install();
        let body = remove_holder_plan();
        let id = plan(&body, DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        approve_schema_change(body.encode());

        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            <Test as crate::Config>::Schema::collection_version(
                &DEPLOYMENT,
                &CollectionId::new("parcel")
            ),
            Some(2)
        );
        assert!(field(&stored("12/3"), "holder").is_none());
        assert_eq!(field(&stored("12/3"), "area"), Some(Value::Integer(400)));
    });
}

#[test]
fn a_breaking_plan_carries_no_reverse() {
    new_test_ext().execute_with(|| {
        install();
        let body = remove_holder_plan();
        let id = plan(&body, DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(
            !Plans::<Test>::get(DEPLOYMENT, id)
                .expect("the registry holds the plan")
                .reversible
        );
        approve_schema_change(body.encode());
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_noop!(
            Evolution::roll_back(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::Irreversible
        );
    });
}

#[test]
fn the_applied_log_records_every_step_of_the_transition() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::verify(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        let log = Evolution::applied_log(&DEPLOYMENT);
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].status, PlanStatus::Applied);
        assert_eq!(log[0].to_version, 2);
        assert_eq!(log[1].status, PlanStatus::Verified);
        assert_eq!(log[2].status, PlanStatus::RolledBack);
        assert_eq!(log[2].to_version, 1);
    });
}

#[test]
fn a_rolled_back_plan_runs_no_further() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::roll_back(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_noop!(
            Evolution::verify(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::PlanClosed
        );
    });
}

#[test]
fn the_platform_version_moves_up_and_down() {
    new_test_ext().execute_with(|| {
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 0);
        assert_ok!(Evolution::set_platform_version(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            3,
            bounded(b"The operator pushed the platform update.".to_vec())
        ));
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 3);
        assert_ok!(Evolution::set_platform_version(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            2,
            bounded(b"The operator moved the platform back one version.".to_vec())
        ));
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 2);
    });
}

#[test]
fn an_unknown_plan_declines_every_call() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Evolution::dry_run(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 9),
            Error::<Test>::PlanUnknown
        );
        assert_noop!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 9),
            Error::<Test>::PlanUnknown
        );
    });
}

#[test]
fn every_evolution_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        install();
        let id = plan(&add_ward_plan(), DAY);
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        let planned = Audit::query_by_event(&DEPLOYMENT, "migration_planned", 0, 64);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].actor, STEWARD);
        assert_eq!(planned[0].module, "evolution");
        assert_eq!(planned[0].class, WorkClass::Structural);

        let applied = Audit::query_by_event(&DEPLOYMENT, "migration_applied", 0, 64);
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0].actor, STEWARD);
    });
}
