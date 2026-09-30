//! Behavioural tests for events and integrations.
//!
//! Each test starts from a clean section, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    actions::{Action, ActionRegistry},
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    events::{validate_events, AuditPolicy, DeliveryTarget, EventDef, EventRegistry, Subscriber},
    identities::{CredentialDef, CredentialIssuer, CredentialValidity, Identities, RoleDef},
    integrations::{
        AttestationRequirement, BridgeDirection, BridgeEndpoint, ExternalTrigger, Feed,
        Integrations,
    },
    types::{
        ActionId, CollectionId, CredentialId, EventId, FieldDef, IntegrationId, RecordDef, TypeDef,
        TypeExpr, TypeId, TypeRegistry, WorkflowId,
    },
    workflows::{Workflow, WorkflowRegistry},
    DiagnosticCode, Location, Path, Report,
};
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("rate"),
            fields: vec![FieldDef::new("code", TypeExpr::Text, "The currency code.")],
            summary: "One exchange rate.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("notice"),
            fields: vec![FieldDef::new("text", TypeExpr::Text, "The notice.")],
            summary: "One notice.".to_string(),
        }),
    ]
}

fn collections() -> Vec<Collection> {
    let mut rates = Collection::new("rates", "rate", "code", "The rates.");
    rates.purpose = PurposeDeclaration::new("The desk keeps rates.", &["pricing"]);
    vec![rates]
}

fn identities() -> Identities {
    Identities {
        roles: vec![
            RoleDef::new("desk", None, "Desk", "The desk signs rates."),
            RoleDef::new("auditor", None, "Auditor", "The auditor signs too."),
        ],
        credentials: vec![CredentialDef {
            id: CredentialId::new("seal"),
            issuer: CredentialIssuer::External {
                authority: "Central bank".to_string(),
            },
            validity: CredentialValidity::UntilRevoked,
            attests: "The holder signs rates.".to_string(),
        }],
        ..Identities::default()
    }
}

fn attestation() -> AttestationRequirement {
    let mut held = AttestationRequirement::signed_by(2, &["desk", "auditor"]);
    held.credentials = vec![CredentialId::new("seal")];
    held
}

fn clean() -> Integrations {
    Integrations {
        feeds: vec![Feed {
            id: IntegrationId::new("rate_feed"),
            target: CollectionId::new("rates"),
            record_type: TypeId::new("rate"),
            refresh_millis: 60_000,
            attestation: attestation(),
            summary: "The feed brings rates.".to_string(),
        }],
        bridges: vec![BridgeEndpoint {
            id: IntegrationId::new("bank_bridge"),
            network: "Central bank ledger".to_string(),
            direction: BridgeDirection::BothWays,
            collection: CollectionId::new("rates"),
            attestation: attestation(),
            summary: "The bridge shares rates.".to_string(),
        }],
        triggers: vec![ExternalTrigger {
            id: IntegrationId::new("bank_order"),
            action: ActionId::new("post_rate"),
            attestation: attestation(),
            summary: "The bank posts a rate.".to_string(),
        }],
    }
}

