# CBC Chain — Consensus Bug Report

All problems found through log analysis and code review. No solutions included.

---

## Bug 1 — Non-Deterministic Randomness Seed (Live Block Number)

**Location:** `generate_deterministic_randomness()` in `pallet-cbc-dcf/src/lib.rs`

**What it does:** Generates the seed used to produce the author sequence for the next epoch. Every node must produce the exact same seed to get the same author sequence.

**The problem:**
```
frame_system::Pallet::<T>::block_number()
```
This reads the node's **current local block number** at the moment of execution, not the epoch transition block number.

**How it breaks:**
```
Epoch boundary is at block 1100.

Alice's consensus loop processes the transition while at block 1100.
  → calls block_number() → gets 1100 → hashes block 1100

Bob's consensus loop is slightly behind, processes it while at block 1099.
  → calls block_number() → gets 1099 → hashes block 1099

Alice's seed: blake2_256([..., hash_of_block_1100])
Bob's seed:   blake2_256([..., hash_of_block_1099])

Same epoch, completely different seeds
→ completely different author sequences for epoch 11
→ every block from 1100 onward: author mismatch
```

**Observed in logs:**
```
Block author mismatch: expected Some(5GoNkf6W...), got 5DbKjhNL... for block 1118
Block author mismatch: expected Some(5GoNkf6W...), got 5FA9nQDV... for block 1143
Error importing block: block has an unknown parent
```

---

## Bug 2 — Live Validator Scores Used as Author Sequence Weights

**Location:** `generate_deterministic_author_sequence()` in `pallet-cbc-dcf/src/lib.rs`

**What it does:** Uses validator scores as weights to decide how many authoring slots each validator gets in the epoch sequence. Higher score = more slots.

**The problem:**
```rust
let state = ValidatorStates::<T>::get(v);
let score = state.map(|s| s.current.final_score).unwrap_or(1);
```
`ValidatorStates` is live runtime state updated by the offchain worker via unsigned transactions. These transactions land in different blocks on different nodes depending on network timing.

**How it breaks:**
```
Epoch boundary is at block 800.
Offchain worker submits score batch at block 795.

Alice's node: batch tx lands at block 796
  → ValidatorStates at block 800: [Alice=50, Bob=30, Charlie=20]

Bob's node: batch tx lands at block 797
  → ValidatorStates at block 800: [Alice=50, Bob=30, Charlie=20]

Charlie's node: batch tx still in mempool at block 800
  → ValidatorStates at block 800: [Alice=45, Bob=28, Charlie=18]

Same seed (Bug 1 fixed) + different weights
→ Alice and Bob compute: block 819 author = 5FA9nQDV
→ Charlie computes:      block 819 author = 5DbKjhNL
→ fork
```

**Observed in logs:**
```
Block author mismatch: expected Some(5FA9nQDV...), got 5DbKjhNL... for block 819
Block author mismatch: expected Some(5FA9nQDV...), got 5DbKjhNL... for block 337
Block author mismatch: expected Some(5FA9nQDV...), got 5DbKjhNL... for block 932
```
Fork happens inside the epoch, not at the boundary, because the sequence was generated with different weights.

---

## Bug 3 — Score-Based Sort of ActiveValidators Written to Storage

**Location:** `handle_epoch_transition()` and `apply_pending_validator_actions()` in `pallet-cbc-dcf/src/lib.rs`

**What it does:** At every epoch boundary, after updating validator states, the code sorts `ActiveValidators` by `final_score` and writes that sorted order back to storage. The block verifier reads this stored order to check who should author each block.

**The problem — three conflicting sorting strategies:**

```
Sort 1 — Score-based sort (handle_epoch_transition, apply_pending_validator_actions):
  Sorts by final_score descending.
  Non-deterministic: scores differ between nodes due to offchain worker timing.
  Writes result to ActiveValidators storage.

Sort 2 — Account ID sort (generate_deterministic_author_sequence):
  Sorts by account ID bytes.
  Fully deterministic.
  Only sorts a local vec, does NOT write to storage.

Sort 3 — Weight-based selection (select_weighted_author):
  Iterates validator_weights in order, picks by cumulative weight threshold.
  Result depends entirely on the order of the input vec.
```

**How they conflict:**
```
Block 800: handle_epoch_transition runs on all nodes.
All PoI scores are 0 (offchain worker not submitted yet).
sort_validators_by_score with all equal scores
→ Rust stable sort preserves existing order on ties
→ but existing ActiveValidators order differs between nodes from previous operations

Alice's storage order after sort: [5GoNkf6W, 5DbKjhNL, 5FA9nQDV]
Bob's storage order after sort:   [5DbKjhNL, 5GoNkf6W, 5FA9nQDV]

generate_deterministic_author_sequence re-sorts by account ID locally
→ same order in the local weights vec on all nodes
→ same author sequence generated on all nodes

BUT the block verifier reads ActiveValidators directly from storage (not re-sorted)
→ Alice's verifier: order [5GoNkf6W, 5DbKjhNL, 5FA9nQDV] → expects 5FA9nQDV for block 819
→ Bob's verifier:   order [5DbKjhNL, 5GoNkf6W, 5FA9nQDV] → expects 5DbKjhNL for block 819
→ fork
```

