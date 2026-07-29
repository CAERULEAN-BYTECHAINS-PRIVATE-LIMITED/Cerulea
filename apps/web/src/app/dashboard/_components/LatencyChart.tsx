'use client';

import { Timer } from 'lucide-react';
import { useMemo } from 'react';
import {
  CartesianGrid,
  Line,
  LineChart,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts';
import { EmptyState } from '@/components';
import { ACCENT, AXIS_TICK, CHROME, MARKS } from './chart-theme';
import { ChartFrame, TooltipShell } from './ChartFrame';
import type { TriggerLatencySummary } from './types';

/** The claim the chart exists to test: a decision inside one second, after finality. */
const SUB_SECOND_MS = 1_000;

/**
 * Measured latency per trigger point.
 *
 * Every point is wall-clock time from submitting the extrinsic to confirming that its
 * block reached DCF finality — the figure `submitAndFinalize` stamps on each response.
 * It is not the time to inclusion, which would be a much easier number to publish and a
 * meaningless one: an included transaction is not a decision.
 *
 * One series, so no legend box — the title names what is plotted. The reference line at
 * one second is the only other mark, because it is the claim being tested.
 */
export function LatencyChart({ byTrigger }: { byTrigger: TriggerLatencySummary[] }) {
  const data = useMemo(
    () =>
      byTrigger.map((entry) => ({
        label: entry.label,
        meanMs: entry.meanMs,
        minMs: entry.minMs,
        maxMs: entry.maxMs,
        samples: entry.samples,
      })),
    [byTrigger],
  );

  if (data.length === 0) {
    return (
      <ChartFrame
        title="Measured latency per trigger point"
        description="Submission to confirmed finality, in milliseconds, for each of the six trigger points."
      >
        <EmptyState
          icon={<Timer className="size-5" aria-hidden="true" />}
          title="No latency measured in this session yet"
          description="This chart plots only figures this console has actually measured — the submission-to-finality time each trigger-point route returns. Run the guided walkthrough and all six points are measured in order; the first bar of the chart appears after the first step."
        />
      </ChartFrame>
    );
  }

  const slowest = Math.max(...data.map((point) => point.maxMs));
  const fastest = Math.min(...data.map((point) => point.minMs));
  const domainMax = Math.max(SUB_SECOND_MS * 1.15, Math.ceil((slowest * 1.25) / 100) * 100);
  const allSubSecond = slowest < SUB_SECOND_MS;
  const totalSamples = data.reduce((total, point) => total + point.samples, 0);

  return (
    <ChartFrame
      title="Measured latency per trigger point"
      description={`Submission to confirmed DCF finality, averaged over ${totalSamples.toLocaleString('en-IN')} measurement${totalSamples === 1 ? '' : 's'} taken in this session.`}
      actions={
        <span className="font-mono text-sm font-semibold text-ink tabular-nums">
          {fastest} – {slowest} ms
        </span>
      }
      footnote={
        allSubSecond
          ? 'Every measurement above sits below the one-second reference line, and each was taken only after the block carrying the transaction reached finality — not at inclusion.'
          : 'At least one measurement crossed the one-second reference line. The figure is reported as measured; nothing here is smoothed or discarded.'
      }
      table={{
        columns: ['Trigger point', 'Mean (ms)', 'Fastest (ms)', 'Slowest (ms)', 'Measurements'],
        rows: data.map((point) => [
          point.label,
          point.meanMs,
          point.minMs,
          point.maxMs,
          point.samples,
        ]),
      }}
    >
      <div className="h-72 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <LineChart data={data} margin={{ top: 16, right: 16, bottom: 4, left: 0 }}>
            <CartesianGrid stroke={CHROME.grid} strokeWidth={1} vertical={false} />
            <XAxis
              dataKey="label"
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={{ stroke: CHROME.grid }}
              interval={0}
              minTickGap={4}
            />
            <YAxis
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={false}
              width={52}
              domain={[0, domainMax]}
              tickFormatter={(value: number) => `${value.toLocaleString('en-IN')}`}
              label={{
                value: 'ms',
                position: 'insideTopLeft',
                offset: 0,
                fill: CHROME.axisSubtle,
                fontSize: 11,
              }}
            />
            <ReferenceLine
              y={SUB_SECOND_MS}
              stroke={CHROME.axis}
              strokeWidth={1}
              label={{
                value: 'One second',
                position: 'insideTopRight',
                fill: CHROME.axis,
                fontSize: 11,
              }}
            />
            <Tooltip
              cursor={{ stroke: CHROME.grid, strokeWidth: 1 }}
              content={({ active, payload, label }) => {
                if (!active || !payload?.length) return null;
                const point = payload[0].payload as (typeof data)[number];
                return (
                  <TooltipShell
                    title={String(label)}
                    rows={[
                      { color: ACCENT, label: 'Mean', value: `${point.meanMs} ms` },
                      { label: 'Fastest', value: `${point.minMs} ms` },
                      { label: 'Slowest', value: `${point.maxMs} ms` },
                      { label: 'Measurements', value: String(point.samples) },
                    ]}
                  />
                );
              }}
            />
            <Line
              type="monotone"
              dataKey="meanMs"
              name="Mean latency"
              stroke={ACCENT}
              strokeWidth={MARKS.lineWidth}
              strokeLinecap="round"
              strokeLinejoin="round"
              // The 2px surface ring keeps a marker legible where it crosses the line.
              dot={{
                r: MARKS.dotRadius,
                fill: ACCENT,
                stroke: CHROME.surface,
                strokeWidth: MARKS.surfaceGap,
              }}
              activeDot={{
                r: MARKS.dotRadius + 2,
                fill: ACCENT,
                stroke: CHROME.surface,
                strokeWidth: MARKS.surfaceGap,
              }}
              isAnimationActive={false}
            />
          </LineChart>
        </ResponsiveContainer>
      </div>
    </ChartFrame>
  );
}
