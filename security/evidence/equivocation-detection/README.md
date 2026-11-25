# Control: equivocation detection and penalty

Task P10-001. Specification section 16.1. Design `03-design/09-security.md`.

Equivocation means one validator signing two conflicting blocks for the same
slot. A slot is one fixed span of time. The Authoring Engine names one author at
each rank of each slot. An author that seals two different headers for one slot
at one rank has signed two futures for one moment, which breaks the single
history every participant relies on.

## What the control does

1. **Detection on import.** Every node keeps the sealed head of each block it
   accepts. A second head for one slot, one rank and one author seal key
   completes the evidence at once.
2. **Detection through a propagation report.** A node that saw one of the two
   headers learns of the other on the path named `/cerulea/equivocation/1`. The
   receiving node runs the whole evidence check itself.
3. **The evidence extrinsic.** `report_equivocation` in the framework pallet
   verifies both seal signatures, resolves the draw key to the validator
   account, returns standing to zero through the Standing Engine reset path,
   records the fault and takes the seat away at once.

Detection and penalty complete inside one epoch. The tracker holds a window of
six hundred slots, which spans a whole epoch, and the penalty lands in the epoch
the report arrives.

Controls implemented, certification not held.

## Plain sentence

This control stops one validator from promising two different histories for one
moment. Without the control a validator could show one block to half the
network and another block to the other half, and the two halves would disagree
about what happened.

## The implementing commit

`2d1630016ee24ceddb25cbecd7917bf0464830c7`, on branch `rebuild/foundation`.
Commit message: `P10-001 implement equivocation detection and penalty`.

## Where the code sits

| Part | Path |
|---|---|
| The shared evidence and its check | `cerulea-primitives/src/equivocation.rs` |
| Detection on the node | `cerulea-node/src/cerulea-consensus/src/engines/authoring/equivocation.rs` |
| The import hook | `cerulea-node/src/cerulea-consensus/src/import_queue.rs` |
| The propagation path | `cerulea-node/src/cerulea-consensus/src/equivocation_gossip.rs` |
| The protocol registration | `cerulea-node/src/service.rs` |
| The fault record | `cerulea-pallets/pallet-cerulea-dcf/src/engines/standing/equivocation.rs` |
| The evidence extrinsic | `cerulea-pallets/pallet-cerulea-dcf/src/lib.rs` |
| The key lookups | `cerulea-pallets/pallet-cerulea-dvf/src/lib.rs` |

## The proving tests

| Target | Command |
|---|---|
| Shared evidence | `cargo test --release -p cerulea-primitives equivocation` |
| Node detection and propagation | `cargo test --release -p cerulea-consensus equivocation` |
| Chain penalty | `cargo test --release -p pallet-cerulea-dcf equivocation` |
| Multi-node harness | `cargo test --release -p cerulea-node --test equivocation_test` |

Run every target from `chain/`.

## The last run

`last-run.txt` holds the output of the last run of the four targets.
