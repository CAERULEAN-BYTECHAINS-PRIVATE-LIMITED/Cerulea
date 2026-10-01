//! Access control on the calls that change how a deployment runs.
//!
//! Task P10-027 closes four gaps the adversarial review found. Each test
//! drives the real runtime. An account outside the authority meets a named
//! refusal and the account holding the authority succeeds.
//!
//! 1. **The control plane key.** The first key binds to the account that
//!    installed the deployment. A later key arrives through a rotation the
//!    current key signed, or through the root origin.
//! 2. **The relayer settings.** Sponsors, quotas, rate limits and throttles
//!    come from an account that administers the deployment.
//! 3. **The operator role.** The root origin or an active operator appoints
//!    and retires an operator.
//! 4. **The validator licence.** While the chain holds a licence key, a
//!    validator joins only under a licence attestation that key signed.

mod support;

use cerulea_runtime::{AccountId, Runtime, RuntimeOrigin};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_dvf::engines::admission::licence::{
    attestation_payload, LicenceAttestation, LicenceClass,
};
use pallet_cerulea_schema::primitives::DeploymentId;
use sp_core::{ed25519, Pair};
use sp_keyring::Ed25519Keyring;
use support::*;

type Tier = pallet_cerulea_tier::Pallet<Runtime>;
type Relayer = pallet_cerulea_relayer::Pallet<Runtime>;
type Operator = pallet_cerulea_operator::Pallet<Runtime>;
type Validators = pallet_cerulea_dvf::Pallet<Runtime>;

/// The deployment the tests administer.
const DEPLOYMENT: DeploymentId = [0xc4; 32];

/// The control plane key the installer names first.
fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x31; 32])
}

/// A key nobody installed.
fn foreign() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[0x32; 32])
}

/// An account with no authority over the deployment.
fn stranger() -> AccountId {
    Ed25519Keyring::Charlie.to_account_id()
}

/// Record the test actor as the account that installed the deployment.
fn claim_deployment() {
    pallet_cerulea_logic::InstalledBy::<Runtime>::insert(DEPLOYMENT, actor());
}

/// Finding one. The first control plane key binds to the installer.
///
/// A stranger cannot name the first key. The installer names it. A stranger
/// cannot rotate it without a signature from the current key, nor with a
/// signature from a foreign key. A rotation the current key signed lands,
/// and the root origin replaces the key outright.
#[test]
fn the_control_plane_key_binds_to_the_installer() {
    development_ext().execute_with(|| {
        claim_deployment();
        assert_noop!(
            Tier::set_control_plane_key(
                RuntimeOrigin::signed(stranger()),
                DEPLOYMENT,
                foreign().public().0
            ),
            pallet_cerulea_tier::Error::<Runtime>::NotInstaller
        );
        assert_ok!(Tier::set_control_plane_key(
            RuntimeOrigin::signed(actor()),
            DEPLOYMENT,
            control_plane().public().0
        ));
        assert_noop!(
            Tier::set_control_plane_key(
                RuntimeOrigin::signed(actor()),
                DEPLOYMENT,
                foreign().public().0
            ),
            pallet_cerulea_tier::Error::<Runtime>::ControlPlaneKeyPresent
        );

        let next = ed25519::Pair::from_seed(&[0x33; 32]).public().0;
        let message = pallet_cerulea_tier::rotation_message(&DEPLOYMENT, &next);
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::signed(stranger()),
                DEPLOYMENT,
                next,
                None
            ),
            pallet_cerulea_tier::Error::<Runtime>::RotationUnsigned
        );
        assert_noop!(
            Tier::rotate_control_plane_key(
                RuntimeOrigin::signed(stranger()),
                DEPLOYMENT,
                next,
                Some(foreign().sign(&message).0)
            ),
            pallet_cerulea_tier::Error::<Runtime>::SignatureInvalid
        );
        assert_ok!(Tier::rotate_control_plane_key(
            RuntimeOrigin::signed(stranger()),
            DEPLOYMENT,
            next,
            Some(control_plane().sign(&message).0)
        ));
        assert_eq!(
            pallet_cerulea_tier::ControlPlaneKey::<Runtime>::get(DEPLOYMENT),
            Some(next)
        );
        assert_ok!(Tier::rotate_control_plane_key(
            RuntimeOrigin::root(),
            DEPLOYMENT,
            control_plane().public().0,
            None
        ));
        assert_eq!(
            pallet_cerulea_tier::ControlPlaneKey::<Runtime>::get(DEPLOYMENT),
            Some(control_plane().public().0)
        );

        // Suspension and resumption come from an account that administers
        // the deployment.
        let reason: BoundedVec<u8, _> =
            BoundedVec::truncate_from(b"A stranger stops the chain.".to_vec());
        assert_noop!(
            Tier::suspend(
                RuntimeOrigin::signed(stranger()),
                DEPLOYMENT,
                reason.clone()
            ),
            pallet_cerulea_tier::Error::<Runtime>::NotSteward
        );
        assert_noop!(
            Tier::resume(RuntimeOrigin::signed(stranger()), DEPLOYMENT),
            pallet_cerulea_tier::Error::<Runtime>::NotSteward
        );
    });
}

/// Finding two. The relayer settings come from a steward of the deployment.
///
/// A stranger meets `NotSteward` on each of the six calls. The installer
/// succeeds on each.
#[test]
fn the_relayer_settings_come_from_a_steward() {
    development_ext().execute_with(|| {
        claim_deployment();
        let action = name("record_parcel");
        for who in [stranger(), actor()] {
            let origin = || RuntimeOrigin::signed(who.clone());
            let outcomes = [
                Relayer::register_sponsor(origin(), DEPLOYMENT, outsider(), 100),
                Relayer::set_quota(origin(), DEPLOYMENT, 3_600_000, 100),
                Relayer::set_account_limit(origin(), DEPLOYMENT, 3_600_000, 10),
                Relayer::set_action_limit(origin(), DEPLOYMENT, action.clone(), 3_600_000, 10),
                Relayer::throttle(origin(), DEPLOYMENT, outsider(), 5),
                Relayer::lift_throttle(origin(), DEPLOYMENT, outsider()),
            ];
            for (position, outcome) in outcomes.into_iter().enumerate() {
                if who == actor() {
                    assert_ok!(outcome);
                } else {
                    assert_eq!(
                        outcome,
                        Err(pallet_cerulea_relayer::Error::<Runtime>::NotSteward.into()),
                        "Call {position} refuses the stranger."
                    );
                }
            }
        }
    });
}
