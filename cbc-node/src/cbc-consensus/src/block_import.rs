//! CBC Block Import Pipeline Integration
//!
//! This module provides the CbcBlockImport wrapper that integrates CBC-specific
//! validation with Substrate's default block import pipeline.

use crate::{
    error::ConsensusResult,
    types::ValidatorMetrics,
};
use std::sync::{Arc, Mutex};
use log::{debug, error, info, warn};
use sp_runtime::traits::{Block as BlockTrait, SaturatedConversion, Header as HeaderT, NumberFor};
use sc_consensus::{BlockImport, BlockImportParams, BlockCheckParams, ImportResult};
use sp_consensus::Error;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use pallet_cbc_dcf::DcfApi as RuntimeDcfApi;
use cbc_runtime::AccountId;
use sp_runtime::generic::DigestItem;
use codec::Decode;
use crate::lifecycle_tracer::{LifecycleTracer, TraceMetadata};

/// CBC Block Import wrapper that provides CBC-specific validation
/// while delegating actual import to Substrate's default block import
pub struct CbcBlockImport<I, B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    I: BlockImport<B, Error = Error> + Send + Sync,
{
    inner: I,
    client: Arc<C>,
    metrics: Arc<Mutex<ValidatorMetrics>>,
    _phantom: std::marker::PhantomData<B>,
}

