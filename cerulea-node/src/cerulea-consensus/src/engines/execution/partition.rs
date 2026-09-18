//! The conflict domain partitioner.
//!
//! A conflict domain is a maximal set of pending transactions whose state
//! reaches never meet the state reaches of another domain. Domains execute on
//! their own cores and commit on their own.
//!
//! The partitioner builds a graph over the pending transactions. An edge joins
//! two transactions where a write of one meets a write or a read of the other,
//! inside one collection, over an overlapping key span. The connected
//! components of that graph become the domains.
//!
//! A disjoint set structure carries the components. Each transaction joins the
//! set once. Union and find both run in close to constant time, so the pass
//! costs time proportional to the number of declared accesses plus the number
//! of true conflicts. An exact key declaration costs one map lookup. A prefix
//! span costs a sorted lookup over the keys of that collection.

use std::collections::BTreeMap;

use crate::engines::manifest::DeclaresAccess;
use crate::engines::{CollectionId, ConflictDomain, KeyRange, TransactionClass};

/// A disjoint set over the pending transactions.
///
/// The structure answers whether two transactions already share a domain, and
/// joins two domains into one. Union by size keeps the trees shallow. Path
/// compression flattens a path on every lookup.
#[derive(Clone, Debug)]
pub struct DisjointSet {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl DisjointSet {
    /// Start a structure holding one member per pending transaction.
    pub fn new(members: usize) -> Self {
        DisjointSet {
            parent: (0..members).collect(),
            size: vec![1; members],
        }
    }

    /// The representative of the domain holding this member.
    pub fn find(&mut self, member: usize) -> usize {
        let mut root = member;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut walk = member;
        while self.parent[walk] != root {
            let next = self.parent[walk];
            self.parent[walk] = root;
            walk = next;
        }
        root
    }

    /// Join the domains holding two members.
    ///
    /// The call reports whether the two members sat in separate domains before.
    pub fn union(&mut self, left: usize, right: usize) -> bool {
        let mut left_root = self.find(left);
        let mut right_root = self.find(right);
        if left_root == right_root {
            return false;
        }
        if self.size[left_root] < self.size[right_root] {
            core::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        self.size[left_root] = self.size[left_root].saturating_add(self.size[right_root]);
        true
    }

    /// The number of members held.
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Report whether the structure holds zero members.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }
}

/// One declared access, reduced to the shape the partitioner reads.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Reach {
    member: usize,
    writes: bool,
}

/// Every access declared inside one collection.
#[derive(Clone, Debug, Default)]
struct CollectionReaches {
    exact: BTreeMap<Vec<u8>, Vec<Reach>>,
    prefix: Vec<(Vec<u8>, Reach)>,
    whole: Vec<Reach>,
}

/// The Execution Engine partitioner.
///
/// The partitioner holds no state between passes. Two nodes running the same
/// pending list produce the same domains.
#[derive(Clone, Copy, Debug, Default)]
pub struct DomainPartitioner;

impl DomainPartitioner {
    /// Build a partitioner.
    pub fn new() -> Self {
        DomainPartitioner
    }

    /// Group the pending transactions into conflict domains.
    ///
    /// The returned domains appear in the order of their lowest member
    /// position. Members inside a domain appear in position order. The
    /// ordering step settles the execution order afterwards.
    pub fn partition<T: DeclaresAccess>(&self, pending: &[T]) -> Vec<ConflictDomain> {
        let mut set = DisjointSet::new(pending.len());
        let reaches = collect_reaches(pending);
        for collection in reaches.values() {
            join_collection(collection, &mut set);
        }
        assemble(pending, &mut set)
    }
}

fn collect_reaches<T: DeclaresAccess>(pending: &[T]) -> BTreeMap<CollectionId, CollectionReaches> {
    let mut reaches: BTreeMap<CollectionId, CollectionReaches> = BTreeMap::new();
    for (member, transaction) in pending.iter().enumerate() {
        let manifest = transaction.manifest();
        for (accesses, writes) in [(&manifest.writes, true), (&manifest.reads, false)] {
            for access in accesses.iter() {
                let entry = reaches.entry(access.collection).or_default();
                let reach = Reach { member, writes };
                match &access.keys {
                    KeyRange::Exact(key) => {
                        entry.exact.entry(key.clone()).or_default().push(reach);
                    }
                    KeyRange::Prefix(prefix) => entry.prefix.push((prefix.clone(), reach)),
                    KeyRange::Full => entry.whole.push(reach),
                }
            }
        }
    }
    reaches
}

