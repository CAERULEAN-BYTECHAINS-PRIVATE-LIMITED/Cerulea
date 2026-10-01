use anyhow::{Context, Result};
use clap::Parser;
use csv::Writer;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use serde::{Deserialize, Serialize};
use std::io::Write;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// WebSocket URL of the node
    #[arg(short, long, default_value = "ws://127.0.0.1:9944")]
    url: String,

    /// Output format (table, json, csv)
    #[arg(short, long, default_value = "table")]
    format: String,

    /// Export file path (optional)
    #[arg(short, long)]
    export: Option<String>,

    /// Show detailed history (optional)
    #[arg(short = 'H', long)]
    history: bool,

    /// Number of recent epochs to show (with --history)
    #[arg(short = 'n', long, default_value = "5")]
    epochs: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidatorInfo {
    account_id: String,
    pos_score: u64,
    poi_score: Option<u64>,
    combined_score: u64,
    stake: u128,
    slashing_count: u32,
    is_active: bool,
    participation: (u32, u32), // (blocks_authored, total_blocks)
    last_active: u32,
    inference_result: Option<u64>,
    inference_confidence: Option<u32>,
    challenge_window: Option<u32>,
    epoch_history: Vec<EpochHistory>,
}

#[derive(Debug, Serialize, Deserialize)]
struct EpochHistory {
    epoch: u32,
    pos_score: u64,
    poi_score: Option<u64>,
    combined_score: u64,
    blocks_authored: u32,
    missed_blocks: u32,
    inference_count: u32,
    slashes: u32,
    rewards: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct RuntimeEpochHistory {
    epoch: u32,
    pos_score: u64,
    poi_score: Option<u64>,
    combined_score: u64,
    blocks_authored: u32,
    missed_blocks: u32,
    inference_count: u32,
    slashes: u32,
    rewards: u32,
}

impl From<RuntimeEpochHistory> for EpochHistory {
    fn from(epoch: RuntimeEpochHistory) -> Self {
        EpochHistory {
            epoch: epoch.epoch,
            pos_score: epoch.pos_score,
            poi_score: epoch.poi_score,
            combined_score: epoch.combined_score,
            blocks_authored: epoch.blocks_authored,
            missed_blocks: epoch.missed_blocks,
            inference_count: epoch.inference_count,
            slashes: epoch.slashes,
            rewards: epoch.rewards,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidatorProfile {
    score: u64,
    blocks_authored: u32,
    missed_blocks: u32,
    slashes: u32,
    rewards: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ValidatorReport {
    validator_id: String,
    pos_score: u64,
    poi_score: Option<u64>,
    combined_score: u64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    run(args).await
}

/// Fetches every active validator from the node and prints the report.
async fn run(args: Args) -> Result<()> {
    let validators = fetch_validators(&args.url, args.history, args.epochs).await?;
    let report = render_report(&args.format, &validators, args.history)?;
    print!("{}", report);
    Ok(())
}

/// Connects to the node, reads every active validator and sorts the list by combined score.
async fn fetch_validators(url: &str, history: bool, epochs: u32) -> Result<Vec<ValidatorInfo>> {
    // Connect to node
    let client = WsClientBuilder::default()
        .build(url)
        .await
        .context("Failed to connect to node")?;

    // Get active validators
    let active_validators: Vec<String> = client
        .request("dcf_getActiveValidators", rpc_params![])
        .await
        .context("Failed to get active validators")?;

    // Create vector to store all validator info
    let mut validators = Vec::new();

    // Fetch detailed info for each validator
    for validator in active_validators {
        let info = ValidatorInfo {
            account_id: validator.clone(),
            pos_score: client
                .request("dcf_getValidatorStakeScore", rpc_params![validator.clone()])
                .await
                .context("Failed to get PoS score")?,
            poi_score: client
                .request(
                    "dcf_getValidatorInferenceScore",
                    rpc_params![validator.clone()],
                )
                .await
                .context("Failed to get inference share")?,
            combined_score: client
                .request("dcf_getValidatorScore", rpc_params![validator.clone()])
                .await
                .context("Failed to get combined score")?,
            stake: client
                .request("pos_getValidatorStake", rpc_params![validator.clone()])
                .await
                .context("Failed to get stake")?,
            slashing_count: client
                .request("pos_getSlashingCount", rpc_params![validator.clone()])
                .await
                .context("Failed to get slashing count")?,
            is_active: client
                .request("dcf_isValidatorActive", rpc_params![validator.clone()])
                .await
                .context("Failed to get validator status")?,
            participation: client
                .request(
                    "dcf_getValidatorParticipation",
                    rpc_params![validator.clone()],
                )
                .await
                .context("Failed to get validator participation")?,
            last_active: client
                .request("dcf_getValidatorLastActive", rpc_params![validator.clone()])
                .await
                .context("Failed to get last active epoch")?,
            inference_result: client
                .request("poi_getInferenceResult", rpc_params![validator.clone()])
                .await
                .context("Failed to get inference result")?,
            inference_confidence: client
                .request("poi_getInferenceConfidence", rpc_params![validator.clone()])
                .await
                .context("Failed to get inference confidence")?,
            challenge_window: client
                .request("poi_getChallengeWindow", rpc_params![validator.clone()])
                .await
                .context("Failed to get challenge window")?,
            epoch_history: if history {
                let recent_epochs: Vec<RuntimeEpochHistory> = client
                    .request(
                        "dcf_getRecentEpochs",
                        rpc_params![epochs, validator.clone()],
                    )
                    .await
                    .context("Failed to get recent epochs")?;
                recent_epochs.into_iter().map(EpochHistory::from).collect()
            } else {
                Vec::new()
            },
        };

        validators.push(info);
    }

    sort_by_combined_score(&mut validators);

    Ok(validators)
}

/// Orders validators from the highest combined score to the lowest.
fn sort_by_combined_score(validators: &mut [ValidatorInfo]) {
    validators.sort_by(|a, b| b.combined_score.cmp(&a.combined_score));
}

/// Returns the share of blocks authored as a whole percentage.
fn participation_rate(participation: (u32, u32)) -> u32 {
    let (blocks_authored, total_blocks) = participation;
    if total_blocks > 0 {
        (blocks_authored as f64 / total_blocks as f64 * 100.0).round() as u32
    } else {
        0
    }
}

/// Renders the report for one output format into a string.
fn render_report(format: &str, validators: &[ValidatorInfo], history: bool) -> Result<String> {
    let mut buffer: Vec<u8> = Vec::new();
    write_report(&mut buffer, format, validators, history)?;
    Ok(String::from_utf8(buffer)?)
}

/// Writes the report for one output format to the given writer.
fn write_report<W: Write>(
    out: &mut W,
    format: &str,
    validators: &[ValidatorInfo],
    history: bool,
) -> Result<()> {
    match format {
        "table" => write_table(out, validators, history)?,
        "json" => writeln!(out, "{}", serde_json::to_string_pretty(validators)?)?,
        "csv" => write_csv(&mut *out, validators)?,
        _ => {
            eprintln!("Invalid format: {}", format);
            return Err(anyhow::anyhow!("Invalid output format"));
        }
    }

    Ok(())
}

/// Writes the validator table with optional epoch history and inference details.
fn write_table<W: Write>(out: &mut W, validators: &[ValidatorInfo], history: bool) -> Result<()> {
    writeln!(
        out,
        "{:<40} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
        "Validator",
        "PoS",
        "PoI",
        "Combined",
        "Stake",
        "Slashes",
        "Active",
        "Participation",
        "Last Active"
    )?;
    writeln!(out, "{}", "-".repeat(120))?;
    for validator in validators {
        let participation_rate = participation_rate(validator.participation);
        writeln!(
            out,
            "{:<40} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
            validator.account_id,
            validator.pos_score,
            validator.poi_score.unwrap_or(0),
            validator.combined_score,
            validator.stake,
            validator.slashing_count,
            if validator.is_active { "Yes" } else { "No" },
            format!("{}%", participation_rate),
            validator.last_active,
        )?;

        if history && !validator.epoch_history.is_empty() {
            writeln!(out, "\nEpoch History:")?;
            writeln!(
                out,
                "{:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
                "Epoch",
                "PoS",
                "PoI",
                "Combined",
                "Blocks",
                "Missed",
                "Inferences",
                "Slashes",
                "Rewards"
            )?;
            writeln!(out, "{}", "-".repeat(90))?;
            for epoch in &validator.epoch_history {
                writeln!(
                    out,
                    "{:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
                    epoch.epoch,
                    epoch.pos_score,
                    epoch.poi_score.unwrap_or(0),
                    epoch.combined_score,
                    epoch.blocks_authored,
                    epoch.missed_blocks,
                    epoch.inference_count,
                    epoch.slashes,
                    epoch.rewards,
                )?;
            }
        }

        if let Some(result) = validator.inference_result {
            writeln!(out, "\nInference Info:")?;
            writeln!(out, "Result: {}", result)?;
            if let Some(confidence) = validator.inference_confidence {
                writeln!(out, "Confidence: {}%", confidence)?;
            }
            if let Some(window) = validator.challenge_window {
                writeln!(out, "Challenge Window: {} epochs", window)?;
            }
        }
    }

    Ok(())
}

/// Writes one CSV header row and one row per validator.
fn write_csv<W: Write>(out: W, validators: &[ValidatorInfo]) -> Result<()> {
    let mut wtr = Writer::from_writer(out);
    wtr.write_record([
        "Validator",
        "PoS",
        "PoI",
        "Combined",
        "Stake",
        "Slashes",
        "Active",
        "Blocks Authored",
        "Total Blocks",
        "Last Active",
        "Inference Result",
        "Inference Confidence",
        "Challenge Window",
    ])?;
    for validator in validators {
        wtr.write_record(&[
            validator.account_id.clone(),
            validator.pos_score.to_string(),
            validator
                .poi_score
                .map_or("N/A".to_string(), |s| s.to_string()),
            validator.combined_score.to_string(),
            validator.stake.to_string(),
            validator.slashing_count.to_string(),
            validator.is_active.to_string(),
            validator.participation.0.to_string(),
            validator.participation.1.to_string(),
            validator.last_active.to_string(),
            validator
                .inference_result
                .map_or("N/A".to_string(), |r| r.to_string()),
            validator
                .inference_confidence
                .map_or("N/A".to_string(), |c| c.to_string()),
            validator
                .challenge_window
                .map_or("N/A".to_string(), |w| w.to_string()),
        ])?;
    }
    wtr.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;
    use clap::CommandFactory;
    use jsonrpsee::server::{Server, ServerHandle};
    use jsonrpsee::types::{ErrorObjectOwned, Params};
    use jsonrpsee::RpcModule;
    use serde_json::{json, Value};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    // One validator as the test node reports it over RPC.
    struct NodeValidator {
        account: &'static str,
        stake_score: u64,
        inference_score: Option<u64>,
        combined: u64,
        stake: u128,
        slashes: u32,
        active: bool,
        participation: (u32, u32),
        last_active: u32,
        inference_result: Option<u64>,
        inference_confidence: Option<u32>,
        challenge_window: Option<u32>,
    }

    // A validator with every inference field present.
    const STEADY: NodeValidator = NodeValidator {
        account: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
        stake_score: 1200,
        inference_score: Some(900),
        combined: 1080,
        stake: 2_500_000_000_000,
        slashes: 0,
        active: true,
        participation: (7, 10),
        last_active: 42,
        inference_result: Some(7),
        inference_confidence: Some(88),
        challenge_window: Some(4),
    };

    // The validator with the highest combined score and no inference record.
    const LEADING: NodeValidator = NodeValidator {
        account: "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
        stake_score: 1500,
        inference_score: Some(1400),
        combined: 1460,
        stake: 3_000_000_000_000,
        slashes: 1,
        active: true,
        participation: (9, 10),
        last_active: 43,
        inference_result: None,
        inference_confidence: None,
        challenge_window: None,
    };

    // An inactive validator that authored no blocks.
    const IDLE: NodeValidator = NodeValidator {
        account: "5FLSigC9HGRKVhB9FiEo4Y3koPsNmBmLJbpXg2mp1hXcS59Y",
        stake_score: 600,
        inference_score: None,
        combined: 540,
        stake: 1_000_000_000_000,
        slashes: 2,
        active: false,
        participation: (0, 0),
        last_active: 30,
        inference_result: Some(3),
        inference_confidence: None,
        challenge_window: Some(2),
    };

    // Every per-validator method the score checker calls.
    const FIELD_METHODS: [&str; 11] = [
        "dcf_getValidatorStakeScore",
        "dcf_getValidatorInferenceScore",
        "dcf_getValidatorScore",
        "pos_getValidatorStake",
        "pos_getSlashingCount",
        "dcf_isValidatorActive",
        "dcf_getValidatorParticipation",
        "dcf_getValidatorLastActive",
        "poi_getInferenceResult",
        "poi_getInferenceConfidence",
        "poi_getChallengeWindow",
    ];

    // State the test node serves. The counter records history requests.
    struct TestNode {
        validators: Vec<NodeValidator>,
        recent_epoch_calls: AtomicUsize,
    }

    impl TestNode {
        fn new(validators: Vec<NodeValidator>) -> Arc<Self> {
            Arc::new(Self {
                validators,
                recent_epoch_calls: AtomicUsize::new(0),
            })
        }

        fn lookup(&self, account: &str) -> Result<&NodeValidator, ErrorObjectOwned> {
            self.validators
                .iter()
                .find(|v| v.account == account)
                .ok_or_else(|| {
                    ErrorObjectOwned::owned(
                        -32602,
                        format!("Unknown validator {account}"),
                        None::<()>,
                    )
                })
        }

        fn validator_for(&self, params: &Params<'_>) -> Result<&NodeValidator, ErrorObjectOwned> {
            let account: String = params.one()?;
            self.lookup(&account)
        }
    }

    fn read_field(method: &str, v: &NodeValidator) -> Value {
        match method {
            "dcf_getValidatorStakeScore" => json!(v.stake_score),
            "dcf_getValidatorInferenceScore" => json!(v.inference_score),
            "dcf_getValidatorScore" => json!(v.combined),
            "pos_getValidatorStake" => json!(v.stake),
            "pos_getSlashingCount" => json!(v.slashes),
            "dcf_isValidatorActive" => json!(v.active),
            "dcf_getValidatorParticipation" => json!(v.participation),
            "dcf_getValidatorLastActive" => json!(v.last_active),
            "poi_getInferenceResult" => json!(v.inference_result),
            "poi_getInferenceConfidence" => json!(v.inference_confidence),
            "poi_getChallengeWindow" => json!(v.challenge_window),
            other => panic!("test node has no field for {other}"),
        }
    }

    // Recent epochs in the shape the runtime returns them.
    fn recent_epochs(v: &NodeValidator, count: u32) -> Value {
        Value::Array(
            (0..count)
                .map(|i| {
                    let slashes = if i == 0 { v.slashes } else { 0 };
                    json!({
                        "epoch": v.last_active - i,
                        "pos_score": v.stake_score - 10 * u64::from(i),
                        "poi_score": v.inference_score,
                        "combined_score": v.combined - 10 * u64::from(i),
                        "blocks_authored": 6,
                        "missed_blocks": i,
                        "inference_count": 3 + i,
                        "slashes": slashes,
                        "rewards": 2
                    })
                })
                .collect(),
        )
    }

    // Starts a real RPC server on a free local port. Methods named in `missing` stay unregistered.
    async fn start_node(node: Arc<TestNode>, missing: &[&str]) -> (String, ServerHandle) {
        let server = Server::builder()
            .build("127.0.0.1:0")
            .await
            .expect("test node binds a local port");
        let addr = server.local_addr().expect("test node reports its address");
        let mut module = RpcModule::from_arc(node);

        module
            .register_method("dcf_getActiveValidators", |_, node, _| {
                Value::Array(node.validators.iter().map(|v| json!(v.account)).collect())
            })
            .expect("method name is unique");

        for method in FIELD_METHODS {
            if missing.contains(&method) {
                continue;
            }
            module
                .register_method(
                    method,
                    move |params, node, _| -> Result<Value, ErrorObjectOwned> {
                        let validator = node.validator_for(&params)?;
                        Ok(read_field(method, validator))
                    },
                )
                .expect("method name is unique");
        }

        if !missing.contains(&"dcf_getRecentEpochs") {
            module
                .register_method(
                    "dcf_getRecentEpochs",
                    |params, node, _| -> Result<Value, ErrorObjectOwned> {
                        let (count, account): (u32, String) = params.parse()?;
                        node.recent_epoch_calls.fetch_add(1, Ordering::SeqCst);
                        let validator = node.lookup(&account)?;
                        Ok(recent_epochs(validator, count))
                    },
                )
                .expect("method name is unique");
        }

        (format!("ws://{addr}"), server.start(module))
    }

    // Returns a local port with no listener behind it.
    fn unused_local_port() -> u16 {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("free port binds");
        listener
            .local_addr()
            .expect("listener has an address")
            .port()
    }

    fn unique_temp_path(label: &str) -> std::path::PathBuf {
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock runs after 1970")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "cerulea-{label}-{}-{nanos}-{}.csv",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ))
    }

    fn info_from(v: &NodeValidator, epoch_history: Vec<EpochHistory>) -> ValidatorInfo {
        ValidatorInfo {
            account_id: v.account.to_string(),
            pos_score: v.stake_score,
            poi_score: v.inference_score,
            combined_score: v.combined,
            stake: v.stake,
            slashing_count: v.slashes,
            is_active: v.active,
            participation: v.participation,
            last_active: v.last_active,
            inference_result: v.inference_result,
            inference_confidence: v.inference_confidence,
            challenge_window: v.challenge_window,
            epoch_history,
        }
    }

    fn history_for(v: &NodeValidator, count: u32) -> Vec<EpochHistory> {
        let runtime: Vec<RuntimeEpochHistory> =
            serde_json::from_value(recent_epochs(v, count)).expect("epoch JSON decodes");
        runtime.into_iter().map(EpochHistory::from).collect()
    }

    fn tokens(line: &str) -> Vec<&str> {
        line.split_whitespace().collect()
    }

    #[test]
    fn argument_definition_passes_clap_checks() {
        Args::command().debug_assert();
    }

    #[test]
    fn default_arguments_match_documented_values() {
        let args = Args::try_parse_from(["score-checker"]).expect("defaults parse");
        assert_eq!(args.url, "ws://127.0.0.1:9944");
        assert_eq!(args.format, "table");
        assert_eq!(args.export, None);
        assert!(!args.history);
        assert_eq!(args.epochs, 5);
    }

    #[test]
    fn short_flags_set_every_field() {
        let args = Args::try_parse_from([
            "score-checker",
            "-u",
            "ws://10.0.0.5:9944",
            "-f",
            "json",
            "-e",
            "scores.csv",
            "-H",
            "-n",
            "12",
        ])
        .expect("short flags parse");
        assert_eq!(args.url, "ws://10.0.0.5:9944");
        assert_eq!(args.format, "json");
        assert_eq!(args.export.as_deref(), Some("scores.csv"));
        assert!(args.history);
        assert_eq!(args.epochs, 12);
    }

    #[test]
    fn long_flags_set_every_field() {
        let args = Args::try_parse_from([
            "score-checker",
            "--url",
            "ws://validator-3.cerulea.internal:9944",
            "--format",
            "csv",
            "--export",
            "/var/tmp/scores.csv",
            "--history",
            "--epochs",
            "3",
        ])
        .expect("long flags parse");
        assert_eq!(args.url, "ws://validator-3.cerulea.internal:9944");
        assert_eq!(args.format, "csv");
        assert_eq!(args.export.as_deref(), Some("/var/tmp/scores.csv"));
        assert!(args.history);
        assert_eq!(args.epochs, 3);
    }

    #[test]
    fn text_epoch_count_is_rejected() {
        let error = Args::try_parse_from(["score-checker", "-n", "many"])
            .expect_err("a word is rejected as an epoch count");
        assert_eq!(error.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn unknown_flag_is_rejected() {
        let error = Args::try_parse_from(["score-checker", "--verbose"])
            .expect_err("an undeclared flag is rejected");
        assert_eq!(error.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn participation_rate_rounds_to_whole_percent() {
        assert_eq!(participation_rate((7, 10)), 70);
        assert_eq!(participation_rate((2, 3)), 67);
        assert_eq!(participation_rate((1, 3)), 33);
        assert_eq!(participation_rate((10, 10)), 100);
        assert_eq!(participation_rate((0, 5)), 0);
    }

    #[test]
    fn participation_rate_is_zero_when_no_blocks_exist() {
        assert_eq!(participation_rate((0, 0)), 0);
        assert_eq!(participation_rate((4, 0)), 0);
    }

    #[test]
    fn runtime_epoch_converts_field_by_field() {
        let runtime = RuntimeEpochHistory {
            epoch: 17,
            pos_score: 1320,
            poi_score: Some(870),
            combined_score: 1140,
            blocks_authored: 11,
            missed_blocks: 1,
            inference_count: 5,
            slashes: 0,
            rewards: 3,
        };
        let epoch = EpochHistory::from(runtime);
        assert_eq!(epoch.epoch, 17);
        assert_eq!(epoch.pos_score, 1320);
        assert_eq!(epoch.poi_score, Some(870));
        assert_eq!(epoch.combined_score, 1140);
        assert_eq!(epoch.blocks_authored, 11);
        assert_eq!(epoch.missed_blocks, 1);
        assert_eq!(epoch.inference_count, 5);
        assert_eq!(epoch.slashes, 0);
        assert_eq!(epoch.rewards, 3);
    }

    #[test]
    fn sort_places_highest_combined_score_first() {
        let mut validators = vec![
            info_from(&STEADY, Vec::new()),
            info_from(&IDLE, Vec::new()),
            info_from(&LEADING, Vec::new()),
        ];
        sort_by_combined_score(&mut validators);
        let scores: Vec<u64> = validators.iter().map(|v| v.combined_score).collect();
        assert_eq!(scores, [1460, 1080, 540]);
    }

    #[test]
    fn sort_keeps_input_order_for_equal_scores() {
        let first = info_from(&STEADY, Vec::new());
        let mut second = info_from(&IDLE, Vec::new());
        second.combined_score = first.combined_score;
        let mut validators = vec![first, second];
        sort_by_combined_score(&mut validators);
        assert_eq!(validators[0].account_id, STEADY.account);
        assert_eq!(validators[1].account_id, IDLE.account);
    }

    #[test]
    fn table_report_prints_header_separator_and_rows() {
        let validators = vec![
            info_from(&LEADING, Vec::new()),
            info_from(&IDLE, Vec::new()),
        ];
        let text = render_report("table", &validators, false).expect("table renders");
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(
            tokens(lines[0]),
            [
                "Validator",
                "PoS",
                "PoI",
                "Combined",
                "Stake",
                "Slashes",
                "Active",
                "Participation",
                "Last",
                "Active"
            ]
        );
        assert_eq!(lines[1], "-".repeat(120));
        assert_eq!(
            tokens(lines[2]),
            [
                LEADING.account,
                "1500",
                "1400",
                "1460",
                "3000000000000",
                "1",
                "Yes",
                "90%",
                "43"
            ]
        );
        // A missing inference score prints as zero. An inactive validator prints No.
        assert_eq!(
            tokens(lines[3]),
            [
                IDLE.account,
                "600",
                "0",
                "540",
                "1000000000000",
                "2",
                "No",
                "0%",
                "30"
            ]
        );
        // The idle validator carries a result and a window with no confidence.
        assert_eq!(
            &lines[4..],
            &[
                "",
                "Inference Info:",
                "Result: 3",
                "Challenge Window: 2 epochs"
            ]
        );
    }

    #[test]
    fn table_report_pads_short_account_to_forty_columns() {
        let mut validator = info_from(&LEADING, Vec::new());
        validator.account_id = "validator-7".to_string();
        let text = render_report("table", &[validator], false).expect("table renders");
        let row = text.lines().nth(2).expect("row line exists");
        assert!(row.starts_with("validator-7"));
        assert_eq!(&row[41..45], "1500");
    }

    #[test]
    fn table_report_prints_epoch_history_when_requested() {
        let validators = vec![info_from(&STEADY, history_for(&STEADY, 2))];
        let text = render_report("table", &validators, true).expect("table renders");
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(lines[3], "");
        assert_eq!(lines[4], "Epoch History:");
        assert_eq!(
            tokens(lines[5]),
            [
                "Epoch",
                "PoS",
                "PoI",
                "Combined",
                "Blocks",
                "Missed",
                "Inferences",
                "Slashes",
                "Rewards"
            ]
        );
        assert_eq!(lines[6], "-".repeat(90));
        assert_eq!(
            tokens(lines[7]),
            ["42", "1200", "900", "1080", "6", "0", "3", "0", "2"]
        );
        assert_eq!(
            tokens(lines[8]),
            ["41", "1190", "900", "1070", "6", "1", "4", "0", "2"]
        );
        assert_eq!(
            &lines[9..],
            &[
                "",
                "Inference Info:",
                "Result: 7",
                "Confidence: 88%",
                "Challenge Window: 4 epochs"
            ]
        );
    }

    #[test]
    fn table_report_omits_epoch_history_without_flag() {
        let validators = vec![info_from(&STEADY, history_for(&STEADY, 2))];
        let text = render_report("table", &validators, false).expect("table renders");
        assert!(!text.contains("Epoch History:"));
        assert!(text.contains("Confidence: 88%"));
    }

    #[test]
    fn table_report_omits_epoch_section_for_empty_history() {
        let validators = vec![info_from(&LEADING, Vec::new())];
        let text = render_report("table", &validators, true).expect("table renders");
        assert!(!text.contains("Epoch History:"));
        assert!(!text.contains("Inference Info:"));
        assert_eq!(text.lines().count(), 3);
    }

    #[test]
    fn json_report_carries_every_field() {
        let validators = vec![
            info_from(&STEADY, history_for(&STEADY, 1)),
            info_from(&IDLE, Vec::new()),
        ];
        let text = render_report("json", &validators, true).expect("JSON renders");
        let parsed: Value = serde_json::from_str(&text).expect("output is valid JSON");

        assert_eq!(parsed.as_array().map(Vec::len), Some(2));
        assert_eq!(parsed[0]["account_id"], STEADY.account);
        assert_eq!(parsed[0]["stake"], json!(2_500_000_000_000u64));
        assert_eq!(parsed[0]["participation"], json!([7, 10]));
        assert_eq!(parsed[0]["epoch_history"][0]["epoch"], 42);
        assert_eq!(parsed[0]["epoch_history"][0]["inference_count"], 3);
        assert_eq!(parsed[1]["poi_score"], Value::Null);
        assert_eq!(parsed[1]["is_active"], false);
        assert_eq!(parsed[1]["epoch_history"], json!([]));
    }

    #[test]
    fn csv_report_marks_missing_values() {
        let validators = vec![
            info_from(&LEADING, Vec::new()),
            info_from(&IDLE, Vec::new()),
        ];
        let text = render_report("csv", &validators, false).expect("CSV renders");
        let mut reader = csv::Reader::from_reader(text.as_bytes());

        let headers: Vec<String> = reader
            .headers()
            .expect("header row exists")
            .iter()
            .map(str::to_string)
            .collect();
        assert_eq!(
            headers,
            [
                "Validator",
                "PoS",
                "PoI",
                "Combined",
                "Stake",
                "Slashes",
                "Active",
                "Blocks Authored",
                "Total Blocks",
                "Last Active",
                "Inference Result",
                "Inference Confidence",
                "Challenge Window"
            ]
        );

        let rows: Vec<csv::StringRecord> = reader
            .records()
            .collect::<std::result::Result<_, _>>()
            .expect("every row parses");
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[0].iter().collect::<Vec<_>>(),
            [
                LEADING.account,
                "1500",
                "1400",
                "1460",
                "3000000000000",
                "1",
                "true",
                "9",
                "10",
                "43",
                "N/A",
                "N/A",
                "N/A"
            ]
        );
        assert_eq!(
            rows[1].iter().collect::<Vec<_>>(),
            [
                IDLE.account,
                "600",
                "N/A",
                "540",
                "1000000000000",
                "2",
                "false",
                "0",
                "0",
                "30",
                "3",
                "N/A",
                "2"
            ]
        );
    }

    #[test]
    fn csv_report_written_to_file_reads_back() {
        let path = unique_temp_path("score-report");
        {
            let mut file = std::fs::File::create(&path).expect("temp file opens");
            let validators = vec![info_from(&STEADY, Vec::new())];
            write_report(&mut file, "csv", &validators, false).expect("CSV writes to file");
        }
        let text = std::fs::read_to_string(&path).expect("temp file reads back");
        std::fs::remove_file(&path).expect("temp file removes");

        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("Validator,PoS,PoI,Combined"));
        assert_eq!(
            lines[1],
            format!(
                "{},1200,900,1080,2500000000000,0,true,7,10,42,7,88,4",
                STEADY.account
            )
        );
    }

    #[test]
    fn unknown_format_returns_error_and_writes_nothing() {
        let validators = vec![info_from(&STEADY, Vec::new())];
        let mut buffer: Vec<u8> = Vec::new();
        let error = write_report(&mut buffer, "xml", &validators, false)
            .expect_err("xml is an unsupported format");
        assert_eq!(error.to_string(), "Invalid output format");
        assert!(buffer.is_empty());
    }

    #[tokio::test]
    async fn fetch_collects_and_sorts_validators_from_node() {
        let node = TestNode::new(vec![STEADY, LEADING, IDLE]);
        let (url, _server) = start_node(node.clone(), &[]).await;

        let validators = fetch_validators(&url, false, 5)
            .await
            .expect("node answers every call");

        let accounts: Vec<&str> = validators.iter().map(|v| v.account_id.as_str()).collect();
        assert_eq!(accounts, [LEADING.account, STEADY.account, IDLE.account]);

        let leading = &validators[0];
        assert_eq!(leading.pos_score, 1500);
        assert_eq!(leading.poi_score, Some(1400));
        assert_eq!(leading.combined_score, 1460);
        assert_eq!(leading.stake, 3_000_000_000_000);
        assert_eq!(leading.slashing_count, 1);
        assert!(leading.is_active);
        assert_eq!(leading.participation, (9, 10));
        assert_eq!(leading.last_active, 43);
        assert_eq!(leading.inference_result, None);
        assert_eq!(leading.inference_confidence, None);
        assert_eq!(leading.challenge_window, None);

        let idle = &validators[2];
        assert_eq!(idle.poi_score, None);
        assert!(!idle.is_active);
        assert_eq!(idle.participation, (0, 0));
        assert_eq!(idle.inference_result, Some(3));
        assert_eq!(idle.inference_confidence, None);
        assert_eq!(idle.challenge_window, Some(2));

        assert!(validators.iter().all(|v| v.epoch_history.is_empty()));
        assert_eq!(node.recent_epoch_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn fetch_with_history_requests_recent_epochs() {
        let node = TestNode::new(vec![STEADY, LEADING, IDLE]);
        let (url, _server) = start_node(node.clone(), &[]).await;

        let validators = fetch_validators(&url, true, 3)
            .await
            .expect("node answers every call");

        assert_eq!(node.recent_epoch_calls.load(Ordering::SeqCst), 3);
        assert!(validators.iter().all(|v| v.epoch_history.len() == 3));

        let leading = &validators[0].epoch_history;
        let epochs: Vec<u32> = leading.iter().map(|e| e.epoch).collect();
        let stake_scores: Vec<u64> = leading.iter().map(|e| e.pos_score).collect();
        let missed: Vec<u32> = leading.iter().map(|e| e.missed_blocks).collect();
        assert_eq!(epochs, [43, 42, 41]);
        assert_eq!(stake_scores, [1500, 1490, 1480]);
        assert_eq!(missed, [0, 1, 2]);
        assert_eq!(leading[0].slashes, 1);
        assert_eq!(leading[1].slashes, 0);

        let idle = &validators[2].epoch_history;
        assert!(idle.iter().all(|e| e.poi_score.is_none()));
        assert_eq!(idle[2].combined_score, 520);
    }

    #[tokio::test]
    async fn fetch_with_no_active_validators_returns_empty_list() {
        let (url, _server) = start_node(TestNode::new(Vec::new()), &[]).await;
        let validators = fetch_validators(&url, true, 5)
            .await
            .expect("empty validator set succeeds");
        assert!(validators.is_empty());
    }

    #[tokio::test]
    async fn fetch_reports_unreachable_node() {
        let url = format!("ws://127.0.0.1:{}", unused_local_port());
        let error = fetch_validators(&url, false, 5)
            .await
            .expect_err("a closed port refuses the connection");
        assert_eq!(error.to_string(), "Failed to connect to node");
    }

    #[tokio::test]
    async fn fetch_names_the_failing_call() {
        let (url, _server) =
            start_node(TestNode::new(vec![STEADY]), &["poi_getChallengeWindow"]).await;
        let error = fetch_validators(&url, false, 5)
            .await
            .expect_err("a missing method fails the fetch");
        assert_eq!(error.to_string(), "Failed to get challenge window");
    }

    #[tokio::test]
    async fn fetch_calls_recent_epochs_only_with_history() {
        let (url, _server) =
            start_node(TestNode::new(vec![STEADY]), &["dcf_getRecentEpochs"]).await;

        let validators = fetch_validators(&url, false, 4)
            .await
            .expect("history stays off");
        assert_eq!(validators.len(), 1);

        let error = fetch_validators(&url, true, 4)
            .await
            .expect_err("history needs the recent epochs method");
        assert_eq!(error.to_string(), "Failed to get recent epochs");
    }

    #[tokio::test]
    async fn run_prints_each_supported_format() {
        let (url, _server) = start_node(TestNode::new(vec![STEADY, IDLE]), &[]).await;
        for format in ["table", "json", "csv"] {
            let args = Args::try_parse_from([
                "score-checker",
                "--url",
                url.as_str(),
                "--format",
                format,
                "--history",
                "--epochs",
                "2",
            ])
            .expect("arguments parse");
            run(args)
                .await
                .expect("run succeeds for a supported format");
        }
    }

    #[tokio::test]
    async fn run_rejects_unknown_format_after_fetch() {
        let node = TestNode::new(vec![STEADY]);
        let (url, _server) = start_node(node.clone(), &[]).await;
        let args = Args::try_parse_from([
            "score-checker",
            "--url",
            url.as_str(),
            "--format",
            "yaml",
            "--history",
        ])
        .expect("arguments parse");

        let error = run(args).await.expect_err("yaml is an unsupported format");
        assert_eq!(error.to_string(), "Invalid output format");
        assert_eq!(node.recent_epoch_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn run_fails_when_node_is_unreachable() {
        let url = format!("ws://127.0.0.1:{}", unused_local_port());
        let args = Args::try_parse_from(["score-checker", "--url", url.as_str()])
            .expect("arguments parse");
        let error = run(args)
            .await
            .expect_err("a closed port refuses the connection");
        assert_eq!(error.to_string(), "Failed to connect to node");
    }
}
