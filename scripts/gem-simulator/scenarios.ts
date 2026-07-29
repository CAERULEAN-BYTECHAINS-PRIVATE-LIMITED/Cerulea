/**
 * The twelve pathway scenarios.
 *
 * One scenario per `pramaan_primitives::PathwayId`. Each one drives the exact sequence
 * of trigger-point calls that its pathway represents and asserts, against the response,
 * that the chain decided what the pathway says it must decide. `pathwayText` on each
 * scenario is the PoC document's own definition, transcribed verbatim from the doc
 * comment on the matching `PathwayId` variant, so the claim being tested and the test
 * sit next to each other.
 *
 * These are genuine tests. Every scenario contains at least one call that MUST be
 * refused, so a stub route that returns `{ result: 'GREEN' }` to everything fails ten of
 * the twelve immediately.
 *
 * Reading this file is the intended way to review the suite: each step is a request body
 * plus a list of declarative expectations, with a `why` on every expectation.
 */

import type { ApiCall } from './api';
import type { Expectation } from './checks';
import {
  DEFAULT_AS_OF,
  makeBid,
  makeCertificateId,
  makeVendor,
  ruleWithOverrides,
  toChainRule,
  type BidOptions,
  type VendorOptions,
} from './generate';
import { crorePaise, ministry, rupeesPaise } from './gem-data';
import { rngFor, type Rng } from './rng';
import { accountIdFor } from './ss58';
import type { GemBid, GemVendor, PathwayId, PreferenceBid } from './types';

// ---------------------------------------------------------------------------------
// Scenario shape
// ---------------------------------------------------------------------------------

export interface Step {
  /** Short label printed in the run log. */
  label: string;
  /** One sentence a presenter can read aloud while the step runs. */
  narrate: string;
  call: ApiCall;
  expect: Expectation[];
}

export interface Scenario {
  id: PathwayId;
  title: string;
  /** Verbatim from `pramaan_primitives::PathwayId`. */
  pathwayText: string;
  /** What passing this scenario proves. */
  proves: string;
  /** The GeM records this scenario generated, rendered for `--show-records`. */
  vendors: GemVendor[];
  bids: GemBid[];
  steps: Step[];
}

export interface ScenarioOptions {
  seed: number;
  runId: string;
  asOf: string;
}

/**
 * Per-scenario generation context. Its RNG is derived from `(seed, pathwayId)` so a
 * scenario produces identical records whether it is run alone or as part of `--all`.
 */
class Ctx {
  readonly rng: Rng;
  readonly vendors: GemVendor[] = [];
  readonly bids: GemBid[] = [];

  constructor(
    readonly id: PathwayId,
    readonly opts: ScenarioOptions,
  ) {
    this.rng = rngFor(opts.seed, id);
  }

  vendor(name: string, extra: Omit<VendorOptions, 'label'> = {}): GemVendor {
    const vendor = makeVendor(this.rng, this.opts.seed, { label: `${this.id}/${name}`, ...extra });
    this.vendors.push(vendor);
    return vendor;
  }

  bid(options: Omit<BidOptions, 'runId' | 'asOf'>): GemBid {
    const bid = makeBid(this.rng, { ...options, runId: this.opts.runId, asOf: this.opts.asOf });
    this.bids.push(bid);
    return bid;
  }

  certificateId(): string {
    return makeCertificateId(this.rng, 2026, this.opts.runId);
  }

  /**
   * The demo's auditor: an account holding the CvcOrAuditReviewer role. The runtime
   * wires `AuditorSource = AnyAccountIsAuditor` for this build (build contract section
   * 2), so any signed account is accepted; the persona name is sent alongside the
   * address so a route that resolves personas instead of addresses still works.
   */
  auditorAccountId(): string {
    return accountIdFor(this.opts.seed, 'auditor/cost-accountant-panel');
  }
}

// ---------------------------------------------------------------------------------
// Shared request-body builders. Field names follow the build contract: monetary fields
// carry a `Paise` suffix, percentages carry `Bps`.
// ---------------------------------------------------------------------------------

interface SubmissionOptions {
  vendor: GemVendor;
  bid: GemBid;
  declaredLocalContentBps?: number;
  isPliManufacturer?: boolean;
  components?: Array<{ name: string; declaredBps: number; weightBps: number }>;
}

function bidSubmission(o: SubmissionOptions): ApiCall {
  const body: Record<string, unknown> = {
    // Contract fields.
    vendor: o.vendor.accountId,
    tender: o.bid.tenderId,
    ministry: o.bid.ministryId,
    // Carried so the route can also record the declaration against the vendor's history
    // (pallet-pramaan-consistency keys on `(vendor, product)`).
    product: o.bid.productId,
    // GeM context, so the route and the UI can show a real bid rather than three ids.
    bidNumber: o.bid.bidNumber,
    vendorName: o.vendor.legalName,
    itemCategory: o.bid.itemCategory,
    hsnCode: o.bid.hsnCode,
    buyerOrganisation: o.bid.organisationName,
    isMse: o.vendor.isMse,
  };
  if (o.components) {
    // Presence of `components` selects `classification.classifyComponentLevel`.
    body.components = o.components;
  } else {
    body.declaredLocalContentBps = o.declaredLocalContentBps ?? 0;
  }
  body.isPliManufacturer = o.isPliManufacturer ?? false;
  return { route: 'bid-submission', body: body as ApiCall['body'] };
}

function bidEvaluation(vendor: GemVendor, bid: GemBid, declaredLocalContentBps: number): ApiCall {
  return {
    route: 'bid-evaluation',
    body: {
      vendor: vendor.accountId,
      tender: bid.tenderId,
      ministry: bid.ministryId,
      // The evaluation route reads debarment and then classifies; the declaration is
      // carried so it can classify without a second round trip to fetch the bid.
      declaredLocalContentBps,
      vendorName: vendor.legalName,
      bidNumber: bid.bidNumber,
    },
  };
}

function preferenceCalculation(
  bid: GemBid,
  bids: PreferenceBid[],
  tenderValuePaise: bigint,
  isTenderGte: boolean,
): ApiCall {
  return {
    route: 'preference-calculation',
    body: {
      tender: bid.tenderId,
      ministry: bid.ministryId,
      tenderValuePaise: tenderValuePaise.toString(),
      isTenderGte,
      bidNumber: bid.bidNumber,
      evaluationMethod: bid.evaluationMethod,
      bids: bids.map((b) => ({
        vendor: b.vendor,
        vendorName: b.vendorName,
        class: b.class,
        pricePaise: b.pricePaise,
        isMse: b.isMse,
        isGte: b.isGte,
      })),
    },
  };
}

function caCertification(o: {
  certificateId: string;
  vendor: GemVendor;
  bid: GemBid;
  valuePaise: bigint;
  auditor: string | null;
}): ApiCall {
  return {
    route: 'ca-certification',
    body: {
      certificateId: o.certificateId,
      ministry: o.bid.ministryId,
      vendor: o.vendor.accountId,
      tender: o.bid.tenderId,
      valuePaise: o.valuePaise.toString(),
      auditor: o.auditor,
      auditorPersona: o.auditor === null ? null : 'auditor',
      vendorName: o.vendor.legalName,
      bidNumber: o.bid.bidNumber,
    },
  };
}

