//! Invariants and preconditions.
//!
//! An invariant holds at all times over every record of one collection. The
//! write path checks every invariant before a record lands.
//!
//! A precondition gates one action. The action names the precondition and
//! supplies the inputs the precondition declares.
//!
//! Every rule carries one sentence stating the problem and one sentence
//! stating the fix. A person reads those sentences when the rule declines a
//! change.

use crate::{
    collections::CollectionRegistry,
    expression::{Env, Expr, Meter, TypeContext},
    permissions::ROW_BINDING,
    types::{CollectionId, ParamName, RuleId, TypeExpr, TypeRegistry, Value},
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// A reference to a rule.
pub type RuleRef = RuleId;

/// One input a precondition reads.
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
pub struct RuleInput {
    /// The name the precondition binds.
    pub name: ParamName,
    /// The type the input carries.
    pub ty: TypeExpr,
}

impl RuleInput {
    /// Declare an input.
    pub fn new(name: &str, ty: TypeExpr) -> Self {
        RuleInput {
            name: ParamName::new(name),
            ty,
        }
    }
}

/// A condition holding at all times over one collection.
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
pub struct Invariant {
    /// The name of the rule.
    pub id: RuleId,
    /// The collection the rule covers.
    pub collection: CollectionId,
    /// The predicate over the record. The record binds to the name `record`.
    pub holds: Expr,
    /// One sentence stating the problem a person sees when the rule declines
    /// a change.
    pub problem: String,
    /// One sentence stating the fix.
    pub fix: String,
}

impl Invariant {
    /// Run the invariant against one record.
    pub fn check_record(
        &self,
        record: &Value,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<bool, Diagnostic> {
        let bound = env.with(&ParamName::new(ROW_BINDING), record.clone());
        match self.holds.evaluate(&bound, meter, path)? {
            Value::Bool(held) => Ok(held),
            other => Err(Diagnostic::at(
                DiagnosticCode::TypeMismatch,
                path.location(),
                format!("The invariant produced {}.", other.infer_type().describe()),
                "Give the invariant a truth value.".to_string(),
            )),
        }
    }

    /// The diagnostic a person reads when the record fails the rule.
    pub fn violation(&self, location: Location) -> Diagnostic {
        Diagnostic::at(
            DiagnosticCode::ValueOutOfRange,
            location,
            self.problem.clone(),
            self.fix.clone(),
        )
    }
}

/// A condition gating one action.
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
pub struct Precondition {
    /// The name of the rule.
    pub id: RuleId,
    /// The inputs the rule reads. An action supplies a parameter of the same
    /// name and type for each one.
    pub inputs: Vec<RuleInput>,
    /// The predicate over the inputs.
    pub holds: Expr,
    /// One sentence stating the problem a person sees when the rule declines
    /// the action.
    pub problem: String,
    /// One sentence stating the fix.
    pub fix: String,
}

impl Precondition {
    /// Run the precondition against bound inputs.
    pub fn check_inputs(
        &self,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<bool, Diagnostic> {
        match self.holds.evaluate(env, meter, path)? {
            Value::Bool(held) => Ok(held),
            other => Err(Diagnostic::at(
                DiagnosticCode::TypeMismatch,
                path.location(),
                format!(
                    "The precondition produced {}.",
                    other.infer_type().describe()
                ),
                "Give the precondition a truth value.".to_string(),
            )),
        }
    }

    /// The diagnostic a person reads when the inputs fail the rule.
    pub fn violation(&self, location: Location) -> Diagnostic {
        Diagnostic::at(
            DiagnosticCode::ValueOutOfRange,
            location,
            self.problem.clone(),
            self.fix.clone(),
        )
    }
}

/// The rules section.
#[derive(
    Clone,
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
pub struct Rules {
    /// The invariants holding at all times.
    pub invariants: Vec<Invariant>,
    /// The preconditions gating actions.
    pub preconditions: Vec<Precondition>,
}

impl Rules {
    /// Look up a precondition.
    pub fn precondition(&self, id: &RuleRef) -> Option<&Precondition> {
        self.preconditions.iter().find(|rule| &rule.id == id)
    }

    /// Look up an invariant.
    pub fn invariant(&self, id: &RuleRef) -> Option<&Invariant> {
        self.invariants.iter().find(|rule| &rule.id == id)
    }

    /// The invariants covering one collection.
    pub fn invariants_over(&self, collection: &CollectionId) -> Vec<&Invariant> {
        self.invariants
            .iter()
            .filter(|rule| &rule.collection == collection)
            .collect()
    }

    /// The precondition names, in declaration order.
    pub fn precondition_names(&self) -> Vec<&str> {
        self.preconditions
            .iter()
            .map(|rule| rule.id.as_str())
            .collect()
    }

    /// Raise a dangling reference diagnostic for a precondition the
    /// Blueprint lacks.
    pub fn missing_precondition(&self, id: &RuleRef, location: Location) -> Diagnostic {
        let listed = self.precondition_names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the rule `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a precondition named `{id}` in the rules section.")
            } else {
                format!("Point the entry at one of these preconditions: {listed}.")
            },
        )
    }

    /// Check the whole rules section.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        let mut seen: BTreeSet<&str> = BTreeSet::new();

        let invariants_path = path.field("invariants");
        for (position, rule) in self.invariants.iter().enumerate() {
            let here = invariants_path.index(position);
            report.absorb(rule.id.validate(&here.field("id")));
            if !seen.insert(rule.id.as_str()) {
                report.push(repeated(&rule.id, here.field("id").location()));
            }
            report.merge(sentences(&rule.problem, &rule.fix, &rule.id, &here));

            match collections.get(&rule.collection) {
                Some(collection) => {
                    let ctx = TypeContext::new(types).with(
                        &ParamName::new(ROW_BINDING),
                        TypeExpr::Record(collection.record_type.clone()),
                    );
                    check_predicate(&rule.holds, &ctx, &here.field("holds"), &mut report);
                }
                None => report.push(
                    collections.missing(&rule.collection, here.field("collection").location()),
                ),
            }
        }

        let preconditions_path = path.field("preconditions");
        for (position, rule) in self.preconditions.iter().enumerate() {
            let here = preconditions_path.index(position);
            report.absorb(rule.id.validate(&here.field("id")));
            if !seen.insert(rule.id.as_str()) {
                report.push(repeated(&rule.id, here.field("id").location()));
            }
            report.merge(sentences(&rule.problem, &rule.fix, &rule.id, &here));

            let mut ctx = TypeContext::new(types);
            let mut names: BTreeSet<&str> = BTreeSet::new();
            for (index, input) in rule.inputs.iter().enumerate() {
                let input_path = here.field("inputs").index(index);
                report.absorb(input.name.validate(&input_path.field("name")));
                report.absorb(input.ty.validate(types, &input_path.field("ty")));
                if !names.insert(input.name.as_str()) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DuplicateName,
                        input_path.field("name").location(),
                        format!(
                            "The rule `{}` declares the input `{}` twice.",
                            rule.id, input.name
                        ),
                        format!("Rename one of the two `{}` inputs.", input.name),
                    ));
                }
                ctx.bind(&input.name, input.ty.clone());
            }
            check_predicate(&rule.holds, &ctx, &here.field("holds"), &mut report);
        }

        report
    }
}

fn repeated(id: &RuleId, location: Location) -> Diagnostic {
    Diagnostic::at(
        DiagnosticCode::DuplicateIdentifier,
        location,
        format!("Two rules share the name `{id}`."),
        format!("Rename one of the two rules away from `{id}`."),
    )
}

fn sentences(problem: &str, fix: &str, id: &RuleId, here: &Path) -> Report {
    let mut report = Report::new();
    if problem.is_empty() {
        report.push(Diagnostic::at(
            DiagnosticCode::MissingValue,
            here.field("problem").location(),
            format!("The rule `{id}` states no problem."),
            "Write one sentence a person reads when the rule declines a change.".to_string(),
        ));
    }
    if fix.is_empty() {
        report.push(Diagnostic::at(
            DiagnosticCode::MissingValue,
            here.field("fix").location(),
            format!("The rule `{id}` names no fix."),
            "Write one sentence stating what a person does next.".to_string(),
        ));
    }
    report
}

fn check_predicate(holds: &Expr, ctx: &TypeContext, path: &Path, report: &mut Report) {
    match holds.check(ctx, path) {
        Ok(TypeExpr::Bool) => {}
        Ok(other) => report.push(Diagnostic::at(
            DiagnosticCode::TypeMismatch,
            path.location(),
            format!("The rule carries {}.", other.describe()),
            "Give the rule a truth value.".to_string(),
        )),
        Err(diagnostic) => report.push(diagnostic),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collections::{Collection, PurposeDeclaration},
        expression::BinaryOp,
        types::{Decimal, FieldDef, FieldName, Identity, RecordDef, Timestamp, TypeDef, TypeId},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new(
                    "area",
                    TypeExpr::Decimal { scale: 4 },
                    "The area in hectares.",
                ),
            ],
            summary: "One land parcel.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut collection =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        collection.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        vec![collection]
    }

    fn sample() -> Rules {
        Rules {
            invariants: vec![Invariant {
                id: RuleId::new("area_stays_positive"),
                collection: CollectionId::new("parcel"),
                holds: Expr::binary(
                    BinaryOp::Greater,
                    Expr::field(Expr::bound(ROW_BINDING), "area"),
                    Expr::decimal(0, 4),
                ),
                problem: "The parcel area sits at zero or below.".to_string(),
                fix: "Enter an area above zero hectares.".to_string(),
            }],
            preconditions: vec![Precondition {
                id: RuleId::new("transfer_area_fits"),
                inputs: vec![
                    RuleInput::new("moved_area", TypeExpr::Decimal { scale: 4 }),
                    RuleInput::new("held_area", TypeExpr::Decimal { scale: 4 }),
                ],
                holds: Expr::binary(
                    BinaryOp::LessOrEqual,
                    Expr::bound("moved_area"),
                    Expr::bound("held_area"),
                ),
                problem: "The transfer moves more land than the holder holds.".to_string(),
                fix: "Reduce the area on the transfer to the area held.".to_string(),
            }],
        }
    }

    fn run(rules: &Rules) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        rules.validate(
            &type_registry,
            &collection_registry,
            &Path::root().field("rules"),
        )
    }

    #[test]
    fn a_complete_rules_section_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn an_invariant_naming_an_absent_collection_reports_its_location() {
        let mut rules = sample();
        rules.invariants[0].collection = CollectionId::new("absent_collection");
        let report = run(&rules);
        let found = report
            .find("rules.invariants[0].collection")
            .expect("the diagnostic sits on the collection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn an_invariant_naming_an_absent_field_reports_its_location() {
        let mut rules = sample();
        rules.invariants[0].holds = Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound(ROW_BINDING), "absent_field"),
            Expr::decimal(0, 4),
        );
        let report = run(&rules);
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .location
                    .as_str()
                    .starts_with("rules.invariants[0].holds")
            })
            .expect("the diagnostic sits on the predicate");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_rule_carrying_a_value_other_than_a_truth_value_is_caught() {
        let mut rules = sample();
        rules.invariants[0].holds = Expr::field(Expr::bound(ROW_BINDING), "area");
        let report = run(&rules);
        let found = report
            .find("rules.invariants[0].holds")
            .expect("the diagnostic sits on the predicate");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
        assert!(found.fix.contains("truth value"));
    }

    #[test]
    fn a_rule_missing_its_fix_sentence_is_caught() {
        let mut rules = sample();
        rules.preconditions[0].fix = String::new();
        let report = run(&rules);
        let found = report
            .find("rules.preconditions[0].fix")
            .expect("the diagnostic sits on the fix sentence");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
    }

    #[test]
    fn two_rules_sharing_a_name_are_caught() {
        let mut rules = sample();
        rules.preconditions[0].id = RuleId::new("area_stays_positive");
        let report = run(&rules);
        let found = report
            .find("rules.preconditions[0].id")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn an_invariant_runs_against_a_record() {
        let rules = sample();
        let env = Env::new(Identity::default(), Timestamp::from_millis(1));
        let mut meter = Meter::default();
        let good = Value::record(
            TypeId::new("parcel"),
            vec![
                (
                    FieldName::new("survey_number"),
                    Value::Text("KL-1".to_string()),
                ),
                (
                    FieldName::new("area"),
                    Value::Decimal(Decimal::new(25_000, 4)),
                ),
            ],
        );
        assert!(rules.invariants[0]
            .check_record(&good, &env, &mut meter, &Path::root().field("holds"))
            .expect("the invariant runs"));

        let bad = Value::record(
            TypeId::new("parcel"),
            vec![
                (
                    FieldName::new("survey_number"),
                    Value::Text("KL-2".to_string()),
                ),
                (FieldName::new("area"), Value::Decimal(Decimal::new(0, 4))),
            ],
        );
        assert!(!rules.invariants[0]
            .check_record(&bad, &env, &mut meter, &Path::root().field("holds"))
            .expect("the invariant runs"));
    }

    #[test]
    fn a_precondition_runs_against_bound_inputs() {
        let rules = sample();
        let mut env = Env::new(Identity::default(), Timestamp::from_millis(1));
        env.bind(
            &ParamName::new("moved_area"),
            Value::Decimal(Decimal::new(500, 4)),
        );
        env.bind(
            &ParamName::new("held_area"),
            Value::Decimal(Decimal::new(25_000, 4)),
        );
        let mut meter = Meter::default();
        assert!(rules.preconditions[0]
            .check_inputs(&env, &mut meter, &Path::root().field("holds"))
            .expect("the precondition runs"));
        let violation =
            rules.preconditions[0].violation(Path::root().field("actions").index(0).location());
        assert_eq!(violation.location.as_str(), "actions[0]");
        assert!(violation.fix.contains("Reduce the area"));
    }

    #[test]
    fn the_invariants_over_a_collection_are_listed() {
        let rules = sample();
        assert_eq!(rules.invariants_over(&CollectionId::new("parcel")).len(), 1);
        assert_eq!(rules.invariants_over(&CollectionId::new("other")).len(), 0);
    }

    #[test]
    fn a_missing_precondition_names_the_rules_on_hand() {
        let rules = sample();
        let diagnostic = rules.missing_precondition(
            &RuleId::new("absent_rule"),
            Path::root()
                .field("actions")
                .index(3)
                .field("preconditions")
                .index(0)
                .location(),
        );
        assert_eq!(diagnostic.location.as_str(), "actions[3].preconditions[0]");
        assert!(diagnostic.fix.contains("transfer_area_fits"));
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let rules = sample();
        let bytes = rules.encode();
        let back = Rules::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(rules, back);
        let text = serde_json::to_string(&rules).expect("the section serialises");
        let parsed: Rules = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(rules, parsed);
    }
}
