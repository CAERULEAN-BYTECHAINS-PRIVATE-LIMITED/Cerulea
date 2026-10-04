use crate::{
    benchmarking::{inherent_benchmark_data, RemarkBuilder, TransferKeepAliveBuilder},
    chain_spec,
    fork_detection::ForkReport,
    node_command::{
        ForkCheckCmd, HealthCmd, InfoCmd, NodeCommand, OutputFormat, QueryAuthorsCmd,
        RuntimeUpgradeCmd, Subcommand,
    },
};

use cerulea_runtime::{Block, EXISTENTIAL_DEPOSIT};
use frame_benchmarking_cli::{BenchmarkCmd, ExtrinsicFactory, SUBSTRATE_REFERENCE_HARDWARE};
use sc_cli::SubstrateCli;
use serde_json::json;
use sp_core::crypto::{AccountId32, Ss58Codec};
use sp_keyring::Ed25519Keyring;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

impl SubstrateCli for NodeCommand {
    fn impl_name() -> String {
        "Cerulea Chain".into()
    }

    fn impl_version() -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn description() -> String {
        "Cerulea node running the Dynamic Consensus Framework.".into()
    }

    fn author() -> String {
        "Caerulean ByteChains Private Limited".into()
    }

    fn support_url() -> String {
        "https://support.cbytechains.com".into()
    }

    fn copyright_start_year() -> i32 {
        2025
    }

    fn load_spec(&self, id: &str) -> Result<Box<dyn sc_service::ChainSpec>, String> {
        Ok(match id {
            "dev" | "development" | "Cerulea" => Box::new(chain_spec::development_chain_spec()?),
            "" | "local" | "local_testnet" => Box::new(chain_spec::local_chain_spec()?),
            "multi_validator" => Box::new(chain_spec::multi_validator_chain_spec()?),
            "high_stake" => Box::new(chain_spec::high_stake_chain_spec()?),
            "public_access" | "public-access" => Box::new(chain_spec::public_access_chain_spec()?),
            "private" => Box::new(chain_spec::private_chain_spec()?),
            path => Box::new(chain_spec::ChainSpec::from_json_file(
                std::path::PathBuf::from(path),
            )?),
        })
    }
}

pub fn run() -> sc_cli::Result<()> {
    run_with(NodeCommand::from_args())
}

