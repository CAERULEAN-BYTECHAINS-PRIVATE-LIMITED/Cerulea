//! Validator Set Management
//! 
//! This module provides functionality for managing validator sets in the CBC consensus system.
//! It interfaces with the DCF pallet to handle validator registration, activation, and scoring.

use sp_std::vec::Vec;
use sp_runtime::traits::{Block as BlockT};
use codec::{Encode, Decode};

use crate::error::{ConsensusResult, ConsensusError as CbcConsensusError};
use pallet_cbc_dcf::DcfApi;

/// Validator set manager for CBC consensus
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
    fn validator_info(&self, validator: &u64, at: &Block::Hash) -> ConsensusResult<Option<ValidatorInfo>>;
    
    /// Update validator activity status
    fn update_validator_activity(&self, validator: &u64, is_active: bool, at: &Block::Hash) -> ConsensusResult<()>;
}

impl<Block, Client> ValidatorSetOps<Block> for ValidatorSetManager<Block, Client>
where
    Block: BlockT,
    Client: sp_api::ProvideRuntimeApi<Block> + Send + Sync,
    Client::Api: pallet_cbc_dcf::DcfApi<Block, u64, u128, u32>,
{
    fn current_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>> {
        let api = self.client.runtime_api();
        
        // Use get_active_validators since there's no direct get_validator_set method
        api.get_active_validators(*at)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get validator set: {:?}", e)))
    }
    
    fn active_validators(&self, at: &Block::Hash) -> ConsensusResult<Vec<u64>> {
        let api = self.client.runtime_api();
        
        api.get_active_validators(*at)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get active validators: {:?}", e)))
    }
    
    fn is_validator_active(&self, validator: &u64, at: &Block::Hash) -> ConsensusResult<bool> {
        let api = self.client.runtime_api();
        
        api.is_validator_active(*at, *validator)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to check validator active status: {:?}", e)))
    }
    
    fn validator_info(&self, validator: &u64, at: &Block::Hash) -> ConsensusResult<Option<ValidatorInfo>> {
        let api = self.client.runtime_api();


        // Check if validator is active
        let is_active = self.is_validator_active(validator, at)?;
        
        // Get validator stake
        let stake = api.get_validator_stake(*at, *validator)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get validator stake: {:?}", e)))?;
        
        // Get validator scores
        let stake_score = api.get_validator_stake_score(*at, *validator)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get validator stake score: {:?}", e)))?;
        
        let inference_score = api.get_validator_inference_score(*at, *validator)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get validator inference score: {:?}", e)))?;
        
        // Get participation info
        let (authored_blocks, missed_blocks) = api.get_validator_participation(*at, *validator)
            .map_err(|e| CbcConsensusError::RuntimeApiError(format!("Failed to get validator participation: {:?}", e)))?;
        
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
    
    fn update_validator_activity(&self, _validator: &u64, _is_active: bool, _at: &Block::Hash) -> ConsensusResult<()> {
        // This would typically be handled by the runtime through extrinsics
        // For now, we just return Ok as this is a read-only operation from the consensus side
        Ok(())
    }
}

/// Validator set utilities
pub mod utils {
    use super::*;
    
    /// Check if a validator set is valid (non-empty and within limits)
    pub fn is_valid_validator_set(validators: &[u64], min_validators: u32, max_validators: u32) -> bool {
        let len = validators.len() as u32;
        len >= min_validators && len <= max_validators && !validators.is_empty()
    }
    
    /// Sort validators by score (descending)
    pub fn sort_validators_by_score(validators: &mut [ValidatorInfo]) {
        validators.sort_by(|a, b| b.score.cmp(&a.score));
    }
    
    /// Filter active validators from a list
    pub fn filter_active_validators(validators: &[ValidatorInfo]) -> Vec<ValidatorInfo> {
        validators.iter()
            .filter(|v| v.is_active)
            .cloned()
            .collect()
    }
    
    /// Calculate total stake for a set of validators
    pub fn calculate_total_stake(validators: &[ValidatorInfo]) -> u128 {
        validators.iter().map(|v| v.stake).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::*;
    
    #[test]
    fn test_validator_info_creation() {
        let validator_info = ValidatorInfo {
            account_id: 1,
            stake: 1000,
            score: 850,
            is_active: true,
            participation_rate: 95,
        };
        
        assert_eq!(validator_info.account_id, 1);
        assert_eq!(validator_info.stake, 1000);
        assert_eq!(validator_info.score, 850);
        assert!(validator_info.is_active);
        assert_eq!(validator_info.participation_rate, 95);
    }
    
    #[test]
    fn test_validator_set_validation() {
        let validators = vec![1, 2, 3, 4];
        assert!(utils::is_valid_validator_set(&validators, 3, 10));
        
        let empty_validators = vec![];
        assert!(!utils::is_valid_validator_set(&empty_validators, 3, 10));
        
        let too_many_validators = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        assert!(!utils::is_valid_validator_set(&too_many_validators, 3, 10));
    }
    
    #[test]
    fn test_validator_sorting() {
        let mut validators = vec![
            ValidatorInfo { account_id: 1, stake: 1000, score: 800, is_active: true, participation_rate: 90 },
            ValidatorInfo { account_id: 2, stake: 1500, score: 950, is_active: true, participation_rate: 95 },
            ValidatorInfo { account_id: 3, stake: 800, score: 750, is_active: true, participation_rate: 85 },
        ];
        
        utils::sort_validators_by_score(&mut validators);
        
        assert_eq!(validators[0].score, 950);
        assert_eq!(validators[1].score, 800);
        assert_eq!(validators[2].score, 750);
    }
    
    #[test]
    fn test_active_validator_filtering() {
        let validators = vec![
            ValidatorInfo { account_id: 1, stake: 1000, score: 800, is_active: true, participation_rate: 90 },
            ValidatorInfo { account_id: 2, stake: 1500, score: 950, is_active: false, participation_rate: 95 },
            ValidatorInfo { account_id: 3, stake: 800, score: 750, is_active: true, participation_rate: 85 },
        ];
        
        let active_validators = utils::filter_active_validators(&validators);
        
        assert_eq!(active_validators.len(), 2);
        assert_eq!(active_validators[0].account_id, 1);
        assert_eq!(active_validators[1].account_id, 3);
    }
    
    #[test]
    fn test_total_stake_calculation() {
        let validators = vec![
            ValidatorInfo { account_id: 1, stake: 1000, score: 800, is_active: true, participation_rate: 90 },
            ValidatorInfo { account_id: 2, stake: 1500, score: 950, is_active: true, participation_rate: 95 },
            ValidatorInfo { account_id: 3, stake: 800, score: 750, is_active: true, participation_rate: 85 },
        ];
        
        let total_stake = utils::calculate_total_stake(&validators);
        assert_eq!(total_stake, 3300);
    }
}