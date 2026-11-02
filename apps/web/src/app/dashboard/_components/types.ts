/** Shapes returned by `/api/chain/metrics` and `/api/chain/latency`. */

export interface VerdictEntry {
  id: string;
  blockNumber: number;
  timestamp: number;
  status: 'GREEN' | 'YELLOW' | 'RED';
  classResult: string;
  vendor: string | null;
  tender: string | null;
  txRef: string | null;
}

export interface MinistryDebarments {
  ministryId: string;
  active: number;
  lapsed: number;
}

export interface MetricsResponse {
  chain: { currentBlock: number; finalizedBlock: number };
  counters: {
    ministriesOnboarded: number;
    totalDeclarations: number;
    declarationPairs: number;
    inconsistenciesFlagged: number;
    vendorProductPairsFlagged: number;
    toleranceBps: number;
    certificatesIssued: number;
    preferenceDecisions: number;
    activeDebarments: number;
    /** Debarment records held in storage now, active plus lapsed — not a total-ever. */
    debarmentRecords: number;
  };
  ministries: { ministryId: string; ruleVersion: number }[];
  debarmentsByMinistry: MinistryDebarments[];
  verdicts: {
    sessionStartedAt: number;
    totals: Record<'GREEN' | 'YELLOW' | 'RED', number>;
    chainTotals: Record<'GREEN' | 'YELLOW' | 'RED', number>;
    entries: VerdictEntry[];
  };
  window: { from: number; to: number; blocksIndexed: number; gapped: boolean };
}

export interface TriggerLatencySummary {
  trigger: string;
  label: string;
  samples: number;
  meanMs: number;
  minMs: number;
  maxMs: number;
  lastMs: number;
}

export interface LatencyResponse {
  sessionStartedAt: number;
  totalSamples: number;
  byTrigger: TriggerLatencySummary[];
  samples: {
    trigger: string;
    latencyMs: number;
    blockNumber: number | null;
    txRef: string | null;
    recordedAt: number;
  }[];
}
