# Cerulea consensus crate

The node side of the Dynamic Consensus Framework. The Dynamic Consensus
Framework is the agreement mechanism of the Cerulea Blockchain. Eight engines
carry the work. Three of them run on the node and live in this crate.

| Engine | Responsibility | Location |
|---|---|---|
| Commit Engine | Reaches agreement and makes blocks irreversible | `src/engines/commit` |
| Authoring Engine | Selects the slot committee and the block author | `src/engines/authoring` |
| Execution Engine | Groups transactions into domains and runs them in parallel | `src/engines/execution` |

The remaining five engines run inside the pallet set. The Assurance Engine, the
Standing Engine and the Amendment Engine sit in the framework pallet. The
Custody Engine sits in its own pallet. The Admission Engine sits in the
validator registry pallet.

## 1. The shared types

`src/engines/mod.rs` holds what the three node side engines share.

- **TransactionClass.** Routine, Financial, Structural, Constitutional. The
  class sets the share of aggregated weight the transaction requires.
- **CollectionId and KeyRange.** A collection is one named store of records. A
  key range is one key, every key beneath a prefix, or a whole collection.
- **TrustScore.** One composite measure in parts per million. The measure
  drives the Authoring, Commit, Custody and Admission Engines.
- **Slot, SlotSeed, Rank, Committee.** The shape of one authoring slot.
- **ConflictDomain.** A maximal set of transactions whose state reaches never
  meet the reaches of another domain.
- **The default threshold table.** Routine 51 percent. Financial 67 percent.
  Structural 80 percent. Constitutional 90 percent.
- **The fixed limits.** The total participant count exceeds three times the
  faulty count. The framework assumes a bound on message delay that eventually
  holds, and states the assumption in the module documentation.

## 2. The Access Manifest

Every Cerulea transaction declares the state it reads and the state it writes
before execution begins. Cerulea derives the declaration from the Blueprint
automatically.

1. **`manifest.rs`.** The type and its encoding. Each side is a bounded list,
   so a decoded manifest sits inside the access limit before the pool sees it.
2. **`manifest_derive.rs`.** Derivation from a Blueprint action. The action
   names its collections and its key expressions. Derivation evaluates the
   expressions against the transaction parameters at admission time.
3. **`manifest_guard.rs`.** Enforcement during execution. An access outside the
   manifest aborts that transaction and holds the abort for every later access.

## 3. The Execution Engine

1. **`execution/partition.rs`.** A disjoint set carries the connected
   components over the pending transactions. An edge joins two transactions
   where a write of one meets a write or a read of the other.
2. **`execution/ordering.rs`.** Class descending, then arrival sequence
   ascending, then transaction hash ascending.
3. **`execution/runner.rs`.** Worker threads pull domains from one shared
   counter. Each domain writes into its own overlay. The merge applies the
   overlays in domain order.
4. **`execution/roots.rs`.** One Merkle root per domain plus a combined root,
   both carried in a block header digest item.
5. **`execution/meter.rs`.** Hard limits on computation, memory and storage per
   transaction. A crossing aborts that transaction alone.

## 4. The Authoring Engine

1. **`authoring/draw.rs`.** Every eligible validator evaluates the sr25519
   verifiable random function over the slot number and the slot seed. The proof
   travels with the claim.
2. **`authoring/committee.rs`.** Standing scales each draw output. The five
   lowest scaled values form the committee in rank order.
3. **`authoring/fallback.rs`.** Rank one holds the slot for the rank timeout. A
   slot still unclaimed passes to rank two.
4. **`authoring/seal.rs`.** The author signs the pre-seal header hash and the
   seal claim with its ed25519 validator key from the node keystore.
5. **`authoring/verify.rs`.** Import checks committee membership, the seal
   signature and the rank timing.
6. **`authoring/gate.rs`.** Production begins only where the local keystore
   holds the scheduled author key for the slot at the claimed rank.

## 5. The Commit Engine

1. **`commit/collect.rs`.** The author gathers one signature per validator
   during the slot window.
2. **`commit/aggregate.rs`.** One envelope carries a signer bitmap and the
   signatures in position order. No public key travels on the wire.
3. **`commit/commit.rs`.** A domain commits once the aggregated Trust Score
   weight crosses the threshold for the highest class that domain carries.
4. **`commit/submission.rs`.** A maximum signature count and a byte limit hold
   before pool admission. The declared weight rises with the count.
5. **`commit/gossip.rs`.** The set of seen messages carries a capacity limit
   with least recently used eviction.

## 6. Identifiers

| Name | Value | Carries |
|---|---|---|
| `CERULEA_ENGINE_ID` | `ceru` | The block seal and the state root digest item |
| `CERULEA_COMMIT_ENGINE_ID` | `cerf` | The Commit Engine messages |
| `CERULEA_KEY_TYPE` | `cerk` | The validator keys in the node keystore |

## 7. Running the tests

```
cargo test --release -p cerulea-consensus
cargo fmt -p cerulea-consensus
cargo clippy --release -p cerulea-consensus --no-deps -- -D warnings
```

Release profile only. Debug and release share no cache.
