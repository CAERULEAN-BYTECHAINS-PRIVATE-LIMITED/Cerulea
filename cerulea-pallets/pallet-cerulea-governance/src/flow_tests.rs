//! The tests of every branch of the change process.
//!
//! The electorate holds four validators, each carrying a weight of one.
//! Each test drives the calls and reads the stored proposal, the events and
//! the audit log afterwards.

use crate::{
    class_of, mock::*, payload_hash, ApprovalGate, ApprovedChanges, Authority, Error, Event,
    ExecutedChanges, NextProposal, ProposalKind, ProposalStatus, Quorum, QuorumSetting,
    RegisteredElectorate, VoterRoll,
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
use alloc::{string::String, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const VALIDATORS: [u64; 4] = [11, 12, 13, 14];
const DAY: u64 = 86_400_000;
const SPAN: u64 = 604_800_000;

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

fn section(rules: Vec<QuorumRule>, roles: &[&str]) -> GovernanceSection {
    GovernanceSection {
        authority: UpgradeAuthority::new(roles, 80, 2),
        quorum: rules,
        process: ChangeProcess::default(),
    }
}

fn rule(class: TransactionClass, threshold: u8, voters: u32, delay: u64) -> QuorumRule {
    QuorumRule {
        class,
        threshold_percent: threshold,
        minimum_voters: voters,
        delay_millis: delay,
    }
}

fn install(held: GovernanceSection) {
    assert_ok!(Governance::install_governance(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(held.encode())
    ));
}

fn open(kind: ProposalKind, body: &str) -> u64 {
    let id = NextProposal::<Test>::get(DEPLOYMENT);
    assert_ok!(Governance::propose(
        RuntimeOrigin::signed(VALIDATORS[0]),
        DEPLOYMENT,
        kind,
        payload(body),
        clock() + DAY
    ));
    id
}

fn cast(voter: u64, id: u64, approve: bool) {
    assert_ok!(Governance::vote(
        RuntimeOrigin::signed(voter),
        DEPLOYMENT,
        id,
        approve
    ));
}

fn tally(id: u64) {
    assert_ok!(Governance::tally(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        id
    ));
}

fn status(id: u64) -> ProposalStatus {
    Governance::proposal(&DEPLOYMENT, id)
        .expect("the register holds the proposal")
        .status
}

/// Carry one proposal to approval with every validator agreeing.
fn approved(kind: ProposalKind, body: &str) -> u64 {
    let id = open(kind, body);
    for voter in VALIDATORS {
        cast(voter, id, true);
    }
    tally(id);
    assert_eq!(status(id), ProposalStatus::Approved);
    id
}

fn governance_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Governance(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// The fix sentence one error carries in the runtime metadata.
fn error_text(name: &str) -> String {
    use scale_info::{TypeDef, TypeInfo};
    let info = <Error<Test> as TypeInfo>::type_info();
    let variants = match info.type_def {
        TypeDef::Variant(found) => found.variants,
        _ => panic!("the error type is an enumeration"),
    };
    let variant = variants
        .into_iter()
        .find(|held| held.name == name)
        .expect("the error type names the variant");
    variant
        .docs
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn a_section_that_fails_to_decode_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Governance::install_governance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(Vec::new())
            ),
            Error::<Test>::SectionUnreadable
        );
        assert_eq!(
            error_text("SectionUnreadable"),
            "The section failed to decode. Submit the canonical encoding the Studio produces."
        );
    });
}

#[test]
fn a_role_name_past_the_name_limit_declines() {
    new_test_ext().execute_with(|| {
        let long = "r".repeat(200);
        assert_noop!(
            Governance::install_governance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(section(Vec::new(), &[long.as_str()]).encode())
            ),
            Error::<Test>::NameTooLong
        );
        assert_eq!(
            error_text("NameTooLong"),
            "The name runs past 128 bytes. Shorten the role name."
        );
    });
}

