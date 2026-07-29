import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from './cn';

/**
 * Button styling, kept in its own module with no `'use client'` directive.
 *
 * `<Button>` itself is a Client Component (it takes event handlers), but a Server
 * Component still needs to style a `next/link` as a button — the landing page's two calls
 * to action are exactly that. A function exported from a client module cannot be called on
 * the server, so the class list lives here and `Button.tsx` imports it.
 *
 * Note the absence of a red or green variant. The three status colours are reserved for a
 * compliance verdict (docs/PRAMAAN_BUILD_CONTRACT section 5); a destructive action such as
 * "Debar vendor" uses the neutral `secondary` variant with a warning icon instead.
 */
export const buttonVariants = cva(
  [
    'inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-lg',
    'font-medium leading-none select-none',
    'transition-[background-color,border-color,color,box-shadow] duration-150 ease-out',
    'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
    'disabled:pointer-events-none disabled:opacity-50',
  ].join(' '),
  {
    variants: {
      variant: {
        primary: 'bg-cerulea text-white shadow-sm hover:bg-cerulea-dark active:bg-cerulea-dark',
        secondary:
          'bg-surface text-ink border border-border shadow-sm hover:bg-surface-sunken hover:border-ink-subtle',
        subtle: 'bg-cerulea-light text-cerulea-dark hover:bg-cerulea-light/70',
        ghost: 'text-ink-muted hover:bg-surface-sunken hover:text-ink',
        link: 'text-cerulea underline-offset-4 hover:underline px-0 h-auto',
      },
      size: {
        sm: 'h-8 px-3 text-[0.8125rem]',
        md: 'h-10 px-4 text-sm',
        lg: 'h-12 px-6 text-base',
      },
      block: {
        true: 'w-full',
        false: '',
      },
    },
    defaultVariants: { variant: 'primary', size: 'md', block: false },
  },
);

export type ButtonVariantProps = VariantProps<typeof buttonVariants>;

/**
 * Style any element as a button.
 * `<Link href="/demo" className={buttonClasses({ variant: 'primary', size: 'lg' })}>`
 */
export function buttonClasses(props: ButtonVariantProps & { className?: string } = {}): string {
  const { className, ...variants } = props;
  return cn(buttonVariants(variants), className);
}
