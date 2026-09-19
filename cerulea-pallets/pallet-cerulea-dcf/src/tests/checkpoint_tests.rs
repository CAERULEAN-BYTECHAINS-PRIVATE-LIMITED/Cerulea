//! On-chain tests for checkpoint anchoring.
//!
//! A checkpoint is one committed block the chain names as settled. The anchored
//! set carries no gap and no repeat, so the highest checkpoint settles every one
//! below it. These tests drive the anchoring extrinsic and read the effect out
//! of pallet storage.

use crate::mock::*;
use crate::{AnchoredCheckpoints, CheckpointInterval, Error, Event, LatestCheckpoint};
use cerulea_primitives::checkpoint::{Checkpoint, DEFAULT_CHECKPOINT_INTERVAL};
use frame_support::{assert_noop, assert_ok};

fn checkpoint(index: u32, block: u32, byte: u8) -> Checkpoint {
    Checkpoint::new(index, block, [byte; 32], [byte.wrapping_add(1); 32])
}

#[test]
fn the_chain_anchors_a_checkpoint_and_records_it() {
    new_test_ext().execute_with(|| {
        let one = checkpoint(0, 900, 1);
        assert_ok!(DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), one));

        assert_eq!(AnchoredCheckpoints::<Test>::get(0), Some(one));
        assert_eq!(LatestCheckpoint::<Test>::get(), Some(one));
        assert_eq!(DcfPallet::highest_anchored_block(), 900);
    });
}

#[test]
fn the_chain_raises_the_checkpoint_event_with_the_commitment() {
    new_test_ext().execute_with(|| {
        frame_system::Pallet::<Test>::set_block_number(2);
        let one = checkpoint(0, 900, 4);
        assert_ok!(DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), one));

        let raised = frame_system::Pallet::<Test>::events()
            .into_iter()
            .any(|record| {
                record.event
                    == RuntimeEvent::DcfPallet(Event::CheckpointAnchored {
                        index: 0,
                        block: 900,
                        hash: [4u8; 32],
                        commitment: one.commitment(),
                    })
            });
        assert!(raised, "the chain names the commitment it settled");
    });
}

#[test]
fn the_anchored_set_carries_no_gap() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::anchor_checkpoint(
            RuntimeOrigin::root(),
            checkpoint(0, 900, 1)
        ));
        assert_noop!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), checkpoint(2, 1_800, 2)),
            Error::<Test>::CheckpointOutOfOrder
        );
        assert_ok!(DcfPallet::anchor_checkpoint(
            RuntimeOrigin::root(),
            checkpoint(1, 1_800, 2)
        ));
        assert_eq!(DcfPallet::anchored_checkpoints().len(), 2);
    });
}

#[test]
fn the_anchored_set_rises_in_block_number() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::anchor_checkpoint(
            RuntimeOrigin::root(),
            checkpoint(0, 900, 1)
        ));
        assert_noop!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), checkpoint(1, 900, 2)),
            Error::<Test>::CheckpointOutOfOrder
        );
    });
}

#[test]
fn one_index_takes_one_checkpoint() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::anchor_checkpoint(
            RuntimeOrigin::root(),
            checkpoint(0, 900, 1)
        ));
        assert_noop!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), checkpoint(0, 900, 1)),
            Error::<Test>::CheckpointAlreadyAnchored
        );
    });
}

#[test]
fn the_anchored_set_starts_at_index_zero() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::root(), checkpoint(1, 900, 1)),
            Error::<Test>::CheckpointOutOfOrder
        );
    });
}

#[test]
fn an_account_outside_the_root_origin_anchors_nothing() {
    new_test_ext().execute_with(|| {
        assert!(
            DcfPallet::anchor_checkpoint(RuntimeOrigin::signed(1), checkpoint(0, 900, 1)).is_err()
        );
        assert!(LatestCheckpoint::<Test>::get().is_none());
    });
}

#[test]
fn the_chain_starts_from_the_published_checkpoint_interval() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::checkpoint_interval_blocks(),
            DEFAULT_CHECKPOINT_INTERVAL
        );
        assert!(DcfPallet::checkpoint_due_at(DEFAULT_CHECKPOINT_INTERVAL));
        assert!(!DcfPallet::checkpoint_due_at(
            DEFAULT_CHECKPOINT_INTERVAL - 1
        ));
    });
}

#[test]
fn the_chain_takes_a_new_checkpoint_interval() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_checkpoint_interval(
            RuntimeOrigin::root(),
            50
        ));
        assert_eq!(CheckpointInterval::<Test>::get(), 50);
        assert_eq!(DcfPallet::checkpoint_interval_blocks(), 50);
        assert!(DcfPallet::checkpoint_due_at(100));
        assert!(!DcfPallet::checkpoint_due_at(99));
    });
}

#[test]
fn an_interval_of_zero_anchors_nothing() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::set_checkpoint_interval(RuntimeOrigin::root(), 0));
        // A stored zero falls back to the interval of the starting profile.
        assert_eq!(
            DcfPallet::checkpoint_interval_blocks(),
            DEFAULT_CHECKPOINT_INTERVAL
        );
    });
}

#[test]
fn the_anchored_set_reads_back_in_index_order() {
    new_test_ext().execute_with(|| {
        for index in 0..5u32 {
            assert_ok!(DcfPallet::anchor_checkpoint(
                RuntimeOrigin::root(),
                checkpoint(index, (index + 1) * 900, index as u8 + 1)
            ));
        }
        let held = DcfPallet::anchored_checkpoints();
        assert_eq!(held.len(), 5);
        for (position, entry) in held.iter().enumerate() {
            assert_eq!(entry.index, position as u32);
        }
        assert_eq!(DcfPallet::highest_anchored_block(), 4_500);
    });
}

#[test]
fn two_checkpoints_fold_into_two_commitments() {
    new_test_ext().execute_with(|| {
        let one = checkpoint(0, 900, 1);
        let two = checkpoint(1, 1_800, 2);
        assert_ne!(one.commitment(), two.commitment());
        assert_eq!(one.commitment(), checkpoint(0, 900, 1).commitment());
    });
}
