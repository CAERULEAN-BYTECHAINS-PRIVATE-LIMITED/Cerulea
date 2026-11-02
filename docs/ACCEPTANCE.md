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

Other work ran on the machine during this pass. The coverage measurement and
the pallet defect work held the cargo queue in turns. The multi-host exercise
ran nine validator containers from 15:58 to 16:42. Times in this document read
India Standard Time unless a line names the coordinated time scale. Every
figure here reads the machine under that load.

Each node_modules tree carries the binaries of the side that installed it.
Cerulea APIs carries Linux binaries. Cerulea Studio, Cerulea Explorer and
Cerulea Dashboard carry Windows ones. Each suite therefore runs on the side
its binaries serve, and each record names its side.

## Repository heads under test

| Repository | Head under test | Where it ran |
| --- | --- | --- |
| chain | `481207b`, the frozen commit of `NOTICE freeze chain 481207b`, the node binary under test; gate reruns on the test-only follow-ups `4a79d72` and `87cd338` | Linux subsystem, git archive export |
| apis | `d2cb3eb` for criteria 6, 9 to 13, 15 and 18; `8f6c717` for the secret scan | Linux subsystem, git archive export; Windows for the browser journey |
| studio | `9650b21` | Windows, git archive export |
| explorer | `17536c7` | Windows, git archive export |
| dashboard | `b7a0acd` | Windows, git archive export |
| content | `6ae5e56` | read by both sides |
| scripts | `c0e441b` for the runners; `7537381` for the secret scan | Linux subsystem, git archive export |
| security | `1fe1924` | Linux subsystem, git archive export |
| intelligence-corpus | `ed93257` | read in place |
| test-results | `71974f9` holds the service and chain gate logs; `86624ec` holds criterion 11 and the route gate over three projects; `d25b2f5` and the commit after it hold the follow-up gate lines | |
| multi-host records | test-results `b200439` | nine validator containers inside the subsystem |

Between the start of this pass and the record, apis moved from `d2cb3eb` to
`8f6c717`. That commit changed one test transcript and its test, and
nothing under `src`. Scripts moved from `c0e441b` to `7537381` in the
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

The second clippy line is the form the commit gate runs. The first adds every
test target.

Each cargo call waited for the shared queue through `flock -o
/root/cargo-queue.lock` and ran with three jobs. The source was
`/root/cerulea-frozen-481207b`, the export the release node was built from,
with `/root/cerulea-clean-target` as its target. A second archive of
`481207b` taken by this pass matches that export file for file.

The service criteria run through the runners under `scripts/acceptance`. Each
runner ran from a git archive export of every repository it exercises, under
`/root/acc` inside the subsystem, so no uncommitted working file reached a
result. Four details of the arrangement differ from 19 September.

1. The apis node_modules tree was copied onto the Linux volume beside the
   export. Read from the Windows volume, collection took 418 seconds for six
   files; from the Linux volume it takes eight.
2. Every vitest call on the default apis project carries
   `--testTimeout=60000`. The security project
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
| 16 | Testing passes | Shortfall. Chain 91.97 per cent lines over 27 crates; node library unmeasured; APIs figure older than the code; consensus 79.52; 3 test failures on 481207b |
| 17 | The term check passes | Passed. 11 of 11 repositories |
| 18 | Recovery works | Passed |

Every criterion carries a measurement from this pass. Criterion 16 also
reads two coverage figures from records written the same day, for the chain
and for Cerulea APIs, and names them as records. Seventeen passed. One
carries a measured shortfall: criterion 16. Every measured coverage figure
reads 90 per cent or more, with the chain at 91.97 per cent of lines over 27
crates. The node library has no figure, the Cerulea APIs figure stands for an
older commit, the consensus crate reads 79.52, and the workspace run on the
frozen commit read three test failures.

The chain gate on the frozen commit failed on three steps: the format check
(four diffs), clippy over every target (sixteen findings in test code) and
the workspace tests (4,352 passed, 3 failed, 2 ignored). The clippy form
`verify.sh` runs is clean and the acceptance suite reads nine of nine. The
test-only follow-up `4a79d72` reads 4,449 passed and 0 failed, and still fails
the format check and clippy over every target in test code. The lint
follow-up `87cd338` passes both and reads 4,449 passed and 0
failed. The
gate section below carries each line with its logs.

Compared with 19 September, criterion 15 moves from shortfall to passed. The
software bill of materials now covers the chain, and the four chain controls
hold fresh test evidence. Criterion 16 stays a shortfall. The chain line
figure rose from 70.09 per cent over six crates to 91.97 over 27.

