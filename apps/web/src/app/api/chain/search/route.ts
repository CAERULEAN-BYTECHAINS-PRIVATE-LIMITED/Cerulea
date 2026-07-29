/**
 * Transaction and block lookup for the explorer's Search tab.
 *
 * `ComplianceResult` links to `/explorer?tx=0x…` immediately after a trigger point
 * returns, so this must resolve a hash that is seconds old. Two tiers:
 *
 *   1. the in-memory index, which already holds the recent window — an O(1) lookup
 *   2. a bounded backwards scan for a hash that has aged out of it
 *
 * A miss returns 404 with an explanation rather than an empty success, so the UI can say
 * what it looked at instead of implying the transaction does not exist.
 */

import { getApi } from '@/lib/chain';
import {
  blockAt,
  eventsInBlock,
  findTransaction,
  indexWindow,
  searchTransactionDeep,
  syncIndex,
} from '../_lib/reader';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url);
    const tx = url.searchParams.get('tx')?.trim().toLowerCase();
    const blockParam = url.searchParams.get('block')?.trim();

    if (!tx && !blockParam) {
      return Response.json(
        { error: 'Provide a transaction reference as ?tx=0x… or a block as ?block=1234.' },
        { status: 400 },
      );
    }

    await syncIndex();
    const api = await getApi();
    const finalizedHeader = await api.rpc.chain.getHeader(await api.rpc.chain.getFinalizedHead());
    const finalizedBlock = finalizedHeader.number.toNumber();

    if (tx) {
      if (!/^0x[0-9a-f]{64}$/.test(tx)) {
        return Response.json(
          {
            error:
              'A transaction reference is a 32-byte hash: 0x followed by 64 hexadecimal characters.',
          },
          { status: 400 },
        );
      }

      const hit = findTransaction(tx) ?? (await searchTransactionDeep(tx));
      if (!hit) {
        const window = indexWindow();
        return Response.json(
          {
            found: false,
            window,
            error:
              `No extrinsic with that hash was found between blocks #${Math.max(0, window.from - 600)} ` +
              `and #${window.to}. It may be older than the explorer's search window, or it may ` +
              `not have been included in a block yet.`,
          },
          { status: 404 },
        );
      }

      return Response.json({
        found: true,
        kind: 'transaction',
        txRef: tx,
        extrinsicIndex: hit.extrinsicIndex,
        extrinsic: hit.block.extrinsics[hit.extrinsicIndex] ?? null,
        block: { ...hit.block, finalized: hit.block.number <= finalizedBlock },
        events: hit.events,
        finalizedBlock,
      });
    }

    const blockNumber = Number(blockParam);
    if (!Number.isInteger(blockNumber) || blockNumber < 0) {
      return Response.json({ error: 'A block number must be a non-negative integer.' }, { status: 400 });
    }

    const block = blockAt(blockNumber);
    if (!block) {
      return Response.json(
        {
          found: false,
          window: indexWindow(),
          error: `Block #${blockNumber} is outside the explorer's indexed window.`,
        },
        { status: 404 },
      );
    }

    return Response.json({
      found: true,
      kind: 'block',
      block: { ...block, finalized: block.number <= finalizedBlock },
      events: eventsInBlock(blockNumber),
      finalizedBlock,
    });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
