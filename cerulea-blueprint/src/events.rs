//! Typed events.
//!
//! An event is a typed record written to the audit log. Subscribers read the
//! events they name. A subscriber is a workflow, a view or an integration.

use crate::{
    types::{EventId, IntegrationId, TypeExpr, TypeId, TypeRegistry, ViewId, WorkflowId},
    workflows::WorkflowRegistry,
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

/// Whether the audit log keeps the event.
#[derive(
    Clone,
    Copy,
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
pub enum AuditPolicy {
    /// The audit log keeps the event for the life of the deployment.
    #[default]
    Keep,
    /// The audit log keeps the event for the retention span of the
    /// deployment and clears it afterwards.
    Expire,
}

/// The party that reads an event.
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
pub enum DeliveryTarget {
    /// A workflow inside this deployment.
    Workflow(WorkflowId),
    /// A view inside this deployment.
    View(ViewId),
    /// A feed, a bridge endpoint or an external trigger.
    Integration(IntegrationId),
}

impl DeliveryTarget {
    /// A word naming the target for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            DeliveryTarget::Workflow(_) => "workflow",
            DeliveryTarget::View(_) => "view",
            DeliveryTarget::Integration(_) => "integration",
        }
    }
}

/// One party reading an event.
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
pub struct Subscriber {
    /// The party that reads the event.
    pub target: DeliveryTarget,
    /// One plain sentence stating what the subscriber does with the event.
    pub summary: String,
}

impl Subscriber {
    /// Declare a workflow subscriber.
    pub fn workflow(id: &str, summary: &str) -> Self {
        Subscriber {
            target: DeliveryTarget::Workflow(WorkflowId::new(id)),
            summary: summary.to_string(),
        }
    }

    /// Declare a view subscriber.
    pub fn view(id: &str, summary: &str) -> Self {
        Subscriber {
            target: DeliveryTarget::View(ViewId::new(id)),
            summary: summary.to_string(),
        }
    }

    /// Declare an integration subscriber.
    pub fn integration(id: &str, summary: &str) -> Self {
        Subscriber {
            target: DeliveryTarget::Integration(IntegrationId::new(id)),
            summary: summary.to_string(),
        }
    }
}

/// A typed event.
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
pub struct EventDef {
    /// The name of the event.
    pub id: EventId,
    /// The record type the payload carries.
    pub payload: TypeId,
    /// Whether the audit log keeps the event.
    pub audit: AuditPolicy,
    /// The parties reading the event.
    pub subscribers: Vec<Subscriber>,
    /// One plain sentence stating what the event records.
    pub summary: String,
}

impl EventDef {
    /// Declare an event kept in the audit log.
    pub fn new(id: &str, payload: &str, summary: &str) -> Self {
        EventDef {
            id: EventId::new(id),
            payload: TypeId::new(payload),
            audit: AuditPolicy::Keep,
            subscribers: Vec::new(),
            summary: summary.to_string(),
        }
    }

    /// The type of the payload.
    pub fn payload_type(&self) -> TypeExpr {
        TypeExpr::Record(self.payload.clone())
    }

    /// The views this event delivers to, each with its location.
    pub fn view_refs(&self, path: &Path) -> Vec<(ViewId, Location)> {
        self.subscribers
            .iter()
            .enumerate()
            .filter_map(|(position, subscriber)| match &subscriber.target {
                DeliveryTarget::View(id) => Some((
                    id.clone(),
                    path.field("subscribers")
                        .index(position)
                        .field("target")
                        .location(),
                )),
                _ => None,
            })
            .collect()
    }

    /// The integrations this event delivers to, each with its location.
    pub fn integration_refs(&self, path: &Path) -> Vec<(IntegrationId, Location)> {
        self.subscribers
            .iter()
            .enumerate()
            .filter_map(|(position, subscriber)| match &subscriber.target {
                DeliveryTarget::Integration(id) => Some((
                    id.clone(),
                    path.field("subscribers")
                        .index(position)
                        .field("target")
                        .location(),
                )),
                _ => None,
            })
            .collect()
    }
}

/// An index over the declared events.
#[derive(Clone, Debug, Default)]
pub struct EventRegistry<'a> {
    entries: BTreeMap<&'a str, &'a EventDef>,
}

impl<'a> EventRegistry<'a> {
    /// Build a registry over a slice of events.
    pub fn build(events: &'a [EventDef]) -> Self {
        let mut entries: BTreeMap<&'a str, &'a EventDef> = BTreeMap::new();
        for event in events {
            entries.entry(event.id.as_str()).or_insert(event);
        }
        EventRegistry { entries }
    }

