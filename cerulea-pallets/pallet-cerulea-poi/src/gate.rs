//! The gate on the optional inference contribution.
//!
//! Deployments running Cerulea Intelligence on their own validators may enable
//! an additional scoring contribution based on served inference work. Custody
//! proof remains the default for every other deployment.
//!
//! The contribution stays absent unless three conditions hold at once.
//!
//! 1. **The build carries the module.** The `inference` cargo feature turns the
//!    module on. The feature stays off by default, so a deployment that never
//!    asks for the module never carries it.
//! 2. **Determinism is proven.** Two validators running the same task produce
//!    identical bytes. That needs pinned weights, a fixed seed, fixed-point
//!    arithmetic and a pinned runtime. The module stays disabled where
//!    determinism is unproven.
//! 3. **The work is real.** Tasks come from Cerulea Intelligence workload. A
//!    synthetic task carries no contribution.
//!
//! A result is accepted provisionally. Any validator challenges inside a
//! window. A challenge triggers redundant execution by a randomly drawn
//! committee, and the losing side is penalised.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// The validators drawn for redundant execution after a challenge.
pub const REDUNDANT_COMMITTEE_SIZE: u32 = 3;

/// The domain separator mixed into every redundant execution draw.
pub const REDUNDANT_DRAW_DOMAIN: &[u8] = b"cerulea.inference.redundant.v1";

/// Report whether this build carries the inference contribution.
///
/// The `inference` cargo feature turns the module on. The feature stays off by
/// default.
pub const fn module_in_build() -> bool {
    cfg!(feature = "inference")
}

/// The four conditions determinism asks for.
///
/// Two validators running the same task produce identical bytes only where
/// every condition holds.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct DeterminismEvidence {
    /// The model weights carry a fixed content hash.
    pub pinned_weights: bool,
    /// Every run uses one fixed seed.
    pub fixed_seed: bool,
    /// Arithmetic runs in fixed point rather than floating point.
    pub fixed_point_arithmetic: bool,
    /// The runtime that serves the model carries a fixed version.
    pub pinned_runtime: bool,
}

impl DeterminismEvidence {
    /// Report whether every condition holds.
    pub const fn proven(&self) -> bool {
        self.pinned_weights && self.fixed_seed && self.fixed_point_arithmetic && self.pinned_runtime
    }

    /// The sentence a person reads where a condition is missing.
    pub const fn plain_language(&self) -> &'static str {
        if !self.pinned_weights {
            "The model weights carry no fixed content hash. Pin the weights, then declare determinism again."
        } else if !self.fixed_seed {
            "The runs use no fixed seed. Set one seed for every run, then declare determinism again."
        } else if !self.fixed_point_arithmetic {
            "The arithmetic runs in floating point. Move the model to fixed point, then declare determinism again."
        } else if !self.pinned_runtime {
            "The serving runtime carries no fixed version. Pin the runtime, then declare determinism again."
        } else {
            "Determinism holds. Two validators running the same task produce identical bytes."
        }
    }
}

/// Where one inference task came from.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum TaskSource {
    /// The task came from Cerulea Intelligence workload.
    #[default]
    IntelligenceWorkload,
    /// The task was generated to raise a score.
    Synthetic,
}

impl TaskSource {
    /// Report whether this source carries a contribution.
    pub const fn carries_contribution(&self) -> bool {
        matches!(self, TaskSource::IntelligenceWorkload)
    }
}

/// A reason the contribution stays absent.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum GateFailure {
    /// The build carries no inference module.
    ModuleAbsentFromBuild,
    /// Determinism is unproven for this deployment.
    DeterminismUnproven,
    /// The task was generated rather than served.
    TaskSynthetic,
}

impl GateFailure {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            GateFailure::ModuleAbsentFromBuild => {
                "This build carries no inference module. Build the runtime with the inference feature to enable the contribution."
            }
            GateFailure::DeterminismUnproven => {
                "Determinism is unproven for this deployment. Pin the weights, the seed, the arithmetic and the runtime, then declare determinism."
            }
            GateFailure::TaskSynthetic => {
                "The task was generated rather than served. Contribution counts served Cerulea Intelligence workload alone."
            }
        }
    }
}

/// Inspect the gate for one task.
///
/// The call returns the first condition the deployment fails, so the sentence
/// a person reads names one fix.
pub fn inspect(determinism_declared: bool, source: TaskSource) -> Result<(), GateFailure> {
    if !module_in_build() {
        return Err(GateFailure::ModuleAbsentFromBuild);
    }
    if !determinism_declared {
        return Err(GateFailure::DeterminismUnproven);
    }
    if !source.carries_contribution() {
        return Err(GateFailure::TaskSynthetic);
    }
    Ok(())
}

