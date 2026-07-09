// Test error handling utilities
//
// This module provides utilities for handling and validating errors in tests,
// including RPC error validation and test failure analysis.

#![allow(dead_code)]

use serde_json::Value;
use std::fmt;

/// Expected RPC error for validation in tests
/// 
/// Defines the expected structure and content of RPC errors for validation
/// in error handling tests.
#[derive(Debug, PartialEq, Clone)]
pub struct ExpectedRpcError {
    /// Expected error code (e.g., -32602 for invalid params)
    pub code: i32,
    /// Expected substring in error message
    pub message_contains: String,
    /// Optional expected error data
    pub data: Option<Value>,
}

impl ExpectedRpcError {
    /// Create a new expected RPC error
    pub fn new(code: i32, message_contains: &str) -> Self {
        Self {
            code,
            message_contains: message_contains.to_string(),
            data: None,
        }
    }
    
    /// Create an expected error for invalid parameters
    pub fn invalid_params(message_contains: &str) -> Self {
        Self::new(-32602, message_contains)
    }
    
    /// Create an expected error for runtime API failures
    pub fn runtime_api_failure(message_contains: &str) -> Self {
        Self::new(-32000, message_contains)
    }
    
    /// Create an expected error for disabled CBC extensions
    pub fn disabled_extensions() -> Self {
        Self::new(-32001, "CBC RPC extensions are disabled")
    }
    
    /// Add expected error data
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Test error types for categorizing test failures
#[derive(Debug, Clone)]
pub enum TestError {
    /// RPC call failed unexpectedly
    RpcCallFailed {
        method: String,
        error: String,
    },
    /// Runtime API call failed unexpectedly
    RuntimeApiCallFailed {
        method: String,
        error: String,
    },
    /// Consistency check failed between RPC and Runtime API
    ConsistencyCheckFailed {
        rpc_result: Value,
        api_result: Value,
    },
    /// Property test failed with counterexample
    PropertyTestFailed {
        property: String,
        counterexample: String,
        iteration: u32,
    },
    /// Integration test setup failed
    IntegrationSetupFailed {
        reason: String,
    },
    /// Mock configuration error
    MockConfigurationError {
        component: String,
        error: String,
    },
    /// Test timeout
    TestTimeout {
        operation: String,
        timeout_ms: u64,
    },
    /// Unexpected error format
    UnexpectedErrorFormat {
        expected: String,
        actual: String,
    },
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestError::RpcCallFailed { method, error } => {
                write!(f, "RPC call to {} failed: {}", method, error)
            }
            TestError::RuntimeApiCallFailed { method, error } => {
                write!(f, "Runtime API call to {} failed: {}", method, error)
            }
            TestError::ConsistencyCheckFailed { rpc_result, api_result } => {
                write!(f, "Consistency check failed - RPC: {:?}, API: {:?}", rpc_result, api_result)
            }
            TestError::PropertyTestFailed { property, counterexample, iteration } => {
                write!(f, "Property '{}' failed at iteration {}: {}", property, iteration, counterexample)
            }
            TestError::IntegrationSetupFailed { reason } => {
                write!(f, "Integration test setup failed: {}", reason)
            }
            TestError::MockConfigurationError { component, error } => {
                write!(f, "Mock configuration error in {}: {}", component, error)
            }
            TestError::TestTimeout { operation, timeout_ms } => {
                write!(f, "Test timeout after {}ms during: {}", timeout_ms, operation)
            }
            TestError::UnexpectedErrorFormat { expected, actual } => {
                write!(f, "Unexpected error format - expected: {}, actual: {}", expected, actual)
            }
        }
    }
}

impl std::error::Error for TestError {}

/// Result type for test operations
pub type TestResult<T> = Result<T, TestError>;

/// Validate that an RPC error matches expected criteria
/// 
/// Checks that an RPC error has the expected code and message content.
/// This is used in error handling tests to verify proper error responses.
pub fn validate_rpc_error(
    result: Result<Value, Box<dyn std::error::Error>>,
    expected: &ExpectedRpcError,
) -> bool {
    match result {
        Err(error) => {
            let error_str = error.to_string();
            
            // Check if error contains expected code (as string)
            let code_str = expected.code.to_string();
            let has_code = error_str.contains(&code_str);
            
            // Check if error contains expected message
            let has_message = error_str.contains(&expected.message_contains);
            
            has_code && has_message
        }
        Ok(_) => false, // Expected an error but got success
    }
}

/// Validate RPC error from JSON-RPC response
/// 
/// Parses a JSON-RPC error response and validates it against expected criteria.
pub fn validate_jsonrpc_error(
    error_response: &Value,
    expected: &ExpectedRpcError,
) -> bool {
    if let Some(error_obj) = error_response.get("error") {
        let code_matches = error_obj.get("code")
            .and_then(|c| c.as_i64())
            .map(|c| c == expected.code as i64)
            .unwrap_or(false);
        
        let message_matches = error_obj.get("message")
            .and_then(|m| m.as_str())
            .map(|m| m.contains(&expected.message_contains))
            .unwrap_or(false);
        
        let data_matches = if let Some(expected_data) = &expected.data {
            error_obj.get("data")
                .map(|d| d == expected_data)
                .unwrap_or(false)
        } else {
            true // No data expected, so any data (or lack thereof) is fine
        };
        
        code_matches && message_matches && data_matches
    } else {
        false
    }
}

