//! Checkpoint anchoring across the multi-node harness.
//!
//! A long-range attack rewrites history from a point far below the head. These
//! tests run the real engines. The network produces a chain, the nodes anchor a
//! checkpoint over it, and a second chain offered from below that checkpoint is
//! refused.
//!
//! The harness lives at `cerulea-node/src/test_harness.rs` behind the
//! `test-harness` feature. Nothing here stands in for an engine.

use cerulea_consensus::engines::checkpoint::{Checkpoint, CheckpointStore, SyncGate};
use cerulea_consensus::engines::Slot;
use cerulea_node::test_harness::TestNetwork;
use sp_runtime::traits::Header as HeaderT;

/// Run a network for the stated number of slots and read back its chain.
fn run(slots: u64) -> TestNetwork {
    let mut network = TestNetwork::with_validators(5);
    for slot in 1..=slots {
        assert!(
            network.run_slot(Slot(slot)).committed(),
            "the honest slot commits"
        );
    }
    network
}

/// The checkpoint one node anchors over the block at the stated height.
fn checkpoint_over(network: &TestNetwork, index: u32, height: u32) -> Checkpoint {
    let header = &network.nodes[0].chain[(height - 1) as usize];
    Checkpoint::new(
        index,
        height,
        header.hash().into(),
        (*header.state_root()).into(),
    )
}

#[test]
fn every_node_anchors_the_same_checkpoint_over_one_chain() {
    let network = run(4);
    let anchored = checkpoint_over(&network, 0, 3);

    let mut stores = Vec::new();
    for _node in network.nodes.iter() {
        let mut store = CheckpointStore::new();
        assert_eq!(store.anchor(anchored), Ok(true));
        stores.push(store);
    }
    for store in stores.iter() {
        assert_eq!(store.anchors(), stores[0].anchors());
        assert_eq!(store.highest_block(), 3);
    }
    assert_eq!(
        anchored.commitment(),
        checkpoint_over(&network, 0, 3).commitment(),
        "every node folds the checkpoint into one commitment"
    );
}

/// The node refuses to reorganise below the latest anchored checkpoint.
#[test]
fn a_node_refuses_a_chain_that_rewrites_settled_history() {
    let network = run(6);
    let mut store = CheckpointStore::new();
    store
        .anchor(checkpoint_over(&network, 0, 4))
        .expect("the first checkpoint anchors");

    // A chain that shares only the first block would replace the blocks the
    // checkpoint settles.
    let error = store
        .accepts_reorganisation(1)
        .expect_err("the chain rewrites settled history");
    assert!(error.to_string().contains("Fix:"));

    // A chain that shares every block up to the checkpoint passes.
    assert_eq!(store.accepts_reorganisation(4), Ok(()));
    assert_eq!(store.accepts_reorganisation(6), Ok(()));
}

#[test]
fn a_node_refuses_a_second_block_at_an_anchored_height() {
    let network = run(4);
    let mut store = CheckpointStore::new();
    store
        .anchor(checkpoint_over(&network, 0, 3))
        .expect("the checkpoint anchors");

    let honest: [u8; 32] = network.nodes[0].chain[2].hash().into();
    assert_eq!(store.accepts_block(3, &honest), Ok(()));
    assert!(
        store.accepts_block(3, &[9u8; 32]).is_err(),
        "another block at the anchored height is refused"
    );
}

/// A fresh node syncing from genesis accepts only a chain through the anchored
/// checkpoints it was given.
#[test]
fn a_fresh_node_accepts_the_chain_through_its_anchors() {
    let network = run(6);
    let anchors = vec![
        checkpoint_over(&network, 0, 2),
        checkpoint_over(&network, 1, 4),
        checkpoint_over(&network, 2, 6),
    ];
    let mut gate = SyncGate::with_anchors(anchors);

    for (position, header) in network.nodes[0].chain.iter().enumerate() {
        let height = position as u32 + 1;
        let hash: [u8; 32] = header.hash().into();
        gate.offer(height, &hash).expect("the honest chain passes");
    }
    assert_eq!(gate.met(), 3);
    assert!(gate.complete());
}

#[test]
fn a_fresh_node_refuses_a_chain_that_misses_an_anchor() {
    let network = run(6);
    let anchors = vec![
        checkpoint_over(&network, 0, 2),
        checkpoint_over(&network, 1, 4),
    ];
    let mut gate = SyncGate::with_anchors(anchors);

    let first: [u8; 32] = network.nodes[0].chain[1].hash().into();
    gate.offer(2, &first).expect("the first anchor passes");

    // The attacker offers another block at the second anchored height.
    let error = gate
        .offer(4, &[3u8; 32])
        .expect_err("the second anchor names another block");
    assert!(error.to_string().contains("Fix:"));
    assert!(!gate.complete());
}

#[test]
fn a_fresh_node_refuses_a_chain_that_stops_below_the_highest_anchor() {
    let network = run(6);
    let anchors = vec![
        checkpoint_over(&network, 0, 2),
        checkpoint_over(&network, 1, 6),
    ];
    let mut gate = SyncGate::with_anchors(anchors);
    let first: [u8; 32] = network.nodes[0].chain[1].hash().into();
    gate.offer(2, &first).expect("the first anchor passes");
    assert!(
        !gate.complete(),
        "the chain stopped below the highest anchor"
    );
}

#[test]
fn the_local_store_survives_a_node_restart() {
    let network = run(4);
    let directory = tempfile::tempdir().expect("the test makes a directory");

    let mut store = CheckpointStore::open(directory.path()).expect("an absent file starts empty");
    assert!(store.is_empty());
    store
        .anchor_and_save(checkpoint_over(&network, 0, 2))
        .expect("the checkpoint anchors");
    store
        .anchor_and_save(checkpoint_over(&network, 1, 4))
        .expect("the checkpoint anchors");

    // The node restarts and reads its own store.
    let read = CheckpointStore::open(directory.path()).expect("the file reads back");
    assert_eq!(read.anchors(), store.anchors());
    assert_eq!(read.highest_block(), 4);
    assert!(read.accepts_reorganisation(1).is_err());
}
