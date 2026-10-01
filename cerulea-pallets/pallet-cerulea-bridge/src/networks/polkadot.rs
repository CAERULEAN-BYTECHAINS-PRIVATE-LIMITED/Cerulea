//! The verifier for a Polkadot network.
//!
//! A Polkadot network settles a block once a registered authority set
//! signs it. Each authority signs a payload naming the block hash, the
//! block number, the round and the identifier of the authority set. A
//! block is settled once the signing weight passes two thirds of the total
//! weight. The verifier here rebuilds that payload, checks every signature
//! and adds the weight.
//!
//! # The chain of custody
//!
//! 1. **The header.** The proof carries the encoded header. The hash of
//!    the header is the hash of those bytes. The header carries the state
//!    trie root, so a signature over the hash covers the state root.
//! 2. **The signatures.** Each authority signs the settlement payload.
//!    An authority signs under the Edwards curve or under the Ristretto
//!    Schnorr scheme, and the registered entry names which.
//! 3. **The storage proof.** The trie nodes prove one key and one value
//!    against the state trie root. The proof format is the one the trie
//!    crate of the pinned set produces.
//! 4. **The message.** The proved value carries the commitment the
//!    envelope holds.
//!
//! # The settlement payload
//!
//! The payload is the canonical binary encoding of five parts in order. A
//! marker byte of one names a settling vote. The block hash follows. The
//! block number follows as four bytes. The round follows as eight bytes.
//! The identifier of the authority set follows as eight bytes. A node of
//! the far network writes the same bytes, so a signature made there
//! verifies here without translation.

use crate::endpoint::{
    reaches_two_thirds, total_weight, AuthoritySet, EndpointState, HeaderRecord,
    PolkadotParameters, SignatureScheme, VerifierParameters,
};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::verify::{
    check_ed25519, check_sr25519, raw_nodes, IndexedSignature, ProofNodes, RawHeader, StoreValue,
    Verifier, MAX_SIGNATURES,
};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;
use sp_runtime::traits::BlakeTwo256;
use sp_trie::LayoutV1;

/// The marker byte a settling vote carries.
pub const VOTE_MARKER: u8 = 1;

/// The fields one encoded header carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PolkadotHeader {
    /// The hash of the header.
    pub hash: [u8; 32],
    /// The hash of the header below.
    pub parent: [u8; 32],
    /// The block number.
    pub number: u32,
    /// The root of the state trie at this height.
    pub state_root: [u8; 32],
    /// The root of the extrinsic trie at this height.
    pub extrinsics_root: [u8; 32],
}

/// Read the fields of one encoded header.
///
/// The encoding writes the parent hash as thirty two bytes, the block
/// number in the compact whole number form, then the state root and the
/// extrinsics root as thirty two bytes each. The reader takes those five
/// fields and leaves the digest behind them alone.
pub fn read_header(encoded: &[u8]) -> Result<PolkadotHeader, BridgeFault> {
    let parent = take_hash(encoded, 0)?;
    let (number, used) = read_compact(encoded, 32)?;
    let state_root = take_hash(encoded, used)?;
    let extrinsics_root = take_hash(encoded, used.saturating_add(32))?;
    Ok(PolkadotHeader {
        hash: sp_crypto_hashing::blake2_256(encoded),
        parent,
        number: u32::try_from(number).map_err(|_| BridgeFault::MalformedEncoding)?,
        state_root,
        extrinsics_root,
    })
}

/// Read thirty two bytes at one offset.
fn take_hash(bytes: &[u8], at: usize) -> Result<[u8; 32], BridgeFault> {
    let end = at.checked_add(32).ok_or(BridgeFault::MalformedEncoding)?;
    let held = bytes.get(at..end).ok_or(BridgeFault::MalformedEncoding)?;
    let mut found = [0u8; 32];
    found.copy_from_slice(held);
    Ok(found)
}

