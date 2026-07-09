// Test data generators for property-based testing
//
// This module provides proptest strategies for generating valid test data
// that covers the full input space for CBC Runtime APIs and RPC endpoints.

use proptest::prelude::*;
use proptest::strategy::{Strategy, ValueTree};
use sp_core::crypto::AccountId32;
use cerulea_runtime::AccountId;

/// Strategy to generate random validator accounts
/// 
/// Generates valid 32-byte AccountId values for use in property tests.
/// These accounts are cryptographically valid but randomly generated.
pub fn validator_account_strategy() -> impl Strategy<Value = AccountId> {
    prop::collection::vec(any::<u8>(), 32)
        .prop_map(|bytes| {
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            AccountId::from(arr)
        })
}

/// Strategy to generate reasonable block numbers
/// 
/// Generates block numbers in a reasonable range to avoid overflow issues
/// while still covering a wide range of values for testing.
pub fn block_number_strategy() -> impl Strategy<Value = u32> {
    0u32..1_000_000u32
}

/// Strategy to generate valid PoS scores
/// 
/// Generates PoS (Proof of Stake) scores in the valid range [0, 100].
/// These scores represent validator performance in the PoS consensus.
pub fn pos_score_strategy() -> impl Strategy<Value = u32> {
    0u32..=100u32
}

/// Strategy to generate valid PoI confidence scores
/// 
/// Generates PoI (Proof of Intelligence) confidence scores in the valid range [0, 100].
/// These scores represent validator performance in the PoI consensus.
pub fn poi_score_strategy() -> impl Strategy<Value = u32> {
    0u32..=100u32
}

/// Strategy to generate valid consensus weights
/// 
/// Generates weight pairs (pos_weight, poi_weight) where both weights are positive.
/// These weights are used in trust score calculations.
pub fn consensus_weights_strategy() -> impl Strategy<Value = (u64, u64)> {
    (1u64..=100u64, 1u64..=100u64)
}

/// Strategy to generate validator sets
/// 
/// Generates collections of 1-10 unique validator accounts for testing
/// scenarios with different validator set sizes.
pub fn validator_set_strategy() -> impl Strategy<Value = Vec<AccountId>> {
    prop::collection::vec(validator_account_strategy(), 1..=10)
        .prop_map(|mut validators| {
            // Remove duplicates to ensure unique validator sets
            validators.sort();
            validators.dedup();
            // Ensure we have at least one validator
            if validators.is_empty() {
                validators.push(AccountId32::from([1u8; 32]));
            }
            validators
        })
}

/// Strategy to generate balance amounts
/// 
/// Generates balance amounts in a reasonable range for staking scenarios.
/// Balances are positive and within practical limits.
pub fn balance_strategy() -> impl Strategy<Value = u128> {
    1u128..=1_000_000_000_000u128
}

/// Strategy to generate slashing counts
/// 
/// Generates slashing counts in the range [0, 10] to test various
/// validator penalty scenarios.
#[allow(dead_code)]
pub fn slashing_count_strategy() -> impl Strategy<Value = u32> {
    0u32..=10u32
}

/// Strategy to generate epoch numbers
/// 
/// Generates epoch numbers in a reasonable range for testing
/// epoch-dependent functionality.
#[allow(dead_code)]
pub fn epoch_strategy() -> impl Strategy<Value = u32> {
    1u32..=1000u32
}

/// Strategy to generate inference results
/// 
/// Generates PoI inference results as (result, confidence) pairs
/// where both values are in valid ranges.
#[allow(dead_code)]
pub fn inference_result_strategy() -> impl Strategy<Value = (u32, u32)> {
    (0u32..=100u32, 0u32..=100u32)
}

/// Strategy to generate validator stakes
/// 
/// Generates (validator, stake) pairs for testing staking scenarios.
/// Each validator has a positive stake amount.
#[allow(dead_code)]
pub fn validator_stakes_strategy() -> impl Strategy<Value = Vec<(AccountId, u128)>> {
    prop::collection::vec(
        (validator_account_strategy(), balance_strategy()),
        1..=10
    ).prop_map(|mut stakes| {
        // Remove duplicate validators, keeping the first stake for each
        stakes.sort_by_key(|(validator, _)| validator.clone());
        stakes.dedup_by_key(|(validator, _)| validator.clone());
        stakes
    })
}

