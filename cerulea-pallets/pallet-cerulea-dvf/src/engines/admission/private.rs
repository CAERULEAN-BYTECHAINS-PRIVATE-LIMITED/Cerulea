//! Private chain admission under governance.
//!
//! A private chain admits every validator under governance. The deploying
//! organisation sets the credential requirements. Identity verification through
//! a public registrar plays no part, since the organisation already knows every
//! participant.
//!
//! Three conditions apply.
//!
//! 1. **A governance approval.** A quorum of the governing body approved the
//!    candidate.
//! 2. **The credentials the organisation asks for.** Each credential is a named
//!    attestation the organisation issues.
//! 3. **A licence of the private class or above.** The control plane issues the
//!    attestation and the attestation still holds.

use super::licence::LicenceClass;
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use frame_support::BoundedVec;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// The greatest number of credentials one organisation asks for.
pub const MAX_CREDENTIALS: u32 = 16;

/// The greatest number of bytes one credential name carries.
pub const MAX_CREDENTIAL_BYTES: u32 = 64;

/// One credential the organisation asks for.
pub type CredentialName = BoundedVec<u8, frame_support::traits::ConstU32<MAX_CREDENTIAL_BYTES>>;

/// The credential list one deployment asks for.
pub type CredentialList =
    BoundedVec<CredentialName, frame_support::traits::ConstU32<MAX_CREDENTIALS>>;

/// The requirements one private chain holds.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct PrivateRequirements {
    /// The credentials every candidate holds.
    pub credentials: CredentialList,
    /// The approvals a candidate collects from the governing body.
    pub approvals_required: u32,
    /// The lowest licence class the deployment accepts.
    pub licence_class: LicenceClass,
}

impl PrivateRequirements {
    /// The requirements a deployment starts from.
    ///
    /// One approval and a private licence. The organisation adds credentials
    /// through the Amendment Engine.
    pub fn starting_profile() -> Self {
        PrivateRequirements {
            credentials: CredentialList::default(),
            approvals_required: 1,
            licence_class: LicenceClass::Private,
        }
    }
}

/// What the chain knows about one candidate on a private chain.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct PrivateEvidence {
    /// The approvals the governing body granted.
    pub approvals: u32,
    /// The credentials the candidate holds.
    pub credentials_held: CredentialList,
    /// The licence still holds at this block.
    pub licence_holds: bool,
    /// The class the licence carries.
    pub licence_class: LicenceClass,
}

/// A reason a candidate fails private admission.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum PrivateAdmissionFailure {
    /// The governing body granted too few approvals.
    ApprovalsTooFew,
    /// The candidate lacks a credential the organisation asks for.
    CredentialAbsent,
    /// The licence lapsed or was never issued.
    LicenceInactive,
    /// The licence class sits below the class the organisation asks for.
    LicenceClassTooLow,
}

impl PrivateAdmissionFailure {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            PrivateAdmissionFailure::ApprovalsTooFew => {
                "The governing body granted too few approvals. Collect the remaining approvals, then apply again."
            }
            PrivateAdmissionFailure::CredentialAbsent => {
                "The candidate lacks a credential this deployment asks for. Obtain the credential, then apply again."
            }
            PrivateAdmissionFailure::LicenceInactive => {
                "The Cerulea validator licence is inactive. Renew the licence and submit the fresh attestation."
            }
            PrivateAdmissionFailure::LicenceClassTooLow => {
                "The licence class sits below the class this deployment asks for. Request a licence of the higher class."
            }
        }
    }
}

/// The credentials a candidate is missing.
pub fn missing_credentials(
    requirements: &PrivateRequirements,
    evidence: &PrivateEvidence,
) -> Vec<CredentialName> {
    requirements
        .credentials
        .iter()
        .filter(|wanted| !evidence.credentials_held.iter().any(|held| held == *wanted))
        .cloned()
        .collect()
}

/// Inspect one candidate against every private condition.
pub fn admits(
    requirements: &PrivateRequirements,
    evidence: &PrivateEvidence,
) -> Result<(), PrivateAdmissionFailure> {
    if evidence.approvals < requirements.approvals_required {
        return Err(PrivateAdmissionFailure::ApprovalsTooFew);
    }
    if !missing_credentials(requirements, evidence).is_empty() {
        return Err(PrivateAdmissionFailure::CredentialAbsent);
    }
    if !evidence.licence_holds {
        return Err(PrivateAdmissionFailure::LicenceInactive);
    }
    if evidence.licence_class < requirements.licence_class {
        return Err(PrivateAdmissionFailure::LicenceClassTooLow);
    }
    Ok(())
}

