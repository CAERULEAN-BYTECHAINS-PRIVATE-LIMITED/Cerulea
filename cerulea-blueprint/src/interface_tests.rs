//! Behavioural tests for the interface section.
//!
//! Each test starts from a clean interface, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    actions::{Action, ActionRegistry, Effect, Param},
    collections::{Collection, CollectionRegistry, KeySelector, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    identities::{Identities, RoleDef},
    interface::{
        Binding, BindingNeed, Breakpoint, BreakpointOverride, ComponentGroup, ComponentKind,
        ComponentNode, ConflictResolution, ConflictRule, Interface, LanguageTag, Property, Screen,
        ScreenKind, StringKey, Theme, Token, Translation, VisibilityRule, DESKTOP_BREAKPOINT,
        MAX_STRING_KEY, PHONE_BREAKPOINT, REQUIRED_COLOUR_TOKENS, TABLET_BREAKPOINT,
    },
    permissions::ROW_BINDING,
    types::{
        ActionId, CollectionId, EventId, FieldDef, FieldPath, ParamName, RecordDef, RoleId,
        StateId, TypeDef, TypeExpr, TypeId, TypeRegistry, ViewId, WorkflowId,
    },
    views::{View, ViewRegistry},
    workflows::{Transition, Workflow, WorkflowRegistry, WorkflowState},
    Diagnostic, DiagnosticCode, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
            FieldDef::new("district", TypeExpr::Text, "The district."),
            FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
            FieldDef::new("recorded_at", TypeExpr::Timestamp, "The recording time."),
        ],
        summary: "One land parcel.".to_string(),
    })]
}

fn collections() -> Vec<Collection> {
    let mut parcel = Collection::new("parcel", "parcel", "survey_number", "The parcels.");
    parcel.purpose = PurposeDeclaration::new("The register records land.", &["registration"]);
    vec![parcel]
}

fn identities() -> Identities {
    Identities {
        roles: vec![
            RoleDef::new("registrar", None, "Registrar", "The registrar records."),
            RoleDef::new("holder", None, "Holder", "The holder reads."),
            RoleDef::new("auditor", None, "Auditor", "The auditor checks."),
        ],
        ..Identities::default()
    }
}

fn actions() -> Vec<Action> {
    let mut action = Action::new("record_parcel", "The action records one parcel.");
    action.params = vec![Param::new(
        "area",
        TypeExpr::Decimal { scale: 4 },
        "The area.",
    )];
    action.roles = vec![RoleId::new("registrar")];
    action.effects = vec![Effect::Delete {
        collection: CollectionId::new("parcel"),
        key: KeySelector::Full,
    }];
    vec![action]
}

fn workflows() -> Vec<Workflow> {
    vec![Workflow {
        id: WorkflowId::new("parcel_life"),
        collection: CollectionId::new("parcel"),
        state_field: FieldPath::parse("district"),
        states: vec![
            WorkflowState::passing("draft", "Draft", "The parcel waits."),
            WorkflowState::resting("recorded", "Recorded", "The parcel rests."),
        ],
        initial: StateId::new("draft"),
        transitions: vec![Transition::new("record", "draft", "recorded")],
        timers: Vec::new(),
        deadlines: Vec::new(),
        cycles: crate::workflows::CyclePolicy::default(),
        summary: "The parcel moves from draft to recorded.".to_string(),
    }]
}

fn views() -> Vec<View> {
    vec![View::new("parcels", "parcel", "The view lists parcels.")]
}

fn page(children: Vec<ComponentNode>) -> ComponentNode {
    ComponentNode::new("page", ComponentKind::Page).holding(children)
}

fn district_field() -> ComponentNode {
    let mut field = ComponentNode::bound(
        "district_field",
        ComponentKind::TextInput,
        Binding::Field {
            collection: CollectionId::new("parcel"),
            field: FieldPath::parse("district"),
        },
    );
    field.visibility = vec![
        VisibilityRule::ForRoles(vec![RoleId::new("registrar")]),
        VisibilityRule::InStates {
            workflow: WorkflowId::new("parcel_life"),
            states: vec![StateId::new("draft")],
        },
        VisibilityRule::WhenValue(Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound(ROW_BINDING), "area"),
            Expr::decimal(0, 4),
        )),
    ];
    field.overrides = vec![BreakpointOverride::new(PHONE_BREAKPOINT, true, 4, 1)];
    field.properties = vec![Property::new("variant", "compact")];
    field
}

