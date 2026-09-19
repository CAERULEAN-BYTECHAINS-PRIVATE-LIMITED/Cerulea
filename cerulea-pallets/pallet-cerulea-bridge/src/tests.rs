//! The tests of the bridge module.
//!
//! The suite runs every target network through the same path. A message
//! arrives with a real proof, passes its verifier, lands once and runs a
//! real Blueprint action. A second attempt carrying the same identifier is
//! declined. A record then leaves the deployment with its inclusion proof,
//! and the far side of that proof is folded here the way a reader on the
//! far network folds it.

use crate::endpoint::{Checkpoint, EndpointKey, EndpointState, ListedHeaders, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, MessageFormat, NetworkKind, Travel, TrustAssumption,
};
use crate::mock::*;
use crate::networks::{avalanche, bitcoin, cosmos, evm, polkadot, rollup, solana};
use crate::verify::{self, Proof};
use crate::{
    fixtures, EndpointDeclaration, Endpoints, Error, Event, ExpirySchedule, Headers, MessageStatus,
    Outbound, OutboundBody, Pallet, Seen, Tips,
};
use cerulea_blueprint::{
    collections::{Collection, KeySelector, PurposeDeclaration},
    expression::Expr,
    types::{CollectionId, FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
    Blueprint, BlueprintMeta,
};
use codec::{Decode, Encode};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId, RecordKey};

/// The account every test acts as.
pub const ACTOR: u64 = 1;

/// The account that relays a message.
pub const RELAYER: u64 = 2;

/// The lifetime every endpoint of the suite carries, in milliseconds.
pub const LIFETIME: u64 = 600_000;

/// Turn text into a bounded name.
pub fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

/// Turn bytes into a bounded payload of one bound.
pub fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

// --- The Blueprint the suite installs -----------------------------------

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
        summary: "One land parcel in the register.".to_string(),
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
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

/// The canonical record key one survey number takes.
fn record_key(survey: &str) -> RecordKey {
    let bytes =
        pallet_cerulea_state::key_bytes_of(&parcel_collection(), &parcel(survey, "Anitha", 1_200))
            .expect("the record carries its key");
    RecordKey::try_from(bytes).expect("the key fits")
}

fn transfer_action() -> cerulea_blueprint::actions::Action {
    let mut action = cerulea_blueprint::actions::Action::new(
        "record_transfer",
        "The registrar records a transfer of one parcel.",
    );
    action.params = vec![cerulea_blueprint::actions::Param::new(
        "parcel",
        TypeExpr::Record(TypeId::new("parcel")),
        "The parcel the registrar records.",
    )];
    action.effects = vec![cerulea_blueprint::actions::Effect::Insert {
        collection: CollectionId::new("parcel"),
        key: KeySelector::Exact(Expr::field(Expr::bound("parcel"), "survey_number")),
        record: Expr::bound("parcel"),
    }];
    action
        .roles
        .push(cerulea_blueprint::types::RoleId::new("registrar"));
    action
}

fn bridge_blueprint() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "district land registry",
        "The registry records land parcels and their holders.",
        "Kollam District Administration",
    ));
    blueprint.types.push(parcel_type());
    blueprint.collections.push(parcel_collection());
    blueprint
        .identities
        .roles
        .push(cerulea_blueprint::identities::RoleDef::new(
            "registrar",
            None,
            "Registrar",
            "The registrar records land transfers.",
        ));
    blueprint.actions.push(transfer_action());
    blueprint
}

/// Install the Blueprint and grant the actor every capability it needs.
fn install() -> DeploymentId {
    let blueprint = bridge_blueprint();
    assert_ok!(Logic::install_blueprint(
        RuntimeOrigin::signed(ACTOR),
        bounded(blueprint.encode())
    ));
    let deployment = blueprint.id().0;

    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(ACTOR),
        ACTOR,
        [7u8; 32]
    ));
    assert_ok!(Identity::define_role(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        name("registrar"),
        None,
        BoundedVec::default(),
        BoundedVec::truncate_from(b"Registrar".to_vec())
    ));
    assert_ok!(Identity::assign_role(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        ACTOR,
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
            RuntimeOrigin::signed(ACTOR),
            deployment,
            name("registrar"),
            name("parcel"),
            capability,
            bounded(GrantPayload::everything().encode())
        ));
    }
    deployment
}

// --- Envelope and endpoint helpers --------------------------------------

/// The endpoint name of one network kind.
fn endpoint_name(kind: NetworkKind) -> BoundedName {
    name(kind.label())
}

/// Build one arriving envelope for one network kind.
fn inbound_envelope(
    deployment: DeploymentId,
    kind: NetworkKind,
    survey: &str,
    nonce: u64,
) -> MessageEnvelope {
    MessageEnvelope {
        deployment,
        endpoint: endpoint_name(kind),
        source: name(kind.label()),
        destination: name("cerulea"),
        party: bounded(vec![0xab; 20]),
        nonce,
        payload: bounded(parcel(survey, "Anitha", 1_200).encode()),
        class: WorkClass::Routine,
        expiry_millis: clock().saturating_add(LIFETIME),
    }
}

