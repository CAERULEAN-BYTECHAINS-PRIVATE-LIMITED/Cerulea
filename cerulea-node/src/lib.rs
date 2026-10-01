// Library interface for cerulea-node
// This allows tests and the binary to access internal modules

pub mod benchmarking;
pub mod block_tracker;
pub mod chain_spec;
pub mod command;
pub mod fork_detection;
pub mod node_command;
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

/// The development Blockchain the node tests share.
#[cfg(test)]
mod chain_fixture_tests;

/// The node interface methods against the real client.
#[cfg(test)]
mod rpc_client_tests;

/// The platform reading surface against the real client.
#[cfg(test)]
mod platform_rpc_client_tests;

/// The import queue and the proposer factory against the real client.
#[cfg(test)]
mod consensus_client_tests;

/// The block authoring tracker against the real client.
#[cfg(test)]
mod block_tracker_client_tests;

/// The node subcommands through the shared entry point.
#[cfg(test)]
mod command_tests;

/// The partial and the full service on real chains.
#[cfg(test)]
mod service_tests;
