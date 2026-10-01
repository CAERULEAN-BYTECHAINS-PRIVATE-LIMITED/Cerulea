//! The platform runtime interfaces against real runtime state.
//!
//! Every test builds the development genesis preset, installs a small land
//! registry Blueprint through the logic module and drives the platform
//! modules through their calls. The reads under test then answer from the
//! state those calls wrote.

use crate::genesis_config_presets::{cerulea_operator, starting_blueprint, starting_deployment};
use crate::platform_api as api;
use crate::platform_tests::development_ext;
use crate::{AccountId, Runtime, RuntimeOrigin};
use cerulea_blueprint::{
    actions::{Action, Effect, Param, TransactionClass},
    collections::{Collection, KeySelector, PurposeDeclaration},
    expression::Expr,
    identities::RoleDef,
    types::{
        CollectionId, FieldDef, FieldName, ParamName, PurposeTag, RecordDef, RoleId, TypeDef,
        TypeExpr, TypeId, Value,
    },
    views::{Projection, View},
    Blueprint, BlueprintMeta,
};
use codec::{Decode, Encode};
use frame_support::{assert_ok, BoundedVec};
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use sp_keyring::Ed25519Keyring;

type Logic = pallet_cerulea_logic::Pallet<Runtime>;
type Identity = pallet_cerulea_identity::Pallet<Runtime>;
type Permission = pallet_cerulea_permission::Pallet<Runtime>;
type Governance = pallet_cerulea_governance::Pallet<Runtime>;
type Timestamp = pallet_timestamp::Pallet<Runtime>;

/// The account that registers parcels in every test.
fn registrar() -> AccountId {
    Ed25519Keyring::Alice.to_account_id()
}

/// An account holding no role in the registry.
fn stranger() -> AccountId {
    Ed25519Keyring::Bob.to_account_id()
}

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits the bound")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
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
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
        ],
        summary: "One land parcel in the register.".into(),
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
        &["registration"],
    );
    collection
}

fn parcel(survey: &str, holder: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (FieldName::new("survey_number"), Value::Text(survey.into())),
            (FieldName::new("holder"), Value::Text(holder.into())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

fn transfer_action() -> Action {
    let mut action = Action::new(
        "record_transfer",
        "The registrar records a transfer of one parcel.",
    );
    action.params = vec![Param::new(
        "parcel",
        TypeExpr::Record(TypeId::new("parcel")),
        "The parcel the registrar records.",
    )];
    action.effects = vec![Effect::Insert {
        collection: CollectionId::new("parcel"),
        key: KeySelector::Exact(Expr::field(Expr::bound("parcel"), "survey_number")),
        record: Expr::bound("parcel"),
    }];
    action.roles.push(RoleId::new("registrar"));
    action
}

fn holder_view() -> View {
    let mut view = View::new(
        "holders",
        "parcel",
        "The view lists the holder of every parcel.",
    );
    view.purposes = vec![PurposeTag::new("registration")];
    view.projection = vec![
        Projection::computed(
            "survey_number",
            Expr::field(Expr::bound("record"), "survey_number"),
        ),
        Projection::computed("holder", Expr::field(Expr::bound("record"), "holder")),
    ];
    view
}

/// The land registry Blueprint the tests install.
fn registry_blueprint() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "district land registry",
        "The registry records land parcels and their holders.",
        "Kollam District Administration",
    ));
    blueprint.types.push(parcel_type());
    blueprint.collections.push(parcel_collection());
    blueprint.identities.roles.push(RoleDef::new(
        "registrar",
        None,
        "Registrar",
        "The registrar records land transfers.",
    ));
    blueprint.actions.push(transfer_action());
    blueprint.views.push(holder_view());
    blueprint
}

