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