/// The declaration one endpoint of one network kind carries.
fn declaration(kind: NetworkKind, params: &VerifierParameters) -> EndpointDeclaration<u64> {
    EndpointDeclaration {
        network: name(kind.label()),
        direction: Travel::Inbound,
        both_ways: true,
        collection: name("parcel"),
        action: name("record_transfer"),
        format: MessageFormat::CanonicalValue,
        class: WorkClass::Financial,
        lifetime_millis: LIFETIME,
        params: params.clone(),
        executor: ACTOR,
    }
}

/// Register one endpoint of one network kind.
fn register(deployment: DeploymentId, kind: NetworkKind, params: &VerifierParameters) {
    assert_ok!(Bridge::register_endpoint(
        RuntimeOrigin::root(),
        deployment,
        endpoint_name(kind),
        declaration(kind, params),
    ));
}

/// Submit the header batch of one prepared far network side.
fn submit(deployment: DeploymentId, kind: NetworkKind, prepared: &fixtures::Prepared) {
    if let Some(batch) = prepared.headers.clone() {
        assert_ok!(Bridge::submit_headers(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            endpoint_name(kind),
            batch
        ));
    }
}

/// Prepare one network kind end to end and answer with the envelope and
/// the proof.
fn arrange(kind: NetworkKind, survey: &str, nonce: u64) -> (DeploymentId, MessageEnvelope, Proof) {
    let deployment = install();
    let envelope = inbound_envelope(deployment, kind, survey, nonce);
    let prepared = fixtures::prepare(kind, &envelope, clock());
    register(deployment, kind, &prepared.params);
    submit(deployment, kind, &prepared);
    (deployment, envelope, prepared.proof)
}

// --- The module contract ------------------------------------------------

#[test]
fn the_module_carries_one_verifier_for_every_target_network() {
    assert_eq!(NetworkKind::every().len(), 7);
    for kind in NetworkKind::every() {
        assert!(!kind.label().is_empty());
    }
}

#[test]
fn every_fault_names_a_fix() {
    for fault in [
        BridgeFault::KindMismatch,
        BridgeFault::CheckpointAbsent,
        BridgeFault::ChainBroken,
        BridgeFault::WorkTooLow,
        BridgeFault::TooFewConfirmations,
        BridgeFault::SignatureInvalid,
        BridgeFault::ThresholdUnmet,
        BridgeFault::SignerUnknown,
        BridgeFault::SignerRepeated,
        BridgeFault::MalformedEncoding,
        BridgeFault::NodeHashMismatch,
        BridgeFault::ValueAbsent,
        BridgeFault::CommitmentMismatch,
        BridgeFault::RootUnknown,
        BridgeFault::ChallengeWindowOpen,
        BridgeFault::ProofTooLarge,
    ] {
        assert!(fault.fix().ends_with('.'), "{} names a fix", fault.label());
        assert!(fault.fix().len() > 20);
    }
}

#[test]
fn an_identifier_covers_every_field_that_separates_two_messages() {
    new_test_ext().execute_with(|| {
        let deployment = [1u8; 32];
        let first = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let mut second = first.clone();
        second.nonce = 1;
        assert_ne!(first.id(), second.id());
        let mut third = first.clone();
        third.payload = bounded(vec![9u8; 8]);
        assert_ne!(first.id(), third.id());
        let mut fourth = first.clone();
        fourth.party = bounded(vec![0xcd; 20]);
        assert_ne!(first.id(), fourth.id());
        assert_eq!(first.id(), first.clone().id());
    });
}

#[test]
fn a_commitment_covers_the_class_and_the_expiry() {
    new_test_ext().execute_with(|| {
        let deployment = [1u8; 32];
        let first = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let mut second = first.clone();
        second.class = WorkClass::Structural;
        assert_eq!(first.id(), second.id());
        assert_ne!(first.commitment(), second.commitment());
    });
}

// --- The endpoint registry ---------------------------------------------

#[test]
fn a_registered_endpoint_holds_its_kind_and_its_parameters() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        register(deployment, NetworkKind::Cosmos, &prepared.params);

        let held = Pallet::<Test>::endpoint(&deployment, &endpoint_name(NetworkKind::Cosmos))
            .expect("the endpoint registered");
        assert_eq!(held.kind, NetworkKind::Cosmos);
        assert_eq!(held.params.kind(), NetworkKind::Cosmos);
        assert_eq!(held.format, MessageFormat::CanonicalValue);
        assert_eq!(held.class, WorkClass::Financial);
        assert!(held.live);
        assert!(held.takes(Travel::Inbound));
        assert!(held.takes(Travel::Outbound));
        assert_eq!(
            held.params.checkpoint(),
            Some(Checkpoint {
                height: fixtures::START_HEIGHT,
                hash: [0xbb; 32]
            })
        );
        assert_eq!(
            Pallet::<Test>::endpoint_names(&deployment),
            vec![endpoint_name(NetworkKind::Cosmos)]
        );
    });
}

#[test]
fn a_repeated_endpoint_name_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        register(deployment, NetworkKind::Cosmos, &prepared.params);
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::root(),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                EndpointDeclaration {
                    lifetime_millis: LIFETIME,
                    class: WorkClass::Routine,
                    ..declaration(NetworkKind::Cosmos, &prepared.params)
                },
            ),
            Error::<Test>::EndpointRepeated
        );
    });
}

