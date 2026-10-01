//! Reject paths of the seven network verifiers.
//!
//! Every test starts from the real far network fixture of one network. The
//! fixture carries real trie nodes, real signatures and real headers. Each
//! test then changes one part of the fixture, runs the verifier and asserts
//! the exact fault the verifier names. The fault carries the plain fix
//! sentence a relayer reads.

use crate::endpoint::{
    Checkpoint, EndpointState, HeaderRecord, HeaderSource, ListedHeaders, VerifierParameters,
};
use crate::envelope::{BridgeFault, MessageEnvelope, MessageFormat, NetworkKind, TrustAssumption};
use crate::fixtures::{self, Prepared};
use crate::mock::Test;
use crate::networks::{avalanche, bitcoin, cosmos, evm, polkadot, rollup, solana};
use crate::verify::{self, KeyedSignature, Proof, Verifier};
use crate::{mpt, rlp, HeaderBatch};
use codec::Encode;
use frame_support::BoundedVec;
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::name_of;
use sp_core::Pair;

/// The moment every verifier call runs at, in milliseconds.
const NOW: u64 = 1_700_000_000_000;

fn envelope() -> MessageEnvelope {
    MessageEnvelope {
        deployment: [3u8; 32],
        endpoint: name_of("verifier").expect("the name fits"),
        source: name_of("far").expect("the name fits"),
        destination: name_of("cerulea").expect("the name fits"),
        party: BoundedVec::try_from(vec![0xab; 20]).expect("the address fits"),
        nonce: 7,
        payload: BoundedVec::try_from(vec![1, 2, 3, 4]).expect("the payload fits"),
        class: WorkClass::Routine,
        expiry_millis: NOW + 60_000,
    }
}

/// The same message identifier under a different commitment.
///
/// The identifier leaves out the class and the expiry. The commitment
/// covers both, so the far network holds a word that differs.
fn recommitted(held: &MessageEnvelope) -> MessageEnvelope {
    let mut changed = held.clone();
    changed.class = WorkClass::Constitutional;
    assert_eq!(changed.id(), held.id());
    assert_ne!(changed.commitment(), held.commitment());
    changed
}

/// A different message identifier.
fn renamed(held: &MessageEnvelope) -> MessageEnvelope {
    let mut changed = held.clone();
    changed.nonce = held.nonce + 1;
    changed
}

/// The headers the endpoint accepted from the prepared batch.
fn accepted(prepared: &Prepared) -> ListedHeaders {
    match prepared.headers.as_ref() {
        Some(batch) => ListedHeaders::many(
            crate::Pallet::<Test>::read_batch(&prepared.params, batch, NOW)
                .expect("the fixture batch reads"),
        ),
        None => ListedHeaders::default(),
    }
}

fn run(
    held: &MessageEnvelope,
    params: &VerifierParameters,
    proof: &Proof,
    headers: &ListedHeaders,
) -> Result<crate::envelope::VerifiedMessage, BridgeFault> {
    let state = EndpointState::new(params, MessageFormat::CanonicalValue, headers, NOW);
    verify::verify(held, proof, &state)
}

fn fault_of(
    held: &MessageEnvelope,
    params: &VerifierParameters,
    proof: &Proof,
    headers: &ListedHeaders,
) -> BridgeFault {
    run(held, params, proof, headers).expect_err("the verifier declines")
}

/// Keep the root node of a proof and drop every node below it.
fn root_node_only(nodes: &verify::ProofNodes, root: &[u8; 32]) -> verify::ProofNodes {
    let kept: Vec<verify::ProofNode> = nodes
        .iter()
        .filter(|node| &sp_crypto_hashing::keccak_256(node) == root)
        .cloned()
        .collect();
    assert_eq!(kept.len(), 1, "the proof carries its root node");
    BoundedVec::try_from(kept).expect("fits")
}

fn nodes_with_garbage(nodes: &verify::ProofNodes) -> verify::ProofNodes {
    let mut held = nodes.to_vec();
    held.push(BoundedVec::try_from(vec![0xf8, 0x01]).expect("fits"));
    BoundedVec::try_from(held).expect("fits")
}

// --- Every verifier refuses parameters of another kind -------------------

#[test]
fn every_verifier_refuses_parameters_of_another_network() {
    let held = envelope();
    let foreign = fixtures::prepare(NetworkKind::Avalanche, &held, NOW).params;
    let other = fixtures::prepare(NetworkKind::Cosmos, &held, NOW).params;
    let headers = ListedHeaders::default();
    for kind in NetworkKind::every() {
        let prepared = fixtures::prepare(kind, &held, NOW);
        let params = if kind == NetworkKind::Avalanche {
            &other
        } else {
            &foreign
        };
        let state = EndpointState::new(params, MessageFormat::CanonicalValue, &headers, NOW);
        let found = match &prepared.proof {
            Proof::EthereumVirtualMachine(inner) => {
                evm::EvmVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Cosmos(inner) => {
                cosmos::CosmosVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Solana(inner) => {
                solana::SolanaVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Polkadot(inner) => {
                polkadot::PolkadotVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Avalanche(inner) => {
                avalanche::AvalancheVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Bitcoin(inner) => {
                bitcoin::BitcoinVerifier::verify(&held, inner, &state).map(|_| ())
            }
            Proof::Rollup(inner) => {
                rollup::RollupVerifier::verify(&held, inner, &state).map(|_| ())
            }
        };
        assert_eq!(found, Err(BridgeFault::KindMismatch), "{}", kind.label());
        assert_eq!(
            fault_of(&held, params, &prepared.proof, &headers),
            BridgeFault::KindMismatch
        );
    }
    assert_eq!(
        evm::EvmVerifier::kind(),
        NetworkKind::EthereumVirtualMachine
    );
    assert_eq!(cosmos::CosmosVerifier::kind(), NetworkKind::Cosmos);
    assert_eq!(solana::SolanaVerifier::kind(), NetworkKind::Solana);
    assert_eq!(polkadot::PolkadotVerifier::kind(), NetworkKind::Polkadot);
    assert_eq!(avalanche::AvalancheVerifier::kind(), NetworkKind::Avalanche);
    assert_eq!(bitcoin::BitcoinVerifier::kind(), NetworkKind::Bitcoin);
    assert_eq!(rollup::RollupVerifier::kind(), NetworkKind::Rollup);
    assert_eq!(
        BridgeFault::KindMismatch.fix(),
        "Send the proof shape the endpoint network kind names."
    );
}

// --- The Ethereum Virtual Machine verifier -------------------------------

fn evm_parts(prepared: &Prepared) -> crate::endpoint::EvmParameters {
    match &prepared.params {
        VerifierParameters::EthereumVirtualMachine(held) => held.clone(),
        _ => panic!("the fixture carries virtual machine parameters"),
    }
}

fn evm_batch(prepared: &Prepared) -> evm::EvmHeaderBatch {
    match prepared.headers.clone() {
        Some(HeaderBatch::EthereumVirtualMachine(held)) => held,
        _ => panic!("the fixture carries a virtual machine batch"),
    }
}

#[test]
fn a_storage_proof_names_the_trust_it_rests_on() {
    let held = envelope();
    let prepared = fixtures::evm_side(
        &held,
        false,
        fixtures::first_base(NetworkKind::EthereumVirtualMachine),
    );
    let headers = accepted(&prepared);
    let found = run(&held, &prepared.params, &prepared.proof, &headers).expect("accepts");
    assert_eq!(found.height, fixtures::START_HEIGHT + 1);
    assert_eq!(found.id, held.id());
    assert_eq!(
        found.trust,
        TrustAssumption::RelayerAttestation {
            signed: 2,
            registered: 3
        }
    );
}

#[test]
fn a_storage_proof_at_a_height_the_endpoint_lacks_names_the_missing_root() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let fault = fault_of(
        &held,
        &prepared.params,
        &prepared.proof,
        &ListedHeaders::default(),
    );
    assert_eq!(fault, BridgeFault::RootUnknown);
    assert_eq!(
        fault.fix(),
        "Submit the far network header before sending the proof."
    );
}

#[test]
fn a_storage_proof_short_of_its_confirmations_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let headers = accepted(&prepared);
    let mut params = evm_parts(&prepared);
    params.confirmations = 3;
    let fault = fault_of(
        &held,
        &VerifierParameters::EthereumVirtualMachine(params),
        &prepared.proof,
        &headers,
    );
    assert_eq!(fault, BridgeFault::TooFewConfirmations);
    assert_eq!(
        fault.fix(),
        "Wait for the confirmation depth the endpoint requires and send the proof again."
    );
}

#[test]
fn a_storage_word_that_differs_from_the_commitment_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let headers = accepted(&prepared);
    let fault = fault_of(
        &recommitted(&held),
        &prepared.params,
        &prepared.proof,
        &headers,
    );
    assert_eq!(fault, BridgeFault::CommitmentMismatch);
    assert_eq!(
        fault.fix(),
        "Send the envelope the far network committed to."
    );
}

#[test]
fn a_storage_slot_the_far_network_never_wrote_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let headers = accepted(&prepared);
    let fault = fault_of(&renamed(&held), &prepared.params, &prepared.proof, &headers);
    assert_eq!(fault, BridgeFault::ValueAbsent);
    assert_eq!(fault.fix(), "Send the merkle path that reaches the value.");
}

#[test]
fn a_storage_proof_for_another_contract_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let headers = accepted(&prepared);
    let mut params = evm_parts(&prepared);
    params.contract = [0x99; 20];
    assert_eq!(
        fault_of(
            &held,
            &VerifierParameters::EthereumVirtualMachine(params),
            &prepared.proof,
            &headers
        ),
        BridgeFault::ValueAbsent
    );
}

