//! The weight figures of the identity module calls.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the identity module calls charge.
pub trait WeightInfo {
    /// Register one account.
    fn register_account() -> Weight;
    /// Stop one account from acting.
    fn suspend_account() -> Weight;
    /// Create one organisational unit.
    fn create_unit() -> Weight;
    /// Define one role.
    fn define_role() -> Weight;
    /// Give one account one role.
    fn assign_role() -> Weight;
    /// Take one role away from one account.
    fn revoke_role() -> Weight;
    /// Attest one credential.
    fn attest_credential() -> Weight;
    /// Withdraw one credential.
    fn revoke_credential() -> Weight;
    /// Pass a scope from one account to another.
    fn delegate() -> Weight;
    /// Withdraw one delegation.
    fn revoke_delegation() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_account() -> Weight {
        Weight::from_parts(26_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn suspend_account() -> Weight {
        Weight::from_parts(24_000_000, 3_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn create_unit() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn define_role() -> Weight {
        Weight::from_parts(38_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn assign_role() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn revoke_role() -> Weight {
        Weight::from_parts(36_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn attest_credential() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn revoke_credential() -> Weight {
        Weight::from_parts(32_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }

    fn delegate() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn revoke_delegation() -> Weight {
        Weight::from_parts(34_000_000, 4_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(1))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_account() -> Weight {
        Weight::from_parts(26_000_000, 3_000)
    }

    fn suspend_account() -> Weight {
        Weight::from_parts(24_000_000, 3_000)
    }

    fn create_unit() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
    }

    fn define_role() -> Weight {
        Weight::from_parts(38_000_000, 5_000)
    }

    fn assign_role() -> Weight {
        Weight::from_parts(44_000_000, 6_000)
    }

    fn revoke_role() -> Weight {
        Weight::from_parts(36_000_000, 5_000)
    }

    fn attest_credential() -> Weight {
        Weight::from_parts(40_000_000, 5_000)
    }

    fn revoke_credential() -> Weight {
        Weight::from_parts(32_000_000, 4_000)
    }

    fn delegate() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
    }

    fn revoke_delegation() -> Weight {
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
            "register_account",
            <CeruleaWeight<Test> as WeightInfo>::register_account(),
            <() as WeightInfo>::register_account(),
        );
        holds(
            "suspend_account",
            <CeruleaWeight<Test> as WeightInfo>::suspend_account(),
            <() as WeightInfo>::suspend_account(),
        );
        holds(
            "create_unit",
            <CeruleaWeight<Test> as WeightInfo>::create_unit(),
            <() as WeightInfo>::create_unit(),
        );
        holds(
            "define_role",
            <CeruleaWeight<Test> as WeightInfo>::define_role(),
            <() as WeightInfo>::define_role(),
        );
        holds(
            "assign_role",
            <CeruleaWeight<Test> as WeightInfo>::assign_role(),
            <() as WeightInfo>::assign_role(),
        );
        holds(
            "revoke_role",
            <CeruleaWeight<Test> as WeightInfo>::revoke_role(),
            <() as WeightInfo>::revoke_role(),
        );
        holds(
            "attest_credential",
            <CeruleaWeight<Test> as WeightInfo>::attest_credential(),
            <() as WeightInfo>::attest_credential(),
        );
        holds(
            "revoke_credential",
            <CeruleaWeight<Test> as WeightInfo>::revoke_credential(),
            <() as WeightInfo>::revoke_credential(),
        );
        holds(
            "delegate",
            <CeruleaWeight<Test> as WeightInfo>::delegate(),
            <() as WeightInfo>::delegate(),
        );
        holds(
            "revoke_delegation",
            <CeruleaWeight<Test> as WeightInfo>::revoke_delegation(),
            <() as WeightInfo>::revoke_delegation(),
        );
    }
}
