# Cerulea platform acceptance

Task P12-011. Section 18.2 of the build specification states eighteen
criteria. This document records what each one reached on the build machine,
the evidence it left and the command that produced it.

Every figure here is measured on this machine. A figure below its target is
written as the figure it reached with the size of the gap. Nothing here
reports a pass that no run produced.

## The machine

The build machine carries eight logical cores and 32 GB of memory. It runs
Windows with a Linux subsystem. Cargo runs inside the subsystem. PostgreSQL 16
answers on port 5544. Three other build agents worked on the same tree
throughout this run, so several figures read the machine under that load.

Each node_modules tree carries the binaries of the side that installed it.
The service runs of criterion 9 ran from Windows. Partway through the run
another range reinstalled the node modules of Cerulea APIs from inside the
Linux subsystem, which replaced the Windows binaries with Linux ones. Every
service run after that moment ran inside the subsystem. Each recorded run
names its side in its own log.

## Repository heads under test

| Repository | Head |
| --- | --- |
| chain | `db10214` |
| apis | `1e55d52` |
| studio | `6b275f9` |
| explorer | `818fdf8` |
| dashboard | `c0738e0` |
| content | `1238d16` |
| scripts | `f694acf` |
| security | `3784c68` |
| test-results | `38ee7ae` |
| intelligence-corpus | `5a02fc3` |

The chain head moved during the run because the convergence range commits to
the same repository. Every recorded run carries the heads of the moment it
started, in its own `COMMITS.txt`.

## How the run is arranged

Nine criteria are provable from the chain alone. Each one is one test inside
`chain/cerulea-node/tests/acceptance_suite.rs`, and every test drives the real
engines of the Dynamic Consensus Framework or the real platform modules. The
command is:

    cargo test --release -p cerulea-node --test acceptance_suite

The other nine live in the services. Each one is one runner under
`scripts/acceptance`, and each runner executes the suites that already prove
its criterion. Every runner records the output of each step and the head of
every repository under `test-results/acceptance/<criterion>`. The command for
one criterion is:

    bash scripts/acceptance/<criterion>.sh

## Summary

| # | Criterion | Result |
| --- | --- | --- |
| 1 | The chain runs | Passed |
| 2 | Partitioned execution works | Passed |
| 3 | Class thresholds hold | Passed |
| 4 | Custody runs automatically | Passed |
| 5 | The solver produces valid profiles | Passed |
| 6 | A non-technical journey completes | Shortfall. One of seven steps passed |
| 7 | All 225 modules install | Passed. 232 of 232 fragments |
| 8 | Every bridge connects | Passed |
| 9 | The generated application works | Passed. 199 tests over four groups |
| 10 | The generated surface works | Shortfall. 46 of 47 tests passed, one timed out |
| 11 | Cerulea AI acts and reverts | Unmeasured on this tree |
| 12 | The manifest is complete | Unmeasured on this tree |
| 13 | Deployment is atomic | Unmeasured on this tree |
| 14 | The operator role is bounded | Passed |
| 15 | Security passes | Unmeasured on this tree |
| 16 | Testing passes | Shortfall. Three coverage figures below target |
| 17 | The term check passes | Passed |
| 18 | Recovery works | Unmeasured on this tree |

Eight criteria passed. Three carry a measured shortfall. Four ran into an
environment the run could not hold still, and this document says so rather
than guessing at their outcome.

## The eighteen criteria

### 1. The chain runs

**Passed.**

Three validators run in one process, each with its own keystore holding an
sr25519 draw key and an ed25519 seal key. The run committed twelve blocks
across two epoch boundaries. Every committed block carries a seal binding the
draw key of the author to the seal key of the author, and every node accepted
that seal. A forged block, where a node signs with its own key while naming
the scheduled author, fails import, and the refusal names the fix.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_one_the_chain_runs_with_sealed_verified_authorship`
- Commits: chain `db10214`

### 2. Partitioned execution works

**Passed.**

One block carrying eight transactions across four conflict domains runs
through the Execution Engine. One worker and eight workers reach the same
per-domain roots and the same combined root. A repeated run reaches the same
roots. A changed transaction changes the combined root, so the result rests on
the input rather than on the order of the work. The claim holds while the
network commits, at every height of a six slot run, and every node holds the
same head.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_two_partitioned_execution_reaches_identical_roots`
- Commits: chain `db10214`

