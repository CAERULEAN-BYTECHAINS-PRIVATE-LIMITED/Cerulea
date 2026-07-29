import type { ReactNode } from 'react';
import { cn } from './cn';

/**
 * The record view: a label column and a value column, hairline-separated.
 *
 * This is what a government file looks like on paper, and it is used for every detail
 * panel in the product. `columns` sets how many label/value pairs sit side by side —
 * one for a narrow sidebar, two for a full-width record.
 */
export function DataList({
  columns = 1,
  className,
  children,
}: {
  columns?: 1 | 2;
  className?: string;
  children: ReactNode;
}) {
  return (
    <dl
      className={cn(
        'grid grid-cols-1',
        columns === 2 && 'sm:grid-cols-2 sm:gap-x-6',
        // A two-column list draws its own top hairline so the first row of each column
        // reads as a row, not as a heading.
        'border-t border-line',
        className,
      )}
    >
      {children}
    </dl>
  );
}

export function DataRow({
  label,
  value,
  mono = false,
  className,
}: {
  label: ReactNode;
  value: ReactNode;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-baseline justify-between gap-x-4 gap-y-0.5 border-b border-line py-1.5',
        className,
      )}
    >
      <dt className="text-xs text-ink-muted">{label}</dt>
      <dd className={cn('text-sm font-medium text-ink', mono && 'font-mono text-xs')}>{value}</dd>
    </div>
  );
}

/**
 * A compact field/value grid for facts that read across rather than down — the inputs to
 * a walkthrough step, the parameters of a request. Stacked label above value, no rules.
 */
export function FactGrid({
  facts,
  className,
}: {
  facts: { label: ReactNode; value: ReactNode; mono?: boolean }[];
  className?: string;
}) {
  return (
    <dl
      className={cn(
        'grid grid-cols-1 gap-x-6 gap-y-2.5 sm:grid-cols-2 lg:grid-cols-3',
        className,
      )}
    >
      {facts.map((fact, index) => (
        <div key={index} className="min-w-0">
          <dt className="text-2xs tracking-wide text-ink-muted uppercase">{fact.label}</dt>
          <dd
            className={cn(
              'mt-0.5 text-sm font-medium break-words text-ink',
              fact.mono && 'font-mono text-xs',
            )}
          >
            {fact.value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
