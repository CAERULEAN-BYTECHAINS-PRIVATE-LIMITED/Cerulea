//! Public-access admission conditions.
//!
//! A validator holds a seat on the public-access chain while meeting every
//! condition. Four conditions apply.
//!
//! 1. **Verified legal identity.** A registrar confirmed the legal person or
//!    organisation behind the account.
//! 2. **Active Cerulea validator licence.** The control plane issued an
//!    attestation and the attestation still holds.
//! 3. **Proven storage of the full history.** The Custody Engine proved the
//!    validator serves records drawn from the whole history.
//! 4. **A machine answering challenges within deadline.** The custody share
//!    sits at or above the floor.
//!
//! Admission runs continuously rather than per block. Losing a condition
//! triggers demotion at the next epoch boundary.

use super::licence::LicenceClass;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::Perbill;

/// The custody share a public-access validator holds, at least.
pub const DEFAULT_CUSTODY_FLOOR_PERCENT: u32 = 60;

/// The history share a public-access validator proves, at least.
///
/// The share is the part of the sampled history the validator served
/// correctly.
pub const DEFAULT_HISTORY_FLOOR_PERCENT: u32 = 90;

/// The class of licence the public-access chain asks for.
pub const PUBLIC_ACCESS_LICENCE_CLASS: LicenceClass = LicenceClass::PublicAccess;

/// The identity a registrar confirmed.
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
pub struct IdentityRecord {
    /// The registrar that confirmed the identity.
    pub registrar: u32,
    /// The block at which the confirmation happened.
    pub confirmed_at: u32,
    /// The block at which the confirmation lapses. Zero means no lapse.
    pub expires_at: u32,
}

impl IdentityRecord {
    /// Report whether the confirmation still holds at one block.
    pub const fn holds(&self, now: u32) -> bool {
        self.confirmed_at > 0 && (self.expires_at == 0 || now < self.expires_at)
    }
}

/// The floors a public-access validator meets.
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
pub struct PublicFloors {
    /// The custody share the validator holds, at least.
    pub custody: Perbill,
    /// The share of the sampled history the validator served correctly.
    pub history: Perbill,
}

impl PublicFloors {
    /// The floors a deployment starts from.
    pub fn starting_profile() -> Self {
        PublicFloors {
            custody: Perbill::from_percent(DEFAULT_CUSTODY_FLOOR_PERCENT),
            history: Perbill::from_percent(DEFAULT_HISTORY_FLOOR_PERCENT),
        }
    }
}

impl Default for PublicFloors {
    fn default() -> Self {
        PublicFloors::starting_profile()
    }
}

/// What the chain knows about one candidate.
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
pub struct PublicEvidence {
    /// The identity a registrar confirmed.
    pub identity: IdentityRecord,
    /// The licence still holds at this block.
    pub licence_holds: bool,
    /// The class the licence carries.
    pub licence_class: LicenceClass,
    /// The share of the sampled history the validator served correctly.
    pub history_share: Perbill,
    /// The custody share the validator carries.
    pub custody_share: Perbill,
}

/// A reason a candidate fails public-access admission.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum PublicAdmissionFailure {
    /// No registrar confirmed the legal identity, or the confirmation lapsed.
    IdentityAbsent,
    /// The licence lapsed or was never issued.
    LicenceInactive,
    /// The licence class sits below the class this network asks for.
    LicenceClassTooLow,
    /// The validator failed to serve records drawn from the whole history.
    HistoryUnproven,
    /// The machine failed to answer challenges within deadline.
    CustodyBelowFloor,
}

impl PublicAdmissionFailure {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            PublicAdmissionFailure::IdentityAbsent => {
                "No registrar confirmed the legal identity. Complete identity verification, then apply again."
            }
            PublicAdmissionFailure::LicenceInactive => {
                "The Cerulea validator licence is inactive. Renew the licence and submit the fresh attestation."
            }
            PublicAdmissionFailure::LicenceClassTooLow => {
                "The licence class sits below the public-access class. Request a public-access licence."
            }
            PublicAdmissionFailure::HistoryUnproven => {
                "The validator failed to serve records from the whole history. Fetch the full history, then apply again."
            }
            PublicAdmissionFailure::CustodyBelowFloor => {
                "The machine answered too few challenges inside the deadline. Raise machine availability, then apply again."
            }
        }
    }
}

