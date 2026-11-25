//! The weight figures of the state module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. The figures below come from the benchmark set in this crate. A
//! write charges for the permission resolution, the schema check, the
//! record write and the index maintenance together.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the state module calls charge.
pub trait WeightInfo {
    /// Add one record.
    fn insert() -> Weight;
    /// Change one record.
    fn update() -> Weight;
    /// Remove one record.
    fn delete() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn insert() -> Weight {
        Weight::from_parts(84_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(6))
    }

    fn update() -> Weight {
        Weight::from_parts(92_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(9))
            .saturating_add(RocksDbWeight::get().writes(6))
    }

    fn delete() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(5))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn insert() -> Weight {
        Weight::from_parts(84_000_000, 9_000)
    }

    fn update() -> Weight {
        Weight::from_parts(92_000_000, 9_000)
    }

    fn delete() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
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
            "insert",
            <CeruleaWeight<Test> as WeightInfo>::insert(),
            <() as WeightInfo>::insert(),
        );
        holds(
            "update",
            <CeruleaWeight<Test> as WeightInfo>::update(),
            <() as WeightInfo>::update(),
        );
        holds(
            "delete",
            <CeruleaWeight<Test> as WeightInfo>::delete(),
            <() as WeightInfo>::delete(),
        );
    }
}
