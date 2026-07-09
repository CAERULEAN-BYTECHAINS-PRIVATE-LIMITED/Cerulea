# CBC Runtime

The CBC Runtime is the core blockchain implementation for the CBC (Caerulean ByteChains) protocol. It implements a sophisticated hybrid consensus mechanism combining Proof of Stake (PoS) and Proof of Inference (PoI) through the Dynamic Consensus Framework (DCF), providing a secure, scalable, and efficient blockchain platform.

## Overview

The CBC Runtime serves as the heart of the CBC blockchain, defining the state transition function and business logic. Built on Substrate's FRAME framework, it integrates multiple custom pallets to deliver a unique consensus mechanism that balances security, performance, and decentralization through hybrid PoS/PoI validation.

## Key Features

- **Hybrid Consensus**: Advanced PoS/PoI consensus through Dynamic Consensus Framework
- **Comprehensive Runtime APIs**: 13+ specialized APIs for blockchain interaction
- **Advanced Validator Management**: On-chain validator registration, scoring, and management
- **Trust Score System**: Sophisticated trust calculation combining PoS and PoI metrics
- **Epoch-based Operations**: Time-based validator rotation and reward distribution
- **Comprehensive Testing**: Full test coverage including integration and API tests
- **Benchmarking Support**: Performance testing and optimization capabilities
- **WASM Compilation**: Optimized WebAssembly runtime for on-chain execution
- **Metadata Hash Extension**: Runtime upgrade compatibility verification
- **Try-Runtime Support**: Safe runtime upgrade testing and validation

## Project Structure

```
cerulea-runtime/
├── Cargo.toml                    # Project dependencies and feature configuration
├── build.rs                      # Build script for WASM binary generation
├── src/
│   ├── apis.rs                   # Runtime APIs exposed to external systems
│   ├── benchmark_tests.rs        # Benchmarking infrastructure tests
│   ├── benchmarks.rs             # Runtime benchmarking logic and implementations
│   ├── configs/                  # Runtime configuration modules
│   │   ├── constants.rs          # Runtime constants and parameters
│   │   ├── mod.rs                # Configuration module definitions
│   │   └── weights.rs            # Weight calculations for extrinsics
│   ├── genesis_config_presets.rs # Genesis block configuration presets
│   ├── lib.rs                    # Main runtime implementation and pallet configuration
│   ├── mock.rs                   # Mock runtime for testing
│   ├── runtime_integration_tests.rs # Runtime-level integration tests
│   └── tests.rs                  # Comprehensive runtime tests
├── tests/
│   └── runtime_api_tests.rs      # Runtime API validation tests
└── README.md                     # This documentation file
```

## Key Files

### Core Runtime Files

#### `Cargo.toml`
- Comprehensive project configuration with 40+ dependencies
- Feature flags for std, runtime-benchmarks, try-runtime, and metadata-hash
- WASM compilation support with substrate-wasm-builder
- Development and build dependencies for testing and compilation

#### `build.rs`
- Advanced build script for WASM runtime generation
- Handles compilation flags and feature toggles
- Ensures proper WASM optimization for on-chain deployment
- Supports metadata hash generation for upgrade compatibility

#### `src/lib.rs`
- Main runtime implementation with complete pallet integration
- Defines runtime version, constants, and executive configuration
- Configures all FRAME pallets (System, Balances, Timestamp, etc.)
- Implements custom CBC pallets (DCF, PoS, PoI)
- Sets up transaction processing and fee calculation
- Defines opaque types for blocks and extrinsics

### API and Integration Files

#### `src/apis.rs`
- Comprehensive Runtime APIs (13+ endpoints) exposed to external systems
- **Core APIs**: Version, metadata, block execution, transaction validation
- **DCF APIs**: Validator management, epoch handling, consensus state
- **PoS APIs**: Validator scoring, stake management, performance tracking
- **PoI APIs**: Inference validation, challenge handling, result verification
- **System APIs**: Block authorship, trust scores, validator profiles
- Used by RPC servers, CLI tools, and external applications

#### `src/benchmarks.rs`
- Runtime benchmarking logic for performance optimization
- Measures execution time and resource usage of runtime operations
- Provides weight calculations for transaction fee estimation
- Supports automated benchmark generation for all pallets

### Testing Infrastructure

#### `src/tests.rs`
- Comprehensive runtime integration tests
- Cross-pallet interaction validation
- End-to-end workflow testing
- Runtime state consistency verification

#### `src/runtime_integration_tests.rs`
- Runtime-level integration tests for complete system validation
- Pallet integration verification
- Runtime constant validation
- System initialization testing

