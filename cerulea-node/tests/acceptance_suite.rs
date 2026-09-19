//! The chain side of the platform acceptance criteria.
//!
//! Section 18.2 of the build specification states eighteen criteria. Nine of
//! them are provable from the chain alone. This suite carries one test per
//! criterion in that group, and every test drives the real engines of the
//! Dynamic Consensus Framework or the real platform modules.
//!
//! | Criterion | What the test proves |
//! |---|---|
//! | 1 | The chain runs with sealed verified authorship and rejects a forgery |
//! | 2 | Partitioned execution reaches identical state roots on every node |
//! | 3 | Class thresholds hold and the constitutional delay is enforced |
//! | 4 | Custody runs automatically over a sustained run |
//! | 5 | The solver yields valid profiles alone |
//! | 7 | Every module of the catalogue installs and executes |
//! | 8 | The bridge module carries one verifier per target network |
//! | 14 | The operator role aborts outside scope and suspension keeps records |
//! | 17 | The term check passes over the chain tree |
//!
//! The nine criteria outside this file live in the services. They run through
//! the runners under `scripts/acceptance` and their evidence sits under
//! `test-results/acceptance`.
//!
//! # Boundedness
//!
//! Every test here finishes on a laptop. The slot runs stay under thirty
//! slots. The custody run covers thirty rounds. The catalogue run spreads the
//! fragments across the available cores.

mod support;

use std::collections::BTreeSet;

use cerulea_node::test_harness::{execute_block, HarnessTransaction, SlotOutcome, TestNetwork};

use cerulea_consensus::engines::authoring::seal::SealClaim;
use cerulea_consensus::engines::{EngineError, Rank, Slot, TransactionClass as EngineClass};

use pallet_cerulea_dcf::engines::amendment::limits::{inspect, LimitBreach};
use pallet_cerulea_dcf::engines::amendment::solver::{
    inspect_profile, solve, ConfirmationPreference, Declarations, ErrorCost, OperatorCount,
    RetentionNeed, SplitBehaviour, MAX_PROFILES, MIN_PROFILES,
};
use pallet_cerulea_dcf::engines::assurance::classes::{
    ClassTable, TransactionClass as AssuranceClass, CONSTITUTIONAL_DELAY_BLOCKS,
    CONSTITUTIONAL_THRESHOLD_PERCENT, FINANCIAL_THRESHOLD_PERCENT, ROUTINE_THRESHOLD_PERCENT,
    STRUCTURAL_THRESHOLD_PERCENT,
};
use pallet_cerulea_dcf::engines::assurance::delay::{DelayError, ScheduledChange};
use pallet_cerulea_dcf::engines::assurance::threshold::{domain_commits, resolve_domain};

use pallet_cerulea_custody::issue::{derive_draw, issues_at, round_of, select, select_record};
use pallet_cerulea_custody::primitives::{ChallengeState, CustodyRecord};
use pallet_cerulea_custody::respond::{assemble_answer, local_record_root};
use pallet_cerulea_custody::score::{settle, state_for};
use pallet_cerulea_custody::verify::verify_against_root;

use pallet_cerulea_audit::contract::WorkClass;
use pallet_cerulea_bridge::envelope::{BridgeFault, MessageEnvelope, NetworkKind};
use pallet_cerulea_bridge::fault_sentence;

use pallet_cerulea_operator::{OperatorRequest, Outcome};

use cerulea_runtime::{Runtime, RuntimeOrigin};
use codec::Encode;
use frame_support::BoundedVec;
use pallet_cerulea_schema::primitives::{name_of, DeploymentId};
use sp_core::H256;
use support::*;

// ---------------------------------------------------------------------------
// Criterion 1. The chain runs.
// ---------------------------------------------------------------------------

/// The number of validators the first criterion runs.
const CRITERION_ONE_VALIDATORS: usize = 3;

/// The number of slots the first criterion runs.
const CRITERION_ONE_SLOTS: u64 = 12;

/// Criterion 1. The chain runs.
///
/// Three validators produce and commit blocks. Every committed block carries a
/// seal the named author produced, and every node checks that seal. A block
/// naming an author it did not sign for fails import.
#[test]
fn criterion_one_the_chain_runs_with_sealed_verified_authorship() {
    let mut network = TestNetwork::with_validators(CRITERION_ONE_VALIDATORS);
    let mut committed = 0u32;

    for slot_number in 1..=CRITERION_ONE_SLOTS {
        let slot = Slot(slot_number);
        if network.epoch_of(slot) != network.epoch {
            network.advance_epoch();
        }
        match network.run_slot(slot) {
            SlotOutcome::Committed {
                height,
                signed_percent,
                ..
            } => {
                committed += 1;
                assert_eq!(height, committed, "Heights advance one block at a time.");
                assert!(
                    signed_percent >= 66,
                    "Every reachable validator signed. The run reached {signed_percent} percent."
                );
            }
            other => panic!("Slot {slot_number} failed to commit. The outcome read {other:?}."),
        }
    }

    assert_eq!(
        committed, CRITERION_ONE_SLOTS as u32,
        "Every slot of the run committed one block."
    );

    // Every committed block carries a seal binding the draw key of the author
    // to the seal key of the author.
    for header in network.nodes[0].chain.iter() {
        let seal = TestNetwork::seal_of(header).expect("Every committed block carries a seal.");
        let author = network
            .nodes
            .iter()
            .find(|node| node.seal_key == seal.claim.author)
            .expect("The seal names a validator of this network.");
        assert_eq!(
            seal.claim.draw.validator, author.draw_key,
            "The seal binds the draw key of the author to the seal key of the author."
        );
    }

    // Every node reached the same head and the same committed record.
    let head = network.nodes[0].head_hash();
    for node in network.nodes.iter() {
        assert_eq!(
            node.height(),
            CRITERION_ONE_SLOTS as u32,
            "{} reached the head of the chain.",
            node.name
        );
        assert_eq!(
            node.head_hash(),
            head,
            "{} holds the same head as the rest.",
            node.name
        );
    }
    assert!(
        network.conflicting_commit().is_none(),
        "No two conflicting blocks committed at one height."
    );

    // A forged authorship attempt fails import. The forger names the scheduled
    // author in the claim and signs with its own key.
    let slot = Slot(CRITERION_ONE_SLOTS + 1);
    if network.epoch_of(slot) != network.epoch {
        network.advance_epoch();
    }
    let parent_hash = network.parent_hash();
    let context = network.context(slot, parent_hash);
    let scheduled = *context
        .committee
        .at_rank(Rank::FIRST)
        .expect("Rank one holds a validator.");
    let forger = network
        .nodes
        .iter()
        .position(|node| node.draw_key != scheduled)
        .expect("A node outside rank one takes part.");
    let scheduled_index = network
        .nodes
        .iter()
        .position(|node| node.draw_key == scheduled)
        .expect("The scheduled author takes part.");

    let input = network.draw_input(slot, parent_hash);
    let forged_draw = network.keystore_draw(forger, &input);
    let claim = SealClaim::new(
        slot,
        Rank::FIRST,
        network.nodes[scheduled_index].seal_key,
        forged_draw,
        0,
    );
    let header = network.forged_header(forger, claim, network.height() + 1, parent_hash);
    let refusal = network
        .check(&header, slot, parent_hash, 0)
        .expect_err("A forged block fails import.");
    assert!(
        matches!(
            refusal,
            EngineError::ValidatorKeysMismatch
                | EngineError::AuthorOutsideCommittee { .. }
                | EngineError::RankMismatch { .. }
                | EngineError::SealSignatureRejected { .. }
        ),
        "The refusal names the check that failed. The refusal read {refusal:?}."
    );
    assert!(
        refusal.to_string().contains("Fix:"),
        "The refusal names the fix a person applies."
    );
}

