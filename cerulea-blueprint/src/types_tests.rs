//! Behavioural tests for the type system.
//!
//! Each test builds real types and values, runs the real checks and asserts
//! the concrete result. Every reject path asserts the diagnostic class and
//! the fix sentence the diagnostic carries.

use crate::{
    types::{
        validate_identifier, validate_types, ArithmeticFault, CollectionId, Decimal,
        EnumerationDef, FieldDef, FieldName, FieldPath, Hash32, Identity, MapValue, RecordDef,
        RecordValue, RoleId, SumDef, SumVariant, Timestamp, TypeDef, TypeExpr, TypeId,
        TypeRegistry, Value, VariantName, MAX_DECIMAL_SCALE,
    },
    DiagnosticCode, Path, Report,
};
use alloc::{format, string::ToString, vec, vec::Vec};

fn defs() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("holder"),
            fields: vec![
                FieldDef::new("name", TypeExpr::Text, "The name."),
                FieldDef::new("district", TypeExpr::Text, "The district."),
            ],
            summary: "One holder.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new(
                    "holder",
                    TypeExpr::optional(TypeExpr::Record(TypeId::new("holder"))),
                    "The holder.",
                ),
                FieldDef::new("area", TypeExpr::Decimal { scale: 2 }, "The area."),
                FieldDef::new(
                    "tenure",
                    TypeExpr::Enumeration(TypeId::new("tenure")),
                    "The tenure.",
                ),
            ],
            summary: "One parcel.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("tenure"),
            variants: vec![VariantName::new("freehold"), VariantName::new("leasehold")],
            summary: "The tenure.".to_string(),
        }),
        TypeDef::Sum(SumDef {
            id: TypeId::new("ruling"),
            variants: vec![
                SumVariant {
                    name: VariantName::new("approval"),
                    payload: Some(TypeExpr::Integer),
                },
                SumVariant {
                    name: VariantName::new("withdrawn"),
                    payload: None,
                },
            ],
            summary: "The ruling.".to_string(),
        }),
    ]
}

fn at<'a>(report: &'a Report, location: &str) -> &'a crate::Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn identifiers_accept_lower_case_names_alone() {
    let path = Path::root().field("id");
    assert!(validate_identifier("parcel_holder_2", "role", &path).is_ok());
    let empty = validate_identifier("", "role", &path).expect_err("an empty name fails");
    assert_eq!(empty.code, DiagnosticCode::InvalidIdentifier);
    assert_eq!(empty.problem, "The role name is empty.");
    assert_eq!(empty.fix, "Give the role a name such as `parcel_holder`.");
    let long_name = "a".repeat(65);
    let long = validate_identifier(&long_name, "role", &path).expect_err("a long name fails");
    assert_eq!(long.fix, "Shorten the role name to 64 characters or fewer.");
    let start = validate_identifier("9lives", "role", &path).expect_err("a digit start fails");
    assert_eq!(start.problem, "The role name `9lives` starts with `9`.");
    assert_eq!(start.fix, "Start the role name with a lower case letter.");
    let middle = validate_identifier("land-parcel", "role", &path).expect_err("a hyphen fails");
    assert_eq!(
        middle.problem,
        "The role name `land-parcel` holds the character `-`."
    );
    assert_eq!(
        middle.fix,
        "Write the role name with lower case letters, digits and underscores."
    );
    assert_eq!(RoleId::label(), "role");
    assert_eq!(CollectionId::label(), "collection");
    let from: RoleId = "registrar".into();
    assert_eq!(from, RoleId::new("registrar"));
    assert_eq!(format!("{from}"), "registrar");
    assert!(RoleId::new("Registrar").validate(&path).is_err());
}

