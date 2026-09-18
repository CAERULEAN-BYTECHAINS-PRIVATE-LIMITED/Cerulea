//! On-chain tests for the Assurance Engine.
//!
//! The engine logic lives in `crate::engines::assurance`. These tests drive the
//! engine through pallet storage, pallet calls and the block initialisation
//! hook, so the behaviour is the behaviour a running chain shows.

use crate::engines::assurance::{ClassTable, TransactionClass};
use crate::mock::*;
use crate::{AssuranceClassTable, Error, Event, ScheduledChanges};
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, assert_ok, BoundedVec};

/// Advance the chain to a block, running the initialisation hook each step.
fn run_to_block(target: u64) {
    let mut now = frame_system::Pallet::<Test>::block_number();
    while now < target {
        now += 1;
        frame_system::Pallet::<Test>::set_block_number(now);
        DcfPallet::on_initialize(now);
    }
}

fn payload(byte: u8) -> BoundedVec<u8, frame_support::traits::ConstU32<512>> {
    BoundedVec::try_from(sp_std::vec![byte; 8]).expect("eight bytes fit")
}

#[test]
fn the_chain_starts_from_the_published_class_table() {
    new_test_ext().execute_with(|| {
        let table = AssuranceClassTable::<Test>::get();
        assert_eq!(table, ClassTable::starting_profile());
        assert_eq!(table.threshold_percent(TransactionClass::Routine), 51);
        assert_eq!(table.threshold_percent(TransactionClass::Financial), 67);
        assert_eq!(table.threshold_percent(TransactionClass::Structural), 80);
        assert_eq!(
            table.threshold_percent(TransactionClass::Constitutional),
            90
        );
        assert_eq!(table.delay_blocks(TransactionClass::Constitutional), 100);
    });
}

#[test]
fn a_domain_commits_at_the_share_of_the_highest_class_present() {
    new_test_ext().execute_with(|| {
        let total = 1_000u128;
        let mixed = sp_std::vec![
            TransactionClass::Routine,
            TransactionClass::Financial,
            TransactionClass::Structural,
        ];
        let resolved = DcfPallet::domain_assurance(mixed, total).expect("resolves");
        assert_eq!(resolved.highest_class, TransactionClass::Structural);
        assert_eq!(resolved.threshold_percent, 80);
        assert_eq!(resolved.required_weight, 800);
    });
}

/// Design assertion 5.
///
/// A routine transaction finalises before a constitutional transaction in the
/// same block. Both domains see the same signers in the same order.
#[test]
fn a_routine_domain_finalises_before_a_constitutional_domain() {
    new_test_ext().execute_with(|| {
        let total = 100u128;
        let mut routine_step = None;
        let mut constitutional_step = None;

        for step in 1..=100u128 {
            if routine_step.is_none()
                && DcfPallet::domain_reaches_threshold(
                    sp_std::vec![TransactionClass::Routine],
                    total,
                    step,
                )
            {
                routine_step = Some(step);
            }
            if constitutional_step.is_none()
                && DcfPallet::domain_reaches_threshold(
                    sp_std::vec![TransactionClass::Constitutional],
                    total,
                    step,
                )
            {
                constitutional_step = Some(step);
            }
        }

        assert_eq!(routine_step, Some(51));
        assert_eq!(constitutional_step, Some(90));
        assert!(routine_step < constitutional_step);
    });
}

#[test]
fn the_class_table_changes_under_root_authority() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        assert_ok!(DcfPallet::set_assurance_class(
            RuntimeOrigin::root(),
            TransactionClass::Financial,
            75,
            0
        ));
        assert_eq!(
            AssuranceClassTable::<Test>::get().threshold_percent(TransactionClass::Financial),
            75
        );
        frame_system::Pallet::<Test>::assert_has_event(
            Event::<Test>::AssuranceClassTableAmended {
                class: TransactionClass::Financial,
                previous_percent: 67,
                current_percent: 75,
                delay_blocks: 0,
            }
            .into(),
        );
    });
}

#[test]
fn a_signed_account_leaves_the_class_table_unchanged() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::set_assurance_class(
            RuntimeOrigin::signed(1),
            TransactionClass::Financial,
            75,
            0
        )
        .is_err());
        assert_eq!(
            AssuranceClassTable::<Test>::get().threshold_percent(TransactionClass::Financial),
            67
        );
    });
}

