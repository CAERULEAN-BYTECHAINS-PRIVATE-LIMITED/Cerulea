//! Throughput and latency, measured.
//!
//! The run drives the release build of the engines and records what the machine
//! reached. Every figure this file writes is measured rather than targeted. The
//! report names the machine beside the figures, so a reader knows what produced
//! them.
//!
//! Three measurements.
//!
//! 1. **Execution throughput.** Transactions each second through the Execution
//!    Engine, across the conflict domains the Access Manifest produces.
//! 2. **Agreement latency per class.** The time the Commit Engine spends
//!    reaching the share each transaction class needs, at the median, the 95th
//!    and the 99th percentile.
//! 3. **Slot latency.** The time one whole slot takes, from the draw to the
//!    commitment.

use std::fs;
use std::time::{Duration, Instant};

use cerulea_node::fault_harness::FaultNetwork;
use cerulea_node::test_harness::{execute_block, HarnessTransaction, TestNetwork};

use cerulea_consensus::engines::commit::commit::{DomainCommitter, ThresholdTable};
use cerulea_consensus::engines::{CommitEngine, Slot, TransactionClass};

/// The validator count the measurement runs.
const VALIDATORS: usize = 9;

/// The number of transactions one measured block carries.
const BLOCK_SIZE: usize = 512;

/// The number of blocks the throughput measurement runs.
const BLOCKS: usize = 20;

/// The number of samples each latency measurement collects.
const SAMPLES: usize = 400;

/// The number of slots the slot latency measurement runs.
const SLOTS: u64 = 40;

fn percentile(sorted: &[Duration], share: f64) -> Duration {
    if sorted.is_empty() {
        return Duration::ZERO;
    }
    let position = ((sorted.len() - 1) as f64 * share).round() as usize;
    sorted[position.min(sorted.len() - 1)]
}

fn micros(value: Duration) -> f64 {
    value.as_secs_f64() * 1_000_000.0
}

fn measured_block(size: usize, class: TransactionClass) -> Vec<HarnessTransaction> {
    (0..size)
        .map(|position| {
            HarnessTransaction::new(
                &format!("collection.{}", position % 16),
                &format!("key-{}", position),
                &format!("value-{}", position),
                class,
            )
        })
        .collect()
}

struct ClassLatency {
    class: TransactionClass,
    median: Duration,
    ninety_fifth: Duration,
    ninety_ninth: Duration,
}

fn measure_class_latency(class: TransactionClass) -> ClassLatency {
    let network = TestNetwork::with_validators(VALIDATORS);
    let table = ThresholdTable::new(66, 67, 80, 90);
    let mut samples: Vec<Duration> = Vec::with_capacity(SAMPLES);

    for sample in 0..SAMPLES {
        let hash = [(sample % 251) as u8; 32];
        let started = Instant::now();
        let mut committer =
            DomainCommitter::open(Slot(sample as u64 + 1), hash, network.total_weight(), table)
                .expect("the validator set carries weight");
        for node in network.nodes.iter() {
            committer
                .add_signer(node.seal_key, node.standing)
                .expect("the signer joins the tally");
            if committer.settle(0, class).committed {
                break;
            }
        }
        samples.push(started.elapsed());
    }

    samples.sort();
    ClassLatency {
        class,
        median: percentile(&samples, 0.50),
        ninety_fifth: percentile(&samples, 0.95),
        ninety_ninth: percentile(&samples, 0.99),
    }
}

