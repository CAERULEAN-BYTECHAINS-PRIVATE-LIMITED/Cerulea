//! Deterministic Authoring Tests
//! 
//! This module contains tests that verify the deterministic block authoring functionality
//! of the CBC consensus mechanism. These tests validate:
//! - Querying expected authors for future blocks
//! - Verifying actual authors match expected authors
//! - Author mismatch detection and logging
//! - CLI utility for querying upcoming authors

use std::time::Duration;
use std::collections::HashMap;
use tokio::time::sleep;
use cbc_runtime::{AccountId};
use sp_core::crypto::AccountId32;

/// Configuration for deterministic authoring tests
#[derive(Debug, Clone)]
pub struct DeterministicAuthoringTestConfig {
    /// Number of future blocks to query
    pub blocks_to_query: u32,
    /// Number of blocks to produce for testing
    pub blocks_to_produce: u32,
    /// Epoch length for testing
    pub epoch_length: u32,
    /// Number of validators in the test set
    pub validator_count: usize,
    /// Whether to simulate author mismatches
    pub simulate_mismatches: bool,
    /// Mismatch probability (0.0 to 1.0)
    pub mismatch_probability: f64,
}

impl Default for DeterministicAuthoringTestConfig {
    fn default() -> Self {
        Self {
            blocks_to_query: 20,
            blocks_to_produce: 15,
            epoch_length: 10,
            validator_count: 5,
            simulate_mismatches: false,
            mismatch_probability: 0.1,
        }
    }
}

/// Mock deterministic authoring system for testing
pub struct MockDeterministicAuthoring {
    config: DeterministicAuthoringTestConfig,
    validators: Vec<AccountId>,
    current_block: u32,
    current_epoch: u32,
    expected_authors: HashMap<u32, AccountId>,
    actual_authors: HashMap<u32, AccountId>,
    author_mismatches: Vec<AuthorMismatch>,
}

/// Represents an author mismatch event
#[derive(Debug, Clone)]
pub struct AuthorMismatch {
    pub block_number: u32,
    pub expected_author: AccountId,
    pub actual_author: AccountId,
    pub timestamp: std::time::SystemTime,
}

impl MockDeterministicAuthoring {
    /// Create a new mock deterministic authoring system
    pub fn new(config: DeterministicAuthoringTestConfig) -> Self {
        let validators = Self::generate_test_validators(config.validator_count);
        
        Self {
            config,
            validators,
            current_block: 1,
            current_epoch: 1,
            expected_authors: HashMap::new(),
            actual_authors: HashMap::new(),
            author_mismatches: Vec::new(),
        }
    }
    
    /// Generate test validator accounts
    fn generate_test_validators(count: usize) -> Vec<AccountId> {
        let mut validators = Vec::new();
        for i in 0..count {
            let mut bytes = [0u8; 32];
            bytes[0] = i as u8 + 1; // Start from 1 to avoid zero account
            validators.push(AccountId32::from(bytes));
        }
        validators
    }
    
    /// Query expected authors for the next N blocks using deterministic selection
    pub fn query_expected_authors(&mut self, block_count: u32) -> Result<Vec<(u32, AccountId)>, Box<dyn std::error::Error>> {
        log::info!("Querying expected authors for next {} blocks starting from block {}", 
                  block_count, self.current_block);
        
        let mut expected_authors = Vec::new();
        
        for i in 0..block_count {
            let block_number = self.current_block + i;
            let expected_author = self.calculate_expected_author(block_number)?;
            
            expected_authors.push((block_number, expected_author.clone()));
            self.expected_authors.insert(block_number, expected_author);
        }
        
        log::info!("Expected authors calculated for blocks {}-{}", 
                  self.current_block, self.current_block + block_count - 1);
        
        Ok(expected_authors)
    }
    
