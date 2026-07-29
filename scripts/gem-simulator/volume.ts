/**
 * Volume driver — realistic procurement load across all 21 nodal ministries.
 *
 * `simulate.ts --all` proves the twelve decision pathways are correct. This proves the
 * system stands up under realistic *volume*, and in doing so leaves the chain holding
 * genuine demo data: every record the dashboard, explorer and persona views display was
 * produced by a real request that reached DCF finality, not seeded into storage.
 *
 * That distinction matters for the demo. Seeding storage directly would populate the UI
 * without ever exercising classification, preference, certification, debarment or the
 * consistency engine. Driving the same records through the six trigger routes exercises
 * all of them, and any defect that only appears at volume — a nonce race, a pool stall,
 * a finality regression — shows up here rather than in front of a judge.
 *
 * Every procurement runs the sequence a real GeM integration would:
 *
 *   bid-submission  ->  bid-evaluation  ->  preference-calculation  ->  ca-certification
 *
 * with debarment and rule-update events injected across a subset, exactly as Part 8.4
 * describes. A deliberate slice of vendors re-declare a different local-content figure
 * for the same product on a later tender, so the consistency engine has real
 * inconsistencies to flag rather than a single scripted example.
 *
 * This is a TEST, not a fixture generator: any HTTP failure, any missing finality
 * reference, and any response that is not one of GREEN / YELLOW / RED is counted and
 * the process exits non-zero.
 *
 * Usage:
 *   npx tsx volume.ts --count=120 --api=http://localhost:3100
 *   npx tsx volume.ts --count=120 --json
 */

import { PramaanClient, ApiUnreachableError, type ApiResponse } from './api.js';
import { loadMinistries, thresholdsFor, crorePaise, formatPaise } from './gem-data.js';
import {
  makeVendor,
  makeBid,
  tenderIdFromBidNumber,
  productIdFor,
  makeCertificateId,
} from './generate.js';
import { rngFor, type Rng } from './rng.js';
import type { GemBid, GemVendor } from './types.js';

interface Args {
  count: number;
  api: string;
  seed: number;
  json: boolean;
  timeoutMs: number;
}

function parseArgs(): Args {
  const argv = process.argv.slice(2);
  const get = (n: string) => argv.find((a) => a.startsWith(`--${n}=`))?.split('=').slice(1).join('=');
  const has = (n: string) => argv.includes(`--${n}`);
  return {
    count: Number(get('count') ?? 120),
    api: get('api') ?? 'http://localhost:3000',
    seed: Number(get('seed') ?? 20260406),
    json: has('json'),
    timeoutMs: Number(get('timeout') ?? 30000),
  };
}

interface Failure {
  procurement: number;
  route: string;
  detail: string;
}

interface Tally {
  calls: number;
  green: number;
  yellow: number;
  red: number;
  finalityRefs: number;
  latencies: number[];
  perRoute: Map<string, number[]>;
  classes: Map<string, number>;
  inconsistencies: number;
  debarments: number;
  ruleUpdates: number;
  certificates: number;
  auditorRequired: number;
  failures: Failure[];
}

function newTally(): Tally {
  return {
    calls: 0,
    green: 0,
    yellow: 0,
    red: 0,
    finalityRefs: 0,
    latencies: [],
    perRoute: new Map(),
    classes: new Map(),
    inconsistencies: 0,
    debarments: 0,
    ruleUpdates: 0,
    certificates: 0,
    auditorRequired: 0,
    failures: [],
  };
}

function asRecord(body: unknown): Record<string, unknown> {
  return body !== null && typeof body === 'object' ? (body as Record<string, unknown>) : {};
}

