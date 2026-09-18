//! The Blueprint type set.
//!
//! The set spans records, enumerations, sum types, lists, maps, optionals,
//! fixed-point decimals, timestamps, identities, hashes and references to
//! records in other collections.
//!
//! A sum type holds one value out of several named alternatives. A visual
//! editor uses a sum type for a field that is either an approval or a
//! rejection, with different data in each case.
//!
//! Fixed-point decimals carry an explicit scale. Floating point stays out of
//! the Blueprint. Determinism across nodes requires exact arithmetic.

use crate::{Diagnostic, DiagnosticCode, Path, Report};
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

/// The largest scale a fixed-point decimal carries.
pub const MAX_DECIMAL_SCALE: u8 = 30;

/// The largest depth a type expression reaches.
pub const MAX_TYPE_DEPTH: u32 = 16;

/// Check that a name suits a Blueprint identifier.
///
/// An identifier starts with a lower case letter. The rest holds lower case
/// letters, digits and underscores. The length stays at 64 or below.
pub fn validate_identifier(text: &str, label: &str, path: &Path) -> Result<(), Diagnostic> {
    if text.is_empty() {
        return Err(Diagnostic::at(
            DiagnosticCode::InvalidIdentifier,
            path.location(),
            format!("The {label} name is empty."),
            format!("Give the {label} a name such as `parcel_holder`."),
        ));
    }
    if text.len() > 64 {
        return Err(Diagnostic::at(
            DiagnosticCode::InvalidIdentifier,
            path.location(),
            format!("The {label} name `{text}` runs past 64 characters."),
            format!("Shorten the {label} name to 64 characters or fewer."),
        ));
    }
    let mut characters = text.chars();
    let first = characters.next().unwrap_or(' ');
    if !first.is_ascii_lowercase() {
        return Err(Diagnostic::at(
            DiagnosticCode::InvalidIdentifier,
            path.location(),
            format!("The {label} name `{text}` starts with `{first}`."),
            format!("Start the {label} name with a lower case letter."),
        ));
    }
    for character in characters {
        let allowed =
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_';
        if !allowed {
            return Err(Diagnostic::at(
                DiagnosticCode::InvalidIdentifier,
                path.location(),
                format!("The {label} name `{text}` holds the character `{character}`."),
                format!("Write the {label} name with lower case letters, digits and underscores."),
            ));
        }
    }
    Ok(())
}

macro_rules! declare_identifier {
    ($name:ident, $label:literal, $doc:literal) => {
        #[doc = $doc]
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
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Build the identifier from text.
            pub fn new(text: &str) -> Self {
                $name(text.to_string())
            }

            /// Read the identifier as text.
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            /// Check the spelling of the identifier.
            pub fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
                validate_identifier(self.as_str(), $label, path)
            }

            /// The kind of element this identifier names.
            pub fn label() -> &'static str {
                $label
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<&str> for $name {
            fn from(text: &str) -> Self {
                $name(text.to_string())
            }
        }
    };
}

declare_identifier!(
    TypeId,
    "type",
    "The name of a declared record, enumeration or sum type."
);
declare_identifier!(CollectionId, "collection", "The name of a collection.");
declare_identifier!(FieldName, "field", "The name of a field inside a record.");
declare_identifier!(
    VariantName,
    "variant",
    "The name of one alternative in an enumeration or a sum type."
);
declare_identifier!(
    ParamName,
    "parameter",
    "The name of an action parameter or a bound value."
);
declare_identifier!(RoleId, "role", "The name of a role.");
declare_identifier!(UnitId, "unit", "The name of an organisational unit.");
declare_identifier!(
    CredentialId,
    "credential",
    "The name of a credential requirement."
);
declare_identifier!(DelegationId, "delegation", "The name of a delegation rule.");
declare_identifier!(ActionId, "action", "The name of an action.");
declare_identifier!(EventId, "event", "The name of an event.");
declare_identifier!(RuleId, "rule", "The name of a rule.");
declare_identifier!(WorkflowId, "workflow", "The name of a workflow.");
declare_identifier!(StateId, "state", "The name of a workflow state.");
declare_identifier!(
    TransitionId,
    "transition",
    "The name of a workflow transition."
);
declare_identifier!(TimerId, "timer", "The name of a workflow timer.");
declare_identifier!(ViewId, "view", "The name of a view.");
declare_identifier!(
    BlockId,
    "block",
    "The name of a reusable composition block."
);
declare_identifier!(
    IntegrationId,
    "integration",
    "The name of a feed, a bridge endpoint or an external trigger."
);
declare_identifier!(IndexName, "index", "The name of a secondary index.");
declare_identifier!(PurposeTag, "purpose", "The name of a declared purpose.");

/// A path to a field inside a record.
///
/// The path `holder.address.district` reaches three levels down.
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
#[serde(transparent)]
pub struct FieldPath(pub Vec<FieldName>);

impl FieldPath {
    /// Build a field path from dotted text such as `holder.district`.
    pub fn parse(text: &str) -> Self {
        FieldPath(
            text.split('.')
                .filter(|part| !part.is_empty())
                .map(FieldName::new)
                .collect(),
        )
    }

    /// Report whether the path names zero fields.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Count the steps in the path.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check every step of the path.
    pub fn validate(&self, path: &Path) -> Result<(), Diagnostic> {
        if self.0.is_empty() {
            return Err(Diagnostic::new(
                DiagnosticCode::MissingValue,
                path.clone(),
                "The field path names zero fields.",
                "Name at least one field such as `holder`.",
            ));
        }
        for (position, field) in self.0.iter().enumerate() {
            field.validate(&path.index(position))?;
        }
        Ok(())
    }

    /// Follow the path through a type and report the type it reaches.
    pub fn resolve(&self, start: &TypeExpr, registry: &TypeRegistry) -> Option<TypeExpr> {
        let mut current = start.clone();
        for step in &self.0 {
            let record_id = match &current {
                TypeExpr::Record(id) => id.clone(),
                TypeExpr::Optional(inner) => match inner.as_ref() {
                    TypeExpr::Record(id) => id.clone(),
                    _ => return None,
                },
                _ => return None,
            };
            let record = registry.record(&record_id)?;
            let field = record
                .fields
                .iter()
                .find(|candidate| &candidate.name == step)?;
            current = field.ty.clone();
        }
        Some(current)
    }
}

impl core::fmt::Display for FieldPath {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut first = true;
        for field in &self.0 {
            if first {
                first = false;
            } else {
                f.write_str(".")?;
            }
            f.write_str(field.as_str())?;
        }
        Ok(())
    }
}

/// A fault raised by exact arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ArithmeticFault {
    /// The result runs past the range of the integer carrying it.
    Overflow,
    /// The divisor is zero.
    DivisionByZero,
    /// The exact result needs more digits than the declared scale.
    PrecisionLoss,
    /// Two decimals carry different scales.
    ScaleMismatch,
    /// The requested scale runs past the permitted maximum.
    ScaleTooLarge,
}

