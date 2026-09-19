//! The verifier for a Cosmos network.
//!
//! A Cosmos network reaches agreement through a weighted validator set.
//! Each member signs a vote over the block. A block is settled once the
//! signing weight passes two thirds of the total weight. The verifier here
//! rebuilds those vote bytes, checks every signature and adds the weight.
//!
//! # The chain of custody
//!
//! 1. **The commit.** The validator set signs a vote naming the chain, the
//!    height, the round, the block hash and the moment. The verifier
//!    rebuilds the vote bytes for each signer and checks the signature.
//!    The signing weight passes two thirds plus one of the total weight.
//! 2. **The application hash.** The block hash is the root of a tree over
//!    the header fields. The proof carries the branch from the application
//!    hash field to the block hash, so the commit covers the application
//!    hash.
//! 3. **The store root.** The application hash is the root of a tree over
//!    the stores. The outer existence proof carries the branch from the
//!    named store to the application hash.
//! 4. **The message.** The inner existence proof carries the branch from
//!    the message key to the store root. The value equals the commitment
//!    the envelope carries.
//!
//! # The vote bytes
//!
//! The vote sits in the length delimited binary form the far network
//! marshals. The form writes one key byte per field, holding the field
//! number and the wire kind. A scalar field holding zero is left out, as
//! the form requires. The whole message then takes a length prefix. The
//! verifier writes those bytes here, so a signature made by a far network
//! node verifies without translation.
//!
//! # The existence proof
//!
//! An existence proof holds a leaf step and a list of inner steps. The
//! leaf step hashes a prefix, the length prefixed key and the length
//! prefixed hash of the value. Each inner step hashes a prefix, the hash
//! carried up and a suffix. The last hash equals the root. The two stock
//! specs differ in the sizes their inner steps allow, and the verifier
//! checks those sizes.

use crate::endpoint::{
    reaches_two_thirds, total_power, EndpointState, HeaderRecord, ValidatorSet, VerifierParameters,
};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::merkle;
use crate::verify::{
    check_ed25519, Branch, IndexedSignature, StoreKey, StoreValue, Verifier, MAX_BRANCH,
    MAX_SIGNATURES, MAX_STEP_BYTES,
};
use alloc::{vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The vote kind a settling vote carries.
pub const VOTE_KIND_PRECOMMIT: u64 = 2;

/// Write one whole number in the variable length form.
///
/// The form writes seven bits per byte, the low bits first. A byte with
/// its top bit set says another byte follows.
pub fn varint(mut value: u64) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(10);
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            found.push(byte);
            return found;
        }
        found.push(byte | 0x80);
    }
}

/// Write one signed whole number in the variable length form.
///
/// A negative value sits in the form as its two complement read as an
/// unsigned number.
pub fn varint_signed(value: i64) -> Vec<u8> {
    varint(value as u64)
}

/// Write one field key from the field number and the wire kind.
fn key(field: u64, wire: u64) -> Vec<u8> {
    varint((field << 3) | wire)
}

/// Write one length delimited field.
fn field_bytes(field: u64, bytes: &[u8]) -> Vec<u8> {
    let mut found = key(field, 2);
    found.extend_from_slice(&varint(bytes.len() as u64));
    found.extend_from_slice(bytes);
    found
}

/// Write one variable length number field, leaving out a zero value.
fn field_varint(field: u64, value: u64) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }
    let mut found = key(field, 0);
    found.extend_from_slice(&varint(value));
    found
}

/// Write one signed variable length number field, leaving out a zero.
fn field_varint_signed(field: u64, value: i64) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }
    let mut found = key(field, 0);
    found.extend_from_slice(&varint_signed(value));
    found
}

/// Write one eight byte fixed field, leaving out a zero value.
fn field_fixed64(field: u64, value: i64) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }
    let mut found = key(field, 1);
    found.extend_from_slice(&value.to_le_bytes());
    found
}

