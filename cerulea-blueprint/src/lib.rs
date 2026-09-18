//! The Cerulea Blueprint.
//!
//! The Blueprint is the single artefact that carries everything a user builds.
//! The visual canvas edits the Blueprint. The Blockchain interprets the
//! Blueprint. Deployment installs the Blueprint.
//!
//! Three properties hold for every Blueprint.
//!
//! 1. Typed. Every element carries a declared type.
//! 2. Versioned. Every Blueprint carries a schema version stamp.
//! 3. Content addressed. The identifier of a Blueprint is the hash of its
//!    canonical encoding.
//!
//! The canonical encoding is the SCALE encoding of the canonical form. JSON
//! export serves the interface layer alone.

#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]

extern crate alloc;

pub mod actions;
pub mod collections;
pub mod expression;
pub mod fold;
pub mod identities;
pub mod permissions;
pub mod rules;
pub mod types;

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

use crate::{
    actions::Action, collections::Collection, identities::Identities, permissions::Permissions,
    rules::Rules, types::TypeDef,
};

/// The schema version of the Blueprint type set carried by this crate.
pub const BLUEPRINT_SCHEMA_VERSION: SchemaVersion = SchemaVersion {
    major: 1,
    minor: 0,
    patch: 0,
};

/// The domain separator mixed into every Blueprint content address.
///
/// The separator keeps Blueprint hashes apart from every other hash the
/// Blockchain computes.
pub const BLUEPRINT_DOMAIN: &[u8] = b"cerulea.blueprint.v1";

/// A three part version number.
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
pub struct SchemaVersion {
    /// The major number. A change here means the encoding changed.
    pub major: u16,
    /// The minor number. A change here means the type set grew.
    pub minor: u16,
    /// The patch number. A change here means a correction landed.
    pub patch: u16,
}

impl SchemaVersion {
    /// Report whether a Blueprint written for `self` decodes under `reader`.
    ///
    /// A reader accepts an artefact with the same major number and a minor
    /// number at or below its own.
    pub fn readable_by(&self, reader: &SchemaVersion) -> bool {
        self.major == reader.major && self.minor <= reader.minor
    }
}

impl core::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The version stamp every Blueprint carries.
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
pub struct VersionStamp {
    /// The version of the Blueprint type set this artefact was written for.
    pub schema: SchemaVersion,
    /// The revision of this deployment. The author raises the revision on
    /// every published change.
    pub revision: u32,
}

impl Default for VersionStamp {
    fn default() -> Self {
        VersionStamp {
            schema: BLUEPRINT_SCHEMA_VERSION,
            revision: 1,
        }
    }
}

/// The descriptive header of a Blueprint.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct BlueprintMeta {
    /// The name the author gave the deployment.
    pub name: String,
    /// One plain sentence stating what the deployment does.
    pub summary: String,
    /// The organisation that owns the deployment.
    pub owner: String,
    /// The version stamp.
    pub version: VersionStamp,
}

impl BlueprintMeta {
    /// Build a header with the current schema version stamped on it.
    pub fn new(name: &str, summary: &str, owner: &str) -> Self {
        BlueprintMeta {
            name: name.to_string(),
            summary: summary.to_string(),
            owner: owner.to_string(),
            version: VersionStamp::default(),
        }
    }
}

/// A Blueprint.
///
/// The header travels with the deployment. Later tasks add the twelve
/// sections beside the header.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Blueprint {
    /// The descriptive header.
    pub meta: BlueprintMeta,
    /// Section one. The declared record, enumeration and sum types.
    pub types: Vec<TypeDef>,
    /// Section two. The collections holding records.
    pub collections: Vec<Collection>,
    /// Section three. The organisation tree, the roles, the credentials and
    /// the delegation rules.
    pub identities: Identities,
    /// Section four. The capability matrix.
    pub permissions: Permissions,
    /// Section five. The invariants and the preconditions.
    pub rules: Rules,
    /// Section six. The actions a caller runs.
    pub actions: Vec<Action>,
}

impl Blueprint {
    /// Build an empty Blueprint carrying the given header.
    pub fn new(meta: BlueprintMeta) -> Self {
        Blueprint {
            meta,
            types: Vec::new(),
            collections: Vec::new(),
            identities: Identities::default(),
            permissions: Permissions::default(),
            rules: Rules::default(),
            actions: Vec::new(),
        }
    }

