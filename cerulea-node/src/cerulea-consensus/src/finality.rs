//! Finality tracking implementation
//! 
//! This module handles block finality and confirmation tracking.

use sp_runtime::traits::{Block as BlockTrait, NumberFor};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use log::{debug, info, warn};
use crate::error::{ConsensusError, ConsensusResult};

/// Tracks block finality and confirmations
pub struct FinalityEngine<B: BlockTrait> {
    confirmations: HashMap<B::Hash, u32>,
    finality_threshold: u32,
}

impl<B: BlockTrait> FinalityEngine<B> {
    /// Create a new finality engine with the specified threshold
    pub fn new(finality_threshold: u32) -> Self {
        Self {
            confirmations: HashMap::new(),
            finality_threshold,
        }
    }

    /// Add a new block to track
    pub fn add_block(&mut self, block_hash: B::Hash) {
        self.confirmations.insert(block_hash, 0);
    }

    /// Update block confirmations and check for finality
    pub fn update_confirmations(&mut self, block_hash: B::Hash) -> ConsensusResult<bool> {
        if let Some(confirmations) = self.confirmations.get_mut(&block_hash) {
            *confirmations += 1;
            Ok(*confirmations >= self.finality_threshold)
        } else {
            Err(ConsensusError::Finality("Block not found".into()))
        }
    }

    /// Check if a block is finalized
    pub fn is_finalized(&self, block_hash: &B::Hash) -> bool {
        self.confirmations
            .get(block_hash)
            .map_or(false, |&c| c >= self.finality_threshold)
    }

    /// Get the number of confirmations for a block
    pub fn get_confirmations(&self, block_hash: &B::Hash) -> Option<u32> {
        self.confirmations.get(block_hash).copied()
    }

    /// Remove old blocks that are no longer needed
    pub fn prune_old_blocks(&mut self, finalized_blocks: &[B::Hash]) {
        for block_hash in finalized_blocks {
            self.confirmations.remove(block_hash);
        }
    }

    /// Set the finality threshold
    pub fn set_finality_threshold(&mut self, threshold: u32) {
        self.finality_threshold = threshold;
    }
}

/// Finality notification sent when a block is finalized
#[derive(Debug, Clone)]
pub struct FinalityNotification<Block: BlockTrait> {
    /// The finalized block number
    pub block_number: NumberFor<Block>,
    /// The finalized block hash
    pub block_hash: Block::Hash,
    /// The round ID that finalized this block
    pub round_id: u32,
}

/// Finality Notifier
///
/// Broadcasts finality events and synchronizes finalized head across node components.
pub struct FinalityNotifier<Block: BlockTrait> {
    subscribers: Arc<tokio::sync::RwLock<Vec<mpsc::UnboundedSender<FinalityNotification<Block>>>>>,
}

impl<Block: BlockTrait> FinalityNotifier<Block> {
    /// Creates a new finality notifier
    pub fn new() -> Self {
        info!("DVF Finality Notifier: Initializing");
        Self {
            subscribers: Arc::new(tokio::sync::RwLock::new(Vec::new())),
        }
    }

    /// Subscribes to finality notifications
    ///
    /// Returns a receiver that will receive finality notifications.
    pub async fn subscribe(&self) -> mpsc::UnboundedReceiver<FinalityNotification<Block>> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut subscribers = self.subscribers.write().await;
        subscribers.push(tx);
        debug!("DVF Finality Notifier: New subscriber added (total: {})", subscribers.len());
        rx
    }

    /// Notifies all subscribers of finalization
    ///
    /// Broadcasts the finality notification to all registered subscribers.
    pub async fn notify(&self, notification: FinalityNotification<Block>) {
        let subscribers = self.subscribers.read().await;
        debug!(
            "DVF Finality Notifier: Broadcasting finality notification for block #{} to {} subscribers",
            notification.block_number, subscribers.len()
        );

        for subscriber in subscribers.iter() {
            if let Err(e) = subscriber.send(notification.clone()) {
                warn!("DVF Finality Notifier: Failed to send notification to subscriber: {:?}", e);
            }
        }
    }
}

impl<Block: BlockTrait> Default for FinalityNotifier<Block> {
    fn default() -> Self {
        Self::new()
    }
}
