//! The weight figures of the evolution module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. An apply call rewrites the record type, rewrites the collection
//! definition, raises the version and rewrites every stored record, so the
//! figure charges for the whole pass.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the evolution module calls charge.
pub trait WeightInfo {
    /// Record one version of the deployment.
    fn record_version() -> Weight;
    /// Plan one migration.
    fn plan_migration() -> Weight;
    /// Run one plan without changing anything.
    fn dry_run() -> Weight;
    /// Apply one plan.
    fn apply() -> Weight;
    /// Confirm the shape one applied plan produced.
    fn verify() -> Weight;
    /// Undo one applied plan inside its window.
    fn roll_back() -> Weight;
    /// Move the deployment to another platform version.
    fn set_platform_version() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn record_version() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn plan_migration() -> Weight {
        Weight::from_parts(96_000_000, 11_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn dry_run() -> Weight {
        Weight::from_parts(150_000_000, 14_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn apply() -> Weight {
        Weight::from_parts(420_000_000, 30_000)
            .saturating_add(RocksDbWeight::get().reads(14))
            .saturating_add(RocksDbWeight::get().writes(14))
    }

    fn verify() -> Weight {
        Weight::from_parts(84_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn roll_back() -> Weight {
        Weight::from_parts(400_000_000, 28_000)
            .saturating_add(RocksDbWeight::get().reads(13))
            .saturating_add(RocksDbWeight::get().writes(13))
    }

    fn set_platform_version() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn record_version() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
    }

    fn plan_migration() -> Weight {
        Weight::from_parts(96_000_000, 11_000)
    }

    fn dry_run() -> Weight {
        Weight::from_parts(150_000_000, 14_000)
    }

    fn apply() -> Weight {
        Weight::from_parts(420_000_000, 30_000)
    }

    fn verify() -> Weight {
        Weight::from_parts(84_000_000, 9_000)
    }

    fn roll_back() -> Weight {
        Weight::from_parts(400_000_000, 28_000)
    }

    fn set_platform_version() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
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
            "record_version",
            <CeruleaWeight<Test> as WeightInfo>::record_version(),
            <() as WeightInfo>::record_version(),
        );
        holds(
            "plan_migration",
            <CeruleaWeight<Test> as WeightInfo>::plan_migration(),
            <() as WeightInfo>::plan_migration(),
        );
        holds(
            "dry_run",
            <CeruleaWeight<Test> as WeightInfo>::dry_run(),
            <() as WeightInfo>::dry_run(),
        );
        holds(
            "apply",
            <CeruleaWeight<Test> as WeightInfo>::apply(),
            <() as WeightInfo>::apply(),
        );
        holds(
            "verify",
            <CeruleaWeight<Test> as WeightInfo>::verify(),
            <() as WeightInfo>::verify(),
        );
        holds(
            "roll_back",
            <CeruleaWeight<Test> as WeightInfo>::roll_back(),
            <() as WeightInfo>::roll_back(),
        );
        holds(
            "set_platform_version",
            <CeruleaWeight<Test> as WeightInfo>::set_platform_version(),
            <() as WeightInfo>::set_platform_version(),
        );
    }
}
