# CBC Chain — Block Confirmation and Finality Architecture

This document describes the layered consensus model of CBC Chain as observed
from the codebase and node logs. There are three distinct levels, each with a
specific responsibility.

---

## Level 0 — Validator Scoring (PoS + PoI)

This is the foundation. Before any block is produced, every validator has a
score that determines how many authoring slots they receive in the next epoch.

### How the score is calculated

```
final_score = (stake_score * pos_weight + inference_score * poi_weight)
              / PercentagePrecision
```

- `stake_score` — derived from the validator's locked stake via the PoS pallet
- `inference_score` — derived from the validator's AI inference results via the PoI pallet
- `pos_weight` and `poi_weight` — configurable weights, must sum to 100
- `PercentagePrecision` — precision divisor (100)

### When scores are updated

| Event | What updates |
|-------|-------------|
| Every 5 blocks | Offchain worker runs, computes PoI inference score, submits unsigned tx |
| Tx lands on chain | `ValidatorStates.inference_score` updated, `update_final_score` called |
| Every 10 blocks | `ScoreDecayInterval` — inactive validators lose score |
| Every 100 blocks | `ParticipationUpdateInterval` — participation rates recalculated |
| Every epoch boundary | `run_comprehensive_score_aggregation` refreshes all scores |

### What the score controls

At every epoch boundary (every 100 blocks), the final scores of all active
validators are used as weights to generate the author sequence for the next
epoch. A validator with `final_score = 50` gets roughly twice as many authoring
slots as one with `final_score = 25`.

---

## Level 1 — Block Production and DCF Confirmation (Per Block)

Every block is produced by exactly one validator as determined by the
pre-computed author sequence for the current epoch.

### Author selection flow

```
Epoch boundary (block N*100)
  → generate_deterministic_author_sequence(epoch, validators, seed)
  → produces a 100-slot sequence, one author per block offset
  → stored in EpochAuthorSequences[epoch] on-chain

Block N*100 + offset
  → get_expected_author(block_number) reads EpochAuthorSequences
  → returns the validator assigned to that offset
  → only that validator's DCF consensus loop produces the block
```

### Block confirmation (DCF — per block)

Once a block is imported, DCF confirms it through progressive finalization:

```
Block imported at height H
  → on_initialize runs for block H
  → Progressive finalization: LastFinalizedBlock advances to H-1
  → DCF logs: "Finality advanced from H-1 to H in epoch E"
  → Block H-1 is now DCF-confirmed
```

This means every block is DCF-confirmed one block after it is imported.
Blocks 0 through H-1 are all auto-confirmed as soon as block H is imported.
This is the "client finality" seen in logs as:

```
DCF: Progressive finalization advanced to block 818
DCF: Finality advanced from 817 to 818 in epoch 8
```

### Block time

