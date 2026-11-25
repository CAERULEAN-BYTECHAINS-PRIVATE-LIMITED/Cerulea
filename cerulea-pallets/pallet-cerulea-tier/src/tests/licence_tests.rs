//! Further tests of the tier module.
//!
//! These tests cover the genesis build, the signed origin of every call,
//! the watch list bound, every licence read and the licence control the
//! Cerulea operator role reaches. The control plane key signs every
//! licence, so the Blockchain verifies a real signature in every test.

use crate::{
    mock::*, Application, ControlPlaneKey, EntitlementGate, Error, Event, GenesisConfig,
    LicenceControl, LicenceState, Licences, ProductionGate, SeatsUsed, Signatures, States,
    SuspensionReason, Tier as TierBand, Watched, WeightInfo,
};
use codec::Encode;
use frame_support::{
    assert_noop, assert_ok,
    traits::{BuildGenesisConfig, ConstU32, Hooks},
    weights::Weight,
    BoundedVec,
};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use sp_core::{ed25519, Pair};
use sp_runtime::DispatchError;

extern crate alloc;
use alloc::{string::String, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const OTHER: DeploymentId = [4u8; 32];
const STEWARD: u64 = 1;
const OPERATOR: u64 = 2;
const YEAR: u64 = 31_536_000_000;
const LAPSE_SENTENCE: &str =
    "The licence lapsed. Renew the subscription and install the new licence.";

type TestLicence = crate::Licence<ConstU32<32>, ConstU32<16>>;

fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[7u8; 32])
}

fn names(list: &[&str]) -> Vec<BoundedName> {
    list.iter()
        .map(|held| name_of(held).expect("the name fits"))
        .collect()
}

fn licence_for(deployment: DeploymentId, sequence: u64, expires_at: u64) -> TestLicence {
    TestLicence {
        deployment,
        tier: TierBand {
            application: Application::Dapps,
            level: 1,
        },
        features: BoundedVec::try_from(names(&["workflows"])).expect("the list fits"),
        seats: 1,
        addons: BoundedVec::try_from(names(&["extended_history"])).expect("the list fits"),
        issued_at: START,
        expires_at,
        sequence,
    }
}

fn licence(sequence: u64, expires_at: u64) -> TestLicence {
    licence_for(DEPLOYMENT, sequence, expires_at)
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

fn live() {
    install_key();
    install(&licence(1, START + YEAR));
}

fn tier_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Tier(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

fn genesis(
    control_plane_keys: Vec<(DeploymentId, [u8; 32])>,
    licences: Vec<(DeploymentId, Vec<u8>, Vec<u8>)>,
) -> GenesisConfig<Test> {
    GenesisConfig::<Test> {
        control_plane_keys,
        licences,
        marker: Default::default(),
    }
}

fn signed_entry(held: &TestLicence) -> (DeploymentId, Vec<u8>, Vec<u8>) {
    let signature = control_plane().sign(&held.message());
    (held.deployment, held.encode(), signature.0.to_vec())
}

#[test]
fn every_call_asks_for_a_signed_origin() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR);
        assert_noop!(
            Tier::set_control_plane_key(RuntimeOrigin::none(), DEPLOYMENT, [1u8; 32]),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                bounded(held.encode()),
                [0u8; 64]
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::root(), DEPLOYMENT),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::record_usage(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                name_of("workflows").expect("the name fits"),
                1
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::take_seat(RuntimeOrigin::none(), DEPLOYMENT),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::suspend(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                bounded(b"The payment failed.".to_vec())
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Tier::resume(RuntimeOrigin::none(), DEPLOYMENT),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_control_plane_key_installs_once() {
    new_test_ext().execute_with(|| {
        install_key();
        assert_eq!(
            ControlPlaneKey::<Test>::get(DEPLOYMENT),
            Some(control_plane().public().0)
        );
        assert_eq!(Watched::<Test>::get().into_inner(), vec![DEPLOYMENT]);
        assert_eq!(
            tier_events().last(),
            Some(&Event::ControlPlaneKeySet {
                deployment: DEPLOYMENT
            })
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "control_plane_key_set", 0, 64);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, STEWARD);
        assert_eq!(found[0].class, WorkClass::Constitutional);

        assert_noop!(
            Tier::set_control_plane_key(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, [1u8; 32]),
            Error::<Test>::ControlPlaneKeyPresent
        );
    });
}

#[test]
fn the_watch_list_holds_its_bound() {
    new_test_ext().execute_with(|| {
        for index in 1u8..=8 {
            assert_ok!(Tier::set_control_plane_key(
                RuntimeOrigin::signed(STEWARD),
                [index; 32],
                [index; 32]
            ));
        }
        assert_eq!(Watched::<Test>::get().len(), 8);
        assert_noop!(
            Tier::set_control_plane_key(RuntimeOrigin::signed(STEWARD), [20u8; 32], [1u8; 32]),
            Error::<Test>::WatchListFull
        );
        assert!(ControlPlaneKey::<Test>::get([20u8; 32]).is_none());
    });
}

#[test]
fn a_watched_deployment_enters_the_watch_list_once() {
    new_test_ext().execute_with(|| {
        Watched::<Test>::put(BoundedVec::truncate_from(vec![DEPLOYMENT]));
        install_key();
        assert_eq!(Watched::<Test>::get().into_inner(), vec![DEPLOYMENT]);
    });
}

#[test]
fn an_unreadable_attestation_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![1u8, 2, 3]),
                [0u8; 64]
            ),
            Error::<Test>::AttestationUnreadable
        );
    });
}

