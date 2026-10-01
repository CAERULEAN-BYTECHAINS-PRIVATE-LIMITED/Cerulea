//! Further tests of the permission module.
//!
//! These tests cover the refusal branches of the matrix calls, the plain
//! sentences every denial carries, the merge of several entries and the
//! statements the Studio previews read.

use crate::{
    mock::*, tests::*, CapabilityKey, EntryCount, Error, Event, GrantEntry, GrantPayload, Matrix,
    Pallet, PermissionMatrix, RoleSimulation,
};
use cerulea_blueprint::{
    permissions::FieldQualification,
    types::{ActionId, CollectionId, FieldPath, PurposeTag, Value},
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest, PermissionGate},
    primitives::BoundedName,
};

fn unreadable_name() -> BoundedName {
    BoundedVec::try_from(vec![0xff, 0xfe]).expect("the name fits")
}

fn long_text() -> String {
    "p".repeat(200)
}

fn simulation(read: bool, insert: bool, actions: &[&str]) -> RoleSimulation {
    RoleSimulation {
        role: name("registrar"),
        collection: name("parcel"),
        read,
        insert,
        update: false,
        delete: false,
        readable_fields: FieldQualification::EveryField,
        row_predicate: None,
        actions: actions.iter().map(|held| name(held)).collect(),
        purposes: Vec::new(),
    }
}

fn fields(names: &[&str]) -> Vec<FieldPath> {
    names.iter().map(|held| FieldPath::parse(held)).collect()
}

#[test]
fn a_grant_under_a_name_that_fails_to_read_as_text_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Permission::grant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                unreadable_name(),
                CapabilityKey::Read,
                bounded(GrantPayload::everything().encode())
            ),
            Error::<Test>::CollectionUnknown
        );
    });
}

#[test]
fn a_grant_carrying_an_unreadable_payload_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Permission::grant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                name("parcel"),
                CapabilityKey::Read,
                bounded(vec![9u8, 9, 9])
            ),
            Error::<Test>::EntryUnreadable
        );
    });
}

#[test]
fn a_pair_past_its_entry_limit_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        for capability in [
            CapabilityKey::Read,
            CapabilityKey::Insert,
            CapabilityKey::Update,
            CapabilityKey::Delete,
        ] {
            grant("registrar", capability, GrantPayload::everything());
        }
        for place in 0..12u8 {
            let mut action = String::from("action_");
            action.push(char::from(b'a' + place));
            grant(
                "registrar",
                CapabilityKey::Invoke(name(&action)),
                GrantPayload::everything(),
            );
        }
        assert_eq!(
            EntryCount::<Test>::get((DEPLOYMENT, name("registrar"), name("parcel"))),
            16
        );
        assert_noop!(
            Permission::grant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name("registrar"),
                name("parcel"),
                CapabilityKey::Invoke(name("action_extra")),
                bounded(GrantPayload::everything().encode())
            ),
            Error::<Test>::EntryLimitReached
        );
        // A revocation frees one place.
        assert_ok!(Permission::revoke(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("registrar"),
            name("parcel"),
            CapabilityKey::Delete
        ));
        assert_eq!(
            EntryCount::<Test>::get((DEPLOYMENT, name("registrar"), name("parcel"))),
            15
        );
        grant(
            "registrar",
            CapabilityKey::Invoke(name("action_extra")),
            GrantPayload::everything(),
        );
    });
}

#[test]
fn a_grant_and_a_revocation_write_their_events() {
    new_test_ext().execute_with(|| {
        install();
        grant("clerk", CapabilityKey::Update, GrantPayload::everything());
        System::assert_last_event(RuntimeEvent::Permission(Event::CapabilityGranted {
            deployment: DEPLOYMENT,
            role: name("clerk"),
            collection: name("parcel"),
            capability: CapabilityKey::Update,
        }));
        let held = Pallet::<Test>::entry(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Update,
        )
        .expect("the matrix holds the entry");
        assert_eq!(held.granter, STEWARD);
        assert_eq!(held.granted_at, START);
        assert_ok!(Permission::revoke(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("clerk"),
            name("parcel"),
            CapabilityKey::Update
        ));
        System::assert_last_event(RuntimeEvent::Permission(Event::CapabilityRevoked {
            deployment: DEPLOYMENT,
            role: name("clerk"),
            collection: name("parcel"),
            capability: CapabilityKey::Update,
        }));
    });
}

