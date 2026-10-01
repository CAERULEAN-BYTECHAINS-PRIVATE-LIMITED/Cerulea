//! Further tests of the collections module.
//!
//! These tests cover the refusal branches of every call, the reads a
//! dependant module performs through the privacy registry, and the events
//! each call writes.

use crate::{
    keys, mock::*, tests::*, Changes, Commitments, Error, Event, KeyEpochs, KeyHolders, Pallet,
    PrivacyLevel, PrivacyRegistry,
};
use cerulea_blueprint::types::CollectionId;
use frame_support::{assert_noop, assert_ok, dispatch::DispatchResult, BoundedVec};
use pallet_cerulea_schema::primitives::{BoundedName, RecordKey};
use pallet_cerulea_state::RecordStore;

/// A collection name holding bytes that fail to read as text.
fn unreadable_name() -> BoundedName {
    BoundedVec::try_from(vec![0xff, 0xfe]).expect("the name fits")
}

/// A collection identifier past the 128 byte name limit.
fn long_id() -> CollectionId {
    CollectionId::new(&"p".repeat(200))
}

fn events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Collections(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

fn distribute(role: &str, role_key: [u8; 32], content_key: [u8; 32]) -> DispatchResult {
    Collections::distribute_key(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("parcel"),
        name(role),
        role_key,
        content_key,
    )
}

fn clerk_role(place: u8) -> String {
    let mut held = String::from("clerk_");
    held.push(char::from(b'a' + place));
    held
}

fn land_five() {
    for survey in ["12/3", "44/9", "07/1", "88/2", "55/5"] {
        insert(&parcel(survey, "Anitha"));
    }
}

#[test]
fn a_collection_name_that_fails_to_read_as_text_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                unreadable_name(),
                PrivacyLevel::HashOnly,
                holders(&[]),
                None,
                None
            ),
            Error::<Test>::CollectionUnknown
        );
        assert_noop!(
            Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                unreadable_name(),
                None,
                None
            ),
            Error::<Test>::CollectionUnknown
        );
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                unreadable_name(),
                CONTENT_KEY,
                NEXT_KEY,
                holder_keys(&[])
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_rotation_over_an_absent_collection_names_the_fix() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Collections::rotate_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                CONTENT_KEY,
                NEXT_KEY,
                holder_keys(&[("registrar", REGISTRAR_KEY)])
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_change_away_from_a_sealed_class_without_the_present_key_names_the_fix() {
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
                None,
                None
            ),
            Error::<Test>::ContentKeyMissing
        );
    });
}

#[test]
fn a_change_between_two_sealed_epochs_re_seals_every_record() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        seal_collection();
        let first = KeyEpochs::<Test>::get(slot());
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::Encrypted,
            holders(&["registrar"]),
            Some(CONTENT_KEY),
            Some(NEXT_KEY)
        ));
        assert_eq!(KeyEpochs::<Test>::get(slot()), first + 1);

        // The earlier content key belongs to the earlier epoch. The new key
        // reaches the registrar and opens the record.
        assert_noop!(
            distribute("registrar", REGISTRAR_KEY, CONTENT_KEY),
            Error::<Test>::ContentKeyMismatch
        );
        assert_ok!(distribute("registrar", REGISTRAR_KEY, NEXT_KEY));
        let opened = Pallet::<Test>::open_record(
            &DEPLOYMENT,
            &id(),
            &key,
            &name("registrar"),
            &REGISTRAR_KEY,
        )
        .expect("the new key opens the record");
        assert_eq!(opened, record.canonical_bytes());
    });
}

#[test]
fn a_record_left_in_plain_form_inside_a_sealed_collection_stops_the_change() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        // A record written after the seal lands in plain form. The content
        // key fails to open that record, so the return to plain form stops.
        insert(&parcel("44/9", "Benny"));
        assert_noop!(
            Collections::set_class(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                PrivacyLevel::Open,
                holders(&[]),
                Some(CONTENT_KEY),
                None
            ),
            Error::<Test>::SealUnreadable
        );
    });
}

#[test]
fn an_advance_of_a_sealing_change_checks_the_next_key() {
    new_test_ext().execute_with(|| {
        install();
        land_five();
        seal_collection();
        assert!(Changes::<Test>::get(slot()).is_some());
        assert_noop!(
            Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                None,
                Some(NEXT_KEY)
            ),
            Error::<Test>::ContentKeyMismatch
        );
        assert_noop!(
            Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                None,
                None
            ),
            Error::<Test>::ContentKeyMissing
        );
        assert!(<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &id()
        ));
        for _ in 0..2 {
            assert_ok!(Collections::advance_class_change(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("parcel"),
                None,
                Some(CONTENT_KEY)
            ));
        }
        assert!(!<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &id()
        ));
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &id()),
            PrivacyLevel::Encrypted
        );
        assert!(events()
            .iter()
            .any(|held| matches!(held, Event::ClassChangeAdvanced { rewritten: 4, .. })));
        assert!(events().iter().any(|held| matches!(
            held,
            Event::ClassChanged {
                class: PrivacyLevel::Encrypted,
                rewritten: 5,
                ..
            }
        )));
    });
}