/// Install the registry and give the registrar every capability over it.
fn install_registry() -> DeploymentId {
    let blueprint = registry_blueprint();
    assert_ok!(Logic::install_blueprint(
        RuntimeOrigin::signed(registrar()),
        bounded(blueprint.encode())
    ));
    let deployment = blueprint.id().0;
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(registrar()),
        registrar(),
        [7u8; 32]
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(registrar()),
        deployment,
        name("registrar"),
        None,
        BoundedVec::default(),
        BoundedVec::truncate_from(b"Registrar".to_vec())
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(registrar()),
        deployment,
        registrar(),
        name("registrar"),
        None
    ));
    for capability in [
        CapabilityKey::Read,
        CapabilityKey::Insert,
        CapabilityKey::Update,
        CapabilityKey::Delete,
        CapabilityKey::Invoke(name("record_transfer")),
    ] {
        assert_ok!(Permission::grant(
            RuntimeOrigin::signed(registrar()),
            deployment,
            name("registrar"),
            name("parcel"),
            capability,
            bounded(GrantPayload::everything().encode())
        ));
    }
    deployment
}

/// The stored key of one parcel, the canonical bytes of its survey number.
fn record_key(survey: &str) -> Vec<u8> {
    let mut value = Value::Text(survey.into());
    value.canonicalise();
    value.canonical_bytes()
}

/// Record one transfer through the action the Blueprint declares.
fn record_transfer(deployment: DeploymentId, survey: &str, holder: &str, area: i128) {
    let arguments: pallet_cerulea_logic::Bindings =
        vec![(ParamName::new("parcel"), parcel(survey, holder, area))];
    assert_ok!(Logic::execute_action(
        RuntimeOrigin::signed(registrar()),
        deployment,
        name("record_transfer"),
        bounded(arguments.encode())
    ));
}

#[test]
fn the_starting_deployment_answers_every_blueprint_read() {
    development_ext().execute_with(|| {
        let deployment = starting_deployment();
        assert_eq!(api::deployments(), vec![deployment]);
        assert_eq!(
            api::blueprint_address(deployment),
            Some(starting_blueprint().id().0)
        );
        let encoded = api::blueprint(deployment).expect("the chain holds the Blueprint");
        let decoded = Blueprint::decode(&mut encoded.as_slice()).expect("the bytes decode");
        assert_eq!(decoded, starting_blueprint());
        assert!(api::type_names(deployment).is_empty());
        assert!(api::collections(deployment).is_empty());
        assert!(api::action_names(deployment).is_empty());
    });
}

#[test]
fn an_unknown_deployment_answers_empty_reads() {
    development_ext().execute_with(|| {
        let unknown = [9u8; 32];
        assert_eq!(api::blueprint_address(unknown), None);
        assert_eq!(api::blueprint(unknown), None);
        assert!(api::type_names(unknown).is_empty());
        assert_eq!(api::type_definition(unknown, b"parcel".to_vec()), None);
        assert!(api::action_names(unknown).is_empty());
        assert_eq!(
            api::action_manifest(unknown, b"record_transfer".to_vec()),
            None
        );
        assert_eq!(api::licence_state(unknown), None);
        assert!(api::operator_accounts(unknown).is_empty());
        assert!(api::workflow_names(unknown).is_empty());
        assert!(api::oracle_feeds(unknown).is_empty());
        assert!(api::versions(unknown).is_empty());
        assert!(api::rollback_windows(unknown).is_empty());
        assert_eq!(api::audit_length(unknown), 0);
        assert!(api::audit_entries(unknown, 0, 10).is_empty());
        assert!(api::proposals(unknown, 10).is_empty());
        assert_eq!(api::proposal(unknown, 0), None);
    });
}

