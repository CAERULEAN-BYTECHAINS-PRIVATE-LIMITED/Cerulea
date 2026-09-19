//! The Merkle Patricia trie walk.
//!
//! A network running Ethereum Virtual Machine bytecode stores its accounts
//! and its receipts in a trie. A proof is the list of trie nodes on the
//! path from the root to one key. The walk here recomputes the hash of
//! every node it takes and compares the hash with the reference the parent
//! node held. A node that fails its own hash stops the walk.
//!
//! # The node shapes
//!
//! A branch node is a list of seventeen items. The first sixteen items
//! reference the child under each nibble. The seventeenth item holds the
//! value of a key that ends at the branch.
//!
//! A leaf node and an extension node are both lists of two items. The
//! first item holds the path segment behind a prefix nibble. A prefix of
//! zero or one marks an extension, which continues to a child. A prefix of
//! two or three marks a leaf, which ends at a value. An odd prefix carries
//! one path nibble in the same byte as the prefix.
//!
//! # The child reference
//!
//! A child of thirty two bytes or more sits behind its hash. A shorter
//! child sits inside the parent as a nested list. The walk handles both.

use crate::envelope::BridgeFault;
use crate::rlp::{self, Item};
use alloc::{collections::BTreeMap, vec::Vec};

/// The longest path the walk follows.
///
/// A key of thirty two bytes carries sixty four nibbles. A walk therefore
/// takes at most sixty five nodes. A limit of eighty leaves room for the
/// account trie and the storage trie of one lookup.
pub const MAX_STEPS: usize = 80;

/// The nibbles of one key, the high nibble of each byte first.
pub fn nibbles(key: &[u8]) -> Vec<u8> {
    let mut found = Vec::with_capacity(key.len() * 2);
    for byte in key.iter() {
        found.push(byte >> 4);
        found.push(byte & 0x0f);
    }
    found
}

/// The path nibbles a leaf node or an extension node holds.
///
/// The answer carries the nibbles and reports whether the node is a leaf.
fn path_of(encoded: &[u8]) -> Result<(Vec<u8>, bool), BridgeFault> {
    let first = *encoded.first().ok_or(BridgeFault::MalformedEncoding)?;
    let prefix = first >> 4;
    if prefix > 3 {
        return Err(BridgeFault::MalformedEncoding);
    }
    let leaf = prefix >= 2;
    let odd = prefix & 1 == 1;
    let mut found: Vec<u8> = Vec::with_capacity(encoded.len() * 2);
    if odd {
        found.push(first & 0x0f);
    } else if first & 0x0f != 0 {
        return Err(BridgeFault::MalformedEncoding);
    }
    for byte in encoded.iter().skip(1) {
        found.push(byte >> 4);
        found.push(byte & 0x0f);
    }
    Ok((found, leaf))
}

/// The nodes one proof carries, keyed by the hash of each node.
struct NodeSet {
    held: BTreeMap<[u8; 32], Item>,
}

impl NodeSet {
    /// Decode every node of a proof and key each by its hash.
    fn build(nodes: &[Vec<u8>]) -> Result<Self, BridgeFault> {
        let mut held: BTreeMap<[u8; 32], Item> = BTreeMap::new();
        for raw in nodes.iter() {
            let item = rlp::decode(raw).map_err(|_| BridgeFault::MalformedEncoding)?;
            held.insert(sp_crypto_hashing::keccak_256(raw), item);
        }
        Ok(NodeSet { held })
    }

    /// The node under one hash.
    fn get(&self, hash: &[u8; 32]) -> Option<&Item> {
        self.held.get(hash)
    }
}

/// One reference to a child node.
enum Reference<'a> {
    /// The child sits behind its hash.
    Hashed([u8; 32]),
    /// The child sits inside the parent.
    Embedded(&'a Item),
    /// The parent holds no child here.
    Absent,
}

/// Read the reference one item holds.
fn reference_of(item: &Item) -> Result<Reference<'_>, BridgeFault> {
    match item {
        Item::List(_) => Ok(Reference::Embedded(item)),
        Item::Bytes(held) => {
            if held.is_empty() {
                Ok(Reference::Absent)
            } else if held.len() == 32 {
                let mut hash = [0u8; 32];
                hash.copy_from_slice(held);
                Ok(Reference::Hashed(hash))
            } else {
                Err(BridgeFault::MalformedEncoding)
            }
        }
    }
}

