//! The weight figures of the audit module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. An append writes the entry, the two indexes, the running digest
//! and the sequence counter, so the figure charges five writes.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the audit module calls charge.
pub trait WeightInfo {
    /// Register one event definition.
    fn define_event() -> Weight;
    /// Append one entry.
    fn emit() -> Weight;
    /// Append one correcting entry.
    fn correct() -> Weight;
    /// Register the interest of one subscriber.
    fn subscribe() -> Weight;
    /// Withdraw the interest of one subscriber.
    fn unsubscribe() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn define_event() -> Weight {
        Weight::from_parts(28_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn emit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn correct() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn subscribe() -> Weight {
        Weight::from_parts(22_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn unsubscribe() -> Weight {
        Weight::from_parts(18_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn define_event() -> Weight {
        Weight::from_parts(28_000_000, 4_000)
    }

    fn emit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn correct() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
    }

    fn subscribe() -> Weight {
        Weight::from_parts(22_000_000, 4_000)
    }

    fn unsubscribe() -> Weight {
        Weight::from_parts(18_000_000, 3_000)
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
            "define_event",
            <CeruleaWeight<Test> as WeightInfo>::define_event(),
            <() as WeightInfo>::define_event(),
        );
        holds(
            "emit",
            <CeruleaWeight<Test> as WeightInfo>::emit(),
            <() as WeightInfo>::emit(),
        );
        holds(
            "correct",
            <CeruleaWeight<Test> as WeightInfo>::correct(),
            <() as WeightInfo>::correct(),
        );
        holds(
            "subscribe",
            <CeruleaWeight<Test> as WeightInfo>::subscribe(),
            <() as WeightInfo>::subscribe(),
        );
        holds(
            "unsubscribe",
            <CeruleaWeight<Test> as WeightInfo>::unsubscribe(),
            <() as WeightInfo>::unsubscribe(),
        );
    }
}
