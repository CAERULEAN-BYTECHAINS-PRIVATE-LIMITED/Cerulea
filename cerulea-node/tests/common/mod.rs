// Common test utilities for CBC API testing
//
// This module provides shared infrastructure for testing CBC Runtime APIs and RPC endpoints:
// - Test data generators for property-based testing
// - Test runtime builder for integration testing
// - Mock client builder for unit testing
// - Test configuration types and utilities
// - Logging and error handling utilities

pub mod generators;
pub mod test_runtime;
pub mod mock_client;
pub mod config;
pub mod logging;
pub mod error_handling;

// Re-export commonly used types and functions
pub use generators::*;
pub use test_runtime::*;
pub use mock_client::*;
pub use config::*;
pub use logging::*;
pub use error_handling::*;