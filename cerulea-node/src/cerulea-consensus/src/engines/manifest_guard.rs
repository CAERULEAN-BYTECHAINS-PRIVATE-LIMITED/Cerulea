//! Enforcement of the Access Manifest during execution.
//!
//! The Access Manifest is a contract rather than a hint. Every state access a
//! transaction makes passes through the guard in this module. An access outside
//! the manifest aborts that transaction. The transaction writes nothing and the
//! block carries on with the rest of the domain.
//!
//! Enforcement is what makes the declaration trustworthy. Parallel execution
//! and per-domain commitment both rest on the manifest naming the true access
//! set. Without enforcement a transaction could reach state another domain
//! holds, and two domains would stop being independent.

use std::collections::BTreeMap;

use super::execution::meter::{ResourceLimits, ResourceMeter};
use super::manifest::AccessManifest;
use super::{CollectionId, EngineError};

/// One record of one collection, as the store reads it back.
///
/// The pair names the collection and the key. The value is the record body.
pub type StoredRecord = ((CollectionId, Vec<u8>), Vec<u8>);

/// A store of collection records.
///
/// The Execution Engine runs every transaction against a type implementing this
/// trait. The guard wraps the store and checks each access against the
/// manifest first.
pub trait StateAccess {
    /// Read one record.
    fn read(&self, collection: &CollectionId, key: &[u8]) -> Option<Vec<u8>>;

    /// Write one record.
    fn write(&mut self, collection: &CollectionId, key: &[u8], value: Vec<u8>);

    /// Remove one record.
    fn remove(&mut self, collection: &CollectionId, key: &[u8]);

    /// Every record the store holds, in a settled order.
    ///
    /// The Execution Engine hashes this sequence into the domain state root, so
    /// the order holds for every node.
    fn records(&self) -> Vec<StoredRecord>;
}

/// A store of collection records held in memory.
///
/// Records sit in a sorted map, so two nodes holding the same records read them
/// back in the same order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryStateStore {
    records: BTreeMap<(CollectionId, Vec<u8>), Vec<u8>>,
}

impl MemoryStateStore {
    /// Start an empty store.
    pub fn new() -> Self {
        MemoryStateStore {
            records: BTreeMap::new(),
        }
    }

    /// Place one record without passing the guard.
    ///
    /// Block execution never calls this. The caller loading state before a
    /// block runs calls this.
    pub fn place(&mut self, collection: CollectionId, key: &[u8], value: Vec<u8>) {
        self.records.insert((collection, key.to_vec()), value);
    }

    /// The number of records held.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Report whether the store holds zero records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl StateAccess for MemoryStateStore {
    fn read(&self, collection: &CollectionId, key: &[u8]) -> Option<Vec<u8>> {
        self.records.get(&(*collection, key.to_vec())).cloned()
    }

    fn write(&mut self, collection: &CollectionId, key: &[u8], value: Vec<u8>) {
        self.records.insert((*collection, key.to_vec()), value);
    }

    fn remove(&mut self, collection: &CollectionId, key: &[u8]) {
        self.records.remove(&(*collection, key.to_vec()));
    }

    fn records(&self) -> Vec<StoredRecord> {
        self.records
            .iter()
            .map(|(place, value)| (place.clone(), value.clone()))
            .collect()
    }
}

/// The count of accesses one transaction made.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AccessTally {
    /// The number of reads the transaction made.
    pub reads: u32,
    /// The number of writes the transaction made.
    pub writes: u32,
    /// The number of removals the transaction made.
    pub removals: u32,
}

impl AccessTally {
    /// The total number of accesses.
    pub fn total(&self) -> u32 {
        self.reads
            .saturating_add(self.writes)
            .saturating_add(self.removals)
    }
}

/// A guard wrapping state access for one transaction.
///
/// Every access passes the manifest check first. An access outside the manifest
/// aborts the transaction. Once a transaction aborts, every later access on the
/// same guard reports the same abort, so a partial write never lands.
pub struct ManifestGuard<'a, S: StateAccess> {
    manifest: &'a AccessManifest,
    store: &'a mut S,
    abort: Option<EngineError>,
    tally: AccessTally,
    meter: ResourceMeter,
}

impl<'a, S: StateAccess> ManifestGuard<'a, S> {
    /// Wrap a store for one transaction under the standard resource limits.
    pub fn new(manifest: &'a AccessManifest, store: &'a mut S) -> Self {
        ManifestGuard::with_limits(manifest, store, ResourceLimits::standard())
    }

