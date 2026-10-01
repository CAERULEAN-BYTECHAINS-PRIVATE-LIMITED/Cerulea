//! The tests of the collections module.

use crate::{
    keys, mock::*, Classes, Commitments, Error, KeyEpochs, Pallet, PendingChange, PrivacyLevel,
    PrivacyRegistry,
};
use cerulea_blueprint::{
    collections::{Collection, PrivacyClass, PurposeDeclaration},
    types::{
        CollectionId, FieldDef, FieldName, RecordDef, RoleId, TypeDef, TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::primitives::{
    name_of, BoundedName, CollectionKey, DeploymentId, RecordKey,
};
use pallet_cerulea_state::RecordStore;

pub(crate) const DEPLOYMENT: DeploymentId = [5u8; 32];
pub(crate) const STEWARD: u64 = 1;
pub(crate) const CONTENT_KEY: [u8; 32] = [11u8; 32];
pub(crate) const NEXT_KEY: [u8; 32] = [22u8; 32];
pub(crate) const REGISTRAR_KEY: [u8; 32] = [33u8; 32];
pub(crate) const CLERK_KEY: [u8; 32] = [44u8; 32];

pub(crate) fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

pub(crate) fn parcel_collection() -> Collection {
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

pub(crate) fn parcel(survey: &str, holder: &str) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
        ],
    );
    value.canonicalise();
    value
}

pub(crate) fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

pub(crate) fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

pub(crate) fn id() -> CollectionId {
    CollectionId::new("parcel")
}

pub(crate) fn slot() -> CollectionKey {
    CollectionKey::new(DEPLOYMENT, name("parcel"))
}

pub(crate) fn install() {
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

pub(crate) fn insert(record: &Value) -> RecordKey {
    <State as RecordStore>::insert(&DEPLOYMENT, &id(), &STEWARD, record).expect("the record lands")
}

pub(crate) fn holders(
    names: &[&str],
) -> BoundedVec<BoundedName, frame_support::traits::ConstU32<8>> {
    BoundedVec::try_from(names.iter().map(|held| name(held)).collect::<Vec<_>>())
        .expect("the holder list fits")
}

pub(crate) fn seal_collection() {
    assert_ok!(Collections::set_class(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("parcel"),
        PrivacyLevel::Encrypted,
        holders(&["registrar"]),
        None,
        Some(CONTENT_KEY)
    ));
}

#[test]
fn the_mock_runtime_implements_every_module_trait() {
    // The mock and the traits stay in step. A trait that grows past its
    // mock stops this test target from compiling.
    fn requires_privacy<P: PrivacyRegistry>() {}
    fn requires_store<S: RecordStore>() {}
    fn requires_gate<G: pallet_cerulea_schema::access::PermissionGate>() {}
    requires_privacy::<Pallet<Test>>();
    requires_store::<State>();
    requires_gate::<OpenGate>();
    assert!(!sample_denial().fix.is_empty());
}

#[test]
fn a_collection_carries_the_class_its_blueprint_declares() {
    new_test_ext().execute_with(|| {
        install();
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::Open
        );
    });
}

#[test]
fn a_declared_encrypted_class_reads_back_from_the_blueprint() {
    new_test_ext().execute_with(|| {
        assert_ok!(Schema::register_type(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(parcel_type().encode())
        ));
        let mut collection = parcel_collection();
        collection.privacy = PrivacyClass::Encrypted {
            key_holders: vec![RoleId::new("registrar")],
        };
        assert_ok!(Schema::register_collection(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            bounded(collection.encode())
        ));
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::Encrypted
        );
    });
}

#[test]
fn a_class_change_to_hash_only_replaces_the_record_with_its_commitment() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        let plain = record.canonical_bytes();

        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));

        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::HashOnly
        );
        let stored = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        assert_eq!(stored, keys::commitment(&plain).to_vec());
        assert!(<Pallet<Test> as PrivacyRegistry>::verify_commitment(
            &DEPLOYMENT,
            &id(),
            &key,
            &plain
        ));
        assert!(!<Pallet<Test> as PrivacyRegistry>::verify_commitment(
            &DEPLOYMENT,
            &id(),
            &key,
            b"another payload"
        ));
    });
}

#[test]
fn a_commitment_check_writes_the_check_into_the_record() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        let plain = record.canonical_bytes();
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::External,
            holders(&[]),
            None,
            None
        ));
        assert_ok!(Collections::verify_commitment(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            key.clone(),
            bounded(plain)
        ));
        assert_noop!(
            Collections::verify_commitment(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                key,
                bounded(b"another payload".to_vec())
            ),
            Error::<Test>::CommitmentMismatch
        );
    });
}

