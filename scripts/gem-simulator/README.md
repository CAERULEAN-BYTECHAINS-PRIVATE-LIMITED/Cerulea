# CBC-PRAMAAN GeM Simulator

Generates realistic Government e-Marketplace procurement records and drives the six
trigger-point API routes across all twelve decision pathways, asserting that the chain
decided what each pathway says it must decide.

This is the artefact behind the PoC document's "12 of 12 pathways" claim. It is a genuine
test suite, not a demo script: run it against a route set that returns `GREEN` to
everything and **all twelve scenarios fail**.

Technical Implementation Specification Part 8.4.

---

## Quick start

```bash
# 1. chain up (3 validators)
./scripts/start_all.sh

# 2. rules seeded (21 nodal ministries + the DPIIT default)
cd scripts/seed-ministries && npx tsx seed.ts

# 3. web app up (serves /api/trigger/*)
cd apps/web && npm run dev

# 4. this
cd scripts/gem-simulator
npm install
npx tsx simulate.ts --all
```

Exit code is `0` when every scenario passes, `1` when any fails, `2` when the run could
not start (bad arguments, or the API was unreachable).

## CLI

| Flag | Meaning |
|---|---|
| `--all` | Run all twelve pathway scenarios and print a summary table. |
| `--scenario=P7` | Run one pathway. Repeatable, or comma-separated: `--scenario=P8,P9`. |
| `--api=URL` | Base URL of the Next.js app. Default `http://localhost:3000`. |
| `--seed=N` | Seed for record generation. Same seed, same records. Default `20260406`. |
| `--run-id=TOKEN` | Disambiguates certificate ids between runs. Default: clock-derived. |
| `--as-of=YYYY-MM-DD` | The simulated "today" that bid dates hang off. Default `2026-04-06`. |
| `--timeout=MS` | Per-request timeout. Default `20000` (routes budget 10 s for finality). |
| `--dry-run` | Print the full request sequence and every expectation without calling anything. |
| `--show-records` | Print each generated GeM bid document and seller profile in full. |
| `--json` | Machine-readable summary on stdout instead of the table. |
| `--no-colour` | Disable ANSI colour. |

```bash
npx tsx simulate.ts --scenario=P10 --show-records
npx tsx simulate.ts --all --dry-run > demo-script.txt     # readable run sheet, no services needed
npx tsx simulate.ts --all --seed=42 --run-id=finals --json
```

### Determinism

Nothing in record generation calls `Math.random()` or reads the wall clock. Every bid
number, GSTIN, Udyam number, quantity, EMD figure and date is a pure function of
`--seed`, the pathway id, and `--as-of`, so a demo rehearsed on Monday shows the same
bid numbers on Friday. Each scenario draws from its own generator derived from
`(seed, pathwayId)`, so `--scenario=P7` produces exactly the records that `--all`
produces for P7.

The single exception is the certificate id, which carries `--run-id`: the certification
pallet rejects a repeated `certificate_id` with `CertificateIdAlreadyUsed`, so a
seed-only id would make the second run of a demo fail. Pass `--run-id` explicitly to pin
a whole run byte-for-byte.

### Re-runnability

Every scenario cleans up after itself. P3 restores the DST rule it changed (and proves
the restore took effect with a fourth call); P12 lifts both debarments it recorded and
re-evaluates the previously-blocked bid to confirm the vendor is clear. You can run
`--all` back to back without resetting the chain.

---

## The twelve scenarios

Each scenario carries its pathway's definition verbatim from
`pramaan-primitives/src/lib.rs`, and each step carries a `why` on every assertion. Run
`--dry-run` to read the whole sequence without any services running.

