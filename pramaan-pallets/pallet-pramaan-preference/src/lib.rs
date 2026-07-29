//! pallet-pramaan-preference
//!
//! "Implements the weighted-average local-content computation across multi-item bids and
//! the 20 percent purchase preference band." (PoC submission document, Part 5.3.)
//!
//! Two things live here, both transcribed verbatim from the PoC document rather than
//! re-derived:
//!
//! 1. A pure, storage-free weighted-average formula for a multi-item bid: "Where a bid
//!    supplies multiple items, the Order specifies a weighted average across them rather
//!    than an item-by-item test: the sum of each item's local value added, divided by the
//!    sum of the sale prices." ([`weighted_local_content_bps`].)
//! 2. The `calculate_preference` extrinsic, which ranks a tender's bids, applies the
//!    P5/P6/P7 eligibility pathways, finds L1, and applies the P8/P9/P10 award-split
//!    rules (PoC document Annexure B) plus the trivial "L1 is already Class-I" case.
//!
//! Technical Implementation Specification Part 5.3. Storage-map, event, and origin-check
//! pattern mirrored from the sibling pallet-pramaan-rule-registry (Part 5.1), which is
//! the primary template for this crate. `calculate_preference`'s origin is
//! `ensure_signed` standing in for "the procuring entity" -- as with the classification
//! pallet, role enforcement (is this signed account actually a procuring-entity account
//! for this ministry?) is a frontend/API concern per Part 9.1/10, not something this
//! pallet's origin check can see.

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

use codec::{Decode, Encode, MaxEncodedLen};
use pramaan_primitives::{
	BasisPoints, CalculationMethod, Divisibility, MinistryId, PathwayId, Rule, RuleLookup, TenderId,
	BPS_DENOMINATOR,
};
use scale_info::TypeInfo;
use sp_runtime::traits::{AtLeast32BitUnsigned, Bounded, SaturatedConversion};
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

/// Balance-unit convention: raw integer Balance units, smallest currency unit (paise),
/// consistent with pallet-pramaan-rule-registry's `Balance` type -- matching the
/// convention pallet-pramaan-certification fixed for the whole build (see its
/// `DPIIT_DEFAULT_CERTIFICATION_THRESHOLD_PAISE`: Rs 10 crore = `100_000_000_00` paise).
/// A `Balance` value read from `Rules[ministry]` via `RuleLookup` and a `tender_value`
/// submitted to this pallet are meant to denote the same unit under this convention.
///
/// PoC document Part 5.3 / Annexure B: P5 "enforces the sub Rupees 200 crore domestic
/// restriction." Rs 200 crore = 200 * 1,00,00,000 rupees * 100 paise/rupee =
/// `200_000_000_000` paise -- too large for `u32` (unlike a rupee-unit reading of the
/// same figure), so it is declared as `u128` and converted into `Balance` via
/// `TryFrom<u128>` (part of `AtLeast32BitUnsigned`'s bound list) with a saturating
/// fallback to `Balance::max_value()` for the pathological case of a runtime whose
/// `Balance` type cannot represent it at all.
pub const DOMESTIC_LIMIT_PAISE_U128: u128 = 200_000_000_000;

/// PoC document Annexure B, verbatim: "MSE and MII, non-MSE Class-I L1: 75 percent to L1;
/// 25 percent offered to an MSE within a 15 percent band." This 15 percent figure is
/// stated independently of the rule's own `preference_margin_bps` (which governs the
/// general Class-I 20-percent-style band in P8/P9), so it is hardcoded here rather than
/// read from `Rule`, per the build instructions.
pub const MSE_BAND_BPS: BasisPoints = 1_500;

