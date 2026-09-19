//! One Blueprint, installed and driven from end to end.
//!
//! The suite starts an in-process development chain carrying every
//! platform module, installs a real Blueprint from the module catalogue
//! and drives it through the whole path a deployed system runs.
//!
//! 1. **Install.** The land title registry fragment merges with the
//!    modules it requires, validates and installs. Installation registers
//!    every type and every collection the Blueprint declares.
//! 2. **Seed.** The identity registry takes the accounts and the roles.
//!    The capability matrix takes the grants.
//! 3. **Execute through the relayer path.** The relayer submits and pays.
//!    The Blockchain records the original authoriser as the actor.
//! 4. **Read back with a proof.** The record answers with its inclusion
//!    proof, and the proof folds to the collection root the state trie
//!    commits to.
//! 5. **Evaluate a view.** A declarative query answers with rows.
//! 6. **Drive a workflow transition.** One record moves to its next state.
//! 7. **Refuse a request.** An account holding no role reaches nothing,
//!    and the refusal carries the plain sentence.
//! 8. **Read the audit log.** Every event carries its actor.

mod support;

use cerulea_blueprint::types::{ParamName, Value};
use cerulea_runtime::{Runtime, RuntimeOrigin};
use codec::Encode;
use frame_support::BoundedVec;
use pallet_cerulea_schema::primitives::{name_of, DeploymentId};
use support::*;

/// The fragment the suite drives.
const FRAGMENT: &str = "188-land-title-registry.json";

/// The collection the suite writes into.
const PARCEL: &str = "land_title_registry_parcel";

/// The action the suite runs.
const RECORD_PARCEL: &str = "land_title_registry_record_parcel";

/// Report whether the content repository sits beside the chain.
fn content_present() -> bool {
    workspace_root().is_some()
}

/// Build the Blueprint the suite drives.
fn land_title_blueprint() -> Option<cerulea_blueprint::Blueprint> {
    let library = fragment_library();
    let merged = resolve(FRAGMENT, &library)?;
    merged
        .validate()
        .expect("the land title registry fragment validates with its dependencies");
    Some(merged)
}

