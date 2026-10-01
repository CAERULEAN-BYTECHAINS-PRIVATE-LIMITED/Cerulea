//! The verifier for the Bitcoin network and the anchoring path.
//!
//! The Bitcoin network settles a block through the work spent on it. A
//! header carries the target the work met. A header hashing below that
//! target carries the work. A chain of such headers above a trusted block
//! makes a block hard to replace.
//!
//! # The header
//!
//! A header is eighty bytes. The fields are the version, the hash of the
//! header below, the transaction merkle root, the moment, the packed
//! target and the counter. The hash of a header is the doubled hash of
//! those eighty bytes, read as a number with the low byte first.
//!
//! # The packed target
//!
//! The target sits in four bytes. The first byte is the size. The
//! remaining three bytes are the leading digits. The target is the digits
//! shifted left by eight times the size less three. The verifier unpacks
//! the target, declines a target above the limit the endpoint holds, and
//! declines a header whose hash sits above its own target.
//!
//! # The transaction proof
//!
//! A transaction sits in a tree of doubled hashes. A level holding an odd
//! count repeats its last entry. The proof carries the position and the
//! sibling at each level. The fold reaches the merkle root the header
//! carries.
//!
//! # The anchoring path
//!
//! Cerulea writes a state root into an output that carries data rather
//! than value. The output script starts with the marker byte for data,
//! then a push of the marker the endpoint holds followed by the state
//! root. The off-chain driver builds and broadcasts that transaction. A
//! later submission carries the transaction, the merkle branch and the
//! headers above it. The verifier reads the output, compares the state
//! root and counts the confirmations.

use crate::endpoint::{BitcoinParameters, EndpointState, HeaderRecord, VerifierParameters};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::merkle;
use crate::verify::{
    accepted_header, confirmations_for, Branch, RawTransaction, Verifier, MAX_HEADERS,
};
use alloc::{vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The size of one header, in bytes.
pub const HEADER_BYTES: usize = 80;

/// The marker byte a data carrying output script starts with.
pub const DATA_MARKER: u8 = 0x6a;

/// The fields one header carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BitcoinHeader {
    /// The hash of the header, read with the high byte first.
    pub hash: [u8; 32],
    /// The hash of the header below.
    pub parent: [u8; 32],
    /// The root of the transaction tree.
    pub merkle_root: [u8; 32],
    /// The moment the header names.
    pub moment: u32,
    /// The packed target the work met.
    pub bits: u32,
}

/// One encoded header.
pub type RawBitcoinHeader = BoundedVec<u8, ConstU32<{ HEADER_BYTES as u32 }>>;

/// Read the fields of one encoded header.
///
/// The encoded bytes carry every hash with the low byte first. The reader
/// turns each hash around, so the whole module compares hashes with the
/// high byte first.
pub fn read_header(encoded: &[u8]) -> Result<BitcoinHeader, BridgeFault> {
    if encoded.len() != HEADER_BYTES {
        return Err(BridgeFault::MalformedEncoding);
    }
    let mut parent = [0u8; 32];
    parent.copy_from_slice(&encoded[4..36]);
    parent.reverse();
    let mut merkle_root = [0u8; 32];
    merkle_root.copy_from_slice(&encoded[36..68]);
    merkle_root.reverse();
    let moment = u32::from_le_bytes([encoded[68], encoded[69], encoded[70], encoded[71]]);
    let bits = u32::from_le_bytes([encoded[72], encoded[73], encoded[74], encoded[75]]);
    let mut hash = merkle::doubled(encoded);
    hash.reverse();
    Ok(BitcoinHeader {
        hash,
        parent,
        merkle_root,
        moment,
        bits,
    })
}

/// Unpack the target one header names.
///
/// The answer carries the target as thirty two bytes with the high byte
/// first. A packed value naming a size above thirty two carries no target.
pub fn unpack_target(bits: u32) -> Option<[u8; 32]> {
    let size = (bits >> 24) as usize;
    let digits = bits & 0x00ff_ffff;
    if digits == 0 {
        return None;
    }
    if digits & 0x0080_0000 != 0 {
        return None;
    }
    let mut found = [0u8; 32];
    if size <= 3 {
        let shifted = digits >> (8 * (3 - size));
        found[28..].copy_from_slice(&shifted.to_be_bytes());
        return Some(found);
    }
    if size > 32 {
        return None;
    }
    let end = 32 - (size - 3);
    if end < 3 {
        return None;
    }
    let bytes = digits.to_be_bytes();
    found[end - 3..end].copy_from_slice(&bytes[1..]);
    Some(found)
}