#[test]
fn an_installed_blueprint_answers_its_types_and_collections() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        assert!(api::deployments().contains(&deployment));
        assert_eq!(api::type_names(deployment), vec![b"parcel".to_vec()]);

        let encoded =
            api::type_definition(deployment, b"parcel".to_vec()).expect("the type is registered");
        let decoded = TypeDef::decode(&mut encoded.as_slice()).expect("the bytes decode");
        assert_eq!(decoded, parcel_type());
        assert_eq!(api::type_definition(deployment, b"deed".to_vec()), None);
        assert_eq!(api::type_definition(deployment, vec![0xff, 0xfe]), None);

        let summaries = api::collections(deployment);
        assert_eq!(summaries.len(), 1);
        let summary = &summaries[0];
        assert_eq!(summary.name, b"parcel".to_vec());
        assert_eq!(summary.record_type, b"parcel".to_vec());
        assert_eq!(summary.privacy, cerulea_platform_api::PrivacyLabel::Open);
        assert_eq!(summary.records, 0);
        assert_eq!(summary.root, pallet_cerulea_state::proof::empty_root());
        assert!(!summary.retired);
        assert_eq!(summary.summary, b"The register of land parcels.".to_vec());
    });
}

#[test]
fn a_recorded_parcel_answers_with_its_value_history_and_proof() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        record_transfer(deployment, "12/3", "Anitha", 400);
        record_transfer(deployment, "44/9", "Benny", 100);

        let summaries = api::collections(deployment);
        assert_eq!(summaries[0].records, 2);
        let root = api::collection_root(deployment, b"parcel".to_vec())
            .expect("the collection holds records");
        assert_eq!(summaries[0].root, root);

        let reply = api::record(deployment, b"parcel".to_vec(), record_key("12/3"))
            .expect("the record exists");
        assert_eq!(reply.collection, b"parcel".to_vec());
        assert_eq!(reply.key, record_key("12/3"));
        assert_eq!(reply.root, root);
        assert_eq!(reply.privacy, cerulea_platform_api::PrivacyLabel::Open);
        let stored = reply.value.clone().expect("an open record shows its value");
        assert!(!stored.is_empty());
        assert_eq!(
            reply.version,
            reply.history.last().map(|entry| entry.version).unwrap()
        );
        assert!(!reply.history.is_empty());

        let proof = reply.proof.expect("an open record carries its proof");
        assert!(api::verify_record_proof(proof.clone(), root));
        assert!(!api::verify_record_proof(proof, [1u8; 32]));
        assert!(!api::verify_record_proof(vec![0xff; 3], root));

        assert_eq!(
            api::record(deployment, b"parcel".to_vec(), record_key("99/9")),
            None
        );
        assert_eq!(
            api::record(deployment, vec![0xff], record_key("12/3")),
            None
        );
        assert_eq!(api::collection_root(deployment, vec![0xff]), None);
        // A collection holding no record answers the empty tree root.
        assert_eq!(
            api::collection_root(deployment, b"deed".to_vec()),
            Some(pallet_cerulea_state::proof::empty_root())
        );
    });
}

#[test]
fn the_record_listing_filters_by_prefix_and_holds_the_limit() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        record_transfer(deployment, "12/3", "Anitha", 400);
        record_transfer(deployment, "12/7", "Chandra", 250);
        record_transfer(deployment, "44/9", "Benny", 100);

        let all = api::records(deployment, b"parcel".to_vec(), Vec::new(), 10);
        assert_eq!(all.len(), 3);

        // Two keys share every canonical byte up to the final character.
        let full = record_key("12/3");
        let prefix = full[..full.len() - 1].to_vec();
        let twelve = api::records(deployment, b"parcel".to_vec(), prefix.clone(), 10);
        let keys: Vec<Vec<u8>> = twelve.iter().map(|held| held.key.clone()).collect();
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|key| key.starts_with(&prefix)));
        assert!(keys.contains(&record_key("12/3")));
        assert!(keys.contains(&record_key("12/7")));

        // A limit of zero rises to the floor of one row.
        assert_eq!(
            api::records(deployment, b"parcel".to_vec(), Vec::new(), 0).len(),
            1
        );
        assert!(api::records(deployment, vec![0xff], Vec::new(), 10).is_empty());
    });
}

