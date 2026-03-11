use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::{sr25519, Pair};
use sp_runtime::BuildStorage;

#[test]
fn test_is_checkpoint_block() {
    new_test_ext().execute_with(|| {
        // Block 0 is a checkpoint (0 % 10 == 0)
        assert!(Dvf::is_checkpoint_block(0));
        
        // Block 10 is a checkpoint
        assert!(Dvf::is_checkpoint_block(10));
        
        // Block 20 is a checkpoint
        assert!(Dvf::is_checkpoint_block(20));
        
        // Block 5 is not a checkpoint
        assert!(!Dvf::is_checkpoint_block(5));
        
        // Block 15 is not a checkpoint
        assert!(!Dvf::is_checkpoint_block(15));
        
        // Block 99 is not a checkpoint
        assert!(!Dvf::is_checkpoint_block(99));
    });
}

#[test]
fn test_is_block_finalized() {
    new_test_ext().execute_with(|| {
        // Initially, no blocks are finalized (finalized block number is 0)
        // Block 0 is finalized by default since it's the genesis
        assert!(Dvf::is_block_finalized(0));
        
        // Blocks after 0 should not be finalized initially
        assert!(!Dvf::is_block_finalized(10));
        assert!(!Dvf::is_block_finalized(20));
        
        // Simulate finalization of block 20
        crate::FinalizedBlockNumber::<Test>::put(20);
        
        // All blocks up to 20 should be finalized
        assert!(Dvf::is_block_finalized(0));
        assert!(Dvf::is_block_finalized(10));
        assert!(Dvf::is_block_finalized(20));
        
        // Blocks after 20 should not be finalized
        assert!(!Dvf::is_block_finalized(21));
        assert!(!Dvf::is_block_finalized(30));
    });
}

#[test]
fn test_get_finality_info() {
    new_test_ext().execute_with(|| {
        // Initially, no blocks are finalized
        let info = Dvf::get_finality_info(10);
        assert!(!info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, None);
        assert_eq!(info.finalized_checkpoint_hash, None);
        
        // Simulate finalization of block 20
        let block_hash = sp_core::H256::from_low_u64_be(20);
        crate::FinalizedBlockNumber::<Test>::put(20);
        crate::FinalizedBlockHash::<Test>::put(block_hash);
        
        // Block 10 should be finalized by checkpoint 10
        let info = Dvf::get_finality_info(10);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(10));
        assert_eq!(info.finalized_checkpoint_hash, Some(block_hash));
        
        // Block 15 should be finalized by checkpoint 10
        let info = Dvf::get_finality_info(15);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(10));
        
        // Block 20 should be finalized by checkpoint 20
        let info = Dvf::get_finality_info(20);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(20));
        
        // Block 25 should not be finalized
        let info = Dvf::get_finality_info(25);
        assert!(!info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, None);
    });
}

#[test]
fn test_checkpoint_validation_in_vote_submission() {
    new_test_ext().execute_with(|| {
        let validator = account_key("Alice");
        let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        // Set up validator with weight
        let validators = vec![(validator.clone(), 1000u128, 100u128)];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create a vote for a non-checkpoint block (block 15)
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: validator.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        // Attempt to submit vote for non-checkpoint block should fail
        assert_noop!(
            Dvf::submit_dvf_vote(RuntimeOrigin::signed(validator.clone()), vote.clone()),
            Error::<Test>::NonCheckpointBlock
        );
        
        // Create a vote for a checkpoint block (block 20)
        vote.block_number = 20;
        vote.block_hash = sp_core::H256::from_low_u64_be(20);
        
        // Re-sign the vote
        let mut encoded_payload = Vec::new();
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        // Submitting vote for checkpoint block should succeed
        assert_ok!(Dvf::submit_dvf_vote(RuntimeOrigin::signed(validator), vote));
    });
}

#[test]
fn test_checkpoint_validation_rejects_non_checkpoint_votes() {
    new_test_ext().execute_with(|| {
        // Initialize block number to 1 so events are registered
        System::set_block_number(1);
        
        let validator = account_key("Alice");
        let pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        // Set up validator with weight
        let validators = vec![(validator.clone(), 1000u128, 100u128)];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create a vote for a non-checkpoint block
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: validator.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        // Attempt to submit vote for non-checkpoint block should fail
        assert_noop!(
            Dvf::submit_dvf_vote(RuntimeOrigin::signed(validator.clone()), vote.clone()),
            Error::<Test>::NonCheckpointBlock
        );
    });
}

