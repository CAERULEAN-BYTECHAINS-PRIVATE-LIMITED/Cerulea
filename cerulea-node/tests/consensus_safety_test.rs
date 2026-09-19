//! The invariant that matters most.
//!
//! No two conflicting blocks finalise at the same height. Ever. Under any
//! partition, any outage and any adversarial input. A failure here halts the
//! build regardless of anything else passing.
//!
//! The assertion rests on one arithmetic fact and one rule the Commit Engine
//! holds. The share of Trust Score weight a domain needs sits above half the
//! set weight. A validator signs one block per slot. Two signer sets that each
//! cross half the weight share at least one member, and that member signed one
//! block, so two blocks at one height never both reach the share.
//!
//! This file states the fact, then drives the engines through every fault the
//! chaos programme injects and asserts the invariant after every slot.

use std::collections::{BTreeMap, BTreeSet};

use cerulea_node::fault_harness::{FaultNetwork, FaultProfile};
use cerulea_node::test_harness::{SlotOutcome, TestNetwork, HARNESS_ROUTINE_PERCENT};

use cerulea_consensus::engines::commit::commit::{DomainCommitter, ThresholdTable};
use cerulea_consensus::engines::{CommitEngine, Slot, TransactionClass, TrustScore};

/// The validator count every case in this file runs.
const VALIDATORS: usize = 9;

/// The number of slots each case drives.
const SLOTS: u64 = 24;

#[test]
fn the_share_every_class_needs_sits_above_half_the_weight() {
    let table = ThresholdTable::new(HARNESS_ROUTINE_PERCENT, 67, 80, 90);
    for class in [
        TransactionClass::Routine,
        TransactionClass::Financial,
        TransactionClass::Structural,
        TransactionClass::Constitutional,
    ] {
        let share = table.percent_for(class);
        assert!(
            share > 50,
            "the {:?} class needs {} percent, which sits at or below half the weight. \
             Fix: raise the share above half so two signer sets always share a member.",
            class,
            share
        );
    }
}

#[test]
fn two_signer_sets_that_each_reach_the_share_share_a_member() {
    // Nine validators of equal Trust Score. Any set crossing sixty six percent
    // holds at least six members. Two sets of six out of nine overlap.
    let standing = TrustScore::from_percent(50);
    let total: u128 = standing.weight() * VALIDATORS as u128;
    let table = ThresholdTable::new(HARNESS_ROUTINE_PERCENT, 67, 80, 90);

    let mut smallest_reaching = VALIDATORS + 1;
    for size in 1..=VALIDATORS {
        let weight = standing.weight() * size as u128;
        if weight * 100 >= total * table.percent_for(TransactionClass::Routine) as u128 {
            smallest_reaching = size;
            break;
        }
    }
    assert!(smallest_reaching <= VALIDATORS);
    assert!(
        smallest_reaching * 2 > VALIDATORS,
        "two sets of {} out of {} would fit side by side without sharing a member",
        smallest_reaching,
        VALIDATORS
    );
}

#[test]
fn one_validator_signing_twice_never_carries_two_blocks_over_the_share() {
    // A validator that signs two blocks at one height gives each block its
    // weight once. The Commit Engine counts a signer once per block, so the
    // second block still needs a set reaching the share on its own.
    let mut network = TestNetwork::with_validators(VALIDATORS);
    assert!(network.run_slot(Slot(1)).committed());

    let slot = Slot(2);
    let total = network.total_weight();
    let table = ThresholdTable::new(HARNESS_ROUTINE_PERCENT, 67, 80, 90);

    let first_hash = [1u8; 32];
    let second_hash = [2u8; 32];
    let mut first = DomainCommitter::open(slot, first_hash, total, table).expect("weight");
    let mut second = DomainCommitter::open(slot, second_hash, total, table).expect("weight");

    // Six validators sign the first block. The remaining three sign the
    // second, and one of the six signs the second as well.
    for node in network.nodes.iter().take(6) {
        first
            .add_signer(node.seal_key, node.standing)
            .expect("the signer joins the tally");
    }
    for node in network.nodes.iter().skip(6) {
        second
            .add_signer(node.seal_key, node.standing)
            .expect("the signer joins the tally");
    }
    second
        .add_signer(network.nodes[0].seal_key, network.nodes[0].standing)
        .expect("the double signer joins the second tally");

    assert!(first.settle(0, TransactionClass::Routine).committed);
    assert!(
        !second.settle(0, TransactionClass::Routine).committed,
        "the second block reached {} percent and still committed. \
         Fix: hold the share above half the weight.",
        second.signed_percent()
    );
}

