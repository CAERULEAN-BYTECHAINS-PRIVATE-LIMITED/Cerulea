//! The tests of the state module.

use crate::{
    head_field, key_bytes_of, mock::*, value_at, Error, Meta, Pallet, RecordCount, RecordStore,
    Records, WriteMode,
};
use cerulea_blueprint::{
    collections::{Collection, KeyRange, PurposeDeclaration},
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, RecordDef, TypeDef, TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest, PermissionGate},
    primitives::{name_of, DeploymentId, RecordKey},
    SchemaRegistry,
};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;

fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

fn parcel_collection() -> Collection {
    let mut collection = Collection::new(
        "parcel",
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records who holds each land parcel.",
        &["registration"],
    );
    collection
}

fn parcel(survey: &str, holder: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn collection_name() -> BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    BoundedVec::try_from(b"parcel".to_vec()).expect("the name fits")
}

fn install() {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_type().encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_collection().encode())
    ));
}

fn key_of(record: &Value) -> RecordKey {
    let bytes = key_bytes_of(&parcel_collection(), record).expect("the record carries its key");
    BoundedVec::try_from(bytes).expect("the key sits inside the bound")
}

fn insert(record: &Value) -> frame_support::pallet_prelude::DispatchResult {
    State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        collection_name(),
        bounded(record.canonical_bytes()),
    )
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the traits stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_store<S: RecordStore>() {}
    fn requires_gate<G: PermissionGate>() {}
    fn requires_hook<H: pallet_cerulea_schema::MigrationHook>() {}
    requires_store::<Pallet<Test>>();
    requires_gate::<TestGate>();
    requires_hook::<Pallet<Test>>();
}

#[test]
fn an_inserted_record_takes_the_key_its_primary_key_field_holds() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        let id = CollectionId::new("parcel");
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key),
            Some(record)
        );
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id), 1);
        let meta = Pallet::<Test>::record_meta(&DEPLOYMENT, &id, &key)
            .expect("the store holds the metadata");
        assert_eq!(meta.version, 1);
        assert_eq!(meta.created_at, 1);
        assert_eq!(meta.author, STEWARD);
    });
}

#[test]
fn a_repeated_insert_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        assert_noop!(insert(&record), Error::<Test>::RecordPresent);
    });
}

#[test]
fn an_update_raises_the_version_and_keeps_the_creation_block() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        System::set_block_number(9);
        let changed = parcel("12/3", "Benny", 400);
        assert_ok!(State::update(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            collection_name(),
            key.clone(),
            bounded(changed.canonical_bytes())
        ));
        let id = CollectionId::new("parcel");
        let meta = Pallet::<Test>::record_meta(&DEPLOYMENT, &id, &key)
            .expect("the store holds the metadata");
        assert_eq!(meta.version, 2);
        assert_eq!(meta.created_at, 1);
        assert_eq!(meta.changed_at, 9);
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key),
            Some(changed)
        );
    });
}

#[test]
fn an_update_naming_another_key_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let other = key_of(&parcel("44/9", "Benny", 100));
        assert_noop!(
            State::update(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                collection_name(),
                other,
                bounded(parcel("12/3", "Benny", 400).canonical_bytes())
            ),
            Error::<Test>::KeyMismatch
        );
    });
}

#[test]
fn an_update_of_an_absent_record_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(
            State::update(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                collection_name(),
                key_of(&record),
                bounded(record.canonical_bytes())
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

#[test]
fn a_delete_clears_the_record_the_metadata_and_the_count() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        assert_ok!(State::delete(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            collection_name(),
            key.clone()
        ));
        let id = CollectionId::new("parcel");
        assert!(<Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key).is_none());
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id), 0);
        let slot = pallet_cerulea_schema::primitives::CollectionKey::new(
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
        );
        assert!(Meta::<Test>::get(&slot, &key).is_none());
        assert!(Records::<Test>::get(&slot, &key).is_none());
        assert_eq!(RecordCount::<Test>::get(&slot), 0);
    });
}

