//! The message envelope every bridge message travels in.
//!
//! One envelope carries a message across one endpoint. The envelope names
//! the network the message came from, the endpoint the message lands on,
//! the position of the message in the stream of that sender, the payload
//! bytes, the transaction class the message runs under and the moment the
//! message stops being acceptable.
//!
//! The envelope stays the same shape for every target network. A target
//! network changes the proof beside the envelope. A target network leaves
//! the envelope alone.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};
use scale_info::TypeInfo;

/// The largest payload one envelope carries, in bytes.
pub const MAX_PAYLOAD_BYTES: u32 = 4_096;

/// The largest sender address one envelope names, in bytes.
///
/// A twenty byte address, a thirty two byte account and a Bitcoin script
/// hash all sit inside this bound.
pub const MAX_ADDRESS_BYTES: u32 = 64;

/// The payload bytes of one envelope.
pub type Payload = BoundedVec<u8, ConstU32<MAX_PAYLOAD_BYTES>>;

/// The address of one sender or one receiver on a target network.
pub type NetworkAddress = BoundedVec<u8, ConstU32<MAX_ADDRESS_BYTES>>;

/// The identity of one message inside one endpoint.
///
/// The replay registry keys on the endpoint and this identifier together.
/// A message therefore lands once on one endpoint and is declined on every
/// later attempt.
pub type MessageId = [u8; 32];

/// The label mixed into the identifier of one message.
pub const MESSAGE_DOMAIN: &[u8] = b"cerulea.bridge.message.v1";

/// The label mixed into the commitment of one message.
pub const COMMITMENT_DOMAIN: &[u8] = b"cerulea.bridge.commitment.v1";

/// The kind of network sitting on the far side of one endpoint.
///
/// The kind picks the verifier. Each kind carries its own proof format and
/// its own trust assumption. The module holds one verifier per kind and
/// declines a message whose proof fails.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum NetworkKind {
    /// A network running Ethereum Virtual Machine bytecode. The proof is a
    /// Merkle Patricia trie proof against a block header.
    #[default]
    EthereumVirtualMachine,
    /// A Cosmos network. The proof is a signed commit of the validator set
    /// beside a store proof against the application hash.
    Cosmos,
    /// A Solana network. The proof is an attested slot header beside an
    /// account proof against the bank hash.
    Solana,
    /// A Polkadot network. The proof is a finality justification of the
    /// registered authority set beside a state trie proof.
    Polkadot,
    /// An Avalanche network. The proof is a threshold of validator
    /// signatures over the message hash.
    Avalanche,
    /// The Bitcoin network. The proof is a header chain carrying the
    /// required work beside a transaction merkle branch.
    Bitcoin,
    /// A Layer 2 rollup. The proof is an output root on the settlement
    /// layer beside a storage proof inside the rollup state.
    Rollup,
}

impl NetworkKind {
    /// A word naming the kind for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            NetworkKind::EthereumVirtualMachine => "ethereum virtual machine",
            NetworkKind::Cosmos => "cosmos",
            NetworkKind::Solana => "solana",
            NetworkKind::Polkadot => "polkadot",
            NetworkKind::Avalanche => "avalanche",
            NetworkKind::Bitcoin => "bitcoin",
            NetworkKind::Rollup => "rollup",
        }
    }

    /// Every kind the module carries, in declaration order.
    pub fn every() -> [NetworkKind; 7] {
        [
            NetworkKind::EthereumVirtualMachine,
            NetworkKind::Cosmos,
            NetworkKind::Solana,
            NetworkKind::Polkadot,
            NetworkKind::Avalanche,
            NetworkKind::Bitcoin,
            NetworkKind::Rollup,
        ]
    }
}

/// How a reader on either side reads the payload bytes.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum MessageFormat {
    /// The payload holds the canonical binary encoding of a Cerulea value.
    /// The logic module reads the value and binds it to the parameters of
    /// the action the message targets.
    #[default]
    CanonicalValue,
    /// The payload holds bytes the far network wrote as they stand. The
    /// action receives the bytes.
    RawBytes,
    /// The payload holds thirty two byte words, as a contract on a network
    /// running Ethereum Virtual Machine bytecode writes them.
    WordAligned,
}

impl MessageFormat {
    /// A word naming the format for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            MessageFormat::CanonicalValue => "canonical value",
            MessageFormat::RawBytes => "raw bytes",
            MessageFormat::WordAligned => "word aligned",
        }
    }

    /// Report whether the payload length suits the format.
    pub fn accepts(&self, length: usize) -> bool {
        match self {
            MessageFormat::CanonicalValue => true,
            MessageFormat::RawBytes => true,
            MessageFormat::WordAligned => length % 32 == 0,
        }
    }
}

/// The direction one message travels through one endpoint.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum Travel {
    /// The message arrives from the far network.
    #[default]
    Inbound,
    /// The message leaves for the far network.
    Outbound,
}

impl Travel {
    /// A word naming the direction for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            Travel::Inbound => "inbound",
            Travel::Outbound => "outbound",
        }
    }
}

