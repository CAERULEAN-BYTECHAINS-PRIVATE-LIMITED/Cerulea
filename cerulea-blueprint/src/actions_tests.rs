//! Behavioural tests for actions and Access Manifest derivation.
//!
//! Each test builds real actions, runs the real validator or the real
//! manifest derivation, and asserts the concrete result. Every reject path
//! asserts the diagnostic class, the location and the fix sentence.

use crate::{
    actions::{
        no_blocks, validate_actions, AccessEntry, AccessMode, Action, ActionRegistry, BlockOutput,
        Effect, Param, TransactionClass,
    },
    collections::{Collection, CollectionRegistry, KeyRange, KeySelector, PurposeDeclaration},
    compose::{Block, BlockLibrary},
    expression::{BinaryOp, Builtin, Expr},
    fold::{FoldExpr, FoldSource},
    identities::{Identities, RoleDef},
    rules::{Precondition, RuleInput, Rules},
    types::{
        ActionId, BlockId, CollectionId, EventId, FieldDef, FieldName, FieldPath, Identity,
        ParamName, RecordDef, RoleId, RuleId, StateId, Timestamp, TypeDef, TypeExpr, TypeId,
        TypeRegistry, Value, WorkflowId,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{collections::BTreeMap, string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
            ],
            summary: "One land parcel.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("ledger"),
            fields: vec![
                FieldDef::new("entry", TypeExpr::Integer, "The entry number."),
                FieldDef::new("note", TypeExpr::Text, "The note."),
            ],
            summary: "One ledger line.".to_string(),
        }),
    ]
}

fn collections() -> Vec<Collection> {
    let purpose = PurposeDeclaration::new("The register records land.", &["registration"]);
    let mut parcel = Collection::new("parcel", "parcel", "survey_number", "The parcels.");
    parcel.purpose = purpose.clone();
    let mut ledger = Collection::new("ledger", "ledger", "entry", "The ledger.");
    ledger.purpose = purpose.clone();
    let mut orphan = Collection::new("orphan", "ghost", "id", "A collection over no type.");
    orphan.purpose = purpose;
    vec![parcel, ledger, orphan]
}

fn identities() -> Identities {
    Identities {
        roles: vec![RoleDef::new(
            "registrar",
            None,
            "Registrar",
            "The registrar records parcels.",
        )],
        ..Identities::default()
    }
}

fn rules() -> Rules {
    Rules {
        invariants: Vec::new(),
        preconditions: vec![Precondition {
            id: RuleId::new("area_fits"),
            inputs: vec![RuleInput::new("area", TypeExpr::Decimal { scale: 4 })],
            holds: Expr::binary(BinaryOp::Greater, Expr::bound("area"), Expr::decimal(0, 4)),
            problem: "The parcel covers zero land.".to_string(),
            fix: "Enter an area above zero.".to_string(),
        }],
    }
}

fn blocks() -> Vec<Block> {
    let mut stamp = Block::new("stamp", "The block writes one ledger line.");
    stamp.inputs = vec![Param::new("note", TypeExpr::Text, "The note.")];
    stamp.outputs = vec![BlockOutput::new(
        "stamped",
        TypeExpr::Text,
        Expr::bound("note"),
    )];
    stamp.effects = vec![Effect::Insert {
        collection: CollectionId::new("ledger"),
        key: KeySelector::Exact(Expr::integer(7)),
        record: Expr::MakeRecord {
            type_id: TypeId::new("ledger"),
            fields: vec![
                (FieldName::new("entry"), Expr::integer(7)),
                (FieldName::new("note"), Expr::bound("note")),
            ],
        },
    }];
    let mut looping = Block::new("looping", "The block runs itself.");
    looping.effects = vec![Effect::RunBlock {
        block: BlockId::new("looping"),
        arguments: Vec::new(),
    }];
    vec![stamp, looping]
}

fn key() -> KeySelector {
    KeySelector::Exact(Expr::bound("survey_number"))
}

