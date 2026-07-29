'use client';

import { useCallback, useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * Connection state for the header indicator.
 *
 * Note the colours below: teal, ink and border grey. The connection indicator deliberately
 * does NOT use --status-green or --status-red. Those three carry a legal meaning in this
 * application (a compliance verdict) and a green dot next to "network live" would teach a
 * judge the wrong association within the first five seconds of the demo.
 */
type Connection = 'checking' | 'live' | 'offline';

export interface ChainStatusData {
  connected: boolean;
  finalizedBlock?: number;
  chainName?: string;
  endpoint?: string;
}

const POLL_INTERVAL_MS = 5_000;

function readNumber(source: Record<string, unknown>, ...keys: string[]): number | undefined {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'number' && Number.isFinite(value)) return value;
    if (typeof value === 'string' && value.trim() !== '' && Number.isFinite(Number(value))) {
      return Number(value);
    }
  }
  return undefined;
}

function readString(source: Record<string, unknown>, ...keys: string[]): string | undefined {
  for (const key of keys) {
    const value = source[key];
    if (typeof value === 'string' && value.trim() !== '') return value;
  }
  return undefined;
}

/**
 * Chain connectivity and the latest finalized block, polled every five seconds.
 *
 * The status endpoint is owned by the API layer, so this component is written to survive
 * it being absent, slow or shaped slightly differently: any failure degrades to a calm
 * "status unavailable" rather than an error, and the payload is read leniently across the
 * obvious field names. A demo must never show a stack trace in its header.
 */
export function ChainStatus({
  endpoint = '/api/chain/status',
  className,
}: {
  endpoint?: string;
  className?: string;
}) {
  const [state, setState] = useState<Connection>('checking');
  const [data, setData] = useState<ChainStatusData | null>(null);

  const poll = useCallback(
    async (signal: AbortSignal) => {
      try {
        const response = await fetch(endpoint, { signal, cache: 'no-store' });
        if (!response.ok) throw new Error(`status ${response.status}`);

        const payload = (await response.json()) as Record<string, unknown>;
        const finalizedBlock = readNumber(
          payload,
          'finalizedBlock',
          'finalizedBlockNumber',
          'latestFinalizedBlock',
          'blockNumber',
          'block',
        );
        const connected =
          typeof payload.connected === 'boolean'
            ? payload.connected
            : finalizedBlock !== undefined;

        setData({
          connected,
          finalizedBlock,
          chainName: readString(payload, 'chainName', 'chain', 'name'),
          endpoint: readString(payload, 'endpoint', 'wsEndpoint', 'rpc'),
        });
        setState(connected ? 'live' : 'offline');
      } catch (error) {
        if (signal.aborted || (error instanceof DOMException && error.name === 'AbortError')) {
          return;
        }
        setState('offline');
        setData(null);
      }
    },
    [endpoint],
  );

  useEffect(() => {
    const controller = new AbortController();
    // The first poll is scheduled rather than called inline: this effect's job is to set up
    // the subscription, and the state it produces arrives from the fetch callback.
    const first = setTimeout(() => void poll(controller.signal), 0);
    const timer = setInterval(() => void poll(controller.signal), POLL_INTERVAL_MS);
    return () => {
      controller.abort();
      clearTimeout(first);
      clearInterval(timer);
    };
  }, [poll]);

  const dot =
    state === 'live'
      ? 'bg-teal'
      : state === 'checking'
        ? 'bg-ink-subtle'
        : 'bg-ink-subtle opacity-50';

  const headline =
    state === 'live'
      ? 'Network live'
      : state === 'checking'
        ? 'Checking network'
        : 'Status unavailable';

  const detail =
    state === 'live' && data?.finalizedBlock !== undefined
      ? `#${data.finalizedBlock.toLocaleString('en-IN')} finalized`
      : state === 'live'
        ? 'Connected'
        : state === 'checking'
          ? '—'
          : 'Retrying';

  return (
    <div
      className={cn(
        'inline-flex items-center gap-2 rounded-full border border-border bg-surface px-3 py-1.5',
        className,
      )}
      title={data?.endpoint ? `Cerulea endpoint: ${data.endpoint}` : undefined}
    >
      <span className="relative flex size-2 shrink-0" aria-hidden="true">
        {state === 'live' && (
          <span className="absolute inline-flex size-full animate-ping rounded-full bg-teal opacity-60" />
        )}
        <span className={cn('relative inline-flex size-2 rounded-full', dot)} />
      </span>

      <span aria-live="polite" className="flex items-baseline gap-1.5 leading-none">
        <span className="text-xs font-medium text-ink">{headline}</span>
        <span className="font-mono text-[0.6875rem] text-ink-muted tabular-nums">{detail}</span>
      </span>

      <span className="sr-only">
        {state === 'live'
          ? `Connected to the Cerulea network${
              data?.finalizedBlock !== undefined
                ? `. Latest finalized block ${data.finalizedBlock}.`
                : '.'
            }`
          : state === 'checking'
            ? 'Checking the network connection.'
            : 'Network status is unavailable. Retrying every five seconds.'}
      </span>
    </div>
  );
}
