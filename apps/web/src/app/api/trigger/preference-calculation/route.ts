/**
 * Trigger point 3 — purchase-preference calculation.
 *
 * The procuring entity ranks a tender's bids. The chain applies the eligibility gates
 * (P5's Rs 200 crore domestic limit, P6's global-tender-enquiry rule, P7's Para 3A
 * restriction), finds L1, and applies the P8 / P9 / P10 award-split rules. Calls
 * `pallet_pramaan_preference::calculate_preference`.
 *
 * Request  { tender, ministry, tenderValuePaise, isTenderGte?, bids: [...] }
 * Response { result, qualifies, matchedPricePaise, matchedPrice, reason, txRef,
 *            blockNumber, latencyMs }
 */

import { getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  BadRequestError,
  BID_CLASSES,
  MAX_BIDS,
  assertMinistryOnboarded,
  calculatePreferenceTx,
  coerceUnsignedInteger,
  handleTrigger,
  optionalBoolean,
  readPreferenceOutcomes,
  requireEnum,
  requireEvent,
  requireId,
  requireIdText,
  requirePaise,
  resolveAccountValue,
  rupees,
  type ChainBidItem,
} from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const tender = requireId(body, 'tender');
    // FLAGGED — spec vs. source. The build contract's route table gives this route a
    // two-field body (tender, bids), but `calculate_preference` takes five arguments.
    // `ministry` is needed to read the rule that supplies the preference margin and the
    // divisibility fork; `tender_value` is what P5's Rs 200 crore gate is tested
    // against; `is_tender_gte` is what P6 reads. All three are required here — none can
    // be inferred from a bid list — with `isTenderGte` defaulting to false, the
    // restrictive reading, since GTE approval is an affirmative act under GFR Rule
    // 161(iv) and must never be assumed.
    const ministry = requireId(body, 'ministry');
    const ministryText = requireIdText(body, 'ministry');
    const tenderValuePaise = requirePaise(body, 'tenderValuePaise', 'tenderValue');
    const isTenderGte = optionalBoolean(body, 'isTenderGte', false);

    const bidsRaw = body.bids;
    if (!Array.isArray(bidsRaw) || bidsRaw.length === 0) {
      throw new BadRequestError('"bids" is required and must be a non-empty array of bid objects.');
    }
    if (bidsRaw.length > MAX_BIDS) {
      throw new BadRequestError(
        `"bids" may hold at most ${MAX_BIDS} entries (the runtime's MaxBids bound); got ${bidsRaw.length}.`,
      );
    }

    const bids: ChainBidItem[] = [];
    for (const [index, entry] of bidsRaw.entries()) {
      if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) {
        throw new BadRequestError(`"bids[${index}]" must be an object.`);
      }
      const bid = entry as Record<string, unknown>;
      const vendorValue = bid.vendor;
      if (typeof vendorValue !== 'string' || vendorValue.trim().length === 0) {
        throw new BadRequestError(`"bids[${index}].vendor" is required.`);
      }
      bids.push({
        vendor: await resolveAccountValue(vendorValue.trim(), `bids[${index}].vendor`),
        class: requireEnum(bid, 'class', BID_CLASSES),
        price: requirePaise(bid, 'pricePaise', 'price').toString(),
        isMse: optionalBoolean(bid, 'isMse', false),
        // FLAGGED — carried for display only. The pallet's own comment on `BidItem.is_gte`
        // states that the P6 gate reads the tender-level `is_tender_gte` argument, because
        // GTE approval is a property of the tender rather than of any one bid. It is
        // accepted and passed through unchanged so the frontend can show what was
        // submitted, but it does not influence the decision.
        isGte: optionalBoolean(bid, 'isGte', false),
      });
    }

    const api = await getApi();
    // The preference margin and the divisibility fork both come out of this ministry's
    // rule, so a ministry the registry does not hold cannot be ranked against one.
    await assertMinistryOnboarded(api, ministryText);
    // Ranking bids and awarding preference is the procuring entity's act, not the
    // vendor's and not DPIIT's.
    const signer = await getSigner('procuringEntity');

    const result = await submitAndFinalize(
      calculatePreferenceTx(api, { tender, ministry, bids, tenderValuePaise, isTenderGte }),
      signer,
    );

    // The pallet emits exactly one PreferenceCalculated event per successful call. An
    // eligibility rejection returns Err and emits nothing, so reaching this line already
    // means P5, P6 and P7 were all satisfied by at least one bid.
    const event = requireEvent(result, 'pramaanPreference', 'PreferenceCalculated');
    const qualifies = event.qualifies === true || event.qualifies === 'true';
    const matchedPricePaise = coerceUnsignedInteger(event.matchedPrice);

    // The tender-level event cannot distinguish P8 from P9: both offer a match at L1's
    // price and differ only in the award share. The per-vendor rows carry
    // `awardedPercentBps` and `decisionPath`, which is what makes each pathway provable
    // rather than merely asserted. Read at the finalized block's own hash.
    const outcomes = await readPreferenceOutcomes(
      api,
      result.blockHash,
      tender,
      bids.map((bid) => bid.vendor),
    );

    // Tri-state, per build contract section 3:
    //   GREEN  — the ranking stands as calculated; the lowest eligible bid wins outright
    //            and no purchase-preference price match had to be offered.
    //   YELLOW — proceeds with a caveat: a price match has been offered under P8/P9/P10
    //            and a human still has to see whether the supplier accepts it.
    // RED for this route arrives via ExtrinsicFailedError (P5/P6/P7 rejections), which
    // the shared handler maps and names.
    const triState = matchedPricePaise === null ? 'GREEN' : 'YELLOW';
    const reason =
      matchedPricePaise === null
        ? 'Compliant: the preference calculation completed and the lowest eligible bid stands as the award, with no price match required.'
        : `Proceeds with a caveat: a purchase-preference price match at ${rupees(matchedPricePaise)} has been offered under the Make in India order and must be accepted by the qualifying supplier before the award is made.`;

    return {
      result: triState,
      qualifies,
      matchedPricePaise: matchedPricePaise === null ? null : matchedPricePaise.toString(),
      // Alias kept for the build contract's route-table field name; section 1 of the
      // same document requires the `...Paise` suffix on any JSON carrying a currency
      // value, so `matchedPricePaise` is the canonical field and this mirrors it.
      matchedPrice: matchedPricePaise === null ? null : matchedPricePaise.toString(),
      // One row per bid submitted, including the bids P6/P7 excluded from ranking — the
      // pallet records those too, with the pathway that excluded them, so the file shows
      // why the lowest price did not win.
      outcomes,
      reason,
      txRef: result.txRef,
      blockNumber: result.blockNumber,
      latencyMs: result.latencyMs,
    };
  });
}