impl ArithmeticFault {
    /// The diagnostic class this fault belongs to.
    pub fn code(&self) -> DiagnosticCode {
        match self {
            ArithmeticFault::Overflow => DiagnosticCode::ArithmeticOverflow,
            ArithmeticFault::DivisionByZero => DiagnosticCode::DivisionByZero,
            ArithmeticFault::PrecisionLoss => DiagnosticCode::PrecisionLoss,
            ArithmeticFault::ScaleMismatch => DiagnosticCode::TypeMismatch,
            ArithmeticFault::ScaleTooLarge => DiagnosticCode::ValueOutOfRange,
        }
    }

    /// One sentence stating the problem.
    pub fn problem(&self) -> &'static str {
        match self {
            ArithmeticFault::Overflow => "The arithmetic result runs past the integer range.",
            ArithmeticFault::DivisionByZero => "The divisor is zero.",
            ArithmeticFault::PrecisionLoss => {
                "The exact result needs more digits than the declared scale."
            }
            ArithmeticFault::ScaleMismatch => "The two decimals carry different scales.",
            ArithmeticFault::ScaleTooLarge => "The decimal scale runs past the permitted maximum.",
        }
    }

    /// One sentence stating the fix.
    pub fn fix(&self) -> &'static str {
        match self {
            ArithmeticFault::Overflow => "Reduce the operand sizes or raise the declared scale.",
            ArithmeticFault::DivisionByZero => {
                "Guard the division with a check that the divisor holds a value above zero."
            }
            ArithmeticFault::PrecisionLoss => {
                "Raise the declared scale of the result field so the exact answer fits."
            }
            ArithmeticFault::ScaleMismatch => {
                "Apply the rescale operation to one side so both carry the same scale."
            }
            ArithmeticFault::ScaleTooLarge => "Choose a scale of 30 or below.",
        }
    }

    /// Turn the fault into a diagnostic sitting at the given location.
    pub fn diagnose(&self, path: &Path) -> Diagnostic {
        Diagnostic::new(self.code(), path.clone(), self.problem(), self.fix())
    }
}

/// A fixed-point decimal.
///
/// The value equals `units` divided by ten raised to `scale`. A scale of 2
/// and units of 1250 carry the value 12.50.
#[derive(
    Clone,
    Copy,
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
pub struct Decimal {
    /// The scaled integer holding every digit of the value.
    pub units: i128,
    /// The count of digits sitting after the decimal point.
    pub scale: u8,
}

impl Decimal {
    /// Build a decimal from its units and its scale.
    pub fn new(units: i128, scale: u8) -> Self {
        Decimal { units, scale }
    }

    /// The zero value at the given scale.
    pub fn zero(scale: u8) -> Self {
        Decimal { units: 0, scale }
    }

    /// Report whether the value equals zero.
    pub fn is_zero(&self) -> bool {
        self.units == 0
    }

    /// Ten raised to the given power, as an exact integer.
    pub fn power_of_ten(exponent: u8) -> Result<i128, ArithmeticFault> {
        if exponent > MAX_DECIMAL_SCALE {
            return Err(ArithmeticFault::ScaleTooLarge);
        }
        let mut result: i128 = 1;
        for _ in 0..exponent {
            result = result.checked_mul(10).ok_or(ArithmeticFault::Overflow)?;
        }
        Ok(result)
    }

    /// Move the value to another scale without losing a digit.
    pub fn rescale(&self, target: u8) -> Result<Decimal, ArithmeticFault> {
        if target > MAX_DECIMAL_SCALE {
            return Err(ArithmeticFault::ScaleTooLarge);
        }
        if target == self.scale {
            return Ok(*self);
        }
        if target > self.scale {
            let factor = Decimal::power_of_ten(target - self.scale)?;
            let units = self
                .units
                .checked_mul(factor)
                .ok_or(ArithmeticFault::Overflow)?;
            return Ok(Decimal {
                units,
                scale: target,
            });
        }
        let factor = Decimal::power_of_ten(self.scale - target)?;
        if self.units % factor != 0 {
            return Err(ArithmeticFault::PrecisionLoss);
        }
        Ok(Decimal {
            units: self.units / factor,
            scale: target,
        })
    }

    /// Add two decimals carrying the same scale.
    pub fn add(&self, other: &Decimal) -> Result<Decimal, ArithmeticFault> {
        if self.scale != other.scale {
            return Err(ArithmeticFault::ScaleMismatch);
        }
        let units = self
            .units
            .checked_add(other.units)
            .ok_or(ArithmeticFault::Overflow)?;
        Ok(Decimal {
            units,
            scale: self.scale,
        })
    }

    /// Subtract two decimals carrying the same scale.
    pub fn subtract(&self, other: &Decimal) -> Result<Decimal, ArithmeticFault> {
        if self.scale != other.scale {
            return Err(ArithmeticFault::ScaleMismatch);
        }
        let units = self
            .units
            .checked_sub(other.units)
            .ok_or(ArithmeticFault::Overflow)?;
        Ok(Decimal {
            units,
            scale: self.scale,
        })
    }

    /// Multiply two decimals. The result scale is the sum of the two scales.
    pub fn multiply(&self, other: &Decimal) -> Result<Decimal, ArithmeticFault> {
        let scale = self
            .scale
            .checked_add(other.scale)
            .ok_or(ArithmeticFault::ScaleTooLarge)?;
        if scale > MAX_DECIMAL_SCALE {
            return Err(ArithmeticFault::ScaleTooLarge);
        }
        let units = self
            .units
            .checked_mul(other.units)
            .ok_or(ArithmeticFault::Overflow)?;
        Ok(Decimal { units, scale })
    }

    /// Divide two decimals carrying the same scale.
    ///
    /// The result carries that scale. An answer needing more digits raises a
    /// precision fault.
    pub fn divide(&self, other: &Decimal) -> Result<Decimal, ArithmeticFault> {
        if self.scale != other.scale {
            return Err(ArithmeticFault::ScaleMismatch);
        }
        if other.units == 0 {
            return Err(ArithmeticFault::DivisionByZero);
        }
        let factor = Decimal::power_of_ten(self.scale)?;
        let lifted = self
            .units
            .checked_mul(factor)
            .ok_or(ArithmeticFault::Overflow)?;
        if lifted % other.units != 0 {
            return Err(ArithmeticFault::PrecisionLoss);
        }
        Ok(Decimal {
            units: lifted / other.units,
            scale: self.scale,
        })
    }

    /// Reverse the sign of the value.
    pub fn negate(&self) -> Result<Decimal, ArithmeticFault> {
        let units = self.units.checked_neg().ok_or(ArithmeticFault::Overflow)?;
        Ok(Decimal {
            units,
            scale: self.scale,
        })
    }

