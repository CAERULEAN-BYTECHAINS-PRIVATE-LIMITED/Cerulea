//! Behavioural tests for invariants and preconditions.
//!
//! Each test runs the real rule evaluation or the real section check and
//! asserts the concrete result, the diagnostic class and the fix.

use crate::{
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    expression::{BinaryOp, Env, Expr, Meter},
    permissions::ROW_BINDING,
    rules::{Invariant, Precondition, RuleInput, Rules},
    types::{
        CollectionId, FieldDef, FieldName, ParamName, RecordDef, RuleId, TypeDef, TypeExpr, TypeId,
        TypeRegistry, Value,
    },
    DiagnosticCode, Location, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
            FieldDef::new("area", TypeExpr::Integer, "The area."),
        ],
        summary: "One parcel.".to_string(),
    })]
}

fn collections() -> Vec<Collection> {
    let mut parcel = Collection::new("parcel", "parcel", "survey_number", "The parcels.");
    parcel.purpose = PurposeDeclaration::new("The register records land.", &["registration"]);
    vec![parcel]
}

fn area_positive() -> Invariant {
    Invariant {
        id: RuleId::new("area_positive"),
        collection: CollectionId::new("parcel"),
        holds: Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound(ROW_BINDING), "area"),
            Expr::integer(0),
        ),
        problem: "The parcel covers zero land.".to_string(),
        fix: "Enter an area above zero.".to_string(),
    }
}

fn fee_fits() -> Precondition {
    Precondition {
        id: RuleId::new("fee_fits"),
        inputs: vec![RuleInput::new("fee", TypeExpr::Integer)],
        holds: Expr::binary(BinaryOp::Less, Expr::bound("fee"), Expr::integer(1_000)),
        problem: "The fee runs past the ceiling.".to_string(),
        fix: "Enter a fee below 1000.".to_string(),
    }
}

fn clean() -> Rules {
    Rules {
        invariants: vec![area_positive()],
        preconditions: vec![fee_fits()],
    }
}

fn report(rules: &Rules) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    rules.validate(&registry, &collections, &Path::root().field("rules"))
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

fn parcel(area: i128) -> Value {
    Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text("KL-1".to_string()),
            ),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    )
}

