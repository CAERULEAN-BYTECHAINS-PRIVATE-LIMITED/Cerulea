//! The tests of the operator module.
//!
//! The Cerulea operator upgrades the platform, verifies the licence and
//! stops block production on a lapse. Every request naming a customer
//! collection aborts through the permission module and lands in the audit
//! log with the actor, the timestamp and the reason.

use crate::{mock::*, Error, OperatorRequest, OperatorRole, Outcome, Requests, PLATFORM_SCOPE};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_evolution::PlatformVersions;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use pallet_cerulea_tier::{Application, LicenceControl, ProductionGate, Tier as TierBand};
use sp_core::{ed25519, Pair};

extern crate alloc;
use alloc::{vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const OPERATOR: u64 = 2;
const OUTSIDER: u64 = 3;
const YEAR: u64 = 31_536_000_000;

type TestLicence = pallet_cerulea_tier::Licence<ConstU32<32>, ConstU32<16>>;

fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[7u8; 32])
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn reason(text: &str) -> BoundedVec<u8, ConstU32<256>> {
    bounded(text.as_bytes().to_vec())
}

fn collection(name: &str) -> BoundedName {
    name_of(name).expect("the name fits")
}

fn install_licence(expires_at: u64) {
    if pallet_cerulea_tier::ControlPlaneKey::<Test>::get(DEPLOYMENT).is_none() {
        assert_ok!(Tier::set_control_plane_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            control_plane().public().0
        ));
    }
    let licence = TestLicence {
        deployment: DEPLOYMENT,
        tier: TierBand {
            application: Application::Enterprise,
            level: 1,
        },
        features: BoundedVec::try_from(vec![collection("workflows")]).expect("the list fits"),
        seats: 10,
        addons: BoundedVec::default(),
        issued_at: clock(),
        expires_at,
        sequence: pallet_cerulea_tier::Licences::<Test>::get(DEPLOYMENT)
            .map(|held| held.sequence + 1)
            .unwrap_or(1),
    };
    let signature = control_plane().sign(&licence.message());
    assert_ok!(Tier::install_licence(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(licence.encode()),
        signature.0
    ));
}

fn appoint() {
    assert_ok!(Operator::appoint_operator(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        OPERATOR
    ));
}

fn ask(request: OperatorRequest, text: &str) {
    assert_ok!(Operator::request(
        RuntimeOrigin::signed(OPERATOR),
        DEPLOYMENT,
        request,
        reason(text)
    ));
}

#[test]
fn an_operator_is_appointed_and_reads_back() {
    new_test_ext().execute_with(|| {
        appoint();
        assert!(Operator::is_operator(&DEPLOYMENT, &OPERATOR));
        assert!(!Operator::is_operator(&DEPLOYMENT, &OUTSIDER));
    });
}

#[test]
fn an_account_outside_the_role_asks_for_nothing() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(OUTSIDER),
                DEPLOYMENT,
                OperatorRequest::VerifyLicence,
                reason("The operator checks the licence of the deployment.")
            ),
            Error::<Test>::NotAnOperator
        );
    });
}

#[test]
fn a_retired_operator_asks_for_nothing() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_ok!(Operator::retire_operator(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            OPERATOR
        ));
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(OPERATOR),
                DEPLOYMENT,
                OperatorRequest::VerifyLicence,
                reason("The operator checks the licence of the deployment.")
            ),
            Error::<Test>::NotAnOperator
        );
    });
}

#[test]
fn a_request_with_no_reason_declines() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(OPERATOR),
                DEPLOYMENT,
                OperatorRequest::VerifyLicence,
                BoundedVec::default()
            ),
            Error::<Test>::ReasonMissing
        );
    });
}

#[test]
fn the_operator_upgrades_and_downgrades_the_platform_version() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::UpgradePlatform { version: 4 },
            "The release train reached version four.",
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 4);

        ask(
            OperatorRequest::DowngradePlatform { version: 3 },
            "Version four showed a fault in the field.",
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 3);

        let first = Operator::request_record(&DEPLOYMENT, 0).expect("the module holds the record");
        assert_eq!(first.outcome, Outcome::Carried);
        assert_eq!(first.actor, OPERATOR);
        assert_eq!(first.at_millis, START);
        assert_eq!(
            first.reason_text(),
            "The release train reached version four."
        );
    });
}

#[test]
fn the_operator_pushes_one_platform_update() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::PushUpdate {
                version: 7,
                digest: [3u8; 32],
            },
            "The platform update closes a fault in the reporting surface.",
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 7);
    });
}

#[test]
fn the_operator_verifies_a_licence_that_holds() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        ask(
            OperatorRequest::VerifyLicence,
            "The operator checks the licence of the deployment.",
        );
        assert!(Tier::holds(&DEPLOYMENT));
        assert!(Tier::production_permitted(&DEPLOYMENT));
    });
}

#[test]
fn the_operator_stops_block_production_on_a_lapse() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        advance_clock(YEAR + 1);

        ask(
            OperatorRequest::SuspendOnLapse,
            "The licence of the deployment lapsed.",
        );
        assert!(!Tier::production_permitted(&DEPLOYMENT));
        // Every existing record stays readable and exportable.
        assert!(Tier::read_permitted(&DEPLOYMENT));
        assert!(Tier::export_permitted(&DEPLOYMENT));
    });
}

#[test]
fn a_verification_of_a_lapsed_licence_stops_production() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        advance_clock(YEAR + 1);
        ask(
            OperatorRequest::VerifyLicence,
            "The operator checks the licence of the deployment.",
        );
        assert!(!Tier::production_permitted(&DEPLOYMENT));
    });
}