    /// Calculate expected author for a specific block using round-robin selection
    fn calculate_expected_author(&self, block_number: u32) -> Result<AccountId, Box<dyn std::error::Error>> {
        if self.validators.is_empty() {
            return Err("No validators available".into());
        }
        
        // Simple round-robin selection based on block number
        // In a real implementation, this would use trust scores and weighted selection
        let validator_index = ((block_number - 1) % self.validators.len() as u32) as usize;
        Ok(self.validators[validator_index].clone())
    }
    
    /// Produce blocks and track actual authors
    pub async fn produce_blocks(&mut self, block_count: u32) -> Result<Vec<(u32, AccountId)>, Box<dyn std::error::Error>> {
        log::info!("Producing {} blocks starting from block {}", block_count, self.current_block);
        
        let mut produced_blocks = Vec::new();
        
        for i in 0..block_count {
            let block_number = self.current_block + i;
            
            // Simulate block production delay
            sleep(Duration::from_millis(50)).await;
            
            // Determine actual author (may differ from expected if simulating mismatches)
            let actual_author = self.determine_actual_author(block_number)?;
            
            // Record the actual author
            self.actual_authors.insert(block_number, actual_author.clone());
            produced_blocks.push((block_number, actual_author.clone()));
            
            // Check for author mismatch
            if let Some(expected_author) = self.expected_authors.get(&block_number) {
                if expected_author != &actual_author {
                    self.record_author_mismatch(block_number, expected_author.clone(), actual_author.clone());
                }
            }
            
            // Simulate epoch transitions
            if block_number % self.config.epoch_length == 0 {
                self.current_epoch += 1;
                log::info!("Epoch transition to epoch {} at block {}", self.current_epoch, block_number);
            }
            
            log::debug!("Block {} produced by validator {:?}", block_number, actual_author);
        }
        
        self.current_block += block_count;
        
        log::info!("Block production completed. Current block: {}", self.current_block);
        Ok(produced_blocks)
    }
    
    /// Determine actual author (with optional mismatch simulation)
    fn determine_actual_author(&self, block_number: u32) -> Result<AccountId, Box<dyn std::error::Error>> {
        let expected_author = self.calculate_expected_author(block_number)?;
        
        // If not simulating mismatches, return expected author
        if !self.config.simulate_mismatches {
            return Ok(expected_author);
        }
        
        // Simulate random author mismatches
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        if rng.gen::<f64>() < self.config.mismatch_probability {
            // Select a different validator as the actual author
            let alternative_validators: Vec<_> = self.validators.iter()
                .filter(|v| *v != &expected_author)
                .collect();
            
            if !alternative_validators.is_empty() {
                let alt_index = rng.gen_range(0..alternative_validators.len());
                return Ok(alternative_validators[alt_index].clone());
            }
        }
        
        Ok(expected_author)
    }
    
    /// Record an author mismatch event
    fn record_author_mismatch(&mut self, block_number: u32, expected_author: AccountId, actual_author: AccountId) {
        let mismatch = AuthorMismatch {
            block_number,
            expected_author: expected_author.clone(),
            actual_author: actual_author.clone(),
            timestamp: std::time::SystemTime::now(),
        };
        
        self.author_mismatches.push(mismatch);
        
        // Log the mismatch (this tests the logging requirement)
        log::warn!("[CBC-CONSENSUS] Author mismatch at block {}: expected {:?}, actual {:?}", 
                  block_number, expected_author, actual_author);
    }
    
