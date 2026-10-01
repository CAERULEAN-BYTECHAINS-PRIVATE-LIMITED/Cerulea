use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use sp_core::{sr25519, Pair};
use sp_runtime::BuildStorage;

/// The epoch commit weight the Commit Engine gives one validator.
///
/// The commit weight is the Trust Score. The Commit Engine compares aggregated
/// weight against a share of the total, so each validator carries the share of
/// the epoch Trust Score total it holds, scaled to
/// [`crate::VOTE_WEIGHT_SCALE`]. This helper repeats the engine rule so each
/// assertion below names the rule it protects.
fn engine_share(trust: u128, total_trust: u128) -> u128 {
    trust * crate::VOTE_WEIGHT_SCALE / total_trust
}

/// The sum of every share across one validator set.
fn engine_total(trust: &[u128]) -> u128 {
    let total: u128 = trust.iter().sum();
    trust.iter().map(|held| engine_share(*held, total)).sum()
}

/// The Trust Score of each validator the mock names at genesis.
///
/// Every figure reads as a share of one in parts per billion. Alice carries
/// eight tenths. Bob carries sixty four hundredths. Charlie carries forty
/// eight hundredths.
const MOCK_TRUST: [u128; 3] = [800_000_000, 640_000_000, 480_000_000];

#[test]
fn test_is_checkpoint_block() {
    new_test_ext().execute_with(|| {
        // Block 0 is a checkpoint (0 % 10 == 0)
        assert!(Registry::is_checkpoint_block(0));

        // Block 10 is a checkpoint
        assert!(Registry::is_checkpoint_block(10));

        // Block 20 is a checkpoint
        assert!(Registry::is_checkpoint_block(20));

        // Block 5 is not a checkpoint
        assert!(!Registry::is_checkpoint_block(5));

        // Block 15 is not a checkpoint
        assert!(!Registry::is_checkpoint_block(15));

        // Block 99 is not a checkpoint
        assert!(!Registry::is_checkpoint_block(99));
    });
}

#[test]
fn test_is_block_finalized() {
    new_test_ext().execute_with(|| {
        // Initially, no blocks are finalized (finalized block number is 0)
        // Block 0 is finalized by default since it's the genesis
        assert!(Registry::is_block_finalized(0));

        // Blocks after 0 should not be finalized initially
        assert!(!Registry::is_block_finalized(10));
        assert!(!Registry::is_block_finalized(20));

        // Simulate finalization of block 20
        crate::FinalizedBlockNumber::<Test>::put(20);

        // All blocks up to 20 should be finalized
        assert!(Registry::is_block_finalized(0));
        assert!(Registry::is_block_finalized(10));
        assert!(Registry::is_block_finalized(20));

        // Blocks after 20 should not be finalized
        assert!(!Registry::is_block_finalized(21));
        assert!(!Registry::is_block_finalized(30));
    });
}

#[test]
fn test_get_finality_info() {
    new_test_ext().execute_with(|| {
        // Initially, no blocks are finalized
        let info = Registry::get_finality_info(10);
        assert!(!info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, None);
        assert_eq!(info.finalized_checkpoint_hash, None);

        // Simulate finalization of block 20
        let block_hash = sp_core::H256::from_low_u64_be(20);
        crate::FinalizedBlockNumber::<Test>::put(20);
        crate::FinalizedBlockHash::<Test>::put(block_hash);

        // Block 10 should be finalized by checkpoint 10
        let info = Registry::get_finality_info(10);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(10));
        assert_eq!(info.finalized_checkpoint_hash, Some(block_hash));

        // Block 15 should be finalized by checkpoint 10
        let info = Registry::get_finality_info(15);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(10));

        // Block 20 should be finalized by checkpoint 20
        let info = Registry::get_finality_info(20);
        assert!(info.is_finalized);
        assert_eq!(info.finalized_by_checkpoint, Some(20));

        // Block 25 should not be finalized
        let info = Registry::get_finality_info(25);
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create a vote for a non-checkpoint block (block 15)
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: validator.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
            Registry::submit_dvf_vote(RuntimeOrigin::signed(validator.clone()), vote.clone()),
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
        assert_ok!(Registry::submit_dvf_vote(
            RuntimeOrigin::signed(validator),
            vote
        ));
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create a vote for a non-checkpoint block
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: validator.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
            Registry::submit_dvf_vote(RuntimeOrigin::signed(validator.clone()), vote.clone()),
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create votes for checkpoint block 10
        let block_number = 10u32;
        let block_hash = sp_core::H256::from_low_u64_be(10);
        let round = 0u32;

        let mut votes = Vec::new();

        for (validator, pair) in &[
            (alice.clone(), &alice_pair),
            (bob.clone(), &bob_pair),
            (charlie.clone(), &charlie_pair),
        ] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: round,
                block_number,
                block_hash,
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(
                    sp_core::sr25519::Signature::from_raw([0u8; 64]),
                ),
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
        assert_ok!(Registry::submit_justification(
            RuntimeOrigin::none(),
            justification
        ));

        // Verify finality state was updated
        assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
        assert_eq!(crate::FinalizedBlockHash::<Test>::get(), Some(block_hash));
        assert_eq!(crate::CurrentRound::<Test>::get(), 1);

        // The commitment record carries the aggregated share of the three
        // signers. Each holds a third of the epoch Trust Score total, so each
        // carries a third of the commit scale.
        let trust = [100u128, 100u128, 100u128];
        System::assert_has_event(
            crate::Event::BlockFinalized {
                block_number: 10,
                block_hash,
                round: 0,
                weight: engine_total(&trust),
            }
            .into(),
        );
    });
}

/// The Commit Engine takes the commitment record as an unsigned submission.
///
/// The record carries the signature of every voter, so the record
/// authenticates itself. A signed origin therefore carries no meaning and the
/// call refuses it. Submit the record from the unsigned origin instead.
#[test]
fn a_signed_commitment_record_is_refused() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: Vec::new(),
        };

        assert_noop!(
            Registry::submit_justification(RuntimeOrigin::signed(alice), justification),
            sp_runtime::DispatchError::BadOrigin
        );
    });
}

