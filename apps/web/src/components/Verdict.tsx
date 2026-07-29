'use client';

import { motion, useReducedMotion } from 'framer-motion';
import { ChevronRight, CircleCheck, CircleX, ExternalLink, TriangleAlert } from 'lucide-react';
import Link from 'next/link';
import { useId, useState, type ReactNode } from 'react';
import { BlockRef, Hash } from './Hash';
import { PathwayChip, type PathwayId } from './Pathway';
import { cn } from './ui/cn';

/** The tri-state every trigger-point route returns (build contract section 3). */
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

export interface VerdictProps {
  status: ComplianceStatus;
  /** One plain sentence a non-technical reader can take at face value. */
  reason: string;
  /** What produced this verdict, e.g. "Bid evaluation · GEM/2025/B/6798497". */
  trigger?: string;
  txRef?: string;
  blockNumber?: number;
  blockHash?: string;
  pathway?: PathwayId;
  /** RED only. Rendered before anyone has to expand anything. */
  blockedBy?: ComplianceBlocker;
  latencyMs?: number;
  explorerHref?: string;
  /** Extra facts for the expanded record, e.g. declared local content. */
  records?: { label: string; value: ReactNode; mono?: boolean }[];
  className?: string;
}

interface Presentation {
  word: string;
  Icon: typeof CircleCheck;
  /** How assistive technology hears it, since colour conveys nothing there. */
  announce: string;
  live: 'polite' | 'assertive';
  role: 'status' | 'alert';
  band: string;
  onBand: string;
  rule: string;
}

/**
 * Colour is never the signal on its own.
 *
 * Each verdict is carried by three independent channels: a distinct ICON shape (tick /
 * triangle / cross), the STATUS WORD set large, and only then the reserved colour. A
 * reader with deuteranopia, a projector with a washed-out gamut, and a screen-reader user
 * all get the same verdict.
 *
 * The colour is a solid filled band across the head of the block — a stamp on a file —
 * rather than an 8% tint, because an 8% tint is the first thing a conference-room
 * projector loses.
 */
const PRESENTATION: Record<ComplianceStatus, Presentation> = {
  GREEN: {
    word: 'Compliant',
    Icon: CircleCheck,
    announce: 'Compliant. Proceeds.',
    live: 'polite',
    role: 'status',
    band: 'bg-status-green',
    onBand: 'text-white',
    rule: 'border-status-green',
  },
  YELLOW: {
    word: 'Review required',
    Icon: TriangleAlert,
    announce: 'Review required. Proceeds with a caveat.',
    live: 'polite',
    role: 'status',
    band: 'bg-status-yellow',
    // #E8A100 carries 7.5:1 against ink and 2.4:1 against white. Ink is the only option.
    onBand: 'text-ink',
    rule: 'border-status-yellow',
  },
  RED: {
    word: 'Blocked',
    Icon: CircleX,
    announce: 'Blocked. This cannot proceed.',
    live: 'assertive',
    role: 'alert',
    band: 'bg-status-red',
    onBand: 'text-white',
    rule: 'border-status-red',
  },
};

const BLOCKER_HEADING: Record<ComplianceBlocker['kind'], string> = {
  rule: 'Rule that blocked this',
  debarment: 'Debarment that blocked this',
  eligibility: 'Eligibility gate that blocked this',
};

/**
 * The single most-seen component in the product (spec Part 9.4). Built once, shared by
 * every persona view, never reimplemented per page.
 */
