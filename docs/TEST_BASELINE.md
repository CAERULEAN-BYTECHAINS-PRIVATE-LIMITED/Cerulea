# Test baseline

This document records the observed test baseline for the Cerulea chain
workspace. Every figure below comes from a real run. A failing test stays
failing here. A failing test is information about code that the Dynamic
Consensus Framework work replaces.

## How the run happened

1. **Toolchain.** Rust 1.85, pinned in `rust-toolchain.toml`.
2. **Profile.** Release for every command. The debug profile shares no cache.
3. **Jobs.** Four parallel jobs. `--all-targets` never runs.
4. **Command.** `cargo test --release -p <crate> --no-fail-fast`, one crate at a time.
5. **Date.** 2026-09-19, after the framework wiring of task P1-046.

## Totals

The engine packages report 567 passing and 80 failing tests.

## Per package

| Package | Passed | Failed | Ignored | Compiles |
|---|---|---|---|---|
| `cerulea-primitives` | 11 | 0 | 0 | yes |
| `cerulea-consensus` | 316 | 0 | 0 | yes |
| `pallet-cerulea-custody` | 67 | 0 | 0 | yes |
| `pallet-cerulea-dcf` | 370 | 56 | 0 | yes |
| `pallet-cerulea-dvf` | 61 | 19 | 0 | yes |
| `pallet-cerulea-poi` | 58 | 3 | 0 | yes |
| `pallet-cerulea-pos` | 28 | 2 | 0 | yes |
| `cerulea-blueprint` | 192 | 0 | 0 | yes |
| `cerulea-tools` | 3 | 0 | 0 | yes |

## What moved since the previous record

1. **A shared primitives crate arrived.** `cerulea-primitives` carries the slot
   seed derivation, the draw arithmetic and the committee ranking. The node and
   the runtime read the same code. The crate reports 11 passing tests.
2. **The validator registry pallet gained the key records.** Each validator
   registers a draw key and a seal key with a proof of possession from each
   key. Each validator publishes one draw per epoch and the chain checks the
   proof. The pallet moved from 13 passing tests to 61.
3. **The framework pallet finality marker left.** `LastFinalizedBlock`, its two
   companions, the four interface calls, the writers, the readers and the
   bridge trait are gone. Every test that asserted on the marker now asserts on
   the engine that took over the same responsibility. The Assurance Engine sets
   the share each transaction class needs. The Standing Engine sets the Trust
   Score that weight is measured in.
4. **The custody pallet reports a clean suite.** 67 passing tests and no
   failures.

## Failing tests by name

### pallet-cerulea-pos

- `tests::tests::test_get_active_validators`
- `tests::tests::test_minimum_active_validators_constraint`

### pallet-cerulea-poi

- `tests::tests::test_challenge_already_challenged`
- `tests::tests::test_comprehensive_error_handling`
- `tests::tests::test_self_challenge_prevention`

### pallet-cerulea-dvf

Nineteen tests in the genesis and vote preservation suite. Each one asserts on
the weight freeze path that predates the Trust Score composition. The list is
unchanged from the previous record.

### pallet-cerulea-dcf

Fifty six tests across the stress, end to end, cross pallet, invariant and
governance suites. Each one asserts on the retired scoring split, the retired
governance proposal flow, or a stake score bound that the Trust Score
composition replaced. The list is unchanged from the previous record apart from
the marker tests, which are rewritten.

## The check that stops the mock drift defect returning

Each pallet mock ends with a compile-time contract block. The block names every
trait the mock claims to satisfy and forces the compiler to prove each one. A
trait that gains an item stops the test target from compiling at that block,
with the trait named in the error. The block runs on every build of the test
target, so every commit gate covers it.

## The engine tests

Three integration tests run the engines across an in-process network of
validator nodes.

| Test | What it asserts |
|---|---|
| `engine_agreement_test` | Sealed authorship across an epoch boundary, a rejected forgery and one committee on every node |
| `engine_partition_test` | Commitment under a one third outage, a safe halt above it and recovery on return |
| `engine_determinism_test` | Identical per-domain roots and identical combined roots from identical input |

The harness lives at `cerulea-node/src/test_harness.rs` behind the
`test-harness` feature. The harness runs the real engines. It supplies a
keystore for each validator, a message path between nodes and a clock.
