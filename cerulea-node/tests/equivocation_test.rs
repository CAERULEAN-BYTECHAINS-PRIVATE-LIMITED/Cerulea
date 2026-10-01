//! Equivocation detection across the multi-node harness.
//!
//! Equivocation means one validator sealing two different headers for one slot
//! at any rank. A slot is one fixed span of time. These tests run the real
//! Authoring Engine. One node seals two headers for one slot. Every other node
//! detects the pair, builds the evidence and checks it.
//!
//! The harness lives at `cerulea-node/src/test_harness.rs` behind the
//! `test-harness` feature. Nothing here stands in for an engine.

use cerulea_consensus::engines::authoring::equivocation::{
    accept_report, check_proof, head_key, sealed_head, EquivocationProof, EquivocationReport,
    EquivocationTracker, ReportQueue, MAX_EQUIVOCATION_REPORT_BYTES,
};
use cerulea_consensus::engines::authoring::gate::{AuthorGate, GateDecision};
use cerulea_consensus::engines::authoring::seal::SealClaim;
use cerulea_consensus::engines::{Rank, Slot};
use cerulea_node::test_harness::{TestHeader, TestNetwork};
use codec::Encode;
use sp_runtime::traits::Header as HeaderT;

/// The slot the tests drive.
const SLOT: Slot = Slot(3);

/// Build the ticket the scheduled author of one slot holds.
///
/// The gate answers with the ticket only where the local keystore holds the
/// scheduled author key, so the ticket names a real author.
fn author_ticket(network: &TestNetwork, slot: Slot) -> (usize, SealClaim, u64) {
    let parent = network.parent_hash();
    let context = network.context(slot, parent);
    let registry = network.registry();
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
    let tracker = cerulea_consensus::engines::authoring::fallback::FallbackTracker::new(schedule);
    let gate = AuthorGate::new(network.nodes[index].keystore.clone());
    let ticket = match gate
        .decide(&context, &registry, &tracker, window.opens_ms)
        .expect("the keystore answers the gate")
    {
        GateDecision::Author(ticket) => ticket,
        GateDecision::StandDown(reason) => {
            panic!("the scheduled author stood down: {:?}", reason)
        }
    };
    (index, ticket.seal_claim(), window.opens_ms)
}

/// Seal two different headers for one slot with one author key.
fn two_headers(network: &TestNetwork, slot: Slot) -> (usize, TestHeader, TestHeader) {
    let (index, claim, _moment) = author_ticket(network, slot);
    let parent = network.parent_hash();
    let first = network.sealed_header(index, claim.clone(), 1, parent);
    let second = network.sealed_header(index, claim, 2, parent);
    (index, first, second)
}

/// Read the sealed head out of one header the harness produced.
fn head_of(header: &TestHeader) -> cerulea_consensus::engines::authoring::SealedHead {
    let seal = TestNetwork::seal_of(header).expect("the header carries a seal");
    sealed_head(header, &seal).expect("the claim encodes to the stated length")
}

#[test]
fn a_node_detects_two_headers_sealed_for_one_slot() {
    let network = TestNetwork::with_validators(5);
    let (_author, first, second) = two_headers(&network, SLOT);
    assert_ne!(first.hash(), second.hash(), "the two headers differ");

    let mut tracker = EquivocationTracker::new();
    assert!(tracker.observe(head_of(&first)).is_none());
    let proof = tracker
        .observe(head_of(&second))
        .expect("the second header raises the proof");
    assert_eq!(tracker.detections(), 1);

    let facts = check_proof(&proof).expect("the evidence stands on its own");
    assert_eq!(facts.slot, SLOT.0);
    assert_eq!(facts.rank, Rank::FIRST.0);
}

#[test]
fn the_evidence_names_the_author_that_sealed_both_headers() {
    let network = TestNetwork::with_validators(5);
    let (author, first, second) = two_headers(&network, SLOT);

    let proof = EquivocationProof::new(head_of(&first), head_of(&second));
    let facts = check_proof(&proof).expect("the evidence stands");
    assert_eq!(facts.seal_key, network.nodes[author].seal_key.0);
    assert_eq!(facts.draw_key, network.nodes[author].draw_key.0);
}

#[test]
fn every_honest_node_reaches_the_same_evidence() {
    let network = TestNetwork::with_validators(7);
    let (_author, first, second) = two_headers(&network, SLOT);

    let mut raised = Vec::new();
    for _node in network.nodes.iter() {
        let mut tracker = EquivocationTracker::new();
        tracker.observe(head_of(&first));
        let proof = tracker
            .observe(head_of(&second))
            .expect("each node raises the proof");
        raised.push(proof);
    }
    assert_eq!(raised.len(), 7);
    for proof in raised.iter() {
        assert_eq!(proof, &raised[0], "every node builds one evidence set");
        check_proof(proof).expect("the evidence stands on every node");
    }
}

#[test]
fn a_node_that_saw_one_header_learns_of_the_other_through_a_report() {
    let network = TestNetwork::with_validators(5);
    let (_author, first, second) = two_headers(&network, SLOT);

    // The reporting node saw both headers and builds the evidence.
    let mut reporter = EquivocationTracker::new();
    reporter.observe(head_of(&first));
    let proof = reporter
        .observe(head_of(&second))
        .expect("the reporter raises the proof");
    let message = EquivocationReport::new(proof).encode();

    // The receiving node saw one header alone. The report carries the rest.
    let mut receiver = EquivocationTracker::new();
    receiver.observe(head_of(&first));
    assert_eq!(receiver.detections(), 0);

    let (report, facts) = accept_report(&message).expect("the report checks out");
    assert_eq!(facts.slot, SLOT.0);

    let mut queue = ReportQueue::new();
    assert!(queue.place(report.proof));
    assert_eq!(queue.len(), 1);
    assert_eq!(head_key(&report.proof.first).0, SLOT.0);
}

#[test]
fn an_oversized_report_is_refused_before_decoding() {
    let message = vec![0u8; MAX_EQUIVOCATION_REPORT_BYTES + 1];
    assert!(
        accept_report(&message).is_err(),
        "the node refuses the oversized report"
    );
}

#[test]
fn an_honest_slot_raises_no_evidence() {
    let mut network = TestNetwork::with_validators(5);
    let mut tracker = EquivocationTracker::new();
    for slot in 1..=6u64 {
        let outcome = network.run_slot(Slot(slot));
        assert!(outcome.committed(), "the honest slot commits");
        let header = network.nodes[0]
            .chain
            .last()
            .expect("the node holds the block")
            .clone();
        assert!(
            tracker.observe(head_of(&header)).is_none(),
            "an honest chain raises no evidence"
        );
    }
    assert_eq!(tracker.detections(), 0);
}

#[test]
fn a_report_of_an_honest_pair_of_blocks_carries_no_evidence() {
    let mut network = TestNetwork::with_validators(5);
    network.run_slot(Slot(1));
    let first = network.nodes[0].chain[0].clone();
    network.run_slot(Slot(2));
    let second = network.nodes[0].chain[1].clone();

    let proof = EquivocationProof::new(head_of(&first), head_of(&second));
    assert!(
        check_proof(&proof).is_err(),
        "two blocks at two slots prove nothing"
    );
}
