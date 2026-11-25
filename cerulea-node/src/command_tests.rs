//! The node subcommands, run through the same entry point the binary runs.
//!
//! Each test parses a command line with the node argument parser and hands it
//! to `command::run_with`. The fork check and the author query read a live
//! HTTP endpoint serving the fixture chain. The subcommands that open a
//! service runner run in their own process from `tests/node_runner_test.rs`,
//! since a runner installs the process logger once.

use clap::Parser;
use jsonrpsee::server::ServerHandle;
use jsonrpsee::RpcModule;
use sc_cli::SubstrateCli;
use sp_blockchain::HeaderBackend;

use crate::chain_fixture_tests::{fixture, serve_fixture};
use crate::command::run_with;
use crate::node_command::NodeCommand;

/// Parse one node command line.
fn parsed(arguments: &[&str]) -> NodeCommand {
    let mut full = vec!["cerulea-node"];
    full.extend_from_slice(arguments);
    NodeCommand::try_parse_from(full).expect("the node arguments parse")
}

/// Run one node command line and read the outcome as text.
fn outcome(arguments: &[&str]) -> Result<(), String> {
    run_with(parsed(arguments)).map_err(|error| error.to_string())
}

/// Serve one real header of the fixture chain as the head of a peer.
///
/// The peer answers the header method alone. The header comes from the
/// fixture client, so the peer reports a height the chain truly reached.
async fn peer_at(number: u32) -> (String, ServerHandle) {
    let chain = fixture();
    let hash = chain.client.hash(number).unwrap().unwrap();
    let header = chain.client.header(hash).unwrap().unwrap();
    let answer = serde_json::to_value(&header).unwrap();
    let mut module = RpcModule::new(());
    module
        .register_method("chain_getHeader", move |_, _, _| Some(answer.clone()))
        .unwrap();
    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();
    let address = server.local_addr().unwrap();
    (format!("http://{address}"), server.start(module))
}

/// Serve a peer whose header carries a height in the wrong shape.
async fn peer_with_numeric_height() -> (String, ServerHandle) {
    let mut module = RpcModule::new(());
    module
        .register_method("chain_getHeader", |_, _, _| {
            Some(serde_json::json!({ "number": 7 }))
        })
        .unwrap();
    let server = jsonrpsee::server::Server::builder()
        .build("127.0.0.1:0")
        .await
        .unwrap();
    let address = server.local_addr().unwrap();
    (format!("http://{address}"), server.start(module))
}

/// Run a command line on a plain thread, so the command builds its own
/// async runtime the way the binary does.
async fn outcome_off_runtime(arguments: Vec<String>) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        std::thread::spawn(move || {
            let borrowed: Vec<&str> = arguments.iter().map(String::as_str).collect();
            outcome(&borrowed)
        })
        .join()
        .expect("the command thread finishes")
    })
    .await
    .expect("the waiting task finishes")
}

fn texts(arguments: &[&str]) -> Vec<String> {
    arguments.iter().map(|text| text.to_string()).collect()
}

#[test]
fn the_node_describes_itself_through_the_shared_argument_parser() {
    use sc_cli::SubstrateCli;
    assert_eq!(NodeCommand::impl_name(), "Cerulea Chain");
    assert_eq!(NodeCommand::impl_version(), env!("CARGO_PKG_VERSION"));
    assert_eq!(
        NodeCommand::description(),
        "Cerulea node running the Dynamic Consensus Framework."
    );
    assert_eq!(
        NodeCommand::author(),
        "Caerulean ByteChains Private Limited"
    );
    assert_eq!(
        NodeCommand::support_url(),
        "https://support.cbytechains.com"
    );
    assert_eq!(NodeCommand::copyright_start_year(), 2025);

    let entry = parsed(&[]);
    for (name, id) in [
        ("dev", "cerulea_dev"),
        ("development", "cerulea_dev"),
        ("Cerulea", "cerulea_dev"),
        ("", "cerulea_local"),
        ("local", "cerulea_local"),
        ("local_testnet", "cerulea_local"),
        ("multi_validator", "cerulea_multi_validator"),
        ("high_stake", "cerulea_high_stake"),
    ] {
        let spec = entry.load_spec(name).expect("the named chain loads");
        assert_eq!(spec.id(), id, "{name}");
    }
    let missing = match entry.load_spec("/no/such/chain/specification.json") {
        Ok(_) => panic!("a missing file fails to load"),
        Err(held) => held,
    };
    assert!(!missing.is_empty());
}

