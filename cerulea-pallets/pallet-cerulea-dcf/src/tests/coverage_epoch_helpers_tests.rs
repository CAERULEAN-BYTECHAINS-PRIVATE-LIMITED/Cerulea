//! Coverage tests for the epoch helpers of the framework pallet.
//!
//! These tests drive the score helpers, the block reports, the invariant
//! checks, the epoch transition, the deterministic engine, the storage
//! checks, the cooldown query, the private chain wrappers and the EVM event
//! query. Each test reads the state the pallet writes.
//!
//! The Standing Engine records every score. The Authoring Engine reads the
//! author sequence the epoch transition writes. The Admission Engine holds
//! the validator set the transition reads.

use crate::mock::*;
use codec::Encode;
use frame_support::traits::{ConstU32, Hooks, ReservableCurrency};
use frame_support::weights::Weight;
use frame_support::{assert_err, assert_noop, assert_ok, BoundedVec};
use sp_runtime::{DispatchError, DispatchResult};

type ValidatorList = BoundedVec<u64, MaxValidators>;
type AuthorList = BoundedVec<u64, ConstU32<1000>>;
type ScoreList = BoundedVec<(u64, u64), MaxValidators>;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Every event the framework pallet deposited in this test.
fn dcf_events() -> Vec<crate::Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// Report whether the framework pallet deposited one exact event.
fn has_event(event: crate::Event<Test>) -> bool {
    dcf_events().contains(&event)
}

/// Read the state record of one validator.
fn state_of(validator: u64) -> crate::ValidatorState<Test> {
    crate::ValidatorStates::<Test>::get(&validator).expect("The validator holds a state record.")
}

/// Change the state record of one validator in place.
fn edit_state(validator: u64, edit: impl FnOnce(&mut crate::ValidatorState<Test>)) {
    crate::ValidatorStates::<Test>::mutate(&validator, |maybe_state| {
        let state = maybe_state
            .as_mut()
            .expect("The validator holds a state record.");
        edit(state);
    });
}

/// Hold one final score on one validator.
fn set_final_score(validator: u64, score: u64) {
    edit_state(validator, move |state| state.current.final_score = score);
}

/// Recompute the final score through the interface the inference module calls.
fn update_final_score(validator: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_poi::DcfInterface<u64>>::update_final_score(&validator)
}

/// Eject one validator through the interface the inference module calls.
fn eject(validator: u64) -> DispatchResult {
    <DcfPallet as pallet_cerulea_poi::DcfInterface<u64>>::eject_validator(
        &validator,
        "standing below the floor",
    )
}

/// Run the block initialisation hook at one block.
fn run_hook(block: u64) -> Weight {
    System::set_block_number(block);
    <DcfPallet as Hooks<u64>>::on_initialize(block)
}

/// Build a bounded validator list.
fn list_of(members: &[u64]) -> ValidatorList {
    BoundedVec::truncate_from(members.to_vec())
}

/// Hold one active set in the validator registry.
fn set_active(members: &[u64]) {
    crate::ActiveValidators::<Test>::put(list_of(members));
}

/// Hold the same author for every slot of one epoch.
fn hold_author_sequence(epoch: u32, author: u64) {
    let sequence: AuthorList = BoundedVec::truncate_from(vec![author; 10]);
    crate::EpochAuthorSequences::<Test>::insert(epoch, sequence);
}

/// Hold one score snapshot for one epoch.
fn hold_snapshot(epoch: u32, entries: Vec<(u64, u64)>) {
    let snapshot: BoundedVec<(u64, u64), ConstU32<1000>> = BoundedVec::truncate_from(entries);
    crate::EpochScoreSnapshot::<Test>::insert(epoch, snapshot);
}

/// Build one epoch history record.
fn history(epoch: u32, members: &[u64]) -> crate::EpochHistory<Test> {
    crate::EpochHistory {
        epoch_number: epoch,
        active_validators: list_of(members),
        score_snapshot: BoundedVec::new(),
        inference_summary: BoundedVec::new(),
    }
}

/// Hold a list of epoch history records.
fn put_histories(entries: Vec<crate::EpochHistory<Test>>) {
    let bounded: BoundedVec<crate::EpochHistory<Test>, MaxEpochHistory> =
        BoundedVec::truncate_from(entries);
    crate::EpochHistories::<Test>::put(bounded);
}

/// Read the final score of each member, in the order given.
fn scores_of(members: &[u64]) -> ScoreList {
    BoundedVec::truncate_from(
        members
            .iter()
            .map(|validator| (*validator, state_of(*validator).current.final_score))
            .collect::<Vec<_>>(),
    )
}

/// Build a processing record from the chain state of this block.
fn stored_output(
    epoch: u32,
    seed: [u8; 32],
    output_hash: [u8; 32],
) -> crate::EpochProcessingOutput<Test> {
    let active = DcfPallet::active_validators();
    let members = active.to_vec();
    let author_sequence = DcfPallet::generate_deterministic_author_sequence(epoch, &active, &seed);
    crate::EpochProcessingOutput {
        epoch,
        transition_block: 70,
        active_validators: active,
        added_validators: BoundedVec::new(),
        removed_validators: BoundedVec::new(),
        validator_scores: scores_of(&members),
        author_sequence,
        randomness_seed: seed,
        input_hash: [3u8; 32],
        output_hash,
    }
}

/// Read the replay hash from an output mismatch.
fn replay_hash_for(epoch: u32) -> [u8; 32] {
    match DcfPallet::validate_epoch_replay(epoch) {
        Err(crate::ReplayValidationError::OutputMismatch { replay_hash, .. }) => replay_hash,
        other => panic!(
            "The replay answered {:?}. An output mismatch was expected.",
            other
        ),
    }
}

/// Read the rule a post validation failure names.
fn rule_of(result: Result<Weight, crate::MigrationError>) -> Vec<u8> {
    match result {
        Err(crate::MigrationError::PostValidationFailed { rule_id, .. }) => rule_id.into_inner(),
        other => panic!(
            "The check answered {:?}. A post validation failure was expected.",
            other
        ),
    }
}

/// Hold the stored governance epoch length at a fixed value inside its range.
///
/// The test genesis seeds ten blocks per epoch, the floor of the mock range.
/// The helper moves the value to one hundred so the tests read one figure.
fn hold_valid_governance() {
    let mut settings = crate::GovernanceConfigStorage::<Test>::get();
    settings.epoch_length.current = 100;
    crate::GovernanceConfigStorage::<Test>::put(settings);
}

/// Copy the stored governance record with a valid epoch length and one edit.
fn governance_with(
    edit: impl FnOnce(&mut crate::GovernanceConfig<Test>),
) -> crate::GovernanceConfig<Test> {
    let mut settings = crate::GovernanceConfigStorage::<Test>::get();
    settings.epoch_length.current = 100;
    edit(&mut settings);
    settings
}

/// Build one EVM record of one type.
fn evm_record(event_type: u32) -> crate::evm_compatibility::EvmCompatibleEvent {
    crate::evm_compatibility::EvmCompatibleEvent {
        event_type,
        indexed_fields: BoundedVec::new(),
        data_fields: BoundedVec::new(),
        raw_data: BoundedVec::new(),
    }
}

/// Store one EVM record at one block.
fn store_evm(block: u32, event_type: u32) {
    crate::EvmCompatibleEvents::<Test>::mutate(block, |events| {
        let _ = events.try_push(evm_record(event_type));
    });
}

/// Evidence of misbehaviour within the evidence bound.
fn evidence() -> BoundedVec<u8, ConstU32<1000>> {
    BoundedVec::truncate_from(vec![7u8; 4])
}

// ---------------------------------------------------------------------------
// Inference contribution
// ---------------------------------------------------------------------------

#[test]
fn inference_contribution_reads_zero_without_a_result() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::inference_contribution(&1), 0);
    });
}

#[test]
fn inference_contribution_keeps_a_result_inside_the_score_ceiling() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (42u32, 0u32));
        assert_eq!(DcfPallet::inference_contribution(&1), 42);
    });
}

#[test]
fn inference_contribution_holds_a_large_result_at_the_highest_score() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (500u32, 0u32));
        assert_eq!(DcfPallet::inference_contribution(&1), 100);
        assert_eq!(PalletCeruleaPoi::contribution_faults(1u64), 0);
    });
}

#[test]
fn inference_contribution_holds_a_result_above_the_share_ceiling() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (u32::MAX, 0u32));
        assert_eq!(DcfPallet::inference_contribution(&1), 100);
        assert_eq!(
            PalletCeruleaPoi::contribution_faults(1u64),
            1,
            "The inference module records one fault for a result above the ceiling."
        );
    });
}

// ---------------------------------------------------------------------------
// Final score composition
// ---------------------------------------------------------------------------

#[test]
fn final_score_composes_the_stake_share_and_the_inference_share() {
    new_test_ext().execute_with(|| {
        System::set_block_number(2);
        crate::CurrentEpoch::<Test>::put(4u32);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (50u32, 0u32));

        assert_ok!(update_final_score(1));

        // Stake score one at a share of 6000 and inference fifty at 4000.
        let state = state_of(1);
        assert_eq!(state.current.stake_score, 1);
        assert_eq!(state.current.inference_score, 50);
        assert_eq!(state.current.final_score, 20);
        assert_eq!(state.last_active_epoch, 4);
        assert!(has_event(crate::Event::ValidatorScoreUpdated {
            validator: 1,
            stake_score: 1,
            inference_score: 50,
            final_score: 20,
        }));

        // The update reaches the EVM store as a score update record.
        let records = DcfPallet::query_evm_events_internal(Some(9), 2, 2);
        assert_eq!(records.len(), 1);
    });
}

