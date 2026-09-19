//! The single commit path.
//!
//! One mechanism makes a block irreversible. The Commit Engine gathers
//! signatures, weighs them by Trust Score and commits a domain once the
//! aggregated weight crosses the threshold for the highest class that domain
//! carries. The ledger in this module records what committed and broadcasts
//! each commitment to the rest of the node.
//!
//! Two other paths used to run beside that one. A confirmation counter on the
//! node advanced a block after a fixed number of descendants, with no weight
//! and no signature behind it. A marker in the framework pallet advanced
//! alongside the vote based path. Both are gone from the node. A block becomes
//! irreversible through the Commit Engine alone.
//!
//! The ledger holds a bounded window of recent commitments. Older entries leave
//! as newer ones arrive, so node memory stays inside a stated ceiling.

use std::collections::BTreeMap;
use std::sync::Arc;

use log::{debug, info, warn};
use sp_runtime::traits::{Block as BlockTrait, NumberFor};
use tokio::sync::mpsc;

use crate::engines::commit::commit::DomainCommitment;
use crate::engines::Slot;

/// The number of recent commitments the ledger holds.
pub const DEFAULT_LEDGER_DEPTH: usize = 1_024;

/// One commitment the Commit Engine reached.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitRecord<Block: BlockTrait> {
    /// The slot the block served.
    pub slot: Slot,
    /// The number of the committed block.
    pub block_number: NumberFor<Block>,
    /// The hash of the committed block.
    pub block_hash: Block::Hash,
    /// The number of validators that signed.
    pub signer_count: usize,
    /// The Trust Score weight the signers carried.
    pub signed_weight: u128,
    /// The Trust Score weight the whole set carries.
    pub total_weight: u128,
    /// The result of each domain of that block, in domain order.
    pub domains: Vec<DomainCommitment>,
}

impl<Block: BlockTrait> CommitRecord<Block> {
    /// The number of domains that committed.
    pub fn committed_domains(&self) -> usize {
        self.domains
            .iter()
            .filter(|domain| domain.committed)
            .count()
    }

    /// Report whether every domain of the block committed.
    pub fn whole_block_committed(&self) -> bool {
        !self.domains.is_empty() && self.domains.iter().all(|domain| domain.committed)
    }
}

/// The record of what the Commit Engine made irreversible.
///
/// The ledger is the one place the node reads to answer whether a block is
/// irreversible.
#[derive(Clone, Debug)]
pub struct CommitLedger<Block: BlockTrait> {
    depth: usize,
    records: BTreeMap<NumberFor<Block>, CommitRecord<Block>>,
    highest: Option<NumberFor<Block>>,
}

impl<Block: BlockTrait> CommitLedger<Block> {
    /// Start a ledger holding the default depth.
    pub fn new() -> Self {
        CommitLedger::with_depth(DEFAULT_LEDGER_DEPTH)
    }

    /// Start a ledger holding the given depth.
    ///
    /// A depth of zero rises to one, so the ledger always holds the last
    /// commitment.
    pub fn with_depth(depth: usize) -> Self {
        CommitLedger {
            depth: depth.max(1),
            records: BTreeMap::new(),
            highest: None,
        }
    }

    /// The number of commitments the ledger holds.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Report whether the ledger holds nothing.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// The depth this ledger holds.
    pub fn depth(&self) -> usize {
        self.depth
    }

    /// Record one commitment.
    ///
    /// A block already recorded leaves the ledger unchanged. The oldest entry
    /// leaves once the ledger stands at its depth.
    pub fn record(&mut self, record: CommitRecord<Block>) -> bool {
        let number = record.block_number;
        if self.records.contains_key(&number) {
            return false;
        }
        self.records.insert(number, record);
        while self.records.len() > self.depth {
            let oldest = match self.records.keys().next().copied() {
                Some(key) => key,
                None => break,
            };
            self.records.remove(&oldest);
        }
        self.highest = match self.highest {
            Some(held) if held >= number => Some(held),
            _ => Some(number),
        };
        true
    }

    /// The highest block the Commit Engine made irreversible.
    pub fn highest_committed(&self) -> Option<NumberFor<Block>> {
        self.highest
    }

    /// Report whether one block sits at or below the highest commitment.
    pub fn committed(&self, block_number: NumberFor<Block>) -> bool {
        match self.highest {
            Some(highest) => block_number <= highest,
            None => false,
        }
    }

    /// The record of one block, where the ledger still holds it.
    pub fn record_of(&self, block_number: NumberFor<Block>) -> Option<&CommitRecord<Block>> {
        self.records.get(&block_number)
    }

    /// The records the ledger holds, from the oldest to the newest.
    pub fn records(&self) -> Vec<&CommitRecord<Block>> {
        self.records.values().collect()
    }
}

impl<Block: BlockTrait> Default for CommitLedger<Block> {
    fn default() -> Self {
        CommitLedger::new()
    }
}

/// The message the node broadcasts when a block becomes irreversible.
#[derive(Debug, Clone)]
pub struct FinalityNotification<Block: BlockTrait> {
    /// The number of the committed block.
    pub block_number: NumberFor<Block>,
    /// The hash of the committed block.
    pub block_hash: Block::Hash,
    /// The round that committed the block.
    pub round_id: u32,
}

