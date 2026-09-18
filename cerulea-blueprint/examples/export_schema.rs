//! Write the Blueprint fragment schema, and the interface document of the
//! reference fragment, into the content repository.
//!
//! Two documents leave this program.
//!
//! 1. `content/schema/blueprint.schema.json`. A JSON Schema describing the
//!    serde form of the Blueprint type set. Every module fragment in the
//!    content repository is one instance of that schema. The fragment
//!    validator reads this document.
//! 2. `content/schema/reference.interface.json`. The document
//!    `schema_export` emits for one Blueprint. Cerulea Studio and Cerulea
//!    APIs read a document of that shape to draw forms. The program writes
//!    it when the reference fragment is present.
//!
//! Both documents leave through the writer in `schema_export`, so the bytes
//! stay the same on every run.
//!
//! Run from the chain directory:
//!
//! ```text
//! cargo run --release -p cerulea-blueprint --example export_schema
//! ```

use cerulea_blueprint::{
    schema_export::{export, Json, SCHEMA_DIALECT},
    Blueprint, BLUEPRINT_SCHEMA_VERSION,
};
use std::{fs, path::PathBuf};

fn main() {
    let root = content_root();
    let schema_dir = root.join("schema");
    fs::create_dir_all(&schema_dir).expect("the schema directory opens");

    let fragment_schema = schema_dir.join("blueprint.schema.json");
    fs::write(&fragment_schema, pretty(&fragment_document())).expect("the schema document writes");
    println!("wrote {}", fragment_schema.display());

    let reference = root.join("modules").join("_reference.json");
    if reference.exists() {
        let text = fs::read_to_string(&reference).expect("the reference fragment reads");
        let blueprint: Blueprint =
            serde_json::from_str(&text).expect("the reference fragment parses");
        let target = schema_dir.join("reference.interface.json");
        fs::write(&target, export(&blueprint)).expect("the interface document writes");
        println!("wrote {}", target.display());
    } else {
        println!("the reference fragment is absent, so the interface document stays unwritten");
    }
}

/// The content repository, beside the chain repository.
fn content_root() -> PathBuf {
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.parent()
        .and_then(|chain| chain.parent())
        .expect("the workspace root sits two levels above the crate")
        .join("content")
}

// ---------------------------------------------------------------------------
// Small builders over the writer in schema_export
// ---------------------------------------------------------------------------

fn obj(members: Vec<(&str, Json)>) -> Json {
    Json::object(members)
}

fn s(text: &str) -> Json {
    Json::text(text)
}

fn n(value: i128) -> Json {
    Json::Number(value)
}

fn words(values: &[&str]) -> Json {
    Json::Array(values.iter().map(|value| s(value)).collect())
}

/// A reference into the definition table.
fn r(name: &str) -> Json {
    obj(vec![("$ref", Json::Text(format!("#/$defs/{name}")))])
}

/// An array of one element shape.
fn arr(items: Json) -> Json {
    obj(vec![("type", s("array")), ("items", items)])
}

/// A two member tuple, as serde writes a pair.
fn pair(left: Json, right: Json) -> Json {
    obj(vec![
        ("type", s("array")),
        ("prefixItems", Json::Array(vec![left, right])),
        ("minItems", n(2)),
        ("maxItems", n(2)),
    ])
}

/// An object with a fixed member list. Every named member is required.
fn record(note: &str, members: Vec<(&str, Json)>) -> Json {
    let required: Vec<&str> = members.iter().map(|(name, _)| *name).collect();
    let required = words(&required);
    obj(vec![
        ("type", s("object")),
        ("description", s(note)),
        ("properties", Json::object(members)),
        ("required", required),
        ("additionalProperties", Json::Bool(false)),
    ])
}

/// An object with a fixed member list where some members carry no value.
fn record_with_optional(
    note: &str,
    members: Vec<(&str, Json)>,
    required: &[&str],
) -> Json {
    obj(vec![
        ("type", s("object")),
        ("description", s(note)),
        ("properties", Json::object(members)),
        ("required", words(required)),
        ("additionalProperties", Json::Bool(false)),
    ])
}

/// A value that is present or absent.
fn nullable(inner: Json) -> Json {
    obj(vec![(
        "oneOf",
        Json::Array(vec![obj(vec![("type", s("null"))]), inner]),
    )])
}

/// One alternative of a serde enumeration that carries data.
fn carried(name: &str, payload: Json) -> Json {
    obj(vec![
        ("type", s("object")),
        (
            "properties",
            Json::object(vec![(name, payload)]),
        ),
        ("required", words(&[name])),
        ("additionalProperties", Json::Bool(false)),
    ])
}

/// A serde enumeration. The bare names carry no data. The carried
/// alternatives arrive already built.
fn choice(note: &str, bare: &[&str], carried_alternatives: Vec<Json>) -> Json {
    let mut alternatives: Vec<Json> = Vec::new();
    if !bare.is_empty() {
        alternatives.push(obj(vec![
            ("type", s("string")),
            ("enum", words(bare)),
        ]));
    }
    alternatives.extend(carried_alternatives);
    obj(vec![
        ("description", s(note)),
        ("oneOf", Json::Array(alternatives)),
    ])
}

/// A plain enumeration of names.
fn names(note: &str, values: &[&str]) -> Json {
    obj(vec![
        ("description", s(note)),
        ("type", s("string")),
        ("enum", words(values)),
    ])
}

fn text_field(note: &str) -> Json {
    obj(vec![("type", s("string")), ("description", s(note))])
}

fn sentence(note: &str) -> Json {
    obj(vec![
        ("type", s("string")),
        ("minLength", n(1)),
        ("description", s(note)),
    ])
}

fn whole(note: &str) -> Json {
    obj(vec![("type", s("integer")), ("description", s(note))])
}

fn counted(note: &str, floor: i128) -> Json {
    obj(vec![
        ("type", s("integer")),
        ("minimum", n(floor)),
        ("description", s(note)),
    ])
}