#[test]
fn the_permission_read_admits_the_registrar_with_every_field() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        let reply = api::resolve_permission(
            deployment,
            registrar(),
            b"parcel".to_vec(),
            b"read".to_vec(),
        );
        assert!(reply.admitted);
        assert_eq!(reply.role, Some(b"registrar".to_vec()));
        assert!(reply.every_field);
        assert!(reply.fields.is_empty());
        assert!(reply.problem.is_empty());
        assert!(reply.fix.is_empty());

        for action in [&b"insert"[..], b"update", b"delete", b"record_transfer"] {
            let reply = api::resolve_permission(
                deployment,
                registrar(),
                b"parcel".to_vec(),
                action.to_vec(),
            );
            assert!(reply.admitted, "the registrar holds {:?}", action);
        }
    });
}

#[test]
fn the_permission_read_refuses_a_stranger_and_names_the_fix() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        let reply =
            api::resolve_permission(deployment, stranger(), b"parcel".to_vec(), b"read".to_vec());
        assert!(!reply.admitted);
        assert_eq!(reply.role, None);
        assert!(!reply.problem.is_empty());
        assert!(!reply.fix.is_empty());

        let unreadable_action = api::resolve_permission(
            deployment,
            registrar(),
            b"parcel".to_vec(),
            vec![0xff, 0xfe],
        );
        assert!(!unreadable_action.admitted);
        assert_eq!(
            unreadable_action.fix,
            b"Name an action the Blueprint declares.".to_vec()
        );

        let unreadable_collection =
            api::resolve_permission(deployment, registrar(), vec![0xff, 0xfe], b"read".to_vec());
        assert!(!unreadable_collection.admitted);
        assert_eq!(
            unreadable_collection.fix,
            b"Shorten the collection name in the Blueprint.".to_vec()
        );
    });
}

#[test]
fn the_role_simulation_answers_what_the_registrar_reaches() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        let reply = api::simulate_as_role(deployment, b"registrar".to_vec(), b"parcel".to_vec());
        assert_eq!(reply.role, b"registrar".to_vec());
        assert_eq!(reply.collection, b"parcel".to_vec());
        assert!(reply.read && reply.insert && reply.update && reply.delete);
        assert!(reply.every_field);
        assert_eq!(reply.actions, vec![b"record_transfer".to_vec()]);
        assert!(!reply.statement.is_empty());

        let outsider = api::simulate_as_role(deployment, b"clerk".to_vec(), b"parcel".to_vec());
        assert!(!outsider.read && !outsider.insert && !outsider.update && !outsider.delete);

        let unreadable = api::simulate_as_role(deployment, vec![0xff], b"parcel".to_vec());
        assert!(!unreadable.read);
        assert_eq!(
            unreadable.statement,
            b"The role reaches nothing in that collection.".to_vec()
        );
        let unreadable_collection =
            api::simulate_as_role(deployment, b"registrar".to_vec(), vec![0xff]);
        assert!(!unreadable_collection.read);
        assert_eq!(unreadable_collection.role, b"registrar".to_vec());
    });
}

#[test]
fn a_view_evaluates_over_the_recorded_parcels() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        record_transfer(deployment, "12/3", "Anitha", 400);
        record_transfer(deployment, "44/9", "Benny", 100);

        let reply = api::evaluate_view(deployment, registrar(), b"holders".to_vec());
        assert!(reply.evaluated);
        assert_eq!(reply.rows, 2);
        assert_eq!(reply.encoded_rows.len(), 2);
        assert_ne!(reply.digest, [0u8; 32]);
        assert!(reply.problem.is_empty());

        // The same state gives the same digest.
        let again = api::evaluate_view(deployment, registrar(), b"holders".to_vec());
        assert_eq!(again.digest, reply.digest);

        let unknown = api::evaluate_view(deployment, registrar(), b"owners".to_vec());
        assert!(!unknown.evaluated);
        assert_eq!(
            unknown.problem,
            b"The view failed to evaluate. Check the roles the view names and the collections it reads."
                .to_vec()
        );

        let unreadable = api::evaluate_view(deployment, registrar(), vec![0xff]);
        assert!(!unreadable.evaluated);
        assert_eq!(
            unreadable.problem,
            b"The request names a view the Blueprint never declared. Name a view the Blueprint declares."
                .to_vec()
        );
    });
}

