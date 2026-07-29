'use client';

import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import {
  ChevronDown,
  CircleCheckBig,
  CircleX,
  ExternalLink,
  Gavel,
  TriangleAlert,
  type LucideIcon,
} from 'lucide-react';
import Link from 'next/link';
import { useId, useState, type ReactNode } from 'react';
import { PathwayBadge, type PathwayId } from './PathwayBadge';
import { TxRef } from './TxRef';
import { cn } from './ui/cn';

/**
 * The tri-state every trigger-point route returns (docs/PRAMAAN_BUILD_CONTRACT section 3).
 */
export type ComplianceStatus = 'GREEN' | 'YELLOW' | 'RED';

/**
 * For a RED result: the specific thing that caused the block. The spec is explicit about
 * why this exists — "since a judge will ask 'why did that fail' live".
 */
export interface ComplianceBlocker {
  kind: 'rule' | 'debarment' | 'eligibility';
  /** e.g. "MeitY local content threshold, HSN 8471" or "Debarred by MoD". */
  label: string;
  /** The measured fact against the required one. */
  detail?: string;
  /** The order, rule or paragraph the block rests on. */
  citation?: string;
}

export interface ComplianceResultProps {
  status: ComplianceStatus;
  /** One plain sentence a non-technical judge can read aloud. */
  reason: string;
  /** What produced this verdict, e.g. "Bid evaluation — GEM/2025/B/6798497". */
  trigger?: string;
  /** Transaction hash returned by the trigger-point route. */
  txRef?: string;
  /** Number of the finalized block carrying the transaction. */
  blockNumber?: number;
  blockHash?: string;
  /** Which of P1–P12 the decision took. */
  pathway?: PathwayId;
  /** RED only. Rendered above the on-chain record, before anyone has to expand anything. */
  blockedBy?: ComplianceBlocker;
  /** Submission-to-finality time, in milliseconds. */
  latencyMs?: number;
  /** Overrides the default `/explorer?tx=…` link. */
  explorerHref?: string;
  /** Extra facts for the expanded record, e.g. declared local content. */
  records?: { label: string; value: ReactNode; mono?: boolean }[];
  className?: string;
}

interface StatusPresentation {
  /** The large word. Never the only signal — always paired with `Icon`. */
  word: string;
  Icon: LucideIcon;
  /** How assistive technology should hear it, since colour conveys nothing there. */
  announce: string;
  live: 'polite' | 'assertive';
  role: 'status' | 'alert';
  banner: string;
  accent: string;
  text: string;
  chip: string;
}

/**
 * Colour is never the signal on its own.
 *
 * Each verdict is carried by three independent channels: a distinct ICON shape (tick /
 * triangle / cross), the STATUS WORD in 30px type, and only then the reserved colour. A
 * judge with deuteranopia, a projector with a washed-out gamut, and a screen-reader user
 * all get the same verdict.
 */
const PRESENTATION: Record<ComplianceStatus, StatusPresentation> = {
  GREEN: {
    word: 'Compliant',
    Icon: CircleCheckBig,
    announce: 'Compliant. Proceeds.',
    live: 'polite',
    role: 'status',
    banner: 'bg-status-green-bg',
    accent: 'bg-status-green',
    text: 'text-status-green',
    chip: 'bg-status-green text-white',
  },
  YELLOW: {
    word: 'Review required',
    Icon: TriangleAlert,
    announce: 'Review required. Proceeds with a caveat.',
    live: 'polite',
    role: 'status',
    banner: 'bg-status-yellow-bg',
    accent: 'bg-status-yellow',
    // #E8A100 on its own tint fails contrast at body size, so the deeper shade of the same
    // hue carries the text while the reserved colour carries the rule and the chip.
    text: 'text-[#8a6100]',
    chip: 'bg-status-yellow text-ink',
  },
  RED: {
    word: 'Blocked',
    Icon: CircleX,
    announce: 'Blocked. This bid cannot proceed.',
    live: 'assertive',
    role: 'alert',
    banner: 'bg-status-red-bg',
    accent: 'bg-status-red',
    text: 'text-status-red',
    chip: 'bg-status-red text-white',
  },
};