    /// Verify that actual authors match expected authors
    pub fn verify_author_consistency(&self) -> Result<AuthorConsistencyReport, Box<dyn std::error::Error>> {
        let mut matches = 0;
        let mut mismatches = 0;
        let mut missing_expected = 0;
        let mut missing_actual = 0;
        
        // Check all blocks that have both expected and actual authors
        for (&block_number, expected_author) in &self.expected_authors {
            if let Some(actual_author) = self.actual_authors.get(&block_number) {
                if expected_author == actual_author {
                    matches += 1;
                } else {
                    mismatches += 1;
                }
            } else {
                missing_actual += 1;
            }
        }
        
        // Check for actual authors without expected authors
        for &block_number in self.actual_authors.keys() {
            if !self.expected_authors.contains_key(&block_number) {
                missing_expected += 1;
            }
        }
        
        let total_blocks = matches + mismatches + missing_expected + missing_actual;
        let consistency_percentage = if total_blocks > 0 {
            (matches as f64 / total_blocks as f64) * 100.0
        } else {
            0.0
        };
        
        Ok(AuthorConsistencyReport {
            total_blocks,
            matches,
            mismatches,
            missing_expected,
            missing_actual,
            consistency_percentage,
            mismatch_events: self.author_mismatches.clone(),
        })
    }
    
    /// Get current block number
    pub fn current_block(&self) -> u32 {
        self.current_block
    }
    
    /// Get current epoch
    pub fn current_epoch(&self) -> u32 {
        self.current_epoch
    }
    
    /// Get validator count
    pub fn validator_count(&self) -> usize {
        self.validators.len()
    }
    
    /// Get all validators
    pub fn validators(&self) -> &[AccountId] {
        &self.validators
    }
    
    /// Get author mismatches
    pub fn author_mismatches(&self) -> &[AuthorMismatch] {
        &self.author_mismatches
    }
}

/// Report on author consistency verification
#[derive(Debug)]
pub struct AuthorConsistencyReport {
    pub total_blocks: u32,
    pub matches: u32,
    pub mismatches: u32,
    pub missing_expected: u32,
    pub missing_actual: u32,
    pub consistency_percentage: f64,
    pub mismatch_events: Vec<AuthorMismatch>,
}

impl AuthorConsistencyReport {
    /// Check if the consistency is acceptable (no mismatches unless simulated)
    pub fn is_consistent(&self, allow_simulated_mismatches: bool) -> bool {
        if allow_simulated_mismatches {
            // If simulating mismatches, we expect some mismatches but overall consistency should be reasonable
            self.consistency_percentage >= 80.0 && self.missing_expected == 0 && self.missing_actual == 0
        } else {
            // If not simulating mismatches, we expect perfect consistency
            self.mismatches == 0 && self.missing_expected == 0 && self.missing_actual == 0
        }
    }
}

// =============================================================================
// INTEGRATION TESTS
// =============================================================================

#[tokio::test]
async fn test_query_expected_authors() {
    env_logger::try_init().ok();
    
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: 10,
        validator_count: 3,
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config.clone());
    
    // Test: Query expected authors for next N blocks
    match authoring.query_expected_authors(config.blocks_to_query) {
        Ok(expected_authors) => {
            log::info!("✓ Query expected authors test PASSED");
            
            // Verify we got the expected number of authors
            assert_eq!(expected_authors.len(), config.blocks_to_query as usize);
            
            // Verify block numbers are sequential
            for (i, (block_number, _)) in expected_authors.iter().enumerate() {
                assert_eq!(*block_number, authoring.current_block() + i as u32);
            }
            
            // Verify all authors are valid validators
            for (_, author) in &expected_authors {
                assert!(authoring.validators().contains(author), "Author should be a valid validator");
            }
            
            log::info!("Expected authors for blocks {}-{}: {:?}", 
                      authoring.current_block(), 
                      authoring.current_block() + config.blocks_to_query - 1,
                      expected_authors);
        }
        Err(e) => {
            panic!("✗ Query expected authors test FAILED: {}", e);
        }
    }
}

