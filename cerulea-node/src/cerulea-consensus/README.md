# CBC Consensus

A high-performance, modular consensus framework for the CBC blockchain, implementing a hybrid Dynamic Consensus Framework (DCF) that combines Proof of Stake (PoS) with Proof of Inference (PoI) mechanisms.

## Overview

The CBC Consensus implements a sophisticated hybrid consensus mechanism that leverages both staking and computational work (inference) to achieve secure and efficient blockchain validation. Built on Substrate's consensus framework, it provides a flexible, metrics-driven architecture supporting multiple consensus modes and comprehensive validator management.

## Features

- **Dynamic Consensus Framework (DCF)**: Advanced consensus engine with real-time adaptation
- **Hybrid PoS/PoI Validation**: Combines staking with computational inference validation
- **Comprehensive Validator Management**: On-chain validator set management with performance tracking
- **Epoch-based Operation**: Time-based epochs for validator rotation and reward distribution
- **Advanced Block Production**: Efficient block production with multiple author selection modes
- **Finality Engine**: Robust finality gadget ensuring block finalization
- **Inherent Data Providers**: Centralized system for managing block inherent data
- **Block Import Pipeline**: CBC-specific validation integrated with Substrate's import system
- **Comprehensive Metrics**: Built-in Prometheus metrics for monitoring consensus health
- **Extensive Test Coverage**: Complete test suite covering all consensus components

## Architecture

### Core Components

1. **DCF Engine** (`dcf.rs`)
   - Implements the Dynamic Consensus Framework core logic
   - Manages block production and validation workflows
   - Handles author selection algorithms and slot management
   - Coordinates validator metrics and performance tracking
   - Manages epoch transitions and consensus state

2. **Validator Management** (`validator_set.rs`, `author_selection.rs`)
   - Tracks active validators and their performance metrics
   - Handles validator set changes and rotations
   - Implements multiple selection algorithms (Round-robin, Stake-weighted, Performance-based)
   - Manages validator economics and reward distribution
   - Tracks blocks produced and missed per validator

3. **Epoch Management** (`epoch_manager.rs`)
   - Manages epoch transitions and timing
   - Handles validator set rotation between epochs
   - Coordinates reward distribution and slashing
   - Maintains epoch configuration and parameters

4. **Block Production** (`proposer_factory.rs`)
   - Creates new block proposals with transaction inclusion
   - Manages block production timing and scheduling
   - Handles inherent data integration
   - Coordinates with transaction pool for block assembly

5. **Block Import Pipeline** (`block_import.rs`)
   - Provides CBC-specific block validation
   - Integrates with Substrate's default block import
   - Validates consensus-specific block properties
   - Tracks import metrics and performance

6. **Finality Engine** (`finality.rs`)
   - Implements robust finality gadget
   - Tracks finalized blocks and finalization proofs
   - Handles finality voting and consensus
   - Manages finality-related metrics

7. **Inherent Data Providers** (`inherent_providers.rs`)
   - Centralized management of inherent data providers
   - Handles timestamp and other system-required data
   - Provides consistent inherent data for block production

8. **Import Queue** (`import_queue.rs`)
   - Manages block import queue and processing
   - Handles block validation and verification
   - Coordinates with network layer for block propagation

9. **Metrics System** (`metrics.rs`)
   - Comprehensive Prometheus metrics collection
   - Validator economics and performance tracking
   - Consensus health monitoring
   - Real-time performance analytics

10. **Type System** (`types.rs`, `error.rs`)
    - Core types and data structures
    - Comprehensive error handling
    - Configuration parameters and validator information

### Module Structure