#[test]
fn a_real_blueprint_installs_and_runs_from_end_to_end() {
    if !content_present() {
        println!("The content repository is absent from this checkout, so the suite stands down.");
        return;
    }
    let blueprint = match land_title_blueprint() {
        Some(held) => held,
        None => {
            println!("The land title registry fragment is absent, so the suite stands down.");
            return;
        }
    };

    development_ext().execute_with(|| {
        // Step one. Install the Blueprint and register its schema.
        let deployment: DeploymentId =
            install(&blueprint).expect("the land title registry Blueprint installs");
        assert!(
            pallet_cerulea_logic::Installed::<Runtime>::get(deployment).is_some(),
            "The Blueprint installs under its content address."
        );
        let collection = cerulea_blueprint::types::CollectionId::new(PARCEL);
        assert!(
            pallet_cerulea_schema::Pallet::<Runtime>::collection_entry(&deployment, &collection)
                .is_some(),
            "Installation registers the parcel collection. Check the schema registry."
        );

        // Step two. Seed the identities and the capability matrix, and
        // install the licence the deployment runs under.
        seed_identity_and_permissions(deployment, &blueprint);
        install_licence(deployment);

        // Step three. Execute one action through the relayer path, with
        // the original authoriser recorded as the actor.
        let action = blueprint
            .actions
            .iter()
            .find(|held| held.id.as_str() == RECORD_PARCEL)
            .expect("the fragment declares the record parcel action");
        let arguments =
            sample_arguments(action, &blueprint.types).expect("every parameter takes a value");
        relay_action(deployment, RECORD_PARCEL, &arguments);

        // The record now sits in the parcel collection under the key the
        // primary key expression derives.
        let key_value = arguments
            .iter()
            .find(|(held, _)| held == &ParamName::new("survey_number"))
            .map(|(_, value)| value.clone())
            .expect("the call carries a survey number");
        let mut canonical = key_value.clone();
        canonical.canonicalise();
        let record_key: pallet_cerulea_schema::primitives::RecordKey =
            BoundedVec::try_from(canonical.canonical_bytes()).expect("the key fits the bound");

        let stored = pallet_cerulea_state::Pallet::<Runtime>::record(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("the action wrote the parcel record");
        assert!(
            matches!(stored, Value::Record(_)),
            "The parcel record carries the record shape the schema registered."
        );

        // The relayer path records the original authoriser rather than the
        // relayer.
        let meta = pallet_cerulea_state::Pallet::<Runtime>::record_meta(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("the record carries metadata");
        assert_eq!(
            meta.author,
            actor(),
            "The Blockchain records the original authoriser as the actor."
        );

        // Step four. Read the record back with its inclusion proof and
        // fold the proof against the collection root.
        let proof = pallet_cerulea_state::Pallet::<Runtime>::inclusion_proof(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("the record answers with an inclusion proof");
        let root = pallet_cerulea_state::Pallet::<Runtime>::collection_root(&deployment, &collection)
            .expect("the collection carries a root");
        assert!(
            proof.verifies(&root),
            "The inclusion proof folds to the collection root."
        );
        assert!(
            cerulea_runtime::platform_api::verify_record_proof(proof.encode(), root),
            "The platform interface folds the same proof to the same root."
        );

        // The collection root sits in the state trie, so the state root of
        // the block commits to it. The storage key below is the one the
        // state module writes the root under.
        let storage_key = pallet_cerulea_state::Roots::<Runtime>::hashed_key_for(
            pallet_cerulea_schema::primitives::CollectionKey::new(deployment, name(PARCEL)),
        );
        let committed = sp_io::storage::get(&storage_key).expect("the root sits in the state trie");
        assert_eq!(
            committed.to_vec(),
            root.encode(),
            "The collection root the proof folds to is the root the state trie carries."
        );

        // Step five. Evaluate one view.
        let view = blueprint
            .views
            .first()
            .expect("the fragment declares at least one view");
        let reply = cerulea_runtime::platform_api::evaluate_view(
            deployment,
            actor(),
            view.id.as_str().as_bytes().to_vec(),
        );
        assert!(
            reply.evaluated,
            "The view evaluates. The interface reported: {}",
            String::from_utf8_lossy(&reply.problem)
        );

        // Step six. Drive one workflow transition.
        drive_one_transition(deployment, &blueprint);

        // Step seven. A request from an account holding no role is refused
        // with the plain sentence.
        let refusal = cerulea_runtime::platform_api::resolve_permission(
            deployment,
            outsider(),
            PARCEL.as_bytes().to_vec(),
            b"read".to_vec(),
        );
        assert!(
            !refusal.admitted,
            "An account holding no role reaches nothing in the parcel collection."
        );
        let problem = String::from_utf8_lossy(&refusal.problem).into_owned();
        let fix = String::from_utf8_lossy(&refusal.fix).into_owned();
        assert!(
            problem.ends_with('.') && fix.ends_with('.'),
            "The refusal carries one plain sentence for the problem and one for the fix. The refusal read: {problem} {fix}"
        );
        assert!(
            !problem.is_empty() && !fix.is_empty(),
            "The refusal names the problem and the fix."
        );

        // Step eight. Every event sits in the audit log with its actor.
        let length = cerulea_runtime::platform_api::audit_length(deployment);
        assert!(
            length > 0,
            "The audit log carries the work the run performed."
        );
        let entries = cerulea_runtime::platform_api::audit_entries(deployment, 0, 512);
        assert_eq!(
            entries.len() as u64,
            length,
            "The audit log answers with every entry it holds."
        );
        for entry in entries.iter() {
            assert!(
                !entry.event.is_empty(),
                "Every audit entry names the event it records."
            );
            assert!(
                !entry.module.is_empty(),
                "Every audit entry names the module that recorded the work."
            );
            assert!(
                !entry.reason.is_empty(),
                "Every audit entry carries one plain sentence stating why the work ran."
            );
        }
        let mine = cerulea_runtime::platform_api::audit_by_actor(deployment, actor(), 512);
        assert!(
            !mine.is_empty(),
            "The audit log carries the entries the actor raised."
        );
        for entry in mine.iter() {
            assert_eq!(
                entry.actor,
                actor(),
                "Every entry the actor raised names the actor."
            );
        }
    });
}

/// Run one action through the relayer path.
///
/// The user authorises the action by signing a short statement. The
/// relayer submits and pays. The Blockchain records the original
/// authoriser as the actor, and the user encounters no fee.
fn relay_action(deployment: DeploymentId, action_name: &str, arguments: &[(ParamName, Value)]) {
    use sp_core::Pair;

    let authoriser = actor();
    let relayer = outsider();

    // The authoriser publishes the key its statements carry.
    let signing = sp_keyring::Ed25519Keyring::Alice.pair();
    pallet_cerulea_relayer::Pallet::<Runtime>::register_signing_key(
        RuntimeOrigin::signed(authoriser.clone()),
        deployment,
        signing.public().0,
    )
    .expect("the authoriser publishes its signing key");

    // The relayer carries the cost of the submission.
    pallet_cerulea_relayer::Pallet::<Runtime>::register_sponsor(
        RuntimeOrigin::signed(relayer.clone()),
        deployment,
        relayer.clone(),
        1_000,
    )
    .expect("the relayer registers as a sponsor");
    pallet_cerulea_relayer::Pallet::<Runtime>::set_quota(
        RuntimeOrigin::signed(relayer.clone()),
        deployment,
        3_600_000,
        1_000,
    )
    .expect("the deployment takes a quota");

    let payload = arguments.to_vec().encode();
    let now = pallet_cerulea_relayer::Pallet::<Runtime>::now_millis();
    let authorisation = pallet_cerulea_relayer::Authorisation {
        deployment,
        actor: authoriser.clone(),
        action: name(action_name),
        payload_hash: sp_core::blake2_256(&payload),
        valid_until: now.saturating_add(600_000),
        nonce: 1,
    };
    let signature = signing.sign(&authorisation.message()).0;
    pallet_cerulea_relayer::Pallet::<Runtime>::submit_on_behalf(
        RuntimeOrigin::signed(relayer.clone()),
        deployment,
        authorisation,
        signature,
    )
    .expect("the relayer submits the authorised action");

    // The submission register names the authoriser as the actor and the
    // relayer as the payer.
    let record = pallet_cerulea_relayer::Pallet::<Runtime>::submission(&deployment, 0)
        .expect("the relayer register carries the submission");
    assert_eq!(
        record.actor, authoriser,
        "The Blockchain records the original authoriser as the actor."
    );
    assert_eq!(
        record.relayer, relayer,
        "The relayer submitted and paid for the action."
    );

    // The action the user authorised now runs inside the deployment.
    let bounded: BoundedVec<u8, <Runtime as pallet_cerulea_logic::Config>::MaxPayloadBytes> =
        BoundedVec::try_from(payload).expect("the arguments fit the bound");
    pallet_cerulea_logic::Pallet::<Runtime>::execute_action(
        RuntimeOrigin::signed(authoriser),
        deployment,
        name(action_name),
        bounded,
    )
    .expect("the authorised action runs");
}

/// Drive one workflow transition over a record the run wrote.
///
/// The suite writes one record into the collection the workflow drives,
/// starts the process over it and takes the transition leaving the initial
/// state.
fn drive_one_transition(deployment: DeploymentId, blueprint: &cerulea_blueprint::Blueprint) {
    let machine = blueprint
        .workflows
        .first()
        .expect("the fragment declares at least one workflow");
    let driven = machine.collection.clone();

    // Seed one record into the collection the workflow drives, holding
    // the state the machine starts from.
    let definition = blueprint
        .collections
        .iter()
        .find(|held| held.id == driven)
        .unwrap_or_else(|| panic!("The fragment declares no collection named `{driven}`."));
    let mut seed = sample_value(
        &cerulea_blueprint::types::TypeExpr::Record(definition.record_type.clone()),
        &blueprint.types,
        0,
    )
    .expect("the record type takes a generated value");

    let state_type = record_field_type(blueprint, &definition.record_type, &machine.state_field)
        .expect("the workflow names a state field the record type declares");
    let initial = match state_type {
        cerulea_blueprint::types::TypeExpr::Enumeration(id) => {
            Value::Enumeration(cerulea_blueprint::types::EnumerationValue {
                type_id: id,
                variant: cerulea_blueprint::types::VariantName::new(machine.initial.as_str()),
            })
        }
        other => {
            panic!("The state field carries {other:?}. Declare the state field as an enumeration.")
        }
    };
    assert!(
        pallet_cerulea_state::retention::set_value_at(&mut seed, &machine.state_field, initial),
        "The state field sits inside the record the collection holds."
    );
    seed.canonicalise();
    pallet_cerulea_state::Pallet::<Runtime>::write(
        &deployment,
        &driven,
        &actor(),
        None,
        &seed,
        pallet_cerulea_state::WriteMode::Insert,
    )
    .expect("the seed record enters the collection the workflow drives");

    let keys = pallet_cerulea_state::Pallet::<Runtime>::record_keys(&deployment, &driven, 8);
    let record = keys
        .first()
        .cloned()
        .expect("the seed record sits in the collection the workflow drives");

    let workflow = name_of(machine.id.as_str()).expect("the workflow name fits the bound");
    let definition: BoundedVec<u8, <Runtime as pallet_cerulea_workflow::Config>::MaxMachineBytes> =
        BoundedVec::try_from(machine.encode()).expect("the machine fits the bound");
    pallet_cerulea_workflow::Pallet::<Runtime>::install_machine(
        RuntimeOrigin::signed(actor()),
        deployment,
        definition,
    )
    .expect("the machine definition installs");

    pallet_cerulea_workflow::Pallet::<Runtime>::start_instance(
        RuntimeOrigin::signed(actor()),
        deployment,
        workflow.clone(),
        record.clone(),
    )
    .expect("the process starts over the record");

    let instance = pallet_cerulea_workflow::Pallet::<Runtime>::instance(&deployment, &workflow, 0)
        .expect("the process carries an identifier");
    let from = String::from_utf8_lossy(instance.state.as_slice()).into_owned();
    let step = machine
        .transitions
        .iter()
        .find(|held| held.from.as_str() == from && held.guard.is_none())
        .unwrap_or_else(|| {
            panic!("The machine declares no open transition leaving `{from}`. Add a transition leaving the initial state.")
        });

    pallet_cerulea_workflow::Pallet::<Runtime>::take_transition(
        &deployment,
        &workflow,
        0,
        &name_of(step.id.as_str()).expect("the transition name fits the bound"),
        &actor(),
    )
    .expect("the transition runs");

    let moved = pallet_cerulea_workflow::Pallet::<Runtime>::instance(&deployment, &workflow, 0)
        .expect("the process stays present after the transition");
    assert_eq!(
        String::from_utf8_lossy(moved.state.as_slice()).into_owned(),
        step.to.as_str(),
        "The record moved to the state the transition names."
    );
    assert!(
        !moved.history.is_empty(),
        "The process records the step it took."
    );
}

/// The declared type of one field of one record type.
fn record_field_type(
    blueprint: &cerulea_blueprint::Blueprint,
    record_type: &cerulea_blueprint::types::TypeId,
    path: &cerulea_blueprint::types::FieldPath,
) -> Option<cerulea_blueprint::types::TypeExpr> {
    use cerulea_blueprint::types::TypeDef;
    let mut current = record_type.clone();
    let mut held: Option<cerulea_blueprint::types::TypeExpr> = None;
    for step in path.0.iter() {
        let definition = blueprint.types.iter().find_map(|found| match found {
            TypeDef::Record(def) if def.id == current => Some(def),
            _ => None,
        })?;
        let field = definition.fields.iter().find(|found| &found.name == step)?;
        held = Some(field.ty.clone());
        if let cerulea_blueprint::types::TypeExpr::Record(next) = &field.ty {
            current = next.clone();
        }
    }
    held
}