#[test]
fn a_field_path_parses_checks_resolves_and_prints() {
    let defs = defs();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let path = FieldPath::parse("holder.district");
    assert_eq!(path.len(), 2);
    assert!(!path.is_empty());
    assert_eq!(format!("{path}"), "holder.district");
    assert!(path.validate(&Path::root()).is_ok());
    let parcel = TypeExpr::Record(TypeId::new("parcel"));
    assert_eq!(path.resolve(&parcel, &registry), Some(TypeExpr::Text));
    assert_eq!(
        FieldPath::parse("area.units").resolve(&parcel, &registry),
        None
    );
    assert_eq!(
        FieldPath::parse("holder").resolve(&TypeExpr::optional(TypeExpr::Text), &registry),
        None
    );
    assert_eq!(
        FieldPath::parse("name").resolve(&TypeExpr::Record(TypeId::new("ghost")), &registry),
        None
    );
    assert_eq!(
        FieldPath::parse("missing").resolve(&parcel, &registry),
        None
    );

    let empty = FieldPath::parse("..");
    assert!(empty.is_empty());
    let fault = empty
        .validate(&Path::root().field("key"))
        .expect_err("an empty path fails");
    assert_eq!(fault.code, DiagnosticCode::MissingValue);
    assert_eq!(fault.fix, "Name at least one field such as `holder`.");
    let bad = FieldPath::parse("holder.District")
        .validate(&Path::root().field("key"))
        .expect_err("a capital fails");
    assert_eq!(bad.location.as_str(), "key[1]");
}

#[test]
fn every_arithmetic_fault_carries_its_class_problem_and_fix() {
    let faults = [
        (
            ArithmeticFault::Overflow,
            DiagnosticCode::ArithmeticOverflow,
            "The arithmetic result runs past the integer range.",
            "Reduce the operand sizes or raise the declared scale.",
        ),
        (
            ArithmeticFault::DivisionByZero,
            DiagnosticCode::DivisionByZero,
            "The divisor is zero.",
            "Guard the division with a check that the divisor holds a value above zero.",
        ),
        (
            ArithmeticFault::PrecisionLoss,
            DiagnosticCode::PrecisionLoss,
            "The exact result needs more digits than the declared scale.",
            "Raise the declared scale of the result field so the exact answer fits.",
        ),
        (
            ArithmeticFault::ScaleMismatch,
            DiagnosticCode::TypeMismatch,
            "The two decimals carry different scales.",
            "Apply the rescale operation to one side so both carry the same scale.",
        ),
        (
            ArithmeticFault::ScaleTooLarge,
            DiagnosticCode::ValueOutOfRange,
            "The decimal scale runs past the permitted maximum.",
            "Choose a scale of 30 or below.",
        ),
    ];
    for (fault, code, problem, fix) in faults {
        assert_eq!(fault.code(), code);
        assert_eq!(fault.problem(), problem);
        assert_eq!(fault.fix(), fix);
        let diagnostic = fault.diagnose(&Path::root().field("sum"));
        assert_eq!(diagnostic.code, code);
        assert_eq!(diagnostic.location.as_str(), "sum");
        assert_eq!(diagnostic.fix, fix);
    }
}

