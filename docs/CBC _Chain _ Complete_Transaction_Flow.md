# CBC Chain — Complete Transaction Flow Documentation
## createTodo Extrinsic: End-to-End Lifecycle

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 1: USER SIGNS & SUBMITS                          │
### │                    (Polkadot.js Apps / any client)                        │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 1 — Building & Signing the Extrinsic
You type "Buy milk" in Polkadot.js and click Submit Transaction.

**Behind the scenes the UI:**

1. **SCALE-encodes the call data:**
   * `callIndex`  = `0x0900` (pallet index 9 (todo), call index 0 (create_todo))
   * `title`      = `0x20 427579206d696c6b` (compact-len + UTF8 bytes)
   * `description`= `0x58 46726f6d207468652067726f636572792073746f7265`
2. **Fetches your account nonce** from `system.account(Alice)`
3. **Creates the signed payload:**
   `payload = { call, nonce, era, tip, spec_version, genesis_hash }`
4. **Signs with Alice's SR25519 private key** → produces a 64-byte signature
5. **Submits via JSON-RPC to your node:**

```json
{ 
  "method": "author_submitExtrinsic", 
  "params": ["0x...signed_bytes..."] 
}
```

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 2: TRANSACTION POOL (MEMPOOL)                    │
### │           cbc-node/src/service.rs  →  sc_transaction_pool                 │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 2 — Mempool Validation & Storage
The RPC call hits `author_submitExtrinsic` which feeds into the `TransactionPool` built in `service.rs`:

```rust
// service.rs line 71-80
let transaction_pool = Arc::from(
    sc_transaction_pool::Builder::new(
        task_manager.spawn_essential_handle(),
        client.clone(),
        config.role.is_authority().into(),  // ← Alice is authority=true
    )
    .with_options(config.transaction_pool.clone())  // 8192 txs, 20MB
    .build(),
);
```

**The pool runs these checks before accepting the tx:**

| Check | What it validates |
| :--- | :--- |
| **Signature** | SR25519 signature mathematically valid |
| **Nonce** | `nonce == current_account_nonce` (prevents replay attacks) |
| **Fee** | Alice has enough balance to pay weight fee (10,000 weight units for `create_todo`) |
| **Mortality** | Transaction hasn't expired (block era check) |
| **Unique hash** | Not a duplicate already in the pool |

If all pass → the extrinsic enters the **Ready queue** of the mempool with priority based on tip. The node also gossips it to peers via libp2p.

At this point the call hash `0x00264c3f...` is known — it's the Blake2-256 hash of the signed extrinsic bytes.

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 3: DCF CONSENSUS — AUTHOR SELECTION              │
### │           cbc-node/src/cbc-consensus/src/dcf.rs                           │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 3 — Who Gets to Produce the Next Block?
Every 6 seconds, the `DcfConsensus::run()` loop fires:

```rust
// dcf.rs line 122-207
loop {
    if self.should_produce_block() {   // ← checks wall-clock ≥ 6s since last block
        let validators = api.get_active_validators(best_hash)?;
        let author = self.select_next_author_from_runtime(&validators)?;
        self.produce_block_with_validation(&author).await?;
    }
    sleep(1000ms).await;
}
```

**Author selection goes through a 3-step waterfall:**

* **Step 1: api.get_expected_author(best_hash, block_number)**
  * ↓ DCF pallet's deterministic schedule (epoch round-robin)
  * ↓ If found → verify it's actually in `active_validators`
  * ↓ Use it ✅
