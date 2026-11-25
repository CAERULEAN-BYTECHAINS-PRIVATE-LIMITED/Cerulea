//! Behavioural tests for the identities section.
//!
//! Each test starts from a clean section, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    identities::{
        CredentialDef, CredentialIssuer, CredentialValidity, DelegationRule, Identities,
        OrganisationUnit, RoleDef,
    },
    types::{ActionId, CredentialId, DelegationId, RoleId, UnitId},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

fn clean() -> Identities {
    let mut registrar = RoleDef::new(
        "registrar",
        Some("district"),
        "Registrar",
        "The registrar records parcels.",
    );
    registrar.requires = vec![CredentialId::new("licence")];
    Identities {
        units: vec![
            OrganisationUnit::top("ministry", "Ministry", "The ministry sets policy."),
            OrganisationUnit::under(
                "district",
                "ministry",
                "District office",
                "The office serves one district.",
            ),
        ],
        roles: vec![
            registrar,
            RoleDef::new("clerk", None, "Clerk", "The clerk files papers."),
        ],
        credentials: vec![
            CredentialDef {
                id: CredentialId::new("licence"),
                issuer: CredentialIssuer::Unit(UnitId::new("ministry")),
                validity: CredentialValidity::UntilRevoked,
                attests: "The holder passed the registrar examination.".to_string(),
            },
            CredentialDef {
                id: CredentialId::new("badge"),
                issuer: CredentialIssuer::Role(RoleId::new("registrar")),
                validity: CredentialValidity::Milliseconds(86_400_000),
                attests: "The holder works in the district office.".to_string(),
            },
            CredentialDef {
                id: CredentialId::new("degree"),
                issuer: CredentialIssuer::External {
                    authority: "State University".to_string(),
                },
                validity: CredentialValidity::UntilRevoked,
                attests: "The holder studied law.".to_string(),
            },
        ],
        delegations: vec![DelegationRule {
            id: DelegationId::new("cover"),
            from: RoleId::new("registrar"),
            to: RoleId::new("clerk"),
            scope: vec![ActionId::new("record_parcel"), ActionId::new("close_file")],
            maximum_millis: 3_600_000,
            requires: Some(CredentialId::new("badge")),
        }],
    }
}

