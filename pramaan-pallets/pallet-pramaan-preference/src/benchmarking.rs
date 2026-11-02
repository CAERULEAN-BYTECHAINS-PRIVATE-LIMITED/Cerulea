#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

/// Benchmarking for `calculate_preference` assumes the benchmarking runtime's
/// `Config::RuleSource` is wired to return `Some(rule)` for the `DPIIT` ministry id used
/// below -- e.g. by seeding pallet-pramaan-rule-registry's `Rules` storage (or its
/// `DefaultRule`) in the runtime's benchmarking genesis. That wiring happens at the
/// runtime crate level, the same place any other cross-pallet `RuleLookup` consumer
/// (pallet-pramaan-classification, pallet-pramaan-certification) needs it -- this
/// pallet's own benchmarking.rs has no way to reach into another pallet's storage. Until
/// the runtime crate provides that wiring, a `RuleSource` that always returns `None` will
/// make this benchmark fail with `NoRuleForMinistry`; that is expected, not a bug in this
/// file.
#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn calculate_preference() {
		let ministry: MinistryId = BoundedVec::try_from(b"DPIIT".to_vec()).unwrap();
		let tender: TenderId = BoundedVec::try_from(b"BENCH-TENDER".to_vec()).unwrap();
		let caller: T::AccountId = whitelisted_caller();
		let vendor: T::AccountId = account("vendor", 0, 0);

		let bids: BoundedVec<BidItemOf<T>, T::MaxBids> = BoundedVec::try_from(vec![BidItem {
			vendor: vendor.clone(),
			class: ClassResultLike::ClassOne,
			price: T::Balance::default(),
			is_mse: false,
			is_gte: false,
		}])
		.unwrap();

		#[extrinsic_call]
		calculate_preference(RawOrigin::Signed(caller), tender.clone(), ministry, bids, T::Balance::default(), false);

		assert!(PreferenceResults::<T>::contains_key(&vendor, &tender));
	}
}
