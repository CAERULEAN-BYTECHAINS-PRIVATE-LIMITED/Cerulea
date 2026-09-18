//! Parallel domain execution across cores.
//!
//! Conflict domains reach disjoint state. Each domain runs on its own core. The
//! runner spreads the domains over a pool of worker threads, waits for every
//! domain, and merges the results in domain order.
//!
//! Determinism holds in three places. Each domain runs its members in the order
//! the ordering step settled. Each domain writes into its own overlay, so one
//! domain never sees a partial result of another. The merge applies the
//! overlays in domain order. Two nodes running the same block on different core
//! counts reach the same state.
//!
//! A transaction that reaches outside its Access Manifest aborts on its own.
//! The runner rolls that transaction back and carries on with the rest of the
//! domain.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::engines::execution::meter::ResourceLimits;
use crate::engines::manifest::DeclaresAccess;
use crate::engines::manifest_guard::{
    AccessTally, ManifestGuard, MemoryStateStore, StateAccess, StoredRecord,
};
use crate::engines::{CollectionId, ConflictDomain, EngineError, TransactionClass};

/// One pending change held by a domain.
///
/// The collection and the key name the record. A body replaces the record. An
/// absent body removes the record.
type Change = (CollectionId, Vec<u8>, Option<Vec<u8>>);

/// The place one record sits, as the overlay keys it.
type Place = (CollectionId, Vec<u8>);

/// One journal entry recording the overlay value a write replaced.
///
/// An absent outer value means the overlay held no entry for that place.
type JournalEntry = (Place, Option<Option<Vec<u8>>>);

/// The state one domain sees while it runs.
///
/// Reads fall through to the block state. Writes land in an overlay held by
/// this domain alone. A journal records the overlay entry each write replaced,
/// so one transaction rolls back without disturbing the transactions before it.
pub struct DomainStore<'a> {
    base: &'a MemoryStateStore,
    overlay: BTreeMap<Place, Option<Vec<u8>>>,
    journal: Vec<JournalEntry>,
    journalling: bool,
}

impl<'a> DomainStore<'a> {
    /// Open a domain view over the block state.
    pub fn new(base: &'a MemoryStateStore) -> Self {
        DomainStore {
            base,
            overlay: BTreeMap::new(),
            journal: Vec::new(),
            journalling: false,
        }
    }

    /// Start recording the changes of one transaction.
    pub fn start_transaction(&mut self) {
        self.journal.clear();
        self.journalling = true;
    }

    /// Keep the changes of the running transaction.
    pub fn keep_transaction(&mut self) {
        self.journal.clear();
        self.journalling = false;
    }

    /// Undo the changes of the running transaction.
    pub fn undo_transaction(&mut self) {
        while let Some((place, previous)) = self.journal.pop() {
            match previous {
                Some(entry) => {
                    self.overlay.insert(place, entry);
                }
                None => {
                    self.overlay.remove(&place);
                }
            }
        }
        self.journalling = false;
    }

    /// The changes this domain made, in key order.
    pub fn changes(&self) -> Vec<Change> {
        self.overlay
            .iter()
            .map(|((collection, key), value)| (*collection, key.clone(), value.clone()))
            .collect()
    }

    fn record(&mut self, place: Place, value: Option<Vec<u8>>) {
        if self.journalling {
            let previous = self.overlay.get(&place).cloned();
            self.journal.push((place.clone(), previous));
        }
        self.overlay.insert(place, value);
    }
}

