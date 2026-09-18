# Test baseline

This document records the observed test baseline for the Cerulea chain
workspace. Every figure below comes from a real run. A failing test stays
failing here. A failing test is information about code that the Dynamic
Consensus Framework work replaces.

## How the run happened

1. **Toolchain.** Rust 1.85, pinned in `rust-toolchain.toml`.
2. **Profile.** Release for every command. The debug profile shares no cache.
3. **Jobs.** Four parallel jobs. `--all-targets` never runs.
4. **Command.** `cargo test --release -p <crate> --no-fail-fast`, one crate at a time.
5. **Logs.** One log per crate under `test-results/baseline/<crate>.log`.

## Totals

661 tests passed. 109 tests failed.

## Per package

| Package | Passed | Failed | Ignored | Compiles |
|---|---|---|---|---|
| `cerulea-blueprint` | 192 | 0 | 0 | yes |
| `pallet-cerulea-pos` | 28 | 2 | 0 | yes |
| `pallet-cerulea-poi` | 38 | 3 | 0 | yes |
| `pallet-cerulea-dvf` | 13 | 19 | 0 | yes |
| `pallet-cerulea-dcf` | 242 | 58 | 0 | yes |
| `cerulea-runtime` | 14 | 27 | 0 | yes |
| `cerulea-consensus` | 131 | 0 | 0 | yes |
| `cerulea-tools` | 3 | 0 | 0 | yes |
| `cerulea-node` | 0 | 0 | 0 | no |

## Failing tests by name

### cerulea-blueprint

Every test passed.

### pallet-cerulea-pos

- `tests::tests::test_get_active_validators`
- `tests::tests::test_minimum_active_validators_constraint`

### pallet-cerulea-poi

- `tests::tests::test_challenge_already_challenged`
- `tests::tests::test_comprehensive_error_handling`
- `tests::tests::test_self_challenge_prevention`

### pallet-cerulea-dvf

- `mock::test_genesis_config_builds`
- `tests::test_genesis_calculates_total_voting_weight`
- `tests::test_genesis_calls_freeze_epoch_weights`
- `tests::test_genesis_five_validators`
- `tests::test_genesis_multiple_validators`
- `tests::test_genesis_ten_validators`
- `tests::test_genesis_weight_initialization_bug_condition`
- `tests::test_preservation_epoch_transition_updates_weights`
- `tests::test_preservation_finalization_after_epoch_boundary`
- `tests::test_preservation_multiple_epoch_transitions`
- `tests::test_preservation_weight_calculation_formula`
- `tests::test_preservation_weight_recalculation_on_changes`
- `tests::test_submit_justification_already_finalized`
- `tests::test_submit_justification_duplicate_validators`
- `tests::test_submit_justification_empty_votes`
- `tests::test_submit_justification_non_checkpoint_block`
- `tests::test_submit_justification_success`
- `tests::test_submit_justification_threshold_not_reached`
- `tests::test_vote_record_pruning`

### pallet-cerulea-dcf

- `private_chain::tests::test_private_mode_proposal_restrictions`
- `private_chain::tests::test_private_mode_validator_restrictions`
- `tests::ci_readiness_tests::ci_comprehensive_invariant_validation`
- `tests::ci_readiness_tests::ci_public_api_coverage`
- `tests::comprehensive_property_tests::prop_validator_set_invariants`
- `tests::consensus_invariant_tests::property_edge_case_handling`
- `tests::consensus_invariant_tests::property_governance_system_integrity`
- `tests::consensus_invariant_tests::property_reward_distribution_bounds`
- `tests::consensus_invariant_tests::property_score_mathematical_bounds`
- `tests::consensus_invariant_tests::property_slashing_economic_security`
- `tests::consensus_invariant_tests::property_system_liveness`
- `tests::cross_pallet_integration_tests::cross_pallet_epoch_transitions`
- `tests::cross_pallet_integration_tests::cross_pallet_error_propagation`
- `tests::cross_pallet_integration_tests::cross_pallet_stress_test`
- `tests::cross_pallet_integration_tests::cross_pallet_validator_rewards`
- `tests::cross_pallet_integration_tests::cross_pallet_validator_slashing`
- `tests::cross_pallet_integration_tests::dcf_pos_integration_validator_lifecycle`
- `tests::cross_pallet_integration_tests::three_way_pallet_integration`
- `tests::deterministic_tests::deterministic_error_handling_works`
- `tests::dos_protection_tests::graceful_degradation_works`
- `tests::end_to_end_workflow_tests::end_to_end_governance_driven_workflow`
- `tests::end_to_end_workflow_tests::end_to_end_high_frequency_operations`
- `tests::end_to_end_workflow_tests::end_to_end_multi_validator_competition`
- `tests::end_to_end_workflow_tests::end_to_end_system_recovery_workflow`
- `tests::end_to_end_workflow_tests::end_to_end_validator_complete_lifecycle`
- `tests::end_to_end_workflow_tests::end_to_end_validator_with_challenges_and_slashing`
- `tests::finality_tests::basic_finality_information_works`
- `tests::genesis_validation_tests::genesis_epoch_configuration_works`
- `tests::genesis_validation_tests::genesis_governance_mode_works`
- `tests::integration_tests::epoch_length_configuration_works`
- `tests::invariant_tests::boundary_condition_invariants_hold`
- `tests::invariant_tests::scoring_invariants_hold`
- `tests::parameter_governance_tests::epoch_configuration_parameters_work`
- `tests::parameter_governance_tests::governance_mode_basic_functionality_works`
- `tests::parameter_governance_tests::validator_score_parameter_bounds_work`
- `tests::property_tests::property_validator_scores_within_bounds`
- `tests::runtime_api_cross_pallet_tests::runtime_api_configuration_consistency`
- `tests::runtime_api_cross_pallet_tests::runtime_api_data_flow_integration`
- `tests::runtime_api_cross_pallet_tests::runtime_api_epoch_synchronization`
- `tests::runtime_api_cross_pallet_tests::runtime_api_error_handling_integration`
- `tests::runtime_api_cross_pallet_tests::runtime_api_finality_integration`
- `tests::runtime_api_cross_pallet_tests::runtime_api_performance_integration`
- `tests::runtime_api_cross_pallet_tests::runtime_api_state_transitions_integration`
- `tests::storage_migration_tests::storage_limits_and_bounds_work`
- `tests::stress_tests::stress_test_concurrent_challenges_and_slashing`
- `tests::stress_tests::stress_test_concurrent_score_updates`
- `tests::stress_tests::stress_test_concurrent_validator_lifecycle`
- `tests::stress_tests::stress_test_governance_under_load`
- `tests::stress_tests::stress_test_maximum_system_load`
- `tests::stress_tests::stress_test_rapid_epoch_transitions`
- `tests::stress_tests::stress_test_storage_limits`
- `tests::system_integration_tests::balance_integration_for_validator_leaving_works`
- `tests::system_integration_tests::multi_validator_system_interaction_works`
- `tests::system_integration_tests::validator_leave_status_change_events_work`
- `tests::system_integration_tests::validator_status_changed_events_work`
- `tests::trust_score_stability_tests::trust_score_anti_gaming_mechanisms_work`
- `tests::trust_score_stability_tests::trust_score_bounds_during_extreme_conditions_work`
- `tests::trust_score_stability_tests::trust_score_growth_limitations_work`

