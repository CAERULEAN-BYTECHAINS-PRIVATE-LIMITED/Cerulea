'use client';

import { useMemo } from 'react';
import { Cell, Pie, PieChart, ResponsiveContainer, Tooltip } from 'recharts';
import { Empty } from '@/components';
import { CATEGORICAL, CHROME, MARKS, OTHER_COLOR } from './chart-theme';
import { ChartFrame, ChartLegend, TooltipShell } from './ChartFrame';
import type { MinistryDebarments } from './types';

/**
 * Active debarments by ministry.
 *
 * A part-to-whole read at a glance — how concentrated debarment is across ministries —
 * which is the one thing a donut does better than a bar. Past four ministries the tail
 * folds into a neutral "Other" slice rather than reaching for a fifth identity hue: the
 * validated categorical palette has four slots that separate cleanly on a white surface,
 * and a generated fifth would not.
 *
 * Deliberately no status colours. A debarment is not a compliance verdict on a bid; it is
 * a standing state of a vendor, and painting it red would collapse two different things a
 * judge needs to keep apart.
 */
export function DebarmentChart({ byMinistry }: { byMinistry: MinistryDebarments[] }) {
  const slices = useMemo(() => {
    const active = byMinistry.filter((entry) => entry.active > 0);
    const head = active.slice(0, CATEGORICAL.length);
    const tail = active.slice(CATEGORICAL.length);
    const named: { name: string; value: number; color: string }[] = head.map((entry, index) => ({
      name: entry.ministryId,
      value: entry.active,
      color: CATEGORICAL[index],
    }));
    if (tail.length === 0) return named;
    return [
      ...named,
      {
        name: `Other (${tail.length} ministries)`,
        value: tail.reduce((total, entry) => total + entry.active, 0),
        color: OTHER_COLOR,
      },
    ];
  }, [byMinistry]);

  const total = slices.reduce((sum, slice) => sum + slice.value, 0);

  if (total === 0) {
    return (
      <ChartFrame
        title="Active debarments by ministry"
        description="Vendors currently barred from bidding, grouped by the ministry that issued the debarment."
      >
        <Empty
          title="No debarments recorded on this chain"
          source="pramaanDebarment.debarments is empty. A debarment issued from the ministry administrator console appears here as soon as its block is finalized, and disappears again when it is lifted or lapses."
        />
      </ChartFrame>
    );
  }

  return (
    <ChartFrame
      title="Active debarments by ministry"
      description={`${total.toLocaleString('en-IN')} debarment${total === 1 ? '' : 's'} in force at the current block head.`}
      legend={
        <ChartLegend
          items={slices.map((slice) => ({
            color: slice.color,
            label: slice.name,
            value: `${slice.value} (${Math.round((slice.value / total) * 100)}%)`,
          }))}
        />
      }
      footnote="Read from pramaanDebarment.debarments. A record counts as active when its effective-from block has passed and its effective-to block has not, evaluated against the current head."
      table={{
        columns: ['Ministry', 'Active debarments', 'Share'],
        rows: slices.map((slice) => [
          slice.name,
          slice.value,
          `${Math.round((slice.value / total) * 100)}%`,
        ]),
      }}
    >
      <div className="h-72 w-full">
        <ResponsiveContainer width="100%" height="100%">
          <PieChart margin={{ top: 4, right: 4, bottom: 4, left: 4 }}>
            <Pie
              data={slices}
              dataKey="value"
              nameKey="name"
              innerRadius="58%"
              outerRadius="88%"
              // The surface-coloured stroke is the 2px gap between arcs, not a border.
              stroke={CHROME.surface}
              strokeWidth={MARKS.surfaceGap}
              isAnimationActive={false}
              paddingAngle={0}
            >
              {slices.map((slice) => (
                <Cell key={slice.name} fill={slice.color} />
              ))}
            </Pie>
            <Tooltip
              content={({ active, payload }) => {
                if (!active || !payload?.length) return null;
                const point = payload[0];
                const value = Number(point.value);
                return (
                  <TooltipShell
                    title={String(point.name)}
                    rows={[
                      {
                        color: String((point.payload as { color?: string })?.color ?? ''),
                        label: 'Active debarments',
                        value: `${value} of ${total}`,
                      },
                    ]}
                  />
                );
              }}
            />
          </PieChart>
        </ResponsiveContainer>
      </div>
    </ChartFrame>
  );
}
