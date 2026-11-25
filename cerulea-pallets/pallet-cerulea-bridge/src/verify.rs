//! The proof interface every target network implements.
//!
//! One trait carries the whole verification surface. A verifier receives
//! the envelope, the proof and the endpoint state. A verifier answers with
//! an accepted message or with a fault naming the fix.
//!
//! The proof shapes live in one enum. The variant names the target
//! network. A proof of one variant against an endpoint of another network
//! kind is declined before any cryptography runs.

use crate::endpoint::{EndpointState, HeaderRecord};
use crate::envelope::{BridgeFault, MessageEnvelope, NetworkKind, VerifiedMessage};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The largest number of trie nodes one proof carries.
pub const MAX_PROOF_NODES: u32 = 64;

/// The largest single trie node one proof carries, in bytes.
pub const MAX_NODE_BYTES: u32 = 2_048;

/// The largest number of signatures one proof carries.
pub const MAX_SIGNATURES: u32 = 128;

/// The largest merkle branch one proof carries.
pub const MAX_BRANCH: u32 = 64;

/// The largest far network header one submission carries, in bytes.
pub const MAX_HEADER_BYTES: u32 = 2_048;

/// The largest number of headers one submission carries.
pub const MAX_HEADERS: u32 = 32;

/// The largest raw transaction one proof carries, in bytes.
pub const MAX_RAW_BYTES: u32 = 8_192;

/// The largest store key one proof carries, in bytes.
pub const MAX_KEY_BYTES: u32 = 256;

/// The largest store value one proof carries, in bytes.
pub const MAX_VALUE_BYTES: u32 = 4_096;

/// The largest prefix or suffix one merkle step carries, in bytes.
pub const MAX_STEP_BYTES: u32 = 128;

/// One encoded trie node.
pub type ProofNode = BoundedVec<u8, ConstU32<MAX_NODE_BYTES>>;

/// The trie nodes one proof carries.
pub type ProofNodes = BoundedVec<ProofNode, ConstU32<MAX_PROOF_NODES>>;

/// A merkle branch of thirty two byte hashes.
pub type Branch = BoundedVec<[u8; 32], ConstU32<MAX_BRANCH>>;

/// One encoded far network header.
pub type RawHeader = BoundedVec<u8, ConstU32<MAX_HEADER_BYTES>>;

/// One raw far network transaction.
pub type RawTransaction = BoundedVec<u8, ConstU32<MAX_RAW_BYTES>>;

/// One store key on a far network.
pub type StoreKey = BoundedVec<u8, ConstU32<MAX_KEY_BYTES>>;

/// One store value on a far network.
pub type StoreValue = BoundedVec<u8, ConstU32<MAX_VALUE_BYTES>>;

/// Read the trie nodes of a proof back as plain byte vectors.
pub fn raw_nodes(nodes: &ProofNodes) -> Vec<Vec<u8>> {
    nodes.iter().map(|held| held.to_vec()).collect()
}

/// One signature over a far network commit, naming the signer by position.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct IndexedSignature {
    /// The position of the signer in the registered set.
    pub signer: u32,
    /// The sixty four byte signature.
    pub signature: [u8; 64],
}

/// One signature over a far network header, naming the signer by key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct KeyedSignature {
    /// The public key of the signer.
    pub key: [u8; 32],
    /// The sixty four byte signature.
    pub signature: [u8; 64],
}

/// The proof one message carries.
///
/// Each variant holds the shape the far network produces. The module holds
/// one verifier per variant.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum Proof {
    /// A proof from a network running Ethereum Virtual Machine bytecode.
    EthereumVirtualMachine(crate::networks::evm::EvmProof),
    /// A proof from a Cosmos network.
    Cosmos(crate::networks::cosmos::CosmosProof),
    /// A proof from a Solana network.
    Solana(crate::networks::solana::SolanaProof),
    /// A proof from a Polkadot network.
    Polkadot(crate::networks::polkadot::PolkadotProof),
    /// A proof from an Avalanche network.
    Avalanche(crate::networks::avalanche::AvalancheProof),
    /// A proof from the Bitcoin network.
    Bitcoin(crate::networks::bitcoin::BitcoinProof),
    /// A proof from a Layer 2 rollup.
    Rollup(crate::networks::rollup::RollupProof),
}