#[test]
fn a_write_resolves_permission_first_and_aborts_on_denial() {
    // Design assertion two. A write to state resolves permission first and
    // aborts on denial.
    new_test_ext().execute_with(|| {
        install();
        refuse("parcel");
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(insert(&record), Error::<Test>::PermissionDenied);
        let id = CollectionId::new("parcel");
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id), 0);
        admit_every_collection();
        assert_ok!(insert(&record));
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id), 1);
    });
}

#[test]
fn a_refusal_carries_a_plain_problem_and_a_plain_fix() {
    new_test_ext().execute_with(|| {
        install();
        refuse("parcel");
        let actor = STEWARD;
        let request = AccessRequest::new(
            DEPLOYMENT,
            &actor,
            CollectionId::new("parcel"),
            AccessAction::Insert,
            cerulea_blueprint::types::Timestamp::from_millis(0),
        );
        let denial = Pallet::<Test>::explain(&request).expect_err("the gate refuses");
        assert!(denial.problem.ends_with('.'));
        assert!(denial.fix.starts_with("Grant the role"));
    });
}

#[test]
fn a_record_of_another_shape_aborts_the_write() {
    new_test_ext().execute_with(|| {
        install();
        let wrong = Value::record(
            TypeId::new("parcel"),
            vec![(
                FieldName::new("survey_number"),
                Value::Text("12/3".to_string()),
            )],
        );
        assert_noop!(insert(&wrong), Error::<Test>::RecordInvalid);
    });
}

#[test]
fn a_write_to_an_absent_collection_is_declined() {
    new_test_ext().execute_with(|| {
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(insert(&record), Error::<Test>::CollectionUnknown);
    });
}

#[test]
fn a_write_to_a_retired_collection_is_declined() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Schema::retire_collection(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits")
        ));
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(insert(&record), Error::<Test>::CollectionRetired);
    });
}

#[test]
fn a_range_read_returns_the_records_in_canonical_key_order() {
    new_test_ext().execute_with(|| {
        install();
        for survey in ["12/3", "44/9", "07/1"] {
            assert_ok!(insert(&parcel(survey, "Anitha", 100)));
        }
        let id = CollectionId::new("parcel");
        let found = <Pallet<Test> as RecordStore>::range(&DEPLOYMENT, &id, &KeyRange::Full, 100);
        assert_eq!(found.len(), 3);
        let mut keys: Vec<Vec<u8>> = found.iter().map(|(key, _)| key.to_vec()).collect();
        let sorted = {
            let mut held = keys.clone();
            held.sort();
            held
        };
        assert_eq!(keys, sorted);
        keys.dedup();
        assert_eq!(keys.len(), 3);
    });
}

#[test]
fn a_range_read_over_one_key_returns_one_record() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        assert_ok!(insert(&parcel("44/9", "Benny", 100)));
        let id = CollectionId::new("parcel");
        let key = key_of(&record);
        let found = <Pallet<Test> as RecordStore>::range(
            &DEPLOYMENT,
            &id,
            &KeyRange::Exact(key.to_vec()),
            100,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].1, record);
    });
}

#[test]
fn a_range_read_holds_its_limit() {
    new_test_ext().execute_with(|| {
        install();
        for survey in ["12/3", "44/9", "07/1", "88/2"] {
            assert_ok!(insert(&parcel(survey, "Anitha", 100)));
        }
        let id = CollectionId::new("parcel");
        let found = <Pallet<Test> as RecordStore>::range(&DEPLOYMENT, &id, &KeyRange::Full, 2);
        assert_eq!(found.len(), 2);
        let keys = <Pallet<Test> as RecordStore>::keys(&DEPLOYMENT, &id, 3);
        assert_eq!(keys.len(), 3);
    });
}

#[test]
fn a_read_of_an_absent_key_answers_with_nothing() {
    new_test_ext().execute_with(|| {
        install();
        let id = CollectionId::new("parcel");
        let key = key_of(&parcel("12/3", "Anitha", 400));
        assert!(<Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key).is_none());
        assert!(<Pallet<Test> as RecordStore>::meta(&DEPLOYMENT, &id, &key).is_none());
    });
}

