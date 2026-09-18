//! Upgrade authority, quorum rules and the change process.
//!
//! The governance section states who changes the deployment, the agreement
//! each class of change requires and the stages a change passes through.
//!
//! The thresholds hold a floor per class. Routine work needs 51 percent.
//! Financial work needs 67 percent. Structural work needs 80 percent.
//! Constitutional work needs 90 percent and a delay.

use crate::{
    actions::TransactionClass,
    identities::Identities,
    types::{RoleId, StageId},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The parties that change the deployment.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct UpgradeAuthority {
    /// The roles that approve a change.
    pub roles: Vec<RoleId>,
    /// The share of the authority that agrees before a change lands.
    pub threshold_percent: u8,
    /// The count of approvals a change collects.
    pub minimum_voters: u32,
}

impl UpgradeAuthority {
    /// Declare an authority.
    pub fn new(roles: &[&str], threshold_percent: u8, minimum_voters: u32) -> Self {
        UpgradeAuthority {
            roles: roles.iter().map(|role| RoleId::new(role)).collect(),
            threshold_percent,
            minimum_voters,
        }
    }
}

/// The agreement one class of change requires.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct QuorumRule {
    /// The class of change the rule covers.
    pub class: TransactionClass,
    /// The share of the validator set that agrees.
    pub threshold_percent: u8,
    /// The count of validators that vote.
    pub minimum_voters: u32,
    /// The span in milliseconds between approval and effect.
    pub delay_millis: u64,
}

impl QuorumRule {
    /// Declare the rule a class carries by default.
    pub fn standard(class: TransactionClass) -> Self {
        QuorumRule {
            class,
            threshold_percent: class.threshold_percent(),
            minimum_voters: 1,
            delay_millis: if class.holds_delay() { 86_400_000 } else { 0 },
        }
    }
}

/// One stage of a change process.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct ChangeStage {
    /// The name of the stage.
    pub id: StageId,
    /// The roles that act in the stage.
    pub roles: Vec<RoleId>,
    /// The longest span the stage runs for, in milliseconds.
    pub duration_millis: u64,
    /// One plain sentence stating what happens in the stage.
    pub summary: String,
}

impl ChangeStage {
    /// Declare a stage.
    pub fn new(id: &str, roles: &[&str], duration_millis: u64, summary: &str) -> Self {
        ChangeStage {
            id: StageId::new(id),
            roles: roles.iter().map(|role| RoleId::new(role)).collect(),
            duration_millis,
            summary: summary.to_string(),
        }
    }
}

/// The stages a change passes through.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct ChangeProcess {
    /// The stages, in order.
    pub stages: Vec<ChangeStage>,
}

/// The governance section.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Governance {
    /// The parties that change the deployment.
    pub authority: UpgradeAuthority,
    /// The agreement each class of change requires.
    pub quorum: Vec<QuorumRule>,
    /// The stages a change passes through.
    pub process: ChangeProcess,
}

impl Governance {
    /// The rule covering one class, when the section declares one.
    pub fn rule_for(&self, class: TransactionClass) -> Option<&QuorumRule> {
        self.quorum.iter().find(|rule| rule.class == class)
    }

    /// The share of the validator set that agrees for one class.
    ///
    /// A class the section leaves out carries the floor of the class.
    pub fn threshold_for(&self, class: TransactionClass) -> u8 {
        self.rule_for(class)
            .map(|rule| rule.threshold_percent)
            .unwrap_or_else(|| class.threshold_percent())
    }

    /// Report whether the deployment declares its own governance.
    ///
    /// A section left empty means the standard thresholds apply. Routine
    /// work needs 51 percent. Financial work needs 67 percent. Structural
    /// work needs 80 percent. Constitutional work needs 90 percent.
    pub fn is_declared(&self) -> bool {
        !self.authority.roles.is_empty()
            || !self.quorum.is_empty()
            || !self.process.stages.is_empty()
    }