#[test]
fn a_distribution_past_the_holder_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        // The seal names the registrar. Seven more roles fill the eight
        // places the test runtime admits.
        for place in 0..7u8 {
            assert_ok!(distribute(&clerk_role(place), [place; 32], CONTENT_KEY));
        }
        assert_eq!(KeyHolders::<Test>::get(slot()).len(), 8);
        // A repeat distribution to a present holder takes no further place.
        assert_ok!(distribute("registrar", REGISTRAR_KEY, CONTENT_KEY));
        assert_eq!(KeyHolders::<Test>::get(slot()).len(), 8);
        assert_noop!(
            distribute("auditor", CLERK_KEY, CONTENT_KEY),
            Error::<Test>::HolderLimitReached
        );
    });
}

#[test]
fn a_distribution_and_a_revocation_write_their_events() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        let epoch = KeyEpochs::<Test>::get(slot());
        assert_ok!(distribute("clerk", CLERK_KEY, CONTENT_KEY));
        System::assert_has_event(RuntimeEvent::Collections(Event::KeyDistributed {
            deployment: DEPLOYMENT,
            collection: name("parcel"),
            role: name("clerk"),
            epoch,
        }));
        assert_ok!(Collections::revoke_access(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            name("clerk")
        ));
        System::assert_last_event(RuntimeEvent::Collections(Event::AccessRevoked {
            deployment: DEPLOYMENT,
            collection: name("parcel"),
            role: name("clerk"),
            epoch,
        }));
        assert!(
            Pallet::<Test>::wrapped_key_at(&DEPLOYMENT, &id(), &name("clerk"), epoch).is_none()
        );
    });
}

#[test]
fn an_open_collection_hands_back_the_stored_record_to_any_role() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        let opened =
            Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name("clerk"), &CLERK_KEY)
                .expect("an open record reads back");
        assert_eq!(opened, record.canonical_bytes());
    });
}

#[test]
fn opening_a_record_the_store_lacks_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        let absent: RecordKey = BoundedVec::try_from(b"absent".to_vec()).expect("the key fits");
        assert_noop!(
            Pallet::<Test>::open_record(
                &DEPLOYMENT,
                &id(),
                &absent,
                &name("registrar"),
                &REGISTRAR_KEY
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

#[test]
fn opening_a_record_with_the_wrong_role_key_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        let key = insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_ok!(distribute("registrar", REGISTRAR_KEY, CONTENT_KEY));
        assert_noop!(
            Pallet::<Test>::open_record(&DEPLOYMENT, &id(), &key, &name("registrar"), &CLERK_KEY),
            Error::<Test>::RoleKeyMismatch
        );
    });
}

#[test]
fn opening_a_record_under_a_name_past_the_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        let key = insert(&parcel("12/3", "Anitha"));
        assert_noop!(
            Pallet::<Test>::open_record(
                &DEPLOYMENT,
                &long_id(),
                &key,
                &name("registrar"),
                &REGISTRAR_KEY
            ),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn a_name_past_the_limit_reads_as_an_open_collection_without_keys() {
    new_test_ext().execute_with(|| {
        install();
        let key = insert(&parcel("12/3", "Anitha"));
        let long = long_id();
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::class(&DEPLOYMENT, &long),
            PrivacyLevel::Open
        );
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::key_epoch(&DEPLOYMENT, &long),
            0
        );
        assert!(<Pallet<Test> as PrivacyRegistry>::key_holders(&DEPLOYMENT, &long).is_empty());
        assert!(<Pallet<Test> as PrivacyRegistry>::commitment(&DEPLOYMENT, &long, &key).is_none());
        assert!(!<Pallet<Test> as PrivacyRegistry>::verify_commitment(
            &DEPLOYMENT,
            &long,
            &key,
            b"payload"
        ));
        assert!(!<Pallet<Test> as PrivacyRegistry>::change_pending(
            &DEPLOYMENT,
            &long
        ));
        assert!(Pallet::<Test>::wrapped_key(&DEPLOYMENT, &long, &name("registrar")).is_none());
        assert!(
            Pallet::<Test>::wrapped_key_at(&DEPLOYMENT, &long, &name("registrar"), 1).is_none()
        );
    });
}

#[test]
fn an_unregistered_collection_reads_as_open() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            Pallet::<Test>::level(&DEPLOYMENT, &CollectionId::new("ledger")),
            PrivacyLevel::Open
        );
    });
}