#[test]
fn opaque_bytes_replace_a_record_and_read_back_unchanged() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        let id = CollectionId::new("parcel");
        assert_ok!(<Pallet<Test> as RecordStore>::write_bytes(
            &DEPLOYMENT,
            &id,
            &STEWARD,
            &key,
            vec![1, 2, 3, 4]
        ));
        assert_eq!(
            <Pallet<Test> as RecordStore>::read_bytes(&DEPLOYMENT, &id, &key),
            Some(vec![1, 2, 3, 4])
        );
        assert!(<Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id, &key).is_none());
    });
}

#[test]
fn a_field_path_reads_the_value_it_names() {
    let record = parcel("12/3", "Anitha", 400);
    assert_eq!(
        value_at(&record, &FieldPath::parse("holder")),
        Some(Value::Text("Anitha".to_string()))
    );
    assert_eq!(value_at(&record, &FieldPath::parse("ward")), None);
}

#[test]
fn a_primary_key_path_names_its_head_field() {
    assert_eq!(
        head_field(&parcel_collection().primary_key),
        Some(FieldName::new("survey_number"))
    );
    assert_eq!(head_field(&FieldPath::parse("")), None);
}

#[test]
fn the_clock_the_module_reads_comes_from_the_runtime() {
    new_test_ext().execute_with(|| {
        set_clock(1_700_000_000_000);
        assert_eq!(Pallet::<Test>::now_millis(), 1_700_000_000_000);
    });
}

#[test]
fn a_write_mode_names_the_access_it_asks_for() {
    new_test_ext().execute_with(|| {
        install();
        refuse("parcel");
        let record = parcel("12/3", "Anitha", 400);
        assert_noop!(
            Pallet::<Test>::write(
                &DEPLOYMENT,
                &CollectionId::new("parcel"),
                &STEWARD,
                None,
                &record,
                WriteMode::Insert
            ),
            Error::<Test>::PermissionDenied
        );
        assert_noop!(
            Pallet::<Test>::write(
                &DEPLOYMENT,
                &CollectionId::new("parcel"),
                &STEWARD,
                None,
                &record,
                WriteMode::Update
            ),
            Error::<Test>::PermissionDenied
        );
    });
}

#[test]
fn an_applied_migration_rewrites_every_stored_record() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(insert(&parcel("12/3", "Anitha", 400)));
        assert_ok!(insert(&parcel("44/9", "Benny", 100)));

        let plan = pallet_cerulea_schema::migrate::MigrationPlan::new(
            "parcel",
            1,
            "The register renames the holder field.",
        )
        .with(pallet_cerulea_schema::migrate::MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .within(0);
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(plan.encode())
        ));
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
            1
        ));

        let id = CollectionId::new("parcel");
        assert_eq!(
            <Schema as SchemaRegistry>::collection_version(&DEPLOYMENT, &id),
            Some(2)
        );
        let found = <Pallet<Test> as RecordStore>::range(&DEPLOYMENT, &id, &KeyRange::Full, 100);
        assert_eq!(found.len(), 2);
        for (_, record) in found {
            match record {
                Value::Record(held) => {
                    assert!(held.field(&FieldName::new("owner")).is_some());
                    assert!(held.field(&FieldName::new("holder")).is_none());
                }
                other => unreachable!("the store holds {:?}", other),
            }
        }
    });
}

fn indexed_collection(unique: bool) -> Collection {
    let mut collection = parcel_collection();
    collection.indexes = vec![cerulea_blueprint::collections::Index {
        name: cerulea_blueprint::types::IndexName::new("by_holder"),
        fields: vec![FieldPath::parse("holder")],
        unique,
    }];
    collection
}

fn install_indexed(unique: bool) {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_type().encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(indexed_collection(unique).encode())
    ));
}

fn holders_named(name: &str) -> Vec<RecordKey> {
    <Pallet<Test> as RecordStore>::by_index(
        &DEPLOYMENT,
        &CollectionId::new("parcel"),
        &cerulea_blueprint::types::IndexName::new("by_holder"),
        &[Value::Text(name.to_string())],
    )
}

