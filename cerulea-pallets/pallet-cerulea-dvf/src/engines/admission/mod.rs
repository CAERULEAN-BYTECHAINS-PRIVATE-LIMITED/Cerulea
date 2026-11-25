//! The Admission Engine.
//!
//! Four files carry the work. `public` holds the conditions of the
//! public-access chain. `private` holds the conditions a private chain sets
//! under governance. `licence` verifies the signed attestation the control
//! plane issues. `exit` holds the conditions that end a seat.

pub mod exit;
pub mod licence;
pub mod private;
pub mod public;

pub use exit::{
    demotions_at_boundary, exit_reason, immediate_removals, is_epoch_boundary, keeps_seat,
    ExitFloors, ExitReason, SeatState,
};

pub use licence::{
    attestation_payload, blocks_until_lapse, inspect_attestation, licence_holds,
    verify_attestation, LicenceAttestation, LicenceClass, LicenceError, LICENCE_DOMAIN,
};

pub use private::{
    admits as admits_private, meets_conditions as meets_private_conditions, missing_credentials,
    CredentialList, CredentialName, PrivateAdmissionFailure, PrivateEvidence, PrivateRequirements,
};

pub use public::{
    admits as admits_public, meets_conditions as meets_public_conditions, IdentityRecord,
    PublicAdmissionFailure, PublicEvidence, PublicFloors, PUBLIC_ACCESS_LICENCE_CLASS,
};