/// Error handler for property tests
/// 
/// Provides structured error handling for property test failures with
/// detailed counterexample information.
pub struct PropertyTestErrorHandler {
    property_name: String,
    failures: Vec<(u32, String)>,
}

impl PropertyTestErrorHandler {
    /// Create a new property test error handler
    pub fn new(property_name: &str) -> Self {
        Self {
            property_name: property_name.to_string(),
            failures: Vec::new(),
        }
    }
    
    /// Record a property test failure
    pub fn record_failure(&mut self, iteration: u32, counterexample: &str) {
        self.failures.push((iteration, counterexample.to_string()));
    }
    
    /// Check if any failures were recorded
    pub fn has_failures(&self) -> bool {
        !self.failures.is_empty()
    }
    
    /// Get the number of failures
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }
    
    /// Get a summary of all failures
    pub fn failure_summary(&self) -> String {
        if self.failures.is_empty() {
            format!("Property '{}' passed all tests", self.property_name)
        } else {
            let mut summary = format!(
                "Property '{}' failed {} times:\n",
                self.property_name,
                self.failures.len()
            );
            
            for (iteration, counterexample) in &self.failures {
                summary.push_str(&format!("  Iteration {}: {}\n", iteration, counterexample));
            }
            
            summary
        }
    }
    
    /// Create a TestError from the recorded failures
    pub fn into_test_error(self) -> Option<TestError> {
        if let Some((iteration, counterexample)) = self.failures.first() {
            Some(TestError::PropertyTestFailed {
                property: self.property_name,
                counterexample: counterexample.clone(),
                iteration: *iteration,
            })
        } else {
            None
        }
    }
}

/// Timeout handler for test operations
/// 
/// Provides timeout functionality for test operations that might hang.
pub struct TestTimeoutHandler {
    timeout_ms: u64,
}

impl TestTimeoutHandler {
    /// Create a new timeout handler with the specified timeout
    pub fn new(timeout_ms: u64) -> Self {
        Self { timeout_ms }
    }
    
    /// Execute an operation with timeout
    pub async fn execute_with_timeout<F, T>(
        &self,
        operation_name: &str,
        operation: F,
    ) -> TestResult<T>
    where
        F: std::future::Future<Output = T>,
    {
        match tokio::time::timeout(
            std::time::Duration::from_millis(self.timeout_ms),
            operation,
        ).await {
            Ok(result) => Ok(result),
            Err(_) => Err(TestError::TestTimeout {
                operation: operation_name.to_string(),
                timeout_ms: self.timeout_ms,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_expected_rpc_error_creation() {
        let error = ExpectedRpcError::invalid_params("malformed AccountId");
        assert_eq!(error.code, -32602);
        assert!(error.message_contains.contains("malformed AccountId"));
        
        let api_error = ExpectedRpcError::runtime_api_failure("call failed");
        assert_eq!(api_error.code, -32000);
        
        let disabled_error = ExpectedRpcError::disabled_extensions();
        assert_eq!(disabled_error.code, -32001);
    }

    #[test]
    fn test_validate_jsonrpc_error() {
        let error_response = json!({
            "error": {
                "code": -32602,
                "message": "Invalid params: malformed AccountId"
            }
        });
        
        let expected = ExpectedRpcError::invalid_params("malformed AccountId");
        assert!(validate_jsonrpc_error(&error_response, &expected));
        
        let wrong_expected = ExpectedRpcError::runtime_api_failure("different error");
        assert!(!validate_jsonrpc_error(&error_response, &wrong_expected));
    }

    #[test]
    fn test_property_test_error_handler() {
        let mut handler = PropertyTestErrorHandler::new("test_property");
        assert!(!handler.has_failures());
        
        handler.record_failure(5, "counterexample data");
        assert!(handler.has_failures());
        assert_eq!(handler.failure_count(), 1);
        
        let summary = handler.failure_summary();
        assert!(summary.contains("test_property"));
        assert!(summary.contains("Iteration 5"));
        assert!(summary.contains("counterexample data"));
    }

    #[test]
    fn test_test_error_display() {
        let error = TestError::RpcCallFailed {
            method: "test_method".to_string(),
            error: "connection failed".to_string(),
        };
        
        let error_str = error.to_string();
        assert!(error_str.contains("test_method"));
        assert!(error_str.contains("connection failed"));
    }

    #[tokio::test]
    async fn test_timeout_handler() {
        let handler = TestTimeoutHandler::new(100); // 100ms timeout
        
        // Test successful operation
        let result = handler.execute_with_timeout("quick_op", async {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            42
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
        
        // Test timeout
        let timeout_result = handler.execute_with_timeout("slow_op", async {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            42
        }).await;
        
        assert!(timeout_result.is_err());
        match timeout_result.unwrap_err() {
            TestError::TestTimeout { operation, timeout_ms } => {
                assert_eq!(operation, "slow_op");
                assert_eq!(timeout_ms, 100);
            }
            _ => panic!("Expected TestTimeout error"),
        }
    }
}