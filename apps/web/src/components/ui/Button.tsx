'use client';

import { LoaderCircle } from 'lucide-react';
import type { ButtonHTMLAttributes, ReactNode } from 'react';
import { buttonClasses, type ButtonVariantProps } from './buttonVariants';

export interface ButtonProps
  extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'color'>,
    ButtonVariantProps {
  /** Renders a spinner, disables the control and marks it `aria-busy`. */
  loading?: boolean;
  /** Announced while `loading`, so the state is not signalled by the spinner alone. */
  loadingLabel?: string;
  icon?: ReactNode;
  iconAfter?: ReactNode;
}

export function Button({
  className,
  variant,
  size,
  block,
  loading = false,
  loadingLabel = 'Working',
  icon,
  iconAfter,
  children,
  disabled,
  type = 'button',
  ...rest
}: ButtonProps) {
  return (
    <button
      type={type}
      className={buttonClasses({ variant, size, block, className })}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      {...rest}
    >
      {loading ? (
        <LoaderCircle className="size-3.5 shrink-0 animate-spin" aria-hidden="true" />
      ) : (
        icon
      )}
      <span>{children}</span>
      {loading ? <span className="sr-only">{loadingLabel}</span> : iconAfter}
    </button>
  );
}
