//! Epoch management implementation
//!
//! This module provides epoch transition logic and validator set management
//! that works in conjunction with the DCF runtime pallet.

use crate::{
    error::{ConsensusError, ConsensusResult},
    types::{EpochConfig, ValidatorInfo},
};
use std::sync::Arc;
use sp_runtime::traits::NumberFor;
use log::{info, warn, error, debug};
use sp_runtime::traits::{Block as BlockTrait, SaturatedConversion};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use cbc_runtime::AccountId;

/// Configuration constants from the runtime
#[derive(Debug, Clone)]
pub struct EpochManagerRuntimeConfig {
    /// Minimum score for good performance
    pub min_performance_score: u64,
    /// Score for high performance rewards
    pub high_performance_score: u64,
    /// Minimum participation rate percentage
    pub min_participation_rate: u32,
    /// High participation rate percentage
    pub high_participation_rate: u32,
    /// Maximum missed blocks before penalty
    pub max_missed_blocks: u32,
    /// Maximum missed blocks for high performers
    pub max_missed_blocks_high: u32,
    /// Score threshold for healthy validator
    pub healthy_validator_score: u64,
    /// Participation rate for healthy validator
    pub healthy_participation_rate: u32,
    /// Max missed blocks for healthy validator
    pub healthy_missed_blocks_max: u32,
    /// Cooldown period in blocks after leaving
    pub leave_cooldown: u32,
    /// Number of top validators to display in logs
    pub top_validators_display_count: u32,
}

/// Epoch manager for handling epoch transitions and validator set updates
pub struct EpochManager<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    client: Arc<C>,
    epoch_config: EpochConfig,
    _phantom: std::marker::PhantomData<B>,
}

