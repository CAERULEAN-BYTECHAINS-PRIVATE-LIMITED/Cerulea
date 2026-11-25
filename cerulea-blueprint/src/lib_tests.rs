//! Behavioural tests for the whole Blueprint check and the diagnostic types.
//!
//! The land registry fixture validates clean. Each test breaks one cross
//! reference between two sections and asserts the one diagnostic the whole
//! Blueprint check raises for it.

use crate::{
    collections::{PrivacyClass, RetentionAction},
    describe_cycle,
    economics::UnitOfAccount,
    events::Subscriber,
    find_cycle,
    permissions::{Capability, CapabilityEntry},
    test_fixture::land_registry,
    types::{ActionId, CollectionId, EventId, RoleId, StateId, WorkflowId},
    workflows::TimerAction,
    Blueprint, BlueprintMeta, Diagnostic, DiagnosticCode, Location, Path, Report, SchemaVersion,
    VersionStamp, BLUEPRINT_SCHEMA_VERSION,
};
use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

fn report(blueprint: &Blueprint) -> Report {
    match blueprint.validate() {
        Ok(()) => Report::new(),
        Err(report) => report,
    }
}

fn all_at<'a>(report: &'a Report, location: &str) -> Vec<&'a Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == location)
        .collect()
}

fn only(report: &Report) -> &Diagnostic {
    assert_eq!(report.len(), 1, "{report}");
    &report.diagnostics[0]
}

#[test]
fn the_land_registry_validates_clean() {
    let blueprint = land_registry();
    let found = report(&blueprint);
    assert!(found.is_clean(), "{found}");
    assert_eq!(blueprint.validate(), Ok(()));
}

#[test]
fn header_faults_each_raise_a_diagnostic() {
    let mut blueprint = land_registry();
    blueprint.meta.name = String::new();
    blueprint.meta.summary = String::new();
    blueprint.meta.version.revision = 0;
    blueprint.meta.version.schema.major = BLUEPRINT_SCHEMA_VERSION.major + 1;
    let found = report(&blueprint);
    let fixes: Vec<(&str, &str)> = found
        .diagnostics
        .iter()
        .map(|d| (d.location.as_str(), d.fix.as_str()))
        .collect();
    assert_eq!(
        fixes,
        [
            (
                "meta.name",
                "Give the deployment a name in the Studio header panel."
            ),
            (
                "meta.summary",
                "Write one plain sentence stating what the deployment does."
            ),
            (
                "meta.version.revision",
                "Set the revision to 1 for a first publication."
            ),
            (
                "meta.version.schema",
                "Run the Studio migration for this Blueprint before deployment."
            ),
        ]
    );
    let mut long = land_registry();
    long.meta.name = "n".repeat(129);
    let too_long = only(&report(&long)).clone();
    assert_eq!(too_long.code, DiagnosticCode::ValueTooLong);
    assert_eq!(too_long.fix, "Shorten the name to 128 characters or fewer.");
}

#[test]
fn a_collection_naming_an_absent_role_or_collection_is_caught() {
    let mut blueprint = land_registry();
    blueprint.collections[0].privacy = PrivacyClass::Encrypted {
        key_holders: vec![RoleId::new("auditor")],
    };
    let found = report(&blueprint);
    let holder = only(&found);
    assert_eq!(
        holder.location.as_str(),
        "collections[0].privacy.key_holders[0]"
    );
    assert_eq!(
        holder.fix,
        "Point the entry at one of these roles: registrar, clerk."
    );

    let mut moved = land_registry();
    moved.collections[1].retention.on_expiry =
        RetentionAction::MoveTo(CollectionId::new("archive"));
    let found = report(&moved);
    let target = only(&found);
    assert_eq!(
        target.location.as_str(),
        "collections[1].retention.on_expiry"
    );
    assert_eq!(
        target.fix,
        "Point the reference at one of these collections: ledger, parcel."
    );
}

