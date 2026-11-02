/**
 * The Cerulea block explorer (spec Part 9.5).
 *
 * Purpose-built and deliberately small: four views that answer the four questions a judge
 * asks about a compliance decision — is the chain live, who is validating it, what did the
 * pallets emit, and where exactly did *my* transaction land. It is not a general-purpose
 * explorer and does not try to be one.
 *
 * A Server Component so `?tx=` is read on the server and handed to the client tree as a
 * prop; the panels themselves are client components because they poll.
 */

import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { ExplorerClient } from './_components/ExplorerClient';

export const metadata: Metadata = {
  title: 'Explorer — CBC-PRAMAAN',
  description:
    'Blocks, validators, runtime events and transaction lookup on the Cerulea DCF network.',
};

export const dynamic = 'force-dynamic';

export default async function ExplorerPage({
  searchParams,
}: {
  searchParams: Promise<Record<string, string | string[] | undefined>>;
}) {
  const params = await searchParams;
  const rawTx = Array.isArray(params.tx) ? params.tx[0] : params.tx;
  const rawBlock = Array.isArray(params.block) ? params.block[0] : params.block;

  const initialTx = rawTx && /^0x[0-9a-fA-F]{64}$/.test(rawTx) ? rawTx.toLowerCase() : undefined;
  const parsedBlock = rawBlock !== undefined ? Number(rawBlock) : Number.NaN;
  const initialBlock = Number.isInteger(parsedBlock) && parsedBlock >= 0 ? parsedBlock : undefined;

  return (
    <AppShell
      width="wide"
      breadcrumb="Network"
      title="Block explorer"
    >
      <ExplorerClient initialTx={initialTx} initialBlock={initialBlock} />
    </AppShell>
  );
}
