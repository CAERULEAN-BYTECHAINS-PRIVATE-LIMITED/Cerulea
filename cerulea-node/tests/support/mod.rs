//! Shared setup for the Blueprint tests.
//!
//! Two suites read this module. The end-to-end suite installs one real
//! Blueprint and drives it from end to end. The coverage suite installs
//! every module fragment of the catalogue and runs one action of each.
//!
//! The module carries four parts.
//!
//! 1. **The fragment library.** Every JSON file under `content/modules`
//!    and `content/government/modules` decodes into the Blueprint type set.
//! 2. **The dependency closure.** A fragment names the modules it requires.
//!    The closure merges each one into the fragment under test.
//! 3. **The chain.** A test runtime built from the development genesis
//!    preset, carrying every platform module.
//! 4. **The parameter generator.** Every declared parameter takes a value
//!    derived from its own type, so two runs produce the same call.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use cerulea_blueprint::{
    actions::Action,
    collections::Collection,
    compose::Block,
    events::EventDef,
    identities::{CredentialDef, DelegationRule, OrganisationUnit, RoleDef},
    permissions::CapabilityEntry,
    rules::{Invariant, Precondition},
    types::{
        Decimal, EnumerationValue, FieldName, Hash32, Identity, ParamName, RecordRef, RecordValue,
        SumValue, Timestamp, TypeDef, TypeExpr, TypeId, Value,
    },
    views::View,
    workflows::Workflow,
    Blueprint,
};
use cerulea_runtime::{AccountId, Runtime, RuntimeOrigin};
use codec::Encode;
use frame_support::BoundedVec;
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use sp_keyring::Ed25519Keyring;
use sp_runtime::BuildStorage;

/// One fragment as it sits on disk.
pub struct Fragment {
    /// The raw JSON, which carries the catalogue entry and the dependency
    /// list beside the Blueprint sections.
    pub raw: serde_json::Value,
    /// The Blueprint sections of the fragment.
    pub blueprint: Blueprint,
    /// The file name the fragment carries.
    pub name: String,
}

/// The workspace root holding the content directory.
pub fn workspace_root() -> Option<PathBuf> {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = here.parent()?.parent()?;
    if root.join("content").is_dir() {
        Some(root.to_path_buf())
    } else {
        None
    }
}

/// Every directory holding module fragments.
pub fn fragment_dirs() -> Vec<PathBuf> {
    let root = match workspace_root() {
        Some(held) => held,
        None => return Vec::new(),
    };
    let mut found = Vec::new();
    let catalogue = root.join("content").join("modules");
    if catalogue.is_dir() {
        found.push(catalogue);
    }
    let government = root.join("content").join("government").join("modules");
    if government.is_dir() {
        found.push(government);
    }
    found
}

/// Every fragment file inside one directory, in name order.
pub fn fragment_files(directory: &Path) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = match fs::read_dir(directory) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().map(|end| end == "json").unwrap_or(false))
            .collect(),
        Err(_) => Vec::new(),
    };
    found.sort();
    found
}

/// Read one fragment from disk.
pub fn load(path: &Path) -> Fragment {
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
    Fragment {
        raw,
        blueprint,
        name: path
            .file_name()
            .expect("the fragment carries a file name")
            .to_string_lossy()
            .to_string(),
    }
}

/// Every fragment of the catalogue and of the Government set, by file name.
pub fn fragment_library() -> BTreeMap<String, Fragment> {
    let mut held: BTreeMap<String, Fragment> = BTreeMap::new();
    for directory in fragment_dirs() {
        for path in fragment_files(&directory) {
            let fragment = load(&path);
            held.insert(fragment.name.clone(), fragment);
        }
    }
    held
}

