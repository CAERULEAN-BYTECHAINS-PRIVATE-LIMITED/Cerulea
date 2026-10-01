// Tests for the inference contribution pallet.
//
// These tests reach the calls, the hook, the off-chain worker path, the
// challenge settlement and the score readers that the first test file leaves
// unreached. Every call carries an assertion on the state it leaves behind.
// Every error branch carries the exact error it returns.

use crate::mock::*;
use crate::weights::WeightInfo;
use crate::{gate, Error, InferenceErrorSeverity, PosInterface};
use frame_support::{assert_noop, assert_ok, dispatch::GetDispatchInfo, traits::Hooks};
use sp_core::offchain::{
    testing::TestOffchainExt, OffchainDbExt, OffchainWorkerExt, StorageKind, Timestamp,
};
use sp_runtime::{offchain::storage::StorageValueRef, BuildStorage, DispatchError};

/// The clock the test off-chain worker reads, in Unix milliseconds.
const WORKER_CLOCK: u64 = 1_700_000_000_000;

/// Build test externalities that carry the off-chain worker extensions.
///
/// The worker reads a clock and a persistent local store. The test store
/// answers both, and the clock reads [`WORKER_CLOCK`].
fn worker_ext() -> sp_io::TestExternalities {
    let mut ext = new_test_ext();
    let (offchain, state) = TestOffchainExt::new();
    state.write().timestamp = Timestamp::from_unix_millis(WORKER_CLOCK);
    ext.register_extension(OffchainDbExt::new(offchain.clone()));
    ext.register_extension(OffchainWorkerExt::new(offchain));
    ext
}

/// Determinism evidence that holds every condition.
fn complete_evidence() -> gate::DeterminismEvidence {
    gate::DeterminismEvidence {
        pinned_weights: true,
        fixed_seed: true,
        fixed_point_arithmetic: true,
        pinned_runtime: true,
    }
}

/// The error a refusing mock interface returns.
fn refusal() -> DispatchError {
    DispatchError::Other(MOCK_REFUSAL)
}

/// Report whether the system pallet holds one exact pallet event.
fn has_event(event: crate::Event<Test>) -> bool {
    let wanted = RuntimeEvent::PalletCeruleaPoi(event);
    System::events().iter().any(|record| record.event == wanted)
}

/// Inference data with one result and one confidence figure.
fn data(result: u32, confidence: u32) -> crate::InferenceData<Test> {
    crate::InferenceData::<Test> {
        validator: 1,
        epoch: 0,
        inference_result: result,
        confidence_score: confidence,
        timestamp: 0,
        data_sources: Vec::new(),
    }
}

// --- Genesis ---

#[test]
fn genesis_seeds_results_challenges_and_the_epoch() {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    crate::GenesisConfig::<Test> {
        inference_results: vec![(1, 40), (2, 55)],
        challenges: vec![(3, 1, 40)],
        current_epoch: 3,
    }
    .assimilate_storage(&mut storage)
    .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((40, 3)));
        assert_eq!(PalletCeruleaPoi::inference_results(2), Some((55, 3)));
        assert_eq!(PalletCeruleaPoi::challenges(3), Some((1, 40, 3)));
        assert_eq!(PalletCeruleaPoi::current_epoch(), 3);

        // A genesis result counts in the genesis epoch. Validator 1 waits for
        // the next epoch before a fresh submission.
        assert_noop!(
            PalletCeruleaPoi::submit_inference(RuntimeOrigin::signed(1), 41, 80),
            Error::<Test>::InferenceAlreadySubmitted
        );
    });
}

#[test]
fn an_empty_genesis_leaves_the_epoch_at_zero() {
    let mut storage = frame_system::GenesisConfig::<Test>::default()
        .build_storage()
        .unwrap();
    crate::GenesisConfig::<Test>::default()
        .assimilate_storage(&mut storage)
        .unwrap();

    let mut ext: sp_io::TestExternalities = storage.into();
    ext.execute_with(|| {
        assert_eq!(PalletCeruleaPoi::current_epoch(), 0);
        assert_eq!(PalletCeruleaPoi::inference_results(1), None);
        assert_eq!(PalletCeruleaPoi::challenges(1), None);
    });
}

// --- submit_inference ---

#[test]
fn a_submission_reports_the_update_the_submission_and_the_acceptance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            95
        ));

        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((42, 0)));
        assert!(has_event(crate::Event::PoiScoreUpdated {
            validator: 1,
            old_score: 0,
            new_score: 42,
            epoch: 0,
        }));
        assert!(has_event(crate::Event::InferenceSubmitted {
            who: 1,
            result: 42,
            confidence: 95,
        }));
        System::assert_last_event(
            crate::Event::InferenceAccepted {
                validator: 1,
                confidence: 95,
            }
            .into(),
        );
        assert_eq!(mock_activity(1), 1);
    });
}

