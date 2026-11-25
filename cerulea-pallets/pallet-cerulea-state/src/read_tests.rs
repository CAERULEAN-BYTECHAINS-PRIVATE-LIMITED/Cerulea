//! The tests of the read side of the store and of the record rewrite.
//!
//! The read side answers the logic module, the interface layer and the
//! Explorer. The rewrite runs inside a schema migration and moves every
//! stored record into the new shape, keys included.

use crate::{mock::*, Error, Pallet, RecordStore};
use cerulea_blueprint::{
    collections::{Collection, Index, KeyRange, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, IndexName, RecordDef, TypeDef, TypeExpr,
        TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::{
    migrate::{MigrationPlan, MigrationStep, OLD_VALUE_BINDING},
    primitives::{name_of, DeploymentId, RecordKey},
};
use sp_runtime::DispatchError;

const DEPLOYMENT: DeploymentId = [6u8; 32];
const STEWARD: u64 = 1;

fn permit_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("permit"),
        fields: vec![
            FieldDef::new("number", TypeExpr::Text, "The permit number."),
            FieldDef::new("holder", TypeExpr::Text, "The holder of the permit."),
            FieldDef::new(
                "note",
                TypeExpr::Optional(Box::new(TypeExpr::Text)),
                "A note the office keeps.",
            ),
        ],
        summary: "One permit.".to_string(),
    })
}

fn permits() -> Collection {
    let mut held = Collection::new("permit", "permit", "number", "The register of permits.");
    held.purpose = PurposeDeclaration::new("The register records permits.", &["licensing"]);
    held.indexes.push(Index {
        name: IndexName::new("by_holder"),
        fields: vec![FieldPath::parse("holder")],
        unique: false,
    });
    held
}

fn permit(number: &str, holder: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("permit"),
        vec![
            (FieldName::new("number"), Value::Text(number.to_string())),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("note"), Value::none(TypeExpr::Text)),
        ],
    );
    value.canonicalise();
    value
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn name() -> BoundedVec<u8, ConstU32<128>> {
    BoundedVec::try_from(b"permit".to_vec()).expect("the name fits")
}

fn id() -> CollectionId {
    CollectionId::new("permit")
}

fn long_id() -> CollectionId {
    CollectionId::new(&"p".repeat(200))
}

fn key(number: &str) -> RecordKey {
    BoundedVec::try_from(Value::Text(number.to_string()).canonical_bytes())
        .expect("the key sits inside the bound")
}

fn install() {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(permit_type().encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(permits().encode())
    ));
}

fn insert(record: &Value) {
    assert_ok!(State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name(),
        bounded(record.canonical_bytes())
    ));
}

fn install_three() {
    install();
    insert(&permit("P-2", "Benny"));
    insert(&permit("P-1", "Anitha"));
    insert(&permit("Q-1", "Anitha"));
}

// Key listings.

#[test]
fn the_key_listing_runs_in_canonical_order_and_holds_its_limit() {
    new_test_ext().execute_with(|| {
        install_three();
        assert_eq!(
            <Pallet<Test> as RecordStore>::keys(&DEPLOYMENT, &id(), 10),
            vec![key("P-1"), key("P-2"), key("Q-1")]
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::keys(&DEPLOYMENT, &id(), 2),
            vec![key("P-1"), key("P-2")]
        );
    });
}

#[test]
fn the_key_listing_after_a_cursor_starts_past_the_cursor() {
    new_test_ext().execute_with(|| {
        install_three();
        assert_eq!(
            <Pallet<Test> as RecordStore>::keys_after(&DEPLOYMENT, &id(), None, 10),
            vec![key("P-1"), key("P-2"), key("Q-1")]
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::keys_after(&DEPLOYMENT, &id(), Some(&key("P-1")), 10),
            vec![key("P-2"), key("Q-1")]
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::keys_after(&DEPLOYMENT, &id(), Some(&key("P-2")), 1),
            vec![key("Q-1")]
        );
        assert!(<Pallet<Test> as RecordStore>::keys_after(
            &DEPLOYMENT,
            &id(),
            Some(&key("Q-1")),
            10
        )
        .is_empty());
    });
}

