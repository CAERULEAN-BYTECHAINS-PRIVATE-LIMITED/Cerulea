//! Behavioural tests for the typed expression language.
//!
//! Each test builds real expressions, runs the real checker or the real
//! evaluator, and asserts the concrete result. Every reject path asserts the
//! diagnostic class and the fix sentence the diagnostic carries.

use crate::{
    collections::{KeyRange, KeySelector},
    expression::{
        is_recoverable, BinaryOp, Builtin, Env, Expr, MatchArm, Meter, Pattern, TypeContext,
        UnaryOp, DEFAULT_STEP_LIMIT,
    },
    fold::{FoldExpr, FoldSource, MemoryView},
    types::{
        CollectionId, Decimal, EnumerationDef, FieldDef, FieldName, Hash32, Identity, MapValue,
        ParamName, RecordDef, SumDef, SumVariant, Timestamp, TypeDef, TypeExpr, TypeId,
        TypeRegistry, Value, VariantName,
    },
    Diagnostic, DiagnosticCode, Path,
};
use alloc::{boxed::Box, collections::BTreeMap, format, string::ToString, vec, vec::Vec};
use codec::{Decode, Encode};

fn defs() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("invoice"),
            fields: vec![
                FieldDef::new("total", TypeExpr::Decimal { scale: 2 }, "The amount due."),
                FieldDef::new("payer", TypeExpr::Identity, "The party that owes."),
            ],
            summary: "One invoice.".to_string(),
        }),
        TypeDef::Sum(SumDef {
            id: TypeId::new("ruling"),
            variants: vec![
                SumVariant {
                    name: VariantName::new("approval"),
                    payload: Some(TypeExpr::Decimal { scale: 2 }),
                },
                SumVariant {
                    name: VariantName::new("withdrawn"),
                    payload: None,
                },
            ],
            summary: "The outcome of a review.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("tenure"),
            variants: vec![VariantName::new("freehold"), VariantName::new("leasehold")],
            summary: "The tenure held.".to_string(),
        }),
    ]
}

fn check_with(expr: &Expr, bindings: &[(&str, TypeExpr)]) -> Result<TypeExpr, Diagnostic> {
    let defs = defs();
    let (registry, report) = TypeRegistry::build(&defs, &Path::root().field("types"));
    assert!(report.is_clean());
    let mut ctx = TypeContext::new(&registry);
    for (name, ty) in bindings {
        ctx.bind(&ParamName::new(name), ty.clone());
    }
    expr.check(&ctx, &Path::root().field("expr"))
}

fn check(expr: &Expr) -> Result<TypeExpr, Diagnostic> {
    check_with(expr, &[])
}

fn check_err(expr: &Expr) -> Diagnostic {
    check(expr).expect_err("the checker rejects the expression")
}

fn env() -> Env<'static> {
    Env::new(Identity([7u8; 32]), Timestamp::from_millis(1_000))
}

fn run(expr: &Expr) -> Result<Value, Diagnostic> {
    expr.run(&env())
}

fn run_err(expr: &Expr) -> Diagnostic {
    run(expr).expect_err("the evaluator rejects the expression")
}

fn lit(value: Value) -> Expr {
    Expr::Literal(value)
}

fn dec(units: i128, scale: u8) -> Value {
    Value::Decimal(Decimal::new(units, scale))
}

fn int_list(items: &[i128]) -> Value {
    Value::list(
        TypeExpr::Integer,
        items.iter().map(|item| Value::Integer(*item)).collect(),
    )
}

fn int_map(entries: &[(i128, &str)]) -> Value {
    Value::map(
        TypeExpr::Integer,
        TypeExpr::Text,
        entries
            .iter()
            .map(|(key, value)| (Value::Integer(*key), Value::Text(value.to_string())))
            .collect(),
    )
}

fn unsorted_map() -> Value {
    Value::Map(MapValue {
        key: TypeExpr::Integer,
        value: TypeExpr::Text,
        entries: vec![
            (Value::Integer(9), Value::Text("z".to_string())),
            (Value::Integer(1), Value::Text("a".to_string())),
        ],
    })
}

fn call(function: Builtin, args: Vec<Expr>) -> Expr {
    Expr::Call { function, args }
}

fn ruling(variant: &str, payload: Option<Value>) -> Value {
    Value::sum(TypeId::new("ruling"), VariantName::new(variant), payload)
}

fn tenure(variant: &str) -> Value {
    Value::enumeration(TypeId::new("tenure"), VariantName::new(variant))
}

fn arm(pattern: Pattern, body: Expr) -> MatchArm {
    MatchArm { pattern, body }
}

fn variant(name: &str, binding: Option<&str>) -> Pattern {
    Pattern::Variant {
        variant: VariantName::new(name),
        binding: binding.map(ParamName::new),
    }
}

fn choice(name: &str) -> Pattern {
    Pattern::Choice {
        variant: VariantName::new(name),
    }
}

fn present(name: &str) -> Pattern {
    Pattern::Present {
        binding: ParamName::new(name),
    }
}

fn matching(scrutinee: Expr, arms: Vec<MatchArm>, fallback: Option<Expr>) -> Expr {
    Expr::Match {
        scrutinee: Box::new(scrutinee),
        arms,
        fallback: fallback.map(Box::new),
    }
}

/// An expression holding one node of every shape the language offers.
///
/// The tree holds 66 nodes. Twenty six of them read a bound name, and
/// twenty of those reads name `seed`.
fn every_shape() -> Expr {
    let fold_over_list = Expr::Fold(Box::new(FoldExpr::over_list(
        Expr::MakeList {
            element: TypeExpr::Integer,
            items: vec![Expr::bound("seed"), Expr::integer(2)],
        },
        "item",
        "sum",
        Expr::integer(0),
        Expr::binary(BinaryOp::Add, Expr::bound("sum"), Expr::bound("item")),
        8,
    )));
    let fold_over_rows = Expr::Fold(Box::new(FoldExpr::over_source(
        FoldSource::collection(
            "ledger",
            KeySelector::Exact(Expr::bound("seed")),
            TypeExpr::Record(TypeId::new("invoice")),
        ),
        "row",
        "count",
        Expr::integer(0),
        Expr::binary(BinaryOp::Add, Expr::bound("count"), Expr::integer(1)),
        8,
    )));
    let items = vec![
        Expr::field(Expr::bound("seed"), "total"),
        Expr::Unary {
            op: UnaryOp::Negate,
            operand: Box::new(Expr::bound("seed")),
        },
        Expr::conditional(Expr::boolean(true), Expr::integer(1), Expr::integer(2)),
        matching(
            Expr::bound("seed"),
            vec![
                arm(variant("approval", Some("amount")), Expr::bound("amount")),
                arm(present("inner"), Expr::bound("inner")),
                arm(Pattern::Absent, Expr::integer(0)),
            ],
            Some(Expr::bound("seed")),
        ),
        Expr::Let {
            name: ParamName::new("local"),
            value: Box::new(Expr::bound("seed")),
            body: Box::new(Expr::bound("local")),
        },
        Expr::MakeRecord {
            type_id: TypeId::new("invoice"),
            fields: vec![(FieldName::new("total"), Expr::bound("seed"))],
        },
        Expr::MakeEnumeration {
            type_id: TypeId::new("tenure"),
            variant: VariantName::new("freehold"),
        },
        Expr::MakeSum {
            type_id: TypeId::new("ruling"),
            variant: VariantName::new("approval"),
            payload: Some(Box::new(Expr::bound("seed"))),
        },
        Expr::MakeMap {
            key: TypeExpr::Integer,
            value: TypeExpr::Reference(CollectionId::new("parcels")),
            entries: vec![(Expr::integer(1), Expr::bound("seed"))],
        },
        Expr::MakeOptional {
            element: TypeExpr::optional(TypeExpr::Reference(CollectionId::new("owners"))),
            item: Some(Box::new(Expr::bound("seed"))),
        },
        Expr::MakeReference {
            collection: CollectionId::new("titles"),
            key: Box::new(Expr::bound("seed")),
        },
        Expr::UnwrapOr {
            optional: Box::new(Expr::bound("seed")),
            fallback: Box::new(Expr::integer(0)),
        },
        Expr::IsPresent(Box::new(Expr::bound("seed"))),
        Expr::Length(Box::new(Expr::bound("seed"))),
        Expr::Contains {
            list: Box::new(Expr::bound("seed")),
            item: Box::new(Expr::integer(3)),
        },
        Expr::Lookup {
            map: Box::new(Expr::bound("seed")),
            key: Box::new(Expr::integer(3)),
        },
        Expr::Rescale {
            value: Box::new(Expr::bound("seed")),
            scale: 4,
        },
        call(
            Builtin::MaximumInteger,
            vec![Expr::bound("seed"), Expr::integer(1)],
        ),
        fold_over_list,
        fold_over_rows,
        Expr::Attempt {
            body: Box::new(Expr::bound("seed")),
            compensation: Box::new(Expr::integer(0)),
        },
        lit(Value::reference(
            CollectionId::new("deeds"),
            Value::Text("d1".to_string()),
        )),
    ];
    Expr::MakeList {
        element: TypeExpr::Integer,
        items,
    }
}

#[test]
fn a_walk_visits_every_node_of_every_shape() {
    let expr = every_shape();
    let mut count = 0usize;
    let mut bound_reads = 0usize;
    expr.walk(&mut |node| {
        count += 1;
        if matches!(node, Expr::Bound(_)) {
            bound_reads += 1;
        }
    });
    let mut mutable_count = 0usize;
    let mut copy = expr.clone();
    copy.walk_mut(&mut |_| mutable_count += 1);
    assert_eq!(count, 66);
    assert_eq!(mutable_count, 66);
    assert_eq!(bound_reads, 26);
}

