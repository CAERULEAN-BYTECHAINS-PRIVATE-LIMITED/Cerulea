import type { ReactNode } from 'react';
import { cn } from './cn';

/**
 * A single headline figure. Kept deliberately plain: one number, one label, one optional
 * footnote — no sparkline chrome, no accent colour that could be mistaken for a verdict.
 */
export function Stat({
  label,
  value,
  hint,
  icon,
  mono = false,
  className,
}: {
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
  icon?: ReactNode;
  mono?: boolean;
  className?: string;
}) {
  return (
    <div className={cn('rounded-card border border-border bg-surface px-5 py-4', className)}>
      <div className="flex items-center gap-2 text-xs font-semibold tracking-wide text-ink-muted uppercase">
        {icon}
        <span>{label}</span>
      </div>
      <p
        className={cn(
          'mt-2 text-2xl font-semibold tracking-tight text-ink tabular-nums',
          mono && 'font-mono text-xl',
        )}
      >
        {value}
      </p>
      {hint && <p className="mt-1 text-xs text-ink-muted">{hint}</p>}
    </div>
  );
}
