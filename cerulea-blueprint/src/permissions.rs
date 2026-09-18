//! The capability matrix.
//!
//! One entry grants one role one capability over one collection. Each entry
//! qualifies down to the field level and down to the row level.
//!
//! Field qualification names the fields the entry reaches. Row qualification
//! carries a predicate over the record. The predicate binds the record to
//! the name `record`, so an author writes `record.holder` beside the caller
//! identity.

use crate::{
    collections::CollectionRegistry,
    expression::{Env, Expr, Meter, TypeContext},
    identities::Identities,
    types::{
        ActionId, CollectionId, FieldPath, ParamName, PurposeTag, RoleId, TypeExpr, TypeRegistry,
        Value,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The name a row predicate binds the record to.
pub const ROW_BINDING: &str = "record";

/// One capability over a collection.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub enum Capability {
    /// Read records.
    Read,
    /// Add records.
    Insert,
    /// Change records already present.
    Update,
    /// Remove records.
    Delete,
    /// Run the named action against the collection.
    Invoke(ActionId),
}

impl Capability {
    /// A word naming the capability for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            Capability::Read => "read",
            Capability::Insert => "insert",
            Capability::Update => "update",
            Capability::Delete => "delete",
            Capability::Invoke(_) => "invoke",
        }
    }

    /// Report whether the capability changes state.
    pub fn writes(&self) -> bool {
        matches!(
            self,
            Capability::Insert | Capability::Update | Capability::Delete
        )
    }
}

/// The fields one entry reaches.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub enum FieldQualification {
    /// Every field of the record.
    #[default]
    EveryField,
    /// The listed fields alone.
    Only(Vec<FieldPath>),
    /// Every field apart from the listed ones.
    Apart(Vec<FieldPath>),
}

impl FieldQualification {
    /// Report whether the entry reaches the named field.
    pub fn reaches(&self, field: &FieldPath) -> bool {
        match self {
            FieldQualification::EveryField => true,
            FieldQualification::Only(listed) => listed.iter().any(|held| held == field),
            FieldQualification::Apart(listed) => listed.iter().all(|held| held != field),
        }
    }

    /// The fields the entry names.
    pub fn listed(&self) -> &[FieldPath] {
        match self {
            FieldQualification::EveryField => &[],
            FieldQualification::Only(listed) | FieldQualification::Apart(listed) => listed,
        }
    }
}

/// The rows one entry reaches.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub enum RowQualification {
    /// Every row of the collection.
    #[default]
    EveryRow,
    /// The rows where the predicate holds.
    Where(Expr),
}

impl RowQualification {
    /// The predicate, present on a qualified entry.
    pub fn predicate(&self) -> Option<&Expr> {
        match self {
            RowQualification::EveryRow => None,
            RowQualification::Where(expr) => Some(expr),
        }
    }

    /// Run the predicate against one record.
    ///
    /// An entry reaching every row admits the record without running an
    /// expression.
    pub fn admits(
        &self,
        record: &Value,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<bool, Diagnostic> {
        match self {
            RowQualification::EveryRow => Ok(true),
            RowQualification::Where(expr) => {
                let bound = env.with(&ParamName::new(ROW_BINDING), record.clone());
                match expr.evaluate(&bound, meter, path)? {
                    Value::Bool(held) => Ok(held),
                    other => Err(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.location(),
                        format!(
                            "The row predicate produced {}.",
                            other.infer_type().describe()
                        ),
                        "Give the row predicate a truth value.".to_string(),
                    )),
                }
            }
        }
    }
}

/// One entry of the capability matrix.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct CapabilityEntry {
    /// The role the entry grants.
    pub role: RoleId,
    /// The collection the entry covers.
    pub collection: CollectionId,
    /// The capability granted.
    pub capability: Capability,
    /// The fields the entry reaches.
    pub fields: FieldQualification,
    /// The rows the entry reaches.
    pub rows: RowQualification,
    /// The purposes a read under this entry declares.
    pub purposes: Vec<PurposeTag>,
}

impl CapabilityEntry {
    /// Grant a role one capability over every field and every row.
    pub fn grant(role: &str, collection: &str, capability: Capability) -> Self {
        CapabilityEntry {
            role: RoleId::new(role),
            collection: CollectionId::new(collection),
            capability,
            fields: FieldQualification::EveryField,
            rows: RowQualification::EveryRow,
            purposes: Vec::new(),
        }
    }