The nine validator run on the frozen node held safety and liveness in all six
fault exercises that ran. The clock exercise was blocked by tooling.

## The eighteen criteria

Criteria 1 to 5, 7, 8, 14 and 17 each hold one test in
`chain/cerulea-node/tests/acceptance_suite.rs`. On the frozen commit the suite
read nine passed of nine in 4.43 seconds, one test at a time:

    running 9 tests
    test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

The suite changed in one line since 19 September: the operator role of
criterion 14 is now appointed by the root origin (chain `cb1abff`).

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
- Chain: `481207b`

### 2. Partitioned execution works

**Passed.**

One block carrying eight transactions across four conflict domains runs
through the Execution Engine. One worker and eight workers reach the same
per-domain roots and the same combined root, a repeated run reaches the same
roots, and a changed transaction changes the combined root. The claim holds
while the network commits, at every height of a six slot run.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_two_partitioned_execution_reaches_identical_roots`
- Chain: `481207b`

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
- Chain: `481207b`

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
- Chain: `481207b`

### 5. The solver produces valid profiles

**Passed.**

The five plain-language declarations carry 108 declaration sets. Every set
produced between three and five profiles, and every profile passed the fixed
limits twice, through the profile inspection the Amendment Engine runs and the
point inspection beneath it. Every profile states its consequences in plain
sentences. A point below four validators fails inspection.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_five_the_solver_yields_valid_profiles_alone`
- Chain: `481207b`

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

A fresh Cerulea APIs ran on port 8180 from a clean export of apis `d2cb3eb`,
built in this pass, against its own database `cerulea_apis_acc_e2e`. Cerulea
Explorer on 3002 and Cerulea Dashboard on 3003 are the running production
builds of explorer `17536c7` and dashboard `b7a0acd`. Chromium ran on Windows.

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
beside the rule. The catalogue came from content `6ae5e56`.

- Evidence: `test-results/acceptance/01-to-17-chain/acceptance-suite.log`
- Test: `criterion_seven_every_module_installs_and_executes`
- Chain: `481207b`

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
- Chain: `481207b`

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
- Side: the Linux subsystem, apis `d2cb3eb` exported

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
   failed in the coverage run of apis `1920324` and in the run of 19
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
- Side: the Linux subsystem, apis `d2cb3eb` exported

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
cargo queue for its whole length, so no Rust build ran beside the model.
Cerulea AI and Rust builds never run at the same time on this machine. The
teardown of the project stops every serving process, so the run started the
server again afterwards and left it running.

- Evidence: `test-results/acceptance/11-intelligence/patch-and-revert.log`,
  `whole-project.log`, `COMMITS.txt`
- Command: `bash scripts/acceptance/11-intelligence.sh`
- Side: the Linux subsystem, apis `d2cb3eb` exported, intelligence-corpus
  `ed93257`

### 12. The manifest is complete

**Passed. 136 tests over 16 files.**

| Group | Tests | Side | Result |
| --- | --- | --- | --- |
| Manifest production in the compiler | 35 over 5 files | Linux subsystem, apis `d2cb3eb` | passed |
| Manifest views in Cerulea Studio | 101 over 11 files | Windows, studio `9650b21` | passed |

The runner reached the Studio step from the subsystem first. There the
bundler stopped on its absent Linux build, `@rollup/rollup-linux-x64-gnu`,
before any test ran, and the runner recorded a shortfall. That log is kept as
`studio-manifest-views.linux-side.log`. The same command then ran on Windows
from a clean export of studio `9650b21` and passed 101 of 101.

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
- Side: the Linux subsystem, apis `d2cb3eb` exported

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
- Chain: `481207b`

### 15. Security passes

**Passed. 25 of 25 controls collected, 243 of 243 security tests, and the
secret scan passes on its rerun. The first scan of this pass failed with four
matches, and two commits during the pass removed them.**

Controls implemented, certification not held.

