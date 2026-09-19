//! The Amendment Engine.
//!
//! The Amendment Engine governs changes to the seven engines beside it. Four
//! files carry the work. `policies` holds the eight policy records. `solver`
//! turns five plain-language declarations into valid deployment profiles.
//! `limits` holds the checks that never bend. `amend` runs a change under
//! authority, quorum and delay.

pub mod amend;
pub mod limits;
pub mod policies;
pub mod solver;

pub use amend::{
    approvals_for_quorum, approve, close_voting, enact, holds_authority, open, Amendment,
    AmendmentError, AmendmentState, DEFAULT_VOTING_WINDOW_BLOCKS,
};

pub use limits::{
    inspect as inspect_point, is_valid, max_faulty, participants_exceed_three_times_faulty,
    CandidatePoint, LimitBreach, PartitionChoice, ASSUMED_MESSAGE_DELAY_BOUND_MS, FAULT_MULTIPLE,
    MAX_VALIDATOR_SET, MIN_VALIDATOR_SET,
};

pub use policies::{
    AdmissionPolicy, AmendmentPolicy, AssurancePolicy, AuthoringPolicy, Authority,
    EligibilityPolicy, PenaltyPolicy, PolicyKind, PolicySet, RewardPolicy, RotationPolicy,
};

pub use solver::{
    inspect_profile, solve, ConfirmationPreference, Declarations, ErrorCost, OperatorCount,
    Profile, RetentionNeed, SplitBehaviour, MAX_PROFILES, MIN_PROFILES,
};