fn report(identities: &Identities) -> Report {
    identities.validate(&Path::root().field("identities"))
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_section_passes_and_answers_lookups() {
    let identities = clean();
    assert!(report(&identities).is_clean());
    assert!(identities.has_role(&RoleId::new("clerk")));
    assert!(!identities.has_role(&RoleId::new("auditor")));
    assert_eq!(
        identities
            .role(&RoleId::new("registrar"))
            .map(|role| role.title.as_str()),
        Some("Registrar")
    );
    assert!(identities.role(&RoleId::new("auditor")).is_none());
    assert_eq!(identities.role_names(), ["registrar", "clerk"]);
}

#[test]
fn the_delegation_scope_lists_each_action_with_its_location() {
    let refs = clean().action_refs(&Path::root().field("identities"));
    let listed: Vec<(&str, &str)> = refs
        .iter()
        .map(|(action, location)| (action.as_str(), location.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("record_parcel", "identities.delegations[0].scope[0]"),
            ("close_file", "identities.delegations[0].scope[1]"),
        ]
    );
}

#[test]
fn the_unit_chain_climbs_to_the_top_and_stops_on_a_loop() {
    let identities = clean();
    let chain = identities.unit_chain(&UnitId::new("district"));
    let names: Vec<&str> = chain.iter().map(|unit| unit.as_str()).collect();
    assert_eq!(names, ["district", "ministry"]);

    let mut looped = clean();
    looped.units[0].parent = Some(UnitId::new("district"));
    let chain = looped.unit_chain(&UnitId::new("district"));
    let names: Vec<&str> = chain.iter().map(|unit| unit.as_str()).collect();
    assert_eq!(names, ["district", "ministry"]);

    let lone = identities.unit_chain(&UnitId::new("elsewhere"));
    assert_eq!(lone, [UnitId::new("elsewhere")]);
}

#[test]
fn a_missing_role_names_the_roles_on_offer() {
    let listed = clean().missing_role(&RoleId::new("auditor"), Location("here".to_string()));
    assert_eq!(listed.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        listed.problem,
        "The entry names the role `auditor`, which the Blueprint lacks."
    );
    assert_eq!(
        listed.fix,
        "Point the entry at one of these roles: registrar, clerk."
    );
    let empty = Identities::default().missing_role(&RoleId::new("auditor"), Location::default());
    assert_eq!(
        empty.fix,
        "Declare a role named `auditor` in the identities section."
    );
}

#[test]
fn unit_faults_each_raise_a_diagnostic() {
    let mut identities = clean();
    identities.units.push(OrganisationUnit::top(
        "Bad Name",
        "",
        "A unit with a malformed name.",
    ));
    identities.units.push(OrganisationUnit::under(
        "ministry",
        "nowhere",
        "Second ministry",
        "A repeated unit.",
    ));
    let found = report(&identities);

    let malformed = at(&found, "identities.units[2].id");
    assert_eq!(malformed.code, DiagnosticCode::InvalidIdentifier);
    let untitled = at(&found, "identities.units[2].title");
    assert_eq!(untitled.code, DiagnosticCode::MissingValue);
    assert_eq!(
        untitled.fix,
        "Give the unit the name a person reads on screen."
    );
    let repeated = at(&found, "identities.units[3].id");
    assert_eq!(repeated.code, DiagnosticCode::DuplicateIdentifier);
    assert_eq!(
        repeated.fix,
        "Rename one of the two units away from `ministry`."
    );
    let orphan = at(&found, "identities.units[3].parent");
    assert_eq!(orphan.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        orphan.fix,
        "Declare a unit named `nowhere`, or clear the parent."
    );
}

#[test]
fn a_loop_in_the_organisation_tree_is_reported_once() {
    let mut identities = clean();
    identities.units[0].parent = Some(UnitId::new("district"));
    let found = report(&identities);
    assert_eq!(found.len(), 1);
    let cycle = &found.diagnostics[0];
    assert_eq!(cycle.code, DiagnosticCode::CycleDetected);
    assert_eq!(cycle.location.as_str(), "identities.units[1].parent");
    assert_eq!(
        cycle.problem,
        "The organisation tree closes a loop: district -> ministry -> district."
    );
    assert_eq!(
        cycle.fix,
        "Point one unit on the loop at a different parent, or clear its parent."
    );
}

#[test]
fn credential_faults_each_raise_a_diagnostic() {
    let mut identities = clean();
    identities.credentials.push(CredentialDef {
        id: CredentialId::new("licence"),
        issuer: CredentialIssuer::Unit(UnitId::new("tribunal")),
        validity: CredentialValidity::Milliseconds(0),
        attests: String::new(),
    });
    identities.credentials.push(CredentialDef {
        id: CredentialId::new("9permit"),
        issuer: CredentialIssuer::Role(RoleId::new("auditor")),
        validity: CredentialValidity::UntilRevoked,
        attests: "The holder holds a permit.".to_string(),
    });
    identities.credentials.push(CredentialDef {
        id: CredentialId::new("seal"),
        issuer: CredentialIssuer::External {
            authority: String::new(),
        },
        validity: CredentialValidity::UntilRevoked,
        attests: "The holder holds a seal.".to_string(),
    });
    let found = report(&identities);

    assert_eq!(
        at(&found, "identities.credentials[3].id").fix,
        "Rename one of the two credentials away from `licence`."
    );
    assert_eq!(
        at(&found, "identities.credentials[3].attests").fix,
        "Write one plain sentence stating what the credential attests."
    );
    let issuer = at(&found, "identities.credentials[3].issuer");
    assert_eq!(issuer.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        issuer.fix,
        "Declare a unit named `tribunal` in the identities section."
    );
    let validity = at(&found, "identities.credentials[3].validity");
    assert_eq!(validity.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        validity.fix,
        "Set a span above zero, or hold the credential until a revocation lands."
    );
    assert_eq!(
        at(&found, "identities.credentials[4].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "identities.credentials[4].issuer").fix,
        "Point the entry at one of these roles: registrar, clerk."
    );
    let authority = at(&found, "identities.credentials[5].issuer");
    assert_eq!(authority.code, DiagnosticCode::MissingValue);
    assert_eq!(
        authority.fix,
        "Name the authority that issues the credential."
    );
}

#[test]
fn role_faults_each_raise_a_diagnostic() {
    let mut identities = clean();
    let mut stray = RoleDef::new("clerk", Some("annex"), "", "A repeated role.");
    stray.requires = vec![CredentialId::new("Pass"), CredentialId::new("permit")];
    identities.roles.push(stray);
    identities
        .roles
        .push(RoleDef::new("_hidden", None, "Hidden", "A malformed role."));
    let found = report(&identities);

    assert_eq!(
        at(&found, "identities.roles[2].id").fix,
        "Rename one of the two roles away from `clerk`."
    );
    assert_eq!(
        at(&found, "identities.roles[2].title").fix,
        "Give the role the name a person reads on screen."
    );
    assert_eq!(
        at(&found, "identities.roles[2].unit").fix,
        "Declare a unit named `annex`, or clear the unit."
    );
    let malformed = at(&found, "identities.roles[2].requires[0]");
    assert_eq!(malformed.code, DiagnosticCode::InvalidIdentifier);
    let missing: Vec<&Diagnostic> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "identities.roles[2].requires[1]")
        .collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(
        missing[0].fix,
        "Declare a credential named `permit` in the identities section."
    );
    assert_eq!(
        at(&found, "identities.roles[3].id").code,
        DiagnosticCode::InvalidIdentifier
    );
}

#[test]
fn delegation_faults_each_raise_a_diagnostic() {
    let mut identities = clean();
    identities.delegations.push(DelegationRule {
        id: DelegationId::new("cover"),
        from: RoleId::new("auditor"),
        to: RoleId::new("inspector"),
        scope: Vec::new(),
        maximum_millis: 0,
        requires: Some(CredentialId::new("warrant")),
    });
    let found = report(&identities);

    assert_eq!(
        at(&found, "identities.delegations[1].id").fix,
        "Rename one of the two rules away from `cover`."
    );
    assert_eq!(
        at(&found, "identities.delegations[1].from").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "identities.delegations[1].to").problem,
        "The entry names the role `inspector`, which the Blueprint lacks."
    );
    assert_eq!(
        at(&found, "identities.delegations[1].scope").fix,
        "Name at least one action the receiving role runs."
    );
    assert_eq!(
        at(&found, "identities.delegations[1].maximum_millis").fix,
        "Set a span above zero."
    );
    assert_eq!(
        at(&found, "identities.delegations[1].requires").fix,
        "Declare a credential named `warrant` in the identities section."
    );
}