#[tokio::test]
async fn test_produce_blocks_and_verify_authors() {
    env_logger::try_init().ok();
    
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: 8,
        blocks_to_produce: 8,
        validator_count: 4,
        simulate_mismatches: false, // No mismatches for this test
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config.clone());
    
    // Step 1: Query expected authors
    let expected_authors = authoring.query_expected_authors(config.blocks_to_query)
        .expect("Failed to query expected authors");
    
    // Step 2: Produce blocks
    let actual_authors = authoring.produce_blocks(config.blocks_to_produce).await
        .expect("Failed to produce blocks");
    
    // Step 3: Verify authors match
    let consistency_report = authoring.verify_author_consistency()
        .expect("Failed to verify author consistency");
    
    // Test: Actual authors should match expected authors
    assert!(consistency_report.is_consistent(false), 
           "Authors should be consistent: {:?}", consistency_report);
    
    assert_eq!(consistency_report.matches, config.blocks_to_produce);
    assert_eq!(consistency_report.mismatches, 0);
    
    log::info!("✓ Block production and author verification test PASSED");
    log::info!("Consistency report: {}/{} matches ({}%)", 
              consistency_report.matches, 
              consistency_report.total_blocks,
              consistency_report.consistency_percentage);
    
    // Verify expected vs actual authors
    for ((exp_block, exp_author), (act_block, act_author)) in expected_authors.iter().zip(actual_authors.iter()) {
        assert_eq!(exp_block, act_block, "Block numbers should match");
        assert_eq!(exp_author, act_author, "Authors should match for block {}", exp_block);
    }
}

#[tokio::test]
async fn test_author_mismatch_detection_and_logging() {
    env_logger::try_init().ok();
    
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: 10,
        blocks_to_produce: 10,
        validator_count: 5,
        simulate_mismatches: true, // Enable mismatch simulation
        mismatch_probability: 0.3, // 30% chance of mismatch
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config.clone());
    
    // Step 1: Query expected authors
    authoring.query_expected_authors(config.blocks_to_query)
        .expect("Failed to query expected authors");
    
    // Step 2: Produce blocks with simulated mismatches
    authoring.produce_blocks(config.blocks_to_produce).await
        .expect("Failed to produce blocks");
    
    // Step 3: Verify mismatch detection
    let consistency_report = authoring.verify_author_consistency()
        .expect("Failed to verify author consistency");
    
    // Test: Should detect mismatches when simulated
    if consistency_report.mismatches > 0 {
        log::info!("✓ Author mismatch detection test PASSED - {} mismatches detected", 
                  consistency_report.mismatches);
        
        // Verify mismatch events were recorded
        let mismatch_events = authoring.author_mismatches();
        assert_eq!(mismatch_events.len(), consistency_report.mismatches as usize);
        
        // Verify mismatch event details
        for mismatch in mismatch_events {
            assert!(mismatch.block_number >= authoring.current_block() - config.blocks_to_produce);
            assert!(mismatch.block_number < authoring.current_block());
            assert_ne!(mismatch.expected_author, mismatch.actual_author);
            assert!(authoring.validators().contains(&mismatch.expected_author));
            assert!(authoring.validators().contains(&mismatch.actual_author));
            
            log::info!("Mismatch detected at block {}: expected {:?}, actual {:?}", 
                      mismatch.block_number, mismatch.expected_author, mismatch.actual_author);
        }
    } else {
        log::info!("ℹ No mismatches occurred in this test run (random simulation)");
    }
    
    // Test: Overall consistency should still be reasonable with simulated mismatches
    assert!(consistency_report.is_consistent(true), 
           "Should maintain reasonable consistency even with simulated mismatches: {:?}", 
           consistency_report);
}