    /// Compare two decimals carrying the same scale.
    pub fn compare(&self, other: &Decimal) -> Result<core::cmp::Ordering, ArithmeticFault> {
        if self.scale != other.scale {
            return Err(ArithmeticFault::ScaleMismatch);
        }
        Ok(self.units.cmp(&other.units))
    }
}

impl core::fmt::Display for Decimal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.scale == 0 {
            return write!(f, "{}", self.units);
        }
        let factor = match Decimal::power_of_ten(self.scale) {
            Ok(value) => value,
            Err(_) => return write!(f, "{}e-{}", self.units, self.scale),
        };
        let sign = if self.units < 0 { "-" } else { "" };
        let magnitude = self.units.unsigned_abs();
        let whole = magnitude / (factor as u128);
        let part = magnitude % (factor as u128);
        write!(
            f,
            "{}{}.{:0width$}",
            sign,
            whole,
            part,
            width = self.scale as usize
        )
    }
}

/// A point in time, held as milliseconds since the start of 1970.
#[derive(
    Clone,
    Copy,
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
#[serde(transparent)]
pub struct Timestamp(pub u64);

impl Timestamp {
    /// Build a timestamp from milliseconds.
    pub fn from_millis(millis: u64) -> Self {
        Timestamp(millis)
    }

    /// Read the timestamp as milliseconds.
    pub fn millis(&self) -> u64 {
        self.0
    }

    /// Move the timestamp forward by a span of milliseconds.
    pub fn plus_millis(&self, span: u64) -> Result<Timestamp, ArithmeticFault> {
        self.0
            .checked_add(span)
            .map(Timestamp)
            .ok_or(ArithmeticFault::Overflow)
    }

    /// The span in milliseconds from an earlier timestamp to this one.
    pub fn since(&self, earlier: &Timestamp) -> Result<u64, ArithmeticFault> {
        self.0
            .checked_sub(earlier.0)
            .ok_or(ArithmeticFault::Overflow)
    }
}

/// An account or an organisation, held as a 32 byte identifier.
#[derive(
    Clone,
    Copy,
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
pub struct Identity(pub [u8; 32]);

impl Identity {
    /// Read the identifier as bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A 32 byte hash.
#[derive(
    Clone,
    Copy,
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
pub struct Hash32(pub [u8; 32]);

impl Hash32 {
    /// Hash a byte string with the Blueprint hash.
    pub fn of(bytes: &[u8]) -> Hash32 {
        Hash32(sp_crypto_hashing::blake2_256(bytes))
    }

    /// Read the hash as bytes.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Write the hash as lower case hexadecimal text.
    pub fn to_hex(&self) -> String {
        let mut text = String::with_capacity(64);
        for byte in self.0.iter() {
            text.push(hex_digit(byte >> 4));
            text.push(hex_digit(byte & 0x0f));
        }
        text
    }
}

fn hex_digit(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        _ => (b'a' + nibble - 10) as char,
    }
}

/// A pointer to one record held in a collection.
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
pub struct RecordRef {
    /// The collection holding the record.
    pub collection: CollectionId,
    /// The primary key of the record.
    pub key: Box<Value>,
}

/// A type in the Blueprint.
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
pub enum TypeExpr {
    /// A truth value.
    Bool,
    /// A whole number.
    Integer,
    /// A fixed-point decimal carrying the given scale.
    Decimal {
        /// The count of digits sitting after the decimal point.
        scale: u8,
    },
    /// A run of characters.
    Text,
    /// A run of bytes.
    Bytes,
    /// A point in time.
    Timestamp,
    /// An account or an organisation.
    Identity,
    /// A 32 byte hash.
    Hash,
    /// An ordered run of values sharing one element type.
    List(Box<TypeExpr>),
    /// A set of key and value pairs sorted by key.
    Map {
        /// The type of every key.
        key: Box<TypeExpr>,
        /// The type of every value.
        value: Box<TypeExpr>,
    },
    /// A value that is present or absent.
    Optional(Box<TypeExpr>),
    /// A declared record type.
    Record(TypeId),
    /// A declared enumeration type.
    Enumeration(TypeId),
    /// A declared sum type.
    Sum(TypeId),
    /// A pointer to a record in the named collection.
    Reference(CollectionId),
}

impl TypeExpr {
    /// A list of the given element type.
    pub fn list(element: TypeExpr) -> TypeExpr {
        TypeExpr::List(Box::new(element))
    }

    /// A map of the given key and value types.
    pub fn map(key: TypeExpr, value: TypeExpr) -> TypeExpr {
        TypeExpr::Map {
            key: Box::new(key),
            value: Box::new(value),
        }
    }

    /// An optional of the given element type.
    pub fn optional(element: TypeExpr) -> TypeExpr {
        TypeExpr::Optional(Box::new(element))
    }

    /// A plain sentence fragment naming the type.
    pub fn describe(&self) -> String {
        match self {
            TypeExpr::Bool => "a truth value".to_string(),
            TypeExpr::Integer => "a whole number".to_string(),
            TypeExpr::Decimal { scale } => format!("a decimal at scale {scale}"),
            TypeExpr::Text => "text".to_string(),
            TypeExpr::Bytes => "bytes".to_string(),
            TypeExpr::Timestamp => "a timestamp".to_string(),
            TypeExpr::Identity => "an identity".to_string(),
            TypeExpr::Hash => "a hash".to_string(),
            TypeExpr::List(element) => format!("a list of {}", element.describe()),
            TypeExpr::Map { key, value } => {
                format!("a map from {} to {}", key.describe(), value.describe())
            }
            TypeExpr::Optional(element) => format!("an optional {}", element.describe()),
            TypeExpr::Record(id) => format!("the record `{id}`"),
            TypeExpr::Enumeration(id) => format!("the enumeration `{id}`"),
            TypeExpr::Sum(id) => format!("the sum type `{id}`"),
            TypeExpr::Reference(id) => format!("a reference into `{id}`"),
        }
    }

    /// The depth of the type expression.
    pub fn depth(&self) -> u32 {
        match self {
            TypeExpr::List(element) | TypeExpr::Optional(element) => 1 + element.depth(),
            TypeExpr::Map { key, value } => 1 + key.depth().max(value.depth()),
            _ => 1,
        }
    }

    /// Report whether a value of this type serves as a map key or a primary
    /// key. Keys carry a total order and a canonical encoding.
    pub fn is_key_safe(&self) -> bool {
        match self {
            TypeExpr::Bool
            | TypeExpr::Integer
            | TypeExpr::Decimal { .. }
            | TypeExpr::Text
            | TypeExpr::Bytes
            | TypeExpr::Timestamp
            | TypeExpr::Identity
            | TypeExpr::Hash
            | TypeExpr::Enumeration(_)
            | TypeExpr::Reference(_) => true,
            TypeExpr::List(element) => element.is_key_safe(),
            TypeExpr::Record(_) => true,
            TypeExpr::Map { .. } | TypeExpr::Optional(_) | TypeExpr::Sum(_) => false,
        }
    }