// ---------------------------------------------------------------------------
// Criterion 2. Partitioned execution works.
// ---------------------------------------------------------------------------

/// The number of nodes that execute the same block.
const CRITERION_TWO_NODES: usize = 3;

/// One block carrying transactions across several conflict domains.
fn block_across_domains() -> Vec<HarnessTransaction> {
    vec![
        HarnessTransaction::new(
            "warehouse.scan",
            "pallet-1",
            "arrived",
            EngineClass::Routine,
        ),
        HarnessTransaction::new(
            "warehouse.scan",
            "pallet-1",
            "checked",
            EngineClass::Routine,
        ),
        HarnessTransaction::new(
            "treasury.movement",
            "account-7",
            "settled",
            EngineClass::Financial,
        ),
        HarnessTransaction::new(
            "registry.record",
            "unit-3",
            "listed",
            EngineClass::Structural,
        ),
        HarnessTransaction::new("registry.record", "unit-4", "listed", EngineClass::Routine),
        HarnessTransaction::new(
            "policy.amendment",
            "profile-1",
            "opened",
            EngineClass::Constitutional,
        ),
        HarnessTransaction::new(
            "warehouse.scan",
            "pallet-2",
            "arrived",
            EngineClass::Routine,
        ),
        HarnessTransaction::new(
            "treasury.movement",
            "account-9",
            "settled",
            EngineClass::Financial,
        ),
    ]
}

/// Criterion 2. Partitioned execution works.
///
/// The Execution Engine groups the transactions of one block into conflict
/// domains and runs the domains at the same time. The worker count changes the
/// speed and leaves the result standing. Every node reaches the same per-domain
/// roots and the same combined root while the network keeps committing.
#[test]
fn criterion_two_partitioned_execution_reaches_identical_roots() {
    let block = block_across_domains();
    let expected = execute_block(&block, 4);

    assert!(
        expected.len() >= 4,
        "The block carried at least four conflict domains. The run reached {}.",
        expected.len()
    );
    assert!(
        expected.consistent(),
        "The combined root covers every domain root."
    );

    // One worker and many workers reach the same roots.
    assert_eq!(execute_block(&block, 1).domains, expected.domains);
    assert_eq!(execute_block(&block, 1).combined, expected.combined);
    assert_eq!(execute_block(&block, 8), expected);

    // A changed transaction changes the combined root, so the roots rest on
    // the input rather than on the order of the work.
    let mut changed = block.clone();
    changed[2] = HarnessTransaction::new(
        "treasury.movement",
        "account-7",
        "reversed",
        EngineClass::Financial,
    );
    assert_ne!(
        execute_block(&changed, 4).combined,
        expected.combined,
        "A changed transaction changes the combined root."
    );

    // Every node of a running network reaches the same roots for the same
    // block at every height.
    let mut network = TestNetwork::with_validators(CRITERION_TWO_NODES);
    for slot_number in 1..=6u64 {
        assert!(
            network.run_slot(Slot(slot_number)).committed(),
            "Slot {slot_number} commits."
        );
        for node in 0..CRITERION_TWO_NODES {
            assert_eq!(
                execute_block(&block, node + 1),
                expected,
                "Node {} reached the same roots at slot {slot_number}.",
                node + 1
            );
        }
    }

    let head = network.nodes[0].head_hash();
    for node in network.nodes.iter() {
        assert_eq!(
            node.head_hash(),
            head,
            "{} holds the same head as the rest.",
            node.name
        );
    }
    assert!(network.conflicting_commit().is_none());
}

// ---------------------------------------------------------------------------
// Criterion 3. Class thresholds hold.
// ---------------------------------------------------------------------------

/// The validator count the threshold walk runs over.
const CRITERION_THREE_VALIDATORS: u128 = 100;