/// The fragments one fragment requires, named by file.
pub fn required_files(raw: &serde_json::Value) -> Vec<String> {
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
/// The fragment under test owns every name it declares. The dependency
/// adds the names the fragment lacks.
pub fn merge(into: &mut Blueprint, from: &Blueprint) {
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

/// The dependency closure of one fragment, merged into one Blueprint.
///
/// The result carries the fragment under test and every module it
/// requires, so a reference the fragment makes reaches a declaration.
pub fn resolve(name: &str, library: &BTreeMap<String, Fragment>) -> Option<Blueprint> {
    let held = library.get(name)?;
    let mut merged = held.blueprint.clone();
    let mut queue: Vec<String> = required_files(&held.raw);
    let mut seen: BTreeSet<String> = BTreeSet::new();
    seen.insert(name.to_string());
    while let Some(next) = queue.pop() {
        if !seen.insert(next.clone()) {
            continue;
        }
        if let Some(dependency) = library.get(&next) {
            merge(&mut merged, &dependency.blueprint);
            queue.extend(required_files(&dependency.raw));
        }
    }
    Some(merged)
}

/// The account every test acts as.
pub fn actor() -> AccountId {
    Ed25519Keyring::Alice.to_account_id()
}

/// A second account, holding no role.
pub fn outsider() -> AccountId {
    Ed25519Keyring::Bob.to_account_id()
}

/// Build the externalities of a development chain.
///
/// The chain carries every platform module, the Cerulea operator role, the
/// starting tier and the starting Blueprint.
pub fn development_ext() -> sp_io::TestExternalities {
    let json = cerulea_runtime::genesis_config_presets::development_config_genesis();
    let patch = serde_json::to_vec(&json).expect("the development preset serialises");
    let storage = frame_system::GenesisConfig::<Runtime>::default()
        .build_storage()
        .expect("the system module builds its storage");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        frame_support::genesis_builder_helper::build_state::<cerulea_runtime::RuntimeGenesisConfig>(
            patch,
        )
        .expect("the development preset builds");
        frame_system::Pallet::<Runtime>::set_block_number(1);
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(1_700_000_000_000);
    });
    ext.commit_all().expect("the genesis state commits");
    ext
}

/// Turn text into a bounded name.
pub fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits inside the bound")
}

/// The role every test acts under.
pub const TEST_ROLE: &str = "cerulea_test_role";

/// Install one Blueprint and answer with the deployment it produced.
pub fn install(blueprint: &Blueprint) -> Result<DeploymentId, sp_runtime::DispatchError> {
    let encoded = blueprint.encode();
    let bounded: BoundedVec<u8, <Runtime as pallet_cerulea_logic::Config>::MaxBlueprintBytes> =
        BoundedVec::try_from(encoded).map_err(|_| {
            sp_runtime::DispatchError::Other(
                "The Blueprint runs past the size the runtime admits. Split the Blueprint into smaller modules.",
            )
        })?;
    pallet_cerulea_logic::Pallet::<Runtime>::install_blueprint(
        RuntimeOrigin::signed(actor()),
        bounded,
    )?;
    Ok(blueprint.id().0)
}