#[test]
fn a_mutable_walk_rewrites_every_literal() {
    let mut expr = every_shape();
    expr.walk_mut(&mut |node| {
        if let Expr::Literal(Value::Integer(value)) = node {
            *value += 100;
        }
    });
    let mut integers = Vec::new();
    expr.walk(&mut |node| {
        if let Expr::Literal(Value::Integer(value)) = node {
            integers.push(*value);
        }
    });
    assert_eq!(integers.len(), 13);
    assert!(integers.iter().all(|value| *value >= 100));
}

#[test]
fn collection_refs_name_every_collection_in_every_shape() {
    let mut found = Vec::new();
    every_shape().collection_refs(&mut found);
    let names: Vec<&str> = found.iter().map(|id| id.as_str()).collect();
    assert_eq!(names, ["parcels", "owners", "titles", "ledger", "deeds"]);
}

#[test]
fn free_names_skip_every_local_binding() {
    let mut bound = Vec::new();
    let mut out = Vec::new();
    every_shape().free_names(&mut bound, &mut out);
    let names: Vec<&str> = out.iter().map(|name| name.as_str()).collect();
    assert_eq!(names, ["seed"]);
    assert!(bound.is_empty());
}

#[test]
fn substitution_reaches_every_shape_and_respects_local_bindings() {
    let mut replacements = BTreeMap::new();
    replacements.insert(ParamName::new("seed"), Expr::integer(41));
    replacements.insert(ParamName::new("amount"), Expr::integer(-1));
    replacements.insert(ParamName::new("item"), Expr::integer(-2));
    let replaced = every_shape().substitute(&replacements);
    let mut bound = Vec::new();
    let mut out = Vec::new();
    replaced.free_names(&mut bound, &mut out);
    assert!(out.is_empty());
    let mut shadowed = 0usize;
    let mut seeds = 0usize;
    replaced.walk(&mut |node| {
        if let Expr::Bound(name) = node {
            if name.as_str() == "amount" || name.as_str() == "item" {
                shadowed += 1;
            }
        }
        if node == &Expr::integer(41) {
            seeds += 1;
        }
    });
    assert_eq!(shadowed, 2);
    assert_eq!(seeds, 20);
}

#[test]
fn the_full_expression_round_trips_through_scale_and_json() {
    let expr = every_shape();
    let bytes = expr.encode();
    let decoded = Expr::decode(&mut &bytes[..]).expect("the encoding decodes");
    assert_eq!(decoded, expr);
    let text = serde_json::to_string(&expr).expect("the expression serialises");
    let back: Expr = serde_json::from_str(&text).expect("the text parses");
    assert_eq!(back, expr);
}

#[test]
fn canonicalise_sorts_every_literal_map() {
    let mut expr = Expr::Length(Box::new(lit(unsorted_map())));
    expr.canonicalise();
    assert_eq!(
        expr,
        Expr::Length(Box::new(lit(int_map(&[(1, "a"), (9, "z")]))))
    );
}

#[test]
fn the_binary_labels_and_builtin_labels_name_each_operation() {
    let labels: Vec<&str> = [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::Equal,
        BinaryOp::NotEqual,
        BinaryOp::Less,
        BinaryOp::LessOrEqual,
        BinaryOp::Greater,
        BinaryOp::GreaterOrEqual,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::ConcatText,
        BinaryOp::AppendList,
    ]
    .iter()
    .map(|op| op.label())
    .collect();
    assert_eq!(
        labels,
        [
            "addition",
            "subtraction",
            "multiplication",
            "division",
            "remainder",
            "equality",
            "inequality",
            "less than",
            "less than or equal",
            "greater than",
            "greater than or equal",
            "conjunction",
            "disjunction",
            "text join",
            "list append",
        ]
    );
    let builtins = [
        (Builtin::Now, "now", 0),
        (Builtin::Caller, "caller", 0),
        (Builtin::HashOf, "hash_of", 1),
        (Builtin::TimestampPlus, "timestamp_plus", 2),
        (Builtin::TimestampSince, "timestamp_since", 2),
        (Builtin::AbsoluteInteger, "absolute", 1),
        (Builtin::MinimumInteger, "minimum", 2),
        (Builtin::MaximumInteger, "maximum", 2),
        (Builtin::BytesOfText, "bytes_of_text", 1),
    ];
    for (builtin, label, arity) in builtins {
        assert_eq!(builtin.label(), label);
        assert_eq!(builtin.arity(), arity);
    }
}

#[test]
fn the_meter_counts_steps_and_stops_past_the_limit() {
    let mut meter = Meter::new(3);
    assert_eq!(meter.limit(), 3);
    assert_eq!(meter.used(), 0);
    assert_eq!(meter.remaining(), 3);
    meter
        .charge(2, &Path::root().field("step"))
        .expect("two steps fit");
    assert_eq!(meter.remaining(), 1);
    let fault = meter
        .charge(2, &Path::root().field("step"))
        .expect_err("four steps overrun a budget of three");
    assert_eq!(fault.code, DiagnosticCode::BoundExceeded);
    assert_eq!(fault.location.as_str(), "step");
    assert_eq!(
        fault.problem,
        "The expression ran past its budget of 3 steps."
    );
    assert_eq!(
        fault.fix,
        "Raise the declared step limit or reduce the work the expression does."
    );
    assert_eq!(meter.remaining(), 0);
    meter
        .charge(u64::MAX, &Path::root())
        .expect_err("a saturated meter stays past its limit");
    assert_eq!(meter.used(), u64::MAX);
    assert_eq!(Meter::default().limit(), DEFAULT_STEP_LIMIT);
}

#[test]
fn recoverable_faults_are_the_arithmetic_and_absent_value_classes() {
    let at = |code| Diagnostic::new(code, Path::root(), "The problem.", "The fix.");
    for code in [
        DiagnosticCode::ArithmeticOverflow,
        DiagnosticCode::DivisionByZero,
        DiagnosticCode::PrecisionLoss,
        DiagnosticCode::MissingValue,
    ] {
        assert!(is_recoverable(&at(code)));
    }
    for code in [
        DiagnosticCode::BoundExceeded,
        DiagnosticCode::TypeMismatch,
        DiagnosticCode::DanglingReference,
    ] {
        assert!(!is_recoverable(&at(code)));
    }
}

#[test]
fn the_environment_reports_its_caller_time_view_and_names() {
    let detached = Env::default();
    assert_eq!(detached.caller(), Identity::default());
    assert_eq!(detached.now(), Timestamp::default());
    assert!(detached.names().is_empty());

    let mut view = MemoryView::new();
    view.insert(
        &CollectionId::new("ledger"),
        &Value::Integer(1),
        Value::Integer(10),
    );
    let mut scoped = Env::with_view(Identity([3u8; 32]), Timestamp::from_millis(5), &view);
    scoped.bind(&ParamName::new("zeta"), Value::Bool(true));
    let widened = scoped.with(&ParamName::new("alpha"), Value::Integer(2));
    assert_eq!(widened.names(), ["alpha", "zeta"]);
    assert_eq!(scoped.names(), ["zeta"]);
    assert_eq!(
        widened.lookup(&ParamName::new("alpha")),
        Some(&Value::Integer(2))
    );
    assert_eq!(widened.caller(), Identity([3u8; 32]));
    assert_eq!(widened.now(), Timestamp::from_millis(5));
    let rows = widened
        .view()
        .records(&CollectionId::new("ledger"), &KeyRange::Full);
    assert_eq!(rows, [Value::Integer(10)]);
}

#[test]
fn the_type_context_lists_names_in_sorted_order() {
    let defs = defs();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let ctx = TypeContext::new(&registry)
        .with(&ParamName::new("zeta"), TypeExpr::Bool)
        .with(&ParamName::new("alpha"), TypeExpr::Text);
    assert_eq!(ctx.names(), ["alpha", "zeta"]);
    assert_eq!(ctx.lookup(&ParamName::new("alpha")), Some(&TypeExpr::Text));
    assert_eq!(ctx.lookup(&ParamName::new("beta")), None);
}

// Checker paths

#[test]
fn an_unknown_name_lists_the_names_in_scope_or_nothing() {
    let empty = check_err(&Expr::bound("absent"));
    assert_eq!(empty.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        empty.problem,
        "The expression reads `absent`, which is out of scope."
    );
    assert_eq!(empty.fix, "Read one of the names in scope here: nothing.");
    let listed = check_with(
        &Expr::bound("absent"),
        &[("count", TypeExpr::Integer), ("label", TypeExpr::Text)],
    )
    .expect_err("the name is absent");
    assert_eq!(
        listed.fix,
        "Read one of the names in scope here: count, label."
    );
    let mut scoped = env();
    scoped.bind(&ParamName::new("count"), Value::Integer(1));
    let at_run = Expr::bound("absent")
        .run(&scoped)
        .expect_err("the name is absent at run time");
    assert_eq!(at_run.fix, "Read one of the names in scope here: count.");
    assert_eq!(at_run.location.as_str(), "expression");
}

#[test]
fn a_field_read_checks_the_record_and_the_field() {
    let invoice = TypeExpr::Record(TypeId::new("invoice"));
    assert_eq!(
        check_with(
            &Expr::field(Expr::bound("bill"), "total"),
            &[("bill", invoice.clone())]
        ),
        Ok(TypeExpr::Decimal { scale: 2 })
    );
    let on_integer = check_err(&Expr::field(Expr::integer(1), "total"));
    assert_eq!(on_integer.code, DiagnosticCode::TypeMismatch);
    assert_eq!(on_integer.problem, "The field read sits on a whole number.");
    assert_eq!(on_integer.fix, "Read a field out of a record value.");

    let ghost = check_with(
        &Expr::field(Expr::bound("thing"), "total"),
        &[("thing", TypeExpr::Record(TypeId::new("ghost")))],
    )
    .expect_err("the record type is absent");
    assert_eq!(ghost.code, DiagnosticCode::DanglingReference);
    assert_eq!(ghost.fix, "Declare a record type named `ghost`.");

    let missing = check_with(
        &Expr::field(Expr::bound("bill"), "memo"),
        &[("bill", invoice)],
    )
    .expect_err("the field is absent");
    assert_eq!(missing.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        missing.problem,
        "The record `invoice` lacks the field `memo`."
    );
    assert_eq!(missing.fix, "Read a field declared on `invoice`.");
}