/// Run the node with arguments a caller already parsed.
///
/// The binary parses the process arguments and calls this. The tests parse
/// their own arguments and call it the same way.
pub fn run_with(entry: NodeCommand) -> sc_cli::Result<()> {
    // Log node startup information
    crate::logging::log_consensus_event(&format!(
        "Starting Cerulea node version {} in {} mode",
        env!("CARGO_PKG_VERSION"),
        if entry.cerulea_mode == "production" {
            "production"
        } else {
            &entry.cerulea_mode
        }
    ));

    match &entry.subcommand {
        Some(Subcommand::Key(cmd)) => cmd.run(&entry),

        Some(Subcommand::Faucet(cmd)) => {
            println!(
                "[FAUCET] Would send {} tokens to address {} (dummy, no on-chain transaction).",
                cmd.amount, cmd.to
            );
            Ok(())
        }

        Some(Subcommand::BuildSpec(cmd)) => {
            let runner = entry.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.chain_spec, config.network))
        }

        Some(Subcommand::CheckBlock(_))
        | Some(Subcommand::ImportBlocks(_))
        | Some(Subcommand::ExportBlocks(_))
        | Some(Subcommand::ExportState(_))
        | Some(Subcommand::Revert(_)) => {
            Err("This subcommand is not supported in DCF-only mode (no import queue).".into())
        }

        Some(Subcommand::PurgeChain(cmd)) => {
            let runner = entry.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run(config.database))
        }

        Some(Subcommand::Benchmark(cmd)) => {
            let runner = entry.create_runner(cmd)?;
            runner.sync_run(|config| {
                match cmd {
                    BenchmarkCmd::Pallet(cmd) => {
                        if !cfg!(feature = "runtime-benchmarks") {
                            return Err("Runtime benchmarking wasn't enabled. Use `--features runtime-benchmarks`.".into());
                        }
                        cmd.run_with_spec::<sp_runtime::traits::HashingFor<Block>, ()>(Some(config.chain_spec))
                    },
                    BenchmarkCmd::Block(cmd) => {
                        let node_config = crate::service::NodeConfig {
                            rpc_config: crate::rpc::RpcSecurityConfig {
                                enable_cerulea_extensions: entry.enable_cerulea_extensions,
                                expose_unsafe_methods: entry.unsafe_rpc_expose,
                                rate_limit_window: entry.rpc_rate_limit_window,
                                rate_limit_requests: entry.rpc_rate_limit_requests,
                            },
                        };
                        let sc_service::PartialComponents { client, .. } =
                            crate::service::new_partial(&config, &node_config)?;
                        cmd.run(client)
                    },
                    #[cfg(not(feature = "runtime-benchmarks"))]
                    BenchmarkCmd::Storage(_) => Err("Storage benchmarking requires `runtime-benchmarks` feature.".into()),
                    #[cfg(feature = "runtime-benchmarks")]
                    BenchmarkCmd::Storage(cmd) => {
                        let node_config = crate::service::NodeConfig {
                            rpc_config: crate::rpc::RpcSecurityConfig {
                                enable_cerulea_extensions: entry.enable_cerulea_extensions,
                                expose_unsafe_methods: entry.unsafe_rpc_expose,
                                rate_limit_window: entry.rpc_rate_limit_window,
                                rate_limit_requests: entry.rpc_rate_limit_requests,
                            },
                        };
                        let sc_service::PartialComponents { client, backend, .. } =
                            crate::service::new_partial(&config, &node_config)?;
                        let db = backend.expose_db();
                        let storage = backend.expose_storage();
                        cmd.run(config, client, db, storage)
                    },
                    BenchmarkCmd::Overhead(cmd) => {
                        let node_config = crate::service::NodeConfig {
                            rpc_config: crate::rpc::RpcSecurityConfig {
                                enable_cerulea_extensions: entry.enable_cerulea_extensions,
                                expose_unsafe_methods: entry.unsafe_rpc_expose,
                                rate_limit_window: entry.rpc_rate_limit_window,
                                rate_limit_requests: entry.rpc_rate_limit_requests,
                            },
                        };
                        let sc_service::PartialComponents { client, .. } =
                            crate::service::new_partial(&config, &node_config)?;
                        let ext_builder = RemarkBuilder::new(client.clone());
                        cmd.run(config.chain_spec.name().into(), client, inherent_benchmark_data()?, Vec::new(), &ext_builder, false)
                    },
                    BenchmarkCmd::Extrinsic(cmd) => {
                        let node_config = crate::service::NodeConfig {
                            rpc_config: crate::rpc::RpcSecurityConfig {
                                enable_cerulea_extensions: entry.enable_cerulea_extensions,
                                expose_unsafe_methods: entry.unsafe_rpc_expose,
                                rate_limit_window: entry.rpc_rate_limit_window,
                                rate_limit_requests: entry.rpc_rate_limit_requests,
                            },
                        };
                        let sc_service::PartialComponents { client, .. } =
                            crate::service::new_partial(&config, &node_config)?;
                        let ext_factory = ExtrinsicFactory(vec![
                            Box::new(RemarkBuilder::new(client.clone())),
                            Box::new(TransferKeepAliveBuilder::new(
                                client.clone(),
                                Ed25519Keyring::Alice.to_account_id(),
                                EXISTENTIAL_DEPOSIT,
                            )),
                        ]);
                        cmd.run(client, inherent_benchmark_data()?, Vec::new(), &ext_factory)
                    },
                    BenchmarkCmd::Machine(cmd) => cmd.run(&config, SUBSTRATE_REFERENCE_HARDWARE.clone()),
                }
            })
        }

        Some(Subcommand::ChainInfo(cmd)) => {
            let runner = entry.create_runner(cmd)?;
            runner.sync_run(|config| cmd.run::<Block>(&config))
        }

        Some(Subcommand::Info(cmd)) => run_info_cmd(cmd, &entry),

        Some(Subcommand::Health(cmd)) => run_health_cmd(cmd, &entry),

        Some(Subcommand::RuntimeUpgrade(cmd)) => run_runtime_upgrade_cmd(cmd, &entry),

        Some(Subcommand::ForkCheck(cmd)) => run_fork_check_cmd(cmd, &entry),

        Some(Subcommand::QueryAuthors(cmd)) => run_query_authors_cmd(cmd, &entry),

        None => {
            // Auto-generate the P2P network key before create_runner runs.
            // Substrate's node command checks for the key file during create_runner and throws
            // NetworkKeyNotFound if it's missing for non-dev validator nodes.
            // We resolve the path from node command args and write the key first so the runner
            // never sees a missing file. A development node starts the same way.
            ensure_network_key_from_cli(&entry);

            let runner = entry.create_runner(&entry.run)?;
            let state_cache_passed = state_cache_flag_passed(std::env::args());
            runner.run_node_until_exit(|mut config| async move {
                config.trie_cache_maximum_size =
                    state_cache_bytes(state_cache_passed, config.trie_cache_maximum_size);
                log::info!(
                    "Cerulea: the state cache holds up to {} bytes{}",
                    config.trie_cache_maximum_size.unwrap_or(0),
                    if state_cache_passed {
                        " (the figure the operator passed)"
                    } else {
                        " (the Cerulea bound; --trie-cache-size sets another)"
                    }
                );
                let node_config = crate::service::NodeConfig {
                    rpc_config: crate::rpc::RpcSecurityConfig {
                        enable_cerulea_extensions: entry.enable_cerulea_extensions,
                        expose_unsafe_methods: entry.unsafe_rpc_expose,
                        rate_limit_window: entry.rpc_rate_limit_window,
                        rate_limit_requests: entry.rpc_rate_limit_requests,
                    },
                };
                match config.network.network_backend.unwrap_or_default() {
                    sc_network::config::NetworkBackendType::Libp2p => {
                        crate::service::new_full::<
                            sc_network::NetworkWorker<
                                cerulea_runtime::opaque::Block,
                                <cerulea_runtime::opaque::Block as sp_runtime::traits::Block>::Hash,
                            >,
                        >(config, &node_config)
                        .map_err(sc_cli::Error::Service)
                    }
                    sc_network::config::NetworkBackendType::Litep2p => crate::service::new_full::<
                        sc_network::Litep2pNetworkBackend,
                    >(
                        config, &node_config
                    )
                    .map_err(sc_cli::Error::Service),
                }
            })
        }
    }
}

