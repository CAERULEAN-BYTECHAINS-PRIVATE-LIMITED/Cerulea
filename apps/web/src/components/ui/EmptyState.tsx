import type { ReactNode } from 'react';
import { cn } from './cn';

/**
 * The "nothing here yet" state. Never a blank panel: it always says what would appear
 * here and what the operator can do to make it appear (spec Part 9.8 — no unstyled
 * defaults, no placeholder text).
 */
export function EmptyState({
  icon,
  title,
  description,
  action,
  className,
}: {
  icon?: ReactNode;
  title: string;
  description: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex flex-col items-center justify-center rounded-card border border-dashed border-border bg-surface-sunken px-6 py-12 text-center',
        className,
      )}
    >
      {icon && (
        <div className="mb-3 flex size-11 items-center justify-center rounded-full bg-cerulea-light text-cerulea">
          {icon}
        </div>
      )}
      <p className="text-sm font-semibold text-ink">{title}</p>
      <p className="mt-1 max-w-md text-sm text-ink-muted">{description}</p>
      {action && <div className="mt-4">{action}</div>}
    </div>
  );
}
