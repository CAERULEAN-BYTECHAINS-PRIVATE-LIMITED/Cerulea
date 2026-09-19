//! The parametrised multi-node harness.
//!
//! Nine validators run in one process under a stated fault profile. The three
//! engine tests beside this file cover agreement, partition and determinism on
//! a healthy network. This file drives the same engines through every fault
//! the field presents and states what the network must do in each case.
//!
//! 1. **Agreement.** A healthy set of nine commits one block per slot and every
//!    node holds the same head.
//! 2. **Partition.** Three of nine away keeps commitment going. Four of nine
//!    away stops it. The return resumes it.
//! 3. **Safe halt.** A network below the share commits nothing and stays
//!    consistent.
//!
//! The consensus safety assertion runs after every slot of every case.

use cerulea_node::fault_harness::{FaultNetwork, FaultProfile};
use cerulea_node::test_harness::{SlotOutcome, HARNESS_ROUTINE_PERCENT};

use cerulea_consensus::engines::Slot;

/// The validator count the harness runs.
const VALIDATORS: usize = 9;

/// The number of slots a case drives.
const SLOTS: u64 = 18;

/// One case the harness runs.
struct Case {
    name: &'static str,
    profile: FaultProfile,
    /// The smallest number of committed slots the case accepts.
    least_committed: usize,
    /// The largest number of committed slots the case accepts.
    most_committed: usize,
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "nine healthy validators",
            profile: FaultProfile::healthy(),
            least_committed: SLOTS as usize,
            most_committed: SLOTS as usize,
        },
        Case {
            name: "three of nine lost",
            profile: FaultProfile::losing(&[0, 1, 2]),
            least_committed: 5,
            most_committed: SLOTS as usize,
        },
        Case {
            name: "four of nine lost",
            profile: FaultProfile::losing(&[0, 1, 2, 3]),
            least_committed: 0,
            most_committed: 0,
        },
        Case {
            name: "five of nine lost",
            profile: FaultProfile::losing(&[0, 1, 2, 3, 4]),
            least_committed: 0,
            most_committed: 0,
        },
        Case {
            name: "three signatures arriving after the window closes",
            profile: FaultProfile::delaying(&[6, 7, 8], 5_000),
            least_committed: 5,
            most_committed: SLOTS as usize,
        },
        Case {
            name: "four signatures arriving after the window closes",
            profile: FaultProfile::delaying(&[5, 6, 7, 8], 5_000),
            least_committed: 0,
            most_committed: 0,
        },
        Case {
            name: "two validators holding no storage",
            profile: FaultProfile::filling_storage(&[7, 8]),
            least_committed: 5,
            most_committed: SLOTS as usize,
        },
    ]
}

#[test]
fn every_case_holds_the_outcome_the_share_dictates() {
    for case in cases() {
        let mut network = FaultNetwork::new(VALIDATORS, case.profile.clone());
        for slot_number in 1..=SLOTS {
            network.run_slot(Slot(slot_number));
            network.assert_no_conflicting_finalisation(case.name);
        }
        let committed = network.committed_count();
        assert!(
            committed >= case.least_committed,
            "{} committed {} slots, below the {} the share allows. Faults: {}",
            case.name,
            committed,
            case.least_committed,
            case.profile.describe()
        );
        assert!(
            committed <= case.most_committed,
            "{} committed {} slots, past the {} the share allows. Faults: {}",
            case.name,
            committed,
            case.most_committed,
            case.profile.describe()
        );
    }
}

#[test]
fn a_healthy_set_of_nine_reaches_one_head_on_every_node() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=SLOTS {
        let report = network.run_slot(Slot(slot_number));
        assert!(
            report.outcome.committed(),
            "slot {} committed nothing: {:?}",
            slot_number,
            report.outcome
        );
        assert_eq!(
            report.signed_by.len(),
            VALIDATORS,
            "every validator signed slot {}",
            slot_number
        );
        network.assert_no_conflicting_finalisation("a healthy set of nine");
    }
    let head = network
        .agreed_head()
        .expect("every node holds the same head");
    for node in network.network.nodes.iter() {
        assert_eq!(
            node.head_hash(),
            head,
            "{} holds the agreed head",
            node.name
        );
        assert_eq!(
            node.committed.len(),
            SLOTS as usize,
            "{} finalised every block",
            node.name
        );
    }
}

