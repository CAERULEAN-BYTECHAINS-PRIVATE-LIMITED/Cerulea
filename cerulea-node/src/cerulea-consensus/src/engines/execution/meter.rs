//! Per-transaction resource bounds.
//!
//! Each transaction carries hard limits on three resources. Computation,
//! memory and storage. A transaction that crosses a limit aborts on its own.
//! The rest of the domain carries on and the block still lands.
//!
//! Bounding the work per transaction is what keeps one action from stalling a
//! whole domain. A person building a deployment never sets these numbers. The
//! deployment profile carries them and the Amendment Engine governs a change.
//!
//! # What each resource measures
//!
//! 1. **Computation.** Units of work. Every state access charges a fixed unit
//!    cost plus a cost for each byte it moves. A transaction body charges its
//!    own arithmetic and its own loops through [`ResourceMeter::charge_work`].
//! 2. **Memory.** The bytes the transaction holds at once. The value bytes of
//!    each read count. A body that builds its own structures charges them and
//!    releases them when it drops them.
//! 3. **Storage.** The bytes the transaction writes. A key and its value both
//!    count. A removal charges the key alone.

use crate::engines::EngineError;

/// The unit cost of one read, before the byte cost.
pub const READ_UNIT_COST: u64 = 1_000;

/// The unit cost of one write, before the byte cost.
pub const WRITE_UNIT_COST: u64 = 2_000;

/// The unit cost of one removal.
pub const REMOVAL_UNIT_COST: u64 = 2_000;

/// The unit cost of each byte a read moves.
pub const READ_BYTE_COST: u64 = 1;

/// The unit cost of each byte a write moves.
pub const WRITE_BYTE_COST: u64 = 2;

/// The limits one transaction runs under.
///
/// The deployment profile carries these values. The table here is the profile a
/// deployment starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceLimits {
    /// The highest number of computation units one transaction spends.
    pub computation: u64,
    /// The highest number of bytes one transaction holds at once.
    pub memory: u64,
    /// The highest number of bytes one transaction writes.
    pub storage: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        ResourceLimits::standard()
    }
}

impl ResourceLimits {
    /// The limits a deployment starts from.
    ///
    /// One hundred million computation units. Sixty four mebibytes of memory.
    /// One mebibyte of storage.
    pub const fn standard() -> Self {
        ResourceLimits {
            computation: 100_000_000,
            memory: 64 * 1024 * 1024,
            storage: 1024 * 1024,
        }
    }

    /// Build limits holding the three stated values.
    pub const fn new(computation: u64, memory: u64, storage: u64) -> Self {
        ResourceLimits {
            computation,
            memory,
            storage,
        }
    }
}

/// The resources one transaction spent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResourceUse {
    /// The computation units spent.
    pub computation: u64,
    /// The highest number of bytes held at one moment.
    pub peak_memory: u64,
    /// The bytes held at this moment.
    pub held_memory: u64,
    /// The bytes written.
    pub storage: u64,
}

/// The meter one transaction runs under.
///
/// The meter charges each resource and reports the crossing that aborts the
/// transaction.
#[derive(Clone, Copy, Debug)]
pub struct ResourceMeter {
    limits: ResourceLimits,
    used: ResourceUse,
}

impl Default for ResourceMeter {
    fn default() -> Self {
        ResourceMeter::new(ResourceLimits::standard())
    }
}

impl ResourceMeter {
    /// Start a meter under the given limits.
    pub const fn new(limits: ResourceLimits) -> Self {
        ResourceMeter {
            limits,
            used: ResourceUse {
                computation: 0,
                peak_memory: 0,
                held_memory: 0,
                storage: 0,
            },
        }
    }

    /// The limits this meter holds.
    pub const fn limits(&self) -> ResourceLimits {
        self.limits
    }

    /// The resources spent so far.
    pub const fn used(&self) -> ResourceUse {
        self.used
    }

    /// Charge computation units.
    ///
    /// A transaction body calls this for the work it does between state
    /// accesses.
    pub fn charge_work(&mut self, units: u64) -> Result<(), EngineError> {
        self.used.computation = self.used.computation.saturating_add(units);
        if self.used.computation > self.limits.computation {
            return Err(EngineError::ComputationLimitCrossed {
                used: self.used.computation,
                limit: self.limits.computation,
            });
        }
        Ok(())
    }

