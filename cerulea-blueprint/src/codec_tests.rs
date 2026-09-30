//! Behavioural tests for the encodings of the Blueprint type set.
//!
//! The Blueprint travels in two encodings. SCALE carries the canonical form
//! the Blockchain hashes. JSON carries the form the interface layer reads.
//! Every alternative of every enumeration in the type set round trips
//! through both encodings here. The type metadata registry reads the whole
//! type set, and a byte string naming an alternative the type set lacks
//! fails to decode.

use crate::{
    actions::{AccessEntry, AccessManifest, AccessMode, Effect, TransactionClass},
    address::BlueprintId,
    collections::{KeyRange, KeySelector, PrivacyClass, RetentionAction, RetentionPeriod},
    events::{AuditPolicy, DeliveryTarget},
    expression::{BinaryOp, Builtin, Expr, Pattern, UnaryOp},
    identities::{CredentialIssuer, CredentialValidity},
    integrations::BridgeDirection,
    interface::{
        Binding, BindingNeed, ComponentGroup, ComponentKind, ConflictResolution, ScreenKind,
        VisibilityRule,
    },
    permissions::{Capability, FieldQualification, RowQualification},
    types::{
        ActionId, BlockId, CollectionId, Decimal, EnumerationDef, EventId, FieldName, FieldPath,
        Hash32, Identity, IntegrationId, ParamName, RecordDef, RoleId, StateId, SumDef, Timestamp,
        TypeDef, TypeExpr, TypeId, Value, VariantName, ViewId, WorkflowId,
    },
    views::{AggregateFunction, JoinKind, SortDirection},
    workflows::{CyclePolicy, TimerAction},
    Blueprint, Diagnostic, DiagnosticCode, Report, SchemaVersion,
};
use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use codec::{Decode, Encode};
use core::fmt::Debug;
use scale_info::{meta_type, PortableRegistry, Registry};
use serde::{de::DeserializeOwned, Serialize};

fn round_trip<T>(value: &T)
where
    T: Encode + Decode + Serialize + DeserializeOwned + PartialEq + Debug,
{
    let bytes = value.encode();
    let decoded = T::decode(&mut &bytes[..]).expect("the SCALE bytes decode");
    assert_eq!(&decoded, value);
    let text = serde_json::to_string(value).expect("the value serialises");
    let back: T = serde_json::from_str(&text).expect("the JSON text parses");
    assert_eq!(&back, value);
}

fn round_trip_all<T>(values: &[T])
where
    T: Encode + Decode + Serialize + DeserializeOwned + PartialEq + Debug,
{
    for value in values {
        round_trip(value);
    }
    let mut encodings: Vec<Vec<u8>> = values.iter().map(|value| value.encode()).collect();
    encodings.sort();
    encodings.dedup();
    assert_eq!(encodings.len(), values.len());
}

fn rejects_unknown_alternative<T: Decode + Debug>() {
    let bytes = [0xffu8, 0, 0, 0, 0];
    assert!(T::decode(&mut &bytes[..]).is_err());
}

#[test]
fn the_type_metadata_registry_reads_the_whole_type_set() {
    let mut registry = Registry::new();
    registry.register_type(&meta_type::<Blueprint>());
    registry.register_type(&meta_type::<Report>());
    registry.register_type(&meta_type::<AccessManifest>());
    registry.register_type(&meta_type::<BlueprintId>());
    registry.register_type(&meta_type::<KeyRange>());
    registry.register_type(&meta_type::<AccessMode>());
    let portable = PortableRegistry::from(registry);
    let names: BTreeSet<String> = portable
        .types
        .iter()
        .filter_map(|entry| entry.ty.path.segments.last().cloned())
        .collect();
    for name in [
        "Blueprint",
        "BlueprintMeta",
        "VersionStamp",
        "SchemaVersion",
        "TypeDef",
        "TypeExpr",
        "Value",
        "Decimal",
        "Expr",
        "FoldExpr",
        "Collection",
        "PrivacyClass",
        "Identities",
        "Permissions",
        "Rules",
        "Action",
        "Effect",
        "Workflow",
        "EventDef",
        "View",
        "Integrations",
        "Governance",
        "Economics",
        "Block",
        "Interface",
        "ComponentKind",
        "Report",
        "Diagnostic",
        "DiagnosticCode",
        "AccessManifest",
        "AccessEntry",
        "BlueprintId",
        "KeyRange",
        "AccessMode",
    ] {
        assert!(names.contains(name), "the registry lacks {name}");
    }
}

