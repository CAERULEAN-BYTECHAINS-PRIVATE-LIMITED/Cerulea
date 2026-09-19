//! Verification sampling and its scoring.
//!
//! Validators re-execute a randomly drawn sample of recent transactions and
//! report the results. Agreement raises standing. Catching a genuine fault
//! raises standing sharply.
//!
//! The draw reads the block hash and the round number, so every node draws the
//! same sample and a validator cannot pick the transactions it prefers. The
//! sample spans the recent window rather than the whole history, since the
//! Custody Engine already covers the full history.

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::RuntimeDebug;
use scale_info::TypeInfo;
use sp_core::H256;
use sp_runtime::Perbill;
use sp_std::vec::Vec;

/// The domain separator mixed into every sampling draw.
pub const SAMPLING_DRAW_DOMAIN: &[u8] = b"cerulea.standing.sampling.v1";

/// The transactions one round draws, by default.
pub const DEFAULT_SAMPLE_SIZE: u32 = 8;

/// The share one agreement gains, in parts per billion.
pub const AGREEMENT_GAIN: u32 = 20_000_000;

/// The share one caught fault gains, in parts per billion.
///
/// Catching a genuine fault is the outcome the network values most, so the gain
/// is five times an agreement.
pub const FAULT_CAUGHT_GAIN: u32 = 100_000_000;

/// The share one false report costs, in parts per billion.
///
/// A false report claims a fault the chain rejects, or claims agreement where
/// the chain disagrees. Either way the report misleads the network.
pub const FALSE_REPORT_LOSS: u32 = 200_000_000;

/// The share a validator carries on its first sampling round, in parts per
/// billion.
pub const STARTING_SHARE: u32 = 500_000_000;

/// The result one validator reports for one sampled transaction.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub enum SampleOutcome {
    /// The re-execution matched the committed result.
    #[default]
    Agreed,
    /// The re-execution found a genuine fault the chain confirmed.
    FaultCaught,
    /// The report disagreed with the chain and the chain rejected the claim.
    FalseReport,
}

impl SampleOutcome {
    /// A plain sentence stating what the outcome means.
    pub const fn plain_language(&self) -> &'static str {
        match self {
            SampleOutcome::Agreed => {
                "The re-execution matched the committed result. Standing rises."
            }
            SampleOutcome::FaultCaught => {
                "The re-execution found a genuine fault. Standing rises sharply."
            }
            SampleOutcome::FalseReport => {
                "The report disagreed with the chain and the chain rejected the claim. Standing falls."
            }
        }
    }
}

/// The sampling record of one validator.
#[derive(
    Encode,
    Decode,
    Clone,
    Copy,
    PartialEq,
    Eq,
    RuntimeDebug,
    TypeInfo,
    MaxEncodedLen,
    Default,
    frame_support::__private::codec::DecodeWithMemTracking,
)]
pub struct SamplingRecord {
    /// Reports that matched the committed result.
    pub agreements: u32,
    /// Genuine faults the validator caught.
    pub faults_caught: u32,
    /// Reports the chain rejected.
    pub false_reports: u32,
    /// The sampling share of the Trust Score, in parts per billion.
    pub share: u32,
}

impl SamplingRecord {
    /// The reports this validator has settled.
    pub const fn settled(&self) -> u32 {
        self.agreements
            .saturating_add(self.faults_caught)
            .saturating_add(self.false_reports)
    }

    /// The sampling share as a share value.
    pub fn share(&self) -> Perbill {
        if self.settled() == 0 {
            Perbill::from_parts(STARTING_SHARE)
        } else {
            Perbill::from_parts(self.share)
        }
    }
}

/// Draw the sample one round re-executes.
///
/// The draw reads the block hash and the round number. Every node reaches the
/// same positions, so every validator re-executes the same transactions.
///
/// The returned positions are distinct and sit inside the recent window.
pub fn draw_sample(block_hash: H256, round: u64, window: u32, sample_size: u32) -> Vec<u32> {
    let mut chosen: Vec<u32> = Vec::new();
    if window == 0 || sample_size == 0 {
        return chosen;
    }
    let wanted = sample_size.min(window) as usize;

    let mut attempt = 0u32;
    while chosen.len() < wanted && attempt < wanted as u32 * 16 + 64 {
        let mut preimage = Vec::with_capacity(SAMPLING_DRAW_DOMAIN.len() + 32 + 8 + 4);
        preimage.extend_from_slice(SAMPLING_DRAW_DOMAIN);
        preimage.extend_from_slice(block_hash.as_bytes());
        preimage.extend_from_slice(&round.to_be_bytes());
        preimage.extend_from_slice(&attempt.to_be_bytes());
        let digest = sp_io::hashing::blake2_256(&preimage);
        let mut raw = [0u8; 4];
        raw.copy_from_slice(&digest[..4]);
        let position = u32::from_be_bytes(raw) % window;
        if !chosen.contains(&position) {
            chosen.push(position);
        }
        attempt = attempt.saturating_add(1);
    }
    chosen.sort_unstable();
    chosen
}

