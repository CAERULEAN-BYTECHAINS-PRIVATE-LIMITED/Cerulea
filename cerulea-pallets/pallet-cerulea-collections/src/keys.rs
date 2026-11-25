//! Envelope encryption for the sealed privacy classes.
//!
//! A sealed collection holds one content key per key epoch. Every record
//! of that collection is sealed with the content key. The Blockchain holds
//! the content key in wrapped form alone, wrapped under the key of each
//! role that holds access. A role holder unwraps the content key with its
//! own role key and then opens the record.
//!
//! # The seal
//!
//! Cerulea builds the seal from the Blake2 hash the Blockchain already
//! carries, so a node needs no further primitive and two nodes seal alike.
//!
//! 1. **The keystream.** Block `n` of the keystream is the hash of the
//!    seal label, the key, the nonce and the block number. Blocks run in
//!    order and concatenate into a stream as long as the payload.
//! 2. **The ciphertext.** The payload takes the exclusive or of itself
//!    with the stream, byte by byte.
//! 3. **The tag.** The tag is the hash of the tag label, the key, the
//!    nonce and the ciphertext. Opening a seal recomputes the tag and
//!    compares it against the stored tag, so a changed byte is detected.
//! 4. **The nonce.** Each record takes its own nonce, derived from the
//!    collection, the key epoch and the record key. One content key
//!    therefore never repeats a keystream across two records, and a
//!    rotation gives every record a fresh keystream.
//!
//! # What the Blockchain keeps
//!
//! The Blockchain keeps the wrapped content key, the hash of the content
//! key and the hash of each role key. It keeps neither the content key nor
//! any role key in plain form. An administrator supplies the plain keys
//! inside the call that distributes or rotates them. The module checks
//! each supplied key against its stored hash before it uses the key.

use alloc::{vec, vec::Vec};
use pallet_cerulea_schema::primitives::{CollectionKey, RecordKey};

/// The label mixed into every keystream block.
pub const SEAL_DOMAIN: &[u8] = b"cerulea.collections.seal.v1";

/// The label mixed into every seal tag.
pub const TAG_DOMAIN: &[u8] = b"cerulea.collections.tag.v1";

/// The label mixed into every record nonce.
pub const NONCE_DOMAIN: &[u8] = b"cerulea.collections.nonce.v1";

/// The label mixed into every wrapped content key.
pub const WRAP_DOMAIN: &[u8] = b"cerulea.collections.wrap.v1";

/// The label mixed into every payload commitment.
pub const COMMITMENT_DOMAIN: &[u8] = b"cerulea.collections.commitment.v1";

/// The length of the tag a seal carries, in bytes.
pub const TAG_BYTES: usize = 32;

/// The length of a nonce, in bytes.
pub const NONCE_BYTES: usize = 16;

/// One keystream block.
fn keystream_block(key: &[u8; 32], nonce: &[u8; NONCE_BYTES], counter: u64) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(SEAL_DOMAIN.len() + 32 + NONCE_BYTES + 8);
    held.extend_from_slice(SEAL_DOMAIN);
    held.extend_from_slice(key);
    held.extend_from_slice(nonce);
    held.extend_from_slice(&counter.to_be_bytes());
    sp_crypto_hashing::blake2_256(&held)
}

/// The tag of one ciphertext.
fn tag_of(key: &[u8; 32], nonce: &[u8; NONCE_BYTES], ciphertext: &[u8]) -> [u8; 32] {
    let mut held: Vec<u8> =
        Vec::with_capacity(TAG_DOMAIN.len() + 32 + NONCE_BYTES + ciphertext.len());
    held.extend_from_slice(TAG_DOMAIN);
    held.extend_from_slice(key);
    held.extend_from_slice(nonce);
    held.extend_from_slice(ciphertext);
    sp_crypto_hashing::blake2_256(&held)
}

fn stream_into(key: &[u8; 32], nonce: &[u8; NONCE_BYTES], payload: &[u8]) -> Vec<u8> {
    let mut produced: Vec<u8> = Vec::with_capacity(payload.len());
    let mut counter = 0u64;
    let mut position = 0usize;
    while position < payload.len() {
        let block = keystream_block(key, nonce, counter);
        let mut step = 0usize;
        while step < block.len() && position < payload.len() {
            produced.push(payload[position] ^ block[step]);
            position = position.saturating_add(1);
            step = step.saturating_add(1);
        }
        counter = counter.saturating_add(1);
    }
    produced
}

