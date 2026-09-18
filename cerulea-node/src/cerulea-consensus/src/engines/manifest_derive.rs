//! Derivation of the Access Manifest from a Blueprint action.
//!
//! A Blueprint action names the collections it reaches and the key expressions
//! it evaluates. The Execution Engine evaluates those expressions against the
//! transaction parameters at admission time. The result is the Access Manifest
//! the transaction carries into execution.
//!
//! Every node derives the same manifest from the same action and the same
//! parameters. Derivation reads no clock, no random source and no local state.
//!
//! The Blueprint crate carries the action type. That crate implements
//! [`DescribesAccess`] for its action type once the Actions section lands. The
//! consensus crate holds [`ActionDescription`] so the Execution Engine has a
//! settled shape to read in the meantime.

use std::collections::BTreeMap;

use super::manifest::{AccessManifest, CollectionAccess};
use super::{CollectionId, EngineError, KeyRange, TransactionClass};

/// One value bound to a transaction parameter.
///
/// The key encoding of each variant is fixed. Two nodes holding the same value
/// produce the same bytes.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ParameterValue {
    /// Text, encoded as its bytes.
    Text(String),
    /// Raw bytes, carried through unchanged.
    Bytes(Vec<u8>),
    /// A whole number, encoded in sixteen big endian bytes with the sign bit
    /// flipped. Byte order then matches numeric order.
    Integer(i128),
    /// A truth value, encoded as one byte holding zero or one.
    Truth(bool),
    /// An identity, encoded as its thirty two bytes.
    Identity([u8; 32]),
}

impl ParameterValue {
    /// The key bytes this value contributes.
    pub fn key_bytes(&self) -> Vec<u8> {
        match self {
            ParameterValue::Text(text) => text.as_bytes().to_vec(),
            ParameterValue::Bytes(bytes) => bytes.clone(),
            ParameterValue::Integer(number) => {
                let mut raw = number.to_be_bytes();
                raw[0] ^= 0x80;
                raw.to_vec()
            }
            ParameterValue::Truth(truth) => vec![u8::from(*truth)],
            ParameterValue::Identity(identity) => identity.to_vec(),
        }
    }
}

/// The parameters one transaction carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TransactionParameters {
    values: BTreeMap<String, ParameterValue>,
}

impl TransactionParameters {
    /// Start an empty parameter set.
    pub fn new() -> Self {
        TransactionParameters {
            values: BTreeMap::new(),
        }
    }

    /// Bind one parameter to one value.
    pub fn bind(mut self, name: &str, value: ParameterValue) -> Self {
        self.values.insert(name.to_string(), value);
        self
    }

    /// Bind one parameter to one value in place.
    pub fn set(&mut self, name: &str, value: ParameterValue) {
        self.values.insert(name.to_string(), value);
    }

    /// Read one parameter.
    pub fn get(&self, name: &str) -> Option<&ParameterValue> {
        self.values.get(name)
    }

    /// The number of bound parameters.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Report whether the set binds zero parameters.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// One part of a key expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyPart {
    /// Fixed bytes written into the Blueprint action.
    Literal(Vec<u8>),
    /// The value of one named transaction parameter.
    Parameter(String),
}

/// A key expression taken from a Blueprint action.
///
/// The expression evaluates to one key span inside one collection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyExpression {
    /// One key, built by joining the parts in order.
    Exact(Vec<KeyPart>),
    /// Every key starting with the bytes the parts build.
    Prefix(Vec<KeyPart>),
    /// Every key of the collection.
    Whole,
}

impl KeyExpression {
    /// An expression naming one literal key.
    pub fn literal(key: &[u8]) -> Self {
        KeyExpression::Exact(vec![KeyPart::Literal(key.to_vec())])
    }

    /// An expression naming the value of one parameter.
    pub fn parameter(name: &str) -> Self {
        KeyExpression::Exact(vec![KeyPart::Parameter(name.to_string())])
    }

    /// Evaluate the expression against the transaction parameters.
    pub fn evaluate(
        &self,
        parameters: &TransactionParameters,
        action: &str,
        collection: &str,
    ) -> Result<KeyRange, EngineError> {
        match self {
            KeyExpression::Whole => Ok(KeyRange::Full),
            KeyExpression::Exact(parts) => {
                let bytes = join_parts(parts, parameters, action, collection)?;
                Ok(KeyRange::Exact(bytes))
            }
            KeyExpression::Prefix(parts) => {
                let bytes = join_parts(parts, parameters, action, collection)?;
                Ok(KeyRange::Prefix(bytes))
            }
        }
    }
}

fn join_parts(
    parts: &[KeyPart],
    parameters: &TransactionParameters,
    action: &str,
    collection: &str,
) -> Result<Vec<u8>, EngineError> {
    if parts.is_empty() {
        return Err(EngineError::KeyExpressionEmpty {
            action: action.to_string(),
            collection: collection.to_string(),
        });
    }
    let mut bytes = Vec::new();
    for part in parts {
        match part {
            KeyPart::Literal(literal) => bytes.extend_from_slice(literal),
            KeyPart::Parameter(name) => {
                let value = parameters
                    .get(name)
                    .ok_or_else(|| EngineError::ParameterAbsent {
                        parameter: name.clone(),
                        action: action.to_string(),
                    })?;
                bytes.extend_from_slice(&value.key_bytes());
            }
        }
    }
    Ok(bytes)
}

