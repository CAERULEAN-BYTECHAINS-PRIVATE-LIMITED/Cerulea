//! The weight figures of the permission module calls.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the permission module calls charge.
pub trait WeightInfo {
    /// Give one role one capability over one collection.
    fn grant() -> Weight;
    /// Take one capability away from one role.
    fn revoke() -> Weight;
    /// Report what one role reaches in one collection.
    fn preview_as_role() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn grant() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn revoke() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn preview_as_role() -> Weight {
        Weight::from_parts(52_000_000, 6_000).saturating_add(RocksDbWeight::get().reads(8))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn grant() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn revoke() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
    }

    fn preview_as_role() -> Weight {
        Weight::from_parts(52_000_000, 6_000)
    }
}