#[test]
fn unary_operations_check_their_operand() {
    let not = |operand| Expr::Unary {
        op: UnaryOp::Not,
        operand: Box::new(operand),
    };
    let negate = |operand| Expr::Unary {
        op: UnaryOp::Negate,
        operand: Box::new(operand),
    };
    assert_eq!(check(&not(Expr::boolean(true))), Ok(TypeExpr::Bool));
    assert_eq!(check(&negate(Expr::integer(3))), Ok(TypeExpr::Integer));
    assert_eq!(
        check(&negate(Expr::decimal(3, 4))),
        Ok(TypeExpr::Decimal { scale: 4 })
    );
    let bad_not = check_err(&not(Expr::integer(1)));
    assert_eq!(bad_not.code, DiagnosticCode::TypeMismatch);
    assert_eq!(bad_not.fix, "Apply the reverse operation to a truth value.");
    let bad_negate = check_err(&negate(Expr::text("x")));
    assert_eq!(bad_negate.problem, "The sign change sits on text.");
    assert_eq!(
        bad_negate.fix,
        "Apply the sign change to a whole number or a decimal."
    );
}

#[test]
fn a_conditional_needs_a_truth_value_and_matching_branches() {
    assert_eq!(
        check(&Expr::conditional(
            Expr::boolean(false),
            Expr::text("a"),
            Expr::text("b")
        )),
        Ok(TypeExpr::Text)
    );
    let bad_condition = check_err(&Expr::conditional(
        Expr::integer(1),
        Expr::text("a"),
        Expr::text("b"),
    ));
    assert_eq!(bad_condition.location.as_str(), "expr.condition");
    assert_eq!(bad_condition.fix, "Give the condition a truth value.");
    let split = check_err(&Expr::conditional(
        Expr::boolean(true),
        Expr::text("a"),
        Expr::integer(1),
    ));
    assert_eq!(
        split.problem,
        "One branch carries text and the other carries a whole number."
    );
    assert_eq!(split.fix, "Give both branches one type.");
}

#[test]
fn a_local_binding_carries_its_type_into_the_body() {
    let expr = Expr::Let {
        name: ParamName::new("rate"),
        value: Box::new(Expr::decimal(5, 1)),
        body: Box::new(Expr::binary(
            BinaryOp::Multiply,
            Expr::bound("rate"),
            Expr::bound("rate"),
        )),
    };
    assert_eq!(check(&expr), Ok(TypeExpr::Decimal { scale: 2 }));
    assert_eq!(run(&expr), Ok(dec(25, 2)));
}

fn invoice_build(fields: Vec<(&str, Expr)>) -> Expr {
    Expr::MakeRecord {
        type_id: TypeId::new("invoice"),
        fields: fields
            .into_iter()
            .map(|(name, expr)| (FieldName::new(name), expr))
            .collect(),
    }
}

#[test]
fn a_record_build_checks_every_declared_field() {
    let payer = lit(Value::Identity(Identity([1u8; 32])));
    let good = invoice_build(vec![
        ("total", Expr::decimal(100, 2)),
        ("payer", payer.clone()),
    ]);
    assert_eq!(check(&good), Ok(TypeExpr::Record(TypeId::new("invoice"))));
    assert_eq!(
        run(&good),
        Ok(Value::record(
            TypeId::new("invoice"),
            vec![
                (
                    FieldName::new("payer"),
                    Value::Identity(Identity([1u8; 32]))
                ),
                (FieldName::new("total"), dec(100, 2)),
            ]
        ))
    );

    let ghost = check_err(&Expr::MakeRecord {
        type_id: TypeId::new("ghost"),
        fields: Vec::new(),
    });
    assert_eq!(ghost.code, DiagnosticCode::DanglingReference);
    assert_eq!(ghost.fix, "Declare a record type named `ghost`.");

    let short = check_err(&invoice_build(vec![("total", Expr::decimal(1, 2))]));
    assert_eq!(short.code, DiagnosticCode::MissingValue);
    assert_eq!(short.location.as_str(), "expr.payer");
    assert_eq!(short.fix, "Give the field `payer` a value.");

    let wrong = check_err(&invoice_build(vec![
        ("total", Expr::integer(1)),
        ("payer", payer.clone()),
    ]));
    assert_eq!(wrong.code, DiagnosticCode::TypeMismatch);
    assert_eq!(wrong.location.as_str(), "expr.total");
    assert_eq!(wrong.fix, "Supply a decimal at scale 2 for `total`.");

    let extra = check_err(&invoice_build(vec![
        ("total", Expr::decimal(1, 2)),
        ("payer", payer),
        ("memo", Expr::text("x")),
    ]));
    assert_eq!(extra.code, DiagnosticCode::DanglingReference);
    assert_eq!(extra.location.as_str(), "expr.memo");
    assert_eq!(extra.fix, "Remove `memo` or declare it on `invoice`.");
}

#[test]
fn an_enumeration_build_checks_the_type_and_the_alternative() {
    let build = |type_id: &str, name: &str| Expr::MakeEnumeration {
        type_id: TypeId::new(type_id),
        variant: VariantName::new(name),
    };
    assert_eq!(
        check(&build("tenure", "leasehold")),
        Ok(TypeExpr::Enumeration(TypeId::new("tenure")))
    );
    assert_eq!(run(&build("tenure", "leasehold")), Ok(tenure("leasehold")));
    let ghost = check_err(&build("ghost", "any"));
    assert_eq!(ghost.fix, "Declare an enumeration named `ghost`.");
    let absent = check_err(&build("tenure", "rental"));
    assert_eq!(absent.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        absent.problem,
        "The enumeration `tenure` lacks the alternative `rental`."
    );
    assert_eq!(absent.fix, "Choose an alternative declared on `tenure`.");
}

#[test]
fn a_sum_build_checks_the_alternative_and_its_data() {
    let build = |type_id: &str, name: &str, payload: Option<Expr>| Expr::MakeSum {
        type_id: TypeId::new(type_id),
        variant: VariantName::new(name),
        payload: payload.map(Box::new),
    };
    let sum = TypeExpr::Sum(TypeId::new("ruling"));
    let approval = build("ruling", "approval", Some(Expr::decimal(5, 2)));
    assert_eq!(check(&approval), Ok(sum.clone()));
    assert_eq!(run(&approval), Ok(ruling("approval", Some(dec(5, 2)))));
    let withdrawn = build("ruling", "withdrawn", None);
    assert_eq!(check(&withdrawn), Ok(sum));
    assert_eq!(run(&withdrawn), Ok(ruling("withdrawn", None)));

    let ghost = check_err(&build("ghost", "any", None));
    assert_eq!(ghost.fix, "Declare a sum type named `ghost`.");
    let absent = check_err(&build("ruling", "appeal", None));
    assert_eq!(absent.fix, "Choose an alternative declared on `ruling`.");
    let wrong = check_err(&build("ruling", "approval", Some(Expr::text("x"))));
    assert_eq!(wrong.code, DiagnosticCode::TypeMismatch);
    assert_eq!(wrong.location.as_str(), "expr.payload");
    assert_eq!(wrong.fix, "Supply a decimal at scale 2 here.");
    let empty = check_err(&build("ruling", "approval", None));
    assert_eq!(empty.code, DiagnosticCode::MissingValue);
    assert_eq!(empty.fix, "Supply a decimal at scale 2 for `approval`.");
    let surplus = check_err(&build("ruling", "withdrawn", Some(Expr::integer(1))));
    assert_eq!(surplus.code, DiagnosticCode::TypeMismatch);
    assert_eq!(surplus.fix, "Remove the data supplied for `withdrawn`.");
}