    /// Wrap a store for one transaction under the given resource limits.
    pub fn with_limits(
        manifest: &'a AccessManifest,
        store: &'a mut S,
        limits: ResourceLimits,
    ) -> Self {
        ManifestGuard {
            manifest,
            store,
            abort: None,
            tally: AccessTally::default(),
            meter: ResourceMeter::new(limits),
        }
    }

    /// The resource meter of this transaction.
    pub fn meter(&self) -> &ResourceMeter {
        &self.meter
    }

    /// Charge computation units for work the transaction body did.
    ///
    /// A charge past the computation limit aborts the transaction.
    pub fn charge_work(&mut self, units: u64) -> Result<(), EngineError> {
        self.still_running()?;
        match self.meter.charge_work(units) {
            Ok(()) => Ok(()),
            Err(reason) => Err(self.abort_with(reason)),
        }
    }

    /// Take memory for bytes the transaction body now holds.
    ///
    /// A take past the memory limit aborts the transaction.
    pub fn take_memory(&mut self, bytes: u64) -> Result<(), EngineError> {
        self.still_running()?;
        match self.meter.take_memory(bytes) {
            Ok(()) => Ok(()),
            Err(reason) => Err(self.abort_with(reason)),
        }
    }

    /// Give back memory for bytes the transaction body dropped.
    pub fn give_memory(&mut self, bytes: u64) {
        self.meter.give_memory(bytes);
    }

    /// Read one record.
    ///
    /// The manifest permits a read where the read side names the key. A
    /// declared write permits a read of the same key.
    pub fn read(
        &mut self,
        collection: &CollectionId,
        key: &[u8],
    ) -> Result<Option<Vec<u8>>, EngineError> {
        self.still_running()?;
        if !self.manifest.permits_read(collection, key) {
            return Err(self.abort_with(EngineError::ReadOutsideManifest {
                collection: readable(collection),
                key: readable_key(key),
            }));
        }
        let value = self.store.read(collection, key);
        let length = value.as_ref().map(|body| body.len()).unwrap_or(0) as u64;
        if let Err(reason) = self.meter.charge_read(length) {
            return Err(self.abort_with(reason));
        }
        self.tally.reads = self.tally.reads.saturating_add(1);
        Ok(value)
    }

    /// Write one record.
    pub fn write(
        &mut self,
        collection: &CollectionId,
        key: &[u8],
        value: Vec<u8>,
    ) -> Result<(), EngineError> {
        self.still_running()?;
        if !self.manifest.permits_write(collection, key) {
            return Err(self.abort_with(EngineError::WriteOutsideManifest {
                collection: readable(collection),
                key: readable_key(key),
            }));
        }
        if let Err(reason) = self
            .meter
            .charge_write(key.len() as u64, value.len() as u64)
        {
            return Err(self.abort_with(reason));
        }
        self.tally.writes = self.tally.writes.saturating_add(1);
        self.store.write(collection, key, value);
        Ok(())
    }

    /// Remove one record.
    ///
    /// Removal counts as a write. The manifest permits removal where the write
    /// side names the key.
    pub fn remove(&mut self, collection: &CollectionId, key: &[u8]) -> Result<(), EngineError> {
        self.still_running()?;
        if !self.manifest.permits_write(collection, key) {
            return Err(self.abort_with(EngineError::WriteOutsideManifest {
                collection: readable(collection),
                key: readable_key(key),
            }));
        }
        if let Err(reason) = self.meter.charge_removal(key.len() as u64) {
            return Err(self.abort_with(reason));
        }
        self.tally.removals = self.tally.removals.saturating_add(1);
        self.store.remove(collection, key);
        Ok(())
    }

    /// Report whether the transaction still runs.
    pub fn running(&self) -> bool {
        self.abort.is_none()
    }

    /// The abort the guard recorded.
    pub fn abort_reason(&self) -> Option<&EngineError> {
        self.abort.as_ref()
    }

    /// The count of accesses the transaction made.
    pub fn tally(&self) -> AccessTally {
        self.tally
    }

    /// Close the guard and report the outcome.
    pub fn finish(self) -> Result<AccessTally, EngineError> {
        match self.abort {
            Some(reason) => Err(reason),
            None => Ok(self.tally),
        }
    }

