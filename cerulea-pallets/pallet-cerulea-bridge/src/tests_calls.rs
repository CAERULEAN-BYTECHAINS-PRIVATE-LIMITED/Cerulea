//! Reject paths and stored effects of the bridge calls.
//!
//! Every test drives the bridge module through its calls on the mock
//! runtime. The endpoints carry the real far network fixtures. Each reject
//! test asserts the exact error the call returns. Each success test asserts
//! the stored record, the audit entry and the event the call produced.

use crate::anchor::{AnchorEvidence, AnchorStatus};
use crate::endpoint::{Checkpoint, EndpointKey, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, MessageFormat, NetworkKind, Travel, TrustAssumption,
};
use crate::mock::*;
use crate::networks::bitcoin;
use crate::verify::Proof;
use crate::weights::{CeruleaWeight, WeightInfo};
use crate::{
    fixtures, AnchorNonce, Anchors, EndpointDeclaration, EndpointNames, Endpoints, Error, Event,
    ExpirySchedule, Floors, HeaderBatch, Headers, Inbound, MessageStatus, Outbound, Pallet,
    SweepCursor, Tips,
};
use cerulea_blueprint::{
    collections::{Collection, KeySelector, PurposeDeclaration},
    expression::Expr,
    types::{CollectionId, FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
    Blueprint, BlueprintMeta,
};
use codec::Encode;
use frame_support::traits::{Get, Hooks};
use frame_support::weights::Weight;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_permission::{CapabilityKey, GrantPayload};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId, RecordKey};
use sp_runtime::DispatchError;

const ACTOR: u64 = 1;
const RELAYER: u64 = 2;
const LIFETIME: u64 = 600_000;

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the bytes sit inside the bound")
}

// --- The Blueprint the suite installs -----------------------------------

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

fn parcel(survey: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text("Anitha".to_string())),
            (FieldName::new("area"), Value::Integer(1_200)),
        ],
    );
    value.canonicalise();
    value
}

fn record_key(survey: &str) -> RecordKey {
    let bytes = pallet_cerulea_state::key_bytes_of(&parcel_collection(), &parcel(survey))
        .expect("the record carries its key");
    RecordKey::try_from(bytes).expect("the key fits")
}

fn blueprint() -> Blueprint {
    let mut blueprint = Blueprint::new(BlueprintMeta::new(
        "district land registry",
        "The registry records land parcels and their holders.",
        "Kollam District Administration",
    ));
    blueprint.types.push(TypeDef::Record(RecordDef {
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
    }));
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
    blueprint.actions.push(action);
    blueprint
}

fn install() -> DeploymentId {
    let held = blueprint();
    assert_ok!(Logic::install_blueprint(
        RuntimeOrigin::signed(ACTOR),
        bounded(held.encode())
    ));
    let deployment = held.id().0;
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

// --- Endpoint helpers ----------------------------------------------------

fn envelope(
    deployment: DeploymentId,
    endpoint: &str,
    payload: Vec<u8>,
    nonce: u64,
) -> MessageEnvelope {
    MessageEnvelope {
        deployment,
        endpoint: name(endpoint),
        source: name("far"),
        destination: name("cerulea"),
        party: bounded(vec![0xab; 20]),
        nonce,
        payload: bounded(payload),
        class: WorkClass::Routine,
        expiry_millis: clock().saturating_add(LIFETIME),
    }
}

fn declaration(params: &VerifierParameters) -> EndpointDeclaration<u64> {
    EndpointDeclaration {
        network: name("far"),
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

fn register_with(deployment: DeploymentId, endpoint: &str, declared: EndpointDeclaration<u64>) {
    assert_ok!(Bridge::register_endpoint(
        RuntimeOrigin::root(),
        deployment,
        name(endpoint),
        declared
    ));
}

/// Register an Avalanche endpoint for one envelope and answer with its proof.
fn avalanche_for(
    deployment: DeploymentId,
    endpoint: &str,
    held: &MessageEnvelope,
    shape: impl FnOnce(EndpointDeclaration<u64>) -> EndpointDeclaration<u64>,
) -> Proof {
    let prepared = fixtures::prepare(NetworkKind::Avalanche, held, clock());
    register_with(deployment, endpoint, shape(declaration(&prepared.params)));
    prepared.proof
}

/// Deliver one parcel through an Avalanche endpoint named "inbound".
fn delivered(deployment: DeploymentId, survey: &str) -> MessageEnvelope {
    let held = envelope(deployment, "inbound", parcel(survey).encode(), 0);
    let proof = avalanche_for(deployment, "inbound", &held, |declared| declared);
    assert_ok!(Bridge::deliver_message(
        RuntimeOrigin::signed(RELAYER),
        held.clone(),
        proof
    ));
    held
}

fn bridge_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Bridge(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

fn audit_events(deployment: &DeploymentId) -> Vec<(String, WorkClass)> {
    Audit::query(deployment, 0, 256)
        .into_iter()
        .map(|entry| (entry.event, entry.class))
        .collect()
}

// --- Registration --------------------------------------------------------

#[test]
fn an_unsigned_registration_is_refused() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, clock());
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::none(),
                deployment,
                name("inbound"),
                declaration(&prepared.params)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_registration_with_no_lifetime_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, clock());
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::root(),
                deployment,
                name("inbound"),
                EndpointDeclaration {
                    lifetime_millis: 0,
                    ..declaration(&prepared.params)
                }
            ),
            Error::<Test>::LifetimeTooLong
        );
    });
}

