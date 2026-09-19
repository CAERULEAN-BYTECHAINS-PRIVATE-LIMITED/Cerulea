use serde::{Serialize, Deserialize};
use clap::Parser;
use jsonrpsee::core::client::ClientT;
use jsonrpsee::rpc_params;
use jsonrpsee::ws_client::WsClientBuilder;
use anyhow::{Result, Context};
use csv::Writer;
use std::io;

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

    // Connect to node
    let client = WsClientBuilder::default()
        .build(&args.url)
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
                .request("dcf_getValidatorInferenceScore", rpc_params![validator.clone()])
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
                .request("dcf_getValidatorParticipation", rpc_params![validator.clone()])
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
            epoch_history: if args.history {
                let recent_epochs: Vec<RuntimeEpochHistory> = client
                    .request("dcf_getRecentEpochs", rpc_params![args.epochs, validator.clone()])
                    .await
                    .context("Failed to get recent epochs")?;
                recent_epochs
                    .into_iter()
                    .map(|epoch| EpochHistory {
                        epoch: epoch.epoch,
                        pos_score: epoch.pos_score,
                        poi_score: epoch.poi_score,
                        combined_score: epoch.combined_score,
                        blocks_authored: epoch.blocks_authored,
                        missed_blocks: epoch.missed_blocks,
                        inference_count: epoch.inference_count,
                        slashes: epoch.slashes,
                        rewards: epoch.rewards,
                    })
                    .collect()
            } else {
                Vec::new()
            },
        };

        validators.push(info);
    }

    // Sort validators by combined score
    validators.sort_by(|a, b| b.combined_score.cmp(&a.combined_score));

    // Print results based on format
    match args.format.as_str() {
        "table" => {
            println!("{:<40} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}", 
                "Validator", "PoS", "PoI", "Combined", "Stake", "Slashes", "Active", "Participation", "Last Active");
            println!("{}", "-".repeat(120));
            for validator in &validators {
                let (blocks_authored, total_blocks) = validator.participation;
                let participation_rate = if total_blocks > 0 {
                    (blocks_authored as f64 / total_blocks as f64 * 100.0).round() as u32
                } else {
                    0
                };
                println!("{:<40} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
                    validator.account_id,
                    validator.pos_score,
                    validator.poi_score.unwrap_or(0),
                    validator.combined_score,
                    validator.stake,
                    validator.slashing_count,
                    if validator.is_active { "Yes" } else { "No" },
                    format!("{}%", participation_rate),
                    validator.last_active,
                );

                if args.history && !validator.epoch_history.is_empty() {
                    println!("\nEpoch History:");
                    println!("{:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
                        "Epoch", "PoS", "PoI", "Combined", "Blocks", "Missed", "Inferences", "Slashes", "Rewards");
                    println!("{}", "-".repeat(90));
                    for epoch in &validator.epoch_history {
                        println!("{:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10} {:<10}",
                            epoch.epoch,
                            epoch.pos_score,
                            epoch.poi_score.unwrap_or(0),
                            epoch.combined_score,
                            epoch.blocks_authored,
                            epoch.missed_blocks,
                            epoch.inference_count,
                            epoch.slashes,
                            epoch.rewards,
                        );
                    }
                }

                if let Some(result) = validator.inference_result {
                    println!("\nInference Info:");
                    println!("Result: {}", result);
                    if let Some(confidence) = validator.inference_confidence {
                        println!("Confidence: {}%", confidence);
                    }
                    if let Some(window) = validator.challenge_window {
                        println!("Challenge Window: {} epochs", window);
                    }
                }
            }
        }
        "json" => {
            println!("{}", serde_json::to_string_pretty(&validators)?);
        }
        "csv" => {
            let mut wtr = Writer::from_writer(io::stdout());
            wtr.write_record(&[
                "Validator", "PoS", "PoI", "Combined", "Stake", "Slashes", "Active", 
                "Blocks Authored", "Total Blocks", "Last Active", "Inference Result", 
                "Inference Confidence", "Challenge Window",
            ])?;
            for validator in &validators {
                wtr.write_record(&[
                    validator.account_id.clone(),
                    validator.pos_score.to_string(),
                    validator.poi_score.map_or("N/A".to_string(), |s| s.to_string()),
                    validator.combined_score.to_string(),
                    validator.stake.to_string(),
                    validator.slashing_count.to_string(),
                    validator.is_active.to_string(),
                    validator.participation.0.to_string(),
                    validator.participation.1.to_string(),
                    validator.last_active.to_string(),
                    validator.inference_result.map_or("N/A".to_string(), |r| r.to_string()),
                    validator.inference_confidence.map_or("N/A".to_string(), |c| c.to_string()),
                    validator.challenge_window.map_or("N/A".to_string(), |w| w.to_string()),
                ])?;
            }
            wtr.flush()?;
        }
        _ => {
            eprintln!("Invalid format: {}", args.format);
            return Err(anyhow::anyhow!("Invalid output format"));
        }
    }

    Ok(())
}