#[test]
fn a_commitment_check_without_a_stored_commitment_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        let key = insert(&parcel("12/3", "Anitha"));
        assert_noop!(
            Collections::verify_commitment(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                key,
                bounded(b"a payload".to_vec())
            ),
            Error::<Test>::CommitmentAbsent
        );
    });
}

#[test]
fn a_record_in_an_encrypted_collection_is_unreadable_without_the_role_key() {
    // Blueprint assertion seven. A record in an encrypted collection is
    // unreadable without the role key.
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();

        let stored = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        assert_ne!(stored, record.canonical_bytes());
        assert!(<State as RecordStore>::read(&DEPLOYMENT, &id(), &key).is_none());

        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        let opened = Pallet::<Test>::open_record(
            &DEPLOYMENT,
            &id(),
            &key,
            &name("registrar"),
            &REGISTRAR_KEY,
        )
        .expect("the role key opens the record");
        assert_eq!(opened, record.canonical_bytes());

        assert_noop!(
            Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name("registrar"), &CLERK_KEY),
            Error::<Test>::RoleKeyMismatch
        );
        assert_noop!(
            Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name("clerk"), &CLERK_KEY),
            Error::<Test>::RoleWithoutKey
        );
    });
}

#[test]
fn a_class_change_re_encrypts_affected_records_and_rotates_keys() {
    // Design assertion four. A privacy class change re-encrypts affected
    // records and rotates keys.
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        let first_epoch = KeyEpochs::<Test>::get(slot());
        let first_sealed = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        let first_wrapped = Pallet::<Test>::wrapped_key(&DEPLOYMENT, &id(), &name("registrar"))
            .expect("the role holds a wrapped key");

        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Encrypted,
            holders(&["registrar"]),
            Some(CONTENT_KEY),
            Some(NEXT_KEY)
        ));
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            NEXT_KEY
        ));

        let second_epoch = KeyEpochs::<Test>::get(slot());
        assert!(second_epoch > first_epoch);
        let second_sealed = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        assert_ne!(first_sealed, second_sealed);
        let second_wrapped = Pallet::<Test>::wrapped_key(&DEPLOYMENT, &id(), &name("registrar"))
            .expect("the role holds a wrapped key");
        assert_ne!(first_wrapped, second_wrapped);

        let opened = Pallet::<Test>::open_record(
            &DEPLOYMENT,
            &id(),
            &key,
            &name("registrar"),
            &REGISTRAR_KEY,
        )
        .expect("the rotated key opens the record");
        assert_eq!(opened, record.canonical_bytes());
    });
}

#[test]
fn a_sealed_collection_returns_to_plain_form_with_the_content_key() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Open,
            holders(&[]),
            Some(CONTENT_KEY),
            None
        ));
        assert_eq!(
            <State as RecordStore>::read(&DEPLOYMENT, &id(), &key),
            Some(record)
        );
    });
}

#[test]
fn a_class_change_declines_a_wrong_content_key() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::Open,
                holders(&[]),
                Some(NEXT_KEY),
                None
            ),
            Error::<Test>::ContentKeyMismatch
        );
    });
}

#[test]
fn a_change_to_a_sealed_class_without_a_key_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::Encrypted,
                holders(&["registrar"]),
                None,
                None
            ),
            Error::<Test>::ContentKeyMissing
        );
    });
}

#[test]
fn a_commitment_class_declines_a_return_to_plain_form() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::Open,
                holders(&[]),
                None,
                None
            ),
            Error::<Test>::PayloadAbsent
        );
    });
}

#[test]
fn a_class_change_runs_in_pages_and_takes_effect_on_the_last_page() {
    new_test_ext().execute_with(|| {
        install();
        let records: Vec<Value> = ["12/3", "44/9", "07/1", "88/2", "55/5"]
            .iter()
            .map(|survey| parcel(survey, "Anitha"))
            .collect();
        let mut stored_keys = Vec::new();
        for record in records.iter() {
            stored_keys.push(insert(record));
        }

        // The page limit of the test runtime is two records.
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Encrypted,
            holders(&["registrar"]),
            None,
            Some(CONTENT_KEY)
        ));
        assert!(<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &id()
        ));
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::Open
        );

        assert_ok!(Collections::advance_class_change(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            None,
            Some(CONTENT_KEY)
        ));
        assert!(<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &id()
        ));

        assert_ok!(Collections::advance_class_change(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            None,
            Some(CONTENT_KEY)
        ));
        assert!(!<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &id()
        ));
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::Encrypted
        );

        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        for (position, key) in stored_keys.iter().enumerate() {
            let opened = Pallet::<Test>::open_record(
                &DEPLOYMENT,
                &id(),
                key,
                &name("registrar"),
                &REGISTRAR_KEY,
            )
            .expect("the role key opens every record");
            assert!(records.iter().any(|held| held.canonical_bytes() == opened));
            let _ = position;
        }
    });
}

