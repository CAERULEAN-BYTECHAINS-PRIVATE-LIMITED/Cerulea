//! The module catalogue fragments, checked by the Rust validator.
//!
//! Every JSON file under `content/modules` is one module fragment. The test
//! reads each one, decodes it into the Blueprint type set, merges it with the
//! fragments of the modules it requires, and runs `validate()` over the
//! result. The Rust validator is the final gate before a fragment installs.
//!
//! The content repository sits beside the chain repository. A checkout
//! holding no content directory skips the test and says so.

#![cfg(feature = "std")]

use cerulea_blueprint::{
    actions::Action,
    collections::Collection,
    compose::Block,
    events::EventDef,
    identities::{CredentialDef, DelegationRule, OrganisationUnit, RoleDef},
    permissions::CapabilityEntry,
    rules::{Invariant, Precondition},
    types::TypeDef,
    views::View,
    workflows::Workflow,
    Blueprint,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

/// The directory holding the module fragments.
fn module_dir() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here.parent()?.parent()?;
    let modules = root.join("content").join("modules");
    if modules.is_dir() {
        Some(modules)
    } else {
        None
    }
}

/// Every fragment file, in name order.
fn fragment_files(directory: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(directory)
        .expect("the module directory opens")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().map(|end| end == "json").unwrap_or(false))
        .collect();
    found.sort();
    found
}