/// The broadcaster of commitment messages.
///
/// Node components subscribe once and receive every commitment after that.
pub struct FinalityNotifier<Block: BlockTrait> {
    subscribers: Arc<tokio::sync::RwLock<Vec<mpsc::UnboundedSender<FinalityNotification<Block>>>>>,
}

impl<Block: BlockTrait> FinalityNotifier<Block> {
    /// Build a broadcaster.
    pub fn new() -> Self {
        info!("Commit Engine: the commitment broadcaster is ready");
        Self {
            subscribers: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Subscribe to commitment messages.
    pub async fn subscribe(&self) -> mpsc::UnboundedReceiver<FinalityNotification<Block>> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(sender);
        debug!(
            "Commit Engine: one subscriber joined, {} in total",
            subscribers.len()
        );
        receiver
    }

    /// Broadcast one commitment to every subscriber.
    pub async fn notify(&self, notification: FinalityNotification<Block>) {
        let subscribers = self.subscribers.read().await;
        debug!(
            "Commit Engine: block #{} committed, telling {} subscribers",
            notification.block_number,
            subscribers.len()
        );
        for subscriber in subscribers.iter() {
            if let Err(reason) = subscriber.send(notification.clone()) {
                warn!(
                    "Commit Engine: one subscriber stopped listening: {:?}",
                    reason
                );
            }
        }
    }
}

impl<Block: BlockTrait> Default for FinalityNotifier<Block> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::TransactionClass;
    use crate::types::BlockT;

    fn commitment(domain: usize, committed: bool) -> DomainCommitment {
        DomainCommitment {
            domain,
            highest_class: TransactionClass::Routine,
            required_percent: 51,
            signed_weight: if committed { 60 } else { 10 },
            total_weight: 100,
            committed,
        }
    }

    fn record(number: u32, committed: bool) -> CommitRecord<BlockT> {
        CommitRecord {
            slot: Slot(number as u64),
            block_number: number,
            block_hash: Default::default(),
            signer_count: 6,
            signed_weight: 60,
            total_weight: 100,
            domains: vec![commitment(0, committed), commitment(1, true)],
        }
    }

    #[test]
    fn the_ledger_records_one_commitment_once() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::new();
        assert!(ledger.is_empty());
        assert!(ledger.record(record(4, true)));
        assert!(!ledger.record(record(4, true)));
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.highest_committed(), Some(4));
    }

    #[test]
    fn the_ledger_answers_whether_a_block_is_irreversible() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::new();
        assert!(!ledger.committed(1));
        ledger.record(record(7, true));
        assert!(ledger.committed(7));
        assert!(ledger.committed(3));
        assert!(!ledger.committed(8));
    }

    #[test]
    fn the_highest_commitment_never_falls() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::new();
        ledger.record(record(9, true));
        ledger.record(record(5, true));
        assert_eq!(ledger.highest_committed(), Some(9));
    }

    #[test]
    fn the_ledger_holds_a_bounded_window() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::with_depth(16);
        for number in 0..500u32 {
            ledger.record(record(number, true));
            assert!(ledger.len() <= 16);
        }
        assert_eq!(ledger.len(), 16);
        assert_eq!(ledger.depth(), 16);
        assert_eq!(ledger.highest_committed(), Some(499));
        assert!(ledger.record_of(499).is_some());
        assert!(ledger.record_of(1).is_none());
    }

    #[test]
    fn a_depth_of_zero_rises_to_one() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::with_depth(0);
        assert_eq!(ledger.depth(), 1);
        ledger.record(record(1, true));
        ledger.record(record(2, true));
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn a_record_reports_the_domains_that_committed() {
        let full = record(3, true);
        assert_eq!(full.committed_domains(), 2);
        assert!(full.whole_block_committed());
        let partial = record(4, false);
        assert_eq!(partial.committed_domains(), 1);
        assert!(!partial.whole_block_committed());
    }

    #[test]
    fn the_records_read_back_in_block_order() {
        let mut ledger: CommitLedger<BlockT> = CommitLedger::with_depth(8);
        for number in [5u32, 1, 3] {
            ledger.record(record(number, true));
        }
        let numbers: Vec<u32> = ledger
            .records()
            .iter()
            .map(|record| record.block_number)
            .collect();
        assert_eq!(numbers, vec![1, 3, 5]);
    }

    #[test]
    fn the_default_ledger_holds_the_stated_depth() {
        let ledger: CommitLedger<BlockT> = CommitLedger::default();
        assert_eq!(ledger.depth(), DEFAULT_LEDGER_DEPTH);
    }

    #[tokio::test]
    async fn every_subscriber_hears_each_commitment() {
        let notifier: FinalityNotifier<BlockT> = FinalityNotifier::new();
        let mut first = notifier.subscribe().await;
        let mut second = notifier.subscribe().await;
        notifier
            .notify(FinalityNotification {
                block_number: 12,
                block_hash: Default::default(),
                round_id: 3,
            })
            .await;
        let heard = first.recv().await.expect("the first subscriber hears it");
        assert_eq!(heard.block_number, 12);
        assert_eq!(heard.round_id, 3);
        assert_eq!(
            second
                .recv()
                .await
                .expect("the second subscriber hears it")
                .block_number,
            12
        );
    }
}