#### `src/benchmark_tests.rs`
- Benchmarking infrastructure validation tests
- Performance testing for all custom pallets
- Benchmark execution verification
- Resource usage validation

#### `src/mock.rs`
- Mock runtime implementation for testing
- Test account creation and funding utilities
- Simplified runtime configuration for unit tests
- Test environment setup and management

#### `tests/runtime_api_tests.rs`
- Comprehensive Runtime API validation tests
- API signature and type safety verification
- Return value validation for all 13+ APIs
- Integration testing with external tools

### Configuration Files

#### `src/configs/constants.rs`
- Runtime constants and parameters
- Block time, slot duration, and epoch configuration
- Balance parameters and existential deposits
- Consensus-specific constants

#### `src/configs/weights.rs`
- Extrinsic weight calculations for fee estimation
- Performance-based weight assignments
- Resource usage optimization
- Benchmark-derived weight values

#### `src/configs/mod.rs`
- Configuration module organization
- Parameter validation and defaults
- Runtime configuration management

#### `src/genesis_config_presets.rs`
- Genesis block configuration presets for different networks
- Initial validator set configuration
- Initial balance distribution
- PoS/PoI parameter initialization
- DCF consensus configuration

## Runtime Features

### Advanced Consensus Mechanism
- **Hybrid PoS/PoI Consensus**: Combines staking with computational inference validation
- **Dynamic Consensus Framework (DCF)**: Real-time consensus parameter adjustment
- **Trust Score System**: Sophisticated scoring combining 65% PoS and 35% PoI weights
- **Validator Performance Tracking**: Comprehensive metrics for block production and validation
- **Epoch-based Operations**: Time-based validator rotation and reward distribution
- **Slashing Protection**: Advanced slashing mechanisms for malicious behavior

### Comprehensive Runtime APIs (13+ Endpoints)

#### Core System APIs
- **Runtime Version**: Version information and compatibility checking
- **Block Execution**: Block processing and state transition validation
- **Transaction Validation**: Comprehensive transaction verification
- **Metadata Access**: Runtime metadata for external tool integration

#### DCF Consensus APIs
- **Current Epoch**: Real-time epoch information and transitions
- **Validator Profiles**: Comprehensive validator information and statistics
- **Expected Author**: Block author prediction and scheduling
- **Validator Set Management**: Active validator tracking and rotation

#### PoS (Proof of Stake) APIs
- **Validator Scoring**: Performance-based validator scoring
- **Stake Management**: Validator stake tracking and updates
- **Reward Distribution**: Automated reward calculation and distribution
- **Validator Status**: Real-time validator state monitoring

#### PoI (Proof of Inference) APIs
- **Inference Validation**: Computational work verification
- **Challenge Management**: Challenge creation and resolution
- **Result Verification**: On-chain inference result validation
- **Performance Metrics**: Inference-based performance tracking

### Advanced Transaction Processing
- **Weight-based Fee Calculation**: Dynamic fee calculation based on resource usage
- **Multi-signature Support**: Advanced signature schemes and validation
- **Transaction Priority**: Priority-based transaction ordering
- **Batch Processing**: Efficient batch transaction execution
- **Fee Estimation**: Accurate fee prediction for transactions

### Security and Validation Features
- **Secure Key Management**: Ed25519 cryptographic key support
- **Block Time Enforcement**: Consistent 6-second block times
- **Balance Protection**: Existential deposit and balance validation
- **Consensus Security**: Multi-layered consensus validation
- **Runtime Upgrade Safety**: Safe runtime upgrade mechanisms with try-runtime

## Building the Runtime

### Basic Build Commands

```bash
# Build native runtime
cargo build

# Build optimized release version
cargo build --release

# Build WASM runtime for on-chain deployment
cargo build --release --target wasm32-unknown-unknown

# Build with all features enabled
cargo build --all-features
```

### Advanced Build Options

```bash
# Build with specific features
cargo build --features runtime-benchmarks
cargo build --features try-runtime
cargo build --features metadata-hash

# Build for on-chain release (optimized)
cargo build --features on-chain-release-build --release --target wasm32-unknown-unknown

# Build with metadata hash for upgrade compatibility
cargo build --features metadata-hash --release
```

### Feature Flags

#### Core Features
- **`std`** (default): Standard library support for native execution
  ```bash
  cargo build --features std
  ```

- **`runtime-benchmarks`**: Enables runtime benchmarking infrastructure
  ```bash
  cargo build --features runtime-benchmarks
  ```