    /// Check the whole Blueprint and collect every diagnostic.
    ///
    /// A clean result means every section type checks and every reference
    /// resolves.
    pub fn validate(&self) -> Result<(), Report> {
        let mut report = Report::new();
        let root = Path::root();
        self.validate_meta(&root.field("meta"), &mut report);

        // The registry pass and the section pass report the same repeated
        // names. The section pass owns the report, so this copy stays quiet.
        let types_path = root.field("types");
        let (type_registry, _) = crate::types::TypeRegistry::build(&self.types, &types_path);
        report.merge(crate::types::validate_types(&self.types, &types_path));

        let collections_path = root.field("collections");
        let (collection_registry, _) =
            crate::collections::CollectionRegistry::build(&self.collections, &collections_path);
        report.merge(crate::collections::validate_collections(
            &self.collections,
            &type_registry,
            &collections_path,
        ));

        let identities_path = root.field("identities");
        report.merge(self.identities.validate(&identities_path));

        for (position, collection) in self.collections.iter().enumerate() {
            let here = collections_path.index(position);
            for (target, location) in collection.collection_refs(&here) {
                if !collection_registry.contains(&target) {
                    report.push(collection_registry.missing(&target, location));
                }
            }
            for (role, location) in collection.role_refs(&here) {
                if !self.identities.has_role(&role) {
                    report.push(self.identities.missing_role(&role, location));
                }
            }
        }

        report.merge(self.permissions.validate(
            &type_registry,
            &collection_registry,
            &self.identities,
            &root.field("permissions"),
        ));

        report.merge(self.rules.validate(
            &type_registry,
            &collection_registry,
            &root.field("rules"),
        ));

        let actions_path = root.field("actions");
        report.merge(crate::actions::validate_actions(
            &self.actions,
            &type_registry,
            &collection_registry,
            &self.identities,
            &self.rules,
            &actions_path,
        ));

        let action_registry = crate::actions::ActionRegistry::build(&self.actions);
        for (action, location) in self.identities.action_refs(&identities_path) {
            if !action_registry.contains(&action) {
                report.push(action_registry.missing(&action, location));
            }
        }
        for (action, location) in self.permissions.action_refs(&root.field("permissions")) {
            if !action_registry.contains(&action) {
                report.push(action_registry.missing(&action, location));
            }
        }

        report.into_result()
    }

    fn validate_meta(&self, path: &Path, report: &mut Report) {
        if self.meta.name.is_empty() {
            report.push(Diagnostic::new(
                DiagnosticCode::MissingValue,
                path.field("name"),
                "The Blueprint carries an empty name.",
                "Give the deployment a name in the Studio header panel.",
            ));
        }
        if self.meta.name.chars().count() > 128 {
            report.push(Diagnostic::new(
                DiagnosticCode::ValueTooLong,
                path.field("name"),
                "The Blueprint name runs past 128 characters.",
                "Shorten the name to 128 characters or fewer.",
            ));
        }
        if self.meta.summary.is_empty() {
            report.push(Diagnostic::new(
                DiagnosticCode::MissingValue,
                path.field("summary"),
                "The Blueprint carries an empty summary.",
                "Write one plain sentence stating what the deployment does.",
            ));
        }
        if self.meta.version.revision == 0 {
            report.push(Diagnostic::new(
                DiagnosticCode::ValueOutOfRange,
                path.field("version").field("revision"),
                "The revision number is zero.",
                "Set the revision to 1 for a first publication.",
            ));
        }
        if self.meta.version.schema.major != BLUEPRINT_SCHEMA_VERSION.major {
            report.push(Diagnostic::new(
                DiagnosticCode::SchemaVersionMismatch,
                path.field("version").field("schema"),
                "The Blueprint was written for a different major schema version.",
                "Run the Studio migration for this Blueprint before deployment.",
            ));
        }
    }
}

/// Find one cycle in a directed graph, when a cycle is present.
///
/// The map holds one entry per node. The value lists the nodes that entry
/// points at. The result names the nodes on the cycle in order, with the
/// entry node repeated at the end.
pub fn find_cycle(
    edges: &alloc::collections::BTreeMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Mark {
        Fresh,
        Open,
        Done,
    }

    let mut marks: alloc::collections::BTreeMap<&str, Mark> = alloc::collections::BTreeMap::new();
    for (node, targets) in edges.iter() {
        marks.insert(node.as_str(), Mark::Fresh);
        for target in targets {
            marks.entry(target.as_str()).or_insert(Mark::Fresh);
        }
    }
    let starts: Vec<&str> = marks.keys().copied().collect();

    for start in starts {
        if marks.get(start).copied() != Some(Mark::Fresh) {
            continue;
        }
        marks.insert(start, Mark::Open);
        let mut path: Vec<&str> = alloc::vec![start];
        let mut stack: Vec<(&str, usize)> = alloc::vec![(start, 0)];
        while let Some((node, position)) = stack.pop() {
            let children: &[String] = match edges.get(node) {
                Some(list) => list.as_slice(),
                None => &[],
            };
            if position < children.len() {
                stack.push((node, position + 1));
                let child = children[position].as_str();
                match marks.get(child).copied().unwrap_or(Mark::Fresh) {
                    Mark::Open => {
                        let at = path.iter().position(|held| *held == child).unwrap_or(0);
                        let mut cycle: Vec<String> =
                            path[at..].iter().map(|held| held.to_string()).collect();
                        cycle.push(child.to_string());
                        return Some(cycle);
                    }
                    Mark::Fresh => {
                        marks.insert(child, Mark::Open);
                        path.push(child);
                        stack.push((child, 0));
                    }
                    Mark::Done => {}
                }
            } else {
                marks.insert(node, Mark::Done);
                path.pop();
            }
        }
    }
    None
}