#[test]
fn a_storage_proof_missing_its_root_node_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let headers = accepted(&prepared);
    let (height, account, storage) = match prepared.proof.clone() {
        Proof::EthereumVirtualMachine(evm::EvmProof::Storage {
            height,
            account,
            storage,
        }) => (height, account, storage),
        _ => panic!("the fixture carries a storage proof"),
    };
    let without_nodes = Proof::EthereumVirtualMachine(evm::EvmProof::Storage {
        height,
        account: BoundedVec::default(),
        storage: storage.clone(),
    });
    let fault = fault_of(&held, &prepared.params, &without_nodes, &headers);
    assert_eq!(fault, BridgeFault::NodeHashMismatch);

    let state_root = headers
        .header(height)
        .expect("the header was accepted")
        .state_root;
    let root_only = Proof::EthereumVirtualMachine(evm::EvmProof::Storage {
        height,
        account: root_node_only(&account, &state_root),
        storage: storage.clone(),
    });
    let fault = fault_of(&held, &prepared.params, &root_only, &headers);
    assert_eq!(fault, BridgeFault::NodeHashMismatch);
    assert_eq!(
        fault.fix(),
        "Send the merkle nodes the far network holds under this root."
    );

    let garbled = Proof::EthereumVirtualMachine(evm::EvmProof::Storage {
        height,
        account,
        storage: nodes_with_garbage(&storage),
    });
    let fault = fault_of(&held, &prepared.params, &garbled, &headers);
    assert_eq!(fault, BridgeFault::MalformedEncoding);
    assert_eq!(
        fault.fix(),
        "Send the encoding the far network produced for this message."
    );
}

fn receipt_side(held: &MessageEnvelope) -> (Prepared, ListedHeaders) {
    let prepared = fixtures::evm_side(
        held,
        true,
        fixtures::first_base(NetworkKind::EthereumVirtualMachine),
    );
    let headers = accepted(&prepared);
    (prepared, headers)
}

fn receipt_at(prepared: &Prepared, index: u64, log_index: u32) -> Proof {
    match prepared.proof.clone() {
        Proof::EthereumVirtualMachine(evm::EvmProof::Receipt { height, nodes, .. }) => {
            Proof::EthereumVirtualMachine(evm::EvmProof::Receipt {
                height,
                index,
                nodes,
                log_index,
            })
        }
        _ => panic!("the fixture carries a receipt proof"),
    }
}

#[test]
fn a_receipt_proof_accepts_the_committed_log() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    let found = run(&held, &prepared.params, &prepared.proof, &headers).expect("accepts");
    assert_eq!(found.height, fixtures::START_HEIGHT + 1);
}