#[test]
fn the_action_manifest_names_what_the_action_writes() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        assert_eq!(
            api::action_names(deployment),
            vec![b"record_transfer".to_vec()]
        );
        let manifest = api::action_manifest(deployment, b"record_transfer".to_vec())
            .expect("the action is declared");
        assert_eq!(manifest.action, b"record_transfer".to_vec());
        assert_eq!(manifest.class, b"routine".to_vec());
        assert_eq!(manifest.writes, vec![b"parcel".to_vec()]);
        assert!(manifest.reads.is_empty());
        assert_eq!(manifest.roles, vec![b"registrar".to_vec()]);
        assert!(manifest.emits.is_empty());

        assert_eq!(
            api::action_manifest(deployment, b"record_sale".to_vec()),
            None
        );
        assert_eq!(api::action_manifest(deployment, vec![0xff]), None);
    });
}

#[test]
fn the_action_manifest_labels_every_class_and_every_effect() {
    development_ext().execute_with(|| {
        let mut blueprint = registry_blueprint();
        let classes = [
            ("routine_read", TransactionClass::Routine, &b"routine"[..]),
            ("financial_read", TransactionClass::Financial, b"financial"),
            (
                "structural_read",
                TransactionClass::Structural,
                b"structural",
            ),
            (
                "constitutional_read",
                TransactionClass::Constitutional,
                b"constitutional",
            ),
        ];
        for (id, class, _) in classes.iter() {
            let mut action = Action::new(id, "The registrar reads one parcel.");
            action.class = *class;
            action.params = vec![Param::new(
                "survey",
                TypeExpr::Text,
                "The survey number the registrar reads.",
            )];
            action.effects = vec![
                Effect::Read {
                    collection: CollectionId::new("parcel"),
                    key: KeySelector::Exact(Expr::bound("survey")),
                    binding: ParamName::new("found"),
                    element: TypeExpr::Record(TypeId::new("parcel")),
                },
                Effect::Delete {
                    collection: CollectionId::new("parcel"),
                    key: KeySelector::Exact(Expr::bound("survey")),
                },
            ];
            action.roles.push(RoleId::new("registrar"));
            blueprint.actions.push(action);
        }
        assert_ok!(Logic::install_blueprint(
            RuntimeOrigin::signed(registrar()),
            bounded(blueprint.encode())
        ));
        let deployment = blueprint.id().0;
        for (id, _, label) in classes.iter() {
            let manifest = api::action_manifest(deployment, id.as_bytes().to_vec())
                .expect("the action is declared");
            assert_eq!(manifest.class, label.to_vec());
            assert_eq!(manifest.reads, vec![b"parcel".to_vec()]);
            assert_eq!(manifest.writes, vec![b"parcel".to_vec()]);
        }
    });
}

#[test]
fn the_starting_licence_answers_live_with_production_permitted() {
    development_ext().execute_with(|| {
        let deployment = starting_deployment();
        let reply = api::licence_state(deployment).expect("the deployment holds a licence");
        assert_eq!(reply.state, b"live".to_vec());
        assert!(reply.production_permitted);
        assert!(reply.suspension_reason.is_empty());
        assert!(
            [&b"dapps"[..], b"enterprise", b"government"].contains(&reply.application.as_slice())
        );
        assert!(api::production_permitted());
    });
}