#[test]
fn a_second_change_waits_for_the_first_to_finish() {
    new_test_ext().execute_with(|| {
        install();
        for survey in ["12/3", "44/9", "07/1"] {
            insert(&parcel(survey, "Anitha"));
        }
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::External,
                holders(&[]),
                None,
                None
            ),
            Error::<Test>::ChangePending
        );
    });
}

#[test]
fn an_advance_without_a_pending_change_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                None,
                None
            ),
            Error::<Test>::ChangeAbsent
        );
    });
}

#[test]
fn a_revoked_role_loses_its_wrapped_key() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        assert_ok!(Collections::revoke_access(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar")
        ));
        assert!(Pallet::<Test>::wrapped_key(&DEPLOYMENT, &id(), &name("registrar")).is_none());
        assert_noop!(
            Pallet::<Test>::open_record(
                &DEPLOYMENT,
                &id(),
                &key,
                &name("registrar"),
                &REGISTRAR_KEY
            ),
            Error::<Test>::RoleWithoutKey
        );
        assert!(<Pallet<Test> as PrivacyRegistry>::key_holders(&DEPLOYMENT, &id()).is_empty());
    });
}

#[test]
fn a_revoke_without_a_key_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        assert_noop!(
            Collections::revoke_access(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                name("clerk")
            ),
            Error::<Test>::RoleWithoutKey
        );
    });
}

#[test]
fn a_distribution_declines_a_wrong_content_key() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        assert_noop!(
            Collections::distribute_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                name("registrar"),
                REGISTRAR_KEY,
                NEXT_KEY
            ),
            Error::<Test>::ContentKeyMismatch
        );
    });
}

#[test]
fn a_distribution_declines_a_changed_role_key() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        assert_noop!(
            Collections::distribute_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                name("registrar"),
                CLERK_KEY,
                CONTENT_KEY
            ),
            Error::<Test>::RoleKeyMismatch
        );
    });
}

#[test]
fn two_roles_each_reach_the_content_key() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        for (role, role_key) in [("registrar", REGISTRAR_KEY), ("clerk", CLERK_KEY)] {
            assert_ok!(Collections::distribute_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                name(role),
                role_key,
                CONTENT_KEY
            ));
            let opened =
                Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name(role), &role_key)
                    .expect("the role key opens the record");
            assert_eq!(opened, record.canonical_bytes());
        }
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::key_holders(&DEPLOYMENT, &id()).len(),
            2
        );
    });
}

#[test]
fn a_pending_change_carries_its_cursor_and_its_count() {
    new_test_ext().execute_with(|| {
        install();
        for survey in ["12/3", "44/9", "07/1"] {
            insert(&parcel(survey, "Anitha"));
        }
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        let change: PendingChange =
            crate::Changes::<Test>::get(slot()).expect("the change is part way through");
        assert_eq!(change.target, PrivacyLevel::HashOnly);
        assert_eq!(change.from, PrivacyLevel::Open);
        assert_eq!(change.rewritten, 2);
        assert!(change.cursor.is_some());
    });
}

#[test]
fn a_class_change_on_an_absent_collection_names_the_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::HashOnly,
                holders(&[]),
                None,
                None
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn the_stored_class_overrides_the_declared_class() {
    new_test_ext().execute_with(|| {
        install();
        assert!(Classes::<Test>::get(slot()).is_none());
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::External,
            holders(&[]),
            None,
            None
        ));
        assert_eq!(Classes::<Test>::get(slot()), Some(PrivacyLevel::External));
        assert!(Commitments::<Test>::iter_prefix(slot()).next().is_none());
    });
}

#[test]
fn every_class_carries_a_label_and_a_shape() {
    assert_eq!(PrivacyLevel::Open.label(), "open");
    assert_eq!(PrivacyLevel::HashOnly.label(), "hash only");
    assert_eq!(PrivacyLevel::Encrypted.label(), "encrypted");
    assert_eq!(PrivacyLevel::External.label(), "external");
    assert!(PrivacyLevel::Open.is_plain());
    assert!(PrivacyLevel::Encrypted.is_sealed());
    assert!(PrivacyLevel::HashOnly.is_commitment());
    assert!(PrivacyLevel::External.is_commitment());
    assert_eq!(
        PrivacyLevel::of(&PrivacyClass::HashOnly),
        PrivacyLevel::HashOnly
    );
}

pub(crate) fn holder_keys(
    pairs: &[(&str, [u8; 32])],
) -> BoundedVec<(BoundedName, [u8; 32]), frame_support::traits::ConstU32<8>> {
    BoundedVec::try_from(
        pairs
            .iter()
            .map(|(role, key)| (name(role), *key))
            .collect::<Vec<_>>(),
    )
    .expect("the holder list fits")
}

