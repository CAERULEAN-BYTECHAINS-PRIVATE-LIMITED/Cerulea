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
#[allow(clippy::module_inception)]
pub mod commit;
pub mod gossip;
pub mod lock;
pub mod submission;

pub use aggregate::{AggregateSignature, SignerBitmap, SCHEME_ENVELOPE_V1};
pub use commit::{
    crosses, DomainCommitment, DomainCommitter, ThresholdTable, THRESHOLD_FLOOR_PERCENT,
};

pub use gossip::{MessageSet, DEFAULT_MESSAGE_CAPACITY};
pub use lock::{
    blocking_locks, certificate_holds, certificate_release, choose_vote, CastVote, LedgerError,
    LedgerStore, TimeoutStatement, VoteChoice, VoteLedger,
};
pub use submission::{
    Admission, CommitSubmission, DeclaredWeight, SubmissionGate, SubmissionPolicy,
    MAX_SUBMISSION_BYTES, MAX_VOTES_PER_SUBMISSION,
};

pub use collect::{
    cast_vote, check_vote, vote_payload, CommitVote, SignatureCollector, VoteAdmission,
};