| Pathway | Scenario | Ministry | What it proves |
|---|---|---|---|
| **P1** | Standard formula and the three classification boundaries | DPIIT (Split Air Conditioner, HSN 8415) | The boundaries are inclusive at the lower end. **5000 bps is Class-I, 2000 bps is Class-II and not Non-local, 1999 bps is Non-local.** Three declarations one basis point apart on one bid. |
| **P2** | Component-level / weighted-module | MeitY (Desktop Computer, HSN 8471) | The aggregate is a *weighted* average. Five components averaging 5100 bps unweighted weight to **4325 bps — Class-II, not Class-I**. A second set weights to 5625 bps and is Class-I. A weight set totalling 9500 bps is **refused, never normalised**. |
| **P3** | No automatable formula | DST (LIMS software licence, HSN 8523) | A rule change is a storage update, not a redeploy. DST is switched to `Custom` by API call, the software bid returns **`ManualReviewRequired`** instead of a fabricated percentage, the rule is restored, and a fourth call proves the restore took. |
| **P4** | PLI deeming | DoT (Cellular Mobile Handset, HSN 8517) | The **same 80% declaration by the same vendor** is Class-I with no PLI incentive and **Class-II with one**. The two requests differ in exactly one field. |
| **P5** | Sub Rs 200 crore domestic restriction | Ministry of Steel (TMT Bar, HSN 7214) | The limit is a strict `>`. **Exactly Rs 200 crore proceeds; Rs 200 crore and one paisa is refused** without an approved GTE, and proceeds with one. |
| **P6** | Non-local only under an approved GTE | MoPNG (Seamless Line Pipe, HSN 7304) | A Non-local bid is **removed from the ranking with P6 recorded**, not merely ranked last. A Non-local bid Rs 60 lakh cheaper loses to a Class-I bid; an all-Non-local field is refused outright; the same field under a GTE evaluates normally. |
| **P7** | Para 3A | Dept of Pharmaceuticals (Patient Monitor, HSN 9018) | On a Para 3A item **the cheapest bid does not win if it is not Class-I**. The Class-II bid at Rs 3.2 cr is excluded with P7 recorded and the Class-I bid at Rs 3.8 cr takes the contract. A field with no Class-I bid at all is refused. |
| **P8** | Divisible award | MNRE (Solar PV Module, HSN 8541) | L1 is Class-II at Rs 8 cr. The Class-I bid at Rs 9.5 cr is **inside the 20% band and takes 50% at L1's price**; the one at Rs 10.2 cr is outside and gets nothing. |
| **P9** | Non-divisible award | MoHUA (CBTC Signalling, HSN 8530) | The **identical price shape** that splits 50/50 under P8 awards **100% under P9**, and the Class-II L1 — who keeps half under P8 — keeps nothing. The engine reads divisibility from the ministry rule, not from the bid. |
| **P10** | MSE overlay | Ministry of Textiles (Cotton Bed Sheet, HSN 6302) | The MSE band is **15%, a different number from the 20% Class-I preference margin**. L1 takes 75%, the MSE at Rs 2.25 cr takes 25% at L1's price, and the MSE at Rs 2.40 cr — sitting *exactly* on the 20% Class-I band edge — gets nothing. |
| **P11** | Certification obligation | Ministry of Railways (Cast Steel Bogie, HSN 8607) | Self-certification at **Rs 9,99,99,999 is accepted**; **exactly Rs 10 crore is refused without an auditor** (the comparison is `>=`, not `>`); with the auditor supplied the certificate issues. |
| **P12** | False declaration → debarment | MoCA → MoR, MoD → MoT | The PoC's own **86% vs 30% case** is detected across two tenders for the same product. The debarment that follows is enforced **cross-ministry**: Sabarmati Systems, debarred by Civil Aviation, is blocked at Railways; **Chambal Devices, debarred by the Ministry of Defence, is blocked on a textiles tender** (PoC Figure 29, verbatim). Both are then lifted and the blocked bid re-evaluates clean. |

### The canonical cases the build brief called out

| Case | Where |
|---|---|
| 50% / 20% / 19.99% boundaries, inclusive at the lower end | **P1**, three steps |
| 86% vs 30% same-product inconsistency (`8600` vs `3000` bps) | **P12**, step 2 |
| A debarred vendor blocked at bid evaluation | **P12**, steps 4 and 6 |
| Above-Rs-10-crore certification requiring an auditor | **P11**, steps 2 and 3 |

---

## What the simulator asserts, and one thing it asserts beyond the route table

Every check is declarative and carries a reason (see `checks.ts`). The kinds are:
HTTP status; the tri-state `result`; the `class`; a named field's value; a monetary field
in paise; a basis-point field; a boolean; a **refusal that names the rule that caused it**;
`txRef` + `blockNumber` both present (proof the answer came after DCF finality); the
response mentioning a given term; and a **per-vendor preference outcome**.

Two expectations go slightly beyond the literal route table in
`docs/PRAMAAN_BUILD_CONTRACT.md` section 3. Both are deliberate and both are needed for
the PoC's claims to be evidenced rather than asserted:

1. **`/api/trigger/preference-calculation` must return a per-vendor `outcomes` array** of
   `{ vendor, qualifies, awardedPercentBps, matchedPricePaise, decisionPath }`.
   The contract's `{ qualifies, matchedPrice }` cannot distinguish P8 from P9 — both
   produce a price match at L1's price — so without `decisionPath` there is no way to
   evidence which pathway fired, and "12 of 12 pathways" becomes unfalsifiable. The
   probing in `json.ts` accepts the array under `outcomes`, `vendorOutcomes`,
   `preferenceResults`, `results` or `bidOutcomes`, at the top level or one level inside
   an envelope key.