#[test]
fn an_authority_naming_more_roles_than_the_runtime_admits_declines() {
    new_test_ext().execute_with(|| {
        let names: Vec<String> = (0..17).map(|n| alloc::format!("role_{n}")).collect();
        let roles: Vec<&str> = names.iter().map(|name| name.as_str()).collect();
        assert_noop!(
            Governance::install_governance(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(section(Vec::new(), &roles).encode())
            ),
            Error::<Test>::AuthorityTooWide
        );
        assert_eq!(
            error_text("AuthorityTooWide"),
            "The section names more roles than the runtime admits. Name fewer roles, or raise the limit in the runtime."
        );
    });
}

#[test]
fn an_installed_rule_keeps_the_mandatory_delay_of_its_class() {
    new_test_ext().execute_with(|| {
        install(section(
            vec![
                rule(TransactionClass::Constitutional, 95, 2, 0),
                rule(TransactionClass::Routine, 60, 1, 5),
            ],
            &["steward"],
        ));
        // The section asked for no delay. The constitutional class carries a
        // delay of one day, so the stored rule keeps that day.
        assert_eq!(
            Quorum::<Test>::get(DEPLOYMENT, WorkClass::Constitutional),
            Some(QuorumSetting {
                threshold_percent: 95,
                minimum_voters: 2,
                delay_millis: DAY
            })
        );
        assert_eq!(
            Governance::quorum_for(&DEPLOYMENT, WorkClass::Routine),
            QuorumSetting {
                threshold_percent: 60,
                minimum_voters: 1,
                delay_millis: 5
            }
        );
        assert_eq!(
            Governance::threshold_for(&DEPLOYMENT, WorkClass::Constitutional),
            95
        );
        assert_eq!(Authority::<Test>::get(DEPLOYMENT).len(), 1);
        assert!(governance_events().contains(&Event::GovernanceInstalled {
            deployment: DEPLOYMENT,
            rules: 2
        }));

        let installed = Audit::query_by_event(&DEPLOYMENT, "governance_installed", 0, 64);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].actor, STEWARD);
        assert_eq!(installed[0].class, WorkClass::Constitutional);
        assert_eq!(
            installed[0].reason,
            "The steward installed the governance section of the Blueprint."
        );
    });
}

#[test]
fn the_floor_of_every_class_carries_its_threshold_and_delay() {
    assert_eq!(
        QuorumSetting::floor(WorkClass::Constitutional),
        QuorumSetting {
            threshold_percent: 90,
            minimum_voters: 1,
            delay_millis: DAY
        }
    );
    assert_eq!(QuorumSetting::floor(WorkClass::Financial).delay_millis, 0);
    assert_eq!(class_of(TransactionClass::Financial), WorkClass::Financial);
    assert_eq!(class_of(TransactionClass::Routine), WorkClass::Routine);
}

#[test]
fn every_proposal_kind_carries_a_label_for_the_interface() {
    assert_eq!(ProposalKind::PermissionChange.label(), "permission change");
    assert_eq!(ProposalKind::PolicyAmendment.label(), "policy amendment");
    assert_eq!(
        ProposalKind::ValidatorSetChange.label(),
        "validator set change"
    );
    assert_eq!(ProposalKind::UpgradeApproval.label(), "upgrade approval");
}

#[test]
fn one_payload_hashes_apart_under_two_kinds() {
    assert_ne!(
        payload_hash(ProposalKind::SchemaChange, b"add the survey field"),
        payload_hash(ProposalKind::PermissionChange, b"add the survey field")
    );
    assert_ne!(
        payload_hash(ProposalKind::SchemaChange, b"add the survey field"),
        payload_hash(ProposalKind::SchemaChange, b"add the ward field")
    );
}

