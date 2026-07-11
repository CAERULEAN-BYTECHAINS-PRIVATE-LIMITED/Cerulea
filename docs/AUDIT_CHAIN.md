# Cerulea Chain Deep Audit

Audit date: 2026-07-11. Scope: the Cerulea chain codebase, roughly 60,000 lines of
Rust across `cerulea-node`, `cerulea-runtime`, the custom consensus crate under
`cerulea-node/src/cerulea-consensus`, and the four pallets `dcf`, `poi`, `pos`,
`dvf`. This document is a read-only analysis. No code was changed to produce it.

A note on naming. Per the repository guardrails, this document uses DCF terminology
for the consensus. Where evidence requires citing real code, the audit references
the actual identifiers and dependency-crate paths that appear in the source (for
example `sc_consensus`, `sp_io::crypto`, `frame_system`, `Perbill`). Those are
internal dependency imports, which the guardrails permit. Where `CONSENSUS_BUGS.md`
refers to the base client finality by a framework name, this document calls it "the
client-layer finality" or "the base-layer finalizer".

Verdict in one line: the deterministic-authoring and finality-coordination defects
described in `CONSENSUS_BUGS.md` have largely been fixed in the current tree, but
two structural weaknesses remain that are more serious than most of the original
bug list: block authorship is unauthenticated (no signature seal, no local-author
gate), and the Proof of Inference score never reaches chain state, so the advertised
PoS 65 / PoI 35 weighting is effectively 100 / 0 today.

---

## 1. Module map

### 1.1 `cerulea-node` (node binary, ~5,000 lines)

The node binary. It builds the client, transaction pool, keystore, RPC surface, and
wires up every background service. Longest-chain fork rule; 6-second target block
time.

| File | Role |
|------|------|
| `src/main.rs` | Entry point. Parses CLI, optionally starts the lifecycle tracer, configures logging, dispatches to `command::run`. |
| `src/cli.rs` | clap definitions: standard run flags plus DCF flags (`--cbc-mode`, `--enable-cbc-extensions`, RPC rate-limit knobs, lifecycle-trace flags) and custom subcommands (`Faucet`, `Info`, `Health`, `ForkCheck`, `QueryAuthors`, `Benchmark`). |
| `src/command.rs` | CLI trait impl and chain-spec loader (`dev`/`local`/`multi_validator`/`high_stake`). `CheckBlock`/`ImportBlocks`/`ExportBlocks`/`ExportState`/`Revert` are rejected as "not supported in DCF-only mode" (`command.rs:79-85`). `Faucet` is a stub. |
| `src/service.rs` | The core wiring. `new_partial` builds client/pool/import queue; `new_full` spawns the DVF gossip engine (`/cbc/dvf/1`), the vote creator / aggregator / pruning services, offchain workers, the DCF block-production engine, the block tracker, and the `dvf-finality-sync` task (`service.rs:796-1014`). |
| `src/rpc.rs` | Custom JSON-RPC (rate-limited): `pos_*`, `poi_*`, `dcf_*`, `cerulea_*`, `dvf_*`, `fork_*` namespaces. |
| `src/chain_spec.rs` | Genesis builders; token symbol CBC, 12 decimals, protocol id `cbc`. |
| `src/fork_detection.rs` | Standalone RPC client tool that compares local and peer heads to detect divergence; backs the `ForkCheck` subcommand and `fork_*` RPCs. |
| `src/block_tracker.rs` | Per-validator authored/missed block and participation tracking with underperformance alerts. |
| `src/logging.rs` | Colorized, deduplicated log formatter and `RUST_LOG` setup. |
| `src/benchmarking.rs` | Overhead benchmark builders. |

### 1.2 `cerulea-consensus` (custom consensus crate, ~9,900 lines incl. tests)

Declares engine ids DCF `b"cbcd"`, DVF `b"dvfd"`, and DVF key type `b"cdvf"`
(`lib.rs`). Two independent mechanisms live here: DCF block authoring, and DVF
vote-based finality.

| File | Role |
|------|------|
| `dcf.rs` | `DcfConsensus` block-production loop plus `RealBlockImport`. Author selection, block build/import, timing. |
| `proposer_factory.rs` | Builds real blocks: applies inherents and pool transactions, computes state/extrinsics roots, injects the author digest. |
| `import_queue.rs` | `DcfImportQueue`: block `Verifier` and `BlockImport` that validate the block author against the on-chain schedule. |
| `vote_creator.rs` | `VoteCreatorService`: on each imported checkpoint block, if the node is an active validator, signs and gossips a DVF vote. |
| `vote_aggregator.rs` | `VoteAggregatorService` (accumulates votes, finalizes on threshold, submits the on-chain justification) and `VotePoolPruningService`. |
| `justification_builder.rs` | `JustificationBuilder`: selects threshold-reaching votes, builds and verifies a `DvfJustification` (1 MB cap). |
| `dvf_gossip.rs` | Gossip protocol `/cbc/dvf/1`: `DvfVoteMessage`, `DvfVotePool`, `DvfGossipValidator`. |
| `epoch_manager.rs` | Epoch-transition and validator-set helpers over the DCF runtime API. |
| `validator_set.rs` | `ValidatorSetManager`: registration/activation/scoring bridge to the DCF pallet. |
| `finality.rs` | `FinalityEngine` (confirmation counting) and `FinalityNotifier` (subscription channel used by the pruning service). |
| `inherent_providers.rs` | Timestamp inherent provider. |
| `author_selection.rs`, `types.rs`, `metrics.rs`, `dvf_config_validator.rs`, `error.rs`, `lifecycle_tracer*`, `mock.rs` | Author-selection helper, shared types, Prometheus metrics, startup config validation, error types, tracing subsystem, test mock. |

