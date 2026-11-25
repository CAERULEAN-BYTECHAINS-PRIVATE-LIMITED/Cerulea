//! Behavioural tests for the composition library.
//!
//! Each test builds real blocks, runs the real inliner or the real library
//! check, and asserts the concrete result, the diagnostic class and the fix.

use crate::{
    actions::{Action, BlockOutput, Effect, Param},
    collections::{Collection, CollectionRegistry, KeySelector, PurposeDeclaration},
    compose::{inline_action, inline_effects, output_type, validate_blocks, Block, BlockLibrary},
    expression::Expr,
    types::{
        BlockId, CollectionId, FieldDef, ParamName, RecordDef, TypeDef, TypeExpr, TypeId,
        TypeRegistry,
    },
    DiagnosticCode, Location, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("note"),
        fields: vec![FieldDef::new("text", TypeExpr::Text, "The text.")],
        summary: "One note.".to_string(),
    })]
}

fn collections() -> Vec<Collection> {
    let mut notes = Collection::new("notes", "note", "text", "The notes.");
    notes.purpose = PurposeDeclaration::new("The desk keeps notes.", &["filing"]);
    vec![notes]
}

fn delete_note(key: Expr) -> Effect {
    Effect::Delete {
        collection: CollectionId::new("notes"),
        key: KeySelector::Exact(key),
    }
}

fn run(block: &str, arguments: Vec<(&str, Expr)>) -> Effect {
    Effect::RunBlock {
        block: BlockId::new(block),
        arguments: arguments
            .into_iter()
            .map(|(name, value)| (ParamName::new(name), value))
            .collect(),
    }
}

fn library_blocks() -> Vec<Block> {
    let mut clear = Block::new("clear", "The block clears one note.");
    clear.inputs = vec![Param::new("text", TypeExpr::Text, "The note text.")];
    clear.outputs = vec![BlockOutput::new(
        "cleared",
        TypeExpr::Text,
        Expr::bound("text"),
    )];
    clear.effects = vec![delete_note(Expr::bound("text"))];
    let mut twice = Block::new("twice", "The block clears two notes.");
    twice.inputs = vec![
        Param::new("first", TypeExpr::Text, "The first text."),
        Param::new("second", TypeExpr::Text, "The second text."),
    ];
    twice.effects = vec![
        run("clear", vec![("text", Expr::bound("first"))]),
        run("clear", vec![("text", Expr::bound("second"))]),
    ];
    vec![clear, twice]
}

fn report(blocks: &[Block]) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    validate_blocks(
        blocks,
        &registry,
        &collections,
        &Path::root().field("blocks"),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_library_passes_and_answers_lookups() {
    let blocks = library_blocks();
    assert!(report(&blocks).is_clean());
    let library = BlockLibrary::build(&blocks);
    assert_eq!(library.len(), 2);
    assert!(!library.is_empty());
    assert!(library.contains(&BlockId::new("twice")));
    assert_eq!(library.names(), ["clear", "twice"]);
    assert_eq!(
        library
            .missing(&BlockId::new("ghost"), Location::default())
            .fix,
        "Point the entry at one of these blocks: clear, twice."
    );
    let none: Vec<Block> = Vec::new();
    let empty = BlockLibrary::build(&none);
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&BlockId::new("ghost"), Location::default())
            .fix,
        "Declare a block named `ghost` in the composition library."
    );
    assert_eq!(
        output_type(&blocks[0], &ParamName::new("cleared")),
        Some(TypeExpr::Text)
    );
    assert_eq!(output_type(&blocks[0], &ParamName::new("absent")), None);
    let refs = blocks[1].block_refs(&Path::root());
    assert_eq!(refs.len(), 2);
    assert_eq!(refs[1].1.as_str(), "effects[1].block");
}

#[test]
fn inlining_flattens_nested_blocks_into_plain_effects() {
    let blocks = library_blocks();
    let library = BlockLibrary::build(&blocks);
    let mut action = Action::new("tidy", "The action clears two notes.");
    action.effects = vec![run(
        "twice",
        vec![("first", Expr::text("a")), ("second", Expr::text("b"))],
    )];
    action.compensation = vec![run("clear", vec![("text", Expr::text("c"))])];
    let flat = inline_action(&action, &library, &Path::root()).expect("the blocks inline");
    assert_eq!(
        flat.effects,
        [delete_note(Expr::text("a")), delete_note(Expr::text("b"))]
    );
    assert_eq!(flat.compensation, [delete_note(Expr::text("c"))]);
    let outputs = blocks[0]
        .inline_outputs(&[(ParamName::new("text"), Expr::text("z"))], &Path::root())
        .expect("the outputs inline");
    assert_eq!(outputs, [(ParamName::new("cleared"), Expr::text("z"))]);
}

