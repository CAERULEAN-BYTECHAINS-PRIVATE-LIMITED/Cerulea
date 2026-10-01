# Cerulea platform acceptance

Task P12-011, final launch pass of 1 October 2026. Section 18.2 of the build
specification states eighteen criteria. This document records what each one
reached on the build machine in this pass, the evidence it left and the
command that produced it. A nineteenth section, outside the eighteen, records
the network exercises on nine validators.

A criterion passes here only on evidence this pass gathered. A figure below its
target is written as the figure it reached, with the size of the gap. A
failure is written as measured, with the path of its log. Nothing here carries
a pass forward from the run of 19 September.

## The machine

The build machine carries eight logical cores and 32 GB of memory. It runs
Windows 11 with a Linux subsystem, Ubuntu 24.04. The subsystem holds 11 GB of
that memory. Cargo runs inside the subsystem alone. PostgreSQL 16 answers on
port 5544 and Redis runs inside the subsystem.

Other streams worked on the machine during this pass. A coverage stream and a
pallet defect stream held the cargo queue in turns. A multi-host stream ran
nine validator containers from 15:58 to 16:42. Times in this document read
India Standard Time unless a line names the coordinated time scale. Every
figure here reads the machine under that load.

Each node_modules tree carries the binaries of the side that installed it.
Cerulea APIs carries Linux binaries. Cerulea Studio, Cerulea Explorer and
Cerulea Dashboard carry Windows ones. Each suite therefore runs on the side
its binaries serve, and each record names its side.

## Repository heads under test

| Repository | Head under test | Where it ran |
| --- | --- | --- |
| chain | `8274288`, the frozen commit of `NOTICE freeze chain 8274288` | Linux subsystem, git archive export |
| apis | `48e3b8a` for criteria 6, 9 to 13, 15 and 18; `22c978e` for the secret scan | Linux subsystem, git archive export; Windows for the browser journey |
| studio | `3036e02` | Windows, git archive export |
| explorer | `9f4fec3` | Windows, git archive export |
| dashboard | `5cf0946` | Windows, git archive export |
| content | `1238d16` | read by both sides |
| scripts | `a399593` for the runners; `6f9f6e1` for the secret scan | Linux subsystem, git archive export |
| security | `7269822` | Linux subsystem, git archive export |
| intelligence-corpus | `01959fe` | read in place |
| test-results | `fe84828` holds the service and chain gate logs; `40ff3b8` holds criterion 11 and the route gate over three projects | |
| multi-host records | test-results `29b1e28` | nine validator containers inside the subsystem |

Between the start of this pass and the record, apis moved from `48e3b8a` to
`22c978e`. That commit changed one test transcript and its test, and
nothing under `src`. Scripts moved from `a399593` to `6f9f6e1` in the
multi-host scripts alone. The secret scan ran again on both new heads. No
other criterion reads the changed files.

Every recorded run carries the heads of its own moment in `COMMITS.txt`
beside its logs.

## How the run is arranged

Nine criteria hold one test each in
`chain/cerulea-node/tests/acceptance_suite.rs`: criteria 1 to 5, 7, 8, 14 and
the chain part of 17. The chain gate runs the whole workspace and that suite
on a clean export of the frozen commit:

    cargo fmt --all -- --check
    cargo clippy --release --workspace --all-targets -- -D warnings
    cargo clippy --release -- -D warnings
    cargo test --release --workspace --no-fail-fast
    cargo test --release -p cerulea-node --test acceptance_suite
    cargo audit --no-fetch

The second clippy line is the form `cerulea-build/05-scripts/verify.sh chain`
runs. The first adds every test target.

Each cargo call waited for the shared queue through `flock -o
/root/cargo-queue.lock` and ran with three jobs. The source was
`/root/cerulea-frozen-8274288`, the export the orchestrator built the release
node from, with `/root/cerulea-clean-target` as its target. A second archive of
`8274288` taken by this pass matches that export file for file.

The service criteria run through the runners under `scripts/acceptance`. Each
runner ran from a git archive export of every repository it exercises, under
`/root/acc` inside the subsystem, so no uncommitted file of another stream
reached a result. Four details of the arrangement differ from 19 September.

1. The apis node_modules tree was copied onto the Linux volume beside the
   export. Read from the Windows volume, collection took 418 seconds for six
   files; from the Linux volume it takes eight.
2. Every vitest call on the default apis project carries
   `--testTimeout=60000`, as the pass instructions ask. The security project
   keeps its own limit of 120 seconds and the intelligence project its own
   600 seconds.
3. The route gate of criterion 10 runs its three test projects one at a time
   into one route log. The intelligence project starts the model server, and
   the model server waits for the multi-host exercises. The comparison is the
   one `scripts/route-coverage.mjs --check` makes, run over that log.
4. The Studio step of criterion 12 runs on Windows, since Studio carries
   Windows binaries. The runner reaches it from the subsystem first, where it
   stops on the absent Linux build of the bundler. Both logs are kept.

## Summary

