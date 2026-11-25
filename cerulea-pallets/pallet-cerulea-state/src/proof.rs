//! Record inclusion proof generation.
//!
//! Every record proves its own presence without revealing the rest of the
//! state. The state module keeps one sparse Merkle tree per collection and
//! answers a proof request from that tree.
//!
//! # The construction
//!
//! 1. **The record digest.** A record reduces to one digest over a domain
//!    label, the record key behind its length, and the stored bytes. The
//!    domain label keeps a record digest apart from every other digest the
//!    Blockchain computes.
//! 2. **The slot.** A record key lands in one of 65536 slots. The slot is
//!    the first two bytes of the hash of the key, read as a big endian
//!    number. Hashing the key spreads the records evenly, so no chosen key
//!    deepens one branch.
//! 3. **The bucket.** A slot holds the record keys that land on it,
//!    ordered by key, each beside its record digest. Two keys sharing a
//!    slot therefore share a leaf. The bucket digest covers every entry in
//!    order.
//! 4. **The tree.** The leaf level holds 65536 bucket digests. Sixteen
//!    levels of pairing reduce the leaves to one root. A missing subtree
//!    takes the empty digest of its level, which the module derives once
//!    from the empty leaf label. A write therefore touches sixteen nodes
//!    rather than the whole collection.
//! 5. **The proof.** A proof carries the slot, the bucket and the sixteen
//!    sibling digests from the leaf to the root. A verifier recomputes the
//!    bucket digest, folds the siblings and compares the result with the
//!    collection root.
//!
//! # Verifying against the state root
//!
//! The collection root lives in Blockchain storage under `Roots`. That
//! storage item sits inside the state trie, so the block state root
//! commits to it. A verifier holding a block header therefore takes two
//! steps. First it reads the collection root with a storage proof against
//! the state root in the header. Then it checks the record with the
//! inclusion proof here. The two steps together prove that the record sits
//! in the state the block committed.

use alloc::{vec, vec::Vec};
use codec::{Decode, Encode};
use pallet_cerulea_schema::primitives::RecordKey;
use scale_info::TypeInfo;

/// The number of pairing levels above the leaf level.
pub const TREE_DEPTH: u8 = 16;

/// The number of slots the leaf level holds.
pub const SLOT_COUNT: u32 = 1 << 16;

/// The label mixed into the digest of one stored record.
pub const RECORD_DOMAIN: &[u8] = b"cerulea.state.record.v1";

/// The label mixed into the digest of one bucket.
pub const BUCKET_DOMAIN: &[u8] = b"cerulea.state.bucket.v1";

/// The label mixed into the digest of one internal node.
pub const NODE_DOMAIN: &[u8] = b"cerulea.state.node.v1";

/// The label the empty digest of the leaf level derives from.
pub const EMPTY_DOMAIN: &[u8] = b"cerulea.state.empty.v1";

/// One entry of one bucket.
pub type BucketEntry = (RecordKey, [u8; 32]);

/// The slot one record key lands in.
pub fn slot_of(key: &[u8]) -> u16 {
    let digest = sp_crypto_hashing::blake2_256(key);
    u16::from_be_bytes([digest[0], digest[1]])
}

/// The digest of one stored record.
pub fn record_digest(key: &[u8], value: &[u8]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(RECORD_DOMAIN.len() + 4 + key.len() + value.len());
    held.extend_from_slice(RECORD_DOMAIN);
    let length = u32::try_from(key.len()).unwrap_or(u32::MAX);
    held.extend_from_slice(&length.to_be_bytes());
    held.extend_from_slice(key);
    held.extend_from_slice(value);
    sp_crypto_hashing::blake2_256(&held)
}

/// The digest of one bucket.
///
/// An empty bucket takes the empty digest of the leaf level, so a slot
/// that holds nothing and a slot that was cleared agree.
pub fn bucket_digest(entries: &[BucketEntry]) -> [u8; 32] {
    if entries.is_empty() {
        return empty_at(0);
    }
    let mut held: Vec<u8> = Vec::new();
    held.extend_from_slice(BUCKET_DOMAIN);
    for (key, digest) in entries.iter() {
        let length = u32::try_from(key.len()).unwrap_or(u32::MAX);
        held.extend_from_slice(&length.to_be_bytes());
        held.extend_from_slice(key.as_slice());
        held.extend_from_slice(digest);
    }
    sp_crypto_hashing::blake2_256(&held)
}