### 3. Class thresholds hold

**Passed.**

The starting class table carries routine at 51 percent, financial at 67,
structural at 80 and constitutional at 90. Over a set of one hundred
validators the routine domain commits on the fifty first signature and the
constitutional domain commits on the ninetieth, so the routine record
finalises first inside the same deployment. A domain carrying several classes
commits at the share of the highest class present. The constitutional delay
window is one hundred blocks. A release inside the window fails and names the
blocks remaining. A release at the effective block succeeds. A second release
of the same change fails.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_three_class_thresholds_hold_with_the_constitutional_delay`
- Commits: chain `db10214`

### 4. Custody runs automatically

**Passed.**

The run covers three hundred blocks at a challenge interval of ten, which
issues thirty rounds. Each round derives the challenge from the block hash and
the round number, selects one validator and one historical record, assembles
the answer from the local copy of that block, verifies the proof against the
record root and moves the custody share. No person takes part at any step, and
every node derives the same challenge from the same published state. One
validator retains the recent history alone. Selection spans the whole history
from genesis, so that validator misses challenges and its standing falls. An
incorrect proof lowers the share sharply and counts a fault. A record taken
from another block fails verification.

The pallet hooks that publish the challenge in the block header and close a
lapsed challenge are covered by the module suite beside it.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_four_custody_runs_automatically_over_a_sustained_run`
- Beside it: `cargo test --release -p pallet-cerulea-custody`
- Commits: chain `db10214`

### 5. The solver produces valid profiles

**Passed.**

The five plain-language declarations carry two, three, three, two and three
answers, which is one hundred and eight declaration sets. Every set produced
between three and five profiles. Every profile passed the fixed limits twice,
once through the profile inspection the Amendment Engine runs and once through
the point inspection beneath it. Every profile carries at least four
validators, a routine share inside the fixed limits, a constitutional share
above the routine share and a constitutional wait at least as long as the
routine wait. Every profile states its consequences in plain sentences. A
point below four validators fails inspection, so the guard is a real guard.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_five_the_solver_yields_valid_profiles_alone`
- Commits: chain `db10214`

### 6. A non-technical journey completes

**Shortfall. One of the seven journey steps passed. The journey stops at step
two.**

The person signs up and opens a session, and that step passed. Step two reads
the plain questions from `/studio/policies/questions`. The endpoint answered
with status 200 and with every question under the field `question`. The
journey reads each question under the field `prompt`, so the step failed at
`apis/e2e/journey.spec.ts:51` with `Cannot read properties of undefined`.
Steps three to seven did not run.

The cross-service journey beside it failed to start its service. The build
step of Cerulea APIs stopped while another range was replacing the node
modules of that repository.

The journey file belongs to task P11-016, which another range holds open. This
range left the file alone.

- Evidence: `test-results/acceptance/06-non-technical-journey/journey.log` and
  `cross-service.log`
- Command: `bash scripts/acceptance/06-non-technical-journey.sh`
- Commits: apis `1e55d52`, studio `6b275f9`, explorer `03b622e`, dashboard
  `c0738e0`

### 7. All 225 modules install

**Passed. 232 of 232 fragments installed and executed.**

The catalogue holds 232 fragments. The 225 modules, the worked example and the
six Government modules. Every fragment resolved its dependency closure,
merged, validated and installed on its own test chain. Every fragment
declaring an action ran one, either to success or to a precondition refusal
carrying the plain sentence the person wrote beside the rule.

| Measurement | Figure |
| --- | --- |
| Fragments read | 232 |
| Blueprints installed | 232 of 232 |
| Fragments declaring an action | 232 |
| Fragments running an action | 232 of 232 |

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_seven_every_module_installs_and_executes`
- Commits: chain `db10214`, content `1238d16`

