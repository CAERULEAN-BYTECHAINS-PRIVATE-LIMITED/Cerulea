//! The typed expression language.
//!
//! An expression carries a declared type. The checker resolves that type
//! before a deployment goes live. The evaluator runs the expression against
//! bound values and produces one value.
//!
//! The language spans data construction and destruction, conditionals,
//! pattern matching over sum types, record field access, optional handling,
//! integer and fixed-point arithmetic, comparison, boolean logic, and error
//! handling with compensation.
//!
//! Every operation is exact. Every operation is deterministic. Two nodes
//! running the same expression over the same bindings reach the same value.

use crate::{
    types::{
        ArithmeticFault, CollectionId, Decimal, FieldName, Hash32, Identity, ParamName, RecordRef,
        Timestamp, TypeExpr, TypeId, TypeRegistry, Value, VariantName, MAX_DECIMAL_SCALE,
    },
    Diagnostic, DiagnosticCode, Path,
};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The step budget a single expression receives when a caller names no other
/// limit.
pub const DEFAULT_STEP_LIMIT: u64 = 100_000;

/// An operation over one operand.
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
pub enum UnaryOp {
    /// Reverse a truth value.
    Not,
    /// Reverse the sign of a number.
    Negate,
}

/// An operation over two operands.
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
pub enum BinaryOp {
    /// Add two numbers.
    Add,
    /// Subtract the right number from the left number.
    Subtract,
    /// Multiply two numbers.
    Multiply,
    /// Divide the left number by the right number.
    Divide,
    /// The remainder left by an integer division.
    Remainder,
    /// Report whether two values match.
    Equal,
    /// Report whether two values differ.
    NotEqual,
    /// Report whether the left value sorts below the right value.
    Less,
    /// Report whether the left value sorts at or below the right value.
    LessOrEqual,
    /// Report whether the left value sorts above the right value.
    Greater,
    /// Report whether the left value sorts at or above the right value.
    GreaterOrEqual,
    /// Report whether both truth values hold.
    And,
    /// Report whether either truth value holds.
    Or,
    /// Join two runs of text.
    ConcatText,
    /// Join two lists sharing one element type.
    AppendList,
}

impl BinaryOp {
    /// A word naming the operation for a diagnostic.
    pub fn label(&self) -> &'static str {
        match self {
            BinaryOp::Add => "addition",
            BinaryOp::Subtract => "subtraction",
            BinaryOp::Multiply => "multiplication",
            BinaryOp::Divide => "division",
            BinaryOp::Remainder => "remainder",
            BinaryOp::Equal => "equality",
            BinaryOp::NotEqual => "inequality",
            BinaryOp::Less => "less than",
            BinaryOp::LessOrEqual => "less than or equal",
            BinaryOp::Greater => "greater than",
            BinaryOp::GreaterOrEqual => "greater than or equal",
            BinaryOp::And => "conjunction",
            BinaryOp::Or => "disjunction",
            BinaryOp::ConcatText => "text join",
            BinaryOp::AppendList => "list append",
        }
    }
}

/// A named operation the Blueprint supplies.
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
pub enum Builtin {
    /// The block timestamp the Blockchain supplies for this transaction.
    Now,
    /// The identity that submitted this transaction.
    Caller,
    /// The Blueprint hash of a run of bytes or a run of text.
    HashOf,
    /// A timestamp moved forward by a span in milliseconds.
    TimestampPlus,
    /// The span in milliseconds between an earlier and a later timestamp.
    TimestampSince,
    /// The magnitude of a whole number.
    AbsoluteInteger,
    /// The smaller of two whole numbers.
    MinimumInteger,
    /// The larger of two whole numbers.
    MaximumInteger,
    /// The bytes of a run of text.
    BytesOfText,
}

impl Builtin {
    /// The count of arguments the operation takes.
    pub fn arity(&self) -> usize {
        match self {
            Builtin::Now | Builtin::Caller => 0,
            Builtin::HashOf | Builtin::AbsoluteInteger | Builtin::BytesOfText => 1,
            Builtin::TimestampPlus
            | Builtin::TimestampSince
            | Builtin::MinimumInteger
            | Builtin::MaximumInteger => 2,
        }
    }

    /// The name of the operation as the interface layer prints it.
    pub fn label(&self) -> &'static str {
        match self {
            Builtin::Now => "now",
            Builtin::Caller => "caller",
            Builtin::HashOf => "hash_of",
            Builtin::TimestampPlus => "timestamp_plus",
            Builtin::TimestampSince => "timestamp_since",
            Builtin::AbsoluteInteger => "absolute",
            Builtin::MinimumInteger => "minimum",
            Builtin::MaximumInteger => "maximum",
            Builtin::BytesOfText => "bytes_of_text",
        }
    }
}

/// A pattern in a match arm.
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
pub enum Pattern {
    /// One alternative of a sum type. The binding receives the data the
    /// alternative carries.
    Variant {
        /// The alternative this arm matches.
        variant: VariantName,
        /// The name the arm gives to the data.
        binding: Option<ParamName>,
    },
    /// One alternative of an enumeration.
    Choice {
        /// The alternative this arm matches.
        variant: VariantName,
    },
    /// A present optional. The binding receives the element.
    Present {
        /// The name the arm gives to the element.
        binding: ParamName,
    },
    /// An absent optional.
    Absent,
}

/// One arm of a match expression.
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
pub struct MatchArm {
    /// The pattern the arm matches.
    pub pattern: Pattern,
    /// The value the arm produces.
    pub body: Expr,
}

/// An expression.
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
pub enum Expr {
    /// A fixed value.
    Literal(Value),
    /// The value bound to a parameter or a local name.
    Bound(ParamName),
    /// One field read out of a record.
    Field {
        /// The record.
        base: Box<Expr>,
        /// The field read out of it.
        field: FieldName,
    },
    /// An operation over one operand.
    Unary {
        /// The operation.
        op: UnaryOp,
        /// The operand.
        operand: Box<Expr>,
    },
    /// An operation over two operands.
    Binary {
        /// The operation.
        op: BinaryOp,
        /// The left operand.
        left: Box<Expr>,
        /// The right operand.
        right: Box<Expr>,
    },
    /// A conditional.
    If {
        /// The truth value that selects a branch.
        condition: Box<Expr>,
        /// The value produced when the condition holds.
        then_branch: Box<Expr>,
        /// The value produced otherwise.
        else_branch: Box<Expr>,
    },
    /// A match over a sum type, an enumeration or an optional.
    Match {
        /// The value the arms inspect.
        scrutinee: Box<Expr>,
        /// The arms, in order.
        arms: Vec<MatchArm>,
        /// The value produced when zero arms match.
        fallback: Option<Box<Expr>>,
    },
    /// A local binding.
    Let {
        /// The name the binding takes.
        name: ParamName,
        /// The value bound to the name.
        value: Box<Expr>,
        /// The expression that reads the binding.
        body: Box<Expr>,
    },
    /// Build a record.
    MakeRecord {
        /// The declared record type.
        type_id: TypeId,
        /// The field values.
        fields: Vec<(FieldName, Expr)>,
    },
    /// Build an enumeration value.
    MakeEnumeration {
        /// The declared enumeration type.
        type_id: TypeId,
        /// The chosen alternative.
        variant: VariantName,
    },
    /// Build a sum value.
    MakeSum {
        /// The declared sum type.
        type_id: TypeId,
        /// The chosen alternative.
        variant: VariantName,
        /// The data the alternative carries.
        payload: Option<Box<Expr>>,
    },
    /// Build a list.
    MakeList {
        /// The element type.
        element: TypeExpr,
        /// The elements, in order.
        items: Vec<Expr>,
    },
    /// Build a map.
    MakeMap {
        /// The key type.
        key: TypeExpr,
        /// The value type.
        value: TypeExpr,
        /// The pairs.
        entries: Vec<(Expr, Expr)>,
    },
    /// Build an optional.
    MakeOptional {
        /// The element type.
        element: TypeExpr,
        /// The element, present or absent.
        item: Option<Box<Expr>>,
    },
    /// Build a reference to a record in a collection.
    MakeReference {
        /// The collection holding the record.
        collection: CollectionId,
        /// The primary key of the record.
        key: Box<Expr>,
    },
    /// The element of a present optional, or a supplied value.
    UnwrapOr {
        /// The optional.
        optional: Box<Expr>,
        /// The value produced when the optional is absent.
        fallback: Box<Expr>,
    },
    /// Report whether an optional holds an element.
    IsPresent(Box<Expr>),
    /// The count of elements in a list, a map, a run of text or a run of
    /// bytes.
    Length(Box<Expr>),
    /// Report whether a list holds a value.
    Contains {
        /// The list.
        list: Box<Expr>,
        /// The value sought.
        item: Box<Expr>,
    },
    /// Read one value out of a map. The result is an optional.
    Lookup {
        /// The map.
        map: Box<Expr>,
        /// The key sought.
        key: Box<Expr>,
    },
    /// Move a decimal to another scale without losing a digit.
    Rescale {
        /// The decimal.
        value: Box<Expr>,
        /// The scale the result carries.
        scale: u8,
    },
    /// Apply a named operation the Blueprint supplies.
    Call {
        /// The operation.
        function: Builtin,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// Run an expression and fall back to a compensation on a recoverable
    /// fault.
    ///
    /// Overflow, division by zero, precision loss and an absent value are
    /// recoverable. A step budget overrun aborts the whole evaluation.
    Attempt {
        /// The expression that runs first.
        body: Box<Expr>,
        /// The expression that runs after a recoverable fault.
        compensation: Box<Expr>,
    },
}

impl Expr {
    /// A literal whole number.
    pub fn integer(value: i128) -> Expr {
        Expr::Literal(Value::Integer(value))
    }

    /// A literal truth value.
    pub fn boolean(value: bool) -> Expr {
        Expr::Literal(Value::Bool(value))
    }

    /// A literal run of text.
    pub fn text(value: &str) -> Expr {
        Expr::Literal(Value::Text(value.to_string()))
    }

    /// A literal decimal.
    pub fn decimal(units: i128, scale: u8) -> Expr {
        Expr::Literal(Value::Decimal(Decimal::new(units, scale)))
    }

    /// A read of a bound name.
    pub fn bound(name: &str) -> Expr {
        Expr::Bound(ParamName::new(name))
    }