    /// Check the whole governance section.
    ///
    /// A section left empty passes. The standard thresholds apply to that
    /// deployment.
    pub fn validate(&self, identities: &Identities, path: &Path) -> Report {
        let mut report = Report::new();
        if !self.is_declared() {
            return report;
        }

        let authority_path = path.field("authority");
        if self.authority.roles.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                authority_path.field("roles").location(),
                "The upgrade authority names zero roles.".to_string(),
                "Name at least one role that approves a change.".to_string(),
            ));
        }
        for (position, role) in self.authority.roles.iter().enumerate() {
            let here = authority_path.field("roles").index(position);
            report.absorb(role.validate(&here));
            if !identities.has_role(role) {
                report.push(identities.missing_role(role, here.location()));
            }
        }
        report.merge(share(
            self.authority.threshold_percent,
            51,
            "the upgrade authority",
            &authority_path.field("threshold_percent"),
        ));
        if self.authority.minimum_voters == 0 {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                authority_path.field("minimum_voters").location(),
                "The upgrade authority collects zero approvals.".to_string(),
                "Require at least one approval.".to_string(),
            ));
        }

        let quorum_path = path.field("quorum");
        let mut seen: BTreeSet<&'static str> = BTreeSet::new();
        for (position, rule) in self.quorum.iter().enumerate() {
            let here = quorum_path.index(position);
            if !seen.insert(rule.class.label()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("class").location(),
                    format!(
                        "The section carries two rules for {} work.",
                        rule.class.label()
                    ),
                    "Merge the two rules into one.".to_string(),
                ));
            }
            report.merge(share(
                rule.threshold_percent,
                rule.class.threshold_percent(),
                rule.class.label(),
                &here.field("threshold_percent"),
            ));
            if rule.minimum_voters == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("minimum_voters").location(),
                    format!(
                        "The rule for {} work counts zero voters.",
                        rule.class.label()
                    ),
                    "Require at least one voter.".to_string(),
                ));
            }
            if rule.class.holds_delay() && rule.delay_millis == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("delay_millis").location(),
                    "Constitutional work carries a delay of zero milliseconds.".to_string(),
                    "Set a delay above zero so a constitutional change stays visible before it lands."
                        .to_string(),
                ));
            }
        }

        let process_path = path.field("process").field("stages");
        let mut stage_names: BTreeSet<&str> = BTreeSet::new();
        for (position, stage) in self.process.stages.iter().enumerate() {
            let here = process_path.index(position);
            report.absorb(stage.id.validate(&here.field("id")));
            if !stage_names.insert(stage.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two stages share the name `{}`.", stage.id),
                    format!("Rename one of the two stages away from `{}`.", stage.id),
                ));
            }
            if stage.summary.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("summary").location(),
                    format!("The stage `{}` states nothing.", stage.id),
                    "Write one plain sentence stating what happens in the stage.".to_string(),
                ));
            }
            if stage.duration_millis == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("duration_millis").location(),
                    format!("The stage `{}` runs for zero milliseconds.", stage.id),
                    "Set a span above zero.".to_string(),
                ));
            }
            if stage.roles.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    here.field("roles").location(),
                    format!("The stage `{}` names zero roles.", stage.id),
                    "Name at least one role that acts in the stage.".to_string(),
                ));
            }
            for (index, role) in stage.roles.iter().enumerate() {
                let role_path = here.field("roles").index(index);
                report.absorb(role.validate(&role_path));
                if !identities.has_role(role) {
                    report.push(identities.missing_role(role, role_path.location()));
                }
            }
        }
        if self.process.stages.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                path.field("process").field("stages").location(),
                "The change process holds zero stages.".to_string(),
                "Declare at least one stage, such as a review stage.".to_string(),
            ));
        }

        report
    }
}

fn share(held: u8, floor: u8, subject: &str, path: &Path) -> Report {
    let mut report = Report::new();
    if held > 100 {
        report.push(Diagnostic::at(
            DiagnosticCode::ValueOutOfRange,
            path.location(),
            format!("The threshold for {subject} sits at {held} percent."),
            "Set a threshold at 100 percent or below.".to_string(),
        ));
    } else if held < floor {
        report.push(Diagnostic::at(
            DiagnosticCode::ThresholdTooLow,
            path.location(),
            format!(
                "The threshold for {subject} sits at {held} percent, below the floor of {floor} percent."
            ),
            format!("Raise the threshold to {floor} percent or above."),
        ));
    }
    report
}

