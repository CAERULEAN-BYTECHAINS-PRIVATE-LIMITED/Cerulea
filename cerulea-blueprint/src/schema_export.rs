//! JSON Schema emission for the interface layer.
//!
//! The Blueprint stays the source of truth. This module writes a JSON
//! Schema document describing the record types, the collections, the action
//! parameters, the view rows, the event payloads and the composition
//! library. Cerulea Studio and Cerulea APIs read that document to draw
//! forms and to check input before a transaction leaves the browser.
//!
//! The emission is deterministic. The same Blueprint produces the same
//! bytes on every run. The pass sorts the Blueprint into canonical order
//! first, then writes every object in a fixed field order.
//!
//! This module is present under the `std` feature.

use crate::{
    actions::Action,
    collections::{Collection, CollectionRegistry, PrivacyClass},
    compose::Block,
    events::EventDef,
    types::{TypeDef, TypeExpr, TypeRegistry},
    views::View,
    Blueprint,
};
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

/// The JSON Schema dialect the document declares.
pub const SCHEMA_DIALECT: &str = "https://json-schema.org/draft/2020-12/schema";

/// A JSON value written in a fixed order.
///
/// An object keeps the order the writer placed its members in, so two runs
/// over the same Blueprint produce the same bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Json {
    /// The absent value.
    Null,
    /// A truth value.
    Bool(bool),
    /// A whole number.
    Number(i128),
    /// A run of characters.
    Text(String),
    /// An ordered run of values.
    Array(Vec<Json>),
    /// A set of named members in writing order.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// Build an object from named members.
    pub fn object(members: Vec<(&str, Json)>) -> Json {
        Json::Object(
            members
                .into_iter()
                .map(|(name, value)| (name.to_string(), value))
                .collect(),
        )
    }

    /// Build a run of text.
    pub fn text(value: &str) -> Json {
        Json::Text(value.to_string())
    }

    /// Build an array of text values.
    pub fn text_array(values: &[String]) -> Json {
        Json::Array(
            values
                .iter()
                .map(|value| Json::Text(value.clone()))
                .collect(),
        )
    }

    /// Write the value into a string.
    pub fn write(&self, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(true) => out.push_str("true"),
            Json::Bool(false) => out.push_str("false"),
            Json::Number(value) => out.push_str(&format!("{value}")),
            Json::Text(value) => write_text(value, out),
            Json::Array(items) => {
                out.push('[');
                for (position, item) in items.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Json::Object(members) => {
                out.push('{');
                for (position, (name, value)) in members.iter().enumerate() {
                    if position > 0 {
                        out.push(',');
                    }
                    write_text(name, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }

    /// The value as compact JSON text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }
}

fn write_text(value: &str, out: &mut String) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => {
                if (other as u32) < 0x20 {
                    out.push_str(&format!("\\u{:04x}", other as u32));
                } else {
                    out.push(other);
                }
            }
        }
    }
    out.push('"');
}