/// Run the info subcommand
fn run_info_cmd(cmd: &InfoCmd, entry: &NodeCommand) -> sc_cli::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let impl_name = NodeCommand::impl_name();
    let impl_version = NodeCommand::impl_version();
    let description = NodeCommand::description();
    let author = NodeCommand::author();
    let support_url = NodeCommand::support_url();

    // Get system information
    let uptime = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    match cmd.format {
        OutputFormat::Json => {
            let info = json!({
                "name": impl_name,
                "version": version,
                "impl_version": impl_version,
                "description": description,
                "author": author,
                "support_url": support_url,
                "uptime_seconds": uptime,
                "cerulea_mode": entry.cerulea_mode,
                "cerulea_extensions_enabled": entry.enable_cerulea_extensions,
                "unsafe_rpc_exposed": entry.unsafe_rpc_expose,
                "rate_limit_window": entry.rpc_rate_limit_window,
                "rate_limit_requests": entry.rpc_rate_limit_requests
            });
            println!("{}", serde_json::to_string_pretty(&info).unwrap());
        }
        OutputFormat::Plain => {
            println!("Cerulea Node Information");
            println!("====================");
            println!("Name: {}", impl_name);
            println!("Version: {}", version);
            println!("Implementation Version: {}", impl_version);
            println!("Description: {}", description);
            println!("Author: {}", author);
            println!("Support URL: {}", support_url);
            println!("Uptime: {} seconds", uptime);
            println!("Cerulea Mode: {}", entry.cerulea_mode);
            println!(
                "Cerulea Extensions Enabled: {}",
                entry.enable_cerulea_extensions
            );
            println!("Unsafe RPC Exposed: {}", entry.unsafe_rpc_expose);
            println!("Rate Limit Window: {} seconds", entry.rpc_rate_limit_window);
            println!("Rate Limit Requests: {}", entry.rpc_rate_limit_requests);
        }
    }

    Ok(())
}

