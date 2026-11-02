use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_rule_registry.
pub trait WeightInfo {
	fn set_rule() -> Weight;
	fn set_default_rule() -> Weight;
}

/// Weights for pallet_pramaan_rule_registry using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn set_rule() -> Weight {
		Weight::from_parts(15_000, 0).saturating_add(Weight::from_parts(0, 1_500))
	}

	fn set_default_rule() -> Weight {
		Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 1_000))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn set_rule() -> Weight {
		Weight::from_parts(15_000, 0)
	}

	fn set_default_rule() -> Weight {
		Weight::from_parts(12_000, 0)
	}
}
