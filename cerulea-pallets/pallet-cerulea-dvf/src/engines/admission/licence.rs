//! On-chain licence attestation verification.
//!
//! The Cerulea control plane issues a signed licence attestation. The Admission
//! Engine verifies that attestation on chain. No external call occurs during
//! block production, so a network outage at the control plane never stalls the
//! Blockchain.
//!
//! An attestation names the holder, the licence class, the block at which the
//! licence began and the block at which the licence lapses. A nonce keeps a
//! replayed attestation from reviving a licence the control plane withdrew.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_std::vec::Vec;

/// The domain separator mixed into every licence attestation payload.
///
/// The separator keeps a licence signature apart from every other signature the
/// same key produces.
pub const LICENCE_DOMAIN: &[u8] = b"cerulea.licence.attestation.v1";

/// The class of one validator licence.
///
/// The class sets what the holder may run. A higher class carries a higher cost
/// and a wider permission.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum LicenceClass {
    /// A validator on a private chain of one organisation.
    #[default]
    Private,
    /// A validator on the public-access chain.
    PublicAccess,
    /// A validator running a government deployment.
    Government,
}

impl LicenceClass {
    /// A plain sentence stating what the class permits.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            LicenceClass::Private => {
                "The holder runs a validator on a private chain of one organisation."
            }
            LicenceClass::PublicAccess => "The holder runs a validator on the public-access chain.",
            LicenceClass::Government => {
                "The holder runs a validator inside a government deployment."
            }
        }
    }
}

/// One licence attestation the control plane signed.
#[derive(
    Encode,
    Decode,
    Clone,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct LicenceAttestation<AccountId, BlockNumber> {
    /// The account the licence names.
    pub holder: AccountId,
    /// The class the licence carries.
    pub class: LicenceClass,
    /// The block at which the licence began.
    pub issued_at: BlockNumber,
    /// The block at which the licence lapses.
    pub expires_at: BlockNumber,
    /// The count that keeps a withdrawn licence from returning.
    pub nonce: u64,
}

/// A reason a licence fails verification.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo)]
pub enum LicenceError {
    /// The signature failed against the control plane key.
    SignatureRejected,
    /// The licence lapsed at an earlier block.
    Lapsed,
    /// The licence begins at a later block.
    NotYetActive,
    /// The nonce repeats or steps backward, so the attestation is a replay.
    NonceReplayed,
    /// The class falls below the class this network asks for.
    ClassTooLow,
}

impl LicenceError {
    /// The sentence a person reads, naming the fix.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            LicenceError::SignatureRejected => {
                "The licence signature failed against the control plane key. Request a fresh attestation."
            }
            LicenceError::Lapsed => {
                "The licence lapsed. Renew the licence, then submit the fresh attestation."
            }
            LicenceError::NotYetActive => {
                "The licence begins at a later block. Submit the attestation once that block passes."
            }
            LicenceError::NonceReplayed => {
                "The attestation repeats an earlier count. Submit the newest attestation the control plane issued."
            }
            LicenceError::ClassTooLow => {
                "The licence class sits below the class this network asks for. Request a licence of the higher class."
            }
        }
    }
}

/// The bytes the control plane signs for one attestation.
///
/// The payload mixes the domain separator with the encoded attestation, so a
/// signature over other bytes never passes here.
pub fn attestation_payload<AccountId, BlockNumber>(
    attestation: &LicenceAttestation<AccountId, BlockNumber>,
) -> Vec<u8>
where
    AccountId: Encode,
    BlockNumber: Encode,
{
    let mut payload = Vec::with_capacity(LICENCE_DOMAIN.len() + 64);
    payload.extend_from_slice(LICENCE_DOMAIN);
    payload.extend_from_slice(&attestation.encode());
    payload
}

/// Inspect one attestation against the chain state.
///
/// The call checks the window, the nonce and the class. The signature check
/// happens beside this call, since the signature scheme belongs to the
/// deployment.
pub fn inspect_attestation<AccountId, BlockNumber>(
    attestation: &LicenceAttestation<AccountId, BlockNumber>,
    now: BlockNumber,
    held_nonce: u64,
    required_class: LicenceClass,
) -> Result<(), LicenceError>
where
    BlockNumber: PartialOrd + Copy,
{
    if attestation.nonce <= held_nonce && held_nonce > 0 {
        return Err(LicenceError::NonceReplayed);
    }
    if now < attestation.issued_at {
        return Err(LicenceError::NotYetActive);
    }
    if now >= attestation.expires_at {
        return Err(LicenceError::Lapsed);
    }
    if attestation.class < required_class {
        return Err(LicenceError::ClassTooLow);
    }
    Ok(())
}

