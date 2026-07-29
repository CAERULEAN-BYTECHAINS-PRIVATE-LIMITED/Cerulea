/**
 * Trigger point 1 — bid submission.
 *
 * A vendor submits a bid carrying a declared local-content percentage. The chain
 * classifies it against the ministry's own rule set and answers GREEN / YELLOW / RED
 * before the bid is accepted.
 *
 * Two calls, in this order:
 *   1. `pallet_pramaan_classification::classify` (PathwayId P1, or P3/P4 where the
 *      ministry's rule selects them), or `::classify_component_level` (P2) when the
 *      body carries a `components` array instead of a single percentage.
 *   2. `pallet_pramaan_consistency::declare`, when the body carries a `product` — the
 *      cross-tender consistency check, which the PoC document is explicit is not a
 *      thirteenth pathway but "runs across all twelve, because a declaration is checked
 *      against the vendor's history regardless of which route it takes".
 *
 * Request  { vendor, tender, ministry, declaredLocalContentBps | components,
 *            product?, isPliManufacturer? }
 * Response { result, class, reason, consistencyFlagged, priorDeclaredBps, priorTender,
 *            txRef, blockNumber, latencyMs }
 */

import { ExtrinsicFailedError, getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  classToTriState,
  classifyComponentLevelTx,
  classifyTx,
  declareTx,
  describeClass,
  findAllEvents,
  handleTrigger,
  optionalBoolean,
  requireBps,
  requireComponents,
  requireEventField,
  requireId,
  requireIdText,
  resolveAccount,
  weightedAverageBps,
  type ClassResult,
} from '@/lib/pramaan';
import { formatBps } from '@/lib/units';

// A compliance verdict is never a cached artefact: it is the state of the chain at the
// moment it was asked. Force every request to run.
export const dynamic = 'force-dynamic';
// polkadot-js needs the Node runtime (WebSocket transport plus WASM crypto).
export const runtime = 'nodejs';

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const vendor = await resolveAccount(body, 'vendor');
    const tender = requireId(body, 'tender');
    const ministry = requireId(body, 'ministry');
    // FLAGGED — spec vs. source. The build contract's route table lists four request
    // fields, but `classify` takes a fifth argument, `is_pli_manufacturer`, which drives
    // PathwayId P4's deeming rule. It is accepted here as an optional field defaulting
    // to false, which is the only safe default: P4 only ever *upgrades* a vendor to
    // Class-II, so defaulting it on would hand out a classification nobody claimed.
    const isPliManufacturer = optionalBoolean(body, 'isPliManufacturer', false);

    // The presence of `components` selects the component-level path (P2). The two forms
    // are mutually exclusive: a component-level declaration has no single declared
    // figure, it has an aggregate the chain computes from the parts.
    const hasComponents = body.components !== undefined && body.components !== null;
    const components = hasComponents ? requireComponents(body.components) : null;
    const declaredLocalContentBps = components
      ? weightedAverageBps(components)
      : requireBps(body, 'declaredLocalContentBps');

    const api = await getApi();
    // Classification is a procuring-entity action: the buyer, not the vendor, records
    // where a bid lands against the rule. Signing as the vendor would let a bidder write
    // its own classification.
    const signer = await getSigner('procuringEntity');

    const result = await submitAndFinalize(
      components
        ? classifyComponentLevelTx(api, { vendor, tender, ministry, components })
        : classifyTx(api, { vendor, tender, ministry, declaredLocalContentBps, isPliManufacturer }),
      signer,
    );

    // Read the outcome out of the event emitted in the finalized block, so the class
    // returned provably came from that block rather than from a later storage read.
    const classResult = requireEventField(
      result,
      'pramaanClassification',
      'Classified',
      'class',
    ) as ClassResult;

    let reason = describeClass(classResult, declaredLocalContentBps);
    if (components) {
      reason +=
        ` That figure is the weighted average of ${components.length} declared components, ` +
        `not an average of their percentages.`;
    }

    // --- Cross-tender consistency ------------------------------------------------------
    // `product` is optional. Without it this route behaves exactly as the build
    // contract's route table describes; with it, the declaration is also recorded against
    // the vendor's history for that product and checked against every prior one.
    //
    // Submitted as a second extrinsic rather than batched: this runtime has no
    // `pallet_utility` (the runtime's pallet indices 0-16 in cerulea-runtime/src/lib.rs
    // contain no Utility pallet), so `utility.batchAll` is not dispatchable here.
    let consistencyFlagged = false;
    let priorDeclaredBps: number | null = null;
    let priorTender: string | null = null;
    let inconsistencyCount = 0;
    let consistencyTxRef: string | null = null;
    let consistencyBlockNumber: number | null = null;
    let consistencyError: string | null = null;

    if (typeof body.product === 'string' && body.product.trim().length > 0) {
      const product = requireId(body, 'product');
      const productText = requireIdText(body, 'product');
      try {
        const declaration = await submitAndFinalize(
          declareTx(api, { vendor, product, tender, localContentBps: declaredLocalContentBps }),
          signer,
        );
        consistencyTxRef = declaration.txRef;
        consistencyBlockNumber = declaration.blockNumber;

        // One event per contradicting prior declaration, so several earlier declarations
        // that each individually contradict this one produce several flags.
        const flags = findAllEvents(declaration, 'pramaanConsistency', 'InconsistencyFlagged');
        inconsistencyCount = flags.length;

        if (flags.length > 0) {
          consistencyFlagged = true;
          const first = flags[0];
          const prior = Number(String(first.priorValue ?? '').replace(/,/g, ''));
          priorDeclaredBps = Number.isFinite(prior) ? prior : null;
          priorTender = typeof first.priorTender === 'string' ? first.priorTender : null;

          reason +=
            ` Flagged as inconsistent: this vendor previously declared ` +
            `${priorDeclaredBps === null ? 'a materially different figure' : formatBps(priorDeclaredBps)} ` +
            `for the same product (${productText}) on another tender, a gap far outside the ` +
            `tolerance the rules allow, so this declaration is contradicted by the vendor's own ` +
            `history${flags.length > 1 ? ` across ${flags.length} earlier declarations` : ''}.`;
        }
      } catch (error) {
        // A failure to record the declaration must not erase a classification that has
        // already reached finality. The classification is this trigger point's answer;
        // the consistency check is evidence recorded alongside it. Surfacing the failure
        // in its own field is truthful; turning it into a RED would misreport what the
        // chain actually decided about the bid.
        if (error instanceof ExtrinsicFailedError) {
          consistencyError = error.palletError;
        } else {
          throw error;
        }
      }
    }

    return {
      // The tri-state stays the classification's own verdict. A consistency flag is
      // reported prominently but does not override it: the flag concerns the vendor's
      // declaration history across tenders, while this trigger point answers whether
      // *this* bid is compliant. The enforcement that follows a false declaration is a
      // separate act — PathwayId P12, the debarment trigger point.
      result: classToTriState(classResult),
      class: classResult,
      reason,
      declaredLocalContentBps,
      consistencyFlagged,
      priorDeclaredBps,
      priorTender,
      inconsistencyCount,
      consistencyTxRef,
      consistencyBlockNumber,
      consistencyError,
      txRef: result.txRef,
      blockNumber: result.blockNumber,
      latencyMs: result.latencyMs,
    };
  });
}