fn list_screen() -> Screen {
    let mut screen = Screen::new(
        "parcel_list",
        ScreenKind::List,
        "/parcel",
        "screen.parcel.list.title",
    );
    screen.collection = Some(CollectionId::new("parcel"));
    screen.workflow = Some(WorkflowId::new("parcel_life"));
    screen.roles = vec![RoleId::new("registrar"), RoleId::new("holder")];
    screen.components = vec![page(vec![
        ComponentNode::bound(
            "parcel_table",
            ComponentKind::Table,
            Binding::Collection(CollectionId::new("parcel")),
        )
        .labelled("screen.parcel.list.table"),
        ComponentNode::bound(
            "board",
            ComponentKind::Kanban,
            Binding::Workflow(WorkflowId::new("parcel_life")),
        ),
        ComponentNode::bound(
            "history",
            ComponentKind::Timeline,
            Binding::Event(EventId::new("recorded")),
        ),
        ComponentNode::bound(
            "summary_chart",
            ComponentKind::Chart,
            Binding::View(ViewId::new("parcels")),
        ),
        district_field(),
        ComponentNode::bound(
            "status",
            ComponentKind::TransactionStatus,
            Binding::Action(ActionId::new("record_parcel")),
        ),
    ])];
    screen
}

fn create_screen() -> Screen {
    let mut screen = Screen::new(
        "parcel_create",
        ScreenKind::Create,
        "/parcel/new",
        "screen.parcel.create.title",
    );
    screen.action = Some(ActionId::new("record_parcel"));
    screen.roles = vec![RoleId::new("registrar")];
    screen.components = vec![page(vec![ComponentNode::bound(
        "area_input",
        ComponentKind::DecimalInput,
        Binding::Parameter {
            action: ActionId::new("record_parcel"),
            parameter: ParamName::new("area"),
        },
    )])];
    screen
}

fn translated(interface: &mut Interface, language: &str) {
    let entries: Vec<(StringKey, String)> = interface
        .string_keys()
        .into_iter()
        .map(|key| {
            let sentence = format!("The words for {key}.");
            (key, sentence)
        })
        .collect();
    interface
        .translations
        .push(Translation::new(language, entries));
}

fn clean() -> Interface {
    let mut interface = Interface::new();
    interface.screens = vec![list_screen(), create_screen()];
    interface.conflict_rules = vec![ConflictRule {
        collection: CollectionId::new("parcel"),
        resolution: ConflictResolution::LaterTimestamp {
            field: FieldPath::parse("recorded_at"),
        },
        statement: "The later recording stays.".to_string(),
    }];
    translated(&mut interface, "en");
    interface
}

fn run_with(interface: &Interface, events: &BTreeSet<&str>) -> Report {
    let type_defs = types();
    let (type_registry, _) = TypeRegistry::build(&type_defs, &Path::root().field("types"));
    let held = collections();
    let (collection_registry, _) =
        CollectionRegistry::build(&held, &Path::root().field("collections"));
    let action_defs = actions();
    let action_registry = ActionRegistry::build(&action_defs);
    let workflow_defs = workflows();
    let workflow_registry = WorkflowRegistry::build(&workflow_defs);
    let view_defs = views();
    let view_registry = ViewRegistry::build(&view_defs);
    interface.validate(
        &type_registry,
        &collection_registry,
        &identities(),
        &action_registry,
        &workflow_registry,
        &view_registry,
        events,
        &Path::root().field("interface"),
    )
}

fn run(interface: &Interface) -> Report {
    let events: BTreeSet<&str> = ["recorded"].into_iter().collect();
    run_with(interface, &events)
}

fn binding_report(binding: &Binding, events: &BTreeSet<&str>) -> Report {
    let type_defs = types();
    let (type_registry, _) = TypeRegistry::build(&type_defs, &Path::root());
    let held = collections();
    let (collection_registry, _) = CollectionRegistry::build(&held, &Path::root());
    let action_defs = actions();
    let action_registry = ActionRegistry::build(&action_defs);
    let workflow_defs = workflows();
    let workflow_registry = WorkflowRegistry::build(&workflow_defs);
    let view_defs = views();
    let view_registry = ViewRegistry::build(&view_defs);
    binding.validate(
        &type_registry,
        &collection_registry,
        &action_registry,
        &workflow_registry,
        &view_registry,
        events,
        &Path::root().field("binding"),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

fn all_at<'a>(report: &'a Report, location: &str) -> Vec<&'a Diagnostic> {
    report
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == location)
        .collect()
}

const FIELD: &str = "interface.screens[0].components[0].children[4]";

