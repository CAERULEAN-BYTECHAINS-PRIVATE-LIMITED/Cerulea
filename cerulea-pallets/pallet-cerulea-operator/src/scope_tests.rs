//! The tests of every branch of the Cerulea operator role.
//!
//! Each test drives the calls and reads the operator register, the request
//! register, the events and the audit log afterwards.

use crate::{
    mock::*, Error, Event, GenesisConfig, NextRequest, OperatorRecord, OperatorRequest,
    OperatorRole, Operators, Outcome, PLATFORM_SCOPE,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_evolution::PlatformVersions;
use pallet_cerulea_schema::{
    access::AccessAction,
    primitives::{name_of, BoundedName, DeploymentId},
};
use pallet_cerulea_tier::{Application, LicenceControl, ProductionGate, Tier as TierBand};
use sp_core::{ed25519, Pair};
use sp_runtime::BuildStorage;

extern crate alloc;
use alloc::{string::String, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const OPERATOR: u64 = 2;
const YEAR: u64 = 31_536_000_000;

type TestLicence = pallet_cerulea_tier::Licence<ConstU32<32>, ConstU32<16>>;

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn reason(text: &str) -> BoundedVec<u8, ConstU32<256>> {
    bounded(text.as_bytes().to_vec())
}

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn install_licence(expires_at: u64) {
    let control_plane = ed25519::Pair::from_seed(&[7u8; 32]);
    assert_ok!(Tier::set_control_plane_key(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        control_plane.public().0
    ));
    let licence = TestLicence {
        deployment: DEPLOYMENT,
        tier: TierBand {
            application: Application::Enterprise,
            level: 1,
        },
        features: BoundedVec::try_from(vec![name("workflows")]).expect("the list fits"),
        seats: 10,
        addons: BoundedVec::default(),
        issued_at: clock(),
        expires_at,
        sequence: 1,
    };
    let signature = control_plane.sign(&licence.message());
    assert_ok!(Tier::install_licence(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(licence.encode()),
        signature.0
    ));
}

fn appoint() {
    assert_ok!(Operator::appoint_operator(
        RuntimeOrigin::root(),
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

fn operator_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Operator(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

fn outcome(id: u64) -> Outcome {
    Operator::request_record(&DEPLOYMENT, id)
        .expect("the module holds the record")
        .outcome
}

/// The fix sentence one error carries in the runtime metadata.
fn error_text(variant: &str) -> String {
    use scale_info::{TypeDef, TypeInfo};
    let info = <Error<Test> as TypeInfo>::type_info();
    let variants = match info.type_def {
        TypeDef::Variant(found) => found.variants,
        _ => panic!("the error type is an enumeration"),
    };
    let found = variants
        .into_iter()
        .find(|held| held.name == variant)
        .expect("the error type names the variant");
    found
        .docs
        .iter()
        .map(|line| line.trim())
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn an_appointment_records_its_time_its_event_and_its_reason() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        appoint();
        assert_eq!(
            Operators::<Test>::get(DEPLOYMENT, OPERATOR),
            Some(OperatorRecord {
                appointed_at: START,
                appointed_block: 3,
                active: true
            })
        );
        assert!(operator_events().contains(&Event::OperatorAppointed {
            deployment: DEPLOYMENT,
            operator: OPERATOR
        }));
        let appointed = Audit::query_by_event(&DEPLOYMENT, "operator_appointed", 0, 64);
        assert_eq!(appointed[0].class, WorkClass::Constitutional);
        assert_eq!(
            appointed[0].reason,
            "The deployment appointed one account to the Cerulea operator role."
        );
    });
}

#[test]
fn an_active_operator_is_appointed_once() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_noop!(
            Operator::appoint_operator(RuntimeOrigin::root(), DEPLOYMENT, OPERATOR),
            Error::<Test>::OperatorPresent
        );
        assert_eq!(
            error_text("OperatorPresent"),
            "The registry already holds that operator. Retire the operator before appointing it again."
        );
    });
}

#[test]
fn a_retired_operator_is_appointed_again_as_its_error_directs() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_ok!(Operator::retire_operator(
            RuntimeOrigin::root(),
            DEPLOYMENT,
            OPERATOR
        ));
        assert!(!Operator::is_operator(&DEPLOYMENT, &OPERATOR));

        advance_clock(500);
        appoint();
        assert!(Operator::is_operator(&DEPLOYMENT, &OPERATOR));
        assert_eq!(
            Operators::<Test>::get(DEPLOYMENT, OPERATOR)
                .expect("the register holds the operator")
                .appointed_at,
            START + 500
        );
        ask(
            OperatorRequest::UpgradePlatform { version: 2 },
            "The release train reached version two.",
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 2);
    });
}

