'use client';

import { Check, Copy } from 'lucide-react';
import { useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * A transaction hash or block hash, rendered the way a hash should be: monospace, middle-
 * truncated so both ends stay readable from the back of a room, and copyable in full.
 *
 * The full value is always in the DOM (`title` plus a screen-reader-only span), so what a
 * judge copies and what an auditor's screen reader reads is the complete reference, not
 * the abbreviation.
 */
export function TxRef({
  value,
  label = 'Transaction reference',
  head = 10,
  tail = 8,
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
    <span className={cn('inline-flex items-center gap-1.5', className)}>
      <span className="font-mono text-[0.8125rem] tracking-tight" title={value}>
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
          className="rounded p-1 text-current opacity-60 transition-opacity duration-150 hover:opacity-100 focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-current"
        >
          {copied ? (
            <Check className="size-3.5" aria-hidden="true" />
          ) : (
            <Copy className="size-3.5" aria-hidden="true" />
          )}
        </button>
      )}
      <span aria-live="polite" className="sr-only">
        {copied ? 'Copied to clipboard' : ''}
      </span>
    </span>
  );
}
