//! Standing accumulation through service.
//!
//! A new validator enters at zero and rises through proven custody, correct
//! authoring, timely voting and accurate verification sampling. Money buys
//! licences and machines quickly. Time cannot be bought.
//!
//! Accumulation runs over a ramp period. The ramp is a count of epochs. A
//! validator that serves every epoch of the ramp reaches the whole standing
//! share. A validator that misses epochs rises more slowly, since only served
//! epochs count.
//!
//! An epoch counts as served where the validator met the service conditions in
//! that epoch. The conditions are proven custody, authored or voted blocks, and
//! accurate sampling reports.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The epochs a validator serves to reach the whole standing share.
///
/// The ramp is long on purpose. Standing is the part of the Trust Score that
/// money cannot buy.
pub const DEFAULT_RAMP_EPOCHS: u32 = 180;

/// The service record of one validator.
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
pub struct ServiceRecord {
    /// The epochs the validator served.
    pub epochs_served: u32,
    /// The last epoch in which the validator met the service conditions.
    pub last_served_epoch: u32,
    /// The epoch at which the validator entered the set.
    pub joined_epoch: u32,
}

/// The conditions one epoch of service asks for.
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
pub struct ServiceEvidence {
    /// The validator answered every custody challenge it received.
    pub custody_held: bool,
    /// The validator authored or voted on blocks in the epoch.
    pub participated: bool,
    /// The validator reported sampling results that matched the chain.
    pub sampling_accurate: bool,
}

impl ServiceEvidence {
    /// Report whether the evidence meets the service conditions.
    ///
    /// Every condition holds for an epoch to count as served.
    pub const fn meets_conditions(&self) -> bool {
        self.custody_held && self.participated && self.sampling_accurate
    }

    /// The sentence a person reads where the epoch fails to count.
    pub const fn plain_language(&self) -> &'static str {
        if !self.custody_held {
            "The validator left a custody challenge unanswered. Answer every challenge inside the deadline."
        } else if !self.participated {
            "The validator authored and voted on no block. Take part in every slot the committee names."
        } else if !self.sampling_accurate {
            "The sampling report disagreed with the chain. Re-execute the sample on the pinned runtime."
        } else {
            "The epoch counts as served."
        }
    }
}

/// The standing share one service record reaches.
///
/// The share rises in a straight line from zero at entry to the whole share at
/// the end of the ramp. A ramp of zero epochs grants the whole share at once,
/// which suits a private chain that admits validators under governance.
pub fn accumulated(record: &ServiceRecord, ramp_epochs: u32) -> Perbill {
    if ramp_epochs == 0 {
        return Perbill::one();
    }
    if record.epochs_served >= ramp_epochs {
        return Perbill::one();
    }
    Perbill::from_rational(record.epochs_served, ramp_epochs)
}

/// Record one epoch of service against a validator.
///
/// An epoch meeting every condition raises the served count. An epoch missing a
/// condition leaves the count where it stands, so standing stalls rather than
/// falls. Decay handles the fall.
pub fn record_epoch(record: &mut ServiceRecord, epoch: u32, evidence: ServiceEvidence) -> bool {
    if !evidence.meets_conditions() {
        return false;
    }
    if epoch <= record.last_served_epoch && record.epochs_served > 0 {
        // The epoch already counted. Service counts once per epoch.
        return false;
    }
    record.epochs_served = record.epochs_served.saturating_add(1);
    record.last_served_epoch = epoch;
    true
}

/// Open a service record for a validator entering at one epoch.
pub fn enter(epoch: u32) -> ServiceRecord {
    ServiceRecord {
        epochs_served: 0,
        last_served_epoch: 0,
        joined_epoch: epoch,
    }
}