const BLOCKER_HEADING: Record<ComplianceBlocker['kind'], string> = {
  rule: 'Rule that blocked this',
  debarment: 'Debarment that blocked this',
  eligibility: 'Eligibility gate that blocked this',
};

/**
 * The single most-seen component in the demo (spec Part 9.4). Built once, shared across
 * every persona view, never reimplemented per page.
 */
export function ComplianceResult({
  status,
  reason,
  trigger,
  txRef,
  blockNumber,
  blockHash,
  pathway,
  blockedBy,
  latencyMs,
  explorerHref,
  records,
  className,
}: ComplianceResultProps) {
  const presentation = PRESENTATION[status];
  const { Icon } = presentation;
  const [expanded, setExpanded] = useState(false);
  const recordId = useId();
  const reduceMotion = useReducedMotion();
  const hasRecord = Boolean(txRef || blockNumber !== undefined || records?.length);

  const explorerLink =
    explorerHref ??
    (txRef
      ? `/explorer?tx=${encodeURIComponent(txRef)}`
      : blockNumber !== undefined
        ? `/explorer?block=${blockNumber}`
        : '/explorer');

  return (
    <motion.section
      // Spec Part 9.4 / section 5 of the build contract: scale 0.95 -> 1.0 with an
      // opacity fade over 200 ms. Honoured exactly, and skipped for reduced motion.
      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.95 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.2, ease: 'easeOut' }}
      role={presentation.role}
      aria-live={presentation.live}
      className={cn(
        'w-full overflow-hidden rounded-card border border-border',
        presentation.banner,
        className,
      )}
    >
      {/* The reserved colour as a solid rule, so the verdict survives a projector that
          washes out the 8%-tint background entirely. */}
      <div className={cn('h-1.5 w-full', presentation.accent)} aria-hidden="true" />

      <div className="px-5 py-5 sm:px-6 sm:py-6">
        <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:gap-5">
          <Icon
            className={cn('size-9 shrink-0 sm:size-10', presentation.text)}
            strokeWidth={2}
            aria-hidden="true"
          />

          <div className="min-w-0 flex-1">
            {trigger && (
              <p className="text-xs font-semibold tracking-wide text-ink-muted uppercase">
                {trigger}
              </p>
            )}

            <div className="mt-1 flex flex-wrap items-center gap-3">
              <h2
                className={cn(
                  'text-2xl leading-none font-semibold tracking-tight sm:text-[1.875rem]',
                  presentation.text,
                )}
              >
                {presentation.word}
              </h2>
              {/* The build contract's tri-state token, kept visible so the screen and the
                  API response say the same word. */}
              <span
                className={cn(
                  'rounded px-1.5 py-0.5 font-mono text-[0.6875rem] font-bold tracking-widest',
                  presentation.chip,
                )}
              >
                {status}
              </span>
              {pathway && <PathwayBadge pathway={pathway} />}
            </div>

            {/* Colour and icon are visual; this is what a screen reader leads with. */}
            <span className="sr-only">{presentation.announce}</span>

            <p className="mt-2 max-w-3xl text-[0.9375rem] leading-relaxed text-ink">{reason}</p>

            {status === 'RED' && blockedBy && (
              <div className="mt-4 rounded-lg border border-status-red/25 bg-surface px-4 py-3">
                <p className="flex items-center gap-2 text-xs font-semibold tracking-wide text-status-red uppercase">
                  <Gavel className="size-3.5" aria-hidden="true" />
                  {BLOCKER_HEADING[blockedBy.kind]}
                </p>
                <p className="mt-1.5 text-sm font-semibold text-ink">{blockedBy.label}</p>
                {blockedBy.detail && (
                  <p className="mt-1 text-sm text-ink-muted">{blockedBy.detail}</p>
                )}
                {blockedBy.citation && (
                  <p className="mt-2 text-xs text-ink-subtle">{blockedBy.citation}</p>
                )}
              </div>
            )}

            {(txRef || latencyMs !== undefined) && (
              <div className="mt-4 flex flex-wrap items-center gap-x-5 gap-y-2 text-xs text-ink-muted">
                {txRef && (
                  <span className="inline-flex items-center gap-1.5">
                    <span className="tracking-wide uppercase">Txn</span>
                    <TxRef value={txRef} />
                  </span>
                )}
                {blockNumber !== undefined && (
                  <span className="inline-flex items-center gap-1.5">
                    <span className="tracking-wide uppercase">Finalized block</span>
                    <span className="font-mono text-[0.8125rem] text-ink">
                      #{blockNumber.toLocaleString('en-IN')}
                    </span>
                  </span>
                )}
                {latencyMs !== undefined && (
                  <span className="inline-flex items-center gap-1.5">
                    <span className="tracking-wide uppercase">Finality</span>
                    <span className="font-mono text-[0.8125rem] text-ink">
                      {(latencyMs / 1000).toFixed(2)}s
                    </span>
                  </span>
                )}
              </div>
            )}
          </div>
        </div>

        {hasRecord && (
          <div className="mt-5 border-t border-ink/8 pt-3">
            <button
              type="button"
              onClick={() => setExpanded((value) => !value)}
              aria-expanded={expanded}
              aria-controls={recordId}
              className={cn(
                'inline-flex items-center gap-1.5 rounded text-sm font-medium',
                'transition-colors duration-150 ease-out hover:underline',
                'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
                presentation.text,
              )}
            >
              <ChevronDown
                className={cn(
                  'size-4 transition-transform duration-200 ease-out',
                  expanded && 'rotate-180',
                )}
                aria-hidden="true"
              />
              See the on-chain record
            </button>

            <AnimatePresence initial={false}>
              {expanded && (
                <motion.div
                  id={recordId}
                  initial={reduceMotion ? { opacity: 0 } : { height: 0, opacity: 0 }}
                  animate={{ height: 'auto', opacity: 1 }}
                  exit={reduceMotion ? { opacity: 0 } : { height: 0, opacity: 0 }}
                  transition={{ duration: 0.2, ease: 'easeOut' }}
                  className="overflow-hidden"
                >
                  <dl className="mt-3 rounded-lg border border-border bg-surface px-4 py-1">
                    {txRef && <RecordRow label="Transaction reference" value={<TxRef value={txRef} />} />}
                    {blockNumber !== undefined && (
                      <RecordRow
                        label="Block number"
                        value={`#${blockNumber.toLocaleString('en-IN')}`}
                        mono
                      />
                    )}
                    {blockHash && (
                      <RecordRow
                        label="Block hash"
                        value={<TxRef value={blockHash} label="Block hash" />}
                      />
                    )}
                    {pathway && <RecordRow label="Decision pathway" value={pathway} mono />}
                    {records?.map((record) => (
                      <RecordRow
                        key={record.label}
                        label={record.label}
                        value={record.value}
                        mono={record.mono}
                      />
                    ))}
                  </dl>

                  <Link
                    href={explorerLink}
                    className="mt-3 inline-flex items-center gap-1.5 rounded text-sm font-medium text-cerulea transition-colors duration-150 hover:text-cerulea-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
                  >
                    Open in the block explorer
                    <ExternalLink className="size-3.5" aria-hidden="true" />
                  </Link>
                </motion.div>
              )}
            </AnimatePresence>
          </div>
        )}
      </div>
    </motion.section>
  );
}

function RecordRow({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-6 gap-y-1 border-b border-border py-2.5 last:border-b-0">
      <dt className="text-sm text-ink-muted">{label}</dt>
      <dd className={cn('text-sm text-ink', mono && 'font-mono text-[0.8125rem]')}>{value}</dd>
    </div>
  );
}
