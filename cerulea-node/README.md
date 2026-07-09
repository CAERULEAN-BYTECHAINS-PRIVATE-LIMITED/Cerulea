# CBC Node

A Substrate-based blockchain node implementing the CBC (Caerulean ByteChains) protocol, featuring a hybrid consensus mechanism combining Proof of Stake (PoS) and Proof of Inference (PoI).

## Features

- **Hybrid Consensus**: Combines PoS and PoI for secure and efficient block validation
- **Dynamic Consensus Framework (DCF)**: Advanced consensus management with epoch transitions
- **Modular Architecture**: Built on Substrate for maximum flexibility and upgradability
- **High Performance**: Optimized for high transaction throughput and low latency
- **Comprehensive RPC API**: 19 specialized CBC RPC endpoints for blockchain interaction
- **Fork Detection**: Built-in fork detection and monitoring capabilities
- **Block Tracking**: Real-time validator performance and missed block monitoring
- **Advanced Logging**: Structured logging with deduplication and file output
- **Benchmarking Suite**: Comprehensive performance testing and optimization tools
- **Multiple Chain Modes**: Support for development, testing, and production configurations
- **Telemetry & Metrics**: Built-in monitoring with Prometheus integration
- **Security Features**: Rate limiting, unsafe RPC controls, and security extensions

## Prerequisites

- Rust (latest stable version)
- LLVM and Clang
- OpenSSL
- Git

## Installation

1. Clone the repository:
```bash
git clone https://github.com/your-org/cerulea-chain.git
cd cerulea-chain/cerulea-node
```

2. Build the node:
```bash
cargo build --release
```

## Usage

### Start a Development Node

```bash
./target/release/cerulea-node --dev
```

### Connect to a Testnet

```bash
./target/release/cerulea-node --chain testnet
```

### Available Chain Specifications

- `dev` or `development` or `CBC` - Development chain with Alice as validator
- `local` or `local_testnet` - Local testnet configuration
- `multi_validator` - Multi-validator testnet setup
- `high_stake` - High-stake validator configuration

### CBC-Specific Modes and Options

```bash
# Run with custom CBC mode
./target/release/cerulea-node --cbc-mode production

# Enable CBC RPC extensions
./target/release/cerulea-node --enable-cbc-extensions

# Log to file with CBC-only logs
./target/release/cerulea-node --log-file cbc.log --cbc-log-only

# Enable unsafe RPC methods (development only)
./target/release/cerulea-node --unsafe-rpc-expose

# Configure RPC rate limiting
./target/release/cerulea-node --rpc-rate-limit-window 60 --rpc-rate-limit-requests 100
```

### Available CLI Commands

#### Core Node Commands
```bash
# Start node with development chain
./target/release/cerulea-node --dev

# Build chain specification
./target/release/cerulea-node build-spec --chain dev

# Check block integrity
./target/release/cerulea-node check-block --chain dev

# Export/Import blocks
./target/release/cerulea-node export-blocks --chain dev
./target/release/cerulea-node import-blocks --chain dev

# Purge chain data
./target/release/cerulea-node purge-chain --chain dev
```

#### CBC-Specific Commands
```bash
# Display node information
./target/release/cerulea-node info

# Check node health status
./target/release/cerulea-node health

# Fork detection and monitoring
./target/release/cerulea-node fork-check --node-url ws://localhost:9944

# Query upcoming block authors
./target/release/cerulea-node query-authors --epoch 10

# Faucet for development (testnet only)
./target/release/cerulea-node faucet --to 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY --amount 1000000000000

# Runtime upgrade
./target/release/cerulea-node runtime-upgrade --wasm-file runtime.wasm
```

#### Benchmarking Commands
```bash
# Run pallet benchmarks
./target/release/cerulea-node benchmark pallet --pallet pallet_cbc_dcf --extrinsic "*"

# Run overhead benchmarks
./target/release/cerulea-node benchmark overhead --chain dev

# Run machine benchmarks
./target/release/cerulea-node benchmark machine --chain dev
```

### Available CLI Options

