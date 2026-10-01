use anyhow::{anyhow, Result};
use clap::{Args, Parser, Subcommand};
use jsonrpsee::RpcModule;
use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Parser)]
#[command(name = "fork-checker")]
#[command(about = "Cerulea Fork Detection Tool")]
struct ForkCheckArguments {
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

    pub async fn check_for_forks(
        &self,
        peer_rpc_endpoints: Vec<String>,
        threshold: u32,
    ) -> Result<Vec<ForkReport>> {
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
            client.request("chain_getHeader", rpc_params![]),
        )
        .await??;

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
            client.request("chain_getFinalizedHead", rpc_params![]),
        )
        .await??;

        let finalized_header: Value = timeout(
            self.timeout_duration,
            client.request("chain_getHeader", rpc_params![finalized_hash.clone()]),
        )
        .await??;

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
    let reports = checker
        .check_for_forks(args.peer_rpc, args.threshold)
        .await?;

    print!(
        "{}",
        render_check_report(&reports, args.threshold, &args.format)?
    );

    Ok(())
}

/// Renders the fork reports in the requested output format.
fn render_check_report(
    reports: &[ForkReport],
    threshold: u32,
    format: &OutputFormat,
) -> Result<String> {
    let mut out = String::new();

    match format {
        OutputFormat::Json => {
            writeln!(out, "{}", serde_json::to_string_pretty(reports)?)?;
        }
        OutputFormat::Plain => {
            writeln!(out, "Fork Detection Report")?;
            writeln!(out, "====================")?;
            writeln!(out, "Threshold: {} blocks", threshold)?;
            writeln!(out)?;

            if reports.is_empty() {
                writeln!(out, "No peers checked.")?;
                return Ok(out);
            }

            let mut has_forks = false;
            for report in reports {
                writeln!(out, "Peer: {}", report.peer_id)?;
                writeln!(out, "  Local Best:  {}", report.local_best)?;
                writeln!(out, "  Peer Best:   {}", report.peer_best)?;
                writeln!(out, "  Divergence:  {}", report.divergence)?;

                if report.divergence > threshold {
                    writeln!(out, "  Status:      FORK DETECTED")?;
                    has_forks = true;
                } else {
                    writeln!(out, "  Status:      OK")?;
                }
                writeln!(out)?;
            }

            if has_forks {
                writeln!(out, "WARNING: One or more forks detected!")?;
            } else {
                writeln!(out, "All peers are in sync within threshold.")?;
            }
        }
    }

    Ok(out)
}

async fn run_serve(args: ServeArgs) -> Result<()> {
    use jsonrpsee::server::{ServerBuilder, ServerHandle};

    let server = ServerBuilder::default()
        .build(format!("127.0.0.1:{}", args.port))
        .await?;

    let module = build_rpc_module(ForkChecker::new(args.local_rpc, 30))?;

    let handle: ServerHandle = server.start(module);

    println!("Fork Detection RPC server started on port {}", args.port);
    println!("Available methods:");
    println!("  - fork_checkPeers(peer_endpoints: Vec<String>, threshold: u32) -> Vec<ForkReport>");
    println!("  - fork_getStatus() -> HashMap<String, Value>");
    println!();
    println!("Example curl command:");
    println!(
        r#"curl -H "Content-Type: application/json" -d '{{"jsonrpc":"2.0","method":"fork_getStatus","params":[],"id":1}}' http://127.0.0.1:{}"#,
        args.port
    );

    // Keep the server running
    handle.stopped().await;

    Ok(())
}

