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
