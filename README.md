# CBC Chain - Caerulean ByteChains

A production-ready Substrate-based blockchain implementing a sophisticated **Dynamic Consensus Framework (DCF)** that combines **Proof of Stake (PoS)** and **Proof of Inference (PoI)** for advanced validator selection and network security.

## Overview

CBC Chain represents a next-generation blockchain platform that extends traditional PoS consensus by incorporating **Proof of Inference (PoI)** - validators are selected and rewarded based on both their economic stake and their computational contributions to AI inference tasks. The system features comprehensive monitoring, advanced RPC APIs, and production-ready infrastructure.

### Key Features

- **Dynamic Consensus Framework (DCF)**: Adaptive consensus with real-time parameter adjustment
- **Hybrid PoS/PoI Validation**: Sophisticated scoring combining 65% PoS and 35% PoI weights
- **Comprehensive RPC APIs**: 19+ specialized endpoints for blockchain interaction
- **Advanced Validator Management**: Complete lifecycle management with performance tracking
- **Fork Detection System**: Real-time network fork monitoring and resolution
- **Production Monitoring**: Grafana dashboards with Prometheus metrics integration
- **Comprehensive Testing**: 100% RPC API coverage with extensive integration tests
- **Multi-Chain Support**: Development, testing, and production chain configurations
- **Advanced Logging**: Structured logging with deduplication and file output
- **Security Features**: Rate limiting, slashing protection, and comprehensive validation

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         CBC Chain Production Architecture                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  Application Layer                                                          │
│  ├── RPC APIs (19+ endpoints)    ├── CLI Tools & Utilities                  │
│  ├── Monitoring (Grafana)        ├── Fork Detection System                  │
│  └── External Integrations       └── Development Tools                      │
├─────────────────────────────────────────────────────────────────────────────┤
│  Runtime Layer (WASM + Native)                                              │
│  ├── pallet-cbc-dcf    (Dynamic Consensus Framework)                        │
│  ├── pallet-cbc-pos    (Proof of Stake with Advanced Scoring)               │
│  ├── pallet-cbc-poi    (Proof of Inference with Challenge System)           │
│  ├── Standard Pallets  (System, Balances, Timestamp, TransactionPayment)    │
│  └── Runtime APIs      (13+ specialized endpoints)                          │
├─────────────────────────────────────────────────────────────────────────────┤
│  Consensus Layer                                                            │
│  ├── DCF Consensus Engine        ├── Advanced Block Import Pipeline         │
│  ├── Epoch Manager               ├── Inherent Data Providers                │
│  ├── Validator Selection         ├── Finality Engine                        │
│  ├── Block Tracker               └── Comprehensive Metrics System           │
├─────────────────────────────────────────────────────────────────────────────┤
│  Node Layer                                                                 │
│  ├── CBC Node (Multi-mode)       ├── Advanced Logging System                │
│  ├── Service Configuration       ├── Security & Rate Limiting               │
│  ├── Network Protocol            ├── Chain Specifications (4 modes)         │
│  └── Client Services             └── Benchmarking Infrastructure            │
├─────────────────────────────────────────────────────────────────────────────┤
│  Infrastructure Layer                                                       │
│  ├── Monitoring (Prometheus)     ├── Testing Infrastructure                 │
│  ├── Docker Compose Setup        ├── Build & Deployment Scripts             │
│  └── Documentation System        └── Development Tools                      │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Consensus Flow

1. **Validator Registration**: Validators register with stake and inference capabilities
2. **Trust Score Calculation**: DCF combines PoS score (65%) + PoI score (35%)
3. **Epoch Management**: Automated validator set updates based on performance metrics
4. **Block Production**: Deterministic author selection with round-robin scheduling
5. **Performance Tracking**: Real-time monitoring of validator uptime and participation
6. **Fork Detection**: Continuous network monitoring with automatic fork resolution
7. **Reward Distribution**: Performance-based rewards with slashing protection

## Current Status - Production Ready

### Fully Implemented Features

