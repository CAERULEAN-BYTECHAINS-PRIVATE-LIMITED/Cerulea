// Library interface for cerulea-node
// This allows tests and the binary to access internal modules

pub mod benchmarking;
pub mod block_tracker;
pub mod chain_spec;
pub mod cli;
pub mod command;
pub mod fork_detection;
pub use cerulea_consensus::lifecycle_tracer;
pub mod logging;
pub mod platform_rpc;
pub mod rpc;
pub mod service;

/// An in-process network of validator nodes for the engine tests.
///
/// The harness runs the real engines. The integration tests under `tests/`
/// build a network, run slots and assert on what the engines decide.
#[cfg(any(test, feature = "test-harness"))]
pub mod test_harness;

/// The parametrised multi-node harness carrying stated faults.
///
/// The module runs the same engines as the healthy harness under node loss,
/// network delay, storage exhaustion and clock skew.
#[cfg(any(test, feature = "test-harness"))]
pub mod fault_harness;
