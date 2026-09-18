//! Organisational units, roles, credential requirements and delegation.
//!
//! A unit is one part of an organisation. A role is a named capacity a
//! person holds inside a unit. A credential is an attestation a role
//! requires. A delegation rule lets one role pass a named scope to another
//! role for a bounded span.

use crate::{
    describe_cycle, find_cycle,
    types::{ActionId, CredentialId, DelegationId, RoleId, UnitId},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The party that issues a credential.
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
pub enum CredentialIssuer {
    /// A unit inside this deployment issues the credential.
    Unit(UnitId),
    /// A role inside this deployment issues the credential.
    Role(RoleId),
    /// A named authority outside this deployment issues the credential.
    External {
        /// The name of the authority.
        authority: String,
    },
}

/// The span a credential stays valid.
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
pub enum CredentialValidity {
    /// The credential stays valid until a revocation lands.
    UntilRevoked,
    /// The credential stays valid for the given span in milliseconds.
    Milliseconds(u64),
}

/// One part of an organisation.
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
pub struct OrganisationUnit {
    /// The name of the unit.
    pub id: UnitId,
    /// The unit that holds this unit. A top unit names no parent.
    pub parent: Option<UnitId>,
    /// The name a person reads on screen.
    pub title: String,
    /// One plain sentence stating what the unit does.
    pub summary: String,
}

impl OrganisationUnit {
    /// Declare a top level unit.
    pub fn top(id: &str, title: &str, summary: &str) -> Self {
        OrganisationUnit {
            id: UnitId::new(id),
            parent: None,
            title: title.to_string(),
            summary: summary.to_string(),
        }
    }

    /// Declare a unit under a parent.
    pub fn under(id: &str, parent: &str, title: &str, summary: &str) -> Self {
        OrganisationUnit {
            id: UnitId::new(id),
            parent: Some(UnitId::new(parent)),
            title: title.to_string(),
            summary: summary.to_string(),
        }
    }
}

/// A named capacity a person holds.
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
pub struct RoleDef {
    /// The name of the role.
    pub id: RoleId,
    /// The unit the role sits in. A deployment wide role names no unit.
    pub unit: Option<UnitId>,
    /// The credentials a holder presents to take the role.
    pub requires: Vec<CredentialId>,
    /// The name a person reads on screen.
    pub title: String,
    /// One plain sentence stating what the role does.
    pub summary: String,
}

impl RoleDef {
    /// Declare a role sitting in a unit.
    pub fn new(id: &str, unit: Option<&str>, title: &str, summary: &str) -> Self {
        RoleDef {
            id: RoleId::new(id),
            unit: unit.map(UnitId::new),
            requires: Vec::new(),
            title: title.to_string(),
            summary: summary.to_string(),
        }
    }
}

/// An attestation a role requires.
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
pub struct CredentialDef {
    /// The name of the credential.
    pub id: CredentialId,
    /// The party that issues the credential.
    pub issuer: CredentialIssuer,
    /// The span the credential stays valid.
    pub validity: CredentialValidity,
    /// One plain sentence stating what the credential attests.
    pub attests: String,
}

/// A rule letting one role pass a scope to another role.
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
pub struct DelegationRule {
    /// The name of the rule.
    pub id: DelegationId,
    /// The role that passes the scope.
    pub from: RoleId,
    /// The role that receives the scope.
    pub to: RoleId,
    /// The actions the receiving role runs under the delegation.
    pub scope: Vec<ActionId>,
    /// The longest span a delegation under this rule lasts, in milliseconds.
    pub maximum_millis: u64,
    /// A credential the receiving role presents first.
    pub requires: Option<CredentialId>,
}

/// The identities section.
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
pub struct Identities {
    /// The organisation tree.
    pub units: Vec<OrganisationUnit>,
    /// The roles.
    pub roles: Vec<RoleDef>,
    /// The credential requirements.
    pub credentials: Vec<CredentialDef>,
    /// The delegation rules.
    pub delegations: Vec<DelegationRule>,
}

impl Identities {
    /// Report whether a role exists.
    pub fn has_role(&self, id: &RoleId) -> bool {
        self.roles.iter().any(|role| &role.id == id)
    }

    /// Look up a role.
    pub fn role(&self, id: &RoleId) -> Option<&RoleDef> {
        self.roles.iter().find(|role| &role.id == id)
    }

    /// The role names, in declaration order.
    pub fn role_names(&self) -> Vec<&str> {
        self.roles.iter().map(|role| role.id.as_str()).collect()
    }

    /// Raise a dangling reference diagnostic for a role the Blueprint lacks.
    pub fn missing_role(&self, id: &RoleId, location: Location) -> Diagnostic {
        let listed = self.role_names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the role `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a role named `{id}` in the identities section.")
            } else {
                format!("Point the entry at one of these roles: {listed}.")
            },
        )
    }

    /// The actions the delegation rules name, each with its location.
    pub fn action_refs(&self, path: &Path) -> Vec<(ActionId, Location)> {
        let mut found = Vec::new();
        for (position, rule) in self.delegations.iter().enumerate() {
            let here = path.field("delegations").index(position).field("scope");
            for (index, action) in rule.scope.iter().enumerate() {
                found.push((action.clone(), here.index(index).location()));
            }
        }
        found
    }

    /// The units a role reaches through its parents, closest first.
    pub fn unit_chain(&self, start: &UnitId) -> Vec<UnitId> {
        let mut chain = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut current = Some(start.clone());
        while let Some(id) = current {
            if !seen.insert(id.as_str().to_string()) {
                break;
            }
            chain.push(id.clone());
            current = self
                .units
                .iter()
                .find(|unit| unit.id == id)
                .and_then(|unit| unit.parent.clone());
        }
        chain
    }

    /// Check the whole identities section.
    pub fn validate(&self, path: &Path) -> Report {
        let mut report = Report::new();
        self.validate_units(path, &mut report);
        self.validate_credentials(path, &mut report);
        self.validate_roles(path, &mut report);
        self.validate_delegations(path, &mut report);
        report
    }

    fn validate_units(&self, path: &Path, report: &mut Report) {
        let units_path = path.field("units");
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (position, unit) in self.units.iter().enumerate() {
            let here = units_path.index(position);
            report.absorb(unit.id.validate(&here.field("id")));
            if !seen.insert(unit.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two units share the name `{}`.", unit.id),
                    format!("Rename one of the two units away from `{}`.", unit.id),
                ));
            }
            if unit.title.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("title").location(),
                    format!("The unit `{}` carries an empty title.", unit.id),
                    "Give the unit the name a person reads on screen.".to_string(),
                ));
            }
            let targets = edges.entry(unit.id.as_str().to_string()).or_default();
            if let Some(parent) = &unit.parent {
                targets.push(parent.as_str().to_string());
                if !self.units.iter().any(|candidate| &candidate.id == parent) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.field("parent").location(),
                        format!(
                            "The unit `{}` names the parent `{parent}`, which the Blueprint lacks.",
                            unit.id
                        ),
                        format!("Declare a unit named `{parent}`, or clear the parent."),
                    ));
                }
            }
        }
        if let Some(cycle) = find_cycle(&edges) {
            let name = cycle.first().cloned().unwrap_or_default();
            let position = self
                .units
                .iter()
                .position(|unit| unit.id.as_str() == name)
                .unwrap_or(0);
            report.push(Diagnostic::at(
                DiagnosticCode::CycleDetected,
                units_path.index(position).field("parent").location(),
                format!(
                    "The organisation tree closes a loop: {}.",
                    describe_cycle(&cycle)
                ),
                "Point one unit on the loop at a different parent, or clear its parent."
                    .to_string(),
            ));
        }
    }

    fn validate_credentials(&self, path: &Path, report: &mut Report) {
        let credentials_path = path.field("credentials");
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (position, credential) in self.credentials.iter().enumerate() {
            let here = credentials_path.index(position);
            report.absorb(credential.id.validate(&here.field("id")));
            if !seen.insert(credential.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two credentials share the name `{}`.", credential.id),
                    format!(
                        "Rename one of the two credentials away from `{}`.",
                        credential.id
                    ),
                ));
            }
            if credential.attests.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("attests").location(),
                    format!("The credential `{}` states nothing.", credential.id),
                    "Write one plain sentence stating what the credential attests.".to_string(),
                ));
            }
            match &credential.issuer {
                CredentialIssuer::Unit(unit) => {
                    if !self.units.iter().any(|candidate| &candidate.id == unit) {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            here.field("issuer").location(),
                            format!(
                                "The credential `{}` names the unit `{unit}`, which the Blueprint lacks.",
                                credential.id
                            ),
                            format!("Declare a unit named `{unit}` in the identities section."),
                        ));
                    }
                }
                CredentialIssuer::Role(role) => {
                    if !self.has_role(role) {
                        report.push(self.missing_role(role, here.field("issuer").location()));
                    }
                }
                CredentialIssuer::External { authority } => {
                    if authority.is_empty() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::MissingValue,
                            here.field("issuer").location(),
                            format!(
                                "The credential `{}` names an empty authority.",
                                credential.id
                            ),
                            "Name the authority that issues the credential.".to_string(),
                        ));
                    }
                }
            }
            if let CredentialValidity::Milliseconds(0) = credential.validity {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("validity").location(),
                    format!(
                        "The credential `{}` stays valid for zero time.",
                        credential.id
                    ),
                    "Set a span above zero, or hold the credential until a revocation lands."
                        .to_string(),
                ));
            }
        }
    }

    fn validate_roles(&self, path: &Path, report: &mut Report) {
        let roles_path = path.field("roles");
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (position, role) in self.roles.iter().enumerate() {
            let here = roles_path.index(position);
            report.absorb(role.id.validate(&here.field("id")));
            if !seen.insert(role.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two roles share the name `{}`.", role.id),
                    format!("Rename one of the two roles away from `{}`.", role.id),
                ));
            }
            if role.title.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("title").location(),
                    format!("The role `{}` carries an empty title.", role.id),
                    "Give the role the name a person reads on screen.".to_string(),
                ));
            }
            if let Some(unit) = &role.unit {
                if !self.units.iter().any(|candidate| &candidate.id == unit) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.field("unit").location(),
                        format!(
                            "The role `{}` names the unit `{unit}`, which the Blueprint lacks.",
                            role.id
                        ),
                        format!("Declare a unit named `{unit}`, or clear the unit."),
                    ));
                }
            }
            for (index, credential) in role.requires.iter().enumerate() {
                let credential_path = here.field("requires").index(index);
                report.absorb(credential.validate(&credential_path));
                if !self
                    .credentials
                    .iter()
                    .any(|candidate| &candidate.id == credential)
                {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        credential_path.location(),
                        format!(
                            "The role `{}` requires the credential `{credential}`, which the Blueprint lacks.",
                            role.id
                        ),
                        format!(
                            "Declare a credential named `{credential}` in the identities section."
                        ),
                    ));
                }
            }
        }
    }

    fn validate_delegations(&self, path: &Path, report: &mut Report) {
        let delegations_path = path.field("delegations");
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (position, rule) in self.delegations.iter().enumerate() {
            let here = delegations_path.index(position);
            report.absorb(rule.id.validate(&here.field("id")));
            if !seen.insert(rule.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two delegation rules share the name `{}`.", rule.id),
                    format!("Rename one of the two rules away from `{}`.", rule.id),
                ));
            }
            if !self.has_role(&rule.from) {
                report.push(self.missing_role(&rule.from, here.field("from").location()));
            }
            if !self.has_role(&rule.to) {
                report.push(self.missing_role(&rule.to, here.field("to").location()));
            }
            if rule.from == rule.to {
                report.push(Diagnostic::at(
                    DiagnosticCode::CycleDetected,
                    here.field("to").location(),
                    format!(
                        "The rule `{}` passes a scope from a role to itself.",
                        rule.id
                    ),
                    "Name a different receiving role.".to_string(),
                ));
            }
            if rule.scope.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    here.field("scope").location(),
                    format!("The rule `{}` passes zero actions.", rule.id),
                    "Name at least one action the receiving role runs.".to_string(),
                ));
            }
            if rule.maximum_millis == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("maximum_millis").location(),
                    format!("The rule `{}` lasts zero milliseconds.", rule.id),
                    "Set a span above zero.".to_string(),
                ));
            }
            if let Some(credential) = &rule.requires {
                if !self
                    .credentials
                    .iter()
                    .any(|candidate| &candidate.id == credential)
                {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.field("requires").location(),
                        format!(
                            "The rule `{}` requires the credential `{credential}`, which the Blueprint lacks.",
                            rule.id
                        ),
                        format!(
                            "Declare a credential named `{credential}` in the identities section."
                        ),
                    ));
                }
            }
            edges
                .entry(rule.from.as_str().to_string())
                .or_default()
                .push(rule.to.as_str().to_string());
        }
        if let Some(cycle) = find_cycle(&edges) {
            let start = cycle.first().cloned().unwrap_or_default();
            let position = self
                .delegations
                .iter()
                .position(|rule| rule.from.as_str() == start)
                .unwrap_or(0);
            report.push(Diagnostic::at(
                DiagnosticCode::CycleDetected,
                delegations_path.index(position).field("to").location(),
                format!(
                    "The delegation rules close a loop: {}.",
                    describe_cycle(&cycle)
                ),
                "Remove one rule on the loop so authority flows one way.".to_string(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn sample() -> Identities {
        Identities {
            units: vec![
                OrganisationUnit::top(
                    "district",
                    "District Administration",
                    "The district office.",
                ),
                OrganisationUnit::under(
                    "village_office",
                    "district",
                    "Village Office",
                    "The village office.",
                ),
            ],
            roles: vec![
                RoleDef {
                    id: RoleId::new("registrar"),
                    unit: Some(UnitId::new("district")),
                    requires: vec![CredentialId::new("service_record")],
                    title: "Registrar".to_string(),
                    summary: "The registrar records transfers.".to_string(),
                },
                RoleDef::new(
                    "clerk",
                    Some("village_office"),
                    "Clerk",
                    "The clerk prepares records.",
                ),
            ],
            credentials: vec![CredentialDef {
                id: CredentialId::new("service_record"),
                issuer: CredentialIssuer::Unit(UnitId::new("district")),
                validity: CredentialValidity::Milliseconds(31_536_000_000),
                attests: "The holder serves in the district office.".to_string(),
            }],
            delegations: vec![DelegationRule {
                id: DelegationId::new("registrar_to_clerk"),
                from: RoleId::new("registrar"),
                to: RoleId::new("clerk"),
                scope: vec![ActionId::new("record_transfer")],
                maximum_millis: 604_800_000,
                requires: Some(CredentialId::new("service_record")),
            }],
        }
    }

    #[test]
    fn a_complete_identities_section_validates() {
        let report = sample().validate(&Path::root().field("identities"));
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_role_naming_an_absent_unit_reports_its_location() {
        let mut identities = sample();
        identities.roles[1].unit = Some(UnitId::new("absent_unit"));
        let report = identities.validate(&Path::root().field("identities"));
        let found = report
            .find("identities.roles[1].unit")
            .expect("the diagnostic sits on the role unit");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_role_requiring_an_absent_credential_reports_its_location() {
        let mut identities = sample();
        identities.roles[0].requires = vec![CredentialId::new("absent_credential")];
        let report = identities.validate(&Path::root().field("identities"));
        assert!(report.find("identities.roles[0].requires[0]").is_some());
    }

    #[test]
    fn a_loop_in_the_organisation_tree_is_detected() {
        let mut identities = sample();
        identities.units[0].parent = Some(UnitId::new("village_office"));
        let report = identities.validate(&Path::root().field("identities"));
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::CycleDetected)
            .expect("the loop is reported");
        assert!(found.problem.contains("->"));
        assert!(found.location.as_str().starts_with("identities.units["));
    }

    #[test]
    fn a_loop_in_the_delegation_rules_is_detected() {
        let mut identities = sample();
        identities.delegations.push(DelegationRule {
            id: DelegationId::new("clerk_to_registrar"),
            from: RoleId::new("clerk"),
            to: RoleId::new("registrar"),
            scope: vec![ActionId::new("record_transfer")],
            maximum_millis: 1000,
            requires: None,
        });
        let report = identities.validate(&Path::root().field("identities"));
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::CycleDetected)
            .expect("the loop is reported");
        assert!(found.problem.contains("registrar"));
        assert!(found.fix.contains("one way"));
    }

    #[test]
    fn a_delegation_naming_an_absent_role_reports_the_fix() {
        let mut identities = sample();
        identities.delegations[0].to = RoleId::new("absent_role");
        let report = identities.validate(&Path::root().field("identities"));
        let found = report
            .find("identities.delegations[0].to")
            .expect("the diagnostic sits on the receiving role");
        assert!(found.fix.contains("registrar"));
    }

    #[test]
    fn the_unit_chain_walks_up_the_tree() {
        let identities = sample();
        let chain = identities.unit_chain(&UnitId::new("village_office"));
        assert_eq!(
            chain,
            vec![UnitId::new("village_office"), UnitId::new("district")]
        );
    }

    #[test]
    fn the_delegation_scope_reports_the_actions_it_names() {
        let identities = sample();
        let refs = identities.action_refs(&Path::root().field("identities"));
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].0, ActionId::new("record_transfer"));
        assert_eq!(refs[0].1.as_str(), "identities.delegations[0].scope[0]");
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let identities = sample();
        let bytes = identities.encode();
        let back = Identities::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(identities, back);
        let text = serde_json::to_string(&identities).expect("the section serialises");
        let parsed: Identities = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(identities, parsed);
    }
}
