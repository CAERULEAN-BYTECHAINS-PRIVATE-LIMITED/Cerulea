//! Every schema version transition, forward and back, against real fragments.
//!
//! The content repository holds the module fragments Cerulea installs. Each
//! fragment declares record types. This file reads those fragments from disk,
//! registers each record type as a real collection, writes a real record into
//! it, then carries the collection forward one version and back again.
//!
//! Three claims carry this file.
//!
//! 1. **Forward works.** A plan that adds a field applies, the collection
//!    version rises and every record reads back carrying the new field at its
//!    default.
//! 2. **Back works.** The inverse plan applies, the version returns and every
//!    record reads back the way it read before the transition.
//! 3. **A breaking transition asks for governance.** A plan that removes a
//!    field carries the structural threshold, so no deployment loses a field
//!    without the electorate.
//!
//! The fragments sit beside the chain repository. A checkout holding no
//! content directory reports that and stops, so a silent pass never happens.

use crate::{mock::*, Error, PlanStatus, Plans};
use cerulea_blueprint::{
    collections::Collection,
    types::{
        CollectionId, Decimal, FieldDef, FieldName, FieldPath, Hash32, Identity, OptionalValue,
        RecordDef, Timestamp, TypeDef, TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_ok, BoundedVec};
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationStep},
    primitives::{name_of, DeploymentId, RecordKey},
};
use pallet_cerulea_state::RecordStore;

extern crate alloc;
use alloc::{string::ToString, vec::Vec};
use std::path::PathBuf;

const DEPLOYMENT: DeploymentId = [21u8; 32];
const STEWARD: u64 = 1;

/// The directories holding the module fragments.
fn fragment_directories() -> Vec<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = match here.parent().and_then(|one| one.parent()).and_then(|two| two.parent()) {
        Some(found) => found.to_path_buf(),
        None => return Vec::new(),
    };
    [
        root.join("content").join("modules"),
        root.join("content").join("government").join("modules"),
    ]
    .into_iter()
    .filter(|path| path.is_dir())
    .collect()
}