    /// Take memory for bytes the transaction now holds.
    pub fn take_memory(&mut self, bytes: u64) -> Result<(), EngineError> {
        self.used.held_memory = self.used.held_memory.saturating_add(bytes);
        if self.used.held_memory > self.used.peak_memory {
            self.used.peak_memory = self.used.held_memory;
        }
        if self.used.peak_memory > self.limits.memory {
            return Err(EngineError::MemoryLimitCrossed {
                used: self.used.peak_memory,
                limit: self.limits.memory,
            });
        }
        Ok(())
    }

    /// Give back memory for bytes the transaction dropped.
    pub fn give_memory(&mut self, bytes: u64) {
        self.used.held_memory = self.used.held_memory.saturating_sub(bytes);
    }

    /// Charge storage bytes.
    pub fn charge_storage(&mut self, bytes: u64) -> Result<(), EngineError> {
        self.used.storage = self.used.storage.saturating_add(bytes);
        if self.used.storage > self.limits.storage {
            return Err(EngineError::StorageLimitCrossed {
                used: self.used.storage,
                limit: self.limits.storage,
            });
        }
        Ok(())
    }

    /// Charge one read of the given value length.
    pub fn charge_read(&mut self, value_bytes: u64) -> Result<(), EngineError> {
        self.charge_work(
            READ_UNIT_COST.saturating_add(value_bytes.saturating_mul(READ_BYTE_COST)),
        )?;
        self.take_memory(value_bytes)
    }

    /// Charge one write of the given key and value lengths.
    pub fn charge_write(&mut self, key_bytes: u64, value_bytes: u64) -> Result<(), EngineError> {
        let moved = key_bytes.saturating_add(value_bytes);
        self.charge_work(WRITE_UNIT_COST.saturating_add(moved.saturating_mul(WRITE_BYTE_COST)))?;
        self.charge_storage(moved)
    }

