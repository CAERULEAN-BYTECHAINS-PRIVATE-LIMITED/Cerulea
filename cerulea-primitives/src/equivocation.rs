//! Equivocation evidence, shared between the node and the pallet set.
//!
//! Equivocation means one validator signing two conflicting blocks for the same
//! slot. A slot is one fixed span of time. Each slot draws its own committee of
//! validators and names one author at each rank. Rank one authors first. An
//! author that seals two different headers for one slot has signed two futures
//! for the same moment. The rank each header claims plays no part, so a second
//! header claiming another rank counts as equivocation too. That breaks the single history every
//! participant relies on.
//!
//! This module carries the evidence a Cerulea node collects and the check the
//! chain runs over that evidence. The node and the runtime read the same code,
//! so a proof the node builds is a proof the chain accepts.
//!
//! # What the evidence carries
//!
//! The evidence holds two sealed heads. A sealed head is the part of a block
//! header the block seal covers, together with the signature the author placed
//! over it. Three values make the proof complete.
//!
//! 1. **The pre-seal hash.** The hash of the header with the seal item removed.
//!    Signing and checking both reach these bytes.
//! 2. **The seal claim.** The encoded claim the author signed. The claim names
//!    the slot, the rank, the author seal key and the author draw key.
//! 3. **The signature.** The author signature over the joined payload.
//!
//! # What the check proves
//!
//! The check confirms that both claims name one slot, one seal key and one draw
//! key. The check confirms that the two headers differ. The check
//! then verifies both signatures against that one seal key. A proof passing all
//! three conditions is proof that the holder of that seal key signed two
//! headers for one slot. Nobody other than the key holder produces either
//! signature, so the proof stands on its own.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// The separator mixed into every Cerulea seal payload.
///
/// The node signs under this separator. The chain checks under the same
/// separator. A signature made for another purpose carries no meaning here.
pub const SEAL_DOMAIN: &[u8] = b"cerulea.authoring.seal.v1";

/// The number of bytes one encoded seal claim carries.
///
/// The claim holds the slot at eight bytes, the rank at one byte, the author
/// seal key at thirty two bytes, the slot draw at one hundred and sixty bytes
/// and the authoring moment at eight bytes.
pub const SEAL_CLAIM_BYTES: usize = 209;

/// The encoded bytes of one seal claim.
pub type SealClaimBytes = [u8; SEAL_CLAIM_BYTES];

/// The number of leading claim bytes that name the author and the slot.
///
/// The prefix holds the slot, the rank, the seal key and the draw key. Two
/// claims sharing this prefix name one author at one rank of one slot.
pub const SEAL_CLAIM_PREFIX_BYTES: usize = 73;

/// The byte offset of the slot inside an encoded claim.
pub const SLOT_OFFSET: usize = 0;

/// The byte offset of the rank inside an encoded claim.
pub const RANK_OFFSET: usize = 8;

/// The byte offset of the author seal key inside an encoded claim.
pub const SEAL_KEY_OFFSET: usize = 9;

/// The byte offset of the author draw key inside an encoded claim.
pub const DRAW_KEY_OFFSET: usize = 41;

/// One sealed block header, reduced to the bytes the seal covers.
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
pub struct SealedHead {
    /// The hash of the header with the seal item removed.
    pub pre_seal_hash: [u8; 32],
    /// The encoded seal claim the author signed.
    pub claim: SealClaimBytes,
    /// The author signature over the seal payload.
    pub signature: [u8; 64],
}

impl SealedHead {
    /// Build one sealed head from its three parts.
    pub const fn new(pre_seal_hash: [u8; 32], claim: SealClaimBytes, signature: [u8; 64]) -> Self {
        SealedHead {
            pre_seal_hash,
            claim,
            signature,
        }
    }

    /// The slot this head names.
    pub fn slot(&self) -> u64 {
        let mut raw = [0u8; 8];
        raw.copy_from_slice(&self.claim[SLOT_OFFSET..SLOT_OFFSET + 8]);
        u64::from_le_bytes(raw)
    }