#[test]
fn a_deployment_holding_the_endpoint_bound_takes_no_more_endpoints() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, clock());
        for position in 0..32u32 {
            register_with(
                deployment,
                &alloc::format!("endpoint{position}"),
                declaration(&prepared.params),
            );
        }
        assert_eq!(EndpointNames::<Test>::get(deployment).len(), 32);
        assert_noop!(
            Bridge::register_endpoint(
                RuntimeOrigin::root(),
                deployment,
                name("one_more"),
                declaration(&prepared.params)
            ),
            Error::<Test>::EndpointLimit
        );
    });
}

#[test]
fn a_registration_stores_the_endpoint_and_announces_it() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, clock());
        register_with(deployment, "inbound", declaration(&prepared.params));

        let key = EndpointKey::new(deployment, name("inbound"));
        let record = Endpoints::<Test>::get(&key).expect("stored");
        assert_eq!(record.kind, NetworkKind::Avalanche);
        assert_eq!(record.registrar, Pallet::<Test>::platform_account());
        assert_eq!(record.registered_at, 1);
        // An Avalanche endpoint trusts no header checkpoint, so no tip lands.
        assert_eq!(Tips::<Test>::get(&key), 0);
        assert_eq!(Floors::<Test>::get(&key), 0);
        assert_eq!(
            bridge_events().last(),
            Some(&Event::EndpointRegistered {
                deployment,
                endpoint: name("inbound"),
                kind: NetworkKind::Avalanche,
            })
        );
        assert!(audit_events(&deployment)
            .contains(&("endpoint.registered".to_string(), WorkClass::Structural)));
    });
}

#[test]
fn a_registration_with_a_checkpoint_sets_the_tip_and_the_floor() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "bitcoin", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, clock());
        register_with(deployment, "bitcoin", declaration(&prepared.params));
        let key = EndpointKey::new(deployment, name("bitcoin"));
        assert_eq!(Tips::<Test>::get(&key), fixtures::START_HEIGHT);
        assert_eq!(Floors::<Test>::get(&key), fixtures::START_HEIGHT);

        let mut params = match prepared.params.clone() {
            VerifierParameters::Bitcoin(inner) => inner,
            _ => panic!("the fixture carries Bitcoin parameters"),
        };
        params.checkpoint = Checkpoint::default();
        register_with(
            deployment,
            "bitcoin_from_genesis",
            declaration(&VerifierParameters::Bitcoin(params)),
        );
        let fresh = EndpointKey::new(deployment, name("bitcoin_from_genesis"));
        assert_eq!(Tips::<Test>::get(&fresh), 0);
    });
}

// --- Retirement ----------------------------------------------------------

#[test]
fn retiring_an_unknown_endpoint_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        assert_noop!(
            Bridge::retire_endpoint(RuntimeOrigin::root(), deployment, name("absent")),
            Error::<Test>::EndpointUnknown
        );
    });
}

#[test]
fn retiring_without_approval_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        avalanche_for(deployment, "inbound", &held, |declared| declared);
        assert_noop!(
            Bridge::retire_endpoint(RuntimeOrigin::signed(ACTOR), deployment, name("inbound")),
            Error::<Test>::ApprovalMissing
        );
    });
}

#[test]
fn a_retirement_marks_the_endpoint_and_announces_it() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1], 0);
        avalanche_for(deployment, "inbound", &held, |declared| declared);
        assert_ok!(Bridge::retire_endpoint(
            RuntimeOrigin::root(),
            deployment,
            name("inbound")
        ));
        let record = Pallet::<Test>::endpoint(&deployment, &name("inbound")).expect("stored");
        assert!(!record.live);
        assert!(!record.takes(Travel::Inbound));
        assert_eq!(
            bridge_events().last(),
            Some(&Event::EndpointRetired {
                deployment,
                endpoint: name("inbound"),
            })
        );
        assert!(audit_events(&deployment)
            .contains(&("endpoint.retired".to_string(), WorkClass::Structural)));
    });
}

// --- Header submission ---------------------------------------------------

fn bitcoin_endpoint(deployment: DeploymentId) -> fixtures::Prepared {
    let held = envelope(deployment, "bitcoin", parcel("12/3").encode(), 0);
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, clock());
    register_with(deployment, "bitcoin", declaration(&prepared.params));
    prepared
}

fn bitcoin_batch(prepared: &fixtures::Prepared) -> HeaderBatch {
    prepared
        .headers
        .clone()
        .expect("the fixture carries headers")
}

#[test]
fn headers_for_an_unknown_endpoint_decline() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "bitcoin", vec![1], 0);
        let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, clock());
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                bitcoin_batch(&prepared)
            ),
            Error::<Test>::EndpointUnknown
        );
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::none(),
                deployment,
                name("bitcoin"),
                bitcoin_batch(&prepared)
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn headers_for_a_retired_endpoint_decline() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let prepared = bitcoin_endpoint(deployment);
        assert_ok!(Bridge::retire_endpoint(
            RuntimeOrigin::root(),
            deployment,
            name("bitcoin")
        ));
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                bitcoin_batch(&prepared)
            ),
            Error::<Test>::EndpointRetired
        );
    });
}