#[test]
fn a_clean_interface_passes_and_answers_lookups() {
    let interface = clean();
    let found = run(&interface);
    assert!(found.is_clean(), "{found}");
    let registrar: Vec<&str> = interface
        .screens_for(&RoleId::new("registrar"))
        .iter()
        .map(|screen| screen.id.as_str())
        .collect();
    assert_eq!(registrar, ["parcel_list", "parcel_create"]);
    let holder: Vec<&str> = interface
        .screens_for(&RoleId::new("holder"))
        .iter()
        .map(|screen| screen.id.as_str())
        .collect();
    assert_eq!(holder, ["parcel_list"]);
    assert!(interface
        .conflict_rule(&CollectionId::new("parcel"))
        .is_some());
    assert!(interface
        .conflict_rule(&CollectionId::new("ghost"))
        .is_none());
    assert_eq!(interface.screens[0].components[0].count(), 7);
    let english = &interface.translations[0];
    assert_eq!(
        english.get(&StringKey::new("screen.parcel.list.title")),
        Some("The words for screen.parcel.list.title.")
    );
    assert_eq!(english.get(&StringKey::new("screen.absent")), None);
    assert!(english.keys().contains("breakpoint.phone"));
}

#[test]
fn a_string_key_rejects_every_malformed_shape() {
    let path = Path::root().field("key");
    assert!(StringKey::new("screen.parcel.list.title")
        .validate(&path)
        .is_ok());
    let empty = StringKey::new("")
        .validate(&path)
        .expect_err("an empty key fails");
    assert_eq!(empty.code, DiagnosticCode::MissingValue);
    assert_eq!(
        empty.fix,
        "Give the string a key such as `screen.parcels.list.title`."
    );
    let long_text = "a".repeat(MAX_STRING_KEY + 1);
    let long = StringKey::new(&long_text)
        .validate(&path)
        .expect_err("a long key fails");
    assert_eq!(long.code, DiagnosticCode::ValueTooLong);
    assert_eq!(long.fix, "Shorten the key to 160 characters or fewer.");
    for shape in ["screen..title", "screen.Title", "screen.9th", "screen.a-b"] {
        let bad = StringKey::new(shape)
            .validate(&path)
            .expect_err("the shape fails");
        assert_eq!(bad.code, DiagnosticCode::InvalidIdentifier);
        assert_eq!(
            bad.fix,
            "Write each part with lower case letters, digits and underscores, joined by full stops."
        );
    }
    let hole = StringKey::new("screen..title")
        .validate(&path)
        .expect_err("the empty part fails");
    assert_eq!(
        hole.problem,
        "The translation key `screen..title` holds the part ``."
    );
    assert_eq!(format!("{}", StringKey::new("a.b")), "a.b");
}

#[test]
fn a_language_tag_accepts_a_language_and_an_optional_region() {
    let path = Path::root().field("language");
    for good in ["en", "en-in", "mal"] {
        assert!(LanguageTag::new(good).validate(&path).is_ok());
    }
    for bad in ["e", "EN", "en-IN", "en-i", "en-in-x", ""] {
        let fault = LanguageTag::new(bad)
            .validate(&path)
            .expect_err("the tag fails");
        assert_eq!(fault.code, DiagnosticCode::InvalidIdentifier);
        assert_eq!(
            fault.fix,
            "Write the tag as a language, with a region after a hyphen where one applies, such as `en` or `en-in`."
        );
    }
    assert_eq!(format!("{}", LanguageTag::new("en-in")), "en-in");
    assert_eq!(LanguageTag::new("en").as_str(), "en");
}

#[test]
fn the_component_set_covers_six_groups_with_fixed_needs() {
    let every = ComponentKind::every();
    assert_eq!(every.len(), 52);
    let labels: BTreeSet<&str> = every.iter().map(|kind| kind.label()).collect();
    assert_eq!(labels.len(), 52);
    let groups: Vec<(ComponentGroup, &str)> = [
        ComponentGroup::Layout,
        ComponentGroup::Data,
        ComponentGroup::Input,
        ComponentGroup::Chain,
        ComponentGroup::Chrome,
        ComponentGroup::Feedback,
    ]
    .into_iter()
    .map(|group| {
        let label = group.label();
        (group, label)
    })
    .collect();
    assert_eq!(
        groups.iter().map(|(_, label)| *label).collect::<Vec<_>>(),
        ["layout", "data", "input", "chain", "chrome", "feedback"]
    );
    for (group, _) in &groups {
        assert!(every.iter().any(|kind| kind.group() == *group));
    }
    assert_eq!(ComponentKind::Table.binds(), BindingNeed::Collection);
    assert_eq!(ComponentKind::Kanban.binds(), BindingNeed::Workflow);
    assert_eq!(ComponentKind::MapInput.binds(), BindingNeed::Field);
    assert_eq!(ComponentKind::CameraCapture.binds(), BindingNeed::Action);
    assert_eq!(ComponentKind::Toast.binds(), BindingNeed::Nothing);
    assert!(ComponentKind::Section.holds_children());
    assert!(ComponentKind::Table.holds_children());
    assert!(ComponentKind::SumInput.holds_children());
    assert!(!ComponentKind::TextInput.holds_children());
    assert!(!ComponentKind::Header.holds_children());
}