- **`try-runtime`**: Enables try-runtime for safe upgrade testing
  ```bash
  cargo build --features try-runtime
  ```

#### Advanced Features
- **`metadata-hash`**: Enables metadata hashing for upgrade compatibility
  ```bash
  cargo build --features metadata-hash
  ```

- **`on-chain-release-build`**: Optimized build for on-chain deployment
  ```bash
  cargo build --features on-chain-release-build
  ```

### WASM Build with All Features
```bash
# Complete WASM build with all features
cargo build --release \
  --features runtime-benchmarks,try-runtime,metadata-hash \
  --target wasm32-unknown-unknown

# Production WASM build
cargo build --release \
  --features on-chain-release-build \
  --target wasm32-unknown-unknown
```

### Build Verification

```bash
# Check build artifacts
ls -la target/release/wbuild/cerulea-runtime/

# Verify WASM binary
file target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm

# Check runtime version
cargo run -- --version
```

## Runtime Configuration

### Runtime Version and Constants

The runtime version is defined in `src/lib.rs` and includes:

```rust
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("cerulea-runtime"),
    impl_name: create_runtime_str!("cerulea-runtime"),
    authoring_version: 1,
    spec_version: 1,
    impl_version: 1,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    state_version: 1,
};
```

### Block and Time Parameters

- **Block Time**: 6 seconds (6000 milliseconds)
- **Slot Duration**: 6 seconds for consensus timing
- **Block Hash Count**: 2400 blocks (4 hours of history)
- **Maximum Block Weight**: Optimized for transaction throughput
- **Maximum Block Length**: Configurable based on network capacity

### Balance and Economic Parameters

```rust
// Base monetary units
pub const UNIT: Balance = 1_000_000_000_000;           // 1 CBC token
pub const MILLIUNIT: Balance = 1_000_000_000;          // 0.001 CBC
pub const MICROUNIT: Balance = 1_000_000;              // 0.000001 CBC

// Economic parameters
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;    // Minimum account balance
pub const DOLLARS: Balance = UNIT;                     // 1 CBC = 1 DOLLAR equivalent
```

### Consensus Parameters

```rust
// DCF Configuration
pub const EPOCH_DURATION: u32 = 600;                   // 600 slots per epoch (1 hour)
pub const MIN_VALIDATORS: u32 = 4;                     // Minimum validator count
pub const MAX_VALIDATORS: u32 = 100;                   // Maximum validator count

// Trust Score Weights
pub const POS_WEIGHT: u64 = 65;                        // 65% weight for PoS
pub const POI_WEIGHT: u64 = 35;                        // 35% weight for PoI
```

### Check Runtime Version

```bash
# Check version in source code
grep -A 10 "pub const VERSION" src/lib.rs

# Check version from built node
./target/release/cerulea-node --version

# Query runtime version via RPC
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"state_getRuntimeVersion","params":[],"id":1}' \
  http://localhost:9944
```

## Pallets

The runtime integrates a comprehensive set of pallets for complete blockchain functionality:

### Core Substrate Pallets

#### 1. **System (`frame_system`)**
   - **Purpose**: Fundamental blockchain system management
   - **Features**: Account management, block execution, event handling
   - **APIs**: Block hash, account info, event storage
   - **Configuration**: 2400 block hash count, custom account data

#### 2. **Timestamp (`pallet_timestamp`)**
   - **Purpose**: Block timestamp management and time-dependent logic
   - **Features**: Consistent 6-second block times, time-based operations
   - **APIs**: Current timestamp, minimum period validation
   - **Integration**: Used by consensus for slot timing

#### 3. **Balances (`pallet_balances`)**
   - **Purpose**: Account balance management and token transfers
   - **Features**: Free/reserved balance tracking, existential deposits
   - **APIs**: Balance queries, transfer operations, account existence
   - **Configuration**: 1 MILLIUNIT existential deposit, dust handling

#### 4. **Transaction Payment (`pallet_transaction_payment`)**
   - **Purpose**: Transaction fee handling and weight-based pricing
   - **Features**: Dynamic fee calculation, tip support, fee estimation
   - **APIs**: Fee calculation, payment info, next fee multiplier
   - **Integration**: Weight-based fees with length and complexity factors

#### 5. **Sudo (`pallet_sudo`)**
   - **Purpose**: Administrative operations and runtime upgrades
   - **Features**: Superuser access, emergency interventions, governance
   - **APIs**: Sudo key management, privileged operations
   - **Security**: Restricted to designated sudo account

### Custom CBC Pallets