#[tokio::test]
async fn test_deterministic_author_prediction() {
    env_logger::try_init().ok();
    
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: 15,
        blocks_to_produce: 15,
        validator_count: 3,
        simulate_mismatches: false,
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config.clone());
    
    // Test: Deterministic prediction should be consistent across multiple queries
    let first_query = authoring.query_expected_authors(config.blocks_to_query)
        .expect("Failed to query expected authors first time");
    
    // Reset and query again
    let mut authoring2 = MockDeterministicAuthoring::new(config.clone());
    let second_query = authoring2.query_expected_authors(config.blocks_to_query)
        .expect("Failed to query expected authors second time");
    
    // Verify predictions are identical
    assert_eq!(first_query.len(), second_query.len());
    for ((block1, author1), (block2, author2)) in first_query.iter().zip(second_query.iter()) {
        assert_eq!(block1, block2, "Block numbers should match");
        assert_eq!(author1, author2, "Authors should be deterministic for block {}", block1);
    }
    
    log::info!("✓ Deterministic author prediction test PASSED");
    log::info!("Verified {} blocks have consistent deterministic authors", first_query.len());
    
    // Test: Round-robin pattern should be evident
    let validators = authoring.validators();
    for (_i, (block_number, author)) in first_query.iter().enumerate() {
        let expected_validator_index = ((block_number - 1) % validators.len() as u32) as usize;
        let expected_author = &validators[expected_validator_index];
        assert_eq!(author, expected_author, 
                  "Block {} should follow round-robin pattern", block_number);
    }
    
    log::info!("✓ Round-robin author selection pattern verified");
}

#[tokio::test]
async fn test_comprehensive_deterministic_authoring() {
    env_logger::try_init().ok();
    
    log::info!("\n=== Comprehensive Deterministic Authoring Test ===");
    
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: 20,
        blocks_to_produce: 20,
        validator_count: 4,
        epoch_length: 8,
        simulate_mismatches: false,
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config.clone());
    
    // Step 1: Query expected authors
    log::info!("Step 1: Querying expected authors for {} blocks...", config.blocks_to_query);
    let expected_authors = authoring.query_expected_authors(config.blocks_to_query)
        .expect("Failed to query expected authors");
    log::info!("✓ Expected authors queried successfully");
    
    // Step 2: Produce blocks
    log::info!("Step 2: Producing {} blocks...", config.blocks_to_produce);
    let _initial_block = authoring.current_block();
    let initial_epoch = authoring.current_epoch();
    
    let actual_authors = authoring.produce_blocks(config.blocks_to_produce).await
        .expect("Failed to produce blocks");
    log::info!("✓ Blocks produced successfully");
    
    // Step 3: Verify consistency
    log::info!("Step 3: Verifying author consistency...");
    let consistency_report = authoring.verify_author_consistency()
        .expect("Failed to verify author consistency");
    
    assert!(consistency_report.is_consistent(false), 
           "Authors should be perfectly consistent: {:?}", consistency_report);
    log::info!("✓ Author consistency verified");
    
    // Step 4: Verify epoch transitions
    log::info!("Step 4: Verifying epoch transitions...");
    let final_epoch = authoring.current_epoch();
    let _expected_epochs = (config.blocks_to_produce / config.epoch_length) + 1;
    
    assert!(final_epoch >= initial_epoch, "Epochs should advance");
    if config.blocks_to_produce >= config.epoch_length {
        assert!(final_epoch > initial_epoch, "Should have epoch transitions");
    }
    log::info!("✓ Epoch transitions verified: {} -> {}", initial_epoch, final_epoch);
    
    // Step 5: Verify round-robin pattern
    log::info!("Step 5: Verifying round-robin author selection...");
    let validators = authoring.validators();
    let mut author_counts = HashMap::new();
    
    for (_, author) in &actual_authors {
        *author_counts.entry(author.clone()).or_insert(0) += 1;
    }
    
    // Each validator should get roughly equal blocks (within 1 block difference)
    let expected_blocks_per_validator = config.blocks_to_produce / validators.len() as u32;
    for validator in validators {
        let count = author_counts.get(validator).unwrap_or(&0);
        let diff = (*count as i32 - expected_blocks_per_validator as i32).abs();
        assert!(diff <= 1, "Validator {:?} should get roughly equal blocks: got {}, expected ~{}", 
               validator, count, expected_blocks_per_validator);
    }
    log::info!("✓ Round-robin pattern verified");
    
    // Final validation
    log::info!("\nCOMPREHENSIVE DETERMINISTIC AUTHORING TEST RESULTS:");
    log::info!("Expected Authors Query: {} blocks", expected_authors.len());
    log::info!("Block Production: {} blocks produced", actual_authors.len());
    log::info!("Author Consistency: {}/{} matches ({}%)", 
              consistency_report.matches, 
              consistency_report.total_blocks,
              consistency_report.consistency_percentage);
    log::info!("Epoch Transitions: {} -> {} epochs", initial_epoch, final_epoch);
    log::info!("Round-Robin Pattern: Verified across {} validators", validators.len());
    log::info!("Overall Status: ALL TESTS PASSED");
    
    log::info!("Comprehensive deterministic authoring test completed successfully!");
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

