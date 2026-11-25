//! A network of real validator nodes on the loopback interface.
//!
//! Each validator runs the node binary in its own process, with its own
//! keystore and database under one temporary folder. The first validator is
//! the meeting point the others dial. Tests read the committed head of each
//! node over its interface and read its log from disk.
#![allow(dead_code)]

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// The node binary cargo built for these tests.
pub const NODE: &str = env!("CARGO_BIN_EXE_cerulea-node");

/// The validators of the multi validator chain, by the name each node runs
/// under. The node takes its well known keys from that name.
pub const VALIDATORS: [&str; 5] = ["alice", "bob", "charlie", "dave", "eve"];

/// Two distinct ports nothing listens on at the moment of the call.
///
/// Both listeners stay open until both ports are known, so the two differ.
pub fn free_ports() -> (u16, u16) {
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
pub fn request(port: u16, method: &str, params: &str) -> Option<serde_json::Value> {
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

fn header_number(reply: &serde_json::Value) -> Option<u32> {
    let number = reply["result"]["number"].as_str()?;
    u32::from_str_radix(number.trim_start_matches("0x"), 16).ok()
}

/// The height of the committed head one node reports.
pub fn committed_height(port: u16) -> Option<u32> {
    let hash = request(port, "chain_getFinalizedHead", "[]")?;
    let hash = hash["result"].as_str()?.to_string();
    header_number(&request(
        port,
        "chain_getHeader",
        &format!(r#"["{hash}"]"#),
    )?)
}

/// One plain storage value of the best block one node holds, encoded.
pub fn storage_value(port: u16, pallet: &[u8], item: &[u8]) -> Option<Vec<u8>> {
    let mut key = sp_core::hashing::twox_128(pallet).to_vec();
    key.extend_from_slice(&sp_core::hashing::twox_128(item));
    let reply = request(
        port,
        "state_getStorage",
        &format!(r#"["0x{}"]"#, hex::encode(key)),
    )?;
    hex::decode(reply["result"].as_str()?.trim_start_matches("0x")).ok()
}

/// The timestamp of the best block one node holds, in milliseconds.
pub fn chain_time_ms(port: u16) -> Option<u64> {
    let bytes = storage_value(port, b"Timestamp", b"Now")?;
    let mut raw = [0u8; 8];
    raw.copy_from_slice(bytes.get(..8)?);
    Some(u64::from_le_bytes(raw))
}

/// The log lines of every node that carry the given text, for a failure
/// message.
pub fn lines_with(nodes: &[Validator], text: &str) -> String {
    nodes
        .iter()
        .map(|node| {
            let log = node.log_text();
            let matched: Vec<&str> = log.lines().filter(|line| line.contains(text)).collect();
            format!(
                "--- {} ({} lines) ---\n{}",
                node.name,
                matched.len(),
                matched.join("\n")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The wall clock in milliseconds since the epoch.
pub fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// The height of the best block one node reports.
pub fn best_height(port: u16) -> Option<u32> {
    header_number(&request(port, "chain_getHeader", "[]")?)
}

/// One running validator.
pub struct Validator {
    pub name: &'static str,
    child: Child,
    pub rpc: u16,
    pub p2p: u16,
    pub log: PathBuf,
}

impl Validator {
    /// Start one validator with its store under the folder.
    ///
    /// `run` numbers the log file, so a restart keeps the log of each run.
    /// `environment` adds settings to the process environment.
    pub fn start(
        name: &'static str,
        folder: &Path,
        bootnode: Option<&str>,
        run: u32,
        environment: &[(&str, &str)],
    ) -> Self {
        Self::start_on("multi_validator", name, folder, bootnode, run, environment)
    }

    /// Start one validator of a named chain.
    pub fn start_on(
        chain: &str,
        name: &'static str,
        folder: &Path,
        bootnode: Option<&str>,
        run: u32,
        environment: &[(&str, &str)],
    ) -> Self {
        let (rpc, p2p) = free_ports();
        let log = folder.join(format!("{name}-{run}.log"));
        let base = folder.join(name);
        let mut arguments: Vec<String> = [
            "--chain",
            chain,
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
            "--enable-cerulea-extensions",
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
        let mut command = Command::new(NODE);
        command
            .args(&arguments)
            .stdout(Stdio::null())
            .stderr(Stdio::from(file));
        for (key, value) in environment {
            command.env(key, value);
        }
        let child = command.spawn().expect("the node binary starts");
        Validator {
            name,
            child,
            rpc,
            p2p,
            log,
        }
    }

    /// The log of the current run.
    pub fn log_text(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// The network identity the node printed at start.
    pub fn identity(&self, limit: Duration) -> Option<String> {
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

    /// The committed head this node reports, zero while it cannot answer.
    pub fn committed(&self) -> u32 {
        committed_height(self.rpc).unwrap_or(0)
    }

    /// Stop the node the way an operator does.
    pub fn stop(&mut self) {
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

/// A network of validators on loopback.
pub struct Network {
    pub nodes: Vec<Validator>,
    pub bootnode: String,
    folder: PathBuf,
    runs: Vec<u32>,
    chain: &'static str,
}

impl Network {
    /// Start the five validators of the multi validator chain.
    ///
    /// `environment` names the settings each validator starts with.
    pub fn start(folder: &Path, environment: impl Fn(&str) -> Vec<(&'static str, String)>) -> Self {
        Self::start_on(folder, "multi_validator", &VALIDATORS, environment)
    }

    /// Start the named validators of a named chain.
    pub fn start_on(
        folder: &Path,
        chain: &'static str,
        names: &[&'static str],
        environment: impl Fn(&str) -> Vec<(&'static str, String)>,
    ) -> Self {
        let settings = |name: &str| environment(name);
        let own = settings(names[0]);
        let pairs: Vec<(&str, &str)> = own.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let first = Validator::start_on(chain, names[0], folder, None, 0, &pairs);
        let identity = first
            .identity(Duration::from_secs(60))
            .unwrap_or_else(|| panic!("the first node printed no identity:\n{}", first.log_text()));
        let bootnode = format!("/ip4/127.0.0.1/tcp/{}/p2p/{identity}", first.p2p);
        let mut nodes = vec![first];
        for name in &names[1..] {
            let own = settings(name);
            let pairs: Vec<(&str, &str)> = own.iter().map(|(k, v)| (*k, v.as_str())).collect();
            nodes.push(Validator::start_on(
                chain,
                name,
                folder,
                Some(&bootnode),
                0,
                &pairs,
            ));
        }
        Network {
            runs: vec![0; nodes.len()],
            nodes,
            bootnode,
            folder: folder.to_path_buf(),
            chain,
        }
    }

    /// The committed head of every node.
    pub fn committed(&self) -> Vec<u32> {
        self.nodes.iter().map(Validator::committed).collect()
    }

    /// Wait until the committed heads satisfy a condition, polling every two
    /// seconds. The answer is whether they did, and the last heads read.
    pub fn wait_until(&self, limit: Duration, done: impl Fn(&[u32]) -> bool) -> (bool, Vec<u32>) {
        let started = Instant::now();
        let mut heights = self.committed();
        while started.elapsed() < limit {
            heights = self.committed();
            if done(&heights) {
                return (true, heights);
            }
            std::thread::sleep(Duration::from_secs(2));
        }
        let met = done(&heights);
        (met, heights)
    }

    /// Stop one validator and start it again from its own store.
    pub fn restart(&mut self, index: usize, environment: &[(&str, &str)]) {
        self.nodes[index].stop();
        self.runs[index] += 1;
        let name = self.nodes[index].name;
        let bootnode = if index == 0 {
            None
        } else {
            Some(self.bootnode.clone())
        };
        self.nodes[index] = Validator::start_on(
            self.chain,
            name,
            &self.folder,
            bootnode.as_deref(),
            self.runs[index],
            environment,
        );
    }

    /// Stop every validator.
    pub fn stop(&mut self) {
        for node in self.nodes.iter_mut() {
            node.stop();
        }
    }

    /// The last forty log lines of every node, for a failure message.
    pub fn report(&self) -> String {
        self.nodes
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
    }
}
