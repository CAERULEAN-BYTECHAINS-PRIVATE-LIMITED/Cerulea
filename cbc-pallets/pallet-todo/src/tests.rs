// tests.rs – unit tests for pallet-todo

use crate::{mock::*, Error, Event, Todos, NextTodoId, TodoCount};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use frame_system::RawOrigin;

// ── helpers ──────────────────────────────────────────────────

fn title(s: &str) -> BoundedVec<u8, <Test as crate::Config>::MaxTitleLength> {
    BoundedVec::try_from(s.as_bytes().to_vec()).unwrap()
}

fn desc(s: &str) -> BoundedVec<u8, <Test as crate::Config>::MaxDescriptionLength> {
    BoundedVec::try_from(s.as_bytes().to_vec()).unwrap()
}

const ALICE: u64 = 1;
const BOB: u64 = 2;

// ── create_todo ───────────────────────────────────────────────

#[test]
fn create_todo_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Buy groceries"),
            desc("Milk, eggs, bread"),
        ));

        // Storage should have the item
        let item = Todos::<Test>::get(ALICE, 0).expect("item should exist");
        assert_eq!(item.id, 0);
        assert!(!item.completed);

        // Next ID advances
        assert_eq!(NextTodoId::<Test>::get(ALICE), 1);
        // Count increments
        assert_eq!(TodoCount::<Test>::get(ALICE), 1);

        // Event emitted
        System::assert_last_event(
            Event::TodoCreated { owner: ALICE, todo_id: 0 }.into(),
        );
    });
}

#[test]
fn create_multiple_todos_increments_ids() {
    new_test_ext().execute_with(|| {
        for i in 0..3u64 {
            assert_ok!(TodoPallet::create_todo(
                RawOrigin::Signed(ALICE).into(),
                title(&format!("Task {i}")),
                desc(""),
            ));
            assert_eq!(NextTodoId::<Test>::get(ALICE), i + 1);
        }
        assert_eq!(TodoCount::<Test>::get(ALICE), 3);
    });
}

#[test]
fn create_todo_ids_are_per_account() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Alice task 1"),
            desc(""),
        ));
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(BOB).into(),
            title("Bob task 1"),
            desc(""),
        ));

        // Both start at id=0 independently
        assert!(Todos::<Test>::get(ALICE, 0).is_some());
        assert!(Todos::<Test>::get(BOB, 0).is_some());
        // Neither owns the other's item
        assert!(Todos::<Test>::get(ALICE, 1).is_none());
        assert!(Todos::<Test>::get(BOB, 1).is_none());
    });
}

// ── complete_todo ─────────────────────────────────────────────

#[test]
fn complete_todo_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Buy groceries"),
            desc(""),
        ));

        assert_ok!(TodoPallet::complete_todo(
            RawOrigin::Signed(ALICE).into(),
            0,
        ));

        let item = Todos::<Test>::get(ALICE, 0).unwrap();
        assert!(item.completed);

        System::assert_last_event(
            Event::TodoCompleted { owner: ALICE, todo_id: 0 }.into(),
        );
    });
}

#[test]
fn complete_todo_fails_if_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            TodoPallet::complete_todo(RawOrigin::Signed(ALICE).into(), 99),
            Error::<Test>::TodoNotFound,
        );
    });
}

#[test]
fn complete_todo_fails_if_already_completed() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Task"),
            desc(""),
        ));
        assert_ok!(TodoPallet::complete_todo(RawOrigin::Signed(ALICE).into(), 0));
        assert_noop!(
            TodoPallet::complete_todo(RawOrigin::Signed(ALICE).into(), 0),
            Error::<Test>::AlreadyCompleted,
        );
    });
}

// ── remove_todo ───────────────────────────────────────────────

#[test]
fn remove_todo_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Task"),
            desc(""),
        ));
        assert_ok!(TodoPallet::remove_todo(RawOrigin::Signed(ALICE).into(), 0));

        assert!(Todos::<Test>::get(ALICE, 0).is_none());
        assert_eq!(TodoCount::<Test>::get(ALICE), 0);

        System::assert_last_event(
            Event::TodoRemoved { owner: ALICE, todo_id: 0 }.into(),
        );
    });
}

#[test]
fn remove_todo_fails_if_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            TodoPallet::remove_todo(RawOrigin::Signed(ALICE).into(), 0),
            Error::<Test>::TodoNotFound,
        );
    });
}

// ── update_todo ───────────────────────────────────────────────

#[test]
fn update_todo_works() {
    new_test_ext().execute_with(|| {
        assert_ok!(TodoPallet::create_todo(
            RawOrigin::Signed(ALICE).into(),
            title("Old title"),
            desc("old desc"),
        ));

        assert_ok!(TodoPallet::update_todo(
            RawOrigin::Signed(ALICE).into(),
            0,
            title("New title"),
            desc("new desc"),
        ));

        let item = Todos::<Test>::get(ALICE, 0).unwrap();
        assert_eq!(&item.title[..], b"New title");
        assert_eq!(&item.description[..], b"new desc");

        System::assert_last_event(
            Event::TodoUpdated { owner: ALICE, todo_id: 0 }.into(),
        );
    });
}

#[test]
fn update_todo_fails_if_not_found() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            TodoPallet::update_todo(
                RawOrigin::Signed(ALICE).into(),
                0,
                title("x"),
                desc("y"),
            ),
            Error::<Test>::TodoNotFound,
        );
    });
}