#[test]
fn final_score_detailed_event_reports_the_previous_inference_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(2);
        crate::CurrentEpoch::<Test>::put(4u32);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (50u32, 0u32));

        assert_ok!(update_final_score(1));

        // Genesis recorded an inference score of 6000 and a final score of 2400.
        assert!(
            has_event(crate::Event::ValidatorScoreUpdatedDetailed {
                validator: 1,
                old_stake_score: 1,
                new_stake_score: 1,
                old_inference_score: 6000,
                new_inference_score: 50,
                old_final_score: 2400,
                new_final_score: 20,
                epoch: 4,
            }),
            "The detailed event carries the inference score held before the update."
        );
    });
}

#[test]
fn final_score_holds_at_the_highest_score() {
    new_test_ext().execute_with(|| {
        edit_state(1, |state| state.current.stake_score = 1000);
        assert_ok!(update_final_score(1));
        assert_eq!(state_of(1).current.final_score, 100);
    });
}

#[test]
fn final_score_reads_the_stored_shares() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::put(10_000u64);
        crate::PoiWeight::<Test>::put(0u64);
        edit_state(1, |state| state.current.stake_score = 77);

        assert_ok!(update_final_score(1));
        assert_eq!(state_of(1).current.final_score, 77);
    });
}

#[test]
fn final_score_restores_the_default_shares_when_storage_holds_none() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::kill();
        crate::PoiWeight::<Test>::kill();
        assert!(!crate::PosWeight::<Test>::exists());

        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (100u32, 0u32));
        assert_ok!(update_final_score(1));

        assert!(crate::PosWeight::<Test>::exists());
        assert!(crate::PoiWeight::<Test>::exists());
        assert_eq!(crate::PosWeight::<Test>::get(), 6000);
        assert_eq!(crate::PoiWeight::<Test>::get(), 4000);
        // Stake one at 6000 and inference one hundred at 4000 gives forty.
        assert_eq!(state_of(1).current.final_score, 40);
    });
}

#[test]
fn final_score_of_an_unknown_validator_fails_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            update_final_score(99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn final_score_update_adds_one_history_entry() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (50u32, 0u32));
        assert_eq!(state_of(1).history.len(), 1);

        assert_ok!(update_final_score(1));

        let history = state_of(1).history;
        assert_eq!(
            history.len(),
            2,
            "The history keeps the genesis entry and adds the new entry below the bound."
        );
        assert_eq!(history[0].final_score, 2400);
        assert_eq!(history[1].final_score, 20);
    });
}

#[test]
fn final_score_update_drops_the_oldest_entry_at_the_history_bound() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(6u32);
        edit_state(1, |state| {
            state.history = BoundedVec::truncate_from(
                (100u32..110)
                    .map(|epoch| crate::EpochStats {
                        epoch,
                        ..Default::default()
                    })
                    .collect::<Vec<_>>(),
            );
        });

        assert_ok!(update_final_score(1));

        let history = state_of(1).history;
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].epoch, 101);
        assert_eq!(history[9].epoch, 6);
    });
}

// ---------------------------------------------------------------------------
// Score decay
// ---------------------------------------------------------------------------

#[test]
fn decay_leaves_a_validator_active_in_the_epoch_untouched() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::apply_score_decay(&1, 0));

        // A record newer than the epoch given also counts as active.
        edit_state(2, |state| state.last_active_epoch = 5);
        assert_ok!(DcfPallet::apply_score_decay(&2, 2));

        assert_eq!(state_of(1).current.final_score, 2400);
        assert_eq!(state_of(2).current.final_score, 2800);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorScoreDecayed { .. })));
    });
}

#[test]
fn decay_reports_the_score_of_an_idle_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        set_final_score(1, 80);

        assert_ok!(DcfPallet::apply_score_decay(&1, 3));

        // A rate of ten in ten thousand removes nothing from eighty.
        assert!(has_event(crate::Event::ValidatorScoreDecayed {
            validator: 1,
            old_score: 80,
            new_score: 80,
        }));
        assert!(DcfPallet::is_validator_active(&1));
    });
}

#[test]
fn decay_holds_a_score_above_the_ceiling_at_the_highest_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::apply_score_decay(&1, 3));

        assert_eq!(state_of(1).current.final_score, 100);
        assert!(has_event(crate::Event::ValidatorScoreDecayed {
            validator: 1,
            old_score: 2400,
            new_score: 100,
        }));
    });
}

#[test]
fn decay_ejects_a_validator_below_the_minimum_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        set_final_score(1, 49);

        assert_ok!(DcfPallet::apply_score_decay(&1, 2));

        assert!(!DcfPallet::is_validator_active(&1));
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 1,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
        assert_eq!(state_of(1).current.final_score, 49);
    });
}

#[test]
fn decay_of_an_unknown_validator_fails_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::apply_score_decay(&99, 3),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

// ---------------------------------------------------------------------------
// Block authorship and score boost
// ---------------------------------------------------------------------------

#[test]
fn block_authorship_counts_the_block_and_boosts_the_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        crate::CurrentEpoch::<Test>::put(2u32);
        set_final_score(1, 40);

        assert_ok!(DcfPallet::record_block_authorship(&1));

        let state = state_of(1);
        assert_eq!(state.current.authored_blocks, 1);
        assert_eq!(state.last_active_block, 7);
        assert_eq!(state.last_active_epoch, 2);
        assert_eq!(state.current.final_score, 50);
        assert_eq!(crate::ValidatorBlocksAuthored::<Test>::get(&1), 1);
        assert!(has_event(crate::Event::ValidatorScoreBoosted {
            validator: 1,
            old_score: 40,
            new_score: 50,
            reason: crate::ScoreBoostReason::ValidBlockAuthored,
        }));
        // The Standing Engine stores the same Trust Score in both records.
        assert_eq!(
            crate::ValidatorTrustScores::<Test>::get(1u64),
            state.trust_score
        );
    });
}

#[test]
fn block_authorship_boost_holds_at_the_highest_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        set_final_score(1, 95);

        assert_ok!(DcfPallet::record_block_authorship(&1));

        assert_eq!(state_of(1).current.final_score, 100);
        assert!(has_event(crate::Event::ValidatorScoreBoosted {
            validator: 1,
            old_score: 95,
            new_score: 100,
            reason: crate::ScoreBoostReason::ValidBlockAuthored,
        }));
    });
}

#[test]
fn block_authorship_of_an_unknown_validator_fails_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        assert_noop!(
            DcfPallet::record_block_authorship(&99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn block_authorship_adds_one_history_entry() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        set_final_score(1, 40);
        assert_eq!(state_of(1).history.len(), 1);

        assert_ok!(DcfPallet::record_block_authorship(&1));

        let history = state_of(1).history;
        assert_eq!(
            history.len(),
            2,
            "The boost keeps the genesis entry and adds the new entry below the bound."
        );
        assert_eq!(history[1].final_score, 50);
    });
}

#[test]
fn block_authorship_by_a_newly_admitted_validator_succeeds() {
    new_test_ext().execute_with(|| {
        System::set_block_number(7);
        assert_ok!(mock_join_validators(RuntimeOrigin::signed(10), None));
        assert!(state_of(10).history.is_empty());

        // The admitted validator holds an empty history. The boost records the
        // first entry.
        assert_ok!(DcfPallet::record_block_authorship(&10));

        let state = state_of(10);
        assert_eq!(state.current.authored_blocks, 1);
        assert_eq!(state.history.len(), 1);
    });
}

// ---------------------------------------------------------------------------
// Block reports
// ---------------------------------------------------------------------------

#[test]
fn missed_blocks_accumulate_on_the_state_record() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::record_missed_block(&2));
        assert_ok!(DcfPallet::record_missed_block(&2));
        assert_eq!(state_of(2).current.missed_blocks, 2);
        assert_eq!(state_of(1).current.missed_blocks, 0);
    });
}

#[test]
fn missed_block_of_an_unknown_validator_fails_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::record_missed_block(&99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

#[test]
fn authorship_report_from_an_inactive_author_fails_with_an_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_err!(
            DcfPallet::report_successful_block_authorship(5, 99),
            DispatchError::Other("Author is not an active validator")
        );
        assert!(has_event(crate::Event::InvalidAuthor {
            block_number: 5,
            author: 99,
        }));
    });
}

#[test]
fn authorship_report_from_an_active_author_records_the_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_ok!(DcfPallet::report_successful_block_authorship(5, 1));
        assert!(has_event(crate::Event::BlockAuthored {
            block_number: 5,
            author: 1,
        }));
        assert_eq!(state_of(1).current.authored_blocks, 1);
    });
}

#[test]
fn authorship_report_from_an_active_author_without_a_record_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        set_active(&[1, 2, 3, 40]);

        assert_err!(
            DcfPallet::report_successful_block_authorship(5, 40),
            crate::Error::<Test>::ValidatorNotFound
        );
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::BlockAuthored { .. })));
    });
}