#[test]
fn inlining_reports_a_missing_block_a_missing_input_and_a_deep_nest() {
    let blocks = library_blocks();
    let library = BlockLibrary::build(&blocks);
    let absent = inline_effects(&[run("ghost", Vec::new())], &library, 0, &Path::root())
        .expect_err("the block is absent");
    assert_eq!(absent.location.as_str(), "[0].block");
    let short = inline_effects(&[run("clear", Vec::new())], &library, 0, &Path::root())
        .expect_err("the text is absent");
    assert_eq!(short.code, DiagnosticCode::MissingValue);
    assert_eq!(
        short.problem,
        "The call to `clear` leaves out the input `text`."
    );
    assert_eq!(short.fix, "Supply a value for `text`.");
    let outputs = blocks[0]
        .inline_outputs(&[], &Path::root())
        .expect_err("the text is absent");
    assert_eq!(outputs.fix, "Supply a value for `text`.");
    let mut looping = Block::new("looping", "The block runs itself.");
    looping.effects = vec![run("looping", Vec::new())];
    let looped = vec![looping];
    let library = BlockLibrary::build(&looped);
    let deep = inline_effects(&[run("looping", Vec::new())], &library, 0, &Path::root())
        .expect_err("the nest runs too deep");
    assert_eq!(deep.code, DiagnosticCode::BoundExceeded);
    assert_eq!(
        deep.fix,
        "Flatten one of the blocks so the nesting stays inside the limit."
    );
}

#[test]
fn block_faults_each_raise_a_diagnostic() {
    let mut blocks = library_blocks();
    let mut repeated = Block::new("clear", "");
    repeated.inputs = vec![
        Param::new("Text", TypeExpr::Text, "A malformed name."),
        Param::new("text", TypeExpr::Decimal { scale: 31 }, "A scale past 30."),
        Param::new("text", TypeExpr::Text, "A repeated name."),
    ];
    repeated.outputs = vec![
        BlockOutput::new("text", TypeExpr::Text, Expr::bound("text")),
        BlockOutput::new("Out", TypeExpr::Integer, Expr::bound("text")),
        BlockOutput::new("broken", TypeExpr::Text, Expr::bound("absent")),
    ];
    repeated.effects = vec![
        Effect::Delete {
            collection: CollectionId::new("ghost"),
            key: KeySelector::Full,
        },
        run("ghost_block", Vec::new()),
    ];
    blocks.push(repeated);
    blocks.push(Block::new("idle", "The block does nothing."));
    let found = report(&blocks);
    let base = "blocks[2]";
    assert_eq!(
        at(&found, &alloc::format!("{base}.id")).fix,
        "Rename one of the two blocks away from `clear`."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.summary")).fix,
        "Write one plain sentence stating what the block does."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.inputs[0].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.inputs[1].ty")).code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.inputs[2].name")).fix,
        "Rename one of the two `text` inputs."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.outputs[0].name")).fix,
        "Rename the output `text`."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.outputs[1].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    let typed = at(&found, &alloc::format!("{base}.outputs[1].value"));
    assert_eq!(
        typed.problem,
        "The output `Out` carries text and declares a whole number."
    );
    assert_eq!(typed.fix, "Give the output the type text.");
    assert_eq!(
        at(&found, &alloc::format!("{base}.outputs[2].value")).code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.effects[0].collection")).fix,
        "Point the reference at one of these collections: notes."
    );
    assert_eq!(
        at(&found, &alloc::format!("{base}.effects[1].block")).fix,
        "Point the entry at one of these blocks: clear, idle, twice."
    );
    let idle = at(&found, "blocks[3].effects");
    assert_eq!(idle.code, DiagnosticCode::SectionEmpty);
    assert_eq!(
        idle.fix,
        "Add at least one step, or declare at least one output."
    );
}

#[test]
fn blocks_calling_one_another_in_a_loop_are_reported() {
    let mut first = Block::new("first", "The block calls the second.");
    first.effects = vec![run("second", Vec::new())];
    let mut second = Block::new("second", "The block calls the first.");
    second.effects = vec![run("first", Vec::new())];
    let found = report(&[first, second]);
    assert_eq!(found.len(), 1);
    let cycle = at(&found, "blocks[0].effects");
    assert_eq!(cycle.code, DiagnosticCode::CycleDetected);
    assert_eq!(
        cycle.problem,
        "The composition closes a loop: first -> second -> first."
    );
    assert_eq!(
        cycle.fix,
        "Remove one call on the loop so the composition stays one way."
    );
}
