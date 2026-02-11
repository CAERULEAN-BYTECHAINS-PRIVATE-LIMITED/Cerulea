#![allow(dead_code, missing_docs)]

use std::sync::Arc;
use std::time::Duration;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use jsonrpsee::RpcModule;
use sc_transaction_pool_api::TransactionPool;
use cbc_runtime::{opaque::Block, AccountId, Balance, Nonce};
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use jsonrpsee::core::RpcResult;
use jsonrpsee::proc_macros::rpc;
use pallet_cbc_pos::PosApi;
use cbc_runtime::pallet_cbc_poi::PoiApi;
use pallet_cbc_dcf::DcfApi;
use crate::fork_detection::ForkReport;

type FullBackend = sc_service::TFullBackend<Block>;

/// Helper macro to time RPC method calls and record metrics
macro_rules! time_rpc_call {
    ($metrics:expr, $method:expr, $call:expr) => {{
        let start = std::time::Instant::now();
        let result = $call;
        let duration = start.elapsed();
        
        if let Some(ref metrics) = $metrics {
            metrics.record_rpc_request_duration($method, duration.as_secs_f64());
        }
        
        result
    }};
}

#[derive(Clone)]
pub struct RateLimiter {
    window: Duration,
    max_requests: u32,
    requests: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl RateLimiter {
    pub fn new(window_secs: u64, max_requests: u32) -> Self {
        Self {
            window: Duration::from_secs(window_secs),
            max_requests,
            requests: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn check_rate_limit(&self, ip: &str) -> bool {
        let now = Instant::now();
        let mut requests = self.requests.lock().unwrap();
        let history = requests.entry(ip.to_string()).or_insert_with(Vec::new);
        history.retain(|&time| now.duration_since(time) <= self.window);
        if history.len() as u32 >= self.max_requests {
            return false;
        }
        history.push(now);
        true
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValidatorStatus {
    Active,
    Inactive,
    Slashed,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InferenceResult {
    pub result: u32,
    pub confidence: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChallengeWindow {
    pub start_block: u32,
    pub end_block: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InferenceStatus {
    Pending,
    Submitted,
    Challenged,
    Verified,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusWeights {
    pub pos_weight: u64,
    pub poi_weight: u64,
}

// CBC Unified RPC Types
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorProfile {
    pub account: AccountId,
    pub stake: Balance,
    pub pos_score: u32,
    pub poi_score: u64,
    pub trust_score: u64,
    pub status: ValidatorStatus,
    pub authored_blocks: u32,
    pub missed_blocks: u32,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustScore {
    pub total: u64,
    pub pos_component: u64,
    pub poi_component: u64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemStatus {
    pub current_epoch: u32,
    pub active_validators: u32,
    pub total_validators: u32,
    pub last_finalized_block: u32,
    pub consensus_health: ConsensusHealth,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsensusHealth {
    Healthy,
    Degraded,
    Critical,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RpcMethodDescription {
    pub name: String,
    pub description: String,
    pub params: Vec<String>,
    pub returns: String,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthStatus {
    pub is_healthy: bool,
    pub issues: Vec<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockAuthoringStats {
    pub authored_blocks: u32,
    pub missed_blocks: u32,
    pub expected_blocks: u32,
    pub participation_rate: f64,
    pub consecutive_misses: u32,
    pub last_authored_block: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct RpcSecurityConfig {
    pub enable_cbc_extensions: bool,
    pub expose_unsafe_methods: bool,
    pub rate_limit_window: u64,
    pub rate_limit_requests: u32,
}

impl Default for RpcSecurityConfig {
    fn default() -> Self {
        Self {
            enable_cbc_extensions: false,
            expose_unsafe_methods: false,
            rate_limit_window: 60,
            rate_limit_requests: 100,
        }
    }
}

pub struct FullDeps<C, P, B> {
    pub client: Arc<C>,
    pub pool: Arc<P>,
    pub rpc_config: RpcSecurityConfig,
    pub consensus_metrics: Option<cbc_consensus::metrics::ConsensusMetrics>,
    pub backend: Arc<B>,
}

#[rpc(server)]
pub trait ChainApi {
    #[method(name = "chain_getChainName")]
    fn get_chain_name(&self) -> RpcResult<String>;
}

pub struct ChainApiImpl<C: ProvideRuntimeApi<Block> + Send + Sync + 'static> {
    pub client: Arc<C>,
}

impl<C> ChainApiServer for ChainApiImpl<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
{
    fn get_chain_name(&self) -> RpcResult<String> {
        Ok("CBC-Chain".to_string())
    }
}

// PoS RPC API
#[rpc(server)]
pub trait PosRpcApi {
    #[method(name = "pos_getValidatorScore")]
    fn get_validator_score(&self, validator: AccountId) -> RpcResult<u32>;
    
    #[method(name = "pos_getValidatorStake")]
    fn get_validator_stake(&self, validator: AccountId) -> RpcResult<Balance>;
    
    #[method(name = "pos_getSlashingCount")]
    fn get_slashing_count(&self, validator: AccountId) -> RpcResult<u32>;
    
    #[method(name = "pos_getValidatorStatus")]
    fn get_validator_status(&self, validator: AccountId) -> RpcResult<ValidatorStatus>;
}

pub struct PosRpcApiImpl<C> {
    client: Arc<C>,
    consensus_metrics: Option<cbc_consensus::metrics::ConsensusMetrics>,
}

impl<C> PosRpcApiImpl<C> {
    pub fn new(client: Arc<C>) -> Self {
        Self { 
            client,
            consensus_metrics: None,
        }
    }
    
    pub fn new_with_metrics(client: Arc<C>, consensus_metrics: cbc_consensus::metrics::ConsensusMetrics) -> Self {
        Self { 
            client,
            consensus_metrics: Some(consensus_metrics),
        }
    }
}

impl<C> PosRpcApiServer for PosRpcApiImpl<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: pallet_cbc_pos::PosApi<Block, AccountId, Balance>,
{
    fn get_validator_score(&self, validator: AccountId) -> RpcResult<u32> {
        time_rpc_call!(self.consensus_metrics, "pos_getValidatorScore", {
            let api = self.client.runtime_api();
            let best_hash = self.client.info().best_hash;
            
            api.get_validator_score(best_hash, validator)
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Runtime API call failed: {:?}", e),
                    None::<()>
                ))
        })
    }
    
    fn get_validator_stake(&self, validator: AccountId) -> RpcResult<Balance> {
        time_rpc_call!(self.consensus_metrics, "pos_getValidatorStake", {
            let api = self.client.runtime_api();
            let best_hash = self.client.info().best_hash;
            
            api.get_validator_stake(best_hash, validator)
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Runtime API call failed: {:?}", e),
                    None::<()>
                ))
        })
    }
    
    fn get_slashing_count(&self, validator: AccountId) -> RpcResult<u32> {
        time_rpc_call!(self.consensus_metrics, "pos_getSlashingCount", {
            let api = self.client.runtime_api();
            let best_hash = self.client.info().best_hash;
            
            api.get_slashing_count(best_hash, validator)
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Runtime API call failed: {:?}", e),
                    None::<()>
                ))
        })
    }
    
    fn get_validator_status(&self, validator: AccountId) -> RpcResult<ValidatorStatus> {
        time_rpc_call!(self.consensus_metrics, "pos_getValidatorStatus", {
            let api = self.client.runtime_api();
            let best_hash = self.client.info().best_hash;
            
            // Get active validators list
            let active_validators = api.get_active_validators(best_hash)
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Runtime API call failed: {:?}", e),
                    None::<()>
                ))?;
            
            // Check if validator is in active list
            if !active_validators.contains(&validator) {
                return Ok(ValidatorStatus::Inactive);
            }
            
            // Check slashing count to determine if slashed
            let slashing_count = api.get_slashing_count(best_hash, validator.clone())
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    format!("Runtime API call failed: {:?}", e),
                    None::<()>
                ))?;
            
            if slashing_count > 0 {
                Ok(ValidatorStatus::Slashed)
            } else {
                Ok(ValidatorStatus::Active)
            }
        })
    }
}

// PoI RPC API
#[rpc(server)]
pub trait PoiRpcApi {
    #[method(name = "poi_getInferenceResult")]
    fn get_inference_result(&self, validator: AccountId) -> RpcResult<Option<InferenceResult>>;
    
    #[method(name = "poi_getInferenceConfidence")]
    fn get_inference_confidence(&self, validator: AccountId) -> RpcResult<Option<u32>>;
    
    #[method(name = "poi_getChallengeWindow")]
    fn get_challenge_window(&self) -> RpcResult<ChallengeWindow>;
    
    #[method(name = "poi_getInferenceStatus")]
    fn get_inference_status(&self, validator: AccountId) -> RpcResult<InferenceStatus>;
}

pub struct PoiRpcApiImpl<C> {
    client: Arc<C>,
}

impl<C> PoiRpcApiImpl<C> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client }
    }
}

impl<C> PoiRpcApiServer for PoiRpcApiImpl<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: cbc_runtime::pallet_cbc_poi::PoiApi<Block, AccountId>,
{
    fn get_inference_result(&self, validator: AccountId) -> RpcResult<Option<InferenceResult>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        match api.get_inference_result(best_hash, validator) {
            Ok(Some((result, confidence))) => Ok(Some(InferenceResult { result, confidence })),
            Ok(None) => Ok(None),
            Err(e) => Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
        }
    }
    
    fn get_inference_confidence(&self, validator: AccountId) -> RpcResult<Option<u32>> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        match api.get_inference_result(best_hash, validator) {
            Ok(Some((_result, confidence))) => Ok(Some(confidence)),
            Ok(None) => Ok(None),
            Err(e) => Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
        }
    }
    
