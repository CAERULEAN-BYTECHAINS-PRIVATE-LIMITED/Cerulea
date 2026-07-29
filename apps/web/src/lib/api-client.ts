/**
 * The typed client the six persona consoles share.
 *
 * Three things live here, and nothing else:
 *
 *   1. `postTrigger` and the six route wrappers — one place where a trigger-point call is
 *      made, so every console handles a 400, a 504 and a dead chain identically.
 *   2. The reference data a console has to submit *against*: the 21 nodal ministries and
 *      their rule parameters, read out of `scripts/seed-ministries/ministries.json` (the
 *      same file `seed.ts` seeds the chain from), plus the GeM-format tender book the
 *      demo bids into. A ministry id is never invented here; an invented one would be
 *      rejected by the chain, which has no rule for it.
 *   3. A small browser-local console ledger, so the CVC, auditor and DPIIT consoles can
 *      show what *this session actually did* — every row carries the transaction
 *      reference and finalized block the chain returned. Nothing in the ledger is
 *      fabricated: a row exists only because a trigger-point route answered.
 *
 * Nothing here imports `@/lib/chain` or `@polkadot/api`. A console talks to the chain
 * through the six API routes and nothing else — which is also why a vendor never sees a
 * key, a wallet or a gas prompt.
 */

import type { ErrorKind } from '@/components';

// =====================================================================================
// Shared vocabulary (docs/PRAMAAN_BUILD_CONTRACT.md sections 2 and 3)
// =====================================================================================

export type TriState = 'GREEN' | 'YELLOW' | 'RED';
export type ClassResult = 'ClassOne' | 'ClassTwo' | 'NonLocal' | 'ManualReviewRequired';
export type BidClass = 'ClassOne' | 'ClassTwo' | 'NonLocal';
export type CalculationMethod = 'Standard' | 'ComponentLevel' | 'WeightedModule' | 'Custom';
export type Divisibility = 'Divisible' | 'NonDivisible';

export const CALCULATION_METHODS: readonly CalculationMethod[] = [
  'Standard',
  'ComponentLevel',
  'WeightedModule',
  'Custom',
];
export const DIVISIBILITIES: readonly Divisibility[] = ['Divisible', 'NonDivisible'];

/** Every trigger-point route returns at least this much. */
export interface TriggerBase {
  result: TriState;
  reason: string;
  txRef: string | null;
  blockNumber: number | null;
  latencyMs: number;
  /** Present only when the pallet itself refused the call. */
  palletError?: string;
}

export interface BidSubmissionResponse extends TriggerBase {
  class?: ClassResult | null;
  declaredLocalContentBps?: number;
  consistencyFlagged?: boolean;
  priorDeclaredBps?: number | null;
  priorTender?: string | null;
  inconsistencyCount?: number;
  consistencyTxRef?: string | null;
  consistencyBlockNumber?: number | null;
  consistencyError?: string | null;
}

export interface BidEvaluationResponse extends TriggerBase {
  class?: ClassResult | null;
  debarredBy?: string | null;
}

/** One vendor's row of `pramaanPreference.preferenceResults`, read at the finalized block. */
export interface PreferenceOutcome {
  vendor: string;
  qualifies: boolean;
  awardedPercentBps: number;
  matchedPricePaise: string | null;
  decisionPath: string | null;
}

export interface PreferenceResponse extends TriggerBase {
  qualifies?: boolean;
  matchedPricePaise?: string | null;
  outcomes?: PreferenceOutcome[];
}

export interface CertificationResponse extends TriggerBase {
  certificateId?: string;
  requiresAuditor?: boolean;
}

export interface DebarmentResponse extends TriggerBase {
  status?: 'Debarred' | 'DebarmentLifted';
  effectiveFrom?: number;
  effectiveTo?: number | null;
}

export interface RuleUpdateResponse extends TriggerBase {
  status?: 'RuleUpdated';
  newVersion?: number;
}

// =====================================================================================
// Request bodies
// =====================================================================================