#[test]
fn every_denial_names_the_problem_and_the_fix_verbatim() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(fields(&["survey_number"]))
                .where_rows(ward_predicate("north"))
                .for_purposes(&["audit"]),
        );

        let denial = Pallet::<Test>::resolve(&request(&BENNY, AccessAction::Read))
            .expect_err("an account without a role is refused");
        assert_eq!(
            denial.problem,
            "The account holds no role, so the read of `parcel` is refused."
        );
        assert_eq!(
            denial.fix,
            "Ask the steward to assign a role to the account."
        );

        let denial = Pallet::<Test>::resolve(&request(&ANITHA, AccessAction::Update))
            .expect_err("a role without the capability is refused");
        assert_eq!(
            denial.problem,
            "No role the account holds reaches the update of `parcel`."
        );
        assert_eq!(
            denial.fix,
            "Grant a role of the account the update capability over `parcel`."
        );

        let resale = request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("resale"));
        let denial = Pallet::<Test>::resolve(&resale).expect_err("an undeclared purpose");
        assert_eq!(
            denial.problem,
            "The collection `parcel` declares no purpose `resale`."
        );
        assert_eq!(
            denial.fix,
            "Declare the purpose `resale` on the collection, or read under a declared purpose."
        );

        let registration =
            request(&ANITHA, AccessAction::Read).for_purpose(PurposeTag::new("registration"));
        let denial =
            Pallet::<Test>::resolve(&registration).expect_err("the entry serves audit alone");
        assert_eq!(
            denial.problem,
            "No entry of the account serves the purpose `registration` on `parcel`."
        );
        assert_eq!(
            denial.fix,
            "Grant an entry serving the purpose `registration`."
        );

        let field = request(&ANITHA, AccessAction::Read).on_field(FieldPath::parse("holder"));
        let denial = Pallet::<Test>::resolve(&field).expect_err("a field outside the entry");
        assert_eq!(
            denial.problem,
            "No role the account holds reaches the field `holder` of `parcel`."
        );
        assert_eq!(
            denial.fix,
            "Add `holder` to the field list of an entry of the account."
        );

        let south = parcel("12/3", "Anitha", "south");
        let row = request(&ANITHA, AccessAction::Read).about(&south);
        let denial = Pallet::<Test>::resolve(&row).expect_err("a row outside the predicate");
        assert_eq!(
            denial.problem,
            "The row predicate of every entry declines this record of `parcel`."
        );
        assert_eq!(
            denial.fix,
            "Widen the row predicate, or act on a record the predicate admits."
        );
    });
}

#[test]
fn a_request_naming_names_past_the_limit_is_refused_with_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        let long_action = request(&ANITHA, AccessAction::Invoke(ActionId::new(&long_text())));
        let denial = Pallet::<Test>::resolve(&long_action).expect_err("the action name is long");
        assert_eq!(
            denial.problem,
            "The request names an action whose name runs past the limit."
        );
        assert_eq!(denial.fix, "Shorten the action name in the Blueprint.");

        let long_collection = AccessRequest::new(
            DEPLOYMENT,
            &ANITHA,
            CollectionId::new(&long_text()),
            AccessAction::Read,
            cerulea_blueprint::types::Timestamp::from_millis(START),
        );
        let denial =
            Pallet::<Test>::resolve(&long_collection).expect_err("the collection name is long");
        assert_eq!(
            denial.problem,
            "The request names a collection whose name runs past the limit."
        );
        assert_eq!(denial.fix, "Shorten the collection name in the Blueprint.");
        assert_eq!(
            CapabilityKey::of(&AccessAction::Invoke(ActionId::new(&long_text()))),
            None
        );
    });
}

