'use client';

import { motion, useReducedMotion } from 'framer-motion';
import { useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * The branded wait state (spec Part 9.5): "a stalled finality wait should show a calm,
 * branded spinner with the elapsed time, not a blank screen".
 *
 * It counts up in tenths of a second so the number is visibly alive even when the chain is
 * slow, and it names the four steps of the Part 8.3 finality wait so a judge watching the
 * screen knows what is being waited on rather than just that something is loading.
 *
 * The spinner is brand blue, never a status colour — nothing here is a verdict yet.
 */
const STEPS = [
  'Signing and submitting the extrinsic',
  'Waiting for inclusion in a block',
  'Waiting for a finalized head at or past that block',
  'Reading the event back',
] as const;

export function FinalityPending({
  label = 'Waiting for finality',
  /** Seconds after which the wait is called out as unusually long. Part 8.3 budgets 10 s. */
  slowAfterSeconds = 4,
  timeoutSeconds = 10,
  showSteps = true,
  className,
}: {
  label?: string;
  slowAfterSeconds?: number;
  timeoutSeconds?: number;
  showSteps?: boolean;
  className?: string;
}) {
  const [elapsedMs, setElapsedMs] = useState(0);
  const reduceMotion = useReducedMotion();

  useEffect(() => {
    const startedAt = Date.now();
    const timer = setInterval(() => setElapsedMs(Date.now() - startedAt), 100);
    return () => clearInterval(timer);
  }, []);

  const seconds = elapsedMs / 1000;
  const isSlow = seconds >= slowAfterSeconds;
  const activeStep = Math.min(STEPS.length - 1, Math.floor(seconds / 1.5));

  return (
    <div
      role="status"
      aria-live="polite"
      className={cn(
        'flex flex-col items-center justify-center rounded-card border border-border bg-surface px-6 py-10 text-center',
        className,
      )}
    >
      <div className="relative flex size-14 items-center justify-center">
        <span className="absolute inset-0 rounded-full border-2 border-cerulea-light" />
        <motion.span
          className="absolute inset-0 rounded-full border-2 border-transparent border-t-cerulea"
          animate={reduceMotion ? undefined : { rotate: 360 }}
          transition={{ duration: 0.9, ease: 'linear', repeat: Infinity }}
        />
        <span className="font-mono text-sm font-semibold text-cerulea-dark tabular-nums">
          {seconds.toFixed(1)}
        </span>
      </div>

      <p className="mt-4 text-sm font-semibold text-ink">{label}</p>
      <p className="mt-1 text-sm text-ink-muted">
        A decision is only a decision once its block is finalized. Nothing is shown until it
        is.
      </p>

      <p className="mt-2 font-mono text-xs text-ink-muted tabular-nums">
        <span className="sr-only">Elapsed </span>
        {seconds.toFixed(1)}s elapsed · {timeoutSeconds}s budget
      </p>

      {showSteps && (
        <ol className="mt-5 w-full max-w-sm space-y-1.5 text-left">
          {STEPS.map((step, index) => {
            const done = index < activeStep;
            const current = index === activeStep;
            return (
              <li
                key={step}
                className={cn(
                  'flex items-start gap-2 text-xs transition-colors duration-200',
                  done && 'text-ink-muted',
                  current && 'font-medium text-ink',
                  !done && !current && 'text-ink-subtle',
                )}
              >
                <span
                  className={cn(
                    'mt-1 size-1.5 shrink-0 rounded-full',
                    done && 'bg-ink-subtle',
                    current && 'bg-cerulea',
                    !done && !current && 'bg-border',
                  )}
                  aria-hidden="true"
                />
                {step}
              </li>
            );
          })}
        </ol>
      )}

      {isSlow && (
        <p className="mt-5 max-w-sm text-xs text-ink-muted">
          This block is taking longer than usual to finalize. The request will return a clear
          timeout after {timeoutSeconds} seconds rather than hang.
        </p>
      )}
    </div>
  );
}