fn truth(note: &str) -> Json {
    obj(vec![("type", s("boolean")), ("description", s(note))])
}

fn byte_run(note: &str, length: Option<i128>) -> Json {
    let mut members = vec![
        ("type", s("array")),
        ("items", obj(vec![
            ("type", s("integer")),
            ("minimum", n(0)),
            ("maximum", n(255)),
        ])),
        ("description", s(note)),
    ];
    if let Some(length) = length {
        members.push(("minItems", n(length)));
        members.push(("maxItems", n(length)));
    }
    obj(members)
}

// ---------------------------------------------------------------------------
// The fragment schema
// ---------------------------------------------------------------------------

fn fragment_document() -> Json {
    let version = BLUEPRINT_SCHEMA_VERSION;
    obj(vec![
        ("$schema", s(SCHEMA_DIALECT)),
        ("$id", s("cerulea:blueprint:fragment:1")),
        ("title", s("Cerulea Blueprint fragment")),
        (
            "description",
            s("One module fragment of the Cerulea module catalogue. A fragment is the serde form of the Blueprint type set, with the catalogue entry and the declared module dependencies beside it."),
        ),
        (
            "x-cerulea",
            obj(vec![
                (
                    "schemaVersion",
                    Json::Text(version.to_string()),
                ),
                ("sections", words(&[
                    "types",
                    "collections",
                    "identities",
                    "permissions",
                    "rules",
                    "actions",
                    "workflows",
                    "events",
                    "views",
                    "integrations",
                    "governance",
                    "economics",
                ])),
            ]),
        ),
        ("type", s("object")),
        (
            "properties",
            Json::object(vec![
                ("module", r("ModuleEntry")),
                ("dependencies", arr(r("Dependency"))),
                ("meta", r("BlueprintMeta")),
                ("types", arr(r("TypeDef"))),
                ("collections", arr(r("Collection"))),
                ("identities", r("Identities")),
                ("permissions", r("Permissions")),
                ("rules", r("Rules")),
                ("actions", arr(r("Action"))),
                ("workflows", arr(r("Workflow"))),
                ("events", arr(r("EventDef"))),
                ("views", arr(r("View"))),
                ("integrations", r("Integrations")),
                ("governance", r("Governance")),
                ("economics", nullable(r("Economics"))),
                ("blocks", arr(r("Block"))),
            ]),
        ),
        (
            "required",
            words(&[
                "module",
                "dependencies",
                "meta",
                "types",
                "collections",
                "identities",
                "permissions",
                "rules",
                "actions",
                "workflows",
                "events",
                "views",
                "integrations",
                "governance",
                "economics",
                "blocks",
            ]),
        ),
        ("additionalProperties", Json::Bool(false)),
        ("$defs", Json::Object(definitions())),
    ])
}

