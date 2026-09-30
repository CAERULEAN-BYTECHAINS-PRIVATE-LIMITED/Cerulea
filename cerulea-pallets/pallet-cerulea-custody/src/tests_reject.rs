//! Reject paths and edge cases of the Custody Engine.
//!
//! Each test drives the engine through its pallet calls, its block hooks or
//! its public helpers. Each test then asserts the exact error, the stored
//! state or the event the engine produced.

use crate::mock::*;
use crate::primitives::{
    Challenge, ChallengeAnswer, ChallengeState, CustodyRecord, CustodyStanding, CustodyValidators,
    ProofStep, RecordRef, MAX_PROOF_STEPS,
};
use crate::respond::{assemble_answer, may_answer, RespondError};
use crate::score::{settle, ANSWER_GAIN, FAULT_LOSS, MISS_LOSS, STARTING_SHARE};
use crate::verify::{build_proof, leaf_hash, record_root, VerifyError};
use crate::{
    ChallengeStates, CustodyRecords, Error, Event, HistoryHead, NextChallengeId, OpenChallengeIds,
    OpenChallenges, PendingLeaves, PublishedChallenge, RecordCounts, RecordRoots,
};
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_core::H256;
use sp_runtime::DispatchError;
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

fn local_copy(count: u8) -> Vec<Vec<u8>> {
    (0..count).map(|byte| sp_std::vec![byte; 16]).collect()
}

/// Issue one challenge at block five over six committed records.
fn issued() -> (u64, Challenge<u64, u64>) {
    commit_records(6);
    run_to_block(5);
    PublishedChallenge::<Test>::get().expect("block five carries a challenge")
}

fn custody_events() -> Vec<Event<Test>> {
    frame_system::Pallet::<Test>::events()
        .into_iter()
        .filter_map(|held| match held.event {
            RuntimeEvent::Custody(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

#[test]
fn an_answer_arriving_after_the_deadline_names_the_deadline() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let answer = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");

        // The block moves past the deadline before the lapse hook runs. The
        // challenge stays open, so the late answer meets the deadline check.
        frame_system::Pallet::<Test>::set_block_number(challenge.deadline + 1);
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, answer),
            Error::<Test>::DeadlinePassed
        );
        assert!(OpenChallenges::<Test>::get(id).is_some());
    });
}

#[test]
fn an_answer_at_the_deadline_block_counts_as_timely() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let answer = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");
        frame_system::Pallet::<Test>::set_block_number(challenge.deadline);
        assert_ok!(Custody::answer_challenge(
            RuntimeOrigin::signed(challenge.target),
            id,
            answer
        ));
        assert_eq!(
            ChallengeStates::<Test>::get(id),
            Some(ChallengeState::AnsweredInTime)
        );
    });
}

#[test]
fn an_answer_against_a_block_lacking_its_root_names_the_absent_root() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let answer = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");
        RecordRoots::<Test>::remove(challenge.record.block);
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, answer),
            Error::<Test>::RecordRootAbsent
        );
    });
}

#[test]
fn an_unsigned_answer_is_refused() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let answer = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::none(), id, answer),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_unsigned_record_commit_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Custody::commit_record(RuntimeOrigin::none(), [0u8; 16], key(), record(1)),
            DispatchError::BadOrigin
        );
        assert!(PendingLeaves::<Test>::get().is_empty());
    });
}

#[test]
fn a_block_holding_the_record_bound_refuses_one_more_record() {
    new_test_ext().execute_with(|| {
        // The mock bound is sixty four records per block.
        commit_records(64);
        assert_eq!(PendingLeaves::<Test>::get().len(), 64);
        assert_noop!(
            Custody::commit_record(RuntimeOrigin::signed(1), [0u8; 16], key(), record(200)),
            Error::<Test>::BlockRecordsFull
        );
        Custody::on_finalize(1);
        assert_eq!(RecordCounts::<Test>::get(1), 64);
        assert!(PendingLeaves::<Test>::get().is_empty());
    });
}

#[test]
fn a_sealed_root_matches_the_root_the_validator_rebuilds() {
    new_test_ext().execute_with(|| {
        commit_records(5);
        Custody::on_finalize(1);
        let expected = crate::respond::local_record_root(&local_copy(5));
        assert_eq!(RecordRoots::<Test>::get(1), Some(expected));
        assert_eq!(Custody::record_root_at(1), Some(expected));
        assert_eq!(Custody::record_count_at(1), 5);
        assert_eq!(Custody::history_head(), 1);
    });
}

