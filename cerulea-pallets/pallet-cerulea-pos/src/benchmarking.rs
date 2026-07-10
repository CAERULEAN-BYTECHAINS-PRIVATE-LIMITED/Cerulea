#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_benchmarking::account;
use frame_system::RawOrigin;
use sp_std::prelude::*;

/// Helper function to create a validator account
fn create_validator<T: Config>(id: u32) -> T::AccountId {
    account("validator", id, 0)
}

#[benchmarks]
mod benchmarks {
    use super::*;

    /// Benchmark register_validator call
    #[benchmark]
    fn register_validator() {
        let caller: T::AccountId = create_validator::<T>(0);
        let origin = RawOrigin::Signed(caller.clone());

        #[extrinsic_call]
        register_validator(origin);

        assert!(Validators::<T>::contains_key(&caller));
    }

    /// Benchmark submit_score call
    #[benchmark]
    fn submit_score() {
        let caller: T::AccountId = create_validator::<T>(0);
        let validator: T::AccountId = create_validator::<T>(1);
        
        // Register the validator first
        Validators::<T>::insert(&validator, true);
        
        let score = T::MinValidatorScore::get() + 10;
        let origin = RawOrigin::Signed(caller);

        #[extrinsic_call]
        submit_score(origin, validator.clone(), score);

        assert_eq!(ValidatorScores::<T>::get(&validator), Some(score));
    }

    /// Benchmark slash_validator call
    #[benchmark]
    fn slash_validator() {
        let caller: T::AccountId = create_validator::<T>(0);
        let validator: T::AccountId = create_validator::<T>(1);
        
        // Register the validator first
        Validators::<T>::insert(&validator, true);
        ValidatorScores::<T>::insert(&validator, 100);
        
        // Set initial slashing count
        let initial_count = T::MaxSlashingCount::get() - 1;
        SlashingCount::<T>::insert(&validator, initial_count);
        
        let origin = RawOrigin::Signed(caller);

        #[extrinsic_call]
        slash_validator(origin, validator.clone());

        // Verify validator was removed after reaching max slashing count
        assert!(!Validators::<T>::contains_key(&validator));
        assert_eq!(ValidatorScores::<T>::get(&validator), None);
        assert_eq!(SlashingCount::<T>::get(&validator), None);
    }

    /// Benchmark boost_score call
    #[benchmark]
    fn boost_score() {
        let validator: T::AccountId = create_validator::<T>(0);
        
        // Register the validator first
        Validators::<T>::insert(&validator, true);
        ValidatorScores::<T>::insert(&validator, 50);
        
        let weight = 10u32;

        #[block]
        {
            let _ = Pallet::<T>::boost_score(&validator, weight);
        }

        // Verify score was boosted
        let new_score = ValidatorScores::<T>::get(&validator).unwrap_or(0);
        assert!(new_score >= 50);
    }

    /// Benchmark slash_score call
    #[benchmark]
    fn slash_score() {
        let validator: T::AccountId = create_validator::<T>(0);
        
        // Register the validator first
        Validators::<T>::insert(&validator, true);
        ValidatorScores::<T>::insert(&validator, 100);
        
        let weight = 10u32;

        #[block]
        {
            let _ = Pallet::<T>::slash_score(&validator, weight);
        }

        // Verify score was slashed
        let new_score = ValidatorScores::<T>::get(&validator).unwrap_or(0);
        assert!(new_score <= 100);
    }

    /// Benchmark bond_stake call
    #[benchmark]
    fn bond_stake() {
        let caller: T::AccountId = create_validator::<T>(0);
        let amount = T::MinStake::get();
        let origin = RawOrigin::Signed(caller.clone());

        #[extrinsic_call]
        bond_stake(origin, amount);

        // Verify stake was bonded
        assert_eq!(ValidatorStakes::<T>::get(&caller), Some(amount));
    }

    /// Benchmark unbond_stake call
    #[benchmark]
    fn unbond_stake() {
        let caller: T::AccountId = create_validator::<T>(0);
        let amount = T::MinStake::get();
        
        // First bond some stake
        ValidatorStakes::<T>::insert(&caller, amount * 2u32.into());
        
        let origin = RawOrigin::Signed(caller.clone());

        #[extrinsic_call]
        unbond_stake(origin, amount);

        // Verify stake was unbonded
        let remaining_stake = ValidatorStakes::<T>::get(&caller).unwrap_or(0u64.into());
        assert_eq!(remaining_stake, amount);
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
} 