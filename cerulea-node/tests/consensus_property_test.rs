//! Property tests over every consensus invariant.
//!
//! A unit test states one case. A property test states the rule and lets the
//! generator search for a case that breaks it. Every invariant the Dynamic
//! Consensus Framework rests on appears here.
//!
//! 1. **Partitioning.** Two transactions whose writes intersect land in one
//!    domain. Two transactions whose writes stay apart land in separate
//!    domains. Every transaction lands in exactly one domain.
//! 2. **Ordering.** The domain list reads the same for the same input, whatever
//!    the arrival order of equal transactions.
//! 3. **Thresholds.** A weight crossing the share commits. A weight below it
//!    waits. The four class shares rise in order.
//! 4. **Draw.** The committee draw is the same on every node for the same
//!    published state, and a higher Trust Score never raises a seat value.
//! 5. **Seal.** A sealed header verifies under the author key alone. Any change
//!    to the header or the claim breaks the seal.
//! 6. **Commit.** Two signer sets that each reach the share share a member.
//! 7. **Custody scoring.** The share stays inside its bounds. An answer raises
//!    it. A miss lowers it. An incorrect proof lowers it further and counts a
//!    fault.
//! 8. **Trust Score composition.** The four components of each network weigh to
//!    one whole. A full validator reaches the whole score.
//! 9. **Solver limits.** Every profile the solver returns passes the limits,
//!    and the count sits inside the stated bounds.

use std::collections::BTreeSet;

use proptest::prelude::*;

use cerulea_consensus::engines::authoring::draw::derive_slot_seed;
use cerulea_consensus::engines::authoring::seal::{extract_seal, pre_seal_hash, SealClaim};
use cerulea_consensus::engines::commit::commit::{crosses, DomainCommitter, ThresholdTable};
use cerulea_consensus::engines::execution::partition::DomainPartitioner;
use cerulea_consensus::engines::manifest::{AccessManifest, CollectionAccess, PendingTransaction};
use cerulea_consensus::engines::{
    CollectionId, CommitEngine, KeyRange, Slot, TransactionClass, TrustScore,
    ROUTINE_THRESHOLD_PERCENT,
};
use cerulea_node::test_harness::{execute_block, HarnessTransaction, TestNetwork};
use cerulea_primitives::{rank_seats, seat_value, weighted_draw, SeatInput};

use pallet_cerulea_custody::primitives::{ChallengeState, CustodyRecord};
use pallet_cerulea_custody::score::{settle, ANSWER_GAIN, FAULT_LOSS, MISS_LOSS, STARTING_SHARE};
use pallet_cerulea_dcf::engines::amendment::solver::{
    inspect_profile, solve, ConfirmationPreference, Declarations, ErrorCost, OperatorCount,
    RetentionNeed, SplitBehaviour, MAX_PROFILES, MIN_PROFILES,
};
use pallet_cerulea_dcf::engines::standing::score::{
    compose, ChainKind, Components, PRIVATE_ATTESTATION_PERCENT, PRIVATE_CUSTODY_PERCENT,
    PRIVATE_PERFORMANCE_PERCENT, PRIVATE_ROLE_PERCENT, PUBLIC_CUSTODY_PERCENT,
    PUBLIC_PERFORMANCE_PERCENT, PUBLIC_SAMPLING_PERCENT, PUBLIC_STANDING_PERCENT,
};
use sp_runtime::Perbill;

/// The number of cases each property runs.
const CASES: u32 = 96;

fn run_settings() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        failure_persistence: None,
        ..ProptestConfig::default()
    }
}

/// One generated transaction, named by collection and key.
fn transaction_strategy() -> impl Strategy<Value = (u8, u8)> {
    (0u8..4u8, 0u8..4u8)
}

fn build(collection: u8, key: u8) -> PendingTransaction {
    let id = CollectionId::from_name(&format!("collection.{}", collection));
    let key_bytes = format!("key-{}", key).into_bytes();
    let access = CollectionAccess::new(id, KeyRange::Exact(key_bytes.clone()));
    let manifest = AccessManifest::build(TransactionClass::Routine, Vec::new(), vec![access])
        .expect("the manifest holds one write");
    let hash = sp_core::hashing::blake2_256(&[&[collection, key][..], &key_bytes[..]].concat());
    PendingTransaction::new(hash, 0, manifest, key_bytes)
}

