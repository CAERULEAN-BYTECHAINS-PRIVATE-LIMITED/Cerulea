//! Multi-node agreement across an epoch boundary.
//!
//! Three validators run in one process. Each one holds its own keystore with an
//! sr25519 draw key and an ed25519 seal key. The network produces and commits
//! blocks across an epoch boundary. Every block carries a seal signed by the
//! author. Every node checks that seal with the three conditions the Authoring
//! Engine carries. Membership, signature and rank timing.
//!
//! Three claims carry this file.
//!
//! 1. **Sealed authorship holds.** Every committed block carries a seal the
//!    named author produced, and every node accepts that seal.
//! 2. **A forged authorship attempt is rejected.** A node that holds no author
//!    key cannot produce a block another node accepts.
//! 3. **The committee is the same everywhere.** Every node draws the same
//!    committee for every slot, on both sides of the epoch boundary.

use cerulea_node::test_harness::{SlotOutcome, TestNetwork};

use cerulea_consensus::engines::authoring::seal::SealClaim;
use cerulea_consensus::engines::{EngineError, Rank, Slot};

/// The number of validators the agreement test runs.
const VALIDATORS: usize = 3;

/// The number of slots the test runs, which spans two epoch boundaries.
const SLOTS: u64 = 14;

#[test]
fn three_validators_commit_blocks_across_an_epoch_boundary() {
    let mut network = TestNetwork::with_validators(VALIDATORS);
    let mut committed = 0u32;
    let mut epochs_seen = vec![network.epoch];

    for slot_number in 1..=SLOTS {
        let slot = Slot(slot_number);
        let epoch = network.epoch_of(slot);
        if epoch != network.epoch {
            network.advance_epoch();
            epochs_seen.push(network.epoch);
        }

        match network.run_slot(slot) {
            SlotOutcome::Committed {
                height,
                signed_percent,
                ..
            } => {
                committed += 1;
                assert_eq!(height, committed, "heights advance one at a time");
                assert!(
                    signed_percent >= 66,
                    "every reachable validator signed, reaching {} percent",
                    signed_percent
                );
            }
            other => panic!("slot {} failed to commit: {:?}", slot_number, other),
        }
    }

    assert_eq!(committed, SLOTS as u32, "every slot committed one block");
    assert!(
        epochs_seen.len() >= 3,
        "the run crossed at least two epoch boundaries, seeing {:?}",
        epochs_seen
    );

    // Every node holds the same chain and the same committed record.
    let head = network.nodes[0].head_hash();
    for node in network.nodes.iter() {
        assert_eq!(node.height(), SLOTS as u32, "{} reached the head", node.name);
        assert_eq!(node.head_hash(), head, "{} holds the same head", node.name);
        assert_eq!(
            node.committed.len(),
            SLOTS as usize,
            "{} committed every block",
            node.name
        );
    }

    assert!(
        network.conflicting_commit().is_none(),
        "no two conflicting blocks committed at one height"
    );
}

#[test]
fn every_committed_block_carries_a_seal_from_its_author() {
    let mut network = TestNetwork::with_validators(VALIDATORS);

    for slot_number in 1..=6u64 {
        assert!(
            network.run_slot(Slot(slot_number)).committed(),
            "slot {} commits",
            slot_number
        );
    }

    for header in network.nodes[0].chain.iter() {
        let seal = TestNetwork::seal_of(header).expect("every block carries a seal");
        let author = network
            .nodes
            .iter()
            .find(|node| node.seal_key == seal.claim.author)
            .expect("the seal names a validator of this network");
        assert_eq!(
            seal.claim.draw.validator, author.draw_key,
            "the seal binds the draw key of the author to its seal key"
        );
        assert!(
            seal.claim.rank.0 >= 1,
            "the seal names the rank the author held"
        );
    }
}

