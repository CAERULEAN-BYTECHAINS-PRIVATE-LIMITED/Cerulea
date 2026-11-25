//! Behavioural tests for collection definitions and key ranges.
//!
//! Each test starts from a clean collection, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    collections::{
        validate_collections, Collection, CollectionRegistry, Index, KeyRange, KeySelector,
        PrivacyClass, PurposeDeclaration, RetentionAction, RetentionPeriod,
    },
    expression::{Env, Expr, Meter},
    types::{
        CollectionId, FieldDef, FieldPath, IndexName, PurposeTag, RecordDef, RoleId, TypeDef,
        TypeExpr, TypeId, TypeRegistry, Value,
    },
    DiagnosticCode, Location, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("document"),
        fields: vec![
            FieldDef::new("number", TypeExpr::Text, "The document number."),
            FieldDef::new("digest", TypeExpr::Hash, "The digest."),
            FieldDef::new("address", TypeExpr::Text, "The store address."),
            FieldDef::new("pages", TypeExpr::Integer, "The page count."),
            FieldDef::new(
                "note",
                TypeExpr::optional(TypeExpr::Text),
                "An optional note.",
            ),
        ],
        summary: "One document.".to_string(),
    })]
}

fn clean() -> Collection {
    let mut documents = Collection::new("documents", "document", "number", "The documents.");
    documents.indexes = vec![Index {
        name: IndexName::new("by_pages"),
        fields: vec![FieldPath::parse("pages")],
        unique: false,
    }];
    documents.privacy = PrivacyClass::External {
        commitment: FieldPath::parse("digest"),
        locator: FieldPath::parse("address"),
    };
    documents.retention.period = RetentionPeriod::Milliseconds(86_400_000);
    documents.retention.on_expiry = RetentionAction::Anonymise(vec![FieldPath::parse("note")]);
    documents.purpose = PurposeDeclaration::new("The office keeps documents.", &["filing"]);
    documents
}

fn report(collections: &[Collection]) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    validate_collections(collections, &registry, &Path::root().field("collections"))
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_collection_passes_and_names_its_parts() {
    let documents = clean();
    assert!(report(&[documents.clone()]).is_clean());
    let labels: Vec<&str> = [
        PrivacyClass::Open,
        PrivacyClass::HashOnly,
        PrivacyClass::Encrypted {
            key_holders: Vec::new(),
        },
        documents.privacy.clone(),
    ]
    .iter()
    .map(|class| class.label())
    .collect();
    assert_eq!(
        labels,
        ["open", "hash only", "encrypted payload", "external payload"]
    );
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    assert_eq!(documents.key_type(&registry), Some(TypeExpr::Text));
    assert!(documents.role_refs(&Path::root()).is_empty());
    assert!(documents.collection_refs(&Path::root()).is_empty());
    let purpose = &documents.purpose;
    assert!(purpose.admits(&PurposeTag::new("filing")));
    assert!(!purpose.admits(&PurposeTag::new("marketing")));
}

#[test]
fn a_collection_lists_the_roles_and_collections_it_names() {
    let mut documents = clean();
    documents.privacy = PrivacyClass::Encrypted {
        key_holders: vec![RoleId::new("clerk"), RoleId::new("auditor")],
    };
    documents.retention.on_expiry = RetentionAction::MoveTo(CollectionId::new("archive"));
    let path = Path::root().field("collections").index(0);
    let refs = documents.role_refs(&path);
    let roles: Vec<(&str, &str)> = refs
        .iter()
        .map(|(role, location)| (role.as_str(), location.as_str()))
        .collect();
    assert_eq!(
        roles,
        [
            ("clerk", "collections[0].privacy.key_holders[0]"),
            ("auditor", "collections[0].privacy.key_holders[1]"),
        ]
    );
    let targets = documents.collection_refs(&path);
    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].0, CollectionId::new("archive"));
    assert_eq!(targets[0].1.as_str(), "collections[0].retention.on_expiry");
}

#[test]
fn the_registry_answers_lookups_and_names_the_collections_on_offer() {
    let held = vec![
        clean(),
        Collection::new("archive", "document", "number", "Old."),
    ];
    let (registry, report) = CollectionRegistry::build(&held, &Path::root());
    assert!(report.is_clean());
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert_eq!(registry.names(), ["archive", "documents"]);
    assert!(registry.get(&CollectionId::new("archive")).is_some());
    let none: Vec<Collection> = Vec::new();
    let (empty, _) = CollectionRegistry::build(&none, &Path::root());
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&CollectionId::new("ghost"), Location::default())
            .fix,
        "Declare a collection named `ghost` in the collections section."
    );
}