#[test]
fn an_endpoint_change_without_approval_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                EndpointDeclaration {
                    lifetime_millis: LIFETIME,
                    class: WorkClass::Routine,
                    ..declaration(NetworkKind::Cosmos, &prepared.params)
                },
            ),
            Error::<Test>::ApprovalMissing
        );
    });
}

#[test]
fn a_retired_endpoint_stops_taking_messages() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_ok!(Bridge::retire_endpoint(
            RuntimeOrigin::root(),
            deployment,
            endpoint_name(NetworkKind::Cosmos)
        ));
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, proof),
            Error::<Test>::EndpointRetired
        );
    });
}

#[test]
fn a_lifetime_past_the_limit_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::root(),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                EndpointDeclaration {
                    lifetime_millis: u64::MAX,
                    class: WorkClass::Routine,
                    ..declaration(NetworkKind::Cosmos, &prepared.params)
                },
            ),
            Error::<Test>::LifetimeTooLong
        );
    });
}

// --- Replay protection --------------------------------------------------

#[test]
fn a_bridge_message_replays_once_and_is_rejected_on_the_second_attempt() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::EthereumVirtualMachine, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof.clone()
        ));
        assert!(Pallet::<Test>::has_seen(
            &deployment,
            &endpoint_name(NetworkKind::EthereumVirtualMachine),
            &envelope.id()
        ));
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, proof),
            Error::<Test>::MessageReplayed
        );
    });
}

#[test]
fn a_replay_entry_survives_cancellation() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::EthereumVirtualMachine, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof.clone()
        ));
        let key = EndpointKey::new(
            deployment,
            endpoint_name(NetworkKind::EthereumVirtualMachine),
        );
        assert!(Seen::<Test>::get(&key, envelope.id()).is_some());
        advance_clock(LIFETIME * 2);
        roll_one_block();
        assert!(Seen::<Test>::get(&key, envelope.id()).is_some());
    });
}

#[test]
fn an_expired_message_declines_before_its_proof_runs() {
    new_test_ext().execute_with(|| {
        let (_, envelope, proof) = arrange(NetworkKind::EthereumVirtualMachine, "12/3", 0);
        advance_clock(LIFETIME * 2);
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, proof),
            Error::<Test>::MessageExpired
        );
    });
}

// --- One acceptance path per target network -----------------------------

/// Deliver one message of one network kind and check the record landed.
fn accepts(kind: NetworkKind) {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof
        ));
        let held =
            Pallet::<Test>::inbound_message(&deployment, &endpoint_name(kind), &envelope.id())
                .expect("the message landed");
        assert_eq!(held.status, MessageStatus::Executed);
        assert_eq!(held.effects, 1);
        assert_eq!(
            State::record(
                &deployment,
                &CollectionId::new("parcel"),
                &record_key("12/3")
            ),
            Some(parcel("12/3", "Anitha", 1_200))
        );
    });
}

/// Deliver one message of one network kind with a changed envelope.
fn declines_a_changed_envelope(kind: NetworkKind) {
    new_test_ext().execute_with(|| {
        let (_, envelope, proof) = arrange(kind, "12/3", 0);
        let mut changed = envelope;
        changed.payload = bounded(parcel("99/9", "Ravi", 10).encode());
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), changed, proof),
            Error::<Test>::ProofDeclined
        );
    });
}

#[test]
fn the_ethereum_virtual_machine_verifier_accepts_a_storage_proof() {
    accepts(NetworkKind::EthereumVirtualMachine);
}

#[test]
fn the_ethereum_virtual_machine_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::EthereumVirtualMachine);
}

#[test]
fn the_ethereum_virtual_machine_verifier_accepts_a_receipt_proof() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::EthereumVirtualMachine;
        let deployment = install();
        let envelope = inbound_envelope(deployment, kind, "12/3", 0);
        let prepared = fixtures::evm_side(&envelope, true, fixtures::first_base(kind));
        register(deployment, kind, &prepared.params);
        submit(deployment, kind, &prepared);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            prepared.proof
        ));
    });
}

#[test]
fn the_cosmos_verifier_accepts_a_signed_commit() {
    accepts(NetworkKind::Cosmos);
}

#[test]
fn the_cosmos_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Cosmos);
}

#[test]
fn the_solana_verifier_accepts_an_attested_slot() {
    accepts(NetworkKind::Solana);
}

#[test]
fn the_solana_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Solana);
}

#[test]
fn the_polkadot_verifier_accepts_a_settled_block() {
    accepts(NetworkKind::Polkadot);
}

#[test]
fn the_polkadot_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Polkadot);
}

#[test]
fn the_avalanche_verifier_accepts_a_signed_message() {
    accepts(NetworkKind::Avalanche);
}

#[test]
fn the_avalanche_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Avalanche);
}

#[test]
fn the_bitcoin_verifier_accepts_a_confirmed_transaction() {
    accepts(NetworkKind::Bitcoin);
}

#[test]
fn the_bitcoin_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Bitcoin);
}

#[test]
fn the_rollup_verifier_accepts_a_settled_output_root() {
    accepts(NetworkKind::Rollup);
}

#[test]
fn the_rollup_verifier_declines_a_changed_envelope() {
    declines_a_changed_envelope(NetworkKind::Rollup);
}

