//! The pallet-side Admission Engine of the Dynamic Consensus Framework.
//!
//! The Admission Engine governs entry to and exit from the validator set.
//! Admission runs continuously rather than per block. A validator holds a seat
//! while meeting every condition. Losing a condition triggers demotion at the
//! next epoch boundary.

pub mod admission;