#[test]
fn a_purpose_read_of_a_collection_the_registry_lacks_names_the_fix() {
    new_test_ext().execute_with(|| {
        install();
        // The matrix holds an entry for a collection the registry dropped.
        Matrix::<Test>::insert(
            (
                DEPLOYMENT,
                name("registrar"),
                name("ledger"),
                CapabilityKey::Read,
            ),
            GrantEntry {
                fields: FieldQualification::EveryField,
                rows: None,
                purposes: Vec::new(),
                granter: STEWARD,
                granted_at: START,
            },
        );
        let held = AccessRequest::new(
            DEPLOYMENT,
            &ANITHA,
            CollectionId::new("ledger"),
            AccessAction::Read,
            cerulea_blueprint::types::Timestamp::from_millis(START),
        );
        assert!(Pallet::<Test>::resolve(&held).is_ok());
        let purposed = held.for_purpose(PurposeTag::new("audit"));
        let denial = Pallet::<Test>::resolve(&purposed).expect_err("the registry lacks it");
        assert_eq!(
            denial.problem,
            "The registry lacks the collection `ledger`."
        );
        assert_eq!(denial.fix, "Register the collection before reading it.");
    });
}

#[test]
fn a_record_preview_names_the_capability_it_lacks_verbatim() {
    new_test_ext().execute_with(|| {
        install();
        let found = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Invoke(name("record_transfer")),
            &parcel("12/3", "Anitha", "north"),
        );
        assert!(!found.visible);
        assert_eq!(
            found.statement(),
            "The role `clerk` reaches no invoke of `parcel`. Grant the role the invoke capability over `parcel`."
        );

        grant(
            "clerk",
            CapabilityKey::Read,
            GrantPayload::everything().where_rows(ward_predicate("north")),
        );
        let declined = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("12/3", "Anitha", "south"),
        );
        assert_eq!(
            declined.statement(),
            "The row predicate of the role `clerk` declines this record. Widen the row predicate, or preview a record the predicate admits."
        );
        let seen = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("clerk"),
            &name("parcel"),
            &CapabilityKey::Read,
            &parcel("12/3", "Anitha", "north"),
        );
        assert!(seen.visible);
        assert_eq!(
            seen.statement(),
            "The role reads the record and reaches `holder`, `survey_number`, `ward`."
        );
    });
}

#[test]
fn a_role_statement_lists_every_capability_it_reaches() {
    assert_eq!(
        simulation(false, false, &[]).statement(),
        "The role `registrar` reaches nothing in `parcel`."
    );
    assert_eq!(
        simulation(true, true, &[]).statement(),
        "The role `registrar` reaches `parcel` for read, insert."
    );
    assert_eq!(
        simulation(false, false, &["record_transfer", "retire"]).statement(),
        "The role `registrar` reaches `parcel` for 2 actions."
    );
    assert_eq!(
        simulation(true, false, &["record_transfer"]).statement(),
        "The role `registrar` reaches `parcel` for read, and 1 action."
    );
    let mut every = simulation(true, true, &[]);
    every.update = true;
    every.delete = true;
    assert_eq!(
        every.statement(),
        "The role `registrar` reaches `parcel` for read, insert, update, delete."
    );
}

#[test]
fn a_simulation_records_update_and_delete() {
    new_test_ext().execute_with(|| {
        install();
        grant("clerk", CapabilityKey::Update, GrantPayload::everything());
        grant("clerk", CapabilityKey::Delete, GrantPayload::everything());
        let found = Pallet::<Test>::role_simulation(&DEPLOYMENT, &name("clerk"), &name("parcel"));
        assert!(found.update);
        assert!(found.delete);
        assert!(!found.read);
        assert_eq!(found.readable_fields, FieldQualification::Only(Vec::new()));
        assert_eq!(
            Pallet::<Test>::capabilities(&DEPLOYMENT, &name("clerk"), &name("parcel")),
            vec![CapabilityKey::Update, CapabilityKey::Delete]
        );
    });
}

#[test]
fn merged_entries_widen_to_the_union_of_their_reach() {
    let every = FieldQualification::EveryField;
    let only_a = FieldQualification::Only(fields(&["survey_number", "holder"]));
    let only_b = FieldQualification::Only(fields(&["holder", "ward"]));
    let apart_a = FieldQualification::Apart(fields(&["holder", "ward"]));
    let apart_b = FieldQualification::Apart(fields(&["ward", "survey_number"]));

    assert_eq!(
        Pallet::<Test>::merge_fields([&only_a, &only_b].into_iter()),
        FieldQualification::Only(fields(&["survey_number", "holder", "ward"]))
    );
    assert_eq!(
        Pallet::<Test>::merge_fields([&apart_a, &apart_b].into_iter()),
        FieldQualification::Apart(fields(&["ward"]))
    );
    assert_eq!(
        Pallet::<Test>::merge_fields([&only_a, &every, &apart_a].into_iter()),
        FieldQualification::EveryField
    );
    assert_eq!(
        Pallet::<Test>::merge_fields(core::iter::empty()),
        FieldQualification::Only(Vec::new())
    );
}

