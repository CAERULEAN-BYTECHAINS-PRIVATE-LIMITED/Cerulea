// Test logging utilities
//
// This module provides logging configuration and utilities for test execution,
// including structured logging for property tests and integration tests.

#![allow(dead_code)]

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test logging with appropriate configuration
/// 
/// Sets up logging for test execution with appropriate levels and formatting.
/// This should be called once at the beginning of test suites that need logging.
pub fn init_test_logging() {
    INIT.call_once(|| {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Info)
            .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
            .format_module_path(false)
            .format_target(false)
            .init();
    });
}

/// Initialize verbose test logging for debugging
/// 
/// Sets up more detailed logging for debugging test failures and issues.
pub fn init_verbose_test_logging() {
    INIT.call_once(|| {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
            .format_module_path(true)
            .format_target(true)
            .init();
    });
}

/// Test logger for structured test output
/// 
/// Provides structured logging specifically for test execution with
/// consistent formatting and categorization.
pub struct TestLogger {
    test_name: String,
    verbose: bool,
}

impl TestLogger {
    /// Create a new test logger for the given test
    pub fn new(test_name: &str, verbose: bool) -> Self {
        Self {
            test_name: test_name.to_string(),
            verbose,
        }
    }
    
    /// Get the test name
    pub fn test_name(&self) -> &str {
        &self.test_name
    }
    
    /// Check if verbose logging is enabled
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }
    
    /// Log the start of a test
    pub fn test_start(&self) {
        if self.verbose {
            log::info!("[{}] Starting test", self.test_name);
        }
    }
    
    /// Log the successful completion of a test
    pub fn test_success(&self) {
        log::info!("[{}] Test completed successfully", self.test_name);
    }
    
    /// Log a test failure with details
    pub fn test_failure(&self, error: &str) {
        log::error!("[{}] Test failed: {}", self.test_name, error);
    }
    
    /// Log property test progress
    pub fn property_progress(&self, iteration: u32, total: u32) {
        if self.verbose && iteration % 10 == 0 {
            log::debug!("[{}] Property test progress: {}/{}", self.test_name, iteration, total);
        }
    }
    
    /// Log property test failure with counterexample
    pub fn property_failure(&self, iteration: u32, counterexample: &str) {
        log::error!(
            "[{}] Property test failed at iteration {}: {}",
            self.test_name,
            iteration,
            counterexample
        );
    }
    
    /// Log RPC call details
    pub fn rpc_call(&self, method: &str, params: &str) {
        if self.verbose {
            log::debug!("[{}] RPC call: {} with params: {}", self.test_name, method, params);
        }
    }
    
    /// Log Runtime API call details
    pub fn runtime_api_call(&self, method: &str, params: &str) {
        if self.verbose {
            log::debug!("[{}] Runtime API call: {} with params: {}", self.test_name, method, params);
        }
    }
    
    /// Log consistency check results
    pub fn consistency_check(&self, rpc_result: &str, api_result: &str, consistent: bool) {
        if consistent {
            if self.verbose {
                log::debug!("[{}] Consistency check passed", self.test_name);
            }
        } else {
            log::error!(
                "[{}] Consistency check failed - RPC: {}, API: {}",
                self.test_name,
                rpc_result,
                api_result
            );
        }
    }
    
    /// Log integration test setup
    pub fn integration_setup(&self, description: &str) {
        if self.verbose {
            log::info!("[{}] Integration test setup: {}", self.test_name, description);
        }
    }
    
    /// Log mock setup details
    pub fn mock_setup(&self, description: &str) {
        if self.verbose {
            log::debug!("[{}] Mock setup: {}", self.test_name, description);
        }
    }
    
    /// Log test data generation
    pub fn data_generation(&self, description: &str) {
        if self.verbose {
            log::debug!("[{}] Generated test data: {}", self.test_name, description);
        }
    }
    
    /// Log performance metrics
    pub fn performance(&self, operation: &str, duration_ms: u64) {
        if self.verbose {
            log::info!("[{}] Performance - {}: {}ms", self.test_name, operation, duration_ms);
        }
    }
}

/// Macro for creating a test logger with the current test name
#[macro_export]
macro_rules! test_logger {
    ($verbose:expr) => {
        $crate::common::logging::TestLogger::new(
            &format!("{}::{}", module_path!(), std::thread::current().name().unwrap_or("unknown")),
            $verbose
        )
    };
}

/// Macro for logging property test iterations
#[macro_export]
macro_rules! log_property_iteration {
    ($logger:expr, $iteration:expr, $total:expr) => {
        $logger.property_progress($iteration, $total);
    };
}

/// Macro for logging RPC/API consistency checks
#[macro_export]
macro_rules! log_consistency_check {
    ($logger:expr, $rpc:expr, $api:expr) => {
        let consistent = $rpc == $api;
        $logger.consistency_check(
            &format!("{:?}", $rpc),
            &format!("{:?}", $api),
            consistent
        );
        consistent
    };
}

/// Test timing utilities
pub struct TestTimer {
    start_time: std::time::Instant,
    test_name: String,
}

impl TestTimer {
    /// Start timing a test operation
    pub fn start(test_name: &str) -> Self {
        Self {
            start_time: std::time::Instant::now(),
            test_name: test_name.to_string(),
        }
    }
    
    /// Get elapsed time in milliseconds
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
    
    /// Log the elapsed time and return it
    pub fn log_elapsed(&self, operation: &str) -> u64 {
        let elapsed = self.elapsed_ms();
        log::info!("[{}] {} completed in {}ms", self.test_name, operation, elapsed);
        elapsed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logger_creation() {
        let logger = TestLogger::new("test_function", false);
        assert_eq!(logger.test_name, "test_function");
        assert!(!logger.verbose);
        
        let verbose_logger = TestLogger::new("test_function", true);
        assert!(verbose_logger.verbose);
    }

    #[test]
    fn test_timer() {
        let timer = TestTimer::start("test_operation");
        std::thread::sleep(std::time::Duration::from_millis(1));
        let elapsed = timer.elapsed_ms();
        assert!(elapsed >= 1);
    }

    #[test]
    fn test_logging_initialization() {
        // Test that logging can be initialized without panicking
        init_test_logging();
        
        // Test that multiple calls don't panic (due to Once)
        init_test_logging();
        init_verbose_test_logging();
    }
}