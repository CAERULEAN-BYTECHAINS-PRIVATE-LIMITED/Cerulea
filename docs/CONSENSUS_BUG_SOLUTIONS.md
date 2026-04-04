# CBC Chain — Consensus Bug Solutions


## Overview

---

## Solution 1 — Non-Deterministic Randomness Seed

**Bug Reference:** Bug 1  
**Severity:** Critical — causes permanent chain fork at every epoch boundary  
**Component:** `pallet-cbc-dcf/src/lib.rs` → `generate_deterministic_randomness()`

### Root Cause

`frame_system::block_hash(block_number)` is called with the epoch boundary
block number (e.g. 1100). However, inside `on_initialize(1100)`, block 1100
is currently being built — its hash does not exist yet. Substrate only stores
hashes of completed past blocks. The call returns zero/empty bytes, providing
no real entropy. The original intent was to anchor the seed to an agreed
on-chain value, but the wrong block was referenced.

### Invariant to Restore

> The randomness seed for epoch N+1 must be identical on every node that has
> reached the epoch boundary block. It must be derived exclusively from
> on-chain state that is finalized and agreed upon before the boundary block
> is executed.

### Solution

Use `block_hash(boundary_block - 1)` — the parent block's hash. The parent
block is fully executed and finalized before `on_initialize(boundary_block)`
runs. Its hash is identical on every node that has imported it. A node that
has not yet reached the boundary block has not executed `on_initialize` for
it and therefore has not generated any sequence — it will generate the correct
sequence when it imports the boundary block.

```
Pattern (correct):
  "Current block IS epoch boundary (1100)"
  → use block_hash(1099)  ← finalized, agreed, identical on all nodes
  → NOT block_hash(1100)  ← does not exist during on_initialize(1100)

Guarantee:
  Bob at height 1099:
    → on_initialize(1100) has NOT run
    → no sequence generated yet
    → waits until block 1100 arrives from network

  Bob receives block 1100:
    → imports it, executes on_initialize(1100)
    → block_hash(1099) = same hash as Alice's block_hash(1099)
    → identical seed → identical author sequence
```

### Code Change

**File:** `cbc-chain/cbc-pallets/pallet-cbc-dcf/src/lib.rs`  
**Function:** `generate_deterministic_randomness(block_number, epoch, randomness_salt, epoch_salt)`

```rust
// BEFORE (broken):
let transition_block_hash = frame_system::Pallet::<T>::block_hash(
    BlockNumberFor::<T>::from(block_number)
);
input.extend_from_slice(transition_block_hash.as_ref());

// AFTER (correct):
// block_number is the epoch boundary block (e.g. 1100).
// Its hash does not exist yet during on_initialize(1100).
// Use the parent block (1099) — finalized, identical on all nodes.
let parent_block_number = block_number.saturating_sub(1);
let parent_block_hash = frame_system::Pallet::<T>::block_hash(
    BlockNumberFor::<T>::from(parent_block_number)
);
input.extend_from_slice(parent_block_hash.as_ref());
```

### Verification Criteria

- Epoch transitions at blocks 100, 200, 300... produce no `author mismatch` errors
- `generate_deterministic_randomness` called with `block_number=100` uses `block_hash(99)`
- All three nodes log identical `randomness_seed` values in `DeterministicEpochProcessed` event

---

## Solution 2 — Live Validator Scores as Author Sequence Weights

**Bug Reference:** Bug 2  
**Severity:** Critical — causes fork inside epoch when offchain worker timing differs  
**Component:** `pallet-cbc-dcf/src/lib.rs` → `generate_deterministic_author_sequence()`

### Root Cause

The author sequence uses `ValidatorStates.final_score` as weights at the
moment of epoch transition. These scores are updated by the offchain worker
via unsigned transactions that land in different blocks on different nodes.
At the epoch boundary, different nodes have different score values, producing
different weighted selections for the same block offsets.

### Invariant to Restore

> The weights used to generate the epoch N+1 author sequence must be identical
> on every node. They must come from on-chain state that is finalized and
> agreed upon before the epoch boundary executes.

### Solution