/// Write a cycle as an arrow chain such as `a -> b -> a`.
pub fn describe_cycle(cycle: &[String]) -> String {
    cycle.join(" -> ")
}

/// A location inside a Blueprint, written as a path.
///
/// An example reads `actions[3].effects[1].collection`.
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
#[serde(transparent)]
pub struct Location(pub String);

impl Location {
    /// Read the location as text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl core::fmt::Display for Location {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A builder for a [`Location`].
///
/// Each step appends one segment. The result names the exact element a
/// diagnostic refers to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Path(String);

impl Path {
    /// Start at the root of the Blueprint.
    pub fn root() -> Self {
        Path(String::new())
    }

    /// Step into a named field.
    pub fn field(&self, name: &str) -> Self {
        if self.0.is_empty() {
            Path(name.to_string())
        } else {
            Path(format!("{}.{}", self.0, name))
        }
    }

    /// Step into a list position.
    pub fn index(&self, position: usize) -> Self {
        Path(format!("{}[{}]", self.0, position))
    }

    /// Step into a member named by an identifier.
    pub fn key(&self, name: &str) -> Self {
        Path(format!("{}['{}']", self.0, name))
    }

    /// Read the path as text.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Freeze the path into a [`Location`].
    pub fn location(&self) -> Location {
        Location(self.0.clone())
    }
}

impl core::fmt::Display for Path {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The class of a diagnostic.
///
/// The interface layer groups diagnostics by class and shows the fix beside
/// each one.
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
pub enum DiagnosticCode {
    /// A reference names an element that the Blueprint lacks.
    DanglingReference,
    /// Two elements in one section share an identifier.
    DuplicateIdentifier,
    /// An identifier carries a character the Blueprint rejects.
    InvalidIdentifier,
    /// A value carries a type other than the declared one.
    TypeMismatch,
    /// A required value is absent.
    MissingValue,
    /// A value sits outside the permitted range.
    ValueOutOfRange,
    /// A value runs past the permitted length.
    ValueTooLong,
    /// A graph that the Blueprint requires to be acyclic contains a cycle.
    CycleDetected,
    /// A bounded step count ran past its declared limit.
    BoundExceeded,
    /// Arithmetic ran past the range of the integer type.
    ArithmeticOverflow,
    /// A division names a zero divisor.
    DivisionByZero,
    /// An exact decimal result needs more digits than the declared scale.
    PrecisionLoss,
    /// A name appears twice inside one record or one parameter list.
    DuplicateName,
    /// The artefact carries a schema version this crate declines to read.
    SchemaVersionMismatch,
    /// A threshold sits below the floor the transaction class requires.
    ThresholdTooLow,
    /// A structure holds fewer members than it requires.
    SectionEmpty,
}

impl DiagnosticCode {
    /// A short label for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            DiagnosticCode::DanglingReference => "dangling reference",
            DiagnosticCode::DuplicateIdentifier => "duplicate identifier",
            DiagnosticCode::InvalidIdentifier => "invalid identifier",
            DiagnosticCode::TypeMismatch => "type mismatch",
            DiagnosticCode::MissingValue => "missing value",
            DiagnosticCode::ValueOutOfRange => "value out of range",
            DiagnosticCode::ValueTooLong => "value too long",
            DiagnosticCode::CycleDetected => "cycle detected",
            DiagnosticCode::BoundExceeded => "bound exceeded",
            DiagnosticCode::ArithmeticOverflow => "arithmetic overflow",
            DiagnosticCode::DivisionByZero => "division by zero",
            DiagnosticCode::PrecisionLoss => "precision loss",
            DiagnosticCode::DuplicateName => "duplicate name",
            DiagnosticCode::SchemaVersionMismatch => "schema version mismatch",
            DiagnosticCode::ThresholdTooLow => "threshold too low",
            DiagnosticCode::SectionEmpty => "section empty",
        }
    }
}

