# Ministry data provenance

What `scripts/seed-ministries/ministries.json` and `scripts/seed-ministries/hsn-map.json`
are seeded from, ministry by ministry, so the submission can be checked against the
actual notifications rather than taken on trust.

Every row below is transcribed from the research package
`CBC-PRAMAAN_HSN_and_Rules_Data`. Where the package gives no figure, the DPIIT general
order's default applies and the row says so; **no number in either JSON file is
invented**.

Confidence tiers are the package's own:

| Tier | Meaning |
|---|---|
| `FULL` | Complete item-level list with percentages and/or HSN codes in hand, verified against the primary government notification. |
| `PARTIAL` | At least one real confirmed figure exists, but the full published item list has not been transcribed. |
| `INDEX_ONLY` | The notification's existence, date and item count are confirmed real; the item-level content was not recovered. |
| `EXTERNAL_POLICY` | The ministry runs its own policy outside the DPIIT PPP-MII notified-ministry framework. |
| `DEFAULT_APPLIES` | No ministry-specific notification exists. The DPIIT 50/20 default **is** the correct rule, not a gap. |

Tally across the 21 rows: **FULL 11, PARTIAL 6, INDEX_ONLY 1, EXTERNAL_POLICY 1,
DEFAULT_APPLIES 2.**

## The 21 rows