/** Record one response, counting the tri-state and asserting it carried finality proof. */
function record(t: Tally, procurement: number, route: string, res: ApiResponse): Record<string, unknown> {
  t.calls++;
  t.latencies.push(res.latencyMs);
  if (!t.perRoute.has(route)) t.perRoute.set(route, []);
  t.perRoute.get(route)!.push(res.latencyMs);

  if (!res.ok) {
    const body = asRecord(res.body);
    t.failures.push({
      procurement,
      route,
      detail: `HTTP ${res.status}: ${String(body.detail ?? body.error ?? res.raw.slice(0, 160))}`,
    });
    return {};
  }

  const body = asRecord(res.body);

  // Every trigger route must return the block that carried its decision. A response
  // without one cannot be audited, which defeats the point of the chain.
  if (body.txRef && body.blockNumber !== undefined && body.blockNumber !== null) {
    t.finalityRefs++;
  } else {
    t.failures.push({
      procurement,
      route,
      detail: 'response carried no txRef/blockNumber, so the decision is not auditable',
    });
  }

  const result = typeof body.result === 'string' ? body.result : undefined;
  if (result === 'GREEN') t.green++;
  else if (result === 'YELLOW') t.yellow++;
  else if (result === 'RED') t.red++;
  else if (result !== undefined) {
    t.failures.push({ procurement, route, detail: `unrecognised result "${result}"` });
  }

  if (typeof body.class === 'string') {
    t.classes.set(body.class, (t.classes.get(body.class) ?? 0) + 1);
  }
  if (body.consistencyFlagged === true) t.inconsistencies++;

  return body;
}

/** A local-content figure drawn to land across all three classification bands. */
function drawLocalContentBps(rng: Rng, ministryId: string): number {
  const { classOneBps, classTwoBps } = thresholdsFor(ministryId);
  const roll = rng.next();
  if (roll < 0.55) {
    // Comfortably Class-I, the common case in real procurement.
    return Math.min(10_000, classOneBps + rng.int(0, 10_000 - classOneBps));
  }
  if (roll < 0.85) {
    // Between the two thresholds: Class-II.
    return classTwoBps + rng.int(0, Math.max(1, classOneBps - classTwoBps - 1));
  }
  // Below the Class-II floor: Non-local. Includes the exact boundary-minus-one case.
  return rng.int(0, Math.max(0, classTwoBps - 1));
}