/// Walk one trie proof and answer with the value the key reaches.
///
/// The walk starts at the root hash and follows the nibbles of the key. A
/// key the trie lacks answers with nothing. A node that fails its own hash
/// or a node the proof omits raises a fault naming the fix.
pub fn walk(
    root: &[u8; 32],
    key: &[u8],
    nodes: &[Vec<u8>],
) -> Result<Option<Vec<u8>>, BridgeFault> {
    if nodes.len() > MAX_STEPS {
        return Err(BridgeFault::ProofTooLarge);
    }
    let set = NodeSet::build(nodes)?;
    let path = nibbles(key);
    let mut cursor = 0usize;
    let mut step = 0usize;
    let mut current: Item = match set.get(root) {
        Some(held) => held.clone(),
        None => return Err(BridgeFault::NodeHashMismatch),
    };

    loop {
        step = step.saturating_add(1);
        if step > MAX_STEPS {
            return Err(BridgeFault::ProofTooLarge);
        }
        let items = current.list().map_err(|_| BridgeFault::MalformedEncoding)?;
        let next: Reference<'_> = match items.len() {
            17 => {
                if cursor == path.len() {
                    let value = items[16]
                        .bytes()
                        .map_err(|_| BridgeFault::MalformedEncoding)?;
                    return Ok(if value.is_empty() {
                        None
                    } else {
                        Some(value.to_vec())
                    });
                }
                let nibble = path[cursor] as usize;
                cursor = cursor.saturating_add(1);
                reference_of(&items[nibble])?
            }
            2 => {
                let encoded = items[0]
                    .bytes()
                    .map_err(|_| BridgeFault::MalformedEncoding)?;
                let (segment, leaf) = path_of(encoded)?;
                let remaining = &path[cursor.min(path.len())..];
                if leaf {
                    if remaining != segment.as_slice() {
                        return Ok(None);
                    }
                    let value = items[1]
                        .bytes()
                        .map_err(|_| BridgeFault::MalformedEncoding)?;
                    return Ok(Some(value.to_vec()));
                }
                if remaining.len() < segment.len() || remaining[..segment.len()] != segment[..] {
                    return Ok(None);
                }
                cursor = cursor.saturating_add(segment.len());
                reference_of(&items[1])?
            }
            _ => return Err(BridgeFault::MalformedEncoding),
        };

        current = match next {
            Reference::Absent => return Ok(None),
            Reference::Embedded(held) => held.clone(),
            Reference::Hashed(hash) => match set.get(&hash) {
                Some(held) => held.clone(),
                None => return Err(BridgeFault::NodeHashMismatch),
            },
        };
    }
}

/// Walk one trie proof and require the key to reach a value.
pub fn require(root: &[u8; 32], key: &[u8], nodes: &[Vec<u8>]) -> Result<Vec<u8>, BridgeFault> {
    walk(root, key, nodes)?.ok_or(BridgeFault::ValueAbsent)
}

/// The account fields one account trie value carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Account {
    /// The count of transactions the account sent.
    pub nonce: u64,
    /// The root of the storage trie of the account.
    pub storage_root: [u8; 32],
    /// The hash of the code the account holds.
    pub code_hash: [u8; 32],
}

/// Read the account fields of one account trie value.
///
/// The value is a list of four items. The first item is the nonce. The
/// second item is the balance. The third item is the storage root. The
/// fourth item is the code hash. The bridge reads the storage root and the
/// code hash.
pub fn account_of(encoded: &[u8]) -> Result<Account, BridgeFault> {
    let item = rlp::decode(encoded).map_err(|_| BridgeFault::MalformedEncoding)?;
    let held = item.list().map_err(|_| BridgeFault::MalformedEncoding)?;
    if held.len() != 4 {
        return Err(BridgeFault::MalformedEncoding);
    }
    Ok(Account {
        nonce: item
            .number_at(0)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
        storage_root: item
            .hash_at(2)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
        code_hash: item
            .hash_at(3)
            .map_err(|_| BridgeFault::MalformedEncoding)?,
    })
}

/// The thirty two byte word one storage trie value carries.
///
/// A storage value sits in the trie as a byte string with no leading zero
/// byte. The reader pads the value back to a full word.
pub fn word_of(encoded: &[u8]) -> Result<[u8; 32], BridgeFault> {
    let item = rlp::decode(encoded).map_err(|_| BridgeFault::MalformedEncoding)?;
    let held = item.bytes().map_err(|_| BridgeFault::MalformedEncoding)?;
    if held.len() > 32 {
        return Err(BridgeFault::MalformedEncoding);
    }
    let mut found = [0u8; 32];
    found[32 - held.len()..].copy_from_slice(held);
    Ok(found)
}