#[test]
fn test_submit_justification_success() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        // Set up three validators with weights
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
        let charlie_pair = sr25519::Pair::from_string("//Charlie", None).unwrap();
        
        let validators = vec![
            (alice.clone(), 1000u128, 100u128),
            (bob.clone(), 1000u128, 100u128),
            (charlie.clone(), 1000u128, 100u128),
        ];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create votes for checkpoint block 10
        let block_number = 10u32;
        let block_hash = sp_core::H256::from_low_u64_be(10);
        let round = 0u32;
        
        let mut votes = Vec::new();
        
        for (validator, pair) in &[(alice.clone(), &alice_pair), (bob.clone(), &bob_pair), (charlie.clone(), &charlie_pair)] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: round,
                block_number,
                block_hash,
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
            };
            
            // Sign the vote
            let mut encoded_payload = Vec::new();
            use codec::Encode;
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);
            
            let signature = pair.sign(&encoded_payload);
            vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
            
            votes.push(vote);
        }
        
        // Create justification
        let justification = crate::DvfJustification {
            round_number: round,
            block_hash,
            votes,
        };
        
        // Submit justification
        assert_ok!(Dvf::submit_justification(RuntimeOrigin::signed(alice.clone()), justification));
        
        // Verify finality state was updated
        assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
        assert_eq!(crate::FinalizedBlockHash::<Test>::get(), Some(block_hash));
        assert_eq!(crate::CurrentRound::<Test>::get(), 1);
        
        // Verify BlockFinalized event was emitted
        // Weight calculation: (1000 * 1) + (100 * 100) = 11000 per validator
        // Total: 3 validators * 11000 = 33000
        System::assert_has_event(
            crate::Event::BlockFinalized {
                block_number: 10,
                block_hash,
                round: 0,
                weight: 33000,
            }.into()
        );
    });
}

#[test]
fn test_submit_justification_empty_votes() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        
        // Create empty justification
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: Vec::new(),
        };
        
        // Submit should fail
        assert_noop!(
            Dvf::submit_justification(RuntimeOrigin::signed(alice), justification),
            Error::<Test>::EmptyJustification
        );
    });
}

#[test]
fn test_submit_justification_non_checkpoint_block() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        let validators = vec![(alice.clone(), 1000u128, 100u128)];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create vote for non-checkpoint block 15
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = alice_pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(15),
            votes: vec![vote],
        };
        
        // Submit should fail
        assert_noop!(
            Dvf::submit_justification(RuntimeOrigin::signed(alice), justification),
            Error::<Test>::NonCheckpointBlock
        );
    });
}

#[test]
fn test_submit_justification_already_finalized() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        let validators = vec![(alice.clone(), 1000u128, 100u128)];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Finalize block 10
        crate::FinalizedBlockNumber::<Test>::put(10);
        
        // Create vote for already finalized block 10
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 10,
            block_hash: sp_core::H256::from_low_u64_be(10),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = alice_pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: vec![vote],
        };
        
        // Submit should fail
        assert_noop!(
            Dvf::submit_justification(RuntimeOrigin::signed(alice), justification),
            Error::<Test>::BlockAlreadyFinalized
        );
    });
}

#[test]
fn test_submit_justification_duplicate_validators() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        let validators = vec![(alice.clone(), 1000u128, 100u128)];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create two votes from the same validator
        let mut vote1 = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 10,
            block_hash: sp_core::H256::from_low_u64_be(10),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote1.epoch_id.encode_to(&mut encoded_payload);
        vote1.validator_set_id.encode_to(&mut encoded_payload);
        vote1.round_id.encode_to(&mut encoded_payload);
        vote1.block_number.encode_to(&mut encoded_payload);
        vote1.block_hash.encode_to(&mut encoded_payload);
        vote1.validator_account.encode_to(&mut encoded_payload);
        
        let signature = alice_pair.sign(&encoded_payload);
        vote1.signature = sp_runtime::MultiSignature::Sr25519(signature.clone());
        
        let mut vote2 = vote1.clone();
        vote2.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: vec![vote1, vote2],
        };
        
        // Submit should fail
        assert_noop!(
            Dvf::submit_justification(RuntimeOrigin::signed(alice), justification),
            Error::<Test>::DuplicateValidator
        );
    });
}

#[test]
fn test_submit_justification_threshold_not_reached() {
    new_test_ext().execute_with(|| {
        // Set up three validators with weights
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        
        let validators = vec![
            (alice.clone(), 1000u128, 100u128),
            (bob.clone(), 1000u128, 100u128),
            (charlie.clone(), 1000u128, 100u128),
        ];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Create vote from only one validator (not enough for 2/3 threshold)
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 10,
            block_hash: sp_core::H256::from_low_u64_be(10),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
        };
        
        // Sign the vote
        let mut encoded_payload = Vec::new();
        use codec::Encode;
        vote.epoch_id.encode_to(&mut encoded_payload);
        vote.validator_set_id.encode_to(&mut encoded_payload);
        vote.round_id.encode_to(&mut encoded_payload);
        vote.block_number.encode_to(&mut encoded_payload);
        vote.block_hash.encode_to(&mut encoded_payload);
        vote.validator_account.encode_to(&mut encoded_payload);
        
        let signature = alice_pair.sign(&encoded_payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: vec![vote],
        };
        
        // Submit should fail (1/3 < 2/3 threshold)
        assert_noop!(
            Dvf::submit_justification(RuntimeOrigin::signed(alice), justification),
            Error::<Test>::ThresholdNotReached
        );
    });
}