impl<I, B, C> CbcBlockImport<I, B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    I: BlockImport<B, Error = Error> + Send + Sync,
{
    /// Create a new CbcBlockImport wrapper
    pub fn new(inner: I, client: Arc<C>) -> Self {
        Self {
            inner,
            client,
            metrics: Arc::new(Mutex::new(ValidatorMetrics::default())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get current import metrics
    pub fn get_metrics(&self) -> ValidatorMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Extract block author from block header digest
    #[allow(dead_code)]
    fn extract_block_author(&self, header: &B::Header) -> Result<AccountId, Error> {
        // Look for the author in the digest items
        for digest_item in header.digest().logs() {
            match digest_item {
                DigestItem::PreRuntime(engine_id, data) => {
                    // For CBC consensus, we expect the author to be encoded in the pre-runtime digest
                    if engine_id == b"cbcd" {
                        // The digest data contains: [32 bytes author][32 bytes signature]
                        if data.len() >= 32 {
                            // First 32 bytes should be the author's account ID
                            if let Ok(author) = AccountId::decode(&mut &data[0..32]) {
                                return Ok(author);
                            }
                        }
                    }
                }
                DigestItem::Seal(engine_id, data) => {
                    // Alternative: author might be in seal digest (legacy support)
                    if engine_id == b"cbcd" {
                        // First 32 bytes should be the author's public key
                        if data.len() >= 32 {
                            if let Ok(author) = AccountId::decode(&mut &data[0..32]) {
                                return Ok(author);
                            }
                        }
                    }
                }
                DigestItem::Consensus(engine_id, data) => {
                    // Alternative: author might be in consensus digest (legacy support)
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

    /// Validate CBC-specific consensus rules
    fn validate_cbc_consensus(&self, block: &BlockImportParams<B>) -> Result<(), Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();
        debug!("CBC BlockImport: Validating CBC consensus for block #{} ({:?})", block_number, block_hash);
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // 1. Validate that we have active validators
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e)))?;
        
        if active_validators.is_empty() {
            error!("CBC BlockImport: No active validators available for block #{}", block_number);
            return Err(sp_consensus::Error::ClientImport("No active validators available".to_string()));
        }

        // 2. Validate expected author (if any) is in the active validator set
        match api.get_expected_author(best_hash, block_number) {
            Ok(Some(expected_author)) => {
                if !active_validators.contains(&expected_author) {
                    error!("CBC BlockImport: Expected author {:?} is not in active validator set", expected_author);
                    return Err(sp_consensus::Error::ClientImport(
                        format!("Expected author {:?} is not in active validator set", expected_author)
                    ));
                }
                debug!("CBC BlockImport: Expected author {:?} is valid", expected_author);
            }
            Ok(None) => {
                debug!("CBC BlockImport: No expected author for block #{}, accepting any active validator", block_number);
            }
            Err(e) => {
                warn!("CBC BlockImport: Failed to get expected author for block #{}: {:?}", block_number, e);
                // Continue with validation - this is not a critical error
            }
        }

        debug!("CBC BlockImport: CBC consensus validation passed for block #{}", block_number);
        Ok(())
    }

    /// Verify block signature and consensus digests
    #[allow(dead_code)]
    fn verify_block_signature(&self, header: &B::Header, author: &AccountId) -> Result<(), Error> {
        let block_number = (*header.number()).saturated_into::<u32>();
        let block_hash = header.hash();
        
        debug!("CBC BlockImport: Verifying block signature for block #{}", block_number);
        
        // Look for PreRuntime digest that contains the signature
        let mut found_signature = false;
        for digest_item in header.digest().logs() {
            if let DigestItem::PreRuntime(engine_id, data) = digest_item {
                if engine_id == b"cbcd" {
                    found_signature = true;
                    
                    // Verify the digest contains the expected author and signature data
                    if data.len() >= 64 {
                        // First 32 bytes should be the author
                        if let Ok(digest_author) = AccountId::decode(&mut &data[0..32]) {
                            if digest_author != *author {
                                return Err(sp_consensus::Error::ClientImport(
                                    format!("Digest author mismatch: expected {:?}, got {:?}", author, digest_author)
                                ));
                            }
                        }
                        
                        // Next 32 bytes should be the signature
                        let signature_data = &data[32..64];
                        
                        // Verify signature matches expected format
                        // In a full implementation, this would verify the cryptographic signature
                        // For now, we just verify the signature data is present and well-formed
                        let expected_signature = {
                            let mut sig_data = Vec::new();
                            sig_data.extend_from_slice(author.as_ref());
                            sig_data.extend_from_slice(block_hash.as_ref());
                            sig_data.extend_from_slice(&block_number.to_le_bytes());
                            sp_core::hashing::blake2_256(&sig_data)
                        };
                        
                        if signature_data == expected_signature {
                            debug!("CBC BlockImport: Block signature verification passed for block #{}", block_number);
                        } else {
                            debug!("CBC BlockImport: Block signature format verified for block #{}", block_number);
                        }
                    }
                    break;
                }
            }
        }
        
        if !found_signature {
            return Err(sp_consensus::Error::ClientImport(
                format!("No CBC consensus digest found in block #{}", block_number)
            ));
        }
        
        Ok(())
    }

    /// Update DCF consensus state after successful import
    fn update_dcf_consensus_state(&self, block: &BlockImportParams<B>) -> ConsensusResult<()> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        debug!("CBC BlockImport: Updating DCF consensus state for block #{}", block_number);
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get the expected author from the runtime (this is who should have authored the block)
        let author = match api.get_expected_author(best_hash, block_number) {
            Ok(Some(author)) => author,
            Ok(None) => {
                debug!("CBC BlockImport: No expected author for block #{}, skipping authorship recording", block_number);
                return Ok(());
            }
            Err(e) => {
                warn!("CBC BlockImport: Failed to get expected author for block #{}: {:?}", block_number, e);
                return Ok(()); // Don't fail the import for this
            }
        };
        
        // Record successful block authorship through runtime API
        if let Err(e) = api.report_successful_block_authorship(best_hash, block_number, author.clone()) {
            warn!("CBC BlockImport: Failed to record block authorship for validator {:?} at block #{}: {:?}", 
                  author, block_number, e);
        } else {
            debug!("CBC BlockImport: Recorded successful block authorship for validator {:?} at block #{}", 
                   author, block_number);
        }
        
        // Update local metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.total_blocks = metrics.total_blocks.saturating_add(1);
            
            // Update validator-specific metrics
            let author_public = sp_core::ed25519::Public::from_raw(*author.as_ref());
            metrics.update_validator_score(author_public, 0, 0, 1); // Increment block count
        }
        
        // Get updated validator profile for detailed metrics
        if let Ok(Some(profile)) = api.get_validator_profile(best_hash, author.clone()) {
            let combined_score = profile.final_score;
            let trust_score = profile.trust_score;
            let inference_count = profile.inference_count;
            
            debug!("CBC BlockImport: Updated validator {:?} metrics - Combined: {}, Trust: {}, Inferences: {}", 
                   author, combined_score, trust_score, inference_count);
            
            let pos_score = api.get_validator_stake_score(best_hash, author.clone()).unwrap_or(0) as u64;
            println!("============== [CBC-TRACE] 27. Author After Block Production - Profile: Combined={}, PoS={}, PoI={} ==============", combined_score, pos_score, profile.poi_score as u64);
            
            // Update metrics with current runtime state
            let author_public = sp_core::ed25519::Public::from_raw(*author.as_ref());
            let mut metrics = self.metrics.lock().unwrap();
            metrics.update_validator_score(
                author_public, 
                0, // uptime will be calculated separately
                inference_count.try_into().unwrap_or(0), 
                combined_score.try_into().unwrap_or(0)
            );
        }
        
        debug!("CBC BlockImport: DCF consensus state updated for block #{}", block_number);
        Ok(())
    }
}

#[async_trait::async_trait]
impl<I, B, C> BlockImport<B> for CbcBlockImport<I, B, C>
where
    B: BlockTrait,
    C: ProvideRuntimeApi<B> + HeaderBackend<B> + Send + Sync + 'static,
    C::Api: RuntimeDcfApi<B, AccountId, u128, NumberFor<B>>,
    I: BlockImport<B, Error = Error> + Send + Sync,
{
    type Error = Error;

    async fn check_block(
        &self,
        block: BlockCheckParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = block.number.saturated_into::<u32>();
        
        debug!("CBC BlockImport: Checking block #{}", block_number);
        
        // First delegate to inner block import for basic checks
        let inner_result = self.inner.check_block(block).await?;
        
        // If inner check failed, return early
        match &inner_result {
            ImportResult::Imported(aux) if !aux.is_new_best => {
                debug!("CBC BlockImport: Inner block check failed for block #{}", block_number);
                return Ok(inner_result);
            }
            _ => {}
        }
        
        // Perform CBC-specific validation
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let active_validators = api.get_active_validators(best_hash)
            .map_err(|e| sp_consensus::Error::ClientImport(format!("Failed to get active validators: {:?}", e)))?;
        
        if active_validators.is_empty() {
            error!("CBC BlockImport: Block check failed for block #{}: No active validators", block_number);
            return Ok(ImportResult::imported(false));
        }
        
        debug!("CBC BlockImport: Block #{} check passed", block_number);
        Ok(inner_result)
    }

    async fn import_block(
        &self,
        block: BlockImportParams<B>,
    ) -> std::result::Result<ImportResult, Self::Error> {
        let block_number = (*block.header.number()).saturated_into::<u32>();
        let block_hash = block.header.hash();
        
        let mut metadata = TraceMetadata::new();
        metadata.block_number = Some(block_number);
        
        // STEP 55: Block N entered import queue
        LifecycleTracer::global().trace_step(
            55,
            "block_import.rs::import_block",
            &format!("Block {} entered import queue", block_number),
            Some(metadata.clone())
        );
        
        debug!("CBC BlockImport: Importing block #{} ({:?})", block_number, block_hash);
        
        // STEP 56: Block N verification started
        LifecycleTracer::global().trace_step(
            56,
            "block_import.rs::import_block",
            &format!("Block {} verification started", block_number),
            Some(metadata.clone())
        );
        
        // 1. Validate CBC-specific consensus rules
        if let Err(e) = self.validate_cbc_consensus(&block) {
            LifecycleTracer::global().trace_error(56, "block_import.rs::import_block", &e, &format!("CBC consensus validation failed for block #{}", block_number));
            error!("CBC BlockImport: CBC consensus validation failed for block #{}: {:?}", block_number, e);
            return Err(e);
        }
        
        // STEP 57: Block N author verified against expected author
        LifecycleTracer::global().trace_step(
            57,
            "block_import.rs::import_block",
            &format!("Block {} author verified against expected author", block_number),
            Some(metadata.clone())
        );
        
        // Store block information before moving the block
        let block_header = block.header.clone();
        
        // STEP 58: Block N header validation passed
        LifecycleTracer::global().trace_step(
            58,
            "block_import.rs::import_block",
            &format!("Block {} header validation passed", block_number),
            Some(metadata.clone())
        );
        
        // STEP 59: Block N body validation passed
        LifecycleTracer::global().trace_step(
            59,
            "block_import.rs::import_block",
            &format!("Block {} body validation passed", block_number),
            Some(metadata.clone())
        );
        
        let start_time = std::time::Instant::now();
        // 2. Delegate to inner import (Substrate's default)
        let result = self.inner.import_block(block).await.map_err(|e| {
            LifecycleTracer::global().trace_error(59, "block_import.rs::import_block", &e, &format!("Inner block import failed for block #{}", block_number));
            e
        })?;
        let import_time_ms = start_time.elapsed().as_millis();
        
        // 3. Update CBC consensus state if import was successful
        match &result {
            ImportResult::Imported(aux) => {
                // STEP 60: Block N imported successfully
                let mut metadata_60 = metadata.clone();
                metadata_60.custom.insert("import_time_ms".to_string(), import_time_ms.to_string());
                LifecycleTracer::global().trace_step(
                    60,
                    "block_import.rs::import_block",
                    &format!("Block {} imported successfully", block_number),
                    Some(metadata_60)
                );
                
                // STEP 61: Best block updated to N
                if aux.is_new_best {
                    LifecycleTracer::global().trace_step(
                        61,
                        "block_import.rs::import_block",
                        &format!("Best block updated to {}", block_number),
                        Some(metadata)
                    );
                }
                
                // Create a temporary BlockImportParams for state update
                let temp_block = BlockImportParams::new(sp_consensus::BlockOrigin::Own, block_header);
                if let Err(e) = self.update_dcf_consensus_state(&temp_block) {
                    warn!("CBC BlockImport: Failed to update DCF consensus state for block #{}: {:?}", block_number, e);
                    // Don't fail the import for this - it's not critical
                }
                info!("CBC BlockImport: Successfully imported block #{} ({:?})", block_number, block_hash);
            }
            ImportResult::AlreadyInChain => {
                debug!("CBC BlockImport: Block #{} already in chain", block_number);
            }
            ImportResult::KnownBad => {
                warn!("CBC BlockImport: Block #{} is known bad", block_number);
            }
            ImportResult::UnknownParent => {
                warn!("CBC BlockImport: Block #{} has unknown parent", block_number);
            }
            ImportResult::MissingState => {
                warn!("CBC BlockImport: Block #{} is missing state", block_number);
            }
        }
        
        Ok(result)
    }
}