#[test]
fn author_mismatch_charges_the_expected_author_with_a_missed_block() {
    new_test_ext().execute_with(|| {
        System::set_block_number(9);
        assert_ok!(DcfPallet::report_author_mismatch(9, Some(2), 3));
        assert!(has_event(crate::Event::AuthorMismatch {
            block_number: 9,
            expected: Some(2),
            actual: 3,
        }));
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(3).current.missed_blocks, 0);
    });
}

#[test]
fn author_mismatch_without_an_expected_author_only_reports() {
    new_test_ext().execute_with(|| {
        System::set_block_number(9);
        assert_ok!(DcfPallet::report_author_mismatch(9, None, 3));
        assert!(has_event(crate::Event::AuthorMismatch {
            block_number: 9,
            expected: None,
            actual: 3,
        }));
        for validator in [1u64, 2, 3] {
            assert_eq!(state_of(validator).current.missed_blocks, 0);
        }
    });
}

#[test]
fn author_mismatch_naming_an_unknown_expected_author_fails_after_reporting() {
    new_test_ext().execute_with(|| {
        System::set_block_number(9);
        assert_err!(
            DcfPallet::report_author_mismatch(9, Some(99), 3),
            crate::Error::<Test>::ValidatorNotFound
        );
        // The mismatch event leaves before the missed block is charged.
        assert!(has_event(crate::Event::AuthorMismatch {
            block_number: 9,
            expected: Some(99),
            actual: 3,
        }));
    });
}

#[test]
fn missed_block_report_records_the_block_and_the_event() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_ok!(DcfPallet::report_missed_block_for_api(4, 2));
        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert!(has_event(crate::Event::MissedBlock {
            block_number: 4,
            validator: 2,
        }));
    });
}

#[test]
fn missed_block_report_for_an_unknown_validator_fails_and_writes_nothing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_noop!(
            DcfPallet::report_missed_block_for_api(4, 99),
            crate::Error::<Test>::ValidatorNotFound
        );
    });
}

// ---------------------------------------------------------------------------
// Ejection
// ---------------------------------------------------------------------------

#[test]
fn ejection_removes_an_active_validator_and_reaches_the_evm_store() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_ok!(eject(2));

        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 3]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 2,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));

        // The EVM store indexes the ejection under type five with the account.
        let records = DcfPallet::query_evm_events_internal(Some(5), 4, 4);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].indexed_fields[0].to_vec(), 2u64.encode());
    });
}

#[test]
fn ejection_reports_the_active_status_the_validator_held() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_ok!(eject(2));

        assert!(
            has_event(crate::Event::ValidatorStatusChanged {
                validator: 2,
                old_status: crate::ValidatorStatus::Active,
                new_status: crate::ValidatorStatus::Ejected,
                block_number: 4,
            }),
            "The status change names the status held before the ejection."
        );
    });
}

#[test]
fn ejection_of_an_inactive_account_reports_the_inactive_status() {
    new_test_ext().execute_with(|| {
        System::set_block_number(4);
        assert_ok!(eject(99));

        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 2, 3]);
        assert!(has_event(crate::Event::ValidatorStatusChanged {
            validator: 99,
            old_status: crate::ValidatorStatus::Inactive,
            new_status: crate::ValidatorStatus::Ejected,
            block_number: 4,
        }));
    });
}

// ---------------------------------------------------------------------------
// Slash amount through the misbehaviour report
// ---------------------------------------------------------------------------

#[test]
fn third_misbehaviour_report_slashes_ten_percent_of_the_free_balance() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert_ok!(mock_join_validators(RuntimeOrigin::signed(10), None));

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(1),
            3,
            evidence()
        ));
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            3,
            evidence()
        ));
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorAutoSlashed { .. })));

        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(10),
            3,
            evidence()
        ));

        // Validator three holds 9000 free after genesis reserved 1000.
        assert!(has_event(crate::Event::ValidatorAutoSlashed {
            validator: 3,
            amount: 900,
            report_count: 3,
        }));
        assert_eq!(DcfPallet::get_misbehavior_report_count(&3), 0);
    });
}

#[test]
fn misbehaviour_slash_without_a_state_record_fails() {
    new_test_ext().execute_with(|| {
        System::set_block_number(3);
        assert_ok!(mock_join_validators(RuntimeOrigin::signed(10), None));
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(1),
            3,
            evidence()
        ));
        assert_ok!(DcfPallet::report_validator_misbehavior(
            RuntimeOrigin::signed(2),
            3,
            evidence()
        ));
        crate::ValidatorStates::<Test>::remove(&3);

        assert_err!(
            DcfPallet::report_validator_misbehavior(RuntimeOrigin::signed(10), 3, evidence()),
            crate::Error::<Test>::ValidatorNotFound
        );
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorAutoSlashed { .. })));
    });
}

// ---------------------------------------------------------------------------
// Economic invariants
// ---------------------------------------------------------------------------

#[test]
fn economic_invariants_hold_at_genesis() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::check_economic_invariants().is_empty());
    });
}

#[test]
fn economic_invariants_flag_reserves_below_the_estimated_slash() {
    new_test_ext().execute_with(|| {
        // A final score of zero estimates one hundred units of loss at the
        // existential deposit of 500.
        set_final_score(1, 0);

        let violations = DcfPallet::check_economic_invariants();
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Economic {
                violation_type: crate::EconomicViolationType::ReservedLessThanSlashed {
                    total_reserved: 3000,
                    total_slashed: 50000,
                },
                context,
            } if context.to_vec() == b"Reserved: 3000, Slashed: 50000".to_vec()
        ));
    });
}

#[test]
fn economic_invariants_flag_an_active_validator_without_a_reserve() {
    new_test_ext().execute_with(|| {
        let _ = <Balances as ReservableCurrency<u64>>::unreserve(&1, 1000);

        let violations = DcfPallet::check_economic_invariants();
        assert_eq!(violations.len(), 2);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Economic {
                violation_type: crate::EconomicViolationType::NegativeReservedBalance {
                    validator: 1,
                    balance: 0,
                },
                context,
            } if context.to_vec() == b"Validator: 1".to_vec()
        ));
        assert!(matches!(
            &violations[1],
            crate::InvariantViolation::Economic {
                violation_type: crate::EconomicViolationType::StakeBelowMinimum {
                    validator: 1,
                    current_stake: 0,
                    minimum_required: 1000,
                },
                context,
            } if context.to_vec() == b"Validator: 1, Stake: 0, Min: 1000".to_vec()
        ));
    });
}

#[test]
fn economic_invariants_skip_an_inactive_validator_without_a_reserve() {
    new_test_ext().execute_with(|| {
        let _ = <Balances as ReservableCurrency<u64>>::unreserve(&3, 1000);
        set_active(&[1, 2]);
        assert!(DcfPallet::check_economic_invariants().is_empty());
    });
}

// ---------------------------------------------------------------------------
// Validator set invariants
// ---------------------------------------------------------------------------

#[test]
fn validator_invariants_hold_at_genesis() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::check_validator_invariants().is_empty());
    });
}

#[test]
fn validator_invariants_flag_an_active_validator_with_a_leave_request() {
    new_test_ext().execute_with(|| {
        crate::ValidatorLeaveRequests::<Test>::insert(&2, 5);

        let violations = DcfPallet::check_validator_invariants();
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Validator {
                violation_type: crate::ValidatorViolationType::ActiveAndInCooldown { validator: 2 },
                context,
            } if context.to_vec() == b"Validator: 2".to_vec()
        ));
    });
}

#[test]
fn validator_invariants_flag_an_active_validator_in_removal_cooldown() {
    new_test_ext().execute_with(|| {
        crate::RecentlyRemovedValidators::<Test>::insert(&3, 1);

        let violations = DcfPallet::check_validator_invariants();
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Validator {
                violation_type: crate::ValidatorViolationType::ActiveAndInCooldown { validator: 3 },
                ..
            }
        ));
    });
}

#[test]
fn validator_invariants_flag_a_duplicate_in_the_active_set() {
    new_test_ext().execute_with(|| {
        set_active(&[1, 2, 3, 2]);

        let violations = DcfPallet::check_validator_invariants();
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Validator {
                violation_type: crate::ValidatorViolationType::DuplicateInActiveSet { validator: 2 },
                context,
            } if context.to_vec() == b"Duplicate validator: 2".to_vec()
        ));
    });
}

// ---------------------------------------------------------------------------
// Temporal invariants
// ---------------------------------------------------------------------------

#[test]
fn temporal_invariants_flag_an_epoch_regression() {
    new_test_ext().execute_with(|| {
        let violations = DcfPallet::check_temporal_invariants(7, 3);
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            crate::InvariantViolation::Temporal {
                violation_type: crate::TemporalViolationType::EpochRegression {
                    previous_epoch: 7,
                    current_epoch: 3,
                },
                context,
            } if context.to_vec() == b"Previous: 7, Current: 3".to_vec()
        ));
    });
}

#[test]
fn temporal_invariants_accept_a_steady_or_forward_epoch() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::check_temporal_invariants(3, 7).is_empty());
        assert!(DcfPallet::check_temporal_invariants(5, 5).is_empty());
    });
}

