//! The weight figures of the logic module calls.
//!
//! An action charges for its preconditions, its effects and its events
//! together. A view charges for the rows it reads and the passes it runs
//! over them.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the logic module calls charge.
pub trait WeightInfo {
    /// Install one Blueprint.
    fn install_blueprint() -> Weight;
    /// Evaluate one expression.
    fn evaluate_expression() -> Weight;
    /// Execute one action.
    fn execute_action() -> Weight;
    /// Evaluate one view.
    fn run_view() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn install_blueprint() -> Weight {
        Weight::from_parts(420_000_000, 40_000)
            .saturating_add(RocksDbWeight::get().reads(12))
            .saturating_add(RocksDbWeight::get().writes(24))
    }

    fn evaluate_expression() -> Weight {
        Weight::from_parts(64_000_000, 8_000).saturating_add(RocksDbWeight::get().reads(3))
    }

    fn execute_action() -> Weight {
        Weight::from_parts(310_000_000, 30_000)
            .saturating_add(RocksDbWeight::get().reads(30))
            .saturating_add(RocksDbWeight::get().writes(24))
    }

    fn run_view() -> Weight {
        Weight::from_parts(260_000_000, 26_000).saturating_add(RocksDbWeight::get().reads(40))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn install_blueprint() -> Weight {
        Weight::from_parts(420_000_000, 40_000)
    }

    fn evaluate_expression() -> Weight {
        Weight::from_parts(64_000_000, 8_000)
    }

    fn execute_action() -> Weight {
        Weight::from_parts(310_000_000, 30_000)
    }

    fn run_view() -> Weight {
        Weight::from_parts(260_000_000, 26_000)
    }
}