```
USAGE:
    cerulea-node [OPTIONS] [SUBCOMMAND]

OPTIONS:
    --dev                           Run in development mode
    --chain <CHAIN>                 Specify the chain specification
    --base-path <PATH>              Specify custom base path
    --port <PORT>                   P2P port (default: 30333)
    --rpc-port <PORT>               HTTP-RPC port (default: 9944)
    --ws-port <PORT>                WebSockets port (default: 9944)
    --telemetry-url <URL>           Send telemetry to URL
    --log <LOG>                     Set custom logging filter
    --cbc-mode <MODE>               CBC custom mode (default: production)
    --enable-cbc-extensions         Enable CBC custom RPC extensions
    --log-file <FILE>               Write logs to file instead of stdout
    --cbc-log-only                  Show only CBC-related logs
    --unsafe-rpc-expose             Enable unsafe RPC methods (use with caution)
    --rpc-rate-limit-window <SECS>  RPC rate limiting window (default: 60)
    --rpc-rate-limit-requests <NUM> Max RPC requests per window (default: 100)
    --help                          Print help information
```

## Architecture

### Core Components

- **Consensus Layer**: Implements the hybrid PoS/PoI consensus mechanism with DCF
- **Runtime**: Contains the blockchain's business logic and state transition function
- **Networking**: Handles peer-to-peer communication and block propagation
- **RPC Server**: Provides comprehensive JSON-RPC interface with 19 CBC-specific endpoints
- **CLI**: Advanced command-line interface for node management and utilities
- **Fork Detection**: Real-time fork monitoring and detection system
- **Block Tracker**: Validator performance monitoring and missed block tracking
- **Logging System**: Structured logging with deduplication and file output
- **Benchmarking**: Performance testing and optimization framework

### Directory Structure

```
cerulea-node/
├── src/
│   ├── cerulea-consensus/              # Consensus implementation
│   │   ├── src/
│   │   │   ├── author_selection.rs     # Validator selection logic
│   │   │   ├── block_import.rs          # Block import handling
│   │   │   ├── dcf.rs                   # Dynamic Consensus Framework core
│   │   │   ├── epoch_manager.rs         # Epoch transition handling
│   │   │   ├── error.rs                 # Consensus error types
│   │   │   ├── finality.rs              # Block finalization logic
│   │   │   ├── import_queue.rs          # Block import queue implementation
│   │   │   ├── inherent_providers.rs    # Inherent data providers
│   │   │   ├── lib.rs                   # Module exports and core types
│   │   │   ├── metrics.rs               # Consensus metrics collection
│   │   │   ├── mock.rs                  # Mock implementations for testing
│   │   │   ├── proposer_factory.rs      # Block proposer creation
│   │   │   ├── types.rs                 # Common types and traits
│   │   │   ├── validator_set.rs         # Validator set management
│   │   │   └── tests/                   # Comprehensive test suite
│   │   │       ├── mod.rs
│   │   │       ├── proposer_integration_test.rs
│   │   │       ├── header_parameter_order_test.rs
│   │   │       ├── task8_metrics_test.rs
│   │   │       └── validator_set_test.rs
│   │   └── Cargo.toml
│   │
│   ├── benchmarking.rs         # Benchmarking infrastructure
│   ├── block_tracker.rs        # Block authoring and missed block tracking
│   ├── chain_spec.rs           # Chain specification definitions
│   ├── cli.rs                  # Command-line interface
│   ├── command.rs              # CLI command handlers
│   ├── fork_detection.rs       # Fork detection and monitoring
│   ├── lib.rs                  # Library exports
│   ├── logging.rs              # Advanced logging system
│   ├── main.rs                 # Entry point
│   ├── rpc.rs                  # RPC server with 19 CBC endpoints
│   └── service.rs              # Node service setup
├── tests/                      # Integration and API tests
│   ├── common/                 # Shared test utilities
│   │   └── generators.rs       # Test data generators
│   ├── deterministic_authoring_test.rs
│   ├── dev_mode_integration_test.rs
│   ├── fork_detection_cli_test.rs
│   ├── fork_detection_test.rs
│   ├── log_file_management_test.rs
│   ├── multi_node_integration_test.rs
│   ├── rpc_api_tests.rs        # Comprehensive RPC API tests
│   ├── test_infrastructure.rs  # Test infrastructure
│   └── README.md               # Test documentation
└── Cargo.toml                  # Project manifest
```

### RPC API Endpoints

The node provides 19 specialized CBC RPC endpoints organized into categories:

#### CBC Unified APIs (7 endpoints)
- `cbc_getCurrentEpoch` - Get current epoch number
- `cbc_getValidatorProfile` - Get comprehensive validator profile
- `cbc_getTrustScore` - Get validator trust score with components
- `cbc_listValidators` - Get list of all active validators
- `cbc_getStatus` - Get system-wide status
- `cbc_describe` - List all available CBC RPC methods
- `cbc_health` - Get health check status

#### Author & Block APIs (3 endpoints)
- `dcf_getCurrentAuthor` - Get current block author
- `dcf_getExpectedAuthor` - Get expected author for specific block
- `dcf_getBlockAuthor` - Get block author information

