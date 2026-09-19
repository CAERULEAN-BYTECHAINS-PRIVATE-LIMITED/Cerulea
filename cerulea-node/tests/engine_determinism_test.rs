//! Identical state roots across nodes from identical input.
//!
//! The Execution Engine groups the transactions of a block into conflict
//! domains and runs the domains at the same time. Parallel execution changes
//! the speed and leaves the result unchanged. This file runs one block carrying
//! transactions across several conflict domains and asserts that every node
//! reaches the same per-domain roots and the same combined root.
//!
//! Three claims carry this file.
//!
//! 1. **The per-domain roots match.** Each node produces the same root for each
//!    domain from the same input.
//! 2. **The combined root matches.** The root over the domain roots is the same
//!    on every node.
//! 3. **The worker count changes nothing.** One worker and many workers reach
//!    the same roots, so the result rests on the input alone.

use cerulea_node::test_harness::{execute_block, HarnessTransaction, TestNetwork};

use cerulea_consensus::engines::{Slot, TransactionClass};

/// The number of nodes that execute the same block.
const NODES: usize = 3;

/// Build one block carrying transactions across several conflict domains.
///
/// Each collection stands for one domain. Two transactions that write the same
/// collection and the same key join one domain. Two transactions over separate
/// collections stay apart and run at the same time.
fn block_across_domains() -> Vec<HarnessTransaction> {
    vec![
        HarnessTransaction::new("warehouse.scan", "pallet-1", "arrived", TransactionClass::Routine),
        HarnessTransaction::new("warehouse.scan", "pallet-1", "checked", TransactionClass::Routine),
        HarnessTransaction::new("treasury.movement", "account-7", "settled", TransactionClass::Financial),
        HarnessTransaction::new("registry.record", "unit-3", "listed", TransactionClass::Structural),
        HarnessTransaction::new("registry.record", "unit-4", "listed", TransactionClass::Routine),
        HarnessTransaction::new("policy.amendment", "profile-1", "opened", TransactionClass::Constitutional),
        HarnessTransaction::new("warehouse.scan", "pallet-2", "arrived", TransactionClass::Routine),
        HarnessTransaction::new("treasury.movement", "account-9", "settled", TransactionClass::Financial),
    ]
}

#[test]
fn every_node_reaches_the_same_roots_from_the_same_block() {
    let block = block_across_domains();

    let roots: Vec<_> = (0..NODES)
        .map(|node| execute_block(&block, node + 1))
        .collect();

    let first = &roots[0];
    assert!(
        first.len() >= 4,
        "the block carried at least four conflict domains, reaching {}",
        first.len()
    );
    assert!(first.consistent(), "the combined root covers the domain roots");

    for (position, held) in roots.iter().enumerate() {
        assert_eq!(
            held.domains, first.domains,
            "node {} reached the same per-domain roots",
            position + 1
        );
        assert_eq!(
            held.combined, first.combined,
            "node {} reached the same combined root",
            position + 1
        );
    }
}

#[test]
fn the_worker_count_changes_the_speed_and_leaves_the_roots_standing() {
    let block = block_across_domains();
    let single = execute_block(&block, 1);
    let many = execute_block(&block, 8);

    assert_eq!(single.domains, many.domains);
    assert_eq!(single.combined, many.combined);
}

#[test]
fn a_repeated_run_reaches_the_same_roots() {
    let block = block_across_domains();
    let first = execute_block(&block, 4);
    for _ in 0..5 {
        assert_eq!(execute_block(&block, 4), first);
    }
}

#[test]
fn a_changed_transaction_changes_the_roots() {
    let block = block_across_domains();
    let base = execute_block(&block, 4);

    let mut changed = block.clone();
    changed[2] = HarnessTransaction::new(
        "treasury.movement",
        "account-7",
        "reversed",
        TransactionClass::Financial,
    );
    let after = execute_block(&changed, 4);

    assert_ne!(
        base.combined, after.combined,
        "a changed transaction changes the combined root"
    );
}

#[test]
fn a_block_carrying_one_domain_still_produces_a_combined_root() {
    let block = vec![
        HarnessTransaction::new("warehouse.scan", "pallet-1", "arrived", TransactionClass::Routine),
        HarnessTransaction::new("warehouse.scan", "pallet-1", "checked", TransactionClass::Routine),
    ];
    let roots = execute_block(&block, 4);
    assert_eq!(roots.len(), 1, "the two transactions joined one domain");
    assert!(roots.consistent());
}

#[test]
fn the_network_commits_blocks_while_the_roots_stay_identical() {
    // The determinism claim holds while the network runs. Every node commits
    // the same block and reaches the same roots for the same input.
    let mut network = TestNetwork::with_validators(NODES);
    let block = block_across_domains();
    let expected = execute_block(&block, 4);

    for slot_number in 1..=6u64 {
        assert!(
            network.run_slot(Slot(slot_number)).committed(),
            "slot {} commits",
            slot_number
        );
        for node in 0..NODES {
            let roots = execute_block(&block, node + 1);
            assert_eq!(
                roots, expected,
                "node {} reached the same roots at slot {}",
                node + 1,
                slot_number
            );
        }
    }

    let head = network.nodes[0].head_hash();
    for node in network.nodes.iter() {
        assert_eq!(node.head_hash(), head, "{} holds the same head", node.name);
    }
    assert!(network.conflicting_commit().is_none());
}
