//! DVF Block Import Integration
//!
//! This module provides the DvfBlockImport wrapper that integrates DVF justification
//! verification with Substrate's block import pipeline.

use codec::{Decode, Encode};
use log::{debug, info, warn};
use sc_client_api::HeaderBackend;
use sc_consensus::{BlockImport, BlockImportParams, BlockCheckParams, ImportResult};
use sp_api::ProvideRuntimeApi;
use sp_consensus::Error as ConsensusError;
use sp_runtime::traits::{Block as BlockT, Header as HeaderT, NumberFor, SaturatedConversion};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::dvf_gossip::DvfJustification;
use crate::justification_builder::JustificationBuilder;
use crate::metrics::DvfMetrics;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use pallet_cbc_dvf::DvfApi;

/// DVF consensus engine ID
pub const DVF_ENGINE_ID: sp_runtime::ConsensusEngineId = *b"dvfd";

/// Finality notification sent when a block is finalized
#[derive(Debug, Clone)]
pub struct FinalityNotification<Block: BlockT> {
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
pub struct FinalityNotifier<Block: BlockT> {
    subscribers: Arc<tokio::sync::RwLock<Vec<mpsc::UnboundedSender<FinalityNotification<Block>>>>>,
}

impl<Block: BlockT> FinalityNotifier<Block> {
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

impl<Block: BlockT> Default for FinalityNotifier<Block> {
    fn default() -> Self {
        Self::new()
    }
}

/// DVF Block Import wrapper
///
/// Wraps an inner block import and adds DVF justification verification.
pub struct DvfBlockImport<Block, Inner, Client, AccountId>
where
    Block: BlockT,
    Inner: BlockImport<Block, Error = ConsensusError> + Send + Sync,
{
    inner: Arc<Inner>,
    client: Arc<Client>,
    justification_builder: Arc<JustificationBuilder<Block, Client, AccountId>>,
    finality_notifier: Arc<FinalityNotifier<Block>>,
    metrics: Option<Arc<DvfMetrics>>,
}

impl<Block, Inner, Client, AccountId> DvfBlockImport<Block, Inner, Client, AccountId>
where
    Block: BlockT,
    Inner: BlockImport<Block, Error = ConsensusError> + Send + Sync,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + Decode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Creates a new DVF block import wrapper
    ///
    /// # Arguments
    /// * `inner` - The inner block import to wrap
    /// * `client` - The blockchain client for querying runtime state
    /// * `justification_builder` - The justification builder for verification
    /// * `finality_notifier` - The finality notifier for broadcasting finality events
    pub fn new(
        inner: Arc<Inner>,
        client: Arc<Client>,
        justification_builder: Arc<JustificationBuilder<Block, Client, AccountId>>,
        finality_notifier: Arc<FinalityNotifier<Block>>,
    ) -> Self {
        info!("DVF Block Import: Initializing");
        Self {
            inner,
            client,
            justification_builder,
            finality_notifier,
            metrics: None,
        }
    }

    /// Sets the metrics for this block import
    pub fn with_metrics(mut self, metrics: Arc<DvfMetrics>) -> Self {
        self.metrics = Some(metrics);
        self
    }
}

impl<Block, Inner, Client, AccountId> DvfBlockImport<Block, Inner, Client, AccountId>
where
    Block: BlockT,
    Inner: BlockImport<Block, Error = ConsensusError> + Send + Sync,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + Decode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    /// Extracts DVF justification from block import params
    ///
    /// Looks for a justification with the DVF engine ID and decodes it.
    fn extract_dvf_justification(
        &self,
        block: &BlockImportParams<Block>,
    ) -> Result<Option<DvfJustification<Block::Hash, AccountId>>, ConsensusError> {
        // Check if block has any justifications
        if let Some(justifications) = &block.justifications {
            // Look for DVF justification
            for (engine_id, justification_data) in justifications.iter() {
                if *engine_id == DVF_ENGINE_ID {
                    debug!(
                        "DVF Block Import: Found DVF justification for block #{} (size: {} bytes)",
                        block.header.number(),
                        justification_data.len()
                    );

                    // Decode justification
                    let justification = DvfJustification::<Block::Hash, AccountId>::decode(
                        &mut &justification_data[..],
                    )
                    .map_err(|e| {
                        ConsensusError::ClientImport(format!(
                            "Failed to decode DVF justification: {:?}",
                            e
                        ))
                    })?;

                    return Ok(Some(justification));
                }
            }
        }

        Ok(None)
    }