#[test]
fn an_attestation_for_another_deployment_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        let held = licence_for(OTHER, 1, START + YEAR);
        let signature = control_plane().sign(&held.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(held.encode()),
                signature.0
            ),
            Error::<Test>::DeploymentMismatch
        );
    });
}

#[test]
fn a_licence_of_equal_sequence_declines() {
    new_test_ext().execute_with(|| {
        live();
        let again = licence(1, START + YEAR * 2);
        let signature = control_plane().sign(&again.message());
        assert_noop!(
            Tier::install_licence(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(again.encode()),
                signature.0
            ),
            Error::<Test>::SequenceReplayed
        );
    });
}

#[test]
fn a_licence_install_deposits_its_event_and_clears_the_suspension() {
    new_test_ext().execute_with(|| {
        live();
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));
        assert_eq!(
            Tier::suspension_reason(&DEPLOYMENT),
            Some(String::from("The subscription payment failed."))
        );
        assert_eq!(
            States::<Test>::get(DEPLOYMENT),
            Some(LicenceState::Suspended)
        );

        install(&licence(2, START + YEAR * 2));
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
        assert!(SuspensionReason::<Test>::get(DEPLOYMENT).is_none());
        assert_eq!(
            tier_events().last(),
            Some(&Event::LicenceInstalled {
                deployment: DEPLOYMENT,
                tier: TierBand {
                    application: Application::Dapps,
                    level: 1,
                },
                expires_at: START + YEAR * 2,
            })
        );
        assert_eq!(
            Tier::licence(&DEPLOYMENT).map(|held| held.sequence),
            Some(2)
        );
    });
}

#[test]
fn a_verification_pass_confirms_a_licence_that_holds() {
    new_test_ext().execute_with(|| {
        live();
        assert_ok!(Tier::verify_licence(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT
        ));
        assert_eq!(
            tier_events().last(),
            Some(&Event::LicenceVerified {
                deployment: DEPLOYMENT,
                expires_at: START + YEAR,
            })
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "licence_verified", 0, 64);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, OPERATOR);
        assert_eq!(found[0].class, WorkClass::Routine);
    });
}

#[test]
fn a_verification_pass_restores_a_lapsed_state_under_a_licence_that_holds() {
    new_test_ext().execute_with(|| {
        live();
        States::<Test>::insert(DEPLOYMENT, LicenceState::Lapsed);
        assert_ok!(Tier::verify_licence(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT
        ));
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
    });
}

#[test]
fn a_verification_pass_leaves_an_operator_suspension_in_place() {
    new_test_ext().execute_with(|| {
        live();
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));
        assert_ok!(Tier::verify_licence(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT
        ));
        assert_eq!(
            States::<Test>::get(DEPLOYMENT),
            Some(LicenceState::Suspended)
        );
    });
}

#[test]
fn a_declined_verification_leaves_the_stop_to_the_block_hook() {
    new_test_ext().execute_with(|| {
        live();
        advance_clock(YEAR);
        // The call declines, so the dispatch keeps no storage change.
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::LicenceLapsed
        );
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
        // Block production stops on the expiry alone.
        assert!(!Tier::production_permitted(&DEPLOYMENT));

        roll_one_block();
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Lapsed));
        assert_eq!(
            Tier::suspension_reason(&DEPLOYMENT),
            Some(String::from(LAPSE_SENTENCE))
        );
        let events = tier_events();
        assert!(events.contains(&Event::LicenceLapsed {
            deployment: DEPLOYMENT
        }));
        assert!(events.contains(&Event::ProductionSuspended {
            deployment: DEPLOYMENT
        }));
    });
}

