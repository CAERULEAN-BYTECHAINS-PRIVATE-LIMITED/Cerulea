# The Cerulea node interface

The Cerulea node publishes one reading surface. Cerulea Explorer, Cerulea
Dashboard and the Cerulea APIs chain client read the Blockchain through
it. Every method sits in the `cerulea_` namespace, changes nothing and
answers from the block the node holds as best.

A node opens the surface when the Cerulea extensions flag is present at
start. A node started without the flag answers every method below with
error code `-32001` and the sentence naming the fix.

Every reading problem answers with error code `-32010` and one plain
sentence stating the problem and naming the fix.

---

## 1 Shared shapes

Every byte string below travels as a hex string carrying the `0x` marker.
Every name declared in a Blueprint travels as an array of byte values,
which a reader reads as text.

### 1.1 CollectionSummary

| Member | JSON type | Holds |
|---|---|---|
| `name` | array of numbers | The collection name |
| `record_type` | array of numbers | The record type the collection holds |
| `privacy` | string | `Open`, `HashOnly`, `Encrypted` or `External` |
| `records` | number | The count of records the collection holds |
| `root` | array of 32 numbers | The inclusion tree root |
| `version` | number | The version the definition carries |
| `retired` | boolean | A retired collection takes no further record |
| `summary` | array of numbers | One plain sentence about the collection |

### 1.2 RecordReply

| Member | JSON type | Holds |
|---|---|---|
| `collection` | array of numbers | The collection name |
| `key` | array of numbers | The record key |
| `value` | array of numbers or null | The stored bytes, where the class permits |
| `commitment` | array of 32 numbers | The hash of the stored bytes |
| `version` | number | The version the record carries now |
| `created_block` | number | The block the record first landed in |
| `changed_block` | number | The block the record last changed in |
| `privacy` | string | The class of the collection |
| `history` | array of RecordVersionEntry | The version history, oldest first |
| `proof` | array of numbers or null | The canonical encoding of the inclusion proof |
| `root` | array of 32 numbers | The root the proof folds to |

A record in an encrypted collection answers with `value` set to null. The
commitment and the proof stay present, so a reader confirms the record
without reading its content.

### 1.3 RecordVersionEntry

| Member | JSON type | Holds |
|---|---|---|
| `version` | number | The version the write produced |
| `commitment` | array of 32 numbers | The hash of the bytes the write stored |
| `block` | number | The block the write landed in |
| `at_millis` | number | The wall clock time of the write |

### 1.4 BlockDetail

| Member | JSON type | Holds |
|---|---|---|
| `number` | number | The height of the block |
| `hash` | string | The hash of the block |
| `parentHash` | string | The hash of the parent block |
| `stateRoot` | string | The state root of the block |
| `extrinsicsRoot` | string | The extrinsics root of the block |
| `author` | string or null | The seal key the author signed with |
| `slot` | number or null | The slot the block serves |
| `slotRank` | number or null | The rank the author claimed |
| `sealedAt` | number or null | The moment the author sealed the block |
| `committee` | array of CommitteeSeat | The slot committee in rank order |
| `domainRoots` | array of strings | One root per conflict domain |
| `combinedDomainRoot` | string or null | The root over the domain roots |
| `transactionCount` | number | The transactions the block carries |
| `commitWeight` | number | The weight the signers brought |
| `finalised` | boolean | The block reached the threshold |

### 1.5 TransactionDetail

| Member | JSON type | Holds |
|---|---|---|
| `hash` | string | The hash of the transaction |
| `blockNumber` | number | The block the transaction landed in |
| `blockHash` | string | The hash of that block |
| `index` | number | The place inside the block |
| `actor` | string or null | The account the Blockchain records as the actor |
| `action` | string | The module and call the transaction runs |
| `class` | string | `routine`, `financial`, `structural` or `constitutional` |
| `domain` | array of strings | The collections the transaction writes |
| `status` | string | `included` or `finalised` |
| `encoded` | string | The canonical encoding of the transaction |
| `events` | array of strings | The events the transaction raises |

### 1.6 CommitteeSeat

| Member | JSON type | Holds |
|---|---|---|
| `rank` | number | The rank the seat holds, counting from one |
| `validator` | string | The validator account |
| `standing` | number | The seat value after scaling by the Trust Score |

### 1.7 ValidatorDetail