#[test]
fn a_receipt_log_from_another_contract_declines() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    let mut params = evm_parts(&prepared);
    params.contract = [0x98; 20];
    assert_eq!(
        fault_of(
            &held,
            &VerifierParameters::EthereumVirtualMachine(params),
            &prepared.proof,
            &headers
        ),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_receipt_log_under_another_topic_declines() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    let mut params = evm_parts(&prepared);
    params.topic = [0x97; 32];
    assert_eq!(
        fault_of(
            &held,
            &VerifierParameters::EthereumVirtualMachine(params),
            &prepared.proof,
            &headers
        ),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_receipt_log_naming_another_message_declines() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    assert_eq!(
        fault_of(&renamed(&held), &prepared.params, &prepared.proof, &headers),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_receipt_log_carrying_another_commitment_declines() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    assert_eq!(
        fault_of(
            &recommitted(&held),
            &prepared.params,
            &prepared.proof,
            &headers
        ),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_receipt_position_holding_no_such_log_declines() {
    let held = envelope();
    let (prepared, headers) = receipt_side(&held);
    for (index, log_index) in [(0u64, 1u32), (1, 0), (9, 0)] {
        assert_eq!(
            fault_of(
                &held,
                &prepared.params,
                &receipt_at(&prepared, index, log_index),
                &headers
            ),
            BridgeFault::ValueAbsent,
            "receipt {index} log {log_index}"
        );
    }
}

fn log() -> evm::EvmLog {
    evm::EvmLog {
        address: [0x11; 20],
        topics: vec![[0x33; 32], [0x44; 32]],
        data: vec![5u8; 32],
    }
}

#[test]
fn a_typed_receipt_reads_its_log_after_the_type_byte() {
    let mut typed = vec![0x02];
    typed.extend_from_slice(&evm::encode_receipt(1, 21_000, &[log()]));
    assert_eq!(evm::read_log(&typed, 0), Ok(log()));
    assert_eq!(
        evm::read_log(&evm::encode_receipt(1, 21_000, &[log()]), 0),
        Ok(log())
    );
}

#[test]
fn a_malformed_receipt_names_the_encoding_fault() {
    let fault = Err(BridgeFault::MalformedEncoding);
    assert_eq!(evm::read_log(&[], 0), fault);
    assert_eq!(evm::read_log(&[0x02], 0), fault);
    assert_eq!(evm::read_log(&[0xf8], 0), fault);
    assert_eq!(evm::read_log(&rlp::encode_bytes(b"text"), 0), fault);
    let three = rlp::encode_list(&[
        rlp::encode_number(1),
        rlp::encode_number(2),
        rlp::encode_bytes(&[0u8; 256]),
    ]);
    assert_eq!(evm::read_log(&three, 0), fault);
    let logs_as_bytes = rlp::encode_list(&[
        rlp::encode_number(1),
        rlp::encode_number(2),
        rlp::encode_bytes(&[0u8; 256]),
        rlp::encode_bytes(b"logs"),
    ]);
    assert_eq!(evm::read_log(&logs_as_bytes, 0), fault);

    let receipt_of = |entry: Vec<u8>| {
        rlp::encode_list(&[
            rlp::encode_number(1),
            rlp::encode_number(2),
            rlp::encode_bytes(&[0u8; 256]),
            rlp::encode_list(&[entry]),
        ])
    };
    assert_eq!(
        evm::read_log(&receipt_of(rlp::encode_bytes(b"x")), 0),
        fault
    );
    let two_parts = rlp::encode_list(&[rlp::encode_bytes(&[0x11; 20]), rlp::encode_list(&[])]);
    assert_eq!(evm::read_log(&receipt_of(two_parts), 0), fault);
    let short_address = rlp::encode_list(&[
        rlp::encode_bytes(&[0x11; 19]),
        rlp::encode_list(&[]),
        rlp::encode_bytes(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(short_address), 0), fault);
    let listed_address = rlp::encode_list(&[
        rlp::encode_list(&[]),
        rlp::encode_list(&[]),
        rlp::encode_bytes(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(listed_address), 0), fault);
    let topics_as_bytes = rlp::encode_list(&[
        rlp::encode_bytes(&[0x11; 20]),
        rlp::encode_bytes(b"topics"),
        rlp::encode_bytes(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(topics_as_bytes), 0), fault);
    let short_topic = rlp::encode_list(&[
        rlp::encode_bytes(&[0x11; 20]),
        rlp::encode_list(&[rlp::encode_bytes(&[0x33; 31])]),
        rlp::encode_bytes(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(short_topic), 0), fault);
    let listed_topic = rlp::encode_list(&[
        rlp::encode_bytes(&[0x11; 20]),
        rlp::encode_list(&[rlp::encode_list(&[])]),
        rlp::encode_bytes(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(listed_topic), 0), fault);
    let listed_data = rlp::encode_list(&[
        rlp::encode_bytes(&[0x11; 20]),
        rlp::encode_list(&[]),
        rlp::encode_list(&[]),
    ]);
    assert_eq!(evm::read_log(&receipt_of(listed_data), 0), fault);
}

fn header_fields() -> Vec<Vec<u8>> {
    let mut fields = vec![
        rlp::encode_bytes(&[0xaa; 32]),
        rlp::encode_bytes(&[0u8; 32]),
        rlp::encode_bytes(&[0u8; 20]),
        rlp::encode_bytes(&[0x01; 32]),
        rlp::encode_bytes(&[0u8; 32]),
        rlp::encode_bytes(&[0x02; 32]),
        rlp::encode_bytes(&[0u8; 256]),
        rlp::encode_number(1),
        rlp::encode_number(101),
    ];
    for _ in 0..6 {
        fields.push(rlp::encode_number(3));
    }
    fields
}

#[test]
fn a_well_formed_header_reads_every_field() {
    let read = evm::read_header(&rlp::encode_list(&header_fields())).expect("reads");
    assert_eq!(read.parent, [0xaa; 32]);
    assert_eq!(read.state_root, [0x01; 32]);
    assert_eq!(read.receipts_root, [0x02; 32]);
    assert_eq!(read.number, 101);
}

#[test]
fn a_malformed_header_names_the_encoding_fault() {
    let fault = Err(BridgeFault::MalformedEncoding);
    assert_eq!(evm::read_header(&[0xf8]).map(|_| ()), fault);
    assert_eq!(
        evm::read_header(&rlp::encode_bytes(b"header")).map(|_| ()),
        fault
    );
    let mut short = header_fields();
    short.truncate(14);
    assert_eq!(
        evm::read_header(&rlp::encode_list(&short)).map(|_| ()),
        fault
    );
    for (position, replacement) in [
        (0usize, rlp::encode_list(&[])),
        (3, rlp::encode_bytes(&[0x01; 31])),
        (5, rlp::encode_bytes(&[0x02; 33])),
        (8, rlp::encode_bytes(&[0, 1])),
        (8, rlp::encode_bytes(&[1u8; 9])),
    ] {
        let mut fields = header_fields();
        fields[position] = replacement;
        assert_eq!(
            evm::read_header(&rlp::encode_list(&fields)).map(|_| ()),
            fault,
            "field {position}"
        );
    }
}

#[test]
fn a_header_batch_with_no_header_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    batch.headers = BoundedVec::default();
    assert_eq!(
        evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );
    assert_eq!(
        BridgeFault::ChainBroken.fix(),
        "Send every header between the trusted checkpoint and the block."
    );
}

#[test]
fn a_header_off_the_trusted_checkpoint_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut params = evm_parts(&prepared);
    params.checkpoint = Checkpoint {
        height: fixtures::START_HEIGHT,
        hash: [0x01; 32],
    };
    assert_eq!(
        evm::accept_headers(&params, &evm_batch(&prepared), NOW),
        Err(BridgeFault::ChainBroken)
    );
    params.checkpoint = Checkpoint {
        height: fixtures::START_HEIGHT + 5,
        hash: fixtures::first_base(NetworkKind::EthereumVirtualMachine).hash,
    };
    assert_eq!(
        evm::accept_headers(&params, &evm_batch(&prepared), NOW),
        Err(BridgeFault::ChainBroken)
    );
}

#[test]
fn an_endpoint_without_a_checkpoint_takes_the_first_header_it_sees() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut params = evm_parts(&prepared);
    params.checkpoint = Checkpoint::default();
    let found = evm::accept_headers(&params, &evm_batch(&prepared), NOW).expect("accepts");
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].height, fixtures::START_HEIGHT + 1);
    assert_eq!(found[0].at_millis, NOW);
}

#[test]
fn a_second_header_that_skips_its_parent_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let first = evm::read_header(&batch.headers[0]).expect("reads");
    let unlinked = evm::encode_header(&[0x55; 32], &[0u8; 32], &[0u8; 32], first.number + 1);
    let misnumbered = evm::encode_header(&first.hash, &[0u8; 32], &[0u8; 32], first.number + 2);
    for second in [unlinked, misnumbered] {
        let mut headers = batch.headers.to_vec();
        headers.truncate(1);
        headers.push(BoundedVec::try_from(second).expect("fits"));
        batch.headers = BoundedVec::try_from(headers).expect("fits");
        assert_eq!(
            evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
            Err(BridgeFault::ChainBroken)
        );
    }
}

#[test]
fn a_feed_signature_from_an_unregistered_key_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let stranger = fixtures::ed_pair(90);
    let mut signatures = batch.signatures.to_vec();
    signatures[0] = KeyedSignature {
        key: fixtures::ed_key(&stranger),
        signature: signatures[0].signature,
    };
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    let fault = evm::accept_headers(&evm_parts(&prepared), &batch, NOW);
    assert_eq!(fault, Err(BridgeFault::SignerUnknown));
    assert_eq!(
        BridgeFault::SignerUnknown.fix(),
        "Register the signer on the endpoint before sending the proof."
    );
}

#[test]
fn a_feed_key_signing_twice_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures[1] = signatures[0].clone();
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
        Err(BridgeFault::SignerRepeated)
    );
    assert_eq!(
        BridgeFault::SignerRepeated.fix(),
        "Send one signature per signer."
    );
}

#[test]
fn a_feed_signature_over_other_bytes_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures[0].signature[0] ^= 0x01;
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
        Err(BridgeFault::SignatureInvalid)
    );
    assert_eq!(
        BridgeFault::SignatureInvalid.fix(),
        "Send the signature the named key produced over these bytes."
    );
}

#[test]
fn a_feed_below_its_threshold_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures.truncate(1);
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
        Err(BridgeFault::ThresholdUnmet)
    );
    batch.signatures = BoundedVec::default();
    let mut params = evm_parts(&prepared);
    params.header_threshold = 0;
    assert_eq!(
        evm::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ThresholdUnmet)
    );
    assert_eq!(
        BridgeFault::ThresholdUnmet.fix(),
        "Gather signatures reaching the threshold the endpoint requires."
    );
}

#[test]
fn a_trie_walk_over_too_many_nodes_declines() {
    let nodes: Vec<Vec<u8>> = (0..=mpt::MAX_STEPS)
        .map(|_| rlp::encode_list(&[]))
        .collect();
    assert_eq!(
        mpt::walk(&[0u8; 32], b"key", &nodes),
        Err(BridgeFault::ProofTooLarge)
    );
    assert_eq!(
        BridgeFault::ProofTooLarge.fix(),
        "Split the work into proofs sitting inside the element limits."
    );
}

// --- The Cosmos verifier -------------------------------------------------

fn cosmos_parts(prepared: &Prepared) -> (crate::endpoint::CosmosParameters, cosmos::CosmosProof) {
    match (&prepared.params, &prepared.proof) {
        (VerifierParameters::Cosmos(params), Proof::Cosmos(proof)) => {
            (params.clone(), proof.clone())
        }
        _ => panic!("the fixture carries Cosmos parts"),
    }
}