#[test]
fn a_full_set_of_open_challenges_stops_the_next_issue() {
    new_test_ext().execute_with(|| {
        commit_records(6);
        let full: BoundedVec<u64, frame_support::traits::ConstU32<16>> =
            BoundedVec::try_from((100u64..116).collect::<Vec<_>>()).expect("sixteen fit");
        OpenChallengeIds::<Test>::put(full);
        run_to_block(5);

        assert!(PublishedChallenge::<Test>::get().is_none());
        assert!(OpenChallenges::<Test>::get(0).is_none());
        assert!(ChallengeStates::<Test>::get(0).is_none());
        assert_eq!(OpenChallengeIds::<Test>::get().len(), 16);
        assert!(!custody_events()
            .iter()
            .any(|held| matches!(held, Event::ChallengeIssued { .. })));
    });
}

#[test]
fn an_issued_challenge_lands_in_every_store_and_announces_itself() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        assert_eq!(Custody::next_challenge_id(), id + 1);
        assert_eq!(NextChallengeId::<Test>::get(), id + 1);
        assert_eq!(Custody::open_challenge(id), Some(challenge.clone()));
        assert_eq!(Custody::challenge_state(id), Some(ChallengeState::Open));
        assert_eq!(
            Custody::published_challenge(),
            Some((id, challenge.clone()))
        );
        assert_eq!(
            OpenChallengeIds::<Test>::get().into_inner(),
            sp_std::vec![id]
        );
        assert!(challenge.record.index < 6);
        assert!([1u64, 2, 3, 4].contains(&challenge.target));
        frame_system::Pallet::<Test>::assert_has_event(
            Event::<Test>::ChallengeIssued {
                id,
                target: challenge.target,
                block: challenge.block,
                deadline: challenge.deadline,
            }
            .into(),
        );
    });
}

#[test]
fn the_published_challenge_clears_on_the_next_block() {
    new_test_ext().execute_with(|| {
        let _ = issued();
        run_to_block(6);
        assert!(PublishedChallenge::<Test>::get().is_none());
        assert_eq!(OpenChallengeIds::<Test>::get().len(), 1);
    });
}

#[test]
fn the_draw_walks_forward_past_blocks_holding_no_record() {
    new_test_ext().execute_with(|| {
        // Block one and block four commit records. Blocks two and three hold
        // none, so every draw landing there walks forward to block four.
        commit_records(3);
        run_to_block(4);
        commit_records(2);
        run_to_block(5);

        let (_, challenge) = PublishedChallenge::<Test>::get().expect("issued");
        assert!(challenge.block == 1 || challenge.block == 4);
        assert!(RecordCounts::<Test>::get(challenge.block) > 0);
        assert!(challenge.record.index < RecordCounts::<Test>::get(challenge.block));
    });
}

#[test]
fn every_draw_over_a_gapped_history_reaches_a_block_holding_records() {
    // Many parent hashes give many draws. Each draw lands on a committed block.
    for seed in 0u8..24 {
        new_test_ext().execute_with(|| {
            frame_system::Pallet::<Test>::set_parent_hash(H256::repeat_byte(seed));
            RecordCounts::<Test>::insert(3u64, 2);
            HistoryHead::<Test>::put(7u64);
            RecordCounts::<Test>::insert(7u64, 1);
            let id = Custody::issue_challenge(10).expect("a draw lands on a committed block");
            let challenge = OpenChallenges::<Test>::get(id).expect("stored");
            assert!(challenge.block == 3 || challenge.block == 7);
            assert_eq!(challenge.deadline, 13);
            assert_eq!(challenge.issued, 10);
        });
    }
}

#[test]
fn a_history_head_with_no_committed_record_issues_nothing() {
    new_test_ext().execute_with(|| {
        // The head names block eight, yet no block holds a record count. The
        // walk wraps round the whole history and finds nothing to challenge.
        HistoryHead::<Test>::put(8u64);
        assert_eq!(Custody::issue_challenge(10), None);
        assert_eq!(NextChallengeId::<Test>::get(), 0);
        assert!(PublishedChallenge::<Test>::get().is_none());
    });
}

#[test]
fn a_block_off_the_interval_issues_nothing() {
    new_test_ext().execute_with(|| {
        commit_records(4);
        Custody::on_finalize(1);
        assert_eq!(Custody::issue_challenge(7), None);
        assert_eq!(Custody::issue_challenge(0), None);
        assert!(Custody::issue_challenge(10).is_some());
    });
}

#[test]
fn an_identifier_listed_without_a_stored_challenge_is_passed_over() {
    new_test_ext().execute_with(|| {
        let listed: BoundedVec<u64, frame_support::traits::ConstU32<16>> =
            BoundedVec::try_from(sp_std::vec![40u64, 41]).expect("two fit");
        OpenChallengeIds::<Test>::put(listed);
        let weight = Custody::close_lapsed_challenges(50);
        assert_eq!(weight, frame_support::weights::Weight::zero());
        assert_eq!(OpenChallengeIds::<Test>::get().len(), 2);
        assert!(ChallengeStates::<Test>::get(40).is_none());
    });
}

