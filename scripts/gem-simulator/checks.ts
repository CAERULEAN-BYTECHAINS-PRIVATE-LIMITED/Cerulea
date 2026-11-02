/**
 * The declarative expectation language the twelve scenarios are written in.
 *
 * Scenarios say what must be true, not how to test it, so `scenarios.ts` reads as a
 * specification a reviewer can check by eye. Every expectation carries a `why` string
 * that is printed both when it passes and when it fails, so the run log doubles as the
 * evidence trail for the "12 of 12 pathways" claim.
 */

import type { ApiResponse } from './api';
import {
  haystack,
  normaliseBool,
  normaliseBps,
  normalisePaise,
  probe,
  probeArray,
  type Json,
} from './json';
import type { ClassResult, TriState } from './types';

export type Expectation =
  /** The route answered 2xx. */
  | { kind: 'http-ok'; why: string }
  /** The tri-state `result` field equals this. */
  | { kind: 'result'; equals: TriState; why: string }
  /** The tri-state `result` field is one of these -- used where two readings are defensible. */
  | { kind: 'result-one-of'; anyOf: readonly TriState[]; why: string }
  /** The `class` field equals this classification. */
  | { kind: 'class'; equals: ClassResult; why: string }
  /** A named field equals a literal (string / number / boolean comparison). */
  | { kind: 'field'; names: readonly string[]; equals: string | number | boolean; why: string }
  /** A named field is present and non-empty. */
  | { kind: 'field-present'; names: readonly string[]; why: string }
  /** A monetary field equals this many paise, or is null when `paise` is null. */
  | { kind: 'paise'; names: readonly string[]; paise: bigint | null; why: string }
  /** A basis-point field equals this. */
  | { kind: 'bps'; names: readonly string[]; bps: number; why: string }
  /** A boolean field equals this. */
  | { kind: 'bool'; names: readonly string[]; value: boolean; why: string }
  /**
   * The call was refused -- non-2xx, or a RED result -- and the refusal names one of
   * `names`, so a judge is told which rule blocked it. `alsoAccept` widens what counts
   * as a refusal for the one case where the build contract admits two readings: an
   * above-threshold certification with no auditor is arguably YELLOW ("still awaiting
   * its auditor certificate") rather than RED.
   */
  | { kind: 'blocked'; names: readonly string[]; alsoAccept?: readonly TriState[]; why: string }
  /** `txRef` and `blockNumber` are both present -- proof the answer came after finality. */
  | { kind: 'finalized'; why: string }
  /** The response text mentions all of these (case-insensitive). */
  | { kind: 'mentions'; texts: readonly string[]; why: string }
  /** One vendor's row in the preference outcome. */
  | {
      kind: 'preference-outcome';
      vendor: string;
      vendorName: string;
      qualifies?: boolean;
      awardedPercentBps?: number;
      decisionPath?: string;
      matchedPricePaise?: bigint | null;
      why: string;
    };

export interface CheckResult {
  passed: boolean;
  why: string;
  detail: string;
}

const RESULT_NAMES = ['result', 'status', 'triState', 'decision'] as const;
const CLASS_NAMES = ['class', 'classResult', 'classification', 'classificationResult'] as const;
const TX_NAMES = ['txRef', 'txHash', 'transactionRef', 'extrinsicHash'] as const;
const BLOCK_NAMES = ['blockNumber', 'block', 'finalizedBlock'] as const;
const OUTCOME_ARRAY_NAMES = [
  'outcomes',
  'vendorOutcomes',
  'preferenceResults',
  'results',
  'bidOutcomes',
  'bids',
] as const;

