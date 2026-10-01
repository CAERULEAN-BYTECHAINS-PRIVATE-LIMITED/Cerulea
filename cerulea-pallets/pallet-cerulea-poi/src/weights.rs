#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_cerulea_poi.
pub trait WeightInfo {
	fn store_something() -> Weight;
	fn submit_inference() -> Weight;
	fn challenge_inference() -> Weight;
	fn resolve_challenge() -> Weight;
	fn epoch_cleanup() -> Weight;
}

/// Weights for pallet_cerulea_poi using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(PhantomData<T>);
impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	/// Storage: Template Something (r:0 w:1)
	/// Proof: Template Something (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn store_something() -> Weight {
		Weight::from_parts(10_000, 0)
			.saturating_add(Weight::from_parts(0, 500)) // db write
	}

	/// Weight for `submit_inference`.
	fn submit_inference() -> Weight {
		Weight::from_parts(20_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64)) // db write
	}

	/// Weight for `challenge_inference`.
	fn challenge_inference() -> Weight {
		Weight::from_parts(30_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(1_u64, 1_u64)) // db read + write
	}

	/// Weight for `resolve_challenge`.
	fn resolve_challenge() -> Weight {
		Weight::from_parts(25_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(2_u64, 2_u64)) // db reads + writes
	}

	/// Weight for `epoch_cleanup`.
	fn epoch_cleanup() -> Weight {
		Weight::from_parts(40_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(10_u64, 5_u64)) // cleanup operations
	}
}

// For backwards compatibility and tests
impl WeightInfo for () {
	/// Storage: Template Something (r:0 w:1)
	/// Proof: Template Something (max_values: Some(1), max_size: Some(4), added: 499, mode: MaxEncodedLen)
	fn store_something() -> Weight {
		// Proof Size summary in bytes:
		//  Measured:  `0`
		//  Estimated: `0`
		// Minimum execution time: 8_000_000 picoseconds.
		Weight::from_parts(9_000_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}

	fn submit_inference() -> Weight {
		Weight::from_parts(20_000, 0)
			.saturating_add(RocksDbWeight::get().writes(1_u64))
	}

	fn challenge_inference() -> Weight {
		Weight::from_parts(30_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(1_u64, 1_u64))
	}

	fn resolve_challenge() -> Weight {
		Weight::from_parts(25_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(2_u64, 2_u64))
	}

	fn epoch_cleanup() -> Weight {
		Weight::from_parts(40_000, 0)
			.saturating_add(RocksDbWeight::get().reads_writes(10_u64, 5_u64))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::mock::Test;
	use frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND;

	/// The reference time one block of the Cerulea chain spends.
	const BLOCK_REF_TIME: u64 = 2 * WEIGHT_REF_TIME_PER_SECOND;

	/// Check that one figure charges reference time and fits inside one block.
	fn fits_one_block(call: &str, figure: Weight) {
		assert!(figure.ref_time() > 0, "the figure of {call} charges reference time");
		assert!(
			figure.ref_time() <= BLOCK_REF_TIME,
			"the figure of {call} fits inside one block"
		);
	}

	/// Check one measured figure against the figure a test runtime charges.
	///
	/// The measured figure fits inside one block. The measured figure reaches
	/// the test runtime figure in reference time and in proof size. Both
	/// figures cover the storage charge the call performs.
	fn holds(call: &str, measured: Weight, plain: Weight, storage: Weight) {
		fits_one_block(call, measured);
		fits_one_block(call, plain);
		assert!(
			measured.all_gte(plain),
			"the figure of {call} reaches the figure a test runtime charges"
		);
		assert!(
			measured.ref_time() >= storage.ref_time(),
			"the figure of {call} covers the storage charge of the call"
		);
		assert!(
			plain.ref_time() >= storage.ref_time(),
			"the test runtime figure of {call} covers the storage charge of the call"
		);
	}

	#[test]
	fn every_call_figure_fits_one_block_and_covers_its_storage_charge() {
		let db = RocksDbWeight::get();
		holds(
			"submit_inference",
			<SubstrateWeight<Test> as WeightInfo>::submit_inference(),
			<() as WeightInfo>::submit_inference(),
			db.writes(1),
		);
		holds(
			"challenge_inference",
			<SubstrateWeight<Test> as WeightInfo>::challenge_inference(),
			<() as WeightInfo>::challenge_inference(),
			db.reads_writes(1, 1),
		);
		holds(
			"resolve_challenge",
			<SubstrateWeight<Test> as WeightInfo>::resolve_challenge(),
			<() as WeightInfo>::resolve_challenge(),
			db.reads_writes(2, 2),
		);
		holds(
			"epoch_cleanup",
			<SubstrateWeight<Test> as WeightInfo>::epoch_cleanup(),
			<() as WeightInfo>::epoch_cleanup(),
			db.reads_writes(10, 5),
		);
	}

	#[test]
	fn every_call_figure_rises_with_the_storage_work_of_the_call() {
		let submit = <SubstrateWeight<Test> as WeightInfo>::submit_inference();
		let challenge = <SubstrateWeight<Test> as WeightInfo>::challenge_inference();
		let resolve = <SubstrateWeight<Test> as WeightInfo>::resolve_challenge();
		let cleanup = <SubstrateWeight<Test> as WeightInfo>::epoch_cleanup();
		assert!(submit.ref_time() < challenge.ref_time());
		assert!(challenge.ref_time() < resolve.ref_time());
		assert!(resolve.ref_time() < cleanup.ref_time());
	}

	#[test]
	fn the_template_figure_fits_one_block_and_carries_its_proof_size() {
		let measured = <SubstrateWeight<Test> as WeightInfo>::store_something();
		let plain = <() as WeightInfo>::store_something();
		fits_one_block("store_something", measured);
		fits_one_block("store_something", plain);
		// The measured template figure carries a proof size of five hundred
		// bytes. The test runtime figure carries the storage write in
		// reference time and a proof size of zero.
		assert_eq!(measured.proof_size(), 500);
		assert!(measured.proof_size() >= plain.proof_size());
		assert!(plain.ref_time() >= RocksDbWeight::get().writes(1).ref_time());
	}
}
