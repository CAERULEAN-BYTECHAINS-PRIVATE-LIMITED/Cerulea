//! Benchmarking setup for pallet-cerulea-dcf
//! 
//! This module provides comprehensive benchmarks for all DCF operations
//! to measure performance characteristics and optimize for production use.

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::v2::*;
use frame_benchmarking::account;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_std::vec;
use crate::{Pallet as DcfPallet, Config, ProposalAction};

/// Helper function to create a validator account with initial state
fn create_validator<T: Config>(id: u32) -> T::AccountId {
    let validator: T::AccountId = account("validator", id, 0);
    
    // Create validator state
    let validator_state = ValidatorState {
        last_active_epoch: 0,
        current: EpochStats {
            epoch: 0,
            stake_score: 1000,
            inference_score: 800,
            final_score: 900,
            authored_blocks: 0,
            missed_blocks: 0,
        },
        history: BoundedVec::default(),
        uptime: 0,
        inference_success_count: 0,
        participation_rate: 100,
        inference_count: 0,
        last_active_block: 0,
        name: None,
        trust_score: 0,
    };
    
    ValidatorStates::<T>::insert(&validator, validator_state);
    validator
}

/// Helper function to setup multiple validators
fn setup_validators<T: Config>(count: u32) -> Vec<T::AccountId> {
    let mut validators = Vec::new();
    for i in 0..count {
        validators.push(create_validator::<T>(i));
    }
    
    // Add to validator set
    let bounded_validators = BoundedVec::try_from(validators.clone())
        .expect("Too many validators for benchmarking");
    ValidatorSet::<T>::put(bounded_validators.clone());
    ActiveValidators::<T>::put(bounded_validators);
    
    validators
}

#[benchmarks]
mod benchmarks {
    use super::*;

    /// Benchmark join_validators call
    #[benchmark]
    fn join_validators() {
        let new_validator: T::AccountId = account("new_validator", 0, 0);
        let min_stake = <T as Config>::MinStake::get();

        // Give the validator sufficient balance
        T::Currency::make_free_balance_be(&new_validator, min_stake * 2u32.into());

        let origin = RawOrigin::Signed(new_validator.clone());

        #[extrinsic_call]
        join_validators(origin, None);

        assert!(ValidatorSet::<T>::get().contains(&new_validator));
        assert_eq!(ValidatorStake::<T>::get(&new_validator), min_stake);
    }

    /// Benchmark leave_validators call
    #[benchmark]
    fn leave_validators() {
        let validator = create_validator::<T>(0);
        let min_stake = <T as Config>::MinStake::get();

        // Setup validator with stake
        T::Currency::make_free_balance_be(&validator, min_stake * 2u32.into());
        let _ = T::Currency::reserve(&validator, min_stake);
        ValidatorStake::<T>::insert(&validator, min_stake);

        // Add to validator set
        let mut validator_set = ValidatorSet::<T>::get();
        let _ = validator_set.try_push(validator.clone());
        ValidatorSet::<T>::put(validator_set.clone());
        ActiveValidators::<T>::put(validator_set);

        let origin = RawOrigin::Signed(validator.clone());

        #[extrinsic_call]
        leave_validators(origin);

        assert!(ValidatorLeaveRequests::<T>::contains_key(&validator));
    }

    /// Benchmark cancel_leave_request call
    #[benchmark]
    fn cancel_leave_request() {
        let validator = create_validator::<T>(0);
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

        // Setup validator with pending leave request
        ValidatorLeaveRequests::<T>::insert(&validator, current_block);

        let origin = RawOrigin::Signed(validator.clone());

        #[extrinsic_call]
        cancel_leave_request(origin);

        assert!(!ValidatorLeaveRequests::<T>::contains_key(&validator));
    }