#[test]
fn a_chain_specification_file_loads_by_path() {
    let entry = parsed(&[]);
    let spec = crate::chain_spec::development_chain_spec().unwrap();
    let json = sc_service::ChainSpec::as_json(&spec, false).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let path = folder.path().join("cerulea.json");
    std::fs::write(&path, json).unwrap();
    let loaded = entry.load_spec(path.to_str().unwrap()).unwrap();
    assert_eq!(loaded.id(), "cerulea_dev");
    assert_eq!(loaded.name(), "Cerulea Development Chain");
}

#[test]
fn the_faucet_the_info_and_the_health_subcommands_answer() {
    assert_eq!(
        outcome(&[
            "faucet",
            "--to",
            "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
            "--amount",
            "10"
        ]),
        Ok(())
    );
    assert_eq!(outcome(&["info"]), Ok(()));
    assert_eq!(
        outcome(&["--cerulea-mode", "testing", "info", "--format", "json"]),
        Ok(())
    );
    assert_eq!(outcome(&["health"]), Ok(()));
    assert_eq!(outcome(&["health", "--format", "json"]), Ok(()));
}

#[test]
fn the_key_subcommand_generates_and_inspects_keys() {
    assert_eq!(outcome(&["key", "generate", "--scheme", "ed25519"]), Ok(()));
    assert_eq!(
        outcome(&["key", "inspect", "//Alice", "--scheme", "ed25519"]),
        Ok(())
    );
    // A scheme the key tool lacks stops the command at the argument parser.
    let refused = NodeCommand::try_parse_from([
        "cerulea-node",
        "key",
        "inspect",
        "//Alice",
        "--scheme",
        "rsa",
    ]);
    assert!(refused.is_err());
}

#[test]
fn the_block_file_subcommands_name_the_unsupported_mode() {
    for arguments in [
        vec!["check-block", "1"],
        vec!["import-blocks"],
        vec!["export-blocks"],
        vec!["export-state"],
        vec!["revert"],
    ] {
        let reason = outcome(&arguments).expect_err("the subcommand stays unsupported");
        assert!(
            reason.contains("This subcommand is not supported in DCF-only mode (no import queue)."),
            "{reason}"
        );
    }
}

