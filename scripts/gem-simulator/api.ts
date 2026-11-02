/**
 * HTTP client for the six trigger-point routes (build contract section 3).
 *
 * The simulator talks to the Next.js API rather than to the chain directly, on purpose:
 * spec Part 8.4 asks it to drive the trigger points "in the sequence a real GeM
 * integration would", and a real GeM integration would call an HTTP endpoint, not sign
 * its own extrinsics. That also means this file has zero dependencies -- Node 18+'s
 * global `fetch` is all it needs.
 */

import type { Json } from './json';

export const ROUTES = [
  'bid-submission',
  'bid-evaluation',
  'preference-calculation',
  'ca-certification',
  'debarment',
  'rule-update',
] as const;

export type RouteName = (typeof ROUTES)[number];

export interface ApiCall {
  route: RouteName;
  body: Record<string, Json>;
}

export interface ApiResponse {
  /** HTTP 2xx. */
  ok: boolean;
  status: number;
  /** Parsed JSON body, or `null` when the response was not JSON. */
  body: Json;
  /** The response text exactly as received, used for "does the reason name X" checks. */
  raw: string;
  latencyMs: number;
}

/**
 * Raised when the API cannot be reached at all. Kept distinct from a route returning an
 * error, because the operator response is completely different: one means "start the
 * services", the other means "the decision engine said no".
 */
export class ApiUnreachableError extends Error {
  constructor(
    readonly baseUrl: string,
    override readonly cause: unknown,
  ) {
    super(
      `Cannot reach the CBC-PRAMAAN API at ${baseUrl}.\n` +
        `  - Is the web app running?   cd apps/web && npm run dev   (expects ${baseUrl})\n` +
        `  - Is the chain running?     ./scripts/start_all.sh       (expects ws://127.0.0.1:9944)\n` +
        `  - Different host or port?   pass --api=http://your-host:port\n` +
        `  Underlying error: ${describeCause(cause)}`,
    );
    this.name = 'ApiUnreachableError';
  }
}

function describeCause(cause: unknown): string {
  if (cause instanceof Error) {
    const inner = (cause as { cause?: unknown }).cause;
    const code =
      inner && typeof inner === 'object' && 'code' in inner
        ? ` (${String((inner as { code: unknown }).code)})`
        : '';
    return `${cause.message}${code}`;
  }
  return String(cause);
}

export interface ClientOptions {
  baseUrl: string;
  /** Print the request body instead of sending it. Used by `--dry-run`. */
  dryRun: boolean;
  timeoutMs: number;
}

export class PramaanClient {
  private readonly baseUrl: string;
  private readonly dryRun: boolean;
  private readonly timeoutMs: number;

  constructor(opts: ClientOptions) {
    this.baseUrl = opts.baseUrl.replace(/\/+$/, '');
    this.dryRun = opts.dryRun;
    this.timeoutMs = opts.timeoutMs;
  }

  get isDryRun(): boolean {
    return this.dryRun;
  }

  urlFor(route: RouteName): string {
    return `${this.baseUrl}/api/trigger/${route}`;
  }

  async call(call: ApiCall): Promise<ApiResponse> {
    if (this.dryRun) {
      return { ok: true, status: 0, body: null, raw: '', latencyMs: 0 };
    }

    const url = this.urlFor(call.route);
    const startedAt = Date.now();
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), this.timeoutMs);

    let response: Response;
    try {
      response = await fetch(url, {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(call.body),
        signal: controller.signal,
      });
    } catch (err) {
      clearTimeout(timer);
      if (controller.signal.aborted) {
        throw new Error(
          `POST ${url} did not respond within ${this.timeoutMs}ms. The routes wait for DCF ` +
            `finality with a 10s budget of their own, so raise --timeout if the chain is ` +
            `starting up, or check that the validators are producing blocks.`,
        );
      }
      throw new ApiUnreachableError(this.baseUrl, err);
    }
    clearTimeout(timer);

    const raw = await response.text();
    let body: Json = null;
    if (raw.length > 0) {
      try {
        body = JSON.parse(raw) as Json;
      } catch {
        body = null;
      }
    }

    return {
      ok: response.ok,
      status: response.status,
      body,
      raw,
      latencyMs: Date.now() - startedAt,
    };
  }
}