/// One problem found in a Blueprint.
///
/// Every diagnostic carries the exact location and one plain sentence naming
/// the fix.
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
pub struct Diagnostic {
    /// The class of the problem.
    pub code: DiagnosticCode,
    /// The exact element the problem sits on.
    pub location: Location,
    /// One sentence stating the problem.
    pub problem: String,
    /// One sentence stating the fix.
    pub fix: String,
}

impl Diagnostic {
    /// Build a diagnostic.
    pub fn new(code: DiagnosticCode, path: Path, problem: &str, fix: &str) -> Self {
        Diagnostic {
            code,
            location: path.location(),
            problem: problem.to_string(),
            fix: fix.to_string(),
        }
    }

    /// Build a diagnostic from a location that is already text.
    pub fn at(code: DiagnosticCode, location: Location, problem: String, fix: String) -> Self {
        Diagnostic {
            code,
            location,
            problem,
            fix,
        }
    }
}

impl core::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {} Fix: {}", self.location, self.problem, self.fix)
    }
}

/// Every diagnostic raised by one pass over a Blueprint.
#[derive(
    Clone, Debug, Default, PartialEq, Eq, Encode, Decode, TypeInfo, Serialize, Deserialize,
)]
pub struct Report {
    /// The diagnostics, in the order the pass found them.
    pub diagnostics: Vec<Diagnostic>,
}

impl Report {
    /// Start an empty report.
    pub fn new() -> Self {
        Report {
            diagnostics: Vec::new(),
        }
    }

    /// Add one diagnostic.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.diagnostics.push(diagnostic);
    }

    /// Add one diagnostic when the outcome carries one.
    pub fn absorb(&mut self, outcome: Result<(), Diagnostic>) {
        if let Err(diagnostic) = outcome {
            self.diagnostics.push(diagnostic);
        }
    }

    /// Add every diagnostic from another report.
    pub fn merge(&mut self, other: Report) {
        self.diagnostics.extend(other.diagnostics);
    }

    /// Report whether the pass found zero problems.
    pub fn is_clean(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Count the diagnostics.
    pub fn len(&self) -> usize {
        self.diagnostics.len()
    }

    /// Report whether the report holds zero diagnostics.
    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Find the first diagnostic sitting at the given location.
    pub fn find(&self, location: &str) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .find(|d| d.location.as_str() == location)
    }

    /// Turn the report into a result. A clean report yields success.
    pub fn into_result(self) -> Result<(), Report> {
        if self.is_clean() {
            Ok(())
        } else {
            Err(self)
        }
    }
}

impl core::fmt::Display for Report {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for diagnostic in &self.diagnostics {
            writeln!(f, "{}", diagnostic)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codec::Decode;

    fn sample() -> Blueprint {
        Blueprint::new(BlueprintMeta::new(
            "district land registry",
            "The registry records land parcels and their holders.",
            "Kollam District Administration",
        ))
    }

    #[test]
    fn version_stamp_carries_the_crate_schema_version() {
        let blueprint = sample();
        assert_eq!(blueprint.meta.version.schema, BLUEPRINT_SCHEMA_VERSION);
        assert_eq!(blueprint.meta.version.revision, 1);
    }

    #[test]
    fn a_reader_accepts_an_equal_or_lower_minor_number() {
        let written = SchemaVersion {
            major: 1,
            minor: 0,
            patch: 0,
        };
        let reader = SchemaVersion {
            major: 1,
            minor: 4,
            patch: 2,
        };
        assert!(written.readable_by(&reader));
        assert!(!reader.readable_by(&written));
        let other_major = SchemaVersion {
            major: 2,
            minor: 0,
            patch: 0,
        };
        assert!(!other_major.readable_by(&reader));
    }

    #[test]
    fn a_path_renders_fields_and_positions() {
        let path = Path::root()
            .field("actions")
            .index(3)
            .field("effects")
            .index(1)
            .field("collection");
        assert_eq!(path.as_str(), "actions[3].effects[1].collection");
        let keyed = Path::root()
            .field("collections")
            .key("parcel")
            .field("primary_key");
        assert_eq!(keyed.as_str(), "collections['parcel'].primary_key");
    }

    #[test]
    fn a_complete_header_validates() {
        assert!(sample().validate().is_ok());
    }