/// The digest of one internal node.
pub fn node_digest(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut held: Vec<u8> = Vec::with_capacity(NODE_DOMAIN.len() + 64);
    held.extend_from_slice(NODE_DOMAIN);
    held.extend_from_slice(left);
    held.extend_from_slice(right);
    sp_crypto_hashing::blake2_256(&held)
}

/// The digest of an empty subtree at one level.
///
/// Level zero is the empty leaf. Each level above pairs the level below
/// with itself.
pub fn empty_at(level: u8) -> [u8; 32] {
    let mut held = sp_crypto_hashing::blake2_256(EMPTY_DOMAIN);
    let mut step = 0u8;
    while step < level {
        held = node_digest(&held, &held);
        step = step.saturating_add(1);
    }
    held
}

/// The root of a collection holding no records.
pub fn empty_root() -> [u8; 32] {
    empty_at(TREE_DEPTH)
}

/// The proof that one record sits in one collection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct InclusionProof {
    /// The record key the proof covers.
    pub key: RecordKey,
    /// The stored bytes of the record.
    pub value: Vec<u8>,
    /// The slot the record key lands in.
    pub slot: u16,
    /// Every entry of the bucket, in key order.
    pub bucket: Vec<BucketEntry>,
    /// The sibling digest at each level, the leaf level first.
    pub siblings: Vec<[u8; 32]>,
}

impl InclusionProof {
    /// The root the proof folds to.
    pub fn fold(&self) -> Option<[u8; 32]> {
        if self.siblings.len() != TREE_DEPTH as usize {
            return None;
        }
        let digest = record_digest(self.key.as_slice(), &self.value);
        if !self
            .bucket
            .iter()
            .any(|(key, held)| key == &self.key && held == &digest)
        {
            return None;
        }
        let mut current = bucket_digest(&self.bucket);
        let mut index = self.slot;
        for sibling in self.siblings.iter() {
            current = if index & 1 == 0 {
                node_digest(&current, sibling)
            } else {
                node_digest(sibling, &current)
            };
            index >>= 1;
        }
        Some(current)
    }

    /// Report whether the proof folds to the given collection root.
    pub fn verifies(&self, root: &[u8; 32]) -> bool {
        self.fold().map(|held| &held == root).unwrap_or(false)
    }
}

/// Fold one proof and compare the result with a collection root.
pub fn verify(proof: &InclusionProof, root: &[u8; 32]) -> bool {
    proof.verifies(root)
}

/// The sibling index at one level for one slot.
pub fn sibling_index(slot: u16, level: u8) -> u16 {
    node_index(slot, level) ^ 1
}

/// The node index at one level for one slot.
///
/// The root level shifts every bit of the slot away, so the root sits at
/// index zero. A shift of sixteen places or more yields zero.
pub fn node_index(slot: u16, level: u8) -> u16 {
    slot.checked_shr(u32::from(level)).unwrap_or(0)
}

/// Place one entry in a bucket, keeping the bucket in key order.
///
/// A key already present takes the new digest. The result reports whether
/// the bucket changed.
pub fn place(bucket: &mut Vec<BucketEntry>, key: &RecordKey, digest: [u8; 32]) -> bool {
    match bucket.binary_search_by(|(held, _)| held.cmp(key)) {
        Ok(position) => {
            if bucket[position].1 == digest {
                return false;
            }
            bucket[position].1 = digest;
            true
        }
        Err(position) => {
            bucket.insert(position, (key.clone(), digest));
            true
        }
    }
}

/// Remove one entry from a bucket.
///
/// The result reports whether the bucket changed.
pub fn displace(bucket: &mut Vec<BucketEntry>, key: &RecordKey) -> bool {
    match bucket.binary_search_by(|(held, _)| held.cmp(key)) {
        Ok(position) => {
            bucket.remove(position);
            true
        }
        Err(_) => false,
    }
}