There is no `dvf_block_import.rs` file. `CONSENSUS_BUGS.md` (Bugs 5 and 11) refers
to it, but in the current tree the block-import target is the client itself
(`service.rs`), `FinalityNotifier` lives in `finality.rs`, and pruning lives in
`vote_aggregator.rs`.

### 1.3 `cerulea-runtime` (~3,300 lines)

`construct_runtime` is the `#[frame_support::runtime] mod runtime` block at
`src/lib.rs:183-230`. Pallets and indices: System=0, Timestamp=1, Balances=2,
TransactionPayment=3, Sudo=4, `Poi`=6, `Pos`=7, `Dcf`=8, `Todo`=9, `Dvf`=10 (index
5 is unused). `src/configs/mod.rs` holds all `impl Config` blocks; `src/apis.rs`
exposes the runtime APIs, including four custom ones: `PosApi`, `PoiApi`, `DcfApi`
(~50 methods, by far the largest), and `DvfApi`.

Cross-pallet wiring (`configs/mod.rs`): PoI depends on Pos and Dcf; Pos uses Dvf as
its `ValidatorHandler`; Dcf uses Dvf as `WeightFreezer`, `DvfFinalizedBlockProvider`,
and `ValidatorRegistry`. In other words DVF is the authoritative validator registry
and the primary finality authority, and DCF's on-chain finalized marker is capped at
the DVF-finalized block.

### 1.4 The four pallets

**`pallet-cerulea-dcf` (~14,000 lines).** The Dynamic Consensus Framework pallet:
validator scoring (weighted PoS + PoI), epoch management, deterministic author
sequences, block-authorship tracking, trust scores, on-chain governance
(slash/reward/eject proposals), misbehavior reporting, and progressive on-chain
finality markers. `Config` at `lib.rs:1740` (supertrait on `pos::Config +
poi::Config`) with ~80 `Get<..>` parameters. Hooks at `lib.rs:9170`: `on_initialize`
(`9260`) runs the per-block epoch/score/finality logic; `on_finalize` (`9321`);
`on_runtime_upgrade` (`9185`). No offchain worker in this pallet.

**`pallet-cerulea-poi` (745 lines).** Proof of Inference: validators submit AI
inference results with confidence, may challenge each other's results within a
window, and earn boosts. The `offchain_worker` (`poi/lib.rs:300`) computes scores
each interval. Calls: `submit_inference`, `challenge_inference`, `simulate_inference`,
`apply_offchain_poi_scores`, `update_validator_inference_score`.

**`pallet-cerulea-pos` (1,037 lines).** Proof of Stake and economics: validator
registration, bond/unbond via `ReservableCurrency`, scoring, slashing and rewards
with per-epoch caps, active-set management. `on_initialize` is currently minimal.

**`pallet-cerulea-dvf` (1,565 lines).** Dynamic Vote-based Finality: stake-and-score
weighted voting on checkpoint blocks, justification verification and finalization.
It also owns the validator lifecycle (join/leave) for the whole runtime. Key storage:
`EpochVotingWeight`, `TotalVotingWeight`, `CurrentRound`, `FinalizedBlockNumber`,
`FinalizedBlockHash`, `VoteRecords`, `VoteTallies`, `ValidatorSetId`. Calls:
`submit_dvf_vote`, `submit_justification` (unsigned), `join_validator_set`,
`leave_validator_set`, and batch variants. `validate_unsigned` at `dvf/lib.rs:1143`.

**`pallet-todo` (249 lines).** A generic on-chain to-do list (CRUD keyed by account),
wired at runtime index 9. Unrelated to consensus, apparently a demo pallet. It is a
live, permissionless extrinsic surface with generous bounds (500 todos/account, 1 KB
descriptions); worth flagging that it ships in the runtime.

---

## 2. The DCF consensus path, end to end

### 2.1 Scoring (Layer 0)

`final_score = (stake_score * pos_weight + inference_score * poi_weight) /
PercentagePrecision`, computed in `update_final_score`
(`dcf/lib.rs:6899-6957`). `stake_score` comes from the PoS pallet; `inference_score`
from the PoI pallet.

Configured weighting is not what the documentation states. `configs/mod.rs:76-77`
sets `PosWeight = 6000` and `PoiWeight = 4000` and wires them as the pallet defaults
(`configs/mod.rs:246-247`); `PercentagePrecision = 10000`
(`configs/mod.rs:204`). That is a 60/40 split, not the 65/35 named in `CLAUDE.md` and
`CONSENSUS_ARCHITECTURE.md`. A second, conflicting pair of constants exists at
`lib.rs:261-262` (`DefaultPosWeight = 10000`, `DefaultPoiWeight = 0`, commented
"100% PoS until PoI integration complete"); these appear unused by the active Config
impl. Regardless of which constants are live, the PoI input is 0 in practice (see
Bug 4 in Section 3), so the effective weighting today is 100% PoS.

