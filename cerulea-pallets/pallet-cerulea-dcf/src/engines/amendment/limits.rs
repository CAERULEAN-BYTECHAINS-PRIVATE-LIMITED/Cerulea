//! The limits that stay fixed.
//!
//! Some parts of the framework are hard limits rather than trades. No control
//! ever lets a customer cross one. Every candidate deployment profile passes
//! these checks before the Amendment Engine presents it, so an invalid point
//! never surfaces.
//!
//! # The fault bound
//!
//! Byzantine fault tolerance means the network stays correct while some
//! participants fail or act against the network. Safety holds while the total
//! participant count exceeds three times the faulty count.
//! [`participants_exceed_three_times_faulty`] states the limit in code.
//!
//! # The timing assumption
//!
//! No deterministic protocol guarantees termination in a fully asynchronous
//! network holding even one faulty participant. Cerulea therefore assumes a
//! bound on message delay that eventually holds, and states the assumption
//! here. [`ASSUMED_MESSAGE_DELAY_BOUND_MS`] carries the assumed bound. Safety
//! holds at every moment. Progress resumes once message delay settles under
//! that bound.
//!
//! # The partition choice
//!
//! A network split forces a choice between continuing to serve and halting to
//! stay consistent. The choice is a proven limit that no design escapes. A
//! profile states which side it takes rather than claiming both.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// The multiple that sets the fault bound.
///
/// Safety holds while the total participant count exceeds this multiple of the
/// faulty count.
pub const FAULT_MULTIPLE: u32 = 3;

/// The assumed bound on message delay, in milliseconds.
///
/// Progress resumes once message delay settles under this bound. Safety holds
/// at every moment regardless of delay.
pub const ASSUMED_MESSAGE_DELAY_BOUND_MS: u64 = 2_000;

/// The smallest validator set the framework runs.
///
/// A set of four tolerates one faulty member. A smaller set tolerates none.
pub const MIN_VALIDATOR_SET: u32 = 4;

/// The largest validator set the framework runs at sub-second finality.
pub const MAX_VALIDATOR_SET: u32 = 1_000;

/// The lowest share of aggregated weight any class carries, in percent.
pub const MIN_THRESHOLD_PERCENT: u8 = 51;

/// The highest share of aggregated weight any class carries, in percent.
pub const MAX_THRESHOLD_PERCENT: u8 = 99;

/// The side a profile takes when the network splits.
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
pub enum PartitionChoice {
    /// The network halts on a split and stays consistent.
    #[default]
    HaltToStayConsistent,
    /// The network keeps serving on a split and reconciles afterwards.
    KeepServing,
}

impl PartitionChoice {
    /// The sentence a person reads, stating the consequence.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            PartitionChoice::HaltToStayConsistent => {
                "A network split halts the smaller side. Every record stays consistent. Service pauses on that side until the split heals."
            }
            PartitionChoice::KeepServing => {
                "A network split leaves both sides serving. Service continues. Records written on the smaller side reconcile once the split heals."
            }
        }
    }
}

/// A reason a candidate profile crosses a fixed limit.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum LimitBreach {
    /// The set tolerates a faulty count at or above a third of the set.
    FaultBoundCrossed,
    /// The set holds fewer members than the framework runs.
    SetTooSmall,
    /// The set holds more members than the framework runs.
    SetTooLarge,
    /// A share sits at or below half the weight.
    ThresholdTooLow,
    /// A share sits above the highest share a class carries.
    ThresholdTooHigh,
    /// The profile claims both consistency and service during a split.
    PartitionChoiceAbsent,
}

impl LimitBreach {
    /// The sentence a person reads, naming what holds instead.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            LimitBreach::FaultBoundCrossed => {
                "The set would tolerate too many failures at once. Raise the participant count or lower the tolerated failures."
            }
            LimitBreach::SetTooSmall => {
                "The set holds too few validators to tolerate one failure. Add validators until four stand."
            }
            LimitBreach::SetTooLarge => {
                "The set holds more validators than the framework runs at this latency. Lower the validator count."
            }
            LimitBreach::ThresholdTooLow => {
                "A share sits at or below half the weight, which lets two groups each commit. Raise that share to 51 percent or above."
            }
            LimitBreach::ThresholdTooHigh => {
                "A share sits above 99 percent, which halts every commitment when one validator falls silent. Lower that share."
            }
            LimitBreach::PartitionChoiceAbsent => {
                "The profile claims both consistency and service during a network split. Choose the side the deployment takes."
            }
        }
    }
}

/// Report whether a participant count respects the fault bound.
///
/// Safety holds while the total participant count exceeds three times the
/// faulty count.
pub const fn participants_exceed_three_times_faulty(total: u32, faulty: u32) -> bool {
    (total as u64) > (FAULT_MULTIPLE as u64) * (faulty as u64)
}

/// The highest faulty count a participant count tolerates.
pub const fn max_faulty(total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        (total - 1) / FAULT_MULTIPLE
    }
}

/// One candidate point in the parameter space.
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
pub struct CandidatePoint {
    /// The validators the set holds.
    pub validator_count: u32,
    /// The failures the profile tolerates at once.
    pub tolerated_failures: u32,
    /// The share a routine transaction needs, in percent.
    pub routine_percent: u8,
    /// The share a constitutional transaction needs, in percent.
    pub constitutional_percent: u8,
    /// The side the profile takes when the network splits.
    pub partition_choice: PartitionChoice,
    /// The profile states the partition choice.
    pub partition_choice_stated: bool,
}

