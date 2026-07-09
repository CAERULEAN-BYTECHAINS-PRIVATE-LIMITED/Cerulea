import { NextResponse } from 'next/server';
import { getSession } from '@/lib/auth';

// Stub network data replace with real on-chain queries after deployment infrastructure is wired.
export async function GET() {
  const session = await getSession();
  if (!session?.user?.id) return NextResponse.json({ error: 'Unauthorized' }, { status: 401 });

  const networks = [
    {
      id: 'net-001',
      name: 'Cerulea Testnet',
      type: 'L1',
      status: 'live',
      blockHeight: 1842371,
      tps: 47.2,
      lastBlock: new Date(Date.now() - 3000).toISOString(),
      region: 'apac-south',
      nodeCount: 3,
      consensusHealth: 99.7,
    },
  ];

  return NextResponse.json({ networks });
}