| # | Criterion | Result |
| --- | --- | --- |
| 1 | The chain runs | Passed |
| 2 | Partitioned execution works | Passed |
| 3 | Class thresholds hold | Passed |
| 4 | Custody runs automatically | Passed |
| 5 | The solver produces valid profiles | Passed |
| 6 | A non-technical journey completes | Passed. 7 of 7 steps, 7 of 7 cross-service checks |
| 7 | All 225 modules install | Passed. 232 of 232 fragments |
| 8 | Every bridge connects | Passed |
| 9 | The generated application works | Passed. 242 tests over four groups |
| 10 | The generated surface works | Passed. 61 tests, route gate 246 of 246 |
| 11 | Cerulea AI acts and reverts | Passed. 31 and 228 tests |
| 12 | The manifest is complete | Passed. 136 tests over 16 files |
| 13 | Deployment is atomic | Passed. 5 of 5 and 43 passed, 1 skipped of 44 |
| 14 | The operator role is bounded | Passed, with the duties resolving to `PermissionDenied` |
| 15 | Security passes | Passed. 25 of 25 controls, 243 tests, secret scan clean on rerun |
| 16 | Testing passes | Shortfall. Chain 89.86 per cent lines against 90, consensus 64.80, node library unmeasured; 3 chain test failures on 8274288 |
| 17 | The term check passes | Passed. 11 of 11 repositories |
| 18 | Recovery works | Passed |

Every criterion carries a measurement from this pass. Criterion 16 also
reads two coverage figures from records written the same day, for the chain
and for Cerulea APIs, and names them as records. Seventeen passed. One
carries a measured shortfall: criterion 16, where the chain reads 89.86 per cent of
lines against the target of 90, the consensus crate reads 64.80, the node
library has no figure, and the workspace run on the frozen commit read three
test failures.

The chain gate failed on three steps: the format check (four diffs, fixed by
the formatting commit `1fc15e4`), clippy over every target (sixteen findings
in test code) and the workspace tests (4,352 passed, 3 failed, 2 ignored).
The clippy form `verify.sh` runs is clean and the acceptance suite reads nine
of nine. The gate section below carries each failure with its log.

Compared with 19 September, criterion 15 moves from shortfall to passed. The
software bill of materials now covers the chain, and the four chain controls
hold fresh test evidence. Criterion 16 stays a shortfall, with the chain gap
narrowed from 19.91 points over six crates to 0.14 points over 27.

The nine validator run on the frozen node held safety and liveness in all six
fault exercises that ran. The clock exercise was blocked by tooling.

## The eighteen criteria

Criteria 1 to 5, 7, 8, 14 and 17 each hold one test in
`chain/cerulea-node/tests/acceptance_suite.rs`. On the frozen commit the suite
read nine passed of nine in 4.43 seconds, one test at a time:

    running 9 tests
    test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

The suite changed in one line since 19 September: the operator role of
criterion 14 is now appointed by the root origin (chain `b29c8f3`).

### 1. The chain runs

**Passed.**

Three validators run in one process, each with its own keystore holding an
sr25519 draw key and an ed25519 seal key. The run commits blocks across two
epoch boundaries. Every committed block carries a seal binding the draw key of
the author to the seal key of the author, and every node accepts that seal. A
forged block, signed by one node while naming the scheduled author, fails
import, and the refusal names the fix.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_one_the_chain_runs_with_sealed_verified_authorship`
- Chain: `8274288`

### 2. Partitioned execution works

**Passed.**

One block carrying eight transactions across four conflict domains runs
through the Execution Engine. One worker and eight workers reach the same
per-domain roots and the same combined root, a repeated run reaches the same
roots, and a changed transaction changes the combined root. The claim holds
while the network commits, at every height of a six slot run.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_two_partitioned_execution_reaches_identical_roots`
- Chain: `8274288`

### 3. Class thresholds hold

**Passed.**

The class table carries routine at 51 percent, financial at 67, structural at
80 and constitutional at 90. Over one hundred validators the routine domain
commits on the fifty first signature and the constitutional domain on the
ninetieth. A domain carrying several classes commits at the share of the
highest class present. A release inside the constitutional delay window of one
hundred blocks fails and names the blocks remaining, a release at the
effective block succeeds, and a second release of the same change fails.

Two failures of the workspace run sit near this criterion: the governance
integration tests of the runtime library (see the chain gate below). The
acceptance test itself passed.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_three_class_thresholds_hold_with_the_constitutional_delay`
- Chain: `8274288`

### 4. Custody runs automatically

**Passed.**

Three hundred blocks at a challenge interval of ten issue thirty rounds. Each
round derives the challenge from the block hash and the round number, selects
one validator and one historical record, assembles the answer from the local
copy, verifies the proof against the record root and moves the custody share.
No person takes part at any step. A validator holding recent history alone
misses challenges and its standing falls. An incorrect proof lowers the share
and counts a fault. The custody module suite beside it read 101 passed of 101
in the workspace run.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`,
  `workspace-suites.txt`
- Test: `criterion_four_custody_runs_automatically_over_a_sustained_run`
- Chain: `8274288`

### 5. The solver produces valid profiles

**Passed.**

