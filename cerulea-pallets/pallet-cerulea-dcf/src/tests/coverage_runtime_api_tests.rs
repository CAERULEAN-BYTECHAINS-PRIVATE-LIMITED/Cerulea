//! Line coverage for the runtime API helpers and storage accessors.
//!
//! Each test reads one helper over real state. The empty case and the
//! populated case both run. Private helpers run through the public calls
//! and the epoch transition that reach them.

use crate::mock::*;
use crate::traits::{ValidatorRegistryProvider, ValidatorState as RegistryState};
use crate::EpochStats;
use frame_support::{assert_ok, traits::ConstU32, traits::Get, BoundedVec};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Build one set of epoch statistics.
fn stats(
    epoch: u32,
    stake_score: u64,
    inference_score: u64,
    final_score: u64,
    authored: u32,
    missed: u32,
) -> EpochStats {
    EpochStats {
        epoch,
        stake_score,
        inference_score,
        final_score,
        authored_blocks: authored,
        missed_blocks: missed,
    }
}

/// Build a registry record around one set of current statistics.
fn registry_state(current: EpochStats) -> RegistryState {
    RegistryState {
        last_active_epoch: current.epoch,
        history: vec![current.clone()],
        current,
        uptime: 1,
        inference_success_count: 0,
        participation_rate: 100,
        inference_count: 0,
        last_active_block: 1,
        name: None,
        trust_score: 5000,
    }
}

/// Write one validator record into the registry.
fn put_state(validator: u64, state: RegistryState) {
    TestValidatorRegistry::update_validator_state(&validator, state);
}

/// Read the active set as a plain list.
fn active() -> Vec<u64> {
    DcfPallet::active_validators().to_vec()
}

/// Read every event the framework pallet deposited.
fn dcf_events() -> Vec<crate::Event<Test>> {
    System::events()
        .into_iter()
        .filter_map(|record| match record.event {
            RuntimeEvent::DcfPallet(inner) => Some(inner),
            _ => None,
        })
        .collect()
}

/// Write one proposal with a chosen status.
fn put_proposal(id: u32, action: crate::ProposalAction<Test>, status: crate::ProposalStatus) {
    crate::Proposals::<Test>::insert(
        id,
        crate::GovernanceProposal::<Test> {
            proposer: 1,
            action,
            status,
            votes_for: 2,
            votes_against: 0,
        },
    );
}

/// The figures one epoch transition event carries.
struct TransitionReading {
    added: Vec<u64>,
    removed: Vec<u64>,
    rewards: u128,
    slashed: u128,
}

/// Read the latest epoch transition event.
fn last_transition() -> TransitionReading {
    dcf_events()
        .into_iter()
        .rev()
        .find_map(|event| match event {
            crate::Event::EpochTransition {
                added_validators,
                removed_validators,
                total_rewards_distributed,
                total_slashed_amount,
                ..
            } => Some(TransitionReading {
                added: added_validators,
                removed: removed_validators,
                rewards: total_rewards_distributed,
                slashed: total_slashed_amount,
            }),
            _ => None,
        })
        .expect("Every epoch transition deposits one transition event.")
}

/// Write one author sequence for one epoch.
fn put_sequence(epoch: u32, authors: Vec<u64>) {
    let sequence: BoundedVec<u64, ConstU32<1000>> = BoundedVec::truncate_from(authors);
    crate::EpochAuthorSequences::<Test>::insert(epoch, sequence);
}

/// Build one epoch history record.
fn history(epoch: u32, validators: Vec<u64>) -> crate::EpochHistory<Test> {
    crate::EpochHistory::<Test> {
        epoch_number: epoch,
        active_validators: BoundedVec::truncate_from(validators.clone()),
        score_snapshot: BoundedVec::truncate_from(
            validators
                .iter()
                .map(|v| (*v, *v * 100 + epoch as u64))
                .collect::<Vec<_>>(),
        ),
        inference_summary: BoundedVec::truncate_from(
            validators
                .iter()
                .map(|v| (*v, Some(epoch as u64)))
                .collect::<Vec<_>>(),
        ),
    }
}

/// Write the epoch history buffer.
fn put_histories(list: Vec<crate::EpochHistory<Test>>) {
    let bounded: BoundedVec<crate::EpochHistory<Test>, <Test as crate::Config>::MaxEpochHistory> =
        BoundedVec::truncate_from(list);
    crate::EpochHistories::<Test>::put(bounded);
}

/// Record one misbehaviour report.
fn report(validator: u64, reporter: u64, evidence: &[u8]) {
    let bounded: BoundedVec<u8, ConstU32<1000>> = BoundedVec::truncate_from(evidence.to_vec());
    crate::MisbehaviorReports::<Test>::insert(validator, reporter, bounded);
}

/// Give every genesis validator a Trust Score inside the bounds.
fn sound_trust_scores() {
    for validator in [1u64, 2, 3] {
        crate::ValidatorTrustScores::<Test>::insert(validator, 5000u64);
    }
}

/// A valid genesis record for the mock runtime.
fn valid_genesis() -> crate::GenesisConfig<Test> {
    crate::GenesisConfig::<Test> {
        validators: vec![1, 2, 3],
        validator_scores: vec![],
        validator_stakes: vec![1000, 2000, 15000],
        validator_names: vec![],
        current_epoch: 0,
        epoch_config: crate::EpochConfig {
            blocks_per_epoch: 10,
            min_stake: 1000,
            max_validators: 100,
        },
        strict_validation: false,
    }
}

// ---------------------------------------------------------------------------
// Storage accessors
// ---------------------------------------------------------------------------

#[test]
fn active_validators_and_validator_set_read_the_registry() {
    new_test_ext().execute_with(|| {
        assert_eq!(active(), vec![1, 2, 3]);
        assert_eq!(DcfPallet::validator_set().to_vec(), vec![1, 2, 3]);

        TestValidatorRegistry::update_active_validators(vec![2]);
        TestValidatorRegistry::update_validator_set(vec![1, 2, 3, 7]);

        assert_eq!(active(), vec![2]);
        assert_eq!(DcfPallet::validator_set().to_vec(), vec![1, 2, 3, 7]);
    });
}

#[test]
fn active_set_reads_empty_once_the_registry_holds_none() {
    new_test_ext().execute_with(|| {
        TestValidatorRegistry::update_active_validators(vec![]);
        assert!(DcfPallet::active_validators().is_empty());
    });
}

#[test]
fn pending_validator_actions_read_each_queued_request() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::pending_validator_actions(&10), None);

        TestValidatorRegistry::add_pending_action(&10, crate::ValidatorAction::Join);
        TestValidatorRegistry::add_pending_action(&2, crate::ValidatorAction::Leave);

        assert_eq!(
            DcfPallet::pending_validator_actions(&10),
            Some(crate::ValidatorAction::Join)
        );
        assert_eq!(
            DcfPallet::pending_validator_actions(&2),
            Some(crate::ValidatorAction::Leave)
        );
        assert_eq!(DcfPallet::pending_validator_actions(&1), None);
    });
}

#[test]
fn validator_join_time_reads_the_recorded_block() {
    new_test_ext().execute_with(|| {
        // The registry seed records block zero for every genesis validator.
        assert_eq!(DcfPallet::validator_join_time(&1), Some(0));
        assert_eq!(DcfPallet::validator_join_time(&10), None);

        TestValidatorRegistry::set_validator_join_time(&10, 25);
        assert_eq!(DcfPallet::validator_join_time(&10), Some(25));
    });
}