```
cerulea-consensus/
├── src/
│   ├── dcf.rs                    # Dynamic Consensus Framework core
│   ├── author_selection.rs       # Validator selection algorithms
│   ├── validator_set.rs          # Validator set management
│   ├── epoch_manager.rs          # Epoch transition handling
│   ├── proposer_factory.rs       # Block proposal creation
│   ├── block_import.rs           # CBC block import pipeline
│   ├── import_queue.rs           # Block import queue management
│   ├── inherent_providers.rs     # Inherent data provider system
│   ├── finality.rs               # Finality engine implementation
│   ├── metrics.rs                # Comprehensive metrics collection
│   ├── types.rs                  # Core types and data structures
│   ├── error.rs                  # Error handling and types
│   ├── lib.rs                    # Module exports and public API
│   ├── mock.rs                   # Mock implementations for testing
│   └── tests/                    # Comprehensive test suite
│       ├── mod.rs                # Test module organization
│       ├── epoch_manager_test.rs # Epoch management tests
│       ├── header_parameter_order_test.rs # Header validation tests
│       ├── metrics_test.rs       # Metrics system tests
│       ├── proposer_integration_test.rs # Block production tests
│       ├── task8_metrics_test.rs # Advanced metrics tests
│       └── validator_set_test.rs # Validator management tests
└── Cargo.toml                    # Dependencies and features
```

## Usage

### Basic Setup

```rust
use cerulea_consensus::{
    DcfConsensus,
    ConsensusParams,
    ValidatorInfo,
    EpochConfig,
    AuthorSelectionMode,
    ProposerFactory,
    CbcBlockImport,
    CeruleaInherentDataProviders,
};

// Initialize consensus parameters
let params = ConsensusParams {
    slot_duration: 6000, // 6 seconds
    epoch_length: 600,   // 600 slots per epoch
    min_validator_stake: 1000000000000, // 1000 tokens
    max_validators: 100,
    block_production_timeout: Duration::from_secs(30),
    author_selection_mode: AuthorSelectionMode::StakeWeighted,
};

// Create inherent data providers
let inherent_providers = CeruleaInherentDataProviders::new();

// Initialize block import pipeline
let block_import = CbcBlockImport::new(
    substrate_block_import,
    client.clone(),
);

// Create proposer factory
let proposer_factory = ProposerFactory::new(
    client.clone(),
    transaction_pool.clone(),
    inherent_providers,
);

// Initialize consensus engine
let consensus = DcfConsensus::new(
    client.clone(),
    proposer_factory,
    Arc::new(block_import),
    params,
);

// Start the consensus engine
consensus.run().await?;
```

### Advanced Configuration

```rust
// Configure epoch parameters
let epoch_config = EpochConfig {
    epoch_length: 600,
    min_validators: 4,
    max_validators: 100,
    min_stake: 1000000000000,
};

// Configure validator information
let validator_info = ValidatorInfo {
    account_id: validator_public_key,
    stake: 5000000000000,
    performance_score: 95,
    blocks_produced: 150,
    blocks_missed: 5,
};

// Configure author selection mode
let selection_modes = vec![
    AuthorSelectionMode::RoundRobin,        // Simple round-robin
    AuthorSelectionMode::StakeWeighted,     // Weighted by stake
    AuthorSelectionMode::PerformanceBased, // Based on performance metrics
];
```

### Metrics Integration

```rust
use cerulea_consensus::metrics::{ConsensusMetrics, ValidatorEconomicsMetrics};

// Initialize metrics
let metrics = ConsensusMetrics::new(&prometheus_registry)?;

// Track consensus events
metrics.block_production_time.observe(production_time.as_secs_f64());
metrics.validator_set_size.set(active_validators.len() as f64);
metrics.epoch_transitions.inc();

// Track validator economics
let economics_metrics = ValidatorEconomicsMetrics::new(&prometheus_registry)?;
economics_metrics.total_stake.set(total_stake as f64);
economics_metrics.total_rewards_distributed.inc_by(rewards as f64);
```

### Configuration Parameters

Key configuration parameters for the consensus engine:

```rust
/// Core consensus configuration
pub struct ConsensusParams {
    /// Slot duration in milliseconds (default: 6000ms)
    pub slot_duration: u64,
    /// Epoch length in slots (default: 600 slots)
    pub epoch_length: u64,
    /// Minimum stake required to become a validator
    pub min_validator_stake: u128,
    /// Maximum number of validators per epoch
    pub max_validators: u32,
    /// Block production timeout duration
    pub block_production_timeout: Duration,
    /// Author selection algorithm mode
    pub author_selection_mode: AuthorSelectionMode,
}

/// Epoch configuration parameters
pub struct EpochConfig {
    /// Number of slots per epoch
    pub epoch_length: u32,
    /// Minimum number of validators required
    pub min_validators: u32,
    /// Maximum number of validators allowed
    pub max_validators: u32,
    /// Minimum stake required to participate
    pub min_stake: u128,
}

/// Author selection modes
pub enum AuthorSelectionMode {
    /// Simple round-robin selection
    RoundRobin,
    /// Selection weighted by validator stake
    StakeWeighted,
    /// Selection based on performance metrics
    PerformanceBased,
    /// Hybrid selection combining multiple factors
    Hybrid { stake_weight: f64, performance_weight: f64 },
}
```

