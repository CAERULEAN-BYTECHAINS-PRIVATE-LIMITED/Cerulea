//! The Standing Engine.
//!
//! One composite measure drives four engines. The Trust Score. Nothing else
//! governs validator behaviour.
//!
//! Five files carry the work. `score` composes the Trust Score from its
//! components. `accumulate` raises standing through service over a ramp period.
//! `sampling` draws the verification sample and scores the reports. `decay`
//! lowers standing during inactivity. `reset` returns standing to zero on
//! proven misconduct.

pub mod accumulate;
pub mod decay;
pub mod reset;
pub mod sampling;
pub mod score;

pub use accumulate::{
    accumulated, enter, epochs_remaining, record_epoch, ServiceEvidence, ServiceRecord,
    DEFAULT_RAMP_EPOCHS,
};

pub use decay::{decay_once, decayed, epochs_until_floor, idle_epochs, DecaySettings};

pub use reset::{reset, Misconduct, ResetRecord};

pub use sampling::{
    draw_sample, is_accurate, settle as settle_sample, SampleOutcome, SamplingRecord,
    DEFAULT_SAMPLE_SIZE,
};

pub use score::{
    compose, shares, ChainKind, Components, TrustScore, PRIVATE_ATTESTATION_PERCENT,
    PRIVATE_CUSTODY_PERCENT, PRIVATE_PERFORMANCE_PERCENT, PRIVATE_ROLE_PERCENT,
    PUBLIC_CUSTODY_PERCENT, PUBLIC_PERFORMANCE_PERCENT, PUBLIC_SAMPLING_PERCENT,
    PUBLIC_STANDING_PERCENT,
};
