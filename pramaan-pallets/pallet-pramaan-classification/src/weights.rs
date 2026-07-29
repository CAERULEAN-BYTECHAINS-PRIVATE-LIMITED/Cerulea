use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_classification.
pub trait WeightInfo {
	fn classify() -> Weight;
	fn classify_component_level() -> Weight;
}

/// Weights for pallet_pramaan_classification using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn classify() -> Weight {
		Weight::from_parts(18_000, 0).saturating_add(Weight::from_parts(0, 2_000))
	}

	fn classify_component_level() -> Weight {
		Weight::from_parts(24_000, 0).saturating_add(Weight::from_parts(0, 3_000))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn classify() -> Weight {
		Weight::from_parts(18_000, 0)
	}

	fn classify_component_level() -> Weight {
		Weight::from_parts(24_000, 0)
	}
}