The five plain-language declarations carry 108 declaration sets. Every set
produced between three and five profiles, and every profile passed the fixed
limits twice, through the profile inspection the Amendment Engine runs and the
point inspection beneath it. Every profile states its consequences in plain
sentences. A point below four validators fails inspection.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_five_the_solver_yields_valid_profiles_alone`
- Chain: `8274288`

### 6. A non-technical journey completes

**Passed. Seven of seven journey steps, seven of seven cross-service checks.**

| Step | Result |
| --- | --- |
| The person signs up and opens a session | passed, 1.3 s |
| The person answers plain questions and Cerulea answers with outcomes | passed |
| The person builds the system and the compiler accepts it | passed |
| The deployment reaches a live result | passed, 3.7 s |
| The person writes the first record and reads it back | passed |
| The screens the person passes show no code | passed, 5.2 s |
| The person reaches the record through the live application in a browser | passed, 2.3 s |

The cross-service file passed seven of seven. The deployment installs the
Blueprint on the Blockchain. A record written through the generated surface
reads back with a proof. The generated application renders the collection in
a browser and serves its own script and its offline worker. A key holding the
wrong role reaches no record it cannot read. Cerulea Explorer and Cerulea
Dashboard open with no page error.

A fresh Cerulea APIs ran on port 8180 from a clean export of apis `48e3b8a`,
built in this pass, against its own database `cerulea_apis_acc_e2e`. Cerulea
Explorer on 3002 and Cerulea Dashboard on 3003 are the running production
builds of explorer `9f4fec3` and dashboard `5cf0946`. Chromium ran on Windows.

- Evidence: `test-results/acceptance/06-non-technical-journey/journey.log`,
  `cross-service.log`, `apis-8180.log`, `COMMITS.txt`
- Command: the two steps of `scripts/acceptance/06-non-technical-journey.sh`,
  the browser runner over `e2e/journey.spec.ts` and `e2e/cross-service.spec.ts`
- Side: Windows

### 7. All 225 modules install

**Passed. 232 of 232 fragments installed and executed.**

| Measurement | Figure |
| --- | --- |
| Fragments read | 232 |
| Blueprints installed | 232 of 232 |
| Fragments declaring an action | 232 |
| Fragments running an action | 232 of 232 |

The catalogue holds the 225 modules, the worked example and the six
Government modules. Every fragment resolved its dependency closure, merged,
validated and installed on its own test chain, and ran an action either to
success or to a precondition refusal carrying the plain sentence written
beside the rule. The catalogue came from content `1238d16`.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_seven_every_module_installs_and_executes`
- Chain: `8274288`

### 8. Every bridge connects

**Passed.**

