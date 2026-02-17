
use clap::Parser;
use cbc_node::{cli, logging, command};

fn main() -> sc_cli::Result<()> {
    let cli = cli::Cli::parse();

    // Configure RUST_LOG environment variable (but don't initialize logger)
    let _log_config = logging::init_cbc_logging_config(cli.cbc_log_only, cli.quiet);
    
    // Display startup information only if not in quiet mode
    if !cli.quiet {
        logging::display_startup_info(
            env!("CARGO_PKG_VERSION"),
            env!("CARGO_PKG_VERSION"),
            "CBC Chain",
            "Node",
            None, // Peer ID will be set later in service startup
            None, // Network latency will be measured later
        );
    }

    command::run()
}