    /// Collect every collection named by a reference inside this type.
    pub fn collection_refs(&self, out: &mut Vec<CollectionId>) {
        match self {
            TypeExpr::Reference(id) => out.push(id.clone()),
            TypeExpr::List(element) | TypeExpr::Optional(element) => element.collection_refs(out),
            TypeExpr::Map { key, value } => {
                key.collection_refs(out);
                value.collection_refs(out);
            }
            _ => {}
        }
    }

    /// Check that every named type resolves and every scale stays in range.
    pub fn validate(&self, registry: &TypeRegistry, path: &Path) -> Result<(), Diagnostic> {
        if self.depth() > MAX_TYPE_DEPTH {
            return Err(Diagnostic::new(
                DiagnosticCode::ValueOutOfRange,
                path.clone(),
                "The type nests deeper than sixteen levels.",
                "Give the inner structure a named record type and point at that name.",
            ));
        }
        match self {
            TypeExpr::Decimal { scale } => {
                if *scale > MAX_DECIMAL_SCALE {
                    Err(Diagnostic::at(
                        DiagnosticCode::ValueOutOfRange,
                        path.location(),
                        format!("The decimal scale {scale} runs past the maximum of 30."),
                        "Choose a scale of 30 or below.".to_string(),
                    ))
                } else {
                    Ok(())
                }
            }
            TypeExpr::List(element) | TypeExpr::Optional(element) => {
                element.validate(registry, path)
            }
            TypeExpr::Map { key, value } => {
                if !key.is_key_safe() {
                    return Err(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.location(),
                        format!(
                            "The map key carries {}, which lacks a total order.",
                            key.describe()
                        ),
                        "Choose a key type such as text, a whole number or an identity."
                            .to_string(),
                    ));
                }
                key.validate(registry, path)?;
                value.validate(registry, path)
            }
            TypeExpr::Record(id) => {
                if registry.record(id).is_some() {
                    Ok(())
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The type names the record `{id}`, which the Blueprint lacks."),
                        format!("Declare a record type named `{id}` in the types section."),
                    ))
                }
            }
            TypeExpr::Enumeration(id) => {
                if registry.enumeration(id).is_some() {
                    Ok(())
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The type names the enumeration `{id}`, which the Blueprint lacks."
                        ),
                        format!("Declare an enumeration named `{id}` in the types section."),
                    ))
                }
            }
            TypeExpr::Sum(id) => {
                if registry.sum(id).is_some() {
                    Ok(())
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!("The type names the sum type `{id}`, which the Blueprint lacks."),
                        format!("Declare a sum type named `{id}` in the types section."),
                    ))
                }
            }
            _ => Ok(()),
        }
    }
}

/// One field of a record.
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
pub struct FieldDef {
    /// The name of the field.
    pub name: FieldName,
    /// The type of the field.
    pub ty: TypeExpr,
    /// One plain sentence stating what the field holds.
    pub summary: String,
}

impl FieldDef {
    /// Declare a field.
    pub fn new(name: &str, ty: TypeExpr, summary: &str) -> Self {
        FieldDef {
            name: FieldName::new(name),
            ty,
            summary: summary.to_string(),
        }
    }
}

/// A record type. A record holds a fixed set of named fields.
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
pub struct RecordDef {
    /// The name of the record type.
    pub id: TypeId,
    /// The fields, in declaration order.
    pub fields: Vec<FieldDef>,
    /// One plain sentence stating what the record holds.
    pub summary: String,
}

impl RecordDef {
    /// Find one field by name.
    pub fn field(&self, name: &FieldName) -> Option<&FieldDef> {
        self.fields.iter().find(|field| &field.name == name)
    }
}

/// An enumeration type. An enumeration holds one name out of a fixed set.
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
pub struct EnumerationDef {
    /// The name of the enumeration type.
    pub id: TypeId,
    /// The alternatives, in declaration order.
    pub variants: Vec<VariantName>,
    /// One plain sentence stating what the enumeration holds.
    pub summary: String,
}

/// One alternative of a sum type.
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
pub struct SumVariant {
    /// The name of the alternative.
    pub name: VariantName,
    /// The data the alternative carries. An alternative carrying no data
    /// holds an absent payload.
    pub payload: Option<TypeExpr>,
}

/// A sum type. A sum type holds one value out of several named alternatives.
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
pub struct SumDef {
    /// The name of the sum type.
    pub id: TypeId,
    /// The alternatives, in declaration order.
    pub variants: Vec<SumVariant>,
    /// One plain sentence stating what the sum type holds.
    pub summary: String,
}

impl SumDef {
    /// Find one alternative by name.
    pub fn variant(&self, name: &VariantName) -> Option<&SumVariant> {
        self.variants.iter().find(|variant| &variant.name == name)
    }
}

/// A declared type.
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
pub enum TypeDef {
    /// A record type.
    Record(RecordDef),
    /// An enumeration type.
    Enumeration(EnumerationDef),
    /// A sum type.
    Sum(SumDef),
}

impl TypeDef {
    /// The name of the declared type.
    pub fn id(&self) -> &TypeId {
        match self {
            TypeDef::Record(def) => &def.id,
            TypeDef::Enumeration(def) => &def.id,
            TypeDef::Sum(def) => &def.id,
        }
    }

    /// The kind of declared type, as a word.
    pub fn kind(&self) -> &'static str {
        match self {
            TypeDef::Record(_) => "record",
            TypeDef::Enumeration(_) => "enumeration",
            TypeDef::Sum(_) => "sum type",
        }
    }
}

/// An index over the declared types.
///
/// The registry answers every lookup the checker makes.
#[derive(Clone, Debug, Default)]
pub struct TypeRegistry<'a> {
    entries: BTreeMap<&'a str, &'a TypeDef>,
}

impl<'a> TypeRegistry<'a> {
    /// Build a registry over a slice of declared types.
    ///
    /// A repeated name raises a diagnostic and the first declaration wins.
    pub fn build(defs: &'a [TypeDef], path: &Path) -> (Self, Report) {
        let mut entries: BTreeMap<&'a str, &'a TypeDef> = BTreeMap::new();
        let mut report = Report::new();
        for (position, def) in defs.iter().enumerate() {
            let here = path.index(position);
            if entries.contains_key(def.id().as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!("Two declared types share the name `{}`.", def.id()),
                    format!("Rename one of the two types away from `{}`.", def.id()),
                ));
                continue;
            }
            entries.insert(def.id().as_str(), def);
        }
        (TypeRegistry { entries }, report)
    }

    /// Look up any declared type.
    pub fn get(&self, id: &TypeId) -> Option<&'a TypeDef> {
        self.entries.get(id.as_str()).copied()
    }

    /// Look up a record type.
    pub fn record(&self, id: &TypeId) -> Option<&'a RecordDef> {
        match self.get(id) {
            Some(TypeDef::Record(def)) => Some(def),
            _ => None,
        }
    }

    /// Look up an enumeration type.
    pub fn enumeration(&self, id: &TypeId) -> Option<&'a EnumerationDef> {
        match self.get(id) {
            Some(TypeDef::Enumeration(def)) => Some(def),
            _ => None,
        }
    }

    /// Look up a sum type.
    pub fn sum(&self, id: &TypeId) -> Option<&'a SumDef> {
        match self.get(id) {
            Some(TypeDef::Sum(def)) => Some(def),
            _ => None,
        }
    }

    /// Count the declared types.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero types.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// A list value carrying its element type.
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
pub struct ListValue {
    /// The type of every element.
    pub element: TypeExpr,
    /// The elements, in order.
    pub items: Vec<Value>,
}

