//! Validator Set Management
//!
//! This module provides functionality for managing validator sets in the Cerulea consensus system.
//! It interfaces with the DCF pallet to handle validator registration, activation, and scoring.

use codec::{Decode, Encode};
use sp_runtime::traits::Block as BlockT;
use sp_std::vec::Vec;

use crate::error::{ConsensusError as CeruleaConsensusError, ConsensusResult};
use pallet_cerulea_dcf::DcfApi;

/// Validator set manager for Cerulea consensus
pub struct ValidatorSetManager<Block: BlockT, Client> {
    client: Client,
    _phantom: sp_std::marker::PhantomData<Block>,
}

impl<Block: BlockT, Client> ValidatorSetManager<Block, Client> {
    /// Create a new validator set manager
    pub fn new(client: Client) -> Self {
        Self {
            client,
            _phantom: Default::default(),
        }
    }
}

/// Validator information structure
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode)]
pub struct ValidatorInfo {
    /// Validator account ID
    pub account_id: u64,
    /// Current stake amount
    pub stake: u128,
    /// Current score
    pub score: u32,
    /// Whether the validator is active
    pub is_active: bool,
    /// Participation rate percentage
    pub participation_rate: u8,
}

/// Validator set operations
pub trait ValidatorSetOps<Block: BlockT> {
    /// Get the current validator set
    fn current_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>>;

    /// Get active validators
    fn active_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>>;

    /// Check if a validator is active
    fn is_validator_active(&self, validator: &u64, at: &Block::Hash) -> ConsensusResult<bool>;

    /// Get validator information
    fn validator_info(
        &self,
        validator: &u64,
        at: &Block::Hash,
    ) -> ConsensusResult<Option<ValidatorInfo>>;

    /// Update validator activity status
    fn update_validator_activity(
        &self,
        validator: &u64,
        is_active: bool,
        at: &Block::Hash,
    ) -> ConsensusResult<()>;
}

impl<Block, Client> ValidatorSetOps<Block> for ValidatorSetManager<Block, Client>
where
    Block: BlockT,
    Client: sp_api::ProvideRuntimeApi<Block> + Send + Sync,
    Client::Api: pallet_cerulea_dcf::DcfApi<Block, u64, u128, u32>,
{
    fn current_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>> {
        let api = self.client.runtime_api();

        // Use get_active_validators since there's no direct get_validator_set method
        api.get_active_validators(*at).map_err(|e| {
            CeruleaConsensusError::RuntimeApiError(format!("Failed to get validator set: {:?}", e))
        })
    }

    fn active_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>> {
        let api = self.client.runtime_api();

        api.get_active_validators(*at).map_err(|e| {
            CeruleaConsensusError::RuntimeApiError(format!(
                "Failed to get active validators: {:?}",
                e
            ))
        })
    }

    fn is_validator_active(&self, validator: &u64, at: &Block::Hash) -> ConsensusResult<bool> {
        let api = self.client.runtime_api();

        api.is_validator_active(*at, *validator).map_err(|e| {
            CeruleaConsensusError::RuntimeApiError(format!(
                "Failed to check validator active status: {:?}",
                e
            ))
        })
    }

    fn validator_info(
        &self,
        validator: &u64,
        at: &Block::Hash,
    ) -> ConsensusResult<Option<ValidatorInfo>> {
        let api = self.client.runtime_api();

        // Check if validator is active
        let is_active = self.is_validator_active(validator, at)?;

        // Get validator stake
        let stake = api.get_validator_stake(*at, *validator).map_err(|e| {
            CeruleaConsensusError::RuntimeApiError(format!(
                "Failed to get validator stake: {:?}",
                e
            ))
        })?;

        // Get validator scores
        let stake_score = api
            .get_validator_stake_score(*at, *validator)
            .map_err(|e| {
                CeruleaConsensusError::RuntimeApiError(format!(
                    "Failed to get validator stake score: {:?}",
                    e
                ))
            })?;

        let inference_score = api
            .get_validator_inference_score(*at, *validator)
            .map_err(|e| {
                CeruleaConsensusError::RuntimeApiError(format!(
                    "Failed to get validator inference score: {:?}",
                    e
                ))
            })?;

        // Get participation info
        let (authored_blocks, missed_blocks) = api
            .get_validator_participation(*at, *validator)
            .map_err(|e| {
                CeruleaConsensusError::RuntimeApiError(format!(
                    "Failed to get validator participation: {:?}",
                    e
                ))
            })?;

        // Calculate participation rate
        let total_blocks = authored_blocks + missed_blocks;
        let participation_rate = if total_blocks > 0 {
            ((authored_blocks * 100) / total_blocks) as u8
        } else {
            100u8
        };

        // Calculate final score (simple average for now)
        let final_score = ((stake_score + inference_score) / 2) as u32;

        Ok(Some(ValidatorInfo {
            account_id: *validator,
            stake,
            score: final_score,
            is_active,
            participation_rate,
        }))
    }

    fn update_validator_activity(
        &self,
        _validator: &u64,
        _is_active: bool,
        _at: &Block::Hash,
    ) -> ConsensusResult<()> {
        // This would typically be handled by the runtime through extrinsics
        // For now, we just return Ok as this is a read-only operation from the consensus side
        Ok(())
    }
}

/// Validator set utilities
pub mod utils {
    use super::*;

    /// Check if a validator set is valid (non-empty and within limits)
    pub fn is_valid_validator_set(
        validators: &[u64],
        min_validators: u32,
        max_validators: u32,
    ) -> bool {
        let len = validators.len() as u32;
        len >= min_validators && len <= max_validators && !validators.is_empty()
    }

    /// Sort validators by score (descending)
    pub fn sort_validators_by_score(validators: &mut [ValidatorInfo]) {
        validators.sort_by(|a, b| b.score.cmp(&a.score));
    }

    /// Filter active validators from a list
    pub fn filter_active_validators(validators: &[ValidatorInfo]) -> Vec<ValidatorInfo> {
        validators.iter().filter(|v| v.is_active).cloned().collect()
    }

    /// Calculate total stake for a set of validators
    pub fn calculate_total_stake(validators: &[ValidatorInfo]) -> u128 {
        validators.iter().map(|v| v.stake).sum()
    }
}