#### **Core Runtime & Consensus**
- **Complete Runtime Implementation**: Production-ready WASM runtime with all pallets integrated
- **Advanced DCF Consensus**: Sophisticated consensus engine with real-time adaptation
- **Hybrid PoS/PoI Validation**: Complete implementation with 65/35 weight distribution
- **Multi-Validator Support**: Full support for multiple validators with proper rotation
- **Epoch Management**: Automated epoch transitions with validator set updates

#### **Comprehensive API Layer**
- **19+ RPC Endpoints**: Complete API coverage for all blockchain operations
  - 7 CBC Unified APIs (epoch, profiles, trust scores, status)
  - 3 Author & Block APIs (current/expected authors)
  - 3 Validator Score APIs (performance scoring)
  - 4 Participation & History APIs (uptime, slashing)
  - 2+ System APIs (runtime version, fork detection)
- **100% API Test Coverage**: All endpoints tested with integration tests

#### **Advanced Node Features**
- **Multi-Chain Support**: 4 chain specifications (dev, local, multi_validator, high_stake)
- **Fork Detection System**: Real-time fork monitoring and resolution
- **Block Tracking**: Comprehensive validator performance monitoring
- **Advanced Logging**: Structured logging with deduplication and file output
- **Security Features**: Rate limiting, unsafe RPC controls, comprehensive validation

#### **Production Infrastructure**
- **Monitoring Stack**: Complete Grafana + Prometheus monitoring setup
- **Docker Integration**: Production-ready containerization
- **Build Scripts**: Automated build and deployment scripts
- **Comprehensive Testing**: 
  - Unit tests for all components
  - Integration tests for cross-pallet functionality
  - API tests for all 19 RPC endpoints
  - Performance and benchmarking tests

#### **Developer Tools & Documentation**
- **CLI Tools**: Advanced command-line interface with 15+ commands
- **Benchmarking Suite**: Complete performance testing infrastructure
- **Fork Checker**: Standalone fork detection utility
- **Score Simulation**: Validator performance simulation tools
- **Comprehensive Documentation**: Complete README files for all components

### Advanced Features

#### **Consensus Engine Capabilities**
- **Dynamic Author Selection**: Multiple selection modes (round-robin, stake-weighted, performance-based)
- **Real-time Metrics**: Comprehensive Prometheus metrics for all consensus operations
- **Block Import Pipeline**: Advanced validation with CBC-specific checks
- **Inherent Data Management**: Centralized inherent data provider system
- **Finality Engine**: Robust finality mechanisms with voting and consensus

#### **Validator Management**
- **Complete Lifecycle**: Registration, performance tracking, rotation, and exit
- **Performance Scoring**: Multi-factor scoring with stake and inference components
- **Slashing Protection**: Economic penalties for malicious behavior
- **Uptime Tracking**: Real-time validator participation monitoring
- **Trust Score System**: Sophisticated trust calculation with configurable weights

#### **Network & Security**
- **Multi-node Support**: Full peer-to-peer network with block propagation
- **Security Hardening**: Rate limiting, access controls, and validation layers
- **Fork Resolution**: Automatic fork detection and resolution mechanisms
- **Upgrade Safety**: Try-runtime support for safe runtime upgrades

## Development Roadmap - Next Phase

### Phase 1: AI Inference Integration (8-12 weeks)

#### 1.1 Real AI Inference System
- [ ] **Inference Task Framework**
  - Define standardized inference task specifications
  - Implement task distribution and scheduling system
  - Add result verification and validation mechanisms
  - Create inference performance benchmarking

- [ ] **Off-chain Worker Integration**
  - Implement off-chain inference computation workers
  - Add secure result submission mechanisms
  - Handle computation failures and timeouts
  - Integrate with existing PoI scoring system

#### 1.2 Advanced PoI Mechanisms
- [ ] **Challenge System Enhancement**
  - Implement sophisticated challenge creation algorithms
  - Add challenge verification and dispute resolution
  - Create economic incentives for challenge participation
  - Add challenge performance metrics

### Phase 2: Governance & Economics (6-8 weeks)

#### 2.1 On-chain Governance
- [ ] **Governance Framework**
  - Implement proposal and voting system
  - Add referendum mechanisms for parameter updates
  - Create treasury and funding mechanisms
  - Add governance participation rewards