/// A map value carrying its key and value types.
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
pub struct MapValue {
    /// The type of every key.
    pub key: TypeExpr,
    /// The type of every value.
    pub value: TypeExpr,
    /// The pairs, sorted by key in canonical form.
    pub entries: Vec<(Value, Value)>,
}

/// An optional value carrying its element type.
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
pub struct OptionalValue {
    /// The type of the element.
    pub element: TypeExpr,
    /// The element, present or absent.
    pub item: Option<Box<Value>>,
}

/// A record value.
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
pub struct RecordValue {
    /// The declared record type.
    pub type_id: TypeId,
    /// The fields. Canonical form sorts the fields by name.
    pub fields: Vec<(FieldName, Value)>,
}

impl RecordValue {
    /// Read one field.
    pub fn field(&self, name: &FieldName) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
}

/// An enumeration value.
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
pub struct EnumerationValue {
    /// The declared enumeration type.
    pub type_id: TypeId,
    /// The chosen alternative.
    pub variant: VariantName,
}

/// A sum value.
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
pub struct SumValue {
    /// The declared sum type.
    pub type_id: TypeId,
    /// The chosen alternative.
    pub variant: VariantName,
    /// The data the alternative carries.
    pub payload: Option<Box<Value>>,
}

/// A Blueprint value.
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
pub enum Value {
    /// A truth value.
    Bool(bool),
    /// A whole number.
    Integer(i128),
    /// A fixed-point decimal.
    Decimal(Decimal),
    /// A run of characters.
    Text(String),
    /// A run of bytes.
    Bytes(Vec<u8>),
    /// A point in time.
    Timestamp(Timestamp),
    /// An account or an organisation.
    Identity(Identity),
    /// A 32 byte hash.
    Hash(Hash32),
    /// An ordered run of values.
    List(ListValue),
    /// A set of key and value pairs.
    Map(MapValue),
    /// A value that is present or absent.
    Optional(OptionalValue),
    /// A record value.
    Record(RecordValue),
    /// An enumeration value.
    Enumeration(EnumerationValue),
    /// A sum value.
    Sum(SumValue),
    /// A pointer to a record in a collection.
    Reference(RecordRef),
}

impl Value {
    /// Build a list value.
    pub fn list(element: TypeExpr, items: Vec<Value>) -> Value {
        Value::List(ListValue { element, items })
    }

    /// Build a map value with its pairs sorted by key.
    pub fn map(key: TypeExpr, value: TypeExpr, mut entries: Vec<(Value, Value)>) -> Value {
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries.dedup_by(|left, right| left.0 == right.0);
        Value::Map(MapValue {
            key,
            value,
            entries,
        })
    }

    /// Build a present optional value.
    pub fn some(element: TypeExpr, item: Value) -> Value {
        Value::Optional(OptionalValue {
            element,
            item: Some(Box::new(item)),
        })
    }

    /// Build an absent optional value.
    pub fn none(element: TypeExpr) -> Value {
        Value::Optional(OptionalValue {
            element,
            item: None,
        })
    }

    /// Build a record value with its fields sorted by name.
    pub fn record(type_id: TypeId, mut fields: Vec<(FieldName, Value)>) -> Value {
        fields.sort_by(|left, right| left.0.cmp(&right.0));
        Value::Record(RecordValue { type_id, fields })
    }

    /// Build an enumeration value.
    pub fn enumeration(type_id: TypeId, variant: VariantName) -> Value {
        Value::Enumeration(EnumerationValue { type_id, variant })
    }

    /// Build a sum value.
    pub fn sum(type_id: TypeId, variant: VariantName, payload: Option<Value>) -> Value {
        Value::Sum(SumValue {
            type_id,
            variant,
            payload: payload.map(Box::new),
        })
    }

    /// Build a reference value.
    pub fn reference(collection: CollectionId, key: Value) -> Value {
        Value::Reference(RecordRef {
            collection,
            key: Box::new(key),
        })
    }

    /// The type this value carries.
    ///
    /// Every value names its own element types, so inference is total.
    pub fn infer_type(&self) -> TypeExpr {
        match self {
            Value::Bool(_) => TypeExpr::Bool,
            Value::Integer(_) => TypeExpr::Integer,
            Value::Decimal(decimal) => TypeExpr::Decimal {
                scale: decimal.scale,
            },
            Value::Text(_) => TypeExpr::Text,
            Value::Bytes(_) => TypeExpr::Bytes,
            Value::Timestamp(_) => TypeExpr::Timestamp,
            Value::Identity(_) => TypeExpr::Identity,
            Value::Hash(_) => TypeExpr::Hash,
            Value::List(list) => TypeExpr::list(list.element.clone()),
            Value::Map(map) => TypeExpr::map(map.key.clone(), map.value.clone()),
            Value::Optional(optional) => TypeExpr::optional(optional.element.clone()),
            Value::Record(record) => TypeExpr::Record(record.type_id.clone()),
            Value::Enumeration(value) => TypeExpr::Enumeration(value.type_id.clone()),
            Value::Sum(value) => TypeExpr::Sum(value.type_id.clone()),
            Value::Reference(reference) => TypeExpr::Reference(reference.collection.clone()),
        }
    }

    /// Sort every map and every record into canonical order.
    ///
    /// Two values carrying the same content produce the same encoding after
    /// this pass.
    pub fn canonicalise(&mut self) {
        match self {
            Value::List(list) => {
                for item in list.items.iter_mut() {
                    item.canonicalise();
                }
            }
            Value::Map(map) => {
                for entry in map.entries.iter_mut() {
                    entry.0.canonicalise();
                    entry.1.canonicalise();
                }
                map.entries.sort_by(|left, right| left.0.cmp(&right.0));
                map.entries.dedup_by(|left, right| left.0 == right.0);
            }
            Value::Optional(optional) => {
                if let Some(item) = optional.item.as_mut() {
                    item.canonicalise();
                }
            }
            Value::Record(record) => {
                for entry in record.fields.iter_mut() {
                    entry.1.canonicalise();
                }
                record.fields.sort_by(|left, right| left.0.cmp(&right.0));
            }
            Value::Sum(value) => {
                if let Some(payload) = value.payload.as_mut() {
                    payload.canonicalise();
                }
            }
            Value::Reference(reference) => reference.key.canonicalise(),
            _ => {}
        }
    }

