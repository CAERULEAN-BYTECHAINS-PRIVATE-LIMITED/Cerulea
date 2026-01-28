//! Block proposer factory implementation
//!
//! This module creates real blocks with transactions using the DCF runtime API for author selection.

use crate::error::{ConsensusError, ConsensusResult};
use crate::inherent_providers::CbcInherentDataProviders;
use sp_api::{ProvideRuntimeApi, Core};
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::NumberFor;
use sp_core::sr25519::Public;
use cbc_runtime::AccountId;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use std::sync::Arc;
use sp_runtime::traits::{Block as BlockTrait, Header as HeaderTrait, Zero, SaturatedConversion};
use std::time::{Duration, Instant};
use std::marker::PhantomData;
use sc_transaction_pool_api::{TransactionPool, InPoolTransaction};
use sp_block_builder::BlockBuilder as BlockBuilderApi;
use log::{debug, warn, error};

/// Factory for creating real blocks with transactions using DCF runtime API for author selection
pub struct ProposerFactory<B: BlockTrait, C, TP>
where
    B: sp_runtime::traits::Block,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + BlockBuilderApi<B>,
    TP: TransactionPool<Block = B> + 'static,
{
    client: Arc<C>,
    transaction_pool: Arc<TP>,
    inherent_providers: CbcInherentDataProviders,
    min_block_time: Duration,
    last_block_time: Option<Instant>,
    max_transactions_per_block: usize,
    _phantom: PhantomData<B>,
}

