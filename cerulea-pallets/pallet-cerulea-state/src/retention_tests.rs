//! The tests of the retention sweep and the version history.
//!
//! The suite drives the four expiry actions through the block hook. A test
//! therefore sees what the Blockchain does at the block the declared span
//! runs out. Each action reaches its own collection, so one sweep of one
//! block reaches one action.

use crate::{
    mock::*,
    retention::{blank_like, clear_field, set_value_at},
    Error, History, Meta, Pallet, RecordCount, RecordStore, Records, RetentionCursor,
    RetentionSettled, RetentionWatch, WrittenAt,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration, RetentionAction, RetentionPeriod},
    types::{
        CollectionId, Decimal, EnumerationValue, FieldDef, FieldName, FieldPath, Hash32, Identity,
        RecordDef, RecordRef, SumValue, Timestamp, TypeDef, TypeExpr, TypeId, Value, VariantName,
    },
};
use codec::Encode;
use frame_support::{
    assert_ok,
    traits::{ConstU32, Hooks},
    BoundedVec,
};
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest},
    primitives::{name_of, CollectionKey, DeploymentId, RecordKey},
};

const DEPLOYMENT: DeploymentId = [4u8; 32];
const STEWARD: u64 = 1;
const SPAN: u64 = 1_000;
const PLANTED: u64 = 2_000_000;

// The record shapes the suite writes.

fn detail_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("detail"),
        fields: vec![
            FieldDef::new("secret", TypeExpr::Text, "The part the sweep clears."),
            FieldDef::new("kept", TypeExpr::Text, "The part the sweep leaves."),
        ],
        summary: "The nested part of one note.".to_string(),
    })
}

fn note_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("note"),
        fields: vec![
            FieldDef::new("id", TypeExpr::Text, "The key of the note."),
            FieldDef::new("body", TypeExpr::Text, "The text of the note."),
            FieldDef::new("count", TypeExpr::Integer, "A whole number the note holds."),
            FieldDef::new(
                "detail",
                TypeExpr::Record(TypeId::new("detail")),
                "The nested part of the note.",
            ),
        ],
        summary: "One note in the register.".to_string(),
    })
}

fn note(id: &str, body: &str, count: i128, secret: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("note"),
        vec![
            (FieldName::new("id"), Value::Text(id.to_string())),
            (FieldName::new("body"), Value::Text(body.to_string())),
            (FieldName::new("count"), Value::Integer(count)),
            (
                FieldName::new("detail"),
                Value::record(
                    TypeId::new("detail"),
                    vec![
                        (FieldName::new("secret"), Value::Text(secret.to_string())),
                        (FieldName::new("kept"), Value::Text("kept".to_string())),
                    ],
                ),
            ),
        ],
    );
    value.canonicalise();
    value
}

fn collection_with(name: &str, period: RetentionPeriod, action: RetentionAction) -> Collection {
    let mut held = Collection::new(name, "note", "id", "The register of notes.");
    held.purpose = PurposeDeclaration::new(
        "The register keeps notes for the span the rule declares.",
        &["registration"],
    );
    held.retention.period = period;
    held.retention.on_expiry = action;
    held
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn name(text: &str) -> BoundedVec<u8, ConstU32<128>> {
    BoundedVec::try_from(text.as_bytes().to_vec()).expect("the name fits")
}

fn slot_of(text: &str) -> CollectionKey {
    CollectionKey::new(DEPLOYMENT, name_of(text).expect("the name fits"))
}

fn key_of(id: &str) -> RecordKey {
    let record = note(id, "body", 1, "secret");
    let shape = collection_with(
        "shape",
        RetentionPeriod::Indefinite,
        RetentionAction::Review,
    );
    let bytes = crate::key_bytes_of(&shape, &record).expect("the record carries its key");
    BoundedVec::try_from(bytes).expect("the key sits inside the bound")
}

fn register(collection: &Collection) {
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(collection.encode())
    ));
}

fn install_types() {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(detail_type().encode())
    ));
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(note_type().encode())
    ));
}

fn insert_into(collection: &str, record: &Value) {
    assert_ok!(State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name(collection),
        bounded(record.canonical_bytes())
    ));
}

/// Move the chain on one block and run the block hook of the state module.
fn next_block() {
    let next = System::block_number() + 1;
    System::set_block_number(next);
    System::reset_events();
    Pallet::<Test>::on_initialize(next);
}