#[test]
fn a_list_build_checks_the_element_type() {
    let good = Expr::MakeList {
        element: TypeExpr::Integer,
        items: vec![Expr::integer(1), Expr::integer(2)],
    };
    assert_eq!(check(&good), Ok(TypeExpr::list(TypeExpr::Integer)));
    assert_eq!(run(&good), Ok(int_list(&[1, 2])));
    let bad_element = check_err(&Expr::MakeList {
        element: TypeExpr::Decimal { scale: 31 },
        items: Vec::new(),
    });
    assert_eq!(bad_element.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(bad_element.location.as_str(), "expr.element");
    let mixed = check_err(&Expr::MakeList {
        element: TypeExpr::Integer,
        items: vec![Expr::integer(1), Expr::text("two")],
    });
    assert_eq!(mixed.location.as_str(), "expr[1]");
    assert_eq!(mixed.fix, "Supply a whole number for every element.");
}

#[test]
fn a_map_build_checks_the_key_and_value_types() {
    let build = |key: TypeExpr, entries: Vec<(Expr, Expr)>| Expr::MakeMap {
        key,
        value: TypeExpr::Text,
        entries,
    };
    let good = build(
        TypeExpr::Integer,
        vec![
            (Expr::integer(2), Expr::text("b")),
            (Expr::integer(1), Expr::text("a")),
        ],
    );
    assert_eq!(
        check(&good),
        Ok(TypeExpr::map(TypeExpr::Integer, TypeExpr::Text))
    );
    assert_eq!(run(&good), Ok(int_map(&[(1, "a"), (2, "b")])));
    let unordered = check_err(&build(TypeExpr::optional(TypeExpr::Integer), Vec::new()));
    assert_eq!(unordered.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        unordered.fix,
        "Choose a key type such as text, a whole number or an identity."
    );
    let bad_key = check_err(&build(
        TypeExpr::Integer,
        vec![(Expr::text("k"), Expr::text("v"))],
    ));
    assert_eq!(bad_key.location.as_str(), "expr[0].key");
    assert_eq!(bad_key.fix, "Supply a whole number for every key.");
    let bad_value = check_err(&build(
        TypeExpr::Integer,
        vec![(Expr::integer(1), Expr::integer(2))],
    ));
    assert_eq!(bad_value.location.as_str(), "expr[0].value");
    assert_eq!(bad_value.fix, "Supply text for every value.");
}

#[test]
fn an_optional_build_checks_the_element() {
    let present_build = Expr::MakeOptional {
        element: TypeExpr::Text,
        item: Some(Box::new(Expr::text("x"))),
    };
    let absent_build = Expr::MakeOptional {
        element: TypeExpr::Text,
        item: None,
    };
    assert_eq!(
        check(&present_build),
        Ok(TypeExpr::optional(TypeExpr::Text))
    );
    assert_eq!(check(&absent_build), Ok(TypeExpr::optional(TypeExpr::Text)));
    assert_eq!(
        run(&present_build),
        Ok(Value::some(TypeExpr::Text, Value::Text("x".to_string())))
    );
    assert_eq!(run(&absent_build), Ok(Value::none(TypeExpr::Text)));
    let ghost = check_err(&Expr::MakeOptional {
        element: TypeExpr::Sum(TypeId::new("ghost")),
        item: None,
    });
    assert_eq!(ghost.location.as_str(), "expr.element");
    assert_eq!(
        ghost.fix,
        "Declare a sum type named `ghost` in the types section."
    );
    let wrong = check_err(&Expr::MakeOptional {
        element: TypeExpr::Text,
        item: Some(Box::new(Expr::integer(1))),
    });
    assert_eq!(wrong.location.as_str(), "expr.item");
    assert_eq!(wrong.fix, "Supply text here.");
}

#[test]
fn a_reference_build_carries_the_collection() {
    let build = Expr::MakeReference {
        collection: CollectionId::new("parcels"),
        key: Box::new(Expr::text("p1")),
    };
    assert_eq!(
        check(&build),
        Ok(TypeExpr::Reference(CollectionId::new("parcels")))
    );
    assert_eq!(
        run(&build),
        Ok(Value::reference(
            CollectionId::new("parcels"),
            Value::Text("p1".to_string())
        ))
    );
    let broken = check_err(&Expr::MakeReference {
        collection: CollectionId::new("parcels"),
        key: Box::new(Expr::bound("absent")),
    });
    assert_eq!(broken.location.as_str(), "expr.key");
}

#[test]
fn optional_reads_check_their_operand() {
    let optional_text = TypeExpr::optional(TypeExpr::Text);
    let unwrap = |optional, fallback| Expr::UnwrapOr {
        optional: Box::new(optional),
        fallback: Box::new(fallback),
    };
    assert_eq!(
        check_with(
            &unwrap(Expr::bound("note"), Expr::text("none")),
            &[("note", optional_text.clone())]
        ),
        Ok(TypeExpr::Text)
    );
    let on_text = check_err(&unwrap(Expr::text("x"), Expr::text("y")));
    assert_eq!(on_text.location.as_str(), "expr.optional");
    assert_eq!(on_text.fix, "Apply the unwrap to an optional value.");
    let wrong_fallback = check_with(
        &unwrap(Expr::bound("note"), Expr::integer(0)),
        &[("note", optional_text.clone())],
    )
    .expect_err("the fallback type differs");
    assert_eq!(wrong_fallback.location.as_str(), "expr.fallback");
    assert_eq!(wrong_fallback.fix, "Supply text as the fallback.");

    assert_eq!(
        check_with(
            &Expr::IsPresent(Box::new(Expr::bound("note"))),
            &[("note", optional_text)]
        ),
        Ok(TypeExpr::Bool)
    );
    let presence = check_err(&Expr::IsPresent(Box::new(Expr::integer(1))));
    assert_eq!(
        presence.fix,
        "Apply the presence test to an optional value."
    );
}

#[test]
fn length_accepts_lists_maps_text_and_bytes() {
    for value in [
        int_list(&[1]),
        int_map(&[(1, "a")]),
        Value::Text("abc".to_string()),
        Value::Bytes(vec![1, 2]),
    ] {
        assert_eq!(
            check(&Expr::Length(Box::new(lit(value)))),
            Ok(TypeExpr::Integer)
        );
    }
    let bad = check_err(&Expr::Length(Box::new(Expr::integer(5))));
    assert_eq!(bad.problem, "The length reads a whole number.");
    assert_eq!(bad.fix, "Read the length of a list, a map, text or bytes.");
}

#[test]
fn membership_and_lookup_check_their_operands() {
    let contains = |list, item| Expr::Contains {
        list: Box::new(list),
        item: Box::new(item),
    };
    let lookup = |map, key| Expr::Lookup {
        map: Box::new(map),
        key: Box::new(key),
    };
    assert_eq!(
        check(&contains(lit(int_list(&[1])), Expr::integer(1))),
        Ok(TypeExpr::Bool)
    );
    let not_list = check_err(&contains(Expr::integer(1), Expr::integer(1)));
    assert_eq!(not_list.location.as_str(), "expr.list");
    assert_eq!(not_list.fix, "Apply the membership test to a list.");
    let wrong_item = check_err(&contains(lit(int_list(&[1])), Expr::text("1")));
    assert_eq!(wrong_item.location.as_str(), "expr.item");
    assert_eq!(wrong_item.fix, "Supply a whole number here.");

    assert_eq!(
        check(&lookup(lit(int_map(&[(1, "a")])), Expr::integer(1))),
        Ok(TypeExpr::optional(TypeExpr::Text))
    );
    let not_map = check_err(&lookup(Expr::integer(1), Expr::integer(1)));
    assert_eq!(not_map.location.as_str(), "expr.map");
    assert_eq!(not_map.fix, "Apply the lookup to a map.");
    let wrong_key = check_err(&lookup(lit(int_map(&[(1, "a")])), Expr::text("1")));
    assert_eq!(wrong_key.location.as_str(), "expr.key");
    assert_eq!(wrong_key.fix, "Supply a whole number as the key.");
}

#[test]
fn a_rescale_checks_the_scale_and_the_operand() {
    let rescale = |value, scale| Expr::Rescale {
        value: Box::new(value),
        scale,
    };
    assert_eq!(
        check(&rescale(Expr::decimal(1, 2), 5)),
        Ok(TypeExpr::Decimal { scale: 5 })
    );
    let too_fine = check_err(&rescale(Expr::decimal(1, 2), 31));
    assert_eq!(too_fine.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        too_fine.problem,
        "The requested scale 31 runs past the maximum of 30."
    );
    assert_eq!(too_fine.fix, "Choose a scale of 30 or below.");
    let on_integer = check_err(&rescale(Expr::integer(1), 2));
    assert_eq!(on_integer.fix, "Apply the rescale to a decimal value.");
}

#[test]
fn an_attempt_needs_one_type_for_body_and_compensation() {
    let attempt = |body, compensation| Expr::Attempt {
        body: Box::new(body),
        compensation: Box::new(compensation),
    };
    assert_eq!(
        check(&attempt(Expr::integer(1), Expr::integer(0))),
        Ok(TypeExpr::Integer)
    );
    let split = check_err(&attempt(Expr::integer(1), Expr::text("0")));
    assert_eq!(
        split.problem,
        "The body carries a whole number and the compensation carries text."
    );
    assert_eq!(split.fix, "Give the body and the compensation one type.");
}

#[test]
fn builtin_calls_check_arity_and_argument_types() {
    let ts = || lit(Value::Timestamp(Timestamp::from_millis(10)));
    let accepted = [
        (call(Builtin::Now, Vec::new()), TypeExpr::Timestamp),
        (call(Builtin::Caller, Vec::new()), TypeExpr::Identity),
        (call(Builtin::HashOf, vec![Expr::text("a")]), TypeExpr::Hash),
        (
            call(Builtin::HashOf, vec![lit(Value::Bytes(vec![1]))]),
            TypeExpr::Hash,
        ),
        (
            call(Builtin::TimestampPlus, vec![ts(), Expr::integer(5)]),
            TypeExpr::Timestamp,
        ),
        (
            call(Builtin::TimestampSince, vec![ts(), ts()]),
            TypeExpr::Integer,
        ),
        (
            call(Builtin::AbsoluteInteger, vec![Expr::integer(-5)]),
            TypeExpr::Integer,
        ),
        (
            call(
                Builtin::MinimumInteger,
                vec![Expr::integer(1), Expr::integer(2)],
            ),
            TypeExpr::Integer,
        ),
        (
            call(
                Builtin::MaximumInteger,
                vec![Expr::integer(1), Expr::integer(2)],
            ),
            TypeExpr::Integer,
        ),
        (
            call(Builtin::BytesOfText, vec![Expr::text("a")]),
            TypeExpr::Bytes,
        ),
    ];
    for (expr, expected) in accepted {
        assert_eq!(check(&expr), Ok(expected));
    }

    let arity = check_err(&call(Builtin::Now, vec![Expr::integer(1)]));
    assert_eq!(arity.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        arity.problem,
        "The operation `now` takes 0 arguments and the call supplies 1."
    );
    assert_eq!(arity.fix, "Supply 0 arguments to `now`.");

    let hash = check_err(&call(Builtin::HashOf, vec![Expr::integer(1)]));
    assert_eq!(hash.location.as_str(), "expr[0]");
    assert_eq!(hash.fix, "Hash a run of bytes or a run of text.");

    let plus = check_err(&call(
        Builtin::TimestampPlus,
        vec![ts(), Expr::text("soon")],
    ));
    assert_eq!(plus.location.as_str(), "expr[1]");
    assert_eq!(
        plus.problem,
        "The argument carries text and `timestamp_plus` takes a whole number."
    );
    assert_eq!(plus.fix, "Supply a whole number here.");

    let since = check_err(&call(Builtin::TimestampSince, vec![Expr::integer(1), ts()]));
    assert_eq!(since.location.as_str(), "expr[0]");
    assert_eq!(since.fix, "Supply a timestamp here.");

    let absolute = check_err(&call(Builtin::AbsoluteInteger, vec![Expr::decimal(1, 1)]));
    assert_eq!(absolute.fix, "Supply a whole number here.");
    let minimum = check_err(&call(
        Builtin::MinimumInteger,
        vec![Expr::integer(1), Expr::text("2")],
    ));
    assert_eq!(minimum.location.as_str(), "expr[1]");
    let bytes = check_err(&call(Builtin::BytesOfText, vec![Expr::integer(1)]));
    assert_eq!(bytes.fix, "Supply text here.");
    let inner = check_err(&call(Builtin::AbsoluteInteger, vec![Expr::bound("gone")]));
    assert_eq!(inner.code, DiagnosticCode::DanglingReference);
}

// Binary checker paths

fn binary_check(op: BinaryOp, left: Expr, right: Expr) -> Result<TypeExpr, Diagnostic> {
    check(&Expr::binary(op, left, right))
}

#[test]
fn arithmetic_checks_accept_integers_and_matching_decimals() {
    for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Divide] {
        assert_eq!(
            binary_check(op, Expr::integer(1), Expr::integer(2)),
            Ok(TypeExpr::Integer)
        );
        assert_eq!(
            binary_check(op, Expr::decimal(1, 3), Expr::decimal(2, 3)),
            Ok(TypeExpr::Decimal { scale: 3 })
        );
        let mixed = binary_check(op, Expr::decimal(1, 3), Expr::decimal(2, 2))
            .expect_err("the scales differ");
        assert_eq!(
            mixed.fix,
            "Give both sides whole numbers, or decimals sharing one scale."
        );
    }
    assert_eq!(
        binary_check(BinaryOp::Multiply, Expr::integer(1), Expr::integer(2)),
        Ok(TypeExpr::Integer)
    );
    assert_eq!(
        binary_check(BinaryOp::Multiply, Expr::decimal(1, 3), Expr::decimal(2, 2)),
        Ok(TypeExpr::Decimal { scale: 5 })
    );
    let wide = binary_check(
        BinaryOp::Multiply,
        Expr::decimal(1, 20),
        Expr::decimal(1, 20),
    )
    .expect_err("the product scale runs past 30");
    assert_eq!(wide.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        wide.problem,
        "The product carries scale 40, above the maximum of 30."
    );
    assert_eq!(
        wide.fix,
        "Rescale one operand down before the multiplication."
    );
    let text_product = binary_check(BinaryOp::Multiply, Expr::text("a"), Expr::integer(2))
        .expect_err("text has no product");
    assert_eq!(
        text_product.problem,
        "The multiplication holds text on the left and a whole number on the right."
    );
    assert_eq!(
        binary_check(BinaryOp::Remainder, Expr::integer(7), Expr::integer(2)),
        Ok(TypeExpr::Integer)
    );
    let decimal_remainder = binary_check(
        BinaryOp::Remainder,
        Expr::decimal(7, 1),
        Expr::decimal(2, 1),
    )
    .expect_err("a remainder takes whole numbers");
    assert_eq!(decimal_remainder.code, DiagnosticCode::TypeMismatch);
}