/// The JSON Schema of one Blueprint type.
pub fn type_schema(ty: &TypeExpr) -> Json {
    match ty {
        TypeExpr::Bool => Json::object(vec![("type", Json::text("boolean"))]),
        TypeExpr::Integer => Json::object(vec![
            ("type", Json::text("integer")),
            ("x-cerulea-type", Json::text("integer")),
        ]),
        TypeExpr::Decimal { scale } => Json::object(vec![
            ("type", Json::text("object")),
            ("x-cerulea-type", Json::text("decimal")),
            (
                "properties",
                Json::object(vec![
                    ("units", Json::object(vec![("type", Json::text("integer"))])),
                    (
                        "scale",
                        Json::object(vec![("const", Json::Number(*scale as i128))]),
                    ),
                ]),
            ),
            (
                "required",
                Json::Array(vec![Json::text("units"), Json::text("scale")]),
            ),
            ("additionalProperties", Json::Bool(false)),
        ]),
        TypeExpr::Text => Json::object(vec![("type", Json::text("string"))]),
        TypeExpr::Bytes => Json::object(vec![
            ("type", Json::text("string")),
            ("x-cerulea-type", Json::text("bytes")),
            ("pattern", Json::text("^([0-9a-f]{2})*$")),
        ]),
        TypeExpr::Timestamp => Json::object(vec![
            ("type", Json::text("integer")),
            ("x-cerulea-type", Json::text("timestamp")),
            ("minimum", Json::Number(0)),
        ]),
        TypeExpr::Identity => Json::object(vec![
            ("type", Json::text("string")),
            ("x-cerulea-type", Json::text("identity")),
            ("pattern", Json::text("^[0-9a-f]{64}$")),
        ]),
        TypeExpr::Hash => Json::object(vec![
            ("type", Json::text("string")),
            ("x-cerulea-type", Json::text("hash")),
            ("pattern", Json::text("^[0-9a-f]{64}$")),
        ]),
        TypeExpr::List(element) => Json::object(vec![
            ("type", Json::text("array")),
            ("items", type_schema(element)),
        ]),
        TypeExpr::Map { key, value } => Json::object(vec![
            ("type", Json::text("array")),
            ("x-cerulea-type", Json::text("map")),
            (
                "items",
                Json::object(vec![
                    ("type", Json::text("object")),
                    (
                        "properties",
                        Json::object(vec![
                            ("key", type_schema(key)),
                            ("value", type_schema(value)),
                        ]),
                    ),
                    (
                        "required",
                        Json::Array(vec![Json::text("key"), Json::text("value")]),
                    ),
                    ("additionalProperties", Json::Bool(false)),
                ]),
            ),
        ]),
        TypeExpr::Optional(element) => Json::object(vec![(
            "oneOf",
            Json::Array(vec![
                Json::object(vec![("type", Json::text("null"))]),
                type_schema(element),
            ]),
        )]),
        TypeExpr::Record(id) | TypeExpr::Enumeration(id) | TypeExpr::Sum(id) => {
            Json::object(vec![("$ref", Json::Text(format!("#/$defs/{id}")))])
        }
        TypeExpr::Reference(collection) => Json::object(vec![
            ("type", Json::text("object")),
            ("x-cerulea-type", Json::text("reference")),
            (
                "properties",
                Json::object(vec![
                    (
                        "collection",
                        Json::object(vec![("const", Json::Text(collection.to_string()))]),
                    ),
                    ("key", Json::object(vec![])),
                ]),
            ),
            (
                "required",
                Json::Array(vec![Json::text("collection"), Json::text("key")]),
            ),
        ]),
    }
}

/// The JSON Schema definition of one declared type.
pub fn definition_schema(def: &TypeDef) -> Json {
    match def {
        TypeDef::Record(record) => {
            let properties: Vec<(String, Json)> = record
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.as_str().to_string(),
                        field_schema(&field.ty, &field.summary),
                    )
                })
                .collect();
            let required: Vec<String> = record
                .fields
                .iter()
                .map(|field| field.name.as_str().to_string())
                .collect();
            Json::object(vec![
                ("type", Json::text("object")),
                ("title", Json::Text(record.id.as_str().to_string())),
                ("description", Json::Text(record.summary.clone())),
                ("properties", Json::Object(properties)),
                ("required", Json::text_array(&required)),
                ("additionalProperties", Json::Bool(false)),
            ])
        }
        TypeDef::Enumeration(enumeration) => {
            let names: Vec<String> = enumeration
                .variants
                .iter()
                .map(|variant| variant.as_str().to_string())
                .collect();
            Json::object(vec![
                ("type", Json::text("string")),
                ("title", Json::Text(enumeration.id.as_str().to_string())),
                ("description", Json::Text(enumeration.summary.clone())),
                ("enum", Json::text_array(&names)),
            ])
        }
        TypeDef::Sum(sum) => {
            let alternatives: Vec<Json> = sum
                .variants
                .iter()
                .map(|variant| {
                    let mut properties = vec![(
                        "variant".to_string(),
                        Json::object(vec![(
                            "const",
                            Json::Text(variant.name.as_str().to_string()),
                        )]),
                    )];
                    let mut required = vec!["variant".to_string()];
                    if let Some(payload) = &variant.payload {
                        properties.push(("payload".to_string(), type_schema(payload)));
                        required.push("payload".to_string());
                    }
                    Json::object(vec![
                        ("type", Json::text("object")),
                        ("properties", Json::Object(properties)),
                        ("required", Json::text_array(&required)),
                        ("additionalProperties", Json::Bool(false)),
                    ])
                })
                .collect();
            Json::object(vec![
                ("title", Json::Text(sum.id.as_str().to_string())),
                ("description", Json::Text(sum.summary.clone())),
                ("oneOf", Json::Array(alternatives)),
            ])
        }
    }
}