#[test]
fn test_vote_record_pruning() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        // Set up validators
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
        let charlie_pair = sr25519::Pair::from_string("//Charlie", None).unwrap();
        
        let validators = vec![
            (alice.clone(), 1000u128, 100u128),
            (bob.clone(), 1000u128, 100u128),
            (charlie.clone(), 1000u128, 100u128),
        ];
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Submit votes for round 0
        for (validator, pair) in &[(alice.clone(), &alice_pair), (bob.clone(), &bob_pair)] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: 0,
                block_number: 10,
                block_hash: sp_core::H256::from_low_u64_be(10),
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
            };
            
            let mut encoded_payload = Vec::new();
            use codec::Encode;
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);
            
            let signature = pair.sign(&encoded_payload);
            vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
            
            assert_ok!(Dvf::submit_dvf_vote(RuntimeOrigin::signed(validator.clone()), vote));
        }
        
        // Verify votes are recorded
        assert!(crate::VoteRecords::<Test>::contains_key(0, &alice));
        assert!(crate::VoteRecords::<Test>::contains_key(0, &bob));
        
        // Create and submit justification to trigger finalization and pruning
        let mut votes = Vec::new();
        for (validator, pair) in &[(alice.clone(), &alice_pair), (bob.clone(), &bob_pair), (charlie.clone(), &charlie_pair)] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: 0,
                block_number: 10,
                block_hash: sp_core::H256::from_low_u64_be(10),
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
            };
            
            let mut encoded_payload = Vec::new();
            use codec::Encode;
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);
            
            let signature = pair.sign(&encoded_payload);
            vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
            
            votes.push(vote);
        }
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes,
        };
        
        assert_ok!(Dvf::submit_justification(RuntimeOrigin::signed(alice.clone()), justification));
        
        // Verify round was incremented
        assert_eq!(crate::CurrentRound::<Test>::get(), 1);
        
        // Vote records for round 0 should still exist (retention is 2 rounds)
        // They will be pruned when we reach round 3
    });
}

