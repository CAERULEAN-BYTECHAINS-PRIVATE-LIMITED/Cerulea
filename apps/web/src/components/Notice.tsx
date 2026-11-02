'use client';

import { CircleAlert, Clock, PlugZap, RotateCw } from 'lucide-react';
import { useState, type ReactNode } from 'react';
import { Button } from './ui/Button';
import { cn } from './ui/cn';

export type ErrorKind = 'finality-timeout' | 'chain-unreachable' | 'rejected' | 'unknown';

/**
 * An operational failure — the request did not produce a verdict.
 *
 * This is NOT a RED result and must never look like one. RED means the chain answered and
 * the answer was "blocked"; this means the chain did not answer at all. The status
 * colours are reserved for verdicts, so a failure here is rendered entirely in ink.
 *
 * It names what failed, what that means for the transaction, and offers the one action
 * worth taking. Nothing more.
 */
const PRESETS: Record<ErrorKind, { icon: typeof CircleAlert; title: string; detail: string }> = {
  'finality-timeout': {
    icon: Clock,
    title: 'The block did not finalize in time',
    detail:
      'The transaction was included in a block, but no finalized head reached it within the 10-second budget. It may still finalize; nothing has been decided either way.',
  },
  'chain-unreachable': {
    icon: PlugZap,
    title: 'Cannot reach the network',
    detail:
      'No connection to a Cerulea validator could be opened. No transaction was submitted, so nothing has changed on chain.',
  },
  rejected: {
    icon: CircleAlert,
    title: 'The transaction was rejected',
    detail:
      'The runtime refused the call before it could be applied. This is a submission failure, not a compliance verdict.',
  },
  unknown: {
    icon: CircleAlert,
    title: 'The request did not complete',
    detail: 'No verdict was recorded, so this can safely be retried.',
  },
};

export function Notice({
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
  detail?: ReactNode;
  technicalDetail?: string;
  onRetry?: () => void;
  retryLabel?: string;
  action?: ReactNode;
  className?: string;
}) {
  const preset = PRESETS[kind];
  const Icon = preset.icon;
  const [showTechnical, setShowTechnical] = useState(false);

  return (
    <div
      role="alert"
      className={cn('rounded-md border border-line-strong bg-paper', className)}
    >
      <div className="flex items-center gap-2 border-b border-line bg-shell px-3 py-1.5">
        <Icon className="size-3.5 shrink-0 text-ink-muted" aria-hidden="true" />
        <p className="text-2xs font-semibold tracking-wide text-ink-2 uppercase">
          No verdict recorded
        </p>
      </div>

      <div className="px-3 py-3">
        <p className="text-sm font-semibold text-ink">{title ?? preset.title}</p>
        <p className="mt-1 max-w-[75ch] text-xs leading-relaxed text-ink-muted">
          {detail ?? preset.detail}
        </p>

        <div className="mt-3 flex flex-wrap items-center gap-2">
          {onRetry && (
            <Button
              variant="primary"
              onClick={onRetry}
              icon={<RotateCw className="size-3.5" aria-hidden="true" />}
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
              className="rounded-sm text-2xs font-medium text-accent underline-offset-2 transition-colors duration-150 hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
            >
              {showTechnical ? 'Hide detail' : 'Technical detail'}
            </button>
          )}
        </div>

        {technicalDetail && showTechnical && (
          <pre className="scroll-x mt-2 rounded-sm border border-line bg-shell px-2.5 py-2 font-mono text-2xs leading-relaxed text-ink-muted">
            {technicalDetail}
          </pre>
        )}
      </div>
    </div>
  );
}

/**
 * A one-line inline banner. Used where a poll failed but the last good data is still on
 * screen — never blank a projector because one refresh timed out.
 */
export function StaleBanner({ message }: { message: string }) {
  return (
    <p className="rounded-md border border-line bg-shell px-3 py-1.5 text-2xs text-ink-muted">
      <span className="font-medium text-ink">Showing the last successful read. </span>
      {message}
    </p>
  );
}