#[test]
fn the_operator_read_names_the_cerulea_operator() {
    development_ext().execute_with(|| {
        assert_eq!(
            api::operator_accounts(starting_deployment()),
            vec![cerulea_operator()]
        );
    });
}

#[test]
fn the_consensus_profile_carries_the_class_thresholds() {
    development_ext().execute_with(|| {
        let profile = api::consensus_profile();
        assert_eq!(profile.routine_percent, 51);
        assert_eq!(profile.financial_percent, 67);
        assert_eq!(profile.structural_percent, 80);
        assert_eq!(profile.constitutional_percent, 90);
        assert_eq!(
            profile.committee_size,
            cerulea_primitives::COMMITTEE_SIZE as u32
        );
        assert_eq!(profile.block_time_millis, crate::MILLI_SECS_PER_BLOCK);
        assert!(profile.epoch_length > 0);
    });
}

#[test]
fn a_proposal_answers_through_its_votes_its_decision_and_the_audit_log() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        Timestamp::set_timestamp(1_000);
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(registrar()),
            deployment,
            registrar(),
            10
        ));
        assert_ok!(Governance::propose(
            RuntimeOrigin::signed(registrar()),
            deployment,
            pallet_cerulea_governance::ProposalKind::PolicyAmendment,
            bounded(b"raise the transfer fee".to_vec()),
            60_000
        ));
        assert_ok!(Governance::propose(
            RuntimeOrigin::signed(registrar()),
            deployment,
            pallet_cerulea_governance::ProposalKind::SchemaChange,
            bounded(b"add a boundary field".to_vec()),
            60_000
        ));

        let open = api::proposals(deployment, 10);
        assert_eq!(open.len(), 2);
        // The newest proposal comes first.
        assert_eq!(open[0].id, 1);
        assert_eq!(open[1].id, 0);
        assert_eq!(open[1].kind, b"policy".to_vec());
        assert_eq!(open[0].kind, b"schema".to_vec());
        assert_eq!(open[1].status, b"open".to_vec());
        assert_eq!(open[1].opened_at, 1_000);
        assert_eq!(open[1].deadline, 60_000);
        assert_eq!(open[1].roll_weight, 10);
        assert_eq!(api::proposals(deployment, 1).len(), 1);

        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(registrar()),
            deployment,
            0,
            true
        ));
        let votes = api::proposal_votes(deployment, 0);
        assert_eq!(votes.len(), 1);
        assert_eq!(votes[0].voter, registrar());
        assert!(votes[0].approve);
        assert_eq!(votes[0].weight, 10);

        Timestamp::set_timestamp(70_000);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(registrar()),
            deployment,
            0
        ));
        let decided = api::proposal(deployment, 0).expect("the proposal exists");
        assert_eq!(decided.status, b"approved".to_vec());
        assert_eq!(decided.approve_weight, 10);
        assert_eq!(decided.voters, 1);
        assert!(api::executed_changes(deployment, 10).is_empty());

        Timestamp::set_timestamp(u64::from(u32::MAX));
        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(registrar()),
            deployment,
            0
        ));
        let executed = api::executed_changes(deployment, 10);
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0].status, b"executed".to_vec());
        assert!(executed[0].executed_block.is_some());

        // Every governance step wrote one entry to the audit log.
        let length = api::audit_length(deployment);
        assert!(length >= 5);
        let entries = api::audit_entries(deployment, 0, 100);
        assert_eq!(entries.len() as u64, length);
        for (index, entry) in entries.iter().enumerate() {
            assert_eq!(entry.sequence, index as u64);
        }
        assert!(entries
            .iter()
            .any(|entry| entry.event == b"proposal_executed".to_vec()));
        let tail = api::audit_entries(deployment, length - 1, 100);
        assert_eq!(tail.len(), 1);
        assert_eq!(api::audit_entries(deployment, 0, 2).len(), 2);
        assert!(api::audit_entries(deployment, length, 10).is_empty());

        let mine = api::audit_by_actor(deployment, registrar(), 100);
        assert!(!mine.is_empty());
        assert!(mine.iter().all(|entry| entry.actor == registrar()));
        assert_eq!(api::audit_by_actor(deployment, registrar(), 1).len(), 1);
        assert!(api::audit_by_actor(deployment, stranger(), 100).is_empty());
    });
}

