//! Signature aggregation and the signer bitmap.
//!
//! The author combines the signatures it collected into one envelope and places
//! the envelope in the block. The envelope carries a bitmap naming the signers
//! by position in the validator set, and the signatures in that same position
//! order. No public key travels with the envelope. Every node recovers the
//! signer keys from the validator set it already holds.
//!
//! # Why an envelope rather than one fixed-size signature
//!
//! A scheme that folds many signatures into one value of fixed size needs
//! pairing friendly curves. The dependency set carries that curve behind an
//! experimental flag that stays off. Cerulea therefore ships a compact envelope
//! over the signature scheme the specification names.
//!
//! The security of the envelope rests on the same assumption as one signature.
//! Each signature verifies on its own against the public key recovered by
//! position. A forged envelope needs a forged signature, so the envelope
//! inherits unforgeability from the underlying scheme and adds no new
//! assumption. The envelope also drops every public key from the wire, which
//! holds the size to sixty four bytes per signer plus one bit per set member.
//!
//! The envelope carries a scheme number. A later release adds a folded
//! signature under a second scheme number and both forms decode.

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_core::ed25519;
use sp_runtime::generic::{Digest, DigestItem};

use super::collect::{vote_payload, SignatureCollector};
use crate::engines::{EngineError, Slot, TrustScore};
use crate::CERULEA_COMMIT_ENGINE_ID;

/// The scheme number of the compact envelope over the named signature scheme.
pub const SCHEME_ENVELOPE_V1: u8 = 1;

/// A bitmap naming the signers by position in the validator set.
///
/// Position zero sits in the lowest bit of the first byte. Position eight sits
/// in the lowest bit of the second byte.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Default)]
pub struct SignerBitmap {
    /// The number of positions the bitmap covers.
    members: u32,
    /// The bits, packed eight to a byte.
    bits: Vec<u8>,
}

impl SignerBitmap {
    /// Build an empty bitmap covering the given number of positions.
    pub fn new(members: usize) -> Self {
        SignerBitmap {
            members: members as u32,
            bits: vec![0u8; members.div_ceil(8)],
        }
    }

    /// The number of positions the bitmap covers.
    pub fn len(&self) -> usize {
        self.members as usize
    }

    /// Report whether the bitmap covers zero positions.
    pub fn is_empty(&self) -> bool {
        self.members == 0
    }

    /// Mark one position as a signer.
    ///
    /// A position outside the bitmap reports a problem.
    pub fn set(&mut self, position: usize) -> Result<(), EngineError> {
        if position >= self.len() {
            return Err(EngineError::BitmapPositionOutside {
                position,
                members: self.len(),
            });
        }
        self.bits[position / 8] |= 1u8 << (position % 8);
        Ok(())
    }

    /// Report whether one position signed.
    pub fn holds(&self, position: usize) -> bool {
        if position >= self.len() {
            return false;
        }
        self.bits[position / 8] & (1u8 << (position % 8)) != 0
    }

    /// The signer positions, in ascending order.
    pub fn positions(&self) -> Vec<usize> {
        (0..self.len()).filter(|index| self.holds(*index)).collect()
    }

    /// The number of signers the bitmap names.
    pub fn count(&self) -> usize {
        self.bits
            .iter()
            .map(|byte| byte.count_ones() as usize)
            .sum()
    }

    /// Report whether the bitmap holds a bit past the covered positions.
    ///
    /// A bitmap arriving from the network passes this check before use.
    pub fn well_formed(&self) -> bool {
        if self.bits.len() != self.len().div_ceil(8) {
            return false;
        }
        let spare = self.bits.len() * 8 - self.len();
        if spare == 0 {
            return true;
        }
        match self.bits.last() {
            Some(last) => (last >> (8 - spare)) == 0,
            None => true,
        }
    }
}

/// The aggregate of every signature the author collected.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct AggregateSignature {
    /// The scheme the envelope carries.
    pub scheme: u8,
    /// The slot the block serves.
    pub slot: Slot,
    /// The hash of the block the signers accepted.
    pub block_hash: [u8; 32],
    /// The signers, named by position in the validator set.
    pub signers: SignerBitmap,
    /// The signatures, in signer position order.
    pub signatures: Vec<ed25519::Signature>,
}

impl AggregateSignature {
    /// Combine the signatures one collector gathered.
    pub fn assemble(collector: &SignatureCollector) -> Result<Self, EngineError> {
        let mut signers = SignerBitmap::new(collector.member_count());
        let mut signatures = Vec::with_capacity(collector.signer_count());
        for (position, vote) in collector
            .signer_positions()
            .into_iter()
            .zip(collector.votes())
        {
            signers.set(position)?;
            signatures.push(vote.signature);
        }
        Ok(AggregateSignature {
            scheme: SCHEME_ENVELOPE_V1,
            slot: collector.slot(),
            block_hash: collector.block_hash(),
            signers,
            signatures,
        })
    }

