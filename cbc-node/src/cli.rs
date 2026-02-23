use sc_cli::RunCmd;
use clap::Parser;
use clap::Args;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "cbc-node",
    about = "CBC Chain Node - A Substrate-based blockchain node.",
    author = "Caerulean ByteChains Private Limited",
    version = env!("CARGO_PKG_VERSION")
)]
pub struct Cli {
    #[command(subcommand)]
    pub subcommand: Option<Subcommand>,

    #[clap(flatten)]
    pub run: RunCmd,

    #[clap(long, default_value = "production", help = "CBC custom mode (e.g., testing, production)")]
    pub cbc_mode: String,

    #[clap(long, help = "Enable CBC custom RPC extensions")]
    pub enable_cbc_extensions: bool,
    
    #[clap(long, help = "Write logs to the given file instead of stdout")]
    pub log_file: Option<String>,

    #[clap(long, help = "Show only CBC-related logs")]
    pub cbc_log_only: bool,

    #[clap(long, help = "Minimal output - suppress startup info and reduce logging")]
    pub quiet: bool,

    #[clap(long, help = "Enable unsafe RPC methods (use with caution)")]
    pub unsafe_rpc_expose: bool,

    #[clap(long, default_value = "60", help = "RPC rate limiting window in seconds")]
    pub rpc_rate_limit_window: u64,

    #[clap(long, default_value = "100", help = "Maximum RPC requests per rate limit window")]
    pub rpc_rate_limit_requests: u32,

    #[clap(long, help = "Enable lifecycle tracing for detailed node initialization and operation logging")]
    pub lifecycle_trace: bool,

    #[clap(long, help = "Lifecycle trace output format (json or human-readable)")]
    pub lifecycle_trace_format: Option<String>,

    #[clap(long, help = "Write lifecycle traces to the specified file")]
    pub lifecycle_trace_output: Option<String>,

    #[clap(long, help = "Only log milestone events in lifecycle tracing")]
    pub lifecycle_trace_milestone_only: bool,
}

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum Subcommand {
    #[command(subcommand)]
    Key(sc_cli::KeySubcommand),

    BuildSpec(sc_cli::BuildSpecCmd),

    CheckBlock(sc_cli::CheckBlockCmd),

    ExportBlocks(sc_cli::ExportBlocksCmd),

    ExportState(sc_cli::ExportStateCmd),

    ImportBlocks(sc_cli::ImportBlocksCmd),

    PurgeChain(sc_cli::PurgeChainCmd),

    Revert(sc_cli::RevertCmd),

    Faucet(FaucetCmd),

    #[command(subcommand)]
    Benchmark(frame_benchmarking_cli::BenchmarkCmd),

    ChainInfo(sc_cli::ChainInfoCmd),

    /// Display node information
    Info(InfoCmd),

    /// Display node health status
    Health(HealthCmd),

    /// Perform runtime upgrade
    RuntimeUpgrade(RuntimeUpgradeCmd),

    /// Fork detection tool
    ForkCheck(ForkCheckCmd),

    /// Query upcoming block authors
    QueryAuthors(QueryAuthorsCmd),
}

#[derive(Debug, Args)]
pub struct FaucetCmd {
    #[clap(long)]
    pub to: String,

    #[clap(long)]
    pub amount: u128,
}

/// Output format for CLI commands
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum OutputFormat {
    /// JSON format
    Json,
    /// Plain text format
    Plain,
}

impl Default for OutputFormat {
    fn default() -> Self {
        OutputFormat::Plain
    }
}

/// Display node information
#[derive(Debug, Args)]
pub struct InfoCmd {
    /// Output format (json or plain)
    #[clap(long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
}

/// Display node health status
#[derive(Debug, Args)]
pub struct HealthCmd {
    /// Output format (json or plain)
    #[clap(long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
}

/// Perform runtime upgrade
#[derive(Debug, Args)]
pub struct RuntimeUpgradeCmd {
    /// Path to new runtime WASM file
    #[clap(long)]
    pub wasm: PathBuf,
    
    /// Poll interval in seconds
    #[clap(long, default_value = "6")]
    pub poll_interval: u64,
    
    /// Output format (json or plain)
    #[clap(long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
}

/// Fork detection tool
#[derive(Debug, Args)]
pub struct ForkCheckCmd {
    /// Local node RPC endpoint
    #[clap(long, default_value = "http://127.0.0.1:9944")]
    pub local_rpc: String,
    
    /// Peer RPC endpoints (comma-separated)
    #[clap(long, value_delimiter = ',')]
    pub peer_rpc: Vec<String>,
    
    /// Divergence threshold for warnings
    #[clap(long, default_value = "10")]
    pub threshold: u32,
    
    /// Request timeout in seconds
    #[clap(long, default_value = "30")]
    pub timeout: u64,
    
    /// Output format (json or plain)
    #[clap(long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
}

/// Query upcoming block authors
#[derive(Debug, Args)]
pub struct QueryAuthorsCmd {
    /// Number of future blocks to query
    #[clap(long, short = 'n', default_value = "10")]
    pub blocks: u32,
    
    /// RPC endpoint to query
    #[clap(long, default_value = "http://127.0.0.1:9944")]
    pub rpc_url: String,
    
    /// Output format (json or plain)
    #[clap(long, value_enum, default_value = "plain")]
    pub format: OutputFormat,
    
    /// Starting block number (default: current + 1)
    #[clap(long)]
    pub start_block: Option<u32>,
}