/// The sixteen byte collection tag the custody module files a record under.
fn parcel_tag() -> [u8; 16] {
    let mut tag = [0u8; 16];
    tag[..6].copy_from_slice(b"parcel");
    tag
}

type Evolution = pallet_cerulea_evolution::Pallet<Runtime>;
type Oracle = pallet_cerulea_oracle::Pallet<Runtime>;
type Custody = pallet_cerulea_custody::Pallet<Runtime>;

#[test]
fn recorded_versions_answer_in_order_with_the_active_one_marked() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        Timestamp::set_timestamp(5_000);
        for (version, byte) in [(2u32, 2u8), (1, 1)] {
            assert_ok!(Evolution::record_version(
                RuntimeOrigin::signed(registrar()),
                deployment,
                version,
                [byte; 32],
                version,
                bounded(b"The registry adds the ward field.".to_vec())
            ));
        }
        let versions = api::versions(deployment);
        assert_eq!(versions.len(), 2);
        assert_eq!(versions[0].revision, 1);
        assert_eq!(versions[1].revision, 2);
        assert_eq!(versions[0].blueprint, [1u8; 32]);
        assert!(!versions[0].active);
        assert!(versions[1].active);
        assert_eq!(versions[1].recorded_at, 5_000);
        assert_eq!(versions[1].recorded_block, 1);
        assert_eq!(
            versions[1].summary,
            b"The registry adds the ward field.".to_vec()
        );
    });
}

#[test]
fn a_migration_plan_answers_through_its_rollback_window() {
    use pallet_cerulea_schema::migrate::{MigrationPlan, MigrationStep};
    development_ext().execute_with(|| {
        let deployment = install_registry();
        record_transfer(deployment, "12/3", "Anitha", 400);
        // A migration rewrites the registry under its steward. The first
        // account to register a definition becomes that steward.
        assert_ok!(pallet_cerulea_schema::Pallet::<Runtime>::register_type(
            RuntimeOrigin::signed(registrar()),
            deployment,
            bounded(
                TypeDef::Record(RecordDef {
                    id: TypeId::new("ward"),
                    fields: vec![FieldDef::new("name", TypeExpr::Text, "The ward name.")],
                    summary: "One ward of the district.".into(),
                })
                .encode()
            )
        ));
        Timestamp::set_timestamp(1_000);
        let plan = MigrationPlan::new("parcel", 1, "The register adds the ward of each parcel.")
            .with(MigrationStep::AddField {
                field: FieldName::new("ward"),
                ty: TypeExpr::Text,
                default: Value::Text("unassigned".into()),
                summary: "The ward the parcel sits in.".into(),
            });
        assert_ok!(Evolution::plan_migration(
            RuntimeOrigin::signed(registrar()),
            deployment,
            bounded(plan.encode()),
            60_000
        ));
        let planned = api::rollback_windows(deployment);
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].plan, 0);
        assert_eq!(planned[0].collection, b"parcel".to_vec());
        assert_eq!(planned[0].status, b"planned".to_vec());
        assert!(!planned[0].breaking);
        assert!(!planned[0].open);
        assert_eq!(planned[0].window_millis, 60_000);

        assert_ok!(Evolution::dry_run(
            RuntimeOrigin::signed(registrar()),
            deployment,
            0
        ));
        assert_eq!(
            api::rollback_windows(deployment)[0].status,
            b"checked".to_vec()
        );

        assert_ok!(Evolution::apply(
            RuntimeOrigin::signed(registrar()),
            deployment,
            0
        ));
        let applied = api::rollback_windows(deployment);
        assert_eq!(applied[0].status, b"applied".to_vec());
        assert_eq!(applied[0].from_version, 1);
        assert_eq!(applied[0].to_version, 2);
        assert!(applied[0].reversible);
        assert_eq!(applied[0].deadline, Some(61_000));
        assert!(applied[0].open);

        // The window closes once the clock passes the deadline.
        Timestamp::set_timestamp(61_000);
        assert!(!api::rollback_windows(deployment)[0].open);
    });
}

