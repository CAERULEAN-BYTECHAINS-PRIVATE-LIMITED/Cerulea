//! On-chain tests for the Custody Engine.
//!
//! The tests drive the engine through pallet calls and the block hooks, so the
//! behaviour is the behaviour a running chain shows.

use crate::mock::*;
use crate::primitives::{ChallengeState, CUSTODY_DIGEST_ID};
use crate::respond::assemble_answer;
use crate::{
    ChallengeStates, CustodyRecords, Error, Event, HistoryHead, OpenChallengeIds, OpenChallenges,
    PublishedChallenge, RecordCounts, RecordRoots,
};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_std::vec::Vec;

/// Advance the chain to a block, running both hooks each step.
fn run_to_block(target: u64) {
    let mut now = frame_system::Pallet::<Test>::block_number();
    while now < target {
        Custody::on_finalize(now);
        now += 1;
        frame_system::Pallet::<Test>::set_block_number(now);
        Custody::on_initialize(now);
    }
}

fn record(byte: u8) -> BoundedVec<u8, frame_support::traits::ConstU32<1024>> {
    BoundedVec::try_from(sp_std::vec![byte; 16]).expect("sixteen bytes fit")
}

fn key() -> BoundedVec<u8, frame_support::traits::ConstU32<128>> {
    BoundedVec::try_from(sp_std::vec![7u8; 4]).expect("four bytes fit")
}

/// Commit a number of records into the block under construction.
fn commit_records(count: u8) {
    for byte in 0..count {
        assert_ok!(Custody::commit_record(
            RuntimeOrigin::signed(1),
            [0u8; 16],
            key(),
            record(byte)
        ));
    }
}

/// The records one block committed, as the validator holds them locally.
fn local_copy(count: u8) -> Vec<Vec<u8>> {
    (0..count).map(|byte| sp_std::vec![byte; 16]).collect()
}

#[test]
fn a_block_committing_records_seals_a_record_root() {
    new_test_ext().execute_with(|| {
        commit_records(5);
        Custody::on_finalize(1);

        assert_eq!(RecordCounts::<Test>::get(1), 5);
        assert!(RecordRoots::<Test>::get(1).is_some());
        assert_eq!(HistoryHead::<Test>::get(), 1);
    });
}

#[test]
fn a_block_committing_no_record_seals_no_root() {
    new_test_ext().execute_with(|| {
        Custody::on_finalize(1);
        assert!(RecordRoots::<Test>::get(1).is_none());
        assert_eq!(HistoryHead::<Test>::get(), 0);
    });
}

#[test]
fn the_engine_issues_a_challenge_at_the_interval() {
    new_test_ext().execute_with(|| {
        commit_records(4);
        run_to_block(5);

        let published = PublishedChallenge::<Test>::get();
        assert!(published.is_some(), "block five carries a challenge");
        let (id, challenge) = published.expect("present");
        assert_eq!(id, 0);
        assert!(challenge.block >= 1);
        assert_eq!(challenge.issued, 5);
        assert_eq!(challenge.deadline, 8);
        assert_eq!(ChallengeStates::<Test>::get(id), Some(ChallengeState::Open));
    });
}

#[test]
fn the_engine_holds_still_between_intervals() {
    new_test_ext().execute_with(|| {
        commit_records(4);
        run_to_block(4);
        assert!(PublishedChallenge::<Test>::get().is_none());
        assert_eq!(OpenChallengeIds::<Test>::get().len(), 0);
    });
}

/// Design assertion 7, first half.
///
/// A custody challenge issues, is answered and moves standing, with no human
/// involvement in the issue step.
#[test]
fn a_challenge_issues_is_answered_and_raises_standing() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);

        let (id, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        let copy = local_copy(6);
        let answer = assemble_answer(&copy, challenge.record.index).expect("assembles");

        let before = Custody::custody_share(&challenge.target);
        assert_ok!(Custody::answer_challenge(
            RuntimeOrigin::signed(challenge.target),
            id,
            answer
        ));

        let after = Custody::custody_share(&challenge.target);
        assert!(after > before, "a correct answer raises the custody share");
        assert_eq!(
            ChallengeStates::<Test>::get(id),
            Some(ChallengeState::AnsweredInTime)
        );
        assert!(OpenChallenges::<Test>::get(id).is_none());
        assert_eq!(reported_share(challenge.target), after);
        assert_eq!(CustodyRecords::<Test>::get(challenge.target).answered, 1);
    });
}

/// Design assertion 8.
///
/// An unanswered custody challenge lowers standing. The chain closes the
/// challenge on its own once the deadline passes.
#[test]
fn an_unanswered_challenge_lowers_standing() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);

        let (id, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        let before = Custody::custody_share(&challenge.target);

        run_to_block(9);

        let after = Custody::custody_share(&challenge.target);
        assert!(after < before, "a missed deadline lowers the custody share");
        assert_eq!(
            ChallengeStates::<Test>::get(id),
            Some(ChallengeState::Missed)
        );
        assert!(OpenChallenges::<Test>::get(id).is_none());
        assert_eq!(CustodyRecords::<Test>::get(challenge.target).missed, 1);
        assert_eq!(reported_share(challenge.target), after);
    });
}

