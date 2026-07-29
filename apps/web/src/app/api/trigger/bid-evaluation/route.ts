/**
 * Trigger point 2 — bid evaluation.
 *
 * The procuring entity evaluates a bid it has received. Debarment is checked first,
 * from the shared national ledger, and an actively debarred vendor is returned RED
 * without any extrinsic being submitted at all — that is the point of PoC document
 * Table 8's "one shared ledger, enforced before bidding" and of the Chambal Devices
 * example (Figure 29), where a debarment recorded by one ministry blocks a bid under an
 * entirely unrelated one.
 *
 * Only if the vendor is clear does the route submit a classification.
 *
 * Request  { vendor, tender, ministry, declaredLocalContentBps, isPliManufacturer? }
 * Response { result, class, reason, txRef, blockNumber, latencyMs }
 */

import { getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  classToTriState,
  classifyTx,
  describeClass,
  describeDebarment,
  handleTrigger,
  optionalBoolean,
  readActiveDebarment,
  requireBps,
  requireEventField,
  requireId,
  resolveAccount,
  type ClassResult,
} from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const startedAt = Date.now();

    const vendor = await resolveAccount(body, 'vendor');
    const tender = requireId(body, 'tender');
    const ministry = requireId(body, 'ministry');
    // FLAGGED — spec vs. source. The build contract's route table gives this route a
    // three-field body (vendor, tender, ministry), but the classify extrinsic it then
    // calls requires a declared local-content percentage, and no pallet stores one that
    // could be recovered instead: pallet-pramaan-classification keeps only the resulting
    // ClassResult, not the input percentage. Rather than fabricate a figure, the field
    // is required and a missing one is a 400 that says exactly what is missing.
    const declaredLocalContentBps = requireBps(body, 'declaredLocalContentBps');
    const isPliManufacturer = optionalBoolean(body, 'isPliManufacturer', false);

    const api = await getApi();

    // --- Debarment first, as a storage read -----------------------------------------
    // No extrinsic is submitted for a debarred vendor. Submitting one would be rejected
    // by the pallet's own `VendorDebarred` guard anyway, so this only avoids a wasted
    // block; but it also makes the demo honest about *where* the block came from — a
    // ledger lookup, not a transaction that failed.
    const debarment = await readActiveDebarment(api, vendor);
    if (debarment) {
      return {
        result: 'RED',
        class: null,
        reason: describeDebarment(debarment),
        debarredBy: debarment.ministry,
        // No transaction was submitted, so there is no transaction reference and no
        // block to point at. The decision came from the shared ledger's current state.
        txRef: null,
        blockNumber: null,
        latencyMs: Date.now() - startedAt,
      };
    }

    // --- Vendor is clear: classify ----------------------------------------------------
    const signer = await getSigner('procuringEntity');
    const result = await submitAndFinalize(
      classifyTx(api, { vendor, tender, ministry, declaredLocalContentBps, isPliManufacturer }),
      signer,
    );

    const classResult = requireEventField(
      result,
      'pramaanClassification',
      'Classified',
      'class',
    ) as ClassResult;

    return {
      result: classToTriState(classResult),
      class: classResult,
      reason: describeClass(classResult, declaredLocalContentBps),
      debarredBy: null,
      txRef: result.txRef,
      blockNumber: result.blockNumber,
      latencyMs: result.latencyMs,
    };
  });
}
