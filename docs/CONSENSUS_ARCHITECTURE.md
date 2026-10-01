# The Dynamic Consensus Framework

The Dynamic Consensus Framework is the agreement mechanism of the Cerulea
Blockchain. The framework separates three decisions that other designs bundle
together. Who may author a block. Who authors the next block. When a block
becomes irreversible.

Eight engines carry the work. One shared measure, the Trust Score, joins them.

| Engine | Responsibility | Location |
|---|---|---|
| Commit Engine | Reaches agreement and makes blocks irreversible | Consensus crate |
| Authoring Engine | Selects the slot committee and the block author | Consensus crate |
| Execution Engine | Groups transactions into domains and runs them in parallel | Consensus crate |
| Assurance Engine | Applies the threshold appropriate to each transaction class | Framework pallet |
| Custody Engine | Issues history challenges and scores the responses | Custody pallet |
| Standing Engine | Computes and maintains the Trust Score | Framework pallet |
| Admission Engine | Governs entry to and exit from the validator set | Validator registry pallet |
| Amendment Engine | Governs changes to the seven engines above | Framework pallet |

## 1 The design principle

Cerulea holds one principle above the rest. A transaction declares the state it
reads and writes before execution. That declaration is the Access Manifest.
Cerulea derives the manifest from the Blueprint automatically, so no person
writes one.

The manifest makes three things possible. Transactions that touch no shared
state run at the same time. A group of transactions reaches irreversibility on
its own schedule. The share of validator weight a transaction needs varies with
what the transaction does.

## 2 The Access Manifest

An Access Manifest names the collections a transaction reads, the collections it
writes, the span of keys inside each collection, and the class of the
transaction. A collection is one named store of records inside a Blueprint.

Derivation reads the Blueprint action definition, which names its collections
and its key expressions. The key expressions evaluate against the transaction
parameters at admission time. Enforcement compares every state access against
the manifest during execution. An access outside the manifest ends that
transaction and leaves the rest of the block untouched.

## 3 The Execution Engine

The Execution Engine builds a graph over the pending transactions. An edge joins
two transactions wherever their manifests intersect on a written collection and
an overlapping key span. The connected components become domains. A disjoint-set
structure keeps the pass proportional to the transaction count.

Ordering inside a domain runs by class descending, then arrival sequence, then
transaction hash ascending. Every validator reproduces the same order, so every
validator reproduces the same result.

Domains execute in parallel across cores. Each domain produces its own state
root. The block header carries the combined root.

## 4 The Authoring Engine

Each slot, every eligible validator evaluates a verifiable random function over
the slot seed and its own public key. A verifiable random function produces a
value that anyone checks and nobody predicts. The slot seed comes from the
parent block hash and the slot number, so every node derives the same seed.

The engine scales each threshold by the Trust Score of the validator, so higher
standing wins more often while every eligible validator keeps a real chance. The
five lowest outputs form the committee in rank order.

Rank one authors. A slot unclaimed after the rank timeout passes to rank two,
then onward. The timeout is a policy value.

The seal is a signature over the block header by the author validator key, taken
from the node keystore. Import verifies three conditions. The named author sits
in the committee for that slot. The seal verifies against that author public
key. The rank claimed matches the timing.

Production begins only where the local validator key matches the scheduled
author at the claimed rank.

## 5 The Commit Engine

Validators sign the block with an aggregate signature scheme. The author
collects signatures during the slot, combines them into one signature of fixed
size, and includes the aggregate and the signer bitmap in the block.

A domain commits when the aggregated Trust Score weight crosses the threshold
for the highest class present in that domain. One round settles a domain.

### Commit weight

The commit weight of a validator is its Trust Score. The Standing Engine
composes that score from its components. Stake enters no part of the weight.
Stake stays an admission input, where the design names resource cost.

1. **Epoch freeze.** At every epoch boundary the framework module hands each
   active validator to the validator registry with its Trust Score. The Trust
   Score reads as a share of one, counted in parts per billion. The registry
   gives each validator the share of the epoch Trust Score total it holds,
   scaled to a fixed commit scale of 32,000. A set whose Trust Score total reads
   zero carries an even split, so a chain with no recorded standing still
   reaches agreement.
