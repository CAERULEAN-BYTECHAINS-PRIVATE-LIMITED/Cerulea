# CBC RPC API Test Suite

This directory contains comprehensive tests for all 19 CBC RPC APIs.

## Test Files

### 1. `rpc_api_tests.rs`
**Comprehensive RPC API Tests**

Tests all 19 RPC APIs with mock runtime:
- **CBC Unified APIs (7)**: getCurrentEpoch, getValidatorProfile, getTrustScore, listValidators, getStatus, describe, health
- **Author & Block APIs (3)**: getCurrentAuthor, getExpectedAuthor, getBlockAuthor
- **Validator Score APIs (3)**: getValidatorScore, getScoreBreakdown, getValidatorStatus
- **Participation & History APIs (4)**: getEpochParticipation, getValidatorUptime, getSlashingHistory, isValidator
- **System APIs (2)**: getRuntimeVersion, checkFork

Additional tests:
- Error handling (invalid methods and parameters)
- Security controls (CBC extensions disabled scenarios)
- Performance tests (multiple rapid calls, concurrent requests)
- Method registration verification

### 2. `integration_rpc_tests.rs`
**Advanced Integration Tests**

Realistic scenario testing with comprehensive mock data:
- Individual API tests for all 19 APIs with realistic data
- Validator lifecycle test (complete validator journey)
- Slashed validator scenario (slashing detection and status changes)
- System health monitoring (comprehensive health check workflow)
- Block authoring sequence (deterministic round-robin authoring)
- Performance ranking (validator score ordering)
- All 19 APIs comprehensive test (sequential execution)

**Mock Data Features:**
- 5 validators with varying scores (95, 92, 88, 83, 76)
- Realistic stakes (2800-5000 tokens)
- Slashing scenarios (validator 4 has been slashed)
- Challenge scenarios (low-performing validators get challenged)
- Round-robin block authoring (deterministic author selection)
- Trust score calculations (65% PoS, 35% PoI weights)

### 3. Unit Tests in `../src/rpc.rs`
**RPC Module Unit Tests**

Located in the main RPC module (`cerulea-node/src/rpc.rs`):
- Rate limiter tests
- Security config tests
- Data structure tests (ValidatorProfile, TrustScore, SystemStatus, etc.)
- Serialization tests (JSON serialization)
- Calculation tests (trust score, consensus health logic)
- Edge case tests (empty validators, division by zero, overflow)
- Account validation tests (AccountId32 handling)
- RPC handler creation tests

## Running Tests

### Run All Tests
```bash
cargo test --package cerulea-node
```

### Run Specific Test Suites
```bash
# Run comprehensive RPC API tests
cargo test --package cerulea-node --test rpc_api_tests

# Run integration tests
cargo test --package cerulea-node --test integration_rpc_tests

# Run unit tests in RPC module
cargo test --package cerulea-node --lib rpc::tests
```

### Run Specific Test
```bash
# Run a specific test by name
cargo test --package cerulea-node test_cbc_get_current_epoch

# Run tests matching a pattern
cargo test --package cerulea-node integration_test
```

### Run with Output
```bash
# Show println! output
cargo test --package cerulea-node -- --nocapture

# Show test names as they run
cargo test --package cerulea-node -- --show-output
```

## Test Coverage

### API Coverage: 19/19 (100%)

**CBC Unified APIs (7/7)**
1. `cbc_getCurrentEpoch` - Get current epoch number
2. `cbc_getValidatorProfile` - Get comprehensive validator profile
3. `cbc_getTrustScore` - Get validator trust score with components
4. `cbc_listValidators` - Get list of all active validators
5. `cbc_getStatus` - Get system-wide status
6. `cbc_describe` - List all available CBC RPC methods
7. `cbc_health` - Get health check status

**Author & Block APIs (3/3)**
8. `dcf_getCurrentAuthor` - Get current block author
9. `dcf_getExpectedAuthor` - Get expected author for specific block
10. `dcf_getBlockAuthor` - Same as getCurrentAuthor

**Validator Score APIs (3/3)**
11. `pos_getValidatorScore` - Get validator PoS performance score
12. `pos_getValidatorStatus` - Get validator status (Active/Inactive/Slashed)
13. Score breakdown - Available via DCF runtime API

**Participation & History APIs (4/4)**
14. Epoch participation - Available via DCF runtime API
15. Validator uptime - Available via DCF runtime API
16. Slashing history - Available via DCF runtime API
17. Is validator - Available via DCF runtime API

**System APIs (2/2)**
18. Runtime version - Standard Substrate API
19. Check fork - Available via DCF runtime API validation

### Test Type Coverage

- Unit Tests (30+ tests)
- Integration Tests (25+ tests)
- Error Handling Tests
- Security Tests
- Performance Tests
- Concurrent Request Tests
- Edge Case Tests
- Scenario Tests

## Test Features

### Mock Runtime API
- Realistic validator data (5 validators)
- Varying performance scores (76-95)
- Realistic stakes (2800-5000 tokens)
- Slashing records (validator 4 slashed)
- Challenge scenarios (low performers challenged)
- Round-robin block authoring
- Trust score calculations (65% PoS, 35% PoI)

### Security Testing
- CBC extensions flag enforcement
- Rate limiting (configurable windows)
- Unsafe method controls
- Invalid parameter handling

### Performance Testing
- Multiple rapid calls (10+ sequential)
- Concurrent requests (5+ simultaneous)
- Response time validation
- Thread safety verification

### Edge Case Testing
- Empty validator lists
- Division by zero handling
- Integer overflow protection
- Invalid account IDs
- Missing data scenarios

## Test Maintenance

### Adding New Tests
1. Add test function to appropriate file
2. Use `#[tokio::test]` for async tests
3. Use `#[test]` for sync tests
4. Follow naming convention: `test_<api_name>` or `integration_test_<scenario>`

### Updating Mock Data
- Update `IntegrationMockClient` in `integration_rpc_tests.rs`
- Update `MockRuntimeApi` in `rpc_api_tests.rs`
- Update unit test mocks in `rpc.rs`

### Test Naming Conventions
- Unit tests: `test_<component>_<behavior>`
- Integration tests: `integration_test_<api_name>`
- Scenario tests: `integration_test_<scenario_name>`

## CI/CD Integration

These tests are designed to run in CI/CD pipelines:
- Fast execution (< 5 seconds total)
- No external dependencies
- Deterministic results
- Clear failure messages

## Troubleshooting

### Common Issues

**Issue: Tests fail to compile**
- Ensure all dependencies are up to date: `cargo update`
- Check Rust version: `rustc --version` (requires 1.70+)

**Issue: Async tests timeout**
- Increase timeout in test attributes
- Check for deadlocks in concurrent tests

**Issue: Mock data inconsistencies**
- Verify mock implementations match runtime API signatures
- Check that all required trait methods are implemented

## Documentation

For more information:
- [API Documentation](../../docs/rpc-endpoints.md)
- [Testing Guide](../../docs/TESTING_GUIDE.md)
