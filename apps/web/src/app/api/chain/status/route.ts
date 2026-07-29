/**
 * Connection state for the header indicator (`src/components/ChainStatus.tsx`).
 *
 * Deliberately the cheapest endpoint in the application: three RPC calls, no block
 * indexing, no storage iteration. It is polled every five seconds from every page, so it
 * must never be the reason a demo feels slow.
 */

import { CHAIN_ENDPOINT, getApi } from '@/lib/chain';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(): Promise<Response> {
  try {
    const api = await getApi();
    const [chainName, finalizedHash, header] = await Promise.all([
      api.rpc.system.chain(),
      api.rpc.chain.getFinalizedHead(),
      api.rpc.chain.getHeader(),
    ]);
    const finalizedHeader = await api.rpc.chain.getHeader(finalizedHash);

    return Response.json({
      connected: api.isConnected,
      chainName: chainName.toString(),
      endpoint: CHAIN_ENDPOINT,
      finalizedBlock: finalizedHeader.number.toNumber(),
      bestBlock: header.number.toNumber(),
      runtime: `${api.runtimeVersion.specName.toString()} v${api.runtimeVersion.specVersion.toString()}`,
    });
  } catch (error) {
    // A calm, well-shaped negative answer. `ChainStatus` degrades to "status unavailable"
    // on this rather than showing a stack trace in the header of a live demo.
    return Response.json(
      {
        connected: false,
        endpoint: CHAIN_ENDPOINT,
        error: error instanceof Error ? error.message : 'Unknown error',
      },
      { status: 503 },
    );
  }
}