    /// The canonical byte string of this value.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut copy = self.clone();
        copy.canonicalise();
        copy.encode()
    }

    /// Check the value against a declared type.
    pub fn check(
        &self,
        expected: &TypeExpr,
        registry: &TypeRegistry,
        path: &Path,
    ) -> Result<(), Diagnostic> {
        match (self, expected) {
            (Value::Bool(_), TypeExpr::Bool) => Ok(()),
            (Value::Integer(_), TypeExpr::Integer) => Ok(()),
            (Value::Decimal(decimal), TypeExpr::Decimal { scale }) => {
                if decimal.scale == *scale {
                    Ok(())
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.location(),
                        format!(
                            "The value carries a decimal at scale {}, and the field declares scale {}.",
                            decimal.scale, scale
                        ),
                        format!("Apply the rescale operation to move the value to scale {scale}."),
                    ))
                }
            }
            (Value::Text(_), TypeExpr::Text) => Ok(()),
            (Value::Bytes(_), TypeExpr::Bytes) => Ok(()),
            (Value::Timestamp(_), TypeExpr::Timestamp) => Ok(()),
            (Value::Identity(_), TypeExpr::Identity) => Ok(()),
            (Value::Hash(_), TypeExpr::Hash) => Ok(()),
            (Value::List(list), TypeExpr::List(element)) => {
                if &list.element != element.as_ref() {
                    return Err(mismatch(self, expected, path));
                }
                for (position, item) in list.items.iter().enumerate() {
                    item.check(element, registry, &path.index(position))?;
                }
                Ok(())
            }
            (Value::Map(map), TypeExpr::Map { key, value }) => {
                if &map.key != key.as_ref() || &map.value != value.as_ref() {
                    return Err(mismatch(self, expected, path));
                }
                for (position, entry) in map.entries.iter().enumerate() {
                    let here = path.index(position);
                    entry.0.check(key, registry, &here.field("key"))?;
                    entry.1.check(value, registry, &here.field("value"))?;
                }
                Ok(())
            }
            (Value::Optional(optional), TypeExpr::Optional(element)) => {
                if &optional.element != element.as_ref() {
                    return Err(mismatch(self, expected, path));
                }
                match &optional.item {
                    Some(item) => item.check(element, registry, path),
                    None => Ok(()),
                }
            }
            (Value::Record(record), TypeExpr::Record(type_id)) => {
                if &record.type_id != type_id {
                    return Err(mismatch(self, expected, path));
                }
                let def = registry.record(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The value names the record `{type_id}`, which the Blueprint lacks."
                        ),
                        format!("Declare a record type named `{type_id}` in the types section."),
                    )
                })?;
                for field in &def.fields {
                    let here = path.field(field.name.as_str());
                    let held = record.field(&field.name).ok_or_else(|| {
                        Diagnostic::at(
                            DiagnosticCode::MissingValue,
                            here.location(),
                            format!(
                                "The record value lacks the field `{}` declared by `{}`.",
                                field.name, type_id
                            ),
                            format!("Give the field `{}` a value.", field.name),
                        )
                    })?;
                    held.check(&field.ty, registry, &here)?;
                }
                for (name, _) in &record.fields {
                    if def.field(name).is_none() {
                        return Err(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.field(name.as_str()).location(),
                            format!("The record value holds `{name}`, which `{type_id}` lacks."),
                            format!(
                                "Remove `{name}` or declare it on the record type `{type_id}`."
                            ),
                        ));
                    }
                }
                Ok(())
            }
            (Value::Enumeration(value), TypeExpr::Enumeration(type_id)) => {
                if &value.type_id != type_id {
                    return Err(mismatch(self, expected, path));
                }
                let def = registry.enumeration(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The value names the enumeration `{type_id}`, which the Blueprint lacks."
                        ),
                        format!("Declare an enumeration named `{type_id}` in the types section."),
                    )
                })?;
                if def.variants.contains(&value.variant) {
                    Ok(())
                } else {
                    Err(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The value names the alternative `{}`, which `{type_id}` lacks.",
                            value.variant
                        ),
                        format!("Choose an alternative declared on `{type_id}`."),
                    ))
                }
            }
            (Value::Sum(value), TypeExpr::Sum(type_id)) => {
                if &value.type_id != type_id {
                    return Err(mismatch(self, expected, path));
                }
                let def = registry.sum(type_id).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The value names the sum type `{type_id}`, which the Blueprint lacks."
                        ),
                        format!("Declare a sum type named `{type_id}` in the types section."),
                    )
                })?;
                let variant = def.variant(&value.variant).ok_or_else(|| {
                    Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        path.location(),
                        format!(
                            "The value names the alternative `{}`, which `{type_id}` lacks.",
                            value.variant
                        ),
                        format!("Choose an alternative declared on `{type_id}`."),
                    )
                })?;
                match (&variant.payload, &value.payload) {
                    (Some(declared), Some(held)) => {
                        held.check(declared, registry, &path.field("payload"))
                    }
                    (None, None) => Ok(()),
                    (Some(declared), None) => Err(Diagnostic::at(
                        DiagnosticCode::MissingValue,
                        path.field("payload").location(),
                        format!(
                            "The alternative `{}` carries {} and the value carries nothing.",
                            value.variant,
                            declared.describe()
                        ),
                        format!("Give the alternative `{}` its data.", value.variant),
                    )),
                    (None, Some(_)) => Err(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.field("payload").location(),
                        format!("The alternative `{}` carries no data.", value.variant),
                        format!("Remove the data from the alternative `{}`.", value.variant),
                    )),
                }
            }
            (Value::Reference(reference), TypeExpr::Reference(collection)) => {
                if &reference.collection == collection {
                    Ok(())
                } else {
                    Err(mismatch(self, expected, path))
                }
            }
            _ => Err(mismatch(self, expected, path)),
        }
    }
}

fn mismatch(value: &Value, expected: &TypeExpr, path: &Path) -> Diagnostic {
    Diagnostic::at(
        DiagnosticCode::TypeMismatch,
        path.location(),
        format!(
            "The value carries {} and the declared type is {}.",
            value.infer_type().describe(),
            expected.describe()
        ),
        format!(
            "Supply {} here, or change the declared type.",
            expected.describe()
        ),
    )
}

