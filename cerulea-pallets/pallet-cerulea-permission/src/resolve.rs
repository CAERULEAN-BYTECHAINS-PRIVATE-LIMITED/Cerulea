//! Field level and row level resolution.
//!
//! A capability entry answers two further questions after it admits an
//! operation. Which fields does the role read, and which rows does the
//! role reach.
//!
//! **Fields.** An entry names every field, a list of fields, or every
//! field apart from a list. A read returns the record with the fields
//! outside the reach of the role removed. A nested record keeps the
//! fields the entry names inside it, so a role that reads one field of a
//! nested record reads that field alone.
//!
//! **Rows.** An entry carries an optional predicate over the record. The
//! predicate reads the record under the name `record` and the caller
//! under the built in caller value. A record the predicate declines stays
//! out of the result.
//!
//! Both passes are pure, so the Studio runs the same passes over a draft
//! matrix and shows a person what a role would see.

use alloc::{vec::Vec};
use cerulea_blueprint::{
    expression::{Env, Expr, Meter},
    permissions::FieldQualification,
    types::{FieldName, FieldPath, Value},
    Path,
};

/// Remove from one record every field outside the reach of one
/// qualification.
///
/// A value other than a record passes through unchanged, since a
/// qualification names fields.
pub fn project(record: &Value, fields: &FieldQualification) -> Value {
    if matches!(fields, FieldQualification::EveryField) {
        return record.clone();
    }
    let mut prefix: Vec<FieldName> = Vec::new();
    project_at(record, fields, &mut prefix)
}

fn project_at(value: &Value, fields: &FieldQualification, prefix: &mut Vec<FieldName>) -> Value {
    match value {
        Value::Record(held) => {
            let mut kept: Vec<(FieldName, Value)> = Vec::new();
            for (name, field) in held.fields.iter() {
                prefix.push(name.clone());
                let path = FieldPath(prefix.clone());
                if fields.reaches(&path) {
                    kept.push((name.clone(), field.clone()));
                } else if reaches_deeper(fields, &path) {
                    kept.push((name.clone(), project_at(field, fields, prefix)));
                }
                prefix.pop();
            }
            Value::record(held.type_id.clone(), kept)
        }
        other => other.clone(),
    }
}

/// Report whether one qualification names a field inside the given path.
pub fn reaches_deeper(fields: &FieldQualification, prefix: &FieldPath) -> bool {
    fields.listed().iter().any(|held| {
        held.0.len() > prefix.0.len() && held.0.starts_with(prefix.0.as_slice())
    })
}

/// Report whether one row predicate admits one record.
///
/// An entry carrying no predicate admits every record. A predicate that
/// produces anything other than a truth value declines the record, which
/// keeps a mistaken predicate closed rather than open.
pub fn admits_row(
    predicate: Option<&Expr>,
    record: &Value,
    env: &Env,
    meter: &mut Meter,
    binding: &cerulea_blueprint::types::ParamName,
) -> bool {
    match predicate {
        None => true,
        Some(held) => {
            let bound = env.with(binding, record.clone());
            matches!(
                held.evaluate(&bound, meter, &Path::root().field("rows")),
                Ok(Value::Bool(true))
            )
        }
    }
}

/// Keep the records one row predicate admits, in the order they arrived.
pub fn admitted_rows(
    predicate: Option<&Expr>,
    records: &[Value],
    env: &Env,
    meter: &mut Meter,
    binding: &cerulea_blueprint::types::ParamName,
) -> Vec<Value> {
    records
        .iter()
        .filter(|record| admits_row(predicate, record, env, meter, binding))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cerulea_blueprint::types::TypeId;

    fn record() -> Value {
        Value::record(
            TypeId::new("parcel"),
            alloc::vec![
                (
                    FieldName::new("survey_number"),
                    Value::Text("12/3".into())
                ),
                (FieldName::new("holder"), Value::Text("Anitha".into())),
                (FieldName::new("ward"), Value::Text("north".into())),
            ],
        )
    }

    fn nested() -> Value {
        Value::record(
            TypeId::new("parcel"),
            alloc::vec![
                (
                    FieldName::new("survey_number"),
                    Value::Text("12/3".into())
                ),
                (
                    FieldName::new("owner"),
                    Value::record(
                        TypeId::new("person"),
                        alloc::vec![
                            (FieldName::new("name"), Value::Text("Anitha".into())),
                            (
                                FieldName::new("licence"),
                                Value::Text("KL-99".into())
                            ),
                        ],
                    )
                ),
            ],
        )
    }

    fn names(value: &Value) -> Vec<alloc::string::String> {
        match value {
            Value::Record(held) => held
                .fields
                .iter()
                .map(|(name, _)| alloc::string::String::from(name.as_str()))
                .collect(),
            _ => Vec::new(),
        }
    }

    #[test]
    fn every_field_passes_the_record_through() {
        let held = project(&record(), &FieldQualification::EveryField);
        assert_eq!(held, record());
    }

    #[test]
    fn a_named_list_keeps_the_named_fields_alone() {
        let held = project(
            &record(),
            &FieldQualification::Only(alloc::vec![FieldPath::parse("holder")]),
        );
        assert_eq!(names(&held), alloc::vec![alloc::string::String::from("holder")]);
    }

    #[test]
    fn a_held_out_list_removes_the_named_fields() {
        let held = project(
            &record(),
            &FieldQualification::Apart(alloc::vec![FieldPath::parse("holder")]),
        );
        let found = names(&held);
        assert!(found.contains(&alloc::string::String::from("survey_number")));
        assert!(found.contains(&alloc::string::String::from("ward")));
        assert!(!found.contains(&alloc::string::String::from("holder")));
    }

    #[test]
    fn a_nested_path_keeps_the_field_it_names() {
        let held = project(
            &nested(),
            &FieldQualification::Only(alloc::vec![FieldPath::parse("owner.name")]),
        );
        match &held {
            Value::Record(outer) => {
                assert_eq!(outer.fields.len(), 1);
                match outer.field(&FieldName::new("owner")) {
                    Some(Value::Record(inner)) => {
                        assert_eq!(inner.fields.len(), 1);
                        assert!(inner.field(&FieldName::new("name")).is_some());
                    }
                    other => unreachable!("the projection produced {:?}", other),
                }
            }
            other => unreachable!("the projection produced {:?}", other),
        }
    }

    #[test]
    fn a_value_that_is_not_a_record_passes_through() {
        let held = project(
            &Value::Integer(7),
            &FieldQualification::Only(alloc::vec![FieldPath::parse("holder")]),
        );
        assert_eq!(held, Value::Integer(7));
    }

    #[test]
    fn a_deeper_field_is_seen_from_its_prefix() {
        let fields = FieldQualification::Only(alloc::vec![FieldPath::parse("owner.name")]);
        assert!(reaches_deeper(&fields, &FieldPath::parse("owner")));
        assert!(!reaches_deeper(&fields, &FieldPath::parse("survey_number")));
    }
}