impl Proof {
    /// The network kind the proof belongs to.
    pub fn kind(&self) -> NetworkKind {
        match self {
            Proof::EthereumVirtualMachine(_) => NetworkKind::EthereumVirtualMachine,
            Proof::Cosmos(_) => NetworkKind::Cosmos,
            Proof::Solana(_) => NetworkKind::Solana,
            Proof::Polkadot(_) => NetworkKind::Polkadot,
            Proof::Avalanche(_) => NetworkKind::Avalanche,
            Proof::Bitcoin(_) => NetworkKind::Bitcoin,
            Proof::Rollup(_) => NetworkKind::Rollup,
        }
    }
}

/// The check one target network performs over one proof.
///
/// The verifier reads the envelope, the proof and the endpoint state. The
/// verifier answers with the accepted message or with a fault. A verifier
/// never writes storage, so a caller runs a verifier inside a read path as
/// well as inside a dispatch.
pub trait Verifier {
    /// The proof shape the verifier reads.
    type Proof;

    /// The network kind the verifier serves.
    fn kind() -> NetworkKind;

    /// Check one proof against one envelope.
    fn verify(
        envelope: &MessageEnvelope,
        proof: &Self::Proof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault>;
}

/// Check one proof against one envelope, picking the verifier of the
/// network kind the endpoint names.
///
/// A proof of a kind the endpoint lacks is declined here, before any
/// cryptography runs.
pub fn verify(
    envelope: &MessageEnvelope,
    proof: &Proof,
    endpoint: &EndpointState<'_>,
) -> Result<VerifiedMessage, BridgeFault> {
    if proof.kind() != endpoint.kind {
        return Err(BridgeFault::KindMismatch);
    }
    match proof {
        Proof::EthereumVirtualMachine(held) => {
            crate::networks::evm::EvmVerifier::verify(envelope, held, endpoint)
        }
        Proof::Cosmos(held) => {
            crate::networks::cosmos::CosmosVerifier::verify(envelope, held, endpoint)
        }
        Proof::Solana(held) => {
            crate::networks::solana::SolanaVerifier::verify(envelope, held, endpoint)
        }
        Proof::Polkadot(held) => {
            crate::networks::polkadot::PolkadotVerifier::verify(envelope, held, endpoint)
        }
        Proof::Avalanche(held) => {
            crate::networks::avalanche::AvalancheVerifier::verify(envelope, held, endpoint)
        }
        Proof::Bitcoin(held) => {
            crate::networks::bitcoin::BitcoinVerifier::verify(envelope, held, endpoint)
        }
        Proof::Rollup(held) => {
            crate::networks::rollup::RollupVerifier::verify(envelope, held, endpoint)
        }
    }
}

/// The header a proof names, read from the accepted header set.
///
/// A proof naming a height the endpoint lacks is declined with a fault
/// telling the caller to submit the header first.
pub fn accepted_header(
    endpoint: &EndpointState<'_>,
    height: u64,
) -> Result<HeaderRecord, BridgeFault> {
    endpoint
        .headers
        .header(height)
        .ok_or(BridgeFault::RootUnknown)
}

/// Count the confirmations above one height and compare them with the
/// depth the endpoint requires.
pub fn confirmations_for(
    endpoint: &EndpointState<'_>,
    height: u64,
    required: u32,
) -> Result<u32, BridgeFault> {
    let tip = endpoint.headers.tip_height();
    if tip < height {
        return Err(BridgeFault::TooFewConfirmations);
    }
    let depth = tip.saturating_sub(height).saturating_add(1);
    let held = u32::try_from(depth).unwrap_or(u32::MAX);
    if held < required.max(1) {
        return Err(BridgeFault::TooFewConfirmations);
    }
    Ok(held)
}

/// Check one Edwards curve signature.
///
/// The check runs through the host of the Blockchain, so every node
/// performs the same arithmetic on the same curve.
pub fn check_ed25519(key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let public = sp_core::ed25519::Public::from_raw(*key);
    let held = sp_core::ed25519::Signature::from_raw(*signature);
    sp_io::crypto::ed25519_verify(&held, message, &public)
}

/// Check one Ristretto Schnorr signature.
pub fn check_sr25519(key: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> bool {
    let public = sp_core::sr25519::Public::from_raw(*key);
    let held = sp_core::sr25519::Signature::from_raw(*signature);
    sp_io::crypto::sr25519_verify(&held, message, &public)
}

/// Read a thirty two byte value from the front of a byte string.
pub fn word_from(bytes: &[u8]) -> Option<[u8; 32]> {
    let held = bytes.get(..32)?;
    let mut found = [0u8; 32];
    found.copy_from_slice(held);
    Some(found)
}
