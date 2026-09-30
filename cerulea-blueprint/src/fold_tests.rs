//! Behavioural tests for the bounded iteration primitive.
//!
//! Each test builds a real fold, runs the real checker or the real
//! evaluator over a real memory view, and asserts the concrete result.

use crate::{
    collections::{Collection, CollectionRegistry, KeyRange, KeySelector, PurposeDeclaration},
    expression::{BinaryOp, Env, Expr, Meter, TypeContext},
    fold::{
        CollectionView, DetachedView, FoldBound, FoldExpr, FoldSource, MemoryView,
        MAX_FOLD_ELEMENTS,
    },
    types::{
        CollectionId, FieldDef, FieldName, Identity, ParamName, RecordDef, Timestamp, TypeDef,
        TypeExpr, TypeId, TypeRegistry, Value,
    },
    DiagnosticCode, Path,
};
use alloc::{boxed::Box, collections::BTreeMap, string::ToString, vec, vec::Vec};

fn defs() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("line"),
        fields: vec![
            FieldDef::new("entry", TypeExpr::Text, "The entry."),
            FieldDef::new("amount", TypeExpr::Integer, "The amount."),
        ],
        summary: "One ledger line.".to_string(),
    })]
}

fn line(entry: &str, amount: i128) -> Value {
    Value::record(
        TypeId::new("line"),
        vec![
            (FieldName::new("entry"), Value::Text(entry.to_string())),
            (FieldName::new("amount"), Value::Integer(amount)),
        ],
    )
}

fn ledger() -> MemoryView {
    let mut view = MemoryView::new();
    let id = CollectionId::new("ledger");
    for (entry, amount) in [("a1", 10), ("a2", 20), ("b1", 5)] {
        view.insert(&id, &Value::Text(entry.to_string()), line(entry, amount));
    }
    view
}

fn sum_over(keys: KeySelector, maximum: u32) -> FoldExpr {
    FoldExpr::over_source(
        FoldSource::collection("ledger", keys, TypeExpr::Record(TypeId::new("line"))),
        "row",
        "total",
        Expr::integer(0),
        Expr::binary(
            BinaryOp::Add,
            Expr::bound("total"),
            Expr::field(Expr::bound("row"), "amount"),
        ),
        maximum,
    )
}

fn check(fold: &FoldExpr) -> Result<TypeExpr, crate::Diagnostic> {
    let defs = defs();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let ctx = TypeContext::new(&registry).with(&ParamName::new("seed"), TypeExpr::Text);
    fold.check(&ctx, &Path::root().field("fold"))
}

#[test]
fn a_memory_view_keeps_one_record_per_key_in_key_order() {
    let mut view = ledger();
    let id = CollectionId::new("ledger");
    assert_eq!(view.len(&id), 3);
    assert!(!view.is_empty());
    view.insert(&id, &Value::Text("a1".to_string()), line("a1", 99));
    assert_eq!(view.len(&id), 3);
    let exact = view.records(
        &id,
        &KeyRange::Exact(Value::Text("a1".to_string()).canonical_bytes()),
    );
    assert_eq!(exact, [line("a1", 99)]);
    assert!(view
        .records(&CollectionId::new("absent"), &KeyRange::Full)
        .is_empty());
    assert_eq!(view.len(&CollectionId::new("absent")), 0);
    assert!(MemoryView::new().is_empty());
    assert!(DetachedView.records(&id, &KeyRange::Full).is_empty());
}

#[test]
fn a_fold_over_a_key_range_reads_the_records_the_range_reaches() {
    let view = ledger();
    let env = Env::with_view(Identity::default(), Timestamp::default(), &view);
    let full = Expr::Fold(Box::new(sum_over(KeySelector::Full, 10)));
    assert_eq!(full.run(&env), Ok(Value::Integer(35)));
    let one = Expr::Fold(Box::new(sum_over(KeySelector::Exact(Expr::text("a2")), 10)));
    assert_eq!(one.run(&env), Ok(Value::Integer(20)));
    let fold = sum_over(KeySelector::Full, 10);
    let mut meter = Meter::default();
    assert_eq!(fold.proven_bound(&env, &mut meter, &Path::root()), Ok(3));
    assert_eq!(
        fold.reads()
            .map(|(collection, keys)| (collection.as_str(), keys.label())),
        Some(("ledger", "full"))
    );
    let tight = Expr::Fold(Box::new(sum_over(KeySelector::Full, 2)));
    let fault = tight
        .run(&env)
        .expect_err("three records pass a bound of two");
    assert_eq!(fault.code, DiagnosticCode::BoundExceeded);
    assert_eq!(
        fault.problem,
        "The fold walks 3 elements and declares a maximum of 2."
    );
    assert_eq!(
        fault.fix,
        "Raise the declared maximum above 3, or narrow the key range the fold reads."
    );
}

