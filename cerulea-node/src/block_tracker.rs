//! Block Authoring and Missed Block Tracking Service
//! 
//! This module provides comprehensive tracking of block authoring and missed blocks
//! for the CBC consensus system. It monitors validator performance and provides
//! real-time statistics and alerts.

use std::{sync::Arc, time::Duration, collections::HashMap};
use log::{debug, info, warn, error};
use sp_runtime::traits::{Block as BlockTrait, SaturatedConversion};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use tokio::time::sleep;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use cerulea_runtime::AccountId;

/// Statistics for a single validator's block authoring performance
#[derive(Debug, Clone, Default)]
pub struct ValidatorBlockStats {
    pub authored_blocks: u32,
    pub missed_blocks: u32,
    pub expected_blocks: u32,
    pub participation_rate: f64, // Percentage (0.0 to 100.0)
    pub last_authored_block: Option<u32>,
    pub consecutive_misses: u32,
}

impl ValidatorBlockStats {
    /// Calculate participation rate as a percentage
    pub fn calculate_participation_rate(&mut self) {
        if self.expected_blocks > 0 {
            self.participation_rate = (self.authored_blocks as f64 / self.expected_blocks as f64) * 100.0;
        } else {
            self.participation_rate = 100.0; // New validators start at 100%
        }
    }
    
    /// Check if validator is underperforming
    pub fn is_underperforming(&self, threshold: f64) -> bool {
        self.participation_rate < threshold && self.expected_blocks >= 10
    }
    
    /// Check if validator has concerning consecutive misses
    pub fn has_concerning_misses(&self, max_consecutive: u32) -> bool {
        self.consecutive_misses >= max_consecutive
    }
}

/// Block tracking service configuration
#[derive(Debug, Clone)]
pub struct BlockTrackerConfig {
    pub monitoring_interval: Duration,
    pub stats_reporting_interval: Duration,
    pub underperformance_threshold: f64, // Participation rate threshold (e.g., 85.0%)
    pub max_consecutive_misses: u32,     // Alert threshold for consecutive misses
    pub enable_alerts: bool,
    pub enable_detailed_logging: bool,
}

impl Default for BlockTrackerConfig {
    fn default() -> Self {
        Self {
            monitoring_interval: Duration::from_secs(30),
            stats_reporting_interval: Duration::from_secs(300), // 5 minutes
            underperformance_threshold: 85.0,
            max_consecutive_misses: 5,
            enable_alerts: true,
            enable_detailed_logging: false,
        }
    }
}

/// Block authoring and missed block tracking service
pub struct BlockTracker<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, u32>,
{
    client: Arc<C>,
    config: BlockTrackerConfig,
    validator_stats: HashMap<AccountId, ValidatorBlockStats>,
    last_processed_block: u32,
    total_blocks_tracked: u32,
    service_start_time: std::time::Instant,
    _phantom: std::marker::PhantomData<B>,
}

impl<B, C> BlockTracker<B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, u32>,
{
    /// Create a new block tracker instance
    pub fn new(client: Arc<C>, config: BlockTrackerConfig) -> Self {
        let current_block = client.info().best_number.saturated_into::<u32>();
        
        Self {
            client,
            config,
            validator_stats: HashMap::new(),
            last_processed_block: current_block,
            total_blocks_tracked: 0,
            service_start_time: std::time::Instant::now(),
            _phantom: std::marker::PhantomData,
        }
    }
    
    /// Start the block tracking service
    pub async fn run(&mut self) {
        info!("Block Tracker: Starting block authoring tracking service");
        
        let mut stats_report_timer = std::time::Instant::now();
        
        loop {
            // Update tracking data
            if let Err(e) = self.update_tracking_data().await {
                error!("Block Tracker: Failed to update tracking data: {:?}", e);
            }
            
            // Check for performance issues and alerts
            if self.config.enable_alerts {
                self.check_performance_alerts().await;
            }
            
            // Periodic statistics reporting
            if stats_report_timer.elapsed() >= self.config.stats_reporting_interval {
                self.report_statistics().await;
                stats_report_timer = std::time::Instant::now();
            }
            
            sleep(self.config.monitoring_interval).await;
        }
    }
    
