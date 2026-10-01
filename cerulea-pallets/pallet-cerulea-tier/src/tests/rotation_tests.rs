//! Tests of the control plane key rotation.
//!
//! The holder of the current key signs the new key under the rotation
//! separator, and any account submits the signed rotation. The root origin
//! replaces the key with no signature. After a rotation the Blockchain
//! accepts licences the new key signs and declines licences the old key
//! signs.

use crate::{
    mock::*, rotation_message, Application, ControlPlaneKey, Error, Event, Tier as TierBand,
    ROTATION_DOMAIN,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, DeploymentId};
use sp_core::{ed25519, Pair};

extern crate alloc;
use alloc::{vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const OTHER: DeploymentId = [4u8; 32];
const STEWARD: u64 = 1;
const STRANGER: u64 = 7;
const YEAR: u64 = 31_536_000_000;

type TestLicence = crate::Licence<ConstU32<32>, ConstU32<16>>;

fn first_key() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[7u8; 32])
}

fn second_key() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[8u8; 32])
}

fn licence(sequence: u64) -> TestLicence {
    TestLicence {
        deployment: DEPLOYMENT,
        tier: TierBand {
            application: Application::Dapps,
            level: 1,
        },
        features: BoundedVec::try_from(vec![name_of("workflows").expect("the name fits")])
            .expect("the list fits"),
        seats: 1,
        addons: BoundedVec::default(),
        issued_at: START,
        expires_at: START + YEAR,
        sequence,
    }
}

fn install_with(
    pair: &ed25519::Pair,
    held: &TestLicence,
) -> frame_support::dispatch::DispatchResult {
    let signature = pair.sign(&held.message());
    Tier::install_licence(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        BoundedVec::try_from(held.encode()).expect("the payload fits"),
        signature.0,
    )
}

fn install_first_key() {
    assert_ok!(Tier::set_control_plane_key(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        first_key().public().0
    ));
}

fn signed_rotation(by: &ed25519::Pair, deployment: &DeploymentId, to: &ed25519::Pair) -> [u8; 64] {
    by.sign(&rotation_message(deployment, &to.public().0)).0
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

#[test]
fn the_rotation_message_carries_the_separator_the_deployment_and_the_key() {
    let key = [3u8; 32];
    let message = rotation_message(&DEPLOYMENT, &key);
    assert_eq!(message, (ROTATION_DOMAIN, DEPLOYMENT, key).encode());
    assert_ne!(message, rotation_message(&OTHER, &key));
    assert_ne!(message, rotation_message(&DEPLOYMENT, &[4u8; 32]));
}

#[test]
fn a_rotation_the_current_key_signs_replaces_the_key() {
    new_test_ext().execute_with(|| {
        install_first_key();
        System::reset_events();
        let signature = signed_rotation(&first_key(), &DEPLOYMENT, &second_key());
        assert_ok!(Tier::rotate_control_plane_key(
            RuntimeOrigin::signed(STRANGER),
            DEPLOYMENT,
            second_key().public().0,
            Some(signature)
        ));
        assert_eq!(
            ControlPlaneKey::<Test>::get(DEPLOYMENT),
            Some(second_key().public().0)
        );
        assert!(tier_events().contains(&Event::ControlPlaneKeySet {
            deployment: DEPLOYMENT
        }));
        let found = Audit::query_by_event(&DEPLOYMENT, "control_plane_key_rotated", 0, 8);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].actor, STRANGER);
    });
}

#[test]
fn after_a_rotation_the_new_key_signs_licences_and_the_old_key_declines() {
    new_test_ext().execute_with(|| {
        install_first_key();
        assert_ok!(install_with(&first_key(), &licence(1)));
        let signature = signed_rotation(&first_key(), &DEPLOYMENT, &second_key());
        assert_ok!(Tier::rotate_control_plane_key(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            second_key().public().0,
            Some(signature)
        ));
        assert_noop!(
            install_with(&first_key(), &licence(2)),
            Error::<Test>::SignatureInvalid
        );
        assert_ok!(install_with(&second_key(), &licence(2)));
    });
}

#[test]
fn a_signed_rotation_without_a_signature_declines() {
    new_test_ext().execute_with(|| {
        install_first_key();
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                second_key().public().0,
                None
            ),
            Error::<Test>::RotationUnsigned
        );
        assert_eq!(
            ControlPlaneKey::<Test>::get(DEPLOYMENT),
            Some(first_key().public().0)
        );
    });
}

#[test]
fn a_rotation_signed_by_another_key_or_for_another_deployment_declines() {
    new_test_ext().execute_with(|| {
        install_first_key();
        let by_new_key = signed_rotation(&second_key(), &DEPLOYMENT, &second_key());
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                second_key().public().0,
                Some(by_new_key)
            ),
            Error::<Test>::SignatureInvalid
        );
        let for_other = signed_rotation(&first_key(), &OTHER, &second_key());
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                second_key().public().0,
                Some(for_other)
            ),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn a_rotation_before_any_key_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::root(),
                DEPLOYMENT,
                second_key().public().0,
                None
            ),
            Error::<Test>::ControlPlaneKeyMissing
        );
    });
}

#[test]
fn the_root_origin_rotates_with_no_signature_and_names_the_installer() {
    new_test_ext().execute_with(|| {
        install_first_key();
        assert_ok!(Tier::rotate_control_plane_key(
            RuntimeOrigin::root(),
            DEPLOYMENT,
            second_key().public().0,
            None
        ));
        assert_eq!(
            ControlPlaneKey::<Test>::get(DEPLOYMENT),
            Some(second_key().public().0)
        );
        let found = Audit::query_by_event(&DEPLOYMENT, "control_plane_key_rotated", 0, 8);
        assert_eq!(found.len(), 1);
        // The mock names account one as the installer of every deployment.
        assert_eq!(found[0].actor, STEWARD);
    });
}