#[test]
fn every_type_reaches_one_input_component() {
    let expected = [
        (TypeExpr::Text, ComponentKind::TextInput),
        (TypeExpr::Integer, ComponentKind::NumberInput),
        (TypeExpr::Decimal { scale: 2 }, ComponentKind::DecimalInput),
        (TypeExpr::Bool, ComponentKind::BooleanInput),
        (TypeExpr::Timestamp, ComponentKind::TimestampInput),
        (TypeExpr::Identity, ComponentKind::IdentityInput),
        (TypeExpr::Hash, ComponentKind::HashInput),
        (TypeExpr::Bytes, ComponentKind::BytesInput),
        (
            TypeExpr::Reference(CollectionId::new("parcel")),
            ComponentKind::ReferenceInput,
        ),
        (
            TypeExpr::Enumeration(TypeId::new("tenure")),
            ComponentKind::EnumerationInput,
        ),
        (
            TypeExpr::Sum(TypeId::new("ruling")),
            ComponentKind::SumInput,
        ),
        (TypeExpr::list(TypeExpr::Text), ComponentKind::ListInput),
        (
            TypeExpr::map(TypeExpr::Text, TypeExpr::Text),
            ComponentKind::MapInput,
        ),
        (
            TypeExpr::optional(TypeExpr::Text),
            ComponentKind::OptionalInput,
        ),
        (
            TypeExpr::Record(TypeId::new("parcel")),
            ComponentKind::Section,
        ),
    ];
    for (ty, component) in expected {
        assert_eq!(ComponentKind::for_type(&ty), component);
    }
}

#[test]
fn every_binding_names_its_kind_its_collection_and_a_line() {
    let parcel = CollectionId::new("parcel");
    let bindings = [
        (
            Binding::Collection(parcel.clone()),
            "collection",
            Some("parcel"),
            "collection parcel",
        ),
        (
            Binding::Field {
                collection: parcel.clone(),
                field: FieldPath::parse("holder.district"),
            },
            "field",
            Some("parcel"),
            "field parcel.holder.district",
        ),
        (
            Binding::Parameter {
                action: ActionId::new("record_parcel"),
                parameter: ParamName::new("area"),
            },
            "parameter",
            None,
            "parameter record_parcel.area",
        ),
        (
            Binding::Action(ActionId::new("record_parcel")),
            "action",
            None,
            "action record_parcel",
        ),
        (
            Binding::Workflow(WorkflowId::new("parcel_life")),
            "workflow",
            None,
            "workflow parcel_life",
        ),
        (
            Binding::View(ViewId::new("parcels")),
            "view",
            None,
            "view parcels",
        ),
        (
            Binding::Event(EventId::new("recorded")),
            "event",
            None,
            "event recorded",
        ),
    ];
    for (binding, label, collection, line) in bindings {
        assert_eq!(binding.label(), label);
        assert_eq!(binding.collection().map(|id| id.as_str()), collection);
        assert_eq!(binding.describe(), line);
    }
}

#[test]
fn a_binding_names_elements_the_blueprint_declares() {
    let events: BTreeSet<&str> = ["recorded"].into_iter().collect();
    let none: BTreeSet<&str> = BTreeSet::new();
    let check = |binding: Binding| binding_report(&binding, &events);

    let collection = check(Binding::Collection(CollectionId::new("ghost")));
    assert_eq!(
        at(&collection, "binding").fix,
        "Point the reference at one of these collections: parcel."
    );
    let field_collection = check(Binding::Field {
        collection: CollectionId::new("ghost"),
        field: FieldPath::parse("area"),
    });
    assert_eq!(
        at(&field_collection, "binding").code,
        DiagnosticCode::DanglingReference
    );
    let empty_field = check(Binding::Field {
        collection: CollectionId::new("parcel"),
        field: FieldPath::default(),
    });
    assert_eq!(
        at(&empty_field, "binding.field").code,
        DiagnosticCode::MissingValue
    );
    let missing_field = check(Binding::Field {
        collection: CollectionId::new("parcel"),
        field: FieldPath::parse("owner"),
    });
    assert_eq!(
        at(&missing_field, "binding.field").fix,
        "Bind to a field declared on the record type of `parcel`."
    );
    let action = check(Binding::Parameter {
        action: ActionId::new("ghost"),
        parameter: ParamName::new("area"),
    });
    assert_eq!(
        at(&action, "binding").fix,
        "Point the entry at one of these actions: record_parcel."
    );
    let parameter = check(Binding::Parameter {
        action: ActionId::new("record_parcel"),
        parameter: ParamName::new("width"),
    });
    assert_eq!(
        at(&parameter, "binding.parameter").fix,
        "Bind to a parameter the action `record_parcel` declares."
    );
    assert_eq!(
        at(&check(Binding::Action(ActionId::new("ghost"))), "binding").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(
            &check(Binding::Workflow(WorkflowId::new("ghost"))),
            "binding"
        )
        .fix,
        "Point the entry at one of these workflows: parcel_life."
    );
    assert_eq!(
        at(&check(Binding::View(ViewId::new("ghost"))), "binding").fix,
        "Point the entry at one of these views: parcels."
    );
    assert_eq!(
        at(&check(Binding::Event(EventId::new("ghost"))), "binding").fix,
        "Bind to one of these events: recorded."
    );
    let no_events = binding_report(&Binding::Event(EventId::new("ghost")), &none);
    assert_eq!(
        at(&no_events, "binding").fix,
        "Declare an event named `ghost` in the events section."
    );
    assert!(check(Binding::Event(EventId::new("recorded"))).is_clean());
}

