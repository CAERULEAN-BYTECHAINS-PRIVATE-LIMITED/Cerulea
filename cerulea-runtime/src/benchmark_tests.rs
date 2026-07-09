//! Runtime benchmarking tests for the CBC Runtime
//! 
//! This module contains tests that verify the benchmarking infrastructure
//! works correctly and provides performance validation for the runtime.

#[cfg(feature = "runtime-benchmarks")]
use crate::mock::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmark_integration_tests {
    use super::*;
    use frame_benchmarking::{account, whitelisted_caller};
    use frame_support::assert_ok;

    #[test]
    fn test_dcf_benchmarks() {
        new_test_ext().execute_with(|| {
            // Test that DCF benchmarks can be executed
            let validator: u64 = whitelisted_caller();
            create_funded_account(validator, 100000);
            
            // These calls should not panic when benchmarking is enabled
            let _ = DcfPallet::join_validators(RuntimeOrigin::signed(validator), None);
            let _ = DcfPallet::leave_validators(RuntimeOrigin::signed(validator));
        });
    }

    #[test]
    fn test_pos_benchmarks() {
        new_test_ext().execute_with(|| {
            // Test that PoS benchmarks can be executed
            let validator: u64 = whitelisted_caller();
            
            // These calls should not panic when benchmarking is enabled
            let _ = PalletCbcPos::register_validator(RuntimeOrigin::signed(validator));
            let _ = PalletCbcPos::submit_score(RuntimeOrigin::signed(validator), validator, 75);
        });
    }

    #[test]
    fn test_poi_benchmarks() {
        new_test_ext().execute_with(|| {
            // Test that PoI benchmarks can be executed
            let validator: u64 = whitelisted_caller();
            
            // These calls should not panic when benchmarking is enabled
            let _ = PalletCbcPoi::submit_inference(RuntimeOrigin::signed(validator), 42, 80);
        });
    }

    #[test]
    fn test_benchmark_performance() {
        new_test_ext_with_validators(50).execute_with(|| {
            use std::time::Instant;
            
            // Test performance of benchmark operations
            let validators = DcfPallet::validator_set();
            let start = Instant::now();
            
            // Perform multiple operations
            for validator in validators.iter().take(10) {
                let _ = DcfPallet::update_validator_stake_score(
                    RuntimeOrigin::signed(*validator),
                    *validator
                );
            }
            
            let duration = start.elapsed();
            
            // Should complete quickly
            assert!(duration.as_millis() < 1000);
        });
    }

    #[test]
    fn test_weight_calculations() {
        new_test_ext().execute_with(|| {
            // Test that weight calculations are reasonable
            use crate::configs::weights::*;
            
            // Verify weights are non-zero
            assert!(CBC_DCF_WEIGHT.ref_time() > 0);
            assert!(CBC_POS_WEIGHT.ref_time() > 0);
            assert!(CBC_POI_WEIGHT.ref_time() > 0);
            
            // Verify DCF operations have appropriate weights
            assert!(DCF_JOIN_VALIDATORS_WEIGHT.ref_time() > DCF_LEAVE_VALIDATORS_WEIGHT.ref_time());
            assert!(DCF_EPOCH_TRANSITION_WEIGHT.ref_time() > DCF_JOIN_VALIDATORS_WEIGHT.ref_time());
            
            // Verify PoI operations are heavier than PoS operations
            assert!(POI_SUBMIT_INFERENCE_WEIGHT.ref_time() > POS_SUBMIT_SCORE_WEIGHT.ref_time());
        });
    }
}

#[cfg(not(feature = "runtime-benchmarks"))]
mod benchmark_disabled_tests {
    use super::*;

    #[test]
    fn test_benchmarks_disabled() {
        // When benchmarking is disabled, we should still be able to run basic operations
        new_test_ext().execute_with(|| {
            let validator = 1u64;
            
            // Basic operations should work
            assert_ok!(DcfPallet::join_validators(RuntimeOrigin::signed(validator), None));
            assert_ok!(PalletCbcPos::register_validator(RuntimeOrigin::signed(validator)));
            assert_ok!(PalletCbcPoi::submit_inference(RuntimeOrigin::signed(validator), 42, 80));
        });
    }
}