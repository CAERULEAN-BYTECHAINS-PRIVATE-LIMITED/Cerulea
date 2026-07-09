//! Test modules for CBC Consensus Engine
//!
//! This module organizes all test files for the CBC consensus implementation.
//! Tests are organized by functionality to improve maintainability and debugging.

/// Epoch manager functionality tests
pub mod epoch_manager_test;

/// Metrics collection and reporting tests
pub mod metrics_test;

/// Validator set management tests
pub mod validator_set_test;

/// Header parameter order and parent hash propagation tests
pub mod header_parameter_order_test;

/// Proposer factory integration tests
pub mod proposer_integration_test;

/// Task 8 Prometheus metrics tests
pub mod task8_metrics_test;

/// State root calculation fix tests (Issue #1)
pub mod state_root_fix_test;