fn field_schema(ty: &TypeExpr, summary: &str) -> Json {
    match type_schema(ty) {
        Json::Object(mut members) => {
            members.push(("description".to_string(), Json::Text(summary.to_string())));
            Json::Object(members)
        }
        other => other,
    }
}

/// The JSON Schema of the parameters one action takes.
pub fn action_schema(action: &Action) -> Json {
    let properties: Vec<(String, Json)> = action
        .params
        .iter()
        .map(|param| {
            (
                param.name.as_str().to_string(),
                field_schema(&param.ty, &param.summary),
            )
        })
        .collect();
    let required: Vec<String> = action
        .params
        .iter()
        .map(|param| param.name.as_str().to_string())
        .collect();
    let roles: Vec<String> = action
        .roles
        .iter()
        .map(|role| role.as_str().to_string())
        .collect();
    let emits: Vec<String> = action
        .emits
        .iter()
        .map(|event| event.as_str().to_string())
        .collect();
    Json::object(vec![
        ("description", Json::Text(action.summary.clone())),
        ("class", Json::text(action.class.label())),
        (
            "thresholdPercent",
            Json::Number(action.class.threshold_percent() as i128),
        ),
        ("holdsDelay", Json::Bool(action.class.holds_delay())),
        ("roles", Json::text_array(&roles)),
        ("emits", Json::text_array(&emits)),
        (
            "parameters",
            Json::object(vec![
                ("type", Json::text("object")),
                ("properties", Json::Object(properties)),
                ("required", Json::text_array(&required)),
                ("additionalProperties", Json::Bool(false)),
            ]),
        ),
    ])
}

/// The JSON Schema of one collection.
pub fn collection_schema(collection: &Collection) -> Json {
    let purposes: Vec<String> = collection
        .purpose
        .purposes
        .iter()
        .map(|purpose| purpose.as_str().to_string())
        .collect();
    let indexes: Vec<String> = collection
        .indexes
        .iter()
        .map(|index| index.name.as_str().to_string())
        .collect();
    Json::object(vec![
        ("description", Json::Text(collection.summary.clone())),
        (
            "record",
            type_schema(&TypeExpr::Record(collection.record_type.clone())),
        ),
        ("primaryKey", Json::Text(collection.primary_key.to_string())),
        ("privacy", Json::text(collection.privacy.label())),
        ("indexes", Json::text_array(&indexes)),
        (
            "purposeStatement",
            Json::Text(collection.purpose.statement.clone()),
        ),
        ("purposes", Json::text_array(&purposes)),
    ])
}

/// The JSON Schema of the rows one view returns.
pub fn view_schema(view: &View, types: &TypeRegistry, collections: &CollectionRegistry) -> Json {
    let fields = view.result_types(types, collections);
    let properties: Vec<(String, Json)> = fields
        .iter()
        .map(|(name, ty)| (name.as_str().to_string(), type_schema(ty)))
        .collect();
    let required: Vec<String> = fields
        .iter()
        .map(|(name, _)| name.as_str().to_string())
        .collect();
    let roles: Vec<String> = view
        .roles
        .iter()
        .map(|role| role.as_str().to_string())
        .collect();
    Json::object(vec![
        ("description", Json::Text(view.summary.clone())),
        ("source", Json::Text(view.source.as_str().to_string())),
        ("roles", Json::text_array(&roles)),
        (
            "row",
            Json::object(vec![
                ("type", Json::text("object")),
                ("properties", Json::Object(properties)),
                ("required", Json::text_array(&required)),
                ("additionalProperties", Json::Bool(false)),
            ]),
        ),
    ])
}