#[test]
fn a_later_epoch_accepts_a_fresh_submission_and_reports_the_old_result() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        assert_eq!(mock_score(1), 5);

        PalletCeruleaPoi::advance_epoch(1);
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            43,
            95
        ));

        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((43, 1)));
        assert!(has_event(crate::Event::PoiScoreUpdated {
            validator: 1,
            old_score: 42,
            new_score: 43,
            epoch: 1,
        }));
        // The second boost of ten lands on the first boost of five.
        assert_eq!(mock_score(1), 15);
        assert_eq!(mock_activity(1), 2);
    });
}

#[test]
fn a_refusing_interface_leaves_the_submission_standing_without_acceptance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_mock_refusal(true);

        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            95
        ));

        // The result and the submission event stand.
        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((42, 0)));
        System::assert_last_event(
            crate::Event::InferenceSubmitted {
                who: 1,
                result: 42,
                confidence: 95,
            }
            .into(),
        );
        // The stake interface refused the boost, so the acceptance event stays
        // absent and the score holds at zero.
        assert!(!has_event(crate::Event::InferenceAccepted {
            validator: 1,
            confidence: 95,
        }));
        assert_eq!(mock_score(1), 0);
        // The framework interface refused the activity record too.
        assert_eq!(mock_activity(1), 0);
    });
}

// --- challenge_inference ---

#[test]
fn a_challenge_reports_the_challenge_the_rejection_and_the_penalty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            95
        ));
        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            1,
            42
        ));

        assert_eq!(PalletCeruleaPoi::challenges(2), Some((1, 42, 0)));
        assert!(has_event(crate::Event::InferenceChallenged {
            challenger: 2,
            challenged: 1,
            result: 42,
        }));
        assert!(has_event(crate::Event::InferenceRejected {
            validator: 1,
            confidence: 0,
        }));
        System::assert_last_event(
            crate::Event::ValidatorSlashed {
                validator: 1,
                reason: b"Invalid inference".to_vec(),
            }
            .into(),
        );
        assert_eq!(mock_score(1), 3);
    });
}

#[test]
fn a_refusing_interface_leaves_the_challenge_standing_without_a_penalty() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            95
        ));
        set_mock_refusal(true);

        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            1,
            42
        ));

        assert_eq!(PalletCeruleaPoi::challenges(2), Some((1, 42, 0)));
        System::assert_last_event(
            crate::Event::InferenceChallenged {
                challenger: 2,
                challenged: 1,
                result: 42,
            }
            .into(),
        );
        assert!(!has_event(crate::Event::ValidatorSlashed {
            validator: 1,
            reason: b"Invalid inference".to_vec(),
        }));
        // The score keeps the boost of ten from the submission.
        assert_eq!(mock_score(1), 10);
    });
}

#[test]
fn a_challenge_past_the_maximum_age_is_refused() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        // The maximum age is ten epochs. Epoch eleven sits past the age.
        PalletCeruleaPoi::advance_epoch(11);
        assert_noop!(
            PalletCeruleaPoi::challenge_inference(RuntimeOrigin::signed(2), 1, 42),
            Error::<Test>::InferenceTooOld
        );
    });
}

#[test]
fn a_challenge_past_the_window_is_refused() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        // The window holds five epochs. Epoch six sits inside the maximum age
        // and past the window.
        PalletCeruleaPoi::advance_epoch(6);
        assert_noop!(
            PalletCeruleaPoi::challenge_inference(RuntimeOrigin::signed(2), 1, 42),
            Error::<Test>::ChallengeWindowExpired
        );
    });
}

#[test]
fn a_challenge_on_the_last_epoch_of_the_window_lands() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        PalletCeruleaPoi::advance_epoch(5);
        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            1,
            42
        ));
        assert_eq!(PalletCeruleaPoi::challenges(2), Some((1, 42, 5)));
    });
}

#[test]
fn one_challenger_lodges_against_a_second_target_or_in_a_later_epoch() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(3),
            77,
            80
        ));

        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            1,
            42
        ));
        // A second target in the same epoch replaces the held challenge.
        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            3,
            77
        ));
        assert_eq!(PalletCeruleaPoi::challenges(2), Some((3, 77, 0)));

        // The first target again in the next epoch lands too.
        PalletCeruleaPoi::advance_epoch(1);
        assert_ok!(PalletCeruleaPoi::challenge_inference(
            RuntimeOrigin::signed(2),
            1,
            42
        ));
        assert_eq!(PalletCeruleaPoi::challenges(2), Some((1, 42, 1)));
    });
}