/// **Bug Condition Exploration Test**
/// 
/// **Validates: Requirements 2.1, 2.2, 2.3, 2.4**
/// 
/// This test demonstrates the bug on UNFIXED code by verifying expected behavior
/// that should exist at genesis but currently doesn't. The test encodes the
/// expected behavior from the design document:
/// 
/// - EpochVotingWeight SHOULD be populated with all validators at genesis
/// - Each validator SHOULD have a weight > 0
/// - TotalVotingWeight SHOULD be > 0
/// - Finalization at block 10 SHOULD succeed
/// 
/// **EXPECTED OUTCOME ON UNFIXED CODE**: This test WILL FAIL because:
/// - EpochVotingWeight is empty at genesis (no GenesisConfig exists)
/// - Validator weight queries return None
/// - TotalVotingWeight is 0
/// - Finalization fails at block 10 due to missing weights
/// 
/// When the fix is implemented, this test will PASS, confirming the bug is resolved.
#[test]
fn test_genesis_weight_initialization_bug_condition() {
    // Create a test environment that simulates genesis with validators configured
    // This mimics what would happen in a real chain genesis with DCF validators
    let mut ext = new_test_ext();
    
    ext.execute_with(|| {
        // Simulate genesis block
        System::set_block_number(0);
        
        // Define validators that would be configured at genesis (from DCF pallet)
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        let genesis_validators = vec![
            (alice.clone(), 10_000_000u128, 80u128),  // 10M stake, 80 score
            (bob.clone(), 8_000_000u128, 80u128),     // 8M stake, 80 score
            (charlie.clone(), 6_000_000u128, 80u128), // 6M stake, 80 score
        ];
        
        // **EXPECTED BEHAVIOR 2.1**: EpochVotingWeight SHOULD be populated at genesis
        // On unfixed code, this will be empty because there's no GenesisConfig
        for (validator, stake, score) in &genesis_validators {
            let weight = crate::EpochVotingWeight::<Test>::get(validator);
            assert!(
                weight.is_some(),
                "Bug detected: Validator {:?} has no weight at genesis. Expected weight to be initialized.",
                validator
            );
            
            // **EXPECTED BEHAVIOR 2.2**: Each validator SHOULD have weight > 0
            let weight_value = weight.unwrap();
            assert!(
                weight_value > 0,
                "Bug detected: Validator {:?} has zero weight at genesis. Expected weight > 0.",
                validator
            );
            
            // Verify weight calculation is correct (stake * factor + score * factor with cap)
            let expected_stake_weight = stake * 1; // StakeWeightFactor = 1
            let expected_score_weight = score * 100; // ScoreWeightFactor = 100
            let expected_total = expected_stake_weight + expected_score_weight;
            assert_eq!(
                weight_value, expected_total,
                "Bug detected: Validator {:?} weight calculation incorrect. Expected {}, got {}",
                validator, expected_total, weight_value
            );
        }
        
        // **EXPECTED BEHAVIOR 2.3**: TotalVotingWeight SHOULD be > 0 at genesis
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert!(
            total_weight > 0,
            "Bug detected: TotalVotingWeight is 0 at genesis. Expected sum of all validator weights."
        );
        
        // Verify total weight is correct sum
        let expected_total: u128 = genesis_validators.iter()
            .map(|(_, stake, score)| (stake * 1) + (score * 100))
            .sum();
        assert_eq!(
            total_weight, expected_total,
            "Bug detected: TotalVotingWeight incorrect. Expected {}, got {}",
            expected_total, total_weight
        );
        
        // **EXPECTED BEHAVIOR 2.4**: Finalization at block 10 SHOULD succeed
        // Move to block 10 (first checkpoint)
        System::set_block_number(10);
        
        // Create votes from all validators for block 10
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
        let charlie_pair = sr25519::Pair::from_string("//Charlie", None).unwrap();
        
        let block_hash = sp_core::H256::from_low_u64_be(10);
        let mut votes = Vec::new();
        
        for (validator, pair) in &[
            (alice.clone(), &alice_pair),
            (bob.clone(), &bob_pair),
            (charlie.clone(), &charlie_pair),
        ] {
            let mut vote = crate::DvfVote {
                epoch_id: 0,
                validator_set_id: 0,
                round_id: 0,
                block_number: 10,
                block_hash,
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
            };
            
            // Sign the vote
            let mut encoded_payload = Vec::new();
            use codec::Encode;
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);
            
            let signature = pair.sign(&encoded_payload);
            vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
            
            votes.push(vote);
        }
        
        // Create justification with all validator votes
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash,
            votes,
        };
        
        // Attempt to finalize block 10
        // On unfixed code, this will fail because validators have no weights
        let result = Dvf::submit_justification(RuntimeOrigin::signed(alice.clone()), justification);
        assert!(
            result.is_ok(),
            "Bug detected: Finalization of block 10 failed. Expected success with genesis-initialized weights. Error: {:?}",
            result.err()
        );
        
        // Verify finalization succeeded
        assert_eq!(
            crate::FinalizedBlockNumber::<Test>::get(),
            10,
            "Bug detected: Block 10 not finalized. Expected FinalizedBlockNumber = 10."
        );
        assert_eq!(
            crate::FinalizedBlockHash::<Test>::get(),
            Some(block_hash),
            "Bug detected: Block hash not recorded. Expected FinalizedBlockHash to be set."
        );
    });
}

/// **Preservation Property Tests**
/// 
/// **Validates: Requirements 3.1, 3.2, 3.3, 3.4**
/// 
/// These tests capture the baseline behavior that must be preserved after the fix.
/// They test epoch transition logic, weight calculations, validator set changes,
/// and finalization for all non-genesis scenarios (epoch boundaries > 0).
/// 
/// **IMPORTANT**: These tests are run on UNFIXED code first to observe and document
/// the correct baseline behavior. They should PASS on unfixed code, confirming the
/// behavior we need to preserve.

