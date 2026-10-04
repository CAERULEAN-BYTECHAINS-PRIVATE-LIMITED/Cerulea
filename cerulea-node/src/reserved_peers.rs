//! Reserved validator peers stay connected.
//!
//! A reserved peer is a validator the operator names at start. The block sync
//! layer bans any peer that disconnects three times while a request to it is
//! in flight, and a banned peer stays out for about a minute while its
//! standing recovers. On a network under delay or loss, honest validators
//! disconnect that way, and the ban then cuts the very links the Commit
//! Engine needs to gather votes.
//!
//! This keeper watches the standing of every reserved peer once a second. A
//! reserved peer the network banned has its standing restored after a wait,
//! and the connection layer dials it again. The wait starts at one second and
//! doubles with every repeated ban, up to one minute, so a peer that keeps
//! failing is retried with backoff rather than at once. A reserved peer that
//! stays in good standing for ten minutes starts again at the shortest wait.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

/// The standing below which the network bans a peer.
pub const BANNED_THRESHOLD: i32 = sc_network::peer_store::BANNED_THRESHOLD;

/// The first wait before a banned reserved peer is restored.
pub const FIRST_BACKOFF: Duration = Duration::from_secs(1);

/// The longest wait before a banned reserved peer is restored.
pub const MAX_BACKOFF: Duration = Duration::from_secs(60);

/// The time in good standing after which the wait starts again at its
/// shortest.
pub const QUIET_RESET: Duration = Duration::from_secs(600);

/// The reason the standing change carries.
pub const RESTORE_REASON: &str = "Reserved validator peer restored after backoff";

/// The backoff state of one reserved peer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PeerBackoff {
    /// The restores since the peer last held good standing for long.
    restores: u32,
    /// The moment the current ban was first seen.
    banned_since: Option<Instant>,
    /// The moment of the last restore.
    last_restore: Option<Instant>,
}

/// The wait before the next restore, after a number of restores.
pub fn backoff_after(restores: u32) -> Duration {
    let doubled = FIRST_BACKOFF.saturating_mul(1u32.checked_shl(restores).unwrap_or(u32::MAX));
    doubled.min(MAX_BACKOFF)
}

/// Decides when each banned reserved peer has its standing restored.
#[derive(Debug, Default)]
pub struct ReservedPeerKeeper<P: Hash + Eq + Clone> {
    peers: HashMap<P, PeerBackoff>,
}

impl<P: Hash + Eq + Clone> ReservedPeerKeeper<P> {
    /// A keeper with no history.
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Read the standing of one reserved peer at one moment.
    ///
    /// The answer is the standing change that restores the peer, where its
    /// ban has outlasted its wait, and nothing otherwise. The change brings
    /// the standing back to zero.
    pub fn observe(&mut self, peer: &P, standing: i32, now: Instant) -> Option<i32> {
        let state = self.peers.entry(peer.clone()).or_default();
        if standing >= BANNED_THRESHOLD {
            state.banned_since = None;
            if let Some(last) = state.last_restore {
                if now.saturating_duration_since(last) >= QUIET_RESET {
                    state.restores = 0;
                    state.last_restore = None;
                }
            }
            return None;
        }
        let since = *state.banned_since.get_or_insert(now);
        if now.saturating_duration_since(since) < backoff_after(state.restores) {
            return None;
        }
        state.restores = state.restores.saturating_add(1);
        state.banned_since = None;
        state.last_restore = Some(now);
        Some(standing.saturating_neg().max(0))
    }

    /// The restores one peer has had since it last held good standing for
    /// long.
    pub fn restores(&self, peer: &P) -> u32 {
        self.peers
            .get(peer)
            .map(|state| state.restores)
            .unwrap_or(0)
    }
}