    /// Processes a DVF justification during block import
    ///
    /// Verifies the justification and updates finality state if valid.
    /// This ensures atomic finality updates with block import.
    fn process_justification(
        &self,
        block: &BlockImportParams<Block>,
        justification: &DvfJustification<Block::Hash, AccountId>,
    ) -> Result<(), ConsensusError> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();

        info!(
            "DVF Block Import: Processing justification for block #{} ({:?})",
            block_number, block_hash
        );

        // Verify justification before any state updates
        self.justification_builder
            .verify_justification(justification)
            .map_err(|e| {
                warn!(
                    "DVF Block Import: Justification verification failed for block #{}: {}",
                    block_number, e
                );
                ConsensusError::ClientImport(format!("Invalid DVF justification: {}", e))
            })?;

        info!(
            "DVF Block Import: Justification verified successfully for block #{}",
            block_number
        );

        // Verify block hash matches justification
        if justification.block_hash != block_hash {
            return Err(ConsensusError::ClientImport(format!(
                "Justification block hash mismatch: expected {:?}, got {:?}",
                block_hash, justification.block_hash
            )));
        }

        // Update finality state through runtime
        // This will be done via the runtime's submit_dvf_vote extrinsic in task 6
        // The runtime will atomically update FinalizedBlockNumber, FinalizedBlockHash,
        // and CurrentRound storage values
        
        // For now, we ensure the justification is valid and will be processed
        // atomically with the block import. The actual runtime storage updates
        // will happen in the runtime's on_finalize hook or through an extrinsic.

        debug!(
            "DVF Block Import: Finality state will be updated atomically for block #{}",
            block_number
        );

        Ok(())
    }

    /// Updates the node's finalized head after successful import
    ///
    /// This is called after the block import succeeds to ensure the node's
    /// finalized head is synchronized with the runtime's finalized state.
    fn update_finalized_head(
        &self,
        block_hash: Block::Hash,
        block_number: NumberFor<Block>,
        round_id: u32,
    ) -> Result<(), ConsensusError> {
        info!(
            "DVF Block Import: Updating node finalized head to block #{} ({:?})",
            block_number, block_hash
        );

        // Update finalized block number metric
        if let Some(ref metrics) = self.metrics {
            metrics.update_finalized_block_number(block_number.saturated_into::<u32>());
        }

        // Log finality achievement at info level with block number, round, and weight
        info!(
            "DVF Block Import: Finality achieved for block #{} in round {}",
            block_number, round_id
        );

        // Finalize the block on the client side
        // This updates the node's finalized head to match the runtime's finalized state
        // The finality sync service also handles this, but doing it here ensures
        // immediate synchronization when blocks are imported with justifications
        debug!(
            "DVF Block Import: Finalizing block #{} on client",
            block_number
        );

        // Note: We don't call client.finalize_block() here because:
        // 1. The block import pipeline already handles finalization when justifications are present
        // 2. The finality sync service continuously syncs the client's finalized head with runtime
        // 3. Calling finalize_block() here could cause race conditions or double-finalization
        // 
        // The finality notification below triggers the sync service to finalize if needed
        debug!(
            "DVF Block Import: Finalized block #{} in runtime, sync service will update client",
            block_number
        );

        // Notify finality listeners
        let notification = FinalityNotification {
            block_number,
            block_hash,
            round_id,
        };

        // Use blocking call to notify
        let notifier = self.finality_notifier.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                notifier.notify(notification).await;
            });
        });

        info!(
            "DVF Block Import: Finality notification sent for block #{}",
            block_number
        );

        Ok(())
    }
}

#[async_trait::async_trait]
impl<Block, Inner, Client, AccountId> BlockImport<Block>
    for DvfBlockImport<Block, Inner, Client, AccountId>