## Integration

The consensus engine integrates seamlessly with multiple Substrate and CBC components:

### Runtime Integration
- **Runtime API**: Comprehensive integration with CBC runtime APIs
- **Pallet Integration**: Direct integration with CBC pallets (DCF, PoS, PoI)
- **State Queries**: Efficient validator set and consensus state queries
- **Extrinsic Validation**: Consensus-specific transaction validation

### Network Integration
- **Block Propagation**: Efficient block and transaction propagation
- **Peer Communication**: Validator communication and coordination
- **Fork Detection**: Network-wide fork detection and resolution
- **Sync Integration**: Block synchronization and catch-up mechanisms

### Storage Integration
- **Finality Gadget**: Integration with Substrate's finality mechanisms
- **Block Import**: Custom block import pipeline with CBC validation
- **State Management**: Consensus state persistence and recovery

### Monitoring Integration
- **Prometheus Metrics**: Comprehensive metrics collection and export
- **Health Monitoring**: Real-time consensus health tracking
- **Performance Analytics**: Validator performance and network statistics
- **Alert Systems**: Configurable alerting for consensus issues

## Metrics and Monitoring

### Core Consensus Metrics

The consensus engine exposes comprehensive Prometheus metrics:

```rust
// Block production metrics
consensus_block_production_time_seconds: Histogram
consensus_block_import_time_seconds: Histogram
consensus_blocks_produced_total: Counter
consensus_blocks_imported_total: Counter

// Validator metrics
consensus_active_validators: Gauge
consensus_validator_set_size: Gauge
consensus_validator_performance_score: GaugeVec
consensus_validator_blocks_produced: CounterVec
consensus_validator_blocks_missed: CounterVec

// Epoch metrics
consensus_epoch_number: Gauge
consensus_epoch_transitions_total: Counter
consensus_epoch_duration_seconds: Histogram
consensus_slots_per_epoch: Gauge

// Economics metrics
consensus_total_stake: Gauge
consensus_rewards_distributed_total: Counter
consensus_slashing_events_total: Counter
consensus_validator_rewards: CounterVec

// Network metrics
consensus_finalized_block_height: Gauge
consensus_best_block_height: Gauge
consensus_fork_events_total: Counter
consensus_import_queue_size: Gauge
```

### Validator Economics Tracking

```rust
// Validator performance metrics
validator_stake_amount: GaugeVec
validator_performance_score: GaugeVec
validator_blocks_authored: CounterVec
validator_blocks_missed: CounterVec
validator_rewards_earned: CounterVec
validator_slashing_amount: CounterVec

// System-wide economics
total_staked_amount: Gauge
average_validator_performance: Gauge
reward_distribution_rate: Gauge
slashing_rate: Gauge
```

## Development

### Building

```bash
# Build the consensus crate
cargo build --release -p cerulea-consensus

# Build with all features
cargo build --release -p cerulea-consensus --all-features

# Build for runtime benchmarks
cargo build --release -p cerulea-consensus --features runtime-benchmarks
```

### Testing

The consensus engine includes comprehensive test coverage:

```bash
# Run all tests
cargo test -p cerulea-consensus --all-features

# Run specific test modules
cargo test -p cerulea-consensus epoch_manager_test
cargo test -p cerulea-consensus validator_set_test
cargo test -p cerulea-consensus metrics_test

# Run integration tests
cargo test -p cerulea-consensus proposer_integration_test

# Run with output for debugging
cargo test -p cerulea-consensus -- --nocapture
```

### Test Coverage

The test suite covers all major components:

- **Epoch Manager Tests** (`epoch_manager_test.rs`)
  - Epoch transition logic
  - Validator set rotation
  - Reward distribution

