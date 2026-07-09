use anyhow::{anyhow, Result};
use jsonrpsee::{
    core::client::ClientT,
    http_client::HttpClientBuilder,
    rpc_params,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;
use sc_client_api::Backend;
use sp_blockchain::HeaderBackend;
use cerulea_runtime::opaque::Block;

#[allow(dead_code)]
pub(crate) type FullClient = sc_service::TFullClient<
    Block,
    cerulea_runtime::apis::RuntimeApi,
    sc_executor::WasmExecutor<sp_io::SubstrateHostFunctions>,
>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkReport {
    pub peer_id: String,
    pub local_best: u32,
    pub peer_best: u32,
    pub divergence: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct BlockInfo {
    number: u32,
    hash: String,
    finalized_number: u32,
    finalized_hash: String,
}

#[allow(dead_code)]
pub struct ForkChecker<B> {
    client: Arc<FullClient>,
    _backend: std::marker::PhantomData<B>,
    timeout_duration: Duration,
}

#[allow(dead_code)]
impl<B> ForkChecker<B>
where
    B: Backend<Block>,
{
    pub fn new(client: Arc<FullClient>, timeout_seconds: u64) -> Self {
        Self {
            client,
            _backend: std::marker::PhantomData,
            timeout_duration: Duration::from_secs(timeout_seconds),
        }
    }
    
    pub async fn check_for_forks(&self, peer_rpc_endpoints: Vec<String>, threshold: u32) -> Result<Vec<ForkReport>> {
        let local_info = self.get_local_block_info()?;
        let mut fork_reports = Vec::new();
        
        for (index, peer_rpc) in peer_rpc_endpoints.iter().enumerate() {
            match self.get_peer_block_info(peer_rpc).await {
                Ok(peer_info) => {
                    let divergence = if local_info.number > peer_info.number {
                        local_info.number - peer_info.number
                    } else {
                        peer_info.number - local_info.number
                    };
                    
                    let peer_id = format!("peer-{}", index);
                    
                    let report = ForkReport {
                        peer_id: peer_id.clone(),
                        local_best: local_info.number,
                        peer_best: peer_info.number,
                        divergence,
                    };
                    
                    if divergence > threshold {
                        log::warn!(
                            "WARNING: Fork detected with {} - Local: {}, Peer: {}, Divergence: {}",
                            peer_id, local_info.number, peer_info.number, divergence
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
    
    fn get_local_block_info(&self) -> Result<BlockInfo> {
        let info = self.client.info();
        
        let best_number = info.best_number;
        let best_hash = info.best_hash.to_string();
        
        let finalized_number = info.finalized_number;
        let finalized_hash = info.finalized_hash.to_string();
        
        Ok(BlockInfo {
            number: best_number,
            hash: best_hash,
            finalized_number,
            finalized_hash,
        })
    }
    
    async fn get_peer_block_info(&self, rpc_url: &str) -> Result<BlockInfo> {
        let client = HttpClientBuilder::default()
            .request_timeout(self.timeout_duration)
            .build(rpc_url)?;
        
        // Get best block info
        let best_header: Value = timeout(
            self.timeout_duration,
            client.request("chain_getHeader", rpc_params![])
        ).await??;
        
        let best_number = best_header["number"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid block number format"))?;
        let best_number = u32::from_str_radix(best_number.trim_start_matches("0x"), 16)?;
        
        let best_hash = best_header["parentHash"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid block hash format"))?
            .to_string();
        
        // Get finalized block info
        let finalized_hash: String = timeout(
            self.timeout_duration,
            client.request("chain_getFinalizedHead", rpc_params![])
        ).await??;
        
        let finalized_header: Value = timeout(
            self.timeout_duration,
            client.request("chain_getHeader", rpc_params![finalized_hash.clone()])
        ).await??;
        
        let finalized_number = finalized_header["number"]
            .as_str()
            .ok_or_else(|| anyhow!("Invalid finalized block number format"))?;
        let finalized_number = u32::from_str_radix(finalized_number.trim_start_matches("0x"), 16)?;
        
        Ok(BlockInfo {
            number: best_number,
            hash: best_hash,
            finalized_number,
            finalized_hash,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_fork_report_creation() {
        let report = ForkReport {
            peer_id: "test-peer".to_string(),
            local_best: 100,
            peer_best: 95,
            divergence: 5,
        };
        
        assert_eq!(report.peer_id, "test-peer");
        assert_eq!(report.local_best, 100);
        assert_eq!(report.peer_best, 95);
        assert_eq!(report.divergence, 5);
    }
    
    #[test]
    fn test_block_info_creation() {
        let info = BlockInfo {
            number: 100,
            hash: "0x123".to_string(),
            finalized_number: 95,
            finalized_hash: "0x456".to_string(),
        };
        
        assert_eq!(info.number, 100);
        assert_eq!(info.hash, "0x123");
        assert_eq!(info.finalized_number, 95);
        assert_eq!(info.finalized_hash, "0x456");
    }
}