Introduce a **score snapshot** written to on-chain storage at a fixed block
before the epoch boundary. Because `on_initialize` runs as part of block
execution from the same state root on all nodes, the snapshot written at
block `(N*100 - 5)` is identical everywhere. The epoch transition at block
`N*100` reads this snapshot instead of live `ValidatorStates`.

```
Timeline for epoch 0 → 1 transition:

Block 95: on_initialize(95) runs on ALL nodes from same state root
  → reads ValidatorStates (same on all nodes at this block)
  → writes EpochScoreSnapshot[1] = [(Alice, 50), (Bob, 30), (Charlie, 20)]
  → this is now agreed on-chain state

Block 100: epoch transition
  → reads EpochScoreSnapshot[1] — same on all nodes
  → generates author sequence with weights [Alice=50, Bob=30, Charlie=20]
  → Alice gets ~50% of slots, Bob ~30%, Charlie ~20%
  → identical sequence on all nodes

PoS+PoI scoring is fully preserved:
  Alice produced more blocks → higher score → more slots in epoch 1
  The scoring system works correctly, just read at a stable point
```

### New Storage Item Required

```rust
/// Snapshotted validator scores taken 5 blocks before each epoch boundary.
/// Key: next epoch number. Value: (AccountId, score) pairs.
#[pallet::storage]
pub type EpochScoreSnapshot<T: Config> = StorageMap<
    _, Blake2_128Concat, u32,
    BoundedVec<(T::AccountId, u64), ConstU32<1000>>,
    OptionQuery
>;
```

### Code Changes

**1. Snapshot trigger in `handle_regular_block_processing`:**

```rust
// 5 blocks before epoch boundary, freeze scores for next epoch
let epoch_config = Self::epoch_config();
let blocks_per_epoch = epoch_config.blocks_per_epoch;
let next_epoch = current_epoch.saturating_add(1);
let snapshot_block = next_epoch
    .saturating_mul(blocks_per_epoch)
    .saturating_sub(5);

if block_number == snapshot_block
    && EpochScoreSnapshot::<T>::get(next_epoch).is_none()
{
    let active = ActiveValidators::<T>::get();
    let snapshot = BoundedVec::truncate_from(
        active.iter().map(|v| {
            let score = ValidatorStates::<T>::get(v)
                .map(|s| s.current.final_score)
                .unwrap_or(1)
                .max(1);
            (v.clone(), score)
        }).collect::<Vec<_>>()
    );
    EpochScoreSnapshot::<T>::insert(next_epoch, snapshot);
}
```

**2. Use snapshot in `generate_deterministic_author_sequence`:**

```rust
// BEFORE (broken — reads live scores):
let score = state.map(|s| s.current.final_score).unwrap_or(1);
(v.clone(), score.max(1))

// AFTER (correct — reads frozen snapshot):
let snapshot = EpochScoreSnapshot::<T>::get(_epoch);
let validator_weights: Vec<(T::AccountId, u64)> = match snapshot {
    Some(snap) => {
        let weight_map: BTreeMap<_, _> = snap.into_iter().collect();
        validators.iter()
            .map(|v| (v.clone(), weight_map.get(v).copied().unwrap_or(1).max(1)))
            .collect()
    }
    None => {
        // Epoch 0 or missing snapshot — equal weights as safe fallback
        validators.iter().map(|v| (v.clone(), 1u64)).collect()
    }
};
// Sort by account ID for deterministic ordering
validator_weights.sort_by(|a, b| a.0.cmp(&b.0));
```

### Verification Criteria

- `EpochScoreSnapshot[N]` is written at block `N*100 - 5` on all nodes with identical values
- Author sequences for epoch N use snapshot weights, not live `ValidatorStates`
- Validators with higher stake/PoI scores receive proportionally more authoring slots
- No `author mismatch` errors caused by weight divergence

---

## Solution 3 — Score-Based Sort of ActiveValidators in Storage

**Bug Reference:** Bug 3  
**Severity:** Critical — causes fork when scores are equal (stable sort preserves different existing orders)  
**Component:** `pallet-cbc-dcf/src/lib.rs` → `handle_epoch_transition()` and `apply_pending_validator_actions()`

### Root Cause

`sort_validators_by_score` sorts by `final_score` and writes the result back
to `ActiveValidators` storage. When scores are equal (all zero during early
blocks), Rust's stable sort preserves the existing order — which differs
between nodes. The block verifier reads `ActiveValidators` directly from
storage to determine the expected author, so different storage orders produce
different expected authors for the same block.

