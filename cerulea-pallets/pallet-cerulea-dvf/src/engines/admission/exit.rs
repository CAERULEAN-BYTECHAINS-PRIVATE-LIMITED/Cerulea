//! Exit conditions and epoch-boundary demotion.
//!
//! A validator holds a seat while meeting every condition. Losing a condition
//! triggers demotion at the next epoch boundary. An epoch is a fixed span of
//! blocks after which the active set may change.
//!
//! Five conditions end a seat.
//!
//! 1. **Standing below the floor.**
//! 2. **Uptime below the floor.**
//! 3. **Participation below the floor.**
//! 4. **A lapsed licence.**
//! 5. **Proven misconduct.**
//!
//! Proven misconduct that breaks safety removes the seat at once rather than at
//! the boundary. Every other condition waits for the boundary, so the active
//! set stays stable inside an epoch.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;
use sp_std::vec::Vec;

/// The standing a seated validator holds, at least.
pub const DEFAULT_STANDING_FLOOR_PERCENT: u32 = 33;

/// The uptime a seated validator holds, at least.
pub const DEFAULT_UPTIME_FLOOR_PERCENT: u32 = 80;

/// The participation a seated validator holds, at least.
pub const DEFAULT_PARTICIPATION_FLOOR_PERCENT: u32 = 50;

/// The floors a seated validator meets.
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
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct ExitFloors {
    /// The standing the validator holds, at least.
    pub standing: Perbill,
    /// The uptime the validator holds, at least.
    pub uptime: Perbill,
    /// The participation the validator holds, at least.
    pub participation: Perbill,
}

impl ExitFloors {
    /// The floors a deployment starts from.
    pub fn starting_profile() -> Self {
        ExitFloors {
            standing: Perbill::from_percent(DEFAULT_STANDING_FLOOR_PERCENT),
            uptime: Perbill::from_percent(DEFAULT_UPTIME_FLOOR_PERCENT),
            participation: Perbill::from_percent(DEFAULT_PARTICIPATION_FLOOR_PERCENT),
        }
    }
}

impl Default for ExitFloors {
    fn default() -> Self {
        ExitFloors::starting_profile()
    }
}

/// The measured state of one seated validator.
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
pub struct SeatState {
    /// The standing the validator carries.
    pub standing: Perbill,
    /// The uptime the validator carries.
    pub uptime: Perbill,
    /// The participation the validator carries.
    pub participation: Perbill,
    /// The licence still holds at this block.
    pub licence_holds: bool,
    /// The chain proved misconduct against the validator.
    pub misconduct_proven: bool,
    /// The proven misconduct broke safety.
    pub misconduct_breaks_safety: bool,
}

/// A reason one seat ends.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum ExitReason {
    /// Standing fell below the floor.
    StandingBelowFloor,
    /// Uptime fell below the floor.
    UptimeBelowFloor,
    /// Participation fell below the floor.
    ParticipationBelowFloor,
    /// The licence lapsed.
    LicenceLapsed,
    /// The chain proved misconduct.
    ProvenMisconduct,
}

impl ExitReason {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            ExitReason::StandingBelowFloor => {
                "Standing fell below the floor. Serve every epoch to rebuild standing, then apply again."
            }
            ExitReason::UptimeBelowFloor => {
                "Uptime fell below the floor. Raise machine availability, then apply again."
            }
            ExitReason::ParticipationBelowFloor => {
                "Participation fell below the floor. Take part in every slot the committee names, then apply again."
            }
            ExitReason::LicenceLapsed => {
                "The licence lapsed. Renew the licence and submit the fresh attestation."
            }
            ExitReason::ProvenMisconduct => {
                "The chain proved misconduct. Standing returns to zero and the climb starts again."
            }
        }
    }

    /// Report whether this reason removes the seat at once.
    ///
    /// Proven misconduct that breaks safety removes the seat immediately. Every
    /// other reason waits for the epoch boundary.
    pub const fn removes_the_seat_at_once(&self) -> bool {
        matches!(self, ExitReason::ProvenMisconduct)
    }
}

/// The reason one seat ends, where a reason holds.
///
/// The call returns the first reason the state meets, so the sentence a person
/// reads names one fix.
pub fn exit_reason(state: &SeatState, floors: &ExitFloors) -> Option<ExitReason> {
    if state.misconduct_proven {
        return Some(ExitReason::ProvenMisconduct);
    }
    if !state.licence_holds {
        return Some(ExitReason::LicenceLapsed);
    }
    if state.standing < floors.standing {
        return Some(ExitReason::StandingBelowFloor);
    }
    if state.uptime < floors.uptime {
        return Some(ExitReason::UptimeBelowFloor);
    }
    if state.participation < floors.participation {
        return Some(ExitReason::ParticipationBelowFloor);
    }
    None
}

/// Report whether one validator keeps its seat.
pub fn keeps_seat(state: &SeatState, floors: &ExitFloors) -> bool {
    exit_reason(state, floors).is_none()
}

/// Report whether a state removes the seat before the epoch boundary.
pub fn removes_the_seat_at_once(state: &SeatState) -> bool {
    state.misconduct_proven && state.misconduct_breaks_safety
}

/// The validators to demote at one epoch boundary.
///
/// The call reads every seated validator and returns each one that lost a
/// condition, with the reason. The active set changes at the boundary alone.
pub fn demotions_at_boundary<ValidatorId: Clone>(
    seated: &[(ValidatorId, SeatState)],
    floors: &ExitFloors,
) -> Vec<(ValidatorId, ExitReason)> {
    seated
        .iter()
        .filter_map(|(validator, state)| {
            exit_reason(state, floors).map(|reason| (validator.clone(), reason))
        })
        .collect()
}

