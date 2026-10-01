//! Further tests of the relayer module.
//!
//! These tests cover the signed origin of every call, the events every call
//! deposits, the rolling of quota periods and rate windows, the answers of
//! the quota check and the read functions over empty storage.

use crate::{
    action_text, mock::*, AccountLimit, AccountWindow, Authorisation, Error, Event, NextSubmission,
    Quotas, RateLimit, RateWindow, RelayedActor, SigningKeys, Sponsor, Sponsors, Submissions,
    TenantQuota, Throttled, UsedNonces, RELAY_FEATURE,
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use pallet_cerulea_tier::{Application, Tier as TierBand};
use sp_core::{ed25519, Pair};
use sp_runtime::DispatchError;

extern crate alloc;
use alloc::{string::String, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const OTHER: DeploymentId = [8u8; 32];
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
        features: BoundedVec::try_from(vec![name_of(RELAY_FEATURE).expect("the name fits")])
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

/// The inner call every relayed submission in these tests carries.
fn inner_call() -> RuntimeCall {
    RuntimeCall::System(frame_system::Call::remark {
        remark: b"relayed".to_vec(),
    })
}

/// The content hash the authorisation names for the inner call.
fn inner_hash() -> [u8; 32] {
    sp_io::hashing::blake2_256(&codec::Encode::encode(&inner_call()))
}

fn authorisation(nonce: u64) -> Authorisation<u64> {
    Authorisation {
        deployment: DEPLOYMENT,
        actor: CITIZEN,
        action: action(),
        payload_hash: inner_hash(),
        valid_until: clock() + 60_000,
        nonce,
    }
}

fn relay(held: &Authorisation<u64>) -> sp_runtime::DispatchResult {
    let signature = citizen_key().sign(&held.message());
    Relayer::submit_on_behalf(
        RuntimeOrigin::signed(RELAYER),
        DEPLOYMENT,
        held.clone(),
        signature.0,
        Box::new(inner_call()),
    )
}

fn relayer_events() -> Vec<Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::Relayer(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

#[test]
fn every_call_asks_for_a_signed_origin() {
    new_test_ext().execute_with(|| {
        let held = authorisation(1);
        assert_noop!(
            Relayer::register_sponsor(RuntimeOrigin::none(), DEPLOYMENT, RELAYER, 10),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::set_quota(RuntimeOrigin::none(), DEPLOYMENT, HOUR, 10),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::set_account_limit(RuntimeOrigin::none(), DEPLOYMENT, HOUR, 1),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::set_action_limit(RuntimeOrigin::none(), DEPLOYMENT, action(), HOUR, 1),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::register_signing_key(RuntimeOrigin::none(), DEPLOYMENT, [1u8; 32]),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::submit_on_behalf(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                held,
                [0u8; 64],
                Box::new(inner_call())
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::throttle(RuntimeOrigin::none(), DEPLOYMENT, CITIZEN, 5),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Relayer::lift_throttle(RuntimeOrigin::root(), DEPLOYMENT, CITIZEN),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_unregistered_account_registers_no_signing_key() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Relayer::register_signing_key(RuntimeOrigin::signed(CITIZEN), DEPLOYMENT, [1u8; 32]),
            Error::<Test>::ActorUnregistered
        );
        assert!(SigningKeys::<Test>::get(DEPLOYMENT, CITIZEN).is_none());
    });
}

#[test]
fn every_steward_call_deposits_its_event() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            5
        ));
        assert_ok!(Relayer::set_action_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            action(),
            HOUR,
            3
        ));
        assert_ok!(Relayer::throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN,
            START + HOUR
        ));
        assert_ok!(Relayer::lift_throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN
        ));

        let events = relayer_events();
        assert!(events.contains(&Event::SigningKeyRegistered {
            deployment: DEPLOYMENT,
            account: CITIZEN,
        }));
        assert!(events.contains(&Event::SponsorRegistered {
            deployment: DEPLOYMENT,
            sponsor: RELAYER,
            budget_units: 1_000,
        }));
        assert!(events.contains(&Event::QuotaSet {
            deployment: DEPLOYMENT,
            units_per_period: 100,
        }));
        // The account limit carries an empty action name.
        assert!(events.contains(&Event::RateLimitSet {
            deployment: DEPLOYMENT,
            action: BoundedName::default(),
            max_count: 5,
        }));
        assert!(events.contains(&Event::RateLimitSet {
            deployment: DEPLOYMENT,
            action: action(),
            max_count: 3,
        }));
        assert!(events.contains(&Event::AccountThrottled {
            deployment: DEPLOYMENT,
            account: CITIZEN,
            until: START + HOUR,
        }));
        assert_eq!(
            events.last(),
            Some(&Event::ThrottleLifted {
                deployment: DEPLOYMENT,
                account: CITIZEN,
            })
        );
        assert!(Throttled::<Test>::get(DEPLOYMENT, CITIZEN).is_none());
        assert_eq!(
            AccountLimit::<Test>::get(DEPLOYMENT),
            Some(RateLimit {
                window_millis: HOUR,
                max_count: 5,
            })
        );
    });
}

