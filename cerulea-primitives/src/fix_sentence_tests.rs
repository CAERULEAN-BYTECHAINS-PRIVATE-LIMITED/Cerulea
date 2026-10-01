//! Tests of the plain fix sentences the shared primitives carry.
//!
//! A node operator reads these sentences in the log when a peer sends a
//! message the node refuses. Each test drives a real refusal through the
//! public check and asserts the sentence the operator reads, word for word.

use crate::bounds::{admit, price, MessagePath};
use crate::checkpoint::{
    accepts_block, accepts_reorganisation, checkpoint_due, checkpoint_index, follows,
    highest_anchored_block, Checkpoint, CheckpointFault,
};
use crate::equivocation::{
    verify_equivocation, EquivocationFault, EquivocationProof, SealClaimBytes, SealedHead,
    DRAW_KEY_OFFSET, RANK_OFFSET, SEAL_CLAIM_BYTES, SEAL_KEY_OFFSET, SLOT_OFFSET,
};
use alloc::vec::Vec;
use sp_core::ed25519::Pair;
use sp_core::Pair as PairTrait;

/// Encode one seal claim the way the Authoring Engine encodes it.
fn claim(slot: u64, rank: u8, seal_key: [u8; 32], draw_key: [u8; 32]) -> SealClaimBytes {
    let mut bytes = [0u8; SEAL_CLAIM_BYTES];
    bytes[SLOT_OFFSET..SLOT_OFFSET + 8].copy_from_slice(&slot.to_le_bytes());
    bytes[RANK_OFFSET] = rank;
    bytes[SEAL_KEY_OFFSET..SEAL_KEY_OFFSET + 32].copy_from_slice(&seal_key);
    bytes[DRAW_KEY_OFFSET..DRAW_KEY_OFFSET + 32].copy_from_slice(&draw_key);
    bytes
}

/// Seal one header claim with the named signer over the named seal key.
fn sealed(signer: &Pair, seal_key: [u8; 32], pre_seal_hash: [u8; 32], slot: u64) -> SealedHead {
    let mut head = SealedHead::new(
        pre_seal_hash,
        claim(slot, 1, seal_key, [5u8; 32]),
        [0u8; 64],
    );
    head.signature = signer.sign(&head.payload()).0;
    head
}

fn anchors() -> Vec<Checkpoint> {
    let mut set: Vec<Checkpoint> = Vec::new();
    for index in 0..3u32 {
        let next = Checkpoint::new(
            index,
            (index + 1) * 900,
            [index as u8 + 1; 32],
            [index as u8 + 40; 32],
        );
        follows(set.last(), &next).expect("each anchor follows the one before it");
        set.push(next);
    }
    set
}

#[test]
fn an_oversized_commit_vote_names_its_size_its_limit_and_the_fix() {
    let fault = admit(MessagePath::CommitVote, 257).expect_err("the vote crosses its bound");
    assert_eq!(
        fault.plain_language(),
        "The commit vote carries 257 bytes and the limit is 256. \
         Fix: send a smaller message on that path."
    );
    assert_eq!(price(MessagePath::CommitVote, 257), Err(fault));
}

#[test]
fn an_oversized_blueprint_artefact_names_the_blueprint_in_its_sentence() {
    let limit = MessagePath::BlueprintArtefact.max_bytes();
    let fault = admit(MessagePath::BlueprintArtefact, limit + 10).expect_err("over the bound");
    assert_eq!(
        fault.plain_language(),
        "The Blueprint artefact carries 8388618 bytes and the limit is 8388608. \
         Fix: send a smaller message on that path."
    );
}

#[test]
fn an_admitted_seal_pays_its_base_and_its_bytes() {
    let admitted = admit(MessagePath::BlockSeal, 300).expect("three hundred bytes fit");
    assert_eq!(admitted.cost, 20_000 + 100 * 300);
    assert_eq!(price(MessagePath::BlockSeal, 300), Ok(50_000));
    assert_eq!(MessagePath::BlockSeal.ceiling_cost(), 20_000 + 100 * 1_024);
}

#[test]
fn every_path_carries_its_own_label_in_the_logs() {
    let labels: Vec<&str> = MessagePath::ALL.iter().map(|path| path.label()).collect();
    assert_eq!(
        labels,
        [
            "block seal",
            "commit vote",
            "commit submission",
            "equivocation report",
            "propagation message",
            "custody answer",
            "bridge proof",
            "extrinsic body",
            "remote procedure call body",
            "Blueprint artefact",
        ]
    );
}

#[test]
fn two_headers_for_two_slots_carry_the_identity_sentence() {
    let pair = Pair::from_seed(&[21u8; 32]);
    let key = pair.public().0;
    let proof = EquivocationProof::new(
        sealed(&pair, key, [1u8; 32], 4),
        sealed(&pair, key, [2u8; 32], 5),
    );
    let fault = verify_equivocation(&proof).expect_err("two slots prove nothing");
    assert_eq!(fault, EquivocationFault::IdentityMismatch);
    let sentence = fault.plain_language();
    assert!(
        sentence.starts_with("The two sealed headers name a different slot"),
        "{sentence}"
    );
    assert!(
        sentence.contains("Fix: report two headers sealed by one validator for one slot"),
        "{sentence}"
    );
}