/// The JSON Schema of one event.
pub fn event_schema(event: &EventDef) -> Json {
    Json::object(vec![
        ("description", Json::Text(event.summary.clone())),
        ("payload", type_schema(&event.payload_type())),
    ])
}

/// The JSON Schema of one reusable block.
pub fn block_schema(block: &Block) -> Json {
    let inputs: Vec<(String, Json)> = block
        .inputs
        .iter()
        .map(|param| {
            (
                param.name.as_str().to_string(),
                field_schema(&param.ty, &param.summary),
            )
        })
        .collect();
    let outputs: Vec<(String, Json)> = block
        .outputs
        .iter()
        .map(|output| (output.name.as_str().to_string(), type_schema(&output.ty)))
        .collect();
    Json::object(vec![
        ("description", Json::Text(block.summary.clone())),
        (
            "inputs",
            Json::object(vec![
                ("type", Json::text("object")),
                ("properties", Json::Object(inputs)),
            ]),
        ),
        (
            "outputs",
            Json::object(vec![
                ("type", Json::text("object")),
                ("properties", Json::Object(outputs)),
            ]),
        ),
    ])
}

/// The whole JSON Schema document for one Blueprint.
pub fn document(blueprint: &Blueprint) -> Json {
    let canonical = blueprint.canonical();
    let types_path = crate::Path::root().field("types");
    let (type_registry, _) = TypeRegistry::build(&canonical.types, &types_path);
    let (collection_registry, _) = CollectionRegistry::build(
        &canonical.collections,
        &crate::Path::root().field("collections"),
    );

    let defs: Vec<(String, Json)> = canonical
        .types
        .iter()
        .map(|def| (def.id().as_str().to_string(), definition_schema(def)))
        .collect();
    let collections: Vec<(String, Json)> = canonical
        .collections
        .iter()
        .map(|collection| {
            (
                collection.id.as_str().to_string(),
                collection_schema(collection),
            )
        })
        .collect();
    let actions: Vec<(String, Json)> = canonical
        .actions
        .iter()
        .map(|action| (action.id.as_str().to_string(), action_schema(action)))
        .collect();
    let views: Vec<(String, Json)> = canonical
        .views
        .iter()
        .map(|view| {
            (
                view.id.as_str().to_string(),
                view_schema(view, &type_registry, &collection_registry),
            )
        })
        .collect();
    let events: Vec<(String, Json)> = canonical
        .events
        .iter()
        .map(|event| (event.id.as_str().to_string(), event_schema(event)))
        .collect();
    let blocks: Vec<(String, Json)> = canonical
        .blocks
        .iter()
        .map(|block| (block.id.as_str().to_string(), block_schema(block)))
        .collect();

    Json::object(vec![
        ("$schema", Json::text(SCHEMA_DIALECT)),
        (
            "$id",
            Json::Text(format!("cerulea:blueprint:{}", canonical.id())),
        ),
        ("title", Json::Text(canonical.meta.name.clone())),
        ("description", Json::Text(canonical.meta.summary.clone())),
        (
            "x-cerulea",
            Json::object(vec![
                ("owner", Json::Text(canonical.meta.owner.clone())),
                (
                    "schemaVersion",
                    Json::Text(canonical.meta.version.schema.to_string()),
                ),
                (
                    "revision",
                    Json::Number(canonical.meta.version.revision as i128),
                ),
                (
                    "carriesEconomics",
                    Json::Bool(canonical.economics.is_some()),
                ),
            ]),
        ),
        ("$defs", Json::Object(defs)),
        ("collections", Json::Object(collections)),
        ("actions", Json::Object(actions)),
        ("views", Json::Object(views)),
        ("events", Json::Object(events)),
        ("blocks", Json::Object(blocks)),
    ])
}

