//! Collection definitions.
//!
//! A collection holds records of one declared type. Every collection names
//! its primary key, its secondary indexes, its privacy class, its retention
//! rule and its declared purpose.
//!
//! Purpose and retention sit on every collection. A read outside the
//! declared purpose is denied and recorded. Retention expiry runs the
//! declared action.

use crate::{
    expression::Expr,
    types::{
        CollectionId, FieldPath, IndexName, PurposeTag, RoleId, TypeExpr, TypeId, TypeRegistry,
        Value,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The visibility class of a collection.
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
pub enum PrivacyClass {
    /// Every party the permission matrix admits reads the record.
    Open,
    /// The Blockchain holds a commitment. The payload lives elsewhere.
    HashOnly,
    /// The Blockchain holds an encrypted payload. Key distribution follows
    /// the listed roles.
    Encrypted {
        /// The roles that receive a key.
        key_holders: Vec<RoleId>,
    },
    /// The Blockchain holds a commitment and a locator. The payload lives in
    /// an external store.
    External {
        /// The field carrying the commitment over the payload.
        commitment: FieldPath,
        /// The field carrying the locator of the external store.
        locator: FieldPath,
    },
}

impl PrivacyClass {
    /// A word naming the class for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            PrivacyClass::Open => "open",
            PrivacyClass::HashOnly => "hash only",
            PrivacyClass::Encrypted { .. } => "encrypted payload",
            PrivacyClass::External { .. } => "external payload",
        }
    }
}

/// The span a record stays in the collection.
#[derive(
    Clone,
    Copy,
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
pub enum RetentionPeriod {
    /// The record stays for the life of the deployment.
    Indefinite,
    /// The record stays for the given span in milliseconds.
    Milliseconds(u64),
}

/// The action retention expiry runs.
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
pub enum RetentionAction {
    /// The record stays in place and the Blockchain records the review date.
    Review,
    /// The Blockchain erases the payload and keeps the commitment.
    Erase,
    /// The Blockchain clears the listed fields and keeps the rest.
    Anonymise(Vec<FieldPath>),
    /// The Blockchain moves the record to the named collection.
    MoveTo(CollectionId),
}

/// The retention rule of a collection.
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
pub struct Retention {
    /// The span a record stays.
    pub period: RetentionPeriod,
    /// The action expiry runs.
    pub on_expiry: RetentionAction,
}

impl Default for Retention {
    fn default() -> Self {
        Retention {
            period: RetentionPeriod::Indefinite,
            on_expiry: RetentionAction::Review,
        }
    }
}

/// The declared purpose of a collection.
///
/// A read outside the declared purpose is denied and recorded in the audit
/// log.
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
pub struct PurposeDeclaration {
    /// One plain sentence stating why the deployment holds this data.
    pub statement: String,
    /// The purposes a read declares to pass the check.
    pub purposes: Vec<PurposeTag>,
}

impl PurposeDeclaration {
    /// Declare a purpose.
    pub fn new(statement: &str, purposes: &[&str]) -> Self {
        PurposeDeclaration {
            statement: statement.to_string(),
            purposes: purposes.iter().map(|tag| PurposeTag::new(tag)).collect(),
        }
    }

    /// Report whether a read carrying the given purpose passes.
    pub fn admits(&self, purpose: &PurposeTag) -> bool {
        self.purposes.contains(purpose)
    }
}

/// A secondary index over a collection.
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
pub struct Index {
    /// The name of the index.
    pub name: IndexName,
    /// The fields the index sorts by, in order.
    pub fields: Vec<FieldPath>,
    /// A unique index admits one record per key.
    pub unique: bool,
}

/// A collection.
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
pub struct Collection {
    /// The name of the collection.
    pub id: CollectionId,
    /// The record type every member carries.
    pub record_type: TypeId,
    /// The field holding the primary key.
    pub primary_key: FieldPath,
    /// The secondary indexes.
    pub indexes: Vec<Index>,
    /// The visibility class.
    pub privacy: PrivacyClass,
    /// The retention rule.
    pub retention: Retention,
    /// The declared purpose.
    pub purpose: PurposeDeclaration,
    /// One plain sentence stating what the collection holds.
    pub summary: String,
}

impl Collection {
    /// Declare an open collection with indefinite retention.
    pub fn new(id: &str, record_type: &str, primary_key: &str, summary: &str) -> Self {
        Collection {
            id: CollectionId::new(id),
            record_type: TypeId::new(record_type),
            primary_key: FieldPath::parse(primary_key),
            indexes: Vec::new(),
            privacy: PrivacyClass::Open,
            retention: Retention::default(),
            purpose: PurposeDeclaration::default(),
            summary: summary.to_string(),
        }
    }