#[test]
fn every_diagnostic_class_carries_a_label_and_round_trips() {
    let classes = [
        (DiagnosticCode::DanglingReference, "dangling reference"),
        (DiagnosticCode::DuplicateIdentifier, "duplicate identifier"),
        (DiagnosticCode::InvalidIdentifier, "invalid identifier"),
        (DiagnosticCode::TypeMismatch, "type mismatch"),
        (DiagnosticCode::MissingValue, "missing value"),
        (DiagnosticCode::ValueOutOfRange, "value out of range"),
        (DiagnosticCode::ValueTooLong, "value too long"),
        (DiagnosticCode::CycleDetected, "cycle detected"),
        (DiagnosticCode::BoundExceeded, "bound exceeded"),
        (DiagnosticCode::ArithmeticOverflow, "arithmetic overflow"),
        (DiagnosticCode::DivisionByZero, "division by zero"),
        (DiagnosticCode::PrecisionLoss, "precision loss"),
        (DiagnosticCode::DuplicateName, "duplicate name"),
        (
            DiagnosticCode::SchemaVersionMismatch,
            "schema version mismatch",
        ),
        (DiagnosticCode::ThresholdTooLow, "threshold too low"),
        (DiagnosticCode::SectionEmpty, "section empty"),
    ];
    for (class, label) in classes {
        assert_eq!(class.label(), label);
    }
    let codes: Vec<DiagnosticCode> = classes.iter().map(|(class, _)| *class).collect();
    round_trip_all(&codes);
    rejects_unknown_alternative::<DiagnosticCode>();
    let diagnostic = Diagnostic::new(
        DiagnosticCode::SectionEmpty,
        crate::Path::root().field("types"),
        "The section is empty.",
        "Add a type.",
    );
    round_trip(&diagnostic);
    round_trip(&Report {
        diagnostics: vec![diagnostic],
    });
    round_trip(&SchemaVersion {
        major: 1,
        minor: 2,
        patch: 3,
    });
}

#[test]
fn every_operator_and_builtin_round_trips() {
    round_trip_all(&[UnaryOp::Not, UnaryOp::Negate]);
    round_trip_all(&[
        BinaryOp::Add,
        BinaryOp::Subtract,
        BinaryOp::Multiply,
        BinaryOp::Divide,
        BinaryOp::Remainder,
        BinaryOp::Equal,
        BinaryOp::NotEqual,
        BinaryOp::Less,
        BinaryOp::LessOrEqual,
        BinaryOp::Greater,
        BinaryOp::GreaterOrEqual,
        BinaryOp::And,
        BinaryOp::Or,
        BinaryOp::ConcatText,
        BinaryOp::AppendList,
    ]);
    round_trip_all(&[
        Builtin::Now,
        Builtin::Caller,
        Builtin::HashOf,
        Builtin::TimestampPlus,
        Builtin::TimestampSince,
        Builtin::AbsoluteInteger,
        Builtin::MinimumInteger,
        Builtin::MaximumInteger,
        Builtin::BytesOfText,
    ]);
    round_trip_all(&[
        Pattern::Variant {
            variant: VariantName::new("approval"),
            binding: Some(ParamName::new("amount")),
        },
        Pattern::Choice {
            variant: VariantName::new("freehold"),
        },
        Pattern::Present {
            binding: ParamName::new("inner"),
        },
        Pattern::Absent,
    ]);
    rejects_unknown_alternative::<UnaryOp>();
    rejects_unknown_alternative::<BinaryOp>();
    rejects_unknown_alternative::<Builtin>();
    rejects_unknown_alternative::<Pattern>();
    rejects_unknown_alternative::<Expr>();
}