| # | `ministry_id` / name | Tier | Notification number and date | Source URL | Reverification flag and why |
|---|---|---|---|---|---|
| 1 | `DPIIT` — DPIIT (default; also nodal for cement and paper) | FULL | Cement 5 items and paper 7 items, dated 13.05.2020 and 04.02.2021. Numeric parameters are the general order P-45021/2/2017-PP(BE-II), 15.06.2017, rev. 28.05.2018 / 29.05.2019 / 04.06.2020 / 16.09.2020 / 19.07.2024 | https://dpiit.gov.in/department-promotion-industry-and-internal-trade-0 | No. Also the source of `set_default_rule`. |
| 2 | `MEITY` — Ministry of Electronics and Information Technology | PARTIAL | 33(1)/2017-IPHW 14.09.2017; 33(5)/2017-IPHW 01.08.2018; W-43/4/2019-IPHW 07.09.2020 amended 19.10.2023; a further 1-item notification 27.03.2025 per DPIIT's index | https://www.cmai.asia/pdf/19.3.21%20Meityh%20PPP%20apply%20on%20Mobile%20irrespective%20of%20order%20value.pdf | No. Only the mobile-phone 50 percent is confirmed current; the per-product mechanism table was not fetched, so the DPIIT default governs. |
| 3 | `DOT` — Department of Telecommunications | FULL | No. 18-10/2017-IP, September 2024 (in force per DPIIT index 21.10.2024) | https://www.dot.gov.in/static/uploads/2025/07/d9dc8635bad67a0fbb6e534a98a9eab8.pdf | No. But the code printed `9001000` is 7 digits where HSN codes are normally 8; recorded verbatim and flagged in `hsn-map.json`. |
| 4 | `DHI` — Ministry of Heavy Industries | FULL | Automobiles: order of October 2020 superseding 04/05/2020. Boilers: F.No.16(4)/2018-HEI-(Part-1)(E-15608), 29.04.2025, superseding 29.09.2020 | https://www.dpiit.gov.in/static/uploads/2025/07/74633e5056ad0a88a3daf0c59970a26b.pdf and https://www.dpiit.gov.in/static/uploads/2025/07/bd5d95649befc0e1d56802b136a73e8e.pdf | No. Gap noted: the 29.12.2021 batch of the 101-item total was not located and the component list is a partial reconstruction (31 of ~56). |
| 5 | `MOPNG` — Ministry of Petroleum and Natural Gas | EXTERNAL_POLICY | PP-LC policy, Letter No. O-27011/44/2015-ONG-II/FP, 25.04.2017; further instruments 06.08.2018, 25.06.2019, 07.10.2019, 17.11.2020; revised 26.04.2022 | https://www.business-standard.com/economy/news/dpiit-seeks-removal-of-services-components-in-local-content-calculation-124050201199_1.html | No. Full text of the 2022 revision not fetched; the Rs 1 crore floor and the AMC/CMC inclusion are the two confirmed provisions. |
| 6 | `DCPC` — Department of Chemicals and Petrochemicals | PARTIAL | OM No. C.I.43012/52/2017-Chem-I(B), 13.08.2024, confirming 28 items from the order of 12.02.2021. Baseline superseded: C.I-43012/52/2017-Chem-1(B), 25.05.2018 | https://www.dpiit.gov.in/static/uploads/2025/07/eeabb9b009e06d2d2bfb17bdaa9cea4c.pdf | **YES.** The doubtful field is **the HSN digits**: six codes are OCR-unresolved (Acetaldehyde, Expandable Polystyrene, PET chips, PVC, Lambda Cyhalothrin, Reactive Dyes). Separately the item count is 26 transcribed against 28 notified. Item names and the 50/20 threshold are not in doubt. |
| 7 | `MOHUA` — Ministry of Housing and Urban Affairs | FULL | F.No. K-14011/10/2019-UT-V, 01.01.2021 (64 items, matching DPIIT's index exactly) | https://www.cmai.asia/pdf/1.1.21%20Metro%20Local%20Manufacturing%20List.pdf | No. But see "judgement calls" below — the 60 percent threshold rests on a DMRC tender document, not the ministry letter. |
| 8 | `MOT` — Ministry of Textiles | INDEX_ONLY | Two notifications covering 17 items, dated 01.02.2019 and 23.10.2019. **Notification numbers were not recovered** — only the dates and the item count. | https://dpiit.gov.in/ministry-textiles | **YES.** The 17 item names are genuinely unresolved after three research attempts (direct search, direct fetch, autonomous research agent), most likely because the only prior mirror is at a dead URL. Nothing was invented to fill the gap; the DPIIT default is seeded. |
| 9 | `MOS` — Ministry of Ports, Shipping and Waterways | FULL | No. SY-13017/4/2017-SBR (part 3), 17.09.2021, superseding the Gazette notification of 05.05.2020; predecessor SY-24015/2/2018-SBR, 31.08.2018; separate MLC notification 13.10.2020 | https://shipmin.gov.in/sites/default/files/MII%20Clause%203%20notification.pdf | No. One direct re-fetch is recommended to confirm the 6 item names verbatim (the list came via a research pass; shipmin.gov.in has returned JS-blank pages to automated fetches). No numeric parameter depends on it. |
| 10 | `MOR` — Ministry of Railways | FULL (mechanism) | Railway Board letter No. 2015/RS(G)/779/5(Vol.III), 12.07.2020; base negative list 22.06.2020 (69 items) and 18.09.2020 (40 items); further 24.09.2020, 19.02.2021, 07.07.2021, 20.01.2023; consolidated by OM 07.02.2025; original adoption 03.08.2017; baseline 01.02.2018 | https://www.cmai.asia/pdf/12.7.20%20Railway%20relaxations%20some%20wagons%20mfg.pdf | No. The rule mechanism is confirmed verbatim; the negative-list item names themselves were not fetched, and the registry has no field to hold them. |
| 11 | `MOD-DEFENCE` — Department of Defence (works) | DEFAULT_APPLIES | **None exists.** Absent from DPIIT's master list of 18; a 2019 PIB release names this department as yet to notify | https://www.pib.gov.in/Pressreleaseshare.aspx?PRID=1563772 | No. Verified-negative finding: the DPIIT default is the correct answer. |
| 12 | `DDP` — Department of Defence Production | FULL | No. 59011/8/2015-D(HAL-II), 26.07.2018 (17 items), amended by 18(2)/19/PPO-Notification/DP(Plg-MS) July 2020, expanded 25.08.2020 (24 items under clause 3(a)) | https://www.dpiit.gov.in/static/uploads/2025/07/05301814eb95c7912a20d7ca9a8a918a.pdf and https://www.cmai.asia/pdf/25.8.2020%20MOD%20notification%20of%2024%20items%20under%203%28a%29.pdf | No. 41 of the 46 items DPIIT cites are in hand; the companion notification of 05.10.2020 was not located. |
| 13 | `MOP` — Ministry of Power | PARTIAL | Current: 210 items, 16.11.2021 (located, content not extractable — scanned image PDF). Predecessor transcribed in full: Order No. 11/05/2018-Coord., 28.07.2020 | https://powermin.gov.in/sites/default/files/webform/notices/Public_Procurent_Preference_to_Make_in_India_to_provide_Purchase_Preference_0.pdf and https://dpiit.gov.in/ministry-power | No. The gap is item scope, not any number: this ministry sets no threshold of its own in either version. |
| 14 | `MNRE` — Ministry of New and Renewable Energy | FULL | Order No. 283/22/2019-GRID SOLAR, 09.02.2021, superseding the same-numbered order of 23.09.2020 | https://www.eqmagpro.com/wp-content/uploads/2021/02/file_f-1612877902917.pdf | No. All 78 items across 8 categories obtained; per-category counts reconcile exactly to 78. Annexure-II is empty for this ministry. |
| 15 | `MOCA` — Ministry of Civil Aviation | PARTIAL | No. AV-29013/12/2018-AAI-MOCA (E-128515), 26.05.2020 (41 items per DPIIT's index) | https://civilaviation.gov.in/sites/default/files/Notification%20_001.pdf | **YES.** The doubtful field is **the whole item list**. The Annex-1 table is heavily OCR-degraded and part of it printed mirror-reversed; the ~34 names recovered are a reconstruction, indicative only. Lowest-confidence dataset in the package. Notification number, date and the three-clause structure are confirmed. |
| 16 | `MOSTEEL` — Ministry of Steel | PARTIAL | DMI and SP Policy. Baseline G.S.R. 451(E), 08.05.2017. DPIIT's index still cites the version "as on 31.12.2020"; the policy was revised 26.05.2025 and amended 25.07.2025 | https://steel.gov.in/policy-providing-preference-domestically | No. Currency warning instead: any citation resting on 31.12.2020 is stale. The Appendix A/B per-item table was not fetched. |
| 17 | `MOM` — Ministry of Mines | FULL (scope) | 1 item (Aluminium Metal), notification dated 06.12.2021 | https://dpiit.gov.in/ministry-mines | No. Single-item scope confirmed; the percentage inside that notification was not fetched and has not been invented. |
| 18 | `DOF` — Department of Fertilizers | FULL (scope) | 1 item (Single Super Phosphate), notification dated 20.08.2020 | https://dpiit.gov.in/department-fertilizers | No. Same basis as Mines. |
| 19 | `DST` — Department of Science and Technology | FULL | Order No. Misc.1/03/2021-CDN (e-31937), 29.11.2022 | https://www.dpiit.gov.in/static/uploads/2025/07/2172ce6992b0054d722411621e774ab1.pdf | No. All 26 items transcribed verbatim. No HSN codes in this notification. |
| 20 | `DAE` — Department of Atomic Energy | DEFAULT_APPLIES | **None exists — no number, no date, no URL to cite.** Absent from DPIIT's master list and from general search across three passes | (none) | No. Verified-negative finding, on the same footing as `MOD-DEFENCE`. |
| 21 | `DOP` — Department of Pharmaceuticals | PARTIAL | 135 items 16.02.2021 plus 19 items 25.03.2021 = the 154 DPIIT cites | https://dpiit.gov.in/department-pharmaceuticals | No. Scope confirmed precisely; the 154 device names were not transcribed. Every parameter on the row is the DPIIT default, which both notifications apply rather than vary. |

## Flagged for reverification: the three rows, and exactly what is doubtful

`needs_reverification: true` is a provenance marker, not a switch — a flagged rule still
applies. It is set on exactly the three rows the research flagged, matching the field's
doc comment in `pramaan-primitives/src/lib.rs`:

| Ministry | What is doubtful | What is **not** doubtful |
|---|---|---|
| `DCPC` Chemicals | The **HSN digits** — six codes unresolved by OCR against a scanned source; and the item count (26 transcribed vs 28 notified). | Item names, the notification number and date, and the 50/20 threshold. |
| `MOCA` Civil Aviation | The **whole item list** — reconstructed from an OCR-degraded scan, part of it printed mirror-reversed. Indicative, not verbatim. | Notification number, date, and the clause 3(a) / 3(b)-(c) / 13 rule structure. |
| `MOT` Textiles | The **item list** (17 items) and the **notification numbers**, neither recovered after three attempts. | The two notification dates (01.02.2019, 23.10.2019) and the item count, from DPIIT's index. |

## HSN to ministry map

`scripts/seed-ministries/hsn-map.json` holds 76 codes, every one appearing literally in a
notification in the package. No code was inferred, padded or invented.

| Ministry | Codes | Source |
|---|---|---|
| `DOT` | 8 | DoT Notification 18-10/2017-IP Table-A |
| `DHI` | 39 (35 goods HSN + 4 service SAC) | Heavy Industries boiler re-notification F.No.16(4)/2018-HEI-(Part-1)(E-15608), 29.04.2025 |
| `DCPC` | 29 | DCPC OM C.I.43012/52/2017-Chem-I(B), 13.08.2024 |

Seven of those entries carry `needs_reverification: true` individually: the six
OCR-unresolved chemicals codes, and the DoT optical-fibre code printed as `9001000` at
seven digits.

## Units

| Quantity | Unit | Example |
|---|---|---|
| `certification_threshold`, `exemption_floor` | **paise**, as a digits-only decimal string | Rs 10 crore = `"10000000000"`; Rs 5 lakh = `"50000000"`; Rs 1 crore = `"1000000000"` |
| `class_one_bps`, `class_two_bps`, `preference_margin_bps` | **basis points**, 10000 = 100 percent | 50 percent = `5000`; 65 percent = `6500`; 20 percent = `2000` |
| `effective_from` | **block number**, not a date | `0` on every row. Notification dates live in `source_order`. |

## What changed against the previous placeholder file

| Ministry | Change | Why |
|---|---|---|
| `DOT` | HSN `8517` at 6000 bps replaced by 8 real HSN codes at their notified minima (5000/5500/6000). | The placeholder's own note admitted 60 percent was chosen as "a realistic, commonly-cited telecom threshold" pending the notification text. That invented number is gone. |
| `DHI` | 5000/2000 replaced by 6500/6000. | The real notified figures: automobiles Class-I 65 percent, automotive components Class-I 60 percent. One of the only ministries setting its own number. |
| `MOR` | `calculation_method` Standard replaced by `NegativeList`. | Railways notifies a negative list, not a positive per-item list. |
| `MOPNG` | `exemption_floor` Rs 5 lakh replaced by Rs 1 crore (`"1000000000"`). | The 26.04.2022 PP-LC revision sets its own floor. |
| `MOHUA` | 5000 replaced by 6000; `para_3a_applicable` false to true. | 60 percent metro-rail Class-I per the package; the 01.01.2021 letter is expressly a clause 3(a) notification. |
| `MOD-DEFENCE` | `para_3a_applicable` and `pli_linked` true to false. | The package's research finds no notification at all for this department, so nothing can have been notified under Para 3A. `DEFAULT_APPLIES`. |
| `DCPC`, `MOCA`, `MNRE`, `DST`, `MOS` | `para_3a_applicable` false to true. | Each ministry's own notification text states Class-I-only eligibility irrespective of purchase value, or cites clause 3(a) directly. |
| `MOS` | Name "Ministry of Shipping" to "Ministry of Ports, Shipping and Waterways". | Current ministry name in the package. |
| `DHI` | Name "Department of Heavy Industries" to "Ministry of Heavy Industries". | Ditto. |
| All 21 | `source_order` rewritten from PoC-document table references to real notification numbers, dates and source URLs; `needs_reverification` and `confidence` added. | This is the provenance a reviewer checks. |

`ministry_id` values are unchanged on all 21 rows — they are the on-chain storage keys,
and renaming one would orphan its rule.

## Judgement calls a reviewer should know about

1. **`DHI` carries two Class-I figures in one threshold pair.** `class_one_bps: 6500` is
   the automobile Class-I threshold and `class_two_bps: 6000` is the automotive-component
   Class-I threshold. `class_two_bps` here is **not** a notified Class-II boundary — the
   DPIIT Class-II floor of 20 percent is not displaced by the notification. Recording both
   in the one pair the schema gives a ministry is a deliberate compression; a
   per-product-family threshold key is the change that would state both properly.
2. **`DHI` boilers are judged at the automobile figure.**
   `pallet-pramaan-classification::thresholds_for_rule` reads `hsn_thresholds.first()`
   only, so a boiler bid routed through `DHI` currently meets 65 percent although the
   boiler notification uses the standard 50 percent Class-I definition. The boiler codes
   are seeded into `HsnMinistryMap` regardless, so the fix is a per-HSN threshold lookup,
   not a data change.
3. **`DOT` thresholds are per-HSN minima, because the notification is per-product.** One
   HSN code carries several products at different Minimum Local Content levels (85176290
   spans 50 to 65 percent). Each row carries the **lowest** MLC the notification actually
   prints for that code; nothing is averaged or interpolated. Product-level classification
   needs the full per-product table in `telecom_full_table.json`.
4. **`MOHUA`'s 60 percent comes from a tender, not the notification.** The 01.01.2021
   letter restricts sourcing to Class-I without restating a percentage; the 60 percent is
   evidenced by Delhi Metro Rail Corporation's approved NIT of 30.12.2021 citing the DPIIT
   Order of 04.06.2020. The package asserts it in two separate files, so it is seeded — but
   if a reviewer holds that DMRC applied a tender-specific enhancement rather than
   restating a notified sectoral threshold, the correct value reverts to 5000 bps.
5. **`DDP`'s 40 percent figures are recorded, not seeded.** The 2018 notification prints
   40 percent for five electronic sub-assemblies. They are not encoded because (a) the
   25.08.2020 notification replaced explicit percentages with a blanket Class-I
   requirement, (b) defence local content is measured under the DPP Indigenous Content
   definition rather than the DPIIT sale-price formula, and (c) PPP-MII permits a nodal
   ministry to notify a *higher* Class-I threshold than 50 percent, never a lower one.
6. **`MOSTEEL`'s 15-50 percent DMI and SP range is recorded, not seeded.** Minimum domestic
   value addition under DMI and SP is a different instrument from the PPP-MII
   Class-I/Class-II boundary, and 15 percent would sit below the Class-II floor of 20
   percent that a nodal ministry may not lower.
7. **`MOPNG` keeps `calculation_method: "Standard"`.** Its PP-LC policy permits AMC, CMC,
   transportation and training to count as local value addition, which the DPIIT order
   excludes. `Rule` has no per-ministry exclusion list, and DPIIT's own Standing Committee
   has formally objected to that wider basis with no resolution found, so the contested
   provision is documented rather than silently applied. Only the Rs 1 crore floor is
   encoded.
8. **`MOCA`'s "under Rs 50 lakh" is recorded, not seeded as `exemption_floor`.** The figure
   appears in the clause 3(a) context of the same OCR-degraded source whose item list is
   flagged; Rs 5 lakh is the only floor confirmed to apply.
9. **Reverification is flagged on three rows only.** Several other rows carry real gaps
   (Power's 2021 revision unextracted, Steel's stale index citation, Shipping's
   research-pass sourcing, Heavy Industries' partial component list, Pharma's untranscribed
   device names). None is flagged, because in each case the gap is the item *scope* while
   every numeric parameter on the row is certain, and because the field's doc comment
   reserves the flag for the three ministries whose *source quality* is caveated. Each gap
   is stated in that row's `source_order`.

## Note on the source package

`README_INSTRUCTIONS.md` lists a `gaps_and_next_steps.md` in its file table, but that file
is not present in the delivered package. The 13 JSON files plus the README were all read.
