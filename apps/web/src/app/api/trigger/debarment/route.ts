/**
 * Trigger point 5 — debarment (PathwayId P12).
 *
 * A nodal ministry records or lifts a debarment on the shared national ledger. Because
 * enforcement is cross-ministry (PoC document Table 8: "one shared ledger, enforced
 * before bidding"), a single record here blocks the vendor everywhere, which is what
 * trigger point 2 reads before it will evaluate a bid. Calls
 * `pallet_pramaan_debarment::debar` or `::lift_debarment`.
 *
 * Request  { vendor, ministry, action: 'debar' | 'lift', effectiveFrom?, effectiveTo?, reason? }
 * Response { result, status, reason, txRef, blockNumber, latencyMs }
 */

import { getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  BadRequestError,
  DEBARMENT_REASON_MAX_BYTES,
  debarTx,
  encodeBoundedText,
  handleTrigger,
  liftDebarmentTx,
  optionalBlockNumber,
  requireEnum,
  requireEvent,
  requireId,
  requireIdText,
  resolveAccount,
  type TriState,
} from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

const ACTIONS = ['debar', 'lift'] as const;

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const vendor = await resolveAccount(body, 'vendor');
    const ministry = requireId(body, 'ministry');
    const ministryText = requireIdText(body, 'ministry');
    const action = requireEnum(body, 'action', ACTIONS);

    const api = await getApi();
    // Debarment is a nodal-ministry-administrator act (and DPIIT's, at national level),
    // never a procuring entity's or a vendor's. The pallet's origin check is a
    // placeholder `ensure_signed` — its own `NotAuthorised` error is documented as
    // reserved for a future role registry — so the persona chosen here is what actually
    // distinguishes the role in this build.
    const signer = await getSigner('ministryAdmin');

    if (action === 'lift') {
      const result = await submitAndFinalize(liftDebarmentTx(api, { vendor, ministry }), signer);
      requireEvent(result, 'pramaanDebarment', 'DebarmentLifted');

      return {
        // The tri-state on this route describes the vendor's resulting enforcement
        // state, which is what a judge reads off the screen: lifted means the vendor may
        // bid again.
        result: 'GREEN' satisfies TriState,
        status: 'DebarmentLifted',
        reason: `Cleared: the debarment recorded by ${ministryText} has been lifted, so this vendor may bid again across every ministry.`,
        txRef: result.txRef,
        blockNumber: result.blockNumber,
        latencyMs: result.latencyMs,
      };
    }

    // `effective_from` defaults to the current block, which is what "debar this vendor
    // now" means. A caller wanting a future or backdated order supplies it explicitly.
    const header = await api.rpc.chain.getHeader();
    const effectiveFrom = optionalBlockNumber(body, 'effectiveFrom') ?? header.number.toNumber();
    const effectiveTo = optionalBlockNumber(body, 'effectiveTo') ?? null;
    if (effectiveTo !== null && effectiveTo <= effectiveFrom) {
      throw new BadRequestError(
        `"effectiveTo" (block ${effectiveTo}) must be later than "effectiveFrom" (block ${effectiveFrom}); ` +
          `a debarment that ends before it starts would never be enforced.`,
      );
    }

    // FLAGGED — the route table omits `reason`, but `debar` requires the field. It is
    // accepted as optional text and defaulted rather than made mandatory, so the
    // documented request shape keeps working; the default states plainly that no reason
    // was given rather than inventing one.
    const reasonInput =
      typeof body.reason === 'string' && body.reason.trim().length > 0
        ? body.reason.trim()
        : 'Debarment order recorded by the nodal ministry; no reason text was supplied.';
    const reasonBytes = encodeBoundedText(reasonInput, DEBARMENT_REASON_MAX_BYTES, 'reason');

    const result = await submitAndFinalize(
      debarTx(api, { vendor, ministry, effectiveFrom, effectiveTo, reason: reasonBytes }),
      signer,
    );
    requireEvent(result, 'pramaanDebarment', 'Debarred');

    const window =
      effectiveTo === null
        ? 'with no end date recorded'
        : `until block ${effectiveTo}`;

    return {
      // RED here is the vendor's resulting state: from this block on, the shared ledger
      // blocks their bids everywhere.
      result: 'RED' satisfies TriState,
      status: 'Debarred',
      reason:
        `Blocked: this vendor is debarred by ${ministryText} from block ${effectiveFrom} ${window}, ` +
        `and the shared national ledger will now block their bids under every ministry, not just ` +
        `this one. The recorded reason is: ${reasonInput}`,
      effectiveFrom,
      effectiveTo,
      txRef: result.txRef,
      blockNumber: result.blockNumber,
      latencyMs: result.latencyMs,
    };
  });
}
