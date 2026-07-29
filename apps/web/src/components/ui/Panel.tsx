import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from './cn';

/**
 * The one container in the product.
 *
 * A hairline box on paper with an optional grey title bar. No shadow, no rounded corners
 * beyond 3px, no nesting — a panel never contains another panel, because two stacked
 * borders is the visual signature of a page that has lost its structure.
 */
export function Panel({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('rounded-md border border-line bg-paper', className)} {...rest} />;
}

/**
 * A panel's title bar. `title` is a short noun phrase, never a sentence; `meta` is the
 * right-hand slot for a count, a chip or a control. There is no description slot on
 * purpose — explanatory prose belongs in a `PanelNote` beneath the content, at 11px,
 * where it does not compete with the data.
 */
export function PanelHead({
  title,
  meta,
  className,
  children,
  ...rest
}: Omit<HTMLAttributes<HTMLDivElement>, 'title'> & {
  title?: ReactNode;
  meta?: ReactNode;
}) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-center justify-between gap-x-4 gap-y-2',
        'rounded-t-md border-b border-line bg-shell px-3 py-2',
        className,
      )}
      {...rest}
    >
      {title && (
        <h2 className="text-xs font-semibold tracking-wide text-ink-2 uppercase">{title}</h2>
      )}
      {children}
      {meta && <div className="flex shrink-0 flex-wrap items-center gap-2">{meta}</div>}
    </div>
  );
}

export function PanelBody({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={cn('px-3 py-3', className)} {...rest} />;
}

export function PanelFoot({ className, ...rest }: HTMLAttributes<HTMLDivElement>) {
  return (
    <div
      className={cn(
        'flex flex-wrap items-center justify-between gap-x-4 gap-y-2',
        'rounded-b-md border-t border-line bg-shell px-3 py-2',
        className,
      )}
      {...rest}
    />
  );
}

/**
 * The provenance line under a table or a figure: where the number came from, what it does
 * not cover, what would change it. Set at 11px so it is available without competing.
 */
export function PanelNote({ className, ...rest }: HTMLAttributes<HTMLParagraphElement>) {
  return (
    <p
      className={cn('border-t border-line px-3 py-2 text-2xs text-ink-muted', className)}
      {...rest}
    />
  );
}

/**
 * A page-level section heading. One line, no paragraph beneath it.
 * `meta` holds whatever the reader needs to qualify the heading — a count, a scope chip.
 */
export function SectionHead({
  title,
  meta,
  id,
  className,
}: {
  title: ReactNode;
  meta?: ReactNode;
  id?: string;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-wrap items-center justify-between gap-x-4 gap-y-1.5', className)}>
      <h2 id={id} className="text-lg font-semibold tracking-tight text-ink">
        {title}
      </h2>
      {meta && <div className="flex flex-wrap items-center gap-2">{meta}</div>}
    </div>
  );
}
