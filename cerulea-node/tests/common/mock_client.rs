// Mock client builder for unit testing
//
// This module provides mock implementations of blockchain clients and Runtime APIs
// for isolated unit testing of RPC handlers without requiring a full blockchain node.

#![allow(dead_code)]

use crate::common::config::RpcSecurityConfig;
use cerulea_runtime::{AccountId, Balance};
use std::collections::HashMap;
use std::sync::Arc;

/// Mock Runtime API implementation for testing
/// 
/// Provides configurable mock responses for all Runtime API methods,
/// allowing unit tests to control the behavior of the underlying blockchain state.
#[derive(Debug, Clone)]
pub struct MockRuntimeApi {
    /// Validator PoS scores
    pub validator_pos_scores: HashMap<AccountId, u32>,
    /// Validator stakes
    pub validator_stakes: HashMap<AccountId, Balance>,
    /// Validator slashing counts
    pub slashing_counts: HashMap<AccountId, u32>,
    /// List of active validators
    pub active_validators: Vec<AccountId>,
    /// PoI inference results (result, confidence)
    pub inference_results: HashMap<AccountId, (u32, u32)>,
    /// Current epoch number
    pub current_epoch: u32,
    /// Consensus weights (pos_weight, poi_weight)
    pub consensus_weights: (u64, u64),
    /// Whether Runtime API calls should fail
    pub should_fail: bool,
    /// Expected author for blocks
    pub expected_authors: HashMap<u32, Option<AccountId>>,
    /// Current block number
    pub current_block: u32,
}

impl MockRuntimeApi {
    /// Create a new mock Runtime API with default values
    pub fn new() -> Self {
        Self {
            validator_pos_scores: HashMap::new(),
            validator_stakes: HashMap::new(),
            slashing_counts: HashMap::new(),
            active_validators: Vec::new(),
            inference_results: HashMap::new(),
            current_epoch: 1,
            consensus_weights: (60, 40),
            should_fail: false,
            expected_authors: HashMap::new(),
            current_block: 1,
        }
    }
    
    /// Set a validator's PoS score
    pub fn with_validator_pos_score(mut self, validator: AccountId, score: u32) -> Self {
        self.validator_pos_scores.insert(validator, score);
        self
    }
    
    /// Set a validator's stake
    pub fn with_validator_stake(mut self, validator: AccountId, stake: Balance) -> Self {
        self.validator_stakes.insert(validator, stake);
        self
    }
    
    /// Set a validator's slashing count
    pub fn with_slashing_count(mut self, validator: AccountId, count: u32) -> Self {
        self.slashing_counts.insert(validator, count);
        self
    }
    
    /// Set the list of active validators
    pub fn with_active_validators(mut self, validators: Vec<AccountId>) -> Self {
        self.active_validators = validators;
        self
    }
    
    /// Set an inference result for a validator
    pub fn with_inference_result(mut self, validator: AccountId, result: u32, confidence: u32) -> Self {
        self.inference_results.insert(validator, (result, confidence));
        self
    }
    
    /// Set the current epoch
    pub fn with_current_epoch(mut self, epoch: u32) -> Self {
        self.current_epoch = epoch;
        self
    }
    
    /// Set consensus weights
    pub fn with_consensus_weights(mut self, pos_weight: u64, poi_weight: u64) -> Self {
        self.consensus_weights = (pos_weight, poi_weight);
        self
    }
    
    /// Configure Runtime API calls to fail
    pub fn should_fail(mut self) -> Self {
        self.should_fail = true;
        self
    }
    
    /// Set expected author for a specific block
    pub fn with_expected_author(mut self, block_number: u32, author: Option<AccountId>) -> Self {
        self.expected_authors.insert(block_number, author);
        self
    }
    
    /// Set current block number
    pub fn with_current_block(mut self, block_number: u32) -> Self {
        self.current_block = block_number;
        self
    }
    
    // Mock Runtime API method implementations
    
