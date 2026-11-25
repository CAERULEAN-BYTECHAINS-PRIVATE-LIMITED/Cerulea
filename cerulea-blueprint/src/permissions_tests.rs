//! Behavioural tests for the capability matrix.
//!
//! Each test runs the real row check, field check or section check and
//! asserts the concrete result, the diagnostic class and the fix.

use crate::{
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    expression::{BinaryOp, Env, Expr, Meter},
    identities::{Identities, RoleDef},
    permissions::{
        Capability, CapabilityEntry, FieldQualification, Permissions, RowQualification, ROW_BINDING,
    },
    types::{
        ActionId, CollectionId, FieldDef, FieldName, FieldPath, RecordDef, RoleId, TypeDef,
        TypeExpr, TypeId, TypeRegistry, Value,
    },
    DiagnosticCode, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("patient"),
        fields: vec![
            FieldDef::new("number", TypeExpr::Text, "The patient number."),
            FieldDef::new("ward", TypeExpr::Integer, "The ward."),
            FieldDef::new("diagnosis", TypeExpr::Text, "The diagnosis."),
        ],
        summary: "One patient.".to_string(),
    })]
}

fn collections() -> Vec<Collection> {
    let purpose = PurposeDeclaration::new("The ward keeps patients.", &["care"]);
    let mut patients = Collection::new("patients", "patient", "number", "The patients.");
    patients.purpose = purpose.clone();
    let mut orphan = Collection::new("orphan", "ghost", "number", "Over no declared type.");
    orphan.purpose = purpose;
    vec![patients, orphan]
}

fn identities() -> Identities {
    Identities {
        roles: vec![
            RoleDef::new("nurse", None, "Nurse", "The nurse reads charts."),
            RoleDef::new("doctor", None, "Doctor", "The doctor writes charts."),
        ],
        ..Identities::default()
    }
}

fn ward_is_three() -> Expr {
    Expr::binary(
        BinaryOp::Equal,
        Expr::field(Expr::bound(ROW_BINDING), "ward"),
        Expr::integer(3),
    )
}

fn clean() -> Permissions {
    Permissions {
        entries: vec![
            CapabilityEntry::grant("nurse", "patients", Capability::Read)
                .only_fields(&["number", "ward"])
                .where_rows(ward_is_three())
                .for_purposes(&["care"]),
            CapabilityEntry::grant("doctor", "patients", Capability::Update),
            CapabilityEntry::grant(
                "doctor",
                "patients",
                Capability::Invoke(ActionId::new("discharge")),
            ),
        ],
    }
}