/// One message as the Blockchain holds it.
///
/// The envelope travels beside a proof. The proof convinces this chain
/// that the far network committed the message. The envelope states what
/// the message is.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct MessageEnvelope {
    /// The deployment the message belongs to.
    pub deployment: DeploymentId,
    /// The endpoint the message travels through.
    pub endpoint: BoundedName,
    /// The name the operator gave the network the message came from.
    pub source: BoundedName,
    /// The name the operator gave the network the message goes to.
    pub destination: BoundedName,
    /// The address on the far network that sent or receives the message.
    pub party: NetworkAddress,
    /// The position of the message in the stream of the sender.
    ///
    /// A sender counts from zero and adds one for every message. The nonce
    /// keeps two messages with identical payloads apart.
    pub nonce: u64,
    /// The payload bytes.
    pub payload: Payload,
    /// The transaction class the message runs under.
    pub class: WorkClass,
    /// The moment the message stops being acceptable, in milliseconds
    /// since the Unix epoch.
    pub expiry_millis: u64,
}

impl MessageEnvelope {
    /// The identifier of the message.
    ///
    /// The identifier covers the deployment, the endpoint, the two network
    /// names, the sending party, the nonce and the payload. Two messages
    /// differing in any of those fields therefore take separate slots in
    /// the replay registry.
    pub fn id(&self) -> MessageId {
        let mut held: Vec<u8> = Vec::new();
        held.extend_from_slice(MESSAGE_DOMAIN);
        held.extend_from_slice(&self.deployment);
        push_field(&mut held, self.endpoint.as_slice());
        push_field(&mut held, self.source.as_slice());
        push_field(&mut held, self.destination.as_slice());
        push_field(&mut held, self.party.as_slice());
        held.extend_from_slice(&self.nonce.to_be_bytes());
        push_field(&mut held, self.payload.as_slice());
        sp_crypto_hashing::blake2_256(&held)
    }

    /// The bytes the far network commits to for this message.
    ///
    /// A contract, a store entry or a transaction output on the far
    /// network carries this digest. The verifier of the target network
    /// compares the committed bytes with this digest. The digest therefore
    /// binds the proof to the envelope.
    pub fn commitment(&self) -> [u8; 32] {
        let id = self.id();
        let mut held: Vec<u8> = Vec::with_capacity(COMMITMENT_DOMAIN.len() + 41);
        held.extend_from_slice(COMMITMENT_DOMAIN);
        held.extend_from_slice(&id);
        held.push(self.class as u8);
        held.extend_from_slice(&self.expiry_millis.to_be_bytes());
        sp_crypto_hashing::blake2_256(&held)
    }

    /// Report whether the message is still acceptable at the given moment.
    pub fn is_live(&self, now_millis: u64) -> bool {
        now_millis < self.expiry_millis
    }
}

/// Append one length prefixed field to a digest preimage.
fn push_field(into: &mut Vec<u8>, bytes: &[u8]) {
    let length = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
    into.extend_from_slice(&length.to_be_bytes());
    into.extend_from_slice(bytes);
}

/// One message the verifier accepted.
///
/// The verifier answers with this shape. The execution path reads it and
/// runs the action the message targets.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct VerifiedMessage {
    /// The envelope the proof covered.
    pub envelope: MessageEnvelope,
    /// The identifier the replay registry keys on.
    pub id: MessageId,
    /// The height, slot or block number of the far network the proof
    /// reached.
    pub height: u64,
    /// The hash of the far network block the proof reached.
    pub block_hash: [u8; 32],
    /// The trust the acceptance rests on.
    pub trust: TrustAssumption,
}

/// What an accepted proof rests on.
///
/// Each target network offers a different strength of evidence. The record
/// states the strength in plain terms, so an operator reading the endpoint
/// sees what the acceptance means.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum TrustAssumption {
    /// The proof rests on the cryptography of the far network alone. The
    /// chain walked the far merkle structure to the committed root.
    #[default]
    NetworkCryptography,
    /// The proof rests on a share of the far validator set signing. The
    /// numbers give the signing weight and the total weight.
    ValidatorThreshold {
        /// The weight that signed.
        signed: u64,
        /// The weight of the whole set.
        total: u64,
    },
    /// The proof rests on a count of registered relayers signing the far
    /// header. The numbers give the signing count and the registered count.
    RelayerAttestation {
        /// The number of registered relayers that signed.
        signed: u32,
        /// The number of registered relayers.
        registered: u32,
    },
    /// The proof rests on the accumulated work of a header chain. The
    /// number gives the confirmations counted above the block.
    AccumulatedWork {
        /// The confirmations counted above the block the message sits in.
        confirmations: u32,
    },
    /// The proof rests on a settlement layer commitment that passed its
    /// challenge window. The number gives the window in milliseconds.
    SettledAfterChallenge {
        /// The challenge window the commitment passed, in milliseconds.
        window_millis: u64,
    },
}

