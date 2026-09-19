//! The weight figures of the tier module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A licence call runs an ed25519 verification over the canonical
//! attestation, which dominates the figure.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the tier module calls charge.
pub trait WeightInfo {
    /// Install the public key of the control plane.
    fn set_control_plane_key() -> Weight;
    /// Install one licence attestation.
    fn install_licence() -> Weight;
    /// Verify the stored licence again.
    fn verify_licence() -> Weight;
    /// Record one unit of usage.
    fn record_usage() -> Weight;
    /// Take one seat of the licence.
    fn take_seat() -> Weight;
    /// Stop block production.
    fn suspend() -> Weight;
    /// Start block production again.
    fn resume() -> Weight;
    /// Check one watched deployment inside the block hook.
    fn sweep_one_deployment() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn set_control_plane_key() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn install_licence() -> Weight {
        Weight::from_parts(210_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(5))
    }

    fn verify_licence() -> Weight {
        Weight::from_parts(200_000_000, 11_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn record_usage() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn take_seat() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn suspend() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(1))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn resume() -> Weight {
        Weight::from_parts(56_000_000, 7_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn sweep_one_deployment() -> Weight {
        Weight::from_parts(9_000_000, 1_800).saturating_add(RocksDbWeight::get().reads(2))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn set_control_plane_key() -> Weight {
        Weight::from_parts(46_000_000, 6_000)
    }

    fn install_licence() -> Weight {
        Weight::from_parts(210_000_000, 12_000)
    }

    fn verify_licence() -> Weight {
        Weight::from_parts(200_000_000, 11_000)
    }

    fn record_usage() -> Weight {
        Weight::from_parts(58_000_000, 7_000)
    }

    fn take_seat() -> Weight {
        Weight::from_parts(54_000_000, 7_000)
    }

    fn suspend() -> Weight {
        Weight::from_parts(50_000_000, 6_000)
    }

    fn resume() -> Weight {
        Weight::from_parts(56_000_000, 7_000)
    }

    fn sweep_one_deployment() -> Weight {
        Weight::from_parts(9_000_000, 1_800)
    }
}
