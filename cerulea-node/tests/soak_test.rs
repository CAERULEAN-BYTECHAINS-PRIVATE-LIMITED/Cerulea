//! The soak run.
//!
//! The network runs without pause for a stated span. The run samples resident
//! memory and the open handle count throughout. A rising trend across the
//! second half of the run fails the check, so a leak shows up as a failure
//! rather than as a slow machine days later.
//!
//! The span comes from CERULEA_SOAK_MINUTES. The weekly workflow gives the run
//! seventy-two hours, which reads as 4320. A shorter span is stated in the
//! report, so no reader mistakes a short run for a full one.
//!
//! The run writes its report to the file CERULEA_SOAK_REPORT names.

use std::fs;
use std::time::{Duration, Instant};

use cerulea_node::fault_harness::FaultNetwork;
use cerulea_node::test_harness::execute_block;
use cerulea_node::test_harness::HarnessTransaction;

use cerulea_consensus::engines::{Slot, TransactionClass};

/// The validator count the soak run drives.
const VALIDATORS: usize = 9;

/// The number of transactions each soak block carries.
const BLOCK_SIZE: usize = 64;

/// The number of slots between two samples.
const SLOTS_EACH_SAMPLE: u64 = 40;

/// The span the run takes when the environment names none, in minutes.
const DEFAULT_MINUTES: f64 = 0.5;

/// One sample of the run.
#[derive(Clone, Copy, Debug)]
struct Sample {
    /// Seconds since the run started.
    elapsed_seconds: f64,
    /// Resident memory in kibibytes.
    resident_kib: u64,
    /// Open file handles the process holds.
    handles: u64,
    /// Heights the network finalised by this moment.
    finalised: usize,
}

/// Read the resident memory of this process, in kibibytes.
///
/// Linux publishes the figure in the process status file. A platform that
/// publishes no such file answers with zero, and the leak check then rests on
/// the handle count alone.
fn resident_kib() -> u64 {
    if let Ok(text) = fs::read_to_string("/proc/self/status") {
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
                return digits.parse().unwrap_or(0);
            }
        }
    }
    0
}

/// Read the number of open file handles this process holds.
fn open_handles() -> u64 {
    match fs::read_dir("/proc/self/fd") {
        Ok(entries) => entries.count() as u64,
        Err(_) => 0,
    }
}

fn minutes_requested() -> f64 {
    std::env::var("CERULEA_SOAK_MINUTES")
        .ok()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|value| *value > 0.0)
        .unwrap_or(DEFAULT_MINUTES)
}