fn join_collection(collection: &CollectionReaches, set: &mut DisjointSet) {
    // A whole collection write reaches every record of the collection. Every
    // transaction touching the collection joins one domain.
    if collection.whole.iter().any(|reach| reach.writes) {
        let mut members = Vec::new();
        members.extend(collection.whole.iter().map(|reach| reach.member));
        members.extend(collection.prefix.iter().map(|(_, reach)| reach.member));
        for group in collection.exact.values() {
            members.extend(group.iter().map(|reach| reach.member));
        }
        join_all(&members, set);
        return;
    }

    // A group of transactions naming one exact key joins where one of them
    // writes that key.
    for group in collection.exact.values() {
        if group.iter().any(|reach| reach.writes) {
            join_all(
                &group.iter().map(|reach| reach.member).collect::<Vec<_>>(),
                set,
            );
        }
    }

    // A whole collection read meets every write of the collection.
    if !collection.whole.is_empty() {
        let readers: Vec<usize> = collection.whole.iter().map(|reach| reach.member).collect();
        let mut writers: Vec<usize> = collection
            .prefix
            .iter()
            .filter(|(_, reach)| reach.writes)
            .map(|(_, reach)| reach.member)
            .collect();
        for group in collection.exact.values() {
            writers.extend(
                group
                    .iter()
                    .filter(|reach| reach.writes)
                    .map(|reach| reach.member),
            );
        }
        for reader in &readers {
            for writer in &writers {
                set.union(*reader, *writer);
            }
        }
        join_all(&readers, set);
    }

    join_prefix_spans(collection, set);
}

fn join_prefix_spans(collection: &CollectionReaches, set: &mut DisjointSet) {
    if collection.prefix.is_empty() {
        return;
    }
    // A prefix span meets every exact key beneath it. The exact keys sit in a
    // sorted map, so the span reads as one contiguous run.
    for (prefix, reach) in &collection.prefix {
        for (key, group) in collection.exact.range(prefix.clone()..) {
            if !key.starts_with(prefix) {
                break;
            }
            for other in group {
                if reach.writes || other.writes {
                    set.union(reach.member, other.member);
                }
            }
        }
    }

    // Two prefix spans meet where one prefix starts the other. Sorting brings
    // every such pair next to each other, so one pass over a stack finds them.
    let mut spans: Vec<(Vec<u8>, Reach)> = collection.prefix.clone();
    spans.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(left.1.member.cmp(&right.1.member))
    });
    let mut stack: Vec<(Vec<u8>, Reach)> = Vec::new();
    for (prefix, reach) in spans {
        while let Some((top, _)) = stack.last() {
            if prefix.starts_with(top) {
                break;
            }
            stack.pop();
        }
        for (_, held) in stack.iter() {
            if reach.writes || held.writes {
                set.union(reach.member, held.member);
            }
        }
        stack.push((prefix, reach));
    }
}

fn join_all(members: &[usize], set: &mut DisjointSet) {
    let mut walk = members.iter();
    if let Some(first) = walk.next() {
        for member in walk {
            set.union(*first, *member);
        }
    }
}