#[test]
fn validator_leave_requests_read_the_request_block() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::validator_leave_requests(&1), None);
        TestValidatorRegistry::set_validator_leave_request(&1, 40);
        assert_eq!(DcfPallet::validator_leave_requests(&1), Some(40));
    });
}

#[test]
fn recently_removed_validators_read_the_removal_block() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::recently_removed_validators(&4), None);
        TestValidatorRegistry::set_recently_removed_validator(&4, 12);
        assert_eq!(DcfPallet::recently_removed_validators(&4), Some(12));
    });
}

#[test]
fn validator_states_read_the_genesis_record() {
    new_test_ext().execute_with(|| {
        let state =
            DcfPallet::validator_states(&1).expect("Genesis opens a record for validator 1.");

        // Genesis divides the stake of 1000 by the boost divisor of 1000.
        assert_eq!(state.current.stake_score, 1);
        assert_eq!(state.current.inference_score, 6000);
        // The final score reads (1 * 6000 + 6000 * 4000) / 10000.
        assert_eq!(state.current.final_score, 2400);
        assert_eq!(state.uptime, 1);
        assert_eq!(state.participation_rate, 100);
        assert_eq!(state.trust_score, 5000);
        assert_eq!(state.last_active_block, 1);
        assert_eq!(state.history.len(), 1);
        assert!(state.name.is_none());

        assert!(DcfPallet::validator_states(&99).is_none());
    });
}

#[test]
fn validator_names_read_the_registry_name() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::validator_names(&1).is_none());

        TestValidatorRegistry::set_validator_name(&1, b"Alpha".to_vec());
        assert_eq!(
            DcfPallet::validator_names(&1).map(|name| name.to_vec()),
            Some(b"Alpha".to_vec())
        );
    });
}

#[test]
fn validator_uptime_reads_the_state_uptime() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::validator_uptime(&99), 0);

        let mut state = registry_state(stats(0, 10, 20, 30, 0, 0));
        state.uptime = 4;
        put_state(10, state);
        assert_eq!(DcfPallet::validator_uptime(&10), 4);
    });
}

#[test]
fn validator_metadata_reads_the_registry_record() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::validator_metadata(&1).is_none());

        let metadata = crate::ValidatorMetadataInfo {
            name: BoundedVec::truncate_from(b"Alpha".to_vec()),
            website: Some(BoundedVec::truncate_from(b"alpha.example".to_vec())),
            contact: None,
            description: Some(BoundedVec::truncate_from(b"Hyderabad node".to_vec())),
            location: Some(BoundedVec::truncate_from(b"IN".to_vec())),
            commission_rate: Some(500),
            min_stake_required: Some(1000),
            created_at: 10,
            updated_at: 20,
        };
        TestValidatorRegistry::set_validator_metadata(&1, metadata.clone());

        assert_eq!(DcfPallet::validator_metadata(&1), Some(metadata));
    });
}

#[test]
fn validator_performance_history_reads_every_record() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::validator_performance_history(&1).is_empty());

        let record = crate::PerformanceRecord {
            epoch: 3,
            blocks_authored: 8,
            blocks_missed: 2,
            uptime_percentage: 8000,
            inference_score: 40,
            stake_score: 1000,
            final_score: 616,
            participation_rate: 9000,
            timestamp: 12_000,
        };
        TestValidatorRegistry::set_validator_performance_history(&1, vec![record.clone()]);

        assert_eq!(
            DcfPallet::validator_performance_history(&1).to_vec(),
            vec![record]
        );
    });
}

#[test]
fn block_counters_read_the_registry_counts() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::validator_last_seen(&1), 0);
        assert_eq!(DcfPallet::validator_blocks_authored(&1), 0);
        assert_eq!(DcfPallet::validator_blocks_missed(&1), 0);

        TestValidatorRegistry::set_validator_last_seen(&1, 9);
        TestValidatorRegistry::set_validator_blocks_authored(&1, 5);
        TestValidatorRegistry::set_validator_blocks_missed(&1, 2);

        assert_eq!(DcfPallet::validator_last_seen(&1), 9);
        assert_eq!(DcfPallet::validator_blocks_authored(&1), 5);
        assert_eq!(DcfPallet::validator_blocks_missed(&1), 2);
    });
}

// ---------------------------------------------------------------------------
// Block author validation
// ---------------------------------------------------------------------------

#[test]
fn is_validator_active_follows_the_active_set() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::is_validator_active(&1));
        assert!(!DcfPallet::is_validator_active(&99));

        // A seated validator outside the active set reads inactive.
        TestValidatorRegistry::update_active_validators(vec![1, 2]);
        assert!(!DcfPallet::is_validator_active(&3));
    });
}

#[test]
fn validate_block_author_stays_quiet_for_an_active_author() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        DcfPallet::validate_block_author(7, 1);

        let flagged = dcf_events()
            .into_iter()
            .filter(|event| matches!(event, crate::Event::InvalidAuthor { .. }))
            .count();
        assert_eq!(
            flagged, 0,
            "An active author raises no invalid author event."
        );
    });
}

#[test]
fn validate_block_author_flags_an_unknown_author() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        DcfPallet::validate_block_author(7, 99);

        System::assert_has_event(RuntimeEvent::DcfPallet(crate::Event::InvalidAuthor {
            block_number: 7,
            author: 99,
        }));
    });
}

#[test]
fn validate_expected_author_accepts_the_scheduled_author() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_sequence(0, vec![2, 3, 1, 2, 3, 1, 2, 3, 1, 2]);

        assert!(DcfPallet::validate_expected_author(1, 3));

        let mismatches = dcf_events()
            .into_iter()
            .filter(|event| matches!(event, crate::Event::AuthorMismatch { .. }))
            .count();
        assert_eq!(mismatches, 0);
    });
}

#[test]
fn validate_expected_author_rejects_another_author_and_reports_it() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_sequence(0, vec![2, 3, 1, 2, 3, 1, 2, 3, 1, 2]);

        assert!(!DcfPallet::validate_expected_author(1, 1));

        System::assert_has_event(RuntimeEvent::DcfPallet(crate::Event::AuthorMismatch {
            block_number: 1,
            expected: Some(3),
            actual: 1,
        }));
    });
}

#[test]
fn validate_expected_author_accepts_any_author_with_an_empty_active_set() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        TestValidatorRegistry::update_active_validators(vec![]);

        assert!(DcfPallet::validate_expected_author(4, 42));
        assert!(dcf_events()
            .iter()
            .all(|event| !matches!(event, crate::Event::AuthorMismatch { .. })));
    });
}

// ---------------------------------------------------------------------------
// Expected author: Authoring Engine sequence and weighted fallback
// ---------------------------------------------------------------------------

#[test]
fn expected_author_reads_none_with_an_empty_active_set() {
    new_test_ext().execute_with(|| {
        TestValidatorRegistry::update_active_validators(vec![]);
        assert_eq!(DcfPallet::get_expected_author(3), None);
        assert_eq!(DcfPallet::get_expected_author_legacy(3), None);
        assert_eq!(DcfPallet::expected_author(3), None);
    });
}