/// Seal one payload.
///
/// The result carries the tag first and the ciphertext after it.
pub fn seal(key: &[u8; 32], nonce: &[u8; NONCE_BYTES], payload: &[u8]) -> Vec<u8> {
    let ciphertext = stream_into(key, nonce, payload);
    let tag = tag_of(key, nonce, &ciphertext);
    let mut held: Vec<u8> = Vec::with_capacity(TAG_BYTES + ciphertext.len());
    held.extend_from_slice(&tag);
    held.extend_from_slice(&ciphertext);
    held
}

/// Open one seal.
///
/// A seal whose tag disagrees answers with nothing. A key other than the
/// sealing key therefore reads nothing.
pub fn open(key: &[u8; 32], nonce: &[u8; NONCE_BYTES], sealed: &[u8]) -> Option<Vec<u8>> {
    if sealed.len() < TAG_BYTES {
        return None;
    }
    let (tag, ciphertext) = sealed.split_at(TAG_BYTES);
    let computed = tag_of(key, nonce, ciphertext);
    if computed.as_slice() != tag {
        return None;
    }
    Some(stream_into(key, nonce, ciphertext))
}

/// The nonce one record takes.
pub fn nonce_for(collection: &CollectionKey, epoch: u32, record: &RecordKey) -> [u8; NONCE_BYTES] {
    let mut held: Vec<u8> = Vec::new();
    held.extend_from_slice(NONCE_DOMAIN);
    held.extend_from_slice(&collection.deployment);
    held.extend_from_slice(collection.name.as_slice());
    held.extend_from_slice(&epoch.to_be_bytes());
    held.extend_from_slice(record.as_slice());
    let digest = sp_crypto_hashing::blake2_256(&held);
    let mut nonce = [0u8; NONCE_BYTES];
    nonce.copy_from_slice(&digest[..NONCE_BYTES]);
    nonce
}

/// The nonce a wrapped content key takes.
pub fn wrap_nonce(collection: &CollectionKey, epoch: u32, role: &[u8]) -> [u8; NONCE_BYTES] {
    let mut held: Vec<u8> = Vec::new();
    held.extend_from_slice(WRAP_DOMAIN);
    held.extend_from_slice(&collection.deployment);
    held.extend_from_slice(collection.name.as_slice());
    held.extend_from_slice(&epoch.to_be_bytes());
    held.extend_from_slice(role);
    let digest = sp_crypto_hashing::blake2_256(&held);
    let mut nonce = [0u8; NONCE_BYTES];
    nonce.copy_from_slice(&digest[..NONCE_BYTES]);
    nonce
}

/// Wrap one content key under one role key.
pub fn wrap(
    role_key: &[u8; 32],
    collection: &CollectionKey,
    epoch: u32,
    role: &[u8],
    content_key: &[u8; 32],
) -> Vec<u8> {
    let nonce = wrap_nonce(collection, epoch, role);
    seal(role_key, &nonce, content_key)
}

/// Unwrap one content key with one role key.
///
/// A role key other than the wrapping key answers with nothing.
pub fn unwrap(
    role_key: &[u8; 32],
    collection: &CollectionKey,
    epoch: u32,
    role: &[u8],
    wrapped: &[u8],
) -> Option<[u8; 32]> {
    let nonce = wrap_nonce(collection, epoch, role);
    let opened = open(role_key, &nonce, wrapped)?;
    if opened.len() != 32 {
        return None;
    }
    let mut held = [0u8; 32];
    held.copy_from_slice(&opened);
    Some(held)
}

/// The commitment one payload carries.
///
/// A hash only collection and an external collection both hold this
/// commitment on the Blockchain while the payload lives elsewhere.
pub fn commitment(payload: &[u8]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(COMMITMENT_DOMAIN.len() + payload.len());
    held.extend_from_slice(COMMITMENT_DOMAIN);
    held.extend_from_slice(payload);
    sp_crypto_hashing::blake2_256(&held)
}