### 8. Every bridge connects

**Passed.**

The bridge module ships one verifier for each of the seven target networks.
Networks running Ethereum Virtual Machine bytecode, Cosmos, Solana, Polkadot,
Avalanche, Bitcoin and Layer 2 rollups. The acceptance suite asserts that the
seven kinds are present and distinct, that the message identifier separates
the outward message from the inward message so the replay registry holds one
entry per direction, that a changed sender, a changed position in the stream
or a changed payload each take their own slot, that the commitment covers the
class and the expiry so a message cannot be reclassified or extended after the
far network committed to it, that a message stops being acceptable at its
expiry, and that all sixteen faults a verifier raises name the fix.

The full round trip of every target network, with a real proof in each
direction, runs through the module suite. The fixture builder that produces
the far network side is a test-only module of the bridge crate, so the round
trip stays there and the acceptance suite names the command beside it.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_eight_every_bridge_carries_a_message_with_proof`
- Beside it: `cargo test --release -p pallet-cerulea-bridge`, which carries
  `the_ethereum_virtual_machine_round_trip_completes` and its six companions
- Commits: chain `db10214`

### 9. The generated application works

**Passed. 199 tests over four groups.**

| Group | Tests | Result |
| --- | --- | --- |
| Screens, every collection | 70 | passed |
| Permissions, visibility, binding, filtering and delivery | see log | passed |
| Offline capture and synchronisation | 29 | passed |
| Rendering, responsiveness, theme and components | 100 | passed |

- Evidence: `test-results/acceptance/09-generated-application/`
- Command: `bash scripts/acceptance/09-generated-application.sh`
- Side: Windows
- Commits: apis `2d004d2`, chain `2546738`

### 10. The generated surface works

**Shortfall. 46 of the 47 recorded tests passed. One test reached the thirty
second limit under machine load. The route gate is unmeasured.**

| Group | Tests | Result |
| --- | --- | --- |
| Endpoints, records, queries, actions, console and diagnostics | 39 | passed |
| Webhooks and the change stream | see log | passed |
| Proofs | 7 of 8 | one timed out |
| Route gate | | unmeasured |

The endpoint group covers the record interface, the query interface, the
action interface, the request console and the diagnostics bundle. The webhook
and stream group passed.

The proof group ran seven of its eight tests to a pass, including the verifier
that refuses a proof a change touched, the verifier that refuses a proof
naming another record, the byte for byte check against the value the chain
holds and the row filter. The eighth, `rejects a request carrying no access
key`, reached the thirty second limit of the suite. That run sat inside the
Linux subsystem over the mounted Windows volume, where the transform stage
alone took 132 seconds and collection took 486 seconds, while a chain build, a
second chain test run and a serving process shared the machine. The same test
passed on the Windows side earlier in this run. The figure recorded here is
the one the machine produced.

The route gate runs the whole suite of Cerulea APIs with the route recorder on
and compares the routes the suite reached against the routes the service
registers. The everyday gate at `apis/test/route-coverage.test.ts` reads the
same manifest and runs inside the ordinary suite.

- Evidence: `test-results/acceptance/10-generated-surface/`
- Command: `bash scripts/acceptance/10-generated-surface.sh`
- Commits: apis `1e55d52`

### 11. Cerulea AI acts and reverts

**Unmeasured on this tree.**

The runner drives the patch generation suite, the patch validation suite and
the reversal suite, then the whole Cerulea Intelligence project. The first
attempt failed because another range had removed the node modules of Cerulea
APIs while the run was starting, and the module loader reported
`Cannot find module .../vitest/vitest.mjs`. The rerun sits behind the route
gate of criterion 10 in the same queue.

The suites themselves are complete and recorded against tasks P6-018 to
P6-027, and `test/intelligence/reversal.test.ts` proves the three permission
tiers, the checkpoint writer, the timeline with one-action restore, the
selective revert and the actor attribution.

- Evidence: `test-results/acceptance/11-intelligence/`
- Command: `bash scripts/acceptance/11-intelligence.sh`
- Commits: apis `1e55d52`

### 12. The manifest is complete

**Unmeasured on this tree.**

The runner drives the manifest production suites of the compiler and the
manifest views of Cerulea Studio. Both attempts failed on the module loader
while two repositories were being reinstalled, once with
`Cannot find module '@rollup/rollup-win32-x64-msvc'` in Cerulea APIs and once
with the vitest entry absent from Cerulea Studio.

- Evidence: `test-results/acceptance/12-manifest/`
- Command: `bash scripts/acceptance/12-manifest.sh`
- Commits: apis `1e55d52`, studio `6b275f9`

### 13. Deployment is atomic

**Unmeasured on this tree.**

The runner drives the rollback suite and the seven stage suites. The attempt
failed on the module loader with
`Cannot find module '@rollup/rollup-win32-x64-msvc'` while the node modules of
Cerulea APIs were being replaced.

- Evidence: `test-results/acceptance/13-atomic-deployment/`
- Command: `bash scripts/acceptance/13-atomic-deployment.sh`
- Commits: apis `1e55d52`

### 14. The operator role is bounded

**Passed.**

The Cerulea operator role reaches the reserved platform scope alone. Each of
the five platform requests names that scope and nothing else. Every request
naming a customer collection, a customer role or the Blueprint aborts through
the permission module and leaves a record carrying the actor, the moment and
one plain sentence stating why. The customer record stood untouched after
every refused request.

The licence of the deployment then lapsed, the licence control stopped block
production and the deployment state read `Lapsed`. Under suspension the parcel
record read back exactly as it stood, answered with its inclusion proof, and
that proof folded to the collection root. The audit log exported every entry
it held, each one naming its event and carrying its reason.

One measured note. On this tree the five platform requests each resolved to
`PermissionDenied` rather than carrying, because the reserved platform scope
belongs to no registered collection and the permission module refuses a
collection the schema registry does not hold. The criterion asks that an
action outside scope aborts and that suspension keeps records readable and
exportable, and both hold. The question register carries the gap.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`
- Test: `criterion_fourteen_the_operator_role_is_bounded`
- Beside it: `apis/test/deploy/operator-scope.test.ts` and
  `apis/test/deploy/suspension.test.ts`
