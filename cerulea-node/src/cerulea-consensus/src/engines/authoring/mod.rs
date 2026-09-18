//! The Authoring Engine.
//!
//! The Authoring Engine draws the committee for each slot, names the author,
//! seals the block and checks the seal on import. Rank one authors. Ranks two
//! through five stand by in order. Production begins only where the local
//! keystore holds the scheduled author key.

pub mod committee;
pub mod draw;

pub use committee::{weighted_draw, CommitteeDraw, SlotCommittee, STANDING_FLOOR};
pub use draw::{
    draw, draw_from_keystore, draw_value, verify_draw, verify_result, DrawInput, DrawProof,
    DrawResult,
};