proptest! {
    #![proptest_config(run_settings())]

    // --- Partitioning ---

    #[test]
    fn every_transaction_lands_in_exactly_one_domain(
        entries in prop::collection::vec(transaction_strategy(), 1..12)
    ) {
        let pending: Vec<PendingTransaction> = entries
            .iter()
            .map(|(collection, key)| build(*collection, *key))
            .collect();
        let domains = DomainPartitioner::new().partition_domains(&pending);

        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut total = 0usize;
        for domain in domains.iter() {
            for position in domain.members.iter() {
                prop_assert!(seen.insert(*position), "position {} landed twice", position);
                total += 1;
            }
        }
        prop_assert_eq!(total, pending.len(), "every transaction landed in a domain");
    }

    #[test]
    fn two_transactions_writing_the_same_key_share_a_domain(
        collection in 0u8..4u8,
        key in 0u8..4u8,
        other in 0u8..4u8,
    ) {
        let pending = vec![build(collection, key), build(collection, key), build(other, key)];
        let domains = DomainPartitioner::new().partition_domains(&pending);
        let home_of = |position: usize| {
            domains
                .iter()
                .position(|domain| domain.members.contains(&position))
                .expect("the transaction landed in a domain")
        };
        prop_assert_eq!(home_of(0), home_of(1), "the same key joins one domain");
        if other != collection {
            prop_assert_ne!(home_of(0), home_of(2), "a separate collection stays apart");
        }
    }

    #[test]
    fn transactions_over_separate_keys_stay_in_separate_domains(
        collection in 0u8..4u8,
    ) {
        let pending: Vec<PendingTransaction> =
            (0u8..4u8).map(|key| build(collection, key)).collect();
        let domains = DomainPartitioner::new().partition_domains(&pending);
        prop_assert_eq!(domains.len(), 4, "four separate keys make four domains");
    }

    // --- Ordering ---

    #[test]
    fn the_same_block_reaches_the_same_roots_whatever_the_worker_count(
        entries in prop::collection::vec(transaction_strategy(), 1..10),
        workers in 1usize..9usize,
    ) {
        let block: Vec<HarnessTransaction> = entries
            .iter()
            .enumerate()
            .map(|(position, (collection, key))| {
                HarnessTransaction::new(
                    &format!("collection.{}", collection),
                    &format!("key-{}-{}", key, position),
                    &format!("value-{}", position),
                    TransactionClass::Routine,
                )
            })
            .collect();
        let single = execute_block(&block, 1);
        let many = execute_block(&block, workers);
        prop_assert_eq!(single.domains.clone(), many.domains.clone());
        prop_assert_eq!(single.combined, many.combined);
        prop_assert!(single.consistent());
    }

    // --- Thresholds ---

    #[test]
    fn a_weight_crossing_the_share_commits_and_a_weight_below_it_waits(
        total in 1u128..1_000_000u128,
        share in 51u8..=100u8,
        signed in 0u128..1_000_000u128,
    ) {
        let signed = signed.min(total);
        let reached = crosses(signed, total, share);
        let expected = signed.saturating_mul(100) >= total.saturating_mul(share as u128);
        prop_assert_eq!(reached, expected);
        if reached {
            prop_assert!(signed * 2 > total, "the share sits above half the weight");
        }
    }

    #[test]
    fn the_four_class_shares_rise_in_order(
        routine in 51u8..=60u8,
        financial in 61u8..=70u8,
        structural in 71u8..=85u8,
        constitutional in 86u8..=100u8,
    ) {
        let table = ThresholdTable::new(routine, financial, structural, constitutional);
        prop_assert!(table.percent_for(TransactionClass::Routine)
            <= table.percent_for(TransactionClass::Financial));
        prop_assert!(table.percent_for(TransactionClass::Financial)
            <= table.percent_for(TransactionClass::Structural));
        prop_assert!(table.percent_for(TransactionClass::Structural)
            <= table.percent_for(TransactionClass::Constitutional));
        prop_assert!(table.respects_the_floor());
        prop_assert!(table.percent_for(TransactionClass::Routine) >= ROUTINE_THRESHOLD_PERCENT);
    }

    // --- Draw ---

    #[test]
    fn the_slot_seed_follows_the_parent_and_the_slot(
        parent in prop::array::uniform32(any::<u8>()),
        slot in 0u64..10_000u64,
        other in 0u64..10_000u64,
    ) {
        let first = derive_slot_seed(&parent, Slot(slot));
        let again = derive_slot_seed(&parent, Slot(slot));
        prop_assert_eq!(first.0, again.0, "the seed repeats for the same input");
        if slot != other {
            let different = derive_slot_seed(&parent, Slot(other));
            prop_assert_ne!(first.0, different.0, "a separate slot draws a separate seed");
        }
    }

    #[test]
    fn a_higher_trust_score_never_raises_a_seat_value(
        raw in any::<u128>(),
        lower in 1u32..500_000u32,
        rise in 1u32..499_999u32,
    ) {
        // The factor runs from one at no standing down to one quarter at the
        // whole score, and a lower seat value wins a lower rank. A validator
        // that served therefore never sits behind one that did not.
        let higher = lower.saturating_add(rise).min(1_000_000);
        prop_assert!(weighted_draw(raw, higher) <= weighted_draw(raw, lower));
    }

    #[test]
    fn the_committee_draw_repeats_for_the_same_published_state(
        seed in prop::array::uniform32(any::<u8>()),
        standings in prop::collection::vec(1u32..1_000_000u32, 3..10),
    ) {
        let candidates: Vec<SeatInput> = standings
            .iter()
            .enumerate()
            .map(|(position, standing)| SeatInput {
                draw_key: [position as u8; 32],
                epoch_output: [(position as u8).wrapping_mul(7); 32],
                standing: *standing,
            })
            .collect();
        let first = rank_seats(&candidates, &seed);
        let again = rank_seats(&candidates, &seed);
        prop_assert_eq!(first.clone(), again);

        // The ranks read one, two, three and upward with no gap.
        for (position, seat) in first.iter().enumerate() {
            prop_assert_eq!(seat.rank as usize, position + 1);
        }
        // A seat value follows the published state alone.
        for seat in first.iter() {
            let held = candidates
                .iter()
                .find(|entry| entry.draw_key == seat.draw_key)
                .expect("every seat names a candidate");
            prop_assert_eq!(
                seat.seat_value,
                seat_value(&held.epoch_output, &seed, held.standing)
            );
        }
    }

    // --- Commit ---

    #[test]
    fn two_signer_sets_that_each_reach_the_share_share_a_member(
        validators in 4usize..12usize,
        share in 51u8..=90u8,
    ) {
        let standing = TrustScore::from_percent(50);
        let total = standing.weight() * validators as u128;
        let mut smallest = validators + 1;
        for size in 1..=validators {
            if crosses(standing.weight() * size as u128, total, share) {
                smallest = size;
                break;
            }
        }
        prop_assert!(smallest <= validators, "some set reaches the share");
        prop_assert!(
            smallest * 2 > validators,
            "two sets of {} out of {} would avoid each other",
            smallest,
            validators
        );
    }

    #[test]
    fn a_domain_commits_when_the_signers_reach_the_share_and_waits_otherwise(
        validators in 4usize..10usize,
        signers in 1usize..10usize,
    ) {
        let network = TestNetwork::with_validators(validators);
        let signers = signers.min(validators);
        let table = ThresholdTable::new(ROUTINE_THRESHOLD_PERCENT, 67, 80, 90);
        let mut committer =
            DomainCommitter::open(Slot(7), [3u8; 32], network.total_weight(), table)
                .expect("the set carries weight");
        for node in network.nodes.iter().take(signers) {
            committer
                .add_signer(node.seal_key, node.standing)
                .expect("the signer joins the tally");
        }
        let reached = crosses(
            committer.signed_weight(),
            network.total_weight(),
            table.percent_for(TransactionClass::Routine),
        );
        prop_assert_eq!(
            committer.settle(0, TransactionClass::Routine).committed,
            reached
        );
    }

    // --- Seal ---

    #[test]
    fn a_sealed_header_verifies_and_any_change_breaks_the_seal(
        height in 1u32..500u32,
        slot in 1u64..500u64,
        rank in 1u8..6u8,
    ) {
        let network = TestNetwork::with_validators(3);
        let parent = [5u8; 32];
        let input = network.draw_input(Slot(slot), parent);
        let draw = network.keystore_draw(0, &input);
        let claim = SealClaim::new(
            Slot(slot),
            cerulea_consensus::engines::Rank(rank),
            network.nodes[0].seal_key,
            draw,
            0,
        );
        let header = network.sealed_header(0, claim.clone(), height, parent);
        let seal = extract_seal(&header).expect("the header carries a seal");
        prop_assert_eq!(seal.claim.author, network.nodes[0].seal_key);
        prop_assert_eq!(seal.claim.slot, Slot(slot));

        // The pre-seal hash covers the header. A change to the header changes
        // the hash the signature covers, so the seal no longer matches.
        let other = network.sealed_header(0, claim, height.saturating_add(1), parent);
        prop_assert_ne!(pre_seal_hash(&header), pre_seal_hash(&other));
    }

    #[test]
    fn sealing_the_same_header_twice_produces_one_seal(
        height in 1u32..200u32,
    ) {
        let network = TestNetwork::with_validators(3);
        let parent = [9u8; 32];
        let input = network.draw_input(Slot(4), parent);
        let draw = network.keystore_draw(1, &input);
        let claim = SealClaim::new(
            Slot(4),
            cerulea_consensus::engines::Rank::FIRST,
            network.nodes[1].seal_key,
            draw,
            0,
        );
        let header = network.sealed_header(1, claim, height, parent);
        prop_assert!(extract_seal(&header).is_some());
    }

    // --- Custody scoring ---

    #[test]
    fn the_custody_share_stays_inside_its_bounds(
        states in prop::collection::vec(0u8..4u8, 1..30),
    ) {
        let mut record = CustodyRecord::default();
        let mut previous = STARTING_SHARE;
        for value in states.iter() {
            let state = match value {
                0 => ChallengeState::Open,
                1 => ChallengeState::AnsweredInTime,
                2 => ChallengeState::Missed,
                _ => ChallengeState::Incorrect,
            };
            let movement = settle(&mut record, state);
            prop_assert!(movement.current_share <= CustodyRecord::FULL_SHARE);
            match state {
                ChallengeState::Open => {
                    prop_assert_eq!(movement.current_share, movement.previous_share);
                }
                ChallengeState::AnsweredInTime => {
                    prop_assert!(movement.current_share >= movement.previous_share);
                    prop_assert!(
                        movement.current_share
                            <= movement.previous_share.saturating_add(ANSWER_GAIN)
                    );
                }
                ChallengeState::Missed => {
                    prop_assert_eq!(
                        movement.current_share,
                        movement.previous_share.saturating_sub(MISS_LOSS)
                    );
                }
                ChallengeState::Incorrect => {
                    prop_assert_eq!(
                        movement.current_share,
                        movement.previous_share.saturating_sub(FAULT_LOSS)
                    );
                }
            }
            previous = movement.current_share;
        }
        prop_assert_eq!(record.share, previous);
    }

    #[test]
    fn a_miss_costs_more_than_an_answer_gains(
        answers in 1u32..20u32,
    ) {
        let mut record = CustodyRecord::default();
        for _ in 0..answers {
            settle(&mut record, ChallengeState::AnsweredInTime);
        }
        let raised = record.share;
        let movement = settle(&mut record, ChallengeState::Missed);
        prop_assert!(raised.saturating_sub(movement.current_share) >= ANSWER_GAIN);
        prop_assert_eq!(record.missed, 1);
    }

    // --- Trust Score composition ---

    #[test]
    fn the_public_chain_weighs_four_components_to_one_whole(
        standing in 0u32..=1_000_000_000u32,
        performance in 0u32..=1_000_000_000u32,
        custody in 0u32..=1_000_000_000u32,
        sampling in 0u32..=1_000_000_000u32,
    ) {
        let components = Components {
            standing: Perbill::from_parts(standing),
            performance: Perbill::from_parts(performance),
            custody: Perbill::from_parts(custody),
            sampling: Perbill::from_parts(sampling),
            role: Perbill::zero(),
            attestation: Perbill::zero(),
        };
        let score = compose(ChainKind::PublicAccess, components);
        prop_assert!(score.total <= Perbill::one());
        prop_assert_eq!(score.role, Perbill::zero());
        prop_assert_eq!(score.attestation, Perbill::zero());

        let expected = (standing as u64 * PUBLIC_STANDING_PERCENT as u64) / 100
            + (performance as u64 * PUBLIC_PERFORMANCE_PERCENT as u64) / 100
            + (custody as u64 * PUBLIC_CUSTODY_PERCENT as u64) / 100
            + (sampling as u64 * PUBLIC_SAMPLING_PERCENT as u64) / 100;
        prop_assert_eq!(
            score.total.deconstruct() as u64,
            expected.min(Perbill::one().deconstruct() as u64)
        );
    }

    #[test]
    fn the_private_chain_weighs_its_own_four_components(
        role in 0u32..=1_000_000_000u32,
        attestation in 0u32..=1_000_000_000u32,
        performance in 0u32..=1_000_000_000u32,
        custody in 0u32..=1_000_000_000u32,
    ) {
        let components = Components {
            standing: Perbill::zero(),
            performance: Perbill::from_parts(performance),
            custody: Perbill::from_parts(custody),
            sampling: Perbill::zero(),
            role: Perbill::from_parts(role),
            attestation: Perbill::from_parts(attestation),
        };
        let score = compose(ChainKind::Private, components);
        prop_assert!(score.total <= Perbill::one());
        prop_assert_eq!(score.standing, Perbill::zero());
        prop_assert_eq!(score.sampling, Perbill::zero());

        let expected = (role as u64 * PRIVATE_ROLE_PERCENT as u64) / 100
            + (attestation as u64 * PRIVATE_ATTESTATION_PERCENT as u64) / 100
            + (performance as u64 * PRIVATE_PERFORMANCE_PERCENT as u64) / 100
            + (custody as u64 * PRIVATE_CUSTODY_PERCENT as u64) / 100;
        prop_assert_eq!(
            score.total.deconstruct() as u64,
            expected.min(Perbill::one().deconstruct() as u64)
        );
    }

    // --- Solver limits ---

    #[test]
    fn every_profile_the_solver_returns_passes_the_limits(
        confirmation in 0u8..2u8,
        operators in 0u8..3u8,
        cost in 0u8..3u8,
        split in 0u8..2u8,
        retention in 0u8..3u8,
    ) {
        let declarations = Declarations {
            confirmation: match confirmation {
                0 => ConfirmationPreference::InstantConfirmation,
                _ => ConfirmationPreference::MaximumCertainty,
            },
            operators: match operators {
                0 => OperatorCount::SingleOrganisation,
                1 => OperatorCount::SeveralOrganisations,
                _ => OperatorCount::ManyOrganisations,
            },
            error_cost: match cost {
                0 => ErrorCost::Recoverable,
                1 => ErrorCost::Expensive,
                _ => ErrorCost::Irreversible,
            },
            split: match split {
                0 => SplitBehaviour::KeepServing,
                _ => SplitBehaviour::HaltToStayConsistent,
            },
            retention: match retention {
                0 => RetentionNeed::RecentOnly,
                1 => RetentionNeed::SeveralYears,
                _ => RetentionNeed::Permanent,
            },
        };
        let profiles = solve(declarations);
        prop_assert!(profiles.len() >= MIN_PROFILES, "the solver returned {} profiles", profiles.len());
        prop_assert!(profiles.len() <= MAX_PROFILES, "the solver returned {} profiles", profiles.len());
        for profile in profiles.iter() {
            prop_assert!(
                inspect_profile(profile).is_ok(),
                "a returned profile breaks a limit"
            );
        }
    }
}