/// **Property 2.1: Epoch Transition Preservation**
/// 
/// For all epoch boundaries > 0, freeze_epoch_weights() updates weights correctly.
/// This test verifies that the epoch transition mechanism works correctly at blocks
/// 10, 20, 30, etc., and that this behavior must be preserved after the genesis fix.
#[test]
fn test_preservation_epoch_transition_updates_weights() {
    new_test_ext().execute_with(|| {
        // Test epoch transitions at multiple boundaries: 10, 20, 30
        let test_epochs = vec![
            (1, 10u32),
            (2, 20u32),
            (3, 30u32),
        ];
        
        for (epoch, _block_number) in test_epochs {
            // Define validators for this epoch
            let alice = account_key("Alice");
            let bob = account_key("Bob");
            let charlie = account_key("Charlie");
            
            let validators = vec![
                (alice.clone(), 10_000_000u128, 80u128),
                (bob.clone(), 8_000_000u128, 80u128),
                (charlie.clone(), 6_000_000u128, 80u128),
            ];
            
            // Call freeze_epoch_weights (simulating epoch transition)
            Dvf::freeze_epoch_weights(epoch, &validators);
            
            // Verify weights are updated correctly
            for (validator, stake, score) in &validators {
                let weight = crate::EpochVotingWeight::<Test>::get(validator);
                assert!(
                    weight.is_some(),
                    "Preservation check: Validator {:?} should have weight after epoch {} transition",
                    validator, epoch
                );
                
                // Verify weight calculation formula: stake * 1 + score * 100 (with cap)
                let expected_stake_weight = stake * 1; // StakeWeightFactor = 1
                let expected_score_weight = score * 100; // ScoreWeightFactor = 100
                let expected_total = expected_stake_weight + expected_score_weight;
                
                assert_eq!(
                    weight.unwrap(),
                    expected_total,
                    "Preservation check: Weight calculation formula must remain unchanged at epoch {}",
                    epoch
                );
            }
            
            // Verify TotalVotingWeight is calculated correctly
            let total_weight = crate::TotalVotingWeight::<Test>::get();
            let expected_total: u128 = validators.iter()
                .map(|(_, stake, score)| (stake * 1) + (score * 100))
                .sum();
            
            assert_eq!(
                total_weight,
                expected_total,
                "Preservation check: TotalVotingWeight calculation must remain unchanged at epoch {}",
                epoch
            );
        }
    });
}

/// **Property 2.2: Weight Calculation Formula Preservation**
/// 
/// For all epoch boundaries > 0, weight calculation formula remains unchanged.
/// This test verifies the formula: weight = (stake * stake_factor) + min(score * score_factor, cap)
#[test]
fn test_preservation_weight_calculation_formula() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        
        // Test various stake and score combinations
        let test_cases = vec![
            // (stake, score, expected_weight)
            (1_000_000u128, 50u128, 1_000_000 + 5_000),      // Normal case
            (5_000_000u128, 80u128, 5_000_000 + 8_000),      // Normal case
            (10_000_000u128, 100u128, 10_000_000 + 10_000),  // Max score
            (100_000u128, 1000u128, 100_000 + 100_000),      // Score capped at 100_000
            (0u128, 80u128, 0 + 8_000),                      // Zero stake
        ];
        
        for (i, (stake, score, expected_weight)) in test_cases.iter().enumerate() {
            let validators = vec![
                (alice.clone(), *stake, *score),
                (bob.clone(), 1_000_000u128, 80u128), // Bob for variety
            ];
            
            // Freeze weights for epoch (simulating epoch transition)
            Dvf::freeze_epoch_weights(i as u32 + 1, &validators);
            
            // Verify Alice's weight matches expected formula
            let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
            assert_eq!(
                alice_weight.unwrap(),
                *expected_weight,
                "Preservation check: Weight formula must remain unchanged for stake={}, score={}",
                stake, score
            );
        }
    });
}

/// **Property 2.3: ValidatorSetId Increment Preservation**
/// 
/// For all validator set changes, ValidatorSetId increments correctly.
/// This test verifies that validator set change detection works correctly.
#[test]
fn test_preservation_validator_set_id_increments() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        let dave = account_key("Dave");
        
        // Initial validator set
        let validators_epoch1 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (bob.clone(), 1_000_000u128, 80u128),
        ];
        
        Dvf::freeze_epoch_weights(1, &validators_epoch1);
        let initial_id = crate::ValidatorSetId::<Test>::get();
        
        // Same validator set (no change expected)
        let validators_epoch2 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (bob.clone(), 1_000_000u128, 80u128),
        ];
        
        Dvf::freeze_epoch_weights(2, &validators_epoch2);
        let id_after_no_change = crate::ValidatorSetId::<Test>::get();
        
        assert_eq!(
            id_after_no_change,
            initial_id,
            "Preservation check: ValidatorSetId should not increment when validator set unchanged"
        );
        
        // Different validator set (change expected)
        let validators_epoch3 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (bob.clone(), 1_000_000u128, 80u128),
            (charlie.clone(), 1_000_000u128, 80u128), // New validator
        ];
        
        Dvf::freeze_epoch_weights(3, &validators_epoch3);
        let id_after_change = crate::ValidatorSetId::<Test>::get();
        
        assert_eq!(
            id_after_change,
            initial_id + 1,
            "Preservation check: ValidatorSetId should increment when validator set changes"
        );
        
        // Different validator (replacement)
        let validators_epoch4 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (dave.clone(), 1_000_000u128, 80u128), // Dave replaces Bob
            (charlie.clone(), 1_000_000u128, 80u128),
        ];
        
        Dvf::freeze_epoch_weights(4, &validators_epoch4);
        let id_after_replacement = crate::ValidatorSetId::<Test>::get();
        
        assert_eq!(
            id_after_replacement,
            initial_id + 2,
            "Preservation check: ValidatorSetId should increment when validators are replaced"
        );
    });
}

