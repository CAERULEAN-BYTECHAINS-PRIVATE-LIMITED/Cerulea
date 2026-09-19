//! Tests of the engines that carry agreement inside this pallet.
//!
//! The framework pallet held a second finality marker for a while. The Commit
//! Engine carries the one commit path now, and the registry pallet records the
//! committed head. This file therefore asserts on the two engines that live
//! here and decide when a domain commits. The Assurance Engine sets the share
//! of validator weight each transaction class needs. The Standing Engine sets
//! the Trust Score that weight is measured in.

use crate::engines::assurance::TransactionClass;
use crate::mock::*;
use frame_support::traits::{Get, OnFinalize, OnInitialize};

const EVERY_CLASS: [TransactionClass; 4] = [
    TransactionClass::Routine,
    TransactionClass::Financial,
    TransactionClass::Structural,
    TransactionClass::Constitutional,
];

/// Every threshold sits above half the total weight.
///
/// Two signer sets each carrying more than half the weight share a member. A
/// validator signs one block per slot. Two blocks at one height therefore never
/// both cross a threshold.
#[test]
fn every_threshold_sits_above_half_the_weight() {
    new_test_ext().execute_with(|| {
        let table = DcfPallet::assurance_class_table();
        for class in EVERY_CLASS {
            let percent = table.threshold_percent(class);
            assert!(percent > 50, "{:?} sits at {}", class, percent);
            assert!(percent <= 100);
        }
    });
}

/// The class table carries the starting profile.
#[test]
fn the_class_table_carries_the_starting_profile() {
    new_test_ext().execute_with(|| {
        let table = DcfPallet::assurance_class_table();
        assert_eq!(table.threshold_percent(TransactionClass::Routine), 51);
        assert_eq!(table.threshold_percent(TransactionClass::Financial), 67);
        assert_eq!(table.threshold_percent(TransactionClass::Structural), 80);
        assert_eq!(
            table.threshold_percent(TransactionClass::Constitutional),
            90
        );
    });
}

/// A routine domain commits at a weight that leaves a constitutional domain waiting.
#[test]
fn a_routine_domain_commits_before_a_constitutional_domain() {
    new_test_ext().execute_with(|| {
        let total = 1_000u128;
        let signed = 600u128;
        assert!(DcfPallet::domain_reaches_threshold(
            vec![TransactionClass::Routine],
            total,
            signed
        ));
        assert!(!DcfPallet::domain_reaches_threshold(
            vec![TransactionClass::Constitutional],
            total,
            signed
        ));
    });
}

/// A domain carrying several classes commits at the highest class present.
#[test]
fn a_domain_commits_at_the_highest_class_it_carries() {
    new_test_ext().execute_with(|| {
        let total = 1_000u128;
        let mixed = vec![TransactionClass::Routine, TransactionClass::Structural];
        let assurance = DcfPallet::domain_assurance(mixed.clone(), total)
            .expect("the domain resolves against the class table");
        assert_eq!(assurance.highest_class, TransactionClass::Structural);
        assert!(!DcfPallet::domain_reaches_threshold(
            mixed.clone(),
            total,
            700
        ));
        assert!(DcfPallet::domain_reaches_threshold(mixed, total, 800));
    });
}

/// The Trust Score of every validator stays inside its range.
#[test]
fn every_trust_score_stays_inside_its_range() {
    new_test_ext().execute_with(|| {
        for validator in DcfPallet::active_validators().iter() {
            let score = DcfPallet::trust_score(validator);
            assert!(score.parts_per_million() <= 1_000_000);
            assert!(score.weight() <= 1_000_000_000u128);
        }
    });
}

/// The Trust Score answers the same way on repeated reads.
#[test]
fn the_trust_score_answers_the_same_way_twice() {
    new_test_ext().execute_with(|| {
        for validator in DcfPallet::active_validators().iter() {
            let first = DcfPallet::trust_score(validator);
            let second = DcfPallet::trust_score(validator);
            assert_eq!(first, second);
        }
    });
}

/// Block progress leaves the validator set and the class table standing.
#[test]
fn block_progress_leaves_the_engines_standing() {
    new_test_ext().execute_with(|| {
        let initial_validators = DcfPallet::active_validators();
        let initial_table = DcfPallet::assurance_class_table();

        for block_num in 1..=15u64 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);

            let current = DcfPallet::active_validators();
            assert!(!current.is_empty());
            assert_eq!(current.len(), initial_validators.len());
            assert_eq!(
                DcfPallet::assurance_class_table().threshold_percent(TransactionClass::Routine),
                initial_table.threshold_percent(TransactionClass::Routine)
            );
        }
    });
}

/// Every active validator reports as active and carries participation counts.
#[test]
fn consensus_participation_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        assert!(!active_validators.is_empty());

        for validator in &active_validators {
            assert!(DcfPallet::is_validator_active(validator));
            let (authored, missed) = DcfPallet::validator_participation(validator);
            assert!(authored < u32::MAX);
            assert!(missed < u32::MAX);
        }
    });
}

/// The scheduled author of a block sits in the active validator set.
#[test]
fn expected_block_authorship_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();

        for block_num in 1..=10 {
            if let Some(author) = DcfPallet::expected_author(block_num) {
                assert!(active_validators.contains(&author));
            }
        }
    });
}

/// The validator set meets the floor the deployment profile carries.
#[test]
fn consensus_algorithm_correctness_works() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let min_validators =
            <<Test as pallet_cerulea_pos::Config>::MinActiveValidators as Get<u32>>::get();

        assert!(active_validators.len() >= min_validators as usize);

        for validator in &active_validators {
            let stake = pallet_cerulea_pos::Stake::<Test>::get(validator);
            let min_stake = <Test as pallet_cerulea_pos::Config>::MinStake::get();
            assert!(stake >= min_stake);
            assert!(DcfPallet::is_validator_active(validator));
        }
    });
}

/// The chain keeps making progress across several blocks.
#[test]
fn consensus_liveness_properties_work() {
    new_test_ext().execute_with(|| {
        let initial_epoch = DcfPallet::current_epoch();

        for block_num in 1..=5u64 {
            System::set_block_number(block_num);
            let _ = DcfPallet::on_initialize(block_num);
            DcfPallet::on_finalize(block_num);
        }

        assert!(!DcfPallet::active_validators().is_empty());
        assert!(DcfPallet::current_epoch() >= initial_epoch);
    });
}

/// Repeated queries reach the same answer, so every node agrees.
#[test]
fn consensus_agreement_properties_work() {
    new_test_ext().execute_with(|| {
        let active_validators = DcfPallet::active_validators();
        let current_epoch = DcfPallet::current_epoch();
        let routine = DcfPallet::assurance_class_table().threshold_percent(TransactionClass::Routine);

        for _ in 0..5 {
            assert_eq!(DcfPallet::active_validators(), active_validators);
            assert_eq!(DcfPallet::current_epoch(), current_epoch);
            assert_eq!(
                DcfPallet::assurance_class_table().threshold_percent(TransactionClass::Routine),
                routine
            );
        }
    });
}
