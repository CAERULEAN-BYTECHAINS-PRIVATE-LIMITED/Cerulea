//! Line coverage for the weight functions of the framework pallet.
//!
//! The pallet ships two weight tables. The measured impl reads the database
//! weight of the runtime. The test impl serves tests and backwards
//! compatibility. The mock runtime charges its own table, so this file is the
//! one place both tables run.
//!
//! The mock runtime sets the database weight to zero. Every read and every
//! write therefore adds zero to a measured figure in this file. The expected
//! measured figures below carry the computation part alone.
//!
//! The test impl mirrors the measured figures. This file asserts the relations
//! that pair holds: the measured figure sits at or above the test figure, both
//! fit inside a two second block, and both rise with the validator count. The
//! exact figures asserted here belong to the measured impl alone.

use crate::mock::Test;
use crate::weights::{SubstrateWeight, WeightInfo};
use frame_support::traits::Get;
use frame_support::weights::constants::{RocksDbWeight, WEIGHT_REF_TIME_PER_SECOND};
use frame_support::weights::Weight;

/// The measured impl, instantiated on the mock runtime.
type Measured = SubstrateWeight<Test>;

/// The test impl.
type TestImpl = ();

/// Two seconds of reference time. A block holds at most this much.
const TWO_SECOND_BLOCK: u64 = 2 * WEIGHT_REF_TIME_PER_SECOND;

/// One trait function, run on both impls.
struct Row {
    name: &'static str,
    measured: Weight,
    test: Weight,
    measured_ref: u64,
}

fn row(name: &'static str, measured: Weight, test: Weight, measured_ref: u64) -> Row {
    Row {
        name,
        measured,
        test,
        measured_ref,
    }
}