#[test]
fn an_index_holds_the_record_after_an_insert() {
    // Design assertion three. An index is present and correct after every
    // write, update and delete.
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        assert_eq!(holders_named("Anitha"), vec![key_of(&record)]);
        assert!(holders_named("Benny").is_empty());
    });
}

#[test]
fn an_index_follows_the_record_through_an_update() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        let changed = parcel("12/3", "Benny", 400);
        assert_ok!(State::update(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            collection_name(),
            key.clone(),
            bounded(changed.canonical_bytes())
        ));
        assert!(holders_named("Anitha").is_empty());
        assert_eq!(holders_named("Benny"), vec![key]);
    });
}

#[test]
fn an_index_clears_after_a_delete() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        assert_ok!(State::delete(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            collection_name(),
            key_of(&record)
        ));
        assert!(holders_named("Anitha").is_empty());
    });
}

#[test]
fn one_index_entry_holds_every_record_reaching_it() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let first = parcel("12/3", "Anitha", 400);
        let second = parcel("44/9", "Anitha", 100);
        assert_ok!(insert(&first));
        assert_ok!(insert(&second));
        let mut want = vec![key_of(&first), key_of(&second)];
        want.sort();
        assert_eq!(holders_named("Anitha"), want);
    });
}

#[test]
fn a_unique_index_declines_a_second_record_under_one_value() {
    new_test_ext().execute_with(|| {
        install_indexed(true);
        assert_ok!(insert(&parcel("12/3", "Anitha", 400)));
        assert_noop!(
            insert(&parcel("44/9", "Anitha", 100)),
            Error::<Test>::IndexConflict
        );
        let id = CollectionId::new("parcel");
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id), 1);
    });
}

#[test]
fn a_probe_of_another_shape_reaches_nothing() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        assert_ok!(insert(&parcel("12/3", "Anitha", 400)));
        let found = <Pallet<Test> as RecordStore>::by_index(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &cerulea_blueprint::types::IndexName::new("by_holder"),
            &[],
        );
        assert!(found.is_empty());
        let absent = <Pallet<Test> as RecordStore>::by_index(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &cerulea_blueprint::types::IndexName::new("by_ward"),
            &[Value::Text("Anitha".to_string())],
        );
        assert!(absent.is_empty());
    });
}

#[test]
fn an_index_value_separates_two_field_splits() {
    let index = cerulea_blueprint::collections::Index {
        name: cerulea_blueprint::types::IndexName::new("by_pair"),
        fields: vec![
            FieldPath::parse("survey_number"),
            FieldPath::parse("holder"),
        ],
        unique: false,
    };
    let left = crate::index::index_probe(
        &index,
        &[Value::Text("ab".to_string()), Value::Text("c".to_string())],
    )
    .expect("the probe builds");
    let right = crate::index::index_probe(
        &index,
        &[Value::Text("a".to_string()), Value::Text("bc".to_string())],
    )
    .expect("the probe builds");
    assert_ne!(left, right);
}

#[test]
fn a_sealed_payload_carries_no_index_entry() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));
        let key = key_of(&record);
        assert_ok!(<Pallet<Test> as RecordStore>::write_bytes(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &STEWARD,
            &key,
            vec![9, 9, 9]
        ));
        assert!(holders_named("Anitha").is_empty());
    });
}

#[test]
fn a_migration_rebuilds_every_index_entry() {
    new_test_ext().execute_with(|| {
        install_indexed(false);
        let record = parcel("12/3", "Anitha", 400);
        assert_ok!(insert(&record));

        let plan = pallet_cerulea_schema::migrate::MigrationPlan::new(
            "parcel",
            1,
            "The register renames the holder field.",
        )
        .with(pallet_cerulea_schema::migrate::MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .within(0);
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(plan.encode())
        ));
        assert_ok!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
            1
        ));

        let found = <Pallet<Test> as RecordStore>::by_index(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &cerulea_blueprint::types::IndexName::new("by_holder"),
            &[Value::Text("Anitha".to_string())],
        );
        assert_eq!(found, vec![key_of(&record)]);
    });
}