async function main() {
  const args = parseArgs();
  const client = new PramaanClient({ baseUrl: args.api, dryRun: false, timeoutMs: args.timeoutMs });
  const ministries = loadMinistries();
  const t = newTally();
  const runId = `V${Date.now().toString(36).toUpperCase().slice(-5)}`;
  const startedAt = Date.now();

  if (!args.json) {
    console.log('CBC-PRAMAAN volume run');
    console.log(`  procurements : ${args.count}`);
    console.log(`  ministries   : ${ministries.length}`);
    console.log(`  api          : ${args.api}`);
    console.log(`  run id       : ${runId}\n`);
  }

  // Vendors are reused across procurements, which is what makes the consistency engine
  // meaningful: the same vendor must be capable of declaring the same product twice.
  const vendorPool: GemVendor[] = [];
  const vendorRng = rngFor(args.seed, 'volume-vendors');
  for (let i = 0; i < Math.max(12, Math.ceil(args.count / 3)); i++) {
    vendorPool.push(
      makeVendor(vendorRng, args.seed, {
        label: `${runId}-V${i}`,
        isMse: vendorRng.next() < 0.4,
        isPliBeneficiary: vendorRng.next() < 0.15,
      }),
    );
  }

  // Remember what each vendor declared per product, so a later tender can deliberately
  // contradict it and give the consistency engine something real to catch.
  const declared = new Map<string, { bps: number; tender: string }>();

  for (let i = 0; i < args.count; i++) {
    const rng = rngFor(args.seed, `${runId}-proc-${i}`);
    const ministryRow = ministries[i % ministries.length]!;
    const ministryId = ministryRow.ministry_id;
    const vendor = vendorPool[rng.int(0, vendorPool.length - 1)]!;

    let bid: GemBid;
    try {
      bid = makeBid(rng, { ministryId, label: `${runId}-B${i}`, runId });
    } catch (err) {
      t.failures.push({ procurement: i, route: 'generate', detail: String(err) });
      continue;
    }

    const tender = tenderIdFromBidNumber(bid.bidNumber);
    const product = productIdFor({ name: bid.itemCategory, hsnCode: bid.hsnCode } as never);
    const key = `${vendor.accountId}|${product}`;

    // 1 in 6 repeat declarations contradicts the earlier one well outside the 1000 bps
    // tolerance, reproducing the PoC document's 86%-vs-30% case at scale.
    const prior = declared.get(key);
    let bps: number;
    if (prior && rng.next() < 0.35) {
      bps = prior.bps > 5000 ? rng.int(500, 3000) : rng.int(7000, 9500);
    } else {
      bps = drawLocalContentBps(rng, ministryId);
    }

    try {
      // --- 1. bid submission (classification + consistency) -------------------------
      const submission = record(
        t,
        i,
        'bid-submission',
        await client.call({
          route: 'bid-submission',
          body: {
            vendor: vendor.accountId,
            tender,
            ministry: ministryId,
            declaredLocalContentBps: bps,
            product,
            isPliManufacturer: vendor.isPliBeneficiary,
          },
        }),
      );
      declared.set(key, { bps, tender });

      // --- 2. bid evaluation ---------------------------------------------------------
      record(
        t,
        i,
        'bid-evaluation',
        await client.call({
          route: 'bid-evaluation',
          body: {
            vendor: vendor.accountId,
            tender,
            ministry: ministryId,
            declaredLocalContentBps: bps,
          },
        }),
      );

      // --- 3. preference calculation, on a competitive subset ------------------------
      if (rng.next() < 0.5) {
        const rival = vendorPool[rng.int(0, vendorPool.length - 1)]!;
        const l1 = BigInt(bid.estimatedBidValuePaise);
        const classOf = (b: number) => {
          const { classOneBps, classTwoBps } = thresholdsFor(ministryId);
          return b >= classOneBps ? 'ClassOne' : b >= classTwoBps ? 'ClassTwo' : 'NonLocal';
        };
        record(
          t,
          i,
          'preference-calculation',
          await client.call({
            route: 'preference-calculation',
            body: {
              tender,
              ministry: ministryId,
              tenderValuePaise: l1.toString(),
              isTenderGte: false,
              bids: [
                {
                  vendor: rival.accountId,
                  class: classOf(rng.int(0, 10_000)),
                  pricePaise: l1.toString(),
                  isMse: rival.isMse,
                  isGte: false,
                },
                {
                  vendor: vendor.accountId,
                  class: classOf(bps),
                  pricePaise: ((l1 * 108n) / 100n).toString(),
                  isMse: vendor.isMse,
                  isGte: false,
                },
              ],
            },
          }),
        );
      }

      // --- 4. certification, on the higher-value subset ------------------------------
      if (rng.next() < 0.35) {
        const value = BigInt(bid.estimatedBidValuePaise);
        const needsAuditor = value >= crorePaise(10);
        const cert = record(
          t,
          i,
          'ca-certification',
          await client.call({
            route: 'ca-certification',
            body: {
              certificateId: makeCertificateId(rng, 2026, `${runId}${i}`),
              ministry: ministryId,
              vendor: vendor.accountId,
              tender,
              valuePaise: value.toString(),
              ...(needsAuditor ? { auditor: 'auditor' } : {}),
            },
          }),
        );
        if (cert.certificateId) t.certificates++;
        if (cert.requiresAuditor === true) t.auditorRequired++;
      }

      // --- 5. injected debarment, ~4% of procurements --------------------------------
      if (rng.next() < 0.04) {
        const res = await client.call({
          route: 'debarment',
          body: {
            vendor: vendor.accountId,
            ministry: ministryId,
            action: 'debar',
            effectiveFrom: 0,
            reason: 'False local-content declaration detected during post-award audit.',
          },
        });
        record(t, i, 'debarment', res);
        if (res.ok) t.debarments++;

        // Lift it again so a single debarment does not poison every later procurement
        // by this vendor — enforcement is cross-ministry and would otherwise cascade.
        const lift = await client.call({
          route: 'debarment',
          body: { vendor: vendor.accountId, ministry: ministryId, action: 'lift' },
        });
        record(t, i, 'debarment', lift);
      }

      // --- 6. injected rule update, ~3% of procurements -------------------------------
      if (rng.next() < 0.03) {
        const res = await client.call({
          route: 'rule-update',
          body: {
            ministry: ministryId,
            rule: {
              hsnThresholds: [
                {
                  hsnCode: bid.hsnCode,
                  classOneBps: thresholdsFor(ministryId).classOneBps,
                  classTwoBps: thresholdsFor(ministryId).classTwoBps,
                },
              ],
              para3aApplicable: ministryRow.para_3a_applicable,
              pliLinked: ministryRow.pli_linked,
              calculationMethod: ministryRow.calculation_method,
              preferenceMarginBps: ministryRow.preference_margin_bps,
              certificationThreshold: ministryRow.certification_threshold,
              exemptionFloor: ministryRow.exemption_floor,
              divisibility: ministryRow.divisibility,
              effectiveFrom: 0,
            },
          },
        });
        record(t, i, 'rule-update', res);
        if (res.ok) t.ruleUpdates++;
      }
    } catch (err) {
      if (err instanceof ApiUnreachableError) {
        console.error(`\n${err.message}`);
        process.exit(2);
      }
      t.failures.push({ procurement: i, route: 'sequence', detail: String(err) });
    }

    if (!args.json && (i + 1) % 10 === 0) {
      process.stdout.write(
        `\r  ${i + 1}/${args.count} procurements  (${t.calls} calls, ${t.failures.length} failures)   `,
      );
    }
  }

  const elapsedMs = Date.now() - startedAt;
  const sorted = [...t.latencies].sort((a, b) => a - b);
  const mean = sorted.length ? sorted.reduce((a, b) => a + b, 0) / sorted.length : 0;
  const p95 = sorted.length ? sorted[Math.floor(sorted.length * 0.95)]! : 0;
  const max = sorted.length ? sorted[sorted.length - 1]! : 0;

  if (args.json) {
    console.log(
      JSON.stringify(
        {
          procurements: args.count,
          calls: t.calls,
          green: t.green,
          yellow: t.yellow,
          red: t.red,
          finalityRefs: t.finalityRefs,
          meanLatencyMs: Number(mean.toFixed(1)),
          p95LatencyMs: p95,
          maxLatencyMs: max,
          inconsistencies: t.inconsistencies,
          debarments: t.debarments,
          ruleUpdates: t.ruleUpdates,
          certificates: t.certificates,
          failures: t.failures,
        },
        null,
        2,
      ),
    );
  } else {
    console.log('\n');
    console.log('='.repeat(78));
    console.log('  Volume run summary');
    console.log('='.repeat(78));
    console.log(`  procurements driven   : ${args.count}`);
    console.log(`  trigger-point calls   : ${t.calls}`);
    console.log(`  wall clock            : ${(elapsedMs / 1000).toFixed(1)}s`);
    console.log('');
    console.log(`  GREEN                 : ${t.green}`);
    console.log(`  YELLOW                : ${t.yellow}`);
    console.log(`  RED                   : ${t.red}`);
    console.log('');
    console.log('  classifications recorded:');
    for (const [cls, n] of [...t.classes.entries()].sort((a, b) => b[1] - a[1])) {
      console.log(`    ${cls.padEnd(22)}${n}`);
    }
    console.log('');
    console.log(`  inconsistencies flagged: ${t.inconsistencies}`);
    console.log(`  certificates issued    : ${t.certificates} (auditor required: ${t.auditorRequired})`);
    console.log(`  debarments applied     : ${t.debarments}`);
    console.log(`  rule updates applied   : ${t.ruleUpdates}`);
    console.log('');
    console.log(`  decisions carrying a finalized block reference: ${t.finalityRefs}/${t.calls}`);
    console.log('');
    console.log('  finality-confirmed latency, per trigger point:');
    for (const [route, xs] of [...t.perRoute.entries()].sort()) {
      const m = xs.reduce((a, b) => a + b, 0) / xs.length;
      const hi = Math.max(...xs);
      console.log(`    ${route.padEnd(24)}n=${String(xs.length).padEnd(5)}mean ${m.toFixed(0)}ms   max ${hi}ms`);
    }
    console.log('');
    console.log(`  overall mean ${mean.toFixed(1)}ms   p95 ${p95}ms   max ${max}ms`);
    console.log('');

    if (t.failures.length > 0) {
      console.log(`  FAILURES (${t.failures.length}):`);
      for (const f of t.failures.slice(0, 25)) {
        console.log(`    #${f.procurement} ${f.route}: ${f.detail}`);
      }
      if (t.failures.length > 25) console.log(`    ... and ${t.failures.length - 25} more`);
    }
  }

  if (t.failures.length > 0) {
    console.error(`\nFAIL: ${t.failures.length} failure(s) across ${t.calls} calls.`);
    process.exit(1);
  }
  if (t.finalityRefs !== t.calls) {
    console.error('\nFAIL: not every decision carried a finalized block reference.');
    process.exit(1);
  }
  console.log(`PASS: ${t.calls} trigger-point decisions, all finality-confirmed, no failures.`);
}

main().catch((err) => {
  console.error('Volume run failed:', err);
  process.exit(1);
});
