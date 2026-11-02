/**
 * A minimal JSON value type plus the response-probing helpers the checks are built on.
 *
 * Probing rather than a fixed accessor is deliberate. The build contract fixes the
 * response fields each route must return, but not their nesting, and the API routes are
 * written by a different agent in parallel. Looking for `qualifies` at the top level and
 * then one level inside the usual envelope keys (`data`, `result`, `outcome`, ...) means
 * a route that wraps its payload still passes the same checks -- while a route that
 * genuinely omits a required field still fails, which is the behaviour that matters.
 */

export type Json = string | number | boolean | null | Json[] | { [key: string]: Json };

/** Keys a route might reasonably wrap its payload in. */
const ENVELOPE_KEYS = ['data', 'result', 'payload', 'outcome', 'response', 'detail', 'chain'];

function isObject(value: Json): value is { [key: string]: Json } {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * Find the first defined value for any of `names` (case-insensitive), searching the
 * top level first and then one level into envelope keys.
 */
export function probe(body: Json, names: readonly string[]): Json | undefined {
  if (!isObject(body)) return undefined;
  const wanted = names.map((n) => n.toLowerCase());

  for (const [key, value] of Object.entries(body)) {
    if (wanted.includes(key.toLowerCase()) && value !== undefined) return value;
  }
  for (const envelopeKey of ENVELOPE_KEYS) {
    const nested = body[envelopeKey];
    if (nested !== undefined && isObject(nested)) {
      for (const [key, value] of Object.entries(nested)) {
        if (wanted.includes(key.toLowerCase()) && value !== undefined) return value;
      }
    }
  }
  return undefined;
}

/** The first array found under any of `names`, top level or one level in. */
export function probeArray(body: Json, names: readonly string[]): Json[] | undefined {
  const found = probe(body, names);
  return Array.isArray(found) ? found : undefined;
}

/**
 * Normalise a monetary field to a plain decimal string of paise.
 * Accepts a number, a bigint-shaped string, a comma-grouped string, or null.
 */
export function normalisePaise(value: Json | undefined): string | null {
  if (value === undefined || value === null) return null;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) return null;
    return BigInt(Math.round(value)).toString();
  }
  if (typeof value === 'string') {
    const cleaned = value.replace(/[,\s_]/g, '');
    if (!/^-?\d+$/.test(cleaned)) return null;
    return BigInt(cleaned).toString();
  }
  return null;
}

/** Normalise a basis-point field to a number. */
export function normaliseBps(value: Json | undefined): number | null {
  if (value === undefined || value === null) return null;
  if (typeof value === 'number') return Math.round(value);
  if (typeof value === 'string') {
    const cleaned = value.replace(/[,\s_%]/g, '');
    if (!/^-?\d+$/.test(cleaned)) return null;
    return Number(cleaned);
  }
  return null;
}

/** Normalise a boolean-ish field. */
export function normaliseBool(value: Json | undefined): boolean | null {
  if (typeof value === 'boolean') return value;
  if (typeof value === 'string') {
    if (/^(true|yes)$/i.test(value)) return true;
    if (/^(false|no)$/i.test(value)) return false;
  }
  return null;
}

/** Everything in the response as one lowercase haystack, for "does it name X" checks. */
export function haystack(raw: string, body: Json): string {
  const bodyText = body === null ? '' : JSON.stringify(body);
  return `${raw}\n${bodyText}`.toLowerCase();
}

/** A short, single-line rendering of a response body for the run log. */
export function summarise(body: Json, maxLength = 220): string {
  if (body === null) return '(no JSON body)';
  const text = JSON.stringify(body);
  return text.length <= maxLength ? text : `${text.slice(0, maxLength - 3)}...`;
}
