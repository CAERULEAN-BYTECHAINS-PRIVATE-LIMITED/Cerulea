//! The tests of the relayer module.
//!
//! The citizen authorises a land registration. The relayer submits and
//! carries the cost. Every test checks that the Blockchain records the
//! citizen as the actor.

use crate::{mock::*, Authorisation, Error, RelayedActor, Sponsors, Submissions, RELAY_FEATURE};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use pallet_cerulea_tier::{Application, Tier as TierBand};
use sp_core::{ed25519, Pair};

extern crate alloc;
use alloc::{vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const CITIZEN: u64 = 2;
const RELAYER: u64 = 3;
const HOUR: u64 = 3_600_000;
const YEAR: u64 = 31_536_000_000;

type TestLicence = pallet_cerulea_tier::Licence<ConstU32<32>, ConstU32<16>>;

fn control_plane() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[7u8; 32])
}

fn citizen_key() -> ed25519::Pair {
    ed25519::Pair::from_seed(&[3u8; 32])
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn action() -> BoundedName {
    name_of("register_parcel").expect("the name fits")
}

/// Install a licence that enables relayed submission.
fn install_licence() {
    assert_ok!(Tier::set_control_plane_key(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        control_plane().public().0
    ));
    let licence = TestLicence {
        deployment: DEPLOYMENT,
        tier: TierBand {
            application: Application::Government,
            level: 1,
        },
        features: BoundedVec::try_from(vec![
            name_of(RELAY_FEATURE).expect("the name fits"),
            name_of("workflows").expect("the name fits"),
        ])
        .expect("the list fits"),
        seats: 100,
        addons: BoundedVec::default(),
        issued_at: START,
        expires_at: START + YEAR,
        sequence: 1,
    };
    let signature = control_plane().sign(&licence.message());
    assert_ok!(Tier::install_licence(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(licence.encode()),
        signature.0
    ));
}

fn install() {
    install_licence();
    assert_ok!(Identity::register_account(
        RuntimeOrigin::signed(STEWARD),
        CITIZEN,
        [2u8; 32]
    ));
    assert_ok!(Relayer::register_signing_key(
        RuntimeOrigin::signed(CITIZEN),
        DEPLOYMENT,
        citizen_key().public().0
    ));
    assert_ok!(Relayer::register_sponsor(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        RELAYER,
        1_000
    ));
    assert_ok!(Relayer::set_quota(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        HOUR,
        100
    ));
}

fn authorisation(nonce: u64) -> Authorisation<u64> {
    Authorisation {
        deployment: DEPLOYMENT,
        actor: CITIZEN,
        action: action(),
        payload_hash: [4u8; 32],
        valid_until: clock() + 60_000,
        nonce,
    }
}

fn relay(held: &Authorisation<u64>) -> frame_support::pallet_prelude::DispatchResult {
    let signature = citizen_key().sign(&held.message());
    Relayer::submit_on_behalf(
        RuntimeOrigin::signed(RELAYER),
        DEPLOYMENT,
        held.clone(),
        signature.0,
    )
}

#[test]
fn a_sponsor_registers_and_reads_back() {
    new_test_ext().execute_with(|| {
        install();
        let held = Sponsors::<Test>::get(DEPLOYMENT, RELAYER).expect("the sponsor is registered");
        assert_eq!(held.budget_units, 1_000);
        assert_eq!(held.spent_units, 0);
        assert!(held.active);
        assert_eq!(Relayer::sponsor_remaining(&DEPLOYMENT, &RELAYER), 1_000);
    });
}

#[test]
fn a_relayed_submission_records_the_authoriser_as_the_actor() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));

        let held = Relayer::submission(&DEPLOYMENT, 0).expect("the module holds the submission");
        assert_eq!(held.actor, CITIZEN);
        assert_eq!(held.relayer, RELAYER);
        assert_eq!(held.action, action());
        assert_eq!(held.units, 10);
        assert_eq!(Relayer::actor_of(&DEPLOYMENT, 0), Some(CITIZEN));

        // The audit log names the citizen rather than the relayer.
        let records = Audit::query_by_event(&DEPLOYMENT, "submitted_on_behalf", 0, 64);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].actor, CITIZEN);
        assert_eq!(records[0].module, "relayer");
        assert_eq!(records[0].class, WorkClass::Routine);
    });
}

