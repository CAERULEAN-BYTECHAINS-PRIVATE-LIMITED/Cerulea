//! pallet-pramaan-rule-registry
//!
//! Holds the nine-parameter rule set for every nodal ministry (PoC document Section
//! 4.1, Table 10, Table 11). This is the single source every other pallet reads from
//! through the `RuleLookup` trait, so a rule change is a storage update, never a
//! redeploy — the mechanism behind PoC document Section 4.6 / 9.3's claim that adding a
//! ministry or rule is configuration, not engineering.
//!
//! Technical Implementation Specification Part 5.1. Storage-map and origin-check
//! pattern mirrored from pallet-cerulea-pos's validator registration extrinsic.

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

use pramaan_primitives::{MinistryId, Rule};
use sp_std::prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pramaan_primitives::{BasisPoints, BPS_DENOMINATOR};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;

		/// The Balance type carried inside `Rule` (certification_threshold,
		/// exemption_floor). Requires `Serialize + DeserializeOwned` (beyond what the
		/// other five pallets' `Balance` associated types need) because this pallet
		/// alone declares a `#[pallet::genesis_config]` embedding `RuleOf<T>`, and the
		/// JSON-based `GenesisBuilder` runtime API serde-(de)serialises the whole
		/// genesis patch.
		type Balance: Parameter
			+ Member
			+ MaxEncodedLen
			+ Copy
			+ Default
			+ TypeInfo
			+ serde::Serialize
			+ serde::de::DeserializeOwned;

		/// DPIIT or Root, per tech spec Table 5's Origin column. A `EnsureOrigin`
		/// combinator (e.g. `EnsureRoot` OR a designated DPIIT signed account) is
		/// supplied by the runtime; this pallet does not hardcode which account is
		/// DPIIT.
		type DpiitOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	pub type RuleOf<T> = Rule<<T as Config>::Balance, BlockNumberFor<T>>;

	#[pallet::storage]
	#[pallet::getter(fn rules)]
	pub type Rules<T: Config> = StorageMap<_, Blake2_128Concat, MinistryId, RuleOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn default_rule)]
	pub type DefaultRule<T: Config> = StorageValue<_, RuleOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn rule_version)]
	pub type RuleVersion<T: Config> = StorageMap<_, Blake2_128Concat, MinistryId, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		RuleUpdated { ministry: MinistryId, new_version: u32, block_number: BlockNumberFor<T> },
		DefaultRuleUpdated { new_version: u32, block_number: BlockNumberFor<T> },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Caller is not DPIIT or Root.
		NotAuthorised,
		/// A field fails validation, e.g. preference_margin_bps > 10000.
		InvalidRule,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::set_rule())]
		pub fn set_rule(origin: OriginFor<T>, ministry: MinistryId, rule: RuleOf<T>) -> DispatchResult {
			T::DpiitOrigin::ensure_origin(origin).map_err(|_| Error::<T>::NotAuthorised)?;
			Self::validate_rule(&rule)?;

			let new_version = RuleVersion::<T>::get(&ministry).saturating_add(1);
			Rules::<T>::insert(&ministry, &rule);
			RuleVersion::<T>::insert(&ministry, new_version);

			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::RuleUpdated { ministry: ministry.clone(), new_version, block_number });
			::log::info!(
				"[pramaan::rule-registry][prometheus] rule_updated{{ministry={:?}}} {}",
				ministry,
				new_version
			);
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::set_default_rule())]
		pub fn set_default_rule(origin: OriginFor<T>, rule: RuleOf<T>) -> DispatchResult {
			T::DpiitOrigin::ensure_origin(origin).map_err(|_| Error::<T>::NotAuthorised)?;
			Self::validate_rule(&rule)?;

			DefaultRule::<T>::put(&rule);
			// The default rule set shares no per-ministry RuleVersion key; report
			// generation 1 always incremented at the storage layer by whichever admin
			// flow calls this. A dedicated DefaultRuleVersion could be added if the
			// default set needs its own audit trail distinct from Rules[ministry].
			let block_number = frame_system::Pallet::<T>::block_number();
			Self::deposit_event(Event::DefaultRuleUpdated { new_version: 1, block_number });
			::log::info!("[pramaan::rule-registry][prometheus] default_rule_updated{{}} 1");
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		fn validate_rule(rule: &RuleOf<T>) -> Result<(), Error<T>> {
			ensure!(rule.preference_margin_bps as u32 <= BPS_DENOMINATOR, Error::<T>::InvalidRule);
			for hsn in rule.hsn_thresholds.iter() {
				ensure!(
					(hsn.class_one_bps as u32) <= BPS_DENOMINATOR && (hsn.class_two_bps as u32) <= BPS_DENOMINATOR,
					Error::<T>::InvalidRule
				);
				ensure!(hsn.class_one_bps >= hsn.class_two_bps, Error::<T>::InvalidRule);
			}
			let _ = BasisPoints::default(); // keep import live if hsn_thresholds is ever empty
			Ok(())
		}

		/// Rules[ministry] if set, else DefaultRule. Used by classification,
		/// preference, and certification via the `RuleLookup` trait below, and by the
		/// Part 5.1 "default fallback" unit test.
		pub fn get_effective_rule(ministry: &MinistryId) -> Option<RuleOf<T>> {
			Rules::<T>::get(ministry).or_else(DefaultRule::<T>::get)
		}
	}

	impl<T: Config> pramaan_primitives::RuleLookup<T::Balance, BlockNumberFor<T>> for Pallet<T> {
		fn rule(ministry: &MinistryId) -> Option<RuleOf<T>> {
			Self::get_effective_rule(ministry)
		}
	}

	/// Pre-loads `DefaultRule` at genesis with the DPIIT default rule set (PoC document
	/// Table 3 / tech spec Part 7.2), so classification/preference/certification have a
	/// fallback rule from block zero. The full 21-ministry set is intentionally NOT
	/// baked in here — it is loaded post-genesis via the idempotent seed script (Part
	/// 7.3/7.4) so it can be re-run during development without a chain restart.
	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub default_rule: Option<RuleOf<T>>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			if let Some(rule) = &self.default_rule {
				Pallet::<T>::validate_rule(rule).expect("genesis default_rule must be valid");
				DefaultRule::<T>::put(rule);
			}
		}
	}
}