fn cosmos_fault(
    held: &MessageEnvelope,
    params: crate::endpoint::CosmosParameters,
    proof: cosmos::CosmosProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Cosmos(params),
        &Proof::Cosmos(proof),
        &ListedHeaders::default(),
    )
}

#[test]
fn a_cosmos_commit_names_the_power_that_signed() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);
    let found = run(
        &held,
        &prepared.params,
        &prepared.proof,
        &ListedHeaders::default(),
    )
    .expect("accepts");
    assert_eq!(
        found.trust,
        TrustAssumption::ValidatorThreshold {
            signed: 40,
            total: 40
        }
    );
}

#[test]
fn a_cosmos_proof_at_or_below_the_checkpoint_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);
    let (mut params, proof) = cosmos_parts(&prepared);
    params.checkpoint.height = proof.height;
    assert_eq!(cosmos_fault(&held, params, proof), BridgeFault::ChainBroken);
}

#[test]
fn a_cosmos_commit_with_a_bad_signature_list_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[0].signer = 40;
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::SignerUnknown
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[1] = signatures[0].clone();
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::SignerRepeated
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    proof.round = 1;
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::SignatureInvalid
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures.truncate(2);
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::ThresholdUnmet
    );
}

#[test]
fn a_cosmos_app_hash_off_the_block_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut branch = proof.app_hash_branch.to_vec();
    branch[0][0] ^= 0x01;
    proof.app_hash_branch = BoundedVec::try_from(branch).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::NodeHashMismatch
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    proof.app_hash_total = 0;
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::MalformedEncoding
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    proof.app_hash_index = proof.app_hash_total;
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::MalformedEncoding
    );
}

#[test]
fn a_cosmos_store_proof_with_a_bad_shape_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);

    let (params, mut proof) = cosmos_parts(&prepared);
    proof.store_proof.value = BoundedVec::try_from(vec![1u8; 5]).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::MalformedEncoding
    );

    let (mut params, proof) = cosmos_parts(&prepared);
    params.store_prefix = BoundedVec::try_from(b"another".to_vec()).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::CommitmentMismatch
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut value = proof.store_proof.value.to_vec();
    value[0] ^= 0x01;
    proof.store_proof.value = BoundedVec::try_from(value).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::NodeHashMismatch
    );
}

#[test]
fn a_cosmos_message_proof_for_another_message_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);

    let (mut params, proof) = cosmos_parts(&prepared);
    params.key_prefix = BoundedVec::try_from(b"elsewhere/".to_vec()).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::CommitmentMismatch
    );

    let (params, proof) = cosmos_parts(&prepared);
    assert_eq!(
        cosmos_fault(&recommitted(&held), params, proof),
        BridgeFault::CommitmentMismatch
    );

    let (params, mut proof) = cosmos_parts(&prepared);
    let mut value = proof.message_proof.value.to_vec();
    value[0] ^= 0x01;
    proof.message_proof.value = BoundedVec::try_from(value).expect("fits");
    assert_eq!(
        cosmos_fault(&held, params, proof),
        BridgeFault::NodeHashMismatch
    );
}

fn step(prefix: Vec<u8>, suffix: Vec<u8>) -> cosmos::InnerStep {
    cosmos::InnerStep {
        prefix: BoundedVec::try_from(prefix).expect("fits"),
        suffix: BoundedVec::try_from(suffix).expect("fits"),
    }
}

#[test]
fn an_existence_proof_with_a_step_outside_its_spec_declines() {
    let entries = vec![
        (b"alpha".to_vec(), vec![1u8; 8]),
        (b"beta".to_vec(), vec![2u8; 8]),
    ];
    let (proof, root) = cosmos::build_store_proof(&entries, 0).expect("builds");
    assert_eq!(proof.check(&root), Ok(()));
    assert_eq!(proof.check(&[0u8; 32]), Err(BridgeFault::NodeHashMismatch));

    for bad in [
        step(vec![1u8; 2], vec![]),
        step(vec![1u8; 12 + 33 + 1], vec![]),
        step(vec![1u8; 4], vec![0x20; 5]),
    ] {
        let mut held = proof.clone();
        held.path = BoundedVec::try_from(vec![bad]).expect("fits");
        assert_eq!(held.fold(), Err(BridgeFault::MalformedEncoding));
    }

    let (root_proof, _) = cosmos::build_root_proof(&entries, 1).expect("builds");
    let mut held = root_proof;
    held.path = BoundedVec::try_from(vec![step(vec![0u8], vec![])]).expect("fits");
    assert_eq!(held.fold(), Err(BridgeFault::MalformedEncoding));

    assert!(cosmos::build_store_proof(&[], 0).is_none());
    assert!(cosmos::build_root_proof(&entries, 2).is_none());
}

#[test]
fn a_cosmos_proof_spec_names_its_shape() {
    assert_eq!(cosmos::ProofSpec::Store.min_prefix(), 4);
    assert_eq!(cosmos::ProofSpec::Root.min_prefix(), 1);
    assert_eq!(cosmos::ProofSpec::Store.max_prefix(), 12);
    assert_eq!(cosmos::ProofSpec::Root.max_prefix(), 1);
    assert_eq!(cosmos::ProofSpec::Store.child_size(), 33);
    assert_eq!(cosmos::ProofSpec::Root.child_size(), 32);
    assert_eq!(cosmos::ProofSpec::Store.separator(), &[0x20u8]);
    assert!(cosmos::ProofSpec::Root.separator().is_empty());
    assert_eq!(cosmos::ProofSpec::default(), cosmos::ProofSpec::Store);
}

#[test]
fn a_cosmos_header_record_carries_the_app_hash() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Cosmos, &held, NOW);
    let (_, proof) = cosmos_parts(&prepared);
    let record = cosmos::record_of(&proof, NOW, 30, 40);
    assert_eq!(record.height, proof.height);
    assert_eq!(record.state_root, proof.app_hash);
    assert_eq!(
        record.trust,
        TrustAssumption::ValidatorThreshold {
            signed: 30,
            total: 40
        }
    );
    assert_eq!(cosmos::varint_signed(-1).len(), 10);
}

// --- The Solana verifier -------------------------------------------------

fn solana_parts(
    prepared: &Prepared,
) -> (
    crate::endpoint::SolanaParameters,
    solana::SolanaHeaderBatch,
    solana::SolanaProof,
) {
    match (&prepared.params, &prepared.headers, &prepared.proof) {
        (
            VerifierParameters::Solana(params),
            Some(HeaderBatch::Solana(batch)),
            Proof::Solana(proof),
        ) => (params.clone(), batch.clone(), proof.clone()),
        _ => panic!("the fixture carries Solana parts"),
    }
}

fn solana_fault(
    held: &MessageEnvelope,
    prepared: &Prepared,
    params: crate::endpoint::SolanaParameters,
    proof: solana::SolanaProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Solana(params),
        &Proof::Solana(proof),
        &accepted(prepared),
    )
}

#[test]
fn a_solana_proof_at_an_unrecorded_slot_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);
    let (params, _, mut proof) = solana_parts(&prepared);
    proof.slot += 9;
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::RootUnknown
    );
}

#[test]
fn a_solana_account_outside_the_endpoint_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);

    let (mut params, _, proof) = solana_parts(&prepared);
    params.account = [0x01; 32];
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::CommitmentMismatch
    );

    let (mut params, _, proof) = solana_parts(&prepared);
    params.program = [0x02; 32];
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_solana_account_path_off_the_delta_hash_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);

    let (params, _, mut proof) = solana_parts(&prepared);
    let mut steps = proof.steps.to_vec();
    steps[0].position = 40;
    proof.steps = BoundedVec::try_from(steps).expect("fits");
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::NodeHashMismatch
    );

    let (params, _, mut proof) = solana_parts(&prepared);
    let mut steps = proof.steps.to_vec();
    steps.truncate(1);
    proof.steps = BoundedVec::try_from(steps).expect("fits");
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::NodeHashMismatch
    );

    let (params, _, mut proof) = solana_parts(&prepared);
    proof.account.lamports += 1;
    assert_eq!(
        solana_fault(&held, &prepared, params, proof),
        BridgeFault::NodeHashMismatch
    );
}

