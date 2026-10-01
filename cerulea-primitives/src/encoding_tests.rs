//! Tests of the wire form of the shared primitives.
//!
//! The node sends these values to peers and the runtime stores them. Each
//! value therefore decodes back to itself under the memory limit the node
//! applies, fits inside the size the runtime reserves for it, and reaches the
//! chain metadata the Cerulea Explorer reads.

use crate::authoring::{CommitteeSeat, SeatInput};
use crate::bounds::MessagePath;
use crate::checkpoint::{Checkpoint, CheckpointFault};
use crate::equivocation::{
    EquivocationFacts, EquivocationFault, EquivocationProof, SealedHead, SEAL_CLAIM_BYTES,
};
use alloc::string::String;
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemLimit, DecodeWithMemTracking, Encode, MaxEncodedLen};
use core::fmt::Debug;
use scale_info::{TypeDef, TypeInfo};

/// The memory ceiling a node applies while it decodes a message from a peer.
const PEER_DECODE_LIMIT: usize = 64 * 1_024;

/// Send one value across the wire and read it back under the peer limit.
fn crosses_the_wire<T>(value: T) -> T
where
    T: Encode + Decode + DecodeWithMemTracking + MaxEncodedLen + PartialEq + Debug,
{
    let bytes = value.encode();
    assert!(
        bytes.len() <= T::max_encoded_len(),
        "the encoded value fits the size the runtime reserves"
    );
    let plain = T::decode(&mut &bytes[..]).expect("the value decodes");
    let limited = T::decode_with_mem_limit(&mut &bytes[..], PEER_DECODE_LIMIT)
        .expect("the value decodes under the peer limit");
    assert_eq!(plain, value);
    assert_eq!(limited, value);
    limited
}

/// The variant names the chain metadata carries for one enum.
fn variant_names<T: TypeInfo + 'static>() -> Vec<String> {
    match T::type_info().type_def {
        TypeDef::Variant(def) => def
            .variants
            .iter()
            .map(|variant| String::from(variant.name))
            .collect(),
        _ => panic!("the type reaches the metadata as an enum"),
    }
}

/// The field names the chain metadata carries for one struct.
fn field_names<T: TypeInfo + 'static>() -> Vec<String> {
    match T::type_info().type_def {
        TypeDef::Composite(def) => def
            .fields
            .iter()
            .filter_map(|field| field.name.map(String::from))
            .collect(),
        _ => panic!("the type reaches the metadata as a struct"),
    }
}

fn head(byte: u8) -> SealedHead {
    let mut claim = [0u8; SEAL_CLAIM_BYTES];
    claim[0] = byte;
    SealedHead::new([byte; 32], claim, [byte; 64])
}

#[test]
fn every_checkpoint_fault_crosses_the_wire_and_reaches_the_metadata() {
    let faults = [
        CheckpointFault::HashMismatch,
        CheckpointFault::BelowAnchor,
        CheckpointFault::OutOfOrder,
        CheckpointFault::BlockOutOfOrder,
    ];
    for fault in faults {
        assert_eq!(crosses_the_wire(fault), fault);
    }
    assert_eq!(
        variant_names::<CheckpointFault>(),
        [
            "HashMismatch",
            "BelowAnchor",
            "OutOfOrder",
            "BlockOutOfOrder"
        ]
    );
    assert!(CheckpointFault::decode(&mut &[4u8][..]).is_err());
}

#[test]
fn every_equivocation_fault_crosses_the_wire_and_reaches_the_metadata() {
    let faults = [
        EquivocationFault::IdentityMismatch,
        EquivocationFault::OneHeaderOnly,
        EquivocationFault::FirstSignatureRejected,
        EquivocationFault::SecondSignatureRejected,
    ];
    for fault in faults {
        assert_eq!(crosses_the_wire(fault), fault);
    }
    assert_eq!(
        variant_names::<EquivocationFault>(),
        [
            "IdentityMismatch",
            "OneHeaderOnly",
            "FirstSignatureRejected",
            "SecondSignatureRejected"
        ]
    );
    assert!(EquivocationFault::decode(&mut &[9u8][..]).is_err());
}

#[test]
fn every_message_path_crosses_the_wire_and_reaches_the_metadata() {
    for path in MessagePath::ALL {
        assert_eq!(crosses_the_wire(path), path);
    }
    assert_eq!(variant_names::<MessagePath>().len(), MessagePath::ALL.len());
    assert!(MessagePath::decode(&mut &[10u8][..]).is_err());
}

#[test]
fn an_equivocation_proof_and_its_facts_cross_the_wire() {
    let proof = EquivocationProof::new(head(1), head(2));
    assert_eq!(crosses_the_wire(proof), proof);
    assert_eq!(proof.encoded_size(), EquivocationProof::max_encoded_len());
    assert_eq!(crosses_the_wire(head(3)), head(3));

    let facts = EquivocationFacts {
        slot: 44,
        rank: 2,
        seal_key: [5u8; 32],
        draw_key: [6u8; 32],
        first_hash: [7u8; 32],
        second_hash: [8u8; 32],
    };
    assert_eq!(crosses_the_wire(facts), facts);
    assert_eq!(
        field_names::<EquivocationFacts>(),
        [
            "slot",
            "rank",
            "seal_key",
            "draw_key",
            "first_hash",
            "second_hash"
        ]
    );
    assert_eq!(
        field_names::<SealedHead>(),
        ["pre_seal_hash", "claim", "signature"]
    );
    assert_eq!(field_names::<EquivocationProof>(), ["first", "second"]);
}

#[test]
fn a_truncated_proof_fails_to_decode() {
    let bytes = EquivocationProof::new(head(1), head(2)).encode();
    let short = &bytes[..bytes.len() - 1];
    assert!(EquivocationProof::decode(&mut &short[..]).is_err());
}

#[test]
fn a_seat_and_a_candidate_cross_the_wire() {
    let candidate = SeatInput {
        draw_key: [1u8; 32],
        epoch_output: [2u8; 32],
        standing: 750_000,
    };
    assert_eq!(crosses_the_wire_plain(candidate), candidate);
    let seat = CommitteeSeat {
        draw_key: [3u8; 32],
        rank: 1,
        seat_value: u128::MAX,
    };
    assert_eq!(crosses_the_wire_plain(seat), seat);
    assert_eq!(
        field_names::<SeatInput>(),
        ["draw_key", "epoch_output", "standing"]
    );
    assert_eq!(
        field_names::<CommitteeSeat>(),
        ["draw_key", "rank", "seat_value"]
    );
}

#[test]
fn a_checkpoint_crosses_the_wire_and_sorts_by_its_index() {
    let first = Checkpoint::new(0, 900, [1u8; 32], [2u8; 32]);
    let second = Checkpoint::new(1, 1_800, [0u8; 32], [0u8; 32]);
    assert_eq!(crosses_the_wire(second), second);
    let mut set = alloc::vec![second, first];
    set.sort();
    assert_eq!(set, [first, second]);
    assert_eq!(
        field_names::<Checkpoint>(),
        ["index", "block", "hash", "state_root"]
    );
}

/// Send one value across the wire where the type carries no memory tracking.
fn crosses_the_wire_plain<T>(value: T) -> T
where
    T: Encode + Decode + MaxEncodedLen + PartialEq + Debug,
{
    let bytes = value.encode();
    assert!(bytes.len() <= T::max_encoded_len());
    T::decode(&mut &bytes[..]).expect("the value decodes")
}
