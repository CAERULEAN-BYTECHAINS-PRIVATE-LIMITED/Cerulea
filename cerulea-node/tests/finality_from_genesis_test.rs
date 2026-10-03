//! Finality from genesis on a network of real validator services.
//!
//! Five validator nodes run from the node binary on the loopback interface,
//! each in its own process with its own keystore and database. They start from
//! the genesis of the multi validator chain, author blocks, gossip commit
//! votes and commit checkpoints. The test asserts two things.
//!
//! 1. **Finality advances.** Every node reports a committed head above height
//!    ten within the time limit.
//! 2. **No runtime refusal.** The commit vote gossip validator of every node
//!    records no refusal for a failed runtime answer. A single node never
//!    gossips a vote to anyone, so only a network shows this.

mod loopback;

use loopback::Network;
use std::time::Duration;

/// The committed height every node must pass.
const COMMITTED_ABOVE: u32 = 10;

/// The time the network has to commit above that height.
const LIMIT: Duration = Duration::from_secs(300);

#[test]
fn five_validators_commit_from_genesis_without_runtime_refusals() {
    let folder = tempfile::tempdir().expect("a temporary folder opens");
    let mut network = Network::start(folder.path(), |_| Vec::new());
    let (reached, heights) = network.wait_until(LIMIT, |heights| {
        heights.iter().all(|height| *height > COMMITTED_ABOVE)
    });
    network.stop();

    let refusals: Vec<(&str, usize)> = network
        .nodes
        .iter()
        .map(|node| {
            (
                node.name,
                node.log_text().matches("runtime_api_error").count(),
            )
        })
        .collect();
    assert!(
        reached,
        "committed heights {:?}; runtime refusals {:?}\n{}",
        heights,
        refusals,
        network.report()
    );
    assert!(
        refusals.iter().all(|(_, count)| *count == 0),
        "the gossip validator refused votes on a failed runtime answer: {:?}",
        refusals
    );
}