The bridge module ships one verifier for each of the seven target networks.
The acceptance test asserts the seven kinds are present and distinct, that the
message identifier separates the two directions, that a changed sender,
position or payload each take their own slot, that the commitment covers the
class and the expiry, that a message stops being acceptable at its expiry, and
that all sixteen faults a verifier raises name the fix. The bridge module
suite read 236 passed of 236, including
`the_ethereum_virtual_machine_round_trip_completes` and its six companions.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`,
  `workspace-suites.txt`
- Test: `criterion_eight_every_bridge_carries_a_message_with_proof`
- Chain: `8274288`

### 9. The generated application works

**Passed. 242 tests over four groups, no failure.**

| Group | Tests | Result |
| --- | --- | --- |
| Screens, every collection | 70 over 6 files | passed |
| Permissions, visibility, binding, filtering and delivery | 43 over 4 files | passed |
| Offline capture and synchronisation | 29 over 2 files | passed |
| Rendering, responsiveness, theme and components | 100 over 8 files | passed |

- Evidence: `test-results/acceptance/09-generated-application/`
- Command: `bash scripts/acceptance/09-generated-application.sh`
- Side: the Linux subsystem, apis `48e3b8a` exported

### 10. The generated surface works

**Passed. 61 tests over three groups, and the route gate reads 246 of 246
routes.**

| Group | Tests | Result |
| --- | --- | --- |
| Endpoints, records, queries, actions, console and diagnostics | 39 over 5 files | passed |
| Webhooks and the change stream | 14 over 2 files | passed |
| Proofs | 8 over 1 file | passed |
| Route gate | 246 routes reached, 246 named in the manifest | ROUTE COVERAGE PASSED |

The route gate ran the default project and the security project of Cerulea
APIs with the route recorder on. Together they reached every one of the 246
routes the manifest names, and reached no route the manifest lacks. The
intelligence project, the third project the gate script runs, joined the same
log in the model window during criterion 11. The log still read 246 routes,
and the comparison over all three projects reads ROUTE COVERAGE PASSED.

The default project is the whole apis suite. It read 1,984 passed, 1 failed
and 46 skipped of 2,031 tests over 205 files. Two files failed.

1. `test/database.test.ts` asks every table for a creation instant, a change
   instant and a row version. One table carries no row version. The same test
   failed in the coverage run of apis `b168dd2` and in the run of 19
   September (P3-039).
2. `test/engine/visual.test.ts` failed to start. The headless browser inside
   the subsystem lacks the shared library `libnspr4.so`. No test of that file
   ran.

The gate script itself passes on a run with failures, since it compares
routes alone. Both failures sit outside the four groups of this criterion.

The first run of the committed gate script was stopped by this pass before its
third project, so the model server stayed down while Rust builds ran. Its
partial log is kept.

- Evidence: `test-results/acceptance/10-generated-surface/endpoints.log`,
  `webhooks.log`, `proofs.log`, `route-coverage-default.log`,
  `route-coverage-security.log`, `routes-reached.log`, `route-gate.log`,
  `route-coverage.stopped.log`
- Command: `bash scripts/acceptance/10-generated-surface.sh` for the three
  groups; the route gate as described under the arrangement above
- Side: the Linux subsystem, apis `48e3b8a` exported

### 11. Cerulea AI acts and reverts

**Passed. 259 tests: 31 over 3 files for patch and revert, 228 over 22 files
for the whole Cerulea Intelligence project.**

| Group | Tests | Result |
| --- | --- | --- |
| Patch generation, patch validation and reversal | 31 over 3 files | passed |
| The whole Cerulea Intelligence project | 228 over 22 files | passed, 186.6 seconds |

Cerulea AI proposes a patch, the patch passes type validation against the
Blueprint before it reaches the person, the person applies it and a checkpoint
lands on the apply. `test/intelligence/reversal.test.ts` carries the three
permission tiers, the checkpoint writer, the project timeline with one-action
restore, the selective revert with dependency detection and the actor
attribution on every checkpoint.

The local model server ran on port 8099 from `/root/start-llama.sh`: Qwen3 8B
at 4-bit, weights `/root/models/Qwen3-8B-Q4_K_M.gguf`, inside the subsystem.
It started after the multi-host completion line at 16:45. The run held the
cargo queue for its whole length, so no Rust build ran beside the model, as
`CLAUDE.md` asks. The teardown of the project stops every serving process, so
the run started the server again afterwards and left it running, as the pass
instructions ask.

- Evidence: `test-results/acceptance/11-intelligence/patch-and-revert.log`,
  `whole-project.log`, `COMMITS.txt`
- Command: `bash scripts/acceptance/11-intelligence.sh`
- Side: the Linux subsystem, apis `48e3b8a` exported, intelligence-corpus
  `01959fe`

### 12. The manifest is complete

**Passed. 136 tests over 16 files.**

| Group | Tests | Side | Result |
| --- | --- | --- | --- |
| Manifest production in the compiler | 35 over 5 files | Linux subsystem, apis `48e3b8a` | passed |
| Manifest views in Cerulea Studio | 101 over 11 files | Windows, studio `3036e02` | passed |

The runner reached the Studio step from the subsystem first. There the
bundler stopped on its absent Linux build, `@rollup/rollup-linux-x64-gnu`,
before any test ran, and the runner recorded a shortfall. That log is kept as
`studio-manifest-views.linux-side.log`. The same command then ran on Windows
from a clean export of studio `3036e02` and passed 101 of 101.

- Evidence: `test-results/acceptance/12-manifest/manifest-production.log`,
  `studio-manifest-views.windows.log`
- Command: `bash scripts/acceptance/12-manifest.sh`, then the Studio step on
  Windows

### 13. Deployment is atomic

**Passed. The rollback suite reads 5 of 5 and the stage suites read 43 passed
and 1 skipped of 44.**

| Group | Tests | Result |
| --- | --- | --- |
| Atomic rollback across every stage | 5 of 5 | passed |
| The seven deployment stages | 43 passed, 1 skipped of 44 | no failure |

The skipped test is `runs real validator processes when the operator names a
node binary`. It runs when `CERULEA_NODE_BIN` names a node binary. The runner
names none, so the test skips by its own condition.

- Evidence: `test-results/acceptance/13-atomic-deployment/`
- Command: `bash scripts/acceptance/13-atomic-deployment.sh`
- Side: the Linux subsystem, apis `48e3b8a` exported

### 14. The operator role is bounded

**Passed, with the measured note below.**

The root origin appoints the Cerulea operator role. Every request naming a
customer collection, a customer role or the Blueprint aborts through the
permission module and leaves a record carrying the actor, the moment and one
plain sentence stating why. The customer record stood untouched after every
refused request. The licence then lapsed, block production stopped and the
deployment read `Lapsed`. Under suspension the record read back with its
inclusion proof, the proof folded to the collection root, and the audit log
exported every entry with its reason.

Measured note. The five platform duties of the operator each resolved to
`PermissionDenied` on this tree:

    Criterion 14. The five platform duties resolved to [PermissionDenied,
    PermissionDenied, PermissionDenied, PermissionDenied, PermissionDenied]

The reserved platform scope belongs to no registered collection, so the
permission module refuses it. The role is bounded, and on this tree it is
bounded to nothing. The question register carries the gap from 19 September.
The operator module suite read 33 passed of 33.

One failure of the workspace run sits near this criterion:
`the_reading_paths_answer_on_a_development_chain` in
`cerulea-node/tests/node_interface_test.rs` (see the chain gate below).

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_fourteen_the_operator_role_is_bounded`
- Chain: `8274288`

### 15. Security passes

**Passed. 25 of 25 controls collected, 243 of 243 security tests, and the
secret scan passes on its rerun. The first scan of this pass failed with four
matches, and two commits during the pass removed them.**

Controls implemented, certification not held.

| Step | Figure | Result |
| --- | --- | --- |
| The evidence set rebuild | 25 of 25 controls collected | EVIDENCE SET COMPLETE |
| The security project of Cerulea APIs | 243 tests over 25 files | passed |
| The secret scan, first run on apis `48e3b8a` and scripts `a399593` | 4 matches | failed |
| The secret scan, rerun on apis `22c978e` and scripts `6f9f6e1` | 2,661 tracked files, 26 patterns, no match | SECRET SCAN PASSED |
| The software bill of materials | five bills, the chain at 1,122 components | complete |
| The reproducible build | two builds, matching digests | DIGESTS MATCH |
| The chain controls P10-001 to P10-004 | their named tests in the chain gate on `8274288` | passed |

**The evidence set.** `security/collect-evidence.sh` ran the proving command
of every control in `security/controls.json` and collected all twenty five.
The control that lacked fresh evidence on 19 September, P10-022, the software
bill of materials, now produces a bill for every repository. Cerulea APIs
reads 509 components, Cerulea Studio 791, Cerulea Explorer 637, Cerulea
Dashboard 467 and the chain 1,122 from its crate metadata. The chain bill came
from `cargo metadata --locked` through the shared cargo queue. The tool
`cargo-cyclonedx` stays absent from the subsystem, and the crate metadata path
of `scripts/write-sbom.sh` carries the bill.