// --- Verifier detail ----------------------------------------------------

#[test]
fn a_proof_of_another_network_kind_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let cosmos_side = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        register(deployment, NetworkKind::Cosmos, &cosmos_side.params);
        let other = fixtures::prepare(NetworkKind::Avalanche, &envelope, clock());
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, other.proof),
            Error::<Test>::KindMismatch
        );
    });
}

#[test]
fn a_proof_naming_a_height_the_endpoint_lacks_declines() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::EthereumVirtualMachine;
        let deployment = install();
        let envelope = inbound_envelope(deployment, kind, "12/3", 0);
        let prepared = fixtures::prepare(kind, &envelope, clock());
        register(deployment, kind, &prepared.params);
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, prepared.proof),
            Error::<Test>::ProofDeclined
        );
    });
}

#[test]
fn a_header_batch_of_another_network_kind_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Bitcoin, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Bitcoin, &envelope, clock());
        register(deployment, NetworkKind::Bitcoin, &prepared.params);
        let other = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &envelope, clock());
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                endpoint_name(NetworkKind::Bitcoin),
                other.headers.expect("the batch exists")
            ),
            Error::<Test>::BatchMismatch
        );
    });
}

#[test]
fn an_accepted_header_moves_the_checkpoint_forward() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::EthereumVirtualMachine;
        let (deployment, _, _) = arrange(kind, "12/3", 0);
        let key = EndpointKey::new(deployment, endpoint_name(kind));
        assert_eq!(Tips::<Test>::get(&key), fixtures::START_HEIGHT + 1);
        let held = Endpoints::<Test>::get(&key).expect("the endpoint registered");
        assert_eq!(
            held.params.checkpoint().map(|found| found.height),
            Some(fixtures::START_HEIGHT + 1)
        );
        assert!(Headers::<Test>::get(&key, fixtures::START_HEIGHT + 1).is_some());
    });
}

#[test]
fn a_bitcoin_header_carrying_too_little_work_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Bitcoin, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Bitcoin, &envelope, clock());
        let mut params = match prepared.params.clone() {
            VerifierParameters::Bitcoin(held) => held,
            _ => panic!("the fixture carries Bitcoin parameters"),
        };
        params.work_limit = [0u8; 32];
        params.work_limit[31] = 1;
        register(
            deployment,
            NetworkKind::Bitcoin,
            &VerifierParameters::Bitcoin(params),
        );
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                endpoint_name(NetworkKind::Bitcoin),
                prepared.headers.expect("the batch exists")
            ),
            Error::<Test>::HeadersDeclined
        );
    });
}

#[test]
fn a_bitcoin_header_meets_its_own_target() {
    let target = bitcoin::unpack_target(fixtures::BITCOIN_BITS).expect("the target unpacks");
    let encoded = fixtures::mine(&[1u8; 32], &[2u8; 32], 1_700_000_000);
    let header = bitcoin::read_header(&encoded).expect("the header reads");
    assert!(bitcoin::meets_target(&header.hash, &target));
    assert_eq!(header.parent, [1u8; 32]);
    assert_eq!(header.merkle_root, [2u8; 32]);
}

#[test]
fn the_packed_target_of_the_first_bitcoin_block_unpacks() {
    let target = bitcoin::unpack_target(0x1d00_ffff).expect("the target unpacks");
    let mut expected = [0u8; 32];
    expected[4] = 0xff;
    expected[5] = 0xff;
    assert_eq!(target, expected);
}

#[test]
fn a_cosmos_commit_below_two_thirds_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Cosmos, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Cosmos, &envelope, clock());
        let params = prepared.params.clone();
        let mut proof = match prepared.proof {
            Proof::Cosmos(held) => held,
            _ => panic!("the fixture carries a Cosmos proof"),
        };
        proof.signatures = BoundedVec::truncate_from(proof.signatures[..2].to_vec());
        let headers = ListedHeaders::default();
        let state = EndpointState::new(&params, MessageFormat::CanonicalValue, &headers, clock());
        assert_eq!(
            verify::verify(&envelope, &Proof::Cosmos(proof), &state),
            Err(BridgeFault::ThresholdUnmet)
        );
    });
}

#[test]
fn a_repeated_signer_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let envelope = inbound_envelope(deployment, NetworkKind::Avalanche, "12/3", 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &envelope, clock());
        let params = prepared.params.clone();
        let mut proof = match prepared.proof {
            Proof::Avalanche(held) => held,
            _ => panic!("the fixture carries an Avalanche proof"),
        };
        let first = proof.signatures[0].clone();
        proof.signatures = BoundedVec::truncate_from(vec![first.clone(), first]);
        let headers = ListedHeaders::default();
        let state = EndpointState::new(&params, MessageFormat::CanonicalValue, &headers, clock());
        assert_eq!(
            verify::verify(&envelope, &Proof::Avalanche(proof), &state),
            Err(BridgeFault::SignerRepeated)
        );
    });
}

#[test]
fn a_rollup_output_inside_its_window_declines() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Rollup;
        let deployment = install();
        let envelope = inbound_envelope(deployment, kind, "12/3", 0);
        let prepared = fixtures::prepare(kind, &envelope, clock());
        register(deployment, kind, &prepared.params);
        submit(deployment, kind, &prepared);
        set_clock(clock().saturating_sub(fixtures::ROLLUP_WINDOW));
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, prepared.proof),
            Error::<Test>::ProofDeclined
        );
    });
}