    /// Benchmark slash_validator call
    #[benchmark]
    fn slash_validator() {
        let validator = create_validator::<T>(0);
        let min_stake = <T as Config>::MinStake::get();
        let slash_amount = min_stake / 4u32.into();

        // Setup validator with stake
        T::Currency::make_free_balance_be(&validator, min_stake * 2u32.into());
        let _ = T::Currency::reserve(&validator, min_stake);
        ValidatorStake::<T>::insert(&validator, min_stake);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        slash_validator(origin, validator.clone(), slash_amount);

        assert!(ValidatorStake::<T>::get(&validator) < min_stake);
    }

    /// Benchmark slash_validator_percentage call
    #[benchmark]
    fn slash_validator_percentage() {
        let validator = create_validator::<T>(0);
        let min_stake = <T as Config>::MinStake::get();

        // Setup validator with stake
        T::Currency::make_free_balance_be(&validator, min_stake * 2u32.into());
        let _ = T::Currency::reserve(&validator, min_stake);
        ValidatorStake::<T>::insert(&validator, min_stake);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        slash_validator_percentage(origin, validator.clone(), 25); // 25%

        assert!(ValidatorStake::<T>::get(&validator) < min_stake);
    }

    /// Benchmark increase_validator_stake call
    #[benchmark]
    fn increase_validator_stake() {
        let validator = create_validator::<T>(0);
        let min_stake = <T as Config>::MinStake::get();
        let additional_stake = min_stake / 2u32.into();

        // Setup validator with stake and extra balance
        T::Currency::make_free_balance_be(&validator, min_stake * 3u32.into());
        let _ = T::Currency::reserve(&validator, min_stake);
        ValidatorStake::<T>::insert(&validator, min_stake);

        // Add to validator set
        let mut validator_set = ValidatorSet::<T>::get();
        let _ = validator_set.try_push(validator.clone());
        ValidatorSet::<T>::put(validator_set);

        let origin = RawOrigin::Signed(validator.clone());

        #[extrinsic_call]
        increase_validator_stake(origin, additional_stake);

        assert_eq!(ValidatorStake::<T>::get(&validator), min_stake + additional_stake);
    }

    /// Benchmark decrease_validator_stake call
    #[benchmark]
    fn decrease_validator_stake() {
        let validator = create_validator::<T>(0);
        let min_stake = <T as Config>::MinStake::get();
        let total_stake = min_stake * 2u32.into();
        let decrease_amount = min_stake / 2u32.into();

        // Setup validator with higher stake
        T::Currency::make_free_balance_be(&validator, total_stake * 2u32.into());
        let _ = T::Currency::reserve(&validator, total_stake);
        ValidatorStake::<T>::insert(&validator, total_stake);

        // Add to validator set
        let mut validator_set = ValidatorSet::<T>::get();
        let _ = validator_set.try_push(validator.clone());
        ValidatorSet::<T>::put(validator_set);

        let origin = RawOrigin::Signed(validator.clone());

        #[extrinsic_call]
        decrease_validator_stake(origin, decrease_amount);

        assert_eq!(ValidatorStake::<T>::get(&validator), total_stake - decrease_amount);
    }

    /// Benchmark report_validator_misbehavior call
    #[benchmark]
    fn report_validator_misbehavior() {
        let reporter = create_validator::<T>(0);
        let reported = create_validator::<T>(1);
        let evidence = vec![1u8; 100]; // 100 bytes of evidence

        // Setup both validators in validator set
        let mut validator_set = ValidatorSet::<T>::get();
        let _ = validator_set.try_push(reporter.clone());
        let _ = validator_set.try_push(reported.clone());
        ValidatorSet::<T>::put(validator_set);

        let bounded_evidence = BoundedVec::try_from(evidence).unwrap();
        let origin = RawOrigin::Signed(reporter.clone());

        #[extrinsic_call]
        report_validator_misbehavior(origin, reported.clone(), bounded_evidence);

        assert!(MisbehaviorReports::<T>::contains_key(&reported, &reporter));
    }

    /// Benchmark simulate_inference call
    #[benchmark]
    fn simulate_inference() {
        let validator = create_validator::<T>(0);
        let trigger = create_validator::<T>(1);

        let origin = RawOrigin::Signed(trigger.clone());

        #[extrinsic_call]
        simulate_inference(origin, Some(validator.clone()));

        // Verify inference count was incremented
        let state = ValidatorStates::<T>::get(&validator).unwrap();
        assert!(state.inference_count > 0);
    }