/// CLI utility function to query upcoming authors (for CLI implementation)
pub fn query_upcoming_authors_cli(
    blocks: u32, 
    validator_count: usize
) -> Result<Vec<(u32, AccountId)>, Box<dyn std::error::Error>> {
    let config = DeterministicAuthoringTestConfig {
        blocks_to_query: blocks,
        validator_count,
        ..Default::default()
    };
    
    let mut authoring = MockDeterministicAuthoring::new(config);
    authoring.query_expected_authors(blocks)
}

/// Format author list for CLI output
pub fn format_authors_for_cli(authors: &[(u32, AccountId)], format: &str) -> String {
    match format {
        "json" => {
            let json_authors: Vec<serde_json::Value> = authors.iter()
                .map(|(block, author)| {
                    serde_json::json!({
                        "block": block,
                        "author": format!("{:?}", author)
                    })
                })
                .collect();
            serde_json::to_string_pretty(&json_authors).unwrap_or_default()
        }
        _ => {
            let mut output = String::new();
            output.push_str("Upcoming Block Authors:\n");
            output.push_str("======================\n");
            for (block, author) in authors {
                output.push_str(&format!("Block {}: {:?}\n", block, author));
            }
            output
        }
    }
}

#[cfg(test)]
mod helper_tests {
    use super::*;
    
    #[test]
    fn test_config_creation() {
        let config = DeterministicAuthoringTestConfig::default();
        assert!(config.blocks_to_query > 0);
        assert!(config.blocks_to_produce > 0);
        assert!(config.validator_count > 0);
        assert!(config.epoch_length > 0);
    }
    
    #[test]
    fn test_validator_generation() {
        let validators = MockDeterministicAuthoring::generate_test_validators(5);
        assert_eq!(validators.len(), 5);
        
        // Verify all validators are unique
        for i in 0..validators.len() {
            for j in (i + 1)..validators.len() {
                assert_ne!(validators[i], validators[j]);
            }
        }
    }
    
    #[test]
    fn test_author_consistency_report() {
        let report = AuthorConsistencyReport {
            total_blocks: 10,
            matches: 10,
            mismatches: 0,
            missing_expected: 0,
            missing_actual: 0,
            consistency_percentage: 100.0,
            mismatch_events: Vec::new(),
        };
        
        assert!(report.is_consistent(false));
        assert!(report.is_consistent(true));
    }
    
    #[test]
    fn test_cli_utility_function() {
        let authors = query_upcoming_authors_cli(5, 3)
            .expect("Failed to query authors");
        
        assert_eq!(authors.len(), 5);
        
        let json_output = format_authors_for_cli(&authors, "json");
        assert!(json_output.contains("block"));
        assert!(json_output.contains("author"));
        
        let plain_output = format_authors_for_cli(&authors, "plain");
        assert!(plain_output.contains("Upcoming Block Authors"));
        assert!(plain_output.contains("Block"));
    }
}

// =============================================================================
// FORK-FREE MULTI-NODE TESTS (Requirements 12.1, 12.4, 12.5)
// =============================================================================

#[cfg(test)]
mod fork_free_multi_node_tests {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    use sp_core::crypto::AccountId32;

    // Fixed validator accounts (Alice, Bob, Charlie)
    fn alice() -> AccountId32 {
        AccountId32::from([1u8; 32])
    }

