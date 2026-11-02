# Adversarial review of the Cerulea chain

Task P10-026 mounts one attack per line of the list below against the real
engine code. An attack is a message or a call a hostile party controls. Each
test asserts that the engine refuses the attack and names the fault. An attack
an engine accepted counted as a defect. We fixed each defect in the engine with
the smallest correct change and kept the test as the regression guard.

Plain sentence. Every row names who stops the attack, the exact refusal a
person reads and the commit that closed any gap.

The suite lives at `cerulea-node/tests/adversarial_test.rs`.

```
cargo test --release -p cerulea-node --test adversarial_test
```

## The attacks

| # | Attack | Engine that refuses it | Fault named | Tests | Fix commit |
|---|---|---|---|---|---|
| 1 | A forged block seal | Authoring Engine, seal check on import | `SealSignatureRejected`, `ValidatorKeysMismatch`, `AuthorOutsideCommittee` | `forged_*` | none needed |
| 2 | A valid seal replayed at a later slot | Authoring Engine, seal check on import | `SealSlotMismatch`, `DrawProofRejected`, `SealSignatureRejected` | `replayed_*` | none needed |
| 3 | A rank three member claiming the slot before the rank one timeout | Authoring Engine, rank timing and arrival check | `RankMismatch`, `RankTimingRejected`, `SealMomentAhead` | `early_rank_*` | `99d28ba` |
| 4 | Vote flooding at and above the submission bound | Commit Engine, submission gate and signature collector | `SubmissionCountAboveLimit`, `SubmissionTooLarge`, `VoterOutsideSet`, `VoteSignatureRejected` | `vote_flood_*` | none needed |
| 5 | Propagation set exhaustion above capacity | Commit Engine, capped message set, and the equivocation memory | eviction at capacity, `MessageAboveBound` | `gossip_exhaustion_*` | none needed |
| 6 | A custody answer carrying a proof for another record | Custody Engine | `RootMismatch`, then the verdict `Incorrect` with one custody fault | `custody_*` | `99d28ba` |
| 7 | Equivocation across two ranks by one author | Authoring Engine, seal check on import and equivocation memory | `RankMismatch`, then an equivocation proof | `equivocation_*` | `99d28ba` |
| 8 | An inbound bridge message carrying a proof issued for another endpoint | Bridge module verifier | `BridgeFault::SignatureInvalid`, `ProofDeclined` | `bridge_*` | none needed |
| 9 | A relayer submission with a tampered authorisation | Relayer module | `SignatureInvalid`, `NonceUsed` | `relayer_*` | none needed |
| 10 | An operator call touching customer state | Operator module with the permission module | `Outcome::PermissionDenied`, `NotAnOperator` | `operator_*` | none needed |
| 11 | A licence attestation signed by a foreign key, and one that has expired | Tier module and Admission Engine | `SignatureInvalid`, `LicenceLapsed`, `LicenceError::SignatureRejected`, `LicenceError::Lapsed` | `licence_*` | `99d28ba` |
| 12 | A Blueprint action whose Access Manifest omits a collection it then writes | Execution Engine, manifest guard | `WriteOutsideManifest` | `manifest_*` | `99d28ba` |
| 13 | A transaction exceeding its metered bound | Execution Engine, resource meter | `ComputationLimitCrossed`, `StorageLimitCrossed` | `transaction_past_*`, `metered_bound_*` | `99d28ba` |
| 14 | A domain commit whose signer bitmap claims a non-member | Commit Engine, aggregate check | `AggregateBitmapMalformed`, `AggregateSignatureRejected` | `domain_commit_*` | none needed |

A placement fault in the hunk staging of the first commits shifted three
insertions. Commit `d5c715c` places them where they belong.

The suite holds 32 tests over the 14 attacks and the review observations
below. Every test passes on the tree that carries the fixes below. No test is
ignored.

## The defects the attack list found and the fixes

1. **A later rank publishing ahead of its window.** Import read the rank
   window against the moment the seal claimed, and the author chooses that
   moment. A rank three member claimed a moment inside its own window and
   published while rank one still held the slot. Import accepted the block.
   The Authoring Engine now carries `verify_seal_on_arrival`. The call refuses
   a claimed moment the local clock has yet to reach, beyond a drift allowance
   of five hundred milliseconds. The allowance sits below one rank timeout, so
   a later rank never reaches into an earlier window. A block fetched long
   after its slot still imports. The import queue runs the call on every
   block. The fault reads `SealMomentAhead`.

2. **A body setting a refusal aside.** The manifest guard refused an access
   outside the Access Manifest and the resource meter refused a charge past
   its bound. The domain runner read only the result the transaction body
   returned. A body that read the refusal as a value and reported success
   kept its earlier writes, and the runner recorded the call as applied. Code
   running as a WebAssembly contract reads a host refusal as a value, so the
   case is real. The runner now reads the verdict the guard holds. A guard
   that aborted aborts the call whatever the body returns, and the writes of
   that call roll back.

3. **A custody answer carrying a proof for another record.** The Custody
   Engine settled the answer as incorrect and then returned a failure. A
   failing call rolls back every change it made, so the fault record vanished
   and the challenge stayed open for a second attempt. The design states that
   an incorrect proof lowers standing sharply and records a fault. The answer
   call now returns success once the verdict lands. The challenge closes as
   incorrect, the custody share falls by the fault loss and the fault count
   rises by one. A second answer to the same challenge finds no open
   challenge.

4. **An expired licence attestation.** The tier module checked the signature,
   the deployment and the sequence of a licence attestation, and installed an
   attestation that had already expired. The installation marked the
   deployment live and cleared its suspension reason. The module now refuses
   an attestation whose expiry has passed, after the signature check. The
   fault reads `LicenceLapsed`.