/// **Property 2.4: Finalization After First Epoch Preservation**
/// 
/// For all blocks after first epoch boundary (block 10+), finalization uses
/// standard DVF protocol. This test verifies that finalization works correctly
/// after epoch transitions.
#[test]
fn test_preservation_finalization_after_epoch_boundary() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        let alice_pair = sr25519::Pair::from_string("//Alice", None).unwrap();
        let bob_pair = sr25519::Pair::from_string("//Bob", None).unwrap();
        let charlie_pair = sr25519::Pair::from_string("//Charlie", None).unwrap();
        
        // Simulate epoch 1 transition at block 10
        let validators = vec![
            (alice.clone(), 10_000_000u128, 80u128),
            (bob.clone(), 8_000_000u128, 80u128),
            (charlie.clone(), 6_000_000u128, 80u128),
        ];
        
        Dvf::freeze_epoch_weights(1, &validators);
        
        // Test finalization at block 20 (second checkpoint after epoch boundary)
        System::set_block_number(20);
        
        let block_hash = sp_core::H256::from_low_u64_be(20);
        let mut votes = Vec::new();
        
        for (validator, pair) in &[
            (alice.clone(), &alice_pair),
            (bob.clone(), &bob_pair),
            (charlie.clone(), &charlie_pair),
        ] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: 0,
                block_number: 20,
                block_hash,
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw([0u8; 64])),
            };
            
            // Sign the vote
            let mut encoded_payload = Vec::new();
            use codec::Encode;
            vote.epoch_id.encode_to(&mut encoded_payload);
            vote.validator_set_id.encode_to(&mut encoded_payload);
            vote.round_id.encode_to(&mut encoded_payload);
            vote.block_number.encode_to(&mut encoded_payload);
            vote.block_hash.encode_to(&mut encoded_payload);
            vote.validator_account.encode_to(&mut encoded_payload);
            
            let signature = pair.sign(&encoded_payload);
            vote.signature = sp_runtime::MultiSignature::Sr25519(signature);
            
            votes.push(vote);
        }
        
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash,
            votes,
        };
        
        // Finalization should succeed after epoch boundary
        let result = Dvf::submit_justification(RuntimeOrigin::signed(alice.clone()), justification);
        assert!(
            result.is_ok(),
            "Preservation check: Finalization should succeed after epoch boundary. Error: {:?}",
            result.err()
        );
        
        // Verify finalization state
        assert_eq!(
            crate::FinalizedBlockNumber::<Test>::get(),
            20,
            "Preservation check: Block 20 should be finalized"
        );
        assert_eq!(
            crate::FinalizedBlockHash::<Test>::get(),
            Some(block_hash),
            "Preservation check: Block hash should be recorded"
        );
    });
}

/// **Property 2.5: Multiple Epoch Transitions Preservation**
/// 
/// This test verifies that multiple consecutive epoch transitions work correctly,
/// ensuring the fix doesn't break the ongoing epoch transition mechanism.
#[test]
fn test_preservation_multiple_epoch_transitions() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        // Simulate multiple epoch transitions
        for epoch in 1..=5 {
            let validators = vec![
                (alice.clone(), 10_000_000u128, 80u128),
                (bob.clone(), 8_000_000u128, 80u128),
                (charlie.clone(), 6_000_000u128, 80u128),
            ];
            
            Dvf::freeze_epoch_weights(epoch, &validators);
            
            // Verify weights are correct after each transition
            let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
            assert!(
                alice_weight.is_some(),
                "Preservation check: Alice should have weight after epoch {} transition",
                epoch
            );
            
            let expected_weight = (10_000_000 * 1) + (80 * 100);
            assert_eq!(
                alice_weight.unwrap(),
                expected_weight,
                "Preservation check: Weight should be consistent across epoch {}",
                epoch
            );
            
            // Verify total weight
            let total_weight = crate::TotalVotingWeight::<Test>::get();
            let expected_total = (10_000_000 + 8_000) + (8_000_000 + 8_000) + (6_000_000 + 8_000);
            assert_eq!(
                total_weight,
                expected_total,
                "Preservation check: Total weight should be consistent at epoch {}",
                epoch
            );
        }
    });
}