/// Builds the RPC module that serves fork_checkPeers and fork_getStatus.
fn build_rpc_module(checker: ForkChecker) -> Result<RpcModule<()>> {
    let mut module = RpcModule::new(());

    // Add fork_checkPeers method
    let checker_clone = checker.clone();
    module.register_async_method("fork_checkPeers", move |params, _, _| {
        let checker = checker_clone.clone();
        async move {
            let (peer_endpoints, threshold): (Vec<String>, u32) = params.parse()?;
            checker
                .check_for_forks(peer_endpoints, threshold)
                .await
                .map_err(|e| {
                    jsonrpsee::types::ErrorObjectOwned::owned(-32000, e.to_string(), None::<()>)
                })
        }
    })?;

    // Add fork_getStatus method
    let checker_clone = checker.clone();
    module.register_async_method("fork_getStatus", move |_params, _, _| {
        let checker = checker_clone.clone();
        async move {
            let mut status = HashMap::new();
            status.insert(
                "service".to_string(),
                Value::String("fork-detection".to_string()),
            );
            status.insert(
                "version".to_string(),
                Value::String(env!("CARGO_PKG_VERSION").to_string()),
            );
            status.insert(
                "local_rpc".to_string(),
                Value::String(checker.local_rpc.clone()),
            );
            Ok::<HashMap<String, Value>, jsonrpsee::types::ErrorObjectOwned>(status)
        }
    })?;

    Ok(module)
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let arguments = ForkCheckArguments::parse();

    match arguments.command {
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
        assert!(matches!(
            "json".parse::<OutputFormat>().unwrap(),
            OutputFormat::Json
        ));
        assert!(matches!(
            "plain".parse::<OutputFormat>().unwrap(),
            OutputFormat::Plain
        ));
        assert!("invalid".parse::<OutputFormat>().is_err());
    }

    #[tokio::test]
    async fn test_fork_checker_creation() {
        let checker = ForkChecker::new("http://localhost:9944".to_string(), 30);
        assert_eq!(checker.local_rpc, "http://localhost:9944");
        assert_eq!(checker.timeout_duration, Duration::from_secs(30));
    }

    use clap::error::ErrorKind;
    use clap::CommandFactory;
    use jsonrpsee::core::ClientError;
    use jsonrpsee::server::{Server, ServerHandle};
    use jsonrpsee::types::ErrorObjectOwned;
    use serde_json::json;

    // The headers a test node serves for its best block and its finalized block.
    struct ChainNode {
        best_header: Value,
        finalized_head: Value,
        finalized_header: Value,
    }

    fn block_hash(number: u32) -> String {
        format!("0x{:064x}", number)
    }

    fn hex_number(number: u32) -> Value {
        Value::String(format!("0x{:x}", number))
    }

    fn header(number: Value, parent_hash: Option<String>) -> Value {
        let mut header = json!({
            "number": number,
            "stateRoot": block_hash(0x5157),
            "extrinsicsRoot": block_hash(0xe471),
            "digest": { "logs": [] }
        });
        if let Some(parent) = parent_hash {
            header["parentHash"] = Value::String(parent);
        }
        header
    }

    fn healthy_node(best: u32, finalized: u32) -> ChainNode {
        ChainNode {
            best_header: header(hex_number(best), Some(block_hash(best - 1))),
            finalized_head: Value::String(block_hash(finalized)),
            finalized_header: header(hex_number(finalized), Some(block_hash(finalized - 1))),
        }
    }

    // Starts a real RPC server that answers the two chain methods the checker calls.
    async fn start_chain_node(node: ChainNode) -> (String, ServerHandle) {
        let server = Server::builder()
            .build("127.0.0.1:0")
            .await
            .expect("test node binds a local port");
        let addr = server.local_addr().expect("test node reports its address");
        let mut module = RpcModule::new(node);
        module
            .register_method(
                "chain_getHeader",
                |params, node, _| -> Result<Value, ErrorObjectOwned> {
                    let mut sequence = params.sequence();
                    let requested: Option<String> = sequence.optional_next()?;
                    match requested {
                        None => Ok(node.best_header.clone()),
                        Some(hash) if node.finalized_head.as_str() == Some(hash.as_str()) => {
                            Ok(node.finalized_header.clone())
                        }
                        Some(hash) => Err(ErrorObjectOwned::owned(
                            -32602,
                            format!("Unknown block {hash}"),
                            None::<()>,
                        )),
                    }
                },
            )
            .expect("method name is unique");
        module
            .register_method("chain_getFinalizedHead", |_, node, _| {
                node.finalized_head.clone()
            })
            .expect("method name is unique");
        (format!("http://{addr}"), server.start(module))
    }

    // Starts a node that answers chain_getHeader only after the given delay.
    async fn start_slow_node(delay: Duration) -> (String, ServerHandle) {
        let server = Server::builder()
            .build("127.0.0.1:0")
            .await
            .expect("test node binds a local port");
        let addr = server.local_addr().expect("test node reports its address");
        let mut module = RpcModule::new(delay);
        module
            .register_async_method("chain_getHeader", |_, delay, _| async move {
                tokio::time::sleep(*delay).await;
                Ok::<Value, ErrorObjectOwned>(header(hex_number(1), Some(block_hash(0))))
            })
            .expect("method name is unique");
        (format!("http://{addr}"), server.start(module))
    }

    // Serves the fork module itself on a free local port.
    async fn serve_fork_module(local_rpc: String) -> (String, ServerHandle) {
        let server = Server::builder()
            .build("127.0.0.1:0")
            .await
            .expect("fork server binds a local port");
        let addr = server
            .local_addr()
            .expect("fork server reports its address");
        let module = build_rpc_module(ForkChecker::new(local_rpc, 5)).expect("module builds");
        (format!("http://{addr}"), server.start(module))
    }

    // Returns a local port with no listener behind it.
    fn unused_local_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port binds");
        listener
            .local_addr()
            .expect("listener has an address")
            .port()
    }

    fn unreachable_url() -> String {
        format!("http://127.0.0.1:{}", unused_local_port())
    }

    fn report(peer_id: &str, local_best: u32, peer_best: u32) -> ForkReport {
        ForkReport {
            peer_id: peer_id.to_string(),
            local_best,
            peer_best,
            divergence: local_best.abs_diff(peer_best),
        }
    }

    fn check_args(argv: &[&str]) -> CheckArgs {
        match ForkCheckArguments::try_parse_from(argv.iter().copied()).map(|a| a.command) {
            Ok(Commands::Check(args)) => args,
            Ok(Commands::Serve(_)) => panic!("the serve subcommand parsed in place of check"),
            Err(error) => panic!("arguments failed to parse: {error}"),
        }
    }

    fn serve_args(argv: &[&str]) -> ServeArgs {
        match ForkCheckArguments::try_parse_from(argv.iter().copied()).map(|a| a.command) {
            Ok(Commands::Serve(args)) => args,
            Ok(Commands::Check(_)) => panic!("the check subcommand parsed in place of serve"),
            Err(error) => panic!("arguments failed to parse: {error}"),
        }
    }

    fn parse_error(argv: &[&str]) -> clap::Error {
        ForkCheckArguments::try_parse_from(argv.iter().copied())
            .err()
            .expect("arguments are rejected")
    }

    #[test]
    fn argument_definition_passes_clap_checks() {
        ForkCheckArguments::command().debug_assert();
    }

    #[test]
    fn check_subcommand_uses_documented_defaults() {
        let args = check_args(&["fork-checker", "check"]);
        assert_eq!(args.local_rpc, "http://127.0.0.1:9944");
        assert!(args.peer_rpc.is_empty());
        assert_eq!(args.threshold, 10);
        assert!(matches!(args.format, OutputFormat::Plain));
        assert_eq!(args.timeout, 30);
    }

    #[test]
    fn check_subcommand_splits_peer_list_on_commas() {
        let args = check_args(&[
            "fork-checker",
            "check",
            "--local-rpc",
            "http://10.0.0.1:9944",
            "--peer-rpc",
            "http://10.0.0.2:9944,http://10.0.0.3:9944",
            "--peer-rpc",
            "http://10.0.0.4:9944",
            "--threshold",
            "4",
            "--format",
            "JSON",
            "--timeout",
            "8",
        ]);
        assert_eq!(args.local_rpc, "http://10.0.0.1:9944");
        assert_eq!(
            args.peer_rpc,
            [
                "http://10.0.0.2:9944",
                "http://10.0.0.3:9944",
                "http://10.0.0.4:9944"
            ]
        );
        assert_eq!(args.threshold, 4);
        assert!(matches!(args.format, OutputFormat::Json));
        assert_eq!(args.timeout, 8);
    }

    #[test]
    fn check_subcommand_rejects_unknown_format() {
        let error = parse_error(&["fork-checker", "check", "--format", "yaml"]);
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn check_subcommand_rejects_text_threshold() {
        let error = parse_error(&["fork-checker", "check", "--threshold", "ten"]);
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn serve_subcommand_uses_documented_defaults() {
        let args = serve_args(&["fork-checker", "serve"]);
        assert_eq!(args.port, 9945);
        assert_eq!(args.local_rpc, "http://127.0.0.1:9944");
    }

    #[test]
    fn serve_subcommand_reads_port_and_endpoint() {
        let args = serve_args(&[
            "fork-checker",
            "serve",
            "--port",
            "9950",
            "--local-rpc",
            "http://10.0.0.9:9944",
        ]);
        assert_eq!(args.port, 9950);
        assert_eq!(args.local_rpc, "http://10.0.0.9:9944");
    }

    #[test]
    fn serve_subcommand_rejects_port_above_range() {
        let error = parse_error(&["fork-checker", "serve", "--port", "70000"]);
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn missing_subcommand_is_rejected() {
        assert!(ForkCheckArguments::try_parse_from(["fork-checker"]).is_err());
    }

    #[test]
    fn unknown_subcommand_is_rejected() {
        let error = parse_error(&["fork-checker", "repair"]);
        assert_eq!(error.kind(), ErrorKind::InvalidSubcommand);
    }

    #[test]
    fn output_format_ignores_letter_case() {
        assert!(matches!(
            "JSON".parse::<OutputFormat>(),
            Ok(OutputFormat::Json)
        ));
        assert!(matches!(
            "Plain".parse::<OutputFormat>(),
            Ok(OutputFormat::Plain)
        ));
    }

    #[test]
    fn output_format_error_names_the_accepted_values() {
        let error = "yaml"
            .parse::<OutputFormat>()
            .expect_err("yaml is an unsupported format");
        assert_eq!(error, "Invalid format: yaml. Use 'json' or 'plain'");
    }

    #[test]
    fn plain_report_flags_divergence_above_threshold() {
        let reports = [report("peer-0", 500, 500), report("peer-2", 500, 480)];
        let text = render_check_report(&reports, 10, &OutputFormat::Plain).expect("report renders");
        let expected = concat!(
            "Fork Detection Report\n",
            "====================\n",
            "Threshold: 10 blocks\n",
            "\n",
            "Peer: peer-0\n",
            "  Local Best:  500\n",
            "  Peer Best:   500\n",
            "  Divergence:  0\n",
            "  Status:      OK\n",
            "\n",
            "Peer: peer-2\n",
            "  Local Best:  500\n",
            "  Peer Best:   480\n",
            "  Divergence:  20\n",
            "  Status:      FORK DETECTED\n",
            "\n",
            "WARNING: One or more forks detected!\n",
        );
        assert_eq!(text, expected);
    }

    #[test]
    fn plain_report_treats_divergence_equal_to_threshold_as_in_sync() {
        let reports = [report("peer-0", 600, 590)];
        let text = render_check_report(&reports, 10, &OutputFormat::Plain).expect("report renders");
        assert!(text.contains("  Divergence:  10\n  Status:      OK\n"));
        assert!(text.ends_with("All peers are in sync within threshold.\n"));
        assert!(!text.contains("FORK DETECTED"));
    }

    #[test]
    fn plain_report_states_when_no_peer_answered() {
        let text = render_check_report(&[], 7, &OutputFormat::Plain).expect("report renders");
        assert_eq!(
            text,
            "Fork Detection Report\n====================\nThreshold: 7 blocks\n\nNo peers checked.\n"
        );
    }

    #[test]
    fn json_report_round_trips() {
        let reports = [report("peer-0", 900, 915), report("peer-1", 900, 899)];
        let text = render_check_report(&reports, 10, &OutputFormat::Json).expect("report renders");
        let parsed: Vec<ForkReport> = serde_json::from_str(&text).expect("output is valid JSON");
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].peer_id, "peer-0");
        assert_eq!(parsed[0].peer_best, 915);
        assert_eq!(parsed[0].divergence, 15);
        assert_eq!(parsed[1].local_best, 900);
        assert_eq!(parsed[1].divergence, 1);
    }

    #[test]
    fn json_report_for_no_peers_is_an_empty_array() {
        let text = render_check_report(&[], 10, &OutputFormat::Json).expect("report renders");
        assert_eq!(text, "[]\n");
    }

    #[tokio::test]
    async fn block_info_reads_best_and_finalized_heights() {
        let (url, _node) = start_chain_node(healthy_node(500, 496)).await;
        let checker = ForkChecker::new(url.clone(), 5);
        let info = checker.get_block_info(&url).await.expect("node answers");
        assert_eq!(info.number, 500);
        // The checker records the parent hash of the best header in this field.
        assert_eq!(info.hash, block_hash(499));
        assert_eq!(info.finalized_number, 496);
        assert_eq!(info.finalized_hash, block_hash(496));
    }

    #[tokio::test]
    async fn block_info_rejects_numeric_block_number() {
        let mut node = healthy_node(500, 496);
        node.best_header = header(json!(500), Some(block_hash(499)));
        let (url, _node) = start_chain_node(node).await;
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a numeric height fails");
        assert_eq!(error.to_string(), "Invalid block number format");
    }

    #[tokio::test]
    async fn block_info_rejects_header_without_parent_hash() {
        let mut node = healthy_node(500, 496);
        node.best_header = header(hex_number(500), None);
        let (url, _node) = start_chain_node(node).await;
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a header without a parent hash fails");
        assert_eq!(error.to_string(), "Invalid block hash format");
    }

    #[tokio::test]
    async fn block_info_rejects_finalized_header_without_number() {
        let mut node = healthy_node(500, 496);
        node.finalized_header = json!({ "parentHash": block_hash(495) });
        let (url, _node) = start_chain_node(node).await;
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a finalized header without a number fails");
        assert_eq!(error.to_string(), "Invalid finalized block number format");
    }

    #[tokio::test]
    async fn block_info_rejects_height_that_is_not_hexadecimal() {
        let mut node = healthy_node(500, 496);
        node.best_header = header(json!("0xzz"), Some(block_hash(499)));
        let (url, _node) = start_chain_node(node).await;
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a height with letters past f fails");
        assert!(error.downcast_ref::<std::num::ParseIntError>().is_some());
    }

    #[tokio::test]
    async fn block_info_rejects_finalized_head_of_wrong_type() {
        let mut node = healthy_node(500, 496);
        node.finalized_head = json!(496);
        let (url, _node) = start_chain_node(node).await;
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a numeric finalized head fails to decode as a hash");
        assert!(error.downcast_ref::<ClientError>().is_some());
    }

    #[tokio::test]
    async fn block_info_reports_unreachable_node() {
        let url = unreachable_url();
        let error = ForkChecker::new(url.clone(), 5)
            .get_block_info(&url)
            .await
            .expect_err("a closed port refuses the connection");
        assert!(matches!(
            error.downcast_ref::<ClientError>(),
            Some(ClientError::Transport(_))
        ));
    }

    #[tokio::test]
    async fn block_info_times_out_on_slow_node() {
        let (url, _node) = start_slow_node(Duration::from_secs(2)).await;
        let checker = ForkChecker {
            local_rpc: url.clone(),
            timeout_duration: Duration::from_millis(200),
        };
        let error = checker
            .get_block_info(&url)
            .await
            .expect_err("a slow node exceeds the timeout");
        let client_timeout = matches!(
            error.downcast_ref::<ClientError>(),
            Some(ClientError::RequestTimeout)
        );
        let outer_timeout = error
            .downcast_ref::<tokio::time::error::Elapsed>()
            .is_some();
        assert!(client_timeout || outer_timeout, "unexpected error: {error}");
    }

    #[tokio::test]
    async fn check_for_forks_measures_divergence_in_both_directions() {
        let (local, _local_node) = start_chain_node(healthy_node(500, 496)).await;
        let (level_peer, _level_node) = start_chain_node(healthy_node(500, 497)).await;
        let (behind_peer, _behind_node) = start_chain_node(healthy_node(480, 476)).await;
        let (ahead_peer, _ahead_node) = start_chain_node(healthy_node(515, 510)).await;

        let checker = ForkChecker::new(local, 5);
        let reports = checker
            .check_for_forks(
                vec![level_peer, unreachable_url(), behind_peer, ahead_peer],
                10,
            )
            .await
            .expect("local node answers");

        // The unreachable peer at index 1 is skipped. Peer numbering keeps the input index.
        let ids: Vec<&str> = reports.iter().map(|r| r.peer_id.as_str()).collect();
        assert_eq!(ids, ["peer-0", "peer-2", "peer-3"]);
        assert!(reports.iter().all(|r| r.local_best == 500));
        let peer_best: Vec<u32> = reports.iter().map(|r| r.peer_best).collect();
        let divergence: Vec<u32> = reports.iter().map(|r| r.divergence).collect();
        assert_eq!(peer_best, [500, 480, 515]);
        assert_eq!(divergence, [0, 20, 15]);
    }

    #[tokio::test]
    async fn check_for_forks_with_no_peers_returns_empty_list() {
        let (local, _local_node) = start_chain_node(healthy_node(320, 318)).await;
        let reports = ForkChecker::new(local, 5)
            .check_for_forks(Vec::new(), 10)
            .await
            .expect("local node answers");
        assert!(reports.is_empty());
    }

    #[tokio::test]
    async fn check_for_forks_fails_when_local_node_is_unreachable() {
        let (peer, _peer_node) = start_chain_node(healthy_node(320, 318)).await;
        let error = ForkChecker::new(unreachable_url(), 5)
            .check_for_forks(vec![peer], 10)
            .await
            .expect_err("the local node is required");
        assert!(matches!(
            error.downcast_ref::<ClientError>(),
            Some(ClientError::Transport(_))
        ));
    }

    #[tokio::test]
    async fn run_check_requires_at_least_one_peer() {
        let args = check_args(&["fork-checker", "check"]);
        let error = run_check(args).await.expect_err("an empty peer list fails");
        assert_eq!(
            error.to_string(),
            "At least one peer RPC endpoint must be provided"
        );
    }

    #[tokio::test]
    async fn run_check_prints_both_formats() {
        let (local, _local_node) = start_chain_node(healthy_node(700, 690)).await;
        let (near, _near_node) = start_chain_node(healthy_node(702, 690)).await;
        let (far, _far_node) = start_chain_node(healthy_node(650, 640)).await;
        let peers = format!("{near},{far}");
        for format in ["plain", "json"] {
            let args = check_args(&[
                "fork-checker",
                "check",
                "--local-rpc",
                local.as_str(),
                "--peer-rpc",
                peers.as_str(),
                "--format",
                format,
                "--timeout",
                "5",
            ]);
            run_check(args).await.expect("check succeeds");
        }
    }

    #[tokio::test]
    async fn run_check_succeeds_when_every_peer_is_unreachable() {
        let (local, _local_node) = start_chain_node(healthy_node(700, 690)).await;
        let peer = unreachable_url();
        let args = check_args(&[
            "fork-checker",
            "check",
            "--local-rpc",
            local.as_str(),
            "--peer-rpc",
            peer.as_str(),
            "--timeout",
            "5",
        ]);
        run_check(args)
            .await
            .expect("unreachable peers are reported and skipped");
    }

    #[tokio::test]
    async fn run_check_fails_when_local_node_is_unreachable() {
        let local = unreachable_url();
        let peer = unreachable_url();
        let args = check_args(&[
            "fork-checker",
            "check",
            "--local-rpc",
            local.as_str(),
            "--peer-rpc",
            peer.as_str(),
            "--timeout",
            "5",
        ]);
        assert!(run_check(args).await.is_err());
    }

    #[tokio::test]
    async fn served_status_reports_service_details() {
        let (url, _server) = serve_fork_module("http://10.0.0.7:9944".to_string()).await;
        let client = HttpClientBuilder::default()
            .build(&url)
            .expect("client builds");
        let status: HashMap<String, Value> = client
            .request("fork_getStatus", rpc_params![])
            .await
            .expect("status call succeeds");
        assert_eq!(status.len(), 3);
        assert_eq!(status["service"], "fork-detection");
        assert_eq!(status["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(status["local_rpc"], "http://10.0.0.7:9944");
    }

    #[tokio::test]
    async fn served_check_peers_returns_reports() {
        let (local, _local_node) = start_chain_node(healthy_node(800, 795)).await;
        let (peer, _peer_node) = start_chain_node(healthy_node(812, 800)).await;
        let (url, _server) = serve_fork_module(local).await;
        let client = HttpClientBuilder::default()
            .build(&url)
            .expect("client builds");

        let reports: Vec<ForkReport> = client
            .request("fork_checkPeers", rpc_params![vec![peer], 10u32])
            .await
            .expect("check call succeeds");

        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].peer_id, "peer-0");
        assert_eq!(reports[0].local_best, 800);
        assert_eq!(reports[0].peer_best, 812);
        assert_eq!(reports[0].divergence, 12);
    }

    #[tokio::test]
    async fn served_check_peers_maps_failure_to_server_error() {
        let (url, _server) = serve_fork_module(unreachable_url()).await;
        let client = HttpClientBuilder::default()
            .build(&url)
            .expect("client builds");

        let result: Result<Vec<ForkReport>, ClientError> = client
            .request(
                "fork_checkPeers",
                rpc_params![vec![unreachable_url()], 10u32],
            )
            .await;

        match result {
            Err(ClientError::Call(object)) => assert_eq!(object.code(), -32000),
            other => panic!("a call error with code -32000 was expected, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn served_check_peers_rejects_malformed_params() {
        let (url, _server) = serve_fork_module(unreachable_url()).await;
        let client = HttpClientBuilder::default()
            .build(&url)
            .expect("client builds");

        let result: Result<Vec<ForkReport>, ClientError> = client
            .request("fork_checkPeers", rpc_params!["http://10.0.0.2:9944"])
            .await;

        match result {
            Err(ClientError::Call(object)) => assert_eq!(object.code(), -32602),
            other => panic!("an invalid params error was expected, got {other:?}"),
        }
    }

    async fn poll_status(port: u16) -> HashMap<String, Value> {
        let client = HttpClientBuilder::default()
            .build(format!("http://127.0.0.1:{port}"))
            .expect("client builds");
        for _ in 0..100 {
            let response: Result<HashMap<String, Value>, ClientError> =
                client.request("fork_getStatus", rpc_params![]).await;
            if let Ok(status) = response {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("the fork server gave no answer on port {port}");
    }

    #[tokio::test]
    async fn run_serve_answers_status_while_running() {
        let port = unused_local_port();
        let serve = run_serve(ServeArgs {
            port,
            local_rpc: "http://10.0.0.8:9944".to_string(),
        });
        tokio::pin!(serve);

        let status = tokio::select! {
            result = &mut serve => panic!("the fork server stopped early: {result:?}"),
            status = poll_status(port) => status,
        };

        assert_eq!(status["service"], "fork-detection");
        assert_eq!(status["local_rpc"], "http://10.0.0.8:9944");
    }

    #[tokio::test]
    async fn run_serve_fails_when_port_is_taken() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port binds");
        let port = listener
            .local_addr()
            .expect("listener has an address")
            .port();

        let outcome = timeout(
            Duration::from_secs(5),
            run_serve(ServeArgs {
                port,
                local_rpc: "http://127.0.0.1:9944".to_string(),
            }),
        )
        .await
        .expect("a bind failure returns at once");

        let error = outcome.expect_err("the port is already in use");
        assert_eq!(
            error.downcast_ref::<std::io::Error>().map(|e| e.kind()),
            Some(std::io::ErrorKind::AddrInUse)
        );
        drop(listener);
    }
}
