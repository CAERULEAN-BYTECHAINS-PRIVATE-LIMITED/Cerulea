//! The weight figures of the relayer module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A relayed submission runs an ed25519 verification, spends the
//! sponsor budget, spends the tenant quota and counts two rate limits, so
//! the figure charges for the whole pass.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the relayer module calls charge.
pub trait WeightInfo {
    /// Register one sponsor account.
    fn register_sponsor() -> Weight;
    /// Set the quota one tenant carries.
    fn set_quota() -> Weight;
    /// Set the rate limit one account carries.
    fn set_account_limit() -> Weight;
    /// Set the rate limit one action carries.
    fn set_action_limit() -> Weight;
    /// Register one signing key.
    fn register_signing_key() -> Weight;
    /// Submit one action on behalf of its authoriser.
    fn submit_on_behalf() -> Weight;
    /// Hold one account back.
    fn throttle() -> Weight;
    /// Let one account submit again.
    fn lift_throttle() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_sponsor() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn set_quota() -> Weight {
        Weight::from_parts(50_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn set_account_limit() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn set_action_limit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn register_signing_key() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn submit_on_behalf() -> Weight {
        Weight::from_parts(260_000_000, 18_000)
            .saturating_add(RocksDbWeight::get().reads(12))
            .saturating_add(RocksDbWeight::get().writes(11))
    }

    fn throttle() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn lift_throttle() -> Weight {
        Weight::from_parts(42_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_sponsor() -> Weight {
        Weight::from_parts(52_000_000, 7_000)
    }

    fn set_quota() -> Weight {
        Weight::from_parts(50_000_000, 7_000)
    }

    fn set_account_limit() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
    }

    fn set_action_limit() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn register_signing_key() -> Weight {
        Weight::from_parts(48_000_000, 6_000)
    }

    fn submit_on_behalf() -> Weight {
        Weight::from_parts(260_000_000, 18_000)
    }

    fn throttle() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
    }

    fn lift_throttle() -> Weight {
        Weight::from_parts(42_000_000, 6_000)
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
            "register_sponsor",
            <CeruleaWeight<Test> as WeightInfo>::register_sponsor(),
            <() as WeightInfo>::register_sponsor(),
        );
        holds(
            "set_quota",
            <CeruleaWeight<Test> as WeightInfo>::set_quota(),
            <() as WeightInfo>::set_quota(),
        );
        holds(
            "set_account_limit",
            <CeruleaWeight<Test> as WeightInfo>::set_account_limit(),
            <() as WeightInfo>::set_account_limit(),
        );
        holds(
            "set_action_limit",
            <CeruleaWeight<Test> as WeightInfo>::set_action_limit(),
            <() as WeightInfo>::set_action_limit(),
        );
        holds(
            "register_signing_key",
            <CeruleaWeight<Test> as WeightInfo>::register_signing_key(),
            <() as WeightInfo>::register_signing_key(),
        );
        holds(
            "submit_on_behalf",
            <CeruleaWeight<Test> as WeightInfo>::submit_on_behalf(),
            <() as WeightInfo>::submit_on_behalf(),
        );
        holds(
            "throttle",
            <CeruleaWeight<Test> as WeightInfo>::throttle(),
            <() as WeightInfo>::throttle(),
        );
        holds(
            "lift_throttle",
            <CeruleaWeight<Test> as WeightInfo>::lift_throttle(),
            <() as WeightInfo>::lift_throttle(),
        );
    }
}