    /// A field read.
    pub fn field(base: Expr, field: &str) -> Expr {
        Expr::Field {
            base: Box::new(base),
            field: FieldName::new(field),
        }
    }

    /// An operation over two operands.
    pub fn binary(op: BinaryOp, left: Expr, right: Expr) -> Expr {
        Expr::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// A conditional.
    pub fn conditional(condition: Expr, then_branch: Expr, else_branch: Expr) -> Expr {
        Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        }
    }

    /// Collect every collection this expression names.
    pub fn collection_refs(&self, out: &mut Vec<CollectionId>) {
        match self {
            Expr::Literal(value) => {
                let mut found = Vec::new();
                value.infer_type().collection_refs(&mut found);
                out.extend(found);
            }
            Expr::MakeReference { collection, key } => {
                out.push(collection.clone());
                key.collection_refs(out);
            }
            Expr::Bound(_) => {}
            Expr::Field { base, .. } => base.collection_refs(out),
            Expr::Unary { operand, .. } => operand.collection_refs(out),
            Expr::Binary { left, right, .. } => {
                left.collection_refs(out);
                right.collection_refs(out);
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.collection_refs(out);
                then_branch.collection_refs(out);
                else_branch.collection_refs(out);
            }
            Expr::Match {
                scrutinee,
                arms,
                fallback,
            } => {
                scrutinee.collection_refs(out);
                for arm in arms {
                    arm.body.collection_refs(out);
                }
                if let Some(fallback) = fallback {
                    fallback.collection_refs(out);
                }
            }
            Expr::Let { value, body, .. } => {
                value.collection_refs(out);
                body.collection_refs(out);
            }
            Expr::MakeRecord { fields, .. } => {
                for (_, value) in fields {
                    value.collection_refs(out);
                }
            }
            Expr::MakeEnumeration { .. } => {}
            Expr::MakeSum { payload, .. } => {
                if let Some(payload) = payload {
                    payload.collection_refs(out);
                }
            }
            Expr::MakeList { element, items } => {
                element.collection_refs(out);
                for item in items {
                    item.collection_refs(out);
                }
            }
            Expr::MakeMap {
                key,
                value,
                entries,
            } => {
                key.collection_refs(out);
                value.collection_refs(out);
                for entry in entries {
                    entry.0.collection_refs(out);
                    entry.1.collection_refs(out);
                }
            }
            Expr::MakeOptional { element, item } => {
                element.collection_refs(out);
                if let Some(item) = item {
                    item.collection_refs(out);
                }
            }
            Expr::UnwrapOr { optional, fallback } => {
                optional.collection_refs(out);
                fallback.collection_refs(out);
            }
            Expr::IsPresent(inner) | Expr::Length(inner) => inner.collection_refs(out),
            Expr::Contains { list, item } => {
                list.collection_refs(out);
                item.collection_refs(out);
            }
            Expr::Lookup { map, key } => {
                map.collection_refs(out);
                key.collection_refs(out);
            }
            Expr::Rescale { value, .. } => value.collection_refs(out),
            Expr::Call { args, .. } => {
                for arg in args {
                    arg.collection_refs(out);
                }
            }
            Expr::Attempt { body, compensation } => {
                body.collection_refs(out);
                compensation.collection_refs(out);
            }
        }
    }

    /// Collect every bound name this expression reads without binding first.
    pub fn free_names(&self, bound: &mut Vec<ParamName>, out: &mut Vec<ParamName>) {
        match self {
            Expr::Bound(name) => {
                if !bound.contains(name) && !out.contains(name) {
                    out.push(name.clone());
                }
            }
            Expr::Let { name, value, body } => {
                value.free_names(bound, out);
                bound.push(name.clone());
                body.free_names(bound, out);
                bound.pop();
            }
            Expr::Match {
                scrutinee,
                arms,
                fallback,
            } => {
                scrutinee.free_names(bound, out);
                for arm in arms {
                    let binding = match &arm.pattern {
                        Pattern::Variant { binding, .. } => binding.clone(),
                        Pattern::Present { binding } => Some(binding.clone()),
                        _ => None,
                    };
                    match binding {
                        Some(name) => {
                            bound.push(name);
                            arm.body.free_names(bound, out);
                            bound.pop();
                        }
                        None => arm.body.free_names(bound, out),
                    }
                }
                if let Some(fallback) = fallback {
                    fallback.free_names(bound, out);
                }
            }
            Expr::Literal(_) | Expr::MakeEnumeration { .. } => {}
            Expr::Field { base, .. } => base.free_names(bound, out),
            Expr::Unary { operand, .. } => operand.free_names(bound, out),
            Expr::Binary { left, right, .. } => {
                left.free_names(bound, out);
                right.free_names(bound, out);
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                condition.free_names(bound, out);
                then_branch.free_names(bound, out);
                else_branch.free_names(bound, out);
            }
            Expr::MakeRecord { fields, .. } => {
                for (_, value) in fields {
                    value.free_names(bound, out);
                }
            }
            Expr::MakeSum { payload, .. } => {
                if let Some(payload) = payload {
                    payload.free_names(bound, out);
                }
            }
            Expr::MakeList { items, .. } => {
                for item in items {
                    item.free_names(bound, out);
                }
            }
            Expr::MakeMap { entries, .. } => {
                for entry in entries {
                    entry.0.free_names(bound, out);
                    entry.1.free_names(bound, out);
                }
            }
            Expr::MakeOptional { item, .. } => {
                if let Some(item) = item {
                    item.free_names(bound, out);
                }
            }
            Expr::MakeReference { key, .. } => key.free_names(bound, out),
            Expr::UnwrapOr { optional, fallback } => {
                optional.free_names(bound, out);
                fallback.free_names(bound, out);
            }
            Expr::IsPresent(inner) | Expr::Length(inner) => inner.free_names(bound, out),
            Expr::Contains { list, item } => {
                list.free_names(bound, out);
                item.free_names(bound, out);
            }
            Expr::Lookup { map, key } => {
                map.free_names(bound, out);
                key.free_names(bound, out);
            }
            Expr::Rescale { value, .. } => value.free_names(bound, out),
            Expr::Call { args, .. } => {
                for arg in args {
                    arg.free_names(bound, out);
                }
            }
            Expr::Attempt { body, compensation } => {
                body.free_names(bound, out);
                compensation.free_names(bound, out);
            }
        }
    }

    /// Replace every read of a name with a supplied expression.
    ///
    /// Composition uses this pass to inline a block body at a call site.
    pub fn substitute(&self, replacements: &BTreeMap<ParamName, Expr>) -> Expr {
        match self {
            Expr::Bound(name) => match replacements.get(name) {
                Some(replacement) => replacement.clone(),
                None => Expr::Bound(name.clone()),
            },
            Expr::Literal(value) => Expr::Literal(value.clone()),
            Expr::Field { base, field } => Expr::Field {
                base: Box::new(base.substitute(replacements)),
                field: field.clone(),
            },
            Expr::Unary { op, operand } => Expr::Unary {
                op: *op,
                operand: Box::new(operand.substitute(replacements)),
            },
            Expr::Binary { op, left, right } => Expr::Binary {
                op: *op,
                left: Box::new(left.substitute(replacements)),
                right: Box::new(right.substitute(replacements)),
            },
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => Expr::If {
                condition: Box::new(condition.substitute(replacements)),
                then_branch: Box::new(then_branch.substitute(replacements)),
                else_branch: Box::new(else_branch.substitute(replacements)),
            },
            Expr::Match {
                scrutinee,
                arms,
                fallback,
            } => {
                let arms = arms
                    .iter()
                    .map(|arm| {
                        let mut inner = replacements.clone();
                        match &arm.pattern {
                            Pattern::Variant {
                                binding: Some(name),
                                ..
                            } => {
                                inner.remove(name);
                            }
                            Pattern::Present { binding } => {
                                inner.remove(binding);
                            }
                            _ => {}
                        }
                        MatchArm {
                            pattern: arm.pattern.clone(),
                            body: arm.body.substitute(&inner),
                        }
                    })
                    .collect();
                Expr::Match {
                    scrutinee: Box::new(scrutinee.substitute(replacements)),
                    arms,
                    fallback: fallback
                        .as_ref()
                        .map(|inner| Box::new(inner.substitute(replacements))),
                }
            }
            Expr::Let { name, value, body } => {
                let mut inner = replacements.clone();
                inner.remove(name);
                Expr::Let {
                    name: name.clone(),
                    value: Box::new(value.substitute(replacements)),
                    body: Box::new(body.substitute(&inner)),
                }
            }
            Expr::MakeRecord { type_id, fields } => Expr::MakeRecord {
                type_id: type_id.clone(),
                fields: fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value.substitute(replacements)))
                    .collect(),
            },
            Expr::MakeEnumeration { type_id, variant } => Expr::MakeEnumeration {
                type_id: type_id.clone(),
                variant: variant.clone(),
            },
            Expr::MakeSum {
                type_id,
                variant,
                payload,
            } => Expr::MakeSum {
                type_id: type_id.clone(),
                variant: variant.clone(),
                payload: payload
                    .as_ref()
                    .map(|inner| Box::new(inner.substitute(replacements))),
            },
            Expr::MakeList { element, items } => Expr::MakeList {
                element: element.clone(),
                items: items
                    .iter()
                    .map(|item| item.substitute(replacements))
                    .collect(),
            },
            Expr::MakeMap {
                key,
                value,
                entries,
            } => Expr::MakeMap {
                key: key.clone(),
                value: value.clone(),
                entries: entries
                    .iter()
                    .map(|(left, right)| {
                        (
                            left.substitute(replacements),
                            right.substitute(replacements),
                        )
                    })
                    .collect(),
            },
            Expr::MakeOptional { element, item } => Expr::MakeOptional {
                element: element.clone(),
                item: item
                    .as_ref()
                    .map(|inner| Box::new(inner.substitute(replacements))),
            },
            Expr::MakeReference { collection, key } => Expr::MakeReference {
                collection: collection.clone(),
                key: Box::new(key.substitute(replacements)),
            },
            Expr::UnwrapOr { optional, fallback } => Expr::UnwrapOr {
                optional: Box::new(optional.substitute(replacements)),
                fallback: Box::new(fallback.substitute(replacements)),
            },
            Expr::IsPresent(inner) => Expr::IsPresent(Box::new(inner.substitute(replacements))),
            Expr::Length(inner) => Expr::Length(Box::new(inner.substitute(replacements))),
            Expr::Contains { list, item } => Expr::Contains {
                list: Box::new(list.substitute(replacements)),
                item: Box::new(item.substitute(replacements)),
            },
            Expr::Lookup { map, key } => Expr::Lookup {
                map: Box::new(map.substitute(replacements)),
                key: Box::new(key.substitute(replacements)),
            },
            Expr::Rescale { value, scale } => Expr::Rescale {
                value: Box::new(value.substitute(replacements)),
                scale: *scale,
            },
            Expr::Call { function, args } => Expr::Call {
                function: *function,
                args: args
                    .iter()
                    .map(|arg| arg.substitute(replacements))
                    .collect(),
            },
            Expr::Attempt { body, compensation } => Expr::Attempt {
                body: Box::new(body.substitute(replacements)),
                compensation: Box::new(compensation.substitute(replacements)),
            },
        }
    }
}

