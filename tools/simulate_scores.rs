use std::fs::File;
use rand::Rng;
use serde::Serialize;

// Constants
const DEFAULT_EPOCHS: u32 = 10;
const DEFAULT_VALIDATORS: u32 = 5;
const DEFAULT_POS_WEIGHT: u64 = 60;
const DEFAULT_POI_WEIGHT: u64 = 40;
const DEFAULT_SCORE_DECAY: u64 = 5;
const DEFAULT_MIN_SCORE: u64 = 100;

// Validator behavior types
#[derive(Debug, Clone, Copy)]
enum ValidatorBehavior {
    Honest,
    Lazy,
    Malicious,
    Inconsistent,
}

// Validator scores
#[derive(Debug, Clone, Copy)]
struct Scores {
    pos_score: u64,
    poi_score: u64,
    final_score: u64,
}

// Validator state
#[derive(Debug, Clone)]
struct Validator {
    id: u32,
    behavior: ValidatorBehavior,
    scores: Scores,
    participation: u32,
    missed_blocks: u32,
    inference_success: u32,
    inference_failure: u32,
    slashes: u32,
    rewards: u32,
}

// Simulation start values
#[derive(Debug, Clone)]
struct SimulationConfig {
    num_epochs: u32,
    num_validators: u32,
    pos_weight: u64,
    poi_weight: u64,
    score_decay: u64,
    min_score: u64,
    participation_rate: f64,
    inference_accuracy: f64,
    slash_probability: f64,
    reward_probability: f64,
}

// Simulation results
#[derive(Debug, Serialize)]
struct SimulationResult {
    epoch: u32,
    validator_id: u32,
    pos_score: u64,
    poi_score: u64,
    final_score: u64,
    participation: u32,
    missed_blocks: u32,
    inference_success: u32,
    inference_failure: u32,
    slashes: u32,
    rewards: u32,
}

impl Validator {
    fn new(id: u32, behavior: ValidatorBehavior) -> Self {
        let scores = Scores {
            pos_score: rand::thread_rng().gen_range(1000..2000),
            poi_score: rand::thread_rng().gen_range(1000..2000),
            final_score: 0,
        };
        
        Self {
            id,
            behavior,
            scores,
            participation: 0,
            missed_blocks: 0,
            inference_success: 0,
            inference_failure: 0,
            slashes: 0,
            rewards: 0,
        }
    }

    fn update_scores(&mut self, config: &SimulationConfig) {
        // Apply score decay
        self.scores.pos_score = self.scores.pos_score.saturating_sub(config.score_decay);
        self.scores.poi_score = self.scores.poi_score.saturating_sub(config.score_decay);
        
        // Calculate final score
        self.scores.final_score = (self.scores.pos_score * config.pos_weight 
            + self.scores.poi_score * config.poi_weight) / 100;
        
        // Ensure minimum score
        self.scores.final_score = self.scores.final_score.max(config.min_score);
    }
}