#[test]
fn a_retirement_records_its_event_and_its_reason() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_ok!(Operator::retire_operator(
            RuntimeOrigin::root(),
            DEPLOYMENT,
            OPERATOR
        ));
        assert_eq!(
            Operators::<Test>::get(DEPLOYMENT, OPERATOR).map(|held| held.active),
            Some(false)
        );
        assert!(operator_events().contains(&Event::OperatorRetired {
            deployment: DEPLOYMENT,
            operator: OPERATOR
        }));
        let retired = Audit::query_by_event(&DEPLOYMENT, "operator_retired", 0, 64);
        // The root origin acts, so the record names the account the call
        // moved.
        assert_eq!(retired[0].actor, OPERATOR);
        assert_eq!(
            retired[0].reason,
            "The deployment took one account out of the Cerulea operator role."
        );
    });
}

#[test]
fn a_retirement_of_an_account_outside_the_role_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Operator::retire_operator(RuntimeOrigin::root(), DEPLOYMENT, OPERATOR),
            Error::<Test>::NotAnOperator
        );
        assert_eq!(
            error_text("NotAnOperator"),
            "The account holds no operator role for that deployment. Appoint the operator before it acts."
        );
        assert_eq!(
            error_text("ReasonMissing"),
            "The reason sentence is empty. State why the operator acts."
        );
    });
}

#[test]
fn the_genesis_appoints_every_named_operator() {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .expect("the genesis storage builds");
    GenesisConfig::<Test> {
        operators: vec![(DEPLOYMENT, OPERATOR), ([4u8; 32], 7)],
    }
    .assimilate_storage(&mut storage)
    .expect("the operator genesis builds");
    let mut ext = sp_io::TestExternalities::new(storage);
    ext.execute_with(|| {
        assert!(Operator::is_operator(&DEPLOYMENT, &OPERATOR));
        assert!(Operator::is_operator(&[4u8; 32], &7));
        assert!(!Operator::is_operator(&[4u8; 32], &OPERATOR));
        assert_eq!(
            Operators::<Test>::get(DEPLOYMENT, OPERATOR).map(|held| held.appointed_at),
            Some(0)
        );
    });
}

#[test]
fn a_carried_request_emits_its_events() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::PushUpdate {
                version: 6,
                digest: [1u8; 32],
            },
            "The platform update closes a fault in the reporting surface.",
        );
        let events = operator_events();
        assert!(events.contains(&Event::PlatformVersionChanged {
            deployment: DEPLOYMENT,
            version: 6
        }));
        assert!(events.contains(&Event::RequestCarried {
            deployment: DEPLOYMENT,
            request: 0,
            actor: OPERATOR
        }));
        let pushed = Audit::query_by_event(&DEPLOYMENT, "platform_update_pushed", 0, 64);
        assert_eq!(pushed.len(), 1);
        assert_eq!(
            pushed[0].reason,
            "The platform update closes a fault in the reporting surface."
        );
        // The platform version registry records the move under the same
        // reason.
        let moved = Audit::query_by_event(&DEPLOYMENT, "platform_version_set", 0, 64);
        assert_eq!(moved[0].actor, OPERATOR);
        assert_eq!(
            moved[0].reason,
            "The platform update closes a fault in the reporting surface."
        );
    });
}

#[test]
fn a_refused_request_emits_its_outcome() {
    new_test_ext().execute_with(|| {
        appoint();
        ask(
            OperatorRequest::ReadRecords {
                collection: name("parcel"),
            },
            "A support ticket asked for the parcel records.",
        );
        assert!(operator_events().contains(&Event::RequestRefused {
            deployment: DEPLOYMENT,
            request: 0,
            actor: OPERATOR,
            outcome: Outcome::PermissionDenied
        }));
    });
}

#[test]
fn a_request_the_resolver_admits_outside_the_role_still_aborts() {
    new_test_ext().execute_with(|| {
        appoint();
        // Each request names the platform scope, so the resolver admits
        // it. The kind of request sits outside the role, so the module
        // aborts it and records the refusal.
        ask(
            OperatorRequest::WriteRecord {
                collection: name(PLATFORM_SCOPE),
            },
            "A support ticket asked for a correction to the platform record.",
        );
        ask(
            OperatorRequest::GrantPermission {
                role: name(PLATFORM_SCOPE),
            },
            "A support ticket asked for a wider platform role.",
        );
        assert_eq!(outcome(0), Outcome::OutsideScope);
        assert_eq!(outcome(1), Outcome::OutsideScope);
        assert!(operator_events().contains(&Event::RequestRefused {
            deployment: DEPLOYMENT,
            request: 1,
            actor: OPERATOR,
            outcome: Outcome::OutsideScope
        }));
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 0);
        assert_eq!(Operator::request_count(&DEPLOYMENT), 2);
        let refused = Audit::query_by_event(&DEPLOYMENT, "customer_record_write_refused", 0, 64);
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].actor, OPERATOR);
    });
}

