'use client';

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
import { Empty } from '@/components';
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
  chainTotals,
  now,
}: {
  entries: VerdictEntry[];
  sessionStartedAt: number;
  totals: Record<'GREEN' | 'YELLOW' | 'RED', number>;
  /**
   * The verdict split across every classification the chain holds, not just the ones
   * observed since this console connected. When no bid has been classified this session
   * the timeline has nothing to draw, so the chart falls back to this chain-wide
   * distribution — one aggregate bar — rather than an empty panel.
   */
  chainTotals: Record<'GREEN' | 'YELLOW' | 'RED', number>;
  /**
   * When the figures were last read, so the axis runs up to the present rather than
   * stopping at the last verdict. Passed in rather than read from `Date.now()` here:
   * a clock read during render is impure, and the poll that produced `entries` already
   * knows the time it landed.
   */
  now: number;
}) {
  const buckets = useMemo<Bucket[]>(() => {
    if (entries.length === 0) return [];
    const first = Math.min(...entries.map((entry) => entry.timestamp), sessionStartedAt);
    const last = Math.max(...entries.map((entry) => entry.timestamp), now);
    const firstBucket = Math.floor(first / BUCKET_MS) * BUCKET_MS;
    const lastBucket = Math.floor(last / BUCKET_MS) * BUCKET_MS;

    // Tallied first, then the bucket objects are built from the tallies in one pass.
    // Constructing them empty and incrementing afterwards is the obvious way to write
    // this and it crashes: the React Compiler freezes values produced inside a memo, so
    // the increment throws on a read-only property.
    const tally = new Map<number, { GREEN: number; YELLOW: number; RED: number }>();
    for (const entry of entries) {
      const key = Math.floor(entry.timestamp / BUCKET_MS) * BUCKET_MS;
      const counts = tally.get(key) ?? { GREEN: 0, YELLOW: 0, RED: 0 };
      tally.set(key, { ...counts, [entry.status]: counts[entry.status] + 1 });
    }

    const all: Bucket[] = [];
    for (let start = firstBucket; start <= lastBucket; start += BUCKET_MS) {
      const counts = tally.get(start) ?? { GREEN: 0, YELLOW: 0, RED: 0 };
      all.push({
        key: String(start),
        label: clock(start),
        startedAt: start,
        GREEN: counts.GREEN,
        YELLOW: counts.YELLOW,
        RED: counts.RED,
        total: counts.GREEN + counts.YELLOW + counts.RED,
      });
    }
    return all.slice(-MAX_BUCKETS);
  }, [entries, sessionStartedAt, now]);

  const sessionTotal = totals.GREEN + totals.YELLOW + totals.RED;
  const chainTotal = chainTotals.GREEN + chainTotals.YELLOW + chainTotals.RED;

  // The timeline is session-scoped by nature; when nothing has been classified since this
  // console connected, fall back to the chain-wide distribution as a single aggregate bar
  // so the panel always shows real on-chain data rather than an empty state.
  const showTimeline = sessionTotal > 0;
  const legendTotals = showTimeline ? totals : chainTotals;
  const displayBuckets: Bucket[] = showTimeline
    ? buckets
    : [
        {
          key: 'chain-wide',
          label: 'All on chain',
          startedAt: 0,
          GREEN: chainTotals.GREEN,
          YELLOW: chainTotals.YELLOW,
          RED: chainTotals.RED,
          total: chainTotal,
        },
      ];

  if (chainTotal === 0 && sessionTotal === 0) {
    return (
      <ChartFrame
        title="Compliance verdicts"
        description="Every classification the chain has returned, split by outcome."
      >
        <Empty
          title="No verdicts recorded on this chain yet"
          source="This chart reads pramaanClassification.classifications from chain state. Submit a bid from the vendor console and the first bar appears within a second."
        />
      </ChartFrame>
    );
  }

  return (
    <ChartFrame
      title={showTimeline ? 'Compliance verdicts this session' : 'Compliance verdicts on this chain'}
      description={
        showTimeline
          ? `${sessionTotal.toLocaleString('en-IN')} classification${sessionTotal === 1 ? '' : 's'} finalized since this console connected, in half-minute intervals.`
          : `${chainTotal.toLocaleString('en-IN')} classification${chainTotal === 1 ? '' : 's'} recorded on this chain, by outcome. A bid classified this session switches this panel to a live timeline.`
      }
      legend={
        <ChartLegend
          items={TRI_STATES.map((status) => ({
            color: STATUS_COLORS[status],
            label: `${STATUS_LABELS[status]} (${status})`,
            value: legendTotals[status].toLocaleString('en-IN'),
          }))}
        />
      }
      footnote={
        showTimeline
          ? 'Counted from pramaanClassification.Classified and ManualReviewRequired events decoded out of finalized blocks. Nothing here is estimated or carried over from a previous run.'
          : 'Read from pramaanClassification.classifications in chain state — the full set of stored verdicts. ClassOne is compliant, NonLocal is blocked, ClassTwo and ManualReviewRequired need review. Nothing here is estimated.'
      }
      table={{
        columns: [
          showTimeline ? 'Interval' : 'Scope',
          'Compliant',
          'Review required',
          'Blocked',
          'Total',
        ],
        rows: displayBuckets.map((bucket) => [
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
          <BarChart data={displayBuckets} margin={{ top: 8, right: 8, bottom: 4, left: 0 }}>
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
                    title={showTimeline ? `Interval starting ${String(label)}` : 'All classifications on chain'}
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
