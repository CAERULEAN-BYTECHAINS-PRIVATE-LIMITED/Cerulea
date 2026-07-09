#![cfg_attr(rustfmt, rustfmt_skip)]
#![allow(unused_parens)]
#![allow(unused_imports)]

use frame_support::{traits::Get, weights::{Weight, constants::RocksDbWeight}};
use core::marker::PhantomData;

/// Weight functions needed for pallet_cbc_poi.
pub trait WeightInfo {
	fn store_something() -> Weight;
	fn submit_inference() -> Weight;
	fn challenge_inference() -> Weight;
	fn resolve_challenge() -> Weight;
	fn epoch_cleanup() -> Weight;
}

/// Weights for pallet_cbc_poi using the Substrate node and recommended hardware.
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