#[test]
fn a_solana_slot_carries_the_bank_hash_of_its_parts() {
    let header = solana::SlotHeader {
        slot: 7,
        parent_bank_hash: [1u8; 32],
        accounts_delta_hash: [2u8; 32],
        signature_count: 9,
        last_blockhash: [3u8; 32],
    };
    assert_eq!(
        header.bank_hash(),
        solana::bank_hash(&[1u8; 32], &[2u8; 32], 9, &[3u8; 32])
    );
}

#[test]
fn a_polkadot_header_reads_its_number_and_its_state_root() {
    let encoded = polkadot::encode_header(&[1u8; 32], 4_096, &[2u8; 32], &[3u8; 32]);
    let header = polkadot::read_header(&encoded).expect("the header reads");
    assert_eq!(header.parent, [1u8; 32]);
    assert_eq!(header.number, 4_096);
    assert_eq!(header.state_root, [2u8; 32]);
    assert_eq!(header.extrinsics_root, [3u8; 32]);
    assert_eq!(header.hash, sp_crypto_hashing::blake2_256(&encoded));
}

#[test]
fn an_avalanche_message_hash_covers_every_field() {
    let first = avalanche::message_hash(1, &[1u8; 32], b"sender", &[2u8; 32]);
    let second = avalanche::message_hash(2, &[1u8; 32], b"sender", &[2u8; 32]);
    let third = avalanche::message_hash(1, &[9u8; 32], b"sender", &[2u8; 32]);
    let fourth = avalanche::message_hash(1, &[1u8; 32], b"other", &[2u8; 32]);
    assert_ne!(first, second);
    assert_ne!(first, third);
    assert_ne!(first, fourth);
}

#[test]
fn a_cosmos_vote_takes_a_length_prefix() {
    let parts = cosmos::PartHeader {
        total: 1,
        hash: [4u8; 32],
    };
    let payload = cosmos::vote_payload(b"chain", 9, 0, &[5u8; 32], &parts, 1_700_000_000, 0);
    let (length, used) = read_varint(&payload);
    assert_eq!(length as usize, payload.len() - used);
}

/// Read one variable length number from the front of a byte string.
fn read_varint(bytes: &[u8]) -> (u64, usize) {
    let mut value = 0u64;
    let mut shift = 0u32;
    for (position, byte) in bytes.iter().enumerate() {
        value |= ((*byte & 0x7f) as u64) << shift;
        if byte & 0x80 == 0 {
            return (value, position + 1);
        }
        shift += 7;
    }
    (value, bytes.len())
}

#[test]
fn an_evm_receipt_reads_its_log_back() {
    let log = evm::EvmLog {
        address: fixtures::EVM_CONTRACT,
        topics: vec![fixtures::EVM_TOPIC, [9u8; 32]],
        data: vec![7u8; 32],
    };
    let encoded = evm::encode_receipt(1, 21_000, &[log.clone()]);
    assert_eq!(evm::read_log(&encoded, 0), Ok(log));
    assert_eq!(evm::read_log(&encoded, 1), Err(BridgeFault::ValueAbsent));
}

#[test]
fn a_rollup_output_root_covers_its_four_words() {
    let first = rollup::output_root(&[0u8; 32], &[1u8; 32], &[2u8; 32], &[3u8; 32]);
    let second = rollup::output_root(&[0u8; 32], &[9u8; 32], &[2u8; 32], &[3u8; 32]);
    assert_ne!(first, second);
    assert_eq!(
        first,
        sp_crypto_hashing::keccak_256(&rollup::output_preimage(
            &[0u8; 32], &[1u8; 32], &[2u8; 32], &[3u8; 32]
        ))
    );
}

// --- Class assignment and the actor context -----------------------------

#[test]
fn an_arriving_message_runs_under_the_higher_class() {
    assert_eq!(
        crate::inbound::class_of(WorkClass::Routine, WorkClass::Financial),
        WorkClass::Financial
    );
    assert_eq!(
        crate::inbound::class_of(WorkClass::Constitutional, WorkClass::Routine),
        WorkClass::Constitutional
    );
}

#[test]
fn an_arriving_message_records_its_source_as_the_actor_context() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Cosmos;
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof
        ));
        let actor = Pallet::<Test>::source_account(&envelope.source, envelope.party.as_slice());
        let log = Audit::query(&deployment, 0, 64);
        let held = log
            .iter()
            .find(|entry| entry.event == "message.executed")
            .expect("the log holds the execution");
        assert_eq!(held.actor, actor);
        assert_eq!(held.class, WorkClass::Financial);
        assert_eq!(held.module, "bridge");
        assert!(log.iter().any(|entry| entry.event == "message.relayed"));
    });
}

#[test]
fn a_source_account_stays_the_same_for_one_sender() {
    new_test_ext().execute_with(|| {
        let first = Pallet::<Test>::source_account(&name("cosmos"), b"party");
        let second = Pallet::<Test>::source_account(&name("cosmos"), b"party");
        let third = Pallet::<Test>::source_account(&name("solana"), b"party");
        assert_eq!(first, second);
        assert_ne!(first, third);
    });
}