/// The part header one block carries.
///
/// The far network splits a block into parts for gossip. The header names
/// the count of parts and the root over them. The vote covers both.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo,
)]
pub struct PartHeader {
    /// The count of parts the block holds.
    pub total: u32,
    /// The root over the parts.
    pub hash: [u8; 32],
}

/// Write the part header field.
fn part_header_bytes(held: &PartHeader) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::new();
    found.extend_from_slice(&field_varint(1, held.total as u64));
    found.extend_from_slice(&field_bytes(2, &held.hash));
    found
}

/// Write the block identity field.
fn block_id_bytes(hash: &[u8; 32], parts: &PartHeader) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::new();
    found.extend_from_slice(&field_bytes(1, hash));
    found.extend_from_slice(&field_bytes(2, &part_header_bytes(parts)));
    found
}

/// Write the moment field.
fn moment_bytes(seconds: i64, nanos: u32) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::new();
    found.extend_from_slice(&field_varint_signed(1, seconds));
    found.extend_from_slice(&field_varint(2, nanos as u64));
    found
}

/// The bytes one validator signs for one block.
///
/// The bytes carry the vote kind, the height, the round, the block
/// identity, the moment and the chain name. The whole message takes a
/// length prefix, so a signer commits to the length as well as the
/// content.
pub fn vote_payload(
    chain_id: &[u8],
    height: u64,
    round: u32,
    block_hash: &[u8; 32],
    parts: &PartHeader,
    seconds: i64,
    nanos: u32,
) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::new();
    body.extend_from_slice(&field_varint(1, VOTE_KIND_PRECOMMIT));
    body.extend_from_slice(&field_fixed64(2, height as i64));
    body.extend_from_slice(&field_fixed64(3, round as i64));
    body.extend_from_slice(&field_bytes(4, &block_id_bytes(block_hash, parts)));
    body.extend_from_slice(&field_bytes(5, &moment_bytes(seconds, nanos)));
    body.extend_from_slice(&field_bytes(6, chain_id));
    let mut found = varint(body.len() as u64);
    found.extend_from_slice(&body);
    found
}

/// The stock proof specs the far network stores answer with.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo,
)]
pub enum ProofSpec {
    /// The spec of a key and value store. An inner step carries a prefix
    /// of four to twelve bytes and a child of thirty three bytes.
    #[default]
    Store,
    /// The spec of the map holding the stores. An inner step carries a
    /// prefix of one byte and a child of thirty two bytes.
    Root,
}

impl ProofSpec {
    /// The smallest prefix an inner step carries, in bytes.
    pub fn min_prefix(&self) -> usize {
        match self {
            ProofSpec::Store => 4,
            ProofSpec::Root => 1,
        }
    }

    /// The largest prefix an inner step carries ahead of the child, in
    /// bytes.
    pub fn max_prefix(&self) -> usize {
        match self {
            ProofSpec::Store => 12,
            ProofSpec::Root => 1,
        }
    }

    /// The size of one child inside an inner step, in bytes.
    pub fn child_size(&self) -> usize {
        match self {
            ProofSpec::Store => 33,
            ProofSpec::Root => 32,
        }
    }

    /// The prefix the leaf step carries.
    pub fn leaf_prefix(&self) -> &'static [u8] {
        &[0u8]
    }

    /// The bytes an inner step writes ahead of each child.
    ///
    /// The store spec writes a one byte length marker ahead of each child,
    /// which is why the child size of that spec is thirty three. The root
    /// spec writes the children as they stand.
    pub fn separator(&self) -> &'static [u8] {
        match self {
            ProofSpec::Store => &[0x20u8],
            ProofSpec::Root => &[],
        }
    }
}

/// One inner step of an existence proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct InnerStep {
    /// The bytes ahead of the hash carried up.
    pub prefix: BoundedVec<u8, ConstU32<MAX_STEP_BYTES>>,
    /// The bytes behind the hash carried up.
    pub suffix: BoundedVec<u8, ConstU32<MAX_STEP_BYTES>>,
}