/// Inspect one candidate against every public-access condition.
///
/// The call returns the first condition the candidate fails, so the sentence a
/// person reads names one fix rather than a list.
pub fn admits(
    evidence: &PublicEvidence,
    floors: &PublicFloors,
    now: u32,
) -> Result<(), PublicAdmissionFailure> {
    if !evidence.identity.holds(now) {
        return Err(PublicAdmissionFailure::IdentityAbsent);
    }
    if !evidence.licence_holds {
        return Err(PublicAdmissionFailure::LicenceInactive);
    }
    if evidence.licence_class < PUBLIC_ACCESS_LICENCE_CLASS {
        return Err(PublicAdmissionFailure::LicenceClassTooLow);
    }
    if evidence.history_share < floors.history {
        return Err(PublicAdmissionFailure::HistoryUnproven);
    }
    if evidence.custody_share < floors.custody {
        return Err(PublicAdmissionFailure::CustodyBelowFloor);
    }
    Ok(())
}

/// Report whether a candidate meets every public-access condition.
pub fn meets_conditions(evidence: &PublicEvidence, floors: &PublicFloors, now: u32) -> bool {
    admits(evidence, floors, now).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn complete() -> PublicEvidence {
        PublicEvidence {
            identity: IdentityRecord {
                registrar: 1,
                confirmed_at: 10,
                expires_at: 0,
            },
            licence_holds: true,
            licence_class: LicenceClass::PublicAccess,
            history_share: Perbill::from_percent(100),
            custody_share: Perbill::from_percent(100),
        }
    }

    #[test]
    fn a_candidate_meeting_every_condition_is_admitted() {
        assert_eq!(admits(&complete(), &PublicFloors::default(), 100), Ok(()));
        assert!(meets_conditions(&complete(), &PublicFloors::default(), 100));
    }

    #[test]
    fn a_candidate_without_a_confirmed_identity_is_refused() {
        let mut evidence = complete();
        evidence.identity = IdentityRecord::default();
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::IdentityAbsent)
        );
    }

    #[test]
    fn a_lapsed_identity_confirmation_is_refused() {
        let mut evidence = complete();
        evidence.identity.expires_at = 50;
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::IdentityAbsent)
        );
        assert_eq!(admits(&evidence, &PublicFloors::default(), 40), Ok(()));
    }

    #[test]
    fn a_candidate_without_an_active_licence_is_refused() {
        let mut evidence = complete();
        evidence.licence_holds = false;
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::LicenceInactive)
        );
    }

    #[test]
    fn a_licence_of_a_lower_class_is_refused() {
        let mut evidence = complete();
        evidence.licence_class = LicenceClass::Private;
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::LicenceClassTooLow)
        );
    }

    #[test]
    fn a_candidate_holding_part_of_the_history_is_refused() {
        let mut evidence = complete();
        evidence.history_share = Perbill::from_percent(80);
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::HistoryUnproven)
        );
    }

    #[test]
    fn a_machine_answering_too_slowly_is_refused() {
        let mut evidence = complete();
        evidence.custody_share = Perbill::from_percent(40);
        assert_eq!(
            admits(&evidence, &PublicFloors::default(), 100),
            Err(PublicAdmissionFailure::CustodyBelowFloor)
        );
    }

    #[test]
    fn a_candidate_exactly_at_the_floors_is_admitted() {
        let floors = PublicFloors::default();
        let mut evidence = complete();
        evidence.history_share = floors.history;
        evidence.custody_share = floors.custody;
        assert_eq!(admits(&evidence, &floors, 100), Ok(()));
    }

    #[test]
    fn the_starting_floors_read_sixty_and_ninety_percent() {
        let floors = PublicFloors::starting_profile();
        assert_eq!(floors.custody, Perbill::from_percent(60));
        assert_eq!(floors.history, Perbill::from_percent(90));
    }

    #[test]
    fn every_admission_failure_names_a_fix() {
        for failure in [
            PublicAdmissionFailure::IdentityAbsent,
            PublicAdmissionFailure::LicenceInactive,
            PublicAdmissionFailure::LicenceClassTooLow,
            PublicAdmissionFailure::HistoryUnproven,
            PublicAdmissionFailure::CustodyBelowFloor,
        ] {
            assert!(failure.plain_language().len() > 30);
        }
    }
}
