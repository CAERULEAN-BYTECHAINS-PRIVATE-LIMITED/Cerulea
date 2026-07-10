use frame_support::weights::Weight;

/// Weight functions needed for pallet_cbc_pos.
pub trait WeightInfo {
    fn register_validator() -> Weight;
    fn submit_score() -> Weight;
    fn slash_validator() -> Weight;
    fn bond_stake() -> Weight;
    fn unbond_stake() -> Weight;
    fn boost_score() -> Weight;
    fn slash_score() -> Weight;
}

/// Weights for pallet_cbc_pos using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_validator() -> Weight {
        Weight::from_parts(10_000, 0)
            .saturating_add(Weight::from_parts(0, 500)) // db write
    }

    fn submit_score() -> Weight {
        Weight::from_parts(15_000, 0)
            .saturating_add(Weight::from_parts(0, 1_000)) // db write
    }

    fn slash_validator() -> Weight {
        Weight::from_parts(20_000, 0)
            .saturating_add(Weight::from_parts(0, 1_500)) // db write
    }

    fn bond_stake() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(Weight::from_parts(0, 2_000)) // db write
    }

    fn unbond_stake() -> Weight {
        Weight::from_parts(25_000, 0)
            .saturating_add(Weight::from_parts(0, 2_000)) // db write
    }

    fn boost_score() -> Weight {
        Weight::from_parts(12_000, 0)
            .saturating_add(Weight::from_parts(0, 800)) // db write
    }

    fn slash_score() -> Weight {
        Weight::from_parts(12_000, 0)
            .saturating_add(Weight::from_parts(0, 800)) // db write
    }
}

/// Default implementation for testing and development.
impl WeightInfo for () {
    fn register_validator() -> Weight {
        Weight::from_parts(10_000, 0)
    }

    fn submit_score() -> Weight {
        Weight::from_parts(15_000, 0)
    }

    fn slash_validator() -> Weight {
        Weight::from_parts(20_000, 0)
    }

    fn bond_stake() -> Weight {
        Weight::from_parts(25_000, 0)
    }

    fn unbond_stake() -> Weight {
        Weight::from_parts(25_000, 0)
    }

    fn boost_score() -> Weight {
        Weight::from_parts(12_000, 0)
    }

    fn slash_score() -> Weight {
        Weight::from_parts(12_000, 0)
    }
}