    /// Mock get_validator_pos_score
    pub fn get_validator_pos_score(&self, validator: &AccountId) -> Result<Option<u32>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.validator_pos_scores.get(validator).copied())
    }
    
    /// Mock get_validator_stake
    pub fn get_validator_stake(&self, validator: &AccountId) -> Result<Option<Balance>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.validator_stakes.get(validator).copied())
    }
    
    /// Mock get_slashing_count
    pub fn get_slashing_count(&self, validator: &AccountId) -> Result<u32, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.slashing_counts.get(validator).copied().unwrap_or(0))
    }
    
    /// Mock get_active_validators
    pub fn get_active_validators(&self) -> Result<Vec<AccountId>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.active_validators.clone())
    }
    
    /// Mock get_inference_result
    pub fn get_inference_result(&self, validator: &AccountId) -> Result<Option<(u32, u32)>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.inference_results.get(validator).copied())
    }
    
    /// Mock get_current_epoch
    pub fn get_current_epoch(&self) -> Result<u32, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.current_epoch)
    }
    
    /// Mock get_consensus_weights
    pub fn get_consensus_weights(&self) -> Result<(u64, u64), String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.consensus_weights)
    }
    
    /// Mock get_expected_author
    pub fn get_expected_author(&self, block_number: u32) -> Result<Option<AccountId>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.expected_authors.get(&block_number).cloned().flatten())
    }
    
    /// Mock get_current_block
    pub fn get_current_block(&self) -> Result<u32, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        Ok(self.current_block)
    }
    
    /// Calculate trust score using the same logic as the real implementation
    pub fn calculate_trust_score(&self, validator: &AccountId) -> Result<Option<u64>, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        
        let pos_score = match self.validator_pos_scores.get(validator) {
            Some(score) => *score as u64,
            None => return Ok(None),
        };
        
        let poi_score = match self.inference_results.get(validator) {
            Some((_, confidence)) => *confidence as u64,
            None => return Ok(None),
        };
        
        let (pos_weight, poi_weight) = self.consensus_weights;
        let trust_score = (pos_score * pos_weight + poi_score * poi_weight) / (pos_weight + poi_weight);
        
        Ok(Some(trust_score))
    }
    
    /// Determine validator status based on active list and slashing count
    pub fn get_validator_status(&self, validator: &AccountId) -> Result<String, String> {
        if self.should_fail {
            return Err("Runtime API call failed".to_string());
        }
        
        let is_active = self.active_validators.contains(validator);
        let slashing_count = self.slashing_counts.get(validator).copied().unwrap_or(0);
        
        let status = if !is_active {
            "Inactive"
        } else if slashing_count > 0 {
            "Slashed"
        } else {
            "Active"
        };
        
        Ok(status.to_string())
    }
}

impl Default for MockRuntimeApi {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock client implementation for testing
/// 
/// Provides a mock blockchain client that uses the MockRuntimeApi for
/// all Runtime API calls, allowing complete control over responses in unit tests.
pub struct MockClient {
    /// Mock Runtime API instance
    pub runtime_api: MockRuntimeApi,
    /// RPC security configuration
    pub security_config: RpcSecurityConfig,
}

impl MockClient {
    /// Create a new mock client with the given Runtime API
    pub fn new(runtime_api: MockRuntimeApi) -> Arc<Self> {
        Arc::new(Self {
            runtime_api,
            security_config: RpcSecurityConfig::default(),
        })
    }
    
    /// Create a new mock client with custom security configuration
    pub fn with_security_config(
        runtime_api: MockRuntimeApi,
        security_config: RpcSecurityConfig,
    ) -> Arc<Self> {
        Arc::new(Self {
            runtime_api,
            security_config,
        })
    }
    
    /// Get the Runtime API instance
    pub fn runtime_api(&self) -> &MockRuntimeApi {
        &self.runtime_api
    }
    
    /// Get the security configuration
    pub fn security_config(&self) -> &RpcSecurityConfig {
        &self.security_config
    }
    
