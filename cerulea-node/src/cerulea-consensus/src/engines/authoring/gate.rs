//! The local author gate on block production.
//!
//! Block production begins only where the local keystore holds the scheduled
//! author key for the slot at the claimed rank. Every other node stands down
//! and waits.
//!
//! The gate closes the second authorship defect. Before the gate, every
//! validator node attempted every block and a timing guard alone kept two nodes
//! apart. A node that holds no key for the slot now produces nothing at all.
//!
//! # What the gate checks
//!
//! 1. The slot still waits for a block.
//! 2. A rank holds the slot at this moment.
//! 3. The committee gives that rank to a validator.
//! 4. The local keystore holds both keys of that validator.
//!
//! A pass on all four returns a ticket. The ticket carries the slot, the rank,
//! the two keys and the slot draw the node produced from its own keystore. The
//! seal step signs that ticket.

use sp_core::{ed25519, sr25519};
use sp_keystore::{Keystore, KeystorePtr};

use super::draw::{draw_from_keystore, DrawResult};
use super::fallback::FallbackTracker;
use super::seal::{SealClaim, ValidatorKeys};
use super::verify::{SlotContext, ValidatorRegistry};
use crate::engines::{EngineError, Rank, Slot};
use crate::CERULEA_KEY_TYPE;

/// The permission to produce one block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthorTicket {
    /// The slot the block serves.
    pub slot: Slot,
    /// The rank the local validator holds.
    pub rank: Rank,
    /// The two keys of the local validator.
    pub keys: ValidatorKeys,
    /// The slot draw the local node produced.
    pub draw: DrawResult,
    /// The moment the gate opened, in milliseconds.
    pub moment_ms: u64,
}

impl AuthorTicket {
    /// The seal claim this ticket carries into the seal step.
    pub fn seal_claim(&self) -> SealClaim {
        SealClaim::new(
            self.slot,
            self.rank,
            self.keys.seal,
            self.draw.clone(),
            self.moment_ms,
        )
    }
}

/// The reason a node stands down for one slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StandDown {
    /// A block already claimed the slot.
    SlotClaimed,
    /// No rank holds the slot at this moment.
    WindowClosed,
    /// The committee gives this rank to no validator.
    RankEmpty {
        /// The rank holding the slot.
        rank: u8,
    },
    /// The local keystore holds no key for the scheduled author.
    KeyAbsent {
        /// The rank holding the slot.
        rank: u8,
    },
    /// The scheduled author sits outside the validator key registry.
    AuthorUnrecorded {
        /// The rank holding the slot.
        rank: u8,
    },
    /// The licence of the deployment lapsed, so block production stopped.
    ///
    /// Reading stays open throughout. Export stays open throughout.
    ProductionSuspended,
}

impl StandDown {
    /// One plain sentence stating why the node stands down.
    pub fn reason(&self) -> &'static str {
        match self {
            StandDown::SlotClaimed => "A block already claimed this slot.",
            StandDown::WindowClosed => "No rank holds this slot at this moment.",
            StandDown::RankEmpty { .. } => "The committee gives this rank to no validator.",
            StandDown::KeyAbsent { .. } => {
                "The local keystore holds no key for the scheduled author."
            }
            StandDown::AuthorUnrecorded { .. } => {
                "The scheduled author sits outside the validator key registry."
            }
            StandDown::ProductionSuspended => {
                "The licence of the deployment lapsed. Renew the subscription and install the new licence."
            }
        }
    }
}

/// What the gate decided for one slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GateDecision {
    /// The local node produces the block.
    Author(Box<AuthorTicket>),
    /// The local node stands down and waits.
    StandDown(StandDown),
}

impl GateDecision {
    /// The ticket, where the gate opened.
    pub fn ticket(&self) -> Option<&AuthorTicket> {
        match self {
            GateDecision::Author(ticket) => Some(ticket),
            GateDecision::StandDown(_) => None,
        }
    }

