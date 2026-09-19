//! The Commit Engine.
//!
//! The Commit Engine reaches agreement and makes blocks irreversible. The
//! author gathers one signature from each validator during the slot, combines
//! them into one aggregate beside a signer bitmap, and includes both in the
//! block. A domain commits when the aggregated Trust Score weight crosses the
//! threshold for the highest transaction class present in that domain.
//!
//! Every input carries a bound. Vote submission carries a maximum count
//! enforced before pool admission. The propagation message set carries a
//! capacity limit with least recently used eviction.

pub mod aggregate;
pub mod collect;

pub use aggregate::{AggregateSignature, SignerBitmap, SCHEME_ENVELOPE_V1};
pub use collect::{
    cast_vote, check_vote, vote_payload, CommitVote, SignatureCollector, VoteAdmission,
};
