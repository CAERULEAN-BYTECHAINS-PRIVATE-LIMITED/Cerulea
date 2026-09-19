# Control: every network-reachable message path bounded and repriced

Task P10-004. Specification section 16.1. Design `03-design/09-security.md`.

A network-reachable path is any place where a Cerulea node turns bytes a
stranger sent into a value. A stranger chooses those bytes.

## What the control does

1. **A hard size bound per path.** Each path names the highest number of bytes
   one message carries. A larger message is refused on the length alone, before
   any decoding runs.
2. **A price that follows the size.** Each path names a base price and a price
   for each byte. The caller pays before the work starts, so a large message
   costs what it costs to handle.
3. **One table.** The bounds and the prices sit in one shared module, so the
   node, the runtime and the enumeration never drift apart.

The enumeration sits at `chain/docs/NETWORK_BOUNDS.md` with the bound per path.

Controls implemented, certification not held.

## Plain sentence

This control stops one message from costing the network far more than the
sender paid for it. Without the control a single message could carry thousands
of items at the price of one and slow every node that reads it.

## The implementing commit

`2c7d301`, on branch `rebuild/foundation`.
Commit message: `P10-004 bound and reprice every network-reachable message path`.

## Where the code sits

| Part | Path |
|---|---|
| The bound and the price of each path | `cerulea-primitives/src/bounds.rs` |
| The enumeration | `chain/docs/NETWORK_BOUNDS.md` |
| The vote submission gate | `cerulea-node/src/cerulea-consensus/src/engines/commit/submission.rs` |
| The propagation gate | `cerulea-node/src/cerulea-consensus/src/dvf_gossip.rs` |
| The equivocation report gate | `cerulea-node/src/cerulea-consensus/src/engines/authoring/equivocation.rs` |
| The protocol and interface limits | `cerulea-node/src/service.rs` |

## The proving tests

| Target | Command |
|---|---|
| The bound table | `cargo test --release -p cerulea-primitives bounds` |
| The node gates | `cargo test --release -p cerulea-consensus bounds` |
| Every path | `cargo test --release -p cerulea-node --test network_bounds_test` |

Run every target from `chain/`.

## The last run

`last-run.txt` holds the output of the last run of the targets.