### 2.2 Epoch boundary: seed, snapshot, author sequence

At each epoch boundary (`block_number % blocks_per_epoch == 0`, checked in
`on_initialize`), `handle_epoch_transition` (`dcf/lib.rs:7831`) runs deterministically
as part of block execution:

1. Five blocks before the boundary, `on_initialize` freezes validator scores into
   `EpochScoreSnapshot` (`dcf/lib.rs:7295-7309`). This fixes the weights all nodes
   will use for the next sequence at a single agreed block.
2. `generate_deterministic_randomness` (`dcf/lib.rs:8122-8150`) derives the epoch
   seed from `block_hash(block_number - 1)`, the parent block's hash, which is
   identical on every node that has imported the boundary block.
3. `generate_deterministic_author_sequence` (`dcf/lib.rs:8169-8228`) reads the frozen
   snapshot (`8188`), sorts validators by account id (`8208`), then fills a
   per-offset sequence with `select_weighted_author` driven by an advancing
   deterministic RNG. Higher weight yields more slots. The sequence is stored in
   `EpochAuthorSequences[epoch]`.

`select_weighted_author` (`dcf/lib.rs:8700`) walks the account-id-sorted, weighted
vector and picks by a cumulative-weight threshold against the RNG state.

### 2.3 Block authoring (Layer 1)

Each node runs `DcfConsensus::run` (`dcf.rs:125`) on a ~1-second loop. When the
6-second block timer elapses (`should_produce_block`, `dcf.rs:275`) it:

1. Reads active validators and calls `select_next_author_from_runtime`
   (`dcf.rs:455`), which returns `get_expected_author(block_number)`. That runtime
   method (`dcf/lib.rs:11117`) reads the precomputed `EpochAuthorSequences` first
   (`11130`) and falls back to live-score selection only if no sequence exists
   (`11147`, `get_expected_author_legacy` at `11155`).
2. If the runtime returns no author or errors, the loop skips the slot rather than
   electing locally (`dcf.rs:471-487`).
3. Before producing, it re-reads `best_number` and skips if a peer already produced
   the slot (the "double block production guard", `dcf.rs:204-240`).
4. `create_block_proposal` (`dcf.rs:582`) builds the block via `proposer_factory`,
   injecting a `PreRuntime` digest containing the author's public-key bytes
   (`dcf.rs:590-593`), then imports it.

On import, both `RealBlockImport::import_block` (`dcf.rs:1010`) and the
`DcfImportQueue` verifier (`import_queue.rs:334`) extract the author from the digest
and hard-reject the block if it does not match `get_expected_author`
(`dcf.rs:1044-1053`, `import_queue.rs:361-371`).

Important structural gap (analyzed in Section 4): the digest carries the author's
public key but no signature, and the production loop never checks that the local
node is the scheduled author. Authorship is asserted, not proven.

### 2.4 DCF confirmation and the finality cap

On each block, `on_initialize` advances the on-chain finalized marker through
`update_finality_markers` (`dcf/lib.rs:7684`). Crucially this marker is capped at
the DVF-finalized block: `capped_block = new_finalized_block.min(dvf_finalized)`
(`dcf/lib.rs:7694-7695`), and it is skipped entirely while DVF has finalized nothing
(`7697-7704`). This is an on-chain storage value only; it does not itself finalize
blocks at the client layer.

### 2.5 DVF finality (Layer 2): vote aggregation and justification

Checkpoint blocks are those where `block_number % FinalityCheckpointInterval == 0`
(interval 10). The path:

1. **Vote creation.** On importing a checkpoint block, `VoteCreatorService`
   (`vote_creator.rs:105`) confirms the block is a checkpoint, is not already
   finalized, and that this node is an active validator, then builds a
   `DvfVoteMessage`, signs it with the `cdvf` ed25519 key (`vote_creator.rs:256`),
   inserts it into the local pool, and gossips it (`vote_creator.rs:286-327`).
2. **Gossip validation.** `DvfGossipValidator::validate` (`dvf_gossip.rs:370`)
   deduplicates by message hash, verifies the ed25519 signature
   (`dvf_gossip.rs:267`), and checks the sender is an active validator and the epoch
   and validator-set id match (with a one-block grace window). Valid votes go into
   `DvfVotePool`, which enforces one vote per `(round, validator)`
   (`dvf_gossip.rs:68-81`).
3. **Aggregation.** `VoteAggregatorService::check_vote_accumulation`
   (`vote_aggregator.rs:165`) computes, per candidate block,
   `accumulated_weight = sum of voter weights` and compares against
   `threshold = FinalityThreshold(Perbill) * total_weight`
   (`vote_aggregator.rs:220-221`). Candidates already triggered locally or at/below
   the DVF-finalized head are filtered out first (`vote_aggregator.rs:181-197`).
4. **Justification.** On reaching threshold it calls `JustificationBuilder`
   (`justification_builder.rs:199`), which selects the minimum weighted vote set,
   checks for duplicate validators, re-verifies the accumulated weight against the
   threshold, and enforces size limits (1 MB, and vote count <= validator count).