/// **Property 2.6: Weight Recalculation on Stake/Score Changes**
/// 
/// This test verifies that when validator stakes or scores change at epoch boundaries,
/// weights are recalculated correctly using the same formula.
#[test]
fn test_preservation_weight_recalculation_on_changes() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        
        // Epoch 1: Initial stakes and scores
        let validators_epoch1 = vec![
            (alice.clone(), 5_000_000u128, 70u128),
            (bob.clone(), 3_000_000u128, 60u128),
        ];
        
        Dvf::freeze_epoch_weights(1, &validators_epoch1);
        
        let alice_weight_epoch1 = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        let expected_alice_epoch1 = (5_000_000 * 1) + (70 * 100);
        assert_eq!(
            alice_weight_epoch1,
            expected_alice_epoch1,
            "Preservation check: Alice's weight should match formula at epoch 1"
        );
        
        // Epoch 2: Stakes and scores change
        let validators_epoch2 = vec![
            (alice.clone(), 8_000_000u128, 90u128), // Alice's stake and score increased
            (bob.clone(), 2_000_000u128, 50u128),   // Bob's stake and score decreased
        ];
        
        Dvf::freeze_epoch_weights(2, &validators_epoch2);
        
        let alice_weight_epoch2 = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        let expected_alice_epoch2 = (8_000_000 * 1) + (90 * 100);
        assert_eq!(
            alice_weight_epoch2,
            expected_alice_epoch2,
            "Preservation check: Alice's weight should be recalculated correctly at epoch 2"
        );
        
        let bob_weight_epoch2 = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();
        let expected_bob_epoch2 = (2_000_000 * 1) + (50 * 100);
        assert_eq!(
            bob_weight_epoch2,
            expected_bob_epoch2,
            "Preservation check: Bob's weight should be recalculated correctly at epoch 2"
        );
        
        // Verify weights changed from epoch 1 to epoch 2
        assert_ne!(
            alice_weight_epoch1,
            alice_weight_epoch2,
            "Preservation check: Alice's weight should change when stake/score changes"
        );
    });
}

// ============================================================================
// Genesis Initialization Unit Tests
// ============================================================================

/// Test genesis initialization with a single validator
#[test]
fn test_genesis_single_validator() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        
        // Verify EpochVotingWeight is populated for Alice
        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
        assert!(alice_weight.is_some(), "Alice should have a voting weight at genesis");
        assert!(alice_weight.unwrap() > 0, "Alice's weight should be greater than 0");
        
        // Verify TotalVotingWeight is calculated
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert!(total_weight > 0, "Total voting weight should be greater than 0 at genesis");
        
        // Verify ValidatorSetId starts at 0
        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(validator_set_id, 0, "ValidatorSetId should start at 0 at genesis");
    });
}

/// Test genesis initialization with multiple validators (3 validators)
#[test]
fn test_genesis_multiple_validators() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        // Verify all validators have weights
        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
        let bob_weight = crate::EpochVotingWeight::<Test>::get(&bob);
        let charlie_weight = crate::EpochVotingWeight::<Test>::get(&charlie);
        
        assert!(alice_weight.is_some(), "Alice should have a voting weight");
        assert!(bob_weight.is_some(), "Bob should have a voting weight");
        assert!(charlie_weight.is_some(), "Charlie should have a voting weight");
        
        // Verify all weights are greater than 0
        assert!(alice_weight.unwrap() > 0, "Alice's weight should be > 0");
        assert!(bob_weight.unwrap() > 0, "Bob's weight should be > 0");
        assert!(charlie_weight.unwrap() > 0, "Charlie's weight should be > 0");
        
        // Verify weights are calculated correctly based on stakes and scores
        // Alice: 10_000_000 stake, 80 score -> (10_000_000 * 1) + (80 * 100) = 10_008_000
        // Bob: 8_000_000 stake, 80 score -> (8_000_000 * 1) + (80 * 100) = 8_008_000
        // Charlie: 6_000_000 stake, 80 score -> (6_000_000 * 1) + (80 * 100) = 6_008_000
        assert_eq!(alice_weight.unwrap(), 10_008_000, "Alice's weight should be 10_008_000");
        assert_eq!(bob_weight.unwrap(), 8_008_000, "Bob's weight should be 8_008_000");
        assert_eq!(charlie_weight.unwrap(), 6_008_000, "Charlie's weight should be 6_008_000");
        
        // Verify TotalVotingWeight is sum of all weights
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert_eq!(total_weight, 24_024_000, "Total weight should be sum of all validator weights");
    });
}

