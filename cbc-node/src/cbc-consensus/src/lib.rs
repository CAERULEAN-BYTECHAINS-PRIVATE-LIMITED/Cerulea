//! CBC Consensus Engine
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

/// Core consensus engine implementation
pub mod dcf;

/// Validator and author selection
pub mod author_selection;
/// Validator set management and operations
pub mod validator_set;

/// Block production and import
pub mod proposer_factory;
pub mod import_queue;
pub mod inherent_providers;
pub mod block_import;

/// Epoch management
pub mod epoch_manager;

/// Core types and error handling
pub mod types;

/// Consensus engine ID for CBC DCF
pub const CBC_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"cbcd";
pub mod error;

/// Monitoring and finality
pub mod metrics;
pub mod finality;
/// Lifecycle tracing subsystem
pub mod lifecycle_tracer;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use error::{ConsensusError, ConsensusResult};
pub use dcf::{RealBlockImport, DcfConsensus};
pub use types::{ValidatorInfo, ValidatorMetrics, EpochConfig, AuthorSelectionMode, ConsensusParams, BlockT};
pub use proposer_factory::ProposerFactory;
pub use finality::FinalityEngine;
pub use inherent_providers::CbcInherentDataProviders;
pub use block_import::CbcBlockImport;


