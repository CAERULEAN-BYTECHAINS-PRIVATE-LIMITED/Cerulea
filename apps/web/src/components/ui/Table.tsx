import type {
  HTMLAttributes,
  TableHTMLAttributes,
  TdHTMLAttributes,
  ThHTMLAttributes,
} from 'react';
import { cn } from './cn';

/**
 * Tables scroll horizontally inside their own container so the page body never does —
 * the demo runs from projector width down to tablet (spec Part 9.8).
 *
 * `relative` is load-bearing, not decoration. `overflow-x: auto` clips a descendant only
 * when the scroll container is in that descendant's containing-block chain, and an
 * absolutely positioned child — every `sr-only` label is one — resolves its containing
 * block to the nearest *positioned* ancestor. With a static container those labels laid
 * out at their static position deep inside a wide table, escaped the scroller entirely and
 * grew the document: `/auditor` measured `documentElement.scrollWidth` 825 against a
 * `clientWidth` of 753 at 768px, from the single `sr-only` "Certify" header cell.
 */
export function Table({
  className,
  containerClassName,
  ...rest
}: TableHTMLAttributes<HTMLTableElement> & { containerClassName?: string }) {
  return (
    <div className={cn('relative w-full overflow-x-auto', containerClassName)}>
      <table className={cn('w-full border-collapse text-left text-sm', className)} {...rest} />
    </div>
  );
}

export function THead({ className, ...rest }: HTMLAttributes<HTMLTableSectionElement>) {
  return <thead className={cn('bg-surface-sunken', className)} {...rest} />;
}

export function TBody({ className, ...rest }: HTMLAttributes<HTMLTableSectionElement>) {
  return <tbody className={cn('divide-y divide-border', className)} {...rest} />;
}

export function TR({ className, ...rest }: HTMLAttributes<HTMLTableRowElement>) {
  return <tr className={cn('transition-colors duration-150', className)} {...rest} />;
}

export function TH({ className, scope = 'col', ...rest }: ThHTMLAttributes<HTMLTableCellElement>) {
  return (
    <th
      scope={scope}
      className={cn(
        'border-b border-border px-4 py-2.5 text-xs font-semibold tracking-wide text-ink-muted uppercase',
        className,
      )}
      {...rest}
    />
  );
}

export function TD({
  className,
  mono = false,
  ...rest
}: TdHTMLAttributes<HTMLTableCellElement> & { mono?: boolean }) {
  return (
    <td
      className={cn(
        'px-4 py-3 align-middle text-ink',
        mono && 'font-mono text-[0.8125rem] text-ink-muted',
        className,
      )}
      {...rest}
    />
  );
}

export function TCaption({ className, ...rest }: HTMLAttributes<HTMLTableCaptionElement>) {
  return (
    <caption
      className={cn('caption-bottom px-4 py-3 text-left text-xs text-ink-muted', className)}
      {...rest}
    />
  );
}
