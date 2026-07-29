#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

#[benchmarks]
mod benchmarks {
	use super::*;

	/// Requires `T::RuleSource::rule(&ministry)` to return `Some(..)` for the ministry
	/// id used here — in a real runtime that means the benchmark wiring must
	/// pre-populate pallet-pramaan-rule-registry (e.g. via genesis config) — and
	/// `T::DebarmentSource::is_debarred` to return `false` for `vendor`. Both are
	/// runtime integration concerns outside this pallet.
	#[benchmark]
	fn classify() {
		let vendor: T::AccountId = whitelisted_caller();
		let tender: TenderId = BoundedVec::try_from(b"TENDER-BENCH".to_vec()).unwrap();
		let ministry: MinistryId = BoundedVec::try_from(b"DPIIT".to_vec()).unwrap();

		#[extrinsic_call]
		classify(RawOrigin::Signed(vendor.clone()), vendor.clone(), tender.clone(), ministry, 5_000, false);

		assert!(Classifications::<T>::contains_key((vendor, tender)));
	}

	/// Same runtime-wiring caveat as `classify`, and additionally requires the
	/// benchmark's ministry rule to use `CalculationMethod::ComponentLevel` or
	/// `WeightedModule`.
	#[benchmark]
	fn classify_component_level() {
		let vendor: T::AccountId = whitelisted_caller();
		let tender: TenderId = BoundedVec::try_from(b"TENDER-BENCH-2".to_vec()).unwrap();
		let ministry: MinistryId = BoundedVec::try_from(b"DPIIT-COMPONENT".to_vec()).unwrap();
		let components: BoundedVec<ComponentDeclaration, T::MaxComponents> = BoundedVec::try_from(vec![
			ComponentDeclaration {
				name: BoundedVec::try_from(b"PCB".to_vec()).unwrap(),
				declared_bps: 6_000,
				weight_bps: 10_000,
			},
		])
		.unwrap();

		#[extrinsic_call]
		classify_component_level(
			RawOrigin::Signed(vendor.clone()),
			vendor.clone(),
			tender.clone(),
			ministry,
			components,
		);

		assert!(Classifications::<T>::contains_key((vendor, tender)));
	}
}