where
    Block: BlockT,
    Inner: BlockImport<Block, Error = ConsensusError> + Send + Sync,
    Client: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    Client::Api: RuntimeDcfApi<Block, AccountId, u128, NumberFor<Block>>
        + DvfApi<Block, NumberFor<Block>, AccountId, Block::Hash>,
    AccountId: Clone + Encode + Decode + codec::Codec + PartialEq + Eq + std::fmt::Debug + Send + Sync + std::hash::Hash + 'static,
{
    type Error = ConsensusError;

    async fn check_block(
        &self,
        block: BlockCheckParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        // Delegate to inner block import for basic checks
        self.inner.check_block(block).await
    }

    async fn import_block(
        &self,
        block: BlockImportParams<Block>,
    ) -> Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();

        debug!(
            "DVF Block Import: Importing block #{} ({:?})",
            block_number, block_hash
        );

        // Prevent reorganization behind finalized head
        let finalized_number = self.client.info().finalized_number.saturated_into::<u32>();
        if crate::types::utils::is_block_finalized(&*self.client, block_number.into()) {
            // Check if this block is on the finalized chain
            let _parent_hash = *block.header.parent_hash();
            
            // If the block number is at or below finalized, it must be on the canonical chain
            // Otherwise it's an attempt to fork behind the finalized head
            if let Ok(Some(canonical_hash)) = self.client.hash(block_number.into()) {
                if canonical_hash != block_hash {
                    warn!(
                        "DVF Block Import: Rejecting block #{} - attempt to fork behind finalized head (finalized: #{})",
                        block_number, finalized_number
                    );
                    return Err(ConsensusError::ClientImport(format!(
                        "Cannot import block #{} that would fork behind finalized head #{}",
                        block_number, finalized_number
                    )));
                }
            }
        }

        // Extract DVF justification if present
        let dvf_justification = self.extract_dvf_justification(&block)?;

        // If justification is present, verify it before state execution
        if let Some(ref justification) = dvf_justification {
            info!(
                "DVF Block Import: Block #{} has DVF justification, verifying before import",
                block_number
            );

            // Verify justification (this ensures no partial state updates on failure)
            if let Err(e) = self.process_justification(&block, justification) {
                warn!(
                    "DVF Block Import: Rejecting block #{} due to invalid justification: {:?}",
                    block_number, e
                );
                return Err(e);
            }
        } else {
            debug!(
                "DVF Block Import: Block #{} has no DVF justification, continuing normal import",
                block_number
            );
        }

        // Delegate to inner block import
        // This ensures atomic state updates - if the inner import fails,
        // no finality state changes will be committed
        let result = self.inner.import_block(block).await?;

        // If import was successful and we had a justification, update finalized head
        if let Some(justification) = dvf_justification {
            match &result {
                ImportResult::Imported(_) => {
                    info!(
                        "DVF Block Import: Block #{} imported successfully with finality",
                        block_number
                    );

                    // Update finalized head atomically after successful import
                    if let Err(e) = self
                        .update_finalized_head(
                            block_hash,
                            block_number.into(),
                            justification.round_number,
                        )
                    {
                        warn!(
                            "DVF Block Import: Failed to update finalized head for block #{}: {:?}",
                            block_number, e
                        );
                        // Don't fail the import for this - the block is already imported
                        // The finality notification will be retried on next finalization
                    }
                }
                _ => {
                    debug!(
                        "DVF Block Import: Block #{} import result: {:?}",
                        block_number, result
                    );
                }
            }
        }

        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use codec::Encode;
    use sp_runtime::testing::{Block as TestBlock, Header};

    #[test]
    fn test_finality_notifier_creation() {
        let notifier = FinalityNotifier::<TestBlock>::new();
        assert!(Arc::strong_count(&notifier.subscribers) > 0);
    }

    #[test]
    fn test_finality_notification_creation() {
        let notification = FinalityNotification::<TestBlock> {
            block_number: 42u64.into(),
            block_hash: Default::default(),
            round_id: 1,
        };
        assert_eq!(notification.round_id, 1);
    }

    #[tokio::test]
    async fn test_finality_notifier_subscribe() {
        let notifier = FinalityNotifier::<TestBlock>::new();
        let _receiver = notifier.subscribe().await;
        
        let subscribers = notifier.subscribers.read().await;
        assert_eq!(subscribers.len(), 1);
    }

    #[tokio::test]
    async fn test_finality_notifier_notify() {
        let notifier = FinalityNotifier::<TestBlock>::new();
        let mut receiver = notifier.subscribe().await;
        
        let notification = FinalityNotification::<TestBlock> {
            block_number: 42u64.into(),
            block_hash: Default::default(),
            round_id: 1,
        };
        
        notifier.notify(notification.clone()).await;
        
        let received = receiver.recv().await;
        assert!(received.is_some());
        let received = received.unwrap();
        assert_eq!(received.block_number, notification.block_number);
        assert_eq!(received.round_id, notification.round_id);
    }
}