#[test]
fn an_absent_action_named_by_another_section_is_caught() {
    let mut delegated = land_registry();
    delegated.identities.delegations[0].scope[1] = ActionId::new("close_file");
    let found = report(&delegated);
    let scope = only(&found);
    assert_eq!(
        scope.location.as_str(),
        "identities.delegations[0].scope[1]"
    );
    assert_eq!(
        scope.fix,
        "Point the entry at one of these actions: charge_fee, record_parcel."
    );

    let mut permitted = land_registry();
    permitted.permissions.entries.push(CapabilityEntry::grant(
        "clerk",
        "parcel",
        Capability::Invoke(ActionId::new("close_file")),
    ));
    let found = report(&permitted);
    let invoked = all_at(&found, "permissions.entries[4].capability");
    assert!(invoked.iter().any(
        |d| d.problem == "The entry names the action `close_file`, which the Blueprint lacks."
    ));

    let mut carried = land_registry();
    carried.workflows[0].transitions[0].action = Some(ActionId::new("close_file"));
    let found = report(&carried);
    let transition = all_at(&found, "workflows[0].transitions[0].action");
    assert_eq!(transition.len(), 2);
    assert!(transition
        .iter()
        .all(|d| d.code == DiagnosticCode::DanglingReference));
}

#[test]
fn an_action_moving_to_an_absent_state_or_workflow_is_caught() {
    use crate::actions::Effect;
    let mut blueprint = land_registry();
    if let Effect::Transition { to, .. } = &mut blueprint.actions[0].effects[4] {
        *to = StateId::new("archived");
    }
    let found = report(&blueprint);
    let state = only(&found);
    assert_eq!(state.location.as_str(), "actions[0].effects[4].workflow");
    assert_eq!(
        state.problem,
        "The entry names the state `archived`, which `parcel_life` lacks."
    );
    assert_eq!(
        state.fix,
        "Point the entry at one of these states: draft, recorded."
    );

    let mut other = land_registry();
    if let Effect::Transition { workflow, .. } = &mut other.actions[0].effects[4] {
        *workflow = WorkflowId::new("title_life");
    }
    let found = report(&other);
    let workflow = only(&found);
    assert_eq!(workflow.location.as_str(), "actions[0].effects[4].workflow");
    assert_eq!(
        workflow.fix,
        "Point the entry at one of these workflows: parcel_life."
    );
}

#[test]
fn an_absent_event_named_by_an_action_or_a_workflow_is_caught() {
    let mut emitted = land_registry();
    emitted.actions[0].emits[1] = EventId::new("archived");
    let found = report(&emitted);
    let emits = only(&found);
    assert_eq!(emits.location.as_str(), "actions[0].emits[1]");
    assert_eq!(
        emits.fix,
        "Point the entry at one of these events: recorded, stamped."
    );

    let mut timed = land_registry();
    timed.workflows[0].timers[0].on_expiry = TimerAction::Emit(EventId::new("archived"));
    let found = report(&timed);
    let timer = only(&found);
    assert_eq!(timer.location.as_str(), "workflows[0].timers[0].on_expiry");
    assert_eq!(timer.code, DiagnosticCode::DanglingReference);
}

#[test]
fn an_event_delivering_to_an_absent_view_or_integration_is_caught() {
    let mut viewed = land_registry();
    viewed.events[0].subscribers[1] = Subscriber::view("archive", "An absent view.");
    let found = report(&viewed);
    let view = only(&found);
    assert_eq!(view.location.as_str(), "events[0].subscribers[1].target");
    assert_eq!(
        view.fix,
        "Point the entry at one of these views: fees, parcels."
    );

    let mut fed = land_registry();
    fed.events[0].subscribers[2] = Subscriber::integration("tax_feed", "An absent feed.");
    let found = report(&fed);
    let integration = only(&found);
    assert_eq!(
        integration.location.as_str(),
        "events[0].subscribers[2].target"
    );
    assert_eq!(integration.code, DiagnosticCode::DanglingReference);
}

#[test]
fn the_economics_section_joins_the_whole_check() {
    let mut blueprint = land_registry();
    if let Some(economics) = blueprint.economics.as_mut() {
        economics.unit = UnitOfAccount::new("", "Indian rupee", 2);
    }
    let found = report(&blueprint);
    let code = only(&found);
    assert_eq!(code.location.as_str(), "economics.unit.code");
    assert_eq!(code.fix, "Give the unit a short code such as `INR`.");
    blueprint.economics = None;
    assert!(report(&blueprint).is_clean());
}