/// Every one of the 42 trait functions on both impls.
///
/// Each parameterised function runs here at ten validators. The expected
/// measured figures are computed by hand from the weight table.
fn every_function() -> Vec<Row> {
    vec![
        // The measured figure runs on_initialize_with_validators at ten.
        row(
            "on_initialize",
            <Measured as WeightInfo>::on_initialize(),
            <TestImpl as WeightInfo>::on_initialize(),
            25_000 + 8_000 * 10,
        ),
        row(
            "offchain_worker",
            <Measured as WeightInfo>::offchain_worker(),
            <TestImpl as WeightInfo>::offchain_worker(),
            50_000,
        ),
        row(
            "update_validator_stake_score",
            <Measured as WeightInfo>::update_validator_stake_score(),
            <TestImpl as WeightInfo>::update_validator_stake_score(),
            15_000,
        ),
        row(
            "update_validator_inference_score",
            <Measured as WeightInfo>::update_validator_inference_score(),
            <TestImpl as WeightInfo>::update_validator_inference_score(),
            15_000,
        ),
        row(
            "update_consensus_weights",
            <Measured as WeightInfo>::update_consensus_weights(),
            <TestImpl as WeightInfo>::update_consensus_weights(),
            8_000,
        ),
        row(
            "set_governance_mode",
            <Measured as WeightInfo>::set_governance_mode(),
            <TestImpl as WeightInfo>::set_governance_mode(),
            5_000,
        ),
        row(
            "sudo_advance_epoch",
            <Measured as WeightInfo>::sudo_advance_epoch(),
            <TestImpl as WeightInfo>::sudo_advance_epoch(),
            50_000,
        ),
        row(
            "submit_proposal",
            <Measured as WeightInfo>::submit_proposal(),
            <TestImpl as WeightInfo>::submit_proposal(),
            20_000,
        ),
        row(
            "vote_proposal",
            <Measured as WeightInfo>::vote_proposal(),
            <TestImpl as WeightInfo>::vote_proposal(),
            18_000,
        ),
        row(
            "execute_proposal",
            <Measured as WeightInfo>::execute_proposal(),
            <TestImpl as WeightInfo>::execute_proposal(),
            35_000,
        ),
        row(
            "join_validator_set",
            <Measured as WeightInfo>::join_validator_set(),
            <TestImpl as WeightInfo>::join_validator_set(),
            25_000,
        ),
        row(
            "leave_validator_set",
            <Measured as WeightInfo>::leave_validator_set(),
            <TestImpl as WeightInfo>::leave_validator_set(),
            20_000,
        ),
        row(
            "set_validator_name",
            <Measured as WeightInfo>::set_validator_name(),
            <TestImpl as WeightInfo>::set_validator_name(),
            12_000,
        ),
        // The measured figure charges every one of the 100 validators.
        row(
            "apply_offchain_poi_scores",
            <Measured as WeightInfo>::apply_offchain_poi_scores(),
            <TestImpl as WeightInfo>::apply_offchain_poi_scores(),
            50_000 + 15_000 * 100,
        ),
        row(
            "on_initialize_with_validators",
            <Measured as WeightInfo>::on_initialize_with_validators(10),
            <TestImpl as WeightInfo>::on_initialize_with_validators(10),
            25_000 + 8_000 * 10,
        ),
        row(
            "apply_score_decay_multiple",
            <Measured as WeightInfo>::apply_score_decay_multiple(10),
            <TestImpl as WeightInfo>::apply_score_decay_multiple(10),
            15_000 + 12_000 * 10,
        ),
        row(
            "validator_set_operations",
            <Measured as WeightInfo>::validator_set_operations(10),
            <TestImpl as WeightInfo>::validator_set_operations(10),
            15_000 + 3_000 * 10,
        ),
        row(
            "runtime_api_calls",
            <Measured as WeightInfo>::runtime_api_calls(10),
            <TestImpl as WeightInfo>::runtime_api_calls(10),
            35_000 + 8_000 * 10,
        ),
        row(
            "epoch_transition_multiple",
            <Measured as WeightInfo>::epoch_transition_multiple(10),
            <TestImpl as WeightInfo>::epoch_transition_multiple(10),
            60_000 + 15_000 * 10,
        ),
        row(
            "governance_with_multiple_voters",
            <Measured as WeightInfo>::governance_with_multiple_voters(10),
            <TestImpl as WeightInfo>::governance_with_multiple_voters(10),
            30_000 + 18_000 * 10,
        ),
        row(
            "join_validators",
            <Measured as WeightInfo>::join_validators(),
            <TestImpl as WeightInfo>::join_validators(),
            120_000,
        ),
        row(
            "leave_validators",
            <Measured as WeightInfo>::leave_validators(),
            <TestImpl as WeightInfo>::leave_validators(),
            60_000,
        ),
        row(
            "cancel_leave_request",
            <Measured as WeightInfo>::cancel_leave_request(),
            <TestImpl as WeightInfo>::cancel_leave_request(),
            25_000,
        ),
        row(
            "slash_validator",
            <Measured as WeightInfo>::slash_validator(),
            <TestImpl as WeightInfo>::slash_validator(),
            45_000,
        ),
        row(
            "slash_validator_percentage",
            <Measured as WeightInfo>::slash_validator_percentage(),
            <TestImpl as WeightInfo>::slash_validator_percentage(),
            40_000,
        ),
        row(
            "increase_validator_stake",
            <Measured as WeightInfo>::increase_validator_stake(),
            <TestImpl as WeightInfo>::increase_validator_stake(),
            35_000,
        ),
        row(
            "decrease_validator_stake",
            <Measured as WeightInfo>::decrease_validator_stake(),
            <TestImpl as WeightInfo>::decrease_validator_stake(),
            35_000,
        ),
        row(
            "report_validator_misbehavior",
            <Measured as WeightInfo>::report_validator_misbehavior(),
            <TestImpl as WeightInfo>::report_validator_misbehavior(),
            55_000,
        ),
        row(
            "simulate_inference",
            <Measured as WeightInfo>::simulate_inference(),
            <TestImpl as WeightInfo>::simulate_inference(),
            30_000,
        ),
        row(
            "propose_slash_validator",
            <Measured as WeightInfo>::propose_slash_validator(),
            <TestImpl as WeightInfo>::propose_slash_validator(),
            35_000,
        ),
        row(
            "propose_reward_validator",
            <Measured as WeightInfo>::propose_reward_validator(),
            <TestImpl as WeightInfo>::propose_reward_validator(),
            35_000,
        ),
        row(
            "propose_eject_validator",
            <Measured as WeightInfo>::propose_eject_validator(),
            <TestImpl as WeightInfo>::propose_eject_validator(),
            35_000,
        ),
        // The measured figure charges 50 validators at 45_000 each.
        row(
            "slash_multiple_validators",
            <Measured as WeightInfo>::slash_multiple_validators(),
            <TestImpl as WeightInfo>::slash_multiple_validators(),
            150_000 + 45_000 * 50,
        ),
        // The measured figure charges 50 proposals at 55_000 each.
        row(
            "execute_proposals",
            <Measured as WeightInfo>::execute_proposals(),
            <TestImpl as WeightInfo>::execute_proposals(),
            200_000 + 55_000 * 50,
        ),
        // The measured figure charges 100 validators and 50 proposals.
        row(
            "epoch_transition",
            <Measured as WeightInfo>::epoch_transition(),
            <TestImpl as WeightInfo>::epoch_transition(),
            500_000 + 35_000 * 100 + 25_000 * 50,
        ),
        row(
            "set_validator_metadata",
            <Measured as WeightInfo>::set_validator_metadata(),
            <TestImpl as WeightInfo>::set_validator_metadata(),
            25_000,
        ),
        row(
            "update_validator_activity",
            <Measured as WeightInfo>::update_validator_activity(),
            <TestImpl as WeightInfo>::update_validator_activity(),
            20_000,
        ),
        row(
            "distribute_epoch_rewards",
            <Measured as WeightInfo>::distribute_epoch_rewards(),
            <TestImpl as WeightInfo>::distribute_epoch_rewards(),
            150_000 + 20_000 * 100,
        ),
        row(
            "propose_default_reward_validator",
            <Measured as WeightInfo>::propose_default_reward_validator(),
            <TestImpl as WeightInfo>::propose_default_reward_validator(),
            30_000,
        ),
        row(
            "propose_default_reward_multiple_validators",
            <Measured as WeightInfo>::propose_default_reward_multiple_validators(),
            <TestImpl as WeightInfo>::propose_default_reward_multiple_validators(),
            35_000,
        ),
        row(
            "propose_reward_all_active_validators",
            <Measured as WeightInfo>::propose_reward_all_active_validators(),
            <TestImpl as WeightInfo>::propose_reward_all_active_validators(),
            40_000,
        ),
        row(
            "propose_reward_multiple_validators",
            <Measured as WeightInfo>::propose_reward_multiple_validators(10),
            <TestImpl as WeightInfo>::propose_reward_multiple_validators(10),
            40_000 + 5_000 * 10,
        ),
    ]
}