#[test]
fn a_solana_account_holding_another_message_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);
    let (params, _, proof) = solana_parts(&prepared);
    assert_eq!(
        solana_fault(&renamed(&held), &prepared, params.clone(), proof.clone()),
        BridgeFault::ValueAbsent
    );
    assert_eq!(
        solana_fault(&recommitted(&held), &prepared, params, proof),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_solana_commitment_reads_from_any_entry_of_the_account() {
    let first = [1u8; 32];
    let second = [2u8; 32];
    let mut data = solana::write_commitment(&first, &[0x10; 32]);
    data.extend_from_slice(&solana::write_commitment(&second, &[0x20; 32]));
    assert_eq!(solana::read_commitment(&data, &second), Some([0x20; 32]));
    assert_eq!(solana::read_commitment(&data, &[3u8; 32]), None);
    assert_eq!(solana::read_commitment(&data[..63], &first), None);
}

#[test]
fn a_solana_batch_with_a_broken_chain_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);

    let (params, mut batch, _) = solana_parts(&prepared);
    batch.headers = BoundedVec::default();
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );

    let (mut params, batch, _) = solana_parts(&prepared);
    params.checkpoint.hash = [0x03; 32];
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let first = batch.headers[0].clone();
    let unlinked = solana::SlotHeader {
        slot: first.slot + 1,
        parent_bank_hash: [0x04; 32],
        ..first.clone()
    };
    let stale = solana::SlotHeader {
        slot: first.slot,
        parent_bank_hash: first.bank_hash(),
        ..first.clone()
    };
    for second in [unlinked, stale] {
        batch.headers = BoundedVec::try_from(vec![first.clone(), second]).expect("fits");
        assert_eq!(
            solana::accept_headers(&params, &batch, NOW),
            Err(BridgeFault::ChainBroken)
        );
    }
}

#[test]
fn a_solana_batch_with_bad_relayer_signatures_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);

    let (params, mut batch, _) = solana_parts(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures[0].key = [0x05; 32];
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignerUnknown)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures[1] = signatures[0].clone();
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignerRepeated)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures[0].signature[5] ^= 0x01;
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignatureInvalid)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let mut signatures = batch.signatures.to_vec();
    signatures.truncate(1);
    batch.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ThresholdUnmet)
    );
}

#[test]
fn a_solana_batch_with_a_bad_leader_signature_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);

    let (params, mut batch, _) = solana_parts(&prepared);
    batch.leader = None;
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ThresholdUnmet)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let relayer = batch.signatures[0].clone();
    batch.leader = Some(relayer);
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignerUnknown)
    );

    let (params, mut batch, _) = solana_parts(&prepared);
    let mut leader = batch.leader.clone().expect("the fixture carries a leader");
    leader.signature[0] ^= 0x01;
    batch.leader = Some(leader);
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignatureInvalid)
    );

    let (mut params, mut batch, _) = solana_parts(&prepared);
    params.leaders = BoundedVec::default();
    batch.leader = None;
    let found = solana::accept_headers(&params, &batch, NOW).expect("accepts");
    assert_eq!(found[0].height, fixtures::START_HEIGHT + 1);
}

// --- The Polkadot verifier -----------------------------------------------

fn polkadot_parts(
    prepared: &Prepared,
) -> (crate::endpoint::PolkadotParameters, polkadot::PolkadotProof) {
    match (&prepared.params, &prepared.proof) {
        (VerifierParameters::Polkadot(params), Proof::Polkadot(proof)) => {
            (params.clone(), proof.clone())
        }
        _ => panic!("the fixture carries Polkadot parts"),
    }
}

fn polkadot_fault(
    held: &MessageEnvelope,
    params: crate::endpoint::PolkadotParameters,
    proof: polkadot::PolkadotProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Polkadot(params),
        &Proof::Polkadot(proof),
        &ListedHeaders::default(),
    )
}

#[test]
fn a_polkadot_settlement_names_the_weight_that_signed() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);
    let found = run(
        &held,
        &prepared.params,
        &prepared.proof,
        &ListedHeaders::default(),
    )
    .expect("accepts");
    assert_eq!(
        found.trust,
        TrustAssumption::ValidatorThreshold {
            signed: 4,
            total: 4
        }
    );
    assert_eq!(found.height, fixtures::START_HEIGHT + 1);
}

#[test]
fn a_polkadot_proof_from_another_authority_set_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);
    let (params, mut proof) = polkadot_parts(&prepared);
    proof.set_id += 1;
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::SignerUnknown
    );
}

#[test]
fn a_polkadot_header_that_fails_to_read_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);
    let (params, mut proof) = polkadot_parts(&prepared);
    proof.header = BoundedVec::try_from(vec![0u8; 40]).expect("fits");
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::MalformedEncoding
    );
}

#[test]
fn a_polkadot_block_at_the_checkpoint_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);
    let (mut params, proof) = polkadot_parts(&prepared);
    params.checkpoint.height = fixtures::START_HEIGHT + 1;
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::ChainBroken
    );
}

#[test]
fn a_polkadot_settlement_with_bad_signatures_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);

    let (params, mut proof) = polkadot_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[0].signer = 17;
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::SignerUnknown
    );

    let (params, mut proof) = polkadot_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[3] = signatures[2].clone();
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::SignerRepeated
    );

    // One Edwards curve signature and one Ristretto signature each fail
    // on their own scheme.
    for position in [0usize, 2] {
        let (params, mut proof) = polkadot_parts(&prepared);
        let mut signatures = proof.signatures.to_vec();
        signatures[position].signature[1] ^= 0x01;
        proof.signatures = BoundedVec::try_from(signatures).expect("fits");
        assert_eq!(
            polkadot_fault(&held, params, proof),
            BridgeFault::SignatureInvalid
        );
    }

    let (params, mut proof) = polkadot_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures.truncate(2);
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::ThresholdUnmet
    );
}

#[test]
fn a_polkadot_storage_proof_off_the_state_root_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);

    let (params, mut proof) = polkadot_parts(&prepared);
    let mut value = proof.value.to_vec();
    value[0] ^= 0x01;
    proof.value = BoundedVec::try_from(value).expect("fits");
    assert_eq!(
        polkadot_fault(&held, params, proof),
        BridgeFault::NodeHashMismatch
    );

    let (params, proof) = polkadot_parts(&prepared);
    assert_eq!(
        polkadot_fault(&renamed(&held), params, proof),
        BridgeFault::NodeHashMismatch
    );
}