| Step | Figure | Result |
| --- | --- | --- |
| The evidence set rebuild | 25 of 25 controls collected | EVIDENCE SET COMPLETE |
| The security project of Cerulea APIs | 243 tests over 25 files | passed |
| The secret scan, first run on apis `d2cb3eb` and scripts `c0e441b` | 4 matches | failed |
| The secret scan, rerun on apis `8f6c717` and scripts `7537381` | 2,661 tracked files, 26 patterns, no match | SECRET SCAN PASSED |
| The software bill of materials | five bills, the chain at 1,122 components | complete |
| The reproducible build | two builds, matching digests | DIGESTS MATCH |
| The chain controls P10-001 to P10-004 | their named tests in the chain gate on `481207b` | passed |

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
the register their proving command prints that the control belongs to the
chain repository, so the collector marks them collected on a printed line
alone. Their real evidence is the chain test suites, and the evidence files
under `chain/security/evidence` were last written on 19 September. These four
are the controls that lacked fresh evidence. This pass gathered it from the
chain gate on `481207b`. The tests the September evidence names all passed:
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
from environment values. Apis `8f6c717` replaced the key with its public half,
and scripts `7537381` moved the three strings to a value assembled at run
time. The rerun over 2,661 tracked files found no match. A run over the
export tree of the runner also counted three files under `apis/local`, which
git ignores and the export copied; that run is kept as
`secret-scan.export-walk.log` and the tracked-file runs carry the verdict.

**Findings.** `security/ASSESSMENT.md` records D-01, Next.js 14 in the three
web surfaces, as closed on 1 October: Studio at `00bc146`, Explorer at
`17536c7` and Dashboard at `b0f87ec` run Next.js 15.5.26. The heads under test
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

**Shortfall. Every measured coverage figure reads 90 per cent or more, and
the chain reads 91.97 per cent of lines over 27 crates. The node library has
no figure, the Cerulea APIs figure stands for `1920324` rather than the code
under test, and the consensus crate reads 79.52. The workspace run on the
frozen commit read 3 failures, which the test-only follow-up `4a79d72`
corrects with 0 failures, and the Studio run read 1 failure under load.**

Every category of Section 17.1 has a suite and a place, which
`test-results/PROGRAMME.md` names one by one. The target of Section 17.2 is 90
per cent.

| Gate | Figure | Measured | Against 90 |
| --- | --- | --- | --- |
| Cerulea Studio, frontend | 97.27 per cent statements, 22,023 of 22,641 | this pass, studio `9650b21` | above, by 7.27 points |
| Cerulea Explorer | 93.62 per cent statements, 3,703 of 3,955 | this pass, explorer `17536c7` | above, by 3.62 points |
| Cerulea Dashboard | 91.70 per cent statements, 5,363 of 5,848 | this pass, dashboard `b7a0acd` | above, by 1.70 points |
| Cerulea APIs | 90.31 per cent statements, 57,310 of 63,455 | 1 October 08:30, apis `1920324`, read from the record | above, by 0.31 points, on `1920324` |
| The chain, 27 crates | 91.97 per cent lines, 74,330 of 80,817 | 1 October, chain `4a79d72`, read from the coverage record | above, by 1.97 points |
| The chain, consensus crate | 79.52 per cent lines, 14,736 of 18,532 | the same record | **below, by 10.48 points** |
| The chain, node library | not measured | the same record | **no figure** |

**Cerulea Studio.** The record states 97.36 per cent on studio `481bd8c`.
Studio has since moved to Next.js 15 and React 19, which changed 45 source
files. This pass measured the frontend again on studio `9650b21` and reads
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
clean export of apis `1920324`. That run read 1,918 passed, 12 failed and 37
skipped of 1,967 tests. Nine failures sit in `test/dashboard/panels.test.ts`,
two in `test/p11-cov2-domains-rfc2136.test.ts` and one in
`test/database.test.ts`. Since `1920324`, apis gained 6,606 lines across 44
source files, most of them the live chain writer of `f04872c`. No coverage
figure exists for apis `d2cb3eb` or `8f6c717`. A fresh measurement takes about
100 minutes of four shards on this machine and did not fit the window, so the
figure stands for `1920324` alone.

**The chain.** The final coverage record,
`test-results/coverage/chain.md` with `chain-llvm-cov.txt` (test-results
`0ee689a`), reads **91.97 per cent of lines, 74,330 of 80,817, over 27
crates**, at chain `4a79d72`. Regions read 86.29 and functions 81.95. That
figure sits 1.97 points above the target of 90. Commit `4a79d72` follows the
frozen commit with test files, test modules, two dev-dependencies and the
formatting commit alone, so the code it measures is the code of `481207b`.
The consensus crate, which holds the eight engines, reads 79.52 per cent,
10.48 points below the target, up from 64.80 at `fbf6c17`. The node library
sits outside the figure. Its instrumented build stops at compiler error E0275,
an overflow while the compiler proves the runtime call type can move between
threads, so it has no coverage figure. The record names that as an open item.
The earlier record at `fbf6c17` read 89.86 per cent. This pass did not repeat
the measurement; the figure comes from the record, which was gathered on this
machine the same afternoon.