#[test]
fn the_run_measures_throughput_and_latency_and_records_them() {
    // --- Execution throughput ---
    let block = measured_block(BLOCK_SIZE, TransactionClass::Routine);
    let workers = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4);

    // One warm run settles the allocator before the measurement starts.
    let warm = execute_block(&block, workers);
    assert!(warm.consistent());

    let started = Instant::now();
    for _ in 0..BLOCKS {
        let roots = execute_block(&block, workers);
        assert!(roots.consistent());
    }
    let execution_span = started.elapsed();
    let transactions = (BLOCK_SIZE * BLOCKS) as f64;
    let transactions_each_second = transactions / execution_span.as_secs_f64();

    // --- Agreement latency per class ---
    let latencies: Vec<ClassLatency> = [
        TransactionClass::Routine,
        TransactionClass::Financial,
        TransactionClass::Structural,
        TransactionClass::Constitutional,
    ]
    .into_iter()
    .map(measure_class_latency)
    .collect();

    // --- Slot latency ---
    let mut network = FaultNetwork::healthy(VALIDATORS);
    let mut slot_samples: Vec<Duration> = Vec::with_capacity(SLOTS as usize);
    for slot_number in 1..=SLOTS {
        let started = Instant::now();
        let report = network.run_slot(Slot(slot_number));
        slot_samples.push(started.elapsed());
        assert!(report.outcome.committed(), "slot {} committed", slot_number);
        network.assert_no_conflicting_finalisation("the performance run");
    }
    slot_samples.sort();

    println!("--- Cerulea measured performance ---");
    println!(
        "execution throughput: {:.0} transactions each second over {} transactions in {:.3} seconds, {} workers",
        transactions_each_second,
        transactions as u64,
        execution_span.as_secs_f64(),
        workers
    );
    for entry in latencies.iter() {
        println!(
            "agreement latency, {}: median {:.1} microseconds, 95th {:.1}, 99th {:.1}",
            entry.class.label(),
            micros(entry.median),
            micros(entry.ninety_fifth),
            micros(entry.ninety_ninth)
        );
    }
    println!(
        "slot latency across {} validators: median {:.1} microseconds, 95th {:.1}, 99th {:.1}",
        VALIDATORS,
        micros(percentile(&slot_samples, 0.50)),
        micros(percentile(&slot_samples, 0.95)),
        micros(percentile(&slot_samples, 0.99))
    );

    // Every figure holds a sensible shape. A measurement of zero means the
    // clock never moved, which makes the figure meaningless.
    assert!(
        transactions_each_second > 0.0,
        "the execution measurement produced a figure"
    );
    assert!(
        percentile(&slot_samples, 0.99) >= percentile(&slot_samples, 0.50),
        "the 99th percentile sits at or above the median"
    );
    for entry in latencies.iter() {
        assert!(entry.ninety_ninth >= entry.median);
    }

    if let Ok(path) = std::env::var("CERULEA_PERFORMANCE_REPORT") {
        let classes: Vec<String> = latencies
            .iter()
            .map(|entry| {
                format!(
                    "{{\"class\":\"{}\",\"medianMicroseconds\":{:.1},\"ninetyFifthMicroseconds\":{:.1},\"ninetyNinthMicroseconds\":{:.1}}}",
                    entry.class.label(),
                    micros(entry.median),
                    micros(entry.ninety_fifth),
                    micros(entry.ninety_ninth)
                )
            })
            .collect();
        let body = format!(
            "{{\"measured\":true,\"workers\":{},\"validators\":{},\
             \"executionThroughputEachSecond\":{:.0},\"transactionsMeasured\":{},\
             \"agreementLatency\":[{}],\
             \"slotLatencyMicroseconds\":{{\"median\":{:.1},\"ninetyFifth\":{:.1},\"ninetyNinth\":{:.1}}}}}\n",
            workers,
            VALIDATORS,
            transactions_each_second,
            transactions as u64,
            classes.join(","),
            micros(percentile(&slot_samples, 0.50)),
            micros(percentile(&slot_samples, 0.95)),
            micros(percentile(&slot_samples, 0.99))
        );
        fs::write(&path, body).expect("the performance report lands on disk");
    }
}

#[test]
fn a_constitutional_transaction_needs_a_wider_set_than_a_routine_one() {
    let network = TestNetwork::with_validators(VALIDATORS);
    let table = ThresholdTable::new(66, 67, 80, 90);

    let smallest_reaching = |class: TransactionClass| -> usize {
        for size in 1..=VALIDATORS {
            let mut committer =
                DomainCommitter::open(Slot(1), [4u8; 32], network.total_weight(), table)
                    .expect("weight");
            for node in network.nodes.iter().take(size) {
                committer
                    .add_signer(node.seal_key, node.standing)
                    .expect("the signer joins the tally");
            }
            if committer.settle(0, class).committed {
                return size;
            }
        }
        VALIDATORS + 1
    };

    let routine = smallest_reaching(TransactionClass::Routine);
    let constitutional = smallest_reaching(TransactionClass::Constitutional);
    println!(
        "measured: a routine domain commits on {} of {} signers, a constitutional domain on {}",
        routine, VALIDATORS, constitutional
    );
    assert!(
        constitutional >= routine,
        "a constitutional domain asks for at least as many signers as a routine one"
    );
    assert!(TransactionClass::Constitutional.waits_delay_window());
    assert!(!TransactionClass::Routine.waits_delay_window());
}
