//! The tests of the oracle module.
//!
//! One feed carries the market price of a land parcel. Three accounts
//! attest and two attestations settle a value. The dispute window runs for
//! one hour.

use crate::{mock::*, Current, Error, FeedReader, PendingRound, RoundStatus, Rounds};
use cerulea_blueprint::types::Value;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

extern crate alloc;
use alloc::{vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const READER: u64 = 2;
const ATTESTORS: [u64; 3] = [11, 12, 13];
const HOUR: u64 = 3_600_000;

fn feed_name() -> BoundedName {
    name_of("parcel_price").expect("the name fits")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn price(units: i128) -> BoundedVec<u8, frame_support::traits::ConstU32<1_024>> {
    bounded(Value::Integer(units).canonical_bytes())
}

fn register(required: u32, window: u64) {
    let attestors: BoundedVec<u64, frame_support::traits::ConstU32<8>> =
        BoundedVec::try_from(ATTESTORS.to_vec()).expect("the list fits");
    assert_ok!(Oracle::register_feed(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        feed_name(),
        attestors,
        required,
        window,
        bounded(b"The market price of one land parcel in rupees.".to_vec())
    ));
}

fn submit(who: u64, units: i128) {
    assert_ok!(Oracle::submit_value(
        RuntimeOrigin::signed(who),
        DEPLOYMENT,
        feed_name(),
        price(units)
    ));
}

#[test]
fn a_feed_registers_and_reads_back() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        let held = crate::Feeds::<Test>::get(DEPLOYMENT, feed_name())
            .expect("the registry holds the feed");
        assert_eq!(held.required, 2);
        assert_eq!(held.dispute_window_millis, HOUR);
        assert_eq!(held.steward, STEWARD);
        assert!(held.attests(&ATTESTORS[0]));
        assert!(!held.attests(&READER));
    });
}

#[test]
fn a_requirement_of_zero_declines() {
    new_test_ext().execute_with(|| {
        let attestors: BoundedVec<u64, frame_support::traits::ConstU32<8>> =
            BoundedVec::try_from(ATTESTORS.to_vec()).expect("the list fits");
        assert_noop!(
            Oracle::register_feed(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                feed_name(),
                attestors,
                0,
                HOUR,
                bounded(b"The market price of one land parcel.".to_vec())
            ),
            Error::<Test>::RequirementTooLow
        );
    });
}

#[test]
fn a_requirement_above_the_attestor_count_declines() {
    new_test_ext().execute_with(|| {
        let attestors: BoundedVec<u64, frame_support::traits::ConstU32<8>> =
            BoundedVec::try_from(ATTESTORS.to_vec()).expect("the list fits");
        assert_noop!(
            Oracle::register_feed(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                feed_name(),
                attestors,
                4,
                HOUR,
                bounded(b"The market price of one land parcel.".to_vec())
            ),
            Error::<Test>::RequirementTooHigh
        );
    });
}

#[test]
fn a_feed_registered_twice_declines() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        let attestors: BoundedVec<u64, frame_support::traits::ConstU32<8>> =
            BoundedVec::try_from(ATTESTORS.to_vec()).expect("the list fits");
        assert_noop!(
            Oracle::register_feed(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                feed_name(),
                attestors,
                2,
                HOUR,
                bounded(b"The market price of one land parcel.".to_vec())
            ),
            Error::<Test>::FeedPresent
        );
    });
}

#[test]
fn an_account_outside_the_attestor_list_submits_nothing() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(READER),
                DEPLOYMENT,
                feed_name(),
                price(1_200)
            ),
            Error::<Test>::NotAnAttestor
        );
    });
}

#[test]
fn one_attestor_signs_one_round_once() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(ATTESTORS[0]),
                DEPLOYMENT,
                feed_name(),
                price(1_200)
            ),
            Error::<Test>::AlreadyAttested
        );
    });
}

#[test]
fn a_round_reaching_its_requirement_opens_its_dispute_window() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 0)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Collecting
        );

        submit(ATTESTORS[1], 1_200);
        let held = Oracle::round(&DEPLOYMENT, &feed_name(), 0).expect("the feed holds the round");
        assert_eq!(held.status, RoundStatus::Attested);
        assert_eq!(held.signers.len(), 2);
        assert_eq!(held.dispute_deadline, Some(START + HOUR));
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed_name()), Some(0));
    });
}

#[test]
fn a_value_becomes_current_once_its_window_closes() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert!(Oracle::current(&DEPLOYMENT, "parcel_price").is_none());

        advance_clock(HOUR);
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_200))
        );
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name()
        ));
        assert_eq!(
            Current::<Test>::get(DEPLOYMENT, feed_name())
                .expect("the feed holds a current value")
                .round,
            0
        );
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 0)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Current
        );
    });
}

#[test]
fn a_feed_with_no_window_settles_at_once() {
    new_test_ext().execute_with(|| {
        register(2, 0);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_200))
        );
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 0)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Current
        );
    });
}

#[test]
fn two_values_open_two_rounds() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_300);
        assert!(Rounds::<Test>::contains_key((DEPLOYMENT, feed_name(), 0)));
        assert!(Rounds::<Test>::contains_key((DEPLOYMENT, feed_name(), 1)));
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 1)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Collecting
        );

        // The third attestor agrees with the second value, so that round
        // reaches the requirement.
        submit(ATTESTORS[2], 1_300);
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 1)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Attested
        );
    });
}