#[test]
fn the_registered_electorate_reports_membership_and_weight() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert!(<RegisteredElectorate<Test> as VoterRoll>::is_member(
            &DEPLOYMENT,
            &VALIDATORS[2]
        ));
        assert!(!<RegisteredElectorate<Test> as VoterRoll>::is_member(
            &DEPLOYMENT,
            &STEWARD
        ));
        assert_eq!(
            <RegisteredElectorate<Test> as VoterRoll>::total_weight(&DEPLOYMENT),
            4
        );
        assert!(governance_events().contains(&Event::VoterWeighted {
            deployment: DEPLOYMENT,
            voter: VALIDATORS[3],
            weight: 1
        }));
        let weighted = Audit::query_by_event(&DEPLOYMENT, "voter_weighted", 0, 64);
        assert_eq!(weighted.len(), 4);
        assert_eq!(
            weighted[0].reason,
            "The steward set the weight one account carries in the electorate."
        );
    });
}

#[test]
fn a_deadline_past_the_largest_vote_span_declines() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        assert_noop!(
            Governance::propose(
                RuntimeOrigin::signed(VALIDATORS[0]),
                DEPLOYMENT,
                ProposalKind::SchemaChange,
                payload("add the survey field"),
                clock() + SPAN + 1
            ),
            Error::<Test>::DeadlineTooLate
        );
        // A deadline at the largest span itself opens.
        assert_ok!(Governance::propose(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            ProposalKind::SchemaChange,
            payload("add the survey field"),
            clock() + SPAN
        ));
        assert_eq!(
            error_text("DeadlineTooLate"),
            "The deadline sits past the largest vote span. Set a nearer deadline, or raise the span in the runtime."
        );
    });
}

#[test]
fn a_full_proposal_register_declines_a_further_proposal() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        NextProposal::<Test>::insert(DEPLOYMENT, u64::MAX);
        assert_noop!(
            Governance::propose(
                RuntimeOrigin::signed(VALIDATORS[0]),
                DEPLOYMENT,
                ProposalKind::SchemaChange,
                payload("add the survey field"),
                clock() + DAY
            ),
            Error::<Test>::RegisterFull
        );
    });
}

#[test]
fn an_opened_proposal_stores_its_hash_and_emits_its_event() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::PermissionChange, "widen the registrar role");
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(
            held.payload_hash,
            payload_hash(ProposalKind::PermissionChange, b"widen the registrar role")
        );
        assert_eq!(held.opened_at, START);
        assert_eq!(held.deadline, START + DAY);
        assert_eq!(NextProposal::<Test>::get(DEPLOYMENT), id + 1);
        assert!(governance_events().contains(&Event::ProposalOpened {
            deployment: DEPLOYMENT,
            proposal: id,
            kind: ProposalKind::PermissionChange,
            proposer: VALIDATORS[0],
        }));
        let opened = Audit::query_by_event(&DEPLOYMENT, "proposal_opened", 0, 64);
        assert_eq!(
            opened[0].reason,
            "A member of the electorate opened one proposal."
        );
    });
}

#[test]
fn a_vote_lands_with_its_event_and_its_record() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        cast(VALIDATORS[1], id, false);
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.reject_weight, 1);
        assert_eq!(held.approve_weight, 0);
        assert_eq!(held.voters, 1);
        let vote = Governance::vote_of(&DEPLOYMENT, id, &VALIDATORS[1]).expect("the vote lands");
        assert!(!vote.approve);
        assert_eq!(vote.at_millis, START);
        assert!(governance_events().contains(&Event::VoteCast {
            deployment: DEPLOYMENT,
            proposal: id,
            voter: VALIDATORS[1],
            approve: false,
            weight: 1,
        }));
        let votes = Audit::query_by_event(&DEPLOYMENT, "vote_cast", 0, 64);
        assert_eq!(votes[0].class, WorkClass::Routine);
        assert_eq!(
            votes[0].reason,
            "A member of the electorate cast one weighted vote."
        );
    });
}

#[test]
fn a_proposal_that_can_no_longer_reach_its_threshold_is_rejected_before_its_deadline() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "remove the holder field");
        // Two of four decline. The best reachable share is 50 percent,
        // below the structural threshold of 80 percent.
        cast(VALIDATORS[0], id, false);
        cast(VALIDATORS[1], id, false);
        tally(id);
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.status, ProposalStatus::Rejected);
        assert_eq!(held.decided_at, Some(START));
        assert!(clock() <= held.deadline);
        assert!(governance_events().contains(&Event::ProposalRejected {
            deployment: DEPLOYMENT,
            proposal: id
        }));
        let rejected = Audit::query_by_event(&DEPLOYMENT, "proposal_rejected", 0, 64);
        assert_eq!(rejected.len(), 1);
        assert_eq!(rejected[0].reason, "The electorate declined one proposal.");
    });
}