/// The types in scope while the checker walks an expression.
#[derive(Clone, Debug)]
pub struct TypeContext<'a> {
    /// The declared types.
    pub registry: &'a TypeRegistry<'a>,
    bindings: BTreeMap<String, TypeExpr>,
}

impl<'a> TypeContext<'a> {
    /// Start a context over a type registry.
    pub fn new(registry: &'a TypeRegistry<'a>) -> Self {
        TypeContext {
            registry,
            bindings: BTreeMap::new(),
        }
    }

    /// Add one name and its type.
    pub fn bind(&mut self, name: &ParamName, ty: TypeExpr) {
        self.bindings.insert(name.as_str().to_string(), ty);
    }

    /// A copy of this context carrying one more name.
    pub fn with(&self, name: &ParamName, ty: TypeExpr) -> TypeContext<'a> {
        let mut next = self.clone();
        next.bind(name, ty);
        next
    }

    /// Look up the type of a name.
    pub fn lookup(&self, name: &ParamName) -> Option<&TypeExpr> {
        self.bindings.get(name.as_str())
    }

    /// The names in scope, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.bindings.keys().map(|name| name.as_str()).collect()
    }
}

/// The values in scope while the evaluator walks an expression.
#[derive(Clone, Debug, Default)]
pub struct Env {
    bindings: BTreeMap<String, Value>,
    caller: Identity,
    now: Timestamp,
}

impl Env {
    /// Start an environment holding the caller and the block time.
    pub fn new(caller: Identity, now: Timestamp) -> Self {
        Env {
            bindings: BTreeMap::new(),
            caller,
            now,
        }
    }

    /// Add one name and its value.
    pub fn bind(&mut self, name: &ParamName, value: Value) {
        self.bindings.insert(name.as_str().to_string(), value);
    }

    /// A copy of this environment carrying one more name.
    pub fn with(&self, name: &ParamName, value: Value) -> Env {
        let mut next = self.clone();
        next.bind(name, value);
        next
    }

    /// Look up the value of a name.
    pub fn lookup(&self, name: &ParamName) -> Option<&Value> {
        self.bindings.get(name.as_str())
    }

    /// The identity that submitted the transaction.
    pub fn caller(&self) -> Identity {
        self.caller
    }

    /// The block time the Blockchain supplied.
    pub fn now(&self) -> Timestamp {
        self.now
    }

    /// The names in scope, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.bindings.keys().map(|name| name.as_str()).collect()
    }
}

/// The step budget of one evaluation.
///
/// The evaluator charges one step for every node it visits. A run that
/// passes the limit stops with a bound diagnostic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meter {
    used: u64,
    limit: u64,
}

impl Default for Meter {
    fn default() -> Self {
        Meter::new(DEFAULT_STEP_LIMIT)
    }
}

impl Meter {
    /// Start a meter with a step limit.
    pub fn new(limit: u64) -> Self {
        Meter { used: 0, limit }
    }

    /// The steps spent so far.
    pub fn used(&self) -> u64 {
        self.used
    }

    /// The step limit.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// The steps left.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Spend steps. A run past the limit stops the evaluation.
    pub fn charge(&mut self, amount: u64, path: &Path) -> Result<(), Diagnostic> {
        self.used = self.used.saturating_add(amount);
        if self.used > self.limit {
            return Err(Diagnostic::at(
                DiagnosticCode::BoundExceeded,
                path.location(),
                format!(
                    "The expression ran past its budget of {} steps.",
                    self.limit
                ),
                "Raise the declared step limit or reduce the work the expression does.".to_string(),
            ));
        }
        Ok(())
    }
}

/// Report whether a fault leaves room for a compensation.
pub fn is_recoverable(diagnostic: &Diagnostic) -> bool {
    matches!(
        diagnostic.code,
        DiagnosticCode::ArithmeticOverflow
            | DiagnosticCode::DivisionByZero
            | DiagnosticCode::PrecisionLoss
            | DiagnosticCode::MissingValue
    )
}

fn type_error(path: impl core::borrow::Borrow<Path>, problem: String, fix: String) -> Diagnostic {
    Diagnostic::at(
        DiagnosticCode::TypeMismatch,
        path.borrow().location(),
        problem,
        fix,
    )
}

fn unknown_name(path: &Path, name: &ParamName, known: &[&str]) -> Diagnostic {
    let listed = if known.is_empty() {
        "nothing".to_string()
    } else {
        known.join(", ")
    };
    Diagnostic::at(
        DiagnosticCode::DanglingReference,
        path.location(),
        format!("The expression reads `{name}`, which is out of scope."),
        format!("Read one of the names in scope here: {listed}."),
    )
}

