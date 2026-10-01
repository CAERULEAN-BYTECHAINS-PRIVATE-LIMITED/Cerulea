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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Test;
    use frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND;

    /// The reference time one block of the Cerulea chain spends.
    const BLOCK_REF_TIME: u64 = 2 * WEIGHT_REF_TIME_PER_SECOND;

    /// Check one measured figure against the figure a test runtime charges.
    ///
    /// The measured figure carries the storage charge the call performs, so
    /// it sits above the plain reference time in at least one part. Every
    /// figure fits inside one block.
    fn holds(call: &str, measured: Weight, plain: Weight) {
        assert!(
            measured.ref_time() > 0,
            "the figure of {call} charges reference time"
        );
        assert!(
            measured.proof_size() > 0,
            "the figure of {call} charges proof size"
        );
        assert!(
            measured.ref_time() <= BLOCK_REF_TIME,
            "the figure of {call} fits inside one block"
        );
        assert!(
            measured.ref_time() >= plain.ref_time(),
            "the figure of {call} charges the reference time a test runtime charges"
        );
        assert!(
            measured.proof_size() >= plain.proof_size(),
            "the figure of {call} charges the proof size a test runtime charges"
        );
        assert_ne!(
            measured, plain,
            "the figure of {call} carries the storage charge a test runtime lacks"
        );
        assert!(
            plain.ref_time() > 0,
            "the test runtime figure of {call} charges reference time"
        );
    }

    #[test]
    fn every_measured_figure_carries_the_storage_charge_its_call_performs() {
        holds(
            "install_blueprint",
            <CeruleaWeight<Test> as WeightInfo>::install_blueprint(),
            <() as WeightInfo>::install_blueprint(),
        );
        holds(
            "evaluate_expression",
            <CeruleaWeight<Test> as WeightInfo>::evaluate_expression(),
            <() as WeightInfo>::evaluate_expression(),
        );
        holds(
            "execute_action",
            <CeruleaWeight<Test> as WeightInfo>::execute_action(),
            <() as WeightInfo>::execute_action(),
        );
        holds(
            "run_view",
            <CeruleaWeight<Test> as WeightInfo>::run_view(),
            <() as WeightInfo>::run_view(),
        );
    }
}
