# Cerulea Testing Guide

This document provides a comprehensive guide to the testing infrastructure for the CBC (Consensus-Based Chain) system.

## Overview

The CBC system includes comprehensive testing across multiple layers:

- **Unit Tests**: Individual pallet functionality
- **Integration Tests**: Cross-pallet interactions
- **Benchmarking**: Performance measurement
- **Mock Runtime**: Isolated testing environment
- **System Tests**: End-to-end scenarios

## Test Structure

### Pallet Tests

#### the framework pallet Pallet (`pallet-cerulea-dcf`)
- **Location**: `cerulea-pallets/pallet-cerulea-dcf/src/`
- **Files**:
  - `tests.rs` - Core functionality tests
  - `integration_tests.rs` - Complex scenario tests
  - `system_integration_tests.rs` - Cross-pallet integration
  - `performance_tests.rs` - Performance validation
  - `mock.rs` - Mock runtime for testing
  - `benchmarking.rs` - Performance benchmarks

#### stake Pallet (`pallet-cerulea-pos`)
- **Location**: `cerulea-pallets/pallet-cerulea-pos/src/`
- **Files**:
  - `tests.rs` - stake functionality
  - `mock.rs` - Mock runtime
  - `benchmarking.rs` - stake benchmarks

#### inference Pallet (`pallet-cerulea-poi`)
- **Location**: `cerulea-pallets/pallet-cerulea-poi/src/`
- **Files**:
  - `tests.rs` - inference functionality
  - `mock.rs` - Mock runtime
  - `benchmarking.rs` - inference benchmarks

### Consensus Tests

#### Consensus Module (`cerulea-consensus`)
- **Location**: `cerulea-node/src/cerulea-consensus/src/`
- **Files**:
  - `mock.rs` - Consensus mock runtime
  - `metrics_test.rs` - Metrics system tests
  - `validator_set_test.rs` - Validator management tests
  - `epoch_manager_test.rs` - Epoch transition tests
  - `dcf.rs` - the framework pallet consensus tests (embedded)

## Running Tests

### All Tests
```bash
cargo test
```
clea
### Specific Pallet Tests
```bash
# the framework pallet pallet tests
cargo test -p pallet-cerulea-dcf

# stake pallet tests
cargo test -p pallet-cerulea-pos

# inference pallet tests
cargo test -p pallet-cerulea-poi
```

### Consensus Tests
```bash
cargo test -p cerulea-consensus
```

### Integration Tests Only
```bash
cargo test integration
```

### Performance Tests
```bash
cargo test performance
```

### Benchmarking
```bash
# Run benchmarks for the framework pallet pallet
cargo test --features runtime-benchmarks -p pallet-cerulea-dcf

# Run benchmarks for stake pallet
cargo test --features runtime-benchmarks -p pallet-cerulea-pos

# Run benchmarks for inference pallet
cargo test --features runtime-benchmarks -p pallet-cerulea-poi
```

## Test Categories

### 1. Unit Tests

**Purpose**: Test individual functions and components in isolation.

**Examples**:
- Validator registration
- Score calculations
- Stake management
- Inference submission

**Key Test Files**:
- `pallet-cerulea-dcf/src/tests.rs`
- `pallet-cerulea-pos/src/tests.rs`
- `pallet-cerulea-poi/src/tests.rs`

### 2. Integration Tests

**Purpose**: Test interactions between different components and pallets.

**Examples**:
- Cross-pallet validator lifecycle
- Consensus weight impact on scoring
- Governance and consensus integration
- Epoch transitions affecting all pallets

**Key Test Files**:
- `pallet-cerulea-dcf/src/integration_tests.rs`
- `pallet-cerulea-dcf/src/system_integration_tests.rs`

### 3. Performance Tests

**Purpose**: Validate system performance under various loads.

**Examples**:
- Batch score updates
- Large validator sets
- High-frequency operations
- Memory usage validation

**Key Test Files**:
- `pallet-cerulea-dcf/src/performance_tests.rs`
- Benchmark files (`benchmarking.rs`)

### 4. Consensus Tests

**Purpose**: Test the consensus mechanism and validator management.

**Examples**:
- Validator selection
- Epoch management
- Metrics collection
- Authority rotation

**Key Test Files**:
- `cerulea-consensus/src/validator_set_test.rs`
- `cerulea-consensus/src/epoch_manager_test.rs`
- `cerulea-consensus/src/metrics_test.rs`

## Mock Runtime

The testing infrastructure uses comprehensive mock runtimes that simulate the full CBC system:

### the framework pallet Mock Runtime
- **File**: `pallet-cerulea-dcf/src/mock.rs`
- **Features**:
  - Full pallet settings
  - Mock stake/inference interfaces
  - Genesis settings
  - Balance management