#[test]
fn test_submit_justification_empty_votes() {
    new_test_ext().execute_with(|| {
        // Create empty justification
        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: Vec::new(),
        };

        // Submit should fail
        assert_noop!(
            Registry::submit_justification(RuntimeOrigin::none(), justification),
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create vote for non-checkpoint block 15
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 15,
            block_hash: sp_core::H256::from_low_u64_be(15),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
            Registry::submit_justification(RuntimeOrigin::none(), justification),
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
        Registry::freeze_epoch_weights(1, &validators);

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
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
            Registry::submit_justification(RuntimeOrigin::none(), justification),
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create two votes from the same validator
        let mut vote1 = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 10,
            block_hash: sp_core::H256::from_low_u64_be(10),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
        vote1.signature = sp_runtime::MultiSignature::Sr25519(signature);

        let mut vote2 = vote1.clone();
        vote2.signature = sp_runtime::MultiSignature::Sr25519(signature);

        let justification = crate::DvfJustification {
            round_number: 0,
            block_hash: sp_core::H256::from_low_u64_be(10),
            votes: vec![vote1, vote2],
        };

        // Submit should fail
        assert_noop!(
            Registry::submit_justification(RuntimeOrigin::none(), justification),
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
        Registry::freeze_epoch_weights(1, &validators);

        // Create vote from only one validator (not enough for 2/3 threshold)
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: 0,
            round_id: 0,
            block_number: 10,
            block_hash: sp_core::H256::from_low_u64_be(10),
            validator_account: alice.clone(),
            signature: sp_runtime::MultiSignature::Sr25519(sp_core::sr25519::Signature::from_raw(
                [0u8; 64],
            )),
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
            Registry::submit_justification(RuntimeOrigin::none(), justification),
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
        Registry::freeze_epoch_weights(1, &validators);

        // Submit votes for round 0
        for (validator, pair) in &[(alice.clone(), &alice_pair), (bob.clone(), &bob_pair)] {
            let mut vote = crate::DvfVote {
                epoch_id: 1,
                validator_set_id: 0,
                round_id: 0,
                block_number: 10,
                block_hash: sp_core::H256::from_low_u64_be(10),
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(
                    sp_core::sr25519::Signature::from_raw([0u8; 64]),
                ),
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

            assert_ok!(Registry::submit_dvf_vote(
                RuntimeOrigin::signed(validator.clone()),
                vote
            ));
        }

        // Verify votes are recorded
        assert!(crate::VoteRecords::<Test>::contains_key(0, &alice));
        assert!(crate::VoteRecords::<Test>::contains_key(0, &bob));

        // Create and submit justification to trigger finalization and pruning
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
                block_number: 10,
                block_hash: sp_core::H256::from_low_u64_be(10),
                validator_account: validator.clone(),
                signature: sp_runtime::MultiSignature::Sr25519(
                    sp_core::sr25519::Signature::from_raw([0u8; 64]),
                ),
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

        assert_ok!(Registry::submit_justification(
            RuntimeOrigin::none(),
            justification
        ));

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
    // Create a test environment that simulates genesis with validators set
    // This mimics what would happen in a real chain genesis with DCF validators
    let mut ext = new_test_ext();

    ext.execute_with(|| {
        // Simulate genesis block
        System::set_block_number(0);

        // Define validators that would be set at genesis (from DCF pallet)
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");

        // Each entry names the stake the validator bonded and the Trust Score
        // the Standing Engine composed for it, as a share of one in parts per
        // billion. The mock genesis names the same three validators.
        let genesis_validators = vec![
            (alice.clone(), 10_000_000u128, MOCK_TRUST[0]),
            (bob.clone(), 8_000_000u128, MOCK_TRUST[1]),
            (charlie.clone(), 6_000_000u128, MOCK_TRUST[2]),
        ];

        // **EXPECTED BEHAVIOR 2.1**: EpochVotingWeight SHOULD be populated at genesis
        // On unfixed code, this will be empty because there's no GenesisConfig
        for (validator, stake, trust) in &genesis_validators {
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

            // The Commit Engine gives each validator its share of the epoch
            // Trust Score total, scaled to the fixed commit scale.
            let total_trust: u128 = genesis_validators.iter().map(|(_, _, held)| *held).sum();
            let expected_total = engine_share(*trust, total_trust);
            assert_eq!(
                weight_value, expected_total,
                "Validator {:?} carries a share the Commit Engine did not set. Expected {}, found {}",
                validator, expected_total, weight_value
            );
            let _ = stake;
        }

        // **EXPECTED BEHAVIOR 2.3**: TotalVotingWeight SHOULD be > 0 at genesis
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert!(
            total_weight > 0,
            "Bug detected: TotalVotingWeight is 0 at genesis. Expected sum of all validator weights."
        );

        // The total is the sum of every share across the set.
        let trust: Vec<u128> = genesis_validators.iter().map(|(_, _, held)| *held).collect();
        let expected_total = engine_total(&trust);
        assert_eq!(
            total_weight, expected_total,
            "The epoch total reads {} where the Commit Engine sets {}",
            total_weight, expected_total
        );
        assert!(
            expected_total <= crate::VOTE_WEIGHT_SCALE,
            "The epoch total stays inside the commit scale"
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
        let result = Registry::submit_justification(RuntimeOrigin::none(), justification);
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
                (alice.clone(), 10_000_000u128, MOCK_TRUST[0]),
                (bob.clone(), 8_000_000u128, MOCK_TRUST[1]),
                (charlie.clone(), 6_000_000u128, MOCK_TRUST[2]),
            ];

            // Call freeze_epoch_weights (simulating epoch transition)
            Registry::freeze_epoch_weights(epoch, &validators);

            // Verify weights are updated correctly
            for (validator, stake, trust) in &validators {
                let weight = crate::EpochVotingWeight::<Test>::get(validator);
                assert!(
                    weight.is_some(),
                    "Preservation check: Validator {:?} should have weight after epoch {} transition",
                    validator, epoch
                );

                // The Commit Engine share rule holds at every epoch boundary.
                let total_trust: u128 = validators.iter().map(|(_, _, held)| *held).sum();
                let expected_total = engine_share(*trust, total_trust);

                assert_eq!(
                    weight.unwrap(),
                    expected_total,
                    "The Commit Engine share rule moved at epoch {}",
                    epoch
                );
                let _ = stake;
            }

            // The epoch total is the sum of every share.
            let total_weight = crate::TotalVotingWeight::<Test>::get();
            let trust: Vec<u128> = validators.iter().map(|(_, _, held)| *held).collect();
            let expected_total = engine_total(&trust);

            assert_eq!(
                total_weight,
                expected_total,
                "The epoch total moved away from the sum of the shares at epoch {}",
                epoch
            );
        }
    });
}

/// The Commit Engine share rule holds across every Trust Score pattern.
///
/// The engine gives a validator the share of the epoch Trust Score total it
/// holds. A validator holding half the total carries half the scale. A
/// validator holding nothing carries nothing. Equal Trust Scores carry equal
/// shares. Stake holds still across the whole run, so the rule answers to the
/// Trust Score alone.
#[test]
fn the_commit_engine_share_follows_the_trust_score_pattern() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");

        // Each pair reads as the Trust Score of Alice beside the Trust Score
        // of Bob, in parts per billion.
        let patterns = vec![
            (100_000_000u128, 100_000_000u128),
            (500_000_000u128, 100_000_000u128),
            (1_000_000_000u128, 100_000_000u128),
            (10_000_000u128, 100_000_000u128),
            (0u128, 100_000_000u128),
        ];

        for (index, (alice_trust, bob_trust)) in patterns.iter().enumerate() {
            let validators = vec![
                (alice.clone(), 5_000_000u128, *alice_trust),
                (bob.clone(), 5_000_000u128, *bob_trust),
            ];

            Registry::freeze_epoch_weights(index as u32 + 1, &validators);

            let total_trust = alice_trust + bob_trust;
            let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
            let bob_weight = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();

            assert_eq!(
                alice_weight,
                engine_share(*alice_trust, total_trust),
                "The share of Alice left the Commit Engine rule at Trust Score {}",
                alice_trust
            );
            assert_eq!(
                bob_weight,
                engine_share(*bob_trust, total_trust),
                "The share of Bob left the Commit Engine rule at Trust Score {}",
                bob_trust
            );
            assert_eq!(
                crate::TotalVotingWeight::<Test>::get(),
                alice_weight + bob_weight,
                "The epoch total reads the sum of the two shares"
            );
        }

        // Equal Trust Scores carry equal shares, and the pair reaches the
        // scale. The two stakes differ, and the shares hold even.
        let even = vec![
            (alice.clone(), 9_000u128, 400_000_000u128),
            (bob.clone(), 4_000u128, 400_000_000u128),
        ];
        Registry::freeze_epoch_weights(99, &even);
        let alice_even = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        let bob_even = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();
        assert_eq!(alice_even, bob_even);
        assert_eq!(alice_even + bob_even, crate::VOTE_WEIGHT_SCALE);
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

        Registry::freeze_epoch_weights(1, &validators_epoch1);
        let initial_id = crate::ValidatorSetId::<Test>::get();

        // Same validator set (no change expected)
        let validators_epoch2 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (bob.clone(), 1_000_000u128, 80u128),
        ];

        Registry::freeze_epoch_weights(2, &validators_epoch2);
        let id_after_no_change = crate::ValidatorSetId::<Test>::get();

        assert_eq!(
            id_after_no_change, initial_id,
            "Preservation check: ValidatorSetId should not increment when validator set unchanged"
        );

        // Different validator set (change expected)
        let validators_epoch3 = vec![
            (alice.clone(), 1_000_000u128, 80u128),
            (bob.clone(), 1_000_000u128, 80u128),
            (charlie.clone(), 1_000_000u128, 80u128), // New validator
        ];

        Registry::freeze_epoch_weights(3, &validators_epoch3);
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

        Registry::freeze_epoch_weights(4, &validators_epoch4);
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
/// standard Registry protocol. This test verifies that finalization works correctly
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

        Registry::freeze_epoch_weights(1, &validators);

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
                signature: sp_runtime::MultiSignature::Sr25519(
                    sp_core::sr25519::Signature::from_raw([0u8; 64]),
                ),
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
        let result = Registry::submit_justification(RuntimeOrigin::none(), justification);
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
                (alice.clone(), 10_000_000u128, MOCK_TRUST[0]),
                (bob.clone(), 8_000_000u128, MOCK_TRUST[1]),
                (charlie.clone(), 6_000_000u128, MOCK_TRUST[2]),
            ];

            Registry::freeze_epoch_weights(epoch, &validators);

            // Verify weights are correct after each transition
            let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
            assert!(
                alice_weight.is_some(),
                "Preservation check: Alice should have weight after epoch {} transition",
                epoch
            );

            let total_trust: u128 = MOCK_TRUST.iter().sum();
            assert_eq!(
                alice_weight.unwrap(),
                engine_share(MOCK_TRUST[0], total_trust),
                "Preservation check: Weight should be consistent across epoch {}",
                epoch
            );

            // Verify total weight
            let total_weight = crate::TotalVotingWeight::<Test>::get();
            assert_eq!(
                total_weight,
                engine_total(&MOCK_TRUST),
                "Preservation check: Total weight should be consistent at epoch {}",
                epoch
            );
        }
    });
}