impl Expr {
    /// Resolve the type of the expression.
    ///
    /// A clean result means the evaluator runs without a type fault.
    pub fn check(&self, ctx: &TypeContext, path: &Path) -> Result<TypeExpr, Diagnostic> {
        match self {
            Expr::Literal(value) => Ok(value.infer_type()),
            Expr::Bound(name) => ctx
                .lookup(name)
                .cloned()
                .ok_or_else(|| unknown_name(path, name, &ctx.names())),
            Expr::Field { base, field } => {
                let base_type = base.check(ctx, &path.field("base"))?;
                let record_id = match &base_type {
                    TypeExpr::Record(id) => id.clone(),
                    other => {
                        return Err(type_error(
                            path,
                            format!("The field read sits on {}.", other.describe()),
                            "Read a field out of a record value.".to_string(),
                        ))
                    }
                };
                let def = ctx.registry.record(&record_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The record `{record_id}` is absent from the Blueprint."),
                        format!("Declare a record type named `{record_id}`."),
                    )
                })?;
                def.field(field)
                    .map(|found| found.ty.clone())
                    .ok_or_else(|| {
                        Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.location(),
                            format!("The record `{record_id}` lacks the field `{field}`."),
                            format!("Read a field declared on `{record_id}`."),
                        )
                    })
            }
            Expr::Unary { op, operand } => {
                let inner = operand.check(ctx, &path.field("operand"))?;
                match (op, &inner) {
                    (UnaryOp::Not, TypeExpr::Bool) => Ok(TypeExpr::Bool),
                    (UnaryOp::Negate, TypeExpr::Integer) => Ok(TypeExpr::Integer),
                    (UnaryOp::Negate, TypeExpr::Decimal { scale }) => {
                        Ok(TypeExpr::Decimal { scale: *scale })
                    }
                    (UnaryOp::Not, other) => Err(type_error(
                        path,
                        format!("The reverse operation sits on {}.", other.describe()),
                        "Apply the reverse operation to a truth value.".to_string(),
                    )),
                    (UnaryOp::Negate, other) => Err(type_error(
                        path,
                        format!("The sign change sits on {}.", other.describe()),
                        "Apply the sign change to a whole number or a decimal.".to_string(),
                    )),
                }
            }
            Expr::Binary { op, left, right } => {
                let left_type = left.check(ctx, &path.field("left"))?;
                let right_type = right.check(ctx, &path.field("right"))?;
                check_binary(*op, &left_type, &right_type, path)
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition_type = condition.check(ctx, &path.field("condition"))?;
                if condition_type != TypeExpr::Bool {
                    return Err(type_error(
                        path.field("condition"),
                        format!("The condition carries {}.", condition_type.describe()),
                        "Give the condition a truth value.".to_string(),
                    ));
                }
                let then_type = then_branch.check(ctx, &path.field("then_branch"))?;
                let else_type = else_branch.check(ctx, &path.field("else_branch"))?;
                if then_type != else_type {
                    return Err(type_error(
                        path,
                        format!(
                            "One branch carries {} and the other carries {}.",
                            then_type.describe(),
                            else_type.describe()
                        ),
                        "Give both branches one type.".to_string(),
                    ));
                }
                Ok(then_type)
            }
            Expr::Match {
                scrutinee,
                arms,
                fallback,
            } => check_match(ctx, path, scrutinee, arms, fallback),
            Expr::Let { name, value, body } => {
                let value_type = value.check(ctx, &path.field("value"))?;
                let inner = ctx.with(name, value_type);
                body.check(&inner, &path.field("body"))
            }
            Expr::MakeRecord { type_id, fields } => {
                let def = ctx.registry.record(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The record `{type_id}` is absent from the Blueprint."),
                        format!("Declare a record type named `{type_id}`."),
                    )
                })?;
                for declared in &def.fields {
                    let supplied = fields
                        .iter()
                        .find(|(name, _)| name == &declared.name)
                        .ok_or_else(|| {
                            Diagnostic::at(
                                DiagnosticCode::MissingValue,
                                path.field(declared.name.as_str()).location(),
                                format!(
                                    "The record build leaves out the field `{}`.",
                                    declared.name
                                ),
                                format!("Give the field `{}` a value.", declared.name),
                            )
                        })?;
                    let found = supplied.1.check(ctx, &path.field(declared.name.as_str()))?;
                    if found != declared.ty {
                        return Err(type_error(
                            path.field(declared.name.as_str()),
                            format!(
                                "The field `{}` carries {} and the record declares {}.",
                                declared.name,
                                found.describe(),
                                declared.ty.describe()
                            ),
                            format!("Supply {} for `{}`.", declared.ty.describe(), declared.name),
                        ));
                    }
                }
                for (name, _) in fields {
                    if def.field(name).is_none() {
                        return Err(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.field(name.as_str()).location(),
                            format!("The record build names `{name}`, which `{type_id}` lacks."),
                            format!("Remove `{name}` or declare it on `{type_id}`."),
                        ));
                    }
                }
                Ok(TypeExpr::Record(type_id.clone()))
            }
            Expr::MakeEnumeration { type_id, variant } => {
                let def = ctx.registry.enumeration(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The enumeration `{type_id}` is absent from the Blueprint."),
                        format!("Declare an enumeration named `{type_id}`."),
                    )
                })?;
                if def.variants.contains(variant) {
                    Ok(TypeExpr::Enumeration(type_id.clone()))
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The enumeration `{type_id}` lacks the alternative `{variant}`."),
                        format!("Choose an alternative declared on `{type_id}`."),
                    ))
                }
            }
            Expr::MakeSum {
                type_id,
                variant,
                payload,
            } => {
                let def = ctx.registry.sum(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The sum type `{type_id}` is absent from the Blueprint."),
                        format!("Declare a sum type named `{type_id}`."),
                    )
                })?;
                let declared = def.variant(variant).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The sum type `{type_id}` lacks the alternative `{variant}`."),
                        format!("Choose an alternative declared on `{type_id}`."),
                    )
                })?;
                match (&declared.payload, payload) {
                    (Some(expected), Some(supplied)) => {
                        let found = supplied.check(ctx, &path.field("payload"))?;
                        if &found != expected {
                            return Err(type_error(
                                path.field("payload"),
                                format!(
                                    "The alternative `{variant}` carries {} and the build supplies {}.",
                                    expected.describe(),
                                    found.describe()
                                ),
                                format!("Supply {} here.", expected.describe()),
                            ));
                        }
                        Ok(TypeExpr::Sum(type_id.clone()))
                    }
                    (None, None) => Ok(TypeExpr::Sum(type_id.clone())),
                    (Some(expected), None) => Err(Diagnostic::at(
                        DiagnosticCode::MissingValue,
                        path.field("payload").location(),
                        format!(
                            "The alternative `{variant}` carries {}.",
                            expected.describe()
                        ),
                        format!("Supply {} for `{variant}`.", expected.describe()),
                    )),
                    (None, Some(_)) => Err(type_error(
                        path.field("payload"),
                        format!("The alternative `{variant}` carries no data."),
                        format!("Remove the data supplied for `{variant}`."),
                    )),
                }
            }
            Expr::MakeList { element, items } => {
                element.validate(ctx.registry, &path.field("element"))?;
                for (position, item) in items.iter().enumerate() {
                    let found = item.check(ctx, &path.index(position))?;
                    if &found != element {
                        return Err(type_error(
                            path.index(position),
                            format!(
                                "The element carries {} and the list declares {}.",
                                found.describe(),
                                element.describe()
                            ),
                            format!("Supply {} for every element.", element.describe()),
                        ));
                    }
                }
                Ok(TypeExpr::list(element.clone()))
            }
            Expr::MakeMap {
                key,
                value,
                entries,
            } => {
                let map_type = TypeExpr::map(key.clone(), value.clone());
                map_type.validate(ctx.registry, path)?;
                for (position, entry) in entries.iter().enumerate() {
                    let here = path.index(position);
                    let found_key = entry.0.check(ctx, &here.field("key"))?;
                    if &found_key != key {
                        return Err(type_error(
                            here.field("key"),
                            format!(
                                "The key carries {} and the map declares {}.",
                                found_key.describe(),
                                key.describe()
                            ),
                            format!("Supply {} for every key.", key.describe()),
                        ));
                    }
                    let found_value = entry.1.check(ctx, &here.field("value"))?;
                    if &found_value != value {
                        return Err(type_error(
                            here.field("value"),
                            format!(
                                "The value carries {} and the map declares {}.",
                                found_value.describe(),
                                value.describe()
                            ),
                            format!("Supply {} for every value.", value.describe()),
                        ));
                    }
                }
                Ok(map_type)
            }
            Expr::MakeOptional { element, item } => {
                element.validate(ctx.registry, &path.field("element"))?;
                if let Some(item) = item {
                    let found = item.check(ctx, &path.field("item"))?;
                    if &found != element {
                        return Err(type_error(
                            path.field("item"),
                            format!(
                                "The element carries {} and the optional declares {}.",
                                found.describe(),
                                element.describe()
                            ),
                            format!("Supply {} here.", element.describe()),
                        ));
                    }
                }
                Ok(TypeExpr::optional(element.clone()))
            }
            Expr::MakeReference { collection, key } => {
                key.check(ctx, &path.field("key"))?;
                Ok(TypeExpr::Reference(collection.clone()))
            }
            Expr::UnwrapOr { optional, fallback } => {
                let optional_type = optional.check(ctx, &path.field("optional"))?;
                let element = match optional_type {
                    TypeExpr::Optional(inner) => *inner,
                    other => {
                        return Err(type_error(
                            path.field("optional"),
                            format!("The unwrap sits on {}.", other.describe()),
                            "Apply the unwrap to an optional value.".to_string(),
                        ))
                    }
                };
                let fallback_type = fallback.check(ctx, &path.field("fallback"))?;
                if fallback_type != element {
                    return Err(type_error(
                        path.field("fallback"),
                        format!(
                            "The fallback carries {} and the element is {}.",
                            fallback_type.describe(),
                            element.describe()
                        ),
                        format!("Supply {} as the fallback.", element.describe()),
                    ));
                }
                Ok(element)
            }
            Expr::IsPresent(inner) => {
                let inner_type = inner.check(ctx, &path.field("value"))?;
                match inner_type {
                    TypeExpr::Optional(_) => Ok(TypeExpr::Bool),
                    other => Err(type_error(
                        path,
                        format!("The presence test sits on {}.", other.describe()),
                        "Apply the presence test to an optional value.".to_string(),
                    )),
                }
            }
            Expr::Length(inner) => {
                let inner_type = inner.check(ctx, &path.field("value"))?;
                match inner_type {
                    TypeExpr::List(_) | TypeExpr::Map { .. } | TypeExpr::Text | TypeExpr::Bytes => {
                        Ok(TypeExpr::Integer)
                    }
                    other => Err(type_error(
                        path,
                        format!("The length reads {}.", other.describe()),
                        "Read the length of a list, a map, text or bytes.".to_string(),
                    )),
                }
            }
            Expr::Contains { list, item } => {
                let list_type = list.check(ctx, &path.field("list"))?;
                let element = match list_type {
                    TypeExpr::List(inner) => *inner,
                    other => {
                        return Err(type_error(
                            path.field("list"),
                            format!("The membership test sits on {}.", other.describe()),
                            "Apply the membership test to a list.".to_string(),
                        ))
                    }
                };
                let item_type = item.check(ctx, &path.field("item"))?;
                if item_type != element {
                    return Err(type_error(
                        path.field("item"),
                        format!(
                            "The sought value carries {} and the list holds {}.",
                            item_type.describe(),
                            element.describe()
                        ),
                        format!("Supply {} here.", element.describe()),
                    ));
                }
                Ok(TypeExpr::Bool)
            }
            Expr::Lookup { map, key } => {
                let map_type = map.check(ctx, &path.field("map"))?;
                let (key_type, value_type) = match map_type {
                    TypeExpr::Map { key, value } => (*key, *value),
                    other => {
                        return Err(type_error(
                            path.field("map"),
                            format!("The lookup sits on {}.", other.describe()),
                            "Apply the lookup to a map.".to_string(),
                        ))
                    }
                };
                let found = key.check(ctx, &path.field("key"))?;
                if found != key_type {
                    return Err(type_error(
                        path.field("key"),
                        format!(
                            "The key carries {} and the map keys are {}.",
                            found.describe(),
                            key_type.describe()
                        ),
                        format!("Supply {} as the key.", key_type.describe()),
                    ));
                }
                Ok(TypeExpr::optional(value_type))
            }
            Expr::Rescale { value, scale } => {
                if *scale > MAX_DECIMAL_SCALE {
                    return Err(Diagnostic::at(
                        DiagnosticCode::ValueOutOfRange,
                        path.location(),
                        format!("The requested scale {scale} runs past the maximum of 30."),
                        "Choose a scale of 30 or below.".to_string(),
                    ));
                }
                let value_type = value.check(ctx, &path.field("value"))?;
                match value_type {
                    TypeExpr::Decimal { .. } => Ok(TypeExpr::Decimal { scale: *scale }),
                    other => Err(type_error(
                        path,
                        format!("The rescale sits on {}.", other.describe()),
                        "Apply the rescale to a decimal value.".to_string(),
                    )),
                }
            }
            Expr::Call { function, args } => check_call(ctx, path, *function, args),
            Expr::Attempt { body, compensation } => {
                let body_type = body.check(ctx, &path.field("body"))?;
                let compensation_type = compensation.check(ctx, &path.field("compensation"))?;
                if body_type != compensation_type {
                    return Err(type_error(
                        path,
                        format!(
                            "The body carries {} and the compensation carries {}.",
                            body_type.describe(),
                            compensation_type.describe()
                        ),
                        "Give the body and the compensation one type.".to_string(),
                    ));
                }
                Ok(body_type)
            }
        }
    }

    /// Run the expression against bound values.
    pub fn evaluate(&self, env: &Env, meter: &mut Meter, path: &Path) -> Result<Value, Diagnostic> {
        meter.charge(1, path)?;
        match self {
            Expr::Literal(value) => Ok(value.clone()),
            Expr::Bound(name) => env
                .lookup(name)
                .cloned()
                .ok_or_else(|| unknown_name(path, name, &env.names())),
            Expr::Field { base, field } => {
                let value = base.evaluate(env, meter, &path.field("base"))?;
                match value {
                    Value::Record(record) => record.field(field).cloned().ok_or_else(|| {
                        Diagnostic::at(
                            DiagnosticCode::MissingValue,
                            path.location(),
                            format!("The record value lacks the field `{field}`."),
                            format!("Give the field `{field}` a value before the read."),
                        )
                    }),
                    other => Err(type_error(
                        path,
                        format!("The field read sits on {}.", other.infer_type().describe()),
                        "Read a field out of a record value.".to_string(),
                    )),
                }
            }
            Expr::Unary { op, operand } => {
                let value = operand.evaluate(env, meter, &path.field("operand"))?;
                match (op, value) {
                    (UnaryOp::Not, Value::Bool(inner)) => Ok(Value::Bool(!inner)),
                    (UnaryOp::Negate, Value::Integer(inner)) => inner
                        .checked_neg()
                        .map(Value::Integer)
                        .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
                    (UnaryOp::Negate, Value::Decimal(inner)) => inner
                        .negate()
                        .map(Value::Decimal)
                        .map_err(|fault| fault.diagnose(path)),
                    (_, other) => Err(type_error(
                        path,
                        format!("The operation sits on {}.", other.infer_type().describe()),
                        "Apply the operation to a value of the declared type.".to_string(),
                    )),
                }
            }
            Expr::Binary { op, left, right } => {
                let left_value = left.evaluate(env, meter, &path.field("left"))?;
                let right_value = right.evaluate(env, meter, &path.field("right"))?;
                apply_binary(*op, left_value, right_value, path)
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let chosen = condition.evaluate(env, meter, &path.field("condition"))?;
                match chosen {
                    Value::Bool(true) => {
                        then_branch.evaluate(env, meter, &path.field("then_branch"))
                    }
                    Value::Bool(false) => {
                        else_branch.evaluate(env, meter, &path.field("else_branch"))
                    }
                    other => Err(type_error(
                        path.field("condition"),
                        format!("The condition carries {}.", other.infer_type().describe()),
                        "Give the condition a truth value.".to_string(),
                    )),
                }
            }
            Expr::Match {
                scrutinee,
                arms,
                fallback,
            } => {
                let subject = scrutinee.evaluate(env, meter, &path.field("scrutinee"))?;
                for (position, arm) in arms.iter().enumerate() {
                    let here = path.field("arms").index(position);
                    if let Some(inner) = match_arm(&subject, arm, env) {
                        return arm.body.evaluate(&inner, meter, &here.field("body"));
                    }
                }
                match fallback {
                    Some(fallback) => fallback.evaluate(env, meter, &path.field("fallback")),
                    None => Err(Diagnostic::at(
                        DiagnosticCode::MissingValue,
                        path.location(),
                        "Zero arms match the value supplied.".to_string(),
                        "Add an arm for every alternative, or add a fallback.".to_string(),
                    )),
                }
            }
            Expr::Let { name, value, body } => {
                let bound = value.evaluate(env, meter, &path.field("value"))?;
                let inner = env.with(name, bound);
                body.evaluate(&inner, meter, &path.field("body"))
            }
            Expr::MakeRecord { type_id, fields } => {
                let mut built = Vec::with_capacity(fields.len());
                for (name, expr) in fields {
                    let value = expr.evaluate(env, meter, &path.field(name.as_str()))?;
                    built.push((name.clone(), value));
                }
                Ok(Value::record(type_id.clone(), built))
            }
            Expr::MakeEnumeration { type_id, variant } => {
                Ok(Value::enumeration(type_id.clone(), variant.clone()))
            }
            Expr::MakeSum {
                type_id,
                variant,
                payload,
            } => {
                let built = match payload {
                    Some(expr) => Some(expr.evaluate(env, meter, &path.field("payload"))?),
                    None => None,
                };
                Ok(Value::sum(type_id.clone(), variant.clone(), built))
            }
            Expr::MakeList { element, items } => {
                let mut built = Vec::with_capacity(items.len());
                for (position, item) in items.iter().enumerate() {
                    built.push(item.evaluate(env, meter, &path.index(position))?);
                }
                Ok(Value::list(element.clone(), built))
            }
            Expr::MakeMap {
                key,
                value,
                entries,
            } => {
                let mut built = Vec::with_capacity(entries.len());
                for (position, entry) in entries.iter().enumerate() {
                    let here = path.index(position);
                    let key_value = entry.0.evaluate(env, meter, &here.field("key"))?;
                    let value_value = entry.1.evaluate(env, meter, &here.field("value"))?;
                    built.push((key_value, value_value));
                }
                Ok(Value::map(key.clone(), value.clone(), built))
            }
            Expr::MakeOptional { element, item } => match item {
                Some(expr) => {
                    let value = expr.evaluate(env, meter, &path.field("item"))?;
                    Ok(Value::some(element.clone(), value))
                }
                None => Ok(Value::none(element.clone())),
            },
            Expr::MakeReference { collection, key } => {
                let key_value = key.evaluate(env, meter, &path.field("key"))?;
                Ok(Value::Reference(RecordRef {
                    collection: collection.clone(),
                    key: Box::new(key_value),
                }))
            }
            Expr::UnwrapOr { optional, fallback } => {
                let value = optional.evaluate(env, meter, &path.field("optional"))?;
                match value {
                    Value::Optional(inner) => match inner.item {
                        Some(item) => Ok(*item),
                        None => fallback.evaluate(env, meter, &path.field("fallback")),
                    },
                    other => Err(type_error(
                        path,
                        format!("The unwrap sits on {}.", other.infer_type().describe()),
                        "Apply the unwrap to an optional value.".to_string(),
                    )),
                }
            }
            Expr::IsPresent(inner) => {
                let value = inner.evaluate(env, meter, &path.field("value"))?;
                match value {
                    Value::Optional(optional) => Ok(Value::Bool(optional.item.is_some())),
                    other => Err(type_error(
                        path,
                        format!(
                            "The presence test sits on {}.",
                            other.infer_type().describe()
                        ),
                        "Apply the presence test to an optional value.".to_string(),
                    )),
                }
            }
            Expr::Length(inner) => {
                let value = inner.evaluate(env, meter, &path.field("value"))?;
                let count = match value {
                    Value::List(list) => list.items.len(),
                    Value::Map(map) => map.entries.len(),
                    Value::Text(text) => text.chars().count(),
                    Value::Bytes(bytes) => bytes.len(),
                    other => {
                        return Err(type_error(
                            path,
                            format!("The length reads {}.", other.infer_type().describe()),
                            "Read the length of a list, a map, text or bytes.".to_string(),
                        ))
                    }
                };
                Ok(Value::Integer(count as i128))
            }
            Expr::Contains { list, item } => {
                let list_value = list.evaluate(env, meter, &path.field("list"))?;
                let sought = item.evaluate(env, meter, &path.field("item"))?;
                match list_value {
                    Value::List(inner) => {
                        meter.charge(inner.items.len() as u64, path)?;
                        Ok(Value::Bool(inner.items.contains(&sought)))
                    }
                    other => Err(type_error(
                        path,
                        format!(
                            "The membership test sits on {}.",
                            other.infer_type().describe()
                        ),
                        "Apply the membership test to a list.".to_string(),
                    )),
                }
            }
            Expr::Lookup { map, key } => {
                let map_value = map.evaluate(env, meter, &path.field("map"))?;
                let sought = key.evaluate(env, meter, &path.field("key"))?;
                match map_value {
                    Value::Map(inner) => {
                        meter.charge(inner.entries.len() as u64, path)?;
                        let found = inner
                            .entries
                            .iter()
                            .find(|(candidate, _)| candidate == &sought)
                            .map(|(_, value)| value.clone());
                        Ok(match found {
                            Some(value) => Value::some(inner.value.clone(), value),
                            None => Value::none(inner.value.clone()),
                        })
                    }
                    other => Err(type_error(
                        path,
                        format!("The lookup sits on {}.", other.infer_type().describe()),
                        "Apply the lookup to a map.".to_string(),
                    )),
                }
            }
            Expr::Rescale { value, scale } => {
                let inner = value.evaluate(env, meter, &path.field("value"))?;
                match inner {
                    Value::Decimal(decimal) => decimal
                        .rescale(*scale)
                        .map(Value::Decimal)
                        .map_err(|fault| fault.diagnose(path)),
                    other => Err(type_error(
                        path,
                        format!("The rescale sits on {}.", other.infer_type().describe()),
                        "Apply the rescale to a decimal value.".to_string(),
                    )),
                }
            }
            Expr::Call { function, args } => {
                let mut values = Vec::with_capacity(args.len());
                for (position, arg) in args.iter().enumerate() {
                    values.push(arg.evaluate(env, meter, &path.index(position))?);
                }
                apply_call(*function, values, env, path)
            }
            Expr::Attempt { body, compensation } => {
                match body.evaluate(env, meter, &path.field("body")) {
                    Ok(value) => Ok(value),
                    Err(fault) => {
                        if is_recoverable(&fault) {
                            compensation.evaluate(env, meter, &path.field("compensation"))
                        } else {
                            Err(fault)
                        }
                    }
                }
            }
        }
    }

    /// Run the expression with a fresh meter carrying the default budget.
    pub fn run(&self, env: &Env) -> Result<Value, Diagnostic> {
        let mut meter = Meter::default();
        self.evaluate(env, &mut meter, &Path::root().field("expression"))
    }
}

