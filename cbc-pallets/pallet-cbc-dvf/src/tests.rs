use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::{sr25519, Pair};

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
