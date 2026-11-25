//! Deterministic ordering inside a conflict domain.
//!
//! Domains run in parallel. The order inside one domain settles before the
//! domain runs. Three keys settle it.
//!
//! 1. **Class, descending.** A constitutional transaction runs ahead of a
//!    structural one. A structural transaction runs ahead of a financial one. A
//!    financial transaction runs ahead of a routine one. The heaviest work
//!    lands first, so a domain that runs short of room still carries the weight
//!    that matters.
//! 2. **Arrival sequence, ascending.** Two transactions of one class run in the
//!    order the pool received them.
//! 3. **Transaction hash, ascending.** Two transactions matching on class and
//!    arrival run in hash order.
//!
//! Every validator holds the same three values for every transaction, so every
//! validator reproduces the same order from the same block.

use core::cmp::Reverse;

use crate::engines::manifest::DeclaresAccess;
use crate::engines::{ConflictDomain, Engine, EngineKind, ExecutionEngine};

use super::partition::DomainPartitioner;

/// The sort key one transaction carries.
///
/// The key renders the three ordering rules as one comparable value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderKey {
    /// The class, reversed so the heaviest class sorts first.
    pub class: Reverse<u8>,
    /// The position the transaction took in the arrival queue.
    pub arrival: u64,
    /// The hash of the transaction.
    pub hash: [u8; 32],
}

impl OrderKey {
    /// Read the sort key of one transaction.
    pub fn of<T: DeclaresAccess>(transaction: &T) -> Self {
        OrderKey {
            class: Reverse(transaction.class() as u8),
            arrival: transaction.arrival_sequence(),
            hash: transaction.transaction_hash(),
        }
    }
}

/// Settle the execution order of one domain.
///
/// The returned list holds the same members in execution order.
pub fn order_members<T: DeclaresAccess>(pending: &[T], members: &[usize]) -> Vec<usize> {
    let mut ordered: Vec<usize> = members.to_vec();
    ordered.sort_by_key(|member| OrderKey::of(&pending[*member]));
    ordered
}

/// Settle the execution order of every domain in place.
pub fn order_domains<T: DeclaresAccess>(pending: &[T], domains: &mut [ConflictDomain]) {
    for domain in domains.iter_mut() {
        domain.members = order_members(pending, &domain.members);
    }
}

impl Engine for DomainPartitioner {
    const KIND: EngineKind = EngineKind::Execution;
}

impl ExecutionEngine for DomainPartitioner {
    fn partition<T: DeclaresAccess>(&self, pending: &[T]) -> Vec<ConflictDomain> {
        let mut domains = self.partition_domains(pending);
        order_domains(pending, &mut domains);
        domains
    }