/// **Property 2.6: Weight Recalculation on Trust Score Changes**
///
/// The commit weight follows the Trust Score. A Trust Score that moves at an
/// epoch boundary moves the weight with it. A stake that moves alone leaves
/// the weight where it stands.
#[test]
fn test_preservation_weight_recalculation_on_changes() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");

        // Epoch 1: the two validators enter with their first Trust Scores.
        let validators_epoch1 = vec![
            (alice.clone(), 5_000_000u128, 700_000_000u128),
            (bob.clone(), 3_000_000u128, 300_000_000u128),
        ];

        Registry::freeze_epoch_weights(1, &validators_epoch1);

        let alice_weight_epoch1 = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        assert_eq!(
            alice_weight_epoch1,
            engine_share(700_000_000, 700_000_000 + 300_000_000),
            "The share of Alice left the Commit Engine rule at epoch 1"
        );

        // Epoch 2: the Standing Engine composes new Trust Scores.
        let validators_epoch2 = vec![
            (alice.clone(), 8_000_000u128, 900_000_000u128),
            (bob.clone(), 2_000_000u128, 100_000_000u128),
        ];

        Registry::freeze_epoch_weights(2, &validators_epoch2);

        let alice_weight_epoch2 = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        assert_eq!(
            alice_weight_epoch2,
            engine_share(900_000_000, 900_000_000 + 100_000_000),
            "The share of Alice left the Commit Engine rule at epoch 2"
        );

        let bob_weight_epoch2 = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();
        assert_eq!(
            bob_weight_epoch2,
            engine_share(100_000_000, 900_000_000 + 100_000_000),
            "The share of Bob left the Commit Engine rule at epoch 2"
        );

        // The weight moved with the Trust Score.
        assert_ne!(
            alice_weight_epoch1, alice_weight_epoch2,
            "The share of Alice follows the Trust Score across epochs"
        );

        // Epoch 3: stake alone moves. The Trust Scores hold, so the shares
        // hold with them.
        let validators_epoch3 = vec![
            (alice.clone(), 1_000u128, 900_000_000u128),
            (bob.clone(), 90_000_000u128, 100_000_000u128),
        ];

        Registry::freeze_epoch_weights(3, &validators_epoch3);

        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(&alice).unwrap(),
            alice_weight_epoch2,
            "Stake moved alone, so the share of Alice holds"
        );
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(&bob).unwrap(),
            bob_weight_epoch2,
            "Stake moved alone, so the share of Bob holds"
        );
    });
}

