//! Shared primitives of the Dynamic Consensus Framework.
//!
//! The Dynamic Consensus Framework is the agreement mechanism of the Cerulea
//! Blockchain. The node side and the pallet set both need the slot seed, the
//! draw arithmetic and the committee ranking. One crate owns each derivation,
//! so the two sides always reach the same answer.
//!
//! The crate works in `no_std`. The runtime compiles it into WebAssembly. The
//! node compiles the same code natively.

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

extern crate alloc;

pub mod authoring;

pub use authoring::{
    derive_slot_seed, draw_value, epoch_draw_sign_data, epoch_draw_transcript, rank_seats,
    seat_value, verify_epoch_draw, weighted_draw, CommitteeSeat, SeatInput, COMMITTEE_SIZE,
    DRAW_LABEL, DRAW_OUTPUT_CONTEXT, EPOCH_DOMAIN, SEAT_MIX_DOMAIN, SEED_DERIVATION_DOMAIN,
    STANDING_FLOOR, TRUST_SCORE_FULL,
};