fn every_type() -> Vec<TypeExpr> {
    vec![
        TypeExpr::Bool,
        TypeExpr::Integer,
        TypeExpr::Decimal { scale: 2 },
        TypeExpr::Text,
        TypeExpr::Bytes,
        TypeExpr::Timestamp,
        TypeExpr::Identity,
        TypeExpr::Hash,
        TypeExpr::list(TypeExpr::Text),
        TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
        TypeExpr::optional(TypeExpr::Hash),
        TypeExpr::Record(TypeId::new("parcel")),
        TypeExpr::Enumeration(TypeId::new("tenure")),
        TypeExpr::Sum(TypeId::new("ruling")),
        TypeExpr::Reference(CollectionId::new("parcels")),
    ]
}

#[test]
fn every_type_and_every_value_round_trips() {
    round_trip_all(&every_type());
    let values = vec![
        Value::Bool(true),
        Value::Integer(-7),
        Value::Decimal(Decimal::new(125, 2)),
        Value::Text("parcel".to_string()),
        Value::Bytes(vec![1, 2, 3]),
        Value::Timestamp(Timestamp::from_millis(5)),
        Value::Identity(Identity([4u8; 32])),
        Value::Hash(Hash32([5u8; 32])),
        Value::list(TypeExpr::Integer, vec![Value::Integer(1)]),
        Value::map(
            TypeExpr::Integer,
            TypeExpr::Text,
            vec![(Value::Integer(1), Value::Text("a".to_string()))],
        ),
        Value::some(TypeExpr::Integer, Value::Integer(2)),
        Value::record(
            TypeId::new("parcel"),
            vec![(FieldName::new("area"), Value::Integer(3))],
        ),
        Value::enumeration(TypeId::new("tenure"), VariantName::new("freehold")),
        Value::sum(TypeId::new("ruling"), VariantName::new("withdrawn"), None),
        Value::reference(CollectionId::new("parcels"), Value::Integer(9)),
    ];
    round_trip_all(&values);
    round_trip_all(&[
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: Vec::new(),
            summary: "One parcel.".to_string(),
        }),
        TypeDef::Enumeration(EnumerationDef {
            id: TypeId::new("tenure"),
            variants: vec![VariantName::new("freehold")],
            summary: "The tenure.".to_string(),
        }),
        TypeDef::Sum(SumDef {
            id: TypeId::new("ruling"),
            variants: Vec::new(),
            summary: "The ruling.".to_string(),
        }),
    ]);
    rejects_unknown_alternative::<TypeExpr>();
    rejects_unknown_alternative::<Value>();
    rejects_unknown_alternative::<TypeDef>();
}