| Member | JSON type | Holds |
|---|---|---|
| `account` | string | The validator account |
| `trustScore` | number | The Trust Score the validator carries |
| `standing` | number | The standing share of the Trust Score |
| `performance` | number | The measured performance share |
| `custody` | number | The custody challenges answered |
| `stake` | number | The stake the validator holds |
| `authoredBlocks` | number | The blocks the validator authored |
| `missedSlots` | number | The slots the validator missed |
| `uptimePercent` | number | The measured uptime |
| `active` | boolean | The validator sits in the active set |
| `custodyAnswered` | number | The challenges answered inside the deadline |
| `custodyMissed` | number | The challenges the validator let pass |
| `custodyFaults` | number | The proofs that failed verification |
| `penalties` | number | The penalties the validator carries |

### 1.8 ValidatorKeyPair

| Member | JSON type | Holds |
|---|---|---|
| `validator` | string | The validator account |
| `drawKey` | string | The sr25519 key the slot draw reads |
| `sealKey` | string | The ed25519 key the block seal reads |

### 1.9 ProposalReply

| Member | JSON type | Holds |
|---|---|---|
| `id` | number | The identifier of the proposal |
| `kind` | array of numbers | `schema`, `permission`, `policy`, `validator_set` or `upgrade` |
| `class` | array of numbers | The class of work the change carries |
| `threshold_percent` | number | The share of the electorate the class needs |
| `payload_hash` | array of 32 numbers | The content hash of the change |
| `status` | array of numbers | `open`, `approved`, `rejected`, `executed` or `cancelled` |
| `opened_at` | number | The wall clock time the proposal opened |
| `deadline` | number | The wall clock time the vote closes |
| `approve_weight` | number | The weight that agrees |
| `reject_weight` | number | The weight that declines |
| `voters` | number | The count of voters that took part |
| `roll_weight` | number | The weight of the whole electorate |
| `decided_at` | number or null | The wall clock time of the decision |
| `executed_block` | number or null | The block the execution landed in |

### 1.10 AuditReply

| Member | JSON type | Holds |
|---|---|---|
| `sequence` | number | The position in the log |
| `event` | array of numbers | The event name |
| `module` | array of numbers | The module that recorded the work |
| `class` | array of numbers | The class of the work |
| `actor` | string | The account the Blockchain records as the actor |
| `at_millis` | number | The wall clock time of the entry |
| `block` | number | The block the entry landed in |
| `reason` | array of numbers | One plain sentence stating why the work ran |
| `payload` | array of numbers | The detail the event carries |
| `corrects` | number or null | The entry this record corrects |
| `digest` | array of 32 numbers | The running digest of the log |

### 1.11 LicenceReply

| Member | JSON type | Holds |
|---|---|---|
| `application` | array of numbers | `dapps`, `enterprise` or `government` |
| `level` | number | The level inside that application |
| `features` | array of arrays | The features the licence enables |
| `addons` | array of arrays | The add-ons the licence enables |
| `seats` | number | The seats the licence covers |
| `seats_used` | number | The seats the deployment uses now |
| `expires_at` | number | The wall clock time the licence lapses |
| `state` | array of numbers | `live`, `lapsed` or `suspended` |
| `suspension_reason` | array of numbers | One plain sentence about the stop |
| `production_permitted` | boolean | Block production runs |

### 1.12 ConsensusProfileReply

| Member | JSON type | Holds |
|---|---|---|
| `routine_percent` | number | The share a routine transaction needs |
| `financial_percent` | number | The share a financial transaction needs |
| `structural_percent` | number | The share a structural transaction needs |
| `constitutional_percent` | number | The share a constitutional transaction needs |
| `committee_size` | number | The seats one slot committee holds |
| `block_time_millis` | number | The span between blocks |
| `epoch_length` | number | The blocks one epoch holds |

---

## 2 The methods

Every method taking `deployment` accepts the content address of the
Blueprint as a hex string. A call leaving the member out reads the first
deployment the chain holds.

### 2.1 Blocks and transactions

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_getBlockDetail` | `blockNumber: number` | BlockDetail |
| `cerulea_getRecentBlocks` | `limit: number`, `before: number or null` | array of BlockDetail |
| `cerulea_getTransaction` | `transactionHash: string` | TransactionDetail |
| `cerulea_getRecentTransactions` | `limit: number`, `beforeBlock: number or null` | array of TransactionDetail |

### 2.2 Records and collections

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_getCollections` | `deployment` | array of CollectionSummary |
| `cerulea_getRecord` | `collection: string`, `key: string`, `deployment` | RecordReply or null |
| `cerulea_getRecords` | `collection: string`, `prefix: string or null`, `limit: number`, `deployment` | array of RecordReply |
| `cerulea_getCollectionRoot` | `collection: string`, `deployment` | string or null |
| `cerulea_verifyRecordProof` | `proof: string`, `root: string` | boolean |

