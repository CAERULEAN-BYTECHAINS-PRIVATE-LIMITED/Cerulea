//! The weight figures of the operator module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A request resolves permission, runs the platform step and writes
//! the request record together with the audit record.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the operator module calls charge.
pub trait WeightInfo {
    /// Appoint one account to the role.
    fn appoint_operator() -> Weight;
    /// Take one account out of the role.
    fn retire_operator() -> Weight;
    /// Handle one operator request.
    fn request() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn appoint_operator() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn retire_operator() -> Weight {
        Weight::from_parts(50_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn request() -> Weight {
        Weight::from_parts(180_000_000, 14_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(8))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn appoint_operator() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
    }

    fn retire_operator() -> Weight {
        Weight::from_parts(50_000_000, 7_000)
    }

    fn request() -> Weight {
        Weight::from_parts(180_000_000, 14_000)
    }
}
