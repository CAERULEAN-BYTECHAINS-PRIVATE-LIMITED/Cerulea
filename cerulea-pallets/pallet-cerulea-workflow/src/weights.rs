//! The weight figures of the workflow module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A transition charges for the role check, the permission
//! resolution, the guard evaluation, the record write and the schedule
//! maintenance together.
//!
//! The block hook charges an idle figure plus one figure per scheduled
//! step and one figure per compensating step. The counts carry hard bounds
//! from the runtime, so the hook never runs without bound.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the workflow module calls charge.
pub trait WeightInfo {
    /// Install one machine definition.
    fn install_machine() -> Weight;
    /// Remove one machine definition.
    fn retire_machine() -> Weight;
    /// Start one process.
    fn start_instance() -> Weight;
    /// Attempt one transition.
    fn attempt_transition() -> Weight;
    /// Fire one due timer.
    fn fire_timer() -> Weight;
    /// Escalate one passed deadline.
    fn escalate_deadline() -> Weight;
    /// Start the compensating steps of one process.
    fn compensate() -> Weight;
    /// The reads the block hook performs while it finds no work.
    fn on_initialize_idle() -> Weight;
    /// Run one scheduled step inside the block hook.
    fn run_scheduled_step() -> Weight;
    /// Run one compensating step inside the block hook.
    fn compensate_step() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn install_machine() -> Weight {
        Weight::from_parts(140_000_000, 14_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn retire_machine() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn start_instance() -> Weight {
        Weight::from_parts(180_000_000, 16_000)
            .saturating_add(RocksDbWeight::get().reads(9))
            .saturating_add(RocksDbWeight::get().writes(9))
    }

    fn attempt_transition() -> Weight {
        Weight::from_parts(210_000_000, 18_000)
            .saturating_add(RocksDbWeight::get().reads(11))
            .saturating_add(RocksDbWeight::get().writes(9))
    }

    fn fire_timer() -> Weight {
        Weight::from_parts(190_000_000, 16_000)
            .saturating_add(RocksDbWeight::get().reads(9))
            .saturating_add(RocksDbWeight::get().writes(8))
    }

    fn escalate_deadline() -> Weight {
        Weight::from_parts(195_000_000, 16_000)
            .saturating_add(RocksDbWeight::get().reads(9))
            .saturating_add(RocksDbWeight::get().writes(8))
    }

    fn compensate() -> Weight {
        Weight::from_parts(120_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn on_initialize_idle() -> Weight {
        Weight::from_parts(8_000_000, 1_500).saturating_add(RocksDbWeight::get().reads(2))
    }

    fn run_scheduled_step() -> Weight {
        Weight::from_parts(165_000_000, 15_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(8))
    }

    fn compensate_step() -> Weight {
        Weight::from_parts(150_000_000, 14_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(7))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn install_machine() -> Weight {
        Weight::from_parts(140_000_000, 14_000)
    }

    fn retire_machine() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
    }

    fn start_instance() -> Weight {
        Weight::from_parts(180_000_000, 16_000)
    }

    fn attempt_transition() -> Weight {
        Weight::from_parts(210_000_000, 18_000)
    }

    fn fire_timer() -> Weight {
        Weight::from_parts(190_000_000, 16_000)
    }

    fn escalate_deadline() -> Weight {
        Weight::from_parts(195_000_000, 16_000)
    }

    fn compensate() -> Weight {
        Weight::from_parts(120_000_000, 12_000)
    }

    fn on_initialize_idle() -> Weight {
        Weight::from_parts(8_000_000, 1_500)
    }

    fn run_scheduled_step() -> Weight {
        Weight::from_parts(165_000_000, 15_000)
    }

    fn compensate_step() -> Weight {
        Weight::from_parts(150_000_000, 14_000)
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
            "install_machine",
            <CeruleaWeight<Test> as WeightInfo>::install_machine(),
            <() as WeightInfo>::install_machine(),
        );
        holds(
            "retire_machine",
            <CeruleaWeight<Test> as WeightInfo>::retire_machine(),
            <() as WeightInfo>::retire_machine(),
        );
        holds(
            "start_instance",
            <CeruleaWeight<Test> as WeightInfo>::start_instance(),
            <() as WeightInfo>::start_instance(),
        );
        holds(
            "attempt_transition",
            <CeruleaWeight<Test> as WeightInfo>::attempt_transition(),
            <() as WeightInfo>::attempt_transition(),
        );
        holds(
            "fire_timer",
            <CeruleaWeight<Test> as WeightInfo>::fire_timer(),
            <() as WeightInfo>::fire_timer(),
        );
        holds(
            "escalate_deadline",
            <CeruleaWeight<Test> as WeightInfo>::escalate_deadline(),
            <() as WeightInfo>::escalate_deadline(),
        );
        holds(
            "compensate",
            <CeruleaWeight<Test> as WeightInfo>::compensate(),
            <() as WeightInfo>::compensate(),
        );
        holds(
            "on_initialize_idle",
            <CeruleaWeight<Test> as WeightInfo>::on_initialize_idle(),
            <() as WeightInfo>::on_initialize_idle(),
        );
        holds(
            "run_scheduled_step",
            <CeruleaWeight<Test> as WeightInfo>::run_scheduled_step(),
            <() as WeightInfo>::run_scheduled_step(),
        );
        holds(
            "compensate_step",
            <CeruleaWeight<Test> as WeightInfo>::compensate_step(),
            <() as WeightInfo>::compensate_step(),
        );
    }
}