function debarmentCall(o: {
  vendor: GemVendor;
  ministryId: string;
  action: 'debar' | 'lift';
  effectiveFrom?: number;
  effectiveTo?: number | null;
  reason?: string;
}): ApiCall {
  return {
    route: 'debarment',
    body: {
      vendor: o.vendor.accountId,
      ministry: o.ministryId,
      action: o.action,
      // Block numbers, not timestamps -- this runtime has no `pallet_timestamp`.
      effectiveFrom: o.effectiveFrom ?? 0,
      effectiveTo: o.effectiveTo ?? null,
      reason: o.reason ?? '',
      vendorName: o.vendor.legalName,
    },
  };
}

function ruleUpdate(ministryId: string, rule: ReturnType<typeof toChainRule>): ApiCall {
  return {
    route: 'rule-update',
    body: { ministry: ministryId, rule: rule as unknown as ApiCall['body'][string] },
  };
}

function bidOf(vendor: GemVendor, cls: PreferenceBid['class'], pricePaise: bigint, isMse = false): PreferenceBid {
  return {
    vendor: vendor.accountId,
    vendorName: vendor.legalName,
    class: cls,
    pricePaise: pricePaise.toString(),
    isMse,
    isGte: false,
  };
}

/** Expectations every accepted call must satisfy. */
function accepted(why: string): Expectation[] {
  return [
    { kind: 'http-ok', why },
    {
      kind: 'finalized',
      why: 'the answer was returned only after the block carrying it reached DCF finality',
    },
  ];
}

// ---------------------------------------------------------------------------------
// P1 -- standard formula, and the three classification boundaries
// ---------------------------------------------------------------------------------

