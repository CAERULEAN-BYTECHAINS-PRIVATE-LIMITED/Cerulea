use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_consistency.
pub trait WeightInfo {
	fn declare() -> Weight;
}

/// Weights for pallet_pramaan_consistency using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn declare() -> Weight {
		Weight::from_parts(18_000, 0).saturating_add(Weight::from_parts(0, 2_000))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn declare() -> Weight {
		Weight::from_parts(18_000, 0)
	}
}
