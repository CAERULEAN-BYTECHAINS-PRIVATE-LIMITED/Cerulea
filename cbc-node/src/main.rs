
use clap::Parser;
use cbc_node::{cli, logging, command, lifecycle_tracer};
use std::path::PathBuf;

fn main() -> sc_cli::Result<()> {
    // STEP 1: Node entry point
    // Note: We can't trace this step yet because tracer isn't initialized
    
    let cli = cli::Cli::parse();

    // Initialize lifecycle tracer based on CLI flags
    if cli.lifecycle_trace {
        let output_file = cli.lifecycle_trace_output.as_ref().map(PathBuf::from);
        let tracer_config = lifecycle_tracer::TracerConfig::from_cli_flags(
            true,
            cli.lifecycle_trace_format.clone(),
            output_file,
            cli.lifecycle_trace_milestone_only,
        );
        
        if let Err(e) = lifecycle_tracer::LifecycleTracer::init(tracer_config) {
            eprintln!("Failed to initialize lifecycle tracer: {}", e);
        }
        
        // STEP 1: Node entry point - main.rs started
        lifecycle_tracer::LifecycleTracer::global().trace_step(
            1,
            "main.rs",
            "Node entry point - main.rs started",
            None,
        );
        
        // STEP 2: CLI configuration parsed
        let cli_metadata = lifecycle_tracer::TraceMetadata {
            block_number: None,
            block_hash: None,
            author: None,
            extrinsic_count: None,
            custom: {
                let mut map = std::collections::HashMap::new();
                map.insert("cbc_mode".to_string(), cli.cbc_mode.clone());
                map.insert("enable_cbc_extensions".to_string(), cli.enable_cbc_extensions.to_string());
                map.insert("unsafe_rpc_expose".to_string(), cli.unsafe_rpc_expose.to_string());
                map.insert("lifecycle_trace".to_string(), cli.lifecycle_trace.to_string());
                map.insert("lifecycle_trace_format".to_string(), 
                    cli.lifecycle_trace_format.clone().unwrap_or_else(|| "human-readable".to_string()));
                map.insert("lifecycle_trace_milestone_only".to_string(), 
                    cli.lifecycle_trace_milestone_only.to_string());
                if let Some(ref output) = cli.lifecycle_trace_output {
                    map.insert("lifecycle_trace_output".to_string(), output.clone());
                }
                map
            },
        };
        
        lifecycle_tracer::LifecycleTracer::global().trace_step(
            2,
            "main.rs",
            "CLI configuration parsed",
            Some(cli_metadata),
        );
    }

    // Configure RUST_LOG environment variable (but don't initialize logger)
    let log_config = logging::init_cbc_logging_config(cli.cbc_log_only, cli.quiet);
    
    // STEP 3: Logging system configured
    if cli.lifecycle_trace {
        let log_metadata = lifecycle_tracer::TraceMetadata {
            block_number: None,
            block_hash: None,
            author: None,
            extrinsic_count: None,
            custom: {
                let mut map = std::collections::HashMap::new();
                map.insert("log_level".to_string(), log_config.clone());
                map.insert("cbc_log_only".to_string(), cli.cbc_log_only.to_string());
                map.insert("quiet".to_string(), cli.quiet.to_string());
                map
            },
        };
        
        lifecycle_tracer::LifecycleTracer::global().trace_step(
            3,
            "main.rs",
            "Logging system configured",
            Some(log_metadata),
        );
    }
    
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
    
    // STEP 4: Node metadata initialized
    if cli.lifecycle_trace {
        let metadata = lifecycle_tracer::TraceMetadata {
            block_number: None,
            block_hash: None,
            author: None,
            extrinsic_count: None,
            custom: {
                let mut map = std::collections::HashMap::new();
                map.insert("version".to_string(), env!("CARGO_PKG_VERSION").to_string());
                map.insert("chain".to_string(), "CBC Chain".to_string());
                map.insert("node_type".to_string(), "Node".to_string());
                map
            },
        };
        
        lifecycle_tracer::LifecycleTracer::global().trace_step(
            4,
            "main.rs",
            "Node metadata initialized",
            Some(metadata),
        );
    }
    
    // STEP 5: Command execution started
    if cli.lifecycle_trace {
        let command_type = if let Some(ref subcommand) = cli.subcommand {
            format!("{:?}", subcommand).split('(').next().unwrap_or("Unknown").to_string()
        } else {
            "Run".to_string()
        };
        
        let command_metadata = lifecycle_tracer::TraceMetadata {
            block_number: None,
            block_hash: None,
            author: None,
            extrinsic_count: None,
            custom: {
                let mut map = std::collections::HashMap::new();
                map.insert("command_type".to_string(), command_type);
                map
            },
        };
        
        lifecycle_tracer::LifecycleTracer::global().trace_step(
            5,
            "main.rs",
            "Command execution started",
            Some(command_metadata),
        );
        
        // Flush traces before command execution to ensure they're written
        lifecycle_tracer::LifecycleTracer::global().flush();
    }

    command::run()
}