#[test]
fn a_lapsed_challenge_closes_as_missed_and_reports_the_share() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let closed_at = challenge.deadline + 1;
        frame_system::Pallet::<Test>::set_block_number(closed_at);
        let _ = Custody::close_lapsed_challenges(closed_at);

        let expected = STARTING_SHARE - MISS_LOSS;
        assert_eq!(
            ChallengeStates::<Test>::get(id),
            Some(ChallengeState::Missed)
        );
        assert!(OpenChallengeIds::<Test>::get().is_empty());
        assert_eq!(Custody::custody_share(&challenge.target), expected);
        assert_eq!(reported_share(challenge.target), expected);
        frame_system::Pallet::<Test>::assert_has_event(
            Event::<Test>::ChallengeMissed {
                id,
                target: challenge.target,
                share: expected,
            }
            .into(),
        );
    });
}

#[test]
fn a_correct_answer_announces_the_raised_share() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let answer = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");
        assert_ok!(Custody::answer_challenge(
            RuntimeOrigin::signed(challenge.target),
            id,
            answer
        ));
        let share = STARTING_SHARE + ANSWER_GAIN;
        frame_system::Pallet::<Test>::assert_last_event(
            Event::<Test>::ChallengeAnswered {
                id,
                target: challenge.target,
                share,
            }
            .into(),
        );
        assert_eq!(reported_share(challenge.target), share);
        assert_eq!(Custody::custody_faults(&challenge.target), 0);
    });
}

#[test]
fn a_fault_settlement_records_the_fault_and_reports_it() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        Custody::settle_challenge(id, &challenge, ChallengeState::Incorrect);

        let share = STARTING_SHARE - FAULT_LOSS;
        assert_eq!(Custody::custody_faults(&challenge.target), 1);
        assert_eq!(Custody::custody_share(&challenge.target), share);
        assert_eq!(reported_faults(challenge.target), 1);
        assert_eq!(reported_share(challenge.target), share);
        assert_eq!(
            ChallengeStates::<Test>::get(id),
            Some(ChallengeState::Incorrect)
        );
        frame_system::Pallet::<Test>::assert_last_event(
            Event::<Test>::ChallengeFaulted {
                id,
                target: challenge.target,
                share,
                faults: 1,
            }
            .into(),
        );
    });
}

#[test]
fn an_open_settlement_moves_nothing_and_announces_nothing() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let before = custody_events().len();
        Custody::settle_challenge(id, &challenge, ChallengeState::Open);

        assert_eq!(custody_events().len(), before);
        assert_eq!(ChallengeStates::<Test>::get(id), Some(ChallengeState::Open));
        assert!(OpenChallenges::<Test>::get(id).is_none());
        let held = CustodyRecords::<Test>::get(challenge.target);
        assert_eq!(held.settled(), 0);
        assert_eq!(held.share, STARTING_SHARE);
        assert_eq!(reported_share(challenge.target), STARTING_SHARE);
    });
}

#[test]
fn a_rejected_proof_leaves_the_challenge_open_for_the_deadline_path() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let wrong = assemble_answer(&local_copy(7), challenge.record.index).expect("assembles");
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, wrong),
            Error::<Test>::ProofRejected
        );
        assert_eq!(ChallengeStates::<Test>::get(id), Some(ChallengeState::Open));
        assert_eq!(Custody::custody_faults(&challenge.target), 0);
        assert_eq!(reported_faults(challenge.target), 0);
    });
}

#[test]
fn a_proof_padded_with_foreign_steps_is_rejected() {
    new_test_ext().execute_with(|| {
        let (id, challenge) = issued();
        let honest = assemble_answer(&local_copy(6), challenge.record.index).expect("assembles");
        let mut steps = honest.proof.into_inner();
        while steps.len() < MAX_PROOF_STEPS as usize {
            steps.push(ProofStep {
                sibling: H256::repeat_byte(5),
                sibling_on_left: false,
            });
        }
        let padded = ChallengeAnswer {
            value: honest.value,
            proof: BoundedVec::try_from(steps).expect("thirty two steps fit"),
        };
        assert_noop!(
            Custody::answer_challenge(RuntimeOrigin::signed(challenge.target), id, padded),
            Error::<Test>::ProofRejected
        );
    });
}

