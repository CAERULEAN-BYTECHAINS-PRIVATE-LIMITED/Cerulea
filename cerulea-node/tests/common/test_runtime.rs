// Test runtime builder for integration testing
//
// This module provides utilities for creating and configuring test blockchain
// instances with specific validator sets, stakes, and consensus parameters.

#![allow(dead_code)]

use crate::common::config::TestConfig;
use cerulea_runtime::{AccountId, Balance};

/// Builder for creating test runtime instances
/// 
/// Provides a fluent interface for configuring test blockchain instances
/// with specific validator sets, stakes, scores, and consensus parameters.
pub struct TestRuntimeBuilder {
    config: TestConfig,
}

impl TestRuntimeBuilder {
    /// Create a new test runtime builder with default configuration
    pub fn new() -> Self {
        Self {
            config: TestConfig::default(),
        }
    }
    
    /// Set the validator list for the test runtime
    pub fn with_validators(mut self, validators: Vec<AccountId>) -> Self {
        self.config.validators = validators;
        self
    }
    
    /// Set the initial stakes for validators
    pub fn with_stakes(mut self, stakes: Vec<(AccountId, Balance)>) -> Self {
        self.config.stakes = stakes;
        self
    }
    
    /// Set the initial PoS scores for validators
    pub fn with_pos_scores(mut self, scores: Vec<(AccountId, u32)>) -> Self {
        self.config.pos_scores = scores;
        self
    }
    
    /// Set the initial PoI scores for validators
    pub fn with_poi_scores(mut self, scores: Vec<(AccountId, u32)>) -> Self {
        self.config.poi_scores = scores;
        self
    }
    
    /// Set the consensus weights for PoS and PoI
    pub fn with_consensus_weights(mut self, pos_weight: u64, poi_weight: u64) -> Self {
        self.config.consensus_weights = (pos_weight, poi_weight);
        self
    }
    
    /// Set the epoch length in blocks
    pub fn with_epoch_length(mut self, epoch_length: u32) -> Self {
        self.config.epoch_length = epoch_length;
        self
    }
    
    /// Enable or disable CBC RPC extensions
    pub fn with_cbc_extensions(mut self, enabled: bool) -> Self {
        self.config.enable_cbc_extensions = enabled;
        self
    }
    
    /// Build the test client with the configured parameters
    /// 
    /// Note: This is a placeholder implementation. In a full implementation,
    /// this would create an actual Substrate test client with the configured
    /// genesis state and runtime parameters.
    pub fn build(self) -> TestClient {
        TestClient::new(self.config)
    }
}

impl Default for TestRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Test client wrapper for blockchain testing
/// 
/// Provides a simplified interface for interacting with test blockchain
/// instances, including RPC calls and Runtime API access.
pub struct TestClient {
    config: TestConfig,
    current_block: u32,
    current_epoch: u32,
}

impl TestClient {
    /// Create a new test client with the given configuration
    pub fn new(config: TestConfig) -> Self {
        Self {
            config,
            current_block: 1,
            current_epoch: 1,
        }
    }
    
    /// Get the current configuration
    pub fn config(&self) -> &TestConfig {
        &self.config
    }
    
    /// Get the current block number
    pub fn current_block(&self) -> u32 {
        self.current_block
    }
    
    /// Get the current epoch number
    pub fn current_epoch(&self) -> u32 {
        self.current_epoch
    }
    
    /// Advance to the next block
    pub fn advance_block(&mut self) {
        self.current_block += 1;
        
        // Check if we need to advance the epoch
        if self.current_block % self.config.epoch_length == 0 {
            self.current_epoch += 1;
        }
    }
    
    /// Advance to a specific block number
    pub fn advance_to_block(&mut self, block_number: u32) {
        if block_number > self.current_block {
            self.current_block = block_number;
            self.current_epoch = (block_number / self.config.epoch_length) + 1;
        }
    }
    
    /// Get validator stake by account
    pub fn get_validator_stake(&self, validator: &AccountId) -> Option<Balance> {
        self.config.stakes.iter()
            .find(|(account, _)| account == validator)
            .map(|(_, stake)| *stake)
    }
    
    /// Get validator PoS score by account
    pub fn get_validator_pos_score(&self, validator: &AccountId) -> Option<u32> {
        self.config.pos_scores.iter()
            .find(|(account, _)| account == validator)
            .map(|(_, score)| *score)
    }
    
    /// Get validator PoI score by account
    pub fn get_validator_poi_score(&self, validator: &AccountId) -> Option<u32> {
        self.config.poi_scores.iter()
            .find(|(account, _)| account == validator)
            .map(|(_, score)| *score)
    }
    
    /// Calculate trust score for a validator
    pub fn calculate_trust_score(&self, validator: &AccountId) -> Option<u64> {
        let pos_score = self.get_validator_pos_score(validator)? as u64;
        let poi_score = self.get_validator_poi_score(validator)? as u64;
        let (pos_weight, poi_weight) = self.config.consensus_weights;
        
        Some((pos_score * pos_weight + poi_score * poi_weight) / (pos_weight + poi_weight))
    }
    
    /// Check if a validator is active
    pub fn is_validator_active(&self, validator: &AccountId) -> bool {
        self.config.validators.contains(validator)
    }
    
    /// Get all active validators
    pub fn get_active_validators(&self) -> &[AccountId] {
        &self.config.validators
    }
    