// ---------------------------------------------------------------------------
// Invariant report at the epoch boundary
// ---------------------------------------------------------------------------

#[test]
fn boundary_report_of_a_clean_chain_is_low_and_stored() {
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        let report = DcfPallet::check_invariants_at_epoch_boundary(3);

        assert_eq!(report.epoch, 3);
        assert_eq!(report.block_number, 12);
        assert_eq!(report.timestamp, 0);
        assert!(report.violations.is_empty());
        assert_eq!(report.severity, crate::InvariantSeverity::Low);
        assert_eq!(
            crate::InvariantReports::<Test>::get(3u32),
            Some(report.clone())
        );
        assert_eq!(crate::LatestInvariantReport::<Test>::get(), Some(report));
        assert!(has_event(crate::Event::InvariantReportGenerated {
            epoch: 3,
            block_number: 12,
            violations_count: 0,
            severity: crate::InvariantSeverity::Low,
        }));
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::InvariantViolationsDetected { .. })));
    });
}

#[test]
fn boundary_report_with_an_economic_violation_alone_is_high() {
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        set_final_score(1, 0);

        let report = DcfPallet::check_invariants_at_epoch_boundary(3);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.severity, crate::InvariantSeverity::High);
    });
}

#[test]
fn boundary_report_with_a_validator_violation_alone_is_medium() {
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        crate::ValidatorLeaveRequests::<Test>::insert(&2, 5);

        let report = DcfPallet::check_invariants_at_epoch_boundary(3);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.severity, crate::InvariantSeverity::Medium);
    });
}

#[test]
fn boundary_report_with_economic_and_validator_violations_is_high() {
    new_test_ext().execute_with(|| {
        System::set_block_number(12);
        set_final_score(1, 0);
        crate::ValidatorLeaveRequests::<Test>::insert(&2, 5);

        let report = DcfPallet::check_invariants_at_epoch_boundary(3);
        assert_eq!(report.violations.len(), 2);
        assert_eq!(report.severity, crate::InvariantSeverity::High);
        assert!(has_event(crate::Event::InvariantViolationsDetected {
            epoch: 3,
            violations: report.violations.clone(),
            severity: crate::InvariantSeverity::High,
        }));
        assert!(has_event(crate::Event::InvariantReportGenerated {
            epoch: 3,
            block_number: 12,
            violations_count: 2,
            severity: crate::InvariantSeverity::High,
        }));
    });
}

// ---------------------------------------------------------------------------
// Epoch transition
// ---------------------------------------------------------------------------

#[test]
fn epoch_transition_advances_the_epoch_and_reports_the_set() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        DcfPallet::handle_epoch_transition();

        assert_eq!(DcfPallet::current_epoch(), 1);
        assert!(has_event(crate::Event::EpochStarted {
            epoch: 1,
            validators: vec![1, 2, 3],
        }));
        // The average of 2400, 2800 and 3200 is 2800.
        assert!(dcf_events().iter().any(|event| matches!(
            event,
            crate::Event::EpochTransition {
                old_epoch: 0,
                new_epoch: 1,
                total_validators: 3,
                average_performance_score: 2800,
                removed_validators,
                added_validators,
                ..
            } if removed_validators.is_empty() && added_validators.is_empty()
        )));
        assert!(crate::InvariantReports::<Test>::get(1u32).is_some());
    });
}

#[test]
fn epoch_transition_reports_the_uptime_of_each_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        edit_state(2, |state| {
            state.current.authored_blocks = 3;
            state.current.missed_blocks = 1;
        });

        DcfPallet::handle_epoch_transition();

        assert!(has_event(crate::Event::ValidatorUptimeUpdated {
            validator: 2,
            epoch: 0,
            blocks_expected: 4,
            blocks_authored: 3,
            blocks_missed: 1,
            uptime_percentage: 7500,
        }));
        assert!(has_event(crate::Event::ValidatorUptimeUpdated {
            validator: 1,
            epoch: 0,
            blocks_expected: 0,
            blocks_authored: 0,
            blocks_missed: 0,
            uptime_percentage: 10000,
        }));
    });
}

#[test]
fn epoch_transition_records_scores_and_inference_results_in_the_history() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (42u32, 0u32));

        DcfPallet::handle_epoch_transition();

        let histories = crate::EpochHistories::<Test>::get();
        let latest = histories
            .last()
            .expect("The transition records one history entry.");
        assert_eq!(latest.epoch_number, 1);
        assert_eq!(latest.active_validators.to_vec(), vec![1, 2, 3]);
        assert_eq!(
            latest.score_snapshot.to_vec(),
            vec![(1, 2400), (2, 2800), (3, 3200)]
        );
        assert_eq!(
            latest.inference_summary.to_vec(),
            vec![(1, Some(42)), (2, None), (3, None)]
        );
    });
}

#[test]
fn epoch_transition_drops_the_oldest_history_at_the_bound() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        put_histories(
            (100u32..124)
                .map(|epoch| history(epoch, &[1, 2, 3]))
                .collect(),
        );

        DcfPallet::handle_epoch_transition();

        let histories = crate::EpochHistories::<Test>::get();
        assert_eq!(histories.len(), 24);
        assert_eq!(histories[0].epoch_number, 101);
        assert_eq!(histories[23].epoch_number, 1);
    });
}

#[test]
fn epoch_transition_with_an_empty_active_set_reports_a_zero_average() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        set_active(&[]);

        DcfPallet::handle_epoch_transition();

        assert!(has_event(crate::Event::EpochStarted {
            epoch: 1,
            validators: vec![],
        }));
        assert!(dcf_events().iter().any(|event| matches!(
            event,
            crate::Event::EpochTransition {
                average_performance_score: 0,
                ..
            }
        )));
    });
}

#[test]
fn epoch_transition_runs_in_governance_mode() {
    new_test_ext().execute_with(|| {
        System::set_block_number(30);
        crate::GovernanceModeEnabled::<Test>::put(true);

        DcfPallet::handle_epoch_transition();

        assert_eq!(DcfPallet::current_epoch(), 1);
        assert!(crate::GovernanceModeEnabled::<Test>::get());
        assert!(has_event(crate::Event::EpochStarted {
            epoch: 1,
            validators: vec![1, 2, 3],
        }));
    });
}

// ---------------------------------------------------------------------------
// Deterministic randomness
// ---------------------------------------------------------------------------

#[test]
fn deterministic_randomness_hashes_every_input_with_the_parent_hash() {
    new_test_ext().execute_with(|| {
        let salt = [1u8; 32];
        let epoch_salt = [2u8; 16];
        frame_system::BlockHash::<Test>::insert(4u64, sp_core::H256::repeat_byte(7));

        let mut input = Vec::new();
        input.extend_from_slice(&5u32.to_le_bytes());
        input.extend_from_slice(&3u32.to_le_bytes());
        input.extend_from_slice(&salt);
        input.extend_from_slice(&epoch_salt);
        input.extend_from_slice(&[7u8; 32]);

        assert_eq!(
            DcfPallet::generate_deterministic_randomness(5, 3, &salt, &epoch_salt),
            sp_io::hashing::blake2_256(&input)
        );
    });
}

#[test]
fn deterministic_randomness_moves_with_each_input() {
    new_test_ext().execute_with(|| {
        let salt = [1u8; 32];
        let epoch_salt = [2u8; 16];
        let base = DcfPallet::generate_deterministic_randomness(5, 3, &salt, &epoch_salt);

        assert_eq!(
            base,
            DcfPallet::generate_deterministic_randomness(5, 3, &salt, &epoch_salt)
        );
        assert_ne!(
            base,
            DcfPallet::generate_deterministic_randomness(6, 3, &salt, &epoch_salt)
        );
        assert_ne!(
            base,
            DcfPallet::generate_deterministic_randomness(5, 4, &salt, &epoch_salt)
        );
        assert_ne!(
            base,
            DcfPallet::generate_deterministic_randomness(5, 3, &[9u8; 32], &epoch_salt)
        );
        assert_ne!(
            base,
            DcfPallet::generate_deterministic_randomness(5, 3, &salt, &[9u8; 16])
        );
    });
}

#[test]
fn deterministic_randomness_moves_with_the_parent_block_hash() {
    new_test_ext().execute_with(|| {
        let salt = [1u8; 32];
        let epoch_salt = [2u8; 16];
        let before = DcfPallet::generate_deterministic_randomness(5, 3, &salt, &epoch_salt);

        frame_system::BlockHash::<Test>::insert(4u64, sp_core::H256::repeat_byte(8));

        let after = DcfPallet::generate_deterministic_randomness(5, 3, &salt, &epoch_salt);
        assert_ne!(before, after);
    });
}

// ---------------------------------------------------------------------------
// Deterministic author sequence
// ---------------------------------------------------------------------------

#[test]
fn author_sequence_of_an_empty_set_is_empty() {
    new_test_ext().execute_with(|| {
        let empty: ValidatorList = BoundedVec::new();
        assert!(
            DcfPallet::generate_deterministic_author_sequence(1, &empty, &[4u8; 32]).is_empty()
        );
    });
}

