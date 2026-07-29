/**
 * The guided walkthrough's six steps — one per trigger point, in the order the PoC
 * document introduces them (Technical Implementation Specification Part 8.2).
 *
 * Everything here is data. The captions are written to be read by a judge with no
 * presenter in the room: each says what the chain was actually asked, what it answered,
 * why that answer is the one the Order requires, and which claim in the submission the
 * step is evidence for. They are explanations, not labels.
 *
 * The scenario is one continuous procurement — a single GeM bid for desktop computers
 * under MeitY — so the six steps compose into a story rather than six disconnected demos.
 */

import { rupeesToPaise } from '@/lib/units';

export type TriggerId =
  | 'bid-submission'
  | 'bid-evaluation'
  | 'preference-calculation'
  | 'ca-certification'
  | 'debarment'
  | 'rule-update';

export interface Scenario {
  /** GeM bid number, generated fresh per walkthrough so a judge can run it twice. */
  tender: string;
  ministry: string;
  ministryName: string;
  otherMinistry: string;
  otherMinistryName: string;
  vendor: string;
  vendorName: string;
  rival: string;
  rivalName: string;
  mseBidder: string;
  mseBidderName: string;
  declaredBps: number;
  tenderValueRupees: number;
  l1Rupees: number;
  classOneRupees: number;
  classTwoRupees: number;
}

/**
 * GeM bid numbers are `GEM/YYYY/B/NNNNNNN` with a seven-digit serial (build contract
 * section 4). Ids are stored hyphenated on chain because the pallet's `IdBound` holds raw
 * bytes and a hyphenated form survives a URL without escaping.
 */
/**
 * The scenario as an external store.
 *
 * The bid number is random, so it cannot be generated during render: the server and the
 * client would produce two different numbers and hydration would mismatch. Reading it
 * through `useSyncExternalStore` with a server snapshot of `null` gives the component a
 * defined server render and a generated value on the client, without a state-setting
 * effect and without the cascading render one would cause.
 */
export const scenarioStore = (() => {
  let value: Scenario | null = null;
  const listeners = new Set<() => void>();
  return {
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    /** Stable between renders: generated once, then returned as the same object. */
    getSnapshot(): Scenario {
      value ??= newScenario();
      return value;
    },
    /** Nothing is generated on the server, so the first paint has no bid number to mismatch. */
    getServerSnapshot(): Scenario | null {
      return null;
    },
    /** Start a fresh walkthrough with a new bid number and new counterparty accounts. */
    reset() {
      value = newScenario();
      listeners.forEach((listener) => listener());
    },
  };
})();

export function newScenario(): Scenario {
  const serial = String(Math.floor(1_000_000 + Math.random() * 9_000_000));
  return {
    tender: `GEM-2026-B-${serial}`,
    ministry: 'MEITY',
    ministryName: 'Ministry of Electronics and Information Technology',
    otherMinistry: 'MOD-DEFENCE',
    otherMinistryName: 'Ministry of Defence',
    vendor: 'vendor',
    vendorName: 'Bharat Precision Instruments Pvt Ltd',
    // Derived per run so a second walkthrough debars a fresh account rather than piling
    // records onto one vendor until the pallet's MaxRecords bound rejects the step.
    rival: `//GlobalInstrumentsTrading${serial}`,
    rivalName: 'Global Instruments Trading',
    mseBidder: `//SaraswatiMicroSystems${serial}`,
    mseBidderName: 'Saraswati Micro Systems (MSE)',
    declaredBps: 6_200,
    tenderValueRupees: 42_00_00_000,
    l1Rupees: 38_00_00_000,
    classOneRupees: 43_50_00_000,
    classTwoRupees: 40_00_00_000,
  };
}

export interface StepFact {
  label: string;
  value: string;
}

export interface FollowUp {
  /** Button label, phrased as the thing the official would do next. */
  action: string;
  endpoint: string;
  trigger: TriggerId;
  /** Why this second call exists, shown above the button. */
  premise: string;
  caption: string;
  body: (scenario: Scenario) => Record<string, unknown>;
}

export interface WalkthroughStep {
  id: TriggerId;
  /** Trigger point number in the PoC document, 1–6. */
  point: number;
  title: string;
  /** Set-up shown before the step runs: what is about to be asked of the chain. */
  intent: string;
  /** The concrete inputs, so a judge can see nothing is hidden in the request. */
  facts: (scenario: Scenario) => StepFact[];
  endpoint: string;
  body: (scenario: Scenario) => Promise<Record<string, unknown>> | Record<string, unknown>;
  /** Shown after the verdict: what just happened, in plain institutional language. */
  caption: string;
  /** The submission's own claim this step is evidence for. */
  claim: string;
  followUp?: FollowUp;
}

