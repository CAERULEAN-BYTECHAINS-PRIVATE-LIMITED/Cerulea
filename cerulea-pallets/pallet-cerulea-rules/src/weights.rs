//! The weight figures of the rules module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. An evaluation charges for the registry read, the expression
//! evaluation under its step budget and the audit record together.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the rules module calls charge.
pub trait WeightInfo {
    /// Register one invariant.
    fn register_invariant() -> Weight;
    /// Register one precondition.
    fn register_precondition() -> Weight;
    /// Remove one registered rule.
    fn retire_rule() -> Weight;
    /// Evaluate one precondition.
    fn evaluate_precondition() -> Weight;
    /// Evaluate every invariant of one collection.
    fn evaluate_invariants() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_invariant() -> Weight {
        Weight::from_parts(72_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn register_precondition() -> Weight {
        Weight::from_parts(64_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn retire_rule() -> Weight {
        Weight::from_parts(56_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn evaluate_precondition() -> Weight {
        Weight::from_parts(95_000_000, 10_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn evaluate_invariants() -> Weight {
        Weight::from_parts(130_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(2))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_invariant() -> Weight {
        Weight::from_parts(72_000_000, 9_000)
    }

    fn register_precondition() -> Weight {
        Weight::from_parts(64_000_000, 8_000)
    }

    fn retire_rule() -> Weight {
        Weight::from_parts(56_000_000, 7_000)
    }

    fn evaluate_precondition() -> Weight {
        Weight::from_parts(95_000_000, 10_000)
    }

    fn evaluate_invariants() -> Weight {
        Weight::from_parts(130_000_000, 12_000)
    }
}
