use frame_support::weights::Weight;

/// Weight functions needed for pallet_pramaan_certification.
pub trait WeightInfo {
	fn certify() -> Weight;
	fn empanel_auditor() -> Weight;
	fn remove_auditor() -> Weight;
}

/// Weights for pallet_pramaan_certification using the Substrate node and recommended
/// hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
	fn certify() -> Weight {
		Weight::from_parts(18_000, 0).saturating_add(Weight::from_parts(0, 2_000))
	}
	fn empanel_auditor() -> Weight {
		Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 1_000))
	}
	fn remove_auditor() -> Weight {
		Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 1_000))
	}
}

/// Default implementation for testing and development.
impl WeightInfo for () {
	fn certify() -> Weight {
		Weight::from_parts(18_000, 0)
	}
	fn empanel_auditor() -> Weight {
		Weight::from_parts(12_000, 0)
	}
	fn remove_auditor() -> Weight {
		Weight::from_parts(12_000, 0)
	}
}