#[test]
fn metering_continues_while_the_user_meets_no_fee() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));
        assert_ok!(relay(&authorisation(2)));

        // The sponsor carries the cost.
        assert_eq!(Relayer::sponsor_remaining(&DEPLOYMENT, &RELAYER), 980);
        // The tenant quota falls by the same units.
        assert_eq!(Relayer::quota_remaining(&DEPLOYMENT), 80);
        // The tier module holds the usage for the licensed feature.
        assert_eq!(Tier::usage_of(&DEPLOYMENT, RELAY_FEATURE), 20);

        let (used, left, count) = Relayer::usage_report(&DEPLOYMENT);
        assert_eq!(used, 20);
        assert_eq!(left, 80);
        assert_eq!(count, 2);
    });
}

#[test]
fn a_signature_from_another_key_declines() {
    new_test_ext().execute_with(|| {
        install();
        let held = authorisation(1);
        let impostor = ed25519::Pair::from_seed(&[9u8; 32]);
        let signature = impostor.sign(&held.message());
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(RELAYER),
                DEPLOYMENT,
                held,
                signature.0
            ),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn a_changed_authorisation_declines() {
    new_test_ext().execute_with(|| {
        install();
        let held = authorisation(1);
        let signature = citizen_key().sign(&held.message());
        let mut changed = held.clone();
        changed.payload_hash = [5u8; 32];
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(RELAYER),
                DEPLOYMENT,
                changed,
                signature.0
            ),
            Error::<Test>::SignatureInvalid
        );
    });
}

#[test]
fn one_authorisation_number_runs_once() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));
        assert_noop!(relay(&authorisation(1)), Error::<Test>::NonceUsed);
    });
}

#[test]
fn an_authorisation_past_its_moment_declines() {
    new_test_ext().execute_with(|| {
        install();
        let held = authorisation(1);
        advance_clock(120_000);
        assert_noop!(relay(&held), Error::<Test>::AuthorisationExpired);
    });
}

#[test]
fn an_authorisation_valid_past_the_span_declines() {
    new_test_ext().execute_with(|| {
        install();
        let mut held = authorisation(1);
        held.valid_until = clock() + HOUR * 4;
        assert_noop!(relay(&held), Error::<Test>::AuthorisationTooLong);
    });
}

#[test]
fn an_account_with_no_signing_key_declines() {
    new_test_ext().execute_with(|| {
        install_licence();
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(STEWARD),
            CITIZEN,
            [2u8; 32]
        ));
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            RELAYER,
            1_000
        ));
        assert_ok!(Relayer::set_quota(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            100
        ));
        assert_noop!(relay(&authorisation(1)), Error::<Test>::SigningKeyMissing);
    });
}

#[test]
fn an_unregistered_account_declines() {
    new_test_ext().execute_with(|| {
        install();
        let mut held = authorisation(1);
        held.actor = 44;
        assert_noop!(relay(&held), Error::<Test>::ActorUnregistered);
    });
}

#[test]
fn a_deployment_without_the_feature_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(STEWARD),
            CITIZEN,
            [2u8; 32]
        ));
        assert_ok!(Relayer::register_signing_key(
            RuntimeOrigin::signed(CITIZEN),
            DEPLOYMENT,
            citizen_key().public().0
        ));
        assert_noop!(relay(&authorisation(1)), Error::<Test>::FeatureUnlicensed);
    });
}

#[test]
fn a_submission_naming_another_deployment_declines() {
    new_test_ext().execute_with(|| {
        install();
        let mut held = authorisation(1);
        held.deployment = [4u8; 32];
        assert_noop!(relay(&held), Error::<Test>::DeploymentMismatch);
    });
}

#[test]
fn a_sponsor_out_of_budget_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            RELAYER,
            10
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_noop!(relay(&authorisation(2)), Error::<Test>::SponsorExhausted);
    });
}