#[test]
fn a_dispute_holds_a_value_out_of_the_feed() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_ok!(Oracle::dispute(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name(),
            0,
            bounded(b"The survey office reports a different price.".to_vec())
        ));
        advance_clock(HOUR * 2);
        assert!(Oracle::current(&DEPLOYMENT, "parcel_price").is_none());
    });
}

#[test]
fn an_upheld_dispute_keeps_the_value_out_for_good() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_ok!(Oracle::dispute(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name(),
            0,
            bounded(b"The survey office reports a different price.".to_vec())
        ));
        assert_ok!(Oracle::resolve(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed_name(),
            0,
            true
        ));
        assert_eq!(
            Oracle::round(&DEPLOYMENT, &feed_name(), 0)
                .expect("the feed holds the round")
                .status,
            RoundStatus::Rejected
        );
        advance_clock(HOUR * 2);
        assert!(Oracle::current(&DEPLOYMENT, "parcel_price").is_none());
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed_name()), None);
    });
}

#[test]
fn a_dispute_set_aside_lets_the_value_settle() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_ok!(Oracle::dispute(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name(),
            0,
            bounded(b"The survey office reports a different price.".to_vec())
        ));
        assert_ok!(Oracle::resolve(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed_name(),
            0,
            false
        ));
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_200))
        );
    });
}

#[test]
fn a_dispute_past_the_window_declines() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        advance_clock(HOUR + 1);
        assert_noop!(
            Oracle::dispute(
                RuntimeOrigin::signed(READER),
                DEPLOYMENT,
                feed_name(),
                0,
                bounded(b"The survey office reports a different price.".to_vec())
            ),
            Error::<Test>::RoundNotDisputable
        );
    });
}

#[test]
fn only_the_steward_resolves_a_dispute() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_ok!(Oracle::dispute(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name(),
            0,
            bounded(b"The survey office reports a different price.".to_vec())
        ));
        assert_noop!(
            Oracle::resolve(
                RuntimeOrigin::signed(READER),
                DEPLOYMENT,
                feed_name(),
                0,
                false
            ),
            Error::<Test>::NotTheSteward
        );
    });
}

#[test]
fn a_round_with_no_dispute_resolves_nothing() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_noop!(
            Oracle::resolve(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                feed_name(),
                0,
                true
            ),
            Error::<Test>::NoOpenDispute
        );
    });
}

#[test]
fn a_later_value_replaces_the_current_value() {
    new_test_ext().execute_with(|| {
        register(2, 0);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_200))
        );

        submit(ATTESTORS[0], 1_450);
        submit(ATTESTORS[2], 1_450);
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_450))
        );
        let (value, attested_at) =
            Oracle::current_at(&DEPLOYMENT, "parcel_price").expect("the feed carries a value");
        assert_eq!(value, Value::Integer(1_450));
        assert_eq!(attested_at, START);
    });
}

#[test]
fn a_retired_feed_takes_no_further_value() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        assert_ok!(Oracle::retire_feed(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed_name()
        ));
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(ATTESTORS[0]),
                DEPLOYMENT,
                feed_name(),
                price(1_200)
            ),
            Error::<Test>::FeedRetired
        );
    });
}

#[test]
fn an_unknown_feed_declines_every_call() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(ATTESTORS[0]),
                DEPLOYMENT,
                feed_name(),
                price(1_200)
            ),
            Error::<Test>::FeedUnknown
        );
        assert!(!Oracle::feed_exists(&DEPLOYMENT, "parcel_price"));
    });
}

#[test]
fn a_value_that_fails_to_decode_declines() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(ATTESTORS[0]),
                DEPLOYMENT,
                feed_name(),
                bounded(vec![255u8, 255, 255])
            ),
            Error::<Test>::ValueUnreadable
        );
    });
}

#[test]
fn every_oracle_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        submit(ATTESTORS[0], 1_200);
        submit(ATTESTORS[1], 1_200);
        assert_ok!(Oracle::dispute(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed_name(),
            0,
            bounded(b"The survey office reports a different price.".to_vec())
        ));
        assert_ok!(Oracle::resolve(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed_name(),
            0,
            false
        ));

        let registered = Audit::query_by_event(&DEPLOYMENT, "feed_registered", 0, 64);
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0].actor, STEWARD);
        assert_eq!(registered[0].module, "oracle");
        assert_eq!(registered[0].class, WorkClass::Structural);

        let submitted = Audit::query_by_event(&DEPLOYMENT, "value_submitted", 0, 64);
        assert_eq!(submitted.len(), 2);
        assert_eq!(submitted[0].actor, ATTESTORS[0]);
        assert_eq!(submitted[1].actor, ATTESTORS[1]);

        let raised = Audit::query_by_event(&DEPLOYMENT, "dispute_raised", 0, 64);
        assert_eq!(raised.len(), 1);
        assert_eq!(raised[0].actor, READER);

        let resolved = Audit::query_by_event(&DEPLOYMENT, "dispute_resolved", 0, 64);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].actor, STEWARD);
    });
}

#[test]
fn the_value_hash_separates_two_values() {
    let first = crate::value_hash(&Value::Integer(1_200).canonical_bytes());
    let second = crate::value_hash(&Value::Integer(1_300).canonical_bytes());
    assert!(first != second);
    assert_eq!(first, crate::value_hash(&Value::Integer(1_200).encode()));
}