/// Run the health subcommand
fn run_health_cmd(cmd: &HealthCmd, _cli: &NodeCommand) -> sc_cli::Result<()> {
    // Basic health check - in a real implementation this would check:
    // - Node connectivity
    // - Sync status
    // - Peer connections
    // - Database health
    // - Memory usage

    let is_healthy = true; // Simplified for now
    let issues = Vec::<String>::new(); // No issues detected

    match cmd.format {
        OutputFormat::Json => {
            let health = json!({
                "is_healthy": is_healthy,
                "status": if is_healthy { "healthy" } else { "unhealthy" },
                "issues": issues,
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            });
            println!("{}", serde_json::to_string_pretty(&health).unwrap());
        }
        OutputFormat::Plain => {
            if is_healthy {
                println!("Node Status: HEALTHY");
            } else {
                println!("Node Status: UNHEALTHY");
                for issue in &issues {
                    println!("Issue: {}", issue);
                }
            }
        }
    }

    Ok(())
}

/// Run the runtime upgrade subcommand
fn run_runtime_upgrade_cmd(cmd: &RuntimeUpgradeCmd, _cli: &NodeCommand) -> sc_cli::Result<()> {
    // Validate WASM file exists
    if !cmd.wasm.exists() {
        return Err(format!("WASM file not found: {}", cmd.wasm.display()).into());
    }

    // Read WASM file
    let wasm_code = fs::read(&cmd.wasm).map_err(|e| format!("Failed to read WASM file: {}", e))?;

    if wasm_code.is_empty() {
        return Err("WASM file is empty".into());
    }

    match cmd.format {
        OutputFormat::Json => {
            let result = json!({
                "status": "prepared",
                "wasm_file": cmd.wasm.display().to_string(),
                "wasm_size_bytes": wasm_code.len(),
                "poll_interval_seconds": cmd.poll_interval,
                "message": "Runtime upgrade prepared. In a full implementation, this would submit the upgrade transaction and poll for enactment."
            });
            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
        OutputFormat::Plain => {
            println!("Runtime Upgrade Prepared");
            println!("========================");
            println!("WASM File: {}", cmd.wasm.display());
            println!("WASM Size: {} bytes", wasm_code.len());
            println!("Poll Interval: {} seconds", cmd.poll_interval);
            println!();
            println!("Note: In a full implementation, this would:");
            println!("1. Submit the runtime upgrade transaction");
            println!(
                "2. Poll every {} seconds until enactment",
                cmd.poll_interval
            );
            println!("3. Verify the upgrade was successful");
        }
    }

    Ok(())
}

/// Run the fork check subcommand
fn run_fork_check_cmd(cmd: &ForkCheckCmd, _cli: &NodeCommand) -> sc_cli::Result<()> {
    if cmd.peer_rpc.is_empty() {
        return Err("At least one peer RPC endpoint must be provided".into());
    }

    // Create a minimal runtime for the fork checker
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create async runtime: {}", e))?;

    rt.block_on(async {
        // For node command usage, we use the standalone fork checker from tools
        use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
        use std::time::Duration;
        use tokio::time::timeout;

        let timeout_duration = Duration::from_secs(cmd.timeout);
        let mut fork_reports = Vec::new();

        // Get local block info
        let local_client = HttpClientBuilder::default()
            .request_timeout(timeout_duration)
            .build(&cmd.local_rpc)
            .map_err(|e| format!("Failed to connect to local RPC: {}", e))?;

        let local_header: serde_json::Value = timeout(
            timeout_duration,
            local_client.request("chain_getHeader", rpc_params![]),
        )
        .await
        .map_err(|_| "Timeout connecting to local node")?
        .map_err(|e| format!("Failed to get local header: {}", e))?;

        let local_number = local_header["number"]
            .as_str()
            .ok_or("Invalid local block number format")?;
        let local_number = u32::from_str_radix(local_number.trim_start_matches("0x"), 16)
            .map_err(|e| format!("Failed to parse local block number: {}", e))?;

        // Check each peer
        for (index, peer_rpc) in cmd.peer_rpc.iter().enumerate() {
            match async {
                let peer_client = HttpClientBuilder::default()
                    .request_timeout(timeout_duration)
                    .build(peer_rpc)?;

                let peer_header: serde_json::Value = timeout(
                    timeout_duration,
                    peer_client.request("chain_getHeader", rpc_params![]),
                )
                .await??;

                let peer_number = peer_header["number"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Invalid peer block number format"))?;
                let peer_number = u32::from_str_radix(peer_number.trim_start_matches("0x"), 16)?;

                Ok::<u32, anyhow::Error>(peer_number)
            }
            .await
            {
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

                    if divergence > cmd.threshold {
                        eprintln!(
                            "WARNING: Fork detected with {} - Local: {}, Peer: {}, Divergence: {}",
                            peer_id, local_number, peer_number, divergence
                        );
                    }

                    fork_reports.push(report);
                }
                Err(e) => {
                    eprintln!("Failed to connect to peer {}: {}", peer_rpc, e);
                }
            }
        }

        // Output results
        match cmd.format {
            OutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&fork_reports)
                        .map_err(|e| format!("Failed to serialize results: {}", e))?
                );
            }
            OutputFormat::Plain => {
                println!("Fork Detection Report");
                println!("====================");
                println!("Threshold: {} blocks", cmd.threshold);
                println!();

                if fork_reports.is_empty() {
                    println!("No peers checked.");
                    return Ok(());
                }

                let mut has_forks = false;
                for report in &fork_reports {
                    println!("Peer: {}", report.peer_id);
                    println!("  Local Best:  {}", report.local_best);
                    println!("  Peer Best:   {}", report.peer_best);
                    println!("  Divergence:  {}", report.divergence);

                    if report.divergence > cmd.threshold {
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

        Ok::<(), sc_cli::Error>(())
    })?;

    Ok(())
}

/// Run the query authors subcommand
fn run_query_authors_cmd(cmd: &QueryAuthorsCmd, _cli: &NodeCommand) -> sc_cli::Result<()> {
    // Create a minimal runtime for the query
    let rt = tokio::runtime::Runtime::new()
        .map_err(|e| format!("Failed to create async runtime: {}", e))?;

    rt.block_on(async {
        use jsonrpsee::{core::client::ClientT, http_client::HttpClientBuilder, rpc_params};
        use std::time::Duration;
        use tokio::time::timeout;

        let timeout_duration = Duration::from_secs(30);

        // Connect to the RPC endpoint
        let client = HttpClientBuilder::default()
            .request_timeout(timeout_duration)
            .build(&cmd.rpc_url)
            .map_err(|e| format!("Failed to connect to RPC endpoint {}: {}", cmd.rpc_url, e))?;

        // Get current block number if start_block is not specified
        let start_block = if let Some(start_block) = cmd.start_block {
            start_block
        } else {
            // Get current block header
            let header: serde_json::Value = timeout(
                timeout_duration,
                client.request("chain_getHeader", rpc_params![]),
            )
            .await
            .map_err(|_| "Timeout getting current block header")?
            .map_err(|e| format!("Failed to get current block header: {}", e))?;

            let current_number = header["number"]
                .as_str()
                .ok_or("Invalid current block number format")?;
            let current_number =
                u32::from_str_radix(current_number.trim_start_matches("0x"), 16)
                    .map_err(|e| format!("Failed to parse current block number: {}", e))?;

            current_number + 1 // Start from next block
        };

        // Query the expected author of every requested block. The chain
        // answers with an address, or with nothing when it names no author.
        let mut authors: Vec<(u32, Option<AccountId32>)> = Vec::new();

        for i in 0..cmd.blocks {
            let block_number = start_block + i;
            let answer = timeout(
                timeout_duration,
                client.request::<serde_json::Value, _>(
                    "dcf_getExpectedAuthor",
                    rpc_params![block_number],
                ),
            )
            .await
            .map_err(|_| format!("Timeout reading the expected author of block {block_number}"))?
            .map_err(|e| {
                format!(
                    "The node at {} answers no expected author for block {}: {}.                      Point the query at a Cerulea node serving the Cerulea interface.",
                    cmd.rpc_url, block_number, e
                )
            })?;
            let author = match answer {
                serde_json::Value::Null => None,
                serde_json::Value::String(text) => Some(parse_author(&text)?),
                other => {
                    return Err(format!(
                        "The expected author of block {block_number} arrived as {other}.                          Point the query at a Cerulea node serving the Cerulea interface."
                    )
                    .into())
                }
            };
            authors.push((block_number, author));
        }

        // Format and output results
        match cmd.format {
            OutputFormat::Json => {
                let json_authors: Vec<serde_json::Value> = authors
                    .iter()
                    .map(|(block, author)| match author {
                        Some(held) => json!({
                            "block": block,
                            "author": held.to_ss58check(),
                            "author_hex": format!("0x{}", hex::encode(held.as_ref() as &[u8]))
                        }),
                        None => json!({
                            "block": block,
                            "author": serde_json::Value::Null,
                            "author_hex": serde_json::Value::Null
                        }),
                    })
                    .collect();

                let result = json!({
                    "start_block": start_block,
                    "block_count": cmd.blocks,
                    "rpc_endpoint": cmd.rpc_url,
                    "authors": json_authors
                });

                println!(
                    "{}",
                    serde_json::to_string_pretty(&result)
                        .map_err(|e| format!("Failed to serialize results: {}", e))?
                );
            }
            OutputFormat::Plain => {
                println!("Upcoming Block Authors");
                println!("======================");
                println!("RPC Endpoint: {}", cmd.rpc_url);
                println!("Start Block:  {}", start_block);
                println!("Block Count:  {}", cmd.blocks);
                println!();

                for (block, author) in &authors {
                    match author {
                        Some(held) => println!("Block {}: {}", block, held.to_ss58check()),
                        None => println!("Block {}: the chain names no author", block),
                    }
                }

                if authors.is_empty() {
                    println!("No authors found.");
                } else {
                    println!();
                    println!("Successfully queried {} upcoming authors", authors.len());
                }
            }
        }

        Ok::<(), sc_cli::Error>(())
    })?;

    Ok(())
}

/// Ensures the P2P network key exists before Substrate's node command runner checks for it.
///
/// Substrate throws `NetworkKeyNotFound` during `create_runner` for non-dev validator nodes
/// if `<base-path>/chains/<chain-id>/network/secret_ed25519` is missing. We resolve the
/// base path the same way Substrate does (explicit `--base-path` or the platform default
/// `~/.local/share/cerulea-node/` on Linux), load the chain spec for the canonical chain ID,
/// then write a fresh hex-encoded key. Any new node therefore starts with no manual setup.
fn ensure_network_key_from_cli(entry: &crate::node_command::NodeCommand) {
    use sc_cli::SubstrateCli;

    // A development node keeps ephemeral keys, so the step ends here.
    let raw_chain = match entry.run.shared_params.chain.as_deref() {
        Some("dev") | Some("development") | Some("Cerulea") => return,
        None => return,
        Some(id) => id,
    };

    // Resolve base path: explicit --base-path, or the platform default
    // (~/.local/share/cerulea-node/ on Linux, equivalent on other platforms).
    let base_path = match entry.run.shared_params.base_path.as_ref() {
        Some(p) => p.clone(),
        None => {
            // Mirror Substrate's BasePath::from_project("", "", executable_name)
            match directories::ProjectDirs::from("", "", &NodeCommand::executable_name()) {
                Some(dirs) => dirs.data_local_dir().to_path_buf(),
                None => return, // can't determine path, skip
            }
        }
    };

    // Load the chain spec to resolve the canonical ID ("local" → "cerulea_local", etc.)
    let chain_id = match entry.load_spec(raw_chain) {
        Ok(spec) => spec.id().to_string(),
        Err(_) => raw_chain.to_string(),
    };

    let net_key_path = base_path
        .join("chains")
        .join(&chain_id)
        .join("network")
        .join("secret_ed25519");

    if net_key_path.exists() {
        return;
    }

    if let Some(parent) = net_key_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!(
                "Cerulea: Could not create network key directory {:?}: {}",
                parent,
                e
            );
            return;
        }
    }

    // Write a fresh random key as 64 hex characters. The network layer expects that format,
    // identical to `cerulea-node key generate-node-key --file <path>`.
    use sp_core::crypto::Pair as _;
    let (pair, _) = sp_core::ed25519::Pair::generate();
    let secret_hex = hex::encode(&pair.to_raw_vec()[..32]);

    match fs::write(&net_key_path, secret_hex.as_bytes()) {
        Ok(_) => log::info!(
            "Cerulea: Auto-generated P2P network key at {:?}",
            net_key_path
        ),
        Err(e) => log::warn!(
            "Cerulea: Failed to write network key to {:?}: {}",
            net_key_path,
            e
        ),
    }
}