    fn get_challenge_window(&self) -> RpcResult<ChallengeWindow> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get current epoch from PoI API
        let current_epoch = api.get_current_epoch(best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))?;
        
        // Calculate challenge window based on current epoch
        // Assuming each epoch is 100 blocks and challenge window is 50 blocks
        let start_block = current_epoch * 100;
        let end_block = start_block + 50;
        
        Ok(ChallengeWindow { start_block, end_block })
    }
    
    fn get_inference_status(&self, validator: AccountId) -> RpcResult<InferenceStatus> {
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Check if validator has an inference result
        let has_inference = api.get_inference_result(best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))?;
        
        // Check if validator has a challenge
        let has_challenge = api.get_challenge(best_hash, validator)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))?;
        
        match (has_inference, has_challenge) {
            (None, None) => Ok(InferenceStatus::Pending),
            (Some(_), None) => Ok(InferenceStatus::Submitted),
            (Some(_), Some(_)) => Ok(InferenceStatus::Challenged),
            (None, Some(_)) => Ok(InferenceStatus::Verified), // Challenge resolved
        }
    }
}

// DCF RPC API
#[rpc(server)]
pub trait DcfRpcApi {
    #[method(name = "dcf_getCurrentAuthor")]
    fn get_current_author(&self) -> RpcResult<Option<AccountId>>;
    