/// A local three-variant mirror of what pallet-pramaan-classification's `ClassResult`
/// would carry (Class-I / Class-II / Non-local), defined independently here rather than
/// imported from that crate so the two pallets stay decoupled -- unified only at the
/// runtime/frontend layer, matching how `RuleLookup`/`DebarmentCheck` are trait-based
/// rather than direct crate dependencies (per pramaan-primitives's own doc comments).
/// Only `ClassOne` and `NonLocal` are read by this pallet's pathway logic; `ClassTwo` is
/// carried through so a caller can submit its full classification without lossy mapping.
#[derive(Encode, Decode, Clone, Copy, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub enum ClassResultLike {
	ClassOne,
	ClassTwo,
	NonLocal,
}

/// One bid in a tender's bid list, as submitted to `calculate_preference`. `is_gte`
/// mirrors the per-bid field named in the build spec, but the actual P6 eligibility gate
/// below reads the extrinsic's tender-level `is_tender_gte` parameter, not this field --
/// GTE approval under GFR Rule 161(iv) is a property of the tender as a whole, not of any
/// one bid, so `is_gte` here is carried through as submitted data for downstream/frontend
/// visibility rather than treated as a second, bid-scoped source of truth. This is a
/// judgment call flagged in the handoff notes: if a future revision intends `is_gte` to
/// mean something bid-specific, the P6 check in `calculate_preference` will need revising
/// to consult it instead of (or in addition to) `is_tender_gte`.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub struct BidItem<AccountId, Balance> {
	pub vendor: AccountId,
	pub class: ClassResultLike,
	pub price: Balance,
	pub is_mse: bool,
	pub is_gte: bool,
}

/// One line item within a multi-item bid, for [`weighted_local_content_bps`]. Pure,
/// storage-free -- not itself an extrinsic parameter of this pallet (per the build spec,
/// `BidItem` carries a single `price`/`class` per vendor, already-classified), but
/// exposed so whichever caller needs to reduce a multi-item bid to the one weighted-
/// average local-content percentage PoC document Part 5.3 describes can do so with the
/// exact formula, before that single number is used upstream (e.g. by
/// pallet-pramaan-classification) to produce the `ClassResultLike` carried in `BidItem`.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub struct BidLineItem<Balance> {
	pub sale_price: Balance,
	pub local_value_added: Balance,
}

/// PoC document Part 5.3, verbatim: "the sum of each item's local value added, divided by
/// the sum of the sale prices," expressed in basis points (10_000 = 100%). Returns `None`
/// if `items` is empty or the sale-price sum is zero (division undefined) rather than
/// panicking or silently returning zero.
pub fn weighted_local_content_bps<Balance>(items: &[BidLineItem<Balance>]) -> Option<BasisPoints>
where
	Balance: AtLeast32BitUnsigned + Copy,
{
	if items.is_empty() {
		return None;
	}
	let mut total_local = Balance::zero();
	let mut total_sale = Balance::zero();
	for item in items {
		total_local = total_local.saturating_add(item.local_value_added);
		total_sale = total_sale.saturating_add(item.sale_price);
	}
	if total_sale.is_zero() {
		return None;
	}
	let denominator: Balance = BPS_DENOMINATOR.into();
	let numerator = total_local.saturating_mul(denominator);
	let bps = numerator / total_sale;
	Some(bps.saturated_into::<BasisPoints>())
}

/// PoC document Part 5.3, verbatim: "the maximum amount by which a Class-I supplier's
/// price may exceed the lowest bid and still qualify for the preference: a Class-I bid
/// priced at or below L1 multiplied by 1.20 is offered the chance to match L1's price,
/// and one priced above that band is not." Generalised over `margin_bps` so the same
/// helper serves both the rule's own `preference_margin_bps` (P8/P9) and the hardcoded
/// [`MSE_BAND_BPS`] (P10). The boundary is inclusive (`<=`), matching the PoC document's
/// "at or below" wording exactly.
pub fn price_within_band<Balance>(candidate_price: Balance, l1_price: Balance, margin_bps: BasisPoints) -> bool
where
	Balance: AtLeast32BitUnsigned + Copy,
{
	let denominator: Balance = BPS_DENOMINATOR.into();
	let multiplier: Balance = BPS_DENOMINATOR.saturating_add(margin_bps as u32).into();
	let threshold = l1_price.saturating_mul(multiplier) / denominator;
	candidate_price <= threshold
}