fn assemble<T: DeclaresAccess>(pending: &[T], set: &mut DisjointSet) -> Vec<ConflictDomain> {
    let mut grouped: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for member in 0..pending.len() {
        let root = set.find(member);
        grouped.entry(root).or_default().push(member);
    }
    let mut domains: Vec<ConflictDomain> = grouped
        .into_values()
        .map(|members| {
            let highest_class = members
                .iter()
                .map(|member| pending[*member].class())
                .max()
                .unwrap_or(TransactionClass::Routine);
            ConflictDomain {
                members,
                highest_class,
            }
        })
        .collect();
    domains.sort_by_key(|domain| domain.members.first().copied().unwrap_or(usize::MAX));
    domains
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn holder() -> CollectionId {
        CollectionId::from_name("holder")
    }

    fn transaction(
        member: u8,
        class: TransactionClass,
        reads: Vec<CollectionAccess>,
        writes: Vec<CollectionAccess>,
    ) -> PendingTransaction {
        let manifest = AccessManifest::build(class, reads, writes).expect("the manifest builds");
        PendingTransaction::new([member; 32], member as u64, manifest, vec![member])
    }

    fn writes_key(member: u8, key: &[u8]) -> PendingTransaction {
        transaction(
            member,
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), key)],
        )
    }

    #[test]
    fn a_disjoint_set_joins_and_answers() {
        let mut set = DisjointSet::new(5);
        assert_eq!(set.len(), 5);
        assert!(!set.is_empty());
        assert!(set.union(0, 1));
        assert!(!set.union(1, 0));
        assert!(set.union(1, 2));
        assert_eq!(set.find(0), set.find(2));
        assert_ne!(set.find(0), set.find(3));
        assert!(DisjointSet::new(0).is_empty());
    }

    #[test]
    fn two_transactions_writing_one_key_share_a_domain() {
        let pending = vec![writes_key(1, b"parcel-1"), writes_key(2, b"parcel-1")];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].members, vec![0, 1]);
    }

    #[test]
    fn two_transactions_writing_two_keys_take_two_domains() {
        let pending = vec![writes_key(1, b"parcel-1"), writes_key(2, b"parcel-2")];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![0]);
        assert_eq!(domains[1].members, vec![1]);
    }

    #[test]
    fn two_collections_holding_the_same_key_stay_apart() {
        let pending = vec![
            writes_key(1, b"row-1"),
            transaction(
                2,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::exact(holder(), b"row-1")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
    }

    #[test]
    fn a_chain_of_conflicts_becomes_one_domain() {
        let pending = vec![
            writes_key(1, b"a"),
            transaction(
                2,
                TransactionClass::Routine,
                vec![],
                vec![
                    CollectionAccess::exact(parcel(), b"a"),
                    CollectionAccess::exact(parcel(), b"b"),
                ],
            ),
            writes_key(3, b"b"),
            writes_key(4, b"z"),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![0, 1, 2]);
        assert_eq!(domains[1].members, vec![3]);
    }

    #[test]
    fn a_write_that_meets_a_read_joins_one_domain() {
        let pending = vec![
            writes_key(1, b"parcel-1"),
            transaction(
                2,
                TransactionClass::Routine,
                vec![CollectionAccess::exact(parcel(), b"parcel-1")],
                vec![CollectionAccess::exact(holder(), b"holder-9")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 1);
    }

    #[test]
    fn two_reads_of_one_key_stay_apart() {
        let read_only = |member: u8| {
            transaction(
                member,
                TransactionClass::Routine,
                vec![CollectionAccess::exact(parcel(), b"parcel-1")],
                vec![CollectionAccess::exact(holder(), &[member])],
            )
        };
        let pending = vec![read_only(1), read_only(2)];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
    }

    #[test]
    fn a_prefix_write_joins_every_key_beneath_it() {
        let pending = vec![
            transaction(
                1,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::prefix(parcel(), b"north-")],
            ),
            writes_key(2, b"north-1"),
            writes_key(3, b"north-2"),
            writes_key(4, b"south-1"),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![0, 1, 2]);
        assert_eq!(domains[1].members, vec![3]);
    }

    #[test]
    fn two_prefix_writes_that_nest_join_one_domain() {
        let pending = vec![
            transaction(
                1,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::prefix(parcel(), b"north-")],
            ),
            transaction(
                2,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::prefix(parcel(), b"north-east-")],
            ),
            transaction(
                3,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::prefix(parcel(), b"south-")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![0, 1]);
        assert_eq!(domains[1].members, vec![2]);
    }

    #[test]
    fn a_whole_collection_write_joins_every_transaction_touching_it() {
        let pending = vec![
            writes_key(1, b"parcel-1"),
            transaction(
                2,
                TransactionClass::Structural,
                vec![],
                vec![CollectionAccess::full(parcel())],
            ),
            writes_key(3, b"parcel-9"),
            transaction(
                4,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::exact(holder(), b"holder-1")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![0, 1, 2]);
        assert_eq!(domains[1].members, vec![3]);
        assert_eq!(domains[0].highest_class, TransactionClass::Structural);
    }

    #[test]
    fn a_whole_collection_read_meets_every_write_of_that_collection() {
        let pending = vec![
            writes_key(1, b"parcel-1"),
            transaction(
                2,
                TransactionClass::Routine,
                vec![CollectionAccess::full(parcel())],
                vec![CollectionAccess::exact(holder(), b"holder-2")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 1);
    }

    #[test]
    fn a_domain_carries_the_highest_class_present() {
        let pending = vec![
            transaction(
                1,
                TransactionClass::Routine,
                vec![],
                vec![CollectionAccess::exact(parcel(), b"k")],
            ),
            transaction(
                2,
                TransactionClass::Constitutional,
                vec![],
                vec![CollectionAccess::exact(parcel(), b"k")],
            ),
        ];
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 1);
        assert_eq!(domains[0].highest_class, TransactionClass::Constitutional);
    }

    #[test]
    fn an_empty_pending_list_produces_no_domain() {
        let pending: Vec<PendingTransaction> = Vec::new();
        assert!(DomainPartitioner::new().partition(&pending).is_empty());
    }

    #[test]
    fn every_transaction_lands_in_exactly_one_domain() {
        let pending: Vec<PendingTransaction> = (0..64u8)
            .map(|index| writes_key(index, format!("key-{}", index % 16).as_bytes()))
            .collect();
        let domains = DomainPartitioner::new().partition(&pending);
        assert_eq!(domains.len(), 16);
        let mut seen: Vec<usize> = domains
            .iter()
            .flat_map(|domain| domain.members.clone())
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..64).collect::<Vec<_>>());
        for domain in &domains {
            assert_eq!(domain.len(), 4);
            assert!(!domain.is_empty());
        }
    }

    #[test]
    fn the_partition_repeats_for_every_node() {
        let pending = vec![
            writes_key(1, b"a"),
            writes_key(2, b"b"),
            writes_key(3, b"a"),
            transaction(
                4,
                TransactionClass::Financial,
                vec![CollectionAccess::exact(parcel(), b"b")],
                vec![CollectionAccess::prefix(holder(), b"h-")],
            ),
        ];
        let first = DomainPartitioner::new().partition(&pending);
        let second = DomainPartitioner::new().partition(&pending);
        assert_eq!(first, second);
        assert_eq!(first[0].members, vec![0, 2]);
        assert_eq!(first[1].members, vec![1, 3]);
    }
}
