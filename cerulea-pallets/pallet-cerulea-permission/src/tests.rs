//! The tests of the permission module.

use crate::{
    mock::*, CapabilityKey, Error, GrantPayload, Matrix, Pallet, PermissionMatrix, RoleSimulation,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    permissions::FieldQualification,
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, PurposeTag, RecordDef, TypeDef, TypeExpr,
        TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest, PermissionGate},
    primitives::{name_of, BoundedName, DeploymentId},
};

const DEPLOYMENT: DeploymentId = [3u8; 32];
const STEWARD: u64 = 1;
const ANITHA: u64 = 2;
const BENNY: u64 = 3;

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn text(value: &str) -> BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    BoundedVec::try_from(value.as_bytes().to_vec()).expect("the text fits")
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
            FieldDef::new("ward", TypeExpr::Text, "The ward the parcel sits in."),
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
        &["registration", "audit"],
    );
    collection
}

fn parcel(survey: &str, holder: &str, ward: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("ward"), Value::Text(ward.to_string())),
        ],
    );
    value.canonicalise();
    value
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
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(STEWARD),
        ANITHA,
        [7u8; 32]
    ));
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(STEWARD),
        BENNY,
        [8u8; 32]
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("registrar"),
        None,
        BoundedVec::default(),
        text("Registrar")
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("clerk"),
        None,
        BoundedVec::default(),
        text("Clerk")
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        ANITHA,
        name("registrar"),
        None
    ));
}

fn grant(role: &str, capability: CapabilityKey, payload: GrantPayload) {
    assert_ok!(Permission::grant(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name(role),
        name("parcel"),
        capability,
        bounded(payload.encode())
    ));
}

fn request(actor: &u64, action: AccessAction) -> AccessRequest<'_, u64> {
    AccessRequest::new(
        DEPLOYMENT,
        actor,
        CollectionId::new("parcel"),
        action,
        cerulea_blueprint::types::Timestamp::from_millis(START),
    )
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the traits stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_gate<G: PermissionGate>() {}
    fn requires_matrix<M: PermissionMatrix>() {}
    requires_gate::<Pallet<Test>>();
    requires_matrix::<Pallet<Test>>();
}

#[test]
fn a_granted_capability_reads_back_from_the_matrix() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let held = Pallet::<Test>::entry(
            &DEPLOYMENT,
            &name("registrar"),
            &name("parcel"),
            &CapabilityKey::Read,
        )
        .expect("the matrix holds the entry");
        assert_eq!(held.fields, FieldQualification::EveryField);
        assert_eq!(held.granter, STEWARD);
        assert_eq!(held.granted_at, START);
    });
}

#[test]
fn a_repeated_grant_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        assert_noop!(
            Permission::grant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                name("parcel"),
                CapabilityKey::Read,
                bounded(GrantPayload::everything().encode())
            ),
            Error::<Test>::EntryPresent
        );
    });
}

#[test]
fn a_grant_over_an_absent_collection_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Permission::grant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                name("ward"),
                CapabilityKey::Read,
                bounded(GrantPayload::everything().encode())
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_revoked_capability_leaves_the_matrix() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        assert_ok!(Permission::revoke(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("registrar"),
            name("parcel"),
            CapabilityKey::Read
        ));
        assert!(Matrix::<Test>::get((
            DEPLOYMENT,
            name("registrar"),
            name("parcel"),
            CapabilityKey::Read
        ))
        .is_none());
        assert_noop!(
            Permission::revoke(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                name("parcel"),
                CapabilityKey::Read
            ),
            Error::<Test>::EntryAbsent
        );
    });
}

#[test]
fn an_account_without_a_role_is_refused() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let denial = Pallet::<Test>::resolve(&request(&BENNY, AccessAction::Read))
            .expect_err("an account without a role is refused");
        assert!(denial.problem.contains("holds no role"));
        assert!(denial.fix.contains("assign a role"));
    });
}

#[test]
fn a_role_without_the_capability_is_refused() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let denial = Pallet::<Test>::resolve(&request(&ANITHA, AccessAction::Delete))
            .expect_err("a role without the capability is refused");
        assert!(denial.problem.contains("delete"));
        assert!(denial.fix.contains("Grant a role"));
    });
}

#[test]
fn a_role_with_the_capability_is_admitted() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let held = Pallet::<Test>::resolve(&request(&ANITHA, AccessAction::Read))
            .expect("the role reaches the read");
        assert_eq!(held.fields, FieldQualification::EveryField);
        assert!(held.reaches(&FieldPath::parse("holder")));
    });
}