/// One parameterised trait function, run on both impls.
struct Scaled {
    name: &'static str,
    measured: fn(u32) -> Weight,
    test: fn(u32) -> Weight,
    measured_base: u64,
    measured_slope: u64,
}

/// Every parameterised trait function with its measured formula.
fn every_scaled_function() -> Vec<Scaled> {
    vec![
        Scaled {
            name: "on_initialize_with_validators",
            measured: <Measured as WeightInfo>::on_initialize_with_validators,
            test: <TestImpl as WeightInfo>::on_initialize_with_validators,
            measured_base: 25_000,
            measured_slope: 8_000,
        },
        Scaled {
            name: "apply_score_decay_multiple",
            measured: <Measured as WeightInfo>::apply_score_decay_multiple,
            test: <TestImpl as WeightInfo>::apply_score_decay_multiple,
            measured_base: 15_000,
            measured_slope: 12_000,
        },
        Scaled {
            name: "validator_set_operations",
            measured: <Measured as WeightInfo>::validator_set_operations,
            test: <TestImpl as WeightInfo>::validator_set_operations,
            measured_base: 15_000,
            measured_slope: 3_000,
        },
        Scaled {
            name: "runtime_api_calls",
            measured: <Measured as WeightInfo>::runtime_api_calls,
            test: <TestImpl as WeightInfo>::runtime_api_calls,
            measured_base: 35_000,
            measured_slope: 8_000,
        },
        Scaled {
            name: "epoch_transition_multiple",
            measured: <Measured as WeightInfo>::epoch_transition_multiple,
            test: <TestImpl as WeightInfo>::epoch_transition_multiple,
            measured_base: 60_000,
            measured_slope: 15_000,
        },
        Scaled {
            name: "governance_with_multiple_voters",
            measured: <Measured as WeightInfo>::governance_with_multiple_voters,
            test: <TestImpl as WeightInfo>::governance_with_multiple_voters,
            measured_base: 30_000,
            measured_slope: 18_000,
        },
        Scaled {
            name: "propose_reward_multiple_validators",
            measured: <Measured as WeightInfo>::propose_reward_multiple_validators,
            test: <TestImpl as WeightInfo>::propose_reward_multiple_validators,
            measured_base: 40_000,
            measured_slope: 5_000,
        },
    ]
}

