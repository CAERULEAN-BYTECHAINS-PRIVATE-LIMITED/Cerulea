//! Test modules for the Dynamic Consensus Framework.
//!
//! Each module covers one part of the node side framework.

/// Metrics collection and reporting tests
pub mod metrics_test;

/// Header parameter order and parent hash propagation tests
pub mod header_parameter_order_test;

/// Proposer factory integration tests
pub mod proposer_integration_test;

/// Prometheus metrics tests
pub mod task8_metrics_test;