#[test]
fn an_expired_assignment_takes_the_capability_away() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            BENNY,
            name("clerk"),
            Some(START + 1_000)
        ));
        grant("clerk", CapabilityKey::Read, GrantPayload::everything());
        assert!(Pallet::<Test>::resolve(&request(&BENNY, AccessAction::Read)).is_ok());
        set_clock(START + 5_000);
        assert!(Pallet::<Test>::resolve(&request(&BENNY, AccessAction::Read)).is_err());
    });
}

#[test]
fn a_field_outside_the_entry_is_refused() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("survey_number")]),
        );
        let held = request(&ANITHA, AccessAction::Read).on_field(FieldPath::parse("survey_number"));
        assert!(Pallet::<Test>::resolve(&held).is_ok());

        let refused = request(&ANITHA, AccessAction::Read).on_field(FieldPath::parse("holder"));
        let denial =
            Pallet::<Test>::resolve(&refused).expect_err("a field outside the entry is refused");
        assert!(denial.problem.contains("holder"));
        assert!(denial.fix.contains("field list"));
    });
}

#[test]
fn a_held_out_field_is_refused_and_the_rest_are_admitted() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::apart_from(vec![FieldPath::parse("holder")]),
        );
        let admitted = request(&ANITHA, AccessAction::Read).on_field(FieldPath::parse("ward"));
        assert!(Pallet::<Test>::resolve(&admitted).is_ok());
        let refused = request(&ANITHA, AccessAction::Read).on_field(FieldPath::parse("holder"));
        assert!(Pallet::<Test>::resolve(&refused).is_err());
    });
}

fn ward_predicate(ward: &str) -> Expr {
    Expr::binary(
        BinaryOp::Equal,
        Expr::field(Expr::bound("record"), "ward"),
        Expr::text(ward),
    )
}

#[test]
fn a_row_predicate_admits_one_record_and_declines_another() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::everything().where_rows(ward_predicate("north")),
        );
        let inside = parcel("12/3", "Anitha", "north");
        let outside = parcel("44/9", "Benny", "south");
        let admitted = request(&ANITHA, AccessAction::Read).about(&inside);
        assert!(Pallet::<Test>::resolve(&admitted).is_ok());

        let refused = request(&ANITHA, AccessAction::Read).about(&outside);
        let denial =
            Pallet::<Test>::resolve(&refused).expect_err("the predicate declines the record");
        assert!(denial.problem.contains("row predicate"));
        assert!(denial.fix.contains("Widen the row predicate"));
    });
}

#[test]
fn an_entry_without_a_predicate_admits_every_record() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let held = parcel("44/9", "Benny", "south");
        assert!(
            Pallet::<Test>::resolve(&request(&ANITHA, AccessAction::Read).about(&held)).is_ok()
        );
    });
}

#[test]
fn a_read_outside_the_declared_purpose_is_refused() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let held =
            request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("registration"));
        assert!(Pallet::<Test>::resolve(&held).is_ok());

        let refused = request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("resale"));
        let denial = Pallet::<Test>::resolve(&refused)
            .expect_err("a purpose outside the declaration is refused");
        assert!(denial.problem.contains("resale"));
    });
}

#[test]
fn an_entry_serving_one_purpose_declines_another() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::everything().for_purposes(&["audit"]),
        );
        let admitted = request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("audit"));
        assert!(Pallet::<Test>::resolve(&admitted).is_ok());
        let refused =
            request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("registration"));
        assert!(Pallet::<Test>::resolve(&refused).is_err());
    });
}

#[test]
fn two_roles_widen_the_field_reach() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Identity::assign_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            ANITHA,
            name("clerk"),
            None
        ));
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("survey_number")]),
        );
        grant(
            "clerk",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("holder")]),
        );
        let held = <Pallet<Test> as PermissionMatrix>::readable_fields(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
        );
        assert!(held.reaches(&FieldPath::parse("survey_number")));
        assert!(held.reaches(&FieldPath::parse("holder")));
        assert!(!held.reaches(&FieldPath::parse("ward")));
    });
}

#[test]
fn an_invoke_capability_reaches_the_named_action() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Invoke(name("record_transfer")),
            GrantPayload::everything(),
        );
        let held = request(
            &ANITHA,
            AccessAction::Invoke(cerulea_blueprint::types::ActionId::new("record_transfer")),
        );
        assert!(Pallet::<Test>::resolve(&held).is_ok());
        let other = request(
            &ANITHA,
            AccessAction::Invoke(cerulea_blueprint::types::ActionId::new("retire_parcel")),
        );
        assert!(Pallet::<Test>::resolve(&other).is_err());
    });
}