#[test]
fn headers_that_fail_their_check_decline() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = bitcoin_endpoint(deployment);
        let orphan = fixtures::mine(&[0x61; 32], &[0x62; 32], 1_700_000_000);
        assert_noop!(
            Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                HeaderBatch::Bitcoin(bitcoin::BitcoinHeaderBatch {
                    headers: BoundedVec::try_from(
                        vec![BoundedVec::try_from(orphan).expect("fits")]
                    )
                    .expect("fits"),
                })
            ),
            Error::<Test>::HeadersDeclined
        );
    });
}

#[test]
fn accepted_headers_move_the_tip_and_announce_the_count() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let prepared = bitcoin_endpoint(deployment);
        assert_ok!(Bridge::submit_headers(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            name("bitcoin"),
            bitcoin_batch(&prepared)
        ));
        let tip = fixtures::START_HEIGHT + 2;
        let key = EndpointKey::new(deployment, name("bitcoin"));
        assert_eq!(Tips::<Test>::get(&key), tip);
        assert_eq!(
            bridge_events().last(),
            Some(&Event::HeadersAccepted {
                deployment,
                endpoint: name("bitcoin"),
                tip,
                count: 2,
            })
        );
        assert!(audit_events(&deployment)
            .contains(&("headers.accepted".to_string(), WorkClass::Routine)));
    });
}

#[test]
fn the_header_window_drops_the_oldest_headers() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = bitcoin_endpoint(deployment);
        let key = EndpointKey::new(deployment, name("bitcoin"));

        // Three batches of thirty two mined headers pass the window of
        // sixty four headers the mock runtime keeps.
        let mut parent = fixtures::first_base(NetworkKind::Bitcoin).hash;
        let mut moment = 1_700_000_000u32;
        for _ in 0..3 {
            let mut batch = Vec::new();
            for _ in 0..32 {
                let header = fixtures::mine(&parent, &[0x2f; 32], moment);
                parent = bitcoin::read_header(&header).expect("reads").hash;
                moment += 600;
                batch.push(BoundedVec::try_from(header).expect("fits"));
            }
            assert_ok!(Bridge::submit_headers(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                HeaderBatch::Bitcoin(bitcoin::BitcoinHeaderBatch {
                    headers: BoundedVec::try_from(batch).expect("fits"),
                })
            ));
        }

        let tip = fixtures::START_HEIGHT + 96;
        assert_eq!(Tips::<Test>::get(&key), tip);
        let floor = tip - 64 + 1;
        assert_eq!(Floors::<Test>::get(&key), floor);
        assert!(Headers::<Test>::get(&key, floor - 1).is_none());
        assert!(Headers::<Test>::get(&key, floor).is_some());
        assert!(Headers::<Test>::get(&key, tip).is_some());
        let checkpoint = Endpoints::<Test>::get(&key)
            .expect("stored")
            .params
            .checkpoint()
            .expect("a Bitcoin endpoint holds a checkpoint");
        assert_eq!(checkpoint.height, tip);
        assert_eq!(checkpoint.hash, parent);
    });
}

// --- Delivery ------------------------------------------------------------

#[test]
fn a_message_for_an_unknown_endpoint_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, clock());
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, prepared.proof),
            Error::<Test>::EndpointUnknown
        );
    });
}

#[test]
fn a_message_for_an_outbound_only_endpoint_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| {
            EndpointDeclaration {
                direction: Travel::Outbound,
                both_ways: false,
                ..declared
            }
        });
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, proof),
            Error::<Test>::DirectionDeclined
        );
    });
}

#[test]
fn a_payload_outside_the_word_alignment_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![1u8; 33], 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| {
            EndpointDeclaration {
                format: MessageFormat::WordAligned,
                ..declared
            }
        });
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, proof),
            Error::<Test>::PayloadTooLarge
        );
    });
}

#[test]
fn a_message_whose_proof_fails_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| declared);
        let mut changed = held;
        changed.class = WorkClass::Structural;
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), changed, proof),
            Error::<Test>::ProofDeclined
        );
    });
}

#[test]
fn an_unsigned_delivery_is_refused() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| declared);
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::none(), held, proof),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_delivered_message_runs_under_the_endpoint_class_and_announces_itself() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = delivered(deployment, "12/3");
        let id = held.id();
        let record =
            Pallet::<Test>::inbound_message(&deployment, &name("inbound"), &id).expect("stored");
        assert_eq!(record.status, MessageStatus::Executed);
        assert_eq!(record.height, fixtures::START_HEIGHT + 1);
        assert_eq!(
            record.trust,
            TrustAssumption::ValidatorThreshold {
                signed: 75,
                total: 100
            }
        );
        assert_eq!(
            bridge_events().last(),
            Some(&Event::MessageExecuted {
                deployment,
                endpoint: name("inbound"),
                id,
                height: fixtures::START_HEIGHT + 1,
                effects: 1,
            })
        );
        let log = audit_events(&deployment);
        assert!(log.contains(&("message.executed".to_string(), WorkClass::Financial)));
        assert!(log.contains(&("message.relayed".to_string(), WorkClass::Routine)));
        let actor_entries = Audit::query(&deployment, 0, 256)
            .into_iter()
            .filter(|entry| entry.event == "message.executed")
            .map(|entry| entry.actor)
            .collect::<Vec<_>>();
        assert_eq!(
            actor_entries,
            vec![Pallet::<Test>::source_account(
                &held.source,
                held.party.as_slice()
            )]
        );
    });
}