2. **`/api/trigger/bid-submission` must also call `pramaanConsistency.declare`** when the
   request body carries a `product` field, and surface any flag in the response. The
   route table lists only `classification.classify`, but the PoC document is explicit that
   cross-tender consistency "is not a thirteenth pathway ... It runs across all twelve,
   because a declaration is checked against the vendor's history regardless of which route
   it takes." P12 step 2 asserts the response mentions the inconsistency.

Everything else follows the contract exactly, including the unit conventions: every
monetary field in a request body carries a `Paise` suffix and every percentage carries
`Bps`. No rupee figure is ever sent.

### Request bodies

| Route | Body |
|---|---|
| `bid-submission` | `vendor, tender, ministry, product, declaredLocalContentBps \| components[], isPliManufacturer` + GeM context (`bidNumber, vendorName, itemCategory, hsnCode, buyerOrganisation, isMse`) |
| `bid-evaluation` | `vendor, tender, ministry, declaredLocalContentBps, vendorName, bidNumber` |
| `preference-calculation` | `tender, ministry, tenderValuePaise, isTenderGte, bids[{ vendor, vendorName, class, pricePaise, isMse, isGte }]` |
| `ca-certification` | `certificateId, ministry, vendor, tender, valuePaise, auditor, auditorPersona` |
| `debarment` | `vendor, ministry, action: 'debar'\|'lift', effectiveFrom, effectiveTo, reason` |
| `rule-update` | `ministry, rule{ 9 fields, camelCase, field-for-field with the pallet struct }` |

Presence of `components` on `bid-submission` selects `classifyComponentLevel`; its absence
selects `classify`. `ministry` is sent to `ca-certification` because the threshold lives at
`Rules[ministry].certification_threshold` and there is no other way to read it.

### Vendor identity

`vendor` is an **SS58 address** (generic Substrate prefix 42), encoded locally in
`ss58.ts` — verified against the well-known `//Alice` test vector. Vendors never sign
anything: `classify`, `certify`, `debar` and `calculate_preference` all take the vendor as
data and are submitted by the procuring entity or ministry admin. That means a vendor
identity only has to be a well-formed 32-byte account id, which lets the simulator mint as
many distinct deterministic vendors as a realistic tender needs instead of recycling the
six `//Alice`-style dev accounts. That matters: debarment is enforced vendor-wide and
cross-ministry, so two scenarios sharing one address would contaminate each other.

`auditorPersona: 'auditor'` is sent alongside the auditor's address so a route that
resolves personas rather than addresses still works.

---

## GeM realism

### What was researched for this simulator

Beyond the research already in `docs/PRAMAAN_BUILD_CONTRACT.md` section 4:

**Bid Details field labels**, taken from published GeM bid documents and mirrored
verbatim by `renderBidDocument()`: `Bid Number`, `Dated`, `Bid End Date/Time`,
`Bid Opening Date/Time`, `Bid Offer Validity (From End Date)`, `Ministry/State Name`,
`Department Name`, `Organisation Name`, `Office Name`, `Item Category`, `Total Quantity`,
`Minimum Average Annual Turnover of the bidder`, `Years of Past Experience Required`,
`MSE Exemption for Years of Experience and Turnover`,
`Startup Exemption for Years of Experience and Turnover`, `Type of Bid`,
`Bid to RA enabled`, `Evaluation Method`,
`Time allowed for Technical Clarifications during technical evaluation`, `EMD Amount`,
`ePBG Percentage(%)`, `Duration of ePBG required (Months)`, `MSE Purchase Preference`,
`Make In India (MII)`.

**Buyer Organisation is a four-level hierarchy** on GeM — Ministry/State Name →
Department Name → Organisation Name → Office Name — not a single field. Each of the 21
seeded ministries is given its real buying arms (MeitY → National Informatics Centre →
NIC State Centre Bengaluru; Ministry of Railways → Railway Board → Integral Coach Factory
→ ICF Perambur; and so on).

**EMD is bid security**, conventionally around 1% of estimated value, with buyers able to
set it between 0.5% and 5%; it must stay valid 45 days beyond bid validity. **ePBG** is an
order-time performance guarantee of 3%–10% of order value, with a duration covering
delivery + warranty + a claim period — which is why the generated durations are 14 to 62
months rather than 12. MSEs with a valid Udyam registration and Startups are exempt from
EMD and, where the buyer allows it, from the turnover and experience criteria.