#[test]
fn an_attested_feed_value_answers_once_its_dispute_window_closes() {
    development_ext().execute_with(|| {
        let deployment = install_registry();
        Timestamp::set_timestamp(1_000);
        assert_ok!(Oracle::register_feed(
            RuntimeOrigin::signed(registrar()),
            deployment,
            name("land_rate"),
            BoundedVec::truncate_from(vec![registrar()]),
            1,
            2_000,
            bounded(b"The district land rate per square metre.".to_vec())
        ));
        assert_eq!(api::oracle_feeds(deployment), vec![b"land_rate".to_vec()]);
        assert_eq!(api::oracle_value(deployment, b"land_rate".to_vec()), None);

        assert_ok!(Oracle::submit_value(
            RuntimeOrigin::signed(registrar()),
            deployment,
            name("land_rate"),
            bounded(Value::Integer(4_200).encode())
        ));
        // The value stays pending while the dispute window runs.
        assert_eq!(api::oracle_value(deployment, b"land_rate".to_vec()), None);

        Timestamp::set_timestamp(3_000);
        let pending =
            api::oracle_value(deployment, b"land_rate".to_vec()).expect("the window closed");
        assert_eq!(pending.feed, b"land_rate".to_vec());
        assert_eq!(pending.value, Value::Integer(4_200).encode());
        assert_eq!(pending.attested_at, 1_000);

        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(registrar()),
            deployment,
            name("land_rate")
        ));
        let settled =
            api::oracle_value(deployment, b"land_rate".to_vec()).expect("the round settled");
        assert_eq!(settled.value, Value::Integer(4_200).encode());
        assert_eq!(settled.round, pending.round);
        assert_eq!(api::oracle_value(deployment, vec![0xff]), None);
    });
}

#[test]
fn a_custody_challenge_answers_in_the_history_of_its_validator() {
    use frame_support::traits::Hooks;
    development_ext().execute_with(|| {
        let alice = registrar();
        assert!(api::custody_history(alice.clone(), 10).is_empty());
        assert_ok!(Custody::commit_record(
            RuntimeOrigin::root(),
            parcel_tag(),
            bounded(b"12/3".to_vec()),
            bounded(b"Anitha holds 400 square metres".to_vec())
        ));
        <Custody as Hooks<u32>>::on_finalize(1);
        let mut issued = None;
        for block in 2u32..=20 {
            frame_system::Pallet::<Runtime>::set_block_number(block);
            <Custody as Hooks<u32>>::on_initialize(block);
            let history = api::custody_history(alice.clone(), 10);
            if issued.is_none() && !history.is_empty() {
                issued = Some(block);
                assert_eq!(history[0].validator, alice);
                assert_eq!(history[0].issued_block, block);
                assert_eq!(history[0].deadline_block, block + 3);
                assert!(!history[0].answered);
                assert!(!history[0].verified);
            }
        }
        let issued = issued.expect("the interval issues a challenge within twenty blocks");
        assert_eq!(issued % 10, 0);
        // The deadline passed with no answer. The chain closed the challenge
        // as missed and the open history holds it no longer.
        let history = api::custody_history(alice, 10);
        assert!(history.iter().all(|held| held.issued_block != issued));
        assert!(api::custody_history(stranger(), 10).is_empty());
    });
}