/// Strategy to generate validator scores
/// 
/// Generates (validator, score) pairs for testing score-based scenarios.
/// Scores are in the valid range for the respective consensus mechanism.
#[allow(dead_code)]
pub fn validator_pos_scores_strategy() -> impl Strategy<Value = Vec<(AccountId, u32)>> {
    prop::collection::vec(
        (validator_account_strategy(), pos_score_strategy()),
        1..=10
    ).prop_map(|mut scores| {
        // Remove duplicate validators, keeping the first score for each
        scores.sort_by_key(|(validator, _)| validator.clone());
        scores.dedup_by_key(|(validator, _)| validator.clone());
        scores
    })
}

/// Strategy to generate PoI inference results for validators
/// 
/// Generates (validator, inference_result) pairs for testing PoI scenarios.
#[allow(dead_code)]
pub fn validator_poi_results_strategy() -> impl Strategy<Value = Vec<(AccountId, (u32, u32))>> {
    prop::collection::vec(
        (validator_account_strategy(), inference_result_strategy()),
        1..=10
    ).prop_map(|mut results| {
        // Remove duplicate validators, keeping the first result for each
        results.sort_by_key(|(validator, _)| validator.clone());
        results.dedup_by_key(|(validator, _)| validator.clone());
        results
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::test_runner::TestRunner;

    #[test]
    fn test_validator_account_strategy() {
        let mut runner = TestRunner::default();
        let strategy = validator_account_strategy();
        
        for _ in 0..10 {
            let account = strategy.new_tree(&mut runner).unwrap().current();
            let account_bytes: &[u8] = account.as_ref();
            assert_eq!(account_bytes.len(), 32);
        }
    }

    #[test]
    fn test_block_number_strategy() {
        let mut runner = TestRunner::default();
        let strategy = block_number_strategy();
        
        for _ in 0..10 {
            let block_number = strategy.new_tree(&mut runner).unwrap().current();
            assert!(block_number < 1_000_000);
        }
    }

    #[test]
    fn test_score_strategies() {
        let mut runner = TestRunner::default();
        
        // Test PoS scores
        let pos_strategy = pos_score_strategy();
        for _ in 0..10 {
            let score = pos_strategy.new_tree(&mut runner).unwrap().current();
            assert!(score <= 100);
        }
        
        // Test PoI scores
        let poi_strategy = poi_score_strategy();
        for _ in 0..10 {
            let score = poi_strategy.new_tree(&mut runner).unwrap().current();
            assert!(score <= 100);
        }
    }

    #[test]
    fn test_consensus_weights_strategy() {
        let mut runner = TestRunner::default();
        let strategy = consensus_weights_strategy();
        
        for _ in 0..10 {
            let (pos_weight, poi_weight) = strategy.new_tree(&mut runner).unwrap().current();
            assert!(pos_weight > 0);
            assert!(poi_weight > 0);
            assert!(pos_weight <= 100);
            assert!(poi_weight <= 100);
        }
    }

    #[test]
    fn test_validator_set_strategy() {
        let mut runner = TestRunner::default();
        let strategy = validator_set_strategy();
        
        for _ in 0..10 {
            let validators = strategy.new_tree(&mut runner).unwrap().current();
            assert!(!validators.is_empty());
            assert!(validators.len() <= 10);
            
            // Verify all validators are unique
            for i in 0..validators.len() {
                for j in (i + 1)..validators.len() {
                    assert_ne!(validators[i], validators[j]);
                }
            }
        }
    }

    #[test]
    fn test_balance_strategy() {
        let mut runner = TestRunner::default();
        let strategy = balance_strategy();
        
        for _ in 0..10 {
            let balance = strategy.new_tree(&mut runner).unwrap().current();
            assert!(balance > 0);
            assert!(balance <= 1_000_000_000_000u128);
        }
    }
}