- Evidence: `test-results/acceptance/16-testing/studio-coverage.log`,
  `studio-coverage-summary.json`, `studio-coverage-gate.log`,
  `studio-step2compose-rerun.log`, `explorer-coverage.log`,
  `dashboard-coverage.log` with their summaries and gates,
  `apis-coverage-gate.log`, `apis-coverage-summary.1920324.json`,
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
| chain | `481207b`, frozen | TERM CHECK PASSED |
| apis | `d2cb3eb` | TERM CHECK PASSED |
| studio | `9650b21` | TERM CHECK PASSED |
| explorer | `17536c7` | TERM CHECK PASSED |
| dashboard | `b7a0acd` | TERM CHECK PASSED |
| content | `6ae5e56` | TERM CHECK PASSED |
| scripts | `c0e441b` | TERM CHECK PASSED |
| security | `1fe1924` | TERM CHECK PASSED |
| test-results | `3271a1e` | TERM CHECK PASSED |
| docs | `54f372b` | TERM CHECK PASSED |
| intelligence-corpus | `ed93257` | TERM CHECK PASSED |

The first sweep also read chain `c666566` and passed.

- Evidence: `test-results/acceptance/17-term-check/term-check.log`,
  `test-results/acceptance/01-to-17-chain/term-check.log`,
  `acceptance-suite.log`
- Command: `bash scripts/term-check.sh <repository>`, and
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
- Side: the Linux subsystem, apis `d2cb3eb` exported, scripts `c0e441b`
  exported

## The chain gate

The gate ran on three commits. **The node binary under test is the build of
the frozen commit `481207b`** (sha256 `031397ac9a9a0296...`). The two later
commits change test files, `#[cfg(test)]` modules, two dev-dependencies and the
formatting commit `2edda44` alone, which the review in
`test-results/acceptance/01-to-17-chain/followup-4a79d72-diff-review.log`
confirms hunk by hunk.

| Gate line | Format | Clippy, every target | Workspace tests |
| --- | --- | --- | --- |
| Frozen commit `481207b`, as measured | **failed**, 4 diffs | **failed**, 16 findings in test code | **4,352 passed, 3 failed, 2 ignored** over 96 suites |
| Test-only follow-up `4a79d72`, as measured | **failed**, 8 diffs in `cerulea-consensus/src/test_support.rs` | **failed**, 10 findings in `pallet-cerulea-poi/src/tests.rs`; the build stopped at that crate | **4,449 passed, 0 failed, 2 ignored** over 96 suites |
| Lint follow-up `87cd338`, as measured | clean, exit 0 | clean, exit 0 | **4,449 passed, 0 failed, 2 ignored** over 96 suites |

The lint follow-up `87cd338` changes 13 files after `4a79d72`: files under
the test directories, `test_support.rs` and `tests_reader.rs`, both declared
under `#[cfg(test)]`. It carries formatting and lint changes in test code
alone. The full workspace suite ran on it as well, at six jobs, with the
model server stopped while cargo ran.

On `4a79d72` the consensus library reads 501 passed, the runtime library 72,
the node interface test 8 and the acceptance suite 9. The three tests that
failed on the frozen commit pass there, as corrected by `fbf6c17`. The
clippy findings on `4a79d72` are eight needless borrows, one module named as
its parent module and one needless `vec!`. Logs:
`test-results/acceptance/01-to-17-chain/followup-4a79d72/` and
`lint-followup-87cd338/`. The follow-up runs used six cargo jobs and the
same target.

### Frozen commit 481207b

The full workspace gate ran on the frozen commit `481207b`, from
`/root/cerulea-frozen-481207b` with target `/root/cerulea-clean-target`, three
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

#### Format

The frozen commit carries four format diffs in the bridge
pallet: `src/live_evidence.rs` at lines 277, 408 and 478, and
`src/networks/polkadot.rs` at line 281. The following commit `2edda44`
changes those two files alone. `git diff -w 481207b 2edda44` shows line
breaks, and the whitespace-stripped content differs in one token, a trailing
comma rustfmt drops when it joins the call at `live_evidence.rs` line 489.
`cargo fmt --all -- --check` on a clean export of `2edda44` exits 0. The binary
under test and every test count above come from `481207b`.

#### Lint

Every one of the sixteen errors sits in test code.
`cerulea-runtime/tests/runtime_api_tests.rs` holds thirteen `assert!(true)`
lines. The library test target of `cerulea-consensus` holds three: a `push`
straight after creation at `engines/commit/collect.rs` line 422, inside its
test module, and a cast to the same type at `tests/metrics_test.rs` line 170.

#### Workspace tests

