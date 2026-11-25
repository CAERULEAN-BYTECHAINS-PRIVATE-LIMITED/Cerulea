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
            "install_governance",
            <CeruleaWeight<Test> as WeightInfo>::install_governance(),
            <() as WeightInfo>::install_governance(),
        );
        holds(
            "set_voter_weight",
            <CeruleaWeight<Test> as WeightInfo>::set_voter_weight(),
            <() as WeightInfo>::set_voter_weight(),
        );
        holds(
            "propose",
            <CeruleaWeight<Test> as WeightInfo>::propose(),
            <() as WeightInfo>::propose(),
        );
        holds(
            "vote",
            <CeruleaWeight<Test> as WeightInfo>::vote(),
            <() as WeightInfo>::vote(),
        );
        holds(
            "tally",
            <CeruleaWeight<Test> as WeightInfo>::tally(),
            <() as WeightInfo>::tally(),
        );
        holds(
            "execute",
            <CeruleaWeight<Test> as WeightInfo>::execute(),
            <() as WeightInfo>::execute(),
        );
        holds(
            "cancel",
            <CeruleaWeight<Test> as WeightInfo>::cancel(),
            <() as WeightInfo>::cancel(),
        );
    }
}
