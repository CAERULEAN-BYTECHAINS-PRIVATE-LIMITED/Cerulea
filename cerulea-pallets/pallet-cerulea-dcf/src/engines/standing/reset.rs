//! Standing reset on proven misconduct.
//!
//! Proven misconduct returns standing to zero. Rebuilding takes the months that
//! accumulation took. This is the equivalent of a financial penalty, with time
//! as the value destroyed.
//!
//! A reset needs proof rather than suspicion. Each kind of misconduct carries
//! the evidence the chain accepts. The chain records the reset so an operator
//! reads what happened and when.

use super::accumulate::ServiceRecord;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;

/// The kinds of misconduct the chain proves.
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
pub enum Misconduct {
    /// The validator authored two blocks at one slot.
    TwoBlocksAtOneSlot,
    /// The validator signed two results at one height.
    TwoResultsAtOneHeight,
    /// The validator sent a custody proof that failed verification.
    FalseCustodyProof,
    /// The validator sent a sampling report the chain rejected.
    FalseSamplingReport,
    /// The validator held a seat with a lapsed licence.
    LapsedLicence,
}

impl Misconduct {
    /// A plain sentence stating what the chain proved.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            Misconduct::TwoBlocksAtOneSlot => {
                "The validator authored two blocks at one slot. Standing returns to zero."
            }
            Misconduct::TwoResultsAtOneHeight => {
                "The validator signed two results at one height. Standing returns to zero."
            }
            Misconduct::FalseCustodyProof => {
                "The validator sent a custody proof that failed verification. Standing returns to zero."
            }
            Misconduct::FalseSamplingReport => {
                "The validator sent a sampling report the chain rejected. Standing returns to zero."
            }
            Misconduct::LapsedLicence => {
                "The validator held a seat with a lapsed licence. Standing returns to zero."
            }
        }
    }

    /// Report whether this kind of misconduct also removes the seat at once.
    ///
    /// Two blocks at one slot and two results at one height break safety, so
    /// the seat goes immediately. The remaining kinds take effect at the epoch
    /// boundary through the Admission Engine.
    pub const fn removes_the_seat_at_once(&self) -> bool {
        matches!(
            self,
            Misconduct::TwoBlocksAtOneSlot | Misconduct::TwoResultsAtOneHeight
        )
    }
}

/// The record of one reset.
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
pub struct ResetRecord {
    /// The misconduct the chain proved.
    pub misconduct: Misconduct,
    /// The epoch at which the reset happened.
    pub epoch: u32,
    /// The epochs of service the reset destroyed.
    pub epochs_destroyed: u32,
}

/// Return the standing of one validator to zero.
///
/// The service record loses every served epoch, so rebuilding takes the same
/// time the first climb took. The record of the reset carries what was lost.
pub fn reset(record: &mut ServiceRecord, misconduct: Misconduct, epoch: u32) -> ResetRecord {
    let epochs_destroyed = record.epochs_served;
    record.epochs_served = 0;
    record.last_served_epoch = 0;
    record.joined_epoch = epoch;
    ResetRecord {
        misconduct,
        epoch,
        epochs_destroyed,
    }
}

#[cfg(test)]
mod tests {
    use super::super::accumulate::{accumulated, enter, record_epoch, ServiceEvidence};
    use super::*;
    use sp_runtime::Perbill;

    fn every_condition() -> ServiceEvidence {
        ServiceEvidence {
            custody_held: true,
            participated: true,
            sampling_accurate: true,
        }
    }

    fn served(epochs: u32) -> ServiceRecord {
        let mut record = enter(0);
        for epoch in 1..=epochs {
            record_epoch(&mut record, epoch, every_condition());
        }
        record
    }

    #[test]
    fn proven_misconduct_returns_standing_to_zero() {
        let mut record = served(120);
        assert!(accumulated(&record, 180) > Perbill::zero());

        let outcome = reset(&mut record, Misconduct::TwoBlocksAtOneSlot, 200);
        assert_eq!(accumulated(&record, 180), Perbill::zero());
        assert_eq!(outcome.epochs_destroyed, 120);
        assert_eq!(outcome.epoch, 200);
    }

    #[test]
    fn rebuilding_takes_the_time_the_first_climb_took() {
        let mut record = served(90);
        let before = accumulated(&record, 180);
        reset(&mut record, Misconduct::FalseCustodyProof, 100);

        // The climb back to the same share takes the same count of epochs.
        let mut regained = 0u32;
        let mut epoch = 101u32;
        while accumulated(&record, 180) < before {
            record_epoch(&mut record, epoch, every_condition());
            regained += 1;
            epoch += 1;
            assert!(regained <= 200, "the climb back ends");
        }
        assert_eq!(regained, 90);
    }

    #[test]
    fn a_reset_record_names_the_misconduct() {
        let mut record = served(10);
        let outcome = reset(&mut record, Misconduct::FalseSamplingReport, 11);
        assert_eq!(outcome.misconduct, Misconduct::FalseSamplingReport);
        assert!(outcome.misconduct.plain_language().len() > 30);
    }

    #[test]
    fn breaking_safety_removes_the_seat_at_once() {
        assert!(Misconduct::TwoBlocksAtOneSlot.removes_the_seat_at_once());
        assert!(Misconduct::TwoResultsAtOneHeight.removes_the_seat_at_once());
        assert!(!Misconduct::FalseCustodyProof.removes_the_seat_at_once());
        assert!(!Misconduct::FalseSamplingReport.removes_the_seat_at_once());
        assert!(!Misconduct::LapsedLicence.removes_the_seat_at_once());
    }

    #[test]
    fn every_kind_of_misconduct_carries_a_plain_sentence() {
        for misconduct in [
            Misconduct::TwoBlocksAtOneSlot,
            Misconduct::TwoResultsAtOneHeight,
            Misconduct::FalseCustodyProof,
            Misconduct::FalseSamplingReport,
            Misconduct::LapsedLicence,
        ] {
            assert!(misconduct.plain_language().len() > 30);
        }
    }

    #[test]
    fn a_reset_sets_the_joining_epoch_to_the_reset_epoch() {
        let mut record = served(5);
        reset(&mut record, Misconduct::LapsedLicence, 77);
        assert_eq!(record.joined_epoch, 77);
        assert_eq!(record.epochs_served, 0);
        assert_eq!(record.last_served_epoch, 0);
    }
}
