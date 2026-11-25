//! Peer diversity across the multi-node harness.
//!
//! An eclipse attack isolates a node behind attacker-controlled peers. Every
//! block and every vote the node sees then comes from one party. These tests
//! run the real Authoring Engine. A node placed behind peers of one address
//! range halts authoring and raises an alert. The rest of the network carries
//! on.
//!
//! The harness lives at `cerulea-node/src/test_harness.rs` behind the
//! `test-harness` feature. Nothing here stands in for an engine.

use cerulea_consensus::engines::authoring::gate::{AuthorGate, GateDecision, StandDown};
use cerulea_consensus::engines::peers::{
    DiversityPolicy, EclipseReason, PeerAddress, PeerRecord, PeerSet, DEFAULT_MIN_PREFIXES,
};
use cerulea_consensus::engines::{Rank, Slot};
use cerulea_node::test_harness::{healthy_peers, TestNetwork};

/// The rank one holder of a slot, with the moment the window opens.
fn rank_one_holder(network: &TestNetwork, slot: Slot) -> (usize, u64) {
    let parent = network.parent_hash();
    let context = network.context(slot, parent);
    let schedule = cerulea_consensus::engines::authoring::fallback::SlotSchedule::for_slot(
        slot,
        network.policy,
    );
    let window = schedule
        .window(Rank::FIRST)
        .expect("rank one holds a window");
    let drawn = *context
        .committee
        .at_rank(Rank::FIRST)
        .expect("the committee draws rank one");
    let index = network
        .nodes
        .iter()
        .position(|node| node.draw_key == drawn)
        .expect("the drawn key belongs to a node");
    (index, window.opens_ms)
}

/// Ask the gate what one node decides for one slot.
fn decision(network: &TestNetwork, index: usize, slot: Slot, moment_ms: u64) -> GateDecision {
    let parent = network.parent_hash();
    let context = network.context(slot, parent);
    let registry = network.registry();
    let schedule = cerulea_consensus::engines::authoring::fallback::SlotSchedule::for_slot(
        slot,
        network.policy,
    );
    let tracker = cerulea_consensus::engines::authoring::fallback::FallbackTracker::new(schedule);
    let gate = AuthorGate::new(network.nodes[index].keystore.clone());
    gate.decide_with_peers(
        &context,
        &registry,
        &tracker,
        moment_ms,
        &network.nodes[index].peers,
    )
    .expect("the keystore answers the gate")
}

#[test]
fn a_healthy_node_authors_its_slot() {
    let network = TestNetwork::with_validators(5);
    let (author, moment) = rank_one_holder(&network, Slot(1));
    assert!(network.nodes[author].authoring_allowed());
    assert!(decision(&network, author, Slot(1), moment).opens());
}

/// An eclipsed node halts authoring and raises an alert.
#[test]
fn an_eclipsed_node_halts_authoring_and_raises_an_alert() {
    let mut network = TestNetwork::with_validators(5);
    let (author, moment) = rank_one_holder(&network, Slot(1));
    network.nodes[author].eclipse();

    assert!(!network.nodes[author].authoring_allowed());
    assert_eq!(
        decision(&network, author, Slot(1), moment),
        GateDecision::StandDown(StandDown::Eclipsed)
    );

    let assessment = network.nodes[author].peers.assess();
    assert_eq!(
        assessment.eclipse,
        Some(EclipseReason::TooFewPrefixes {
            held: 1,
            needed: DEFAULT_MIN_PREFIXES
        })
    );
    let alert = assessment.alert().expect("the node raises an alert");
    assert!(alert.contains("Block production stopped"));
    assert!(alert.contains("Reading stays open"));
    assert!(alert.contains("Fix:"));
}

#[test]
fn the_network_carries_on_while_one_node_sits_eclipsed() {
    let mut network = TestNetwork::with_validators(5);
    let (author, _moment) = rank_one_holder(&network, Slot(1));
    network.nodes[author].eclipse();

    // The slot passes to the next rank, exactly as it does for an absent node.
    let outcome = network.run_slot(Slot(1));
    assert!(outcome.committed(), "the network keeps committing");
    match outcome {
        cerulea_node::test_harness::SlotOutcome::Committed { rank, .. } => {
            assert!(rank >= 1, "a standing rank authored the slot");
        }
        other => panic!("the slot committed: {:?}", other),
    }
}

#[test]
fn a_node_resumes_authoring_once_the_peer_set_recovers() {
    let mut network = TestNetwork::with_validators(5);
    let (author, moment) = rank_one_holder(&network, Slot(1));
    network.nodes[author].eclipse();
    assert!(!decision(&network, author, Slot(1), moment).opens());

    network.nodes[author].restore_peers(author as u8 + 1);
    assert!(network.nodes[author].authoring_allowed());
    assert!(decision(&network, author, Slot(1), moment).opens());
}

#[test]
fn every_node_starts_from_a_peer_set_that_meets_the_requirement() {
    let network = TestNetwork::with_validators(7);
    for node in network.nodes.iter() {
        let assessment = node.peers.assess();
        assert!(assessment.authoring_allowed(), "{} authors", node.name);
        assert!(assessment.prefixes >= DEFAULT_MIN_PREFIXES);
        assert!(assessment.alert().is_none());
    }
}

/// A private chain admits the validators the registry names.
#[test]
fn a_private_chain_refuses_a_peer_outside_the_allow_list() {
    let network = TestNetwork::with_validators(4);
    let mut set = PeerSet::new(DiversityPolicy::private_chain());
    set.set_allow_list(network.nodes.iter().map(|node| node.seal_key.0));

    // Every validator of the network peers.
    for (index, node) in network.nodes.iter().enumerate() {
        set.admit(PeerRecord::new(
            &node.name,
            PeerAddress::V4([10, 0, index as u8, 1]),
            Some(node.seal_key.0),
        ))
        .expect("the registry names this validator");
    }
    assert!(set.authoring_allowed());
    assert_eq!(set.assess().validator_peers, 4);

    // A stranger presents a key the registry never recorded.
    let error = set
        .admit(PeerRecord::new(
            "stranger",
            PeerAddress::V4([198, 51, 100, 9]),
            Some([0xAA; 32]),
        ))
        .expect_err("the allow-list leaves this key out");
    assert!(error.to_string().contains("Fix:"));
    assert_eq!(set.len(), 4);
}

#[test]
fn a_healthy_peer_set_spans_several_address_ranges() {
    let set = healthy_peers(3);
    assert!(set.distinct_prefixes() >= DEFAULT_MIN_PREFIXES);
    assert!(set.authoring_allowed());
}
