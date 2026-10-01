use rand::Rng;
use serde::Serialize;
use std::error::Error;
use std::fmt::Write as _;
use std::fs::File;
use std::path::Path;

// Constants
const RESULTS_FILE: &str = "simulation_results.csv";
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
            + self.scores.poi_score * config.poi_weight)
            / 100;

        // Ensure minimum score
        self.scores.final_score = self.scores.final_score.max(config.min_score);
    }
}

fn simulate_epoch(
    validators: &mut [Validator],
    config: &SimulationConfig,
) -> Vec<SimulationResult> {
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

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    run(&args, Path::new(RESULTS_FILE))
}

/// Runs the whole simulation and writes every epoch result to `results_path`.
fn run(args: &[String], results_path: &Path) -> Result<(), Box<dyn Error>> {
    let config = parse_args(args);
    let mut validators = create_validators(config.num_validators);
    let all_results = run_simulation(&mut validators, &config);

    print!("{}", render_summary(&config, &validators)?);

    export_results(results_path, &all_results)?;

    println!("\nResults exported to {}", results_path.display());

    Ok(())
}

/// Reads `--name=value` arguments after the program name. Unparsable values keep their defaults.
fn parse_args(args: &[String]) -> SimulationConfig {
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

    config
}

/// Returns the behavior label for a validator index. The four labels repeat in order.
fn behavior_for(index: u32) -> ValidatorBehavior {
    match index % 4 {
        0 => ValidatorBehavior::Honest,
        1 => ValidatorBehavior::Lazy,
        2 => ValidatorBehavior::Malicious,
        _ => ValidatorBehavior::Inconsistent,
    }
}

/// Creates validators with different behaviors.
fn create_validators(count: u32) -> Vec<Validator> {
    let mut validators = Vec::new();
    for i in 0..count {
        validators.push(Validator::new(i, behavior_for(i)));
    }
    validators
}

/// Runs every epoch in order and labels each result with its epoch number.
fn run_simulation(
    validators: &mut [Validator],
    config: &SimulationConfig,
) -> Vec<SimulationResult> {
    let mut all_results = Vec::new();
    for epoch in 1..=config.num_epochs {
        println!("\nEpoch {}", epoch);
        let mut results = simulate_epoch(validators, config);

        // Update epoch number in results
        for result in results.iter_mut() {
            result.epoch = epoch;
        }

        all_results.extend(results);
    }
    all_results
}

/// Renders the run summary and the statistics for each validator.
fn render_summary(
    config: &SimulationConfig,
    validators: &[Validator],
) -> Result<String, std::fmt::Error> {
    let mut out = String::new();

    // Print summary
    writeln!(out, "\nSimulation Summary:")?;
    writeln!(out, "==================")?;
    writeln!(out, "Total Epochs: {}", config.num_epochs)?;
    writeln!(out, "Total Validators: {}", config.num_validators)?;
    writeln!(out, "standing share: {}%", config.pos_weight)?;
    writeln!(out, "inference share: {}%", config.poi_weight)?;

    // Print individual validator stats
    writeln!(out, "\nValidator Statistics:")?;
    writeln!(out, "====================")?;
    for validator in validators.iter() {
        writeln!(
            out,
            "\nValidator {} ({:?}):",
            validator.id, validator.behavior
        )?;
        writeln!(out, "Final Scores:")?;
        writeln!(out, "  PoS: {}", validator.scores.pos_score)?;
        writeln!(out, "  PoI: {}", validator.scores.poi_score)?;
        writeln!(out, "  Final: {}", validator.scores.final_score)?;
        writeln!(out, "Performance:")?;
        writeln!(out, "  Participation: {} blocks", validator.participation)?;
        writeln!(out, "  Missed Blocks: {}", validator.missed_blocks)?;
        writeln!(out, "  Inference Success: {}", validator.inference_success)?;
        writeln!(out, "  Inference Failure: {}", validator.inference_failure)?;
        writeln!(out, "  Slashes: {}", validator.slashes)?;
        writeln!(out, "  Rewards: {}", validator.rewards)?;
    }

    Ok(out)
}

