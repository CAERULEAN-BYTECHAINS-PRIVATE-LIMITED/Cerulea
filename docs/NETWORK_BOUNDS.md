# Network bounds

Every value a Cerulea node decodes from the network crosses a trust bound. A
sender chooses those bytes. This document lists each decode path a stranger can
reach, states what the path decodes and names the fuzz target that drives it.

Plain sentence. A bound is the line between the bytes somebody else sent and
the state our node keeps. Every line here carries a check, and every check
carries a fuzz target that throws malformed bytes at it.

## The paths

1. **The block seal.** A block header arrives carrying a seal in its digest.
   The Authoring Engine decodes the seal claim and the signature, then checks
   membership, signature and rank timing. Target: `seal_claim_decode`.

2. **The commit vote.** A validator sends one vote per block to the author of
   the slot. The Commit Engine decodes the vote and verifies the signature
   before the vote joins the tally. Target: `commit_vote_decode`.

3. **The aggregate signature.** The author sends one aggregate to the network.
   The envelope carries a signer bitmap and a signature list. Target:
   `aggregate_signature_decode`.

4. **The Access Manifest.** Every transaction declares the state it reads and
   writes before execution. The Execution Engine decodes that declaration and
   validates it before partitioning. Target: `access_manifest_decode`.

5. **The equivocation proof.** Any node may report two blocks sealed by one
   validator at one slot. The report carries two sealed heads. Target:
   `equivocation_proof_decode`.

6. **The state roots digest.** A block carries the per-domain roots and the
   combined root in its digest. An importing node decodes both. Target:
   `state_roots_decode`.

7. **The Blueprint.** A deployment installs a Blueprint. The artefact arrives
   as bytes and decodes into the twelve sections before anything runs. Target:
   `blueprint_decode`.

8. **The header digest.** The whole digest of an arriving header decodes
   before any engine reads it. Target: `header_digest_decode`.

## The rule each target holds

A target passes when the decode ends in one of two ways. The bytes decode into
a value the checks then accept or refuse by name. The bytes fail to decode and
the call answers with an error. A target fails when the decode panics, runs out
of memory, or loops without end.

## Running the targets

```
bash scripts/ci/fuzz.sh
```

The nightly workflow gives each target the same slice of time. A crash writes
the input under `test-results/fuzz` beside the commit hash of the tree under
test.

## The bound and the price of each path

Task P10-004 adds a hard size bound and a price to every network-reachable
path. The shared primitives crate owns the table at
`cerulea-primitives/src/bounds.rs`, so the node, the runtime and this document
never drift apart.

Two rules hold at every path.

1. **A hard size bound.** The path names the highest number of bytes one
   message carries. A larger message is refused on the length alone, before any
   decoding runs. Refusing an oversized message therefore costs one comparison.
2. **A price that follows the size.** The path names a base price and a price
   for each byte. The caller pays the price before the work starts, so a large
   message costs what it costs to handle. The figures are weight units, the
   same unit the Execution Engine meters.

Plain sentence. Without these two rules one message could carry thousands of
items at the price of one and slow every node that reads it.

| Path | Bound, bytes | Base price | Price per byte | Where the gate runs |
|---|---|---|---|---|
| block seal | 1,024 | 20,000 | 100 | `engines/authoring/seal.rs` |
| commit vote | 256 | 10,000 | 100 | `engines/commit/collect.rs` |
| commit submission | 65,536 | 10,000 | 80 | `engines/commit/submission.rs` |
| equivocation report | 768 | 40,000 | 120 | `engines/authoring/equivocation.rs` |
| propagation message | 8,192 | 5,000 | 40 | `dvf_gossip.rs` and `service.rs` |
| custody answer | 4,096 | 30,000 | 60 | `pallet-cerulea-custody/src/respond.rs` |
| bridge proof | 262,144 | 50,000 | 60 | `pallet-cerulea-bridge/src/verify.rs` |
| extrinsic body | 3,932,160 | 10,000 | 40 | the runtime block length |
| remote procedure call body | 10,485,760 | 5,000 | 10 | `service.rs` |
| Blueprint artefact | 8,388,608 | 100,000 | 20 | the deployment path |

### How each figure was reached

1. **The block seal.** The seal claim encodes to two hundred and nine bytes and
   the signature to sixty four. The bound leaves room for the digest envelope.
2. **The commit vote.** A vote carries the slot, the block hash, the signer key
   and one signature.
3. **The commit submission.** One aggregate carries up to two hundred and fifty
   six signatures with the signer bitmap.
4. **The equivocation report.** The proof carries two sealed heads at three
   hundred and five bytes each.
5. **The propagation message.** The Commit Engine vote path carries one vote
   per message. The bound leaves room for the envelope of the propagation
   layer.
6. **The custody answer.** The Custody Engine caps one record at one thousand
   and twenty four bytes and one proof at thirty two steps.
7. **The bridge proof.** The bridge module caps a proof at sixty four nodes of
   two thousand and forty eight bytes each.
8. **The extrinsic body.** The runtime carries a block of five megabytes and
   gives three quarters of it to the ordinary transaction class.
9. **The remote procedure call body.** The node clamps the request size of the
   interface server to this figure at start.
10. **The Blueprint artefact.** A Blueprint of eight megabytes covers the
    largest deployment the specification names.

### The proving tests

```
cargo test --release -p cerulea-primitives bounds
cargo test --release -p cerulea-consensus bounds
cargo test --release -p cerulea-node --test network_bounds_test
```

Controls implemented, certification not held.
