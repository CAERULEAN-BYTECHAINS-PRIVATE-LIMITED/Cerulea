//! The bounded iteration primitive.
//!
//! A fold applies one operation across every element of a list or a
//! collection range and accumulates one result. Fold covers iteration while
//! keeping termination guaranteed. The element count is known before the
//! fold runs, so the bound is proven rather than assumed.
//!
//! Three checks hold the guarantee.
//!
//! 1. The author declares a maximum element count.
//! 2. The evaluator counts the elements before the first step and stops when
//!    the count runs past the declared maximum.
//! 3. The evaluator meters every step against the expression budget.

use crate::{
    collections::{CollectionRegistry, KeyRange, KeySelector},
    expression::{Env, Expr, Meter, TypeContext},
    types::{CollectionId, ParamName, TypeExpr, Value},
    Diagnostic, DiagnosticCode, Path, Report,
};
use alloc::{collections::BTreeMap, format, string::ToString, vec::Vec};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The largest element count a fold declares.
pub const MAX_FOLD_ELEMENTS: u32 = 1_000_000;

/// A read over loaded Blockchain state.
///
/// The evaluator asks a view for the records a key range reaches. The view
/// answers in canonical key order, so two nodes fold in the same order.
pub trait CollectionView: core::fmt::Debug {
    /// The records the range reaches, in canonical key order.
    fn records(&self, collection: &CollectionId, keys: &KeyRange) -> Vec<Value>;
}

/// A view holding zero records.
///
/// Manifest derivation runs against this view. Key expressions read the
/// transaction parameters alone, so derivation needs no loaded state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DetachedView;

impl CollectionView for DetachedView {
    fn records(&self, _collection: &CollectionId, _keys: &KeyRange) -> Vec<Value> {
        Vec::new()
    }
}

/// A view over records held in memory.
///
/// The Studio preview and the crate tests fold over this view. The records
/// stay sorted by canonical key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryView {
    entries: BTreeMap<CollectionId, Vec<(Vec<u8>, Value)>>,
}

impl MemoryView {
    /// Start an empty view.
    pub fn new() -> Self {
        MemoryView {
            entries: BTreeMap::new(),
        }
    }

    /// Place one record under its canonical key.
    pub fn insert(&mut self, collection: &CollectionId, key: &Value, record: Value) {
        let bytes = key.canonical_bytes();
        let slot = self.entries.entry(collection.clone()).or_default();
        match slot.iter().position(|(held, _)| held == &bytes) {
            Some(position) => slot[position] = (bytes, record),
            None => {
                slot.push((bytes, record));
                slot.sort_by(|left, right| left.0.cmp(&right.0));
            }
        }
    }

    /// Count the records held in one collection.
    pub fn len(&self, collection: &CollectionId) -> usize {
        self.entries
            .get(collection)
            .map(|slot| slot.len())
            .unwrap_or(0)
    }

    /// Report whether the view holds zero records.
    pub fn is_empty(&self) -> bool {
        self.entries.values().all(|slot| slot.is_empty())
    }
}

impl CollectionView for MemoryView {
    fn records(&self, collection: &CollectionId, keys: &KeyRange) -> Vec<Value> {
        let slot = match self.entries.get(collection) {
            Some(slot) => slot,
            None => return Vec::new(),
        };
        slot.iter()
            .filter(|(held, _)| keys.covers(&KeyRange::Exact(held.clone())))
            .map(|(_, record)| record.clone())
            .collect()
    }
}

/// The elements a fold walks.
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
pub enum FoldSource {
    /// A list value. The length is the length of the list.
    List(Expr),
    /// A key range inside a collection. The length is the count of records
    /// the range reaches.
    Collection {
        /// The collection the fold reads.
        collection: CollectionId,
        /// The key range the fold reads.
        keys: KeySelector,
        /// The record type every element carries.
        element: TypeExpr,
    },
}