**The chain controls.** Four controls belong to the chain: equivocation
detection, checkpoint anchoring, peer diversity and bounded message paths. In
the register their proving command prints that the chain agent owns the
control, so the collector marks them collected on a printed line alone. Their
real evidence is the chain test suites, and the evidence files under
`chain/security/evidence` were last written on 19 September. These four are
the controls that lacked fresh evidence. This pass gathered it from the
chain gate on `8274288`. The tests the September evidence names all passed:
`proven_equivocation_returns_standing_to_zero_inside_one_epoch`,
`a_fresh_node_refuses_a_chain_that_stops_below_the_highest_anchor`,
`a_fresh_node_refuses_a_chain_that_misses_an_anchor` and
`a_private_chain_refuses_a_peer_outside_the_allow_list`. The consensus
library read 408 passed of 408 and the primitives library 64 of 64. A sweep
of the test names each control covers found 67 tests for equivocation, 83 for
anchors and checkpoints, 32 for peers and 181 for bounds and sizes, all
passing. The extract sits at `test-results/acceptance/15-security/chain-controls.txt`.
The evidence files under `chain/security/evidence` stay dated 19 September.

**The secret scan.** The first run over the tracked files of every repository
found four matches. One was a private key block in
`apis/test/transcripts/certificates/pebble.json`, the account key of a local
test certificate authority recorded in a transcript. Three were database
connection strings in `scripts/multihost/docker-compose.multihost.yml` lines
433 and 456 and `scripts/multihost/tenant-separation.sh` line 48, each built
from environment values. Apis `22c978e` replaced the key with its public half,
and scripts `6f9f6e1` moved the three strings to a value assembled at run
time. The rerun over 2,661 tracked files found no match. A run over the
export tree of the runner also counted three files under `apis/local`, which
git ignores and the export copied; that run is kept as
`secret-scan.export-walk.log` and the tracked-file runs carry the verdict.

**Findings.** `security/ASSESSMENT.md` records D-01, Next.js 14 in the three
web surfaces, as closed on 1 October: Studio at `ce4bf2e`, Explorer at
`9f4fec3` and Dashboard at `75f0795` run Next.js 15.5.26. The heads under test
carry those commits. D-02 and the low findings stay accepted as the assessment
states. The independent penetration test and the protocol audit remain for
the operator to commission.

- Evidence: `test-results/acceptance/15-security/evidence-set.log`,
  `evidence/`, `sbom/`, `CONTROLS.md`, `security-suites.log`,
  `secret-scan.tracked.log`, `secret-scan.tracked.rerun.log`,
  `secret-scan.tracked.rerun2.log`, `secret-scan.export-walk.log`
- Command: `bash scripts/acceptance/15-security.sh`, then
  `node scripts/ci/secret-scan.mjs <workspace root>` over tracked files
- Side: the Linux subsystem for the runner, Windows for the tracked-file
  scans

### 16. Testing passes

**Shortfall. The chain reads 89.86 per cent of lines against a target of 90,
a gap of 0.14 points, with the consensus crate at 64.80 and the node library
unmeasured. The chain workspace run on the frozen commit read 3 failures, and
the Studio run read 1 failure under load.**

Every category of Section 17.1 has a suite and a place, which
`test-results/PROGRAMME.md` names one by one. The target of Section 17.2 is 90
per cent.

| Gate | Figure | Measured | Against 90 |
| --- | --- | --- | --- |
| Cerulea Studio, frontend | 97.27 per cent statements, 22,023 of 22,641 | this pass, studio `3036e02` | above, by 7.27 points |
| Cerulea Explorer | 93.62 per cent statements, 3,703 of 3,955 | this pass, explorer `9f4fec3` | above, by 3.62 points |
| Cerulea Dashboard | 91.70 per cent statements, 5,363 of 5,848 | this pass, dashboard `5cf0946` | above, by 1.70 points |
| Cerulea APIs | 90.31 per cent statements, 57,310 of 63,455 | 1 October 08:30, apis `b168dd2`, read from the record | above, by 0.31 points, on `b168dd2` |
| The chain, 27 crates | 89.86 per cent lines, 68,834 of 76,605 | 1 October 16:43, chain `6fda0ee`, read from the coverage stream record | **below, by 0.14 points** |
| The chain, consensus crate | 64.80 per cent lines, 9,466 of 14,608 | the same record | **below, by 25.20 points** |
| The chain, node library | not measured | the same record | **no figure** |

**Cerulea Studio.** The record states 97.36 per cent on studio `a6c1867`.
Studio has since moved to Next.js 15 and React 19, which changed 45 source
files. This pass measured the frontend again on studio `3036e02` and reads
97.27 per cent statements, 90.36 branches and 92.65 functions. The run read
1,676 passed and 1 failed of 1,677 tests over 137 files in 969 seconds, with
two threads. The failing test, `Step 2 Compose saves the selection to the
build state and the records to the Blueprint`, reached its own limit of 40
seconds in the full run. Run alone, its file passed 3 of 3 and that test took
21.7 seconds. The failure is written as measured.

