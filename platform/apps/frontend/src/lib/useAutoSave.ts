'use client';

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

export type AutoSaveStatus = 'idle' | 'saving' | 'saved' | 'error';

type RemoteConfig<T> = {
  /** e.g. `/api/drafts?projectId=${id}&bucket=project&step=1` */
  url: string;
  method?: 'POST' | 'PUT' | 'PATCH';
  headers?: Record<string, string>;
  body?: (data: T) => unknown;
};

export interface UseAutoSaveOptions<T> {
  projectId?: string | null;
  stepCode: number | string;
  data: T;
  debounceMs?: number;
  /** Optional remote save endpoint */
  remote?: RemoteConfig<T> | null;
  /** Optional: load remote draft once on mount (recommended true only for real projectIds) */
  loadRemoteOnce?: boolean;
}

export interface UseAutoSaveReturn<T> {
  status: AutoSaveStatus;
  lastSavedAt: number | null;
  loadLocal: () => T | null;
  clearLocal: () => void;
  saveNow: () => Promise<void>;
  /** If you enable loadRemoteOnce, you can call this to manually pull once */
  loadRemoteNow: () => Promise<T | null>;
}

function isLocalProjectId(projectId?: string | null) {
  return !projectId || projectId === 'local';
}

export function useAutoSave<T>({
  projectId,
  stepCode,
  data,
  debounceMs = 300,
  remote = null,
  loadRemoteOnce = false,
}: UseAutoSaveOptions<T>): UseAutoSaveReturn<T> {
  const [status, setStatus] = useState<AutoSaveStatus>('idle');
  const [lastSavedAt, setLastSavedAt] = useState<number | null>(null);

  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Kill-switch for remote drafts if API missing (404) or repeatedly failing
  const remoteDisabledRef = useRef(false);

  // Cancel in-flight remote request when new save happens
  const inFlightAbortRef = useRef<AbortController | null>(null);

  const storageKey = useMemo(() => {
    if (!projectId) return null;
    return `draft:${projectId}:${String(stepCode)}`;
  }, [projectId, stepCode]);

  const writeLocal = useCallback(
    (payload: T) => {
      if (!storageKey) return;
      try {
        const record = { data: payload, t: Date.now() };
        localStorage.setItem(storageKey, JSON.stringify(record));
        setLastSavedAt(record.t);
      } catch {
        // ignore quota errors
      }
    },
    [storageKey]
  );

  const loadLocal = useCallback((): T | null => {
    if (!storageKey) return null;
    try {
      const raw = localStorage.getItem(storageKey);
      if (!raw) return null;
      const parsed = JSON.parse(raw) as { data: T; t: number };
      return parsed?.data ?? null;
    } catch {
      return null;
    }
  }, [storageKey]);

  const clearLocal = useCallback(() => {
    if (!storageKey) return;
    try {
      localStorage.removeItem(storageKey);
    } catch {
      // ignore
    }
  }, [storageKey]);

  const loadRemoteNow = useCallback(async (): Promise<T | null> => {
    // never load remote for local projects
    if (isLocalProjectId(projectId)) return null;
    if (!remote || remoteDisabledRef.current) return null;

    try {
      const res = await fetch(remote.url, { method: 'GET' });

      // IMPORTANT: if drafts route doesn't exist, stop forever (prevents paid loops)
      if (res.status === 404) {
        remoteDisabledRef.current = true;
        return null;
      }

      if (!res.ok) throw new Error(`Remote load failed: ${res.status}`);

      const json = (await res.json()) as any;
      // allow either {data: ...} or raw payload
      return (json?.data ?? json) as T;
    } catch {
      // do not retry loop here; caller can choose to re-run manually
      return null;
    }
  }, [projectId, remote]);

  const sendRemote = useCallback(
    async (payload: T) => {
      // never remote-save for local projects
      if (isLocalProjectId(projectId)) return;

      if (!remote || remoteDisabledRef.current) return;

      // cancel previous
      if (inFlightAbortRef.current) inFlightAbortRef.current.abort();
      const ac = new AbortController();
      inFlightAbortRef.current = ac;

      try {
        const res = await fetch(remote.url, {
          method: remote.method ?? 'POST',
          headers: {
            'Content-Type': 'application/json',
            ...(remote.headers || {}),
          },
          body: JSON.stringify(remote.body ? remote.body(payload) : payload),
          signal: ac.signal,
        });

        // If route missing or not supported, disable remote drafts for this session
        if (res.status === 404) {
          remoteDisabledRef.current = true;
          return;
        }

        // Don’t keep hammering on auth/validation errors either
        if (res.status === 401 || res.status === 403 || res.status === 400) {
          remoteDisabledRef.current = true;
          return;
        }

        if (!res.ok) throw new Error(`Remote save failed: ${res.status}`);
      } catch (e: any) {
        if (e?.name === 'AbortError') return; // normal during rapid changes
        setStatus('error');
      }
    },
    [projectId, remote]
  );

  const performSave = useCallback(async () => {
    // If no projectId at all, don’t do anything (prevents weird states)
    if (!storageKey) return;

    setStatus('saving');
    writeLocal(data);
    await sendRemote(data);
    setStatus('saved');
  }, [data, storageKey, writeLocal, sendRemote]);

  // Debounced autosave on data changes
  useEffect(() => {
    if (!storageKey) return;

    // For projectId="local", this still saves locally — but never hits server
    if (timer.current) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      void performSave();
    }, debounceMs);

    return () => {
      if (timer.current) clearTimeout(timer.current);
    };
  }, [data, storageKey, debounceMs, performSave]);

  // Optional: load remote draft ONCE (no polling)
  useEffect(() => {
    if (!loadRemoteOnce) return;
    if (isLocalProjectId(projectId)) return;
    if (!remote || remoteDisabledRef.current) return;

    // one-shot
    void loadRemoteNow();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [loadRemoteOnce, projectId]);

  const saveNow = useCallback(async () => {
    if (!storageKey) return;
    if (timer.current) clearTimeout(timer.current);
    await performSave();
  }, [storageKey, performSave]);

  return { status, lastSavedAt, loadLocal, clearLocal, saveNow, loadRemoteNow };
}
