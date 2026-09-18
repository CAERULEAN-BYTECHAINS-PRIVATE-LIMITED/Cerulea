//! The per-domain state root and the combined root.
//!
//! A state root is a cryptographic summary of the state after execution. The
//! Execution Engine produces one root per conflict domain plus one combined
//! root. The block header carries both in a digest item.
//!
//! Each domain root is the Merkle root over the records that domain changed. A
//! Merkle root is the top of a tree of hashes. Every record sits at a leaf, so
//! a later inclusion proof shows one record belongs to the domain without
//! sending the whole domain.
//!
//! The combined root is the Merkle root over the domain roots in domain order.
//! Two nodes running the same block reach the same domain roots, so they reach
//! the same combined root.

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::generic::{Digest, DigestItem};

use crate::engines::execution::runner::{DomainOutcome, ExecutionReport};
use crate::engines::CollectionId;
use crate::CERULEA_ENGINE_ID;

/// The separator mixed into the hash of an empty tree.
pub const EMPTY_DOMAIN: &[u8] = b"cerulea.roots.empty.v1";

/// The separator mixed into the hash of one record.
pub const LEAF_DOMAIN: &[u8] = b"cerulea.roots.leaf.v1";

/// The separator mixed into the hash of one tree node.
pub const NODE_DOMAIN: &[u8] = b"cerulea.roots.node.v1";

/// A state root.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct StateRoot(pub [u8; 32]);

impl StateRoot {
    /// Read the root as bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The root of a tree holding zero records.
pub fn empty_root() -> StateRoot {
    StateRoot(sp_core::hashing::blake2_256(EMPTY_DOMAIN))
}

/// The hash of one changed record.
///
/// A record that the domain removed hashes under a separate tag, so a removal
/// and a write of empty bytes take two different hashes.
pub fn leaf_hash(collection: &CollectionId, key: &[u8], value: Option<&[u8]>) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(LEAF_DOMAIN.len() + 64 + key.len());
    preimage.extend_from_slice(LEAF_DOMAIN);
    preimage.extend_from_slice(collection.as_bytes());
    preimage.extend_from_slice(&(key.len() as u32).to_be_bytes());
    preimage.extend_from_slice(key);
    match value {
        Some(body) => {
            preimage.push(1);
            preimage.extend_from_slice(&(body.len() as u32).to_be_bytes());
            preimage.extend_from_slice(body);
        }
        None => preimage.push(0),
    }
    sp_core::hashing::blake2_256(&preimage)
}

/// The hash of one tree node.
pub fn node_hash(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(NODE_DOMAIN.len() + 64);
    preimage.extend_from_slice(NODE_DOMAIN);
    preimage.extend_from_slice(left);
    preimage.extend_from_slice(right);
    sp_core::hashing::blake2_256(&preimage)
}

/// The Merkle root over a list of leaf hashes.
///
/// A level holding an odd count carries the last hash up unchanged. An empty
/// list takes the empty root.
pub fn merkle_root(leaves: &[[u8; 32]]) -> StateRoot {
    if leaves.is_empty() {
        return empty_root();
    }
    let mut level: Vec<[u8; 32]> = leaves.to_vec();
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        let mut index = 0;
        while index < level.len() {
            if index + 1 < level.len() {
                next.push(node_hash(&level[index], &level[index + 1]));
            } else {
                next.push(level[index]);
            }
            index += 2;
        }
        level = next;
    }
    StateRoot(level[0])
}

/// The state root of one domain.
///
/// The root summarises the records the domain changed. The changes read back in
/// key order, so every node builds the same tree.
pub fn domain_root(outcome: &DomainOutcome) -> StateRoot {
    let leaves: Vec<[u8; 32]> = outcome
        .changes
        .iter()
        .map(|(collection, key, value)| leaf_hash(collection, key, value.as_deref()))
        .collect();
    merkle_root(&leaves)
}

/// The per-domain roots and the combined root of one block.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo, Default)]
pub struct StateRoots {
    /// One root per domain, in domain order.
    pub domains: Vec<StateRoot>,
    /// The root over the domain roots.
    pub combined: StateRoot,
}

impl StateRoots {
    /// Build the roots from a list of domain roots.
    pub fn from_domain_roots(domains: Vec<StateRoot>) -> Self {
        let leaves: Vec<[u8; 32]> = domains.iter().map(|root| root.0).collect();
        let combined = merkle_root(&leaves);
        StateRoots { domains, combined }
    }

    /// Build the roots from an execution report.
    pub fn from_report(report: &ExecutionReport) -> Self {
        let domains = report.domains.iter().map(domain_root).collect();
        StateRoots::from_domain_roots(domains)
    }

    /// Report whether the combined root matches the domain roots.
    pub fn consistent(&self) -> bool {
        let leaves: Vec<[u8; 32]> = self.domains.iter().map(|root| root.0).collect();
        merkle_root(&leaves) == self.combined
    }

    /// The number of domains the block carried.
    pub fn len(&self) -> usize {
        self.domains.len()
    }