### cerulea-runtime

- `test_genesis_config_builds`
- `tests::runtime_api_tests::test_account_nonce_api`
- `tests::runtime_api_tests::test_api_edge_cases`
- `tests::runtime_api_tests::test_api_error_handling`
- `tests::runtime_api_tests::test_api_performance`
- `tests::runtime_api_tests::test_cross_pallet_api_consistency`
- `tests::runtime_api_tests::test_dcf_api_functions`
- `tests::runtime_api_tests::test_dcf_finality_functions`
- `tests::runtime_api_tests::test_dcf_system_functions`
- `tests::runtime_api_tests::test_dcf_validator_functions`
- `tests::runtime_api_tests::test_epoch_consistency_across_pallets`
- `tests::runtime_api_tests::test_poi_api_functions`
- `tests::runtime_api_tests::test_pos_api_functions`
- `tests::runtime_benchmark_tests::test_benchmark_integration`
- `tests::runtime_integration_tests::test_cross_pallet_interactions`
- `tests::runtime_integration_tests::test_economic_model`
- `tests::runtime_integration_tests::test_epoch_transitions`
- `tests::runtime_integration_tests::test_error_handling`
- `tests::runtime_integration_tests::test_governance_integration`
- `tests::runtime_integration_tests::test_multi_validator_system`
- `tests::runtime_integration_tests::test_performance_under_load`
- `tests::runtime_integration_tests::test_runtime_apis`
- `tests::runtime_integration_tests::test_runtime_initialization`
- `tests::runtime_integration_tests::test_runtime_upgrade_compatibility`
- `tests::runtime_integration_tests::test_system_limits`
- `tests::runtime_mock_tests::test_helper_functions`
- `tests::runtime_mock_tests::test_mock_configurations`

### cerulea-consensus

Every test passed.

### cerulea-tools

Every test passed.

### cerulea-node

The test target holds 3 compile errors. Zero tests ran.

## What this run repaired

1. **The stake pallet test target.** The mock runtime names the balances
   pallet. The development dependency was absent, so 159 errors followed and
   zero tests ran. Task P0-021 added the dependency.
2. **The inference pallet test target.** The mock lacked one accessor, two
   interface methods and thirteen associated types. Task P0-022 supplied every
   one and gave the mock interfaces real state.
3. **The validator registry pallet test target.** The balances development
   dependency carried no standard library feature, and one interval parameter
   carried the wrong width. Both are corrected.
4. **The framework pallet test target.** Stake storage, validator lifecycle and
   inference scoring moved into their own pallets. The mock and 23 test files
   still named the old locations, which produced 322 errors. Every call site now
   names the pallet that holds the item. The mock carries a validator registry
   that keeps real state in the test externalities.
5. **The runtime test target.** The same moved items appeared in the runtime
   mock and tests. Every call site now names the pallet that holds the item.

## The check that stops the defect returning

Each pallet mock ends with a compile-time contract block. The block names
every trait the mock claims to satisfy and forces the compiler to prove each
one. A trait that gains an item stops the test target from compiling at that
block, with the trait named in the error. The block runs on every build of the
test target, so every commit gate covers it.

The pattern reads:

```rust
const _: fn() = || {
    fn settings_hold<T: crate::Config>() {}
    settings_hold::<Test>();

    fn stake_interface_holds<T: crate::PosInterface<u64>>() {}
    stake_interface_holds::<<Test as crate::Config>::PosInterface>();
};
```

## Known compile failure carried forward

The node package holds one test target that names `init_cerulea_logging` in
`cerulea_node::logging`. That function is absent. The node crate belongs to the
consensus rebuild, so this record leaves the target untouched and names the gap
here.
