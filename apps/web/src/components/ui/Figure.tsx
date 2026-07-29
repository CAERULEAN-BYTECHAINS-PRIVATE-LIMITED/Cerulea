import type { ReactNode } from 'react';
import { cn } from './cn';

/**
 * A headline figure in a counter row.
 *
 * Deliberately plain and deliberately small — 28px, not 48px. There is no accent colour,
 * no sparkline and no trend arrow: a figure that decorates itself is a figure a reader
 * stops trusting. The label sits above in 11px caps and the provenance line sits below.
 */
export function Figure({
  label,
  value,
  note,
  mono = false,
  className,
}: {
  label: ReactNode;
  value: ReactNode;
  /** Where the number came from, or what it does not cover. */
  note?: ReactNode;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div className={cn('border border-line bg-paper px-3 py-2.5 first:rounded-l-md last:rounded-r-md', className)}>
      <p className="text-2xs font-semibold tracking-wide text-ink-muted uppercase">{label}</p>
      <p
        className={cn(
          'mt-1 text-3xl leading-none font-semibold tracking-tight text-ink tabular-nums',
          mono && 'font-mono text-2xl',
        )}
      >
        {value}
      </p>
      {note && <p className="mt-1.5 text-2xs leading-snug text-ink-muted">{note}</p>}
    </div>
  );
}

/**
 * The counter row itself. On a wide screen the figures butt against one another and share
 * a single hairline grid, the way a return form does; below `sm` they stack.
 */
export function FigureRow({
  className,
  children,
  columns = 4,
}: {
  className?: string;
  children: ReactNode;
  columns?: 3 | 4;
}) {
  return (
    <div
      className={cn(
        'grid grid-cols-1 gap-2 sm:grid-cols-2',
        columns === 4 ? 'xl:grid-cols-4' : 'lg:grid-cols-3',
        className,
      )}
    >
      {children}
    </div>
  );
}