    /// Benchmark propose_slash_validator call
    #[benchmark]
    fn propose_slash_validator() {
        let validator = create_validator::<T>(0);
        let proposer = create_validator::<T>(1);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_slash_validator(origin, proposer.clone(), validator.clone(), 1000u32.into());

        assert!(Proposals::<T>::contains_key(0));
    }

    /// Benchmark propose_reward_validator call
    #[benchmark]
    fn propose_reward_validator() {
        let validator = create_validator::<T>(0);
        let proposer = create_validator::<T>(1);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_reward_validator(origin, proposer.clone(), validator.clone(), 1000u32.into());

        assert!(Proposals::<T>::contains_key(0));
    }

    /// Benchmark propose_eject_validator call
    #[benchmark]
    fn propose_eject_validator() {
        let validator = create_validator::<T>(0);
        let proposer = create_validator::<T>(1);
        let reason = EjectionReason::ScoreBelowThreshold;

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_eject_validator(origin, proposer.clone(), validator.clone(), reason);

        assert!(Proposals::<T>::contains_key(0));
    }

    /// Benchmark slash_multiple_validators call
    #[benchmark]
    fn slash_multiple_validators() {
        let validators = setup_validators::<T>(5);
        let slash_amount = 100u32.into();
        let min_stake = <T as Config>::MinStake::get();

        // Setup all validators with stake
        for validator in &validators {
            T::Currency::make_free_balance_be(validator, min_stake * 2u32.into());
            let _ = T::Currency::reserve(validator, min_stake);
            ValidatorStake::<T>::insert(validator, min_stake);
        }

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        slash_multiple_validators(origin, validators.clone(), slash_amount);

        // Verify all validators were slashed
        for validator in &validators {
            assert!(ValidatorStake::<T>::get(validator) < min_stake);
        }
    }

    /// Benchmark execute_proposals for proposal execution measurement
    #[benchmark]
    fn execute_proposals() {
        let validators = setup_validators::<T>(5);
        let proposer = &validators[0];
        let target = &validators[1];
        
        GovernanceModeEnabled::<T>::put(true);
        
        // Create multiple approved proposals
        for i in 0..3 {
            let action = ProposalAction::Reward {
                validator: target.clone(),
                amount: (1000 * (i + 1)).into(),
            };
            
            let proposal = GovernanceProposal {
                proposer: proposer.clone(),
                action,
                status: ProposalStatus::Approved,
                votes_for: 10,
                votes_against: 0,
            };
            
            Proposals::<T>::insert(i, proposal);
        }
        
        let origin = RawOrigin::Root;

        #[block]
        {
            // Execute multiple proposals
            for i in 0..3 {
                let _ = DcfPallet::<T>::execute_proposal(origin.clone().into(), i);
            }
        }

        // Verify proposals were executed
        for i in 0..3 {
            let executed_proposal = Proposals::<T>::get(i).unwrap();
            assert_eq!(executed_proposal.status, ProposalStatus::Executed);
        }
    }

    /// Benchmark epoch_transition for epoch change performance
    #[benchmark]
    fn epoch_transition() {
        let validators = setup_validators::<T>(10);
        let epoch_length = T::EpochLength::get();
        
        // Setup epoch config
        let epoch_config = EpochConfig {
            blocks_per_epoch: epoch_length,
            min_stake: 1000u128,
            max_validators: 100,
        };
        EpochConfigStorage::<T>::put(epoch_config);
        
        // Setup validator states and stakes
        let min_stake = <T as Config>::MinStake::get();
        for validator in &validators {
            T::Currency::make_free_balance_be(validator, min_stake * 2u32.into());
            let _ = T::Currency::reserve(validator, min_stake);
            ValidatorStake::<T>::insert(validator, min_stake);
        }
        
        // Add some pending actions
        for (i, validator) in validators.iter().enumerate() {
            if i < 3 {
                PendingValidatorActions::<T>::insert(validator, ValidatorAction::Join);
            } else if i < 6 {
                PendingValidatorActions::<T>::insert(validator, ValidatorAction::Leave);
            }
        }
        
        let initial_epoch = CurrentEpoch::<T>::get();
        let epoch_boundary_block = (initial_epoch + 1) * epoch_length + 1;
        frame_system::Pallet::<T>::set_block_number(epoch_boundary_block.into());

        #[block]
        {
            // Trigger epoch transition
            DcfPallet::<T>::on_initialize(epoch_boundary_block.into());
        }

        // Verify epoch transition occurred and actions were processed
        assert!(CurrentEpoch::<T>::get() > initial_epoch);
    }

