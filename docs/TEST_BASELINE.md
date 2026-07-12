# Test Baseline

This document records the honest, as-observed test baseline for the Cerulea Rust
workspace. It is a record of what currently passes and fails. Failing tests are
not fixed and not deleted as part of this task; a failing test is information.

## Toolchain

```
$ rustc --version
rustc 1.85.1 (4eb161250 2025-03-15)

$ cargo --version
cargo 1.85.1 (d73d2caf9 2024-12-31)
```

Build environment notes:
- `.cargo/config.toml` pins `[build] jobs = 1`. To fit this baseline run inside a
  reasonable wall clock, `CARGO_BUILD_JOBS=8` was set in the environment for
  every command below (host has 12 cores, 47 GB RAM). This does not modify any
  tracked file.
- `CARGO_TARGET_DIR=/opt/cerulea/cargo-target` (pre-existing shared cache).
- All commands were run with `--release`. `--all-targets` was never used.

## Workspace packages

From the root `Cargo.toml`, the workspace has these members (directory -> crate name):

- `cerulea-node` -> `cerulea-node`
- `cerulea-runtime` -> `cerulea-runtime`
- `cerulea-pallets/pallet-cerulea-poi` -> `pallet-cerulea-poi`
- `cerulea-pallets/pallet-cerulea-pos` -> `pallet-cerulea-pos`
- `cerulea-pallets/pallet-cerulea-dcf` -> `pallet-cerulea-dcf`
- `cerulea-pallets/pallet-todo` -> `pallet-todo`
- `tools` -> `cerulea-tools`
- `cerulea-node/src/cerulea-consensus` -> `cerulea-consensus`
- `cerulea-pallets/pallet-cerulea-dvf` -> `pallet-cerulea-dvf`

## Commands used

Test binaries were compiled first, in the background, to avoid a single foreground
call exceeding tool time limits:

```
CARGO_BUILD_JOBS=8 cargo test --release --workspace --no-run --no-fail-fast
```

Then the suite was run per package so every package's results are isolated and no
single failure short-circuits the rest of the workspace:

```
CARGO_BUILD_JOBS=8 cargo test --release -p <crate-name> --no-fail-fast
```

## Results

Status: IN PROGRESS. This section is filled in per package as each run completes.

### pallet-cerulea-pos: COMPILE FAILURE (test target)

`cargo test --release --workspace --no-run --no-fail-fast` failed to compile this
package's test target with 159 errors, 1 warning. Representative errors:

```
error[E0433]: failed to resolve: use of undeclared crate or module `pallet_balances`
error[E0412]: cannot find type `Test` in this scope
error[E0412]: cannot find type `RuntimeOrigin` in this scope
error[E0412]: cannot find type `RuntimeCall` in this scope
error[E0412]: cannot find type `RuntimeEvent` in this scope
error[E0412]: cannot find type `PalletInfo` in this scope
error[E0412]: cannot find type `System` in this scope
error[E0412]: cannot find type `Balances` in this scope
error[E0433]: failed to resolve: use of undeclared type `PalletCbcPos`
error: could not compile `pallet-cerulea-pos` (lib test) due to 159 previous errors; 1 warning emitted
```

The errors are concentrated in the test mock runtime (construct_runtime! output types
such as `Test`, `RuntimeOrigin`, `RuntimeCall`, `RuntimeEvent`, `System`, `Balances`,
`PalletCbcPos` are unresolved), indicating the pallet's `mock.rs` test scaffolding is
out of sync with its `construct_runtime!` invocation or macro expansion.

No tests could run. Passed: 0. Failed: 0. Compile failure: yes.

### pallet-cerulea-poi: COMPILE FAILURE (test target)

`cargo test --release --workspace --no-run --no-fail-fast` failed to compile this
package's test target with 3 errors:

```
error[E0046]: not all trait items implemented, missing: `get_active_validators`
  --> cerulea-pallets/pallet-cerulea-poi/src/mock.rs:68:1
  impl pallet_cerulea_poi::PosInterface<u64> for DummyPosInterface { ... }

error[E0046]: not all trait items implemented, missing: `InferenceBoostLow`,
`InferenceBoostMedium`, `InferenceBoostHigh`, `InferencePenaltyLow`,
`InferencePenaltyMedium`, `InferencePenaltyHigh`, `InferenceConfidenceThresholdLow`,
`InferenceConfidenceThresholdHigh`, `MaxValidatorScore`, `PercentagePrecision`,
`OffchainWorkerInterval`, `MaxValidatorIterationWeight`, `MaxLoopIterations`
  --> cerulea-pallets/pallet-cerulea-poi/src/mock.rs:73:1
  impl pallet_cerulea_poi::Config for Test { ... }

error[E0046]: not all trait items implemented, missing: `eject_validator`,
`update_final_score`
  --> cerulea-pallets/pallet-cerulea-poi/src/mock.rs:87:1
  impl pallet_cerulea_poi::DcfInterface<u64> for MockDcfInterface { ... }

error: could not compile `pallet-cerulea-poi` (lib test) due to 3 previous errors
```

The mock trait implementations in `mock.rs` (`DummyPosInterface`, `Test`'s
`pallet_cerulea_poi::Config`, `MockDcfInterface`) no longer satisfy the trait
definitions in `src/lib.rs`. The trait surface has grown (new associated types and
methods) without the mocks being updated to match.

No tests could run. Passed: 0. Failed: 0. Compile failure: yes.

