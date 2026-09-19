//! Partition recovery and safe halt above one third outage.
//!
//! Nine validators run in one process. The test takes nodes out of the network
//! and puts them back. Two outcomes matter.
//!
//! 1. **Three of nine partitioned.** Six validators remain. Six of nine carry
//!    two thirds of the Trust Score weight, which crosses the share a routine
//!    domain needs. The network keeps committing.
//! 2. **Four of nine down.** Five validators remain. Five of nine fall below
//!    the share, so no domain commits. The network halts and stays consistent.
//!    Commitment resumes as soon as the absent validators return.
//!
//! The invariant that matters most runs through every step. No two conflicting
//! blocks commit at one height. The share a domain needs sits above half the
//! weight, so two signer sets that each cross it share at least one member. A
//! validator signs one block per slot, so two blocks at one height never both
//! commit.

use cerulea_node::test_harness::{SlotOutcome, TestNetwork, HARNESS_ROUTINE_PERCENT};

use cerulea_consensus::engines::Slot;

/// The number of validators the partition test runs.
const VALIDATORS: usize = 9;

/// The number of validators the network survives losing.
const SURVIVABLE_OUTAGE: usize = 3;

/// The number of validators whose loss halts commitment.
const HALTING_OUTAGE: usize = 4;

#[test]
fn the_share_a_routine_domain_needs_sits_above_half_the_weight() {
    // Two signer sets that each cross the share share at least one member.
    assert!(HARNESS_ROUTINE_PERCENT > 50);
    // Six of nine cross the share. Five of nine fall below it.
    assert!(6 * 100 >= 9 * HARNESS_ROUTINE_PERCENT as u32);
    assert!(5 * 100 < 9 * HARNESS_ROUTINE_PERCENT as u32);
}

#[test]
fn three_of_nine_partitioned_keeps_the_network_committing() {
    let mut network = TestNetwork::with_validators(VALIDATORS);

    for slot_number in 1..=3u64 {
        assert!(network.run_slot(Slot(slot_number)).committed());
    }

    for index in 0..SURVIVABLE_OUTAGE {
        network.partition(index);
    }
    assert_eq!(network.reachable_count(), VALIDATORS - SURVIVABLE_OUTAGE);

    let mut committed_under_partition = 0;
    for slot_number in 4..=12u64 {
        match network.run_slot(Slot(slot_number)) {
            SlotOutcome::Committed { signed_percent, .. } => {
                committed_under_partition += 1;
                assert!(
                    signed_percent >= HARNESS_ROUTINE_PERCENT,
                    "six of nine carry {} percent of the weight",
                    signed_percent
                );
            }
            SlotOutcome::NoAuthor => {
                // Every rank of this slot belongs to a partitioned node. The
                // slot passes with no block, which is the rank timeout doing
                // its work. The next slot draws a fresh committee.
            }
            other => panic!("the network stopped committing: {:?}", other),
        }
    }

    assert!(
        committed_under_partition >= 5,
        "the network kept committing under the outage, reaching {} blocks",
        committed_under_partition
    );
    assert!(network.conflicting_commit().is_none());
}

#[test]
fn four_of_nine_down_halts_commitment_and_the_return_resumes_it() {
    let mut network = TestNetwork::with_validators(VALIDATORS);

    for slot_number in 1..=3u64 {
        assert!(network.run_slot(Slot(slot_number)).committed());
    }
    let height_before = network.height();
    let committed_before = network.nodes[VALIDATORS - 1].committed.len();

    for index in 0..HALTING_OUTAGE {
        network.partition(index);
    }
    assert_eq!(network.reachable_count(), VALIDATORS - HALTING_OUTAGE);

    let mut withheld = 0;
    for slot_number in 4..=12u64 {
        match network.run_slot(Slot(slot_number)) {
            SlotOutcome::Withheld { signed_percent, .. } => {
                withheld += 1;
                assert!(
                    signed_percent < HARNESS_ROUTINE_PERCENT,
                    "five of nine carry {} percent of the weight",
                    signed_percent
                );
            }
            SlotOutcome::NoAuthor => {}
            SlotOutcome::Committed { .. } => {
                panic!("the network committed while more than one third stayed down")
            }
        }
    }
    assert!(
        withheld > 0,
        "the network held blocks back during the outage"
    );

    // Safety held. Nothing new committed while the outage lasted.
    for node in network.nodes.iter().filter(|node| node.reachable) {
        assert_eq!(
            node.committed.len(),
            committed_before,
            "{} committed nothing during the outage",
            node.name
        );
    }
    assert!(network.height() > height_before, "blocks kept arriving");
    assert!(network.conflicting_commit().is_none());

    // The absent validators return and commitment resumes.
    for index in 0..HALTING_OUTAGE {
        network.restore(index);
    }
    assert_eq!(network.reachable_count(), VALIDATORS);

    let mut resumed = 0;
    for slot_number in 13..=20u64 {
        match network.run_slot(Slot(slot_number)) {
            SlotOutcome::Committed { signed_percent, .. } => {
                resumed += 1;
                assert_eq!(signed_percent, 100, "every validator signed again");
            }
            SlotOutcome::NoAuthor => {}
            other => panic!("commitment failed to resume: {:?}", other),
        }
    }

    assert!(
        resumed > 0,
        "commitment resumed once the validators returned"
    );
    assert!(network.conflicting_commit().is_none());
}

#[test]
fn a_partitioned_node_catches_up_and_agrees_with_the_rest() {
    let mut network = TestNetwork::with_validators(VALIDATORS);

    for slot_number in 1..=2u64 {
        assert!(network.run_slot(Slot(slot_number)).committed());
    }

    network.partition(0);
    for slot_number in 3..=8u64 {
        let outcome = network.run_slot(Slot(slot_number));
        assert!(
            outcome.committed() || outcome == SlotOutcome::NoAuthor,
            "the remaining validators carried on: {:?}",
            outcome
        );
    }

    let reachable_head = network
        .nodes
        .iter()
        .find(|node| node.reachable)
        .map(|node| node.head_hash())
        .expect("the network holds reachable nodes");
    assert_ne!(
        network.nodes[0].head_hash(),
        reachable_head,
        "the partitioned node fell behind"
    );

    network.restore(0);
    assert_eq!(
        network.nodes[0].head_hash(),
        reachable_head,
        "the returning node caught up with the rest"
    );
    assert!(network.conflicting_commit().is_none());
}

#[test]
fn no_two_conflicting_blocks_commit_at_one_height() {
    let mut network = TestNetwork::with_validators(VALIDATORS);

    // A run that partitions and restores repeatedly still commits one block per
    // height at most.
    for slot_number in 1..=24u64 {
        if slot_number % 5 == 0 {
            network.partition((slot_number as usize / 5) % VALIDATORS);
        }
        if slot_number % 7 == 0 {
            for index in 0..VALIDATORS {
                network.restore(index);
            }
        }
        let _ = network.run_slot(Slot(slot_number));
        assert!(
            network.conflicting_commit().is_none(),
            "two conflicting blocks committed at one height after slot {}",
            slot_number
        );
    }

    for (height, hashes) in network.accepted.iter() {
        assert_eq!(
            hashes.len(),
            1,
            "height {} accepted more than one block",
            height
        );
    }
}