    /// Narrow the entry to the listed fields.
    pub fn only_fields(mut self, fields: &[&str]) -> Self {
        self.fields =
            FieldQualification::Only(fields.iter().map(|f| FieldPath::parse(f)).collect());
        self
    }

    /// Narrow the entry to the rows where the predicate holds.
    pub fn where_rows(mut self, predicate: Expr) -> Self {
        self.rows = RowQualification::Where(predicate);
        self
    }

    /// Declare the purposes a read under this entry carries.
    pub fn for_purposes(mut self, purposes: &[&str]) -> Self {
        self.purposes = purposes.iter().map(|tag| PurposeTag::new(tag)).collect();
        self
    }
}

/// The permissions section.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Permissions {
    /// The entries of the capability matrix.
    pub entries: Vec<CapabilityEntry>,
}

impl Permissions {
    /// The entries granting one role one capability over one collection.
    pub fn resolve(
        &self,
        role: &RoleId,
        collection: &CollectionId,
        capability: &Capability,
    ) -> Vec<&CapabilityEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                &entry.role == role
                    && &entry.collection == collection
                    && &entry.capability == capability
            })
            .collect()
    }

    /// Report whether a role reaches one field of one collection under one
    /// capability.
    ///
    /// The Studio uses this call for the simulate as role view.
    pub fn reaches_field(
        &self,
        role: &RoleId,
        collection: &CollectionId,
        capability: &Capability,
        field: &FieldPath,
    ) -> bool {
        self.resolve(role, collection, capability)
            .iter()
            .any(|entry| entry.fields.reaches(field))
    }

    /// The actions the matrix names, each with its location.
    pub fn action_refs(&self, path: &Path) -> Vec<(ActionId, Location)> {
        let mut found = Vec::new();
        for (position, entry) in self.entries.iter().enumerate() {
            if let Capability::Invoke(action) = &entry.capability {
                found.push((
                    action.clone(),
                    path.field("entries")
                        .index(position)
                        .field("capability")
                        .location(),
                ));
            }
        }
        found
    }

    /// Check the whole permissions section.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        let entries_path = path.field("entries");
        let mut seen: BTreeSet<String> = BTreeSet::new();

        for (position, entry) in self.entries.iter().enumerate() {
            let here = entries_path.index(position);
            report.absorb(entry.role.validate(&here.field("role")));
            report.absorb(entry.collection.validate(&here.field("collection")));

            if !identities.has_role(&entry.role) {
                report.push(identities.missing_role(&entry.role, here.field("role").location()));
            }

            let collection = collections.get(&entry.collection);
            if collection.is_none() {
                report.push(
                    collections.missing(&entry.collection, here.field("collection").location()),
                );
            }

            let signature = format!(
                "{}|{}|{}",
                entry.role,
                entry.collection,
                match &entry.capability {
                    Capability::Invoke(action) => format!("invoke:{action}"),
                    other => other.label().to_string(),
                }
            );
            if !seen.insert(signature) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.location(),
                    format!(
                        "The matrix grants `{}` the {} capability over `{}` twice.",
                        entry.role,
                        entry.capability.label(),
                        entry.collection
                    ),
                    "Merge the two entries into one.".to_string(),
                ));
            }

            if let Capability::Invoke(action) = &entry.capability {
                report.absorb(action.validate(&here.field("capability")));
            }

            let record_type = collection.map(|found| TypeExpr::Record(found.record_type.clone()));

            for (index, field) in entry.fields.listed().iter().enumerate() {
                let field_path = here.field("fields").index(index);
                report.absorb(field.validate(&field_path));
                if let Some(record_type) = &record_type {
                    if field.resolve(record_type, types).is_none()
                        && types
                            .record(&collection.expect("the collection is present").record_type)
                            .is_some()
                    {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            field_path.location(),
                            format!(
                                "The entry names the field `{field}`, which `{}` lacks.",
                                entry.collection
                            ),
                            format!(
                                "Name a field declared on the record type of `{}`.",
                                entry.collection
                            ),
                        ));
                    }
                }
            }

            if let Some(predicate) = entry.rows.predicate() {
                let rows_path = here.field("rows");
                if let Some(record_type) = &record_type {
                    let ctx = TypeContext::new(types)
                        .with(&ParamName::new(ROW_BINDING), record_type.clone());
                    match predicate.check(&ctx, &rows_path) {
                        Ok(TypeExpr::Bool) => {}
                        Ok(other) => report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            rows_path.location(),
                            format!("The row predicate carries {}.", other.describe()),
                            "Give the row predicate a truth value.".to_string(),
                        )),
                        Err(diagnostic) => report.push(diagnostic),
                    }
                }
            }

            if let Some(collection) = collection {
                for (index, purpose) in entry.purposes.iter().enumerate() {
                    let purpose_path = here.field("purposes").index(index);
                    report.absorb(purpose.validate(&purpose_path));
                    if !collection.purpose.admits(purpose) {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            purpose_path.location(),
                            format!(
                                "The entry declares the purpose `{purpose}`, which `{}` leaves out.",
                                entry.collection
                            ),
                            format!(
                                "Declare `{purpose}` on the collection `{}`, or name a purpose the collection already declares.",
                                entry.collection
                            ),
                        ));
                    }
                }
                if entry.capability == Capability::Read && entry.purposes.is_empty() {
                    report.push(Diagnostic::at(
                        DiagnosticCode::SectionEmpty,
                        here.field("purposes").location(),
                        format!(
                            "The read entry over `{}` declares zero purposes.",
                            entry.collection
                        ),
                        "Declare the purpose the read serves so the purpose check passes."
                            .to_string(),
                    ));
                }
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collections::{Collection, PurposeDeclaration},
        expression::{BinaryOp, Builtin},
        identities::{Identities, RoleDef},
        types::{FieldDef, Identity, RecordDef, Timestamp, TypeDef, TypeId},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                FieldDef::new(
                    "valuation",
                    TypeExpr::Decimal { scale: 2 },
                    "The valuation.",
                ),
            ],
            summary: "One land parcel.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut collection =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        collection.purpose = PurposeDeclaration::new(
            "The register records land holdings.",
            &["registration", "audit"],
        );
        vec![collection]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![
                RoleDef::new(
                    "registrar",
                    None,
                    "Registrar",
                    "The registrar records transfers.",
                ),
                RoleDef::new(
                    "holder",
                    None,
                    "Holder",
                    "The holder reads their own parcel.",
                ),
            ],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn own_rows() -> Expr {
        Expr::binary(
            BinaryOp::Equal,
            Expr::field(Expr::bound(ROW_BINDING), "holder"),
            Expr::Call {
                function: Builtin::Caller,
                args: Vec::new(),
            },
        )
    }

    fn sample() -> Permissions {
        Permissions {
            entries: vec![
                CapabilityEntry::grant("registrar", "parcel", Capability::Read)
                    .for_purposes(&["registration"]),
                CapabilityEntry::grant("registrar", "parcel", Capability::Update),
                CapabilityEntry::grant("holder", "parcel", Capability::Read)
                    .only_fields(&["survey_number", "holder"])
                    .where_rows(own_rows())
                    .for_purposes(&["registration"]),
            ],
        }
    }

    fn run(permissions: &Permissions) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        permissions.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &Path::root().field("permissions"),
        )
    }

    #[test]
    fn a_complete_matrix_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn an_entry_naming_an_absent_role_reports_its_location() {
        let mut permissions = sample();
        permissions.entries[0].role = RoleId::new("absent_role");
        let report = run(&permissions);
        let found = report
            .find("permissions.entries[0].role")
            .expect("the diagnostic sits on the role");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("registrar"));
    }

    #[test]
    fn an_entry_naming_an_absent_collection_reports_its_location() {
        let mut permissions = sample();
        permissions.entries[1].collection = CollectionId::new("absent_collection");
        let report = run(&permissions);
        let found = report
            .find("permissions.entries[1].collection")
            .expect("the diagnostic sits on the collection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_field_qualification_naming_an_absent_field_reports_its_location() {
        let mut permissions = sample();
        permissions.entries[2].fields =
            FieldQualification::Only(vec![FieldPath::parse("absent_field")]);
        let report = run(&permissions);
        assert!(report.find("permissions.entries[2].fields[0]").is_some());
    }

    #[test]
    fn a_row_predicate_carrying_a_value_other_than_a_truth_value_is_caught() {
        let mut permissions = sample();
        permissions.entries[2].rows =
            RowQualification::Where(Expr::field(Expr::bound(ROW_BINDING), "valuation"));
        let report = run(&permissions);
        let found = report
            .find("permissions.entries[2].rows")
            .expect("the diagnostic sits on the row predicate");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
        assert!(found.fix.contains("truth value"));
    }

    #[test]
    fn a_row_predicate_naming_an_absent_field_is_caught() {
        let mut permissions = sample();
        permissions.entries[2].rows = RowQualification::Where(Expr::binary(
            BinaryOp::Equal,
            Expr::field(Expr::bound(ROW_BINDING), "absent_field"),
            Expr::Call {
                function: Builtin::Caller,
                args: Vec::new(),
            },
        ));
        let report = run(&permissions);
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .location
                    .as_str()
                    .starts_with("permissions.entries[2].rows")
            })
            .expect("the diagnostic sits on the row predicate");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_purpose_outside_the_collection_declaration_is_caught() {
        let mut permissions = sample();
        permissions.entries[0].purposes = vec![PurposeTag::new("marketing_list")];
        let report = run(&permissions);
        let found = report
            .find("permissions.entries[0].purposes[0]")
            .expect("the diagnostic sits on the purpose");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_repeated_grant_is_caught() {
        let mut permissions = sample();
        permissions.entries.push(
            CapabilityEntry::grant("registrar", "parcel", Capability::Read)
                .for_purposes(&["audit"]),
        );
        let report = run(&permissions);
        let found = report
            .find("permissions.entries[3]")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn the_row_predicate_admits_the_rows_it_names() {
        let permissions = sample();
        let entry = &permissions.entries[2];
        let caller = Identity([5u8; 32]);
        let env = Env::new(caller, Timestamp::from_millis(1));
        let mut meter = Meter::default();
        let own = Value::record(
            TypeId::new("parcel"),
            vec![
                (
                    crate::types::FieldName::new("survey_number"),
                    Value::Text("KL-1".to_string()),
                ),
                (
                    crate::types::FieldName::new("holder"),
                    Value::Identity(caller),
                ),
                (
                    crate::types::FieldName::new("valuation"),
                    Value::Decimal(crate::types::Decimal::new(1000, 2)),
                ),
            ],
        );
        assert!(entry
            .rows
            .admits(&own, &env, &mut meter, &Path::root().field("rows"))
            .expect("the predicate runs"));

        let other = Value::record(
            TypeId::new("parcel"),
            vec![
                (
                    crate::types::FieldName::new("survey_number"),
                    Value::Text("KL-2".to_string()),
                ),
                (
                    crate::types::FieldName::new("holder"),
                    Value::Identity(Identity([6u8; 32])),
                ),
                (
                    crate::types::FieldName::new("valuation"),
                    Value::Decimal(crate::types::Decimal::new(1000, 2)),
                ),
            ],
        );
        assert!(!entry
            .rows
            .admits(&other, &env, &mut meter, &Path::root().field("rows"))
            .expect("the predicate runs"));
    }

    #[test]
    fn field_qualification_reaches_the_fields_it_names() {
        let permissions = sample();
        let role = RoleId::new("holder");
        let collection = CollectionId::new("parcel");
        assert!(permissions.reaches_field(
            &role,
            &collection,
            &Capability::Read,
            &FieldPath::parse("holder")
        ));
        assert!(!permissions.reaches_field(
            &role,
            &collection,
            &Capability::Read,
            &FieldPath::parse("valuation")
        ));
        let apart = FieldQualification::Apart(vec![FieldPath::parse("valuation")]);
        assert!(apart.reaches(&FieldPath::parse("holder")));
        assert!(!apart.reaches(&FieldPath::parse("valuation")));
    }

    #[test]
    fn the_matrix_reports_the_actions_it_names() {
        let mut permissions = sample();
        permissions.entries.push(CapabilityEntry::grant(
            "registrar",
            "parcel",
            Capability::Invoke(ActionId::new("record_transfer")),
        ));
        let refs = permissions.action_refs(&Path::root().field("permissions"));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].1.as_str(), "permissions.entries[3].capability");
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let permissions = sample();
        let bytes = permissions.encode();
        let back = Permissions::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(permissions, back);
        let text = serde_json::to_string(&permissions).expect("the section serialises");
        let parsed: Permissions = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(permissions, parsed);
    }
}