/// Every fragment file, in name order.
fn fragment_files() -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    for directory in fragment_directories() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|end| end == "json").unwrap_or(false) {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// One record type taken from one fragment.
struct FragmentType {
    fragment: String,
    types: Vec<TypeDef>,
    record: RecordDef,
    /// The collection the fragment declares over this record type.
    collection: Collection,
}

/// Read every record type out of every fragment.
///
/// A record type carrying no field takes no migration, so the reader leaves it
/// out. A record type carrying a field named `ward` already holds the field the
/// forward plan adds, so the reader leaves that one out as well.
fn fragment_types() -> Vec<FragmentType> {
    let mut found: Vec<FragmentType> = Vec::new();
    for file in fragment_files() {
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(_) => continue,
        };
        let raw: serde_json::Value = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let list = match raw.get("types").and_then(|value| value.as_array()) {
            Some(list) => list,
            None => continue,
        };
        let name = file
            .file_name()
            .map(|piece| piece.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut declared: Vec<TypeDef> = Vec::new();
        for entry in list.iter() {
            if let Ok(decoded) = serde_json::from_value::<TypeDef>(entry.clone()) {
                declared.push(decoded);
            }
        }
        // The fragment declares its own collections. The transition carries
        // those rather than a collection the test invents, so the registry
        // reads the shape a deployment really installs.
        let mut collections: Vec<Collection> = Vec::new();
        for entry in raw
            .get("collections")
            .and_then(|value| value.as_array())
            .unwrap_or(&Vec::new())
            .iter()
        {
            if let Ok(decoded) = serde_json::from_value::<Collection>(entry.clone()) {
                collections.push(decoded);
            }
        }
        for decoded in declared.iter() {
            if let TypeDef::Record(record) = decoded.clone() {
                if record.fields.is_empty() {
                    continue;
                }
                if record
                    .fields
                    .iter()
                    .any(|field| field.name == FieldName::new("ward"))
                {
                    continue;
                }
                let collection = collections
                    .iter()
                    .find(|entry| entry.record_type.as_str() == record.id.as_str())
                    .cloned();
                let collection = match collection {
                    Some(held) => held,
                    None => continue,
                };
                found.push(FragmentType {
                    fragment: name.clone(),
                    types: declared.clone(),
                    record,
                    collection,
                });
            }
        }
    }
    found
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

/// The first field of a record, which the collection keys on.
fn key_field(record: &RecordDef) -> FieldDef {
    record.fields.first().cloned().expect("the record holds a field")
}

/// A value of the stated type, drawn from the types the fragment declares.
///
/// The generator walks the whole type language, because the catalogue uses
/// the whole type language. `depth` stops a record type that holds itself,
/// directly or through a list, from recursing without end: past the limit an
/// optional answers absent and a list answers empty, both of which the
/// checker accepts for any element type.
fn value_of(ty: &TypeExpr, declared: &[TypeDef], seed: usize, depth: usize) -> Option<Value> {
    match ty {
        TypeExpr::Bool => Some(Value::Bool(seed % 2 == 0)),
        TypeExpr::Integer => Some(Value::Integer(seed as i128 + 1)),
        TypeExpr::Decimal { scale } => Some(Value::Decimal(Decimal {
            units: seed as i128 + 1,
            scale: *scale,
        })),
        TypeExpr::Text => Some(Value::Text(alloc::format!("value-{seed}"))),
        TypeExpr::Bytes => Some(Value::Bytes(alloc::vec![seed as u8; 4])),
        TypeExpr::Timestamp => Some(Value::Timestamp(Timestamp::from_millis(
            1_700_000_000_000 + seed as u64,
        ))),
        TypeExpr::Identity => Some(Value::Identity(Identity([seed as u8; 32]))),
        TypeExpr::Hash => Some(Value::Hash(Hash32::of(&[seed as u8]))),
        TypeExpr::List(element) => {
            if depth >= DEPTH_LIMIT {
                return Some(Value::list(element.as_ref().clone(), Vec::new()));
            }
            let item = value_of(element, declared, seed, depth + 1)?;
            Some(Value::list(element.as_ref().clone(), alloc::vec![item]))
        }
        TypeExpr::Map { key, value } => {
            if depth >= DEPTH_LIMIT {
                return Some(Value::map(
                    key.as_ref().clone(),
                    value.as_ref().clone(),
                    Vec::new(),
                ));
            }
            let held_key = value_of(key, declared, seed, depth + 1)?;
            let held_value = value_of(value, declared, seed + 1, depth + 1)?;
            Some(Value::map(
                key.as_ref().clone(),
                value.as_ref().clone(),
                alloc::vec![(held_key, held_value)],
            ))
        }
        TypeExpr::Optional(element) => {
            let item = if depth >= DEPTH_LIMIT {
                None
            } else {
                value_of(element, declared, seed, depth + 1).map(alloc::boxed::Box::new)
            };
            Some(Value::Optional(OptionalValue {
                element: element.as_ref().clone(),
                item,
            }))
        }
        TypeExpr::Record(type_id) => {
            if depth >= DEPTH_LIMIT {
                return None;
            }
            let held = declared.iter().find_map(|entry| match entry {
                TypeDef::Record(record) if &record.id == type_id => Some(record),
                _ => None,
            })?;
            record_of(held, declared, seed, depth + 1)
        }
        TypeExpr::Enumeration(type_id) => {
            let held = declared.iter().find_map(|entry| match entry {
                TypeDef::Enumeration(enumeration) if &enumeration.id == type_id => {
                    Some(enumeration)
                }
                _ => None,
            })?;
            let variant = held.variants.first()?.clone();
            Some(Value::enumeration(type_id.clone(), variant))
        }
        TypeExpr::Sum(type_id) => {
            let held = declared.iter().find_map(|entry| match entry {
                TypeDef::Sum(sum) if &sum.id == type_id => Some(sum),
                _ => None,
            })?;
            let variant = held.variants.first()?;
            let payload = match &variant.payload {
                None => None,
                Some(declared_payload) => {
                    if depth >= DEPTH_LIMIT {
                        return None;
                    }
                    Some(value_of(declared_payload, declared, seed, depth + 1)?)
                }
            };
            Some(Value::sum(type_id.clone(), variant.name.clone(), payload))
        }
        // A pointer carries the collection it points into and a key. The key
        // stands for a record the test does not install, and the checker asks
        // only that the collection match.
        TypeExpr::Reference(collection) => Some(Value::reference(
            collection.clone(),
            Value::Text(alloc::format!("pointer-{seed}")),
        )),
    }
}

/// How deep the generator walks before it stops on a type that holds itself.
const DEPTH_LIMIT: usize = 6;

/// A record value of the stated record type.
fn record_of(
    record: &RecordDef,
    declared: &[TypeDef],
    seed: usize,
    depth: usize,
) -> Option<Value> {
    let mut fields = Vec::new();
    for (position, field) in record.fields.iter().enumerate() {
        let held = value_of(&field.ty, declared, seed + position, depth)?;
        fields.push((FieldName::new(field.name.as_str()), held));
    }
    let mut value = Value::record(TypeId::new(record.id.as_str()), fields);
    value.canonicalise();
    Some(value)
}

/// One record of the stated type, or nothing where the fragment leans on a
/// type it does not itself declare.
fn record_value(record: &RecordDef, declared: &[TypeDef], seed: usize) -> Option<Value> {
    record_of(record, declared, seed, 0)
}

fn record_key_of(record: &RecordDef, declared: &[TypeDef], seed: usize) -> Option<RecordKey> {
    let key = key_field(record);
    let mut value = value_of(&key.ty, declared, seed, 0)?;
    value.canonicalise();
    Some(BoundedVec::try_from(value.canonical_bytes()).expect("the key fits"))
}

/// The plan that carries a collection forward one version.
fn forward_plan(record: &RecordDef) -> MigrationPlan {
    MigrationPlan::new(
        record.id.as_str(),
        1,
        "The collection gains the ward each record sits in.",
    )
    .with(MigrationStep::AddField {
        field: FieldName::new("ward"),
        ty: TypeExpr::Text,
        default: Value::Text("unassigned".to_string()),
        summary: "The ward the record sits in.".to_string(),
    })
}

/// Install one record type, its collection and one record.
///
/// The call answers with whether the registry took the collection and the
/// record. A fragment declaring a shape the registry refuses in isolation
/// answers false, and the caller counts it.
fn install(entry: &FragmentType, seed: usize) -> bool {
    install_reported(entry, seed).is_none()
}

/// The reason a fragment stands outside this test: its record leans on a type
/// another fragment declares, and the test installs one fragment at a time.
const UNDECLARED: &str = "the record leans on a type the fragment does not declare";

/// Install and answer with the reason the registry refused, where it did.
fn install_reported(entry: &FragmentType, seed: usize) -> Option<alloc::string::String> {
    let record = &entry.record;
    // Every type the fragment declares registers first, so a record referring
    // to another type of the same fragment resolves.
    for declared in entry.types.iter() {
        let _ = Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(declared.encode()),
        );
    }
    let _ = Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(TypeDef::Record(record.clone()).encode()),
    );
    if let Err(cause) = Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(entry.collection.encode()),
    ) {
        return Some(alloc::format!("collection {:?}", cause));
    }
    let body = match record_value(record, &entry.types, seed) {
        Some(found) => found,
        None => return Some(UNDECLARED.to_string()),
    };
    match State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name_of(record.id.as_str()).expect("the name fits"),
        bounded(body.canonical_bytes()),
    ) {
        Ok(()) => None,
        Err(cause) => Some(alloc::format!("record {cause:?}")),
    }
}