    #[method(name = "dcf_getExpectedAuthor")]
    fn get_expected_author(&self, block_number: u32) -> RpcResult<Option<AccountId>>;
    
    #[method(name = "dcf_getValidatorScores")]
    fn get_validator_scores(&self) -> RpcResult<Vec<(AccountId, u64)>>;
    
    #[method(name = "dcf_getConsensusWeights")]
    fn get_consensus_weights(&self) -> RpcResult<ConsensusWeights>;
}

pub struct DcfRpcApiImpl<C> {
    client: Arc<C>,
    security_config: RpcSecurityConfig,
}

impl<C> DcfRpcApiImpl<C> {
    pub fn new(client: Arc<C>, security_config: RpcSecurityConfig) -> Self {
        Self { client, security_config }
    }
    
    fn check_cbc_extensions_enabled(&self) -> RpcResult<()> {
        if !self.security_config.enable_cbc_extensions {
            return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "CBC RPC extensions are disabled. Use --enable-cbc-extensions flag.".to_string(),
                None::<()>
            ));
        }
        Ok(())
    }
}

impl<C> DcfRpcApiServer for DcfRpcApiImpl<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>,
{
    fn get_current_author(&self) -> RpcResult<Option<AccountId>> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get current block number
        let current_block = self.client.info().best_number as u32;
        
        api.get_expected_author(best_hash, current_block)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
    }
    
    fn get_expected_author(&self, block_number: u32) -> RpcResult<Option<AccountId>> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        api.get_expected_author(best_hash, block_number)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
    }
    
    fn get_validator_scores(&self) -> RpcResult<Vec<(AccountId, u64)>> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        api.get_validator_scores(best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
    }
    
    fn get_consensus_weights(&self) -> RpcResult<ConsensusWeights> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        let (pos_weight, poi_weight) = api.get_consensus_weights(best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))?;
        
        Ok(ConsensusWeights { pos_weight, poi_weight })
    }
}

// CBC Unified RPC API
#[rpc(server)]
pub trait CbcRpcApi {
    #[method(name = "cbc_getCurrentEpoch")]
    fn get_current_epoch(&self) -> RpcResult<u32>;
    
    #[method(name = "cbc_getValidatorProfile")]
    fn get_validator_profile(&self, validator: AccountId) -> RpcResult<ValidatorProfile>;
    
