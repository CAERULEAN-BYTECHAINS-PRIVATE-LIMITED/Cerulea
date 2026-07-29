import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from './cn';

/**
 * The single surface used across every persona view. Flat, hairline-bordered, one soft
 * shadow — a government console, not a startup dashboard.
 */
export function Card({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'rounded-card border border-border bg-surface shadow-[0_1px_2px_rgba(26,26,26,0.04)]',
        className,
      )}
      {...rest}
    />
  );
}

export function CardHeader({
  className,
  title,
  description,
  actions,
  children,
  ...rest
}: Omit<HTMLAttributes<HTMLDivElement>, 'title'> & {
  title?: ReactNode;
  description?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-start justify-between gap-3 border-b border-border px-5 py-4',
        className,
      )}
      {...rest}
    >
      {(title || description) && (
        <div className="min-w-0">
          {title && <h2 className="text-base font-semibold text-ink">{title}</h2>}
          {description && <p className="mt-1 text-sm text-ink-muted">{description}</p>}
        </div>
      )}
      {children}
      {actions && <div className="flex shrink-0 items-center gap-2">{actions}</div>}
    </div>
  );
}

export function CardBody({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('px-5 py-4', className)} {...rest} />;
}

export function CardFooter({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-center justify-between gap-3 border-t border-border bg-surface-sunken px-5 py-3',
        className,
      )}
      {...rest}
    />
  );
}

/** A label/value row, the workhorse of every detail panel in the demo. */
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
        'flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1 py-2.5',
        'border-b border-border last:border-b-0',
        className,
      )}
    >
      <dt className="text-sm text-ink-muted">{label}</dt>
      <dd className={cn('text-sm font-medium text-ink', mono && 'font-mono text-[0.8125rem]')}>
        {value}
      </dd>
    </div>
  );
}
