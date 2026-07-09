# CBC Validator Score Checker

## Purpose
The Validator Score Checker is a comprehensive command-line tool for analyzing validator performance in the Cerulea network. It provides detailed insights into validator metrics across multiple dimensions:

1. PoS (Proof of Stake) scores and stake amounts
2. PoI (Proof of Inference) scores and inference results
3. Combined scores with configurable weights
4. Validator participation rates
5. Slashing history and rewards
6. Epoch-by-epoch performance history
7. Inference accuracy and confidence metrics

This tool is particularly useful for:
- Monitoring validator performance and ranking
- Debugging score calculation issues
- Analyzing network health and consensus participation
- Tracking validator behavior over time
- Evaluating inference accuracy and reliability
- Managing stake and slashing events

## Features
- Real-time validator score monitoring
- Detailed epoch history tracking
- Export to CSV for analysis
- Multiple output formats (table, JSON, CSV)
- Validator participation statistics
- Slashing and reward history
- Inference accuracy metrics
- Stake amount monitoring

## Example Output
```
Validator        PoS    PoI    Combined    Stake    Slashes    Active    Participation    Last Active
---------------------------------------------------------------------------------------------
5GrwvaEF5zXb... 1000   800    1800        10000    0          Yes       95%             12345
5FHneW46xGXg... 900    700    1600        8000     1          Yes       90%             12344
5FLSigC9HGRK... 800    600    1400        7000     0          No        85%             12343

Epoch History:
Epoch    PoS    PoI    Combined    Blocks    Missed    Inferences    Slashes    Rewards
--------------------------------------------------------------------------
12345    1000   800    1800        100       0         50            0          100
12344    950    750    1700        95        5         45            0          95
12343    900    700    1600        90        10        40            1          90

Inference Info:
Result: 95
Confidence: 90%
Challenge Window: 3 epochs
```

## Usage
```bash
# Basic usage
cargo run --bin score-checker -- --url <node-url>

# With history (shows epoch-by-epoch information)
cargo run --bin score-checker -- --url <node-url> -H

# Specify number of epochs to show (with -H)
cargo run --bin score-checker -- --url <node-url> -H -n 10

# Export to CSV
cargo run --bin score-checker -- --url <node-url> --export scores.csv

# JSON output
cargo run --bin score-checker -- --url <node-url> --format json

# Full help
cargo run --bin score-checker -- --help
```

## Installation
1. Ensure you have Rust installed
2. Clone the Cerulea repository
3. Navigate to the tools directory
4. Build the tool:
```bash
cargo build --bin score-checker
```

## Options
- `--url`: WebSocket URL of the Cerulea node (default: ws://127.0.0.1:9944)
- `--format`: Output format (table, json, csv) (default: table)
- `--export`: Export file path (optional)
- `-H, --history`: Show detailed epoch history (optional)
- `-n, --epochs`: Number of recent epochs to show (with --history) (default: 5)

## Requirements
- Rust 1.68 or later
- Cerulea node running with WebSocket RPC enabled
- JSON-RPC enabled on the node

## Troubleshooting
- Connection refused: Ensure the Cerulea node is running and accessible at the specified URL
- Invalid format: Check the format parameter is one of: table, json, or csv
- Missing RPC methods: Ensure the node has the required runtime APIs enabled

## Security Notes
- The tool only reads data from the blockchain
- No private keys or sensitive information is required
- All data is fetched through public RPC endpoints
2. Navigate to the project root
3. Build the tool:
```bash
cargo build --release
```

## Usage
1. Start your Cerulea node
2. Run the tool:
```bash
./target/release/score-checker
```

The tool will:
1. Connect to your local node (default: ws://127.0.0.1:9944)
2. Fetch validator scores from the blockchain
3. Calculate combined scores
4. Display the results in a sorted table

## Configuration
You can modify the following in the source code:
- Node connection URL
- Score calculation weights
- Output format

## Future Improvements
- Add command-line arguments for configuration
- Support for historical score analysis
- Export to CSV/JSON
- Real-time monitoring mode
- Custom scoring algorithms 