#[test]
fn a_name_too_long_to_store_reads_as_an_empty_collection() {
    new_test_ext().execute_with(|| {
        install_three();
        let long = long_id();
        assert!(Pallet::<Test>::record_keys(&DEPLOYMENT, &long, 10).is_empty());
        assert!(Pallet::<Test>::record_keys_after(&DEPLOYMENT, &long, None, 10).is_empty());
        assert!(Pallet::<Test>::range_bytes(&DEPLOYMENT, &long, &KeyRange::Full, 10).is_empty());
        assert_eq!(Pallet::<Test>::record_count(&DEPLOYMENT, &long), 0);
        assert_eq!(
            Pallet::<Test>::record_bytes(&DEPLOYMENT, &long, &key("P-1")),
            None
        );
        assert_eq!(
            Pallet::<Test>::record_meta(&DEPLOYMENT, &long, &key("P-1")),
            None
        );
        assert_eq!(Pallet::<Test>::collection_root(&DEPLOYMENT, &long), None);
        assert!(Pallet::<Test>::inclusion_proof(&DEPLOYMENT, &long, &key("P-1")).is_none());
        assert!(Pallet::<Test>::index_lookup(
            &DEPLOYMENT,
            &long,
            &IndexName::new("by_holder"),
            &[Value::Text("Anitha".to_string())]
        )
        .is_empty());
    });
}

// Range reads.

#[test]
fn a_prefix_range_reaches_every_key_carrying_the_prefix() {
    new_test_ext().execute_with(|| {
        install_three();
        // The canonical encoding of a text key starts with the same bytes
        // for every key sharing its first characters and its length.
        let shared = key("P-1");
        let prefix = shared[..shared.len() - 1].to_vec();
        let found = Pallet::<Test>::range_bytes(&DEPLOYMENT, &id(), &KeyRange::Prefix(prefix), 10);
        let keys: Vec<RecordKey> = found.iter().map(|(held, _)| held.clone()).collect();
        assert_eq!(keys, vec![key("P-1"), key("P-2")]);
        assert_eq!(found[0].1, permit("P-1", "Anitha").canonical_bytes());
    });
}

#[test]
fn an_exact_range_reaches_one_key_or_nothing() {
    new_test_ext().execute_with(|| {
        install_three();
        let found = Pallet::<Test>::range_bytes(
            &DEPLOYMENT,
            &id(),
            &KeyRange::Exact(key("Q-1").into_inner()),
            10,
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, key("Q-1"));
        assert!(Pallet::<Test>::range_bytes(
            &DEPLOYMENT,
            &id(),
            &KeyRange::Exact(key("Z-9").into_inner()),
            10
        )
        .is_empty());
    });
}

// Index reads.

#[test]
fn an_index_read_reaches_every_record_under_the_probe() {
    new_test_ext().execute_with(|| {
        install_three();
        let mut found = <Pallet<Test> as RecordStore>::by_index(
            &DEPLOYMENT,
            &id(),
            &IndexName::new("by_holder"),
            &[Value::Text("Anitha".to_string())],
        );
        found.sort();
        assert_eq!(found, vec![key("P-1"), key("Q-1")]);
    });
}

#[test]
fn an_index_read_over_an_unknown_collection_or_index_reaches_nothing() {
    new_test_ext().execute_with(|| {
        install_three();
        let probe = [Value::Text("Anitha".to_string())];
        assert!(Pallet::<Test>::index_lookup(
            &DEPLOYMENT,
            &CollectionId::new("licence"),
            &IndexName::new("by_holder"),
            &probe
        )
        .is_empty());
        assert!(Pallet::<Test>::index_lookup(
            &DEPLOYMENT,
            &id(),
            &IndexName::new("by_district"),
            &probe
        )
        .is_empty());
    });
}

// Metadata.

