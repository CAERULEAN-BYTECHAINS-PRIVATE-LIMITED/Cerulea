import { cva, type VariantProps } from 'class-variance-authority';
import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from './cn';

/**
 * A rectangular status chip. Deliberately not a pill: a 999px radius reads as a consumer
 * product, and this sits in table cells beside transcribed identifiers.
 *
 * `green` / `yellow` / `red` are RESERVED tones. Use them only where the chip restates a
 * compliance verdict under the PPP-MII Order — a classification outcome, a debarment
 * state, a certification gate. Counts, categories, ministry tags, pathway ids, connection
 * state and severity all use `neutral`, `accent` or `strong`.
 */
const chip = cva(
  'inline-flex items-center gap-1 rounded-sm border font-medium whitespace-nowrap',
  {
    variants: {
      tone: {
        neutral: 'border-line bg-shell text-ink-muted',
        accent: 'border-accent-line bg-accent-tint text-accent-dark',
        strong: 'border-ink-2 bg-ink-2 text-white',
        outline: 'border-line-strong bg-paper text-ink',
        green: 'border-status-green/30 bg-status-green-bg text-status-green-ink',
        yellow: 'border-status-yellow/40 bg-status-yellow-bg text-status-yellow-ink',
        red: 'border-status-red/30 bg-status-red-bg text-status-red-ink',
      },
      size: {
        sm: 'px-1.5 py-px text-2xs',
        md: 'px-2 py-0.5 text-xs',
      },
    },
    defaultVariants: { tone: 'neutral', size: 'sm' },
  },
);

export interface ChipProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, 'color'>,
    VariantProps<typeof chip> {
  icon?: ReactNode;
}

export function Chip({ className, tone, size, icon, children, ...rest }: ChipProps) {
  return (
    <span className={cn(chip({ tone, size }), className)} {...rest}>
      {icon}
      {children}
    </span>
  );
}

/**
 * The tri-state token itself — GREEN / YELLOW / RED — set solid in the reserved colour so
 * the word on screen and the word in the API response are visibly the same value.
 *
 * Yellow takes ink rather than white text: #E8A100 carries 7.5:1 against ink and 2.4:1
 * against white, so white here would be unreadable on a washed-out projector.
 */
export function StatusToken({
  status,
  className,
}: {
  status: 'GREEN' | 'YELLOW' | 'RED';
  className?: string;
}) {
  return (
    <span
      className={cn(
        'inline-flex rounded-sm px-1.5 py-px font-mono text-2xs font-semibold tracking-widest',
        status === 'GREEN' && 'bg-status-green text-white',
        status === 'YELLOW' && 'bg-status-yellow text-ink',
        status === 'RED' && 'bg-status-red text-white',
        className,
      )}
    >
      {status}
    </span>
  );
}