impl TrustAssumption {
    /// One plain sentence stating what the acceptance rests on.
    pub fn sentence(&self) -> &'static str {
        match self {
            TrustAssumption::NetworkCryptography => {
                "The chain walked the far network merkle structure to the committed root."
            }
            TrustAssumption::ValidatorThreshold { .. } => {
                "A share of the far validator set signed the block the message sits in."
            }
            TrustAssumption::RelayerAttestation { .. } => {
                "A threshold of registered relayers signed the far network header."
            }
            TrustAssumption::AccumulatedWork { .. } => {
                "The far header chain carries the work the endpoint requires."
            }
            TrustAssumption::SettledAfterChallenge { .. } => {
                "The settlement layer commitment passed its challenge window."
            }
        }
    }
}

/// What went wrong while a proof was checked.
///
/// Every fault names the fix. The dispatch path turns a fault into an
/// error a person reads.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum BridgeFault {
    /// The proof names a network kind the endpoint lacks.
    KindMismatch,
    /// The endpoint holds no trusted checkpoint yet.
    CheckpointAbsent,
    /// The header chain fails to reach the trusted checkpoint.
    ChainBroken,
    /// A header carries less work than the endpoint requires.
    WorkTooLow,
    /// The block sits below the confirmation depth the endpoint requires.
    TooFewConfirmations,
    /// A signature failed to verify against the key that claimed it.
    SignatureInvalid,
    /// The signing weight sits below the threshold the endpoint requires.
    ThresholdUnmet,
    /// A signer sits outside the registered set.
    SignerUnknown,
    /// A signer appears twice in one proof.
    SignerRepeated,
    /// The encoded bytes failed to decode into the shape the format names.
    MalformedEncoding,
    /// The merkle walk reached a node that fails its own hash.
    NodeHashMismatch,
    /// The merkle walk reached the end of the path with no value.
    ValueAbsent,
    /// The proved value differs from the commitment the envelope carries.
    CommitmentMismatch,
    /// The proof reaches a root the accepted header set lacks.
    RootUnknown,
    /// The rollup output root sits inside its challenge window.
    ChallengeWindowOpen,
    /// The proof carries more elements than the module accepts.
    ProofTooLarge,
}

impl BridgeFault {
    /// One plain sentence naming the fix.
    pub fn fix(&self) -> &'static str {
        match self {
            BridgeFault::KindMismatch => "Send the proof shape the endpoint network kind names.",
            BridgeFault::CheckpointAbsent => {
                "Set a trusted checkpoint on the endpoint before sending a proof."
            }
            BridgeFault::ChainBroken => {
                "Send every header between the trusted checkpoint and the block."
            }
            BridgeFault::WorkTooLow => "Send headers from the network the endpoint names.",
            BridgeFault::TooFewConfirmations => {
                "Wait for the confirmation depth the endpoint requires and send the proof again."
            }
            BridgeFault::SignatureInvalid => {
                "Send the signature the named key produced over these bytes."
            }
            BridgeFault::ThresholdUnmet => {
                "Gather signatures reaching the threshold the endpoint requires."
            }
            BridgeFault::SignerUnknown => {
                "Register the signer on the endpoint before sending the proof."
            }
            BridgeFault::SignerRepeated => "Send one signature per signer.",
            BridgeFault::MalformedEncoding => {
                "Send the encoding the far network produced for this message."
            }
            BridgeFault::NodeHashMismatch => {
                "Send the merkle nodes the far network holds under this root."
            }
            BridgeFault::ValueAbsent => "Send the merkle path that reaches the value.",
            BridgeFault::CommitmentMismatch => "Send the envelope the far network committed to.",
            BridgeFault::RootUnknown => "Submit the far network header before sending the proof.",
            BridgeFault::ChallengeWindowOpen => {
                "Wait for the challenge window on the endpoint to pass and send the proof again."
            }
            BridgeFault::ProofTooLarge => {
                "Split the work into proofs sitting inside the element limits."
            }
        }
    }

    /// A word naming the fault for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            BridgeFault::KindMismatch => "kind mismatch",
            BridgeFault::CheckpointAbsent => "checkpoint absent",
            BridgeFault::ChainBroken => "chain broken",
            BridgeFault::WorkTooLow => "work too low",
            BridgeFault::TooFewConfirmations => "too few confirmations",
            BridgeFault::SignatureInvalid => "signature invalid",
            BridgeFault::ThresholdUnmet => "threshold unmet",
            BridgeFault::SignerUnknown => "signer unknown",
            BridgeFault::SignerRepeated => "signer repeated",
            BridgeFault::MalformedEncoding => "malformed encoding",
            BridgeFault::NodeHashMismatch => "node hash mismatch",
            BridgeFault::ValueAbsent => "value absent",
            BridgeFault::CommitmentMismatch => "commitment mismatch",
            BridgeFault::RootUnknown => "root unknown",
            BridgeFault::ChallengeWindowOpen => "challenge window open",
            BridgeFault::ProofTooLarge => "proof too large",
        }
    }
}
