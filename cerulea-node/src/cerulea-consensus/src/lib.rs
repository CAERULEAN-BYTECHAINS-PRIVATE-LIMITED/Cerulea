//! The Dynamic Consensus Framework, node side.
//!
//! The Dynamic Consensus Framework is the agreement mechanism of the Cerulea
//! Blockchain. Eight engines carry the work. Three of them run on the node.
//! The Commit Engine reaches agreement and makes blocks irreversible. The
//! Authoring Engine selects the slot committee and the block author. The
//! Execution Engine groups transactions into domains and runs them in
//! parallel. The remaining five engines run inside the pallet set.
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

/// The engines of the Dynamic Consensus Framework.
pub mod engines;

/// Core consensus engine implementation
pub mod dcf;

/// Validator and author selection
pub mod author_selection;
/// Validator set management and operations
pub mod validator_set;

/// Epoch management
pub mod epoch_manager;
pub mod import_queue;
pub mod inherent_providers;
/// Block production and import
pub mod proposer_factory;

/// Core types and error handling
pub mod types;

/// The engine identifier the Cerulea block seal carries.
pub const CERULEA_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"ceru";

/// The key type identifier of the Cerulea validator signing key.
///
/// The node keystore holds the ed25519 validator key under this identifier.
/// The Authoring Engine signs the block seal with that key. The Commit Engine
/// signs every vote with that key. Signing and keystore lookup both use this
/// identifier.
pub const CERULEA_KEY_TYPE: sp_core::crypto::KeyTypeId = sp_core::crypto::KeyTypeId(*b"cerk");

/// The engine identifier the Commit Engine carries on its messages.
pub const CERULEA_COMMIT_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"cerf";

pub mod error;

/// Deployment policy checks
pub mod dvf_config_validator;
/// Vote propagation across peers
pub mod dvf_gossip;
/// Commitment record builder
pub mod justification_builder;
/// Vote aggregation service
pub mod vote_aggregator;
/// Vote creation service
pub mod vote_creator;

pub mod finality;
/// Lifecycle tracing subsystem
pub mod lifecycle_tracer;
/// Monitoring and finality
pub mod metrics;

#[cfg(test)]
mod tests;

// Re-export commonly used types
pub use dcf::{DcfConsensus, RealBlockImport};
pub use dvf_config_validator::{ConfigValidationError, DvfConfig};
pub use error::{ConsensusError, ConsensusResult};
pub use finality::FinalityEngine;
pub use finality::{FinalityNotification, FinalityNotifier};
pub use inherent_providers::CeruleaInherentDataProviders;
pub use proposer_factory::ProposerFactory;
pub use types::{
    AuthorSelectionMode, BlockT, ConsensusParams, EpochConfig, ValidatorInfo, ValidatorMetrics,
};
pub use vote_aggregator::{VoteAggregatorService, VotePoolPruningService};
pub use vote_creator::VoteCreatorService;

pub use engines::{
    AuthoringEngine, CollectionId, CommitEngine, Committee, CommitteeMember, ConflictDomain,
    Engine, EngineError, EngineKind, ExecutionEngine, KeyRange, Rank, Slot, SlotSeed,
    TransactionClass, TrustScore,
};