/**
 * How an account is named on the wire. The routes accept a demo persona (`vendor`), a
 * derivation URI (`//ChambalDevices`) or an SS58 address; the consoles use the first two
 * so a judge can rerun the demo and land on the same on-chain identities.
 */
export type AccountRef = string;

export interface BidSubmissionRequest {
  vendor: AccountRef;
  tender: string;
  ministry: string;
  declaredLocalContentBps: number;
  /** Supplying it also records the declaration in the cross-tender consistency history. */
  product?: string;
  isPliManufacturer?: boolean;
}

export interface BidEvaluationRequest {
  vendor: AccountRef;
  tender: string;
  ministry: string;
  declaredLocalContentBps: number;
  isPliManufacturer?: boolean;
}

export interface PreferenceBidInput {
  vendor: AccountRef;
  class: BidClass;
  pricePaise: string;
  isMse?: boolean;
  isGte?: boolean;
}

export interface PreferenceRequest {
  tender: string;
  ministry: string;
  tenderValuePaise: string;
  isTenderGte?: boolean;
  bids: PreferenceBidInput[];
}

export interface CertificationRequest {
  vendor: AccountRef;
  tender: string;
  ministry: string;
  valuePaise: string;
  /** Omitted below the ministry's threshold: that is a vendor self-certification. */
  auditor?: AccountRef;
  certificateId?: string;
}

export interface DebarmentRequest {
  vendor: AccountRef;
  ministry: string;
  action: 'debar' | 'lift';
  effectiveFrom?: number;
  effectiveTo?: number | null;
  reason?: string;
}

export interface HsnThresholdInput {
  hsnCode: string;
  classOneBps: number;
  classTwoBps: number;
}

/** The nine rule parameters, exactly as `pramaanRuleRegistry::Rule` orders them. */
export interface RuleInput {
  hsnThresholds: HsnThresholdInput[];
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: CalculationMethod;
  preferenceMarginBps: number;
  certificationThreshold: string;
  exemptionFloor: string;
  divisibility: Divisibility;
  effectiveFrom: number;
}

export interface RuleUpdateRequest {
  ministry: string;
  rule: RuleInput;
}

// =====================================================================================
// The wrapper
// =====================================================================================

export interface ApiSuccess<T> {
  ok: true;
  data: T;
}

export interface ApiFailure {
  ok: false;
  /** Chosen so the caller can hand it straight to `<ErrorState kind=… />`. */
  kind: ErrorKind;
  message: string;
  technicalDetail?: string;
}

export type ApiResult<T> = ApiSuccess<T> | ApiFailure;

const CHAIN_UNREACHABLE = /econnrefused|websocket|disconnect|connection|socket hang up|getaddrinfo/i;

/**
 * POST a trigger point and map every outcome onto something a console can render.
 *
 * A RED verdict is a SUCCESS here — the chain answered and the answer was "blocked".
 * Only the four failure kinds below mean no verdict was recorded, and those are the ones
 * `ErrorState` exists for. Confusing the two would show an operational failure in the
 * reserved red, which the build contract forbids.
 */
async function postTrigger<T extends TriggerBase>(
  route: string,
  body: unknown,
): Promise<ApiResult<T>> {
  let response: Response;
  try {
    response = await fetch(route, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });
  } catch (error) {
    return {
      ok: false,
      kind: 'chain-unreachable',
      message:
        'The console could not reach the compliance service. No transaction was submitted, so nothing has changed on chain.',
      technicalDetail: error instanceof Error ? error.message : String(error),
    };
  }

  let payload: Record<string, unknown>;
  try {
    payload = (await response.json()) as Record<string, unknown>;
  } catch {
    return {
      ok: false,
      kind: 'unknown',
      message: `${route} returned a response that was not JSON (HTTP ${response.status}).`,
    };
  }

  if (response.ok) return { ok: true, data: payload as unknown as T };

  const message = typeof payload.error === 'string' ? payload.error : `HTTP ${response.status}`;
  const detail = typeof payload.detail === 'string' ? payload.detail : undefined;

  if (response.status === 504) {
    return { ok: false, kind: 'finality-timeout', message, technicalDetail: detail };
  }
  if (response.status === 400) {
    return { ok: false, kind: 'rejected', message, technicalDetail: detail };
  }
  return {
    ok: false,
    kind: CHAIN_UNREACHABLE.test(detail ?? message) ? 'chain-unreachable' : 'unknown',
    message,
    technicalDetail: detail,
  };
}