#### 6. **Pallet CBC PoI (`pallet_cbc_poi`)**
   - **Purpose**: Proof of Inference implementation and validation
   - **Features**: 
     - Inference result submission and validation
     - Challenge creation and resolution system
     - On-chain computational work verification
     - Performance-based scoring for validators
   - **APIs**: Submit inference, create challenges, validate results
   - **Integration**: Provides 35% weight in trust score calculation

#### 7. **Pallet CBC PoS (`pallet_cbc_pos`)**
   - **Purpose**: Proof of Stake implementation and validator management
   - **Features**:
     - Validator registration and stake management
     - Performance score tracking and updates
     - Stake-based reward distribution
     - Validator status monitoring (Active/Inactive/Slashed)
   - **APIs**: Register validator, submit scores, query performance
   - **Integration**: Provides 65% weight in trust score calculation

#### 8. **Pallet CBC DCF (`pallet_cbc_dcf`)**
   - **Purpose**: Dynamic Consensus Framework - core consensus coordination
   - **Features**:
     - Hybrid PoS/PoI consensus coordination
     - Validator set management and rotation
     - Epoch-based operations and transitions
     - Trust score calculation and validation
     - Governance mechanisms for consensus parameters
   - **APIs**: 
     - Validator management (join/leave validators)
     - Epoch information and transitions
     - Trust score calculations
     - Validator profiles and statistics
     - Block authorship tracking
   - **Integration**: Central coordinator for all consensus operations

### Pallet Integration and Dependencies

```rust
// Pallet dependency graph
System ← Timestamp ← Balances ← TransactionPayment
                              ↑
                         CBC Pallets
                    (PoS, PoI, DCF) ← Sudo
```

### Runtime Executive Configuration

The runtime uses FRAME Executive to coordinate all pallets:

```rust
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
>;
```

## Security Considerations

### Cryptographic Security
1. **Key Management**: Secure Ed25519 cryptographic keys for all operations
2. **Signature Verification**: Multi-signature support with robust validation
3. **Hash Functions**: Blake2 256-bit hashing for all critical operations
4. **Randomness**: Secure randomness generation for consensus operations

### Consensus Security
1. **Hybrid Validation**: Combined PoS/PoI prevents single-point-of-failure attacks
2. **Slashing Mechanisms**: Economic penalties for malicious validator behavior
3. **Validator Rotation**: Regular validator set rotation prevents centralization
4. **Trust Score Validation**: Multi-factor trust calculation prevents gaming

### Runtime Security
1. **Weight-based Fees**: Prevents DoS attacks through resource pricing
2. **Existential Deposits**: Prevents dust attacks and state bloat
3. **Transaction Validation**: Multi-layer transaction verification
4. **Safe Arithmetic**: Overflow protection in all mathematical operations

### Upgrade Security
1. **Try-Runtime**: Safe runtime upgrade testing before deployment
2. **Metadata Hash**: Compatibility verification for runtime upgrades
3. **Version Management**: Strict version control and compatibility checking
4. **Rollback Capability**: Safe rollback mechanisms for failed upgrades

## Performance Optimization

### Runtime Performance
- **WASM Optimization**: Optimized WebAssembly compilation for on-chain execution
- **Weight Calculation**: Accurate resource usage measurement for fee calculation
- **Parallel Processing**: Multi-threaded transaction validation where possible
- **Memory Management**: Efficient memory usage in no-std environment

### Storage Optimization
- **State Pruning**: Automatic pruning of old state data
- **Compact Encoding**: SCALE codec for efficient data serialization
- **Storage Layouts**: Optimized storage layouts for frequently accessed data
- **Caching**: Strategic caching of frequently used runtime data

### Network Optimization
- **Block Size**: Optimized block size for network propagation
- **Transaction Priority**: Priority-based transaction ordering
- **Batch Processing**: Efficient batch transaction processing
- **Compression**: Data compression for network communication

## Deployment and Operations

### Production Deployment

```bash
# Build production runtime
cargo build --release --features on-chain-release-build

# Generate WASM for on-chain deployment
cargo build --release \
  --features on-chain-release-build \
  --target wasm32-unknown-unknown

# Verify WASM binary
ls -la target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm
```

### Runtime Upgrade Process

1. **Development**: Implement changes and test thoroughly
2. **Testing**: Use try-runtime for upgrade validation
3. **Staging**: Deploy to testnet for integration testing
4. **Production**: Deploy via governance or sudo (if available)

```bash
# Test upgrade with try-runtime
./target/release/cerulea-node try-runtime \
  --runtime ./target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm \
  on-runtime-upgrade live --uri ws://localhost:9944
```

