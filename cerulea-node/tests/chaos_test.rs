//! The chaos programme.
//!
//! Four faults, each injected on its own and then together. Node loss. Network
//! delay. Storage exhaustion. Clock skew. Every case asserts the invariant that
//! matters most after every slot. No two conflicting blocks finalise at one
//! height.
//!
//! The run writes a report naming each case, the fault it carried, the blocks
//! it finalised and whether the invariant held. The file the report lands in is
//! named by CERULEA_CHAOS_REPORT.

use std::collections::BTreeSet;
use std::fs;

use cerulea_node::fault_harness::{FaultNetwork, FaultProfile};
use cerulea_node::test_harness::SlotOutcome;

use cerulea_consensus::engines::Slot;

/// The validator count every chaos case runs.
const VALIDATORS: usize = 9;

/// The number of slots a chaos case drives.
const SLOTS: u64 = 30;

/// One chaos case.
struct ChaosCase {
    name: &'static str,
    profile: FaultProfile,
    /// Whether the case expects the network to keep finalising.
    keeps_finalising: bool,
}

fn chaos_cases() -> Vec<ChaosCase> {
    vec![
        ChaosCase {
            name: "node loss, one of nine",
            profile: FaultProfile::losing(&[4]),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "node loss, three of nine",
            profile: FaultProfile::losing(&[0, 4, 8]),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "node loss, four of nine",
            profile: FaultProfile::losing(&[0, 2, 4, 6]),
            keeps_finalising: false,
        },
        ChaosCase {
            name: "network delay, two signatures late",
            profile: FaultProfile::delaying(&[1, 5], 4_000),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "network delay, every signature late",
            profile: FaultProfile::delaying(&[0, 1, 2, 3, 4, 5, 6, 7, 8], 4_000),
            keeps_finalising: false,
        },
        ChaosCase {
            name: "disk exhaustion, two validators",
            profile: FaultProfile::filling_storage(&[3, 7]),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "clock skew, one minute ahead on three validators",
            profile: FaultProfile::skewing(&[2, 5, 8], 60_000),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "clock skew, one minute behind on three validators",
            profile: FaultProfile::skewing(&[1, 4, 7], -60_000),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "loss and delay together",
            profile: FaultProfile::losing(&[0]).with(&FaultProfile::delaying(&[1, 2], 4_000)),
            keeps_finalising: true,
        },
        ChaosCase {
            name: "loss, delay, disk exhaustion and clock skew together",
            profile: FaultProfile::losing(&[0])
                .with(&FaultProfile::delaying(&[1], 4_000))
                .with(&FaultProfile::filling_storage(&[7]))
                .with(&FaultProfile::skewing(&[8], 60_000)),
            keeps_finalising: true,
        },
    ]
}

/// One line of the report.
struct CaseResult {
    name: &'static str,
    faults: String,
    finalised: usize,
    withheld: usize,
    no_author: usize,
    invariant_held: bool,
}

fn run_case(case: &ChaosCase) -> CaseResult {
    let mut network = FaultNetwork::new(VALIDATORS, case.profile.clone());
    let mut no_author = 0usize;
    for slot_number in 1..=SLOTS {
        let report = network.run_slot(Slot(slot_number));
        if report.outcome == SlotOutcome::NoAuthor {
            no_author += 1;
        }
        network.assert_no_conflicting_finalisation(case.name);
    }
    let finalised: BTreeSet<u32> = network
        .network
        .nodes
        .iter()
        .flat_map(|node| node.committed.keys().copied())
        .collect();
    CaseResult {
        name: case.name,
        faults: case.profile.describe(),
        finalised: finalised.len(),
        withheld: network.withheld_count(),
        no_author,
        invariant_held: network.conflicting_commit().is_none(),
    }
}

#[test]
fn every_chaos_case_holds_the_safety_invariant() {
    let mut lines: Vec<String> = Vec::new();
    for case in chaos_cases() {
        let result = run_case(&case);
        assert!(
            result.invariant_held,
            "the case {} finalised two conflicting blocks at one height",
            result.name
        );
        if case.keeps_finalising {
            assert!(
                result.finalised > 0,
                "the case {} finalised nothing while the share still stood. Faults: {}",
                result.name,
                result.faults
            );
        } else {
            assert_eq!(
                result.finalised, 0,
                "the case {} finalised {} heights while the set sat below the share. Faults: {}",
                result.name, result.finalised, result.faults
            );
        }
        lines.push(format!(
            "{{\"case\":\"{}\",\"faults\":\"{}\",\"heightsFinalised\":{},\"slotsWithheld\":{},\"slotsWithoutAuthor\":{},\"invariantHeld\":{}}}",
            result.name,
            result.faults.replace('"', "'"),
            result.finalised,
            result.withheld,
            result.no_author,
            result.invariant_held
        ));
        println!(
            "chaos case: {} | faults: {} | heights finalised: {} | withheld: {} | no author: {}",
            result.name, result.faults, result.finalised, result.withheld, result.no_author
        );
    }

    if let Ok(path) = std::env::var("CERULEA_CHAOS_REPORT") {
        let body = format!(
            "{{\"validators\":{},\"slotsPerCase\":{},\"cases\":[{}]}}\n",
            VALIDATORS,
            SLOTS,
            lines.join(",")
        );
        fs::write(&path, body).expect("the chaos report lands on disk");
    }
}

#[test]
fn a_fault_injected_partway_never_finalises_a_second_block_at_a_finalised_height() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=8u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("the healthy opening");
    }
    let held: Vec<(u32, [u8; 32])> = network.network.nodes[0]
        .committed
        .iter()
        .map(|(height, hash)| (*height, *hash))
        .collect();

    for extra in [
        FaultProfile::losing(&[0, 1]),
        FaultProfile::delaying(&[2, 3], 4_000),
        FaultProfile::filling_storage(&[4]),
        FaultProfile::skewing(&[5], 60_000),
    ] {
        network.inject(&extra);
        for slot_number in 9..=20u64 {
            network.run_slot(Slot(slot_number));
            network.assert_no_conflicting_finalisation("a fault injected partway");
        }
    }

    for (height, hash) in held.iter() {
        for node in network.network.nodes.iter() {
            if let Some(found) = node.committed.get(height) {
                assert_eq!(
                    found, hash,
                    "{} changed the block finalised at height {}",
                    node.name, height
                );
            }
        }
    }
}

#[test]
fn the_network_recovers_after_every_fault_clears() {
    let mut network = FaultNetwork::new(
        VALIDATORS,
        FaultProfile::losing(&[0, 1, 2, 3])
            .with(&FaultProfile::delaying(&[4], 4_000))
            .with(&FaultProfile::filling_storage(&[5]))
            .with(&FaultProfile::skewing(&[6], 60_000)),
    );
    for slot_number in 1..=12u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("every fault at once");
    }
    let before = network.committed_count();

    network.heal();
    for slot_number in 13..=30u64 {
        network.run_slot(Slot(slot_number));
        network.assert_no_conflicting_finalisation("the recovery");
    }
    assert!(
        network.committed_count() > before,
        "the network finalised again once every fault cleared"
    );
    assert!(
        network.agreed_head().is_some(),
        "every validator reached one head after the recovery"
    );
}