// --- Outbound production ------------------------------------------------

/// Deliver one message, then produce the outbound reply covering the
/// record the message wrote.
fn round_trip(kind: NetworkKind) {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);

        // The far network sends a message in.
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof.clone()
        ));
        let held =
            Pallet::<Test>::inbound_message(&deployment, &endpoint_name(kind), &envelope.id())
                .expect("the message landed");
        assert_eq!(held.status, MessageStatus::Executed);

        // Cerulea sends the record back out with its inclusion proof.
        let held_key = record_key("12/3");
        assert_ok!(Bridge::dispatch_outbound(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(kind),
            held_key.clone(),
            bounded(vec![0xab; 20]),
        ));

        // The far side folds the proof the way a reader there folds it.
        let key = EndpointKey::new(deployment, endpoint_name(kind));
        let (id, outbound) = Outbound::<Test>::iter_prefix(&key)
            .next()
            .expect("the outbound message exists");
        assert_eq!(outbound.status, MessageStatus::Pending);
        let body_bytes = Pallet::<Test>::outbound_body(&deployment, &endpoint_name(kind), &id)
            .expect("the body exists");
        let body = OutboundBody::decode(&mut &body_bytes[..]).expect("the body decodes");
        assert_eq!(body.key, held_key);
        assert!(
            body.proof.verifies(&body.root),
            "the far side folds the inclusion proof to the collection root"
        );
        assert_eq!(
            body.root,
            <State as pallet_cerulea_state::RecordStore>::root(
                &deployment,
                &CollectionId::new("parcel")
            )
            .expect("the root exists")
        );

        // The driver reports the far network reference.
        assert_ok!(Bridge::confirm_outbound(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            endpoint_name(kind),
            id,
            [0x7e; 32],
        ));
        let settled = Pallet::<Test>::outbound_message(&deployment, &endpoint_name(kind), &id)
            .expect("the outbound message exists");
        assert_eq!(settled.status, MessageStatus::Delivered);
        assert_eq!(settled.reference, [0x7e; 32]);

        // A reply arrives with its own proof and runs its own action.
        let reply = inbound_envelope(deployment, kind, "45/6", 1);
        let base = Endpoints::<Test>::get(&key)
            .expect("the endpoint registered")
            .params
            .checkpoint()
            .unwrap_or_else(|| fixtures::first_base(kind));
        let prepared = fixtures::prepare_from(kind, &reply, clock(), base);
        if let Some(batch) = prepared.headers.clone() {
            assert_ok!(Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                endpoint_name(kind),
                batch
            ));
        }
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            reply.clone(),
            prepared.proof
        ));
        assert_eq!(
            State::record(
                &deployment,
                &CollectionId::new("parcel"),
                &record_key("45/6")
            ),
            Some(parcel("45/6", "Anitha", 1_200))
        );

        // The first message never runs twice.
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), envelope, proof),
            Error::<Test>::MessageReplayed
        );
    });
}

#[test]
fn the_ethereum_virtual_machine_round_trip_completes() {
    round_trip(NetworkKind::EthereumVirtualMachine);
}

#[test]
fn the_cosmos_round_trip_completes() {
    round_trip(NetworkKind::Cosmos);
}

#[test]
fn the_solana_round_trip_completes() {
    round_trip(NetworkKind::Solana);
}

#[test]
fn the_polkadot_round_trip_completes() {
    round_trip(NetworkKind::Polkadot);
}

#[test]
fn the_avalanche_round_trip_completes() {
    round_trip(NetworkKind::Avalanche);
}

#[test]
fn the_bitcoin_round_trip_completes() {
    round_trip(NetworkKind::Bitcoin);
}

#[test]
fn the_rollup_round_trip_completes() {
    round_trip(NetworkKind::Rollup);
}

#[test]
fn an_outbound_message_over_a_missing_record_declines() {
    new_test_ext().execute_with(|| {
        let (deployment, _, _) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                record_key("nothing"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

// --- Expiry and cancellation --------------------------------------------

#[test]
fn an_expired_outbound_message_is_cancelled_by_the_sweep() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));
        assert_ok!(Bridge::dispatch_outbound(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(NetworkKind::Cosmos),
            record_key("12/3"),
            bounded(vec![0xab; 20]),
        ));
        let key = EndpointKey::new(deployment, endpoint_name(NetworkKind::Cosmos));
        let (id, _) = Outbound::<Test>::iter_prefix(&key)
            .next()
            .expect("the outbound message exists");

        advance_clock(LIFETIME * 2);
        for _ in 0..12 {
            roll_one_block();
        }

        let held = Outbound::<Test>::get(&key, id).expect("the outbound message exists");
        assert_eq!(held.status, MessageStatus::Cancelled);
        assert!(System::events().iter().any(|record| matches!(
            record.event,
            RuntimeEvent::Bridge(Event::MessageCancelled { .. })
        )));
        let log = Audit::query(&deployment, 0, 128);
        assert!(log.iter().any(|entry| entry.event == "message.cancelled"));
    });
}