/// Verify one attestation in full.
///
/// The signature check runs first, then the window, the nonce and the class.
/// The caller supplies the verification closure, so the deployment picks the
/// signature scheme.
pub fn verify_attestation<AccountId, BlockNumber, Check>(
    attestation: &LicenceAttestation<AccountId, BlockNumber>,
    now: BlockNumber,
    held_nonce: u64,
    required_class: LicenceClass,
    signature_holds: Check,
) -> Result<(), LicenceError>
where
    AccountId: Encode,
    BlockNumber: Encode + PartialOrd + Copy,
    Check: FnOnce(&[u8]) -> bool,
{
    let payload = attestation_payload(attestation);
    if !signature_holds(&payload) {
        return Err(LicenceError::SignatureRejected);
    }
    inspect_attestation(attestation, now, held_nonce, required_class)
}

/// Report whether a licence still holds at one block.
pub fn licence_holds<AccountId, BlockNumber>(
    attestation: &LicenceAttestation<AccountId, BlockNumber>,
    now: BlockNumber,
) -> bool
where
    BlockNumber: PartialOrd + Copy,
{
    now >= attestation.issued_at && now < attestation.expires_at
}

/// The blocks remaining before a licence lapses.
pub fn blocks_until_lapse<AccountId>(
    attestation: &LicenceAttestation<AccountId, u32>,
    now: u32,
) -> u32 {
    attestation.expires_at.saturating_sub(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn attestation() -> LicenceAttestation<u64, u32> {
        LicenceAttestation {
            holder: 7,
            class: LicenceClass::PublicAccess,
            issued_at: 100,
            expires_at: 1_000,
            nonce: 5,
        }
    }

    #[test]
    fn an_attestation_inside_its_window_passes() {
        assert_eq!(
            verify_attestation(&attestation(), 500, 4, LicenceClass::PublicAccess, |_| true),
            Ok(())
        );
    }

    #[test]
    fn a_rejected_signature_fails_verification() {
        assert_eq!(
            verify_attestation(&attestation(), 500, 4, LicenceClass::PublicAccess, |_| {
                false
            }),
            Err(LicenceError::SignatureRejected)
        );
    }

    #[test]
    fn a_lapsed_licence_fails_verification() {
        assert_eq!(
            verify_attestation(&attestation(), 1_000, 4, LicenceClass::PublicAccess, |_| {
                true
            }),
            Err(LicenceError::Lapsed)
        );
        assert!(!licence_holds(&attestation(), 1_000));
    }

    #[test]
    fn a_licence_before_its_first_block_fails_verification() {
        assert_eq!(
            verify_attestation(&attestation(), 50, 4, LicenceClass::PublicAccess, |_| true),
            Err(LicenceError::NotYetActive)
        );
        assert!(!licence_holds(&attestation(), 50));
    }

    #[test]
    fn a_replayed_attestation_fails_verification() {
        assert_eq!(
            verify_attestation(&attestation(), 500, 5, LicenceClass::PublicAccess, |_| true),
            Err(LicenceError::NonceReplayed)
        );
        assert_eq!(
            verify_attestation(&attestation(), 500, 9, LicenceClass::PublicAccess, |_| true),
            Err(LicenceError::NonceReplayed)
        );
    }

    #[test]
    fn a_class_below_the_network_requirement_fails_verification() {
        let mut held = attestation();
        held.class = LicenceClass::Private;
        assert_eq!(
            verify_attestation(&held, 500, 4, LicenceClass::PublicAccess, |_| true),
            Err(LicenceError::ClassTooLow)
        );
    }

    #[test]
    fn a_higher_class_satisfies_a_lower_requirement() {
        let mut held = attestation();
        held.class = LicenceClass::Government;
        assert_eq!(
            verify_attestation(&held, 500, 4, LicenceClass::PublicAccess, |_| true),
            Ok(())
        );
    }

    #[test]
    fn the_payload_carries_the_domain_separator() {
        let payload = attestation_payload(&attestation());
        assert!(payload.starts_with(LICENCE_DOMAIN));
        assert_ne!(payload, attestation().encode());
    }

    #[test]
    fn two_attestations_sign_over_different_payloads() {
        let first = attestation();
        let mut second = attestation();
        second.nonce = 6;
        assert_ne!(attestation_payload(&first), attestation_payload(&second));
    }

    #[test]
    fn the_first_attestation_passes_with_no_held_count() {
        let mut held = attestation();
        held.nonce = 1;
        assert_eq!(
            verify_attestation(&held, 500, 0, LicenceClass::PublicAccess, |_| true),
            Ok(())
        );
    }

    #[test]
    fn the_engine_reports_the_blocks_until_a_lapse() {
        assert_eq!(blocks_until_lapse(&attestation(), 900), 100);
        assert_eq!(blocks_until_lapse(&attestation(), 1_100), 0);
    }

    #[test]
    fn every_licence_failure_names_a_fix() {
        for failure in [
            LicenceError::SignatureRejected,
            LicenceError::Lapsed,
            LicenceError::NotYetActive,
            LicenceError::NonceReplayed,
            LicenceError::ClassTooLow,
        ] {
            assert!(failure.plain_language().len() > 30);
        }
        for class in [
            LicenceClass::Private,
            LicenceClass::PublicAccess,
            LicenceClass::Government,
        ] {
            assert!(class.plain_language().len() > 30);
        }
    }
}