    #[method(name = "cbc_getTrustScore")]
    fn get_trust_score(&self, validator: AccountId) -> RpcResult<TrustScore>;
    
    #[method(name = "cbc_listValidators")]
    fn list_validators(&self) -> RpcResult<Vec<AccountId>>;
    
    #[method(name = "cbc_getStatus")]
    fn get_status(&self) -> RpcResult<SystemStatus>;
    
    #[method(name = "cbc_describe")]
    fn describe(&self) -> RpcResult<Vec<RpcMethodDescription>>;
    
    #[method(name = "cbc_health")]
    fn health(&self) -> RpcResult<HealthStatus>;
    
    #[method(name = "cbc_getBlockAuthoringStats")]
    fn get_block_authoring_stats(&self, validator: AccountId) -> RpcResult<BlockAuthoringStats>;
    
    #[method(name = "cbc_getAllBlockAuthoringStats")]
    fn get_all_block_authoring_stats(&self) -> RpcResult<Vec<(AccountId, BlockAuthoringStats)>>;
}

pub struct CbcRpcApiImpl<C> {
    client: Arc<C>,
    security_config: RpcSecurityConfig,
}

impl<C> CbcRpcApiImpl<C> {
    pub fn new(client: Arc<C>, security_config: RpcSecurityConfig) -> Self {
        Self { client, security_config }
    }
    
    fn check_cbc_extensions_enabled(&self) -> RpcResult<()> {
        if !self.security_config.enable_cbc_extensions {
            return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "CBC RPC extensions are disabled. Use --enable-cbc-extensions flag.".to_string(),
                None::<()>
            ));
        }
        Ok(())
    }
}