#[test]
fn the_table_names_every_trait_function_once() {
    let rows = every_function();
    assert_eq!(rows.len(), 42, "The weight trait carries 42 functions.");

    let mut names: Vec<&'static str> = rows.iter().map(|held| held.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(
        names.len(),
        42,
        "Each trait function appears in the table once."
    );
}

#[test]
fn the_mock_database_weight_charges_zero_for_reads_and_writes() {
    let db = <<Test as frame_system::Config>::DbWeight as Get<
        frame_support::weights::RuntimeDbWeight,
    >>::get();
    assert_eq!(db.reads(1_000), Weight::zero());
    assert_eq!(db.writes(1_000), Weight::zero());
    assert_eq!(db.reads_writes(7, 9), Weight::zero());
}

#[test]
fn the_rocksdb_weight_charges_the_published_figures() {
    // One read costs 25 microseconds and one write costs 100 microseconds.
    assert_eq!(RocksDbWeight::get().reads(1).ref_time(), 25_000_000);
    assert_eq!(RocksDbWeight::get().writes(1).ref_time(), 100_000_000);
    assert_eq!(RocksDbWeight::get().reads(1).proof_size(), 0);
    assert_eq!(RocksDbWeight::get().writes(1).proof_size(), 0);
}

#[test]
fn every_measured_function_returns_its_hand_computed_figure() {
    for held in every_function() {
        assert_eq!(
            held.measured.ref_time(),
            held.measured_ref,
            "The measured figure of {} matches the weight table.",
            held.name
        );
    }
}

#[test]
fn every_function_charges_positive_reference_time_on_both_impls() {
    for held in every_function() {
        assert!(
            held.measured.ref_time() > 0,
            "The measured figure of {} charges reference time.",
            held.name
        );
        assert!(
            held.test.ref_time() > 0,
            "The test figure of {} charges reference time.",
            held.name
        );
    }
}

#[test]
fn every_function_fits_inside_a_two_second_block_on_both_impls() {
    for held in every_function() {
        assert!(
            held.measured.ref_time() < TWO_SECOND_BLOCK,
            "The measured figure of {} fits inside a two second block.",
            held.name
        );
        assert!(
            held.test.ref_time() < TWO_SECOND_BLOCK,
            "The test figure of {} fits inside a two second block.",
            held.name
        );
    }
}

