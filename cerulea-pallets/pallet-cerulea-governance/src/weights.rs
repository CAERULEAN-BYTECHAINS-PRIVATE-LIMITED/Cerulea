//! The weight figures of the governance module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A tally reads the proposal, the quorum rule and the weight of
//! the electorate, then writes the outcome and the audit record.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the governance module calls charge.
pub trait WeightInfo {
    /// Install the Governance section of the Blueprint.
    fn install_governance() -> Weight;
    /// Give one account its weight in the electorate.
    fn set_voter_weight() -> Weight;
    /// Open one proposal.
    fn propose() -> Weight;
    /// Cast one weighted vote.
    fn vote() -> Weight;
    /// Settle one proposal against its quorum.
    fn tally() -> Weight;
    /// Record that a module carried one approved change out.
    fn execute() -> Weight;
    /// Withdraw one open proposal.
    fn cancel() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn install_governance() -> Weight {
        Weight::from_parts(88_000_000, 10_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(7))
    }

    fn set_voter_weight() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn propose() -> Weight {
        Weight::from_parts(78_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn vote() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn tally() -> Weight {
        Weight::from_parts(74_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(5))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn execute() -> Weight {
        Weight::from_parts(68_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn cancel() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn install_governance() -> Weight {
        Weight::from_parts(88_000_000, 10_000)
    }

    fn set_voter_weight() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn propose() -> Weight {
        Weight::from_parts(78_000_000, 9_000)
    }

    fn vote() -> Weight {
        Weight::from_parts(70_000_000, 8_000)
    }

    fn tally() -> Weight {
        Weight::from_parts(74_000_000, 9_000)
    }

    fn execute() -> Weight {
        Weight::from_parts(68_000_000, 8_000)
    }

    fn cancel() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
    }
}