/// The sixteen sibling digests a proof carries, given a reader over the
/// stored nodes.
///
/// The reader answers with the stored digest of one node, or with nothing
/// when the node holds an empty subtree.
pub fn siblings_for(slot: u16, read: impl Fn(u8, u16) -> Option<[u8; 32]>) -> Vec<[u8; 32]> {
    let mut found = vec![[0u8; 32]; TREE_DEPTH as usize];
    let mut level = 0u8;
    while level < TREE_DEPTH {
        let index = sibling_index(slot, level);
        found[level as usize] = read(level, index).unwrap_or_else(|| empty_at(level));
        level = level.saturating_add(1);
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use frame_support::BoundedVec;

    fn key(text: &str) -> RecordKey {
        BoundedVec::try_from(text.as_bytes().to_vec()).expect("the key fits")
    }

    #[test]
    fn the_root_level_sits_at_index_zero_for_every_slot() {
        for slot in [0u16, 1, 2, 0x00ff, 0x8000, u16::MAX] {
            assert_eq!(node_index(slot, TREE_DEPTH), 0);
            assert_eq!(node_index(slot, 0), slot);
            assert_eq!(node_index(slot, TREE_DEPTH - 1), slot >> 15);
        }
    }

    #[test]
    fn a_sibling_differs_from_its_node_in_the_lowest_bit_alone() {
        for level in 0..TREE_DEPTH {
            let slot = 0b1010_1100_0011_0101u16;
            let node = node_index(slot, level);
            let sibling = sibling_index(slot, level);
            assert_eq!(node ^ sibling, 1);
            assert_eq!(node >> 1, sibling >> 1);
        }
    }

    #[test]
    fn an_empty_level_derives_from_the_level_below() {
        assert_eq!(empty_at(1), node_digest(&empty_at(0), &empty_at(0)));
        assert_eq!(empty_at(2), node_digest(&empty_at(1), &empty_at(1)));
        assert_ne!(empty_at(0), empty_at(TREE_DEPTH));
    }

    #[test]
    fn an_empty_bucket_takes_the_empty_leaf_digest() {
        assert_eq!(bucket_digest(&[]), empty_at(0));
    }

    #[test]
    fn a_record_digest_separates_the_key_from_the_value() {
        let left = record_digest(b"ab", b"c");
        let right = record_digest(b"a", b"bc");
        assert_ne!(left, right);
    }

    #[test]
    fn a_slot_comes_from_the_hash_of_the_key() {
        let digest = sp_crypto_hashing::blake2_256(b"12/3");
        assert_eq!(slot_of(b"12/3"), u16::from_be_bytes([digest[0], digest[1]]));
    }

    #[test]
    fn a_bucket_keeps_its_entries_in_key_order() {
        let mut bucket: Vec<BucketEntry> = Vec::new();
        assert!(place(&mut bucket, &key("b"), [1u8; 32]));
        assert!(place(&mut bucket, &key("a"), [2u8; 32]));
        assert_eq!(bucket[0].0, key("a"));
        assert_eq!(bucket[1].0, key("b"));
        assert!(!place(&mut bucket, &key("a"), [2u8; 32]));
        assert!(place(&mut bucket, &key("a"), [3u8; 32]));
        assert!(displace(&mut bucket, &key("a")));
        assert!(!displace(&mut bucket, &key("a")));
        assert_eq!(bucket.len(), 1);
    }

    #[test]
    fn a_proof_folds_to_the_root_the_tree_holds() {
        let record = key("12/3");
        let value = vec![7u8, 8, 9];
        let digest = record_digest(record.as_slice(), &value);
        let bucket = vec![(record.clone(), digest)];
        let slot = slot_of(record.as_slice());

        let mut current = bucket_digest(&bucket);
        let mut index = slot;
        let mut siblings = Vec::new();
        let mut level = 0u8;
        while level < TREE_DEPTH {
            let sibling = empty_at(level);
            siblings.push(sibling);
            current = if index & 1 == 0 {
                node_digest(&current, &sibling)
            } else {
                node_digest(&sibling, &current)
            };
            index >>= 1;
            level = level.saturating_add(1);
        }

        let proof = InclusionProof {
            key: record,
            value,
            slot,
            bucket,
            siblings,
        };
        assert!(proof.verifies(&current));
        assert!(!proof.verifies(&[0u8; 32]));
    }

    #[test]
    fn a_proof_carrying_another_value_declines() {
        let record = key("12/3");
        let value = vec![7u8];
        let digest = record_digest(record.as_slice(), &value);
        let bucket = vec![(record.clone(), digest)];
        let proof = InclusionProof {
            key: record,
            value: vec![8u8],
            slot: slot_of(b"12/3"),
            bucket,
            siblings: vec![[0u8; 32]; TREE_DEPTH as usize],
        };
        assert!(proof.fold().is_none());
    }

    #[test]
    fn a_proof_of_the_wrong_height_declines() {
        let record = key("12/3");
        let proof = InclusionProof {
            key: record,
            value: vec![1u8],
            slot: 0,
            bucket: Vec::new(),
            siblings: vec![[0u8; 32]; 3],
        };
        assert!(proof.fold().is_none());
    }
}