/// The events the state module raised in the present block.
fn raised() -> Vec<crate::Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::State(held) => Some(held),
            _ => None,
        })
        .collect()
}

/// Plant one collection, write one record and move the clock past the span.
fn plant(collection: &Collection, record: &Value) {
    install_types();
    register(collection);
    set_clock(PLANTED);
    insert_into(collection.id.as_str(), record);
    set_clock(PLANTED + SPAN);
}

// The blank value of every shape.

#[test]
fn a_cleared_field_takes_the_blank_value_of_its_own_shape() {
    assert_eq!(blank_like(&Value::Bool(true)), Value::Bool(false));
    assert_eq!(blank_like(&Value::Integer(42)), Value::Integer(0));
    assert_eq!(
        blank_like(&Value::Decimal(Decimal::new(1_234, 2))),
        Value::Decimal(Decimal::zero(2))
    );
    assert_eq!(
        blank_like(&Value::Text("Anitha".to_string())),
        Value::Text(String::new())
    );
    assert_eq!(
        blank_like(&Value::Bytes(vec![1, 2, 3])),
        Value::Bytes(Vec::new())
    );
    assert_eq!(
        blank_like(&Value::Timestamp(Timestamp::from_millis(99))),
        Value::Timestamp(Timestamp::from_millis(0))
    );
    assert_eq!(
        blank_like(&Value::Identity(Identity([7u8; 32]))),
        Value::Identity(Identity([0u8; 32]))
    );
    assert_eq!(
        blank_like(&Value::Hash(Hash32([7u8; 32]))),
        Value::Hash(Hash32([0u8; 32]))
    );
}

#[test]
fn a_cleared_decimal_keeps_the_scale_the_schema_registered() {
    match blank_like(&Value::Decimal(Decimal::new(-500, 4))) {
        Value::Decimal(number) => {
            assert_eq!(number.scale, 4);
            assert!(number.is_zero());
        }
        other => panic!("the blank of a decimal stays a decimal, and this reads {other:?}"),
    }
}

#[test]
fn a_cleared_container_keeps_its_element_type_and_loses_its_content() {
    let list = Value::list(
        TypeExpr::Integer,
        vec![Value::Integer(1), Value::Integer(2)],
    );
    assert_eq!(
        blank_like(&list),
        Value::list(TypeExpr::Integer, Vec::new())
    );

    let map = Value::map(
        TypeExpr::Text,
        TypeExpr::Integer,
        vec![(Value::Text("a".to_string()), Value::Integer(1))],
    );
    assert_eq!(
        blank_like(&map),
        Value::map(TypeExpr::Text, TypeExpr::Integer, Vec::new())
    );

    let present = Value::some(TypeExpr::Text, Value::Text("held".to_string()));
    assert_eq!(blank_like(&present), Value::none(TypeExpr::Text));
}

#[test]
fn a_cleared_record_clears_every_field_it_nests() {
    let blanked = blank_like(&note("one", "body", 9, "secret"));
    assert_eq!(
        crate::value_at(&blanked, &FieldPath::parse("body")),
        Some(Value::Text(String::new()))
    );
    assert_eq!(
        crate::value_at(&blanked, &FieldPath::parse("count")),
        Some(Value::Integer(0))
    );
    assert_eq!(
        crate::value_at(&blanked, &FieldPath::parse("detail.secret")),
        Some(Value::Text(String::new()))
    );
    assert_eq!(
        crate::value_at(&blanked, &FieldPath::parse("detail.kept")),
        Some(Value::Text(String::new()))
    );
}

#[test]
fn a_value_naming_one_alternative_keeps_the_name_the_schema_admits() {
    let choice = Value::Enumeration(EnumerationValue {
        type_id: TypeId::new("stage"),
        variant: VariantName::new("filed"),
    });
    assert_eq!(blank_like(&choice), choice);

    let sum = Value::Sum(SumValue {
        type_id: TypeId::new("outcome"),
        variant: VariantName::new("granted"),
        payload: Some(Box::new(Value::Integer(5))),
    });
    assert_eq!(blank_like(&sum), sum);

    let pointer = Value::Reference(RecordRef {
        collection: CollectionId::new("note"),
        key: Box::new(Value::Text("one".to_string())),
    });
    assert_eq!(blank_like(&pointer), pointer);
}

