//! The parametrised multi-node harness.
//!
//! The harness in `test_harness` runs a healthy network. This module runs the
//! same engines under stated faults. Every fault here is the real behaviour the
//! engines meet in the field rather than a switch inside an engine.
//!
//! 1. **Node loss.** A lost validator sends nothing and receives nothing. The
//!    remaining validators carry on.
//! 2. **Network delay.** A delayed validator signs the block and the signature
//!    arrives after the collection window closes. The Commit Engine counts the
//!    signatures that arrived in time and leaves the rest out.
//! 3. **Storage exhaustion.** A validator whose storage is full imports no
//!    block. It falls behind and returns after the storage clears.
//! 4. **Clock skew.** A skewed validator reads the rank window at a shifted
//!    moment. The Authoring Engine checks rank timing against that moment, so a
//!    skew past the rank window makes the validator refuse the block.
//!
//! The invariant that matters most runs after every slot. No two conflicting
//! blocks commit at one height.

use std::collections::{BTreeMap, BTreeSet};

use cerulea_consensus::engines::authoring::fallback::{FallbackTracker, SlotSchedule};
use cerulea_consensus::engines::authoring::gate::{AuthorGate, GateDecision};
use cerulea_consensus::engines::authoring::seal::seal_and_apply;
use cerulea_consensus::engines::authoring::verify::verify_header;
use cerulea_consensus::engines::commit::collect::{cast_vote, SignatureCollector};
use cerulea_consensus::engines::commit::commit::DomainCommitter;
use cerulea_consensus::engines::{CommitEngine, Rank, Slot, TransactionClass, TrustScore};
use sp_core::ed25519;
use sp_runtime::generic::Digest;
use sp_runtime::traits::Header as HeaderT;

use crate::test_harness::{SlotOutcome, TestHeader, TestNetwork, COLLECTION_WINDOW_MS};

/// The faults one run carries.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FaultProfile {
    /// Validators taken out of the network.
    pub lost: BTreeSet<usize>,
    /// Milliseconds each named validator's signature spends on the wire.
    pub delay_ms: BTreeMap<usize, u64>,
    /// Milliseconds each named validator's clock sits away from the network.
    pub clock_skew_ms: BTreeMap<usize, i64>,
    /// Validators whose storage holds no further block.
    pub storage_full: BTreeSet<usize>,
}

impl FaultProfile {
    /// A run carrying no fault.
    pub fn healthy() -> Self {
        FaultProfile::default()
    }

    /// A run losing the stated validators.
    pub fn losing(indexes: &[usize]) -> Self {
        FaultProfile {
            lost: indexes.iter().copied().collect(),
            ..FaultProfile::default()
        }
    }

    /// A run delaying the signatures of the stated validators.
    pub fn delaying(indexes: &[usize], milliseconds: u64) -> Self {
        FaultProfile {
            delay_ms: indexes.iter().map(|index| (*index, milliseconds)).collect(),
            ..FaultProfile::default()
        }
    }

    /// A run filling the storage of the stated validators.
    pub fn filling_storage(indexes: &[usize]) -> Self {
        FaultProfile {
            storage_full: indexes.iter().copied().collect(),
            ..FaultProfile::default()
        }
    }

    /// A run shifting the clock of the stated validators.
    pub fn skewing(indexes: &[usize], milliseconds: i64) -> Self {
        FaultProfile {
            clock_skew_ms: indexes
                .iter()
                .map(|index| (*index, milliseconds))
                .collect(),
            ..FaultProfile::default()
        }
    }

    /// Join two profiles into one run carrying both.
    pub fn with(mut self, other: &FaultProfile) -> Self {
        self.lost.extend(other.lost.iter().copied());
        self.storage_full.extend(other.storage_full.iter().copied());
        for (index, value) in other.delay_ms.iter() {
            self.delay_ms.insert(*index, *value);
        }
        for (index, value) in other.clock_skew_ms.iter() {
            self.clock_skew_ms.insert(*index, *value);
        }
        self
    }

    /// A plain name for a report.
    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if !self.lost.is_empty() {
            parts.push(format!("node loss {:?}", self.lost));
        }
        if !self.delay_ms.is_empty() {
            parts.push(format!("network delay {:?}", self.delay_ms));
        }
        if !self.storage_full.is_empty() {
            parts.push(format!("storage exhaustion {:?}", self.storage_full));
        }
        if !self.clock_skew_ms.is_empty() {
            parts.push(format!("clock skew {:?}", self.clock_skew_ms));
        }
        if parts.is_empty() {
            return "no fault".to_string();
        }
        parts.join(", ")
    }
}

/// What one slot produced under a fault profile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaultSlotReport {
    /// The outcome the Commit Engine reached.
    pub outcome: SlotOutcome,
    /// The validators that accepted the block on import.
    pub accepted_by: Vec<usize>,
    /// The validators whose signature arrived inside the collection window.
    pub signed_by: Vec<usize>,
    /// The validators that refused the block.
    pub refused_by: Vec<usize>,
}

