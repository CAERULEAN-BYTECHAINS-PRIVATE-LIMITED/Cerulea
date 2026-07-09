// Test infrastructure verification
//
// This test file verifies that the test infrastructure foundation is working correctly.

mod common;

use common::*;
use proptest::strategy::{Strategy, ValueTree};
use sp_core::crypto::AccountId32;

#[test]
fn test_infrastructure_config() {
    // Test PropertyTestConfig
    let config = PropertyTestConfig::new()
        .with_iterations(200)
        .with_max_validators(5)
        .with_verbose_logging(true);
    
    assert_eq!(config.iterations, 200);
    assert_eq!(config.max_validators, 5);
    assert!(config.verbose_logging);
}

#[test]
fn test_infrastructure_test_config() {
    let validators = vec![AccountId32::from([1u8; 32])];
    let stakes = vec![(AccountId32::from([1u8; 32]), 1000u128)];
    
    let config = TestConfig::new()
        .with_validators(validators.clone())
        .with_stakes(stakes.clone())
        .with_consensus_weights(70, 30)
        .with_epoch_length(200);
    
    assert_eq!(config.validators, validators);
    assert_eq!(config.stakes, stakes);
    assert_eq!(config.consensus_weights, (70, 30));
    assert_eq!(config.epoch_length, 200);
}

#[test]
fn test_infrastructure_generators() {
    use proptest::test_runner::TestRunner;
    
    let mut runner = TestRunner::default();
    
    // Test validator account strategy
    let account_strategy = validator_account_strategy();
    let account = account_strategy.new_tree(&mut runner).unwrap().current();
    let account_bytes: &[u8] = account.as_ref();
    assert_eq!(account_bytes.len(), 32);
    
    // Test score strategies
    let pos_strategy = pos_score_strategy();
    let pos_score = pos_strategy.new_tree(&mut runner).unwrap().current();
    assert!(pos_score <= 100);
    
    let poi_strategy = poi_score_strategy();
    let poi_score = poi_strategy.new_tree(&mut runner).unwrap().current();
    assert!(poi_score <= 100);
}

#[test]
fn test_infrastructure_test_runtime() {
    let validator = AccountId32::from([1u8; 32]);
    let mut client = TestRuntimeBuilder::new()
        .with_validators(vec![validator.clone()])
        .with_stakes(vec![(validator.clone(), 1000u128)])
        .with_pos_scores(vec![(validator.clone(), 85u32)])
        .with_poi_scores(vec![(validator.clone(), 75u32)])
        .with_consensus_weights(60, 40)
        .build();
    
    assert_eq!(client.current_block(), 1);
    assert_eq!(client.current_epoch(), 1);
    
    // Test stake retrieval
    assert_eq!(client.get_validator_stake(&validator), Some(1000u128));
    
    // Test score retrieval
    assert_eq!(client.get_validator_pos_score(&validator), Some(85u32));
    assert_eq!(client.get_validator_poi_score(&validator), Some(75u32));
    
    // Test trust score calculation
    // (85 * 60 + 75 * 40) / (60 + 40) = (5100 + 3000) / 100 = 81
    assert_eq!(client.calculate_trust_score(&validator), Some(81u64));
    
    // Test validator status
    assert!(client.is_validator_active(&validator));
    
    // Test block advancement
    client.advance_block();
    assert_eq!(client.current_block(), 2);
}

#[test]
fn test_infrastructure_mock_client() {
    let validator = AccountId32::from([1u8; 32]);
    let client = MockClientBuilder::new()
        .with_validator_pos_score(validator.clone(), 85)
        .with_validator_stake(validator.clone(), 1000)
        .with_active_validators(vec![validator.clone()])
        .build();
    
    assert_eq!(
        client.runtime_api().get_validator_pos_score(&validator).unwrap(),
        Some(85)
    );
    assert_eq!(
        client.runtime_api().get_validator_stake(&validator).unwrap(),
        Some(1000)
    );
    assert!(client.cbc_extensions_enabled());
}

#[test]
fn test_infrastructure_error_handling() {
    let expected = ExpectedRpcError::invalid_params("malformed AccountId");
    assert_eq!(expected.code, -32602);
    assert!(expected.message_contains.contains("malformed AccountId"));
    
    let api_error = ExpectedRpcError::runtime_api_failure("call failed");
    assert_eq!(api_error.code, -32000);
    
    let disabled_error = ExpectedRpcError::disabled_extensions();
    assert_eq!(disabled_error.code, -32001);
}

#[test]
fn test_infrastructure_logging() {
    // Test that logging can be initialized without panicking
    init_test_logging();
    
    // Test logger creation
    let logger = TestLogger::new("test_function", false);
    assert_eq!(logger.test_name(), "test_function");
    assert!(!logger.is_verbose());
    
    let verbose_logger = TestLogger::new("test_function", true);
    assert!(verbose_logger.is_verbose());
}

#[test]
fn test_infrastructure_scenario_builder() {
    let healthy = TestScenarioBuilder::healthy_consensus();
    assert!(healthy.get_active_validators().len() >= 3);
    
    let degraded = TestScenarioBuilder::degraded_consensus();
    assert!(degraded.get_active_validators().len() >= 1);
    assert!(degraded.get_active_validators().len() <= 2);
    
    let critical = TestScenarioBuilder::critical_consensus();
    assert_eq!(critical.get_active_validators().len(), 0);
    
    let custom = TestScenarioBuilder::custom_weights(80, 20);
    assert_eq!(custom.get_consensus_weights(), (80, 20));
}

#[test]
fn test_infrastructure_mock_helpers() {
    let healthy = create_healthy_consensus_mock();
    assert!(healthy.runtime_api().get_active_validators().unwrap().len() >= 3);
    
    let degraded = create_degraded_consensus_mock();
    let active_validators = degraded.runtime_api().get_active_validators().unwrap();
    assert!(active_validators.len() >= 1 && active_validators.len() <= 2);
    
    let critical = create_critical_consensus_mock();
    assert_eq!(critical.runtime_api().get_active_validators().unwrap().len(), 0);
    
    let failing = create_failing_mock();
    assert!(failing.runtime_api().get_current_epoch().is_err());
    
    let disabled = create_disabled_extensions_mock();
    assert!(!disabled.cbc_extensions_enabled());
}