// Writing one field of a record value.

#[test]
fn an_empty_path_replaces_the_whole_value() {
    let mut held = Value::Integer(1);
    assert!(set_value_at(
        &mut held,
        &FieldPath::parse(""),
        Value::Integer(9)
    ));
    assert_eq!(held, Value::Integer(9));
}

#[test]
fn a_path_reaching_a_nested_field_writes_that_field_alone() {
    let mut held = note("one", "body", 3, "secret");
    assert!(set_value_at(
        &mut held,
        &FieldPath::parse("detail.secret"),
        Value::Text("changed".to_string())
    ));
    assert_eq!(
        crate::value_at(&held, &FieldPath::parse("detail.secret")),
        Some(Value::Text("changed".to_string()))
    );
    assert_eq!(
        crate::value_at(&held, &FieldPath::parse("detail.kept")),
        Some(Value::Text("kept".to_string()))
    );
    assert_eq!(
        crate::value_at(&held, &FieldPath::parse("body")),
        Some(Value::Text("body".to_string()))
    );
}

#[test]
fn a_path_walks_through_a_present_optional() {
    let mut held = Value::some(
        TypeExpr::Record(TypeId::new("detail")),
        Value::record(
            TypeId::new("detail"),
            vec![
                (FieldName::new("secret"), Value::Text("old".to_string())),
                (FieldName::new("kept"), Value::Text("kept".to_string())),
            ],
        ),
    );
    assert!(set_value_at(
        &mut held,
        &FieldPath::parse("secret"),
        Value::Text("new".to_string())
    ));
    assert_eq!(
        crate::value_at(&held, &FieldPath::parse("secret")),
        Some(Value::Text("new".to_string()))
    );
}

#[test]
fn a_path_reaching_an_absent_optional_writes_nothing() {
    let mut held = Value::none(TypeExpr::Record(TypeId::new("detail")));
    let before = held.clone();
    assert!(!set_value_at(
        &mut held,
        &FieldPath::parse("secret"),
        Value::Text("new".to_string())
    ));
    assert_eq!(held, before);
}

#[test]
fn a_path_naming_a_field_the_record_lacks_writes_nothing() {
    let mut held = note("one", "body", 3, "secret");
    let before = held.clone();
    assert!(!set_value_at(
        &mut held,
        &FieldPath::parse("district"),
        Value::Text("new".to_string())
    ));
    assert_eq!(held, before);
}

#[test]
fn a_path_reaching_past_a_leaf_writes_nothing() {
    let mut held = Value::Integer(4);
    assert!(!set_value_at(
        &mut held,
        &FieldPath::parse("body"),
        Value::Integer(5)
    ));
    assert_eq!(held, Value::Integer(4));
}

#[test]
fn clearing_a_field_the_record_lacks_reports_the_miss() {
    let mut held = note("one", "body", 3, "secret");
    let before = held.clone();
    assert!(!clear_field(&mut held, &FieldPath::parse("district")));
    assert_eq!(held, before);
    assert!(clear_field(&mut held, &FieldPath::parse("body")));
    assert_eq!(
        crate::value_at(&held, &FieldPath::parse("body")),
        Some(Value::Text(String::new()))
    );
}

// The watch list.

#[test]
fn a_collection_holding_records_for_the_life_of_the_deployment_reaches_no_sweep() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "forever",
            RetentionPeriod::Indefinite,
            RetentionAction::Review,
        ));
        insert_into("forever", &note("one", "body", 1, "secret"));
        assert!(Pallet::<Test>::retention_watch().is_empty());
    });
}

#[test]
fn a_span_of_zero_milliseconds_is_declined_and_reaches_no_sweep() {
    new_test_ext().execute_with(|| {
        install_types();
        let held = collection_with(
            "instant",
            RetentionPeriod::Milliseconds(0),
            RetentionAction::Review,
        );
        let report = Schema::check_collection(&DEPLOYMENT, &held);
        let fixes: Vec<String> = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.fix.clone())
            .collect();
        assert!(
            fixes.contains(&"Set a span above zero, or choose indefinite retention.".to_string())
        );
        // The watch list guards the same bound, so a definition that reached
        // the store by another path still reaches no sweep.
        Pallet::<Test>::note_retention(&slot_of("instant"), &held);
        assert!(Pallet::<Test>::retention_watch().is_empty());
    });
}