/// Report whether the contribution counts for one task.
pub fn contribution_counts(determinism_declared: bool, source: TaskSource) -> bool {
    inspect(determinism_declared, source).is_ok()
}

/// One result accepted provisionally.
///
/// The result stands unless a validator challenges inside the window.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ProvisionalResult {
    /// The epoch at which the chain accepted the result.
    pub accepted_at: u32,
    /// The last epoch at which a challenge counts.
    pub challenge_closes_at: u32,
    /// The result the validator reported.
    pub value: u32,
    /// Where the task came from.
    pub source: TaskSource,
}

impl ProvisionalResult {
    /// Open a provisional acceptance.
    pub const fn accept(value: u32, source: TaskSource, now: u32, window: u32) -> Self {
        ProvisionalResult {
            accepted_at: now,
            challenge_closes_at: now.saturating_add(window),
            value,
            source,
        }
    }

    /// Report whether the window still admits a challenge.
    pub const fn open_to_challenge(&self, now: u32) -> bool {
        now <= self.challenge_closes_at
    }

    /// Report whether the result is settled.
    pub const fn settled(&self, now: u32) -> bool {
        now > self.challenge_closes_at
    }
}

/// The side that loses a challenge.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum ChallengeOutcome {
    /// The redundant execution matched the reported result. The challenger
    /// loses.
    ReporterHeld,
    /// The redundant execution differed. The reporter loses.
    ChallengerHeld,
}

impl ChallengeOutcome {
    /// A plain sentence stating who carries the penalty.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ChallengeOutcome::ReporterHeld => {
                "The redundant execution matched the reported result. The challenger carries the penalty."
            }
            ChallengeOutcome::ChallengerHeld => {
                "The redundant execution differed from the reported result. The reporter carries the penalty."
            }
        }
    }
}

/// Settle one challenge from the redundant execution results.
///
/// The committee re-executes the task. A majority matching the reported value
/// holds the reporter. A majority differing holds the challenger.
pub fn settle_challenge(reported: u32, redundant: &[u32]) -> ChallengeOutcome {
    let matching = redundant.iter().filter(|held| **held == reported).count();
    if matching * 2 > redundant.len() {
        ChallengeOutcome::ReporterHeld
    } else {
        ChallengeOutcome::ChallengerHeld
    }
}