#[test]
fn the_store_metadata_reports_block_numbers_as_whole_numbers() {
    new_test_ext().execute_with(|| {
        install();
        System::set_block_number(7);
        insert(&permit("P-1", "Anitha"));
        let meta = <Pallet<Test> as RecordStore>::meta(&DEPLOYMENT, &id(), &key("P-1"))
            .expect("the store holds the metadata");
        let held = Pallet::<Test>::record_meta(&DEPLOYMENT, &id(), &key("P-1"))
            .expect("the store holds the metadata");
        assert_eq!(meta.created_at, 7);
        assert_eq!(meta.changed_at, 7);
        assert_eq!(meta.version, 1);
        assert_eq!(meta.author, STEWARD);
        assert_eq!(meta.value_hash, held.value_hash);
        assert!(<Pallet<Test> as RecordStore>::meta(&DEPLOYMENT, &id(), &key("P-9")).is_none());
    });
}

// The shape check maps each report onto one error code.

#[test]
fn the_shape_check_maps_each_problem_onto_its_error_code() {
    new_test_ext().execute_with(|| {
        // The registry lacks the type, so the check names the type.
        assert_eq!(
            Pallet::<Test>::check_shape(&DEPLOYMENT, &permits(), &permit("P-1", "Anitha")),
            Err(DispatchError::from(Error::<Test>::RecordTypeUnknown))
        );

        install();
        assert_eq!(
            Pallet::<Test>::check_shape(&DEPLOYMENT, &permits(), &permit("P-1", "Anitha")),
            Ok(key("P-1").into_inner())
        );

        // A record carrying another shape.
        assert_eq!(
            Pallet::<Test>::check_shape(&DEPLOYMENT, &permits(), &Value::Integer(3)),
            Err(DispatchError::from(Error::<Test>::RecordInvalid))
        );

        // A primary key past the stored bound.
        let long = permit(&"n".repeat(300), "Anitha");
        assert_eq!(
            Pallet::<Test>::check_shape(&DEPLOYMENT, &permits(), &long),
            Err(DispatchError::from(Error::<Test>::PrimaryKeyTooLong))
        );

        // A primary key over an optional field sorts differently across
        // nodes, so the key check declines it.
        let mut optional_key = permits();
        optional_key.primary_key = FieldPath::parse("note");
        assert_eq!(
            Pallet::<Test>::check_shape(&DEPLOYMENT, &optional_key, &permit("P-1", "Anitha")),
            Err(DispatchError::from(Error::<Test>::PrimaryKeyMissing))
        );
    });
}

// Raw byte writes.

#[test]
fn raw_bytes_past_the_record_bound_are_declined() {
    new_test_ext().execute_with(|| {
        install_three();
        assert_noop!(
            <Pallet<Test> as RecordStore>::write_bytes(
                &DEPLOYMENT,
                &id(),
                &STEWARD,
                &key("P-1"),
                vec![0u8; 8_193],
            ),
            Error::<Test>::RecordTooLarge
        );
    });
}

#[test]
fn raw_bytes_for_an_unknown_collection_are_declined() {
    new_test_ext().execute_with(|| {
        install_three();
        assert!(<Pallet<Test> as RecordStore>::write_bytes(
            &DEPLOYMENT,
            &CollectionId::new("licence"),
            &STEWARD,
            &key("P-1"),
            vec![1, 2, 3],
        )
        .is_err());
    });
}

// The rewrite a migration runs.

fn apply(plan: &MigrationPlan) {
    assert_ok!(Schema::propose_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(plan.encode())
    ));
    assert_ok!(Schema::apply_migration(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name_of("permit").expect("the name fits"),
        plan.from_version
    ));
}