/// The epochs remaining before the validator reaches the whole standing share.
pub fn epochs_remaining(record: &ServiceRecord, ramp_epochs: u32) -> u32 {
    ramp_epochs.saturating_sub(record.epochs_served)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_condition() -> ServiceEvidence {
        ServiceEvidence {
            custody_held: true,
            participated: true,
            sampling_accurate: true,
        }
    }

    #[test]
    fn a_new_validator_enters_at_zero() {
        let record = enter(10);
        assert_eq!(record.epochs_served, 0);
        assert_eq!(record.joined_epoch, 10);
        assert_eq!(accumulated(&record, DEFAULT_RAMP_EPOCHS), Perbill::zero());
    }

    #[test]
    fn standing_rises_in_a_straight_line_over_the_ramp() {
        let mut record = enter(0);
        for epoch in 1..=100u32 {
            assert!(record_epoch(&mut record, epoch, every_condition()));
        }
        assert_eq!(record.epochs_served, 100);
        assert_eq!(accumulated(&record, 200), Perbill::from_percent(50));
        assert_eq!(accumulated(&record, 100), Perbill::one());
        assert_eq!(accumulated(&record, 400), Perbill::from_percent(25));
    }

    #[test]
    fn service_over_the_whole_ramp_reaches_the_whole_share() {
        let mut record = enter(0);
        for epoch in 1..=DEFAULT_RAMP_EPOCHS {
            record_epoch(&mut record, epoch, every_condition());
        }
        assert_eq!(accumulated(&record, DEFAULT_RAMP_EPOCHS), Perbill::one());
        assert_eq!(epochs_remaining(&record, DEFAULT_RAMP_EPOCHS), 0);
    }

    #[test]
    fn an_epoch_missing_a_condition_fails_to_count() {
        let mut record = enter(0);
        let missing_custody = ServiceEvidence {
            custody_held: false,
            ..every_condition()
        };
        assert!(!record_epoch(&mut record, 1, missing_custody));
        assert_eq!(record.epochs_served, 0);

        let missing_participation = ServiceEvidence {
            participated: false,
            ..every_condition()
        };
        assert!(!record_epoch(&mut record, 1, missing_participation));
        assert_eq!(record.epochs_served, 0);

        let missing_sampling = ServiceEvidence {
            sampling_accurate: false,
            ..every_condition()
        };
        assert!(!record_epoch(&mut record, 1, missing_sampling));
        assert_eq!(record.epochs_served, 0);
    }

    #[test]
    fn one_epoch_counts_once() {
        let mut record = enter(0);
        assert!(record_epoch(&mut record, 5, every_condition()));
        assert!(!record_epoch(&mut record, 5, every_condition()));
        assert_eq!(record.epochs_served, 1);
    }

    #[test]
    fn a_validator_missing_epochs_rises_more_slowly() {
        let mut steady = enter(0);
        let mut patchy = enter(0);
        for epoch in 1..=60u32 {
            record_epoch(&mut steady, epoch, every_condition());
            if epoch % 2 == 0 {
                record_epoch(&mut patchy, epoch, every_condition());
            }
        }
        assert!(accumulated(&patchy, 180) < accumulated(&steady, 180));
        assert_eq!(patchy.epochs_served, 30);
        assert_eq!(steady.epochs_served, 60);
    }

    #[test]
    fn time_cannot_be_shortened() {
        // Reaching half the share takes half the ramp. No call shortens it.
        let mut record = enter(0);
        let mut reached_half = None;
        for epoch in 1..=DEFAULT_RAMP_EPOCHS {
            record_epoch(&mut record, epoch, every_condition());
            if reached_half.is_none()
                && accumulated(&record, DEFAULT_RAMP_EPOCHS) >= Perbill::from_percent(50)
            {
                reached_half = Some(epoch);
            }
        }
        assert_eq!(reached_half, Some(DEFAULT_RAMP_EPOCHS / 2));
    }

    #[test]
    fn a_ramp_of_zero_grants_the_whole_share_at_once() {
        let record = enter(0);
        assert_eq!(accumulated(&record, 0), Perbill::one());
    }

    #[test]
    fn every_failing_condition_names_a_fix() {
        let cases = [
            ServiceEvidence {
                custody_held: false,
                ..every_condition()
            },
            ServiceEvidence {
                participated: false,
                ..every_condition()
            },
            ServiceEvidence {
                sampling_accurate: false,
                ..every_condition()
            },
        ];
        for case in cases.iter() {
            assert!(case.plain_language().len() > 30);
        }
        assert!(every_condition().meets_conditions());
    }
}
