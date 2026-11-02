/**
 * A running ledger of the compliance verdicts this console has actually observed on chain.
 *
 * The block index in `reader.ts` is a bounded ring buffer, so an event emitted early in a
 * long demo eventually falls out of it. The dashboard's session counts must not fall with
 * it — a judge who watched a RED at the start of the walkthrough should still see it
 * counted at the end. So every verdict event the reader surfaces is copied here once,
 * deduplicated by its `blockNumber-index` id, and kept for the life of the process.
 *
 * This ledger is derived, never authored: an entry exists only because a
 * `pramaanClassification.Classified` (or `ManualReviewRequired`) event was decoded out of
 * a block. It is not seeded and it is not persisted across restarts, which is why the
 * dashboard labels it "this session" rather than "to date".
 */

import type { EventRecord } from './reader';

export type TriState = 'GREEN' | 'YELLOW' | 'RED';

export interface VerdictEntry {
  id: string;
  blockNumber: number;
  timestamp: number;
  status: TriState;
  /** `ClassOne` / `ClassTwo` / `NonLocal` / `ManualReviewRequired`. */
  classResult: string;
  vendor: string | null;
  tender: string | null;
  txRef: string | null;
}

const globalForVerdicts = globalThis as unknown as {
  __pramaanVerdicts?: { byId: Map<string, VerdictEntry>; sessionStartedAt: number };
};

function store() {
  globalForVerdicts.__pramaanVerdicts ??= { byId: new Map(), sessionStartedAt: Date.now() };
  return globalForVerdicts.__pramaanVerdicts;
}

/** Mirrors `classToTriState` in `src/lib/pramaan.ts` — one meaning, stated the same way. */
function toTriState(classResult: string): TriState {
  switch (classResult) {
    case 'ClassOne':
      return 'GREEN';
    case 'ClassTwo':
    case 'ManualReviewRequired':
      return 'YELLOW';
    default:
      return 'RED';
  }
}

function text(value: unknown): string | null {
  if (typeof value === 'string') return value;
  if (Array.isArray(value)) return value.map(String).join('');
  return null;
}

export function ingestVerdicts(events: EventRecord[]): void {
  const ledger = store();
  for (const event of events) {
    if (event.section !== 'pramaanClassification') continue;
    if (event.method !== 'Classified' && event.method !== 'ManualReviewRequired') continue;
    if (ledger.byId.has(event.id)) continue;

    const classResult =
      event.method === 'ManualReviewRequired'
        ? 'ManualReviewRequired'
        : (text(event.data.class) ?? 'NonLocal');

    ledger.byId.set(event.id, {
      id: event.id,
      blockNumber: event.blockNumber,
      timestamp: event.timestamp,
      status: toTriState(classResult),
      classResult,
      vendor: text(event.data.vendor),
      tender: text(event.data.tender),
      txRef: event.txRef,
    });
  }
}

export function verdictLedger(): { sessionStartedAt: number; entries: VerdictEntry[] } {
  const ledger = store();
  return {
    sessionStartedAt: ledger.sessionStartedAt,
    entries: [...ledger.byId.values()].sort((a, b) => a.blockNumber - b.blockNumber),
  };
}