impl<C> CbcRpcApiServer for CbcRpcApiImpl<C>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    C::Api: pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>,
    C::Api: pallet_cbc_pos::PosApi<Block, AccountId, Balance>,
    C::Api: cbc_runtime::pallet_cbc_poi::PoiApi<Block, AccountId>,
{
    fn get_current_epoch(&self) -> RpcResult<u32> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_current_epoch(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Runtime API call failed: {:?}", e),
                None::<()>
            ))
    }
    
    fn get_validator_profile(&self, validator: AccountId) -> RpcResult<ValidatorProfile> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Aggregate data from multiple pallets
        let stake = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_validator_stake(&api, best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get validator stake: {:?}", e),
                None::<()>
            ))?;
        
        let pos_score = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_validator_score(&api, best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get PoS score: {:?}", e),
                None::<()>
            ))?;
        
        // Get PoI score from inference result
        let poi_score = match <C::Api as cbc_runtime::pallet_cbc_poi::PoiApi<Block, AccountId>>::get_inference_result(&api, best_hash, validator.clone()) {
            Ok(Some((_result, confidence))) => confidence as u64,
            Ok(None) => 0,
            Err(e) => {
                log::warn!("Failed to get PoI score for validator {:?}: {:?}", validator, e);
                0
            }
        };
        
        // Calculate trust score using consensus weights
        let (pos_weight, poi_weight) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_consensus_weights(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get consensus weights: {:?}", e),
                None::<()>
            ))?;
        
        let trust_score = (pos_score as u64 * pos_weight + poi_score * poi_weight) / (pos_weight + poi_weight);
        
        // Get validator status
        let active_validators = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_active_validators(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get active validators: {:?}", e),
                None::<()>
            ))?;
        
        let slashing_count = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_slashing_count(&api, best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get slashing count: {:?}", e),
                None::<()>
            ))?;
        
        let status = if !active_validators.contains(&validator) {
            ValidatorStatus::Inactive
        } else if slashing_count > 0 {
            ValidatorStatus::Slashed
        } else {
            ValidatorStatus::Active
        };
        
        // Get authored and missed blocks from validator state
        let (authored_blocks, missed_blocks) = match <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_validator_participation(&api, best_hash, validator.clone()) {
            Ok((authored, missed)) => (authored, missed),
            Err(e) => {
                log::warn!("Failed to get block participation for validator {:?}: {:?}", validator, e);
                (0u32, 0u32)
            }
        };
        
        Ok(ValidatorProfile {
            account: validator,
            stake,
            pos_score,
            poi_score,
            trust_score,
            status,
            authored_blocks,
            missed_blocks,
        })
    }
    
    fn get_trust_score(&self, validator: AccountId) -> RpcResult<TrustScore> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get PoS component
        let pos_score = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_validator_score(&api, best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get PoS score: {:?}", e),
                None::<()>
            ))?;
        
        // Get PoI component
        let poi_score = match <C::Api as cbc_runtime::pallet_cbc_poi::PoiApi<Block, AccountId>>::get_inference_result(&api, best_hash, validator) {
            Ok(Some((_result, confidence))) => confidence as u64,
            Ok(None) => 0,
            Err(e) => {
                log::warn!("Failed to get PoI score: {:?}", e);
                0
            }
        };
        
        // Get consensus weights
        let (pos_weight, poi_weight) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_consensus_weights(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get consensus weights: {:?}", e),
                None::<()>
            ))?;
        
        // Calculate weighted components
        let pos_component = pos_score as u64 * pos_weight;
        let poi_component = poi_score * poi_weight;
        let total = (pos_component + poi_component) / (pos_weight + poi_weight);
        
        Ok(TrustScore {
            total,
            pos_component,
            poi_component,
        })
    }
    
    fn list_validators(&self) -> RpcResult<Vec<AccountId>> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get active validators from PoS pallet
        let active_validators = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_active_validators(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get active validators: {:?}", e),
                None::<()>
            ))?;
        
        Ok(active_validators)
    }
    
    fn get_status(&self) -> RpcResult<SystemStatus> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get current epoch
        let current_epoch = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_current_epoch(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get current epoch: {:?}", e),
                None::<()>
            ))?;
        
        // Get validator counts
        let active_validators = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_active_validators(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get active validators: {:?}", e),
                None::<()>
            ))?;
        
        let active_validators_count = active_validators.len() as u32;
        
        // Get total validators count from runtime
        let total_validators = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_total_validators_count(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get total validators count: {:?}", e),
                None::<()>
            ))?;
        
        // Get last finalized block
        let last_finalized_block = self.client.info().finalized_number as u32;
        
        // Determine consensus health
        let consensus_health = if active_validators_count >= 3 {
            ConsensusHealth::Healthy
        } else if active_validators_count >= 1 {
            ConsensusHealth::Degraded
        } else {
            ConsensusHealth::Critical
        };
        
        Ok(SystemStatus {
            current_epoch,
            active_validators: active_validators_count,
            total_validators,
            last_finalized_block,
            consensus_health,
        })
    }
    
    fn describe(&self) -> RpcResult<Vec<RpcMethodDescription>> {
        self.check_cbc_extensions_enabled()?;
        
        let methods = vec![
            // CBC Unified methods
            RpcMethodDescription {
                name: "cbc_getCurrentEpoch".to_string(),
                description: "Get the current epoch number".to_string(),
                params: vec![],
                returns: "u32".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_getValidatorProfile".to_string(),
                description: "Get comprehensive validator profile including stake, scores, and status".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "ValidatorProfile".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_getTrustScore".to_string(),
                description: "Get validator trust score with PoS and PoI components".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "TrustScore".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_listValidators".to_string(),
                description: "Get list of all active validators".to_string(),
                params: vec![],
                returns: "Vec<AccountId>".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_getStatus".to_string(),
                description: "Get system-wide status including epoch, validators, and health".to_string(),
                params: vec![],
                returns: "SystemStatus".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_describe".to_string(),
                description: "List all available CBC RPC methods".to_string(),
                params: vec![],
                returns: "Vec<RpcMethodDescription>".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_health".to_string(),
                description: "Get health check status for monitoring".to_string(),
                params: vec![],
                returns: "HealthStatus".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_getBlockAuthoringStats".to_string(),
                description: "Get block authoring statistics for a specific validator".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "BlockAuthoringStats".to_string(),
            },
            RpcMethodDescription {
                name: "cbc_getAllBlockAuthoringStats".to_string(),
                description: "Get block authoring statistics for all active validators".to_string(),
                params: vec![],
                returns: "Vec<(AccountId, BlockAuthoringStats)>".to_string(),
            },
            // PoS methods
            RpcMethodDescription {
                name: "pos_getValidatorScore".to_string(),
                description: "Get validator PoS performance score".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "u32".to_string(),
            },
            RpcMethodDescription {
                name: "pos_getValidatorStake".to_string(),
                description: "Get validator staked amount".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "Balance".to_string(),
            },
            RpcMethodDescription {
                name: "pos_getSlashingCount".to_string(),
                description: "Get number of times validator has been slashed".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "u32".to_string(),
            },
            RpcMethodDescription {
                name: "pos_getValidatorStatus".to_string(),
                description: "Get validator status (Active/Inactive/Slashed)".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "ValidatorStatus".to_string(),
            },
            // PoI methods
            RpcMethodDescription {
                name: "poi_getInferenceResult".to_string(),
                description: "Get validator inference result and confidence".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "Option<InferenceResult>".to_string(),
            },
            RpcMethodDescription {
                name: "poi_getInferenceConfidence".to_string(),
                description: "Get inference confidence score".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "Option<u32>".to_string(),
            },
            RpcMethodDescription {
                name: "poi_getChallengeWindow".to_string(),
                description: "Get current challenge window parameters".to_string(),
                params: vec![],
                returns: "ChallengeWindow".to_string(),
            },
            RpcMethodDescription {
                name: "poi_getInferenceStatus".to_string(),
                description: "Get inference submission status".to_string(),
                params: vec!["AccountId".to_string()],
                returns: "InferenceStatus".to_string(),
            },
            // DCF methods
            RpcMethodDescription {
                name: "dcf_getCurrentAuthor".to_string(),
                description: "Get current block author".to_string(),
                params: vec![],
                returns: "Option<AccountId>".to_string(),
            },
            RpcMethodDescription {
                name: "dcf_getExpectedAuthor".to_string(),
                description: "Get expected author for a specific block".to_string(),
                params: vec!["u32".to_string()],
                returns: "Option<AccountId>".to_string(),
            },
            RpcMethodDescription {
                name: "dcf_getValidatorScores".to_string(),
                description: "Get trust scores for all active validators".to_string(),
                params: vec![],
                returns: "Vec<(AccountId, u64)>".to_string(),
            },
            RpcMethodDescription {
                name: "dcf_getConsensusWeights".to_string(),
                description: "Get current PoS and PoI weight distribution".to_string(),
                params: vec![],
                returns: "ConsensusWeights".to_string(),
            },
            // Fork Detection methods
            RpcMethodDescription {
                name: "fork_checkPeers".to_string(),
                description: "Check for forks by comparing local and peer states".to_string(),
                params: vec!["Vec<String>".to_string(), "u32".to_string()],
                returns: "Vec<ForkReport>".to_string(),
            },
            RpcMethodDescription {
                name: "fork_getStatus".to_string(),
                description: "Get fork detection service status".to_string(),
                params: vec![],
                returns: "HashMap<String, Value>".to_string(),
            },
        ];
        
        Ok(methods)
    }
    
    fn health(&self) -> RpcResult<HealthStatus> {
        self.check_cbc_extensions_enabled()?;
        
        let mut issues = Vec::new();
        let mut is_healthy = true;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Check if we can get current epoch
        if let Err(e) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_current_epoch(&api, best_hash) {
            issues.push(format!("Cannot get current epoch: {:?}", e));
            is_healthy = false;
        }
        
        // Check if we have active validators
        match <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_active_validators(&api, best_hash) {
            Ok(validators) => {
                if validators.is_empty() {
                    issues.push("No active validators".to_string());
                    is_healthy = false;
                } else if validators.len() < 3 {
                    issues.push(format!("Low validator count: {}", validators.len()));
                    // Don't mark as unhealthy, just degraded
                }
            }
            Err(e) => {
                issues.push(format!("Cannot get active validators: {:?}", e));
                is_healthy = false;
            }
        }
        
        // Check consensus weights
        if let Err(e) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_consensus_weights(&api, best_hash) {
            issues.push(format!("Cannot get consensus weights: {:?}", e));
            is_healthy = false;
        }
        
        // Check if we're syncing
        let client_info = self.client.info();
        if client_info.best_number < client_info.finalized_number {
            issues.push("Node is behind finalized block".to_string());
            is_healthy = false;
        }
        
        Ok(HealthStatus {
            is_healthy,
            issues,
        })
    }
    
    fn get_block_authoring_stats(&self, validator: AccountId) -> RpcResult<BlockAuthoringStats> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get validator participation data
        let (authored_blocks, missed_blocks) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_validator_participation(&api, best_hash, validator.clone())
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get validator participation: {:?}", e),
                None::<()>
            ))?;
        
        let expected_blocks = authored_blocks + missed_blocks;
        let participation_rate = if expected_blocks > 0 {
            (authored_blocks as f64 / expected_blocks as f64) * 100.0
        } else {
            100.0
        };
        
        // Get validator state for additional info
        let consecutive_misses = if let Ok(Some(_profile)) = <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_validator_profile(&api, best_hash, validator.clone()) {
            // Calculate consecutive misses based on recent performance
            // This is a simplified calculation - in practice you'd track this more precisely
            if participation_rate < 90.0 && missed_blocks > 0 {
                std::cmp::min(missed_blocks, 5) // Cap at 5 for display
            } else {
                0
            }
        } else {
            0
        };
        
        let last_authored_block = if authored_blocks > 0 {
            // Get the last active block from validator state
            <C::Api as pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>>::get_validator_last_active(&api, best_hash, validator.clone())
                .ok()
                .filter(|&block| block > 0)
        } else {
            None
        };
        
        Ok(BlockAuthoringStats {
            authored_blocks,
            missed_blocks,
            expected_blocks,
            participation_rate,
            consecutive_misses,
            last_authored_block,
        })
    }
    
    fn get_all_block_authoring_stats(&self) -> RpcResult<Vec<(AccountId, BlockAuthoringStats)>> {
        self.check_cbc_extensions_enabled()?;
        
        let api = self.client.runtime_api();
        let best_hash = self.client.info().best_hash;
        
        // Get all active validators
        let active_validators = <C::Api as pallet_cbc_pos::PosApi<Block, AccountId, Balance>>::get_active_validators(&api, best_hash)
            .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                -32000,
                format!("Failed to get active validators: {:?}", e),
                None::<()>
            ))?;
        
        let mut results = Vec::new();
        
        for validator in active_validators {
            match self.get_block_authoring_stats(validator.clone()) {
                Ok(stats) => {
                    results.push((validator, stats));
                }
                Err(e) => {
                    log::warn!("Failed to get block authoring stats for validator {:?}: {:?}", validator, e);
                    // Continue with other validators instead of failing the entire request
                }
            }
        }
        
        // Sort by participation rate (highest first)
        results.sort_by(|a, b| b.1.participation_rate.partial_cmp(&a.1.participation_rate).unwrap_or(std::cmp::Ordering::Equal));
        
        Ok(results)
    }
}