#[test]
fn a_polkadot_value_lacking_the_commitment_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Polkadot, &held, NOW);
    let (params, proof) = polkadot_parts(&prepared);
    assert_eq!(
        polkadot_fault(&recommitted(&held), params, proof),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_compact_number_reads_in_every_mode() {
    assert_eq!(polkadot::read_compact(&[0b0000_0100], 0), Ok((1, 1)));
    let two = codec::Compact(300u32).encode();
    assert_eq!(polkadot::read_compact(&two, 0), Ok((300, 2)));
    let four = codec::Compact(70_000u32).encode();
    assert_eq!(polkadot::read_compact(&four, 0), Ok((70_000, 4)));
    let big = codec::Compact(u64::MAX / 3).encode();
    assert_eq!(
        polkadot::read_compact(&big, 0),
        Ok((u64::MAX / 3, big.len()))
    );

    let fault = Err(BridgeFault::MalformedEncoding);
    assert_eq!(polkadot::read_compact(&[], 0), fault);
    assert_eq!(polkadot::read_compact(&two[..1], 0), fault);
    assert_eq!(polkadot::read_compact(&four[..3], 0), fault);
    assert_eq!(polkadot::read_compact(&big[..3], 0), fault);
    assert_eq!(polkadot::read_compact(&[0b1111_1111; 12], 0), fault);
}

#[test]
fn a_polkadot_number_above_the_block_range_declines() {
    let mut header = vec![0x11; 32];
    header.extend_from_slice(&codec::Compact(u64::from(u32::MAX) + 1).encode());
    header.extend_from_slice(&[0x22; 64]);
    assert_eq!(
        polkadot::read_header(&header).map(|_| ()),
        Err(BridgeFault::MalformedEncoding)
    );
    let fine = polkadot::encode_header(&[0x11; 32], 9, &[0x22; 32], &[0x33; 32]);
    let read = polkadot::read_header(&fine).expect("reads");
    let record = polkadot::record_of(&read, NOW, 3, 4);
    assert_eq!(record.height, 9);
    assert_eq!(record.extra_root, [0x33; 32]);
}

// --- The Avalanche verifier ----------------------------------------------

fn avalanche_parts(
    prepared: &Prepared,
) -> (
    crate::endpoint::AvalancheParameters,
    avalanche::AvalancheProof,
) {
    match (&prepared.params, &prepared.proof) {
        (VerifierParameters::Avalanche(params), Proof::Avalanche(proof)) => {
            (params.clone(), proof.clone())
        }
        _ => panic!("the fixture carries Avalanche parts"),
    }
}

fn avalanche_fault(
    held: &MessageEnvelope,
    params: crate::endpoint::AvalancheParameters,
    proof: avalanche::AvalancheProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Avalanche(params),
        &Proof::Avalanche(proof),
        &ListedHeaders::default(),
    )
}

#[test]
fn an_avalanche_message_from_another_chain_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, NOW);
    let (params, mut proof) = avalanche_parts(&prepared);
    proof.source_chain = [0x01; 32];
    assert_eq!(
        avalanche_fault(&held, params, proof),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn an_avalanche_message_with_bad_signatures_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, NOW);

    let (params, mut proof) = avalanche_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[0].signer = 9;
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        avalanche_fault(&held, params, proof),
        BridgeFault::SignerUnknown
    );

    let (params, mut proof) = avalanche_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures[1] = signatures[0].clone();
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        avalanche_fault(&held, params, proof),
        BridgeFault::SignerRepeated
    );

    let (params, proof) = avalanche_parts(&prepared);
    assert_eq!(
        avalanche_fault(&recommitted(&held), params, proof),
        BridgeFault::SignatureInvalid
    );

    let (params, mut proof) = avalanche_parts(&prepared);
    let mut signatures = proof.signatures.to_vec();
    signatures.truncate(2);
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        avalanche_fault(&held, params, proof),
        BridgeFault::ThresholdUnmet
    );
}

#[test]
fn an_avalanche_endpoint_without_a_share_takes_the_default_share() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Avalanche, &held, NOW);
    let (mut params, mut proof) = avalanche_parts(&prepared);
    params.threshold_percent = 0;
    assert_eq!(
        avalanche::threshold_of(&params),
        avalanche::DEFAULT_THRESHOLD_PERCENT
    );
    // Three of four equal validators hold seventy five percent, which
    // clears the default share of sixty seven percent.
    let found = run(
        &held,
        &VerifierParameters::Avalanche(params.clone()),
        &Proof::Avalanche(proof.clone()),
        &ListedHeaders::default(),
    )
    .expect("accepts");
    assert_eq!(
        found.trust,
        TrustAssumption::ValidatorThreshold {
            signed: 75,
            total: 100
        }
    );
    let mut signatures = proof.signatures.to_vec();
    signatures.truncate(2);
    proof.signatures = BoundedVec::try_from(signatures).expect("fits");
    assert_eq!(
        avalanche_fault(&held, params, proof),
        BridgeFault::ThresholdUnmet
    );
}

// --- The Bitcoin verifier ------------------------------------------------

fn bitcoin_parts(
    prepared: &Prepared,
) -> (
    crate::endpoint::BitcoinParameters,
    bitcoin::BitcoinHeaderBatch,
    bitcoin::BitcoinProof,
) {
    match (&prepared.params, &prepared.headers, &prepared.proof) {
        (
            VerifierParameters::Bitcoin(params),
            Some(HeaderBatch::Bitcoin(batch)),
            Proof::Bitcoin(proof),
        ) => (params.clone(), batch.clone(), proof.clone()),
        _ => panic!("the fixture carries Bitcoin parts"),
    }
}

fn bitcoin_fault(
    held: &MessageEnvelope,
    prepared: &Prepared,
    params: crate::endpoint::BitcoinParameters,
    proof: bitcoin::BitcoinProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Bitcoin(params),
        &Proof::Bitcoin(proof),
        &accepted(prepared),
    )
}

#[test]
fn a_bitcoin_proof_names_its_confirmations() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let found = run(
        &held,
        &prepared.params,
        &prepared.proof,
        &accepted(&prepared),
    )
    .expect("accepts");
    assert_eq!(
        found.trust,
        TrustAssumption::AccumulatedWork { confirmations: 2 }
    );
}

#[test]
fn a_bitcoin_proof_short_of_its_depth_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let (mut params, _, proof) = bitcoin_parts(&prepared);
    params.confirmations = 3;
    assert_eq!(
        bitcoin_fault(&held, &prepared, params, proof),
        BridgeFault::TooFewConfirmations
    );
    let (params, _, mut proof) = bitcoin_parts(&prepared);
    proof.height += 5;
    assert_eq!(
        bitcoin_fault(&held, &prepared, params, proof),
        BridgeFault::RootUnknown
    );
}

#[test]
fn a_bitcoin_transaction_that_fails_to_read_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let (params, _, mut proof) = bitcoin_parts(&prepared);
    let mut raw = proof.transaction.to_vec();
    raw.truncate(raw.len() - 2);
    proof.transaction = BoundedVec::try_from(raw).expect("fits");
    assert_eq!(
        bitcoin_fault(&held, &prepared, params, proof),
        BridgeFault::MalformedEncoding
    );
}

#[test]
fn a_bitcoin_branch_off_the_merkle_root_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let (params, _, mut proof) = bitcoin_parts(&prepared);
    proof.position = 0;
    assert_eq!(
        bitcoin_fault(&held, &prepared, params, proof),
        BridgeFault::NodeHashMismatch
    );
}

#[test]
fn a_bitcoin_output_carrying_no_anchor_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    for output_index in [1u32, 7] {
        let (params, _, mut proof) = bitcoin_parts(&prepared);
        proof.output_index = output_index;
        assert_eq!(
            bitcoin_fault(&held, &prepared, params, proof),
            BridgeFault::ValueAbsent
        );
    }
    let (params, _, proof) = bitcoin_parts(&prepared);
    assert_eq!(
        bitcoin_fault(&recommitted(&held), &prepared, params, proof),
        BridgeFault::CommitmentMismatch
    );
}

#[test]
fn a_bitcoin_batch_with_a_broken_chain_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);

    let (params, mut batch, _) = bitcoin_parts(&prepared);
    batch.headers = BoundedVec::default();
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );

    let (mut params, batch, _) = bitcoin_parts(&prepared);
    params.checkpoint.hash = [0x07; 32];
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );

    let (params, mut batch, _) = bitcoin_parts(&prepared);
    let mut headers = batch.headers.to_vec();
    headers.swap(0, 1);
    batch.headers = BoundedVec::try_from(headers).expect("fits");
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::ChainBroken)
    );

    let (params, mut batch, _) = bitcoin_parts(&prepared);
    batch.headers = BoundedVec::try_from(vec![BoundedVec::try_from(vec![0u8; 79]).expect("fits")])
        .expect("fits");
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::MalformedEncoding)
    );
}

#[test]
fn a_bitcoin_endpoint_without_a_checkpoint_hash_takes_the_first_header() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let (mut params, batch, _) = bitcoin_parts(&prepared);
    params.checkpoint.hash = [0u8; 32];
    params.work_limit = [0u8; 32];
    let found = bitcoin::accept_headers(&params, &batch, NOW).expect("accepts");
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].height, fixtures::START_HEIGHT + 2);
}