#[test]
fn a_read_of_customer_records_aborts_and_is_logged() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::ReadRecords {
                collection: collection("parcel"),
            },
            "A support ticket asked for the parcel records.",
        );

        let held = Operator::request_record(&DEPLOYMENT, 0).expect("the module holds the record");
        assert_eq!(held.outcome, Outcome::PermissionDenied);
        assert_eq!(held.actor, OPERATOR);
        assert_eq!(held.at_millis, START);
        assert_eq!(
            held.reason_text(),
            "A support ticket asked for the parcel records."
        );

        let records = Audit::query_by_event(&DEPLOYMENT, "customer_record_read_refused", 0, 64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].actor, OPERATOR);
        assert_eq!(records[0].module, "operator");
        assert_eq!(records[0].class, WorkClass::Constitutional);
        assert_eq!(
            records[0].reason,
            "A support ticket asked for the parcel records."
        );
        assert_eq!(records[0].at_millis, START);
    });
}

#[test]
fn a_write_of_customer_data_aborts_and_is_logged() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::WriteRecord {
                collection: collection("parcel"),
            },
            "A support ticket asked for a correction to one parcel.",
        );
        assert_eq!(
            Operator::request_record(&DEPLOYMENT, 0)
                .expect("the module holds the record")
                .outcome,
            Outcome::PermissionDenied
        );
        assert_eq!(
            Audit::query_by_event(&DEPLOYMENT, "customer_record_write_refused", 0, 64).len(),
            1
        );
    });
}

#[test]
fn a_change_to_customer_governance_aborts_and_is_logged() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::GrantPermission {
                role: collection("registrar"),
            },
            "A support ticket asked for a wider role.",
        );
        assert_eq!(
            Operator::request_record(&DEPLOYMENT, 0)
                .expect("the module holds the record")
                .outcome,
            Outcome::PermissionDenied
        );
        assert_eq!(
            Audit::query_by_event(&DEPLOYMENT, "customer_permission_refused", 0, 64).len(),
            1
        );
    });
}

#[test]
fn a_change_to_the_blueprint_aborts_and_is_logged() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::ChangeBlueprint {
                blueprint: [5u8; 32],
            },
            "A support ticket asked for a change to the model.",
        );
        assert_eq!(
            Operator::request_record(&DEPLOYMENT, 0)
                .expect("the module holds the record")
                .outcome,
            Outcome::PermissionDenied
        );
        assert_eq!(
            Audit::query_by_event(&DEPLOYMENT, "blueprint_change_refused", 0, 64).len(),
            1
        );
    });
}

#[test]
fn a_refused_request_changes_nothing_of_the_platform() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::UpgradePlatform { version: 4 },
            "The release train reached version four.",
        );
        ask(
            OperatorRequest::WriteRecord {
                collection: collection("parcel"),
            },
            "A support ticket asked for a correction to one parcel.",
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 4);
        assert_eq!(Operator::request_count(&DEPLOYMENT), 2);
    });
}

#[test]
fn every_scope_answer_follows_the_request() {
    assert!(OperatorRequest::UpgradePlatform { version: 1 }.in_scope());
    assert!(OperatorRequest::DowngradePlatform { version: 1 }.in_scope());
    assert!(OperatorRequest::VerifyLicence.in_scope());
    assert!(OperatorRequest::SuspendOnLapse.in_scope());
    assert!(OperatorRequest::PushUpdate {
        version: 1,
        digest: [0u8; 32]
    }
    .in_scope());

    let read = OperatorRequest::ReadRecords {
        collection: name_of("parcel").expect("the name fits"),
    };
    assert!(!read.in_scope());
    assert_eq!(read.scope().as_str(), "parcel");
    assert_eq!(read.label(), "read of customer records");
    assert_eq!(
        OperatorRequest::VerifyLicence.scope().as_str(),
        PLATFORM_SCOPE
    );
    assert_eq!(Outcome::Carried.label(), "carried");
    assert_eq!(Outcome::OutsideScope.label(), "outside scope");
    assert_eq!(Outcome::PermissionDenied.label(), "permission denied");
}

#[test]
fn every_operator_action_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        ask(
            OperatorRequest::UpgradePlatform { version: 4 },
            "The release train reached version four.",
        );
        ask(
            OperatorRequest::VerifyLicence,
            "The operator checks the licence of the deployment.",
        );

        let appointed = Audit::query_by_event(&DEPLOYMENT, "operator_appointed", 0, 64);
        assert_eq!(appointed.len(), 1);
        assert_eq!(appointed[0].actor, STEWARD);

        let upgraded = Audit::query_by_event(&DEPLOYMENT, "platform_upgraded", 0, 64);
        assert_eq!(upgraded.len(), 1);
        assert_eq!(upgraded[0].actor, OPERATOR);
        assert_eq!(upgraded[0].at_millis, START);
        assert_eq!(
            upgraded[0].reason,
            "The release train reached version four."
        );

        let checked = Audit::query_by_event(&DEPLOYMENT, "licence_checked", 0, 64);
        assert_eq!(checked.len(), 1);
        assert_eq!(checked[0].actor, OPERATOR);

        // Every request the module handled reaches the stored register as
        // well as the log.
        assert_eq!(Operator::request_count(&DEPLOYMENT), 2);
        assert_eq!(Requests::<Test>::iter_prefix(DEPLOYMENT).count(), 2);
    });
}
