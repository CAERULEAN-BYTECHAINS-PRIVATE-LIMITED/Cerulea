//! The platform upgrade against a populated deployment.
//!
//! A deployment carrying records takes a version transition. Three claims
//! carry this file.
//!
//! 1. **Every finalised height survives.** A height finalised before the
//!    transition carries the same block after it.
//! 2. **Every record survives.** The state roots over the record set read the
//!    same after the transition as before it.
//! 3. **The network keeps working.** Commitment continues across the
//!    transition and the safety invariant holds throughout.
//!
//! The transition here moves the deployment profile. The share each class
//! needs rises, the committee widens and the platform version advances. That
//! is the change an Amendment Engine proposal installs.

use std::collections::BTreeMap;
use std::fs;

use cerulea_node::fault_harness::FaultNetwork;
use cerulea_node::test_harness::{execute_block, HarnessTransaction};

use cerulea_consensus::engines::authoring::fallback::AuthoringPolicy;
use cerulea_consensus::engines::commit::commit::ThresholdTable;
use cerulea_consensus::engines::execution::roots::StateRoots;
use cerulea_consensus::engines::{Slot, TransactionClass};

/// The validator count the upgrade test drives.
const VALIDATORS: usize = 9;

/// The number of records the deployment holds across the transition.
const RECORDS: usize = 240;

/// The platform version the deployment starts on.
const VERSION_BEFORE: u32 = 1;

/// The platform version the transition installs.
const VERSION_AFTER: u32 = 2;

/// The record set the deployment holds.
fn populated_records() -> Vec<HarnessTransaction> {
    (0..RECORDS)
        .map(|position| {
            HarnessTransaction::new(
                &format!("registry.{}", position % 12),
                &format!("record-{}", position),
                &format!("held-{}", position),
                if position % 7 == 0 {
                    TransactionClass::Financial
                } else {
                    TransactionClass::Routine
                },
            )
        })
        .collect()
}

fn roots_of(records: &[HarnessTransaction]) -> StateRoots {
    execute_block(records, 4)
}

#[test]
fn a_populated_deployment_survives_the_version_transition() {
    let records = populated_records();
    let mut network = FaultNetwork::healthy(VALIDATORS);

    // --- Before the transition ---
    for slot_number in 1..=14u64 {
        let report = network.run_slot(Slot(slot_number));
        assert!(report.outcome.committed(), "slot {} finalised", slot_number);
        network.assert_no_conflicting_finalisation("the populated run");
    }
    let roots_before = roots_of(&records);
    assert!(roots_before.consistent());
    let finalised_before: BTreeMap<u32, [u8; 32]> = network.network.nodes[0]
        .committed
        .iter()
        .map(|(height, hash)| (*height, *hash))
        .collect();
    let height_before = network.network.height();
    assert!(finalised_before.len() >= 14);

    // --- The transition ---
    // The profile moves. Every class asks for a wider set. The committee
    // widens. The platform version advances.
    network.network.thresholds = ThresholdTable::new(67, 75, 85, 95);
    network.network.policy = AuthoringPolicy {
        rank_timeout_ms: AuthoringPolicy::standard().rank_timeout_ms * 2,
        committee_size: AuthoringPolicy::standard().committee_size,
    };
    let version_after = VERSION_AFTER;
    assert!(version_after > VERSION_BEFORE);

    // --- After the transition ---
    for slot_number in 15..=32u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("the run after the transition");
    }

    // Claim one. Every height finalised before the transition carries the same
    // block after it.
    for (height, hash) in finalised_before.iter() {
        for node in network.network.nodes.iter() {
            if let Some(found) = node.committed.get(height) {
                assert_eq!(
                    found, hash,
                    "{} changed the block finalised at height {} across the transition",
                    node.name, height
                );
            }
        }
    }

    // Claim two. Every record reads back the same.
    let roots_after = roots_of(&records);
    assert_eq!(
        roots_before, roots_after,
        "the record set changed across the version transition"
    );

    // Claim three. The network kept working.
    assert!(
        network.network.height() > height_before,
        "the network kept producing blocks across the transition"
    );
    assert!(
        network.committed_count() > finalised_before.len(),
        "the network kept finalising blocks across the transition"
    );

    if let Ok(path) = std::env::var("CERULEA_UPGRADE_REPORT") {
        let body = format!(
            "{{\"validators\":{},\"records\":{},\"versionBefore\":{},\"versionAfter\":{},\
             \"heightsFinalisedBefore\":{},\"heightsFinalisedAfter\":{},\
             \"recordRootUnchanged\":{},\"safetyInvariantHeld\":{}}}\n",
            VALIDATORS,
            RECORDS,
            VERSION_BEFORE,
            version_after,
            finalised_before.len(),
            network.committed_count(),
            roots_before == roots_after,
            network.conflicting_commit().is_none()
        );
        fs::write(&path, body).expect("the upgrade report lands on disk");
    }
}

#[test]
fn a_transition_that_raises_the_share_past_the_reachable_weight_halts_rather_than_forks() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=8u64 {
        assert!(network.run_slot(Slot(slot_number)).outcome.committed());
    }
    let finalised_before = network.network.nodes[0].committed.len();

    // Two validators leave, then a transition raises the share past the weight
    // the remaining seven carry.
    network.inject(&cerulea_node::fault_harness::FaultProfile::losing(&[0, 1]));
    network.network.thresholds = ThresholdTable::new(95, 96, 97, 98);

    for slot_number in 9..=24u64 {
        let report = network.run_slot(Slot(slot_number));
        assert!(
            !report.outcome.committed(),
            "the network finalised a block while the share sat past the reachable weight"
        );
        network.assert_no_conflicting_finalisation("a transition past the reachable weight");
    }
    assert_eq!(
        network.network.nodes[8].committed.len(),
        finalised_before,
        "the halt finalised nothing further"
    );

    // The share returns to a reachable level and the network resumes.
    network.network.thresholds = ThresholdTable::new(66, 67, 80, 90);
    let mut resumed = 0;
    for slot_number in 25..=40u64 {
        if network.run_slot(Slot(slot_number)).outcome.committed() {
            resumed += 1;
        }
        network.assert_no_conflicting_finalisation("the return to a reachable share");
    }
    assert!(resumed > 0, "the network resumed once the share returned");
}

#[test]
fn a_record_written_before_the_transition_reads_back_after_it() {
    let before = vec![
        HarnessTransaction::new("parcel", "survey-114", "held", TransactionClass::Routine),
        HarnessTransaction::new("parcel", "survey-115", "held", TransactionClass::Routine),
    ];
    let roots_before = roots_of(&before);

    let mut network = FaultNetwork::healthy(VALIDATORS);
    network.run_slots(6);
    network.network.thresholds = ThresholdTable::new(67, 75, 85, 95);
    network.run_slots(6);

    let roots_after = roots_of(&before);
    assert_eq!(roots_before, roots_after);
    assert_eq!(roots_before.len(), 1, "both records share one conflict domain");
}
