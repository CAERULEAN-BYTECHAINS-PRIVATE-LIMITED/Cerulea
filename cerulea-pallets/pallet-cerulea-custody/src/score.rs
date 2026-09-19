//! Custody scoring and the deadline rule.
//!
//! A correct proof inside the deadline raises standing. A missed deadline
//! lowers standing. An incorrect proof lowers standing sharply and records a
//! fault.
//!
//! The custody share is the part of the Trust Score the Custody Engine owns.
//! The share reads in parts per million, where one million is the whole share.
//! A validator that answers every challenge reaches the whole share. A missed
//! challenge costs twice what an answer gains. An incorrect proof costs ten
//! times what an answer gains and records a fault.

use crate::primitives::{ChallengeState, CustodyRecord};
use codec::{Decode, Encode};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// The share one correct answer inside the deadline gains, in parts per
/// million.
pub const ANSWER_GAIN: u32 = 50_000;

/// The share one missed deadline costs, in parts per million.
pub const MISS_LOSS: u32 = 100_000;

/// The share one incorrect proof costs, in parts per million.
pub const FAULT_LOSS: u32 = 500_000;

/// The share a validator carries on its first challenge, in parts per million.
///
/// A new validator starts at half the share. Proven custody carries the share
/// upward. A missed challenge carries the share downward at once.
pub const STARTING_SHARE: u32 = 500_000;

/// The movement one settled challenge produces.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub struct Movement {
    /// The state the challenge reached.
    pub state: ChallengeState,
    /// The custody share before the movement, in parts per million.
    pub previous_share: u32,
    /// The custody share after the movement, in parts per million.
    pub current_share: u32,
    /// The fault count after the movement.
    pub faults: u32,
}

impl Movement {
    /// Report whether the movement raised the share.
    pub const fn raised(&self) -> bool {
        self.current_share > self.previous_share
    }

    /// Report whether the movement lowered the share.
    pub const fn lowered(&self) -> bool {
        self.current_share < self.previous_share
    }
}

/// Report whether an answer arrived inside the deadline.
pub fn inside_deadline(arrived: u32, deadline: u32) -> bool {
    arrived <= deadline
}

/// Settle one challenge against a custody record.
///
/// The call moves the share, counts the outcome and returns the movement.
pub fn settle(record: &mut CustodyRecord, state: ChallengeState) -> Movement {
    let previous_share = if record.settled() == 0 {
        STARTING_SHARE
    } else {
        record.share
    };

    let current_share = match state {
        ChallengeState::Open => previous_share,
        ChallengeState::AnsweredInTime => previous_share
            .saturating_add(ANSWER_GAIN)
            .min(CustodyRecord::FULL_SHARE),
        ChallengeState::Missed => previous_share.saturating_sub(MISS_LOSS),
        ChallengeState::Incorrect => previous_share.saturating_sub(FAULT_LOSS),
    };

    match state {
        ChallengeState::Open => {}
        ChallengeState::AnsweredInTime => record.answered = record.answered.saturating_add(1),
        ChallengeState::Missed => record.missed = record.missed.saturating_add(1),
        ChallengeState::Incorrect => record.faults = record.faults.saturating_add(1),
    }
    record.share = current_share;

    Movement {
        state,
        previous_share,
        current_share,
        faults: record.faults,
    }
}

/// The state one challenge reaches from the arrival and the proof outcome.
///
/// An answer that arrives after the deadline counts as missed even where the
/// proof verifies. The deadline closes faster than a network fetch and a proof
/// rebuild complete, so a late answer shows that the validator lacked the
/// record.
pub fn state_for(arrived: u32, deadline: u32, proof_verifies: bool) -> ChallengeState {
    if !inside_deadline(arrived, deadline) {
        ChallengeState::Missed
    } else if proof_verifies {
        ChallengeState::AnsweredInTime
    } else {
        ChallengeState::Incorrect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> CustodyRecord {
        CustodyRecord::default()
    }

    #[test]
    fn a_correct_answer_inside_the_deadline_raises_the_share() {
        let mut record = fresh();
        let movement = settle(&mut record, ChallengeState::AnsweredInTime);
        assert!(movement.raised());
        assert_eq!(movement.previous_share, STARTING_SHARE);
        assert_eq!(movement.current_share, STARTING_SHARE + ANSWER_GAIN);
        assert_eq!(record.answered, 1);
        assert_eq!(record.faults, 0);
    }

    #[test]
    fn a_missed_deadline_lowers_the_share() {
        let mut record = fresh();
        let movement = settle(&mut record, ChallengeState::Missed);
        assert!(movement.lowered());
        assert_eq!(movement.current_share, STARTING_SHARE - MISS_LOSS);
        assert_eq!(record.missed, 1);
        assert_eq!(record.faults, 0);
    }

    #[test]
    fn an_incorrect_proof_lowers_the_share_sharply_and_records_a_fault() {
        let mut record = fresh();
        let movement = settle(&mut record, ChallengeState::Incorrect);
        assert!(movement.lowered());
        assert_eq!(movement.current_share, STARTING_SHARE - FAULT_LOSS);
        assert_eq!(movement.faults, 1);
        assert_eq!(record.faults, 1);
    }

    #[test]
    fn an_incorrect_proof_costs_more_than_a_missed_deadline() {
        let mut missed = fresh();
        let mut incorrect = fresh();
        settle(&mut missed, ChallengeState::Missed);
        settle(&mut incorrect, ChallengeState::Incorrect);
        assert!(incorrect.share < missed.share);
    }

    #[test]
    fn steady_service_carries_the_share_to_the_whole() {
        let mut record = fresh();
        for _ in 0..20 {
            settle(&mut record, ChallengeState::AnsweredInTime);
        }
        assert_eq!(record.share, CustodyRecord::FULL_SHARE);
        assert_eq!(record.answered, 20);
    }

    #[test]
    fn repeated_failure_carries_the_share_to_zero() {
        let mut record = fresh();
        for _ in 0..10 {
            settle(&mut record, ChallengeState::Missed);
        }
        assert_eq!(record.share, 0);
        assert_eq!(record.missed, 10);
    }

    #[test]
    fn the_deadline_rule_names_a_late_answer_as_missed() {
        assert_eq!(
            state_for(43, 43, true),
            ChallengeState::AnsweredInTime,
            "an answer at the deadline counts as timely"
        );
        assert_eq!(state_for(44, 43, true), ChallengeState::Missed);
        assert_eq!(state_for(41, 43, false), ChallengeState::Incorrect);
    }

    #[test]
    fn an_open_challenge_moves_nothing() {
        let mut record = fresh();
        settle(&mut record, ChallengeState::AnsweredInTime);
        let held = record;
        let movement = settle(&mut record, ChallengeState::Open);
        assert_eq!(record.share, held.share);
        assert!(!movement.raised());
        assert!(!movement.lowered());
    }
}