#[test]
fn author_sequence_spans_one_epoch_of_active_members() {
    new_test_ext().execute_with(|| {
        let active = DcfPallet::active_validators();
        let sequence = DcfPallet::generate_deterministic_author_sequence(1, &active, &[4u8; 32]);

        assert_eq!(sequence.len(), 10);
        assert!(sequence.iter().all(|author| active.contains(author)));
        assert_eq!(
            sequence,
            DcfPallet::generate_deterministic_author_sequence(1, &active, &[4u8; 32])
        );
    });
}

#[test]
fn author_sequence_ignores_the_order_of_the_input_set() {
    new_test_ext().execute_with(|| {
        let ordered = list_of(&[1, 2, 3]);
        let shuffled = list_of(&[3, 1, 2]);
        assert_eq!(
            DcfPallet::generate_deterministic_author_sequence(1, &ordered, &[4u8; 32]),
            DcfPallet::generate_deterministic_author_sequence(1, &shuffled, &[4u8; 32])
        );
    });
}

#[test]
fn author_sequence_holds_at_one_thousand_slots() {
    new_test_ext().execute_with(|| {
        crate::EpochConfigStorage::<Test>::put(crate::EpochConfig {
            blocks_per_epoch: 2000,
            min_stake: 1000,
            max_validators: 100,
        });
        let active = DcfPallet::active_validators();

        let first = DcfPallet::generate_deterministic_author_sequence(1, &active, &[4u8; 32]);
        let second = DcfPallet::generate_deterministic_author_sequence(1, &active, &[5u8; 32]);

        assert_eq!(first.len(), 1000);
        assert_ne!(first, second, "Two seeds draw two different sequences.");
    });
}

#[test]
fn author_sequence_follows_the_snapshot_weights() {
    new_test_ext().execute_with(|| {
        hold_snapshot(3, vec![(1, 1_000_000_000), (2, 1), (3, 1)]);
        let active = DcfPallet::active_validators();

        let sequence = DcfPallet::generate_deterministic_author_sequence(3, &active, &[4u8; 32]);
        assert!(
            sequence.iter().all(|author| *author == 1),
            "The dominant snapshot weight wins every slot."
        );
    });
}

#[test]
fn author_sequence_floors_absent_and_zero_snapshot_weights_at_one() {
    new_test_ext().execute_with(|| {
        let active = DcfPallet::active_validators();
        let seed = [4u8; 32];
        let equal = DcfPallet::generate_deterministic_author_sequence(3, &active, &seed);

        hold_snapshot(3, vec![(1, 0), (2, 0), (3, 0)]);
        assert_eq!(
            DcfPallet::generate_deterministic_author_sequence(3, &active, &seed),
            equal
        );

        hold_snapshot(3, vec![(77, 5)]);
        assert_eq!(
            DcfPallet::generate_deterministic_author_sequence(3, &active, &seed),
            equal
        );
    });
}

// ---------------------------------------------------------------------------
// Deterministic epoch transition
// ---------------------------------------------------------------------------

#[test]
fn deterministic_transition_records_every_input_and_output() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let genesis_hash = frame_system::Pallet::<Test>::block_hash(0u64);
        let active_before = DcfPallet::active_validators();

        DcfPallet::handle_deterministic_epoch_transition(100);

        let engine = crate::DeterministicEngineState::<Test>::get();

        // The network salt comes from the genesis hash and the storage version.
        let mut salt_input = genesis_hash.as_bytes().to_vec();
        salt_input.extend_from_slice(b"DCF_DETERMINISTIC_SALT_V1");
        salt_input.extend_from_slice(&crate::CURRENT_STORAGE_VERSION.to_le_bytes());
        assert_eq!(
            engine.randomness_salt,
            sp_io::hashing::blake2_256(&salt_input)
        );

        // The epoch salt is the first half of the epoch hash.
        let mut epoch_input = 1u32.to_le_bytes().to_vec();
        epoch_input.extend_from_slice(&engine.randomness_salt);
        epoch_input.extend_from_slice(b"EPOCH_SALT");
        let epoch_hash = sp_io::hashing::blake2_256(&epoch_input);
        assert_eq!(&engine.epoch_salt[..], &epoch_hash[0..16]);

        let seed = DcfPallet::generate_deterministic_randomness(
            100,
            1,
            &engine.randomness_salt,
            &engine.epoch_salt,
        );
        let output =
            crate::EpochProcessingOutputs::<Test>::get(1u32).expect("The record is stored.");
        assert_eq!(output.epoch, 1);
        assert_eq!(output.transition_block, 100);
        assert_eq!(output.randomness_seed, seed);

        // The input hash covers the old epoch, the block, the set and the shares.
        let mut input = 0u32.to_le_bytes().to_vec();
        input.extend_from_slice(&100u32.to_le_bytes());
        input.extend_from_slice(&active_before.encode());
        input.extend_from_slice(&seed);
        input.extend_from_slice(&6000u64.to_le_bytes());
        input.extend_from_slice(&4000u64.to_le_bytes());
        assert_eq!(output.input_hash, sp_io::hashing::blake2_256(&input));

        // The output hash covers every recorded output.
        let mut produced = 1u32.to_le_bytes().to_vec();
        produced.extend_from_slice(&output.active_validators.encode());
        produced.extend_from_slice(&output.added_validators.encode());
        produced.extend_from_slice(&output.removed_validators.encode());
        produced.extend_from_slice(&output.validator_scores.encode());
        produced.extend_from_slice(&output.author_sequence.encode());
        assert_eq!(output.output_hash, sp_io::hashing::blake2_256(&produced));

        assert_eq!(output.author_sequence.len(), 10);
        assert_eq!(
            crate::EpochAuthorSequences::<Test>::get(1u32),
            Some(output.author_sequence.clone())
        );
        assert_eq!(engine.last_processed_epoch, 1);
        assert_eq!(engine.last_epoch_output_hash, output.output_hash);
        assert_eq!(
            engine.author_sequence_cache.to_vec(),
            output.author_sequence.encode()
        );
        assert!(has_event(crate::Event::DeterministicEpochProcessed {
            epoch: 1,
            block_number: 100,
            randomness_seed: seed,
            author_sequence_length: 10,
            output_hash: output.output_hash,
        }));
    });
}

#[test]
fn deterministic_transition_keeps_a_salt_already_held() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        let mut engine = crate::DeterministicEngineState::<Test>::get();
        engine.randomness_salt = [5u8; 32];
        crate::DeterministicEngineState::<Test>::put(engine);

        DcfPallet::handle_deterministic_epoch_transition(100);

        let engine = crate::DeterministicEngineState::<Test>::get();
        assert_eq!(engine.randomness_salt, [5u8; 32]);
        let output =
            crate::EpochProcessingOutputs::<Test>::get(1u32).expect("The record is stored.");
        assert_eq!(
            output.randomness_seed,
            DcfPallet::generate_deterministic_randomness(100, 1, &[5u8; 32], &engine.epoch_salt)
        );
    });
}

#[test]
fn deterministic_transition_freezes_the_commit_weights_of_the_new_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        DcfPallet::handle_deterministic_epoch_transition(100);

        let frozen = frozen_epoch_weights();
        let (_, entries) = frozen
            .iter()
            .find(|(epoch, _)| *epoch == 1)
            .expect("The Commit Engine weights of epoch one are frozen.");
        assert_eq!(entries.len(), 3);
        for (index, (validator, stake, trust)) in entries.iter().enumerate() {
            assert_eq!(*validator, index as u64 + 1);
            assert_eq!(*stake, 1000);
            assert!(*trust >= 1);
        }
    });
}

#[test]
fn deterministic_transition_records_the_changes_into_the_new_epoch() {
    new_test_ext().execute_with(|| {
        System::set_block_number(600);
        crate::CurrentEpoch::<Test>::put(5u32);
        put_histories(vec![history(4, &[1, 2, 9]), history(5, &[1, 2, 9])]);

        DcfPallet::handle_deterministic_epoch_transition(600);

        // Epoch five held 1, 2 and 9. Epoch six holds 1, 2 and 3.
        let output =
            crate::EpochProcessingOutputs::<Test>::get(6u32).expect("The record is stored.");
        assert_eq!(output.added_validators.to_vec(), vec![3]);
        assert_eq!(output.removed_validators.to_vec(), vec![9]);
    });
}

#[test]
fn deterministic_transition_passes_its_own_replay() {
    new_test_ext().execute_with(|| {
        System::set_block_number(100);
        DcfPallet::handle_deterministic_epoch_transition(100);

        assert_eq!(
            DcfPallet::validate_epoch_replay(1),
            Ok(()),
            "A replay in the block of the transition reproduces the recorded output."
        );
    });
}

// ---------------------------------------------------------------------------
// Replay validation
// ---------------------------------------------------------------------------

#[test]
fn replay_of_an_unrecorded_epoch_reports_the_missing_output() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::validate_epoch_replay(8),
            Err(crate::ReplayValidationError::MissingOriginalOutput)
        );
    });
}

#[test]
fn replay_reports_both_hashes_on_an_output_mismatch() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));

        match DcfPallet::validate_epoch_replay(7) {
            Err(crate::ReplayValidationError::OutputMismatch {
                original_hash,
                replay_hash,
            }) => {
                assert_eq!(original_hash, [0u8; 32]);
                assert_ne!(replay_hash, [0u8; 32]);
            }
            other => panic!("The replay answered {:?}. A mismatch was expected.", other),
        }
    });
}