/// A network of validators running under a fault profile.
pub struct FaultNetwork {
    /// The network the engines run over.
    pub network: TestNetwork,
    /// The faults this run carries.
    pub faults: FaultProfile,
    /// One report per slot, in slot order.
    pub reports: Vec<FaultSlotReport>,
}

impl FaultNetwork {
    /// Build a network of the stated size under the stated faults.
    pub fn new(validators: usize, faults: FaultProfile) -> Self {
        let mut network = TestNetwork::with_validators(validators);
        for index in faults.lost.iter() {
            network.partition(*index);
        }
        FaultNetwork {
            network,
            faults,
            reports: Vec::new(),
        }
    }

    /// Build a healthy network of the stated size.
    pub fn healthy(validators: usize) -> Self {
        FaultNetwork::new(validators, FaultProfile::healthy())
    }

    /// The number of validators taking part.
    pub fn validator_count(&self) -> usize {
        self.network.nodes.len()
    }

    /// Report whether one validator takes part in this slot.
    pub fn takes_part(&self, index: usize) -> bool {
        self.network.nodes[index].reachable && !self.faults.lost.contains(&index)
    }

    /// Report whether one validator can hold a further block.
    pub fn holds_storage(&self, index: usize) -> bool {
        !self.faults.storage_full.contains(&index)
    }

    /// Apply a further fault to a run already under way.
    pub fn inject(&mut self, extra: &FaultProfile) {
        self.faults = self.faults.clone().with(extra);
        for index in extra.lost.iter() {
            self.network.partition(*index);
        }
    }

    /// Clear every fault and bring every validator back up to date.
    pub fn heal(&mut self) {
        self.faults = FaultProfile::healthy();
        for index in 0..self.network.nodes.len() {
            self.network.restore(index);
        }
    }

    /// The moment one validator reads for the network moment.
    fn moment_for(&self, index: usize, moment_ms: u64) -> u64 {
        let skew = self.faults.clock_skew_ms.get(&index).copied().unwrap_or(0);
        if skew >= 0 {
            moment_ms.saturating_add(skew as u64)
        } else {
            moment_ms.saturating_sub(skew.unsigned_abs())
        }
    }

    /// The moment one validator's signature reaches the author.
    fn arrival_for(&self, index: usize, opens_ms: u64) -> u64 {
        opens_ms.saturating_add(self.faults.delay_ms.get(&index).copied().unwrap_or(0))
    }

