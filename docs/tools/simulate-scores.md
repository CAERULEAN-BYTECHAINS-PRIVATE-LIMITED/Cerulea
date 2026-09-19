# CBC Validator Score Simulation Tool

## Purpose
The Validator Score Simulation Tool is a command-line utility designed to simulate and analyze validator behavior in the Cerulea network. It helps model various scenarios of validator participation, inference accuracy, slashing events, and rewards over multiple epochs.

This tool is particularly useful for:
- Testing validator scoring algorithms
- Simulating network conditions
- Analyzing consensus participation patterns
- Evaluating stake and inference weight settingss
- Testing slashing and reward mechanisms
- Planning network upgrades and parameter changes

## Features
- Multi-epoch simulation
- Adjustable validator parameters
- stake and inference score simulation
- Slashing and reward simulation
- Score decay modeling
- CSV export for analysis
- Detailed epoch-by-epoch reporting
- Validator behavior customization

## Example Output
```
Epoch: 1
Validator: 1
stake Score: 1000
inference Score: 800
Combined Score: 1800
Blocks Authored: 100/100 (100%)
Inferences: 50/50 (100% accurate)
Rewards: 100
Slashes: 0

Epoch: 2
Validator: 1
stake Score: 1050
inference Score: 850
Combined Score: 1900
Blocks Authored: 95/100 (95%)
Inferences: 45/50 (90% accurate)
Rewards: 95
Slashes: 0

Final Summary:
Validator 1:
- Average stake Score: 1025
- Average inference Score: 825
- Total Rewards: 195
- Total Slashes: 0
- Average Participation: 97.5%
- Average Inference Accuracy: 95%
```

## Usage
```bash
# Basic usage
cargo run --bin simulate-scores -- --epochs 10

# With custom validator count
cargo run --bin simulate-scores -- --epochs 10 --validators 5

# With custom stake/inference weights
cargo run --bin simulate-scores -- --epochs 10 --pos-weight 0.7 --poi-weight 0.3

# Export to CSV
cargo run --bin simulate-scores -- --epochs 10 --export simulation.csv

# Full help
cargo run --bin simulate-scores -- --help
```

## Installation
1. Ensure you have Rust installed
2. Clone the Cerulea repository
3. Navigate to the tools directory
4. Build the tool:
```bash
cargo build --bin simulate-scores
```

## Options
- `--epochs`: Number of epochs to simulate (default: 10)
- `--validators`: Number of validators to simulate (default: 10)
- `--pos-weight`: Weight for stake score in combined score (default: 0.7)
- `--poi-weight`: Weight for inference score in combined score (default: 0.3)
- `--export`: Export file path for CSV results (optional)
- `--seed`: Random number generator seed for reproducible results (optional)

## Simulation Parameters
- stake Score Range: 500-2000
- inference Score Range: 500-1500
- Block Authoring Rate: 80-100%
- Inference Accuracy: 70-95%
- Score Decay Rate: 5% per epoch
- Base Reward: 100 units
- Slashing Rate: 10% per slash

## Output Format
The tool outputs:
1. Epoch-by-epoch validator performance
2. Detailed score breakdowns
3. Participation statistics
4. Inference accuracy metrics
5. Reward and slashing history
6. Final summary statistics

## CSV Export
When using the --export option, the tool generates a CSV file containing:
- Epoch number
- Validator ID
- stake score
- inference score
- Combined score
- Blocks authored
- Inferences made
- Accuracy rate
- Rewards earned
- Slashes received

## Use Cases
1. **Algorithm Testing**: Test different stake/inference weight settingss
2. **Network Planning**: Simulate validator participation patterns
3. **Parameter Tuning**: Find optimal score decay rates
4. **Risk Analysis**: Model slashing scenarios
5. **Performance Analysis**: Analyze consensus participation

## Security Notes
- The tool is purely simulation-based and does not interact with real blockchain nodes
- No private keys or sensitive information is required
- All data is generated locally and can be exported for analysis