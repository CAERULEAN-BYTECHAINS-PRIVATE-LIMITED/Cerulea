//! The block seal signed from the node keystore.
//!
//! The seal is a signature over the block header by the validator key of the
//! author. The key lives in the node keystore under the Cerulea key type. The
//! private key never leaves the keystore.
//!
//! The seal closes the forgeable authorship defect. Before the seal, authorship
//! was asserted by an unsigned digest naming a public key, so any node built a
//! block naming the scheduled author. A sealed block carries a signature that
//! only the holder of the author key produces.
//!
//! # What the seal covers
//!
//! The signature covers the pre-seal header hash and the seal claim. The
//! pre-seal header hash is the hash of the header with the seal item removed,
//! so signing and checking reach the same bytes. The claim names the slot, the
//! rank the author claimed, the author signing key and the slot draw of the
//! author. Binding all of that under one signature means a block cannot move to
//! another slot, another rank or another author.

use codec::{Decode, Encode};
use sp_core::{ed25519, sr25519};
use sp_keystore::KeystorePtr;
use sp_runtime::generic::DigestItem;
use sp_runtime::traits::{Hash as HashT, Header as HeaderT};

use super::draw::DrawResult;
use crate::engines::{EngineError, Rank, Slot};
use crate::{CERULEA_ENGINE_ID, CERULEA_KEY_TYPE};

/// The separator mixed into every Cerulea seal payload.
pub const SEAL_DOMAIN: &[u8] = b"cerulea.authoring.seal.v1";

/// The two public keys one validator carries.
///
/// The draw key evaluates the verifiable random function for the slot. The seal
/// key signs the block header. The Admission Engine records the pair when the
/// validator joins the set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode)]
pub struct ValidatorKeys {
    /// The sr25519 key the slot draw uses.
    pub draw: sr25519::Public,
    /// The ed25519 key the block seal uses.
    pub seal: ed25519::Public,
}

impl ValidatorKeys {
    /// Pair one draw key with one seal key.
    pub fn new(draw: sr25519::Public, seal: ed25519::Public) -> Self {
        ValidatorKeys { draw, seal }
    }
}

/// The claim the block seal carries.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct SealClaim {
    /// The slot the block serves.
    pub slot: Slot,
    /// The rank the author claimed.
    pub rank: Rank,
    /// The signing key of the author.
    pub author: ed25519::Public,
    /// The slot draw of the author, with its proof.
    pub draw: DrawResult,
}

impl SealClaim {
    /// Build a claim.
    pub fn new(slot: Slot, rank: Rank, author: ed25519::Public, draw: DrawResult) -> Self {
        SealClaim {
            slot,
            rank,
            author,
            draw,
        }
    }

    /// The two keys the claim binds together.
    pub fn keys(&self) -> ValidatorKeys {
        ValidatorKeys::new(self.draw.validator, self.author)
    }
}

/// A block seal.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
pub struct BlockSeal {
    /// The claim the seal covers.
    pub claim: SealClaim,
    /// The signature of the author over the payload.
    pub signature: ed25519::Signature,
}

impl BlockSeal {
    /// The digest item the block header carries.
    pub fn digest_item(&self) -> DigestItem {
        DigestItem::Seal(CERULEA_ENGINE_ID, self.encode())
    }
}

/// The bytes the author signs.
///
/// The payload joins the Cerulea separator, the pre-seal header hash and the
/// encoded claim.
pub fn seal_payload(pre_seal_hash: &[u8], claim: &SealClaim) -> Vec<u8> {
    let mut payload = Vec::with_capacity(SEAL_DOMAIN.len() + pre_seal_hash.len() + 128);
    payload.extend_from_slice(SEAL_DOMAIN);
    payload.extend_from_slice(pre_seal_hash);
    claim.encode_to(&mut payload);
    payload
}

/// The header with the Cerulea seal item removed.
pub fn strip_seal<H: HeaderT>(header: &H) -> H {
    let mut stripped = header.clone();
    stripped
        .digest_mut()
        .logs
        .retain(|item| !is_cerulea_seal(item));
    stripped
}

