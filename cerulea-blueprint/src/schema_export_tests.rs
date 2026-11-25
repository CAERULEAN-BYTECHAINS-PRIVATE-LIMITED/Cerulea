//! Behavioural tests for the JSON Schema export over a complete Blueprint.
//!
//! The export writes its own JSON text. The tests parse that text with an
//! independent JSON reader and assert the structure the interface layer
//! reads.

use crate::{
    collections::PrivacyClass,
    schema_export::{event_schema, privacy_label, view_schema, Json, SCHEMA_DIALECT},
    test_fixture::land_registry,
    types::{CollectionId, TypeRegistry},
    Path,
};
use alloc::{string::String, vec};
use serde_json::Value as Parsed;

#[test]
fn every_json_value_writes_as_valid_json_text() {
    let value = Json::Array(vec![
        Json::Null,
        Json::Bool(true),
        Json::Bool(false),
        Json::Number(-170_141_183_460_469_231_731_687_303_715_884_105_728),
        Json::text("quote \" slash \\ line\n return\r tab\t bell\u{7} end"),
        Json::object(vec![
            ("empty", Json::Array(vec![])),
            ("nested", Json::object(vec![])),
        ]),
    ]);
    let text = value.to_text();
    assert_eq!(
        text,
        "[null,true,false,-170141183460469231731687303715884105728,\"quote \\\" slash \\\\ line\\n return\\r tab\\t bell\\u0007 end\",{\"empty\":[],\"nested\":{}}]"
    );
    let parsed: Parsed = serde_json::from_str(&text).expect("the text is valid JSON");
    assert_eq!(
        parsed[4],
        Parsed::String(String::from(
            "quote \" slash \\ line\n return\r tab\t bell\u{7} end"
        ))
    );
    let mut out = String::from("prefix:");
    Json::Null.write(&mut out);
    assert_eq!(out, "prefix:null");
}

#[test]
fn the_document_carries_every_section_the_interface_layer_reads() {
    let blueprint = land_registry();
    let text = blueprint.json_schema();
    let document: Parsed = serde_json::from_str(&text).expect("the document is valid JSON");
    assert_eq!(document["$schema"], SCHEMA_DIALECT);
    assert_eq!(
        document["$id"],
        Parsed::String(alloc::format!("cerulea:blueprint:{}", blueprint.id()))
    );
    assert_eq!(document["title"], "land registry");
    assert_eq!(document["x-cerulea"]["carriesEconomics"], true);
    assert_eq!(document["x-cerulea"]["revision"], 1);
    assert_eq!(document["x-cerulea"]["schemaVersion"], "1.0.0");
    let stage = &document["$defs"]["stage"];
    assert_eq!(stage["enum"], serde_json::json!(["draft", "recorded"]));
    assert_eq!(
        document["collections"]["parcel"]["privacy"],
        "encrypted payload"
    );
    assert_eq!(
        document["actions"]["record_parcel"]["parameters"]["required"],
        serde_json::json!(["survey_number", "area"])
    );
    let parcels = &document["views"]["parcels"];
    assert_eq!(parcels["source"], "parcel");
    assert_eq!(parcels["roles"], serde_json::json!(["clerk", "registrar"]));
    assert_eq!(
        parcels["row"]["required"],
        serde_json::json!(["survey_number", "total_area"])
    );
    assert_eq!(
        parcels["row"]["properties"]["survey_number"]["type"],
        "string"
    );
    let fees = &document["views"]["fees"];
    assert_eq!(fees["row"]["required"], serde_json::json!(["fee_count"]));
    assert_eq!(
        document["events"]["recorded"]["payload"]["$ref"],
        "#/$defs/parcel"
    );
    assert_eq!(
        document["events"]["stamped"]["description"],
        "The registry stamped a fee."
    );
    let stamp = &document["blocks"]["stamp"];
    assert_eq!(stamp["description"], "The block writes one ledger line.");
    assert_eq!(
        stamp["inputs"]["properties"]["note"]["description"],
        "The note."
    );
    assert_eq!(stamp["outputs"]["properties"]["stamped"]["type"], "string");
    assert_eq!(
        document["blocks"]["clear"]["outputs"]["properties"],
        serde_json::json!({})
    );
}

#[test]
fn the_view_and_event_schemas_stand_alone() {
    let blueprint = land_registry();
    let (types, _) = TypeRegistry::build(&blueprint.types, &Path::root());
    let (collections, _) =
        crate::collections::CollectionRegistry::build(&blueprint.collections, &Path::root());
    let view = view_schema(&blueprint.views[1], &types, &collections);
    let parsed: Parsed = serde_json::from_str(&view.to_text()).expect("valid JSON");
    assert_eq!(parsed["source"], "ledger");
    assert_eq!(parsed["row"]["properties"]["fee_count"]["type"], "integer");
    let event = event_schema(&blueprint.events[1]);
    let parsed: Parsed = serde_json::from_str(&event.to_text()).expect("valid JSON");
    assert_eq!(parsed["payload"]["$ref"], "#/$defs/ledger");
    assert_eq!(privacy_label(&PrivacyClass::HashOnly), "hash only");
    assert_eq!(
        privacy_label(&PrivacyClass::Encrypted {
            key_holders: vec![]
        }),
        "encrypted payload"
    );
    assert!(collections.contains(&CollectionId::new("ledger")));
}