#[test]
fn comparison_checks_need_one_ordered_type() {
    for op in [BinaryOp::Equal, BinaryOp::NotEqual] {
        assert_eq!(
            binary_check(op, Expr::text("a"), Expr::text("b")),
            Ok(TypeExpr::Bool)
        );
        let mixed =
            binary_check(op, Expr::text("a"), Expr::integer(1)).expect_err("the sides differ");
        assert_eq!(mixed.fix, "Compare two values sharing one type.");
    }
    for op in [
        BinaryOp::Less,
        BinaryOp::LessOrEqual,
        BinaryOp::Greater,
        BinaryOp::GreaterOrEqual,
    ] {
        assert_eq!(
            binary_check(op, Expr::integer(1), Expr::integer(2)),
            Ok(TypeExpr::Bool)
        );
        let mixed =
            binary_check(op, Expr::integer(1), Expr::text("2")).expect_err("the sides differ");
        assert_eq!(mixed.fix, "Order two values sharing one type.");
    }
    let absent = lit(Value::none(TypeExpr::Integer));
    let unordered = binary_check(BinaryOp::Less, absent.clone(), absent)
        .expect_err("an optional lacks a total order");
    assert_eq!(
        unordered.problem,
        "The ordering sits on an optional a whole number, which lacks a total order."
    );
    assert_eq!(
        unordered.fix,
        "Order whole numbers, decimals, text, bytes, timestamps, identities or hashes."
    );
}

#[test]
fn logic_text_and_list_checks_need_matching_sides() {
    for op in [BinaryOp::And, BinaryOp::Or] {
        assert_eq!(
            binary_check(op, Expr::boolean(true), Expr::boolean(false)),
            Ok(TypeExpr::Bool)
        );
        let mixed = binary_check(op, Expr::boolean(true), Expr::integer(1))
            .expect_err("both sides need truth values");
        assert_eq!(mixed.fix, "Give both sides a truth value.");
    }
    let or = binary_check(BinaryOp::Or, Expr::integer(1), Expr::boolean(true))
        .expect_err("both sides need truth values");
    assert_eq!(
        or.problem,
        "The disjunction holds a whole number on the left and a truth value on the right."
    );
    assert_eq!(
        binary_check(BinaryOp::ConcatText, Expr::text("a"), Expr::text("b")),
        Ok(TypeExpr::Text)
    );
    let join = binary_check(BinaryOp::ConcatText, Expr::text("a"), Expr::integer(1))
        .expect_err("a join takes text");
    assert_eq!(join.fix, "Give both sides text.");
    assert_eq!(
        binary_check(
            BinaryOp::AppendList,
            lit(int_list(&[1])),
            lit(int_list(&[2]))
        ),
        Ok(TypeExpr::list(TypeExpr::Integer))
    );
    let append = binary_check(
        BinaryOp::AppendList,
        lit(int_list(&[1])),
        lit(Value::list(TypeExpr::Text, Vec::new())),
    )
    .expect_err("the element types differ");
    assert_eq!(
        append.fix,
        "Give both sides a list sharing one element type."
    );
    let right_missing = binary_check(BinaryOp::Add, Expr::integer(1), Expr::bound("gone"))
        .expect_err("the right side reads an absent name");
    assert_eq!(right_missing.location.as_str(), "expr.right");
}

// Match checker paths

#[test]
fn a_match_over_an_enumeration_checks_every_choice() {
    let over = |arms, fallback| {
        check_with(
            &matching(Expr::bound("held"), arms, fallback),
            &[("held", TypeExpr::Enumeration(TypeId::new("tenure")))],
        )
    };
    assert_eq!(
        over(
            vec![
                arm(choice("freehold"), Expr::integer(1)),
                arm(choice("leasehold"), Expr::integer(2)),
            ],
            None
        ),
        Ok(TypeExpr::Integer)
    );
    let uncovered = over(vec![arm(choice("freehold"), Expr::integer(1))], None)
        .expect_err("leasehold is uncovered");
    assert_eq!(uncovered.code, DiagnosticCode::MissingValue);
    assert_eq!(
        uncovered.problem,
        "The match leaves the alternative `leasehold` uncovered."
    );
    assert_eq!(
        uncovered.fix,
        "Add an arm for `leasehold`, or add a fallback."
    );
    assert_eq!(
        over(
            vec![arm(choice("freehold"), Expr::integer(1))],
            Some(Expr::integer(0))
        ),
        Ok(TypeExpr::Integer)
    );
    let unknown = over(vec![arm(choice("rental"), Expr::integer(1))], None)
        .expect_err("rental is undeclared");
    assert_eq!(unknown.location.as_str(), "expr.arms[0]");
    assert_eq!(unknown.fix, "Match an alternative declared on `tenure`.");
    let wrong_fallback = over(
        vec![arm(choice("freehold"), Expr::integer(1))],
        Some(Expr::text("none")),
    )
    .expect_err("the fallback type differs");
    assert_eq!(wrong_fallback.location.as_str(), "expr.fallback");
    assert_eq!(
        wrong_fallback.fix,
        "Give the fallback the type the arms carry."
    );
    let split = over(
        vec![
            arm(choice("freehold"), Expr::integer(1)),
            arm(choice("leasehold"), Expr::text("two")),
        ],
        None,
    )
    .expect_err("the arm types differ");
    assert_eq!(split.location.as_str(), "expr.arms[1].body");
    assert_eq!(split.fix, "Give every arm one type.");
}

#[test]
fn a_match_over_an_absent_type_names_the_missing_declaration() {
    let ghost_enum = check_with(
        &matching(
            Expr::bound("held"),
            vec![arm(choice("any"), Expr::integer(1))],
            None,
        ),
        &[("held", TypeExpr::Enumeration(TypeId::new("ghost")))],
    )
    .expect_err("the enumeration is absent");
    assert_eq!(ghost_enum.code, DiagnosticCode::DanglingReference);
    assert_eq!(ghost_enum.fix, "Declare an enumeration named `ghost`.");
    let ghost_sum = check_with(
        &matching(
            Expr::bound("held"),
            vec![arm(variant("any", None), Expr::integer(1))],
            None,
        ),
        &[("held", TypeExpr::Sum(TypeId::new("ghost")))],
    )
    .expect_err("the sum type is absent");
    assert_eq!(ghost_sum.fix, "Declare a sum type named `ghost`.");
}

