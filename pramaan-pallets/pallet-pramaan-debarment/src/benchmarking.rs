#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::account;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

fn sample_ministry() -> MinistryId {
	BoundedVec::try_from(b"DPIIT".to_vec()).unwrap()
}

fn sample_reason() -> Reason {
	BoundedVec::try_from(b"false declaration, GFR Rule 151(iii)".to_vec()).unwrap()
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn debar() {
		let caller: T::AccountId = account("nodal_ministry_admin", 0, 0);
		let vendor: T::AccountId = account("vendor", 0, 0);
		let ministry = sample_ministry();
		let reason = sample_reason();

		#[extrinsic_call]
		debar(
			RawOrigin::Signed(caller),
			vendor.clone(),
			ministry.clone(),
			BlockNumberFor::<T>::default(),
			None,
			reason,
		);

		assert!(Debarments::<T>::get(&vendor).iter().any(|r| r.ministry == ministry));
	}

	#[benchmark]
	fn lift_debarment() {
		let caller: T::AccountId = account("nodal_ministry_admin", 0, 0);
		let vendor: T::AccountId = account("vendor", 0, 0);
		let ministry = sample_ministry();
		let reason = sample_reason();

		// Pre-populate a debarment record so lift_debarment has something to remove.
		Pallet::<T>::debar(
			RawOrigin::Signed(caller.clone()).into(),
			vendor.clone(),
			ministry.clone(),
			BlockNumberFor::<T>::default(),
			None,
			reason,
		)
		.unwrap();

		#[extrinsic_call]
		lift_debarment(RawOrigin::Signed(caller), vendor.clone(), ministry.clone());

		assert!(!Debarments::<T>::get(&vendor).iter().any(|r| r.ministry == ministry));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
