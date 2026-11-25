//! One validator with its clock thirty seconds ahead.
//!
//! Five validator nodes run on loopback from the genesis of the multi
//! validator chain. The fifth runs with `CERULEA_CLOCK_OFFSET_MS` at thirty
//! seconds, so it names slots and seals blocks thirty seconds ahead of the
//! others. The test asserts two things.
//!
//! 1. **The others keep committing.** The four validators with true clocks
//!    hold eighty percent of the weight, above the sixty seven percent the
//!    Commit Engine needs. Their committed head keeps rising while the fifth
//!    runs ahead.
//! 2. **All five converge.** The fifth restarts from its own store with its
//!    clock true, and every node then commits past the height the four had
//!    reached.
//!
//! The test build carries the `clock-offset` feature, the one build besides
//! the multi-host exercise image that honours the setting.

mod loopback;

use cerulea_consensus::clock::CLOCK_OFFSET_ENV;
use loopback::Network;
use std::time::Duration;

/// The index of the validator whose clock runs ahead.
const AHEAD: usize = 4;

/// How far that clock runs ahead, in milliseconds.
const OFFSET_MS: &str = "30000";

/// One checkpoint interval of the multi validator chain.
const CHECKPOINT: u32 = 10;

#[test]
fn four_true_clocks_keep_committing_and_all_five_converge() {
    let folder = tempfile::tempdir().expect("a temporary folder opens");
    let mut network = Network::start(folder.path(), |name| {
        if name == loopback::VALIDATORS[AHEAD] {
            vec![(CLOCK_OFFSET_ENV, OFFSET_MS.to_string())]
        } else {
            Vec::new()
        }
    });
    let honest = |heights: &[u32]| -> u32 {
        heights
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != AHEAD)
            .map(|(_, height)| *height)
            .min()
            .unwrap_or(0)
    };

    // The four commit past the first checkpoints with the fifth ahead.
    let (started, heights) = network.wait_until(Duration::from_secs(240), |heights| {
        honest(heights) >= 2 * CHECKPOINT
    });
    assert!(
        started,
        "the four true clocks committed {:?}\n{}",
        heights,
        network.report()
    );
    let ahead_log = network.nodes[AHEAD].log_text();
    assert!(
        ahead_log.contains("milliseconds ahead of the system clock"),
        "the fifth node runs without its offset:\n{}",
        network.report()
    );

    // They keep committing while the fifth stays ahead.
    let before = honest(&heights);
    let (kept, heights) = network.wait_until(Duration::from_secs(180), |heights| {
        honest(heights) >= before + 2 * CHECKPOINT
    });
    assert!(
        kept,
        "the four stopped at {:?} after {}\n{}",
        heights,
        before,
        network.report()
    );

    // The fifth returns with a true clock and every node converges.
    network.restart(AHEAD, &[]);
    let mark = honest(&heights);
    let (converged, heights) = network.wait_until(Duration::from_secs(240), |heights| {
        heights.iter().all(|height| *height >= mark + CHECKPOINT)
    });
    network.stop();
    assert!(
        converged,
        "the five did not converge past {}: {:?}\n{}",
        mark + CHECKPOINT,
        heights,
        network.report()
    );
}
