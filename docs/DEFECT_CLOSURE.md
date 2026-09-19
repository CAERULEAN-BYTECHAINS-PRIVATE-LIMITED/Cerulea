# Defect closure

This document records every defect prior audit work found in the consensus
tree, and the task that closes it. The two earlier documents, the defect report
and the proposed solutions, are replaced by this one record. Each row names
where the closure sits so a reader verifies rather than assumes.

A defect marked open carries the task that closes it and the reason the work is
still in flight.

## The defects the build specification carried forward

| Defect | State | Closed by |
|---|---|---|
| Block authorship is forgeable. No seal exists and the production path touches no keystore. | Closed | P1-013 adds the seal signed from the keystore. P1-014 verifies the seal on import. P1-016 removes the unsigned authorship digest path. |
| No local author gate exists. Every validator node attempts every block. | Closed | P1-015 adds the local author gate on block production. |
| Inference scoring reaches zero. The runtime sets a 60 and 40 split while a second pair sets 100 and 0. | Closed | P1-032 replaces the scoring model with the Trust Score composition. P1-045 makes the inference contribution a feature-gated module that stays off by default. P1-046 removes the second pair and renames the first to state plainly that it feeds the retired path alone. |
| Two finality paths coexist. The framework pallet advances an on-chain marker while the vote pallet performs the real weighted voting. | Open | P1-022 removed the node-side path. The framework pallet marker, its two interface calls, its four writers and its two readers are still in place. P1-046 carries the removal. |
| The justification path is unbounded. Vote submission accepts unbounded counts with a fixed declared weight. | Closed | P1-020 bounds and reprices the vote submission path. |
| The propagation message set is unbounded. The message validator retains messages with no eviction. | Closed | P1-021 caps the message set and evicts at capacity. |

## The defects the earlier report recorded

| Defect | State | Closed by |
|---|---|---|
| The randomness seed reads the live block number, so two nodes derive different author sequences. | Closed | P1-010 derives the draw from the slot seed, which comes from the parent block hash and the slot number. Every node reaches the same value. |
| Live validator scores weight the author sequence, so a score change mid-epoch changes the sequence. | Closed | P1-011 ranks the committee on the draw output scaled by the Trust Score held at the slot. |
| A score-based sort of the active validator list is written to storage. | Closed | P1-011 keeps the ordering inside the committee draw. The active list stays in admission order. |
| The inference score is computed per block and always returns zero. | Closed | P1-045 gates the contribution behind a cargo feature that stays off by default, a determinism declaration and a real workload source. |
| The finalised head of the vote pallet is stuck at block zero. | Closed | P1-019 commits a domain in one round on aggregated Trust Score weight. |
| Vote weight overflows at startup. | Closed | P1-018 aggregates through the signer bitmap with saturating arithmetic. |
| Three finality systems run at once with no coordination. | Open | P1-022 left one commit path on the node side. The framework pallet marker remains. P1-046 carries the removal. |
| Votes are cast for every tenth block only. | Closed | P1-019 commits every domain rather than a block interval. |
| Two validators produce a block for one slot. | Closed | P1-015 adds the local author gate. P1-014 rejects a second seal at one slot on import. |
| The epoch transition fires from the consensus loop rather than the runtime. | Closed | P1-044 moves the epoch work into the block initialisation hook of the framework pallet. |
| The vote pool is flushed after every finalisation. | Closed | P1-021 replaces the flush with a capacity limit and least-recently-used eviction. |

## How to verify a closure

1. **Read the commit.** The pallet task record sits in
   `cerulea-build/02-tasks/status/pallets.md`. The consensus task record sits in
   `cerulea-build/02-tasks/status/consensus.md`. Each line carries the commit
   hash.
2. **Run the tests.** Each task ships tests beside the code it changes. The
   design assertions are listed at the end of `CONSENSUS_ARCHITECTURE.md`.
3. **Read the baseline.** `TEST_BASELINE.md` carries the per-package counts and
   the named list of every failing test at the moment the framework work began.

## The check that stops mock drift returning

Each pallet mock ends with a compile-time contract block naming every trait the
mock claims to satisfy. A trait that gains an item stops the test target from
compiling at that block, with the trait named in the error. Every commit gate
builds the test target, so the check runs on every commit.
