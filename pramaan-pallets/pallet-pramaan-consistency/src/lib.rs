//! pallet-pramaan-consistency
//!
//! The Declaration Consistency Engine. PoC submission document Section 4.7, verbatim:
//! "The Declaration Consistency Engine cross-references every declaration against the
//! vendor's history, identifying the vendor who claims 86 percent local content on one
//! tender and 30 percent for the same product on another." Technical Implementation
//! Specification Part 5.6.
//!
//! **Not a thirteenth pathway.** `pramaan_primitives::PathwayId::P12`'s doc comment
//! quotes the PoC document directly: "Cross-tender consistency is not a thirteenth
//! pathway... It runs across all twelve, because a declaration is checked against the
//! vendor's history regardless of which route it takes." This pallet's `declare`
//! extrinsic is therefore called alongside/after whichever of the other five pallets'
//! extrinsics handled a given vendor declaration's pathway, not in place of any of
//! them.
//!
//! **The canonical named example (PoC document Table 6, verbatim):** "The vendor view
//! for Sabarmati Systems surfaces its earlier 86 percent declaration against the later
//! 30 percent one for the same product." I.e. vendor "Sabarmati Systems" declares 8600
//! bps (86%) local content for a product on one tender, then later declares 3000 bps
//! (30%) for the SAME product on a DIFFERENT tender. This is reproduced exactly as
//! `tests::sabarmati_systems_86_vs_30_flags_inconsistency` (see `tests.rs`), using a
//! test `AccountId` in place of the named vendor for traceability back to Table 6.
//!
//! **Storage-map and origin-check pattern mirrored from pallet-pramaan-rule-registry;
//! per-`(vendor, product)`-key BoundedVec-of-records pattern mirrored from
//! pallet-cerulea-pos::ValidatorSlashingHistory and pallet-pramaan-certification's
//! AuditorLedger.**
//!
//! **Origin.** `declare`'s origin is `ensure_signed` only — the procuring entity
//! submitting a vendor's declared bid content on the vendor's behalf, same placeholder
//! pattern as every other pallet in this build: role enforcement (confirming the signer
//! actually holds the ProcuringEntity role) is a frontend/API-layer concern, not
//! encoded here.
//!
//! **"Cross-references every declaration against the vendor's history" — interpreted
//! as the FULL history, not just the immediately-prior entry.** A new declaration is
//! compared against every existing entry in `Declarations[(vendor, product)]` before it
//! is appended. Two events are involved: `DeclarationRecorded` always fires;
//! `InconsistencyFlagged` fires once per prior entry that differs from the new value by
//! more than `Config::ToleranceBps`. This "once per differing prior entry" reading was
//! chosen over a single aggregate flag because it matches "compares against prior
//! entries" (plural) most literally, and because it gives a reviewer one event per
//! concrete pair of inconsistent tenders rather than collapsing several into one —
//! useful evidence when several prior declarations each individually contradict the new
//! one. `tests::three_prior_wild_declarations_emit_three_flags_on_fourth`
//! demonstrates this: three wildly-differing prior entries for one product make the
//! fourth `declare` call emit three separate `InconsistencyFlagged` events.
//!
//! **Event fields omit `tx_ref`/`timestamp`**, consistent with this build's convention
//! across all six pallets (the general Part 8.2 API route-response shape adds `txRef`
//! at the API layer regardless; pallet-level events stay free of it).

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

