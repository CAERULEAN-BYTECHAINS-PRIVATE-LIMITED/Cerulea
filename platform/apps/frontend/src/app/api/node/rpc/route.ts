import { NextRequest, NextResponse } from 'next/server';
import { getSession } from '@/lib/auth';
import { ALLOWED_READ_RPC_METHODS, callNodeRpc } from '@/lib/cerulea-node';

export const dynamic = 'force-dynamic';

export async function POST(req: NextRequest) {
  const session = await getSession();
  if (!session?.user?.id) {
    return NextResponse.json({ ok: false, error: 'Unauthorized' }, { status: 401 });
  }

  try {
    const body = (await req.json()) as { method?: string; params?: unknown[] };
    const method = typeof body?.method === 'string' ? body.method : '';
    if (!ALLOWED_READ_RPC_METHODS.has(method)) {
      return NextResponse.json(
        { ok: false, error: `RPC method "${method}" is not in the allowlisted read surface.` },
        { status: 403 }
      );
    }
    const params = Array.isArray(body?.params) ? body.params : [];
    const reply = await callNodeRpc(method, params);
    if (!reply.ok) {
      return NextResponse.json({ ok: false, nodeOnline: false, error: reply.error }, { status: 502 });
    }
    return NextResponse.json({ ok: true, nodeOnline: true, result: reply.result });
  } catch (err: unknown) {
    const message = err instanceof Error ? err.message : String(err);
    return NextResponse.json({ ok: false, error: message }, { status: 500 });
  }
}