**Cerulea Explorer and Cerulea Dashboard.** The record of 19 September predates
the move to Next.js 15. This pass measured both again: Explorer 75 of 75 tests
over 13 files, Dashboard 122 of 122 over 27 files, no failure.

**Cerulea APIs.** The figure of 90.31 per cent comes from
`test-results/coverage/typescript.md`, the merged report of four shards over a
clean export of apis `b168dd2`. That run read 1,918 passed, 12 failed and 37
skipped of 1,967 tests. Nine failures sit in `test/dashboard/panels.test.ts`,
two in `test/p11-cov2-domains-rfc2136.test.ts` and one in
`test/database.test.ts`. Since `b168dd2`, apis gained 6,606 lines across 44
source files, most of them the live chain writer of `e8e15b5`. No coverage
figure exists for apis `48e3b8a` or `22c978e`. A fresh measurement takes about
100 minutes of four shards on this machine and did not fit the window, so the
figure stands for `b168dd2` alone.

**The chain.** The final record of the coverage stream,
`test-results/coverage/chain.md` with `chain-llvm-cov.txt` (test-results
`b9d44fa`), reads **89.86 per cent of lines, 68,834 of 76,605, over 27
crates**, at chain `6fda0ee`. Regions read 82.98 and functions 80.85. That
figure sits 0.14 points below the target of 90. Commit `6fda0ee` follows the
frozen commit with test files and the formatting commit alone, so its source
equals `8274288` in every token apart from one trailing comma. Twenty six of
the 27 crates read 91 per cent or more. The consensus crate, which holds the
eight engines, reads 64.80 per cent, 25.20 points below the target. The node
library has no figure: its instrumented build stops at compiler error E0275,
and the record names that as an open item. The measured coverage runs read
3,970 passed, 0 failed and 1 ignored. This pass did not repeat the
measurement; the figure comes from the record, which the coverage stream
gathered on this machine the same afternoon.

- Evidence: `test-results/acceptance/16-testing/studio-coverage.log`,
  `studio-coverage-summary.json`, `studio-coverage-gate.log`,
  `studio-step2compose-rerun.log`, `explorer-coverage.log`,
  `dashboard-coverage.log` with their summaries and gates,
  `apis-coverage-gate.log`, `apis-coverage-summary.b168dd2.json`,
  `categories.log`; the chain record at `test-results/coverage/chain.md` and
  `chain-llvm-cov.txt`; the chain test failures at
  `test-results/acceptance/01-to-17-chain/workspace-tests.log`
- Command: `node node_modules/vitest/vitest.mjs run --config
  vitest.coverage.config.ts --testTimeout=60000` in each repository, then
  `node scripts/ci/coverage-gate.mjs --summary coverage/coverage-summary.json
  --target 90`
- Side: Windows for Studio, Explorer and Dashboard

### 17. The term check passes

**Passed. Eleven of eleven repositories, and the acceptance suite test.**

The guardrail ran over a git archive export of every repository at its head.

| Repository | Head | Result |
| --- | --- | --- |
| chain | `8274288`, frozen | TERM CHECK PASSED |
| apis | `48e3b8a` | TERM CHECK PASSED |
| studio | `3036e02` | TERM CHECK PASSED |
| explorer | `9f4fec3` | TERM CHECK PASSED |
| dashboard | `5cf0946` | TERM CHECK PASSED |
| content | `1238d16` | TERM CHECK PASSED |
| scripts | `a399593` | TERM CHECK PASSED |
| security | `7269822` | TERM CHECK PASSED |
| test-results | `1d3a4c5` | TERM CHECK PASSED |
| docs | `e6178a2` | TERM CHECK PASSED |
| intelligence-corpus | `01959fe` | TERM CHECK PASSED |

The first sweep also read chain `e1f3c2c` and passed.

- Evidence: `test-results/acceptance/17-term-check/term-check.log`,
  `test-results/acceptance/01-to-17-chain/term-check.log`,
  `acceptance-suite.log`
- Command: `bash cerulea-build/04-guardrails/term-check.sh <repository>`, and
  `criterion_seventeen_the_term_check_passes` in the acceptance suite

### 18. Recovery works

**Passed.**

| Step | Result |
| --- | --- |
| Restoration | RESTORATION PASSED |
| Exit export | EXPORT VERIFICATION PASSED, EXIT EXPORT PASSED |
| Backup and export suites of Cerulea APIs | 22 tests over 4 files, passed |

The restoration took snapshot `snapshot_0a374ffb75144390b7b883af4e43fab3` of a
deployment holding five records over four blocks. It started a fresh instance
with an empty schema, carried the keys across and restored 21 platform rows
and the chain state. On the fresh instance the record `parcels/parcels-0001`
read back with its stored bytes, its inclusion proof folded to the state root,
and block production moved from block 3 to block 5.

The exit export wrote 64 files: five records, four blocks, one Blueprint
version and four generated application files. Every file carries the digest
the index names. A fresh instance started from the export alone read the same
record with its proof and produced blocks.

These steps run the local chain driver of Cerulea APIs, the driver local mode
serves. No step of this criterion started the Rust node.

- Evidence: `test-results/acceptance/18-recovery/restoration.log`,
  `exit-export.log`, `apis-recovery.log`
- Command: `bash scripts/acceptance/18-recovery.sh`
- Side: the Linux subsystem, apis `48e3b8a` exported, scripts `a399593`
  exported

## The chain gate