#### Validator Score APIs (3 endpoints)
- `pos_getValidatorScore` - Get validator PoS performance score
- `pos_getValidatorStatus` - Get validator status (Active/Inactive/Slashed)
- Score breakdown via DCF runtime API

#### Participation & History APIs (4 endpoints)
- Epoch participation tracking
- Validator uptime monitoring
- Slashing history records
- Validator status verification

#### System APIs (2 endpoints)
- Runtime version information
- Fork detection and validation

## Development

### Building for Production

```bash
cargo build --release
```

### Running Tests

```bash
# Run all tests
cargo test --all-features

# Run specific test suites
cargo test --package cerulea-node --test rpc_api_tests
cargo test --package cerulea-node --test integration_rpc_tests
cargo test --package cerulea-node --lib rpc::tests

# Run with output
cargo test --package cerulea-node -- --nocapture
```

### Test Coverage

The project includes comprehensive test coverage:
- **19/19 RPC API endpoints tested** (100% coverage)
- **Unit Tests**: 30+ tests covering core functionality
- **Integration Tests**: 25+ tests for realistic scenarios
- **Performance Tests**: Concurrent requests and rapid calls
- **Security Tests**: Rate limiting and access controls
- **Edge Case Tests**: Error handling and boundary conditions

### Generating Documentation

```bash
cargo doc --open
```

### Benchmarking

```bash
# Run pallet benchmarks
./target/release/cerulea-node benchmark pallet --pallet pallet_cbc_dcf --extrinsic "*"

# Run overhead benchmarks
./target/release/cerulea-node benchmark overhead --chain dev

# Run machine benchmarks to verify hardware compatibility
./target/release/cerulea-node benchmark machine --chain dev
```

### Fork Detection and Monitoring

```bash
# Check for forks across network
./target/release/cerulea-node fork-check --node-url ws://localhost:9944

# Monitor validator performance
./target/release/cerulea-node query-authors --epoch 10
```

## Configuration

### Chain Specification

Chain specifications define the genesis state and network parameters. Available chains:
- `dev` - Development chain with Alice as validator
- `local` - Local testnet configuration  
- `multi_validator` - Multi-validator testnet setup
- `high_stake` - High-stake validator configuration

Custom chain specs can be loaded from JSON files.

### Runtime Configuration

The node's runtime configuration is defined in the `cerulea-runtime` crate with support for:
- Dynamic Consensus Framework (DCF) parameters
- Validator set management
- Epoch transition settings
- Trust score calculations (65% PoS, 35% PoI weights)

### CBC Mode Configuration

The node supports different operational modes via `--cbc-mode`:
- `production` (default) - Production-ready configuration
- `testing` - Testing environment settings
- `development` - Development-specific optimizations

### Logging Configuration

Advanced logging features:
- File output: `--log-file cbc.log`
- CBC-only logs: `--cbc-log-only`
- Log deduplication for repeated messages
- Structured logging with CBC-specific prefixes
- Color-coded output support

### RPC Security Configuration

- Rate limiting: `--rpc-rate-limit-window` and `--rpc-rate-limit-requests`
- CBC extensions: `--enable-cbc-extensions`
- Unsafe methods: `--unsafe-rpc-expose` (development only)

## Monitoring

### Prometheus Metrics

The node exposes comprehensive metrics on the configured endpoint (default: `127.0.0.1:9615`):
- Consensus metrics (block authoring, finalization)
- Validator performance metrics
- Network metrics (peer connections, block propagation)
- RPC metrics (request rates, response times)
- Fork detection metrics

### Health Monitoring

```bash
# Check node health via CLI
./target/release/cerulea-node health

# Check health via RPC
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"cerulea_health","params":[],"id":1}' \
  http://localhost:9944
```

### Block Tracking

Real-time monitoring of:
- Validator block authoring performance
- Missed block detection and alerts
- Participation rate calculations
- Consecutive miss tracking

## Tools and Utilities

The project includes additional tools in the `tools/` directory:
- `fork-checker.rs` - Standalone fork detection utility
- `score-checker.rs` - Validator score analysis tool
- `simulate_scores.rs` - Score simulation and testing

## Related Documentation

- [Testing Guide](../docs/TESTING_GUIDE.md) - Comprehensive testing documentation
- [RPC Endpoints](../docs/rpc-endpoints.md) - Complete RPC API reference
- [Node Architecture](../docs/node-architecture.md) - Detailed architecture overview
- [Developer Onboarding](../docs/developer-onboarding.md) - Getting started guide

