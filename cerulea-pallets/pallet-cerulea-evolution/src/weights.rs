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
