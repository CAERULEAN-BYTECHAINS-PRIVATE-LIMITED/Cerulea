/**
 * Record generation: turns a seeded RNG plus the reference data in `gem-data.ts` into
 * GeM sellers and GeM bid documents that look like the real thing.
 *
 * Everything here is deterministic. Given the same `--seed`, `--run-id` and pathway id,
 * every bid number, GSTIN, Udyam number, quantity, EMD figure and date is identical run
 * to run, which is what makes a rehearsed demo reproducible.
 */

import {
  BID_TYPES,
  BID_VALIDITY_DAYS_CHOICES,
  EMD_PERCENT_CHOICES,
  EPBG_DURATION_MONTHS_CHOICES,
  EPBG_PERCENT_CHOICES,
  EVALUATION_METHODS,
  GEM_BID_THRESHOLD_PAISE,
  MSME_BANDS,
  ORG_SUFFIXES,
  PACKET_TYPES,
  PLACE_STEMS,
  SECTOR_NOUNS,
  STATE_CODES,
  TECHNICAL_CLARIFICATION_DAYS_CHOICES,
  buyerFor,
  crorePaise,
  formatBps,
  formatPaise,
  itemCategoriesFor,
  ministry,
  thresholdsFor,
  type ItemCategory,
} from './gem-data';
import type { Rng } from './rng';
import { accountIdFor } from './ss58';
import type {
  BoqLineItem,
  ChainRule,
  GemBid,
  GemBidType,
  GemVendor,
  MinistryRow,
  MsmeCategory,
} from './types';

/**
 * The simulator's "today".
 *
 * Fixed rather than read from the system clock, because `--seed` promises reproducible
 * records and a wall-clock date would break that promise every midnight. Override with
 * `--as-of=YYYY-MM-DD` when a demo wants dates near the day it is given.
 */
export const DEFAULT_AS_OF = '2026-04-06';

// ---------------------------------------------------------------------------------
// Identifiers
// ---------------------------------------------------------------------------------

const GSTIN_ALPHABET = '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ';

/**
 * The real GSTIN check-digit algorithm: weight each of the first 14 characters
 * alternately by 1 and 2 over the base-36 alphabet, fold each product's digits, and map
 * the complement of the running total back into the alphabet. Generating a GSTIN that
 * actually validates costs fifteen lines and makes the sample data survive a judge
 * pasting one into a checker.
 */
export function gstinCheckDigit(first14: string): string {
  if (first14.length !== 14) {
    throw new RangeError(`gstinCheckDigit: expected 14 characters, got ${first14.length}`);
  }
  let sum = 0;
  for (let i = 0; i < 14; i++) {
    const value = GSTIN_ALPHABET.indexOf(first14.charAt(i));
    if (value < 0) throw new RangeError(`gstinCheckDigit: illegal character "${first14.charAt(i)}"`);
    const product = value * (i % 2 === 0 ? 1 : 2);
    sum += Math.floor(product / 36) + (product % 36);
  }
  return GSTIN_ALPHABET.charAt((36 - (sum % 36)) % 36);
}

/** `<state><PAN><entity><Z><check>`, e.g. `27AABCS1429B1ZQ`. */
export function makeGstin(rng: Rng, stateCode: string): string {
  // PAN shape: five letters, four digits, one letter. The fourth letter is the entity
  // type -- 'C' for company, 'F' for partnership/LLP -- and the fifth is the first
  // letter of the surname/company name for a company.
  const pan = `${rng.letters(3)}${rng.chance(0.8) ? 'C' : 'F'}${rng.letters(1)}${rng.digits(4)}${rng.letters(1)}`;
  const first14 = `${stateCode}${pan}${rng.int(1, 9)}Z`;
  return `${first14}${gstinCheckDigit(first14)}`;
}

/** `UDYAM-<state>-<district>-<7 digits>`, the format issued at udyam.gov.in. */
export function makeUdyamNumber(rng: Rng, udyamStateCode: string): string {
  return `UDYAM-${udyamStateCode}-${String(rng.int(1, 99)).padStart(2, '0')}-${rng.digits(7)}`;
}

/**
 * `GEM/YYYY/B/NNNNNNN` — the live GeM bid-number format, a seven-digit running serial.
 * Published bids in 2025 sit in the 5.8-8.4 million range, so serials are drawn from
 * there rather than from 0000001.
 */
export function makeBidNumber(rng: Rng, year: number): string {
  return `GEM/${year}/B/${rng.int(5_800_000, 8_499_999)}`;
}