/// Settle one sampling report against a record.
pub fn settle(record: &mut SamplingRecord, outcome: SampleOutcome) -> Perbill {
    let previous = if record.settled() == 0 {
        STARTING_SHARE
    } else {
        record.share
    };

    let whole = Perbill::one().deconstruct();
    let current = match outcome {
        SampleOutcome::Agreed => previous.saturating_add(AGREEMENT_GAIN).min(whole),
        SampleOutcome::FaultCaught => previous.saturating_add(FAULT_CAUGHT_GAIN).min(whole),
        SampleOutcome::FalseReport => previous.saturating_sub(FALSE_REPORT_LOSS),
    };

    match outcome {
        SampleOutcome::Agreed => record.agreements = record.agreements.saturating_add(1),
        SampleOutcome::FaultCaught => record.faults_caught = record.faults_caught.saturating_add(1),
        SampleOutcome::FalseReport => record.false_reports = record.false_reports.saturating_add(1),
    }
    record.share = current;
    Perbill::from_parts(current)
}

/// Report whether a sampling record counts as accurate.
///
/// An accurate record carries no rejected report in the recent window. The
/// Standing Engine reads this when it counts an epoch of service.
pub const fn is_accurate(record: &SamplingRecord) -> bool {
    record.false_reports == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_node_draws_the_same_sample() {
        let hash = H256::repeat_byte(3);
        assert_eq!(draw_sample(hash, 7, 100, 8), draw_sample(hash, 7, 100, 8));
    }

    #[test]
    fn a_different_round_draws_a_different_sample() {
        let hash = H256::repeat_byte(3);
        assert_ne!(draw_sample(hash, 7, 1000, 8), draw_sample(hash, 8, 1000, 8));
    }

    #[test]
    fn the_sample_holds_distinct_positions_inside_the_window() {
        let sample = draw_sample(H256::repeat_byte(5), 1, 50, 10);
        assert_eq!(sample.len(), 10);
        for position in sample.iter() {
            assert!(*position < 50);
        }
        let mut sorted = sample.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), sample.len());
    }

    #[test]
    fn a_sample_larger_than_the_window_takes_the_whole_window() {
        let sample = draw_sample(H256::repeat_byte(5), 1, 4, 20);
        assert_eq!(sample.len(), 4);
    }

    #[test]
    fn an_empty_window_draws_nothing() {
        assert!(draw_sample(H256::repeat_byte(1), 1, 0, 8).is_empty());
        assert!(draw_sample(H256::repeat_byte(1), 1, 10, 0).is_empty());
    }

    #[test]
    fn agreement_raises_the_sampling_share() {
        let mut record = SamplingRecord::default();
        let before = record.share();
        settle(&mut record, SampleOutcome::Agreed);
        assert!(record.share() > before);
        assert_eq!(record.agreements, 1);
    }

    #[test]
    fn catching_a_fault_raises_the_share_sharply() {
        let mut agreed = SamplingRecord::default();
        let mut caught = SamplingRecord::default();
        settle(&mut agreed, SampleOutcome::Agreed);
        settle(&mut caught, SampleOutcome::FaultCaught);
        assert!(caught.share() > agreed.share());
        assert_eq!(caught.faults_caught, 1);
    }

    #[test]
    fn a_rejected_report_lowers_the_share() {
        let mut record = SamplingRecord::default();
        let before = record.share();
        settle(&mut record, SampleOutcome::FalseReport);
        assert!(record.share() < before);
        assert_eq!(record.false_reports, 1);
        assert!(!is_accurate(&record));
    }

    #[test]
    fn steady_agreement_reaches_the_whole_share() {
        let mut record = SamplingRecord::default();
        for _ in 0..40 {
            settle(&mut record, SampleOutcome::Agreed);
        }
        assert_eq!(record.share(), Perbill::one());
    }

    #[test]
    fn repeated_rejected_reports_reach_zero() {
        let mut record = SamplingRecord::default();
        for _ in 0..5 {
            settle(&mut record, SampleOutcome::FalseReport);
        }
        assert_eq!(record.share(), Perbill::zero());
    }

    #[test]
    fn a_fresh_record_carries_the_starting_share() {
        let record = SamplingRecord::default();
        assert_eq!(record.share(), Perbill::from_parts(STARTING_SHARE));
        assert!(is_accurate(&record));
    }

    #[test]
    fn every_outcome_carries_a_plain_sentence() {
        for outcome in [
            SampleOutcome::Agreed,
            SampleOutcome::FaultCaught,
            SampleOutcome::FalseReport,
        ] {
            assert!(outcome.plain_language().len() > 30);
        }
    }
}
