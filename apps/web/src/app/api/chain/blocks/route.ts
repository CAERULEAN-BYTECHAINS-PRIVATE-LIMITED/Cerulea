/**
 * Recent blocks for the explorer's Blocks tab.
 *
 * Finality is reported per block rather than assumed: a block is marked finalized only
 * when its number is at or below the finalized head the node reports at read time. That
 * distinction is the whole point of this PoC, so the explorer must not blur it.
 */

import { getApi } from '@/lib/chain';
import { indexWindow, recentBlocks, syncIndex } from '../_lib/reader';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url);
    const limit = Math.min(100, Math.max(1, Number(url.searchParams.get('limit') ?? 20) || 20));

    await syncIndex();
    const api = await getApi();
    const finalizedHeader = await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead());
    const indexedHead = recentBlocks(1)[0]?.number ?? 0;

    /**
     * Clamp the finalized height to what the index has actually seen.
     *
     * The finalized head is read fresh from RPC AFTER `syncIndex()` resolves, so it is
     * strictly newer than the index it just awaited. Worse, a concurrent request can
     * join an in-flight catch-up that targeted an EARLIER head (see `reader.ts`'s
     * `if (store.catchUp) return store.catchUp`), and then pair that stale index with
     * its own current finalized read. The explorer polls four panels at once, so this
     * is the normal case, not a rare race: it rendered "finalized #9,516" above
     * "best #9,508" roughly one poll in four.
     *
     * Finality can never exceed the head, so reporting a number the index cannot show a
     * block for is simply wrong. Clamping keeps the two panels mutually consistent, and
     * the next poll advances both together.
     */
    const finalizedBlock = Math.min(finalizedHeader.number.toNumber(), indexedHead);

    const blocks = recentBlocks(limit).map((block) => ({
      number: block.number,
      hash: block.hash,
      timestamp: block.timestamp,
      author: block.author,
      extrinsicCount: block.extrinsicCount,
      eventCount: block.eventCount,
      eventsAvailable: block.eventsAvailable,
      finalized: block.number <= finalizedBlock,
      extrinsics: block.extrinsics,
    }));

    return Response.json({ finalizedBlock, bestBlock: indexedHead, blocks, window: indexWindow() });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