#[test]
fn a_message_for_a_deployment_without_a_blueprint_names_the_install() {
    new_test_ext().execute_with(|| {
        let deployment = [0x5e; 32];
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| declared);
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, proof),
            DispatchError::Other("Install the Blueprint before a bridge message runs an action.")
        );
    });
}

#[test]
fn a_message_naming_an_undeclared_action_names_the_declaration() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", parcel("12/3").encode(), 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| {
            EndpointDeclaration {
                action: name("absent_action"),
                ..declared
            }
        });
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, proof),
            DispatchError::Other("Declare the action the bridge endpoint names in the Blueprint.")
        );
    });
}

#[test]
fn a_payload_that_fails_to_decode_names_the_canonical_encoding() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![0xff; 6], 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| declared);
        assert_noop!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held, proof),
            DispatchError::Other("Send the canonical encoding of a Cerulea value as the payload.")
        );
    });
}

#[test]
fn a_raw_payload_reaches_the_action_as_bytes() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = envelope(deployment, "inbound", vec![0x42; 32], 0);
        let proof = avalanche_for(deployment, "inbound", &held, |declared| {
            EndpointDeclaration {
                format: MessageFormat::RawBytes,
                ..declared
            }
        });
        // The action takes a parcel record, so plain bytes fail its type
        // check and nothing lands.
        assert!(
            Bridge::deliver_message(RuntimeOrigin::signed(RELAYER), held.clone(), proof).is_err()
        );
        assert!(
            Inbound::<Test>::get(EndpointKey::new(deployment, name("inbound")), held.id())
                .is_none()
        );
        assert!(!Pallet::<Test>::has_seen(
            &deployment,
            &name("inbound"),
            &held.id()
        ));
    });
}

// --- Outbound messages ---------------------------------------------------

#[test]
fn an_outbound_message_needs_a_live_outbound_endpoint() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("absent"),
                record_key("12/3"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::EndpointUnknown
        );

        let held = envelope(deployment, "inbound_only", vec![1], 0);
        avalanche_for(deployment, "inbound_only", &held, |declared| {
            EndpointDeclaration {
                both_ways: false,
                ..declared
            }
        });
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound_only"),
                record_key("12/3"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::DirectionDeclined
        );

        assert_ok!(Bridge::retire_endpoint(
            RuntimeOrigin::root(),
            deployment,
            name("inbound")
        ));
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                record_key("12/3"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::EndpointRetired
        );
    });
}

#[test]
fn an_outbound_message_over_an_unknown_collection_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let held = envelope(deployment, "ledger", vec![1], 0);
        avalanche_for(deployment, "ledger", &held, |declared| {
            EndpointDeclaration {
                collection: name("ledger"),
                ..declared
            }
        });
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("ledger"),
                record_key("12/3"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

fn dispatched(deployment: DeploymentId) -> [u8; 32] {
    assert_ok!(Bridge::dispatch_outbound(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        name("inbound"),
        record_key("12/3"),
        bounded(vec![0xab; 20]),
    ));
    let key = EndpointKey::new(deployment, name("inbound"));
    let (id, _) = Outbound::<Test>::iter_prefix(&key)
        .max_by_key(|(_, held)| held.envelope.nonce)
        .expect("the outbound message exists");
    id
}

#[test]
fn an_outbound_message_is_stored_scheduled_and_announced() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let id = dispatched(deployment);
        let held =
            Pallet::<Test>::outbound_message(&deployment, &name("inbound"), &id).expect("stored");
        assert_eq!(held.status, MessageStatus::Pending);
        assert_eq!(held.envelope.nonce, 0);
        assert_eq!(held.envelope.class, WorkClass::Financial);
        let expiry = clock() + LIFETIME;
        assert_eq!(held.envelope.expiry_millis, expiry);
        assert_eq!(
            SweepCursor::<Test>::get(),
            Pallet::<Test>::bucket_of(expiry)
        );
        assert_eq!(
            bridge_events().last(),
            Some(&Event::MessageReady {
                deployment,
                endpoint: name("inbound"),
                id,
                commitment: held.commitment,
                expiry_millis: expiry,
            })
        );
        assert!(audit_events(&deployment)
            .contains(&("message.ready".to_string(), WorkClass::Financial)));

        // The next message takes the next nonce.
        let second = dispatched(deployment);
        assert_ne!(second, id);
    });
}

#[test]
fn a_full_expiry_bucket_takes_no_more_messages() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        for _ in 0..64 {
            let _ = dispatched(deployment);
        }
        let bucket = Pallet::<Test>::bucket_of(clock() + LIFETIME);
        assert_eq!(ExpirySchedule::<Test>::get(bucket).len(), 64);
        assert_noop!(
            Bridge::dispatch_outbound(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                record_key("12/3"),
                bounded(vec![0xab; 20]),
            ),
            Error::<Test>::BucketFull
        );
    });
}

#[test]
fn an_earlier_expiry_pulls_the_sweep_cursor_back() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let later = Pallet::<Test>::bucket_of(clock() + LIFETIME) + 5;
        SweepCursor::<Test>::put(later);
        let _ = dispatched(deployment);
        assert_eq!(
            SweepCursor::<Test>::get(),
            Pallet::<Test>::bucket_of(clock() + LIFETIME)
        );
    });
}