#[test]
fn an_invariant_holds_over_a_record_or_names_the_violation() {
    let rule = area_positive();
    let env = Env::default();
    let mut meter = Meter::default();
    assert_eq!(
        rule.check_record(&parcel(5), &env, &mut meter, &Path::root()),
        Ok(true)
    );
    assert_eq!(
        rule.check_record(&parcel(0), &env, &mut meter, &Path::root()),
        Ok(false)
    );
    let violation = rule.violation(Location("parcel['KL-1']".to_string()));
    assert_eq!(violation.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(violation.location.as_str(), "parcel['KL-1']");
    assert_eq!(violation.problem, "The parcel covers zero land.");
    assert_eq!(violation.fix, "Enter an area above zero.");

    let mut numeric = area_positive();
    numeric.holds = Expr::field(Expr::bound(ROW_BINDING), "area");
    let fault = numeric
        .check_record(&parcel(5), &env, &mut meter, &Path::root().field("rule"))
        .expect_err("a number is no truth value");
    assert_eq!(fault.code, DiagnosticCode::TypeMismatch);
    assert_eq!(fault.problem, "The invariant produced a whole number.");
    assert_eq!(fault.fix, "Give the invariant a truth value.");
    let broken = area_positive();
    let absent = broken
        .check_record(&Value::Integer(1), &env, &mut meter, &Path::root())
        .expect_err("a whole number holds no field");
    assert_eq!(absent.code, DiagnosticCode::TypeMismatch);
}

#[test]
fn a_precondition_gates_the_inputs_or_names_the_violation() {
    let rule = fee_fits();
    let mut meter = Meter::default();
    let low = Env::default().with(&ParamName::new("fee"), Value::Integer(10));
    let high = Env::default().with(&ParamName::new("fee"), Value::Integer(5_000));
    assert_eq!(rule.check_inputs(&low, &mut meter, &Path::root()), Ok(true));
    assert_eq!(
        rule.check_inputs(&high, &mut meter, &Path::root()),
        Ok(false)
    );
    let violation = rule.violation(Location::default());
    assert_eq!(violation.fix, "Enter a fee below 1000.");
    let mut texty = fee_fits();
    texty.holds = Expr::text("yes");
    let fault = texty
        .check_inputs(&low, &mut meter, &Path::root())
        .expect_err("text is no truth value");
    assert_eq!(fault.problem, "The precondition produced text.");
    assert_eq!(fault.fix, "Give the precondition a truth value.");
    let unbound = rule
        .check_inputs(
            &crate::expression::Env::default(),
            &mut meter,
            &Path::root(),
        )
        .expect_err("the fee is unbound");
    assert_eq!(unbound.code, DiagnosticCode::DanglingReference);
}

#[test]
fn the_section_answers_lookups_by_name_and_by_collection() {
    let rules = clean();
    assert!(rules.precondition(&RuleId::new("fee_fits")).is_some());
    assert!(rules.precondition(&RuleId::new("area_positive")).is_none());
    assert!(rules.invariant(&RuleId::new("area_positive")).is_some());
    assert!(rules.invariant(&RuleId::new("fee_fits")).is_none());
    assert_eq!(rules.invariants_over(&CollectionId::new("parcel")).len(), 1);
    assert!(rules
        .invariants_over(&CollectionId::new("ledger"))
        .is_empty());
    assert_eq!(rules.precondition_names(), ["fee_fits"]);
    assert_eq!(
        rules
            .missing_precondition(&RuleId::new("ghost"), Location::default())
            .fix,
        "Point the entry at one of these preconditions: fee_fits."
    );
    assert_eq!(
        Rules::default()
            .missing_precondition(&RuleId::new("ghost"), Location::default())
            .fix,
        "Declare a precondition named `ghost` in the rules section."
    );
    assert!(report(&rules).is_clean());
}

#[test]
fn invariant_faults_each_raise_a_diagnostic() {
    let mut rules = clean();
    let mut repeated = area_positive();
    repeated.problem.clear();
    repeated.fix.clear();
    repeated.holds = Expr::field(Expr::bound(ROW_BINDING), "area");
    rules.invariants.push(repeated);
    let mut stray = area_positive();
    stray.id = RuleId::new("Stray");
    stray.collection = CollectionId::new("ghost");
    rules.invariants.push(stray);
    let mut broken = area_positive();
    broken.id = RuleId::new("broken");
    broken.holds = Expr::bound("absent");
    rules.invariants.push(broken);
    let found = report(&rules);
    assert_eq!(
        at(&found, "rules.invariants[1].id").fix,
        "Rename one of the two rules away from `area_positive`."
    );
    assert_eq!(
        at(&found, "rules.invariants[1].problem").fix,
        "Write one sentence a person reads when the rule declines a change."
    );
    assert_eq!(
        at(&found, "rules.invariants[1].fix").fix,
        "Write one sentence stating what a person does next."
    );
    let holds = at(&found, "rules.invariants[1].holds");
    assert_eq!(holds.problem, "The rule carries a whole number.");
    assert_eq!(holds.fix, "Give the rule a truth value.");
    assert_eq!(
        at(&found, "rules.invariants[2].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "rules.invariants[2].collection").fix,
        "Point the reference at one of these collections: parcel."
    );
    assert_eq!(
        at(&found, "rules.invariants[3].holds").code,
        DiagnosticCode::DanglingReference
    );
}

#[test]
fn precondition_faults_each_raise_a_diagnostic() {
    let mut rules = clean();
    rules.preconditions.push(Precondition {
        id: RuleId::new("area_positive"),
        inputs: vec![
            RuleInput::new("Fee", TypeExpr::Integer),
            RuleInput::new("fee", TypeExpr::Decimal { scale: 31 }),
            RuleInput::new("fee", TypeExpr::Integer),
        ],
        holds: Expr::bound("fee"),
        problem: "A problem.".to_string(),
        fix: "A fix.".to_string(),
    });
    let found = report(&rules);
    let prefix = "rules.preconditions[1]";
    assert_eq!(
        at(&found, &alloc::format!("{prefix}.id")).code,
        DiagnosticCode::DuplicateIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}.inputs[0].name")).code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}.inputs[1].ty")).code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}.inputs[2].name")).fix,
        "Rename one of the two `fee` inputs."
    );
    assert_eq!(
        at(&found, &alloc::format!("{prefix}.holds")).problem,
        "The rule carries a whole number."
    );
}