fn clean() -> Action {
    let mut action = Action::new("record_parcel", "The action records one parcel.");
    action.params = vec![
        Param::new("survey_number", TypeExpr::Text, "The parcel."),
        Param::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
    ];
    action.preconditions = vec![RuleId::new("area_fits")];
    action.roles = vec![RoleId::new("registrar")];
    action.emits = vec![EventId::new("recorded")];
    action.class = TransactionClass::Structural;
    action.effects = vec![
        Effect::Read {
            collection: CollectionId::new("parcel"),
            key: key(),
            binding: ParamName::new("existing"),
            element: TypeExpr::Record(TypeId::new("parcel")),
        },
        Effect::Insert {
            collection: CollectionId::new("parcel"),
            key: key(),
            record: Expr::MakeRecord {
                type_id: TypeId::new("parcel"),
                fields: vec![
                    (
                        FieldName::new("survey_number"),
                        Expr::bound("survey_number"),
                    ),
                    (
                        FieldName::new("holder"),
                        Expr::Call {
                            function: Builtin::Caller,
                            args: Vec::new(),
                        },
                    ),
                    (FieldName::new("area"), Expr::bound("area")),
                ],
            },
        },
        Effect::Update {
            collection: CollectionId::new("parcel"),
            key: key(),
            field: FieldPath::parse("area"),
            value: Expr::bound("area"),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: Expr::bound("survey_number"),
        },
        Effect::Transition {
            workflow: WorkflowId::new("parcel_life"),
            collection: CollectionId::new("parcel"),
            key: key(),
            to: StateId::new("recorded"),
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![(ParamName::new("note"), Expr::bound("survey_number"))],
        },
        Effect::Emit {
            event: EventId::new("stamped"),
            payload: Expr::bound("stamped"),
        },
        Effect::Delete {
            collection: CollectionId::new("ledger"),
            key: KeySelector::Prefix(Expr::integer(1)),
        },
    ];
    action.compensation = vec![Effect::Delete {
        collection: CollectionId::new("parcel"),
        key: key(),
    }];
    action
}

fn report_with(action: &Action, rules: &Rules) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root().field("collections"));
    let library_blocks = blocks();
    let library = BlockLibrary::build(&library_blocks);
    action.validate(
        &registry,
        &collections,
        &identities(),
        rules,
        &library,
        &Path::root().field("actions").index(0),
    )
}