#[test]
fn a_bitcoin_header_with_an_unreadable_target_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Bitcoin, &held, NOW);
    let (params, mut batch, _) = bitcoin_parts(&prepared);
    let mut raw = batch.headers[0].to_vec();
    raw[72..76].copy_from_slice(&0x2000_0000u32.to_le_bytes());
    batch.headers =
        BoundedVec::try_from(vec![BoundedVec::try_from(raw).expect("fits")]).expect("fits");
    assert_eq!(
        bitcoin::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::WorkTooLow)
    );
    assert_eq!(
        BridgeFault::WorkTooLow.fix(),
        "Send headers from the network the endpoint names."
    );
}

#[test]
fn a_packed_target_outside_the_rules_unpacks_to_nothing() {
    assert_eq!(bitcoin::unpack_target(0x0300_0000), None);
    assert_eq!(bitcoin::unpack_target(0x0380_0000), None);
    assert_eq!(bitcoin::unpack_target(0x2100_ffff), None);
    let small = bitcoin::unpack_target(0x0112_3456).expect("unpacks");
    assert_eq!(&small[28..], &[0, 0, 0, 0x12]);
    let three = bitcoin::unpack_target(0x0312_3456).expect("unpacks");
    assert_eq!(&three[28..], &[0, 0x12, 0x34, 0x56]);
    let whole = bitcoin::unpack_target(0x2012_3456).expect("unpacks");
    assert_eq!(&whole[..3], &[0x12, 0x34, 0x56]);
}

#[test]
fn a_varint_of_every_width_reads_back_through_a_transaction() {
    for count in [0xfcu64, 0xfd, 0x1_0000, 0x1_0000_0000] {
        let written = bitcoin::write_varint(count);
        let expected = match count {
            0..=0xfc => 1,
            0xfd..=0xffff => 3,
            0x1_0000..=0xffff_ffff => 5,
            _ => 9,
        };
        assert_eq!(written.len(), expected, "count {count}");
    }

    // A transaction naming more outputs than it carries runs out of bytes
    // and declines rather than reading past its end. The widest form here
    // carries a small count in nine bytes.
    let wide_three = vec![0xffu8, 3, 0, 0, 0, 0, 0, 0, 0];
    for count in [
        bitcoin::write_varint(0xfd),
        bitcoin::write_varint(0x1_0000),
        wide_three,
    ] {
        let mut raw = 2u32.to_le_bytes().to_vec();
        raw.extend_from_slice(&bitcoin::write_varint(0));
        raw.extend_from_slice(&count);
        assert_eq!(
            bitcoin::read_transaction(&raw).map(|_| ()),
            Err(BridgeFault::MalformedEncoding),
            "count {count:?}"
        );
    }
    for truncated in [vec![0xfdu8, 1], vec![0xfe, 1, 2], vec![0xff, 1, 2, 3]] {
        let mut raw = 2u32.to_le_bytes().to_vec();
        raw.extend_from_slice(&truncated);
        assert_eq!(
            bitcoin::read_transaction(&raw).map(|_| ()),
            Err(BridgeFault::MalformedEncoding)
        );
    }
}

#[test]
fn a_segregated_witness_transaction_keeps_the_identifier_of_its_plain_form() {
    let plain = bitcoin::encode_anchor_transaction(&[0x5a; 32], 0, b"CRLA", &[9u8; 32], 7, &[1]);
    let plain_read = bitcoin::read_transaction(&plain).expect("reads");

    // The witness form carries the marker and flag after the version and
    // one witness stack per input before the lock time.
    let mut witness = plain[..4].to_vec();
    witness.extend_from_slice(&[0x00, 0x01]);
    witness.extend_from_slice(&plain[4..plain.len() - 4]);
    witness.extend_from_slice(&bitcoin::write_varint(2));
    witness.extend_from_slice(&bitcoin::write_varint(3));
    witness.extend_from_slice(&[0xaa; 3]);
    witness.extend_from_slice(&bitcoin::write_varint(1));
    witness.extend_from_slice(&[0xbb]);
    witness.extend_from_slice(&plain[plain.len() - 4..]);
    let witness_read = bitcoin::read_transaction(&witness).expect("reads");
    assert_eq!(witness_read.txid, plain_read.txid);
    assert_eq!(witness_read.outputs, plain_read.outputs);

    let cut = &witness[..witness.len() - 6];
    assert_eq!(
        bitcoin::read_transaction(cut).map(|_| ()),
        Err(BridgeFault::MalformedEncoding)
    );
    assert_eq!(
        bitcoin::read_transaction(&[1, 2]).map(|_| ()),
        Err(BridgeFault::MalformedEncoding)
    );
}

#[test]
fn an_anchor_script_of_the_wrong_shape_reads_nothing() {
    let value = [4u8; 32];
    let script = bitcoin::write_anchor(b"CRLA", &value);
    assert_eq!(bitcoin::read_anchor(&script, b"CRLA"), Some(value));
    assert_eq!(bitcoin::read_anchor(&script, b"XXXX"), None);
    assert_eq!(bitcoin::read_anchor(&script, b"CRL"), None);
    assert_eq!(bitcoin::read_anchor(&script[..10], b"CRLA"), None);
    assert_eq!(bitcoin::read_anchor(&[0x6a], b"CRLA"), None);
    let mut other = script.clone();
    other[0] = 0x76;
    assert_eq!(bitcoin::read_anchor(&other, b"CRLA"), None);
    assert_eq!(bitcoin::read_anchor(&[], b"CRLA"), None);
}

// --- The rollup verifier -------------------------------------------------

fn rollup_parts(prepared: &Prepared) -> (crate::endpoint::RollupParameters, rollup::RollupProof) {
    match (&prepared.params, &prepared.proof) {
        (VerifierParameters::Rollup(params), Proof::Rollup(proof)) => {
            (params.clone(), proof.clone())
        }
        _ => panic!("the fixture carries rollup parts"),
    }
}

fn rollup_fault(
    held: &MessageEnvelope,
    prepared: &Prepared,
    params: crate::endpoint::RollupParameters,
    proof: rollup::RollupProof,
) -> BridgeFault {
    fault_of(
        held,
        &VerifierParameters::Rollup(params),
        &Proof::Rollup(proof),
        &accepted(prepared),
    )
}

#[test]
fn a_rollup_proof_names_its_challenge_window() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let found = run(
        &held,
        &prepared.params,
        &prepared.proof,
        &accepted(&prepared),
    )
    .expect("accepts");
    assert_eq!(
        found.trust,
        TrustAssumption::SettledAfterChallenge {
            window_millis: fixtures::ROLLUP_WINDOW
        }
    );
}

#[test]
fn a_rollup_proof_at_an_unsettled_height_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let (params, mut proof) = rollup_parts(&prepared);
    proof.settlement_height += 3;
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::RootUnknown
    );
    let (mut params, proof) = rollup_parts(&prepared);
    params.settlement.confirmations = 4;
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::TooFewConfirmations
    );
}

#[test]
fn a_rollup_output_root_that_differs_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let (params, mut proof) = rollup_parts(&prepared);
    proof.withdrawal_root = [0x01; 32];
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::CommitmentMismatch
    );
    let (params, mut proof) = rollup_parts(&prepared);
    proof.output_index = 1;
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::ValueAbsent
    );
    let (mut params, proof) = rollup_parts(&prepared);
    params.output_contract = [0x02; 20];
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::ValueAbsent
    );
}

#[test]
fn a_rollup_output_missing_its_moment_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let (params, mut proof) = rollup_parts(&prepared);
    proof.moment_storage = BoundedVec::default();
    assert_eq!(
        rollup_fault(&held, &prepared, params, proof),
        BridgeFault::NodeHashMismatch
    );
}

#[test]
fn a_rollup_output_inside_its_window_names_the_wait() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let headers = accepted(&prepared);
    let early = EndpointState::new(
        &prepared.params,
        MessageFormat::CanonicalValue,
        &headers,
        NOW - 60_000,
    );
    assert_eq!(
        verify::verify(&held, &prepared.proof, &early).map(|_| ()),
        Err(BridgeFault::ChallengeWindowOpen)
    );
    assert_eq!(
        BridgeFault::ChallengeWindowOpen.fix(),
        "Wait for the challenge window on the endpoint to pass and send the proof again."
    );
}

