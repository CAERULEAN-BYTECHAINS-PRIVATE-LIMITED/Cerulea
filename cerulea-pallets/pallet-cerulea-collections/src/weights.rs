//! The weight figures of the collections module calls.
//!
//! A class change page rewrites up to the page limit the runtime sets, so
//! the figure for a page covers that many record rewrites.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the collections module calls charge.
pub trait WeightInfo {
    /// Start a privacy class change and rewrite its first page.
    fn set_class() -> Weight;
    /// Rewrite the next page of a class change.
    fn advance_class_change() -> Weight;
    /// Give one role the content key of one collection.
    fn distribute_key() -> Weight;
    /// Take the content key away from one role.
    fn revoke_access() -> Weight;
    /// Rotate the content key of one sealed collection.
    fn rotate_key() -> Weight;
    /// Check one payload against a stored commitment.
    fn verify_commitment() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn set_class() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(40))
            .saturating_add(RocksDbWeight::get().writes(32))
    }

    fn advance_class_change() -> Weight {
        Weight::from_parts(230_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(38))
            .saturating_add(RocksDbWeight::get().writes(30))
    }

    fn distribute_key() -> Weight {
        Weight::from_parts(56_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn revoke_access() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn rotate_key() -> Weight {
        Weight::from_parts(280_000_000, 28_000)
            .saturating_add(RocksDbWeight::get().reads(46))
            .saturating_add(RocksDbWeight::get().writes(40))
    }

    fn verify_commitment() -> Weight {
        Weight::from_parts(34_000_000, 4_000).saturating_add(RocksDbWeight::get().reads(2))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn set_class() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
    }

    fn advance_class_change() -> Weight {
        Weight::from_parts(230_000_000, 24_000)
    }

    fn distribute_key() -> Weight {
        Weight::from_parts(56_000_000, 6_000)
    }

    fn revoke_access() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
    }

    fn rotate_key() -> Weight {
        Weight::from_parts(280_000_000, 28_000)
    }

    fn verify_commitment() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
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
            "set_class",
            <CeruleaWeight<Test> as WeightInfo>::set_class(),
            <() as WeightInfo>::set_class(),
        );
        holds(
            "advance_class_change",
            <CeruleaWeight<Test> as WeightInfo>::advance_class_change(),
            <() as WeightInfo>::advance_class_change(),
        );
        holds(
            "distribute_key",
            <CeruleaWeight<Test> as WeightInfo>::distribute_key(),
            <() as WeightInfo>::distribute_key(),
        );
        holds(
            "revoke_access",
            <CeruleaWeight<Test> as WeightInfo>::revoke_access(),
            <() as WeightInfo>::revoke_access(),
        );
        holds(
            "rotate_key",
            <CeruleaWeight<Test> as WeightInfo>::rotate_key(),
            <() as WeightInfo>::rotate_key(),
        );
        holds(
            "verify_commitment",
            <CeruleaWeight<Test> as WeightInfo>::verify_commitment(),
            <() as WeightInfo>::verify_commitment(),
        );
    }
}
