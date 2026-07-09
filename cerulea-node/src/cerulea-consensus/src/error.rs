//! Error types for the consensus engine
//! 
//! This module defines error types used throughout the consensus engine.

use thiserror::Error;

/// Result type for consensus operations
pub type ConsensusResult<T> = std::result::Result<T, ConsensusError>;

/// Consensus error types
#[derive(Debug, Error)]
pub enum ConsensusError {
    /// Error during block validation
    #[error("Block validation failed: {0}")]
    BlockValidation(String),

    /// Error during epoch transition
    #[error("Epoch transition failed: {0}")]
    EpochTransition(String),

    /// Error during author selection
    #[error("Author selection failed: {0}")]
    AuthorSelection(String),

    /// Error during block import
    #[error("Block import failed: {0}")]
    BlockImport(String),

    /// Error during finality
    #[error("Finality error: {0}")]
    Finality(String),

    /// Error in validator set management
    #[error("Validator set error: {0}")]
    ValidatorSet(String),

    /// Error with block author
    #[error("Invalid block author: {0}")]
    InvalidAuthor(String),

    /// Error during validator set update
    #[error("Validator set update failed: {0}")]
    ValidatorSetUpdate(String),

    /// Error during metrics tracking
    #[error("Metrics error: {0}")]
    Metrics(String),

    /// Internal consensus error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Error during block proposal
    #[error("Proposer error: {0}")]
    Proposer(String),

    /// Error during finality check
    #[error("Finality check failed: {0}")]
    FinalityCheck(String),

    /// Error during block production
    #[error("Block production failed: {0}")]
    BlockProduction(String),

    /// Block author mismatch detected
    #[error("Block author mismatch: expected {expected:?}, got {actual:?} for block {block_number}")]
    AuthorMismatch {
        /// Block number where the mismatch occurred
        block_number: u32,
        /// Expected block author
        expected: Option<cerulea_runtime::AccountId>,
        /// Actual block author found
        actual: cerulea_runtime::AccountId,
    },

    /// Runtime API error
    #[error("Runtime API error: {0}")]
    RuntimeApiError(String),
}