// --- simulate_inference ---

#[test]
fn a_simulated_inference_records_activity_and_reports_the_drawn_figures() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::simulate_inference(
            RuntimeOrigin::signed(1),
            3
        ));

        assert_eq!(PalletCeruleaPoi::validator_inference_count(3), 1);
        assert_eq!(mock_activity(3), 1);
        // The figures come from the hash of validator 3 and epoch 0.
        System::assert_last_event(
            crate::Event::InferenceSubmitted {
                who: 3,
                result: 9552,
                confidence: 88,
            }
            .into(),
        );

        PalletCeruleaPoi::advance_epoch(1);
        assert_ok!(PalletCeruleaPoi::simulate_inference(
            RuntimeOrigin::signed(1),
            2
        ));
        System::assert_last_event(
            crate::Event::InferenceSubmitted {
                who: 2,
                result: 6675,
                confidence: 74,
            }
            .into(),
        );
    });
}

#[test]
fn a_simulated_inference_for_an_inactive_validator_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::simulate_inference(RuntimeOrigin::signed(1), 9),
            Error::<Test>::ValidatorNotFound
        );
        assert_eq!(PalletCeruleaPoi::validator_inference_count(9), 0);
    });
}

#[test]
fn a_simulated_inference_asks_for_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::simulate_inference(RuntimeOrigin::root(), 1),
            DispatchError::BadOrigin
        );
    });
}

// --- apply_offchain_poi_scores ---

#[test]
fn applying_worker_scores_records_each_score_inside_the_maximum() {
    worker_ext().execute_with(|| {
        System::set_block_number(1);
        // A boost of nine shows the final score settles after the update.
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&1, 9));

        assert_ok!(PalletCeruleaPoi::submit_score(1, 40, 6));
        // The maximum validator score is one hundred. A score at the maximum
        // lands. A score above the maximum stays out.
        assert_ok!(PalletCeruleaPoi::submit_score(2, 150, 6));
        assert_ok!(PalletCeruleaPoi::submit_score(3, 100, 6));
        PalletCeruleaPoi::advance_epoch(2);

        assert_ok!(PalletCeruleaPoi::apply_offchain_poi_scores(
            RuntimeOrigin::signed(1),
            6
        ));

        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((40, 2)));
        assert_eq!(PalletCeruleaPoi::inference_results(2), None);
        assert_eq!(PalletCeruleaPoi::inference_results(3), Some((100, 2)));
        assert_eq!(mock_final_score(1), 9);
        assert!(has_event(crate::Event::ValidatorPoiScoreUpdated {
            validator: 1,
            poi_score: 40,
        }));
        assert!(has_event(crate::Event::ValidatorPoiScoreUpdated {
            validator: 3,
            poi_score: 100,
        }));
        assert!(!has_event(crate::Event::ValidatorPoiScoreUpdated {
            validator: 2,
            poi_score: 150,
        }));
    });
}

#[test]
fn applying_worker_scores_reads_the_named_block_alone() {
    worker_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_score(1, 40, 6));
        assert_ok!(PalletCeruleaPoi::apply_offchain_poi_scores(
            RuntimeOrigin::signed(1),
            7
        ));
        assert_eq!(PalletCeruleaPoi::inference_results(1), None);
    });
}

#[test]
fn applying_worker_scores_passes_over_an_undecodable_entry() {
    worker_ext().execute_with(|| {
        sp_io::offchain::local_storage_set(
            StorageKind::PERSISTENT,
            b"dcf::poi_score::1::9",
            &[1u8],
        );
        assert_ok!(PalletCeruleaPoi::apply_offchain_poi_scores(
            RuntimeOrigin::signed(1),
            9
        ));
        assert_eq!(PalletCeruleaPoi::inference_results(1), None);
    });
}

#[test]
fn applying_worker_scores_above_the_weight_bound_is_refused() {
    new_test_ext().execute_with(|| {
        // Each validator carries an estimate of fifty thousand. The bound is one
        // billion, so twenty thousand and one validators sit past the bound.
        set_mock_active_validators((1..=20_001u64).collect());
        assert_noop!(
            PalletCeruleaPoi::apply_offchain_poi_scores(RuntimeOrigin::signed(1), 5),
            Error::<Test>::WeightLimitExceeded
        );
    });
}