#[test]
fn a_verification_pass_without_a_licence_declines() {
    new_test_ext().execute_with(|| {
        install_key();
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::LicenceMissing
        );

        install(&licence(1, START + YEAR));
        Signatures::<Test>::remove(DEPLOYMENT);
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::LicenceMissing
        );
    });
}

#[test]
fn a_verification_pass_over_a_changed_signature_declines() {
    new_test_ext().execute_with(|| {
        live();
        Signatures::<Test>::insert(DEPLOYMENT, [5u8; 64]);
        assert_noop!(
            Tier::verify_licence(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn a_seat_needs_a_licence_that_holds() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT),
            Error::<Test>::LicenceMissing
        );
        live();
        assert_ok!(Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT));
        assert_eq!(SeatsUsed::<Test>::get(DEPLOYMENT), 1);
        let found = Audit::query_by_event(&DEPLOYMENT, "seat_taken", 0, 64);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, STEWARD);

        advance_clock(YEAR);
        assert_noop!(
            Tier::take_seat(RuntimeOrigin::signed(STEWARD), DEPLOYMENT),
            Error::<Test>::LicenceLapsed
        );
    });
}

#[test]
fn usage_needs_a_licence() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Tier::record_usage(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name_of("workflows").expect("the name fits"),
                1
            ),
            Error::<Test>::LicenceMissing
        );
    });
}

#[test]
fn usage_through_the_gate_deposits_the_running_total() {
    new_test_ext().execute_with(|| {
        live();
        assert_ok!(<Tier as EntitlementGate>::record_usage(
            &DEPLOYMENT,
            "workflows",
            3,
            &STEWARD
        ));
        assert_ok!(<Tier as EntitlementGate>::record_usage(
            &DEPLOYMENT,
            "workflows",
            4,
            &STEWARD
        ));
        assert_eq!(Tier::usage_of(&DEPLOYMENT, "workflows"), 7);
        assert_eq!(
            tier_events().last(),
            Some(&Event::UsageRecorded {
                deployment: DEPLOYMENT,
                feature: name_of("workflows").expect("the name fits"),
                units: 7,
            })
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "usage_recorded", 0, 64);
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].actor, STEWARD);
    });
}

#[test]
fn a_feature_name_past_the_bound_declines_at_the_gate() {
    new_test_ext().execute_with(|| {
        live();
        let long: String = "f".repeat(129);
        assert_eq!(
            <Tier as EntitlementGate>::record_usage(&DEPLOYMENT, &long, 1, &STEWARD),
            Err(Error::<Test>::NameTooLong.into())
        );
        assert!(!Tier::entitled(&DEPLOYMENT, &long));
        assert_eq!(Tier::usage_of(&DEPLOYMENT, &long), 0);
    });
}

#[test]
fn the_reads_answer_plainly_for_a_deployment_without_a_licence() {
    new_test_ext().execute_with(|| {
        assert!(!Tier::licence_holds(&OTHER));
        assert!(!Tier::entitled(&OTHER, "workflows"));
        assert_eq!(Tier::seats(&OTHER), 0);
        assert_eq!(Tier::tier(&OTHER), None);
        assert!(Tier::licence(&OTHER).is_none());
        assert_eq!(Tier::usage_of(&OTHER, "workflows"), 0);
        assert!(Tier::suspension_reason(&OTHER).is_none());
        assert!(!<Tier as LicenceControl>::holds(&OTHER));
        // A deployment the control plane has yet to reach keeps authoring.
        assert!(Tier::production_permitted(&OTHER));
        assert!(Tier::read_permitted(&OTHER));
        assert!(Tier::export_permitted(&OTHER));
    });
}

#[test]
fn an_entitlement_ends_with_the_licence() {
    new_test_ext().execute_with(|| {
        live();
        assert!(Tier::licence_holds(&DEPLOYMENT));
        assert!(Tier::entitled(&DEPLOYMENT, "workflows"));
        advance_clock(YEAR);
        assert!(!Tier::licence_holds(&DEPLOYMENT));
        assert!(!Tier::entitled(&DEPLOYMENT, "workflows"));
        // The licence still reads back after the lapse.
        assert_eq!(Tier::seats(&DEPLOYMENT), 1);
    });
}