#[test]
fn an_unknown_sponsor_declines() {
    new_test_ext().execute_with(|| {
        install();
        let held = authorisation(1);
        let signature = citizen_key().sign(&held.message());
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                held,
                signature.0
            ),
            Error::<Test>::SponsorUnknown
        );
    });
}

#[test]
fn a_tenant_out_of_quota_declines_until_the_next_period() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_quota(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            20
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_ok!(relay(&authorisation(2)));
        assert_noop!(relay(&authorisation(3)), Error::<Test>::QuotaExhausted);

        // The next period restores the quota of the tenant.
        advance_clock(HOUR);
        assert_eq!(Relayer::quota_remaining(&DEPLOYMENT), 20);
        assert_ok!(relay(&authorisation(4)));
    });
}

#[test]
fn a_rate_limit_per_account_holds_the_window() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            2
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_ok!(relay(&authorisation(2)));
        assert_noop!(relay(&authorisation(3)), Error::<Test>::AccountRateLimited);

        advance_clock(HOUR);
        assert_ok!(relay(&authorisation(4)));
    });
}

#[test]
fn a_rate_limit_per_action_holds_the_window() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_action_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            action(),
            HOUR,
            1
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_noop!(relay(&authorisation(2)), Error::<Test>::ActionRateLimited);

        // Another action stays open inside the same window.
        let mut other = authorisation(3);
        other.action = name_of("transfer_parcel").expect("the name fits");
        assert_ok!(relay(&other));
    });
}

#[test]
fn a_throttled_account_submits_nothing_until_the_moment_passes() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN,
            clock() + HOUR
        ));
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        assert_noop!(relay(&authorisation(1)), Error::<Test>::AccountThrottled);

        assert_ok!(Relayer::lift_throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN
        ));
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        assert_ok!(relay(&authorisation(1)));
    });
}

#[test]
fn the_quota_check_answers_before_a_submission() {
    new_test_ext().execute_with(|| {
        install();
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        assert_ok!(Relayer::set_quota(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            0
        ));
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
    });
}

#[test]
fn a_deployment_without_a_quota_declines() {
    new_test_ext().execute_with(|| {
        install_licence();
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(STEWARD),
            CITIZEN,
            [2u8; 32]
        ));
        assert_ok!(Relayer::register_signing_key(
            RuntimeOrigin::signed(CITIZEN),
            DEPLOYMENT,
            citizen_key().public().0
        ));
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            RELAYER,
            1_000
        ));
        assert_noop!(relay(&authorisation(1)), Error::<Test>::QuotaMissing);
    });
}

#[test]
fn two_accounts_carry_two_windows() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            1
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_noop!(relay(&authorisation(2)), Error::<Test>::AccountRateLimited);

        // A second citizen carries a window of its own.
        let second_key = ed25519::Pair::from_seed(&[6u8; 32]);
        assert_ok!(Identity::register_account(
            RuntimeOrigin::signed(STEWARD),
            5,
            [5u8; 32]
        ));
        assert_ok!(Relayer::register_signing_key(
            RuntimeOrigin::signed(5),
            DEPLOYMENT,
            second_key.public().0
        ));
        let mut held = authorisation(1);
        held.actor = 5;
        let signature = second_key.sign(&held.message());
        assert_ok!(Relayer::submit_on_behalf(
            RuntimeOrigin::signed(RELAYER),
            DEPLOYMENT,
            held,
            signature.0
        ));
        assert_eq!(Submissions::<Test>::iter_prefix(DEPLOYMENT).count(), 2);
    });
}

#[test]
fn every_relayer_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));

        let sponsors = Audit::query_by_event(&DEPLOYMENT, "sponsor_registered", 0, 64);
        assert_eq!(sponsors.len(), 1);
        assert_eq!(sponsors[0].actor, STEWARD);

        let keys = Audit::query_by_event(&DEPLOYMENT, "signing_key_registered", 0, 64);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].actor, CITIZEN);

        let relayed = Audit::query_by_event(&DEPLOYMENT, "submitted_on_behalf", 0, 64);
        assert_eq!(relayed.len(), 1);
        assert_eq!(relayed[0].actor, CITIZEN);
    });
}