**Observed in logs:**
```
Block author mismatch: expected Some(5FA9nQDV...), got 5DbKjhNL... for block 819
```
Epoch transition at block 800 logged successfully on all nodes. Fork appears 19 blocks later.

---

## Bug 4 — PoI Score Computed Per Block, Always Returns Zero

**Location:** Offchain worker in `pallet-cbc-dcf/src/lib.rs`

**What it does:** The offchain worker runs every block and computes PoI scores for all validators, submitting results as unsigned transactions.

**The problem:**
```
Every single block in the logs:
  DCF: Computed PoI score for validator=5FA9nQDV..., score=0, block=820
  DCF: Computed PoI score for validator=5GoNkf6W..., score=0, block=820
  DCF: Computed PoI score for validator=5DbKjhNL..., score=0, block=820
  DCF off-chain worker completed computation

score=0 on every block for every validator across the entire run.
poi::Pallet::<T>::inference_results(validator) returns None for all validators.
→ All validators always have equal PoI scores.
→ The PoI component of the consensus is completely non-functional.
```

**Secondary effect:**
```
Offchain worker runs every block → submits 3 score txs per block (one per validator)
→ 3 unsigned txs always in mempool
→ These txs land at different blocks on different nodes
→ ValidatorStates never stable, always in flux between nodes
→ Amplifies Bug 2 — scores are not just different, they are constantly changing
```

---

## Bug 5 — DVF Finalized Head Permanently Stuck at Block 0

**Location:** DVF finality sync service in `service.rs`

**What it does:** DVF finality is supposed to advance as validators vote on blocks. The DVF finalized head should track the highest block that received 2/3 validator vote weight.

**The problem:**
```
From the very first block, DVF finalized head never advances past 0.

Client finalized head (#10) is ahead of DVF finalized head (#0)!
Client finalized head (#20) is ahead of DVF finalized head (#0)!
Client finalized head (#30) is ahead of DVF finalized head (#0)!
...
Client finalized head (#820) is ahead of DVF finalized head (#0)!

This message repeats every 6 seconds for the entire run.
DVF finalized head = 0 from block 1 to block 1500+.
```

**What happens:**
```
DVF Vote Aggregator successfully finalizes block #820 with justification.
→ DVF finalized head should advance to 820.
→ Instead it stays at 0.

Next check (1 second later):
DVF Vote Aggregator: Found 0 candidate blocks for round 0
DVF Vote Aggregator: Vote pool size: 0 votes total

Votes were pruned immediately after finalization.
DVF finalized head in the runtime storage never gets updated.
→ DVF finality is not functioning as a finality mechanism.
→ Substrate client finality is doing all the work.
```

---

## Bug 6 — DVF Vote Weight Overflow at Startup

**Location:** DVF Vote Aggregator in `service.rs` / vote weight calculation

**What it does:** The vote aggregator calculates total validator weight to determine the finality threshold (67% of total weight).

**The problem:**
```
At node startup, before any blocks are produced:

DVF Vote Aggregator: Total weight: 24000000000000240000
DVF Vote Aggregator: Finality threshold: 16080000000000160800 (67% of total)

Expected total weight for 3 validators: 96000 (32000 each)
Actual total weight at startup: 24000000000000240000

This is a u64 overflow — the value wraps around or is computed incorrectly
from uninitialized or incorrectly scaled stake values.

After a few blocks the weight corrects to 96000.
But during the overflow window, the finality threshold is impossibly high
→ no block can ever reach finality threshold
→ DVF finality cannot work during this window
→ votes cast during this window are wasted
```

**Observed in logs:**
```
15:53:44 DVF Vote Aggregator: Total weight: 24000000000000240000, Finality threshold: 16080000000000160800
15:53:45 DVF Vote Aggregator: Total weight: 24000000000000240000, Finality threshold: 16080000000000160800
...
[several seconds later]
DVF Vote Aggregator: Total weight: 96000, Finality threshold: 64320
```

---

## Bug 7 — Three Finality Systems Running Simultaneously Without Coordination

**Location:** `service.rs` — DVF finality sync, DCF progressive finality, Substrate client finality

**What it does:** Three separate finality mechanisms all try to finalize blocks independently.