    /// The number of signers the envelope carries.
    pub fn signer_count(&self) -> usize {
        self.signatures.len()
    }

    /// The bytes the envelope occupies on the wire.
    pub fn encoded_len(&self) -> usize {
        self.encode().len()
    }

    /// Check the envelope against the validator set.
    ///
    /// The set arrives in the same order the collector held. Each signature
    /// verifies against the key recovered by position.
    pub fn verify(&self, members: &[ed25519::Public]) -> Result<(), EngineError> {
        if self.scheme != SCHEME_ENVELOPE_V1 {
            return Err(EngineError::AggregateSchemeUnknown {
                scheme: self.scheme,
            });
        }
        if self.signers.len() != members.len() || !self.signers.well_formed() {
            return Err(EngineError::AggregateBitmapMalformed);
        }
        let positions = self.signers.positions();
        if positions.len() != self.signatures.len() {
            return Err(EngineError::AggregateCountMismatch {
                named: positions.len(),
                carried: self.signatures.len(),
            });
        }
        let payload = vote_payload(self.slot, &self.block_hash);
        for (position, signature) in positions.iter().zip(self.signatures.iter()) {
            let key = &members[*position];
            if !sp_io::crypto::ed25519_verify(signature, &payload, key) {
                return Err(EngineError::AggregateSignatureRejected {
                    position: *position,
                });
            }
        }
        Ok(())
    }

    /// The Trust Score weight the signers carry.
    ///
    /// The weights arrive in the same order the collector held.
    pub fn signed_weight(&self, weights: &[TrustScore]) -> u128 {
        self.signers
            .positions()
            .into_iter()
            .filter_map(|position| weights.get(position))
            .map(|weight| weight.weight())
            .sum()
    }

    /// The digest item the block header carries.
    pub fn digest_item(&self) -> DigestItem {
        DigestItem::Consensus(CERULEA_COMMIT_ENGINE_ID, self.encode())
    }