#[test]
fn a_delegation_carries_an_invoke_capability_to_another_account() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Invoke(name("record_transfer")),
            GrantPayload::everything(),
        );
        let held = request(
            &BENNY,
            AccessAction::Invoke(cerulea_blueprint::types::ActionId::new("record_transfer")),
        );
        assert!(Pallet::<Test>::resolve(&held).is_err());

        assert_ok!(Identity::delegate(
            RuntimeOrigin::signed(ANITHA),
            DEPLOYMENT,
            BENNY,
            name("registrar"),
            BoundedVec::try_from(vec![name("record_transfer")]).expect("the scope fits"),
            START + 10_000,
            None
        ));
        let after = request(
            &BENNY,
            AccessAction::Invoke(cerulea_blueprint::types::ActionId::new("record_transfer")),
        );
        assert!(Pallet::<Test>::resolve(&after).is_ok());

        set_clock(START + 20_000);
        let lapsed = request(
            &BENNY,
            AccessAction::Invoke(cerulea_blueprint::types::ActionId::new("record_transfer")),
        );
        assert!(Pallet::<Test>::resolve(&lapsed).is_err());
    });
}

#[test]
fn a_simulation_reports_what_one_role_reaches() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("survey_number")])
                .where_rows(ward_predicate("north")),
        );
        grant(
            "registrar",
            CapabilityKey::Insert,
            GrantPayload::everything(),
        );
        grant(
            "registrar",
            CapabilityKey::Invoke(name("record_transfer")),
            GrantPayload::everything(),
        );
        let found: RoleSimulation = <Pallet<Test> as PermissionMatrix>::simulate(
            &DEPLOYMENT,
            &name("registrar"),
            &name("parcel"),
        );
        assert!(found.read);
        assert!(found.insert);
        assert!(!found.update);
        assert!(!found.delete);
        assert_eq!(found.actions, vec![name("record_transfer")]);
        assert!(found.row_predicate.is_some());
        assert!(found
            .readable_fields
            .reaches(&FieldPath::parse("survey_number")));
        assert!(!found.is_empty());
        let statement = found.statement();
        assert!(statement.contains("registrar"));
        assert!(statement.ends_with('.'));
    });
}

#[test]
fn a_simulation_of_a_role_without_entries_reports_nothing() {
    new_test_ext().execute_with(|| {
        install();
        let found = <Pallet<Test> as PermissionMatrix>::simulate(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
        );
        assert!(found.is_empty());
        assert!(found.statement().contains("reaches nothing"));
    });
}

#[test]
fn a_preview_writes_the_statement_into_an_event() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        assert_ok!(Permission::preview_as_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("registrar"),
            name("parcel")
        ));
        let found = System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::Permission(crate::Event::RolePreviewed { statement, .. }) => {
                    Some(statement)
                }
                _ => None,
            })
            .last()
            .expect("the preview wrote an event");
        let held = String::from_utf8(found.to_vec()).expect("the statement reads back");
        assert!(held.contains("read"));
    });
}

#[test]
fn a_capability_key_names_its_shape() {
    assert_eq!(CapabilityKey::Read.label(), "read");
    assert!(CapabilityKey::Insert.writes());
    assert!(!CapabilityKey::Read.writes());
    assert_eq!(
        CapabilityKey::of(&AccessAction::Delete),
        Some(CapabilityKey::Delete)
    );
}

#[test]
fn the_matrix_allows_a_capability_the_account_reaches() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Update,
            GrantPayload::everything(),
        );
        assert!(<Pallet<Test> as PermissionMatrix>::allows(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &CapabilityKey::Update
        ));
        assert!(!<Pallet<Test> as PermissionMatrix>::allows(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &CapabilityKey::Delete
        ));
    });
}

#[test]
fn a_read_returns_the_named_fields_alone() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![
                FieldPath::parse("survey_number"),
                FieldPath::parse("ward"),
            ]),
        );
        let held = parcel("12/3", "Anitha", "north");
        let seen = <Pallet<Test> as PermissionMatrix>::visible_record(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &held,
        )
        .expect("the role reads the record");
        match seen {
            Value::Record(record) => {
                assert_eq!(record.fields.len(), 2);
                assert!(record.field(&FieldName::new("holder")).is_none());
                assert!(record.field(&FieldName::new("ward")).is_some());
            }
            other => unreachable!("the read produced {:?}", other),
        }
    });
}