#[test]
fn one_header_reported_twice_carries_the_one_header_sentence() {
    let pair = Pair::from_seed(&[22u8; 32]);
    let head = sealed(&pair, pair.public().0, [3u8; 32], 8);
    let fault = verify_equivocation(&EquivocationProof::new(head, head))
        .expect_err("one header proves nothing");
    assert_eq!(fault, EquivocationFault::OneHeaderOnly);
    assert_eq!(
        fault.plain_language(),
        "The two sealed headers carry one pre-seal hash, so they are one header. \
         Fix: report two headers that differ."
    );
}

#[test]
fn a_first_header_sealed_by_a_stranger_carries_the_first_signature_sentence() {
    let author = Pair::from_seed(&[23u8; 32]);
    let stranger = Pair::from_seed(&[24u8; 32]);
    let key = author.public().0;
    let proof = EquivocationProof::new(
        sealed(&stranger, key, [1u8; 32], 6),
        sealed(&author, key, [2u8; 32], 6),
    );
    let fault = verify_equivocation(&proof).expect_err("the stranger holds no seal key");
    assert_eq!(fault, EquivocationFault::FirstSignatureRejected);
    assert_eq!(
        fault.plain_language(),
        "The first seal signature failed the check against the named validator key. \
         Fix: report the signature the author placed on that header."
    );
}

#[test]
fn a_second_header_sealed_by_a_stranger_carries_the_second_signature_sentence() {
    let author = Pair::from_seed(&[25u8; 32]);
    let stranger = Pair::from_seed(&[26u8; 32]);
    let key = author.public().0;
    let proof = EquivocationProof::new(
        sealed(&author, key, [1u8; 32], 7),
        sealed(&stranger, key, [2u8; 32], 7),
    );
    let fault = verify_equivocation(&proof).expect_err("the stranger holds no seal key");
    assert_eq!(fault, EquivocationFault::SecondSignatureRejected);
    assert_eq!(
        fault.plain_language(),
        "The second seal signature failed the check against the named validator key. \
         Fix: report the signature the author placed on that header."
    );
}

#[test]
fn a_block_disagreeing_with_an_anchor_carries_the_mismatch_sentence() {
    let set = anchors();
    assert_eq!(accepts_block(&set, 1_800, &[2u8; 32]), Ok(()));
    assert_eq!(accepts_block(&set, 1_801, &[9u8; 32]), Ok(()));
    let fault = accepts_block(&set, 1_800, &[9u8; 32]).expect_err("another block at 1800");
    assert_eq!(fault, CheckpointFault::HashMismatch);
    assert_eq!(
        fault.plain_language(),
        "The chain offered names another block at an anchored checkpoint height. \
         Fix: sync from a peer whose history matches the anchored checkpoints."
    );
}

#[test]
fn a_reorganisation_below_the_top_anchor_carries_the_below_anchor_sentence() {
    let set = anchors();
    assert_eq!(highest_anchored_block(&set), 2_700);
    assert_eq!(accepts_reorganisation(&set, 2_700), Ok(()));
    let fault = accepts_reorganisation(&set, 2_699).expect_err("the ancestor sits below");
    assert_eq!(fault, CheckpointFault::BelowAnchor);
    assert_eq!(
        fault.plain_language(),
        "The chain offered would replace blocks below the highest anchored checkpoint. \
         Fix: sync from a peer that builds above the anchored checkpoints."
    );
}

#[test]
fn an_anchor_out_of_index_order_carries_the_order_sentence() {
    let set = anchors();
    let skipped = Checkpoint::new(4, 4_500, [7u8; 32], [8u8; 32]);
    let fault = follows(set.last(), &skipped).expect_err("index three is missing");
    assert_eq!(fault, CheckpointFault::OutOfOrder);
    assert_eq!(
        fault.plain_language(),
        "The checkpoint carries an index out of order with the anchored set. \
         Fix: anchor checkpoints in index order starting from zero."
    );
    let late_start = Checkpoint::new(1, 900, [1u8; 32], [1u8; 32]);
    assert_eq!(follows(None, &late_start), Err(CheckpointFault::OutOfOrder));
}

#[test]
fn an_anchor_below_the_block_before_it_carries_the_block_order_sentence() {
    let set = anchors();
    let behind = Checkpoint::new(3, 2_700, [7u8; 32], [8u8; 32]);
    let fault = follows(set.last(), &behind).expect_err("block 2700 is already settled");
    assert_eq!(fault, CheckpointFault::BlockOutOfOrder);
    assert_eq!(
        fault.plain_language(),
        "The checkpoint names a block at or below the checkpoint before it. \
         Fix: anchor each checkpoint above the block the one before it settles."
    );
}

#[test]
fn a_deployment_anchoring_nothing_raises_no_checkpoint() {
    assert!(!checkpoint_due(900, 0));
    assert_eq!(checkpoint_index(900, 0), 0);
    assert!(!checkpoint_due(0, 900));
    assert!(checkpoint_due(1_800, 900));
    assert_eq!(checkpoint_index(1_800, 900), 2);
    assert_eq!(highest_anchored_block(&[]), 0);
}