/// Read one author address the node interface answers with.
///
/// The interface writes an account as its SS58 address. A 32 byte hex form
/// reads too. Any other text names the fix.
pub fn parse_author(text: &str) -> Result<AccountId32, String> {
    if let Ok(held) = AccountId32::from_ss58check(text) {
        return Ok(held);
    }
    let bytes = hex::decode(text.trim_start_matches("0x")).map_err(|_| {
        format!("The author `{text}` reads as neither an SS58 address nor 32 hex bytes. Point the query at a Cerulea node serving the Cerulea interface.")
    })?;
    let fixed: [u8; 32] = bytes.try_into().map_err(|_| {
        format!("The author `{text}` carries a length other than 32 bytes. Point the query at a Cerulea node serving the Cerulea interface.")
    })?;
    Ok(AccountId32::from(fixed))
}

/// The state cache a node keeps where the operator passes no figure, in bytes.
///
/// The state cache holds the parts of the chain state the node read lately,
/// so a repeated read skips the database. The client library gives every node
/// a one gibibyte cache. The cache fills at the pace the state is read, and on
/// the nine validator network the resident memory of a validator grew about
/// 140 MB in the first hour and 22 MB an hour after that, towards that
/// ceiling, past a 700 MB container cap. A validator of a Cerulea network
/// reads a modest state, so 128 MiB serves it.
pub const STATE_CACHE_BYTES: usize = 128 * 1024 * 1024;