fn report(permissions: &Permissions) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    permissions.validate(
        &registry,
        &collections,
        &identities(),
        &Path::root().field("permissions"),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

fn patient(ward: i128) -> Value {
    Value::record(
        TypeId::new("patient"),
        vec![
            (FieldName::new("number"), Value::Text("P1".to_string())),
            (FieldName::new("ward"), Value::Integer(ward)),
            (FieldName::new("diagnosis"), Value::Text("flu".to_string())),
        ],
    )
}

#[test]
fn capabilities_name_themselves_and_state_whether_they_write() {
    let cases = [
        (Capability::Read, "read", false),
        (Capability::Insert, "insert", true),
        (Capability::Update, "update", true),
        (Capability::Delete, "delete", true),
        (
            Capability::Invoke(ActionId::new("discharge")),
            "invoke",
            false,
        ),
    ];
    for (capability, label, writes) in cases {
        assert_eq!(capability.label(), label);
        assert_eq!(capability.writes(), writes);
    }
}

#[test]
fn field_qualifications_reach_the_fields_they_name() {
    let ward = FieldPath::parse("ward");
    let diagnosis = FieldPath::parse("diagnosis");
    assert!(FieldQualification::EveryField.reaches(&diagnosis));
    assert!(FieldQualification::EveryField.listed().is_empty());
    let only = FieldQualification::Only(vec![ward.clone()]);
    assert!(only.reaches(&ward));
    assert!(!only.reaches(&diagnosis));
    let apart = FieldQualification::Apart(vec![diagnosis.clone()]);
    assert!(apart.reaches(&ward));
    assert!(!apart.reaches(&diagnosis));
    assert_eq!(apart.listed(), [diagnosis.clone()]);

    let permissions = clean();
    let nurse = RoleId::new("nurse");
    let patients = CollectionId::new("patients");
    assert!(permissions.reaches_field(&nurse, &patients, &Capability::Read, &ward));
    assert!(!permissions.reaches_field(&nurse, &patients, &Capability::Read, &diagnosis));
    assert!(!permissions.reaches_field(&nurse, &patients, &Capability::Update, &ward));
    assert_eq!(
        permissions
            .resolve(&RoleId::new("doctor"), &patients, &Capability::Update)
            .len(),
        1
    );
    let refs = permissions.action_refs(&Path::root().field("permissions"));
    assert_eq!(refs.len(), 1);
    assert_eq!(refs[0].0, ActionId::new("discharge"));
    assert_eq!(refs[0].1.as_str(), "permissions.entries[2].capability");
}

#[test]
fn row_qualifications_admit_the_rows_their_predicate_holds_for() {
    let env = Env::default();
    let mut meter = Meter::default();
    let every = RowQualification::EveryRow;
    assert_eq!(every.predicate(), None);
    assert_eq!(
        every.admits(&patient(1), &env, &mut meter, &Path::root()),
        Ok(true)
    );
    let ward = RowQualification::Where(ward_is_three());
    assert_eq!(ward.predicate(), Some(&ward_is_three()));
    assert_eq!(
        ward.admits(&patient(3), &env, &mut meter, &Path::root()),
        Ok(true)
    );
    assert_eq!(
        ward.admits(&patient(4), &env, &mut meter, &Path::root()),
        Ok(false)
    );
    let numeric = RowQualification::Where(Expr::field(Expr::bound(ROW_BINDING), "ward"));
    let fault = numeric
        .admits(&patient(3), &env, &mut meter, &Path::root().field("rows"))
        .expect_err("a number is no truth value");
    assert_eq!(fault.code, DiagnosticCode::TypeMismatch);
    assert_eq!(fault.problem, "The row predicate produced a whole number.");
    assert_eq!(fault.fix, "Give the row predicate a truth value.");
}

#[test]
fn a_clean_matrix_passes() {
    let found = report(&clean());
    assert!(found.is_clean(), "{found}");
}

#[test]
fn entry_faults_each_raise_a_diagnostic() {
    let mut permissions = clean();
    permissions.entries.push(CapabilityEntry::grant(
        "Porter",
        "Ghost",
        Capability::Invoke(ActionId::new("Bad")),
    ));
    permissions.entries.push(CapabilityEntry::grant(
        "doctor",
        "patients",
        Capability::Update,
    ));
    permissions.entries.push(
        CapabilityEntry::grant("nurse", "patients", Capability::Delete)
            .only_fields(&["", "room"])
            .where_rows(Expr::field(Expr::bound(ROW_BINDING), "ward"))
            .for_purposes(&["Care", "billing"]),
    );
    permissions.entries.push(CapabilityEntry::grant(
        "doctor",
        "patients",
        Capability::Read,
    ));
    permissions.entries.push(
        CapabilityEntry::grant("doctor", "patients", Capability::Insert)
            .where_rows(Expr::bound("absent")),
    );
    permissions.entries.push(
        CapabilityEntry::grant("doctor", "orphan", Capability::Insert).only_fields(&["room"]),
    );
    let found = report(&permissions);
    assert_eq!(
        at(&found, "permissions.entries[3].role").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "permissions.entries[3].collection").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "permissions.entries[3].capability").code,
        DiagnosticCode::InvalidIdentifier
    );
    let repeated = at(&found, "permissions.entries[4]");
    assert_eq!(
        repeated.problem,
        "The matrix grants `doctor` the update capability over `patients` twice."
    );
    assert_eq!(repeated.fix, "Merge the two entries into one.");
    assert_eq!(
        at(&found, "permissions.entries[5].fields[0]").code,
        DiagnosticCode::MissingValue
    );
    assert_eq!(
        at(&found, "permissions.entries[5].fields[1]").fix,
        "Name a field declared on the record type of `patients`."
    );
    let rows = at(&found, "permissions.entries[5].rows");
    assert_eq!(rows.problem, "The row predicate carries a whole number.");
    assert_eq!(rows.fix, "Give the row predicate a truth value.");
    assert_eq!(
        at(&found, "permissions.entries[5].purposes[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "permissions.entries[5].purposes[1]").fix,
        "Declare `billing` on the collection `patients`, or name a purpose the collection already declares."
    );
    assert_eq!(
        at(&found, "permissions.entries[6].purposes").fix,
        "Declare the purpose the read serves so the purpose check passes."
    );
    assert_eq!(
        at(&found, "permissions.entries[7].rows").code,
        DiagnosticCode::DanglingReference
    );
    assert!(found.find("permissions.entries[8].fields[0]").is_none());
}
