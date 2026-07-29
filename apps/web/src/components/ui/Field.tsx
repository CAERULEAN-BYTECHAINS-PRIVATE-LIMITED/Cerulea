'use client';

import type {
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from 'react';
import { useId } from 'react';
import { cn } from './cn';

/**
 * One form-control vocabulary. Every control in the product is 32px tall, 13px, hairline
 * bordered, 3px radius, and takes the same focus ring.
 *
 * Invalid controls take `--form-error`, a deep plum, and NOT the reserved red. A badly
 * formatted HSN code is a typo; #D93025 in this application legally means "blocked under
 * the PPP-MII Order", and using it here would teach the wrong association within seconds.
 */
const control = [
  'w-full rounded-md border border-line-strong bg-paper px-2 text-sm text-ink',
  'placeholder:text-ink-muted',
  'transition-[border-color] duration-150',
  'focus:border-accent focus:outline-2 focus:-outline-offset-1 focus:outline-accent',
  'disabled:cursor-not-allowed disabled:bg-shell disabled:text-ink-muted',
  'aria-[invalid=true]:border-form-error aria-[invalid=true]:bg-form-error-bg',
].join(' ');

/**
 * A labelled control. Label, hint and error are wired to the control by generated ids so
 * a screen reader announces all three together, and an error is never colour-only: the
 * message is text and the control carries `aria-invalid`.
 */
export function Field({
  label,
  hint,
  error,
  required,
  children,
  className,
}: {
  label: string;
  hint?: ReactNode;
  error?: string;
  required?: boolean;
  /** Receives `id`, `aria-describedby`, `aria-invalid` and `required`. */
  children: (props: {
    id: string;
    'aria-describedby': string | undefined;
    'aria-invalid': boolean | undefined;
    required: boolean | undefined;
  }) => ReactNode;
  className?: string;
}) {
  const id = useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const describedBy = [hintId, errorId].filter(Boolean).join(' ') || undefined;

  return (
    <div className={cn('flex min-w-0 flex-col gap-1', className)}>
      <label htmlFor={id} className="text-xs font-semibold text-ink-2">
        {label}
        {required && (
          <span className="ml-0.5 text-form-error" aria-hidden="true">
            *
          </span>
        )}
        {required && <span className="sr-only"> (required)</span>}
      </label>

      {children({
        id,
        'aria-describedby': describedBy,
        'aria-invalid': error ? true : undefined,
        required,
      })}

      {hint && !error && (
        <p id={hintId} className="text-2xs text-ink-muted">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="text-2xs font-medium text-form-error">
          <span className="sr-only">Error: </span>
          {error}
        </p>
      )}
    </div>
  );
}

export function Input({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cn(control, 'h-8', className)} {...rest} />;
}

export function Textarea({ className, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cn(control, 'min-h-20 py-1.5 leading-relaxed', className)} {...rest} />;
}

export function Select({ className, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={cn(control, 'h-8 appearance-none pr-7', className)}
      style={{
        backgroundImage:
          "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='14' height='14' viewBox='0 0 24 24' fill='none' stroke='%2355606e' stroke-width='2.5' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E\")",
        backgroundRepeat: 'no-repeat',
        backgroundPosition: 'right 0.5rem center',
      }}
      {...rest}
    />
  );
}

/** A checkbox with its label, sized and spaced like every other control on the form. */
export function Check({
  checked,
  onChange,
  label,
  hint,
  disabled,
  className,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: ReactNode;
  hint?: ReactNode;
  disabled?: boolean;
  className?: string;
}) {
  return (
    <label
      className={cn(
        'flex cursor-pointer items-start gap-2 rounded-md border border-line bg-paper px-2.5 py-2',
        'transition-colors duration-150 hover:bg-shell',
        disabled && 'cursor-not-allowed opacity-55 hover:bg-paper',
        className,
      )}
    >
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
        className="mt-0.5 size-3.5 shrink-0 accent-[#1b4a8f]"
      />
      <span className="min-w-0">
        <span className="block text-xs font-medium text-ink">{label}</span>
        {hint && <span className="mt-0.5 block text-2xs text-ink-muted">{hint}</span>}
      </span>
    </label>
  );
}
