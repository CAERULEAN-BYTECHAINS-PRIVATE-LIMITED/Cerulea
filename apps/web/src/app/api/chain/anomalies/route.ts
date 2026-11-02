/**
 * GET /api/chain/anomalies
 *
 * The anomaly surface behind the CVC console. Read-only: it signs nothing and submits
 * nothing, exactly like the rest of `/api/chain/*`.
 *
 * Every figure in the response is a storage read or a decoded event — see the long note
 * at the top of `../_lib/anomalies.ts` for the split between the one signal the chain's
 * own consensus rules raise and the four review heuristics computed here over finalized
 * records. A signal with no hits returns an empty `findings` array and a `count` of zero
 * rather than anything invented.
 *
 * The event index is advanced first so a transaction hash can be attached to any
 * contradiction still inside the indexed window; contradictions older than the window are
 * reported all the same, from storage, with `txRef: null` and their block number.
 */

import { buildAnomalyReport } from '../_lib/anomalies';
import { syncIndex } from '../_lib/reader';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(): Promise<Response> {
  try {
    await syncIndex();
    return Response.json(await buildAnomalyReport());
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