#[test]
fn every_steward_call_lands_in_the_audit_log_with_its_class() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            5
        ));
        assert_ok!(Relayer::set_action_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            action(),
            HOUR,
            3
        ));
        assert_ok!(Relayer::throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN,
            START + HOUR
        ));
        assert_ok!(Relayer::lift_throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN
        ));

        for (event, class) in [
            ("quota_set", WorkClass::Structural),
            ("account_limit_set", WorkClass::Structural),
            ("action_limit_set", WorkClass::Structural),
            ("account_throttled", WorkClass::Routine),
            ("throttle_lifted", WorkClass::Routine),
        ] {
            let found = Audit::query_by_event(&DEPLOYMENT, event, 0, 64);
            assert_eq!(found.len(), 1, "the log holds one {event} record");
            assert_eq!(found[0].actor, STEWARD);
            assert_eq!(found[0].module, "relayer");
            assert_eq!(found[0].class, class);
        }
    });
}

#[test]
fn a_relayed_submission_deposits_its_event_and_stores_every_field() {
    new_test_ext().execute_with(|| {
        install();
        System::set_block_number(7);
        let held = authorisation(11);
        assert_ok!(relay(&held));

        assert_eq!(
            relayer_events().last(),
            Some(&Event::SubmittedOnBehalf {
                deployment: DEPLOYMENT,
                submission: 0,
                actor: CITIZEN,
                relayer: RELAYER,
                action: action(),
            })
        );
        let stored = Submissions::<Test>::get(DEPLOYMENT, 0).expect("the module holds the record");
        assert_eq!(stored.payload_hash, inner_hash());
        assert_eq!(stored.nonce, 11);
        assert_eq!(stored.at_millis, START);
        assert_eq!(stored.block, 7);
        assert_eq!(NextSubmission::<Test>::get(DEPLOYMENT), 1);
        assert!(UsedNonces::<Test>::get((DEPLOYMENT, CITIZEN, 11)).is_some());
        assert_eq!(action_text(&stored.action), "register_parcel");
    });
}

#[test]
fn an_inactive_sponsor_relays_nothing() {
    new_test_ext().execute_with(|| {
        install();
        Sponsors::<Test>::mutate(DEPLOYMENT, RELAYER, |slot| {
            if let Some(held) = slot.as_mut() {
                held.active = false;
            }
        });
        assert_noop!(relay(&authorisation(1)), Error::<Test>::SponsorInactive);

        // A fresh registration activates the sponsor again.
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            RELAYER,
            1_000
        ));
        assert_ok!(relay(&authorisation(1)));
    });
}

#[test]
fn a_fresh_sponsor_registration_keeps_the_units_already_spent() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));
        assert_ok!(Relayer::register_sponsor(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            RELAYER,
            50
        ));
        assert_eq!(
            Sponsors::<Test>::get(DEPLOYMENT, RELAYER),
            Some(Sponsor {
                budget_units: 50,
                spent_units: 10,
                active: true,
            })
        );
        assert_eq!(Relayer::sponsor_remaining(&DEPLOYMENT, &RELAYER), 40);
    });
}