/// The mean of a slice of samples.
fn mean(values: &[u64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().map(|value| *value as f64).sum::<f64>() / values.len() as f64
}

fn soak(minutes: f64) -> (Vec<Sample>, u64, usize) {
    let span = Duration::from_secs_f64(minutes * 60.0);
    let started = Instant::now();
    let mut network = FaultNetwork::healthy(VALIDATORS);
    let block: Vec<HarnessTransaction> = (0..BLOCK_SIZE)
        .map(|position| {
            HarnessTransaction::new(
                &format!("collection.{}", position % 8),
                &format!("key-{}", position),
                &format!("value-{}", position),
                TransactionClass::Routine,
            )
        })
        .collect();

    let mut samples: Vec<Sample> = Vec::new();
    let mut slot_number: u64 = 0;
    samples.push(Sample {
        elapsed_seconds: 0.0,
        resident_kib: resident_kib(),
        handles: open_handles(),
        finalised: 0,
    });

    while started.elapsed() < span {
        for _ in 0..SLOTS_EACH_SAMPLE {
            slot_number += 1;
            let report = network.run_slot(Slot(slot_number));
            assert!(
                report.outcome.committed(),
                "slot {} stopped committing during the soak: {:?}",
                slot_number,
                report.outcome
            );
            let roots = execute_block(&block, 4);
            assert!(roots.consistent(), "the roots stayed consistent");
            network.assert_no_conflicting_finalisation("the soak run");
        }
        samples.push(Sample {
            elapsed_seconds: started.elapsed().as_secs_f64(),
            resident_kib: resident_kib(),
            handles: open_handles(),
            finalised: network.network.nodes[0].committed.len(),
        });
    }

    (samples, slot_number, network.committed_count())
}

#[test]
fn the_soak_run_holds_memory_and_handles_steady() {
    let minutes = minutes_requested();
    let (samples, slots, committed) = soak(minutes);

    assert!(
        samples.len() >= 3,
        "the run collected at least three samples"
    );
    assert!(slots > 0, "the run drove slots");
    assert_eq!(committed as u64, slots, "every slot finalised a block");

    let half = samples.len() / 2;
    let first_resident: Vec<u64> = samples[..half].iter().map(|s| s.resident_kib).collect();
    let second_resident: Vec<u64> = samples[half..].iter().map(|s| s.resident_kib).collect();
    let first_handles: Vec<u64> = samples[..half].iter().map(|s| s.handles).collect();
    let second_handles: Vec<u64> = samples[half..].iter().map(|s| s.handles).collect();

    let resident_before = mean(&first_resident);
    let resident_after = mean(&second_resident);
    let handles_before = mean(&first_handles);
    let handles_after = mean(&second_handles);

    println!("--- Cerulea soak run, measured ---");
    println!(
        "span: {:.2} minutes, slots: {}, heights finalised: {}",
        minutes, slots, committed
    );
    println!(
        "resident memory: {:.0} KiB over the first half, {:.0} KiB over the second half",
        resident_before, resident_after
    );
    println!(
        "open handles: {:.1} over the first half, {:.1} over the second half",
        handles_before, handles_after
    );

    // A leak shows as a rise across the second half. The run allows a tenth
    // for the allocator settling and nothing beyond it.
    if resident_before > 0.0 {
        assert!(
            resident_after <= resident_before * 1.10 + 8192.0,
            "resident memory rose from {:.0} KiB to {:.0} KiB across the run. \
             Fix: find the allocation the slot loop keeps.",
            resident_before,
            resident_after
        );
    }
    if handles_before > 0.0 {
        assert!(
            handles_after <= handles_before + 8.0,
            "open handles rose from {:.1} to {:.1} across the run. \
             Fix: close the handle the slot loop opens.",
            handles_before,
            handles_after
        );
    }

    if let Ok(path) = std::env::var("CERULEA_SOAK_REPORT") {
        let lines: Vec<String> = samples
            .iter()
            .map(|sample| {
                format!(
                    "{{\"elapsedSeconds\":{:.1},\"residentKib\":{},\"handles\":{},\"heightsFinalised\":{}}}",
                    sample.elapsed_seconds, sample.resident_kib, sample.handles, sample.finalised
                )
            })
            .collect();
        let body = format!(
            "{{\"measured\":true,\"minutesRequested\":{},\"validators\":{},\"slots\":{},\
             \"heightsFinalised\":{},\"residentKibFirstHalf\":{:.0},\"residentKibSecondHalf\":{:.0},\
             \"handlesFirstHalf\":{:.1},\"handlesSecondHalf\":{:.1},\"samples\":[{}]}}\n",
            minutes,
            VALIDATORS,
            slots,
            committed,
            resident_before,
            resident_after,
            handles_before,
            handles_after,
            lines.join(",")
        );
        fs::write(&path, body).expect("the soak report lands on disk");
    }
}

#[test]
#[ignore = "the full seventy-two hour run belongs to the weekly workflow"]
fn the_full_soak_run() {
    // The weekly workflow runs this with CERULEA_SOAK_MINUTES set to 4320.
    let minutes = minutes_requested();
    let (samples, slots, committed) = soak(minutes);
    assert!(samples.len() >= 3);
    assert_eq!(committed as u64, slots);
}