#[test]
fn expected_author_reads_the_deterministic_sequence() {
    new_test_ext().execute_with(|| {
        put_sequence(0, vec![2, 3, 1, 2, 3, 1, 2, 3, 1, 2]);

        assert_eq!(DcfPallet::get_expected_author(0), Some(2));
        assert_eq!(DcfPallet::get_expected_author(1), Some(3));
        assert_eq!(DcfPallet::get_expected_author(2), Some(1));

        // Epoch 1 starts at block 10 with ten blocks to an epoch.
        put_sequence(1, vec![3, 1]);
        assert_eq!(DcfPallet::get_expected_author(10), Some(3));
        assert_eq!(DcfPallet::get_expected_author(11), Some(1));
    });
}

#[test]
fn expected_author_falls_back_to_weighted_selection_past_a_short_sequence() {
    new_test_ext().execute_with(|| {
        put_sequence(1, vec![3]);

        // Block 12 sits at offset 2 of a sequence one entry long.
        let fallback = DcfPallet::get_expected_author(12);
        assert_eq!(fallback, DcfPallet::get_expected_author_legacy(12));
        // Each genesis validator weighs 100. The target reads 132.
        assert_eq!(fallback, Some(2));
    });
}

#[test]
fn expected_author_falls_back_to_weighted_selection_without_a_sequence() {
    new_test_ext().execute_with(|| {
        crate::EpochAuthorSequences::<Test>::remove(0);
        crate::EpochAuthorSequences::<Test>::remove(2);

        for block in [1u32, 4, 25] {
            assert_eq!(
                DcfPallet::get_expected_author(block),
                DcfPallet::get_expected_author_legacy(block),
                "Block {} reads the weighted selection.",
                block
            );
        }
    });
}

#[test]
fn expected_author_wrapper_matches_the_runtime_api_helper() {
    new_test_ext().execute_with(|| {
        put_sequence(0, vec![3, 3, 2, 1, 1, 2, 3, 1, 2, 3]);
        for block in 0u32..30 {
            assert_eq!(
                DcfPallet::expected_author(block),
                DcfPallet::get_expected_author(block)
            );
        }
    });
}

#[test]
fn expected_author_answers_an_active_validator_with_a_zero_epoch_length() {
    new_test_ext().execute_with(|| {
        crate::EpochConfigStorage::<Test>::mutate(|epoch| epoch.blocks_per_epoch = 0);
        crate::EpochAuthorSequences::<Test>::remove(0);

        // A genesis record built without strict checks may carry a zero
        // epoch length. The author lookup still answers a seated author.
        let author = DcfPallet::get_expected_author(5)
            .expect("A non empty active set always yields an author.");
        assert!(active().contains(&author));
    });
}

#[test]
fn legacy_selection_splits_equal_weights_by_block_number() {
    new_test_ext().execute_with(|| {
        // Stake 1000 at a stake share of 6000 caps each weight at 100.
        // The seed multiplier leaves 61 modulo the total of 300.
        assert_eq!(DcfPallet::get_expected_author_legacy(0), Some(1));
        assert_eq!(DcfPallet::get_expected_author_legacy(1), Some(1));
        assert_eq!(DcfPallet::get_expected_author_legacy(2), Some(2));
        assert_eq!(DcfPallet::get_expected_author_legacy(3), Some(2));
        assert_eq!(DcfPallet::get_expected_author_legacy(4), Some(3));
        assert_eq!(DcfPallet::get_expected_author_legacy(5), Some(1));
    });
}

#[test]
fn legacy_selection_follows_uneven_stake_weights() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::put(100u64);
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 1000u128);
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 3000u128);
        pallet_cerulea_pos::Stake::<Test>::insert(3u64, 6000u128);

        // Weights read 10, 30 and 60. The total reads 100.
        assert_eq!(DcfPallet::get_expected_author_legacy(0), Some(1));
        assert_eq!(DcfPallet::get_expected_author_legacy(1), Some(3));
        assert_eq!(DcfPallet::get_expected_author_legacy(2), Some(2));
        assert_eq!(DcfPallet::get_expected_author_legacy(3), Some(3));
    });
}

#[test]
fn legacy_selection_keeps_a_zero_score_validator_at_weight_one() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::put(100u64);
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 0u128);
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 3000u128);
        pallet_cerulea_pos::Stake::<Test>::insert(3u64, 6000u128);

        // Validator 1 holds the first slot of weight one.
        assert_eq!(DcfPallet::get_expected_author_legacy(0), Some(1));
    });
}

// ---------------------------------------------------------------------------
// Genesis checks and invariants
// ---------------------------------------------------------------------------

#[test]
fn validate_genesis_accepts_a_sound_record() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::validate_genesis_config(&valid_genesis()), Ok(()));
    });
}

#[test]
fn validate_genesis_rejects_an_empty_validator_list() {
    new_test_ext().execute_with(|| {
        let mut genesis = valid_genesis();
        genesis.validators = vec![];
        genesis.validator_stakes = vec![];

        let message = DcfPallet::validate_genesis_config(&genesis)
            .expect_err("An empty validator list fails the genesis check.");
        assert!(message.contains("at least one validator"));
    });
}

#[test]
fn validate_genesis_rejects_a_duplicate_validator() {
    new_test_ext().execute_with(|| {
        let mut genesis = valid_genesis();
        genesis.validators = vec![1, 2, 2];

        let message = DcfPallet::validate_genesis_config(&genesis)
            .expect_err("A duplicate validator fails the genesis check.");
        assert!(message.contains("Duplicate validator"));
    });
}

#[test]
fn dry_run_genesis_build_reports_stake_figures_and_warnings() {
    new_test_ext().execute_with(|| {
        let genesis = valid_genesis();
        let report = DcfPallet::dry_run_genesis_build(&genesis)
            .expect("A sound genesis record passes the dry run.");

        assert_eq!(report.validator_count, 3);
        assert_eq!(report.total_stake, 18_000);
        assert_eq!(report.average_stake, 6_000);
        assert_eq!(report.min_stake_validator, Some((1, 1000)));
        assert_eq!(report.max_stake_validator, Some((3, 15_000)));
        assert_eq!(report.epoch_config, genesis.epoch_config);
        assert_eq!(report.invariant_checks.len(), 5);
        assert!(report.validation_passed);

        // The highest stake sits above ten times the lowest. The epoch runs
        // ten blocks. Each fact raises one warning.
        assert_eq!(report.warnings.len(), 2);
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("High stake inequality")));
        assert!(report
            .warnings
            .iter()
            .any(|w| w.contains("Short epoch length")));
    });
}

#[test]
fn dry_run_genesis_build_returns_the_validation_error() {
    new_test_ext().execute_with(|| {
        let mut genesis = valid_genesis();
        genesis.validator_stakes = vec![1000, 1000, 10];

        let message = DcfPallet::dry_run_genesis_build(&genesis)
            .expect_err("A stake below the minimum fails the dry run.");
        assert!(message.contains("below MinStake"));
    });
}

#[test]
fn current_invariants_hold_for_a_sound_state() {
    new_test_ext().execute_with(|| {
        sound_trust_scores();
        assert_eq!(DcfPallet::validate_current_invariants(), Ok(()));
    });
}