#[test]
fn a_spent_past_budget_sponsor_reports_nothing_left() {
    let held = Sponsor {
        budget_units: 5,
        spent_units: 9,
        active: true,
    };
    assert_eq!(held.remaining(), 0);
    let quota = TenantQuota {
        period_millis: HOUR,
        units_per_period: 5,
        used_units: 9,
        period_start: 0,
    };
    assert_eq!(quota.remaining(), 0);
}

#[test]
fn a_fresh_quota_keeps_the_period_and_the_units_already_used() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));
        advance_clock(1_000);
        assert_ok!(Relayer::set_quota(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR * 2,
            300
        ));
        assert_eq!(
            Quotas::<Test>::get(DEPLOYMENT),
            Some(TenantQuota {
                period_millis: HOUR * 2,
                units_per_period: 300,
                used_units: 10,
                period_start: START,
            })
        );
        assert_eq!(Relayer::quota_remaining(&DEPLOYMENT), 290);
    });
}

#[test]
fn a_quota_rolls_forward_by_whole_periods() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));

        // Three and a half periods later the period starts at the third
        // boundary and the used units return to zero.
        advance_clock(HOUR * 3 + HOUR / 2);
        let rolled = Relayer::quota_state(&DEPLOYMENT).expect("the tenant holds a quota");
        assert_eq!(rolled.period_start, START + HOUR * 3);
        assert_eq!(rolled.used_units, 0);

        assert_ok!(relay(&authorisation(2)));
        let stored = Quotas::<Test>::get(DEPLOYMENT).expect("the tenant holds a quota");
        assert_eq!(stored.period_start, START + HOUR * 3);
        assert_eq!(stored.used_units, 10);
    });
}

#[test]
fn a_quota_with_no_period_never_restores() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_quota(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            0,
            10
        ));
        assert_ok!(relay(&authorisation(1)));
        advance_clock(HOUR * 24);
        assert_eq!(Relayer::quota_remaining(&DEPLOYMENT), 0);
        assert_noop!(relay(&authorisation(2)), Error::<Test>::QuotaExhausted);
    });
}

#[test]
fn a_rate_window_with_no_span_holds_its_count_for_good() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            0,
            1
        ));
        assert_ok!(relay(&authorisation(1)));

        // The first submission stamps the start of the window.
        assert_eq!(
            AccountWindow::<Test>::get(DEPLOYMENT, CITIZEN),
            RateWindow {
                window_start: START,
                count: 1,
            }
        );
        advance_clock(HOUR * 24);
        assert_noop!(relay(&authorisation(2)), Error::<Test>::AccountRateLimited);
    });
}

#[test]
fn an_action_window_with_no_span_holds_its_count_for_good() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_action_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            action(),
            0,
            1
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_eq!(
            Relayer::action_window(&DEPLOYMENT, &CITIZEN, &action()),
            RateWindow {
                window_start: START,
                count: 1,
            }
        );
        advance_clock(HOUR * 24);
        assert_noop!(relay(&authorisation(2)), Error::<Test>::ActionRateLimited);
    });
}

#[test]
fn a_rate_window_restarts_once_its_span_passes() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            3
        ));
        assert_ok!(relay(&authorisation(1)));
        assert_eq!(Relayer::account_window(&DEPLOYMENT, &CITIZEN).count, 1);

        advance_clock(HOUR);
        let fresh = Relayer::account_window(&DEPLOYMENT, &CITIZEN);
        assert_eq!(fresh.count, 0);
        assert_eq!(fresh.window_start, START + HOUR);
    });
}

#[test]
fn a_window_without_a_limit_reads_back_unchanged() {
    new_test_ext().execute_with(|| {
        let window = RateWindow {
            window_start: 5,
            count: 4,
        };
        assert_eq!(Relayer::rolled(window, None), window);
        assert_eq!(
            Relayer::account_window(&DEPLOYMENT, &CITIZEN),
            RateWindow::default()
        );
    });
}