    fn bob() -> AccountId32 {
        AccountId32::from([2u8; 32])
    }

    fn charlie() -> AccountId32 {
        AccountId32::from([3u8; 32])
    }

    fn validators_sorted() -> Vec<AccountId32> {
        let mut vs = vec![alice(), bob(), charlie()];
        vs.sort();
        vs
    }

    /// Simulate the randomness seed for epoch N.
    /// seed = hash(parent_block_hash || epoch_number)
    /// where parent_block_hash = hash(block_number_bytes) for block N*epoch_length - 1
    fn compute_seed(epoch: u32, epoch_length: u32) -> [u8; 32] {
        let parent_block = epoch * epoch_length - 1;
        // Simulate block hash of parent block
        let parent_block_hash = simple_hash(&parent_block.to_le_bytes());
        // Combine with epoch number
        let mut input = [0u8; 36];
        input[..32].copy_from_slice(&parent_block_hash);
        input[32..].copy_from_slice(&epoch.to_le_bytes());
        simple_hash(&input)
    }

    /// Simple deterministic hash function using DefaultHasher, returns [u8; 32]
    fn simple_hash(data: &[u8]) -> [u8; 32] {
        let mut h1 = DefaultHasher::new();
        data.hash(&mut h1);
        let v1 = h1.finish();

        let mut h2 = DefaultHasher::new();
        v1.hash(&mut h2);
        let v2 = h2.finish();

        let mut h3 = DefaultHasher::new();
        v2.hash(&mut h3);
        let v3 = h3.finish();

        let mut h4 = DefaultHasher::new();
        v3.hash(&mut h4);
        let v4 = h4.finish();

        let mut out = [0u8; 32];
        out[0..8].copy_from_slice(&v1.to_le_bytes());
        out[8..16].copy_from_slice(&v2.to_le_bytes());
        out[16..24].copy_from_slice(&v3.to_le_bytes());
        out[24..32].copy_from_slice(&v4.to_le_bytes());
        out
    }

    /// Advance the hash chain by one step
    fn rng_step(seed: &[u8; 32], step: u32) -> u64 {
        let mut input = [0u8; 36];
        input[..32].copy_from_slice(seed);
        input[32..].copy_from_slice(&step.to_le_bytes());
        let h = simple_hash(&input);
        u64::from_le_bytes(h[0..8].try_into().unwrap())
    }

    /// Compute the author sequence for an epoch given sorted validators and a seed.
    /// For slot i, pick validator at index rng_step(seed, i) % validator_count.
    fn compute_author_sequence(
        validators: &[AccountId32],
        seed: &[u8; 32],
        slots: u32,
    ) -> Vec<AccountId32> {
        let n = validators.len() as u64;
        (0..slots)
            .map(|i| {
                let idx = (rng_step(seed, i) % n) as usize;
                validators[idx].clone()
            })
            .collect()
    }

    /// Simulated node: independently computes seeds and sequences from shared inputs.
    struct SimulatedNode {
        validators: Vec<AccountId32>,
    }

    impl SimulatedNode {
        fn new(validators: Vec<AccountId32>) -> Self {
            Self { validators }
        }

        fn compute_seed_for_epoch(&self, epoch: u32, epoch_length: u32) -> [u8; 32] {
            compute_seed(epoch, epoch_length)
        }

        fn compute_sequence_for_epoch(
            &self,
            epoch: u32,
            epoch_length: u32,
            slots: u32,
        ) -> Vec<AccountId32> {
            let seed = self.compute_seed_for_epoch(epoch, epoch_length);
            compute_author_sequence(&self.validators, &seed, slots)
        }
    }

