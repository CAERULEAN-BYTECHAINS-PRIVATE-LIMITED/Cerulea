'use client';

import { useCallback, useEffect, useState } from 'react';
import { cn } from './ui/cn';

/**
 * Connection state and the latest finalized block, polled every five seconds and shown in
 * the masthead.
 *
 * Note what the indicator is NOT: it is not green when connected. The three status
 * colours carry a legal meaning in this application, and a green dot beside "network
 * live" would teach a judge the wrong association inside the first five seconds. Live is
 * a filled white dot; unavailable is a hollow one.
 */
type Connection = 'checking' | 'live' | 'offline';

export interface ChainStatusData {
  connected: boolean;
  finalizedBlock?: number;
  bestBlock?: number;
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
          typeof payload.connected === 'boolean' ? payload.connected : finalizedBlock !== undefined;

        setData({
          connected,
          finalizedBlock,
          bestBlock: readNumber(payload, 'bestBlock'),
          chainName: readString(payload, 'chainName', 'chain', 'name'),
          endpoint: readString(payload, 'endpoint', 'wsEndpoint', 'rpc'),
        });
        setState(connected ? 'live' : 'offline');
      } catch (error) {
        if (signal.aborted || (error instanceof DOMException && error.name === 'AbortError')) return;
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

  const headline =
    state === 'live' ? 'Network live' : state === 'checking' ? 'Connecting' : 'Unavailable';

  return (
    <span
      className={cn('inline-flex items-center gap-2 text-2xs whitespace-nowrap', className)}
      title={data?.endpoint ? `Cerulea endpoint: ${data.endpoint}` : undefined}
    >
      <span
        aria-hidden="true"
        className={cn(
          'size-1.5 shrink-0 rounded-full',
          state === 'live' ? 'bg-white' : 'border border-white/50 bg-transparent',
        )}
      />
      <span aria-live="polite" className="flex items-baseline gap-2">
        <span className="text-white/70">{headline}</span>
        {state === 'live' && data?.finalizedBlock !== undefined && (
          <span className="font-mono text-white tabular-nums">
            <span className="text-white/50">final</span> #
            {data.finalizedBlock.toLocaleString('en-IN')}
          </span>
        )}
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
    </span>
  );
}