#[test]
fn the_sweep_stays_inside_its_budget() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));
        let key = EndpointKey::new(deployment, endpoint_name(NetworkKind::Cosmos));
        for step in 0..20u8 {
            let record = parcel(&alloc::format!("a{step}"), "Anitha", 1);
            assert_ok!(Logic::execute_action(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("record_transfer"),
                bounded(
                    vec![(cerulea_blueprint::types::ParamName::new("parcel"), record)].encode()
                )
            ));
            assert_ok!(Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                record_key(&alloc::format!("a{step}")),
                bounded(vec![0xab; 20]),
            ));
        }
        let bucket = Pallet::<Test>::bucket_of(clock().saturating_add(LIFETIME));
        assert_eq!(ExpirySchedule::<Test>::get(bucket).len(), 20);

        advance_clock(LIFETIME * 2);
        roll_one_block();
        let left = ExpirySchedule::<Test>::get(bucket).len();
        assert_eq!(left, 4, "one block cancels the budget of sixteen");
        roll_one_block();
        assert_eq!(ExpirySchedule::<Test>::get(bucket).len(), 0);
        let cancelled = Outbound::<Test>::iter_prefix(&key)
            .filter(|(_, held)| held.status == MessageStatus::Cancelled)
            .count();
        assert_eq!(cancelled, 20);
    });
}

#[test]
fn a_message_inside_its_lifetime_resists_cancellation() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));
        assert_ok!(Bridge::dispatch_outbound(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(NetworkKind::Cosmos),
            record_key("12/3"),
            bounded(vec![0xab; 20]),
        ));
        let key = EndpointKey::new(deployment, endpoint_name(NetworkKind::Cosmos));
        let (id, _) = Outbound::<Test>::iter_prefix(&key)
            .next()
            .expect("the outbound message exists");
        assert_noop!(
            Bridge::cancel_message(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                endpoint_name(NetworkKind::Cosmos),
                id,
                Travel::Outbound
            ),
            Error::<Test>::MessageExpired
        );
    });
}

#[test]
fn an_expired_message_cancels_on_request() {
    new_test_ext().execute_with(|| {
        let (deployment, envelope, proof) = arrange(NetworkKind::Cosmos, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));
        assert_ok!(Bridge::dispatch_outbound(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(NetworkKind::Cosmos),
            record_key("12/3"),
            bounded(vec![0xab; 20]),
        ));
        let key = EndpointKey::new(deployment, endpoint_name(NetworkKind::Cosmos));
        let (id, _) = Outbound::<Test>::iter_prefix(&key)
            .next()
            .expect("the outbound message exists");
        advance_clock(LIFETIME * 2);
        assert_ok!(Bridge::cancel_message(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(NetworkKind::Cosmos),
            id,
            Travel::Outbound
        ));
        let held = Outbound::<Test>::get(&key, id).expect("the outbound message exists");
        assert_eq!(held.status, MessageStatus::Cancelled);
    });
}

// --- The anchoring path -------------------------------------------------

#[test]
fn a_checkpoint_anchors_its_commitment_into_the_far_network() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Bitcoin;
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));

        // The anchored set of the framework pallet is the set this path
        // commits externally.
        let checkpoint =
            cerulea_primitives::checkpoint::Checkpoint::new(2, 2_700, [7u8; 32], [8u8; 32]);
        assert_ok!(Bridge::request_checkpoint_anchor(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(kind),
            checkpoint,
        ));

        let key = EndpointKey::new(deployment, endpoint_name(kind));
        let record = crate::Anchors::<Test>::get(&key, 0).expect("the anchor exists");
        assert_eq!(record.status, crate::anchor::AnchorStatus::Requested);
        assert_eq!(
            record.root,
            checkpoint.commitment(),
            "the anchor commits the checkpoint commitment"
        );
        assert_eq!(
            record.collection.as_slice(),
            crate::anchor::CHECKPOINT_RECORD_NAME
        );

        // The driver writes that value into a data carrying output. The value
        // reads back from the script byte for byte.
        let script = bitcoin::write_anchor(fixtures::BITCOIN_MARKER, &record.root);
        assert_eq!(
            bitcoin::read_anchor(&script, fixtures::BITCOIN_MARKER),
            Some(checkpoint.commitment())
        );
    });
}

#[test]
fn two_checkpoints_anchor_two_commitments() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Bitcoin;
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));

        let one = cerulea_primitives::checkpoint::Checkpoint::new(0, 900, [1u8; 32], [2u8; 32]);
        let two = cerulea_primitives::checkpoint::Checkpoint::new(1, 1_800, [3u8; 32], [4u8; 32]);
        assert_ok!(Bridge::request_checkpoint_anchor(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(kind),
            one,
        ));
        assert_ok!(Bridge::request_checkpoint_anchor(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(kind),
            two,
        ));

        let key = EndpointKey::new(deployment, endpoint_name(kind));
        let first = crate::Anchors::<Test>::get(&key, 0).expect("the first anchor exists");
        let second = crate::Anchors::<Test>::get(&key, 1).expect("the second anchor exists");
        assert_eq!(first.root, one.commitment());
        assert_eq!(second.root, two.commitment());
        assert_ne!(first.root, second.root);
    });
}

