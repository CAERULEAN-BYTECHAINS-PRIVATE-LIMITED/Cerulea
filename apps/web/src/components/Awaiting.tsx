'use client';

import { useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * The finality wait (spec Part 9.5): "a stalled finality wait should show a calm, branded
 * spinner with the elapsed time, not a blank screen".
 *
 * It counts in tenths of a second so the figure is visibly alive on a slow chain, and it
 * names the four steps of the Part 8.3 wait so a reader knows what is being waited on
 * rather than only that something is loading.
 *
 * Rendered entirely in ink and accent. Nothing here is a verdict yet, so no status colour
 * appears — not even to say the wait is going well.
 */
const STEPS = [
  'Signing and submitting the extrinsic',
  'Waiting for inclusion in a block',
  'Waiting for a finalized head at or past that block',
  'Reading the event back',
] as const;

export function Awaiting({
  label = 'Waiting for finality',
  /** Seconds after which the wait is called out as unusually long. Part 8.3 budgets 10 s. */
  slowAfterSeconds = 4,
  timeoutSeconds = 10,
  steps = true,
  className,
}: {
  label?: string;
  slowAfterSeconds?: number;
  timeoutSeconds?: number;
  steps?: boolean;
  className?: string;
}) {
  const [elapsedMs, setElapsedMs] = useState(0);

  useEffect(() => {
    const startedAt = Date.now();
    const timer = setInterval(() => setElapsedMs(Date.now() - startedAt), 100);
    return () => clearInterval(timer);
  }, []);

  const seconds = elapsedMs / 1000;
  const isSlow = seconds >= slowAfterSeconds;
  const activeStep = Math.min(STEPS.length - 1, Math.floor(seconds / 1.5));
  const progress = Math.min(1, seconds / timeoutSeconds);

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn('rounded-md border border-line bg-paper', className)}
    >
      <div className="flex flex-wrap items-baseline justify-between gap-x-4 gap-y-1 border-b border-line bg-shell px-3 py-1.5">
        <p className="text-2xs font-semibold tracking-wide text-ink-2 uppercase">{label}</p>
        <p className="font-mono text-2xs text-ink tabular-nums">
          <span className="sr-only">Elapsed </span>
          {seconds.toFixed(1)}s
          <span className="text-ink-muted"> / {timeoutSeconds}s budget</span>
        </p>
      </div>

      {/* A determinate bar against the stated budget, not an indeterminate spinner: the
          wait has a known ceiling and the reader is entitled to see where in it they are. */}
      <div className="h-0.5 w-full bg-shell-2">
        <div
          className="h-full bg-accent transition-[width] duration-100 ease-linear"
          style={{ width: `${progress * 100}%` }}
        />
      </div>

      <div className="px-3 py-2.5">
        {steps ? (
          <ol className="space-y-1">
            {STEPS.map((step, index) => {
              const done = index < activeStep;
              const current = index === activeStep;
              return (
                <li
                  key={step}
                  className={cn(
                    'flex items-baseline gap-2 text-2xs transition-colors duration-200',
                    current ? 'font-medium text-ink' : done ? 'text-ink-muted' : 'text-ink-subtle',
                  )}
                >
                  <span
                    aria-hidden="true"
                    className={cn(
                      'inline-block size-1.5 shrink-0 translate-y-px rounded-full',
                      current ? 'bg-accent' : done ? 'bg-ink-subtle' : 'bg-line-strong',
                    )}
                  />
                  {step}
                </li>
              );
            })}
          </ol>
        ) : (
          <p className="text-2xs text-ink-muted">
            A decision is only a decision once its block is finalized.
          </p>
        )}

        {isSlow && (
          <p className="mt-2 text-2xs text-ink-muted">
            This block is taking longer than usual. The request returns a clear timeout after{' '}
            {timeoutSeconds} seconds rather than hang.
          </p>
        )}
      </div>
    </div>
  );
}