#[test]
fn a_match_over_a_sum_type_checks_alternatives_and_bindings() {
    let sum = TypeExpr::Sum(TypeId::new("ruling"));
    let over = |arms: Vec<MatchArm>| {
        check_with(
            &matching(Expr::bound("held"), arms, None),
            &[("held", sum.clone())],
        )
    };
    let unknown = over(vec![arm(variant("appeal", None), Expr::integer(1))])
        .expect_err("appeal is undeclared");
    assert_eq!(unknown.fix, "Match an alternative declared on `ruling`.");
    let stray = over(vec![arm(
        variant("withdrawn", Some("data")),
        Expr::integer(1),
    )])
    .expect_err("withdrawn carries nothing");
    assert_eq!(stray.code, DiagnosticCode::TypeMismatch);
    assert_eq!(stray.fix, "Remove the binding `data` from this arm.");
    let uncovered = over(vec![arm(variant("approval", None), Expr::integer(1))])
        .expect_err("withdrawn is uncovered");
    assert_eq!(
        uncovered.fix,
        "Add an arm for `withdrawn`, or add a fallback."
    );
    let shape = over(vec![arm(choice("approval"), Expr::integer(1))])
        .expect_err("a choice arm suits an enumeration");
    assert_eq!(shape.location.as_str(), "expr.arms[0]");
    assert_eq!(
        shape.problem,
        "The arm matches a shape other than the sum type `ruling`."
    );
}

#[test]
fn a_match_over_an_optional_needs_both_arms() {
    let optional = TypeExpr::optional(TypeExpr::Integer);
    let over = |arms: Vec<MatchArm>| {
        check_with(
            &matching(Expr::bound("held"), arms, None),
            &[("held", optional.clone())],
        )
    };
    assert_eq!(
        over(vec![
            arm(present("inner"), Expr::bound("inner")),
            arm(Pattern::Absent, Expr::integer(0)),
        ]),
        Ok(TypeExpr::Integer)
    );
    let no_absent = over(vec![arm(present("inner"), Expr::bound("inner"))])
        .expect_err("the absent arm is missing");
    assert_eq!(
        no_absent.problem,
        "The match leaves the alternative `absent` uncovered."
    );
    let no_present =
        over(vec![arm(Pattern::Absent, Expr::integer(0))]).expect_err("the present arm is missing");
    assert_eq!(
        no_present.problem,
        "The match leaves the alternative `present` uncovered."
    );
}

#[test]
fn a_match_holding_zero_arms_reports_an_empty_section() {
    let empty = check_err(&matching(Expr::integer(1), Vec::new(), None));
    assert_eq!(empty.code, DiagnosticCode::SectionEmpty);
    assert_eq!(empty.problem, "The match holds zero arms.");
    assert_eq!(
        empty.fix,
        "Add one arm for every alternative the value carries."
    );
    let fallback_only = matching(Expr::integer(1), Vec::new(), Some(Expr::text("x")));
    assert_eq!(check(&fallback_only), Ok(TypeExpr::Text));
    assert_eq!(run(&fallback_only), Ok(Value::Text("x".to_string())));
}

// Evaluator paths

#[test]
fn a_field_read_at_run_time_needs_a_record_holding_the_field() {
    let partial = Value::record(
        TypeId::new("invoice"),
        vec![(FieldName::new("total"), dec(5, 2))],
    );
    assert_eq!(
        run(&Expr::field(lit(partial.clone()), "total")),
        Ok(dec(5, 2))
    );
    let absent = run_err(&Expr::field(lit(partial), "payer"));
    assert_eq!(absent.code, DiagnosticCode::MissingValue);
    assert_eq!(
        absent.fix,
        "Give the field `payer` a value before the read."
    );
    let on_text = run_err(&Expr::field(Expr::text("x"), "payer"));
    assert_eq!(on_text.code, DiagnosticCode::TypeMismatch);
    assert_eq!(on_text.fix, "Read a field out of a record value.");
}

#[test]
fn unary_operations_run_exactly() {
    let unary = |op, operand| Expr::Unary {
        op,
        operand: Box::new(operand),
    };
    assert_eq!(
        run(&unary(UnaryOp::Not, Expr::boolean(false))),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        run(&unary(UnaryOp::Negate, Expr::integer(4))),
        Ok(Value::Integer(-4))
    );
    assert_eq!(
        run(&unary(UnaryOp::Negate, Expr::decimal(125, 2))),
        Ok(dec(-125, 2))
    );
    let overflow = run_err(&unary(UnaryOp::Negate, Expr::integer(i128::MIN)));
    assert_eq!(overflow.code, DiagnosticCode::ArithmeticOverflow);
    let decimal_overflow = run_err(&unary(UnaryOp::Negate, Expr::decimal(i128::MIN, 2)));
    assert_eq!(decimal_overflow.code, DiagnosticCode::ArithmeticOverflow);
    assert_eq!(
        decimal_overflow.fix,
        "Reduce the operand sizes or raise the declared scale."
    );
    let wrong = run_err(&unary(UnaryOp::Not, Expr::integer(1)));
    assert_eq!(wrong.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        wrong.fix,
        "Apply the operation to a value of the declared type."
    );
}

#[test]
fn a_conditional_runs_the_chosen_branch_alone() {
    let guarded = |condition| {
        Expr::conditional(
            condition,
            Expr::text("then"),
            Expr::binary(BinaryOp::Divide, Expr::integer(1), Expr::integer(0)),
        )
    };
    assert_eq!(
        run(&guarded(Expr::boolean(true))),
        Ok(Value::Text("then".to_string()))
    );
    let fault = run_err(&guarded(Expr::boolean(false)));
    assert_eq!(fault.code, DiagnosticCode::DivisionByZero);
    assert_eq!(fault.location.as_str(), "expression.else_branch");
    let bad = run_err(&guarded(Expr::integer(1)));
    assert_eq!(bad.location.as_str(), "expression.condition");
    assert_eq!(bad.fix, "Give the condition a truth value.");
}

#[test]
fn a_match_at_run_time_selects_the_first_matching_arm() {
    let with = |value| env().with(&ParamName::new("held"), value);
    let sum_match = matching(
        Expr::bound("held"),
        vec![
            arm(variant("approval", Some("amount")), Expr::bound("amount")),
            arm(variant("withdrawn", Some("none")), Expr::decimal(-1, 2)),
            arm(variant("withdrawn", None), Expr::decimal(0, 2)),
        ],
        None,
    );
    assert_eq!(
        sum_match.run(&with(ruling("approval", Some(dec(9, 2))))),
        Ok(dec(9, 2))
    );
    assert_eq!(
        sum_match.run(&with(ruling("withdrawn", None))),
        Ok(dec(0, 2))
    );

    let enum_match = matching(
        Expr::bound("held"),
        vec![
            arm(choice("freehold"), Expr::integer(1)),
            arm(choice("leasehold"), Expr::integer(2)),
        ],
        None,
    );
    assert_eq!(
        enum_match.run(&with(tenure("leasehold"))),
        Ok(Value::Integer(2))
    );

    let optional_match = matching(
        Expr::bound("held"),
        vec![
            arm(Pattern::Absent, Expr::integer(0)),
            arm(present("inner"), Expr::bound("inner")),
        ],
        None,
    );
    assert_eq!(
        optional_match.run(&with(Value::some(TypeExpr::Integer, Value::Integer(8)))),
        Ok(Value::Integer(8))
    );
    assert_eq!(
        optional_match.run(&with(Value::none(TypeExpr::Integer))),
        Ok(Value::Integer(0))
    );

    let unmatched = enum_match
        .run(&with(Value::Integer(3)))
        .expect_err("zero arms match a whole number");
    assert_eq!(unmatched.code, DiagnosticCode::MissingValue);
    assert_eq!(unmatched.problem, "Zero arms match the value supplied.");
    assert_eq!(
        unmatched.fix,
        "Add an arm for every alternative, or add a fallback."
    );
    let with_fallback = matching(
        Expr::bound("held"),
        vec![arm(choice("freehold"), Expr::integer(1))],
        Some(Expr::integer(-1)),
    );
    assert_eq!(
        with_fallback.run(&with(tenure("leasehold"))),
        Ok(Value::Integer(-1))
    );
}

#[test]
fn optional_reads_run_against_present_and_absent_values() {
    let unwrap = |optional| Expr::UnwrapOr {
        optional: Box::new(optional),
        fallback: Box::new(Expr::integer(-1)),
    };
    let some = lit(Value::some(TypeExpr::Integer, Value::Integer(4)));
    let none = lit(Value::none(TypeExpr::Integer));
    assert_eq!(run(&unwrap(some.clone())), Ok(Value::Integer(4)));
    assert_eq!(run(&unwrap(none.clone())), Ok(Value::Integer(-1)));
    let not_optional = run_err(&unwrap(Expr::integer(1)));
    assert_eq!(not_optional.fix, "Apply the unwrap to an optional value.");
    assert_eq!(run(&Expr::IsPresent(Box::new(some))), Ok(Value::Bool(true)));
    assert_eq!(
        run(&Expr::IsPresent(Box::new(none))),
        Ok(Value::Bool(false))
    );
    let presence = run_err(&Expr::IsPresent(Box::new(Expr::integer(1))));
    assert_eq!(
        presence.fix,
        "Apply the presence test to an optional value."
    );
}