5. **Finalization.** The aggregator finalizes the block at the client layer via
   `apply_finality` (`vote_aggregator.rs:425-427`) and then submits an on-chain
   `Dvf::submit_justification` extrinsic (`vote_aggregator.rs:466-507`) so the DVF
   pallet advances `FinalizedBlockNumber`. The pallet re-verifies everything in
   `verify_and_finalize_justification` (`dvf/lib.rs:865-959`) before calling
   `finalize_block`, which sets `FinalizedBlockNumber`/`FinalizedBlockHash` and
   increments `CurrentRound` (`dvf/lib.rs:961-978`).
6. **Sync.** The `dvf-finality-sync` task (`service.rs:796-1014`) periodically reads
   the DVF-finalized block and finalizes all client blocks up to it (`928-976`),
   including a startup catch-up (`814-867`).

Blocks between checkpoints are finalized implicitly as ancestors of the next
finalized checkpoint. `CurrentRound` advances only on a successful finalization
(`dvf/lib.rs:971`), which has a liveness consequence analyzed in Section 4.4.

---

## 3. Cross-reference against `CONSENSUS_BUGS.md`

Status summary. Every bug below is followed by file and line evidence.

| Bug | Component | Status |
|-----|-----------|--------|
| 1 | `generate_deterministic_randomness` | Fixed |
| 2 | `generate_deterministic_author_sequence` weights | Fixed (score snapshot) |
| 3 | Score-sorted `ActiveValidators` written to storage | Fixed |
| 4 | PoI score always 0, tx flooding | Flooding fixed; core still present |
| 5 | DVF finalized head stuck at 0 | Addressed; mechanism now exists |
| 6 | DVF weight overflow at startup | Fixed |
| 7 | Three finality systems uncoordinated | Fixed (DVF is authority, DCF capped) |
| 8 | Votes only every 10th block | By design; not a defect |
| 9 | Double block production / fork on divergence | Root cause fixed; residual (Section 4) |
| 10 | Epoch transition fired from the consensus loop | Fixed |
| 11 | Aggressive vote-pool flushing | Fixed |

### Bug 1: Non-deterministic randomness seed. FIXED.

`generate_deterministic_randomness` now anchors entropy to the parent block hash:
`parent_block_number = block_number.saturating_sub(1)` then
`frame_system::Pallet::<T>::block_hash(parent_block_number)`
(`dcf/lib.rs:8142-8146`). The parent block is fully executed and agreed before
`on_initialize(boundary)` runs, so the seed is identical on every node that reaches
the boundary. This is exactly the fix proposed in `CONSENSUS_BUG_SOLUTIONS.md`
Solution 1. The live-block-number read named in the bug report is gone.

### Bug 2: Live validator scores used as author-sequence weights. FIXED.

`generate_deterministic_author_sequence` no longer reads live `ValidatorStates` at
the boundary. It reads `EpochScoreSnapshot::<T>::get(_epoch)`
(`dcf/lib.rs:8188-8205`), a snapshot frozen five blocks before the boundary in
`on_initialize` (`dcf/lib.rs:7283-7316`, insert at `7309`). Because the snapshot is
taken at a single fixed block and read from on-chain state, all nodes on the same
canonical chain compute identical weights. Residual notes: the snapshot still derives
from `ValidatorStates` (so it inherits whatever score-timing determinism the chain
has, which is fine given one canonical chain), and a missing snapshot falls back to
equal weights (`8201-8204`), which is safe because it is also deterministic.

### Bug 3: Score-based sort of `ActiveValidators` written to storage. FIXED.

Three code sites that the bug report identified as conflicting now all use account-id
ordering for anything written to storage:

- `handle_epoch_transition` sorts `ActiveValidators` by account id and writes that:
  `active_validators.sort_by(|a, b| a.cmp(b))` (`dcf/lib.rs:7847-7852`), with an
  explicit comment warning against score sorting.
- `apply_pending_validator_actions` sorts by account id before writing
  (`dcf/lib.rs:11726-11729`).
- The participation update no longer writes a score-sorted order to storage
  (`dcf/lib.rs:7278-7281`, comment only).
- The verifier reads the precomputed sequence `EpochAuthorSequences`
  (`dcf/lib.rs:11130`), not the raw storage order.

`sort_validators_by_score` still exists (`dcf/lib.rs:11630`) but has no callers in
the pallet, so it is now dead code. Recommend deletion to prevent reintroduction.

### Bug 4: PoI score computed per block, always zero. FLOODING FIXED; CORE STILL PRESENT.

The offchain worker was moved out of the DCF pallet into the PoI pallet
(`poi/lib.rs:300`, comment "moved from DCF" at `poi/lib.rs:473`). It now runs only
every `OffchainWorkerInterval` blocks (interval 5, `poi/lib.rs:301`,
`configs/mod.rs:205`) and skips submission when the score is unchanged
(`poi/lib.rs:729-737`). The score itself is now a nonzero deterministic value
(`simulate_inference_computation` / `compute_poi_score`, `poi/lib.rs:612-632`). That
resolves the "3 unsigned txs every block" secondary effect.