#[test]
fn a_collection_with_a_finite_span_joins_the_watch_list_once() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        insert_into("review", &note("one", "body", 1, "secret"));
        insert_into("review", &note("two", "body", 2, "secret"));
        assert_eq!(Pallet::<Test>::retention_watch(), vec![slot_of("review")]);
    });
}

// The four expiry actions.

#[test]
fn review_keeps_the_record_and_records_the_review_moment() {
    new_test_ext().execute_with(|| {
        let record = note("one", "body", 1, "secret");
        plant(
            &collection_with(
                "review",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Review,
            ),
            &record,
        );
        next_block();

        let id = CollectionId::new("review");
        let key = key_of("one");
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key),
            Some(record)
        );
        assert_eq!(
            RetentionSettled::<Test>::get(slot_of("review"), &key),
            Some(PLANTED + SPAN)
        );
        assert_eq!(
            raised(),
            vec![crate::Event::RetentionReviewDue {
                deployment: DEPLOYMENT,
                collection: name("review"),
                key,
            }]
        );
    });
}

#[test]
fn a_record_inside_its_span_reaches_no_action() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("review", &note("one", "body", 1, "secret"));
        set_clock(PLANTED + SPAN - 1);
        next_block();

        assert!(RetentionSettled::<Test>::get(slot_of("review"), key_of("one")).is_none());
        assert!(raised().is_empty());
    });
}

#[test]
fn a_settled_record_reaches_the_action_once() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "review",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Review,
            ),
            &note("one", "body", 1, "secret"),
        );
        next_block();
        assert_eq!(raised().len(), 1);
        next_block();
        assert!(raised().is_empty());
    });
}

#[test]
fn erase_clears_the_payload_and_keeps_the_commitment() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "erase",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Erase,
            ),
            &note("one", "body", 1, "secret"),
        );
        let id = CollectionId::new("erase");
        let key = key_of("one");
        let root_before = Pallet::<Test>::collection_root(&DEPLOYMENT, &id);
        let empty = Pallet::<Test>::collection_root(&DEPLOYMENT, &CollectionId::new("blank"));
        assert!(root_before.is_some());
        assert_ne!(root_before, empty);

        next_block();

        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key),
            None
        );
        assert_eq!(
            Pallet::<Test>::collection_root(&DEPLOYMENT, &id),
            root_before
        );
        assert_eq!(
            raised(),
            vec![crate::Event::RetentionErased {
                deployment: DEPLOYMENT,
                collection: name("erase"),
                key,
            }]
        );
    });
}

#[test]
fn erase_over_a_record_the_store_lost_settles_the_key() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "erase",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Erase,
            ),
            &note("one", "body", 1, "secret"),
        );
        let key = key_of("one");
        Records::<Test>::remove(slot_of("erase"), &key);

        next_block();

        assert_eq!(
            RetentionSettled::<Test>::get(slot_of("erase"), &key),
            Some(PLANTED + SPAN)
        );
        assert!(raised().is_empty());
    });
}

#[test]
fn anonymise_clears_the_listed_fields_and_keeps_the_rest() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "anon",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Anonymise(vec![
                    FieldPath::parse("body"),
                    FieldPath::parse("detail.secret"),
                ]),
            ),
            &note("one", "body", 7, "secret"),
        );
        next_block();

        let id = CollectionId::new("anon");
        let key = key_of("one");
        let produced = <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key)
            .expect("the record stays in place");
        assert_eq!(
            crate::value_at(&produced, &FieldPath::parse("body")),
            Some(Value::Text(String::new()))
        );
        assert_eq!(
            crate::value_at(&produced, &FieldPath::parse("detail.secret")),
            Some(Value::Text(String::new()))
        );
        assert_eq!(
            crate::value_at(&produced, &FieldPath::parse("detail.kept")),
            Some(Value::Text("kept".to_string()))
        );
        assert_eq!(
            crate::value_at(&produced, &FieldPath::parse("count")),
            Some(Value::Integer(7))
        );
        assert_eq!(
            crate::value_at(&produced, &FieldPath::parse("id")),
            Some(Value::Text("one".to_string()))
        );
        assert_eq!(
            raised(),
            vec![crate::Event::RetentionAnonymised {
                deployment: DEPLOYMENT,
                collection: name("anon"),
                key: key.clone(),
            }]
        );

        let meta = Pallet::<Test>::record_meta(&DEPLOYMENT, &id, &key)
            .expect("the store holds the metadata");
        assert_eq!(meta.version, 2);
        let history = Pallet::<Test>::version_history(&DEPLOYMENT, &id, &key);
        assert_eq!(history.len(), 2);
        assert_eq!(history[1].version, 2);
        assert_eq!(history[1].commitment, meta.value_hash);
    });
}