    /// Benchmark set_validator_metadata call
    #[benchmark]
    fn set_validator_metadata() {
        let validator = create_validator::<T>(0);
        let origin = RawOrigin::Signed(validator.clone());
        let name = b"Benchmark Validator".to_vec();
        let website = Some(b"https://example.com".to_vec());
        let contact = Some(b"test@example.com".to_vec());
        let description = Some(b"A test validator for benchmarking".to_vec());
        let location = Some(b"Test Location".to_vec());

        #[extrinsic_call]
        set_validator_metadata(origin, name, website, contact, description, location, None, None);

        // Verify metadata was set
        assert!(ValidatorMetadata::<T>::contains_key(&validator));
    }

    /// Benchmark update_validator_activity call
    #[benchmark]
    fn update_validator_activity() {
        let validator = create_validator::<T>(0);
        let caller = create_validator::<T>(1);
        let origin = RawOrigin::Signed(caller.clone());
        let current_block = frame_system::Pallet::<T>::block_number().saturated_into::<u32>();

        #[extrinsic_call]
        update_validator_activity(origin, validator.clone(), current_block, 0);

        // Verify activity was updated
        let state = ValidatorStates::<T>::get(&validator).unwrap();
        assert_eq!(state.last_active_block, current_block);
    }

    /// Benchmark distribute_epoch_rewards call
    #[benchmark]
    fn distribute_epoch_rewards() {
        let validators = setup_validators::<T>(5);
        let total_reward_pool = 10000u32.into();
        let min_stake = <T as Config>::MinStake::get();

        // Setup validators with stakes
        for validator in &validators {
            T::Currency::make_free_balance_be(validator, min_stake * 2u32.into());
            let _ = T::Currency::reserve(validator, min_stake);
            ValidatorStake::<T>::insert(validator, min_stake);
        }

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        distribute_epoch_rewards(origin, total_reward_pool);

        // Verify rewards were distributed (check that function completed)
        // Function completed successfully if we reach this point
    }

    /// Benchmark propose_default_reward_validator call
    #[benchmark]
    fn propose_default_reward_validator() {
        let validator = create_validator::<T>(0);
        let proposer = create_validator::<T>(1);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_default_reward_validator(origin, proposer.clone(), validator.clone());

        assert!(Proposals::<T>::contains_key(0));
    }

    /// Benchmark propose_default_reward_multiple_validators call
    #[benchmark]
    fn propose_default_reward_multiple_validators() {
        let validators = setup_validators::<T>(3);
        let proposer = create_validator::<T>(10);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_default_reward_multiple_validators(origin, proposer.clone(), validators.clone());

        assert!(Proposals::<T>::contains_key(0));
    }

    /// Benchmark propose_reward_all_active_validators call
    #[benchmark]
    fn propose_reward_all_active_validators() {
        let _validators = setup_validators::<T>(5);
        let proposer = create_validator::<T>(10);

        let origin = RawOrigin::Root;

        #[extrinsic_call]
        propose_reward_all_active_validators(origin, proposer.clone());

        assert!(Proposals::<T>::contains_key(0));
    }

    impl_benchmark_test_suite!(DcfPallet, crate::mock::new_test_ext(), crate::mock::Test);
}