#[test]
fn a_tally_below_the_minimum_voter_count_leaves_the_proposal_open() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        install(section(
            vec![rule(TransactionClass::Structural, 80, 3, 0)],
            &["steward"],
        ));
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            VALIDATORS[0],
            100
        ));
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        // One voter carries 100 of 103 weight, above the threshold. The
        // rule asks for three voters, so the proposal stays open.
        cast(VALIDATORS[0], id, true);
        tally(id);
        assert_eq!(status(id), ProposalStatus::Open);
        assert!(!governance_events()
            .iter()
            .any(|event| matches!(event, Event::ProposalApproved { .. })));

        cast(VALIDATORS[1], id, true);
        cast(VALIDATORS[2], id, true);
        tally(id);
        assert_eq!(status(id), ProposalStatus::Approved);
    });
}

#[test]
fn a_tally_reads_an_electorate_that_grew_after_the_proposal_opened() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = open(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            21,
            4
        ));
        for voter in VALIDATORS {
            cast(voter, id, true);
        }
        tally(id);
        // Four of eight agree. The share of 50 percent sits below the
        // threshold, and the outstanding weight keeps the threshold within
        // reach, so the proposal stays open.
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.status, ProposalStatus::Open);
        assert_eq!(held.roll_weight, 8);

        cast(21, id, true);
        tally(id);
        assert_eq!(status(id), ProposalStatus::Approved);
    });
}

#[test]
fn an_approval_records_its_effect_time_and_its_event() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = approved(ProposalKind::UpgradeApproval, "platform version two");
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.decided_at, Some(START));
        assert_eq!(held.effective_at, Some(START + DAY));
        assert!(governance_events().contains(&Event::ProposalApproved {
            deployment: DEPLOYMENT,
            proposal: id,
            effective_at: START + DAY
        }));
        assert_eq!(
            ApprovedChanges::<Test>::get((
                DEPLOYMENT,
                ProposalKind::UpgradeApproval,
                held.payload_hash
            )),
            Some(id)
        );
        let approvals = Audit::query_by_event(&DEPLOYMENT, "proposal_approved", 0, 64);
        assert_eq!(approvals[0].class, WorkClass::Constitutional);
        assert_eq!(
            approvals[0].reason,
            "The electorate agreed to one proposal."
        );
    });
}

#[test]
fn a_decided_proposal_takes_no_further_vote_or_tally() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = approved(ProposalKind::SchemaChange, "add the survey field");
        assert_noop!(
            Governance::tally(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::ProposalClosed
        );
        assert_ok!(Governance::set_voter_weight(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            21,
            1
        ));
        assert_noop!(
            Governance::vote(RuntimeOrigin::signed(21), DEPLOYMENT, id, true),
            Error::<Test>::ProposalClosed
        );
        assert_eq!(
            error_text("ProposalClosed"),
            "The proposal takes no further vote. Open a new proposal."
        );
    });
}

#[test]
fn an_executed_change_executes_once_and_emits_its_event() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = approved(ProposalKind::SchemaChange, "add the survey field");
        System::set_block_number(5);
        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            id
        ));
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.executed_by, Some(STEWARD));
        assert_eq!(held.executed_block, Some(5));
        assert_eq!(
            ExecutedChanges::<Test>::get((
                DEPLOYMENT,
                ProposalKind::SchemaChange,
                held.payload_hash
            )),
            Some(id)
        );
        assert!(governance_events().contains(&Event::ProposalExecuted {
            deployment: DEPLOYMENT,
            proposal: id,
            executor: STEWARD
        }));
        let executed = Audit::query_by_event(&DEPLOYMENT, "proposal_executed", 0, 64);
        assert_eq!(
            executed[0].reason,
            "A module carried out one approved change."
        );

        // The executed change still answers as approved.
        assert!(Governance::is_approved(
            &DEPLOYMENT,
            ProposalKind::SchemaChange,
            b"add the survey field"
        ));
        assert_noop!(
            Governance::execute(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::ProposalUndecided
        );
        assert_eq!(
            error_text("ProposalUndecided"),
            "The proposal waits for its decision. Call the tally before executing the change."
        );
    });
}