export function submitBid(body: BidSubmissionRequest) {
  return postTrigger<BidSubmissionResponse>('/api/trigger/bid-submission', body);
}

export function evaluateBid(body: BidEvaluationRequest) {
  return postTrigger<BidEvaluationResponse>('/api/trigger/bid-evaluation', body);
}

export function calculatePreference(body: PreferenceRequest) {
  return postTrigger<PreferenceResponse>('/api/trigger/preference-calculation', body);
}

export function certify(body: CertificationRequest) {
  return postTrigger<CertificationResponse>('/api/trigger/ca-certification', body);
}

export function setDebarment(body: DebarmentRequest) {
  return postTrigger<DebarmentResponse>('/api/trigger/debarment', body);
}

export function updateRule(body: RuleUpdateRequest) {
  return postTrigger<RuleUpdateResponse>('/api/trigger/rule-update', body);
}

// =====================================================================================
// The 21 nodal ministries — scripts/seed-ministries/ministries.json
// =====================================================================================

export interface MinistryRef {
  /** The `ministry_id` the chain knows. Never invent one. */
  id: string;
  name: string;
  /** What fits in a table cell. */
  short: string;
  hsnThresholds: HsnThresholdInput[];
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: CalculationMethod;
  preferenceMarginBps: number;
  certificationThresholdPaise: string;
  exemptionFloorPaise: string;
  divisibility: Divisibility;
  effectiveFrom: number;
}

/** Every seeded ministry carries the DPIIT figures unless its own notification differs. */
const DEFAULT_MARGIN_BPS = 2_000;
const DEFAULT_CERTIFICATION_THRESHOLD_PAISE = '10000000000';
const DEFAULT_EXEMPTION_FLOOR_PAISE = '50000000';
const ANY_HSN: HsnThresholdInput[] = [{ hsnCode: '*', classOneBps: 5_000, classTwoBps: 2_000 }];

function ministry(
  id: string,
  name: string,
  short: string,
  overrides: Partial<Omit<MinistryRef, 'id' | 'name' | 'short'>> = {},
): MinistryRef {
  return {
    id,
    name,
    short,
    hsnThresholds: ANY_HSN,
    para3aApplicable: false,
    pliLinked: false,
    calculationMethod: 'Standard',
    preferenceMarginBps: DEFAULT_MARGIN_BPS,
    certificationThresholdPaise: DEFAULT_CERTIFICATION_THRESHOLD_PAISE,
    exemptionFloorPaise: DEFAULT_EXEMPTION_FLOOR_PAISE,
    divisibility: 'Divisible',
    effectiveFrom: 0,
    ...overrides,
  };
}