#[test]
fn a_retyped_primary_key_moves_every_record_to_its_new_key() {
    new_test_ext().execute_with(|| {
        install_three();
        let plan = MigrationPlan::new("permit", 1, "The register prefixes every permit number.")
            .with(MigrationStep::RetypeField {
                field: FieldName::new("number"),
                ty: TypeExpr::Text,
                convert: Expr::binary(
                    BinaryOp::ConcatText,
                    Expr::text("TS-"),
                    Expr::bound(OLD_VALUE_BINDING),
                ),
            });
        apply(&plan);

        assert_eq!(
            <Pallet<Test> as RecordStore>::keys(&DEPLOYMENT, &id(), 10),
            vec![key("TS-P-1"), key("TS-P-2"), key("TS-Q-1")]
        );
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id()), 3);
        assert!(Pallet::<Test>::record(&DEPLOYMENT, &id(), &key("P-1")).is_none());
        assert!(Pallet::<Test>::record_meta(&DEPLOYMENT, &id(), &key("P-1")).is_none());
        assert_eq!(
            Pallet::<Test>::record(&DEPLOYMENT, &id(), &key("TS-P-1")),
            Some(permit("TS-P-1", "Anitha"))
        );
        let meta = Pallet::<Test>::record_meta(&DEPLOYMENT, &id(), &key("TS-P-1"))
            .expect("the moved record keeps its metadata");
        assert_eq!(meta.version, 2);

        // The index and the inclusion tree follow the move.
        let mut found = Pallet::<Test>::index_lookup(
            &DEPLOYMENT,
            &id(),
            &IndexName::new("by_holder"),
            &[Value::Text("Anitha".to_string())],
        );
        found.sort();
        assert_eq!(found, vec![key("TS-P-1"), key("TS-Q-1")]);
        let root =
            Pallet::<Test>::collection_root(&DEPLOYMENT, &id()).expect("the root is present");
        let proof = Pallet::<Test>::inclusion_proof(&DEPLOYMENT, &id(), &key("TS-P-2"))
            .expect("the moved record proves itself");
        assert!(proof.verifies(&root));
    });
}

#[test]
fn a_reverted_migration_rewrites_every_record_back() {
    new_test_ext().execute_with(|| {
        install_three();
        let plan = MigrationPlan::new("permit", 1, "The register renames the holder field.")
            .with(MigrationStep::RenameField {
                from: FieldName::new("holder"),
                to: FieldName::new("owner"),
            })
            .within(10);
        apply(&plan);
        let renamed = Pallet::<Test>::record(&DEPLOYMENT, &id(), &key("P-1"))
            .expect("the record stays under its key");
        match &renamed {
            Value::Record(held) => assert!(held.field(&FieldName::new("owner")).is_some()),
            other => panic!("the store holds a record, and this reads {other:?}"),
        }

        assert_ok!(Schema::revert_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("permit").expect("the name fits"),
            1
        ));
        assert_eq!(
            Pallet::<Test>::record(&DEPLOYMENT, &id(), &key("P-1")),
            Some(permit("P-1", "Anitha"))
        );
        let meta = Pallet::<Test>::record_meta(&DEPLOYMENT, &id(), &key("P-1"))
            .expect("the record keeps its metadata");
        assert_eq!(meta.version, 3);
    });
}

#[test]
fn a_conversion_producing_a_key_past_the_bound_leaves_every_record_in_place() {
    new_test_ext().execute_with(|| {
        install_three();
        let plan = MigrationPlan::new("permit", 1, "The register pads every permit number.").with(
            MigrationStep::RetypeField {
                field: FieldName::new("number"),
                ty: TypeExpr::Text,
                convert: Expr::binary(
                    BinaryOp::ConcatText,
                    Expr::text(&"x".repeat(300)),
                    Expr::bound(OLD_VALUE_BINDING),
                ),
            },
        );
        assert_ok!(Schema::propose_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(plan.encode())
        ));
        assert!(Schema::apply_migration(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("permit").expect("the name fits"),
            1
        )
        .is_err());
        assert_eq!(
            Pallet::<Test>::record(&DEPLOYMENT, &id(), &key("P-1")),
            Some(permit("P-1", "Anitha"))
        );
    });
}

// The store trait the logic module writes through.

