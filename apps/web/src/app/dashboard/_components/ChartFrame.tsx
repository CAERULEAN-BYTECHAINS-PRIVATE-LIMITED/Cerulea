'use client';

import { useId, useState, type ReactNode } from 'react';
import {
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from '@/components';
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
    <Panel>
      <PanelHead title={title} meta={actions} />
      <PanelBody className="space-y-3">
        {legend}
        {children}
        {table && (
          <div className="border-t border-line pt-2.5">
            <button
              type="button"
              onClick={() => setShowTable((value) => !value)}
              aria-expanded={showTable}
              aria-controls={tableId}
              className="rounded-sm text-2xs font-semibold tracking-wide text-accent uppercase transition-colors duration-150 hover:text-accent-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
            >
              {showTable ? 'Hide the numbers' : 'Show the numbers'}
            </button>
            {showTable && (
              <div id={tableId} className="mt-2">
                <Table>
                  <THead>
                    <TR>
                      {table.columns.map((column, index) => (
                        <TH key={column} numeric={index !== 0}>
                          {column}
                        </TH>
                      ))}
                    </TR>
                  </THead>
                  <TBody>
                    {table.rows.map((row) => (
                      <TR key={String(row[0])}>
                        {row.map((cell, index) => (
                          <TD key={index} numeric={index !== 0}>
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
      </PanelBody>
      <PanelNote>{footnote ?? description}</PanelNote>
    </Panel>
  );
}

/** A legend row. Identity comes from the swatch; the text stays in an ink token. */
export function ChartLegend({
  items,
}: {
  items: { color: string; label: string; value?: string }[];
}) {
  return (
    <ul className="flex flex-wrap items-center gap-x-4 gap-y-1.5">
      {items.map((item) => (
        <li key={item.label} className="flex items-center gap-1.5 text-2xs text-ink-muted">
          <span
            aria-hidden="true"
            className="size-2 shrink-0 rounded-[2px]"
            style={{ backgroundColor: item.color }}
          />
          <span>{item.label}</span>
          {item.value !== undefined && (
            <span className="font-mono text-2xs text-ink tabular-nums">{item.value}</span>
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
      className="rounded-md border border-line-strong px-2.5 py-1.5 shadow-pop"
      style={{ backgroundColor: CHROME.surface }}
    >
      <p className="text-2xs font-semibold text-ink">{title}</p>
      <ul className="mt-1 space-y-0.5">
        {rows.map((row) => (
          <li key={row.label} className="flex items-center gap-2 text-2xs text-ink-muted">
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