#[test]
fn a_resume_needs_a_stopped_deployment() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Tier::resume(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::NotSuspended
        );
        live();
        assert_noop!(
            Tier::resume(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT),
            Error::<Test>::NotSuspended
        );
    });
}

#[test]
fn a_resume_without_a_licence_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            OTHER,
            bounded(b"The deployment awaits its licence.".to_vec())
        ));
        assert!(!Tier::production_permitted(&OTHER));
        assert_noop!(
            Tier::resume(RuntimeOrigin::signed(OPERATOR), OTHER),
            Error::<Test>::LicenceMissing
        );
    });
}

#[test]
fn a_resume_deposits_its_event_and_lands_in_the_audit_log() {
    new_test_ext().execute_with(|| {
        live();
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            DEPLOYMENT,
            bounded(b"The subscription payment failed.".to_vec())
        ));
        assert!(tier_events().contains(&Event::ProductionSuspended {
            deployment: DEPLOYMENT
        }));
        assert_ok!(Tier::resume(RuntimeOrigin::signed(OPERATOR), DEPLOYMENT));
        assert_eq!(
            tier_events().last(),
            Some(&Event::ProductionResumed {
                deployment: DEPLOYMENT
            })
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "production_resumed", 0, 64);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, OPERATOR);
        assert_eq!(found[0].class, WorkClass::Constitutional);
    });
}

#[test]
fn the_block_hook_charges_nothing_over_an_empty_watch_list() {
    new_test_ext().execute_with(|| {
        assert_eq!(<Tier as Hooks<u64>>::on_initialize(2), Weight::zero());
    });
}

#[test]
fn the_block_hook_checks_every_watched_deployment_and_stops_a_lapse_alone() {
    new_test_ext().execute_with(|| {
        // One deployment holds a key and awaits its licence.
        assert_ok!(Tier::set_control_plane_key(
            RuntimeOrigin::signed(STEWARD),
            [1u8; 32],
            [1u8; 32]
        ));
        // One deployment sits under an operator suspension.
        assert_ok!(Tier::set_control_plane_key(
            RuntimeOrigin::signed(STEWARD),
            [2u8; 32],
            [2u8; 32]
        ));
        assert_ok!(Tier::suspend(
            RuntimeOrigin::signed(OPERATOR),
            [2u8; 32],
            bounded(b"The subscription payment failed.".to_vec())
        ));
        // One deployment runs under a licence that lapses.
        live();

        let charged = <Tier as Hooks<u64>>::on_initialize(2);
        assert_eq!(
            charged,
            <() as WeightInfo>::sweep_one_deployment().saturating_mul(3)
        );
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));

        advance_clock(YEAR);
        <Tier as Hooks<u64>>::on_initialize(3);
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Lapsed));
        assert_eq!(
            States::<Test>::get([2u8; 32]),
            Some(LicenceState::Suspended)
        );
        assert_eq!(
            Tier::suspension_reason(&[2u8; 32]),
            Some(String::from("The subscription payment failed."))
        );
        assert!(States::<Test>::get([1u8; 32]).is_none());
        assert!(Tier::production_permitted(&[1u8; 32]));
    });
}

#[test]
fn the_licence_control_verifies_and_reports_whether_the_licence_holds() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            <Tier as LicenceControl>::verify(&DEPLOYMENT, &OPERATOR),
            Error::<Test>::LicenceMissing
        );
        live();
        assert!(<Tier as LicenceControl>::holds(&DEPLOYMENT));
        assert_eq!(
            <Tier as LicenceControl>::verify(&DEPLOYMENT, &OPERATOR),
            Ok(true)
        );
        assert_eq!(
            tier_events().last(),
            Some(&Event::LicenceVerified {
                deployment: DEPLOYMENT,
                expires_at: START + YEAR,
            })
        );

        advance_clock(YEAR);
        System::reset_events();
        assert!(!<Tier as LicenceControl>::holds(&DEPLOYMENT));
        assert_eq!(
            <Tier as LicenceControl>::verify(&DEPLOYMENT, &OPERATOR),
            Ok(false)
        );
        assert!(tier_events().is_empty());
        let found = Audit::query_by_event(&DEPLOYMENT, "licence_verified", 0, 64);
        assert_eq!(found.len(), 2);
        assert_eq!(found[1].actor, OPERATOR);
    });
}