fn match_arm(subject: &Value, arm: &MatchArm, env: &Env) -> Option<Env> {
    match (&arm.pattern, subject) {
        (Pattern::Variant { variant, binding }, Value::Sum(value)) => {
            if &value.variant != variant {
                return None;
            }
            match (binding, &value.payload) {
                (Some(name), Some(payload)) => Some(env.with(name, (**payload).clone())),
                (None, _) => Some(env.clone()),
                (Some(_), None) => None,
            }
        }
        (Pattern::Choice { variant }, Value::Enumeration(value)) => {
            if &value.variant == variant {
                Some(env.clone())
            } else {
                None
            }
        }
        (Pattern::Present { binding }, Value::Optional(value)) => value
            .item
            .as_ref()
            .map(|item| env.with(binding, (**item).clone())),
        (Pattern::Absent, Value::Optional(value)) => {
            if value.item.is_none() {
                Some(env.clone())
            } else {
                None
            }
        }
        _ => None,
    }
}

fn check_match(
    ctx: &TypeContext,
    path: &Path,
    scrutinee: &Expr,
    arms: &[MatchArm],
    fallback: &Option<Box<Expr>>,
) -> Result<TypeExpr, Diagnostic> {
    let subject = scrutinee.check(ctx, &path.field("scrutinee"))?;
    if arms.is_empty() && fallback.is_none() {
        return Err(Diagnostic::at(
            DiagnosticCode::SectionEmpty,
            path.location(),
            "The match holds zero arms.".to_string(),
            "Add one arm for every alternative the value carries.".to_string(),
        ));
    }
    let mut result: Option<TypeExpr> = None;
    let mut covered: Vec<VariantName> = Vec::new();
    let mut saw_present = false;
    let mut saw_absent = false;

    for (position, arm) in arms.iter().enumerate() {
        let here = path.field("arms").index(position);
        let inner = match (&arm.pattern, &subject) {
            (Pattern::Variant { variant, binding }, TypeExpr::Sum(type_id)) => {
                let def = ctx.registry.sum(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.location(),
                        format!("The sum type `{type_id}` is absent from the Blueprint."),
                        format!("Declare a sum type named `{type_id}`."),
                    )
                })?;
                let declared = def.variant(variant).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.location(),
                        format!("The sum type `{type_id}` lacks the alternative `{variant}`."),
                        format!("Match an alternative declared on `{type_id}`."),
                    )
                })?;
                covered.push(variant.clone());
                match (binding, &declared.payload) {
                    (Some(name), Some(payload)) => ctx.with(name, payload.clone()),
                    (Some(name), None) =>
                        return Err(type_error(
                            here.clone(),
                            format!("The alternative `{variant}` carries no data."),
                            format!("Remove the binding `{name}` from this arm."),
                        )),
                    (None, _) => ctx.clone(),
                }
            },
            (Pattern::Choice { variant }, TypeExpr::Enumeration(type_id)) => {
                let def = ctx.registry.enumeration(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.location(),
                        format!("The enumeration `{type_id}` is absent from the Blueprint."),
                        format!("Declare an enumeration named `{type_id}`."),
                    )
                })?;
                if !def.variants.contains(variant) {
                    return Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.location(),
                        format!("The enumeration `{type_id}` lacks the alternative `{variant}`."),
                        format!("Match an alternative declared on `{type_id}`."),
                    ));
                }
                covered.push(variant.clone());
                ctx.clone()
            },
            (Pattern::Present { binding }, TypeExpr::Optional(element)) => {
                saw_present = true;
                ctx.with(binding, element.as_ref().clone())
            },
            (Pattern::Absent, TypeExpr::Optional(_)) => {
                saw_absent = true;
                ctx.clone()
            },
            _ =>
                return Err(type_error(
                    here.clone(),
                    format!("The arm matches a shape other than {}.", subject.describe()),
                    "Match a sum type with variant arms, an enumeration with choice arms, and an optional with present and absent arms."
                        .to_string(),
                )),
        };
        let arm_type = arm.body.check(&inner, &here.field("body"))?;
        match &result {
            None => result = Some(arm_type),
            Some(first) => {
                if first != &arm_type {
                    return Err(type_error(
                        here.field("body"),
                        format!(
                            "One arm carries {} and another carries {}.",
                            first.describe(),
                            arm_type.describe()
                        ),
                        "Give every arm one type.".to_string(),
                    ));
                }
            }
        }
    }

    if let Some(fallback) = fallback {
        let fallback_type = fallback.check(ctx, &path.field("fallback"))?;
        match &result {
            None => result = Some(fallback_type),
            Some(first) => {
                if first != &fallback_type {
                    return Err(type_error(
                        path.field("fallback"),
                        format!(
                            "The arms carry {} and the fallback carries {}.",
                            first.describe(),
                            fallback_type.describe()
                        ),
                        "Give the fallback the type the arms carry.".to_string(),
                    ));
                }
            }
        }
    } else {
        let missing = missing_alternatives(ctx, &subject, &covered, saw_present, saw_absent);
        if let Some(word) = missing {
            return Err(Diagnostic::at(
                DiagnosticCode::MissingValue,
                path.location(),
                format!("The match leaves the alternative `{word}` uncovered."),
                format!("Add an arm for `{word}`, or add a fallback."),
            ));
        }
    }

    result.ok_or_else(|| {
        Diagnostic::at(
            DiagnosticCode::SectionEmpty,
            path.location(),
            "The match produces no value.".to_string(),
            "Add one arm for every alternative the value carries.".to_string(),
        )
    })
}