/// Read one compact whole number and answer with the value and the offset
/// after it.
///
/// The compact form carries the byte count in the low two bits of the
/// first byte. A count of zero holds the value in the remaining six bits.
/// A count of one holds a two byte value. A count of two holds a four byte
/// value. A count of three names the byte count in the remaining six bits.
pub fn read_compact(bytes: &[u8], at: usize) -> Result<(u64, usize), BridgeFault> {
    let first = *bytes.get(at).ok_or(BridgeFault::MalformedEncoding)?;
    match first & 0b11 {
        0 => Ok(((first >> 2) as u64, at.saturating_add(1))),
        1 => {
            let second = *bytes
                .get(at.saturating_add(1))
                .ok_or(BridgeFault::MalformedEncoding)?;
            let held = u16::from_le_bytes([first, second]);
            Ok(((held >> 2) as u64, at.saturating_add(2)))
        }
        2 => {
            let end = at.checked_add(4).ok_or(BridgeFault::MalformedEncoding)?;
            let held = bytes.get(at..end).ok_or(BridgeFault::MalformedEncoding)?;
            let value = u32::from_le_bytes([held[0], held[1], held[2], held[3]]);
            Ok(((value >> 2) as u64, end))
        }
        _ => {
            let count = ((first >> 2) as usize).saturating_add(4);
            if count > 8 {
                return Err(BridgeFault::MalformedEncoding);
            }
            let start = at.saturating_add(1);
            let end = start
                .checked_add(count)
                .ok_or(BridgeFault::MalformedEncoding)?;
            let held = bytes
                .get(start..end)
                .ok_or(BridgeFault::MalformedEncoding)?;
            let mut value = 0u64;
            for (position, byte) in held.iter().enumerate() {
                value |= (*byte as u64) << (position * 8);
            }
            Ok((value, end))
        }
    }
}

/// The bytes one authority signs for one settled block.
pub fn settlement_payload(block_hash: &[u8; 32], number: u32, round: u64, set_id: u64) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(53);
    found.push(VOTE_MARKER);
    found.extend_from_slice(block_hash);
    found.extend_from_slice(&number.to_le_bytes());
    found.extend_from_slice(&round.to_le_bytes());
    found.extend_from_slice(&set_id.to_le_bytes());
    found
}

/// The proof one message from a Polkadot network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct PolkadotProof {
    /// The encoded header of the block the message sits in.
    pub header: RawHeader,
    /// The round the settlement covered.
    pub round: u64,
    /// The identifier of the authority set that signed.
    pub set_id: u64,
    /// The signatures of the authority set over the settlement payload.
    pub signatures: BoundedVec<IndexedSignature, ConstU32<MAX_SIGNATURES>>,
    /// The trie nodes from the state root to the storage key.
    pub nodes: ProofNodes,
    /// The value the storage key holds.
    pub value: StoreValue,
}

/// The verifier of a Polkadot network.
pub struct PolkadotVerifier;

impl Verifier for PolkadotVerifier {
    type Proof = PolkadotProof;

    fn kind() -> NetworkKind {
        NetworkKind::Polkadot
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &PolkadotProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Polkadot(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        if proof.set_id != params.set_id {
            return Err(BridgeFault::SignerUnknown);
        }
        let header = read_header(proof.header.as_slice())?;
        if params.checkpoint.height > 0 && (header.number as u64) <= params.checkpoint.height {
            return Err(BridgeFault::ChainBroken);
        }

        let signed = check_settlement(&params.authorities, &header, proof)?;
        let total = total_weight(&params.authorities);
        if !reaches_two_thirds(signed, total) {
            return Err(BridgeFault::ThresholdUnmet);
        }

        let id = envelope.id();
        let key = storage_key_for(params, &id);
        check_storage(&header.state_root, &key, proof)?;

        let commitment = envelope.commitment();
        if !proof
            .value
            .as_slice()
            .windows(32)
            .any(|window| window == commitment.as_slice())
        {
            return Err(BridgeFault::CommitmentMismatch);
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id,
            height: header.number as u64,
            block_hash: header.hash,
            trust: TrustAssumption::ValidatorThreshold { signed, total },
        })
    }
}