/// The validators to remove before the epoch boundary.
///
/// Proven misconduct that breaks safety leaves no room to wait.
pub fn immediate_removals<ValidatorId: Clone>(
    seated: &[(ValidatorId, SeatState)],
) -> Vec<ValidatorId> {
    seated
        .iter()
        .filter(|(_, state)| removes_the_seat_at_once(state))
        .map(|(validator, _)| validator.clone())
        .collect()
}

/// Report whether one block sits on an epoch boundary.
pub fn is_epoch_boundary(block: u32, epoch_length: u32) -> bool {
    epoch_length > 0 && block > 0 && block % epoch_length == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy() -> SeatState {
        SeatState {
            standing: Perbill::from_percent(80),
            uptime: Perbill::from_percent(95),
            participation: Perbill::from_percent(90),
            licence_holds: true,
            misconduct_proven: false,
            misconduct_breaks_safety: false,
        }
    }

    #[test]
    fn a_healthy_validator_keeps_its_seat() {
        assert_eq!(exit_reason(&healthy(), &ExitFloors::default()), None);
        assert!(keeps_seat(&healthy(), &ExitFloors::default()));
    }

    #[test]
    fn standing_below_the_floor_ends_the_seat() {
        let mut state = healthy();
        state.standing = Perbill::from_percent(20);
        assert_eq!(
            exit_reason(&state, &ExitFloors::default()),
            Some(ExitReason::StandingBelowFloor)
        );
    }

    #[test]
    fn uptime_below_the_floor_ends_the_seat() {
        let mut state = healthy();
        state.uptime = Perbill::from_percent(50);
        assert_eq!(
            exit_reason(&state, &ExitFloors::default()),
            Some(ExitReason::UptimeBelowFloor)
        );
    }

    #[test]
    fn participation_below_the_floor_ends_the_seat() {
        let mut state = healthy();
        state.participation = Perbill::from_percent(20);
        assert_eq!(
            exit_reason(&state, &ExitFloors::default()),
            Some(ExitReason::ParticipationBelowFloor)
        );
    }

    #[test]
    fn a_lapsed_licence_ends_the_seat() {
        let mut state = healthy();
        state.licence_holds = false;
        assert_eq!(
            exit_reason(&state, &ExitFloors::default()),
            Some(ExitReason::LicenceLapsed)
        );
    }

    #[test]
    fn proven_misconduct_ends_the_seat() {
        let mut state = healthy();
        state.misconduct_proven = true;
        assert_eq!(
            exit_reason(&state, &ExitFloors::default()),
            Some(ExitReason::ProvenMisconduct)
        );
    }

    #[test]
    fn a_validator_exactly_at_the_floors_keeps_its_seat() {
        let floors = ExitFloors::default();
        let mut state = healthy();
        state.standing = floors.standing;
        state.uptime = floors.uptime;
        state.participation = floors.participation;
        assert!(keeps_seat(&state, &floors));
    }

    #[test]
    fn misconduct_breaking_safety_removes_the_seat_at_once() {
        let mut state = healthy();
        state.misconduct_proven = true;
        state.misconduct_breaks_safety = true;
        assert!(removes_the_seat_at_once(&state));

        state.misconduct_breaks_safety = false;
        assert!(!removes_the_seat_at_once(&state));
    }

    #[test]
    fn the_boundary_names_every_validator_that_lost_a_condition() {
        let mut failing = healthy();
        failing.standing = Perbill::from_percent(10);
        let mut lapsed = healthy();
        lapsed.licence_holds = false;

        let seated = sp_std::vec![
            (1u64, healthy()),
            (2u64, failing),
            (3u64, healthy()),
            (4u64, lapsed),
        ];
        let demotions = demotions_at_boundary(&seated, &ExitFloors::default());
        assert_eq!(
            demotions,
            sp_std::vec![
                (2u64, ExitReason::StandingBelowFloor),
                (4u64, ExitReason::LicenceLapsed),
            ]
        );
    }

    #[test]
    fn only_safety_breaking_misconduct_is_removed_before_the_boundary() {
        let mut breaks_safety = healthy();
        breaks_safety.misconduct_proven = true;
        breaks_safety.misconduct_breaks_safety = true;
        let mut other = healthy();
        other.misconduct_proven = true;

        let seated = sp_std::vec![(1u64, breaks_safety), (2u64, other), (3u64, healthy())];
        assert_eq!(immediate_removals(&seated), sp_std::vec![1u64]);
    }

    #[test]
    fn the_epoch_boundary_falls_on_a_multiple_of_the_epoch_length() {
        assert!(!is_epoch_boundary(0, 100));
        assert!(!is_epoch_boundary(50, 100));
        assert!(is_epoch_boundary(100, 100));
        assert!(is_epoch_boundary(200, 100));
        assert!(!is_epoch_boundary(100, 0));
    }

    #[test]
    fn the_starting_floors_read_thirty_three_eighty_and_fifty_percent() {
        let floors = ExitFloors::starting_profile();
        assert_eq!(floors.standing, Perbill::from_percent(33));
        assert_eq!(floors.uptime, Perbill::from_percent(80));
        assert_eq!(floors.participation, Perbill::from_percent(50));
    }

    #[test]
    fn every_exit_reason_names_a_fix() {
        for reason in [
            ExitReason::StandingBelowFloor,
            ExitReason::UptimeBelowFloor,
            ExitReason::ParticipationBelowFloor,
            ExitReason::LicenceLapsed,
            ExitReason::ProvenMisconduct,
        ] {
            assert!(reason.plain_language().len() > 30);
        }
        assert!(ExitReason::ProvenMisconduct.removes_the_seat_at_once());
        assert!(!ExitReason::LicenceLapsed.removes_the_seat_at_once());
    }
}
