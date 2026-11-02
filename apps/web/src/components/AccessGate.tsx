'use client';

import { useEffect, useState, type FormEvent } from 'react';

/**
 * Asks for the demo access code before any console can fire a trigger point.
 *
 * Rendered once, inside the shell, on every page. It asks `/api/session` whether the
 * deployment is protected and whether this browser already holds a session; only when
 * the answer is "protected, not authenticated" does it show the modal. Machine callers
 * never see this: they send `x-api-key` instead (src/lib/auth.ts).
 */
type State = 'checking' | 'open' | 'locked' | 'ready';

export function AccessGate() {
  const [state, setState] = useState<State>('checking');
  const [code, setCode] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const controller = new AbortController();
    fetch('/api/session', { cache: 'no-store', signal: controller.signal })
      .then((r) => r.json())
      .then((j: { protected?: boolean; authenticated?: boolean }) => {
        if (!j.protected) setState('open');
        else setState(j.authenticated ? 'ready' : 'locked');
      })
      .catch(() => setState('open'));
    return () => controller.abort();
  }, []);

  async function submit(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const r = await fetch('/api/session', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ password: code }),
      });
      if (r.ok) {
        setState('ready');
        setCode('');
      } else {
        const j = (await r.json().catch(() => ({}))) as { error?: string };
        setError(j.error ?? `Sign-in failed (${r.status}).`);
      }
    } catch {
      setError('Network error while signing in.');
    } finally {
      setBusy(false);
    }
  }

  if (state !== 'locked') return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="access-gate-title"
      className="fixed inset-0 z-50 flex items-center justify-center bg-masthead/70 p-4"
    >
      <form
        onSubmit={submit}
        className="w-full max-w-sm rounded-md border border-line-strong bg-paper p-5 text-ink shadow-modal"
      >
        <p className="text-2xs font-medium tracking-wide text-ink-muted uppercase">
          Restricted console
        </p>
        <h2 id="access-gate-title" className="mt-1 text-lg font-semibold tracking-tight">
          Enter the access code
        </h2>
        <p className="mt-2 text-xs text-ink-muted">
          Every action in these consoles writes a finalized record to the Cerulea chain, so
          access is limited to invited reviewers. Integrations use an API key instead.
        </p>
        <label className="mt-4 block text-xs font-medium" htmlFor="access-code">
          Access code
        </label>
        <input
          id="access-code"
          type="password"
          autoComplete="current-password"
          autoFocus
          value={code}
          onChange={(e) => setCode(e.target.value)}
          aria-invalid={error ? true : undefined}
          className="mt-1 h-8 w-full rounded-md border border-line-strong bg-paper px-2 text-sm text-ink focus:border-accent focus:outline-2 focus:-outline-offset-1 focus:outline-accent aria-[invalid=true]:border-form-error aria-[invalid=true]:bg-form-error-bg"
        />
        {error && <p className="mt-2 text-xs text-form-error">{error}</p>}
        <button
          type="submit"
          disabled={busy || code.length === 0}
          className="mt-4 h-8 w-full rounded-md bg-accent px-3 text-sm font-medium text-white disabled:cursor-not-allowed disabled:opacity-60"
        >
          {busy ? 'Signing in...' : 'Continue'}
        </button>
      </form>
    </div>
  );
}