#[test]
fn a_delivery_report_needs_a_known_open_message() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        assert_noop!(
            Bridge::confirm_outbound(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("absent"),
                [1u8; 32],
                [2u8; 32]
            ),
            Error::<Test>::EndpointUnknown
        );
        assert_noop!(
            Bridge::confirm_outbound(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("inbound"),
                [1u8; 32],
                [2u8; 32]
            ),
            Error::<Test>::MessageUnknown
        );
        let id = dispatched(deployment);
        assert_ok!(Bridge::confirm_outbound(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            name("inbound"),
            id,
            [3u8; 32]
        ));
        assert_eq!(
            bridge_events().last(),
            Some(&Event::MessageDelivered {
                deployment,
                endpoint: name("inbound"),
                id,
                reference: [3u8; 32],
            })
        );
        assert_noop!(
            Bridge::confirm_outbound(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("inbound"),
                id,
                [4u8; 32]
            ),
            Error::<Test>::MessageSettled
        );
    });
}

// --- Cancellation and the sweep ------------------------------------------

#[test]
fn a_cancellation_needs_a_known_open_message() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = delivered(deployment, "12/3");
        assert_noop!(
            Bridge::cancel_message(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("absent"),
                held.id(),
                Travel::Inbound
            ),
            Error::<Test>::EndpointUnknown
        );
        for travel in [Travel::Inbound, Travel::Outbound] {
            assert_noop!(
                Bridge::cancel_message(
                    RuntimeOrigin::signed(ACTOR),
                    deployment,
                    name("inbound"),
                    [9u8; 32],
                    travel
                ),
                Error::<Test>::MessageUnknown
            );
        }
        // The inbound message already ran, so it has nothing left to stop.
        assert_noop!(
            Bridge::cancel_message(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                held.id(),
                Travel::Inbound
            ),
            Error::<Test>::MessageSettled
        );
        let id = dispatched(deployment);
        assert_ok!(Bridge::confirm_outbound(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            name("inbound"),
            id,
            [3u8; 32]
        ));
        advance_clock(LIFETIME * 2);
        assert_noop!(
            Bridge::cancel_message(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                id,
                Travel::Outbound
            ),
            Error::<Test>::MessageSettled
        );
    });
}

#[test]
fn a_cancelled_message_is_announced_with_its_travel() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let id = dispatched(deployment);
        advance_clock(LIFETIME * 2);
        assert_ok!(Bridge::cancel_message(
            RuntimeOrigin::signed(ACTOR),
            deployment,
            name("inbound"),
            id,
            Travel::Outbound
        ));
        assert_eq!(
            bridge_events().last(),
            Some(&Event::MessageCancelled {
                deployment,
                endpoint: name("inbound"),
                id,
                travel: Travel::Outbound,
            })
        );
        // The sweep later finds the message settled and moves nothing.
        roll_one_block();
        let held =
            Pallet::<Test>::outbound_message(&deployment, &name("inbound"), &id).expect("stored");
        assert_eq!(held.status, MessageStatus::Cancelled);
        let cancellations = bridge_events()
            .iter()
            .filter(|event| matches!(event, Event::MessageCancelled { .. }))
            .count();
        assert_eq!(cancellations, 1);
    });
}

#[test]
fn the_sweep_passes_over_an_inbound_entry_that_already_ran() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let held = delivered(deployment, "12/3");
        let key = EndpointKey::new(deployment, name("inbound"));
        let bucket = Pallet::<Test>::bucket_of(held.expiry_millis);
        ExpirySchedule::<Test>::insert(
            bucket,
            BoundedVec::<_, frame_support::traits::ConstU32<64>>::try_from(vec![
                (key.clone(), held.id(), Travel::Inbound),
                (key.clone(), [8u8; 32], Travel::Outbound),
            ])
            .expect("fits"),
        );
        SweepCursor::<Test>::put(bucket);
        advance_clock(LIFETIME * 2);
        let weight = Bridge::on_initialize(2);
        assert!(ExpirySchedule::<Test>::get(bucket).is_empty());
        assert!(SweepCursor::<Test>::get() > bucket);
        assert_eq!(
            Inbound::<Test>::get(&key, held.id()).map(|record| record.status),
            Some(MessageStatus::Executed)
        );
        // Each visited entry costs one sweep step on top of the bookkeeping,
        // whether the entry moved or not.
        let step = <() as WeightInfo>::sweep_one();
        let db: frame_support::weights::RuntimeDbWeight =
            <<Test as frame_system::Config>::DbWeight as Get<_>>::get();
        assert_eq!(
            weight,
            step.saturating_add(step)
                .saturating_add(db.reads_writes(2, 2))
        );
    });
}

#[test]
fn the_sweep_rests_until_the_cursor_falls_due() {
    new_test_ext().execute_with(|| {
        assert_eq!(Bridge::on_initialize(1), Weight::zero());
        let future = Pallet::<Test>::bucket_of(clock()) + 3;
        SweepCursor::<Test>::put(future);
        assert_eq!(Bridge::on_initialize(1), Weight::zero());
        assert_eq!(SweepCursor::<Test>::get(), future);
    });
}

// --- Anchoring -----------------------------------------------------------