    fn order<T: DeclaresAccess>(&self, pending: &[T], members: &[usize]) -> Vec<usize> {
        order_members(pending, members)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
    use crate::engines::{CollectionId, TransactionClass};

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn pending_transaction(
        hash_seed: u8,
        arrival: u64,
        class: TransactionClass,
        key: &[u8],
    ) -> PendingTransaction {
        let manifest =
            AccessManifest::build(class, vec![], vec![CollectionAccess::exact(parcel(), key)])
                .expect("the manifest builds");
        PendingTransaction::new([hash_seed; 32], arrival, manifest, vec![hash_seed])
    }

    #[test]
    fn the_heaviest_class_runs_first() {
        let pending = vec![
            pending_transaction(1, 1, TransactionClass::Routine, b"k"),
            pending_transaction(2, 2, TransactionClass::Constitutional, b"k"),
            pending_transaction(3, 3, TransactionClass::Financial, b"k"),
            pending_transaction(4, 4, TransactionClass::Structural, b"k"),
        ];
        let order = order_members(&pending, &[0, 1, 2, 3]);
        assert_eq!(order, vec![1, 3, 2, 0]);
    }

    #[test]
    fn one_class_runs_in_arrival_order() {
        let pending = vec![
            pending_transaction(9, 30, TransactionClass::Routine, b"k"),
            pending_transaction(8, 10, TransactionClass::Routine, b"k"),
            pending_transaction(7, 20, TransactionClass::Routine, b"k"),
        ];
        let order = order_members(&pending, &[0, 1, 2]);
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn one_arrival_position_runs_in_hash_order() {
        let pending = vec![
            pending_transaction(9, 5, TransactionClass::Routine, b"k"),
            pending_transaction(2, 5, TransactionClass::Routine, b"k"),
            pending_transaction(5, 5, TransactionClass::Routine, b"k"),
        ];
        let order = order_members(&pending, &[0, 1, 2]);
        assert_eq!(order, vec![1, 2, 0]);
    }

    #[test]
    fn the_class_rule_outranks_the_arrival_rule() {
        let pending = vec![
            pending_transaction(1, 1, TransactionClass::Routine, b"k"),
            pending_transaction(2, 99, TransactionClass::Financial, b"k"),
        ];
        assert_eq!(order_members(&pending, &[0, 1]), vec![1, 0]);
    }

    #[test]
    fn the_arrival_rule_outranks_the_hash_rule() {
        let pending = vec![
            pending_transaction(1, 9, TransactionClass::Routine, b"k"),
            pending_transaction(9, 1, TransactionClass::Routine, b"k"),
        ];
        assert_eq!(order_members(&pending, &[0, 1]), vec![1, 0]);
    }

    #[test]
    fn the_order_repeats_for_every_node() {
        let pending: Vec<PendingTransaction> = (0..40u8)
            .map(|index| {
                let class = match index % 4 {
                    0 => TransactionClass::Routine,
                    1 => TransactionClass::Financial,
                    2 => TransactionClass::Structural,
                    _ => TransactionClass::Constitutional,
                };
                pending_transaction(index, (index as u64 * 7) % 13, class, b"k")
            })
            .collect();
        let members: Vec<usize> = (0..pending.len()).collect();
        let first = order_members(&pending, &members);
        let mut shuffled = members.clone();
        shuffled.reverse();
        let second = order_members(&pending, &shuffled);
        assert_eq!(first, second);
        assert_eq!(first.len(), pending.len());
    }

    #[test]
    fn the_engine_partitions_and_orders_in_one_pass() {
        let pending = vec![
            pending_transaction(1, 5, TransactionClass::Routine, b"a"),
            pending_transaction(2, 1, TransactionClass::Constitutional, b"a"),
            pending_transaction(3, 1, TransactionClass::Routine, b"b"),
        ];
        let engine = DomainPartitioner::new();
        let domains = ExecutionEngine::partition(&engine, &pending);
        assert_eq!(domains.len(), 2);
        assert_eq!(domains[0].members, vec![1, 0]);
        assert_eq!(domains[0].highest_class, TransactionClass::Constitutional);
        assert_eq!(domains[1].members, vec![2]);
        assert_eq!(
            ExecutionEngine::order(&engine, &pending, &[0, 1]),
            vec![1, 0]
        );
    }

    #[test]
    fn the_engine_names_itself() {
        assert_eq!(DomainPartitioner::KIND, EngineKind::Execution);
        assert_eq!(DomainPartitioner::engine_name(), "Execution Engine");
    }

    #[test]
    fn an_empty_domain_orders_to_an_empty_list() {
        let pending: Vec<PendingTransaction> = Vec::new();
        assert!(order_members(&pending, &[]).is_empty());
        let mut domains: Vec<ConflictDomain> = Vec::new();
        order_domains(&pending, &mut domains);
        assert!(domains.is_empty());
    }

    #[test]
    fn a_sort_key_reads_the_three_rules() {
        let heavy = OrderKey::of(&pending_transaction(
            1,
            1,
            TransactionClass::Constitutional,
            b"k",
        ));
        let light = OrderKey::of(&pending_transaction(1, 1, TransactionClass::Routine, b"k"));
        assert!(heavy < light);
        let early = OrderKey::of(&pending_transaction(1, 1, TransactionClass::Routine, b"k"));
        let late = OrderKey::of(&pending_transaction(1, 2, TransactionClass::Routine, b"k"));
        assert!(early < late);
    }
}