#### 2.2 Economic Model Refinement
- [ ] **Dynamic Economics**
  - Implement dynamic fee adjustment mechanisms
  - Add inflation and deflation controls
  - Create validator reward optimization
  - Add economic attack prevention measures

### Phase 3: Network Optimization (4-6 weeks)

#### 3.1 Performance Enhancements
- [ ] **Throughput Optimization**
  - Optimize transaction processing pipeline
  - Implement parallel block validation
  - Add transaction batching and compression
  - Target 1000+ TPS throughput

#### 3.2 Network Resilience
- [ ] **Advanced Security**
  - Implement additional slashing conditions
  - Add network partition recovery mechanisms
  - Create advanced fork choice rules
  - Add DDoS protection and rate limiting enhancements

### Phase 4: Production Deployment (4-6 weeks)

#### 4.1 Testnet Launch
- [ ] **Multi-node Testnet**
  - Deploy comprehensive testnet infrastructure
  - Add advanced monitoring and alerting systems
  - Conduct security audits and penetration testing
  - Create validator onboarding documentation

#### 4.2 Mainnet Preparation
- [ ] **Production Readiness**
  - Finalize economic parameters and tokenomics
  - Complete security audits and formal verification
  - Create disaster recovery procedures
  - Establish governance transition mechanisms

## Technical Implementation

### Advanced Consensus Algorithm

The DCF consensus implements sophisticated validator selection with multiple factors:

```rust
// Trust score calculation (production implementation)
fn calculate_trust_score(validator: &ValidatorProfile) -> TrustScore {
    let pos_score = validator.pos_performance_score;
    let poi_score = validator.poi_performance_score;
    let pos_weight = 65; // 65% weight for PoS
    let poi_weight = 35; // 35% weight for PoI
    
    let combined_score = (pos_score * pos_weight + poi_score * poi_weight) / 100;
    
    TrustScore {
        total: combined_score,
        pos_component: pos_score,
        poi_component: poi_score,
        pos_weight,
        poi_weight,
    }
}

// Advanced validator selection with multiple modes
fn select_next_author(validators: &[ValidatorProfile], mode: AuthorSelectionMode) -> AccountId {
    match mode {
        AuthorSelectionMode::RoundRobin => select_round_robin(validators),
        AuthorSelectionMode::StakeWeighted => select_by_stake(validators),
        AuthorSelectionMode::PerformanceBased => select_by_performance(validators),
        AuthorSelectionMode::Hybrid { stake_weight, performance_weight } => {
            select_hybrid(validators, stake_weight, performance_weight)
        }
    }
}
```

### Architecture Components

#### **Runtime Layer** (`cbc-runtime/`)
- **Complete WASM Runtime**: Production-ready runtime with all pallets
- **13+ Runtime APIs**: Comprehensive API coverage for external integration
- **Advanced Configuration**: Multiple chain specifications and genesis presets
- **Testing Infrastructure**: Complete test coverage with mock runtime

#### **Consensus Engine** (`cbc-node/src/cbc-consensus/`)
- **DCF Implementation**: Sophisticated consensus with real-time adaptation
- **Validator Management**: Complete lifecycle with performance tracking
- **Block Import Pipeline**: Advanced validation with CBC-specific checks
- **Metrics System**: Comprehensive Prometheus metrics collection

#### **Node Implementation** (`cbc-node/src/`)
- **Multi-mode Support**: Development, testing, and production configurations
- **Advanced RPC Server**: 19+ specialized endpoints with rate limiting
- **Fork Detection**: Real-time network monitoring and resolution
- **Security Features**: Comprehensive validation and access controls

#### **Custom Pallets** (`cbc-pallets/`)
- **pallet-cbc-dcf**: Dynamic Consensus Framework with advanced features
- **pallet-cbc-pos**: Proof of Stake with sophisticated scoring
- **pallet-cbc-poi**: Proof of Inference with challenge mechanisms

#### **Infrastructure** (`monitoring/`, `scripts/`, `docs/`)
- **Monitoring Stack**: Grafana dashboards with Prometheus integration
- **Build System**: Automated build and deployment scripts
- **Documentation**: Comprehensive technical documentation
- **Testing Tools**: Advanced testing and validation utilities

## Quick Start