/// The registry takes every record type the catalogue declares in full.
///
/// A record leaning on a type another fragment declares stands outside this
/// test, because the test installs one fragment at a time. Every other
/// refusal is a defect, and the test names it.
#[test]
fn the_registry_takes_every_self_contained_record_type() {
    let types = fragment_types();
    let mut refusals: Vec<alloc::string::String> = Vec::new();
    let mut outside = 0usize;
    let mut taken = 0usize;
    for (seed, entry) in types.iter().enumerate() {
        new_test_ext().execute_with(|| match install_reported(entry, seed) {
            None => taken += 1,
            Some(reason) if reason == UNDECLARED => outside += 1,
            Some(reason) => {
                if refusals.len() < 12 {
                    refusals.push(alloc::format!("{}: {}", entry.fragment, reason));
                }
            }
        });
    }
    assert!(
        refusals.is_empty(),
        "the registry refused {} record types the catalogue declares. First: {:?}",
        refusals.len(),
        refusals
    );
    std::println!(
        "the registry took {taken} of {} record types; {outside} lean on a type another fragment declares",
        types.len()
    );
    assert!(
        taken * 2 > types.len(),
        "the registry took {taken} of {} record types, and {outside} lean on a type          another fragment declares, which leaves too few to stand for the catalogue",
        types.len()
    );
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

fn field_of(record: &Value, name: &str) -> Option<Value> {
    pallet_cerulea_state::value_at(record, &FieldPath::parse(name))
}

#[test]
fn the_content_repository_holds_fragments_to_read() {
    let files = fragment_files();
    assert!(
        !files.is_empty(),
        "the transition test found no fragment under content/modules. \
         Fix: clone the content repository beside the chain repository."
    );
    let types = fragment_types();
    assert!(
        types.len() >= 50,
        "the transition test read {} record types from {} fragments, which is too few to stand for the catalogue",
        types.len(),
        files.len()
    );
}

#[test]
fn every_fragment_carries_its_collection_forward_and_back() {
    let types = fragment_types();
    assert!(!types.is_empty(), "the reader found record types to carry");

    let mut carried = 0usize;
    let mut refused: Vec<String> = Vec::new();
    for (seed, entry) in types.iter().enumerate() {
        new_test_ext().execute_with(|| {
            if !install(entry, seed) {
                refused.push(entry.fragment.clone());
                return;
            }
            let before = State::read(
                &DEPLOYMENT,
                &CollectionId::new(entry.record.id.as_str()),
                &record_key_of(&entry.record, &entry.types, seed).expect("the key stands"),
            )
            .expect("the store holds the record before the transition");

            // --- Forward ---
            let body = forward_plan(&entry.record);
            let id = plan(&body, 100);
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
            let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the register holds the plan");
            assert_eq!(
                held.status,
                PlanStatus::Verified,
                "{} left the plan at {:?}",
                entry.fragment,
                held.status
            );

            let after = State::read(
                &DEPLOYMENT,
                &CollectionId::new(entry.record.id.as_str()),
                &record_key_of(&entry.record, &entry.types, seed).expect("the key stands"),
            )
            .expect("the store holds the record after the transition");
            assert_eq!(
                field_of(&after, "ward"),
                Some(Value::Text("unassigned".to_string())),
                "{} carried no ward after the forward transition",
                entry.fragment
            );

            // --- Back ---
            assert_ok!(Evolution::roll_back(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                id
            ));
            let returned = State::read(
                &DEPLOYMENT,
                &CollectionId::new(entry.record.id.as_str()),
                &record_key_of(&entry.record, &entry.types, seed).expect("the key stands"),
            )
            .expect("the store holds the record after the rollback");
            assert_eq!(
                field_of(&returned, "ward"),
                None,
                "{} kept the ward after the rollback",
                entry.fragment
            );
            assert_eq!(
                returned, before,
                "{} read back differently after the rollback",
                entry.fragment
            );
            carried += 1;
        });
    }
    let installable = types.len() - refused.len();
    assert_eq!(
        carried, installable,
        "every record type the registry took made the round trip. Refused: {:?}",
        refused
    );
    assert!(
        carried >= 50,
        "the transition carried {} record types forward and back, which is too few to stand for the catalogue. Refused: {:?}",
        carried,
        refused
    );
}

#[test]
fn a_transition_that_removes_a_field_asks_for_the_electorate() {
    // The field the collection keys on cannot go, because the collection
    // would lose the way it finds a record. The plan drops a plain field, so
    // the refusal comes from the electorate and not from the key.
    let (seed, entry) = fragment_types()
        .into_iter()
        .enumerate()
        .find(|(seed, entry)| {
            entry.record.fields.len() > 1
                && new_test_ext().execute_with(|| install(entry, *seed))
        })
        .expect("the catalogue holds a record type carrying a plain field");
    new_test_ext().execute_with(|| {
        assert!(install(&entry, seed), "the record type installs");
        let plain = entry.record.fields[1].clone();
        let removing = MigrationPlan::new(
            entry.record.id.as_str(),
            1,
            "The collection drops a field an integration reads.",
        )
        .with(MigrationStep::RemoveField {
            field: FieldName::new(plain.name.as_str()),
        });
        let id = plan(&removing, 100);
        let held = Plans::<Test>::get(DEPLOYMENT, id).expect("the register holds the plan");
        assert!(
            crate::plan_breaks(&removing),
            "a plan removing a field reads as breaking"
        );
        assert_eq!(held.status, PlanStatus::Planned);
        // A plan reaches the electorate only after the dry run, so the first
        // refusal names the dry run and the second names the approval.
        assert_eq!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id).unwrap_err(),
            Error::<Test>::DryRunPending.into(),
            "the refusal names the dry run the plan waits on"
        );
        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Evolution::apply(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id).unwrap_err(),
            Error::<Test>::ApprovalMissing.into(),
            "the refusal names the approval the plan waits on"
        );
    });
}
