/**
 * Aggregates for the Compliance Analytics Dashboard.
 *
 * Every number below is a storage read or a decoded event. Where the chain holds nothing
 * — no debarments, no classifications yet — this returns an empty collection and a
 * `sources` note saying which storage item was read and came back empty, so the dashboard
 * can render an honest empty state naming the thing that would fill it.
 *
 * Reads:
 *   pramaanRuleRegistry.rules        — ministries onboarded, and their rule versions
 *   pramaanConsistency.declarations  — declarations recorded, and the contradictions in them
 *   pramaanDebarment.debarments      — debarments, split active vs. lapsed at the current head
 *   pramaanCertification.certificates — certificates issued
 *   pramaanClassification (events)   — the GREEN / YELLOW / RED verdicts seen this session
 */

import { getApi } from '@/lib/chain';
import { indexWindow, recentEvents, syncIndex } from '../_lib/reader';
import { ingestVerdicts, verdictLedger } from '../_lib/verdicts';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

/** Ministry ids are short ASCII byte strings on chain (`MEITY`, `DPIIT`, `MOD`). */
function decodeId(value: unknown): string {
  const raw = String(value ?? '');
  if (!raw.startsWith('0x')) return raw;
  const bytes = raw.slice(2).match(/.{2}/g) ?? [];
  const text = bytes.map((byte) => String.fromCharCode(parseInt(byte, 16))).join('');
  return /^[\x20-\x7e]+$/.test(text) ? text : raw;
}

function toNumber(value: unknown): number {
  return Number(String(value ?? '0').replace(/,/g, ''));
}