#[test]
fn the_small_types_name_themselves() {
    let rules = [
        (VisibilityRule::ForRoles(Vec::new()), "role"),
        (
            VisibilityRule::InStates {
                workflow: WorkflowId::new("parcel_life"),
                states: Vec::new(),
            },
            "workflow state",
        ),
        (VisibilityRule::WhenValue(Expr::boolean(true)), "data value"),
    ];
    for (rule, label) in rules {
        assert_eq!(rule.label(), label);
    }
    let kinds = [
        (ScreenKind::List, "list", false),
        (ScreenKind::Detail, "detail", false),
        (ScreenKind::Create, "create", true),
        (ScreenKind::Edit, "edit", true),
        (ScreenKind::Approval, "approval", true),
        (ScreenKind::Custom, "custom", false),
    ];
    for (kind, label, writes) in kinds {
        assert_eq!(kind.label(), label);
        assert_eq!(kind.writes(), writes);
    }
    let resolutions = [
        (ConflictResolution::KeepStored, "keep_stored"),
        (ConflictResolution::TakeCaptured, "take_captured"),
        (
            ConflictResolution::LaterTimestamp {
                field: FieldPath::parse("recorded_at"),
            },
            "later_timestamp",
        ),
        (
            ConflictResolution::LargerValue {
                field: FieldPath::parse("area"),
            },
            "larger_value",
        ),
        (
            ConflictResolution::MergeFields { fields: Vec::new() },
            "merge_fields",
        ),
        (ConflictResolution::AskAPerson, "ask_a_person"),
    ];
    for (resolution, label) in resolutions {
        assert_eq!(resolution.label(), label);
    }
    let points = Breakpoint::standard();
    let standard: Vec<(&str, u16, u8)> = points
        .iter()
        .map(|point| (point.id.as_str(), point.min_width, point.columns))
        .collect();
    assert_eq!(standard.len(), 3);
    assert_eq!(standard[0].0, PHONE_BREAKPOINT);
    assert_eq!(standard[1].0, TABLET_BREAKPOINT);
    assert_eq!(standard[2], (DESKTOP_BREAKPOINT, 1024, 12));
    let theme = Theme::cerulea();
    assert_eq!(theme.light("brand_primary"), Some("#004AAD"));
    assert_eq!(theme.light("absent"), None);
    assert!(theme.validate(&Path::root()).is_clean());
    let screen = list_screen();
    assert!(screen.reached_by(&RoleId::new("holder")));
    assert!(!screen.reached_by(&RoleId::new("auditor")));
    let keys: Vec<String> = screen
        .string_keys()
        .iter()
        .map(|key| key.as_str().to_string())
        .collect();
    assert_eq!(
        keys,
        ["screen.parcel.list.title", "screen.parcel.list.table"]
    );
}