- Commits: chain `db10214`, content `1238d16`

### 15. Security passes

**Unmeasured on this tree.**

Controls implemented, certification not held.

The runner drives `security/collect-evidence.sh`, which runs the proving
command of all twenty five controls and rewrites `security/CONTROLS.md` from
the register, then the security project of Cerulea APIs, then the secret scan,
then a listing of the chain evidence set. The attempt stopped while the node
modules of Cerulea APIs were being replaced, and the rerun sits behind the
route gate of criterion 10 in the same queue.

The evidence set on this tree holds a directory for every one of the twenty
five controls, each with its README and its last run, and the chain carries
four more under `chain/security/evidence`. The last collection before this run
reported every control collected.

- Evidence: `test-results/acceptance/15-security/`, `security/evidence/`,
  `chain/security/evidence/`
- Command: `bash scripts/acceptance/15-security.sh`
- Commits: security `3784c68`, chain `db10214`, apis `1e55d52`

### 16. Testing passes

**Shortfall. Three of the six coverage gates read below the target of 90 per
cent.**

Every category of Section 17.1 has a suite and a place, which
`test-results/PROGRAMME.md` names one by one. The coverage gates measured on
this tree read:

| Gate | Measured | Target | Gap |
| --- | --- | --- | --- |
| Cerulea Explorer | 93.09 per cent statements | 90 | none |
| Cerulea Dashboard | 90.99 per cent statements | 90 | none |
| Cerulea APIs | 88.02 per cent statements | 90 | **1.98 points** |
| Cerulea Studio, frontend | 83.82 per cent statements | 90 | **6.18 points** |
| The chain, six crates | 70.09 per cent lines | 90 | **19.91 points** |