#[test]
fn a_fresh_validator_holds_the_starting_share() {
    new_test_ext().execute_with(|| {
        assert_eq!(Custody::custody_share(&9), STARTING_SHARE);
        assert_eq!(Custody::custody_faults(&9), 0);
        assert_eq!(Custody::custody_record(9), CustodyRecord::default());
    });
}

#[test]
fn a_settled_validator_reads_its_stored_share_even_at_zero() {
    new_test_ext().execute_with(|| {
        CustodyRecords::<Test>::insert(
            3u64,
            CustodyRecord {
                answered: 0,
                missed: 5,
                faults: 0,
                share: 0,
            },
        );
        assert_eq!(Custody::custody_share(&3), 0);
    });
}

#[test]
fn the_empty_validator_source_offers_no_candidate() {
    let held: Vec<u64> = <() as CustodyValidators<u64>>::custody_candidates();
    assert!(held.is_empty());
}

#[test]
fn the_empty_standing_sink_takes_every_report() {
    new_test_ext().execute_with(|| {
        <() as CustodyStanding<u64>>::report_custody_share(&1, 700_000);
        <() as CustodyStanding<u64>>::report_custody_fault(&1, 2);
        assert_eq!(reported_share(1), 0);
        assert_eq!(reported_faults(1), 0);
    });
}

#[test]
fn every_state_sentence_names_its_movement() {
    assert_eq!(
        ChallengeState::Open.plain_language(),
        "The challenge waits for an answer."
    );
    assert!(ChallengeState::AnsweredInTime
        .plain_language()
        .contains("Standing rises."));
    assert!(ChallengeState::Missed
        .plain_language()
        .contains("Standing falls."));
    assert!(ChallengeState::Incorrect
        .plain_language()
        .contains("records a fault"));
    assert_eq!(ChallengeState::default(), ChallengeState::Open);
}

#[test]
fn every_rejection_sentence_names_its_fix() {
    assert!(VerifyError::RootAbsent
        .plain_language()
        .contains("Name a block that committed records."));
    assert!(VerifyError::RootMismatch
        .plain_language()
        .contains("Send the record and the proof"));
    assert!(VerifyError::ProofTooLong
        .plain_language()
        .contains("Send the shortest chain"));
    assert!(RespondError::RecordTooLong
        .plain_language()
        .contains("Split the record"));
    assert!(RespondError::ProofTooLong
        .plain_language()
        .contains("Build the proof against the named block"));
    assert!(RespondError::WrongValidator
        .plain_language()
        .contains("Answer the challenge that names this account."));
    assert!(crate::publish::PublishError::OtherEngine
        .plain_language()
        .contains("custody identifier"));
    assert!(crate::publish::PublishError::PayloadUnreadable
        .plain_language()
        .contains("Rebuild the block"));
}

#[test]
fn a_proof_for_a_position_outside_the_block_is_empty() {
    let leaves: Vec<H256> = (0..4u32).map(|at| leaf_hash(at, &[at as u8])).collect();
    assert!(build_proof(&leaves, 4).is_empty());
    assert!(build_proof(&[], 0).is_empty());
    assert_eq!(record_root(&leaves[..1]), leaves[0]);
}

#[test]
fn the_named_validator_alone_passes_the_answer_check() {
    let held = Challenge {
        target: 5u64,
        block: 2u64,
        record: RecordRef {
            block: 2u64,
            index: 0,
            collection: [1u8; 16],
            key: BoundedVec::default(),
        },
        issued: 5u64,
        deadline: 8u64,
    };
    assert_eq!(may_answer(&held, &5), Ok(()));
    assert_eq!(may_answer(&held, &6), Err(RespondError::WrongValidator));
}

#[test]
fn an_empty_local_copy_holds_no_record_to_serve() {
    assert_eq!(assemble_answer(&[], 0), Err(RespondError::RecordAbsent));
}

#[test]
fn a_share_near_the_whole_stops_at_the_whole() {
    let mut record = CustodyRecord {
        answered: 3,
        missed: 0,
        faults: 0,
        share: CustodyRecord::FULL_SHARE - 1,
    };
    let movement = settle(&mut record, ChallengeState::AnsweredInTime);
    assert_eq!(movement.previous_share, CustodyRecord::FULL_SHARE - 1);
    assert_eq!(movement.current_share, CustodyRecord::FULL_SHARE);
    assert_eq!(record.answered, 4);
}

#[test]
fn a_fault_on_a_low_share_stops_at_zero() {
    let mut record = CustodyRecord {
        answered: 1,
        missed: 0,
        faults: 0,
        share: 10,
    };
    let movement = settle(&mut record, ChallengeState::Incorrect);
    assert_eq!(movement.current_share, 0);
    assert_eq!(movement.faults, 1);
    assert!(movement.lowered());
}