#[test]
fn a_rule_passing_a_scope_to_its_own_role_is_a_loop() {
    let mut identities = clean();
    identities.delegations[0].to = RoleId::new("registrar");
    let found = report(&identities);
    let codes: Vec<(DiagnosticCode, &str)> = found
        .diagnostics
        .iter()
        .map(|d| (d.code, d.location.as_str()))
        .collect();
    assert_eq!(
        codes,
        [
            (
                DiagnosticCode::CycleDetected,
                "identities.delegations[0].to"
            ),
            (
                DiagnosticCode::CycleDetected,
                "identities.delegations[0].to"
            ),
        ]
    );
    assert_eq!(
        found.diagnostics[0].problem,
        "The rule `cover` passes a scope from a role to itself."
    );
    assert_eq!(
        found.diagnostics[1].problem,
        "The delegation rules close a loop: registrar -> registrar."
    );
}

#[test]
fn two_rules_passing_scope_both_ways_close_a_loop() {
    let mut identities = clean();
    identities.delegations.push(DelegationRule {
        id: DelegationId::new("return"),
        from: RoleId::new("clerk"),
        to: RoleId::new("registrar"),
        scope: vec![ActionId::new("record_parcel")],
        maximum_millis: 60_000,
        requires: None,
    });
    let found = report(&identities);
    assert_eq!(found.len(), 1);
    let cycle = &found.diagnostics[0];
    assert_eq!(cycle.code, DiagnosticCode::CycleDetected);
    assert_eq!(cycle.location.as_str(), "identities.delegations[1].to");
    assert_eq!(
        cycle.problem,
        "The delegation rules close a loop: clerk -> registrar -> clerk."
    );
    assert_eq!(
        cycle.fix,
        "Remove one rule on the loop so authority flows one way."
    );
}

#[test]
fn the_section_round_trips_through_scale_and_json() {
    use codec::{Decode, Encode};
    let identities = clean();
    let bytes = identities.encode();
    assert_eq!(
        Identities::decode(&mut &bytes[..]).expect("the bytes decode"),
        identities
    );
    let text = serde_json::to_string(&identities).expect("the section serialises");
    let back: Identities = serde_json::from_str(&text).expect("the text parses");
    assert_eq!(back, identities);
}