- **Validator Set Tests** (`validator_set_test.rs`)
  - Validator addition and removal
  - Performance tracking
  - Stake management

- **Metrics Tests** (`metrics_test.rs`, `task8_metrics_test.rs`)
  - Prometheus metrics collection
  - Performance analytics
  - Economics tracking

- **Proposer Integration Tests** (`proposer_integration_test.rs`)
  - Block production workflows
  - Transaction inclusion
  - Inherent data handling

- **Header Parameter Tests** (`header_parameter_order_test.rs`)
  - Block header validation
  - Parameter ordering
  - Consensus data integrity

### Benchmarking

```bash
# Run consensus benchmarks
cargo bench -p cerulea-consensus

# Run runtime benchmarks (requires runtime-benchmarks feature)
cargo build --release -p cerulea-consensus --features runtime-benchmarks
./target/release/cerulea-node benchmark pallet --pallet pallet_cbc_dcf

# Profile consensus performance
cargo build --release -p cerulea-consensus
perf record ./target/release/consensus-benchmark
```

### Development Tools

```bash
# Check code formatting
cargo fmt -p cerulea-consensus

# Run clippy lints
cargo clippy -p cerulea-consensus --all-features

# Generate documentation
cargo doc -p cerulea-consensus --open

# Check for unused dependencies
cargo machete cerulea-consensus
```

## Features and Cargo Configuration

### Available Features

```toml
[features]
default = ["std"]

# Standard library support
std = [
    "codec/std",
    "frame-support/std",
    "sp-runtime/std",
    # ... other std features
]

# Runtime benchmarking support
runtime-benchmarks = [
    "frame-benchmarking/runtime-benchmarks",
    "frame-support/runtime-benchmarks",
    # ... other benchmark features
]

# Try-runtime support for testing
try-runtime = [
    "frame-support/try-runtime",
    "sp-runtime/try-runtime",
    # ... other try-runtime features
]
```

### Dependencies

Key dependencies include:
- **Substrate Core**: `sp-runtime`, `sp-api`, `sp-consensus`
- **Substrate Client**: `sc-consensus`, `sc-client-api`
- **CBC Pallets**: `pallet-cerulea-dcf`, `pallet-cerulea-pos`, `pallet-cerulea-poi`
- **Async Runtime**: `tokio` for async operations
- **Metrics**: `prometheus` for metrics collection
- **Serialization**: `codec`, `serde` for data serialization

## Error Handling

The consensus engine provides comprehensive error handling:

```rust
/// Consensus-specific error types
#[derive(Debug, thiserror::Error)]
pub enum ConsensusError {
    #[error("Validator not found: {0}")]
    ValidatorNotFound(String),
    
    #[error("Invalid epoch transition: {0}")]
    InvalidEpochTransition(String),
    
    #[error("Block production failed: {0}")]
    BlockProductionFailed(String),
    
    #[error("Import queue error: {0}")]
    ImportQueueError(String),
    
    #[error("Metrics error: {0}")]
    MetricsError(String),
}

/// Result type for consensus operations
pub type ConsensusResult<T> = Result<T, ConsensusError>;
```

## Performance Considerations

### Optimization Guidelines

1. **Validator Set Size**: Optimal performance with 50-100 validators
2. **Slot Duration**: 6-second slots provide good balance of speed and security
3. **Epoch Length**: 600-slot epochs (1 hour) for efficient validator rotation
4. **Block Production**: Parallel transaction processing for high throughput
5. **Metrics Collection**: Configurable metrics granularity to balance monitoring and performance

### Resource Requirements

- **Memory**: ~100MB base + ~1MB per validator
- **CPU**: Multi-core recommended for parallel block processing
- **Storage**: ~10GB for consensus state and metrics history
- **Network**: Low latency connection for optimal block propagation

## Related Documentation

- [CBC Node README](../../README.md) - Main node documentation
- [DCF Pallet Documentation](../../../cerulea-pallets/pallet-cerulea-dcf/README.md) - DCF pallet details
- [Testing Guide](../../../docs/TESTING_GUIDE.md) - Comprehensive testing documentation
- [Node Architecture](../../../docs/node-architecture.md) - Overall system architecture