/// Start the keeper for the reserved peers of one network.
pub fn spawn<N>(spawner: &sc_service::SpawnTaskHandle, network: std::sync::Arc<N>)
where
    N: sc_network::service::traits::NetworkPeers + ?Sized + Send + Sync + 'static,
{
    spawner.spawn("reserved-peer-keeper", None, async move {
        let mut keeper: ReservedPeerKeeper<sc_network::PeerId> = ReservedPeerKeeper::new();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let Ok(reserved) = network.reserved_peers().await else {
                continue;
            };
            let now = Instant::now();
            for peer in reserved {
                let standing = network.peer_reputation(&peer);
                if let Some(change) = keeper.observe(&peer, standing, now) {
                    log::info!(
                        "Reserved peers: restored the standing of reserved validator peer {} after restore {} (it stood at {}).",
                        peer,
                        keeper.restores(&peer),
                        standing
                    );
                    network.report_peer(
                        peer,
                        sc_network::ReputationChange::new(change, RESTORE_REASON),
                    );
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fatal standing the sync layer reports for a peer that disconnects
    /// with a request in flight after its backoffs.
    const FATAL: i32 = i32::MIN;

    #[test]
    fn a_reserved_peer_in_good_standing_is_left_alone() {
        let mut keeper = ReservedPeerKeeper::new();
        let now = Instant::now();
        assert_eq!(keeper.observe(&1u8, 0, now), None);
        assert_eq!(keeper.observe(&1u8, BANNED_THRESHOLD, now), None);
        assert_eq!(keeper.restores(&1u8), 0);
    }

    #[test]
    fn a_banned_reserved_peer_is_restored_after_its_wait() {
        let mut keeper = ReservedPeerKeeper::new();
        let start = Instant::now();
        // The ban is seen; the first wait is one second.
        assert_eq!(keeper.observe(&1u8, FATAL, start), None);
        assert_eq!(
            keeper.observe(&1u8, FATAL, start + Duration::from_millis(500)),
            None
        );
        let change = keeper
            .observe(&1u8, FATAL, start + FIRST_BACKOFF)
            .expect("the peer is restored after one second");
        // The change brings the standing back to zero, out of the ban.
        assert_eq!(FATAL.saturating_add(change), -1);
        assert!(FATAL.saturating_add(change) >= BANNED_THRESHOLD);
        assert_eq!(keeper.restores(&1u8), 1);
    }

    #[test]
    fn a_peer_banned_again_and_again_waits_longer_each_time() {
        let mut keeper = ReservedPeerKeeper::new();
        let mut now = Instant::now();
        let mut waits = Vec::new();
        for _ in 0..9 {
            let seen = now;
            assert_eq!(keeper.observe(&1u8, FATAL, seen), None);
            let wait = backoff_after(keeper.restores(&1u8));
            assert_eq!(
                keeper.observe(&1u8, FATAL, seen + wait - Duration::from_millis(1)),
                None
            );
            assert!(keeper.observe(&1u8, FATAL, seen + wait).is_some());
            waits.push(wait.as_secs());
            now = seen + wait + Duration::from_secs(1);
        }
        assert_eq!(waits, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
    }

    #[test]
    fn a_long_quiet_spell_starts_the_wait_again_at_its_shortest() {
        let mut keeper = ReservedPeerKeeper::new();
        let start = Instant::now();
        keeper.observe(&1u8, FATAL, start);
        keeper.observe(&1u8, FATAL, start + Duration::from_secs(1));
        keeper.observe(&1u8, FATAL, start + Duration::from_secs(2));
        keeper.observe(&1u8, FATAL, start + Duration::from_secs(4));
        assert_eq!(keeper.restores(&1u8), 2);
        // Good standing for less than the quiet spell keeps the count.
        keeper.observe(&1u8, 0, start + Duration::from_secs(60));
        assert_eq!(keeper.restores(&1u8), 2);
        // Good standing for the quiet spell resets it.
        keeper.observe(&1u8, 0, start + Duration::from_secs(4) + QUIET_RESET);
        assert_eq!(keeper.restores(&1u8), 0);
    }

    #[test]
    fn peers_keep_separate_waits() {
        let mut keeper = ReservedPeerKeeper::new();
        let start = Instant::now();
        keeper.observe(&1u8, FATAL, start);
        assert!(keeper.observe(&1u8, FATAL, start + FIRST_BACKOFF).is_some());
        // The second peer starts its own first wait.
        assert_eq!(keeper.observe(&2u8, FATAL, start + FIRST_BACKOFF), None);
        assert!(keeper
            .observe(&2u8, FATAL, start + FIRST_BACKOFF * 2)
            .is_some());
        assert_eq!(keeper.restores(&1u8), 1);
        assert_eq!(keeper.restores(&2u8), 1);
    }

    #[test]
    fn the_wait_saturates_at_one_minute() {
        assert_eq!(backoff_after(0), Duration::from_secs(1));
        assert_eq!(backoff_after(5), Duration::from_secs(32));
        assert_eq!(backoff_after(6), Duration::from_secs(60));
        assert_eq!(backoff_after(40), MAX_BACKOFF);
    }
}