#[test]
fn the_store_trait_inserts_updates_and_deletes_one_record() {
    new_test_ext().execute_with(|| {
        install();
        let written = <Pallet<Test> as RecordStore>::insert(
            &DEPLOYMENT,
            &id(),
            &STEWARD,
            &permit("P-1", "Anitha"),
        )
        .expect("the insert lands");
        assert_eq!(written, key("P-1"));
        assert_ok!(<Pallet<Test> as RecordStore>::update(
            &DEPLOYMENT,
            &id(),
            &STEWARD,
            &key("P-1"),
            &permit("P-1", "Benny"),
        ));
        assert_eq!(
            <Pallet<Test> as RecordStore>::read(&DEPLOYMENT, &id(), &key("P-1")),
            Some(permit("P-1", "Benny"))
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key("P-1")),
            Some(permit("P-1", "Benny").canonical_bytes())
        );
        assert_ok!(<Pallet<Test> as RecordStore>::delete(
            &DEPLOYMENT,
            &id(),
            &STEWARD,
            &key("P-1"),
        ));
        assert_eq!(<Pallet<Test> as RecordStore>::count(&DEPLOYMENT, &id()), 0);
        assert_noop!(
            <Pallet<Test> as RecordStore>::delete(&DEPLOYMENT, &id(), &STEWARD, &key("P-1")),
            Error::<Test>::RecordAbsent
        );
    });
}

// Index maintenance.

fn slot() -> pallet_cerulea_schema::primitives::CollectionKey {
    pallet_cerulea_schema::primitives::CollectionKey::new(
        DEPLOYMENT,
        name_of("permit").expect("the name fits"),
    )
}

fn holders(holder: &str) -> Vec<RecordKey> {
    Pallet::<Test>::index_lookup(
        &DEPLOYMENT,
        &id(),
        &IndexName::new("by_holder"),
        &[Value::Text(holder.to_string())],
    )
}

#[test]
fn clearing_the_indexes_of_a_record_drops_every_entry_it_held() {
    new_test_ext().execute_with(|| {
        install_three();
        crate::index::clear::<Test>(&slot(), &permits(), &key("P-1"), &permit("P-1", "Anitha"));
        assert_eq!(holders("Anitha"), vec![key("Q-1")]);
        crate::index::clear::<Test>(&slot(), &permits(), &key("Q-1"), &permit("Q-1", "Anitha"));
        assert!(holders("Anitha").is_empty());
        assert_eq!(holders("Benny"), vec![key("P-2")]);
    });
}

#[test]
fn a_repeated_index_entry_for_one_key_is_held_once() {
    new_test_ext().execute_with(|| {
        install_three();
        assert_ok!(crate::index::maintain::<Test>(
            &slot(),
            &permits(),
            &key("P-2"),
            None,
            Some(&permit("P-2", "Benny")),
        ));
        assert_eq!(holders("Benny"), vec![key("P-2")]);
    });
}

#[test]
fn a_definition_past_the_index_bound_declines_maintenance() {
    new_test_ext().execute_with(|| {
        install();
        let mut crowded = permits();
        // The mock admits eight indexes per collection.
        for number in 0..9 {
            crowded.indexes.push(Index {
                name: IndexName::new(&format!("by_holder_{number}")),
                fields: vec![FieldPath::parse("holder")],
                unique: false,
            });
        }
        assert_noop!(
            crate::index::maintain::<Test>(
                &slot(),
                &crowded,
                &key("P-1"),
                None,
                Some(&permit("P-1", "Anitha")),
            ),
            Error::<Test>::IndexLimitReached
        );
    });
}

#[test]
fn an_index_entry_past_its_bound_declines_the_write() {
    new_test_ext().execute_with(|| {
        install();
        // The mock holds one hundred and twenty eight keys per index entry.
        for number in 0..128u32 {
            insert(&permit(&format!("P-{number:03}"), "Anitha"));
        }
        assert_noop!(
            State::insert(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name(),
                bounded(permit("P-999", "Anitha").canonical_bytes())
            ),
            Error::<Test>::IndexFull
        );
        assert_eq!(holders("Anitha").len(), 128);
    });
}

#[test]
fn a_collection_the_schema_lacks_carries_no_store_root() {
    new_test_ext().execute_with(|| {
        install();
        assert_eq!(
            <Pallet<Test> as RecordStore>::root(&DEPLOYMENT, &CollectionId::new("ledger")),
            None
        );
        assert_eq!(
            <Pallet<Test> as RecordStore>::root(&DEPLOYMENT, &id()),
            Some(crate::proof::empty_root())
        );
    });
}