export const MINISTRIES: readonly MinistryRef[] = [
  ministry('DPIIT', 'Department for Promotion of Industry and Internal Trade (default)', 'DPIIT', {
    para3aApplicable: true,
  }),
  ministry('MEITY', 'Ministry of Electronics and Information Technology', 'MeitY', {
    hsnThresholds: [{ hsnCode: '8471', classOneBps: 5_000, classTwoBps: 2_000 }],
    para3aApplicable: true,
    pliLinked: true,
    calculationMethod: 'ComponentLevel',
  }),
  ministry('DOT', 'Department of Telecommunications', 'DoT', {
    hsnThresholds: [{ hsnCode: '8517', classOneBps: 6_000, classTwoBps: 2_000 }],
    para3aApplicable: true,
    pliLinked: true,
  }),
  ministry('DHI', 'Department of Heavy Industries', 'DHI', {
    para3aApplicable: true,
    pliLinked: true,
  }),
  ministry('MOPNG', 'Ministry of Petroleum and Natural Gas', 'MoPNG'),
  ministry('DCPC', 'Department of Chemicals and Petrochemicals', 'DCPC'),
  ministry('MOHUA', 'Ministry of Housing and Urban Affairs', 'MoHUA', {
    divisibility: 'NonDivisible',
  }),
  ministry('MOT', 'Ministry of Textiles', 'MoT'),
  ministry('MOS', 'Ministry of Shipping', 'MoS'),
  ministry('MOR', 'Ministry of Railways', 'MoR', {
    para3aApplicable: true,
    pliLinked: true,
    divisibility: 'NonDivisible',
  }),
  ministry('MOD-DEFENCE', 'Department of Defence, Ministry of Defence', 'MoD (Defence)', {
    para3aApplicable: true,
    pliLinked: true,
    divisibility: 'NonDivisible',
  }),
  ministry('DDP', 'Department of Defence Production', 'DDP', {
    para3aApplicable: true,
    pliLinked: true,
    divisibility: 'NonDivisible',
  }),
  ministry('MOP', 'Ministry of Power', 'MoP', { para3aApplicable: true, pliLinked: true }),
  ministry('MNRE', 'Ministry of New and Renewable Energy', 'MNRE'),
  ministry('MOCA', 'Ministry of Civil Aviation', 'MoCA'),
  ministry('MOSTEEL', 'Ministry of Steel', 'MoSteel', { para3aApplicable: true, pliLinked: true }),
  ministry('MOM', 'Ministry of Mines', 'MoM'),
  ministry('DOF', 'Department of Fertilizers', 'DoF'),
  ministry('DST', 'Department of Science and Technology', 'DST'),
  ministry('DAE', 'Department of Atomic Energy', 'DAE', { divisibility: 'NonDivisible' }),
  ministry('DOP', 'Department of Pharmaceuticals', 'DoP', {
    para3aApplicable: true,
    pliLinked: true,
  }),
];

/** 21 — the figure the DPIIT console reports as "ministries onboarded". */
export const MINISTRY_COUNT = MINISTRIES.length;

const MINISTRY_BY_ID = new Map(MINISTRIES.map((row) => [row.id, row]));

export function getMinistry(id: string): MinistryRef | undefined {
  return MINISTRY_BY_ID.get(id);
}

export function ministryName(id: string): string {
  return MINISTRY_BY_ID.get(id)?.name ?? id;
}

export function ministryShort(id: string): string {
  return MINISTRY_BY_ID.get(id)?.short ?? id;
}

/** Mirrors `Pallet::thresholds_for_rule`: the first configured entry, DPIIT default if none. */
export function thresholdsFor(ministryId: string): { classOneBps: number; classTwoBps: number } {
  const first = MINISTRY_BY_ID.get(ministryId)?.hsnThresholds[0];
  return first
    ? { classOneBps: first.classOneBps, classTwoBps: first.classTwoBps }
    : { classOneBps: 5_000, classTwoBps: 2_000 };
}

// =====================================================================================
// The demonstration tender book
// =====================================================================================

/**
 * GeM-format tender records the consoles bid into.
 *
 * These are the *inputs* to a demo, in the format and field vocabulary of a real GeM bid
 * document (bid number `GEM/YYYY/B/NNNNNNN`, Buyer Organisation, Item Category, Total
 * Quantity, EMD, ePBG, Evaluation Method). They are not chain state and no console ever
 * presents them as a verdict: a classification, a preference outcome or a certificate
 * appears on screen only after a trigger-point route has answered.
 *
 * Vendor names follow the PoC document's own idiom — an Indian river or place name plus
 * a sector noun — so no real company is ever named against a compliance verdict.
 */
export interface VendorRef {
  /** Sent as the `vendor` field: a derivation URI, or the `vendor` demo persona. */
  account: AccountRef;
  name: string;
  msme: 'Micro' | 'Small' | 'Medium' | 'Not Registered';
  /** Micro and Small carry the GeM MSE purchase preference; Medium does not. */
  isMse: boolean;
  city: string;
  state: string;
  udyam: string;
}

