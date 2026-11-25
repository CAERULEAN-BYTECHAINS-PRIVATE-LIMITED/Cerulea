//! A validator cut off through a commitment catches up from a peer.
//!
//! Five validator nodes run on loopback from the genesis of the multi
//! validator chain. Once they commit, the fourth stops. The other four hold
//! eighty percent of the weight and commit two more checkpoints without it.
//! The fourth then starts again from its own store. On reconnect its peers
//! send the newest commitment they hold, the fourth checks the votes inside it
//! and commits the block, and from there it keeps pace with the others.

mod loopback;

use loopback::Network;
use std::time::Duration;

/// The validator cut off.
const CUT_OFF: usize = 3;

/// One checkpoint interval of the multi validator chain.
const CHECKPOINT: u32 = 10;

#[test]
fn a_validator_cut_off_through_a_commitment_catches_up_from_a_peer() {
    let folder = tempfile::tempdir().expect("a temporary folder opens");
    let mut network = Network::start(folder.path(), |_| Vec::new());
    let (started, heights) = network.wait_until(Duration::from_secs(240), |heights| {
        heights.iter().all(|height| *height >= CHECKPOINT)
    });
    assert!(
        started,
        "the network committed {:?}\n{}",
        heights,
        network.report()
    );

    // Cut the fourth off and let the others commit past it.
    network.nodes[CUT_OFF].stop();
    let left_at = heights[CUT_OFF];
    let others = |heights: &[u32]| -> u32 {
        heights
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != CUT_OFF)
            .map(|(_, height)| *height)
            .min()
            .unwrap_or(0)
    };
    let (moved, heights) = network.wait_until(Duration::from_secs(240), |heights| {
        others(heights) >= left_at + 2 * CHECKPOINT
    });
    assert!(
        moved,
        "the four committed {:?} after the fourth left at {}\n{}",
        heights,
        left_at,
        network.report()
    );

    // The fourth returns and catches up.
    let mark = others(&heights);
    network.restart(CUT_OFF, &[]);
    let (caught_up, heights) =
        network.wait_until(Duration::from_secs(120), |heights| heights[CUT_OFF] >= mark);
    let log = network.nodes[CUT_OFF].log_text();
    network.stop();
    assert!(
        caught_up,
        "the fourth stayed at {:?} against {}\n{}",
        heights,
        mark,
        network.report()
    );
    assert!(
        log.contains("from the commitment a peer sent"),
        "the fourth caught up without a commitment from a peer:\n{}",
        network.report()
    );
}