    /// Report whether the gate opened.
    pub fn opens(&self) -> bool {
        matches!(self, GateDecision::Author(_))
    }
}

/// The local author gate.
#[derive(Clone)]
pub struct AuthorGate {
    keystore: KeystorePtr,
}

impl AuthorGate {
    /// Build a gate over the node keystore.
    pub fn new(keystore: KeystorePtr) -> Self {
        AuthorGate { keystore }
    }

    /// The draw keys the local keystore holds.
    pub fn local_draw_keys(&self) -> Vec<sr25519::Public> {
        self.keystore.sr25519_public_keys(CERULEA_KEY_TYPE)
    }

    /// The seal keys the local keystore holds.
    pub fn local_seal_keys(&self) -> Vec<ed25519::Public> {
        self.keystore.ed25519_public_keys(CERULEA_KEY_TYPE)
    }

    /// Report whether the local keystore holds both keys of a validator.
    pub fn holds(&self, keys: &ValidatorKeys) -> bool {
        self.local_draw_keys().contains(&keys.draw) && self.local_seal_keys().contains(&keys.seal)
    }

    /// Decide whether the local node produces the block for one slot.
    ///
    /// The call reports a problem only where the keystore itself refuses. Every
    /// other outcome is a decision to author or to stand down.
    pub fn decide<R: ValidatorRegistry>(
        &self,
        context: &SlotContext,
        registry: &R,
        tracker: &FallbackTracker,
        moment_ms: u64,
    ) -> Result<GateDecision, EngineError> {
        if !tracker.waiting() {
            return Ok(GateDecision::StandDown(StandDown::SlotClaimed));
        }
        let rank = match tracker.holder_at(moment_ms) {
            Some(rank) => rank,
            None => return Ok(GateDecision::StandDown(StandDown::WindowClosed)),
        };
        let scheduled = match context.committee.at_rank(rank) {
            Some(validator) => *validator,
            None => {
                return Ok(GateDecision::StandDown(StandDown::RankEmpty {
                    rank: rank.0,
                }))
            }
        };
        let seal_key = match registry.seal_key_of(&scheduled) {
            Some(key) => key,
            None => {
                return Ok(GateDecision::StandDown(StandDown::AuthorUnrecorded {
                    rank: rank.0,
                }))
            }
        };
        let keys = ValidatorKeys::new(scheduled, seal_key);
        if !self.holds(&keys) {
            return Ok(GateDecision::StandDown(StandDown::KeyAbsent {
                rank: rank.0,
            }));
        }
        let draw = draw_from_keystore(&self.keystore, &scheduled, &context.input)?;
        Ok(GateDecision::Author(Box::new(AuthorTicket {
            slot: context.input.slot,
            rank,
            keys,
            draw,
            moment_ms,
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::authoring::committee::CommitteeDraw;
    use crate::engines::authoring::draw::{draw_from_keystore, DrawInput};
    use crate::engines::authoring::fallback::{AuthoringPolicy, DEFAULT_RANK_TIMEOUT_MS};
    use crate::engines::authoring::verify::KeyRegistry;
    use crate::engines::authoring::SlotSchedule;
    use crate::engines::{SlotSeed, TrustScore};
    use sp_keystore::testing::MemoryKeystore;
    use std::sync::Arc;

    const SLOT: u64 = 33;
    const SLOT_OPENS_MS: u64 = 50_000;

    struct Network {
        context: SlotContext,
        registry: KeyRegistry,
        keystores: Vec<KeystorePtr>,
        keys: Vec<ValidatorKeys>,
    }

    fn build(count: usize) -> Network {
        let input = DrawInput::new(Slot(SLOT), SlotSeed([8u8; 32]));
        let mut keystores = Vec::new();
        let mut keys = Vec::new();
        let mut registry = KeyRegistry::new();
        let mut entries = Vec::new();
        for index in 0..count {
            let memory = MemoryKeystore::new();
            let draw_key = memory
                .sr25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//draw//{}", index)),
                )
                .expect("the keystore generates a draw key");
            let seal_key = memory
                .ed25519_generate_new(
                    CERULEA_KEY_TYPE,
                    Some(&format!("//Cerulea//seal//{}", index)),
                )
                .expect("the keystore generates a seal key");
            let store: KeystorePtr = Arc::new(memory);
            let pair = ValidatorKeys::new(draw_key, seal_key);
            registry.record(pair);
            entries.push((
                draw_from_keystore(&store, &draw_key, &input).expect("the node draws"),
                TrustScore::from_percent(50),
            ));
            keystores.push(store);
            keys.push(pair);
        }
        let committee = CommitteeDraw::new()
            .assemble(&input, &entries)
            .expect("the committee assembles");
        let schedule = SlotSchedule::new(Slot(SLOT), SLOT_OPENS_MS, AuthoringPolicy::standard());
        Network {
            context: SlotContext::new(input, committee, schedule),
            registry,
            keystores,
            keys,
        }
    }

    fn index_at_rank(network: &Network, rank: Rank) -> usize {
        let member = network
            .context
            .committee
            .at_rank(rank)
            .expect("the committee holds this rank");
        network
            .keys
            .iter()
            .position(|keys| keys.draw == *member)
            .expect("the member sits in the set")
    }

    #[test]
    fn the_scheduled_author_passes_the_gate() {
        let network = build(8);
        let index = index_at_rank(&network, Rank::FIRST);
        let gate = AuthorGate::new(network.keystores[index].clone());
        let tracker = FallbackTracker::new(network.context.schedule);
        let decision = gate
            .decide(&network.context, &network.registry, &tracker, SLOT_OPENS_MS)
            .expect("the gate decides");
        assert!(decision.opens());
        let ticket = decision.ticket().expect("the gate opened");
        assert_eq!(ticket.rank, Rank::FIRST);
        assert_eq!(ticket.slot, Slot(SLOT));
        assert_eq!(ticket.keys, network.keys[index]);
        let claim = ticket.seal_claim();
        assert_eq!(claim.rank, Rank::FIRST);
        assert_eq!(claim.author, network.keys[index].seal);
    }

    #[test]
    fn a_node_whose_key_differs_from_the_scheduled_author_produces_no_block() {
        let network = build(8);
        let author = index_at_rank(&network, Rank::FIRST);
        let mut stood_down = 0;
        for index in 0..network.keystores.len() {
            if index == author {
                continue;
            }
            let gate = AuthorGate::new(network.keystores[index].clone());
            let tracker = FallbackTracker::new(network.context.schedule);
            let decision = gate
                .decide(&network.context, &network.registry, &tracker, SLOT_OPENS_MS)
                .expect("the gate decides");
            assert!(!decision.opens());
            assert!(decision.ticket().is_none());
            match decision {
                GateDecision::StandDown(reason) => {
                    assert!(matches!(reason, StandDown::KeyAbsent { rank: 1 }));
                    assert!(!reason.reason().is_empty());
                }
                GateDecision::Author(_) => panic!("only the scheduled author passes the gate"),
            }
            stood_down += 1;
        }
        assert_eq!(stood_down, 7);
    }

    #[test]
    fn exactly_one_node_passes_the_gate_at_each_moment() {
        let network = build(9);
        for position in 0..5u64 {
            let moment = SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS * position;
            let passes = network
                .keystores
                .iter()
                .filter(|store| {
                    let gate = AuthorGate::new((*store).clone());
                    let tracker = FallbackTracker::new(network.context.schedule);
                    gate.decide(&network.context, &network.registry, &tracker, moment)
                        .expect("the gate decides")
                        .opens()
                })
                .count();
            assert_eq!(passes, 1, "one node authors at each moment");
        }
    }

    #[test]
    fn rank_two_passes_the_gate_once_rank_one_lets_the_slot_go() {
        let network = build(8);
        let second = index_at_rank(&network, Rank(2));
        let gate = AuthorGate::new(network.keystores[second].clone());
        let tracker = FallbackTracker::new(network.context.schedule);
        assert!(!gate
            .decide(&network.context, &network.registry, &tracker, SLOT_OPENS_MS)
            .expect("the gate decides")
            .opens());
        let handover = SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS;
        let decision = gate
            .decide(&network.context, &network.registry, &tracker, handover)
            .expect("the gate decides");
        assert_eq!(decision.ticket().expect("the gate opened").rank, Rank(2));
    }

    #[test]
    fn a_claimed_slot_closes_the_gate_for_every_node() {
        let network = build(8);
        let mut tracker = FallbackTracker::new(network.context.schedule);
        assert!(tracker.record_claim(Rank::FIRST));
        for store in &network.keystores {
            let gate = AuthorGate::new(store.clone());
            let decision = gate
                .decide(&network.context, &network.registry, &tracker, SLOT_OPENS_MS)
                .expect("the gate decides");
            assert_eq!(decision, GateDecision::StandDown(StandDown::SlotClaimed));
        }
    }

    #[test]
    fn a_moment_outside_every_window_closes_the_gate() {
        let network = build(8);
        let gate = AuthorGate::new(network.keystores[0].clone());
        let tracker = FallbackTracker::new(network.context.schedule);
        let decision = gate
            .decide(
                &network.context,
                &network.registry,
                &tracker,
                network.context.schedule.closes_ms(),
            )
            .expect("the gate decides");
        assert_eq!(decision, GateDecision::StandDown(StandDown::WindowClosed));
        let early = gate
            .decide(&network.context, &network.registry, &tracker, 0)
            .expect("the gate decides");
        assert_eq!(early, GateDecision::StandDown(StandDown::WindowClosed));
    }

    #[test]
    fn an_unrecorded_author_closes_the_gate() {
        let network = build(8);
        let index = index_at_rank(&network, Rank::FIRST);
        let gate = AuthorGate::new(network.keystores[index].clone());
        let tracker = FallbackTracker::new(network.context.schedule);
        let empty = KeyRegistry::new();
        let decision = gate
            .decide(&network.context, &empty, &tracker, SLOT_OPENS_MS)
            .expect("the gate decides");
        assert_eq!(
            decision,
            GateDecision::StandDown(StandDown::AuthorUnrecorded { rank: 1 })
        );
    }

    #[test]
    fn an_empty_rank_closes_the_gate() {
        let network = build(2);
        let gate = AuthorGate::new(network.keystores[0].clone());
        let tracker = FallbackTracker::new(network.context.schedule);
        let moment = SLOT_OPENS_MS + DEFAULT_RANK_TIMEOUT_MS * 3;
        let decision = gate
            .decide(&network.context, &network.registry, &tracker, moment)
            .expect("the gate decides");
        assert_eq!(
            decision,
            GateDecision::StandDown(StandDown::RankEmpty { rank: 4 })
        );
    }

    #[test]
    fn the_gate_reads_back_the_local_keys() {
        let network = build(3);
        let gate = AuthorGate::new(network.keystores[0].clone());
        // The in memory keystore reads every stored seed under both key
        // shapes, so the lists hold the local keys among others.
        assert!(gate.local_draw_keys().contains(&network.keys[0].draw));
        assert!(gate.local_seal_keys().contains(&network.keys[0].seal));
        assert!(gate.holds(&network.keys[0]));
        assert!(!gate.holds(&network.keys[1]));
        assert!(!gate.holds(&network.keys[2]));
    }

    #[test]
    fn every_stand_down_reason_reads_as_a_sentence() {
        let reasons = [
            StandDown::SlotClaimed,
            StandDown::WindowClosed,
            StandDown::RankEmpty { rank: 3 },
            StandDown::KeyAbsent { rank: 1 },
            StandDown::AuthorUnrecorded { rank: 2 },
        ];
        for reason in reasons {
            let text = reason.reason();
            assert!(text.ends_with('.'));
            assert!(text.starts_with(char::is_uppercase));
        }
    }
}