/// Inspect one candidate point against every fixed limit.
///
/// A point that fails never reaches a customer. The Amendment Engine runs this
/// check before it presents any profile.
pub fn inspect(point: &CandidatePoint) -> Result<(), LimitBreach> {
    if point.validator_count < MIN_VALIDATOR_SET {
        return Err(LimitBreach::SetTooSmall);
    }
    if point.validator_count > MAX_VALIDATOR_SET {
        return Err(LimitBreach::SetTooLarge);
    }
    if !participants_exceed_three_times_faulty(point.validator_count, point.tolerated_failures) {
        return Err(LimitBreach::FaultBoundCrossed);
    }
    if point.routine_percent < MIN_THRESHOLD_PERCENT
        || point.constitutional_percent < MIN_THRESHOLD_PERCENT
    {
        return Err(LimitBreach::ThresholdTooLow);
    }
    if point.routine_percent > MAX_THRESHOLD_PERCENT
        || point.constitutional_percent > MAX_THRESHOLD_PERCENT
    {
        return Err(LimitBreach::ThresholdTooHigh);
    }
    if !point.partition_choice_stated {
        return Err(LimitBreach::PartitionChoiceAbsent);
    }
    Ok(())
}

/// Report whether a candidate point passes every fixed limit.
pub fn is_valid(point: &CandidatePoint) -> bool {
    inspect(point).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> CandidatePoint {
        CandidatePoint {
            validator_count: 10,
            tolerated_failures: 3,
            routine_percent: 51,
            constitutional_percent: 90,
            partition_choice: PartitionChoice::HaltToStayConsistent,
            partition_choice_stated: true,
        }
    }

    #[test]
    fn the_fault_bound_reads_three_times_the_faulty_count() {
        assert!(participants_exceed_three_times_faulty(4, 1));
        assert!(!participants_exceed_three_times_faulty(3, 1));
        assert!(participants_exceed_three_times_faulty(10, 3));
        assert!(!participants_exceed_three_times_faulty(9, 3));
        assert!(participants_exceed_three_times_faulty(100, 33));
        assert!(!participants_exceed_three_times_faulty(99, 33));
    }

    #[test]
    fn the_engine_reports_the_highest_tolerated_faulty_count() {
        assert_eq!(max_faulty(0), 0);
        assert_eq!(max_faulty(1), 0);
        assert_eq!(max_faulty(4), 1);
        assert_eq!(max_faulty(10), 3);
        assert_eq!(max_faulty(100), 33);
        for total in 1..500u32 {
            assert!(participants_exceed_three_times_faulty(
                total,
                max_faulty(total)
            ));
            assert!(!participants_exceed_three_times_faulty(
                total,
                max_faulty(total) + 1
            ));
        }
    }

    #[test]
    fn a_valid_point_passes_every_limit() {
        assert_eq!(inspect(&valid()), Ok(()));
        assert!(is_valid(&valid()));
    }

    /// Design assertion 12, first half.
    ///
    /// A point that would cross the fault bound never passes inspection.
    #[test]
    fn a_point_crossing_the_fault_bound_is_refused() {
        let mut point = valid();
        point.tolerated_failures = 4;
        assert_eq!(inspect(&point), Err(LimitBreach::FaultBoundCrossed));
        assert!(!is_valid(&point));
    }

    #[test]
    fn a_set_below_four_is_refused() {
        let mut point = valid();
        point.validator_count = 3;
        assert_eq!(inspect(&point), Err(LimitBreach::SetTooSmall));
    }

    #[test]
    fn a_set_above_the_framework_bound_is_refused() {
        let mut point = valid();
        point.validator_count = MAX_VALIDATOR_SET + 1;
        point.tolerated_failures = 1;
        assert_eq!(inspect(&point), Err(LimitBreach::SetTooLarge));
    }

    #[test]
    fn a_share_at_or_below_half_is_refused() {
        let mut point = valid();
        point.routine_percent = 50;
        assert_eq!(inspect(&point), Err(LimitBreach::ThresholdTooLow));
    }

    #[test]
    fn a_share_of_the_whole_weight_is_refused() {
        let mut point = valid();
        point.constitutional_percent = 100;
        assert_eq!(inspect(&point), Err(LimitBreach::ThresholdTooHigh));
    }

    #[test]
    fn a_profile_that_states_no_partition_choice_is_refused() {
        let mut point = valid();
        point.partition_choice_stated = false;
        assert_eq!(inspect(&point), Err(LimitBreach::PartitionChoiceAbsent));
    }

    #[test]
    fn the_timing_assumption_is_stated_in_the_code() {
        assert_eq!(ASSUMED_MESSAGE_DELAY_BOUND_MS, 2_000);
    }

    #[test]
    fn every_partition_choice_states_its_consequence() {
        for choice in [
            PartitionChoice::HaltToStayConsistent,
            PartitionChoice::KeepServing,
        ] {
            assert!(choice.plain_language().len() > 50);
        }
    }

    #[test]
    fn every_limit_breach_names_what_holds_instead() {
        for breach in [
            LimitBreach::FaultBoundCrossed,
            LimitBreach::SetTooSmall,
            LimitBreach::SetTooLarge,
            LimitBreach::ThresholdTooLow,
            LimitBreach::ThresholdTooHigh,
            LimitBreach::PartitionChoiceAbsent,
        ] {
            assert!(breach.plain_language().len() > 40);
        }
    }
}
