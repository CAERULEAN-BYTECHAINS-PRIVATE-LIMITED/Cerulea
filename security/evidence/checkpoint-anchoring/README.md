# Control: checkpoint anchoring against the long-range attack

Task P10-002. Specification section 16.1. Design `03-design/09-security.md`.

A long-range attack rewrites history from a point far below the head. The
attacker builds a second chain from an old block and offers it to a node that
holds little history of its own. A node joining the network from genesis is the
target that matters. Block rules alone leave that node with two chains and no
way to tell which one the network lived through.

## What the control does

1. **Checkpoints on chain.** A checkpoint is one committed block the chain
   names as settled. The framework pallet anchors checkpoints in index order
   with no gap and no repeat.
2. **Checkpoints in the local store.** Each node keeps its own copy of the
   anchored set in a file beside the chain database.
3. **No reorganisation below the anchor.** A node refuses any chain whose
   common ancestor with the chain it holds sits below the highest anchored
   checkpoint.
4. **Sync runs through the anchors.** A node syncing from genesis accepts a
   chain only where the block at each anchored height carries the hash that
   anchor names.
5. **An external witness.** The Bitcoin anchoring path of the bridge module
   writes the checkpoint commitment into a data carrying output, so the Cerulea
   history gains a witness outside Cerulea.

Controls implemented, certification not held.

## Plain sentence

This control stops somebody from rewriting the distant past. Without the
control a party holding old validator keys could build a second history from
far below the head and offer it to a node that just joined.

## The implementing commit

IMPLEMENTING_COMMIT_PLACEHOLDER

## Where the code sits

| Part | Path |
|---|---|
| The shared checkpoint and its commitment | `cerulea-primitives/src/checkpoint.rs` |
| The local store, the reorganisation guard and the sync gate | `cerulea-node/src/cerulea-consensus/src/engines/checkpoint.rs` |
| The chain record and the anchoring extrinsic | `cerulea-pallets/pallet-cerulea-dcf/src/lib.rs` |
| The external witness | `cerulea-pallets/pallet-cerulea-bridge/src/anchor.rs` |

## The proving tests

| Target | Command |
|---|---|
| The shared checkpoint | `cargo test --release -p cerulea-primitives checkpoint` |
| The node store and the gates | `cargo test --release -p cerulea-consensus checkpoint` |
| The chain record | `cargo test --release -p pallet-cerulea-dcf checkpoint` |
| The external witness | `cargo test --release -p pallet-cerulea-bridge checkpoint` |
| The multi-node harness | `cargo test --release -p cerulea-node --test checkpoint_test` |

Run every target from `chain/`.

## The last run

`last-run.txt` holds the output of the last run of the targets.