**The problem:**
```
System 1 — Substrate client finality (progressive, 1-block lag):
  DCF: Progressive finalization advanced to block 818
  DCF: Progressive finalization advanced to block 819
  → Finalizes every block automatically, 1 block behind best head
  → Runs unconditionally, cannot be stopped

System 2 — DVF finality (vote-based, 2/3 threshold):
  DVF Vote Aggregator: Successfully finalized block #820 with justification
  → Tries to finalize via validator votes
  → But Substrate client already finalized block 820 first
  → DVF vote pool pruner sees block 820 is client-finalized → prunes all votes
  → DVF aggregator: 0 candidates (votes gone before DVF can act)

System 3 — DCF finality (epoch-based, in pallet on_initialize):
  DCF: Finality advanced from 819 to 820 in epoch 8
  → Advances finality block by block inside the epoch
  → Conflicts with both above

Result:
  Substrate client finalizes block N
  → DVF pruner removes votes for block N
  → DVF finalized head stays at 0
  → DCF also advances independently
  → Three different finalized heads, no single source of truth
  → "Client finalized head is ahead of DVF finalized head" logged every 6 seconds forever
```

---

## Bug 8 — DVF Votes Cast for Every 10th Block Only (Sparse Voting)

**Location:** DVF Vote Creator in `vote_creator.rs`

**What it does:** Validators cast votes to finalize blocks. Votes should be cast for every block or at minimum for every block that needs finalization.

**The problem:**
```
DVF votes are only broadcast every 10 blocks:
  block #10  → vote broadcast
  block #20  → vote broadcast
  block #30  → vote broadcast
  block #40  → vote broadcast

Blocks 11-19, 21-29, 31-39 → no votes cast

Between vote rounds, DVF Vote Aggregator finds 0 candidates every second:
  DVF Vote Aggregator: Found 0 candidate blocks for round 0
  DVF Vote Aggregator: Vote pool size: 0 votes total

→ DVF finality only has a chance to work every 10 blocks
→ 9 out of every 10 blocks can never be DVF-finalized
→ Substrate client finality fills the gap, making DVF redundant
```

---

## Bug 9 — Double Block Production (Two Validators Producing Same Slot)

**Location:** DCF consensus loop in `dcf.rs`

**What it does:** Each node runs its own consensus loop and independently decides when to produce a block. The loop checks `should_produce_block()` and if true, selects the expected author and produces.

**The problem:**
```
When author sequences diverge (Bugs 1, 2, 3):

Alice's sequence for epoch 8, block 819: expected author = 5FA9nQDV (Charlie)
Bob's sequence for epoch 8, block 819:   expected author = 5DbKjhNL (Bob)

Charlie's consensus loop: "it is my turn for block 819" → produces block 819A
Bob's consensus loop:     "it is my turn for block 819" → produces block 819B

Both blocks are valid according to each node's own sequence.
Both are broadcast to the network simultaneously.

Alice receives block 819B from Bob → rejects (expected 5FA9nQDV, got 5DbKjhNL)
Bob receives block 819A from Charlie → rejects (expected 5DbKjhNL, got 5FA9nQDV)

Both chains grow independently from block 819.
Nodes remain P2P connected (2 peers shown in logs) but reject all blocks.
"block has an unknown parent" errors appear as chains diverge further.

This is not equivocation (malicious double signing).
It is deterministic disagreement — each node is 100% certain it is correct.
No node is misbehaving. The consensus algorithm produces two valid answers.
```

**Observed in logs:**
```
💔 Verification failed for block 0xaa890b...: "Block author mismatch:
   expected Some(5FA9nQDV...), got 5DbKjhNL... for block 819"

[simultaneously on Bob's node]
💔 Verification failed for block 0x60ca...: "Block author mismatch:
   expected Some(5DbKjhNL...), got 5FA9nQDV... for block 819"

[minutes later]
💔 Error importing block 0x...: block has an unknown parent
💔 Error importing block 0x...: block has an unknown parent
```

---

## Bug 10 — Epoch Transition Triggered from Consensus Loop (Not Runtime)

**Location:** `dcf.rs` `run()` loop — `handle_epoch_transition` call

**What it does:** The DCF consensus loop in `service.rs` independently detects epoch boundaries and triggers epoch transitions outside of the runtime's `on_initialize`.

