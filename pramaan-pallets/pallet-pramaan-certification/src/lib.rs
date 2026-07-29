//! pallet-pramaan-certification
//!
//! Enforces the ₹10 crore self-certification vs. auditor-certificate threshold and
//! maintains the Auditor Accountability Ledger, binding every certificate to the
//! auditor who signed it (PoC submission document Section 4.7; Technical
//! Implementation Specification Part 5.4). Implements PathwayId::P11: "P11 carries the
//! certification obligation, which the 19.07.2024 amendment places at execution rather
//! than at bidding" (pramaan-primitives::PathwayId doc comment, transcribed verbatim).
//!
//! **The rule, verbatim from the PoC document (Annexure B / Table 10):**
//! - "Self-certification: Accepted at bidding for all values."
//! - "Auditor certificate: Mandatory at execution for contracts above Rupees 10 crore;
//!   may follow completion within a time limit acceptable to the buyer."
//! - "Futuristic declarations: Not permitted. Local content is assessed as it stands,
//!   not as promised."
//!
//! **Balance-unit convention (must match pallet-pramaan-rule-registry's `Balance`
//! type and every other pallet that compares against `certification_threshold`):**
//! all `Balance` values in this pallet are raw integer Balance units, smallest currency
//! unit, e.g. paise, consistent with pallet-pramaan-rule-registry's Balance type. Under
//! that convention the DPIIT default `certification_threshold` of ₹10 crore is
//! `100_000_000_00` (10,00,00,000 rupees × 100 paise/rupee) — see
//! [`DPIIT_DEFAULT_CERTIFICATION_THRESHOLD_PAISE`] below. This pallet does not itself
//! store the default; pallet-pramaan-rule-registry's `Rules[ministry]` /
//! `DefaultRule.certification_threshold` is the single source of truth, read here
//! through `Config::RuleSource: RuleLookup<...>`, exactly as
//! pallet-pramaan-rule-registry's own doc comment describes for classification,
//! preference, and certification alike.
//!
//! **The Auditor Accountability Ledger** (PoC document Section 4.7, verbatim): "The
//! Auditor Accountability Ledger binds certificates to auditors and surfaces an
//! auditor's related certificates when one of their vendors is found false." Every
//! certificate is stored both by its own id (`Certificates`) AND appended to a
//! per-auditor list (`AuditorLedger`), so that given an auditor's identity, a reviewer
//! can retrieve every certificate they've ever signed the moment one is found false —
//! the cross-reference into pallet-pramaan-debarment's false-declaration finding
//! happens at the frontend/API layer per Section 4.7's own end-to-end description;
//! this pallet's job is only to make the lookup possible by keeping the ledger
//! complete.
//!
//! Storage-map and origin-check pattern mirrored from pallet-pramaan-rule-registry;
//! per-account BoundedVec-of-records pattern mirrored from
//! pallet-cerulea-pos::ValidatorSlashingHistory.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod weights;
pub use weights::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

use pramaan_primitives::{CertificateId, MinistryId, TenderId};
use sp_std::prelude::*;

/// ₹10 crore expressed in the chosen Balance-unit convention (raw integer Balance
/// units, smallest currency unit, e.g. paise, consistent with
/// pallet-pramaan-rule-registry's Balance type): 10,00,00,000 rupees × 100 paise per
/// rupee = 100_000_000_00 paise. DPIIT default per PoC document Table 10. Callers that
/// configure `pallet-pramaan-rule-registry`'s `certification_threshold` in a different
/// Balance denomination must convert accordingly; this constant is documentation of
/// the convention, not itself read by any extrinsic (the threshold actually enforced
/// always comes from `Config::RuleSource`).
pub const DPIIT_DEFAULT_CERTIFICATION_THRESHOLD_PAISE: u128 = 100_000_000_00;

/// Implemented by whatever pallet or mock tracks which accounts hold the
/// `Role::CvcOrAuditReviewer` role (tech spec Part 5.4: "that account must hold the
/// CvcOrAuditReviewer role, or the call fails"). Pallet-local rather than shared via
/// pramaan-primitives because, unlike `DebarmentCheck`, no other pallet in this build
/// needs it — role enforcement is otherwise a frontend/API-layer concern per Part
/// 9.1/10 across this whole build, and there is no role-registry pallet here to bind
/// to instead. Mirrors the shape of `pramaan_primitives::DebarmentCheck`.
pub trait AuditorRoleSource<AccountId> {
	fn is_registered_auditor(who: &AccountId) -> bool;
}

/// No-op default: nobody is a registered auditor, matching `DebarmentCheck`'s
/// fail-safe-closed default of `()` in pramaan-primitives (an unconfigured runtime
/// enforces the safer "no auditor qualifies" rather than the unsafe "every auditor
/// qualifies").
impl<AccountId> AuditorRoleSource<AccountId> for () {
	fn is_registered_auditor(_who: &AccountId) -> bool {
		false
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pramaan_primitives::RuleLookup;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;

		/// The Balance type carried inside a certificate's `value`. Raw integer Balance
		/// units, smallest currency unit, e.g. paise, consistent with
		/// pallet-pramaan-rule-registry's Balance type — this pallet's `RuleSource`
		/// should be instantiated with the same runtime pallet-pramaan-rule-registry so
		/// the two agree on denomination without conversion.
		type Balance: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo + PartialOrd;

		/// Source of `Rules[ministry].certification_threshold`, per PoC document
		/// Section 4.7 / tech spec Part 5.4. Wired to `pallet-pramaan-rule-registry` in
		/// the runtime.
		type RuleSource: RuleLookup<Self::Balance, BlockNumberFor<Self>>;

		/// Source of "does this account hold the CvcOrAuditReviewer role", per tech
		/// spec Part 5.4's certify() behaviour table.
		type AuditorSource: AuditorRoleSource<Self::AccountId>;

		/// Upper bound on how many certificates a single auditor's `AuditorLedger`
		/// entry can hold.
		#[pallet::constant]
		type MaxCertsPerAuditor: Get<u32>;
	}