* **Step 2: (fallback) select_author_by_combined_score()**
  * ↓ For each validator: get PoS score + PoI score
  * ↓ `combined_weight` = final_score (from DCF pallet's validator profile)
  * ↓ Weighted random selection: `seed = current_slot × 2654435761 mod total_weight`
  * ↓ Pick the validator whose cumulative weight crosses the target
* **Step 3: (emergency) fallback_author_selection()**
  * ↓ Simple round-robin: `current_slot % validator_count`

**The PoS + PoI score that drives selection is calculated inside the DCF pallet:**
* **PoS (60% weight):** `api.get_validator_stake_score()` → how much stake Alice has bonded
* **PoI (40% weight):** `profile.poi_score` → Alice's inference/participation score from the PoI pallet
* **Calculation:** `final_score = (pos_score × 60 + poi_score × 40) / 100`

Validators with higher combined scores appear in more block slots.

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 4: BLOCK BUILDING                                │
### │           cbc-node/src/cbc-consensus/src/proposer_factory.rs              │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 4 — Block Construction (ProposerFactory)
Once Alice is selected, `produce_block_with_validation()` calls `ProposerFactory::create_complete_block()`:

* **Step 1:** Validate block author with runtime API: `api.validate_block_author(block_number, alice_account_id)` ✅
* **Step 2:** Create inherent data (`inherent_providers.rs`): `timestamp::now()` (current UNIX timestamp) + any other inherents your runtime requires.
* **Step 3:** `api.inherent_extrinsics(parent_hash, inherent_data)`
  * → Runtime converts inherent data into extrinsic bytes
  * → Result: `[timestamp_extrinsic]`
* **Step 4:** `collect_transactions_from_pool()`
  * → `transaction_pool.ready().take(1000)` (up to 1000 txs)
  * → **YOUR createTodo tx is here** ✅
* **Step 5:** `all_extrinsics = [timestamp_inherent] + [your_create_todo_tx]`
* **Step 6:** `build_block_with_state_root()` — the Substrate block builder pipeline:
  * ├─ `api.initialize_block(parent_hash, temp_header)` (runtime sets up state for this block)
  * ├─ `api.apply_extrinsic(timestamp_inherent)` (System clock updated in runtime state)
  * ├─ `api.apply_extrinsic(create_todo_extrinsic)` ← **YOUR TX!** (pallet_todo::create_todo() runs)
  * └─ `api.finalize_block()`
    * → Calculates correct `state_root` (Merkle root of all storage changes)
    * → Calculates `extrinsics_root` (Merkle root of all extrinsics)
    * → Returns final block header

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 5: RUNTIME EXECUTION — pallet_todo               │
### │           cbc-pallets/pallet-todo/src/lib.rs                              │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 5 — Your `create_todo` Executes Inside the WASM Runtime
The WASM runtime (compiled from `cbc-runtime`) executes `pallet_todo::create_todo()`:

```rust
// lib.rs line 161-189
pub fn create_todo(origin, title: BoundedVec<u8, 256>, description: BoundedVec<u8, 1024>) {
    let who = ensure_signed(origin)?;          // ← verifies Alice signed this
    
    // Guard 1: per-account cap
    let count = TodoCount::<T>::get(&who);     // reads from RocksDB trie
    ensure!(count < 500, Error::TooManyTodos); // Alice's current count < 500
    
    // Guard 2: get next ID
    let todo_id = NextTodoId::<T>::get(&who);  // Alice's next ID (0 for first todo)
    let next_id = todo_id.checked_add(1)?;     // overflow check
    
    // Build the struct
    let item = TodoItem {
        id: todo_id,           // = 0
        title,                 // = "Buy milk" as BoundedVec
        description,           // = "From the grocery store"
        completed: false,
        created_at: frame_system::block_number(),  // = block #N
    };
    
    // Write to state (3 storage writes, all go into the block's state trie)
    Todos::<T>::insert(&who, todo_id, item);    // key: Blake2_128Concat(Alice) + Blake2_128Concat(0)
    NextTodoId::<T>::insert(&who, next_id);     // key: Blake2_128Concat(Alice) → 1
    TodoCount::<T>::insert(&who, count + 1);    // key: Blake2_128Concat(Alice) → 1
    
    // Emit event (indexed in the block receipts)
    Self::deposit_event(Event::TodoCreated { owner: alice, todo_id: 0 });
    Ok(())
}
```

All 3 storage writes go into an in-memory overlay during block execution. After `finalize_block()` they get committed to the state trie and the state root is calculated — this root is in the block header.

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 6: BLOCK IMPORT & DCF VALIDATION                 │
### │           cbc-node/src/cbc-consensus/src/import_queue.rs                  │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 6 — DcfImportQueue Verifies the Block
The freshly-built block passes through the `DcfImportQueue` verifier:

```rust
// import_queue.rs  Verifier::verify()
async fn verify(block: BlockImportParams<B>) -> Result<BlockImportParams<B>, String> {
    // Check 1: active validators exist
    let validators = api.get_active_validators(best_hash)?;    // must be non-empty
    
    // Check 2: extract author from block header digest
    let author = self.extract_block_author(&block.header)?;    // reads "cbcc" engine digest
    
    // Check 3: is this the correct author for this block?
    let expected = api.get_expected_author(best_hash, block_number)?;
    if Some(author) != expected {
        metrics.record_author_mismatch();          // Prometheus metric
        api.report_author_mismatch(...);           // emits on-chain event
        return Err("Block author mismatch");       // BLOCK REJECTED ❌
    }
    Ok(block)  // ✅ block accepted into the chain
}
```

```rust
// import_queue.rs  BlockImport::import_block()
async fn import_block(block) {
    // Check DCF finality state
    let last_finalized = api.get_last_finalized_block(best_hash)?;
    block.finalized = block_number <= last_finalized;   // mark if DCF already finalized it
    block.fork_choice = ForkChoiceStrategy::LongestChain;
    block.origin = BlockOrigin::Own;
    Ok(ImportResult::Imported { is_new_best: true })
}
```

---

### ┌──────────────────────────────────────────────────────────────────────────┐
### │                    PHASE 7: DCF FINALITY                                  │
### │           cbc-node/src/service.rs  →  dcf-finality-sync task              │
### └──────────────────────────────────────────────────────────────────────────┘

#### Phase 7 — DCF Finality Sync Confirms the Block
The `dcf-finality-sync` background task (`service.rs` line 311-399) runs every 6 seconds:

```rust
loop {
    sleep(6s).await;
    // Ask DCF pallet: what's the latest finalized block?
    let dcf_finalized = api.get_last_finalized_block(best_hash)?;
    let client_finalized = client.info().finalized_number;
    
    if dcf_finalized > client_finalized {
        // For each newly finalized block:
        for block_num in (client_finalized+1)..=dcf_finalized {
            let hash = client.hash(block_num)?;
            client.finalize_block(hash, None, true)?;  // ← commits to RocksDB permanently!
        }
        log!("DCF Finality Sync: Sync complete. Client finalized head now at #{dcf_finalized}");
    }
}
```

**You see this in your logs:**
```
DCF: Finality advanced from 299 to 300 in epoch 30
DCF: Progressive finalization advanced to block 300
DCF Finality Sync: Sync complete. Client finalized head now at block #300
🏆 Imported #301
```

Finalized means the block can never be reverted — it's permanently part of the canonical chain.

---

### The Complete Flow — One Timeline

| Wall clock | What happens | Your log output |
| :--- | :--- | :--- |
| **T+0.000s** | You click "Submit Transaction" in Polkadot.js | |
| **T+0.001s** | UI signs with Alice's SR25519 key | |
| **T+0.001s** | JSON-RPC: `author_submitExtrinsic(0x...)` | |
| **T+0.002s** | TransactionPool validates: ✅ signature OK, ✅ nonce OK, ✅ fee OK, ✅ unique | |
| | TX sits in mempool "Ready" queue | |
| **T+0.0-6s** | (waiting for next block slot) | |
| **T+6.000s** | `DcfConsensus::should_produce_block()` → true | |
| **T+6.001s** | `select_next_author_from_runtime()` → Alice | "Producing block #N with Alice..." |
| | `validate_block_author()` ✅ | |
| **T+6.002s** | `ProposerFactory::create_complete_block()` | |
| | → inherent_extrinsics: [timestamp] | |
| | → collect_transactions_from_pool: [createTodo] | |
| **T+6.003s** | `api.initialize_block()` (WASM runtime activated) | |
| **T+6.004s** | `api.apply_extrinsic(timestamp)` (timestamp written to state) | |
| **T+6.005s** | `api.apply_extrinsic(createTodo)` ← **YOUR TX EXECUTES** | |
| | → ensure_signed ✅, TodoCount < 500 ✅, TodoId = 0 | |
| | → Todos[(Alice,0)], NextTodoId[Alice], TodoCount[Alice] updated | |
| | → `Event::TodoCreated` emitted | |
| **T+6.006s** | `api.finalize_block()` (state trie committed, state_root calc) | |
| **T+6.007s** | `DcfImportQueue::verify()` (author matches expected ✅) | |
| | `DcfImportQueue::import_block()` (finalized = false, is_new_best = true) | |
| **T+6.008s** | Block #N written to RocksDB | "ProposerFactory: Successfully created #N" |
| | | "🏆 Imported #N" |
| **T+12.000s** | DcfConsensus runs again, produces #N+1 | |
| **T+18.000s** | `dcf-finality-sync` wakes up | |
| | → DCF pallet says: finalized up to #N | |
| | → `client.finalize_block(#N)` | "DCF Finality Sync: Sync complete" |
| **T+18.001s** | Your todo is immutably on-chain ✅ | |

---

### Where is the data physically stored?
`/home/bharat/.cbc-chain-data/chains/cbc_local/db/full/`

```text
│
├── state/           ← RocksDB key-value store
│   │                  Key   = storage_key (TwoX128 + Blake2_128Concat hashes)
│   │                  Value = SCALE-encoded value
│   ├── "c6c53fc2...9dd3..." → TodoItem { id:0, title:"Buy milk", ... }
│   ├── "c6c53fc2...0361..." → NextTodoId[Alice] = 1
│   └── "c6c53fc2...3ec8..." → TodoCount[Alice] = 1
│
├── blockchain/      ← Block headers, bodies, justifications
│   └──  Block #N header: { parent, state_root, extrinsics_root, ... }
│
└── network/
    └── secret_ed25519   ← Your node's permanent identity key
```

The `state_root` in block #N's header is a cryptographic commitment to **ALL** storage values at that point — including your todo. Anyone can prove your todo exists by providing a Merkle proof against that root.