    /// The rank this head claims.
    pub fn rank(&self) -> u8 {
        self.claim[RANK_OFFSET]
    }

    /// The author seal key this head names.
    pub fn seal_key(&self) -> [u8; 32] {
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&self.claim[SEAL_KEY_OFFSET..SEAL_KEY_OFFSET + 32]);
        raw
    }

    /// The author draw key this head names.
    pub fn draw_key(&self) -> [u8; 32] {
        let mut raw = [0u8; 32];
        raw.copy_from_slice(&self.claim[DRAW_KEY_OFFSET..DRAW_KEY_OFFSET + 32]);
        raw
    }

    /// The leading claim bytes that name the author, the slot and the rank.
    pub fn identity(&self) -> &[u8] {
        &self.claim[..SEAL_CLAIM_PREFIX_BYTES]
    }

    /// The bytes the author signed.
    ///
    /// The payload joins the Cerulea separator, the pre-seal hash and the
    /// encoded claim. The node builds the same bytes when it seals a header.
    pub fn payload(&self) -> alloc::vec::Vec<u8> {
        let mut payload = alloc::vec::Vec::with_capacity(SEAL_DOMAIN.len() + 32 + SEAL_CLAIM_BYTES);
        payload.extend_from_slice(SEAL_DOMAIN);
        payload.extend_from_slice(&self.pre_seal_hash);
        payload.extend_from_slice(&self.claim);
        payload
    }
}

/// The evidence that one validator sealed two headers for one slot.
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
pub struct EquivocationProof {
    /// The head the reporter saw first.
    pub first: SealedHead,
    /// The second head the reporter saw for the same slot and rank.
    pub second: SealedHead,
}

impl EquivocationProof {
    /// Pair two sealed heads as one proof.
    pub const fn new(first: SealedHead, second: SealedHead) -> Self {
        EquivocationProof { first, second }
    }
}

/// What a verified proof states.
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
pub struct EquivocationFacts {
    /// The slot both headers served.
    pub slot: u64,
    /// The rank the author claimed in both headers.
    pub rank: u8,
    /// The seal key that signed both headers.
    pub seal_key: [u8; 32],
    /// The draw key that won the seat.
    pub draw_key: [u8; 32],
    /// The pre-seal hash of the first header.
    pub first_hash: [u8; 32],
    /// The pre-seal hash of the second header.
    pub second_hash: [u8; 32],
}

/// A reason one equivocation proof failed the check.
///
/// Every reason names the fix, so an operator reads what to send instead.
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
pub enum EquivocationFault {
    /// The two claims name different slots or keys.
    IdentityMismatch,
    /// The two heads carry one pre-seal hash, so they are one header.
    OneHeaderOnly,
    /// The first signature failed the check against the named seal key.
    FirstSignatureRejected,
    /// The second signature failed the check against the named seal key.
    SecondSignatureRejected,
}

impl EquivocationFault {
    /// A plain sentence naming the problem and the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            EquivocationFault::IdentityMismatch => {
                "The two sealed headers name a different slot or validator key. \
                 Fix: report two headers sealed by one validator for one slot."
            }
            EquivocationFault::OneHeaderOnly => {
                "The two sealed headers carry one pre-seal hash, so they are one header. \
                 Fix: report two headers that differ."
            }
            EquivocationFault::FirstSignatureRejected => {
                "The first seal signature failed the check against the named validator key. \
                 Fix: report the signature the author placed on that header."
            }
            EquivocationFault::SecondSignatureRejected => {
                "The second seal signature failed the check against the named validator key. \
                 Fix: report the signature the author placed on that header."
            }
        }
    }
}