/// Every fault profile the safety assertion runs under.
fn profiles() -> Vec<(&'static str, FaultProfile)> {
    vec![
        ("a healthy network", FaultProfile::healthy()),
        ("three validators lost", FaultProfile::losing(&[0, 1, 2])),
        ("four validators lost", FaultProfile::losing(&[0, 1, 2, 3])),
        (
            "four signatures arriving late",
            FaultProfile::delaying(&[0, 1, 2, 3], 5_000),
        ),
        (
            "two validators holding no storage",
            FaultProfile::filling_storage(&[7, 8]),
        ),
        (
            "three clocks running ahead",
            FaultProfile::skewing(&[0, 1, 2], 30_000),
        ),
        (
            "three clocks running behind",
            FaultProfile::skewing(&[3, 4, 5], -30_000),
        ),
        (
            "loss, delay, storage and skew together",
            FaultProfile::losing(&[0])
                .with(&FaultProfile::delaying(&[1, 2], 5_000))
                .with(&FaultProfile::filling_storage(&[8]))
                .with(&FaultProfile::skewing(&[3], 30_000)),
        ),
    ]
}

#[test]
fn no_two_conflicting_blocks_finalise_under_any_fault() {
    for (name, profile) in profiles() {
        let mut network = FaultNetwork::new(VALIDATORS, profile);
        for slot_number in 1..=SLOTS {
            network.run_slot(Slot(slot_number));
            network.assert_no_conflicting_finalisation(name);
        }
        // Every height that finalised carries one block on every node.
        let mut finalised: BTreeMap<u32, BTreeSet<[u8; 32]>> = BTreeMap::new();
        for node in network.network.nodes.iter() {
            for (height, hash) in node.committed.iter() {
                finalised.entry(*height).or_default().insert(*hash);
            }
        }
        for (height, hashes) in finalised.iter() {
            assert_eq!(
                hashes.len(),
                1,
                "height {} finalised {} blocks under {}",
                height,
                hashes.len(),
                name
            );
        }
    }
}

#[test]
fn a_partition_that_heals_finalises_one_block_at_every_height() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=6u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("the healthy opening");
    }

    network.inject(&FaultProfile::losing(&[0, 1, 2, 3]));
    for slot_number in 7..=16u64 {
        let report = network.run_slot(Slot(slot_number));
        assert!(
            !report.outcome.committed(),
            "five of nine committed a block while four stayed away"
        );
        network.assert_no_conflicting_finalisation("the outage");
    }

    network.heal();
    for slot_number in 17..=28u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("the recovery");
    }

    assert!(
        network.committed_count() > 6,
        "commitment resumed after the partition healed"
    );
}

#[test]
fn the_assertion_names_the_height_where_two_blocks_finalise() {
    // The assertion is worth only what its failure says. A network holding two
    // conflicting records at one height reports that height by name.
    let mut network = FaultNetwork::healthy(3);
    network.run_slot(Slot(1));
    network.network.nodes[0].committed.insert(99, [7u8; 32]);
    network.network.nodes[1].committed.insert(99, [8u8; 32]);
    assert_eq!(network.conflicting_commit(), Some(99));

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        network.assert_no_conflicting_finalisation("the deliberate conflict");
    }));
    let payload = outcome.expect_err("the assertion stops the run");
    let text = payload
        .downcast_ref::<String>()
        .cloned()
        .unwrap_or_default();
    assert!(text.contains("height 99"), "the failure names the height: {}", text);
    assert!(text.contains("Fix:"), "the failure names the fix: {}", text);
}

#[test]
fn the_network_never_finalises_a_block_the_set_refused() {
    let mut network = FaultNetwork::new(VALIDATORS, FaultProfile::skewing(&[0, 1, 2, 3, 4], 60_000));
    for slot_number in 1..=SLOTS {
        let report = network.run_slot(Slot(slot_number));
        if let SlotOutcome::Committed { .. } = report.outcome {
            for index in report.refused_by.iter() {
                assert!(
                    !network.network.nodes[*index].committed.contains_key(&network.network.height()),
                    "validator {} finalised a block it refused",
                    index
                );
            }
        }
        network.assert_no_conflicting_finalisation("a skewed set");
    }
}