/// Check a whole types section.
///
/// The pass reports repeated names, malformed identifiers, repeated field
/// names and every type that names an element the Blueprint lacks.
pub fn validate_types(defs: &[TypeDef], path: &Path) -> Report {
    let (registry, mut report) = TypeRegistry::build(defs, path);
    for (position, def) in defs.iter().enumerate() {
        let here = path.index(position);
        report.absorb(def.id().validate(&here.field("id")));
        match def {
            TypeDef::Record(record) => {
                let mut seen: Vec<&FieldName> = Vec::new();
                for (index, field) in record.fields.iter().enumerate() {
                    let field_path = here.field("fields").index(index);
                    report.absorb(field.name.validate(&field_path.field("name")));
                    if seen.contains(&&field.name) {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DuplicateName,
                            field_path.field("name").location(),
                            format!(
                                "The record `{}` declares the field `{}` twice.",
                                record.id, field.name
                            ),
                            format!("Rename one of the two `{}` fields.", field.name),
                        ));
                    }
                    seen.push(&field.name);
                    report.absorb(field.ty.validate(&registry, &field_path.field("ty")));
                }
                if record.fields.is_empty() {
                    report.push(Diagnostic::at(
                        DiagnosticCode::SectionEmpty,
                        here.field("fields").location(),
                        format!("The record `{}` declares zero fields.", record.id),
                        "Add at least one field to the record.".to_string(),
                    ));
                }
            }
            TypeDef::Enumeration(enumeration) => {
                let mut seen: Vec<&VariantName> = Vec::new();
                for (index, variant) in enumeration.variants.iter().enumerate() {
                    let variant_path = here.field("variants").index(index);
                    report.absorb(variant.validate(&variant_path));
                    if seen.contains(&variant) {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DuplicateName,
                            variant_path.location(),
                            format!(
                                "The enumeration `{}` declares `{}` twice.",
                                enumeration.id, variant
                            ),
                            format!("Rename one of the two `{variant}` alternatives."),
                        ));
                    }
                    seen.push(variant);
                }
                if enumeration.variants.is_empty() {
                    report.push(Diagnostic::at(
                        DiagnosticCode::SectionEmpty,
                        here.field("variants").location(),
                        format!(
                            "The enumeration `{}` declares zero alternatives.",
                            enumeration.id
                        ),
                        "Add at least one alternative to the enumeration.".to_string(),
                    ));
                }
            }
            TypeDef::Sum(sum) => {
                let mut seen: Vec<&VariantName> = Vec::new();
                for (index, variant) in sum.variants.iter().enumerate() {
                    let variant_path = here.field("variants").index(index);
                    report.absorb(variant.name.validate(&variant_path.field("name")));
                    if seen.contains(&&variant.name) {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DuplicateName,
                            variant_path.field("name").location(),
                            format!(
                                "The sum type `{}` declares `{}` twice.",
                                sum.id, variant.name
                            ),
                            format!("Rename one of the two `{}` alternatives.", variant.name),
                        ));
                    }
                    seen.push(&variant.name);
                    if let Some(payload) = &variant.payload {
                        report.absorb(payload.validate(&registry, &variant_path.field("payload")));
                    }
                }
                if sum.variants.is_empty() {
                    report.push(Diagnostic::at(
                        DiagnosticCode::SectionEmpty,
                        here.field("variants").location(),
                        format!("The sum type `{}` declares zero alternatives.", sum.id),
                        "Add at least one alternative to the sum type.".to_string(),
                    ));
                }
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn parcel_types() -> Vec<TypeDef> {
        vec![
            TypeDef::Enumeration(EnumerationDef {
                id: TypeId::new("tenure"),
                variants: vec![VariantName::new("freehold"), VariantName::new("leasehold")],
                summary: "The tenure held over a parcel.".to_string(),
            }),
            TypeDef::Sum(SumDef {
                id: TypeId::new("ruling"),
                variants: vec![
                    SumVariant {
                        name: VariantName::new("approval"),
                        payload: Some(TypeExpr::Text),
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
            TypeDef::Record(RecordDef {
                id: TypeId::new("parcel"),
                fields: vec![
                    FieldDef::new(
                        "area",
                        TypeExpr::Decimal { scale: 4 },
                        "The area in hectares.",
                    ),
                    FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                    FieldDef::new(
                        "tenure",
                        TypeExpr::Enumeration(TypeId::new("tenure")),
                        "The tenure held.",
                    ),
                ],
                summary: "One land parcel.".to_string(),
            }),
        ]
    }

    fn every_primitive() -> Vec<Value> {
        vec![
            Value::Bool(true),
            Value::Integer(-9_000_000_000_000_000_000_000i128),
            Value::Decimal(Decimal::new(1250, 2)),
            Value::Text("Kollam".to_string()),
            Value::Bytes(vec![0, 1, 2, 250]),
            Value::Timestamp(Timestamp::from_millis(1_700_000_000_000)),
            Value::Identity(Identity([7u8; 32])),
            Value::Hash(Hash32::of(b"parcel")),
            Value::list(
                TypeExpr::Integer,
                vec![Value::Integer(1), Value::Integer(2)],
            ),
            Value::map(
                TypeExpr::Text,
                TypeExpr::Integer,
                vec![
                    (Value::Text("b".to_string()), Value::Integer(2)),
                    (Value::Text("a".to_string()), Value::Integer(1)),
                ],
            ),
            Value::some(TypeExpr::Text, Value::Text("present".to_string())),
            Value::none(TypeExpr::Text),
            Value::record(
                TypeId::new("parcel"),
                vec![
                    (
                        FieldName::new("area"),
                        Value::Decimal(Decimal::new(25_000, 4)),
                    ),
                    (
                        FieldName::new("holder"),
                        Value::Identity(Identity([3u8; 32])),
                    ),
                    (
                        FieldName::new("tenure"),
                        Value::enumeration(TypeId::new("tenure"), VariantName::new("freehold")),
                    ),
                ],
            ),
            Value::enumeration(TypeId::new("tenure"), VariantName::new("leasehold")),
            Value::sum(
                TypeId::new("ruling"),
                VariantName::new("approval"),
                Some(Value::Text("The survey matched.".to_string())),
            ),
            Value::sum(TypeId::new("ruling"), VariantName::new("withdrawn"), None),
            Value::reference(
                CollectionId::new("parcel"),
                Value::Text("KL-07-119".to_string()),
            ),
        ]
    }

    #[test]
    fn every_primitive_round_trips_through_scale() {
        for value in every_primitive() {
            let bytes = value.encode();
            let back = Value::decode(&mut &bytes[..]).expect("the encoding decodes");
            assert_eq!(value, back);
        }
    }

    #[test]
    fn every_primitive_round_trips_through_json() {
        for value in every_primitive() {
            let text = serde_json::to_string(&value).expect("the value serialises");
            let back: Value = serde_json::from_str(&text).expect("the text parses");
            assert_eq!(value, back);
        }
    }

    #[test]
    fn every_primitive_infers_its_own_type() {
        let defs = parcel_types();
        let (registry, report) = TypeRegistry::build(&defs, &Path::root().field("types"));
        assert!(report.is_clean());
        for value in every_primitive() {
            let ty = value.infer_type();
            value
                .check(&ty, &registry, &Path::root().field("value"))
                .expect("a value checks against its own inferred type");
        }
    }

    #[test]
    fn the_types_section_validates() {
        let report = validate_types(&parcel_types(), &Path::root().field("types"));
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_repeated_type_name_reports_its_location() {
        let mut defs = parcel_types();
        defs.push(TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("tenure"),
            variants: vec![VariantName::new("freehold")],
            summary: "A repeat.".to_string(),
        }));
        let report = validate_types(&defs, &Path::root().field("types"));
        let found = report
            .find("types[3].id")
            .expect("the diagnostic sits on the repeat");
        assert_eq!(found.code, DiagnosticCode::DuplicateIdentifier);
    }

    #[test]
    fn a_type_naming_an_absent_record_reports_its_location() {
        let mut defs = parcel_types();
        defs.push(TypeDef::Record(RecordDef {
            id: TypeId::new("transfer"),
            fields: vec![FieldDef::new(
                "parcel",
                TypeExpr::Record(TypeId::new("absent_record")),
                "The parcel moved.",
            )],
            summary: "One transfer.".to_string(),
        }));
        let report = validate_types(&defs, &Path::root().field("types"));
        let found = report
            .find("types[3].fields[0].ty")
            .expect("the diagnostic sits on the field type");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("absent_record"));
    }

    #[test]
    fn a_decimal_keeps_every_digit_inside_its_scale() {
        let price = Decimal::new(1_234_567, 4);
        assert_eq!(price.to_string(), "123.4567");
        let doubled = price.add(&price).expect("equal scales add");
        assert_eq!(doubled.units, 2_469_134);
        assert_eq!(doubled.scale, 4);
        let lifted = price.rescale(8).expect("a scale rise stays exact");
        assert_eq!(lifted.units, 12_345_670_000);
        let back = lifted.rescale(4).expect("the exact digits return");
        assert_eq!(back, price);
    }

    #[test]
    fn a_decimal_declines_a_scale_move_that_drops_a_digit() {
        let price = Decimal::new(1_234_567, 4);
        assert_eq!(price.rescale(2), Err(ArithmeticFault::PrecisionLoss));
    }

    #[test]
    fn decimal_multiplication_adds_the_scales() {
        let rate = Decimal::new(125, 3);
        let area = Decimal::new(4_000, 2);
        let product = rate.multiply(&area).expect("the product fits");
        assert_eq!(product.scale, 5);
        assert_eq!(product.units, 500_000);
        assert_eq!(product.to_string(), "5.00000");
    }

    #[test]
    fn decimal_division_stays_exact() {
        let total = Decimal::new(1_000, 2);
        let share = Decimal::new(400, 2);
        let quotient = total.divide(&share).expect("the division is exact");
        assert_eq!(quotient, Decimal::new(250, 2));
        let third = Decimal::new(300, 2);
        assert_eq!(total.divide(&third), Err(ArithmeticFault::PrecisionLoss));
        assert_eq!(
            total.divide(&Decimal::zero(2)),
            Err(ArithmeticFault::DivisionByZero)
        );
    }

    #[test]
    fn decimal_addition_declines_a_scale_mismatch() {
        let left = Decimal::new(100, 2);
        let right = Decimal::new(100, 3);
        assert_eq!(left.add(&right), Err(ArithmeticFault::ScaleMismatch));
    }

    #[test]
    fn a_wrong_scale_reports_a_type_mismatch() {
        let defs = parcel_types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let value = Value::Decimal(Decimal::new(100, 2));
        let outcome = value.check(
            &TypeExpr::Decimal { scale: 4 },
            &registry,
            &Path::root().field("area"),
        );
        let diagnostic = outcome.expect_err("a scale difference raises a diagnostic");
        assert_eq!(diagnostic.code, DiagnosticCode::TypeMismatch);
        assert_eq!(diagnostic.location.as_str(), "area");
    }

    #[test]
    fn a_record_value_missing_a_field_reports_its_location() {
        let defs = parcel_types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let value = Value::record(
            TypeId::new("parcel"),
            vec![(FieldName::new("area"), Value::Decimal(Decimal::new(1, 4)))],
        );
        let outcome = value.check(
            &TypeExpr::Record(TypeId::new("parcel")),
            &registry,
            &Path::root().field("record"),
        );
        let diagnostic = outcome.expect_err("an absent field raises a diagnostic");
        assert_eq!(diagnostic.code, DiagnosticCode::MissingValue);
        assert_eq!(diagnostic.location.as_str(), "record.holder");
    }

    #[test]
    fn canonical_form_sorts_records_and_maps() {
        let mut left = Value::Record(RecordValue {
            type_id: TypeId::new("parcel"),
            fields: vec![
                (
                    FieldName::new("holder"),
                    Value::Identity(Identity([1u8; 32])),
                ),
                (FieldName::new("area"), Value::Decimal(Decimal::new(5, 4))),
            ],
        });
        let mut right = Value::Record(RecordValue {
            type_id: TypeId::new("parcel"),
            fields: vec![
                (FieldName::new("area"), Value::Decimal(Decimal::new(5, 4))),
                (
                    FieldName::new("holder"),
                    Value::Identity(Identity([1u8; 32])),
                ),
            ],
        });
        assert_ne!(left, right);
        left.canonicalise();
        right.canonicalise();
        assert_eq!(left, right);
        assert_eq!(left.canonical_bytes(), right.canonical_bytes());
    }

    #[test]
    fn a_field_path_resolves_through_a_record() {
        let defs = parcel_types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let path = FieldPath::parse("area");
        let found = path
            .resolve(&TypeExpr::Record(TypeId::new("parcel")), &registry)
            .expect("the field resolves");
        assert_eq!(found, TypeExpr::Decimal { scale: 4 });
        assert!(FieldPath::parse("absent")
            .resolve(&TypeExpr::Record(TypeId::new("parcel")), &registry)
            .is_none());
    }

    #[test]
    fn an_identifier_declines_an_upper_case_start() {
        let outcome = TypeId::new("Parcel").validate(&Path::root().field("types").index(0));
        let diagnostic = outcome.expect_err("an upper case start raises a diagnostic");
        assert_eq!(diagnostic.code, DiagnosticCode::InvalidIdentifier);
        assert_eq!(diagnostic.location.as_str(), "types[0]");
        assert!(diagnostic.fix.contains("lower case"));
    }

    #[test]
    fn a_map_declines_an_unordered_key_type() {
        let defs = parcel_types();
        let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let ty = TypeExpr::map(TypeExpr::optional(TypeExpr::Text), TypeExpr::Integer);
        let outcome = ty.validate(&registry, &Path::root().field("ty"));
        assert_eq!(
            outcome
                .expect_err("an optional key raises a diagnostic")
                .code,
            DiagnosticCode::TypeMismatch
        );
    }

    #[test]
    fn a_hash_prints_as_hexadecimal() {
        let hash = Hash32::of(b"cerulea");
        let text = hash.to_hex();
        assert_eq!(text.len(), 64);
        assert!(text
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }
}
