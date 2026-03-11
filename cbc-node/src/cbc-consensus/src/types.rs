//! Common types used throughout the consensus engine
//! 
//! This module defines the core types used in the consensus implementation.

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::traits::Block as BlockTrait;
use sp_runtime::generic::{Block, Header, UncheckedExtrinsic};
use sp_runtime::traits::BlakeTwo256;
use sp_core::ed25519::Public;
use std::default::Default;


/// Block type alias for the consensus engine
pub type BlockT = Block<Header<u32, BlakeTwo256>, UncheckedExtrinsic<Public, (), (), ()>>;

/// Configuration for epoch management
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct EpochConfig {
    /// Number of slots per epoch
    pub epoch_length: u32,
    /// Minimum stake required to be a validator
    pub min_validators: u32,
    /// Maximum number of validators per epoch
    pub max_validators: u32,
    /// Minimum stake required
    pub min_stake: u128,
}

/// Validator information including stake and performance metrics
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct ValidatorInfo {
    /// Validator's public key
    pub account_id: Public,
    /// Current stake amount
    pub stake: u128,
    /// Proof of Inference score
    pub performance_score: u32,
    /// Number of blocks produced
    pub blocks_produced: u32,
    /// Number of blocks missed
    pub blocks_missed: u32,
}

/// Mode for selecting block authors
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub enum AuthorSelectionMode {
    /// Round-robin selection
    RoundRobin,
    /// Stake-weighted selection
    StakeWeighted,
    /// Performance-based selection
    PerformanceBased,
    /// Pure Proof of Stake
    PoS,
    /// Hybrid PoS + Proof of Importance
    Hybrid,
}

/// Consensus parameters
#[derive(Debug, Clone, Encode, Decode, TypeInfo)]
pub struct ConsensusParams {
    /// Current author selection mode
    pub author_selection_mode: AuthorSelectionMode,
    /// Number of blocks required for finality
    pub finality_threshold: u32,
    /// Minimum time between blocks (in seconds)
    pub block_time: u64,
    /// Maximum block size
    pub max_block_size: u32,
    /// Maximum transactions per block
    pub max_transactions_per_block: u32,
    /// Slot duration for consensus timing
    pub slot_duration: std::time::Duration,
    /// Minimum block time in milliseconds
    pub min_block_time: u64,
    /// Interval for updating validator metrics (in slots)
    pub metrics_update_interval: u64,
    /// Interval for refreshing validator scores (in slots)
    pub score_refresh_interval: u64,
    /// Interval for consensus loop sleep (in milliseconds)
    pub consensus_loop_interval: u64,
    /// Interval for detailed logging (in slots)
    pub detailed_logging_interval: u64,
    /// Interval for health checks (in blocks)
    pub health_check_interval: u32,
    /// Minimum score threshold for validator performance
    pub min_performance_score: u64,
    /// High score threshold for top performer identification
    pub high_performance_score: u64,
    /// Minimum participation rate percentage for validators
    pub min_participation_rate: u32,
    /// High participation rate percentage for top performers
    pub high_participation_rate: u32,
    /// Maximum missed blocks before penalty
    pub max_missed_blocks: u32,
    /// Maximum missed blocks for high performers
    pub max_missed_blocks_high: u32,
    /// Score threshold for healthy validator classification
    pub healthy_validator_score: u64,
    /// Participation rate threshold for healthy validators
    pub healthy_participation_rate: u32,
    /// Maximum missed blocks for healthy validators
    pub healthy_missed_blocks_max: u32,
    /// Number of top validators to display in logs
    pub top_validators_display_count: u32,
    /// Number of validators to sample for health checks
    pub health_check_sample_size: u32,
}

/// Block import result
#[derive(Debug)]
pub enum ImportResult<B: BlockTrait> {
    /// Block imported successfully
    Imported(B::Hash),
    /// Block already exists
    AlreadyInChain(B::Hash),
    /// Block rejected
    Rejected(B::Hash, String),
}

/// Validator performance metrics
#[derive(Debug, Clone, Encode, Decode, TypeInfo, Default)]
pub struct ValidatorMetrics {
    /// Number of blocks produced
    pub blocks_produced: u32,
    /// Total blocks validated in current epoch
    pub blocks_validated: u32,
    /// Uptime percentage
    pub uptime: u32,
    /// Total blocks in epoch
    pub total_blocks: u32,
    /// Total missed blocks
    pub total_missed: u32,
    /// Failed block production attempts
    pub failed_blocks: u32,
}

impl ValidatorMetrics {
    /// Update validator score with new metrics
    pub fn update_validator_score(&mut self, _author: Public, _stake_weight: u32, _inference_weight: u32, final_score: u32) {
        self.blocks_produced = self.blocks_produced.saturating_add(1);
        self.blocks_validated = self.blocks_validated.saturating_add(1);
        self.uptime = final_score;
    }
}

/// Utility functions for consensus operations
pub mod utils {
    use sc_client_api::HeaderBackend;
    use sp_runtime::traits::{Block as BlockT, NumberFor};

    /// Checks if a block number is at or below the client's finalized head
    ///
    /// Returns true if the block is already finalized, false otherwise.
    pub fn is_block_finalized<Block, Client>(
        client: &Client,
        block_number: NumberFor<Block>,
    ) -> bool
    where
        Block: BlockT,
        Client: HeaderBackend<Block>,
    {
        let finalized_number = client.info().finalized_number;
        block_number <= finalized_number
    }
}