#[test]
fn an_execution_of_an_unknown_proposal_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Governance::execute(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 3),
            Error::<Test>::ProposalUnknown
        );
        assert_noop!(
            Governance::cancel(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, 3),
            Error::<Test>::ProposalUnknown
        );
        assert_eq!(
            error_text("ProposalUnknown"),
            "The module holds no proposal under that identifier. Open a proposal before voting on it."
        );
    });
}

#[test]
fn the_proposer_withdraws_an_approved_proposal_and_the_approval_lapses() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let id = approved(ProposalKind::PolicyAmendment, "raise the retention span");
        assert!(Governance::is_approved(
            &DEPLOYMENT,
            ProposalKind::PolicyAmendment,
            b"raise the retention span"
        ));
        advance_clock(10);
        assert_ok!(Governance::cancel(
            RuntimeOrigin::signed(VALIDATORS[0]),
            DEPLOYMENT,
            id
        ));
        let held = Governance::proposal(&DEPLOYMENT, id).expect("the register holds the proposal");
        assert_eq!(held.status, ProposalStatus::Cancelled);
        assert_eq!(held.decided_at, Some(START + 10));
        assert!(!Governance::is_approved(
            &DEPLOYMENT,
            ProposalKind::PolicyAmendment,
            b"raise the retention span"
        ));
        assert!(governance_events().contains(&Event::ProposalCancelled {
            deployment: DEPLOYMENT,
            proposal: id
        }));
        let cancelled = Audit::query_by_event(&DEPLOYMENT, "proposal_cancelled", 0, 64);
        assert_eq!(cancelled[0].actor, VALIDATORS[0]);
        assert_eq!(cancelled[0].reason, "The proposer withdrew one proposal.");

        // A cancelled proposal executes no change.
        assert_noop!(
            Governance::execute(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, id),
            Error::<Test>::ProposalUndecided
        );
    });
}

#[test]
fn a_rejected_or_executed_proposal_is_withdrawn_no_further() {
    new_test_ext().execute_with(|| {
        seat_the_electorate();
        let rejected = open(ProposalKind::SchemaChange, "remove the holder field");
        for voter in VALIDATORS {
            cast(voter, rejected, false);
        }
        tally(rejected);
        assert_noop!(
            Governance::cancel(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, rejected),
            Error::<Test>::ProposalClosed
        );

        let executed = approved(ProposalKind::SchemaChange, "add the survey field");
        assert_ok!(Governance::execute(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            executed
        ));
        assert_noop!(
            Governance::cancel(RuntimeOrigin::signed(VALIDATORS[0]), DEPLOYMENT, executed),
            Error::<Test>::ProposalClosed
        );
        assert_eq!(
            error_text("NotTheProposer"),
            "The caller opened no proposal under that identifier. Ask the proposer to withdraw the proposal."
        );
    });
}

#[test]
fn every_remaining_error_names_its_fix() {
    assert_eq!(
        error_text("DelayPending"),
        "The mandatory delay has yet to pass. Wait for the delay of the class, then execute the change."
    );
    assert_eq!(
        error_text("ThresholdBelowFloor"),
        "The section declares a threshold below the floor of its class. Raise the threshold to the floor of the class or above it."
    );
    assert_eq!(
        error_text("RollEmpty"),
        "The electorate carries no weight. Give at least one account its weight before opening a vote."
    );
    assert_eq!(
        error_text("VoterUnknown"),
        "The account carries no weight in the electorate. Ask the steward to give the account its weight."
    );
}