#[test]
fn anonymise_naming_a_field_the_record_lacks_is_declined_with_its_fix() {
    new_test_ext().execute_with(|| {
        install_types();
        let held = collection_with(
            "anon",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Anonymise(vec![FieldPath::parse("district")]),
        );
        let report = Schema::check_collection(&DEPLOYMENT, &held);
        let fixes: Vec<String> = report
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.fix.clone())
            .collect();
        assert_eq!(
            fixes,
            vec!["Name a field declared on the record type `note`.".to_string()]
        );
        assert!(Pallet::<Test>::retention_watch().is_empty());
    });
}

#[test]
fn anonymise_over_a_record_the_store_lost_settles_the_key() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "anon",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Anonymise(vec![FieldPath::parse("body")]),
            ),
            &note("one", "body", 7, "secret"),
        );
        let key = key_of("one");
        Records::<Test>::remove(slot_of("anon"), &key);

        next_block();

        assert_eq!(
            RetentionSettled::<Test>::get(slot_of("anon"), &key),
            Some(PLANTED + SPAN)
        );
        assert!(raised().is_empty());
    });
}

/// Register the archive and the live collection the move tests share.
fn plant_move_pair(target: &str) {
    install_types();
    register(&collection_with(
        "archive",
        RetentionPeriod::Indefinite,
        RetentionAction::Review,
    ));
    register(&collection_with(
        "live",
        RetentionPeriod::Milliseconds(SPAN),
        RetentionAction::MoveTo(CollectionId::new(target)),
    ));
}

#[test]
fn move_writes_the_record_into_the_named_collection_and_removes_the_original() {
    new_test_ext().execute_with(|| {
        plant_move_pair("archive");
        set_clock(PLANTED);
        let record = note("one", "body", 5, "secret");
        insert_into("live", &record);
        set_clock(PLANTED + SPAN);

        let live = CollectionId::new("live");
        let filed = CollectionId::new("archive");
        let key = key_of("one");
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &live), 1);

        next_block();

        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &filed, &key),
            Some(record)
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &live, &key),
            None
        );
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &live), 0);
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &filed), 1);
        assert!(History::<Test>::get(slot_of("live"), &key).is_empty());
        assert_eq!(
            Pallet::<Test>::version_history(&DEPLOYMENT, &filed, &key).len(),
            1
        );
        assert!(WrittenAt::<Test>::get(slot_of("live"), &key).is_none());
        assert_eq!(
            WrittenAt::<Test>::get(slot_of("archive"), &key),
            Some(PLANTED + SPAN)
        );
        assert_eq!(
            raised(),
            vec![crate::Event::RetentionMoved {
                deployment: DEPLOYMENT,
                collection: name("live"),
                key,
                target: name("archive"),
            }]
        );
    });
}

#[test]
fn move_naming_a_collection_the_deployment_lacks_leaves_the_record_in_place() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "live",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::MoveTo(CollectionId::new("absent")),
        ));
        set_clock(PLANTED);
        let record = note("one", "body", 5, "secret");
        insert_into("live", &record);
        set_clock(PLANTED + SPAN);

        next_block();

        let live = CollectionId::new("live");
        let key = key_of("one");
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &live, &key),
            Some(record)
        );
        assert!(RetentionSettled::<Test>::get(slot_of("live"), &key).is_none());
        assert!(raised().is_empty());
    });
}

#[test]
fn move_onto_a_key_the_target_already_holds_settles_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        plant_move_pair("archive");
        set_clock(PLANTED);
        let standing = note("one", "filed already", 9, "secret");
        insert_into("archive", &standing);
        insert_into("live", &note("one", "body", 5, "secret"));
        set_clock(PLANTED + SPAN);

        next_block();

        let filed = CollectionId::new("archive");
        let key = key_of("one");
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &filed, &key),
            Some(standing)
        );
        assert_eq!(
            RetentionSettled::<Test>::get(slot_of("live"), &key),
            Some(PLANTED + SPAN)
        );
        assert!(raised().is_empty());
    });
}