impl FoldSource {
    /// Read a key range inside a collection.
    pub fn collection(collection: &str, keys: KeySelector, element: TypeExpr) -> FoldSource {
        FoldSource::Collection {
            collection: CollectionId::new(collection),
            keys,
            element,
        }
    }
}

/// The declared maximum element count of a fold.
#[derive(
    Clone,
    Copy,
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
pub struct FoldBound {
    /// The largest element count the fold accepts.
    pub maximum_elements: u32,
}

impl FoldBound {
    /// Declare a bound.
    pub fn new(maximum_elements: u32) -> Self {
        FoldBound { maximum_elements }
    }

    /// Check the declared bound.
    pub fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        if self.maximum_elements == 0 {
            return Err(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                path.location(),
                "The fold declares a maximum of zero elements.".to_string(),
                "Declare a maximum above zero.".to_string(),
            ));
        }
        if self.maximum_elements > MAX_FOLD_ELEMENTS {
            return Err(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                path.location(),
                format!(
                    "The fold declares a maximum of {} elements, above the ceiling of {}.",
                    self.maximum_elements, MAX_FOLD_ELEMENTS
                ),
                format!("Declare a maximum of {MAX_FOLD_ELEMENTS} elements or fewer."),
            ));
        }
        Ok(())
    }
}

/// A fold.
///
/// The fold binds the running result to `accumulator` and each element to
/// `element`. The step expression produces the next running result.
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
pub struct FoldExpr {
    /// The elements the fold walks.
    pub source: FoldSource,
    /// The name each element takes inside the step.
    pub element: ParamName,
    /// The name the running result takes inside the step.
    pub accumulator: ParamName,
    /// The starting value of the running result.
    pub initial: Expr,
    /// The step that produces the next running result.
    pub step: Expr,
    /// The declared maximum element count.
    pub bound: FoldBound,
}

impl FoldExpr {
    /// Build a fold over a list expression.
    pub fn over_list(
        list: Expr,
        element: &str,
        accumulator: &str,
        initial: Expr,
        step: Expr,
        maximum_elements: u32,
    ) -> Self {
        FoldExpr {
            source: FoldSource::List(list),
            element: ParamName::new(element),
            accumulator: ParamName::new(accumulator),
            initial,
            step,
            bound: FoldBound::new(maximum_elements),
        }
    }

    /// Build a fold over a named source.
    pub fn over_source(
        source: FoldSource,
        element: &str,
        accumulator: &str,
        initial: Expr,
        step: Expr,
        maximum_elements: u32,
    ) -> Self {
        FoldExpr {
            source,
            element: ParamName::new(element),
            accumulator: ParamName::new(accumulator),
            initial,
            step,
            bound: FoldBound::new(maximum_elements),
        }
    }

    /// The collection this fold reads, present on a collection source.
    pub fn reads(&self) -> Option<(&CollectionId, &KeySelector)> {
        match &self.source {
            FoldSource::Collection {
                collection, keys, ..
            } => Some((collection, keys)),
            FoldSource::List(_) => None,
        }
    }