#[test]
fn a_rotation_re_seals_every_record_and_re_wraps_every_holder() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        let before = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        let first_epoch = KeyEpochs::<Test>::get(slot());

        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY), ("clerk", CLERK_KEY)])
        ));

        assert_eq!(KeyEpochs::<Test>::get(slot()), first_epoch + 1);
        let after = <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key)
            .expect("the store holds the record");
        assert_ne!(before, after);

        for (role, role_key) in [("registrar", REGISTRAR_KEY), ("clerk", CLERK_KEY)] {
            let opened =
                Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name(role), &role_key)
                    .expect("the rotated key opens the record");
            assert_eq!(opened, record.canonical_bytes());
        }
    });
}

#[test]
fn a_rotation_leaves_out_a_role_and_that_role_reads_nothing() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        for (role, role_key) in [("registrar", REGISTRAR_KEY), ("clerk", CLERK_KEY)] {
            assert_ok!(Collections::distribute_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                name(role),
                role_key,
                CONTENT_KEY
            ));
        }
        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));

        assert_ok!(Pallet::<Test>::open_record(
            &DEPLOYMENT,
            &id(),
            &key,
            &name("registrar"),
            &REGISTRAR_KEY
        ));
        assert_noop!(
            Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name("clerk"), &CLERK_KEY),
            Error::<Test>::RoleWithoutKey
        );
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::key_holders(&DEPLOYMENT, &id()),
            vec![name("registrar")]
        );
    });
}

#[test]
fn a_rotation_keeps_the_recent_epoch_and_retires_the_older_one() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_ok!(Collections::distribute_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("registrar"),
            REGISTRAR_KEY,
            CONTENT_KEY
        ));
        let first = KeyEpochs::<Test>::get(slot());

        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));
        // The history depth of the test runtime keeps one epoch behind the
        // live epoch.
        assert!(
            Pallet::<Test>::wrapped_key_at(&DEPLOYMENT, &id(), &name("registrar"), first).is_some()
        );

        let third: [u8; 32] = [99u8; 32];
        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            NEXT_KEY,
            third,
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));
        assert!(
            Pallet::<Test>::wrapped_key_at(&DEPLOYMENT, &id(), &name("registrar"), first).is_none()
        );
    });
}

#[test]
fn a_rotation_of_a_plain_collection_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                CONTENT_KEY,
                NEXT_KEY,
                holder_keys(&[("registrar", REGISTRAR_KEY)])
            ),
            Error::<Test>::ClassNotSealed
        );
    });
}

#[test]
fn a_rotation_declines_the_same_key_twice() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                CONTENT_KEY,
                CONTENT_KEY,
                holder_keys(&[("registrar", REGISTRAR_KEY)])
            ),
            Error::<Test>::KeyUnchanged
        );
    });
}

#[test]
fn a_rotation_declines_a_wrong_present_key() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                [77u8; 32],
                NEXT_KEY,
                holder_keys(&[("registrar", REGISTRAR_KEY)])
            ),
            Error::<Test>::ContentKeyMismatch
        );
    });
}

#[test]
fn a_rotation_over_several_pages_finishes_with_every_record_re_sealed() {
    new_test_ext().execute_with(|| {
        install();
        let records: Vec<Value> = ["12/3", "44/9", "07/1", "88/2", "55/5"]
            .iter()
            .map(|survey| parcel(survey, "Anitha"))
            .collect();
        let mut stored = Vec::new();
        for record in records.iter() {
            stored.push(insert(record));
        }
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Encrypted,
            holders(&["registrar"]),
            None,
            Some(CONTENT_KEY)
        ));
        while <Pallet<Test> as PrivacyRegistry>::change_pending(&DEPLOYMENT, &id()) {
            assert_ok!(Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                None,
                Some(CONTENT_KEY)
            ));
        }

        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));
        while <Pallet<Test> as PrivacyRegistry>::change_pending(&DEPLOYMENT, &id()) {
            assert_ok!(Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                Some(CONTENT_KEY),
                Some(NEXT_KEY)
            ));
        }

        for key in stored.iter() {
            let opened = Pallet::<Test>::open_record(
                &DEPLOYMENT,
                &id(),
                key,
                &name("registrar"),
                &REGISTRAR_KEY,
            )
            .expect("the rotated key opens every record");
            assert!(records.iter().any(|held| held.canonical_bytes() == opened));
        }
    });
}

#[test]
fn a_rotation_waits_for_a_pending_class_change() {
    new_test_ext().execute_with(|| {
        install();
        for survey in ["12/3", "44/9", "07/1"] {
            insert(&parcel(survey, "Anitha"));
        }
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Encrypted,
            holders(&["registrar"]),
            None,
            Some(CONTENT_KEY)
        ));
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                CONTENT_KEY,
                NEXT_KEY,
                holder_keys(&[("registrar", REGISTRAR_KEY)])
            ),
            Error::<Test>::ChangePending
        );
    });
}