fn simulate_epoch(validators: &mut Vec<Validator>, config: &SimulationConfig) -> Vec<SimulationResult> {
    let mut results = Vec::new();
    let mut rng = rand::thread_rng();
    
    for validator in validators.iter_mut() {
        // Update scores
        validator.update_scores(config);
        
        // Simulate participation
        if rng.gen_bool(config.participation_rate) {
            validator.participation += 1;
        } else {
            validator.missed_blocks += 1;
        }
        
        // Simulate inference
        if rng.gen_bool(config.inference_accuracy) {
            validator.inference_success += 1;
            validator.scores.poi_score += 100;
        } else {
            validator.inference_failure += 1;
            validator.scores.poi_score = validator.scores.poi_score.saturating_sub(50);
        }
        
        // Simulate slashing
        if rng.gen_bool(config.slash_probability) {
            validator.scores.pos_score = validator.scores.pos_score.saturating_sub(500);
            validator.slashes += 1;
        }
        
        // Simulate rewards
        if rng.gen_bool(config.reward_probability) {
            validator.scores.pos_score += 200;
            validator.rewards += 1;
        }
        
        // Store results
        results.push(SimulationResult {
            epoch: 0,
            validator_id: validator.id,
            pos_score: validator.scores.pos_score,
            poi_score: validator.scores.poi_score,
            final_score: validator.scores.final_score,
            participation: validator.participation,
            missed_blocks: validator.missed_blocks,
            inference_success: validator.inference_success,
            inference_failure: validator.inference_failure,
            slashes: validator.slashes,
            rewards: validator.rewards,
        });
    }
    
    results
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut config = SimulationConfig {
        num_epochs: DEFAULT_EPOCHS,
        num_validators: DEFAULT_VALIDATORS,
        pos_weight: DEFAULT_POS_WEIGHT,
        poi_weight: DEFAULT_POI_WEIGHT,
        score_decay: DEFAULT_SCORE_DECAY,
        min_score: DEFAULT_MIN_SCORE,
        participation_rate: 0.95,
        inference_accuracy: 0.90,
        slash_probability: 0.05,
        reward_probability: 0.10,
    };

    // Parse arguments
    if args.len() > 1 {
        for arg in args.iter().skip(1) {
            if let Some(value) = arg.strip_prefix("--epochs=") {
                config.num_epochs = value.parse().unwrap_or(DEFAULT_EPOCHS);
            } else if let Some(value) = arg.strip_prefix("--validators=") {
                config.num_validators = value.parse().unwrap_or(DEFAULT_VALIDATORS);
            } else if let Some(value) = arg.strip_prefix("--pos-weight=") {
                config.pos_weight = value.parse().unwrap_or(DEFAULT_POS_WEIGHT);
            } else if let Some(value) = arg.strip_prefix("--poi-weight=") {
                config.poi_weight = value.parse().unwrap_or(DEFAULT_POI_WEIGHT);
            }
        }
    }

    // Create validators with different behaviors
    let mut validators = Vec::new();
    for i in 0..config.num_validators {
        let behavior = match i % 4 {
            0 => ValidatorBehavior::Honest,
            1 => ValidatorBehavior::Lazy,
            2 => ValidatorBehavior::Malicious,
            _ => ValidatorBehavior::Inconsistent,
        };
        validators.push(Validator::new(i, behavior));
    }

    // Run simulation
    let mut all_results = Vec::new();
    for epoch in 1..=config.num_epochs {
        println!("\nEpoch {}", epoch);
        let mut results = simulate_epoch(&mut validators, &config);
        
        // Update epoch number in results
        for result in results.iter_mut() {
            result.epoch = epoch;
        }
        
        all_results.extend(results);
    }

    // Print summary
    println!("\nSimulation Summary:");
    println!("==================");
    println!("Total Epochs: {}", config.num_epochs);
    println!("Total Validators: {}", config.num_validators);
    println!("standing share: {}%", config.pos_weight);
    println!("inference share: {}%", config.poi_weight);

    // Print individual validator stats
    println!("\nValidator Statistics:");
    println!("====================");
    for validator in validators.iter() {
        println!("\nValidator {} ({:?}):", validator.id, validator.behavior);
        println!("Final Scores:");
        println!("  PoS: {}", validator.scores.pos_score);
        println!("  PoI: {}", validator.scores.poi_score);
        println!("  Final: {}", validator.scores.final_score);
        println!("Performance:");
        println!("  Participation: {} blocks", validator.participation);
        println!("  Missed Blocks: {}", validator.missed_blocks);
        println!("  Inference Success: {}", validator.inference_success);
        println!("  Inference Failure: {}", validator.inference_failure);
        println!("  Slashes: {}", validator.slashes);
        println!("  Rewards: {}", validator.rewards);
    }

    // Export results to CSV
    let mut wtr = csv::Writer::from_writer(File::create("simulation_results.csv")?);
    for result in all_results {
        wtr.serialize(result)?;
    }
    wtr.flush()?;

    println!("\nResults exported to simulation_results.csv");

    Ok(())
}