/// One existence proof over a store.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct ExistenceProof {
    /// The key the proof covers.
    pub key: StoreKey,
    /// The value the key holds.
    pub value: StoreValue,
    /// The spec the steps follow.
    pub spec: ProofSpec,
    /// The inner steps from the leaf to the root.
    pub path: BoundedVec<InnerStep, ConstU32<MAX_BRANCH>>,
}

/// The hash of one leaf step.
///
/// The step hashes the prefix, the length prefixed key and the length
/// prefixed hash of the value. Hashing the value first keeps the leaf
/// hash a fixed size for any value.
pub fn leaf_hash(spec: ProofSpec, key: &[u8], value: &[u8]) -> [u8; 32] {
    let value_hash = sp_crypto_hashing::sha2_256(value);
    let mut held: Vec<u8> = Vec::with_capacity(key.len() + 48);
    held.extend_from_slice(spec.leaf_prefix());
    held.extend_from_slice(&varint(key.len() as u64));
    held.extend_from_slice(key);
    held.extend_from_slice(&varint(value_hash.len() as u64));
    held.extend_from_slice(&value_hash);
    sp_crypto_hashing::sha2_256(&held)
}

/// The hash of one inner step.
pub fn inner_hash(prefix: &[u8], child: &[u8; 32], suffix: &[u8]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(prefix.len() + 32 + suffix.len());
    held.extend_from_slice(prefix);
    held.extend_from_slice(child);
    held.extend_from_slice(suffix);
    sp_crypto_hashing::sha2_256(&held)
}

impl ExistenceProof {
    /// The root the proof folds to.
    ///
    /// The fold checks the size of every step against the spec, so a step
    /// carrying a crafted prefix is declined before it reaches the root.
    pub fn fold(&self) -> Result<[u8; 32], BridgeFault> {
        let mut current = leaf_hash(self.spec, self.key.as_slice(), self.value.as_slice());
        for step in self.path.iter() {
            let prefix = step.prefix.as_slice();
            let suffix = step.suffix.as_slice();
            if prefix.len() < self.spec.min_prefix() {
                return Err(BridgeFault::MalformedEncoding);
            }
            if prefix.len()
                > self
                    .spec
                    .max_prefix()
                    .saturating_add(self.spec.child_size())
            {
                return Err(BridgeFault::MalformedEncoding);
            }
            if !suffix.is_empty() && suffix.len() != self.spec.child_size() {
                return Err(BridgeFault::MalformedEncoding);
            }
            if prefix.first() == Some(&0u8) && prefix.len() == 1 {
                return Err(BridgeFault::MalformedEncoding);
            }
            current = inner_hash(prefix, &current, suffix);
        }
        Ok(current)
    }

    /// Check the proof against one root.
    pub fn check(&self, root: &[u8; 32]) -> Result<(), BridgeFault> {
        if &self.fold()? == root {
            Ok(())
        } else {
            Err(BridgeFault::NodeHashMismatch)
        }
    }
}

/// The proof one message from a Cosmos network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct CosmosProof {
    /// The height of the block the message sits in.
    pub height: u64,
    /// The hash of that block.
    pub block_hash: [u8; 32],
    /// The round the vote covered.
    pub round: u32,
    /// The part header the vote covered.
    pub parts: PartHeader,
    /// The moment the vote covered, in whole seconds.
    pub seconds: i64,
    /// The fraction of the moment the vote covered, in nanoseconds.
    pub nanos: u32,
    /// The signatures of the validator set over the vote bytes.
    pub signatures: BoundedVec<IndexedSignature, ConstU32<MAX_SIGNATURES>>,
    /// The application hash the header carries.
    pub app_hash: [u8; 32],
    /// The position of the application hash among the header fields.
    pub app_hash_index: u32,
    /// The count of header fields.
    pub app_hash_total: u32,
    /// The branch from the application hash field to the block hash.
    pub app_hash_branch: Branch,
    /// The proof of the store root inside the application hash.
    pub store_proof: ExistenceProof,
    /// The proof of the message inside the store.
    pub message_proof: ExistenceProof,
}