    /// Check if CBC extensions are enabled
    pub fn cbc_extensions_enabled(&self) -> bool {
        self.security_config.enable_cbc_extensions
    }
}

/// Builder for creating mock clients with specific configurations
/// 
/// Provides a fluent interface for building mock clients with various
/// Runtime API responses and security configurations.
pub struct MockClientBuilder {
    runtime_api: MockRuntimeApi,
    security_config: RpcSecurityConfig,
}

impl MockClientBuilder {
    /// Create a new mock client builder
    pub fn new() -> Self {
        Self {
            runtime_api: MockRuntimeApi::new(),
            security_config: RpcSecurityConfig::default(),
        }
    }
    
    /// Add a validator with PoS score
    pub fn with_validator_pos_score(mut self, validator: AccountId, score: u32) -> Self {
        self.runtime_api = self.runtime_api.with_validator_pos_score(validator, score);
        self
    }
    
    /// Add a validator with stake
    pub fn with_validator_stake(mut self, validator: AccountId, stake: Balance) -> Self {
        self.runtime_api = self.runtime_api.with_validator_stake(validator, stake);
        self
    }
    
    /// Add a validator with slashing count
    pub fn with_slashing_count(mut self, validator: AccountId, count: u32) -> Self {
        self.runtime_api = self.runtime_api.with_slashing_count(validator, count);
        self
    }
    
    /// Set active validators
    pub fn with_active_validators(mut self, validators: Vec<AccountId>) -> Self {
        self.runtime_api = self.runtime_api.with_active_validators(validators);
        self
    }
    
    /// Add inference result for a validator
    pub fn with_inference_result(mut self, validator: AccountId, result: u32, confidence: u32) -> Self {
        self.runtime_api = self.runtime_api.with_inference_result(validator, result, confidence);
        self
    }
    
    /// Set current epoch
    pub fn with_current_epoch(mut self, epoch: u32) -> Self {
        self.runtime_api = self.runtime_api.with_current_epoch(epoch);
        self
    }
    
    /// Set consensus weights
    pub fn with_consensus_weights(mut self, pos_weight: u64, poi_weight: u64) -> Self {
        self.runtime_api = self.runtime_api.with_consensus_weights(pos_weight, poi_weight);
        self
    }
    
    /// Configure Runtime API to fail
    pub fn should_fail(mut self) -> Self {
        self.runtime_api = self.runtime_api.should_fail();
        self
    }
    
    /// Set expected author for a block
    pub fn with_expected_author(mut self, block_number: u32, author: Option<AccountId>) -> Self {
        self.runtime_api = self.runtime_api.with_expected_author(block_number, author);
        self
    }
    
    /// Disable CBC extensions
    pub fn with_disabled_extensions(mut self) -> Self {
        self.security_config = RpcSecurityConfig::disabled_extensions();
        self
    }
    
    /// Enable unsafe methods
    pub fn with_unsafe_methods(mut self) -> Self {
        self.security_config = RpcSecurityConfig::with_unsafe_methods();
        self
    }
    