/// Higher standing carries more commit weight at equal stake.
///
/// The Standing Engine composes the Trust Score. The Commit Engine reads that
/// score as the commit weight. Two validators bonding the same stake therefore
/// separate on standing alone, and the validator with the higher score carries
/// the larger share.
#[test]
fn higher_standing_carries_more_commit_weight_at_equal_stake() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");

        let equal_stake = 7_000_000u128;
        let validators = vec![
            (alice.clone(), equal_stake, 750_000_000u128),
            (bob.clone(), equal_stake, 250_000_000u128),
        ];

        Registry::freeze_epoch_weights(1, &validators);

        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        let bob_weight = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();

        assert!(
            alice_weight > bob_weight,
            "The higher standing carries the larger share at equal stake"
        );
        assert_eq!(
            alice_weight,
            engine_share(750_000_000, 1_000_000_000),
            "Alice carries three quarters of the commit scale"
        );
        assert_eq!(
            bob_weight,
            engine_share(250_000_000, 1_000_000_000),
            "Bob carries one quarter of the commit scale"
        );
        assert_eq!(alice_weight, bob_weight * 3);
    });
}

/// The class thresholds resolve against the epoch Trust Score total.
///
/// The Assurance Engine names four shares. Routine holds fifty one percent.
/// Financial holds sixty seven. Structural holds eighty. Constitutional holds
/// ninety. Each share resolves against the total the Commit Engine froze, and
/// that total is the sum of every Trust Score share.
#[test]
fn the_class_thresholds_resolve_against_the_trust_score_total() {
    use pallet_cerulea_dcf::engines::assurance::{
        required_weight, CONSTITUTIONAL_THRESHOLD_PERCENT, FINANCIAL_THRESHOLD_PERCENT,
        ROUTINE_THRESHOLD_PERCENT, STRUCTURAL_THRESHOLD_PERCENT,
    };

    assert_eq!(ROUTINE_THRESHOLD_PERCENT, 51);
    assert_eq!(FINANCIAL_THRESHOLD_PERCENT, 67);
    assert_eq!(STRUCTURAL_THRESHOLD_PERCENT, 80);
    assert_eq!(CONSTITUTIONAL_THRESHOLD_PERCENT, 90);

    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");

        // Every stake reads the same, so the shares separate on standing.
        let validators = vec![
            (alice.clone(), 4_000_000u128, 600_000_000u128),
            (bob.clone(), 4_000_000u128, 300_000_000u128),
            (charlie.clone(), 4_000_000u128, 100_000_000u128),
        ];
        Registry::freeze_epoch_weights(1, &validators);

        let total = crate::TotalVotingWeight::<Test>::get();
        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice).unwrap();
        let bob_weight = crate::EpochVotingWeight::<Test>::get(&bob).unwrap();
        let charlie_weight = crate::EpochVotingWeight::<Test>::get(&charlie).unwrap();

        // The epoch total is the sum of the three Trust Score shares.
        assert_eq!(total, alice_weight + bob_weight + charlie_weight);

        // Alice alone carries six tenths. Routine passes. Financial waits.
        assert!(alice_weight >= required_weight(total, ROUTINE_THRESHOLD_PERCENT));
        assert!(alice_weight < required_weight(total, FINANCIAL_THRESHOLD_PERCENT));

        // Alice beside Bob carries nine tenths, so every class through
        // constitutional passes.
        let pair = alice_weight + bob_weight;
        assert!(pair >= required_weight(total, FINANCIAL_THRESHOLD_PERCENT));
        assert!(pair >= required_weight(total, STRUCTURAL_THRESHOLD_PERCENT));
        assert!(pair >= required_weight(total, CONSTITUTIONAL_THRESHOLD_PERCENT));

        // Bob beside Charlie carries four tenths, so routine waits.
        let lower = bob_weight + charlie_weight;
        assert!(lower < required_weight(total, ROUTINE_THRESHOLD_PERCENT));
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
        assert!(
            alice_weight.is_some(),
            "Alice should have a voting weight at genesis"
        );
        assert!(
            alice_weight.unwrap() > 0,
            "Alice's weight should be greater than 0"
        );

        // Verify TotalVotingWeight is calculated
        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert!(
            total_weight > 0,
            "Total voting weight should be greater than 0 at genesis"
        );

        // Verify ValidatorSetId starts at 0
        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(
            validator_set_id, 0,
            "ValidatorSetId should start at 0 at genesis"
        );
    });
}

/// Genesis seeds the Commit Engine weight table for every validator named.
#[test]
fn genesis_seeds_a_share_for_every_named_validator() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let bob = account_key("Bob");
        let charlie = account_key("Charlie");

        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);
        let bob_weight = crate::EpochVotingWeight::<Test>::get(&bob);
        let charlie_weight = crate::EpochVotingWeight::<Test>::get(&charlie);

        assert!(alice_weight.is_some(), "Alice carries a share at genesis");
        assert!(bob_weight.is_some(), "Bob carries a share at genesis");
        assert!(
            charlie_weight.is_some(),
            "Charlie carries a share at genesis"
        );

        // The mock genesis names three Trust Scores. Each validator carries
        // the share of the epoch Trust Score total it holds.
        let total_trust: u128 = MOCK_TRUST.iter().sum();

        assert_eq!(
            alice_weight.unwrap(),
            engine_share(MOCK_TRUST[0], total_trust)
        );
        assert_eq!(
            bob_weight.unwrap(),
            engine_share(MOCK_TRUST[1], total_trust)
        );
        assert_eq!(
            charlie_weight.unwrap(),
            engine_share(MOCK_TRUST[2], total_trust)
        );

        // The share of Alice exceeds the share of Bob, which exceeds the
        // share of Charlie, in the order their Trust Scores run.
        assert!(alice_weight.unwrap() > bob_weight.unwrap());
        assert!(bob_weight.unwrap() > charlie_weight.unwrap());

        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert_eq!(total_weight, engine_total(&MOCK_TRUST));
    });
}

