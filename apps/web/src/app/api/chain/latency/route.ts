/**
 * Measured trigger-point latency.
 *
 * `GET`  — the samples recorded so far, per trigger point, for the dashboard's chart.
 * `POST` — records one measurement. The only writer is the walkthrough at `/demo`, which
 *          posts the `latencyMs` a trigger-point route returned. That figure is produced
 *          by `submitAndFinalize` in `src/lib/chain.ts` as submission-to-confirmed-finality
 *          wall-clock time; nothing here computes, smooths, or estimates it.
 *
 * This writes to process memory, not to the chain. It is a session recorder for numbers
 * the chain already produced, which is why it lives under the read-only `api/chain`
 * namespace rather than alongside the six trigger points.
 */

import { isTriggerPoint, latencySummary, recordLatency } from '../_lib/latency';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function GET(): Promise<Response> {
  return Response.json(latencySummary());
}

export async function POST(request: Request): Promise<Response> {
  let body: unknown;
  try {
    body = await request.json();
  } catch {
    return Response.json({ error: 'Expected a JSON body.' }, { status: 400 });
  }

  const payload = body as Record<string, unknown>;
  const trigger = payload.trigger;
  const latencyMs = Number(payload.latencyMs);

  if (!isTriggerPoint(trigger)) {
    return Response.json(
      { error: '"trigger" must be one of the six trigger-point ids.' },
      { status: 400 },
    );
  }
  if (!Number.isFinite(latencyMs) || latencyMs < 0) {
    return Response.json({ error: '"latencyMs" must be a non-negative number.' }, { status: 400 });
  }

  recordLatency({
    trigger,
    latencyMs: Math.round(latencyMs),
    blockNumber: Number.isFinite(Number(payload.blockNumber)) ? Number(payload.blockNumber) : null,
    txRef: typeof payload.txRef === 'string' ? payload.txRef : null,
    recordedAt: Date.now(),
  });

  return Response.json(latencySummary(), { status: 201 });
}