### Invariant to Restore

> `ActiveValidators` storage order must be identical on all nodes at all times.
> The only ordering that guarantees this is account ID sort — account IDs are
> fixed, never change, and produce the same result regardless of score state.

### Solution

Replace all `sort_validators_by_score` calls that write to `ActiveValidators`
storage with account ID sort. Score-based ordering can still be used for
display purposes (e.g. metrics, health checks) but must never be written to
the storage that the block verifier reads.

```
Why account ID sort is correct:
  Account IDs are 32-byte fixed values derived from public keys.
  They never change during a validator's lifetime.
  Byte comparison is fully deterministic.
  All nodes produce identical sort results regardless of score state,
  network timing, or offchain worker submission order.
```

### Code Changes

**File:** `pallet-cbc-dcf/src/lib.rs`

**Change 1 — `handle_epoch_transition`:**
```rust
// BEFORE (broken):
Self::sort_validators_by_score(&mut active_validators);
ActiveValidators::<T>::put(active_validators.clone());

// AFTER (correct):
active_validators.sort_by(|a, b| a.cmp(b));  // account ID sort
ActiveValidators::<T>::put(active_validators.clone());
```

**Change 2 — `apply_pending_validator_actions`:**
```rust
// BEFORE (broken):
Self::sort_validators_by_score(&mut active);
ActiveValidators::<T>::put(active);

// AFTER (correct):
active.sort_by(|a, b| a.cmp(b));  // account ID sort
ActiveValidators::<T>::put(active);
```

**Change 3 — `handle_regular_block_processing` (participation update):**
```rust
// BEFORE (broken — writes score-sorted order to storage):
let mut active_validators = Self::active_validators();
Self::sort_validators_by_score(&mut active_validators);
ActiveValidators::<T>::put(active_validators);

// AFTER (correct — sort only for local use, do not write back):
// Score-based ordering is for metrics only, not written to storage.
// ActiveValidators storage order is maintained as account ID sorted.
```

### Verification Criteria

- `ActiveValidators` storage contains validators in account ID ascending order at all times
- Epoch transitions produce identical `ActiveValidators` storage on all nodes
- Block verifier reads same expected author from all nodes for any given block number

---

## Solution 4 — PoI Score Submission Frequency Causing State Flux

**Bug Reference:** Bug 4  
**Severity:** Medium — constant mempool flux amplifies score divergence between nodes  
**Component:** Offchain worker in `pallet-cbc-dcf/src/lib.rs`

### Context

PoI scores are currently 0 for all validators. This is **expected and intentional** —
the AI inference integration is not yet developed. Zero PoI scores are the correct
placeholder state until the AI layer is connected. This is not a bug.

### Actual Problem (Bug 4b only)

The offchain worker runs every 5 blocks and submits 3 unsigned transactions per
run (one score update per validator), even when all scores are 0. These transactions
are always in the mempool and land at different blocks on different nodes.

```
Every 5 blocks, on every node:
  offchain worker runs → computes score=0 for Alice, Bob, Charlie
  → submits 3 unsigned txs to mempool

These txs land at different blocks on different nodes:
  Alice's node:   tx lands at block 796 → ValidatorStates updated at 796
  Bob's node:     tx lands at block 797 → ValidatorStates updated at 797
  Charlie's node: tx lands at block 799 → ValidatorStates updated at 799

At epoch boundary (block 800):
  All scores are 0 on all nodes — but the STATE ROOT differs
  because the tx landed at different blocks
  → ValidatorStates storage differs between nodes
  → Amplifies Bug 2 and Bug 3
```

Even with all scores equal to 0, submitting redundant transactions every 5 blocks
keeps the chain state unnecessarily noisy and makes the score snapshot (Solution 2)
less reliable.

### Solution

Suppress offchain worker score submissions when the computed score has not changed
from the last submitted value. Since all scores are 0 and will remain 0 until AI
integration is complete, this means the offchain worker effectively becomes a no-op
until real inference data exists.