**The problem:**
```
Two separate epoch transition triggers exist:

Trigger 1 — Runtime on_initialize (correct path):
  Runs as part of block execution on every node for every block.
  Deterministic: all nodes execute it for the same block at the same time.

Trigger 2 — DCF consensus loop (incorrect path):
  Runs in a background task on each node independently.
  Non-deterministic: timing depends on each node's consensus loop interval.
  Can fire at different wall-clock times on different nodes.
  Can fire multiple times (seen in logs: epoch transition logged twice for same epoch).

From logs:
  DCF: Auto epoch transition 6 -> 7 (standard mode) - 3 active validators  [line 13087]
  DCF: Deterministic epoch transition completed for epoch 7 at block 700    [line 13089]
  DCF: Auto epoch transition 6 -> 7 (standard mode) - 3 active validators  [line 13105]  ← duplicate
  DCF: Deterministic epoch transition completed for epoch 7 at block 700    [line 13107]  ← duplicate

The same epoch transition runs twice on Alice for epochs 2, 3, 6, 7, 10, 11.
→ EpochAuthorSequences written twice for the same epoch
→ Second write may use different state than first write
→ Unpredictable which sequence is actually used for block verification
```

---

## Bug 11 — Aggressive Vote Pool Flushing After Every Block Finalization

**Location:** `VotePoolPruningService` in `vote_aggregator.rs`, `FinalityNotifier` in `dvf_block_import.rs`

**What it does:** After any block is finalized, the pruning service immediately
removes all votes from the pool for blocks at or below the finalized block number.

**The problem — two compounding issues:**

**Issue A — Pruner fires on every block, not just DVF checkpoints:**
```
FinalityNotifier is triggered by dvf_block_import on every finalization.
Substrate's progressive client finality finalizes every block (N-1 lag).

Block 11 imported → Substrate finalizes block 10 → FinalityNotifier fires
→ prune_by_finalized_block(10) → removes all votes for blocks <= 10

Block 12 imported → Substrate finalizes block 11 → FinalityNotifier fires
→ prune_by_finalized_block(11) → removes all votes for blocks <= 11

...

Block 19 imported → Substrate finalizes block 18 → FinalityNotifier fires
→ prune_by_finalized_block(18) → removes all votes for blocks <= 18

Block 20 imported → VoteCreator casts votes for block 20
→ Substrate finalizes block 19 → FinalityNotifier fires IMMEDIATELY
→ prune_by_finalized_block(19) runs while votes for block 20 are being collected
→ votes for block 20 may be pruned mid-collection
→ VoteAggregator finds 0 or 1 votes → threshold not reached → block 20 never DVF-finalized
```

**Issue B — Votes pruned immediately after DVF finalization:**
```
Block 10 reaches DVF threshold:
  → trigger_justification_construction() finalizes block 10
  → FinalityNotifier fires immediately
  → prune_by_finalized_block(10) removes ALL votes for block 10
  → VoteAggregator checks 1 second later
  → "Found 0 candidate blocks for round 0"
  → "Vote pool size: 0 votes total"
  → This repeats every second for 9 blocks until block 20

The votes were correctly used to finalize block 10, but they are pruned
before the aggregator can confirm it already handled them. The aggregator
then spends 9 seconds finding nothing, logging 0 candidates every second.
```

**Observed in logs:**
```
DVF Vote Aggregator: Successfully finalized block #820 with justification
DVF Vote Aggregator: Finality latency for round 0: 0.00s

[1 second later]
DVF Vote Aggregator: Found 0 candidate blocks for round 0
DVF Vote Aggregator: Vote pool size: 0 votes total

[repeats every second for 9 blocks]
DVF Vote Aggregator: Found 0 candidate blocks for round 0
DVF Vote Aggregator: Vote pool size: 0 votes total
```

**Combined effect:**
```
DVF finality only works occasionally — when all 3 votes arrive and are
collected by the aggregator before the pruner fires.
When the pruner fires mid-collection, the checkpoint block is never
DVF-finalized and Substrate client finality fills the gap instead.
This is why DVF finalized head stays at 0 most of the time (Bug 5).
```

| Bug | Component | Core Problem |
|-----|-----------|--------------|
| 1 | `generate_deterministic_randomness` | Live local block number used as seed entropy |
| 2 | `generate_deterministic_author_sequence` | Live ValidatorStates scores used as weights |
| 3 | `handle_epoch_transition` + `apply_pending_validator_actions` | Score-based sort of ActiveValidators written to storage |
| 4 | Offchain worker | PoI score always 0, submits txs every block causing constant state flux |
| 5 | DVF finality sync | DVF finalized head permanently stuck at 0 |
| 6 | DVF Vote Aggregator | Weight overflow at startup producing impossibly high finality threshold |
| 7 | service.rs | Three finality systems (Substrate client, DVF, DCF) running without coordination |
| 8 | DVF Vote Creator | Votes only cast every 10 blocks, 9/10 blocks can never be DVF-finalized |
| 9 | DCF consensus loop | Two validators simultaneously produce blocks for same slot when sequences diverge |
| 10 | DCF consensus loop | Epoch transition triggered from background task, fires multiple times per epoch |
| 11 | VotePoolPruningService + FinalityNotifier | Pruner fires on every block finalization, flushes votes mid-collection, starves DVF finality |