impl StateAccess for DomainStore<'_> {
    fn read(&self, collection: &CollectionId, key: &[u8]) -> Option<Vec<u8>> {
        match self.overlay.get(&(*collection, key.to_vec())) {
            Some(entry) => entry.clone(),
            None => self.base.read(collection, key),
        }
    }

    fn write(&mut self, collection: &CollectionId, key: &[u8], value: Vec<u8>) {
        self.record((*collection, key.to_vec()), Some(value));
    }

    fn remove(&mut self, collection: &CollectionId, key: &[u8]) {
        self.record((*collection, key.to_vec()), None);
    }

    fn records(&self) -> Vec<StoredRecord> {
        let mut records: BTreeMap<Place, Vec<u8>> = self.base.records().into_iter().collect();
        for ((collection, key), value) in self.overlay.iter() {
            match value {
                Some(body) => {
                    records.insert((*collection, key.clone()), body.clone());
                }
                None => {
                    records.remove(&(*collection, key.clone()));
                }
            }
        }
        records.into_iter().collect()
    }
}

/// The result of running one transaction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransactionOutcome {
    /// The position the transaction held in the pending list.
    pub member: usize,
    /// The hash of the transaction.
    pub hash: [u8; 32],
    /// The count of accesses the transaction made.
    pub tally: AccessTally,
    /// The reason the transaction aborted, where it aborted.
    pub abort: Option<EngineError>,
}

impl TransactionOutcome {
    /// Report whether the transaction kept its changes.
    pub fn applied(&self) -> bool {
        self.abort.is_none()
    }
}

/// The result of running one domain.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DomainOutcome {
    /// The position the domain held in the domain list.
    pub domain: usize,
    /// The highest transaction class the domain carried.
    pub highest_class: TransactionClass,
    /// The result of each member, in execution order.
    pub outcomes: Vec<TransactionOutcome>,
    /// The changes the domain made, in key order.
    pub changes: Vec<Change>,
}

impl DomainOutcome {
    /// The number of members that kept their changes.
    pub fn applied_count(&self) -> usize {
        self.outcomes
            .iter()
            .filter(|outcome| outcome.applied())
            .count()
    }

    /// The number of members that aborted.
    pub fn aborted_count(&self) -> usize {
        self.outcomes.len().saturating_sub(self.applied_count())
    }
}

/// The result of running every domain of one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReport {
    /// One entry per domain, in domain order.
    pub domains: Vec<DomainOutcome>,
}

impl ExecutionReport {
    /// The number of transactions that kept their changes.
    pub fn applied_count(&self) -> usize {
        self.domains
            .iter()
            .map(|domain| domain.applied_count())
            .sum()
    }

    /// The number of transactions that aborted.
    pub fn aborted_count(&self) -> usize {
        self.domains
            .iter()
            .map(|domain| domain.aborted_count())
            .sum()
    }
}

/// A body the Execution Engine runs for one transaction.
///
/// A closure of the matching shape implements this trait.
pub trait TransactionRunner<T>: Send + Sync {
    /// Run one transaction against the guard.
    fn run(
        &self,
        transaction: &T,
        guard: &mut ManifestGuard<'_, DomainStore<'_>>,
    ) -> Result<(), EngineError>;
}

impl<T, F> TransactionRunner<T> for F
where
    F: Fn(&T, &mut ManifestGuard<'_, DomainStore<'_>>) -> Result<(), EngineError> + Send + Sync,
{
    fn run(
        &self,
        transaction: &T,
        guard: &mut ManifestGuard<'_, DomainStore<'_>>,
    ) -> Result<(), EngineError> {
        self(transaction, guard)
    }
}

/// The Execution Engine runner.
///
/// The worker count sets how many domains run at once. The count changes the
/// speed and leaves the result unchanged.
#[derive(Clone, Copy, Debug)]
pub struct DomainRunner {
    workers: usize,
}

impl Default for DomainRunner {
    fn default() -> Self {
        DomainRunner::across_available_cores()
    }
}