2. **Genesis.** Every genesis validator starts under the same Trust Score
   share of eight tenths. The commit weights therefore start even. Service,
   performance, custody and sampling move them from the first epoch boundary.
3. **Node registry.** The node records the Trust Score of each active validator
   beside its key pair. The Commit Engine tally opens over those weights, so the
   node and the chain read one figure.
4. **Slot committee.** The seat value of each validator scales by the same Trust
   Score, so authoring and commitment answer to one measure.
5. **Readers.** The epoch weight interface answers the frozen Trust Score share
   of each validator. Cerulea Explorer reads that answer on its validator pages.

Two validators bonding the same stake separate on standing alone. The validator
with the higher Trust Score carries the larger commit weight. Each class
threshold below resolves against the epoch Trust Score total.

Vote submission carries a maximum count enforced before pool admission. The
declared weight scales with the vote count. The propagation message set carries
a capacity limit with least-recently-used eviction, so memory stays bounded
under load.

## 6 The Assurance Engine

The threshold required varies with what the transaction does. Cerulea settles a
warehouse scan in a fraction of the time a treasury movement takes, on one
chain, under one framework.

| Class | Covers | Share of weight | Delay |
|---|---|---|---|
| Routine | Scans, readings, log entries | 51 percent | none |
| Financial | Transfers, settlements, payments | 67 percent | none |
| Structural | Schema, permission and validator set changes | 80 percent | none |
| Constitutional | Policy amendment, upgrade authority | 90 percent | mandatory window |

Class assignment comes from the Blueprint action definition. A domain carrying
mixed classes commits at the highest threshold present. The required weight
rounds upward, so two thirds of ten validators is seven rather than six.

A constitutional change commits and then waits. The window runs one hundred
blocks in the starting profile. The chain releases the change at the first block
at or after the effective block, with no person acting. A release inside the
window fails and names the blocks remaining.

The table above is the profile a deployment starts from. The Amendment Engine
governs every later change. A table that fails inspection never reaches storage.
Inspection refuses a share at or below half the weight, a share above ninety
nine percent, a stricter class sitting at or below a lighter class, and a
constitutional class carrying no window.

## 7 The Custody Engine

Cerulea verifies continuously that every validator holds the complete history
and serves that history on demand. The mechanism runs automatically for as long
as the network produces blocks. No person is involved at any point.

1. **Issue.** At a set block interval the engine mixes a domain separator, the
   parent block hash and the round number. The result selects one validator and
   one historical record. Every node derives the same value from state every
   node already holds, so every node agrees on the challenge without
   coordination.
2. **Publish.** The challenge appears in the block header as a digest item under
   the identifier `cecu`. Every node sees the challenge at the same moment.
3. **Answer.** The challenged validator reads the record from its local store,
   builds the inclusion proof and submits both. An inclusion proof is a short
   chain of hashes showing the record belongs to a committed block.
4. **Verify.** The chain hashes the record, walks the chain of hashes upward and
   compares the result against the record root it already holds for that block.
5. **Score.** A correct proof inside the deadline raises standing. A missed
   deadline lowers standing. An incorrect proof lowers standing sharply and
   records a fault.

The deadline defaults to three blocks, which closes faster than a network fetch
and a proof rebuild complete. Selection spans the full history from genesis, so
retaining a subset fails over time. Challenge frequency is a policy value.

Every block carries a record root over the records that block committed.
Separate domain separators keep a leaf hash apart from a node hash, and the
position of a record enters its leaf, so no rearranged proof passes.

## 8 The Standing Engine

One composite measure drives four engines. The Trust Score. Nothing else governs
validator behaviour.

| Component | Public-access chain | Private chain |
|---|---|---|
| Standing accumulated through service | 40 percent | absent |
| Role weight | absent | 35 percent |
| Credential attestation | absent | 25 percent |
| Measured operational performance | 30 percent | 25 percent |
| Custody proof | 20 percent | 15 percent |
| Verification sampling | 10 percent | absent |

1. **Accumulation.** A new validator enters at zero and rises through proven
   custody, correct authoring, timely voting and accurate verification sampling.
   The ramp runs 180 epochs in the starting profile. An epoch counts once, and
   only where every service condition held. Money buys licences and machines
   quickly. Time cannot be bought.
