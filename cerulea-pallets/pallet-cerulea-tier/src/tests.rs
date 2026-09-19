//! The tests of the tier module.
//!
//! The control plane signs every licence with an ed25519 key. The tests
//! hold that key, sign a real attestation and let the Blockchain verify
//! it. One test lets the licence lapse and then reads and exports the
//! records of the suspended deployment.

use crate::{
    mock::*, Application, EntitlementGate, Error, LicenceState, Licences, ProductionGate,
    Signatures, States, Tier as TierBand, Usage,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    types::{CollectionId, FieldDef, FieldName, RecordDef, TypeDef, TypeExpr, TypeId, Value},
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use pallet_cerulea_state::RecordStore;
use sp_core::{ed25519, Pair};

extern crate alloc;
use alloc::{string::ToString, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const OPERATOR: u64 = 2;
const YEAR: u64 = 31_536_000_000;

type TestLicence = crate::Licence<ConstU32<32>, ConstU32<16>>;

fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[7u8; 32])
}

fn names(list: &[&str]) -> Vec<BoundedName> {
    list.iter()
        .map(|held| name_of(held).expect("the name fits"))
        .collect()
}

fn licence(sequence: u64, expires_at: u64, features: &[&str], seats: u32) -> TestLicence {
    TestLicence {
        deployment: DEPLOYMENT,
        tier: TierBand {
            application: Application::Enterprise,
            level: 2,
        },
        features: BoundedVec::try_from(names(features)).expect("the list fits"),
        seats,
        addons: BoundedVec::try_from(names(&["extended_history"])).expect("the list fits"),
        issued_at: START,
        expires_at,
        sequence,
    }
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn install_key() {
    assert_ok!(Tier::set_control_plane_key(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        control_plane().public().0
    ));
}

fn install(held: &TestLicence) {
    let signature = control_plane().sign(&held.message());
    assert_ok!(Tier::install_licence(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(held.encode()),
        signature.0
    ));
}

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

fn parcel(survey: &str, holder: &str) -> Value {
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

fn store_records() {
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
    for (survey, holder) in [("12/3", "Anitha"), ("14/1", "Benny")] {
        assert_ok!(State::insert(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("parcel").expect("the name fits"),
            bounded(parcel(survey, holder).canonical_bytes())
        ));
    }
}

#[test]
fn a_signed_licence_installs_and_reads_back() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows", "reports"], 25));

        let held = Licences::<Test>::get(DEPLOYMENT).expect("the deployment holds a licence");
        assert_eq!(held.seats, 25);
        assert_eq!(held.tier.application, Application::Enterprise);
        assert_eq!(held.tier.level, 2);
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
        assert!(Signatures::<Test>::get(DEPLOYMENT).is_some());
    });
}

#[test]
fn a_licence_signed_by_another_key_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        let held = licence(1, START + YEAR, &["workflows"], 25);
        let impostor = ed25519::Pair::from_seed(&[8u8; 32]);
        let signature = impostor.sign(&held.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(held.encode()),
                signature.0
            ),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn a_changed_attestation_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        let held = licence(1, START + YEAR, &["workflows"], 25);
        let signature = control_plane().sign(&held.message());
        let mut changed = held.clone();
        changed.seats = 500;
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(changed.encode()),
                signature.0
            ),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn a_licence_without_a_control_plane_key_declines() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR, &["workflows"], 25);
        let signature = control_plane().sign(&held.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(held.encode()),
                signature.0
            ),
            Error::<Test>::ControlPlaneKeyMissing
        );
    });
}

#[test]
fn a_replayed_licence_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(2, START + YEAR, &["workflows"], 25));
        let earlier = licence(1, START + YEAR, &["workflows"], 25);
        let signature = control_plane().sign(&earlier.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(earlier.encode()),
                signature.0
            ),
            Error::<Test>::SequenceReplayed
        );
    });
}

#[test]
fn a_tier_level_the_application_lacks_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        let mut held = licence(1, START + YEAR, &["workflows"], 25);
        held.tier = TierBand {
            application: Application::Government,
            level: 3,
        };
        let signature = control_plane().sign(&held.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(held.encode()),
                signature.0
            ),
            Error::<Test>::TierInvalid
        );
    });
}

#[test]
fn an_entitlement_follows_the_feature_set() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows", "reports"], 25));
        assert!(Tier::entitled(&DEPLOYMENT, "workflows"));
        assert!(Tier::entitled(&DEPLOYMENT, "reports"));
        assert!(!Tier::entitled(&DEPLOYMENT, "bridges"));
        assert_eq!(Tier::seats(&DEPLOYMENT), 25);
        assert_eq!(
            Tier::tier(&DEPLOYMENT),
            Some(TierBand {
                application: Application::Enterprise,
                level: 2
            })
        );
    });
}

#[test]
fn usage_records_against_a_licensed_feature() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_ok!(Tier::record_usage(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("workflows").expect("the name fits"),
            4
        ));
        assert_ok!(Tier::record_usage(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("workflows").expect("the name fits"),
            6
        ));
        assert_eq!(
            Usage::<Test>::get(DEPLOYMENT, name_of("workflows").expect("the name fits")),
            10
        );
        assert_eq!(Tier::usage_of(&DEPLOYMENT, "workflows"), 10);
    });
}