use pramaan_primitives::{ProductId, TenderId};
use sp_std::prelude::*;

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

		/// Upper bound on how many declarations one `(vendor, product)` pair can retain
		/// in `Declarations`. Mirrors `pallet-pramaan-certification::MaxCertsPerAuditor`
		/// / `pallet-cerulea-pos::ValidatorSlashingHistory`'s bound pattern.
		#[pallet::constant]
		type MaxHistory: Get<u32>;

		/// Tolerance, in basis points, within which two declarations for the same
		/// `(vendor, product)` pair across different tenders are considered consistent.
		/// `abs_diff(new, prior) > ToleranceBps::get()` triggers `InconsistencyFlagged`.
		/// The PoC document Section 4.7 example (86% vs. 30%, an 5600 bps gap) must
		/// clearly exceed whatever a runtime configures here (e.g. 1000 bps / 10
		/// percentage points is a reasonable default) — this pallet does not hardcode
		/// any specific tolerance value.
		#[pallet::constant]
		type ToleranceBps: Get<u16>;
	}

	/// `VendorId = T::AccountId`, named per the spec's storage-map description for
	/// traceability; used directly as `T::AccountId` throughout the extrinsic body.
	pub type VendorId<T> = <T as frame_system::Config>::AccountId;

	/// One declaration a vendor has made for a given product, on a given tender.
	/// "Every declaration a vendor has made for a given product, across tenders" (spec).
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
	pub struct DeclarationRecord<BlockNumber> {
		pub tender: TenderId,
		pub local_content_bps: u16,
		pub block_number: BlockNumber,
	}

	pub type DeclarationRecordOf<T> = DeclarationRecord<BlockNumberFor<T>>;

	/// Every declaration a vendor has made for a given product, across tenders (spec,
	/// verbatim). Keyed by the full `(vendor, product)` pair with a single
	/// `Blake2_128Concat` hasher over the tuple, exactly as the spec's storage
	/// declaration `StorageMap<(VendorId, ProductId), ...>` states.
	#[pallet::storage]
	#[pallet::getter(fn declarations)]
	pub type Declarations<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		(VendorId<T>, ProductId),
		BoundedVec<DeclarationRecordOf<T>, T::MaxHistory>,
		ValueQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		DeclarationRecorded {
			vendor: T::AccountId,
			product: ProductId,
			tender: TenderId,
			local_content_bps: u16,
			block_number: BlockNumberFor<T>,
		},
		InconsistencyFlagged {
			vendor: T::AccountId,
			product: ProductId,
			prior_tender: TenderId,
			prior_value: u16,
			new_tender: TenderId,
			new_value: u16,
			block_number: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `Declarations[(vendor, product)]` is already at `MaxHistory` entries.
		HistoryFull,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Records a vendor's declared local-content percentage for a product on a
		/// tender, then cross-references it against every prior declaration this vendor
		/// has made for the SAME product (regardless of which tender each prior entry
		/// belongs to) — PoC document Section 4.7: "cross-references every declaration
		/// against the vendor's history." `origin` is `ensure_signed` only; see the
		/// module doc comment for the role-enforcement placeholder note.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::declare())]
		pub fn declare(
			origin: OriginFor<T>,
			vendor: T::AccountId,
			product: ProductId,
			tender: TenderId,
			local_content_bps: u16,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			let key = (vendor.clone(), product.clone());
			let block_number = frame_system::Pallet::<T>::block_number();
			let history = Declarations::<T>::get(&key);

			// Reject before touching anything else (including events) if this
			// (vendor, product) pair's history is already full. Checked first, and
			// strictly before any `deposit_event` call, so a failed `declare` never
			// leaves storage mutated — required for `assert_noop!` in
			// `history_full_rejects_once_max_history_reached` to hold, since FRAME
			// unit tests calling a dispatchable directly do not automatically roll
			// back storage (including the Events map) on error the way on-chain
			// extrinsic application does.
			ensure!((history.len() as u32) < T::MaxHistory::get(), Error::<T>::HistoryFull);

			// Compare the new declaration against every prior entry for this
			// (vendor, product) pair BEFORE appending it, so the new entry never
			// compares against itself. One InconsistencyFlagged per differing prior
			// entry — see module doc comment for why this reading was chosen.
			let mut flag_count: u32 = 0;
			for prior in history.iter() {
				if local_content_bps.abs_diff(prior.local_content_bps) > T::ToleranceBps::get() {
					flag_count = flag_count.saturating_add(1);
					Self::deposit_event(Event::InconsistencyFlagged {
						vendor: vendor.clone(),
						product: product.clone(),
						prior_tender: prior.tender.clone(),
						prior_value: prior.local_content_bps,
						new_tender: tender.clone(),
						new_value: local_content_bps,
						block_number,
					});
				}
			}

			let record = DeclarationRecord { tender: tender.clone(), local_content_bps, block_number };
			Declarations::<T>::try_mutate(&key, |records| {
				records.try_push(record).map_err(|_| Error::<T>::HistoryFull)
			})?;

			Self::deposit_event(Event::DeclarationRecorded {
				vendor: vendor.clone(),
				product,
				tender,
				local_content_bps,
				block_number,
			});
			::log::info!(
				"[pramaan::consistency][prometheus] declaration_recorded{{vendor={:?}}} {}",
				vendor,
				flag_count
			);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Full declaration history for a `(vendor, product)` pair, for the vendor-view
		/// lookup described in PoC document Table 6 ("The vendor view for Sabarmati
		/// Systems surfaces its earlier 86 percent declaration against the later 30
		/// percent one for the same product").
		pub fn history_for(vendor: &T::AccountId, product: &ProductId) -> Vec<DeclarationRecordOf<T>> {
			Declarations::<T>::get((vendor.clone(), product.clone())).into_inner()
		}
	}
}
