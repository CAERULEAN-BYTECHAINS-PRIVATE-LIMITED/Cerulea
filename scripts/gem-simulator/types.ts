/**
 * Types shared by the record generator, the API client, and the scenario definitions.
 *
 * Unit conventions (docs/PRAMAAN_BUILD_CONTRACT.md section 1) are enforced by naming:
 * every monetary field ends in `Paise` and every percentage field ends in `Bps` or
 * `Percent`. Nothing in this file carries a rupee figure into a request body.
 */

/** The twelve decision pathways, `pramaan_primitives::PathwayId`. */
export type PathwayId =
  | 'P1'
  | 'P2'
  | 'P3'
  | 'P4'
  | 'P5'
  | 'P6'
  | 'P7'
  | 'P8'
  | 'P9'
  | 'P10'
  | 'P11'
  | 'P12';

export const ALL_PATHWAYS: readonly PathwayId[] = [
  'P1',
  'P2',
  'P3',
  'P4',
  'P5',
  'P6',
  'P7',
  'P8',
  'P9',
  'P10',
  'P11',
  'P12',
];

/** `pallet_pramaan_classification::ClassResult`. */
export type ClassResult = 'ClassOne' | 'ClassTwo' | 'NonLocal' | 'ManualReviewRequired';

/** `pallet_pramaan_preference::ClassResultLike` — the three-variant bid-side mirror. */
export type BidClass = 'ClassOne' | 'ClassTwo' | 'NonLocal';

/** `pramaan_primitives::CalculationMethod`. */
export type CalculationMethod = 'Standard' | 'ComponentLevel' | 'WeightedModule' | 'Custom';

/** `pramaan_primitives::Divisibility`. */
export type Divisibility = 'Divisible' | 'NonDivisible';

/** The tri-state every trigger-point route returns (build contract section 3). */
export type TriState = 'GREEN' | 'YELLOW' | 'RED';

/** One row of `scripts/seed-ministries/ministries.json`, as seeded on chain. */
export interface MinistryRow {
  ministry_id: string;
  ministry_name: string;
  hsn_thresholds: Array<{ hsn_code: string; class_one_bps: number; class_two_bps: number }>;
  para_3a_applicable: boolean;
  pli_linked: boolean;
  calculation_method: CalculationMethod;
  preference_margin_bps: number;
  certification_threshold: string;
  exemption_floor: string;
  divisibility: Divisibility;
  effective_from: number;
  source_order: string;
}

/**
 * `pramaan_primitives::Rule<Balance, BlockNumber>` in the camelCase shape
 * `@polkadot/api` encodes and the `/api/trigger/rule-update` route accepts. Field order
 * matches the pallet's struct field-for-field.
 */
export interface ChainRule {
  hsnThresholds: Array<{ hsnCode: string; classOneBps: number; classTwoBps: number }>;
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: CalculationMethod;
  preferenceMarginBps: number;
  certificationThreshold: string;
  exemptionFloor: string;
  divisibility: Divisibility;
  effectiveFrom: number;
}

/** MSME category under the Udyam classification revised with effect from 01.04.2025. */
export type MsmeCategory = 'Micro' | 'Small' | 'Medium' | 'Not Registered';

/**
 * A GeM seller.
 *
 * Company names are deliberately fictional. The two named vendors that appear in the
 * PoC submission document itself — Sabarmati Systems and Chambal Devices — are reused
 * verbatim for the scenarios the document describes; every other name is generated in
 * the same idiom. No real Indian company is named anywhere in this simulator, because
 * several scenarios attach debarments and false-declaration findings to their vendors.
 */
export interface GemVendor {
  /** SS58 account id — what every pallet extrinsic actually receives. */
  accountId: string;
  legalName: string;
  gstin: string;
  /** `UDYAM-<state>-<district>-<7 digits>`, present only for a registered MSME. */
  udyamRegistrationNumber: string | null;
  msmeCategory: MsmeCategory;
  /** GeM's "MSE Purchase Preference" applies to Micro and Small only, not Medium. */
  isMse: boolean;
  city: string;
  state: string;
  /** Whether this vendor has actually received a PLI incentive (drives P4 deeming). */
  isPliBeneficiary: boolean;
  averageAnnualTurnoverPaise: string;
  yearsOfExperience: number;
}

/** GeM bid types, per the portal's own bid-creation options. */
export type GemBidType =
  | 'Standard Bid'
  | 'Custom Catalogue-Based Bid'
  | 'BOQ Bid'
  | 'Bid-to-RA';

export type GemPacketType = 'Single Packet Bid' | 'Two Packet Bid';

export type GemEvaluationMethod = 'Total value wise evaluation' | 'Item wise evaluation';

/**
 * One GeM bid (what the portal calls a "Bid Document"). Field names mirror the labels
 * printed on a real GeM bid document so a judge can hold the two side by side.
 */
export interface GemBid {
  /** `GEM/YYYY/B/NNNNNNN`. */
  bidNumber: string;
  dated: string;
  bidEndDateTime: string;
  bidOpeningDateTime: string;
  /** "Bid Offer Validity (From End Date)", in days. */
  bidOfferValidityDays: number;

  // --- Buyer Organisation, the four-level GeM hierarchy -----------------------------
  ministryStateName: string;
  departmentName: string;
  organisationName: string;
  officeName: string;
  /** The `ministry_id` from ministries.json this bid's rule is read under. */
  ministryId: string;

  // --- What is being bought ---------------------------------------------------------
  itemCategory: string;
  hsnCode: string;
  totalQuantity: number;
  unit: string;
  estimatedBidValuePaise: string;

  // --- Qualification ----------------------------------------------------------------
  minimumAverageAnnualTurnoverPaise: string;
  yearsOfPastExperience: number;
  mseExemptionForYearsOfExperienceAndTurnover: boolean;
  startupExemptionForYearsOfExperienceAndTurnover: boolean;

  // --- Process ----------------------------------------------------------------------
  bidType: GemBidType;
  typeOfBid: GemPacketType;
  bidToRaEnabled: boolean;
  evaluationMethod: GemEvaluationMethod;
  timeAllowedForTechnicalClarificationsDays: number;

  // --- Security -----------------------------------------------------------------
  emdAmountPaise: string;
  emdPercent: number;
  epbgPercent: number;
  epbgDurationMonths: number;

  // --- Preference policy ------------------------------------------------------------
  msePurchasePreference: boolean;
  makeInIndiaPreference: boolean;
  /** "Minimum local content to qualify as a Class 1 local supplier", in basis points. */
  minimumLocalContentBpsForClassOne: number;
  /** The `TenderId` this bid is submitted to on chain. */
  tenderId: string;
  /** The `ProductId` used for the cross-tender consistency check. */
  productId: string;
}

/** One line of a BOQ (multi-item) bid, for the weighted-average local-content path. */
export interface BoqLineItem {
  description: string;
  hsnCode: string;
  quantity: number;
  unit: string;
  salePricePaise: string;
  localValueAddedPaise: string;
}

/** `pallet_pramaan_classification::ComponentDeclaration`. */
export interface ComponentDeclaration {
  name: string;
  declaredBps: number;
  weightBps: number;
}

/** One bid as submitted to `/api/trigger/preference-calculation`. */
export interface PreferenceBid {
  vendor: string;
  vendorName: string;
  class: BidClass;
  pricePaise: string;
  isMse: boolean;
  isGte: boolean;
}