**Evaluation Method** is `Total value wise evaluation` or `Item wise evaluation`;
**Type of Bid** is `Single Packet Bid` or `Two Packet Bid` (the latter runs a separate
technical evaluation before financials are opened). BOQ bids are generated as Two Packet /
Item wise, which is what they are in practice.

**MSE Purchase Preference**: MSE sellers are offered the chance to match L-1 and are
awarded 25% of total quantity, under the Public Procurement Policy for MSEs Order 2012,
validated through the Udyam Registration portal. **This applies to Micro and Small only —
not Medium**, which is why `GemVendor.isMse` is false for the `Medium` band.

**MSME classification revised with effect from 01.04.2025** (Budget 2025): Micro —
investment ≤ Rs 2.5 crore and turnover ≤ Rs 10 crore; Small — ≤ Rs 25 crore / ≤ Rs 100
crore; Medium — ≤ Rs 125 crore / ≤ Rs 500 crore. Generated turnovers sit inside these
bands.

**Real four-digit HSN headings** per ministry category, e.g. 8471 automatic data
processing machines (MeitY), 8517 telephone sets and communication apparatus (DoT), 8607
parts of railway rolling stock (MoR), 8541 photovoltaic cells (MNRE), 8504 electrical
transformers (MoP), 7214 hot-rolled steel bars (Ministry of Steel), 9018 medical
instruments and 3004 medicaments (Dept of Pharmaceuticals), 7308 iron and steel
structures (MoHUA), 6302 bed linen (Ministry of Textiles), 9022 X-ray apparatus (MoCA),
7304 seamless steel tubes (MoPNG), 3105 mineral fertilisers (Dept of Fertilizers).

**GSTIN check digits are real.** `gstinCheckDigit()` implements the published algorithm —
alternate weights of 1 and 2 over the base-36 alphabet, digit-folded, complemented mod 36
— so a generated GSTIN validates if a judge pastes it into a checker. It reproduces the
canonical example `27AAPFU0939F1ZV`. Udyam numbers follow the issued
`UDYAM-<state>-<district>-<7 digits>` format.

**Bid numbers** are `GEM/YYYY/B/NNNNNNN` with serials drawn from 5,800,000–8,499,999,
which is where published 2025 bids actually sit.

### On company names

**No real company is named anywhere in this simulator, deliberately.** Several scenarios
attach debarments and false-declaration findings to their vendors, and doing that to a
real firm's name would be indefensible even in a demo. The generated names follow the PoC
submission document's own idiom — an Indian river or place name plus a sector noun — and
the two vendors the submission itself names by hand, **Sabarmati Systems** and **Chambal
Devices**, are reused verbatim in P12 because those are the document's own worked
examples.

### Sources

