//! The round and dispute tests of the oracle module.
//!
//! One feed carries the market price of a land parcel. Three accounts
//! attest and two attestations settle a value. These tests walk every
//! refusal a feed raises, every stage a round passes through and every
//! record the audit log keeps.

use crate::{
    mock::*, value_hash, Current, CurrentValue, Error, Event, FeedNames, FeedReader, Feeds,
    NextRound, OpenRound, PendingRound, Round, RoundStatus, Rounds,
};
use cerulea_blueprint::types::Value;
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};
use sp_runtime::DispatchError;

extern crate alloc;
use alloc::{format, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [5u8; 32];
const STEWARD: u64 = 1;
const READER: u64 = 2;
const ATTESTORS: [u64; 3] = [11, 12, 13];
const HOUR: u64 = 3_600_000;
const SUMMARY: &[u8] = b"The market price of one land parcel in rupees.";
const REASON: &[u8] = b"The survey office reports a different price.";

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
}

fn feed() -> BoundedName {
    name("parcel_price")
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn attestors() -> BoundedVec<u64, ConstU32<8>> {
    BoundedVec::try_from(ATTESTORS.to_vec()).expect("the list fits")
}

fn price_bytes(units: i128) -> Vec<u8> {
    Value::Integer(units).canonical_bytes()
}

fn price(units: i128) -> BoundedVec<u8, ConstU32<1_024>> {
    bounded(price_bytes(units))
}

fn register_named(
    feed: BoundedName,
    required: u32,
    window: u64,
) -> frame_support::dispatch::DispatchResult {
    Oracle::register_feed(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        feed,
        attestors(),
        required,
        window,
        bounded(SUMMARY.to_vec()),
    )
}

fn register(required: u32, window: u64) {
    assert_ok!(register_named(feed(), required, window));
}

fn submit(who: u64, units: i128) {
    assert_ok!(Oracle::submit_value(
        RuntimeOrigin::signed(who),
        DEPLOYMENT,
        feed(),
        price(units)
    ));
}

fn raise(round: u64) -> frame_support::dispatch::DispatchResult {
    Oracle::dispute(
        RuntimeOrigin::signed(READER),
        DEPLOYMENT,
        feed(),
        round,
        bounded(REASON.to_vec()),
    )
}

fn status(round: u64) -> RoundStatus {
    Oracle::round(&DEPLOYMENT, &feed(), round)
        .expect("the feed holds the round")
        .status
}

fn attest_both(units: i128) {
    submit(ATTESTORS[0], units);
    submit(ATTESTORS[1], units);
}

#[test]
fn an_unsigned_origin_reaches_no_call() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Oracle::register_feed(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                feed(),
                attestors(),
                2,
                HOUR,
                bounded(SUMMARY.to_vec())
            ),
            DispatchError::BadOrigin
        );
        register(2, HOUR);
        assert_noop!(
            Oracle::settle(RuntimeOrigin::none(), DEPLOYMENT, feed()),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn a_registered_feed_lists_its_name_summary_and_audit_record() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        let held = Feeds::<Test>::get(DEPLOYMENT, feed()).expect("the registry holds the feed");
        assert!(held.active);
        assert_eq!(held.summary.to_vec(), SUMMARY.to_vec());
        assert_eq!(held.attestors.to_vec(), ATTESTORS.to_vec());
        assert_eq!(
            FeedNames::<Test>::get(DEPLOYMENT).into_inner(),
            vec![feed()]
        );
        assert!(Oracle::feed_exists(&DEPLOYMENT, "parcel_price"));
        System::assert_last_event(RuntimeEvent::Oracle(Event::FeedRegistered {
            deployment: DEPLOYMENT,
            feed: feed(),
            required: 2,
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "feed_registered", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].reason,
            "The steward registered one external data feed."
        );
        assert_eq!(entries[0].payload, (feed(), 2u32).encode());
    });
}

#[test]
fn a_window_past_the_largest_dispute_window_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(register_named(name("weekly_price"), 2, 604_800_000));
        assert_noop!(
            register_named(feed(), 2, 604_800_001),
            Error::<Test>::WindowTooLong
        );
    });
}