#[test]
fn find_cycle_reports_a_loop_and_passes_a_tree() {
    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    edges.insert("a".to_string(), vec!["b".to_string(), "c".to_string()]);
    edges.insert("b".to_string(), vec!["d".to_string()]);
    edges.insert("c".to_string(), vec!["d".to_string()]);
    assert_eq!(find_cycle(&edges), None);
    edges.insert("d".to_string(), vec!["e".to_string()]);
    edges.insert("e".to_string(), vec!["b".to_string()]);
    let cycle = find_cycle(&edges).expect("b, d and e close a loop");
    assert_eq!(cycle, ["b", "d", "e", "b"]);
    assert_eq!(describe_cycle(&cycle), "b -> d -> e -> b");
    assert_eq!(find_cycle(&BTreeMap::new()), None);
}

#[test]
fn paths_and_locations_print_the_element_they_name() {
    let path = Path::root()
        .field("actions")
        .index(3)
        .field("effects")
        .key("parcel");
    assert_eq!(path.as_str(), "actions[3].effects['parcel']");
    assert_eq!(format!("{path}"), "actions[3].effects['parcel']");
    let location = path.location();
    assert_eq!(location.as_str(), "actions[3].effects['parcel']");
    assert_eq!(format!("{location}"), "actions[3].effects['parcel']");
    assert_eq!(Path::root().as_str(), "");
    assert_eq!(Path::root().field("types").as_str(), "types");
}

#[test]
fn a_report_collects_merges_finds_and_prints_diagnostics() {
    let first = Diagnostic::new(
        DiagnosticCode::MissingValue,
        Path::root().field("meta").field("name"),
        "The name is empty.",
        "Give the deployment a name.",
    );
    let second = Diagnostic::at(
        DiagnosticCode::SectionEmpty,
        Location("types".to_string()),
        "The section is empty.".to_string(),
        "Add a type.".to_string(),
    );
    assert_eq!(
        format!("{first}"),
        "meta.name: The name is empty. Fix: Give the deployment a name."
    );
    let mut report = Report::new();
    assert!(report.is_empty());
    assert!(report.is_clean());
    report.absorb(Ok(()));
    assert!(report.is_empty());
    report.absorb(Err(first.clone()));
    let mut other = Report::default();
    other.push(second.clone());
    report.merge(other);
    assert_eq!(report.len(), 2);
    assert!(!report.is_empty());
    assert_eq!(report.find("types"), Some(&second));
    assert_eq!(report.find("views"), None);
    assert_eq!(
        format!("{report}"),
        "meta.name: The name is empty. Fix: Give the deployment a name.\ntypes: The section is empty. Fix: Add a type.\n"
    );
    assert_eq!(report.clone().into_result(), Err(report));
    assert_eq!(Report::new().into_result(), Ok(()));
}

#[test]
fn schema_versions_read_forward_within_one_major_number() {
    let reader = SchemaVersion {
        major: 1,
        minor: 2,
        patch: 0,
    };
    let older = SchemaVersion {
        major: 1,
        minor: 1,
        patch: 9,
    };
    let newer = SchemaVersion {
        major: 1,
        minor: 3,
        patch: 0,
    };
    let other = SchemaVersion {
        major: 2,
        minor: 0,
        patch: 0,
    };
    assert!(older.readable_by(&reader));
    assert!(reader.readable_by(&reader));
    assert!(!newer.readable_by(&reader));
    assert!(!other.readable_by(&reader));
    assert_eq!(format!("{older}"), "1.1.9");
    let stamp = VersionStamp::default();
    assert_eq!(stamp.schema, BLUEPRINT_SCHEMA_VERSION);
    assert_eq!(stamp.revision, 1);
    let meta = BlueprintMeta::new("registry", "The registry records.", "Owner");
    assert_eq!(meta.version, stamp);
    let blueprint = Blueprint::new(meta.clone());
    assert_eq!(blueprint.meta, meta);
    assert!(blueprint.types.is_empty());
    assert!(blueprint.economics.is_none());
    assert!(blueprint.interface.is_none());
    assert_eq!(blueprint.validate(), Ok(()));
}