#[test]
fn move_into_a_full_collection_leaves_the_record_in_place() {
    new_test_ext().execute_with(|| {
        plant_move_pair("archive");
        set_clock(PLANTED);
        let record = note("one", "body", 5, "secret");
        insert_into("live", &record);
        set_clock(PLANTED + SPAN);
        // The mock admits one thousand and twenty four records per
        // collection, so this count fills the archive.
        RecordCount::<Test>::insert(slot_of("archive"), 1_024u32);

        next_block();

        let live = CollectionId::new("live");
        let key = key_of("one");
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &live, &key),
            Some(record)
        );
        assert!(raised().is_empty());
    });
}

#[test]
fn move_over_a_record_the_store_lost_its_metadata_for_leaves_the_record_in_place() {
    new_test_ext().execute_with(|| {
        plant_move_pair("archive");
        set_clock(PLANTED);
        insert_into("live", &note("one", "body", 5, "secret"));
        set_clock(PLANTED + SPAN);
        let key = key_of("one");
        Meta::<Test>::remove(slot_of("live"), &key);

        next_block();

        assert!(RetentionSettled::<Test>::get(slot_of("live"), &key).is_none());
        assert!(raised().is_empty());
    });
}

#[test]
fn move_over_a_record_the_store_lost_settles_the_key() {
    new_test_ext().execute_with(|| {
        plant_move_pair("archive");
        set_clock(PLANTED);
        insert_into("live", &note("one", "body", 5, "secret"));
        set_clock(PLANTED + SPAN);
        let key = key_of("one");
        Records::<Test>::remove(slot_of("live"), &key);

        next_block();

        assert_eq!(
            RetentionSettled::<Test>::get(slot_of("live"), &key),
            Some(PLANTED + SPAN)
        );
        assert!(raised().is_empty());
    });
}

// The bound on one block.

#[test]
fn one_block_checks_at_most_the_declared_count_of_records() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        for number in 0..70u32 {
            insert_into(
                "review",
                &note(&format!("k{number:03}"), "body", 1, "secret"),
            );
        }
        set_clock(PLANTED + SPAN);

        next_block();

        // The mock declares a scan budget of sixty four records.
        assert_eq!(raised().len(), 64);
    });
}

#[test]
fn the_cursor_moves_one_place_each_block() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "first",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        register(&collection_with(
            "second",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("first", &note("one", "body", 1, "secret"));
        insert_into("second", &note("one", "body", 1, "secret"));
        set_clock(PLANTED + SPAN);
        assert_eq!(Pallet::<Test>::retention_watch().len(), 2);
        assert_eq!(RetentionCursor::<Test>::get(), 0);

        next_block();
        assert_eq!(RetentionCursor::<Test>::get(), 1);
        assert_eq!(raised().len(), 1);

        next_block();
        assert_eq!(RetentionCursor::<Test>::get(), 0);
        assert_eq!(raised().len(), 1);

        let key = key_of("one");
        assert!(RetentionSettled::<Test>::get(slot_of("first"), &key).is_some());
        assert!(RetentionSettled::<Test>::get(slot_of("second"), &key).is_some());
    });
}

#[test]
fn an_empty_watch_list_leaves_the_block_hook_with_nothing_to_do() {
    new_test_ext().execute_with(|| {
        assert!(RetentionWatch::<Test>::get().is_empty());
        next_block();
        assert!(raised().is_empty());
        assert_eq!(RetentionCursor::<Test>::get(), 0);
    });
}

#[test]
fn a_watched_collection_the_schema_lacks_reaches_no_action() {
    new_test_ext().execute_with(|| {
        RetentionWatch::<Test>::mutate(|held| {
            held.try_push(slot_of("ghost")).expect("the list has room");
        });
        next_block();
        assert!(raised().is_empty());
        assert_eq!(RetentionCursor::<Test>::get(), 0);
    });
}

#[test]
fn a_watched_collection_carrying_no_finite_span_reaches_no_action() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "forever",
            RetentionPeriod::Indefinite,
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("forever", &note("one", "body", 1, "secret"));
        set_clock(PLANTED + SPAN);
        RetentionWatch::<Test>::mutate(|held| {
            held.try_push(slot_of("forever"))
                .expect("the list has room");
        });

        next_block();

        assert!(RetentionSettled::<Test>::get(slot_of("forever"), key_of("one")).is_none());
        assert!(raised().is_empty());
    });
}