/// Report whether a candidate meets every private condition.
pub fn meets_conditions(requirements: &PrivateRequirements, evidence: &PrivateEvidence) -> bool {
    admits(requirements, evidence).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(bytes: &[u8]) -> CredentialName {
        CredentialName::try_from(bytes.to_vec()).expect("the name fits")
    }

    fn requirements() -> PrivateRequirements {
        let mut held = PrivateRequirements::starting_profile();
        held.approvals_required = 3;
        held.credentials = CredentialList::try_from(sp_std::vec![
            name(b"operator-certificate"),
            name(b"data-protection-officer"),
        ])
        .expect("two names fit");
        held
    }

    fn complete() -> PrivateEvidence {
        PrivateEvidence {
            approvals: 3,
            credentials_held: CredentialList::try_from(sp_std::vec![
                name(b"operator-certificate"),
                name(b"data-protection-officer"),
                name(b"an-extra-credential"),
            ])
            .expect("three names fit"),
            licence_holds: true,
            licence_class: LicenceClass::Private,
        }
    }

    #[test]
    fn a_candidate_meeting_every_condition_is_admitted() {
        assert_eq!(admits(&requirements(), &complete()), Ok(()));
        assert!(meets_conditions(&requirements(), &complete()));
    }

    #[test]
    fn too_few_approvals_refuse_the_candidate() {
        let mut evidence = complete();
        evidence.approvals = 2;
        assert_eq!(
            admits(&requirements(), &evidence),
            Err(PrivateAdmissionFailure::ApprovalsTooFew)
        );
    }

    #[test]
    fn a_missing_credential_refuses_the_candidate() {
        let mut evidence = complete();
        evidence.credentials_held =
            CredentialList::try_from(sp_std::vec![name(b"operator-certificate")])
                .expect("one name fits");
        assert_eq!(
            admits(&requirements(), &evidence),
            Err(PrivateAdmissionFailure::CredentialAbsent)
        );
        assert_eq!(
            missing_credentials(&requirements(), &evidence),
            sp_std::vec![name(b"data-protection-officer")]
        );
    }

    #[test]
    fn an_inactive_licence_refuses_the_candidate() {
        let mut evidence = complete();
        evidence.licence_holds = false;
        assert_eq!(
            admits(&requirements(), &evidence),
            Err(PrivateAdmissionFailure::LicenceInactive)
        );
    }

    #[test]
    fn a_licence_class_below_the_requirement_refuses_the_candidate() {
        let mut held = requirements();
        held.licence_class = LicenceClass::Government;
        assert_eq!(
            admits(&held, &complete()),
            Err(PrivateAdmissionFailure::LicenceClassTooLow)
        );
    }

    #[test]
    fn a_deployment_asking_for_no_credential_admits_on_approval_alone() {
        let held = PrivateRequirements::starting_profile();
        let mut evidence = PrivateEvidence::default();
        evidence.approvals = 1;
        evidence.licence_holds = true;
        evidence.licence_class = LicenceClass::Private;
        assert_eq!(admits(&held, &evidence), Ok(()));
    }

    #[test]
    fn a_candidate_holding_extra_credentials_is_admitted() {
        assert!(missing_credentials(&requirements(), &complete()).is_empty());
    }

    #[test]
    fn the_starting_profile_asks_for_one_approval_and_a_private_licence() {
        let held = PrivateRequirements::starting_profile();
        assert_eq!(held.approvals_required, 1);
        assert_eq!(held.licence_class, LicenceClass::Private);
        assert!(held.credentials.is_empty());
    }

    #[test]
    fn every_admission_failure_names_a_fix() {
        for failure in [
            PrivateAdmissionFailure::ApprovalsTooFew,
            PrivateAdmissionFailure::CredentialAbsent,
            PrivateAdmissionFailure::LicenceInactive,
            PrivateAdmissionFailure::LicenceClassTooLow,
        ] {
            assert!(failure.plain_language().len() > 30);
        }
    }
}