/// The location of the threshold for one class, for an interface layer that
/// links a diagnostic to a control.
pub fn threshold_location(path: &Path, position: usize) -> Location {
    path.field("quorum")
        .index(position)
        .field("threshold_percent")
        .location()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identities::RoleDef;
    use alloc::vec;

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![
                RoleDef::new("steward", None, "Steward", "The steward approves a change."),
                RoleDef::new("auditor", None, "Auditor", "The auditor reviews a change."),
            ],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn sample() -> Governance {
        Governance {
            authority: UpgradeAuthority::new(&["steward", "auditor"], 67, 2),
            quorum: vec![
                QuorumRule::standard(TransactionClass::Routine),
                QuorumRule::standard(TransactionClass::Financial),
                QuorumRule::standard(TransactionClass::Structural),
                QuorumRule::standard(TransactionClass::Constitutional),
            ],
            process: ChangeProcess {
                stages: vec![
                    ChangeStage::new(
                        "review",
                        &["auditor"],
                        604_800_000,
                        "The auditor reads the change and records a finding.",
                    ),
                    ChangeStage::new(
                        "approval",
                        &["steward"],
                        259_200_000,
                        "The steward approves the change.",
                    ),
                ],
            },
        }
    }

    fn run(governance: &Governance) -> Report {
        governance.validate(&identities(), &Path::root().field("governance"))
    }

    #[test]
    fn an_empty_section_carries_the_standard_thresholds() {
        let governance = Governance::default();
        assert!(!governance.is_declared());
        assert!(run(&governance).is_clean());
        assert_eq!(governance.threshold_for(TransactionClass::Financial), 67);
    }

    #[test]
    fn a_complete_governance_section_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_threshold_below_the_class_floor_is_caught() {
        let mut governance = sample();
        governance.quorum[3].threshold_percent = 60;
        let report = run(&governance);
        let found = report
            .find("governance.quorum[3].threshold_percent")
            .expect("the diagnostic sits on the threshold");
        assert_eq!(found.code, DiagnosticCode::ThresholdTooLow);
        assert!(found.fix.contains("90 percent"));
    }

    #[test]
    fn a_threshold_above_one_hundred_percent_is_caught() {
        let mut governance = sample();
        governance.quorum[0].threshold_percent = 120;
        let report = run(&governance);
        let found = report
            .find("governance.quorum[0].threshold_percent")
            .expect("the diagnostic sits on the threshold");
        assert_eq!(found.code, DiagnosticCode::ValueOutOfRange);
    }

    #[test]
    fn a_constitutional_rule_without_a_delay_is_caught() {
        let mut governance = sample();
        governance.quorum[3].delay_millis = 0;
        let report = run(&governance);
        let found = report
            .find("governance.quorum[3].delay_millis")
            .expect("the diagnostic sits on the delay");
        assert_eq!(found.code, DiagnosticCode::ValueOutOfRange);
    }

    #[test]
    fn two_rules_for_one_class_are_caught() {
        let mut governance = sample();
        governance
            .quorum
            .push(QuorumRule::standard(TransactionClass::Routine));
        let report = run(&governance);
        let found = report
            .find("governance.quorum[4].class")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn an_authority_naming_an_absent_role_reports_its_location() {
        let mut governance = sample();
        governance.authority.roles = vec![RoleId::new("absent_role")];
        let report = run(&governance);
        let found = report
            .find("governance.authority.roles[0]")
            .expect("the diagnostic sits on the role");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_stage_naming_an_absent_role_reports_its_location() {
        let mut governance = sample();
        governance.process.stages[0].roles = vec![RoleId::new("absent_role")];
        let report = run(&governance);
        assert!(report
            .find("governance.process.stages[0].roles[0]")
            .is_some());
    }

    #[test]
    fn an_empty_change_process_is_caught() {
        let mut governance = sample();
        governance.process.stages = Vec::new();
        let report = run(&governance);
        let found = report
            .find("governance.process.stages")
            .expect("the diagnostic sits on the stages");
        assert_eq!(found.code, DiagnosticCode::SectionEmpty);
    }

    #[test]
    fn a_class_left_out_carries_the_floor_of_the_class() {
        let mut governance = sample();
        governance
            .quorum
            .retain(|rule| rule.class != TransactionClass::Structural);
        assert_eq!(governance.threshold_for(TransactionClass::Structural), 80);
        assert_eq!(governance.threshold_for(TransactionClass::Routine), 51);
        assert!(governance.rule_for(TransactionClass::Structural).is_none());
    }

    #[test]
    fn the_threshold_location_points_at_the_control() {
        let location = threshold_location(&Path::root().field("governance"), 2);
        assert_eq!(location.as_str(), "governance.quorum[2].threshold_percent");
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let governance = sample();
        let bytes = governance.encode();
        let back = Governance::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(governance, back);
        let text = serde_json::to_string(&governance).expect("the section serialises");
        let parsed: Governance = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(governance, parsed);
    }
}
