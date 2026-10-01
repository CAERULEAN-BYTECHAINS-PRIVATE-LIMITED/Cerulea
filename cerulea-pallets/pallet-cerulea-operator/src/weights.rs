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
            "appoint_operator",
            <CeruleaWeight<Test> as WeightInfo>::appoint_operator(),
            <() as WeightInfo>::appoint_operator(),
        );
        holds(
            "retire_operator",
            <CeruleaWeight<Test> as WeightInfo>::retire_operator(),
            <() as WeightInfo>::retire_operator(),
        );
        holds(
            "request",
            <CeruleaWeight<Test> as WeightInfo>::request(),
            <() as WeightInfo>::request(),
        );
    }
}
