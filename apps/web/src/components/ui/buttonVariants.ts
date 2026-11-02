import { cva, type VariantProps } from 'class-variance-authority';
import { cn } from './cn';

/**
 * Button styling, kept in a module with no `'use client'` directive so a Server Component
 * can style a `next/link` with the same class list a Client Component gets.
 *
 * Four variants and no more. There is no destructive red: the three status colours are
 * reserved for compliance verdicts, so "Record debarment order" is a `primary` button
 * with a confirmation step behind it, not a red one.
 */
export const buttonVariants = cva(
  [
    'inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-md',
    'border font-medium leading-none select-none',
    'transition-[background-color,border-color,color] duration-150',
    'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
    'disabled:pointer-events-none disabled:opacity-45',
  ].join(' '),
  {
    variants: {
      variant: {
        primary: 'border-accent bg-accent text-white hover:border-accent-dark hover:bg-accent-dark',
        default: 'border-line-strong bg-paper text-ink hover:bg-shell',
        quiet: 'border-transparent bg-transparent text-ink-muted hover:bg-shell hover:text-ink',
        link: 'h-auto border-transparent px-0 text-accent underline-offset-2 hover:underline',
      },
      size: {
        xs: 'h-7 px-2 text-2xs',
        sm: 'h-8 px-2.5 text-xs',
        md: 'h-9 px-3.5 text-sm',
      },
      block: { true: 'w-full', false: '' },
    },
    defaultVariants: { variant: 'default', size: 'sm', block: false },
  },
);

export type ButtonVariantProps = VariantProps<typeof buttonVariants>;

/** Style any element as a button: `<Link className={buttonClasses({ variant: 'primary' })}>`. */
export function buttonClasses(props: ButtonVariantProps & { className?: string } = {}): string {
  const { className, ...variants } = props;
  return cn(buttonVariants(variants), className);
}
