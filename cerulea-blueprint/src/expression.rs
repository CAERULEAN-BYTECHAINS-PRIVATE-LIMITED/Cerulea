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
    fold::{CollectionView, DetachedView, FoldExpr},
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
    rc::Rc,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The step budget a single expression receives when a caller names no other
/// limit.
pub const DEFAULT_STEP_LIMIT: u64 = 100_000;

/// Maximum permitted AST nesting depth for an expression.
pub const MAX_EXPRESSION_DEPTH: usize = 256;

/// Maximum permitted total AST nodes inside a single expression.
pub const MAX_EXPRESSION_NODES: usize = 10_000;

/// Maximum permitted elements in an expression list, map, record, match arms, or call arguments.
pub const MAX_COLLECTION_SIZE: usize = 10_000;

/// Default maximum evaluation frame stack depth in `Meter`.
pub const MAX_EVAL_STACK_DEPTH: usize = 512;

/// Maximum permitted nested fold evaluations.
pub const MAX_FOLD_DEPTH: usize = 32;

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

impl Decode for MatchArm {
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        Self::decode_at_depth(input, 0)
    }
}

impl MatchArm {
    pub(crate) fn decode_at_depth<I: codec::Input>(
        input: &mut I,
        depth: usize,
    ) -> Result<Self, codec::Error> {
        Ok(MatchArm {
            pattern: Pattern::decode(input)?,
            body: Expr::decode_at_depth(input, depth)?,
        })
    }
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
    /// Walk every element of a list or a collection range and accumulate
    /// one result. The element count is known before the fold runs.
    Fold(Box<FoldExpr>),
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

impl Decode for Expr {
    fn decode<I: codec::Input>(input: &mut I) -> Result<Self, codec::Error> {
        Self::decode_at_depth(input, 0)
    }
}

impl Drop for Expr {
    fn drop(&mut self) {
        #[inline]
        fn take_boxed(boxed: &mut Box<Expr>, queue: &mut Vec<Expr>) {
            queue.push(core::mem::replace(
                boxed.as_mut(),
                Expr::Literal(Value::Bool(false)),
            ));
        }

        fn take_children(expr: &mut Expr, queue: &mut Vec<Expr>) {
            match expr {
                Expr::Literal(_) | Expr::Bound(_) | Expr::MakeEnumeration { .. } => {}
                Expr::Field { base, .. } => take_boxed(base, queue),
                Expr::Unary { operand, .. } => take_boxed(operand, queue),
                Expr::Binary { left, right, .. } => {
                    take_boxed(left, queue);
                    take_boxed(right, queue);
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    take_boxed(condition, queue);
                    take_boxed(then_branch, queue);
                    take_boxed(else_branch, queue);
                }
                Expr::Match {
                    scrutinee,
                    arms,
                    fallback,
                } => {
                    take_boxed(scrutinee, queue);
                    for arm in core::mem::take(arms) {
                        queue.push(arm.body);
                    }
                    if let Some(fb) = core::mem::take(fallback) {
                        queue.push(*fb);
                    }
                }
                Expr::Let { value, body, .. } => {
                    take_boxed(value, queue);
                    take_boxed(body, queue);
                }
                Expr::MakeRecord { fields, .. } => {
                    for (_, v) in core::mem::take(fields) {
                        queue.push(v);
                    }
                }
                Expr::MakeSum { payload, .. } => {
                    if let Some(p) = core::mem::take(payload) {
                        queue.push(*p);
                    }
                }
                Expr::MakeList { items, .. } => {
                    queue.extend(core::mem::take(items));
                }
                Expr::MakeMap { entries, .. } => {
                    for (k, v) in core::mem::take(entries) {
                        queue.push(k);
                        queue.push(v);
                    }
                }
                Expr::MakeOptional { item, .. } => {
                    if let Some(i) = core::mem::take(item) {
                        queue.push(*i);
                    }
                }
                Expr::MakeReference { key, .. } => take_boxed(key, queue),
                Expr::UnwrapOr {
                    optional, fallback, ..
                } => {
                    take_boxed(optional, queue);
                    take_boxed(fallback, queue);
                }
                Expr::IsPresent(inner) | Expr::Length(inner) => take_boxed(inner, queue),
                Expr::Contains { list, item } => {
                    take_boxed(list, queue);
                    take_boxed(item, queue);
                }
                Expr::Lookup { map, key } => {
                    take_boxed(map, queue);
                    take_boxed(key, queue);
                }
                Expr::Rescale { value, .. } => take_boxed(value, queue),
                Expr::Call { args, .. } => {
                    queue.extend(core::mem::take(args));
                }
                Expr::Fold(fold) => {
                    let mut children = Vec::new();
                    fold.push_exprs_mut_rev(&mut children);
                    for child in children {
                        queue.push(core::mem::replace(
                            child,
                            Expr::Literal(Value::Bool(false)),
                        ));
                    }
                }
                Expr::Attempt { body, compensation } => {
                    take_boxed(body, queue);
                    take_boxed(compensation, queue);
                }
            }
        }

        let mut queue = Vec::new();
        take_children(self, &mut queue);
        while let Some(mut next) = queue.pop() {
            take_children(&mut next, &mut queue);
        }
    }
}

impl Expr {
    /// Decode an expression iteratively while enforcing `MAX_EXPRESSION_DEPTH`
    /// and `MAX_COLLECTION_SIZE` with O(1) native call-stack usage.
    pub(crate) fn decode_at_depth<I: codec::Input>(
        input: &mut I,
        base_depth: usize,
    ) -> Result<Self, codec::Error> {
        enum DecodeFrame {
            Field,
            Unary(UnaryOp),
            BinaryLeft(BinaryOp),
            BinaryRight {
                op: BinaryOp,
                left: Expr,
            },
            IfCond,
            IfThen {
                condition: Expr,
            },
            IfElse {
                condition: Expr,
                then_branch: Expr,
            },
            MatchScrutinee,
            MatchArmBody {
                scrutinee: Expr,
                remaining: usize,
                arms: Vec<MatchArm>,
                pattern: Pattern,
            },
            MatchFallback {
                scrutinee: Expr,
                arms: Vec<MatchArm>,
            },
            LetValue(ParamName),
            LetBody {
                name: ParamName,
                value: Expr,
            },
            MakeRecordField {
                type_id: TypeId,
                remaining: usize,
                fields: Vec<(FieldName, Expr)>,
                field_name: FieldName,
            },
            MakeSumPayload {
                type_id: TypeId,
                variant: VariantName,
            },
            MakeListItem {
                element: TypeExpr,
                remaining: usize,
                items: Vec<Expr>,
            },
            MakeMapKey {
                key: TypeExpr,
                value: TypeExpr,
                remaining: usize,
                entries: Vec<(Expr, Expr)>,
            },
            MakeMapValue {
                key: TypeExpr,
                value: TypeExpr,
                remaining: usize,
                entries: Vec<(Expr, Expr)>,
                entry_key: Expr,
            },
            MakeOptionalItem(TypeExpr),
            MakeReferenceKey(CollectionId),
            UnwrapOrOptional,
            UnwrapOrFallback(Expr),
            IsPresent,
            Length,
            ContainsList,
            ContainsItem(Expr),
            LookupMap,
            LookupKey(Expr),
            Rescale,
            CallArg {
                function: Builtin,
                remaining: usize,
                args: Vec<Expr>,
            },
            AttemptBody,
            AttemptComp(Expr),
        }

        enum DecodeStep {
            Read(usize),
            Ready(Expr),
        }

        #[inline]
        fn read_bounded_count<I: codec::Input>(input: &mut I) -> Result<usize, codec::Error> {
            let count = codec::Compact::<u32>::decode(input)?.0 as usize;
            if count > MAX_COLLECTION_SIZE {
                return Err("Expression collection exceeds maximum size.".into());
            }
            Ok(count)
        }

        let mut stack: Vec<(DecodeFrame, usize)> = Vec::new();
        let mut step = DecodeStep::Read(base_depth);

        loop {
            match step {
                DecodeStep::Read(depth) => {
                    if depth > MAX_EXPRESSION_DEPTH {
                        return Err("Expression exceeds maximum nesting depth.".into());
                    }
                    let next_depth = depth + 1;
                    let variant = input.read_byte()?;
                    step = match variant {
                        0 => DecodeStep::Ready(Expr::Literal(Value::decode(input)?)),
                        1 => DecodeStep::Ready(Expr::Bound(ParamName::decode(input)?)),
                        2 => {
                            stack.push((DecodeFrame::Field, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        3 => {
                            let op = UnaryOp::decode(input)?;
                            stack.push((DecodeFrame::Unary(op), next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        4 => {
                            let op = BinaryOp::decode(input)?;
                            stack.push((DecodeFrame::BinaryLeft(op), next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        5 => {
                            stack.push((DecodeFrame::IfCond, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        6 => {
                            stack.push((DecodeFrame::MatchScrutinee, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        7 => {
                            let name = ParamName::decode(input)?;
                            stack.push((DecodeFrame::LetValue(name), next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        8 => {
                            let type_id = TypeId::decode(input)?;
                            let count = read_bounded_count(input)?;
                            if count == 0 {
                                DecodeStep::Ready(Expr::MakeRecord {
                                    type_id,
                                    fields: Vec::new(),
                                })
                            } else {
                                let field_name = FieldName::decode(input)?;
                                stack.push((
                                    DecodeFrame::MakeRecordField {
                                        type_id,
                                        remaining: count - 1,
                                        fields: Vec::with_capacity(count.min(256)),
                                        field_name,
                                    },
                                    next_depth,
                                ));
                                DecodeStep::Read(next_depth)
                            }
                        }
                        9 => DecodeStep::Ready(Expr::MakeEnumeration {
                            type_id: TypeId::decode(input)?,
                            variant: VariantName::decode(input)?,
                        }),
                        10 => {
                            let type_id = TypeId::decode(input)?;
                            let variant = VariantName::decode(input)?;
                            match Option::<()>::decode(input)? {
                                Some(()) => {
                                    stack.push((
                                        DecodeFrame::MakeSumPayload { type_id, variant },
                                        next_depth,
                                    ));
                                    DecodeStep::Read(next_depth)
                                }
                                None => DecodeStep::Ready(Expr::MakeSum {
                                    type_id,
                                    variant,
                                    payload: None,
                                }),
                            }
                        }
                        11 => {
                            let element = TypeExpr::decode(input)?;
                            let count = read_bounded_count(input)?;
                            if count == 0 {
                                DecodeStep::Ready(Expr::MakeList {
                                    element,
                                    items: Vec::new(),
                                })
                            } else {
                                stack.push((
                                    DecodeFrame::MakeListItem {
                                        element,
                                        remaining: count - 1,
                                        items: Vec::with_capacity(count.min(256)),
                                    },
                                    next_depth,
                                ));
                                DecodeStep::Read(next_depth)
                            }
                        }
                        12 => {
                            let key = TypeExpr::decode(input)?;
                            let value = TypeExpr::decode(input)?;
                            let count = read_bounded_count(input)?;
                            if count == 0 {
                                DecodeStep::Ready(Expr::MakeMap {
                                    key,
                                    value,
                                    entries: Vec::new(),
                                })
                            } else {
                                stack.push((
                                    DecodeFrame::MakeMapKey {
                                        key,
                                        value,
                                        remaining: count - 1,
                                        entries: Vec::with_capacity(count.min(256)),
                                    },
                                    next_depth,
                                ));
                                DecodeStep::Read(next_depth)
                            }
                        }
                        13 => {
                            let element = TypeExpr::decode(input)?;
                            match Option::<()>::decode(input)? {
                                Some(()) => {
                                    stack.push((
                                        DecodeFrame::MakeOptionalItem(element),
                                        next_depth,
                                    ));
                                    DecodeStep::Read(next_depth)
                                }
                                None => DecodeStep::Ready(Expr::MakeOptional {
                                    element,
                                    item: None,
                                }),
                            }
                        }
                        14 => {
                            let collection = CollectionId::decode(input)?;
                            stack.push((DecodeFrame::MakeReferenceKey(collection), next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        15 => {
                            stack.push((DecodeFrame::UnwrapOrOptional, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        16 => {
                            stack.push((DecodeFrame::IsPresent, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        17 => {
                            stack.push((DecodeFrame::Length, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        18 => {
                            stack.push((DecodeFrame::ContainsList, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        19 => {
                            stack.push((DecodeFrame::LookupMap, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        20 => {
                            stack.push((DecodeFrame::Rescale, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        21 => {
                            let function = Builtin::decode(input)?;
                            let count = read_bounded_count(input)?;
                            if count == 0 {
                                DecodeStep::Ready(Expr::Call {
                                    function,
                                    args: Vec::new(),
                                })
                            } else {
                                stack.push((
                                    DecodeFrame::CallArg {
                                        function,
                                        remaining: count - 1,
                                        args: Vec::with_capacity(count.min(256)),
                                    },
                                    next_depth,
                                ));
                                DecodeStep::Read(next_depth)
                            }
                        }
                        22 => DecodeStep::Ready(Expr::Fold(Box::new(FoldExpr::decode_at_depth(
                            input, next_depth,
                        )?))),
                        23 => {
                            stack.push((DecodeFrame::AttemptBody, next_depth));
                            DecodeStep::Read(next_depth)
                        }
                        _ => return Err("Invalid Expr variant.".into()),
                    };
                }
                DecodeStep::Ready(expr) => {
                    let Some((frame, frame_depth)) = stack.pop() else {
                        return Ok(expr);
                    };
                    step = match frame {
                        DecodeFrame::Field => DecodeStep::Ready(Expr::Field {
                            base: Box::new(expr),
                            field: FieldName::decode(input)?,
                        }),
                        DecodeFrame::Unary(op) => DecodeStep::Ready(Expr::Unary {
                            op,
                            operand: Box::new(expr),
                        }),
                        DecodeFrame::BinaryLeft(op) => {
                            stack.push((DecodeFrame::BinaryRight { op, left: expr }, frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::BinaryRight { op, left } => {
                            DecodeStep::Ready(Expr::Binary {
                                op,
                                left: Box::new(left),
                                right: Box::new(expr),
                            })
                        }
                        DecodeFrame::IfCond => {
                            stack.push((DecodeFrame::IfThen { condition: expr }, frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::IfThen { condition } => {
                            stack.push((
                                DecodeFrame::IfElse {
                                    condition,
                                    then_branch: expr,
                                },
                                frame_depth,
                            ));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::IfElse {
                            condition,
                            then_branch,
                        } => DecodeStep::Ready(Expr::If {
                            condition: Box::new(condition),
                            then_branch: Box::new(then_branch),
                            else_branch: Box::new(expr),
                        }),
                        DecodeFrame::MatchScrutinee => {
                            let count = read_bounded_count(input)?;
                            if count == 0 {
                                match Option::<()>::decode(input)? {
                                    Some(()) => {
                                        stack.push((
                                            DecodeFrame::MatchFallback {
                                                scrutinee: expr,
                                                arms: Vec::new(),
                                            },
                                            frame_depth,
                                        ));
                                        DecodeStep::Read(frame_depth)
                                    }
                                    None => DecodeStep::Ready(Expr::Match {
                                        scrutinee: Box::new(expr),
                                        arms: Vec::new(),
                                        fallback: None,
                                    }),
                                }
                            } else {
                                let pattern = Pattern::decode(input)?;
                                stack.push((
                                    DecodeFrame::MatchArmBody {
                                        scrutinee: expr,
                                        remaining: count - 1,
                                        arms: Vec::with_capacity(count.min(256)),
                                        pattern,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            }
                        }
                        DecodeFrame::MatchArmBody {
                            scrutinee,
                            remaining,
                            mut arms,
                            pattern,
                        } => {
                            arms.push(MatchArm {
                                pattern,
                                body: expr,
                            });
                            if remaining > 0 {
                                let next_pattern = Pattern::decode(input)?;
                                stack.push((
                                    DecodeFrame::MatchArmBody {
                                        scrutinee,
                                        remaining: remaining - 1,
                                        arms,
                                        pattern: next_pattern,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            } else {
                                match Option::<()>::decode(input)? {
                                    Some(()) => {
                                        stack.push((
                                            DecodeFrame::MatchFallback { scrutinee, arms },
                                            frame_depth,
                                        ));
                                        DecodeStep::Read(frame_depth)
                                    }
                                    None => DecodeStep::Ready(Expr::Match {
                                        scrutinee: Box::new(scrutinee),
                                        arms,
                                        fallback: None,
                                    }),
                                }
                            }
                        }
                        DecodeFrame::MatchFallback { scrutinee, arms } => {
                            DecodeStep::Ready(Expr::Match {
                                scrutinee: Box::new(scrutinee),
                                arms,
                                fallback: Some(Box::new(expr)),
                            })
                        }
                        DecodeFrame::LetValue(name) => {
                            stack.push((DecodeFrame::LetBody { name, value: expr }, frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::LetBody { name, value } => DecodeStep::Ready(Expr::Let {
                            name,
                            value: Box::new(value),
                            body: Box::new(expr),
                        }),
                        DecodeFrame::MakeRecordField {
                            type_id,
                            remaining,
                            mut fields,
                            field_name,
                        } => {
                            fields.push((field_name, expr));
                            if remaining > 0 {
                                let next_field = FieldName::decode(input)?;
                                stack.push((
                                    DecodeFrame::MakeRecordField {
                                        type_id,
                                        remaining: remaining - 1,
                                        fields,
                                        field_name: next_field,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            } else {
                                DecodeStep::Ready(Expr::MakeRecord { type_id, fields })
                            }
                        }
                        DecodeFrame::MakeSumPayload { type_id, variant } => {
                            DecodeStep::Ready(Expr::MakeSum {
                                type_id,
                                variant,
                                payload: Some(Box::new(expr)),
                            })
                        }
                        DecodeFrame::MakeListItem {
                            element,
                            remaining,
                            mut items,
                        } => {
                            items.push(expr);
                            if remaining > 0 {
                                stack.push((
                                    DecodeFrame::MakeListItem {
                                        element,
                                        remaining: remaining - 1,
                                        items,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            } else {
                                DecodeStep::Ready(Expr::MakeList { element, items })
                            }
                        }
                        DecodeFrame::MakeMapKey {
                            key,
                            value,
                            remaining,
                            entries,
                        } => {
                            stack.push((
                                DecodeFrame::MakeMapValue {
                                    key,
                                    value,
                                    remaining,
                                    entries,
                                    entry_key: expr,
                                },
                                frame_depth,
                            ));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::MakeMapValue {
                            key,
                            value,
                            remaining,
                            mut entries,
                            entry_key,
                        } => {
                            entries.push((entry_key, expr));
                            if remaining > 0 {
                                stack.push((
                                    DecodeFrame::MakeMapKey {
                                        key,
                                        value,
                                        remaining: remaining - 1,
                                        entries,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            } else {
                                DecodeStep::Ready(Expr::MakeMap {
                                    key,
                                    value,
                                    entries,
                                })
                            }
                        }
                        DecodeFrame::MakeOptionalItem(element) => {
                            DecodeStep::Ready(Expr::MakeOptional {
                                element,
                                item: Some(Box::new(expr)),
                            })
                        }
                        DecodeFrame::MakeReferenceKey(collection) => {
                            DecodeStep::Ready(Expr::MakeReference {
                                collection,
                                key: Box::new(expr),
                            })
                        }
                        DecodeFrame::UnwrapOrOptional => {
                            stack.push((DecodeFrame::UnwrapOrFallback(expr), frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::UnwrapOrFallback(optional) => {
                            DecodeStep::Ready(Expr::UnwrapOr {
                                optional: Box::new(optional),
                                fallback: Box::new(expr),
                            })
                        }
                        DecodeFrame::IsPresent => {
                            DecodeStep::Ready(Expr::IsPresent(Box::new(expr)))
                        }
                        DecodeFrame::Length => DecodeStep::Ready(Expr::Length(Box::new(expr))),
                        DecodeFrame::ContainsList => {
                            stack.push((DecodeFrame::ContainsItem(expr), frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::ContainsItem(list) => DecodeStep::Ready(Expr::Contains {
                            list: Box::new(list),
                            item: Box::new(expr),
                        }),
                        DecodeFrame::LookupMap => {
                            stack.push((DecodeFrame::LookupKey(expr), frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::LookupKey(map) => DecodeStep::Ready(Expr::Lookup {
                            map: Box::new(map),
                            key: Box::new(expr),
                        }),
                        DecodeFrame::Rescale => DecodeStep::Ready(Expr::Rescale {
                            value: Box::new(expr),
                            scale: u8::decode(input)?,
                        }),
                        DecodeFrame::CallArg {
                            function,
                            remaining,
                            mut args,
                        } => {
                            args.push(expr);
                            if remaining > 0 {
                                stack.push((
                                    DecodeFrame::CallArg {
                                        function,
                                        remaining: remaining - 1,
                                        args,
                                    },
                                    frame_depth,
                                ));
                                DecodeStep::Read(frame_depth)
                            } else {
                                DecodeStep::Ready(Expr::Call { function, args })
                            }
                        }
                        DecodeFrame::AttemptBody => {
                            stack.push((DecodeFrame::AttemptComp(expr), frame_depth));
                            DecodeStep::Read(frame_depth)
                        }
                        DecodeFrame::AttemptComp(body) => DecodeStep::Ready(Expr::Attempt {
                            body: Box::new(body),
                            compensation: Box::new(expr),
                        }),
                    };
                }
            }
        }
    }

    /// Iteratively verify structural depth, fold nesting depth, total node count, and collection size bounds.
    pub fn validate_structure(&self, path: &Path) -> Result<(), Diagnostic> {
        let mut stack: Vec<(&Expr, usize, usize)> = alloc::vec![(self, 0, 0)];
        let mut nodes: usize = 0;

        while let Some((expr, depth, fold_depth)) = stack.pop() {
            if depth > MAX_EXPRESSION_DEPTH {
                return Err(Diagnostic::at(
                    DiagnosticCode::BoundExceeded,
                    path.location(),
                    format!(
                        "The expression exceeds the maximum nesting depth of {MAX_EXPRESSION_DEPTH}."
                    ),
                    format!("Keep expression nesting at or below {MAX_EXPRESSION_DEPTH} levels."),
                ));
            }
            nodes = nodes.saturating_add(1);
            if nodes > MAX_EXPRESSION_NODES {
                return Err(Diagnostic::at(
                    DiagnosticCode::BoundExceeded,
                    path.location(),
                    format!(
                        "The expression exceeds the maximum node count of {MAX_EXPRESSION_NODES}."
                    ),
                    format!("Keep expressions at or below {MAX_EXPRESSION_NODES} total nodes."),
                ));
            }

            let next_depth = depth + 1;
            match expr {
                Expr::Literal(_) | Expr::Bound(_) | Expr::MakeEnumeration { .. } => {}
                Expr::Field { base, .. } => stack.push((base, next_depth, fold_depth)),
                Expr::Unary { operand, .. } => stack.push((operand, next_depth, fold_depth)),
                Expr::Binary { left, right, .. } => {
                    stack.push((right, next_depth, fold_depth));
                    stack.push((left, next_depth, fold_depth));
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    stack.push((else_branch, next_depth, fold_depth));
                    stack.push((then_branch, next_depth, fold_depth));
                    stack.push((condition, next_depth, fold_depth));
                }
                Expr::Match {
                    scrutinee,
                    arms,
                    fallback,
                } => {
                    if arms.len() > MAX_COLLECTION_SIZE {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The match expression holds {} arms, past the {MAX_COLLECTION_SIZE} limit.",
                                arms.len()
                            ),
                            format!("Keep match arms at or below {MAX_COLLECTION_SIZE}."),
                        ));
                    }
                    if let Some(fb) = fallback {
                        stack.push((fb, next_depth, fold_depth));
                    }
                    for arm in arms.iter().rev() {
                        stack.push((&arm.body, next_depth, fold_depth));
                    }
                    stack.push((scrutinee, next_depth, fold_depth));
                }
                Expr::Let { value, body, .. } => {
                    stack.push((body, next_depth, fold_depth));
                    stack.push((value, next_depth, fold_depth));
                }
                Expr::MakeRecord { fields, .. } => {
                    if fields.len() > MAX_COLLECTION_SIZE {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The record expression holds {} fields, past the {MAX_COLLECTION_SIZE} limit.",
                                fields.len()
                            ),
                            format!("Keep record fields at or below {MAX_COLLECTION_SIZE}."),
                        ));
                    }
                    for (_, v) in fields.iter().rev() {
                        stack.push((v, next_depth, fold_depth));
                    }
                }
                Expr::MakeSum { payload, .. } => {
                    if let Some(p) = payload {
                        stack.push((p, next_depth, fold_depth));
                    }
                }
                Expr::MakeList { items, .. } => {
                    if items.len() > MAX_COLLECTION_SIZE {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The list expression holds {} items, past the {MAX_COLLECTION_SIZE} limit.",
                                items.len()
                            ),
                            format!("Keep list items at or below {MAX_COLLECTION_SIZE}."),
                        ));
                    }
                    for item in items.iter().rev() {
                        stack.push((item, next_depth, fold_depth));
                    }
                }
                Expr::MakeMap { entries, .. } => {
                    if entries.len() > MAX_COLLECTION_SIZE {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The map expression holds {} entries, past the {MAX_COLLECTION_SIZE} limit.",
                                entries.len()
                            ),
                            format!("Keep map entries at or below {MAX_COLLECTION_SIZE}."),
                        ));
                    }
                    for (k, v) in entries.iter().rev() {
                        stack.push((v, next_depth, fold_depth));
                        stack.push((k, next_depth, fold_depth));
                    }
                }
                Expr::MakeOptional { item, .. } => {
                    if let Some(i) = item {
                        stack.push((i, next_depth, fold_depth));
                    }
                }
                Expr::MakeReference { key, .. } => stack.push((key, next_depth, fold_depth)),
                Expr::UnwrapOr { optional, fallback } => {
                    stack.push((fallback, next_depth, fold_depth));
                    stack.push((optional, next_depth, fold_depth));
                }
                Expr::IsPresent(inner) | Expr::Length(inner) => {
                    stack.push((inner, next_depth, fold_depth))
                }
                Expr::Contains { list, item } => {
                    stack.push((item, next_depth, fold_depth));
                    stack.push((list, next_depth, fold_depth));
                }
                Expr::Lookup { map, key } => {
                    stack.push((key, next_depth, fold_depth));
                    stack.push((map, next_depth, fold_depth));
                }
                Expr::Rescale { value, .. } => stack.push((value, next_depth, fold_depth)),
                Expr::Call { args, .. } => {
                    if args.len() > MAX_COLLECTION_SIZE {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The call passes {} arguments, past the {MAX_COLLECTION_SIZE} limit.",
                                args.len()
                            ),
                            format!("Keep call arguments at or below {MAX_COLLECTION_SIZE}."),
                        ));
                    }
                    for arg in args.iter().rev() {
                        stack.push((arg, next_depth, fold_depth));
                    }
                }
                Expr::Fold(fold) => {
                    let next_fold = fold_depth + 1;
                    if next_fold > MAX_FOLD_DEPTH {
                        return Err(Diagnostic::at(
                            DiagnosticCode::BoundExceeded,
                            path.location(),
                            format!(
                                "The expression exceeds the maximum fold nesting depth of {MAX_FOLD_DEPTH}."
                            ),
                            format!("Keep fold nesting at or below {MAX_FOLD_DEPTH} levels."),
                        ));
                    }
                    let mut children = Vec::new();
                    fold.push_exprs_rev(&mut children);
                    for child in children {
                        stack.push((child, next_depth, next_fold));
                    }
                }
                Expr::Attempt { body, compensation } => {
                    stack.push((compensation, next_depth, fold_depth));
                    stack.push((body, next_depth, fold_depth));
                }
            }
        }
        Ok(())
    }

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

    /// Visit this expression and every expression inside it.
    ///
    /// The visit runs on the outer expression first.
    pub fn walk(&self, visit: &mut dyn FnMut(&Expr)) {
        let mut stack: Vec<&Expr> = alloc::vec![self];
        while let Some(expr) = stack.pop() {
            visit(expr);
            match expr {
                Expr::Literal(_) | Expr::Bound(_) | Expr::MakeEnumeration { .. } => {}
                Expr::Field { base, .. } => stack.push(base),
                Expr::Unary { operand, .. } => stack.push(operand),
                Expr::Binary { left, right, .. } => {
                    stack.push(right);
                    stack.push(left);
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    stack.push(else_branch);
                    stack.push(then_branch);
                    stack.push(condition);
                }
                Expr::Match {
                    scrutinee,
                    arms,
                    fallback,
                } => {
                    if let Some(fallback) = fallback {
                        stack.push(fallback);
                    }
                    for arm in arms.iter().rev() {
                        stack.push(&arm.body);
                    }
                    stack.push(scrutinee);
                }
                Expr::Let { value, body, .. } => {
                    stack.push(body);
                    stack.push(value);
                }
                Expr::MakeRecord { fields, .. } => {
                    for (_, value) in fields.iter().rev() {
                        stack.push(value);
                    }
                }
                Expr::MakeSum { payload, .. } => {
                    if let Some(payload) = payload {
                        stack.push(payload);
                    }
                }
                Expr::MakeList { items, .. } => {
                    for item in items.iter().rev() {
                        stack.push(item);
                    }
                }
                Expr::MakeMap { entries, .. } => {
                    for entry in entries.iter().rev() {
                        stack.push(&entry.1);
                        stack.push(&entry.0);
                    }
                }
                Expr::MakeOptional { item, .. } => {
                    if let Some(item) = item {
                        stack.push(item);
                    }
                }
                Expr::MakeReference { key, .. } => stack.push(key),
                Expr::UnwrapOr { optional, fallback } => {
                    stack.push(fallback);
                    stack.push(optional);
                }
                Expr::IsPresent(inner) | Expr::Length(inner) => stack.push(inner),
                Expr::Contains { list, item } => {
                    stack.push(item);
                    stack.push(list);
                }
                Expr::Lookup { map, key } => {
                    stack.push(key);
                    stack.push(map);
                }
                Expr::Rescale { value, .. } => stack.push(value),
                Expr::Call { args, .. } => {
                    for arg in args.iter().rev() {
                        stack.push(arg);
                    }
                }
                Expr::Fold(fold) => fold.push_exprs_rev(&mut stack),
                Expr::Attempt { body, compensation } => {
                    stack.push(compensation);
                    stack.push(body);
                }
            }
        }
    }

    /// Visit this expression and every expression inside it, with the power
    /// to change each one.
    pub fn walk_mut(&mut self, visit: &mut dyn FnMut(&mut Expr)) {
        let mut stack: Vec<&mut Expr> = alloc::vec![self];
        while let Some(expr) = stack.pop() {
            visit(expr);
            match expr {
                Expr::Literal(_) | Expr::Bound(_) | Expr::MakeEnumeration { .. } => {}
                Expr::Field { base, .. } => stack.push(base),
                Expr::Unary { operand, .. } => stack.push(operand),
                Expr::Binary { left, right, .. } => {
                    stack.push(right);
                    stack.push(left);
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    stack.push(else_branch);
                    stack.push(then_branch);
                    stack.push(condition);
                }
                Expr::Match {
                    scrutinee,
                    arms,
                    fallback,
                } => {
                    if let Some(fallback) = fallback {
                        stack.push(fallback);
                    }
                    for arm in arms.iter_mut().rev() {
                        stack.push(&mut arm.body);
                    }
                    stack.push(scrutinee);
                }
                Expr::Let { value, body, .. } => {
                    stack.push(body);
                    stack.push(value);
                }
                Expr::MakeRecord { fields, .. } => {
                    for (_, value) in fields.iter_mut().rev() {
                        stack.push(value);
                    }
                }
                Expr::MakeSum { payload, .. } => {
                    if let Some(payload) = payload {
                        stack.push(payload);
                    }
                }
                Expr::MakeList { items, .. } => {
                    for item in items.iter_mut().rev() {
                        stack.push(item);
                    }
                }
                Expr::MakeMap { entries, .. } => {
                    for entry in entries.iter_mut().rev() {
                        stack.push(&mut entry.1);
                        stack.push(&mut entry.0);
                    }
                }
                Expr::MakeOptional { item, .. } => {
                    if let Some(item) = item {
                        stack.push(item);
                    }
                }
                Expr::MakeReference { key, .. } => stack.push(key),
                Expr::UnwrapOr { optional, fallback } => {
                    stack.push(fallback);
                    stack.push(optional);
                }
                Expr::IsPresent(inner) | Expr::Length(inner) => stack.push(inner),
                Expr::Contains { list, item } => {
                    stack.push(item);
                    stack.push(list);
                }
                Expr::Lookup { map, key } => {
                    stack.push(key);
                    stack.push(map);
                }
                Expr::Rescale { value, .. } => stack.push(value),
                Expr::Call { args, .. } => {
                    for arg in args.iter_mut().rev() {
                        stack.push(arg);
                    }
                }
                Expr::Fold(fold) => fold.push_exprs_mut_rev(&mut stack),
                Expr::Attempt { body, compensation } => {
                    stack.push(compensation);
                    stack.push(body);
                }
            }
        }
    }

    /// Sort every literal inside this expression into canonical order.
    pub fn canonicalise(&mut self) {
        self.walk_mut(&mut |expr| {
            if let Expr::Literal(value) = expr {
                value.canonicalise();
            }
        });
    }

    /// Collect every collection this expression names.
    pub fn collection_refs(&self, out: &mut Vec<CollectionId>) {
        let mut stack: Vec<&Expr> = alloc::vec![self];
        while let Some(expr) = stack.pop() {
            match expr {
                Expr::Literal(value) => {
                    let mut found = Vec::new();
                    value.infer_type().collection_refs(&mut found);
                    out.extend(found);
                }
                Expr::MakeReference { collection, key } => {
                    out.push(collection.clone());
                    stack.push(key);
                }
                Expr::Bound(_) | Expr::MakeEnumeration { .. } => {}
                Expr::Field { base, .. } => stack.push(base),
                Expr::Unary { operand, .. } => stack.push(operand),
                Expr::Binary { left, right, .. } => {
                    stack.push(right);
                    stack.push(left);
                }
                Expr::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    stack.push(else_branch);
                    stack.push(then_branch);
                    stack.push(condition);
                }
                Expr::Match {
                    scrutinee,
                    arms,
                    fallback,
                } => {
                    if let Some(fallback) = fallback {
                        stack.push(fallback);
                    }
                    for arm in arms.iter().rev() {
                        stack.push(&arm.body);
                    }
                    stack.push(scrutinee);
                }
                Expr::Let { value, body, .. } => {
                    stack.push(body);
                    stack.push(value);
                }
                Expr::MakeRecord { fields, .. } => {
                    for (_, value) in fields.iter().rev() {
                        stack.push(value);
                    }
                }
                Expr::MakeSum { payload, .. } => {
                    if let Some(payload) = payload {
                        stack.push(payload);
                    }
                }
                Expr::MakeList { element, items } => {
                    element.collection_refs(out);
                    for item in items.iter().rev() {
                        stack.push(item);
                    }
                }
                Expr::MakeMap {
                    key,
                    value,
                    entries,
                } => {
                    key.collection_refs(out);
                    value.collection_refs(out);
                    for entry in entries.iter().rev() {
                        stack.push(&entry.1);
                        stack.push(&entry.0);
                    }
                }
                Expr::MakeOptional { element, item } => {
                    element.collection_refs(out);
                    if let Some(item) = item {
                        stack.push(item);
                    }
                }
                Expr::UnwrapOr { optional, fallback } => {
                    stack.push(fallback);
                    stack.push(optional);
                }
                Expr::IsPresent(inner) | Expr::Length(inner) => stack.push(inner),
                Expr::Contains { list, item } => {
                    stack.push(item);
                    stack.push(list);
                }
                Expr::Lookup { map, key } => {
                    stack.push(key);
                    stack.push(map);
                }
                Expr::Rescale { value, .. } => stack.push(value),
                Expr::Call { args, .. } => {
                    for arg in args.iter().rev() {
                        stack.push(arg);
                    }
                }
                Expr::Fold(fold) => {
                    if let crate::fold::FoldSource::Collection {
                        collection,
                        element,
                        ..
                    } = &fold.source
                    {
                        out.push(collection.clone());
                        element.collection_refs(out);
                    }
                    fold.push_exprs_rev(&mut stack);
                }
                Expr::Attempt { body, compensation } => {
                    stack.push(compensation);
                    stack.push(body);
                }
            }
        }
    }

    /// Collect every bound name this expression reads without binding first.
    pub fn free_names(&self, bound: &mut Vec<ParamName>, out: &mut Vec<ParamName>) {
        enum FreeTask<'a> {
            Visit(&'a Expr),
            PushBound(&'a ParamName),
            PopBound(usize),
        }

        let mut stack: Vec<FreeTask<'_>> = alloc::vec![FreeTask::Visit(self)];
        while let Some(task) = stack.pop() {
            match task {
                FreeTask::PushBound(name) => bound.push(name.clone()),
                FreeTask::PopBound(count) => {
                    let new_len = bound.len().saturating_sub(count);
                    bound.truncate(new_len);
                }
                FreeTask::Visit(expr) => match expr {
                    Expr::Bound(name) => {
                        if !bound.contains(name) && !out.contains(name) {
                            out.push(name.clone());
                        }
                    }
                    Expr::Let { name, value, body } => {
                        stack.push(FreeTask::PopBound(1));
                        stack.push(FreeTask::Visit(body));
                        stack.push(FreeTask::PushBound(name));
                        stack.push(FreeTask::Visit(value));
                    }
                    Expr::Match {
                        scrutinee,
                        arms,
                        fallback,
                    } => {
                        if let Some(fallback) = fallback {
                            stack.push(FreeTask::Visit(fallback));
                        }
                        for arm in arms.iter().rev() {
                            let binding = match &arm.pattern {
                                Pattern::Variant { binding, .. } => binding.as_ref(),
                                Pattern::Present { binding } => Some(binding),
                                _ => None,
                            };
                            if let Some(name) = binding {
                                stack.push(FreeTask::PopBound(1));
                                stack.push(FreeTask::Visit(&arm.body));
                                stack.push(FreeTask::PushBound(name));
                            } else {
                                stack.push(FreeTask::Visit(&arm.body));
                            }
                        }
                        stack.push(FreeTask::Visit(scrutinee));
                    }
                    Expr::Literal(_) | Expr::MakeEnumeration { .. } => {}
                    Expr::Field { base, .. } => stack.push(FreeTask::Visit(base)),
                    Expr::Unary { operand, .. } => stack.push(FreeTask::Visit(operand)),
                    Expr::Binary { left, right, .. } => {
                        stack.push(FreeTask::Visit(right));
                        stack.push(FreeTask::Visit(left));
                    }
                    Expr::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        stack.push(FreeTask::Visit(else_branch));
                        stack.push(FreeTask::Visit(then_branch));
                        stack.push(FreeTask::Visit(condition));
                    }
                    Expr::MakeRecord { fields, .. } => {
                        for (_, value) in fields.iter().rev() {
                            stack.push(FreeTask::Visit(value));
                        }
                    }
                    Expr::MakeSum { payload, .. } => {
                        if let Some(payload) = payload {
                            stack.push(FreeTask::Visit(payload));
                        }
                    }
                    Expr::MakeList { items, .. } => {
                        for item in items.iter().rev() {
                            stack.push(FreeTask::Visit(item));
                        }
                    }
                    Expr::MakeMap { entries, .. } => {
                        for entry in entries.iter().rev() {
                            stack.push(FreeTask::Visit(&entry.1));
                            stack.push(FreeTask::Visit(&entry.0));
                        }
                    }
                    Expr::MakeOptional { item, .. } => {
                        if let Some(item) = item {
                            stack.push(FreeTask::Visit(item));
                        }
                    }
                    Expr::MakeReference { key, .. } => stack.push(FreeTask::Visit(key)),
                    Expr::UnwrapOr { optional, fallback } => {
                        stack.push(FreeTask::Visit(fallback));
                        stack.push(FreeTask::Visit(optional));
                    }
                    Expr::IsPresent(inner) | Expr::Length(inner) => {
                        stack.push(FreeTask::Visit(inner));
                    }
                    Expr::Contains { list, item } => {
                        stack.push(FreeTask::Visit(item));
                        stack.push(FreeTask::Visit(list));
                    }
                    Expr::Lookup { map, key } => {
                        stack.push(FreeTask::Visit(key));
                        stack.push(FreeTask::Visit(map));
                    }
                    Expr::Rescale { value, .. } => stack.push(FreeTask::Visit(value)),
                    Expr::Call { args, .. } => {
                        for arg in args.iter().rev() {
                            stack.push(FreeTask::Visit(arg));
                        }
                    }
                    Expr::Fold(fold) => {
                        stack.push(FreeTask::PopBound(2));
                        stack.push(FreeTask::Visit(&fold.step));
                        stack.push(FreeTask::PushBound(&fold.element));
                        stack.push(FreeTask::PushBound(&fold.accumulator));
                        stack.push(FreeTask::Visit(&fold.initial));
                        match &fold.source {
                            crate::fold::FoldSource::List(list) => {
                                stack.push(FreeTask::Visit(list));
                            }
                            crate::fold::FoldSource::Collection { keys, .. } => {
                                if let Some(expr) = keys.expression() {
                                    stack.push(FreeTask::Visit(expr));
                                }
                            }
                        }
                    }
                    Expr::Attempt { body, compensation } => {
                        stack.push(FreeTask::Visit(compensation));
                        stack.push(FreeTask::Visit(body));
                    }
                },
            }
        }
    }

    /// Replace every read of a name with a supplied expression.
    ///
    /// Composition uses this pass to inline a block body at a call site.
    pub fn substitute(&self, replacements: &BTreeMap<ParamName, Expr>) -> Expr {
        enum SubFrame<'a> {
            Field {
                field: &'a FieldName,
            },
            Unary {
                op: UnaryOp,
            },
            BinaryLeft {
                op: BinaryOp,
                right: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            BinaryRight {
                op: BinaryOp,
                left: Expr,
            },
            IfCond {
                then_branch: &'a Expr,
                else_branch: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            IfThen {
                cond: Expr,
                else_branch: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            IfElse {
                cond: Expr,
                then_val: Expr,
            },
            MatchScrutinee {
                arms: &'a [MatchArm],
                fallback: &'a Option<Box<Expr>>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MatchArmBody {
                arms: &'a [MatchArm],
                index: usize,
                scrutinee: Expr,
                built_arms: Vec<MatchArm>,
                fallback: &'a Option<Box<Expr>>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MatchFallback {
                scrutinee: Expr,
                built_arms: Vec<MatchArm>,
            },
            LetValue {
                name: &'a ParamName,
                body: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            LetBody {
                name: &'a ParamName,
                value: Expr,
            },
            MakeRecord {
                type_id: &'a TypeId,
                fields: &'a [(FieldName, Expr)],
                index: usize,
                built: Vec<(FieldName, Expr)>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MakeSum {
                type_id: &'a TypeId,
                variant: &'a VariantName,
            },
            MakeList {
                element: &'a TypeExpr,
                items: &'a [Expr],
                index: usize,
                built: Vec<Expr>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MakeMapKey {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                built: Vec<(Expr, Expr)>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MakeMapVal {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                key_expr: Expr,
                built: Vec<(Expr, Expr)>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            MakeOptional {
                element: &'a TypeExpr,
            },
            MakeReference {
                collection: &'a CollectionId,
            },
            UnwrapOrLeft {
                fallback: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            UnwrapOrRight {
                optional: Expr,
            },
            IsPresent,
            Length,
            ContainsLeft {
                item: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            ContainsRight {
                list: Expr,
            },
            LookupLeft {
                key: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            LookupRight {
                map_expr: Expr,
            },
            Rescale {
                scale: u8,
            },
            Call {
                function: Builtin,
                args: &'a [Expr],
                index: usize,
                built: Vec<Expr>,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            AttemptLeft {
                compensation: &'a Expr,
                map: Rc<BTreeMap<ParamName, Expr>>,
            },
            AttemptRight {
                body: Expr,
            },
        }

        enum SubStep<'a> {
            Visit(&'a Expr, Rc<BTreeMap<ParamName, Expr>>),
            Apply(Expr),
        }

        let mut stack: Vec<SubFrame<'_>> = Vec::new();
        let mut step = SubStep::Visit(self, Rc::new(replacements.clone()));

        loop {
            match step {
                SubStep::Visit(expr, map) => match expr {
                    Expr::Bound(name) => {
                        let out = match map.get(name) {
                            Some(replacement) => replacement.clone(),
                            None => Expr::Bound(name.clone()),
                        };
                        step = SubStep::Apply(out);
                    }
                    Expr::Literal(value) => step = SubStep::Apply(Expr::Literal(value.clone())),
                    Expr::MakeEnumeration { type_id, variant } => {
                        step = SubStep::Apply(Expr::MakeEnumeration {
                            type_id: type_id.clone(),
                            variant: variant.clone(),
                        });
                    }
                    Expr::Field { base, field } => {
                        stack.push(SubFrame::Field { field });
                        step = SubStep::Visit(base, map);
                    }
                    Expr::Unary { op, operand } => {
                        stack.push(SubFrame::Unary { op: *op });
                        step = SubStep::Visit(operand, map);
                    }
                    Expr::Binary { op, left, right } => {
                        stack.push(SubFrame::BinaryLeft {
                            op: *op,
                            right,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(left, map);
                    }
                    Expr::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        stack.push(SubFrame::IfCond {
                            then_branch,
                            else_branch,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(condition, map);
                    }
                    Expr::Match {
                        scrutinee,
                        arms,
                        fallback,
                    } => {
                        stack.push(SubFrame::MatchScrutinee {
                            arms,
                            fallback,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(scrutinee, map);
                    }
                    Expr::Let { name, value, body } => {
                        stack.push(SubFrame::LetValue {
                            name,
                            body,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(value, map);
                    }
                    Expr::MakeRecord { type_id, fields } => {
                        if fields.is_empty() {
                            step = SubStep::Apply(Expr::MakeRecord {
                                type_id: type_id.clone(),
                                fields: Vec::new(),
                            });
                        } else {
                            stack.push(SubFrame::MakeRecord {
                                type_id,
                                fields,
                                index: 0,
                                built: Vec::with_capacity(fields.len()),
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(&fields[0].1, map);
                        }
                    }
                    Expr::MakeSum {
                        type_id,
                        variant,
                        payload,
                    } => match payload {
                        Some(inner) => {
                            stack.push(SubFrame::MakeSum { type_id, variant });
                            step = SubStep::Visit(inner, map);
                        }
                        None => {
                            step = SubStep::Apply(Expr::MakeSum {
                                type_id: type_id.clone(),
                                variant: variant.clone(),
                                payload: None,
                            });
                        }
                    },
                    Expr::MakeList { element, items } => {
                        if items.is_empty() {
                            step = SubStep::Apply(Expr::MakeList {
                                element: element.clone(),
                                items: Vec::new(),
                            });
                        } else {
                            stack.push(SubFrame::MakeList {
                                element,
                                items,
                                index: 0,
                                built: Vec::with_capacity(items.len()),
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(&items[0], map);
                        }
                    }
                    Expr::MakeMap {
                        key,
                        value,
                        entries,
                    } => {
                        if entries.is_empty() {
                            step = SubStep::Apply(Expr::MakeMap {
                                key: key.clone(),
                                value: value.clone(),
                                entries: Vec::new(),
                            });
                        } else {
                            stack.push(SubFrame::MakeMapKey {
                                key,
                                value,
                                entries,
                                index: 0,
                                built: Vec::with_capacity(entries.len()),
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(&entries[0].0, map);
                        }
                    }
                    Expr::MakeOptional { element, item } => match item {
                        Some(inner) => {
                            stack.push(SubFrame::MakeOptional { element });
                            step = SubStep::Visit(inner, map);
                        }
                        None => {
                            step = SubStep::Apply(Expr::MakeOptional {
                                element: element.clone(),
                                item: None,
                            });
                        }
                    },
                    Expr::MakeReference { collection, key } => {
                        stack.push(SubFrame::MakeReference { collection });
                        step = SubStep::Visit(key, map);
                    }
                    Expr::UnwrapOr { optional, fallback } => {
                        stack.push(SubFrame::UnwrapOrLeft {
                            fallback,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(optional, map);
                    }
                    Expr::IsPresent(inner) => {
                        stack.push(SubFrame::IsPresent);
                        step = SubStep::Visit(inner, map);
                    }
                    Expr::Length(inner) => {
                        stack.push(SubFrame::Length);
                        step = SubStep::Visit(inner, map);
                    }
                    Expr::Contains { list, item } => {
                        stack.push(SubFrame::ContainsLeft {
                            item,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(list, map);
                    }
                    Expr::Lookup { map: m, key } => {
                        stack.push(SubFrame::LookupLeft {
                            key,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(m, map);
                    }
                    Expr::Rescale { value, scale } => {
                        stack.push(SubFrame::Rescale { scale: *scale });
                        step = SubStep::Visit(value, map);
                    }
                    Expr::Call { function, args } => {
                        if args.is_empty() {
                            step = SubStep::Apply(Expr::Call {
                                function: *function,
                                args: Vec::new(),
                            });
                        } else {
                            stack.push(SubFrame::Call {
                                function: *function,
                                args,
                                index: 0,
                                built: Vec::with_capacity(args.len()),
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(&args[0], map);
                        }
                    }
                    Expr::Fold(fold) => {
                        step = SubStep::Apply(Expr::Fold(Box::new(fold.substitute(&map))));
                    }
                    Expr::Attempt { body, compensation } => {
                        stack.push(SubFrame::AttemptLeft {
                            compensation,
                            map: Rc::clone(&map),
                        });
                        step = SubStep::Visit(body, map);
                    }
                },
                SubStep::Apply(val) => {
                    let Some(frame) = stack.pop() else {
                        return val;
                    };
                    match frame {
                        SubFrame::Field { field } => {
                            step = SubStep::Apply(Expr::Field {
                                base: Box::new(val),
                                field: field.clone(),
                            });
                        }
                        SubFrame::Unary { op } => {
                            step = SubStep::Apply(Expr::Unary {
                                op,
                                operand: Box::new(val),
                            });
                        }
                        SubFrame::BinaryLeft { op, right, map } => {
                            stack.push(SubFrame::BinaryRight { op, left: val });
                            step = SubStep::Visit(right, map);
                        }
                        SubFrame::BinaryRight { op, left } => {
                            step = SubStep::Apply(Expr::Binary {
                                op,
                                left: Box::new(left),
                                right: Box::new(val),
                            });
                        }
                        SubFrame::IfCond {
                            then_branch,
                            else_branch,
                            map,
                        } => {
                            stack.push(SubFrame::IfThen {
                                cond: val,
                                else_branch,
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(then_branch, map);
                        }
                        SubFrame::IfThen {
                            cond,
                            else_branch,
                            map,
                        } => {
                            stack.push(SubFrame::IfElse {
                                cond,
                                then_val: val,
                            });
                            step = SubStep::Visit(else_branch, map);
                        }
                        SubFrame::IfElse { cond, then_val } => {
                            step = SubStep::Apply(Expr::If {
                                condition: Box::new(cond),
                                then_branch: Box::new(then_val),
                                else_branch: Box::new(val),
                            });
                        }
                        SubFrame::MatchScrutinee {
                            arms,
                            fallback,
                            map,
                        } => {
                            if arms.is_empty() {
                                if let Some(fb) = fallback {
                                    stack.push(SubFrame::MatchFallback {
                                        scrutinee: val,
                                        built_arms: Vec::new(),
                                    });
                                    step = SubStep::Visit(fb, map);
                                } else {
                                    step = SubStep::Apply(Expr::Match {
                                        scrutinee: Box::new(val),
                                        arms: Vec::new(),
                                        fallback: None,
                                    });
                                }
                            } else {
                                let mut inner = (*map).clone();
                                match &arms[0].pattern {
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
                                stack.push(SubFrame::MatchArmBody {
                                    arms,
                                    index: 0,
                                    scrutinee: val,
                                    built_arms: Vec::with_capacity(arms.len()),
                                    fallback,
                                    map,
                                });
                                step = SubStep::Visit(&arms[0].body, Rc::new(inner));
                            }
                        }
                        SubFrame::MatchArmBody {
                            arms,
                            index,
                            scrutinee,
                            mut built_arms,
                            fallback,
                            map,
                        } => {
                            built_arms.push(MatchArm {
                                pattern: arms[index].pattern.clone(),
                                body: val,
                            });
                            let next_index = index + 1;
                            if next_index < arms.len() {
                                let mut inner = (*map).clone();
                                match &arms[next_index].pattern {
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
                                stack.push(SubFrame::MatchArmBody {
                                    arms,
                                    index: next_index,
                                    scrutinee,
                                    built_arms,
                                    fallback,
                                    map,
                                });
                                step = SubStep::Visit(&arms[next_index].body, Rc::new(inner));
                            } else if let Some(fb) = fallback {
                                stack.push(SubFrame::MatchFallback {
                                    scrutinee,
                                    built_arms,
                                });
                                step = SubStep::Visit(fb, map);
                            } else {
                                step = SubStep::Apply(Expr::Match {
                                    scrutinee: Box::new(scrutinee),
                                    arms: built_arms,
                                    fallback: None,
                                });
                            }
                        }
                        SubFrame::MatchFallback {
                            scrutinee,
                            built_arms,
                        } => {
                            step = SubStep::Apply(Expr::Match {
                                scrutinee: Box::new(scrutinee),
                                arms: built_arms,
                                fallback: Some(Box::new(val)),
                            });
                        }
                        SubFrame::LetValue { name, body, map } => {
                            let mut inner = (*map).clone();
                            inner.remove(name);
                            stack.push(SubFrame::LetBody { name, value: val });
                            step = SubStep::Visit(body, Rc::new(inner));
                        }
                        SubFrame::LetBody { name, value } => {
                            step = SubStep::Apply(Expr::Let {
                                name: name.clone(),
                                value: Box::new(value),
                                body: Box::new(val),
                            });
                        }
                        SubFrame::MakeRecord {
                            type_id,
                            fields,
                            index,
                            mut built,
                            map,
                        } => {
                            built.push((fields[index].0.clone(), val));
                            let next_index = index + 1;
                            if next_index < fields.len() {
                                stack.push(SubFrame::MakeRecord {
                                    type_id,
                                    fields,
                                    index: next_index,
                                    built,
                                    map: Rc::clone(&map),
                                });
                                step = SubStep::Visit(&fields[next_index].1, map);
                            } else {
                                step = SubStep::Apply(Expr::MakeRecord {
                                    type_id: type_id.clone(),
                                    fields: built,
                                });
                            }
                        }
                        SubFrame::MakeSum { type_id, variant } => {
                            step = SubStep::Apply(Expr::MakeSum {
                                type_id: type_id.clone(),
                                variant: variant.clone(),
                                payload: Some(Box::new(val)),
                            });
                        }
                        SubFrame::MakeList {
                            element,
                            items,
                            index,
                            mut built,
                            map,
                        } => {
                            built.push(val);
                            let next_index = index + 1;
                            if next_index < items.len() {
                                stack.push(SubFrame::MakeList {
                                    element,
                                    items,
                                    index: next_index,
                                    built,
                                    map: Rc::clone(&map),
                                });
                                step = SubStep::Visit(&items[next_index], map);
                            } else {
                                step = SubStep::Apply(Expr::MakeList {
                                    element: element.clone(),
                                    items: built,
                                });
                            }
                        }
                        SubFrame::MakeMapKey {
                            key,
                            value,
                            entries,
                            index,
                            built,
                            map,
                        } => {
                            stack.push(SubFrame::MakeMapVal {
                                key,
                                value,
                                entries,
                                index,
                                key_expr: val,
                                built,
                                map: Rc::clone(&map),
                            });
                            step = SubStep::Visit(&entries[index].1, map);
                        }
                        SubFrame::MakeMapVal {
                            key,
                            value,
                            entries,
                            index,
                            key_expr,
                            mut built,
                            map,
                        } => {
                            built.push((key_expr, val));
                            let next_index = index + 1;
                            if next_index < entries.len() {
                                stack.push(SubFrame::MakeMapKey {
                                    key,
                                    value,
                                    entries,
                                    index: next_index,
                                    built,
                                    map: Rc::clone(&map),
                                });
                                step = SubStep::Visit(&entries[next_index].0, map);
                            } else {
                                step = SubStep::Apply(Expr::MakeMap {
                                    key: key.clone(),
                                    value: value.clone(),
                                    entries: built,
                                });
                            }
                        }
                        SubFrame::MakeOptional { element } => {
                            step = SubStep::Apply(Expr::MakeOptional {
                                element: element.clone(),
                                item: Some(Box::new(val)),
                            });
                        }
                        SubFrame::MakeReference { collection } => {
                            step = SubStep::Apply(Expr::MakeReference {
                                collection: collection.clone(),
                                key: Box::new(val),
                            });
                        }
                        SubFrame::UnwrapOrLeft { fallback, map } => {
                            stack.push(SubFrame::UnwrapOrRight { optional: val });
                            step = SubStep::Visit(fallback, map);
                        }
                        SubFrame::UnwrapOrRight { optional } => {
                            step = SubStep::Apply(Expr::UnwrapOr {
                                optional: Box::new(optional),
                                fallback: Box::new(val),
                            });
                        }
                        SubFrame::IsPresent => {
                            step = SubStep::Apply(Expr::IsPresent(Box::new(val)));
                        }
                        SubFrame::Length => {
                            step = SubStep::Apply(Expr::Length(Box::new(val)));
                        }
                        SubFrame::ContainsLeft { item, map } => {
                            stack.push(SubFrame::ContainsRight { list: val });
                            step = SubStep::Visit(item, map);
                        }
                        SubFrame::ContainsRight { list } => {
                            step = SubStep::Apply(Expr::Contains {
                                list: Box::new(list),
                                item: Box::new(val),
                            });
                        }
                        SubFrame::LookupLeft { key, map } => {
                            stack.push(SubFrame::LookupRight { map_expr: val });
                            step = SubStep::Visit(key, map);
                        }
                        SubFrame::LookupRight { map_expr } => {
                            step = SubStep::Apply(Expr::Lookup {
                                map: Box::new(map_expr),
                                key: Box::new(val),
                            });
                        }
                        SubFrame::Rescale { scale } => {
                            step = SubStep::Apply(Expr::Rescale {
                                value: Box::new(val),
                                scale,
                            });
                        }
                        SubFrame::Call {
                            function,
                            args,
                            index,
                            mut built,
                            map,
                        } => {
                            built.push(val);
                            let next_index = index + 1;
                            if next_index < args.len() {
                                stack.push(SubFrame::Call {
                                    function,
                                    args,
                                    index: next_index,
                                    built,
                                    map: Rc::clone(&map),
                                });
                                step = SubStep::Visit(&args[next_index], map);
                            } else {
                                step = SubStep::Apply(Expr::Call {
                                    function,
                                    args: built,
                                });
                            }
                        }
                        SubFrame::AttemptLeft { compensation, map } => {
                            stack.push(SubFrame::AttemptRight { body: val });
                            step = SubStep::Visit(compensation, map);
                        }
                        SubFrame::AttemptRight { body } => {
                            step = SubStep::Apply(Expr::Attempt {
                                body: Box::new(body),
                                compensation: Box::new(val),
                            });
                        }
                    }
                }
            }
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

/// A view holding zero records. Every detached environment reads this one.
static DETACHED_VIEW: DetachedView = DetachedView;

/// The values in scope while the evaluator walks an expression.
///
/// The environment carries the bound names, the caller, the block time and
/// a view over loaded Blockchain state. A fold over a collection reads the
/// view.
#[derive(Clone, Debug)]
pub struct Env<'a> {
    bindings: BTreeMap<String, Value>,
    caller: Identity,
    now: Timestamp,
    view: &'a dyn CollectionView,
}

impl Default for Env<'static> {
    fn default() -> Self {
        Env::new(Identity::default(), Timestamp::default())
    }
}

impl Env<'static> {
    /// Start an environment over no loaded state.
    pub fn new(caller: Identity, now: Timestamp) -> Env<'static> {
        Env {
            bindings: BTreeMap::new(),
            caller,
            now,
            view: &DETACHED_VIEW,
        }
    }
}

impl<'a> Env<'a> {
    /// Start an environment over a view of loaded state.
    pub fn with_view(caller: Identity, now: Timestamp, view: &'a dyn CollectionView) -> Env<'a> {
        Env {
            bindings: BTreeMap::new(),
            caller,
            now,
            view,
        }
    }

    /// The view over loaded state.
    pub fn view(&self) -> &'a dyn CollectionView {
        self.view
    }

    /// Add one name and its value.
    pub fn bind(&mut self, name: &ParamName, value: Value) {
        self.bindings.insert(name.as_str().to_string(), value);
    }

    /// A copy of this environment carrying one more name.
    pub fn with(&self, name: &ParamName, value: Value) -> Env<'a> {
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
    stack_limit: usize,
    peak_stack: usize,
    fold_depth: usize,
    fold_limit: usize,
}

impl Default for Meter {
    fn default() -> Self {
        Meter::new(DEFAULT_STEP_LIMIT)
    }
}

impl Meter {
    /// Start a meter with a step limit and default stack/fold bounds.
    pub fn new(limit: u64) -> Self {
        Meter {
            used: 0,
            limit,
            stack_limit: MAX_EVAL_STACK_DEPTH,
            peak_stack: 0,
            fold_depth: 0,
            fold_limit: MAX_FOLD_DEPTH,
        }
    }

    /// Start a meter with explicit step and evaluation-stack limits.
    pub fn with_stack_limit(limit: u64, stack_limit: usize) -> Self {
        Meter {
            used: 0,
            limit,
            stack_limit,
            peak_stack: 0,
            fold_depth: 0,
            fold_limit: MAX_FOLD_DEPTH,
        }
    }

    /// The steps spent so far.
    pub fn used(&self) -> u64 {
        self.used
    }

    /// The step limit.
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// The evaluation stack frame limit.
    pub fn stack_limit(&self) -> usize {
        self.stack_limit
    }

    /// The peak evaluation stack frame depth observed.
    pub fn peak_stack(&self) -> usize {
        self.peak_stack
    }

    /// The steps left.
    pub fn remaining(&self) -> u64 {
        self.limit.saturating_sub(self.used)
    }

    /// Verify and record evaluation stack depth against `stack_limit`.
    pub fn check_stack(&mut self, depth: usize, path: &Path) -> Result<(), Diagnostic> {
        if depth > self.peak_stack {
            self.peak_stack = depth;
        }
        if depth > self.stack_limit {
            return Err(Diagnostic::at(
                DiagnosticCode::BoundExceeded,
                path.location(),
                format!(
                    "The expression evaluation stack reached {depth} frames, past the {} limit.",
                    self.stack_limit
                ),
                "Reduce expression nesting or raise the evaluation stack limit.".to_string(),
            ));
        }
        Ok(())
    }

    /// Enter a fold evaluation, enforcing `fold_limit`.
    pub fn enter_fold(&mut self, path: &Path) -> Result<(), Diagnostic> {
        if self.fold_depth >= self.fold_limit {
            return Err(Diagnostic::at(
                DiagnosticCode::BoundExceeded,
                path.location(),
                format!(
                    "Nested fold depth reached {}, past the {} limit.",
                    self.fold_depth + 1,
                    self.fold_limit
                ),
                "Flatten nested fold expressions.".to_string(),
            ));
        }
        self.fold_depth += 1;
        Ok(())
    }

    /// Leave a fold evaluation.
    pub fn exit_fold(&mut self) {
        self.fold_depth = self.fold_depth.saturating_sub(1);
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
        self.validate_structure(path)?;
        self.check_inner(ctx, path)
    }

    fn check_inner(&self, ctx: &TypeContext, path: &Path) -> Result<TypeExpr, Diagnostic> {
        enum CheckFrame<'a, 'b> {
            Field {
                field: &'a FieldName,
                path: Path,
                registry: &'b TypeRegistry<'b>,
            },
            Unary {
                op: UnaryOp,
                path: Path,
            },
            BinaryLeft {
                op: BinaryOp,
                right: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            BinaryRight {
                op: BinaryOp,
                left_type: TypeExpr,
                path: Path,
            },
            IfCond {
                then_branch: &'a Expr,
                else_branch: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            IfThen {
                else_branch: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            IfElse {
                then_type: TypeExpr,
                path: Path,
            },
            MatchScrutinee {
                arms: &'a [MatchArm],
                fallback: &'a Option<Box<Expr>>,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MatchArmBody {
                subject: TypeExpr,
                arms: &'a [MatchArm],
                index: usize,
                result: Option<TypeExpr>,
                covered: Vec<VariantName>,
                saw_present: bool,
                saw_absent: bool,
                fallback: &'a Option<Box<Expr>>,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MatchFallback {
                result: Option<TypeExpr>,
                path: Path,
            },
            LetValue {
                name: &'a ParamName,
                body: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MakeRecordField {
                type_id: &'a TypeId,
                def_fields: &'b [crate::types::FieldDef],
                fields: &'a [(FieldName, Expr)],
                index: usize,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MakeSumPayload {
                type_id: &'a TypeId,
                variant: &'a VariantName,
                expected: TypeExpr,
                path: Path,
            },
            MakeListItem {
                element: &'a TypeExpr,
                items: &'a [Expr],
                index: usize,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MakeMapKey {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MakeMapValue {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            MakeOptionalItem {
                element: &'a TypeExpr,
                path: Path,
            },
            MakeReferenceKey {
                collection: &'a CollectionId,
            },
            UnwrapOrOptional {
                fallback: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            UnwrapOrFallback {
                element: TypeExpr,
                path: Path,
            },
            IsPresent {
                path: Path,
            },
            Length {
                path: Path,
            },
            ContainsList {
                item: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            ContainsItem {
                element: TypeExpr,
                path: Path,
            },
            LookupMap {
                key: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            LookupKey {
                key_type: TypeExpr,
                value_type: TypeExpr,
                path: Path,
            },
            Rescale {
                scale: u8,
                path: Path,
            },
            CallArg {
                function: Builtin,
                args: &'a [Expr],
                index: usize,
                found: Vec<TypeExpr>,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            AttemptBody {
                compensation: &'a Expr,
                path: Path,
                ctx: Rc<TypeContext<'b>>,
            },
            AttemptComp {
                body_type: TypeExpr,
                path: Path,
            },
        }

        enum CheckStep<'a, 'b> {
            Eval {
                expr: &'a Expr,
                ctx: Rc<TypeContext<'b>>,
                path: Path,
            },
            Return(TypeExpr),
        }

        let mut stack: Vec<CheckFrame<'_, '_>> = Vec::new();
        let mut step = CheckStep::Eval {
            expr: self,
            ctx: Rc::new(ctx.clone()),
            path: path.clone(),
        };

        loop {
            match step {
                CheckStep::Eval {
                    expr,
                    ctx: cur_ctx,
                    path: cur_path,
                } => {
                    step = match expr {
                        Expr::Literal(value) => CheckStep::Return(value.infer_type()),
                        Expr::Bound(name) => {
                            let ty = cur_ctx
                                .lookup(name)
                                .cloned()
                                .ok_or_else(|| unknown_name(&cur_path, name, &cur_ctx.names()))?;
                            CheckStep::Return(ty)
                        }
                        Expr::Field { base, field } => {
                            let base_path = cur_path.field("base");
                            stack.push(CheckFrame::Field {
                                field,
                                path: cur_path,
                                registry: cur_ctx.registry,
                            });
                            CheckStep::Eval {
                                expr: base,
                                ctx: cur_ctx,
                                path: base_path,
                            }
                        }
                        Expr::Unary { op, operand } => {
                            let op_path = cur_path.field("operand");
                            stack.push(CheckFrame::Unary {
                                op: *op,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: operand,
                                ctx: cur_ctx,
                                path: op_path,
                            }
                        }
                        Expr::Binary { op, left, right } => {
                            let left_path = cur_path.field("left");
                            stack.push(CheckFrame::BinaryLeft {
                                op: *op,
                                right,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: left,
                                ctx: cur_ctx,
                                path: left_path,
                            }
                        }
                        Expr::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            let cond_path = cur_path.field("condition");
                            stack.push(CheckFrame::IfCond {
                                then_branch,
                                else_branch,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: condition,
                                ctx: cur_ctx,
                                path: cond_path,
                            }
                        }
                        Expr::Match {
                            scrutinee,
                            arms,
                            fallback,
                        } => {
                            let scrutinee_path = cur_path.field("scrutinee");
                            stack.push(CheckFrame::MatchScrutinee {
                                arms,
                                fallback,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: scrutinee,
                                ctx: cur_ctx,
                                path: scrutinee_path,
                            }
                        }
                        Expr::Let { name, value, body } => {
                            let val_path = cur_path.field("value");
                            stack.push(CheckFrame::LetValue {
                                name,
                                body,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: value,
                                ctx: cur_ctx,
                                path: val_path,
                            }
                        }
                        Expr::MakeRecord { type_id, fields } => {
                            let def = cur_ctx.registry.record(type_id).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!("The record `{type_id}` is absent from the Blueprint."),
                                    format!("Declare a record type named `{type_id}`."),
                                )
                            })?;
                            if let Some(first_declared) = def.fields.first() {
                                let supplied = fields
                                    .iter()
                                    .find(|(name, _)| name == &first_declared.name)
                                    .ok_or_else(|| {
                                        Diagnostic::at(
                                            DiagnosticCode::MissingValue,
                                            cur_path.field(first_declared.name.as_str()).location(),
                                            format!(
                                                "The record build leaves out the field `{}`.",
                                                first_declared.name
                                            ),
                                            format!(
                                                "Give the field `{}` a value.",
                                                first_declared.name
                                            ),
                                        )
                                    })?;
                                let field_path = cur_path.field(first_declared.name.as_str());
                                stack.push(CheckFrame::MakeRecordField {
                                    type_id,
                                    def_fields: &def.fields,
                                    fields,
                                    index: 0,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: &supplied.1,
                                    ctx: cur_ctx,
                                    path: field_path,
                                }
                            } else {
                                for (name, _) in fields {
                                    if def.field(name).is_none() {
                                        return Err(Diagnostic::at(
                                            DiagnosticCode::DanglingReference,
                                            cur_path.field(name.as_str()).location(),
                                            format!(
                                                "The record build names `{name}`, which `{type_id}` lacks."
                                            ),
                                            format!("Remove `{name}` or declare it on `{type_id}`."),
                                        ));
                                    }
                                }
                                CheckStep::Return(TypeExpr::Record(type_id.clone()))
                            }
                        }
                        Expr::MakeEnumeration { type_id, variant } => {
                            let def = cur_ctx.registry.enumeration(type_id).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The enumeration `{type_id}` is absent from the Blueprint."
                                    ),
                                    format!("Declare an enumeration named `{type_id}`."),
                                )
                            })?;
                            if def.variants.contains(variant) {
                                CheckStep::Return(TypeExpr::Enumeration(type_id.clone()))
                            } else {
                                return Err(Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The enumeration `{type_id}` lacks the alternative `{variant}`."
                                    ),
                                    format!("Choose an alternative declared on `{type_id}`."),
                                ));
                            }
                        }
                        Expr::MakeSum {
                            type_id,
                            variant,
                            payload,
                        } => {
                            let def = cur_ctx.registry.sum(type_id).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The sum type `{type_id}` is absent from the Blueprint."
                                    ),
                                    format!("Declare a sum type named `{type_id}`."),
                                )
                            })?;
                            let declared = def.variant(variant).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The sum type `{type_id}` lacks the alternative `{variant}`."
                                    ),
                                    format!("Choose an alternative declared on `{type_id}`."),
                                )
                            })?;
                            match (&declared.payload, payload) {
                                (Some(expected), Some(supplied)) => {
                                    let payload_path = cur_path.field("payload");
                                    stack.push(CheckFrame::MakeSumPayload {
                                        type_id,
                                        variant,
                                        expected: expected.clone(),
                                        path: cur_path,
                                    });
                                    CheckStep::Eval {
                                        expr: supplied,
                                        ctx: cur_ctx,
                                        path: payload_path,
                                    }
                                }
                                (None, None) => CheckStep::Return(TypeExpr::Sum(type_id.clone())),
                                (Some(expected), None) => {
                                    return Err(Diagnostic::at(
                                        DiagnosticCode::MissingValue,
                                        cur_path.field("payload").location(),
                                        format!(
                                            "The alternative `{variant}` carries {}.",
                                            expected.describe()
                                        ),
                                        format!("Supply {} for `{variant}`.", expected.describe()),
                                    ));
                                }
                                (None, Some(_)) => {
                                    return Err(type_error(
                                        cur_path.field("payload"),
                                        format!("The alternative `{variant}` carries no data."),
                                        format!("Remove the data supplied for `{variant}`."),
                                    ));
                                }
                            }
                        }
                        Expr::MakeList { element, items } => {
                            element.validate(cur_ctx.registry, &cur_path.field("element"))?;
                            if let Some(first) = items.first() {
                                let item_path = cur_path.index(0);
                                stack.push(CheckFrame::MakeListItem {
                                    element,
                                    items,
                                    index: 0,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: first,
                                    ctx: cur_ctx,
                                    path: item_path,
                                }
                            } else {
                                CheckStep::Return(TypeExpr::list(element.clone()))
                            }
                        }
                        Expr::MakeMap {
                            key,
                            value,
                            entries,
                        } => {
                            let map_type = TypeExpr::map(key.clone(), value.clone());
                            map_type.validate(cur_ctx.registry, &cur_path)?;
                            if let Some(first) = entries.first() {
                                let key_path = cur_path.index(0).field("key");
                                stack.push(CheckFrame::MakeMapKey {
                                    key,
                                    value,
                                    entries,
                                    index: 0,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: &first.0,
                                    ctx: cur_ctx,
                                    path: key_path,
                                }
                            } else {
                                CheckStep::Return(map_type)
                            }
                        }
                        Expr::MakeOptional { element, item } => {
                            element.validate(cur_ctx.registry, &cur_path.field("element"))?;
                            if let Some(item) = item {
                                let item_path = cur_path.field("item");
                                stack.push(CheckFrame::MakeOptionalItem {
                                    element,
                                    path: cur_path,
                                });
                                CheckStep::Eval {
                                    expr: item,
                                    ctx: cur_ctx,
                                    path: item_path,
                                }
                            } else {
                                CheckStep::Return(TypeExpr::optional(element.clone()))
                            }
                        }
                        Expr::MakeReference { collection, key } => {
                            let key_path = cur_path.field("key");
                            stack.push(CheckFrame::MakeReferenceKey { collection });
                            CheckStep::Eval {
                                expr: key,
                                ctx: cur_ctx,
                                path: key_path,
                            }
                        }
                        Expr::UnwrapOr { optional, fallback } => {
                            let opt_path = cur_path.field("optional");
                            stack.push(CheckFrame::UnwrapOrOptional {
                                fallback,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: optional,
                                ctx: cur_ctx,
                                path: opt_path,
                            }
                        }
                        Expr::IsPresent(inner) => {
                            let val_path = cur_path.field("value");
                            stack.push(CheckFrame::IsPresent { path: cur_path });
                            CheckStep::Eval {
                                expr: inner,
                                ctx: cur_ctx,
                                path: val_path,
                            }
                        }
                        Expr::Length(inner) => {
                            let val_path = cur_path.field("value");
                            stack.push(CheckFrame::Length { path: cur_path });
                            CheckStep::Eval {
                                expr: inner,
                                ctx: cur_ctx,
                                path: val_path,
                            }
                        }
                        Expr::Contains { list, item } => {
                            let list_path = cur_path.field("list");
                            stack.push(CheckFrame::ContainsList {
                                item,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: list,
                                ctx: cur_ctx,
                                path: list_path,
                            }
                        }
                        Expr::Lookup { map, key } => {
                            let map_path = cur_path.field("map");
                            stack.push(CheckFrame::LookupMap {
                                key,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: map,
                                ctx: cur_ctx,
                                path: map_path,
                            }
                        }
                        Expr::Rescale { value, scale } => {
                            if *scale > MAX_DECIMAL_SCALE {
                                return Err(Diagnostic::at(
                                    DiagnosticCode::ValueOutOfRange,
                                    cur_path.location(),
                                    format!(
                                        "The requested scale {scale} runs past the maximum of 30."
                                    ),
                                    "Choose a scale of 30 or below.".to_string(),
                                ));
                            }
                            let val_path = cur_path.field("value");
                            stack.push(CheckFrame::Rescale {
                                scale: *scale,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: value,
                                ctx: cur_ctx,
                                path: val_path,
                            }
                        }
                        Expr::Call { function, args } => {
                            if args.len() != function.arity() {
                                return Err(Diagnostic::at(
                                    DiagnosticCode::TypeMismatch,
                                    cur_path.location(),
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
                            if let Some(first) = args.first() {
                                let arg_path = cur_path.index(0);
                                stack.push(CheckFrame::CallArg {
                                    function: *function,
                                    args,
                                    index: 0,
                                    found: Vec::with_capacity(args.len()),
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: first,
                                    ctx: cur_ctx,
                                    path: arg_path,
                                }
                            } else {
                                CheckStep::Return(finish_call_type(&cur_path, *function, &[])?)
                            }
                        }
                        Expr::Fold(fold) => {
                            let ty = fold.check(&cur_ctx, &cur_path.field("fold"))?;
                            CheckStep::Return(ty)
                        }
                        Expr::Attempt { body, compensation } => {
                            let body_path = cur_path.field("body");
                            stack.push(CheckFrame::AttemptBody {
                                compensation,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: body,
                                ctx: cur_ctx,
                                path: body_path,
                            }
                        }
                    };
                }
                CheckStep::Return(val_type) => {
                    let Some(frame) = stack.pop() else {
                        return Ok(val_type);
                    };
                    step = match frame {
                        CheckFrame::Field {
                            field,
                            path: cur_path,
                            registry,
                        } => {
                            let record_id = match &val_type {
                                TypeExpr::Record(id) => id.clone(),
                                other => {
                                    return Err(type_error(
                                        cur_path,
                                        format!("The field read sits on {}.", other.describe()),
                                        "Read a field out of a record value.".to_string(),
                                    ));
                                }
                            };
                            let def = registry.record(&record_id).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The record `{record_id}` is absent from the Blueprint."
                                    ),
                                    format!("Declare a record type named `{record_id}`."),
                                )
                            })?;
                            let found = def.field(field).map(|f| f.ty.clone()).ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    cur_path.location(),
                                    format!(
                                        "The record `{record_id}` lacks the field `{field}`."
                                    ),
                                    format!("Read a field declared on `{record_id}`."),
                                )
                            })?;
                            CheckStep::Return(found)
                        }
                        CheckFrame::Unary { op, path: cur_path } => {
                            let ty = match (op, &val_type) {
                                (UnaryOp::Not, TypeExpr::Bool) => TypeExpr::Bool,
                                (UnaryOp::Negate, TypeExpr::Integer) => TypeExpr::Integer,
                                (UnaryOp::Negate, TypeExpr::Decimal { scale }) => {
                                    TypeExpr::Decimal { scale: *scale }
                                }
                                (UnaryOp::Not, other) => {
                                    return Err(type_error(
                                        cur_path,
                                        format!(
                                            "The reverse operation sits on {}.",
                                            other.describe()
                                        ),
                                        "Apply the reverse operation to a truth value.".to_string(),
                                    ));
                                }
                                (UnaryOp::Negate, other) => {
                                    return Err(type_error(
                                        cur_path,
                                        format!("The sign change sits on {}.", other.describe()),
                                        "Apply the sign change to a whole number or a decimal."
                                            .to_string(),
                                    ));
                                }
                            };
                            CheckStep::Return(ty)
                        }
                        CheckFrame::BinaryLeft {
                            op,
                            right,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let right_path = cur_path.field("right");
                            stack.push(CheckFrame::BinaryRight {
                                op,
                                left_type: val_type,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: right,
                                ctx: cur_ctx,
                                path: right_path,
                            }
                        }
                        CheckFrame::BinaryRight {
                            op,
                            left_type,
                            path: cur_path,
                        } => CheckStep::Return(check_binary(op, &left_type, &val_type, &cur_path)?),
                        CheckFrame::IfCond {
                            then_branch,
                            else_branch,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            if val_type != TypeExpr::Bool {
                                return Err(type_error(
                                    cur_path.field("condition"),
                                    format!("The condition carries {}.", val_type.describe()),
                                    "Give the condition a truth value.".to_string(),
                                ));
                            }
                            let then_path = cur_path.field("then_branch");
                            stack.push(CheckFrame::IfThen {
                                else_branch,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: then_branch,
                                ctx: cur_ctx,
                                path: then_path,
                            }
                        }
                        CheckFrame::IfThen {
                            else_branch,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let else_path = cur_path.field("else_branch");
                            stack.push(CheckFrame::IfElse {
                                then_type: val_type,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: else_branch,
                                ctx: cur_ctx,
                                path: else_path,
                            }
                        }
                        CheckFrame::IfElse {
                            then_type,
                            path: cur_path,
                        } => {
                            if then_type != val_type {
                                return Err(type_error(
                                    cur_path,
                                    format!(
                                        "One branch carries {} and the other carries {}.",
                                        then_type.describe(),
                                        val_type.describe()
                                    ),
                                    "Give both branches one type.".to_string(),
                                ));
                            }
                            CheckStep::Return(then_type)
                        }
                        CheckFrame::MatchScrutinee {
                            arms,
                            fallback,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let subject = val_type;
                            if arms.is_empty() && fallback.is_none() {
                                return Err(Diagnostic::at(
                                    DiagnosticCode::SectionEmpty,
                                    cur_path.location(),
                                    "The match holds zero arms.".to_string(),
                                    "Add one arm for every alternative the value carries."
                                        .to_string(),
                                ));
                            }
                            let mut covered = Vec::new();
                            let mut saw_present = false;
                            let mut saw_absent = false;
                            if let Some(arm) = arms.first() {
                                let here = cur_path.field("arms").index(0);
                                let inner = match_arm_context(
                                    &cur_ctx,
                                    &here,
                                    &subject,
                                    arm,
                                    &mut covered,
                                    &mut saw_present,
                                    &mut saw_absent,
                                )?;
                                stack.push(CheckFrame::MatchArmBody {
                                    subject,
                                    arms,
                                    index: 0,
                                    result: None,
                                    covered,
                                    saw_present,
                                    saw_absent,
                                    fallback,
                                    path: cur_path,
                                    ctx: cur_ctx,
                                });
                                CheckStep::Eval {
                                    expr: &arm.body,
                                    ctx: Rc::new(inner),
                                    path: here.field("body"),
                                }
                            } else if let Some(fb) = fallback {
                                let fb_path = cur_path.field("fallback");
                                stack.push(CheckFrame::MatchFallback {
                                    result: None,
                                    path: cur_path,
                                });
                                CheckStep::Eval {
                                    expr: fb,
                                    ctx: cur_ctx,
                                    path: fb_path,
                                }
                            } else {
                                return Err(Diagnostic::at(
                                    DiagnosticCode::SectionEmpty,
                                    cur_path.location(),
                                    "The match produces no value.".to_string(),
                                    "Add one arm for every alternative the value carries."
                                        .to_string(),
                                ));
                            }
                        }
                        CheckFrame::MatchArmBody {
                            subject,
                            arms,
                            index,
                            mut result,
                            mut covered,
                            mut saw_present,
                            mut saw_absent,
                            fallback,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let here = cur_path.field("arms").index(index);
                            match &result {
                                None => result = Some(val_type),
                                Some(first) => {
                                    if first != &val_type {
                                        return Err(type_error(
                                            here.field("body"),
                                            format!(
                                                "One arm carries {} and another carries {}.",
                                                first.describe(),
                                                val_type.describe()
                                            ),
                                            "Give every arm one type.".to_string(),
                                        ));
                                    }
                                }
                            }
                            let next = index + 1;
                            if let Some(arm) = arms.get(next) {
                                let next_here = cur_path.field("arms").index(next);
                                let inner = match_arm_context(
                                    &cur_ctx,
                                    &next_here,
                                    &subject,
                                    arm,
                                    &mut covered,
                                    &mut saw_present,
                                    &mut saw_absent,
                                )?;
                                stack.push(CheckFrame::MatchArmBody {
                                    subject,
                                    arms,
                                    index: next,
                                    result,
                                    covered,
                                    saw_present,
                                    saw_absent,
                                    fallback,
                                    path: cur_path,
                                    ctx: cur_ctx,
                                });
                                CheckStep::Eval {
                                    expr: &arm.body,
                                    ctx: Rc::new(inner),
                                    path: next_here.field("body"),
                                }
                            } else if let Some(fb) = fallback {
                                let fb_path = cur_path.field("fallback");
                                stack.push(CheckFrame::MatchFallback {
                                    result,
                                    path: cur_path,
                                });
                                CheckStep::Eval {
                                    expr: fb,
                                    ctx: cur_ctx,
                                    path: fb_path,
                                }
                            } else {
                                let missing = missing_alternatives(
                                    &cur_ctx,
                                    &subject,
                                    &covered,
                                    saw_present,
                                    saw_absent,
                                );
                                if let Some(word) = missing {
                                    return Err(Diagnostic::at(
                                        DiagnosticCode::MissingValue,
                                        cur_path.location(),
                                        format!(
                                            "The match leaves the alternative `{word}` uncovered."
                                        ),
                                        format!("Add an arm for `{word}`, or add a fallback."),
                                    ));
                                }
                                let ty = result.ok_or_else(|| {
                                    Diagnostic::at(
                                        DiagnosticCode::SectionEmpty,
                                        cur_path.location(),
                                        "The match produces no value.".to_string(),
                                        "Add one arm for every alternative the value carries."
                                            .to_string(),
                                    )
                                })?;
                                CheckStep::Return(ty)
                            }
                        }
                        CheckFrame::MatchFallback {
                            mut result,
                            path: cur_path,
                        } => {
                            match &result {
                                None => result = Some(val_type),
                                Some(first) => {
                                    if first != &val_type {
                                        return Err(type_error(
                                            cur_path.field("fallback"),
                                            format!(
                                                "The arms carry {} and the fallback carries {}.",
                                                first.describe(),
                                                val_type.describe()
                                            ),
                                            "Give the fallback the type the arms carry."
                                                .to_string(),
                                        ));
                                    }
                                }
                            }
                            let ty = result.ok_or_else(|| {
                                Diagnostic::at(
                                    DiagnosticCode::SectionEmpty,
                                    cur_path.location(),
                                    "The match produces no value.".to_string(),
                                    "Add one arm for every alternative the value carries."
                                        .to_string(),
                                )
                            })?;
                            CheckStep::Return(ty)
                        }
                        CheckFrame::LetValue {
                            name,
                            body,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let inner = Rc::new(cur_ctx.with(name, val_type));
                            CheckStep::Eval {
                                expr: body,
                                ctx: inner,
                                path: cur_path.field("body"),
                            }
                        }
                        CheckFrame::MakeRecordField {
                            type_id,
                            def_fields,
                            fields,
                            index,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let declared = &def_fields[index];
                            if val_type != declared.ty {
                                return Err(type_error(
                                    cur_path.field(declared.name.as_str()),
                                    format!(
                                        "The field `{}` carries {} and the record declares {}.",
                                        declared.name,
                                        val_type.describe(),
                                        declared.ty.describe()
                                    ),
                                    format!(
                                        "Supply {} for `{}`.",
                                        declared.ty.describe(),
                                        declared.name
                                    ),
                                ));
                            }
                            let next = index + 1;
                            if let Some(next_declared) = def_fields.get(next) {
                                let supplied = fields
                                    .iter()
                                    .find(|(name, _)| name == &next_declared.name)
                                    .ok_or_else(|| {
                                        Diagnostic::at(
                                            DiagnosticCode::MissingValue,
                                            cur_path
                                                .field(next_declared.name.as_str())
                                                .location(),
                                            format!(
                                                "The record build leaves out the field `{}`.",
                                                next_declared.name
                                            ),
                                            format!(
                                                "Give the field `{}` a value.",
                                                next_declared.name
                                            ),
                                        )
                                    })?;
                                let field_path = cur_path.field(next_declared.name.as_str());
                                stack.push(CheckFrame::MakeRecordField {
                                    type_id,
                                    def_fields,
                                    fields,
                                    index: next,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: &supplied.1,
                                    ctx: cur_ctx,
                                    path: field_path,
                                }
                            } else {
                                for (name, _) in fields {
                                    if !def_fields.iter().any(|f| &f.name == name) {
                                        return Err(Diagnostic::at(
                                            DiagnosticCode::DanglingReference,
                                            cur_path.field(name.as_str()).location(),
                                            format!(
                                                "The record build names `{name}`, which `{type_id}` lacks."
                                            ),
                                            format!("Remove `{name}` or declare it on `{type_id}`."),
                                        ));
                                    }
                                }
                                CheckStep::Return(TypeExpr::Record(type_id.clone()))
                            }
                        }
                        CheckFrame::MakeSumPayload {
                            type_id,
                            variant,
                            expected,
                            path: cur_path,
                        } => {
                            if val_type != expected {
                                return Err(type_error(
                                    cur_path.field("payload"),
                                    format!(
                                        "The alternative `{variant}` carries {} and the build supplies {}.",
                                        expected.describe(),
                                        val_type.describe()
                                    ),
                                    format!("Supply {} here.", expected.describe()),
                                ));
                            }
                            CheckStep::Return(TypeExpr::Sum(type_id.clone()))
                        }
                        CheckFrame::MakeListItem {
                            element,
                            items,
                            index,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            if &val_type != element {
                                return Err(type_error(
                                    cur_path.index(index),
                                    format!(
                                        "The element carries {} and the list declares {}.",
                                        val_type.describe(),
                                        element.describe()
                                    ),
                                    format!("Supply {} for every element.", element.describe()),
                                ));
                            }
                            let next = index + 1;
                            if let Some(next_item) = items.get(next) {
                                let item_path = cur_path.index(next);
                                stack.push(CheckFrame::MakeListItem {
                                    element,
                                    items,
                                    index: next,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: next_item,
                                    ctx: cur_ctx,
                                    path: item_path,
                                }
                            } else {
                                CheckStep::Return(TypeExpr::list(element.clone()))
                            }
                        }
                        CheckFrame::MakeMapKey {
                            key,
                            value,
                            entries,
                            index,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let here = cur_path.index(index);
                            if &val_type != key {
                                return Err(type_error(
                                    here.field("key"),
                                    format!(
                                        "The key carries {} and the map declares {}.",
                                        val_type.describe(),
                                        key.describe()
                                    ),
                                    format!("Supply {} for every key.", key.describe()),
                                ));
                            }
                            let val_path = here.field("value");
                            stack.push(CheckFrame::MakeMapValue {
                                key,
                                value,
                                entries,
                                index,
                                path: cur_path,
                                ctx: Rc::clone(&cur_ctx),
                            });
                            CheckStep::Eval {
                                expr: &entries[index].1,
                                ctx: cur_ctx,
                                path: val_path,
                            }
                        }
                        CheckFrame::MakeMapValue {
                            key,
                            value,
                            entries,
                            index,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let here = cur_path.index(index);
                            if &val_type != value {
                                return Err(type_error(
                                    here.field("value"),
                                    format!(
                                        "The value carries {} and the map declares {}.",
                                        val_type.describe(),
                                        value.describe()
                                    ),
                                    format!("Supply {} for every value.", value.describe()),
                                ));
                            }
                            let next = index + 1;
                            if let Some(next_entry) = entries.get(next) {
                                let key_path = cur_path.index(next).field("key");
                                stack.push(CheckFrame::MakeMapKey {
                                    key,
                                    value,
                                    entries,
                                    index: next,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: &next_entry.0,
                                    ctx: cur_ctx,
                                    path: key_path,
                                }
                            } else {
                                CheckStep::Return(TypeExpr::map(key.clone(), value.clone()))
                            }
                        }
                        CheckFrame::MakeOptionalItem {
                            element,
                            path: cur_path,
                        } => {
                            if &val_type != element {
                                return Err(type_error(
                                    cur_path.field("item"),
                                    format!(
                                        "The element carries {} and the optional declares {}.",
                                        val_type.describe(),
                                        element.describe()
                                    ),
                                    format!("Supply {} here.", element.describe()),
                                ));
                            }
                            CheckStep::Return(TypeExpr::optional(element.clone()))
                        }
                        CheckFrame::MakeReferenceKey { collection } => {
                            CheckStep::Return(TypeExpr::Reference(collection.clone()))
                        }
                        CheckFrame::UnwrapOrOptional {
                            fallback,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let element = match val_type {
                                TypeExpr::Optional(inner) => *inner,
                                other => {
                                    return Err(type_error(
                                        cur_path.field("optional"),
                                        format!("The unwrap sits on {}.", other.describe()),
                                        "Apply the unwrap to an optional value.".to_string(),
                                    ));
                                }
                            };
                            let fb_path = cur_path.field("fallback");
                            stack.push(CheckFrame::UnwrapOrFallback {
                                element,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: fallback,
                                ctx: cur_ctx,
                                path: fb_path,
                            }
                        }
                        CheckFrame::UnwrapOrFallback {
                            element,
                            path: cur_path,
                        } => {
                            if val_type != element {
                                return Err(type_error(
                                    cur_path.field("fallback"),
                                    format!(
                                        "The fallback carries {} and the element is {}.",
                                        val_type.describe(),
                                        element.describe()
                                    ),
                                    format!("Supply {} as the fallback.", element.describe()),
                                ));
                            }
                            CheckStep::Return(element)
                        }
                        CheckFrame::IsPresent { path: cur_path } => match val_type {
                            TypeExpr::Optional(_) => CheckStep::Return(TypeExpr::Bool),
                            other => {
                                return Err(type_error(
                                    cur_path,
                                    format!("The presence test sits on {}.", other.describe()),
                                    "Apply the presence test to an optional value.".to_string(),
                                ));
                            }
                        },
                        CheckFrame::Length { path: cur_path } => match val_type {
                            TypeExpr::List(_)
                            | TypeExpr::Map { .. }
                            | TypeExpr::Text
                            | TypeExpr::Bytes => CheckStep::Return(TypeExpr::Integer),
                            other => {
                                return Err(type_error(
                                    cur_path,
                                    format!("The length reads {}.", other.describe()),
                                    "Read the length of a list, a map, text or bytes.".to_string(),
                                ));
                            }
                        },
                        CheckFrame::ContainsList {
                            item,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let element = match val_type {
                                TypeExpr::List(inner) => *inner,
                                other => {
                                    return Err(type_error(
                                        cur_path.field("list"),
                                        format!(
                                            "The membership test sits on {}.",
                                            other.describe()
                                        ),
                                        "Apply the membership test to a list.".to_string(),
                                    ));
                                }
                            };
                            let item_path = cur_path.field("item");
                            stack.push(CheckFrame::ContainsItem {
                                element,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: item,
                                ctx: cur_ctx,
                                path: item_path,
                            }
                        }
                        CheckFrame::ContainsItem {
                            element,
                            path: cur_path,
                        } => {
                            if val_type != element {
                                return Err(type_error(
                                    cur_path.field("item"),
                                    format!(
                                        "The sought value carries {} and the list holds {}.",
                                        val_type.describe(),
                                        element.describe()
                                    ),
                                    format!("Supply {} here.", element.describe()),
                                ));
                            }
                            CheckStep::Return(TypeExpr::Bool)
                        }
                        CheckFrame::LookupMap {
                            key,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let (key_type, value_type) = match val_type {
                                TypeExpr::Map { key, value } => (*key, *value),
                                other => {
                                    return Err(type_error(
                                        cur_path.field("map"),
                                        format!("The lookup sits on {}.", other.describe()),
                                        "Apply the lookup to a map.".to_string(),
                                    ));
                                }
                            };
                            let key_path = cur_path.field("key");
                            stack.push(CheckFrame::LookupKey {
                                key_type,
                                value_type,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: key,
                                ctx: cur_ctx,
                                path: key_path,
                            }
                        }
                        CheckFrame::LookupKey {
                            key_type,
                            value_type,
                            path: cur_path,
                        } => {
                            if val_type != key_type {
                                return Err(type_error(
                                    cur_path.field("key"),
                                    format!(
                                        "The key carries {} and the map keys are {}.",
                                        val_type.describe(),
                                        key_type.describe()
                                    ),
                                    format!("Supply {} as the key.", key_type.describe()),
                                ));
                            }
                            CheckStep::Return(TypeExpr::optional(value_type))
                        }
                        CheckFrame::Rescale {
                            scale,
                            path: cur_path,
                        } => match val_type {
                            TypeExpr::Decimal { .. } => {
                                CheckStep::Return(TypeExpr::Decimal { scale })
                            }
                            other => {
                                return Err(type_error(
                                    cur_path,
                                    format!("The rescale sits on {}.", other.describe()),
                                    "Apply the rescale to a decimal value.".to_string(),
                                ));
                            }
                        },
                        CheckFrame::CallArg {
                            function,
                            args,
                            index,
                            mut found,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            found.push(val_type);
                            let next = index + 1;
                            if let Some(next_arg) = args.get(next) {
                                let arg_path = cur_path.index(next);
                                stack.push(CheckFrame::CallArg {
                                    function,
                                    args,
                                    index: next,
                                    found,
                                    path: cur_path,
                                    ctx: Rc::clone(&cur_ctx),
                                });
                                CheckStep::Eval {
                                    expr: next_arg,
                                    ctx: cur_ctx,
                                    path: arg_path,
                                }
                            } else {
                                CheckStep::Return(finish_call_type(&cur_path, function, &found)?)
                            }
                        }
                        CheckFrame::AttemptBody {
                            compensation,
                            path: cur_path,
                            ctx: cur_ctx,
                        } => {
                            let comp_path = cur_path.field("compensation");
                            stack.push(CheckFrame::AttemptComp {
                                body_type: val_type,
                                path: cur_path,
                            });
                            CheckStep::Eval {
                                expr: compensation,
                                ctx: cur_ctx,
                                path: comp_path,
                            }
                        }
                        CheckFrame::AttemptComp {
                            body_type,
                            path: cur_path,
                        } => {
                            if body_type != val_type {
                                return Err(type_error(
                                    cur_path,
                                    format!(
                                        "The body carries {} and the compensation carries {}.",
                                        body_type.describe(),
                                        val_type.describe()
                                    ),
                                    "Give the body and the compensation one type.".to_string(),
                                ));
                            }
                            CheckStep::Return(body_type)
                        }
                    };
                }
            }
        }
    }

    /// Run the expression against bound values using an iterative stack machine
    /// so deeply nested expressions cannot overflow the native call stack.
    pub fn evaluate(&self, env: &Env, meter: &mut Meter, path: &Path) -> Result<Value, Diagnostic> {
        enum EvalFrame<'a, 'b> {
            Field {
                field: &'a FieldName,
                path: Path,
            },
            Unary {
                op: UnaryOp,
                path: Path,
            },
            BinaryLeft {
                op: BinaryOp,
                right: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            BinaryRight {
                op: BinaryOp,
                left_value: Value,
                path: Path,
            },
            IfCond {
                then_branch: &'a Expr,
                else_branch: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MatchScrutinee {
                arms: &'a [MatchArm],
                fallback: &'a Option<Box<Expr>>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            LetValue {
                name: &'a ParamName,
                body: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MakeRecord {
                type_id: &'a TypeId,
                fields: &'a [(FieldName, Expr)],
                index: usize,
                built: Vec<(FieldName, Value)>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MakeSum {
                type_id: &'a TypeId,
                variant: &'a VariantName,
            },
            MakeList {
                element: &'a TypeExpr,
                items: &'a [Expr],
                index: usize,
                built: Vec<Value>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MakeMapKey {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                built: Vec<(Value, Value)>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MakeMapValue {
                key: &'a TypeExpr,
                value: &'a TypeExpr,
                entries: &'a [(Expr, Expr)],
                index: usize,
                key_value: Value,
                built: Vec<(Value, Value)>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            MakeOptional {
                element: &'a TypeExpr,
            },
            MakeReference {
                collection: &'a CollectionId,
            },
            UnwrapOr {
                fallback: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            IsPresent {
                path: Path,
            },
            Length {
                path: Path,
            },
            ContainsList {
                item: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            ContainsItem {
                list_value: Value,
                path: Path,
            },
            LookupMap {
                key: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            LookupKey {
                map_value: Value,
                path: Path,
            },
            Rescale {
                scale: u8,
                path: Path,
            },
            Call {
                function: Builtin,
                args: &'a [Expr],
                index: usize,
                built: Vec<Value>,
                env: Rc<Env<'b>>,
                path: Path,
            },
            Attempt {
                compensation: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
        }

        enum Step<'a, 'b> {
            Eval {
                expr: &'a Expr,
                env: Rc<Env<'b>>,
                path: Path,
            },
            Apply(Result<Value, Diagnostic>),
        }

        let mut stack: Vec<EvalFrame<'_, '_>> = Vec::new();
        let mut step = Step::Eval {
            expr: self,
            env: Rc::new(env.clone()),
            path: path.clone(),
        };

        loop {
            match step {
                Step::Eval {
                    expr,
                    env: cur_env,
                    path: cur_path,
                } => {
                    if let Err(fault) = meter.charge(1, &cur_path) {
                        step = Step::Apply(Err(fault));
                        continue;
                    }
                    if let Err(fault) = meter.check_stack(stack.len(), &cur_path) {
                        step = Step::Apply(Err(fault));
                        continue;
                    }
                    match expr {
                        Expr::Literal(value) => {
                            step = Step::Apply(Ok(value.clone()));
                        }
                        Expr::Bound(name) => {
                            let res = cur_env
                                .lookup(name)
                                .cloned()
                                .ok_or_else(|| unknown_name(&cur_path, name, &cur_env.names()));
                            step = Step::Apply(res);
                        }
                        Expr::Field { base, field } => {
                            let next_path = cur_path.field("base");
                            stack.push(EvalFrame::Field {
                                field,
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: base,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Unary { op, operand } => {
                            let next_path = cur_path.field("operand");
                            stack.push(EvalFrame::Unary {
                                op: *op,
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: operand,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Binary { op, left, right } => {
                            let next_path = cur_path.field("left");
                            stack.push(EvalFrame::BinaryLeft {
                                op: *op,
                                right,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: left,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::If {
                            condition,
                            then_branch,
                            else_branch,
                        } => {
                            let next_path = cur_path.field("condition");
                            stack.push(EvalFrame::IfCond {
                                then_branch,
                                else_branch,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: condition,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Match {
                            scrutinee,
                            arms,
                            fallback,
                        } => {
                            let next_path = cur_path.field("scrutinee");
                            stack.push(EvalFrame::MatchScrutinee {
                                arms,
                                fallback,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: scrutinee,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Let { name, value, body } => {
                            let next_path = cur_path.field("value");
                            stack.push(EvalFrame::LetValue {
                                name,
                                body,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: value,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::MakeRecord { type_id, fields } => {
                            if fields.is_empty() {
                                step = Step::Apply(Ok(Value::record(type_id.clone(), Vec::new())));
                            } else {
                                let next_path = cur_path.field(fields[0].0.as_str());
                                stack.push(EvalFrame::MakeRecord {
                                    type_id,
                                    fields,
                                    index: 0,
                                    built: Vec::with_capacity(fields.len()),
                                    env: Rc::clone(&cur_env),
                                    path: cur_path,
                                });
                                step = Step::Eval {
                                    expr: &fields[0].1,
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                        }
                        Expr::MakeEnumeration { type_id, variant } => {
                            step = Step::Apply(Ok(Value::enumeration(
                                type_id.clone(),
                                variant.clone(),
                            )));
                        }
                        Expr::MakeSum {
                            type_id,
                            variant,
                            payload,
                        } => match payload {
                            Some(inner) => {
                                let next_path = cur_path.field("payload");
                                stack.push(EvalFrame::MakeSum { type_id, variant });
                                step = Step::Eval {
                                    expr: inner,
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                            None => {
                                step = Step::Apply(Ok(Value::sum(
                                    type_id.clone(),
                                    variant.clone(),
                                    None,
                                )));
                            }
                        },
                        Expr::MakeList { element, items } => {
                            if items.is_empty() {
                                step = Step::Apply(Ok(Value::list(element.clone(), Vec::new())));
                            } else {
                                let next_path = cur_path.index(0);
                                stack.push(EvalFrame::MakeList {
                                    element,
                                    items,
                                    index: 0,
                                    built: Vec::with_capacity(items.len()),
                                    env: Rc::clone(&cur_env),
                                    path: cur_path,
                                });
                                step = Step::Eval {
                                    expr: &items[0],
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                        }
                        Expr::MakeMap {
                            key,
                            value,
                            entries,
                        } => {
                            if entries.is_empty() {
                                step = Step::Apply(Ok(Value::map(
                                    key.clone(),
                                    value.clone(),
                                    Vec::new(),
                                )));
                            } else {
                                let next_path = cur_path.index(0).field("key");
                                stack.push(EvalFrame::MakeMapKey {
                                    key,
                                    value,
                                    entries,
                                    index: 0,
                                    built: Vec::with_capacity(entries.len()),
                                    env: Rc::clone(&cur_env),
                                    path: cur_path,
                                });
                                step = Step::Eval {
                                    expr: &entries[0].0,
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                        }
                        Expr::MakeOptional { element, item } => match item {
                            Some(inner) => {
                                let next_path = cur_path.field("item");
                                stack.push(EvalFrame::MakeOptional { element });
                                step = Step::Eval {
                                    expr: inner,
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                            None => {
                                step = Step::Apply(Ok(Value::none(element.clone())));
                            }
                        },
                        Expr::MakeReference { collection, key } => {
                            let next_path = cur_path.field("key");
                            stack.push(EvalFrame::MakeReference { collection });
                            step = Step::Eval {
                                expr: key,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::UnwrapOr { optional, fallback } => {
                            let next_path = cur_path.field("optional");
                            stack.push(EvalFrame::UnwrapOr {
                                fallback,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: optional,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::IsPresent(inner) => {
                            let next_path = cur_path.field("value");
                            stack.push(EvalFrame::IsPresent { path: cur_path });
                            step = Step::Eval {
                                expr: inner,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Length(inner) => {
                            let next_path = cur_path.field("value");
                            stack.push(EvalFrame::Length { path: cur_path });
                            step = Step::Eval {
                                expr: inner,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Contains { list, item } => {
                            let next_path = cur_path.field("list");
                            stack.push(EvalFrame::ContainsList {
                                item,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: list,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Lookup { map, key } => {
                            let next_path = cur_path.field("map");
                            stack.push(EvalFrame::LookupMap {
                                key,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: map,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Rescale { value, scale } => {
                            let next_path = cur_path.field("value");
                            stack.push(EvalFrame::Rescale {
                                scale: *scale,
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: value,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                        Expr::Call { function, args } => {
                            if args.is_empty() {
                                step = Step::Apply(apply_call(
                                    *function,
                                    Vec::new(),
                                    &cur_env,
                                    &cur_path,
                                ));
                            } else {
                                let next_path = cur_path.index(0);
                                stack.push(EvalFrame::Call {
                                    function: *function,
                                    args,
                                    index: 0,
                                    built: Vec::with_capacity(args.len()),
                                    env: Rc::clone(&cur_env),
                                    path: cur_path,
                                });
                                step = Step::Eval {
                                    expr: &args[0],
                                    env: cur_env,
                                    path: next_path,
                                };
                            }
                        }
                        Expr::Fold(fold) => {
                            step = Step::Apply(fold.evaluate(
                                &cur_env,
                                meter,
                                &cur_path.field("fold"),
                            ));
                        }
                        Expr::Attempt { body, compensation } => {
                            let next_path = cur_path.field("body");
                            stack.push(EvalFrame::Attempt {
                                compensation,
                                env: Rc::clone(&cur_env),
                                path: cur_path,
                            });
                            step = Step::Eval {
                                expr: body,
                                env: cur_env,
                                path: next_path,
                            };
                        }
                    }
                }
                Step::Apply(Err(fault)) => {
                    let mut recovered = None;
                    while let Some(frame) = stack.pop() {
                        if let EvalFrame::Attempt {
                            compensation,
                            env: attempt_env,
                            path: attempt_path,
                        } = frame
                        {
                            if is_recoverable(&fault) {
                                recovered = Some((
                                    compensation,
                                    attempt_env,
                                    attempt_path.field("compensation"),
                                ));
                                break;
                            }
                        }
                    }
                    match recovered {
                        Some((compensation, attempt_env, next_path)) => {
                            step = Step::Eval {
                                expr: compensation,
                                env: attempt_env,
                                path: next_path,
                            };
                        }
                        None => return Err(fault),
                    }
                }
                Step::Apply(Ok(value)) => {
                    let Some(frame) = stack.pop() else {
                        return Ok(value);
                    };
                    match frame {
                        EvalFrame::Field { field, path: frame_path } => {
                            let res = match value {
                                Value::Record(record) => record.field(field).cloned().ok_or_else(|| {
                                    Diagnostic::at(
                                        DiagnosticCode::MissingValue,
                                        frame_path.location(),
                                        format!("The record value lacks the field `{field}`."),
                                        format!(
                                            "Give the field `{field}` a value before the read."
                                        ),
                                    )
                                }),
                                other => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The field read sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Read a field out of a record value.".to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::Unary { op, path: frame_path } => {
                            let res = match (op, value) {
                                (UnaryOp::Not, Value::Bool(inner)) => Ok(Value::Bool(!inner)),
                                (UnaryOp::Negate, Value::Integer(inner)) => inner
                                    .checked_neg()
                                    .map(Value::Integer)
                                    .ok_or_else(|| ArithmeticFault::Overflow.diagnose(&frame_path)),
                                (UnaryOp::Negate, Value::Decimal(inner)) => inner
                                    .negate()
                                    .map(Value::Decimal)
                                    .map_err(|fault| fault.diagnose(&frame_path)),
                                (_, other) => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The operation sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the operation to a value of the declared type."
                                        .to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::BinaryLeft {
                            op,
                            right,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let next_path = frame_path.field("right");
                            stack.push(EvalFrame::BinaryRight {
                                op,
                                left_value: value,
                                path: frame_path,
                            });
                            step = Step::Eval {
                                expr: right,
                                env: frame_env,
                                path: next_path,
                            };
                        }
                        EvalFrame::BinaryRight {
                            op,
                            left_value,
                            path: frame_path,
                        } => {
                            step = Step::Apply(apply_binary(op, left_value, value, &frame_path));
                        }
                        EvalFrame::IfCond {
                            then_branch,
                            else_branch,
                            env: frame_env,
                            path: frame_path,
                        } => match value {
                            Value::Bool(true) => {
                                step = Step::Eval {
                                    expr: then_branch,
                                    env: frame_env,
                                    path: frame_path.field("then_branch"),
                                };
                            }
                            Value::Bool(false) => {
                                step = Step::Eval {
                                    expr: else_branch,
                                    env: frame_env,
                                    path: frame_path.field("else_branch"),
                                };
                            }
                            other => {
                                step = Step::Apply(Err(type_error(
                                    frame_path.field("condition"),
                                    format!(
                                        "The condition carries {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Give the condition a truth value.".to_string(),
                                )));
                            }
                        },
                        EvalFrame::MatchScrutinee {
                            arms,
                            fallback,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let mut matched = None;
                            for (position, arm) in arms.iter().enumerate() {
                                let here = frame_path.field("arms").index(position);
                                if let Some(inner) = match_arm(&value, arm, &frame_env) {
                                    matched = Some((&arm.body, inner, here.field("body")));
                                    break;
                                }
                            }
                            if let Some((body, inner, next_path)) = matched {
                                step = Step::Eval {
                                    expr: body,
                                    env: Rc::new(inner),
                                    path: next_path,
                                };
                            } else if let Some(fallback_expr) = fallback {
                                step = Step::Eval {
                                    expr: fallback_expr,
                                    env: frame_env,
                                    path: frame_path.field("fallback"),
                                };
                            } else {
                                step = Step::Apply(Err(Diagnostic::at(
                                    DiagnosticCode::MissingValue,
                                    frame_path.location(),
                                    "Zero arms match the value supplied.".to_string(),
                                    "Add an arm for every alternative, or add a fallback."
                                        .to_string(),
                                )));
                            }
                        }
                        EvalFrame::LetValue {
                            name,
                            body,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let inner = frame_env.with(name, value);
                            step = Step::Eval {
                                expr: body,
                                env: Rc::new(inner),
                                path: frame_path.field("body"),
                            };
                        }
                        EvalFrame::MakeRecord {
                            type_id,
                            fields,
                            index,
                            mut built,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            built.push((fields[index].0.clone(), value));
                            let next_index = index + 1;
                            if next_index < fields.len() {
                                let next_path =
                                    frame_path.field(fields[next_index].0.as_str());
                                stack.push(EvalFrame::MakeRecord {
                                    type_id,
                                    fields,
                                    index: next_index,
                                    built,
                                    env: Rc::clone(&frame_env),
                                    path: frame_path,
                                });
                                step = Step::Eval {
                                    expr: &fields[next_index].1,
                                    env: frame_env,
                                    path: next_path,
                                };
                            } else {
                                step = Step::Apply(Ok(Value::record(type_id.clone(), built)));
                            }
                        }
                        EvalFrame::MakeSum { type_id, variant } => {
                            step = Step::Apply(Ok(Value::sum(
                                type_id.clone(),
                                variant.clone(),
                                Some(value),
                            )));
                        }
                        EvalFrame::MakeList {
                            element,
                            items,
                            index,
                            mut built,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            built.push(value);
                            let next_index = index + 1;
                            if next_index < items.len() {
                                let next_path = frame_path.index(next_index);
                                stack.push(EvalFrame::MakeList {
                                    element,
                                    items,
                                    index: next_index,
                                    built,
                                    env: Rc::clone(&frame_env),
                                    path: frame_path,
                                });
                                step = Step::Eval {
                                    expr: &items[next_index],
                                    env: frame_env,
                                    path: next_path,
                                };
                            } else {
                                step = Step::Apply(Ok(Value::list(element.clone(), built)));
                            }
                        }
                        EvalFrame::MakeMapKey {
                            key,
                            value: value_ty,
                            entries,
                            index,
                            built,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let next_path = frame_path.index(index).field("value");
                            stack.push(EvalFrame::MakeMapValue {
                                key,
                                value: value_ty,
                                entries,
                                index,
                                key_value: value,
                                built,
                                env: Rc::clone(&frame_env),
                                path: frame_path,
                            });
                            step = Step::Eval {
                                expr: &entries[index].1,
                                env: frame_env,
                                path: next_path,
                            };
                        }
                        EvalFrame::MakeMapValue {
                            key,
                            value: value_ty,
                            entries,
                            index,
                            key_value,
                            mut built,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            built.push((key_value, value));
                            let next_index = index + 1;
                            if next_index < entries.len() {
                                let next_path = frame_path.index(next_index).field("key");
                                stack.push(EvalFrame::MakeMapKey {
                                    key,
                                    value: value_ty,
                                    entries,
                                    index: next_index,
                                    built,
                                    env: Rc::clone(&frame_env),
                                    path: frame_path,
                                });
                                step = Step::Eval {
                                    expr: &entries[next_index].0,
                                    env: frame_env,
                                    path: next_path,
                                };
                            } else {
                                step = Step::Apply(Ok(Value::map(
                                    key.clone(),
                                    value_ty.clone(),
                                    built,
                                )));
                            }
                        }
                        EvalFrame::MakeOptional { element } => {
                            step = Step::Apply(Ok(Value::some(element.clone(), value)));
                        }
                        EvalFrame::MakeReference { collection } => {
                            step = Step::Apply(Ok(Value::Reference(RecordRef {
                                collection: collection.clone(),
                                key: Box::new(value),
                            })));
                        }
                        EvalFrame::UnwrapOr {
                            fallback,
                            env: frame_env,
                            path: frame_path,
                        } => match value {
                            Value::Optional(inner) => match inner.item {
                                Some(item) => step = Step::Apply(Ok(*item)),
                                None => {
                                    step = Step::Eval {
                                        expr: fallback,
                                        env: frame_env,
                                        path: frame_path.field("fallback"),
                                    };
                                }
                            },
                            other => {
                                step = Step::Apply(Err(type_error(
                                    frame_path,
                                    format!(
                                        "The unwrap sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the unwrap to an optional value.".to_string(),
                                )));
                            }
                        },
                        EvalFrame::IsPresent { path: frame_path } => {
                            let res = match value {
                                Value::Optional(optional) => Ok(Value::Bool(optional.item.is_some())),
                                other => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The presence test sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the presence test to an optional value.".to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::Length { path: frame_path } => {
                            let count = match value {
                                Value::List(list) => list.items.len(),
                                Value::Map(map) => map.entries.len(),
                                Value::Text(text) => text.chars().count(),
                                Value::Bytes(bytes) => bytes.len(),
                                other => {
                                    step = Step::Apply(Err(type_error(
                                        frame_path,
                                        format!(
                                            "The length reads {}.",
                                            other.infer_type().describe()
                                        ),
                                        "Read the length of a list, a map, text or bytes."
                                            .to_string(),
                                    )));
                                    continue;
                                }
                            };
                            step = Step::Apply(Ok(Value::Integer(count as i128)));
                        }
                        EvalFrame::ContainsList {
                            item,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let next_path = frame_path.field("item");
                            stack.push(EvalFrame::ContainsItem {
                                list_value: value,
                                path: frame_path,
                            });
                            step = Step::Eval {
                                expr: item,
                                env: frame_env,
                                path: next_path,
                            };
                        }
                        EvalFrame::ContainsItem {
                            list_value,
                            path: frame_path,
                        } => {
                            let res = match list_value {
                                Value::List(inner) => {
                                    meter.charge(inner.items.len() as u64, &frame_path)?;
                                    Ok(Value::Bool(inner.items.contains(&value)))
                                }
                                other => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The membership test sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the membership test to a list.".to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::LookupMap {
                            key,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            let next_path = frame_path.field("key");
                            stack.push(EvalFrame::LookupKey {
                                map_value: value,
                                path: frame_path,
                            });
                            step = Step::Eval {
                                expr: key,
                                env: frame_env,
                                path: next_path,
                            };
                        }
                        EvalFrame::LookupKey {
                            map_value,
                            path: frame_path,
                        } => {
                            let res = match map_value {
                                Value::Map(inner) => {
                                    meter.charge(inner.entries.len() as u64, &frame_path)?;
                                    let found = inner
                                        .entries
                                        .iter()
                                        .find(|(candidate, _)| candidate == &value)
                                        .map(|(_, val)| val.clone());
                                    Ok(match found {
                                        Some(val) => Value::some(inner.value.clone(), val),
                                        None => Value::none(inner.value.clone()),
                                    })
                                }
                                other => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The lookup sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the lookup to a map.".to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::Rescale {
                            scale,
                            path: frame_path,
                        } => {
                            let res = match value {
                                Value::Decimal(decimal) => decimal
                                    .rescale(scale)
                                    .map(Value::Decimal)
                                    .map_err(|fault| fault.diagnose(&frame_path)),
                                other => Err(type_error(
                                    frame_path,
                                    format!(
                                        "The rescale sits on {}.",
                                        other.infer_type().describe()
                                    ),
                                    "Apply the rescale to a decimal value.".to_string(),
                                )),
                            };
                            step = Step::Apply(res);
                        }
                        EvalFrame::Call {
                            function,
                            args,
                            index,
                            mut built,
                            env: frame_env,
                            path: frame_path,
                        } => {
                            built.push(value);
                            let next_index = index + 1;
                            if next_index < args.len() {
                                let next_path = frame_path.index(next_index);
                                stack.push(EvalFrame::Call {
                                    function,
                                    args,
                                    index: next_index,
                                    built,
                                    env: Rc::clone(&frame_env),
                                    path: frame_path,
                                });
                                step = Step::Eval {
                                    expr: &args[next_index],
                                    env: frame_env,
                                    path: next_path,
                                };
                            } else {
                                step = Step::Apply(apply_call(
                                    function,
                                    built,
                                    &frame_env,
                                    &frame_path,
                                ));
                            }
                        }
                        EvalFrame::Attempt { .. } => {
                            // Body succeeded; pass its value outward.
                            step = Step::Apply(Ok(value));
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

fn match_arm<'a>(subject: &Value, arm: &MatchArm, env: &Env<'a>) -> Option<Env<'a>> {
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

fn match_arm_context<'a>(
    ctx: &TypeContext<'a>,
    here: &Path,
    subject: &TypeExpr,
    arm: &MatchArm,
    covered: &mut Vec<VariantName>,
    saw_present: &mut bool,
    saw_absent: &mut bool,
) -> Result<TypeContext<'a>, Diagnostic> {
    match (&arm.pattern, subject) {
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
                (Some(name), Some(payload)) => Ok(ctx.with(name, payload.clone())),
                (Some(name), None) => Err(type_error(
                    here.clone(),
                    format!("The alternative `{variant}` carries no data."),
                    format!("Remove the binding `{name}` from this arm."),
                )),
                (None, _) => Ok(ctx.clone()),
            }
        }
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
            Ok(ctx.clone())
        }
        (Pattern::Present { binding }, TypeExpr::Optional(element)) => {
            *saw_present = true;
            Ok(ctx.with(binding, element.as_ref().clone()))
        }
        (Pattern::Absent, TypeExpr::Optional(_)) => {
            *saw_absent = true;
            Ok(ctx.clone())
        }
        _ => Err(type_error(
            here.clone(),
            format!("The arm matches a shape other than {}.", subject.describe()),
            "Match a sum type with variant arms, an enumeration with choice arms, and an optional with present and absent arms."
                .to_string(),
        )),
    }
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

fn finish_call_type(
    path: &Path,
    function: Builtin,
    found: &[TypeExpr],
) -> Result<TypeExpr, Diagnostic> {
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

    fn env() -> Env<'static> {
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