/// One vendor's outcome for one tender, per the tech spec's per-vendor storage map.
/// `decision_path` records which of P5-P10 determined this specific outcome: an
/// eligibility rejection records P6/P7; an award (won, matched, or excluded from the
/// award but still ranked) records P8/P9/P10 depending on the rule's divisibility and
/// which award-split case applied.
#[derive(Encode, Decode, Clone, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen, frame_support::__private::codec::DecodeWithMemTracking)]
pub struct PreferenceOutcome<Balance> {
	pub qualifies: bool,
	pub matched_price: Option<Balance>,
	pub awarded_percent_bps: u16,
	pub decision_path: PathwayId,
}

/// Is sourcing restricted to Class-I suppliers for this ministry?
///
/// Two independent grounds, both of which bar a non-Class-I supplier outright rather
/// than merely deprioritising them:
///   - `para_3a_applicable`: Para 3A of the Order, for the enumerated categories.
///   - `CalculationMethod::NegativeList`: Railways, where everything NOT on a published
///     negative list is Class-I-only irrespective of purchase value.
fn class_one_only<Balance, BlockNumber>(rule: &Rule<Balance, BlockNumber>) -> bool
where
	Balance: codec::Encode + codec::Decode + Clone + PartialEq + Eq + codec::MaxEncodedLen + scale_info::TypeInfo + 'static,
	BlockNumber: codec::Encode + codec::Decode + Clone + PartialEq + Eq + codec::MaxEncodedLen + scale_info::TypeInfo + 'static,
{
	rule.para_3a_applicable || rule.calculation_method == CalculationMethod::NegativeList
}

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

		/// The Balance type carried by `Rule` (via `RuleSource`) and by every bid's
		/// price. `AtLeast32BitUnsigned` (mirrored from pallet-cerulea-pos's `Balance`
		/// bound) is required in addition to rule-registry's own `Balance` bound because
		/// this pallet, unlike rule-registry, does arithmetic on it: summing, comparing,
		/// and computing the basis-point band thresholds.
		type Balance: Parameter + Member + MaxEncodedLen + Copy + Default + TypeInfo + AtLeast32BitUnsigned;

		/// pallet-pramaan-rule-registry (or any other `RuleLookup` implementer), same
		/// associated-type pattern as rule-registry's own `Config::Balance`/
		/// `Config::DpiitOrigin` pattern, decoupled via the trait rather than a direct
		/// crate dependency.
		type RuleSource: RuleLookup<Self::Balance, BlockNumberFor<Self>>;

		/// Upper bound on how many bids one `calculate_preference` call can carry.
		#[pallet::constant]
		type MaxBids: Get<u32>;
	}

	pub type BidItemOf<T> = BidItem<<T as frame_system::Config>::AccountId, <T as Config>::Balance>;
	pub type PreferenceOutcomeOf<T> = PreferenceOutcome<<T as Config>::Balance>;

	#[pallet::storage]
	#[pallet::getter(fn preference_results)]
	pub type PreferenceResults<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		TenderId,
		PreferenceOutcomeOf<T>,
		OptionQuery,
	>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// One event per `calculate_preference` call, summarising the tender-level
		/// outcome: `qualifies` is true whenever the call completed (an eligibility
		/// rejection returns `Err` and emits no event at all, so a successful call is
		/// always "qualifying" in that sense); `matched_price` is `Some(l1_price)` when
		/// an award-split case actually matched a Class-I or MSE bidder to L1's price
		/// (P8/P9/P10), or `None` for the trivial "L1 already Class-I, no match needed"
		/// case. Per-vendor detail lives in `PreferenceResults`, queryable by
		/// `(vendor, tender)`; this event does not name a vendor. No fabricated
		/// `tx_ref`/`timestamp` fields -- the API layer enriches responses with those
		/// from the extrinsic-submission response itself.
		PreferenceCalculated {
			tender: TenderId,
			qualifies: bool,
			matched_price: Option<T::Balance>,
			block_number: BlockNumberFor<T>,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `bids` was empty.
		EmptyBidList,
		/// `bids` longer than `MaxBids`. In practice unreachable because `BoundedVec`
		/// already enforces this bound at the type level before the call body runs;
		/// kept as a named error to match the tech spec's exact error list and as a
		/// defensive re-check.
		ItemCountExceeded,
		/// No `Rule` configured for `ministry` (and no `DefaultRule` fallback set) in
		/// `RuleSource`.
		NoRuleForMinistry,
		/// P5 violation: a non-GTE tender's `tender_value` exceeds the Rs 200 crore
		/// domestic-preference-path limit.
		TenderValueExceedsDomesticLimit,
		/// P6 violation: after excluding every `NonLocal` bid on a non-GTE tender, no
		/// bid remains eligible to be ranked.
		NonLocalNotPermittedOnDomesticTender,
		/// P7 violation: `rule.para_3a_applicable` is true and, after excluding every
		/// non-`ClassOne` bid, no bid remains eligible to be ranked.
		Para3ARequiresClassOne,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Ranks `bids` for `tender` under `ministry`'s rule, applies the P5/P6/P7
		/// eligibility pathways, finds L1 (lowest price among eligible bids), applies
		/// the P8/P9/P10 award-split rules (or the trivial "L1 already Class-I" case),
		/// and writes a `PreferenceResults` entry for every bid that was in scope --
		/// both the ones that ended up ranked and the ones P6/P7 excluded from ranking
		/// (unless exclusion left nothing ranked at all, in which case the whole call
		/// is rejected and nothing is written, per P6/P7's error variants below).
		///
		/// Origin: `ensure_signed`, standing in for "the procuring entity" per the tech
		/// spec Table 5 Origin column. This pallet does not check that the signer is
		/// authorised for `ministry`/`tender` -- that role check is a frontend/API
		/// concern (Part 9.1/10), same placeholder approach as the classification
		/// pallet's origin check.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::calculate_preference())]
		pub fn calculate_preference(
			origin: OriginFor<T>,
			tender: TenderId,
			ministry: MinistryId,
			bids: BoundedVec<BidItemOf<T>, T::MaxBids>,
			tender_value: T::Balance,
			is_tender_gte: bool,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			ensure!(!bids.is_empty(), Error::<T>::EmptyBidList);
			ensure!(bids.len() as u32 <= T::MaxBids::get(), Error::<T>::ItemCountExceeded);

			let rule = T::RuleSource::rule(&ministry).ok_or(Error::<T>::NoRuleForMinistry)?;

			// P5: "enforces the sub Rupees 200 crore domestic restriction."
			let domestic_limit: T::Balance =
				T::Balance::try_from(DOMESTIC_LIMIT_PAISE_U128).unwrap_or_else(|_| T::Balance::max_value());
			ensure!(
				!(tender_value > domestic_limit && !is_tender_gte),
				Error::<T>::TenderValueExceedsDomesticLimit
			);

			// P6 / P7: partition into eligible-to-rank vs excluded-from-ranking, with
			// each exclusion's reason recorded so the right error can be raised if
			// nothing remains eligible.
			let mut eligible: Vec<(usize, &BidItemOf<T>)> = Vec::new();
			let mut excluded: Vec<(usize, PathwayId)> = Vec::new();
			for (i, bid) in bids.iter().enumerate() {
				if class_one_only(&rule) && bid.class != ClassResultLike::ClassOne {
					// P7: "restricting sourcing to Class-I suppliers for items a nodal
					// ministry has notified as having sufficient local capacity."
					excluded.push((i, PathwayId::P7));
				} else if bid.class == ClassResultLike::NonLocal && !is_tender_gte {
					// P6: "admits Non-local suppliers only where a global tender
					// enquiry has been approved under GFR Rule 161(iv)."
					excluded.push((i, PathwayId::P6));
				} else {
					eligible.push((i, bid));
				}
			}

			if eligible.is_empty() {
				// P7 is a ministry-wide setting, so if it's active it is the
				// systemic reason nothing survived filtering (any Non-local bid
				// would also have failed P7 first, since Non-local != Class-I).
				if class_one_only(&rule) {
					return Err(Error::<T>::Para3ARequiresClassOne.into());
				}
				return Err(Error::<T>::NonLocalNotPermittedOnDomesticTender.into());
			}

			// L1: lowest price among eligible bids. `min_by_key` returns the first of
			// several equally-minimum elements, giving a deterministic tie-break.
			let l1_index = eligible
				.iter()
				.min_by_key(|(_, b)| b.price)
				.map(|(i, _)| *i)
				.expect("eligible is non-empty, checked immediately above; qed");
			let l1_bid = &bids[l1_index];
			let l1_price = l1_bid.price;
			let l1_is_class_one = l1_bid.class == ClassResultLike::ClassOne;

			// PathwayId for the cases the PoC document doesn't name explicitly (the
			// trivial "L1 already Class-I" base case, and each case's "no qualifying
			// counter-bid within band" fallback): P8 if the rule's tender is Divisible,
			// P9 if NonDivisible -- the same fork P8/P9 themselves split on.
			let base_path = if rule.divisibility == Divisibility::Divisible { PathwayId::P8 } else { PathwayId::P9 };

			#[derive(Clone, Copy)]
			enum AwardCase {
				/// L1 keeps 100%: either L1 is already Class-I and (already MSE, or no
				/// MSE qualifies within the 15% band), or L1 is not Class-I and no
				/// Class-I bid qualifies within the rule's band.
				Trivial,
				/// PoC Annexure B: "Divisible award when L1 is not Class-I: 50 percent
				/// to L1; the rest offered to the lowest Class-I within the band on a
				/// price match." Payload is the winning bid's index in `bids`.
				SplitP8(usize),
				/// PoC Annexure B: "Non-divisible award when L1 is not Class-I: The
				/// lowest Class-I within the band is offered a price match for the full
				/// contract."
				FullP9(usize),
				/// PoC Annexure B: "MSE and MII, non-MSE Class-I L1: 75 percent to L1;
				/// 25 percent offered to an MSE within a 15 percent band."
				MseP10(usize),
			}

			let case = if !l1_is_class_one {
				let best = eligible
					.iter()
					.filter(|(i, b)| {
						*i != l1_index
							&& b.class == ClassResultLike::ClassOne
							&& price_within_band(b.price, l1_price, rule.preference_margin_bps)
					})
					.min_by_key(|(_, b)| b.price)
					.map(|(i, _)| *i);
				match (rule.divisibility, best) {
					(Divisibility::Divisible, Some(w)) => AwardCase::SplitP8(w),
					(Divisibility::NonDivisible, Some(w)) => AwardCase::FullP9(w),
					(_, None) => AwardCase::Trivial,
				}
			} else if !l1_bid.is_mse {
				let best = eligible
					.iter()
					.filter(|(i, b)| *i != l1_index && b.is_mse && price_within_band(b.price, l1_price, MSE_BAND_BPS))
					.min_by_key(|(_, b)| b.price)
					.map(|(i, _)| *i);
				match best {
					Some(w) => AwardCase::MseP10(w),
					None => AwardCase::Trivial,
				}
			} else {
				AwardCase::Trivial
			};

			let mut event_matched_price: Option<T::Balance> = None;
			let mut outcomes: Vec<(T::AccountId, PreferenceOutcomeOf<T>)> = Vec::new();

			for (i, bid) in eligible.iter() {
				let i = *i;
				let outcome = if i == l1_index {
					match case {
						AwardCase::Trivial => PreferenceOutcome {
							qualifies: true,
							matched_price: None,
							awarded_percent_bps: 10_000,
							decision_path: base_path,
						},
						AwardCase::SplitP8(_) => PreferenceOutcome {
							qualifies: true,
							matched_price: None,
							awarded_percent_bps: 5_000,
							decision_path: PathwayId::P8,
						},
						AwardCase::FullP9(_) => PreferenceOutcome {
							qualifies: false,
							matched_price: None,
							awarded_percent_bps: 0,
							decision_path: PathwayId::P9,
						},
						AwardCase::MseP10(_) => PreferenceOutcome {
							qualifies: true,
							matched_price: None,
							awarded_percent_bps: 7_500,
							decision_path: PathwayId::P10,
						},
					}
				} else {
					match case {
						AwardCase::SplitP8(w) if w == i => {
							event_matched_price = Some(l1_price);
							PreferenceOutcome {
								qualifies: true,
								matched_price: Some(l1_price),
								awarded_percent_bps: 5_000,
								decision_path: PathwayId::P8,
							}
						},
						AwardCase::FullP9(w) if w == i => {
							event_matched_price = Some(l1_price);
							PreferenceOutcome {
								qualifies: true,
								matched_price: Some(l1_price),
								awarded_percent_bps: 10_000,
								decision_path: PathwayId::P9,
							}
						},
						AwardCase::MseP10(w) if w == i => {
							event_matched_price = Some(l1_price);
							PreferenceOutcome {
								qualifies: true,
								matched_price: Some(l1_price),
								awarded_percent_bps: 2_500,
								decision_path: PathwayId::P10,
							}
						},
						AwardCase::Trivial => {
							let qualifies = bid.class == ClassResultLike::ClassOne
								&& price_within_band(bid.price, l1_price, rule.preference_margin_bps);
							PreferenceOutcome { qualifies, matched_price: None, awarded_percent_bps: 0, decision_path: base_path }
						},
						AwardCase::SplitP8(_) => {
							let qualifies = bid.class == ClassResultLike::ClassOne
								&& price_within_band(bid.price, l1_price, rule.preference_margin_bps);
							PreferenceOutcome {
								qualifies,
								matched_price: None,
								awarded_percent_bps: 0,
								decision_path: PathwayId::P8,
							}
						},
						AwardCase::FullP9(_) => {
							let qualifies = bid.class == ClassResultLike::ClassOne
								&& price_within_band(bid.price, l1_price, rule.preference_margin_bps);
							PreferenceOutcome {
								qualifies,
								matched_price: None,
								awarded_percent_bps: 0,
								decision_path: PathwayId::P9,
							}
						},
						AwardCase::MseP10(_) => {
							let qualifies = bid.is_mse && price_within_band(bid.price, l1_price, MSE_BAND_BPS);
							PreferenceOutcome {
								qualifies,
								matched_price: None,
								awarded_percent_bps: 0,
								decision_path: PathwayId::P10,
							}
						},
					}
				};
				outcomes.push((bid.vendor.clone(), outcome));
			}

			// Only reached once every eligibility check has passed for at least one
			// bid, so it is now safe to persist -- a `DispatchResult::Err` anywhere
			// above this point rolls back any writes, but there are none before here.
			for (vendor, outcome) in outcomes.iter() {
				PreferenceResults::<T>::insert(vendor, &tender, outcome.clone());
			}
			for (i, path) in excluded.iter() {
				let vendor = bids[*i].vendor.clone();
				PreferenceResults::<T>::insert(
					&vendor,
					&tender,
					PreferenceOutcome { qualifies: false, matched_price: None, awarded_percent_bps: 0, decision_path: *path },
				);
			}

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::PreferenceCalculated {
				tender: tender.clone(),
				qualifies: true,
				matched_price: event_matched_price,
				block_number,
			});
			::log::info!(
				"[pramaan::preference][prometheus] preference_calculated{{tender={:?}}} {}",
				tender,
				outcomes.len()
			);
			Ok(())
		}
	}
}