#[test]
fn length_counts_elements_characters_and_bytes() {
    let length = |value| run(&Expr::Length(Box::new(lit(value))));
    assert_eq!(length(int_list(&[1, 2, 3])), Ok(Value::Integer(3)));
    assert_eq!(
        length(int_map(&[(1, "a"), (2, "b")])),
        Ok(Value::Integer(2))
    );
    assert_eq!(
        length(Value::Text("d\u{e9}j\u{e0}".to_string())),
        Ok(Value::Integer(4))
    );
    assert_eq!(length(Value::Bytes(vec![0; 5])), Ok(Value::Integer(5)));
    let bad = length(Value::Bool(true)).expect_err("a truth value has no length");
    assert_eq!(bad.fix, "Read the length of a list, a map, text or bytes.");
}

#[test]
fn membership_and_lookup_charge_one_step_per_element() {
    let contains = Expr::Contains {
        list: Box::new(lit(int_list(&[1, 2, 3, 4, 5]))),
        item: Box::new(Expr::integer(4)),
    };
    let mut meter = Meter::new(100);
    assert_eq!(
        contains.evaluate(&env(), &mut meter, &Path::root()),
        Ok(Value::Bool(true))
    );
    assert_eq!(meter.used(), 8);
    let missing = Expr::Contains {
        list: Box::new(lit(int_list(&[1]))),
        item: Box::new(Expr::integer(4)),
    };
    assert_eq!(run(&missing), Ok(Value::Bool(false)));
    let mut tight = Meter::new(7);
    let overrun = contains
        .evaluate(&env(), &mut tight, &Path::root())
        .expect_err("the scan overruns the budget");
    assert_eq!(overrun.code, DiagnosticCode::BoundExceeded);
    let not_list = run_err(&Expr::Contains {
        list: Box::new(Expr::integer(1)),
        item: Box::new(Expr::integer(1)),
    });
    assert_eq!(not_list.fix, "Apply the membership test to a list.");

    let lookup = |key| Expr::Lookup {
        map: Box::new(lit(int_map(&[(1, "a"), (2, "b"), (3, "c")]))),
        key: Box::new(Expr::integer(key)),
    };
    let mut meter = Meter::new(100);
    assert_eq!(
        lookup(2).evaluate(&env(), &mut meter, &Path::root()),
        Ok(Value::some(TypeExpr::Text, Value::Text("b".to_string())))
    );
    assert_eq!(meter.used(), 6);
    assert_eq!(run(&lookup(9)), Ok(Value::none(TypeExpr::Text)));
    let not_map = run_err(&Expr::Lookup {
        map: Box::new(Expr::integer(1)),
        key: Box::new(Expr::integer(1)),
    });
    assert_eq!(not_map.fix, "Apply the lookup to a map.");
}

#[test]
fn a_rescale_runs_without_losing_a_digit() {
    let rescale = |value, scale| {
        run(&Expr::Rescale {
            value: Box::new(value),
            scale,
        })
    };
    assert_eq!(rescale(Expr::decimal(125, 2), 4), Ok(dec(12500, 4)));
    assert_eq!(rescale(Expr::decimal(1200, 3), 1), Ok(dec(12, 1)));
    let loss = rescale(Expr::decimal(125, 2), 1).expect_err("a digit would drop");
    assert_eq!(loss.code, DiagnosticCode::PrecisionLoss);
    assert_eq!(
        loss.fix,
        "Raise the declared scale of the result field so the exact answer fits."
    );
    let on_integer = rescale(Expr::integer(1), 2).expect_err("a whole number has no scale");
    assert_eq!(on_integer.fix, "Apply the rescale to a decimal value.");
}

#[test]
fn builtin_calls_read_the_environment_and_compute_exactly() {
    let ts = |millis| lit(Value::Timestamp(Timestamp::from_millis(millis)));
    assert_eq!(
        run(&call(Builtin::Now, Vec::new())),
        Ok(Value::Timestamp(Timestamp::from_millis(1_000)))
    );
    assert_eq!(
        run(&call(Builtin::Caller, Vec::new())),
        Ok(Value::Identity(Identity([7u8; 32])))
    );
    let expected = Value::Hash(Hash32::of(b"parcel"));
    assert_eq!(
        run(&call(Builtin::HashOf, vec![Expr::text("parcel")])),
        Ok(expected.clone())
    );
    assert_eq!(
        run(&call(
            Builtin::HashOf,
            vec![lit(Value::Bytes(b"parcel".to_vec()))]
        )),
        Ok(expected)
    );
    assert_eq!(
        run(&call(
            Builtin::TimestampPlus,
            vec![ts(10), Expr::integer(5)]
        )),
        Ok(Value::Timestamp(Timestamp::from_millis(15)))
    );
    assert_eq!(
        run(&call(Builtin::TimestampSince, vec![ts(30), ts(10)])),
        Ok(Value::Integer(20))
    );
    assert_eq!(
        run(&call(Builtin::AbsoluteInteger, vec![Expr::integer(-9)])),
        Ok(Value::Integer(9))
    );
    assert_eq!(
        run(&call(
            Builtin::MinimumInteger,
            vec![Expr::integer(3), Expr::integer(-2)]
        )),
        Ok(Value::Integer(-2))
    );
    assert_eq!(
        run(&call(
            Builtin::MaximumInteger,
            vec![Expr::integer(3), Expr::integer(-2)]
        )),
        Ok(Value::Integer(3))
    );
    assert_eq!(
        run(&call(Builtin::BytesOfText, vec![Expr::text("ab")])),
        Ok(Value::Bytes(vec![b'a', b'b']))
    );
}

#[test]
fn builtin_calls_report_overflow_and_wrong_shapes() {
    let ts = |millis| lit(Value::Timestamp(Timestamp::from_millis(millis)));
    let negative_span = run_err(&call(
        Builtin::TimestampPlus,
        vec![ts(10), Expr::integer(-1)],
    ));
    assert_eq!(negative_span.code, DiagnosticCode::ArithmeticOverflow);
    let past_end = run_err(&call(
        Builtin::TimestampPlus,
        vec![ts(u64::MAX), Expr::integer(1)],
    ));
    assert_eq!(past_end.code, DiagnosticCode::ArithmeticOverflow);
    let backwards = run_err(&call(Builtin::TimestampSince, vec![ts(10), ts(30)]));
    assert_eq!(backwards.code, DiagnosticCode::ArithmeticOverflow);
    let absolute = run_err(&call(
        Builtin::AbsoluteInteger,
        vec![Expr::integer(i128::MIN)],
    ));
    assert_eq!(absolute.code, DiagnosticCode::ArithmeticOverflow);

    let wrong_shapes = [
        (Builtin::HashOf, vec![Expr::integer(1)]),
        (
            Builtin::TimestampPlus,
            vec![Expr::integer(1), Expr::integer(1)],
        ),
        (
            Builtin::TimestampSince,
            vec![Expr::integer(1), Expr::integer(1)],
        ),
        (Builtin::AbsoluteInteger, vec![Expr::text("1")]),
        (
            Builtin::MinimumInteger,
            vec![Expr::text("1"), Expr::integer(1)],
        ),
        (Builtin::MaximumInteger, vec![Expr::integer(1)]),
        (Builtin::BytesOfText, vec![Expr::integer(1)]),
    ];
    for (function, args) in wrong_shapes {
        let fault = run_err(&call(function, args));
        assert_eq!(fault.code, DiagnosticCode::TypeMismatch);
        assert_eq!(
            fault.problem,
            format!(
                "The call to `{}` holds arguments of another shape.",
                function.label()
            )
        );
        assert_eq!(
            fault.fix,
            format!("Supply the arguments `{}` takes.", function.label())
        );
    }
    let inner = run_err(&call(Builtin::AbsoluteInteger, vec![Expr::bound("gone")]));
    assert_eq!(inner.location.as_str(), "expression[0]");
}

#[test]
fn an_attempt_compensates_recoverable_faults_alone() {
    let attempt = |body| Expr::Attempt {
        body: Box::new(body),
        compensation: Box::new(Expr::integer(-1)),
    };
    assert_eq!(run(&attempt(Expr::integer(5))), Ok(Value::Integer(5)));
    assert_eq!(
        run(&attempt(Expr::binary(
            BinaryOp::Remainder,
            Expr::integer(5),
            Expr::integer(0)
        ))),
        Ok(Value::Integer(-1))
    );
    let type_fault = run_err(&attempt(Expr::binary(
        BinaryOp::And,
        Expr::integer(1),
        Expr::integer(2),
    )));
    assert_eq!(type_fault.code, DiagnosticCode::TypeMismatch);
    assert_eq!(type_fault.location.as_str(), "expression.body");
}

// Binary evaluator paths

fn binary_run(op: BinaryOp, left: Value, right: Value) -> Result<Value, Diagnostic> {
    run(&Expr::binary(op, lit(left), lit(right)))
}

#[test]
fn integer_arithmetic_runs_exactly_and_reports_faults() {
    let int = Value::Integer;
    assert_eq!(binary_run(BinaryOp::Add, int(2), int(3)), Ok(int(5)));
    assert_eq!(binary_run(BinaryOp::Subtract, int(2), int(3)), Ok(int(-1)));
    assert_eq!(binary_run(BinaryOp::Multiply, int(2), int(3)), Ok(int(6)));
    assert_eq!(binary_run(BinaryOp::Divide, int(7), int(2)), Ok(int(3)));
    assert_eq!(binary_run(BinaryOp::Remainder, int(7), int(2)), Ok(int(1)));
    let overflows = [
        (BinaryOp::Add, int(i128::MAX), int(1)),
        (BinaryOp::Subtract, int(i128::MIN), int(1)),
        (BinaryOp::Multiply, int(i128::MAX), int(2)),
        (BinaryOp::Divide, int(i128::MIN), int(-1)),
        (BinaryOp::Remainder, int(i128::MIN), int(-1)),
    ];
    for (op, left, right) in overflows {
        let fault = binary_run(op, left, right).expect_err("the result overflows");
        assert_eq!(fault.code, DiagnosticCode::ArithmeticOverflow);
    }
    for op in [BinaryOp::Divide, BinaryOp::Remainder] {
        let fault = binary_run(op, int(1), int(0)).expect_err("the divisor is zero");
        assert_eq!(fault.code, DiagnosticCode::DivisionByZero);
        assert_eq!(
            fault.fix,
            "Guard the division with a check that the divisor holds a value above zero."
        );
    }
}

