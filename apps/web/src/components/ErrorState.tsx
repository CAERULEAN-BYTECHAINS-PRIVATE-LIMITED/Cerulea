'use client';

import { motion, useReducedMotion } from 'framer-motion';
import { CircleAlert, Clock, PlugZap, RotateCw } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { Button } from './ui/Button';
import { cn } from './ui/cn';

export type ErrorKind = 'finality-timeout' | 'chain-unreachable' | 'rejected' | 'unknown';

/**
 * An operational failure — the request did not produce a verdict.
 *
 * This is *not* a RED result and must never look like one. RED means the chain answered
 * and the answer was "blocked"; this means the chain did not answer. The status colours
 * are reserved for verdicts, so a failure here is rendered in ink and brand blue.
 *
 * Built with the same care as the success state (spec Part 9.5): it names what failed,
 * what it means, what happens to the transaction, and offers the one action worth taking.
 */
const PRESETS: Record<
  ErrorKind,
  { icon: typeof CircleAlert; title: string; detail: string }
> = {
  'finality-timeout': {
    icon: Clock,
    title: 'The block did not finalize in time',
    detail:
      'The transaction was included in a block, but no finalized head reached that block within the 10-second budget. It may still finalize; nothing has been decided either way.',
  },
  'chain-unreachable': {
    icon: PlugZap,
    title: 'Cannot reach the network',
    detail:
      'The console could not open a connection to a Cerulea validator. No transaction was submitted, so nothing has changed on chain.',
  },
  rejected: {
    icon: CircleAlert,
    title: 'The transaction was rejected',
    detail:
      'The runtime refused the call before it could be applied. This is a submission failure, not a compliance verdict.',
  },
  unknown: {
    icon: CircleAlert,
    title: 'Something went wrong',
    detail:
      'The request did not complete. No verdict was recorded, so this can safely be retried.',
  },
};

export function ErrorState({
  kind = 'unknown',
  title,
  detail,
  /** The raw message from the API or the chain, shown behind a disclosure. */
  technicalDetail,
  onRetry,
  retryLabel = 'Try again',
  action,
  className,
}: {
  kind?: ErrorKind;
  title?: string;
  detail?: string;
  technicalDetail?: string;
  onRetry?: () => void;
  retryLabel?: string;
  action?: ReactNode;
  className?: string;
}) {
  const preset = PRESETS[kind];
  const Icon = preset.icon;
  const [showTechnical, setShowTechnical] = useState(false);
  const reduceMotion = useReducedMotion();

  return (
    <motion.div
      role="alert"
      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
      className={cn(
        'w-full rounded-card border border-border bg-surface px-5 py-5 sm:px-6',
        className,
      )}
    >
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:gap-5">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-surface-sunken text-ink-muted">
          <Icon className="size-5" aria-hidden="true" />
        </div>

        <div className="min-w-0 flex-1">
          <p className="text-xs font-semibold tracking-wide text-ink-subtle uppercase">
            No verdict recorded
          </p>
          <h2 className="mt-1 text-lg font-semibold text-ink">{title ?? preset.title}</h2>
          <p className="mt-1.5 max-w-2xl text-sm leading-relaxed text-ink-muted">
            {detail ?? preset.detail}
          </p>

          <div className="mt-4 flex flex-wrap items-center gap-2">
            {onRetry && (
              <Button
                size="sm"
                variant="primary"
                onClick={onRetry}
                leadingIcon={<RotateCw className="size-4" aria-hidden="true" />}
              >
                {retryLabel}
              </Button>
            )}
            {action}
            {technicalDetail && (
              <button
                type="button"
                onClick={() => setShowTechnical((value) => !value)}
                aria-expanded={showTechnical}
                className="rounded px-1 text-sm font-medium text-ink-muted underline-offset-4 transition-colors duration-150 hover:text-ink hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
              >
                {showTechnical ? 'Hide technical detail' : 'Show technical detail'}
              </button>
            )}
          </div>

          {technicalDetail && showTechnical && (
            <pre className="mt-3 overflow-x-auto rounded-lg border border-border bg-surface-sunken px-3 py-2.5 font-mono text-xs leading-relaxed text-ink-muted">
              {technicalDetail}
            </pre>
          )}
        </div>
      </div>
    </motion.div>
  );
}
