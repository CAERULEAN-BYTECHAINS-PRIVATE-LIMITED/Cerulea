//! The node binary, run the way an operator runs it.
//!
//! Every subcommand that opens a service runner installs the process logger
//! and the signal handlers, so each run here starts the real binary in its
//! own process. The tests read what the binary prints, what it writes to
//! disk and what it serves over its interface while it runs.

use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

const NODE: &str = env!("CARGO_BIN_EXE_cerulea-node");

/// A port nothing listens on at the moment of the call.
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("the loopback address binds")
        .local_addr()
        .expect("the listener reports its port")
        .port()
}

/// Run the binary to completion and keep what it printed.
fn run(arguments: &[&str]) -> Output {
    Command::new(NODE)
        .args(arguments)
        .output()
        .expect("the node binary starts")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Call one interface method over HTTP with the standard library alone.
fn request(port: u16, method: &str, params: &str) -> Option<serde_json::Value> {
    use std::io::{Read, Write};
    let body = format!(r#"{{"jsonrpc":"2.0","id":1,"method":"{method}","params":{params}}}"#);
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let head = format!(
        "POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply).ok()?;
    let json = reply.split("\r\n\r\n").nth(1)?;
    serde_json::from_str(json).ok()
}

/// The head height the node reports.
fn head(port: u16) -> Option<u32> {
    let reply = request(port, "chain_getHeader", "[]")?;
    let number = reply["result"]["number"].as_str()?;
    u32::from_str_radix(number.trim_start_matches("0x"), 16).ok()
}

/// Wait until the node reports a head at or above a height.
fn wait_for_head(port: u16, height: u32, limit: Duration) -> Option<u32> {
    let started = Instant::now();
    while started.elapsed() < limit {
        if let Some(found) = head(port) {
            if found >= height {
                return Some(found);
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    head(port)
}

/// Start the node in the background.
fn start(arguments: &[&str], data_home: Option<&Path>) -> Child {
    let mut command = Command::new(NODE);
    command
        .args(arguments)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(home) = data_home {
        command.env("HOME", home).env("XDG_DATA_HOME", home);
    }
    command.spawn().expect("the node binary starts")
}

/// Ask the node to stop the way an operator does, and wait for the exit.
fn interrupt(mut child: Child) -> Output {
    let _ = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(60) {
        if child
            .try_wait()
            .expect("the child reports its state")
            .is_some()
        {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if child
        .try_wait()
        .expect("the child reports its state")
        .is_none()
    {
        let _ = child.kill();
    }
    child.wait_with_output().expect("the child output reads")
}

/// The arguments every running node in this suite shares.
fn quiet_node(base: &str, rpc: &str) -> Vec<String> {
    [
        "--base-path",
        base,
        "--rpc-port",
        rpc,
        "--port",
        "0",
        "--no-prometheus",
        "--no-telemetry",
        "--no-mdns",
    ]
    .iter()
    .map(|part| part.to_string())
    .collect()
}

fn borrowed(parts: &[String]) -> Vec<&str> {
    parts.iter().map(String::as_str).collect()
}

#[test]
fn the_chain_specification_builds_in_plain_and_raw_form() {
    let plain = run(&["build-spec", "--chain", "dev", "--disable-default-bootnode"]);
    assert!(plain.status.success(), "{}", text(&plain.stderr));
    let spec: serde_json::Value = serde_json::from_slice(&plain.stdout).unwrap();
    assert_eq!(spec["id"], serde_json::json!("cerulea_dev"));
    assert_eq!(spec["name"], serde_json::json!("Cerulea Development Chain"));
    assert_eq!(
        spec["properties"]["tokenSymbol"],
        serde_json::json!("CERULEA")
    );

    let raw = run(&[
        "build-spec",
        "--chain",
        "local",
        "--raw",
        "--disable-default-bootnode",
    ]);
    assert!(raw.status.success(), "{}", text(&raw.stderr));
    let spec: serde_json::Value = serde_json::from_slice(&raw.stdout).unwrap();
    assert_eq!(spec["id"], serde_json::json!("cerulea_local"));
    assert!(spec["genesis"]["raw"]["top"].is_object());
}

#[test]
fn the_chain_info_and_the_purge_run_against_one_base_path() {
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().to_str().unwrap();

    let info = run(&["chain-info", "--dev", "--base-path", base]);
    assert!(info.status.success(), "{}", text(&info.stderr));
    let printed = text(&info.stdout);
    assert!(printed.contains("finalized_number"), "{printed}");
    assert!(folder.path().join("chains/cerulea_dev/db").exists());

    let purge = run(&["purge-chain", "--dev", "--base-path", base, "-y"]);
    assert!(purge.status.success(), "{}", text(&purge.stderr));
    assert!(!folder.path().join("chains/cerulea_dev/db/full").exists());
}

#[test]
fn the_benchmarks_without_the_benchmark_feature_name_the_feature() {
    let pallet = run(&[
        "benchmark",
        "pallet",
        "--chain",
        "dev",
        "--pallet",
        "pallet_balances",
        "--extrinsic",
        "transfer_keep_alive",
    ]);
    assert!(!pallet.status.success());
    assert!(
        text(&pallet.stderr)
            .contains("Runtime benchmarking wasn't enabled. Use `--features runtime-benchmarks`."),
        "{}",
        text(&pallet.stderr)
    );

    let folder = tempfile::tempdir().unwrap();
    let storage = run(&[
        "benchmark",
        "storage",
        "--dev",
        "--base-path",
        folder.path().to_str().unwrap(),
        "--state-version",
        "1",
    ]);
    assert!(!storage.status.success());
    assert!(
        text(&storage.stderr)
            .contains("Storage benchmarking requires `runtime-benchmarks` feature."),
        "{}",
        text(&storage.stderr)
    );
}

#[test]
fn the_machine_benchmark_measures_this_host() {
    let folder = tempfile::tempdir().unwrap();
    let machine = run(&[
        "benchmark",
        "machine",
        "--dev",
        "--base-path",
        folder.path().to_str().unwrap(),
        "--allow-fail",
        "--verify-duration",
        "0.1",
        "--hash-duration",
        "0.1",
        "--memory-duration",
        "0.1",
        "--disk-duration",
        "0.1",
    ]);
    assert!(machine.status.success(), "{}", text(&machine.stderr));
    let printed = format!("{}{}", text(&machine.stdout), text(&machine.stderr));
    assert!(printed.contains("BLAKE2-256"), "{printed}");
}

#[test]
fn a_development_node_authors_blocks_serves_the_interface_and_stops_on_interrupt() {
    let folder = tempfile::tempdir().unwrap();
    let base = folder.path().join("node");
    let base = base.to_str().unwrap();
    let port = free_port();
    let mut arguments = quiet_node(base, &port.to_string());
    arguments.extend(
        ["--dev", "--enable-cerulea-extensions"]
            .iter()
            .map(|part| part.to_string()),
    );
    let child = start(&borrowed(&arguments), None);
    let reached = wait_for_head(port, 2, Duration::from_secs(90));

    // The author query reads the live node while it runs.
    let url = format!("http://127.0.0.1:{port}");
    let authors = run(&[
        "query-authors",
        "--rpc-url",
        &url,
        "-n",
        "2",
        "--format",
        "json",
    ]);
    let fork = run(&[
        "fork-check",
        "--local-rpc",
        &url,
        "--peer-rpc",
        &url,
        "--format",
        "json",
    ]);
    let status = request(port, "fork_getStatus", "[]");

    let output = interrupt(child);
    let reached = reached.expect("the node answers over its interface");
    assert!(reached >= 2, "the node reached block {reached}");
    assert!(output.status.success(), "{}", text(&output.stderr));

    assert!(authors.status.success(), "{}", text(&authors.stderr));
    let listed: serde_json::Value = serde_json::from_slice(&authors.stdout).unwrap();
    assert_eq!(listed["block_count"], serde_json::json!(2));
    assert_eq!(listed["authors"].as_array().map(Vec::len), Some(2));

    assert!(fork.status.success(), "{}", text(&fork.stderr));
    let reports: serde_json::Value = serde_json::from_slice(&fork.stdout).unwrap();
    assert_eq!(reports[0]["peer_id"], serde_json::json!("peer-0"));

    let status = status.expect("the fork status answers");
    assert_eq!(
        status["result"]["service"],
        serde_json::json!("fork-detection")
    );

    // The database the node wrote serves the block benchmark.
    let block = run(&[
        "benchmark",
        "block",
        "--dev",
        "--base-path",
        base,
        "--from",
        "1",
        "--to",
        "1",
        "--repeat",
        "1",
    ]);
    assert!(block.status.success(), "{}", text(&block.stderr));

    let overhead = run(&[
        "benchmark",
        "overhead",
        "--dev",
        "--base-path",
        base,
        "--warmup",
        "1",
        "--repeat",
        "1",
        "--max-ext-per-block",
        "8",
        "--weight-path",
        folder.path().to_str().unwrap(),
    ]);
    assert!(overhead.status.success(), "{}", text(&overhead.stderr));
    // The measured figures land beside the database, away from the sources.
    assert!(folder.path().join("block_weights.rs").exists());
    assert!(folder.path().join("extrinsic_weights.rs").exists());
}

#[test]
fn a_base_path_that_names_a_file_stops_the_node_with_a_problem() {
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("occupied");
    std::fs::write(&file, b"a file stands here").unwrap();
    let output = run(&[
        "--chain",
        "local",
        "--base-path",
        file.to_str().unwrap(),
        "--no-prometheus",
        "--no-telemetry",
    ]);
    assert!(!output.status.success());
}