    /// Report whether the block carried zero domains.
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }

    /// The digest item the block header carries.
    pub fn digest_item(&self) -> DigestItem {
        DigestItem::Consensus(CERULEA_ENGINE_ID, self.encode())
    }

    /// Read the roots out of a block header digest.
    ///
    /// The call reports nothing where the header carries no Cerulea state root
    /// item.
    pub fn from_digest(digest: &Digest) -> Option<StateRoots> {
        digest.logs().iter().find_map(|item| match item {
            DigestItem::Consensus(engine, payload) if *engine == CERULEA_ENGINE_ID => {
                StateRoots::decode(&mut &payload[..]).ok()
            }
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::execution::partition::DomainPartitioner;
    use crate::engines::execution::runner::{DomainRunner, DomainStore};
    use crate::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
    use crate::engines::manifest_guard::{ManifestGuard, MemoryStateStore};
    use crate::engines::{EngineError, ExecutionEngine, KeyRange, TransactionClass};

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn holder() -> CollectionId {
        CollectionId::from_name("holder")
    }

    fn writer(seed: u8, collection: CollectionId, key: &[u8]) -> PendingTransaction {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(collection, key)],
        )
        .expect("the manifest builds");
        PendingTransaction::new([seed; 32], seed as u64, manifest, key.to_vec())
    }

    fn write_body(
        transaction: &PendingTransaction,
        guard: &mut ManifestGuard<'_, DomainStore<'_>>,
    ) -> Result<(), EngineError> {
        let access = transaction
            .manifest
            .writes
            .first()
            .expect("one write declared");
        let key = match &access.keys {
            KeyRange::Exact(key) => key.clone(),
            _ => Vec::new(),
        };
        guard.write(&access.collection, &key, vec![transaction.hash[0]])?;
        Ok(())
    }

    fn run(workers: usize, pending: &[PendingTransaction]) -> StateRoots {
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), pending);
        let mut state = MemoryStateStore::new();
        let report =
            DomainRunner::with_workers(workers).run(pending, &domains, &mut state, &write_body);
        StateRoots::from_report(&report)
    }

    #[test]
    fn two_disjoint_domains_produce_identical_roots_on_every_node() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, holder(), b"holder-1"),
            writer(3, parcel(), b"parcel-2"),
        ];
        let serial = run(1, &pending);
        let parallel = run(8, &pending);
        assert_eq!(serial, parallel);
        assert_eq!(serial.len(), 3);
        assert!(serial.consistent());
    }

    #[test]
    fn a_block_carries_one_root_per_domain_plus_a_combined_root() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, holder(), b"holder-1"),
        ];
        let roots = run(4, &pending);
        assert_eq!(roots.domains.len(), 2);
        assert_ne!(roots.domains[0], roots.domains[1]);
        assert_ne!(roots.combined, empty_root());
        assert!(roots.consistent());
    }

    #[test]
    fn a_changed_record_changes_the_domain_root() {
        let first = run(1, &[writer(1, parcel(), b"parcel-1")]);
        let second = run(1, &[writer(2, parcel(), b"parcel-1")]);
        assert_ne!(first.domains[0], second.domains[0]);
        assert_ne!(first.combined, second.combined);
    }

    #[test]
    fn the_domain_order_reaches_the_combined_root() {
        let one = StateRoot([1u8; 32]);
        let two = StateRoot([2u8; 32]);
        let forward = StateRoots::from_domain_roots(vec![one, two]);
        let backward = StateRoots::from_domain_roots(vec![two, one]);
        assert_ne!(forward.combined, backward.combined);
    }

    #[test]
    fn an_empty_block_takes_the_empty_root() {
        let roots = StateRoots::from_domain_roots(Vec::new());
        assert_eq!(roots.combined, empty_root());
        assert!(roots.is_empty());
        assert!(roots.consistent());
    }

    #[test]
    fn a_removal_and_an_empty_write_take_two_leaf_hashes() {
        let removed = leaf_hash(&parcel(), b"k", None);
        let emptied = leaf_hash(&parcel(), b"k", Some(&[]));
        assert_ne!(removed, emptied);
    }

    #[test]
    fn two_keys_of_one_collection_take_two_leaf_hashes() {
        assert_ne!(
            leaf_hash(&parcel(), b"a", Some(b"v")),
            leaf_hash(&parcel(), b"b", Some(b"v"))
        );
        assert_ne!(
            leaf_hash(&parcel(), b"a", Some(b"v")),
            leaf_hash(&holder(), b"a", Some(b"v"))
        );
    }

    #[test]
    fn a_key_boundary_never_shifts_into_the_value() {
        // The length prefix keeps two splits of the same bytes apart.
        assert_ne!(
            leaf_hash(&parcel(), b"ab", Some(b"c")),
            leaf_hash(&parcel(), b"a", Some(b"bc"))
        );
    }

    #[test]
    fn the_merkle_root_handles_an_odd_level() {
        let leaves = [[1u8; 32], [2u8; 32], [3u8; 32]];
        let root = merkle_root(&leaves);
        let expected = node_hash(&node_hash(&leaves[0], &leaves[1]), &leaves[2]);
        assert_eq!(root, StateRoot(expected));
    }

    #[test]
    fn one_leaf_is_its_own_root() {
        assert_eq!(merkle_root(&[[7u8; 32]]), StateRoot([7u8; 32]));
    }

    #[test]
    fn the_roots_round_trip_through_the_block_header_digest() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, holder(), b"holder-1"),
        ];
        let roots = run(2, &pending);
        let mut digest = Digest::default();
        digest.push(roots.digest_item());
        let read = StateRoots::from_digest(&digest).expect("the header carries the roots");
        assert_eq!(read, roots);
        assert!(read.consistent());
    }

    #[test]
    fn a_header_without_the_item_reports_nothing() {
        let digest = Digest::default();
        assert!(StateRoots::from_digest(&digest).is_none());
    }

    #[test]
    fn a_tampered_combined_root_fails_the_check() {
        let mut roots = StateRoots::from_domain_roots(vec![StateRoot([1u8; 32])]);
        assert!(roots.consistent());
        roots.combined = StateRoot([9u8; 32]);
        assert!(!roots.consistent());
    }

    #[test]
    fn a_state_root_reads_back_its_bytes() {
        let root = StateRoot([4u8; 32]);
        assert_eq!(root.as_bytes(), &[4u8; 32]);
    }
}