    /// Look up an event.
    pub fn get(&self, id: &EventId) -> Option<&'a EventDef> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether an event exists.
    pub fn contains(&self, id: &EventId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The event names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the events.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero events.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for an event the Blueprint
    /// lacks.
    pub fn missing(&self, id: &EventId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the event `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare an event named `{id}` in the events section.")
            } else {
                format!("Point the entry at one of these events: {listed}.")
            },
        )
    }
}

/// Check a whole events section.
pub fn validate_events(
    events: &[EventDef],
    types: &TypeRegistry,
    workflows: &WorkflowRegistry,
    path: &Path,
) -> Report {
    let mut report = Report::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (position, event) in events.iter().enumerate() {
        let here = path.index(position);
        report.absorb(event.id.validate(&here.field("id")));
        if !seen.insert(event.id.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                here.field("id").location(),
                format!("Two events share the name `{}`.", event.id),
                format!("Rename one of the two events away from `{}`.", event.id),
            ));
        }
        report.absorb(event.payload.validate(&here.field("payload")));
        if types.record(&event.payload).is_none() {
            report.push(Diagnostic::at(
                DiagnosticCode::DanglingReference,
                here.field("payload").location(),
                format!(
                    "The event `{}` names the record type `{}`, which the Blueprint lacks.",
                    event.id, event.payload
                ),
                format!(
                    "Declare a record type named `{}` in the types section.",
                    event.payload
                ),
            ));
        }
        if event.summary.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                here.field("summary").location(),
                format!("The event `{}` states nothing.", event.id),
                "Write one plain sentence stating what the event records.".to_string(),
            ));
        }
        for (index, subscriber) in event.subscribers.iter().enumerate() {
            let subscriber_path = here.field("subscribers").index(index).field("target");
            if let DeliveryTarget::Workflow(workflow) = &subscriber.target {
                if !workflows.contains(workflow) {
                    report.push(workflows.missing(workflow, subscriber_path.location()));
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::{CollectionId, FieldDef, FieldPath, RecordDef, StateId, TypeDef},
        workflows::{CyclePolicy, Workflow, WorkflowState},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("transfer_record"),
            fields: vec![
                FieldDef::new("reference", TypeExpr::Text, "The transfer reference."),
                FieldDef::new("at", TypeExpr::Timestamp, "The time of the transfer."),
            ],
            summary: "The payload of a transfer event.".to_string(),
        })]
    }

    fn workflows() -> Vec<Workflow> {
        vec![Workflow {
            id: WorkflowId::new("claim_flow"),
            collection: CollectionId::new("claim"),
            state_field: FieldPath::parse("state"),
            states: vec![WorkflowState::passing(
                "lodged",
                "Lodged",
                "The claim arrived.",
            )],
            initial: StateId::new("lodged"),
            transitions: Vec::new(),
            timers: Vec::new(),
            deadlines: Vec::new(),
            cycles: CyclePolicy::Forbid,
            summary: "The claim workflow.".to_string(),
        }]
    }

    fn sample() -> Vec<EventDef> {
        let mut event = EventDef::new(
            "transfer_recorded",
            "transfer_record",
            "The event records one land transfer.",
        );
        event.subscribers = vec![
            Subscriber::workflow("claim_flow", "The workflow reads the transfer."),
            Subscriber::view("transfer_log", "The view lists the transfers."),
            Subscriber::integration("registry_feed", "The feed carries the transfer out."),
        ];
        vec![event]
    }

    fn run(events: &[EventDef]) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = workflows();
        let workflow_registry = WorkflowRegistry::build(&held);
        validate_events(
            events,
            &type_registry,
            &workflow_registry,
            &Path::root().field("events"),
        )
    }

    #[test]
    fn a_complete_event_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn an_event_naming_an_absent_payload_type_reports_its_location() {
        let mut events = sample();
        events[0].payload = TypeId::new("absent_record");
        let report = run(&events);
        let found = report
            .find("events[0].payload")
            .expect("the diagnostic sits on the payload type");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_subscriber_naming_an_absent_workflow_reports_its_location() {
        let mut events = sample();
        events[0].subscribers[0] = Subscriber::workflow("absent_flow", "The workflow reads it.");
        let report = run(&events);
        let found = report
            .find("events[0].subscribers[0].target")
            .expect("the diagnostic sits on the subscriber");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("claim_flow"));
    }

    #[test]
    fn two_events_sharing_a_name_are_caught() {
        let mut events = sample();
        events.push(events[0].clone());
        let report = run(&events);
        let found = report
            .find("events[1].id")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn the_event_reports_the_views_and_the_integrations_it_names() {
        let events = sample();
        let path = Path::root().field("events").index(0);
        let views = events[0].view_refs(&path);
        assert_eq!(views.len(), 1);
        assert_eq!(views[0].1.as_str(), "events[0].subscribers[1].target");
        let integrations = events[0].integration_refs(&path);
        assert_eq!(integrations.len(), 1);
        assert_eq!(integrations[0].0, IntegrationId::new("registry_feed"));
    }

    #[test]
    fn the_registry_reports_a_missing_event_with_the_fix() {
        let events = sample();
        let registry = EventRegistry::build(&events);
        assert!(registry.contains(&EventId::new("transfer_recorded")));
        assert_eq!(registry.len(), 1);
        let diagnostic = registry.missing(
            &EventId::new("absent_event"),
            Path::root()
                .field("actions")
                .index(2)
                .field("emits")
                .index(0)
                .location(),
        );
        assert_eq!(diagnostic.location.as_str(), "actions[2].emits[0]");
        assert!(diagnostic.fix.contains("transfer_recorded"));
    }

    #[test]
    fn the_payload_type_names_the_record() {
        let events = sample();
        assert_eq!(
            events[0].payload_type(),
            TypeExpr::Record(TypeId::new("transfer_record"))
        );
        assert_eq!(events[0].audit, AuditPolicy::Keep);
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let events = sample();
        let bytes = events.encode();
        let back = Vec::<EventDef>::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(events, back);
        let text = serde_json::to_string(&events).expect("the section serialises");
        let parsed: Vec<EventDef> = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(events, parsed);
    }
}