/** `GEM/YYYY/C/NNNNNNN` — the certificate id used for the CA-certification trigger. */
export function makeCertificateId(rng: Rng, year: number, runId: string): string {
  // The certificate id is the one identifier that must differ between runs: the
  // certification pallet rejects a repeat with `CertificateIdAlreadyUsed`, so a
  // seed-only id would make the second run of a demo fail. `--run-id` keeps it
  // reproducible when you want it to be.
  return `GEM/${year}/C/${rng.int(1_000_000, 9_999_999)}-${runId}`;
}

// ---------------------------------------------------------------------------------
// Dates
// ---------------------------------------------------------------------------------

function addDays(iso: string, days: number): Date {
  const base = new Date(`${iso}T00:00:00Z`);
  if (Number.isNaN(base.getTime())) {
    throw new RangeError(`Invalid as-of date "${iso}"; expected YYYY-MM-DD.`);
  }
  return new Date(base.getTime() + days * 86_400_000);
}

/** GeM prints dates as `DD-MM-YYYY HH:mm:ss`. */
export function formatGemDateTime(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return (
    `${pad(d.getUTCDate())}-${pad(d.getUTCMonth() + 1)}-${d.getUTCFullYear()} ` +
    `${pad(d.getUTCHours())}:${pad(d.getUTCMinutes())}:${pad(d.getUTCSeconds())}`
  );
}

export function formatGemDate(d: Date): string {
  const pad = (n: number) => String(n).padStart(2, '0');
  return `${pad(d.getUTCDate())}-${pad(d.getUTCMonth() + 1)}-${d.getUTCFullYear()}`;
}

// ---------------------------------------------------------------------------------
// Vendors
// ---------------------------------------------------------------------------------

export interface VendorOptions {
  /** Stable label the SS58 account id is derived from. Must be unique per vendor. */
  label: string;
  /** Force a specific legal name (used for the PoC's own named vendors). */
  legalName?: string;
  /** Force MSE status; otherwise drawn from the MSME bands. */
  isMse?: boolean;
  /** Force PLI-beneficiary status (drives the P4 deeming rule). */
  isPliBeneficiary?: boolean;
  sector?: keyof typeof SECTOR_NOUNS;
}

export function makeVendor(rng: Rng, seed: number, opts: VendorOptions): GemVendor {
  const stateEntry = rng.pick(STATE_CODES);
  const sector = opts.sector ?? 'general';
  const nouns = SECTOR_NOUNS[sector] ?? SECTOR_NOUNS.general;
  const noun = rng.pick(nouns ?? []);
  // "Industries" appears both as a sector noun and inside a suffix; drawing both would
  // produce "Aravalli Industries Industries Private Limited".
  const suffixChoices = ORG_SUFFIXES.filter((s) => !s.includes(noun));
  const legalName =
    opts.legalName ??
    `${rng.pick(PLACE_STEMS)} ${noun} ${rng.pick(suffixChoices.length > 0 ? suffixChoices : ORG_SUFFIXES)}`;

  let band = rng.pick(MSME_BANDS);
  if (opts.isMse !== undefined) {
    const matching = MSME_BANDS.filter((b) => b.isMse === opts.isMse);
    band = rng.pick(matching);
  }

  const turnoverCrore = Math.max(0.4, band.maxTurnoverCrore * (0.25 + rng.next() * 0.7));
  const registered = band.category !== 'Not Registered';

  return {
    accountId: accountIdFor(seed, opts.label),
    legalName,
    gstin: makeGstin(rng, stateEntry.code),
    udyamRegistrationNumber: registered ? makeUdyamNumber(rng, stateEntry.udyamCode) : null,
    msmeCategory: band.category as MsmeCategory,
    isMse: band.isMse,
    city: rng.pick(stateEntry.cities),
    state: stateEntry.state,
    isPliBeneficiary: opts.isPliBeneficiary ?? false,
    averageAnnualTurnoverPaise: crorePaise(Number(turnoverCrore.toFixed(2))).toString(),
    yearsOfExperience: rng.int(3, 24),
  };
}

// ---------------------------------------------------------------------------------
// Bids
// ---------------------------------------------------------------------------------

export interface BidOptions {
  ministryId: string;
  /** Stable label the tender id is derived from. Must be unique per bid. */
  label: string;
  /** Pin the item category by name instead of drawing one. */
  itemCategoryName?: string;
  /** Pin the total contract value; otherwise derived from quantity x unit price. */
  estimatedBidValuePaise?: bigint;
  bidType?: GemBidType;
  asOf?: string;
  runId: string;
}

