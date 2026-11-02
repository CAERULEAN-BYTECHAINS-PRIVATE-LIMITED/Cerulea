#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::account;
use frame_benchmarking::v2::*;
use frame_support::BoundedVec;
use frame_system::RawOrigin;

fn sample_product() -> ProductId {
	BoundedVec::try_from(b"PRODUCT-BENCH".to_vec()).unwrap()
}

fn sample_tender(seed: u32) -> TenderId {
	// Seed-distinguished byte identifiers (prefix + big-endian seed bytes) so each
	// pre-populated history entry is a distinct tender for any seed value, staying
	// well within IdBound's 64-byte cap.
	let mut bytes = Vec::from([b'T']);
	bytes.extend_from_slice(&seed.to_be_bytes());
	BoundedVec::try_from(bytes).unwrap()
}

#[benchmarks]
mod benchmarks {
	use super::*;

	#[benchmark]
	fn declare() {
		let caller: T::AccountId = account("procuring_entity", 0, 0);
		let vendor: T::AccountId = account("vendor", 0, 0);
		let product = sample_product();

		// Worst case: pre-fill this (vendor, product) pair's history to one below
		// MaxHistory, so the extrinsic both walks the maximum number of prior entries
		// for comparison AND successfully appends the new one.
		let max_history = T::MaxHistory::get();
		let mut records: BoundedVec<DeclarationRecordOf<T>, T::MaxHistory> = BoundedVec::default();
		for i in 0..max_history.saturating_sub(1) {
			records
				.try_push(DeclarationRecord {
					tender: sample_tender(i),
					local_content_bps: 8_600,
					block_number: BlockNumberFor::<T>::default(),
				})
				.unwrap();
		}
		Declarations::<T>::insert((vendor.clone(), product.clone()), records);

		let new_tender = sample_tender(max_history);

		#[extrinsic_call]
		declare(RawOrigin::Signed(caller), vendor.clone(), product.clone(), new_tender, 3_000);

		assert_eq!(Declarations::<T>::get((vendor, product)).len() as u32, max_history);
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}