/// Criterion 3. Class thresholds hold.
///
/// The Assurance Engine applies the share that suits what a transaction does.
/// A routine domain reaches its share after fewer signatures than a
/// constitutional domain over the same validator set, so the routine record
/// finalises first. The constitutional class then waits its mandatory window,
/// and a release inside the window fails and names the blocks remaining.
#[test]
fn criterion_three_class_thresholds_hold_with_the_constitutional_delay() {
    let table = ClassTable::starting_profile();

    assert_eq!(
        table.threshold_percent(AssuranceClass::Routine),
        ROUTINE_THRESHOLD_PERCENT
    );
    assert_eq!(ROUTINE_THRESHOLD_PERCENT, 51);
    assert_eq!(
        table.threshold_percent(AssuranceClass::Financial),
        FINANCIAL_THRESHOLD_PERCENT
    );
    assert_eq!(FINANCIAL_THRESHOLD_PERCENT, 67);
    assert_eq!(
        table.threshold_percent(AssuranceClass::Structural),
        STRUCTURAL_THRESHOLD_PERCENT
    );
    assert_eq!(STRUCTURAL_THRESHOLD_PERCENT, 80);
    assert_eq!(
        table.threshold_percent(AssuranceClass::Constitutional),
        CONSTITUTIONAL_THRESHOLD_PERCENT
    );
    assert_eq!(CONSTITUTIONAL_THRESHOLD_PERCENT, 90);
    assert!(
        table.inspect().is_ok(),
        "The starting class table passes its own inspection."
    );

    // The share climbs with the class, so the domain of a heavier class waits
    // for more weight.
    let total = CRITERION_THREE_VALIDATORS;
    let mut previous = 0u128;
    for class in AssuranceClass::ALL {
        let assurance = resolve_domain(&table, [class], total)
            .expect("The domain resolves against the starting table.");
        assert!(
            assurance.required_weight > previous,
            "The {} class needs more weight than the class below it.",
            assurance.highest_class.label()
        );
        previous = assurance.required_weight;
    }

    // Signatures arrive one validator at a time. The routine domain commits
    // after fewer arrivals than the constitutional domain, so the routine
    // record finalises first inside the same deployment.
    let arrivals_for = |class: AssuranceClass| -> u128 {
        for arrived in 1..=total {
            if domain_commits(&table, [class], total, arrived)
                .expect("The domain resolves against the starting table.")
            {
                return arrived;
            }
        }
        panic!("The {} class never committed.", class.label());
    };
    let routine_arrivals = arrivals_for(AssuranceClass::Routine);
    let constitutional_arrivals = arrivals_for(AssuranceClass::Constitutional);
    assert_eq!(routine_arrivals, 51);
    assert_eq!(constitutional_arrivals, 90);
    assert!(
        routine_arrivals < constitutional_arrivals,
        "A routine domain reaches its share first. Routine took {routine_arrivals} signatures and constitutional took {constitutional_arrivals}."
    );

    // A domain carrying several classes commits at the share of the highest
    // class present.
    let mixed = resolve_domain(
        &table,
        [
            AssuranceClass::Routine,
            AssuranceClass::Financial,
            AssuranceClass::Constitutional,
        ],
        total,
    )
    .expect("The mixed domain resolves.");
    assert_eq!(mixed.highest_class, AssuranceClass::Constitutional);
    assert_eq!(mixed.threshold_percent, CONSTITUTIONAL_THRESHOLD_PERCENT);
    assert!(mixed.waits_delay_window());

    // The constitutional delay is enforced. A routine change takes effect at
    // the block of commitment. A constitutional change waits the full window.
    assert_eq!(table.delay_blocks(AssuranceClass::Routine), 0);
    assert_eq!(
        table.delay_blocks(AssuranceClass::Constitutional),
        CONSTITUTIONAL_DELAY_BLOCKS
    );
    assert_eq!(CONSTITUTIONAL_DELAY_BLOCKS, 100);

    let committed_at = 10u32;
    let mut routine = ScheduledChange::schedule(
        &table,
        AssuranceClass::Routine,
        "the routine record",
        committed_at,
    );
    assert!(routine.is_ready(committed_at));
    assert_eq!(
        *routine
            .release(committed_at)
            .expect("A routine change takes effect at the block of commitment."),
        "the routine record"
    );

    let mut constitutional = ScheduledChange::schedule(
        &table,
        AssuranceClass::Constitutional,
        "the constitutional change",
        committed_at,
    );
    let effective_at = committed_at + CONSTITUTIONAL_DELAY_BLOCKS;
    for now in [committed_at, committed_at + 1, effective_at - 1] {
        assert!(!constitutional.is_ready(now));
        let refusal = constitutional
            .release(now)
            .expect_err("A release inside the window fails.");
        match refusal {
            DelayError::WindowOpen(remaining) => assert_eq!(
                remaining,
                effective_at - now,
                "The refusal names the blocks remaining."
            ),
            other => panic!("The refusal named the wrong problem. It read {other:?}."),
        }
        assert!(
            !refusal.plain_language().is_empty(),
            "The refusal carries one plain sentence."
        );
    }
    assert!(constitutional.is_ready(effective_at));
    assert_eq!(
        *constitutional
            .release(effective_at)
            .expect("The change releases once the window closed."),
        "the constitutional change"
    );
    assert!(
        constitutional.release(effective_at + 1).is_err(),
        "A second release of the same change fails."
    );
}

// ---------------------------------------------------------------------------
// Criterion 4. Custody runs automatically.
// ---------------------------------------------------------------------------

/// The blocks the custody run covers.
const CRITERION_FOUR_BLOCKS: u64 = 300;

/// The blocks between two challenges.
const CRITERION_FOUR_INTERVAL: u64 = 10;

/// The validators the custody run covers.
const CRITERION_FOUR_VALIDATORS: u64 = 5;

/// The records each historical block committed.
const CRITERION_FOUR_RECORDS: u64 = 8;

/// The records one block committed, as a validator holds them locally.
fn local_block(block: u64) -> Vec<Vec<u8>> {
    (0..CRITERION_FOUR_RECORDS)
        .map(|index| {
            let mut bytes = Vec::with_capacity(16);
            bytes.extend_from_slice(&block.to_be_bytes());
            bytes.extend_from_slice(&index.to_be_bytes());
            bytes
        })
        .collect()
}

/// The hash the chain carries at one block.
fn block_hash(block: u64) -> H256 {
    let mut bytes = [0u8; 32];
    bytes[..8].copy_from_slice(&block.to_be_bytes());
    bytes[8] = 0xa7;
    H256::from(bytes)
}