/// Genesis seeds a share for each of five validators.
#[test]
fn genesis_seeds_five_validators() {
    let validators = vec![
        (account_key("Alice"), 1000u128, 100_000_000u128),
        (account_key("Bob"), 2000u128, 200_000_000u128),
        (account_key("Charlie"), 3000u128, 300_000_000u128),
        (account_key("Dave"), 4000u128, 400_000_000u128),
        (account_key("Eve"), 5000u128, 500_000_000u128),
    ];

    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: validators.clone(),
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext: sp_io::TestExternalities = t.into();

    ext.execute_with(|| {
        let trust: Vec<u128> = validators.iter().map(|(_, _, held)| *held).collect();
        let total_trust: u128 = trust.iter().sum();

        for (validator, _stake, held) in &validators {
            let weight = crate::EpochVotingWeight::<Test>::get(validator);
            assert!(weight.is_some(), "Each validator carries a share");
            assert_eq!(weight.unwrap(), engine_share(*held, total_trust));
        }

        let total_weight = crate::TotalVotingWeight::<Test>::get();
        assert_eq!(total_weight, engine_total(&trust));
        assert!(total_weight <= crate::VOTE_WEIGHT_SCALE);
    });
}

/// Genesis seeds a share for each of ten validators.
#[test]
fn genesis_seeds_ten_validators() {
    let validators: Vec<_> = (0..10)
        .map(|i| {
            let name = format!("Validator{}", i);
            let stake = (i as u128 + 1) * 1000;
            let trust = (i as u128 + 1) * 90_000_000;
            (account_key(&name), stake, trust)
        })
        .collect();

    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: validators.clone(),
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext: sp_io::TestExternalities = t.into();

    ext.execute_with(|| {
        let total_trust: u128 = validators.iter().map(|(_, _, held)| *held).sum();

        for (validator, _stake, held) in &validators {
            let weight = crate::EpochVotingWeight::<Test>::get(validator);
            assert!(weight.is_some(), "Each validator carries a share");
            assert_eq!(weight.unwrap(), engine_share(*held, total_trust));
        }

        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(
            validator_set_id, 0,
            "The validator set identifier starts at zero"
        );
    });
}

/// A strict chain spec naming no validator stops at genesis.
#[test]
#[should_panic(expected = "Registry genesis requires at least one validator")]
fn genesis_rejects_a_strict_spec_naming_no_validator() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: vec![],
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// A deployment that admits every validator after genesis starts empty.
///
/// The Admission Engine governs entry to the validator set. A chain spec that
/// leaves the strict check off therefore builds from an empty weight table and
/// takes its validators through the engine afterwards.
#[test]
fn genesis_accepts_an_open_spec_naming_no_validator() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: vec![],
        strict_validation: false,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| {
        assert_eq!(crate::TotalVotingWeight::<Test>::get(), 0);
        assert_eq!(crate::ValidatorSetId::<Test>::get(), 0);

        // The Admission Engine admits a validator, and the Commit Engine
        // weight table follows at the next epoch boundary.
        let alice = account_key("Alice");
        Registry::freeze_epoch_weights(1, &[(alice.clone(), 1_000u128, 800_000_000u128)]);
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(&alice),
            Some(crate::VOTE_WEIGHT_SCALE)
        );
    });
}

/// Test that genesis validation rejects zero stakes
#[test]
#[should_panic(expected = "has invalid stake")]
fn test_genesis_rejects_zero_stake() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: vec![
            (account_key("Alice"), 0, 80), // Zero stake
        ],
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// Genesis refuses a Trust Score above the ceiling of one.
#[test]
#[should_panic(expected = "has invalid score")]
fn test_genesis_rejects_invalid_score() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: vec![(
            account_key("Alice"),
            1000,
            crate::TRUST_SCORE_CEILING + 1,
        )],
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();
}

/// Genesis accepts a Trust Score that reaches the ceiling of one.
#[test]
fn genesis_accepts_a_trust_score_at_the_ceiling() {
    let mut t = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();

    crate::GenesisConfig::<Test> {
        initial_validator_keys: Vec::new(),
        initial_validator_weights: vec![(account_key("Alice"), 1000, crate::TRUST_SCORE_CEILING)],
        strict_validation: true,
        validator_licence_key: None,
        required_licence_class: 0,
    }
    .assimilate_storage(&mut t)
    .unwrap();

    let mut ext: sp_io::TestExternalities = t.into();
    ext.execute_with(|| {
        assert_eq!(
            crate::EpochVotingWeight::<Test>::get(&account_key("Alice")),
            Some(crate::VOTE_WEIGHT_SCALE)
        );
    });
}

/// Genesis runs the epoch weight freeze for epoch zero.
#[test]
fn genesis_freezes_the_weights_of_epoch_zero() {
    new_test_ext().execute_with(|| {
        let alice = account_key("Alice");
        let alice_weight = crate::EpochVotingWeight::<Test>::get(&alice);

        assert!(
            alice_weight.is_some(),
            "Genesis runs the epoch weight freeze, so Alice carries a share"
        );

        let total_trust: u128 = MOCK_TRUST.iter().sum();
        assert_eq!(
            alice_weight.unwrap(),
            engine_share(MOCK_TRUST[0], total_trust)
        );
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
        assert!(
            crate::EpochVotingWeight::<Test>::get(&alice).is_some(),
            "Alice should be in EpochVotingWeight"
        );
        assert!(
            crate::EpochVotingWeight::<Test>::get(&bob).is_some(),
            "Bob should be in EpochVotingWeight"
        );
        assert!(
            crate::EpochVotingWeight::<Test>::get(&charlie).is_some(),
            "Charlie should be in EpochVotingWeight"
        );
    });
}

/// Genesis records the epoch total the Commit Engine reads.
#[test]
fn genesis_records_the_epoch_total() {
    new_test_ext().execute_with(|| {
        let total_weight = crate::TotalVotingWeight::<Test>::get();

        assert_eq!(total_weight, engine_total(&MOCK_TRUST));
        assert!(
            total_weight > 0,
            "The epoch total carries weight at genesis"
        );
        assert!(
            total_weight <= crate::VOTE_WEIGHT_SCALE,
            "The epoch total stays inside the commit scale"
        );
    });
}

/// Test that ValidatorSetId starts at 0 at genesis
#[test]
fn test_genesis_validator_set_id_starts_at_zero() {
    new_test_ext().execute_with(|| {
        let validator_set_id = crate::ValidatorSetId::<Test>::get();
        assert_eq!(
            validator_set_id, 0,
            "ValidatorSetId should start at 0 at genesis"
        );
    });
}

// ============================================================================
// Round timeout. A split vote closes its round and finality stays live.
// ============================================================================

mod round_timeout {
    use super::*;
    use frame_support::traits::Hooks;
    use sp_core::H256;

    /// The nine validator names the split vote tests use.
    const NINE: [&str; 9] = [
        "Round0", "Round1", "Round2", "Round3", "Round4", "Round5", "Round6", "Round7", "Round8",
    ];

    fn pair(name: &str) -> sr25519::Pair {
        sr25519::Pair::from_string(&format!("//{}", name), None).unwrap()
    }