    fn still_running(&self) -> Result<(), EngineError> {
        match &self.abort {
            Some(reason) => Err(reason.clone()),
            None => Ok(()),
        }
    }

    fn abort_with(&mut self, reason: EngineError) -> EngineError {
        self.abort = Some(reason.clone());
        reason
    }
}

/// Run one transaction body against a store under the manifest.
///
/// The body receives the guard and makes its accesses through it. A body that
/// reaches outside the manifest aborts and the store keeps the writes the body
/// already made. The caller running a domain snapshots the store before each
/// transaction and restores the snapshot on abort.
pub fn run_under_manifest<S, F, T>(
    manifest: &AccessManifest,
    store: &mut S,
    body: F,
) -> Result<(T, AccessTally), EngineError>
where
    S: StateAccess,
    F: FnOnce(&mut ManifestGuard<'_, S>) -> Result<T, EngineError>,
{
    let mut guard = ManifestGuard::new(manifest, store);
    let outcome = body(&mut guard);
    match outcome {
        Ok(value) => {
            let tally = guard.finish()?;
            Ok((value, tally))
        }
        Err(reason) => Err(reason),
    }
}

/// Render a collection identifier for an error message.
fn readable(collection: &CollectionId) -> String {
    hex(collection.as_bytes())
}

/// Render a key for an error message.
///
/// A key that reads as text appears as text. Every other key appears as hex.
fn readable_key(key: &[u8]) -> String {
    match core::str::from_utf8(key) {
        Ok(text) if text.chars().all(|c| !c.is_control()) => text.to_string(),
        _ => hex(key),
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        text.push_str(&format!("{:02x}", byte));
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::manifest::CollectionAccess;
    use crate::engines::TransactionClass;

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn holder() -> CollectionId {
        CollectionId::from_name("holder")
    }

    fn manifest() -> AccessManifest {
        AccessManifest::build(
            TransactionClass::Financial,
            vec![CollectionAccess::exact(holder(), b"holder-1")],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest builds")
    }

    fn loaded_store() -> MemoryStateStore {
        let mut store = MemoryStateStore::new();
        store.place(holder(), b"holder-1", b"Kollam".to_vec());
        store.place(parcel(), b"parcel-1", b"north field".to_vec());
        store.place(parcel(), b"parcel-2", b"south field".to_vec());
        store
    }

    #[test]
    fn a_declared_read_passes_the_guard() {
        let manifest = manifest();
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        let value = guard.read(&holder(), b"holder-1").expect("the read passes");
        assert_eq!(value, Some(b"Kollam".to_vec()));
        assert_eq!(guard.tally().reads, 1);
        assert!(guard.running());
    }

    #[test]
    fn a_read_outside_the_manifest_aborts_the_transaction() {
        let manifest = manifest();
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        let error = guard
            .read(&parcel(), b"parcel-2")
            .expect_err("the guard rejects the read");
        match &error {
            EngineError::ReadOutsideManifest { key, .. } => assert_eq!(key, "parcel-2"),
            other => panic!("the guard names the read: {:?}", other),
        }
        assert!(error.to_string().contains("Fix:"));
        assert!(!guard.running());
    }

    #[test]
    fn a_declared_write_passes_the_guard() {
        let manifest = manifest();
        let mut store = loaded_store();
        {
            let mut guard = ManifestGuard::new(&manifest, &mut store);
            guard
                .write(&parcel(), b"parcel-1", b"east field".to_vec())
                .expect("the write passes");
            assert_eq!(guard.tally().writes, 1);
        }
        assert_eq!(
            store.read(&parcel(), b"parcel-1"),
            Some(b"east field".to_vec())
        );
    }

    #[test]
    fn a_write_outside_the_manifest_aborts_and_writes_nothing() {
        let manifest = manifest();
        let mut store = loaded_store();
        {
            let mut guard = ManifestGuard::new(&manifest, &mut store);
            let error = guard
                .write(&parcel(), b"parcel-2", b"seized".to_vec())
                .expect_err("the guard rejects the write");
            match &error {
                EngineError::WriteOutsideManifest { key, .. } => assert_eq!(key, "parcel-2"),
                other => panic!("the guard names the write: {:?}", other),
            }
        }
        assert_eq!(
            store.read(&parcel(), b"parcel-2"),
            Some(b"south field".to_vec()),
            "the record the guard rejected keeps its value"
        );
    }

    #[test]
    fn a_read_of_a_collection_outside_the_manifest_aborts() {
        let manifest = manifest();
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        let other = CollectionId::from_name("ledger");
        assert!(guard.read(&other, b"holder-1").is_err());
    }

    #[test]
    fn an_abort_holds_for_every_later_access() {
        let manifest = manifest();
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        assert!(guard.read(&parcel(), b"parcel-2").is_err());
        let second = guard
            .write(&parcel(), b"parcel-1", b"east field".to_vec())
            .expect_err("the guard holds the abort");
        assert!(matches!(second, EngineError::ReadOutsideManifest { .. }));
        assert!(guard.abort_reason().is_some());
        assert!(guard.finish().is_err());
    }

    #[test]
    fn a_declared_write_permits_a_read_of_the_same_key() {
        let manifest = manifest();
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        let value = guard.read(&parcel(), b"parcel-1").expect("the read passes");
        assert_eq!(value, Some(b"north field".to_vec()));
    }

    #[test]
    fn a_removal_needs_a_write_declaration() {
        let manifest = manifest();
        let mut store = loaded_store();
        {
            let mut guard = ManifestGuard::new(&manifest, &mut store);
            guard
                .remove(&parcel(), b"parcel-1")
                .expect("the removal passes");
            assert_eq!(guard.tally().removals, 1);
            assert!(guard.remove(&holder(), b"holder-1").is_err());
        }
        assert_eq!(store.read(&parcel(), b"parcel-1"), None);
    }

    #[test]
    fn a_body_that_stays_inside_the_manifest_reports_its_tally() {
        let manifest = manifest();
        let mut store = loaded_store();
        let (value, tally) = run_under_manifest(&manifest, &mut store, |guard| {
            let held = guard.read(&holder(), b"holder-1")?;
            guard.write(&parcel(), b"parcel-1", b"transferred".to_vec())?;
            Ok(held)
        })
        .expect("the body stays inside the manifest");
        assert_eq!(value, Some(b"Kollam".to_vec()));
        assert_eq!(tally.reads, 1);
        assert_eq!(tally.writes, 1);
        assert_eq!(tally.total(), 2);
    }

    #[test]
    fn a_body_that_reaches_outside_the_manifest_reports_the_abort() {
        let manifest = manifest();
        let mut store = loaded_store();
        let outcome = run_under_manifest(&manifest, &mut store, |guard| {
            guard.write(&parcel(), b"parcel-2", b"seized".to_vec())?;
            Ok(())
        });
        assert!(matches!(
            outcome,
            Err(EngineError::WriteOutsideManifest { .. })
        ));
    }

    #[test]
    fn a_prefix_declaration_passes_every_key_beneath_it() {
        let sweep = AccessManifest::build(
            TransactionClass::Structural,
            vec![],
            vec![CollectionAccess::prefix(parcel(), b"parcel-")],
        )
        .expect("the manifest builds");
        let mut store = loaded_store();
        let mut guard = ManifestGuard::new(&sweep, &mut store);
        assert!(guard.write(&parcel(), b"parcel-1", vec![1]).is_ok());
        assert!(guard.write(&parcel(), b"parcel-2", vec![2]).is_ok());
        assert!(guard.write(&parcel(), b"other-3", vec![3]).is_err());
    }

    #[test]
    fn the_store_reads_back_its_records_in_a_settled_order() {
        let store = loaded_store();
        let first = store.records();
        let mut rebuilt = MemoryStateStore::new();
        rebuilt.place(parcel(), b"parcel-2", b"south field".to_vec());
        rebuilt.place(parcel(), b"parcel-1", b"north field".to_vec());
        rebuilt.place(holder(), b"holder-1", b"Kollam".to_vec());
        assert_eq!(first, rebuilt.records());
        assert_eq!(store.len(), 3);
        assert!(!store.is_empty());
        assert!(MemoryStateStore::new().is_empty());
    }

    #[test]
    fn an_error_message_renders_a_binary_key_as_hex() {
        let manifest = manifest();
        let mut store = MemoryStateStore::new();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        let error = guard
            .read(&parcel(), &[0u8, 255u8])
            .expect_err("the guard rejects the read");
        match &error {
            EngineError::ReadOutsideManifest { key, .. } => assert_eq!(key, "00ff"),
            other => panic!("the guard renders the key: {:?}", other),
        }
    }
}
