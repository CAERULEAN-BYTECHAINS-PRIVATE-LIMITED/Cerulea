//! Author selection and distributed slot-timing validation for the consensus engine.

use crate::engines::authoring::fallback::{AuthoringPolicy, SlotSchedule};
use crate::engines::authoring::verify::ARRIVAL_ALLOWANCE_MS;
use crate::engines::{Rank, Slot};
use crate::error::{ConsensusError, ConsensusResult};
use cerulea_runtime::AccountId;
use pallet_cerulea_dcf::DcfApi as RuntimeDcfApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_core::ed25519::Public;
use sp_runtime::traits::NumberFor;
use std::sync::Arc;

/// Maximum permitted clock drift (in milliseconds) between a validator's local
/// clock and the network/reference clock before standby authoring is blocked.
pub const MAX_CLOCK_DRIFT_MS: u64 = ARRIVAL_ALLOWANCE_MS;

/// Default scheduling jitter guard band (in milliseconds) applied at the start
/// of a standby rank's window to absorb network propagation latency.
pub const DEFAULT_STANDBY_JITTER_MARGIN_MS: u64 = 250;

/// Minimum remaining time (in milliseconds) inside a rank window required to
/// start proposing a block without colliding with the next rank.
pub const DEFAULT_MIN_WINDOW_REMAINING_MS: u64 = 100;

/// Distributed slot-timing policy governing clock drift and standby jitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotTimingPolicy {
    /// Maximum allowed clock drift against reference/peer time (ms).
    pub max_clock_drift_ms: u64,
    /// Guard band after a standby rank window opens before authoring (ms).
    pub standby_jitter_margin_ms: u64,
    /// Minimum time remaining in the window to begin authoring (ms).
    pub min_window_remaining_ms: u64,
}

impl Default for SlotTimingPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

impl SlotTimingPolicy {
    /// Standard distributed slot-timing policy.
    pub const fn standard() -> Self {
        SlotTimingPolicy {
            max_clock_drift_ms: MAX_CLOCK_DRIFT_MS,
            standby_jitter_margin_ms: DEFAULT_STANDBY_JITTER_MARGIN_MS,
            min_window_remaining_ms: DEFAULT_MIN_WINDOW_REMAINING_MS,
        }
    }
}

/// Outcome of evaluating whether a validator rank may author at a given moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotTimingDecision {
    /// The rank's effective authoring window is open and clock drift is within bounds.
    Ready {
        /// The slot number.
        slot: Slot,
        /// The committee rank authorised to author.
        rank: Rank,
    },
    /// Standby rank must wait for the jitter guard band to pass before proposing.
    WaitJitterGuard {
        /// Remaining milliseconds to wait for the jitter guard band.
        wait_ms: u64,
    },
    /// Local clock drift exceeds `max_clock_drift_ms`; authoring is suppressed.
    ClockDriftExceeded {
        /// Observed clock drift in milliseconds.
        drift_ms: u64,
        /// Maximum allowed clock drift in milliseconds.
        max_ms: u64,
    },
    /// The moment is outside the rank's effective authoring window.
    WindowClosed,
}

/// Validate clock drift between a local validator clock and a reference/peer
/// timestamp (in milliseconds).
pub fn validate_slot_clock_drift(
    local_ms: u64,
    reference_ms: u64,
    max_drift_ms: u64,
) -> ConsensusResult<u64> {
    let drift_ms = local_ms.abs_diff(reference_ms);
    if drift_ms > max_drift_ms {
        return Err(ConsensusError::AuthorSelection(format!(
            "Validator clock drift {}ms exceeds maximum {}ms",
            drift_ms, max_drift_ms
        )));
    }
    Ok(drift_ms)
}

/// Evaluate whether `rank` can safely author in `schedule` at `local_ms`
/// given `reference_ms` and `timing_policy`.
pub fn evaluate_slot_timing(
    schedule: &SlotSchedule,
    rank: Rank,
    local_ms: u64,
    reference_ms: u64,
    timing_policy: SlotTimingPolicy,
) -> SlotTimingDecision {
    let drift_ms = local_ms.abs_diff(reference_ms);
    if drift_ms > timing_policy.max_clock_drift_ms {
        return SlotTimingDecision::ClockDriftExceeded {
            drift_ms,
            max_ms: timing_policy.max_clock_drift_ms,
        };
    }

    let Some(window) = schedule.window(rank) else {
        return SlotTimingDecision::WindowClosed;
    };

    if local_ms < window.opens_ms {
        return SlotTimingDecision::WindowClosed;
    }

    // Standby ranks (rank > 1) wait for the jitter guard band after window open
    // so delayed rank-1 blocks do not cause duplicate proposals.
    if rank.0 > Rank::FIRST.0 {
        let effective_open = window
            .opens_ms
            .saturating_add(timing_policy.standby_jitter_margin_ms);
        if local_ms < effective_open {
            return SlotTimingDecision::WaitJitterGuard {
                wait_ms: effective_open - local_ms,
            };
        }
    }

    let effective_close = window
        .closes_ms
        .saturating_sub(timing_policy.min_window_remaining_ms);
    if local_ms >= effective_close {
        return SlotTimingDecision::WindowClosed;
    }

    SlotTimingDecision::Ready {
        slot: schedule.slot,
        rank,
    }
}