    /// Get consensus weights
    pub fn get_consensus_weights(&self) -> (u64, u64) {
        self.config.consensus_weights
    }
    
    /// Check if CBC extensions are enabled
    pub fn cbc_extensions_enabled(&self) -> bool {
        self.config.enable_cbc_extensions
    }
}

/// Helper function to create a test client with validators and stakes
/// 
/// Convenience function for common test scenarios where you need a test client
/// with specific validators and their corresponding stakes.
pub fn create_test_client_with_validators(
    validators: Vec<AccountId>,
    stakes: Vec<(AccountId, Balance)>,
) -> TestClient {
    TestRuntimeBuilder::new()
        .with_validators(validators)
        .with_stakes(stakes)
        .build()
}

/// Helper function to create a test client with full validator configuration
/// 
/// Convenience function for creating a test client with validators, stakes,
/// and both PoS and PoI scores configured.
pub fn create_test_client_with_full_config(
    validators: Vec<AccountId>,
    stakes: Vec<(AccountId, Balance)>,
    pos_scores: Vec<(AccountId, u32)>,
    poi_scores: Vec<(AccountId, u32)>,
) -> TestClient {
    TestRuntimeBuilder::new()
        .with_validators(validators)
        .with_stakes(stakes)
        .with_pos_scores(pos_scores)
        .with_poi_scores(poi_scores)
        .build()
}

/// Test scenario builder for common testing patterns
/// 
/// Provides pre-configured test scenarios for common testing situations.
pub struct TestScenarioBuilder;

impl TestScenarioBuilder {
    /// Create a healthy consensus scenario with 3+ validators
    pub fn healthy_consensus() -> TestClient {
        use sp_core::crypto::AccountId32;
        
        let validators = vec![
            AccountId32::from([1u8; 32]),
            AccountId32::from([2u8; 32]),
            AccountId32::from([3u8; 32]),
            AccountId32::from([4u8; 32]),
        ];
        
        let stakes = validators.iter().enumerate()
            .map(|(i, validator)| (validator.clone(), (i as u128 + 1) * 1000))
            .collect();
        
        let pos_scores = validators.iter().enumerate()
            .map(|(i, validator)| (validator.clone(), 80 + (i as u32 * 5)))
            .collect();
        
        let poi_scores = validators.iter().enumerate()
            .map(|(i, validator)| (validator.clone(), 75 + (i as u32 * 5)))
            .collect();
        
        create_test_client_with_full_config(validators, stakes, pos_scores, poi_scores)
    }
    
    /// Create a degraded consensus scenario with 1-2 validators
    pub fn degraded_consensus() -> TestClient {
        use sp_core::crypto::AccountId32;
        
        let validators = vec![
            AccountId32::from([1u8; 32]),
            AccountId32::from([2u8; 32]),
        ];
        
        let stakes = vec![
            (validators[0].clone(), 1000u128),
            (validators[1].clone(), 2000u128),
        ];
        
        let pos_scores = vec![
            (validators[0].clone(), 85u32),
            (validators[1].clone(), 90u32),
        ];
        
        let poi_scores = vec![
            (validators[0].clone(), 80u32),
            (validators[1].clone(), 85u32),
        ];
        
        create_test_client_with_full_config(validators, stakes, pos_scores, poi_scores)
    }
    
    /// Create a critical consensus scenario with 0 active validators
    pub fn critical_consensus() -> TestClient {
        TestRuntimeBuilder::new()
            .with_validators(vec![]) // No active validators
            .build()
    }
    
    /// Create a scenario with custom consensus weights
    pub fn custom_weights(pos_weight: u64, poi_weight: u64) -> TestClient {
        use sp_core::crypto::AccountId32;
        
        let validators = vec![
            AccountId32::from([1u8; 32]),
            AccountId32::from([2u8; 32]),
            AccountId32::from([3u8; 32]),
        ];
        
        TestRuntimeBuilder::new()
            .with_validators(validators)
            .with_consensus_weights(pos_weight, poi_weight)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::crypto::AccountId32;

    #[test]
    fn test_runtime_builder() {
        let validators = vec![AccountId32::from([1u8; 32])];
        let stakes = vec![(AccountId32::from([1u8; 32]), 1000u128)];
        
        let client = TestRuntimeBuilder::new()
            .with_validators(validators.clone())
            .with_stakes(stakes.clone())
            .with_consensus_weights(70, 30)
            .build();
        
        assert_eq!(client.config().validators, validators);
        assert_eq!(client.config().stakes, stakes);
        assert_eq!(client.config().consensus_weights, (70, 30));
    }

    #[test]
    fn test_client_functionality() {
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
        
        // Test epoch advancement (default epoch length is 100)
        client.advance_to_block(100);
        assert_eq!(client.current_block(), 100);
        assert_eq!(client.current_epoch(), 2);
    }

    #[test]
    fn test_helper_functions() {
        let validators = vec![AccountId32::from([1u8; 32])];
        let stakes = vec![(AccountId32::from([1u8; 32]), 1000u128)];
        
        let client = create_test_client_with_validators(validators.clone(), stakes.clone());
        assert_eq!(client.config().validators, validators);
        assert_eq!(client.config().stakes, stakes);
    }

    #[test]
    fn test_scenario_builder() {
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
}