#[test]
fn applying_worker_scores_stops_at_the_loop_bound() {
    worker_ext().execute_with(|| {
        // Twenty thousand validators sit exactly on the weight bound. The loop
        // bound of one thousand keeps the pass to the first thousand.
        set_mock_active_validators((1..=20_000u64).collect());
        assert_ok!(PalletCeruleaPoi::submit_score(1, 30, 8));
        assert_ok!(PalletCeruleaPoi::submit_score(1000, 45, 8));
        assert_ok!(PalletCeruleaPoi::submit_score(1001, 60, 8));

        assert_ok!(PalletCeruleaPoi::apply_offchain_poi_scores(
            RuntimeOrigin::signed(1),
            8
        ));

        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((30, 0)));
        assert_eq!(PalletCeruleaPoi::inference_results(1000), Some((45, 0)));
        assert_eq!(PalletCeruleaPoi::inference_results(1001), None);
    });
}

#[test]
fn applying_worker_scores_asks_for_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::apply_offchain_poi_scores(RuntimeOrigin::root(), 5),
            DispatchError::BadOrigin
        );
    });
}

// --- update_validator_inference_score ---

#[test]
fn a_score_update_settles_the_final_score_of_a_validator_with_a_result() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&1, 9));
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&2, 4));
        crate::InferenceResults::<Test>::insert(1, (40u32, 0u32));

        assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
            RuntimeOrigin::signed(3),
            1
        ));
        assert_eq!(mock_final_score(1), 9);

        // Validator 2 holds no result, so the final score stays unsettled.
        assert_ok!(PalletCeruleaPoi::update_validator_inference_score(
            RuntimeOrigin::signed(3),
            2
        ));
        assert_eq!(mock_final_score(2), 0);
    });
}

#[test]
fn a_score_update_reports_a_refusing_framework_interface() {
    new_test_ext().execute_with(|| {
        crate::InferenceResults::<Test>::insert(1, (40u32, 0u32));
        set_mock_refusal(true);
        assert_noop!(
            PalletCeruleaPoi::update_validator_inference_score(RuntimeOrigin::signed(3), 1),
            refusal()
        );
    });
}

#[test]
fn a_score_update_asks_for_a_signed_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::update_validator_inference_score(RuntimeOrigin::root(), 1),
            DispatchError::BadOrigin
        );
    });
}

// --- declare_determinism ---

#[test]
fn a_determinism_declaration_reports_the_evidence() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(PalletCeruleaPoi::declare_determinism(
            RuntimeOrigin::root(),
            complete_evidence()
        ));
        assert_eq!(
            PalletCeruleaPoi::determinism_evidence(),
            complete_evidence()
        );
        System::assert_last_event(
            crate::Event::DeterminismDeclared {
                evidence: complete_evidence(),
            }
            .into(),
        );
    });
}

#[test]
fn a_determinism_declaration_asks_for_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::declare_determinism(RuntimeOrigin::signed(1), complete_evidence()),
            DispatchError::BadOrigin
        );
    });
}

// --- record_served_inference and the contribution gate ---

#[test]
fn a_served_inference_asks_for_the_root_origin() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::record_served_inference(
                RuntimeOrigin::signed(1),
                1,
                95,
                gate::TaskSource::IntelligenceWorkload
            ),
            DispatchError::BadOrigin
        );
    });
}

#[cfg(not(feature = "inference"))]
#[test]
fn the_default_build_refuses_every_served_inference() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::declare_determinism(
            RuntimeOrigin::root(),
            complete_evidence()
        ));
        for source in [
            gate::TaskSource::IntelligenceWorkload,
            gate::TaskSource::Synthetic,
        ] {
            assert_noop!(
                PalletCeruleaPoi::check_contribution_gate(source),
                Error::<Test>::InferenceModuleAbsent
            );
            assert_noop!(
                PalletCeruleaPoi::record_served_inference(RuntimeOrigin::root(), 1, 95, source),
                Error::<Test>::InferenceModuleAbsent
            );
        }
        assert_eq!(PalletCeruleaPoi::provisional_result(1), None);
        assert_eq!(PalletCeruleaPoi::validator_inference_count(1), 0);
    });
}

#[cfg(feature = "inference")]
#[test]
fn the_gate_names_unproven_determinism_and_a_synthetic_task() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::check_contribution_gate(gate::TaskSource::IntelligenceWorkload),
            Error::<Test>::DeterminismUnproven
        );
        assert_ok!(PalletCeruleaPoi::declare_determinism(
            RuntimeOrigin::root(),
            complete_evidence()
        ));
        assert_noop!(
            PalletCeruleaPoi::record_served_inference(
                RuntimeOrigin::root(),
                1,
                95,
                gate::TaskSource::Synthetic
            ),
            Error::<Test>::TaskSynthetic
        );
        assert_ok!(PalletCeruleaPoi::check_contribution_gate(
            gate::TaskSource::IntelligenceWorkload
        ));
    });
}

