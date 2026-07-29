'use client';

import { Gavel } from 'lucide-react';
import { useMemo } from 'react';
import {
  Bar,
  BarChart,
  CartesianGrid,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { EmptyState } from '@/components';
import { AXIS_TICK, CHROME, MARKS, STATUS_COLORS, STATUS_LABELS, TRI_STATES } from './chart-theme';
import { ChartFrame, ChartLegend, TooltipShell } from './ChartFrame';
import type { VerdictEntry } from './types';

/** Half-minute buckets: a 250 ms chain fills a minute-wide bar too fast to read. */
const BUCKET_MS = 30_000;
const MAX_BUCKETS = 16;

interface Bucket {
  key: string;
  label: string;
  startedAt: number;
  GREEN: number;
  YELLOW: number;
  RED: number;
  total: number;
}

function clock(timestamp: number): string {
  return new Date(timestamp).toLocaleTimeString('en-IN', {
    minute: '2-digit',
    second: '2-digit',
    hour12: false,
  });
}

/**
 * GREEN / YELLOW / RED verdicts over the session, stacked.
 *
 * This is the one chart in the application entitled to the reserved status colours: the
 * series *are* the compliance verdicts, so the colour is the data rather than decoration.
 * Each series still carries its own name in the legend and its own row in the table view,
 * so the reading never depends on hue alone.
 */
export function VerdictChart({
  entries,
  sessionStartedAt,
  totals,
}: {
  entries: VerdictEntry[];
  sessionStartedAt: number;
  totals: Record<'GREEN' | 'YELLOW' | 'RED', number>;
}) {
  const buckets = useMemo<Bucket[]>(() => {
    if (entries.length === 0) return [];
    const first = Math.min(...entries.map((entry) => entry.timestamp), sessionStartedAt);
    const last = Math.max(...entries.map((entry) => entry.timestamp), Date.now());
    const firstBucket = Math.floor(first / BUCKET_MS) * BUCKET_MS;
    const lastBucket = Math.floor(last / BUCKET_MS) * BUCKET_MS;

    const all: Bucket[] = [];
    for (let start = firstBucket; start <= lastBucket; start += BUCKET_MS) {
      all.push({
        key: String(start),
        label: clock(start),
        startedAt: start,
        GREEN: 0,
        YELLOW: 0,
        RED: 0,
        total: 0,
      });
    }
    const index = new Map(all.map((bucket) => [bucket.key, bucket]));
    for (const entry of entries) {
      const bucket = index.get(String(Math.floor(entry.timestamp / BUCKET_MS) * BUCKET_MS));
      if (!bucket) continue;
      bucket[entry.status] += 1;
      bucket.total += 1;
    }
    return all.slice(-MAX_BUCKETS);
  }, [entries, sessionStartedAt]);

  const total = totals.GREEN + totals.YELLOW + totals.RED;

  if (total === 0) {
    return (
      <ChartFrame
        title="Compliance verdicts this session"
        description="Every classification the chain has returned since this console connected, split by outcome."
      >
        <EmptyState
          icon={<Gavel className="size-5" aria-hidden="true" />}
          title="No verdicts recorded yet in this session"
          description="This chart counts pramaanClassification events as they are finalized. Run the guided walkthrough, or submit a bid from the vendor console, and the first bar appears within a second."
        />
      </ChartFrame>
    );
  }

  return (
    <ChartFrame
      title="Compliance verdicts this session"
      description={`${total.toLocaleString('en-IN')} classification${total === 1 ? '' : 's'} finalized since this console connected, in half-minute intervals.`}
      legend={
        <ChartLegend
          items={TRI_STATES.map((status) => ({
            color: STATUS_COLORS[status],
            label: `${STATUS_LABELS[status]} (${status})`,
            value: totals[status].toLocaleString('en-IN'),
          }))}
        />
      }
      footnote="Counted from pramaanClassification.Classified and ManualReviewRequired events decoded out of finalized blocks. Nothing here is estimated or carried over from a previous run."
      table={{
        columns: ['Interval', 'Compliant', 'Review required', 'Blocked', 'Total'],
        rows: buckets.map((bucket) => [
          bucket.label,
          bucket.GREEN,
          bucket.YELLOW,
          bucket.RED,
          bucket.total,
        ]),
      }}
    >
      <div className="h-72 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <BarChart data={buckets} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
            <CartesianGrid stroke={CHROME.grid} strokeWidth={1} vertical={false} />
            <XAxis
              dataKey="label"
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={{ stroke: CHROME.grid }}
              interval="preserveStartEnd"
              minTickGap={16}
            />
            <YAxis
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={false}
              allowDecimals={false}
              width={36}
            />
            <Tooltip
              cursor={{ fill: CHROME.grid, fillOpacity: 0.45 }}
              content={({ active, payload, label }) => {
                if (!active || !payload?.length) return null;
                return (
                  <TooltipShell
                    title={`Interval starting ${String(label)}`}
                    rows={payload
                      .filter((item) => Number(item.value) > 0)
                      .map((item) => ({
                        color: String(item.color),
                        label: STATUS_LABELS[item.dataKey as 'GREEN' | 'YELLOW' | 'RED'],
                        value: String(item.value),
                      }))}
                  />
                );
              }}
            />
            {TRI_STATES.map((status) => (
              <Bar
                key={status}
                dataKey={status}
                name={STATUS_LABELS[status]}
                stackId="verdicts"
                fill={STATUS_COLORS[status]}
                // A 2px stroke in the surface colour is the surface gap: it separates the
                // stacked segments with white rather than with a drawn border.
                stroke={CHROME.surface}
                strokeWidth={MARKS.surfaceGap}
                maxBarSize={MARKS.maxBarSize}
                isAnimationActive={false}
              />
            ))}
          </BarChart>
        </ResponsiveContainer>
      </div>
    </ChartFrame>
  );
}