#[test]
fn a_deployment_holding_its_full_count_of_feeds_declines_one_more() {
    new_test_ext().execute_with(|| {
        for index in 0..32u32 {
            assert_ok!(register_named(name(&format!("price_{index}")), 2, HOUR));
        }
        assert_eq!(FeedNames::<Test>::get(DEPLOYMENT).len(), 32);
        assert_noop!(
            register_named(name("price_32"), 2, HOUR),
            Error::<Test>::FeedLimitReached
        );
    });
}

#[test]
fn retiring_a_feed_marks_it_inactive_and_records_the_steward() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        assert_ok!(Oracle::retire_feed(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed()
        ));
        let held = Feeds::<Test>::get(DEPLOYMENT, feed()).expect("the registry keeps the feed");
        assert!(!held.active);
        assert!(Oracle::feed_exists(&DEPLOYMENT, "parcel_price"));
        System::assert_last_event(RuntimeEvent::Oracle(Event::FeedRetired {
            deployment: DEPLOYMENT,
            feed: feed(),
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "feed_retired", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].actor, STEWARD);
        assert_eq!(entries[0].class, WorkClass::Structural);
        assert_eq!(
            entries[0].reason,
            "The steward retired one external data feed."
        );
        assert_eq!(entries[0].payload, b"parcel_price".to_vec());
    });
}

#[test]
fn retiring_declines_for_an_unknown_feed_a_stranger_and_a_second_attempt() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Oracle::retire_feed(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, feed()),
            Error::<Test>::FeedUnknown
        );
        register(2, HOUR);
        assert_noop!(
            Oracle::retire_feed(RuntimeOrigin::signed(READER), DEPLOYMENT, feed()),
            Error::<Test>::NotTheSteward
        );
        assert_ok!(Oracle::retire_feed(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed()
        ));
        assert_noop!(
            Oracle::retire_feed(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, feed()),
            Error::<Test>::FeedRetired
        );
    });
}

#[test]
fn a_first_attestation_opens_a_round_stamped_with_its_value() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        System::set_block_number(3);
        submit(ATTESTORS[0], 1_200);

        let held = Oracle::round(&DEPLOYMENT, &feed(), 0).expect("the feed holds the round");
        assert_eq!(held.value.to_vec(), price_bytes(1_200));
        assert_eq!(held.value_hash, value_hash(&price_bytes(1_200)));
        assert_eq!(held.signers.to_vec(), vec![ATTESTORS[0]]);
        assert_eq!(held.opened_at, START);
        assert_eq!(held.opened_block, 3);
        assert_eq!(held.attested_at, None);
        assert_eq!(held.dispute_deadline, None);
        assert_eq!(OpenRound::<Test>::get(DEPLOYMENT, feed()), Some(0));
        assert_eq!(NextRound::<Test>::get(DEPLOYMENT, feed()), 1);
        System::assert_last_event(RuntimeEvent::Oracle(Event::ValueSubmitted {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
            signer: ATTESTORS[0],
            signatures: 1,
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "value_submitted", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].class, WorkClass::Routine);
        assert_eq!(
            entries[0].reason,
            "An attestor submitted one external value."
        );
        assert_eq!(
            entries[0].payload,
            (feed(), 0u64, value_hash(&price_bytes(1_200))).encode()
        );
    });
}

#[test]
fn a_round_reaching_its_requirement_announces_its_deadline() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        System::assert_last_event(RuntimeEvent::Oracle(Event::RoundAttested {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
            dispute_deadline: START + HOUR,
        }));
        let held = Oracle::round(&DEPLOYMENT, &feed(), 0).expect("the feed holds the round");
        assert_eq!(held.attested_at, Some(START));
        assert_eq!(OpenRound::<Test>::get(DEPLOYMENT, feed()), None);
    });
}

#[test]
fn a_new_submission_settles_the_round_whose_window_closed() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        advance_clock(HOUR);
        assert!(Current::<Test>::get(DEPLOYMENT, feed()).is_none());

        submit(ATTESTORS[2], 1_300);
        let held = Current::<Test>::get(DEPLOYMENT, feed()).expect("the feed carries a value");
        assert_eq!(held.round, 0);
        assert_eq!(held.value.to_vec(), price_bytes(1_200));
        assert_eq!(held.attested_at, START);
        assert_eq!(status(0), RoundStatus::Current);
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed()), None);
        System::assert_has_event(RuntimeEvent::Oracle(Event::ValueSettled {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
        }));
        assert_eq!(status(1), RoundStatus::Collecting);
    });
}