#[test]
fn usage_against_an_unlicensed_feature_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_noop!(
            Tier::record_usage(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name_of("bridges").expect("the name fits"),
                1
            ),
            Error::<Test>::FeatureUnlicensed
        );
    });
}

#[test]
fn a_seat_beyond_the_licence_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 2));
        assert_ok!(Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT));
        assert_ok!(Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT));
        assert_noop!(
            Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT),
            Error::<Test>::SeatsExhausted
        );
    });
}

#[test]
fn a_lapsed_licence_stops_block_production_and_leaves_export_working() {
    new_test_ext().execute_with(|| {
        install_key();
        store_records();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert!(Tier::production_permitted(&DEPLOYMENT));

        // The licence lapses and the block hook stops block production.
        advance_clock(YEAR + 1);
        roll_one_block();
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Lapsed));
        assert!(!Tier::production_permitted(&DEPLOYMENT));

        // Every existing record stays readable.
        assert!(Tier::read_permitted(&DEPLOYMENT));
        let collection = CollectionId::new("parcel");
        let keys = State::keys(&DEPLOYMENT, &collection, 16);
        assert_eq!(keys.len(), 2);
        let record = State::read(&DEPLOYMENT, &collection, &keys[0])
            .expect("the store holds the record after the lapse");
        assert!(matches!(record, Value::Record(_)));

        // Every existing record stays exportable.
        assert!(Tier::export_permitted(&DEPLOYMENT));
        let export: Vec<Vec<u8>> = keys
            .iter()
            .filter_map(|key| State::read_bytes(&DEPLOYMENT, &collection, key))
            .collect();
        assert_eq!(export.len(), 2);
        assert!(export.iter().all(|held| !held.is_empty()));

        let reason = Tier::suspension_reason(&DEPLOYMENT).expect("the module states the reason");
        assert!(reason.starts_with("The licence lapsed."));
    });
}

#[test]
fn an_operator_suspension_leaves_read_and_export_working() {
    new_test_ext().execute_with(|| {
        install_key();
        store_records();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));

        assert!(!Tier::production_permitted(&DEPLOYMENT));
        assert!(Tier::read_permitted(&DEPLOYMENT));
        assert!(Tier::export_permitted(&DEPLOYMENT));
        let collection = CollectionId::new("parcel");
        assert_eq!(State::count(&DEPLOYMENT, &collection), 2);
        assert!(State::read(
            &DEPLOYMENT,
            &collection,
            &State::keys(&DEPLOYMENT, &collection, 8)[0]
        )
        .is_some());
    });
}

#[test]
fn a_resumed_deployment_produces_blocks_again() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));
        assert!(!Tier::production_permitted(&DEPLOYMENT));

        assert_ok!(Tier::resume(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT));
        assert!(Tier::production_permitted(&DEPLOYMENT));
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
        assert!(Tier::suspension_reason(&DEPLOYMENT).is_none());
    });
}

#[test]
fn a_resume_under_a_lapsed_licence_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        advance_clock(YEAR + 1);
        roll_one_block();
        assert_noop!(
            Tier::resume(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::LicenceLapsed
        );
    });
}

#[test]
fn a_renewed_licence_starts_production_again() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        advance_clock(YEAR + 1);
        roll_one_block();
        assert!(!Tier::production_permitted(&DEPLOYMENT));

        install(&licence(2, clock() + YEAR, &["workflows"], 25));
        assert!(Tier::production_permitted(&DEPLOYMENT));
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
    });
}

#[test]
fn a_verification_pass_stops_production_on_a_lapse() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_ok!(Tier::verify_licence(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT
        ));

        advance_clock(YEAR + 1);
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::LicenceLapsed
        );
    });
}

#[test]
fn a_lapsed_licence_records_no_usage() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        advance_clock(YEAR + 1);
        assert_noop!(
            Tier::record_usage(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name_of("workflows").expect("the name fits"),
                1
            ),
            Error::<Test>::LicenceLapsed
        );
        assert!(!Tier::licence_holds(&DEPLOYMENT));
    });
}

#[test]
fn an_application_carries_its_levels() {
    assert_eq!(Application::Dapps.levels(), 3);
    assert_eq!(Application::Enterprise.levels(), 3);
    assert_eq!(Application::Government.levels(), 1);
    assert_eq!(Application::Government.label(), "government");
    assert!(TierBand {
        application: Application::Dapps,
        level: 3
    }
    .is_valid());
    assert!(!TierBand {
        application: Application::Dapps,
        level: 4
    }
    .is_valid());
    assert!(!TierBand {
        application: Application::Government,
        level: 0
    }
    .is_valid());
}

#[test]
fn every_tier_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        install_key();
        install(&licence(1, START + YEAR, &["workflows"], 25));
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));

        let installed = Audit::query_by_event(&DEPLOYMENT, "licence_installed", 0, 64);
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].actor, STEWARD);
        assert_eq!(installed[0].module, "tier");
        assert_eq!(installed[0].class, WorkClass::Constitutional);

        let suspended = Audit::query_by_event(&DEPLOYMENT, "production_suspended", 0, 64);
        assert_eq!(suspended.len(), 1);
        assert_eq!(suspended[0].actor, OPERATOR);
    });
}
