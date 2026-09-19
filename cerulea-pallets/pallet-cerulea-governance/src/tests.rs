//! The tests of the governance module.
//!
//! The electorate holds four validators, each carrying a weight of one.
//! A structural change needs 80 percent of that weight. A constitutional
//! change needs 90 percent and a delay.

use crate::{
    mock::*, ApprovalGate, Error, ProposalKind, ProposalStatus, Proposals, QuorumSetting,
    RollWeight,
};
use cerulea_blueprint::{
    actions::TransactionClass,
    governance::{ChangeProcess, Governance as GovernanceSection, QuorumRule, UpgradeAuthority},
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::DeploymentId;

extern crate alloc;
use alloc::{vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const VALIDATORS: [u64; 4] = [11, 12, 13, 14];
const DAY: u64 = 86_400_000;

fn bounded(bytes: Vec<u8>) -> BoundedVec<u8, ConstU32<4_096>> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn payload(text: &str) -> BoundedVec<u8, ConstU32<4_096>> {
    bounded(text.as_bytes().to_vec())
}

fn seat_the_electorate() {
    for voter in VALIDATORS {
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            voter,
            1
        ));
    }
}

fn section() -> GovernanceSection {
    GovernanceSection {
        authority: UpgradeAuthority::new(&["steward", "auditor"], 80, 2),
        quorum: vec![
            QuorumRule {
                class: TransactionClass::Structural,
                threshold_percent: 80,
                minimum_voters: 3,
                delay_millis: 0,
            },
            QuorumRule {
                class: TransactionClass::Constitutional,
                threshold_percent: 90,
                minimum_voters: 4,
                delay_millis: DAY,
            },
        ],
        process: ChangeProcess::default(),
    }
}

fn open(kind: ProposalKind, body: &str) -> u64 {
    let id = crate::NextProposal::<Test>::get(DEPLOYMENT);
    assert_ok!(Governance::propose(
        RuntimeOrigin::signed(VALIDATORS[0]),
        DEPLOYMENT,
        kind,
        payload(body),
        clock() + DAY
    ));
    id
}

fn vote_all(id: u64, approve: bool) {
    for voter in VALIDATORS {
        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(voter),
            DEPLOYMENT,
            id,
            approve
        ));
    }
}

#[test]
fn a_class_with_no_declared_rule_carries_its_floor() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            Governance::quorum_for(&DEPLOYMENT, WorkClass::Structural),
            QuorumSetting {
                threshold_percent: 80,
                minimum_voters: 1,
                delay_millis: 0
            }
        );
        assert_eq!(
            Governance::quorum_for(&DEPLOYMENT, WorkClass::Constitutional).threshold_percent,
            90
        );
        assert_eq!(
            Governance::quorum_for(&DEPLOYMENT, WorkClass::Routine).threshold_percent,
            51
        );
        assert_eq!(
            Governance::quorum_for(&DEPLOYMENT, WorkClass::Financial).threshold_percent,
            67
        );
    });
}

#[test]
fn the_governance_section_installs_and_reads_back() {
    new_test_ext().execute_with(|| {
        assert_ok!(Governance::install_governance(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(section().encode())
        ));
        let held = Governance::quorum_for(&DEPLOYMENT, WorkClass::Constitutional);
        assert_eq!(held.threshold_percent, 90);
        assert_eq!(held.minimum_voters, 4);
        assert_eq!(held.delay_millis, DAY);
        assert_eq!(crate::Authority::<Test>::get(DEPLOYMENT).len(), 2);
    });
}

#[test]
fn a_threshold_below_the_floor_of_its_class_declines() {
    new_test_ext().execute_with(|| {
        let mut low = section();
        low.quorum[0].threshold_percent = 60;
        assert_noop!(
            Governance::install_governance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(low.encode())
            ),
            Error::<Test>::ThresholdBelowFloor
        );
    });
}

#[test]
fn every_proposal_kind_carries_its_class() {
    assert_eq!(ProposalKind::SchemaChange.class(), WorkClass::Structural);
    assert_eq!(
        ProposalKind::PermissionChange.class(),
        WorkClass::Structural
    );
    assert_eq!(
        ProposalKind::ValidatorSetChange.class(),
        WorkClass::Structural
    );
    assert_eq!(
        ProposalKind::PolicyAmendment.class(),
        WorkClass::Constitutional
    );
    assert_eq!(
        ProposalKind::UpgradeApproval.class(),
        WorkClass::Constitutional
    );
    assert_eq!(ProposalKind::SchemaChange.label(), "schema change");
}

#[test]
fn a_proposal_opens_over_a_seated_electorate() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_eq!(RollWeight::<Test>::get(DEPLOYMENT), 4);
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.status, ProposalStatus::Open);
        assert_eq!(held.class, WorkClass::Structural);
        assert_eq!(held.roll_weight, 4);
        assert_eq!(held.proposer, VALIDATORS[0]);
    });
}

#[test]
fn a_proposal_over_an_empty_electorate_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Governance::propose(
                RuntimeOrigin::signed(VALIDATORS[0]),
                DEPLOYMENT,
                ProposalKind::SchemaChange,
                payload("add the survey field"),
                clock() + DAY
            ),
            Error::<Test>::RollEmpty
        );
    });
}

#[test]
fn a_deadline_in_the_past_declines() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_noop!(
            Governance::propose(
                RuntimeOrigin::signed(VALIDATORS[0]),
                DEPLOYMENT,
                ProposalKind::SchemaChange,
                payload("add the survey field"),
                clock()
            ),
            Error::<Test>::DeadlineTooEarly
        );
    });
}