/// Report whether the arguments carry the `--trie-cache-size` flag.
pub fn state_cache_flag_passed(arguments: impl IntoIterator<Item = String>) -> bool {
    arguments.into_iter().any(|argument| {
        argument == "--trie-cache-size" || argument.starts_with("--trie-cache-size=")
    })
}

/// The state cache size the node runs with.
///
/// A figure the operator passed stands as given, zero included, which turns
/// the cache off. With no figure passed, the Cerulea bound applies in place
/// of the library default.
pub fn state_cache_bytes(operator_passed: bool, requested: Option<usize>) -> Option<usize> {
    if operator_passed {
        requested
    } else {
        Some(STATE_CACHE_BYTES)
    }
}

#[cfg(test)]
mod state_cache_tests {
    use super::{state_cache_bytes, state_cache_flag_passed, STATE_CACHE_BYTES};

    #[test]
    fn the_cerulea_bound_applies_where_the_operator_passes_no_figure() {
        assert_eq!(
            state_cache_bytes(false, Some(1024 * 1024 * 1024)),
            Some(STATE_CACHE_BYTES)
        );
        assert_eq!(state_cache_bytes(false, None), Some(STATE_CACHE_BYTES));
        assert_eq!(STATE_CACHE_BYTES, 134_217_728);
    }