    #[test]
    fn an_empty_name_reports_its_location_and_fix() {
        let mut blueprint = sample();
        blueprint.meta.name = String::new();
        let report = blueprint
            .validate()
            .expect_err("an empty name raises a diagnostic");
        let found = report
            .find("meta.name")
            .expect("the diagnostic sits on meta.name");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
        assert!(!found.fix.is_empty());
    }

    #[test]
    fn a_zero_revision_reports_its_location() {
        let mut blueprint = sample();
        blueprint.meta.version.revision = 0;
        let report = blueprint
            .validate()
            .expect_err("a zero revision raises a diagnostic");
        assert!(report.find("meta.version.revision").is_some());
    }

    #[test]
    fn a_foreign_major_version_reports_a_mismatch() {
        let mut blueprint = sample();
        blueprint.meta.version.schema.major = BLUEPRINT_SCHEMA_VERSION.major + 1;
        let report = blueprint
            .validate()
            .expect_err("a foreign major version raises a diagnostic");
        let found = report
            .find("meta.version.schema")
            .expect("the diagnostic sits on the schema");
        assert_eq!(found.code, DiagnosticCode::SchemaVersionMismatch);
    }

    #[test]
    fn the_header_round_trips_through_scale() {
        let blueprint = sample();
        let bytes = blueprint.encode();
        let back = Blueprint::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(blueprint, back);
    }

    #[test]
    fn the_header_round_trips_through_json() {
        let blueprint = sample();
        let text = serde_json::to_string(&blueprint).expect("the header serialises");
        let back: Blueprint = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(blueprint, back);
    }

    #[test]
    fn a_blueprint_reports_a_dangling_collection_reference() {
        use crate::collections::{Collection, PurposeDeclaration};
        let mut blueprint = sample();
        let mut collection =
            Collection::new("parcel", "absent_record", "survey_number", "The register.");
        collection.purpose =
            PurposeDeclaration::new("The register records land.", &["registration"]);
        blueprint.collections.push(collection);
        let report = blueprint
            .validate()
            .expect_err("an absent record type raises a diagnostic");
        let found = report
            .find("collections[0].record_type")
            .expect("the diagnostic sits on the record type");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_blueprint_reports_a_key_holder_role_the_identities_lack() {
        use crate::{
            collections::{Collection, PrivacyClass, PurposeDeclaration},
            types::{FieldDef, RecordDef, RoleId, TypeDef, TypeExpr, TypeId},
        };
        let mut blueprint = sample();
        blueprint.types.push(TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: alloc::vec![FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number."
            )],
            summary: "One land parcel.".to_string(),
        }));
        let mut collection = Collection::new("parcel", "parcel", "survey_number", "The register.");
        collection.purpose =
            PurposeDeclaration::new("The register records land.", &["registration"]);
        collection.privacy = PrivacyClass::Encrypted {
            key_holders: alloc::vec![RoleId::new("absent_role")],
        };
        blueprint.collections.push(collection);
        let report = blueprint
            .validate()
            .expect_err("an absent role raises a diagnostic");
        let found = report
            .find("collections[0].privacy.key_holders[0]")
            .expect("the diagnostic sits on the key holder");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn find_cycle_reports_the_loop_it_walks() {
        let mut edges: alloc::collections::BTreeMap<String, Vec<String>> =
            alloc::collections::BTreeMap::new();
        edges.insert("a".to_string(), alloc::vec!["b".to_string()]);
        edges.insert("b".to_string(), alloc::vec!["c".to_string()]);
        edges.insert("c".to_string(), alloc::vec!["a".to_string()]);
        let cycle = find_cycle(&edges).expect("the loop is found");
        assert_eq!(describe_cycle(&cycle), "a -> b -> c -> a");

        let mut straight: alloc::collections::BTreeMap<String, Vec<String>> =
            alloc::collections::BTreeMap::new();
        straight.insert("a".to_string(), alloc::vec!["b".to_string()]);
        straight.insert("b".to_string(), alloc::vec!["c".to_string()]);
        straight.insert("c".to_string(), Vec::new());
        assert!(find_cycle(&straight).is_none());
    }

    #[test]
    fn a_diagnostic_prints_its_location_and_fix() {
        let diagnostic = Diagnostic::new(
            DiagnosticCode::DanglingReference,
            Path::root()
                .field("actions")
                .index(3)
                .field("effects")
                .index(1)
                .field("collection"),
            "The effect names a collection the Blueprint lacks.",
            "Add the collection or point the effect at an existing collection.",
        );
        let text = alloc::format!("{}", diagnostic);
        assert!(text.starts_with("actions[3].effects[1].collection: "));
        assert!(text.contains("Fix: "));
    }
}