#[cfg(feature = "inference")]
#[test]
fn a_served_inference_records_a_provisional_result_and_boosts_the_validator() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::declare_determinism(
            RuntimeOrigin::root(),
            complete_evidence()
        ));
        PalletCeruleaPoi::advance_epoch(4);
        assert_ok!(PalletCeruleaPoi::record_served_inference(
            RuntimeOrigin::root(),
            1,
            95,
            gate::TaskSource::IntelligenceWorkload
        ));

        assert_eq!(
            PalletCeruleaPoi::provisional_result(1),
            Some(gate::ProvisionalResult::accept(
                95,
                gate::TaskSource::IntelligenceWorkload,
                4,
                5
            ))
        );
        assert_eq!(PalletCeruleaPoi::validator_inference_count(1), 1);
        assert_eq!(mock_score(1), 10);
        assert_eq!(mock_final_score(1), 11);
    });
}

// --- settle_inference_challenge ---

#[test]
fn a_matching_committee_holds_the_reporter_and_penalises_the_challenger() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::ProvisionalResults::<Test>::insert(
            1,
            gate::ProvisionalResult::accept(42, gate::TaskSource::IntelligenceWorkload, 0, 5),
        );
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&2, 10));

        assert_ok!(PalletCeruleaPoi::settle_inference_challenge(
            &1,
            &2,
            vec![42, 42, 7]
        ));

        // The medium penalty is three.
        assert_eq!(mock_score(2), 7);
        assert_eq!(mock_final_score(2), 7);
        assert!(PalletCeruleaPoi::provisional_result(1).is_some());
        System::assert_last_event(
            crate::Event::InferenceChallengeSettled {
                validator: 1,
                challenger: 2,
                reporter_held: true,
            }
            .into(),
        );
    });
}

#[test]
fn a_differing_committee_holds_the_challenger_and_removes_the_result() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::ProvisionalResults::<Test>::insert(
            1,
            gate::ProvisionalResult::accept(42, gate::TaskSource::IntelligenceWorkload, 0, 5),
        );
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&1, 10));

        assert_ok!(PalletCeruleaPoi::settle_inference_challenge(
            &1,
            &2,
            vec![7, 7, 42]
        ));

        // The high penalty is seven.
        assert_eq!(mock_score(1), 3);
        assert_eq!(mock_final_score(1), 3);
        assert_eq!(PalletCeruleaPoi::provisional_result(1), None);
        System::assert_last_event(
            crate::Event::InferenceChallengeSettled {
                validator: 1,
                challenger: 2,
                reporter_held: false,
            }
            .into(),
        );
    });
}

#[test]
fn an_empty_committee_holds_the_challenger() {
    new_test_ext().execute_with(|| {
        crate::ProvisionalResults::<Test>::insert(
            1,
            gate::ProvisionalResult::accept(42, gate::TaskSource::IntelligenceWorkload, 0, 5),
        );
        assert_ok!(PalletCeruleaPoi::settle_inference_challenge(
            &1,
            &2,
            Vec::new()
        ));
        assert_eq!(PalletCeruleaPoi::provisional_result(1), None);
    });
}

#[test]
fn a_settlement_without_a_provisional_result_is_refused() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            PalletCeruleaPoi::settle_inference_challenge(&5, &2, vec![1, 1, 1]),
            Error::<Test>::InferenceNotFound
        );
    });
}

#[test]
fn a_settlement_reports_a_refusing_stake_interface_and_keeps_the_result() {
    new_test_ext().execute_with(|| {
        crate::ProvisionalResults::<Test>::insert(
            1,
            gate::ProvisionalResult::accept(42, gate::TaskSource::IntelligenceWorkload, 0, 5),
        );
        set_mock_refusal(true);

        assert_noop!(
            PalletCeruleaPoi::settle_inference_challenge(&1, &2, vec![42, 42, 42]),
            refusal()
        );
        assert_noop!(
            PalletCeruleaPoi::settle_inference_challenge(&1, &2, vec![7, 7, 7]),
            refusal()
        );
        assert!(PalletCeruleaPoi::provisional_result(1).is_some());
    });
}

// --- Score readers and the epoch counter ---