#[test]
fn a_rollup_message_the_rollup_never_wrote_declines() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Rollup, &held, NOW);
    let (params, proof) = rollup_parts(&prepared);
    assert_eq!(
        rollup_fault(&renamed(&held), &prepared, params.clone(), proof.clone()),
        BridgeFault::ValueAbsent
    );
    assert_eq!(
        rollup_fault(
            &recommitted(&held),
            &prepared,
            params.clone(),
            proof.clone()
        ),
        BridgeFault::CommitmentMismatch
    );
    let mut moved = params;
    moved.contract = [0x03; 20];
    assert_eq!(
        rollup_fault(&held, &prepared, moved, proof),
        BridgeFault::ValueAbsent
    );
}

#[test]
fn a_rollup_word_adds_with_carry() {
    let mut word = [0u8; 32];
    word[31] = 0xff;
    let sum = rollup::add_word(&word, 1);
    assert_eq!(&sum[30..], &[1, 0]);
    assert_eq!(rollup::word_number(&sum), 256);
    assert_eq!(rollup::add_word(&word, 0), word);
    let preimage = rollup::output_preimage(&[1u8; 32], &[2u8; 32], &[3u8; 32], &[4u8; 32]);
    assert_eq!(preimage.len(), 128);
    assert_eq!(
        sp_crypto_hashing::keccak_256(&preimage),
        rollup::output_root(&[1u8; 32], &[2u8; 32], &[3u8; 32], &[4u8; 32])
    );
}

// --- Shared verifier helpers ---------------------------------------------

#[test]
fn a_depth_check_names_too_few_confirmations() {
    let record = |height: u64| HeaderRecord {
        height,
        ..HeaderRecord::default()
    };
    let headers = ListedHeaders::many(vec![record(10), record(11), record(12)]);
    let params = fixtures::prepare(NetworkKind::Avalanche, &envelope(), NOW).params;
    let state = EndpointState::new(&params, MessageFormat::RawBytes, &headers, NOW);
    assert_eq!(verify::confirmations_for(&state, 10, 3), Ok(3));
    assert_eq!(verify::confirmations_for(&state, 12, 0), Ok(1));
    assert_eq!(
        verify::confirmations_for(&state, 11, 3),
        Err(BridgeFault::TooFewConfirmations)
    );
    assert_eq!(
        verify::confirmations_for(&state, 13, 1),
        Err(BridgeFault::TooFewConfirmations)
    );
    assert_eq!(verify::accepted_header(&state, 11), Ok(record(11)));
    assert_eq!(
        verify::accepted_header(&state, 9),
        Err(BridgeFault::RootUnknown)
    );
    assert_eq!(ListedHeaders::one(record(4)).tip_height(), 4);
}

#[test]
fn a_word_reads_only_from_thirty_two_bytes_or_more() {
    assert_eq!(verify::word_from(&[7u8; 31]), None);
    assert_eq!(verify::word_from(&[7u8; 40]), Some([7u8; 32]));
    let sr = fixtures::sr_pair(3);
    let signature = sr.sign(b"bytes").0;
    assert!(verify::check_sr25519(&sr.public().0, b"bytes", &signature));
    assert!(!verify::check_sr25519(&sr.public().0, b"other", &signature));
}

// --- Batches carrying more than one header ------------------------------

/// The feed signatures of the virtual machine fixture over one header.
fn evm_signed(
    header: &evm::EvmHeader,
) -> BoundedVec<KeyedSignature, frame_support::traits::ConstU32<{ verify::MAX_SIGNATURES }>> {
    let payload = evm::feed_payload(31_337, header);
    let signed: Vec<KeyedSignature> = [1u8, 2, 3]
        .iter()
        .map(|seed| {
            let pair = fixtures::ed_pair(*seed);
            KeyedSignature {
                key: fixtures::ed_key(&pair),
                signature: fixtures::ed_sign(&pair, &payload),
            }
        })
        .collect();
    BoundedVec::try_from(signed).expect("three signatures fit")
}

#[test]
fn a_virtual_machine_batch_signed_over_its_highest_header_records_every_header() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::EthereumVirtualMachine, &held, NOW);
    let mut batch = evm_batch(&prepared);
    let first = evm::read_header(&batch.headers[0]).expect("reads");
    let encoded = evm::encode_header(&first.hash, &[0x09; 32], &[0x0a; 32], first.number + 1);
    let second = evm::read_header(&encoded).expect("reads");
    let mut headers = batch.headers.to_vec();
    headers.push(BoundedVec::try_from(encoded).expect("fits"));
    batch.headers = BoundedVec::try_from(headers).expect("fits");

    // Signatures over the lower header alone leave the highest header
    // unsigned, so the batch declines.
    assert_eq!(
        evm::accept_headers(&evm_parts(&prepared), &batch, NOW),
        Err(BridgeFault::SignatureInvalid)
    );

    batch.signatures = evm_signed(&second);
    let found = evm::accept_headers(&evm_parts(&prepared), &batch, NOW).expect("accepts");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].height, first.number);
    assert_eq!(found[1].height, second.number);
    assert_eq!(found[1].state_root, [0x09; 32]);
    for record in found.iter() {
        assert_eq!(
            record.trust,
            TrustAssumption::RelayerAttestation {
                signed: 3,
                registered: 3
            }
        );
    }
}

#[test]
fn a_solana_batch_signed_over_its_highest_header_records_every_header() {
    let held = envelope();
    let prepared = fixtures::prepare(NetworkKind::Solana, &held, NOW);
    let (params, mut batch, _) = solana_parts(&prepared);
    let first = batch.headers[0].clone();
    let second = solana::SlotHeader {
        slot: first.slot + 1,
        parent_bank_hash: first.bank_hash(),
        ..first.clone()
    };
    batch.headers = BoundedVec::try_from(vec![first.clone(), second.clone()]).expect("fits");

    // The leader and the relayers signed the lower header, so the highest
    // header carries no valid signature.
    assert_eq!(
        solana::accept_headers(&params, &batch, NOW),
        Err(BridgeFault::SignatureInvalid)
    );

    let payload = second.payload();
    let relayers: Vec<KeyedSignature> = [21u8, 22, 23]
        .iter()
        .map(|seed| {
            let pair = fixtures::ed_pair(*seed);
            KeyedSignature {
                key: fixtures::ed_key(&pair),
                signature: fixtures::ed_sign(&pair, &payload),
            }
        })
        .collect();
    batch.signatures = BoundedVec::try_from(relayers).expect("three signatures fit");
    let leader = fixtures::ed_pair(24);
    batch.leader = Some(KeyedSignature {
        key: fixtures::ed_key(&leader),
        signature: fixtures::ed_sign(&leader, &payload),
    });
    let found = solana::accept_headers(&params, &batch, NOW).expect("accepts");
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].height, first.slot);
    assert_eq!(found[1].height, second.slot);
    assert_eq!(found[1].hash, second.bank_hash());
}

#[test]
fn a_transaction_claiming_more_outputs_than_its_bytes_hold_declines() {
    // Version one, zero inputs, then an output count of the largest value
    // the count encoding carries and no output bytes behind it.
    let mut claimed = vec![1u8, 0, 0, 0, 0x00, 0xff];
    claimed.extend_from_slice(&u64::MAX.to_le_bytes());
    assert_eq!(
        bitcoin::read_transaction(&claimed).map(|held| held.outputs.len()),
        Err(BridgeFault::MalformedEncoding)
    );

    // Two outputs claimed and one present.
    let mut short = vec![1u8, 0, 0, 0, 0x00, 0x02];
    short.extend_from_slice(&5u64.to_le_bytes());
    short.push(0x00);
    assert_eq!(
        bitcoin::read_transaction(&short).map(|held| held.outputs.len()),
        Err(BridgeFault::MalformedEncoding)
    );
}
