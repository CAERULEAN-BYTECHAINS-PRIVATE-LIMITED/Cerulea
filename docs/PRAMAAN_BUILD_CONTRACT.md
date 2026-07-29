# CBC-PRAMAAN build contract

The single reference for anything built on top of the six `pramaan-pallets/*` crates:
the API routes (spec Part 8.2), the GeM Simulator (Part 8.4), and the frontend (Part 9).

Everything below was read out of the actual pallet source, not from the spec's prose.
Where the two disagree the source wins and the disagreement is flagged.

---

## 1. Conventions that apply everywhere

| Concept | Convention |
|---|---|
| Currency | **paise** (smallest unit) as `u128`. Rs 10 crore = `100_000_000_00`. Rs 5 lakh = `500_000_00`. |
| Percentages | **basis points** as `u16`. `BPS_DENOMINATOR = 10_000`. 50% = `5000`. |
| Ids (`MinistryId`/`TenderId`/`ProductId`/`CertificateId`/`HsnCode`) | `BoundedVec<u8, 64>` — pass as a plain string from JS, `@polkadot/api` encodes it. |
| Time | Block numbers, not timestamps. The runtime has no `pallet_timestamp`. |

Never send a rupee figure to a pallet. Convert at the API boundary and label the field
`...Paise` in any JSON that carries one.

---

## 2. Extrinsic signatures (verbatim from source)

### `pramaanRuleRegistry`
```
setRule(ministry: MinistryId, rule: Rule)            // origin: DPIIT or Root
setDefaultRule(rule: Rule)                           // origin: DPIIT or Root
```

`Rule` has exactly nine fields, in this order:
```
hsnThresholds:        Vec<{ hsnCode, classOneBps, classTwoBps }>   // max 64
para3aApplicable:     bool
pliLinked:            bool
calculationMethod:    'Standard' | 'ComponentLevel' | 'WeightedModule' | 'Custom'
preferenceMarginBps:  u16
certificationThreshold: u128   // paise
exemptionFloor:       u128     // paise
divisibility:         'Divisible' | 'NonDivisible'
effectiveFrom:        BlockNumber
```

### `pramaanClassification`
```
classify(vendor, tender, ministry, declaredLocalContentBps: u16, isPliManufacturer: bool)
classifyComponentLevel(vendor, tender, ministry, components: Vec<{ name, declaredBps, weightBps }>)  // max 64
```
Result stored at `classifications((vendor, tender))`, one of
`ClassOne | ClassTwo | NonLocal | ManualReviewRequired`.

### `pramaanPreference`
```
calculatePreference(tender, ministry, bids: Vec<BidItem>, tenderValue: u128, isTenderGte: bool)  // max 128 bids
```
```
BidItem = { vendor, class: 'ClassOne'|'ClassTwo'|'NonLocal', price: u128, isMse: bool, isGte: bool }
```
Result `PreferenceOutcome { qualifies, matchedPrice: Option<u128>, awardedPercentBps: u16, decisionPath: PathwayId }`.

> **Flagged:** `BidItem.isGte` is carried for display only. The actual P6 eligibility gate
> reads the tender-level `isTenderGte` argument, because GTE approval under GFR Rule
> 161(iv) is a property of the tender, not of a bid. Do not rely on the per-bid field.

### `pramaanCertification`
```
certify(certificateId, ministry, vendor, tender, value: u128, auditor: Option<AccountId>)
```
At or above `rule.certificationThreshold` an `auditor` is **mandatory** and must pass the
runtime's `AuditorRoleSource` check. Below it, `auditor: null` is valid (self-certification).

> **Flagged (demo scope):** the runtime wires `AuditorSource = AnyAccountIsAuditor`, which
> accepts any signed account. There is no role-registry pallet in this build. The frontend
> Role check is the only thing distinguishing an auditor in the demo.

### `pramaanDebarment`
```
debar(vendor, ministry, effectiveFrom: BlockNumber, effectiveTo: Option<BlockNumber>, reason: Bytes)
liftDebarment(vendor, ministry)
```
Enforcement is **cross-ministry**: any active debarment from any ministry blocks the vendor
everywhere (PoC Table 8 reading; both interpretations are covered by unit tests).

### `pramaanConsistency`
```
declare(vendor, product, tender, localContentBps: u16)
```
Emits `InconsistencyFlagged` when a new declaration differs from a prior one for the same
`(vendor, product)` by more than `ToleranceBps` (runtime: 1000 bps = 10 points).
The canonical 86% vs 30% case (`8600` vs `3000`) is far outside tolerance and must flag.

---

## 3. The six API routes (spec Part 8.2)

