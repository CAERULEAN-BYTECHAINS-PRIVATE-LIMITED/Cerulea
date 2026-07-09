// Test configuration types and builders
//
// This module provides configuration types and builders for setting up
// test environments with specific parameters and constraints.

#![allow(dead_code)]

use cerulea_runtime::{AccountId, Balance};

/// Configuration for property-based tests
/// 
/// Controls the behavior and limits of property-based test execution.
#[derive(Clone, Debug)]
pub struct PropertyTestConfig {
    /// Number of iterations to run for each property test
    pub iterations: u32,
    /// Maximum number of validators to generate in test scenarios
    pub max_validators: usize,
    /// Maximum block number to generate for testing
    pub max_block_number: u32,
    /// Maximum score value to generate for PoS/PoI testing
    pub max_score: u32,
    /// Whether to enable verbose logging during property tests
    pub verbose_logging: bool,
}

impl Default for PropertyTestConfig {
    fn default() -> Self {
        Self {
            iterations: 100,
            max_validators: 10,
            max_block_number: 1_000_000,
            max_score: 100,
            verbose_logging: false,
        }
    }
}

impl PropertyTestConfig {
    /// Create a new property test configuration with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the number of iterations for property tests
    pub fn with_iterations(mut self, iterations: u32) -> Self {
        self.iterations = iterations;
        self
    }
    
    /// Set the maximum number of validators for test scenarios
    pub fn with_max_validators(mut self, max_validators: usize) -> Self {
        self.max_validators = max_validators;
        self
    }
    
    /// Set the maximum block number for testing
    pub fn with_max_block_number(mut self, max_block_number: u32) -> Self {
        self.max_block_number = max_block_number;
        self
    }
    
    /// Set the maximum score value for testing
    pub fn with_max_score(mut self, max_score: u32) -> Self {
        self.max_score = max_score;
        self
    }
    
    /// Enable or disable verbose logging
    pub fn with_verbose_logging(mut self, verbose: bool) -> Self {
        self.verbose_logging = verbose;
        self
    }
}

/// Configuration for test runtime setup
/// 
/// Defines the initial state and parameters for test blockchain instances.
#[derive(Clone, Debug)]
pub struct TestConfig {
    /// List of validator accounts to include in the test runtime
    pub validators: Vec<AccountId>,
    /// Initial stake amounts for validators
    pub stakes: Vec<(AccountId, Balance)>,
    /// Initial PoS scores for validators
    pub pos_scores: Vec<(AccountId, u32)>,
    /// Initial PoI scores for validators
    pub poi_scores: Vec<(AccountId, u32)>,
    /// Consensus weights (pos_weight, poi_weight)
    pub consensus_weights: (u64, u64),
    /// Length of each epoch in blocks
    pub epoch_length: u32,
    /// Whether CBC RPC extensions are enabled
    pub enable_cbc_extensions: bool,
    /// Whether to expose unsafe RPC methods
    pub expose_unsafe_methods: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            validators: Vec::new(),
            stakes: Vec::new(),
            pos_scores: Vec::new(),
            poi_scores: Vec::new(),
            consensus_weights: (60, 40), // 60% PoS, 40% PoI
            epoch_length: 100,
            enable_cbc_extensions: true,
            expose_unsafe_methods: false,
        }
    }
}

impl TestConfig {
    /// Create a new test configuration with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Set the validator list for the test runtime
    pub fn with_validators(mut self, validators: Vec<AccountId>) -> Self {
        self.validators = validators;
        self
    }
    
    /// Set the initial stakes for validators
    pub fn with_stakes(mut self, stakes: Vec<(AccountId, Balance)>) -> Self {
        self.stakes = stakes;
        self
    }
    
    /// Set the initial PoS scores for validators
    pub fn with_pos_scores(mut self, scores: Vec<(AccountId, u32)>) -> Self {
        self.pos_scores = scores;
        self
    }
    
    /// Set the initial PoI scores for validators
    pub fn with_poi_scores(mut self, scores: Vec<(AccountId, u32)>) -> Self {
        self.poi_scores = scores;
        self
    }
    
    /// Set the consensus weights for PoS and PoI
    pub fn with_consensus_weights(mut self, pos_weight: u64, poi_weight: u64) -> Self {
        self.consensus_weights = (pos_weight, poi_weight);
        self
    }
    
    /// Set the epoch length in blocks
    pub fn with_epoch_length(mut self, epoch_length: u32) -> Self {
        self.epoch_length = epoch_length;
        self
    }
    
    /// Enable or disable CBC RPC extensions
    pub fn with_cbc_extensions(mut self, enabled: bool) -> Self {
        self.enable_cbc_extensions = enabled;
        self
    }
    
    /// Enable or disable unsafe RPC methods
    pub fn with_unsafe_methods(mut self, enabled: bool) -> Self {
        self.expose_unsafe_methods = enabled;
        self
    }
}

