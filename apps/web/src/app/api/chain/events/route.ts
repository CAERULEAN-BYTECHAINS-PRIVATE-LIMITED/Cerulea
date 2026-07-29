/**
 * Decoded runtime events for the explorer's Events tab, filterable by pallet.
 *
 * The six `pramaan*` pallets are the ones a judge cares about, so the tab defaults to
 * them — but every section present in the indexed window is offered, with its count, so
 * the consensus machinery underneath is one click away rather than hidden.
 */

import { eventSections, indexWindow, recentEvents, syncIndex } from '../_lib/reader';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export const PRAMAAN_SECTIONS = [
  'pramaanRuleRegistry',
  'pramaanClassification',
  'pramaanPreference',
  'pramaanCertification',
  'pramaanDebarment',
  'pramaanConsistency',
];

export async function GET(request: Request): Promise<Response> {
  try {
    const url = new URL(request.url);
    const limit = Math.min(300, Math.max(1, Number(url.searchParams.get('limit') ?? 60) || 60));
    const pallets = url.searchParams.getAll('pallet').filter(Boolean);
    const search = url.searchParams.get('q') ?? undefined;

    await syncIndex();

    return Response.json({
      events: recentEvents({ limit, sections: pallets.length > 0 ? pallets : undefined, search }),
      sections: eventSections(),
      pramaanSections: PRAMAAN_SECTIONS,
      window: indexWindow(),
    });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