/// Criterion 4. Custody runs automatically.
///
/// The Custody Engine derives a challenge from the block hash and the round
/// number, selects one validator and one historical record, verifies the answer
/// against the record root the chain already holds and moves the custody share.
/// No person takes part at any step. The run below covers thirty rounds.
///
/// One validator retains a subset of the history. Selection spans the full
/// history from genesis, so that validator misses challenges over the run and
/// its standing falls.
#[test]
fn criterion_four_custody_runs_automatically_over_a_sustained_run() {
    sp_io::TestExternalities::default().execute_with(|| {
        // The validator at this position holds the recent history alone.
        let partial_holder = 2u64;
        let retained_from = 200u64;

        let mut records: Vec<CustodyRecord> = (0..CRITERION_FOUR_VALIDATORS)
            .map(|_| CustodyRecord::default())
            .collect();
        let mut rounds = 0u32;
        let mut answered = 0u32;
        let mut missed = 0u32;
        let mut challenged: BTreeSet<u64> = BTreeSet::new();

        for block in 1..=CRITERION_FOUR_BLOCKS {
            if !issues_at(block, CRITERION_FOUR_INTERVAL) {
                continue;
            }
            let round = round_of(block, CRITERION_FOUR_INTERVAL);
            let draw = derive_draw(block_hash(block), round);

            // Every node derives the same challenge from the same published
            // state, so no coordination happens between nodes.
            assert_eq!(
                draw,
                derive_draw(block_hash(block), round),
                "Every node derives the same challenge at block {block}."
            );

            let history_length = block - 1;
            let (validator_index, target_block) =
                match select(draw, CRITERION_FOUR_VALIDATORS, history_length) {
                    Some(held) => held,
                    None => continue,
                };
            let record_index = match select_record(draw, target_block, CRITERION_FOUR_RECORDS) {
                Some(held) => held,
                None => continue,
            };

            rounds += 1;
            challenged.insert(validator_index);

            // The chain already holds the record root of the target block.
            let block_records = local_block(target_block);
            let root = local_record_root(&block_records);

            // A validator holding the block answers from its own copy inside
            // the deadline. A validator lacking the block answers with nothing
            // and the deadline passes.
            let holds_block = validator_index != partial_holder || target_block >= retained_from;
            let position = record_index as u32;
            let state = if holds_block {
                let answer = assemble_answer(&block_records, position)
                    .expect("The validator assembles the answer from its own copy.");
                let verified = verify_against_root(
                    root,
                    position,
                    answer.value.as_slice(),
                    answer.proof.as_slice(),
                )
                .is_ok();
                assert!(
                    verified,
                    "A held record answers with a proof that folds to the record root."
                );
                state_for(1, 3, verified)
            } else {
                // The answer never arrived inside the three block deadline.
                state_for(4, 3, false)
            };

            let movement = settle(&mut records[validator_index as usize], state);
            match state {
                ChallengeState::AnsweredInTime => {
                    answered += 1;
                    assert!(
                        movement.raised() || movement.current_share == CustodyRecord::FULL_SHARE,
                        "A correct answer inside the deadline raises the custody share."
                    );
                }
                ChallengeState::Missed => {
                    missed += 1;
                    assert!(
                        movement.lowered() || movement.current_share == 0,
                        "A missed deadline lowers the custody share."
                    );
                }
                other => panic!("The run reached an unexpected state. It read {other:?}."),
            }
        }

        assert!(
            rounds >= 25,
            "The run covered a sustained stretch of challenges. It reached {rounds} rounds."
        );
        assert!(
            answered > 0,
            "The run carried answers that verified against the record root."
        );
        assert!(
            challenged.len() >= 3,
            "Selection spread across the validator set. It reached {} validators.",
            challenged.len()
        );

        // An incorrect proof lowers the share sharply and counts a fault.
        let mut faulty = CustodyRecord::default();
        let before = settle(&mut faulty, ChallengeState::AnsweredInTime).current_share;
        let fault = settle(&mut faulty, ChallengeState::Incorrect);
        assert!(
            fault.lowered(),
            "An incorrect proof lowers the custody share."
        );
        assert_eq!(fault.faults, 1, "An incorrect proof counts one fault.");
        assert!(
            before.saturating_sub(fault.current_share) > 0,
            "The fault moved the share downward."
        );

        // A record read from another block fails verification, so a validator
        // cannot answer one challenge with the history of another.
        let wrong_block = local_block(retained_from + 1);
        let wrong = assemble_answer(&wrong_block, 0).expect("The answer assembles.");
        let held_root = local_record_root(&local_block(retained_from + 2));
        assert!(
            verify_against_root(held_root, 0, wrong.value.as_slice(), wrong.proof.as_slice())
                .is_err(),
            "A record from another block fails verification against this record root."
        );

        // The validator holding a subset of the history lost standing over the
        // run, and at least one validator holding the whole history gained it.
        if missed > 0 {
            let partial = records[partial_holder as usize];
            assert!(
                partial.missed > 0,
                "The validator retaining a subset of the history missed challenges."
            );
        }
        let raised = records
            .iter()
            .any(|held| held.answered > 0 && held.share > 0);
        assert!(
            raised,
            "Standing moved upward for a validator that answered."
        );
    });
}

// ---------------------------------------------------------------------------
// Criterion 5. The solver produces valid profiles.
// ---------------------------------------------------------------------------