/// RPC security configuration for testing
/// 
/// Controls security settings and access controls for RPC endpoints during testing.
#[derive(Clone, Debug)]
pub struct RpcSecurityConfig {
    /// Whether CBC RPC extensions are enabled
    pub enable_cbc_extensions: bool,
    /// Whether to expose unsafe RPC methods
    pub expose_unsafe_methods: bool,
    /// Rate limiting window in seconds
    pub rate_limit_window: u64,
    /// Maximum requests per rate limit window
    pub rate_limit_requests: u32,
}

impl Default for RpcSecurityConfig {
    fn default() -> Self {
        Self {
            enable_cbc_extensions: true,
            expose_unsafe_methods: false,
            rate_limit_window: 60,
            rate_limit_requests: 100,
        }
    }
}

impl RpcSecurityConfig {
    /// Create a new RPC security configuration with default values
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Create a configuration with CBC extensions disabled
    pub fn disabled_extensions() -> Self {
        Self {
            enable_cbc_extensions: false,
            ..Self::default()
        }
    }
    
    /// Create a configuration with unsafe methods enabled (for testing only)
    pub fn with_unsafe_methods() -> Self {
        Self {
            expose_unsafe_methods: true,
            ..Self::default()
        }
    }
    
    /// Set rate limiting parameters
    pub fn with_rate_limit(mut self, window_seconds: u64, max_requests: u32) -> Self {
        self.rate_limit_window = window_seconds;
        self.rate_limit_requests = max_requests;
        self
    }
}

/// Test result types for consistency checking
/// 
/// Used to capture and compare results from RPC endpoints and Runtime APIs.
#[derive(Debug, PartialEq)]
pub struct ConsistencyTestResult {
    /// Result from RPC endpoint call
    pub rpc_result: serde_json::Value,
    /// Result from Runtime API call
    pub runtime_api_result: serde_json::Value,
    /// Whether the results are consistent
    pub is_consistent: bool,
}

impl ConsistencyTestResult {
    /// Create a new consistency test result
    pub fn new(
        rpc_result: serde_json::Value,
        runtime_api_result: serde_json::Value,
    ) -> Self {
        let is_consistent = rpc_result == runtime_api_result;
        Self {
            rpc_result,
            runtime_api_result,
            is_consistent,
        }
    }
    
    /// Check if the results are consistent
    pub fn is_consistent(&self) -> bool {
        self.is_consistent
    }
}

/// Property test result tracking
/// 
/// Tracks the execution and results of property-based tests.
#[derive(Debug)]
pub struct PropertyTestResult {
    /// Name of the property being tested
    pub property_name: String,
    /// Number of iterations that were run
    pub iterations_run: u32,
    /// List of failure messages, if any
    pub failures: Vec<String>,
    /// Whether the property test succeeded overall
    pub success: bool,
}

impl PropertyTestResult {
    /// Create a new property test result
    pub fn new(property_name: String) -> Self {
        Self {
            property_name,
            iterations_run: 0,
            failures: Vec::new(),
            success: false,
        }
    }
    
    /// Mark the test as successful with the given number of iterations
    pub fn success(mut self, iterations: u32) -> Self {
        self.iterations_run = iterations;
        self.success = true;
        self
    }
    
    /// Mark the test as failed with the given failures
    pub fn failure(mut self, iterations: u32, failures: Vec<String>) -> Self {
        self.iterations_run = iterations;
        self.failures = failures;
        self.success = false;
        self
    }
    
    /// Add a failure message
    pub fn add_failure(&mut self, failure: String) {
        self.failures.push(failure);
        self.success = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sp_core::crypto::AccountId32;

    #[test]
    fn test_property_test_config_builder() {
        let config = PropertyTestConfig::new()
            .with_iterations(200)
            .with_max_validators(5)
            .with_verbose_logging(true);
        
        assert_eq!(config.iterations, 200);
        assert_eq!(config.max_validators, 5);
        assert!(config.verbose_logging);
    }

    #[test]
    fn test_test_config_builder() {
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
    fn test_rpc_security_config() {
        let config = RpcSecurityConfig::disabled_extensions();
        assert!(!config.enable_cbc_extensions);
        
        let unsafe_config = RpcSecurityConfig::with_unsafe_methods();
        assert!(unsafe_config.expose_unsafe_methods);
    }

    #[test]
    fn test_consistency_test_result() {
        let rpc_result = serde_json::json!({"value": 42});
        let api_result = serde_json::json!({"value": 42});
        
        let result = ConsistencyTestResult::new(rpc_result, api_result);
        assert!(result.is_consistent());
        
        let rpc_result2 = serde_json::json!({"value": 42});
        let api_result2 = serde_json::json!({"value": 43});
        
        let result2 = ConsistencyTestResult::new(rpc_result2, api_result2);
        assert!(!result2.is_consistent());
    }

    #[test]
    fn test_property_test_result() {
        let mut result = PropertyTestResult::new("test_property".to_string());
        assert!(!result.success);
        
        result.add_failure("Test failed".to_string());
        assert!(!result.success);
        assert_eq!(result.failures.len(), 1);
        
        let success_result = PropertyTestResult::new("test_property".to_string())
            .success(100);
        assert!(success_result.success);
        assert_eq!(success_result.iterations_run, 100);
    }
}