#[test]
fn every_function_stays_inside_a_quarter_of_the_block() {
    // The weight table states that a single operation stays inside a quarter
    // of the block. Both impls hold that at the counts the table charges.
    let quarter = TWO_SECOND_BLOCK / 4;
    for held in every_function() {
        assert!(
            held.measured.ref_time() < quarter,
            "The measured figure of {} stays inside a quarter of the block.",
            held.name
        );
        assert!(
            held.test.ref_time() < quarter,
            "The test figure of {} stays inside a quarter of the block.",
            held.name
        );
    }
}

/// Depends on the fix that makes the test impl mirror the measured impl.
///
/// Before that fix the test impl charges RocksDB reads and writes on 22
/// functions and a higher figure for validator_set_operations, so 23 rows sit
/// above the measured figure.
#[test]
fn the_measured_figure_sits_at_or_above_the_test_figure() {
    for held in every_function() {
        assert!(
            held.measured.ref_time() >= held.test.ref_time(),
            "The measured figure of {} sits at or above the test figure.",
            held.name
        );
    }
}

/// Depends on the same fix for every count.
#[test]
fn every_parameterised_measured_figure_sits_at_or_above_the_test_figure() {
    for held in every_scaled_function() {
        for count in [0u32, 1, 10, 100] {
            assert!(
                (held.measured)(count).ref_time() >= (held.test)(count).ref_time(),
                "The measured figure of {} at {} validators sits at or above the test figure.",
                held.name,
                count
            );
        }
    }
}

#[test]
fn every_function_leaves_the_proof_size_at_zero_on_both_impls() {
    // Both impls leave the proof size at zero. The database weight adds zero too.
    for held in every_function() {
        assert_eq!(
            held.measured.proof_size(),
            0,
            "The measured figure of {} carries zero proof size.",
            held.name
        );
        assert_eq!(
            held.test.proof_size(),
            0,
            "The test figure of {} carries zero proof size.",
            held.name
        );
    }
}

#[test]
fn the_measured_on_initialize_charges_ten_validators() {
    assert_eq!(
        <Measured as WeightInfo>::on_initialize(),
        <Measured as WeightInfo>::on_initialize_with_validators(10)
    );
    assert_eq!(
        <Measured as WeightInfo>::on_initialize().ref_time(),
        105_000
    );
}

#[test]
fn every_parameterised_figure_rises_strictly_with_the_validator_count() {
    let counts = [1u32, 10, 100];
    for held in every_scaled_function() {
        for pair in counts.windows(2) {
            let (low, high) = (pair[0], pair[1]);
            assert!(
                (held.measured)(high).ref_time() > (held.measured)(low).ref_time(),
                "The measured figure of {} rises from {} to {} validators.",
                held.name,
                low,
                high
            );
            assert!(
                (held.test)(high).ref_time() > (held.test)(low).ref_time(),
                "The test figure of {} rises from {} to {} validators.",
                held.name,
                low,
                high
            );
        }
    }
}

#[test]
fn every_parameterised_measured_figure_follows_its_linear_formula() {
    for held in every_scaled_function() {
        for count in [0u32, 1, 10, 100, 1_000] {
            assert_eq!(
                (held.measured)(count).ref_time(),
                held.measured_base + held.measured_slope * count as u64,
                "The measured figure of {} at {} validators follows the formula.",
                held.name,
                count
            );
        }
    }
}

#[test]
fn every_parameterised_figure_adds_a_constant_step_per_validator() {
    for held in every_scaled_function() {
        let measured_step = (held.measured)(51).ref_time() - (held.measured)(50).ref_time();
        assert_eq!(
            measured_step, held.measured_slope,
            "One validator adds one measured slope to {}.",
            held.name
        );

        // The test impl adds the same positive step at every count.
        let test_step = (held.test)(51).ref_time() - (held.test)(50).ref_time();
        assert!(test_step > 0, "One validator adds weight to {}.", held.name);
        assert_eq!(
            (held.test)(91).ref_time() - (held.test)(90).ref_time(),
            test_step,
            "The test figure of {} grows by a constant step.",
            held.name
        );
    }
}