#[test]
fn replay_of_a_matching_record_passes() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));
        let replay_hash = replay_hash_for(7);

        crate::EpochProcessingOutputs::<Test>::insert(
            7u32,
            stored_output(7, [9u8; 32], replay_hash),
        );
        assert_eq!(DcfPallet::validate_epoch_replay(7), Ok(()));
    });
}

#[test]
fn replay_flags_a_validator_set_mismatch() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));
        let replay_hash = replay_hash_for(7);

        let mut output = stored_output(7, [9u8; 32], replay_hash);
        output.active_validators = list_of(&[1, 2]);
        crate::EpochProcessingOutputs::<Test>::insert(7u32, output);

        assert_eq!(
            DcfPallet::validate_epoch_replay(7),
            Err(crate::ReplayValidationError::ValidatorSetMismatch)
        );
    });
}

#[test]
fn replay_flags_an_author_sequence_mismatch() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));
        let replay_hash = replay_hash_for(7);

        let mut output = stored_output(7, [9u8; 32], replay_hash);
        output.author_sequence = BoundedVec::truncate_from(vec![99u64; 10]);
        crate::EpochProcessingOutputs::<Test>::insert(7u32, output);

        assert_eq!(
            DcfPallet::validate_epoch_replay(7),
            Err(crate::ReplayValidationError::AuthorSequenceMismatch)
        );
    });
}

#[test]
fn replay_flags_a_score_mismatch() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));
        let replay_hash = replay_hash_for(7);

        let mut output = stored_output(7, [9u8; 32], replay_hash);
        output.validator_scores = BoundedVec::truncate_from(vec![(1u64, 1u64), (2, 2), (3, 3)]);
        crate::EpochProcessingOutputs::<Test>::insert(7u32, output);

        assert_eq!(
            DcfPallet::validate_epoch_replay(7),
            Err(crate::ReplayValidationError::ScoreMismatch)
        );
    });
}

#[test]
fn replay_hash_reads_the_set_changes_from_the_epoch_history() {
    new_test_ext().execute_with(|| {
        crate::EpochProcessingOutputs::<Test>::insert(7u32, stored_output(7, [9u8; 32], [0u8; 32]));

        // No history names epoch seven, so the replay reads no change.
        let without_history = replay_hash_for(7);

        // Two equal sets read no change either.
        put_histories(vec![history(6, &[1, 2, 3]), history(7, &[1, 2, 3])]);
        assert_eq!(replay_hash_for(7), without_history);

        // Account three joined and account nine left.
        put_histories(vec![history(6, &[1, 2, 9]), history(7, &[1, 2, 3])]);
        let with_changes = replay_hash_for(7);
        assert_ne!(with_changes, without_history);

        // Without the previous epoch every member reads as joined.
        put_histories(vec![history(7, &[1, 2, 3])]);
        let all_joined = replay_hash_for(7);
        assert_ne!(all_joined, without_history);
        assert_ne!(all_joined, with_changes);
    });
}

// ---------------------------------------------------------------------------
// Block hooks
// ---------------------------------------------------------------------------

#[test]
fn hook_marks_block_one_hundred_as_an_epoch_boundary() {
    new_test_ext().execute_with(|| {
        run_hook(100);

        assert!(has_event(crate::Event::EpochBoundaryDetected {
            block_number: 100,
            epoch: 0,
            governance_mode: false,
        }));
        assert_eq!(DcfPallet::current_epoch(), 1);
        let sequence =
            crate::EpochAuthorSequences::<Test>::get(1u32).expect("The sequence is stored.");
        assert_eq!(sequence.len(), 10);
        let output =
            crate::EpochProcessingOutputs::<Test>::get(1u32).expect("The record is stored.");
        assert_eq!(output.transition_block, 100);
    });
}

#[test]
fn hook_aggregates_the_scores_at_the_boundary() {
    new_test_ext().execute_with(|| {
        run_hook(100);

        // Each validator holds a third of the stake, so the scaled stake score
        // is 32000 divided by three. The final score holds at the ceiling.
        for validator in [1u64, 2, 3] {
            let state = state_of(validator);
            assert_eq!(state.current.stake_score, 10666);
            assert_eq!(state.current.inference_score, 0);
            assert_eq!(state.current.final_score, 100);
        }
    });
}

#[test]
fn hook_reports_governance_mode_at_the_boundary() {
    new_test_ext().execute_with(|| {
        crate::GovernanceModeEnabled::<Test>::put(true);
        run_hook(100);

        assert!(has_event(crate::Event::EpochBoundaryDetected {
            block_number: 100,
            epoch: 0,
            governance_mode: true,
        }));
        assert_eq!(DcfPallet::current_epoch(), 1);
    });
}

#[test]
fn hook_closes_the_previous_epoch_at_the_second_boundary() {
    new_test_ext().execute_with(|| {
        run_hook(100);
        System::reset_events();
        run_hook(200);

        assert_eq!(DcfPallet::current_epoch(), 2);
        assert!(has_event(crate::Event::EpochEnded { epoch: 1 }));
        assert!(crate::EpochAuthorSequences::<Test>::get(2u32).is_some());
    });
}

#[test]
fn hook_treats_other_blocks_as_regular_blocks() {
    new_test_ext().execute_with(|| {
        run_hook(150);
        run_hook(0);

        assert_eq!(DcfPallet::current_epoch(), 0);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::EpochBoundaryDetected { .. })));
    });
}

#[test]
fn hook_snapshots_the_scores_five_blocks_before_the_boundary() {
    new_test_ext().execute_with(|| {
        run_hook(5);

        let snapshot =
            crate::EpochScoreSnapshot::<Test>::get(1u32).expect("The snapshot is stored.");
        assert_eq!(snapshot.to_vec(), vec![(1, 2400), (2, 2800), (3, 3200)]);
    });
}

#[test]
fn hook_keeps_the_first_snapshot_of_an_epoch() {
    new_test_ext().execute_with(|| {
        run_hook(5);
        set_final_score(1, 7);
        run_hook(5);

        let snapshot =
            crate::EpochScoreSnapshot::<Test>::get(1u32).expect("The snapshot is stored.");
        assert_eq!(snapshot[0], (1, 2400));
    });
}

#[test]
fn hook_snapshot_floors_a_missing_or_zero_score_at_one() {
    new_test_ext().execute_with(|| {
        set_active(&[1, 2, 3, 40]);
        set_final_score(1, 0);
        run_hook(5);

        let snapshot =
            crate::EpochScoreSnapshot::<Test>::get(1u32).expect("The snapshot is stored.");
        assert_eq!(
            snapshot.to_vec(),
            vec![(1, 1), (2, 2800), (3, 3200), (40, 1)]
        );
    });
}

#[test]
fn hook_applies_score_decay_on_the_decay_interval() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(3u32);
        run_hook(10);

        assert!(has_event(crate::Event::ValidatorScoreDecayed {
            validator: 1,
            old_score: 2400,
            new_score: 100,
        }));
        assert!(has_event(crate::Event::ValidatorScoreDecayed {
            validator: 3,
            old_score: 3200,
            new_score: 100,
        }));

        System::reset_events();
        run_hook(11);
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorScoreDecayed { .. })));
    });
}

#[test]
fn hook_ejects_a_low_scoring_validator_on_the_check_interval() {
    new_test_ext().execute_with(|| {
        set_final_score(2, 10);
        run_hook(50);

        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 3]);
        assert!(has_event(crate::Event::ValidatorEjected {
            validator: 2,
            reason: crate::EjectionReason::ScoreBelowThreshold,
        }));
    });
}

#[test]
fn hook_ejects_a_validator_idle_past_the_inactive_limit() {
    new_test_ext().execute_with(|| {
        crate::CurrentEpoch::<Test>::put(9u32);
        edit_state(2, |state| state.last_active_epoch = 9);
        edit_state(3, |state| state.last_active_epoch = 9);
        run_hook(50);

        // Validator one last served in epoch zero, nine epochs ago.
        assert_eq!(DcfPallet::active_validators().to_vec(), vec![2, 3]);
    });
}

#[test]
fn hook_at_block_zero_reaches_every_interval_of_regular_processing() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        hold_author_sequence(0, 3);
        edit_state(1, |state| {
            state.current.authored_blocks = 3;
            state.current.missed_blocks = 1;
        });
        set_final_score(2, 10);

        <DcfPallet as Hooks<u64>>::on_initialize(0);

        // The expected author of block zero is validator three, with no seal.
        assert_eq!(state_of(3).current.missed_blocks, 1);
        // The participation interval recomputes each rate.
        assert_eq!(state_of(1).participation_rate, 7500);
        assert_eq!(state_of(3).participation_rate, 0);
        assert_eq!(state_of(2).participation_rate, 100);
        // The underperformance interval removes the low scorer.
        assert!(!DcfPallet::is_validator_active(&2));
    });
}

#[test]
fn hook_credits_the_expected_author_named_in_the_digest() {
    new_test_ext().execute_with(|| {
        hold_author_sequence(0, 2);
        let mut seal = 2u64.encode();
        seal.resize(64, 0);
        System::deposit_log(sp_runtime::DigestItem::PreRuntime(*b"ceru", seal));

        run_hook(3);

        assert_eq!(state_of(2).current.authored_blocks, 1);
        assert_eq!(state_of(2).current.missed_blocks, 0);
    });
}

