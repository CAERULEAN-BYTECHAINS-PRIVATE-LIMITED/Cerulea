/**
 * Provenance / tamper-evidence read.
 *
 * Not a trigger point — it writes nothing. It takes a decision already on the chain
 * (the classification recorded at trigger point 1) and re-derives it independently from
 * the chain's own state *as of the block that decision was written into*, then reports
 * whether that historical reading matches what current state holds.
 *
 * The point it evidences: a recorded verdict is not the application remembering an
 * answer. It is `pramaanClassification.classifications[(vendor, tender)]` read at
 * `api.at(blockHash)` — the state of a finalized, immutable block. Altering that record
 * would change the block's state root and therefore its hash, which would break finality
 * for every block built on top of it. So "it cannot be quietly changed" is not a claim
 * about this software; it is a property of the chain, and this endpoint demonstrates it.
 *
 * Request  { vendor, tender, blockNumber }   — blockNumber is trigger point 1's block
 * Response { tender, recordedBlock, blockHash, classAtBlock, classAtHead, result, match,
 *            finalizedHead, confirmations }
 */

import { getApi } from '@/lib/chain';
import { BadRequestError, handleTrigger, requireId, requireIdText, resolveAccount } from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

/** ClassResult → the tri-state a judge reads. Mirrors lib/pramaan classToTriState. */
function classToResult(raw: string): 'GREEN' | 'YELLOW' | 'RED' | null {
  if (raw.includes('ClassOne')) return 'GREEN';
  if (raw.includes('NonLocal')) return 'RED';
  if (raw.includes('ClassTwo') || raw.includes('ManualReviewRequired')) return 'YELLOW';
  return null;
}

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const vendor = await resolveAccount(body, 'vendor');
    const tender = requireId(body, 'tender');
    const tenderText = requireIdText(body, 'tender');

    const blockNumber = body.blockNumber;
    if (typeof blockNumber !== 'number' || !Number.isInteger(blockNumber) || blockNumber < 0) {
      throw new BadRequestError(
        '"blockNumber" is required and must be the block that recorded the decision (trigger point 1 returns it).',
      );
    }

    const api = await getApi();

    // The hash of the exact block trigger point 1's classification was finalized into.
    const blockHash = (await api.rpc.chain.getBlockHash(blockNumber)).toHex();

    // The record as it stood in that block — read from historical state, not from any
    // cache or from the response trigger point 1 happened to return.
    const at = await api.at(blockHash);
    const rawAtBlock = (await at.query.pramaanClassification.classifications([vendor, tender])).toString();

    // The same record as current state holds it now, after every later trigger point —
    // including the rule amendment at step 6 — has run.
    const rawAtHead = (await api.query.pramaanClassification.classifications([vendor, tender])).toString();

    const classAtBlock = classToResult(rawAtBlock);
    const classAtHead = classToResult(rawAtHead);

    const finalizedHash = await api.rpc.chain.getFinalizedHead();
    const finalizedHeader = await api.rpc.chain.getHeader(finalizedHash);
    const finalizedHead = finalizedHeader.number.toNumber();

    const match = classAtBlock !== null && classAtBlock === classAtHead;

    return {
      // No verdict is being taken here; `result` is the classification being re-derived,
      // so the walkthrough can colour the panel with the same tri-state step 1 returned.
      result: classAtBlock ?? undefined,
      tender: tenderText,
      recordedBlock: blockNumber,
      blockHash,
      classAtBlock,
      classAtHead,
      match,
      finalizedHead,
      // How many finalized blocks are stacked on top of the decision's block. A settled
      // decision is buried under thousands; rewriting it would mean re-finalizing all of
      // them, which the validator set will not do.
      confirmations: Math.max(0, finalizedHead - blockNumber),
    };
  });
}
