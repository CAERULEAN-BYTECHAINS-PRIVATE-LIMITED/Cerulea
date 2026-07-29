import type {
  HTMLAttributes,
  TableHTMLAttributes,
  TdHTMLAttributes,
  ThHTMLAttributes,
} from 'react';
import { cn } from './cn';

/**
 * One table style, used on every screen in the product.
 *
 * Column heads are 11px uppercase on the shell tint; cells are 13px with a hairline under
 * each row; anything numeric is right-aligned, monospaced and tabular so a column of
 * figures can be compared by eye. Nothing is striped — hairlines carry the row rhythm and
 * zebra banding on top of them is noise.
 *
 * `relative` on the scroll container is load-bearing, not decoration. `overflow-x: auto`
 * clips a descendant only when the scroll container is in that descendant's
 * containing-block chain, and an absolutely positioned child — every `sr-only` label is
 * one — resolves its containing block to the nearest *positioned* ancestor. With a static
 * container those labels lay out at their static position deep inside a wide table,
 * escaped the scroller entirely, and grew the document.
 */
export function Table({
  className,
  containerClassName,
  ...rest
}: TableHTMLAttributes<HTMLTableElement> & { containerClassName?: string }) {
  return (
    <div className={cn('scroll-x relative w-full', containerClassName)}>
      <table
        className={cn('w-full border-collapse text-left text-sm text-ink', className)}
        {...rest}
      />
    </div>
  );
}

export function THead({ className, ...rest }: HTMLAttributes<HTMLTableSectionElement>) {
  return <thead className={cn('bg-shell', className)} {...rest} />;
}

export function TBody({ className, ...rest }: HTMLAttributes<HTMLTableSectionElement>) {
  return <tbody className={cn('divide-y divide-line', className)} {...rest} />;
}

export function TR({ className, ...rest }: HTMLAttributes<HTMLTableRowElement>) {
  return <tr className={cn('transition-colors duration-100', className)} {...rest} />;
}

export function TH({
  className,
  scope = 'col',
  numeric = false,
  ...rest
}: ThHTMLAttributes<HTMLTableCellElement> & { numeric?: boolean }) {
  return (
    <th
      scope={scope}
      className={cn(
        'border-b border-line-strong px-3 py-1.5 align-bottom',
        'text-2xs font-semibold tracking-wide text-ink-muted uppercase',
        numeric && 'text-right',
        className,
      )}
      {...rest}
    />
  );
}

export function TD({
  className,
  mono = false,
  numeric = false,
  ...rest
}: TdHTMLAttributes<HTMLTableCellElement> & { mono?: boolean; numeric?: boolean }) {
  return (
    <td
      className={cn(
        'px-3 py-2 align-middle',
        mono && 'font-mono text-2xs',
        numeric && 'text-right font-mono tabular-nums',
        className,
      )}
      {...rest}
    />
  );
}

/** A muted em dash for a cell with nothing in it. Never a zero standing in for "unknown". */
export function Nil({ label = 'Not recorded' }: { label?: string }) {
  return (
    <span className="text-ink-subtle">
      <span aria-hidden="true">—</span>
      <span className="sr-only">{label}</span>
    </span>
  );
}