#[test]
fn every_score_reader_answers_the_same_share() {
    new_test_ext().execute_with(|| {
        crate::InferenceResults::<Test>::insert(1, (300u32, 0u32));
        assert_eq!(PalletCeruleaPoi::get_score(&1), 300);
        assert_eq!(PalletCeruleaPoi::get_poi_score(&1), 300);
        assert_eq!(PalletCeruleaPoi::get_inference_score(&1), 300);
        assert_eq!(PalletCeruleaPoi::validator_inference_score(&1), 300);
        assert_eq!(PalletCeruleaPoi::contribution_share(&1), 300);
        assert_eq!(PalletCeruleaPoi::raw_inference_result(&1), 300);
        assert_eq!(PalletCeruleaPoi::raw_inference_result(&2), 0);
    });
}

#[test]
fn the_epoch_counter_follows_every_advance() {
    new_test_ext().execute_with(|| {
        assert_eq!(PalletCeruleaPoi::current_epoch(), 0);
        PalletCeruleaPoi::advance_epoch(7);
        assert_eq!(PalletCeruleaPoi::current_epoch(), 7);
        PalletCeruleaPoi::advance_epoch(8);
        assert_eq!(PalletCeruleaPoi::current_epoch(), 8);

        // A submission after the advance carries the new epoch.
        assert_ok!(PalletCeruleaPoi::submit_inference(
            RuntimeOrigin::signed(1),
            42,
            80
        ));
        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((42, 8)));
    });
}

#[test]
fn every_activity_record_raises_the_count() {
    new_test_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::record_inference_activity(&1));
        assert_ok!(PalletCeruleaPoi::record_inference_activity(&1));
        assert_eq!(PalletCeruleaPoi::validator_inference_count(1), 2);
        assert_eq!(mock_activity(1), 2);

        // A refusing framework interface leaves the pallet count rising.
        set_mock_refusal(true);
        assert_ok!(PalletCeruleaPoi::record_inference_activity(&1));
        assert_eq!(PalletCeruleaPoi::validator_inference_count(1), 3);
        assert_eq!(mock_activity(1), 2);
    });
}

// --- Valid and invalid inference handling ---

#[test]
fn a_valid_inference_carries_the_boost_its_confidence_band_names() {
    new_test_ext().execute_with(|| {
        // The high band starts at ninety and carries ten.
        assert_ok!(PalletCeruleaPoi::handle_valid_inference(&1, 90));
        assert_eq!(mock_score(1), 10);
        // The medium band starts at seventy and carries five.
        assert_ok!(PalletCeruleaPoi::handle_valid_inference(&2, 70));
        assert_eq!(mock_score(2), 5);
        assert_ok!(PalletCeruleaPoi::handle_valid_inference(&3, 89));
        assert_eq!(mock_score(3), 5);
        // Confidence below seventy carries two.
        assert_ok!(PalletCeruleaPoi::handle_valid_inference(&4, 69));
        assert_eq!(mock_score(4), 2);
        assert_eq!(mock_final_score(4), 3);
    });
}

#[test]
fn a_valid_inference_reports_a_refusing_stake_interface() {
    new_test_ext().execute_with(|| {
        set_mock_refusal(true);
        assert_eq!(
            PalletCeruleaPoi::handle_valid_inference(&1, 95),
            Err(refusal())
        );
        // The activity record runs before the boost.
        assert_eq!(PalletCeruleaPoi::validator_inference_count(1), 1);
        assert_eq!(mock_score(1), 0);
    });
}

#[test]
fn an_invalid_inference_carries_the_medium_and_the_low_penalty() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&1, 10));

        assert_ok!(PalletCeruleaPoi::handle_invalid_inference(
            &1,
            InferenceErrorSeverity::Medium
        ));
        assert_eq!(mock_score(1), 7);

        assert_ok!(PalletCeruleaPoi::handle_invalid_inference(
            &1,
            InferenceErrorSeverity::Low
        ));
        assert_eq!(mock_score(1), 6);
        assert_eq!(mock_final_score(1), 6);
    });
}

#[test]
fn an_invalid_inference_reports_a_refusing_stake_interface() {
    new_test_ext().execute_with(|| {
        assert_ok!(<Test as crate::Config>::PosInterface::boost_score(&1, 10));
        set_mock_refusal(true);
        assert_noop!(
            PalletCeruleaPoi::handle_invalid_inference(&1, InferenceErrorSeverity::Low),
            refusal()
        );
        assert_eq!(mock_score(1), 10);
    });
}

// --- Inference computation ---