#[test]
fn an_anchor_request_needs_a_live_bitcoin_endpoint() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let checkpoint = cerulea_primitives::checkpoint::Checkpoint::new(0, 900, [1; 32], [2; 32]);
        assert_noop!(
            Bridge::request_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("absent"),
                name("parcel")
            ),
            Error::<Test>::EndpointUnknown
        );
        assert_noop!(
            Bridge::request_checkpoint_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("absent"),
                checkpoint
            ),
            Error::<Test>::EndpointUnknown
        );
        assert_noop!(
            Bridge::request_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                name("parcel")
            ),
            Error::<Test>::KindMismatch
        );
        assert_noop!(
            Bridge::request_checkpoint_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("inbound"),
                checkpoint
            ),
            Error::<Test>::KindMismatch
        );

        let _ = bitcoin_endpoint(deployment);
        // The Blueprint never declared a ledger collection, so the store
        // holds no root for it and the request records nothing.
        assert_noop!(
            Bridge::request_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("bitcoin"),
                name("ledger")
            ),
            Error::<Test>::RootAbsent
        );
        assert!(Anchors::<Test>::get(EndpointKey::new(deployment, name("bitcoin")), 0).is_none());
        assert_ok!(Bridge::retire_endpoint(
            RuntimeOrigin::root(),
            deployment,
            name("bitcoin")
        ));
        assert_noop!(
            Bridge::request_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("bitcoin"),
                name("parcel")
            ),
            Error::<Test>::EndpointRetired
        );
        assert_noop!(
            Bridge::request_checkpoint_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("bitcoin"),
                checkpoint
            ),
            Error::<Test>::EndpointRetired
        );
    });
}

/// Request one checkpoint anchor and mine the far chain that holds it.
///
/// The answer carries the evidence of the anchoring transaction and the
/// commitment the anchor holds.
fn anchored(deployment: DeploymentId, value: [u8; 32]) -> (AnchorEvidence, [u8; 32]) {
    let prepared = bitcoin_endpoint(deployment);
    assert_ok!(Bridge::submit_headers(
        RuntimeOrigin::signed(RELAYER),
        deployment,
        name("bitcoin"),
        bitcoin_batch(&prepared)
    ));
    let checkpoint = cerulea_primitives::checkpoint::Checkpoint::new(1, 1_800, [5; 32], [6; 32]);
    assert_ok!(Bridge::request_checkpoint_anchor(
        RuntimeOrigin::signed(ACTOR),
        deployment,
        name("bitcoin"),
        checkpoint
    ));
    assert_eq!(
        bridge_events().last(),
        Some(&Event::AnchorRequested {
            deployment,
            endpoint: name("bitcoin"),
            anchor: 0,
            root: checkpoint.commitment(),
        })
    );

    let transaction = bitcoin::encode_anchor_transaction(
        &[0x6b; 32],
        0,
        fixtures::BITCOIN_MARKER,
        &value,
        10_000,
        &[0x76, 0xa9, 0x14],
    );
    let read = bitcoin::read_transaction(&transaction).expect("reads");
    let key = EndpointKey::new(deployment, name("bitcoin"));
    let parent = Headers::<Test>::get(&key, Tips::<Test>::get(&key))
        .expect("the tip header exists")
        .hash;
    let first = fixtures::mine(&parent, &read.txid, 1_700_001_200);
    let first_read = bitcoin::read_header(&first).expect("reads");
    let second = fixtures::mine(&first_read.hash, &[0x3f; 32], 1_700_001_800);
    assert_ok!(Bridge::submit_headers(
        RuntimeOrigin::signed(RELAYER),
        deployment,
        name("bitcoin"),
        HeaderBatch::Bitcoin(bitcoin::BitcoinHeaderBatch {
            headers: BoundedVec::try_from(vec![
                BoundedVec::try_from(first).expect("fits"),
                BoundedVec::try_from(second).expect("fits"),
            ])
            .expect("fits"),
        })
    ));
    let evidence = AnchorEvidence {
        height: Tips::<Test>::get(&key) - 1,
        transaction: BoundedVec::try_from(transaction).expect("fits"),
        position: 0,
        branch: BoundedVec::default(),
        output_index: 0,
    };
    (evidence, checkpoint.commitment())
}

#[test]
fn an_anchor_confirmation_needs_a_known_bitcoin_anchor() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = delivered(deployment, "12/3");
        let checkpoint =
            cerulea_primitives::checkpoint::Checkpoint::new(1, 1_800, [5; 32], [6; 32]);
        let (evidence, _) = anchored(deployment, checkpoint.commitment());
        assert_noop!(
            Bridge::confirm_anchor(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("absent"),
                0,
                evidence.clone()
            ),
            Error::<Test>::EndpointUnknown
        );
        assert_noop!(
            Bridge::confirm_anchor(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("inbound"),
                0,
                evidence.clone()
            ),
            Error::<Test>::KindMismatch
        );
        assert_noop!(
            Bridge::confirm_anchor(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                4,
                evidence
            ),
            Error::<Test>::AnchorUnknown
        );
    });
}

