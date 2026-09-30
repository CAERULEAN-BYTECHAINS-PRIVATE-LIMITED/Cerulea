//! Schema validation before every write.
//!
//! Step two of the write path decodes the submitted bytes and checks the
//! result against the registered type the collection names. A record
//! carrying another shape aborts the write, and the whole transaction
//! aborts with it.
//!
//! The check runs five passes in order.
//!
//! 1. The bytes decode into a value.
//! 2. The registry holds the record type the collection names.
//! 3. The value carries that record type, field by field and to any depth.
//! 4. The record holds a value at the declared primary key.
//! 5. The primary key encoding sits inside the key bound.
//!
//! Every pass that finds a problem writes one diagnostic carrying the
//! exact location inside the record and one sentence naming the fix. The
//! dispatch returns an error code. The interface layer calls
//! [`check_record`] to read the diagnostics and show them beside the field
//! a person typed into.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    collections::Collection,
    types::{TypeExpr, TypeRegistry, Value},
    Diagnostic, DiagnosticCode, Path, Report,
};
use codec::Decode;
use pallet_cerulea_schema::{
    primitives::{DeploymentId, MAX_KEY_BYTES},
    registry::SchemaRegistry,
};

use crate::{key_bytes_of, value_at};

fn problem(code: DiagnosticCode, path: &Path, statement: String, fix: String) -> Diagnostic {
    Diagnostic::at(code, path.location(), statement, fix)
}

fn single(diagnostic: Diagnostic) -> Report {
    let mut held = Report::new();
    held.push(diagnostic);
    held
}

/// Decode submitted bytes into a value in canonical form.
///
/// The bytes carry the canonical encoding the Studio produces. A payload
/// that fails to decode reports the fix rather than a codec message.
pub fn decode_record(bytes: &[u8], path: &Path) -> Result<Value, Report> {
    match Value::decode(&mut &bytes[..]) {
        Ok(mut held) => {
            held.canonicalise();
            Ok(held)
        }
        Err(_) => Err(single(problem(
            DiagnosticCode::TypeMismatch,
            path,
            "The payload carries bytes the record encoding declines.".to_string(),
            "Submit the canonical encoding the Studio produces for this collection.".to_string(),
        ))),
    }
}

/// Check one record against the registered schema of its collection.
///
/// The result carries the canonical key bytes the record holds at its
/// declared primary key. A problem at any pass yields the report instead.
pub fn check_record<S: SchemaRegistry>(
    deployment: &DeploymentId,
    definition: &Collection,
    record: &Value,
    path: &Path,
) -> Result<Vec<u8>, Report> {
    let defs = S::type_defs(deployment);
    let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));

    if types.record(&definition.record_type).is_none() {
        return Err(single(problem(
            DiagnosticCode::DanglingReference,
            path,
            format!(
                "The registry lacks the record type `{}` the collection `{}` names.",
                definition.record_type, definition.id
            ),
            "Register the record type before writing records to the collection.".to_string(),
        )));
    }

    let expected = TypeExpr::Record(definition.record_type.clone());
    if let Err(diagnostic) = record.check(&expected, &types, path) {
        return Err(single(diagnostic));
    }

    check_key(definition, record, path, &types)
}

/// Check that a record carries a usable primary key value.
pub fn check_key(
    definition: &Collection,
    record: &Value,
    path: &Path,
    types: &TypeRegistry,
) -> Result<Vec<u8>, Report> {
    let key_path = path.field("primary_key");

    if let Some(declared) = definition.key_type(types) {
        if !declared.is_key_safe() {
            return Err(single(problem(
                DiagnosticCode::TypeMismatch,
                &key_path,
                format!(
                    "The primary key of `{}` carries {}, which sorts differently across nodes.",
                    definition.id,
                    declared.describe()
                ),
                "Point the primary key at a text, integer, timestamp, identity or hash field."
                    .to_string(),
            )));
        }
    }

    if value_at(record, &definition.primary_key).is_none() {
        return Err(single(problem(
            DiagnosticCode::MissingValue,
            &key_path,
            format!(
                "The record holds no value at the primary key `{}`.",
                definition.primary_key
            ),
            format!(
                "Give the record a value at `{}` before submitting it.",
                definition.primary_key
            ),
        )));
    }

    let bytes = match key_bytes_of(definition, record) {
        Some(held) => held,
        None => {
            return Err(single(problem(
                DiagnosticCode::MissingValue,
                &key_path,
                format!(
                    "The record holds no value at the primary key `{}`.",
                    definition.primary_key
                ),
                format!(
                    "Give the record a value at `{}` before submitting it.",
                    definition.primary_key
                ),
            )))
        }
    };

    if bytes.len() > MAX_KEY_BYTES as usize {
        return Err(single(problem(
            DiagnosticCode::ValueTooLong,
            &key_path,
            format!(
                "The primary key value encodes to {} bytes and the Blockchain stores {}.",
                bytes.len(),
                MAX_KEY_BYTES
            ),
            "Point the primary key at a shorter field, or shorten the value.".to_string(),
        )));
    }

    Ok(bytes)
}