#[test]
fn current_invariants_hold_straight_after_genesis() {
    new_test_ext().execute_with(|| {
        // Genesis fills the Trust Score map for every validator it seats.
        for validator in [1u64, 2, 3] {
            let score = crate::ValidatorTrustScores::<Test>::get(validator);
            assert!(
                (1000..=10_000).contains(&score),
                "Genesis seats validator {} with a Trust Score inside the bounds.",
                validator
            );
        }
        assert_eq!(DcfPallet::validate_current_invariants(), Ok(()));
    });
}

#[test]
fn current_invariants_flag_every_cleared_trust_score() {
    new_test_ext().execute_with(|| {
        for validator in [1u64, 2, 3] {
            crate::ValidatorTrustScores::<Test>::remove(validator);
        }

        // A cleared score reads zero, below the minimum of 1000.
        let violations = DcfPallet::validate_current_invariants()
            .expect_err("Three cleared Trust Scores raise three violations.");
        assert_eq!(violations.len(), 3);
        assert!(violations
            .iter()
            .all(|v| v.contains("Trust score invariant violation")));
    });
}

#[test]
fn validate_genesis_rejects_a_zero_epoch_length() {
    new_test_ext().execute_with(|| {
        let mut genesis = valid_genesis();
        genesis.epoch_config.blocks_per_epoch = 0;

        let message = DcfPallet::validate_genesis_config(&genesis)
            .expect_err("A zero epoch length fails the genesis check.");
        assert!(message.contains("Epoch length cannot be zero"));
    });
}

#[test]
fn current_invariants_flag_a_stake_below_the_minimum() {
    new_test_ext().execute_with(|| {
        sound_trust_scores();
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 500u128);

        let violations = DcfPallet::validate_current_invariants()
            .expect_err("A stake of 500 sits below the minimum of 1000.");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Economic invariant violation"));
    });
}

#[test]
fn current_invariants_flag_an_active_set_below_the_minimum() {
    new_test_ext().execute_with(|| {
        sound_trust_scores();
        TestValidatorRegistry::update_active_validators(vec![1, 2]);

        let violations = DcfPallet::validate_current_invariants()
            .expect_err("Two active validators sit below the minimum of three.");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("below MinActiveValidators"));
    });
}

#[test]
fn current_invariants_flag_a_trust_score_above_the_maximum() {
    new_test_ext().execute_with(|| {
        sound_trust_scores();
        crate::ValidatorTrustScores::<Test>::insert(3u64, 10_001u64);

        let violations = DcfPallet::validate_current_invariants()
            .expect_err("A Trust Score of 10001 sits above the maximum.");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Trust score invariant violation"));
    });
}

#[test]
fn current_invariants_flag_zero_consensus_shares() {
    new_test_ext().execute_with(|| {
        sound_trust_scores();
        crate::PosWeight::<Test>::put(0u64);
        crate::PoiWeight::<Test>::put(0u64);

        let violations = DcfPallet::validate_current_invariants()
            .expect_err("Two zero shares leave the score without weight.");
        assert_eq!(violations.len(), 1);
        assert!(violations[0].contains("Consensus weight invariant violation"));
    });
}

// ---------------------------------------------------------------------------
// Validator proposals
// ---------------------------------------------------------------------------

#[test]
fn proposals_reward_every_validator_at_the_score_ceiling() {
    new_test_ext().execute_with(|| {
        // Stake 1000 at a stake share of 6000 reads 600 and caps at 100.
        let proposals = DcfPallet::generate_validator_proposals();
        let expected: Vec<_> = [1u64, 2, 3]
            .iter()
            .map(|v| {
                (
                    *v,
                    crate::ProposalAction::<Test>::Reward {
                        validator: *v,
                        amount: 10_000,
                    },
                    100u64,
                    1000u64,
                    0u64,
                )
            })
            .collect();
        assert_eq!(proposals, expected);
    });
}

#[test]
fn proposals_eject_a_validator_without_stake_or_inference() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_pos::Stake::<Test>::insert(2u64, 0u128);

        let proposals = DcfPallet::generate_validator_proposals();
        assert_eq!(proposals.len(), 3);
        assert_eq!(
            proposals[1],
            (
                2,
                crate::ProposalAction::<Test>::Eject {
                    validator: 2,
                    reason: crate::EjectionReason::ScoreBelowThreshold,
                },
                0,
                0,
                0
            )
        );
    });
}

#[test]
fn proposals_eject_a_validator_carried_by_inference_alone() {
    new_test_ext().execute_with(|| {
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 0u128);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (100u32, 0u32));

        // The inference share of 4000 turns 100 into 40, below 50.
        let proposals = DcfPallet::generate_validator_proposals();
        assert_eq!(
            proposals[0],
            (
                1,
                crate::ProposalAction::<Test>::Eject {
                    validator: 1,
                    reason: crate::EjectionReason::ScoreBelowThreshold,
                },
                40,
                0,
                100
            )
        );
    });
}

#[test]
fn proposals_skip_scores_between_the_two_thresholds() {
    new_test_ext().execute_with(|| {
        // A stake share of 600 reads 60 for every validator.
        crate::PosWeight::<Test>::put(600u64);
        assert!(DcfPallet::generate_validator_proposals().is_empty());

        // A score of exactly 50 meets the minimum and earns no reward.
        crate::PosWeight::<Test>::put(500u64);
        assert!(DcfPallet::generate_validator_proposals().is_empty());
    });
}

#[test]
fn proposals_reward_a_score_exactly_at_the_high_threshold() {
    new_test_ext().execute_with(|| {
        // A stake share of 800 reads 80, the high performance line.
        crate::PosWeight::<Test>::put(800u64);
        let proposals = DcfPallet::generate_validator_proposals();
        assert_eq!(proposals.len(), 3);
        assert!(proposals
            .iter()
            .all(|(_, action, combined, _, _)| *combined == 80
                && matches!(action, crate::ProposalAction::Reward { amount: 10_000, .. })));
    });
}

#[test]
fn proposals_read_empty_for_an_empty_validator_set() {
    new_test_ext().execute_with(|| {
        TestValidatorRegistry::update_validator_set(vec![]);
        assert!(DcfPallet::generate_validator_proposals().is_empty());
    });
}

// ---------------------------------------------------------------------------
// Validator profiles
// ---------------------------------------------------------------------------

#[test]
fn profile_reads_none_for_an_unknown_validator() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_validator_profile(99), None);
        assert!(DcfPallet::get_validator_profile_new(99).is_none());
        assert_eq!(DcfPallet::get_validator_score_breakdown(99), None);
        assert_eq!(DcfPallet::get_validator_uptime_stats(99), None);
    });
}

#[test]
fn profile_reads_the_stored_inference_score_without_a_fresh_result() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 1, 30, 0, 4, 2));
        state.uptime = 4;
        state.participation_rate = 85;
        put_state(1, state);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 6400u64);
        crate::PosWeight::<Test>::put(100u64);
        crate::PoiWeight::<Test>::put(1000u64);

        // The score reads (1000 * 100 + 30 * 1000) / 10000.
        assert_eq!(
            DcfPallet::get_validator_profile(1),
            Some((13, 1000, 30, 6400, 4, 0, 85, 2))
        );
    });
}