/// Validate that distributed validator clocks `(node_index, local_ms)` are
/// synchronized within `timing_policy.max_clock_drift_ms` and that the authoring
/// policy rank timeout exceeds the combined drift + jitter + remaining margins.
pub fn validate_distributed_slot_timing(
    authoring_policy: AuthoringPolicy,
    timing_policy: SlotTimingPolicy,
    validator_clocks_ms: &[u64],
) -> ConsensusResult<()> {
    let required_min_timeout = timing_policy
        .max_clock_drift_ms
        .saturating_add(timing_policy.standby_jitter_margin_ms)
        .saturating_add(timing_policy.min_window_remaining_ms);
    if authoring_policy.rank_timeout_ms <= required_min_timeout {
        return Err(ConsensusError::AuthorSelection(format!(
            "Rank timeout {}ms must exceed combined drift/jitter/tail margin {}ms",
            authoring_policy.rank_timeout_ms, required_min_timeout
        )));
    }

    if let (Some(&min_clock), Some(&max_clock)) = (
        validator_clocks_ms.iter().min(),
        validator_clocks_ms.iter().max(),
    ) {
        let skew_ms = max_clock.saturating_sub(min_clock);
        if skew_ms > timing_policy.max_clock_drift_ms {
            return Err(ConsensusError::AuthorSelection(format!(
                "Distributed validator clock skew {}ms exceeds maximum {}ms",
                skew_ms, timing_policy.max_clock_drift_ms
            )));
        }
    }

    Ok(())
}

/// Fetch the expected block author for a given block number/slot using the DCF runtime API.
pub fn get_expected_author<B, C>(client: Arc<C>, block_number: u32) -> ConsensusResult<Public>
where
    B: sp_runtime::traits::Block,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
{
    let api = client.runtime_api();
    let best_hash = client.info().best_hash;
    match api.get_expected_author(best_hash, block_number) {
        Ok(Some(account_id)) => Ok(Public::from_raw(*account_id.as_ref())),
        Ok(None) => Err(ConsensusError::AuthorSelection(
            "No expected author returned by runtime".into(),
        )),
        Err(e) => Err(ConsensusError::AuthorSelection(format!(
            "Runtime API error: {:?}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_drift_validation_rejects_excessive_skew() {
        assert_eq!(validate_slot_clock_drift(10_200, 10_000, 500).unwrap(), 200);
        assert_eq!(validate_slot_clock_drift(10_000, 10_500, 500).unwrap(), 500);
        assert!(validate_slot_clock_drift(10_600, 10_000, 500).is_err());
    }

    #[test]
    fn standby_rank_respects_jitter_guard_and_tail_margin() {
        let policy = AuthoringPolicy::standard(); // 1,500ms per rank
        let schedule = SlotSchedule::new(Slot(10), 10_000, policy);
        let timing = SlotTimingPolicy::standard();

        // Rank 1 is ready immediately at slot open (10_000ms).
        assert_eq!(
            evaluate_slot_timing(&schedule, Rank(1), 10_000, 10_000, timing),
            SlotTimingDecision::Ready {
                slot: Slot(10),
                rank: Rank(1),
            }
        );

        // Rank 2 opens at 11_500ms, but must wait 250ms jitter guard until 11_750ms.
        assert_eq!(
            evaluate_slot_timing(&schedule, Rank(2), 11_550, 11_550, timing),
            SlotTimingDecision::WaitJitterGuard { wait_ms: 200 }
        );
        assert_eq!(
            evaluate_slot_timing(&schedule, Rank(2), 11_750, 11_750, timing),
            SlotTimingDecision::Ready {
                slot: Slot(10),
                rank: Rank(2),
            }
        );

        // Near window close (13_000 - 100 = 12_900ms), authoring is closed to avoid collision.
        assert_eq!(
            evaluate_slot_timing(&schedule, Rank(2), 12_950, 12_950, timing),
            SlotTimingDecision::WindowClosed
        );

        // Excessive clock drift suppresses authoring.
        assert_eq!(
            evaluate_slot_timing(&schedule, Rank(1), 10_800, 10_000, timing),
            SlotTimingDecision::ClockDriftExceeded {
                drift_ms: 800,
                max_ms: 500,
            }
        );
    }

    #[test]
    fn distributed_slot_timing_checks_skew_and_policy_margins() {
        let authoring = AuthoringPolicy::standard();
        let timing = SlotTimingPolicy::standard();

        // Synchronized 5-validator cluster within 500ms skew passes.
        assert!(validate_distributed_slot_timing(
            authoring,
            timing,
            &[10_000, 10_120, 10_250, 10_390, 10_450]
        )
        .is_ok());

        // Cluster skew of 700ms (> 500ms) fails.
        assert!(validate_distributed_slot_timing(
            authoring,
            timing,
            &[10_000, 10_200, 10_700]
        )
        .is_err());

        // Rank timeout too tight for drift + jitter + tail margin fails.
        assert!(validate_distributed_slot_timing(
            AuthoringPolicy::new(600, 5),
            timing,
            &[10_000, 10_100]
        )
        .is_err());
    }
}