/// The modules one fragment requires, named by fragment file.
fn required_files(raw: &serde_json::Value) -> Vec<String> {
    raw.get("dependencies")
        .and_then(|value| value.as_array())
        .map(|list| {
            list.iter()
                .filter(|entry| {
                    entry.get("kind").and_then(|kind| kind.as_str()) == Some("requires")
                })
                .filter_map(|entry| {
                    entry
                        .get("fragment")
                        .and_then(|name| name.as_str())
                        .map(|name| name.to_string())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Add the members of `extra` that carry a name `held` lacks.
fn absorb<T, K: Ord, F: Fn(&T) -> K>(held: &mut Vec<T>, extra: &[T], key: F)
where
    T: Clone,
{
    let present: BTreeSet<K> = held.iter().map(&key).collect();
    for item in extra {
        if !present.contains(&key(item)) {
            held.push(item.clone());
        }
    }
}

fn entry_signature(entry: &CapabilityEntry) -> String {
    format!("{}|{}|{:?}", entry.role, entry.collection, entry.capability)
}

/// Merge one dependency fragment into the fragment under test.
///
/// The fragment under test owns every name it declares. The dependency adds
/// the names the fragment lacks. Every module carries its own name prefix, so
/// the two sets stay apart in practice.
fn merge(into: &mut Blueprint, from: &Blueprint) {
    absorb(&mut into.types, &from.types, |def: &TypeDef| {
        def.id().as_str().to_string()
    });
    absorb(
        &mut into.collections,
        &from.collections,
        |collection: &Collection| collection.id.as_str().to_string(),
    );
    absorb(
        &mut into.identities.units,
        &from.identities.units,
        |unit: &OrganisationUnit| unit.id.as_str().to_string(),
    );
    absorb(
        &mut into.identities.roles,
        &from.identities.roles,
        |role: &RoleDef| role.id.as_str().to_string(),
    );
    absorb(
        &mut into.identities.credentials,
        &from.identities.credentials,
        |credential: &CredentialDef| credential.id.as_str().to_string(),
    );
    absorb(
        &mut into.identities.delegations,
        &from.identities.delegations,
        |rule: &DelegationRule| rule.id.as_str().to_string(),
    );
    absorb(
        &mut into.permissions.entries,
        &from.permissions.entries,
        entry_signature,
    );
    absorb(
        &mut into.rules.invariants,
        &from.rules.invariants,
        |rule: &Invariant| rule.id.as_str().to_string(),
    );
    absorb(
        &mut into.rules.preconditions,
        &from.rules.preconditions,
        |rule: &Precondition| rule.id.as_str().to_string(),
    );
    absorb(&mut into.actions, &from.actions, |action: &Action| {
        action.id.as_str().to_string()
    });
    absorb(
        &mut into.workflows,
        &from.workflows,
        |workflow: &Workflow| workflow.id.as_str().to_string(),
    );
    absorb(&mut into.events, &from.events, |event: &EventDef| {
        event.id.as_str().to_string()
    });
    absorb(&mut into.views, &from.views, |view: &View| {
        view.id.as_str().to_string()
    });
    absorb(
        &mut into.integrations.feeds,
        &from.integrations.feeds,
        |feed| feed.id.as_str().to_string(),
    );
    absorb(
        &mut into.integrations.bridges,
        &from.integrations.bridges,
        |bridge| bridge.id.as_str().to_string(),
    );
    absorb(
        &mut into.integrations.triggers,
        &from.integrations.triggers,
        |trigger| trigger.id.as_str().to_string(),
    );
    absorb(&mut into.blocks, &from.blocks, |block: &Block| {
        block.id.as_str().to_string()
    });
}

struct Loaded {
    raw: serde_json::Value,
    blueprint: Blueprint,
}

fn load(path: &Path) -> Loaded {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("{} fails to open: {error}", path.display()));
    let raw: serde_json::Value = serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!(
            "{} holds text that parses as no JSON: {error}",
            path.display()
        )
    });
    let blueprint: Blueprint = serde_json::from_str(&text).unwrap_or_else(|error| {
        panic!(
            "{} fails to decode into the Blueprint type set: {error}",
            path.display()
        )
    });
    Loaded { raw, blueprint }
}

#[test]
fn every_fragment_decodes_and_validates() {
    let directory = match module_dir() {
        Some(found) => found,
        None => {
            println!("The content repository is absent from this checkout, so the fragment test stands down.");
            return;
        }
    };
    let files = fragment_files(&directory);
    assert!(
        !files.is_empty(),
        "The module directory holds zero fragments. Author at least the reference fragment."
    );

    let mut loaded: BTreeMap<String, Loaded> = BTreeMap::new();
    for path in &files {
        let name = path
            .file_name()
            .expect("the fragment carries a file name")
            .to_string_lossy()
            .to_string();
        loaded.insert(name, load(path));
    }

    let mut pending: BTreeSet<String> = BTreeSet::new();
    let mut failures: Vec<String> = Vec::new();

    for (name, held) in loaded.iter() {
        let mut merged = held.blueprint.clone();
        let mut queue: Vec<String> = required_files(&held.raw);
        let mut seen: BTreeSet<String> = BTreeSet::new();
        seen.insert(name.clone());
        while let Some(next) = queue.pop() {
            if !seen.insert(next.clone()) {
                continue;
            }
            match loaded.get(&next) {
                Some(dependency) => {
                    merge(&mut merged, &dependency.blueprint);
                    queue.extend(required_files(&dependency.raw));
                }
                None => {
                    pending.insert(next);
                }
            }
        }
        if let Err(report) = merged.validate() {
            failures.push(format!("{name}\n{report}"));
        }
    }

    if !pending.is_empty() {
        println!(
            "{} required fragments await an author: {}",
            pending.len(),
            pending.iter().cloned().collect::<Vec<_>>().join(", ")
        );
    }

    assert!(
        failures.is_empty(),
        "{} fragments carry diagnostics:\n{}",
        failures.len(),
        failures.join("\n")
    );
    println!("{} fragments decode and validate.", files.len());
}

#[test]
fn every_fragment_names_its_catalogue_entry() {
    let directory = match module_dir() {
        Some(found) => found,
        None => return,
    };
    for path in fragment_files(&directory) {
        let held = load(&path);
        let module = held
            .raw
            .get("module")
            .unwrap_or_else(|| panic!("{} carries no catalogue entry", path.display()));
        for member in ["id", "title", "prefix", "contributes", "removal"] {
            let value = module
                .get(member)
                .and_then(|held| held.as_str())
                .unwrap_or_default();
            assert!(
                !value.is_empty(),
                "{} leaves the catalogue member `{member}` empty",
                path.display()
            );
        }
        let prefix = module
            .get("prefix")
            .and_then(|held| held.as_str())
            .expect("the entry names a prefix");
        for collection in &held.blueprint.collections {
            assert!(
                collection.id.as_str().starts_with(prefix),
                "{} declares the collection `{}` outside the module prefix `{prefix}`",
                path.display(),
                collection.id
            );
        }
        for action in &held.blueprint.actions {
            assert!(
                action.id.as_str().starts_with(prefix),
                "{} declares the action `{}` outside the module prefix `{prefix}`",
                path.display(),
                action.id
            );
        }
    }
}

#[test]
fn the_reference_fragment_fills_every_section() {
    let directory = match module_dir() {
        Some(found) => found,
        None => return,
    };
    let path = directory.join("_reference.json");
    assert!(
        path.is_file(),
        "The reference fragment is absent from {}",
        directory.display()
    );
    let held = load(&path);
    let blueprint = held.blueprint;
    assert!(!blueprint.types.is_empty(), "the reference declares types");
    assert!(
        !blueprint.collections.is_empty(),
        "the reference declares collections"
    );
    assert!(
        !blueprint.identities.roles.is_empty(),
        "the reference declares roles"
    );
    assert!(
        !blueprint.permissions.entries.is_empty(),
        "the reference declares a permission matrix"
    );
    assert!(
        !blueprint.rules.invariants.is_empty() && !blueprint.rules.preconditions.is_empty(),
        "the reference declares rules"
    );
    assert!(
        !blueprint.actions.is_empty(),
        "the reference declares actions"
    );
    assert!(
        !blueprint.workflows.is_empty(),
        "the reference declares a workflow"
    );
    assert!(
        !blueprint.events.is_empty(),
        "the reference declares events"
    );
    assert!(!blueprint.views.is_empty(), "the reference declares views");
    assert!(
        !blueprint.integrations.feeds.is_empty()
            && !blueprint.integrations.bridges.is_empty()
            && !blueprint.integrations.triggers.is_empty(),
        "the reference declares every kind of integration"
    );
    assert!(
        blueprint.governance.is_declared(),
        "the reference declares governance"
    );
    assert!(
        blueprint.economics.is_some(),
        "the reference declares a unit of account"
    );
    assert!(
        !blueprint.blocks.is_empty(),
        "the reference declares a block"
    );
    blueprint
        .validate()
        .expect("the reference fragment validates on its own");
}