/// The hash of the header with the seal item removed.
///
/// Signing and checking both reach these bytes, so the seal covers the whole
/// header apart from the seal itself.
pub fn pre_seal_hash<H: HeaderT>(header: &H) -> H::Hash {
    strip_seal(header).hash()
}

/// Read the Cerulea seal out of a block header.
pub fn extract_seal<H: HeaderT>(header: &H) -> Option<BlockSeal> {
    header.digest().logs().iter().find_map(|item| match item {
        DigestItem::Seal(engine, payload) if *engine == CERULEA_ENGINE_ID => {
            BlockSeal::decode(&mut &payload[..]).ok()
        }
        _ => None,
    })
}

/// Report whether a digest item is the Cerulea seal.
pub fn is_cerulea_seal(item: &DigestItem) -> bool {
    matches!(item, DigestItem::Seal(engine, _) if *engine == CERULEA_ENGINE_ID)
}

/// Seal one block header with the author key held in the node keystore.
///
/// The call reports a problem where the keystore holds no key for the named
/// author. Block production stops there rather than publishing an unsigned
/// block.
pub fn seal_header<H: HeaderT>(
    keystore: &KeystorePtr,
    header: &H,
    claim: SealClaim,
) -> Result<BlockSeal, EngineError> {
    let hash = pre_seal_hash(header);
    let payload = seal_payload(hash.as_ref(), &claim);
    let signature = keystore
        .ed25519_sign(CERULEA_KEY_TYPE, &claim.author, &payload)
        .map_err(|reason| EngineError::KeystoreRefused {
            detail: format!("{:?}", reason),
        })?
        .ok_or(EngineError::SealKeyAbsent)?;
    Ok(BlockSeal { claim, signature })
}

/// Seal one block header and place the seal on the header.
pub fn seal_and_apply<H: HeaderT>(
    keystore: &KeystorePtr,
    header: &mut H,
    claim: SealClaim,
) -> Result<BlockSeal, EngineError> {
    let seal = seal_header(keystore, header, claim)?;
    header.digest_mut().push(seal.digest_item());
    Ok(seal)
}

/// Check one seal signature against the header it covers.
///
/// The check reads the author public key alone. A header carrying a signature
/// from another key fails.
pub fn check_seal_signature<H: HeaderT>(header: &H, seal: &BlockSeal) -> Result<(), EngineError> {
    let hash = pre_seal_hash(header);
    let payload = seal_payload(hash.as_ref(), &seal.claim);
    if sp_io::crypto::ed25519_verify(&seal.signature, &payload, &seal.claim.author) {
        Ok(())
    } else {
        Err(EngineError::SealSignatureRejected {
            slot: seal.claim.slot.0,
        })
    }
}