fn missing_alternatives(
    ctx: &TypeContext,
    subject: &TypeExpr,
    covered: &[VariantName],
    saw_present: bool,
    saw_absent: bool,
) -> Option<String> {
    match subject {
        TypeExpr::Sum(type_id) => {
            let def = ctx.registry.sum(type_id)?;
            def.variants
                .iter()
                .find(|variant| !covered.contains(&variant.name))
                .map(|variant| variant.name.as_str().to_string())
        }
        TypeExpr::Enumeration(type_id) => {
            let def = ctx.registry.enumeration(type_id)?;
            def.variants
                .iter()
                .find(|variant| !covered.contains(variant))
                .map(|variant| variant.as_str().to_string())
        }
        TypeExpr::Optional(_) => {
            if saw_present && saw_absent {
                None
            } else if saw_present {
                Some("absent".to_string())
            } else {
                Some("present".to_string())
            }
        }
        _ => None,
    }
}

fn check_binary(
    op: BinaryOp,
    left: &TypeExpr,
    right: &TypeExpr,
    path: &Path,
) -> Result<TypeExpr, Diagnostic> {
    match op {
        BinaryOp::Add | BinaryOp::Subtract => match (left, right) {
            (TypeExpr::Integer, TypeExpr::Integer) => Ok(TypeExpr::Integer),
            (TypeExpr::Decimal { scale: a }, TypeExpr::Decimal { scale: b }) if a == b => {
                Ok(TypeExpr::Decimal { scale: *a })
            }
            _ => Err(arithmetic_mismatch(op, left, right, path)),
        },
        BinaryOp::Multiply => match (left, right) {
            (TypeExpr::Integer, TypeExpr::Integer) => Ok(TypeExpr::Integer),
            (TypeExpr::Decimal { scale: a }, TypeExpr::Decimal { scale: b }) => {
                let scale = a.checked_add(*b).unwrap_or(u8::MAX);
                if scale > MAX_DECIMAL_SCALE {
                    return Err(Diagnostic::at(
                        DiagnosticCode::ValueOutOfRange,
                        path.location(),
                        format!("The product carries scale {scale}, above the maximum of 30."),
                        "Rescale one operand down before the multiplication.".to_string(),
                    ));
                }
                Ok(TypeExpr::Decimal { scale })
            }
            _ => Err(arithmetic_mismatch(op, left, right, path)),
        },
        BinaryOp::Divide => match (left, right) {
            (TypeExpr::Integer, TypeExpr::Integer) => Ok(TypeExpr::Integer),
            (TypeExpr::Decimal { scale: a }, TypeExpr::Decimal { scale: b }) if a == b => {
                Ok(TypeExpr::Decimal { scale: *a })
            }
            _ => Err(arithmetic_mismatch(op, left, right, path)),
        },
        BinaryOp::Remainder => match (left, right) {
            (TypeExpr::Integer, TypeExpr::Integer) => Ok(TypeExpr::Integer),
            _ => Err(arithmetic_mismatch(op, left, right, path)),
        },
        BinaryOp::Equal | BinaryOp::NotEqual => {
            if left == right {
                Ok(TypeExpr::Bool)
            } else {
                Err(type_error(
                    path.clone(),
                    format!(
                        "The comparison holds {} on the left and {} on the right.",
                        left.describe(),
                        right.describe()
                    ),
                    "Compare two values sharing one type.".to_string(),
                ))
            }
        }
        BinaryOp::Less | BinaryOp::LessOrEqual | BinaryOp::Greater | BinaryOp::GreaterOrEqual => {
            if left != right {
                return Err(type_error(
                    path.clone(),
                    format!(
                        "The ordering holds {} on the left and {} on the right.",
                        left.describe(),
                        right.describe()
                    ),
                    "Order two values sharing one type.".to_string(),
                ));
            }
            if left.is_key_safe() {
                Ok(TypeExpr::Bool)
            } else {
                Err(type_error(
                    path.clone(),
                    format!(
                        "The ordering sits on {}, which lacks a total order.",
                        left.describe()
                    ),
                    "Order whole numbers, decimals, text, bytes, timestamps, identities or hashes."
                        .to_string(),
                ))
            }
        }
        BinaryOp::And | BinaryOp::Or => match (left, right) {
            (TypeExpr::Bool, TypeExpr::Bool) => Ok(TypeExpr::Bool),
            _ => Err(type_error(
                path.clone(),
                format!(
                    "The {} holds {} on the left and {} on the right.",
                    op.label(),
                    left.describe(),
                    right.describe()
                ),
                "Give both sides a truth value.".to_string(),
            )),
        },
        BinaryOp::ConcatText => match (left, right) {
            (TypeExpr::Text, TypeExpr::Text) => Ok(TypeExpr::Text),
            _ => Err(type_error(
                path.clone(),
                format!(
                    "The text join holds {} on the left and {} on the right.",
                    left.describe(),
                    right.describe()
                ),
                "Give both sides text.".to_string(),
            )),
        },
        BinaryOp::AppendList => match (left, right) {
            (TypeExpr::List(a), TypeExpr::List(b)) if a == b => Ok(left.clone()),
            _ => Err(type_error(
                path.clone(),
                format!(
                    "The list append holds {} on the left and {} on the right.",
                    left.describe(),
                    right.describe()
                ),
                "Give both sides a list sharing one element type.".to_string(),
            )),
        },
    }
}

fn arithmetic_mismatch(op: BinaryOp, left: &TypeExpr, right: &TypeExpr, path: &Path) -> Diagnostic {
    type_error(
        path.clone(),
        format!(
            "The {} holds {} on the left and {} on the right.",
            op.label(),
            left.describe(),
            right.describe()
        ),
        "Give both sides whole numbers, or decimals sharing one scale.".to_string(),
    )
}

