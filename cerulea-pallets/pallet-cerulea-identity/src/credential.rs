//! Credential attestation with issuer and expiry.
//!
//! A role names the credentials a holder needs. A credential reaches an
//! account through an attestation. Every attestation carries three facts.
//!
//! 1. **The issuer.** The account that made the attestation. A reader
//!    checks the issuer against the deployment policy.
//! 2. **The expiry.** The moment the attestation lapses, in milliseconds.
//!    An attestation without an expiry holds until a withdrawal.
//! 3. **The evidence.** A commitment over the document the issuer saw.
//!    The document itself stays outside the Blockchain.
//!
//! A credential past its expiry counts as absent. A role that requires an
//! absent credential resolves to nothing, so an expired credential takes
//! the role away without a further call.

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use pallet_cerulea_schema::primitives::BoundedName;
use scale_info::TypeInfo;

/// Why one credential fails to hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub enum CredentialFault {
    /// The account holds no attestation under that credential.
    Absent,
    /// The attestation lapsed.
    Expired,
    /// The issuer withdrew the attestation.
    Withdrawn,
}

impl CredentialFault {
    /// One sentence stating the problem.
    pub fn problem(&self) -> &'static str {
        match self {
            CredentialFault::Absent => "The account holds no attestation under that credential.",
            CredentialFault::Expired => "The attestation of that credential lapsed.",
            CredentialFault::Withdrawn => "The issuer withdrew that attestation.",
        }
    }

    /// One sentence naming the fix.
    pub fn fix(&self) -> &'static str {
        match self {
            CredentialFault::Absent => "Ask the issuer to attest the credential.",
            CredentialFault::Expired => "Ask the issuer to attest the credential again.",
            CredentialFault::Withdrawn => "Ask the issuer to attest the credential again.",
        }
    }
}

/// One credential attestation.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(AccountId))]
pub struct Attestation<AccountId> {
    /// The account that made the attestation.
    pub issuer: AccountId,
    /// The moment the attestation landed, in milliseconds.
    pub issued_at: u64,
    /// The moment the attestation lapses, in milliseconds. An attestation
    /// that holds until a withdrawal names none.
    pub expires_at: Option<u64>,
    /// A commitment over the document the issuer saw.
    pub evidence: [u8; 32],
    /// The issuer withdrew the attestation.
    pub withdrawn: bool,
}

impl<AccountId> Attestation<AccountId> {
    /// Report whether the attestation holds at one moment.
    pub fn holds_at(&self, now: u64) -> Result<(), CredentialFault> {
        if self.withdrawn {
            return Err(CredentialFault::Withdrawn);
        }
        match self.expires_at {
            Some(expiry) if now >= expiry => Err(CredentialFault::Expired),
            _ => Ok(()),
        }
    }

    /// The moment the attestation lapses, or the largest moment when the
    /// attestation holds until a withdrawal.
    pub fn horizon(&self) -> u64 {
        self.expires_at.unwrap_or(u64::MAX)
    }
}

/// The outcome of checking every credential one role requires.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequirementOutcome {
    /// The credentials the account lacks, each with the reason.
    pub missing: Vec<(BoundedName, CredentialFault)>,
}

impl RequirementOutcome {
    /// An outcome naming nothing.
    pub fn clean() -> Self {
        RequirementOutcome {
            missing: Vec::new(),
        }
    }

    /// Report whether every requirement holds.
    pub fn holds(&self) -> bool {
        self.missing.is_empty()
    }

    /// The first credential the account lacks.
    pub fn first(&self) -> Option<&(BoundedName, CredentialFault)> {
        self.missing.first()
    }
}