The four TypeScript summaries were written on this tree between 11:07 and
12:36 in the coordinated time scale, and the runner records the moment of each
one. The chain figure comes from `cargo llvm-cov --release` over six crates
with `SKIP_WASM_BUILD=1`, recorded at `test-results/coverage/chain.md`.

- Evidence: `test-results/acceptance/16-testing/`
- Command: `bash scripts/acceptance/16-testing.sh`
- Side: the Linux subsystem
- Commits: apis `1e55d52`, studio `6b275f9`, explorer `818fdf8`, dashboard
  `c0738e0`, chain `db10214`

### 17. The term check passes

**Passed.**

The guardrail reads every forbidden term from the list beside it and scans the
tree. The acceptance suite runs the guardrail over the whole chain repository
and fails on any hit. The same guardrail ran over all nine repositories
separately and each one reported a pass.

| Repository | Result |
| --- | --- |
| chain | TERM CHECK PASSED |
| apis | TERM CHECK PASSED |
| studio | TERM CHECK PASSED |
| explorer | TERM CHECK PASSED |
| dashboard | TERM CHECK PASSED |
| content | TERM CHECK PASSED |
| scripts | TERM CHECK PASSED |
| security | TERM CHECK PASSED |
| test-results | TERM CHECK PASSED |

One transient hit appeared in an earlier run of the suite, at
`chain/cerulea-node/src/service.rs:4`, while the convergence range was part
way through aliasing the settings type of the service crate. The line now
reads as a dependency import path, which the guardrail exempts, and the check
passes from both sides of the machine.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance_suite.log`
- Command: `cargo test --release -p cerulea-node --test acceptance_suite`, and
  `bash cerulea-build/04-guardrails/term-check.sh <repository>`
- Test: `criterion_seventeen_the_term_check_passes`
- Commits: chain `db10214` and the heads of every repository above

### 18. Recovery works

**Unmeasured on this tree.**

The runner drives `scripts/deploy/restore-test.sh`, which takes a snapshot,
destroys the deployment and brings it back in a clean directory, then
`scripts/deploy/exit-test.sh`, which runs a full export and starts a fresh
instance from the export alone, then the backup and export suites of Cerulea
APIs. The run sits behind the route gate of criterion 10 in the same queue.

The recorded runs of these two scripts from task P12-006 and task P12-007 sit
at `test-results/deployment/P12-006-restoration.txt` and
`test-results/deployment/P12-007-exit-export.txt`.

- Evidence: `test-results/acceptance/18-recovery/`,
  `test-results/deployment/`
- Command: `bash scripts/acceptance/18-recovery.sh`
- Commits: scripts `f694acf`, apis `1e55d52`

## The acceptance command

`bash 05-scripts/verify.sh acceptance`, run from `cerulea-build` inside the
Linux subsystem, reported:

    FAIL: format
    VERIFY: FAILED
    FAIL: chain scope
    VERIFY: FAILED
    FAIL: full suite
    VERIFY: FAILED

The command stops at `cargo fmt --check` in the chain scope, before it reaches
clippy, the workspace tests or the acceptance suite. The diff it reports sits
in `chain/tools/simulate_scores.rs` at lines 217, 256 and above, and every hit
is trailing whitespace on an otherwise blank line. That file belongs to
another range, and the convergence range holds the chain tree open while it
drives the workspace to zero failures. `cargo fmt --check` over the whole
chain tree reported 3,605 diffs at the moment of this run, so the gate stays
red until that range closes.

The command was repeated on a ten minute cycle. The first repeat reported the
same three lines, at the same file. Every repeat is recorded at
`test-results/acceptance/01-to-17-chain/verify-acceptance-retries.log`.

The suite that the acceptance command reaches after the chain scope was
therefore run on its own, and it passes.

The acceptance suite itself passes on its own, in 17.44 seconds:

    running 9 tests
    test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

## Performance, measured

Every figure here was measured on the build machine described above. A figure
this document does not carry is a target rather than a measurement. Other
ranges worked on the same machine during these runs, so the figures read the
machine under that load.

### The chain, nine validators, release profile

| Measurement | Figure |
| --- | --- |
| Execution throughput | 192,254 transactions each second over 10,240 transactions, eight workers |
| Agreement latency, routine | median 0.3 microseconds, 95th 0.3, 99th 0.3 |
| Agreement latency, financial | median 0.4 microseconds, 95th 0.4, 99th 0.8 |
| Agreement latency, structural | median 0.6 microseconds, 95th 0.6, 99th 0.8 |
| Agreement latency, constitutional | median 0.5 microseconds, 95th 0.5, 99th 0.6 |
| Slot latency, whole slot | median 39,000.6 microseconds, 95th 41,712.6, 99th 42,240.4 |

The agreement latency measures the Commit Engine settling one domain over the
signatures already in hand. The slot latency measures a whole slot, from the
committee draw through the seal, the import check on every node, the signature
collection and the commitment. The source sits at
`test-results/performance/60f31f7`.

### The generated surface, through Cerulea APIs in local mode

| Measurement | Median | 95th percentile | 99th percentile |
| --- | --- | --- | --- |
| Record read | 99.71 ms | 178.72 ms | 222.17 ms |
| Collection query, 25 records | 93.85 ms | 130.21 ms | 148.09 ms |
| Proof read | 72.18 ms | 122.00 ms | 149.72 ms |

Record writes reached 6.3 each second over 120 writes. A write travels through
the in-process chain driver and waits for the block that carries it, so the
figure reads the block interval rather than the service. The source sits at
`test-results/performance/fd7a17e`.

### The acceptance suite

Nine tests, 17.44 seconds, release profile, with three other ranges sharing
the machine.

## Coverage, measured

Section 17.2 sets a target of 90 per cent. Every figure below is a measurement
on this tree, and every figure below the target carries the size of the gap.

### The chain

Measured with `cargo llvm-cov --release` over six crates, inside the Linux
subsystem, with `SKIP_WASM_BUILD=1`. The measurement needs that flag, because
the runtime otherwise builds its own WebAssembly blob, that build inherits the
coverage instrument flag, and the WebAssembly target carries no profiler
runtime.

| Crate | Lines covered |
| --- | --- |
| cerulea-primitives | 95.54 per cent, 728 of 762 |
| pallet-cerulea-evolution | 81.20 per cent, 527 of 649 |
| cerulea-blueprint | 74.48 per cent, 10,678 of 14,337 |
| pallet-cerulea-schema | 68.68 per cent, 1,048 of 1,526 |
| pallet-cerulea-state | 67.67 per cent, 1,105 of 1,633 |
| cerulea-node, which holds the consensus engines | 63.60 per cent, 8,153 of 12,820 |
| **Whole measurement** | **70.09 per cent lines, 62.62 regions, 67.58 functions** |

**Shortfall. The chain measurement reads 70.09 per cent lines against a target
of 90. The gap is 19.91 points.** The thinnest files are named in
`test-results/coverage/chain.md`. Closing the gap means tests over
`pallet-cerulea-state/src/retention.rs` at 9.90 per cent, the weight modules
at nil, and the migration paths of `pallet-cerulea-schema/src/migrate.rs` at
62.41 per cent.

### The TypeScript repositories

| Repository | Statements | Branches | Functions | Gap to 90 |
| --- | --- | --- | --- | --- |
| Cerulea Explorer | 93.09 per cent, 2,913 of 3,129 | 80.00 | 81.00 | none |
| Cerulea Dashboard | 90.99 per cent, 4,313 of 4,740 | 79.18 | 85.57 | none |
| Cerulea APIs | 88.02 per cent, 49,234 of 55,935 | 80.10 | 88.45 | **1.98 points** |
| Cerulea Studio, frontend | 83.82 per cent, 18,112 of 21,607 | 79.07 | 76.13 | **6.18 points** |

**Shortfall. Cerulea APIs reads 88.02 per cent against 90, a gap of 1.98
points. Cerulea Studio reads 83.82 per cent against 90, a gap of 6.18
points.** The source sits at `test-results/coverage/typescript.md` and the
gate output sits at `test-results/acceptance/16-testing/`.

## Compliance

Controls implemented, certification not held.

The control register sits at `security/controls.json` and maps every control
to ISO 27001, ISO 27017, ISO 27018 and the National Institute of Standards and
Technology Special Publication 800-53 revision 5. The mapping document at
`security/CONTROLS.md` is written from that register by
`security/collect-evidence.sh`, so the document and the evidence never drift.

## Open questions that affect acceptance

These entries come from `cerulea-build/02-tasks/QUESTIONS.md` and each one
bears on a criterion above.

1. **Thirty-one module names in the template seed have no module** (P4-008).
   `templates.seed.json` lists module identifiers that `modules.seed.json`
   does not hold. The gallery reports each one as a missing part. This bears
   on criterion 7 for the template path and on criterion 6, where a person
   picking one of those templates meets a gap.
2. **One Government template names a part the catalogue lacks** (P4-043).
   `gov-court-records` reaches `indexer-sink` through its dependency closure
   and the catalogue holds no part of that name. Same bearing as the entry
   above.
3. **The faucet subcommand of the node is a stand-in** (P12 deployment). The
   subcommand prints a line and performs no transaction. The no-placeholder
   rule covers it. This bears on criterion 1, where a reader of the node
   expects every subcommand to do what it says.
4. **Running Cerulea APIs from source stops on a type the runtime rejects**
   (P12 deployment). `src/drivers/notifications/live.ts` declares a
   constructor parameter property, which the strip-only mode of the runtime
   refuses. The built service runs. This bears on criteria 6, 9, 10 and 13,
   which all start the service.
5. **The node crate lint gate** (P10). `cargo clippy --release -p
   cerulea-node -- -D warnings` reports style findings in files that predate
   the security range. This bears on the acceptance command, which runs
   clippy over the workspace with warnings denied.
6. **The CycloneDX tool for node and a pnpm install** (P10-022). The bill of
   materials reader for the node repositories stops on a pnpm tree. This
   bears on criterion 15, where the bill of materials is one of the twenty
   five controls.
7. **The per-component call site of the message catalogue is still moving**
   (P4-044). Several screen groups hold their strings inline while the
   catalogue mirrors them. This bears on criterion 6 for a person reading a
   translated surface.

Two further questions opened during this run and sit in the register under
P12-011.

8. **The reserved platform scope belongs to no registered collection.** On the
   real runtime the permission module refuses the Cerulea operator role every
   one of its five platform duties, because `cerulea_platform` reaches the
   schema registry nowhere. The role is bounded, and on this tree it is
   bounded to nothing. Criterion 14 above records the measurement.
9. **The runtime declares a constant named after a token.** The settled
   architecture states that Cerulea carries no token.
   `chain/cerulea-runtime/src/lib.rs:133` declares `pub const Cerulea:
   Balance = UNIT` with a comment naming a token. This bears on the whole
   platform rather than on one criterion.

## Repeating the run

    # the chain side, inside the Linux subsystem
    cd chain
    cargo test --release -p cerulea-node --test acceptance_suite

    # the service side
    bash scripts/acceptance/run-all.sh

    # the whole acceptance command
    cd cerulea-build
    bash 05-scripts/verify.sh acceptance

Every runner writes into `test-results/acceptance/<criterion>` and records the
head of every repository beside its output.