#[test]
fn every_collection_and_effect_alternative_round_trips() {
    round_trip_all(&[
        PrivacyClass::Open,
        PrivacyClass::HashOnly,
        PrivacyClass::Encrypted {
            key_holders: vec![RoleId::new("registrar")],
        },
        PrivacyClass::External {
            commitment: FieldPath::parse("digest"),
            locator: FieldPath::parse("address"),
        },
    ]);
    round_trip_all(&[
        RetentionPeriod::Indefinite,
        RetentionPeriod::Milliseconds(1_000),
    ]);
    round_trip_all(&[
        RetentionAction::Review,
        RetentionAction::Erase,
        RetentionAction::Anonymise(vec![FieldPath::parse("holder")]),
        RetentionAction::MoveTo(CollectionId::new("archive")),
    ]);
    round_trip_all(&[
        KeySelector::Exact(Expr::integer(1)),
        KeySelector::Prefix(Expr::text("KL")),
        KeySelector::Full,
    ]);
    round_trip_all(&[
        KeyRange::Exact(vec![1]),
        KeyRange::Prefix(vec![2]),
        KeyRange::Full,
    ]);
    round_trip_all(&[
        TransactionClass::Routine,
        TransactionClass::Financial,
        TransactionClass::Structural,
        TransactionClass::Constitutional,
    ]);
    round_trip_all(&[AccessMode::Read, AccessMode::Write]);
    round_trip(&AccessEntry {
        collection: CollectionId::new("parcel"),
        range: KeyRange::Full,
    });
    round_trip(&BlueprintId([6u8; 32]));
    let parcel = || CollectionId::new("parcel");
    round_trip_all(&[
        Effect::Insert {
            collection: parcel(),
            key: KeySelector::Full,
            record: Expr::integer(1),
        },
        Effect::Update {
            collection: parcel(),
            key: KeySelector::Full,
            field: FieldPath::parse("area"),
            value: Expr::integer(2),
        },
        Effect::Delete {
            collection: parcel(),
            key: KeySelector::Full,
        },
        Effect::Read {
            collection: parcel(),
            key: KeySelector::Full,
            binding: ParamName::new("found"),
            element: TypeExpr::Record(TypeId::new("parcel")),
        },
        Effect::Emit {
            event: EventId::new("recorded"),
            payload: Expr::integer(3),
        },
        Effect::Transition {
            workflow: WorkflowId::new("parcel_life"),
            collection: parcel(),
            key: KeySelector::Full,
            to: StateId::new("recorded"),
        },
        Effect::RunBlock {
            block: BlockId::new("stamp"),
            arguments: vec![(ParamName::new("note"), Expr::text("x"))],
        },
    ]);
    rejects_unknown_alternative::<PrivacyClass>();
    rejects_unknown_alternative::<RetentionPeriod>();
    rejects_unknown_alternative::<RetentionAction>();
    rejects_unknown_alternative::<KeySelector>();
    rejects_unknown_alternative::<KeyRange>();
    rejects_unknown_alternative::<TransactionClass>();
    rejects_unknown_alternative::<AccessMode>();
    rejects_unknown_alternative::<Effect>();
}

#[test]
fn every_identity_permission_and_event_alternative_round_trips() {
    round_trip_all(&[
        CredentialIssuer::Unit(crate::types::UnitId::new("ministry")),
        CredentialIssuer::Role(RoleId::new("registrar")),
        CredentialIssuer::External {
            authority: "State University".to_string(),
        },
    ]);
    round_trip_all(&[
        CredentialValidity::UntilRevoked,
        CredentialValidity::Milliseconds(60_000),
    ]);
    round_trip_all(&[
        Capability::Read,
        Capability::Insert,
        Capability::Update,
        Capability::Delete,
        Capability::Invoke(ActionId::new("record_parcel")),
    ]);
    round_trip_all(&[
        FieldQualification::EveryField,
        FieldQualification::Only(vec![FieldPath::parse("area")]),
        FieldQualification::Apart(vec![FieldPath::parse("holder")]),
    ]);
    round_trip_all(&[
        RowQualification::EveryRow,
        RowQualification::Where(Expr::boolean(true)),
    ]);
    round_trip_all(&[AuditPolicy::Keep, AuditPolicy::Expire]);
    round_trip_all(&[
        DeliveryTarget::Workflow(WorkflowId::new("parcel_life")),
        DeliveryTarget::View(ViewId::new("parcels")),
        DeliveryTarget::Integration(IntegrationId::new("land_feed")),
    ]);
    round_trip_all(&[
        BridgeDirection::Inbound,
        BridgeDirection::Outbound,
        BridgeDirection::BothWays,
    ]);
    rejects_unknown_alternative::<CredentialIssuer>();
    rejects_unknown_alternative::<CredentialValidity>();
    rejects_unknown_alternative::<Capability>();
    rejects_unknown_alternative::<FieldQualification>();
    rejects_unknown_alternative::<RowQualification>();
    rejects_unknown_alternative::<AuditPolicy>();
    rejects_unknown_alternative::<DeliveryTarget>();
    rejects_unknown_alternative::<BridgeDirection>();
}