#[test]
fn profile_reads_the_fresh_inference_result_and_counters() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 1, 6000, 0, 0, 1));
        state.uptime = 6;
        state.participation_rate = 90;
        put_state(1, state);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (42u32, 0u32));
        pallet_cerulea_poi::ValidatorInferenceCount::<Test>::insert(1u64, 9u64);
        crate::ValidatorTrustScores::<Test>::insert(1u64, 7000u64);
        crate::PosWeight::<Test>::put(500u64);
        crate::PoiWeight::<Test>::put(1000u64);

        // The score reads (1000 * 500 + 42 * 1000) / 10000.
        assert_eq!(
            DcfPallet::get_validator_profile(1),
            Some((54, 1000, 42, 7000, 6, 9, 90, 1))
        );
    });
}

#[test]
fn profile_caps_the_combined_score_at_the_maximum() {
    new_test_ext().execute_with(|| {
        put_state(1, registry_state(stats(0, 1, 6000, 0, 0, 0)));

        let profile = DcfPallet::get_validator_profile(1).expect("Validator 1 holds a record.");
        // The raw figure reads 3000. The ceiling holds it at 100.
        assert_eq!(profile.0, 100);
        assert_eq!(profile.2, 6000);
    });
}

#[test]
fn profile_uses_default_shares_once_stored_shares_are_cleared() {
    new_test_ext().execute_with(|| {
        crate::PosWeight::<Test>::kill();
        crate::PoiWeight::<Test>::kill();
        pallet_cerulea_pos::Stake::<Test>::insert(1u64, 10u128);
        pallet_cerulea_poi::InferenceResults::<Test>::insert(1u64, (1u32, 0u32));

        // The defaults 6000 and 4000 give (10 * 6000 + 1 * 4000) / 10000.
        let profile = DcfPallet::get_validator_profile(1).expect("Validator 1 holds a record.");
        assert_eq!(profile.0, 6);
        assert_eq!(profile.1, 10);
        assert_eq!(profile.2, 1);
    });
}

#[test]
fn validator_name_reads_the_registry_name() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_validator_name(&2), None);
        TestValidatorRegistry::set_validator_name(&2, b"Beta".to_vec());
        assert_eq!(DcfPallet::get_validator_name(&2), Some(b"Beta".to_vec()));
    });
}

#[test]
fn profile_new_reads_an_active_validator() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 7, 6000, 2400, 0, 0));
        state.inference_count = 12;
        state.name = Some(b"Alpha".to_vec());
        state.last_active_block = 33;
        put_state(1, state);

        let expected = crate::ValidatorProfile::<u64, u128, u64> {
            stake: 1000,
            poi_score: 6000,
            final_score: 2400,
            trust_score: 5000,
            status: crate::ValidatorStatus::Active,
            inference_count: 12,
            name: Some(BoundedVec::truncate_from(b"Alpha".to_vec())),
            last_active_block: 33,
            _phantom: core::marker::PhantomData,
        };
        assert_eq!(DcfPallet::get_validator_profile_new(1), Some(expected));
    });
}

#[test]
fn profile_new_marks_a_validator_outside_the_active_set_inactive() {
    new_test_ext().execute_with(|| {
        TestValidatorRegistry::update_active_validators(vec![1, 2]);

        let profile = DcfPallet::get_validator_profile_new(3).expect("Validator 3 holds a record.");
        assert_eq!(profile.status, crate::ValidatorStatus::Inactive);
        assert!(profile.name.is_none());
    });
}

#[test]
fn score_breakdown_reads_the_stored_scores_and_shares() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 5000, 2500, 4000, 0, 0));
        state.trust_score = 6100;
        put_state(1, state);

        assert_eq!(
            DcfPallet::get_validator_score_breakdown(1),
            Some(crate::ScoreBreakdown {
                pos_score: 5000,
                poi_score: 2500,
                pos_weight: 6000,
                poi_weight: 4000,
                final_score: 4000,
                trust_score: 6100,
            })
        );
    });
}

#[test]
fn uptime_stats_read_the_authored_share_in_basis_points() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 1, 1, 1, 3, 1));
        state.uptime = 6;
        state.participation_rate = 77;
        put_state(1, state);

        assert_eq!(
            DcfPallet::get_validator_uptime_stats(1),
            Some(crate::UptimeStats {
                total_epochs: 6,
                blocks_authored: 3,
                blocks_missed: 1,
                uptime_percentage: 7500,
                participation_rate: 77,
            })
        );
    });
}

#[test]
fn uptime_stats_read_full_uptime_before_any_slot() {
    new_test_ext().execute_with(|| {
        put_state(1, registry_state(stats(0, 1, 1, 1, 0, 0)));

        let uptime = DcfPallet::get_validator_uptime_stats(1).expect("Validator 1 holds a record.");
        assert_eq!(uptime.uptime_percentage, 10_000);
    });
}

#[test]
fn slashing_history_reads_the_stake_pallet_record() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::get_slashing_history(1).is_empty());

        let record = pallet_cerulea_pos::SlashingRecord::<u128, u64> {
            block_number: 7,
            amount: 250,
            reason: pallet_cerulea_pos::SlashReason::Downtime,
            epoch: 2,
        };
        let held: BoundedVec<pallet_cerulea_pos::SlashingRecord<u128, u64>, ConstU32<100>> =
            BoundedVec::truncate_from(vec![record.clone()]);
        pallet_cerulea_pos::ValidatorSlashingHistory::<Test>::insert(1u64, held);

        assert_eq!(DcfPallet::get_slashing_history(1), vec![record]);
    });
}

#[test]
fn system_constants_read_the_runtime_values() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::get_system_constants(),
            crate::SystemConstants::<u128, u64> {
                min_stake: 1000,
                min_score_threshold: 50,
                epoch_length: 100,
                max_validators: 100,
                cooldown_period: 1000,
            }
        );
    });
}

// ---------------------------------------------------------------------------
// Cooldown status
// ---------------------------------------------------------------------------

#[test]
fn cooldown_status_reads_the_leave_request_block() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_validator_cooldown_status(1), None);
        TestValidatorRegistry::set_validator_leave_request(&1, 40);
        assert_eq!(DcfPallet::get_validator_cooldown_status(1), Some(40u64));
    });
}

#[test]
fn detailed_cooldown_reads_none_without_a_request_or_removal() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_validator_detailed_cooldown_status(1), None);
    });
}

#[test]
fn detailed_cooldown_counts_down_a_leave_request() {
    new_test_ext().execute_with(|| {
        System::set_block_number(300);
        TestValidatorRegistry::set_validator_leave_request(&1, 100);
        // A recorded removal stays behind the pending leave request.
        TestValidatorRegistry::set_recently_removed_validator(&1, 50);

        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status(1),
            Some((800, false))
        );
    });
}

#[test]
fn detailed_cooldown_counts_down_a_recent_removal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(300);
        TestValidatorRegistry::set_recently_removed_validator(&4, 100);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status(4),
            Some((800, false))
        );
    });
}

#[test]
fn detailed_cooldown_releases_a_removal_after_the_full_period() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1100);
        TestValidatorRegistry::set_recently_removed_validator(&4, 100);
        assert_eq!(
            DcfPallet::get_validator_detailed_cooldown_status(4),
            Some((0, true))
        );
    });
}