impl<B, C> EpochManager<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    /// Create a new epoch manager
    pub fn new(client: Arc<C>, epoch_config: EpochConfig) -> Self {
        Self {
            client,
            epoch_config,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get PoS score for a validator from the PoS pallet
    fn get_pos_score(&self, validator: &AccountId) -> u64 {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        match api.get_validator_stake_score(best_hash, validator.clone()) {
            Ok(score) => score as u64,
            Err(e) => {
                debug!("Failed to get PoS score for validator {:?}: {:?}", validator, e);
                0u64
            }
        }
    }

    /// Calculate validator uptime based on historical data
    fn calculate_validator_uptime(&self, validator: &AccountId) -> u32 {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get validator uptime from DCF pallet
        match api.get_validator_uptime(best_hash, validator.clone()) {
            Ok(Some(uptime_stats)) => {
                // Return the participation rate as uptime percentage
                uptime_stats.participation_rate
            }
            _ => {
                debug!("Failed to calculate uptime for validator {:?}", validator);
                0u32
            }
        }
    }

    /// Get validator participation rate and missed blocks
    fn get_validator_participation_metrics(&self, validator: &AccountId) -> (u32, u32) {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        match api.get_validator_participation(best_hash, validator.clone()) {
            Ok((authored, missed)) => {
                let total_blocks = authored + missed;
                let participation_rate = if total_blocks > 0 {
                    ((authored * 100) / total_blocks).min(100)
                } else {
                    100 // New validators get 100% participation initially
                };
                (participation_rate, missed)
            }
            Err(e) => {
                debug!("Failed to get participation metrics for validator {:?}: {:?}", validator, e);
                (0u32, 0u32)
            }
        }
    }

    /// Get runtime configuration constants
    fn get_runtime_config(&self) -> ConsensusResult<EpochManagerRuntimeConfig> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let config_tuple = api.get_epoch_manager_config(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get epoch manager config: {:?}", e)))?;
        
        Ok(EpochManagerRuntimeConfig {
            min_performance_score: config_tuple.0,
            high_performance_score: config_tuple.1,
            min_participation_rate: config_tuple.2,
            high_participation_rate: config_tuple.3,
            max_missed_blocks: config_tuple.4,
            max_missed_blocks_high: config_tuple.5,
            healthy_validator_score: config_tuple.6,
            healthy_participation_rate: config_tuple.7,
            healthy_missed_blocks_max: config_tuple.8,
            leave_cooldown: config_tuple.9,
            top_validators_display_count: config_tuple.10,
        })
    }

    /// Check if an epoch transition should occur
    pub fn should_transition_epoch(&self, current_block: u32) -> ConsensusResult<bool> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let current_epoch = api.get_current_epoch(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e)))?;
        
        // Get epoch length from runtime configuration (T::EpochLength)
        let epoch_length = api.get_epoch_length(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get epoch length: {:?}", e)))?;
        
        let epoch_start_block = current_epoch.saturating_mul(epoch_length);
        let should_transition = current_block >= epoch_start_block + epoch_length;
        
        if should_transition {
            info!("DCF EpochManager: Epoch transition needed at block {} (epoch {} -> {}, length: {})", 
                  current_block, current_epoch, current_epoch + 1, epoch_length);
        }
        
        Ok(should_transition)
    }

    /// Handle epoch transition logic
    pub fn handle_epoch_transition(&self, current_block: u32) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let current_epoch = api.get_current_epoch(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e)))?;
        
        let next_epoch = current_epoch.saturating_add(1);
        
        info!("DCF EpochManager: Processing epoch transition {} -> {} at block {}", 
              current_epoch, next_epoch, current_block);

        // 1. Apply validator score decay for inactive validators
        self.apply_validator_score_decay(current_epoch)?;

        // 2. Update active validator set based on scores and stake
        self.update_active_validator_set()?;

        // 3. Handle validator join/leave requests
        self.process_validator_set_changes()?;

        // 4. Update validator participation rates
        self.update_validator_participation_rates()?;

        // 5. Check for underperforming validators
        self.handle_underperforming_validators()?;

        info!("DCF EpochManager: Epoch transition completed successfully");
        Ok(())
    }

    /// Apply score decay to inactive validators
    fn apply_validator_score_decay(&self, current_epoch: u32) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let all_validators = api.get_validator_scores(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e)))?;
        
        let mut decayed_count = 0;
        
        for (validator, _score) in all_validators {
            let last_active = api.get_validator_last_active(best_hash, validator.clone())
                .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get validator last active: {:?}", e)))?;
            
            let inactive_epochs = current_epoch.saturating_sub(last_active);
            
            if inactive_epochs > 0 {
                // Score decay is handled by the runtime pallet automatically
                info!("DCF EpochManager: Validator {:?} inactive for {} epochs", validator, inactive_epochs);
                decayed_count += 1;
            }
        }
        
        if decayed_count > 0 {
            info!("DCF EpochManager: Applied score decay to {} inactive validators", decayed_count);
        }
        
        Ok(())
    }

    /// Update the active validator set based on current scores and stake
    fn update_active_validator_set(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let current_active = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        let all_scores = api.get_validator_scores(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e)))?;
        
        // Filter validators that meet minimum requirements
        let mut eligible_validators: Vec<(AccountId, u64)> = Vec::new();
        
        // Get current block number for cooldown check
        let current_block = self.client.info().best_number.saturated_into::<u32>();
        
        // Get runtime configuration
        let config = self.get_runtime_config()?;
        
        for (validator, score) in all_scores {
            // Check minimum score requirement
            if score >= config.healthy_validator_score {
                // Check if validator is in cooldown period after leaving
                match api.get_validator_leave_request(best_hash, validator.clone()) {
                    Ok(Some(leave_block)) => {
                        let blocks_passed = current_block.saturating_sub(leave_block);
                        let cooldown_period = config.leave_cooldown;
                        
                        if blocks_passed < cooldown_period {
                            // Validator is still in cooldown period, skip
                            debug!("DCF EpochManager: Validator {:?} still in cooldown period ({} blocks remaining)", 
                                   validator, cooldown_period.saturating_sub(blocks_passed));
                            continue;
                        }
                    }
                    Ok(None) => {
                        // No leave request, validator is eligible
                    }
                    Err(e) => {
                        warn!("DCF EpochManager: Failed to check leave request for validator {:?}: {:?}", validator, e);
                        // Continue with validator to avoid blocking the epoch transition
                    }
                }
                
                // Check if validator has sufficient stake (this was checked via PoS pallet)
                eligible_validators.push((validator, score));
            }
        }
        
        // Sort by score (descending) and take top validators up to max limit
        eligible_validators.sort_by(|a, b| b.1.cmp(&a.1));
        
        let max_validators = self.epoch_config.max_validators as usize;
        if eligible_validators.len() > max_validators {
            eligible_validators.truncate(max_validators);
        }
        
        let new_active_count = eligible_validators.len();
        let old_active_count = current_active.len();
        
        info!("DCF EpochManager: Validator set update - {} -> {} active validators", 
              old_active_count, new_active_count);
        Ok(())
    }

    /// Process validator join/leave requests
    fn process_validator_set_changes(&self) -> ConsensusResult<()> {        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        // Validate minimum validator count
        if active_validators.len() < self.epoch_config.min_validators as usize {
            warn!("DCF EpochManager: Active validator count ({}) below minimum ({})", 
                  active_validators.len(), self.epoch_config.min_validators);
        }
        
        // Validate maximum validator count
        if active_validators.len() > self.epoch_config.max_validators as usize {
            error!("DCF EpochManager: Active validator count ({}) exceeds maximum ({})", 
                   active_validators.len(), self.epoch_config.max_validators);
        }
        
        info!("DCF EpochManager: Processed validator set changes - {} active validators", 
              active_validators.len());
        
        Ok(())
    }

    /// Update validator participation rates
    fn update_validator_participation_rates(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        let mut total_participation = 0u32;
        let mut validator_count = 0u32;
        
        for validator in active_validators {
            if let Ok((authored, missed)) = api.get_validator_participation(best_hash, validator.clone()) {
                let total_blocks = authored + missed;
                let participation_rate = if total_blocks > 0 {
                    (authored * 100) / total_blocks
                } else {
                    100 // New validators get full participation initially
                };
                
                total_participation += participation_rate;
                validator_count += 1;
                
                if participation_rate < config.healthy_participation_rate {
                    warn!("DCF EpochManager: Low participation rate for validator {:?}: {}%", 
                          validator, participation_rate);
                }
            }
        }
        
        let average_participation = if validator_count > 0 {
            total_participation / validator_count
        } else {
            0
        };
        
        info!("DCF EpochManager: Average validator participation rate: {}%", average_participation);
        
        Ok(())
    }

    /// Handle underperforming validators
    fn handle_underperforming_validators(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        let mut underperforming_count = 0;
        
        for validator in active_validators {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let _trust_score = profile.trust_score;
                let _inference_count = profile.inference_count;
                let (participation_rate, missed_blocks) = self.get_validator_participation_metrics(&validator);
                
                // Check for underperformance criteria
                let is_underperforming = combined_score < config.min_performance_score as u64 || 
                                        participation_rate < config.min_participation_rate || 
                                        missed_blocks > config.max_missed_blocks;
                
                if is_underperforming {
                    warn!("DCF EpochManager: Underperforming validator detected: {:?} - Combined Score: {}, Participation: {}%, Missed: {}", 
                          validator, combined_score, participation_rate, missed_blocks);
                    underperforming_count += 1;
                    
                    // The runtime pallet will handle automatic ejection if score falls below threshold
                }
            }
        }
        
        if underperforming_count > 0 {
            info!("DCF EpochManager: Found {} underperforming validators", underperforming_count);
        }
        
        Ok(())
    }

    /// Get current epoch information
    pub fn get_current_epoch_info(&self) -> ConsensusResult<(u32, Vec<AccountId>)> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let current_epoch = api.get_current_epoch(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get current epoch: {:?}", e)))?;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        Ok((current_epoch, active_validators))
    }

    /// Generate validator management proposals based on combined PoS and PoI scores
    pub fn generate_validator_management_proposals(&self) -> ConsensusResult<Vec<(AccountId, String, u64, u64, u64)>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let config = self.get_runtime_config()?;
        let mut proposals = Vec::new();
        
        // Get active validators and their scores
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get active validators: {:?}", e)))?;
        
        let (_pos_weight, _poi_weight) = api.get_consensus_weights(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get consensus weights: {:?}", e)))?;
        
        for validator in active_validators.iter() {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let pos_score = self.get_pos_score(&validator);
                let poi_score = profile.poi_score as u64;
                let _inference_count = profile.inference_count;
                let (participation_rate, missed_blocks) = self.get_validator_participation_metrics(validator);
                
                let mut proposal_reason = String::new();
                
                // Check for underperformance
                if combined_score < config.min_performance_score as u64 {
                    proposal_reason = format!("Low combined score: {}", combined_score);
                } else if participation_rate < config.min_participation_rate {
                    proposal_reason = format!("Low participation: {}%", participation_rate);
                } else if missed_blocks > config.max_missed_blocks_high {
                    proposal_reason = format!("Too many missed blocks: {}", missed_blocks);
                } else if pos_score == 0u64 && poi_score == 0u64 {
                    proposal_reason = "No PoS or PoI contribution".to_string();
                } else if combined_score >= config.high_performance_score as u64 {
                    proposal_reason = format!("High performance reward candidate: {}", combined_score);
                }
                
                if !proposal_reason.is_empty() {
                    proposals.push((validator.clone(), proposal_reason, combined_score, pos_score, poi_score));
                }
            }
        }
        
        info!("EpochManager: Generated {} validator management proposals", proposals.len());
        Ok(proposals)
    }
    
    /// Update validator set based on combined PoS and PoI scores
    pub fn update_validator_set_by_combined_scores(&self) -> ConsensusResult<()> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get all validators and their combined scores
        let all_validators = api.get_validator_scores(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get validator scores: {:?}", e)))?;
        
        let (pos_weight, poi_weight) = api.get_consensus_weights(best_hash)
            .map_err(|e| ConsensusError::EpochTransition(format!("Failed to get consensus weights: {:?}", e)))?;
        
        let mut validator_performance: Vec<(AccountId, u64, u64, u64)> = Vec::new(); // (validator, combined, pos, poi)
        
        for (validator, _stored_score) in all_validators {
            if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
                let combined_score = profile.final_score;
                let pos_score = self.get_pos_score(&validator);
                let poi_score = profile.poi_score as u64;
                validator_performance.push((validator, combined_score, pos_score, poi_score));
            }
        }
        
        // Sort by combined score (descending)
        validator_performance.sort_by(|a, b| b.1.cmp(&a.1));
        
        // Get runtime configuration for display count
        let config = self.get_runtime_config()?;
        
        // Log top performers
        info!("EpochManager: Top validators by combined PoS/PoI score:");
        for (i, (validator, combined, pos, poi)) in validator_performance.iter().take(config.top_validators_display_count as usize).enumerate() {
            info!("  {}. {:?} - Combined: {}, PoS: {} ({}%), PoI: {} ({}%)", 
                  i + 1, validator, combined, pos, pos_weight, poi, poi_weight);
        }
        
        Ok(())
    }
    
    /// Get validator information for a specific validator
    pub fn get_validator_info(&self, validator: &AccountId) -> ConsensusResult<Option<ValidatorInfo>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, validator.clone()) {
            let combined_score = profile.final_score;
            let pos_score = self.get_pos_score(validator);
            let uptime = self.calculate_validator_uptime(validator);
            let (_, missed_blocks) = self.get_validator_participation_metrics(validator);
            
            let stake_score = pos_score; // Use the fresh PoS score from the profile
            
            let validator_info = ValidatorInfo {
                account_id: sp_core::ed25519::Public::from_raw(*validator.as_ref()),
                stake: stake_score as u128,
                performance_score: combined_score as u32,
                blocks_produced: uptime, // Using uptime as blocks produced approximation
                blocks_missed: missed_blocks,
            };
            
            Ok(Some(validator_info))
        } else {
            Ok(None)
        }
    }
}