```rust
// In offchain worker score submission:
// Only submit if score has changed from last known on-chain value.

let current_on_chain_score = ValidatorStates::<T>::get(&validator)
    .map(|s| s.current.inference_score)
    .unwrap_or(0);

let new_score = compute_poi_score(&validator);  // returns 0 until AI connected

if new_score != current_on_chain_score {
    // Only submit tx if score actually changed
    submit_score_update(validator, new_score);
} else {
    // Score unchanged — skip submission, no mempool noise
    log::trace!("DCF: PoI score unchanged for {:?}, skipping submission", validator);
}
```

Additionally, when AI integration is complete, move score submission to a
fixed epoch-relative block offset rather than every N blocks, so submissions
are predictable and the snapshot window (Solution 2) captures stable values.

### Note on PoI Integration

When the AI inference layer is connected:
- `poi::Pallet::<T>::inference_results(validator)` must be populated by the AI oracle
- The offchain worker will then compute non-zero scores and submit them
- Solution 2 (score snapshot) ensures these scores are read at a stable point
- No changes to the scoring formula are needed — it is already correct

### Verification Criteria

- Offchain worker does not submit transactions when scores are unchanged
- Mempool does not contain redundant score-0 transactions
- When AI integration is added, scores flow correctly into the snapshot mechanism

---

## Solution 5 — DVF Finalized Head Stuck at Block 0

**Bug Reference:** Bug 5  
**Severity:** High — DVF finality mechanism non-functional  
**Component:** DVF finality sync service in `service.rs`, DVF pallet `trigger_finalization`

### Root Cause

When DVF finalizes a block via `client.finalize_block()`, the DVF pallet's
internal `LastFinalizedBlock` storage is not updated. The finality sync
service checks `get_dvf_finalized_block()` which reads `LastFinalizedBlock`
— it always returns 0. The sync service then logs "Client finalized head is
ahead of DVF finalized head" every 6 seconds indefinitely.

### Solution

`trigger_finalization` in the DVF pallet must update `LastFinalizedBlock`
after successfully finalizing a block. This is the single missing write.

```rust
// In pallet-cbc-dvf/src/lib.rs → trigger_finalization():

// AFTER successful client.finalize_block() call:
LastFinalizedBlock::<T>::put(block_number);
PreviousFinalizedBlock::<T>::put(
    LastFinalizedBlock::<T>::get().saturating_sub(1)
);

log::info!(
    "DVF: LastFinalizedBlock updated to {}",
    block_number
);
```

Additionally, the finality sync service in `service.rs` must read the correct
storage key. Verify that `get_dvf_finalized_block()` runtime API reads
`LastFinalizedBlock` and not a different storage item.

### Verification Criteria

- `DVF finalized head` advances in logs after each checkpoint block finalization
- "Client finalized head is ahead of DVF finalized head" warning disappears
- `get_dvf_finalized_block()` returns the most recently DVF-finalized block number

---

## Solution 6 — DVF Vote Weight Overflow at Startup

**Bug Reference:** Bug 6  
**Severity:** Medium — DVF finality cannot work during startup window  
**Component:** DVF Vote Aggregator weight calculation in `service.rs`

### Root Cause

At node startup, `EpochVotingWeight` is read before the runtime has fully
initialized validator stake values. The raw stake values (e.g. `1_000_000`
tokens with 12 decimal places = `1_000_000_000_000_000_000`) are used
directly as vote weights without scaling, causing u64 overflow when summed
across validators.

### Solution

Vote weights must be normalized to a fixed scale before use in the aggregator.
The normalization should happen at the point where `EpochVotingWeight` is
populated (genesis and epoch transitions), not at the aggregator.

```rust
// When populating EpochVotingWeight, normalize to u32 range:
// weight = (validator_stake / total_stake) * 32000
// This gives each validator a weight proportional to stake,
// bounded to a safe range that cannot overflow u64 when summed.

const VOTE_WEIGHT_SCALE: u64 = 32_000;

let total_stake: u128 = validators.iter()
    .map(|v| pos::Pallet::<T>::stake(v))
    .sum();

for validator in validators.iter() {
    let stake = pos::Pallet::<T>::stake(validator);
    let weight = if total_stake > 0 {
        ((stake as u128 * VOTE_WEIGHT_SCALE as u128) / total_stake) as u64
    } else {
        VOTE_WEIGHT_SCALE / validators.len() as u64
    };
    EpochVotingWeight::<T>::insert(validator, weight);
}
```

