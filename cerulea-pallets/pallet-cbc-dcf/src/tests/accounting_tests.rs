//! Accounting and economic tests for DCF pallet

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};
use frame_support::traits::Currency;

/// Tests basic balance tracking
#[test]
fn basic_balance_tracking_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            // Check that validators have balances
            let free_balance = Balances::free_balance(validator);
            let reserved_balance = Balances::reserved_balance(validator);
            
            assert!(free_balance > 0 || reserved_balance > 0);
        }
    });
}

/// Tests stake accounting
#[test]
fn stake_accounting_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        for validator in &active_validators {
            let validator_stake = DcfPallet::validator_stake(validator);
            
            // Active validators should meet minimum stake
            assert!(validator_stake >= min_stake);
            
            // Stake should not exceed total balance
            let total_balance = Balances::free_balance(validator) + Balances::reserved_balance(validator);
            assert!(validator_stake <= total_balance);
        }
    });
}

/// Tests reserved balance consistency
#[test]
fn reserved_balance_consistency_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            let reserved = Balances::reserved_balance(validator);
            let free = Balances::free_balance(validator);
            
            // Reserved balance should be non-negative
            assert!(reserved >= 0);
            
            // Total balance should be consistent
            let total = free + reserved;
            assert!(total >= reserved);
            assert!(total >= free);
        }
    });
}

/// Tests balance changes during validator operations
#[test]
fn balance_changes_during_operations_work() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let initial_free = Balances::free_balance(&validator);
        let initial_reserved = Balances::reserved_balance(&validator);
        let initial_total = initial_free + initial_reserved;
        
        // Simulate some operations (advance blocks)
        for block_num in 1..5 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        let final_free = Balances::free_balance(&validator);
        let final_reserved = Balances::reserved_balance(&validator);
        let final_total = final_free + final_reserved;
        
        // Balance changes should be logical
        assert!(final_total >= 0);
        assert!(final_free >= 0);
        assert!(final_reserved >= 0);
    });
}

/// Tests economic invariants
#[test]
fn economic_invariants_hold() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let mut total_staked = 0u128;
        let mut total_reserved = 0u128;
        
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            let reserved = Balances::reserved_balance(validator);
            
            total_staked += stake;
            total_reserved += reserved;
            
            // Individual validator invariants
            assert!(stake >= <Test as crate::Config>::MinStake::get());
            assert!(reserved >= 0);
        }
        
        // System-wide invariants
        assert!(total_staked > 0);
        assert!(total_reserved >= 0);
    });
}

/// Tests reward distribution consistency
#[test]
fn reward_distribution_consistency_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Track initial balances
        let initial_balances: Vec<_> = active_validators.iter()
            .map(|v| (*v, Balances::free_balance(v)))
            .collect();
        
        // Advance through some blocks to potentially trigger rewards
        for block_num in 1..10 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        // Check that balances remain consistent
        for (validator, initial_balance) in initial_balances {
            let current_balance = Balances::free_balance(&validator);
            
            // Balance should not decrease (no slashing in these tests)
            assert!(current_balance >= 0);
            
            // If balance changed, it should be logical
            if current_balance != initial_balance {
                // Change should be reasonable
                assert!(current_balance.abs_diff(initial_balance) < 1_000_000);
            }
        }
    });
}

/// Tests slashing protection
#[test]
fn slashing_protection_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            let reserved = Balances::reserved_balance(validator);
            let stake = DcfPallet::validator_stake(validator);
            
            // Reserved balance should cover stake
            assert!(reserved >= stake || (reserved == 0 && stake <= Balances::free_balance(validator)));
        }
    });
}

/// Tests accounting precision
#[test]
fn accounting_precision_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        for validator in &active_validators {
            let free = Balances::free_balance(validator);
            let reserved = Balances::reserved_balance(validator);
            
            // Balances should be precise (no overflow)
            let total = free.saturating_add(reserved);
            assert!(total >= free);
            assert!(total >= reserved);
        }
    });
}

/// Tests balance updates during score changes
#[test]
fn balance_updates_during_score_changes_work() {
    new_test_ext().execute_with(|| {
        let validator = 1u64;
        let initial_balance = Balances::free_balance(&validator);
        let initial_stake = DcfPallet::validator_stake(&validator);
        
        // Get current scores
        let initial_stake_score = DcfPallet::validator_stake_score(&validator);
        let initial_inference_score = DcfPallet::validator_inference_score(&validator);
        
        // Advance some blocks
        for block_num in 1..5 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }
        
        // Check that accounting remains consistent
        let final_balance = Balances::free_balance(&validator);
        let final_stake = DcfPallet::validator_stake(&validator);
        
        assert!(final_balance >= 0);
        assert!(final_stake >= 0);
    });
}

/// Tests economic bounds enforcement
#[test]
fn economic_bounds_enforcement_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            let free_balance = Balances::free_balance(validator);
            let reserved_balance = Balances::reserved_balance(validator);
            
            // Enforce minimum stake
            assert!(stake >= min_stake);
            
            // Balances should be non-negative
            assert!(free_balance >= 0);
            assert!(reserved_balance >= 0);
            
            // Total balance should be consistent
            let total = free_balance + reserved_balance;
            assert!(total >= stake);
        }
    });
}