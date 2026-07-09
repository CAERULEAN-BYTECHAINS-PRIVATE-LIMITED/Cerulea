use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use jsonrpsee::{
    core::client::ClientT,
    http_client::HttpClientBuilder,
    rpc_params,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Parser)]
#[command(name = "fork-checker")]
#[command(about = "Cerulea Fork Detection Tool")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check for forks by comparing local and peer states
    Check(CheckArgs),
    /// Start RPC server for fork detection
    Serve(ServeArgs),
}

#[derive(Args)]
struct CheckArgs {
    /// Local node RPC endpoint
    #[arg(long, default_value = "http://127.0.0.1:9944")]
    local_rpc: String,
    
    /// Peer RPC endpoints (comma-separated)
    #[arg(long, value_delimiter = ',')]
    peer_rpc: Vec<String>,
    
    /// Divergence threshold for warnings
    #[arg(long, default_value = "10")]
    threshold: u32,
    
    /// Output format (json or plain)
    #[arg(long, default_value = "plain")]
    format: OutputFormat,
    
    /// Timeout for RPC calls in seconds
    #[arg(long, default_value = "30")]
    timeout: u64,
}

#[derive(Args)]
struct ServeArgs {
    /// Port to serve RPC on
    #[arg(long, default_value = "9945")]
    port: u16,
    
    /// Local node RPC endpoint
    #[arg(long, default_value = "http://127.0.0.1:9944")]
    local_rpc: String,
}

#[derive(Clone, Debug)]
enum OutputFormat {
    Json,
    Plain,
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "json" => Ok(OutputFormat::Json),
            "plain" => Ok(OutputFormat::Plain),
            _ => Err(format!("Invalid format: {}. Use 'json' or 'plain'", s)),
        }
    }
}

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

#[derive(Clone)]
pub struct ForkChecker {
    local_rpc: String,
    timeout_duration: Duration,
}

impl ForkChecker {
    pub fn new(local_rpc: String, timeout_seconds: u64) -> Self {
        Self {
            local_rpc,
            timeout_duration: Duration::from_secs(timeout_seconds),
        }
    }
    
    pub async fn check_for_forks(&self, peer_rpc_endpoints: Vec<String>, threshold: u32) -> Result<Vec<ForkReport>> {
        let local_info = self.get_block_info(&self.local_rpc).await?;
        let mut fork_reports = Vec::new();
        
        for (index, peer_rpc) in peer_rpc_endpoints.iter().enumerate() {
            match self.get_block_info(peer_rpc).await {
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
                        eprintln!(
                            "WARNING: Fork detected with {} - Local: {}, Peer: {}, Divergence: {}",
                            peer_id, local_info.number, peer_info.number, divergence
                        );
                    }
                    
                    fork_reports.push(report);
                }
                Err(e) => {
                    eprintln!("Failed to connect to peer {}: {}", peer_rpc, e);
                }
            }
        }
        
        Ok(fork_reports)
    }
    
    async fn get_block_info(&self, rpc_url: &str) -> Result<BlockInfo> {
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

async fn run_check(args: CheckArgs) -> Result<()> {
    if args.peer_rpc.is_empty() {
        return Err(anyhow!("At least one peer RPC endpoint must be provided"));
    }
    
    let checker = ForkChecker::new(args.local_rpc, args.timeout);
    let reports = checker.check_for_forks(args.peer_rpc, args.threshold).await?;
    
    match args.format {
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&reports)?);
        }
        OutputFormat::Plain => {
            println!("Fork Detection Report");
            println!("====================");
            println!("Threshold: {} blocks", args.threshold);
            println!();
            
            if reports.is_empty() {
                println!("No peers checked.");
                return Ok(());
            }
            
            let mut has_forks = false;
            for report in &reports {
                println!("Peer: {}", report.peer_id);
                println!("  Local Best:  {}", report.local_best);
                println!("  Peer Best:   {}", report.peer_best);
                println!("  Divergence:  {}", report.divergence);
                
                if report.divergence > args.threshold {
                    println!("  Status:      FORK DETECTED");
                    has_forks = true;
                } else {
                    println!("  Status:      OK");
                }
                println!();
            }
            
            if has_forks {
                println!("WARNING: One or more forks detected!");
            } else {
                println!("All peers are in sync within threshold.");
            }
        }
    }
    
    Ok(())
}

async fn run_serve(args: ServeArgs) -> Result<()> {
    use jsonrpsee::server::{ServerBuilder, ServerHandle};
    use jsonrpsee::RpcModule;
    
    let server = ServerBuilder::default()
        .build(format!("127.0.0.1:{}", args.port))
        .await?;
    
    let mut module = RpcModule::new(());
    let checker = ForkChecker::new(args.local_rpc, 30);
    
    // Add fork_checkPeers method
    let checker_clone = checker.clone();
    module.register_async_method("fork_checkPeers", move |params, _, _| {
        let checker = checker_clone.clone();
        async move {
            let (peer_endpoints, threshold): (Vec<String>, u32) = params.parse()?;
            checker.check_for_forks(peer_endpoints, threshold)
                .await
                .map_err(|e| jsonrpsee::types::ErrorObjectOwned::owned(
                    -32000,
                    e.to_string(),
                    None::<()>
                ))
        }
    })?;
    
    // Add fork_getStatus method
    let checker_clone = checker.clone();
    module.register_async_method("fork_getStatus", move |_params, _, _| {
        let checker = checker_clone.clone();
        async move {
            let mut status = HashMap::new();
            status.insert("service".to_string(), Value::String("fork-detection".to_string()));
            status.insert("version".to_string(), Value::String(env!("CARGO_PKG_VERSION").to_string()));
            status.insert("local_rpc".to_string(), Value::String(checker.local_rpc.clone()));
            Ok::<HashMap<String, Value>, jsonrpsee::types::ErrorObjectOwned>(status)
        }
    })?;
    
    let handle: ServerHandle = server.start(module);
    
    println!("Fork Detection RPC server started on port {}", args.port);
    println!("Available methods:");
    println!("  - fork_checkPeers(peer_endpoints: Vec<String>, threshold: u32) -> Vec<ForkReport>");
    println!("  - fork_getStatus() -> HashMap<String, Value>");
    println!();
    println!("Example curl command:");
    println!(r#"curl -H "Content-Type: application/json" -d '{{"jsonrpc":"2.0","method":"fork_getStatus","params":[],"id":1}}' http://127.0.0.1:{}"#, args.port);
    
    // Keep the server running
    handle.stopped().await;
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    let cli = Cli::parse();
    
    match cli.command {
        Commands::Check(args) => run_check(args).await,
        Commands::Serve(args) => run_serve(args).await,
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
    fn test_output_format_parsing() {
        assert!(matches!("json".parse::<OutputFormat>().unwrap(), OutputFormat::Json));
        assert!(matches!("plain".parse::<OutputFormat>().unwrap(), OutputFormat::Plain));
        assert!("invalid".parse::<OutputFormat>().is_err());
    }
    
    #[tokio::test]
    async fn test_fork_checker_creation() {
        let checker = ForkChecker::new("http://localhost:9944".to_string(), 30);
        assert_eq!(checker.local_rpc, "http://localhost:9944");
        assert_eq!(checker.timeout_duration, Duration::from_secs(30));
    }
}