The full workspace gate ran on the frozen commit `8274288`, from
`/root/cerulea-frozen-8274288` with target `/root/cerulea-clean-target`, three
cargo jobs, every call queued. **The gate failed on three of its steps.**

| Step | Command | Result |
| --- | --- | --- |
| Term check | `term-check.sh` over the export | TERM CHECK PASSED |
| Format | `cargo fmt --all -- --check` | **failed**, 4 diffs in two files |
| Lint, every target | `cargo clippy --release --workspace --all-targets -- -D warnings` | **failed**, 16 errors in test targets |
| Lint, the form `verify.sh` runs | `cargo clippy --release -- -D warnings` | clean, exit 0 |
| Workspace tests | `cargo test --release --workspace --no-fail-fast` | **4,352 passed, 3 failed, 2 ignored over 96 suites** |
| Acceptance suite | `cargo test --release -p cerulea-node --test acceptance_suite` | 9 passed of 9 |
| Dependency audit | `cargo audit --no-fetch` | exit 0, 4 allowed warnings |

**Format.** The frozen commit carries four format diffs in the bridge
pallet: `src/live_evidence.rs` at lines 277, 408 and 478, and
`src/networks/polkadot.rs` at line 281. The following commit `1fc15e4`
changes those two files alone. `git diff -w 8274288 1fc15e4` shows line
breaks, and the whitespace-stripped content differs in one token, a trailing
comma rustfmt drops when it joins the call at `live_evidence.rs` line 489.
`cargo fmt --all -- --check` on a clean export of `1fc15e4` exits 0. The binary
under test and every test count above come from `8274288`.

**Lint.** Every one of the sixteen errors sits in test code.
`cerulea-runtime/tests/runtime_api_tests.rs` holds thirteen `assert!(true)`
lines. The library test target of `cerulea-consensus` holds three: a `push`
straight after creation at `engines/commit/collect.rs` line 422, inside its
test module, and a cast to the same type at `tests/metrics_test.rs` line 170.

**Workspace tests.** Three tests failed.

1. `cerulea-node`, `tests/node_interface_test.rs`,
   `the_reading_paths_answer_on_a_development_chain`, line 321. The collection
   root of a collection the chain never registered answers `None`, and the
   test expects the empty root.
2. `cerulea-runtime`, library tests,
   `runtime_integration_tests::test_governance_integration`, line 151. The
   proposal ends `Failed`, and the test expects `Executed`.
3. `cerulea-runtime`, library tests,
   `runtime_integration_tests::test_error_handling`, line 363. A vote by a
   non-validator fails with `NotValidator`, and the test expects
   `ProposalNotApproved`.

The two runtime tests meet the vote permission rule of the pallet defect fixes
committed before the freeze. The interface test meets runtime `8a5b31c`, by which a
collection root answers nothing for an undeclared collection. Each failure is
written as measured. Neither file is a coverage-only test: the interface test
dates from P2-052 and the runtime tests from P2-051 and P1-046.

After the freeze, chain `6fda0ee` corrected all three tests, a change to test
files alone. The coverage stream reports the runtime integration suite at 12
passed and the interface test at 8 passed on that commit. This pass did not
rerun them: the model server stays up after criterion 11, and `CLAUDE.md`
keeps Cerulea AI and Rust builds apart. The counts above stand for
`8274288`.

The dcf pallet suite read 1,194 passed, 0 failed and 1 ignored. The ignored
test names the open defect group (h), the migration record from storage
version 0 to 1.

- Evidence: `test-results/acceptance/01-to-17-chain/fmt.log`,
  `fmt-follow-up.log`, `clippy.log`, `clippy-verify-form.log`,
  `workspace-tests.log`, `workspace-suites.txt`, `workspace-counts.txt`,
  `acceptance-suite.log`, `cargo-audit.log`, `term-check.log`, `COMMITS.txt`

## Network exercises

This section sits outside the eighteen criteria. Nine Cerulea validators ran
on three container networks joined through a router container, each network
standing in for one machine. Every validator ran under a cap of half a core
and 700 MB. The scripts sit under `scripts/multihost/` and the procedure under
`docs/MULTI_HOST_SETUP.md`.

Each exercise carries two verdicts. Safety holds when no two conflicting
blocks commit at one height, read from the hash every validator committed at
every height. Liveness holds when every validator commits past the highest
height of the fault within 300 seconds of the heal.

### The run on the frozen node 8274288

**Network verification passed. Safety held in all six fault exercises that
ran. Liveness held in all six. The clock exercise was blocked by tooling, so
it carries no verdict.**

The node binary hashes `031397ac9a9a0296...`, built from a clean export of
`8274288`, and the multi-host stream confirmed that hash inside the image
before the run. The run went from 10:28 to 11:12 UTC. Every validator carries
the same commit weight, 3,555 of 31,995, against a commit threshold of 21,437.

| Exercise | Safety | Liveness |
| --- | --- | --- |
| Network verification | One chain from height 141 to 200 on all nine, the same author and hash for 60 blocks, the outside node held 0 peers | 12 blocks committed in the window at 77.77 to 100 per cent of the weight; all four class thresholds cleared |
| Partition of three | Held over 320 heights | Held. Commitment stopped at 270 while one machine was cut off, and returned 45 seconds after the heal |
| Node loss | Held over 640 heights | Held. 90 blocks committed while validator eight was down, returned 30 seconds after the heal |
| Delay | Held over 920 heights | Held. 50 blocks committed under the delay, returned 30 seconds after the heal |
| Loss | Held over 1,140 heights | Held. 120 blocks committed under the loss, returned 15 seconds after the heal |
| Delay held for one epoch plus one minute | Held over 1,390 heights | Held. 110 blocks committed, returned 45 seconds after the heal |
| Partition of four | Held over 1,560 heights | Held. Neither side committed, all nine committed again 45 seconds after the heal |
| Clock | Blocked by tooling | Blocked by tooling |