#[test]
fn the_deployment_runs_out_of_submission_identifiers() {
    new_test_ext().execute_with(|| {
        install();
        NextSubmission::<Test>::insert(DEPLOYMENT, u64::MAX);
        assert_noop!(relay(&authorisation(1)), Error::<Test>::RegisterFull);
    });
}

#[test]
fn a_throttle_lifts_by_itself_once_its_moment_passes() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::throttle(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            CITIZEN,
            START + HOUR
        ));
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));

        advance_clock(HOUR);
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        assert_ok!(relay(&authorisation(1)));
    });
}

#[test]
fn the_quota_check_follows_both_rate_limits() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Relayer::set_account_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            HOUR,
            2
        ));
        assert_ok!(Relayer::set_action_limit(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            action(),
            HOUR,
            1
        ));
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        assert_ok!(relay(&authorisation(1)));

        // The action limit holds the named action back.
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, "register_parcel"));
        // Another action stays open under the account limit.
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, "transfer_parcel"));

        let mut other = authorisation(2);
        other.action = name_of("transfer_parcel").expect("the name fits");
        assert_ok!(relay(&other));

        // The account limit now holds every action back.
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, "survey_parcel"));
    });
}

#[test]
fn the_quota_check_refuses_an_action_name_past_the_bound() {
    new_test_ext().execute_with(|| {
        install();
        let long: String = "a".repeat(129);
        assert!(!Relayer::admits(&DEPLOYMENT, &CITIZEN, &long));
        assert!(Relayer::admits(&DEPLOYMENT, &CITIZEN, &"a".repeat(128)));
    });
}

#[test]
fn the_reads_answer_plainly_over_an_empty_deployment() {
    new_test_ext().execute_with(|| {
        assert_eq!(Relayer::quota_remaining(&OTHER), 0);
        assert_eq!(
            <Relayer as RelayedActor>::quota_remaining(&OTHER),
            0,
            "the trait read agrees with the module read"
        );
        assert!(Relayer::quota_state(&OTHER).is_none());
        assert_eq!(Relayer::sponsor_remaining(&OTHER, &RELAYER), 0);
        assert_eq!(Relayer::usage_report(&OTHER), (0, 0, 0));
        assert!(Relayer::submission(&OTHER, 0).is_none());
        assert_eq!(Relayer::actor_of(&OTHER, 0), None);
        // A deployment with no quota admits nothing.
        assert!(!Relayer::admits(&OTHER, &CITIZEN, "register_parcel"));
    });
}

#[test]
fn the_trait_quota_read_follows_the_spent_units() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));
        assert_eq!(<Relayer as RelayedActor>::quota_remaining(&DEPLOYMENT), 90);
    });
}

#[test]
fn an_authorisation_on_its_last_moment_still_runs() {
    new_test_ext().execute_with(|| {
        install();
        let mut held = authorisation(1);
        held.valid_until = clock() + 60_000;
        set_clock(held.valid_until);
        assert_ok!(relay(&held));
    });
}

#[test]
fn an_authorisation_on_the_largest_span_still_runs() {
    new_test_ext().execute_with(|| {
        install();
        let mut held = authorisation(1);
        held.valid_until = clock() + HOUR;
        assert_ok!(relay(&held));

        let mut past = authorisation(2);
        past.valid_until = clock() + HOUR + 1;
        assert_noop!(relay(&past), Error::<Test>::AuthorisationTooLong);
    });
}

#[test]
fn one_number_runs_once_per_account_and_per_deployment() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(relay(&authorisation(1)));

        // A second account uses the same number freely.
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
            signature.0,
            Box::new(inner_call()),
        ));
        assert_eq!(Relayer::actor_of(&DEPLOYMENT, 1), Some(5));
        assert_eq!(Relayer::usage_report(&DEPLOYMENT), (20, 80, 2));
    });
}

