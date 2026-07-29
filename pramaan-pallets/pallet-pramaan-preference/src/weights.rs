use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_preference.
pub trait WeightInfo {
	fn calculate_preference() -> Weight;
}

/// Weights for pallet_pramaan_preference using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn calculate_preference() -> Weight {
		Weight::from_parts(25_000, 0).saturating_add(Weight::from_parts(0, 3_500))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn calculate_preference() -> Weight {
		Weight::from_parts(25_000, 0)
	}
}
