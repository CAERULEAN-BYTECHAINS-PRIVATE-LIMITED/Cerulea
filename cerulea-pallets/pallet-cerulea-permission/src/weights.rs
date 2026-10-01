//! The weight figures of the permission module calls.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the permission module calls charge.
pub trait WeightInfo {
    /// Give one role one capability over one collection.
    fn grant() -> Weight;
    /// Take one capability away from one role.
    fn revoke() -> Weight;
    /// Report what one role reaches in one collection.
    fn preview_as_role() -> Weight;
    /// Report what one role sees of one sample record.
    fn preview_record_as_role() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn grant() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn revoke() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn preview_as_role() -> Weight {
        Weight::from_parts(52_000_000, 6_000).saturating_add(RocksDbWeight::get().reads(8))
    }

    fn preview_record_as_role() -> Weight {
        Weight::from_parts(64_000_000, 7_000).saturating_add(RocksDbWeight::get().reads(3))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn grant() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn revoke() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
    }

    fn preview_as_role() -> Weight {
        Weight::from_parts(52_000_000, 6_000)
    }

    fn preview_record_as_role() -> Weight {
        Weight::from_parts(64_000_000, 7_000)
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
            "grant",
            <CeruleaWeight<Test> as WeightInfo>::grant(),
            <() as WeightInfo>::grant(),
        );
        holds(
            "revoke",
            <CeruleaWeight<Test> as WeightInfo>::revoke(),
            <() as WeightInfo>::revoke(),
        );
        holds(
            "preview_as_role",
            <CeruleaWeight<Test> as WeightInfo>::preview_as_role(),
            <() as WeightInfo>::preview_as_role(),
        );
        holds(
            "preview_record_as_role",
            <CeruleaWeight<Test> as WeightInfo>::preview_record_as_role(),
            <() as WeightInfo>::preview_record_as_role(),
        );
    }
}