#[test]
fn every_parameterised_measured_figure_at_zero_validators_is_its_base() {
    for held in every_scaled_function() {
        assert_eq!((held.measured)(0).ref_time(), held.measured_base);
        assert!(
            held.measured_base > 0,
            "{} carries a measured base.",
            held.name
        );
        assert!(
            (held.test)(0).ref_time() > 0,
            "{} carries a test base.",
            held.name
        );
    }
}

#[test]
fn every_parameterised_measured_figure_stays_exact_at_the_largest_count() {
    // The largest slope times the largest count fits inside a u64, so the
    // saturating arithmetic of the weight type returns the exact product.
    for held in every_scaled_function() {
        assert_eq!(
            (held.measured)(u32::MAX).ref_time(),
            held.measured_base + held.measured_slope * u32::MAX as u64,
            "The measured figure of {} stays exact at the largest count.",
            held.name
        );
        assert!(
            (held.test)(u32::MAX).ref_time() > (held.test)(100).ref_time(),
            "The test figure of {} keeps rising up to the largest count.",
            held.name
        );
    }
}

#[test]
fn every_parameterised_figure_fits_the_block_at_one_hundred_validators() {
    // One hundred validators is the largest set the mock runtime admits.
    for held in every_scaled_function() {
        assert!((held.measured)(100).ref_time() < TWO_SECOND_BLOCK);
        assert!((held.test)(100).ref_time() < TWO_SECOND_BLOCK);
        assert_eq!((held.measured)(100).proof_size(), 0);
        assert_eq!((held.test)(100).proof_size(), 0);
    }
}

#[test]
fn the_epoch_transition_charges_the_most_of_the_measured_functions() {
    // The epoch transition processes every validator and every proposal.
    let rows = every_function();
    let heaviest = rows
        .iter()
        .max_by_key(|held| held.measured.ref_time())
        .map(|held| held.name);
    assert_eq!(heaviest, Some("epoch_transition"));
    assert_eq!(
        <Measured as WeightInfo>::epoch_transition().ref_time(),
        5_250_000
    );
}

#[test]
fn the_three_proposal_figures_charge_the_same_within_each_impl() {
    // Slash, reward and eject proposals write the same records.
    assert!([
        <Measured as WeightInfo>::propose_slash_validator(),
        <Measured as WeightInfo>::propose_reward_validator(),
        <Measured as WeightInfo>::propose_eject_validator(),
    ]
    .iter()
    .all(|held| held.ref_time() == 35_000));

    let test = <TestImpl as WeightInfo>::propose_slash_validator();
    assert_eq!(<TestImpl as WeightInfo>::propose_reward_validator(), test);
    assert_eq!(<TestImpl as WeightInfo>::propose_eject_validator(), test);
}

#[test]
fn the_stake_change_figures_charge_the_same_in_both_directions() {
    assert_eq!(
        <Measured as WeightInfo>::increase_validator_stake(),
        <Measured as WeightInfo>::decrease_validator_stake()
    );
    assert_eq!(
        <TestImpl as WeightInfo>::increase_validator_stake(),
        <TestImpl as WeightInfo>::decrease_validator_stake()
    );
    assert_eq!(
        <Measured as WeightInfo>::increase_validator_stake().ref_time(),
        35_000
    );
}

#[test]
fn the_batch_slash_charges_more_than_one_single_slash() {
    assert!(
        <Measured as WeightInfo>::slash_multiple_validators().ref_time()
            > <Measured as WeightInfo>::slash_validator().ref_time()
    );
    assert!(
        <TestImpl as WeightInfo>::slash_multiple_validators().ref_time()
            > <TestImpl as WeightInfo>::slash_validator().ref_time()
    );
    assert_eq!(
        <Measured as WeightInfo>::slash_multiple_validators().ref_time(),
        2_400_000
    );
}
