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
    const finalizedBlock = finalizedHeader.number.toNumber();

    const blocks = recentBlocks(limit).map((block) => ({
      number: block.number,
      hash: block.hash,
      timestamp: block.timestamp,
      author: block.author,
      extrinsicCount: block.extrinsicCount,
      eventCount: block.eventCount,
      finalized: block.number <= finalizedBlock,
      extrinsics: block.extrinsics,
    }));

    return Response.json({ finalizedBlock, blocks, window: indexWindow() });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