#[test]
fn theme_faults_each_raise_a_diagnostic() {
    let mut theme = Theme::cerulea();
    theme
        .colour_light
        .push(Token::new("brand_primary", "#004AAD"));
    theme.colour_light.push(Token::new("Glow", ""));
    theme
        .colour_dark
        .retain(|token| token.name.as_str() != "focus_ring");
    theme.colour_dark[0].value = "blue".to_string();
    theme.colour_dark[1].value = "#12345G".to_string();
    theme.type_scale[0].value = String::new();
    let found = theme.validate(&Path::root().field("theme"));
    let base = REQUIRED_COLOUR_TOKENS.len();
    assert_eq!(
        at(&found, &format!("theme.colour_light[{base}].name")).fix,
        "Remove one of the two `brand_primary` tokens."
    );
    let glow = format!("theme.colour_light[{}]", base + 1);
    assert_eq!(
        at(&found, &format!("{glow}.name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let glow_value: Vec<DiagnosticCode> = all_at(&found, &format!("{glow}.value"))
        .iter()
        .map(|d| d.code)
        .collect();
    assert_eq!(
        glow_value,
        [DiagnosticCode::MissingValue, DiagnosticCode::TypeMismatch]
    );
    let word = at(&found, "theme.colour_dark[0].value");
    assert_eq!(
        word.problem,
        "The colour token `brand_primary` carries `blue`."
    );
    assert_eq!(
        word.fix,
        "Write the colour as six hexadecimal digits after a hash, such as `#004AAD`."
    );
    assert_eq!(
        at(&found, "theme.colour_dark[1].value").code,
        DiagnosticCode::TypeMismatch
    );
    assert_eq!(
        at(&found, "theme.colour_dark").fix,
        "Add a token named `focus_ring` so every screen reads a value."
    );
    assert_eq!(
        at(&found, "theme.type_scale[0].value").fix,
        "Give the token a value."
    );
}

#[test]
fn language_and_breakpoint_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    interface.default_language = LanguageTag::new("EN");
    interface
        .breakpoints
        .push(Breakpoint::new("Wide", 2000, 16, "breakpoint.Wide"));
    interface
        .breakpoints
        .push(Breakpoint::new(PHONE_BREAKPOINT, 0, 0, "breakpoint.phone"));
    let found = run(&interface);
    assert_eq!(
        at(&found, "interface.default_language").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "interface.breakpoints[3].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "interface.breakpoints[3].label_key").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "interface.breakpoints[4].id").fix,
        "Rename one of the two `phone` breakpoints."
    );
    assert_eq!(
        at(&found, "interface.breakpoints[4].columns").fix,
        "Set the column count above zero."
    );

    let mut bare = clean();
    bare.breakpoints.clear();
    let found = run(&bare);
    assert_eq!(
        at(&found, "interface.breakpoints").fix,
        "Declare the phone, tablet and desktop breakpoints so every screen answers at every width."
    );
    let override_path = format!("{FIELD}.overrides[0].breakpoint");
    assert_eq!(
        at(&found, &override_path).fix,
        "Declare a breakpoint named `phone`."
    );
}

#[test]
fn screen_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    let mut stray = Screen::new("parcel_list", ScreenKind::Edit, "parcel", "Screen.Title");
    stray.roles = vec![RoleId::new("Clerk"), RoleId::new("clerk")];
    stray.collection = Some(CollectionId::new("ghost"));
    stray.workflow = Some(WorkflowId::new("ghost_flow"));
    interface.screens.push(stray);
    let mut copy = Screen::new("Copy", ScreenKind::Approval, "/parcel", "screen.copy.title");
    copy.action = Some(ActionId::new("ghost_action"));
    interface.screens.push(copy);
    let found = run(&interface);
    let stray_path = "interface.screens[2]";
    assert_eq!(
        at(&found, &format!("{stray_path}.id")).fix,
        "Rename one of the two `parcel_list` screens."
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.title_key")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.route")).fix,
        "Write the address with a leading slash, such as `/parcels`."
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.roles[0]")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.roles[1]")).fix,
        "Point the entry at one of these roles: registrar, holder, auditor."
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.collection")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.workflow")).fix,
        "Point the entry at one of these workflows: parcel_life."
    );
    let writes = at(&found, &format!("{stray_path}.action"));
    assert_eq!(
        writes.problem,
        "The edit screen `parcel_list` writes state and names no action."
    );
    assert_eq!(
        writes.fix,
        "Name the action the screen runs, so the write passes through the permission matrix."
    );
    assert_eq!(
        at(&found, &format!("{stray_path}.components")).fix,
        "Place at least one component on the screen."
    );
    let copy_path = "interface.screens[3]";
    assert_eq!(
        at(&found, &format!("{copy_path}.id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let shared = at(&found, &format!("{copy_path}.route"));
    assert_eq!(
        shared.problem,
        "The screens `parcel_list` and `Copy` answer on `/parcel`."
    );
    assert_eq!(shared.fix, "Give each screen its own address.");
    assert_eq!(
        at(&found, &format!("{copy_path}.roles")).code,
        DiagnosticCode::SectionEmpty
    );
    assert_eq!(
        at(&found, &format!("{copy_path}.action")).fix,
        "Point the entry at one of these actions: record_parcel."
    );
}

#[test]
fn translation_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    translated(&mut interface, "en");
    interface
        .translations
        .push(Translation::new("Hindi", Vec::new()));
    let found = run(&interface);
    assert_eq!(
        at(&found, "interface.translations[1].language").fix,
        "Keep one file for each language."
    );
    assert_eq!(
        at(&found, "interface.translations[2].language").code,
        DiagnosticCode::InvalidIdentifier
    );
    let gaps = all_at(&found, "interface.translations[2].entries");
    assert_eq!(gaps.len(), interface.string_keys().len());
    assert_eq!(
        gaps[0].fix,
        "Add a sentence for `breakpoint.desktop` so every reader sees words."
    );

    let mut other = clean();
    other.default_language = LanguageTag::new("ml");
    let found = run(&other);
    let opens = at(&found, "interface.translations");
    assert_eq!(
        opens.problem,
        "The interface opens in `ml` and carries no file for that language."
    );
    assert_eq!(
        opens.fix,
        "Add the `ml` translation file, or open in a language a file carries."
    );
}

#[test]
fn conflict_rule_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    interface.conflict_rules.push(ConflictRule {
        collection: CollectionId::new("parcel"),
        resolution: ConflictResolution::LargerValue {
            field: FieldPath::parse("width"),
        },
        statement: String::new(),
    });
    interface.conflict_rules.push(ConflictRule {
        collection: CollectionId::new("ghost"),
        resolution: ConflictResolution::KeepStored,
        statement: "The stored value stays.".to_string(),
    });
    let found = run(&interface);
    assert_eq!(
        at(&found, "interface.conflict_rules[1].collection").fix,
        "Keep one rule for each collection."
    );
    assert_eq!(
        at(&found, "interface.conflict_rules[1].statement").fix,
        "Write one plain sentence stating why the rule reads this way."
    );
    assert_eq!(
        at(&found, "interface.conflict_rules[1].resolution").fix,
        "Read a field declared on the record type of `parcel`."
    );
    assert_eq!(
        at(&found, "interface.conflict_rules[2].collection").code,
        DiagnosticCode::DanglingReference
    );

    let mut merged = clean();
    merged.conflict_rules[0].resolution = ConflictResolution::MergeFields {
        fields: vec![FieldPath::default(), FieldPath::parse("area")],
    };
    let found = run(&merged);
    assert_eq!(
        at(&found, "interface.conflict_rules[0].resolution").code,
        DiagnosticCode::MissingValue
    );

    let mut wrong_type = clean();
    wrong_type.conflict_rules[0].resolution = ConflictResolution::LaterTimestamp {
        field: FieldPath::parse("area"),
    };
    let found = run(&wrong_type);
    let timestamp = at(&found, "interface.conflict_rules[0].resolution");
    assert_eq!(timestamp.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        timestamp.problem,
        "The later timestamp rule reads `area`, which carries a decimal at scale 4."
    );
    assert_eq!(
        timestamp.fix,
        "Read a timestamp field, or choose another rule."
    );
}