Three measured observations stand beside the verdicts.

1. **A machine carrying three of nine validators halts commitment while it is
   away.** Six equal validators hold 21,330 of weight, under the threshold of
   21,437. The partition of three stopped commitment for its whole duration.
   Liveness returned inside the bound, and the multi-host stream filed a
   request for a decision on placement or weight.
2. **The clock exercise has never run.** Its time shim starts in a separate
   container and aborts there. No run on any node has measured a clock
   offset.
3. **The standing records read no custody activity on the network.** Over the
   verification window every validator reads `custodyAnswered` 0,
   `custodyMissed` 0 and a custody share of 0. Validator eight read the same
   before its loss and after its restart, with `custodyResumed` false.
   Criterion 4 proves custody in the acceptance suite. The nine validator
   network shows no custody round in these records.

- Evidence: `test-results/multihost/frozen-node-8274288/network-verification.json`,
  `exercise-*.json`, `fault-exercises.log`, `run-all.log`, `node-logs/`, and
  `test-results/multihost/index.md`; test-results `29b1e28`

### Earlier runs

These runs led to the fixes the frozen node carries. They are history and
carry no verdict for the frozen node.

**Node c411dab**, the block import fix. Safety broke in the delay exercise at
height 1440, where validators five and eight committed a block apart from the
other seven, and validator six committed blocks 3277 to 3280 apart from the
other eight during the clock exercise. Both came from second votes in one
round, which later nodes refuse. Records:
`test-results/multihost/first-node-c411dab/`.

**Node 36b1118**, vote lock, round timeout and a separate draw key type.
Safety held in every exercise. Liveness returned after the partition of three,
the node loss and a repeated delay. Liveness failed twice: finality stalled at
1400 after the first delay and at 280 after the loss, and never returned. The
validators locked on one block and voted for it every round while the gossip
validator refused those votes once the chain entered a later epoch. The clock
exercise was blocked by tooling. Records: `test-results/multihost/second-node-36b1118/`.

**Node 80439ac**, the first liveness fix. Nothing committed on nine
validators. The gossip validator refused every incoming commit vote as a
runtime interface error from the first block, so each validator counted its
own vote alone. No exercise ran. Records:
`test-results/multihost/third-node-80439ac/`.

**Node fbd1f64**, finality from genesis. All nine voted and each committed
block gathered 88.88 to 100 per cent of the weight. Safety held in the six
exercises that ran. Liveness returned after the partition of three (90
seconds), the node loss (30 seconds) and the loss (75 seconds), and finality
advanced through the delay. After the delay heal the validators lost most of
their peers and dropped and rejoined them dozens of times a minute.
**Liveness then failed:** finality stopped at 930 at 08:21:43 UTC on seven
validators, two reached 940, and none returned. The delay held for one epoch
and the partition of four ran on the stalled network. The clock exercise was blocked by tooling. P1-022 at `2aafc75`
traced the stall to the commit vote gossip store, which expired nothing and
resent every vote since genesis every 750 ms. Records:
`test-results/multihost/fourth-node-fbd1f64/`.

## Open items that bear on acceptance

1. **The reserved platform scope belongs to no registered collection.** The
   five platform duties of the operator resolve to `PermissionDenied`
   (criterion 14). Open since 19 September.
2. **The faucet subcommand of the node is a stand-in.** On `8274288`,
   `cerulea-node/src/command.rs` line 85 prints that it would send and
   performs no transaction. The no-placeholder rule covers it.
3. **Two pallet defect groups stay open at the freeze.** Group (h), the
   migration from storage version 0 to 1 installs a governance record whose
   ranges and shares read zero, carried by one ignored test. Group (m), the
   event compatibility check reads an empty sample list and checks nothing.
   Both sit as `OPEN dcf` lines in `cerulea-build/02-tasks/status/launch.md`.
4. **The chain security evidence files are dated 19 September.** Fresh test
   evidence for the four chain controls sits in this record (criterion 15),
   and `chain/security/evidence` waits for its next collection.
5. **No Cerulea APIs coverage figure exists for the code under test.** The
   figure of 90.31 per cent stands for `b168dd2`.

## Compliance

Controls implemented, certification not held.

The control register sits at `security/controls.json` and maps every control
to ISO 27001, ISO 27017, ISO 27018 and the National Institute of Standards and
Technology Special Publication 800-53 revision 5. The mapping document
`security/CONTROLS.md` is written from that register by
`security/collect-evidence.sh`. This pass wrote a fresh copy of it beside the
evidence at `test-results/acceptance/15-security/CONTROLS.md`.

## Repeating the run

    # chain gate, inside the Linux subsystem, from a clean export of the commit
    cargo fmt --all -- --check
    cargo clippy --release --workspace --all-targets -- -D warnings
    cargo test --release --workspace --no-fail-fast
    cargo test --release -p cerulea-node --test acceptance_suite

    # one service criterion
    bash scripts/acceptance/<criterion>.sh

    # term check of one repository
    bash cerulea-build/04-guardrails/term-check.sh <repository>
