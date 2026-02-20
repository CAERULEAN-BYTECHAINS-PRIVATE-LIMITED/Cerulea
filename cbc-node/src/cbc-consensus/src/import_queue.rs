//! Block import queue implementation
//!
//! This module provides block import validation and processing logic
//! that works in conjunction with the DCF runtime pallet.

use crate::types::ValidatorMetrics;
use crate::metrics::ConsensusMetrics;
use std::sync::Arc;
use log::{info, error, debug, warn};
use sp_runtime::traits::{Block as BlockTrait, SaturatedConversion, Header as HeaderT, NumberFor};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sc_consensus::{BlockImport, BlockImportParams, BlockCheckParams, ImportResult, Verifier};
use sp_consensus::Error;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use cbc_runtime::AccountId;
use sp_runtime::generic::DigestItem;
use codec::Decode;
use sc_client_api::Backend;

/// Block import queue for DCF consensus
pub struct DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    client: Arc<C>,
    metrics: ValidatorMetrics,
    consensus_metrics: Option<ConsensusMetrics>,
    _phantom: std::marker::PhantomData<(B, BE)>,
}

impl<B, C, BE> DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    /// Create a new import queue
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Create a new import queue with consensus metrics
    pub fn new_with_metrics(client: Arc<C>, consensus_metrics: ConsensusMetrics) -> Self {
        Self {
            client,
            metrics: ValidatorMetrics::default(),
            consensus_metrics: Some(consensus_metrics),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get current import queue metrics
    pub fn get_metrics(&self) -> &ValidatorMetrics {
        &self.metrics
    }

    /// Reset metrics (useful for testing or periodic resets)
    pub fn reset_metrics(&mut self) {
        self.metrics = ValidatorMetrics::default();
        info!("DCF ImportQueue: Metrics reset");
    }

    /// Extract block author from block header digest
    fn extract_block_author(&self, header: &B::Header) -> Result<AccountId, Error> {
        // Look for the author in the digest items
        for digest_item in header.digest().logs() {
            match digest_item {
                DigestItem::PreRuntime(engine_id, data) => {
                    // For CBC consensus, we expect the author to be encoded in the pre-runtime digest
                    if *engine_id == crate::CBC_ENGINE_ID {
                        // Try to decode the author from the digest data
                        if let Ok(author) = AccountId::decode(&mut &data[..]) {
                            return Ok(author);
                        }
                    }
                }
                DigestItem::Consensus(engine_id, data) => {
                    // Alternative: author might be in consensus digest
                    if *engine_id == crate::CBC_ENGINE_ID {
                        if let Ok(author) = AccountId::decode(&mut &data[..]) {
                            return Ok(author);
                        }
                    }
                }
                _ => {}
            }
        }
        
        // If we can't extract the author from digest, this is an error
        Err(sp_consensus::Error::ClientImport(
            "Failed to extract block author from header digest".to_string()
        ))
    }
}

#[async_trait::async_trait]
impl<B, C, BE> BlockImport<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    type Error = Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = block.number.saturated_into::<u32>();
        
        debug!("DCF ImportQueue: Checking block #{}", block_number);
        
        // Basic validation - check if we have active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e)))?;
        
        if active_validators.is_empty() {
            error!("DCF ImportQueue: Block check failed for block #{}: No active validators", block_number);
            return Ok(ImportResult::imported(false));
        }
        
        info!("DCF ImportQueue: Block #{} check passed", block_number);
        Ok(ImportResult::imported(true))
    }

    async fn import_block(
        &self,
        mut block: BlockImportParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();
        
        debug!("DCF ImportQueue: Importing block #{} ({:?})", block_number, block_hash);
        
        // Extract block author from header digest
        let author = match self.extract_block_author(&block.header) {
            Ok(author) => author,
            Err(e) => {
                error!("DCF ImportQueue: Failed to extract block author for block #{}: {:?}", block_number, e);
                return Err(e);
            }
        };
        
        // Validate that we have active validators
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e)))?;
        
        if active_validators.is_empty() {
            error!("DCF ImportQueue: Block import failed for block #{}: No active validators", block_number);
            return Ok(ImportResult::imported(false));
        }

        // Validate block author using runtime API
        let expected_author = api.get_expected_author(best_hash, block_number)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get expected author: {:?}", e)))?;
        
        if Some(author.clone()) != expected_author {
            // Task 8 requirement: Record author mismatch metric
            if let Some(ref metrics) = self.consensus_metrics {
                metrics.record_author_mismatch();
            }
            
            // Report author mismatch via runtime API for event emission
            if let Err(e) = api.report_author_mismatch(best_hash, block_number, expected_author.clone(), author.clone()) {
                warn!("DCF ImportQueue: Failed to report author mismatch: {:?}", e);
            }
            
            // Reject the block with author mismatch error
            let error_msg = format!(
                "Block author mismatch: expected {:?}, got {:?} for block {}",
                expected_author, author, block_number
            );
            error!("DCF ImportQueue: {}", error_msg);
            
            return Err(sp_consensus::Error::ClientImport(error_msg));
        }
        
        // Check if this block should be finalized according to DCF
        let last_finalized = api.get_last_finalized_block(best_hash)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get last finalized block: {:?}", e)))?;
        
        // Mark block as finalized if DCF has finalized it
        block.finalized = block_number <= last_finalized;
        
        if block.finalized {
            debug!("DCF ImportQueue: Block #{} marked as finalized (DCF finalized up to #{})", 
                   block_number, last_finalized);
        }
        
        // Set proper import parameters
        block.origin = sp_consensus::BlockOrigin::Own;
        block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);
        
        // For now, just return imported - the actual import will be handled by the client
        // This is a simplified implementation that validates the block and accepts it
        let import_result = ImportResult::imported(true);
        
        match &import_result {
            ImportResult::Imported(_) => {
                info!("DCF ImportQueue: Successfully imported block #{}", block_number);
            }
            ImportResult::AlreadyInChain => {
                debug!("DCF ImportQueue: Block #{} already in chain", block_number);
            }
            ImportResult::KnownBad => {
                warn!("DCF ImportQueue: Block #{} is known bad", block_number);
            }
            ImportResult::UnknownParent => {
                warn!("DCF ImportQueue: Block #{} has unknown parent", block_number);
            }
            ImportResult::MissingState => {
                warn!("DCF ImportQueue: Block #{} is missing state", block_number);
            }
        }
        
        // Update consensus metrics if available
        if let Some(ref _metrics) = self.consensus_metrics {
            // Update metrics manually since we can't use the generic update_from_runtime here
            // due to trait bound constraints. We'll update the metrics in the service layer instead.
            debug!("DCF ImportQueue: Consensus metrics available but update deferred to service layer");
        }

        // Log periodic statistics
        if block_number % 10u32 == 0 {
            let current_epoch = api.get_current_epoch(best_hash).unwrap_or(0);
            info!("DCF ImportQueue: Block #{} processed - {} active validators, epoch {}", 
                  block_number, active_validators.len(), current_epoch);
        }
        
        info!("DCF ImportQueue: Block #{} ({:?}) validated and accepted", block_number, block_hash);
        
        // Return imported with default aux data
        let aux = sc_consensus::ImportedAux {
            header_only: false,
            clear_justification_requests: false,
            needs_justification: false,
            bad_justification: false,
            is_new_best: true,
        };
        
        Ok(ImportResult::Imported(aux))
    }
}