#[test]
fn the_runtime_upgrade_subcommand_checks_the_file_before_preparing() {
    let folder = tempfile::tempdir().unwrap();
    let missing = folder.path().join("absent.wasm");
    let reason = outcome(&["runtime-upgrade", "--wasm", missing.to_str().unwrap()])
        .expect_err("a missing file fails");
    assert!(reason.contains(&format!("WASM file not found: {}", missing.display())));

    let empty = folder.path().join("empty.wasm");
    std::fs::write(&empty, b"").unwrap();
    let reason = outcome(&["runtime-upgrade", "--wasm", empty.to_str().unwrap()])
        .expect_err("an empty file fails");
    assert!(reason.contains("WASM file is empty"));

    let reason = outcome(&["runtime-upgrade", "--wasm", folder.path().to_str().unwrap()])
        .expect_err("a folder fails to read");
    assert!(reason.contains("Failed to read WASM file: "));

    let runtime = folder.path().join("runtime.wasm");
    std::fs::write(&runtime, cerulea_runtime::WASM_BINARY.unwrap()).unwrap();
    assert_eq!(
        outcome(&["runtime-upgrade", "--wasm", runtime.to_str().unwrap()]),
        Ok(())
    );
    assert_eq!(
        outcome(&[
            "runtime-upgrade",
            "--wasm",
            runtime.to_str().unwrap(),
            "--poll-interval",
            "2",
            "--format",
            "json"
        ]),
        Ok(())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_fork_check_subcommand_reads_every_peer() {
    let (local, local_handle) = serve_fixture().await;
    let (behind, behind_handle) = peer_at(1).await;
    let (odd, odd_handle) = peer_with_numeric_height().await;

    let reason = outcome_off_runtime(texts(&["fork-check", "--local-rpc", &local]))
        .await
        .expect_err("a check without peers fails");
    assert!(reason.contains("At least one peer RPC endpoint must be provided"));

    let peers = format!("{local},{behind},{odd},http://127.0.0.1:1");
    assert_eq!(
        outcome_off_runtime(texts(&[
            "fork-check",
            "--local-rpc",
            &local,
            "--peer-rpc",
            &peers,
            "--threshold",
            "2",
            "--timeout",
            "5"
        ]))
        .await,
        Ok(())
    );
    assert_eq!(
        outcome_off_runtime(texts(&[
            "fork-check",
            "--local-rpc",
            &local,
            "--peer-rpc",
            &local,
            "--format",
            "json"
        ]))
        .await,
        Ok(())
    );
    // Every peer out of reach leaves the report empty.
    assert_eq!(
        outcome_off_runtime(texts(&[
            "fork-check",
            "--local-rpc",
            &local,
            "--peer-rpc",
            "http://127.0.0.1:1",
            "--timeout",
            "2"
        ]))
        .await,
        Ok(())
    );

    let reason = outcome_off_runtime(texts(&[
        "fork-check",
        "--local-rpc",
        "http://127.0.0.1:1",
        "--peer-rpc",
        &local,
        "--timeout",
        "2",
    ]))
    .await
    .expect_err("an unreachable local node fails");
    assert!(reason.contains("Failed to get local header: "), "{reason}");

    let reason = outcome_off_runtime(texts(&[
        "fork-check",
        "--local-rpc",
        "not a url",
        "--peer-rpc",
        &local,
    ]))
    .await
    .expect_err("a malformed local address fails");
    assert!(
        reason.contains("Failed to connect to local RPC: "),
        "{reason}"
    );

    let reason = outcome_off_runtime(texts(&[
        "fork-check",
        "--local-rpc",
        &odd,
        "--peer-rpc",
        &local,
    ]))
    .await
    .expect_err("a local height in the wrong shape fails");
    assert!(
        reason.contains("Invalid local block number format"),
        "{reason}"
    );

    local_handle.stop().unwrap();
    behind_handle.stop().unwrap();
    odd_handle.stop().unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_author_query_subcommand_reads_the_live_endpoint() {
    let (local, handle) = serve_fixture().await;
    let (odd, odd_handle) = peer_with_numeric_height().await;
    assert_eq!(
        outcome_off_runtime(texts(&["query-authors", "--rpc-url", &local, "-n", "3"])).await,
        Ok(())
    );
    assert_eq!(
        outcome_off_runtime(texts(&[
            "query-authors",
            "--rpc-url",
            &local,
            "--blocks",
            "2",
            "--start-block",
            "4",
            "--format",
            "json"
        ]))
        .await,
        Ok(())
    );
    // A peer serving no author method answers with no estimate. The query
    // names the fix.
    let (behind, behind_handle) = peer_at(1).await;
    let reason = outcome_off_runtime(texts(&["query-authors", "--rpc-url", &behind, "-n", "2"]))
        .await
        .expect_err("a peer without the author method names no author");
    assert!(
        reason.contains("Point the query at a Cerulea node serving the Cerulea interface."),
        "{reason}"
    );
    // A query over zero blocks reads the height alone.
    assert_eq!(
        outcome_off_runtime(texts(&["query-authors", "--rpc-url", &behind, "-n", "0"])).await,
        Ok(())
    );

    let reason = outcome_off_runtime(texts(&["query-authors", "--rpc-url", "http://127.0.0.1:1"]))
        .await
        .expect_err("an unreachable endpoint fails");
    assert!(
        reason.contains("Failed to get current block header: "),
        "{reason}"
    );
    let reason = outcome_off_runtime(texts(&["query-authors", "--rpc-url", "not a url"]))
        .await
        .expect_err("a malformed endpoint fails");
    assert!(
        reason.contains("Failed to connect to RPC endpoint not a url: "),
        "{reason}"
    );
    let reason = outcome_off_runtime(texts(&["query-authors", "--rpc-url", &odd]))
        .await
        .expect_err("a height in the wrong shape fails");
    assert!(
        reason.contains("Invalid current block number format"),
        "{reason}"
    );

    handle.stop().unwrap();
    behind_handle.stop().unwrap();
    odd_handle.stop().unwrap();
}