fn report(action: &Action) -> Report {
    report_with(action, &rules())
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

fn arguments() -> BTreeMap<ParamName, Value> {
    let mut held = BTreeMap::new();
    held.insert(
        ParamName::new("survey_number"),
        Value::Text("KL-1".to_string()),
    );
    held.insert(
        ParamName::new("area"),
        Value::Decimal(crate::types::Decimal::new(5, 4)),
    );
    held
}

fn exact(value: Value) -> KeyRange {
    KeyRange::Exact(value.canonical_bytes())
}

#[test]
fn transaction_classes_carry_their_thresholds_and_labels() {
    let classes = [
        (TransactionClass::Routine, 51, false, "routine"),
        (TransactionClass::Financial, 67, false, "financial"),
        (TransactionClass::Structural, 80, false, "structural"),
        (TransactionClass::Constitutional, 90, true, "constitutional"),
    ];
    for (class, threshold, delay, label) in classes {
        assert_eq!(class.threshold_percent(), threshold);
        assert_eq!(class.holds_delay(), delay);
        assert_eq!(class.label(), label);
    }
}

fn every_effect() -> Vec<Effect> {
    let n = || Expr::bound("n");
    let c = || CollectionId::new("parcel");
    vec![
        Effect::Insert {
            collection: c(),
            key: KeySelector::Exact(n()),
            record: n(),
        },
        Effect::Update {
            collection: c(),
            key: KeySelector::Prefix(n()),
            field: FieldPath::parse("area"),
            value: n(),
        },
        Effect::Delete {
            collection: c(),
            key: KeySelector::Full,
        },
        Effect::Read {
            collection: c(),
            key: KeySelector::Exact(n()),
            binding: ParamName::new("found"),
            element: TypeExpr::Record(TypeId::new("parcel")),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: n(),
        },
        Effect::Transition {
            workflow: WorkflowId::new("parcel_life"),
            collection: c(),
            key: KeySelector::Exact(n()),
            to: StateId::new("recorded"),
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![
                (ParamName::new("first"), n()),
                (ParamName::new("second"), n()),
            ],
        },
    ]
}

#[test]
fn every_effect_names_its_label_and_its_access() {
    let effects = every_effect();
    let labels: Vec<&str> = effects.iter().map(|effect| effect.label()).collect();
    assert_eq!(
        labels,
        [
            "insert",
            "update",
            "delete",
            "read",
            "emit",
            "transition",
            "run block"
        ]
    );
    let modes: Vec<Option<bool>> = effects
        .iter()
        .map(|effect| {
            effect
                .access()
                .map(|(_, _, mode)| matches!(mode, AccessMode::Write))
        })
        .collect();
    assert_eq!(
        modes,
        [
            Some(true),
            Some(true),
            Some(true),
            Some(false),
            None,
            Some(true),
            None
        ]
    );
}

#[test]
fn substitution_reaches_every_expression_an_effect_carries() {
    let mut replacements = BTreeMap::new();
    replacements.insert(ParamName::new("n"), Expr::integer(5));
    let mut before = 0usize;
    let mut after_bound = 0usize;
    let mut after_literal = 0usize;
    for effect in every_effect() {
        effect.walk_expressions(&mut |expr| {
            if expr == &Expr::bound("n") {
                before += 1;
            }
        });
        let replaced = effect.substitute(&replacements);
        assert_eq!(replaced.label(), effect.label());
        replaced.walk_expressions(&mut |expr| {
            if matches!(expr, Expr::Bound(_)) {
                after_bound += 1;
            }
            if expr == &Expr::integer(5) {
                after_literal += 1;
            }
        });
    }
    assert_eq!(before, 9);
    assert_eq!(after_bound, 0);
    assert_eq!(after_literal, 9);
}

#[test]
fn an_action_lists_its_parameters_events_transitions_and_blocks() {
    let action = clean();
    let path = Path::root().field("actions").index(0);
    assert_eq!(
        action
            .param(&ParamName::new("area"))
            .map(|param| param.ty.clone()),
        Some(TypeExpr::Decimal { scale: 4 })
    );
    assert!(action.param(&ParamName::new("absent")).is_none());
    let refs = action.event_refs(&path);
    let events: Vec<(&str, &str)> = refs
        .iter()
        .map(|(event, location)| (event.as_str(), location.as_str()))
        .collect();
    assert_eq!(
        events,
        [
            ("recorded", "actions[0].emits[0]"),
            ("recorded", "actions[0].effects[3].event"),
            ("stamped", "actions[0].effects[6].event"),
        ]
    );
    let transitions = action.transition_refs(&path);
    assert_eq!(transitions.len(), 1);
    assert_eq!(transitions[0].0, WorkflowId::new("parcel_life"));
    assert_eq!(transitions[0].1, StateId::new("recorded"));
    assert_eq!(transitions[0].2.as_str(), "actions[0].effects[4].workflow");
    let blocks = action.block_refs(&path);
    assert_eq!(
        blocks,
        [(
            BlockId::new("stamp"),
            Location("actions[0].effects[5].block".to_string())
        )]
    );
}

#[test]
fn a_clean_action_validates() {
    let found = report(&clean());
    assert!(found.is_clean(), "{found}");
}

#[test]
fn the_manifest_names_every_read_and_every_write_before_execution() {
    let action = clean();
    let library_blocks = blocks();
    let library = BlockLibrary::build(&library_blocks);
    let manifest = action
        .derive_manifest(
            &arguments(),
            Identity([1u8; 32]),
            Timestamp::from_millis(10),
            &library,
            &Path::root().field("actions").index(0),
        )
        .expect("the manifest derives");
    let parcel = CollectionId::new("parcel");
    let ledger = CollectionId::new("ledger");
    let parcel_key = exact(Value::Text("KL-1".to_string()));
    assert_eq!(manifest.action, ActionId::new("record_parcel"));
    assert_eq!(manifest.class, TransactionClass::Structural);
    assert_eq!(
        manifest.reads,
        [AccessEntry {
            collection: parcel.clone(),
            range: parcel_key.clone(),
        }]
    );
    assert_eq!(
        manifest.writes,
        [
            AccessEntry {
                collection: ledger.clone(),
                range: exact(Value::Integer(7)),
            },
            AccessEntry {
                collection: ledger.clone(),
                range: KeyRange::Prefix(Value::Integer(1).canonical_bytes()),
            },
            AccessEntry {
                collection: parcel.clone(),
                range: parcel_key.clone(),
            },
        ]
    );
    assert!(manifest.admits_read(&parcel, &parcel_key));
    assert!(manifest.admits_read(&ledger, &exact(Value::Integer(7))));
    assert!(manifest.admits_write(&parcel, &parcel_key));
    assert!(!manifest.admits_write(&parcel, &exact(Value::Text("KL-2".to_string()))));
    assert!(!manifest.admits_read(&CollectionId::new("orphan"), &KeyRange::Full));
    assert_eq!(manifest.collections(), [ledger, parcel]);
}

#[test]
fn a_fold_over_a_collection_adds_a_read_to_the_manifest() {
    let mut action = Action::new("count_lines", "The action counts ledger lines.");
    action.effects = vec![Effect::Emit {
        event: EventId::new("counted"),
        payload: Expr::Fold(alloc::boxed::Box::new(FoldExpr::over_source(
            FoldSource::collection(
                "ledger",
                KeySelector::Full,
                TypeExpr::Record(TypeId::new("ledger")),
            ),
            "line",
            "count",
            Expr::integer(0),
            Expr::binary(BinaryOp::Add, Expr::bound("count"), Expr::integer(1)),
            100,
        ))),
    }];
    let manifest = action
        .derive_manifest(
            &BTreeMap::new(),
            Identity::default(),
            Timestamp::default(),
            no_blocks().as_ref(),
            &Path::root(),
        )
        .expect("the manifest derives");
    assert_eq!(
        manifest.reads,
        [AccessEntry {
            collection: CollectionId::new("ledger"),
            range: KeyRange::Full,
        }]
    );
    assert!(manifest.writes.is_empty());
}

#[test]
fn manifest_derivation_reports_every_fault() {
    let library_blocks = blocks();
    let library = BlockLibrary::build(&library_blocks);
    let derive = |action: &Action, arguments: &BTreeMap<ParamName, Value>| {
        action
            .derive_manifest(
                arguments,
                Identity::default(),
                Timestamp::default(),
                &library,
                &Path::root().field("actions").index(0),
            )
            .expect_err("the derivation stops")
    };

    let missing = derive(&clean(), &BTreeMap::new());
    assert_eq!(missing.code, DiagnosticCode::MissingValue);
    assert_eq!(missing.location.as_str(), "actions[0].params");
    assert_eq!(
        missing.problem,
        "The call to `record_parcel` leaves out the parameter `survey_number`."
    );
    assert_eq!(missing.fix, "Supply a value for `survey_number`.");

    let mut ghost = Action::new("ghost_block", "The action runs an absent block.");
    ghost.effects = vec![Effect::RunBlock {
        block: BlockId::new("absent"),
        arguments: Vec::new(),
    }];
    let absent = derive(&ghost, &BTreeMap::new());
    assert_eq!(absent.code, DiagnosticCode::DanglingReference);
    assert_eq!(absent.location.as_str(), "actions[0].effects[0].block");
    assert_eq!(
        absent.fix,
        "Declare a block named `absent` in the composition library."
    );

    let mut short = Action::new("short_call", "The action leaves out an input.");
    short.effects = vec![Effect::RunBlock {
        block: BlockId::new("stamp"),
        arguments: Vec::new(),
    }];
    let input = derive(&short, &BTreeMap::new());
    assert_eq!(input.code, DiagnosticCode::MissingValue);
    assert_eq!(input.location.as_str(), "actions[0].effects[0].arguments");
    assert_eq!(input.fix, "Supply a value for `note`.");

    let mut deep = Action::new("deep_call", "The action runs a block that runs itself.");
    deep.effects = vec![Effect::RunBlock {
        block: BlockId::new("looping"),
        arguments: Vec::new(),
    }];
    let nested = derive(&deep, &BTreeMap::new());
    assert_eq!(nested.code, DiagnosticCode::BoundExceeded);
    assert_eq!(nested.problem, "The blocks nest deeper than 16 levels.");
    assert_eq!(
        nested.fix,
        "Flatten one of the blocks so the nesting stays inside the limit."
    );

    let mut unbound = Action::new("unbound_key", "The action reads an absent name.");
    unbound.compensation = vec![Effect::Delete {
        collection: CollectionId::new("parcel"),
        key: KeySelector::Exact(Expr::bound("absent")),
    }];
    let key_fault = derive(&unbound, &BTreeMap::new());
    assert_eq!(key_fault.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        key_fault.location.as_str(),
        "actions[0].compensation[0].key"
    );

    let no_library = clean()
        .derive_manifest(
            &arguments(),
            Identity::default(),
            Timestamp::default(),
            no_blocks().as_ref(),
            &Path::root(),
        )
        .expect_err("the detached source holds no blocks");
    assert_eq!(no_library.code, DiagnosticCode::DanglingReference);
    assert!(no_blocks().block(&BlockId::new("stamp")).is_none());
}

#[test]
fn header_and_parameter_faults_each_raise_a_diagnostic() {
    let mut action = clean();
    action.id = ActionId::new("Bad");
    action.summary.clear();
    action.params = vec![
        Param::new("Bad Name", TypeExpr::Text, "A malformed name."),
        Param::new("area", TypeExpr::Decimal { scale: 31 }, "A scale past 30."),
        Param::new("area", TypeExpr::Integer, "A repeated name."),
    ];
    action.preconditions.clear();
    action.effects.clear();
    action.compensation.clear();
    action.roles.clear();
    let found = report(&action);
    assert_eq!(
        at(&found, "actions[0].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "actions[0].summary").fix,
        "Write one plain sentence stating what the action does."
    );
    assert_eq!(
        at(&found, "actions[0].params[0].name").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "actions[0].params[1].ty").code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found, "actions[0].params[2].name").fix,
        "Rename one of the two `area` parameters."
    );
    assert_eq!(
        at(&found, "actions[0].roles").fix,
        "Name at least one role that runs the action."
    );
}