#[test]
fn a_table_that_fails_inspection_leaves_storage_unchanged() {
    new_test_ext().execute_with(|| {
        // A share of 50 percent lets two disjoint groups each reach the share.
        assert_noop!(
            DcfPallet::set_assurance_class(RuntimeOrigin::root(), TransactionClass::Routine, 50, 0),
            Error::<Test>::AssuranceTableInvalid
        );
        assert_eq!(
            AssuranceClassTable::<Test>::get().threshold_percent(TransactionClass::Routine),
            51
        );

        // A constitutional class carrying no window is refused as well.
        assert_noop!(
            DcfPallet::set_assurance_class(
                RuntimeOrigin::root(),
                TransactionClass::Constitutional,
                90,
                0
            ),
            Error::<Test>::AssuranceTableInvalid
        );
        assert_eq!(
            AssuranceClassTable::<Test>::get().delay_blocks(TransactionClass::Constitutional),
            100
        );
    });
}

#[test]
fn a_routine_change_takes_effect_at_the_block_of_commitment() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(5);
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Routine,
            payload(1)
        ));
        let change = ScheduledChanges::<Test>::get(0).expect("scheduled");
        assert_eq!(change.window.committed_at, 5);
        assert_eq!(change.window.effective_at, 5);
        assert_ok!(DcfPallet::release_scheduled_change(
            RuntimeOrigin::signed(1),
            0
        ));
        assert!(ScheduledChanges::<Test>::get(0).is_none());
    });
}

/// Design assertion 6.
///
/// A constitutional change waits the full delay window. Every release inside
/// the window fails. The chain releases the change on its own at the effective
/// block.
#[test]
fn a_constitutional_change_waits_the_full_delay_window() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload(9)
        ));

        let change = ScheduledChanges::<Test>::get(0).expect("scheduled");
        assert_eq!(change.class, TransactionClass::Constitutional);
        assert_eq!(change.window.committed_at, 1);
        assert_eq!(change.window.effective_at, 101);

        // A release inside the window fails and the change stays scheduled.
        assert_noop!(
            DcfPallet::release_scheduled_change(RuntimeOrigin::signed(1), 0),
            Error::<Test>::DelayWindowOpen
        );

        run_to_block(100);
        assert!(
            ScheduledChanges::<Test>::get(0).is_some(),
            "the change holds at the last block of the window"
        );
        assert_noop!(
            DcfPallet::release_scheduled_change(RuntimeOrigin::signed(1), 0),
            Error::<Test>::DelayWindowOpen
        );

        run_to_block(101);
        assert!(
            ScheduledChanges::<Test>::get(0).is_none(),
            "the change takes effect at the effective block"
        );
        frame_system::Pallet::<Test>::assert_has_event(
            Event::<Test>::ScheduledChangeReleased {
                id: 0,
                class: TransactionClass::Constitutional,
                released_at: 101,
            }
            .into(),
        );
    });
}

#[test]
fn a_release_naming_an_absent_change_fails() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::release_scheduled_change(RuntimeOrigin::signed(1), 99),
            Error::<Test>::ScheduledChangeNotFound
        );
    });
}

#[test]
fn each_scheduled_change_receives_its_own_identifier() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload(1)
        ));
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload(2)
        ));
        assert!(ScheduledChanges::<Test>::get(0).is_some());
        assert!(ScheduledChanges::<Test>::get(1).is_some());
        assert_eq!(DcfPallet::next_scheduled_change_id(), 2);
    });
}

#[test]
fn an_amended_window_governs_the_next_change() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(1);
        assert_ok!(DcfPallet::set_assurance_class(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            95,
            10
        ));
        assert_ok!(DcfPallet::schedule_class_change(
            RuntimeOrigin::root(),
            TransactionClass::Constitutional,
            payload(3)
        ));
        let change = ScheduledChanges::<Test>::get(0).expect("scheduled");
        assert_eq!(change.window.effective_at, 11);

        run_to_block(10);
        assert!(ScheduledChanges::<Test>::get(0).is_some());
        run_to_block(11);
        assert!(ScheduledChanges::<Test>::get(0).is_none());

        let resolved =
            DcfPallet::domain_assurance(sp_std::vec![TransactionClass::Constitutional], 1_000)
                .expect("resolves");
        assert_eq!(resolved.threshold_percent, 95);
        assert_eq!(resolved.required_weight, 950);
    });
}