    /// Build the mock client
    pub fn build(self) -> Arc<MockClient> {
        MockClient::with_security_config(self.runtime_api, self.security_config)
    }
}

impl Default for MockClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper functions for creating common mock client configurations

/// Create a mock client with healthy consensus (3+ validators)
pub fn create_healthy_consensus_mock() -> Arc<MockClient> {
    use sp_core::crypto::AccountId32;
    
    let validators = vec![
        AccountId32::from([1u8; 32]),
        AccountId32::from([2u8; 32]),
        AccountId32::from([3u8; 32]),
        AccountId32::from([4u8; 32]),
    ];
    
    let mut builder = MockClientBuilder::new()
        .with_active_validators(validators.clone())
        .with_current_epoch(1);
    
    // Add stakes and scores for each validator
    for (i, validator) in validators.iter().enumerate() {
        builder = builder
            .with_validator_stake(validator.clone(), (i as u128 + 1) * 1000)
            .with_validator_pos_score(validator.clone(), 80 + (i as u32 * 5))
            .with_inference_result(validator.clone(), 90, 75 + (i as u32 * 5));
    }
    
    builder.build()
}

/// Create a mock client with degraded consensus (1-2 validators)
pub fn create_degraded_consensus_mock() -> Arc<MockClient> {
    use sp_core::crypto::AccountId32;
    
    let validators = vec![
        AccountId32::from([1u8; 32]),
        AccountId32::from([2u8; 32]),
    ];
    
    MockClientBuilder::new()
        .with_active_validators(validators.clone())
        .with_validator_stake(validators[0].clone(), 1000)
        .with_validator_stake(validators[1].clone(), 2000)
        .with_validator_pos_score(validators[0].clone(), 85)
        .with_validator_pos_score(validators[1].clone(), 90)
        .with_inference_result(validators[0].clone(), 90, 80)
        .with_inference_result(validators[1].clone(), 95, 85)
        .build()
}

/// Create a mock client with critical consensus (0 validators)
pub fn create_critical_consensus_mock() -> Arc<MockClient> {
    MockClientBuilder::new()
        .with_active_validators(vec![]) // No active validators
        .build()
}

/// Create a mock client that fails all Runtime API calls
pub fn create_failing_mock() -> Arc<MockClient> {
    MockClientBuilder::new()
        .should_fail()
        .build()
}

/// Create a mock client with disabled CBC extensions
pub fn create_disabled_extensions_mock() -> Arc<MockClient> {
    MockClientBuilder::new()
        .with_disabled_extensions()
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::crypto::AccountId32;

    #[test]
    fn test_mock_runtime_api() {
        let validator = AccountId32::from([1u8; 32]);
        let api = MockRuntimeApi::new()
            .with_validator_pos_score(validator.clone(), 85)
            .with_validator_stake(validator.clone(), 1000)
            .with_active_validators(vec![validator.clone()]);
        
        assert_eq!(api.get_validator_pos_score(&validator).unwrap(), Some(85));
        assert_eq!(api.get_validator_stake(&validator).unwrap(), Some(1000));
        assert!(api.get_active_validators().unwrap().contains(&validator));
    }

    #[test]
    fn test_mock_runtime_api_failure() {
        let validator = AccountId32::from([1u8; 32]);
        let api = MockRuntimeApi::new().should_fail();
        
        assert!(api.get_validator_pos_score(&validator).is_err());
        assert!(api.get_validator_stake(&validator).is_err());
        assert!(api.get_active_validators().is_err());
    }

    #[test]
    fn test_trust_score_calculation() {
        let validator = AccountId32::from([1u8; 32]);
        let api = MockRuntimeApi::new()
            .with_validator_pos_score(validator.clone(), 85)
            .with_inference_result(validator.clone(), 90, 75)
            .with_consensus_weights(60, 40);
        
        // Expected: (85 * 60 + 75 * 40) / (60 + 40) = (5100 + 3000) / 100 = 81
        assert_eq!(api.calculate_trust_score(&validator).unwrap(), Some(81));
    }

    #[test]
    fn test_validator_status() {
        let validator = AccountId32::from([1u8; 32]);
        
        // Test active validator
        let api_active = MockRuntimeApi::new()
            .with_active_validators(vec![validator.clone()]);
        assert_eq!(api_active.get_validator_status(&validator).unwrap(), "Active");
        
        // Test inactive validator
        let api_inactive = MockRuntimeApi::new();
        assert_eq!(api_inactive.get_validator_status(&validator).unwrap(), "Inactive");
        
        // Test slashed validator
        let api_slashed = MockRuntimeApi::new()
            .with_active_validators(vec![validator.clone()])
            .with_slashing_count(validator.clone(), 1);
        assert_eq!(api_slashed.get_validator_status(&validator).unwrap(), "Slashed");
    }

    #[test]
    fn test_mock_client_builder() {
        let validator = AccountId32::from([1u8; 32]);
        let client = MockClientBuilder::new()
            .with_validator_pos_score(validator.clone(), 85)
            .with_active_validators(vec![validator.clone()])
            .build();
        
        assert_eq!(
            client.runtime_api().get_validator_pos_score(&validator).unwrap(),
            Some(85)
        );
        assert!(client.cbc_extensions_enabled());
    }

    #[test]
    fn test_disabled_extensions() {
        let client = MockClientBuilder::new()
            .with_disabled_extensions()
            .build();
        
        assert!(!client.cbc_extensions_enabled());
    }

    #[test]
    fn test_helper_functions() {
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
}