#[test]
fn a_reader_sees_the_settled_value_while_a_newer_round_serves_its_window() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        advance_clock(HOUR);
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed()
        ));

        advance_clock(10);
        attest_both(1_450);
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed()), Some(1));
        assert_eq!(
            Oracle::current_at(&DEPLOYMENT, "parcel_price"),
            Some((Value::Integer(1_200), START))
        );

        advance_clock(HOUR);
        assert_eq!(
            Oracle::current_at(&DEPLOYMENT, "parcel_price"),
            Some((Value::Integer(1_450), START + HOUR + 10))
        );
        // The read promotes nothing. The stored current value stays with
        // the first round until a call settles the second.
        assert_eq!(
            Current::<Test>::get(DEPLOYMENT, feed())
                .expect("the feed carries a value")
                .round,
            0
        );
    });
}

#[test]
fn settling_with_nothing_due_changes_nothing() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed()
        ));

        attest_both(1_200);
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed()
        ));
        assert_eq!(status(0), RoundStatus::Attested);
        assert!(Current::<Test>::get(DEPLOYMENT, feed()).is_none());

        assert_ok!(raise(0));
        advance_clock(HOUR * 2);
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed()
        ));
        assert_eq!(status(0), RoundStatus::Disputed);
        assert!(Current::<Test>::get(DEPLOYMENT, feed()).is_none());
    });
}

#[test]
fn a_pending_marker_naming_an_absent_round_settles_nothing() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        PendingRound::<Test>::insert(DEPLOYMENT, feed(), 9);
        assert_ok!(Oracle::settle(
            RuntimeOrigin::signed(READER),
            DEPLOYMENT,
            feed()
        ));
        assert!(Current::<Test>::get(DEPLOYMENT, feed()).is_none());
        assert!(Oracle::current(&DEPLOYMENT, "parcel_price").is_none());
    });
}

#[test]
fn an_open_marker_naming_an_absent_round_opens_a_fresh_round() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        OpenRound::<Test>::insert(DEPLOYMENT, feed(), 7);
        submit(ATTESTORS[0], 1_200);
        assert_eq!(OpenRound::<Test>::get(DEPLOYMENT, feed()), Some(0));
        assert_eq!(status(0), RoundStatus::Collecting);
    });
}

#[test]
fn a_feed_at_its_last_round_identifier_declines_a_new_round() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        NextRound::<Test>::insert(DEPLOYMENT, feed(), u64::MAX);
        assert_noop!(
            Oracle::submit_value(
                RuntimeOrigin::signed(ATTESTORS[0]),
                DEPLOYMENT,
                feed(),
                price(1_200)
            ),
            Error::<Test>::RegisterFull
        );
    });
}

#[test]
fn a_dispute_records_its_raiser_and_reason() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        assert_ok!(raise(0));

        let held = Oracle::round(&DEPLOYMENT, &feed(), 0).expect("the feed holds the round");
        assert_eq!(held.status, RoundStatus::Disputed);
        assert_eq!(held.disputed_by, Some(READER));
        assert_eq!(
            held.dispute_reason.map(|reason| reason.to_vec()),
            Some(REASON.to_vec())
        );
        System::assert_last_event(RuntimeEvent::Oracle(Event::DisputeRaised {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
            raiser: READER,
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "dispute_raised", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].reason,
            "A reader raised one dispute against an attested value."
        );
        assert_eq!(entries[0].payload, (feed(), 0u64, REASON.to_vec()).encode());
    });
}

#[test]
fn a_dispute_lands_on_the_last_moment_of_the_window() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        advance_clock(HOUR);
        assert_ok!(raise(0));
        assert_eq!(status(0), RoundStatus::Disputed);
    });
}

#[test]
fn a_dispute_declines_outside_an_attested_round() {
    new_test_ext().execute_with(|| {
        register(2, 0);
        assert_noop!(raise(0), Error::<Test>::RoundUnknown);

        submit(ATTESTORS[0], 1_200);
        assert_eq!(status(0), RoundStatus::Collecting);
        assert_noop!(raise(0), Error::<Test>::RoundNotDisputable);

        submit(ATTESTORS[1], 1_200);
        assert_eq!(status(0), RoundStatus::Current);
        assert_noop!(raise(0), Error::<Test>::RoundNotDisputable);
    });
}