    /// Freeze nine validators of equal Trust Score and equal stake.
    fn freeze_nine() -> Vec<AccountId> {
        let validators: Vec<(AccountId, u128, u128)> = NINE
            .iter()
            .map(|name| (account_key(name), 1_000_000u128, 500_000_000u128))
            .collect();
        Registry::freeze_epoch_weights(1, &validators);
        validators
            .into_iter()
            .map(|(account, _, _)| account)
            .collect()
    }

    /// A candidate block hash at one height.
    fn candidate(height: u32, choice: u8) -> H256 {
        H256::from_low_u64_be(((height as u64) << 8) | choice as u64)
    }

    /// Build one signed vote in the round the chain runs now.
    fn signed_vote(
        index: usize,
        round: u32,
        height: u32,
        hash: H256,
    ) -> crate::DvfVote<H256, AccountId, sp_runtime::MultiSignature> {
        use codec::Encode;
        let signer = pair(NINE[index]);
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: crate::ValidatorSetId::<Test>::get(),
            round_id: round,
            block_number: height,
            block_hash: hash,
            validator_account: account_key(NINE[index]),
            signature: sp_runtime::MultiSignature::Sr25519(sr25519::Signature::from_raw([0u8; 64])),
        };
        let mut payload = Vec::new();
        vote.epoch_id.encode_to(&mut payload);
        vote.validator_set_id.encode_to(&mut payload);
        vote.round_id.encode_to(&mut payload);
        vote.block_number.encode_to(&mut payload);
        vote.block_hash.encode_to(&mut payload);
        vote.validator_account.encode_to(&mut payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(signer.sign(&payload));
        vote
    }

    fn cast(index: usize, height: u32, hash: H256) -> sp_runtime::DispatchResult {
        let round = crate::CurrentRound::<Test>::get();
        let vote = signed_vote(index, round, height, hash);
        Registry::submit_dvf_vote(RuntimeOrigin::signed(account_key(NINE[index])), vote)
    }

    /// Move the chain forward one block and run the registry hook.
    fn step(block: u64) {
        System::set_block_number(block);
        Registry::on_initialize(block);
    }

    /// Every block the registry finalized, as (height, hash) pairs.
    fn finalized_events() -> Vec<(u64, H256)> {
        System::events()
            .into_iter()
            .filter_map(|record| match record.event {
                RuntimeEvent::Registry(crate::Event::BlockFinalized {
                    block_number,
                    block_hash,
                    ..
                }) => Some((block_number, block_hash)),
                _ => None,
            })
            .collect()
    }

