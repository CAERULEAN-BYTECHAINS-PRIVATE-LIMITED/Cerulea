import { cva, type VariantProps } from 'class-variance-authority';
import type { HTMLAttributes, ReactNode } from 'react';
import { cn } from './cn';

/**
 * `green` / `yellow` / `red` are RESERVED tones.
 *
 * Use them only where the badge restates a compliance verdict under the PPP-MII Order
 * (a classification outcome, a debarment state, a certification gate). Everything else —
 * counts, categories, ministry tags, pathway ids — must use `neutral`, `brand` or `teal`.
 */
const badgeVariants = cva(
  'inline-flex items-center gap-1.5 rounded-full border font-medium whitespace-nowrap',
  {
    variants: {
      tone: {
        neutral: 'border-border bg-surface-sunken text-ink-muted',
        brand: 'border-cerulea/15 bg-cerulea-light text-cerulea-dark',
        teal: 'border-teal/20 bg-teal/8 text-teal',
        outline: 'border-border bg-surface text-ink',
        green: 'border-status-green/20 bg-status-green-bg text-status-green',
        yellow: 'border-status-yellow/25 bg-status-yellow-bg text-[#8a6100]',
        red: 'border-status-red/20 bg-status-red-bg text-status-red',
      },
      size: {
        sm: 'px-2 py-0.5 text-[0.6875rem]',
        md: 'px-2.5 py-1 text-xs',
      },
    },
    defaultVariants: { tone: 'neutral', size: 'md' },
  },
);

export interface BadgeProps
  extends Omit<HTMLAttributes<HTMLSpanElement>, 'color'>,
    VariantProps<typeof badgeVariants> {
  icon?: ReactNode;
}

export function Badge({ className, tone, size, icon, children, ...rest }: BadgeProps) {
  return (
    <span className={cn(badgeVariants({ tone, size }), className)} {...rest}>
      {icon}
      {children}
    </span>
  );
}