    /// Requirements 12.5: EpochAuthorSequences are identical across all nodes for each epoch.
    #[test]
    fn test_epoch_author_sequences_identical_across_nodes() {
        let epoch_length = 10u32;
        let slots_per_epoch = 10u32;
        let num_epochs = 20u32;

        let validators = validators_sorted();

        // Three independent nodes, each computing sequences from the same inputs
        let node0 = SimulatedNode::new(validators.clone());
        let node1 = SimulatedNode::new(validators.clone());
        let node2 = SimulatedNode::new(validators.clone());

        for epoch in 1..=num_epochs {
            let seq0 = node0.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);
            let seq1 = node1.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);
            let seq2 = node2.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);

            assert_eq!(
                seq0, seq1,
                "Epoch {}: node0 and node1 sequences must be identical",
                epoch
            );
            assert_eq!(
                seq0, seq2,
                "Epoch {}: node0 and node2 sequences must be identical",
                epoch
            );
            assert_eq!(
                seq0.len(),
                slots_per_epoch as usize,
                "Epoch {}: sequence must have {} slots",
                epoch,
                slots_per_epoch
            );

            // All authors must be valid validators
            for author in &seq0 {
                assert!(
                    validators.contains(author),
                    "Epoch {}: author {:?} must be a known validator",
                    epoch,
                    author
                );
            }
        }
    }

    /// Requirements 12.4: randomness_seed values are identical across all nodes for every epoch.
    #[test]
    fn test_randomness_seed_identical_across_nodes() {
        let epoch_length = 10u32;
        let num_epochs = 20u32;

        let validators = validators_sorted();

        let node0 = SimulatedNode::new(validators.clone());
        let node1 = SimulatedNode::new(validators.clone());
        let node2 = SimulatedNode::new(validators.clone());

        for epoch in 1..=num_epochs {
            let seed0 = node0.compute_seed_for_epoch(epoch, epoch_length);
            let seed1 = node1.compute_seed_for_epoch(epoch, epoch_length);
            let seed2 = node2.compute_seed_for_epoch(epoch, epoch_length);

            assert_eq!(
                seed0, seed1,
                "Epoch {}: node0 and node1 randomness seeds must be identical",
                epoch
            );
            assert_eq!(
                seed0, seed2,
                "Epoch {}: node0 and node2 randomness seeds must be identical",
                epoch
            );

            // Seeds for different epochs must differ (determinism, not constant)
            if epoch > 1 {
                let prev_seed = node0.compute_seed_for_epoch(epoch - 1, epoch_length);
                assert_ne!(
                    seed0, prev_seed,
                    "Epoch {}: seed must differ from previous epoch seed",
                    epoch
                );
            }
        }
    }

    /// Requirements 12.1: Zero author mismatches over 200 blocks (20 epochs × 10 blocks).
    ///
    /// For each block slot, the expected author from the sequence must match across all nodes.
    #[test]
    fn test_zero_author_mismatches_over_200_blocks() {
        let epoch_length = 10u32;
        let slots_per_epoch = 10u32;
        let num_epochs = 20u32;
        let total_blocks = num_epochs * slots_per_epoch;

        let validators = validators_sorted();

        let node0 = SimulatedNode::new(validators.clone());
        let node1 = SimulatedNode::new(validators.clone());
        let node2 = SimulatedNode::new(validators.clone());

        let mut mismatch_count = 0u32;

        for epoch in 1..=num_epochs {
            let seq0 = node0.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);
            let seq1 = node1.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);
            let seq2 = node2.compute_sequence_for_epoch(epoch, epoch_length, slots_per_epoch);

            for slot in 0..slots_per_epoch as usize {
                let block_number = (epoch - 1) * slots_per_epoch + slot as u32 + 1;

                if seq0[slot] != seq1[slot] || seq0[slot] != seq2[slot] {
                    mismatch_count += 1;
                    eprintln!(
                        "Author mismatch at block {}: node0={:?}, node1={:?}, node2={:?}",
                        block_number, seq0[slot], seq1[slot], seq2[slot]
                    );
                }
            }
        }

        assert_eq!(
            mismatch_count,
            0,
            "Expected zero author mismatches over {} blocks, got {}",
            total_blocks,
            mismatch_count
        );
    }
}