#[test]
fn the_registry_reports_the_live_epoch_and_the_holders() {
    new_test_ext().execute_with(|| {
        install();
        seal_collection();
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::key_epoch(&DEPLOYMENT, &id()),
            1
        );
        assert_eq!(
            <Pallet<Test> as PrivacyRegistry>::key_holders(&DEPLOYMENT, &id()),
            vec![name("registrar")]
        );
    });
}

#[test]
fn a_move_between_two_commitment_classes_keeps_a_commitment_per_record() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        let first = Commitments::<Test>::get(slot(), &key).expect("a commitment lands");
        assert_eq!(first, keys::commitment(&record.canonical_bytes()));
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::External,
            holders(&[]),
            None,
            None
        ));
        // The store held the commitment, so the external class commits to
        // that commitment.
        let second = Commitments::<Test>::get(slot(), &key).expect("a commitment lands");
        assert_eq!(second, keys::commitment(&first));
        assert_eq!(
            <State as RecordStore>::read_bytes(&DEPLOYMENT, &id(), &key),
            Some(second.to_vec())
        );
    });
}

#[test]
fn a_move_from_a_commitment_class_to_a_sealed_class_seals_the_commitment() {
    new_test_ext().execute_with(|| {
        install();
        let key = insert(&parcel("12/3", "Anitha"));
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        let held = Commitments::<Test>::get(slot(), &key).expect("a commitment lands");
        seal_collection();
        assert!(Commitments::<Test>::get(slot(), &key).is_none());
        assert_ok!(distribute("registrar", REGISTRAR_KEY, CONTENT_KEY));
        let opened = Pallet::<Test>::open_record(
            &DEPLOYMENT,
            &id(),
            &key,
            &name("registrar"),
            &REGISTRAR_KEY,
        )
        .expect("the content key opens the sealed commitment");
        assert_eq!(opened, held.to_vec());
    });
}

#[test]
fn a_second_rotation_retires_the_oldest_epoch_and_writes_the_event() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_ok!(distribute("registrar", REGISTRAR_KEY, CONTENT_KEY));
        let first = KeyEpochs::<Test>::get(slot());
        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));
        System::assert_has_event(RuntimeEvent::Collections(Event::KeyRotated {
            deployment: DEPLOYMENT,
            collection: name("parcel"),
            from_epoch: first,
            to_epoch: first + 1,
        }));
        assert!(!events()
            .iter()
            .any(|held| matches!(held, Event::EpochRetired { .. })));
        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            NEXT_KEY,
            [99u8; 32],
            holder_keys(&[("registrar", REGISTRAR_KEY)])
        ));
        System::assert_has_event(RuntimeEvent::Collections(Event::EpochRetired {
            deployment: DEPLOYMENT,
            collection: name("parcel"),
            epoch: first,
        }));
    });
}

#[test]
fn a_rotation_naming_one_role_twice_keeps_one_holder_entry() {
    new_test_ext().execute_with(|| {
        install();
        insert(&parcel("12/3", "Anitha"));
        seal_collection();
        assert_ok!(Collections::rotate_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            CONTENT_KEY,
            NEXT_KEY,
            holder_keys(&[("registrar", REGISTRAR_KEY), ("registrar", REGISTRAR_KEY)])
        ));
        assert_eq!(
            KeyHolders::<Test>::get(slot()).into_inner(),
            vec![name("registrar")]
        );
    });
}

#[test]
fn a_matching_payload_writes_the_verification_event() {
    new_test_ext().execute_with(|| {
        install();
        let record = parcel("12/3", "Anitha");
        let key = insert(&record);
        assert_ok!(Collections::set_class(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            PrivacyLevel::HashOnly,
            holders(&[]),
            None,
            None
        ));
        assert_ok!(Collections::verify_commitment(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel"),
            key.clone(),
            bounded(record.canonical_bytes())
        ));
        System::assert_last_event(RuntimeEvent::Collections(Event::CommitmentVerified {
            deployment: DEPLOYMENT,
            collection: name("parcel"),
            key,
        }));
    });
}

#[test]
fn every_declared_class_maps_to_its_stored_class() {
    use cerulea_blueprint::{collections::PrivacyClass, types::RoleId};
    assert_eq!(PrivacyLevel::of(&PrivacyClass::Open), PrivacyLevel::Open);
    assert_eq!(
        PrivacyLevel::of(&PrivacyClass::Encrypted {
            key_holders: vec![RoleId::new("registrar")]
        }),
        PrivacyLevel::Encrypted
    );
    assert!(!PrivacyLevel::Open.is_sealed());
    assert!(!PrivacyLevel::Open.is_commitment());
    assert!(!PrivacyLevel::Encrypted.is_plain());
}
