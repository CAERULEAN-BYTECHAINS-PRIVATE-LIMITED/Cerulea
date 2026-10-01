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
            "register_invariant",
            <CeruleaWeight<Test> as WeightInfo>::register_invariant(),
            <() as WeightInfo>::register_invariant(),
        );
        holds(
            "register_precondition",
            <CeruleaWeight<Test> as WeightInfo>::register_precondition(),
            <() as WeightInfo>::register_precondition(),
        );
        holds(
            "retire_rule",
            <CeruleaWeight<Test> as WeightInfo>::retire_rule(),
            <() as WeightInfo>::retire_rule(),
        );
        holds(
            "evaluate_precondition",
            <CeruleaWeight<Test> as WeightInfo>::evaluate_precondition(),
            <() as WeightInfo>::evaluate_precondition(),
        );
        holds(
            "evaluate_invariants",
            <CeruleaWeight<Test> as WeightInfo>::evaluate_invariants(),
            <() as WeightInfo>::evaluate_invariants(),
        );
    }
}
