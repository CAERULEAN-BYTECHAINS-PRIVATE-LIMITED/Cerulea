use frame_support::weights::Weight;

/// Weight functions needed for pallet_cerulea_pos.
pub trait WeightInfo {
    fn register_validator() -> Weight;
    fn submit_score() -> Weight;
    fn slash_validator() -> Weight;
    fn bond_stake() -> Weight;
    fn unbond_stake() -> Weight;
    fn boost_score() -> Weight;
    fn slash_score() -> Weight;
}

/// Weights for pallet_cerulea_pos using the Substrate node and recommended hardware.
pub struct SubstrateWeight<T>(core::marker::PhantomData<T>);

impl<T: frame_system::Config> WeightInfo for SubstrateWeight<T> {
    fn register_validator() -> Weight {
        Weight::from_parts(10_000, 0).saturating_add(Weight::from_parts(0, 500))
        // db write
    }

    fn submit_score() -> Weight {
        Weight::from_parts(15_000, 0).saturating_add(Weight::from_parts(0, 1_000))
        // db write
    }

    fn slash_validator() -> Weight {
        Weight::from_parts(20_000, 0).saturating_add(Weight::from_parts(0, 1_500))
        // db write
    }

    fn bond_stake() -> Weight {
        Weight::from_parts(25_000, 0).saturating_add(Weight::from_parts(0, 2_000))
        // db write
    }

    fn unbond_stake() -> Weight {
        Weight::from_parts(25_000, 0).saturating_add(Weight::from_parts(0, 2_000))
        // db write
    }

    fn boost_score() -> Weight {
        Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 800))
        // db write
    }

    fn slash_score() -> Weight {
        Weight::from_parts(12_000, 0).saturating_add(Weight::from_parts(0, 800))
        // db write
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::Test;
    use frame_support::weights::constants::WEIGHT_REF_TIME_PER_SECOND;

    /// The reference time one block of the Cerulea chain spends.
    const BLOCK_REF_TIME: u64 = 2 * WEIGHT_REF_TIME_PER_SECOND;

    /// Check one measured figure against the figure a test runtime charges.
    ///
    /// The measured figure carries the storage charge the call performs, so
    /// it sits above the plain reference time in at least one part. Every
    /// figure fits inside one block.
    fn holds(call: &str, measured: Weight, plain: Weight) {
        assert!(
            measured.ref_time() > 0,
            "the figure of {call} charges reference time"
        );
        assert!(
            measured.proof_size() > 0,
            "the figure of {call} charges proof size"
        );
        assert!(
            measured.ref_time() <= BLOCK_REF_TIME,
            "the figure of {call} fits inside one block"
        );
        assert!(
            measured.ref_time() >= plain.ref_time(),
            "the figure of {call} charges the reference time a test runtime charges"
        );
        assert!(
            measured.proof_size() >= plain.proof_size(),
            "the figure of {call} charges the proof size a test runtime charges"
        );
        assert_ne!(
            measured, plain,
            "the figure of {call} carries the storage charge a test runtime lacks"
        );
        assert!(
            plain.ref_time() > 0,
            "the test runtime figure of {call} charges reference time"
        );
    }

    #[test]
    fn every_measured_figure_carries_the_storage_charge_its_call_performs() {
        holds(
            "register_validator",
            <SubstrateWeight<Test> as WeightInfo>::register_validator(),
            <() as WeightInfo>::register_validator(),
        );
        holds(
            "submit_score",
            <SubstrateWeight<Test> as WeightInfo>::submit_score(),
            <() as WeightInfo>::submit_score(),
        );
        holds(
            "slash_validator",
            <SubstrateWeight<Test> as WeightInfo>::slash_validator(),
            <() as WeightInfo>::slash_validator(),
        );
        holds(
            "bond_stake",
            <SubstrateWeight<Test> as WeightInfo>::bond_stake(),
            <() as WeightInfo>::bond_stake(),
        );
        holds(
            "unbond_stake",
            <SubstrateWeight<Test> as WeightInfo>::unbond_stake(),
            <() as WeightInfo>::unbond_stake(),
        );
        holds(
            "boost_score",
            <SubstrateWeight<Test> as WeightInfo>::boost_score(),
            <() as WeightInfo>::boost_score(),
        );
        holds(
            "slash_score",
            <SubstrateWeight<Test> as WeightInfo>::slash_score(),
            <() as WeightInfo>::slash_score(),
        );
    }
}