#[test]
fn every_view_and_workflow_alternative_round_trips() {
    round_trip_all(&[JoinKind::Matched, JoinKind::Optional]);
    round_trip_all(&[SortDirection::Ascending, SortDirection::Descending]);
    round_trip_all(&[
        AggregateFunction::Count,
        AggregateFunction::Total,
        AggregateFunction::Smallest,
        AggregateFunction::Largest,
    ]);
    round_trip_all(&[CyclePolicy::Forbid, CyclePolicy::Allow]);
    round_trip_all(&[
        TimerAction::MoveTo(StateId::new("closed")),
        TimerAction::Run(ActionId::new("approve")),
        TimerAction::Emit(EventId::new("reminded")),
    ]);
    rejects_unknown_alternative::<JoinKind>();
    rejects_unknown_alternative::<SortDirection>();
    rejects_unknown_alternative::<AggregateFunction>();
    rejects_unknown_alternative::<CyclePolicy>();
    rejects_unknown_alternative::<TimerAction>();
}

#[test]
fn every_interface_alternative_round_trips() {
    round_trip_all(ComponentKind::every());
    let groups: Vec<ComponentGroup> = ComponentKind::every()
        .iter()
        .map(|kind| kind.group())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert_eq!(groups.len(), 6);
    round_trip_all(&groups);
    round_trip_all(&[
        BindingNeed::Nothing,
        BindingNeed::Collection,
        BindingNeed::Field,
        BindingNeed::Action,
        BindingNeed::Workflow,
    ]);
    round_trip_all(&[
        ScreenKind::List,
        ScreenKind::Detail,
        ScreenKind::Create,
        ScreenKind::Edit,
        ScreenKind::Approval,
        ScreenKind::Custom,
    ]);
    round_trip_all(&[
        ConflictResolution::KeepStored,
        ConflictResolution::TakeCaptured,
        ConflictResolution::LaterTimestamp {
            field: FieldPath::parse("recorded_at"),
        },
        ConflictResolution::LargerValue {
            field: FieldPath::parse("area"),
        },
        ConflictResolution::MergeFields {
            fields: vec![FieldPath::parse("area")],
        },
        ConflictResolution::AskAPerson,
    ]);
    round_trip_all(&[
        Binding::Collection(CollectionId::new("parcel")),
        Binding::Field {
            collection: CollectionId::new("parcel"),
            field: FieldPath::parse("area"),
        },
        Binding::Parameter {
            action: ActionId::new("record_parcel"),
            parameter: ParamName::new("area"),
        },
        Binding::Action(ActionId::new("record_parcel")),
        Binding::Workflow(WorkflowId::new("parcel_life")),
        Binding::View(ViewId::new("parcels")),
        Binding::Event(EventId::new("recorded")),
    ]);
    round_trip_all(&[
        VisibilityRule::ForRoles(vec![RoleId::new("registrar")]),
        VisibilityRule::InStates {
            workflow: WorkflowId::new("parcel_life"),
            states: vec![StateId::new("draft")],
        },
        VisibilityRule::WhenValue(Expr::boolean(true)),
    ]);
    rejects_unknown_alternative::<ComponentKind>();
    rejects_unknown_alternative::<ComponentGroup>();
    rejects_unknown_alternative::<BindingNeed>();
    rejects_unknown_alternative::<ScreenKind>();
    rejects_unknown_alternative::<ConflictResolution>();
    rejects_unknown_alternative::<Binding>();
    rejects_unknown_alternative::<VisibilityRule>();
}