    /// Update tracking data from runtime
    async fn update_tracking_data(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let current_block = self.client.info().best_number.saturated_into::<u32>();
        
        // Process any new blocks since last update
        if current_block > self.last_processed_block {
            let blocks_to_process = current_block - self.last_processed_block;
            
            if self.config.enable_detailed_logging {
                debug!("Block Tracker: Processing {} new blocks (#{} to #{})", 
                       blocks_to_process, self.last_processed_block + 1, current_block);
            }
            
            // Get active validators
            let active_validators = api.get_active_validators(best_hash)
                .map_err(|e| format!("Failed to get active validators: {:?}", e))?;
            
            // Update stats for each validator
            for validator in active_validators.iter() {
                self.update_validator_stats(validator).await?;
            }
            
            self.last_processed_block = current_block;
            self.total_blocks_tracked += blocks_to_process;
        }
        
        Ok(())
    }
    
    /// Update statistics for a specific validator
    async fn update_validator_stats(&mut self, validator: &AccountId) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get validator participation data from runtime
        let (authored_blocks, missed_blocks) = api.get_validator_participation(best_hash, validator.clone())
            .map_err(|e| format!("Failed to get validator participation: {:?}", e))?;
        
        // Get or create validator stats
        let stats = self.validator_stats.entry(validator.clone()).or_default();
        
        // Check if there are new authored or missed blocks
        let new_authored = authored_blocks.saturating_sub(stats.authored_blocks);
        let new_missed = missed_blocks.saturating_sub(stats.missed_blocks);
        
        if new_authored > 0 || new_missed > 0 {
            // Update stats
            let _old_authored = stats.authored_blocks;
            let _old_missed = stats.missed_blocks;
            
            stats.authored_blocks = authored_blocks;
            stats.missed_blocks = missed_blocks;
            stats.expected_blocks = authored_blocks + missed_blocks;
            
            // Update consecutive misses counter
            if new_authored > 0 {
                stats.consecutive_misses = 0;
                stats.last_authored_block = Some(self.last_processed_block);
            } else if new_missed > 0 {
                stats.consecutive_misses += new_missed;
            }
            
            // Recalculate participation rate
            stats.calculate_participation_rate();
            
            if self.config.enable_detailed_logging {
                debug!("Block Tracker: Updated stats for validator {:?} - Authored: {} (+{}), Missed: {} (+{}), Rate: {:.2}%", 
                       validator, authored_blocks, new_authored, missed_blocks, new_missed, stats.participation_rate);
            }
        }
        