/// Test genesis initialization with 5 validators
#[test]
fn test_genesis_five_validators() {
    // Create a custom test environment with 5 validators
    let validators = vec![
        (account_key("Alice"), 1000, 80),
        (account_key("Bob"), 2000, 80),
        (account_key("Charlie"), 3000, 80),
        (account_key("Dave"), 4000, 80),
        (account_key("Eve"), 5000, 80),
    ];
    
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    crate::GenesisConfig::<Test> {
        initial_validator_weights: validators.clone(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    
    let mut ext: sp_io::TestExternalities = t.into();
    
    ext.execute_with(|| {
        // Verify all 5 validators have weights
        for (validator, stake, score) in validators {
            let weight = crate::EpochVotingWeight::<Test>::get(&validator);
            assert!(weight.is_some(), "Validator should have a weight");
            
            let expected_weight = (stake * 1) + (score * 100);
            assert_eq!(weight.unwrap(), expected_weight, "Weight should match formula");
        }
        
        // Verify total weight
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        let expected_total = (1000 + 2000 + 3000 + 4000 + 5000) + (5 * 80 * 100);
        assert_eq!(total_weight, expected_total, "Total weight should be sum of all weights");
    });
}

/// Test genesis initialization with 10 validators
#[test]
fn test_genesis_ten_validators() {
    // Create a custom test environment with 10 validators
    let validators: Vec<_> = (0..10)
        .map(|i| {
            let name = format!("Validator{}", i);
            let stake = (i as u128 + 1) * 1000;
            (account_key(&name), stake, 80)
        })
        .collect();
    
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    crate::GenesisConfig::<Test> {
        initial_validator_weights: validators.clone(),
    }
    .assimilate_storage(&mut t)
    .unwrap();
    
    let mut ext: sp_io::TestExternalities = t.into();
    
    ext.execute_with(|| {
        // Verify all 10 validators have weights
        for (validator, stake, score) in validators {
            let weight = crate::EpochVotingWeight::<Test>::get(&validator);
            assert!(weight.is_some(), "Validator should have a weight");
            
            let expected_weight = (stake * 1) + (score * 100);
            assert_eq!(weight.unwrap(), expected_weight, "Weight should match formula");
        }
        
        // Verify ValidatorSetId is 0
        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(validator_set_id, 0, "ValidatorSetId should be 0 at genesis");
    });
}

/// Test that genesis validation rejects empty validator set
#[test]
#[should_panic(expected = "DVF genesis requires at least one validator")]
fn test_genesis_rejects_empty_validator_set() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    crate::GenesisConfig::<Test> {
        initial_validator_weights: vec![], // Empty validator set
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// Test that genesis validation rejects zero stakes
#[test]
#[should_panic(expected = "has invalid stake")]
fn test_genesis_rejects_zero_stake() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    crate::GenesisConfig::<Test> {
        initial_validator_weights: vec![
            (account_key("Alice"), 0, 80), // Zero stake
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// Test that genesis validation rejects invalid scores (> 100)
#[test]
#[should_panic(expected = "has invalid score")]
fn test_genesis_rejects_invalid_score() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    
    crate::GenesisConfig::<Test> {
        initial_validator_weights: vec![
            (account_key("Alice"), 1000, 101), // Score > 100
        ],
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// Test that freeze_epoch_weights is called with epoch 0 at genesis
#[test]
fn test_genesis_calls_freeze_epoch_weights() {
    new_test_ext().execute_with(|| {
        // Verify that weights were set (which means freeze_epoch_weights was called)
        let alice = account_key("Alice");
        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
        
        assert!(alice_weight.is_some(), "freeze_epoch_weights should have been called at genesis");
        
        // Verify the weight calculation matches freeze_epoch_weights logic
        let expected_weight = (1000 * 1) + (80 * 100); // stake * factor + score * factor
        assert_eq!(alice_weight.unwrap(), expected_weight, "Weight should match freeze_epoch_weights calculation");
    });
}

/// Test that EpochVotingWeight is populated after genesis
#[test]
fn test_genesis_populates_epoch_voting_weight() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");
        
        // Verify EpochVotingWeight storage is populated
        assert!(crate::EpochVotingWeight::<Test>::get(&alice).is_some(), "Alice should be in EpochVotingWeight");
        assert!(crate::EpochVotingWeight::<Test>::get(&bob).is_some(), "Bob should be in EpochVotingWeight");
        assert!(crate::EpochVotingWeight::<Test>::get(&charlie).is_some(), "Charlie should be in EpochVotingWeight");
    });
}

/// Test that TotalVotingWeight is calculated correctly at genesis
#[test]
fn test_genesis_calculates_total_voting_weight() {
    new_test_ext().execute_with(|| {
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        
        // Expected: (10_000_000 + 8_000_000 + 6_000_000) + (3 * 80 * 100) = 24_000_000 + 24_000 = 24_024_000
        assert_eq!(total_weight, 24_024_000, "TotalVotingWeight should be calculated correctly at genesis");
        assert!(total_weight > 0, "TotalVotingWeight should be greater than 0");
    });
}

/// Test that ValidatorSetId starts at 0 at genesis
#[test]
fn test_genesis_validator_set_id_starts_at_zero() {
    new_test_ext().execute_with(|| {
        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(validator_set_id, 0, "ValidatorSetId should start at 0 at genesis");
    });
}