export function evaluate(expectation: Expectation, response: ApiResponse): CheckResult {
  switch (expectation.kind) {
    case 'http-ok':
      return {
        passed: response.ok,
        why: expectation.why,
        detail: response.ok ? `HTTP ${response.status}` : `HTTP ${response.status}: ${trim(response.raw)}`,
      };

    case 'result': {
      const value = probe(response.body, RESULT_NAMES);
      const actual = typeof value === 'string' ? value.toUpperCase() : undefined;
      return {
        passed: actual === expectation.equals,
        why: expectation.why,
        detail:
          actual === undefined
            ? `no "result" field in the response; the route must return the tri-state GREEN|YELLOW|RED`
            : `result = ${actual} (wanted ${expectation.equals})`,
      };
    }

    case 'result-one-of': {
      const value = probe(response.body, RESULT_NAMES);
      const actual = typeof value === 'string' ? value.toUpperCase() : undefined;
      const passed = actual !== undefined && expectation.anyOf.includes(actual as TriState);
      return {
        passed,
        why: expectation.why,
        detail:
          actual === undefined
            ? 'no "result" field in the response'
            : `result = ${actual} (wanted one of ${expectation.anyOf.join(', ')})`,
      };
    }

    case 'class': {
      const value = probe(response.body, CLASS_NAMES);
      const actual = typeof value === 'string' ? value : undefined;
      return {
        passed: actual !== undefined && normaliseClass(actual) === expectation.equals,
        why: expectation.why,
        detail:
          actual === undefined
            ? 'no "class" field in the response'
            : `class = ${actual} (wanted ${expectation.equals})`,
      };
    }

    case 'field': {
      const value = probe(response.body, expectation.names);
      const passed = looseEquals(value, expectation.equals);
      return {
        passed,
        why: expectation.why,
        detail:
          value === undefined
            ? `no field named ${expectation.names.join('/')} in the response`
            : `${expectation.names[0]} = ${JSON.stringify(value)} (wanted ${JSON.stringify(expectation.equals)})`,
      };
    }

    case 'field-present': {
      const value = probe(response.body, expectation.names);
      const passed = value !== undefined && value !== null && value !== '';
      return {
        passed,
        why: expectation.why,
        detail: passed
          ? `${expectation.names[0]} = ${JSON.stringify(value)}`
          : `no non-empty field named ${expectation.names.join('/')} in the response`,
      };
    }

    case 'paise': {
      const value = probe(response.body, expectation.names);
      const actual = normalisePaise(value);
      const wanted = expectation.paise === null ? null : expectation.paise.toString();
      const present = value !== undefined;
      return {
        passed: present && actual === wanted,
        why: expectation.why,
        detail: !present
          ? `no field named ${expectation.names.join('/')} in the response`
          : `${expectation.names[0]} = ${actual ?? 'null'} paise (wanted ${wanted ?? 'null'})`,
      };
    }

    case 'bps': {
      const actual = normaliseBps(probe(response.body, expectation.names));
      return {
        passed: actual === expectation.bps,
        why: expectation.why,
        detail:
          actual === null
            ? `no field named ${expectation.names.join('/')} in the response`
            : `${expectation.names[0]} = ${actual} bps (wanted ${expectation.bps})`,
      };
    }

    case 'bool': {
      const actual = normaliseBool(probe(response.body, expectation.names));
      return {
        passed: actual === expectation.value,
        why: expectation.why,
        detail:
          actual === null
            ? `no boolean field named ${expectation.names.join('/')} in the response`
            : `${expectation.names[0]} = ${actual} (wanted ${expectation.value})`,
      };
    }

    case 'blocked': {
      const resultValue = probe(response.body, RESULT_NAMES);
      const resultText = typeof resultValue === 'string' ? resultValue.toUpperCase() : undefined;
      const refusalStates: readonly string[] = ['RED', ...(expectation.alsoAccept ?? [])];
      const isRefusalState = resultText !== undefined && refusalStates.includes(resultText);
      const refused = !response.ok || isRefusalState;
      const text = haystack(response.raw, response.body);
      const named = expectation.names.find((n) => text.includes(n.toLowerCase()));
      const passed = refused && named !== undefined;
      let detail: string;
      if (!refused) {
        detail =
          `the call was accepted (HTTP ${response.status}` +
          `${resultText ? `, result ${resultText}` : ''}) but it had to be refused`;
      } else if (named === undefined) {
        detail =
          `refused, but the response never names the rule that caused it. ` +
          `Expected one of: ${expectation.names.join(', ')}. Got: ${trim(response.raw)}`;
      } else {
        detail = `refused and named "${named}"`;
      }
      return { passed, why: expectation.why, detail };
    }

    case 'finalized': {
      const txRef = probe(response.body, TX_NAMES);
      const blockNumber = normaliseBps(probe(response.body, BLOCK_NAMES));
      const hasTx = typeof txRef === 'string' && txRef.length > 0;
      const hasBlock = blockNumber !== null && blockNumber >= 0;
      return {
        passed: hasTx && hasBlock,
        why: expectation.why,
        detail:
          hasTx && hasBlock
            ? `txRef ${trim(String(txRef), 20)} finalized in block #${blockNumber}`
            : `missing ${!hasTx ? 'txRef' : ''}${!hasTx && !hasBlock ? ' and ' : ''}${!hasBlock ? 'blockNumber' : ''}` +
              ` -- a decision is only a decision once its block is final`,
      };
    }

    case 'mentions': {
      const text = haystack(response.raw, response.body);
      const missing = expectation.texts.filter((t) => !text.includes(t.toLowerCase()));
      return {
        passed: missing.length === 0,
        why: expectation.why,
        detail:
          missing.length === 0
            ? `response mentions ${expectation.texts.map((t) => `"${t}"`).join(', ')}`
            : `response never mentions ${missing.map((t) => `"${t}"`).join(', ')}`,
      };
    }

    case 'preference-outcome': {
      const row = findVendorOutcome(response.body, expectation.vendor);
      if (row === undefined) {
        return {
          passed: false,
          why: expectation.why,
          detail:
            `no per-vendor outcome for ${expectation.vendorName} in the response. ` +
            `The preference route must return an "outcomes" array of ` +
            `{ vendor, qualifies, awardedPercentBps, matchedPricePaise, decisionPath } -- ` +
            `without it there is no evidence of which of P8/P9/P10 fired.`,
        };
      }
      const problems: string[] = [];
      const seen: string[] = [];

      if (expectation.qualifies !== undefined) {
        const actual = normaliseBool(probe(row, ['qualifies', 'qualified']));
        seen.push(`qualifies=${actual}`);
        if (actual !== expectation.qualifies) {
          problems.push(`qualifies is ${actual}, wanted ${expectation.qualifies}`);
        }
      }
      if (expectation.awardedPercentBps !== undefined) {
        const actual = normaliseBps(probe(row, ['awardedPercentBps', 'awardedPercent', 'awardBps']));
        seen.push(`awarded=${actual}bps`);
        if (actual !== expectation.awardedPercentBps) {
          problems.push(`awardedPercentBps is ${actual}, wanted ${expectation.awardedPercentBps}`);
        }
      }
      if (expectation.decisionPath !== undefined) {
        const value = probe(row, ['decisionPath', 'pathway', 'pathwayId', 'path']);
        const actual = typeof value === 'string' ? value.toUpperCase() : null;
        seen.push(`path=${actual}`);
        if (actual !== expectation.decisionPath.toUpperCase()) {
          problems.push(`decisionPath is ${actual}, wanted ${expectation.decisionPath}`);
        }
      }
      if (expectation.matchedPricePaise !== undefined) {
        const raw = probe(row, ['matchedPricePaise', 'matchedPrice']);
        const actual = normalisePaise(raw);
        const wanted = expectation.matchedPricePaise === null ? null : expectation.matchedPricePaise.toString();
        seen.push(`matched=${actual ?? 'null'}`);
        if (actual !== wanted) {
          problems.push(`matchedPrice is ${actual ?? 'null'} paise, wanted ${wanted ?? 'null'}`);
        }
      }

      return {
        passed: problems.length === 0,
        why: expectation.why,
        detail:
          problems.length === 0
            ? `${expectation.vendorName}: ${seen.join(', ')}`
            : `${expectation.vendorName}: ${problems.join('; ')}`,
      };
    }

    default: {
      // Exhaustiveness: adding an expectation kind without handling it is a type error.
      const never: never = expectation;
      throw new Error(`Unhandled expectation ${JSON.stringify(never)}`);
    }
  }
}