#[test]
fn node_placement_and_binding_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    let children = &mut interface.screens[0].components[0].children;
    children.push(ComponentNode::new("parcel_table", ComponentKind::Table));
    children.push(ComponentNode::new("Bad", ComponentKind::Toast).labelled("Bad.Key"));
    children.push(
        ComponentNode::bound(
            "boxed",
            ComponentKind::TextInput,
            Binding::Field {
                collection: CollectionId::new("parcel"),
                field: FieldPath::parse("district"),
            },
        )
        .holding(vec![ComponentNode::new("inner", ComponentKind::Toast)]),
    );
    children.push(ComponentNode::new(
        "form_status",
        ComponentKind::Confirmation,
    ));
    let found = run(&interface);
    let base = "interface.screens[0].components[0].children";
    let repeated = all_at(&found, &format!("{base}[6].id"));
    assert_eq!(repeated.len(), 1);
    assert_eq!(
        repeated[0].fix,
        "Rename one of the two `parcel_table` placements."
    );
    let unbound = at(&found, &format!("{base}[6].binding"));
    assert_eq!(unbound.code, DiagnosticCode::MissingValue);
    assert_eq!(
        unbound.problem,
        "The table component `parcel_table` reads nothing."
    );
    assert_eq!(
        unbound.fix,
        "Bind the component to the collection, view or event it shows."
    );
    assert_eq!(
        at(&found, &format!("{base}[7].id")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &format!("{base}[7].label_key")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let nested = at(&found, &format!("{base}[8].children"));
    assert_eq!(nested.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        nested.problem,
        "The text_input component `boxed` holds components inside itself."
    );
    assert!(found.find(&format!("{base}[8].children[0].id")).is_none());
    assert_eq!(
        at(&found, &format!("{base}[9].binding")).fix,
        "Bind the component to the action it shows."
    );

    let mut wrong_kinds = clean();
    let children = &mut wrong_kinds.screens[0].components[0].children;
    children[1].binding = Some(Binding::Collection(CollectionId::new("parcel")));
    children[4].binding = Some(Binding::Action(ActionId::new("record_parcel")));
    children[5].binding = Some(Binding::View(ViewId::new("parcels")));
    let found = run(&wrong_kinds);
    assert_eq!(
        at(&found, &format!("{base}[1].binding")).fix,
        "Bind the component to the workflow it shows."
    );
    assert_eq!(
        at(&found, &format!("{base}[4].binding")).fix,
        "Bind the component to the field or action parameter it shows."
    );
    assert_eq!(
        at(&found, &format!("{base}[5].binding")).problem,
        "The transaction_status component `status` binds to a view."
    );
}

#[test]
fn visibility_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    interface.screens[0].components[0].children[4].visibility = vec![
        VisibilityRule::ForRoles(Vec::new()),
        VisibilityRule::ForRoles(vec![
            RoleId::new("Bad"),
            RoleId::new("clerk"),
            RoleId::new("auditor"),
        ]),
        VisibilityRule::InStates {
            workflow: WorkflowId::new("ghost_flow"),
            states: vec![StateId::new("draft")],
        },
        VisibilityRule::InStates {
            workflow: WorkflowId::new("parcel_life"),
            states: Vec::new(),
        },
        VisibilityRule::WhenValue(Expr::bound("absent")),
        VisibilityRule::WhenValue(Expr::text("shown")),
    ];
    let found = run(&interface);
    let rules = format!("{FIELD}.visibility");
    assert_eq!(
        at(&found, &format!("{rules}[0]")).fix,
        "Name the roles that see the component."
    );
    assert_eq!(
        at(&found, &format!("{rules}[1][0]")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &format!("{rules}[1][1]")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &format!("{rules}[1][2]")).fix,
        "Add `auditor` to the roles of the screen `parcel_list`, or name a role the screen already carries."
    );
    assert_eq!(
        at(&found, &format!("{rules}[2]")).fix,
        "Point the entry at one of these workflows: parcel_life."
    );
    assert_eq!(
        at(&found, &format!("{rules}[3]")).fix,
        "Name the workflow states that show the component."
    );
    assert_eq!(
        at(&found, &format!("{rules}[4]")).code,
        DiagnosticCode::DanglingReference
    );
    let text = at(&found, &format!("{rules}[5]"));
    assert_eq!(
        text.problem,
        "The data rule on `district_field` carries text."
    );
    assert_eq!(text.fix, "Give the rule a truth value.");

    let mut detached = clean();
    detached.screens[0].collection = None;
    let found = run(&detached);
    assert_eq!(
        at(&found, &format!("{rules}[2].left.base")).code,
        DiagnosticCode::DanglingReference
    );
}