All are `POST`, all live at `apps/web/src/app/api/trigger/<name>/route.ts`, and all
return **only after DCF finality** (Part 8.3).

| Route | Calls | Request body | Response |
|---|---|---|---|
| `/api/trigger/bid-submission` | `classification.classify` | `vendor, tender, ministry, declaredLocalContentBps` | `{ result, class, reason, txRef, blockNumber }` |
| `/api/trigger/bid-evaluation` | `debarment` read, then `classify` | `vendor, tender, ministry` | `{ result, reason, txRef, blockNumber }` |
| `/api/trigger/preference-calculation` | `preference.calculatePreference` | `tender, bids[]` | `{ qualifies, matchedPrice, txRef, blockNumber }` |
| `/api/trigger/ca-certification` | `certification.certify` | `vendor, tender, value, auditor?` | `{ certificateId, requiresAuditor, txRef, blockNumber }` |
| `/api/trigger/debarment` | `debarment.debar` / `liftDebarment` | `vendor, ministry, action, effectiveFrom, effectiveTo?` | `{ status, txRef, blockNumber }` |
| `/api/trigger/rule-update` | `ruleRegistry.setRule` | `ministry, rule` | `{ status, newVersion, txRef, blockNumber }` |

`result` is the tri-state `GREEN | YELLOW | RED`:

- **GREEN** — compliant and proceeds (`ClassOne`, or a qualifying preference outcome).
- **YELLOW** — proceeds with a caveat or needs a human (`ClassTwo`, `ManualReviewRequired`,
  an above-threshold certification still awaiting its auditor certificate).
- **RED** — blocked (`NonLocal`, an active debarment, a failed eligibility gate).

`reason` is one plain sentence a non-technical judge can read aloud. RED must additionally
name the rule or debarment that caused the block.

### Finality wait (Part 8.3) — identical in all six

1. Submit via `@polkadot/api`, capture the inclusion block hash.
2. Subscribe to `rpc.chain.subscribeFinalizedHeads`.
3. Treat as final once a finalized head's number `>=` the inclusion block's number.
4. Only then read the event back and return.
5. **10 s timeout** → return a clear timeout error, never hang.

Use the shared helper in `src/lib/chain.ts`; do not reimplement per route.

---

## 4. GeM realism (spec Part 8.4 asks for this explicitly)

Researched from live GeM bid documents, not invented:

- **Bid number format:** `GEM/YYYY/B/NNNNNNN` — 7-digit serial, e.g. `GEM/2025/B/6798497`.
- **Real field labels** to mirror in generated records: `Bid Number`, `Bid End Date`,
  `Buyer Organisation` (ministry/department), `Total Quantity`, `Item Category`,
  `MSE Purchase Preference`, `Make In India (MII)`, `Local Content Percentage`,
  `Class I / Class II Local Supplier`, `EMD`, `ePBG`, `Evaluation Method`.
- **Bid types** worth representing: Standard Bid (above Rs 3 lakh), Custom
  Catalogue-Based Bid, BOQ (Bill of Quantities, multi-item), and Bid-to-RA (reverse
  auction). BOQ is what exercises the multi-item weighted-average path.
- Real ministry names come from `scripts/seed-ministries/ministries.json` (21 rows) —
  use those `ministry_id`s, never made-up ones.

Sources: [GeM portal](https://gem.gov.in/), [a real GeM bid document](https://www.indianspices.com/sites/default/files/GeM%20Bid%20&%20ATC.pdf).

---

## 5. Design tokens (spec Part 9.1)

| Token | Value | Use |
|---|---|---|
| `--cerulea-primary` | `#004AAD` | Brand blue |
| `--status-green` | `#1E8E3E` | Compliant — **reserved** |
| `--status-yellow` | `#E8A100` | Caveat / manual review — **reserved** |
| `--status-red` | `#D93025` | Blocked — **reserved** |
| `--ink` | `#1A1A1A` | Body text |

The three status colours carry legal meaning. Never use them for decoration, charts
unrelated to compliance, or hover states.

Motion: every transition under 300 ms. The result reveal is scale `0.95 → 1.0` with an
opacity fade over 200 ms.

---

## 6. Known deviations from the spec

| Spec says | Reality | Why |
|---|---|---|
| Next.js 14 | **Next.js 16.2.12 / React 19** | The repo's `AGENTS.md` mandates reading `node_modules/next/dist/docs/`; 16 is what's current. App Router either way. |
| `construct_runtime!` | `#[frame_support::runtime]` | The macro style this codebase actually uses. |
| Config impls in `lib.rs` | `configs/mod.rs` | Where this codebase actually puts them. |
| Contabo VM deploy | Render.com | User's explicit choice; reuses the existing 3-validator `render.yaml`. |