/// The hash the Blockchain keeps of one key.
///
/// The module checks a supplied key against this hash before it uses the
/// key, so a wrong key is refused rather than producing an unreadable
/// record.
pub fn key_commitment(key: &[u8; 32]) -> [u8; 32] {
    let mut held: Vec<u8> = vec![0u8; 0];
    held.extend_from_slice(b"cerulea.collections.key.v1");
    held.extend_from_slice(key);
    sp_crypto_hashing::blake2_256(&held)
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::BoundedVec;
    use pallet_cerulea_schema::primitives::name_of;

    fn slot() -> CollectionKey {
        CollectionKey::new([4u8; 32], name_of("parcel").expect("the name fits"))
    }

    fn record() -> RecordKey {
        BoundedVec::try_from(b"12/3".to_vec()).expect("the key fits")
    }

    #[test]
    fn a_seal_opens_with_the_key_that_sealed_it() {
        let key = [7u8; 32];
        let nonce = nonce_for(&slot(), 1, &record());
        let payload = b"the holder of parcel 12/3 is Anitha".to_vec();
        let sealed = seal(&key, &nonce, &payload);
        assert_ne!(sealed[TAG_BYTES..], payload[..]);
        assert_eq!(open(&key, &nonce, &sealed), Some(payload));
    }

    #[test]
    fn a_seal_declines_another_key() {
        let nonce = nonce_for(&slot(), 1, &record());
        let sealed = seal(&[7u8; 32], &nonce, b"a payload");
        assert_eq!(open(&[8u8; 32], &nonce, &sealed), None);
    }

    #[test]
    fn a_seal_declines_a_changed_byte() {
        let key = [7u8; 32];
        let nonce = nonce_for(&slot(), 1, &record());
        let mut sealed = seal(&key, &nonce, b"a payload");
        let last = sealed.len().saturating_sub(1);
        sealed[last] ^= 1;
        assert_eq!(open(&key, &nonce, &sealed), None);
    }

    #[test]
    fn a_seal_declines_a_payload_shorter_than_its_tag() {
        assert_eq!(open(&[7u8; 32], &[0u8; NONCE_BYTES], &[1, 2, 3]), None);
    }

    #[test]
    fn an_empty_payload_seals_and_opens() {
        let key = [7u8; 32];
        let nonce = nonce_for(&slot(), 1, &record());
        let sealed = seal(&key, &nonce, &[]);
        assert_eq!(sealed.len(), TAG_BYTES);
        assert_eq!(open(&key, &nonce, &sealed), Some(Vec::new()));
    }

    #[test]
    fn two_epochs_give_one_record_two_keystreams() {
        let first = nonce_for(&slot(), 1, &record());
        let second = nonce_for(&slot(), 2, &record());
        assert_ne!(first, second);
    }

    #[test]
    fn two_records_take_two_nonces() {
        let other: RecordKey = BoundedVec::try_from(b"44/9".to_vec()).expect("the key fits");
        assert_ne!(
            nonce_for(&slot(), 1, &record()),
            nonce_for(&slot(), 1, &other)
        );
    }

    #[test]
    fn a_wrapped_content_key_unwraps_with_the_role_key() {
        let role_key = [3u8; 32];
        let content = [9u8; 32];
        let wrapped = wrap(&role_key, &slot(), 1, b"registrar", &content);
        assert_eq!(
            unwrap(&role_key, &slot(), 1, b"registrar", &wrapped),
            Some(content)
        );
        assert_eq!(unwrap(&[4u8; 32], &slot(), 1, b"registrar", &wrapped), None);
        assert_eq!(unwrap(&role_key, &slot(), 2, b"registrar", &wrapped), None);
        assert_eq!(unwrap(&role_key, &slot(), 1, b"clerk", &wrapped), None);
    }

    #[test]
    fn a_commitment_separates_two_payloads() {
        assert_ne!(commitment(b"one"), commitment(b"two"));
        assert_eq!(commitment(b"one"), commitment(b"one"));
        assert_ne!(key_commitment(&[1u8; 32]), key_commitment(&[2u8; 32]));
    }

    #[test]
    fn a_long_payload_seals_across_several_blocks() {
        let key = [7u8; 32];
        let nonce = nonce_for(&slot(), 1, &record());
        let payload: Vec<u8> = (0..300u32).map(|held| (held % 251) as u8).collect();
        let sealed = seal(&key, &nonce, &payload);
        assert_eq!(sealed.len(), TAG_BYTES + payload.len());
        assert_eq!(open(&key, &nonce, &sealed), Some(payload));
    }
}