### Verification Criteria

- `DVF Vote Aggregator: Total weight` shows `96000` (or proportional value) from block 1
- No overflow values (`24000000000000240000`) appear in logs at any point
- Finality threshold is reachable from the first checkpoint block

---

## Solution 7 — Three Finality Systems Without Coordination

**Bug Reference:** Bug 7  
**Severity:** High — finality is undefined, DVF and DCF are redundant  
**Component:** `service.rs` — architecture level

### Root Cause

Three independent finality mechanisms run simultaneously with no coordination:
1. Substrate progressive client finality (finalizes every block, 1-block lag)
2. DVF vote-based finality (finalizes every 10th block with 2/3 votes)
3. DCF epoch-based finality (advances finality in `on_initialize`)

Substrate client finality runs unconditionally and finalizes blocks before
DVF can act on them. DVF's vote pool pruner then removes votes for already-
finalized blocks, leaving DVF with nothing to finalize.

### Solution

Establish a single finality authority with a clear hierarchy:

```
Hierarchy (highest authority first):
  1. DVF finality — 2/3 validator vote threshold (Byzantine fault tolerant)
  2. DCF progressive finality — fallback for non-checkpoint blocks
  3. Substrate client finality — disabled or deferred to DVF

Implementation:
  - Substrate progressive finality should only advance to the last
    DVF-finalized block, not beyond it
  - DCF progressive finality advances between DVF checkpoints
    (blocks N*10+1 through N*10+9 are confirmed by DCF after DVF
    finalizes block N*10)
  - DVF finality is the only mechanism that can advance the
    "hard finalized" head
  - Vote pool pruner must NOT prune votes for blocks that are
    client-finalized but not yet DVF-finalized
```

```
Correct flow:
  Blocks 1-9:   DCF progressive finality (soft confirmation)
  Block 10:     DVF finalizes with 2/3 votes (hard finality)
                → blocks 1-9 become hard-finalized as ancestors
  Blocks 11-19: DCF progressive finality
  Block 20:     DVF finalizes
  ...
```

### Verification Criteria

- Only one "finalized head" value exists and advances monotonically
- DVF finalized head equals client finalized head at all checkpoint blocks
- "Client finalized head is ahead of DVF finalized head" warning never appears
- Vote pool pruner only removes votes for DVF-finalized blocks

---

## Solution 8 — DVF Votes Only Cast Every 10 Blocks

**Bug Reference:** Bug 8  
**Severity:** Medium — by design, but interaction with Bug 7 makes DVF non-functional  
**Component:** `vote_creator.rs` → `is_checkpoint_block()`

### Root Cause

This is intentional design — DVF is designed to finalize every 10th block.
The bug is not the 10-block interval itself but the interaction with Bug 7:
Substrate client finality finalizes blocks 1-9 before DVF can act on block 10,
causing DVF to find 0 candidates.

### Solution

Once Bug 7 is resolved (Substrate client finality defers to DVF), the 10-block
checkpoint interval works correctly. Blocks 1-9 are soft-confirmed by DCF and
hard-finalized as ancestors when block 10 receives DVF votes.

If faster finality is required, the checkpoint interval can be reduced:

```rust
// Current: every 10 blocks
const CHECKPOINT_INTERVAL: u32 = 10;

// For faster finality (trade-off: more vote traffic):
const CHECKPOINT_INTERVAL: u32 = 5;
```

The checkpoint interval should be a runtime-configurable parameter, not a
hardcoded constant in `vote_creator.rs`. It must match `FinalityCheckpointInterval`
in the DVF pallet.

### Verification Criteria

- `FinalityCheckpointInterval` in DVF pallet matches hardcoded value in `vote_creator.rs`
- DVF votes are cast and aggregated successfully at every checkpoint block
- Checkpoint interval is configurable via runtime parameter, not hardcoded

---

## Solution 9 — Double Block Production (Two Validators Same Slot)

**Bug Reference:** Bug 9  
**Severity:** Critical — direct consequence of Bugs 1, 2, 3  
**Component:** `dcf.rs` → consensus loop

### Root Cause

