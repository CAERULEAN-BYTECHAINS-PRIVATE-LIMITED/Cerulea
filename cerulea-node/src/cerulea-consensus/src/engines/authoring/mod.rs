//! The Authoring Engine.
//!
//! The Authoring Engine draws the committee for each slot, names the author,
//! seals the block and checks the seal on import. Rank one authors. Ranks two
//! through five stand by in order. Production begins only where the local
//! keystore holds the scheduled author key.

pub mod committee;
pub mod draw;
pub mod fallback;
pub mod seal;
pub mod verify;

pub use committee::{weighted_draw, CommitteeDraw, SlotCommittee, STANDING_FLOOR};
pub use fallback::{
    AuthoringPolicy, FallbackTracker, RankWindow, SlotSchedule, DEFAULT_RANK_TIMEOUT_MS,
};

pub use seal::{
    check_seal_signature, extract_seal, header_hash, is_cerulea_seal, pre_seal_hash,
    seal_and_apply, seal_header, seal_payload, strip_seal, BlockSeal, SealClaim, ValidatorKeys,
};

pub use verify::{
    verify_header, verify_seal, KeyRegistry, SlotContext, ValidatorRegistry, VerifiedSeal,
};

pub use draw::{
    draw, draw_from_keystore, draw_value, verify_draw, verify_result, DrawInput, DrawProof,
    DrawResult,
};