#[test]
fn the_licence_control_refuses_a_changed_signature() {
    new_test_ext().execute_with(|| {
        live();
        Signatures::<Test>::insert(DEPLOYMENT, [5u8; 64]);
        assert_noop!(
            <Tier as LicenceControl>::verify(&DEPLOYMENT, &OPERATOR),
            Error::<Test>::SignatureInvalid
        );
        Signatures::<Test>::remove(DEPLOYMENT);
        assert_noop!(
            <Tier as LicenceControl>::verify(&DEPLOYMENT, &OPERATOR),
            Error::<Test>::LicenceMissing
        );
    });
}

#[test]
fn the_licence_control_stops_production_on_a_lapse_alone() {
    new_test_ext().execute_with(|| {
        live();
        assert_eq!(
            <Tier as LicenceControl>::suspend_on_lapse(&DEPLOYMENT, &OPERATOR),
            Ok(false)
        );
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));

        advance_clock(YEAR);
        assert_eq!(
            <Tier as LicenceControl>::suspend_on_lapse(&DEPLOYMENT, &OPERATOR),
            Ok(true)
        );
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Lapsed));
        assert_eq!(
            Tier::suspension_reason(&DEPLOYMENT),
            Some(String::from(LAPSE_SENTENCE))
        );
        assert_eq!(
            tier_events().last(),
            Some(&Event::LicenceLapsed {
                deployment: DEPLOYMENT
            })
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "production_suspended", 0, 64);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, OPERATOR);
        assert!(!Tier::production_permitted(&DEPLOYMENT));
    });
}

#[test]
fn a_licence_answers_for_its_features_addons_and_expiry() {
    let held = licence(1, START + YEAR);
    assert!(held.enables(&name_of("workflows").expect("the name fits")));
    assert!(!held.enables(&name_of("bridges").expect("the name fits")));
    assert!(held.carries_addon(&name_of("extended_history").expect("the name fits")));
    assert!(!held.carries_addon(&name_of("workflows").expect("the name fits")));
    assert!(held.holds_at(START + YEAR - 1));
    assert!(!held.holds_at(START + YEAR));
    assert_ne!(held.message(), licence(2, START + YEAR).message());
}

#[test]
fn every_application_carries_its_label() {
    assert_eq!(Application::Dapps.label(), "dapps");
    assert_eq!(Application::Enterprise.label(), "enterprise");
    assert_eq!(Application::Government.label(), "government");
    assert!(TierBand {
        application: Application::Enterprise,
        level: 3
    }
    .is_valid());
    assert!(!TierBand {
        application: Application::Enterprise,
        level: 4
    }
    .is_valid());
    assert!(TierBand {
        application: Application::Government,
        level: 1
    }
    .is_valid());
}

#[test]
fn the_genesis_build_installs_a_verified_licence() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR);
        genesis(
            vec![(DEPLOYMENT, control_plane().public().0), (OTHER, [3u8; 32])],
            vec![signed_entry(&held)],
        )
        .build();

        assert_eq!(
            Licences::<Test>::get(DEPLOYMENT).map(|stored| stored.encode()),
            Some(held.encode())
        );
        assert_eq!(
            Signatures::<Test>::get(DEPLOYMENT),
            Some(control_plane().sign(&held.message()).0)
        );
        assert_eq!(States::<Test>::get(DEPLOYMENT), Some(LicenceState::Live));
        assert_eq!(ControlPlaneKey::<Test>::get(OTHER), Some([3u8; 32]));
        // Each deployment enters the watch list once.
        assert_eq!(Watched::<Test>::get().into_inner(), vec![DEPLOYMENT, OTHER]);
        assert!(Tier::production_permitted(&DEPLOYMENT));
        assert!(Tier::entitled(&DEPLOYMENT, "workflows"));
    });
}

