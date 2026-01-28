//! Tests for the state root calculation fix (Issue #1)
//!
//! This module tests that the proposer factory correctly calculates state roots
//! instead of using hardcoded zero values.

use super::super::*;
use crate::mock::*;
use sp_core::{sr25519::{Pair, Public}, Pair as PairTrait, H256};
use sp_runtime::traits::{Header as HeaderT, Zero, BlakeTwo256, Hash};
use std::sync::Arc;

/// Test that the new block building method produces blocks with non-zero state roots
#[tokio::test]
async fn test_state_root_calculation_fix() {
    setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
        // This test verifies that Issue #1 (hardcoded zero state root) is fixed
        
        // Create a mock proposer factory
        let (client, transaction_pool) = create_mock_client_and_pool();
        let mut proposer_factory = ProposerFactory::new(
            client.clone(),
            transaction_pool,
            std::time::Duration::from_millis(1000),
            100,
        );
        
        // Create a test block
        let parent_hash = client.info().best_hash;
        let slot = 1u64;
        
        // Use async runtime for the test
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            proposer_factory.create_block_with_transactions(parent_hash, slot).await
        });
        
        match result {
            Ok((block, _author)) => {
                let header = block.header();
                
                // The main fix: state root should NOT be zero
                let zero_hash: H256 = Default::default();
                assert_ne!(
                    header.state_root(), 
                    &zero_hash,
                    "State root should not be zero after fix - this was the main issue!"
                );
                
                // Extrinsics root should also not be zero if we have extrinsics
                if !block.extrinsics().is_empty() {
                    assert_ne!(
                        header.extrinsics_root(),
                        &zero_hash,
                        "Extrinsics root should not be zero when block has extrinsics"
                    );
                }
                
                // Block number should be correct
                assert_eq!(*header.number(), 1u32, "Block number should be 1");
                
                // Parent hash should be set correctly
                assert_eq!(header.parent_hash(), &parent_hash, "Parent hash should match");
                
                println!("✅ State root fix test passed!");
                println!("   State root: {:?}", header.state_root());
                println!("   Extrinsics root: {:?}", header.extrinsics_root());
                println!("   Block number: {:?}", header.number());
                println!("   Parent hash: {:?}", header.parent_hash());
            }
            Err(e) => {
                panic!("Block creation failed: {:?}", e);
            }
        }
    });
}

/// Test that the comprehensive block creation method works correctly
#[tokio::test]
async fn test_comprehensive_block_creation() {
    setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
        let (client, transaction_pool) = create_mock_client_and_pool();
        let mut proposer_factory = ProposerFactory::new(
            client.clone(),
            transaction_pool,
            std::time::Duration::from_millis(1000),
            100,
        );
        
        let parent_hash = client.info().best_hash;
        let slot = 1u64;
        let test_author = Pair::generate().0.public();
        
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            proposer_factory.create_complete_block(
                parent_hash,
                slot,
                None, // No special digest
                Some(test_author.clone()), // Force specific author
            ).await
        });
        
        match result {
            Ok((block, author)) => {
                assert_eq!(author, test_author, "Author should match forced author");
                
                let header = block.header();
                let zero_hash: H256 = Default::default();
                
                // Verify all the fixes
                assert_ne!(header.state_root(), &zero_hash, "State root must not be zero");
                assert_eq!(*header.number(), 1u32, "Block number should be 1");
                assert_eq!(header.parent_hash(), &parent_hash, "Parent hash should match");
                
                println!("✅ Comprehensive block creation test passed!");
            }
            Err(e) => {
                panic!("Comprehensive block creation failed: {:?}", e);
            }
        }
    });
}

/// Test that emergency block creation works as a fallback
#[tokio::test]
async fn test_emergency_block_creation() {
    setup_consensus_test(MockConsensusConfig::default()).execute_with(|| {
        let (client, transaction_pool) = create_mock_client_and_pool();
        let mut proposer_factory = ProposerFactory::new(
            client.clone(),
            transaction_pool,
            std::time::Duration::from_millis(1000),
            100,
        );
        
        let parent_hash = client.info().best_hash;
        let slot = 1u64;
        let test_author = Pair::generate().0.public();
        
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let result = runtime.block_on(async {
            proposer_factory.create_emergency_block(parent_hash, slot, test_author.clone()).await
        });
        
        match result {
            Ok((block, author)) => {
                assert_eq!(author, test_author, "Author should match");
                
                let header = block.header();
                let zero_hash: H256 = Default::default();
                
                // Even emergency blocks should have proper state roots
                assert_ne!(header.state_root(), &zero_hash, "Emergency block state root must not be zero");
                assert_eq!(*header.number(), 1u32, "Block number should be 1");
                assert_eq!(header.parent_hash(), &parent_hash, "Parent hash should match");
                
                // Emergency blocks should have at least inherent extrinsics
                assert!(!block.extrinsics().is_empty(), "Emergency block should have inherent extrinsics");
                
                println!("✅ Emergency block creation test passed!");
            }
            Err(e) => {
                panic!("Emergency block creation failed: {:?}", e);
            }
        }
    });
}

/// Test that block validation catches zero state roots
#[test]
fn test_block_validation_catches_zero_state_root() {
    // Create a block with zero state root (the old broken behavior)
    let zero_hash: H256 = Default::default();
    let parent_hash = BlakeTwo256::hash(b"parent");
    
    let bad_header = cbc_runtime::Header::new(
        1u32,
        BlakeTwo256::hash(b"extrinsics"), // Non-zero extrinsics root
        zero_hash, // Zero state root - this should be caught
        parent_hash,
        Default::default(),
    );
    
    let bad_block = cbc_runtime::Block::new(bad_header, vec![]);
    
    // Create a mock proposer factory to test validation
    let (client, transaction_pool) = create_mock_client_and_pool();
    let proposer_factory = ProposerFactory::new(
        client,
        transaction_pool,
        std::time::Duration::from_millis(1000),
        100,
    );
    
    // This should fail validation
    let validation_result = proposer_factory.validate_built_block(&bad_block);
    
    assert!(validation_result.is_err(), "Validation should fail for zero state root");
    
    if let Err(e) = validation_result {
        assert!(
            format!("{:?}", e).contains("zero state root"),
            "Error should mention zero state root"
        );
    }
    
    println!("✅ Block validation test passed - correctly caught zero state root!");
}

/// Helper function to create mock client and transaction pool
fn create_mock_client_and_pool() -> (Arc<MockClient>, Arc<MockTransactionPool>) {
    let client = Arc::new(MockClient::new());
    let transaction_pool = Arc::new(MockTransactionPool::new());
    (client, transaction_pool)
}

/// Mock client for testing
struct MockClient {
    // Add mock implementation
}

impl MockClient {
    fn new() -> Self {
        Self {}
    }
    
    fn info(&self) -> MockClientInfo {
        MockClientInfo {
            best_hash: BlakeTwo256::hash(b"genesis"),
            best_number: 0u32,
        }
    }
}

struct MockClientInfo {
    best_hash: H256,
    best_number: u32,
}

/// Mock transaction pool for testing
struct MockTransactionPool {
    // Add mock implementation
}

impl MockTransactionPool {
    fn new() -> Self {
        Self {}
    }
}

// Note: Full mock implementations would be needed for complete testing
// This is a framework for the tests that demonstrates the validation logic