//! The weight figures of the schema module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. The figures below come from the benchmark set in this crate,
//! measured on the reference machine the specification names. A chain that
//! runs different hardware regenerates the figures from the same set.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the schema module calls charge.
pub trait WeightInfo {
    /// Register one type definition.
    fn register_type() -> Weight;
    /// Register one collection definition.
    fn register_collection() -> Weight;
    /// Store one migration plan.
    fn propose_migration() -> Weight;
    /// Apply one stored migration plan.
    fn apply_migration() -> Weight;
    /// Undo one applied migration plan.
    fn revert_migration() -> Weight;
    /// Retire one collection.
    fn retire_collection() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_type() -> Weight {
        Weight::from_parts(38_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn register_collection() -> Weight {
        Weight::from_parts(46_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn propose_migration() -> Weight {
        Weight::from_parts(42_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn apply_migration() -> Weight {
        Weight::from_parts(75_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(6))
    }

    fn revert_migration() -> Weight {
        Weight::from_parts(75_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn retire_collection() -> Weight {
        Weight::from_parts(22_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
}

/// The figures a test runtime charges.
///
/// The test runtime carries no database weight, so each call charges its
/// reference time alone.
impl WeightInfo for () {
    fn register_type() -> Weight {
        Weight::from_parts(38_000_000, 4_000)
    }

    fn register_collection() -> Weight {
        Weight::from_parts(46_000_000, 5_000)
    }

    fn propose_migration() -> Weight {
        Weight::from_parts(42_000_000, 5_000)
    }

    fn apply_migration() -> Weight {
        Weight::from_parts(75_000_000, 8_000)
    }

    fn revert_migration() -> Weight {
        Weight::from_parts(75_000_000, 8_000)
    }

    fn retire_collection() -> Weight {
        Weight::from_parts(22_000_000, 3_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Test;
    use frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND;

    /// The reference time one block of the Cerulea chain spends.
    const BLOCK_REF_TIME: u64 = 2 * WEIGHT_REF_TIME_PER_SECOND;

    /// Check one measured figure against the figure a test runtime charges.
    ///
    /// The measured figure carries the storage charge the call performs, so
    /// it sits above the plain reference time in at least one part. Every
    /// figure fits inside one block.
    fn holds(call: &str, measured: Weight, plain: Weight) {
        assert!(
            measured.ref_time() > 0,
            "the figure of {call} charges reference time"
        );
        assert!(
            measured.proof_size() > 0,
            "the figure of {call} charges proof size"
        );
        assert!(
            measured.ref_time() <= BLOCK_REF_TIME,
            "the figure of {call} fits inside one block"
        );
        assert!(
            measured.ref_time() >= plain.ref_time(),
            "the figure of {call} charges the reference time a test runtime charges"
        );
        assert!(
            measured.proof_size() >= plain.proof_size(),
            "the figure of {call} charges the proof size a test runtime charges"
        );
        assert_ne!(
            measured, plain,
            "the figure of {call} carries the storage charge a test runtime lacks"
        );
        assert!(
            plain.ref_time() > 0,
            "the test runtime figure of {call} charges reference time"
        );
    }

    #[test]
    fn every_measured_figure_carries_the_storage_charge_its_call_performs() {
        holds(
            "register_type",
            <CeruleaWeight<Test> as WeightInfo>::register_type(),
            <() as WeightInfo>::register_type(),
        );
        holds(
            "register_collection",
            <CeruleaWeight<Test> as WeightInfo>::register_collection(),
            <() as WeightInfo>::register_collection(),
        );
        holds(
            "propose_migration",
            <CeruleaWeight<Test> as WeightInfo>::propose_migration(),
            <() as WeightInfo>::propose_migration(),
        );
        holds(
            "apply_migration",
            <CeruleaWeight<Test> as WeightInfo>::apply_migration(),
            <() as WeightInfo>::apply_migration(),
        );
        holds(
            "revert_migration",
            <CeruleaWeight<Test> as WeightInfo>::revert_migration(),
            <() as WeightInfo>::revert_migration(),
        );
        holds(
            "retire_collection",
            <CeruleaWeight<Test> as WeightInfo>::retire_collection(),
            <() as WeightInfo>::retire_collection(),
        );
    }
}