/// Report whether one hash sits at or below one target.
pub fn meets_target(hash: &[u8; 32], target: &[u8; 32]) -> bool {
    hash.as_slice() <= target.as_slice()
}

/// One batch of headers the driver submits.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct BitcoinHeaderBatch {
    /// The encoded headers, the lowest first.
    pub headers: BoundedVec<RawBitcoinHeader, ConstU32<MAX_HEADERS>>,
}

/// Check one header batch and answer with the headers to record.
///
/// Each header links to the header below. Each header carries a target at
/// or below the limit the endpoint holds. Each header hashes at or below
/// its own target.
pub fn accept_headers(
    params: &BitcoinParameters,
    batch: &BitcoinHeaderBatch,
    now_millis: u64,
) -> Result<Vec<HeaderRecord>, BridgeFault> {
    if batch.headers.is_empty() {
        return Err(BridgeFault::ChainBroken);
    }
    let mut found: Vec<HeaderRecord> = Vec::with_capacity(batch.headers.len());
    let mut previous: Option<BitcoinHeader> = None;
    let mut height = params.checkpoint.height;
    for encoded in batch.headers.iter() {
        let header = read_header(encoded.as_slice())?;
        match previous {
            Some(below) => {
                if header.parent != below.hash {
                    return Err(BridgeFault::ChainBroken);
                }
            }
            None => {
                if params.checkpoint.hash != [0u8; 32] && header.parent != params.checkpoint.hash {
                    return Err(BridgeFault::ChainBroken);
                }
            }
        }
        let target = unpack_target(header.bits).ok_or(BridgeFault::WorkTooLow)?;
        if params.work_limit != [0u8; 32] && target.as_slice() > params.work_limit.as_slice() {
            return Err(BridgeFault::WorkTooLow);
        }
        if !meets_target(&header.hash, &target) {
            return Err(BridgeFault::WorkTooLow);
        }
        height = height.saturating_add(1);
        found.push(HeaderRecord {
            height,
            hash: header.hash,
            state_root: header.merkle_root,
            extra_root: header.merkle_root,
            at_millis: now_millis,
            trust: TrustAssumption::AccumulatedWork {
                confirmations: params.confirmations,
            },
        });
        previous = Some(header);
    }
    Ok(found)
}

/// One output of one transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Output {
    /// The value the output carries.
    pub value: u64,
    /// The script the output carries.
    pub script: Vec<u8>,
}

/// One parsed transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transaction {
    /// The identifier of the transaction.
    pub txid: [u8; 32],
    /// The outputs the transaction carries.
    pub outputs: Vec<Output>,
}

/// Read one whole number written in the variable length form.
fn read_varint(bytes: &[u8], at: usize) -> Result<(u64, usize), BridgeFault> {
    let first = *bytes.get(at).ok_or(BridgeFault::MalformedEncoding)?;
    match first {
        0xfd => {
            let held = bytes
                .get(at + 1..at + 3)
                .ok_or(BridgeFault::MalformedEncoding)?;
            Ok((u16::from_le_bytes([held[0], held[1]]) as u64, at + 3))
        }
        0xfe => {
            let held = bytes
                .get(at + 1..at + 5)
                .ok_or(BridgeFault::MalformedEncoding)?;
            Ok((
                u32::from_le_bytes([held[0], held[1], held[2], held[3]]) as u64,
                at + 5,
            ))
        }
        0xff => {
            let held = bytes
                .get(at + 1..at + 9)
                .ok_or(BridgeFault::MalformedEncoding)?;
            let mut value = 0u64;
            for (position, byte) in held.iter().enumerate() {
                value |= (*byte as u64) << (position * 8);
            }
            Ok((value, at + 9))
        }
        _ => Ok((first as u64, at + 1)),
    }
}

/// Write one whole number in the variable length form.
pub fn write_varint(value: u64) -> Vec<u8> {
    if value < 0xfd {
        return vec![value as u8];
    }
    if value <= u16::MAX as u64 {
        let mut found = vec![0xfd];
        found.extend_from_slice(&(value as u16).to_le_bytes());
        return found;
    }
    if value <= u32::MAX as u64 {
        let mut found = vec![0xfe];
        found.extend_from_slice(&(value as u32).to_le_bytes());
        return found;
    }
    let mut found = vec![0xff];
    found.extend_from_slice(&value.to_le_bytes());
    found
}