#[test]
fn the_simulated_computation_answers_the_hash_figures() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            PalletCeruleaPoi::simulate_inference_computation(&1, 0),
            (5672, 89)
        );
        assert_eq!(
            PalletCeruleaPoi::simulate_inference_computation(&3, 0),
            (9552, 88)
        );
        assert_eq!(
            PalletCeruleaPoi::simulate_inference_computation(&2, 1),
            (6675, 74)
        );
        assert_eq!(
            PalletCeruleaPoi::simulate_inference_computation(&7, 3),
            (1757, 76)
        );
    });
}

#[test]
fn the_simulated_computation_stays_inside_its_bounds() {
    new_test_ext().execute_with(|| {
        for validator in 1..=20u64 {
            for epoch in 0..5u32 {
                let (result, confidence) =
                    PalletCeruleaPoi::simulate_inference_computation(&validator, epoch);
                // The result reads below the percentage precision of ten
                // thousand. The confidence sits in the band from seventy up to
                // ninety.
                assert!(result < 10_000);
                assert!((70..90).contains(&confidence));
            }
        }
    });
}

#[test]
fn the_computed_score_weighs_the_result_by_confidence_and_holds_at_the_maximum() {
    // The weighted score is the result times the confidence over ten
    // thousand. The maximum validator score is one hundred.
    assert_eq!(PalletCeruleaPoi::compute_poi_score(&data(5000, 80)), 40);
    assert_eq!(PalletCeruleaPoi::compute_poi_score(&data(9000, 100)), 90);
    assert_eq!(PalletCeruleaPoi::compute_poi_score(&data(10_000, 100)), 100);
    assert_eq!(PalletCeruleaPoi::compute_poi_score(&data(20_000, 100)), 100);
    assert_eq!(
        PalletCeruleaPoi::compute_poi_score(&data(u32::MAX, u32::MAX)),
        100
    );
    assert_eq!(PalletCeruleaPoi::compute_poi_score(&data(99, 100)), 0);
}

#[test]
fn collected_data_reads_the_chain_result_where_one_exists() {
    worker_ext().execute_with(|| {
        crate::InferenceResults::<Test>::insert(4, (6000u32, 2u32));
        let held = PalletCeruleaPoi::collect_inference_data(&4, 9).unwrap();
        assert_eq!(held.validator, 4);
        assert_eq!(held.epoch, 9);
        assert_eq!(held.inference_result, 6000);
        assert_eq!(held.timestamp, WORKER_CLOCK);
        assert_eq!(held.data_sources, vec![b"poi_pallet".to_vec()]);
    });
}

#[test]
fn collected_data_draws_from_the_simulation_where_the_chain_holds_nothing() {
    worker_ext().execute_with(|| {
        let drawn = PalletCeruleaPoi::collect_inference_data(&1, 3).unwrap();
        assert_eq!(drawn.validator, 1);
        assert_eq!(drawn.epoch, 3);
        assert_eq!(drawn.inference_result, 2559);
        assert_eq!(drawn.confidence_score, 73);
        assert_eq!(drawn.timestamp, WORKER_CLOCK);
        assert_eq!(drawn.data_sources, vec![b"simulation".to_vec()]);
    });
}

// --- Off-chain score store ---

#[test]
fn a_submitted_score_verifies_against_its_validator_score_and_block() {
    worker_ext().execute_with(|| {
        assert_ok!(PalletCeruleaPoi::submit_score(1, 77, 9));
        assert!(PalletCeruleaPoi::verify_score(&1, 77, 9));
        assert!(!PalletCeruleaPoi::verify_score(&1, 78, 9));
        assert!(!PalletCeruleaPoi::verify_score(&1, 77, 10));
        assert!(!PalletCeruleaPoi::verify_score(&2, 77, 9));
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&1, 9),
            Ok(Some(77))
        );

        let stored = StorageValueRef::persistent(b"dcf::poi_score::1::9")
            .get::<crate::OffchainPoiScore<Test>>()
            .unwrap()
            .unwrap();
        assert_eq!(stored.validator, 1);
        assert_eq!(stored.score, 77);
        assert_eq!(stored.block_number, 9);
        assert_eq!(stored.timestamp, WORKER_CLOCK);
    });
}

#[test]
fn an_undecodable_stored_score_reads_as_an_error() {
    worker_ext().execute_with(|| {
        sp_io::offchain::local_storage_set(
            StorageKind::PERSISTENT,
            b"dcf::poi_score::1::9",
            &[1u8],
        );
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&1, 9),
            Err("Failed to retrieve inference score from off-chain storage")
        );
        assert!(!PalletCeruleaPoi::verify_score(&1, 1, 9));
    });
}

