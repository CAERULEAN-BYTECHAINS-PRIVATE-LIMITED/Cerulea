//! Finality from genesis on a network of real validator services.
//!
//! Five validator nodes run from the node binary on the loopback interface,
//! each in its own process with its own keystore and database. They start from
//! the genesis of the multi validator chain, author blocks, gossip commit
//! votes and commit checkpoints. The test asserts two things.
//!
//! 1. **Finality advances.** Every node reports a committed head above height
//!    ten within the time limit.
//! 2. **No runtime refusal.** The commit vote gossip validator of every node
//!    records no refusal for a failed runtime answer. A single node never
//!    gossips a vote to anyone, so only a network shows this.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

const NODE: &str = env!("CARGO_BIN_EXE_cerulea-node");

/// The validators of the multi validator chain, by the name each node runs
/// under. The node takes its well known keys from that name.
const VALIDATORS: [&str; 5] = ["alice", "bob", "charlie", "dave", "eve"];

/// The committed height every node must pass.
const COMMITTED_ABOVE: u32 = 10;

/// The time the network has to commit above that height.
const LIMIT: Duration = Duration::from_secs(300);

/// Two distinct ports nothing listens on at the moment of the call.
///
/// Both listeners stay open until both ports are known, so the two differ.
fn free_ports() -> (u16, u16) {
    let bind = || std::net::TcpListener::bind("127.0.0.1:0").expect("the loopback address binds");
    let (first, second) = (bind(), bind());
    let port = |listener: &std::net::TcpListener| {
        listener
            .local_addr()
            .expect("the listener reports its port")
            .port()
    };
    (port(&first), port(&second))
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
        // The interface checks the host header, port included.
        "POST / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        port,
        body.len()
    );
    stream.write_all(head.as_bytes()).ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut reply = String::new();
    stream.read_to_string(&mut reply).ok()?;
    let json = reply.split("\r\n\r\n").nth(1)?;
    serde_json::from_str(json).ok()
}

/// The height of the committed head one node reports.
fn committed_height(port: u16) -> Option<u32> {
    let hash = request(port, "chain_getFinalizedHead", "[]")?;
    let hash = hash["result"].as_str()?.to_string();
    let header = request(port, "chain_getHeader", &format!(r#"["{hash}"]"#))?;
    let number = header["result"]["number"].as_str()?;
    u32::from_str_radix(number.trim_start_matches("0x"), 16).ok()
}

/// One running validator.
struct Validator {
    name: &'static str,
    child: Child,
    rpc: u16,
    log: PathBuf,
}

impl Validator {
    fn start(name: &'static str, folder: &Path, bootnode: Option<&str>) -> (Self, u16) {
        let (rpc, p2p) = free_ports();
        let log = folder.join(format!("{name}.log"));
        let base = folder.join(name);
        let mut arguments: Vec<String> = [
            "--chain",
            "multi_validator",
            "--name",
            name,
            "--validator",
            "--base-path",
            base.to_str().expect("the folder path is text"),
            "--port",
            &p2p.to_string(),
            "--rpc-port",
            &rpc.to_string(),
            "--no-prometheus",
            "--no-telemetry",
            "--no-mdns",
            "-l",
            "cerulea_consensus::dvf_gossip=debug",
        ]
        .iter()
        .map(|part| part.to_string())
        .collect();
        if let Some(address) = bootnode {
            arguments.push("--bootnodes".into());
            arguments.push(address.into());
        }
        let file = File::create(&log).expect("the log file opens");
        let child = Command::new(NODE)
            .args(&arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::from(file))
            .spawn()
            .expect("the node binary starts");
        (
            Validator {
                name,
                child,
                rpc,
                log,
            },
            p2p,
        )
    }

    fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// The network identity the node printed at start.
    fn identity(&self, limit: Duration) -> Option<String> {
        let started = Instant::now();
        while started.elapsed() < limit {
            let text = self.log_text();
            if let Some(found) = text
                .lines()
                .find_map(|line| line.split("Local node identity is: ").nth(1))
            {
                return Some(found.trim().to_string());
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        None
    }

    fn stop(&mut self) {
        let _ = Command::new("kill")
            .args(["-INT", &self.child.id().to_string()])
            .status();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(30) {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(200));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for Validator {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn five_validators_commit_from_genesis_without_runtime_refusals() {
    let folder = tempfile::tempdir().expect("a temporary folder opens");
    let (first, first_port) = Validator::start(VALIDATORS[0], folder.path(), None);
    let identity = first
        .identity(Duration::from_secs(60))
        .unwrap_or_else(|| panic!("the first node printed no identity:\n{}", first.log_text()));
    let bootnode = format!("/ip4/127.0.0.1/tcp/{first_port}/p2p/{identity}");
    let mut network = vec![first];
    for name in &VALIDATORS[1..] {
        let (node, _) = Validator::start(name, folder.path(), Some(&bootnode));
        network.push(node);
    }

    let started = Instant::now();
    let mut heights = vec![0u32; network.len()];
    while started.elapsed() < LIMIT {
        for (index, node) in network.iter().enumerate() {
            if let Some(height) = committed_height(node.rpc) {
                heights[index] = height;
            }
        }
        if heights.iter().all(|height| *height > COMMITTED_ABOVE) {
            break;
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    for node in network.iter_mut() {
        node.stop();
    }

    let refusals: Vec<(&str, usize)> = network
        .iter()
        .map(|node| {
            (
                node.name,
                node.log_text().matches("runtime_api_error").count(),
            )
        })
        .collect();
    let report = || {
        network
            .iter()
            .map(|node| {
                let text = node.log_text();
                let tail: Vec<&str> = text.lines().rev().take(40).collect();
                format!(
                    "--- {} ---\n{}",
                    node.name,
                    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert!(
        heights.iter().all(|height| *height > COMMITTED_ABOVE),
        "committed heights {:?} after {:?}; runtime refusals {:?}\n{}",
        heights,
        started.elapsed(),
        refusals,
        report()
    );
    assert!(
        refusals.iter().all(|(_, count)| *count == 0),
        "the gossip validator refused votes on a failed runtime answer: {:?}",
        refusals
    );
}