    /// Read the envelope out of a block header digest.
    pub fn from_digest(digest: &Digest) -> Option<AggregateSignature> {
        digest.logs().iter().find_map(|item| match item {
            DigestItem::Consensus(engine, payload) if *engine == CERULEA_COMMIT_ENGINE_ID => {
                AggregateSignature::decode(&mut &payload[..]).ok()
            }
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::commit::collect::cast_vote;
    use crate::CERULEA_KEY_TYPE;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::{Keystore, KeystorePtr};
    use std::sync::Arc;

    const SLOT: Slot = Slot(90);
    const BLOCK: [u8; 32] = [21u8; 32];
    const OPENS: u64 = 1_000;
    const WINDOW: u64 = 900;

    struct Network {
        keystores: Vec<KeystorePtr>,
        keys: Vec<ed25519::Public>,
        weights: Vec<TrustScore>,
        collector: SignatureCollector,
    }

    fn build(count: usize) -> Network {
        let mut keystores = Vec::new();
        let mut keys = Vec::new();
        for index in 0..count {
            let memory = MemoryKeystore::new();
            let key = memory
                .ed25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//aggregate//{}", index)),
                )
                .expect("the keystore generates a key");
            keystores.push(Arc::new(memory) as KeystorePtr);
            keys.push(key);
        }
        let set: Vec<(ed25519::Public, TrustScore)> = keys
            .iter()
            .map(|key| (*key, TrustScore::from_percent(50)))
            .collect();
        let collector = SignatureCollector::open(SLOT, BLOCK, &set, OPENS, WINDOW)
            .expect("the collector opens");
        // The collector sorts the set. The keystore list follows that order.
        let members = collector.members().to_vec();
        let mut ordered_stores = Vec::with_capacity(count);
        for member in &members {
            let position = keys
                .iter()
                .position(|key| key == member)
                .expect("the member sits in the set");
            ordered_stores.push(keystores[position].clone());
        }
        let weights = vec![TrustScore::from_percent(50); count];
        Network {
            keystores: ordered_stores,
            keys: members,
            weights,
            collector,
        }
    }

    fn sign_all(network: &mut Network, positions: &[usize]) {
        for position in positions {
            let vote = cast_vote(
                &network.keystores[*position],
                &network.keys[*position],
                SLOT,
                BLOCK,
            )
            .expect("the validator signs");
            network
                .collector
                .accept(vote, OPENS + 1)
                .expect("the vote lands");
        }
    }

    #[test]
    fn the_envelope_carries_one_bit_per_member_and_one_signature_per_signer() {
        let mut network = build(9);
        sign_all(&mut network, &[0, 2, 5]);
        let envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        assert_eq!(envelope.signers.len(), 9);
        assert_eq!(envelope.signers.count(), 3);
        assert_eq!(envelope.signer_count(), 3);
        assert_eq!(envelope.signers.positions(), vec![0, 2, 5]);
        envelope
            .verify(&network.keys)
            .expect("the envelope checks out");
    }

    #[test]
    fn the_envelope_verifies_against_the_validator_set_alone() {
        let mut network = build(6);
        sign_all(&mut network, &[0, 1, 2, 3, 4, 5]);
        let envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        assert!(envelope.verify(&network.keys).is_ok());
        assert_eq!(envelope.signed_weight(&network.weights), 6 * 500_000u128);
    }

    #[test]
    fn a_changed_signature_is_rejected() {
        let mut network = build(5);
        sign_all(&mut network, &[0, 1, 2]);
        let mut envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        envelope.signatures[1] = ed25519::Signature::from_raw([4u8; 64]);
        let error = envelope
            .verify(&network.keys)
            .expect_err("a changed signature fails");
        assert!(matches!(
            error,
            EngineError::AggregateSignatureRejected { position: 1 }
        ));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_bitmap_naming_more_signers_than_the_envelope_carries_is_rejected() {
        let mut network = build(5);
        sign_all(&mut network, &[0, 1]);
        let mut envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        envelope.signers.set(4).expect("the position sits inside");
        let error = envelope
            .verify(&network.keys)
            .expect_err("a count mismatch fails");
        assert_eq!(
            error,
            EngineError::AggregateCountMismatch {
                named: 3,
                carried: 2
            }
        );
    }

    #[test]
    fn a_bitmap_sized_for_another_set_is_rejected() {
        let mut network = build(5);
        sign_all(&mut network, &[0]);
        let envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        let error = envelope
            .verify(&network.keys[..4])
            .expect_err("a bitmap sized for another set fails");
        assert_eq!(error, EngineError::AggregateBitmapMalformed);
    }

    #[test]
    fn a_bitmap_holding_a_bit_past_the_set_is_rejected() {
        let mut network = build(5);
        sign_all(&mut network, &[0]);
        let mut envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        // Position five sits past the five member set and the check catches it.
        envelope.signers.bits[0] |= 1u8 << 5;
        assert!(!envelope.signers.well_formed());
        assert_eq!(
            envelope.verify(&network.keys).expect_err("the check fails"),
            EngineError::AggregateBitmapMalformed
        );
    }

    #[test]
    fn an_unknown_scheme_is_rejected() {
        let mut network = build(3);
        sign_all(&mut network, &[0]);
        let mut envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        envelope.scheme = 9;
        assert_eq!(
            envelope.verify(&network.keys).expect_err("the check fails"),
            EngineError::AggregateSchemeUnknown { scheme: 9 }
        );
    }

    #[test]
    fn the_envelope_round_trips_through_the_block_header_digest() {
        let mut network = build(7);
        sign_all(&mut network, &[1, 3, 6]);
        let envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        let mut digest = Digest::default();
        digest.push(envelope.digest_item());
        let read = AggregateSignature::from_digest(&digest).expect("the header carries it");
        assert_eq!(read, envelope);
        read.verify(&network.keys).expect("the envelope checks out");
        assert!(Digest::default().logs().is_empty());
        assert!(AggregateSignature::from_digest(&Digest::default()).is_none());
    }

    #[test]
    fn the_envelope_carries_no_public_key() {
        let mut network = build(8);
        sign_all(&mut network, &[0, 1, 2, 3, 4, 5, 6, 7]);
        let envelope =
            AggregateSignature::assemble(&network.collector).expect("the envelope assembles");
        let bytes = envelope.encode();
        for key in &network.keys {
            assert!(
                !bytes.windows(32).any(|window| window == key.0),
                "the wire form carries no public key"
            );
        }
        // Sixty four bytes per signer plus one byte of bitmap for eight members.
        assert!(envelope.encoded_len() < 64 * 8 + 128);
    }

    #[test]
    fn a_bitmap_reads_back_every_position_it_holds() {
        let mut bitmap = SignerBitmap::new(20);
        assert_eq!(bitmap.len(), 20);
        assert!(!bitmap.is_empty());
        assert_eq!(bitmap.count(), 0);
        for position in [0usize, 7, 8, 19] {
            bitmap.set(position).expect("the position sits inside");
        }
        assert_eq!(bitmap.positions(), vec![0, 7, 8, 19]);
        assert_eq!(bitmap.count(), 4);
        assert!(bitmap.holds(19));
        assert!(!bitmap.holds(18));
        assert!(!bitmap.holds(20));
        assert!(bitmap.well_formed());
        assert!(SignerBitmap::new(0).is_empty());
    }

    #[test]
    fn a_position_outside_the_bitmap_names_the_fix() {
        let mut bitmap = SignerBitmap::new(4);
        let error = bitmap.set(4).expect_err("the position sits outside");
        assert_eq!(
            error,
            EngineError::BitmapPositionOutside {
                position: 4,
                members: 4
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_bitmap_round_trips_through_scale() {
        let mut bitmap = SignerBitmap::new(11);
        bitmap.set(3).expect("the position sits inside");
        bitmap.set(10).expect("the position sits inside");
        let bytes = bitmap.encode();
        let back = SignerBitmap::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(back, bitmap);
        assert_eq!(back.positions(), vec![3, 10]);
    }
}