/// The storage slot one mapping entry sits at.
///
/// A mapping in Ethereum Virtual Machine storage places the entry for one
/// key at the hash of the key word followed by the base slot word. The
/// bridge maps a message identifier to its slot through this rule.
pub fn mapping_slot(key: &[u8; 32], base: &[u8; 32]) -> [u8; 32] {
    let mut held = [0u8; 64];
    held[..32].copy_from_slice(key);
    held[32..].copy_from_slice(base);
    sp_crypto_hashing::keccak_256(&held)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn nibbles_split_every_byte() {
        assert_eq!(nibbles(&[0xab, 0x0c]), vec![0x0a, 0x0b, 0x00, 0x0c]);
    }

    #[test]
    fn an_even_leaf_path_drops_the_prefix_byte() {
        let (path, leaf) = path_of(&[0x20, 0xab]).expect("the path reads");
        assert!(leaf);
        assert_eq!(path, vec![0x0a, 0x0b]);
    }

    #[test]
    fn an_odd_leaf_path_keeps_the_first_nibble() {
        let (path, leaf) = path_of(&[0x3a, 0xbc]).expect("the path reads");
        assert!(leaf);
        assert_eq!(path, vec![0x0a, 0x0b, 0x0c]);
    }

    #[test]
    fn an_even_extension_path_reads_back() {
        let (path, leaf) = path_of(&[0x00, 0x12]).expect("the path reads");
        assert!(!leaf);
        assert_eq!(path, vec![0x01, 0x02]);
    }

    #[test]
    fn an_even_path_carrying_a_filled_low_nibble_declines() {
        assert_eq!(path_of(&[0x21, 0xab]), Err(BridgeFault::MalformedEncoding));
    }

    #[test]
    fn a_single_leaf_trie_answers_with_its_value() {
        let leaf = rlp::encode_list(&[
            rlp::encode_bytes(&[0x20, 0xab, 0xcd]),
            rlp::encode_bytes(b"value"),
        ]);
        let root = sp_crypto_hashing::keccak_256(&leaf);
        let found = walk(&root, &[0xab, 0xcd], &[leaf]).expect("the walk runs");
        assert_eq!(found, Some(b"value".to_vec()));
    }

    #[test]
    fn a_missing_node_raises_a_hash_mismatch() {
        let root = [3u8; 32];
        assert_eq!(
            walk(&root, &[0x01], &[]),
            Err(BridgeFault::NodeHashMismatch)
        );
    }

    #[test]
    fn a_key_the_trie_lacks_answers_with_nothing() {
        let leaf = rlp::encode_list(&[
            rlp::encode_bytes(&[0x20, 0xab, 0xcd]),
            rlp::encode_bytes(b"value"),
        ]);
        let root = sp_crypto_hashing::keccak_256(&leaf);
        let found = walk(&root, &[0xab, 0xce], &[leaf]).expect("the walk runs");
        assert_eq!(found, None);
    }

    #[test]
    fn a_branch_node_follows_the_nibble() {
        let leaf = rlp::encode_list(&[
            rlp::encode_bytes(&[0x37, 0x77, 0x77]),
            rlp::encode_bytes(b"deep"),
        ]);
        let leaf_hash = sp_crypto_hashing::keccak_256(&leaf);
        let mut items: Vec<Vec<u8>> = (0..17).map(|_| rlp::encode_bytes(&[])).collect();
        items[1] = rlp::encode_bytes(&leaf_hash);
        let branch = rlp::encode_list(&items);
        let root = sp_crypto_hashing::keccak_256(&branch);
        let found = walk(&root, &[0x17, 0x77, 0x77], &[branch, leaf]).expect("the walk runs");
        assert_eq!(found, Some(b"deep".to_vec()));
    }

    #[test]
    fn a_branch_node_holding_the_value_answers_at_the_path_end() {
        let mut items: Vec<Vec<u8>> = (0..17).map(|_| rlp::encode_bytes(&[])).collect();
        items[16] = rlp::encode_bytes(b"here");
        let branch = rlp::encode_list(&items);
        let root = sp_crypto_hashing::keccak_256(&branch);
        let found = walk(&root, &[], &[branch]).expect("the walk runs");
        assert_eq!(found, Some(b"here".to_vec()));
    }

    #[test]
    fn an_extension_node_consumes_its_segment() {
        let leaf =
            rlp::encode_list(&[rlp::encode_bytes(&[0x20, 0xcd]), rlp::encode_bytes(b"tail")]);
        let leaf_hash = sp_crypto_hashing::keccak_256(&leaf);
        let extension = rlp::encode_list(&[
            rlp::encode_bytes(&[0x00, 0xab]),
            rlp::encode_bytes(&leaf_hash),
        ]);
        let root = sp_crypto_hashing::keccak_256(&extension);
        let found = walk(&root, &[0xab, 0xcd], &[extension, leaf]).expect("the walk runs");
        assert_eq!(found, Some(b"tail".to_vec()));
    }

    #[test]
    fn a_storage_word_pads_back_to_a_full_word() {
        let encoded = rlp::encode_bytes(&[0x01, 0x02]);
        let word = word_of(&encoded).expect("the word reads");
        assert_eq!(word[30], 0x01);
        assert_eq!(word[31], 0x02);
        assert_eq!(word[0], 0x00);
    }

    #[test]
    fn a_mapping_slot_follows_the_key_and_the_base() {
        let key = [1u8; 32];
        let base = [2u8; 32];
        let mut preimage = [0u8; 64];
        preimage[..32].copy_from_slice(&key);
        preimage[32..].copy_from_slice(&base);
        assert_eq!(
            mapping_slot(&key, &base),
            sp_crypto_hashing::keccak_256(&preimage)
        );
    }

    #[test]
    fn an_account_value_reads_its_storage_root() {
        let encoded = rlp::encode_list(&[
            rlp::encode_number(3),
            rlp::encode_number(1_000),
            rlp::encode_bytes(&[5u8; 32]),
            rlp::encode_bytes(&[6u8; 32]),
        ]);
        let account = account_of(&encoded).expect("the account reads");
        assert_eq!(account.nonce, 3);
        assert_eq!(account.storage_root, [5u8; 32]);
        assert_eq!(account.code_hash, [6u8; 32]);
    }
}