export const VENDORS: readonly VendorRef[] = [
  {
    account: 'vendor',
    name: 'Bharat Precision Instruments Pvt Ltd',
    msme: 'Small',
    isMse: true,
    city: 'Pune',
    state: 'Maharashtra',
    udyam: 'UDYAM-MH-26-0041827',
  },
  {
    account: '//SabarmatiMicrosystems',
    name: 'Sabarmati Microsystems Private Limited',
    msme: 'Micro',
    isMse: true,
    city: 'Sanand',
    state: 'Gujarat',
    udyam: 'UDYAM-GJ-11-0008913',
  },
  {
    account: '//ChambalDevices',
    name: 'Chambal Devices Private Limited',
    msme: 'Small',
    isMse: true,
    city: 'Bhiwadi',
    state: 'Rajasthan',
    udyam: 'UDYAM-RJ-08-0027455',
  },
  {
    account: '//GodavariSystems',
    name: 'Godavari Systems Limited',
    msme: 'Not Registered',
    isMse: false,
    city: 'Hyderabad',
    state: 'Telangana',
    udyam: '—',
  },
  {
    account: '//NarmadaRollingStock',
    name: 'Narmada Rolling Stock Private Limited',
    msme: 'Small',
    isMse: true,
    city: 'Pithampur',
    state: 'Madhya Pradesh',
    udyam: 'UDYAM-MP-23-0013304',
  },
  {
    account: '//KaveriForgings',
    name: 'Kaveri Forgings Limited',
    msme: 'Medium',
    isMse: false,
    city: 'Coimbatore',
    state: 'Tamil Nadu',
    udyam: 'UDYAM-TN-33-0004120',
  },
  {
    account: '//TungabhadraNetworks',
    name: 'Tungabhadra Networks Private Limited',
    msme: 'Small',
    isMse: true,
    city: 'Hubballi',
    state: 'Karnataka',
    udyam: 'UDYAM-KR-29-0019662',
  },
  {
    account: '//MahanadiInfratech',
    name: 'Mahanadi Infratech Private Limited',
    msme: 'Medium',
    isMse: false,
    city: 'Rourkela',
    state: 'Odisha',
    udyam: 'UDYAM-OD-21-0006781',
  },
  {
    account: '//SutlejPowerSystems',
    name: 'Sutlej Power Systems Private Limited',
    msme: 'Small',
    isMse: true,
    city: 'Ludhiana',
    state: 'Punjab',
    udyam: 'UDYAM-PB-03-0022190',
  },
];

const VENDOR_BY_ACCOUNT = new Map(VENDORS.map((row) => [row.account, row]));

export function vendorName(account: string): string {
  return VENDOR_BY_ACCOUNT.get(account)?.name ?? account;
}

export function getVendor(account: string): VendorRef | undefined {
  return VENDOR_BY_ACCOUNT.get(account);
}

/** One bid received on a tender, as the buyer sees it on the GeM evaluation screen. */
export interface TenderBid {
  vendor: AccountRef;
  /** What the vendor declared on the bid form, in basis points. */
  declaredLocalContentBps: number;
  pricePaise: string;
  /** The vendor has claimed a Production Linked Incentive for this category. */
  isPliManufacturer?: boolean;
}

export interface TenderRef {
  /** GeM bid number, and the `tender` id sent to every route. */
  id: string;
  ministryId: string;
  buyerOrganisation: string;
  itemCategory: string;
  hsnCode: string;
  /** The consistency history is kept per (vendor, product), not per tender. */
  productId: string;
  totalQuantity: number;
  unit: string;
  valuePaise: string;
  bidType: 'Standard Bid' | 'Custom Catalogue-Based Bid' | 'BOQ Bid' | 'Bid-to-RA';
  packetType: 'Single Packet Bid' | 'Two Packet Bid';
  evaluationMethod: 'Total value wise evaluation' | 'Item wise evaluation';
  bidEndDate: string;
  emdPaise: string;
  epbgPercent: number;
  /** Global tender enquiry approved under GFR Rule 161(iv). Read by the P6 gate. */
  isTenderGte: boolean;
  bids: TenderBid[];
}

