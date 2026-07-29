//! pallet-pramaan-debarment
//!
//! Central debarment registry, checked by every other pallet through the shared
//! `DebarmentCheck` trait so debarment is enforced once rather than duplicated across
//! five engines. Technical Implementation Specification Part 5.5.
//!
//! PoC document Table 8, transcribed verbatim: "Cross-ministry debarment: Siloed and
//! reactive -> One shared ledger, enforced before bidding." A vendor debarred by ANY
//! ministry is blocked when ANY ministry checks them — debarment is not scoped narrowly
//! to only the debarring ministry. See the doc comment on `is_debarred` below for how
//! this pallet resolves the apparent tension between that framing and the tech spec's
//! own "debar then check" unit-test table entry.
//!
//! PoC document Figure 29 / narrative example: "A bid for Chambal Devices, debarred by
//! the Ministry of Defence, is returned RED and blocked on an unrelated tender." See
//! `cross_ministry_debarment_blocks_on_unrelated_ministry` in tests.rs for the
//! integration-shaped reproduction of this scenario.
//!
//! PathwayId::P12 (pramaan-primitives): "P12 handles the consequence of a false
//! declaration: a downgrade in class triggers a penalty of up to 10 percent of contract
//! value ... and debarment of up to two years follows under GFR Rule 151(iii))."
//!
//! Storage-map and origin-check pattern mirrored from pallet-pramaan-rule-registry;
//! `DebarmentRecord<BlockNumber>` / per-vendor BoundedVec-of-records pattern mirrored
//! from pallet-pramaan-consistency's `DeclarationRecord<BlockNumber>` /
//! `Declarations` (itself mirrored from pallet-cerulea-pos::ValidatorSlashingHistory
//! and pallet-pramaan-certification's AuditorLedger) — a record generic only over
//! `BlockNumber`, with fixed-bound fields, rather than a record generic over the whole
//! `T: Config`.

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

use frame_support::traits::Get;
use frame_support::BoundedVec;
use pramaan_primitives::MinistryId;
use sp_std::prelude::*;

/// Upper bound on the byte length of a debarment record's free-text reason. Fixed (not
/// a Config-associated type) since it lives on this pallet's own record type, not on a
/// per-runtime-configurable list length — mirrors `pramaan_primitives::IdBound`
/// (`ConstU32Id`)'s pattern of a concrete `Get<u32>` unit struct. Kept small (256
/// bytes) since this exists for DPIIT national-rollup and CVC audit-review context
/// (tech spec Part 12.2.6 / Section 4.7), not a full case file.
pub struct ReasonBound;
impl Get<u32> for ReasonBound {
	fn get() -> u32 {
		256
	}
}