/// One collection an action reaches, with the key expression it evaluates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessSpec {
    /// The name of the collection the Blueprint action carries.
    pub collection: String,
    /// The key expression the action evaluates inside that collection.
    pub keys: KeyExpression,
}

impl AccessSpec {
    /// Describe one access.
    pub fn new(collection: &str, keys: KeyExpression) -> Self {
        AccessSpec {
            collection: collection.to_string(),
            keys,
        }
    }
}

/// The state access one Blueprint action declares.
///
/// The Blueprint crate produces this description from its Actions section. The
/// Execution Engine reads the description at admission time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionDescription {
    /// The name the Blueprint gave the action.
    pub name: String,
    /// The class the action carries.
    pub class: TransactionClass,
    /// The collections and key expressions the action reads.
    pub reads: Vec<AccessSpec>,
    /// The collections and key expressions the action writes.
    pub writes: Vec<AccessSpec>,
}

impl ActionDescription {
    /// Describe one action.
    pub fn new(
        name: &str,
        class: TransactionClass,
        reads: Vec<AccessSpec>,
        writes: Vec<AccessSpec>,
    ) -> Self {
        ActionDescription {
            name: name.to_string(),
            class,
            reads,
            writes,
        }
    }
}

/// A Blueprint action that describes its state access.
///
/// The Blueprint crate implements this trait for its action type. The
/// Execution Engine then derives the manifest from any Blueprint action without
/// knowing the Blueprint type set.
pub trait DescribesAccess {
    /// The access description of this action.
    fn action_description(&self) -> ActionDescription;
}

impl DescribesAccess for ActionDescription {
    fn action_description(&self) -> ActionDescription {
        self.clone()
    }
}

/// Derive the Access Manifest of one transaction.
///
/// The derivation evaluates every key expression the action carries against the
/// transaction parameters. The result names the exact state the transaction
/// reaches. Two nodes holding the same action and the same parameters derive
/// the same manifest.
pub fn derive_manifest(
    action: &ActionDescription,
    parameters: &TransactionParameters,
) -> Result<AccessManifest, EngineError> {
    let mut manifest = AccessManifest::new(action.class);
    for spec in &action.reads {
        manifest.declare_read(evaluate_spec(spec, parameters, &action.name)?)?;
    }
    for spec in &action.writes {
        manifest.declare_write(evaluate_spec(spec, parameters, &action.name)?)?;
    }
    Ok(manifest)
}

/// Derive the Access Manifest from any type that describes its access.
pub fn derive_manifest_from<A: DescribesAccess>(
    action: &A,
    parameters: &TransactionParameters,
) -> Result<AccessManifest, EngineError> {
    derive_manifest(&action.action_description(), parameters)
}

