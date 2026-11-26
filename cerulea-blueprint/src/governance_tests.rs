//! Behavioural tests for the governance section.
//!
//! Each test starts from a clean section, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    actions::TransactionClass,
    governance::{
        threshold_location, ChangeProcess, ChangeStage, Governance, QuorumRule, UpgradeAuthority,
    },
    identities::{Identities, RoleDef},
    DiagnosticCode, Path, Report,
};
use alloc::{vec, vec::Vec};

fn identities() -> Identities {
    Identities {
        roles: vec![
            RoleDef::new("council", None, "Council", "The council approves."),
            RoleDef::new("auditor", None, "Auditor", "The auditor reviews."),
        ],
        ..Identities::default()
    }
}

fn clean() -> Governance {
    Governance {
        authority: UpgradeAuthority::new(&["council"], 67, 3),
        quorum: vec![
            QuorumRule::standard(TransactionClass::Routine),
            QuorumRule::standard(TransactionClass::Constitutional),
        ],
        process: ChangeProcess {
            stages: vec![ChangeStage::new(
                "review",
                &["auditor"],
                86_400_000,
                "The auditor reviews the change.",
            )],
        },
    }
}

fn report(governance: &Governance) -> Report {
    governance.validate(&identities(), &Path::root().field("governance"))
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_section_passes_and_answers_threshold_lookups() {
    let governance = clean();
    assert!(report(&governance).is_clean());
    assert!(governance.is_declared());
    assert_eq!(
        governance
            .rule_for(TransactionClass::Constitutional)
            .map(|rule| rule.delay_millis),
        Some(86_400_000)
    );
    assert_eq!(
        governance
            .rule_for(TransactionClass::Routine)
            .map(|rule| rule.delay_millis),
        Some(0)
    );
    assert!(governance.rule_for(TransactionClass::Financial).is_none());
    assert_eq!(governance.threshold_for(TransactionClass::Routine), 51);
    assert_eq!(governance.threshold_for(TransactionClass::Financial), 67);
    assert_eq!(
        threshold_location(&Path::root().field("governance"), 2).as_str(),
        "governance.quorum[2].threshold_percent"
    );
    let silent = Governance::default();
    assert!(!silent.is_declared());
    assert!(report(&silent).is_clean());
}

#[test]
fn authority_faults_each_raise_a_diagnostic() {
    let mut governance = clean();
    governance.authority = UpgradeAuthority::new(&[], 120, 0);
    let found = report(&governance);
    assert_eq!(
        at(&found, "governance.authority.roles").fix,
        "Name at least one role that approves a change."
    );
    let over = at(&found, "governance.authority.threshold_percent");
    assert_eq!(over.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        over.problem,
        "The threshold for the upgrade authority sits at 120 percent."
    );
    assert_eq!(over.fix, "Set a threshold at 100 percent or below.");
    assert_eq!(
        at(&found, "governance.authority.minimum_voters").fix,
        "Require at least one approval."
    );

    let mut weak = clean();
    weak.authority = UpgradeAuthority::new(&["Council", "treasurer"], 40, 1);
    let found = report(&weak);
    assert_eq!(
        at(&found, "governance.authority.roles[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "governance.authority.roles[1]").fix,
        "Point the entry at one of these roles: council, auditor."
    );
    let low = at(&found, "governance.authority.threshold_percent");
    assert_eq!(low.code, DiagnosticCode::ThresholdTooLow);
    assert_eq!(
        low.problem,
        "The threshold for the upgrade authority sits at 40 percent, below the floor of 51 percent."
    );
    assert_eq!(low.fix, "Raise the threshold to 51 percent or above.");
}

#[test]
fn quorum_faults_each_raise_a_diagnostic() {
    let mut governance = clean();
    let mut repeated = QuorumRule::standard(TransactionClass::Routine);
    repeated.minimum_voters = 0;
    governance.quorum.push(repeated);
    let mut financial = QuorumRule::standard(TransactionClass::Financial);
    financial.threshold_percent = 60;
    governance.quorum.push(financial);
    let mut constitutional = QuorumRule::standard(TransactionClass::Structural);
    constitutional.class = TransactionClass::Constitutional;
    constitutional.threshold_percent = 90;
    constitutional.delay_millis = 0;
    governance.quorum = vec![
        governance.quorum[0],
        governance.quorum[2],
        governance.quorum[3],
        constitutional,
    ];
    let found = report(&governance);
    assert_eq!(
        at(&found, "governance.quorum[1].class").fix,
        "Merge the two rules into one."
    );
    assert_eq!(
        at(&found, "governance.quorum[1].minimum_voters").problem,
        "The rule for routine work counts zero voters."
    );
    let floor = at(&found, "governance.quorum[2].threshold_percent");
    assert_eq!(floor.code, DiagnosticCode::ThresholdTooLow);
    assert_eq!(floor.fix, "Raise the threshold to 67 percent or above.");
    assert_eq!(
        at(&found, "governance.quorum[3].delay_millis").fix,
        "Set a delay above zero so a constitutional change stays visible before it lands."
    );
}

#[test]
fn stage_faults_each_raise_a_diagnostic() {
    let mut governance = clean();
    governance
        .process
        .stages
        .push(ChangeStage::new("review", &[], 0, ""));
    governance.process.stages.push(ChangeStage::new(
        "Vote",
        &["Council", "treasurer"],
        60_000,
        "The council votes.",
    ));
    let found = report(&governance);
    let stages = "governance.process.stages";
    assert_eq!(
        at(&found, &alloc::format!("{stages}[1].id")).fix,
        "Rename one of the two stages away from `review`."
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[1].summary")).fix,
        "Write one plain sentence stating what happens in the stage."
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[1].duration_millis")).fix,
        "Set a span above zero."
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[1].roles")).fix,
        "Name at least one role that acts in the stage."
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[2].id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[2].roles[0]")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{stages}[2].roles[1]")).code,
        DiagnosticCode::DanglingReference
    );

    let mut bare = clean();
    bare.process.stages.clear();
    let found = report(&bare);
    let empty = at(&found, stages);
    assert_eq!(empty.code, DiagnosticCode::SectionEmpty);
    assert_eq!(
        empty.fix,
        "Declare at least one stage, such as a review stage."
    );
    let stages_only: Vec<&str> = found
        .diagnostics
        .iter()
        .map(|d| d.location.as_str())
        .collect();
    assert_eq!(stages_only, [stages]);
}