/// Seed the identity registry and the capability matrix.
///
/// The actor takes one role that reaches every collection and every action
/// the Blueprint declares, and takes every role the Blueprint names beside
/// it. A caller therefore runs any action the Blueprint carries.
pub fn seed_identity_and_permissions(deployment: DeploymentId, blueprint: &Blueprint) {
    let who = actor();
    let _ = pallet_cerulea_identity::Pallet::<Runtime>::register_account(
        RuntimeOrigin::signed(who.clone()),
        who.clone(),
        [7u8; 32],
    );

    let mut roles: Vec<BoundedName> = vec![name(TEST_ROLE)];
    for role in blueprint.identities.roles.iter() {
        if let Ok(held) = name_of(role.id.as_str()) {
            roles.push(held);
        }
    }
    for role in roles.iter() {
        let _ = pallet_cerulea_identity::Pallet::<Runtime>::define_role(
            RuntimeOrigin::signed(who.clone()),
            deployment,
            role.clone(),
            None,
            BoundedVec::default(),
            BoundedVec::truncate_from(b"Cerulea test role".to_vec()),
        );
        let _ = pallet_cerulea_identity::Pallet::<Runtime>::assign_role(
            RuntimeOrigin::signed(who.clone()),
            deployment,
            who.clone(),
            role.clone(),
            None,
        );
    }

    let payload: BoundedVec<u8, <Runtime as pallet_cerulea_permission::Config>::MaxEntryBytes> =
        BoundedVec::truncate_from(GrantPayload::everything().encode());
    let actions: Vec<BoundedName> = blueprint
        .actions
        .iter()
        .filter_map(|action| name_of(action.id.as_str()).ok())
        .collect();

    for collection in blueprint.collections.iter() {
        let held = match name_of(collection.id.as_str()) {
            Ok(found) => found,
            Err(_) => continue,
        };
        for role in roles.iter() {
            for capability in [
                CapabilityKey::Read,
                CapabilityKey::Insert,
                CapabilityKey::Update,
                CapabilityKey::Delete,
            ] {
                let _ = pallet_cerulea_permission::Pallet::<Runtime>::grant(
                    RuntimeOrigin::signed(who.clone()),
                    deployment,
                    role.clone(),
                    held.clone(),
                    capability,
                    payload.clone(),
                );
            }
            for action in actions.iter() {
                let _ = pallet_cerulea_permission::Pallet::<Runtime>::grant(
                    RuntimeOrigin::signed(who.clone()),
                    deployment,
                    role.clone(),
                    held.clone(),
                    CapabilityKey::Invoke(action.clone()),
                    payload.clone(),
                );
            }
        }
    }
}

/// Install a licence covering one deployment.
///
/// The control plane signs the licence with its ed25519 key. The
/// deployment holds the public key of the control plane and checks the
/// signature, so the chain verifies its own licence.
pub fn install_licence(deployment: DeploymentId) {
    use sp_core::Pair;

    let who = actor();
    let control_plane = Ed25519Keyring::Alice.pair();
    let _ = pallet_cerulea_tier::Pallet::<Runtime>::set_control_plane_key(
        RuntimeOrigin::signed(who.clone()),
        deployment,
        control_plane.public().0,
    );

    let features: Vec<BoundedName> = [
        "schema",
        "state",
        "collections",
        "identity",
        "permission",
        "logic",
        "workflow",
        "rules",
        "governance",
        "evolution",
        "oracle",
        "relayer",
        "tier",
        "audit",
        "operator",
        "custody",
        pallet_cerulea_relayer::RELAY_FEATURE,
    ]
    .iter()
    .filter_map(|held| name_of(held).ok())
    .collect();

    let licence: pallet_cerulea_tier::Licence<
        <Runtime as pallet_cerulea_tier::Config>::MaxFeatures,
        <Runtime as pallet_cerulea_tier::Config>::MaxAddons,
    > = pallet_cerulea_tier::Licence {
        deployment,
        tier: pallet_cerulea_tier::Tier {
            application: pallet_cerulea_tier::Application::Enterprise,
            level: 1,
        },
        features: BoundedVec::truncate_from(features),
        seats: 64,
        addons: BoundedVec::truncate_from(Vec::new()),
        issued_at: 0,
        expires_at: 3_155_760_000_000,
        sequence: 1,
    };
    let signature = control_plane.sign(&licence.message()).0;
    pallet_cerulea_tier::Pallet::<Runtime>::install_licence(
        RuntimeOrigin::signed(who),
        deployment,
        BoundedVec::truncate_from(licence.encode()),
        signature,
    )
    .expect("the licence the control plane issued installs");
}

/// Find one declared type by name.
fn type_def<'a>(types: &'a [TypeDef], id: &TypeId) -> Option<&'a TypeDef> {
    types.iter().find(|held| held.id() == id)
}