/// The hash of a header, using the hashing of that header type.
pub fn header_hash<H: HeaderT>(header: &H) -> H::Hash {
    <H::Hashing as HashT>::hash_of(header)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::draw::{draw, DrawInput};
    use crate::engines::SlotSeed;
    use sp_core::Pair as PairTrait;
    use sp_keystore::testing::MemoryKeystore;
    use sp_keystore::Keystore;
    use sp_runtime::generic::{Digest, Header};
    use sp_runtime::traits::BlakeTwo256;
    use std::sync::Arc;

    type TestHeader = Header<u32, BlakeTwo256>;

    fn header(number: u32) -> TestHeader {
        TestHeader::new(
            number,
            Default::default(),
            Default::default(),
            Default::default(),
            Digest::default(),
        )
    }

    fn keystore() -> KeystorePtr {
        Arc::new(MemoryKeystore::new())
    }

    fn author_key(keystore: &KeystorePtr, seed: &str) -> ed25519::Public {
        keystore
            .ed25519_generate_new(CERULEA_KEY_TYPE, Some(seed))
            .expect("the keystore generates a key")
    }

    fn claim(author: ed25519::Public, slot: u64, rank: u8) -> SealClaim {
        let pair = sr25519::Pair::from_seed(&[3u8; 32]);
        let input = DrawInput::new(Slot(slot), SlotSeed([1u8; 32]));
        SealClaim::new(Slot(slot), Rank(rank), author, draw(&pair, &input))
    }

    #[test]
    fn a_sealed_header_verifies_against_the_author_key() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let seal = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        assert_eq!(head.digest().logs().len(), 1);
        check_seal_signature(&head, &seal).expect("the seal checks out");
        let read = extract_seal(&head).expect("the header carries the seal");
        assert_eq!(read, seal);
    }

    #[test]
    fn a_forged_seal_is_rejected() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let other = author_key(&keystore, "//Cerulea//other");
        let mut head = header(7);
        let honest = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        // The forger names itself as the author and reuses the signature of the
        // honest author.
        let mut forged = honest.clone();
        forged.claim.author = other;
        let error =
            check_seal_signature(&head, &forged).expect_err("the forged seal fails the check");
        assert!(matches!(error, EngineError::SealSignatureRejected { .. }));
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_seal_moved_to_another_header_is_rejected() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let seal = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        let other = header(8);
        assert!(check_seal_signature(&other, &seal).is_err());
    }

    #[test]
    fn a_seal_moved_to_another_slot_is_rejected() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let seal = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        let mut moved = seal.clone();
        moved.claim.slot = Slot(8);
        assert!(check_seal_signature(&head, &moved).is_err());
    }

    #[test]
    fn a_seal_moved_to_another_rank_is_rejected() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let seal = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        let mut moved = seal.clone();
        moved.claim.rank = Rank(2);
        assert!(check_seal_signature(&head, &moved).is_err());
    }

    #[test]
    fn a_changed_header_field_breaks_the_seal() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let seal = seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        head.set_number(9);
        assert!(check_seal_signature(&head, &seal).is_err());
    }

    #[test]
    fn a_keystore_without_the_author_key_names_the_fix() {
        let keystore = keystore();
        let absent = ed25519::Pair::from_seed(&[9u8; 32]).public();
        let head = header(7);
        let error = seal_header(&keystore, &head, claim(absent, 7, 1))
            .expect_err("the keystore holds no key for this author");
        assert_eq!(error, EngineError::SealKeyAbsent);
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn the_pre_seal_hash_leaves_the_seal_item_out() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        let before = pre_seal_hash(&head);
        seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        assert_eq!(pre_seal_hash(&head), before);
        assert_ne!(header_hash(&head), before);
    }

    #[test]
    fn stripping_the_seal_leaves_every_other_item() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let mut head = header(7);
        head.digest_mut()
            .push(DigestItem::Other(b"another item".to_vec()));
        seal_and_apply(&keystore, &mut head, claim(author, 7, 1))
            .expect("the keystore seals the header");
        assert_eq!(head.digest().logs().len(), 2);
        let stripped = strip_seal(&head);
        assert_eq!(stripped.digest().logs().len(), 1);
        assert!(extract_seal(&stripped).is_none());
    }

    #[test]
    fn a_header_without_a_seal_carries_no_claim() {
        assert!(extract_seal(&header(7)).is_none());
    }

    #[test]
    fn a_seal_round_trips_through_scale() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let head = header(7);
        let seal =
            seal_header(&keystore, &head, claim(author, 7, 1)).expect("the keystore seals it");
        let bytes = seal.encode();
        assert_eq!(
            BlockSeal::decode(&mut &bytes[..]).expect("the encoding decodes"),
            seal
        );
    }

    #[test]
    fn the_claim_reads_back_the_two_validator_keys() {
        let keystore = keystore();
        let author = author_key(&keystore, "//Cerulea//author");
        let claim = claim(author, 7, 1);
        let keys = claim.keys();
        assert_eq!(keys.seal, author);
        assert_eq!(keys.draw, sr25519::Pair::from_seed(&[3u8; 32]).public());
        assert_eq!(keys, ValidatorKeys::new(keys.draw, keys.seal));
    }

    #[test]
    fn the_engine_identifier_and_the_key_type_hold_their_stated_values() {
        assert_eq!(CERULEA_ENGINE_ID, *b"ceru");
        assert_eq!(CERULEA_KEY_TYPE.0, *b"cerk");
    }
}