export const TENDERS: readonly TenderRef[] = [
  {
    id: 'GEM/2025/B/6798497',
    ministryId: 'MEITY',
    buyerOrganisation: 'National Informatics Centre, Ministry of Electronics and IT',
    itemCategory: 'Desktop Computer (All in One)',
    hsnCode: '8471',
    productId: 'PRD-8471-AIO-DESKTOP',
    totalQuantity: 1_200,
    unit: 'Nos',
    valuePaise: '69600000000',
    bidType: 'BOQ Bid',
    packetType: 'Two Packet Bid',
    evaluationMethod: 'Total value wise evaluation',
    bidEndDate: '18 August 2025, 15:00',
    emdPaise: '696000000',
    epbgPercent: 3,
    isTenderGte: false,
    bids: [
      { vendor: '//GodavariSystems', declaredLocalContentBps: 1_500, pricePaise: '64200000000' },
      { vendor: '//SabarmatiMicrosystems', declaredLocalContentBps: 5_800, pricePaise: '71400000000' },
      { vendor: 'vendor', declaredLocalContentBps: 3_200, pricePaise: '68900000000' },
    ],
  },
  {
    id: 'GEM/2025/B/6563045',
    ministryId: 'MOR',
    buyerOrganisation: 'Central Procurement Cell, Northern Railway',
    itemCategory: 'Cast Steel Bogie for BOXNHL Wagon',
    hsnCode: '8607',
    productId: 'PRD-8607-BOGIE-BOXNHL',
    totalQuantity: 220,
    unit: 'Nos',
    valuePaise: '13420000000',
    bidType: 'Standard Bid',
    packetType: 'Two Packet Bid',
    evaluationMethod: 'Total value wise evaluation',
    bidEndDate: '02 September 2025, 12:00',
    emdPaise: '268400000',
    epbgPercent: 5,
    isTenderGte: false,
    bids: [
      { vendor: '//KaveriForgings', declaredLocalContentBps: 4_400, pricePaise: '12980000000' },
      { vendor: '//NarmadaRollingStock', declaredLocalContentBps: 6_700, pricePaise: '14100000000' },
      { vendor: '//MahanadiInfratech', declaredLocalContentBps: 1_800, pricePaise: '12610000000' },
    ],
  },
  {
    id: 'GEM/2025/B/6712330',
    ministryId: 'DOT',
    buyerOrganisation: 'Bharat Broadband Network Limited, Department of Telecommunications',
    itemCategory: 'Optical Fibre Cable (24F, Armoured)',
    hsnCode: '8544',
    productId: 'PRD-8544-OFC-24F',
    totalQuantity: 850,
    unit: 'Km',
    valuePaise: '3230000000',
    bidType: 'Bid-to-RA',
    packetType: 'Single Packet Bid',
    evaluationMethod: 'Item wise evaluation',
    bidEndDate: '26 August 2025, 17:00',
    emdPaise: '32300000',
    epbgPercent: 3,
    isTenderGte: false,
    bids: [
      { vendor: '//TungabhadraNetworks', declaredLocalContentBps: 6_200, pricePaise: '3310000000' },
      { vendor: '//GodavariSystems', declaredLocalContentBps: 2_400, pricePaise: '2980000000' },
      { vendor: '//ChambalDevices', declaredLocalContentBps: 8_600, pricePaise: '3190000000' },
    ],
  },
  {
    id: 'GEM/2025/B/6390218',
    ministryId: 'MOHUA',
    buyerOrganisation: 'Municipal Corporation Engineering Wing, Ministry of Housing and Urban Affairs',
    itemCategory: 'LED Street Light Luminaire (90 W)',
    hsnCode: '9405',
    productId: 'PRD-9405-LED-90W',
    totalQuantity: 12_000,
    unit: 'Nos',
    valuePaise: '5880000000',
    bidType: 'Custom Catalogue-Based Bid',
    packetType: 'Single Packet Bid',
    evaluationMethod: 'Total value wise evaluation',
    bidEndDate: '11 August 2025, 15:00',
    emdPaise: '58800000',
    epbgPercent: 3,
    isTenderGte: false,
    bids: [
      { vendor: '//MahanadiInfratech', declaredLocalContentBps: 5_200, pricePaise: '5990000000' },
      { vendor: '//SutlejPowerSystems', declaredLocalContentBps: 3_100, pricePaise: '5640000000' },
    ],
  },
  {
    id: 'GEM/2025/B/6844911',
    ministryId: 'MNRE',
    buyerOrganisation: 'Solar Energy Corporation of India, Ministry of New and Renewable Energy',
    itemCategory: 'Solar Photovoltaic Module (540 Wp, Mono PERC)',
    hsnCode: '8541',
    productId: 'PRD-8541-SPV-540WP',
    totalQuantity: 24_000,
    unit: 'Nos',
    valuePaise: '30240000000',
    bidType: 'BOQ Bid',
    packetType: 'Two Packet Bid',
    evaluationMethod: 'Total value wise evaluation',
    bidEndDate: '05 September 2025, 15:00',
    emdPaise: '302400000',
    epbgPercent: 5,
    isTenderGte: false,
    bids: [
      { vendor: '//SutlejPowerSystems', declaredLocalContentBps: 5_500, pricePaise: '31100000000' },
      { vendor: '//GodavariSystems', declaredLocalContentBps: 900, pricePaise: '28900000000' },
      { vendor: '//SabarmatiMicrosystems', declaredLocalContentBps: 2_600, pricePaise: '30800000000' },
    ],
  },
  {
    id: 'GEM/2025/B/6907254',
    ministryId: 'DDP',
    buyerOrganisation: 'Directorate of Procurement, Department of Defence Production',
    itemCategory: 'Armoured Vehicle Transmission Assembly',
    hsnCode: '8708',
    productId: 'PRD-8708-AVT-ASSY',
    totalQuantity: 45,
    unit: 'Nos',
    valuePaise: '18900000000',
    bidType: 'Standard Bid',
    packetType: 'Two Packet Bid',
    evaluationMethod: 'Total value wise evaluation',
    bidEndDate: '22 September 2025, 12:00',
    emdPaise: '378000000',
    epbgPercent: 10,
    isTenderGte: false,
    bids: [
      { vendor: '//KaveriForgings', declaredLocalContentBps: 5_900, pricePaise: '18400000000' },
      { vendor: '//NarmadaRollingStock', declaredLocalContentBps: 4_100, pricePaise: '17950000000' },
    ],
  },
];