This is a consequence of Bugs 1, 2, and 3. When author sequences diverge,
two validators simultaneously believe it is their turn to produce a block.
Each node's consensus loop independently calls `should_produce_block()` and
produces a block if it believes it is the expected author.

### Solution

Double block production is eliminated by fixing Bugs 1, 2, and 3. Once all
nodes have identical author sequences, only one validator will pass the
`is_expected_author` check for any given block number.

Additionally, add an explicit guard in the consensus loop:

```rust
// In dcf.rs → produce_block_with_validation():
// Before producing, verify no block at this height has been imported yet.
let current_best = self.client.info().best_number;
let target_block = current_best + 1;

// If a peer already produced this block and we imported it, skip.
if self.client.info().best_number >= target_block.into() {
    debug!("DCF: Block {} already produced by peer, skipping", target_block);
    return Ok(());
}
```

### Verification Criteria

- No two validators produce blocks for the same block number
- `Verification failed: Block author mismatch` errors do not appear
- `block has an unknown parent` errors do not appear

---

## Solution 10 — Epoch Transition Triggered from Consensus Loop

**Bug Reference:** Bug 10  
**Severity:** High — epoch transition runs multiple times, second run may use different state  
**Component:** `dcf.rs` → `run()` loop

### Root Cause

The DCF consensus loop in `dcf.rs` independently detects epoch boundaries
and calls `handle_epoch_transition` as a background task. This duplicates
the epoch transition that already runs correctly in `on_initialize`. The
background task fires at different wall-clock times on different nodes and
can fire multiple times for the same epoch.

### Solution

Remove the epoch transition trigger from the DCF consensus loop entirely.
Epoch transitions must only happen in `on_initialize` — the runtime hook
that is guaranteed to run exactly once per block on every node in the same
order.

```rust
// In dcf.rs → run() loop:

// REMOVE this entire block:
let current_block = self.client.info().best_number.saturated_into::<u32>();
let api = self.client.runtime_api();

if let Ok(current_epoch) = api.get_current_epoch(best_hash) {
    if let Ok(epoch_config) = api.get_epoch_config(best_hash) {
        let blocks_per_epoch = epoch_config.blocks_per_epoch;
        let should_transition = current_block > 0 && current_block % blocks_per_epoch == 0;
        if should_transition {
            // ...
            self.handle_epoch_transition(current_epoch).await;  // ← REMOVE
        }
    }
}

// Epoch transitions are handled exclusively by on_initialize in the runtime.
// The consensus loop only needs to read the current epoch for block production.
```

### Verification Criteria

- Each epoch transition appears exactly once in logs per epoch boundary
- No duplicate `DCF: Auto epoch transition N -> N+1` log lines
- `EpochAuthorSequences[N]` is written exactly once per epoch

---

## Implementation Order

Bugs must be fixed in this order due to dependencies:

```
Phase 1 — Author Sequence Determinism (eliminates fork):
  Fix 3 first  → ActiveValidators storage order deterministic
  Fix 1        → Randomness seed uses correct parent block hash
  Fix 2        → Score snapshot replaces live score reads
  Fix 10       → Remove duplicate epoch transition trigger

Phase 2 — DVF Finality (makes finality functional):
  Fix 6        → Correct vote weight calculation (no overflow)
  Fix 5        → DVF finalized head advances correctly
  Fix 7        → Single finality authority, remove three-way conflict
  Fix 8        → Verify checkpoint interval consistency

Phase 3 — Scoring Accuracy (makes PoI meaningful):
  Fix 4a       → Connect real inference results to PoI pallet
  Fix 4b       → Reduce offchain worker frequency to epoch-relative

Phase 4 — Double Production Guard (defensive):
  Fix 9        → Add explicit guard (becomes no-op once Phase 1 is complete)
```

---

## Testing Criteria (All Fixes)

A successful implementation must pass all of the following:

```
1. Run 3-node local network for 2000+ blocks with no author mismatch errors
2. Epoch transitions at blocks 100, 200, ..., 2000 produce no fork
3. DVF finalized head advances to match client finalized head at every checkpoint
4. No "Client finalized head is ahead of DVF finalized head" warnings
5. No duplicate epoch transition log lines
6. Vote weight shows 96000 total from block 1 (no overflow)
7. EpochScoreSnapshot written 5 blocks before each epoch boundary
8. ActiveValidators storage is account-ID sorted after every epoch transition
9. peer count stays at 2 throughout the entire 2000-block run
10. No "block has an unknown parent" errors
```

