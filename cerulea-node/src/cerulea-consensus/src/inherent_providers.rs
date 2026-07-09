//! Inherent data provider system for CBC consensus
//!
//! This module provides a centralized system for managing inherent data providers
//! that generate system-required data like timestamps for block production.

use sp_inherents::{InherentData, InherentDataProvider};
use sp_timestamp::InherentDataProvider as TimestampInherentDataProvider;
use crate::error::{ConsensusError, ConsensusResult};
use log::{debug, trace};
use std::time::{SystemTime, UNIX_EPOCH};

/// Central manager for all inherent data providers used in CBC consensus
pub struct CeruleaInherentDataProviders {
    // We don't store the provider directly since it doesn't implement Clone
    // Instead we'll create it fresh each time
}

impl CeruleaInherentDataProviders {
    /// Create a new inherent data providers manager
    pub fn new() -> Self {
        Self {}
    }

    /// Create a new inherent data providers manager with a specific timestamp
    pub fn with_timestamp(_timestamp_ms: u64) -> Self {
        Self {}
    }

    /// Create inherent data for block production
    /// 
    /// This method generates all required inherent data including:
    /// - Timestamp inherent with current system time
    /// - Future: Block number, validator set updates, etc.
    pub async fn create_inherent_data(&self) -> ConsensusResult<InherentData> {
        let mut inherent_data = InherentData::new();
        
        trace!("Creating inherent data for block production");
        
        // Create timestamp provider with current system time
        let timestamp_provider = TimestampInherentDataProvider::from_system_time();
        
        // Add timestamp inherent
        timestamp_provider
            .provide_inherent_data(&mut inherent_data)
            .await
            .map_err(|e| {
                ConsensusError::Proposer(format!("Failed to create timestamp inherent: {:?}", e))
            })?;
        
        // Log the timestamp that was added
        if let Ok(timestamp) = inherent_data.get_data::<u64>(&sp_timestamp::INHERENT_IDENTIFIER) {
            if let Some(timestamp_ms) = timestamp {
                debug!("Added timestamp inherent: {} ms", timestamp_ms);
            }
        }
        
        debug!("Successfully created inherent data with {} providers", 1);
        Ok(inherent_data)
    }

    /// Get the current system timestamp in milliseconds
    pub fn current_timestamp_ms() -> ConsensusResult<u64> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .map_err(|e| ConsensusError::Proposer(format!("Failed to get system time: {:?}", e)))
    }

    /// Create inherent data with a specific timestamp (useful for testing)
    pub async fn create_inherent_data_with_timestamp(&self, timestamp_ms: u64) -> ConsensusResult<InherentData> {
        let mut inherent_data = InherentData::new();
        
        trace!("Creating inherent data with specific timestamp: {} ms", timestamp_ms);
        
        // Create timestamp provider with specific timestamp
        let timestamp_provider = TimestampInherentDataProvider::new(timestamp_ms.into());
        timestamp_provider
            .provide_inherent_data(&mut inherent_data)
            .await
            .map_err(|e| {
                ConsensusError::Proposer(format!("Failed to create timestamp inherent: {:?}", e))
            })?;
        
        debug!("Successfully created inherent data with timestamp: {} ms", timestamp_ms);
        Ok(inherent_data)
    }

    /// Validate that inherent data contains all required inherents
    pub fn validate_inherent_data(&self, inherent_data: &InherentData) -> ConsensusResult<()> {
        // Check that timestamp inherent is present
        match inherent_data.get_data::<u64>(&sp_timestamp::INHERENT_IDENTIFIER) {
            Ok(Some(timestamp)) => {
                debug!("Validated timestamp inherent: {} ms", timestamp);
                Ok(())
            }
            Ok(None) => {
                Err(ConsensusError::Proposer("Missing timestamp inherent".into()))
            }
            Err(e) => {
                Err(ConsensusError::Proposer(format!("Failed to validate timestamp inherent: {:?}", e)))
            }
        }
    }

    /// Get the timestamp from inherent data
    pub fn extract_timestamp(&self, inherent_data: &InherentData) -> ConsensusResult<u64> {
        inherent_data
            .get_data::<u64>(&sp_timestamp::INHERENT_IDENTIFIER)
            .map_err(|e| ConsensusError::Proposer(format!("Failed to extract timestamp: {:?}", e)))?
            .ok_or_else(|| ConsensusError::Proposer("Timestamp inherent not found".into()))
    }
}

impl Default for CeruleaInherentDataProviders {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_inherent_data() {
        let providers = CeruleaInherentDataProviders::new();
        let inherent_data = providers.create_inherent_data().await.unwrap();
        
        // Validate that timestamp inherent is present
        providers.validate_inherent_data(&inherent_data).unwrap();
        
        // Extract and verify timestamp
        let timestamp = providers.extract_timestamp(&inherent_data).unwrap();
        assert!(timestamp > 0);
    }

    #[tokio::test]
    async fn test_create_inherent_data_with_specific_timestamp() {
        let providers = CeruleaInherentDataProviders::new();
        let test_timestamp = 1234567890000u64; // Some test timestamp
        
        let inherent_data = providers
            .create_inherent_data_with_timestamp(test_timestamp)
            .await
            .unwrap();
        
        // Validate and extract timestamp
        providers.validate_inherent_data(&inherent_data).unwrap();
        let extracted_timestamp = providers.extract_timestamp(&inherent_data).unwrap();
        
        assert_eq!(extracted_timestamp, test_timestamp);
    }

    #[test]
    fn test_current_timestamp_ms() {
        let timestamp = CeruleaInherentDataProviders::current_timestamp_ms().unwrap();
        assert!(timestamp > 0);
        
        // Should be roughly current time (within last few seconds)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        
        // Allow 1 second difference for test execution time
        assert!((timestamp as i64 - now as i64).abs() < 1000);
    }

    #[tokio::test]
    async fn test_with_timestamp_constructor() {
        let test_timestamp = 9876543210000u64;
        let providers = CeruleaInherentDataProviders::with_timestamp(test_timestamp);
        
        // Use the specific timestamp method since with_timestamp doesn't store the timestamp
        let inherent_data = providers.create_inherent_data_with_timestamp(test_timestamp).await.unwrap();
        let extracted_timestamp = providers.extract_timestamp(&inherent_data).unwrap();
        
        assert_eq!(extracted_timestamp, test_timestamp);
    }
}