//! The weight figures of the oracle module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A submission reads the feed, decodes the value, joins or opens a
//! round and records the attestation.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the oracle module calls charge.
pub trait WeightInfo {
    /// Register one feed.
    fn register_feed() -> Weight;
    /// Retire one feed.
    fn retire_feed() -> Weight;
    /// Submit one attested value.
    fn submit_value() -> Weight;
    /// Raise one dispute.
    fn dispute() -> Weight;
    /// Resolve one dispute.
    fn resolve() -> Weight;
    /// Promote the round that passed its window.
    fn settle() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_feed() -> Weight {
        Weight::from_parts(62_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn retire_feed() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn submit_value() -> Weight {
        Weight::from_parts(104_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(7))
            .saturating_add(RocksDbWeight::get().writes(7))
    }

    fn dispute() -> Weight {
        Weight::from_parts(66_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn resolve() -> Weight {
        Weight::from_parts(80_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn settle() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_feed() -> Weight {
        Weight::from_parts(62_000_000, 8_000)
    }

    fn retire_feed() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
    }

    fn submit_value() -> Weight {
        Weight::from_parts(104_000_000, 12_000)
    }

    fn dispute() -> Weight {
        Weight::from_parts(66_000_000, 8_000)
    }

    fn resolve() -> Weight {
        Weight::from_parts(80_000_000, 9_000)
    }

    fn settle() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
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
            "register_feed",
            <CeruleaWeight<Test> as WeightInfo>::register_feed(),
            <() as WeightInfo>::register_feed(),
        );
        holds(
            "retire_feed",
            <CeruleaWeight<Test> as WeightInfo>::retire_feed(),
            <() as WeightInfo>::retire_feed(),
        );
        holds(
            "submit_value",
            <CeruleaWeight<Test> as WeightInfo>::submit_value(),
            <() as WeightInfo>::submit_value(),
        );
        holds(
            "dispute",
            <CeruleaWeight<Test> as WeightInfo>::dispute(),
            <() as WeightInfo>::dispute(),
        );
        holds(
            "resolve",
            <CeruleaWeight<Test> as WeightInfo>::resolve(),
            <() as WeightInfo>::resolve(),
        );
        holds(
            "settle",
            <CeruleaWeight<Test> as WeightInfo>::settle(),
            <() as WeightInfo>::settle(),
        );
    }
}