## Review observations from the security assessment

The security assessment of the protocol left five observations for the chain.
Each one carries a test in the suite.

1. **P-01, every path runs the shared size check.** Six paths relied on field
   limits alone. Each path in `NETWORK_BOUNDS.md` now calls the shared check
   from `cerulea-primitives` before any decoding. The block seal check sits in
   `extract_seal`, so a seal padded past its bound reads as no seal and import
   refuses the header. The commit vote and the propagation message share
   `admit_vote_message`. The equivocation report reads through the shared
   check. The custody answer, the bridge proof and the Blueprint artefact each
   run the check first in their calls. The runtime ties its block length to the
   extrinsic body bound at compile time. Tests: `network_path_*`. Fix commit:
   `99d28ba`.

2. **P-02, a validator sealing at two ranks.** Import refused the header at
   the rank the author lacks and recorded nothing. The refused head now joins
   the equivocation memory once the tracker confirms the recorded seal key and
   the seal signature. The memory keys one head by slot and seal key, and the
   shared evidence check compares slot, seal key and draw key, so the pair
   completes a proof that travels the report path to the framework module.
   Tests: `equivocation_across_two_ranks_is_refused_at_import_and_raises_evidence`.
   Fix commit: `99d28ba`.

3. **P-03, an expired attestation at install.** The tier module refuses the
   attestation with `LicenceLapsed`. The fix sentence reads "The licence
   lapsed. Renew the subscription and install the new licence." Fix commit:
   `99d28ba`.

4. **P-04, the operator module answers success on a refusal.** Every call in a
   module runs inside one storage layer. A call that answers with an error
   rolls back every change it made, the audit record included. Acceptance
   criterion 14 asks that a refused request lands in the record with its
   reason. The call therefore keeps its success answer, stores the request
   record with the outcome and deposits `RequestRefused` naming the outcome.
   A caller reads the refusal from that event. The suite asserts the outcome
   and the unchanged customer record.

5. **P-05, a repeated vote.** The signature collector answers a repeated vote
   from one member with `AlreadyHeld` and counts the member once, so a repeat
   moves no weight. A named refusal for the repeat belongs to the vote lock the
   Trust Score work builds in the vote path. The suite asserts the weight today
   and follows that change when it lands.

## The state module and the Custody Engine

Task P1-029 closes a gap the nine validator network found. Records entered
the custody record tree only through the direct custody call, so a live chain
held no custody record and issued no challenge. The state module now calls the
custody record hook on every insert, update and removal. The hook is an
associated type of the state module settings, the custody module implements
it and every mock takes the empty hook. The entry for one record joins the
collection, the key and the digest of the stored value, so the entry stays
inside the answer bound whatever the record size. The direct custody call now
asks for the root origin, so no account fills the record tree of a block and
crowds out the state records. Tests: `cerulea-node/tests/custody_state_test.rs`.
Fix commit: `b27ded9`.

## The relayer dispatches the authorised call

Task P1-030 closes a second gap the live write pass found. The relayer module
recorded the authoriser and the payload hash, and the write that followed ran
as the relayer, so permission resolved against the relayer. The submission now
carries the inner call. The authorisation names the content hash of that call.
Once the signature holds, the module dispatches the call with the authoriser
as the origin. A call other than the one the authorisation names fails on
`CallMismatch`. Tests: `cerulea-node/tests/relayed_write_test.rs`. Fix commit:
`33bca25`.

## Decisions recorded

1. **The admission licence library.** The Admission Engine attestation check
   now runs on chain through `submit_validator_licence`. A chain that names no
   licence key admits validators through governance alone, which is the
   private chain path. A chain specification for the public-access chain
   names the key.

2. **A repeated header.** The seal check reads one header at a time and holds
   no memory. The block store keys each block on its header hash and keeps one
   copy, so a repeat of one honest block adds nothing.

## Access control on deployment settings

The review found calls that any signed account reached. Task P10-027 closes
each one. The tests sit in `cerulea-node/tests/access_control_test.rs`. Each
test shows a stranger meeting a named refusal and the account holding the
authority succeeding.

Three accounts administer a deployment. The account that installed the
Blueprint, an active Cerulea operator of the deployment and the account whose
public key is the control plane key. The shared interface
`DeploymentStewards` in the schema module answers for every module, and the
runtime joins the three answers.

1. **The control plane key.** The first key binds to the account that
   installed the deployment. A chain specification places the key of a
   genesis deployment. A later key arrives through `rotate_control_plane_key`,
   signed by the current key or carried by the root origin. Suspension and
   resumption come from an account that administers the deployment. Faults:
   `NotInstaller`, `RotationUnsigned`, `SignatureInvalid`, `NotSteward`. Fix
   commit: `ee95e4b`.
2. **The relayer settings.** `register_sponsor`, `set_quota`,
   `set_account_limit`, `set_action_limit`, `throttle` and `lift_throttle`
   come from an account that administers the deployment. Fault: `NotSteward`.
   Fix commit: `1ffacee`.
3. **The operator role.** The root origin or an active operator of the
   deployment appoints and retires an operator. A chain specification appoints
   the first operator. Fault: `NotAnOperator`. Fix commit: `cb1abff`.
4. **The validator licence.** The validator registry module holds the key the
   control plane signs validator licences with. A validator presents its
   attestation through `submit_validator_licence`, which runs the Admission
   Engine check for the signature, the window, the nonce and the class. While
   the key is present, `join_validators` and `join_validator_set` admit only a
   validator whose licence still holds. The root origin installs the key and
   the class. Faults: `LicenceRequired`, `LicenceSignatureRejected`,
   `LicenceLapsed`, `LicenceReplayed`, `LicenceHolderMismatch`. Fix commit:
   `4c24438`.

Controls implemented, certification not held.