/// The whole JSON Schema document as compact text.
pub fn export(blueprint: &Blueprint) -> String {
    document(blueprint).to_text()
}

impl Blueprint {
    /// The JSON Schema document the interface layer reads.
    pub fn json_schema(&self) -> String {
        export(self)
    }
}

/// Check the privacy label a collection carries, for an interface layer
/// that colours a panel by class.
pub fn privacy_label(privacy: &PrivacyClass) -> &'static str {
    privacy.label()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{Effect, Param, TransactionClass},
        collections::{Collection, KeySelector, PurposeDeclaration},
        expression::Expr,
        identities::RoleDef,
        types::{
            CollectionId, EnumerationDef, FieldDef, FieldName, RecordDef, RoleId, SumDef,
            SumVariant, TypeId, VariantName,
        },
        BlueprintMeta,
    };
    use alloc::vec;

    fn sample() -> Blueprint {
        let mut blueprint = Blueprint::new(BlueprintMeta::new(
            "district land registry",
            "The registry records land parcels and their holders.",
            "Kollam District Administration",
        ));
        blueprint.types = vec![
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
                        name: VariantName::new("withdrawn"),
                        payload: None,
                    },
                ],
                summary: "The outcome of a review.".to_string(),
            }),
            TypeDef::Record(RecordDef {
                id: TypeId::new("parcel"),
                fields: vec![
                    FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
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
        ];
        let mut parcel =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        parcel.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        blueprint.collections = vec![parcel];
        blueprint.identities.roles = vec![RoleDef::new(
            "registrar",
            None,
            "Registrar",
            "The registrar records.",
        )];

        let mut action =
            crate::actions::Action::new("record_parcel", "The action records one land parcel.");
        action.roles = vec![RoleId::new("registrar")];
        action.class = TransactionClass::Structural;
        action.params = vec![
            Param::new("survey_number", TypeExpr::Text, "The survey number."),
            Param::new(
                "area",
                TypeExpr::Decimal { scale: 4 },
                "The area in hectares.",
            ),
            Param::new("holder", TypeExpr::Identity, "The registered holder."),
        ];
        action.effects = vec![Effect::Insert {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Exact(Expr::bound("survey_number")),
            record: Expr::MakeRecord {
                type_id: TypeId::new("parcel"),
                fields: vec![
                    (
                        FieldName::new("survey_number"),
                        Expr::bound("survey_number"),
                    ),
                    (FieldName::new("area"), Expr::bound("area")),
                    (FieldName::new("holder"), Expr::bound("holder")),
                    (
                        FieldName::new("tenure"),
                        Expr::MakeEnumeration {
                            type_id: TypeId::new("tenure"),
                            variant: VariantName::new("freehold"),
                        },
                    ),
                ],
            },
        }];
        blueprint.actions = vec![action];
        blueprint
    }

    #[test]
    fn the_export_is_deterministic() {
        let blueprint = sample();
        assert_eq!(export(&blueprint), export(&blueprint));
        let mut shuffled = sample();
        shuffled.types.reverse();
        assert_eq!(export(&blueprint), export(&shuffled));
    }

    #[test]
    fn a_change_to_the_blueprint_changes_the_export() {
        let blueprint = sample();
        let mut changed = sample();
        changed.meta.summary.push('.');
        assert_ne!(export(&blueprint), export(&changed));
    }

    #[test]
    fn the_export_parses_as_json() {
        let text = export(&sample());
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
        assert_eq!(parsed["$schema"], SCHEMA_DIALECT);
        assert!(parsed["$id"]
            .as_str()
            .expect("the identifier is text")
            .starts_with("cerulea:blueprint:"));
        assert_eq!(parsed["title"], "district land registry");
    }

    #[test]
    fn the_definitions_carry_every_declared_type() {
        let text = export(&sample());
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
        let defs = parsed["$defs"]
            .as_object()
            .expect("the definitions are an object");
        assert_eq!(defs.len(), 3);
        assert_eq!(defs["parcel"]["type"], "object");
        assert_eq!(defs["tenure"]["enum"][0], "freehold");
        assert_eq!(
            defs["ruling"]["oneOf"][0]["properties"]["variant"]["const"],
            "approval"
        );
        assert!(defs["ruling"]["oneOf"][1]["properties"]
            .get("payload")
            .is_none());
    }

    #[test]
    fn a_record_definition_names_its_required_fields() {
        let text = export(&sample());
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
        let parcel = &parsed["$defs"]["parcel"];
        let required = parcel["required"]
            .as_array()
            .expect("the required list is an array");
        assert_eq!(required.len(), 4);
        assert_eq!(parcel["properties"]["survey_number"]["type"], "string");
        assert_eq!(parcel["properties"]["area"]["x-cerulea-type"], "decimal");
        assert_eq!(
            parcel["properties"]["area"]["properties"]["scale"]["const"],
            4
        );
        assert_eq!(parcel["properties"]["tenure"]["$ref"], "#/$defs/tenure");
        assert_eq!(
            parcel["properties"]["holder"]["description"],
            "The registered holder."
        );
    }

    #[test]
    fn an_action_carries_its_parameter_schema_and_its_threshold() {
        let text = export(&sample());
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
        let action = &parsed["actions"]["record_parcel"];
        assert_eq!(action["class"], "structural");
        assert_eq!(action["thresholdPercent"], 80);
        assert_eq!(action["holdsDelay"], false);
        assert_eq!(action["roles"][0], "registrar");
        assert_eq!(action["parameters"]["type"], "object");
        assert_eq!(
            action["parameters"]["required"]
                .as_array()
                .expect("an array")
                .len(),
            3
        );
    }

    #[test]
    fn a_collection_carries_its_privacy_and_its_purposes() {
        let text = export(&sample());
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the document parses");
        let collection = &parsed["collections"]["parcel"];
        assert_eq!(collection["privacy"], "open");
        assert_eq!(collection["primaryKey"], "survey_number");
        assert_eq!(collection["purposes"][0], "registration");
        assert_eq!(collection["record"]["$ref"], "#/$defs/parcel");
    }

    #[test]
    fn every_primitive_carries_a_schema() {
        for ty in [
            TypeExpr::Bool,
            TypeExpr::Integer,
            TypeExpr::Decimal { scale: 2 },
            TypeExpr::Text,
            TypeExpr::Bytes,
            TypeExpr::Timestamp,
            TypeExpr::Identity,
            TypeExpr::Hash,
            TypeExpr::list(TypeExpr::Integer),
            TypeExpr::map(TypeExpr::Text, TypeExpr::Integer),
            TypeExpr::optional(TypeExpr::Text),
            TypeExpr::Record(TypeId::new("parcel")),
            TypeExpr::Enumeration(TypeId::new("tenure")),
            TypeExpr::Sum(TypeId::new("ruling")),
            TypeExpr::Reference(CollectionId::new("parcel")),
        ] {
            let text = type_schema(&ty).to_text();
            let parsed: serde_json::Value = serde_json::from_str(&text).expect("the schema parses");
            assert!(parsed.is_object(), "{ty:?}");
        }
    }

    #[test]
    fn the_writer_escapes_text() {
        let value = Json::Text("a \"quoted\" line\nand a tab\t".to_string());
        let text = value.to_text();
        assert!(text.contains("\\\""));
        assert!(text.contains("\\n"));
        assert!(text.contains("\\t"));
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(parsed, "a \"quoted\" line\nand a tab\t");
    }

    #[test]
    fn the_writer_keeps_the_order_it_was_given() {
        let value = Json::object(vec![
            ("second", Json::Number(2)),
            ("first", Json::Number(1)),
        ]);
        assert_eq!(value.to_text(), "{\"second\":2,\"first\":1}");
    }

    #[test]
    fn the_blueprint_exports_its_own_schema() {
        let blueprint = sample();
        assert_eq!(blueprint.json_schema(), export(&blueprint));
        assert_eq!(privacy_label(&blueprint.collections[0].privacy), "open");
    }
}