#[test]
fn role_and_precondition_faults_each_raise_a_diagnostic() {
    let mut action = clean();
    action.roles = vec![RoleId::new("Auditor"), RoleId::new("auditor")];
    action.preconditions = vec![RuleId::new("ghost_rule"), RuleId::new("area_fits")];
    action.params = vec![Param::new("survey_number", TypeExpr::Text, "The parcel.")];
    action.effects.clear();
    let found = report(&action);
    assert_eq!(
        at(&found, "actions[0].roles[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "actions[0].roles[1]").fix,
        "Point the entry at one of these roles: registrar."
    );
    assert_eq!(
        at(&found, "actions[0].preconditions[0]").fix,
        "Point the entry at one of these preconditions: area_fits."
    );
    let unread = at(&found, "actions[0].preconditions[1]");
    assert_eq!(unread.code, DiagnosticCode::MissingValue);
    assert_eq!(
        unread.fix,
        "Declare a parameter named `area` carrying a decimal at scale 4."
    );

    let mut typed = clean();
    typed.params[1].ty = TypeExpr::Integer;
    typed.effects.clear();
    let found = report(&typed);
    let mismatch = at(&found, "actions[0].preconditions[0]");
    assert_eq!(mismatch.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        mismatch.fix,
        "Declare the parameter `area` as a decimal at scale 4."
    );

    let empty_rules = Rules::default();
    let found = report_with(&clean(), &empty_rules);
    assert_eq!(
        at(&found, "actions[0].preconditions[0]").fix,
        "Declare a precondition named `area_fits` in the rules section."
    );
}

fn delete(collection: &str, key: KeySelector) -> Effect {
    Effect::Delete {
        collection: CollectionId::new(collection),
        key,
    }
}

fn update(collection: &str, field: FieldPath, value: Expr) -> Effect {
    Effect::Update {
        collection: CollectionId::new(collection),
        key: KeySelector::Full,
        field,
        value,
    }
}

#[test]
fn key_and_write_faults_each_raise_a_diagnostic() {
    let mut action = clean();
    action.effects = vec![
        delete("ghost", KeySelector::Full),
        delete("parcel", KeySelector::Exact(Expr::bound("absent"))),
        delete("parcel", KeySelector::Exact(Expr::integer(1))),
        delete(
            "parcel",
            KeySelector::Prefix(Expr::Literal(Value::none(TypeExpr::Text))),
        ),
        Effect::Insert {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Full,
            record: Expr::MakeRecord {
                type_id: TypeId::new("ledger"),
                fields: vec![
                    (FieldName::new("entry"), Expr::integer(1)),
                    (FieldName::new("note"), Expr::text("x")),
                ],
            },
        },
        Effect::Insert {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Full,
            record: Expr::bound("absent"),
        },
        update("parcel", FieldPath::default(), Expr::text("x")),
        update("parcel", FieldPath::parse("missing"), Expr::text("x")),
        update("parcel", FieldPath::parse("area"), Expr::integer(1)),
        update("parcel", FieldPath::parse("area"), Expr::bound("absent")),
        update("orphan", FieldPath::parse("missing"), Expr::integer(1)),
    ];
    action.compensation = vec![delete("ghost", KeySelector::Full)];
    let found = report(&action);
    let prefix = "actions[0].effects";
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[0].collection")).fix,
        "Point the reference at one of these collections: ledger, orphan, parcel."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[1].key")).code,
        DiagnosticCode::DanglingReference
    );
    let exact_key = at(&found, &alloc::format!("{prefix}[2].key"));
    assert_eq!(exact_key.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        exact_key.problem,
        "The key carries a whole number and `parcel` keys records by text."
    );
    assert_eq!(exact_key.fix, "Supply text as the key.");
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[3].key")).fix,
        "Supply a key prefix carrying text, a whole number, a timestamp, an identity or a hash."
    );
    let wrong_record = at(&found, &alloc::format!("{prefix}[4].record"));
    assert_eq!(wrong_record.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        wrong_record.fix,
        "Build the record `parcel` for the insert."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[5].record")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[6].field")).code,
        DiagnosticCode::MissingValue
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[7].field")).fix,
        "Name a field declared on the record type of `parcel`."
    );
    let wrong_value = at(&found, &alloc::format!("{prefix}[8].value"));
    assert_eq!(wrong_value.code, DiagnosticCode::TypeMismatch);
    assert_eq!(wrong_value.fix, "Supply a decimal at scale 4 here.");
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[9].value")).code,
        DiagnosticCode::DanglingReference
    );
    assert!(found
        .diagnostics
        .iter()
        .all(|d| !d.location.as_str().starts_with("actions[0].effects[10]")));
    assert_eq!(
        at(&found, "actions[0].compensation[0].collection").code,
        DiagnosticCode::DanglingReference
    );
}