    /// The roles this collection names, each with its location.
    pub fn role_refs(&self, path: &Path) -> Vec<(RoleId, Location)> {
        match &self.privacy {
            PrivacyClass::Encrypted { key_holders } => key_holders
                .iter()
                .enumerate()
                .map(|(position, role)| {
                    (
                        role.clone(),
                        path.field("privacy")
                            .field("key_holders")
                            .index(position)
                            .location(),
                    )
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The collections this collection moves records into on expiry.
    pub fn collection_refs(&self, path: &Path) -> Vec<(CollectionId, Location)> {
        match &self.retention.on_expiry {
            RetentionAction::MoveTo(target) => alloc::vec![(
                target.clone(),
                path.field("retention").field("on_expiry").location()
            )],
            _ => Vec::new(),
        }
    }

    /// The type of the primary key, resolved against the declared types.
    pub fn key_type(&self, registry: &TypeRegistry) -> Option<TypeExpr> {
        self.primary_key
            .resolve(&TypeExpr::Record(self.record_type.clone()), registry)
    }
}

/// An index over the declared collections.
#[derive(Clone, Debug, Default)]
pub struct CollectionRegistry<'a> {
    entries: BTreeMap<&'a str, &'a Collection>,
}

impl<'a> CollectionRegistry<'a> {
    /// Build a registry over a slice of collections.
    pub fn build(collections: &'a [Collection], path: &Path) -> (Self, Report) {
        let mut entries: BTreeMap<&'a str, &'a Collection> = BTreeMap::new();
        let mut report = Report::new();
        for (position, collection) in collections.iter().enumerate() {
            let here = path.index(position);
            if entries.contains_key(collection.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two collections share the name `{}`.", collection.id),
                    format!(
                        "Rename one of the two collections away from `{}`.",
                        collection.id
                    ),
                ));
                continue;
            }
            entries.insert(collection.id.as_str(), collection);
        }
        (CollectionRegistry { entries }, report)
    }

    /// Look up a collection.
    pub fn get(&self, id: &CollectionId) -> Option<&'a Collection> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether a collection exists.
    pub fn contains(&self, id: &CollectionId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The collection names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the collections.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero collections.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for a collection the Blueprint
    /// lacks.
    pub fn missing(&self, id: &CollectionId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The reference names the collection `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a collection named `{id}` in the collections section.")
            } else {
                format!("Point the reference at one of these collections: {listed}.")
            },
        )
    }
}

/// The span of keys a read or a write touches inside one collection.
///
/// The Execution Engine evaluates the key expression at admission time and
/// turns the selector into a resolved range.
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
pub enum KeySelector {
    /// One record, named by an exact key.
    Exact(Expr),
    /// Every record whose key starts with the given value.
    Prefix(Expr),
    /// Every record in the collection.
    Full,
}

impl KeySelector {
    /// A word naming the shape for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            KeySelector::Exact(_) => "exact",
            KeySelector::Prefix(_) => "prefix",
            KeySelector::Full => "full",
        }
    }

    /// The key expression, present on an exact and a prefix selector.
    pub fn expression(&self) -> Option<&Expr> {
        match self {
            KeySelector::Exact(expr) | KeySelector::Prefix(expr) => Some(expr),
            KeySelector::Full => None,
        }
    }
}

/// A key span with the key expression already evaluated.
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
pub enum KeyRange {
    /// One record, named by the canonical encoding of its key.
    Exact(Vec<u8>),
    /// Every record whose canonical key starts with these bytes.
    Prefix(Vec<u8>),
    /// Every record in the collection.
    Full,
}

impl KeyRange {
    /// Build a range from an evaluated key value.
    pub fn from_value(selector_label: &str, value: &Value) -> KeyRange {
        match selector_label {
            "exact" => KeyRange::Exact(value.canonical_bytes()),
            "prefix" => KeyRange::Prefix(value.canonical_bytes()),
            _ => KeyRange::Full,
        }
    }

    /// Report whether this range holds every key the other range holds.
    pub fn covers(&self, other: &KeyRange) -> bool {
        match (self, other) {
            (KeyRange::Full, _) => true,
            (KeyRange::Prefix(prefix), KeyRange::Exact(key)) => key.starts_with(prefix),
            (KeyRange::Prefix(outer), KeyRange::Prefix(inner)) => inner.starts_with(outer),
            (KeyRange::Exact(left), KeyRange::Exact(right)) => left == right,
            _ => false,
        }
    }

    /// A word naming the shape for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            KeyRange::Exact(_) => "exact",
            KeyRange::Prefix(_) => "prefix",
            KeyRange::Full => "full",
        }
    }
}

/// Check a whole collections section.
///
/// The pass reports repeated names, malformed identifiers, record types the
/// Blueprint lacks, primary keys that resolve to no field, index fields that
/// resolve to no field, and purposes left blank.
pub fn validate_collections(
    collections: &[Collection],
    types: &TypeRegistry,
    path: &Path,
) -> Report {
    let (_, mut report) = CollectionRegistry::build(collections, path);
    for (position, collection) in collections.iter().enumerate() {
        let here = path.index(position);
        report.absorb(collection.id.validate(&here.field("id")));
        report.absorb(collection.record_type.validate(&here.field("record_type")));

        let record = types.record(&collection.record_type);
        if record.is_none() {
            report.push(Diagnostic::at(
                DiagnosticCode::DanglingReference,
                here.field("record_type").location(),
                format!(
                    "The collection `{}` names the record type `{}`, which the Blueprint lacks.",
                    collection.id, collection.record_type
                ),
                format!(
                    "Declare a record type named `{}` in the types section.",
                    collection.record_type
                ),
            ));
        }

        report.absorb(collection.primary_key.validate(&here.field("primary_key")));
        if record.is_some() {
            match collection.key_type(types) {
                Some(key_type) => {
                    if !key_type.is_key_safe() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            here.field("primary_key").location(),
                            format!(
                                "The primary key of `{}` carries {}, which lacks a total order.",
                                collection.id,
                                key_type.describe()
                            ),
                            "Choose a primary key field carrying text, a whole number, a timestamp, an identity or a hash."
                                .to_string(),
                        ));
                    }
                }
                None => report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.field("primary_key").location(),
                    format!(
                        "The primary key `{}` resolves to no field on `{}`.",
                        collection.primary_key, collection.record_type
                    ),
                    format!(
                        "Name a field declared on the record type `{}`.",
                        collection.record_type
                    ),
                )),
            }
        }

        let mut index_names: Vec<&IndexName> = Vec::new();
        for (index_position, index) in collection.indexes.iter().enumerate() {
            let index_path = here.field("indexes").index(index_position);
            report.absorb(index.name.validate(&index_path.field("name")));
            if index_names.contains(&&index.name) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    index_path.field("name").location(),
                    format!(
                        "The collection `{}` declares the index `{}` twice.",
                        collection.id, index.name
                    ),
                    format!("Rename one of the two `{}` indexes.", index.name),
                ));
            }
            index_names.push(&index.name);
            if index.fields.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    index_path.field("fields").location(),
                    format!("The index `{}` names zero fields.", index.name),
                    "Name at least one field for the index to sort by.".to_string(),
                ));
            }
            for (field_position, field) in index.fields.iter().enumerate() {
                let field_path = index_path.field("fields").index(field_position);
                report.absorb(field.validate(&field_path));
                if record.is_some()
                    && field
                        .resolve(&TypeExpr::Record(collection.record_type.clone()), types)
                        .is_none()
                {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        field_path.location(),
                        format!(
                            "The index `{}` names the field `{}`, which `{}` lacks.",
                            index.name, field, collection.record_type
                        ),
                        format!(
                            "Name a field declared on the record type `{}`.",
                            collection.record_type
                        ),
                    ));
                }
            }
        }

        validate_privacy(collection, types, &here, &mut report);

        if collection.purpose.statement.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                here.field("purpose").field("statement").location(),
                format!("The collection `{}` states no purpose.", collection.id),
                "Write one plain sentence stating why the deployment holds this data.".to_string(),
            ));
        }
        if collection.purpose.purposes.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                here.field("purpose").field("purposes").location(),
                format!("The collection `{}` declares zero purposes.", collection.id),
                "Declare at least one purpose so a read passes the purpose check.".to_string(),
            ));
        }
        for (tag_position, tag) in collection.purpose.purposes.iter().enumerate() {
            report
                .absorb(tag.validate(&here.field("purpose").field("purposes").index(tag_position)));
        }

        if let RetentionPeriod::Milliseconds(0) = collection.retention.period {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                here.field("retention").field("period").location(),
                format!(
                    "The retention span of `{}` is zero milliseconds.",
                    collection.id
                ),
                "Set a span above zero, or choose indefinite retention.".to_string(),
            ));
        }
        if let RetentionAction::Anonymise(fields) = &collection.retention.on_expiry {
            if fields.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    here.field("retention").field("on_expiry").location(),
                    format!(
                        "The anonymise action on `{}` names zero fields.",
                        collection.id
                    ),
                    "Name the fields the Blockchain clears at expiry.".to_string(),
                ));
            }
            for (field_position, field) in fields.iter().enumerate() {
                let field_path = here
                    .field("retention")
                    .field("on_expiry")
                    .index(field_position);
                report.absorb(field.validate(&field_path));
                if record.is_some()
                    && field
                        .resolve(&TypeExpr::Record(collection.record_type.clone()), types)
                        .is_none()
                {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        field_path.location(),
                        format!(
                            "The anonymise action names the field `{}`, which `{}` lacks.",
                            field, collection.record_type
                        ),
                        format!(
                            "Name a field declared on the record type `{}`.",
                            collection.record_type
                        ),
                    ));
                }
            }
        }
    }
    report
}

fn validate_privacy(
    collection: &Collection,
    types: &TypeRegistry,
    here: &Path,
    report: &mut Report,
) {
    let privacy_path = here.field("privacy");
    match &collection.privacy {
        PrivacyClass::Open | PrivacyClass::HashOnly => {}
        PrivacyClass::Encrypted { key_holders } => {
            if key_holders.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    privacy_path.field("key_holders").location(),
                    format!(
                        "The encrypted collection `{}` names zero key holders.",
                        collection.id
                    ),
                    "Name at least one role that receives the key.".to_string(),
                ));
            }
            for (position, role) in key_holders.iter().enumerate() {
                report.absorb(role.validate(&privacy_path.field("key_holders").index(position)));
            }
        }
        PrivacyClass::External {
            commitment,
            locator,
        } => {
            for (name, field) in [("commitment", commitment), ("locator", locator)] {
                let field_path = privacy_path.field(name);
                report.absorb(field.validate(&field_path));
                match field.resolve(&TypeExpr::Record(collection.record_type.clone()), types) {
                    Some(found) => {
                        let wanted = if name == "commitment" {
                            TypeExpr::Hash
                        } else {
                            TypeExpr::Text
                        };
                        if found != wanted {
                            report.push(Diagnostic::at(
                                DiagnosticCode::TypeMismatch,
                                field_path.location(),
                                format!(
                                    "The {name} field of `{}` carries {}.",
                                    collection.id,
                                    found.describe()
                                ),
                                format!("Give the {name} field {}.", wanted.describe()),
                            ));
                        }
                    }
                    None => {
                        if types.record(&collection.record_type).is_some() {
                            report.push(Diagnostic::at(
                                DiagnosticCode::DanglingReference,
                                field_path.location(),
                                format!(
                                    "The {name} field `{field}` resolves to no field on `{}`.",
                                    collection.record_type
                                ),
                                format!(
                                    "Name a field declared on the record type `{}`.",
                                    collection.record_type
                                ),
                            ));
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{FieldDef, RecordDef, TypeDef};
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                FieldDef::new("district", TypeExpr::Text, "The district."),
                FieldDef::new("payload_hash", TypeExpr::Hash, "The payload commitment."),
                FieldDef::new("payload_locator", TypeExpr::Text, "The external locator."),
            ],
            summary: "One land parcel.".to_string(),
        })]
    }

    fn parcel_collection() -> Collection {
        let mut collection =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        collection.indexes = vec![Index {
            name: IndexName::new("by_district"),
            fields: vec![FieldPath::parse("district")],
            unique: false,
        }];
        collection.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        collection.retention = Retention {
            period: RetentionPeriod::Milliseconds(315_360_000_000),
            on_expiry: RetentionAction::Anonymise(vec![FieldPath::parse("holder")]),
        };
        collection
    }

    #[test]
    fn a_complete_collection_validates() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let report = validate_collections(
            &[parcel_collection()],
            &registry,
            &Path::root().field("collections"),
        );
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_collection_naming_an_absent_record_type_reports_its_location() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.record_type = TypeId::new("absent_record");
        let report =
            validate_collections(&[collection], &registry, &Path::root().field("collections"));
        let found = report
            .find("collections[0].record_type")
            .expect("the diagnostic sits on the record type");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_primary_key_naming_an_absent_field_reports_its_location() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.primary_key = FieldPath::parse("absent_field");
        let report =
            validate_collections(&[collection], &registry, &Path::root().field("collections"));
        let found = report
            .find("collections[0].primary_key")
            .expect("the diagnostic sits on the primary key");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("parcel"));
    }

    #[test]
    fn an_index_naming_an_absent_field_reports_its_location() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.indexes[0].fields = vec![FieldPath::parse("absent_field")];
        let report =
            validate_collections(&[collection], &registry, &Path::root().field("collections"));
        assert!(report.find("collections[0].indexes[0].fields[0]").is_some());
    }

    #[test]
    fn a_repeated_collection_name_reports_its_location() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let report = validate_collections(
            &[parcel_collection(), parcel_collection()],
            &registry,
            &Path::root().field("collections"),
        );
        let found = report
            .find("collections[1].id")
            .expect("the diagnostic sits on the second entry");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn an_external_payload_checks_its_commitment_and_locator() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.privacy = PrivacyClass::External {
            commitment: FieldPath::parse("payload_hash"),
            locator: FieldPath::parse("payload_locator"),
        };
        let report = validate_collections(
            &[collection.clone()],
            &registry,
            &Path::root().field("collections"),
        );
        assert!(report.is_clean(), "{report}");

        collection.privacy = PrivacyClass::External {
            commitment: FieldPath::parse("district"),
            locator: FieldPath::parse("payload_locator"),
        };
        let report =
            validate_collections(&[collection], &registry, &Path::root().field("collections"));
        let found = report
            .find("collections[0].privacy.commitment")
            .expect("the diagnostic sits on the commitment field");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn an_encrypted_collection_lists_its_key_holders() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.privacy = PrivacyClass::Encrypted {
            key_holders: Vec::new(),
        };
        let report = validate_collections(
            &[collection.clone()],
            &registry,
            &Path::root().field("collections"),
        );
        assert!(report.find("collections[0].privacy.key_holders").is_some());

        collection.privacy = PrivacyClass::Encrypted {
            key_holders: vec![RoleId::new("registrar")],
        };
        let report = validate_collections(
            &[collection.clone()],
            &registry,
            &Path::root().field("collections"),
        );
        assert!(report.is_clean(), "{report}");
        let roles = collection.role_refs(&Path::root().field("collections").index(0));
        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].1.as_str(), "collections[0].privacy.key_holders[0]");
    }

    #[test]
    fn a_collection_without_a_purpose_reports_the_fix() {
        let defs = types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let mut collection = parcel_collection();
        collection.purpose = PurposeDeclaration::default();
        let report =
            validate_collections(&[collection], &registry, &Path::root().field("collections"));
        let found = report
            .find("collections[0].purpose.statement")
            .expect("the diagnostic sits on the purpose statement");
        assert!(found.fix.contains("sentence"));
    }

    #[test]
    fn a_key_range_covers_the_ranges_inside_it() {
        let full = KeyRange::Full;
        let prefix = KeyRange::Prefix(alloc::vec![1, 2]);
        let exact = KeyRange::Exact(alloc::vec![1, 2, 3]);
        assert!(full.covers(&prefix));
        assert!(full.covers(&exact));
        assert!(prefix.covers(&exact));
        assert!(!exact.covers(&prefix));
        assert!(!prefix.covers(&KeyRange::Exact(alloc::vec![9])));
    }

    #[test]
    fn the_collection_round_trips_through_scale_and_json() {
        let collection = parcel_collection();
        let bytes = collection.encode();
        let back = Collection::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(collection, back);
        let text = serde_json::to_string(&collection).expect("the collection serialises");
        let parsed: Collection = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(collection, parsed);
    }

    #[test]
    fn the_registry_reports_a_missing_collection_with_the_fix() {
        let collections = [parcel_collection()];
        let (registry, _) =
            CollectionRegistry::build(&collections, &Path::root().field("collections"));
        let diagnostic = registry.missing(
            &CollectionId::new("absent"),
            Path::root()
                .field("actions")
                .index(3)
                .field("effects")
                .index(1)
                .field("collection")
                .location(),
        );
        assert_eq!(
            diagnostic.location.as_str(),
            "actions[3].effects[1].collection"
        );
        assert!(diagnostic.fix.contains("parcel"));
    }

    #[test]
    fn the_purpose_check_admits_a_declared_purpose() {
        let collection = parcel_collection();
        assert!(collection.purpose.admits(&PurposeTag::new("registration")));
        assert!(!collection
            .purpose
            .admits(&PurposeTag::new("marketing_list")));
    }

    #[test]
    fn a_key_selector_names_its_shape() {
        let exact = KeySelector::Exact(Expr::text("KL-07"));
        assert_eq!(exact.label(), "exact");
        assert!(exact.expression().is_some());
        assert_eq!(KeySelector::Full.label(), "full");
        assert!(KeySelector::Full.expression().is_none());
        let key = Value::Text("KL-07".to_string());
        assert_eq!(
            KeyRange::from_value(exact.label(), &key),
            KeyRange::Exact(key.canonical_bytes())
        );
    }
}