fn definitions() -> Vec<(String, Json)> {
    let mut defs: Vec<(String, Json)> = Vec::new();
    let mut push = |name: &str, value: Json| defs.push((name.to_string(), value));

    push(
        "Identifier",
        obj(vec![
            (
                "description",
                s("A Blueprint name. The name starts with a lower case letter and holds lower case letters, digits and underscores."),
            ),
            ("type", s("string")),
            ("pattern", s("^[a-z][a-z0-9_]*$")),
            ("minLength", n(1)),
            ("maxLength", n(64)),
        ]),
    );
    push(
        "FieldPath",
        obj(vec![
            (
                "description",
                s("A path to a field inside a record, held as one name per step."),
            ),
            ("type", s("array")),
            ("items", r("Identifier")),
            ("minItems", n(1)),
        ]),
    );
    push(
        "ModuleEntry",
        record(
            "The catalogue entry this fragment installs.",
            vec![
                ("id", text_field("The catalogue identifier of the module.")),
                (
                    "position",
                    counted("The position of the module in the catalogue, counting from one.", 1),
                ),
                ("title", sentence("The title a person reads on screen.")),
                (
                    "track",
                    names("The application track the module serves.", &["dapp", "enterprise", "government", "blockchain"]),
                ),
                ("category", sentence("The catalogue category.")),
                (
                    "prefix",
                    r("Identifier"),
                ),
                (
                    "contributes",
                    sentence("One plain sentence stating what the module adds to a deployment."),
                ),
                (
                    "removal",
                    sentence("One plain sentence stating what stops working after removal."),
                ),
            ],
        ),
    );
    push(
        "Dependency",
        record(
            "One module this fragment depends on.",
            vec![
                ("module", text_field("The catalogue identifier of the module depended on.")),
                ("fragment", text_field("The fragment file holding that module.")),
                (
                    "kind",
                    names("The strength of the link.", &["requires", "recommends"]),
                ),
                ("reason", sentence("One plain sentence stating why the link holds.")),
            ],
        ),
    );
    push(
        "SchemaVersion",
        record(
            "A three part version number.",
            vec![
                ("major", counted("The major number.", 0)),
                ("minor", counted("The minor number.", 0)),
                ("patch", counted("The patch number.", 0)),
            ],
        ),
    );
    push(
        "VersionStamp",
        record(
            "The version stamp every Blueprint carries.",
            vec![
                ("schema", r("SchemaVersion")),
                ("revision", counted("The revision of this deployment.", 1)),
            ],
        ),
    );
    push(
        "BlueprintMeta",
        record(
            "The descriptive header of a fragment.",
            vec![
                ("name", sentence("The name of the deployment part.")),
                ("summary", sentence("One plain sentence stating what the part does.")),
                ("owner", sentence("The organisation that owns the deployment part.")),
                ("version", r("VersionStamp")),
            ],
        ),
    );

    push(
        "TypeExpr",
        choice(
            "A type in the Blueprint.",
            &[
                "Bool",
                "Integer",
                "Text",
                "Bytes",
                "Timestamp",
                "Identity",
                "Hash",
            ],
            vec![
                carried(
                    "Decimal",
                    record(
                        "A fixed-point decimal carrying an explicit scale.",
                        vec![(
                            "scale",
                            obj(vec![
                                ("type", s("integer")),
                                ("minimum", n(0)),
                                ("maximum", n(30)),
                                ("description", s("The count of digits after the decimal point.")),
                            ]),
                        )],
                    ),
                ),
                carried("List", r("TypeExpr")),
                carried(
                    "Map",
                    record(
                        "A set of key and value pairs sorted by key.",
                        vec![("key", r("TypeExpr")), ("value", r("TypeExpr"))],
                    ),
                ),
                carried("Optional", r("TypeExpr")),
                carried("Record", r("Identifier")),
                carried("Enumeration", r("Identifier")),
                carried("Sum", r("Identifier")),
                carried("Reference", r("Identifier")),
            ],
        ),
    );
    push(
        "Decimal",
        record(
            "A fixed-point decimal. The value equals the units divided by ten raised to the scale.",
            vec![
                ("units", whole("The scaled integer holding every digit.")),
                (
                    "scale",
                    obj(vec![
                        ("type", s("integer")),
                        ("minimum", n(0)),
                        ("maximum", n(30)),
                    ]),
                ),
            ],
        ),
    );
    push(
        "Value",
        choice(
            "A Blueprint value.",
            &[],
            vec![
                carried("Bool", truth("A truth value.")),
                carried("Integer", whole("A whole number.")),
                carried("Decimal", r("Decimal")),
                carried("Text", text_field("A run of characters.")),
                carried("Bytes", byte_run("A run of bytes.", None)),
                carried("Timestamp", counted("Milliseconds since the start of 1970.", 0)),
                carried("Identity", byte_run("An account or an organisation.", Some(32))),
                carried("Hash", byte_run("A 32 byte hash.", Some(32))),
                carried(
                    "List",
                    record(
                        "A list value carrying its element type.",
                        vec![("element", r("TypeExpr")), ("items", arr(r("Value")))],
                    ),
                ),
                carried(
                    "Map",
                    record(
                        "A map value carrying its key and value types.",
                        vec![
                            ("key", r("TypeExpr")),
                            ("value", r("TypeExpr")),
                            ("entries", arr(pair(r("Value"), r("Value")))),
                        ],
                    ),
                ),
                carried(
                    "Optional",
                    record(
                        "An optional value carrying its element type.",
                        vec![("element", r("TypeExpr")), ("item", nullable(r("Value")))],
                    ),
                ),
                carried(
                    "Record",
                    record(
                        "A record value.",
                        vec![
                            ("type_id", r("Identifier")),
                            ("fields", arr(pair(r("Identifier"), r("Value")))),
                        ],
                    ),
                ),
                carried(
                    "Enumeration",
                    record(
                        "An enumeration value.",
                        vec![("type_id", r("Identifier")), ("variant", r("Identifier"))],
                    ),
                ),
                carried(
                    "Sum",
                    record(
                        "A sum value.",
                        vec![
                            ("type_id", r("Identifier")),
                            ("variant", r("Identifier")),
                            ("payload", nullable(r("Value"))),
                        ],
                    ),
                ),
                carried(
                    "Reference",
                    record(
                        "A pointer to one record held in a collection.",
                        vec![("collection", r("Identifier")), ("key", r("Value"))],
                    ),
                ),
            ],
        ),
    );

    push(
        "UnaryOp",
        names("An operation over one operand.", &["Not", "Negate"]),
    );
    push(
        "BinaryOp",
        names(
            "An operation over two operands.",
            &[
                "Add",
                "Subtract",
                "Multiply",
                "Divide",
                "Remainder",
                "Equal",
                "NotEqual",
                "Less",
                "LessOrEqual",
                "Greater",
                "GreaterOrEqual",
                "And",
                "Or",
                "ConcatText",
                "AppendList",
            ],
        ),
    );
    push(
        "Builtin",
        names(
            "A named operation the Blueprint supplies.",
            &[
                "Now",
                "Caller",
                "HashOf",
                "TimestampPlus",
                "TimestampSince",
                "AbsoluteInteger",
                "MinimumInteger",
                "MaximumInteger",
                "BytesOfText",
            ],
        ),
    );
    push(
        "Pattern",
        choice(
            "A pattern in a match arm.",
            &["Absent"],
            vec![
                carried(
                    "Variant",
                    record_with_optional(
                        "One alternative of a sum type.",
                        vec![
                            ("variant", r("Identifier")),
                            ("binding", nullable(r("Identifier"))),
                        ],
                        &["variant", "binding"],
                    ),
                ),
                carried(
                    "Choice",
                    record(
                        "One alternative of an enumeration.",
                        vec![("variant", r("Identifier"))],
                    ),
                ),
                carried(
                    "Present",
                    record(
                        "A present optional.",
                        vec![("binding", r("Identifier"))],
                    ),
                ),
            ],
        ),
    );
    push(
        "MatchArm",
        record(
            "One arm of a match expression.",
            vec![("pattern", r("Pattern")), ("body", r("Expr"))],
        ),
    );
    push(
        "FoldBound",
        record(
            "The declared maximum element count of a fold.",
            vec![(
                "maximum_elements",
                counted("The largest element count the fold accepts.", 1),
            )],
        ),
    );
    push(
        "FoldSource",
        choice(
            "The elements a fold walks.",
            &[],
            vec![
                carried("List", r("Expr")),
                carried(
                    "Collection",
                    record(
                        "A key range inside a collection.",
                        vec![
                            ("collection", r("Identifier")),
                            ("keys", r("KeySelector")),
                            ("element", r("TypeExpr")),
                        ],
                    ),
                ),
            ],
        ),
    );
    push(
        "FoldExpr",
        record(
            "A bounded walk over a list or a collection range.",
            vec![
                ("source", r("FoldSource")),
                ("element", r("Identifier")),
                ("accumulator", r("Identifier")),
                ("initial", r("Expr")),
                ("step", r("Expr")),
                ("bound", r("FoldBound")),
            ],
        ),
    );
    push(
        "Expr",
        choice(
            "An expression. Every expression carries a declared type.",
            &[],
            vec![
                carried("Literal", r("Value")),
                carried("Bound", r("Identifier")),
                carried(
                    "Field",
                    record(
                        "One field read out of a record.",
                        vec![("base", r("Expr")), ("field", r("Identifier"))],
                    ),
                ),
                carried(
                    "Unary",
                    record(
                        "An operation over one operand.",
                        vec![("op", r("UnaryOp")), ("operand", r("Expr"))],
                    ),
                ),
                carried(
                    "Binary",
                    record(
                        "An operation over two operands.",
                        vec![
                            ("op", r("BinaryOp")),
                            ("left", r("Expr")),
                            ("right", r("Expr")),
                        ],
                    ),
                ),
                carried(
                    "If",
                    record(
                        "A conditional. Both branches carry one type.",
                        vec![
                            ("condition", r("Expr")),
                            ("then_branch", r("Expr")),
                            ("else_branch", r("Expr")),
                        ],
                    ),
                ),
                carried(
                    "Match",
                    record_with_optional(
                        "A match over a sum type, an enumeration or an optional.",
                        vec![
                            ("scrutinee", r("Expr")),
                            ("arms", arr(r("MatchArm"))),
                            ("fallback", nullable(r("Expr"))),
                        ],
                        &["scrutinee", "arms", "fallback"],
                    ),
                ),
                carried(
                    "Let",
                    record(
                        "A local binding.",
                        vec![
                            ("name", r("Identifier")),
                            ("value", r("Expr")),
                            ("body", r("Expr")),
                        ],
                    ),
                ),
                carried(
                    "MakeRecord",
                    record(
                        "Build a record.",
                        vec![
                            ("type_id", r("Identifier")),
                            ("fields", arr(pair(r("Identifier"), r("Expr")))),
                        ],
                    ),
                ),
                carried(
                    "MakeEnumeration",
                    record(
                        "Build an enumeration value.",
                        vec![("type_id", r("Identifier")), ("variant", r("Identifier"))],
                    ),
                ),
                carried(
                    "MakeSum",
                    record_with_optional(
                        "Build a sum value.",
                        vec![
                            ("type_id", r("Identifier")),
                            ("variant", r("Identifier")),
                            ("payload", nullable(r("Expr"))),
                        ],
                        &["type_id", "variant", "payload"],
                    ),
                ),
                carried(
                    "MakeList",
                    record(
                        "Build a list.",
                        vec![("element", r("TypeExpr")), ("items", arr(r("Expr")))],
                    ),
                ),
                carried(
                    "MakeMap",
                    record(
                        "Build a map.",
                        vec![
                            ("key", r("TypeExpr")),
                            ("value", r("TypeExpr")),
                            ("entries", arr(pair(r("Expr"), r("Expr")))),
                        ],
                    ),
                ),
                carried(
                    "MakeOptional",
                    record_with_optional(
                        "Build an optional.",
                        vec![("element", r("TypeExpr")), ("item", nullable(r("Expr")))],
                        &["element", "item"],
                    ),
                ),
                carried(
                    "MakeReference",
                    record(
                        "Build a reference to a record in a collection.",
                        vec![("collection", r("Identifier")), ("key", r("Expr"))],
                    ),
                ),
                carried(
                    "UnwrapOr",
                    record(
                        "The element of a present optional, or a supplied value.",
                        vec![("optional", r("Expr")), ("fallback", r("Expr"))],
                    ),
                ),
                carried("IsPresent", r("Expr")),
                carried("Length", r("Expr")),
                carried(
                    "Contains",
                    record(
                        "Report whether a list holds a value.",
                        vec![("list", r("Expr")), ("item", r("Expr"))],
                    ),
                ),
                carried(
                    "Lookup",
                    record(
                        "Read one value out of a map.",
                        vec![("map", r("Expr")), ("key", r("Expr"))],
                    ),
                ),
                carried(
                    "Rescale",
                    record(
                        "Move a decimal to another scale without losing a digit.",
                        vec![
                            ("value", r("Expr")),
                            (
                                "scale",
                                obj(vec![
                                    ("type", s("integer")),
                                    ("minimum", n(0)),
                                    ("maximum", n(30)),
                                ]),
                            ),
                        ],
                    ),
                ),
                carried(
                    "Call",
                    record(
                        "Apply a named operation the Blueprint supplies.",
                        vec![("function", r("Builtin")), ("args", arr(r("Expr")))],
                    ),
                ),
                carried("Fold", r("FoldExpr")),
                carried(
                    "Attempt",
                    record(
                        "Run an expression and fall back to a compensation on a recoverable fault.",
                        vec![("body", r("Expr")), ("compensation", r("Expr"))],
                    ),
                ),
            ],
        ),
    );

    push(
        "FieldDef",
        record(
            "One field of a record.",
            vec![
                ("name", r("Identifier")),
                ("ty", r("TypeExpr")),
                ("summary", sentence("One plain sentence stating what the field holds.")),
            ],
        ),
    );
    push(
        "RecordDef",
        record(
            "A record type. A record holds a fixed set of named fields.",
            vec![
                ("id", r("Identifier")),
                (
                    "fields",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("FieldDef")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("summary", sentence("One plain sentence stating what the record holds.")),
            ],
        ),
    );
    push(
        "EnumerationDef",
        record(
            "An enumeration type. An enumeration holds one name out of a fixed set.",
            vec![
                ("id", r("Identifier")),
                (
                    "variants",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("summary", sentence("One plain sentence stating what the enumeration holds.")),
            ],
        ),
    );
    push(
        "SumVariant",
        record_with_optional(
            "One alternative of a sum type.",
            vec![
                ("name", r("Identifier")),
                ("payload", nullable(r("TypeExpr"))),
            ],
            &["name", "payload"],
        ),
    );
    push(
        "SumDef",
        record(
            "A sum type. A sum type holds one value out of several named alternatives.",
            vec![
                ("id", r("Identifier")),
                (
                    "variants",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("SumVariant")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("summary", sentence("One plain sentence stating what the sum type holds.")),
            ],
        ),
    );
    push(
        "TypeDef",
        choice(
            "A declared type.",
            &[],
            vec![
                carried("Record", r("RecordDef")),
                carried("Enumeration", r("EnumerationDef")),
                carried("Sum", r("SumDef")),
            ],
        ),
    );

    push(
        "PrivacyClass",
        choice(
            "The visibility class of a collection.",
            &["Open", "HashOnly"],
            vec![
                carried(
                    "Encrypted",
                    record(
                        "The Blockchain holds an encrypted payload.",
                        vec![(
                            "key_holders",
                            obj(vec![
                                ("type", s("array")),
                                ("items", r("Identifier")),
                                ("minItems", n(1)),
                            ]),
                        )],
                    ),
                ),
                carried(
                    "External",
                    record(
                        "The Blockchain holds a commitment and a locator.",
                        vec![
                            ("commitment", r("FieldPath")),
                            ("locator", r("FieldPath")),
                        ],
                    ),
                ),
            ],
        ),
    );
    push(
        "RetentionPeriod",
        choice(
            "The span a record stays in the collection.",
            &["Indefinite"],
            vec![carried(
                "Milliseconds",
                counted("The span a record stays, in milliseconds.", 1),
            )],
        ),
    );
    push(
        "RetentionAction",
        choice(
            "The action retention expiry runs.",
            &["Review", "Erase"],
            vec![
                carried(
                    "Anonymise",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("FieldPath")),
                        ("minItems", n(1)),
                    ]),
                ),
                carried("MoveTo", r("Identifier")),
            ],
        ),
    );
    push(
        "Retention",
        record(
            "The retention rule of a collection.",
            vec![
                ("period", r("RetentionPeriod")),
                ("on_expiry", r("RetentionAction")),
            ],
        ),
    );
    push(
        "PurposeDeclaration",
        record(
            "The declared purpose of a collection. A read outside the declared purpose is denied and recorded.",
            vec![
                ("statement", sentence("One plain sentence stating why the deployment holds this data.")),
                (
                    "purposes",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
            ],
        ),
    );
    push(
        "Index",
        record(
            "A secondary index over a collection.",
            vec![
                ("name", r("Identifier")),
                (
                    "fields",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("FieldPath")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("unique", truth("A unique index admits one record per key.")),
            ],
        ),
    );
    push(
        "Collection",
        record(
            "A collection holding records of one declared type.",
            vec![
                ("id", r("Identifier")),
                ("record_type", r("Identifier")),
                ("primary_key", r("FieldPath")),
                ("indexes", arr(r("Index"))),
                ("privacy", r("PrivacyClass")),
                ("retention", r("Retention")),
                ("purpose", r("PurposeDeclaration")),
                ("summary", sentence("One plain sentence stating what the collection holds.")),
            ],
        ),
    );

    push(
        "OrganisationUnit",
        record_with_optional(
            "One part of an organisation.",
            vec![
                ("id", r("Identifier")),
                ("parent", nullable(r("Identifier"))),
                ("title", sentence("The name a person reads on screen.")),
                ("summary", sentence("One plain sentence stating what the unit does.")),
            ],
            &["id", "parent", "title", "summary"],
        ),
    );
    push(
        "RoleDef",
        record_with_optional(
            "A named capacity a person holds.",
            vec![
                ("id", r("Identifier")),
                ("unit", nullable(r("Identifier"))),
                ("requires", arr(r("Identifier"))),
                ("title", sentence("The name a person reads on screen.")),
                ("summary", sentence("One plain sentence stating what the role does.")),
            ],
            &["id", "unit", "requires", "title", "summary"],
        ),
    );
    push(
        "CredentialIssuer",
        choice(
            "The party that issues a credential.",
            &[],
            vec![
                carried("Unit", r("Identifier")),
                carried("Role", r("Identifier")),
                carried(
                    "External",
                    record(
                        "A named authority outside this deployment.",
                        vec![("authority", sentence("The name of the authority."))],
                    ),
                ),
            ],
        ),
    );
    push(
        "CredentialValidity",
        choice(
            "The span a credential stays valid.",
            &["UntilRevoked"],
            vec![carried(
                "Milliseconds",
                counted("The span the credential stays valid, in milliseconds.", 1),
            )],
        ),
    );
    push(
        "CredentialDef",
        record(
            "An attestation a role requires.",
            vec![
                ("id", r("Identifier")),
                ("issuer", r("CredentialIssuer")),
                ("validity", r("CredentialValidity")),
                ("attests", sentence("One plain sentence stating what the credential attests.")),
            ],
        ),
    );
    push(
        "DelegationRule",
        record_with_optional(
            "A rule letting one role pass a scope to another role.",
            vec![
                ("id", r("Identifier")),
                ("from", r("Identifier")),
                ("to", r("Identifier")),
                (
                    "scope",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("maximum_millis", counted("The longest span a delegation lasts.", 1)),
                ("requires", nullable(r("Identifier"))),
            ],
            &["id", "from", "to", "scope", "maximum_millis", "requires"],
        ),
    );
    push(
        "Identities",
        record(
            "The organisation tree, the roles, the credentials and the delegation rules.",
            vec![
                ("units", arr(r("OrganisationUnit"))),
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("RoleDef")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("credentials", arr(r("CredentialDef"))),
                ("delegations", arr(r("DelegationRule"))),
            ],
        ),
    );

    push(
        "Capability",
        choice(
            "One capability over a collection.",
            &["Read", "Insert", "Update", "Delete"],
            vec![carried("Invoke", r("Identifier"))],
        ),
    );
    push(
        "FieldQualification",
        choice(
            "The fields one entry reaches.",
            &["EveryField"],
            vec![
                carried(
                    "Only",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("FieldPath")),
                        ("minItems", n(1)),
                    ]),
                ),
                carried(
                    "Apart",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("FieldPath")),
                        ("minItems", n(1)),
                    ]),
                ),
            ],
        ),
    );
    push(
        "RowQualification",
        choice(
            "The rows one entry reaches.",
            &["EveryRow"],
            vec![carried("Where", r("Expr"))],
        ),
    );
    push(
        "CapabilityEntry",
        record(
            "One entry of the capability matrix.",
            vec![
                ("role", r("Identifier")),
                ("collection", r("Identifier")),
                ("capability", r("Capability")),
                ("fields", r("FieldQualification")),
                ("rows", r("RowQualification")),
                ("purposes", arr(r("Identifier"))),
            ],
        ),
    );
    push(
        "Permissions",
        record(
            "The capability matrix.",
            vec![(
                "entries",
                obj(vec![
                    ("type", s("array")),
                    ("items", r("CapabilityEntry")),
                    ("minItems", n(1)),
                ]),
            )],
        ),
    );

    push(
        "Invariant",
        record(
            "A condition holding at all times over one collection.",
            vec![
                ("id", r("Identifier")),
                ("collection", r("Identifier")),
                ("holds", r("Expr")),
                ("problem", sentence("One sentence a person reads when the rule declines a change.")),
                ("fix", sentence("One sentence stating what a person does next.")),
            ],
        ),
    );
    push(
        "RuleInput",
        record(
            "One input a precondition reads.",
            vec![("name", r("Identifier")), ("ty", r("TypeExpr"))],
        ),
    );
    push(
        "Precondition",
        record(
            "A condition gating one action.",
            vec![
                ("id", r("Identifier")),
                ("inputs", arr(r("RuleInput"))),
                ("holds", r("Expr")),
                ("problem", sentence("One sentence a person reads when the rule declines the action.")),
                ("fix", sentence("One sentence stating what a person does next.")),
            ],
        ),
    );
    push(
        "Rules",
        record(
            "The invariants and the preconditions.",
            vec![
                ("invariants", arr(r("Invariant"))),
                ("preconditions", arr(r("Precondition"))),
            ],
        ),
    );

    push(
        "TransactionClass",
        names(
            "The class of a transaction. The class sets the agreement threshold.",
            &["Routine", "Financial", "Structural", "Constitutional"],
        ),
    );
    push(
        "Param",
        record(
            "One parameter of an action or a reusable block.",
            vec![
                ("name", r("Identifier")),
                ("ty", r("TypeExpr")),
                ("summary", sentence("One plain sentence stating what the parameter holds.")),
            ],
        ),
    );
    push(
        "KeySelector",
        choice(
            "The span of keys a read or a write touches.",
            &["Full"],
            vec![
                carried("Exact", r("Expr")),
                carried("Prefix", r("Expr")),
            ],
        ),
    );
    push(
        "Effect",
        choice(
            "One step of an effect sequence.",
            &[],
            vec![
                carried(
                    "Insert",
                    record(
                        "Add one record.",
                        vec![
                            ("collection", r("Identifier")),
                            ("key", r("KeySelector")),
                            ("record", r("Expr")),
                        ],
                    ),
                ),
                carried(
                    "Update",
                    record(
                        "Change one field of the records the key reaches.",
                        vec![
                            ("collection", r("Identifier")),
                            ("key", r("KeySelector")),
                            ("field", r("FieldPath")),
                            ("value", r("Expr")),
                        ],
                    ),
                ),
                carried(
                    "Delete",
                    record(
                        "Remove the records the key reaches.",
                        vec![
                            ("collection", r("Identifier")),
                            ("key", r("KeySelector")),
                        ],
                    ),
                ),
                carried(
                    "Read",
                    record(
                        "Read one record and bind it to a name for the later steps.",
                        vec![
                            ("collection", r("Identifier")),
                            ("key", r("KeySelector")),
                            ("binding", r("Identifier")),
                            ("element", r("TypeExpr")),
                        ],
                    ),
                ),
                carried(
                    "Emit",
                    record(
                        "Write one event to the audit log.",
                        vec![("event", r("Identifier")), ("payload", r("Expr"))],
                    ),
                ),
                carried(
                    "Transition",
                    record(
                        "Move one record to another workflow state.",
                        vec![
                            ("workflow", r("Identifier")),
                            ("collection", r("Identifier")),
                            ("key", r("KeySelector")),
                            ("to", r("Identifier")),
                        ],
                    ),
                ),
                carried(
                    "RunBlock",
                    record(
                        "Run a reusable block.",
                        vec![
                            ("block", r("Identifier")),
                            ("arguments", arr(pair(r("Identifier"), r("Expr")))),
                        ],
                    ),
                ),
            ],
        ),
    );
    push(
        "Action",
        record(
            "An action a caller runs.",
            vec![
                ("id", r("Identifier")),
                ("params", arr(r("Param"))),
                ("preconditions", arr(r("Identifier"))),
                ("effects", arr(r("Effect"))),
                ("compensation", arr(r("Effect"))),
                ("emits", arr(r("Identifier"))),
                ("class", r("TransactionClass")),
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("summary", sentence("One plain sentence stating what the action does.")),
            ],
        ),
    );

    push(
        "CyclePolicy",
        names(
            "Whether a loop between states is lawful in this workflow.",
            &["Forbid", "Allow"],
        ),
    );
    push(
        "WorkflowState",
        record(
            "One state of a workflow.",
            vec![
                ("id", r("Identifier")),
                ("final_state", truth("A record in a final state stays there.")),
                ("title", sentence("The name a person reads on screen.")),
                ("summary", sentence("One plain sentence stating what the state means.")),
            ],
        ),
    );
    push(
        "Transition",
        record_with_optional(
            "One guarded transition between two states.",
            vec![
                ("id", r("Identifier")),
                ("from", r("Identifier")),
                ("to", r("Identifier")),
                ("guard", nullable(r("Expr"))),
                ("action", nullable(r("Identifier"))),
                ("roles", arr(r("Identifier"))),
            ],
            &["id", "from", "to", "guard", "action", "roles"],
        ),
    );
    push(
        "TimerAction",
        choice(
            "The step a timer or a deadline runs.",
            &[],
            vec![
                carried("MoveTo", r("Identifier")),
                carried("Run", r("Identifier")),
                carried("Emit", r("Identifier")),
            ],
        ),
    );
    push(
        "Timer",
        record(
            "A timer firing after a span inside one state.",
            vec![
                ("id", r("Identifier")),
                ("state", r("Identifier")),
                ("after_millis", counted("The span from entry to firing.", 1)),
                ("on_expiry", r("TimerAction")),
            ],
        ),
    );
    push(
        "Deadline",
        record(
            "A deadline falling due at a computed time.",
            vec![
                ("id", r("Identifier")),
                ("state", r("Identifier")),
                ("due", r("Expr")),
                ("on_miss", r("TimerAction")),
            ],
        ),
    );
    push(
        "Workflow",
        record(
            "A workflow driving records through states.",
            vec![
                ("id", r("Identifier")),
                ("collection", r("Identifier")),
                ("state_field", r("FieldPath")),
                (
                    "states",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("WorkflowState")),
                        ("minItems", n(2)),
                    ]),
                ),
                ("initial", r("Identifier")),
                (
                    "transitions",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Transition")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("timers", arr(r("Timer"))),
                ("deadlines", arr(r("Deadline"))),
                ("cycles", r("CyclePolicy")),
                ("summary", sentence("One plain sentence stating what the workflow drives.")),
            ],
        ),
    );

    push(
        "AuditPolicy",
        names("Whether the audit log keeps the event.", &["Keep", "Expire"]),
    );
    push(
        "DeliveryTarget",
        choice(
            "The party that reads an event.",
            &[],
            vec![
                carried("Workflow", r("Identifier")),
                carried("View", r("Identifier")),
                carried("Integration", r("Identifier")),
            ],
        ),
    );
    push(
        "Subscriber",
        record(
            "One party reading an event.",
            vec![
                ("target", r("DeliveryTarget")),
                ("summary", sentence("One plain sentence stating what the subscriber does with the event.")),
            ],
        ),
    );
    push(
        "EventDef",
        record(
            "A typed event written to the audit log.",
            vec![
                ("id", r("Identifier")),
                ("payload", r("Identifier")),
                ("audit", r("AuditPolicy")),
                ("subscribers", arr(r("Subscriber"))),
                ("summary", sentence("One plain sentence stating what the event records.")),
            ],
        ),
    );

    push(
        "JoinKind",
        names("The shape of a join.", &["Matched", "Optional"]),
    );
    push(
        "Join",
        record(
            "One join a view follows.",
            vec![
                ("collection", r("Identifier")),
                ("alias", r("Identifier")),
                ("left", r("FieldPath")),
                ("right", r("FieldPath")),
                ("kind", r("JoinKind")),
            ],
        ),
    );
    push(
        "Projection",
        record(
            "One projected field.",
            vec![("name", r("Identifier")), ("value", r("Expr"))],
        ),
    );
    push(
        "SortDirection",
        names("The direction a sort runs.", &["Ascending", "Descending"]),
    );
    push(
        "SortKey",
        record(
            "One sort key.",
            vec![("name", r("Identifier")), ("direction", r("SortDirection"))],
        ),
    );
    push(
        "AggregateFunction",
        names(
            "The operation an aggregate runs over a group.",
            &["Count", "Total", "Smallest", "Largest"],
        ),
    );
    push(
        "Aggregate",
        record_with_optional(
            "One aggregate a view computes.",
            vec![
                ("name", r("Identifier")),
                ("function", r("AggregateFunction")),
                ("value", nullable(r("Expr"))),
            ],
            &["name", "function", "value"],
        ),
    );
    push(
        "View",
        record_with_optional(
            "A declarative query. A view reads state and changes nothing.",
            vec![
                ("id", r("Identifier")),
                ("source", r("Identifier")),
                ("joins", arr(r("Join"))),
                ("projection", arr(r("Projection"))),
                ("filter", nullable(r("Expr"))),
                ("group_by", arr(r("Identifier"))),
                ("aggregates", arr(r("Aggregate"))),
                ("sort", arr(r("SortKey"))),
                ("limit", nullable(counted("The largest count of rows the view returns.", 1))),
                (
                    "purposes",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("summary", sentence("One plain sentence stating what the view answers.")),
            ],
            &[
                "id",
                "source",
                "joins",
                "projection",
                "filter",
                "group_by",
                "aggregates",
                "sort",
                "limit",
                "purposes",
                "roles",
                "summary",
            ],
        ),
    );

    push(
        "AttestationRequirement",
        record(
            "The proof an integration presents before the Blockchain accepts its work.",
            vec![
                ("signers", counted("The count of signatures required.", 1)),
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("credentials", arr(r("Identifier"))),
            ],
        ),
    );
    push(
        "Feed",
        record(
            "A feed carrying values into a collection.",
            vec![
                ("id", r("Identifier")),
                ("target", r("Identifier")),
                ("record_type", r("Identifier")),
                ("refresh_millis", counted("The span between two reads.", 1)),
                ("attestation", r("AttestationRequirement")),
                ("summary", sentence("One plain sentence stating what the feed carries.")),
            ],
        ),
    );
    push(
        "BridgeDirection",
        names(
            "The direction records move across a bridge endpoint.",
            &["Inbound", "Outbound", "BothWays"],
        ),
    );
    push(
        "BridgeEndpoint",
        record(
            "An endpoint moving records between this deployment and a named network.",
            vec![
                ("id", r("Identifier")),
                ("network", sentence("The name the operator gave the network on the other side.")),
                ("direction", r("BridgeDirection")),
                ("collection", r("Identifier")),
                ("attestation", r("AttestationRequirement")),
                ("summary", sentence("One plain sentence stating what the endpoint moves.")),
            ],
        ),
    );
    push(
        "ExternalTrigger",
        record(
            "A trigger letting an outside system run one action.",
            vec![
                ("id", r("Identifier")),
                ("action", r("Identifier")),
                ("attestation", r("AttestationRequirement")),
                ("summary", sentence("One plain sentence stating what the trigger starts.")),
            ],
        ),
    );
    push(
        "Integrations",
        record(
            "The feeds, the bridge endpoints and the external triggers.",
            vec![
                ("feeds", arr(r("Feed"))),
                ("bridges", arr(r("BridgeEndpoint"))),
                ("triggers", arr(r("ExternalTrigger"))),
            ],
        ),
    );

    push(
        "UpgradeAuthority",
        record(
            "The parties that change the deployment.",
            vec![
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                (
                    "threshold_percent",
                    obj(vec![
                        ("type", s("integer")),
                        ("minimum", n(51)),
                        ("maximum", n(100)),
                    ]),
                ),
                ("minimum_voters", counted("The count of approvals a change collects.", 1)),
            ],
        ),
    );
    push(
        "QuorumRule",
        record(
            "The agreement one class of change requires.",
            vec![
                ("class", r("TransactionClass")),
                (
                    "threshold_percent",
                    obj(vec![
                        ("type", s("integer")),
                        ("minimum", n(51)),
                        ("maximum", n(100)),
                    ]),
                ),
                ("minimum_voters", counted("The count of validators that vote.", 1)),
                ("delay_millis", counted("The span between approval and effect.", 0)),
            ],
        ),
    );
    push(
        "ChangeStage",
        record(
            "One stage of a change process.",
            vec![
                ("id", r("Identifier")),
                (
                    "roles",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("Identifier")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("duration_millis", counted("The longest span the stage runs for.", 1)),
                ("summary", sentence("One plain sentence stating what happens in the stage.")),
            ],
        ),
    );
    push(
        "ChangeProcess",
        record(
            "The stages a change passes through.",
            vec![(
                "stages",
                obj(vec![
                    ("type", s("array")),
                    ("items", r("ChangeStage")),
                    ("minItems", n(1)),
                ]),
            )],
        ),
    );
    push(
        "Governance",
        record(
            "The upgrade authority, the quorum rules and the change process.",
            vec![
                ("authority", r("UpgradeAuthority")),
                (
                    "quorum",
                    obj(vec![
                        ("type", s("array")),
                        ("items", r("QuorumRule")),
                        ("minItems", n(1)),
                    ]),
                ),
                ("process", r("ChangeProcess")),
            ],
        ),
    );

    push(
        "UnitOfAccount",
        record(
            "The currency or the measure a deployment records.",
            vec![
                ("code", sentence("The short code a person reads.")),
                ("name", sentence("The name a person reads.")),
                (
                    "scale",
                    obj(vec![
                        ("type", s("integer")),
                        ("minimum", n(0)),
                        ("maximum", n(30)),
                    ]),
                ),
            ],
        ),
    );
    push(
        "BalanceSheet",
        record(
            "A collection holding balances.",
            vec![
                ("collection", r("Identifier")),
                ("amount_field", r("FieldPath")),
                ("holder_field", r("FieldPath")),
                ("summary", sentence("One plain sentence stating what the sheet records.")),
            ],
        ),
    );
    push(
        "StandardAmount",
        record(
            "A standard amount one action records.",
            vec![
                ("action", r("Identifier")),
                ("amount", r("Decimal")),
                ("summary", sentence("One plain sentence stating what the amount covers.")),
            ],
        ),
    );
    push(
        "AmountLimit",
        record(
            "A ceiling on the amount one role moves in one action.",
            vec![
                ("role", r("Identifier")),
                ("maximum", r("Decimal")),
                ("summary", sentence("One plain sentence stating why the ceiling holds.")),
            ],
        ),
    );
    push(
        "Economics",
        record(
            "The unit of account. A deployment that records no amounts leaves this section out.",
            vec![
                ("unit", r("UnitOfAccount")),
                ("sheets", arr(r("BalanceSheet"))),
                ("amounts", arr(r("StandardAmount"))),
                ("limits", arr(r("AmountLimit"))),
            ],
        ),
    );

    push(
        "BlockOutput",
        record(
            "One typed output a reusable block produces.",
            vec![
                ("name", r("Identifier")),
                ("ty", r("TypeExpr")),
                ("value", r("Expr")),
            ],
        ),
    );
    push(
        "Block",
        record(
            "A reusable block grouping a subgraph under a name.",
            vec![
                ("id", r("Identifier")),
                ("inputs", arr(r("Param"))),
                ("outputs", arr(r("BlockOutput"))),
                ("effects", arr(r("Effect"))),
                ("summary", sentence("One plain sentence stating what the block does.")),
            ],
        ),
    );

    defs
}

// ---------------------------------------------------------------------------
// Readable output
// ---------------------------------------------------------------------------

/// Lay the compact text out over lines so a person reads the document.
///
/// The member order stays exactly the order the writer produced, so two runs
/// still yield the same bytes.
fn pretty(value: &Json) -> String {
    let compact = value.to_text();
    let mut out = String::with_capacity(compact.len() * 2);
    let mut depth = 0usize;
    let mut inside_text = false;
    let mut escaped = false;
    for character in compact.chars() {
        if inside_text {
            out.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                inside_text = false;
            }
            continue;
        }
        match character {
            '"' => {
                inside_text = true;
                out.push(character);
            }
            '{' | '[' => {
                depth += 1;
                out.push(character);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            '}' | ']' => {
                depth = depth.saturating_sub(1);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
                out.push(character);
            }
            ',' => {
                out.push(character);
                out.push('\n');
                out.push_str(&"  ".repeat(depth));
            }
            ':' => {
                out.push(character);
                out.push(' ');
            }
            other => out.push(other),
        }
    }
    out.push('\n');
    out
}