#[test]
fn a_record_the_store_never_dated_reaches_no_action() {
    new_test_ext().execute_with(|| {
        plant(
            &collection_with(
                "review",
                RetentionPeriod::Milliseconds(SPAN),
                RetentionAction::Review,
            ),
            &note("one", "body", 1, "secret"),
        );
        let key = key_of("one");
        WrittenAt::<Test>::remove(slot_of("review"), &key);

        next_block();

        assert!(RetentionSettled::<Test>::get(slot_of("review"), &key).is_none());
        assert!(raised().is_empty());
    });
}

// The version history.

#[test]
fn every_write_appends_one_entry_to_the_history_of_the_record() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("review", &note("one", "first", 1, "secret"));
        let id = CollectionId::new("review");
        let key = key_of("one");
        for round in 2..=4u32 {
            set_clock(PLANTED + round as u64);
            assert_ok!(State::update(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("review"),
                key.clone(),
                bounded(note("one", "later", round as i128, "secret").canonical_bytes())
            ));
        }
        let history = Pallet::<Test>::version_history(&DEPLOYMENT, &id, &key);
        assert_eq!(history.len(), 4);
        assert_eq!(
            history
                .iter()
                .map(|entry| entry.version)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(history[0].at_millis, PLANTED);
        assert_eq!(history[3].at_millis, PLANTED + 4);
        assert!(history.iter().all(|entry| entry.author == STEWARD));
        assert_eq!(history[3].block, System::block_number());
    });
}

#[test]
fn the_oldest_entry_leaves_once_the_history_bound_is_reached() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("review", &note("one", "first", 1, "secret"));
        let id = CollectionId::new("review");
        let key = key_of("one");
        for round in 2..=40u32 {
            assert_ok!(State::update(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("review"),
                key.clone(),
                bounded(note("one", "later", round as i128, "secret").canonical_bytes())
            ));
        }
        let history = Pallet::<Test>::version_history(&DEPLOYMENT, &id, &key);
        // The mock declares a history bound of thirty two entries.
        assert_eq!(history.len(), 32);
        assert_eq!(history[0].version, 9);
        assert_eq!(history[31].version, 40);
    });
}

#[test]
fn the_history_of_a_name_too_long_to_store_reads_empty() {
    new_test_ext().execute_with(|| {
        let long = "n".repeat(200);
        let held =
            Pallet::<Test>::version_history(&DEPLOYMENT, &CollectionId::new(&long), &key_of("one"));
        assert!(held.is_empty());
    });
}

#[test]
fn a_deleted_record_loses_its_history() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        set_clock(PLANTED);
        insert_into("review", &note("one", "first", 1, "secret"));
        let id = CollectionId::new("review");
        let key = key_of("one");
        assert_eq!(
            Pallet::<Test>::version_history(&DEPLOYMENT, &id, &key).len(),
            1
        );
        assert_ok!(State::delete(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("review"),
            key.clone()
        ));
        assert!(Pallet::<Test>::version_history(&DEPLOYMENT, &id, &key).is_empty());
    });
}

#[test]
fn a_refused_collection_declines_the_write_and_names_the_fix() {
    new_test_ext().execute_with(|| {
        install_types();
        register(&collection_with(
            "review",
            RetentionPeriod::Milliseconds(SPAN),
            RetentionAction::Review,
        ));
        refuse("review");
        let outcome = State::insert(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("review"),
            bounded(note("one", "body", 1, "secret").canonical_bytes()),
        );
        assert_eq!(
            outcome,
            Err(sp_runtime::DispatchError::from(
                Error::<Test>::PermissionDenied
            ))
        );

        let request = AccessRequest::new(
            DEPLOYMENT,
            &STEWARD,
            CollectionId::new("review"),
            AccessAction::Insert,
            Timestamp::from_millis(PLANTED),
        );
        let denial = Pallet::<Test>::explain(&request).expect_err("the gate refuses the request");
        assert_eq!(
            denial.problem,
            "The role reaches no record of `review`, so the insert is refused."
        );
        assert_eq!(
            denial.fix,
            "Grant the role the capability over that collection."
        );
        admit_every_collection();
    });
}