#[test]
fn a_read_removes_the_held_out_fields() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::apart_from(vec![FieldPath::parse("holder")]),
        );
        let held = parcel("12/3", "Anitha", "north");
        let seen = <Pallet<Test> as PermissionMatrix>::visible_record(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &held,
        )
        .expect("the role reads the record");
        match seen {
            Value::Record(record) => {
                assert_eq!(record.fields.len(), 2);
                assert!(record.field(&FieldName::new("holder")).is_none());
            }
            other => unreachable!("the read produced {:?}", other),
        }
    });
}

#[test]
fn a_row_predicate_removes_the_records_it_declines() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::everything().where_rows(ward_predicate("north")),
        );
        let records = vec![
            parcel("12/3", "Anitha", "north"),
            parcel("44/9", "Benny", "south"),
            parcel("55/5", "Chandra", "north"),
        ];
        let seen = <Pallet<Test> as PermissionMatrix>::visible_records(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &records,
        );
        assert_eq!(seen.len(), 2);
        let declined = <Pallet<Test> as PermissionMatrix>::visible_record(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &parcel("44/9", "Benny", "south"),
        );
        assert!(declined.is_none());
    });
}

#[test]
fn a_row_predicate_and_a_field_list_run_together() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("survey_number")])
                .where_rows(ward_predicate("north")),
        );
        let records = vec![
            parcel("12/3", "Anitha", "north"),
            parcel("44/9", "Benny", "south"),
        ];
        let seen = <Pallet<Test> as PermissionMatrix>::visible_records(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &records,
        );
        assert_eq!(seen.len(), 1);
        match &seen[0] {
            Value::Record(record) => {
                assert_eq!(record.fields.len(), 1);
                assert!(record.field(&FieldName::new("survey_number")).is_some());
            }
            other => unreachable!("the read produced {:?}", other),
        }
    });
}

#[test]
fn an_account_without_the_read_capability_sees_nothing() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Insert,
            GrantPayload::everything(),
        );
        let seen = <Pallet<Test> as PermissionMatrix>::visible_records(
            &DEPLOYMENT,
            &ANITHA,
            &CollectionId::new("parcel"),
            &[parcel("12/3", "Anitha", "north")],
        );
        assert!(seen.is_empty());
    });
}

#[test]
fn a_record_preview_shows_what_a_role_would_see() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "clerk",
            CapabilityKey::Read,
            GrantPayload::only_fields(vec![FieldPath::parse("survey_number")]),
        );
        // No account holds the clerk role. The preview reads the matrix
        // directly.
        let found = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("12/3", "Anitha", "north"),
        );
        assert!(found.visible);
        assert_eq!(found.kept, vec![FieldName::new("survey_number")]);
        assert!(found.statement().contains("survey_number"));
    });
}

#[test]
fn a_record_preview_of_a_role_without_the_capability_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        let found = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("12/3", "Anitha", "north"),
        );
        assert!(!found.visible);
        let refusal = found.refusal.expect("the preview names the refusal");
        assert!(refusal.fix.contains("Grant the role"));
    });
}

#[test]
fn a_record_preview_reports_a_row_the_predicate_declines() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "clerk",
            CapabilityKey::Read,
            GrantPayload::everything().where_rows(ward_predicate("north")),
        );
        let inside = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("12/3", "Anitha", "north"),
        );
        assert!(inside.visible);
        let outside = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("44/9", "Benny", "south"),
        );
        assert!(!outside.visible);
        assert!(outside.statement().contains("row predicate"));
    });
}

#[test]
fn a_record_preview_writes_its_statement_into_an_event() {
    new_test_ext().execute_with(|| {
        install();
        grant("clerk", CapabilityKey::Read, GrantPayload::everything());
        assert_ok!(Permission::preview_record_as_role(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("clerk"),
            name("parcel"),
            CapabilityKey::Read,
            bounded(parcel("12/3", "Anitha", "north").canonical_bytes())
        ));
        let found = System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::Permission(crate::Event::RecordPreviewed {
                    visible,
                    statement,
                    ..
                }) => Some((visible, statement)),
                _ => None,
            })
            .last()
            .expect("the preview wrote an event");
        assert!(found.0);
        let held = String::from_utf8(found.1.to_vec()).expect("the statement reads back");
        assert!(held.contains("holder"));
    });
}

#[test]
fn a_record_preview_declines_a_payload_that_fails_to_decode() {
    new_test_ext().execute_with(|| {
        install();
        grant("clerk", CapabilityKey::Read, GrantPayload::everything());
        assert_noop!(
            Permission::preview_record_as_role(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("clerk"),
                name("parcel"),
                CapabilityKey::Read,
                bounded(vec![255u8, 255, 255, 255])
            ),
            Error::<Test>::RecordUnreadable
        );
    });
}