#[test]
fn anchor_evidence_that_fails_its_check_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let checkpoint =
            cerulea_primitives::checkpoint::Checkpoint::new(1, 1_800, [5; 32], [6; 32]);
        let (evidence, _) = anchored(deployment, checkpoint.commitment());

        let unrecorded = AnchorEvidence {
            height: 9_999,
            ..evidence.clone()
        };
        let shallow = AnchorEvidence {
            height: evidence.height + 1,
            ..evidence.clone()
        };
        let off_branch = AnchorEvidence {
            position: 1,
            branch: BoundedVec::try_from(vec![[0x12; 32]]).expect("fits"),
            ..evidence.clone()
        };
        let no_output = AnchorEvidence {
            output_index: 5,
            ..evidence.clone()
        };
        for bad in [unrecorded, shallow, off_branch, no_output] {
            assert_noop!(
                Bridge::confirm_anchor(
                    RuntimeOrigin::signed(RELAYER),
                    deployment,
                    name("bitcoin"),
                    0,
                    bad
                ),
                Error::<Test>::ProofDeclined
            );
        }
    });
}

#[test]
fn an_anchor_holding_another_value_declines() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let (evidence, _) = anchored(deployment, [0x77; 32]);
        assert_noop!(
            Bridge::confirm_anchor(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                0,
                evidence
            ),
            Error::<Test>::AnchorMismatch
        );
    });
}

#[test]
fn a_confirmed_anchor_is_stored_announced_and_settled_once() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let checkpoint =
            cerulea_primitives::checkpoint::Checkpoint::new(1, 1_800, [5; 32], [6; 32]);
        let (evidence, commitment) = anchored(deployment, checkpoint.commitment());
        assert_ok!(Bridge::confirm_anchor(
            RuntimeOrigin::signed(RELAYER),
            deployment,
            name("bitcoin"),
            0,
            evidence.clone()
        ));
        let key = EndpointKey::new(deployment, name("bitcoin"));
        let held = Anchors::<Test>::get(&key, 0).expect("stored");
        assert_eq!(held.status, AnchorStatus::Confirmed);
        assert_eq!(held.root, commitment);
        assert_eq!(held.confirmations, 2);
        assert_eq!(
            held.reference,
            bitcoin::read_transaction(&evidence.transaction)
                .expect("reads")
                .txid
        );
        assert_eq!(
            bridge_events().last(),
            Some(&Event::AnchorConfirmed {
                deployment,
                endpoint: name("bitcoin"),
                anchor: 0,
                height: evidence.height,
            })
        );
        assert!(audit_events(&deployment)
            .contains(&("anchor.confirmed".to_string(), WorkClass::Structural)));
        assert_noop!(
            Bridge::confirm_anchor(
                RuntimeOrigin::signed(RELAYER),
                deployment,
                name("bitcoin"),
                0,
                evidence
            ),
            Error::<Test>::MessageSettled
        );
        assert_eq!(AnchorStatus::Confirmed.label(), "confirmed");
        assert_eq!(AnchorStatus::Requested.label(), "requested");
    });
}