#[test]
fn the_genesis_build_fills_the_watch_list_to_its_bound() {
    new_test_ext().execute_with(|| {
        let keys: Vec<(DeploymentId, [u8; 32])> =
            (1u8..=10).map(|index| ([index; 32], [index; 32])).collect();
        genesis(keys, Vec::new()).build();
        assert_eq!(Watched::<Test>::get().len(), 8);
        assert_eq!(ControlPlaneKey::<Test>::get([10u8; 32]), Some([10u8; 32]));
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence carries a signature of another length. Supply the sixty four signature bytes the control plane produced."
)]
fn a_genesis_signature_of_another_length_stops_the_build() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR);
        genesis(
            vec![(DEPLOYMENT, control_plane().public().0)],
            vec![(DEPLOYMENT, held.encode(), vec![0u8; 12])],
        )
        .build();
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence fails to decode. Supply the canonical encoding the control plane produces."
)]
fn an_unreadable_genesis_licence_stops_the_build() {
    new_test_ext().execute_with(|| {
        genesis(
            vec![(DEPLOYMENT, control_plane().public().0)],
            vec![(DEPLOYMENT, vec![1u8, 2, 3], vec![0u8; 64])],
        )
        .build();
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence names a level the application lacks. Issue the licence at a level the application carries."
)]
fn a_genesis_licence_at_a_missing_level_stops_the_build() {
    new_test_ext().execute_with(|| {
        let mut held = licence(1, START + YEAR);
        held.tier = TierBand {
            application: Application::Government,
            level: 2,
        };
        genesis(
            vec![(DEPLOYMENT, control_plane().public().0)],
            vec![signed_entry(&held)],
        )
        .build();
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence names another deployment. Install the licence issued for this deployment."
)]
fn a_genesis_licence_for_another_deployment_stops_the_build() {
    new_test_ext().execute_with(|| {
        let held = licence_for(OTHER, 1, START + YEAR);
        let (_, attestation, signature) = signed_entry(&held);
        genesis(
            vec![(DEPLOYMENT, control_plane().public().0)],
            vec![(DEPLOYMENT, attestation, signature)],
        )
        .build();
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence carries a signature the control plane key refuses. Issue the licence again."
)]
fn a_genesis_licence_under_another_key_stops_the_build() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR);
        genesis(vec![(DEPLOYMENT, [3u8; 32])], vec![signed_entry(&held)]).build();
    });
}

#[test]
#[should_panic(
    expected = "A genesis licence carries a signature the control plane key refuses. Issue the licence again."
)]
fn a_genesis_licence_without_a_control_plane_key_stops_the_build() {
    new_test_ext().execute_with(|| {
        let held = licence(1, START + YEAR);
        genesis(Vec::new(), vec![signed_entry(&held)]).build();
    });
}

#[test]
fn every_error_names_its_fix() {
    let cases: [(Error<Test>, &str); 14] = [
        (Error::NameTooLong, "Shorten the feature name."),
        (
            Error::ControlPlaneKeyMissing,
            "Install the key before installing a licence.",
        ),
        (
            Error::ControlPlaneKeyPresent,
            "Install a new licence rather than a second key.",
        ),
        (
            Error::AttestationUnreadable,
            "Submit the canonical encoding the control plane produces.",
        ),
        (
            Error::SignatureInvalid,
            "Ask the control plane to issue the licence again.",
        ),
        (
            Error::DeploymentMismatch,
            "Install the licence issued for this deployment.",
        ),
        (
            Error::SequenceReplayed,
            "Install the newest licence the control plane issued.",
        ),
        (
            Error::TierInvalid,
            "Ask the control plane to issue the licence at a level the application carries.",
        ),
        (
            Error::LicenceMissing,
            "Install the licence the control plane issued.",
        ),
        (
            Error::LicenceLapsed,
            "Renew the subscription and install the new licence.",
        ),
        (
            Error::FeatureUnlicensed,
            "Add the feature to the subscription and install the new licence.",
        ),
        (
            Error::SeatsExhausted,
            "Add seats to the subscription and install the new licence.",
        ),
        (Error::WatchListFull, "Raise the limit in the runtime."),
        (
            Error::NotSuspended,
            "Suspend the deployment before resuming it.",
        ),
    ];
    for (error, fix) in cases {
        let text = error_text(&error);
        assert!(
            text.ends_with(fix),
            "the error text {text:?} ends with the fix sentence {fix:?}"
        );
    }
}

/// The documentation text of one error, read from the type registry.
fn error_text(error: &Error<Test>) -> String {
    use scale_info::{form::PortableForm, TypeDef};
    let mut registry = scale_info::Registry::new();
    let id = registry.register_type(&scale_info::meta_type::<Error<Test>>());
    let portable: scale_info::PortableRegistry = registry.into();
    let found = portable
        .resolve(id.id)
        .expect("the registry holds the error type");
    let variants = match &found.type_def {
        TypeDef::<PortableForm>::Variant(held) => &held.variants,
        _ => panic!("the error type is an enumeration"),
    };
    let index = error.encode()[0];
    let variant = variants
        .iter()
        .find(|held| held.index == index)
        .expect("the error carries a variant");
    variant
        .docs
        .iter()
        .flat_map(|line| line.split_whitespace())
        .collect::<Vec<_>>()
        .join(" ")
}