/// Export results to CSV. The header row comes from the field names of the first result.
fn export_results(path: &Path, results: &[SimulationResult]) -> Result<(), Box<dyn Error>> {
    let mut wtr = csv::Writer::from_writer(File::create(path)?);
    for result in results {
        wtr.serialize(result)?;
    }
    wtr.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    const RESULT_HEADERS: [&str; 11] = [
        "epoch",
        "validator_id",
        "pos_score",
        "poi_score",
        "final_score",
        "participation",
        "missed_blocks",
        "inference_success",
        "inference_failure",
        "slashes",
        "rewards",
    ];

    fn argv(values: &[&str]) -> Vec<String> {
        values.iter().map(|v| v.to_string()).collect()
    }

    fn unique_temp_path(label: &str) -> PathBuf {
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

    // Simulation inputs where every random draw has the same outcome.
    fn certain_inputs(probability: f64) -> SimulationConfig {
        SimulationConfig {
            num_epochs: 1,
            num_validators: 1,
            pos_weight: 60,
            poi_weight: 40,
            score_decay: 5,
            min_score: 100,
            participation_rate: probability,
            inference_accuracy: probability,
            slash_probability: probability,
            reward_probability: probability,
        }
    }

    fn validator_with(id: u32, pos_score: u64, poi_score: u64) -> Validator {
        Validator {
            id,
            behavior: behavior_for(id),
            scores: Scores {
                pos_score,
                poi_score,
                final_score: 0,
            },
            participation: 0,
            missed_blocks: 0,
            inference_success: 0,
            inference_failure: 0,
            slashes: 0,
            rewards: 0,
        }
    }

    // Reads a results file and removes it before any assertion runs.
    fn read_and_remove(path: &Path) -> (Vec<String>, Vec<Vec<u64>>) {
        let text = std::fs::read_to_string(path).expect("results file reads back");
        std::fs::remove_file(path).expect("results file removes");
        let mut reader = csv::Reader::from_reader(text.as_bytes());
        let headers = reader
            .headers()
            .expect("header row exists")
            .iter()
            .map(str::to_string)
            .collect::<Vec<String>>();
        let rows = reader
            .records()
            .map(|record| {
                record
                    .expect("row parses")
                    .iter()
                    .map(|field| field.parse::<u64>().expect("field is a whole number"))
                    .collect::<Vec<u64>>()
            })
            .collect::<Vec<Vec<u64>>>();
        (headers, rows)
    }

    #[test]
    fn parse_args_returns_defaults_for_program_name_only() {
        let config = parse_args(&argv(&["simulate_scores"]));
        assert_eq!(config.num_epochs, 10);
        assert_eq!(config.num_validators, 5);
        assert_eq!(config.pos_weight, 60);
        assert_eq!(config.poi_weight, 40);
        assert_eq!(config.score_decay, 5);
        assert_eq!(config.min_score, 100);
        assert_eq!(config.participation_rate, 0.95);
        assert_eq!(config.inference_accuracy, 0.90);
        assert_eq!(config.slash_probability, 0.05);
        assert_eq!(config.reward_probability, 0.10);
    }

    #[test]
    fn parse_args_returns_defaults_for_empty_list() {
        let config = parse_args(&[]);
        assert_eq!(config.num_epochs, 10);
        assert_eq!(config.num_validators, 5);
    }

    #[test]
    fn parse_args_reads_every_supported_flag() {
        let config = parse_args(&argv(&[
            "simulate_scores",
            "--epochs=25",
            "--validators=8",
            "--pos-weight=70",
            "--poi-weight=30",
        ]));
        assert_eq!(config.num_epochs, 25);
        assert_eq!(config.num_validators, 8);
        assert_eq!(config.pos_weight, 70);
        assert_eq!(config.poi_weight, 30);
        assert_eq!(config.score_decay, 5);
    }

    #[test]
    fn parse_args_keeps_default_for_unparsable_value() {
        let config = parse_args(&argv(&[
            "simulate_scores",
            "--epochs=many",
            "--validators=-3",
            "--pos-weight=",
            "--poi-weight=4.5",
        ]));
        assert_eq!(config.num_epochs, 10);
        assert_eq!(config.num_validators, 5);
        assert_eq!(config.pos_weight, 60);
        assert_eq!(config.poi_weight, 40);
    }

    #[test]
    fn parse_args_ignores_unknown_flags_and_skips_program_name() {
        let config = parse_args(&argv(&["--epochs=3", "--decay=9", "--validators=2"]));
        // The first entry is the program name. The tool reads it as a name only.
        assert_eq!(config.num_epochs, 10);
        assert_eq!(config.score_decay, 5);
        assert_eq!(config.num_validators, 2);
    }

    #[test]
    fn parse_args_applies_the_last_repeated_flag() {
        let config = parse_args(&argv(&["simulate_scores", "--epochs=4", "--epochs=6"]));
        assert_eq!(config.num_epochs, 6);
    }

    #[test]
    fn behavior_for_cycles_through_four_labels() {
        assert!(matches!(behavior_for(0), ValidatorBehavior::Honest));
        assert!(matches!(behavior_for(1), ValidatorBehavior::Lazy));
        assert!(matches!(behavior_for(2), ValidatorBehavior::Malicious));
        assert!(matches!(behavior_for(3), ValidatorBehavior::Inconsistent));
        assert!(matches!(behavior_for(4), ValidatorBehavior::Honest));
        assert!(matches!(behavior_for(7), ValidatorBehavior::Inconsistent));
    }

    #[test]
    fn new_validator_starts_with_scores_in_range_and_zero_counters() {
        for id in 0..50 {
            let validator = Validator::new(id, ValidatorBehavior::Lazy);
            assert_eq!(validator.id, id);
            assert!(matches!(validator.behavior, ValidatorBehavior::Lazy));
            assert!((1000..2000).contains(&validator.scores.pos_score));
            assert!((1000..2000).contains(&validator.scores.poi_score));
            assert_eq!(validator.scores.final_score, 0);
            assert_eq!(validator.participation, 0);
            assert_eq!(validator.missed_blocks, 0);
            assert_eq!(validator.inference_success, 0);
            assert_eq!(validator.inference_failure, 0);
            assert_eq!(validator.slashes, 0);
            assert_eq!(validator.rewards, 0);
        }
    }

    #[test]
    fn create_validators_numbers_and_labels_each_validator() {
        let validators = create_validators(6);
        let ids: Vec<u32> = validators.iter().map(|v| v.id).collect();
        assert_eq!(ids, [0, 1, 2, 3, 4, 5]);
        assert!(matches!(
            validators[2].behavior,
            ValidatorBehavior::Malicious
        ));
        assert!(matches!(validators[5].behavior, ValidatorBehavior::Lazy));
        assert!(create_validators(0).is_empty());
    }

    #[test]
    fn update_scores_applies_decay_then_weighted_average() {
        let mut validator = validator_with(0, 1500, 1200);
        validator.update_scores(&certain_inputs(1.0));
        assert_eq!(validator.scores.pos_score, 1495);
        assert_eq!(validator.scores.poi_score, 1195);
        // (1495 * 60 + 1195 * 40) / 100 = 1375
        assert_eq!(validator.scores.final_score, 1375);
    }

    #[test]
    fn update_scores_truncates_fractional_result() {
        let mut inputs = certain_inputs(1.0);
        inputs.score_decay = 0;
        inputs.min_score = 0;
        let mut validator = validator_with(0, 105, 104);
        validator.update_scores(&inputs);
        // (105 * 60 + 104 * 40) / 100 = 104.6. Integer division keeps 104.
        assert_eq!(validator.scores.final_score, 104);
    }

    #[test]
    fn update_scores_raises_result_to_minimum_score() {
        let mut validator = validator_with(0, 3, 2);
        validator.update_scores(&certain_inputs(1.0));
        assert_eq!(validator.scores.pos_score, 0);
        assert_eq!(validator.scores.poi_score, 0);
        assert_eq!(validator.scores.final_score, 100);
    }

    #[test]
    fn update_scores_follows_custom_weights() {
        let mut inputs = certain_inputs(1.0);
        inputs.pos_weight = 70;
        inputs.poi_weight = 30;
        let mut validator = validator_with(0, 2005, 1005);
        validator.update_scores(&inputs);
        // (2000 * 70 + 1000 * 30) / 100 = 1700
        assert_eq!(validator.scores.final_score, 1700);
    }

    #[test]
    fn epoch_with_every_event_certain_updates_all_counters() {
        let mut validators = vec![validator_with(3, 1500, 1200)];
        let results = simulate_epoch(&mut validators, &certain_inputs(1.0));

        let validator = &validators[0];
        assert_eq!(validator.participation, 1);
        assert_eq!(validator.missed_blocks, 0);
        assert_eq!(validator.inference_success, 1);
        assert_eq!(validator.inference_failure, 0);
        assert_eq!(validator.slashes, 1);
        assert_eq!(validator.rewards, 1);
        // Stake score: 1500 - 5 decay - 500 slash + 200 reward = 1195.
        assert_eq!(validator.scores.pos_score, 1195);
        // Inference score: 1200 - 5 decay + 100 success = 1295.
        assert_eq!(validator.scores.poi_score, 1295);

        assert_eq!(results.len(), 1);
        let result = &results[0];
        assert_eq!(result.epoch, 0);
        assert_eq!(result.validator_id, 3);
        assert_eq!(result.pos_score, 1195);
        assert_eq!(result.poi_score, 1295);
        // The final score comes from the scores before this epoch's events.
        assert_eq!(result.final_score, 1375);
        assert_eq!(result.participation, 1);
        assert_eq!(result.inference_success, 1);
        assert_eq!(result.slashes, 1);
        assert_eq!(result.rewards, 1);
    }

    #[test]
    fn epoch_with_every_event_absent_records_misses_and_failures() {
        let mut validators = vec![validator_with(1, 1500, 1200)];
        let results = simulate_epoch(&mut validators, &certain_inputs(0.0));

        let validator = &validators[0];
        assert_eq!(validator.participation, 0);
        assert_eq!(validator.missed_blocks, 1);
        assert_eq!(validator.inference_success, 0);
        assert_eq!(validator.inference_failure, 1);
        assert_eq!(validator.slashes, 0);
        assert_eq!(validator.rewards, 0);
        assert_eq!(validator.scores.pos_score, 1495);
        // Inference score: 1200 - 5 decay - 50 failure = 1145.
        assert_eq!(validator.scores.poi_score, 1145);
        assert_eq!(results[0].missed_blocks, 1);
        assert_eq!(results[0].inference_failure, 1);
    }

    #[test]
    fn epoch_penalties_stop_at_zero() {
        let mut inputs = certain_inputs(1.0);
        inputs.inference_accuracy = 0.0;
        inputs.reward_probability = 0.0;
        let mut validators = vec![validator_with(0, 300, 30)];
        simulate_epoch(&mut validators, &inputs);
        assert_eq!(validators[0].scores.pos_score, 0);
        assert_eq!(validators[0].scores.poi_score, 0);
        // (295 * 60 + 25 * 40) / 100 = 187. The decay runs before the penalties.
        assert_eq!(validators[0].scores.final_score, 187);
    }

    #[test]
    fn epoch_returns_one_result_per_validator_in_order() {
        let mut validators = vec![
            validator_with(0, 1100, 1100),
            validator_with(1, 1200, 1200),
            validator_with(2, 1300, 1300),
        ];
        let results = simulate_epoch(&mut validators, &certain_inputs(0.0));
        let ids: Vec<u32> = results.iter().map(|r| r.validator_id).collect();
        assert_eq!(ids, [0, 1, 2]);
    }

    #[test]
    fn run_simulation_labels_results_with_epoch_numbers() {
        let mut inputs = certain_inputs(0.0);
        inputs.num_epochs = 3;
        let mut validators = vec![validator_with(0, 1500, 1500), validator_with(1, 1600, 1600)];
        let results = run_simulation(&mut validators, &inputs);

        let labels: Vec<(u32, u32)> = results.iter().map(|r| (r.epoch, r.validator_id)).collect();
        assert_eq!(labels, [(1, 0), (1, 1), (2, 0), (2, 1), (3, 0), (3, 1)]);
        let missed: Vec<u32> = results.iter().map(|r| r.missed_blocks).collect();
        assert_eq!(missed, [1, 1, 2, 2, 3, 3]);
        // Three decays of 5 and three failures of 50 remove 165 from 1500.
        assert_eq!(validators[0].scores.poi_score, 1335);
    }

    #[test]
    fn run_simulation_with_zero_epochs_returns_nothing() {
        let mut inputs = certain_inputs(1.0);
        inputs.num_epochs = 0;
        let mut validators = vec![validator_with(0, 1500, 1500)];
        assert!(run_simulation(&mut validators, &inputs).is_empty());
        assert_eq!(validators[0].scores.pos_score, 1500);
    }

    #[test]
    fn summary_lists_totals_and_each_validator() {
        let mut inputs = certain_inputs(1.0);
        inputs.num_epochs = 4;
        inputs.num_validators = 2;
        let mut first = validator_with(0, 1195, 1295);
        first.scores.final_score = 1375;
        first.participation = 4;
        first.inference_success = 4;
        first.slashes = 1;
        first.rewards = 2;
        let mut second = validator_with(1, 900, 800);
        second.scores.final_score = 860;
        second.missed_blocks = 3;
        second.participation = 1;
        second.inference_failure = 4;

        let text = render_summary(&inputs, &[first, second]).expect("summary renders");
        let lines: Vec<&str> = text.lines().collect();

        assert_eq!(
            &lines[..9],
            &[
                "",
                "Simulation Summary:",
                "==================",
                "Total Epochs: 4",
                "Total Validators: 2",
                "standing share: 60%",
                "inference share: 40%",
                "",
                "Validator Statistics:",
            ]
        );
        assert_eq!(
            &lines[10..23],
            &[
                "",
                "Validator 0 (Honest):",
                "Final Scores:",
                "  PoS: 1195",
                "  PoI: 1295",
                "  Final: 1375",
                "Performance:",
                "  Participation: 4 blocks",
                "  Missed Blocks: 0",
                "  Inference Success: 4",
                "  Inference Failure: 0",
                "  Slashes: 1",
                "  Rewards: 2",
            ]
        );
        assert_eq!(lines[24], "Validator 1 (Lazy):");
        assert_eq!(lines[28], "  Final: 860");
        assert_eq!(lines[31], "  Missed Blocks: 3");
        assert_eq!(lines.len(), 36);
    }

    #[test]
    fn export_writes_header_and_rows_that_read_back() {
        let mut validators = vec![validator_with(0, 1500, 1200), validator_with(1, 1400, 1100)];
        let mut results = simulate_epoch(&mut validators, &certain_inputs(1.0));
        for result in results.iter_mut() {
            result.epoch = 7;
        }

        let path = unique_temp_path("simulation-export");
        export_results(&path, &results).expect("results export");
        let (headers, rows) = read_and_remove(&path);

        assert_eq!(headers, RESULT_HEADERS);
        assert_eq!(
            rows,
            [
                vec![7u64, 0, 1195, 1295, 1375, 1, 0, 1, 0, 1, 1],
                vec![7u64, 1, 1095, 1195, 1275, 1, 0, 1, 0, 1, 1],
            ]
        );
    }

    #[test]
    fn export_of_no_results_writes_an_empty_file() {
        let path = unique_temp_path("simulation-empty");
        export_results(&path, &[]).expect("empty export succeeds");
        let text = std::fs::read_to_string(&path).expect("results file reads back");
        std::fs::remove_file(&path).expect("results file removes");
        assert!(text.is_empty());
    }

    #[test]
    fn export_into_missing_directory_fails() {
        let path = unique_temp_path("simulation-missing-dir").join("results.csv");
        let results = simulate_epoch(&mut [validator_with(0, 1500, 1200)], &certain_inputs(1.0));
        assert!(export_results(&path, &results).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn run_writes_one_row_per_validator_per_epoch() {
        let path = unique_temp_path("simulation-run");
        run(
            &argv(&["simulate_scores", "--epochs=3", "--validators=4"]),
            &path,
        )
        .expect("simulation runs");
        let (headers, rows) = read_and_remove(&path);

        assert_eq!(headers, RESULT_HEADERS);
        assert_eq!(rows.len(), 12);
        for (index, row) in rows.iter().enumerate() {
            let epoch = row[0];
            assert_eq!(epoch, index as u64 / 4 + 1);
            assert_eq!(row[1], index as u64 % 4);
            // Each epoch adds exactly one participation or one miss.
            assert_eq!(row[5] + row[6], epoch);
            // Each epoch adds exactly one inference success or one failure.
            assert_eq!(row[7] + row[8], epoch);
            assert!(row[9] <= epoch);
            assert!(row[10] <= epoch);
            assert!(row[4] >= 100);
        }
    }

    #[test]
    fn run_fails_when_results_path_is_unwritable() {
        let path = unique_temp_path("simulation-run-missing-dir").join("results.csv");
        let outcome = run(
            &argv(&["simulate_scores", "--epochs=1", "--validators=1"]),
            &path,
        );
        assert!(outcome.is_err());
    }
}