/**
 * The `TenderId`/`ProductId` a pallet sees.
 *
 * A GeM bid number contains slashes, which are legal inside a `BoundedVec<u8, 64>` but
 * awkward in a URL and in a demo screenshot, so the on-chain tender id is the bid number
 * with slashes turned into dashes. The product id is the HSN heading plus the item
 * category, because the consistency engine keys on `(vendor, product)` and "the same
 * product on another tender" means the same catalogue item, not the same tender.
 */
export function tenderIdFromBidNumber(bidNumber: string): string {
  return bidNumber.replace(/\//g, '-');
}

export function productIdFor(item: ItemCategory): string {
  const slug = item.name
    .toUpperCase()
    .replace(/[^A-Z0-9]+/g, '-')
    .replace(/^-|-$/g, '')
    .slice(0, 48);
  return `HSN${item.hsnCode}-${slug}`;
}

export function makeBid(rng: Rng, opts: BidOptions): GemBid {
  const row = ministry(opts.ministryId);
  const buyer = buyerFor(opts.ministryId);
  const categories = itemCategoriesFor(opts.ministryId);
  const item = opts.itemCategoryName
    ? (categories.find((c) => c.name === opts.itemCategoryName) ??
      (() => {
        throw new Error(
          `Item category "${opts.itemCategoryName}" is not configured for ${opts.ministryId}.`,
        );
      })())
    : rng.pick(categories);

  const asOf = opts.asOf ?? DEFAULT_AS_OF;
  const publishedOn = addDays(asOf, -rng.int(1, 20));
  const bidEnd = addDays(asOf, rng.int(6, 28));
  // GeM opens a bid immediately after it closes, on the same date.
  const bidOpening = new Date(bidEnd.getTime() + rng.int(15, 90) * 60_000);
  const year = publishedOn.getUTCFullYear();

  let quantity: number;
  let estimatedBidValuePaise: bigint;
  if (opts.estimatedBidValuePaise !== undefined) {
    estimatedBidValuePaise = opts.estimatedBidValuePaise;
    const unitPaise = BigInt(Math.round(item.unitPriceRupees * 100));
    quantity = Math.max(1, Number(estimatedBidValuePaise / (unitPaise > 0n ? unitPaise : 1n)));
  } else {
    quantity = rng.int(...quantityBandFor(item));
    estimatedBidValuePaise = BigInt(Math.round(item.unitPriceRupees * 100)) * BigInt(quantity);
    if (estimatedBidValuePaise < GEM_BID_THRESHOLD_PAISE) {
      // Below Rs 3 lakh a GeM purchase is a direct order, not a bid. Scale up rather
      // than emit a record that could not legally have been a bid at all.
      const factor =
        Number(GEM_BID_THRESHOLD_PAISE / (estimatedBidValuePaise > 0n ? estimatedBidValuePaise : 1n)) + 1;
      quantity *= Math.max(2, factor);
      estimatedBidValuePaise = BigInt(Math.round(item.unitPriceRupees * 100)) * BigInt(quantity);
    }
  }

  const emdPercent = rng.pick(EMD_PERCENT_CHOICES);
  const emdAmountPaise = roundToRupee(
    (estimatedBidValuePaise * BigInt(Math.round(emdPercent * 100))) / 10_000n,
  );

  const bidNumber = makeBidNumber(rng, year);
  const bidType = opts.bidType ?? pickBidType(rng, estimatedBidValuePaise);
  const thresholds = thresholdsFor(opts.ministryId);

  return {
    bidNumber,
    dated: formatGemDate(publishedOn),
    bidEndDateTime: formatGemDateTime(bidEnd),
    bidOpeningDateTime: formatGemDateTime(bidOpening),
    bidOfferValidityDays: rng.pick(BID_VALIDITY_DAYS_CHOICES),

    ministryStateName: buyer.ministryStateName,
    departmentName: buyer.departmentName,
    organisationName: buyer.organisationName,
    officeName: rng.pick(buyer.officeNames),
    ministryId: row.ministry_id,

    itemCategory: item.name,
    hsnCode: item.hsnCode,
    totalQuantity: quantity,
    unit: item.unit,
    estimatedBidValuePaise: estimatedBidValuePaise.toString(),

    // GeM commonly asks for turnover of roughly the order value and 3 years' experience.
    minimumAverageAnnualTurnoverPaise: roundToRupee(estimatedBidValuePaise).toString(),
    yearsOfPastExperience: rng.int(1, 5),
    mseExemptionForYearsOfExperienceAndTurnover: rng.chance(0.6),
    startupExemptionForYearsOfExperienceAndTurnover: rng.chance(0.4),

    bidType,
    typeOfBid: bidType === 'BOQ Bid' ? 'Two Packet Bid' : rng.pick(PACKET_TYPES),
    bidToRaEnabled: bidType === 'Bid-to-RA',
    evaluationMethod:
      bidType === 'BOQ Bid' ? 'Item wise evaluation' : rng.pick(EVALUATION_METHODS),
    timeAllowedForTechnicalClarificationsDays: rng.pick(TECHNICAL_CLARIFICATION_DAYS_CHOICES),

    emdAmountPaise: emdAmountPaise.toString(),
    emdPercent,
    epbgPercent: rng.pick(EPBG_PERCENT_CHOICES),
    epbgDurationMonths: rng.pick(EPBG_DURATION_MONTHS_CHOICES),

    msePurchasePreference: true,
    makeInIndiaPreference: true,
    minimumLocalContentBpsForClassOne: thresholds.classOneBps,
    tenderId: tenderIdFromBidNumber(bidNumber),
    productId: productIdFor(item),
  };
}

function quantityBandFor(item: ItemCategory): [number, number] {
  // Big-ticket capital items are bought in ones and twos; consumables in thousands.
  if (item.unitPriceRupees >= 10_000_000) return [1, 4];
  if (item.unitPriceRupees >= 1_000_000) return [2, 25];
  if (item.unitPriceRupees >= 100_000) return [10, 240];
  if (item.unitPriceRupees >= 10_000) return [50, 1_800];
  if (item.unitPriceRupees >= 1_000) return [500, 9_000];
  return [5_000, 90_000];
}

function pickBidType(rng: Rng, valuePaise: bigint): GemBidType {
  if (valuePaise < GEM_BID_THRESHOLD_PAISE) return 'Custom Catalogue-Based Bid';
  return rng.pick(BID_TYPES);
}

function roundToRupee(paise: bigint): bigint {
  return (paise / 100n) * 100n;
}

/**
 * A BOQ (Bill of Quantities) bid's line items, the case that exercises the
 * weighted-average local-content path: "the sum of each item's local value added,
 * divided by the sum of the sale prices."
 */
export function makeBoqLines(
  rng: Rng,
  ministryId: string,
  totalValuePaise: bigint,
  lineCount: number,
): BoqLineItem[] {
  const categories = itemCategoriesFor(ministryId);
  const chosen = rng.sample(categories, Math.min(lineCount, categories.length));
  const weights = chosen.map(() => rng.int(10, 60));
  const weightTotal = weights.reduce((a, b) => a + b, 0);

  return chosen.map((item, i) => {
    const share = (totalValuePaise * BigInt(weights[i] as number)) / BigInt(weightTotal);
    const salePricePaise = roundToRupee(share);
    const localBps = rng.int(1_500, 8_500);
    return {
      description: item.name,
      hsnCode: item.hsnCode,
      quantity: Math.max(1, Number(salePricePaise / BigInt(Math.round(item.unitPriceRupees * 100)))),
      unit: item.unit,
      salePricePaise: salePricePaise.toString(),
      localValueAddedPaise: ((salePricePaise * BigInt(localBps)) / 10_000n).toString(),
    };
  });
}

/** The weighted-average local content of a BOQ bid, in basis points. */
export function weightedLocalContentBps(lines: readonly BoqLineItem[]): number {
  const totalSale = lines.reduce((acc, l) => acc + BigInt(l.salePricePaise), 0n);
  if (totalSale === 0n) throw new RangeError('weightedLocalContentBps: total sale price is zero');
  const totalLocal = lines.reduce((acc, l) => acc + BigInt(l.localValueAddedPaise), 0n);
  return Number((totalLocal * 10_000n) / totalSale);
}

// ---------------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------------

/** A `ministries.json` row in the camelCase shape the rule-update route accepts. */
export function toChainRule(row: MinistryRow): ChainRule {
  return {
    hsnThresholds: row.hsn_thresholds.map((h) => ({
      hsnCode: h.hsn_code,
      classOneBps: h.class_one_bps,
      classTwoBps: h.class_two_bps,
    })),
    para3aApplicable: row.para_3a_applicable,
    pliLinked: row.pli_linked,
    calculationMethod: row.calculation_method,
    preferenceMarginBps: row.preference_margin_bps,
    certificationThreshold: row.certification_threshold,
    exemptionFloor: row.exemption_floor,
    divisibility: row.divisibility,
    effectiveFrom: row.effective_from,
  };
}

/** The on-chain rule for a ministry, with named fields overridden. */
export function ruleWithOverrides(ministryId: string, overrides: Partial<ChainRule>): ChainRule {
  return { ...toChainRule(ministry(ministryId)), ...overrides };
}

/** A one-line summary of a GeM bid, for the run log. */
export function describeBid(bid: GemBid): string {
  return (
    `${bid.bidNumber} | ${bid.organisationName} | ${bid.itemCategory} ` +
    `(HSN ${bid.hsnCode}) | ${bid.totalQuantity} ${bid.unit} | ${bid.bidType}`
  );
}

/**
 * The generated bid rendered with the field labels a real GeM bid document prints, so a
 * judge can hold the two side by side. Used by `--show-records`.
 */
export function renderBidDocument(bid: GemBid): string[] {
  return [
    `Bid Number                                  : ${bid.bidNumber}`,
    `Dated                                       : ${bid.dated}`,
    `Bid End Date/Time                           : ${bid.bidEndDateTime}`,
    `Bid Opening Date/Time                       : ${bid.bidOpeningDateTime}`,
    `Bid Offer Validity (From End Date)          : ${bid.bidOfferValidityDays} (Days)`,
    `Ministry/State Name                         : ${bid.ministryStateName}`,
    `Department Name                             : ${bid.departmentName}`,
    `Organisation Name                           : ${bid.organisationName}`,
    `Office Name                                 : ${bid.officeName}`,
    `Item Category                               : ${bid.itemCategory}`,
    `HSN Code                                    : ${bid.hsnCode}`,
    `Total Quantity                              : ${bid.totalQuantity} ${bid.unit}`,
    `Estimated Bid Value                         : ${formatPaise(bid.estimatedBidValuePaise)}`,
    `Minimum Average Annual Turnover of bidder   : ${formatPaise(bid.minimumAverageAnnualTurnoverPaise)}`,
    `Years of Past Experience Required           : ${bid.yearsOfPastExperience} Year(s)`,
    `MSE Exemption for Experience and Turnover   : ${yesNo(bid.mseExemptionForYearsOfExperienceAndTurnover)}`,
    `Startup Exemption for Experience/Turnover   : ${yesNo(bid.startupExemptionForYearsOfExperienceAndTurnover)}`,
    `Type of Bid                                 : ${bid.typeOfBid}`,
    `Bid Type                                    : ${bid.bidType}`,
    `Bid to RA enabled                           : ${yesNo(bid.bidToRaEnabled)}`,
    `Evaluation Method                           : ${bid.evaluationMethod}`,
    `Time allowed for Technical Clarifications   : ${bid.timeAllowedForTechnicalClarificationsDays} Days`,
    `EMD Amount                                  : ${formatPaise(bid.emdAmountPaise)} (${bid.emdPercent}%)`,
    `ePBG Percentage(%)                          : ${bid.epbgPercent.toFixed(2)}`,
    `Duration of ePBG required (Months)          : ${bid.epbgDurationMonths}`,
    `MSE Purchase Preference                     : ${yesNo(bid.msePurchasePreference)}`,
    `Make In India (MII) Preference              : ${yesNo(bid.makeInIndiaPreference)}`,
    `Minimum local content for Class 1 supplier  : ${formatBps(bid.minimumLocalContentBpsForClassOne)}`,
    `On-chain TenderId                           : ${bid.tenderId}`,
    `On-chain ProductId                          : ${bid.productId}`,
  ];
}

/** A GeM seller rendered the way the portal shows a bidder's profile. */
export function renderVendor(vendor: GemVendor): string[] {
  return [
    `Seller                                      : ${vendor.legalName}`,
    `GSTIN                                       : ${vendor.gstin}`,
    `Udyam Registration Number                   : ${vendor.udyamRegistrationNumber ?? 'Not Registered'}`,
    `MSME Category                               : ${vendor.msmeCategory}` +
      `${vendor.isMse ? ' (eligible for MSE Purchase Preference)' : ''}`,
    `Place of Business                           : ${vendor.city}, ${vendor.state}`,
    `Average Annual Turnover (last 3 FYs)        : ${formatPaise(vendor.averageAnnualTurnoverPaise)}`,
    `Years of Experience                         : ${vendor.yearsOfExperience}`,
    `PLI Incentive Received                      : ${yesNo(vendor.isPliBeneficiary)}`,
    `On-chain AccountId                          : ${vendor.accountId}`,
  ];
}

function yesNo(value: boolean): string {
  return value ? 'Yes' : 'No';
}