### Consensus Mock Runtime
- **File**: `cerulea-consensus/src/mock.rs`
- **Features**:
  - Authority management
  - Keystore integration
  - Performance testing utilities

## Test Data and Scenarios

### Genesis Settings
- 3-4 initial validators
- Balanced stakes and scores
- Proper consensus weights (60% stake, 40% inference)

### Test Validators
- Validator IDs: 1, 2, 3, 4
- Initial stakes: 1000 units each
- Initial scores: 6000-8000 range

### Performance Profiles
Tests include validators with different performance characteristics:
- **High Performer**: 95% participation, 90% inference success
- **Medium Performer**: 85% participation, 75% inference success
- **Low Performer**: 70% participation, 50% inference success

## Benchmarking

### the framework pallet Pallet Benchmarks
- `join_validators` - Validator joining process
- `leave_validators` - Validator leaving process
- `slash_validator` - Slashing operations
- `propose_*` - Governance proposals
- `epoch_transition` - Epoch changes
- `distribute_rewards` - Reward distribution

### stake Pallet Benchmarks
- `register_validator` - Validator registration
- `submit_score` - Score submission
- `slash_validator` - stake slashing
- `boost_score` / `slash_score` - Score adjustments

### inference Pallet Benchmarks
- `submit_inference` - Inference submission
- `challenge_inference` - Challenge mechanism
- `resolve_challenge` - Challenge resolution
- `epoch_cleanup` - Cleanup operations

## Test Utilities

### Helper Functions
- `new_test_ext()` - Create test environment
- `setup_validators()` - Create multiple test validators
- `create_test_authorities()` - Generate consensus authorities
- `setup_consensus_test()` - Full consensus test setup

### Mock Interfaces
- `MockPosInterface` - stake pallet simulation
- `MockDcfInterface` - the framework pallet pallet simulation
- `MockWeightInfo` - Weight calculation simulation

## Coverage Areas

### Functional Coverage
- Validator lifecycle management
- Consensus weight calculations
- Governance proposal system
- Epoch transitions
- Slashing and rewards
- Misbehavior reporting
- Performance tracking

### Edge Case Coverage
- Maximum validator limits
- Minimum stake requirements
- Score boundary conditions
- Concurrent operations
- System recovery scenarios
- Invalid input handling

### Performance Coverage
- Large validator sets (up to 100)
- High-frequency operations (10k+ ops)
- Memory efficiency
- Concurrent access patterns
- Benchmark validation

## Best Practices

### Writing Tests
1. **Use descriptive test names** that explain the scenario
2. **Test both success and failure cases**
3. **Verify state changes** after operations
4. **Use appropriate assertions** (assert_ok!, assert_noop!, assert_eq!)
5. **Clean up test state** when necessary

### Mock Data
1. **Use realistic values** for stakes, scores, and counts
2. **Test boundary conditions** (min/max values)
3. **Simulate real-world scenarios** with multiple validators
4. **Include edge cases** in test data

### Performance Testing
1. **Measure execution time** for critical operations
2. **Test with maximum expected load**
3. **Verify memory usage** doesn't grow unbounded
4. **Test concurrent access patterns**

## Continuous Integration

### Test Execution
Tests are automatically run on:
- Pull requests
- Main branch commits
- Release candidates

### Coverage Requirements
- Minimum 80% code coverage
- All critical paths tested
- Edge cases covered
- Performance benchmarks passing

### Quality Gates
- All tests must pass
- No performance regressions
- Benchmark results within acceptable ranges
- Memory usage within limits

## Troubleshooting

### Common Issues

#### Test Failures
1. **Check mock settings** - Ensure all required traits are implemented
2. **Verify genesis state** - Make sure initial state is correct
3. **Check balance setup** - Ensure validators have sufficient funds
4. **Review error messages** - Use assert_noop! to check expected errors

#### Performance Issues
1. **Profile test execution** - Use `cargo test --release` for performance tests
2. **Check resource usage** - Monitor memory and CPU during tests
3. **Optimize test data** - Use minimal data sets for unit tests
4. **Parallel execution** - Be aware of test isolation requirements

#### Mock Runtime Issues
1. **Settings consistency** - Ensure all pallets are properly held
2. **Trait implementations** - Verify all required traits are implemented
3. **Genesis compatibility** - Check genesis config matches runtime expectations

## Future Enhancements

### Planned Improvements
- [ ] Fuzzing tests for edge case discovery
- [ ] Property-based testing for invariant validation
- [ ] Load testing with realistic network conditions
- [ ] Integration with external testing frameworks
- [ ] Automated performance regression detection

### Test Infrastructure
- [ ] Test result reporting and analytics
- [ ] Automated test generation for new features
- [ ] Cross-platform testing validation
- [ ] Integration with monitoring systems

This testing infrastructure ensures the CBC system is robust, performant, and reliable across all operational scenarios.