impl DomainRunner {
    /// Build a runner using one worker per available core.
    pub fn across_available_cores() -> Self {
        let workers = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1);
        DomainRunner { workers }
    }

    /// Build a runner holding a fixed worker count.
    ///
    /// A count of zero rises to one, so the runner always makes progress.
    pub fn with_workers(workers: usize) -> Self {
        DomainRunner {
            workers: workers.max(1),
        }
    }

    /// The worker count this runner holds.
    pub fn workers(&self) -> usize {
        self.workers
    }

    /// Run every domain and merge the results into the block state.
    ///
    /// The runner returns one report per domain in domain order. The block
    /// state carries every change the domains kept.
    pub fn run<T, R>(
        &self,
        pending: &[T],
        domains: &[ConflictDomain],
        state: &mut MemoryStateStore,
        runner: &R,
    ) -> ExecutionReport
    where
        T: DeclaresAccess + Sync,
        R: TransactionRunner<T>,
    {
        self.run_with_limits(pending, domains, state, runner, ResourceLimits::standard())
    }

    /// Run every domain under the given resource limits.
    ///
    /// A transaction that crosses a limit aborts on its own. The rest of the
    /// domain carries on.
    pub fn run_with_limits<T, R>(
        &self,
        pending: &[T],
        domains: &[ConflictDomain],
        state: &mut MemoryStateStore,
        runner: &R,
        limits: ResourceLimits,
    ) -> ExecutionReport
    where
        T: DeclaresAccess + Sync,
        R: TransactionRunner<T>,
    {
        if domains.is_empty() {
            return ExecutionReport {
                domains: Vec::new(),
            };
        }
        let base: &MemoryStateStore = state;
        let next = AtomicUsize::new(0);
        let collected: Mutex<Vec<DomainOutcome>> = Mutex::new(Vec::with_capacity(domains.len()));
        let workers = self.workers.min(domains.len()).max(1);

        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(|| loop {
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    if index >= domains.len() {
                        break;
                    }
                    let outcome =
                        run_one_domain(index, &domains[index], pending, base, runner, limits);
                    collected
                        .lock()
                        .expect("the result list stays available")
                        .push(outcome);
                });
            }
        });

        let mut results = collected
            .into_inner()
            .expect("the result list stays available");
        results.sort_by_key(|outcome| outcome.domain);
        for outcome in &results {
            for (collection, key, value) in &outcome.changes {
                match value {
                    Some(body) => state.write(collection, key, body.clone()),
                    None => state.remove(collection, key),
                }
            }
        }
        ExecutionReport { domains: results }
    }
}