// ---------------------------------------------------------------------------
// Score readers
// ---------------------------------------------------------------------------

#[test]
fn score_readers_answer_zero_for_an_unknown_validator() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_inference_result(99), None);
        assert_eq!(DcfPallet::validator_stake_score(&99), 0);
        assert_eq!(DcfPallet::validator_inference_score(&99), 0);
        assert_eq!(DcfPallet::validator_participation(&99), (0, 0));
        assert_eq!(DcfPallet::validator_last_active(&99), 0);
        assert!(DcfPallet::validator_score_history(&99).is_empty());
        assert_eq!(DcfPallet::get_validator_consensus_contribution(&99), None);
    });
}

#[test]
fn score_readers_answer_the_stored_record() {
    new_test_ext().execute_with(|| {
        let mut state = registry_state(stats(0, 1, 77, 50, 6, 2));
        state.last_active_block = 33;
        put_state(1, state);

        assert_eq!(DcfPallet::get_inference_result(1), Some(77));
        assert_eq!(DcfPallet::validator_stake_score(&1), 1000);
        assert_eq!(DcfPallet::validator_inference_score(&1), 77);
        assert_eq!(DcfPallet::validator_participation(&1), (6, 2));
        assert_eq!(DcfPallet::validator_last_active(&1), 33);
    });
}

#[test]
fn score_history_reads_every_epoch_record() {
    new_test_ext().execute_with(|| {
        let first = stats(1, 10, 20, 30, 1, 0);
        let second = stats(2, 11, 21, 31, 2, 1);
        let mut state = registry_state(second.clone());
        state.history = vec![first.clone(), second.clone()];
        put_state(1, state);

        assert_eq!(DcfPallet::validator_score_history(&1), vec![first, second]);
    });
}

#[test]
fn consensus_contribution_splits_the_score_by_share() {
    new_test_ext().execute_with(|| {
        put_state(1, registry_state(stats(0, 5000, 2500, 4000, 0, 0)));

        // 5000 * 6000 / 10000 and 2500 * 4000 / 10000.
        assert_eq!(
            DcfPallet::get_validator_consensus_contribution(&1),
            Some((3000, 1000, 4000))
        );
    });
}

#[test]
fn consensus_weights_read_the_stored_shares() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::consensus_weights(), (6000, 4000));
        crate::PosWeight::<Test>::put(7000u64);
        crate::PoiWeight::<Test>::put(3000u64);
        assert_eq!(DcfPallet::consensus_weights(), (7000, 3000));
    });
}

#[test]
fn epoch_stats_read_the_current_epoch_first() {
    new_test_ext().execute_with(|| {
        let older = stats(2, 1, 2, 3, 4, 5);
        let previous = stats(3, 6, 7, 8, 9, 10);
        let current = stats(4, 11, 12, 13, 14, 15);
        let mut state = registry_state(current.clone());
        state.history = vec![older.clone(), previous.clone()];
        put_state(1, state);

        assert_eq!(DcfPallet::get_validator_epoch_stats(&1, 4), Some(current));
        assert_eq!(DcfPallet::get_validator_epoch_stats(&1, 3), Some(previous));
        assert_eq!(DcfPallet::get_validator_epoch_stats(&1, 2), Some(older));
        assert_eq!(DcfPallet::get_validator_epoch_stats(&1, 9), None);
        assert_eq!(DcfPallet::get_validator_epoch_stats(&99, 4), None);
    });
}

// ---------------------------------------------------------------------------
// Set level readers
// ---------------------------------------------------------------------------

#[test]
fn governance_mode_follows_the_epoch_length_range() {
    new_test_ext().execute_with(|| {
        // Genesis records the epoch length of 10 as the current value.
        assert!(DcfPallet::governance_mode());
        crate::GovernanceConfigStorage::<Test>::kill();
        assert!(!DcfPallet::governance_mode());
    });
}

#[test]
fn governance_mode_flag_reads_the_stored_switch() {
    new_test_ext().execute_with(|| {
        assert!(!DcfPallet::get_governance_mode());
        crate::GovernanceModeEnabled::<Test>::put(true);
        assert!(DcfPallet::get_governance_mode());
    });
}

#[test]
fn validator_counts_separate_records_from_seats() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::total_validators_count(), 3);
        assert_eq!(DcfPallet::get_total_validators_count(), 3);

        // A seat without a state record counts as a seat alone.
        TestValidatorRegistry::update_validator_set(vec![1, 2, 3, 50]);
        assert_eq!(DcfPallet::total_validators_count(), 3);
        assert_eq!(DcfPallet::get_total_validators_count(), 4);

        put_state(50, registry_state(stats(0, 1, 1, 1, 0, 0)));
        assert_eq!(DcfPallet::total_validators_count(), 4);
    });
}

#[test]
fn validator_set_info_reads_both_orderings() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::validator_set_info(), (3, 3, 100));
        assert_eq!(DcfPallet::get_validator_set_info(), (3, 3, 100));

        TestValidatorRegistry::update_active_validators(vec![1, 2]);
        TestValidatorRegistry::update_validator_set(vec![1, 2, 3, 50]);

        // The first helper answers active, records, ceiling.
        assert_eq!(DcfPallet::validator_set_info(), (2, 3, 100));
        // The second helper answers seats, active, ceiling.
        assert_eq!(DcfPallet::get_validator_set_info(), (4, 2, 100));
    });
}

#[test]
fn validators_by_score_rank_highest_first() {
    new_test_ext().execute_with(|| {
        put_state(1, registry_state(stats(0, 1, 1, 100, 0, 0)));
        put_state(2, registry_state(stats(0, 1, 1, 300, 0, 0)));
        put_state(3, registry_state(stats(0, 1, 1, 200, 0, 0)));
        TestValidatorRegistry::update_active_validators(vec![1, 2, 3, 60]);

        let expected = vec![(2u64, 300u64), (3, 200), (1, 100), (60, 0)];
        assert_eq!(DcfPallet::get_validators_by_score(), expected);
        assert_eq!(DcfPallet::validators_by_score(), expected);
    });
}

#[test]
fn validators_by_score_read_empty_for_an_empty_active_set() {
    new_test_ext().execute_with(|| {
        TestValidatorRegistry::update_active_validators(vec![]);
        assert!(DcfPallet::get_validators_by_score().is_empty());
    });
}

#[test]
fn epoch_parameters_read_the_genesis_record() {
    new_test_ext().execute_with(|| {
        assert_eq!(
            DcfPallet::get_epoch_config(),
            crate::EpochConfig {
                blocks_per_epoch: 10,
                min_stake: 1000,
                max_validators: 100,
            }
        );
    });
}

// ---------------------------------------------------------------------------
// Epoch history
// ---------------------------------------------------------------------------

#[test]
fn epoch_history_reads_none_for_an_empty_buffer() {
    new_test_ext().execute_with(|| {
        crate::EpochHistories::<Test>::kill();
        assert_eq!(DcfPallet::get_epoch_history_api(1), None);
        assert!(DcfPallet::get_recent_epochs_api(5).is_empty());
    });
}

