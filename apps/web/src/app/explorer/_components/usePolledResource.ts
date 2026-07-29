'use client';

import { useCallback, useEffect, useRef, useState } from 'react';

export interface PolledResource<T> {
  data: T | null;
  /** True only before the first successful load — a refresh must not blank the screen. */
  loading: boolean;
  error: string | null;
  /** When the last successful response landed. */
  updatedAt: number | null;
  refresh: () => void;
}

/**
 * Poll a read-only endpoint on an interval, keeping the last good payload on screen.
 *
 * Two behaviours matter for a live demo and are why this is not a bare `useEffect`:
 *
 *  - **A failed refresh never clears the view.** The last good data stays, and the error
 *    is reported alongside it. A projector going momentarily blank because one poll timed
 *    out is worse than a slightly stale block height.
 *  - **Polls do not overlap.** The next tick is scheduled after the previous response, so
 *    a slow chain read cannot pile up requests behind it.
 */
export function usePolledResource<T>(
  url: string,
  intervalMs: number,
  options: { enabled?: boolean } = {},
): PolledResource<T> {
  const { enabled = true } = options;
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [updatedAt, setUpdatedAt] = useState<number | null>(null);
  const [nonce, setNonce] = useState(0);
  const inFlight = useRef(false);

  const refresh = useCallback(() => setNonce((value) => value + 1), []);

  useEffect(() => {
    if (!enabled) return;
    const controller = new AbortController();
    let timer: ReturnType<typeof setTimeout> | undefined;
    let cancelled = false;

    async function tick() {
      if (inFlight.current) return;
      inFlight.current = true;
      try {
        const response = await fetch(url, { signal: controller.signal, cache: 'no-store' });
        const payload = (await response.json()) as T & { error?: string };
        if (cancelled) return;
        if (!response.ok) throw new Error(payload.error ?? `Request failed (${response.status})`);
        setData(payload);
        setError(null);
        setUpdatedAt(Date.now());
      } catch (caught) {
        if (cancelled || controller.signal.aborted) return;
        setError(caught instanceof Error ? caught.message : 'Request failed');
      } finally {
        inFlight.current = false;
        if (!cancelled) {
          setLoading(false);
          timer = setTimeout(() => void tick(), intervalMs);
        }
      }
    }

    void tick();

    return () => {
      cancelled = true;
      controller.abort();
      if (timer) clearTimeout(timer);
    };
  }, [url, intervalMs, enabled, nonce]);

  return { data, loading, error, updatedAt, refresh };
}