The core symptom is still present, by a different mechanism. `submit_poi_score_update`
only writes to node-local offchain storage (`poi/lib.rs:638-669`); it never submits
an on-chain transaction. The only path that would move a computed score into on-chain
`InferenceResults` is the signed `apply_offchain_poi_scores` extrinsic
(`poi/lib.rs:476-510`), and that extrinsic is never invoked anywhere in the node or
runtime (a repository-wide search finds only a comment reference in
`configs/mod.rs:207`). Consequently `get_inference_score` returns 0 whenever a
validator has not manually called `submit_inference` (`poi/lib.rs:540-546`), which is
the default. PoI therefore contributes nothing to `final_score` on chain, matching
the "score=0 on every block" observation in the bug report. Combined with the weight
finding in Section 2.1, the PoI dimension of consensus is non-functional today.

As a correctness aside, `apply_offchain_poi_scores` reads node-local offchain storage
from within an on-chain call (`poi/lib.rs:495`). If it were ever wired in, it would
make on-chain state depend on the authoring node's local storage, which is
non-deterministic; that extrinsic needs redesign (for example an unsigned inherent
with an included, verifiable payload) before use.

### Bug 5: DVF finalized head permanently stuck at 0. ADDRESSED.

A mechanism to advance the DVF-finalized head now exists. On reaching threshold the
aggregator both finalizes at the client layer and submits `Dvf::submit_justification`
(`vote_aggregator.rs:466-507`); the pallet's `finalize_block` sets
`FinalizedBlockNumber` (`dvf/lib.rs:967`). The `dvf-finality-sync` task reads that
value and closes the client-side gap (`service.rs:889-976`). This is a real change
from the reported behavior. Residual risk: the head advances only when the
`submit_justification` extrinsic is actually included on chain, and only at
checkpoint granularity, so transient windows where the client head leads the DVF head
are expected (the code treats this as normal, `service.rs:988-994`). Given the near
absence of automated tests for this path (Section 5), the audit cannot confirm from
code alone that the head advances reliably under load; this needs an integration test.

### Bug 6: DVF vote-weight overflow at startup. FIXED.

Weights are now normalized to a fixed scale at freeze time.
`freeze_epoch_weights` computes `normalized_weight = stake * VOTE_WEIGHT_SCALE /
total_stake` with `VOTE_WEIGHT_SCALE = 32_000` (`dvf/lib.rs:844-857`) and sums them
into `TotalVotingWeight` (`860`). Genesis calls `freeze_epoch_weights(0, ...)` so
`EpochVotingWeight` is populated at block 0 (`dvf/lib.rs:406-408`), eliminating the
uninitialized-startup window. Because every weight is bounded by construction and the
threshold is `Perbill * total_weight` (`vote_aggregator.rs:221`,
`dvf/lib.rs:938`), the `24000000000000240000`-style overflow cannot occur.

### Bug 7: Three finality systems running without coordination. FIXED.

There is now a single client-layer finalizer and a clear hierarchy. The only calls to
`finalize_block`/`apply_finality` in the node are the DVF aggregator
(`vote_aggregator.rs:426`) and the `dvf-finality-sync` task
(`service.rs:830`, `service.rs:941`). The DCF "progressive finalization" is now only
an on-chain marker (`LastFinalizedBlock`) and is explicitly capped at the
DVF-finalized block (`dcf/lib.rs:7694-7695`). So DVF is the sole finality authority;
DCF cannot advance the hard-finalized head past DVF; the base-layer client finality
is driven exclusively from DVF. The "client head ahead of DVF head" message that
recurred in the logs is now an expected transient (client finalizes on threshold
before the justification extrinsic lands on chain), not a symptom of three
uncoordinated systems.

### Bug 8: DVF votes only for every 10th block. BY DESIGN, NOT A DEFECT.

Sparse checkpoint voting is intentional (`CONSENSUS_ARCHITECTURE.md` Level 2):
only checkpoint blocks receive votes, and intermediate blocks are finalized as
ancestors. The vote creator explicitly gates on `is_checkpoint_block`
(`vote_creator.rs:111`, `147-166`). The original complaint (the aggregator finding
zero candidates every second between checkpoints) is now mitigated by the fast-path
early return when there are no candidates (`vote_aggregator.rs:199-203`) and by the
pruning fix (Bug 11). The residual concern is not the sparsity itself but the
round-keyed voting interaction under fault, analyzed in Section 4.4.

### Bug 9: Double block production / deterministic disagreement. ROOT CAUSE FIXED; RESIDUAL.