// --- The off-chain worker hook ---

#[test]
fn the_worker_hook_runs_on_the_interval_and_stores_each_changed_score() {
    worker_ext().execute_with(|| {
        // Validator 1 holds a raw result of zero. The worker computes zero
        // too, so the worker stores nothing for validator 1.
        crate::InferenceResults::<Test>::insert(1, (0u32, 0u32));

        // Block four sits off the interval of five.
        <PalletCeruleaPoi as Hooks<u64>>::offchain_worker(4);
        for validator in [1u64, 2, 3] {
            assert_eq!(
                PalletCeruleaPoi::get_offchain_poi_score(&validator, 4),
                Ok(None)
            );
        }

        <PalletCeruleaPoi as Hooks<u64>>::offchain_worker(5);
        assert_eq!(PalletCeruleaPoi::get_offchain_poi_score(&1, 5), Ok(None));
        // Validators 2 and 3 hold no chain result. The worker draws from the
        // simulation and stores the weighted score.
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&2, 5),
            Ok(Some(22))
        );
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&3, 5),
            Ok(Some(84))
        );

        let stored = StorageValueRef::persistent(b"dcf::poi_score::3::5")
            .get::<crate::OffchainPoiScore<Test>>()
            .unwrap()
            .unwrap();
        assert_eq!(stored.block_number, 5);
        assert_eq!(stored.timestamp, WORKER_CLOCK);
    });
}

#[test]
fn the_worker_computation_runs_on_any_block_when_called_directly() {
    worker_ext().execute_with(|| {
        assert_eq!(PalletCeruleaPoi::run_offchain_computation(7), Ok(()));
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&1, 7),
            Ok(Some(50))
        );
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&2, 7),
            Ok(Some(22))
        );
        assert_eq!(
            PalletCeruleaPoi::get_offchain_poi_score(&3, 7),
            Ok(Some(84))
        );
    });
}

#[test]
fn worker_scores_flow_from_the_hook_to_the_chain() {
    worker_ext().execute_with(|| {
        System::set_block_number(1);
        <PalletCeruleaPoi as Hooks<u64>>::offchain_worker(5);
        assert_ok!(PalletCeruleaPoi::apply_offchain_poi_scores(
            RuntimeOrigin::signed(1),
            5
        ));

        assert_eq!(PalletCeruleaPoi::inference_results(1), Some((50, 0)));
        assert_eq!(PalletCeruleaPoi::inference_results(2), Some((22, 0)));
        assert_eq!(PalletCeruleaPoi::inference_results(3), Some((84, 0)));
        assert!(PalletCeruleaPoi::verify_score(&3, 84, 5));
        System::assert_last_event(
            crate::Event::ValidatorPoiScoreUpdated {
                validator: 3,
                poi_score: 84,
            }
            .into(),
        );
    });
}

// --- Dispatch weights ---

#[test]
fn every_call_charges_the_figure_its_weight_function_names() {
    new_test_ext().execute_with(|| {
        let submit = <() as WeightInfo>::submit_inference();
        let challenge = <() as WeightInfo>::challenge_inference();

        assert_eq!(
            crate::Call::<Test>::submit_inference {
                result: 1,
                confidence: 60
            }
            .get_dispatch_info()
            .call_weight,
            submit
        );
        assert_eq!(
            crate::Call::<Test>::challenge_inference {
                challenged: 1,
                result: 1
            }
            .get_dispatch_info()
            .call_weight,
            challenge
        );
        assert_eq!(
            crate::Call::<Test>::simulate_inference {
                target_validator: 1
            }
            .get_dispatch_info()
            .call_weight,
            submit
        );
        assert_eq!(
            crate::Call::<Test>::apply_offchain_poi_scores { block_number: 1 }
                .get_dispatch_info()
                .call_weight,
            submit
        );
        assert_eq!(
            crate::Call::<Test>::update_validator_inference_score { validator: 1 }
                .get_dispatch_info()
                .call_weight,
            submit
        );
        assert_eq!(
            crate::Call::<Test>::declare_determinism {
                evidence: complete_evidence()
            }
            .get_dispatch_info()
            .call_weight,
            submit
        );
        assert_eq!(
            crate::Call::<Test>::record_served_inference {
                validator: 1,
                confidence: 95,
                source: gate::TaskSource::IntelligenceWorkload
            }
            .get_dispatch_info()
            .call_weight,
            submit
        );
        // A challenge reads and writes storage. The figure sits above the
        // submission figure, which writes alone.
        assert!(challenge.ref_time() > submit.ref_time());
    });
}