#[test]
fn a_signing_key_serves_one_deployment_alone() {
    new_test_ext().execute_with(|| {
        install();
        assert!(SigningKeys::<Test>::get(DEPLOYMENT, CITIZEN).is_some());
        assert!(SigningKeys::<Test>::get(OTHER, CITIZEN).is_none());

        // A registration replaces the key the account signs with.
        let fresh = ed25519::Pair::from_seed(&[12u8; 32]);
        assert_ok!(Relayer::register_signing_key(
            RuntimeOrigin::signed(CITIZEN),
            DEPLOYMENT,
            fresh.public().0
        ));
        let held = authorisation(1);
        let signature = fresh.sign(&held.message());
        assert_ok!(Relayer::submit_on_behalf(
            RuntimeOrigin::signed(RELAYER),
            DEPLOYMENT,
            held,
            signature.0,
            Box::new(inner_call()),
        ));
        assert_noop!(relay(&authorisation(2)), Error::<Test>::SignatureInvalid);
    });
}

#[test]
fn an_authorisation_message_changes_with_every_field() {
    let held = authorisation_at(1, 5);
    let mut by_nonce = held.clone();
    by_nonce.nonce = 2;
    let mut by_moment = held.clone();
    by_moment.valid_until = 6;
    let mut by_actor = held.clone();
    by_actor.actor = 44;
    assert_ne!(held.message(), by_nonce.message());
    assert_ne!(held.message(), by_moment.message());
    assert_ne!(held.message(), by_actor.message());
    // The domain separator leads the message.
    assert!(held
        .message()
        .windows(crate::submit::AUTHORISATION_DOMAIN.len())
        .any(|part| part == crate::submit::AUTHORISATION_DOMAIN));
}

fn authorisation_at(nonce: u64, valid_until: u64) -> Authorisation<u64> {
    Authorisation {
        deployment: DEPLOYMENT,
        actor: CITIZEN,
        action: name_of("register_parcel").expect("the name fits"),
        payload_hash: inner_hash(),
        valid_until,
        nonce,
    }
}

#[test]
fn every_error_names_its_fix() {
    let cases: [(Error<Test>, &str); 18] = [
        (Error::NameTooLong, "Shorten the action name."),
        (
            Error::SponsorUnknown,
            "Register the sponsor before relaying a submission.",
        ),
        (
            Error::SponsorInactive,
            "Activate the sponsor before relaying a submission.",
        ),
        (Error::SponsorExhausted, "Raise the budget of the sponsor."),
        (
            Error::QuotaExhausted,
            "Wait for the next period, or raise the quota of the tenant.",
        ),
        (
            Error::QuotaMissing,
            "Set the quota of the tenant before relaying a submission.",
        ),
        (Error::AccountRateLimited, "Wait for the next window."),
        (Error::ActionRateLimited, "Wait for the next window."),
        (Error::AccountThrottled, "Wait for the throttle to lift."),
        (
            Error::SigningKeyMissing,
            "Register the key the account signs authorisations with.",
        ),
        (
            Error::SignatureInvalid,
            "Sign the authorisation with the registered key.",
        ),
        (
            Error::DeploymentMismatch,
            "Submit the authorisation to the deployment it names.",
        ),
        (
            Error::AuthorisationExpired,
            "Ask the user to authorise the action again.",
        ),
        (
            Error::AuthorisationTooLong,
            "Shorten the span the authorisation names.",
        ),
        (
            Error::NonceUsed,
            "Ask the user to authorise the action with a fresh number.",
        ),
        (
            Error::ActorUnregistered,
            "Register the account in the identity module first.",
        ),
        (
            Error::FeatureUnlicensed,
            "Add the feature to the subscription and install the new licence.",
        ),
        (Error::RegisterFull, "Start a new deployment."),
    ];
    for (error, fix) in cases {
        let text = error_text(&error);
        assert!(
            text.ends_with(fix),
            "the error text {text:?} ends with the fix sentence {fix:?}"
        );
    }
}

/// The documentation text of one error, read from the runtime metadata.
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