#[test]
fn hook_credits_the_expected_author_named_in_a_consensus_digest() {
    new_test_ext().execute_with(|| {
        hold_author_sequence(0, 2);
        System::deposit_log(sp_runtime::DigestItem::Consensus(*b"ceru", 2u64.encode()));

        run_hook(3);

        assert_eq!(state_of(2).current.authored_blocks, 1);
    });
}

#[test]
fn hook_charges_the_expected_author_when_another_account_seals() {
    new_test_ext().execute_with(|| {
        hold_author_sequence(0, 2);
        let mut seal = 77u64.encode();
        seal.resize(64, 0);
        System::deposit_log(sp_runtime::DigestItem::Seal(*b"ceru", seal));

        run_hook(3);

        assert_eq!(state_of(2).current.missed_blocks, 1);
        assert_eq!(state_of(2).current.authored_blocks, 0);
        assert!(has_event(crate::Event::InvalidAuthor {
            block_number: 3,
            author: 77,
        }));
    });
}

#[test]
fn hook_charges_the_expected_author_when_no_seal_arrives() {
    new_test_ext().execute_with(|| {
        hold_author_sequence(0, 1);
        run_hook(3);

        assert_eq!(state_of(1).current.missed_blocks, 1);
        assert_eq!(state_of(2).current.missed_blocks, 0);
    });
}

#[test]
fn finalisation_hook_leaves_storage_untouched() {
    new_test_ext().execute_with(|| {
        run_hook(3);
        let before = sp_io::storage::root(sp_runtime::StateVersion::V1);
        <DcfPallet as Hooks<u64>>::on_finalize(3);
        let after = sp_io::storage::root(sp_runtime::StateVersion::V1);
        assert_eq!(before, after);
    });
}

// ---------------------------------------------------------------------------
// Storage migration and integrity
// ---------------------------------------------------------------------------

#[test]
fn migration_between_unknown_versions_reports_no_path() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::perform_storage_migration(1, 2),
            Err(crate::MigrationError::NoMigrationPath { from: 1, to: 2 })
        );
        assert_eq!(
            DcfPallet::perform_storage_migration(3, 1),
            Err(crate::MigrationError::NoMigrationPath { from: 3, to: 1 })
        );
    });
}

#[test]
fn initial_migration_keeps_a_valid_governance_record() {
    new_test_ext().execute_with(|| {
        hold_valid_governance();
        let held = crate::GovernanceConfigStorage::<Test>::get();

        // Base weight plus the integrity check with governance and epoch checks.
        assert_eq!(
            DcfPallet::perform_storage_migration(0, 1),
            Ok(Weight::from_parts(40_000, 0))
        );
        assert_eq!(crate::GovernanceConfigStorage::<Test>::get(), held);
    });
}

#[test]
fn initial_migration_installs_a_governance_record_when_none_exists() {
    new_test_ext().execute_with(|| {
        crate::GovernanceConfigStorage::<Test>::kill();

        assert_ok!(DcfPallet::perform_storage_migration(0, 1));
        assert!(crate::GovernanceConfigStorage::<Test>::exists());
        // The installed record is the starting record genesis writes, built
        // from the stored epoch settings.
        let epoch = crate::EpochConfigStorage::<Test>::get();
        assert_eq!(
            crate::GovernanceConfigStorage::<Test>::get(),
            DcfPallet::starting_governance_record(epoch.blocks_per_epoch, epoch.max_validators)
        );
        assert_ne!(
            crate::GovernanceConfigStorage::<Test>::get(),
            crate::GovernanceConfig::<Test>::default()
        );
    });
}

#[test]
fn initial_migration_leaves_storage_that_passes_the_integrity_check() {
    new_test_ext().execute_with(|| {
        crate::GovernanceConfigStorage::<Test>::kill();
        assert_ok!(DcfPallet::perform_storage_migration(0, 1));

        assert!(
            DcfPallet::validate_storage_integrity().is_ok(),
            "The governance record the migration installs passes the governance check."
        );
    });
}

#[test]
fn initial_migration_stops_on_a_failed_integrity_check() {
    new_test_ext().execute_with(|| {
        // A stored record whose epoch length sits outside its own range fails
        // the check before the migration changes anything.
        crate::GovernanceConfigStorage::<Test>::mutate(|held| {
            held.epoch_length.current = held.epoch_length.max + 1
        });
        assert_eq!(
            rule_of(DcfPallet::perform_storage_migration(0, 1)),
            b"epoch_length_range".to_vec()
        );
    });
}

#[test]
fn same_version_migration_runs_the_integrity_check() {
    new_test_ext().execute_with(|| {
        hold_valid_governance();
        assert_eq!(
            DcfPallet::perform_storage_migration(1, 1),
            Ok(Weight::from_parts(40_000, 0))
        );

        crate::EpochConfigStorage::<Test>::put(crate::EpochConfig {
            blocks_per_epoch: 0,
            min_stake: 1000,
            max_validators: 100,
        });
        assert_eq!(
            rule_of(DcfPallet::perform_storage_migration(1, 1)),
            b"epoch_length_zero".to_vec()
        );
    });
}

#[test]
fn storage_integrity_rejects_an_epoch_length_outside_its_range() {
    new_test_ext().execute_with(|| {
        // The genesis record passes; a held value under its floor fails.
        assert!(DcfPallet::validate_storage_integrity().is_ok());
        crate::GovernanceConfigStorage::<Test>::mutate(|held| {
            held.epoch_length.current = held.epoch_length.min - 1
        });
        assert_eq!(
            rule_of(DcfPallet::validate_storage_integrity()),
            b"epoch_length_range".to_vec()
        );
    });
}

#[test]
fn storage_integrity_weighs_each_check_it_runs() {
    new_test_ext().execute_with(|| {
        hold_valid_governance();
        assert_eq!(
            DcfPallet::validate_storage_integrity(),
            Ok(Weight::from_parts(30_000, 0))
        );

        crate::GovernanceConfigStorage::<Test>::kill();
        assert_eq!(
            DcfPallet::validate_storage_integrity(),
            Ok(Weight::from_parts(20_000, 0))
        );
    });
}

#[test]
fn storage_accessibility_reads_and_rewrites_the_version() {
    new_test_ext().execute_with(|| {
        crate::pallet::StorageVersion::<Test>::put(7u32);
        assert_eq!(
            DcfPallet::validate_storage_accessibility(),
            Ok(Weight::from_parts(10_000, 0))
        );
        assert_eq!(crate::pallet::StorageVersion::<Test>::get(), 7);
    });
}

#[test]
fn governance_check_accepts_a_record_inside_every_range() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::validate_governance_config(&governance_with(|_| {})),
            Ok(Weight::from_parts(10_000, 0))
        );
        // One share alone keeps the sum above zero.
        assert_ok!(DcfPallet::validate_governance_config(&governance_with(
            |settings| settings.pos_weight.current = 0
        )));
    });
}

#[test]
fn governance_check_rejects_each_broken_epoch_length_range() {
    new_test_ext().execute_with(|| {
        for range in [
            crate::ParameterRange {
                min: 200u32,
                max: 100,
                current: 150,
            },
            crate::ParameterRange {
                min: 50,
                max: 14400,
                current: 10,
            },
            crate::ParameterRange {
                min: 50,
                max: 14400,
                current: 20000,
            },
        ] {
            let settings = governance_with(|settings| settings.epoch_length = range);
            assert_eq!(
                rule_of(DcfPallet::validate_governance_config(&settings)),
                b"epoch_length_range".to_vec()
            );
        }
    });
}

#[test]
fn governance_check_rejects_each_broken_validator_count_range() {
    new_test_ext().execute_with(|| {
        for range in [
            crate::ParameterRange {
                min: 10u32,
                max: 5,
                current: 7,
            },
            crate::ParameterRange {
                min: 1,
                max: 1000,
                current: 0,
            },
            crate::ParameterRange {
                min: 1,
                max: 1000,
                current: 2000,
            },
        ] {
            let settings = governance_with(|settings| settings.max_validators = range);
            assert_eq!(
                rule_of(DcfPallet::validate_governance_config(&settings)),
                b"max_validators_range".to_vec()
            );
        }
    });
}

#[test]
fn governance_check_rejects_shares_that_sum_to_zero() {
    new_test_ext().execute_with(|| {
        let settings = governance_with(|settings| {
            settings.pos_weight.current = 0;
            settings.poi_weight.current = 0;
        });
        assert_eq!(
            rule_of(DcfPallet::validate_governance_config(&settings)),
            b"consensus_weights_sum".to_vec()
        );
    });
}

#[test]
fn epoch_check_accepts_the_genesis_epoch_settings() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::validate_epoch_config(&crate::EpochConfig {
                blocks_per_epoch: 10,
                min_stake: 1000,
                max_validators: 100,
            }),
            Ok(Weight::from_parts(10_000, 0))
        );
    });
}