/// Read one transaction and answer with its identifier and its outputs.
///
/// A transaction carrying witness data holds a marker byte and a flag byte
/// after the version. The identifier covers the form without the witness
/// data, so the reader rebuilds that form and hashes it.
pub fn read_transaction(bytes: &[u8]) -> Result<Transaction, BridgeFault> {
    let version = bytes.get(..4).ok_or(BridgeFault::MalformedEncoding)?;
    let mut cursor = 4usize;
    let segregated = bytes.get(4) == Some(&0x00) && bytes.get(5) == Some(&0x01);
    if segregated {
        cursor = 6;
    }

    let (input_count, next) = read_varint(bytes, cursor)?;
    cursor = next;
    let inputs_start = cursor;
    for _ in 0..input_count {
        cursor = cursor
            .checked_add(36)
            .ok_or(BridgeFault::MalformedEncoding)?;
        let (script_len, next) = read_varint(bytes, cursor)?;
        cursor = next
            .checked_add(script_len as usize)
            .ok_or(BridgeFault::MalformedEncoding)?;
        cursor = cursor
            .checked_add(4)
            .ok_or(BridgeFault::MalformedEncoding)?;
    }
    let inputs_end = cursor;

    let (output_count, next) = read_varint(bytes, cursor)?;
    cursor = next;
    let outputs_start = cursor;
    // Every output takes at least nine bytes: eight for the value and one
    // for the script length. A count the remaining bytes cannot hold is
    // malformed, and the reader declines it before reserving memory.
    let room = (bytes.len().saturating_sub(cursor) / 9) as u64;
    if output_count > room {
        return Err(BridgeFault::MalformedEncoding);
    }
    let mut outputs: Vec<Output> = Vec::with_capacity(output_count as usize);
    for _ in 0..output_count {
        let held = bytes
            .get(cursor..cursor + 8)
            .ok_or(BridgeFault::MalformedEncoding)?;
        let mut value = 0u64;
        for (position, byte) in held.iter().enumerate() {
            value |= (*byte as u64) << (position * 8);
        }
        cursor += 8;
        let (script_len, next) = read_varint(bytes, cursor)?;
        let end = next
            .checked_add(script_len as usize)
            .ok_or(BridgeFault::MalformedEncoding)?;
        let script = bytes
            .get(next..end)
            .ok_or(BridgeFault::MalformedEncoding)?
            .to_vec();
        outputs.push(Output { value, script });
        cursor = end;
    }
    let outputs_end = cursor;

    if segregated {
        for _ in 0..input_count {
            let (items, next) = read_varint(bytes, cursor)?;
            cursor = next;
            for _ in 0..items {
                let (item_len, next) = read_varint(bytes, cursor)?;
                cursor = next
                    .checked_add(item_len as usize)
                    .ok_or(BridgeFault::MalformedEncoding)?;
            }
        }
    }
    let lock_time = bytes
        .get(cursor..cursor + 4)
        .ok_or(BridgeFault::MalformedEncoding)?;

    let mut plain: Vec<u8> = Vec::with_capacity(bytes.len());
    plain.extend_from_slice(version);
    plain.extend_from_slice(&write_varint(input_count));
    plain.extend_from_slice(
        bytes
            .get(inputs_start..inputs_end)
            .ok_or(BridgeFault::MalformedEncoding)?,
    );
    plain.extend_from_slice(&write_varint(output_count));
    plain.extend_from_slice(
        bytes
            .get(outputs_start..outputs_end)
            .ok_or(BridgeFault::MalformedEncoding)?,
    );
    plain.extend_from_slice(lock_time);

    let mut txid = merkle::doubled(&plain);
    txid.reverse();
    Ok(Transaction { txid, outputs })
}

/// Read the thirty two byte value one data carrying output holds.
///
/// The script starts with the marker byte for data. A push of the marker
/// the endpoint holds followed by the value follows. The reader answers
/// with the value where the marker matches.
pub fn read_anchor(script: &[u8], marker: &[u8]) -> Option<[u8; 32]> {
    if script.first() != Some(&DATA_MARKER) {
        return None;
    }
    let length = *script.get(1)? as usize;
    let body = script.get(2..2 + length)?;
    if body.len() != marker.len() + 32 {
        return None;
    }
    if &body[..marker.len()] != marker {
        return None;
    }
    let mut found = [0u8; 32];
    found.copy_from_slice(&body[marker.len()..]);
    Some(found)
}

/// Build the script one data carrying output holds.
pub fn write_anchor(marker: &[u8], value: &[u8; 32]) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::with_capacity(marker.len() + 32);
    body.extend_from_slice(marker);
    body.extend_from_slice(value);
    let mut found: Vec<u8> = Vec::with_capacity(body.len() + 2);
    found.push(DATA_MARKER);
    found.push(body.len() as u8);
    found.extend_from_slice(&body);
    found
}