### Prerequisites
- Rust 1.70+ (latest stable recommended)
- LLVM and Clang for compilation
- Git for version control

### Building and Running

```bash
# Clone the repository
git clone https://github.com/CAERULEAN-BYTECHAINS-PRIVATE-LIMITED/CBC-Chain.git
cd CBC-Chain

# Build the node (optimized release)
cargo build --release

# Run development node with temporary storage
./target/release/cbc-node --dev --tmp

# Run with custom chain specification
./target/release/cbc-node --chain local

# Run multi-validator testnet
./target/release/cbc-node --chain multi_validator
```

### Available Chain Modes

```bash
# Development mode (single validator - Alice)
./target/release/cbc-node --dev

# Local testnet (multiple validators)
./target/release/cbc-node --chain local

# Multi-validator testnet configuration
./target/release/cbc-node --chain multi_validator

# High-stake validator configuration
./target/release/cbc-node --chain high_stake
```

### Advanced Node Options

```bash
# Enable CBC RPC extensions
./target/release/cbc-node --dev --enable-cbc-extensions

# Custom logging with file output
./target/release/cbc-node --dev --log-file cbc.log --cbc-log-only

# Production mode with monitoring
./target/release/cbc-node --chain multi_validator \
  --cbc-mode production \
  --enable-cbc-extensions \
  --rpc-rate-limit-window 60 \
  --rpc-rate-limit-requests 100
```

### Testing and Validation

```bash
# Run comprehensive test suite
cargo test --all-features

# Test specific components
cargo test -p pallet-cbc-dcf
cargo test -p cbc-consensus
cargo test -p cbc-node

# Run RPC API tests (19 endpoints)
cargo test --package cbc-node --test rpc_api_tests

# Run integration tests
cargo test --package cbc-node --test multi_node_integration_test

# Run with detailed output
cargo test -- --nocapture
```

### Monitoring and Tools

```bash
# Start monitoring stack
cd monitoring
docker-compose up -d

# Check node health
./target/release/cbc-node health

# Fork detection
./target/release/cbc-node fork-check --node-url ws://localhost:9944

# Query validator information
curl -X POST -H "Content-Type: application/json" \
  --data '{"jsonrpc":"2.0","method":"cbc_listValidators","params":[],"id":1}' \
  http://localhost:9944
```


## Performance Specifications

### Target Specifications
- **Block Time**: 6 seconds (configurable)
- **Validator Set Size**: 4-100 validators (configurable)
- **Storage Efficiency**: SCALE codec with optimized layouts

### Development Environment Performance
- **Consensus Operations**: Optimized for development testing
- **RPC API Response**: Fast response times for all endpoints
- **Test Suite Execution**: Comprehensive test coverage validation
- **Build Performance**: Optimized compilation and WASM generation

### Monitoring and Analytics

#### **Available Metrics** (Prometheus + Grafana)
- **Consensus Health**: Block production and finalization tracking
- **Validator Performance**: Uptime and participation monitoring
- **Network Statistics**: Peer connections and block propagation
- **System Resources**: CPU, memory, and storage usage monitoring

#### **Dashboard Categories**
- **Validator Dashboard**: Individual validator performance tracking
- **Network Overview**: System-wide health monitoring
- **Security Dashboard**: Fork detection and security event tracking

## Project Structure

### Repository Organization

```
CBC-Chain/
├── cbc-node/                    # Node implementation
│   ├── src/                     # Node source code
│   │   ├── cbc-consensus/       # Consensus engine
│   │   ├── fork_detection.rs    # Fork detection system
│   │   ├── logging.rs           # Advanced logging
│   │   ├── rpc.rs               # 19+ RPC endpoints
│   │   └── ...                  # Other node components
│   ├── tests/                   # Comprehensive test suite
│   └── README.md                # Node documentation
├── cbc-runtime/                 # Runtime implementation
│   ├── src/                     # Runtime source code
│   ├── tests/                   # Runtime API tests
│   └── README.md                # Runtime documentation
├── cbc-pallets/                 # Custom pallets
│   ├── pallet-cbc-dcf/          # Dynamic Consensus Framework
│   ├── pallet-cbc-pos/          # Proof of Stake
│   └── pallet-cbc-poi/          # Proof of Inference
├── monitoring/                  # Monitoring infrastructure
│   ├── grafana/                 # Grafana dashboards
│   ├── prometheus.yml           # Prometheus configuration
│   └── README.md                # Monitoring setup guide
├── scripts/                     # Build and deployment scripts
├── tools/                       # Utility tools
├── docs/                        # Technical documentation
└── README.md                    # This file
```

