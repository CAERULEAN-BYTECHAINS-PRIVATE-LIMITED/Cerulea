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