#[test]
fn key_selectors_resolve_into_ranges_that_cover_one_another() {
    let exact = KeySelector::Exact(Expr::text("KL-1"));
    let prefix = KeySelector::Prefix(Expr::text("KL"));
    assert_eq!(exact.label(), "exact");
    assert_eq!(prefix.label(), "prefix");
    assert_eq!(KeySelector::Full.label(), "full");
    assert_eq!(KeySelector::Full.expression(), None);
    assert_eq!(prefix.expression(), Some(&Expr::text("KL")));
    let env = Env::default();
    let mut meter = Meter::default();
    let exact_range = exact
        .resolve(&env, &mut meter, &Path::root())
        .expect("the key resolves");
    let prefix_range = prefix
        .resolve(&env, &mut meter, &Path::root())
        .expect("the prefix resolves");
    assert_eq!(
        exact_range,
        KeyRange::Exact(Value::Text("KL-1".to_string()).canonical_bytes())
    );
    assert_eq!(
        prefix_range,
        KeyRange::Prefix(Value::Text("KL".to_string()).canonical_bytes())
    );
    assert_eq!(
        KeySelector::Full.resolve(&env, &mut meter, &Path::root()),
        Ok(KeyRange::Full)
    );
    let fault = KeySelector::Prefix(Expr::bound("absent"))
        .resolve(&env, &mut meter, &Path::root())
        .expect_err("the prefix reads an absent name");
    assert_eq!(fault.code, DiagnosticCode::DanglingReference);

    let key = Value::Integer(7);
    assert_eq!(
        KeyRange::from_value("exact", &key),
        KeyRange::Exact(key.canonical_bytes())
    );
    assert_eq!(
        KeyRange::from_value("prefix", &key),
        KeyRange::Prefix(key.canonical_bytes())
    );
    assert_eq!(KeyRange::from_value("full", &key), KeyRange::Full);
    assert_eq!(KeyRange::Exact(vec![1]).label(), "exact");
    assert_eq!(KeyRange::Prefix(vec![1]).label(), "prefix");
    assert_eq!(KeyRange::Full.label(), "full");

    let wide = KeyRange::Prefix(vec![1, 2]);
    assert!(KeyRange::Full.covers(&wide));
    assert!(wide.covers(&KeyRange::Exact(vec![1, 2, 3])));
    assert!(!wide.covers(&KeyRange::Exact(vec![1, 3])));
    assert!(wide.covers(&KeyRange::Prefix(vec![1, 2, 9])));
    assert!(!wide.covers(&KeyRange::Prefix(vec![1])));
    assert!(KeyRange::Exact(vec![4]).covers(&KeyRange::Exact(vec![4])));
    assert!(!KeyRange::Exact(vec![4]).covers(&KeyRange::Exact(vec![5])));
    assert!(!KeyRange::Exact(vec![4]).covers(&KeyRange::Full));
    assert!(!wide.covers(&KeyRange::Full));
}

#[test]
fn header_and_key_faults_each_raise_a_diagnostic() {
    let mut first = clean();
    first.id = CollectionId::new("Documents");
    first.record_type = TypeId::new("Ghost");
    let mut second = clean();
    second.primary_key = FieldPath::default();
    let mut third = clean();
    third.primary_key = FieldPath::parse("note");
    let mut fourth = clean();
    fourth.primary_key = FieldPath::parse("title");
    let mut fifth = clean();
    fifth.id = CollectionId::new("documents");
    let found = report(&[first, second, third, fourth, fifth]);
    assert_eq!(
        at(&found, "collections[0].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "collections[0].record_type").code,
        DiagnosticCode::InvalidIdentifier
    );
    let lacks: Vec<&str> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "collections[0].record_type")
        .map(|d| d.fix.as_str())
        .collect();
    assert_eq!(
        lacks[1],
        "Declare a record type named `Ghost` in the types section."
    );
    assert_eq!(
        at(&found, "collections[1].primary_key").code,
        DiagnosticCode::MissingValue
    );
    let unordered = at(&found, "collections[2].primary_key");
    assert_eq!(unordered.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        unordered.problem,
        "The primary key of `documents` carries an optional text, which lacks a total order."
    );
    assert_eq!(
        at(&found, "collections[3].primary_key").fix,
        "Name a field declared on the record type `document`."
    );
    assert_eq!(
        at(&found, "collections[4].id").fix,
        "Rename one of the two collections away from `documents`."
    );
}

