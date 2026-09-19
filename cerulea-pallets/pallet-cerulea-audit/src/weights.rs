//! The weight figures of the audit module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. An append writes the entry, the two indexes, the running digest
//! and the sequence counter, so the figure charges five writes.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the audit module calls charge.
pub trait WeightInfo {
    /// Register one event definition.
    fn define_event() -> Weight;
    /// Append one entry.
    fn emit() -> Weight;
    /// Append one correcting entry.
    fn correct() -> Weight;
    /// Register the interest of one subscriber.
    fn subscribe() -> Weight;
    /// Withdraw the interest of one subscriber.
    fn unsubscribe() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn define_event() -> Weight {
        Weight::from_parts(28_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn emit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn correct() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn subscribe() -> Weight {
        Weight::from_parts(22_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn unsubscribe() -> Weight {
        Weight::from_parts(18_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn define_event() -> Weight {
        Weight::from_parts(28_000_000, 4_000)
    }

    fn emit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn correct() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
    }

    fn subscribe() -> Weight {
        Weight::from_parts(22_000_000, 4_000)
    }

    fn unsubscribe() -> Weight {
        Weight::from_parts(18_000_000, 3_000)
    }
}
