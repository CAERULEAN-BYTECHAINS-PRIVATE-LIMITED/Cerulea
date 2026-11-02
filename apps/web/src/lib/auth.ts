/**
 * Access control for the six trigger-point routes.
 *
 * Every `/api/trigger/*` route signs an extrinsic on the caller's behalf, so an open
 * route means anyone on the internet can write bids, debarments and rule changes to the
 * chain. Two credentials are accepted:
 *
 *   1. `x-api-key: <key>` for machine callers (the GeM integration, the simulator
 *      scripts). Keys are listed, comma-separated, in `PRAMAAN_API_KEYS`.
 *   2. A session cookie set by `POST /api/session` after the demo access code from
 *      `PRAMAAN_DEMO_PASSWORD` is entered in the browser. The cookie value is an HMAC
 *      over a fixed label with `PRAMAAN_SESSION_SECRET` (falls back to the password), so
 *      it is unforgeable without the secret and carries nothing sensitive.
 *
 * If NEITHER `PRAMAAN_API_KEYS` nor `PRAMAAN_DEMO_PASSWORD` is set the routes stay open
 * (local development), and a single warning is logged so this never goes unnoticed on a
 * hosted deployment.
 */

import { createHmac, timingSafeEqual } from 'crypto';

export const SESSION_COOKIE = 'pramaan_session';
export const SESSION_MAX_AGE_SECONDS = 12 * 60 * 60;

export type AuthMode = 'open' | 'api-key' | 'session';

export interface AuthResult {
  ok: boolean;
  mode: AuthMode;
  /** Set when `ok` is false. */
  reason?: string;
}

function apiKeys(): string[] {
  return (process.env.PRAMAAN_API_KEYS ?? '')
    .split(',')
    .map((k) => k.trim())
    .filter((k) => k.length >= 16);
}

function demoPassword(): string | null {
  const value = process.env.PRAMAAN_DEMO_PASSWORD?.trim();
  return value && value.length > 0 ? value : null;
}

function sessionSecret(): string | null {
  return process.env.PRAMAAN_SESSION_SECRET?.trim() || demoPassword();
}

/** True when any credential is configured, i.e. the routes are protected. */
export function accessControlEnabled(): boolean {
  return apiKeys().length > 0 || demoPassword() !== null;
}

let warnedOpen = false;
function warnOpenOnce() {
  if (warnedOpen) return;
  warnedOpen = true;
  console.warn(
    '[auth] PRAMAAN_API_KEYS and PRAMAAN_DEMO_PASSWORD are both unset: trigger routes are OPEN. ' +
      'Set at least one before exposing this deployment.',
  );
}

function safeEqual(a: string, b: string): boolean {
  const ab = Buffer.from(a);
  const bb = Buffer.from(b);
  return ab.length === bb.length && timingSafeEqual(ab, bb);
}

/** The one valid session cookie value for the configured secret. */
export function sessionCookieValue(): string | null {
  const secret = sessionSecret();
  if (!secret) return null;
  return createHmac('sha256', secret).update('cbc-pramaan-session-v1').digest('hex');
}

export function passwordMatches(candidate: string): boolean {
  const expected = demoPassword();
  return expected !== null && safeEqual(candidate, expected);
}

function readCookie(request: Request, name: string): string | null {
  const header = request.headers.get('cookie');
  if (!header) return null;
  for (const part of header.split(';')) {
    const [k, ...rest] = part.trim().split('=');
    if (k === name) return decodeURIComponent(rest.join('='));
  }
  return null;
}

export function authorize(request: Request): AuthResult {
  if (!accessControlEnabled()) {
    warnOpenOnce();
    return { ok: true, mode: 'open' };
  }

  const presented = request.headers.get('x-api-key');
  if (presented) {
    if (apiKeys().some((k) => safeEqual(k, presented))) return { ok: true, mode: 'api-key' };
    return { ok: false, mode: 'api-key', reason: 'Invalid API key.' };
  }

  const expected = sessionCookieValue();
  const cookie = readCookie(request, SESSION_COOKIE);
  if (expected && cookie && safeEqual(cookie, expected)) return { ok: true, mode: 'session' };

  return {
    ok: false,
    mode: 'session',
    reason:
      'Unauthorized. Send a valid x-api-key header, or sign in with the demo access code to obtain a session.',
  };
}

export function sessionCookieHeader(value: string | null): string {
  const base = `${SESSION_COOKIE}=${value ?? ''}; Path=/; HttpOnly; SameSite=Lax; Secure`;
  return value ? `${base}; Max-Age=${SESSION_MAX_AGE_SECONDS}` : `${base}; Max-Age=0`;
}