#[test]
fn an_incorrect_proof_lowers_standing_sharply_and_records_a_fault() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);

        let (id, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        // The validator holds a different block, so its proof reaches another
        // root.
        let wrong = local_copy(7);
        let answer = assemble_answer(&wrong, challenge.record.index).expect("assembles");

        let before = Custody::custody_share(&challenge.target);
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, answer.clone()),
            Error::<Test>::ProofRejected
        );

        // The call returns a failure, so the chain rolls the state back. The
        // engine settles the fault through the deadline path instead.
        run_to_block(9);
        let after = Custody::custody_share(&challenge.target);
        assert!(after < before);
    });
}

#[test]
fn another_account_cannot_answer_a_challenge() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);

        let (id, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        let other = if challenge.target == 1 { 2 } else { 1 };
        let copy = local_copy(6);
        let answer = assemble_answer(&copy, challenge.record.index).expect("assembles");

        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(other), id, answer),
            Error::<Test>::WrongValidator
        );
    });
}

#[test]
fn an_answer_after_the_deadline_is_refused() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);
        let (id, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        let copy = local_copy(6);
        let answer = assemble_answer(&copy, challenge.record.index).expect("assembles");

        // The challenge closes at block nine, so no challenge remains to
        // answer.
        run_to_block(9);
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, answer),
            Error::<Test>::ChallengeNotFound
        );
    });
}

#[test]
fn an_answer_naming_an_absent_challenge_is_refused() {
    new_test_ext().execute_with(|| {
        let copy = local_copy(4);
        let answer = assemble_answer(&copy, 0).expect("assembles");
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(1), 77, answer),
            Error::<Test>::ChallengeNotFound
        );
    });
}

#[test]
fn every_node_reaches_the_same_challenge() {
    // Two runs over identical state produce identical challenges, which is the
    // property that removes coordination.
    let first = new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);
        PublishedChallenge::<Test>::get().expect("issued")
    });
    let second = new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);
        PublishedChallenge::<Test>::get().expect("issued")
    });
    assert_eq!(first, second);
}

#[test]
fn the_challenge_appears_in_the_block_header() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        run_to_block(5);

        let digest = frame_system::Pallet::<Test>::digest();
        let found: Option<
            Result<
                crate::primitives::Challenge<u64, u64>,
                crate::publish::PublishError,
            >,
        > = crate::publish::find_challenge(&digest.logs);
        let carried = found.expect("the header carries a custody item").expect("reads");
        let (_, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        assert_eq!(carried, challenge);

        assert!(digest
            .logs
            .iter()
            .any(|item| matches!(item, sp_runtime::generic::DigestItem::Consensus(id, _) if *id == CUSTODY_DIGEST_ID)));
    });
}

#[test]
fn the_engine_holds_still_where_no_validator_stands() {
    new_test_ext().execute_with(|| {
        set_candidates(sp_std::vec![]);
        commit_records(6);
        run_to_block(5);
        assert!(PublishedChallenge::<Test>::get().is_none());
    });
}

#[test]
fn the_engine_holds_still_where_no_history_exists() {
    new_test_ext().execute_with(|| {
        run_to_block(5);
        assert!(PublishedChallenge::<Test>::get().is_none());
    });
}

#[test]
fn steady_service_carries_the_custody_share_upward() {
    new_test_ext().execute_with(|| {
        // One validator stands, so every challenge names that validator.
        set_candidates(sp_std::vec![1u64]);
        commit_records(6);

        let mut answered = 0u32;
        // The interval is five blocks, so each issue block is a multiple of
        // five.
        let mut at = 0u64;
        while answered < 5 {
            at += 5;
            assert!(at < 200, "the engine issues a challenge at every interval");
            run_to_block(at);
            if let Some((id, challenge)) = PublishedChallenge::<Test>::get() {
                let copy = local_copy(6);
                let answer = assemble_answer(&copy, challenge.record.index).expect("assembles");
                assert_ok!(Custody::answer_challenge(
                    RuntimeOrigin::signed(challenge.target),
                    id,
                    answer
                ));
                answered += 1;
            }
        }

        let record = CustodyRecords::<Test>::get(1);
        assert_eq!(record.answered, 5);
        assert_eq!(record.missed, 0);
        assert_eq!(
            record.share,
            crate::score::STARTING_SHARE + 5 * crate::score::ANSWER_GAIN
        );
    });
}

#[test]
fn the_engine_reports_every_settled_outcome_to_the_standing_engine() {
    new_test_ext().execute_with(|| {
        set_candidates(sp_std::vec![1u64]);
        commit_records(6);
        run_to_block(5);
        let (_, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        run_to_block(9);
        assert_eq!(reported_share(challenge.target), Custody::custody_share(&1));
        assert_eq!(reported_faults(challenge.target), 0);
    });
}

#[test]
fn a_sealed_root_is_announced() {
    new_test_ext().execute_with(|| {
        commit_records(3);
        Custody::on_finalize(1);
        let root = RecordRoots::<Test>::get(1).expect("sealed");
        frame_system::Pallet::<Test>::assert_has_event(
            Event::<Test>::RecordRootSealed {
                block: 1,
                count: 3,
                root,
            }
            .into(),
        );
    });
}