#[test]
fn epoch_history_finds_the_matching_epoch() {
    new_test_ext().execute_with(|| {
        put_histories(vec![
            history(1, vec![1, 2]),
            history(2, vec![1, 2, 3]),
            history(3, vec![2, 3]),
        ]);

        let found = DcfPallet::get_epoch_history_api(2).expect("Epoch 2 sits in the buffer.");
        assert_eq!(found.epoch_number, 2);
        assert_eq!(found.active_validators.to_vec(), vec![1, 2, 3]);
        assert_eq!(
            found.score_snapshot.to_vec(),
            vec![(1, 102), (2, 202), (3, 302)]
        );
        assert_eq!(
            found.inference_summary.to_vec(),
            vec![(1, Some(2)), (2, Some(2)), (3, Some(2))]
        );

        assert_eq!(DcfPallet::get_epoch_history_api(9), None);
    });
}

#[test]
fn recent_epochs_read_the_newest_entries_in_order() {
    new_test_ext().execute_with(|| {
        put_histories(vec![
            history(1, vec![1]),
            history(2, vec![2]),
            history(3, vec![3]),
        ]);

        let numbers = |n: u32| -> Vec<u32> {
            DcfPallet::get_recent_epochs_api(n)
                .into_iter()
                .map(|h| h.epoch_number)
                .collect()
        };

        assert_eq!(numbers(2), vec![2, 3]);
        assert_eq!(numbers(3), vec![1, 2, 3]);
        // A request above the buffer length reads the whole buffer.
        assert_eq!(numbers(10), vec![1, 2, 3]);
        assert!(numbers(0).is_empty());
    });
}

// ---------------------------------------------------------------------------
// Proposals
// ---------------------------------------------------------------------------

#[test]
fn proposal_details_read_the_stored_proposal() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_proposal_details(7), None);

        let action = crate::ProposalAction::<Test>::Reward {
            validator: 2,
            amount: 300,
        };
        put_proposal(7, action.clone(), crate::ProposalStatus::Pending);

        let held = DcfPallet::get_proposal_details(7).expect("Proposal 7 sits in storage.");
        assert_eq!(held.action, action);
        assert_eq!(held.status, crate::ProposalStatus::Pending);
        assert_eq!(held.votes_for, 2);
    });
}

#[test]
fn active_proposals_list_pending_proposals_alone() {
    new_test_ext().execute_with(|| {
        assert!(DcfPallet::get_active_proposals().is_empty());

        let action = |v: u64| crate::ProposalAction::<Test>::Reward {
            validator: v,
            amount: 10,
        };
        put_proposal(0, action(1), crate::ProposalStatus::Pending);
        put_proposal(1, action(2), crate::ProposalStatus::Approved);
        put_proposal(2, action(3), crate::ProposalStatus::Pending);
        put_proposal(3, action(1), crate::ProposalStatus::Executed);
        put_proposal(4, action(2), crate::ProposalStatus::Rejected);

        let mut ids: Vec<u32> = DcfPallet::get_active_proposals()
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        ids.sort();
        assert_eq!(ids, vec![0, 2]);
    });
}

// ---------------------------------------------------------------------------
// Governance execution: Amendment Engine actions on the validator set
// ---------------------------------------------------------------------------

#[test]
fn add_validator_proposal_seats_a_staked_candidate() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 1000u128);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::AddValidator { validator: 10 },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert!(DcfPallet::validator_set().contains(&10));
        System::assert_has_event(RuntimeEvent::DcfPallet(crate::Event::ValidatorAdded {
            validator: 10,
        }));

        let state = DcfPallet::validator_states(&10).expect("Admission opens a state record.");
        assert_eq!(state.current.epoch, 0);
        assert_eq!(state.current.stake_score, 1000);
        assert_eq!(state.current.inference_score, 0);
        // The score reads 1000 * 6000 / 10000 = 600 and caps at 100.
        assert_eq!(state.current.final_score, 100);
        assert_eq!(state.uptime, 1);
        assert_eq!(state.trust_score, 0);
        assert_eq!(state.last_active_block, 5);
        assert_eq!(state.history.len(), 1);
        assert_eq!(DcfPallet::validator_join_time(&10), Some(5));
    });
}

#[test]
fn add_validator_proposal_keeps_an_existing_state_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        pallet_cerulea_pos::Stake::<Test>::insert(10u64, 1000u128);
        let mut state = registry_state(stats(0, 1, 1, 1, 0, 0));
        state.uptime = 9;
        put_state(10, state);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::AddValidator { validator: 10 },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert!(DcfPallet::validator_set().contains(&10));
        assert_eq!(DcfPallet::validator_uptime(&10), 9);
        assert_eq!(DcfPallet::validator_join_time(&10), None);
    });
}

#[test]
fn add_validator_proposal_leaves_a_seated_validator_alone() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::AddValidator { validator: 1 },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert_eq!(DcfPallet::validator_set().to_vec(), vec![1, 2, 3]);
        assert!(dcf_events()
            .iter()
            .all(|event| !matches!(event, crate::Event::ValidatorAdded { .. })));
        // A refused action leaves the proposal outside the executed status.
        assert_ne!(
            crate::Proposals::<Test>::get(0).map(|p| p.status),
            Some(crate::ProposalStatus::Executed)
        );
    });
}

#[test]
fn add_validator_proposal_refuses_a_candidate_below_the_minimum_stake() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::AddValidator { validator: 10 },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert!(!DcfPallet::validator_set().contains(&10));
        assert!(DcfPallet::validator_states(&10).is_none());
        assert!(dcf_events()
            .iter()
            .all(|event| !matches!(event, crate::Event::ValidatorAdded { .. })));
        // A refused action leaves the proposal outside the executed status.
        assert_ne!(
            crate::Proposals::<Test>::get(0).map(|p| p.status),
            Some(crate::ProposalStatus::Executed)
        );
    });
}

#[test]
fn eject_proposal_releases_the_stake_and_reports_the_reason() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_eq!(Balances::reserved_balance(1), 1000);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::Eject {
                validator: 1,
                reason: crate::EjectionReason::ManualEjection,
            },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(1), 0);
        assert_eq!(Balances::reserved_balance(1), 0);
        assert!(!pallet_cerulea_pos::Validators::<Test>::contains_key(1));
        System::assert_has_event(RuntimeEvent::DcfPallet(crate::Event::ValidatorEjected {
            validator: 1,
            reason: crate::EjectionReason::ManualEjection,
        }));
    });
}

#[test]
fn remove_proposal_releases_the_stake_and_reports_the_removal() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::RemoveValidator { validator: 2 },
            crate::ProposalStatus::Approved,
        );

        assert_ok!(DcfPallet::execute_proposal(RuntimeOrigin::root(), 0));

        assert_eq!(pallet_cerulea_pos::Stake::<Test>::get(2), 0);
        assert_eq!(Balances::reserved_balance(2), 0);
        assert!(!pallet_cerulea_pos::Validators::<Test>::contains_key(2));
        System::assert_has_event(RuntimeEvent::DcfPallet(crate::Event::ValidatorRemoved {
            validator: 2,
        }));
    });
}

// ---------------------------------------------------------------------------
// Epoch transition: pending actions and economic impact
// ---------------------------------------------------------------------------

#[test]
fn epoch_transition_applies_a_leave_for_an_active_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        TestValidatorRegistry::add_pending_action(&3, crate::ValidatorAction::Leave);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2]);
        let reading = last_transition();
        assert_eq!(reading.removed, vec![3]);
        assert!(reading.added.is_empty());
        assert!(TestValidatorRegistry::get_pending_actions().is_empty());
    });
}