2. **Verification sampling.** Validators re-execute a randomly drawn sample of
   recent transactions and report the results. The draw reads the block hash and
   the round number, so no validator picks the transactions it prefers.
   Agreement raises standing. Catching a genuine fault raises standing sharply,
   at five times the gain of an agreement.
3. **Decay.** Standing falls slowly during inactivity. Two idle epochs cost
   nothing. Every idle epoch past that removes five percent of the remaining
   standing. A validator holding a seat without answering challenges or signing
   blocks drifts below the standing floor and loses the seat.
4. **Reset.** Proven misconduct returns standing to zero. Rebuilding takes the
   months that accumulation took. This is the equivalent of a financial penalty,
   with time as the value destroyed.

## 9 The Admission Engine

Admission runs continuously rather than per block. A validator holds a seat
while meeting every condition. Losing a condition triggers demotion at the next
epoch boundary. An epoch is a fixed span of blocks after which the active set
may change.

1. **Public-access admission.** A registrar confirmed the legal identity. The
   Cerulea validator licence is active and carries the public-access class. The
   validator proved storage of the full history. The machine answers challenges
   within deadline.
2. **Private admission.** Governance admits every validator. The deploying
   organisation sets the approvals and the credentials. Identity verification
   through a public registrar plays no part, since the organisation already
   knows every participant.
3. **Licence verification.** The control plane issues a signed licence
   attestation. The engine verifies the attestation on chain. No external call
   occurs during block production, so an outage at the control plane never
   stalls the Blockchain. A nonce keeps a replayed attestation from reviving a
   licence the control plane withdrew.
4. **Exit conditions.** Standing below the floor, uptime below the floor,
   participation below the floor, a lapsed licence, or proven misconduct.
   Misconduct that breaks safety removes the seat at once. Every other condition
   waits for the epoch boundary, so the active set stays stable inside an epoch.

## 10 The Amendment Engine and the profile solver

Seven consensus properties genuinely trade against one another. Cerulea exposes
the outcomes and solves for the properties. The customer declares what matters
and never touches a parameter.

### 10.1 The seven properties that trade

1. **Fault tolerance.** The share of validators that may fail while safety
   holds. Raising tolerance raises the quorum, which raises latency.
2. **Finality latency.** Time from submission to irreversibility. Lowering
   latency lowers the quorum or the set size, which lowers tolerance.
3. **Validator set size.** More validators spread trust wider and raise message
   volume, latency and operating cost.
4. **Availability under partition.** A network split forces a choice between
   continuing to serve and halting to stay consistent.
5. **Throughput.** Raising throughput raises state growth, storage cost and the
   hardware floor for participation.
6. **Ordering fairness.** Guaranteeing arrival-order commitment costs latency.
   Author discretion is faster and permits reordering for advantage.
7. **History retention.** Full verifiable history costs storage on every node.
   Pruning lowers cost and weakens long-term auditability.

### 10.2 The limits that stay fixed

Some parts are hard limits rather than trades. No control ever lets a customer
cross one. Every candidate profile passes these checks before presentation, so
an invalid point never surfaces.

1. **The fault bound.** Safety holds while the total participant count exceeds
   three times the faulty count.
2. **The timing assumption.** No deterministic protocol guarantees termination
   in a fully asynchronous network holding even one faulty participant. Cerulea
   therefore assumes a bound on message delay that eventually holds, and states
   the assumption in the code. Safety holds at every moment. Progress resumes
   once delay settles under that bound.
3. **The partition choice.** A network split forces a choice between continuing
   to serve and halting to stay consistent. The choice is a proven limit that no
   design escapes. A profile states which side it takes rather than claiming
   both.

### 10.3 The solver

1. **Declare.** Five questions in ordinary language. Whether instant
   confirmation or maximum certainty matters more. How many organisations run
   infrastructure. What a single wrong record would cost. Whether the system
   must keep serving during a network split. How long records must remain
   provable.
2. **Search.** The solver searches the parameter space for points satisfying the
   declarations, checking each against the fixed limits.
3. **Present.** Three to five valid profiles appear with plain-language
   consequences. Every number is stated as a consequence rather than a setting.
4. **Show the trade.** Moving toward one goal shows immediately what recedes,
   with the reason written out.

The solver is a pure function, so the same call answers a chain query and an
off-chain question. A customer sees the profiles the chain would accept.