        Ok(())
    }
    
    /// Check for performance alerts and warnings
    async fn check_performance_alerts(&self) {
        for (validator, stats) in self.validator_stats.iter() {
            // Check for underperformance
            if stats.is_underperforming(self.config.underperformance_threshold) {
                warn!("Block Tracker: Validator {:?} underperforming - Participation rate: {:.2}% (threshold: {:.2}%)", 
                      validator, stats.participation_rate, self.config.underperformance_threshold);
            }
            
            // Check for concerning consecutive misses
            if stats.has_concerning_misses(self.config.max_consecutive_misses) {
                warn!("Block Tracker: Validator {:?} has {} consecutive missed blocks (threshold: {})", 
                      validator, stats.consecutive_misses, self.config.max_consecutive_misses);
            }
        }
    }
    
    /// Report comprehensive statistics
    async fn report_statistics(&self) {
        let uptime = self.service_start_time.elapsed();
        let active_validators = self.validator_stats.len();
        
        info!("Block Tracker: Statistics Report");
        info!("  Service uptime: {:?}", uptime);
        info!("  Total blocks tracked: {}", self.total_blocks_tracked);
        info!("  Active validators monitored: {}", active_validators);
        
        if !self.validator_stats.is_empty() {
            // Calculate aggregate statistics
            let total_authored: u32 = self.validator_stats.values().map(|s| s.authored_blocks).sum();
            let total_missed: u32 = self.validator_stats.values().map(|s| s.missed_blocks).sum();
            let total_expected: u32 = self.validator_stats.values().map(|s| s.expected_blocks).sum();
            
            let overall_participation = if total_expected > 0 {
                (total_authored as f64 / total_expected as f64) * 100.0
            } else {
                100.0
            };
            
            info!("  Overall participation rate: {:.2}%", overall_participation);
            info!("  Total blocks authored: {}", total_authored);
            info!("  Total blocks missed: {}", total_missed);
            
            // Report top and bottom performers
            let mut sorted_validators: Vec<_> = self.validator_stats.iter().collect();
            sorted_validators.sort_by(|a, b| b.1.participation_rate.partial_cmp(&a.1.participation_rate).unwrap());
            
            info!("  Top performers:");
            for (validator, stats) in sorted_validators.iter().take(3) {
                info!("    {:?}: {:.2}% ({}/{} blocks)", 
                      validator, stats.participation_rate, stats.authored_blocks, stats.expected_blocks);
            }
            
            // Report underperformers
            let underperformers: Vec<_> = sorted_validators.iter()
                .filter(|(_, stats)| stats.is_underperforming(self.config.underperformance_threshold))
                .collect();
            
            if !underperformers.is_empty() {
                warn!("  Underperforming validators ({}):", underperformers.len());
                for (validator, stats) in underperformers.iter().take(5) {
                    warn!("    {:?}: {:.2}% ({}/{} blocks, {} consecutive misses)", 
                          validator, stats.participation_rate, stats.authored_blocks, 
                          stats.expected_blocks, stats.consecutive_misses);
                }
            }
        }
    }
    
    /// Get statistics for a specific validator
    #[allow(dead_code)]
    pub fn get_validator_stats(&self, validator: &AccountId) -> Option<&ValidatorBlockStats> {
        self.validator_stats.get(validator)
    }
    
    /// Get all validator statistics
    #[allow(dead_code)]
    pub fn get_all_stats(&self) -> &HashMap<AccountId, ValidatorBlockStats> {
        &self.validator_stats
    }
    
    /// Get service uptime
    #[allow(dead_code)]
    pub fn get_uptime(&self) -> Duration {
        self.service_start_time.elapsed()
    }
    
    /// Get total blocks tracked
    #[allow(dead_code)]
    pub fn get_total_blocks_tracked(&self) -> u32 {
        self.total_blocks_tracked
    }
}

/// Start the block tracking service
#[allow(dead_code)]
pub async fn start_block_tracker<B, C>(
    client: Arc<C>,
    config: Option<BlockTrackerConfig>,
) where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, u32>,
{
    let config = config.unwrap_or_default();
    let mut tracker = BlockTracker::<B, C>::new(client, config);
    
    info!("Starting CBC Block Tracker service");
    tracker.run().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validator_block_stats() {
        let mut stats = ValidatorBlockStats::default();
        
        // Test initial state
        assert_eq!(stats.authored_blocks, 0);
        assert_eq!(stats.missed_blocks, 0);
        assert_eq!(stats.participation_rate, 100.0);
        
        // Test with some blocks
        stats.authored_blocks = 8;
        stats.missed_blocks = 2;
        stats.expected_blocks = 10;
        stats.calculate_participation_rate();
        
        assert_eq!(stats.participation_rate, 80.0);
        assert!(stats.is_underperforming(85.0));
        assert!(!stats.is_underperforming(75.0));
        
        // Test consecutive misses
        stats.consecutive_misses = 6;
        assert!(stats.has_concerning_misses(5));
        assert!(!stats.has_concerning_misses(10));
    }
    
    #[test]
    fn test_block_tracker_config() {
        let config = BlockTrackerConfig::default();
        
        assert_eq!(config.underperformance_threshold, 85.0);
        assert_eq!(config.max_consecutive_misses, 5);
        assert!(config.enable_alerts);
    }
}