- Target: 6 seconds per block
- Consensus loop interval: 1000ms (checks every 1 second if it is this node's turn)
- Slot duration: 6 seconds

---

## Level 2 — DVF Finality (Every 10th Block)

DVF (Distributed Validator Finality) provides Byzantine-fault-tolerant
finality on top of DCF confirmation. It requires a 2/3 weighted threshold of
validator votes to finalize a checkpoint block.

### Which blocks get DVF votes

Only checkpoint blocks receive DVF votes. A checkpoint block is any block
where:

```
block_number % FinalityCheckpointInterval == 0
FinalityCheckpointInterval = 10
```

So blocks 10, 20, 30, 40, ... are checkpoint blocks.

### DVF vote flow

```
Block 10 imported
  → VoteCreatorService receives import notification
  → Checks: is block 10 a checkpoint? (10 % 10 == 0) → YES
  → Checks: is block 10 already finalized? → NO
  → Checks: is this node an active validator? → YES
  → Creates DVF vote: { block_number: 10, block_hash: 0x..., epoch: 0, round: 0 }
  → Signs vote with validator's ed25519 key (CBC_DVF_KEY_TYPE = b"cdvf")
  → Broadcasts vote via DVF gossip protocol (/cbc/dvf/1)

Other nodes receive the vote via gossip
  → DVF Gossip Validator accepts and inserts into local vote pool
  → VoteAggregatorService checks every 1 second

VoteAggregatorService at block 10:
  → Finds 3 votes for block 0x... in round 0
  → Calculates accumulated weight: 32000 + 32000 + 32000 = 96000
  → Checks threshold: 96000 >= 64320 (67% of 96000) → REACHED
  → Triggers JustificationBuilder
  → Justification constructed with 3 votes, weight 96000
  → client.finalize_block(block_10_hash, justification) called
  → Block 10 is DVF-finalized
```

### What DVF finalization means for blocks 1-9

Once block 10 is DVF-finalized, all blocks from 1 to 9 are implicitly
finalized because they are ancestors of block 10. Substrate's finality model
guarantees that finalizing block N finalizes all ancestors.

```
DVF finalizes block 10
  → blocks 1, 2, 3, 4, 5, 6, 7, 8, 9 are all finalized as ancestors
DVF finalizes block 20
  → blocks 11 through 19 are all finalized as ancestors
```

### Vote weights

Each validator's vote weight comes from `EpochVotingWeight` in the DVF pallet,
which is populated at genesis and epoch transitions:

```
Total weight (3 validators, equal stake): 96000
  Alice:   32000
  Bob:     32000
  Charlie: 32000

Finality threshold: 67% of total = 64320
Minimum votes needed: any 2 of 3 validators (64000 >= 64320 is NOT enough,
all 3 needed with equal weights: 96000 >= 64320)
```

### Vote retention

Votes older than `VoteRetentionRounds = 20` rounds are pruned from the pool.
The pruning service runs after each DVF finalization.

---

## Full Block Lifecycle (Example: Blocks 1 to 20)

```
Block 1 produced by Alice (epoch 0, offset 1)
  → DCF: block 0 auto-confirmed (progressive finalization)

Block 2 produced by Bob
  → DCF: block 1 auto-confirmed

...

Block 9 produced by Charlie
  → DCF: block 8 auto-confirmed

Block 10 produced by Alice (checkpoint block)
  → DCF: block 9 auto-confirmed
  → DVF: all 3 validators vote on block 10
  → DVF: 96000 weight accumulated, threshold 64320 reached
  → DVF: block 10 finalized with justification
  → DVF: blocks 1-9 finalized as ancestors of block 10

Block 11 produced by Bob
  → DCF: block 10 auto-confirmed (already DVF-finalized, no conflict)

...

Block 20 produced by Alice (checkpoint block)
  → DVF: all 3 validators vote on block 20
  → DVF: block 20 finalized with justification
  → DVF: blocks 11-19 finalized as ancestors of block 20

Block 100 (epoch boundary)
  → Score aggregation runs
  → Author sequence for epoch 1 generated using validator scores
  → EpochAuthorSequences[1] stored on-chain
  → Epoch 1 begins, new 100-slot author sequence active
```

---

## Interval Summary

| Interval | Value | What happens |
|----------|-------|-------------|
| Every block | 1 block | DCF progressive finalization (N-1 confirmed) |
| Every 5 blocks | 5 blocks | Offchain worker computes PoI scores |
| Every 10 blocks | 10 blocks | DVF checkpoint vote and finalization |
| Every 10 blocks | 10 blocks | Score decay check, leave request check |
| Every 50 blocks | 50 blocks | Underperformance check |
| Every 100 blocks | 100 blocks | Epoch boundary — new author sequence generated |
| Every 100 blocks | 100 blocks | Participation rates updated |
| Every 200 blocks | 200 blocks | Validator proposals generated |
| Every 1000 blocks | 1000 blocks | Health metrics emitted |

---

## Three-Layer Summary

```
Layer 0 — Scoring (PoS + PoI)
  Purpose: Determine how many authoring slots each validator earns
  Frequency: Continuous (offchain worker every 5 blocks)
  Output: final_score per validator → used at epoch boundary

Layer 1 — DCF (per block)
  Purpose: Produce blocks in deterministic order, confirm each block
  Frequency: Every block (6 second target)
  Output: Imported block + progressive finalization of N-1

Layer 2 — DVF (every 10 blocks)
  Purpose: Byzantine-fault-tolerant finality with 2/3 threshold
  Frequency: Every 10th block (checkpoint)
  Output: Justified finalization of checkpoint + all ancestor blocks
```