#[test]
fn read_emit_and_block_faults_each_raise_a_diagnostic() {
    let mut action = clean();
    action.effects = vec![
        Effect::Read {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Full,
            binding: ParamName::new("Bad"),
            element: TypeExpr::Record(TypeId::new("ledger")),
        },
        Effect::Read {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Full,
            binding: ParamName::new("found"),
            element: TypeExpr::Sum(TypeId::new("ghost")),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: Expr::bound("absent"),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: Expr::bound("found"),
        },
        Effect::RunBlock {
            block: BlockId::new("ghost"),
            arguments: Vec::new(),
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![
                (ParamName::new("Bad"), Expr::integer(1)),
                (ParamName::new("extra"), Expr::bound("absent")),
            ],
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![(ParamName::new("note"), Expr::integer(1))],
        },
        Effect::Emit {
            event: EventId::new("stamped"),
            payload: Expr::bound("stamped"),
        },
    ];
    let found = report(&action);
    let prefix = "actions[0].effects";
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[0].binding")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[0].element")).fix,
        "Declare the record `parcel` as the element type."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[1].element")).fix,
        "Declare a sum type named `ghost` in the types section."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[2].payload")).code,
        DiagnosticCode::DanglingReference
    );
    assert!(found.find(&alloc::format!("{prefix}[3].payload")).is_none());
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[4].block")).fix,
        "Declare a block named `ghost` in the composition library."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[5].arguments[0].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}[5].arguments[1].value")).code,
        DiagnosticCode::DanglingReference
    );
    let call_faults: Vec<&Diagnostic> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "actions[0].effects[5].arguments")
        .collect();
    let fixes: Vec<&str> = call_faults.iter().map(|d| d.fix.as_str()).collect();
    assert_eq!(
        fixes,
        [
            "Supply a value for `note`.",
            "Remove `Bad`, or declare it on the block `stamp`.",
            "Remove `extra`, or declare it on the block `stamp`.",
        ]
    );
    let wrong_input = at(&found, &alloc::format!("{prefix}[6].arguments"));
    assert_eq!(wrong_input.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        wrong_input.problem,
        "The input `note` carries a whole number and the block declares text."
    );
    assert_eq!(wrong_input.fix, "Supply text for `note`.");
    assert!(found.find(&alloc::format!("{prefix}[7].payload")).is_none());
}