    /// Run one slot under the fault profile.
    ///
    /// The rank holding the slot authors. Every taking part validator checks
    /// the seal at the moment its own clock reads. Every validator that
    /// accepted the block signs, and the signature reaches the author after
    /// the delay the profile states. The Commit Engine settles on the
    /// signatures that arrived in time.
    pub fn run_slot(&mut self, slot: Slot) -> FaultSlotReport {
        let parent_hash = self.network.parent_hash();
        let context = self.network.context(slot, parent_hash);
        let registry = self.network.registry();
        let schedule = SlotSchedule::for_slot(slot, self.network.policy);

        for rank_index in 0..self.network.policy.committee_size {
            let rank = Rank::from_index(rank_index);
            let window = match schedule.window(rank) {
                Some(window) => window,
                None => continue,
            };
            let moment_ms = window.opens_ms;
            let drawn = match context.committee.at_rank(rank) {
                Some(validator) => *validator,
                None => continue,
            };
            let author = match self.network.nodes.iter().position(|node| {
                node.draw_key == drawn
            }) {
                Some(index) => index,
                None => continue,
            };
            if !self.takes_part(author) || !self.holds_storage(author) {
                continue;
            }

            let ticket = {
                let gate = AuthorGate::new(self.network.nodes[author].keystore.clone());
                let tracker = FallbackTracker::new(schedule);
                match gate
                    .decide(&context, &registry, &tracker, self.moment_for(author, moment_ms))
                    .expect("the keystore answers the gate")
                {
                    GateDecision::Author(ticket) => ticket,
                    GateDecision::StandDown(_) => continue,
                }
            };

            let height = self.network.height() + 1;
            let mut header = TestHeader::new(
                height,
                Default::default(),
                Default::default(),
                sp_core::H256::from(parent_hash),
                Digest::default(),
            );
            seal_and_apply(
                &self.network.nodes[author].keystore,
                &mut header,
                ticket.seal_claim(),
            )
            .expect("the keystore seals the header");
            let hash: [u8; 32] = header.hash().into();

            // Every taking part validator checks the seal at its own moment.
            let mut accepted_by: Vec<usize> = Vec::new();
            let mut refused_by: Vec<usize> = Vec::new();
            for index in 0..self.network.nodes.len() {
                if !self.takes_part(index) {
                    continue;
                }
                let own_moment = self.moment_for(index, moment_ms);
                if verify_header(&header, &context, &registry, own_moment).is_ok() {
                    accepted_by.push(index);
                } else {
                    refused_by.push(index);
                }
            }

            // Every validator that accepted the block signs it. The signature
            // reaches the author after the delay the profile states.
            let set: Vec<(ed25519::Public, TrustScore)> = self
                .network
                .nodes
                .iter()
                .map(|node| (node.seal_key, node.standing))
                .collect();
            let mut collector =
                SignatureCollector::open(slot, hash, &set, moment_ms, COLLECTION_WINDOW_MS)
                    .expect("the validator set holds members");
            let mut signed_by: Vec<usize> = Vec::new();
            for index in accepted_by.iter().copied() {
                let node = &self.network.nodes[index];
                let vote = cast_vote(&node.keystore, &node.seal_key, slot, hash)
                    .expect("the keystore signs the vote");
                let arrival = self.arrival_for(index, moment_ms);
                if collector.accept(vote, arrival).is_ok() {
                    signed_by.push(index);
                }
            }

            let mut committer = DomainCommitter::open(
                slot,
                hash,
                self.network.total_weight(),
                self.network.thresholds,
            )
            .expect("the validator set carries weight");
            for index in signed_by.iter().copied() {
                let node = &self.network.nodes[index];
                committer
                    .add_signer(node.seal_key, node.standing)
                    .expect("the signer joins the tally");
            }
            let signed_percent = committer.signed_percent();
            let committed = committer.settle(0, TransactionClass::Routine).committed;

            // A validator that accepted the block writes it, unless its
            // storage holds no further block.
            self.network.accepted.entry(height).or_default().insert(hash);
            for index in accepted_by.iter().copied() {
                if !self.holds_storage(index) {
                    continue;
                }
                let node = &mut self.network.nodes[index];
                node.chain.push(header.clone());
                if committed {
                    node.committed.insert(height, hash);
                }
            }

            let outcome = if committed {
                SlotOutcome::Committed {
                    height,
                    hash,
                    rank: rank.0,
                    signed_percent,
                }
            } else {
                SlotOutcome::Withheld {
                    height,
                    signed_percent,
                }
            };
            let report = FaultSlotReport {
                outcome,
                accepted_by,
                signed_by,
                refused_by,
            };
            self.reports.push(report.clone());
            return report;
        }

        let report = FaultSlotReport {
            outcome: SlotOutcome::NoAuthor,
            accepted_by: Vec::new(),
            signed_by: Vec::new(),
            refused_by: Vec::new(),
        };
        self.reports.push(report.clone());
        report
    }

    /// Run the stated number of slots, starting at slot one.
    pub fn run_slots(&mut self, count: u64) -> Vec<FaultSlotReport> {
        (1..=count).map(|number| self.run_slot(Slot(number))).collect()
    }

    /// The number of slots that committed a block.
    pub fn committed_count(&self) -> usize {
        self.reports
            .iter()
            .filter(|report| report.outcome.committed())
            .count()
    }

    /// The number of slots that held a block back.
    pub fn withheld_count(&self) -> usize {
        self.reports
            .iter()
            .filter(|report| matches!(report.outcome, SlotOutcome::Withheld { .. }))
            .count()
    }

    /// The invariant that matters most.
    ///
    /// The call answers with the height where two conflicting blocks committed.
    /// A healthy run and a faulted run both answer with nothing.
    pub fn conflicting_commit(&self) -> Option<u32> {
        self.network.conflicting_commit()
    }

    /// Stop the run where two conflicting blocks committed at one height.
    ///
    /// Every multi-node test and every chaos case calls this after each slot.
    /// The call names the height, so a failure reads plainly.
    pub fn assert_no_conflicting_finalisation(&self, moment: &str) {
        if let Some(height) = self.conflicting_commit() {
            panic!(
                "two conflicting blocks finalised at height {} during {}. \
                 Fix: hold the commit threshold above half the Trust Score weight \
                 so two signer sets always share a member.",
                height, moment
            );
        }
        for (height, hashes) in self.network.accepted.iter() {
            if hashes.len() > 1 {
                let committed: BTreeSet<[u8; 32]> = self
                    .network
                    .nodes
                    .iter()
                    .filter_map(|node| node.committed.get(height).copied())
                    .collect();
                assert!(
                    committed.len() <= 1,
                    "height {} finalised {} conflicting blocks during {}",
                    height,
                    committed.len(),
                    moment
                );
            }
        }
    }

    /// The head every validator that holds storage agrees on.
    pub fn agreed_head(&self) -> Option<[u8; 32]> {
        let heads: BTreeSet<[u8; 32]> = self
            .network
            .nodes
            .iter()
            .enumerate()
            .filter(|(index, node)| self.takes_part(*index) && self.holds_storage(*index) && node.height() > 0)
            .map(|(_, node)| node.head_hash())
            .collect();
        if heads.len() == 1 {
            heads.into_iter().next()
        } else {
            None
        }
    }
}