const TENDER_BY_ID = new Map(TENDERS.map((row) => [row.id, row]));

export function getTender(id: string): TenderRef | undefined {
  return TENDER_BY_ID.get(id);
}

/** Contracts at or above their ministry's threshold need a chartered accountant (P11). */
export function requiresAuditorCertificate(tender: TenderRef): boolean {
  const threshold = getMinistry(tender.ministryId)?.certificationThresholdPaise;
  if (!threshold) return false;
  return BigInt(tender.valuePaise) >= BigInt(threshold);
}

/**
 * Which of P1–P12 a `classify` call took, derived from the pallet's own branch order in
 * `pallet-pramaan-classification::classify`. Only stated where the branch is unambiguous.
 */
export function classificationPathway(
  ministryId: string,
  classResult: ClassResult | null | undefined,
  isPliManufacturer: boolean,
): 'P1' | 'P3' | 'P4' | undefined {
  if (!classResult) return undefined;
  if (classResult === 'ManualReviewRequired') return 'P3';
  const rule = getMinistry(ministryId);
  if (rule?.pliLinked && isPliManufacturer && classResult === 'ClassTwo') return 'P4';
  if (rule?.calculationMethod === 'Custom') return 'P3';
  return 'P1';
}

// =====================================================================================
// The browser-local console ledger
// =====================================================================================

