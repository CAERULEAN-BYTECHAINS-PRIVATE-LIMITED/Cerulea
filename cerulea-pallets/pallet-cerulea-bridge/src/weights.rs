//! The weight figures of the bridge module calls.
//!
//! Each figure carries a reference time in picoseconds and a proof size in
//! bytes. A delivery decodes a proof, walks a merkle structure, checks
//! signatures and runs an action, so a delivery carries the largest
//! figure of the module.

use frame_support::weights::{constants::RocksDbWeight, Weight};

/// The weight figures the bridge module calls charge.
pub trait WeightInfo {
    /// Register one endpoint.
    fn register_endpoint() -> Weight;
    /// Retire one endpoint.
    fn retire_endpoint() -> Weight;
    /// Submit one batch of far network headers.
    fn submit_headers() -> Weight;
    /// Deliver one inbound message with its proof.
    fn deliver_message() -> Weight;
    /// Produce one outbound message.
    fn dispatch_outbound() -> Weight;
    /// Record the delivery of one outbound message.
    fn confirm_outbound() -> Weight;
    /// Request one state root anchor.
    fn request_anchor() -> Weight;
    /// Confirm one state root anchor.
    fn confirm_anchor() -> Weight;
    /// Cancel one message.
    fn cancel_message() -> Weight;
    /// Sweep one expired message.
    fn sweep_one() -> Weight;
}

/// The measured weight figures.
pub struct CeruleaWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for CeruleaWeight<T> {
    fn register_endpoint() -> Weight {
        Weight::from_parts(78_000_000, 12_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn retire_endpoint() -> Weight {
        Weight::from_parts(52_000_000, 8_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(2))
    }

    fn submit_headers() -> Weight {
        Weight::from_parts(420_000_000, 40_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(36))
    }

    fn deliver_message() -> Weight {
        Weight::from_parts(900_000_000, 60_000)
            .saturating_add(RocksDbWeight::get().reads(12))
            .saturating_add(RocksDbWeight::get().writes(10))
    }

    fn dispatch_outbound() -> Weight {
        Weight::from_parts(180_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(8))
            .saturating_add(RocksDbWeight::get().writes(6))
    }

    fn confirm_outbound() -> Weight {
        Weight::from_parts(62_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn request_anchor() -> Weight {
        Weight::from_parts(96_000_000, 14_000)
            .saturating_add(RocksDbWeight::get().reads(4))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn confirm_anchor() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
            .saturating_add(RocksDbWeight::get().reads(6))
            .saturating_add(RocksDbWeight::get().writes(4))
    }

    fn cancel_message() -> Weight {
        Weight::from_parts(60_000_000, 9_000)
            .saturating_add(RocksDbWeight::get().reads(3))
            .saturating_add(RocksDbWeight::get().writes(3))
    }

    fn sweep_one() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
            .saturating_add(RocksDbWeight::get().reads(2))
            .saturating_add(RocksDbWeight::get().writes(3))
    }
}

/// The figures a test runtime charges.
impl WeightInfo for () {
    fn register_endpoint() -> Weight {
        Weight::from_parts(78_000_000, 12_000)
    }

    fn retire_endpoint() -> Weight {
        Weight::from_parts(52_000_000, 8_000)
    }

    fn submit_headers() -> Weight {
        Weight::from_parts(420_000_000, 40_000)
    }

    fn deliver_message() -> Weight {
        Weight::from_parts(900_000_000, 60_000)
    }

    fn dispatch_outbound() -> Weight {
        Weight::from_parts(180_000_000, 24_000)
    }

    fn confirm_outbound() -> Weight {
        Weight::from_parts(62_000_000, 9_000)
    }

    fn request_anchor() -> Weight {
        Weight::from_parts(96_000_000, 14_000)
    }

    fn confirm_anchor() -> Weight {
        Weight::from_parts(240_000_000, 24_000)
    }

    fn cancel_message() -> Weight {
        Weight::from_parts(60_000_000, 9_000)
    }

    fn sweep_one() -> Weight {
        Weight::from_parts(40_000_000, 6_000)
    }
}