fn evaluate_spec(
    spec: &AccessSpec,
    parameters: &TransactionParameters,
    action: &str,
) -> Result<CollectionAccess, EngineError> {
    let keys = spec.keys.evaluate(parameters, action, &spec.collection)?;
    Ok(CollectionAccess::new(
        CollectionId::from_name(&spec.collection),
        keys,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transfer_action() -> ActionDescription {
        ActionDescription::new(
            "transfer parcel",
            TransactionClass::Financial,
            vec![AccessSpec::new(
                "holder",
                KeyExpression::parameter("from_holder"),
            )],
            vec![
                AccessSpec::new("parcel", KeyExpression::parameter("parcel_id")),
                AccessSpec::new(
                    "ledger",
                    KeyExpression::Exact(vec![
                        KeyPart::Literal(b"entry-".to_vec()),
                        KeyPart::Parameter("parcel_id".to_string()),
                    ]),
                ),
            ],
        )
    }

    fn transfer_parameters() -> TransactionParameters {
        TransactionParameters::new()
            .bind("parcel_id", ParameterValue::Text("parcel-1".to_string()))
            .bind("from_holder", ParameterValue::Identity([9u8; 32]))
    }

    #[test]
    fn derivation_reads_the_collections_the_action_names() {
        let manifest =
            derive_manifest(&transfer_action(), &transfer_parameters()).expect("derivation lands");
        assert_eq!(manifest.class, TransactionClass::Financial);
        assert_eq!(manifest.reads.len(), 1);
        assert_eq!(manifest.writes.len(), 2);
        assert!(manifest.permits_write(&CollectionId::from_name("parcel"), b"parcel-1"));
        assert!(manifest.permits_read(&CollectionId::from_name("holder"), &[9u8; 32]));
    }

    #[test]
    fn a_literal_prefix_joins_with_a_parameter_value() {
        let manifest =
            derive_manifest(&transfer_action(), &transfer_parameters()).expect("derivation lands");
        assert!(manifest.permits_write(&CollectionId::from_name("ledger"), b"entry-parcel-1"));
        assert!(!manifest.permits_write(&CollectionId::from_name("ledger"), b"entry-parcel-2"));
    }

    #[test]
    fn derivation_repeats_for_every_node() {
        let action = transfer_action();
        let parameters = transfer_parameters();
        let first = derive_manifest(&action, &parameters).expect("derivation lands");
        let second = derive_manifest(&action, &parameters).expect("derivation lands");
        assert_eq!(first, second);
    }

    #[test]
    fn two_parameter_values_produce_two_manifests() {
        let action = transfer_action();
        let first = derive_manifest(&action, &transfer_parameters()).expect("derivation lands");
        let second = derive_manifest(
            &action,
            &TransactionParameters::new()
                .bind("parcel_id", ParameterValue::Text("parcel-2".to_string()))
                .bind("from_holder", ParameterValue::Identity([9u8; 32])),
        )
        .expect("derivation lands");
        assert_ne!(first, second);
        assert!(!first.writes_intersect(&second));
    }

    #[test]
    fn an_absent_parameter_names_the_fix() {
        let error = derive_manifest(&transfer_action(), &TransactionParameters::new())
            .expect_err("the derivation reports the absent parameter");
        match &error {
            EngineError::ParameterAbsent { parameter, action } => {
                assert_eq!(parameter, "from_holder");
                assert_eq!(action, "transfer parcel");
            }
            other => panic!("the derivation names the absent parameter: {:?}", other),
        }
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn an_empty_key_expression_names_the_fix() {
        let action = ActionDescription::new(
            "record scan",
            TransactionClass::Routine,
            vec![],
            vec![AccessSpec::new("scan", KeyExpression::Exact(vec![]))],
        );
        let error = derive_manifest(&action, &TransactionParameters::new())
            .expect_err("an empty expression reports a problem");
        match &error {
            EngineError::KeyExpressionEmpty { action, collection } => {
                assert_eq!(action, "record scan");
                assert_eq!(collection, "scan");
            }
            other => panic!("the derivation names the empty expression: {:?}", other),
        }
        assert!(error.to_string().contains("Fix:"));
    }

    #[test]
    fn a_whole_collection_expression_covers_every_key() {
        let action = ActionDescription::new(
            "rebuild index",
            TransactionClass::Structural,
            vec![],
            vec![AccessSpec::new("parcel", KeyExpression::Whole)],
        );
        let manifest =
            derive_manifest(&action, &TransactionParameters::new()).expect("derivation lands");
        assert!(manifest.permits_write(&CollectionId::from_name("parcel"), b"any key at all"));
    }

    #[test]
    fn a_prefix_expression_covers_the_keys_beneath_it() {
        let action = ActionDescription::new(
            "close district",
            TransactionClass::Structural,
            vec![],
            vec![AccessSpec::new(
                "parcel",
                KeyExpression::Prefix(vec![
                    KeyPart::Literal(b"district-".to_vec()),
                    KeyPart::Parameter("district".to_string()),
                ]),
            )],
        );
        let parameters =
            TransactionParameters::new().bind("district", ParameterValue::Text("north".into()));
        let manifest = derive_manifest(&action, &parameters).expect("derivation lands");
        let parcel = CollectionId::from_name("parcel");
        assert!(manifest.permits_write(&parcel, b"district-north-1"));
        assert!(!manifest.permits_write(&parcel, b"district-south-1"));
    }

    #[test]
    fn every_parameter_value_carries_a_settled_encoding() {
        assert_eq!(
            ParameterValue::Text("abc".into()).key_bytes(),
            b"abc".to_vec()
        );
        assert_eq!(
            ParameterValue::Bytes(vec![1, 2, 3]).key_bytes(),
            vec![1, 2, 3]
        );
        assert_eq!(ParameterValue::Truth(true).key_bytes(), vec![1]);
        assert_eq!(ParameterValue::Truth(false).key_bytes(), vec![0]);
        assert_eq!(ParameterValue::Identity([4u8; 32]).key_bytes().len(), 32);
        let low = ParameterValue::Integer(-5).key_bytes();
        let high = ParameterValue::Integer(5).key_bytes();
        assert_eq!(low.len(), 16);
        assert!(low < high, "the byte order matches the numeric order");
    }

    #[test]
    fn a_describing_type_derives_through_the_trait() {
        let action = transfer_action();
        let through_trait =
            derive_manifest_from(&action, &transfer_parameters()).expect("derivation lands");
        let direct = derive_manifest(&action, &transfer_parameters()).expect("derivation lands");
        assert_eq!(through_trait, direct);
    }

    #[test]
    fn a_parameter_set_reads_back_its_bindings() {
        let parameters = transfer_parameters();
        assert_eq!(parameters.len(), 2);
        assert!(!parameters.is_empty());
        assert_eq!(
            parameters.get("parcel_id"),
            Some(&ParameterValue::Text("parcel-1".to_string()))
        );
        assert_eq!(parameters.get("absent"), None);
        let mut mutable = TransactionParameters::new();
        mutable.set("one", ParameterValue::Integer(1));
        assert_eq!(mutable.len(), 1);
    }
}