#[test]
fn a_collection_name_past_the_limit_reaches_nothing() {
    new_test_ext().execute_with(|| {
        install();
        grant("registrar", CapabilityKey::Read, GrantPayload::everything());
        let long = CollectionId::new(&long_text());
        assert!(!<Pallet<Test> as PermissionMatrix>::allows(
            &DEPLOYMENT,
            &ANITHA,
            &long,
            &CapabilityKey::Read
        ));
        assert_eq!(
            <Pallet<Test> as PermissionMatrix>::readable_fields(&DEPLOYMENT, &ANITHA, &long),
            FieldQualification::Only(Vec::new())
        );
        let record = parcel("12/3", "Anitha", "north");
        assert!(<Pallet<Test> as PermissionMatrix>::visible_record(
            &DEPLOYMENT,
            &ANITHA,
            &long,
            &record
        )
        .is_none());
    });
}

#[test]
fn a_record_preview_call_carrying_a_value_other_than_a_record_keeps_it_whole() {
    new_test_ext().execute_with(|| {
        install();
        grant(
            "registrar",
            CapabilityKey::Read,
            GrantPayload::only_fields(fields(&["holder"])),
        );
        let found = Pallet::<Test>::record_preview(
            &DEPLOYMENT,
            &name("registrar"),
            &name("parcel"),
            &CapabilityKey::Read,
            &Value::Integer(7),
        );
        assert!(found.visible);
        assert_eq!(found.record, Some(Value::Integer(7)));
        assert_eq!(
            found.statement(),
            "The role reads the record and reaches no field of it."
        );
    });
}

#[test]
fn a_grant_payload_builds_every_field_shape() {
    assert_eq!(GrantPayload::default(), GrantPayload::everything());
    let apart = GrantPayload::apart_from(fields(&["holder"])).for_purposes(&["audit"]);
    assert_eq!(apart.fields, FieldQualification::Apart(fields(&["holder"])));
    assert_eq!(apart.purposes, vec![PurposeTag::new("audit")]);
    assert!(apart.rows.is_none());
    assert_eq!(CapabilityKey::Update.label(), "update");
    assert_eq!(CapabilityKey::Delete.label(), "delete");
    assert_eq!(CapabilityKey::Insert.label(), "insert");
    assert_eq!(CapabilityKey::Invoke(name("retire")).label(), "invoke");
    assert!(!CapabilityKey::Invoke(name("retire")).writes());
    assert_eq!(
        CapabilityKey::of(&AccessAction::Read),
        Some(CapabilityKey::Read)
    );
    assert_eq!(
        CapabilityKey::of(&AccessAction::Insert),
        Some(CapabilityKey::Insert)
    );
    assert_eq!(
        CapabilityKey::of(&AccessAction::Update),
        Some(CapabilityKey::Update)
    );
    assert_eq!(
        CapabilityKey::of(&AccessAction::Invoke(ActionId::new("retire"))),
        Some(CapabilityKey::Invoke(name("retire")))
    );
}

#[test]
fn the_admitted_rows_pass_keeps_the_records_the_predicate_admits() {
    use cerulea_blueprint::{
        expression::{Env, Meter},
        permissions::ROW_BINDING,
        types::{Identity, ParamName, Timestamp},
    };
    let records = vec![
        parcel("12/3", "Anitha", "north"),
        parcel("44/9", "Benny", "south"),
    ];
    let env = Env::new(Identity([0u8; 32]), Timestamp::from_millis(START));
    let mut meter = Meter::new(10_000);
    let binding = ParamName::new(ROW_BINDING);
    let predicate = ward_predicate("north");
    let kept =
        crate::resolve::admitted_rows(Some(&predicate), &records, &env, &mut meter, &binding);
    assert_eq!(kept, vec![records[0].clone()]);
    let every = crate::resolve::admitted_rows(None, &records, &env, &mut meter, &binding);
    assert_eq!(every, records);
}
