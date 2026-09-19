# Control: peer diversity against the eclipse attack

Task P10-003. Specification sections 16.1 and 16.7. Design
`03-design/09-security.md`.

An eclipse attack isolates a node behind attacker-controlled peers. Every block,
every vote and every transaction the node sees then comes from one party. The
node keeps following its own rules and still reaches a false view of the
network, so authoring from that node harms the chain.

## What the control does

1. **A minimum peer count.** A node with too few peers sees too little of the
   network.
2. **A minimum count of distinct address ranges.** Peers inside one range count
   once. An address of the fourth version counts by its leading three bytes. An
   address of the sixth version counts by its leading six bytes.
3. **The validator allow-list on a private chain.** A private chain admits the
   validators its registry names and nobody else.
4. **The halt and the alert.** A node failing the first or the second condition
   halts authoring and raises an alert naming the fix. Reading stays open.
   Import stays open. Authoring resumes as soon as the peer set recovers.

Controls implemented, certification not held.

## Plain sentence

This control stops one party from surrounding a node and feeding it a false
picture of the network. Without the control a node with every peer inside one
operator network would keep producing blocks on a history nobody else shares.

## The implementing commit

IMPLEMENTING_COMMIT_PLACEHOLDER

## Where the code sits

| Part | Path |
|---|---|
| The peer set and the assessment | `cerulea-node/src/cerulea-consensus/src/engines/peers.rs` |
| The authoring halt | `cerulea-node/src/cerulea-consensus/src/engines/authoring/gate.rs` |
| The node peer record | `cerulea-node/src/service.rs` |
| The harness peer sets | `cerulea-node/src/test_harness.rs` |

## The proving tests

| Target | Command |
|---|---|
| The peer set and the assessment | `cargo test --release -p cerulea-consensus peers` |
| The authoring halt | `cargo test --release -p cerulea-consensus gate` |
| The multi-node harness | `cargo test --release -p cerulea-node --test eclipse_test` |

Run every target from `chain/`.

## The last run

`last-run.txt` holds the output of the last run of the targets.