fn run_one_domain<T, R>(
    index: usize,
    domain: &ConflictDomain,
    pending: &[T],
    base: &MemoryStateStore,
    runner: &R,
    limits: ResourceLimits,
) -> DomainOutcome
where
    T: DeclaresAccess + Sync,
    R: TransactionRunner<T>,
{
    let mut store = DomainStore::new(base);
    let mut outcomes = Vec::with_capacity(domain.members.len());
    for member in &domain.members {
        let transaction = &pending[*member];
        store.start_transaction();
        let (result, tally) = {
            let mut guard = ManifestGuard::with_limits(transaction.manifest(), &mut store, limits);
            let result = runner.run(transaction, &mut guard);
            let tally = guard.tally();
            (result, tally)
        };
        match result {
            Ok(()) => {
                store.keep_transaction();
                outcomes.push(TransactionOutcome {
                    member: *member,
                    hash: transaction.transaction_hash(),
                    tally,
                    abort: None,
                });
            }
            Err(reason) => {
                store.undo_transaction();
                outcomes.push(TransactionOutcome {
                    member: *member,
                    hash: transaction.transaction_hash(),
                    tally,
                    abort: Some(reason),
                });
            }
        }
    }
    DomainOutcome {
        domain: index,
        highest_class: domain.highest_class,
        outcomes,
        changes: store.changes(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::execution::partition::DomainPartitioner;
    use crate::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
    use crate::engines::ExecutionEngine;

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

    /// The body writes its own hash seed into the key it declared.
    fn write_body(
        transaction: &PendingTransaction,
        guard: &mut ManifestGuard<'_, DomainStore<'_>>,
    ) -> Result<(), EngineError> {
        let access = transaction
            .manifest
            .writes
            .first()
            .expect("the transaction declares one write");
        let key = match &access.keys {
            crate::engines::KeyRange::Exact(key) => key.clone(),
            _ => Vec::new(),
        };
        guard.write(&access.collection, &key, vec![transaction.hash[0]])?;
        Ok(())
    }

    fn run_block(
        workers: usize,
        pending: &[PendingTransaction],
    ) -> (MemoryStateStore, ExecutionReport) {
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), pending);
        let mut state = MemoryStateStore::new();
        let report =
            DomainRunner::with_workers(workers).run(pending, &domains, &mut state, &write_body);
        (state, report)
    }

    #[test]
    fn two_disjoint_domains_run_in_parallel_and_produce_one_state() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, holder(), b"holder-1"),
        ];
        let (state, report) = run_block(4, &pending);
        assert_eq!(report.domains.len(), 2);
        assert_eq!(report.applied_count(), 2);
        assert_eq!(report.aborted_count(), 0);
        assert_eq!(state.read(&parcel(), b"parcel-1"), Some(vec![1]));
        assert_eq!(state.read(&holder(), b"holder-1"), Some(vec![2]));
    }

    #[test]
    fn one_worker_and_many_workers_reach_the_same_state() {
        let pending: Vec<PendingTransaction> = (1..=32u8)
            .map(|index| writer(index, parcel(), format!("parcel-{}", index).as_bytes()))
            .collect();
        let (serial, serial_report) = run_block(1, &pending);
        let (parallel, parallel_report) = run_block(8, &pending);
        assert_eq!(serial.records(), parallel.records());
        assert_eq!(serial_report, parallel_report);
        assert_eq!(serial.len(), 32);
    }

    #[test]
    fn two_transactions_on_one_key_run_in_order_inside_one_domain() {
        let pending = vec![
            writer(7, parcel(), b"parcel-1"),
            writer(3, parcel(), b"parcel-1"),
        ];
        let (state, report) = run_block(4, &pending);
        assert_eq!(report.domains.len(), 1);
        // Arrival sequence three runs ahead of arrival sequence seven, so the
        // later arrival leaves the final value.
        assert_eq!(report.domains[0].outcomes[0].member, 1);
        assert_eq!(report.domains[0].outcomes[1].member, 0);
        assert_eq!(state.read(&parcel(), b"parcel-1"), Some(vec![7]));
    }

    #[test]
    fn a_transaction_reaching_outside_its_manifest_aborts_alone() {
        let honest = writer(1, parcel(), b"parcel-1");
        let stray = writer(2, parcel(), b"parcel-2");
        let pending = vec![honest, stray];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            if transaction.hash[0] == 2 {
                // The stray transaction reaches a key it never declared.
                guard.write(&parcel(), b"parcel-9", vec![9])?;
            } else {
                guard.write(&parcel(), b"parcel-1", vec![1])?;
            }
            Ok(())
        };
        let report = DomainRunner::with_workers(2).run(&pending, &domains, &mut state, &body);
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.aborted_count(), 1);
        assert_eq!(state.read(&parcel(), b"parcel-1"), Some(vec![1]));
        assert_eq!(state.read(&parcel(), b"parcel-9"), None);
    }

    #[test]
    fn an_abort_rolls_back_the_earlier_writes_of_that_transaction_alone() {
        let first = writer(1, parcel(), b"parcel-1");
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![
                CollectionAccess::exact(parcel(), b"parcel-1"),
                CollectionAccess::exact(parcel(), b"parcel-2"),
            ],
        )
        .expect("the manifest builds");
        let second = PendingTransaction::new([2u8; 32], 2, manifest, vec![]);
        let pending = vec![first, second];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            if transaction.hash[0] == 2 {
                guard.write(&parcel(), b"parcel-2", vec![22])?;
                guard.write(&parcel(), b"parcel-1", vec![21])?;
                guard.write(&holder(), b"holder-1", vec![23])?;
            } else {
                guard.write(&parcel(), b"parcel-1", vec![11])?;
            }
            Ok(())
        };
        let report = DomainRunner::with_workers(1).run(&pending, &domains, &mut state, &body);
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.aborted_count(), 1);
        assert_eq!(
            state.read(&parcel(), b"parcel-1"),
            Some(vec![11]),
            "the first transaction keeps its write"
        );
        assert_eq!(
            state.read(&parcel(), b"parcel-2"),
            None,
            "the aborted transaction leaves nothing behind"
        );
    }

    #[test]
    fn a_domain_reads_the_value_an_earlier_member_wrote() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, parcel(), b"parcel-1"),
        ];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let seen: Mutex<Vec<Option<Vec<u8>>>> = Mutex::new(Vec::new());
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            let before = guard.read(&parcel(), b"parcel-1")?;
            seen.lock().expect("the list stays available").push(before);
            guard.write(&parcel(), b"parcel-1", vec![transaction.hash[0]])?;
            Ok(())
        };
        DomainRunner::with_workers(1).run(&pending, &domains, &mut state, &body);
        let seen = seen.into_inner().expect("the list stays available");
        assert_eq!(seen, vec![None, Some(vec![1])]);
    }

    #[test]
    fn a_domain_never_sees_the_changes_of_another_domain() {
        let pending = vec![
            writer(1, parcel(), b"parcel-1"),
            writer(2, holder(), b"holder-1"),
        ];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            let access = transaction
                .manifest
                .writes
                .first()
                .expect("one write declared");
            let key = match &access.keys {
                crate::engines::KeyRange::Exact(key) => key.clone(),
                _ => Vec::new(),
            };
            // A read of the other domain key sits outside this manifest and
            // would abort. The domain reads its own key alone.
            let before = guard.read(&access.collection, &key)?;
            assert_eq!(before, None);
            guard.write(&access.collection, &key, vec![transaction.hash[0]])?;
            Ok(())
        };
        let report = DomainRunner::with_workers(2).run(&pending, &domains, &mut state, &body);
        assert_eq!(report.aborted_count(), 0);
    }

    #[test]
    fn a_domain_reads_the_block_state_it_started_from() {
        let mut state = MemoryStateStore::new();
        state.place(parcel(), b"parcel-1", b"before".to_vec());
        let pending = vec![writer(1, parcel(), b"parcel-1")];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let body = |_: &PendingTransaction, guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            let before = guard.read(&parcel(), b"parcel-1")?;
            assert_eq!(before, Some(b"before".to_vec()));
            guard.write(&parcel(), b"parcel-1", b"after".to_vec())?;
            Ok(())
        };
        DomainRunner::with_workers(1).run(&pending, &domains, &mut state, &body);
        assert_eq!(state.read(&parcel(), b"parcel-1"), Some(b"after".to_vec()));
    }

    #[test]
    fn a_removal_reaches_the_block_state() {
        let mut state = MemoryStateStore::new();
        state.place(parcel(), b"parcel-1", b"before".to_vec());
        let pending = vec![writer(1, parcel(), b"parcel-1")];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let body = |_: &PendingTransaction, guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            guard.remove(&parcel(), b"parcel-1")?;
            Ok(())
        };
        DomainRunner::with_workers(1).run(&pending, &domains, &mut state, &body);
        assert_eq!(state.read(&parcel(), b"parcel-1"), None);
        assert!(state.is_empty());
    }

    #[test]
    fn an_empty_block_runs_to_an_empty_report() {
        let pending: Vec<PendingTransaction> = Vec::new();
        let mut state = MemoryStateStore::new();
        let report = DomainRunner::with_workers(4).run(&pending, &[], &mut state, &write_body);
        assert!(report.domains.is_empty());
        assert_eq!(report.applied_count(), 0);
    }

    #[test]
    fn a_runner_holds_at_least_one_worker() {
        assert_eq!(DomainRunner::with_workers(0).workers(), 1);
        assert!(DomainRunner::across_available_cores().workers() >= 1);
        assert!(DomainRunner::default().workers() >= 1);
    }
}