The bug's root cause was divergent author sequences (Bugs 1-3), which are fixed, so
the sequence is now identical across nodes and the deterministic-disagreement fork
described in the report should not occur. A best-effort guard was also added: the
loop re-reads `best_number` immediately before producing and skips if a peer already
produced the slot (`dcf.rs:204-240`). However, the guard only checks block height, not
author identity, and the loop produces on behalf of the scheduled author without
verifying the local node is that author and without a signature seal. Under simultaneous
production (all nodes' 6-second timers fire within the same ~1-second window) multiple
nodes can still build distinct block-N candidates for the same scheduled author, with
different timestamps and therefore different hashes, producing transient same-height
forks resolved only by the longest-chain rule. See Section 4.1 and 4.2.

### Bug 10: Epoch transition triggered from the consensus loop. FIXED.

The consensus loop no longer triggers epoch transitions. The site the bug report
identified now carries only the comment "Epoch transitions are handled exclusively by
on_initialize; do not trigger from the consensus loop" (`dcf.rs:249`). The single
remaining trigger is deterministic, in `on_initialize`
(`dcf/lib.rs:7232` boundary check calling `handle_epoch_transition` via the per-block
hook). The duplicate-transition symptom is therefore removed.

### Bug 11: Aggressive vote-pool flushing after every finalization. FIXED.

Two changes address both sub-issues. First, `prune_after_finalization` now runs only
at checkpoint boundaries: it returns early unless
`finalized_block_number % checkpoint_interval == 0`
(`vote_aggregator.rs:686-692`). Second, finalized-block pruning retains the votes for
the finalized checkpoint block itself; `prune_by_finalized_block` removes only votes
strictly below the checkpoint (`dvf_gossip.rs:140-154`,
`vote_aggregator.rs:722-731`). Separately, `check_validator_set_changes` no longer
clears the pool on epoch transitions (`vote_aggregator.rs:142-156`). Memory is still
bounded by round-age pruning and a 10,000-entry cap (`vote_aggregator.rs:713-741`).

---

## 4. Safety analysis of the finality gadget

### 4.1 Unauthenticated block authorship (most serious finding)

Block authorship is asserted by an unsigned `PreRuntime` digest containing the
author's public-key bytes (`dcf.rs:590-593`) and validated only by comparing that
public key against the on-chain schedule (`import_queue.rs:361-371`,
`dcf.rs:1044-1053`). There is no signature seal: the block-production path uses no
keystore, and the "Block N sealed successfully" line is only a log message
(`proposer_factory.rs:476-486`). Consequences:

- **Forgeable authorship.** Any node, including a single malicious peer, can build a
  block whose digest names the current scheduled author without possessing that
  validator's key. The verifier accepts it. Authorship confers no cryptographic
  accountability, so misbehavior reporting keyed on authorship cannot be trusted.
- **No local-author gate.** `DcfConsensus` holds no validator key (`dcf.rs:60-89`)
  and `run` never checks that the local node is the scheduled author before producing
  (`dcf.rs:125-250`). Every validator node attempts to produce every block on behalf
  of the scheduled author. Correct single-proposer behavior is only approximated by
  the timing guard.

This is not in `CONSENSUS_BUGS.md` and is more fundamental than most of the listed
bugs. It should be treated as the top remediation item: add an ed25519 seal signed by
the scheduled author's key, verify the seal on import, and gate production on
`author == local_key`.

### 4.2 Can a validator equivocate?

Two senses.

- **Block-authoring equivocation.** Because blocks are unsigned (4.1), the scheduled
  author can trivially produce two different blocks for the same slot, and so can any
  other node impersonating them. There is no seal to detect it and no slashing for it.
  Fork choice (longest chain) resolves the tie, but there is no accountability.
- **Vote equivocation.** A validator voting for two different block hashes in the same
  round is prevented locally: the gossip pool and the on-chain `submit_dvf_vote` both
  enforce one vote per `(round, validator)` (`dvf_gossip.rs:68-81`,
  `dvf/lib.rs:531-534`), and the gossip validator logs a double-vote rejection
  (`dvf_gossip.rs:405-414`). However, the second vote is merely dropped, not recorded
  as slashable evidence, and the enforcement is per node: a Byzantine validator can
  gossip vote-for-A to one partition and vote-for-B to another, and each side keeps
  whichever it saw first. With an honest supermajority this does not split finality
  (honest votes still concentrate on one block), but there is no cryptographic
  equivocation proof and no penalty, so the security model relies on there being fewer
  than one third Byzantine weight rather than on punishment.

### 4.3 Can the chain fork under a network partition?

For finality: no unsafe fork. Finalization requires `FinalityThreshold * total_weight`
(default two thirds). A partition holding less than the threshold cannot finalize, and
`verify_and_finalize_justification` rejects justifications below threshold
(`dvf/lib.rs:940-943`). Two partitions cannot both finalize conflicting blocks unless
each holds at least the threshold, which is impossible for a two-thirds threshold with
disjoint validator sets. Safety is preserved at the cost of liveness (Section 4.4).

For block production (unfinalized head): yes, partitions produce divergent
best-chains, and even within a single partition the unauthenticated, all-nodes-produce
behavior (4.1) can create transient same-height forks. These are resolved by longest
chain once connectivity returns and are never finalized while below threshold, so they
do not threaten finality safety, but they do mean the visible head can reorg. One
narrower gap: `verify_and_finalize_justification` finalizes the block number and hash
from the justification (`dvf/lib.rs:945-950`) without checking that the hash is a
block this node actually has on its canonical chain, while the sync task finalizes the
node's local hash at that height (`service.rs:938-941`). With an honest supermajority
these agree; a supermajority signing a hash a node lacks would desynchronize the
pallet's `FinalizedBlockHash` from the client's finalized hash. This requires
Byzantine supermajority and is therefore lower priority, but the missing "is this hash
on my chain" check is worth adding.

### 4.4 What if more than one third of validators are offline?

Finality halts, safely. No checkpoint can reach the two-thirds threshold, so the
aggregator never triggers finalization and `FinalizedBlockNumber` stops advancing.
Block production continues (DCF is independent of votes), so the chain keeps growing an
unfinalized head; the DCF on-chain finalized marker is capped and simply stops
(`dcf/lib.rs:7694-7704`). The stall warning fires after ten rounds
(`vote_aggregator.rs:358-368`). This is the correct BFT trade-off (safety over
liveness).

One recovery wrinkle: `CurrentRound` advances only on successful finalization
(`dvf/lib.rs:970-971`), and votes are keyed by round. While finality is stalled the
round does not advance, so a validator that already voted in the stuck round cannot
cast a counted vote for a later checkpoint (the pool's per-`(round, validator)`
dedup rejects it, `dvf_gossip.rs:68-73`). Once enough validators return, finality can
resume on the stuck checkpoint if it is still the relevant candidate, but the design
couples "which block we are voting on" to "have we finalized the previous one," which
can make recovery after a long stall depend on ordering. This deserves an explicit
test and, ideally, decoupling round identity from finalization success (for example
key rounds by checkpoint height).

### 4.5 Unbounded loops or panics reachable from a network message

- **Gossip vote path: no panic, but an unbounded set.** `validate_core`
  (`dvf_gossip.rs:246-360`) decodes with bounded SCALE, verifies the signature, and
  returns `Err` on every failure; there is no `unwrap`/`expect`/`panic` on this path
  (the one `B::Hash::decode(...).unwrap_or_default()` at `dvf_gossip.rs:378` is
  infallible). However, `known_messages` is inserted into for every incoming message
  hash before validation (`dvf_gossip.rs:380-383`) and is never pruned. A peer sending
  an unbounded stream of distinct byte blobs (valid or not) grows this `HashSet`
  without bound: a memory-exhaustion DoS reachable from the network. It needs a size
  cap or age-based eviction.
- **Unsigned justification extrinsic: mispriced, unbounded work.**
  `submit_justification` is unsigned (`ensure_none`, `dvf/lib.rs:556`) and its
  `validate_unsigned` gate only checks that votes are non-empty and the block number
  exceeds the finalized head (`dvf/lib.rs:1147-1177`); it does not verify signatures,
  cap the vote count, check the checkpoint, or check the threshold before pool
  admission. Verification, including one ed25519 check per vote, happens during
  dispatch in a loop over `justification.votes` with no length bound
  (`dvf/lib.rs:893-935`), while the declared weight is a fixed
  `reads_writes(10, 10)` (`dvf/lib.rs:551`). A crafted unsigned justification with a
  very large vote vector (bounded only by extrinsic/block size) forces thousands of
  signature verifications for a grossly underpriced, feeless transaction: a
  block-production DoS. Add a `MaxVotes` bound checked in `validate_unsigned`, verify
  at least a cheap structural precondition before pool admission, and make the weight
  scale with the vote count.
- **Aggregator `.expect`.** `vote_aggregator.rs:464` uses
  `.expect("Failed to decode generic justification to concrete type")` on data it just
  encoded. Practically unreachable, but a decode failure would panic the aggregator
  task. Replace with graceful error handling. The `RwLock` `.unwrap()`s in the same
  file panic only on a poisoned lock (another thread already panicked); low risk but
  worth hardening.
- Other `unwrap`/`expect` occurrences in the crate are in test code, the lifecycle
  tracer (mutex locks), and inherent-provider tests, none reachable from a network
  message.

---

## 5. Test coverage gaps, ranked by risk

The pallets are heavily tested (the DCF pallet alone has ~25 test files including
`consensus_invariant_tests`, `deterministic_tests`, `property_tests`,
`dos_protection_tests`, and `finality_tests`; DVF has a 1,392-line `tests.rs`; PoI and
PoS have substantial suites). The node-side consensus crate is where coverage is thin,
and that is exactly where the safety-critical logic and the historical bugs live.

1. **DVF finality gadget (node side): essentially zero tests.** `vote_aggregator.rs`,
   `vote_creator.rs`, `dvf_gossip.rs`, `justification_builder.rs`, `finality.rs`, and
   `import_queue.rs` contain no test modules. The tests directory
   (`cerulea-consensus/src/tests/`) covers only `epoch_manager`,
   `header_parameter_order`, `metrics`, `proposer_integration`, `state_root_fix`,
   `task8_metrics`, and `validator_set`. The vote-to-threshold-to-justify-to-finalize
   path, gossip validation and dedup, double-vote handling, and justification
   verification are all unverified by automated tests. Highest risk: this is where
   Bugs 5, 6, 8, and 11 lived and where the fixes now depend on runtime behavior the
   audit cannot confirm from code alone.
2. **Block-author authentication.** There is no test that a block from a
   non-scheduled or key-less author is rejected, because there is currently no seal to
   test (Section 4.1). Add the seal, then test forgery rejection.
3. **Multi-node divergence and partition.** Validation to date has been manual log
   inspection of three-node runs (`node_output_*.log`, `TESTING_GUIDE.md`), not
   automated. There is no harness that asserts all nodes derive the same author
   sequence across an epoch boundary, that the head converges after a partition, or
   that finality halts and resumes cleanly under a greater-than-one-third outage.
4. **Unsigned-extrinsic abuse.** No test submits an oversized or malformed
   `submit_justification` to confirm it is bounded and correctly priced (Section 4.5).
5. **PoI end-to-end on chain.** No test asserts that a computed offchain PoI score
   actually reaches on-chain `InferenceResults` and changes `final_score`; the broken
   path in Bug 4 would have been caught by one.
6. **Finality-marker cap and round recovery.** No test exercises the DCF cap at the
   DVF head (`dcf/lib.rs:7694`) or the round-keyed recovery after a stall
   (Section 4.4).

---

## 6. Prioritized remediation list

Priority 1 (safety and correctness; do before any production use):

1. **Authenticate block authorship.** Add an ed25519 seal signed by the scheduled
   author's key, sign it in the production path using the keystore, verify it on
   import, and gate production on `author == local_key`. This closes forgeable
   authorship, the all-nodes-produce behavior, and the residual fork surface of Bug 9
   (Section 4.1, 4.2).
2. **Bound and reprice `submit_justification`.** Add a `MaxVotes` limit enforced in
   `validate_unsigned`, verify cheap structural preconditions before pool admission,
   and make the declared weight scale with the vote count
   (`dvf/lib.rs:551`, `1147-1177`, `893-935`) (Section 4.5).
3. **Cap or evict `known_messages` in the gossip validator**
   (`dvf_gossip.rs:380-383`) to remove the memory-DoS (Section 4.5).
4. **Make PoI reach chain state, or disable it honestly.** Either wire a
   deterministic, verifiable path from the offchain computation into on-chain
   `InferenceResults` (not the current local-storage-reading extrinsic), or explicitly
   set the PoI weight to 0 and stop advertising a 65/35 (or 60/40) split until the AI
   integration lands. Also reconcile the conflicting weight constants
   (`configs/mod.rs:76-77` vs `lib.rs:261-262`) against the documented split
   (Section 2.1, Bug 4).

Priority 2 (robustness and confidence):

5. **Add integration tests for the DVF finality gadget** covering vote aggregation,
   threshold finalization, justification verify/reject, double-vote handling, and the
   finality-sync loop (Section 5, item 1).
6. **Add a multi-node harness** asserting identical author sequences across epoch
   boundaries, head convergence after a partition, and safe halt/resume under a
   greater-than-one-third outage (Section 5, item 3).
7. **Decouple `CurrentRound` from finalization success** (key rounds by checkpoint
   height) to make recovery after a stall deterministic, and add a test for it
   (Section 4.4).
8. **Add an "is this hash on my canonical chain" check** to
   `verify_and_finalize_justification` before finalizing
   (`dvf/lib.rs:945-950`) (Section 4.3).
9. Replace the aggregator `.expect` with graceful handling
   (`vote_aggregator.rs:464`).

Priority 3 (hygiene):

10. Delete the dead `sort_validators_by_score` (`dcf/lib.rs:11630`) to prevent
    reintroduction of Bug 3.
11. Reconsider shipping `pallet-todo` in the production runtime, or at least document
    why a permissionless CRUD surface is present (Section 1.4).
12. Update `CONSENSUS_ARCHITECTURE.md` and `CONSENSUS_BUGS.md`: the former still
    states 65/35 and describes uncoordinated finality that no longer exists; the
    latter references a `dvf_block_import.rs` file that does not exist and a base
    client finality that has since been subordinated to DVF.

---

## Appendix: evidence index

- Randomness seed fix: `cerulea-pallets/pallet-cerulea-dcf/src/lib.rs:8122-8150`
- Score snapshot: `.../dcf/src/lib.rs:7283-7316`, `8169-8228`
- Account-id sort of active set: `.../dcf/src/lib.rs:7847-7852`, `11726-11729`
- Expected-author read of precomputed sequence: `.../dcf/src/lib.rs:11117-11148`
- DCF finality cap at DVF head: `.../dcf/src/lib.rs:7684-7729`
- PoI offchain worker (local-only write): `cerulea-pallets/pallet-cerulea-poi/src/lib.rs:300-305`, `638-745`
- PoI on-chain application extrinsic (never called): `.../poi/src/lib.rs:476-510`
- DVF weight normalization and genesis: `cerulea-pallets/pallet-cerulea-dvf/src/lib.rs:406-408`, `806-862`
- DVF unsigned justification and gate: `.../dvf/src/lib.rs:552-559`, `865-978`, `1143-1178`
- Block production loop and guard: `cerulea-node/src/cerulea-consensus/src/dcf.rs:125-250`
- Import author check (no seal): `.../dcf.rs:1010-1135`, `.../import_queue.rs:334-382`
- Vote creation/gossip/aggregation/justification: `.../vote_creator.rs`, `.../dvf_gossip.rs`, `.../vote_aggregator.rs`, `.../justification_builder.rs`
- Finality sync task: `cerulea-node/src/service.rs:796-1014`
- Configured weights: `cerulea-runtime/src/configs/mod.rs:76-77,204,246-247`; `cerulea-runtime/src/lib.rs:261-262`