    /// A split vote at nine validators closes its round within the timeout.
    ///
    /// Five validators vote for one block and four for another. Neither side
    /// reaches the sixty seven percent threshold. A second vote in the same
    /// round is refused. The round closes once the timeout runs out. Every
    /// validator then votes again in the next round and the block commits.
    #[test]
    fn a_split_vote_at_nine_validators_recovers_within_the_timeout() {
        new_test_ext().execute_with(|| {
            step(11);
            freeze_nine();
            let height = 10u32;
            let first = candidate(height, 1);
            let second = candidate(height, 2);

            for index in 0..5 {
                assert_ok!(cast(index, height, first));
            }
            for index in 5..9 {
                assert_ok!(cast(index, height, second));
            }
            assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 0);
            assert_eq!(crate::CurrentRound::<Test>::get(), 0);

            // One vote per round. The validators wait for the next round.
            assert_noop!(cast(5, height, first), Error::<Test>::DoubleVote);

            // The round holds until the timeout runs out.
            let timeout = crate::RoundTimeoutBlocks::<Test>::get() as u64;
            let deadline = height as u64 + timeout;
            for block in 12..deadline {
                step(block);
                assert_eq!(crate::CurrentRound::<Test>::get(), 0, "held at {}", block);
            }
            step(deadline);
            assert_eq!(crate::CurrentRound::<Test>::get(), 1);
            let threshold = Registry::threshold_weight();
            match crate::RoundOutcomes::<Test>::get(0) {
                Some(crate::RoundOutcome::TimedOut {
                    highest_weight,
                    threshold: recorded,
                }) => {
                    assert!(highest_weight < recorded);
                    assert_eq!(recorded, threshold);
                }
                other => panic!("round zero closed as {:?}", other),
            }
            assert_eq!(crate::VoteTallies::<Test>::get(first), 0);
            assert_eq!(crate::VoteTallies::<Test>::get(second), 0);

            // Every validator votes again on the leading candidate.
            for index in 0..9 {
                let _ = cast(index, height, first);
            }
            assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
            assert_eq!(crate::FinalizedBlockHash::<Test>::get(), Some(first));
            assert!(deadline <= height as u64 + timeout);
        });
    }

    /// Repeated splits keep the round counter moving.
    #[test]
    fn repeated_splits_keep_advancing() {
        new_test_ext().execute_with(|| {
            step(11);
            freeze_nine();
            let height = 10u32;
            let timeout = crate::RoundTimeoutBlocks::<Test>::get() as u64;
            let mut block = 11u64;

            for round in 0..4u32 {
                assert_eq!(crate::CurrentRound::<Test>::get(), round);
                // The split alternates, so each round leaves both candidates
                // below the threshold.
                for index in 0..9 {
                    let choice = if (index + round as usize) % 2 == 0 {
                        1
                    } else {
                        2
                    };
                    assert_ok!(cast(index, height, candidate(height, choice)));
                }
                assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 0);
                let opened = crate::RoundOpenedAt::<Test>::get();
                let closes_at = (opened + timeout).max(height as u64 + timeout);
                while crate::CurrentRound::<Test>::get() == round {
                    block += 1;
                    step(block);
                    assert!(block <= closes_at, "round {} held past its timeout", round);
                }
            }

            // The fifth round agrees and commits.
            for index in 0..9 {
                let _ = cast(index, height, candidate(height, 2));
            }
            assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
            assert_eq!(crate::CurrentRound::<Test>::get(), 5);
        });
    }

    /// A healthy round never closes on its timeout.
    #[test]
    fn a_round_that_commits_in_time_never_times_out() {
        new_test_ext().execute_with(|| {
            step(11);
            freeze_nine();
            for index in 0..9 {
                let _ = cast(index, 10, candidate(10, 1));
            }
            assert_eq!(crate::FinalizedBlockNumber::<Test>::get(), 10);
            assert_eq!(crate::CurrentRound::<Test>::get(), 1);
            for block in 12..25 {
                step(block);
            }
            assert_eq!(crate::CurrentRound::<Test>::get(), 1);
        });
    }

    /// A changed vote needs a recorded close below the threshold.
    #[test]
    fn a_changed_vote_needs_a_round_closed_below_the_threshold() {
        new_test_ext().execute_with(|| {
            step(11);
            freeze_nine();
            let first = candidate(10, 1);
            let second = candidate(10, 2);
            let voter = account_key(NINE[0]);
            crate::VoteRecords::<Test>::insert(0, &voter, signed_vote(0, 0, 10, first));
            crate::CurrentRound::<Test>::put(1);

            // No outcome recorded for round zero. The same block passes, a
            // different block at the same height is refused.
            assert!(Registry::vote_respects_earlier_rounds(
                1, &voter, 10, &first
            ));
            assert!(!Registry::vote_respects_earlier_rounds(
                1, &voter, 10, &second
            ));
            assert_noop!(cast(0, 10, second), Error::<Test>::ConflictingVote);

            // A block at another height stays open.
            assert!(Registry::vote_respects_earlier_rounds(
                1,
                &voter,
                20,
                &candidate(20, 1)
            ));

            // A recorded close below the threshold opens the change.
            crate::RoundOutcomes::<Test>::insert(
                0,
                crate::RoundOutcome::TimedOut {
                    highest_weight: 1,
                    threshold: 2,
                },
            );
            assert!(Registry::vote_respects_earlier_rounds(
                1, &voter, 10, &second
            ));
        });
    }

    /// The round timeout is a policy value root sets.
    #[test]
    fn the_round_timeout_is_a_policy_value() {
        new_test_ext().execute_with(|| {
            assert_eq!(
                crate::RoundTimeoutBlocks::<Test>::get(),
                crate::DEFAULT_ROUND_TIMEOUT_BLOCKS
            );
            assert_noop!(
                Registry::set_round_timeout(RuntimeOrigin::root(), 0),
                Error::<Test>::RoundTimeoutZero
            );
            assert!(
                Registry::set_round_timeout(RuntimeOrigin::signed(account_key("Alice")), 5)
                    .is_err()
            );
            assert_ok!(Registry::set_round_timeout(RuntimeOrigin::root(), 5));
            assert_eq!(crate::RoundTimeoutBlocks::<Test>::get(), 5);
        });
    }

    /// A justification naming a round the chain has yet to open is refused.
    #[test]
    fn a_justification_from_a_future_round_is_refused() {
        new_test_ext().execute_with(|| {
            step(11);
            freeze_nine();
            let hash = candidate(10, 1);
            let votes: Vec<_> = (0..9)
                .map(|index| signed_vote(index, 3, 10, hash))
                .collect();
            let justification = crate::DvfJustification {
                round_number: 3,
                block_hash: hash,
                votes,
            };
            assert_noop!(
                Registry::submit_justification(RuntimeOrigin::none(), justification),
                Error::<Test>::RoundMismatch
            );
        });
    }

    /// No two conflicting blocks commit at one height under any ordering.
    ///
    /// Each seed drives one ordering of votes, block steps and justifications
    /// built from recorded votes of any retained round. Every run checks four
    /// properties. One hash per committed height. The finalized head only
    /// rises. A round that timed out closed below the threshold. A committed
    /// round carried the threshold.
    #[test]
    fn no_conflicting_commitment_under_any_ordering() {
        for seed in 1..=160u64 {
            new_test_ext().execute_with(|| {
                let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
                let mut next = move |bound: u64| {
                    state ^= state << 13;
                    state ^= state >> 7;
                    state ^= state << 17;
                    state % bound
                };

                step(11);
                freeze_nine();
                let _ = Registry::set_round_timeout(RuntimeOrigin::root(), 1 + next(12) as u32);
                let mut block = 11u64;
                let mut last_head = 0u64;

                for _ in 0..400 {
                    let height = crate::FinalizedBlockNumber::<Test>::get() as u32 + 10;
                    match next(10) {
                        0..=5 => {
                            let index = next(9) as usize;
                            let choice = 1 + next(3) as u8;
                            let _ = cast(index, height, candidate(height, choice));
                        }
                        6..=8 => {
                            block += 1;
                            step(block);
                        }
                        _ => {
                            // Build a justification from the recorded votes of
                            // one retained round for one candidate.
                            let current = crate::CurrentRound::<Test>::get();
                            let round = current.saturating_sub(next(3) as u32);
                            let choice = 1 + next(3) as u8;
                            let hash = candidate(height, choice);
                            let votes: Vec<_> =
                                crate::VoteRecords::<Test>::iter_prefix_values(round)
                                    .filter(|vote| vote.block_hash == hash)
                                    .collect();
                            if !votes.is_empty() {
                                let _ = Registry::submit_justification(
                                    RuntimeOrigin::none(),
                                    crate::DvfJustification {
                                        round_number: round,
                                        block_hash: hash,
                                        votes,
                                    },
                                );
                            }
                        }
                    }

                    let head = crate::FinalizedBlockNumber::<Test>::get();
                    assert!(head >= last_head, "seed {}: the head fell", seed);
                    last_head = head;
                }

                let mut committed: std::collections::BTreeMap<u64, H256> = Default::default();
                for (height, hash) in finalized_events() {
                    if let Some(held) = committed.insert(height, hash) {
                        assert_eq!(held, hash, "seed {}: two blocks at height {}", seed, height);
                    }
                }
                for (_, outcome) in crate::RoundOutcomes::<Test>::iter() {
                    match outcome {
                        crate::RoundOutcome::TimedOut {
                            highest_weight,
                            threshold,
                        } => assert!(highest_weight < threshold, "seed {}", seed),
                        crate::RoundOutcome::Committed { weight, .. } => {
                            assert!(weight >= Registry::threshold_weight(), "seed {}", seed)
                        }
                    }
                }
            });
        }
    }
}

// ============================================================================
// One vote per validator per round, and vote equivocation evidence.
// ============================================================================

mod vote_safety {
    use super::*;
    use sp_core::H256;

    const NINE: [&str; 9] = [
        "Safety0", "Safety1", "Safety2", "Safety3", "Safety4", "Safety5", "Safety6", "Safety7",
        "Safety8",
    ];

    fn freeze_nine() {
        let validators: Vec<(AccountId, u128, u128)> = NINE
            .iter()
            .map(|name| (account_key(name), 1_000_000u128, 500_000_000u128))
            .collect();
        Registry::freeze_epoch_weights(1, &validators);
    }