#[test]
fn a_repeated_action_name_is_reported_by_the_section_pass() {
    let actions = vec![clean(), clean()];
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let library_blocks = blocks();
    let library = BlockLibrary::build(&library_blocks);
    let found = validate_actions(
        &actions,
        &registry,
        &collections,
        &identities(),
        &rules(),
        &library,
        &Path::root().field("actions"),
    );
    assert_eq!(found.len(), 1);
    assert_eq!(
        at(&found, "actions[1].id").fix,
        "Rename one of the two actions away from `record_parcel`."
    );
}

#[test]
fn the_registry_answers_lookups_and_names_the_actions_on_offer() {
    let actions = vec![
        clean(),
        Action::new("close_file", "The action closes a file."),
    ];
    let registry = ActionRegistry::build(&actions);
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert!(registry.contains(&ActionId::new("close_file")));
    assert_eq!(
        registry
            .get(&ActionId::new("record_parcel"))
            .map(|action| action.class),
        Some(TransactionClass::Structural)
    );
    assert_eq!(registry.names(), ["close_file", "record_parcel"]);
    assert_eq!(
        registry
            .missing(&ActionId::new("ghost"), Location::default())
            .fix,
        "Point the entry at one of these actions: close_file, record_parcel."
    );
    let none: Vec<Action> = Vec::new();
    let empty = ActionRegistry::build(&none);
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&ActionId::new("ghost"), Location::default())
            .fix,
        "Declare an action named `ghost` in the actions section."
    );
}
