//! pallet-pramaan-classification
//!
//! Implements the local-content percentage classification and the Class-I / Class-II /
//! Non-local boundary logic exactly as PoC document Section 7.1 states it, including the
//! two inclusive-boundary edge cases the submission document calls out explicitly: "The
//! classification boundaries are inclusive at the lower end: a supplier at exactly 50
//! percent is Class-I, and a supplier at exactly 20 percent is Class-II rather than
//! Non-local." Formula (PoC document Section 7.1, transcribed verbatim, not
//! re-derived): "(Sale price minus value of imported content) x 100 / sale price." This
//! pallet does not itself compute that price arithmetic — the caller supplies an
//! already-computed `declared_local_content_bps` (the price arithmetic runs off-chain /
//! in the GeM simulator that supplies the declared percentage, tech spec Part 8.3); this
//! pallet's job is the boundary/classification logic plus the PathwayId::P2-P4 special
//! cases.
//!
//! Technical Implementation Specification Part 5.2. Storage-map and origin-check pattern
//! mirrored from pallet-pramaan-rule-registry, this build's sibling pallet using the
//! same primitives crate.

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

use pramaan_primitives::{CalculationMethod, IdBound, MinistryId, Rule, TenderId};
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pramaan_primitives::{BasisPoints, BPS_DENOMINATOR, DebarmentCheck, RuleLookup};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;

		/// The Balance type carried inside `Rule` (certification_threshold,
		/// exemption_floor). Mirrors pallet-pramaan-rule-registry's `Config::Balance` so
		/// the two pallets' Balance types can be unified at the runtime level.
		type Balance: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo;

		/// Reads the nine-parameter rule for a ministry (falling back to the DPIIT
		/// default where the ministry has none of its own). In the real runtime this is
		/// wired to pallet-pramaan-rule-registry.
		type RuleSource: pramaan_primitives::RuleLookup<Self::Balance, BlockNumberFor<Self>>;

		/// Cross-ministry debarment check, consulted before every classification. In the
		/// real runtime this is wired to pallet-pramaan-debarment.
		type DebarmentSource: pramaan_primitives::DebarmentCheck<Self::AccountId>;

		/// Upper bound on how many components a single component-level / weighted-module
		/// declaration (PathwayId::P2) may carry.
		#[pallet::constant]
		type MaxComponents: Get<u32>;
	}

	pub type RuleOf<T> = Rule<<T as Config>::Balance, BlockNumberFor<T>>;

	/// The tri-state classification result (PoC document Section 7.1), plus a fourth
	/// variant for PathwayId::P3's "no automatable formula" case. Variant names match the
	/// tech spec's unit-test table exactly: "Class = ClassOne", "Class = ClassTwo, not
	/// NonLocal", "Class = NonLocal".
	#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
	pub enum ClassResult {
		ClassOne,
		ClassTwo,
		NonLocal,
		/// PathwayId::P3: "P3 covers categories where no automatable formula exists and
		/// routes them to a recorded human decision rather than computing one."
		ManualReviewRequired,
	}

	/// One component's declared local-content share and its weight within the product,
	/// for PathwayId::P2 (ComponentLevel / WeightedModule). Weights across all
	/// components in one declaration must sum to `BPS_DENOMINATOR` (10_000 = 100%).
	#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
	pub struct ComponentDeclaration {
		pub name: BoundedVec<u8, IdBound>,
		pub declared_bps: BasisPoints,
		pub weight_bps: BasisPoints,
	}

	/// Last classification result per vendor per tender.
	#[pallet::storage]
	#[pallet::getter(fn classifications)]
	pub type Classifications<T: Config> =
		StorageMap<_, Blake2_128Concat, (T::AccountId, TenderId), ClassResult>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A classification was recorded. There is no `tx_ref` field: a pallet extrinsic
		/// cannot read its own transaction hash mid-execution (that is a node/RPC-layer
		/// concept), and the frontend API layer (Part 8.3) already has the hash from the
		/// extrinsic submission call, so it enriches the response with `tx_ref` when it
		/// reads this event back after finality. There is likewise no `timestamp` field:
		/// this runtime's `construct_runtime!` does not wire in `pallet_timestamp`, so
		/// `block_number` is the only time reference a pallet event can carry here —
		/// matching pallet-pramaan-rule-registry's own events.
		Classified {
			vendor: T::AccountId,
			tender: TenderId,
			class: ClassResult,
			block_number: BlockNumberFor<T>,
		},
		/// PathwayId::P3: emitted in addition to `Classified` (whose `class` field will
		/// be `ManualReviewRequired`) so a listener can act on the "needs a human
		/// decision" case without inspecting the `class` field of every `Classified`
		/// event.
		ManualReviewRequired { vendor: T::AccountId, tender: TenderId, block_number: BlockNumberFor<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `DebarmentSource::is_debarred` returned true for this vendor/ministry pair.
		VendorDebarred,
		/// Neither `Rules[ministry]` nor `DefaultRule` is set in the rule source.
		NoRuleForMinistry,
		/// PathwayId::P2 component weights must sum to exactly 10_000 (100%).
		WeightsDoNotSumToOneHundredPercent,
		/// `classify_component_level` was called against a rule whose
		/// `calculation_method` is not `ComponentLevel` or `WeightedModule`.
		NotComponentLevelMethod,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// PathwayId::P1 (standard formula against the applicable threshold), P3
		/// (Custom -> manual review), and P4 (PLI deeming). Origin: any signed account —
		/// for this PoC, any registered procuring entity is accepted as an origin-check
		/// placeholder; role/entitlement checks happen at the frontend/API layer (tech
		/// spec Part 9.1 / 10) since there is no on-chain procuring-entity registry yet.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::classify())]
		pub fn classify(
			origin: OriginFor<T>,
			vendor: T::AccountId,
			tender: TenderId,
			ministry: MinistryId,
			declared_local_content_bps: BasisPoints,
			is_pli_manufacturer: bool,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			ensure!(!T::DebarmentSource::is_debarred(&vendor, &ministry), Error::<T>::VendorDebarred);

			let rule = T::RuleSource::rule(&ministry).ok_or(Error::<T>::NoRuleForMinistry)?;

			let class = if rule.calculation_method == CalculationMethod::Custom {
				// PathwayId::P3: no automatable formula exists; route to a recorded
				// human decision rather than computing one.
				ClassResult::ManualReviewRequired
			} else if rule.pli_linked && is_pli_manufacturer {
				// PathwayId::P4: PLI deeming — Class-II regardless of the declared
				// percentage, because the incentive has been received.
				ClassResult::ClassTwo
			} else {
				// PathwayId::P1: standard formula against the rule's own threshold.
				let (class_one_bps, class_two_bps) = Self::thresholds_for_rule(&rule);
				Self::classify_bps(declared_local_content_bps, class_one_bps, class_two_bps)
			};

			Classifications::<T>::insert((vendor.clone(), tender.clone()), class);

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::Classified {
				vendor: vendor.clone(),
				tender: tender.clone(),
				class,
				block_number,
			});
			if class == ClassResult::ManualReviewRequired {
				Self::deposit_event(Event::ManualReviewRequired { vendor, tender, block_number });
			}
			::log::info!(
				"[pramaan::classification][prometheus] classified{{ministry={:?}}} {:?}",
				ministry,
				class
			);
			Ok(())
		}

		/// PathwayId::P2 (ComponentLevel / WeightedModule): computes the weighted
		/// average of each component's declared local content, then applies the same
		/// boundary logic as `classify`. Does not layer in the P3/P4 special cases: a
		/// rule using ComponentLevel or WeightedModule is, by construction, not Custom,
		/// and the tech spec defines PLI deeming only for the single-declaration path.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::classify_component_level())]
		pub fn classify_component_level(
			origin: OriginFor<T>,
			vendor: T::AccountId,
			tender: TenderId,
			ministry: MinistryId,
			components: BoundedVec<ComponentDeclaration, T::MaxComponents>,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			ensure!(!T::DebarmentSource::is_debarred(&vendor, &ministry), Error::<T>::VendorDebarred);

			let rule = T::RuleSource::rule(&ministry).ok_or(Error::<T>::NoRuleForMinistry)?;
			ensure!(
				matches!(
					rule.calculation_method,
					CalculationMethod::ComponentLevel | CalculationMethod::WeightedModule
				),
				Error::<T>::NotComponentLevelMethod
			);

			let total_weight: u32 =
				components.iter().fold(0u32, |acc, c| acc.saturating_add(c.weight_bps as u32));
			ensure!(total_weight == BPS_DENOMINATOR, Error::<T>::WeightsDoNotSumToOneHundredPercent);

			let effective_bps = Self::weighted_average_bps(&components);
			let (class_one_bps, class_two_bps) = Self::thresholds_for_rule(&rule);
			let class = Self::classify_bps(effective_bps, class_one_bps, class_two_bps);

			Classifications::<T>::insert((vendor.clone(), tender.clone()), class);

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::Classified { vendor, tender, class, block_number });
			::log::info!(
				"[pramaan::classification][prometheus] classified_component_level{{ministry={:?}}} {:?}",
				ministry,
				class
			);
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Inclusive-lower-bound boundary logic, PoC document Section 7.1: `>=
		/// class_one_bps` is Class-I, `>= class_two_bps` (and below class_one_bps) is
		/// Class-II, else Non-local.
		pub fn classify_bps(bps: BasisPoints, class_one_bps: BasisPoints, class_two_bps: BasisPoints) -> ClassResult {
			if bps >= class_one_bps {
				ClassResult::ClassOne
			} else if bps >= class_two_bps {
				ClassResult::ClassTwo
			} else {
				ClassResult::NonLocal
			}
		}

		/// The applicable Class-I/Class-II threshold pair for a rule. `Rule::hsn_thresholds`
		/// is keyed per-HSN-code (PoC Section 4.1.2), but the tech spec's `classify`
		/// extrinsic signature carries no HSN code parameter, so this pallet takes the
		/// rule's first configured threshold entry — matching how every reference rule in
		/// this build (rule-registry's own tests, this pallet's mock) carries exactly one
		/// entry — and falls back to the DPIIT default (5000/2000 bps) if none is
		/// configured.
		fn thresholds_for_rule(rule: &RuleOf<T>) -> (BasisPoints, BasisPoints) {
			rule.hsn_thresholds
				.first()
				.map(|t| (t.class_one_bps, t.class_two_bps))
				.unwrap_or_else(pramaan_primitives::dpiit_default_thresholds)
		}

		/// Weighted average of `declared_bps` across components, weighted by
		/// `weight_bps`. Callers must first ensure weights sum to `BPS_DENOMINATOR` (see
		/// `WeightsDoNotSumToOneHundredPercent`); this function does not re-check that.
		pub fn weighted_average_bps(
			components: &BoundedVec<ComponentDeclaration, T::MaxComponents>,
		) -> BasisPoints {
			let weighted_sum: u64 = components.iter().fold(0u64, |acc, c| {
				acc.saturating_add((c.declared_bps as u64).saturating_mul(c.weight_bps as u64))
			});
			(weighted_sum / (BPS_DENOMINATOR as u64)) as BasisPoints
		}
	}
}