    fn signed(
        index: usize,
        round: u32,
        height: u32,
        hash: H256,
    ) -> crate::DvfVote<H256, AccountId, sp_runtime::MultiSignature> {
        use codec::Encode;
        let pair = sr25519::Pair::from_string(&format!("//{}", NINE[index]), None).unwrap();
        let mut vote = crate::DvfVote {
            epoch_id: 1,
            validator_set_id: crate::ValidatorSetId::<Test>::get(),
            round_id: round,
            block_number: height,
            block_hash: hash,
            validator_account: account_key(NINE[index]),
            signature: sp_runtime::MultiSignature::Sr25519(sr25519::Signature::from_raw([0u8; 64])),
        };
        let mut payload = Vec::new();
        vote.epoch_id.encode_to(&mut payload);
        vote.validator_set_id.encode_to(&mut payload);
        vote.round_id.encode_to(&mut payload);
        vote.block_number.encode_to(&mut payload);
        vote.block_hash.encode_to(&mut payload);
        vote.validator_account.encode_to(&mut payload);
        vote.signature = sp_runtime::MultiSignature::Sr25519(pair.sign(&payload));
        vote
    }

    fn penalised() -> Vec<AccountId> {
        crate::mock::PENALISED.with(|held| held.borrow().clone())
    }

    /// Two votes one validator signed in one round prove an equivocation.
    #[test]
    fn a_conflicting_pair_resets_standing_and_records_the_fault() {
        new_test_ext().execute_with(|| {
            System::set_block_number(12);
            freeze_nine();
            let first = signed(2, 142, 1440, H256::repeat_byte(0xe8));
            let second = signed(2, 142, 1440, H256::repeat_byte(0xa3));
            assert_ok!(Registry::report_vote_equivocation(
                RuntimeOrigin::none(),
                first.clone(),
                second.clone()
            ));
            assert_eq!(penalised(), vec![account_key(NINE[2])]);
            let record = crate::VoteEquivocations::<Test>::get(account_key(NINE[2]))
                .expect("the fault is recorded");
            assert_eq!(record.round, 142);
            assert_eq!(record.first, (1440, H256::repeat_byte(0xe8)));
            assert_eq!(record.second, (1440, H256::repeat_byte(0xa3)));
            assert_eq!(record.epochs_destroyed, crate::mock::MOCK_EPOCHS_DESTROYED);
            System::assert_last_event(
                crate::Event::VoteEquivocationProven {
                    validator: account_key(NINE[2]),
                    round: 142,
                    epochs_destroyed: crate::mock::MOCK_EPOCHS_DESTROYED,
                }
                .into(),
            );

            // The same fault carries one penalty.
            assert_noop!(
                Registry::report_vote_equivocation(RuntimeOrigin::none(), first, second),
                Error::<Test>::VoteEquivocationAlreadyPenalised
            );
        });
    }

    /// A pair that proves nothing is refused.
    #[test]
    fn a_pair_that_proves_nothing_is_refused() {
        new_test_ext().execute_with(|| {
            freeze_nine();
            let vote = signed(1, 7, 70, H256::repeat_byte(1));
            // The same vote twice.
            assert_noop!(
                Registry::report_vote_equivocation(
                    RuntimeOrigin::none(),
                    vote.clone(),
                    vote.clone()
                ),
                Error::<Test>::VoteEquivocationUnproven
            );
            // Two rounds.
            assert_noop!(
                Registry::report_vote_equivocation(
                    RuntimeOrigin::none(),
                    vote.clone(),
                    signed(1, 8, 70, H256::repeat_byte(2))
                ),
                Error::<Test>::VoteEquivocationUnproven
            );
            // Two validators.
            assert_noop!(
                Registry::report_vote_equivocation(
                    RuntimeOrigin::none(),
                    vote.clone(),
                    signed(3, 7, 70, H256::repeat_byte(2))
                ),
                Error::<Test>::VoteEquivocationUnproven
            );
            // A forged signature.
            let mut forged = signed(1, 7, 70, H256::repeat_byte(2));
            forged.signature = signed(1, 7, 70, H256::repeat_byte(3)).signature;
            assert_noop!(
                Registry::report_vote_equivocation(RuntimeOrigin::none(), vote, forged),
                Error::<Test>::VoteEquivocationUnproven
            );
            assert!(penalised().is_empty());
        });
    }

    /// A commitment record built from a vote that conflicts with a recorded
    /// vote of the same round is refused.
    #[test]
    fn a_justification_with_a_conflicting_vote_is_refused() {
        new_test_ext().execute_with(|| {
            System::set_block_number(11);
            freeze_nine();
            let first = H256::repeat_byte(0xe8);
            let second = H256::repeat_byte(0xa3);
            // Validator zero voted for the first block in round zero.
            assert_ok!(Registry::submit_dvf_vote(
                RuntimeOrigin::signed(account_key(NINE[0])),
                signed(0, 0, 10, first)
            ));
            // A record for the second block carries a vote of validator zero.
            let votes: Vec<_> = (0..9).map(|index| signed(index, 0, 10, second)).collect();
            assert_noop!(
                Registry::submit_justification(
                    RuntimeOrigin::none(),
                    crate::DvfJustification {
                        round_number: 0,
                        block_hash: second,
                        votes,
                    }
                ),
                Error::<Test>::ConflictingVote
            );
            // Without the conflicting vote the record commits.
            let votes: Vec<_> = (1..9).map(|index| signed(index, 0, 10, second)).collect();
            assert_ok!(Registry::submit_justification(
                RuntimeOrigin::none(),
                crate::DvfJustification {
                    round_number: 0,
                    block_hash: second,
                    votes,
                }
            ));
            assert_eq!(crate::FinalizedBlockHash::<Test>::get(), Some(second));
        });
    }

    /// The commitment record keeps its votes, so a second record in the same
    /// round naming another block for the same validators is refused.
    #[test]
    fn a_second_record_in_one_round_is_refused() {
        new_test_ext().execute_with(|| {
            System::set_block_number(21);
            freeze_nine();
            let first = H256::repeat_byte(0x11);
            let votes: Vec<_> = (0..9).map(|index| signed(index, 0, 10, first)).collect();
            assert_ok!(Registry::submit_justification(
                RuntimeOrigin::none(),
                crate::DvfJustification {
                    round_number: 0,
                    block_hash: first,
                    votes,
                }
            ));
            // The round moved on. A record naming round zero again for a
            // different block at a later height is refused.
            let other = H256::repeat_byte(0x22);
            let votes: Vec<_> = (0..9).map(|index| signed(index, 0, 20, other)).collect();
            assert_noop!(
                Registry::submit_justification(
                    RuntimeOrigin::none(),
                    crate::DvfJustification {
                        round_number: 0,
                        block_hash: other,
                        votes,
                    }
                ),
                Error::<Test>::ConflictingVote
            );
        });
    }
}