const paise = (rupees: number) => rupeesToPaise(rupees).toString();

async function currentBlock(): Promise<number> {
  const response = await fetch('/api/chain/status', { cache: 'no-store' });
  const payload = (await response.json()) as { bestBlock?: number };
  return payload.bestBlock ?? 0;
}

export const WALKTHROUGH: WalkthroughStep[] = [
  {
    id: 'bid-submission',
    point: 1,
    title: 'A vendor submits a bid with a local content declaration',
    intent:
      'Bharat Precision Instruments is bidding on a MeitY tender for desktop computers, HSN 8471. It declares that 62% of the contract value is domestic. On the GeM portal today that declaration is a self-certified line in a PDF that nobody checks until an audit, possibly years later. Here it is submitted as a transaction and classified before the bid is accepted.',
    facts: (scenario) => [
      { label: 'GeM bid number', value: scenario.tender },
      { label: 'Buyer organisation', value: scenario.ministryName },
      { label: 'Bidder', value: scenario.vendorName },
      { label: 'Item category', value: 'Desktop computers (HSN 8471)' },
      { label: 'Declared local content', value: '62%' },
    ],
    endpoint: '/api/trigger/bid-submission',
    body: (scenario) => ({
      vendor: scenario.vendor,
      tender: scenario.tender,
      ministry: scenario.ministry,
      declaredLocalContentBps: scenario.declaredBps,
    }),
    caption:
      "The chain read MeitY's own rule set — not a national default — and judged the declared 62% against the Class-I threshold that ministry has itself notified for HSN 8471. The verdict states which side of that threshold the bid fell on and why, in the pallet's own words. What matters as much as the answer is when it arrived: the response was withheld until the block carrying it reached finality on the three-validator network, so the classification a judge is reading is not a pending transaction that could still be reorganised away. The vendor never saw a wallet, a key, or a gas fee — from their side this was a form on a procurement portal.",
    claim:
      'Trigger point 1 (Part 8.2), decision pathway P1, and the sub-second finality claim in Part 8.3.',
  },
  {
    id: 'bid-evaluation',
    point: 2,
    title: 'The procuring entity evaluates the bid',
    intent:
      "The buyer's evaluation cell now opens the bid. Before it looks at price or classification at all, it checks one thing: is this vendor barred from public procurement anywhere in the Union Government? Today that means writing to other ministries and hoping for a reply. Here it is a read against a single shared ledger.",
    facts: (scenario) => [
      { label: 'Evaluated by', value: 'Central Procurement Cell, Northern Railway' },
      { label: 'Bidder', value: scenario.vendorName },
      { label: 'GeM bid number', value: scenario.tender },
      { label: 'Debarment check', value: 'All ministries, single ledger' },
    ],
    endpoint: '/api/trigger/bid-evaluation',
    body: (scenario) => ({
      vendor: scenario.vendor,
      tender: scenario.tender,
      ministry: scenario.ministry,
      declaredLocalContentBps: scenario.declaredBps,
    }),
    caption:
      'Two gates ran in order. The first read the national debarment ledger for this vendor across every ministry, not just MeitY, and found nothing. Only because that gate passed did the second gate — the classification against the ministry rule — run at all. That ordering is the point: a debarment is an absolute bar, so no amount of local content can rescue a debarred bidder. Step 5 comes back to this vendor check and shows what happens when the ledger is not empty.',
    claim:
      'Trigger point 2 (Part 8.2), and the cross-ministry enforcement claim in Section 4.6 — one ledger, read by every buyer.',
  },
  {
    id: 'preference-calculation',
    point: 3,
    title: 'Purchase preference is applied across all bids',
    intent:
      'Three bids are in, and the cheapest is from a non-local supplier at ₹38 crore. Under the Public Procurement (Preference to Make in India) Order the lowest price does not automatically win. MeitY has notified this category under Para 3A, and no global tender enquiry has been approved for the tender — two facts that decide the outcome before price is looked at.',
    facts: (scenario) => [
      { label: 'Tender value', value: '₹42 crore' },
      { label: 'L1 bid (Non-local)', value: `₹38 crore — ${scenario.rivalName}` },
      { label: 'Class-I bid', value: `₹43.5 crore — ${scenario.vendorName}` },
      { label: 'Class-II bid (MSE)', value: `₹40 crore — ${scenario.mseBidderName}` },
      { label: 'Para 3A', value: 'Applicable to this category' },
      { label: 'Global tender enquiry', value: 'Not approved' },
    ],
    endpoint: '/api/trigger/preference-calculation',
    body: (scenario) => ({
      tender: scenario.tender,
      ministry: scenario.ministry,
      tenderValuePaise: paise(scenario.tenderValueRupees),
      isTenderGte: false,
      bids: [
        { vendor: scenario.rival, class: 'NonLocal', pricePaise: paise(scenario.l1Rupees) },
        { vendor: scenario.vendor, class: 'ClassOne', pricePaise: paise(scenario.classOneRupees) },
        {
          vendor: scenario.mseBidder,
          class: 'ClassTwo',
          pricePaise: paise(scenario.classTwoRupees),
          isMse: true,
        },
      ],
    }),
    caption:
      "Expand the on-chain record and read the decision path recorded against each bid, because that is where the reasoning is. Both the non-local bid and the Class-II bid come back on pathway P7 — Para 3A. Where a nodal ministry has notified that domestic capacity is sufficient for a category, sourcing is restricted to Class-I suppliers and the other bids are removed from the ranking entirely, before price is compared at all. That leaves one eligible bid, which takes the full award on P8, the divisible fork. This is why the cheapest bid did not win, and it is recorded per bid rather than inferred: an auditor opening this file in two years gets the pathway, not a conclusion. Had Para 3A not applied to this category, the same call would have run the P8/P9 price-match instead — the Class-I bid at ₹43.5 crore sits inside the ₹45.6 crore band that MeitY's 20% margin draws around L1, and would have been offered the award at the matched L1 price.",
    claim:
      'Trigger point 3 (Part 8.2), and decision pathways P7, P8 and P9 in Section 4.2 — the Para 3A restriction and the price-match forks it takes precedence over.',
  },
  {
    id: 'ca-certification',
    point: 4,
    title: 'The contract crosses the certification threshold',
    intent:
      "The award is worth ₹42 crore. The 19.07.2024 amendment moved the local-content certificate from bidding to execution and made a statutory auditor's signature mandatory above the certification threshold each ministry notifies. The vendor lodges its certificate — without one.",
    facts: (scenario) => [
      { label: 'Contract value', value: '₹42 crore' },
      { label: 'Certification threshold', value: 'As notified by the ministry, read from chain' },
      { label: 'Certifying party', value: 'Not supplied' },
      { label: 'Vendor', value: scenario.vendorName },
    ],
    endpoint: '/api/trigger/ca-certification',
    body: (scenario) => ({
      vendor: scenario.vendor,
      tender: scenario.tender,
      ministry: scenario.ministry,
      valuePaise: paise(scenario.tenderValueRupees),
    }),
    caption:
      "This is the amber state, and it is the one worth dwelling on. The chain did not block the contract and it did not wave it through: it stopped and said a human with a statutory qualification has to sign before execution can proceed. Nothing was written to the certificate registry, so there is no transaction reference on this verdict — the chain declined to record a certificate that does not exist. A system that only ever returns pass or fail cannot express \"this needs a chartered accountant\", and most compliance workflows in government are exactly that shape.",
    claim:
      'Trigger point 4 (Part 8.2), decision pathway P11, and the amber tri-state in Part 8.2 — proceeds with a caveat, or needs a human.',
    followUp: {
      action: 'Lodge the chartered accountant’s certificate',
      endpoint: '/api/trigger/ca-certification',
      trigger: 'ca-certification',
      premise:
        'S. Raghavan & Associates, Cost Accountants, certifies the local content for this contract and lodges it against the same tender.',
      caption:
        "With an auditor supplied the certificate is written to chain and the verdict turns green. The auditor's own account is bound to the certificate in the Auditor Accountability Ledger, so every certificate that auditor has ever issued is queryable as a set. That is the mechanism behind the accountability claim: an auditor who signs off on a false declaration is not an anonymous signature on a PDF, they are an account with a history.",
      body: (scenario) => ({
        vendor: scenario.vendor,
        tender: scenario.tender,
        ministry: scenario.ministry,
        valuePaise: paise(scenario.tenderValueRupees),
        auditor: 'auditor',
      }),
    },
  },
  {
    id: 'debarment',
    point: 5,
    title: 'A different ministry debars the losing bidder',
    intent:
      'Unrelated to this tender, the Ministry of Defence concludes an inquiry into Global Instruments Trading — the non-local supplier who bid L1 at step 3 — and debars it for two years for a false local-content declaration on another contract. Under GFR Rule 151(iii) that debarment binds the whole Union Government, not only the ministry that issued it.',
    facts: (scenario) => [
      { label: 'Debarring ministry', value: scenario.otherMinistryName },
      { label: 'Vendor', value: scenario.rivalName },
      { label: 'Ground', value: 'False local content declaration' },
      { label: 'Authority', value: 'GFR Rule 151(iii)' },
    ],
    endpoint: '/api/trigger/debarment',
    body: (scenario) => ({
      vendor: scenario.rival,
      ministry: scenario.otherMinistry,
      action: 'debar',
      reason: 'False local content declaration on an unrelated defence contract',
    }),
    caption:
      'The Ministry of Defence wrote one record to a ledger every other ministry reads. Nothing was emailed, no circular was issued, and no other ministry had to do anything at all. The verdict shown here is red because it describes the vendor\'s resulting state: from this block onward, their bids are barred everywhere. The follow-up below is the part worth showing a judge — it re-runs step 2\'s evaluation for this vendor under MeitY, a ministry with no involvement in the defence inquiry whatsoever.',
    claim:
      'Trigger point 5 (Part 8.2), decision pathway P12, and the single-ledger cross-ministry enforcement claim in Section 4.6.',
    followUp: {
      action: 'Re-run the MeitY bid evaluation for this vendor',
      endpoint: '/api/trigger/bid-evaluation',
      trigger: 'bid-evaluation',
      premise:
        'MeitY is a different ministry with no part in the defence inquiry. Its evaluation cell now opens a bid from the same vendor.',
      caption:
        "Blocked, by a decision MeitY never made and was never told about. The evaluation never reached the classification stage: the debarment gate is absolute, so the vendor's local content was not even computed. The verdict names the ministry that issued the debarment, which is what an aggrieved bidder is entitled to know and what an auditor needs in order to trace the block. Today, this is the failure mode the CVC reports on most often — a vendor debarred by one ministry continuing to win contracts under another because the two registers never met.",
      body: (scenario) => ({
        vendor: scenario.rival,
        tender: `${scenario.tender}-R`,
        ministry: scenario.ministry,
        declaredLocalContentBps: 3_000,
      }),
    },
  },
  {
    id: 'rule-update',
    point: 6,
    title: 'MeitY amends its rule set',
    intent:
      'MeitY sets the Class-I local content threshold for HSN 8471 to 60% and its certification threshold to ₹10 crore, effective from a named future block. In the system this PoC replaces, that is a notification, a circular, a change request, and eventually a software release. Here it is a transaction signed by the nodal ministry administrator.',
    facts: (scenario) => [
      { label: 'Ministry', value: scenario.ministryName },
      { label: 'Item', value: 'HSN 8471 — computers' },
      { label: 'Class-I threshold', value: 'Set to 60%' },
      { label: 'Class-II threshold', value: 'Set to 20%' },
      { label: 'Certification threshold', value: 'Set to ₹10 crore' },
      { label: 'Effective from', value: 'Ten blocks from now' },
    ],
    endpoint: '/api/trigger/rule-update',
    body: async (scenario) => ({
      ministry: scenario.ministry,
      rule: {
        hsnThresholds: [{ hsnCode: '8471', classOneBps: 6_000, classTwoBps: 2_000 }],
        para3aApplicable: true,
        pliLinked: true,
        calculationMethod: 'ComponentLevel',
        preferenceMarginBps: 2_000,
        certificationThresholdPaise: paise(10_00_00_000),
        exemptionFloorPaise: paise(5_00_000),
        divisibility: 'Divisible',
        effectiveFrom: (await currentBlock()) + 10,
      },
    }),
    caption:
      "The rule set is now at a new version and takes effect from the block named in the record, not from the moment the transaction landed — which is how a policy with a notified commencement date actually works. No code was deployed and no service was restarted. The part that matters for audit is what did not change: the classification returned at step 1 is still judged against the version that was in force at its block. A rule change going forward is not a rewriting of decisions already taken, and a system that cannot make that distinction cannot be trusted with procurement law.",
    claim:
      'Trigger point 6 (Part 8.2), and the Smart Evolution upgrade path in Part 6.3 — policy as versioned on-chain state rather than as a software release.',
  },
];