/// The proof one message from the Bitcoin network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct BitcoinProof {
    /// The height of the block the transaction sits in.
    pub height: u64,
    /// The encoded transaction.
    pub transaction: RawTransaction,
    /// The position of the transaction in the block.
    pub position: u32,
    /// The sibling hash at each level, the leaf level first.
    pub branch: Branch,
    /// The position of the data carrying output in the transaction.
    pub output_index: u32,
}

/// The verifier of the Bitcoin network.
pub struct BitcoinVerifier;

impl Verifier for BitcoinVerifier {
    type Proof = BitcoinProof;

    fn kind() -> NetworkKind {
        NetworkKind::Bitcoin
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &BitcoinProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Bitcoin(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        let header = accepted_header(endpoint, proof.height)?;
        let confirmations = confirmations_for(endpoint, proof.height, params.confirmations)?;
        let transaction = read_transaction(proof.transaction.as_slice())?;
        check_branch(&transaction.txid, proof, &header.state_root)?;

        let output = transaction
            .outputs
            .get(proof.output_index as usize)
            .ok_or(BridgeFault::ValueAbsent)?;
        let held = read_anchor(&output.script, params.anchor_marker.as_slice())
            .ok_or(BridgeFault::ValueAbsent)?;
        let commitment = envelope.commitment();
        if held != commitment {
            return Err(BridgeFault::CommitmentMismatch);
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id: envelope.id(),
            height: proof.height,
            block_hash: header.hash,
            trust: TrustAssumption::AccumulatedWork { confirmations },
        })
    }
}

/// Check the merkle branch of one transaction against one root.
///
/// The encoded tree carries every hash with the low byte first. The reader
/// turns the identifier and the branch around, folds them and turns the
/// result back, so the comparison runs against the root the header reader
/// produced.
pub fn check_branch(
    txid: &[u8; 32],
    proof: &BitcoinProof,
    merkle_root: &[u8; 32],
) -> Result<(), BridgeFault> {
    let mut leaf = *txid;
    leaf.reverse();
    let branch: Vec<[u8; 32]> = proof
        .branch
        .iter()
        .map(|held| {
            let mut found = *held;
            found.reverse();
            found
        })
        .collect();
    let mut folded = merkle::doubled_fold(&leaf, proof.position, &branch);
    folded.reverse();
    if &folded == merkle_root {
        Ok(())
    } else {
        Err(BridgeFault::NodeHashMismatch)
    }
}

/// Build the encoding of one header from its fields.
///
/// The builder writes the eighty bytes a header carries. The bridge uses
/// the builder while it produces fixtures and while it searches for a
/// counter meeting a target.
pub fn encode_header(
    parent: &[u8; 32],
    merkle_root: &[u8; 32],
    moment: u32,
    bits: u32,
    counter: u32,
) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(HEADER_BYTES);
    found.extend_from_slice(&1u32.to_le_bytes());
    let mut held = *parent;
    held.reverse();
    found.extend_from_slice(&held);
    let mut root = *merkle_root;
    root.reverse();
    found.extend_from_slice(&root);
    found.extend_from_slice(&moment.to_le_bytes());
    found.extend_from_slice(&bits.to_le_bytes());
    found.extend_from_slice(&counter.to_le_bytes());
    found
}

/// Build the encoding of one transaction carrying one data output.
///
/// The transaction holds one input spending a named earlier output and two
/// outputs. The first output carries the data. The second output carries
/// the change. The bridge uses the builder while it produces fixtures and
/// while the off-chain driver prepares an anchoring transaction.
pub fn encode_anchor_transaction(
    previous_txid: &[u8; 32],
    previous_index: u32,
    marker: &[u8],
    value: &[u8; 32],
    change: u64,
    change_script: &[u8],
) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::new();
    found.extend_from_slice(&2u32.to_le_bytes());
    found.extend_from_slice(&write_varint(1));
    let mut held = *previous_txid;
    held.reverse();
    found.extend_from_slice(&held);
    found.extend_from_slice(&previous_index.to_le_bytes());
    found.extend_from_slice(&write_varint(0));
    found.extend_from_slice(&u32::MAX.to_le_bytes());
    found.extend_from_slice(&write_varint(2));
    let script = write_anchor(marker, value);
    found.extend_from_slice(&0u64.to_le_bytes());
    found.extend_from_slice(&write_varint(script.len() as u64));
    found.extend_from_slice(&script);
    found.extend_from_slice(&change.to_le_bytes());
    found.extend_from_slice(&write_varint(change_script.len() as u64));
    found.extend_from_slice(change_script);
    found.extend_from_slice(&0u32.to_le_bytes());
    found
}
