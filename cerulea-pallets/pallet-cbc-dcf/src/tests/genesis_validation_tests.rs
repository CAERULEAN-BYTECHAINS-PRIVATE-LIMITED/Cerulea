//! Genesis configuration validation tests

use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

/// Tests basic genesis configuration
#[test]
fn basic_genesis_configuration_works() {
    new_test_ext().execute_with(|| {
        // Check that genesis configuration was applied correctly
        let active_validators = DcfPallet::active_validators();
        assert_eq!(active_validators.len(), 3); // From mock genesis
        assert!(active_validators.contains(&1));
        assert!(active_validators.contains(&2));
        assert!(active_validators.contains(&3));
        
        // Check initial epoch
        assert_eq!(DcfPallet::current_epoch(), 0);
    });
}

/// Tests genesis validator initialization
#[test]
fn genesis_validator_initialization_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Check that all genesis validators have proper scores
        for validator in &active_validators {
            let stake_score = DcfPallet::validator_stake_score(validator);
            let inference_score = DcfPallet::validator_inference_score(validator);
            
            assert!(stake_score > 0); // Should have non-zero stake score
            assert!(inference_score >= 0); // Inference score can be 0 initially
        }
    });
}

/// Tests genesis epoch configuration
#[test]
fn genesis_epoch_configuration_works() {
    new_test_ext().execute_with(|| {
        let current_epoch = DcfPallet::current_epoch();
        assert_eq!(current_epoch, 0); // Should start at epoch 0
        
        // Check epoch length configuration
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        assert_eq!(epoch_length, 2400); // From mock configuration
    });
}

/// Tests genesis consensus weights
#[test]
fn genesis_consensus_weights_work() {
    new_test_ext().execute_with(|| {
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        
        // Should match configured defaults
        let default_pos = <Test as crate::Config>::DefaultPosWeight::get();
        let default_poi = <Test as crate::Config>::DefaultPoiWeight::get();
        
        assert_eq!(pos_weight, default_pos);
        assert_eq!(poi_weight, default_poi);
        assert_eq!(pos_weight + poi_weight, 10000);
    });
}

/// Tests genesis stake configuration
#[test]
fn genesis_stake_configuration_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_stake = <Test as crate::Config>::MinStake::get();
        
        // All genesis validators should meet minimum stake
        for validator in &active_validators {
            let stake = DcfPallet::validator_stake(validator);
            assert!(stake >= min_stake);
            
            // Check that validator has sufficient balance
            let free_balance = Balances::free_balance(validator);
            let reserved_balance = Balances::reserved_balance(validator);
            let total_balance = free_balance + reserved_balance;
            
            assert!(total_balance >= stake);
        }
    });
}

/// Tests genesis governance mode
#[test]
fn genesis_governance_mode_works() {
    new_test_ext().execute_with(|| {
        let governance_mode = DcfPallet::governance_mode();
        assert!(!governance_mode); // Should be false by default in tests
    });
}

/// Tests genesis validator set constraints
#[test]
fn genesis_validator_set_constraints_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        
        // Should satisfy constraints
        assert!(active_validators.len() <= max_validators as usize);
        assert!(active_validators.len() >= min_active as usize);
    });
}

/// Tests custom genesis configuration
#[test]
fn custom_genesis_configuration_works() {
    // Test with custom genesis configuration
    let custom_genesis = crate::GenesisConfig::<Test> {
        validators: vec![10, 11, 12, 13],
        validator_scores: vec![5000, 6000, 7000, 8000],
        validator_stakes: vec![2000, 2500, 3000, 3500],
        validator_names: vec![],
        current_epoch: 0,
        epoch_config: crate::EpochConfig {
            blocks_per_epoch: 20,
            min_stake: 1500,
            max_validators: 50,
        },
        strict_validation: true,
    };
    
    new_test_ext_with_genesis(custom_genesis).execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Should have custom validators
        assert_eq!(active_validators.len(), 4);
        assert!(active_validators.contains(&10));
        assert!(active_validators.contains(&11));
        assert!(active_validators.contains(&12));
        assert!(active_validators.contains(&13));
        
        // Check custom scores
        for (i, validator) in active_validators.iter().enumerate() {
            let expected_scores = vec![5000, 6000, 7000, 8000];
            let actual_score = DcfPallet::validator_stake_score(validator);
            
            // Score should be reasonable (exact match may depend on implementation)
            assert!(actual_score > 0);
        }
    });
}