function buildP1(ctx: Ctx): Scenario {
  // DPIIT's own rule set is the DPIIT default (Class-I 50%, Class-II 20%), which makes
  // it the right ministry to demonstrate the canonical boundaries on.
  const bid = ctx.bid({
    ministryId: 'DPIIT',
    label: 'p1-ac',
    itemCategoryName: 'Split Air Conditioner',
    bidType: 'Standard Bid',
  });
  const exactlyFifty = ctx.vendor('exactly-50pc', { sector: 'general', isMse: false });
  const exactlyTwenty = ctx.vendor('exactly-20pc', { sector: 'general', isMse: true });
  const justUnderTwenty = ctx.vendor('19-99pc', { sector: 'general', isMse: true });

  return {
    id: 'P1',
    title: 'Standard formula and the Class-I / Class-II / Non-local boundaries',
    pathwayText: 'P1 applies the standard formula against the applicable threshold.',
    proves:
      'The boundaries are inclusive at the lower end. Exactly 50% is Class-I, exactly 20% ' +
      'is Class-II and not Non-local, and 19.99% is Non-local. These three are the cases ' +
      'the PoC document Section 7.1 calls out by name, and they are one basis point apart.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'declare exactly 50.00% (5000 bps)',
        narrate: `${exactlyFifty.legalName} declares exactly 50 percent local content on ${bid.bidNumber}.`,
        call: bidSubmission({ vendor: exactlyFifty, bid, declaredLocalContentBps: 5_000 }),
        expect: [
          ...accepted('a declaration at the threshold is a valid submission, not an error'),
          {
            kind: 'class',
            equals: 'ClassOne',
            why: 'exactly 50% is Class-I: the boundary is inclusive at the lower end',
          },
          { kind: 'result', equals: 'GREEN', why: 'a Class-I supplier is compliant and proceeds' },
        ],
      },
      {
        label: 'declare exactly 20.00% (2000 bps)',
        narrate: `${exactlyTwenty.legalName} declares exactly 20 percent -- the Class-II boundary.`,
        call: bidSubmission({ vendor: exactlyTwenty, bid, declaredLocalContentBps: 2_000 }),
        expect: [
          ...accepted('the call is accepted; the caveat is in the answer, not in an error'),
          {
            kind: 'class',
            equals: 'ClassTwo',
            why: 'exactly 20% is Class-II rather than Non-local -- the single most commonly mis-implemented boundary in this order',
          },
          { kind: 'result', equals: 'YELLOW', why: 'Class-II proceeds with a caveat' },
        ],
      },
      {
        label: 'declare 19.99% (1999 bps)',
        narrate: `${justUnderTwenty.legalName} declares 19.99 percent -- one basis point below the boundary.`,
        call: bidSubmission({ vendor: justUnderTwenty, bid, declaredLocalContentBps: 1_999 }),
        expect: [
          { kind: 'http-ok', why: 'a Non-local classification is an answer, not a server error' },
          {
            kind: 'class',
            equals: 'NonLocal',
            why: 'one basis point below 20% is Non-local -- proving the comparison is >= and not >',
          },
          { kind: 'result', equals: 'RED', why: 'a Non-local supplier is blocked on a domestic tender' },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P2 -- component-level / weighted-module
// ---------------------------------------------------------------------------------

const DESKTOP_COMPONENTS_CLASS_TWO = [
  { name: 'Motherboard Assembly', declaredBps: 3_500, weightBps: 2_500 },
  { name: 'Chassis and Sheet Metal', declaredBps: 9_000, weightBps: 1_500 },
  { name: 'SMPS Power Supply', declaredBps: 7_000, weightBps: 1_000 },
  { name: 'Display Panel', declaredBps: 2_000, weightBps: 3_000 },
  { name: 'SSD Storage Module', declaredBps: 4_000, weightBps: 2_000 },
];

const DESKTOP_COMPONENTS_CLASS_ONE = [
  { name: 'Motherboard Assembly', declaredBps: 6_000, weightBps: 2_500 },
  { name: 'Chassis and Sheet Metal', declaredBps: 9_500, weightBps: 1_500 },
  { name: 'SMPS Power Supply', declaredBps: 8_000, weightBps: 1_000 },
  { name: 'Display Panel', declaredBps: 3_000, weightBps: 3_000 },
  { name: 'SSD Storage Module', declaredBps: 5_000, weightBps: 2_000 },
];

/** Same components, one weight wrong, so the weights total 9500 instead of 10000. */
const DESKTOP_COMPONENTS_BAD_WEIGHTS = DESKTOP_COMPONENTS_CLASS_TWO.map((c) =>
  c.name === 'Display Panel' ? { ...c, weightBps: 2_500 } : c,
);

function buildP2(ctx: Ctx): Scenario {
  // MeitY is the one ministry seeded with ComponentLevel, per its 07.09.2020 notification.
  const bid = ctx.bid({
    ministryId: 'MEITY',
    label: 'p2-desktop',
    itemCategoryName: 'Desktop Computer (All in One)',
    bidType: 'BOQ Bid',
  });
  const integrator = ctx.vendor('integrator', { sector: 'electronics', isMse: false });

  return {
    id: 'P2',
    title: 'Component-level and weighted-module validation',
    pathwayText:
      'P2 handles component-level and weighted-module methods, validating each component ' +
      'against its own condition before aggregating.',
    proves:
      'The aggregate is a weighted average, not an average of the component percentages. ' +
      'The first declaration averages 51% unweighted -- which would read Class-I -- but ' +
      'weights to 43.25%, which is Class-II. A weight set that does not total 100% is refused ' +
      'outright rather than silently normalised.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'five components, weighted average 43.25%',
        narrate: `${integrator.legalName} declares five components on ${bid.bidNumber}; the panel carries 30 percent of the value and only 20 percent local content.`,
        call: bidSubmission({ vendor: integrator, bid, components: DESKTOP_COMPONENTS_CLASS_TWO }),
        expect: [
          ...accepted('a component-level declaration is a normal submission'),
          {
            kind: 'class',
            equals: 'ClassTwo',
            why: 'weighted average 4325 bps is Class-II; the unweighted mean of the same five figures is 5100 bps and would wrongly read Class-I',
          },
          { kind: 'result', equals: 'YELLOW', why: 'Class-II proceeds with a caveat' },
        ],
      },
      {
        label: 'same components with domestic panel sourcing, weighted average 56.25%',
        narrate: 'The bidder re-sources the display panel domestically and re-declares.',
        call: bidSubmission({ vendor: integrator, bid, components: DESKTOP_COMPONENTS_CLASS_ONE }),
        expect: [
          ...accepted('re-declaring against the same tender overwrites the prior classification'),
          {
            kind: 'class',
            equals: 'ClassOne',
            why: 'weighted average 5625 bps crosses the 50% Class-I threshold',
          },
          { kind: 'result', equals: 'GREEN', why: 'Class-I is compliant and proceeds' },
        ],
      },
      {
        label: 'weights totalling 95%, which must be refused',
        narrate: 'A declaration whose component weights add up to 95 percent is submitted.',
        call: bidSubmission({ vendor: integrator, bid, components: DESKTOP_COMPONENTS_BAD_WEIGHTS }),
        expect: [
          {
            kind: 'blocked',
            names: ['WeightsDoNotSumToOneHundredPercent', 'weight'],
            why: 'component weights must total exactly 10000 bps; a short weight set is refused, never normalised, because normalising would silently invent local content',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P3 -- no automatable formula, routed to a human
// ---------------------------------------------------------------------------------

function buildP3(ctx: Ctx): Scenario {
  // DST's seeded rule uses the Standard method. This scenario updates it to Custom for
  // a bespoke-software procurement, classifies under it, and then restores the seeded
  // rule -- so it is safe to run repeatedly and in any order.
  const original = toChainRule(ministry('DST'));
  const custom = ruleWithOverrides('DST', { calculationMethod: 'Custom' });

  const softwareBid = ctx.bid({
    ministryId: 'DST',
    label: 'p3-lims',
    itemCategoryName: 'Laboratory Information Management Software',
    bidType: 'Custom Catalogue-Based Bid',
  });
  const afterRestoreBid = ctx.bid({
    ministryId: 'DST',
    label: 'p3-spectro',
    itemCategoryName: 'UV-Visible Spectrophotometer',
    bidType: 'Standard Bid',
  });
  const softwareHouse = ctx.vendor('software-house', { sector: 'electronics', isMse: true });

  return {
    id: 'P3',
    title: 'No automatable formula, routed to a recorded human decision',
    pathwayText:
      'P3 covers categories where no automatable formula exists and routes them to a ' +
      'recorded human decision rather than computing one.',
    proves:
      'A rule change is a storage update, not a redeploy. DST is switched to the Custom ' +
      'calculation method by API call, a software bid under it returns ManualReviewRequired ' +
      'instead of a fabricated percentage, and the seeded rule is then restored -- with a ' +
      'second bid proving the restore actually took effect.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'DPIIT sets DST to the Custom calculation method',
        narrate:
          'DPIIT updates the Department of Science and Technology rule so bespoke software uses the Custom method, because there is no sale-price-minus-imported-content figure for software.',
        call: ruleUpdate('DST', custom),
        expect: [
          ...accepted('a rule update is a configuration change, applied without a redeploy'),
          {
            kind: 'field-present',
            names: ['newVersion', 'version', 'ruleVersion'],
            why: 'every rule change increments a version, which is what makes the audit history reconstructable',
          },
        ],
      },
      {
        label: 'software bid classified under the Custom method',
        narrate: `${softwareHouse.legalName} declares 72 percent local content on the LIMS licence bid.`,
        call: bidSubmission({ vendor: softwareHouse, bid: softwareBid, declaredLocalContentBps: 7_200 }),
        expect: [
          ...accepted('the submission is accepted; it is the answer that defers to a human'),
          {
            kind: 'class',
            equals: 'ManualReviewRequired',
            why: 'under Custom the engine refuses to compute a number it has no formula for, and records that a human must decide -- it does not fall back to the standard formula',
          },
          {
            kind: 'result',
            equals: 'YELLOW',
            why: 'manual review is a caveat awaiting a human, not a block',
          },
        ],
      },
      {
        label: 'DPIIT restores the seeded DST rule',
        narrate: 'DPIIT reverts the Department of Science and Technology to its seeded Standard method.',
        call: ruleUpdate('DST', original),
        expect: [
          ...accepted('the restore must succeed so the scenario is safe to run again'),
          {
            kind: 'field-present',
            names: ['newVersion', 'version', 'ruleVersion'],
            why: 'the restore is itself a versioned change, not an erasure of the Custom period',
          },
        ],
      },
      {
        label: 'the same declaration is computable again after the restore',
        narrate: 'A spectrophotometer bid at the same 72 percent is submitted under the restored rule.',
        call: bidSubmission({
          vendor: softwareHouse,
          bid: afterRestoreBid,
          declaredLocalContentBps: 7_200,
        }),
        expect: [
          ...accepted('the restored rule is in force for the very next call'),
          {
            kind: 'class',
            equals: 'ClassOne',
            why: '72% against the restored Standard method is Class-I -- proving the Custom method was genuinely in force for the previous step and is genuinely gone now',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P4 -- the PLI deeming rule
// ---------------------------------------------------------------------------------

function buildP4(ctx: Ctx): Scenario {
  // DoT is seeded with pli_linked = true and a raised Class-I threshold of 60%.
  const controlBid = ctx.bid({
    ministryId: 'DOT',
    label: 'p4-handset-control',
    itemCategoryName: 'Cellular Mobile Handset (4G)',
    bidType: 'Standard Bid',
  });
  const pliBid = ctx.bid({
    ministryId: 'DOT',
    label: 'p4-handset-pli',
    itemCategoryName: 'Cellular Mobile Handset (4G)',
    bidType: 'Standard Bid',
  });
  const handsetMaker = ctx.vendor('handset-maker', {
    sector: 'telecom',
    isMse: false,
    isPliBeneficiary: true,
  });

  return {
    id: 'P4',
    title: 'PLI deeming caps a manufacturer at Class-II',
    pathwayText:
      'P4 applies the PLI deeming rule, which treats a manufacturer as Class-II only where ' +
      'the incentive has been received and only for the period the PLI ministry notified.',
    proves:
      'The deeming rule overrides the declared percentage rather than sitting alongside it. ' +
      'The same 80% declaration by the same vendor under the same ministry is Class-I when no ' +
      'PLI incentive has been received and Class-II when one has. The two calls differ in ' +
      'exactly one field, so nothing else can explain the difference.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'control: 80% declared, no PLI incentive received',
        narrate: `${handsetMaker.legalName} declares 80 percent local content on ${controlBid.bidNumber}, having drawn no PLI incentive.`,
        call: bidSubmission({
          vendor: handsetMaker,
          bid: controlBid,
          declaredLocalContentBps: 8_000,
          isPliManufacturer: false,
        }),
        expect: [
          ...accepted('an ordinary telecom submission'),
          {
            kind: 'class',
            equals: 'ClassOne',
            why: '80% clears the Department of Telecommunications raised 60% Class-I threshold',
          },
          { kind: 'result', equals: 'GREEN', why: 'Class-I proceeds' },
        ],
      },
      {
        label: 'the identical declaration from a PLI beneficiary',
        narrate: 'The same manufacturer bids on a second handset tender, this time having received the Telecom PLI incentive.',
        call: bidSubmission({
          vendor: handsetMaker,
          bid: pliBid,
          declaredLocalContentBps: 8_000,
          isPliManufacturer: true,
        }),
        expect: [
          ...accepted('the PLI case is a normal submission with a different answer'),
          {
            kind: 'class',
            equals: 'ClassTwo',
            why: 'the deeming rule caps a PLI beneficiary at Class-II regardless of the declared percentage; 80% does not buy Class-I here',
          },
          {
            kind: 'result',
            equals: 'YELLOW',
            why: 'Class-II proceeds with a caveat -- the vendor is not excluded, only capped',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P5 -- the sub Rs 200 crore domestic restriction
// ---------------------------------------------------------------------------------

function buildP5(ctx: Ctx): Scenario {
  const atLimit = ctx.bid({
    ministryId: 'MOSTEEL',
    label: 'p5-tmt-at-limit',
    itemCategoryName: 'TMT Reinforcement Bar (Fe 550D)',
    estimatedBidValuePaise: crorePaise(200),
    bidType: 'Standard Bid',
  });
  const overLimit = ctx.bid({
    ministryId: 'MOSTEEL',
    label: 'p5-tmt-over-limit',
    itemCategoryName: 'TMT Reinforcement Bar (Fe 550D)',
    estimatedBidValuePaise: crorePaise(200) + 1n,
    bidType: 'Standard Bid',
  });

  // The Ministry of Steel applies Para 3A, so every ranked bid has to be Class-I; this
  // scenario is about the tender's value, so all three bidders are Class-I.
  const millA = ctx.vendor('mill-a', { sector: 'steel' });
  const millB = ctx.vendor('mill-b', { sector: 'steel' });
  const millC = ctx.vendor('mill-c', { sector: 'steel' });
  const bids = [
    bidOf(millA, 'ClassOne', crorePaise(198)),
    bidOf(millB, 'ClassOne', crorePaise(205)),
    bidOf(millC, 'ClassOne', crorePaise(212)),
  ];

  return {
    id: 'P5',
    title: 'The sub Rs 200 crore domestic restriction',
    pathwayText: 'P5 enforces the sub Rupees 200 crore domestic restriction.',
    proves:
      'The limit is a strict "greater than". A tender at exactly Rs 200 crore proceeds on the ' +
      'domestic path; the same tender one paisa higher is refused unless a global tender ' +
      'enquiry has been approved. One paisa is the entire difference between the two calls.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'exactly Rs 200 crore, no GTE approval',
        narrate: `${atLimit.organisationName} evaluates ${atLimit.bidNumber} at exactly Rs 200 crore with no global tender enquiry approved.`,
        call: preferenceCalculation(atLimit, bids, crorePaise(200), false),
        expect: [
          ...accepted('at exactly the limit the domestic path is still open'),
          {
            kind: 'bool',
            names: ['qualifies'],
            value: true,
            why: 'Rs 200 crore is not above Rs 200 crore, so the restriction does not bite',
          },
        ],
      },
      {
        label: 'Rs 200 crore and one paisa, no GTE approval',
        narrate: 'The same procurement is re-scoped upward by a single paisa.',
        call: preferenceCalculation(overLimit, bids, crorePaise(200) + 1n, false),
        expect: [
          {
            kind: 'blocked',
            names: ['TenderValueExceedsDomesticLimit', 'domestic', '200'],
            why: 'one paisa above Rs 200 crore the domestic-preference path closes, and the response must name the limit that closed it',
          },
        ],
      },
      {
        label: 'the same over-limit tender with GTE approval',
        narrate: 'The buyer obtains approval for a global tender enquiry under GFR Rule 161(iv) and re-runs the evaluation.',
        call: preferenceCalculation(overLimit, bids, crorePaise(200) + 1n, true),
        expect: [
          ...accepted('an approved GTE reopens the evaluation on the same figures'),
          {
            kind: 'bool',
            names: ['qualifies'],
            value: true,
            why: 'the GTE approval, and nothing else about the tender, is what changed the answer',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P6 -- Non-local admitted only under an approved global tender enquiry
// ---------------------------------------------------------------------------------

function buildP6(ctx: Ctx): Scenario {
  const domesticBid = ctx.bid({
    ministryId: 'MOPNG',
    label: 'p6-linepipe-domestic',
    itemCategoryName: 'Seamless Line Pipe (API 5L X65)',
    estimatedBidValuePaise: crorePaise(46),
    bidType: 'Standard Bid',
  });
  const gteBid = ctx.bid({
    ministryId: 'MOPNG',
    label: 'p6-linepipe-gte',
    itemCategoryName: 'Seamless Line Pipe (API 5L X65)',
    estimatedBidValuePaise: crorePaise(46),
    bidType: 'Standard Bid',
  });
  const mixedBid = ctx.bid({
    ministryId: 'MOPNG',
    label: 'p6-linepipe-mixed',
    itemCategoryName: 'Seamless Line Pipe (API 5L X65)',
    estimatedBidValuePaise: crorePaise(5),
    bidType: 'Standard Bid',
  });

  const importerA = ctx.vendor('importer-a', { sector: 'steel' });
  const importerB = ctx.vendor('importer-b', { sector: 'steel' });
  const importer = ctx.vendor('importer-mixed', { sector: 'steel' });
  const domesticMill = ctx.vendor('domestic-mill', { sector: 'steel' });

  const allNonLocal = [
    bidOf(importerA, 'NonLocal', crorePaise(41)),
    bidOf(importerB, 'NonLocal', crorePaise(44)),
  ];
  const mixed = [
    bidOf(importer, 'NonLocal', crorePaise(4)),
    bidOf(domesticMill, 'ClassOne', crorePaise(4.6)),
  ];

  return {
    id: 'P6',
    title: 'Non-local suppliers admitted only under an approved global tender enquiry',
    pathwayText:
      'P6 admits Non-local suppliers only where a global tender enquiry has been approved ' +
      'under GFR Rule 161(iv).',
    proves:
      'On a domestic tender a Non-local bid is not merely ranked last -- it is removed from ' +
      'the ranking, with P6 recorded as the reason. A Non-local bid that is the cheapest by ' +
      'Rs 60 lakh loses to a Class-I bid, and an all-Non-local field leaves nothing to rank ' +
      'at all. The same field under an approved GTE is evaluated normally.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'an all-Non-local field on a domestic tender',
        narrate: `Both bidders on ${domesticBid.bidNumber} are Non-local and no global tender enquiry has been approved.`,
        call: preferenceCalculation(domesticBid, allNonLocal, crorePaise(46), false),
        expect: [
          {
            kind: 'blocked',
            names: ['NonLocalNotPermittedOnDomesticTender', 'global tender', 'non-local', 'nonlocal'],
            why: 'with every bid excluded there is nothing left to rank, and the refusal must name the gate rather than report an empty result',
          },
        ],
      },
      {
        label: 'the same field with GTE approval',
        narrate: 'A global tender enquiry is approved under GFR Rule 161(iv) and the identical bids are re-evaluated.',
        call: preferenceCalculation(gteBid, allNonLocal, crorePaise(46), true),
        expect: [
          ...accepted('an approved GTE admits the Non-local field'),
          {
            kind: 'bool',
            names: ['qualifies'],
            value: true,
            why: 'the GTE approval is a property of the tender, and it is the only thing that changed',
          },
        ],
      },
      {
        label: 'a cheaper Non-local bid against a dearer Class-I bid, no GTE',
        narrate: `On ${mixedBid.bidNumber} the Non-local bid is Rs 60 lakh cheaper than the only domestic bid.`,
        call: preferenceCalculation(mixedBid, mixed, crorePaise(5), false),
        expect: [
          ...accepted('the evaluation completes because one eligible bid remains'),
          {
            kind: 'preference-outcome',
            vendor: importer.accountId,
            vendorName: importer.legalName,
            qualifies: false,
            decisionPath: 'P6',
            why: 'the cheapest bid is excluded, and P6 is recorded against it so the file shows why the lowest price did not win',
          },
          {
            kind: 'preference-outcome',
            vendor: domesticMill.accountId,
            vendorName: domesticMill.legalName,
            qualifies: true,
            awardedPercentBps: 10_000,
            why: 'the Class-I bid takes the whole contract once the Non-local bid is out of the ranking',
          },
          {
            kind: 'paise',
            names: ['matchedPricePaise', 'matchedPrice'],
            paise: null,
            why: 'nobody was offered a price match: the surviving bid is already L1 among eligible bids',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P7 -- Para 3A
// ---------------------------------------------------------------------------------

function buildP7(ctx: Ctx): Scenario {
  const bid = ctx.bid({
    ministryId: 'DOP',
    label: 'p7-monitors',
    itemCategoryName: 'Multipara Patient Monitor',
    estimatedBidValuePaise: crorePaise(4),
    bidType: 'Standard Bid',
  });
  const noClassOneBid = ctx.bid({
    ministryId: 'DOP',
    label: 'p7-monitors-no-class-one',
    itemCategoryName: 'Multipara Patient Monitor',
    estimatedBidValuePaise: crorePaise(4),
    bidType: 'Standard Bid',
  });

  const assembler = ctx.vendor('assembler', { sector: 'pharma' });
  const manufacturerA = ctx.vendor('manufacturer-a', { sector: 'pharma' });
  const manufacturerB = ctx.vendor('manufacturer-b', { sector: 'pharma' });
  const importer = ctx.vendor('importer', { sector: 'pharma' });

  const field = [
    bidOf(assembler, 'ClassTwo', crorePaise(3.2)),
    bidOf(manufacturerA, 'ClassOne', crorePaise(3.8)),
    bidOf(manufacturerB, 'ClassOne', crorePaise(4.6)),
  ];
  const noClassOneField = [
    bidOf(assembler, 'ClassTwo', crorePaise(3.2)),
    bidOf(importer, 'NonLocal', crorePaise(3.0)),
  ];

  return {
    id: 'P7',
    title: 'Para 3A restricts sourcing to Class-I suppliers',
    pathwayText:
      'P7 enforces Para 3A, restricting sourcing to Class-I suppliers for items a nodal ' +
      'ministry has notified as having sufficient local capacity, in system integration, EPC, ' +
      'turnkey, and service tenders.',
    proves:
      'On a Para 3A item the cheapest bid does not win if it is not Class-I. The Class-II bid ' +
      'at Rs 3.2 crore is excluded outright, P7 is recorded against it, and the Class-I bid ' +
      'at Rs 3.8 crore takes the contract. Where no Class-I bid exists at all, the evaluation ' +
      'is refused rather than quietly awarded to the best of an ineligible field.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'cheapest bid is Class-II on a Para 3A item',
        narrate: `On ${bid.bidNumber}, ${assembler.legalName} is Rs 60 lakh cheaper than the lowest Class-I bidder -- but patient monitors are on the Department of Pharmaceuticals Para 3A list.`,
        call: preferenceCalculation(bid, field, crorePaise(4), false),
        expect: [
          ...accepted('the evaluation completes on the Class-I bids that remain'),
          {
            kind: 'preference-outcome',
            vendor: assembler.accountId,
            vendorName: assembler.legalName,
            qualifies: false,
            decisionPath: 'P7',
            why: 'Para 3A removes the cheapest bid from the ranking and records P7 as the reason',
          },
          {
            kind: 'preference-outcome',
            vendor: manufacturerA.accountId,
            vendorName: manufacturerA.legalName,
            qualifies: true,
            awardedPercentBps: 10_000,
            why: 'the lowest Class-I bid takes the full contract',
          },
          {
            kind: 'preference-outcome',
            vendor: manufacturerB.accountId,
            vendorName: manufacturerB.legalName,
            qualifies: false,
            why: 'Rs 4.6 crore is outside the 20% band above the Rs 3.8 crore L1, so the second Class-I bidder gets nothing',
          },
          {
            kind: 'paise',
            names: ['matchedPricePaise', 'matchedPrice'],
            paise: null,
            why: 'no price match arises: the eligible L1 is already Class-I',
          },
        ],
      },
      {
        label: 'a field with no Class-I bid at all',
        narrate: 'A re-tender attracts only a Class-II bid and a Non-local bid.',
        call: preferenceCalculation(noClassOneBid, noClassOneField, crorePaise(4), false),
        expect: [
          {
            kind: 'blocked',
            names: ['Para3ARequiresClassOne', 'para 3a', 'para3a', 'class-i', 'classone'],
            why: 'with no Class-I bidder a Para 3A tender cannot be awarded, and the refusal must name Para 3A rather than report an empty ranking',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P8 -- divisible award, price match at 50/50
// ---------------------------------------------------------------------------------

function buildP8(ctx: Ctx): Scenario {
  const bid = ctx.bid({
    ministryId: 'MNRE',
    label: 'p8-solar-modules',
    itemCategoryName: 'Solar Photovoltaic Module (540 Wp, Mono PERC)',
    estimatedBidValuePaise: crorePaise(9),
    bidType: 'Standard Bid',
  });

  const l1 = ctx.vendor('l1-class-two', { sector: 'energy' });
  const inBand = ctx.vendor('class-one-in-band', { sector: 'energy' });
  const outOfBand = ctx.vendor('class-one-out-of-band', { sector: 'energy' });

  const field = [
    bidOf(l1, 'ClassTwo', crorePaise(8)),
    bidOf(inBand, 'ClassOne', crorePaise(9.5)),
    bidOf(outOfBand, 'ClassOne', crorePaise(10.2)),
  ];

  return {
    id: 'P8',
    title: 'Divisible award: 50 percent to L1, 50 percent to the lowest Class-I on a price match',
    pathwayText:
      'P8 and P9 split on divisibility ... a Class-I bid priced at or below L1 multiplied by ' +
      '1.20 is offered the chance to match L1 price. Divisible award.',
    proves:
      'On a divisible contract where L1 is not Class-I, the contract splits 50/50 and the ' +
      'lowest Class-I bid inside the 20 percent band is offered L1 price. Rs 9.5 crore is ' +
      'inside the band above an Rs 8 crore L1 and is offered the match; Rs 10.2 crore is ' +
      'outside it and is offered nothing.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'divisible tender, L1 is Class-II',
        narrate: `${bid.organisationName} evaluates ${bid.bidNumber}: L1 is a Class-II bid at Rs 8 crore, and the lowest Class-I bid is Rs 9.5 crore.`,
        call: preferenceCalculation(bid, field, crorePaise(9), false),
        expect: [
          ...accepted('the evaluation completes and the award splits'),
          {
            kind: 'paise',
            names: ['matchedPricePaise', 'matchedPrice'],
            paise: crorePaise(8),
            why: 'the price offered to the Class-I bidder is L1 own price of Rs 8 crore, not its bid of Rs 9.5 crore',
          },
          {
            kind: 'preference-outcome',
            vendor: l1.accountId,
            vendorName: l1.legalName,
            qualifies: true,
            awardedPercentBps: 5_000,
            decisionPath: 'P8',
            why: 'L1 keeps half the divisible contract',
          },
          {
            kind: 'preference-outcome',
            vendor: inBand.accountId,
            vendorName: inBand.legalName,
            qualifies: true,
            awardedPercentBps: 5_000,
            matchedPricePaise: crorePaise(8),
            decisionPath: 'P8',
            why: 'the lowest Class-I bid within the 20% band takes the other half at L1 price',
          },
          {
            kind: 'preference-outcome',
            vendor: outOfBand.accountId,
            vendorName: outOfBand.legalName,
            qualifies: false,
            awardedPercentBps: 0,
            why: 'Rs 10.2 crore is above Rs 8 crore x 1.20, so the second Class-I bidder is outside the band and gets nothing',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P9 -- non-divisible award, full contract on a price match
// ---------------------------------------------------------------------------------

function buildP9(ctx: Ctx): Scenario {
  const bid = ctx.bid({
    ministryId: 'MOHUA',
    label: 'p9-cbtc',
    itemCategoryName: 'Metro Rail Signalling System (CBTC)',
    estimatedBidValuePaise: crorePaise(13),
    bidType: 'Standard Bid',
  });

  const l1 = ctx.vendor('l1-class-two', { sector: 'works' });
  const inBand = ctx.vendor('class-one-in-band', { sector: 'works' });
  const outOfBand = ctx.vendor('class-one-out-of-band', { sector: 'works' });

  const field = [
    bidOf(l1, 'ClassTwo', crorePaise(12)),
    bidOf(inBand, 'ClassOne', crorePaise(14)),
    bidOf(outOfBand, 'ClassOne', crorePaise(15)),
  ];

  return {
    id: 'P9',
    title: 'Non-divisible award: the whole contract to the lowest Class-I on a price match',
    pathwayText:
      'As P8, for Divisibility::NonDivisible: the lowest Class-I within the band is offered a ' +
      'price match for the full contract.',
    proves:
      'The identical price shape that splits 50/50 on a divisible contract awards 100 percent ' +
      'on a non-divisible one, and L1 -- who would keep half under P8 -- keeps nothing. A ' +
      'signalling system cannot be half-delivered, and the engine reads that from the ' +
      'ministry rule rather than from the bid.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'non-divisible tender, L1 is Class-II',
        narrate: `${bid.organisationName} evaluates ${bid.bidNumber}: a CBTC signalling system cannot be split between two suppliers.`,
        call: preferenceCalculation(bid, field, crorePaise(13), false),
        expect: [
          ...accepted('the evaluation completes and the award transfers whole'),
          {
            kind: 'paise',
            names: ['matchedPricePaise', 'matchedPrice'],
            paise: crorePaise(12),
            why: 'the Class-I bidder takes the contract at L1 price of Rs 12 crore',
          },
          {
            kind: 'preference-outcome',
            vendor: inBand.accountId,
            vendorName: inBand.legalName,
            qualifies: true,
            awardedPercentBps: 10_000,
            matchedPricePaise: crorePaise(12),
            decisionPath: 'P9',
            why: 'on a non-divisible contract the matching Class-I bidder takes all of it, not half',
          },
          {
            kind: 'preference-outcome',
            vendor: l1.accountId,
            vendorName: l1.legalName,
            qualifies: false,
            awardedPercentBps: 0,
            decisionPath: 'P9',
            why: 'this is the difference between P8 and P9: the Class-II L1 keeps 50 percent under P8 and nothing under P9',
          },
          {
            kind: 'preference-outcome',
            vendor: outOfBand.accountId,
            vendorName: outOfBand.legalName,
            qualifies: false,
            why: 'Rs 15 crore is above Rs 12 crore x 1.20 and so outside the band',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P10 -- the MSE overlay
// ---------------------------------------------------------------------------------

function buildP10(ctx: Ctx): Scenario {
  const bid = ctx.bid({
    ministryId: 'MOT',
    label: 'p10-bedsheets',
    itemCategoryName: 'Cotton Bed Sheet (Hospital Grade)',
    estimatedBidValuePaise: crorePaise(2.1),
    bidType: 'Standard Bid',
  });

  const largeMill = ctx.vendor('large-mill', { sector: 'textile', isMse: false });
  const mseInBand = ctx.vendor('mse-in-band', { sector: 'textile', isMse: true });
  const mseOutOfBand = ctx.vendor('mse-out-of-band', { sector: 'textile', isMse: true });

  const field = [
    bidOf(largeMill, 'ClassOne', crorePaise(2)),
    bidOf(mseInBand, 'ClassOne', crorePaise(2.25), true),
    bidOf(mseOutOfBand, 'ClassOne', crorePaise(2.4), true),
  ];

  return {
    id: 'P10',
    title: 'The MSE overlay where both preferences are concurrently active',
    pathwayText:
      'P10 overlays the MSE preference where both preferences are concurrently active ... the ' +
      'Department of Expenditure memorandum of 18.05.2023 governs how the two apply together.',
    proves:
      'The MSE band is 15 percent and is not the same number as the Class-I preference margin ' +
      'of 20 percent. The MSE bid at Rs 2.40 crore sits exactly on the 20 percent Class-I band ' +
      'edge and still gets nothing, because the MSE overlay is measured against 15 percent. ' +
      'The MSE at Rs 2.25 crore is inside it and takes 25 percent at L1 price.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'Class-I non-MSE L1 with two MSE bids behind it',
        narrate: `${bid.organisationName} evaluates ${bid.bidNumber}: L1 is a Class-I mill that is not an MSE, and two Udyam-registered MSEs have bid behind it.`,
        call: preferenceCalculation(bid, field, crorePaise(2.1), false),
        expect: [
          ...accepted('the evaluation completes and the MSE overlay applies'),
          {
            kind: 'paise',
            names: ['matchedPricePaise', 'matchedPrice'],
            paise: crorePaise(2),
            why: 'the MSE inside the band is offered L1 price of Rs 2 crore',
          },
          {
            kind: 'preference-outcome',
            vendor: largeMill.accountId,
            vendorName: largeMill.legalName,
            qualifies: true,
            awardedPercentBps: 7_500,
            decisionPath: 'P10',
            why: '75 percent to L1 under the 18.05.2023 memorandum',
          },
          {
            kind: 'preference-outcome',
            vendor: mseInBand.accountId,
            vendorName: mseInBand.legalName,
            qualifies: true,
            awardedPercentBps: 2_500,
            matchedPricePaise: crorePaise(2),
            decisionPath: 'P10',
            why: '25 percent offered to an MSE within the 15 percent band, on a price match',
          },
          {
            kind: 'preference-outcome',
            vendor: mseOutOfBand.accountId,
            vendorName: mseOutOfBand.legalName,
            qualifies: false,
            awardedPercentBps: 0,
            why: 'Rs 2.40 crore is exactly Rs 2 crore x 1.20 -- inside the Class-I preference margin but outside the 15 percent MSE band. This is the check that proves the two bands are separate numbers.',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P11 -- the certification obligation
// ---------------------------------------------------------------------------------

function buildP11(ctx: Ctx): Scenario {
  const belowBid = ctx.bid({
    ministryId: 'MOR',
    label: 'p11-bogies-below',
    itemCategoryName: 'Cast Steel Bogie for BOXNHL Wagon',
    estimatedBidValuePaise: rupeesPaise(99_999_999),
    bidType: 'Standard Bid',
  });
  const atBid = ctx.bid({
    ministryId: 'MOR',
    label: 'p11-bogies-at',
    itemCategoryName: 'Cast Steel Bogie for BOXNHL Wagon',
    estimatedBidValuePaise: crorePaise(10),
    bidType: 'Standard Bid',
  });

  const foundry = ctx.vendor('foundry', { sector: 'rail', isMse: false });
  const auditor = ctx.auditorAccountId();
  const belowThresholdCertificate = ctx.certificateId();
  const atThresholdCertificate = ctx.certificateId();

  // Rs 9,99,99,999 -- one rupee below the Rs 10 crore certification threshold.
  const belowThreshold = rupeesPaise(99_999_999);
  const atThreshold = crorePaise(10);

  return {
    id: 'P11',
    title: 'The certification obligation at execution',
    pathwayText:
      'P11 carries the certification obligation, which the 19.07.2024 amendment places at ' +
      'execution rather than at bidding.',
    proves:
      'Self-certification is accepted below the threshold and refused at it. Rs 9,99,99,999 ' +
      'self-certifies; one rupee more requires an auditor and is refused without one; supplying ' +
      'the auditor issues the certificate. The comparison is at-or-above, not strictly above, ' +
      'so exactly Rs 10 crore already needs the auditor.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'self-certification at Rs 9,99,99,999 (one rupee below the threshold)',
        narrate: `${foundry.legalName} self-certifies a contract one rupee below the Rs 10 crore threshold.`,
        call: caCertification({
          certificateId: belowThresholdCertificate,
          vendor: foundry,
          bid: belowBid,
          valuePaise: belowThreshold,
          auditor: null,
        }),
        expect: [
          ...accepted('below the threshold self-certification is accepted at all values'),
          {
            kind: 'bool',
            names: ['requiresAuditor'],
            value: false,
            why: 'one rupee below Rs 10 crore no auditor is required',
          },
          {
            kind: 'field-present',
            names: ['certificateId'],
            why: 'a certificate was actually issued and bound to this vendor and tender',
          },
          { kind: 'result', equals: 'GREEN', why: 'a complete certification proceeds' },
        ],
      },
      {
        label: 'self-certification at exactly Rs 10 crore, which must be refused',
        narrate: 'The same foundry attempts to self-certify a contract at exactly Rs 10 crore.',
        call: caCertification({
          certificateId: atThresholdCertificate,
          vendor: foundry,
          bid: atBid,
          valuePaise: atThreshold,
          auditor: null,
        }),
        expect: [
          {
            kind: 'blocked',
            names: ['AuditorRequired', 'auditor'],
            alsoAccept: ['YELLOW'],
            why: 'at exactly the threshold an auditor becomes mandatory -- the comparison is >= and not > -- and the response must say so rather than issue the certificate',
          },
        ],
      },
      {
        label: 'the same certification with an auditor',
        narrate: 'A cost accountant holding the CVC or audit reviewer role signs the certificate and it is resubmitted.',
        call: caCertification({
          certificateId: atThresholdCertificate,
          vendor: foundry,
          bid: atBid,
          valuePaise: atThreshold,
          auditor,
        }),
        expect: [
          ...accepted('with the auditor supplied the certification completes'),
          {
            kind: 'bool',
            names: ['requiresAuditor'],
            value: true,
            why: 'the response still reports that an auditor was required, which is what the Auditor Accountability Ledger is built on',
          },
          {
            kind: 'field-present',
            names: ['certificateId'],
            why: 'the certificate is issued and bound to the auditor who signed it',
          },
          { kind: 'result', equals: 'GREEN', why: 'an auditor-signed certification proceeds' },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// P12 -- false declaration, cross-tender consistency, and cross-ministry debarment
// ---------------------------------------------------------------------------------

function buildP12(ctx: Ctx): Scenario {
  // Both tenders are for the same catalogue item, so the consistency engine sees the
  // same `(vendor, product)` key across two different tenders -- which is exactly the
  // shape of the PoC document Section 4.7 example.
  const firstTender = ctx.bid({
    ministryId: 'MOCA',
    label: 'p12-xray-first',
    itemCategoryName: 'X-Ray Baggage Inspection System (Dual View)',
    estimatedBidValuePaise: crorePaise(6),
    bidType: 'Standard Bid',
  });
  const secondTender = ctx.bid({
    ministryId: 'MOCA',
    label: 'p12-xray-second',
    itemCategoryName: 'X-Ray Baggage Inspection System (Dual View)',
    estimatedBidValuePaise: crorePaise(4.4),
    bidType: 'Standard Bid',
  });
  const unrelatedRailTender = ctx.bid({
    ministryId: 'MOR',
    label: 'p12-unrelated-rail',
    itemCategoryName: 'Electric Point Machine (IRS S-24)',
    estimatedBidValuePaise: crorePaise(3.2),
    bidType: 'Standard Bid',
  });
  const unrelatedTextileTender = ctx.bid({
    ministryId: 'MOT',
    label: 'p12-unrelated-textile',
    itemCategoryName: 'Woollen Blanket (Relief Grade)',
    estimatedBidValuePaise: crorePaise(1.4),
    bidType: 'Standard Bid',
  });

  // The two vendors the PoC submission document names by hand.
  const sabarmati = ctx.vendor('sabarmati-systems', {
    legalName: 'Sabarmati Systems Private Limited',
    sector: 'electronics',
    isMse: false,
  });
  const chambal = ctx.vendor('chambal-devices', {
    legalName: 'Chambal Devices Private Limited',
    sector: 'electronics',
    isMse: false,
  });

  const reason =
    `False declaration of local content: 86% declared on ${firstTender.bidNumber} against ` +
    `30% on ${secondTender.bidNumber} for the same product (${firstTender.productId}). ` +
    `Debarment under GFR Rule 151(iii).`;

  return {
    id: 'P12',
    title: 'False declaration, cross-tender consistency, and cross-ministry debarment',
    pathwayText:
      'P12 handles the consequence of a false declaration: a downgrade in class triggers a ' +
      'penalty of up to 10 percent of contract value ... and debarment of up to two years ' +
      'follows under GFR Rule 151(iii). Cross-tender consistency is explicitly not a ' +
      'thirteenth pathway: it runs across all twelve, because a declaration is checked ' +
      'against the vendor history regardless of which route it takes.',
    proves:
      'The 86 percent versus 30 percent case from the PoC document is detected across two ' +
      'different tenders for the same product. The debarment that follows is enforced by ' +
      'every ministry, not only the one that recorded it -- Chambal Devices, debarred by the ' +
      'Ministry of Defence, is blocked on a textiles tender. Both debarments are then lifted, ' +
      'so the scenario leaves the chain exactly as it found it and can be re-run.',
    vendors: ctx.vendors,
    bids: ctx.bids,
    steps: [
      {
        label: 'Sabarmati Systems declares 86% on the first tender',
        narrate: `Sabarmati Systems declares 86 percent local content for the X-ray baggage system on ${firstTender.bidNumber}.`,
        call: bidSubmission({ vendor: sabarmati, bid: firstTender, declaredLocalContentBps: 8_600 }),
        expect: [
          ...accepted('the first declaration has nothing to contradict'),
          { kind: 'class', equals: 'ClassOne', why: '86% is comfortably Class-I' },
          { kind: 'result', equals: 'GREEN', why: 'nothing is wrong with this bid on its own' },
        ],
      },
      {
        label: 'the same vendor declares 30% for the same product on a second tender',
        narrate: 'Weeks later the same company declares 30 percent for the same catalogue item to a different airport office.',
        call: bidSubmission({ vendor: sabarmati, bid: secondTender, declaredLocalContentBps: 3_000 }),
        expect: [
          ...accepted('the second submission is accepted; the contradiction is reported, not thrown away'),
          { kind: 'class', equals: 'ClassTwo', why: '30% falls to Class-II under the 50% threshold' },
          {
            kind: 'mentions',
            texts: ['inconsisten'],
            why:
              'a 5600 basis point gap for the same (vendor, product) pair is far outside the 1000 bps tolerance and must surface as an inconsistency. ' +
              'This is the one behaviour asserted here that the route table in the build contract does not spell out: the bid-submission route must also call ' +
              'pramaanConsistency.declare when the request carries a `product`, because the PoC document says the consistency check "runs across all twelve".',
          },
        ],
      },
      {
        label: 'the nodal ministry debars Sabarmati Systems',
        narrate: 'The Ministry of Civil Aviation records a debarment against Sabarmati Systems for the false declaration.',
        call: debarmentCall({
          vendor: sabarmati,
          ministryId: 'MOCA',
          action: 'debar',
          effectiveFrom: 0,
          effectiveTo: null,
          reason,
        }),
        expect: [
          ...accepted('the debarment is recorded on the shared ledger'),
          {
            kind: 'field-present',
            names: ['status'],
            why: 'the route reports the resulting state of the debarment record',
          },
          {
            kind: 'mentions',
            texts: ['debar'],
            why: 'the response names the action taken, so the audit trail reads without decoding',
          },
        ],
      },
      {
        label: 'the debarred vendor is blocked on an unrelated railway tender',
        narrate: `${unrelatedRailTender.organisationName} evaluates a point-machine bid from the same company.`,
        call: bidEvaluation(sabarmati, unrelatedRailTender, 6_800),
        expect: [
          {
            kind: 'blocked',
            names: ['VendorDebarred', 'debarred', 'debarment'],
            why: 'one shared ledger, enforced before bidding: a debarment recorded by Civil Aviation blocks the vendor at Railways, and the RED response names the debarment',
          },
        ],
      },
      {
        label: 'Chambal Devices is debarred by the Ministry of Defence',
        narrate: 'Separately, the Ministry of Defence debars Chambal Devices.',
        call: debarmentCall({
          vendor: chambal,
          ministryId: 'MOD-DEFENCE',
          action: 'debar',
          effectiveFrom: 0,
          effectiveTo: null,
          reason:
            'Debarment recorded by the Department of Defence for a false local-content declaration, GFR Rule 151(iii).',
        }),
        expect: [
          ...accepted('the second debarment is recorded on the same shared ledger'),
          { kind: 'mentions', texts: ['debar'], why: 'the response names the action taken' },
        ],
      },
      {
        label: 'Chambal Devices is blocked on a textiles tender',
        narrate: `${unrelatedTextileTender.organisationName} evaluates a blanket bid from Chambal Devices.`,
        call: bidEvaluation(chambal, unrelatedTextileTender, 7_400),
        expect: [
          {
            kind: 'blocked',
            names: ['VendorDebarred', 'debarred', 'debarment'],
            why: 'the PoC document own worked example: a bid for Chambal Devices, debarred by the Ministry of Defence, is returned RED and blocked on an unrelated tender',
          },
        ],
      },
      {
        label: 'lift the Sabarmati Systems debarment',
        narrate: 'The debarment against Sabarmati Systems is lifted on appeal.',
        call: debarmentCall({ vendor: sabarmati, ministryId: 'MOCA', action: 'lift' }),
        expect: [
          ...accepted('lifting must succeed, otherwise the demo cannot be run twice'),
          { kind: 'mentions', texts: ['lift'], why: 'the response names the lift so the ledger reads as a history' },
        ],
      },
      {
        label: 'lift the Chambal Devices debarment',
        narrate: 'The debarment against Chambal Devices is likewise lifted.',
        call: debarmentCall({ vendor: chambal, ministryId: 'MOD-DEFENCE', action: 'lift' }),
        expect: [
          ...accepted('the second lift restores the chain to its pre-scenario state'),
          { kind: 'mentions', texts: ['lift'], why: 'the response names the lift' },
        ],
      },
      {
        label: 'the same railway bid now evaluates normally',
        narrate: 'The point-machine bid from Sabarmati Systems is re-evaluated after the lift.',
        call: bidEvaluation(sabarmati, unrelatedRailTender, 6_800),
        expect: [
          { kind: 'http-ok', why: 'with no active debarment the evaluation proceeds' },
          {
            kind: 'result-one-of',
            anyOf: ['GREEN', 'YELLOW'],
            why: 'the block was the debarment and nothing else; lifting it restores the vendor, and this step is also what leaves the ledger clean for the next run',
          },
        ],
      },
    ],
  };
}

// ---------------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------------

const BUILDERS: Record<PathwayId, (ctx: Ctx) => Scenario> = {
  P1: buildP1,
  P2: buildP2,
  P3: buildP3,
  P4: buildP4,
  P5: buildP5,
  P6: buildP6,
  P7: buildP7,
  P8: buildP8,
  P9: buildP9,
  P10: buildP10,
  P11: buildP11,
  P12: buildP12,
};

export function buildScenario(id: PathwayId, opts: ScenarioOptions): Scenario {
  const builder = BUILDERS[id];
  const ctx = new Ctx(id, opts);
  return builder(ctx);
}

export const DEFAULT_SCENARIO_OPTIONS: Omit<ScenarioOptions, 'seed' | 'runId'> = {
  asOf: DEFAULT_AS_OF,
};