    /// Resolve the type of the fold.
    pub fn check(&self, ctx: &TypeContext, path: &Path) -> Result<TypeExpr, Diagnostic> {
        self.bound.validate(&path.field("bound"))?;
        if self.element == self.accumulator {
            return Err(Diagnostic::at(
                DiagnosticCode::DuplicateName,
                path.location(),
                format!(
                    "The fold binds the element and the running result to one name, `{}`.",
                    self.element
                ),
                "Give the element and the running result two names.".to_string(),
            ));
        }
        let element_type = match &self.source {
            FoldSource::List(list) => {
                let list_type = list.check(ctx, &path.field("source"))?;
                match list_type {
                    TypeExpr::List(inner) => *inner,
                    other => {
                        return Err(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            path.field("source").location(),
                            format!("The fold walks {}.", other.describe()),
                            "Fold over a list value or over a collection range.".to_string(),
                        ))
                    }
                }
            }
            FoldSource::Collection { keys, element, .. } => {
                element.validate(ctx.registry, &path.field("source").field("element"))?;
                if let Some(expr) = keys.expression() {
                    expr.check(ctx, &path.field("source").field("keys"))?;
                }
                element.clone()
            }
        };
        let accumulator_type = self.initial.check(ctx, &path.field("initial"))?;
        let inner = ctx
            .with(&self.accumulator, accumulator_type.clone())
            .with(&self.element, element_type);
        let step_type = self.step.check(&inner, &path.field("step"))?;
        if step_type != accumulator_type {
            return Err(Diagnostic::at(
                DiagnosticCode::TypeMismatch,
                path.field("step").location(),
                format!(
                    "The step produces {} and the running result carries {}.",
                    step_type.describe(),
                    accumulator_type.describe()
                ),
                format!("Give the step the type {}.", accumulator_type.describe()),
            ));
        }
        Ok(accumulator_type)
    }

    /// The elements the fold walks, read before the first step.
    ///
    /// The length of this run is the proven bound.
    pub fn elements(
        &self,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<Vec<Value>, Diagnostic> {
        match &self.source {
            FoldSource::List(list) => {
                let value = list.evaluate(env, meter, &path.field("source"))?;
                match value {
                    Value::List(inner) => Ok(inner.items),
                    other => Err(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.field("source").location(),
                        format!("The fold walks {}.", other.infer_type().describe()),
                        "Fold over a list value or over a collection range.".to_string(),
                    )),
                }
            }
            FoldSource::Collection {
                collection, keys, ..
            } => {
                let range = keys.resolve(env, meter, &path.field("source").field("keys"))?;
                Ok(env.view().records(collection, &range))
            }
        }
    }

    /// The element count the fold walks, read before the first step.
    ///
    /// A count above the declared maximum stops the fold.
    pub fn proven_bound(
        &self,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<u32, Diagnostic> {
        let elements = self.elements(env, meter, path)?;
        self.check_length(elements.len(), path)
    }

    fn check_length(&self, length: usize, path: &Path) -> Result<u32, Diagnostic> {
        let count = u32::try_from(length).unwrap_or(u32::MAX);
        if count > self.bound.maximum_elements {
            return Err(Diagnostic::at(
                DiagnosticCode::BoundExceeded,
                path.location(),
                format!(
                    "The fold walks {} elements and declares a maximum of {}.",
                    count, self.bound.maximum_elements
                ),
                format!(
                    "Raise the declared maximum above {count}, or narrow the key range the fold reads."
                ),
            ));
        }
        Ok(count)
    }

    /// Run the fold.
    pub fn evaluate(&self, env: &Env, meter: &mut Meter, path: &Path) -> Result<Value, Diagnostic> {
        let elements = self.elements(env, meter, path)?;
        let count = self.check_length(elements.len(), path)?;
        meter.charge(count as u64, path)?;
        let mut running = self.initial.evaluate(env, meter, &path.field("initial"))?;
        for (position, element) in elements.into_iter().enumerate() {
            let inner = env
                .with(&self.accumulator, running)
                .with(&self.element, element);
            running = self
                .step
                .evaluate(&inner, meter, &path.field("step").index(position))?;
        }
        Ok(running)
    }

    /// Collect every collection this fold names.
    pub fn collection_refs(&self, out: &mut Vec<CollectionId>) {
        match &self.source {
            FoldSource::List(list) => list.collection_refs(out),
            FoldSource::Collection {
                collection,
                keys,
                element,
            } => {
                out.push(collection.clone());
                if let Some(expr) = keys.expression() {
                    expr.collection_refs(out);
                }
                element.collection_refs(out);
            }
        }
        self.initial.collection_refs(out);
        self.step.collection_refs(out);
    }

    /// Collect every name this fold reads without binding first.
    pub fn free_names(&self, bound: &mut Vec<ParamName>, out: &mut Vec<ParamName>) {
        match &self.source {
            FoldSource::List(list) => list.free_names(bound, out),
            FoldSource::Collection { keys, .. } => {
                if let Some(expr) = keys.expression() {
                    expr.free_names(bound, out);
                }
            }
        }
        self.initial.free_names(bound, out);
        bound.push(self.accumulator.clone());
        bound.push(self.element.clone());
        self.step.free_names(bound, out);
        bound.pop();
        bound.pop();
    }

    /// Replace every read of a name with a supplied expression.
    pub fn substitute(&self, replacements: &BTreeMap<ParamName, Expr>) -> FoldExpr {
        let source = match &self.source {
            FoldSource::List(list) => FoldSource::List(list.substitute(replacements)),
            FoldSource::Collection {
                collection,
                keys,
                element,
            } => FoldSource::Collection {
                collection: collection.clone(),
                keys: match keys {
                    KeySelector::Exact(expr) => KeySelector::Exact(expr.substitute(replacements)),
                    KeySelector::Prefix(expr) => KeySelector::Prefix(expr.substitute(replacements)),
                    KeySelector::Full => KeySelector::Full,
                },
                element: element.clone(),
            },
        };
        let mut inner = replacements.clone();
        inner.remove(&self.accumulator);
        inner.remove(&self.element);
        FoldExpr {
            source,
            element: self.element.clone(),
            accumulator: self.accumulator.clone(),
            initial: self.initial.substitute(replacements),
            step: self.step.substitute(&inner),
            bound: self.bound,
        }
    }

    /// Check that a collection source names a real collection and declares
    /// the record type that collection holds.
    pub fn validate_source(&self, collections: &CollectionRegistry, path: &Path) -> Report {
        let mut report = Report::new();
        if let FoldSource::Collection {
            collection,
            element,
            ..
        } = &self.source
        {
            let here = path.field("source");
            match collections.get(collection) {
                Some(found) => {
                    let declared = TypeExpr::Record(found.record_type.clone());
                    if element != &declared {
                        report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            here.field("element").location(),
                            format!(
                                "The fold declares {} and the collection `{}` holds {}.",
                                element.describe(),
                                collection,
                                declared.describe()
                            ),
                            format!("Declare {} as the element type.", declared.describe()),
                        ));
                    }
                }
                None => report
                    .push(collections.missing(collection, here.field("collection").location())),
            }
        }
        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        expression::{BinaryOp, Meter},
        types::{
            Decimal, FieldDef, FieldName, Identity, RecordDef, Timestamp, TypeDef, TypeId,
            TypeRegistry,
        },
    };
    use alloc::{boxed::Box, string::ToString, vec};

    fn defs() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("line_item"),
            fields: vec![
                FieldDef::new("reference", TypeExpr::Text, "The line reference."),
                FieldDef::new("amount", TypeExpr::Decimal { scale: 2 }, "The line amount."),
            ],
            summary: "One line of an invoice.".to_string(),
        })]
    }

    fn env() -> Env<'static> {
        Env::new(
            Identity([4u8; 32]),
            Timestamp::from_millis(1_700_000_000_000),
        )
    }

    fn sum_of_list(maximum: u32) -> FoldExpr {
        FoldExpr::over_list(
            Expr::bound("amounts"),
            "amount",
            "running",
            Expr::integer(0),
            Expr::binary(BinaryOp::Add, Expr::bound("running"), Expr::bound("amount")),
            maximum,
        )
    }

    fn list_of(count: i128) -> Value {
        Value::list(TypeExpr::Integer, (1..=count).map(Value::Integer).collect())
    }

    #[test]
    fn a_fold_terminates_inside_its_bound() {
        let fold = sum_of_list(10);
        let mut bound = env();
        bound.bind(&ParamName::new("amounts"), list_of(4));
        let mut meter = Meter::default();
        let path = Path::root().field("fold");
        assert_eq!(
            fold.proven_bound(&bound, &mut meter, &path)
                .expect("the count reads"),
            4
        );
        let mut meter = Meter::default();
        let value = fold
            .evaluate(&bound, &mut meter, &path)
            .expect("the fold runs");
        assert_eq!(value, Value::Integer(10));
    }

    #[test]
    fn a_fold_above_its_bound_aborts_with_the_fix() {
        let fold = sum_of_list(3);
        let mut bound = env();
        bound.bind(&ParamName::new("amounts"), list_of(7));
        let mut meter = Meter::default();
        let path = Path::root().field("fold");
        let diagnostic = fold
            .evaluate(&bound, &mut meter, &path)
            .expect_err("the bound stops the fold");
        assert_eq!(diagnostic.code, DiagnosticCode::BoundExceeded);
        assert_eq!(diagnostic.location.as_str(), "fold");
        assert!(diagnostic.problem.contains('7'));
        assert!(diagnostic.fix.contains("Raise the declared maximum"));
    }

    #[test]
    fn the_evaluator_meters_every_step() {
        let fold = sum_of_list(100);
        let mut bound = env();
        bound.bind(&ParamName::new("amounts"), list_of(50));
        let mut meter = Meter::new(20);
        let diagnostic = fold
            .evaluate(&bound, &mut meter, &Path::root().field("fold"))
            .expect_err("the budget stops the fold");
        assert_eq!(diagnostic.code, DiagnosticCode::BoundExceeded);
        assert!(diagnostic.fix.contains("step limit"));
    }

    #[test]
    fn the_checker_resolves_the_fold_type() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry).with(
            &ParamName::new("amounts"),
            TypeExpr::list(TypeExpr::Integer),
        );
        let fold = sum_of_list(10);
        let ty = fold
            .check(&ctx, &Path::root().field("fold"))
            .expect("the fold type checks");
        assert_eq!(ty, TypeExpr::Integer);
    }

    #[test]
    fn the_checker_catches_a_step_of_the_wrong_type() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry).with(
            &ParamName::new("amounts"),
            TypeExpr::list(TypeExpr::Integer),
        );
        let fold = FoldExpr::over_list(
            Expr::bound("amounts"),
            "amount",
            "running",
            Expr::integer(0),
            Expr::text("wrong"),
            10,
        );
        let diagnostic = fold
            .check(&ctx, &Path::root().field("fold"))
            .expect_err("a step of the wrong type fails");
        assert_eq!(diagnostic.code, DiagnosticCode::TypeMismatch);
        assert_eq!(diagnostic.location.as_str(), "fold.step");
    }

    #[test]
    fn the_checker_declines_a_zero_bound() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry).with(
            &ParamName::new("amounts"),
            TypeExpr::list(TypeExpr::Integer),
        );
        let fold = sum_of_list(0);
        let diagnostic = fold
            .check(&ctx, &Path::root().field("fold"))
            .expect_err("a zero bound fails");
        assert_eq!(diagnostic.code, DiagnosticCode::ValueOutOfRange);
        assert_eq!(diagnostic.location.as_str(), "fold.bound");
    }

    #[test]
    fn a_fold_over_a_collection_reads_the_key_range() {
        let mut view = MemoryView::new();
        let collection = CollectionId::new("line_item");
        for (reference, amount) in [("a_0001", 1000i128), ("a_0002", 2500), ("b_0001", 400)] {
            view.insert(
                &collection,
                &Value::Text(reference.to_string()),
                Value::record(
                    TypeId::new("line_item"),
                    vec![
                        (
                            FieldName::new("reference"),
                            Value::Text(reference.to_string()),
                        ),
                        (
                            FieldName::new("amount"),
                            Value::Decimal(Decimal::new(amount, 2)),
                        ),
                    ],
                ),
            );
        }
        assert_eq!(view.len(&collection), 3);

        let fold = FoldExpr::over_source(
            FoldSource::collection(
                "line_item",
                KeySelector::Full,
                TypeExpr::Record(TypeId::new("line_item")),
            ),
            "line",
            "running",
            Expr::decimal(0, 2),
            Expr::binary(
                BinaryOp::Add,
                Expr::bound("running"),
                Expr::field(Expr::bound("line"), "amount"),
            ),
            100,
        );
        let bound = Env::with_view(Identity([4u8; 32]), Timestamp::from_millis(1), &view);
        let mut meter = Meter::default();
        let total = fold
            .evaluate(&bound, &mut meter, &Path::root().field("fold"))
            .expect("the fold runs");
        assert_eq!(total, Value::Decimal(Decimal::new(3900, 2)));
    }

    #[test]
    fn a_detached_view_holds_zero_records() {
        let fold = FoldExpr::over_source(
            FoldSource::collection(
                "line_item",
                KeySelector::Full,
                TypeExpr::Record(TypeId::new("line_item")),
            ),
            "line",
            "running",
            Expr::integer(0),
            Expr::binary(BinaryOp::Add, Expr::bound("running"), Expr::integer(1)),
            100,
        );
        let mut meter = Meter::default();
        let value = fold
            .evaluate(&env(), &mut meter, &Path::root().field("fold"))
            .expect("the fold runs");
        assert_eq!(value, Value::Integer(0));
        assert_eq!(
            DetachedView
                .records(&CollectionId::new("line_item"), &KeyRange::Full)
                .len(),
            0
        );
    }

    #[test]
    fn a_fold_inside_an_expression_type_checks_and_runs() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry).with(
            &ParamName::new("amounts"),
            TypeExpr::list(TypeExpr::Integer),
        );
        let expr = Expr::binary(
            BinaryOp::Multiply,
            Expr::Fold(Box::new(sum_of_list(10))),
            Expr::integer(2),
        );
        let ty = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect("the expression checks");
        assert_eq!(ty, TypeExpr::Integer);
        let mut bound = env();
        bound.bind(&ParamName::new("amounts"), list_of(4));
        assert_eq!(
            expr.run(&bound).expect("the expression runs"),
            Value::Integer(20)
        );
    }

    #[test]
    fn the_fold_round_trips_through_scale_and_json() {
        let fold = sum_of_list(25);
        let bytes = fold.encode();
        let back = FoldExpr::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(fold, back);
        let text = serde_json::to_string(&fold).expect("the fold serialises");
        let parsed: FoldExpr = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(fold, parsed);
    }

    #[test]
    fn free_names_leaves_out_the_fold_bindings() {
        let fold = sum_of_list(10);
        let mut bound = Vec::new();
        let mut found = Vec::new();
        fold.free_names(&mut bound, &mut found);
        assert_eq!(found, vec![ParamName::new("amounts")]);
    }

    #[test]
    fn a_collection_source_names_a_real_collection() {
        use crate::collections::{Collection, CollectionRegistry, PurposeDeclaration};
        let mut collection =
            Collection::new("line_item", "line_item", "reference", "The invoice lines.");
        collection.purpose = PurposeDeclaration::new("The lines carry billing.", &["billing"]);
        let held = [collection];
        let (registry, _) = CollectionRegistry::build(&held, &Path::root().field("collections"));

        let good = FoldExpr::over_source(
            FoldSource::collection(
                "line_item",
                KeySelector::Full,
                TypeExpr::Record(TypeId::new("line_item")),
            ),
            "line",
            "running",
            Expr::integer(0),
            Expr::bound("running"),
            10,
        );
        assert!(good
            .validate_source(&registry, &Path::root().field("fold"))
            .is_clean());

        let absent = FoldExpr::over_source(
            FoldSource::collection(
                "absent_collection",
                KeySelector::Full,
                TypeExpr::Record(TypeId::new("line_item")),
            ),
            "line",
            "running",
            Expr::integer(0),
            Expr::bound("running"),
            10,
        );
        let report = absent.validate_source(&registry, &Path::root().field("fold"));
        let found = report
            .find("fold.source.collection")
            .expect("the diagnostic sits on the collection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }
}