#[test]
fn the_anchor_listing_answers_in_position_order_inside_its_limit() {
    new_test_ext().execute_with(|| {
        let deployment = install();
        let _ = bitcoin_endpoint(deployment);
        for index in 0..4u32 {
            let checkpoint = cerulea_primitives::checkpoint::Checkpoint::new(
                index,
                900 * (index + 1),
                [index as u8; 32],
                [9; 32],
            );
            assert_ok!(Bridge::request_checkpoint_anchor(
                RuntimeOrigin::signed(ACTOR),
                deployment,
                name("bitcoin"),
                checkpoint
            ));
        }
        let key = EndpointKey::new(deployment, name("bitcoin"));
        assert_eq!(AnchorNonce::<Test>::get(&key), 4);
        let listed = crate::anchor::listed::<Test>(&deployment, &name("bitcoin"), 3);
        assert_eq!(
            listed
                .iter()
                .map(|(position, _)| *position)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert!(listed
            .iter()
            .all(|(_, held)| held.status == AnchorStatus::Requested));
        assert!(crate::anchor::listed::<Test>(&deployment, &name("absent"), 3).is_empty());
    });
}

// --- Shapes and sentences ------------------------------------------------

#[test]
fn every_status_and_label_reads_as_plain_text() {
    assert_eq!(MessageStatus::Pending.label(), "pending");
    assert_eq!(MessageStatus::Delivered.label(), "delivered");
    assert_eq!(MessageStatus::Executed.label(), "executed");
    assert_eq!(MessageStatus::Cancelled.label(), "cancelled");
    assert!(MessageStatus::Pending.is_open());
    assert!(!MessageStatus::Executed.is_open());
    assert_eq!(Travel::Inbound.label(), "inbound");
    assert_eq!(Travel::Outbound.label(), "outbound");
    assert_eq!(MessageFormat::CanonicalValue.label(), "canonical value");
    assert_eq!(MessageFormat::RawBytes.label(), "raw bytes");
    assert_eq!(MessageFormat::WordAligned.label(), "word aligned");
    assert!(MessageFormat::WordAligned.accepts(64));
    assert!(!MessageFormat::WordAligned.accepts(65));
    assert!(MessageFormat::RawBytes.accepts(65));
    let labels: Vec<&str> = NetworkKind::every()
        .iter()
        .map(|kind| kind.label())
        .collect();
    assert_eq!(
        labels,
        vec![
            "ethereum virtual machine",
            "cosmos",
            "solana",
            "polkadot",
            "avalanche",
            "bitcoin",
            "rollup"
        ]
    );
}

#[test]
fn every_fault_carries_a_distinct_label_and_its_fix() {
    let faults = [
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
    ];
    let mut labels: Vec<&str> = faults.iter().map(|fault| fault.label()).collect();
    labels.sort();
    labels.dedup();
    assert_eq!(labels.len(), faults.len());
    assert_eq!(BridgeFault::CheckpointAbsent.label(), "checkpoint absent");
    assert_eq!(
        crate::fault_sentence(BridgeFault::CheckpointAbsent),
        "Set a trusted checkpoint on the endpoint before sending a proof."
    );
    for fault in faults {
        assert_eq!(crate::fault_sentence(fault), fault.fix());
    }
}

#[test]
fn every_trust_assumption_states_what_it_rests_on() {
    assert_eq!(
        TrustAssumption::NetworkCryptography.sentence(),
        "The chain walked the far network merkle structure to the committed root."
    );
    assert_eq!(
        TrustAssumption::ValidatorThreshold {
            signed: 1,
            total: 2
        }
        .sentence(),
        "A share of the far validator set signed the block the message sits in."
    );
    assert_eq!(
        TrustAssumption::RelayerAttestation {
            signed: 1,
            registered: 2
        }
        .sentence(),
        "A threshold of registered relayers signed the far network header."
    );
    assert_eq!(
        TrustAssumption::AccumulatedWork { confirmations: 6 }.sentence(),
        "The far header chain carries the work the endpoint requires."
    );
    assert_eq!(
        TrustAssumption::SettledAfterChallenge { window_millis: 1 }.sentence(),
        "The settlement layer commitment passed its challenge window."
    );
}

#[test]
fn a_checkpoint_moves_forward_only_and_never_on_avalanche() {
    new_test_ext().execute_with(|| {
        let held = envelope([1u8; 32], "far", vec![1], 0);
        for kind in NetworkKind::every() {
            let mut params = fixtures::prepare(kind, &held, clock()).params;
            let current = params.checkpoint();
            let higher = Checkpoint {
                height: fixtures::START_HEIGHT + 10,
                hash: [0x0f; 32],
            };
            let lower = Checkpoint {
                height: fixtures::START_HEIGHT - 1,
                hash: [0x0e; 32],
            };
            if kind == NetworkKind::Avalanche {
                assert_eq!(current, None);
                assert!(!params.advance(higher));
                continue;
            }
            assert!(!params.advance(lower), "{}", kind.label());
            assert_eq!(params.checkpoint(), current);
            assert!(params.advance(higher), "{}", kind.label());
            assert_eq!(params.checkpoint(), Some(higher));
        }
    });
}

#[test]
fn every_header_batch_names_its_network() {
    new_test_ext().execute_with(|| {
        let held = envelope([1u8; 32], "far", vec![1], 0);
        for kind in [
            NetworkKind::EthereumVirtualMachine,
            NetworkKind::Solana,
            NetworkKind::Bitcoin,
            NetworkKind::Rollup,
        ] {
            let batch = fixtures::prepare(kind, &held, clock())
                .headers
                .expect("the fixture carries headers");
            assert_eq!(batch.kind(), kind);
        }
        let solana = fixtures::prepare(NetworkKind::Solana, &held, clock());
        let bitcoin_side = fixtures::prepare(NetworkKind::Bitcoin, &held, clock());
        assert_eq!(
            Pallet::<Test>::read_batch(
                &bitcoin_side.params,
                &solana.headers.expect("present"),
                clock()
            ),
            Err(BridgeFault::KindMismatch)
        );
    });
}

#[test]
fn a_threshold_over_an_empty_set_is_never_reached() {
    use crate::endpoint::{reaches_percent, reaches_two_thirds};
    assert!(!reaches_percent(0, 0, 50));
    assert!(!reaches_two_thirds(5, 0));
    assert!(reaches_percent(1, 1, 100));
}

#[test]
fn every_bridge_call_fits_inside_one_block() {
    // A block spends two seconds of computation. Every call the bridge
    // module prices must fit inside that span on its own.
    let block = Weight::from_parts(
        2 * frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND,
        5 * 1024 * 1024,
    );
    let priced = [
        CeruleaWeight::<Test>::register_endpoint(),
        CeruleaWeight::<Test>::retire_endpoint(),
        CeruleaWeight::<Test>::submit_headers(),
        CeruleaWeight::<Test>::deliver_message(),
        CeruleaWeight::<Test>::dispatch_outbound(),
        CeruleaWeight::<Test>::confirm_outbound(),
        CeruleaWeight::<Test>::request_anchor(),
        CeruleaWeight::<Test>::confirm_anchor(),
        CeruleaWeight::<Test>::cancel_message(),
        CeruleaWeight::<Test>::sweep_one(),
    ];
    for weight in priced.iter() {
        assert!(weight.all_lte(block));
        assert!(weight.ref_time() > 0);
    }
    // A full header batch writes the most, so header submission is the
    // heaviest call. Proof verification puts delivery next.
    assert!(priced
        .iter()
        .all(|weight| weight.ref_time() <= priced[2].ref_time()));
    assert!(priced
        .iter()
        .enumerate()
        .filter(|(position, _)| *position != 2)
        .all(|(_, weight)| weight.ref_time() <= priced[3].ref_time()));
    // One swept message costs less than a cancellation a person requests.
    assert!(priced[9].ref_time() < priced[8].ref_time());
    // The Cerulea figures add the storage access on top of the computation
    // the test figures charge.
    assert!(<() as WeightInfo>::deliver_message().all_lte(priced[3]));
    assert!(<() as WeightInfo>::sweep_one().all_lte(priced[9]));
}
