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
