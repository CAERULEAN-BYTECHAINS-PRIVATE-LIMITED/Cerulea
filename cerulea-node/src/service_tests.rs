//! The node service, started in process on real chains.
//!
//! The partial service opens the client, the pool and the import queue. The
//! full service adds the network, the Dynamic Consensus Framework, the
//! finality sync, the block tracker and the interface server. Every test
//! parses real node arguments, builds the service settings the node builds
//! and starts the service on free ports inside a temporary folder.

use std::time::{Duration, Instant};

use cerulea_runtime::opaque::Block;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use sp_blockchain::HeaderBackend;
use sp_core::H256;

use crate::chain_fixture_tests::{node_settings, open_extensions, seal_key};

/// A port nothing listens on at the moment of the call.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("the loopback address binds")
        .local_addr()
        .expect("the listener reports its port")
        .port()
}

fn test_runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .expect("the test runtime starts")
}

/// Read the head height a node reports over its interface.
async fn head_of(url: &str) -> Option<u32> {
    let client = jsonrpsee::http_client::HttpClientBuilder::default()
        .request_timeout(Duration::from_secs(5))
        .build(url)
        .ok()?;
    let header: serde_json::Value = client
        .request("chain_getHeader", rpc_params![])
        .await
        .ok()?;
    let number = header["number"].as_str()?;
    u32::from_str_radix(number.trim_start_matches("0x"), 16).ok()
}