#[test]
fn the_declared_bound_sits_between_one_and_the_ceiling() {
    let path = Path::root().field("bound");
    assert!(FoldBound::new(1).validate(&path).is_ok());
    assert!(FoldBound::new(MAX_FOLD_ELEMENTS).validate(&path).is_ok());
    let zero = FoldBound::new(0).validate(&path).expect_err("zero fails");
    assert_eq!(zero.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(zero.fix, "Declare a maximum above zero.");
    let over = FoldBound::new(MAX_FOLD_ELEMENTS + 1)
        .validate(&path)
        .expect_err("the ceiling fails");
    assert_eq!(
        over.problem,
        "The fold declares a maximum of 1000001 elements, above the ceiling of 1000000."
    );
    assert_eq!(over.fix, "Declare a maximum of 1000000 elements or fewer.");
    let unbounded = sum_over(KeySelector::Full, 0);
    let fault = check(&unbounded).expect_err("the bound fails first");
    assert_eq!(fault.location.as_str(), "fold.bound");
}

#[test]
fn the_checker_rejects_every_malformed_fold() {
    assert_eq!(
        check(&sum_over(KeySelector::Prefix(Expr::bound("seed")), 5)),
        Ok(TypeExpr::Integer)
    );
    let mut same_name = sum_over(KeySelector::Full, 5);
    same_name.element = ParamName::new("total");
    let duplicate = check(&same_name).expect_err("one name for both fails");
    assert_eq!(duplicate.code, DiagnosticCode::DuplicateName);
    assert_eq!(
        duplicate.fix,
        "Give the element and the running result two names."
    );
    let not_list = FoldExpr::over_list(
        Expr::integer(4),
        "item",
        "total",
        Expr::integer(0),
        Expr::bound("total"),
        5,
    );
    let walks = check(&not_list).expect_err("a whole number is no list");
    assert_eq!(walks.location.as_str(), "fold.source");
    assert_eq!(walks.problem, "The fold walks a whole number.");
    let ghost = sum_over(KeySelector::Full, 5);
    let mut ghost = ghost;
    if let FoldSource::Collection { element, .. } = &mut ghost.source {
        *element = TypeExpr::Record(TypeId::new("ghost"));
    }
    let element = check(&ghost).expect_err("the element type is absent");
    assert_eq!(element.location.as_str(), "fold.source.element");
    let bad_key = check(&sum_over(KeySelector::Exact(Expr::bound("absent")), 5))
        .expect_err("the key reads an absent name");
    assert_eq!(bad_key.location.as_str(), "fold.source.keys");
    let mut wrong_step = sum_over(KeySelector::Full, 5);
    wrong_step.step = Expr::text("x");
    let step = check(&wrong_step).expect_err("the step carries text");
    assert_eq!(step.location.as_str(), "fold.step");
    assert_eq!(step.fix, "Give the step the type a whole number.");
}

#[test]
fn a_list_source_evaluating_to_another_shape_stops_the_fold() {
    let fold = FoldExpr::over_list(
        Expr::integer(4),
        "item",
        "total",
        Expr::integer(0),
        Expr::bound("total"),
        5,
    );
    let mut meter = Meter::default();
    let fault = fold
        .evaluate(&Env::default(), &mut meter, &Path::root().field("fold"))
        .expect_err("a whole number is no list");
    assert_eq!(fault.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        fault.fix,
        "Fold over a list value or over a collection range."
    );
    assert_eq!(fold.reads(), None);
}

#[test]
fn walking_and_substitution_reach_every_key_shape() {
    for keys in [
        KeySelector::Exact(Expr::bound("seed")),
        KeySelector::Prefix(Expr::bound("seed")),
        KeySelector::Full,
    ] {
        let label = keys.label();
        let fold = sum_over(keys, 5);
        let mut replacements = BTreeMap::new();
        replacements.insert(ParamName::new("seed"), Expr::text("a"));
        replacements.insert(ParamName::new("row"), Expr::integer(0));
        let replaced = fold.substitute(&replacements);
        let mut reads = Vec::new();
        replaced.walk(&mut |expr| {
            if let Expr::Bound(name) = expr {
                reads.push(name.as_str().to_string());
            }
        });
        assert_eq!(reads, ["total", "row"], "{label}");
        let mut visited = 0usize;
        let mut copy = fold.clone();
        copy.walk_mut(&mut |_| visited += 1);
        let expected = if label == "full" { 5 } else { 6 };
        assert_eq!(visited, expected, "{label}");
        let mut bound = Vec::new();
        let mut free = Vec::new();
        fold.free_names(&mut bound, &mut free);
        let names: Vec<&str> = free.iter().map(|name| name.as_str()).collect();
        let expected: &[&str] = if label == "full" { &[] } else { &["seed"] };
        assert_eq!(names, expected, "{label}");
    }
}

#[test]
fn the_source_check_matches_the_collection_record_type() {
    let mut ledger = Collection::new("ledger", "line", "entry", "The ledger.");
    ledger.purpose = PurposeDeclaration::new("The ledger records lines.", &["accounting"]);
    let held = vec![ledger];
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    assert!(sum_over(KeySelector::Full, 5)
        .validate_source(&collections, &Path::root().field("fold"))
        .is_clean());
    let mut other = sum_over(KeySelector::Full, 5);
    if let FoldSource::Collection { element, .. } = &mut other.source {
        *element = TypeExpr::Record(TypeId::new("entry"));
    }
    let found = other.validate_source(&collections, &Path::root().field("fold"));
    let mismatch = found
        .find("fold.source.element")
        .expect("the element type differs");
    assert_eq!(
        mismatch.fix,
        "Declare the record `line` as the element type."
    );
    let mut absent = sum_over(KeySelector::Full, 5);
    if let FoldSource::Collection { collection, .. } = &mut absent.source {
        *collection = CollectionId::new("archive");
    }
    let found = absent.validate_source(&collections, &Path::root().field("fold"));
    assert_eq!(
        found.find("fold.source.collection").map(|d| d.fix.as_str()),
        Some("Point the reference at one of these collections: ledger.")
    );
    let list = FoldExpr::over_list(
        Expr::integer(1),
        "item",
        "total",
        Expr::integer(0),
        Expr::bound("total"),
        5,
    );
    assert!(list.validate_source(&collections, &Path::root()).is_clean());
}
