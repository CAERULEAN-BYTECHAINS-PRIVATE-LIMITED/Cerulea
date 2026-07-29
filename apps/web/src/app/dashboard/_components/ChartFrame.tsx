'use client';

import { useId, useState, type ReactNode } from 'react';
import { Card, CardBody, CardHeader, Table, TBody, TD, TH, THead, TR } from '@/components';
import { CHROME } from './chart-theme';

/**
 * The shell every chart on this page sits in: a titled card, a legend, the plot, and a
 * table view of the same numbers behind a disclosure.
 *
 * The table is not decoration. Two of the three charts use a colour that sits below 3:1
 * against a white surface (the reserved `#E8A100`, and the aqua identity slot), which the
 * data-viz checks flag as needing relief — a text route to the same values. It also
 * carries the numbers for anyone reading this on a washed-out projector, and it is what a
 * screen reader gets instead of an SVG.
 */
export function ChartFrame({
  title,
  description,
  legend,
  footnote,
  table,
  children,
  actions,
}: {
  title: string;
  description: string;
  legend?: ReactNode;
  footnote?: ReactNode;
  table?: { columns: string[]; rows: (string | number)[][] };
  children: ReactNode;
  actions?: ReactNode;
}) {
  const [showTable, setShowTable] = useState(false);
  const tableId = useId();

  return (
    <Card>
      <CardHeader title={title} description={description} actions={actions} />
      <CardBody className="space-y-4">
        {legend}
        {children}
        {footnote && <p className="text-xs leading-relaxed text-ink-muted">{footnote}</p>}
        {table && (
          <div className="border-t border-border pt-3">
            <button
              type="button"
              onClick={() => setShowTable((value) => !value)}
              aria-expanded={showTable}
              aria-controls={tableId}
              className="rounded text-sm font-medium text-cerulea transition-colors duration-150 hover:text-cerulea-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
            >
              {showTable ? 'Hide the numbers' : 'Show the numbers'}
            </button>
            {showTable && (
              <div id={tableId} className="mt-3">
                <Table>
                  <THead>
                    <TR>
                      {table.columns.map((column, index) => (
                        <TH key={column} className={index === 0 ? undefined : 'text-right'}>
                          {column}
                        </TH>
                      ))}
                    </TR>
                  </THead>
                  <TBody>
                    {table.rows.map((row) => (
                      <TR key={String(row[0])}>
                        {row.map((cell, index) => (
                          <TD
                            key={index}
                            className={index === 0 ? undefined : 'text-right tabular-nums'}
                            mono={index !== 0}
                          >
                            {cell}
                          </TD>
                        ))}
                      </TR>
                    ))}
                  </TBody>
                </Table>
              </div>
            )}
          </div>
        )}
      </CardBody>
    </Card>
  );
}

/** A legend row. Identity comes from the swatch; the text stays in an ink token. */
export function ChartLegend({
  items,
}: {
  items: { color: string; label: string; value?: string }[];
}) {
  return (
    <ul className="flex flex-wrap items-center gap-x-5 gap-y-2">
      {items.map((item) => (
        <li key={item.label} className="flex items-center gap-2 text-sm text-ink-muted">
          <span
            aria-hidden="true"
            className="size-2.5 shrink-0 rounded-[2px]"
            style={{ backgroundColor: item.color }}
          />
          <span>{item.label}</span>
          {item.value !== undefined && (
            <span className="font-mono text-[0.8125rem] text-ink tabular-nums">{item.value}</span>
          )}
        </li>
      ))}
    </ul>
  );
}

/** Tooltip surface shared by all three charts. */
export function TooltipShell({
  title,
  rows,
}: {
  title: string;
  rows: { color?: string; label: string; value: string }[];
}) {
  return (
    <div
      className="rounded-lg border border-border px-3 py-2 shadow-[0_2px_8px_rgba(26,26,26,0.10)]"
      style={{ backgroundColor: CHROME.surface }}
    >
      <p className="text-xs font-semibold text-ink">{title}</p>
      <ul className="mt-1.5 space-y-1">
        {rows.map((row) => (
          <li key={row.label} className="flex items-center gap-2 text-xs text-ink-muted">
            {row.color && (
              <span
                aria-hidden="true"
                className="size-2 shrink-0 rounded-[2px]"
                style={{ backgroundColor: row.color }}
              />
            )}
            <span>{row.label}</span>
            <span className="ml-auto pl-3 font-mono text-ink tabular-nums">{row.value}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}