fn report(integrations: &Integrations) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let action_defs = vec![Action::new("post_rate", "The action posts a rate.")];
    let actions = ActionRegistry::build(&action_defs);
    integrations.validate(
        &registry,
        &collections,
        &identities(),
        &actions,
        &Path::root().field("integrations"),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_section_passes_and_names_its_integrations() {
    let integrations = clean();
    assert!(report(&integrations).is_clean());
    assert!(integrations.contains(&IntegrationId::new("rate_feed")));
    assert!(integrations.contains(&IntegrationId::new("bank_bridge")));
    assert!(integrations.contains(&IntegrationId::new("bank_order")));
    assert!(!integrations.contains(&IntegrationId::new("ghost")));
    assert_eq!(
        integrations.names(),
        ["rate_feed", "bank_bridge", "bank_order"]
    );
    assert_eq!(
        integrations
            .missing(&IntegrationId::new("ghost"), Location::default())
            .fix,
        "Point the entry at one of these integrations: rate_feed, bank_bridge, bank_order."
    );
    assert_eq!(
        Integrations::default()
            .missing(&IntegrationId::new("ghost"), Location::default())
            .fix,
        "Declare an integration named `ghost` in the integrations section."
    );
}

#[test]
fn feed_faults_each_raise_a_diagnostic() {
    let mut integrations = clean();
    integrations.feeds.push(Feed {
        id: IntegrationId::new("rate_feed"),
        target: CollectionId::new("rates"),
        record_type: TypeId::new("notice"),
        refresh_millis: 0,
        attestation: attestation(),
        summary: String::new(),
    });
    integrations.feeds.push(Feed {
        id: IntegrationId::new("Bad"),
        target: CollectionId::new("ghost"),
        record_type: TypeId::new("ghost"),
        refresh_millis: 5,
        attestation: attestation(),
        summary: "A broken feed.".to_string(),
    });
    let found = report(&integrations);
    assert_eq!(
        at(&found, "integrations.feeds[1].id").fix,
        "Rename one of the two integrations away from `rate_feed`."
    );
    assert_eq!(
        at(&found, "integrations.feeds[1].summary").fix,
        "Write one plain sentence stating what the integration carries."
    );
    let carried = at(&found, "integrations.feeds[1].record_type");
    assert_eq!(
        carried.problem,
        "The feed `rate_feed` carries `notice` and `rates` holds `rate`."
    );
    assert_eq!(carried.fix, "Carry the record type `rate` on the feed.");
    assert_eq!(
        at(&found, "integrations.feeds[1].refresh_millis").fix,
        "Set a span above zero."
    );
    assert_eq!(
        at(&found, "integrations.feeds[2].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "integrations.feeds[2].target").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "integrations.feeds[2].record_type").fix,
        "Declare a record type named `ghost` in the types section."
    );
}

#[test]
fn bridge_and_trigger_faults_each_raise_a_diagnostic() {
    let mut integrations = clean();
    integrations.bridges.push(BridgeEndpoint {
        id: IntegrationId::new("rate_feed"),
        network: String::new(),
        direction: BridgeDirection::Inbound,
        collection: CollectionId::new("ghost"),
        attestation: attestation(),
        summary: String::new(),
    });
    integrations.triggers.push(ExternalTrigger {
        id: IntegrationId::new("bank_bridge"),
        action: ActionId::new("ghost_action"),
        attestation: attestation(),
        summary: String::new(),
    });
    let found = report(&integrations);
    assert_eq!(
        at(&found, "integrations.bridges[1].id").code,
        DiagnosticCode::DuplicateIdentifier
    );
    assert_eq!(
        at(&found, "integrations.bridges[1].network").fix,
        "Name the network on the other side of the endpoint."
    );
    assert_eq!(
        at(&found, "integrations.bridges[1].collection").fix,
        "Point the reference at one of these collections: rates."
    );
    assert_eq!(
        at(&found, "integrations.bridges[1].summary").code,
        DiagnosticCode::MissingValue
    );
    assert_eq!(
        at(&found, "integrations.triggers[1].id").fix,
        "Rename one of the two integrations away from `bank_bridge`."
    );
    assert_eq!(
        at(&found, "integrations.triggers[1].action").fix,
        "Point the entry at one of these actions: post_rate."
    );
    assert_eq!(
        at(&found, "integrations.triggers[1].summary").code,
        DiagnosticCode::MissingValue
    );
}

#[test]
fn attestation_faults_each_raise_a_diagnostic() {
    let path = Path::root().field("attestation");
    assert!(attestation().validate(&identities(), &path).is_clean());
    let empty = AttestationRequirement::signed_by(0, &[]);
    let found = empty.validate(&identities(), &path);
    assert_eq!(
        at(&found, "attestation.signers").fix,
        "Require at least one signature."
    );
    assert_eq!(
        at(&found, "attestation.roles").fix,
        "Name at least one role that signs."
    );
    let mut greedy = AttestationRequirement::signed_by(3, &["Desk", "treasurer"]);
    greedy.credentials = vec![CredentialId::new("Stamp"), CredentialId::new("stamp")];
    let found = greedy.validate(&identities(), &path);
    let signers = at(&found, "attestation.signers");
    assert_eq!(
        signers.problem,
        "The attestation requires 3 signatures from 2 roles."
    );
    assert_eq!(signers.fix, "Name more roles, or lower the count to 2.");
    assert_eq!(
        at(&found, "attestation.roles[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "attestation.roles[1]").fix,
        "Point the entry at one of these roles: desk, auditor."
    );
    assert_eq!(
        at(&found, "attestation.credentials[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "attestation.credentials[1]").fix,
        "Declare a credential named `stamp` in the identities section."
    );
}

fn events() -> Vec<EventDef> {
    let mut posted = EventDef::new("posted", "rate", "The desk posted a rate.");
    posted.subscribers = vec![
        Subscriber::workflow("rate_life", "The workflow follows the rate."),
        Subscriber::view("rates_view", "The view lists rates."),
        Subscriber::integration("rate_feed", "The feed hears of rates."),
    ];
    vec![posted]
}

fn events_report(events: &[EventDef], workflows: &[Workflow]) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let workflows = WorkflowRegistry::build(workflows);
    validate_events(events, &registry, &workflows, &Path::root().field("events"))
}

#[test]
fn events_name_their_payload_their_policy_and_their_subscribers() {
    let events = events();
    let posted = &events[0];
    assert_eq!(posted.payload_type(), TypeExpr::Record(TypeId::new("rate")));
    assert_eq!(posted.audit, AuditPolicy::Keep);
    let labels: Vec<&str> = posted
        .subscribers
        .iter()
        .map(|subscriber| subscriber.target.label())
        .collect();
    assert_eq!(labels, ["workflow", "view", "integration"]);
    let path = Path::root().field("events").index(0);
    let views = posted.view_refs(&path);
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].1.as_str(), "events[0].subscribers[1].target");
    let feeds = posted.integration_refs(&path);
    assert_eq!(feeds[0].0, IntegrationId::new("rate_feed"));
    assert_eq!(feeds[0].1.as_str(), "events[0].subscribers[2].target");
    assert!(matches!(
        posted.subscribers[0].target,
        DeliveryTarget::Workflow(ref id) if id == &WorkflowId::new("rate_life")
    ));

    let registry = EventRegistry::build(&events);
    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());
    assert!(registry.contains(&EventId::new("posted")));
    assert_eq!(
        registry
            .get(&EventId::new("posted"))
            .map(|event| event.summary.as_str()),
        Some("The desk posted a rate.")
    );
    assert_eq!(registry.names(), ["posted"]);
    let none: Vec<EventDef> = Vec::new();
    let empty = EventRegistry::build(&none);
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&EventId::new("ghost"), Location::default())
            .fix,
        "Declare an event named `ghost` in the events section."
    );
}

#[test]
fn event_faults_each_raise_a_diagnostic() {
    let mut events = events();
    let mut repeated = EventDef::new("posted", "ghost", "");
    repeated.subscribers = vec![Subscriber::workflow("ghost_flow", "An absent workflow.")];
    events.push(repeated);
    events.push(EventDef::new("Bad", "Rate", "A malformed event."));
    let found = events_report(&events, &[]);
    assert_eq!(
        at(&found, "events[0].subscribers[0].target").fix,
        "Declare a workflow named `rate_life` in the workflows section."
    );
    assert_eq!(
        at(&found, "events[1].id").fix,
        "Rename one of the two events away from `posted`."
    );
    assert_eq!(
        at(&found, "events[1].payload").fix,
        "Declare a record type named `ghost` in the types section."
    );
    assert_eq!(
        at(&found, "events[1].summary").fix,
        "Write one plain sentence stating what the event records."
    );
    assert_eq!(
        at(&found, "events[1].subscribers[0].target").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "events[2].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "events[2].payload").code,
        DiagnosticCode::InvalidIdentifier
    );
}