### 2.3 Validators, custody and the consensus profile

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_getValidatorDetail` | `account: string` | ValidatorDetail |
| `cerulea_getValidatorDetails` | none | array of ValidatorDetail |
| `cerulea_getValidatorKeys` | none | array of ValidatorKeyPair |
| `cerulea_getSlotCommittee` | `slot: number` | array of CommitteeSeat |
| `cerulea_getCustodyHistory` | `account: string`, `limit: number or null` | array of CustodyReply |
| `cerulea_getConsensusProfile` | none | ConsensusProfileReply |

### 2.4 Governance

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_listProposals` | `limit: number or null`, `deployment` | array of ProposalReply |
| `cerulea_getProposal` | `proposalId: number`, `deployment` | ProposalReply or null |
| `cerulea_getProposalVotes` | `proposalId: number`, `deployment` | array of VoteReply |
| `cerulea_getExecutedChanges` | `limit: number or null`, `deployment` | array of ProposalReply |

### 2.5 The audit log

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_getAuditLog` | `from: number`, `limit: number`, `deployment` | array of AuditReply |
| `cerulea_getAuditLength` | `deployment` | number |
| `cerulea_getAuditByActor` | `actor: string`, `limit: number`, `deployment` | array of AuditReply |

### 2.6 Permission, views and processes

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_resolvePermission` | `actor: string`, `collection: string`, `action: string`, `deployment` | PermissionReply |
| `cerulea_simulateAsRole` | `role: string`, `collection: string`, `deployment` | RoleReply |
| `cerulea_evaluateView` | `view: string`, `actor: string`, `deployment` | ViewReply |
| `cerulea_getWorkflows` | `deployment` | array of strings |
| `cerulea_getWorkflowInstance` | `workflow: string`, `instance: number`, `deployment` | WorkflowInstanceReply or null |
| `cerulea_getWorkflowInstances` | `workflow: string`, `limit: number`, `deployment` | array of WorkflowInstanceReply |

### 2.7 Tier, evolution and the schema registry

| Method | Parameters | Answers |
|---|---|---|
| `cerulea_getLicenceState` | `deployment` | LicenceReply or null |
| `cerulea_getProductionPermitted` | none | boolean |
| `cerulea_getVersions` | `deployment` | array of VersionReply |
| `cerulea_getRollbackWindows` | `deployment` | array of RollbackWindowReply |
| `cerulea_getSchemaTypes` | `deployment` | array of strings |
| `cerulea_getSchemaType` | `name: string`, `deployment` | string or null |
| `cerulea_getActions` | `deployment` | array of strings |
| `cerulea_getActionManifest` | `action: string`, `deployment` | ActionManifestReply or null |
| `cerulea_getOperators` | `deployment` | array of strings |
| `cerulea_getOracleFeeds` | `deployment` | array of strings |
| `cerulea_getOracleValue` | `feed: string`, `deployment` | OracleReply or null |
| `cerulea_getDeployments` | none | array of strings |

### 2.8 The engine reads present before this range

| Method | Answers |
|---|---|
| `cerulea_getCurrentEpoch` | number |
| `cerulea_getValidatorProfile` | the validator profile |
| `cerulea_getTrustScore` | number |
| `cerulea_listValidators` | array of strings |
| `cerulea_getStatus` | the node status |
| `cerulea_describe` | the node description |
| `cerulea_health` | the node health |
| `cerulea_getBlockAuthoringStats` | the authoring figures of one validator |
| `cerulea_getAllBlockAuthoringStats` | the authoring figures of every validator |
| `cerulea_getFinalizedHead` | number |
| `cerulea_getFinalizedHash` | string |
| `cerulea_isBlockFinalized` | boolean |
| `cerulea_getCurrentRound` | number |
| `cerulea_getAccumulatedWeight` | number |
| `cerulea_getValidatorSetId` | number |
| `cerulea_getValidatorWeights` | array of pairs |

---

## 3 Verifying a record outside the node

A reader confirms a record in two steps.

1. **Read the collection root.** Call `cerulea_getCollectionRoot` for the
   collection, or read the `root` member of a `RecordReply`. That root
   sits in the state trie, so the state root inside the block header
   commits to it.
2. **Fold the proof.** Call `cerulea_verifyRecordProof` with the `proof`
   member of the record and the root. The call folds the sixteen sibling
   digests and compares the result with the root.

A reader holding the proof folds it without the node, since the fold rule
is the one the state module publishes.

---

## 4 Tests

The tests live at `chain/cerulea-node/tests/node_interface_test.rs`. Each
method above carries a test that runs it against a chain built from the
development genesis preset and checks the JSON shape this document states.