---

## Solution 11 — Aggressive Vote Pool Flushing

**Bug Reference:** Bug 11  
**Severity:** High — DVF finality starved, votes flushed mid-collection  
**Component:** `vote_aggregator.rs` → `VotePoolPruningService`, `dvf_block_import.rs` → `FinalityNotifier`

### Root Cause

Two compounding issues cause the vote pool to be flushed too aggressively:

**Issue A — FinalityNotifier fires on every block finalization:**
Substrate's progressive client finality finalizes every block (N-1 lag).
`dvf_block_import` sends a `FinalityNotification` on every finalization event,
not just DVF checkpoint finalizations. The pruner therefore runs after every
single block, not just after every 10th block.

```
Block 19 imported → Substrate finalizes block 18 → FinalityNotifier fires
→ prune_by_finalized_block(18) runs
→ any votes for blocks <= 18 are removed

Block 20 imported → VoteCreator starts casting votes for block 20
→ Substrate finalizes block 19 → FinalityNotifier fires SIMULTANEOUSLY
→ prune_by_finalized_block(19) runs while votes for block 20 are in flight
→ votes may be pruned before aggregator collects all 3
→ threshold never reached → block 20 never DVF-finalized
```

**Issue B — Votes pruned immediately after DVF finalization:**
After DVF finalizes block 10, `FinalityNotifier` fires immediately.
`prune_by_finalized_block(10)` removes all votes for block 10 before
the aggregator's next 1-second check. The aggregator then finds 0 candidates
for 9 consecutive seconds until block 20 arrives.

### Invariant to Restore

> The vote pool pruner must only fire after DVF checkpoint finalizations,
> not after every Substrate client finalization. Votes for the current
> checkpoint window must remain in the pool until the aggregator has
> confirmed finalization.

### Solution

**Fix A — Separate DVF finality notifications from Substrate client finality:**

The `FinalityNotifier` in `dvf_block_import.rs` should only fire when a
DVF justification is verified — not on every block import. Substrate's
progressive finality events must not trigger the DVF vote pool pruner.

```rust
// In dvf_block_import.rs:
// Only notify when a DVF justification is present and verified.
// Do NOT notify on every block finalization.

if justification_verified {
    // This is a real DVF checkpoint finalization
    notifier.notify(FinalityNotification { block_number, block_hash, round_id }).await;
}
// else: Substrate progressive finality — do NOT notify DVF pruner
```

**Fix B — Add a grace period before pruning checkpoint votes:**

After DVF finalizes a checkpoint block, retain its votes for at least one
full aggregator check interval (1 second) before pruning. This ensures the
aggregator can confirm finalization before votes disappear.

```rust
// In prune_after_finalization():
// Only prune votes for blocks strictly BELOW the finalized checkpoint.
// Keep votes for the finalized block itself until next pruning cycle.

votes.retain(|_, vote_list| {
    vote_list.retain(|vote| {
        vote.block_number >= finalized_block_number  // keep finalized block votes
    });
    !vote_list.is_empty()
});
```

**Fix C — Prune only on DVF checkpoint boundaries:**

The pruner should run on a schedule aligned with checkpoint intervals,
not reactively on every finality event:

```rust
// In VotePoolPruningService:
// Instead of reacting to every finality notification,
// prune on a fixed schedule every checkpoint_interval blocks.

let checkpoint_interval: u32 = 10;
if finalized_block_number % checkpoint_interval == 0 {
    self.prune_after_finalization(finalized_block_number, current_round);
}
// else: skip pruning for non-checkpoint finalizations
```

### Verification Criteria

- Vote pool contains votes for the current checkpoint block until aggregator confirms finalization
- `prune_by_finalized_block` only fires at checkpoint block boundaries (every 10 blocks)
- No votes are pruned while collection for the current checkpoint is in progress
- DVF Vote Aggregator finds 3 candidate votes at every checkpoint block
- "Found 0 candidate blocks" only appears between checkpoint blocks, not at them
- DVF finalized head advances by 10 at every checkpoint block