Three tests failed.

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
committed before the freeze. The interface test meets runtime `2960e62`, by which a
collection root answers nothing for an undeclared collection. Each failure is
written as measured. Neither file is a coverage-only test: the interface test
dates from P2-052 and the runtime tests from P2-051 and P1-046.

After the freeze, chain `fbf6c17` corrected all three tests, a change to test
files alone. All three pass in the run on `4a79d72` above. The counts in this
subsection stand for `481207b`.

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

### The run on the frozen node 481207b

**Network verification passed. Safety held in all six fault exercises that
ran. Liveness held in all six. The clock exercise was blocked by tooling, so
it carries no verdict.**

The node binary hashes `031397ac9a9a0296...`, built from a clean export of
`481207b`, and the multi-host exercise confirmed that hash inside the image
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
   Liveness returned inside the bound. The placement or weight of validators
   on one machine stands as an open decision.
2. **The clock exercise has never run.** Its time shim starts in a separate
   container and aborts there. No run on any node has measured a clock
   offset.
3. **The standing records read no custody activity on the network.** Over the
   verification window every validator reads `custodyAnswered` 0,
   `custodyMissed` 0 and a custody share of 0. Validator eight read the same
   before its loss and after its restart, with `custodyResumed` false.
   Criterion 4 proves custody in the acceptance suite. The nine validator
   network shows no custody round in these records.

- Evidence: `test-results/multihost/frozen-node-481207b/network-verification.json`,
  `exercise-*.json`, `fault-exercises.log`, `run-all.log`, `node-logs/`, and
  `test-results/multihost/index.md`; test-results `b200439`

### Earlier runs

These runs led to the fixes the frozen node carries. They are history and
carry no verdict for the frozen node.

**Node ea7cf58**, the block import fix. Safety broke in the delay exercise at
height 1440, where validators five and eight committed a block apart from the
other seven, and validator six committed blocks 3277 to 3280 apart from the
other eight during the clock exercise. Both came from second votes in one
round, which later nodes refuse. Records:
`test-results/multihost/first-node-ea7cf58/`.

**Node 2a18668**, vote lock, round timeout and a separate draw key type.
Safety held in every exercise. Liveness returned after the partition of three,
the node loss and a repeated delay. Liveness failed twice: finality stalled at
1400 after the first delay and at 280 after the loss, and never returned. The
validators locked on one block and voted for it every round while the gossip
validator refused those votes once the chain entered a later epoch. The clock
exercise was blocked by tooling. Records: `test-results/multihost/second-node-2a18668/`.

**Node 7bfbabc**, the first liveness fix. Nothing committed on nine
validators. The gossip validator refused every incoming commit vote as a
runtime interface error from the first block, so each validator counted its
own vote alone. No exercise ran. Records:
`test-results/multihost/third-node-7bfbabc/`.

**Node 9604635**, finality from genesis. All nine voted and each committed
block gathered 88.88 to 100 per cent of the weight. Safety held in the six
exercises that ran. Liveness returned after the partition of three (90
seconds), the node loss (30 seconds) and the loss (75 seconds), and finality
advanced through the delay. After the delay heal the validators lost most of
their peers and dropped and rejoined them dozens of times a minute.
**Liveness then failed:** finality stopped at 930 at 08:21:43 UTC on seven
validators, two reached 940, and none returned. The delay held for one epoch
and the partition of four ran on the stalled network. The clock exercise was blocked by tooling. P1-022 at `0401df6`
traced the stall to the commit vote gossip store, which expired nothing and
resent every vote since genesis every 750 ms. Records:
`test-results/multihost/fourth-node-9604635/`.

## Open items that bear on acceptance

1. **The reserved platform scope belongs to no registered collection.** The
   five platform duties of the operator resolve to `PermissionDenied`
   (criterion 14). Open since 19 September.
2. **The faucet subcommand of the node is a stand-in.** On `481207b`,
   `cerulea-node/src/command.rs` line 85 prints that it would send and
   performs no transaction. The no-placeholder rule covers it.
3. **Two pallet defect groups stay open at the freeze.** Group (h), the
   migration from storage version 0 to 1 installs a governance record whose
   ranges and shares read zero, carried by one ignored test. Group (m), the
   event compatibility check reads an empty sample list and checks nothing.
   Both remain open items.
4. **The chain security evidence files are dated 19 September.** Fresh test
   evidence for the four chain controls sits in this record (criterion 15),
   and `chain/security/evidence` waits for its next collection.
5. **No Cerulea APIs coverage figure exists for the code under test.** The
   figure of 90.31 per cent stands for `1920324`.

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
    bash scripts/term-check.sh <repository>