impl<B: BlockTrait, C, TP> ProposerFactory<B, C, TP>
where
    B: sp_runtime::traits::Block,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>> + BlockBuilderApi<B>,
    TP: TransactionPool<Block = B> + 'static,
{
    /// Create a new proposer factory with the specified parameters
    pub fn new(client: Arc<C>, transaction_pool: Arc<TP>, min_block_time: Duration, max_transactions_per_block: usize) -> Self {
        Self {
            client,
            transaction_pool,
            inherent_providers: CbcInherentDataProviders::new(),
            min_block_time,
            last_block_time: None,
            max_transactions_per_block,
            _phantom: PhantomData,
        }
    }

    /// Create a new block with transactions from the pool using the expected author from DCF runtime API
    /// This method now delegates to the comprehensive block creation method
    pub async fn create_block_with_transactions_and_digest(
        &mut self, 
        parent_hash: B::Hash, 
        slot: u64, 
        author_digest: Option<sp_runtime::generic::DigestItem>
    ) -> ConsensusResult<(B, Public)> {
        debug!("ProposerFactory: create_block_with_transactions_and_digest called for slot {}", slot);
        
        // Use the comprehensive block creation method
        self.create_complete_block(parent_hash, slot, author_digest, None).await
    }

    /// Create a new block with transactions from the pool using the expected author from DCF runtime API
    /// This is the main entry point for block creation with proper state root calculation
    pub async fn create_block_with_transactions(&mut self, parent_hash: B::Hash, slot: u64) -> ConsensusResult<(B, Public)> {
        self.create_block_with_transactions_and_digest(parent_hash, slot, None).await
    }
    
    /// Create a complete block with proper state root calculation and comprehensive error handling
    /// This method implements the full Substrate block building pipeline
    pub async fn create_complete_block(
        &mut self,
        parent_hash: B::Hash,
        _slot: u64,
        author_digest: Option<sp_runtime::generic::DigestItem>,
        force_author: Option<Public>,
    ) -> ConsensusResult<(B, Public)> {
        log::trace!("ProposerFactory: Starting complete block creation for slot {}", _slot);
        
        // Check timing constraints
        if let Some(last_time) = self.last_block_time {
            if last_time.elapsed() < self.min_block_time {
                return Err(ConsensusError::Proposer(
                    format!("Not enough time since last block: {:?} < {:?}", 
                           last_time.elapsed(), self.min_block_time)
                ));
            }
        }

        // Get parent header and validate it
        let parent_header = self.client.header(parent_hash)
            .map_err(|e| ConsensusError::Proposer(format!("Failed to get parent header: {:?}", e)))?
            .ok_or_else(|| ConsensusError::Proposer("Parent header not found".into()))?;
        
        let block_number = (*parent_header.number()).saturated_into::<u32>() + 1;
        let header_number = (block_number as u64).saturated_into::<<B::Header as HeaderTrait>::Number>();
        
        log::trace!("ProposerFactory: Creating block #{} with parent #{} (hash: {:?})", 
               block_number, parent_header.number(), parent_hash);
        
        // Determine the block author
        let author = if let Some(forced_author) = force_author {
            log::trace!("ProposerFactory: Using forced author: {:?}", forced_author);
            forced_author
        } else {
            // Fetch expected author from runtime API
            let api = self.client.runtime_api();
            let best_hash = self.client.info().best_hash;
            
            match api.get_expected_author(best_hash, block_number) {
                Ok(Some(account_id)) => {
                    let author = Public::from_raw(*account_id.as_ref());
                    log::trace!("ProposerFactory: Runtime selected author: {:?}", author);
                    author
                }
                Ok(None) => {
                    return Err(ConsensusError::AuthorSelection(
                        "No expected author returned by runtime".into()
                    ));
                }
                Err(e) => {
                    return Err(ConsensusError::AuthorSelection(
                        format!("Runtime API error: {:?}", e)
                    ));
                }
            }
        };

        // Create inherent data
        log::trace!("ProposerFactory: Creating inherent data");
        let inherent_data = self.inherent_providers.create_inherent_data().await
            .map_err(|e| ConsensusError::Proposer(format!("Failed to create inherent data: {:?}", e)))?;
        
        // Convert inherent data to extrinsics
        let inherent_extrinsics = match self.client.runtime_api().inherent_extrinsics(parent_hash, inherent_data) {
            Ok(extrinsics) => {
                log::trace!("ProposerFactory: Created {} inherent extrinsics", extrinsics.len());
                extrinsics
            }
            Err(e) => {
                error!("ProposerFactory: Failed to create inherent extrinsics: {:?}", e);
                // For critical failure, we cannot proceed without inherents
                return Err(ConsensusError::Proposer(
                    format!("Failed to create inherent extrinsics: {:?}", e)
                ));
            }
        };
        
        // Get transactions from the pool
        let ready_transactions = self.collect_transactions_from_pool().await?;
        
        // Combine inherents + transactions (inherents must come first)
        let mut all_extrinsics = inherent_extrinsics;
        let inherent_count = all_extrinsics.len();
        all_extrinsics.extend(ready_transactions);
        let transaction_count = all_extrinsics.len() - inherent_count;
        
        log::trace!("ProposerFactory: Combined {} inherent + {} transaction extrinsics (total: {})", 
               inherent_count, transaction_count, all_extrinsics.len());
        
        // Create digest with author information
        let mut digest = sp_runtime::generic::Digest::default();
        if let Some(author_digest_item) = author_digest {
            digest.push(author_digest_item);
            log::trace!("ProposerFactory: Added author digest to block header");
        }
        
        // Build the block with proper state root calculation
        log::trace!("ProposerFactory: Building block with proper state root calculation");
        let extrinsics_count = all_extrinsics.len(); // Store count before move
        let block = self.build_block_with_state_root(
            parent_hash,
            header_number,
            digest,
            all_extrinsics,
        ).await?;
        
        // Final validation
        log::trace!("ProposerFactory: Performing final block validation");
        self.validate_final_block(&block, &author, block_number)?;
        
        // Update timing
        self.last_block_time = Some(Instant::now());
        
        log::info!("ProposerFactory: Successfully created block #{} with {} extrinsics", 
               block_number, extrinsics_count);
        log::trace!("ProposerFactory: Block hash: {:?}, state root: {:?}", 
               block.header().hash(), block.header().state_root());
        
        Ok((block, author))
    }
    
    /// Validate the final block before returning it
    fn validate_final_block(&self, block: &B, author: &Public, expected_block_number: u32) -> ConsensusResult<()> {
        let header = block.header();
        
        log::trace!("ProposerFactory: Validating final block");
        
        // Check block number
        let actual_block_number = (*header.number()).saturated_into::<u32>();
        if actual_block_number != expected_block_number {
            return Err(ConsensusError::Proposer(
                format!("Block number mismatch: expected {}, got {}", 
                       expected_block_number, actual_block_number)
            ));
        }
        
        // Check that we have a valid state root
        let zero_hash = Default::default();
        if header.state_root() == &zero_hash {
            return Err(ConsensusError::Proposer(
                "Final block still has zero state root - block building failed".into()
            ));
        }
        
        // Check extrinsics root for non-empty blocks
        if !block.extrinsics().is_empty() && header.extrinsics_root() == &zero_hash {
            return Err(ConsensusError::Proposer(
                "Final block has zero extrinsics root but contains extrinsics".into()
            ));
        }
        
        log::trace!("ProposerFactory: Final block validation passed");
        log::trace!("ProposerFactory: - Block number: {}", actual_block_number);
        log::trace!("ProposerFactory: - Author: {:?}", author);
        log::trace!("ProposerFactory: - State root: {:?}", header.state_root());
        log::trace!("ProposerFactory: - Extrinsics root: {:?}", header.extrinsics_root());
        log::trace!("ProposerFactory: - Parent hash: {:?}", header.parent_hash());
        log::trace!("ProposerFactory: - Extrinsics count: {}", block.extrinsics().len());
        
        Ok(())
    }
    
    /// Emergency block creation method that creates a minimal block with only inherents
    /// This is used as a fallback when normal block creation fails
    pub async fn create_emergency_block(
        &mut self,
        parent_hash: B::Hash,
        _slot: u64,
        author: Public,
    ) -> ConsensusResult<(B, Public)> {
        warn!("ProposerFactory: Creating emergency block - this should only be used as a fallback");
        
        // Get parent header
        let parent_header = self.client.header(parent_hash)
            .map_err(|e| ConsensusError::Proposer(format!("Failed to get parent header: {:?}", e)))?
            .ok_or_else(|| ConsensusError::Proposer("Parent header not found".into()))?;
        
        let block_number = (*parent_header.number()).saturated_into::<u32>() + 1;
        let header_number = (block_number as u64).saturated_into::<<B::Header as HeaderTrait>::Number>();
        
        warn!("ProposerFactory: Creating emergency block #{} with author {:?}", block_number, author);
        
        // Create minimal inherent data (timestamp only)
        let inherent_data = self.inherent_providers.create_inherent_data().await
            .map_err(|e| ConsensusError::Proposer(format!("Failed to create inherent data: {:?}", e)))?;
        
        // Convert to extrinsics
        let inherent_extrinsics = self.client.runtime_api().inherent_extrinsics(parent_hash, inherent_data)
            .map_err(|e| ConsensusError::Proposer(format!("Failed to create inherent extrinsics: {:?}", e)))?;
        
        debug!("ProposerFactory: Emergency block using {} inherent extrinsics only", inherent_extrinsics.len());
        
        // Build block with only inherents
        let block = self.build_block_with_state_root(
            parent_hash,
            header_number,
            Default::default(), // No special digest
            inherent_extrinsics,
        ).await?;
        
        warn!("ProposerFactory: Emergency block #{} created successfully", block_number);
        
        self.last_block_time = Some(Instant::now());
        Ok((block, author))
    }
    
    /// Build a block with proper state root calculation using the BlockBuilder API
    /// This method follows the proper Substrate block building pattern:
    /// 1. Initialize block with temporary header
    /// 2. Apply all extrinsics one by one
    /// 3. Finalize block to get header with calculated state root and extrinsics root
    async fn build_block_with_state_root(
        &self,
        parent_hash: B::Hash,
        block_number: <<B as BlockTrait>::Header as HeaderTrait>::Number,
        digest: sp_runtime::generic::Digest,
        extrinsics: Vec<B::Extrinsic>,
    ) -> ConsensusResult<B> {
        log::trace!("ProposerFactory: Starting block building process for block #{}", block_number);
        log::trace!("ProposerFactory: Parent hash: {:?}, Extrinsics count: {}", parent_hash, extrinsics.len());
        
        let api = self.client.runtime_api();
        
        // Step 1: Create a temporary header for block initialization
        // The state_root and extrinsics_root will be calculated during the building process
        let temp_header = B::Header::new(
            block_number,
            Default::default(), // extrinsics_root - will be calculated by finalize_block
            Default::default(), // state_root - will be calculated by finalize_block
            parent_hash,
            digest.clone(),
        );
        
        log::trace!("ProposerFactory: Created temporary header for block #{}", block_number);
        log::trace!("ProposerFactory: Temp header - number: {:?}, parent: {:?}", 
               temp_header.number(), temp_header.parent_hash());
        
        // Step 2: Initialize the block in the runtime state
        // This sets up the runtime state for block building
        log::trace!("ProposerFactory: Initializing block in runtime state");
        let _inclusion_mode = api.initialize_block(parent_hash, &temp_header)
            .map_err(|e| {
                error!("ProposerFactory: Failed to initialize block: {:?}", e);
                ConsensusError::Proposer(format!("Failed to initialize block: {:?}", e))
            })?;
        
        log::trace!("ProposerFactory: Block initialized successfully");
        
        // Step 3: Apply all extrinsics to the runtime state
        log::trace!("ProposerFactory: Applying {} extrinsics to runtime state", extrinsics.len());
        let mut applied_count = 0;
        let mut failed_count = 0;
        
        for (i, extrinsic) in extrinsics.iter().enumerate() {
            log::trace!("ProposerFactory: Applying extrinsic {} of {}", i + 1, extrinsics.len());
            
            match api.apply_extrinsic(parent_hash, extrinsic.clone()) {
                Ok(_apply_result) => {
                    log::trace!("ProposerFactory: Successfully applied extrinsic {}", i);
                    applied_count += 1;
                }
                Err(api_error) => {
                    error!("ProposerFactory: Failed to apply extrinsic {} due to API error: {:?}", i, api_error);
                    failed_count += 1;
                    
                    // For critical extrinsics (like inherents), we might want to fail the entire block
                    if i < 1 { // Assume first extrinsic is critical (timestamp inherent)
                        return Err(ConsensusError::Proposer(
                            format!("Failed to apply critical extrinsic {}: {:?}", i, api_error)
                        ));
                    }
                    
                    // For non-critical extrinsics, we can continue but log the failure
                    warn!("ProposerFactory: Skipping failed extrinsic {} and continuing", i);
                }
            }
        }
        
        log::trace!("ProposerFactory: Applied {} extrinsics successfully, {} failed", 
               applied_count, failed_count);
        
        // Step 4: Finalize the block to calculate the correct state root and extrinsics root
        log::trace!("ProposerFactory: Finalizing block to calculate state root and extrinsics root");
        let final_header = api.finalize_block(parent_hash)
            .map_err(|e| {
                error!("ProposerFactory: Failed to finalize block: {:?}", e);
                ConsensusError::Proposer(format!("Failed to finalize block: {:?}", e))
            })?;
        
        log::trace!("ProposerFactory: Block finalized successfully");
        log::trace!("ProposerFactory: Final header - number: {:?}, state_root: {:?}, extrinsics_root: {:?}", 
               final_header.number(), final_header.state_root(), final_header.extrinsics_root());
        log::trace!("ProposerFactory: Final header - parent_hash: {:?}, digest logs: {}", 
               final_header.parent_hash(), final_header.digest().logs().len());
        
        // Step 5: Verify the final header has the correct parent hash and digest
        if final_header.parent_hash() != &parent_hash {
            error!("ProposerFactory: Final header parent hash mismatch! Expected: {:?}, Got: {:?}", 
                   parent_hash, final_header.parent_hash());
            return Err(ConsensusError::Proposer("Parent hash mismatch in final header".into()));
        }
        
        // Verify the digest is preserved (it should contain our author information)
        if final_header.digest().logs().len() != digest.logs().len() {
            warn!("ProposerFactory: Digest logs count changed during finalization. Original: {}, Final: {}", 
                  digest.logs().len(), final_header.digest().logs().len());
        }
        
        // Step 6: Create the final block with the calculated header and original extrinsics
        // Note: We use the original extrinsics list, not just the applied ones,
        // because the block should contain all extrinsics that were attempted
        let final_block = B::new(final_header, extrinsics);
        
        log::trace!("ProposerFactory: Created final block with {} extrinsics", final_block.extrinsics().len());
        log::trace!("ProposerFactory: Final block hash: {:?}", final_block.header().hash());
        
        // Step 7: Validate the final block structure
        self.validate_built_block(&final_block)?;
        
        log::trace!("ProposerFactory: Block building completed successfully for block #{}", block_number);
        Ok(final_block)
    }
    
    /// Validate the structure of a built block to ensure it's correct
    fn validate_built_block(&self, block: &B) -> ConsensusResult<()> {
        let header = block.header();
        let extrinsics = block.extrinsics();
        
        debug!("ProposerFactory: Validating built block structure");
        
        // Check that state root is not the default (zero) value
        let zero_hash = Default::default();
        if header.state_root() == &zero_hash {
            error!("ProposerFactory: Block validation failed - state root is still zero!");
            return Err(ConsensusError::Proposer("Built block has zero state root".into()));
        }
        
        // Check that extrinsics root is not the default (zero) value if we have extrinsics
        if !extrinsics.is_empty() && header.extrinsics_root() == &zero_hash {
            error!("ProposerFactory: Block validation failed - extrinsics root is zero but block has extrinsics!");
            return Err(ConsensusError::Proposer("Built block has zero extrinsics root with non-empty extrinsics".into()));
        }
        
        // Check that parent hash is not zero (unless this is genesis)
        if *header.number() != Zero::zero() && header.parent_hash() == &zero_hash {
            error!("ProposerFactory: Block validation failed - parent hash is zero for non-genesis block!");
            return Err(ConsensusError::Proposer("Built block has zero parent hash".into()));
        }
        
        debug!("ProposerFactory: Block validation passed");
        debug!("ProposerFactory: - State root: {:?} (non-zero: {})", 
               header.state_root(), header.state_root() != &zero_hash);
        debug!("ProposerFactory: - Extrinsics root: {:?} (non-zero: {})", 
               header.extrinsics_root(), header.extrinsics_root() != &zero_hash);
        debug!("ProposerFactory: - Parent hash: {:?} (non-zero: {})", 
               header.parent_hash(), header.parent_hash() != &zero_hash);
        debug!("ProposerFactory: - Block number: {:?}", header.number());
        debug!("ProposerFactory: - Extrinsics count: {}", extrinsics.len());
        
        Ok(())
    }

    /// Collect transactions from the transaction pool
    async fn collect_transactions_from_pool(&self) -> ConsensusResult<Vec<B::Extrinsic>> {
        let ready_transactions = self.transaction_pool.ready()
            .take(self.max_transactions_per_block)
            .map(|tx| (**tx.data()).clone())
            .collect::<Vec<_>>();
        
        debug!("Collected {} transactions from pool", ready_transactions.len());
        Ok(ready_transactions)
    }
    


    /// Create a new block with the expected author from the DCF runtime API (legacy method for compatibility)
    pub fn create_block(&mut self, parent_hash: B::Hash, slot: u64) -> ConsensusResult<(B::Header, Public)> {
        warn!("ProposerFactory: Using legacy create_block method - this method creates headers with placeholder state roots");
        warn!("ProposerFactory: Consider using create_block_with_transactions for proper state root calculation");
        
        // Fetch expected author from runtime API
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        let block_number = slot as u32;
        let author = match api.get_expected_author(best_hash, block_number) {
            Ok(Some(account_id)) => Public::from_raw(*account_id.as_ref()),
            Ok(None) => return Err(ConsensusError::AuthorSelection("No expected author returned by runtime".into())),
            Err(e) => return Err(ConsensusError::AuthorSelection(format!("Runtime API error: {:?}", e))),
        };

        // Create basic block header with placeholder values
        // NOTE: This method is deprecated because it doesn't calculate proper state roots
        let number = <<B as BlockTrait>::Header as HeaderTrait>::Number::zero();
        debug!("ProposerFactory: Legacy create_block - creating header with parent_hash: {:?}", parent_hash);
        
        let header = B::Header::new(
            number,
            Default::default(), // extrinsics_root - placeholder
            Default::default(), // state_root - placeholder (THIS IS THE PROBLEM!)
            parent_hash,
            Default::default(), // digest
        );
        
        debug!("ProposerFactory: Legacy header created - parent hash verification: expected: {:?}, actual: {:?}", 
               parent_hash, header.parent_hash());
        
        // Verify parent hash propagation
        if header.parent_hash() != &parent_hash {
            error!("ProposerFactory: Legacy method parent hash mismatch! Expected: {:?}, Got: {:?}", 
                   parent_hash, header.parent_hash());
            return Err(ConsensusError::Proposer("Parent hash mismatch in legacy header creation".into()));
        }

        warn!("ProposerFactory: Legacy method returning header with placeholder state root - this will cause block import failures!");
        Ok((header, author))
    }

    /// Set the minimum time between blocks
    pub fn set_min_block_time(&mut self, min_block_time: Duration) {
        self.min_block_time = min_block_time;
    }

    /// Create a new block proposer (header only, for compatibility)
    /// WARNING: This method creates headers with placeholder state roots and should not be used for actual block production
    pub fn create_proposer(&self) -> ConsensusResult<B::Header> {
        warn!("ProposerFactory: create_proposer method creates headers with placeholder state roots");
        warn!("ProposerFactory: This method should not be used for actual block production");
        
        let number = <<B as BlockTrait>::Header as HeaderTrait>::Number::zero();
        let header = B::Header::new(
            number,
            Default::default(), // extrinsics_root - placeholder
            Default::default(), // state_root - placeholder (THIS IS THE PROBLEM!)
            Default::default(), // parent_hash - placeholder
            Default::default(), // digest
        );
        
        warn!("ProposerFactory: Returning header with placeholder values - this will cause block import failures if used!");
        Ok(header)
    }
}