fn apply_binary(op: BinaryOp, left: Value, right: Value, path: &Path) -> Result<Value, Diagnostic> {
    match op {
        BinaryOp::Add => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => a
                .checked_add(b)
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            (Value::Decimal(a), Value::Decimal(b)) => a
                .add(&b)
                .map(Value::Decimal)
                .map_err(|fault| fault.diagnose(path)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Subtract => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => a
                .checked_sub(b)
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            (Value::Decimal(a), Value::Decimal(b)) => a
                .subtract(&b)
                .map(Value::Decimal)
                .map_err(|fault| fault.diagnose(path)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Multiply => match (left, right) {
            (Value::Integer(a), Value::Integer(b)) => a
                .checked_mul(b)
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            (Value::Decimal(a), Value::Decimal(b)) => a
                .multiply(&b)
                .map(Value::Decimal)
                .map_err(|fault| fault.diagnose(path)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Divide => match (left, right) {
            (Value::Integer(_), Value::Integer(0)) => {
                Err(ArithmeticFault::DivisionByZero.diagnose(path))
            }
            (Value::Integer(a), Value::Integer(b)) => a
                .checked_div(b)
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            (Value::Decimal(a), Value::Decimal(b)) => a
                .divide(&b)
                .map(Value::Decimal)
                .map_err(|fault| fault.diagnose(path)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Remainder => match (left, right) {
            (Value::Integer(_), Value::Integer(0)) => {
                Err(ArithmeticFault::DivisionByZero.diagnose(path))
            }
            (Value::Integer(a), Value::Integer(b)) => a
                .checked_rem(b)
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Equal => Ok(Value::Bool(canonical(left) == canonical(right))),
        BinaryOp::NotEqual => Ok(Value::Bool(canonical(left) != canonical(right))),
        BinaryOp::Less | BinaryOp::LessOrEqual | BinaryOp::Greater | BinaryOp::GreaterOrEqual => {
            let order = compare_values(&left, &right, path)?;
            let held = match op {
                BinaryOp::Less => order == core::cmp::Ordering::Less,
                BinaryOp::LessOrEqual => order != core::cmp::Ordering::Greater,
                BinaryOp::Greater => order == core::cmp::Ordering::Greater,
                _ => order != core::cmp::Ordering::Less,
            };
            Ok(Value::Bool(held))
        }
        BinaryOp::And => match (left, right) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a && b)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::Or => match (left, right) {
            (Value::Bool(a), Value::Bool(b)) => Ok(Value::Bool(a || b)),
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::ConcatText => match (left, right) {
            (Value::Text(a), Value::Text(b)) => {
                let mut joined = String::with_capacity(a.len() + b.len());
                joined.push_str(&a);
                joined.push_str(&b);
                Ok(Value::Text(joined))
            }
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
        BinaryOp::AppendList => match (left, right) {
            (Value::List(mut a), Value::List(b)) => {
                if a.element != b.element {
                    return Err(type_error(
                        path.clone(),
                        "The two lists hold different element types.".to_string(),
                        "Append two lists sharing one element type.".to_string(),
                    ));
                }
                a.items.extend(b.items);
                Ok(Value::List(a))
            }
            (a, b) => Err(runtime_mismatch(op, &a, &b, path)),
        },
    }
}

fn canonical(value: Value) -> Value {
    let mut copy = value;
    copy.canonicalise();
    copy
}

fn compare_values(
    left: &Value,
    right: &Value,
    path: &Path,
) -> Result<core::cmp::Ordering, Diagnostic> {
    match (left, right) {
        (Value::Integer(a), Value::Integer(b)) => Ok(a.cmp(b)),
        (Value::Decimal(a), Value::Decimal(b)) => {
            a.compare(b).map_err(|fault| fault.diagnose(path))
        }
        (Value::Text(a), Value::Text(b)) => Ok(a.cmp(b)),
        (Value::Bytes(a), Value::Bytes(b)) => Ok(a.cmp(b)),
        (Value::Timestamp(a), Value::Timestamp(b)) => Ok(a.cmp(b)),
        (Value::Identity(a), Value::Identity(b)) => Ok(a.cmp(b)),
        (Value::Hash(a), Value::Hash(b)) => Ok(a.cmp(b)),
        (Value::Bool(a), Value::Bool(b)) => Ok(a.cmp(b)),
        (a, b) => Err(type_error(
            path.clone(),
            format!(
                "The ordering holds {} on the left and {} on the right.",
                a.infer_type().describe(),
                b.infer_type().describe()
            ),
            "Order two values sharing one ordered type.".to_string(),
        )),
    }
}

fn runtime_mismatch(op: BinaryOp, left: &Value, right: &Value, path: &Path) -> Diagnostic {
    type_error(
        path.clone(),
        format!(
            "The {} holds {} on the left and {} on the right.",
            op.label(),
            left.infer_type().describe(),
            right.infer_type().describe()
        ),
        "Supply operands the operation accepts.".to_string(),
    )
}

fn check_call(
    ctx: &TypeContext,
    path: &Path,
    function: Builtin,
    args: &[Expr],
) -> Result<TypeExpr, Diagnostic> {
    if args.len() != function.arity() {
        return Err(Diagnostic::at(
            DiagnosticCode::TypeMismatch,
            path.location(),
            format!(
                "The operation `{}` takes {} arguments and the call supplies {}.",
                function.label(),
                function.arity(),
                args.len()
            ),
            format!(
                "Supply {} arguments to `{}`.",
                function.arity(),
                function.label()
            ),
        ));
    }
    let mut found = Vec::with_capacity(args.len());
    for (position, arg) in args.iter().enumerate() {
        found.push(arg.check(ctx, &path.index(position))?);
    }
    let expect = |position: usize, wanted: TypeExpr| -> Result<(), Diagnostic> {
        if found[position] == wanted {
            Ok(())
        } else {
            Err(type_error(
                path.index(position),
                format!(
                    "The argument carries {} and `{}` takes {}.",
                    found[position].describe(),
                    function.label(),
                    wanted.describe()
                ),
                format!("Supply {} here.", wanted.describe()),
            ))
        }
    };
    match function {
        Builtin::Now => Ok(TypeExpr::Timestamp),
        Builtin::Caller => Ok(TypeExpr::Identity),
        Builtin::HashOf => match found[0] {
            TypeExpr::Bytes | TypeExpr::Text => Ok(TypeExpr::Hash),
            _ => Err(type_error(
                path.index(0),
                format!("The hash reads {}.", found[0].describe()),
                "Hash a run of bytes or a run of text.".to_string(),
            )),
        },
        Builtin::TimestampPlus => {
            expect(0, TypeExpr::Timestamp)?;
            expect(1, TypeExpr::Integer)?;
            Ok(TypeExpr::Timestamp)
        }
        Builtin::TimestampSince => {
            expect(0, TypeExpr::Timestamp)?;
            expect(1, TypeExpr::Timestamp)?;
            Ok(TypeExpr::Integer)
        }
        Builtin::AbsoluteInteger => {
            expect(0, TypeExpr::Integer)?;
            Ok(TypeExpr::Integer)
        }
        Builtin::MinimumInteger | Builtin::MaximumInteger => {
            expect(0, TypeExpr::Integer)?;
            expect(1, TypeExpr::Integer)?;
            Ok(TypeExpr::Integer)
        }
        Builtin::BytesOfText => {
            expect(0, TypeExpr::Text)?;
            Ok(TypeExpr::Bytes)
        }
    }
}

fn apply_call(
    function: Builtin,
    args: Vec<Value>,
    env: &Env,
    path: &Path,
) -> Result<Value, Diagnostic> {
    let wrong_shape = || {
        type_error(
            path.clone(),
            format!(
                "The call to `{}` holds arguments of another shape.",
                function.label()
            ),
            format!("Supply the arguments `{}` takes.", function.label()),
        )
    };
    match function {
        Builtin::Now => Ok(Value::Timestamp(env.now())),
        Builtin::Caller => Ok(Value::Identity(env.caller())),
        Builtin::HashOf => match args.first() {
            Some(Value::Bytes(bytes)) => Ok(Value::Hash(Hash32::of(bytes))),
            Some(Value::Text(text)) => Ok(Value::Hash(Hash32::of(text.as_bytes()))),
            _ => Err(wrong_shape()),
        },
        Builtin::TimestampPlus => match (args.first(), args.get(1)) {
            (Some(Value::Timestamp(base)), Some(Value::Integer(span))) => {
                let span =
                    u64::try_from(*span).map_err(|_| ArithmeticFault::Overflow.diagnose(path))?;
                base.plus_millis(span)
                    .map(Value::Timestamp)
                    .map_err(|fault| fault.diagnose(path))
            }
            _ => Err(wrong_shape()),
        },
        Builtin::TimestampSince => match (args.first(), args.get(1)) {
            (Some(Value::Timestamp(later)), Some(Value::Timestamp(earlier))) => later
                .since(earlier)
                .map(|span| Value::Integer(span as i128))
                .map_err(|fault| fault.diagnose(path)),
            _ => Err(wrong_shape()),
        },
        Builtin::AbsoluteInteger => match args.first() {
            Some(Value::Integer(value)) => value
                .checked_abs()
                .map(Value::Integer)
                .ok_or_else(|| ArithmeticFault::Overflow.diagnose(path)),
            _ => Err(wrong_shape()),
        },
        Builtin::MinimumInteger => match (args.first(), args.get(1)) {
            (Some(Value::Integer(a)), Some(Value::Integer(b))) => Ok(Value::Integer(*a.min(b))),
            _ => Err(wrong_shape()),
        },
        Builtin::MaximumInteger => match (args.first(), args.get(1)) {
            (Some(Value::Integer(a)), Some(Value::Integer(b))) => Ok(Value::Integer(*a.max(b))),
            _ => Err(wrong_shape()),
        },
        Builtin::BytesOfText => match args.first() {
            Some(Value::Text(text)) => Ok(Value::Bytes(text.as_bytes().to_vec())),
            _ => Err(wrong_shape()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        EnumerationDef, FieldDef, RecordDef, SumDef, SumVariant, TypeDef, TypeRegistry,
    };
    use alloc::vec;

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
                        name: VariantName::new("rejection"),
                        payload: Some(TypeExpr::Text),
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

    fn env() -> Env {
        Env::new(
            Identity([9u8; 32]),
            Timestamp::from_millis(1_700_000_000_000),
        )
    }

    #[test]
    fn arithmetic_type_checks_and_evaluates() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry);
        let expr = Expr::binary(BinaryOp::Add, Expr::decimal(1250, 2), Expr::decimal(375, 2));
        let ty = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect("the sum type checks");
        assert_eq!(ty, TypeExpr::Decimal { scale: 2 });
        let value = expr.run(&env()).expect("the sum evaluates");
        assert_eq!(value, Value::Decimal(Decimal::new(1625, 2)));
    }

    #[test]
    fn the_checker_catches_a_scale_mismatch() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry);
        let expr = Expr::binary(BinaryOp::Add, Expr::decimal(1250, 2), Expr::decimal(375, 3));
        let diagnostic = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect_err("the mixed scales fail");
        assert_eq!(diagnostic.code, DiagnosticCode::TypeMismatch);
        assert_eq!(diagnostic.location.as_str(), "expr");
        assert!(diagnostic.fix.contains("scale"));
    }

    #[test]
    fn the_checker_catches_an_unknown_name() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry);
        let expr = Expr::binary(BinaryOp::Add, Expr::bound("absent"), Expr::integer(1));
        let diagnostic = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect_err("an absent name fails");
        assert_eq!(diagnostic.code, DiagnosticCode::DanglingReference);
        assert_eq!(diagnostic.location.as_str(), "expr.left");
    }

    #[test]
    fn evaluation_is_deterministic() {
        let expr = Expr::binary(
            BinaryOp::Multiply,
            Expr::binary(BinaryOp::Add, Expr::integer(17), Expr::bound("count")),
            Expr::integer(3),
        );
        let mut first = env();
        first.bind(&ParamName::new("count"), Value::Integer(5));
        first.bind(&ParamName::new("spare"), Value::Text("x".to_string()));
        let mut second = env();
        second.bind(&ParamName::new("spare"), Value::Text("x".to_string()));
        second.bind(&ParamName::new("count"), Value::Integer(5));
        let a = expr.run(&first).expect("the run succeeds");
        let b = expr.run(&second).expect("the run succeeds");
        assert_eq!(a, b);
        assert_eq!(a, Value::Integer(66));
    }

    #[test]
    fn pattern_matching_over_a_sum_type_selects_an_arm() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry);
        let expr = Expr::Match {
            scrutinee: Box::new(Expr::bound("outcome")),
            arms: vec![
                MatchArm {
                    pattern: Pattern::Variant {
                        variant: VariantName::new("approval"),
                        binding: Some(ParamName::new("amount")),
                    },
                    body: Expr::bound("amount"),
                },
                MatchArm {
                    pattern: Pattern::Variant {
                        variant: VariantName::new("rejection"),
                        binding: Some(ParamName::new("reason")),
                    },
                    body: Expr::decimal(0, 2),
                },
                MatchArm {
                    pattern: Pattern::Variant {
                        variant: VariantName::new("withdrawn"),
                        binding: None,
                    },
                    body: Expr::decimal(0, 2),
                },
            ],
            fallback: None,
        };
        let ctx = ctx.with(
            &ParamName::new("outcome"),
            TypeExpr::Sum(TypeId::new("ruling")),
        );
        let ty = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect("the match type checks");
        assert_eq!(ty, TypeExpr::Decimal { scale: 2 });

        let mut bound = env();
        bound.bind(
            &ParamName::new("outcome"),
            Value::sum(
                TypeId::new("ruling"),
                VariantName::new("approval"),
                Some(Value::Decimal(Decimal::new(4200, 2))),
            ),
        );
        assert_eq!(
            expr.run(&bound).expect("the match runs"),
            Value::Decimal(Decimal::new(4200, 2))
        );

        let mut withdrawn = env();
        withdrawn.bind(
            &ParamName::new("outcome"),
            Value::sum(TypeId::new("ruling"), VariantName::new("withdrawn"), None),
        );
        assert_eq!(
            expr.run(&withdrawn).expect("the match runs"),
            Value::Decimal(Decimal::new(0, 2))
        );
    }

    #[test]
    fn a_match_leaving_an_alternative_uncovered_reports_the_fix() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry).with(
            &ParamName::new("outcome"),
            TypeExpr::Sum(TypeId::new("ruling")),
        );
        let expr = Expr::Match {
            scrutinee: Box::new(Expr::bound("outcome")),
            arms: vec![MatchArm {
                pattern: Pattern::Variant {
                    variant: VariantName::new("approval"),
                    binding: Some(ParamName::new("amount")),
                },
                body: Expr::bound("amount"),
            }],
            fallback: None,
        };
        let diagnostic = expr
            .check(&ctx, &Path::root().field("expr"))
            .expect_err("the gap is reported");
        assert_eq!(diagnostic.code, DiagnosticCode::MissingValue);
        assert!(diagnostic.problem.contains("rejection"));
    }

    #[test]
    fn optional_handling_reads_the_element_or_the_fallback() {
        let present = Expr::UnwrapOr {
            optional: Box::new(Expr::Literal(Value::some(
                TypeExpr::Integer,
                Value::Integer(11),
            ))),
            fallback: Box::new(Expr::integer(0)),
        };
        assert_eq!(
            present.run(&env()).expect("the run succeeds"),
            Value::Integer(11)
        );
        let absent = Expr::UnwrapOr {
            optional: Box::new(Expr::Literal(Value::none(TypeExpr::Integer))),
            fallback: Box::new(Expr::integer(0)),
        };
        assert_eq!(
            absent.run(&env()).expect("the run succeeds"),
            Value::Integer(0)
        );
    }

    #[test]
    fn a_record_build_and_a_field_read_agree() {
        let defs = defs();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ctx = TypeContext::new(&registry);
        let build = Expr::MakeRecord {
            type_id: TypeId::new("invoice"),
            fields: vec![
                (FieldName::new("total"), Expr::decimal(9950, 2)),
                (
                    FieldName::new("payer"),
                    Expr::Literal(Value::Identity(Identity([2u8; 32]))),
                ),
            ],
        };
        let read = Expr::field(build, "total");
        let ty = read
            .check(&ctx, &Path::root().field("expr"))
            .expect("the read type checks");
        assert_eq!(ty, TypeExpr::Decimal { scale: 2 });
        assert_eq!(
            read.run(&env()).expect("the read runs"),
            Value::Decimal(Decimal::new(9950, 2))
        );
    }

    #[test]
    fn compensation_covers_a_recoverable_fault() {
        let expr = Expr::Attempt {
            body: Box::new(Expr::binary(
                BinaryOp::Divide,
                Expr::integer(10),
                Expr::integer(0),
            )),
            compensation: Box::new(Expr::integer(-1)),
        };
        assert_eq!(
            expr.run(&env()).expect("the compensation runs"),
            Value::Integer(-1)
        );
    }

    #[test]
    fn a_step_budget_overrun_aborts_the_run() {
        let expr = Expr::binary(
            BinaryOp::Add,
            Expr::binary(BinaryOp::Add, Expr::integer(1), Expr::integer(2)),
            Expr::integer(3),
        );
        let mut meter = Meter::new(3);
        let diagnostic = expr
            .evaluate(&env(), &mut meter, &Path::root().field("expr"))
            .expect_err("the budget stops the run");
        assert_eq!(diagnostic.code, DiagnosticCode::BoundExceeded);
        assert!(diagnostic.fix.contains("step limit"));
    }

    #[test]
    fn a_budget_overrun_stays_outside_compensation() {
        let expr = Expr::Attempt {
            body: Box::new(Expr::binary(
                BinaryOp::Add,
                Expr::integer(1),
                Expr::integer(2),
            )),
            compensation: Box::new(Expr::integer(-1)),
        };
        let mut meter = Meter::new(2);
        let diagnostic = expr
            .evaluate(&env(), &mut meter, &Path::root().field("expr"))
            .expect_err("the budget stops the run");
        assert_eq!(diagnostic.code, DiagnosticCode::BoundExceeded);
    }

    #[test]
    fn substitution_inlines_a_bound_name() {
        let expr = Expr::binary(BinaryOp::Add, Expr::bound("amount"), Expr::integer(1));
        let mut replacements = BTreeMap::new();
        replacements.insert(ParamName::new("amount"), Expr::integer(41));
        let inlined = expr.substitute(&replacements);
        assert_eq!(
            inlined.run(&env()).expect("the run succeeds"),
            Value::Integer(42)
        );
    }

    #[test]
    fn a_local_binding_shadows_a_substitution() {
        let expr = Expr::Let {
            name: ParamName::new("amount"),
            value: Box::new(Expr::integer(7)),
            body: Box::new(Expr::binary(
                BinaryOp::Add,
                Expr::bound("amount"),
                Expr::integer(1),
            )),
        };
        let mut replacements = BTreeMap::new();
        replacements.insert(ParamName::new("amount"), Expr::integer(41));
        let inlined = expr.substitute(&replacements);
        assert_eq!(
            inlined.run(&env()).expect("the run succeeds"),
            Value::Integer(8)
        );
    }

    #[test]
    fn free_names_lists_the_reads_a_caller_supplies() {
        let expr = Expr::Let {
            name: ParamName::new("half"),
            value: Box::new(Expr::binary(
                BinaryOp::Divide,
                Expr::bound("total"),
                Expr::integer(2),
            )),
            body: Box::new(Expr::binary(
                BinaryOp::Add,
                Expr::bound("half"),
                Expr::bound("bonus"),
            )),
        };
        let mut bound = Vec::new();
        let mut found = Vec::new();
        expr.free_names(&mut bound, &mut found);
        assert_eq!(
            found,
            vec![ParamName::new("total"), ParamName::new("bonus")]
        );
    }

    #[test]
    fn the_expression_round_trips_through_scale_and_json() {
        let expr = Expr::conditional(
            Expr::binary(BinaryOp::Greater, Expr::bound("count"), Expr::integer(3)),
            Expr::text("many"),
            Expr::text("few"),
        );
        let bytes = expr.encode();
        let back = Expr::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(expr, back);
        let text = serde_json::to_string(&expr).expect("the expression serialises");
        let parsed: Expr = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(expr, parsed);
    }

    #[test]
    fn a_map_lookup_reports_presence() {
        let map = Expr::Literal(Value::map(
            TypeExpr::Text,
            TypeExpr::Integer,
            vec![(Value::Text("a".to_string()), Value::Integer(1))],
        ));
        let found = Expr::Lookup {
            map: Box::new(map.clone()),
            key: Box::new(Expr::text("a")),
        };
        assert_eq!(
            found.run(&env()).expect("the lookup runs"),
            Value::some(TypeExpr::Integer, Value::Integer(1))
        );
        let absent = Expr::Lookup {
            map: Box::new(map),
            key: Box::new(Expr::text("b")),
        };
        assert_eq!(
            absent.run(&env()).expect("the lookup runs"),
            Value::none(TypeExpr::Integer)
        );
    }

    #[test]
    fn the_builtin_operations_read_the_environment() {
        let now = Expr::Call {
            function: Builtin::Now,
            args: vec![],
        };
        assert_eq!(
            now.run(&env()).expect("the read runs"),
            Value::Timestamp(Timestamp::from_millis(1_700_000_000_000))
        );
        let caller = Expr::Call {
            function: Builtin::Caller,
            args: vec![],
        };
        assert_eq!(
            caller.run(&env()).expect("the read runs"),
            Value::Identity(Identity([9u8; 32]))
        );
    }
}