/// Decode and check one payload in one pass.
///
/// The write path calls this before it touches storage.
pub fn accept<S: SchemaRegistry>(
    deployment: &DeploymentId,
    definition: &Collection,
    bytes: &[u8],
    path: &Path,
) -> Result<(Value, Vec<u8>), Report> {
    let record = decode_record(bytes, path)?;
    let key = check_record::<S>(deployment, definition, &record, path)?;
    Ok((record, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{new_test_ext, RuntimeOrigin, Schema};
    use cerulea_blueprint::types::{FieldDef, FieldName, RecordDef, TypeDef, TypeId};
    use codec::Encode;
    use frame_support::{assert_ok, BoundedVec};

    const DEPLOYMENT: DeploymentId = [3u8; 32];

    fn tag_type(key: TypeExpr) -> TypeDef {
        TypeDef::Record(RecordDef {
            id: TypeId::new("tag"),
            fields: vec![
                FieldDef::new("code", key, "The code of the tag."),
                FieldDef::new("label", TypeExpr::Text, "The label of the tag."),
            ],
            summary: "One tag.".to_string(),
        })
    }

    fn tag(code: Value) -> Value {
        let mut held = Value::record(
            TypeId::new("tag"),
            vec![
                (FieldName::new("code"), code),
                (FieldName::new("label"), Value::Text("Label".to_string())),
            ],
        );
        held.canonicalise();
        held
    }

    fn tags() -> Collection {
        Collection::new("tag", "tag", "code", "The register of tags.")
    }

    fn at() -> Path {
        Path::root().field("record")
    }

    fn fix_of(report: Report) -> String {
        assert_eq!(report.len(), 1, "{:?}", report.diagnostics);
        report.diagnostics[0].fix.clone()
    }

    fn install(key: TypeExpr) {
        let bytes: BoundedVec<u8, _> =
            BoundedVec::try_from(tag_type(key).encode()).expect("the type fits");
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(1),
            DEPLOYMENT,
            bytes
        ));
    }

    #[test]
    fn a_payload_the_record_encoding_declines_names_the_studio_encoding() {
        let report = decode_record(&[0xff, 0xff, 0xff], &at()).expect_err("the bytes decline");
        assert_eq!(
            fix_of(report),
            "Submit the canonical encoding the Studio produces for this collection."
        );
    }

    #[test]
    fn a_decoded_payload_comes_back_in_canonical_form() {
        let held = tag(Value::Text("T-1".to_string()));
        let decoded = decode_record(&held.canonical_bytes(), &at()).expect("the bytes decode");
        assert_eq!(decoded, held);
    }

    #[test]
    fn a_collection_naming_an_unregistered_type_names_the_registration_fix() {
        new_test_ext().execute_with(|| {
            let report = check_record::<Schema>(
                &DEPLOYMENT,
                &tags(),
                &tag(Value::Text("T-1".to_string())),
                &at(),
            )
            .expect_err("the registry lacks the type");
            assert_eq!(
                fix_of(report),
                "Register the record type before writing records to the collection."
            );
        });
    }

    #[test]
    fn a_record_of_another_shape_reports_the_field_it_breaks() {
        new_test_ext().execute_with(|| {
            install(TypeExpr::Text);
            let report =
                check_record::<Schema>(&DEPLOYMENT, &tags(), &tag(Value::Integer(4)), &at())
                    .expect_err("the code carries a number");
            assert_eq!(report.len(), 1);
            assert!(report.diagnostics[0]
                .location
                .as_str()
                .starts_with("record"));
        });
    }

    #[test]
    fn a_record_of_the_registered_shape_yields_its_key_bytes() {
        new_test_ext().execute_with(|| {
            install(TypeExpr::Text);
            let record = tag(Value::Text("T-1".to_string()));
            let (value, key) =
                accept::<Schema>(&DEPLOYMENT, &tags(), &record.canonical_bytes(), &at())
                    .expect("the record carries the shape");
            assert_eq!(value, record);
            assert_eq!(key, Value::Text("T-1".to_string()).canonical_bytes());
        });
    }

    #[test]
    fn a_key_without_a_total_order_names_the_key_safe_fields() {
        let defs = vec![tag_type(TypeExpr::Optional(Box::new(TypeExpr::Text)))];
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let record = tag(Value::some(TypeExpr::Text, Value::Text("T-1".to_string())));
        let report =
            check_key(&tags(), &record, &at(), &types).expect_err("the key lacks an order");
        assert_eq!(
            fix_of(report),
            "Point the primary key at a text, integer, timestamp, identity or hash field."
        );
    }

    #[test]
    fn a_record_holding_no_key_value_names_the_field_to_fill() {
        let defs = vec![tag_type(TypeExpr::Text)];
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let record = Value::record(
            TypeId::new("tag"),
            vec![(FieldName::new("label"), Value::Text("Label".to_string()))],
        );
        let report = check_key(&tags(), &record, &at(), &types).expect_err("the key is absent");
        assert_eq!(
            fix_of(report),
            "Give the record a value at `code` before submitting it."
        );
    }

    #[test]
    fn a_key_past_the_stored_bound_names_the_shorter_field_fix() {
        let defs = vec![tag_type(TypeExpr::Text)];
        let (types, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let record = tag(Value::Text("k".repeat(MAX_KEY_BYTES as usize + 1)));
        let report = check_key(&tags(), &record, &at(), &types).expect_err("the key is too long");
        assert_eq!(
            fix_of(report),
            "Point the primary key at a shorter field, or shorten the value."
        );
        let fits = tag(Value::Text("k".repeat(200)));
        assert!(check_key(&tags(), &fits, &at(), &types).is_ok());
    }
}