/// The verifier of a Cosmos network.
pub struct CosmosVerifier;

impl Verifier for CosmosVerifier {
    type Proof = CosmosProof;

    fn kind() -> NetworkKind {
        NetworkKind::Cosmos
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &CosmosProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Cosmos(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        if proof.height <= params.checkpoint.height && params.checkpoint.height > 0 {
            return Err(BridgeFault::ChainBroken);
        }

        let signed = check_commit(&params.validators, params.chain_id.as_slice(), proof)?;
        let total = total_power(&params.validators);
        if !reaches_two_thirds(signed, total) {
            return Err(BridgeFault::ThresholdUnmet);
        }

        check_app_hash(proof)?;

        let store_root = crate::verify::word_from(proof.store_proof.value.as_slice())
            .ok_or(BridgeFault::MalformedEncoding)?;
        if proof.store_proof.key.as_slice() != params.store_prefix.as_slice() {
            return Err(BridgeFault::CommitmentMismatch);
        }
        proof.store_proof.check(&proof.app_hash)?;
        proof.message_proof.check(&store_root)?;

        let id = envelope.id();
        let expected_key = message_key(params.key_prefix.as_slice(), &id);
        if proof.message_proof.key.as_slice() != expected_key.as_slice() {
            return Err(BridgeFault::CommitmentMismatch);
        }
        let commitment = envelope.commitment();
        if proof.message_proof.value.as_slice() != commitment.as_slice() {
            return Err(BridgeFault::CommitmentMismatch);
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id,
            height: proof.height,
            block_hash: proof.block_hash,
            trust: TrustAssumption::ValidatorThreshold { signed, total },
        })
    }
}

/// The store key one message sits under.
pub fn message_key(prefix: &[u8], id: &[u8; 32]) -> Vec<u8> {
    let mut found: Vec<u8> = Vec::with_capacity(prefix.len() + 32);
    found.extend_from_slice(prefix);
    found.extend_from_slice(id);
    found
}

/// Add the weight of every validator that signed the vote.
pub fn check_commit(
    validators: &ValidatorSet,
    chain_id: &[u8],
    proof: &CosmosProof,
) -> Result<u64, BridgeFault> {
    let payload = vote_payload(
        chain_id,
        proof.height,
        proof.round,
        &proof.block_hash,
        &proof.parts,
        proof.seconds,
        proof.nanos,
    );
    let mut seen: Vec<u32> = Vec::with_capacity(proof.signatures.len());
    let mut signed = 0u64;
    for held in proof.signatures.iter() {
        let member = validators
            .get(held.signer as usize)
            .ok_or(BridgeFault::SignerUnknown)?;
        if seen.contains(&held.signer) {
            return Err(BridgeFault::SignerRepeated);
        }
        seen.push(held.signer);
        if !check_ed25519(&member.key, &payload, &held.signature) {
            return Err(BridgeFault::SignatureInvalid);
        }
        signed = signed.saturating_add(member.power);
    }
    Ok(signed)
}

/// Check that the application hash sits under the block hash.
pub fn check_app_hash(proof: &CosmosProof) -> Result<(), BridgeFault> {
    let folded = merkle::prefixed_fold(
        &proof.app_hash,
        proof.app_hash_index as usize,
        proof.app_hash_total as usize,
        proof.app_hash_branch.as_slice(),
    )
    .ok_or(BridgeFault::MalformedEncoding)?;
    if folded == proof.block_hash {
        Ok(())
    } else {
        Err(BridgeFault::NodeHashMismatch)
    }
}

/// Record one accepted Cosmos header.
pub fn record_of(proof: &CosmosProof, now_millis: u64, signed: u64, total: u64) -> HeaderRecord {
    HeaderRecord {
        height: proof.height,
        hash: proof.block_hash,
        state_root: proof.app_hash,
        extra_root: proof.app_hash,
        at_millis: now_millis,
        trust: TrustAssumption::ValidatorThreshold { signed, total },
    }
}