/// Wait until the node reports a head at or above a height.
async fn wait_for_head(url: &str, height: u32, limit: Duration) -> Option<u32> {
    let started = Instant::now();
    while started.elapsed() < limit {
        if let Some(head) = head_of(url).await {
            if head >= height {
                return Some(head);
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    head_of(url).await
}

/// The folder name the keystore writes one key under.
fn key_file(public: &[u8]) -> String {
    format!(
        "{}{}",
        hex::encode(cerulea_consensus::CERULEA_KEY_TYPE.0),
        hex::encode(public)
    )
}

/// The folder name the keystore writes one draw key under.
fn draw_key_file(public: &[u8]) -> String {
    format!(
        "{}{}",
        hex::encode(cerulea_consensus::CERULEA_DRAW_KEY_TYPE.0),
        hex::encode(public)
    )
}

#[test]
fn the_partial_service_opens_the_development_genesis() {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let (_, settings) = node_settings(
        &[
            "--dev",
            "--base-path",
            &base,
            "--no-prometheus",
            "--no-telemetry",
        ],
        runtime.handle().clone(),
    );
    let _entered = runtime.enter();
    let parts = crate::service::new_partial(&settings, &open_extensions())
        .expect("the partial service builds");
    let info = parts.client.info();
    assert_eq!(info.best_number, 0);
    assert_eq!(
        Some(info.genesis_hash),
        parts.client.hash(0).unwrap(),
        "the client indexes the genesis block"
    );
    // A node without a metrics registry runs without consensus metrics.
    assert!(parts.other.is_none());
}

#[test]
fn a_telemetry_endpoint_starts_the_telemetry_worker() {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let (_, settings) = node_settings(
        &[
            "--dev",
            "--base-path",
            &base,
            "--telemetry-url",
            "ws://127.0.0.1:9 0",
            "--prometheus-port",
            "0",
        ],
        runtime.handle().clone(),
    );
    assert!(settings.telemetry_endpoints.is_some());
    let _entered = runtime.enter();
    let parts = crate::service::new_partial(&settings, &open_extensions())
        .expect("the partial service builds with a telemetry endpoint");
    assert_eq!(parts.client.info().best_number, 0);
    assert!(parts.other.is_some());
}

#[test]
fn a_registry_already_holding_the_metrics_leaves_the_node_without_them() {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let (_, settings) = node_settings(
        &["--dev", "--base-path", &base, "--prometheus-port", "0"],
        runtime.handle().clone(),
    );
    let registry = settings
        .prometheus_registry()
        .expect("the node carries a metrics registry");
    cerulea_consensus::metrics::ConsensusMetrics::new(registry)
        .expect("the first registration succeeds");
    let _entered = runtime.enter();
    let parts = crate::service::new_partial(&settings, &open_extensions())
        .expect("the partial service still builds");
    assert!(parts.other.is_none());
}

#[test]
fn the_full_development_service_authors_sealed_blocks() {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let port = free_port().to_string();
    let (_, settings) = node_settings(
        &[
            "--dev",
            "--base-path",
            &base,
            "--rpc-port",
            &port,
            "--port",
            "0",
            "--prometheus-port",
            "0",
            "--no-telemetry",
            "--no-mdns",
        ],
        runtime.handle().clone(),
    );
    let task_manager = {
        let _entered = runtime.enter();
        crate::service::new_full::<sc_network::NetworkWorker<Block, H256>>(
            settings,
            &open_extensions(),
        )
        .expect("the full service starts")
    };
    let url = format!("http://127.0.0.1:{port}");
    let head = runtime
        .block_on(wait_for_head(&url, 2, Duration::from_secs(60)))
        .expect("the node answers over its interface");
    assert!(head >= 2, "the node reached block {head}");

    let detail: crate::platform_rpc::BlockDetail = runtime.block_on(async {
        let client = jsonrpsee::http_client::HttpClientBuilder::default()
            .build(&url)
            .unwrap();
        client
            .request("cerulea_getBlockDetail", rpc_params![head])
            .await
            .expect("the platform surface answers")
    });
    // The Authoring Engine sealed the block with the development key.
    assert_eq!(
        detail.author,
        Some(format!("0x{}", hex::encode(seal_key().0)))
    );
    assert_eq!(detail.slot_rank, Some(1));
    // The block tracker, the finality sync and the metrics updater run on a
    // timer. The node keeps running long enough for each to take a pass.
    runtime.block_on(async { tokio::time::sleep(Duration::from_secs(12)).await });
    let later = runtime
        .block_on(head_of(&url))
        .expect("the node still answers");
    assert!(later > head);

    drop(task_manager);
    runtime.shutdown_timeout(Duration::from_secs(20));

    // The keystore on disk holds the development seal key.
    let keystore = folder.path().join("chains/cerulea_dev/keystore");
    assert!(keystore.join(key_file(&seal_key().0)).exists());
}

#[test]
fn the_full_service_runs_on_the_second_network_backend() {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let port = free_port().to_string();
    let (_, settings) = node_settings(
        &[
            "--dev",
            "--base-path",
            &base,
            "--rpc-port",
            &port,
            "--port",
            "0",
            "--no-prometheus",
            "--no-telemetry",
            "--network-backend",
            "litep2p",
        ],
        runtime.handle().clone(),
    );
    assert!(matches!(
        settings.network.network_backend,
        Some(sc_network::config::NetworkBackendType::Litep2p)
    ));
    let task_manager = {
        let _entered = runtime.enter();
        crate::service::new_full::<sc_network::Litep2pNetworkBackend>(settings, &open_extensions())
            .expect("the full service starts on the second backend")
    };
    let url = format!("http://127.0.0.1:{port}");
    let head = runtime.block_on(wait_for_head(&url, 0, Duration::from_secs(30)));
    assert!(head.is_some(), "the node answers over its interface");
    drop(task_manager);
    runtime.shutdown_timeout(Duration::from_secs(20));
}

/// Start a local testnet node under one name and read the keys it recorded.
fn start_named(name: &str, validator: bool) -> Vec<String> {
    let runtime = test_runtime();
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap().to_string();
    let port = free_port().to_string();
    let mut arguments = vec![
        "--chain",
        "local",
        "--base-path",
        &base,
        "--name",
        name,
        "--rpc-port",
        &port,
        "--port",
        "0",
        "--no-prometheus",
        "--no-telemetry",
        "--no-mdns",
        "--node-key",
        "0000000000000000000000000000000000000000000000000000000000000007",
    ];
    if validator {
        arguments.push("--validator");
    }
    let (_, settings) = node_settings(&arguments, runtime.handle().clone());
    let task_manager = {
        let _entered = runtime.enter();
        crate::service::new_full::<sc_network::NetworkWorker<Block, H256>>(
            settings,
            &open_extensions(),
        )
        .expect("the full service starts on the local testnet")
    };
    let url = format!("http://127.0.0.1:{port}");
    let head = runtime.block_on(wait_for_head(&url, 0, Duration::from_secs(30)));
    // A named validator may author at once, so the head reads genesis or
    // the first block it sealed.
    assert!(
        matches!(head, Some(0) | Some(1)),
        "the node answers over its interface: {head:?}"
    );
    drop(task_manager);
    runtime.shutdown_timeout(Duration::from_secs(20));
    let keystore = folder.path().join("chains/cerulea_local/keystore");
    let mut files: Vec<String> = std::fs::read_dir(&keystore)
        .map(|entries| {
            entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

#[test]
fn a_named_local_validator_records_the_well_known_keys_of_its_name() {
    let files = start_named("Bob", true);
    let seal = sp_keyring::Ed25519Keyring::Bob.public();
    assert!(files.contains(&key_file(&seal.0)), "{files:?}");
    let draw = sp_keyring::Sr25519Keyring::Bob.public();
    assert!(files.contains(&draw_key_file(&draw.0)), "{files:?}");
    assert!(!files.contains(&key_file(&draw.0)), "{files:?}");
}

#[test]
fn an_unnamed_local_validator_records_a_fresh_key_pair() {
    let files = start_named("Nova", true);
    let prefix = hex::encode(cerulea_consensus::CERULEA_KEY_TYPE.0);
    let recorded: Vec<&String> = files
        .iter()
        .filter(|file| file.starts_with(&prefix))
        .collect();
    assert_eq!(recorded.len(), 1, "{files:?}");
    let draw_prefix = hex::encode(cerulea_consensus::CERULEA_DRAW_KEY_TYPE.0);
    let drawn = files
        .iter()
        .filter(|file| file.starts_with(&draw_prefix))
        .count();
    assert_eq!(drawn, 1, "{files:?}");
    let alice = key_file(&sp_keyring::Ed25519Keyring::Alice.public().0);
    assert!(!files.contains(&alice));
}

#[test]
fn a_full_node_without_the_validator_role_records_no_key() {
    let files = start_named("Observer", false);
    let prefix = hex::encode(cerulea_consensus::CERULEA_KEY_TYPE.0);
    assert!(
        files.iter().all(|file| !file.starts_with(&prefix)),
        "{files:?}"
    );
}
