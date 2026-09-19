//! Simulate as role for preview.
//!
//! A person editing the capability matrix needs to see the result before
//! anyone holds the role. Simulation answers that. The module resolves a
//! named role directly against the matrix, without asking the identity
//! module which accounts hold the role.
//!
//! Two previews serve the Studio.
//!
//! 1. **The role preview.** What the role reaches in one collection.
//!    Which capabilities, which fields and which row predicate.
//! 2. **The record preview.** What the role sees of one sample record.
//!    The result carries the record with the fields outside the reach of
//!    the role removed, or the plain sentences stating the refusal.
//!
//! Simulation changes nothing. The Studio runs it over a draft matrix and
//! shows the outcome beside the entry a person edits.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    permissions::FieldQualification,
    types::{FieldName, Value},
};
use pallet_cerulea_schema::access::AccessDenial;

/// What one role sees of one record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordPreview {
    /// The role reaches the record.
    pub visible: bool,
    /// The record with the fields outside the reach of the role removed.
    pub record: Option<Value>,
    /// The refusal, where the role reaches nothing.
    pub refusal: Option<AccessDenial>,
    /// The fields the preview kept, in name order.
    pub kept: Vec<FieldName>,
}

impl RecordPreview {
    /// A preview that reaches the record.
    pub fn seen(record: Value) -> Self {
        let kept = field_names(&record);
        RecordPreview {
            visible: true,
            record: Some(record),
            refusal: None,
            kept,
        }
    }

    /// A preview that reaches nothing.
    pub fn refused(refusal: AccessDenial) -> Self {
        RecordPreview {
            visible: false,
            record: None,
            refusal: Some(refusal),
            kept: Vec::new(),
        }
    }

    /// One plain sentence stating the outcome.
    pub fn statement(&self) -> String {
        match &self.refusal {
            Some(held) => held.line(),
            None => {
                if self.kept.is_empty() {
                    "The role reads the record and reaches no field of it.".to_string()
                } else {
                    let mut held = String::from("The role reads the record and reaches ");
                    for (position, name) in self.kept.iter().enumerate() {
                        if position > 0 {
                            held.push_str(", ");
                        }
                        held.push('`');
                        held.push_str(name.as_str());
                        held.push('`');
                    }
                    held.push('.');
                    held
                }
            }
        }
    }
}

/// The field names one record carries, in the order the record holds
/// them.
pub fn field_names(record: &Value) -> Vec<FieldName> {
    match record {
        Value::Record(held) => held.fields.iter().map(|(name, _)| name.clone()).collect(),
        _ => Vec::new(),
    }
}

/// A short label for one field qualification.
pub fn field_label(fields: &FieldQualification) -> &'static str {
    match fields {
        FieldQualification::EveryField => "every field",
        FieldQualification::Only(_) => "the named fields",
        FieldQualification::Apart(_) => "every field apart from the named ones",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cerulea_blueprint::types::TypeId;

    fn record() -> Value {
        Value::record(
            TypeId::new("parcel"),
            alloc::vec![(
                FieldName::new("survey_number"),
                Value::Text("12/3".into())
            )],
        )
    }

    #[test]
    fn a_preview_of_a_read_names_the_fields_it_kept() {
        let held = RecordPreview::seen(record());
        assert!(held.visible);
        assert_eq!(held.kept, alloc::vec![FieldName::new("survey_number")]);
        assert!(held.statement().contains("survey_number"));
    }

    #[test]
    fn a_preview_of_a_refusal_reads_the_problem_and_the_fix() {
        let held = RecordPreview::refused(AccessDenial::new(
            "The role reaches nothing.",
            "Grant the role a capability.",
        ));
        assert!(!held.visible);
        assert!(held.record.is_none());
        assert!(held.statement().contains("Grant the role"));
    }

    #[test]
    fn a_preview_of_an_empty_projection_states_the_outcome() {
        let held = RecordPreview::seen(Value::record(TypeId::new("parcel"), alloc::vec![]));
        assert!(held.visible);
        assert!(held.statement().contains("no field"));
    }

    #[test]
    fn every_qualification_carries_a_label() {
        assert_eq!(
            field_label(&FieldQualification::EveryField),
            "every field"
        );
        assert_eq!(
            field_label(&FieldQualification::Only(Vec::new())),
            "the named fields"
        );
        assert_eq!(
            field_label(&FieldQualification::Apart(Vec::new())),
            "every field apart from the named ones"
        );
    }
}