/// Draw the committee that re-executes a challenged task.
///
/// The draw mixes a domain separator, the block hash and the task identifier,
/// so every node draws the same committee and no party picks its own judges.
pub fn draw_redundant_committee(
    block_hash: [u8; 32],
    task_id: u64,
    candidate_count: u32,
    committee_size: u32,
) -> Vec<u32> {
    let mut chosen: Vec<u32> = Vec::new();
    if candidate_count == 0 || committee_size == 0 {
        return chosen;
    }
    let wanted = committee_size.min(candidate_count) as usize;
    let mut attempt = 0u32;
    while chosen.len() < wanted && attempt < wanted as u32 * 16 + 64 {
        let mut preimage = Vec::with_capacity(REDUNDANT_DRAW_DOMAIN.len() + 32 + 12);
        preimage.extend_from_slice(REDUNDANT_DRAW_DOMAIN);
        preimage.extend_from_slice(&block_hash);
        preimage.extend_from_slice(&task_id.to_be_bytes());
        preimage.extend_from_slice(&attempt.to_be_bytes());
        let digest = sp_io::hashing::blake2_256(&preimage);
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&digest[..4]);
        let position = u32::from_be_bytes(raw) % candidate_count;
        if !chosen.contains(&position) {
            chosen.push(position);
        }
        attempt = attempt.saturating_add(1);
    }
    chosen.sort_unstable();
    chosen
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete() -> DeterminismEvidence {
        DeterminismEvidence {
            pinned_weights: true,
            fixed_seed: true,
            fixed_point_arithmetic: true,
            pinned_runtime: true,
        }
    }

    #[test]
    fn the_module_stays_out_of_the_build_by_default() {
        assert_eq!(module_in_build(), cfg!(feature = "inference"));
        #[cfg(not(feature = "inference"))]
        assert!(!module_in_build());
    }

    #[test]
    fn a_build_without_the_module_carries_no_contribution() {
        #[cfg(not(feature = "inference"))]
        {
            assert_eq!(
                inspect(true, TaskSource::IntelligenceWorkload),
                Err(GateFailure::ModuleAbsentFromBuild)
            );
            assert!(!contribution_counts(true, TaskSource::IntelligenceWorkload));
        }
    }

    #[test]
    fn determinism_asks_for_every_condition() {
        assert!(complete().proven());

        let mut held = complete();
        held.pinned_weights = false;
        assert!(!held.proven());

        let mut held = complete();
        held.fixed_seed = false;
        assert!(!held.proven());

        let mut held = complete();
        held.fixed_point_arithmetic = false;
        assert!(!held.proven());

        let mut held = complete();
        held.pinned_runtime = false;
        assert!(!held.proven());
    }

    #[test]
    fn every_missing_condition_names_a_fix() {
        let cases = [
            DeterminismEvidence {
                pinned_weights: false,
                ..complete()
            },
            DeterminismEvidence {
                fixed_seed: false,
                ..complete()
            },
            DeterminismEvidence {
                fixed_point_arithmetic: false,
                ..complete()
            },
            DeterminismEvidence {
                pinned_runtime: false,
                ..complete()
            },
        ];
        for case in cases.iter() {
            assert!(case.plain_language().len() > 40);
        }
        assert!(complete().plain_language().len() > 40);
    }

    #[test]
    fn a_synthetic_task_carries_no_contribution() {
        assert!(!TaskSource::Synthetic.carries_contribution());
        assert!(TaskSource::IntelligenceWorkload.carries_contribution());
    }

    #[test]
    fn the_gate_names_the_first_condition_that_fails() {
        #[cfg(feature = "inference")]
        {
            assert_eq!(
                inspect(false, TaskSource::IntelligenceWorkload),
                Err(GateFailure::DeterminismUnproven)
            );
            assert_eq!(
                inspect(true, TaskSource::Synthetic),
                Err(GateFailure::TaskSynthetic)
            );
            assert_eq!(inspect(true, TaskSource::IntelligenceWorkload), Ok(()));
        }
        #[cfg(not(feature = "inference"))]
        assert_eq!(
            inspect(false, TaskSource::Synthetic),
            Err(GateFailure::ModuleAbsentFromBuild)
        );
    }

    #[test]
    fn every_gate_failure_names_a_fix() {
        for failure in [
            GateFailure::ModuleAbsentFromBuild,
            GateFailure::DeterminismUnproven,
            GateFailure::TaskSynthetic,
        ] {
            assert!(failure.plain_language().len() > 40);
        }
    }

    #[test]
    fn a_result_stays_open_to_challenge_inside_the_window() {
        let held = ProvisionalResult::accept(42, TaskSource::IntelligenceWorkload, 10, 5);
        assert_eq!(held.challenge_closes_at, 15);
        assert!(held.open_to_challenge(10));
        assert!(held.open_to_challenge(15));
        assert!(!held.open_to_challenge(16));
        assert!(!held.settled(15));
        assert!(held.settled(16));
    }

    #[test]
    fn a_matching_redundant_execution_holds_the_reporter() {
        assert_eq!(
            settle_challenge(42, &[42, 42, 42]),
            ChallengeOutcome::ReporterHeld
        );
        assert_eq!(
            settle_challenge(42, &[42, 42, 7]),
            ChallengeOutcome::ReporterHeld
        );
    }

    #[test]
    fn a_differing_redundant_execution_holds_the_challenger() {
        assert_eq!(
            settle_challenge(42, &[7, 7, 7]),
            ChallengeOutcome::ChallengerHeld
        );
        assert_eq!(
            settle_challenge(42, &[42, 7, 7]),
            ChallengeOutcome::ChallengerHeld
        );
    }

    #[test]
    fn every_challenge_outcome_names_who_carries_the_penalty() {
        for outcome in [
            ChallengeOutcome::ReporterHeld,
            ChallengeOutcome::ChallengerHeld,
        ] {
            assert!(outcome.plain_language().len() > 40);
        }
    }

    #[test]
    fn every_node_draws_the_same_redundant_committee() {
        let hash = [9u8; 32];
        assert_eq!(
            draw_redundant_committee(hash, 7, 20, REDUNDANT_COMMITTEE_SIZE),
            draw_redundant_committee(hash, 7, 20, REDUNDANT_COMMITTEE_SIZE)
        );
    }

    #[test]
    fn the_redundant_committee_holds_distinct_members() {
        let drawn = draw_redundant_committee([1u8; 32], 3, 10, 3);
        assert_eq!(drawn.len(), 3);
        let mut held = drawn.clone();
        held.dedup();
        assert_eq!(held.len(), drawn.len());
        for position in drawn.iter() {
            assert!(*position < 10);
        }
    }

    #[test]
    fn a_different_task_draws_a_different_committee() {
        let hash = [4u8; 32];
        assert_ne!(
            draw_redundant_committee(hash, 1, 50, 3),
            draw_redundant_committee(hash, 2, 50, 3)
        );
    }

    #[test]
    fn an_empty_candidate_list_draws_nothing() {
        assert!(draw_redundant_committee([0u8; 32], 1, 0, 3).is_empty());
        assert!(draw_redundant_committee([0u8; 32], 1, 10, 0).is_empty());
    }
}