/// Build an existence proof over a list of leaves under the root spec.
///
/// The builder writes the same steps a far network store answers with. The
/// bridge uses the builder while it produces fixtures and while it tests
/// the store path.
pub fn build_root_proof(
    entries: &[(Vec<u8>, Vec<u8>)],
    index: usize,
) -> Option<(ExistenceProof, [u8; 32])> {
    build_proof(entries, index, ProofSpec::Root)
}

/// Build an existence proof over a list of leaves under the store spec.
pub fn build_store_proof(
    entries: &[(Vec<u8>, Vec<u8>)],
    index: usize,
) -> Option<(ExistenceProof, [u8; 32])> {
    build_proof(entries, index, ProofSpec::Store)
}

/// The fixed header bytes an inner step of one spec carries.
fn step_header(spec: ProofSpec) -> Vec<u8> {
    vec![1u8; spec.min_prefix()]
}

/// The hash of one parent under one spec.
///
/// The parent hashes the header bytes, the left child and the right child,
/// each child behind the separator the spec carries. The two step shapes
/// of the proof cut the same bytes at two places, so a fold of either
/// shape reaches this hash.
pub fn parent_hash(spec: ProofSpec, left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut held = step_header(spec);
    held.extend_from_slice(spec.separator());
    held.extend_from_slice(left);
    held.extend_from_slice(spec.separator());
    held.extend_from_slice(right);
    sp_crypto_hashing::sha2_256(&held)
}

/// Build an existence proof over a list of leaves under one spec.
///
/// The leaves sit in a balanced pair tree. A level holding an odd count
/// repeats its last entry. The step of the left child carries the right
/// child in its suffix. The step of the right child carries the left child
/// in its prefix.
fn build_proof(
    entries: &[(Vec<u8>, Vec<u8>)],
    index: usize,
    spec: ProofSpec,
) -> Option<(ExistenceProof, [u8; 32])> {
    if entries.is_empty() || index >= entries.len() {
        return None;
    }
    let mut level: Vec<[u8; 32]> = entries
        .iter()
        .map(|(key, value)| leaf_hash(spec, key, value))
        .collect();
    let mut position = index;
    let mut path: Vec<InnerStep> = Vec::new();
    while level.len() > 1 {
        let mut next: Vec<[u8; 32]> = Vec::with_capacity(level.len().div_ceil(2));
        let mut cursor = 0usize;
        while cursor < level.len() {
            let left = level[cursor];
            let right = if cursor + 1 < level.len() {
                level[cursor + 1]
            } else {
                left
            };
            if cursor == position {
                let mut prefix = step_header(spec);
                prefix.extend_from_slice(spec.separator());
                let mut suffix: Vec<u8> = spec.separator().to_vec();
                suffix.extend_from_slice(&right);
                path.push(InnerStep {
                    prefix: BoundedVec::try_from(prefix).ok()?,
                    suffix: BoundedVec::try_from(suffix).ok()?,
                });
            } else if cursor + 1 == position {
                let mut prefix = step_header(spec);
                prefix.extend_from_slice(spec.separator());
                prefix.extend_from_slice(&left);
                prefix.extend_from_slice(spec.separator());
                path.push(InnerStep {
                    prefix: BoundedVec::try_from(prefix).ok()?,
                    suffix: BoundedVec::try_from(Vec::new()).ok()?,
                });
            }
            next.push(parent_hash(spec, &left, &right));
            cursor += 2;
        }
        position /= 2;
        level = next;
    }
    let proof = ExistenceProof {
        key: BoundedVec::try_from(entries[index].0.clone()).ok()?,
        value: BoundedVec::try_from(entries[index].1.clone()).ok()?,
        spec,
        path: BoundedVec::try_from(path).ok()?,
    };
    let root = proof.fold().ok()?;
    Some((proof, root))
}