// Fork Detection RPC API
#[rpc(server)]
pub trait ForkDetectionRpcApi {
    #[method(name = "fork_checkPeers")]
    async fn check_peers(&self, peer_endpoints: Vec<String>, threshold: u32) -> RpcResult<Vec<ForkReport>>;
    
    #[method(name = "fork_getStatus")]
    async fn get_status(&self) -> RpcResult<std::collections::HashMap<String, serde_json::Value>>;
}

pub struct ForkDetectionRpcApiImpl<C, B> {
    client: Arc<C>,
    security_config: RpcSecurityConfig,
    _backend: std::marker::PhantomData<B>,
}

impl<C, B> ForkDetectionRpcApiImpl<C, B> {
    pub fn new(client: Arc<C>, security_config: RpcSecurityConfig) -> Self {
        Self { 
            client, 
            security_config,
            _backend: std::marker::PhantomData,
        }
    }
    
    fn check_cbc_extensions_enabled(&self) -> RpcResult<()> {
        if !self.security_config.enable_cbc_extensions {
            return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32001,
                "CBC RPC extensions are disabled. Use --enable-cbc-extensions flag.".to_string(),
                None::<()>
            ));
        }
        Ok(())
    }
}

#[jsonrpsee::core::async_trait]
impl<C, B> ForkDetectionRpcApiServer for ForkDetectionRpcApiImpl<C, B>
where
    C: ProvideRuntimeApi<Block> + HeaderBackend<Block> + Send + Sync + 'static,
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
{
    async fn check_peers(&self, peer_endpoints: Vec<String>, threshold: u32) -> RpcResult<Vec<ForkReport>> {
        self.check_cbc_extensions_enabled()?;
        
        if peer_endpoints.is_empty() {
            return Err(jsonrpsee::types::ErrorObjectOwned::owned(
                -32602,
                "At least one peer endpoint must be provided".to_string(),
                None::<()>
            ));
        }
        
        // Use the standalone fork checker approach for RPC
        use jsonrpsee::{
            core::client::ClientT,
            http_client::HttpClientBuilder,
            rpc_params,
        };
        use std::time::Duration;
        use tokio::time::timeout;
        
        let timeout_duration = Duration::from_secs(30);
        let mut fork_reports = Vec::new();
        
        // Get local block info
        let local_info = self.client.info();
        let local_number = local_info.best_number;
        
        // Check each peer
        for (index, peer_rpc) in peer_endpoints.iter().enumerate() {
            match async {
                let peer_client = HttpClientBuilder::default()
                    .request_timeout(timeout_duration)
                    .build(peer_rpc)?;
                
                let peer_header: serde_json::Value = timeout(
                    timeout_duration,
                    peer_client.request("chain_getHeader", rpc_params![])
                ).await??;
                
                let peer_number = peer_header["number"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid peer block number format"))?;
                let peer_number = u32::from_str_radix(peer_number.trim_start_matches("0x"), 16)?;
                
                Ok::<u32, anyhow::Error>(peer_number)
            }.await {
                Ok(peer_number) => {
                    let divergence = if local_number > peer_number {
                        local_number - peer_number
                    } else {
                        peer_number - local_number
                    };
                    
                    let peer_id = format!("peer-{}", index);
                    
                    let report = ForkReport {
                        peer_id: peer_id.clone(),
                        local_best: local_number,
                        peer_best: peer_number,
                        divergence,
                    };
                    
                    if divergence > threshold {
                        log::warn!(
                            "WARNING: Fork detected with {} - Local: {}, Peer: {}, Divergence: {}",
                            peer_id, local_number, peer_number, divergence
                        );
                    }
                    
                    fork_reports.push(report);
                }
                Err(e) => {
                    log::error!("Failed to connect to peer {}: {}", peer_rpc, e);
                }
            }
        }
        
        Ok(fork_reports)
    }
    
    async fn get_status(&self) -> RpcResult<std::collections::HashMap<String, serde_json::Value>> {
        self.check_cbc_extensions_enabled()?;
        
        let mut status = std::collections::HashMap::new();
        status.insert("service".to_string(), serde_json::Value::String("fork-detection".to_string()));
        status.insert("version".to_string(), serde_json::Value::String(env!("CARGO_PKG_VERSION").to_string()));
        
        // Get local node info
        let client_info = self.client.info();
        status.insert("local_best_block".to_string(), serde_json::Value::Number(client_info.best_number.into()));
        status.insert("local_finalized_block".to_string(), serde_json::Value::Number(client_info.finalized_number.into()));
        status.insert("local_best_hash".to_string(), serde_json::Value::String(client_info.best_hash.to_string()));
        
        Ok(status)
    }
}

pub fn create_full<C, P, B>(
    deps: FullDeps<C, P, B>,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + Send + Sync + 'static,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Nonce>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BlockBuilder<Block>,
    C::Api: pallet_cbc_pos::PosApi<Block, AccountId, Balance>,
    C::Api: cbc_runtime::pallet_cbc_poi::PoiApi<Block, AccountId>,
    C::Api: pallet_cbc_dcf::DcfApi<Block, AccountId, Balance, u32>,
    P: TransactionPool + 'static,
    B: sc_client_api::Backend<Block> + Send + Sync + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    let mut module = RpcModule::new(());
    let FullDeps { client, pool, rpc_config, consensus_metrics, backend: _ } = deps;

    let _rate_limiter = RateLimiter::new(
        rpc_config.rate_limit_window,
        rpc_config.rate_limit_requests,
    );

    module.merge(System::new(client.clone(), pool).into_rpc())?;
    module.merge(TransactionPayment::new(client.clone()).into_rpc())?;

    if rpc_config.enable_cbc_extensions {
        let chain_api = ChainApiImpl { client: client.clone() };
        module.merge(ChainApiServer::into_rpc(chain_api))?;
        
        // Register PoS RPC handler
        let pos_api = if let Some(ref metrics) = consensus_metrics {
            PosRpcApiImpl::new_with_metrics(client.clone(), metrics.clone())
        } else {
            PosRpcApiImpl::new(client.clone())
        };
        module.merge(PosRpcApiServer::into_rpc(pos_api))?;
        
        // Register PoI RPC handler
        let poi_api = PoiRpcApiImpl::new(client.clone());
        module.merge(PoiRpcApiServer::into_rpc(poi_api))?;
        
        // Register DCF RPC handler
        let dcf_api = DcfRpcApiImpl::new(client.clone(), rpc_config.clone());
        module.merge(DcfRpcApiServer::into_rpc(dcf_api))?;
        
        // Register CBC Unified RPC handler
        let cbc_api = CbcRpcApiImpl::new(client.clone(), rpc_config.clone());
        module.merge(CbcRpcApiServer::into_rpc(cbc_api))?;
        
        // Register Fork Detection RPC handler
        let fork_api = ForkDetectionRpcApiImpl::<C, FullBackend>::new(client.clone(), rpc_config.clone());
        module.merge(ForkDetectionRpcApiServer::into_rpc(fork_api))?;
    }

    if rpc_config.expose_unsafe_methods {
    }

    Ok(module)
}

