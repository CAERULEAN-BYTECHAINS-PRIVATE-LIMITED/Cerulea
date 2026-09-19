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
pub mod bounds;
pub mod checkpoint;
pub mod equivocation;

pub use authoring::{
    derive_slot_seed, draw_value, epoch_draw_sign_data, epoch_draw_transcript, rank_seats,
    seat_value, verify_epoch_draw, weighted_draw, CommitteeSeat, SeatInput, COMMITTEE_SIZE,
    DRAW_LABEL, DRAW_OUTPUT_CONTEXT, EPOCH_DOMAIN, SEAT_MIX_DOMAIN, SEED_DERIVATION_DOMAIN,
    STANDING_FLOOR, TRUST_SCORE_FULL,
};

pub use equivocation::{
    verify_equivocation, EquivocationFacts, EquivocationFault, EquivocationProof, SealClaimBytes,
    SealedHead, DRAW_KEY_OFFSET, RANK_OFFSET, SEAL_CLAIM_BYTES, SEAL_CLAIM_PREFIX_BYTES,
    SEAL_DOMAIN, SEAL_KEY_OFFSET, SLOT_OFFSET,
};

pub use checkpoint::{
    accepts_block, accepts_reorganisation, checkpoint_due, checkpoint_index, follows,
    highest_anchored_block, Checkpoint, CheckpointFault, CHECKPOINT_DOMAIN,
    DEFAULT_CHECKPOINT_INTERVAL,
};

pub use bounds::{admit, price, Admitted, BoundFault, MessagePath};