export async function GET(): Promise<Response> {
  try {
    await syncIndex();
    ingestVerdicts(recentEvents({ limit: 6_000, sections: ['pramaanClassification'] }));

    const api = await getApi();
    const [head, finalizedHash] = await Promise.all([
      api.rpc.chain.getHeader(),
      api.rpc.chain.getFinalizedHead(),
    ]);
    const finalizedBlock = (await api.rpc.chain.getHeader(finalizedHash)).number.toNumber();
    const currentBlock = head.number.toNumber();

    const [rules, declarations, debarments, certificates, preferences, classifications] =
      await Promise.all([
        api.query.pramaanRuleRegistry.rules.entries(),
        api.query.pramaanConsistency.declarations.entries(),
        api.query.pramaanDebarment.debarments.entries(),
        api.query.pramaanCertification.certificates.entries(),
        api.query.pramaanPreference.preferenceResults.entries(),
        // Every classification result the chain holds, so the verdict distribution is
        // chain-wide and populated on load, not limited to what happened to be in the
        // recent event window. ClassResult maps to the tri-state a judge reads:
        // ClassOne -> GREEN, ClassTwo / ManualReviewRequired -> YELLOW, NonLocal -> RED.
        api.query.pramaanClassification.classifications.entries(),
      ]);

    // --- Ministries -------------------------------------------------------------------
    const ministries = await Promise.all(
      rules.map(async ([key]) => {
        const id = key.args[0];
        const version = await api.query.pramaanRuleRegistry.ruleVersion(id);
        return { ministryId: decodeId(id.toHex()), ruleVersion: toNumber(version.toString()) };
      }),
    );
    ministries.sort((a, b) => a.ministryId.localeCompare(b.ministryId));

    // --- Declarations and the contradictions inside them --------------------------------
    // Reproduces the pallet's own test exactly: `abs_diff(new, prior) > ToleranceBps`,
    // counted once per contradicting prior, which is one `InconsistencyFlagged` event each.
    const toleranceBps = toNumber(api.consts.pramaanConsistency?.toleranceBps?.toString() ?? '0');
    let totalDeclarations = 0;
    let inconsistenciesFlagged = 0;
    let vendorProductPairsFlagged = 0;

    for (const [, value] of declarations) {
      const history = (value.toJSON() as { localContentBps?: number; local_content_bps?: number }[]) ?? [];
      totalDeclarations += history.length;
      let pairFlagged = false;
      for (let i = 1; i < history.length; i += 1) {
        const current = history[i].localContentBps ?? history[i].local_content_bps ?? 0;
        for (let j = 0; j < i; j += 1) {
          const prior = history[j].localContentBps ?? history[j].local_content_bps ?? 0;
          if (Math.abs(current - prior) > toleranceBps) {
            inconsistenciesFlagged += 1;
            pairFlagged = true;
          }
        }
      }
      if (pairFlagged) vendorProductPairsFlagged += 1;
    }

    // --- Debarments, split active vs. lapsed at the current head -------------------------
    // `debarmentRecords` is every record CURRENTLY held in
    // `pramaanDebarment.debarments`, active plus lapsed — not a cumulative total of every
    // debarment ever issued. `lift_debarment` removes the record outright, so lifting one
    // decrements this figure; a name like `totalDebarments` promised a running total the
    // storage cannot supply and would have read as a fall in enforcement activity.
    const byMinistry = new Map<string, { active: number; lapsed: number }>();
    let activeDebarments = 0;
    let debarmentRecords = 0;

    for (const [key, value] of debarments) {
      const vendor = key.args[0].toString();
      const records =
        (value.toJSON() as {
          ministry?: string;
          effectiveFrom?: number;
          effective_from?: number;
          effectiveTo?: number | null;
          effective_to?: number | null;
        }[]) ?? [];
      for (const record of records) {
        debarmentRecords += 1;
        const ministry = decodeId(record.ministry);
        const from = record.effectiveFrom ?? record.effective_from ?? 0;
        const to = record.effectiveTo ?? record.effective_to ?? null;
        const active = from <= currentBlock && (to === null || to >= currentBlock);
        if (active) activeDebarments += 1;
        const bucket = byMinistry.get(ministry) ?? { active: 0, lapsed: 0 };
        if (active) bucket.active += 1;
        else bucket.lapsed += 1;
        byMinistry.set(ministry, bucket);
      }
      void vendor;
    }

    const debarmentsByMinistry = [...byMinistry.entries()]
      .map(([ministryId, counts]) => ({ ministryId, ...counts }))
      .sort((a, b) => b.active - a.active || a.ministryId.localeCompare(b.ministryId));

    // --- Verdicts observed this session ------------------------------------------------
    const ledger = verdictLedger();
    const totals = { GREEN: 0, YELLOW: 0, RED: 0 };
    for (const entry of ledger.entries) totals[entry.status] += 1;

    // --- Chain-wide verdict distribution ----------------------------------------------
    // From every stored classification result, not the recent-event window, so the chart
    // reflects the whole chain and is populated the moment the page loads.
    const chainTotals = { GREEN: 0, YELLOW: 0, RED: 0 };
    for (const [, value] of classifications) {
      const raw = value.toString();
      const cls = raw.includes('ClassOne')
        ? 'GREEN'
        : raw.includes('NonLocal')
          ? 'RED'
          : 'YELLOW'; // ClassTwo and ManualReviewRequired both read as caveat
      chainTotals[cls] += 1;
    }

    return Response.json({
      chain: { currentBlock, finalizedBlock },
      counters: {
        ministriesOnboarded: rules.length,
        totalDeclarations,
        declarationPairs: declarations.length,
        inconsistenciesFlagged,
        vendorProductPairsFlagged,
        toleranceBps,
        certificatesIssued: certificates.length,
        preferenceDecisions: preferences.length,
        activeDebarments,
        /** Records held in storage right now, active plus lapsed. Never a total-ever. */
        debarmentRecords,
      },
      ministries,
      debarmentsByMinistry,
      verdicts: {
        sessionStartedAt: ledger.sessionStartedAt,
        totals,
        chainTotals,
        entries: ledger.entries,
      },
      sources: {
        verdicts: 'pramaanClassification.Classified / ManualReviewRequired events',
        debarments: 'pramaanDebarment.debarments',
        declarations: 'pramaanConsistency.declarations',
        ministries: 'pramaanRuleRegistry.rules',
      },
      window: indexWindow(),
    });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