#[test]
fn a_structural_change_lands_at_eighty_percent() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_ok!(Governance::install_governance(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(section().encode())
        ));
        let id = open(ProposalKind::SchemaChange, "add the survey field");

        // Three of four voters agree. That share is 75 percent, below the
        // structural threshold.
        for voter in VALIDATORS.iter().take(3) {
            assert_ok!(Governance::vote(
                RuntimeOrigin::signed(*voter),
                DEPLOYMENT,
                id,
                true
            ));
        }
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Open
        );

        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(VALIDATORS[3]),
            DEPLOYMENT,
            id,
            true
        ));
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Approved
        );
    });
}

#[test]
fn a_constitutional_change_waits_its_delay() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_ok!(Governance::install_governance(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(section().encode())
        ));
        let id = open(ProposalKind::PolicyAmendment, "raise the retention span");
        vote_all(id, true);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.status, ProposalStatus::Approved);
        assert_eq!(held.effective_at, Some(clock() + DAY));

        assert_noop!(
            Governance::execute(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::DelayPending
        );

        advance_clock(DAY);
        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Executed
        );
    });
}

#[test]
fn a_proposal_the_electorate_declines_is_rejected() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "remove the holder field");
        vote_all(id, false);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Rejected
        );
    });
}

#[test]
fn a_proposal_past_its_deadline_is_rejected() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id,
            true
        ));
        advance_clock(DAY + 1);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Rejected
        );
    });
}

#[test]
fn a_vote_after_the_deadline_declines() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        advance_clock(DAY + 1);
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, id, true),
            Error::<Test>::DeadlinePassed
        );
    });
}

#[test]
fn a_second_vote_from_one_account_declines() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id,
            true
        ));
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, id, false),
            Error::<Test>::VoteAlreadyCast
        );
    });
}

#[test]
fn an_account_outside_the_electorate_casts_no_vote() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id, true),
            Error::<Test>::VoterUnknown
        );
    });
}

#[test]
fn a_vote_carries_the_weight_of_its_account() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            VALIDATORS[0],
            7
        ));
        assert_eq!(RollWeight::<Test>::get(DEPLOYMENT), 10);
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::vote(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id,
            true
        ));
        let vote = Governance::vote_of(&DEPLOYMENT, id, &VALIDATORS[0]).expect("the vote lands");
        assert_eq!(vote.weight, 7);
        assert!(vote.approve);
    });
}

#[test]
fn a_weight_of_zero_takes_an_account_out_of_the_electorate() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            VALIDATORS[0],
            0
        ));
        assert_eq!(RollWeight::<Test>::get(DEPLOYMENT), 3);
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, id, true),
            Error::<Test>::VoterUnknown
        );
    });
}

#[test]
fn the_approval_gate_answers_for_an_executed_change() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        vote_all(id, true);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(Governance::is_approved(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"add the survey field"
        ));
        assert!(!Governance::is_executed(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"add the survey field"
        ));

        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert!(Governance::is_executed(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"add the survey field"
        ));
        assert!(!Governance::is_executed(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"remove the holder field"
        ));
        assert_eq!(
            Governance::threshold_for(&DEPLOYMENT, WorkClass::Structural),
            80
        );
    });
}

#[test]
fn an_undecided_proposal_executes_no_change() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_noop!(
            Governance::execute(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::ProposalUndecided
        );
    });
}

#[test]
fn the_proposer_withdraws_an_open_proposal() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_noop!(
            Governance::cancel(RuntimeOrigin::signed(VALIDATORS[1]), DEPLOYMENT, id),
            Error::<Test>::NotTheProposer
        );
        assert_ok!(Governance::cancel(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id
        ));
        assert_eq!(
            Governance::proposal(&DEPLOYMENT, id)
                .expect("the register holds the proposal")
                .status,
            ProposalStatus::Cancelled
        );
        assert!(!Governance::is_approved(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"add the survey field"
        ));
    });
}

#[test]
fn a_cancelled_proposal_takes_no_further_vote() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::cancel(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id
        ));
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(VALIDATORS[1]), DEPLOYMENT, id, true),
            Error::<Test>::ProposalClosed
        );
    });
}

#[test]
fn an_unknown_proposal_declines_every_call() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, 7, true),
            Error::<Test>::ProposalUnknown
        );
        assert_noop!(
            Governance::tally(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 7),
            Error::<Test>::ProposalUnknown
        );
    });
}

#[test]
fn every_governance_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        vote_all(id, true);
        assert_ok!(Governance::tally(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));

        let opened = Audit::query_by_event(&DEPLOYMENT, "proposal_opened", 0, 64);
        assert_eq!(opened.len(), 1);
        assert_eq!(opened[0].actor, VALIDATORS[0]);
        assert_eq!(opened[0].module, "governance");
        assert_eq!(opened[0].class, WorkClass::Structural);

        let votes = Audit::query_by_event(&DEPLOYMENT, "vote_cast", 0, 64);
        assert_eq!(votes.len(), 4);

        let approved = Audit::query_by_event(&DEPLOYMENT, "proposal_approved", 0, 64);
        assert_eq!(approved.len(), 1);
        assert_eq!(approved[0].actor, STEWARD);

        let executed = Audit::query_by_event(&DEPLOYMENT, "proposal_executed", 0, 64);
        assert_eq!(executed.len(), 1);
        assert_eq!(executed[0].actor, STEWARD);
    });
}

#[test]
fn a_validator_set_change_carries_the_structural_class() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::ValidatorSetChange, "seat one more validator");
        let held = Proposals::<Test>::get(DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.class, WorkClass::Structural);
        assert_eq!(held.kind, ProposalKind::ValidatorSetChange);
    });
}

#[test]
fn an_upgrade_approval_carries_the_constitutional_class() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::UpgradeApproval, "platform version two");
        let held = Proposals::<Test>::get(DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.class, WorkClass::Constitutional);
    });
}