#[test]
fn a_round_takes_one_open_dispute_at_a_time() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        assert_ok!(raise(0));
        assert_noop!(raise(0), Error::<Test>::RoundNotDisputable);
    });
}

#[test]
fn resolving_declines_for_an_unknown_feed_and_an_unknown_round() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Oracle::resolve(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, feed(), 0, true),
            Error::<Test>::FeedUnknown
        );
        register(2, HOUR);
        assert_noop!(
            Oracle::resolve(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, feed(), 4, true),
            Error::<Test>::RoundUnknown
        );
    });
}

#[test]
fn a_dispute_set_aside_records_the_steward_and_settles_the_value() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        assert_ok!(raise(0));
        advance_clock(60_000);
        assert_ok!(Oracle::resolve(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed(),
            0,
            false
        ));

        let held = Oracle::round(&DEPLOYMENT, &feed(), 0).expect("the feed holds the round");
        assert_eq!(held.status, RoundStatus::Current);
        assert_eq!(held.dispute_deadline, Some(START + 60_000));
        System::assert_has_event(RuntimeEvent::Oracle(Event::DisputeResolved {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
            upheld: false,
        }));
        System::assert_last_event(RuntimeEvent::Oracle(Event::ValueSettled {
            deployment: DEPLOYMENT,
            feed: feed(),
            round: 0,
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "dispute_resolved", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].reason,
            "The steward of the feed resolved one dispute."
        );
        assert_eq!(entries[0].payload, (feed(), 0u64, false).encode());
    });
}

#[test]
fn an_upheld_dispute_over_a_superseded_round_keeps_the_newer_pending_round() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        attest_both(1_200);
        assert_ok!(raise(0));

        submit(ATTESTORS[0], 1_300);
        submit(ATTESTORS[2], 1_300);
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed()), Some(1));

        assert_ok!(Oracle::resolve(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            feed(),
            0,
            true
        ));
        assert_eq!(status(0), RoundStatus::Rejected);
        assert_eq!(PendingRound::<Test>::get(DEPLOYMENT, feed()), Some(1));

        advance_clock(HOUR);
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_300))
        );
    });
}

#[test]
fn an_undecodable_stored_value_reads_as_absent() {
    new_test_ext().execute_with(|| {
        register(2, HOUR);
        let garbled: BoundedVec<u8, ConstU32<1_024>> = bounded(vec![255, 255, 255]);
        Current::<Test>::insert(
            DEPLOYMENT,
            feed(),
            CurrentValue {
                round: 0,
                value: garbled.clone(),
                attested_at: START,
            },
        );
        assert!(Oracle::current(&DEPLOYMENT, "parcel_price").is_none());

        Rounds::<Test>::insert(
            (DEPLOYMENT, feed(), 1u64),
            Round {
                value: garbled,
                value_hash: value_hash(&[255, 255, 255]),
                signers: attestors(),
                opened_at: START,
                attested_at: Some(START),
                dispute_deadline: Some(START),
                status: RoundStatus::Attested,
                disputed_by: None,
                dispute_reason: None,
                opened_block: 1u64,
            },
        );
        PendingRound::<Test>::insert(DEPLOYMENT, feed(), 1);
        assert!(Oracle::current_at(&DEPLOYMENT, "parcel_price").is_none());
    });
}

#[test]
fn a_feed_name_past_128_bytes_reads_as_absent() {
    new_test_ext().execute_with(|| {
        register(2, 0);
        attest_both(1_200);
        let long = "p".repeat(129);
        assert!(!Oracle::feed_exists(&DEPLOYMENT, &long));
        assert!(Oracle::current(&DEPLOYMENT, &long).is_none());
        assert!(Oracle::current_at(&DEPLOYMENT, &long).is_none());
        assert_eq!(
            Oracle::current(&DEPLOYMENT, "parcel_price"),
            Some(Value::Integer(1_200))
        );
    });
}

#[test]
fn a_single_attestor_feed_settles_on_one_submission() {
    new_test_ext().execute_with(|| {
        register(1, 0);
        submit(ATTESTORS[2], 980);
        assert_eq!(status(0), RoundStatus::Current);
        assert_eq!(
            Current::<Test>::get(DEPLOYMENT, feed())
                .expect("the feed carries a value")
                .value
                .to_vec(),
            price_bytes(980)
        );
    });
}