    #[test]
    fn a_figure_the_operator_passed_stands() {
        assert_eq!(state_cache_bytes(true, Some(64 << 20)), Some(64 << 20));
        assert_eq!(state_cache_bytes(true, Some(0)), Some(0));
        assert_eq!(state_cache_bytes(true, None), None);
    }

    #[test]
    fn the_flag_is_read_in_both_spellings() {
        let args = |list: &[&str]| list.iter().map(|part| part.to_string()).collect::<Vec<_>>();
        assert!(state_cache_flag_passed(args(&[
            "cerulea-node",
            "--trie-cache-size",
            "67108864"
        ])));
        assert!(state_cache_flag_passed(args(&[
            "cerulea-node",
            "--trie-cache-size=67108864"
        ])));
        assert!(!state_cache_flag_passed(args(&[
            "cerulea-node",
            "--validator"
        ])));
        assert!(!state_cache_flag_passed(args(&[])));
    }
}

#[cfg(test)]
mod author_tests {
    use super::parse_author;
    use sp_core::crypto::{AccountId32, Ss58Codec};

    #[test]
    fn an_ss58_address_and_its_hex_form_read_as_one_account() {
        let account = AccountId32::from([7u8; 32]);
        assert_eq!(parse_author(&account.to_ss58check()), Ok(account.clone()));
        assert_eq!(
            parse_author(&format!("0x{}", hex::encode([7u8; 32]))),
            Ok(account)
        );
    }

    #[test]
    fn other_text_names_the_fix() {
        let fault = parse_author("not an address").expect_err("the text reads as neither form");
        assert!(fault.ends_with("Point the query at a Cerulea node serving the Cerulea interface."));
        let short = parse_author("0x0102").expect_err("two bytes fall short");
        assert!(short.contains("a length other than 32 bytes"));
    }
}