/// The value one type takes in a generated call.
///
/// The value comes from the type alone, so two runs of the suite produce
/// the same call and the same record. A type reaching past the depth limit
/// answers with nothing, and the caller reports the parameter it failed to
/// fill.
pub fn sample_value(ty: &TypeExpr, types: &[TypeDef], depth: u8) -> Option<Value> {
    if depth > 8 {
        return None;
    }
    let held = match ty {
        TypeExpr::Bool => Value::Bool(true),
        TypeExpr::Integer => Value::Integer(1),
        TypeExpr::Decimal { scale } => Value::Decimal(Decimal::new(100, *scale)),
        TypeExpr::Text => Value::Text("cerulea".to_string()),
        TypeExpr::Bytes => Value::Bytes(vec![1, 2, 3, 4]),
        TypeExpr::Timestamp => Value::Timestamp(Timestamp::from_millis(1_700_000_000_000)),
        TypeExpr::Identity => Value::Identity(Identity([7u8; 32])),
        TypeExpr::Hash => Value::Hash(Hash32([9u8; 32])),
        TypeExpr::List(element) => Value::list((**element).clone(), Vec::new()),
        TypeExpr::Map { key, value } => Value::map((**key).clone(), (**value).clone(), Vec::new()),
        TypeExpr::Optional(element) => Value::none((**element).clone()),
        TypeExpr::Record(id) => match type_def(types, id)? {
            TypeDef::Record(def) => {
                let mut fields: Vec<(FieldName, Value)> = Vec::new();
                for field in def.fields.iter() {
                    let value = sample_value(&field.ty, types, depth.saturating_add(1))?;
                    fields.push((field.name.clone(), value));
                }
                let mut record = Value::Record(RecordValue {
                    type_id: id.clone(),
                    fields,
                });
                record.canonicalise();
                record
            }
            _ => return None,
        },
        TypeExpr::Enumeration(id) => match type_def(types, id)? {
            TypeDef::Enumeration(def) => Value::Enumeration(EnumerationValue {
                type_id: id.clone(),
                variant: def.variants.first()?.clone(),
            }),
            _ => return None,
        },
        TypeExpr::Sum(id) => match type_def(types, id)? {
            TypeDef::Sum(def) => {
                let variant = def.variants.first()?;
                let payload = match variant.payload.as_ref() {
                    Some(inner) => Some(Box::new(sample_value(
                        inner,
                        types,
                        depth.saturating_add(1),
                    )?)),
                    None => None,
                };
                Value::Sum(SumValue {
                    type_id: id.clone(),
                    variant: variant.name.clone(),
                    payload,
                })
            }
            _ => return None,
        },
        TypeExpr::Reference(collection) => Value::Reference(RecordRef {
            collection: collection.clone(),
            key: Box::new(Value::Text("cerulea".to_string())),
        }),
    };
    Some(held)
}

/// The arguments one action takes in a generated call.
pub fn sample_arguments(action: &Action, types: &[TypeDef]) -> Option<Vec<(ParamName, Value)>> {
    let mut found: Vec<(ParamName, Value)> = Vec::new();
    for param in action.params.iter() {
        let value = sample_value(&param.ty, types, 0)?;
        found.push((param.name.clone(), value));
    }
    Some(found)
}

/// Run one action of one deployment as the test actor.
pub fn run_action(
    deployment: DeploymentId,
    action: &Action,
    arguments: &[(ParamName, Value)],
) -> Result<(), sp_runtime::DispatchError> {
    let encoded: BoundedVec<u8, <Runtime as pallet_cerulea_logic::Config>::MaxPayloadBytes> =
        BoundedVec::try_from(arguments.to_vec().encode()).map_err(|_| {
            sp_runtime::DispatchError::Other(
                "The arguments run past the size the runtime admits. Shorten the values the call carries.",
            )
        })?;
    pallet_cerulea_logic::Pallet::<Runtime>::execute_action(
        RuntimeOrigin::signed(actor()),
        deployment,
        name(action.id.as_str()),
        encoded,
    )
}