#[test]
fn a_full_request_register_declines_a_further_request() {
    new_test_ext().execute_with(|| {
        appoint();
        NextRequest::<Test>::insert(DEPLOYMENT, u64::MAX);
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(OPERATOR),
                DEPLOYMENT,
                OperatorRequest::UpgradePlatform { version: 4 },
                reason("The release train reached version four.")
            ),
            Error::<Test>::RegisterFull
        );
        assert_eq!(Evolution::platform_version(&DEPLOYMENT), 0);
    });
}

#[test]
fn a_licence_check_over_a_deployment_with_no_licence_declines() {
    new_test_ext().execute_with(|| {
        appoint();
        assert_noop!(
            Operator::request(
                RuntimeOrigin::signed(OPERATOR),
                DEPLOYMENT,
                OperatorRequest::VerifyLicence,
                reason("The operator checks the licence of the deployment.")
            ),
            pallet_cerulea_tier::Error::<Test>::LicenceMissing
        );
        assert_eq!(Operator::request_count(&DEPLOYMENT), 0);
    });
}

#[test]
fn a_suspension_request_over_a_licence_that_holds_leaves_production_running() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        ask(
            OperatorRequest::SuspendOnLapse,
            "The operator checks for a lapse.",
        );
        assert_eq!(outcome(0), Outcome::Carried);
        assert!(Tier::production_permitted(&DEPLOYMENT));
        assert!(!operator_events()
            .iter()
            .any(|event| matches!(event, Event::SuspendedOnLapse { .. })));
    });
}

#[test]
fn a_lapse_emits_the_suspension_event() {
    new_test_ext().execute_with(|| {
        appoint();
        install_licence(START + YEAR);
        advance_clock(YEAR + 1);
        ask(
            OperatorRequest::VerifyLicence,
            "The operator checks the licence of the deployment.",
        );
        assert!(!Tier::holds(&DEPLOYMENT));
        assert!(operator_events().contains(&Event::SuspendedOnLapse {
            deployment: DEPLOYMENT
        }));
        let checked = Audit::query_by_event(&DEPLOYMENT, "licence_checked", 0, 64);
        assert_eq!(checked[0].class, WorkClass::Constitutional);
    });
}

#[test]
fn every_request_names_its_scope_its_action_its_event_and_its_label() {
    let platform = [
        (
            OperatorRequest::UpgradePlatform { version: 1 },
            "platform_upgraded",
            "platform version upgrade",
        ),
        (
            OperatorRequest::DowngradePlatform { version: 1 },
            "platform_downgraded",
            "platform version downgrade",
        ),
        (
            OperatorRequest::VerifyLicence,
            "licence_checked",
            "licence verification",
        ),
        (
            OperatorRequest::SuspendOnLapse,
            "suspension_on_lapse",
            "suspension on licence lapse",
        ),
        (
            OperatorRequest::PushUpdate {
                version: 1,
                digest: [0u8; 32],
            },
            "platform_update_pushed",
            "platform update",
        ),
    ];
    for (request, event, label) in platform.iter() {
        assert_eq!(request.scope().as_str(), PLATFORM_SCOPE);
        assert!(matches!(request.access(), AccessAction::Invoke(_)));
        assert_eq!(request.event(), *event);
        assert_eq!(request.label(), *label);
        assert!(<Operator as OperatorRole>::permits(request));
    }

    let customer = [
        (
            OperatorRequest::ReadRecords {
                collection: name("parcel"),
            },
            "parcel",
            AccessAction::Read,
            "customer_record_read_refused",
        ),
        (
            OperatorRequest::WriteRecord {
                collection: name("parcel"),
            },
            "parcel",
            AccessAction::Update,
            "customer_record_write_refused",
        ),
        (
            OperatorRequest::GrantPermission {
                role: name("registrar"),
            },
            "registrar",
            AccessAction::Update,
            "customer_permission_refused",
        ),
        (
            OperatorRequest::ChangeBlueprint {
                blueprint: [5u8; 32],
            },
            "cerulea_blueprint",
            AccessAction::Update,
            "blueprint_change_refused",
        ),
    ];
    for (request, scope, action, event) in customer.iter() {
        assert_eq!(request.scope().as_str(), *scope);
        assert_eq!(request.access(), *action);
        assert_eq!(request.event(), *event);
        assert!(!<Operator as OperatorRole>::permits(request));
    }
    assert_eq!(
        OperatorRequest::WriteRecord {
            collection: name("parcel")
        }
        .label(),
        "write of customer data"
    );
    assert_eq!(
        OperatorRequest::GrantPermission {
            role: name("registrar")
        }
        .label(),
        "change to customer governance"
    );
    assert_eq!(
        OperatorRequest::ChangeBlueprint {
            blueprint: [5u8; 32]
        }
        .label(),
        "change to the Blueprint"
    );
}