### Component Documentation

- **[Node Documentation](cbc-node/README.md)** - Complete node setup and operation
- **[Runtime Documentation](cbc-runtime/README.md)** - Runtime implementation details
- **[Consensus Documentation](cbc-node/src/cbc-consensus/README.md)** - Consensus mechanism
- **[DCF Pallet Documentation](cbc-pallets/pallet-cbc-dcf/README.md)** - DCF implementation
- **[Testing Guide](docs/TESTING_GUIDE.md)** - Comprehensive testing documentation
- **[Node Architecture](docs/node-architecture.md)** - System architecture overview
- **[Developer Onboarding](docs/developer-onboarding.md)** - Getting started guide

## Contributing

### Development Workflow

1. **Fork and Clone**: Fork the repository and create a feature branch
2. **Development**: Implement changes with comprehensive tests
3. **Testing**: Run full test suite including all 19 RPC API tests
4. **Documentation**: Update relevant documentation
5. **Pull Request**: Submit PR with detailed description and test results

### Code Standards

- **Rust Best Practices**: Follow Rust idioms and conventions
- **Test Coverage**: Maintain comprehensive test coverage (currently 100% for RPC APIs)
- **Documentation**: Document all public APIs and complex logic
- **Performance**: Optimize for both performance and readability
- **Security**: Follow security best practices and conduct reviews

### Development Areas

#### **High Priority**
- **AI Inference Integration**: Real AI inference system implementation
- **Governance Framework**: On-chain governance and parameter updates
- **Performance Optimization**: Transaction throughput improvements
- **Security Enhancements**: Advanced slashing and attack prevention

#### **Medium Priority**
- **Developer Tools**: SDK development and external integrations
- **Network Optimization**: Advanced peer-to-peer improvements
- **Economic Model**: Dynamic fee adjustment and tokenomics refinement
- **Monitoring**: Enhanced analytics and alerting systems

#### **Community Contributions Welcome**
- **Documentation**: Technical guides and tutorials
- **Testing**: Additional test cases and edge case coverage
- **Tooling**: Development utilities and helper scripts
- **Integration**: External tool and service integrations

### Getting Help

- **Issues**: Report bugs and feature requests via GitHub Issues
- **Discussions**: Join technical discussions in GitHub Discussions
- **Documentation**: Comprehensive docs in the `docs/` directory
- **Code Review**: All contributions receive thorough code review

## License

This project is licensed under the MIT-0 License - see the [LICENSE](LICENSE) file for details.

## Links and Resources

### Official Links
- **Repository**: https://github.com/CAERULEAN-BYTECHAINS-PRIVATE-LIMITED/CBC-Chain
- **Organization**: https://cbytechains.com/
- **Technical Documentation**: See `docs/` directory for comprehensive guides

### Key Documentation
- **[Node Setup Guide](cbc-node/README.md)** - Complete node installation and configuration
- **[Runtime Guide](cbc-runtime/README.md)** - Runtime development and deployment
- **[Testing Guide](docs/TESTING_GUIDE.md)** - Comprehensive testing procedures
- **[API Reference](docs/rpc-endpoints.md)** - Complete RPC API documentation
- **[Architecture Overview](docs/node-architecture.md)** - System design and components

### Development Resources
- **[Developer Onboarding](docs/developer-onboarding.md)** - Getting started for developers
- **[Monitoring Setup](monitoring/README.md)** - Production monitoring configuration
- **[Build Scripts](scripts/)** - Automated build and deployment tools

### Community and Support
- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Technical discussions and community support
- **Documentation**: Comprehensive technical guides and API references

---

**CBC Chain** - Next-generation blockchain with hybrid PoS/PoI consensus  
*Built by Caerulean ByteChains Private Limited*  