/// Free-text reason attached to a debarment record.
pub type Reason = BoundedVec<u8, ReasonBound>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;

		/// Upper bound on how many debarment records one vendor can accumulate (one per
		/// debarring ministry, since a vendor can be debarred by more than one
		/// ministry). Mirrors pallet-pramaan-consistency::MaxHistory /
		/// pallet-pramaan-certification::MaxCertsPerAuditor's bound pattern.
		#[pallet::constant]
		type MaxRecords: Get<u32>;
	}

	/// One debarment record. `effective_to: None` means the debarment has no fixed end
	/// (indefinite, until lifted). "Active" means `effective_from <= current_block &&
	/// (effective_to.is_none() || effective_to > current_block)`.
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	pub struct DebarmentRecord<BlockNumber> {
		pub ministry: MinistryId,
		pub effective_from: BlockNumber,
		pub effective_to: Option<BlockNumber>,
		pub reason: Reason,
	}

	pub type DebarmentRecordOf<T> = DebarmentRecord<BlockNumberFor<T>>;

	/// One or more debarment records per vendor, since a vendor can be debarred by more
	/// than one ministry.
	#[pallet::storage]
	#[pallet::getter(fn debarments)]
	pub type Debarments<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<DebarmentRecordOf<T>, T::MaxRecords>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Debarred {
			vendor: T::AccountId,
			ministry: MinistryId,
			effective_from: BlockNumberFor<T>,
			effective_to: Option<BlockNumberFor<T>>,
			block_number: BlockNumberFor<T>,
		},
		DebarmentLifted {
			vendor: T::AccountId,
			ministry: MinistryId,
			block_number: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Caller does not hold NodalMinistryAdmin for this ministry or DPIIT. Reserved
		/// for when role-checking is wired at the runtime level — there is no on-chain
		/// role registry in this build, so `debar`/`lift_debarment` currently only
		/// require `ensure_signed`. Role enforcement is a frontend/API-layer concern for
		/// now, matching the placeholder pattern used by the other pramaan-pallets.
		NotAuthorised,
		/// `lift_debarment` was called but no record for that (vendor, ministry) pair
		/// exists.
		NoSuchDebarment,
		/// The vendor's `Debarments` BoundedVec is already at `MaxRecords`.
		DebarmentRecordsFull,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Record a debarment. Origin: Signed (intended to be restricted to
		/// NodalMinistryAdmin for `ministry`, or DPIIT — see `Error::NotAuthorised`'s doc
		/// comment for why that isn't enforced on-chain yet).
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::debar())]
		pub fn debar(
			origin: OriginFor<T>,
			vendor: T::AccountId,
			ministry: MinistryId,
			effective_from: BlockNumberFor<T>,
			effective_to: Option<BlockNumberFor<T>>,
			reason: Reason,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let record: DebarmentRecordOf<T> = DebarmentRecord {
				ministry: ministry.clone(),
				effective_from,
				effective_to,
				reason,
			};

			Debarments::<T>::try_mutate(&vendor, |records| {
				records.try_push(record).map_err(|_| Error::<T>::DebarmentRecordsFull)
			})?;

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::Debarred {
				vendor: vendor.clone(),
				ministry: ministry.clone(),
				effective_from,
				effective_to,
				block_number,
			});
			::log::info!(
				"[pramaan::debarment][prometheus] debarred{{vendor={:?},ministry={:?}}} 1",
				vendor,
				ministry
			);
			Ok(())
		}

		/// Lift a debarment. Matches by `ministry` — if a vendor has records from
		/// multiple ministries, only the one matching this ministry is removed. Origin:
		/// Signed, same placeholder-authorisation note as `debar`.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::lift_debarment())]
		pub fn lift_debarment(origin: OriginFor<T>, vendor: T::AccountId, ministry: MinistryId) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			Debarments::<T>::try_mutate(&vendor, |records| {
				let len_before = records.len();
				records.retain(|r| r.ministry != ministry);
				ensure!(records.len() < len_before, Error::<T>::NoSuchDebarment);
				Ok::<(), Error<T>>(())
			})?;

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::DebarmentLifted { vendor: vendor.clone(), ministry: ministry.clone(), block_number });
			::log::info!(
				"[pramaan::debarment][prometheus] debarment_lifted{{vendor={:?},ministry={:?}}} 1",
				vendor,
				ministry
			);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// True if `record` is active at `at`: `effective_from <= at &&
		/// (effective_to.is_none() || effective_to > at)`.
		fn is_active(record: &DebarmentRecordOf<T>, at: BlockNumberFor<T>) -> bool {
			record.effective_from <= at && record.effective_to.map_or(true, |to| to > at)
		}

		/// Whether `vendor` has any currently-active debarment record, from any
		/// ministry. This is the vendor-wide enforcement decision described below.
		pub fn has_active_debarment(vendor: &T::AccountId) -> bool {
			let now = frame_system::Pallet::<T>::block_number();
			Debarments::<T>::get(vendor).iter().any(|r| Self::is_active(r, now))
		}
	}

	/// Implements the shared `DebarmentCheck` trait (pramaan-primitives) that
	/// classification, preference, certification, and consistency all consume.
	///
	/// **Interpretation note (see this pallet's build report for the full reasoning):**
	/// The tech spec's own unit-test table says "Debar then check: Debar a vendor, then
	/// call is_debarred -> Returns true for that ministry, false for others," which read
	/// literally would scope enforcement to the debarring ministry only. That directly
	/// conflicts with PoC document Table 8's explicitly-quoted, more prominent claim:
	/// "Cross-ministry debarment: Siloed and reactive -> One shared ledger, enforced
	/// before bidding," and with Figure 29 / the Chambal Devices example, where a
	/// debarment recorded by the Ministry of Defence blocks a bid under an unrelated
	/// ministry. This implementation follows Table 8: `is_debarred(vendor, _ministry)`
	/// returns true if `vendor` has ANY active debarment record from ANY ministry — the
	/// `ministry` parameter is accepted (to match the trait signature fixed in
	/// pramaan-primitives, and for future logging/context use) but does not narrow the
	/// block/deny decision. The tech spec's "false for others" is read as covering an
	/// undebarred vendor/ministry pair, not as a request to scope enforcement narrowly.
	/// Both readings are exercised as separate tests in tests.rs.
	impl<T: Config> pramaan_primitives::DebarmentCheck<T::AccountId> for Pallet<T> {
		fn is_debarred(vendor: &T::AccountId, _ministry: &MinistryId) -> bool {
			Self::has_active_debarment(vendor)
		}
	}
}