    /// Charge one removal of the given key length.
    pub fn charge_removal(&mut self, key_bytes: u64) -> Result<(), EngineError> {
        self.charge_work(
            REMOVAL_UNIT_COST.saturating_add(key_bytes.saturating_mul(WRITE_BYTE_COST)),
        )?;
        self.charge_storage(key_bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::execution::partition::DomainPartitioner;
    use crate::engines::execution::runner::{DomainRunner, DomainStore};
    use crate::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
    use crate::engines::manifest_guard::{ManifestGuard, MemoryStateStore};
    use crate::engines::{CollectionId, ExecutionEngine, StateAccess, TransactionClass};

    fn parcel() -> CollectionId {
        CollectionId::from_name("parcel")
    }

    fn writer(seed: u8, key: &[u8]) -> PendingTransaction {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), key)],
        )
        .expect("the manifest builds");
        PendingTransaction::new([seed; 32], seed as u64, manifest, key.to_vec())
    }

    #[test]
    fn the_standard_limits_hold_their_stated_values() {
        let limits = ResourceLimits::standard();
        assert_eq!(limits.computation, 100_000_000);
        assert_eq!(limits.memory, 64 * 1024 * 1024);
        assert_eq!(limits.storage, 1024 * 1024);
        assert_eq!(ResourceLimits::default(), limits);
        assert_eq!(ResourceMeter::default().limits(), limits);
    }

    #[test]
    fn computation_past_the_limit_names_the_fix() {
        let mut meter = ResourceMeter::new(ResourceLimits::new(100, 1_000, 1_000));
        assert!(meter.charge_work(60).is_ok());
        let error = meter
            .charge_work(60)
            .expect_err("the meter reports the crossing");
        assert_eq!(
            error,
            EngineError::ComputationLimitCrossed {
                used: 120,
                limit: 100
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn memory_past_the_limit_names_the_fix() {
        let mut meter = ResourceMeter::new(ResourceLimits::new(1_000_000, 100, 1_000));
        assert!(meter.take_memory(80).is_ok());
        meter.give_memory(80);
        assert!(meter.take_memory(90).is_ok());
        let error = meter
            .take_memory(20)
            .expect_err("the meter reports the crossing");
        assert_eq!(
            error,
            EngineError::MemoryLimitCrossed {
                used: 110,
                limit: 100
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn storage_past_the_limit_names_the_fix() {
        let mut meter = ResourceMeter::new(ResourceLimits::new(1_000_000, 1_000, 50));
        assert!(meter.charge_storage(30).is_ok());
        let error = meter
            .charge_storage(30)
            .expect_err("the meter reports the crossing");
        assert_eq!(
            error,
            EngineError::StorageLimitCrossed {
                used: 60,
                limit: 50
            }
        );
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn memory_records_the_highest_moment() {
        let mut meter = ResourceMeter::new(ResourceLimits::new(1_000_000, 1_000, 1_000));
        meter.take_memory(400).expect("the take lands");
        meter.take_memory(300).expect("the take lands");
        meter.give_memory(500);
        meter.take_memory(100).expect("the take lands");
        assert_eq!(meter.used().peak_memory, 700);
        assert_eq!(meter.used().held_memory, 300);
    }

    #[test]
    fn an_access_charges_a_unit_cost_and_a_byte_cost() {
        let mut meter = ResourceMeter::default();
        meter.charge_read(10).expect("the read lands");
        assert_eq!(meter.used().computation, READ_UNIT_COST + 10);
        assert_eq!(meter.used().peak_memory, 10);
        meter.charge_write(4, 6).expect("the write lands");
        assert_eq!(
            meter.used().computation,
            READ_UNIT_COST + 10 + WRITE_UNIT_COST + 20
        );
        assert_eq!(meter.used().storage, 10);
        meter.charge_removal(4).expect("the removal lands");
        assert_eq!(meter.used().storage, 14);
    }

    #[test]
    fn the_guard_charges_every_access_it_passes() {
        let manifest = AccessManifest::build(
            TransactionClass::Routine,
            vec![],
            vec![CollectionAccess::exact(parcel(), b"parcel-1")],
        )
        .expect("the manifest builds");
        let mut store = MemoryStateStore::new();
        let mut guard = ManifestGuard::new(&manifest, &mut store);
        guard
            .write(&parcel(), b"parcel-1", vec![7; 100])
            .expect("the write passes");
        let used = guard.meter().used();
        assert_eq!(used.storage, 108);
        assert!(used.computation >= WRITE_UNIT_COST);
    }

    #[test]
    fn a_transaction_crossing_a_storage_limit_aborts_alone() {
        let pending = vec![writer(1, b"parcel-1"), writer(2, b"parcel-2")];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            let key: Vec<u8> = transaction.body.clone();
            let size = if transaction.hash[0] == 2 { 400 } else { 10 };
            guard.write(&parcel(), &key, vec![1u8; size])?;
            Ok(())
        };
        let report = DomainRunner::with_workers(2).run_with_limits(
            &pending,
            &domains,
            &mut state,
            &body,
            ResourceLimits::new(100_000_000, 64 * 1024 * 1024, 100),
        );
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.aborted_count(), 1);
        assert!(state.read(&parcel(), b"parcel-1").is_some());
        assert!(state.read(&parcel(), b"parcel-2").is_none());
        let aborted = report
            .domains
            .iter()
            .flat_map(|domain| domain.outcomes.iter())
            .find(|outcome| !outcome.applied())
            .expect("one transaction aborted");
        assert!(matches!(
            aborted.abort,
            Some(EngineError::StorageLimitCrossed { .. })
        ));
    }

    #[test]
    fn a_transaction_crossing_a_computation_limit_aborts_alone() {
        let pending = vec![writer(1, b"parcel-1"), writer(2, b"parcel-2")];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            if transaction.hash[0] == 2 {
                for _ in 0..100 {
                    guard.charge_work(1_000)?;
                }
            }
            guard.write(&parcel(), &transaction.body, vec![1u8])?;
            Ok(())
        };
        let report = DomainRunner::with_workers(2).run_with_limits(
            &pending,
            &domains,
            &mut state,
            &body,
            ResourceLimits::new(20_000, 64 * 1024 * 1024, 1024),
        );
        assert_eq!(report.applied_count(), 1);
        assert_eq!(report.aborted_count(), 1);
        assert!(state.read(&parcel(), b"parcel-2").is_none());
    }

    #[test]
    fn the_meter_starts_fresh_for_each_transaction() {
        let pending = vec![writer(1, b"parcel-1"), writer(2, b"parcel-1")];
        let domains = ExecutionEngine::partition(&DomainPartitioner::new(), &pending);
        let mut state = MemoryStateStore::new();
        let body = |transaction: &PendingTransaction,
                    guard: &mut ManifestGuard<'_, DomainStore<'_>>| {
            guard.write(&parcel(), b"parcel-1", vec![transaction.hash[0]; 40])?;
            Ok(())
        };
        let report = DomainRunner::with_workers(1).run_with_limits(
            &pending,
            &domains,
            &mut state,
            &body,
            ResourceLimits::new(100_000_000, 64 * 1024 * 1024, 60),
        );
        assert_eq!(
            report.applied_count(),
            2,
            "each transaction carries its own allowance"
        );
    }
}
