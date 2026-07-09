#![cfg(feature = "runtime-benchmarks")]

//! Benchmarking file for pallet-cerulea-poi.
//! Provides benchmarks for the main extrinsics: submit_inference and challenge_inference.

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

    /// Benchmark for the `submit_inference` extrinsic.
    #[benchmark]
    fn submit_inference() {
        let caller: T::AccountId = create_validator::<T>(0);
        let result: u32 = 42;
        let confidence = T::MinInferenceConfidence::get() + 10;
        let origin = RawOrigin::Signed(caller.clone());

        #[extrinsic_call]
        submit_inference(origin, result, confidence);

        // Verify that the inference result was stored for the caller.
        assert!(InferenceResults::<T>::contains_key(&caller));
        let (stored_result, _) = InferenceResults::<T>::get(&caller).unwrap();
        assert_eq!(stored_result, result);
    }

    /// Benchmark for the `challenge_inference` extrinsic.
    #[benchmark]
    fn challenge_inference() {
        let caller: T::AccountId = create_validator::<T>(0);
        let challenged: T::AccountId = create_validator::<T>(1);
        let result: u32 = 42;
        let confidence = T::MinInferenceConfidence::get() + 10;

        // Pre-insert an inference result for the challenged account.
        InferenceResults::<T>::insert(&challenged, (result, 0));
        
        let origin = RawOrigin::Signed(caller.clone());

        #[extrinsic_call]
        challenge_inference(origin, challenged.clone(), result);

        // Verify that the challenge was stored for the caller.
        let (challenged_account, stored_result, _) = Challenges::<T>::get(&caller).unwrap();
        assert_eq!(challenged_account, challenged);
        assert_eq!(stored_result, result);
    }

    /// Benchmark for resolving challenges (internal function)
    #[benchmark]
    fn resolve_challenge() {
        let challenger: T::AccountId = create_validator::<T>(0);
        let challenged: T::AccountId = create_validator::<T>(1);
        let result: u32 = 42;

        // Setup challenge
        InferenceResults::<T>::insert(&challenged, (result, 0));
        Challenges::<T>::insert(&challenger, (challenged.clone(), result, 0));

        #[block]
        {
            // Simulate challenge resolution
            let _ = Pallet::<T>::resolve_challenge(&challenger, &challenged, result, true);
        }

        // Verify challenge was resolved
        assert!(!Challenges::<T>::contains_key(&challenger));
    }

    /// Benchmark for epoch cleanup operations
    #[benchmark]
    fn epoch_cleanup() {
        let validator1: T::AccountId = create_validator::<T>(0);
        let validator2: T::AccountId = create_validator::<T>(1);
        
        // Setup old inference results and challenges
        InferenceResults::<T>::insert(&validator1, (42, 0));
        InferenceResults::<T>::insert(&validator2, (43, 0));
        Challenges::<T>::insert(&validator1, (validator2.clone(), 43, 0));

        #[block]
        {
            // Simulate epoch cleanup
            let current_epoch = 10u32;
            let max_age = T::MaxInferenceAge::get();
            
            // Clean up old inferences
            InferenceResults::<T>::iter().for_each(|(validator, (_, epoch))| {
                if current_epoch.saturating_sub(epoch) > max_age {
                    InferenceResults::<T>::remove(&validator);
                }
            });
            
            // Clean up old challenges
            Challenges::<T>::iter().for_each(|(challenger, (_, _, epoch))| {
                if current_epoch.saturating_sub(epoch) > T::ChallengeWindow::get() {
                    Challenges::<T>::remove(&challenger);
                }
            });
        }
    }

    impl_benchmark_test_suite!(
        Pallet,
        crate::mock::new_test_ext(),
        crate::mock::Test
    );
}