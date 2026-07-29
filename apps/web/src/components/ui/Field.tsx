'use client';

import type {
  InputHTMLAttributes,
  ReactNode,
  SelectHTMLAttributes,
  TextareaHTMLAttributes,
} from 'react';
import { useId } from 'react';
import { cn } from './cn';

const controlClasses = [
  'w-full rounded-lg border border-border bg-surface px-3 text-sm text-ink',
  'placeholder:text-ink-subtle',
  'transition-[border-color,box-shadow] duration-150 ease-out',
  'focus:border-cerulea focus:outline-2 focus:-outline-offset-1 focus:outline-cerulea',
  'disabled:cursor-not-allowed disabled:bg-surface-sunken disabled:text-ink-muted',
  // Not `status-red`. The three status colours are reserved for compliance verdicts
  // (docs/PRAMAAN_BUILD_CONTRACT.md section 5); a badly formatted field is not a verdict.
  'aria-[invalid=true]:border-form-error',
].join(' ');

/**
 * A labelled form control.
 *
 * The label, hint and error are wired to the control by generated ids, so a screen reader
 * announces all three together. Errors are never colour-only: the message is text, prefixed
 * with "Error:", and the control carries `aria-invalid`.
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
  hint?: string;
  error?: string;
  required?: boolean;
  /** Receives `id`, `aria-describedby` and `aria-invalid`. */
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
    <div className={cn('flex flex-col gap-1.5', className)}>
      <label htmlFor={id} className="text-sm font-medium text-ink">
        {label}
        {required && (
          <span className="ml-1 text-ink-muted" aria-hidden="true">
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
        <p id={hintId} className="text-xs text-ink-muted">
          {hint}
        </p>
      )}
      {error && (
        <p id={errorId} className="text-xs font-medium text-form-error">
          <span className="sr-only">Error: </span>
          {error}
        </p>
      )}
    </div>
  );
}

export function Input({ className, ...rest }: InputHTMLAttributes<HTMLInputElement>) {
  return <input className={cn(controlClasses, 'h-10', className)} {...rest} />;
}

export function Textarea({ className, ...rest }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className={cn(controlClasses, 'min-h-24 py-2', className)} {...rest} />;
}

export function Select({ className, ...rest }: SelectHTMLAttributes<HTMLSelectElement>) {
  return (
    <select
      className={cn(controlClasses, 'h-10 appearance-none bg-right pr-9', className)}
      style={{
        backgroundImage:
          "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='16' height='16' viewBox='0 0 24 24' fill='none' stroke='%235f6368' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E\")",
        backgroundRepeat: 'no-repeat',
        backgroundPosition: 'right 0.65rem center',
      }}
      {...rest}
    />
  );
}
