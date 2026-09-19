//! The weight figures of the oracle module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A submission reads the feed, decodes the value, joins or opens a
//! round and records the attestation.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the oracle module calls charge.
pub trait WeightInfo {
    /// Register one feed.
    fn register_feed() -> Weight;
    /// Retire one feed.
    fn retire_feed() -> Weight;
    /// Submit one attested value.
    fn submit_value() -> Weight;
    /// Raise one dispute.
    fn dispute() -> Weight;
    /// Resolve one dispute.
    fn resolve() -> Weight;
    /// Promote the round that passed its window.
    fn settle() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_feed() -> Weight {
        Weight::from_parts(62_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn retire_feed() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn submit_value() -> Weight {
        Weight::from_parts(104_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(7))
    }

    fn dispute() -> Weight {
        Weight::from_parts(66_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn resolve() -> Weight {
        Weight::from_parts(80_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn settle() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_feed() -> Weight {
        Weight::from_parts(62_000_000, 8_000)
    }

    fn retire_feed() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
    }

    fn submit_value() -> Weight {
        Weight::from_parts(104_000_000, 12_000)
    }

    fn dispute() -> Weight {
        Weight::from_parts(66_000_000, 8_000)
    }

    fn resolve() -> Weight {
        Weight::from_parts(80_000_000, 9_000)
    }

    fn settle() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
    }
}
