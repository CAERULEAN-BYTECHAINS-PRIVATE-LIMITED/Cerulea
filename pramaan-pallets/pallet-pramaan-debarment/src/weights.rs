use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_debarment.
pub trait WeightInfo {
	fn debar() -> Weight;
	fn lift_debarment() -> Weight;
}

/// Weights for pallet_pramaan_debarment using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn debar() -> Weight {
		Weight::from_parts(15_000, 0).saturating_add(Weight::from_parts(0, 1_500))
	}

	fn lift_debarment() -> Weight {
		Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 1_000))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn debar() -> Weight {
		Weight::from_parts(15_000, 0)
	}

	fn lift_debarment() -> Weight {
		Weight::from_parts(12_000, 0)
	}
}