/**
 * What this session actually did, kept in `localStorage` so the CVC, auditor and DPIIT
 * consoles can report across pages.
 *
 * Every entry is written from a trigger-point response, so every row carries the reason
 * the chain gave and — unless the decision came from a storage read with no extrinsic —
 * the transaction reference and finalized block it was recorded in. Nothing is invented:
 * an empty ledger renders an EmptyState, never a plausible-looking row.
 *
 * This is a console-side convenience, not an audit source. The audit source is the chain;
 * the explorer reads it.
 */
export type LedgerKind =
  | 'classification'
  | 'evaluation'
  | 'preference'
  | 'certification'
  | 'debarment'
  | 'rule-update';

export interface LedgerEntry {
  id: string;
  /** ISO timestamp of when the console recorded it. */
  at: string;
  kind: LedgerKind;
  /** Which console produced it. */
  persona: string;
  result: TriState;
  headline: string;
  reason: string;
  tender?: string;
  ministry?: string;
  vendor?: string;
  txRef?: string | null;
  blockNumber?: number | null;
  pathway?: string | null;
  /** Kind-specific facts, e.g. a certificate id or an award share. */
  facts?: Record<string, string>;
}

/** A declaration that contradicted the same vendor's earlier one for the same product. */
export interface InconsistencyEntry {
  id: string;
  at: string;
  vendor: string;
  product: string;
  tender: string;
  declaredBps: number;
  priorBps: number | null;
  priorTender: string | null;
  ministry: string;
  txRef?: string | null;
  blockNumber?: number | null;
}

interface ConsoleStore {
  ledger: LedgerEntry[];
  inconsistencies: InconsistencyEntry[];
}

const STORE_KEY = 'pramaan.console.v1';
const MAX_ENTRIES = 200;
const EMPTY_STORE: ConsoleStore = { ledger: [], inconsistencies: [] };

function readStore(): ConsoleStore {
  if (typeof window === 'undefined') return EMPTY_STORE;
  try {
    const raw = window.localStorage.getItem(STORE_KEY);
    if (!raw) return EMPTY_STORE;
    const parsed = JSON.parse(raw) as Partial<ConsoleStore>;
    return {
      ledger: Array.isArray(parsed.ledger) ? parsed.ledger : [],
      inconsistencies: Array.isArray(parsed.inconsistencies) ? parsed.inconsistencies : [],
    };
  } catch {
    return EMPTY_STORE;
  }
}

function writeStore(store: ConsoleStore): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(STORE_KEY, JSON.stringify(store));
    window.dispatchEvent(new CustomEvent('pramaan:ledger'));
  } catch {
    // A full or disabled localStorage must never break a live demo. The verdict on screen
    // is the product; this ledger is a convenience layered on top of it.
  }
}

function newId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}

export function recordLedgerEntry(entry: Omit<LedgerEntry, 'id' | 'at'>): LedgerEntry {
  const full: LedgerEntry = { ...entry, id: newId(), at: new Date().toISOString() };
  const store = readStore();
  writeStore({ ...store, ledger: [full, ...store.ledger].slice(0, MAX_ENTRIES) });
  return full;
}

export function recordInconsistency(
  entry: Omit<InconsistencyEntry, 'id' | 'at'>,
): InconsistencyEntry {
  const full: InconsistencyEntry = { ...entry, id: newId(), at: new Date().toISOString() };
  const store = readStore();
  writeStore({
    ...store,
    inconsistencies: [full, ...store.inconsistencies].slice(0, MAX_ENTRIES),
  });
  return full;
}

export function readLedger(): LedgerEntry[] {
  return readStore().ledger;
}

export function readInconsistencies(): InconsistencyEntry[] {
  return readStore().inconsistencies;
}

export function clearConsoleRecord(): void {
  writeStore(EMPTY_STORE);
}

/** Subscribe to console-record changes, including those made in another tab. */
export function subscribeConsoleRecord(listener: () => void): () => void {
  if (typeof window === 'undefined') return () => {};
  window.addEventListener('pramaan:ledger', listener);
  window.addEventListener('storage', listener);
  return () => {
    window.removeEventListener('pramaan:ledger', listener);
    window.removeEventListener('storage', listener);
  };
}