#[test]
fn three_of_nine_away_keeps_commitment_and_four_stops_it() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    for slot_number in 1..=4u64 {
        assert!(network.run_slot(Slot(slot_number)).outcome.committed());
    }
    let finalised_before = network.network.nodes[VALIDATORS - 1].committed.len();

    network.inject(&FaultProfile::losing(&[0, 1, 2]));
    let mut committed_under_three = 0;
    for slot_number in 5..=14u64 {
        match network.run_slot(Slot(slot_number)).outcome {
            SlotOutcome::Committed { signed_percent, .. } => {
                committed_under_three += 1;
                assert!(
                    signed_percent >= HARNESS_ROUTINE_PERCENT,
                    "six of nine carry {} percent",
                    signed_percent
                );
            }
            SlotOutcome::NoAuthor => {}
            other => panic!(
                "the network stopped committing under three away: {:?}",
                other
            ),
        }
        network.assert_no_conflicting_finalisation("three of nine away");
    }
    assert!(committed_under_three > 0);

    network.inject(&FaultProfile::losing(&[3]));
    let finalised_at_halt = network.network.nodes[VALIDATORS - 1].committed.len();
    for slot_number in 15..=26u64 {
        let report = network.run_slot(Slot(slot_number));
        assert!(
            !report.outcome.committed(),
            "the network committed with four of nine away: {:?}",
            report.outcome
        );
        network.assert_no_conflicting_finalisation("four of nine away");
    }
    assert_eq!(
        network.network.nodes[VALIDATORS - 1].committed.len(),
        finalised_at_halt,
        "the safe halt finalised nothing further"
    );
    assert!(finalised_at_halt > finalised_before);

    network.heal();
    let mut resumed = 0;
    for slot_number in 27..=38u64 {
        if network.run_slot(Slot(slot_number)).outcome.committed() {
            resumed += 1;
        }
        network.assert_no_conflicting_finalisation("the return");
    }
    assert!(
        resumed > 0,
        "commitment resumed once the validators returned"
    );
}

#[test]
fn a_delayed_signature_leaves_the_tally_and_the_block_still_reaches_the_share() {
    let mut network = FaultNetwork::new(VALIDATORS, FaultProfile::delaying(&[8], 5_000));
    for slot_number in 1..=SLOTS {
        let report = network.run_slot(Slot(slot_number));
        assert!(
            report.outcome.committed(),
            "eight of nine reach the share: {:?}",
            report.outcome
        );
        assert!(
            report.signed_by.len() == VALIDATORS - 1,
            "the delayed signature stayed out of the tally, {} signed",
            report.signed_by.len()
        );
        network.assert_no_conflicting_finalisation("one delayed signature");
    }
}

#[test]
fn a_validator_holding_no_storage_falls_behind_and_catches_up() {
    let mut network = FaultNetwork::new(VALIDATORS, FaultProfile::filling_storage(&[8]));
    for slot_number in 1..=8u64 {
        assert!(network.run_slot(Slot(slot_number)).outcome.committed());
        network.assert_no_conflicting_finalisation("storage exhaustion");
    }
    assert_eq!(
        network.network.nodes[8].height(),
        0,
        "the validator holding no storage wrote no block"
    );

    network.heal();
    assert!(
        network.network.nodes[8].height() > 0,
        "the validator caught up once its storage cleared"
    );
    let head = network
        .agreed_head()
        .expect("every node holds the same head");
    assert_eq!(network.network.nodes[8].head_hash(), head);
}

#[test]
fn a_clock_running_far_ahead_makes_the_validator_refuse_the_block() {
    let mut network = FaultNetwork::new(VALIDATORS, FaultProfile::skewing(&[8], 600_000));
    let mut refusals = 0;
    for slot_number in 1..=SLOTS {
        let report = network.run_slot(Slot(slot_number));
        refusals += report.refused_by.len();
        network.assert_no_conflicting_finalisation("a clock running far ahead");
    }
    assert!(
        refusals > 0,
        "the skewed validator refused at least one block. \
         Fix: check that the Authoring Engine reads the rank window against the moment the node holds."
    );
}

#[test]
fn the_harness_reports_what_each_slot_did() {
    let mut network = FaultNetwork::healthy(VALIDATORS);
    network.run_slots(6);
    assert_eq!(network.reports.len(), 6);
    for report in network.reports.iter() {
        assert!(report.outcome.committed());
        assert_eq!(report.refused_by.len(), 0);
        assert_eq!(report.accepted_by.len(), VALIDATORS);
    }
    assert_eq!(network.withheld_count(), 0);
    assert_eq!(network.committed_count(), 6);
}