#[test]
fn epoch_transition_drops_a_leave_for_an_inactive_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        TestValidatorRegistry::add_pending_action(&10, crate::ValidatorAction::Leave);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2, 3]);
        assert!(last_transition().removed.is_empty());
        assert!(TestValidatorRegistry::get_pending_actions().is_empty());
    });
}

#[test]
fn epoch_transition_drops_a_join_for_an_active_validator() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        TestValidatorRegistry::add_pending_action(&1, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2, 3]);
        assert!(last_transition().added.is_empty());
        assert!(TestValidatorRegistry::get_pending_actions().is_empty());
    });
}

#[test]
fn epoch_transition_seats_a_qualified_joiner_with_free_seats() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_state(10, registry_state(stats(0, 1, 1, 500, 0, 0)));
        TestValidatorRegistry::add_pending_action(&10, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        // The active set holds 3 of 100 seats. The joiner scores 500.
        assert_eq!(active(), vec![1, 2, 3, 10]);
        assert_eq!(last_transition().added, vec![10]);
    });
}

#[test]
fn epoch_transition_swaps_a_leaver_for_a_joiner() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_state(10, registry_state(stats(0, 1, 1, 500, 0, 0)));
        TestValidatorRegistry::add_pending_action(&3, crate::ValidatorAction::Leave);
        TestValidatorRegistry::add_pending_action(&10, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2, 10]);
        let reading = last_transition();
        assert_eq!(reading.removed, vec![3]);
        assert_eq!(reading.added, vec![10]);
    });
}

#[test]
fn epoch_transition_rejects_a_joiner_below_the_minimum_score() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_state(10, registry_state(stats(0, 1, 1, 10, 0, 0)));
        TestValidatorRegistry::add_pending_action(&3, crate::ValidatorAction::Leave);
        TestValidatorRegistry::add_pending_action(&10, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2]);
        assert!(last_transition().added.is_empty());
    });
}

#[test]
fn epoch_transition_rejects_a_joiner_without_a_state_record() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        TestValidatorRegistry::add_pending_action(&3, crate::ValidatorAction::Leave);
        TestValidatorRegistry::add_pending_action(&11, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        assert_eq!(active(), vec![1, 2]);
        assert!(last_transition().added.is_empty());
    });
}

#[test]
fn epoch_transition_fills_the_last_seat_with_the_highest_scorer() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        let mut seated: Vec<u64> = vec![1, 2, 3];
        seated.extend(1000u64..1096);
        assert_eq!(seated.len(), 99);
        TestValidatorRegistry::update_active_validators(seated);

        put_state(2001, registry_state(stats(0, 1, 1, 900, 0, 0)));
        put_state(2002, registry_state(stats(0, 1, 1, 700, 0, 0)));
        TestValidatorRegistry::add_pending_action(&2002, crate::ValidatorAction::Join);
        TestValidatorRegistry::add_pending_action(&2001, crate::ValidatorAction::Join);

        DcfPallet::handle_epoch_transition();

        // One seat remains under the ceiling of 100. The score 900 takes it.
        let now = active();
        assert_eq!(now.len(), 100);
        assert!(now.contains(&2001));
        assert!(!now.contains(&2002));
        assert_eq!(last_transition().added, vec![2001]);
    });
}

#[test]
fn epoch_transition_reports_zero_economic_impact_without_proposals() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        DcfPallet::handle_epoch_transition();

        let reading = last_transition();
        assert_eq!(reading.rewards, 0);
        assert_eq!(reading.slashed, 0);
    });
}

#[test]
fn epoch_transition_totals_executed_rewards_and_slashes() {
    new_test_ext().execute_with(|| {
        System::set_block_number(1);
        put_proposal(
            0,
            crate::ProposalAction::<Test>::Reward {
                validator: 1,
                amount: 300,
            },
            crate::ProposalStatus::Executed,
        );
        put_proposal(
            1,
            crate::ProposalAction::<Test>::RewardMultiple {
                validators: BoundedVec::truncate_from(vec![1u64, 2u64]),
                amount: 50,
            },
            crate::ProposalStatus::Executed,
        );
        put_proposal(
            2,
            crate::ProposalAction::<Test>::Slash {
                validator: 3,
                amount: 70,
            },
            crate::ProposalStatus::Executed,
        );
        put_proposal(
            3,
            crate::ProposalAction::<Test>::Eject {
                validator: 2,
                reason: crate::EjectionReason::ManualEjection,
            },
            crate::ProposalStatus::Executed,
        );
        // Proposals awaiting execution carry no economic impact.
        put_proposal(
            4,
            crate::ProposalAction::<Test>::Reward {
                validator: 1,
                amount: 999,
            },
            crate::ProposalStatus::Pending,
        );
        put_proposal(
            5,
            crate::ProposalAction::<Test>::Slash {
                validator: 1,
                amount: 999,
            },
            crate::ProposalStatus::Approved,
        );

        DcfPallet::handle_epoch_transition();

        let reading = last_transition();
        // 300 plus two rewards of 50.
        assert_eq!(reading.rewards, 400);
        assert_eq!(reading.slashed, 70);
    });
}

// ---------------------------------------------------------------------------
// Misbehaviour reports
// ---------------------------------------------------------------------------

#[test]
fn misbehaviour_readers_answer_empty_without_reports() {
    new_test_ext().execute_with(|| {
        assert_eq!(DcfPallet::get_misbehavior_report_count(&1), 0);
        assert!(DcfPallet::get_misbehavior_reporters(&1).is_empty());
        assert_eq!(DcfPallet::get_misbehavior_evidence(&1, &2), None);
    });
}

#[test]
fn misbehaviour_readers_answer_every_report() {
    new_test_ext().execute_with(|| {
        report(1, 4, b"double seal at 12");
        report(1, 5, b"double seal at 13");
        report(2, 4, b"late seal");

        assert_eq!(DcfPallet::get_misbehavior_report_count(&1), 2);
        assert_eq!(DcfPallet::get_misbehavior_report_count(&2), 1);

        let mut reporters = DcfPallet::get_misbehavior_reporters(&1);
        reporters.sort();
        assert_eq!(reporters, vec![4, 5]);

        assert_eq!(
            DcfPallet::get_misbehavior_evidence(&1, &5),
            Some(b"double seal at 13".to_vec())
        );
        assert_eq!(DcfPallet::get_misbehavior_evidence(&2, &5), None);
    });
}

#[test]
fn at_risk_starts_one_report_below_the_threshold() {
    new_test_ext().execute_with(|| {
        let threshold: u32 = <Test as crate::Config>::MisbehaviorSlashThreshold::get();
        assert_eq!(threshold, 3);

        assert!(!DcfPallet::is_validator_at_risk(&1));
        report(1, 4, b"first");
        assert!(!DcfPallet::is_validator_at_risk(&1));
        report(1, 5, b"second");
        assert!(DcfPallet::is_validator_at_risk(&1));
        report(1, 6, b"third");
        assert!(DcfPallet::is_validator_at_risk(&1));

        // Reports against one validator leave the others clear.
        assert!(!DcfPallet::is_validator_at_risk(&2));
    });
}