/// Check one equivocation proof and read back what it states.
///
/// The check runs three conditions in order. The two claims name one slot and
/// one validator, whatever rank each claims. The two headers differ. Both signatures verify
/// against the seal key the claims name. The call reports the first condition
/// that fails.
pub fn verify_equivocation(
    proof: &EquivocationProof,
) -> Result<EquivocationFacts, EquivocationFault> {
    let first = &proof.first;
    let second = &proof.second;
    if first.slot() != second.slot()
        || first.seal_key() != second.seal_key()
        || first.draw_key() != second.draw_key()
    {
        return Err(EquivocationFault::IdentityMismatch);
    }
    if proof.first.pre_seal_hash == proof.second.pre_seal_hash {
        return Err(EquivocationFault::OneHeaderOnly);
    }
    let seal_key = sp_core::ed25519::Public::from_raw(proof.first.seal_key());
    let first_signature = sp_core::ed25519::Signature::from_raw(proof.first.signature);
    if !sp_io::crypto::ed25519_verify(&first_signature, &proof.first.payload(), &seal_key) {
        return Err(EquivocationFault::FirstSignatureRejected);
    }
    let second_signature = sp_core::ed25519::Signature::from_raw(proof.second.signature);
    if !sp_io::crypto::ed25519_verify(&second_signature, &proof.second.payload(), &seal_key) {
        return Err(EquivocationFault::SecondSignatureRejected);
    }
    Ok(EquivocationFacts {
        slot: proof.first.slot(),
        rank: proof.first.rank(),
        seal_key: proof.first.seal_key(),
        draw_key: proof.first.draw_key(),
        first_hash: proof.first.pre_seal_hash,
        second_hash: proof.second.pre_seal_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::ed25519::Pair;
    use sp_core::Pair as PairTrait;

    /// Build one encoded claim the way the Authoring Engine encodes it.
    fn claim_bytes(slot: u64, rank: u8, seal_key: [u8; 32], draw_key: [u8; 32]) -> SealClaimBytes {
        let mut claim = [0u8; SEAL_CLAIM_BYTES];
        claim[SLOT_OFFSET..SLOT_OFFSET + 8].copy_from_slice(&slot.to_le_bytes());
        claim[RANK_OFFSET] = rank;
        claim[SEAL_KEY_OFFSET..SEAL_KEY_OFFSET + 32].copy_from_slice(&seal_key);
        claim[DRAW_KEY_OFFSET..DRAW_KEY_OFFSET + 32].copy_from_slice(&draw_key);
        claim
    }

    fn signed_head(pair: &Pair, pre_seal_hash: [u8; 32], slot: u64, rank: u8) -> SealedHead {
        let mut head = SealedHead::new(
            pre_seal_hash,
            claim_bytes(slot, rank, pair.public().0, [7u8; 32]),
            [0u8; 64],
        );
        head.signature = pair.sign(&head.payload()).0;
        head
    }

    fn author() -> Pair {
        Pair::from_seed(&[11u8; 32])
    }

    #[test]
    fn two_sealed_headers_for_one_slot_prove_equivocation() {
        let pair = author();
        let proof = EquivocationProof::new(
            signed_head(&pair, [1u8; 32], 9, 1),
            signed_head(&pair, [2u8; 32], 9, 1),
        );
        let facts = verify_equivocation(&proof).expect("the proof stands");
        assert_eq!(facts.slot, 9);
        assert_eq!(facts.rank, 1);
        assert_eq!(facts.seal_key, pair.public().0);
        assert_eq!(facts.draw_key, [7u8; 32]);
        assert_eq!(facts.first_hash, [1u8; 32]);
        assert_eq!(facts.second_hash, [2u8; 32]);
    }

    #[test]
    fn two_headers_for_two_slots_prove_nothing() {
        let pair = author();
        let proof = EquivocationProof::new(
            signed_head(&pair, [1u8; 32], 9, 1),
            signed_head(&pair, [2u8; 32], 10, 1),
        );
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::IdentityMismatch)
        );
    }

    #[test]
    fn two_headers_at_two_ranks_prove_equivocation() {
        let pair = author();
        let proof = EquivocationProof::new(
            signed_head(&pair, [1u8; 32], 9, 1),
            signed_head(&pair, [2u8; 32], 9, 2),
        );
        let facts = verify_equivocation(&proof).expect("the proof stands");
        assert_eq!(facts.slot, 9);
        assert_eq!(facts.rank, 1);
        assert_eq!(facts.seal_key, pair.public().0);
    }

    #[test]
    fn two_headers_from_two_authors_prove_nothing() {
        let one = author();
        let two = Pair::from_seed(&[12u8; 32]);
        let proof = EquivocationProof::new(
            signed_head(&one, [1u8; 32], 9, 1),
            signed_head(&two, [2u8; 32], 9, 1),
        );
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::IdentityMismatch)
        );
    }

    #[test]
    fn one_header_reported_twice_proves_nothing() {
        let pair = author();
        let head = signed_head(&pair, [1u8; 32], 9, 1);
        let proof = EquivocationProof::new(head, head);
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::OneHeaderOnly)
        );
    }

    #[test]
    fn a_broken_first_signature_fails_the_check() {
        let pair = author();
        let mut first = signed_head(&pair, [1u8; 32], 9, 1);
        first.signature[0] ^= 0xff;
        let proof = EquivocationProof::new(first, signed_head(&pair, [2u8; 32], 9, 1));
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::FirstSignatureRejected)
        );
    }

    #[test]
    fn a_broken_second_signature_fails_the_check() {
        let pair = author();
        let mut second = signed_head(&pair, [2u8; 32], 9, 1);
        second.signature[63] ^= 0xff;
        let proof = EquivocationProof::new(signed_head(&pair, [1u8; 32], 9, 1), second);
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::SecondSignatureRejected)
        );
    }

    #[test]
    fn a_signature_lifted_from_another_header_fails_the_check() {
        let pair = author();
        let first = signed_head(&pair, [1u8; 32], 9, 1);
        let mut second = signed_head(&pair, [2u8; 32], 9, 1);
        second.signature = first.signature;
        let proof = EquivocationProof::new(first, second);
        assert_eq!(
            verify_equivocation(&proof),
            Err(EquivocationFault::SecondSignatureRejected)
        );
    }

    #[test]
    fn a_head_reads_back_every_field_of_its_claim() {
        let pair = author();
        let head = signed_head(&pair, [3u8; 32], 4_000_000_001, 3);
        assert_eq!(head.slot(), 4_000_000_001);
        assert_eq!(head.rank(), 3);
        assert_eq!(head.seal_key(), pair.public().0);
        assert_eq!(head.draw_key(), [7u8; 32]);
        assert_eq!(head.identity().len(), SEAL_CLAIM_PREFIX_BYTES);
    }

    #[test]
    fn the_payload_starts_with_the_cerulea_separator() {
        let pair = author();
        let head = signed_head(&pair, [3u8; 32], 5, 1);
        let payload = head.payload();
        assert!(payload.starts_with(SEAL_DOMAIN));
        assert_eq!(payload.len(), SEAL_DOMAIN.len() + 32 + SEAL_CLAIM_BYTES);
    }

    #[test]
    fn every_fault_names_a_fix() {
        for fault in [
            EquivocationFault::IdentityMismatch,
            EquivocationFault::OneHeaderOnly,
            EquivocationFault::FirstSignatureRejected,
            EquivocationFault::SecondSignatureRejected,
        ] {
            assert!(fault.plain_language().contains("Fix:"));
        }
    }

    #[test]
    fn a_proof_round_trips_through_the_encoding() {
        let pair = author();
        let proof = EquivocationProof::new(
            signed_head(&pair, [1u8; 32], 9, 1),
            signed_head(&pair, [2u8; 32], 9, 1),
        );
        let bytes = proof.encode();
        assert_eq!(
            EquivocationProof::decode(&mut &bytes[..]).expect("the encoding decodes"),
            proof
        );
    }

    #[test]
    fn the_claim_offsets_hold_their_stated_places() {
        assert_eq!(SLOT_OFFSET, 0);
        assert_eq!(RANK_OFFSET, 8);
        assert_eq!(SEAL_KEY_OFFSET, 9);
        assert_eq!(DRAW_KEY_OFFSET, 41);
        assert_eq!(SEAL_CLAIM_PREFIX_BYTES, DRAW_KEY_OFFSET + 32);
        const { assert!(SEAL_CLAIM_BYTES > SEAL_CLAIM_PREFIX_BYTES) };
    }
}