export function Verdict({
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
}: VerdictProps) {
  const presentation = PRESENTATION[status];
  const { Icon } = presentation;
  const [expanded, setExpanded] = useState(false);
  const recordId = useId();
  const reduceMotion = useReducedMotion();
  const hasRecord = Boolean(txRef || blockNumber !== undefined || blockHash || records?.length);

  const explorerLink =
    explorerHref ??
    (txRef
      ? `/explorer?tx=${encodeURIComponent(txRef)}`
      : blockNumber !== undefined
        ? `/explorer?block=${blockNumber}`
        : '/explorer');

  return (
    <motion.section
      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, scale: 0.985 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.2, ease: [0.25, 1, 0.5, 1] }}
      role={presentation.role}
      aria-live={presentation.live}
      className={cn(
        'overflow-hidden rounded-md border-2 bg-paper',
        presentation.rule,
        className,
      )}
    >
      {/* The stamp */}
      <div
        className={cn(
          'flex flex-wrap items-center gap-x-3 gap-y-1.5 px-3 py-2',
          presentation.band,
          presentation.onBand,
        )}
      >
        <Icon className="size-5 shrink-0" strokeWidth={2.25} aria-hidden="true" />
        <p className="text-2xl font-semibold tracking-tight">{presentation.word}</p>
        <span
          className={cn(
            'rounded-sm px-1.5 py-px font-mono text-2xs font-semibold tracking-widest',
            status === 'YELLOW' ? 'bg-ink/12 text-ink' : 'bg-white/20 text-white',
          )}
        >
          {status}
        </span>
        {trigger && (
          <p
            className={cn(
              'ml-auto truncate text-2xs tracking-wide uppercase',
              status === 'YELLOW' ? 'text-ink/70' : 'text-white/80',
            )}
          >
            {trigger}
          </p>
        )}
        <span className="sr-only">{presentation.announce}</span>
      </div>

      <div className="px-3 py-3">
        <p className="max-w-[75ch] text-base text-ink">{reason}</p>

        {status === 'RED' && blockedBy && (
          <div className="mt-3 border border-status-red/30 bg-status-red-bg px-3 py-2">
            <p className="text-2xs font-semibold tracking-wide text-status-red-ink uppercase">
              {BLOCKER_HEADING[blockedBy.kind]}
            </p>
            <p className="mt-1 text-sm font-semibold text-ink">{blockedBy.label}</p>
            {blockedBy.detail && <p className="mt-0.5 text-sm text-ink-muted">{blockedBy.detail}</p>}
            {blockedBy.citation && (
              <p className="mt-1.5 text-2xs text-ink-muted">{blockedBy.citation}</p>
            )}
          </div>
        )}

        {/* The record strip: what a judge reads out loud. */}
        {(txRef || blockNumber !== undefined || latencyMs !== undefined || pathway) && (
          <dl className="mt-3 flex flex-wrap items-center gap-x-6 gap-y-1.5 border-t border-line pt-2.5 text-2xs">
            {pathway && (
              <div className="flex items-center gap-1.5">
                <dt className="tracking-wide text-ink-muted uppercase">Pathway</dt>
                <dd>
                  <PathwayChip pathway={pathway} />
                </dd>
              </div>
            )}
            {txRef && (
              <div className="flex items-center gap-1.5">
                <dt className="tracking-wide text-ink-muted uppercase">Txn</dt>
                <dd className="text-ink">
                  <Hash value={txRef} />
                </dd>
              </div>
            )}
            {blockNumber !== undefined && (
              <div className="flex items-center gap-1.5">
                <dt className="tracking-wide text-ink-muted uppercase">Finalized block</dt>
                <dd className="text-ink">
                  <BlockRef value={blockNumber} />
                </dd>
              </div>
            )}
            {latencyMs !== undefined && (
              <div className="flex items-center gap-1.5">
                <dt className="tracking-wide text-ink-muted uppercase">Finality</dt>
                <dd className="font-mono text-ink tabular-nums">
                  {(latencyMs / 1000).toFixed(2)}s
                </dd>
              </div>
            )}
          </dl>
        )}

        {hasRecord && (
          <div className="mt-2.5">
            <button
              type="button"
              onClick={() => setExpanded((value) => !value)}
              aria-expanded={expanded}
              aria-controls={recordId}
              className="inline-flex items-center gap-1 rounded-sm text-2xs font-semibold tracking-wide text-accent uppercase transition-colors duration-150 hover:text-accent-dark focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
            >
              <ChevronRight
                className={cn('size-3 transition-transform duration-150', expanded && 'rotate-90')}
                aria-hidden="true"
              />
              On-chain record
            </button>

            {expanded && (
              <div id={recordId} className="mt-2">
                <dl className="grid grid-cols-1 border-t border-line sm:grid-cols-2 sm:gap-x-6">
                  {txRef && (
                    <Row label="Transaction reference" value={<Hash value={txRef} head={12} tail={10} />} />
                  )}
                  {blockNumber !== undefined && (
                    <Row label="Block number" value={<BlockRef value={blockNumber} />} />
                  )}
                  {blockHash && (
                    <Row
                      label="Block hash"
                      value={<Hash value={blockHash} label="Block hash" head={12} tail={10} />}
                    />
                  )}
                  {pathway && <Row label="Decision pathway" value={pathway} mono />}
                  {records?.map((record) => (
                    <Row
                      key={record.label}
                      label={record.label}
                      value={record.value}
                      mono={record.mono}
                    />
                  ))}
                </dl>

                <Link
                  href={explorerLink}
                  className="mt-2 inline-flex items-center gap-1 rounded-sm text-2xs font-medium text-accent transition-colors duration-150 hover:text-accent-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                >
                  Open in the block explorer
                  <ExternalLink className="size-3" aria-hidden="true" />
                </Link>
              </div>
            )}
          </div>
        )}
      </div>
    </motion.section>
  );
}

function Row({
  label,
  value,
  mono = false,
}: {
  label: string;
  value: ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex flex-wrap items-baseline justify-between gap-x-4 border-b border-line py-1.5">
      <dt className="text-2xs text-ink-muted">{label}</dt>
      <dd className={cn('text-xs font-medium text-ink', mono && 'font-mono')}>{value}</dd>
    </div>
  );
}
