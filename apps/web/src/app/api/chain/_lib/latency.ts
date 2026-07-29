/**
 * Measured trigger-point latency for the analytics dashboard.
 *
 * Every sample here is a real wall-clock measurement returned by one of the six
 * trigger-point routes: `submitAndFinalize` in `src/lib/chain.ts` stamps `latencyMs` as
 * submission-to-confirmed-finality, and the route passes it back in its response. The
 * walkthrough at `/demo` records each one as it runs, and the dashboard plots them.
 *
 * There is deliberately no seeding, no synthetic sample, and no default series. Before a
 * trigger point has run in this session the store is empty and the dashboard says so.
 * A latency chart that draws a plausible line before anything has been measured would be
 * the single most misleading thing this PoC could put on a projector.
 */

/** The six trigger points, in the order the PoC document walks through them. */
export const TRIGGER_POINTS = [
  'bid-submission',
  'bid-evaluation',
  'preference-calculation',
  'ca-certification',
  'debarment',
  'rule-update',
] as const;

export type TriggerPoint = (typeof TRIGGER_POINTS)[number];

export const TRIGGER_LABELS: Record<TriggerPoint, string> = {
  'bid-submission': 'Bid submission',
  'bid-evaluation': 'Bid evaluation',
  'preference-calculation': 'Preference calculation',
  'ca-certification': 'CA certification',
  debarment: 'Debarment',
  'rule-update': 'Rule update',
};

export interface LatencySample {
  trigger: TriggerPoint;
  /** Submission to confirmed finality, in milliseconds. */
  latencyMs: number;
  blockNumber: number | null;
  txRef: string | null;
  recordedAt: number;
}

const MAX_SAMPLES = 300;

const globalForLatency = globalThis as unknown as {
  __pramaanLatencyV2?: { samples: LatencySample[]; sessionStartedAt: number };
};

function store() {
  globalForLatency.__pramaanLatencyV2 ??= { samples: [], sessionStartedAt: Date.now() };
  return globalForLatency.__pramaanLatencyV2;
}

export function isTriggerPoint(value: unknown): value is TriggerPoint {
  return typeof value === 'string' && (TRIGGER_POINTS as readonly string[]).includes(value);
}

export function recordLatency(sample: LatencySample): void {
  const state = store();
  state.samples.push(sample);
  if (state.samples.length > MAX_SAMPLES) {
    state.samples.splice(0, state.samples.length - MAX_SAMPLES);
  }
}

export interface TriggerLatencySummary {
  trigger: TriggerPoint;
  label: string;
  samples: number;
  meanMs: number;
  minMs: number;
  maxMs: number;
  lastMs: number;
}

export function latencySummary(): {
  sessionStartedAt: number;
  totalSamples: number;
  byTrigger: TriggerLatencySummary[];
  samples: LatencySample[];
} {
  const state = store();
  const byTrigger: TriggerLatencySummary[] = [];

  for (const trigger of TRIGGER_POINTS) {
    const measured = state.samples.filter((sample) => sample.trigger === trigger);
    if (measured.length === 0) continue;
    const values = measured.map((sample) => sample.latencyMs);
    byTrigger.push({
      trigger,
      label: TRIGGER_LABELS[trigger],
      samples: values.length,
      meanMs: Math.round(values.reduce((total, value) => total + value, 0) / values.length),
      minMs: Math.min(...values),
      maxMs: Math.max(...values),
      lastMs: values[values.length - 1],
    });
  }

  return {
    sessionStartedAt: state.sessionStartedAt,
    totalSamples: state.samples.length,
    byTrigger,
    samples: state.samples,
  };
}
