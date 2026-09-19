//! Every network-reachable message path bounded and repriced.
//!
//! A network-reachable path is any place where a Cerulea node turns bytes a
//! stranger sent into a value. A stranger chooses those bytes. These tests walk
//! each path of the enumeration at `chain/docs/NETWORK_BOUNDS.md` and assert two
//! properties. An oversized message is refused before any decoding runs. The
//! price the path charges rises with the byte count.

use cerulea_consensus::engines::bounds::{admit, admit_and_decode, MessagePath};
use cerulea_consensus::engines::commit::aggregate::{
    AggregateSignature, SignerBitmap, SCHEME_ENVELOPE_V1,
};
use cerulea_consensus::engines::commit::submission::{read_submission, CommitSubmission};
use cerulea_consensus::engines::EngineError;
use codec::Encode;

/// Design assertion 1 of the security domain.
///
/// Every network-reachable path rejects an oversized message.
#[test]
fn every_path_rejects_an_oversized_message() {
    for path in MessagePath::ALL {
        let error = admit(path, path.max_bytes() + 1)
            .expect_err("the message crosses the bound of its path");
        assert!(
            matches!(error, EngineError::MessageAboveBound { .. }),
            "{} refuses on the length",
            path.label()
        );
        let text = error.to_string();
        assert!(text.contains(path.label()));
        assert!(text.contains("Fix:"));
    }
}

#[test]
fn every_path_admits_a_message_at_its_bound() {
    for path in MessagePath::ALL {
        let admitted = admit(path, path.max_bytes()).expect("a message at the bound fits");
        assert_eq!(admitted.bytes, path.max_bytes());
        assert_eq!(admitted.cost, path.ceiling_cost());
    }
}

#[test]
fn every_path_prices_its_bytes() {
    for path in MessagePath::ALL {
        let empty = admit(path, 0).expect("an empty message fits");
        let half = admit(path, path.max_bytes() / 2).expect("half a message fits");
        let full = admit(path, path.max_bytes()).expect("a full message fits");
        assert!(
            empty.cost < half.cost && half.cost < full.cost,
            "{} prices its bytes",
            path.label()
        );
        assert_eq!(empty.cost, path.base_cost());
    }
}

#[test]
fn an_oversized_message_never_reaches_the_decoder() {
    for path in MessagePath::ALL {
        // A byte pattern that decodes to a valid value where it reaches the
        // decoder. The size check runs first, so the decoder never sees it.
        let mut bytes = 7u64.encode();
        bytes.resize(path.max_bytes() + 1, 0);
        let outcome = admit_and_decode::<u64>(path, &bytes);
        assert!(
            matches!(outcome, Err(EngineError::MessageAboveBound { .. })),
            "{} refuses before decoding",
            path.label()
        );
    }
}

#[test]
fn the_vote_submission_path_holds_its_stated_bound() {
    let submission = CommitSubmission::new(AggregateSignature {
        scheme: SCHEME_ENVELOPE_V1,
        slot: cerulea_consensus::engines::Slot(4),
        block_hash: [1u8; 32],
        signers: SignerBitmap::new(8),
        signatures: Vec::new(),
    });
    let bytes = submission.encode();
    assert!(bytes.len() <= MessagePath::CommitSubmission.max_bytes());
    read_submission(&bytes).expect("a submission inside the bound reads back");

    let oversized = vec![0u8; MessagePath::CommitSubmission.max_bytes() + 1];
    let error = read_submission(&oversized).expect_err("the oversized submission is refused");
    assert!(error.to_string().contains("Fix:"));
}

#[test]
fn the_equivocation_report_path_holds_its_stated_bound() {
    use cerulea_consensus::engines::authoring::equivocation::{
        read_report, MAX_EQUIVOCATION_REPORT_BYTES,
    };
    assert_eq!(
        MAX_EQUIVOCATION_REPORT_BYTES,
        MessagePath::EquivocationReport.max_bytes(),
        "the node and the shared table name one bound"
    );
    let oversized = vec![0u8; MAX_EQUIVOCATION_REPORT_BYTES + 1];
    assert!(read_report(&oversized).is_err());
}

#[test]
fn the_propagation_path_holds_its_stated_bound() {
    let oversized = vec![0u8; MessagePath::GossipMessage.max_bytes() + 1];
    let error = admit(MessagePath::GossipMessage, oversized.len())
        .expect_err("the oversized message is refused");
    assert!(error.to_string().contains("propagation message"));
}

#[test]
fn the_custody_answer_path_holds_its_stated_bound() {
    // The custody answer carries a record and its inclusion proof. The record
    // limit and the proof step limit of the Custody Engine sit inside the byte
    // bound of the path.
    let record_bytes = pallet_cerulea_custody::primitives::MAX_RECORD_BYTES as usize;
    let proof_steps = pallet_cerulea_custody::primitives::MAX_PROOF_STEPS as usize;
    let widest = record_bytes + proof_steps * 33 + 64;
    assert!(
        widest <= MessagePath::CustodyAnswer.max_bytes(),
        "the widest custody answer fits inside the bound of its path"
    );
    assert!(admit(MessagePath::CustodyAnswer, widest).is_ok());
    assert!(admit(
        MessagePath::CustodyAnswer,
        MessagePath::CustodyAnswer.max_bytes() + 1
    )
    .is_err());
}

#[test]
fn the_bridge_proof_path_holds_its_stated_bound() {
    // The bridge proof carries the proof nodes, the headers and the encoded
    // transaction. Each part carries its own bound inside the module.
    let nodes = pallet_cerulea_bridge::verify::MAX_PROOF_NODES as usize
        * pallet_cerulea_bridge::verify::MAX_NODE_BYTES as usize;
    assert!(
        nodes <= MessagePath::BridgeProof.max_bytes(),
        "the widest proof node set fits inside the bound of its path"
    );
    assert!(admit(MessagePath::BridgeProof, nodes).is_ok());
    assert!(admit(
        MessagePath::BridgeProof,
        MessagePath::BridgeProof.max_bytes() + 1
    )
    .is_err());
}

#[test]
fn the_extrinsic_body_path_matches_the_runtime_block_length() {
    // The runtime carries a block of five megabytes and gives three quarters of
    // it to the ordinary transaction class. One extrinsic body therefore never
    // crosses that figure.
    let normal = 5 * 1024 * 1024 * 75 / 100;
    assert_eq!(MessagePath::ExtrinsicBody.max_bytes(), normal);
    assert!(admit(MessagePath::ExtrinsicBody, normal).is_ok());
    assert!(admit(MessagePath::ExtrinsicBody, normal + 1).is_err());
}

#[test]
fn the_interface_body_path_holds_its_stated_bound() {
    assert_eq!(MessagePath::RpcBody.max_bytes(), 10 * 1024 * 1024);
    assert!(admit(MessagePath::RpcBody, 10 * 1024 * 1024).is_ok());
    assert!(admit(MessagePath::RpcBody, 10 * 1024 * 1024 + 1).is_err());
}

#[test]
fn the_enumeration_names_every_path_the_table_holds() {
    let document = include_str!("../../docs/NETWORK_BOUNDS.md");
    for path in MessagePath::ALL {
        assert!(
            document.contains(path.label()),
            "the enumeration names the {}",
            path.label()
        );
    }
}
