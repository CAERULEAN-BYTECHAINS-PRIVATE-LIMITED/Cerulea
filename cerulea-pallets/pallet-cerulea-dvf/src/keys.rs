//! The validator key records of the Admission Engine.
//!
//! Each active validator carries two keys. The draw key is an sr25519 key. The
//! Authoring Engine evaluates the verifiable random function with that key. The
//! seal key is an ed25519 key. The Authoring Engine signs the block seal with
//! that key. The Commit Engine signs every vote with the same seal key.
//!
//! A validator registers both keys itself. The registration extrinsic is signed
//! by the validator account. Each new key signs the account in turn, so the
//! chain learns that the registering account holds both private keys. That
//! proof is the proof of possession.
//!
//! Registered keys wait in a pending record. The pending record takes effect at
//! the next epoch boundary. Waiting for the boundary means a rotation never
//! splits one slot between an old key and a new key.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};
use sp_core::sr25519::vrf::VrfSignature;
use sp_core::{ed25519, sr25519};

/// The separator mixed into every proof of possession payload.
pub const KEY_POSSESSION_DOMAIN: &[u8] = b"cerulea.admission.keys.v1";

/// The number of bytes one published draw proof carries.
///
/// The proof is one verifiable random function signature. The signature holds a
/// thirty two byte pre-output and a sixty four byte proof.
pub const DRAW_PROOF_BYTES: usize = 96;

/// The proof bytes one published draw carries.
pub type DrawProofBytes = [u8; DRAW_PROOF_BYTES];

/// The two public keys one validator carries.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    Serialize,
    Deserialize,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ValidatorKeySet {
    /// The sr25519 key the slot draw uses.
    pub draw: [u8; 32],
    /// The ed25519 key the block seal uses.
    pub seal: [u8; 32],
}

impl ValidatorKeySet {
    /// Pair one draw key with one seal key.
    pub const fn new(draw: [u8; 32], seal: [u8; 32]) -> Self {
        ValidatorKeySet { draw, seal }
    }

    /// The draw key as an sr25519 public key.
    pub fn draw_public(&self) -> sr25519::Public {
        sr25519::Public::from_raw(self.draw)
    }

    /// The seal key as an ed25519 public key.
    pub fn seal_public(&self) -> ed25519::Public {
        ed25519::Public::from_raw(self.seal)
    }
}

/// One draw a validator published for one epoch.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct PublishedDraw {
    /// The epoch the draw serves.
    pub epoch: u32,
    /// The thirty two output bytes the draw produced.
    pub output: [u8; 32],
    /// The proof anyone checks with the validator draw key.
    pub proof: DrawProofBytes,
}

/// One seat of one slot committee, as the chain reports it.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct SlotCommitteeSeat<AccountId> {
    /// The validator holding the seat.
    pub validator: AccountId,
    /// The two keys of that validator.
    pub keys: ValidatorKeySet,
    /// The rank the validator holds. Rank one authors.
    pub rank: u8,
    /// The seat value after scaling by the Trust Score.
    pub seat_value: u128,
    /// The epoch draw the validator published, with its proof.
    ///
    /// The field is empty until the validator publishes its draw for the epoch.
    /// The seat value then rests on the starting value below.
    pub draw: Option<PublishedDraw>,
}

/// The starting epoch output of one validator.
///
/// A chain produces its first blocks before any validator has published a draw.
/// The seat value of a validator with no published draw rests on the hash of
/// its draw key. Every node derives the same value, so the committee settles
/// from genesis. A published draw replaces the starting value as soon as the
/// draw lands on chain.
pub fn starting_epoch_output(draw_key: &[u8; 32]) -> [u8; 32] {
    let mut preimage = sp_std::vec::Vec::with_capacity(KEY_POSSESSION_DOMAIN.len() + 32);
    preimage.extend_from_slice(KEY_POSSESSION_DOMAIN);
    preimage.extend_from_slice(draw_key);
    sp_core::hashing::blake2_256(&preimage)
}

/// The bytes a new key signs to prove possession.
///
/// The payload joins the Cerulea separator and the encoded validator account.
/// A signature over those bytes carries no meaning for another account.
pub fn possession_payload(account: &[u8]) -> sp_std::vec::Vec<u8> {
    let mut payload = sp_std::vec::Vec::with_capacity(KEY_POSSESSION_DOMAIN.len() + account.len());
    payload.extend_from_slice(KEY_POSSESSION_DOMAIN);
    payload.extend_from_slice(account);
    payload
}

/// Read one published proof back into a signature.
///
/// The call yields nothing where the bytes fail to decode. The fix is to
/// publish the proof the keystore produced, with no byte changed.
pub fn read_proof(bytes: &DrawProofBytes) -> Option<VrfSignature> {
    VrfSignature::decode(&mut &bytes[..]).ok()
}

/// Write one signature out as published proof bytes.
///
/// The call yields nothing where the encoding runs to another length. Every
/// signature the keystore produces encodes to the stated length.
pub fn write_proof(signature: &VrfSignature) -> Option<DrawProofBytes> {
    let encoded = signature.encode();
    let mut bytes = [0u8; DRAW_PROOF_BYTES];
    if encoded.len() != DRAW_PROOF_BYTES {
        return None;
    }
    bytes.copy_from_slice(&encoded);
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::crypto::VrfSecret;
    use sp_core::Pair as PairTrait;

    #[test]
    fn the_possession_payload_binds_one_account() {
        let one = possession_payload(&[1u8; 32]);
        let two = possession_payload(&[2u8; 32]);
        assert_ne!(one, two);
        assert!(one.starts_with(KEY_POSSESSION_DOMAIN));
    }

    #[test]
    fn a_proof_round_trips_through_the_published_bytes() {
        let pair = sr25519::Pair::from_seed(&[5u8; 32]);
        let signature = pair.vrf_sign(&cerulea_primitives::epoch_draw_sign_data(3));
        let bytes = write_proof(&signature).expect("the signature encodes to the stated length");
        let back = read_proof(&bytes).expect("the published bytes decode");
        assert_eq!(back, signature);
    }

    #[test]
    fn broken_proof_bytes_fail_to_read() {
        let bytes = [0xffu8; DRAW_PROOF_BYTES];
        assert!(read_proof(&bytes).is_none());
    }

    #[test]
    fn a_key_set_reads_back_as_public_keys() {
        let set = ValidatorKeySet::new([7u8; 32], [8u8; 32]);
        assert_eq!(set.draw_public(), sr25519::Public::from_raw([7u8; 32]));
        assert_eq!(set.seal_public(), ed25519::Public::from_raw([8u8; 32]));
    }
}
