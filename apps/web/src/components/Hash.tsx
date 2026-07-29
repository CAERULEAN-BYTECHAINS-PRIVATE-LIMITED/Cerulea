'use client';

import { Check, Copy } from 'lucide-react';
import { useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * A transaction hash, block hash or account address, rendered the way an identifier
 * should be: monospace, middle-truncated so both ends stay readable from the back of a
 * room, and copyable in full.
 *
 * The complete value is always in the DOM — as `title` and in a screen-reader-only span —
 * so what a judge copies and what an auditor's screen reader reads is the whole
 * reference, not the abbreviation.
 */
export function Hash({
  value,
  label = 'Transaction reference',
  head = 8,
  tail = 6,
  copyable = true,
  className,
}: {
  value: string;
  label?: string;
  head?: number;
  tail?: number;
  copyable?: boolean;
  className?: string;
}) {
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), 1600);
    return () => clearTimeout(timer);
  }, [copied]);

  const shortened =
    value.length > head + tail + 3 ? `${value.slice(0, head)}…${value.slice(-tail)}` : value;

  async function copy() {
    try {
      await navigator.clipboard.writeText(value);
      setCopied(true);
    } catch {
      // Clipboard access can be denied; the full value is on screen via `title` regardless.
    }
  }

  return (
    <span className={cn('inline-flex items-center gap-1', className)}>
      <span className="font-mono text-2xs" title={value}>
        <span aria-hidden="true">{shortened}</span>
        <span className="sr-only">
          {label}: {value}
        </span>
      </span>
      {copyable && (
        <button
          type="button"
          onClick={copy}
          aria-label={copied ? `${label} copied` : `Copy ${label.toLowerCase()}`}
          className="rounded-sm p-0.5 text-ink-subtle transition-colors duration-150 hover:text-ink focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
        >
          {copied ? (
            <Check className="size-3" aria-hidden="true" />
          ) : (
            <Copy className="size-3" aria-hidden="true" />
          )}
        </button>
      )}
      <span aria-live="polite" className="sr-only">
        {copied ? 'Copied to clipboard' : ''}
      </span>
    </span>
  );
}

/** A block height, always with the same `#1,234` shape and Indian digit grouping. */
export function BlockRef({ value, className }: { value: number; className?: string }) {
  return (
    <span className={cn('font-mono text-2xs tabular-nums', className)}>
      #{value.toLocaleString('en-IN')}
    </span>
  );
}