- [Government e Marketplace portal](https://gem.gov.in/) and [GeM Buyer FAQs](https://gem.gov.in/userFaqs)
- Published GeM bid documents used for field labels, bid-number format and value ranges:
  [GEM/2025/B/6798497](https://www.indianspices.com/sites/default/files/GeM%20Bid%20&%20ATC.pdf),
  [GEM/2025/B/6563045](https://cdnbbsr.s3waas.gov.in/s3ee8fe9093fbbb687bef15a38facc44d2/uploads/2025/08/202508151797538828.pdf),
  [GEM/2025/B/5895496](https://cdnbbsr.s3waas.gov.in/s3ee8fe9093fbbb687bef15a38facc44d2/uploads/2025/02/20250204954534623.pdf),
  [MeitY / STQC bid GeM-Bidding-7341509](https://www.stqc.gov.in/sites/default/files/tenders/GeM-Bidding-7341509_0.pdf),
  [GEM/2023/B/2949270 (ESIC)](https://esic.gov.in/attachments/tenderfile/2e0d1b8b988e92b34b1a87494c70d239.pdf),
  [NITI Aayog GeM-Bidding-5626382](https://www.niti.gov.in/sites/default/files/2023-11/GeM-Bidding-5626382.pdf),
  [MoHUA bid GEM/2023/B/3513281](https://mohua.gov.in/upload/whatsnew/64899aba253ebBid-No-GEM2023B3513281-dated-05-06-2023-published-on-GeM-Portal.pdf)
- EMD and ePBG rules: [EMD in GeM](https://tendersplus.com/blogs/emd-in-gem-payment-process),
  [ePBG in GeM](https://www.professionalutilities.com/epbg-in-gem),
  [EMD and ePBG](https://www.edafter.com/docs/bid/emd-epbg),
  [EMD exemption for MSME and Startups](https://tendersplus.com/blogs/emd-exemption-for-msme-startups)
- Bid types and evaluation methods:
  [Types of bids on GeM](https://www.bidz365.com/blog/types-of-bids-tenders-on-gem),
  [Evaluation methods on GeM](https://www.bidz365.com/blog/evaluation-methods-on-government-e-marketplace-gem),
  [Single and two packet bids](https://www.edafter.com/blog/single-and-double-packet-bid-on-gem)
- MSME classification revised 2025: [MSME new classification criteria](https://www.indiafilings.com/learn/msme-new-definition)
- HSN headings: [8517](https://busy.in/hsn/sub-chapter-8517/), [8607](https://busy.in/hsn/sub-chapter-8607/),
  [8504](https://www.flexport.com/data/hs-code/8504-electrical-transformers-static-converters-for-example-rectifiers-and-inductors-parts-thereof/),
  [7308](https://www.flexport.com/data/hs-code/7308-structures-excluding-prefabricated-buildings-of-heading-9406-and-parts-of-structures-for-example-bridges-and-bridge-sections-lock-gates-towers-lattice/)

The ministry list itself is **not** duplicated here. `gem-data.ts` reads
`scripts/seed-ministries/ministries.json` — the same file `seed.ts` seeds the chain from —
and refuses to start if it does not hold exactly 21 rows. The simulator can never name a
ministry that is not on chain.

---

## Files

| File | Contents |
|---|---|
| `simulate.ts` | CLI entry: argument parsing, the run loop, the report and the summary table. |
| `scenarios.ts` | The twelve pathway scenarios. Read this file to review the suite. |
| `generate.ts` | Record generation: sellers, bid documents, GSTIN/Udyam/bid numbers, BOQ lines, rule payloads. |
| `gem-data.ts` | Researched reference data: item categories, HSN codes, buyer organisations, EMD/ePBG bands, MSME bands, and the ministry loader. |
| `checks.ts` | The declarative expectation language and its evaluator. |
| `api.ts` | HTTP client for the six routes, with the unreachable-API diagnostic. |
| `json.ts` | Response probing and unit normalisation. |
| `types.ts` | Shared types, mirroring the pallet types. |
| `rng.ts` | Seeded PRNG and the per-scenario derivation. |
| `ss58.ts` | Local SS58 address encoding (base58 + blake2b-512). |

**Zero runtime dependencies.** Node 18+'s global `fetch` and `node:crypto` are all it
uses. `typescript` and `tsx` are dev dependencies only.

## Troubleshooting

**"Cannot reach the CBC-PRAMAAN API at ..."** — the web app is not listening. The error
message lists the three things to check (web app, chain, `--api` URL). The simulator exits
`2` here rather than reporting twelve failures, because an unreachable API is an operator
problem, not a decision-engine result.

**A request times out** — the routes wait for DCF finality with a 10 s budget of their
own. If the validators are still starting up, raise `--timeout`.

**`CertificateIdAlreadyUsed`** — you re-ran with the same `--run-id`. Drop the flag to get
a fresh clock-derived one.

**A scenario fails on a route that is not written yet** — run `--dry-run` to see exactly
what it would have sent and what it expects back. Every expectation prints its reason.

## Verification status

The chain node binary was still compiling and the API routes were being written in
parallel when this was built, so it has not yet been run against the real stack. What has
been verified:

- `npx tsc --noEmit` passes clean under `strict`, `noUncheckedIndexedAccess`,
  `noImplicitOverride` and `noFallthroughCasesInSwitch`.
- `--dry-run --all` renders all 35 steps and 118 expectations across the twelve scenarios.
- The suite was run against a throwaway mock that re-implements the six pallets'
  semantics in JavaScript from their `lib.rs` sources: **12 of 12 pass, 118 of 118
  checks**. This is what confirms the scenarios' arithmetic agrees with the pallets — the
  price bands, the 50/50 and 75/25 splits, the P8-vs-P9 divisibility fork, and the
  boundary comparisons.
- The same suite run against a stub returning `{ result: 'GREEN', qualifies: true, ... }`
  to every request: **12 of 12 fail**. That is the property that makes this a test rather
  than a demo.
- The SS58 encoder reproduces the canonical `//Alice` address; the GSTIN check-digit
  routine reproduces the canonical `27AAPFU0939F1ZV`.