#[test]
fn override_and_property_faults_each_raise_a_diagnostic() {
    let mut interface = clean();
    let field = &mut interface.screens[0].components[0].children[4];
    field.overrides = vec![
        BreakpointOverride::new(PHONE_BREAKPOINT, true, 4, 1),
        BreakpointOverride::new(PHONE_BREAKPOINT, true, 0, 2),
        BreakpointOverride::new("watch", false, 0, 3),
        BreakpointOverride::new("Wide", false, 2, 4),
    ];
    field.properties = vec![
        Property::new("Bad", "value"),
        Property::new("variant", ""),
        Property::new("variant", "compact"),
    ];
    let found = run(&interface);
    let overrides = format!("{FIELD}.overrides");
    assert_eq!(
        at(&found, &format!("{overrides}[1].breakpoint")).fix,
        "Keep one override for `phone`."
    );
    let zero = at(&found, &format!("{overrides}[1].span"));
    assert_eq!(zero.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        zero.fix,
        "Set the column span above zero, or hide the component at this width."
    );
    assert_eq!(
        at(&found, &format!("{overrides}[2].breakpoint")).fix,
        "Point the override at one of these breakpoints: desktop, phone, tablet."
    );
    assert!(found.find(&format!("{overrides}[2].span")).is_none());
    assert_eq!(
        at(&found, &format!("{overrides}[3].breakpoint")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let properties = format!("{FIELD}.properties");
    assert_eq!(
        at(&found, &format!("{properties}[0].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &format!("{properties}[1].value")).fix,
        "Give the property a value, or remove it."
    );
    assert_eq!(
        at(&found, &format!("{properties}[2].name")).fix,
        "Keep one `variant` property."
    );
}