#[test]
fn index_faults_each_raise_a_diagnostic() {
    let mut documents = clean();
    documents.indexes.push(Index {
        name: IndexName::new("by_pages"),
        fields: Vec::new(),
        unique: true,
    });
    documents.indexes.push(Index {
        name: IndexName::new("Bad"),
        fields: vec![FieldPath::default(), FieldPath::parse("title")],
        unique: false,
    });
    let found = report(&[documents]);
    let base = "collections[0].indexes";
    assert_eq!(
        at(&found, &alloc::format!("{base}[1].name")).fix,
        "Rename one of the two `by_pages` indexes."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}[1].fields")).fix,
        "Name at least one field for the index to sort by."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}[2].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}[2].fields[0]")).code,
        DiagnosticCode::MissingValue
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}[2].fields[1]")).fix,
        "Name a field declared on the record type `document`."
    );
}

#[test]
fn privacy_faults_each_raise_a_diagnostic() {
    let mut empty_keys = clean();
    empty_keys.privacy = PrivacyClass::Encrypted {
        key_holders: Vec::new(),
    };
    let mut bad_key = clean();
    bad_key.id = CollectionId::new("second");
    bad_key.privacy = PrivacyClass::Encrypted {
        key_holders: vec![RoleId::new("Clerk")],
    };
    let mut swapped = clean();
    swapped.id = CollectionId::new("third");
    swapped.privacy = PrivacyClass::External {
        commitment: FieldPath::parse("address"),
        locator: FieldPath::parse("digest"),
    };
    let mut missing = clean();
    missing.id = CollectionId::new("fourth");
    missing.privacy = PrivacyClass::External {
        commitment: FieldPath::parse("seal"),
        locator: FieldPath::default(),
    };
    let found = report(&[empty_keys, bad_key, swapped, missing]);
    assert_eq!(
        at(&found, "collections[0].privacy.key_holders").fix,
        "Name at least one role that receives the key."
    );
    assert_eq!(
        at(&found, "collections[1].privacy.key_holders[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    let commitment = at(&found, "collections[2].privacy.commitment");
    assert_eq!(
        commitment.problem,
        "The commitment field of `third` carries text."
    );
    assert_eq!(commitment.fix, "Give the commitment field a hash.");
    assert_eq!(
        at(&found, "collections[2].privacy.locator").fix,
        "Give the locator field text."
    );
    assert_eq!(
        at(&found, "collections[3].privacy.commitment").fix,
        "Name a field declared on the record type `document`."
    );
    assert_eq!(
        at(&found, "collections[3].privacy.locator").code,
        DiagnosticCode::MissingValue
    );
}

#[test]
fn purpose_and_retention_faults_each_raise_a_diagnostic() {
    let mut documents = clean();
    documents.purpose = PurposeDeclaration::new("", &[]);
    documents.retention.period = RetentionPeriod::Milliseconds(0);
    documents.retention.on_expiry = RetentionAction::Anonymise(Vec::new());
    let mut tagged = clean();
    tagged.id = CollectionId::new("tagged");
    tagged.purpose = PurposeDeclaration::new("The office keeps documents.", &["Filing"]);
    tagged.retention.on_expiry =
        RetentionAction::Anonymise(vec![FieldPath::default(), FieldPath::parse("title")]);
    let found = report(&[documents, tagged]);
    assert_eq!(
        at(&found, "collections[0].purpose.statement").fix,
        "Write one plain sentence stating why the deployment holds this data."
    );
    assert_eq!(
        at(&found, "collections[0].purpose.purposes").fix,
        "Declare at least one purpose so a read passes the purpose check."
    );
    assert_eq!(
        at(&found, "collections[0].retention.period").fix,
        "Set a span above zero, or choose indefinite retention."
    );
    assert_eq!(
        at(&found, "collections[0].retention.on_expiry").fix,
        "Name the fields the Blockchain clears at expiry."
    );
    assert_eq!(
        at(&found, "collections[1].purpose.purposes[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "collections[1].retention.on_expiry[0]").code,
        DiagnosticCode::MissingValue
    );
    assert_eq!(
        at(&found, "collections[1].retention.on_expiry[1]").problem,
        "The anonymise action names the field `title`, which `document` lacks."
    );
}