#[test]
fn multi_org_governance_validates_independent_organisations() {
    use crate::governance::{MultiOrgGovernance, OrgAuthority};

    // Valid 2-org governance passes cleanly.
    let mut gov = clean();
    gov.authority = UpgradeAuthority::multi_org(
        &["council", "auditor"],
        67,
        2,
        MultiOrgGovernance::new(
            vec![
                OrgAuthority::new("ministry_a", &["council"], 1),
                OrgAuthority::new("ministry_b", &["auditor"], 1),
            ],
            2,
        ),
    );
    assert!(gov.authority.is_multi_org());
    assert!(report(&gov).is_clean());

    // Fewer than 2 organisations and required_organisations < 2 both fail.
    let mut single_org = clean();
    single_org.authority = UpgradeAuthority::multi_org(
        &["council"],
        67,
        1,
        MultiOrgGovernance::new(vec![OrgAuthority::new("ministry_a", &["council"], 1)], 1),
    );
    let found_single = report(&single_org);
    assert_eq!(
        at(&found_single, "governance.authority.multi_org.organisations").code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found_single, "governance.authority.multi_org.required_organisations").code,
        DiagnosticCode::ValueOutOfRange
    );

    // Duplicate org id, empty org roles, zero approvals, and dangling role are caught.
    let mut broken = clean();
    broken.authority = UpgradeAuthority::multi_org(
        &["council"],
        67,
        1,
        MultiOrgGovernance::new(
            vec![
                OrgAuthority::new("ministry_a", &["missing_role"], 0),
                OrgAuthority::new("ministry_a", &[], 1),
            ],
            3,
        ),
    );
    let found_broken = report(&broken);
    assert_eq!(
        at(&found_broken, "governance.authority.multi_org.required_organisations").code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found_broken, "governance.authority.multi_org.organisations[0].roles[0]").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found_broken, "governance.authority.multi_org.organisations[0].minimum_approvals").code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found_broken, "governance.authority.multi_org.organisations[1].id").code,
        DiagnosticCode::DuplicateIdentifier
    );
    assert_eq!(
        at(&found_broken, "governance.authority.multi_org.organisations[1].roles").code,
        DiagnosticCode::SectionEmpty
    );
}