#[test]
fn epoch_check_rejects_each_broken_epoch_setting() {
    new_test_ext().execute_with(|| {
        let cases: [(crate::EpochConfig, &[u8]); 3] = [
            (
                crate::EpochConfig {
                    blocks_per_epoch: 0,
                    min_stake: 1000,
                    max_validators: 100,
                },
                &b"epoch_length_zero"[..],
            ),
            (
                crate::EpochConfig {
                    blocks_per_epoch: 10,
                    min_stake: 1000,
                    max_validators: 0,
                },
                &b"max_validators_zero"[..],
            ),
            (
                crate::EpochConfig {
                    blocks_per_epoch: 10,
                    min_stake: 1000,
                    max_validators: 101,
                },
                &b"max_validators_exceeds_limit"[..],
            ),
        ];
        for (settings, rule) in cases {
            assert_eq!(
                rule_of(DcfPallet::validate_epoch_config(&settings)),
                rule.to_vec()
            );
        }
    });
}

// ---------------------------------------------------------------------------
// Cooldown status
// ---------------------------------------------------------------------------

#[test]
fn cooldown_status_is_absent_without_a_request_or_removal() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            None
        );
    });
}

#[test]
fn cooldown_status_counts_down_a_leave_request() {
    new_test_ext().execute_with(|| {
        crate::ValidatorLeaveRequests::<Test>::insert(&1, 10);

        System::set_block_number(110);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            Some((900, false))
        );

        // A leave request keeps the validator out after the window as well.
        System::set_block_number(5000);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            Some((0, false))
        );
    });
}

#[test]
fn cooldown_status_counts_down_a_recent_removal() {
    new_test_ext().execute_with(|| {
        crate::RecentlyRemovedValidators::<Test>::insert(&1, 10);

        System::set_block_number(110);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            Some((900, false))
        );

        System::set_block_number(1010);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            Some((0, true))
        );
    });
}

#[test]
fn cooldown_status_reads_the_leave_request_first() {
    new_test_ext().execute_with(|| {
        crate::ValidatorLeaveRequests::<Test>::insert(&1, 100);
        crate::RecentlyRemovedValidators::<Test>::insert(&1, 10);

        System::set_block_number(2000);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status_internal(&1),
            Some((0, false))
        );
    });
}

// ---------------------------------------------------------------------------
// Private chain wrappers
// ---------------------------------------------------------------------------

#[test]
fn private_mode_records_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            true
        ));

        assert!(DcfPallet::is_private_chain_mode());
        assert_eq!(DcfPallet::get_validator_allowlist(), vec![1, 2, 3]);
        assert_eq!(DcfPallet::active_validators().to_vec(), vec![1, 2, 3]);
        assert!(has_event(crate::Event::PrivateChainModeEnabled {
            allowlist_size: 3,
            allow_updates: true,
        }));
    });
}

#[test]
fn private_mode_refuses_an_allowlist_above_the_validator_limit() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            DcfPallet::enable_private_chain_mode_internal((1u64..=101).collect(), true),
            crate::Error::<Test>::TooManyValidators
        );
    });
}

#[test]
fn private_mode_releases_a_validator_outside_the_allowlist() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2],
            false
        ));

        assert!(!DcfPallet::validator_set().contains(&3));
        assert!(crate::ValidatorStates::<Test>::get(&3).is_none());
        assert_eq!(
            <Balances as ReservableCurrency<u64>>::reserved_balance(&3),
            0
        );
        assert!(has_event(crate::Event::ValidatorForcedToLeave {
            validator: 3,
            reason: b"Removed from private chain allowlist".to_vec(),
        }));
    });
}

#[test]
fn private_mode_takes_a_validator_outside_the_allowlist_out_of_the_active_set() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2],
            false
        ));
        assert!(
            !DcfPallet::is_validator_active(&3),
            "A validator outside the allowlist leaves the active set at once."
        );
    });
}

#[test]
fn private_mode_disable_needs_an_enabled_mode() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            DcfPallet::disable_private_chain_mode_internal(),
            crate::Error::<Test>::PrivateChainModeDisabled
        );

        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            true
        ));
        assert_ok!(DcfPallet::disable_private_chain_mode_internal());
        assert!(!DcfPallet::is_private_chain_mode());
        assert!(DcfPallet::get_validator_allowlist().is_empty());
        assert!(has_event(crate::Event::PrivateChainModeDisabled));

        assert_noop!(
            DcfPallet::disable_private_chain_mode_internal(),
            crate::Error::<Test>::PrivateChainModeDisabled
        );
    });
}

#[test]
fn allowlist_addition_follows_each_mode_rule() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            DcfPallet::add_to_validator_allowlist_internal(10),
            crate::Error::<Test>::PrivateChainModeDisabled
        );

        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            false
        ));
        assert_noop!(
            DcfPallet::add_to_validator_allowlist_internal(10),
            crate::Error::<Test>::AllowlistUpdatesDisabled
        );

        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            true
        ));
        assert_ok!(DcfPallet::add_to_validator_allowlist_internal(10));
        assert!(DcfPallet::get_validator_allowlist().contains(&10));
        assert!(has_event(crate::Event::ValidatorAddedToAllowlist {
            validator: 10
        }));

        assert_ok!(DcfPallet::disable_private_chain_mode_internal());
        assert_noop!(
            DcfPallet::add_to_validator_allowlist_internal(11),
            crate::Error::<Test>::PrivateChainModeDisabled
        );
    });
}

#[test]
fn allowlist_addition_refuses_a_full_allowlist() {
    new_test_ext().execute_with(|| {
        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            (1u64..=100).collect(),
            true
        ));
        assert_noop!(
            DcfPallet::add_to_validator_allowlist_internal(200),
            crate::Error::<Test>::AllowlistFull
        );
    });
}

#[test]
fn allowlist_removal_follows_each_mode_rule() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_noop!(
            DcfPallet::remove_from_validator_allowlist_internal(&3),
            crate::Error::<Test>::PrivateChainModeDisabled
        );

        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            false
        ));
        assert_noop!(
            DcfPallet::remove_from_validator_allowlist_internal(&3),
            crate::Error::<Test>::AllowlistUpdatesDisabled
        );

        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3],
            true
        ));
        assert_ok!(DcfPallet::disable_private_chain_mode_internal());
        assert_noop!(
            DcfPallet::remove_from_validator_allowlist_internal(&3),
            crate::Error::<Test>::PrivateChainModeDisabled
        );
    });
}

#[test]
fn allowlist_removal_releases_an_active_member() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::enable_private_chain_mode_internal(
            vec![1u64, 2, 3, 50],
            true
        ));

        // An account outside the allowlist leaves nothing to remove.
        System::reset_events();
        assert_ok!(DcfPallet::remove_from_validator_allowlist_internal(&60));
        assert!(dcf_events().is_empty());

        // A member outside the active set leaves the allowlist alone.
        assert_ok!(DcfPallet::remove_from_validator_allowlist_internal(&50));
        assert!(has_event(crate::Event::ValidatorRemovedFromAllowlist {
            validator: 50
        }));
        assert!(!dcf_events()
            .iter()
            .any(|event| matches!(event, crate::Event::ValidatorForcedToLeave { .. })));

        // An active member leaves the validator set as well.
        assert_ok!(DcfPallet::remove_from_validator_allowlist_internal(&3));
        assert!(!DcfPallet::get_validator_allowlist().contains(&3));
        assert!(!DcfPallet::validator_set().contains(&3));
        assert!(has_event(crate::Event::ValidatorForcedToLeave {
            validator: 3,
            reason: b"Removed from private chain allowlist".to_vec(),
        }));
    });
}

// ---------------------------------------------------------------------------
// EVM event helpers
// ---------------------------------------------------------------------------

#[test]
fn evm_compatibility_check_succeeds_with_a_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        assert_ok!(DcfPallet::test_evm_event_compatibility_internal());
        assert!(has_event(crate::Event::EvmCompatibilityTested {
            success: true
        }));
    });
}

#[test]
fn evm_compatibility_check_succeeds_with_an_empty_set() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        crate::ValidatorSet::<Test>::put(BoundedVec::new());
        assert_ok!(DcfPallet::test_evm_event_compatibility_internal());
        assert!(has_event(crate::Event::EvmCompatibilityTested {
            success: true
        }));
    });
}

#[test]
fn evm_query_returns_nothing_for_a_reversed_range() {
    new_test_ext().execute_with(|| {
        store_evm(5, 1);
        assert!(DcfPallet::query_evm_events_internal(None, 6, 5).is_empty());
        assert_eq!(DcfPallet::query_evm_events_internal(None, 5, 5).len(), 1);
    });
}

#[test]
fn evm_query_spans_at_most_one_hundred_blocks() {
    new_test_ext().execute_with(|| {
        store_evm(5, 1);
        store_evm(101, 2);

        assert_eq!(DcfPallet::query_evm_events_internal(None, 1, 101).len(), 2);
        assert!(DcfPallet::query_evm_events_internal(None, 1, 102).is_empty());
    });
}

#[test]
fn evm_query_filters_by_event_type() {
    new_test_ext().execute_with(|| {
        store_evm(5, 1);
        store_evm(6, 2);
        store_evm(7, 1);

        let first_type = DcfPallet::query_evm_events_internal(Some(1), 5, 7);
        assert_eq!(first_type.len(), 2);
        assert!(first_type.iter().all(|record| record.event_type == 1));
        assert_eq!(DcfPallet::query_evm_events_internal(Some(2), 5, 7).len(), 1);
        assert!(DcfPallet::query_evm_events_internal(Some(3), 5, 7).is_empty());
    });
}