/// The storage key one message sits under on the far network.
///
/// The endpoint holds the prefix of the map. The key of one entry is that
/// prefix followed by the hash of the message identifier and the
/// identifier itself, which is the layout a map with a concatenating
/// hasher uses.
pub fn storage_key_for(params: &PolkadotParameters, id: &[u8; 32]) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(params.storage_key.len() + 48);
    found.extend_from_slice(params.storage_key.as_slice());
    found.extend_from_slice(&sp_crypto_hashing::blake2_128(id));
    found.extend_from_slice(id);
    found
}

/// Add the weight of every authority that signed the settlement payload.
pub fn check_settlement(
    authorities: &AuthoritySet,
    header: &PolkadotHeader,
    proof: &PolkadotProof,
) -> Result<u64, BridgeFault> {
    let payload = settlement_payload(&header.hash, header.number, proof.round, proof.set_id);
    let mut seen: Vec<u32> = Vec::with_capacity(proof.signatures.len());
    let mut signed = 0u64;
    for held in proof.signatures.iter() {
        let member = authorities
            .get(held.signer as usize)
            .ok_or(BridgeFault::SignerUnknown)?;
        if seen.contains(&held.signer) {
            return Err(BridgeFault::SignerRepeated);
        }
        seen.push(held.signer);
        let passed = match member.scheme {
            SignatureScheme::Ed25519 => check_ed25519(&member.key, &payload, &held.signature),
            SignatureScheme::Sr25519 => check_sr25519(&member.key, &payload, &held.signature),
        };
        if !passed {
            return Err(BridgeFault::SignatureInvalid);
        }
        signed = signed.saturating_add(member.weight);
    }
    Ok(signed)
}

/// Check one storage proof against one state trie root.
///
/// The proof is the set of trie nodes a far network node answers to a
/// storage read proof request. The check places the nodes in a store keyed
/// by their hash and reads the key from the state root through the trie
/// crate of the pinned set. A read that reaches a node missing from the
/// set, or a value other than the one the proof names, declines. The walk
/// matches the far network node byte for byte.
pub fn check_storage(
    state_root: &[u8; 32],
    key: &[u8],
    proof: &PolkadotProof,
) -> Result<(), BridgeFault> {
    let store = sp_trie::StorageProof::new(raw_nodes(&proof.nodes)).into_memory_db::<BlakeTwo256>();
    let root = sp_core::H256::from(*state_root);
    let found =
        sp_trie::read_trie_value::<LayoutV1<BlakeTwo256>, _>(&store, &root, key, None, None)
            .map_err(|_| BridgeFault::NodeHashMismatch)?;
    match found {
        Some(held) if held.as_slice() == proof.value.as_slice() => Ok(()),
        _ => Err(BridgeFault::NodeHashMismatch),
    }
}

/// Record one accepted Polkadot header.
pub fn record_of(
    header: &PolkadotHeader,
    now_millis: u64,
    signed: u64,
    total: u64,
) -> HeaderRecord {
    HeaderRecord {
        height: header.number as u64,
        hash: header.hash,
        state_root: header.state_root,
        extra_root: header.extrinsics_root,
        at_millis: now_millis,
        trust: TrustAssumption::ValidatorThreshold { signed, total },
    }
}

/// Build the encoding of one header from its fields.
///
/// The builder writes the parent hash, the block number in the compact
/// form, the state root, the extrinsics root and an empty digest. The
/// bridge uses the builder while it produces fixtures.
pub fn encode_header(
    parent: &[u8; 32],
    number: u32,
    state_root: &[u8; 32],
    extrinsics_root: &[u8; 32],
) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(100);
    found.extend_from_slice(parent);
    found.extend_from_slice(&codec::Compact(number).encode());
    found.extend_from_slice(state_root);
    found.extend_from_slice(extrinsics_root);
    found.push(0u8);
    found
}