### Monitoring and Maintenance

- **Runtime Version**: Monitor runtime version across network
- **Performance Metrics**: Track transaction throughput and block times
- **Validator Health**: Monitor validator performance and participation
- **Storage Growth**: Monitor state size and implement pruning as needed

## Related Documentation

- [CBC Node Documentation](../cerulea-node/README.md) - Complete node setup and operation
- [CBC Consensus Documentation](../cerulea-node/src/cerulea-consensus/README.md) - Consensus mechanism details
- [DCF Pallet Documentation](../cerulea-pallets/pallet-cerulea-dcf/README.md) - DCF pallet implementation
- [Testing Guide](../docs/TESTING_GUIDE.md) - Comprehensive testing documentation
- [Node Architecture](../docs/node-architecture.md) - Overall system architecture
- [Developer Onboarding](../docs/developer-onboarding.md) - Getting started guide

## Contributing

### Development Workflow
1. Fork the repository and create a feature branch
2. Implement changes with comprehensive tests
3. Run full test suite and benchmarks
4. Update documentation as needed
5. Submit pull request with detailed description

### Code Standards
- Follow Rust best practices and idioms
- Maintain comprehensive test coverage
- Document all public APIs and complex logic
- Use consistent naming conventions
- Optimize for both performance and readability

## Development and Testing

### Testing Infrastructure

The runtime includes comprehensive testing at multiple levels:

#### Unit Tests
```bash
# Run all runtime tests
cargo test

# Run specific test modules
cargo test tests::runtime_integration_tests
cargo test mock::test_runtime_setup
cargo test benchmark_tests::test_dcf_benchmarks

# Run with output for debugging
cargo test -- --nocapture
```

#### Integration Tests
```bash
# Run runtime API tests
cargo test --test runtime_api_tests

# Run cross-pallet integration tests
cargo test runtime_integration_tests

# Run benchmarking tests
cargo test --features runtime-benchmarks benchmark_tests
```

#### Test Coverage

The test suite provides comprehensive coverage:

- **Runtime Integration Tests** (`src/tests.rs`)
  - Cross-pallet interaction validation
  - End-to-end workflow testing
  - Runtime state consistency verification
  - Validator lifecycle testing

- **Runtime API Tests** (`tests/runtime_api_tests.rs`)
  - All 13+ Runtime API endpoint validation
  - API signature and type safety verification
  - Return value validation and consistency
  - External tool integration testing

- **Benchmark Tests** (`src/benchmark_tests.rs`)
  - Benchmarking infrastructure validation
  - Performance testing for all custom pallets
  - Resource usage validation
  - Weight calculation verification

- **Mock Runtime Tests** (`src/mock.rs`)
  - Simplified runtime for unit testing
  - Test environment setup and management
  - Account creation and funding utilities

### Benchmarking

#### Runtime Benchmarking
```bash
# Enable benchmarking features
cargo build --features runtime-benchmarks

# Run pallet benchmarks
cargo test --features runtime-benchmarks benchmarks

# Run specific pallet benchmarks
./target/release/cerulea-node benchmark pallet \
  --pallet pallet_cbc_dcf \
  --extrinsic "*" \
  --steps 50 \
  --repeat 20
```

#### Performance Testing
```bash
# Run overhead benchmarks
./target/release/cerulea-node benchmark overhead \
  --chain dev \
  --execution wasm \
  --wasm-execution compiled

# Run machine benchmarks
./target/release/cerulea-node benchmark machine \
  --chain dev
```

### Try-Runtime for Safe Upgrades

```bash
# Build with try-runtime support
cargo build --features try-runtime

# Test runtime upgrade
./target/release/cerulea-node try-runtime \
  --runtime ./target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm \
  on-runtime-upgrade \
  live \
  --uri ws://localhost:9944

# Dry run runtime upgrade
./target/release/cerulea-node try-runtime \
  --runtime ./target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm \
  on-runtime-upgrade \
  snap \
  --snapshot snapshot.json
```

### Development Tools

#### Code Quality
```bash
# Format code
cargo fmt

# Run clippy lints
cargo clippy --all-features

# Check for unused dependencies
cargo machete
```

#### Documentation
```bash
# Generate documentation
cargo doc --open

# Generate documentation with private items
cargo doc --document-private-items --open
```

#### WASM Analysis
```bash
# Analyze WASM binary size
wasm-opt --print-stack-ir target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm

# Validate WASM binary
wasm-validate target/release/wbuild/cerulea-runtime/cerulea_runtime.wasm
```



