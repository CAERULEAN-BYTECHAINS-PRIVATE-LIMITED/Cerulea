//! The verifier for an Avalanche network.
//!
//! An Avalanche network carries a message between its chains as a signed
//! statement. The validator set of the sending chain signs the message
//! bytes. A receiver accepts the message once the signing weight passes
//! the share the receiver requires.
//!
//! # The signature scheme, stated plainly
//!
//! The far network aggregates signatures on a pairing friendly curve. The
//! pinned dependency set of this Blockchain carries no library for that
//! curve. The endpoint therefore collects one Edwards curve signature per
//! validator over the same message bytes, and adds the weight of each
//! signer. The threshold rule stays the same. The message bytes stay the
//! same. The cost is size rather than strength, since the proof carries
//! one signature per signer instead of one aggregate.
//!
//! The endpoint record states this choice. A reader of the endpoint sees
//! the validator count and the share required, so the strength of the
//! evidence is visible.
//!
//! # The message bytes
//!
//! The signed bytes carry the network identifier as four bytes with the
//! high byte first, the identifier of the sending chain as thirty two
//! bytes, the payload length as four bytes with the high byte first, and
//! the payload. The payload carries the sending address and the commitment
//! the envelope holds. The hash of those bytes is what each validator
//! signs.

use crate::endpoint::{
    reaches_percent, total_power, AvalancheParameters, EndpointState, ValidatorSet,
    VerifierParameters,
};
use crate::envelope::{
    BridgeFault, MessageEnvelope, NetworkKind, TrustAssumption, VerifiedMessage,
};
use crate::verify::{check_ed25519, IndexedSignature, Verifier, MAX_SIGNATURES};
use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::{pallet_prelude::ConstU32, BoundedVec};
use scale_info::TypeInfo;

/// The share of the total weight an endpoint requires by default.
pub const DEFAULT_THRESHOLD_PERCENT: u8 = 67;

/// Build the unsigned message bytes.
pub fn unsigned_message(
    network_id: u32,
    source_chain: &[u8; 32],
    sender: &[u8],
    commitment: &[u8; 32],
) -> Vec<u8> {
    let mut payload: Vec<u8> = Vec::with_capacity(sender.len() + 36);
    payload.extend_from_slice(&(sender.len() as u32).to_be_bytes());
    payload.extend_from_slice(sender);
    payload.extend_from_slice(commitment);

    let mut found: Vec<u8> = Vec::with_capacity(payload.len() + 40);
    found.extend_from_slice(&network_id.to_be_bytes());
    found.extend_from_slice(source_chain);
    found.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    found.extend_from_slice(&payload);
    found
}

/// The hash each validator signs over.
pub fn message_hash(
    network_id: u32,
    source_chain: &[u8; 32],
    sender: &[u8],
    commitment: &[u8; 32],
) -> [u8; 32] {
    sp_crypto_hashing::sha2_256(&unsigned_message(
        network_id,
        source_chain,
        sender,
        commitment,
    ))
}

/// The proof one message from an Avalanche network carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub struct AvalancheProof {
    /// The identifier of the network the message crossed.
    pub network_id: u32,
    /// The identifier of the chain the message left.
    pub source_chain: [u8; 32],
    /// The height the sending chain stood at.
    pub height: u64,
    /// The signatures of the validator set over the message hash.
    pub signatures: BoundedVec<IndexedSignature, ConstU32<MAX_SIGNATURES>>,
}

/// The verifier of an Avalanche network.
pub struct AvalancheVerifier;

impl Verifier for AvalancheVerifier {
    type Proof = AvalancheProof;

    fn kind() -> NetworkKind {
        NetworkKind::Avalanche
    }

    fn verify(
        envelope: &MessageEnvelope,
        proof: &AvalancheProof,
        endpoint: &EndpointState<'_>,
    ) -> Result<VerifiedMessage, BridgeFault> {
        let params = match endpoint.params {
            VerifierParameters::Avalanche(held) => held,
            _ => return Err(BridgeFault::KindMismatch),
        };
        if proof.source_chain != params.chain_id {
            return Err(BridgeFault::CommitmentMismatch);
        }
        let commitment = envelope.commitment();
        let hash = message_hash(
            proof.network_id,
            &proof.source_chain,
            params.sender.as_slice(),
            &commitment,
        );
        let signed = count_weight(&params.validators, &hash, &proof.signatures)?;
        let total = total_power(&params.validators);
        let share = threshold_of(params);
        if !reaches_percent(signed, total, share) {
            return Err(BridgeFault::ThresholdUnmet);
        }

        Ok(VerifiedMessage {
            envelope: envelope.clone(),
            id: envelope.id(),
            height: proof.height,
            block_hash: hash,
            trust: TrustAssumption::ValidatorThreshold { signed, total },
        })
    }
}

/// The share of the total weight the endpoint requires.
pub fn threshold_of(params: &AvalancheParameters) -> u8 {
    if params.threshold_percent == 0 {
        DEFAULT_THRESHOLD_PERCENT
    } else {
        params.threshold_percent
    }
}

/// Add the weight of every validator that signed the message hash.
pub fn count_weight(
    validators: &ValidatorSet,
    hash: &[u8; 32],
    signatures: &[IndexedSignature],
) -> Result<u64, BridgeFault> {
    let mut seen: Vec<u32> = Vec::with_capacity(signatures.len());
    let mut signed = 0u64;
    for held in signatures.iter() {
        let member = validators
            .get(held.signer as usize)
            .ok_or(BridgeFault::SignerUnknown)?;
        if seen.contains(&held.signer) {
            return Err(BridgeFault::SignerRepeated);
        }
        seen.push(held.signer);
        if !check_ed25519(&member.key, hash, &held.signature) {
            return Err(BridgeFault::SignatureInvalid);
        }
        signed = signed.saturating_add(member.power);
    }
    Ok(signed)
}