#[test]
fn a_state_root_anchors_into_the_far_network_and_proves_back() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Bitcoin;
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope,
            proof
        ));

        assert_ok!(Bridge::request_anchor(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            endpoint_name(kind),
            name("parcel"),
        ));
        let key = EndpointKey::new(deployment, endpoint_name(kind));
        let anchor = crate::Anchors::<Test>::get(&key, 0).expect("the anchor exists");
        assert_eq!(anchor.status, crate::anchor::AnchorStatus::Requested);
        assert_eq!(
            anchor.root,
            <State as pallet_cerulea_state::RecordStore>::root(
                &deployment,
                &CollectionId::new("parcel")
            )
            .expect("the root exists")
        );

        // The driver writes the anchor into a data carrying output.
        let transaction = bitcoin::encode_anchor_transaction(
            &[0x6b; 32],
            0,
            fixtures::BITCOIN_MARKER,
            &anchor.root,
            10_000,
            &[0x76, 0xa9, 0x14],
        );
        let read = bitcoin::read_transaction(&transaction).expect("the transaction reads");
        let mut internal = read.txid;
        internal.reverse();
        let mut merkle_root = internal;
        merkle_root.reverse();

        let parent = Headers::<Test>::get(&key, Tips::<Test>::get(&key))
            .expect("the tip header exists")
            .hash;
        let first = fixtures::mine(&parent, &merkle_root, 1_700_001_200);
        let first_read = bitcoin::read_header(&first).expect("the header reads");
        let second = fixtures::mine(&first_read.hash, &[0x3f; 32], 1_700_001_800);
        assert_ok!(Bridge::submit_headers(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            endpoint_name(kind),
            crate::HeaderBatch::Bitcoin(bitcoin::BitcoinHeaderBatch {
                headers: BoundedVec::try_from(vec![
                    BoundedVec::try_from(first).expect("the header fits"),
                    BoundedVec::try_from(second).expect("the header fits"),
                ])
                .expect("two headers fit"),
            })
        ));

        let height = Tips::<Test>::get(&key) - 1;
        assert_ok!(Bridge::confirm_anchor(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            endpoint_name(kind),
            0,
            crate::anchor::AnchorEvidence {
                height,
                transaction: BoundedVec::try_from(transaction).expect("the transaction fits"),
                position: 0,
                branch: BoundedVec::default(),
                output_index: 0,
            }
        ));
        let settled = crate::Anchors::<Test>::get(&key, 0).expect("the anchor exists");
        assert_eq!(settled.status, crate::anchor::AnchorStatus::Confirmed);
        assert_eq!(settled.height, height);
        assert_eq!(settled.confirmations, 2);
    });
}

#[test]
fn an_anchor_script_reads_its_value_back() {
    let script = bitcoin::write_anchor(fixtures::BITCOIN_MARKER, &[9u8; 32]);
    assert_eq!(
        bitcoin::read_anchor(&script, fixtures::BITCOIN_MARKER),
        Some([9u8; 32])
    );
    assert_eq!(bitcoin::read_anchor(&script, b"XXXX"), None);
}

// --- The trust assumptions ----------------------------------------------

#[test]
fn every_endpoint_states_what_its_acceptance_rests_on() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        for kind in NetworkKind::every() {
            let envelope = inbound_envelope(deployment, kind, "12/3", 0);
            let prepared = fixtures::prepare(kind, &envelope, clock());
            let trust = prepared.params.trust();
            assert!(trust.sentence().ends_with('.'));
            let expected = match kind {
                NetworkKind::EthereumVirtualMachine | NetworkKind::Solana => {
                    matches!(trust, TrustAssumption::RelayerAttestation { .. })
                }
                NetworkKind::Cosmos | NetworkKind::Polkadot | NetworkKind::Avalanche => {
                    matches!(trust, TrustAssumption::ValidatorThreshold { .. })
                }
                NetworkKind::Bitcoin => matches!(trust, TrustAssumption::AccumulatedWork { .. }),
                NetworkKind::Rollup => {
                    matches!(trust, TrustAssumption::SettledAfterChallenge { .. })
                }
            };
            assert!(expected, "the {} endpoint states its trust", kind.label());
        }
    });
}

#[test]
fn an_accepted_message_carries_the_trust_of_its_network() {
    new_test_ext().execute_with(|| {
        let kind = NetworkKind::Bitcoin;
        let (deployment, envelope, proof) = arrange(kind, "12/3", 0);
        assert_ok!(Bridge::deliver_message(
            RuntimeOrigin::signed(RELAYER),
            envelope.clone(),
            proof
        ));
        let held =
            Pallet::<Test>::inbound_message(&deployment, &endpoint_name(kind), &envelope.id())
                .expect("the message landed");
        assert_eq!(
            held.trust,
            TrustAssumption::AccumulatedWork { confirmations: 2 }
        );
    });
}

#[test]
fn two_thirds_plus_one_counts_in_whole_numbers() {
    use crate::endpoint::{reaches_percent, reaches_two_thirds};
    assert!(!reaches_two_thirds(2, 3));
    assert!(reaches_two_thirds(3, 4));
    assert!(!reaches_two_thirds(2, 4));
    assert!(!reaches_two_thirds(0, 0));
    assert!(reaches_percent(67, 100, 67));
    assert!(!reaches_percent(66, 100, 67));
}
