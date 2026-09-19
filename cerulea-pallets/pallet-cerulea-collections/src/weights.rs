//! The weight figures of the collections module calls.
//!
//! A class change page rewrites up to the page limit the runtime sets, so
//! the figure for a page covers that many record rewrites.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the collections module calls charge.
pub trait WeightInfo {
    /// Start a privacy class change and rewrite its first page.
    fn set_class() -> Weight;
    /// Rewrite the next page of a class change.
    fn advance_class_change() -> Weight;
    /// Give one role the content key of one collection.
    fn distribute_key() -> Weight;
    /// Take the content key away from one role.
    fn revoke_access() -> Weight;
    /// Check one payload against a stored commitment.
    fn verify_commitment() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn set_class() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(40))
            .saturating_add(RocksDbWeight::get().writes(32))
    }

    fn advance_class_change() -> Weight {
        Weight::from_parts(230_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(38))
            .saturating_add(RocksDbWeight::get().writes(30))
    }

    fn distribute_key() -> Weight {
        Weight::from_parts(56_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn revoke_access() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn verify_commitment() -> Weight {
        Weight::from_parts(34_000_000, 4_000).saturating_add(RocksDbWeight::get().reads(2))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn set_class() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
    }

    fn advance_class_change() -> Weight {
        Weight::from_parts(230_000_000, 24_000)
    }

    fn distribute_key() -> Weight {
        Weight::from_parts(56_000_000, 6_000)
    }

    fn revoke_access() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
    }

    fn verify_commitment() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
    }
}