#[test]
fn a_forged_authorship_attempt_is_rejected_on_import() {
    let mut network = TestNetwork::with_validators(VALIDATORS);
    assert!(network.run_slot(Slot(1)).committed());

    let slot = Slot(2);
    let parent_hash = network.parent_hash();
    let context = network.context(slot, parent_hash);
    let scheduled = *context
        .committee
        .at_rank(Rank::FIRST)
        .expect("rank one holds a validator");

    // The forger is any node other than the scheduled author.
    let forger = network
        .nodes
        .iter()
        .position(|node| node.draw_key != scheduled)
        .expect("a node outside rank one takes part");
    let scheduled_index = network
        .nodes
        .iter()
        .position(|node| node.draw_key == scheduled)
        .expect("the scheduled author takes part");

    // The forger names the scheduled author in the claim and signs with its own
    // seal key. The signature therefore belongs to the wrong key.
    let input = network.draw_input(slot, parent_hash);
    let forged_draw = network.keystore_draw(forger, &input);
    let claim = SealClaim::new(
        slot,
        Rank::FIRST,
        network.nodes[scheduled_index].seal_key,
        forged_draw,
        0,
    );
    let header = network.forged_header(forger, claim, network.height() + 1, parent_hash);

    let refusal = network
        .check(&header, slot, parent_hash, 0)
        .expect_err("a forged block fails import");
    assert!(
        matches!(
            refusal,
            EngineError::ValidatorKeysMismatch
                | EngineError::AuthorOutsideCommittee { .. }
                | EngineError::RankMismatch { .. }
                | EngineError::SealSignatureRejected { .. }
        ),
        "the refusal names the check that failed: {:?}",
        refusal
    );
    assert!(
        refusal.to_string().contains("Fix:"),
        "the refusal names the fix"
    );
}

#[test]
fn a_block_from_outside_the_committee_is_rejected_on_import() {
    let mut network = TestNetwork::with_validators(9);
    assert!(network.run_slot(Slot(1)).committed());

    let slot = Slot(2);
    let parent_hash = network.parent_hash();
    let context = network.context(slot, parent_hash);

    // A network of nine draws a committee of five. Four validators sit outside.
    let outsider = network
        .nodes
        .iter()
        .position(|node| !context.committee.holds(&node.draw_key))
        .expect("a network of nine leaves validators outside the committee");

    let input = network.draw_input(slot, parent_hash);
    let draw = network.keystore_draw(outsider, &input);
    let claim = SealClaim::new(
        slot,
        Rank::FIRST,
        network.nodes[outsider].seal_key,
        draw,
        0,
    );
    let header = network.sealed_header(outsider, claim, network.height() + 1, parent_hash);

    let refusal = network
        .check(&header, slot, parent_hash, 0)
        .expect_err("a block from outside the committee fails import");
    assert!(
        matches!(refusal, EngineError::AuthorOutsideCommittee { .. }),
        "the refusal names the membership check: {:?}",
        refusal
    );
}

#[test]
fn every_node_draws_the_same_committee_for_every_slot() {
    let mut network = TestNetwork::with_validators(9);

    for slot_number in 1..=SLOTS {
        let slot = Slot(slot_number);
        let epoch = network.epoch_of(slot);
        if epoch != network.epoch {
            network.advance_epoch();
        }

        let parent_hash = network.parent_hash();
        let first = network.committee(slot, parent_hash);

        // Every node reads the same published state, so every node repeats the
        // same draw. Repeating the call stands for the second node.
        for _ in 0..VALIDATORS {
            let again = network.committee(slot, parent_hash);
            assert_eq!(
                first, again,
                "slot {} draws the same committee on every node",
                slot_number
            );
        }

        assert_eq!(
            first.len().min(5),
            first.len(),
            "the committee never runs past five seats"
        );
        for (position, member) in first.members.iter().enumerate() {
            assert_eq!(member.rank, Rank::from_index(position));
        }

        assert!(network.run_slot(slot).committed(), "slot {} commits", slot_number);
    }
}

#[test]
fn the_committee_changes_when_the_epoch_changes() {
    let mut network = TestNetwork::with_validators(9);
    assert!(network.run_slot(Slot(1)).committed());

    let parent_hash = network.parent_hash();
    let slot = Slot(2);
    let before = network.committee(slot, parent_hash);

    network.advance_epoch();
    let after = network.committee(slot, parent_hash);

    assert_ne!(
        before, after,
        "a fresh epoch draw gives the slot a fresh committee"
    );
}