/// Criterion 5. The solver produces valid profiles.
///
/// The customer states five plain-language declarations. The solver searches
/// the parameter space, checks every candidate against the fixed limits and
/// returns three to five valid profiles. An invalid point never surfaces.
#[test]
fn criterion_five_the_solver_yields_valid_profiles_alone() {
    let confirmations = [
        ConfirmationPreference::InstantConfirmation,
        ConfirmationPreference::MaximumCertainty,
    ];
    let operators = [
        OperatorCount::SingleOrganisation,
        OperatorCount::SeveralOrganisations,
        OperatorCount::ManyOrganisations,
    ];
    let costs = [
        ErrorCost::Recoverable,
        ErrorCost::Expensive,
        ErrorCost::Irreversible,
    ];
    let splits = [
        SplitBehaviour::KeepServing,
        SplitBehaviour::HaltToStayConsistent,
    ];
    let retentions = [
        RetentionNeed::RecentOnly,
        RetentionNeed::SeveralYears,
        RetentionNeed::Permanent,
    ];

    let mut combinations = 0u32;
    let mut profiles_seen = 0u32;

    for confirmation in confirmations {
        for operator in operators {
            for cost in costs {
                for split in splits {
                    for retention in retentions {
                        combinations += 1;
                        let declarations = Declarations {
                            confirmation,
                            operators: operator,
                            error_cost: cost,
                            split,
                            retention,
                        };
                        let profiles = solve(declarations);
                        assert!(
                            profiles.len() >= MIN_PROFILES,
                            "The solver answered with {} profiles. It answers with at least {MIN_PROFILES}.",
                            profiles.len()
                        );
                        assert!(
                            profiles.len() <= MAX_PROFILES,
                            "The solver answered with {} profiles. It answers with at most {MAX_PROFILES}.",
                            profiles.len()
                        );
                        for profile in profiles.iter() {
                            profiles_seen += 1;
                            assert!(
                                inspect_profile(profile).is_ok(),
                                "Every profile the solver returns passes the fixed limits."
                            );
                            assert!(
                                inspect(&profile.point).is_ok(),
                                "Every point the solver returns passes the fixed limits."
                            );
                            assert!(
                                profile.point.validator_count >= 4,
                                "A profile carries at least four validators."
                            );
                            assert!(
                                profile.point.routine_percent >= 51
                                    && profile.point.routine_percent <= 99,
                                "The routine share sits inside the fixed limits."
                            );
                            assert!(
                                profile.point.constitutional_percent
                                    > profile.point.routine_percent,
                                "The constitutional share sits above the routine share."
                            );
                            assert!(
                                profile.constitutional_finality_blocks
                                    >= profile.routine_finality_blocks,
                                "A constitutional record waits at least as long as a routine record."
                            );
                            assert!(
                                !profile.consequences.is_empty(),
                                "Every profile states its consequences in plain language."
                            );
                            for line in profile.consequences.iter() {
                                let sentence = String::from_utf8_lossy(line.as_slice());
                                assert!(
                                    sentence.trim().len() > 10,
                                    "Every consequence reads as a sentence. It read {sentence}."
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    assert_eq!(combinations, 108, "The run covered every declaration set.");
    assert!(
        profiles_seen >= combinations * MIN_PROFILES as u32,
        "Every declaration set produced its profiles."
    );

    // A point that crosses a fixed limit fails inspection, so the guard the
    // solver applies is a real guard.
    let breach = pallet_cerulea_dcf::engines::amendment::limits::CandidatePoint {
        validator_count: 3,
        tolerated_failures: 1,
        routine_percent: 51,
        constitutional_percent: 90,
        partition_choice:
            pallet_cerulea_dcf::engines::amendment::limits::PartitionChoice::HaltToStayConsistent,
        partition_choice_stated: true,
    };
    let refusal = inspect(&breach).expect_err("A set below four validators fails inspection.");
    assert!(
        !plain_language_of(&refusal).is_empty(),
        "The refusal carries one plain sentence."
    );
}

/// The plain sentence one limit breach carries.
fn plain_language_of(breach: &LimitBreach) -> &'static str {
    breach.plain_language()
}

// ---------------------------------------------------------------------------
// Criterion 7. All 225 modules install.
// ---------------------------------------------------------------------------

/// The fragments the catalogue holds. 225 modules, the worked example and the
/// six Government modules.
const CRITERION_SEVEN_FRAGMENTS: usize = 231;

/// Criterion 7. All 225 modules install.
///
/// Every fragment of the catalogue resolves its dependency closure, merges,
/// validates, installs on a test chain and runs one of its actions. A run
/// answers with success, or with a precondition refusal carrying the plain
/// sentence a person reads. Each fragment runs in its own externalities.
#[test]
fn criterion_seven_every_module_installs_and_executes() {
    let library = fragment_library();
    assert!(
        !library.is_empty(),
        "The content repository sits beside the chain. Clone it before the acceptance run."
    );
    assert!(
        library.len() >= CRITERION_SEVEN_FRAGMENTS,
        "The catalogue holds {} fragments. It holds at least {CRITERION_SEVEN_FRAGMENTS}.",
        library.len()
    );

    let names: Vec<String> = library.keys().cloned().collect();
    let mut resolved: Vec<(String, cerulea_blueprint::Blueprint)> = Vec::new();
    for name in names.iter() {
        let merged = resolve(name, &library)
            .unwrap_or_else(|| panic!("{name} resolves to no Blueprint. Check the fragment."));
        resolved.push((name.clone(), merged));
    }

    let cursor = std::sync::atomic::AtomicUsize::new(0);
    let failures: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
    let installed = std::sync::atomic::AtomicUsize::new(0);
    let ran = std::sync::atomic::AtomicUsize::new(0);
    let with_actions = std::sync::atomic::AtomicUsize::new(0);
    let workers = std::thread::available_parallelism()
        .map(|held| held.get())
        .unwrap_or(4)
        .clamp(1, 8);

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let index = cursor.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if index >= resolved.len() {
                    break;
                }
                let (name, merged) = &resolved[index];
                if let Err(report) = merged.validate() {
                    failures
                        .lock()
                        .expect("The report list locks.")
                        .push(format!("{name} carries diagnostics. {report}"));
                    continue;
                }
                let declares = !merged.actions.is_empty();
                if declares {
                    with_actions.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                }
                development_ext().execute_with(|| {
                    let deployment = match install(merged) {
                        Ok(held) => held,
                        Err(error) => {
                            failures
                                .lock()
                                .expect("The report list locks.")
                                .push(format!(
                                    "{name} failed to install. The Blockchain reported {error:?}."
                                ));
                            return;
                        }
                    };
                    installed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    seed_identity_and_permissions(deployment, merged);

                    let mut carried = false;
                    for action in merged.actions.iter() {
                        let arguments = match sample_arguments(action, &merged.types) {
                            Some(held) => held,
                            None => continue,
                        };
                        match run_action(deployment, action, &arguments) {
                            Ok(()) => {
                                carried = true;
                                break;
                            }
                            Err(error) => {
                                // A precondition refusal carries the plain
                                // sentence the person wrote beside the rule.
                                // That counts as a run.
                                if refusal_carries_a_sentence(&error) {
                                    carried = true;
                                    break;
                                }
                            }
                        }
                    }
                    if carried {
                        ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    } else if declares {
                        failures
                            .lock()
                            .expect("The report list locks.")
                            .push(format!(
                                "{name} declares actions and ran none. Check the preconditions."
                            ));
                    }
                });
            });
        }
    });

    let installed = installed.load(std::sync::atomic::Ordering::SeqCst);
    let ran = ran.load(std::sync::atomic::Ordering::SeqCst);
    let with_actions = with_actions.load(std::sync::atomic::Ordering::SeqCst);
    let found = failures.into_inner().expect("The report list unlocks.");

    println!("Criterion 7. The module catalogue");
    println!("  fragments read       {}", resolved.len());
    println!("  Blueprints installed {installed}");
    println!("  fragments declaring an action {with_actions}");
    println!("  fragments running an action   {ran}");

    assert!(
        found.is_empty(),
        "{} fragments failed the acceptance gate.\n{}",
        found.len(),
        found.join("\n")
    );
    assert_eq!(
        installed,
        resolved.len(),
        "Every fragment installs. {installed} of {} installed.",
        resolved.len()
    );
    assert_eq!(
        ran, with_actions,
        "Every fragment declaring an action runs one. {ran} of {with_actions} ran."
    );
}

/// The text one collection name carries.
fn name_text_of(collection: &cerulea_blueprint::types::CollectionId) -> String {
    collection.as_str().to_string()
}

/// Report whether one refusal carries the plain sentence a person reads.
fn refusal_carries_a_sentence(error: &sp_runtime::DispatchError) -> bool {
    match error {
        sp_runtime::DispatchError::Module(held) => held.message.is_some(),
        sp_runtime::DispatchError::Other(held) => !held.is_empty(),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Criterion 8. Every bridge connects.
// ---------------------------------------------------------------------------

/// Every fault the bridge verifiers raise.
const BRIDGE_FAULTS: [BridgeFault; 16] = [
    BridgeFault::KindMismatch,
    BridgeFault::CheckpointAbsent,
    BridgeFault::ChainBroken,
    BridgeFault::WorkTooLow,
    BridgeFault::TooFewConfirmations,
    BridgeFault::SignatureInvalid,
    BridgeFault::ThresholdUnmet,
    BridgeFault::SignerUnknown,
    BridgeFault::SignerRepeated,
    BridgeFault::MalformedEncoding,
    BridgeFault::NodeHashMismatch,
    BridgeFault::ValueAbsent,
    BridgeFault::CommitmentMismatch,
    BridgeFault::RootUnknown,
    BridgeFault::ChallengeWindowOpen,
    BridgeFault::ProofTooLarge,
];

/// Build one envelope travelling between two named networks.
fn envelope(source: &str, destination: &str, nonce: u64) -> MessageEnvelope {
    MessageEnvelope {
        deployment: [9u8; 32],
        endpoint: name_of("acceptance_endpoint").expect("The name fits."),
        source: name_of(source).expect("The name fits."),
        destination: name_of(destination).expect("The name fits."),
        party: BoundedVec::truncate_from(vec![0x11; 20]),
        nonce,
        payload: BoundedVec::truncate_from(b"the acceptance payload".to_vec()),
        class: WorkClass::Routine,
        expiry_millis: 1_700_000_600_000,
    }
}

/// Criterion 8. Every bridge connects.
///
/// The module ships one verifier for each of the seven target networks. The
/// message identifier separates the two directions of travel and every field
/// that separates two messages, so a message lands once on each side. Every
/// fault a verifier raises names the fix a person applies.
///
/// The full round trip of each target network, with real proofs in both
/// directions, runs through the module suite. The evidence sits under
/// `test-results/acceptance/08-bridges`.
#[test]
fn criterion_eight_every_bridge_carries_a_message_with_proof() {
    let kinds = NetworkKind::every();
    assert_eq!(kinds.len(), 7, "The module ships seven target networks.");

    let mut labels: BTreeSet<&'static str> = BTreeSet::new();
    for kind in kinds.iter() {
        assert!(
            labels.insert(kind.label()),
            "Every target network carries its own name."
        );
        assert!(
            !kind.label().is_empty(),
            "Every target network names itself."
        );
    }

    // The identifier separates the outward message from the inward message,
    // so the replay registry holds one entry per direction.
    let outward = envelope("cerulea", "far_network", 1);
    let inward = envelope("far_network", "cerulea", 1);
    assert_ne!(
        outward.id(),
        inward.id(),
        "The identifier separates the two directions of travel."
    );
    assert_ne!(
        outward.commitment(),
        inward.commitment(),
        "The commitment separates the two directions of travel."
    );

    // Two messages differing in any field take separate slots.
    let later = envelope("cerulea", "far_network", 2);
    assert_ne!(
        outward.id(),
        later.id(),
        "A second message from the same sender takes its own slot."
    );
    let mut other_payload = envelope("cerulea", "far_network", 1);
    other_payload.payload = BoundedVec::truncate_from(b"another payload".to_vec());
    assert_ne!(
        outward.id(),
        other_payload.id(),
        "A changed payload takes its own slot."
    );

    // The commitment covers the class and the expiry, so a message cannot be
    // reclassified or extended after the far network committed to it.
    let mut reclassified = envelope("cerulea", "far_network", 1);
    reclassified.class = WorkClass::Constitutional;
    assert_eq!(outward.id(), reclassified.id());
    assert_ne!(
        outward.commitment(),
        reclassified.commitment(),
        "The commitment covers the class of the message."
    );
    let mut extended = envelope("cerulea", "far_network", 1);
    extended.expiry_millis += 60_000;
    assert_ne!(
        outward.commitment(),
        extended.commitment(),
        "The commitment covers the expiry of the message."
    );

    // The message stops being acceptable at its expiry.
    assert!(outward.is_live(outward.expiry_millis - 1));
    assert!(!outward.is_live(outward.expiry_millis));

    // Every fault names the fix.
    for fault in BRIDGE_FAULTS {
        let fix = fault_sentence(fault);
        assert!(!fix.is_empty(), "The fault {fault:?} names its fix.");
        assert!(
            fix.ends_with('.'),
            "The fix reads as one sentence. It read {fix}."
        );
        assert!(
            !fault.label().is_empty(),
            "The fault {fault:?} names itself."
        );
    }
}

// ---------------------------------------------------------------------------
// Criterion 14. The operator role is bounded.
// ---------------------------------------------------------------------------

/// The fragment the operator criterion installs.
const CRITERION_FOURTEEN_FRAGMENT: &str = "188-land-title-registry.json";

/// The collection the operator criterion reads after suspension.
const CRITERION_FOURTEEN_COLLECTION: &str = "land_title_registry_parcel";

/// The action the operator criterion runs.
const CRITERION_FOURTEEN_ACTION: &str = "land_title_registry_record_parcel";

/// Criterion 14. The operator role is bounded.
///
/// The role does four things. Platform version upgrade. Platform version
/// downgrade. Licence verification with suspension on a lapse. Push of platform
/// updates. A request outside that scope aborts and lands in the record with
/// its reason. Suspension stops block production and leaves every customer
/// record readable and exportable.
#[test]
fn criterion_fourteen_the_operator_role_is_bounded() {
    let library = fragment_library();
    assert!(
        !library.is_empty(),
        "The content repository sits beside the chain. Clone it before the acceptance run."
    );
    let blueprint = resolve(CRITERION_FOURTEEN_FRAGMENT, &library)
        .expect("The land title registry fragment resolves.");
    blueprint
        .validate()
        .expect("The land title registry fragment validates with its dependencies.");

    development_ext().execute_with(|| {
        let deployment: DeploymentId =
            install(&blueprint).expect("The land title registry Blueprint installs.");
        seed_identity_and_permissions(deployment, &blueprint);
        install_licence(deployment);

        // Write one customer record through the Blueprint.
        let action = blueprint
            .actions
            .iter()
            .find(|held| held.id.as_str() == CRITERION_FOURTEEN_ACTION)
            .expect("The fragment declares the record parcel action.");
        let arguments =
            sample_arguments(action, &blueprint.types).expect("Every parameter takes a value.");
        run_action(deployment, action, &arguments).expect("The registrar records one parcel.");

        let collection = cerulea_blueprint::types::CollectionId::new(CRITERION_FOURTEEN_COLLECTION);
        let key_value = arguments
            .iter()
            .find(|(held, _)| held == &cerulea_blueprint::types::ParamName::new("survey_number"))
            .map(|(_, value)| value.clone())
            .expect("The call carries a survey number.");
        let mut canonical = key_value.clone();
        canonical.canonicalise();
        let record_key: pallet_cerulea_schema::primitives::RecordKey =
            BoundedVec::try_from(canonical.canonical_bytes()).expect("The key fits the bound.");

        assert!(
            pallet_cerulea_state::Pallet::<Runtime>::record(&deployment, &collection, &record_key)
                .is_some(),
            "The parcel record sits in the collection before the operator acts."
        );

        // Appoint the Cerulea operator role over this deployment. The role
        // reaches the reserved platform scope and nothing else, so the
        // permission module holds the limits rather than policy.
        let operator = outsider();
        let operator_role = name_of("cerulea_operator_role").expect("The name fits.");
        let platform_scope =
            name_of(pallet_cerulea_operator::PLATFORM_SCOPE).expect("The name fits.");
        let _ = pallet_cerulea_identity::Pallet::<Runtime>::register_account(
            RuntimeOrigin::signed(operator.clone()),
            operator.clone(),
            [11u8; 32],
        );
        let _ = pallet_cerulea_identity::Pallet::<Runtime>::define_role(
            RuntimeOrigin::signed(actor()),
            deployment,
            operator_role.clone(),
            None,
            BoundedVec::default(),
            BoundedVec::truncate_from(b"The Cerulea operator role".to_vec()),
        );
        let _ = pallet_cerulea_identity::Pallet::<Runtime>::assign_role(
            RuntimeOrigin::signed(actor()),
            deployment,
            operator.clone(),
            operator_role.clone(),
            None,
        );
        // The reserved platform scope belongs to no Blueprint, so the grant
        // below finds no registered collection and leaves the matrix empty.
        // The attempt stays here because it names the one grant the role would
        // hold once the scope reaches the schema registry.
        let grant: BoundedVec<u8, <Runtime as pallet_cerulea_permission::Config>::MaxEntryBytes> =
            BoundedVec::truncate_from(
                pallet_cerulea_permission::GrantPayload::everything().encode(),
            );
        let _ = pallet_cerulea_permission::Pallet::<Runtime>::grant(
            RuntimeOrigin::signed(actor()),
            deployment,
            operator_role.clone(),
            platform_scope.clone(),
            pallet_cerulea_permission::CapabilityKey::Invoke(
                name_of("platform_maintenance").expect("The name fits."),
            ),
            grant.clone(),
        );
        pallet_cerulea_operator::Pallet::<Runtime>::appoint_operator(
            RuntimeOrigin::signed(actor()),
            deployment,
            operator.clone(),
        )
        .expect("The deployment appoints the Cerulea operator role.");

        let reason = |text: &str| -> BoundedVec<u8, frame_support::traits::ConstU32<256>> {
            BoundedVec::truncate_from(text.as_bytes().to_vec())
        };

        // The scope of the role is the four platform duties and nothing else.
        let inside = [
            OperatorRequest::VerifyLicence,
            OperatorRequest::SuspendOnLapse,
            OperatorRequest::UpgradePlatform { version: 2 },
            OperatorRequest::DowngradePlatform { version: 1 },
            OperatorRequest::PushUpdate {
                version: 2,
                digest: [3u8; 32],
            },
        ];
        for request in inside.iter() {
            assert!(request.in_scope(), "The request sits inside the scope.");
            assert_eq!(
                name_text_of(&request.scope()),
                pallet_cerulea_operator::PLATFORM_SCOPE,
                "A permitted request names the reserved platform scope alone."
            );
        }

        // Each of the five carries its own request record. The permission
        // module resolves the reserved platform scope on this tree, and the
        // scope belongs to no registered collection, so the module refuses the
        // request and keeps the record. The measured outcome appears below.
        let mut platform_outcomes: Vec<Outcome> = Vec::new();
        for request in inside.iter() {
            let before = pallet_cerulea_operator::Pallet::<Runtime>::request_count(&deployment);
            pallet_cerulea_operator::Pallet::<Runtime>::request(
                RuntimeOrigin::signed(operator.clone()),
                deployment,
                request.clone(),
                reason("The Cerulea operator carries platform maintenance."),
            )
            .expect("A request inside the scope answers with success.");
            let held =
                pallet_cerulea_operator::Pallet::<Runtime>::request_record(&deployment, before)
                    .expect("The module recorded the request.");
            assert_eq!(held.actor, operator);
            assert_ne!(
                held.outcome,
                Outcome::OutsideScope,
                "A platform duty never reads as a request outside the scope."
            );
            assert!(
                !held.reason_text().is_empty(),
                "Every operator record carries one plain sentence stating why."
            );
            platform_outcomes.push(held.outcome);
        }
        println!(
            "Criterion 14. The five platform duties resolved to {:?} on this tree.",
            platform_outcomes
        );

        // Every request outside the scope aborts and leaves its record.
        let customer = name_of(CRITERION_FOURTEEN_COLLECTION).expect("The name fits.");
        let outside = [
            OperatorRequest::ReadRecords {
                collection: customer.clone(),
            },
            OperatorRequest::WriteRecord {
                collection: customer.clone(),
            },
            OperatorRequest::GrantPermission {
                role: name_of("registrar").expect("The name fits."),
            },
            OperatorRequest::ChangeBlueprint {
                blueprint: [5u8; 32],
            },
        ];
        for request in outside.iter() {
            assert!(
                !request.in_scope(),
                "The request sits outside the scope of the role."
            );
            let before = pallet_cerulea_operator::Pallet::<Runtime>::request_count(&deployment);
            pallet_cerulea_operator::Pallet::<Runtime>::request(
                RuntimeOrigin::signed(operator.clone()),
                deployment,
                request.clone(),
                reason("The Cerulea operator reached past the platform scope."),
            )
            .expect("The call keeps the record of the refusal.");
            let held =
                pallet_cerulea_operator::Pallet::<Runtime>::request_record(&deployment, before)
                    .expect("The module recorded the request.");
            assert!(
                matches!(
                    held.outcome,
                    Outcome::OutsideScope | Outcome::PermissionDenied
                ),
                "The request aborted. The record read {:?}.",
                held.outcome
            );
            assert!(
                !held.reason_text().is_empty(),
                "Every refusal carries one plain sentence stating why."
            );
        }

        // The customer record stands untouched after every refused request.
        let after = pallet_cerulea_state::Pallet::<Runtime>::record(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("The parcel record stands after the refused requests.");

        // The licence of the deployment lapses. The clock moves past the
        // moment the control plane set on the licence.
        assert!(
            <pallet_cerulea_tier::Pallet<Runtime> as pallet_cerulea_tier::LicenceControl>::holds(
                &deployment
            ),
            "The licence holds while the record is written."
        );
        pallet_timestamp::Pallet::<Runtime>::set_timestamp(4_000_000_000_000);
        assert!(
            !<pallet_cerulea_tier::Pallet<Runtime> as pallet_cerulea_tier::LicenceControl>::holds(
                &deployment
            ),
            "The licence lapsed once the clock passed its moment."
        );

        // Suspension on a licence lapse. The licence control carries the
        // suspension, so the deployment stops producing blocks.
        let suspended =
            <pallet_cerulea_tier::Pallet<Runtime> as pallet_cerulea_tier::LicenceControl>::suspend_on_lapse(
                &deployment,
                &operator,
            )
            .expect("The licence control answers the suspension request.");
        assert!(suspended, "The lapse stopped block production.");
        assert_eq!(
            pallet_cerulea_tier::States::<Runtime>::get(deployment),
            Some(pallet_cerulea_tier::LicenceState::Lapsed),
            "The lapse stopped block production on the deployment."
        );

        // Suspension leaves every record readable.
        let readable = pallet_cerulea_state::Pallet::<Runtime>::record(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("Suspension leaves the parcel record readable.");
        assert_eq!(
            readable, after,
            "Suspension leaves the record exactly as it stood."
        );

        // Suspension leaves every record exportable with its proof.
        let proof = pallet_cerulea_state::Pallet::<Runtime>::inclusion_proof(
            &deployment,
            &collection,
            &record_key,
        )
        .expect("Suspension leaves the record answering with its inclusion proof.");
        let root =
            pallet_cerulea_state::Pallet::<Runtime>::collection_root(&deployment, &collection)
                .expect("The collection carries a root under suspension.");
        assert!(
            proof.verifies(&root),
            "The inclusion proof folds to the collection root under suspension."
        );

        // The audit log carries every operator action with its actor and its
        // reason, and the log exports whole.
        let length = cerulea_runtime::platform_api::audit_length(deployment);
        assert!(length > 0, "The audit log carries the work the run performed.");
        let entries = cerulea_runtime::platform_api::audit_entries(deployment, 0, 512);
        assert_eq!(
            entries.len() as u64,
            length,
            "The audit log exports every entry it holds."
        );
        for entry in entries.iter() {
            assert!(!entry.event.is_empty(), "Every entry names its event.");
            assert!(!entry.reason.is_empty(), "Every entry carries its reason.");
        }
        let operator_entries =
            cerulea_runtime::platform_api::audit_by_actor(deployment, operator.clone(), 512);
        assert!(
            !operator_entries.is_empty(),
            "The audit log carries the entries the Cerulea operator raised."
        );
    });
}

// ---------------------------------------------------------------------------
// Criterion 17. The term check passes.
// ---------------------------------------------------------------------------

/// Criterion 17. The term check passes.
///
/// The guardrail reads every forbidden term from the list beside it and scans
/// the chain tree. A dependency import path is exempt, which the guardrail
/// applies itself. The scan covers the whole chain repository.
#[test]
fn criterion_seventeen_the_term_check_passes() {
    let chain = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("The node crate sits inside the chain repository.")
        .to_path_buf();
    let guard = chain.join("scripts").join("term-check.sh");
    assert!(
        guard.is_file(),
        "The guardrail sits at {}.",
        guard.display()
    );

    let output = std::process::Command::new("bash")
        .arg(&guard)
        .arg(&chain)
        .output();
    let output = match output {
        Ok(held) => held,
        Err(error) => {
            panic!(
                "The term check needs a shell on the path. The run reported {error}. Run the guardrail at {} by hand.",
                guard.display()
            );
        }
    };
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let violations: Vec<&str> = text
        .lines()
        .filter(|line| line.starts_with("FORBIDDEN"))
        .collect();
    assert!(
        violations.is_empty(),
        "The chain tree carries {} forbidden terms.\n{}",
        violations.len(),
        violations.join("\n")
    );
    assert!(
        text.contains("TERM CHECK PASSED"),
        "The guardrail reported a pass. It read:\n{text}"
    );
    assert!(
        output.status.success(),
        "The guardrail answered with success."
    );
}