	/// One issued certificate. `auditor: None` means self-certification (PoC document:
	/// "Self-certification: Accepted at bidding for all values."); `Some(account)`
	/// means the named account, holding the CvcOrAuditReviewer role, certified it.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct Certificate<AccountId, Balance, BlockNumber> {
		pub vendor: AccountId,
		pub tender: TenderId,
		pub value: Balance,
		pub auditor: Option<AccountId>,
		pub block_number: BlockNumber,
	}

	pub type CertificateOf<T> =
		Certificate<<T as frame_system::Config>::AccountId, <T as Config>::Balance, BlockNumberFor<T>>;

	/// One entry per issued certificate.
	#[pallet::storage]
	#[pallet::getter(fn certificates)]
	pub type Certificates<T: Config> = StorageMap<_, Blake2_128Concat, CertificateId, CertificateOf<T>>;

	/// Every certificate a given auditor has signed, for the accountability lookup
	/// (PoC document Section 4.7). Keyed by the auditor's own `AccountId`, matching how
	/// `certify`'s `auditor: Option<AccountId>` parameter is typed per the tech spec's
	/// route table.
	#[pallet::storage]
	#[pallet::getter(fn auditor_ledger)]
	pub type AuditorLedger<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<CertificateId, T::MaxCertsPerAuditor>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Certified {
			certificate_id: CertificateId,
			vendor: T::AccountId,
			auditor: Option<T::AccountId>,
			block_number: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `value >= Rules[ministry].certification_threshold` and `auditor` is `None`.
		AuditorRequired,
		/// Supplied `auditor` account does not hold the CvcOrAuditReviewer role.
		AuditorRoleMissing,
		/// No rule (explicit or default) is configured for `ministry`.
		NoRuleForMinistry,
		/// `certificate_id` has already been used by a prior `certify` call.
		CertificateIdAlreadyUsed,
		/// The auditor's `AuditorLedger` entry is already at `MaxCertsPerAuditor`.
		AuditorLedgerFull,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// PoC document Section 4.7 / tech spec Part 5.4. Below
		/// `Rules[ministry].certification_threshold`, `auditor` may be `None`
		/// (self-certification). At or above threshold, `auditor` must be `Some` and
		/// that account must hold the CvcOrAuditReviewer role, or the call fails. A
		/// voluntary auditor signature below threshold is allowed (the PoC document
		/// does not forbid early certification; only the reverse — an auditor being
		/// required above threshold — is enforced).
		///
		/// `ministry` is not listed as a `certify()` parameter in the tech spec's route
		/// table, but is required here: there is no other way to read
		/// `Rules[ministry].certification_threshold` without knowing which ministry's
		/// rule applies. `certificate_id` is caller-supplied and must be unique,
		/// mirroring how `TenderId`/`MinistryId` are caller-supplied identifiers
		/// elsewhere in this build.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::certify())]
		pub fn certify(
			origin: OriginFor<T>,
			certificate_id: CertificateId,
			ministry: MinistryId,
			vendor: T::AccountId,
			tender: TenderId,
			value: T::Balance,
			auditor: Option<T::AccountId>,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			ensure!(!Certificates::<T>::contains_key(&certificate_id), Error::<T>::CertificateIdAlreadyUsed);

			let rule = T::RuleSource::rule(&ministry).ok_or(Error::<T>::NoRuleForMinistry)?;

			if value >= rule.certification_threshold {
				let auditor_account = auditor.as_ref().ok_or(Error::<T>::AuditorRequired)?;
				ensure!(T::AuditorSource::is_registered_auditor(auditor_account), Error::<T>::AuditorRoleMissing);
			} else if let Some(ref auditor_account) = auditor {
				// Voluntary auditor signature below threshold: still requires the role,
				// so a certificate is never stamped with an unqualified "auditor".
				ensure!(T::AuditorSource::is_registered_auditor(auditor_account), Error::<T>::AuditorRoleMissing);
			}

			let block_number = frame_system::Pallet::<T>::block_number();

			let certificate = Certificate {
				vendor: vendor.clone(),
				tender,
				value,
				auditor: auditor.clone(),
				block_number,
			};
			Certificates::<T>::insert(&certificate_id, &certificate);

			if let Some(ref auditor_account) = auditor {
				AuditorLedger::<T>::try_mutate(auditor_account, |ledger| {
					ledger.try_push(certificate_id.clone()).map_err(|_| Error::<T>::AuditorLedgerFull)
				})?;
			}

			Self::deposit_event(Event::Certified {
				certificate_id: certificate_id.clone(),
				vendor,
				auditor: auditor.clone(),
				block_number,
			});
			::log::info!(
				"[pramaan::certification][prometheus] certified{{certificate_id={:?}}} {}",
				certificate_id,
				auditor.is_some() as u8
			);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Every certificate a given auditor has ever signed, for the Auditor
		/// Accountability Ledger lookup (PoC document Section 4.7).
		pub fn certificates_for_auditor(auditor: &T::AccountId) -> Vec<CertificateId> {
			AuditorLedger::<T>::get(auditor).into_inner()
		}
	}
}