#[test]
fn decimals_stay_exact_and_report_every_fault() {
    assert!(Decimal::zero(3).is_zero());
    assert_eq!(Decimal::zero(3).scale, 3);
    assert!(!Decimal::new(1, 0).is_zero());
    assert_eq!(Decimal::power_of_ten(3), Ok(1000));
    assert_eq!(
        Decimal::power_of_ten(MAX_DECIMAL_SCALE + 1),
        Err(ArithmeticFault::ScaleTooLarge)
    );
    let value = Decimal::new(1250, 2);
    assert_eq!(value.rescale(2), Ok(value));
    assert_eq!(value.rescale(4), Ok(Decimal::new(125_000, 4)));
    assert_eq!(value.rescale(1), Ok(Decimal::new(125, 1)));
    assert_eq!(value.rescale(0), Err(ArithmeticFault::PrecisionLoss));
    assert_eq!(value.rescale(31), Err(ArithmeticFault::ScaleTooLarge));
    assert_eq!(
        Decimal::new(i128::MAX, 0).rescale(5),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(
        Decimal::new(1, 200).multiply(&Decimal::new(1, 100)),
        Err(ArithmeticFault::ScaleTooLarge)
    );
    assert_eq!(
        Decimal::new(i128::MAX, 1).multiply(&Decimal::new(2, 1)),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(
        Decimal::new(i128::MAX, 0).add(&Decimal::new(1, 0)),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(
        Decimal::new(i128::MIN, 0).subtract(&Decimal::new(1, 0)),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(
        Decimal::new(1, 1).divide(&Decimal::new(1, 2)),
        Err(ArithmeticFault::ScaleMismatch)
    );
    assert_eq!(
        Decimal::new(i128::MAX, 2).divide(&Decimal::new(3, 2)),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(
        Decimal::new(1, 1).compare(&Decimal::new(1, 2)),
        Err(ArithmeticFault::ScaleMismatch)
    );
    assert_eq!(
        Decimal::new(5, 1).compare(&Decimal::new(7, 1)),
        Ok(core::cmp::Ordering::Less)
    );
}

#[test]
fn decimals_print_with_the_declared_digits() {
    assert_eq!(format!("{}", Decimal::new(1250, 2)), "12.50");
    assert_eq!(format!("{}", Decimal::new(-5, 2)), "-0.05");
    assert_eq!(format!("{}", Decimal::new(42, 0)), "42");
    assert_eq!(format!("{}", Decimal::new(7, 3)), "0.007");
    assert_eq!(format!("{}", Decimal::new(7, 40)), "7e-40");
}

#[test]
fn timestamps_identities_and_hashes_read_back_exactly() {
    let start = Timestamp::from_millis(1_000);
    assert_eq!(start.millis(), 1_000);
    assert_eq!(start.plus_millis(500), Ok(Timestamp(1_500)));
    assert_eq!(
        Timestamp(u64::MAX).plus_millis(1),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(Timestamp(1_500).since(&start), Ok(500));
    assert_eq!(
        start.since(&Timestamp(1_500)),
        Err(ArithmeticFault::Overflow)
    );
    assert_eq!(Identity([3u8; 32]).as_bytes(), &[3u8; 32]);
    let mut raw = [0u8; 32];
    for (position, byte) in raw.iter_mut().enumerate() {
        *byte = (position as u8) * 8;
    }
    let hash = Hash32(raw);
    assert_eq!(hash.as_bytes(), &raw);
    let hex = hash.to_hex();
    assert_eq!(hex.len(), 64);
    assert!(hex.starts_with("0008101820283038"));
    assert!(hex.ends_with("f0f8"));
    assert_eq!(Hash32::of(b"parcel"), Hash32::of(b"parcel"));
    assert_ne!(Hash32::of(b"parcel"), Hash32::of(b"parcels"));
}

#[test]
fn every_type_describes_itself_and_reports_its_depth_and_order() {
    let cases = [
        (TypeExpr::Bool, "a truth value", true),
        (TypeExpr::Integer, "a whole number", true),
        (TypeExpr::Decimal { scale: 2 }, "a decimal at scale 2", true),
        (TypeExpr::Text, "text", true),
        (TypeExpr::Bytes, "bytes", true),
        (TypeExpr::Timestamp, "a timestamp", true),
        (TypeExpr::Identity, "an identity", true),
        (TypeExpr::Hash, "a hash", true),
        (TypeExpr::list(TypeExpr::Text), "a list of text", true),
        (
            TypeExpr::list(TypeExpr::optional(TypeExpr::Text)),
            "a list of an optional text",
            false,
        ),
        (
            TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
            "a map from text to a whole number",
            false,
        ),
        (
            TypeExpr::optional(TypeExpr::Hash),
            "an optional a hash",
            false,
        ),
        (
            TypeExpr::Record(TypeId::new("parcel")),
            "the record `parcel`",
            true,
        ),
        (
            TypeExpr::Enumeration(TypeId::new("tenure")),
            "the enumeration `tenure`",
            true,
        ),
        (
            TypeExpr::Sum(TypeId::new("ruling")),
            "the sum type `ruling`",
            false,
        ),
        (
            TypeExpr::Reference(CollectionId::new("parcels")),
            "a reference into `parcels`",
            true,
        ),
    ];
    for (ty, words, ordered) in cases {
        assert_eq!(ty.describe(), words);
        assert_eq!(ty.is_key_safe(), ordered, "{words}");
    }
    let nested = TypeExpr::map(
        TypeExpr::Text,
        TypeExpr::list(TypeExpr::optional(TypeExpr::Integer)),
    );
    assert_eq!(nested.depth(), 4);
    let mut found = Vec::new();
    TypeExpr::map(
        TypeExpr::Reference(CollectionId::new("owners")),
        TypeExpr::list(TypeExpr::optional(TypeExpr::Reference(CollectionId::new(
            "parcels",
        )))),
    )
    .collection_refs(&mut found);
    assert_eq!(
        found,
        [CollectionId::new("owners"), CollectionId::new("parcels")]
    );
}

#[test]
fn type_validation_resolves_names_and_bounds_every_shape() {
    let defs = defs();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let path = Path::root().field("ty");
    for good in [
        TypeExpr::Record(TypeId::new("parcel")),
        TypeExpr::Enumeration(TypeId::new("tenure")),
        TypeExpr::Sum(TypeId::new("ruling")),
        TypeExpr::map(TypeExpr::Text, TypeExpr::list(TypeExpr::Integer)),
        TypeExpr::Reference(CollectionId::new("parcels")),
    ] {
        assert!(good.validate(&registry, &path).is_ok());
    }
    let mut deep = TypeExpr::Integer;
    for _ in 0..16 {
        deep = TypeExpr::list(deep);
    }
    let too_deep = deep
        .validate(&registry, &path)
        .expect_err("seventeen levels fail");
    assert_eq!(too_deep.code, DiagnosticCode::ValueOutOfRange);
    assert_eq!(
        too_deep.fix,
        "Give the inner structure a named record type and point at that name."
    );
    let fine = TypeExpr::optional(TypeExpr::Decimal { scale: 31 })
        .validate(&registry, &path)
        .expect_err("scale 31 fails");
    assert_eq!(
        fine.problem,
        "The decimal scale 31 runs past the maximum of 30."
    );
    let unordered = TypeExpr::map(TypeExpr::optional(TypeExpr::Text), TypeExpr::Text)
        .validate(&registry, &path)
        .expect_err("an optional key fails");
    assert_eq!(unordered.code, DiagnosticCode::TypeMismatch);
    let ghost_key = TypeExpr::map(TypeExpr::Record(TypeId::new("ghost")), TypeExpr::Text)
        .validate(&registry, &path)
        .expect_err("the key type is absent");
    assert_eq!(ghost_key.code, DiagnosticCode::DanglingReference);
    let ghost_value = TypeExpr::map(TypeExpr::Text, TypeExpr::Enumeration(TypeId::new("ghost")))
        .validate(&registry, &path)
        .expect_err("the value type is absent");
    assert_eq!(
        ghost_value.fix,
        "Declare an enumeration named `ghost` in the types section."
    );
    let wrong_kind = TypeExpr::Record(TypeId::new("tenure"))
        .validate(&registry, &path)
        .expect_err("tenure is an enumeration");
    assert_eq!(
        wrong_kind.fix,
        "Declare a record type named `tenure` in the types section."
    );
    let sum = TypeExpr::Sum(TypeId::new("parcel"))
        .validate(&registry, &path)
        .expect_err("parcel is a record");
    assert_eq!(
        sum.fix,
        "Declare a sum type named `parcel` in the types section."
    );
}

#[test]
fn the_registry_answers_each_kind_of_lookup() {
    let defs = defs();
    let mut repeated = defs.clone();
    repeated.push(defs[0].clone());
    let (registry, report) = TypeRegistry::build(&repeated, &Path::root().field("types"));
    assert_eq!(registry.len(), 4);
    assert!(!registry.is_empty());
    assert_eq!(report.len(), 1);
    assert_eq!(
        at(&report, "types[4].id").fix,
        "Rename one of the two types away from `holder`."
    );
    let parcel = TypeId::new("parcel");
    let tenure = TypeId::new("tenure");
    let ruling = TypeId::new("ruling");
    assert!(registry.record(&parcel).is_some());
    assert!(registry.record(&tenure).is_none());
    assert!(registry.enumeration(&tenure).is_some());
    assert!(registry.enumeration(&ruling).is_none());
    assert!(registry.sum(&ruling).is_some());
    assert!(registry.sum(&parcel).is_none());
    assert!(registry.get(&TypeId::new("ghost")).is_none());
    let kinds: Vec<(&str, &str)> = defs
        .iter()
        .map(|def| (def.id().as_str(), def.kind()))
        .collect();
    assert_eq!(
        kinds,
        [
            ("holder", "record"),
            ("parcel", "record"),
            ("tenure", "enumeration"),
            ("ruling", "sum type"),
        ]
    );
    let record = registry.record(&parcel).expect("parcel is a record");
    assert_eq!(
        record.field(&FieldName::new("area")).map(|field| &field.ty),
        Some(&TypeExpr::Decimal { scale: 2 })
    );
    let sum = registry.sum(&ruling).expect("ruling is a sum type");
    assert!(sum.variant(&VariantName::new("withdrawn")).is_some());
    assert!(sum.variant(&VariantName::new("appeal")).is_none());
    let empty_defs: Vec<TypeDef> = Vec::new();
    let (empty, _) = TypeRegistry::build(&empty_defs, &Path::root());
    assert!(empty.is_empty());
}

#[test]
fn every_value_infers_its_type() {
    let cases = [
        (Value::Bool(true), TypeExpr::Bool),
        (Value::Integer(1), TypeExpr::Integer),
        (
            Value::Decimal(Decimal::new(1, 3)),
            TypeExpr::Decimal { scale: 3 },
        ),
        (Value::Text("x".to_string()), TypeExpr::Text),
        (Value::Bytes(vec![1]), TypeExpr::Bytes),
        (Value::Timestamp(Timestamp(1)), TypeExpr::Timestamp),
        (Value::Identity(Identity::default()), TypeExpr::Identity),
        (Value::Hash(Hash32::default()), TypeExpr::Hash),
        (
            Value::list(TypeExpr::Text, Vec::new()),
            TypeExpr::list(TypeExpr::Text),
        ),
        (
            Value::map(TypeExpr::Text, TypeExpr::Bool, Vec::new()),
            TypeExpr::map(TypeExpr::Text, TypeExpr::Bool),
        ),
        (
            Value::none(TypeExpr::Hash),
            TypeExpr::optional(TypeExpr::Hash),
        ),
        (
            Value::record(TypeId::new("parcel"), Vec::new()),
            TypeExpr::Record(TypeId::new("parcel")),
        ),
        (
            Value::enumeration(TypeId::new("tenure"), VariantName::new("freehold")),
            TypeExpr::Enumeration(TypeId::new("tenure")),
        ),
        (
            Value::sum(TypeId::new("ruling"), VariantName::new("withdrawn"), None),
            TypeExpr::Sum(TypeId::new("ruling")),
        ),
        (
            Value::reference(CollectionId::new("parcels"), Value::Integer(1)),
            TypeExpr::Reference(CollectionId::new("parcels")),
        ),
    ];
    for (value, ty) in cases {
        assert_eq!(value.infer_type(), ty);
    }
}

fn unsorted_record(district: &str) -> Value {
    Value::Record(RecordValue {
        type_id: TypeId::new("holder"),
        fields: vec![
            (FieldName::new("name"), Value::Text("Asha".to_string())),
            (
                FieldName::new("district"),
                Value::Text(district.to_string()),
            ),
        ],
    })
}

#[test]
fn canonical_form_sorts_every_nested_record_and_map() {
    let map = Value::Map(MapValue {
        key: TypeExpr::Integer,
        value: TypeExpr::Record(TypeId::new("holder")),
        entries: vec![
            (Value::Integer(2), unsorted_record("Kollam")),
            (Value::Integer(1), unsorted_record("Thrissur")),
            (Value::Integer(1), unsorted_record("Thrissur")),
        ],
    });
    let nested = Value::list(
        TypeExpr::Integer,
        vec![
            map.clone(),
            Value::some(TypeExpr::Integer, unsorted_record("Kollam")),
            Value::sum(
                TypeId::new("ruling"),
                VariantName::new("approval"),
                Some(unsorted_record("Kollam")),
            ),
            Value::reference(CollectionId::new("holders"), unsorted_record("Kollam")),
        ],
    );
    let mut sorted = nested.clone();
    sorted.canonicalise();
    assert_ne!(sorted, nested);
    let mut again = sorted.clone();
    again.canonicalise();
    assert_eq!(again, sorted);
    assert_eq!(sorted.canonical_bytes(), nested.canonical_bytes());
    match &sorted {
        Value::List(list) => {
            match &list.items[0] {
                Value::Map(inner) => {
                    assert_eq!(inner.entries.len(), 2);
                    assert_eq!(inner.entries[0].0, Value::Integer(1));
                }
                other => panic!("the map changed shape: {other:?}"),
            }
            match &list.items[1] {
                Value::Optional(inner) => match inner.item.as_deref() {
                    Some(Value::Record(record)) => {
                        assert_eq!(record.fields[0].0, FieldName::new("district"))
                    }
                    other => panic!("the record changed shape: {other:?}"),
                },
                other => panic!("the optional changed shape: {other:?}"),
            }
        }
        other => panic!("the list changed shape: {other:?}"),
    }
}

fn check(value: &Value, ty: &TypeExpr) -> Result<(), crate::Diagnostic> {
    let defs = defs();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    value.check(ty, &registry, &Path::root().field("value"))
}

#[test]
fn a_value_passes_the_check_against_its_declared_type() {
    let holder = Value::record(
        TypeId::new("holder"),
        vec![
            (FieldName::new("name"), Value::Text("Asha".to_string())),
            (
                FieldName::new("district"),
                Value::Text("Kollam".to_string()),
            ),
        ],
    );
    let parcel = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("holder"),
                Value::some(TypeExpr::Record(TypeId::new("holder")), holder),
            ),
            (FieldName::new("area"), Value::Decimal(Decimal::new(5, 2))),
            (
                FieldName::new("tenure"),
                Value::enumeration(TypeId::new("tenure"), VariantName::new("freehold")),
            ),
        ],
    );
    let accepted = [
        (Value::Bool(true), TypeExpr::Bool),
        (Value::Integer(1), TypeExpr::Integer),
        (Value::Text("x".to_string()), TypeExpr::Text),
        (Value::Bytes(Vec::new()), TypeExpr::Bytes),
        (Value::Timestamp(Timestamp(1)), TypeExpr::Timestamp),
        (Value::Identity(Identity::default()), TypeExpr::Identity),
        (Value::Hash(Hash32::default()), TypeExpr::Hash),
        (
            Value::list(TypeExpr::Integer, vec![Value::Integer(1)]),
            TypeExpr::list(TypeExpr::Integer),
        ),
        (
            Value::map(
                TypeExpr::Text,
                TypeExpr::Integer,
                vec![(Value::Text("a".to_string()), Value::Integer(1))],
            ),
            TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
        ),
        (
            Value::none(TypeExpr::Integer),
            TypeExpr::optional(TypeExpr::Integer),
        ),
        (parcel, TypeExpr::Record(TypeId::new("parcel"))),
        (
            Value::sum(
                TypeId::new("ruling"),
                VariantName::new("approval"),
                Some(Value::Integer(4)),
            ),
            TypeExpr::Sum(TypeId::new("ruling")),
        ),
        (
            Value::sum(TypeId::new("ruling"), VariantName::new("withdrawn"), None),
            TypeExpr::Sum(TypeId::new("ruling")),
        ),
        (
            Value::reference(CollectionId::new("parcels"), Value::Integer(1)),
            TypeExpr::Reference(CollectionId::new("parcels")),
        ),
    ];
    for (value, ty) in accepted {
        assert_eq!(check(&value, &ty), Ok(()), "{}", ty.describe());
    }
}

#[test]
fn a_value_of_another_shape_fails_the_check_with_a_fix() {
    let scale = check(
        &Value::Decimal(Decimal::new(1, 3)),
        &TypeExpr::Decimal { scale: 2 },
    )
    .expect_err("the scale differs");
    assert_eq!(
        scale.fix,
        "Apply the rescale operation to move the value to scale 2."
    );
    let plain = check(&Value::Integer(1), &TypeExpr::Text).expect_err("the kind differs");
    assert_eq!(
        plain.problem,
        "The value carries a whole number and the declared type is text."
    );
    assert_eq!(plain.fix, "Supply text here, or change the declared type.");
    let mismatches = [
        (
            Value::list(TypeExpr::Text, Vec::new()),
            TypeExpr::list(TypeExpr::Integer),
        ),
        (
            Value::map(TypeExpr::Text, TypeExpr::Text, Vec::new()),
            TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
        ),
        (
            Value::none(TypeExpr::Text),
            TypeExpr::optional(TypeExpr::Integer),
        ),
        (
            Value::record(TypeId::new("holder"), Vec::new()),
            TypeExpr::Record(TypeId::new("parcel")),
        ),
        (
            Value::enumeration(TypeId::new("other"), VariantName::new("x")),
            TypeExpr::Enumeration(TypeId::new("tenure")),
        ),
        (
            Value::sum(TypeId::new("other"), VariantName::new("x"), None),
            TypeExpr::Sum(TypeId::new("ruling")),
        ),
        (
            Value::reference(CollectionId::new("owners"), Value::Integer(1)),
            TypeExpr::Reference(CollectionId::new("parcels")),
        ),
    ];
    for (value, ty) in mismatches {
        let fault = check(&value, &ty).expect_err("the shape differs");
        assert_eq!(
            fault.code,
            DiagnosticCode::TypeMismatch,
            "{}",
            ty.describe()
        );
    }
    let element = check(
        &Value::list(TypeExpr::Integer, vec![Value::Text("x".to_string())]),
        &TypeExpr::list(TypeExpr::Integer),
    )
    .expect_err("the element differs");
    assert_eq!(element.location.as_str(), "value[0]");
    let map_value = check(
        &Value::map(
            TypeExpr::Text,
            TypeExpr::Integer,
            vec![(Value::Text("a".to_string()), Value::Bool(true))],
        ),
        &TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
    )
    .expect_err("the map value differs");
    assert_eq!(map_value.location.as_str(), "value[0].value");
    let present = check(
        &Value::some(TypeExpr::Integer, Value::Bool(true)),
        &TypeExpr::optional(TypeExpr::Integer),
    )
    .expect_err("the element differs");
    assert_eq!(present.code, DiagnosticCode::TypeMismatch);
}

#[test]
fn record_enumeration_and_sum_values_report_each_gap() {
    let ghost = check(
        &Value::record(TypeId::new("ghost"), Vec::new()),
        &TypeExpr::Record(TypeId::new("ghost")),
    )
    .expect_err("the record type is absent");
    assert_eq!(
        ghost.fix,
        "Declare a record type named `ghost` in the types section."
    );
    let short = check(
        &Value::record(
            TypeId::new("holder"),
            vec![(FieldName::new("name"), Value::Text("Asha".to_string()))],
        ),
        &TypeExpr::Record(TypeId::new("holder")),
    )
    .expect_err("the district is absent");
    assert_eq!(short.code, DiagnosticCode::MissingValue);
    assert_eq!(short.location.as_str(), "value.district");
    assert_eq!(short.fix, "Give the field `district` a value.");
    let extra = check(
        &Value::record(
            TypeId::new("holder"),
            vec![
                (FieldName::new("name"), Value::Text("Asha".to_string())),
                (FieldName::new("district"), Value::Text("K".to_string())),
                (FieldName::new("phone"), Value::Text("1".to_string())),
            ],
        ),
        &TypeExpr::Record(TypeId::new("holder")),
    )
    .expect_err("the phone is undeclared");
    assert_eq!(extra.location.as_str(), "value.phone");
    assert_eq!(
        extra.fix,
        "Remove `phone` or declare it on the record type `holder`."
    );
    let ghost_enumeration = check(
        &Value::enumeration(TypeId::new("ghost"), VariantName::new("x")),
        &TypeExpr::Enumeration(TypeId::new("ghost")),
    )
    .expect_err("the enumeration is absent");
    assert_eq!(
        ghost_enumeration.fix,
        "Declare an enumeration named `ghost` in the types section."
    );
    let alternative = check(
        &Value::enumeration(TypeId::new("tenure"), VariantName::new("rental")),
        &TypeExpr::Enumeration(TypeId::new("tenure")),
    )
    .expect_err("rental is undeclared");
    assert_eq!(
        alternative.fix,
        "Choose an alternative declared on `tenure`."
    );
    let ghost_sum = check(
        &Value::sum(TypeId::new("ghost"), VariantName::new("x"), None),
        &TypeExpr::Sum(TypeId::new("ghost")),
    )
    .expect_err("the sum type is absent");
    assert_eq!(
        ghost_sum.fix,
        "Declare a sum type named `ghost` in the types section."
    );
    let ruling = TypeExpr::Sum(TypeId::new("ruling"));
    let appeal = check(
        &Value::sum(TypeId::new("ruling"), VariantName::new("appeal"), None),
        &ruling,
    )
    .expect_err("appeal is undeclared");
    assert_eq!(appeal.fix, "Choose an alternative declared on `ruling`.");
    let empty = check(
        &Value::sum(TypeId::new("ruling"), VariantName::new("approval"), None),
        &ruling,
    )
    .expect_err("approval carries a number");
    assert_eq!(empty.code, DiagnosticCode::MissingValue);
    assert_eq!(
        empty.problem,
        "The alternative `approval` carries a whole number and the value carries nothing."
    );
    let surplus = check(
        &Value::sum(
            TypeId::new("ruling"),
            VariantName::new("withdrawn"),
            Some(Value::Integer(1)),
        ),
        &ruling,
    )
    .expect_err("withdrawn carries nothing");
    assert_eq!(
        surplus.fix,
        "Remove the data from the alternative `withdrawn`."
    );
    let payload = check(
        &Value::sum(
            TypeId::new("ruling"),
            VariantName::new("approval"),
            Some(Value::Bool(true)),
        ),
        &ruling,
    )
    .expect_err("the payload differs");
    assert_eq!(payload.location.as_str(), "value.payload");
}

#[test]
fn the_types_section_reports_every_malformed_declaration() {
    let broken = vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("Parcel"),
            fields: vec![
                FieldDef::new("Area", TypeExpr::Integer, "A malformed name."),
                FieldDef::new("size", TypeExpr::Integer, "The size."),
                FieldDef::new("size", TypeExpr::Decimal { scale: 40 }, "Again."),
            ],
            summary: "A broken record.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("empty_record"),
            fields: Vec::new(),
            summary: "A record with no fields.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("tenure"),
            variants: vec![
                VariantName::new("Freehold"),
                VariantName::new("lease"),
                VariantName::new("lease"),
            ],
            summary: "A broken enumeration.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("empty_enumeration"),
            variants: Vec::new(),
            summary: "An empty enumeration.".to_string(),
        }),
        TypeDef::Sum(SumDef {
            id: TypeId::new("ruling"),
            variants: vec![
                SumVariant {
                    name: VariantName::new("Approval"),
                    payload: None,
                },
                SumVariant {
                    name: VariantName::new("appeal"),
                    payload: Some(TypeExpr::Record(TypeId::new("ghost"))),
                },
                SumVariant {
                    name: VariantName::new("appeal"),
                    payload: None,
                },
            ],
            summary: "A broken sum type.".to_string(),
        }),
        TypeDef::Sum(SumDef {
            id: TypeId::new("empty_sum"),
            variants: Vec::new(),
            summary: "An empty sum type.".to_string(),
        }),
    ];
    let found = validate_types(&broken, &Path::root().field("types"));
    assert_eq!(
        at(&found, "types[0].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "types[0].fields[0].name").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "types[0].fields[2].name").fix,
        "Rename one of the two `size` fields."
    );
    assert_eq!(
        at(&found, "types[0].fields[2].ty").code,
        DiagnosticCode::ValueOutOfRange
    );
    assert_eq!(
        at(&found, "types[1].fields").fix,
        "Add at least one field to the record."
    );
    assert_eq!(
        at(&found, "types[2].variants[0]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "types[2].variants[2]").fix,
        "Rename one of the two `lease` alternatives."
    );
    assert_eq!(
        at(&found, "types[3].variants").fix,
        "Add at least one alternative to the enumeration."
    );
    assert_eq!(
        at(&found, "types[4].variants[0].name").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "types[4].variants[1].payload").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "types[4].variants[2].name").fix,
        "Rename one of the two `appeal` alternatives."
    );
    assert_eq!(
        at(&found, "types[5].variants").fix,
        "Add at least one alternative to the sum type."
    );
    assert!(validate_types(&defs(), &Path::root()).is_clean());
}
