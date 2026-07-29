import type { ReactNode } from 'react';
import { cn } from './cn';

/**
 * "Nothing here yet", said in one line.
 *
 * Never a blank panel and never an invented row: it states what would appear here, and
 * where that comes from. The `source` line names the storage item or the action, which is
 * what stops an empty register reading as a broken page.
 */
export function Empty({
  title,
  source,
  action,
  className,
}: {
  /** One clause. "No debarment on this chain." */
  title: string;
  /** Where a row would come from. One short sentence, 11px. */
  source?: ReactNode;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center gap-1.5 border border-dashed border-line bg-shell px-4 py-8 text-center',
        className,
      )}
    >
      <p className="text-sm font-medium text-ink">{title}</p>
      {source && <p className="max-w-lg text-2xs leading-relaxed text-ink-muted">{source}</p>}
      {action && <div className="mt-1.5">{action}</div>}
    </div>
  );
}

/** A neutral loading placeholder. Skeleton rows, never a spinner in the middle of content. */
export function SkeletonRows({
  rows = 6,
  label,
  className,
}: {
  rows?: number;
  label: string;
  className?: string;
}) {
  return (
    <div className={cn('space-y-1 p-3', className)} role="status" aria-live="polite">
      <span className="sr-only">{label}</span>
      {Array.from({ length: rows }, (_, index) => (
        <div key={index} className="h-6 animate-pulse rounded-sm bg-shell-2" />
      ))}
    </div>
  );
}