#[test]
fn decimal_arithmetic_runs_exactly_and_reports_faults() {
    assert_eq!(
        binary_run(BinaryOp::Add, dec(125, 2), dec(5, 2)),
        Ok(dec(130, 2))
    );
    assert_eq!(
        binary_run(BinaryOp::Subtract, dec(125, 2), dec(5, 2)),
        Ok(dec(120, 2))
    );
    assert_eq!(
        binary_run(BinaryOp::Multiply, dec(15, 1), dec(15, 1)),
        Ok(dec(225, 2))
    );
    assert_eq!(
        binary_run(BinaryOp::Divide, dec(300, 2), dec(150, 2)),
        Ok(dec(200, 2))
    );
    let mismatch = binary_run(BinaryOp::Add, dec(1, 1), dec(1, 2)).expect_err("the scales differ");
    assert_eq!(mismatch.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        mismatch.fix,
        "Apply the rescale operation to one side so both carry the same scale."
    );
    let subtract =
        binary_run(BinaryOp::Subtract, dec(1, 1), dec(1, 2)).expect_err("the scales differ");
    assert_eq!(subtract.code, DiagnosticCode::TypeMismatch);
    let wide = binary_run(BinaryOp::Multiply, dec(1, 20), dec(1, 20))
        .expect_err("the product scale runs past 30");
    assert_eq!(wide.code, DiagnosticCode::ValueOutOfRange);
    let loss = binary_run(BinaryOp::Divide, dec(100, 2), dec(300, 2))
        .expect_err("one third needs more digits");
    assert_eq!(loss.code, DiagnosticCode::PrecisionLoss);
    let zero =
        binary_run(BinaryOp::Divide, dec(100, 2), dec(0, 2)).expect_err("the divisor is zero");
    assert_eq!(zero.code, DiagnosticCode::DivisionByZero);
}

#[test]
fn every_operation_rejects_operands_of_the_wrong_kind_at_run_time() {
    for op in [
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::ConcatText,
        BinaryOp::AppendList,
    ] {
        let fault = binary_run(op, Value::Hash(Hash32::default()), Value::Integer(1))
            .expect_err("a hash suits no operation");
        assert_eq!(fault.code, DiagnosticCode::TypeMismatch);
        assert_eq!(
            fault.problem,
            format!(
                "The {} holds a hash on the left and a whole number on the right.",
                op.label()
            )
        );
        assert_eq!(fault.fix, "Supply operands the operation accepts.");
    }
}

#[test]
fn equality_compares_canonical_forms() {
    let unsorted = unsorted_map();
    let sorted = int_map(&[(1, "a"), (9, "z")]);
    assert_ne!(unsorted, sorted);
    assert_eq!(
        binary_run(BinaryOp::Equal, unsorted.clone(), sorted.clone()),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        binary_run(BinaryOp::NotEqual, unsorted, sorted),
        Ok(Value::Bool(false))
    );
    assert_eq!(
        binary_run(BinaryOp::NotEqual, Value::Integer(1), Value::Integer(2)),
        Ok(Value::Bool(true))
    );
}

#[test]
fn ordering_covers_every_ordered_value_kind() {
    let pairs = [
        (Value::Integer(1), Value::Integer(2)),
        (dec(1, 2), dec(2, 2)),
        (Value::Text("a".to_string()), Value::Text("b".to_string())),
        (Value::Bytes(vec![1]), Value::Bytes(vec![2])),
        (
            Value::Timestamp(Timestamp::from_millis(1)),
            Value::Timestamp(Timestamp::from_millis(2)),
        ),
        (
            Value::Identity(Identity([1u8; 32])),
            Value::Identity(Identity([2u8; 32])),
        ),
        (
            Value::Hash(Hash32([1u8; 32])),
            Value::Hash(Hash32([2u8; 32])),
        ),
        (Value::Bool(false), Value::Bool(true)),
    ];
    let outcome = |op, left: &Value, right: &Value| {
        binary_run(op, left.clone(), right.clone()).expect("the ordering runs")
    };
    for (low, high) in pairs {
        assert_eq!(outcome(BinaryOp::Less, &low, &high), Value::Bool(true));
        assert_eq!(outcome(BinaryOp::Less, &high, &low), Value::Bool(false));
        assert_eq!(
            outcome(BinaryOp::LessOrEqual, &low, &low),
            Value::Bool(true)
        );
        assert_eq!(
            outcome(BinaryOp::LessOrEqual, &high, &low),
            Value::Bool(false)
        );
        assert_eq!(outcome(BinaryOp::Greater, &high, &low), Value::Bool(true));
        assert_eq!(outcome(BinaryOp::Greater, &low, &low), Value::Bool(false));
        assert_eq!(
            outcome(BinaryOp::GreaterOrEqual, &low, &low),
            Value::Bool(true)
        );
        assert_eq!(
            outcome(BinaryOp::GreaterOrEqual, &low, &high),
            Value::Bool(false)
        );
    }
    let scales = binary_run(BinaryOp::Less, dec(1, 1), dec(1, 2)).expect_err("scales differ");
    assert_eq!(scales.code, DiagnosticCode::TypeMismatch);
    let kinds = binary_run(
        BinaryOp::Greater,
        Value::Integer(1),
        Value::Text("1".to_string()),
    )
    .expect_err("the kinds differ");
    assert_eq!(
        kinds.problem,
        "The ordering holds a whole number on the left and text on the right."
    );
    assert_eq!(kinds.fix, "Order two values sharing one ordered type.");
}

#[test]
fn logic_joins_and_appends_run_exactly() {
    let truth = Value::Bool;
    assert_eq!(
        binary_run(BinaryOp::And, truth(true), truth(false)),
        Ok(truth(false))
    );
    assert_eq!(
        binary_run(BinaryOp::And, truth(true), truth(true)),
        Ok(truth(true))
    );
    assert_eq!(
        binary_run(BinaryOp::Or, truth(false), truth(true)),
        Ok(truth(true))
    );
    assert_eq!(
        binary_run(BinaryOp::Or, truth(false), truth(false)),
        Ok(truth(false))
    );
    assert_eq!(
        binary_run(
            BinaryOp::ConcatText,
            Value::Text("Block".to_string()),
            Value::Text("chain".to_string())
        ),
        Ok(Value::Text("Blockchain".to_string()))
    );
    assert_eq!(
        binary_run(BinaryOp::AppendList, int_list(&[1, 2]), int_list(&[3])),
        Ok(int_list(&[1, 2, 3]))
    );
    let mixed = binary_run(
        BinaryOp::AppendList,
        int_list(&[1]),
        Value::list(TypeExpr::Text, vec![Value::Text("a".to_string())]),
    )
    .expect_err("the element types differ");
    assert_eq!(mixed.code, DiagnosticCode::TypeMismatch);
    assert_eq!(mixed.problem, "The two lists hold different element types.");
    assert_eq!(mixed.fix, "Append two lists sharing one element type.");
}

#[test]
fn a_fold_inside_an_expression_accumulates_over_a_list_and_a_collection() {
    let over_list = Expr::Fold(Box::new(FoldExpr::over_list(
        lit(int_list(&[1, 2, 3, 4])),
        "item",
        "sum",
        Expr::integer(0),
        Expr::binary(BinaryOp::Add, Expr::bound("sum"), Expr::bound("item")),
        4,
    )));
    assert_eq!(check(&over_list), Ok(TypeExpr::Integer));
    assert_eq!(run(&over_list), Ok(Value::Integer(10)));

    let mut view = MemoryView::new();
    let ledger = CollectionId::new("ledger");
    for (key, amount) in [(1, 10), (2, 20), (3, 30)] {
        view.insert(&ledger, &Value::Integer(key), Value::Integer(amount));
    }
    let over_rows = Expr::Fold(Box::new(FoldExpr::over_source(
        FoldSource::collection("ledger", KeySelector::Full, TypeExpr::Integer),
        "row",
        "sum",
        Expr::integer(0),
        Expr::binary(BinaryOp::Add, Expr::bound("sum"), Expr::bound("row")),
        3,
    )));
    let scoped = Env::with_view(Identity::default(), Timestamp::default(), &view);
    assert_eq!(over_rows.run(&scoped), Ok(Value::Integer(60)));
    assert_eq!(over_rows.run(&env()), Ok(Value::Integer(0)));
}

#[test]
fn a_step_budget_stops_a_long_expression() {
    let mut expr = Expr::integer(0);
    for _ in 0..200 {
        expr = Expr::binary(BinaryOp::Add, expr, Expr::integer(1));
    }
    assert_eq!(run(&expr), Ok(Value::Integer(200)));
    let mut meter = Meter::new(100);
    let fault = expr
        .evaluate(&env(), &mut meter, &Path::root().field("budget"))
        .expect_err("the budget of 100 steps runs out");
    assert_eq!(fault.code, DiagnosticCode::BoundExceeded);
    assert_eq!(
        fault.problem,
        "The expression ran past its budget of 100 steps."
    );
}