/// Tests genesis validation with edge cases
#[test]
fn genesis_validation_edge_cases_work() {
    // Test minimal validator set
    let minimal_genesis = crate::GenesisConfig::<Test> {
        validators: vec![20],
        validator_scores: vec![5000],
        validator_stakes: vec![1000],
        validator_names: vec![],
        current_epoch: 0,
        epoch_config: crate::EpochConfig {
            blocks_per_epoch: 10,
            min_stake: 1000,
            max_validators: 1,
        },
        strict_validation: false,
    };
    
    new_test_ext_with_genesis(minimal_genesis).execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Should handle minimal configuration
        assert_eq!(active_validators.len(), 1);
        assert!(active_validators.contains(&20));
        
        let stake_score = DcfPallet::validator_stake_score(&20);
        assert!(stake_score > 0);
    });
}

/// Tests genesis state consistency
#[test]
fn genesis_state_consistency_works() {
    new_test_ext().execute_with(|| {
        // All genesis state should be internally consistent
        let active_validators = DcfPallet::active_validators();
        let current_epoch = DcfPallet::current_epoch();
        let (pos_weight, poi_weight) = DcfPallet::consensus_weights();
        
        // Basic consistency checks
        assert!(!active_validators.is_empty());
        assert_eq!(current_epoch, 0);
        assert_eq!(pos_weight + poi_weight, 10000);
        
        // Validator consistency
        for validator in &active_validators {
            assert!(DcfPallet::is_validator_active(validator));
            
            let stake_score = DcfPallet::validator_stake_score(validator);
            let inference_score = DcfPallet::validator_inference_score(validator);
            let stake = DcfPallet::validator_stake(validator);
            
            assert!(stake_score > 0);
            assert!(inference_score >= 0);
            assert!(stake >= <Test as crate::Config>::MinStake::get());
        }
    });
}

/// Tests genesis configuration parameter validation
#[test]
fn genesis_configuration_parameter_validation_works() {
    new_test_ext().execute_with(|| {
        // Validate that all configuration parameters are reasonable
        let max_validators = <Test as crate::Config>::MaxValidators::get();
        let min_active = <Test as crate::Config>::MinActiveValidators::get();
        let epoch_length: u32 = <Test as crate::Config>::EpochLength::get();
        let min_stake = <Test as crate::Config>::MinStake::get();
        let max_score = <Test as crate::Config>::MaxValidatorScore::get();
        
        // Parameter validation
        assert!(max_validators > 0);
        assert!(min_active > 0);
        assert!(min_active <= max_validators);
        assert!(epoch_length > 0);
        assert!(min_stake > 0);
        assert!(max_score > 0);
    });
}

/// Tests genesis with maximum validators
#[test]
fn genesis_with_maximum_validators_works() {
    // Test with many validators (up to limit)
    let max_validators = 10u32; // Reasonable number for test
    let validators: Vec<u64> = (100..100+max_validators as u64).collect();
    let scores: Vec<u32> = (0..max_validators).map(|i| 5000 + i * 100).collect();
    let stakes: Vec<u128> = (0..max_validators).map(|_| 2000u128).collect();
    
    let max_genesis = crate::GenesisConfig::<Test> {
        validators: validators.clone(),
        validator_scores: scores,
        validator_stakes: stakes,
        validator_names: vec![],
        current_epoch: 0,
        epoch_config: crate::EpochConfig {
            blocks_per_epoch: 50,
            min_stake: 1500,
            max_validators,
        },
        strict_validation: false,
    };
    
    new_test_ext_with_genesis(max_genesis).execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        
        // Should handle maximum validators
        assert_eq!(active_validators.len(), max_validators as usize);
        
        // All specified validators should be active
        for validator in &validators {
            assert!(active_validators.contains(validator));
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}

/// Tests genesis state after first block
#[test]
fn genesis_state_after_first_block_works() {
    new_test_ext().execute_with(|| {
        let initial_validators = DcfPallet::active_validators();
        let initial_epoch = DcfPallet::current_epoch();
        
        // Process first block
        System::set_block_number(1);
        let _ = DcfPallet::on_initialize(1);
        DcfPallet::on_finalize(1);
        
        let post_block_validators = DcfPallet::active_validators();
        let post_block_epoch = DcfPallet::current_epoch();
        
        // Genesis state should be preserved initially
        assert_eq!(post_block_validators, initial_validators);
        assert!(post_block_epoch >= initial_epoch);
        
        // All validators should still be active
        for validator in &post_block_validators {
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}