impl<B, C, BE> DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    /// Handle justifications for finality (stub implementation as requested)
    pub fn import_justifications(
        &self,
        _who: String, // Simplified origin type
        hash: B::Hash,
        number: sp_runtime::traits::NumberFor<B>,
        _justifications: Vec<u8>, // Simplified justifications type
    ) -> Result<(), Error> {
        let block_number = number.saturated_into::<u32>();
        
        debug!("DCF ImportQueue: Received justifications for block #{} ({:?})", 
               block_number, hash);
        
        // Get the current finalized block from the DCF pallet
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        match api.get_last_finalized_block(best_hash) {
            Ok(last_finalized) => {
                // Check if this justification is for a block that should be finalized
                if block_number >= last_finalized {
                    info!("DCF ImportQueue: Justification received for block #{} (last finalized: {})", 
                          block_number, last_finalized);
                    
                    // Check if the block is already considered finalized by the DCF pallet
                    match api.is_block_finalized(best_hash, block_number) {
                        Ok(is_finalized) => {
                            if is_finalized {
                                info!("DCF ImportQueue: Block #{} is already finalized according to DCF pallet", 
                                      block_number);
                            } else {
                                info!("DCF ImportQueue: Block #{} justification received but not yet finalized by DCF pallet", 
                                      block_number);
                                
                                // Here we could potentially update the finalized block
                                // but for now we just log it for testing purpose
                            }
                        }
                        Err(e) => {
                            error!("DCF ImportQueue: Failed to check finalization status for block #{}: {:?}", 
                                   block_number, e);
                        }
                    }
                } else {
                    debug!("DCF ImportQueue: Justification for block #{} is older than last finalized block {}", 
                           block_number, last_finalized);
                }
            }
            Err(e) => {
                error!("DCF ImportQueue: Failed to get last finalized block: {:?}", e);
            }
        }
        
        // Return Ok() as requested - no actual justification processing
        Ok(())
    }
}

#[async_trait::async_trait]
impl<B, C, BE> Verifier<B> for DcfImportQueue<B, C, BE>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    BE: Backend<B>,
{
    async fn verify(
        &self,
        block: BlockImportParams<B>,
    ) -> Result<BlockImportParams<B>, String> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        
        debug!("DCF Verifier: Verifying block #{}", block_number);
        
        // Perform the same validation as in check_block
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| format!("Failed to get active validators: {:?}", e))?;
        
        if active_validators.is_empty() {
            return Err(format!("Block verification failed for block #{}: No active validators", block_number));
        }
        
        // Extract and validate block author
        let author = self.extract_block_author(&block.header)
            .map_err(|e| format!("Failed to extract block author: {:?}", e))?;
        
        // Validate block author using runtime API
        let expected_author = api.get_expected_author(best_hash, block_number)
            .map_err(|e| format!("Failed to get expected author: {:?}", e))?;
        
        if Some(author.clone()) != expected_author {
            // Record author mismatch metric
            if let Some(ref metrics) = self.consensus_metrics {
                metrics.record_author_mismatch();
            }
            
            return Err(format!(
                "Block author mismatch: expected {:?}, got {:?} for block {}",
                expected_author, author, block_number
            ));
        }
        
        info!("DCF Verifier: Block #{} verification passed", block_number);
        
        // Ensure fork choice is set before passing to inner import
        let mut block = block;
        if block.fork_choice.is_none() {
            block.fork_choice = Some(sc_consensus::ForkChoiceStrategy::LongestChain);
        }
        
        // Return the verified block
        Ok(block)
    }
}