function findVendorOutcome(body: Json, vendor: string): Json | undefined {
  const rows = probeArray(body, OUTCOME_ARRAY_NAMES);
  if (!rows) return undefined;
  return rows.find((row) => {
    const value = probe(row, ['vendor', 'vendorId', 'accountId', 'account', 'address']);
    return typeof value === 'string' && value === vendor;
  });
}

function normaliseClass(value: string): string {
  // Accept `ClassOne`, `Class-I`, `CLASS_ONE`, `Class I` and friends.
  const compact = value.replace(/[\s_-]/g, '').toLowerCase();
  if (compact === 'classone' || compact === 'classi' || compact === 'class1') return 'ClassOne';
  if (compact === 'classtwo' || compact === 'classii' || compact === 'class2') return 'ClassTwo';
  if (compact === 'nonlocal') return 'NonLocal';
  if (compact === 'manualreviewrequired' || compact === 'manualreview') return 'ManualReviewRequired';
  return value;
}

function looseEquals(value: Json | undefined, wanted: string | number | boolean): boolean {
  if (value === undefined || value === null) return false;
  if (typeof wanted === 'string' && typeof value === 'string') {
    return value.toLowerCase() === wanted.toLowerCase();
  }
  if (typeof wanted === 'number') return normaliseBps(value) === wanted;
  if (typeof wanted === 'boolean') return normaliseBool(value) === wanted;
  return false;
}

function trim(text: string, max = 180): string {
  const flat = text.replace(/\s+/g, ' ').trim();
  return flat.length <= max ? flat : `${flat.slice(0, max - 3)}...`;
}