### 10.4 The eight policies

A deployment profile is expressed as eight policies. The Amendment Engine
governs changes to the first seven. The eighth governs itself.

| Policy | Governs |
|---|---|
| Admission | Identity requirement, sponsoring authority, credentials, licence class |
| Rotation | Epoch length, set size, rotation trigger |
| Authoring | Trust Score mix and the draw threshold curve |
| Assurance | The class table and the threshold per class |
| Eligibility | Uptime floor, participation floor, standing floor |
| Penalty | Offence catalogue, severity ladder, demotion conditions |
| Reward | Distribution formula, including throughput quota and priority |
| Amendment | Authority, quorum and delay governing changes to the seven above |

Three conditions hold for every change. The account opening the amendment holds
the authority the Amendment policy names. The approvals reach the quorum that
policy names. The approved amendment waits the delay that policy names. An
amendment carries the quorum and the delay in force at the moment it opened, so
no change lowers its own bar.

Combining tunable profiles with class thresholds means one deployment carries
several agreement points at once. A logistics scan commits under a fast profile.
A payment in the same system commits under a strict one. That combination is
what makes the framework dynamic in a literal sense.

## 11 The optional inference contribution

Deployments running Cerulea Intelligence on their own validators may enable an
additional scoring contribution based on served inference work. Custody proof
remains the default for every other deployment.

The contribution stays absent unless three conditions hold at once.

1. **The build carries the module.** A cargo feature turns the module on. The
   feature stays off by default.
2. **Determinism is proven.** Two validators running the same task produce
   identical bytes. That needs pinned weights, a fixed seed, fixed-point
   arithmetic and a pinned runtime.
3. **The work is real.** Tasks come from Cerulea Intelligence workload. A
   generated task carries no contribution.

A result is accepted provisionally. Any validator challenges inside a window. A
challenge triggers redundant execution by a randomly drawn committee, and the
losing side carries the penalty.

The contribution reports a share of one, counted in parts per billion, the way
every Standing Engine component reports. The inference module answers that
share. The framework module reads it as a share with a ceiling of one. A raw
result above the ceiling holds at the ceiling. The module then raises the fault
count of that validator and reports the event, so an operator finds a validator
reporting figures the share cannot carry.

## 12 Where each engine lives

```
chain/cerulea-node/src/cerulea-consensus/src/engines/
  manifest.rs, manifest_derive.rs, manifest_guard.rs
  execution/    partition, ordering, runner, roots, meter
  authoring/    draw, committee, fallback, seal, verify, gate
  commit/       collect, aggregate, commit, submission, gossip

chain/cerulea-pallets/pallet-cerulea-dcf/src/engines/
  assurance/    classes, threshold, delay
  standing/     score, accumulate, sampling, decay, reset
  amendment/    policies, solver, limits, amend

chain/cerulea-pallets/pallet-cerulea-dvf/src/engines/
  admission/    public, private, licence, exit

chain/cerulea-pallets/pallet-cerulea-custody/src/
  primitives, issue, publish, respond, verify, score
```

## 13 Throughput targets

Every figure below is a design target. Published claims follow measurement
rather than design.

| Measure | Target |
|---|---|
| Sustained throughput, private deployment, sixteen cores | 20,000 to 30,000 transactions per second |
| Routine class finality | Under 400 milliseconds |
| Financial class finality | Under 800 milliseconds |
| Validator set at sub-second finality | 200 validators |

## 14 The assertions the tests hold

1. A forged seal is rejected on import.
2. A node whose local key differs from the scheduled author produces no block.
3. An unclaimed slot passes to rank two within the timeout.
4. Two disjoint domains execute in parallel and produce identical roots on every
   node.
5. A routine transaction finalises before a constitutional transaction in the
   same block.
6. A constitutional change waits the full delay window.
7. A custody challenge issues, is answered and moves standing, with no human
   involvement.
8. An unanswered custody challenge lowers standing.
9. A vote submission above the maximum count is rejected before pool admission.
10. The message set evicts at capacity and memory stays bounded.
11. No two conflicting blocks finalise at the same height under any test.
12. A solver profile that would cross a fixed limit never appears in the result
    set.

Defect closure is recorded in `DEFECT_CLOSURE.md`.
