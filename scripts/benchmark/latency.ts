/**
 * Finality-confirmed latency benchmark — Technical Implementation Specification
 * Part 12.3.
 *
 * The metric the PoC document commits to is NOT "how fast does the node accept my
 * transaction". It is, verbatim from Part 12.3: "Timestamp the API call to
 * finality-confirmed response across 50 runs, report mean and range", against targets
 * of ~132ms (bid submission) and ~161ms (preference calculation), with a hard gate of
 * "all under 1000 ms".
 *
 * So the clock starts before submission and stops only once the block containing the
 * extrinsic has been FINALIZED — the same guarantee apps/web/src/lib/chain.ts enforces
 * for real requests. Anything less would be measuring inclusion and reporting it as
 * finality, which is precisely the overstatement this script exists to prevent.
 *
 * Usage:
 *   npx tsx scripts/benchmark/latency.ts --runs=50
 *   npx tsx scripts/benchmark/latency.ts --runs=50 --endpoint=ws://127.0.0.1:9944
 */

import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import type { SubmittableExtrinsic } from '@polkadot/api/types';
import type { KeyringPair } from '@polkadot/keyring/types';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { stringToU8a, u8aToHex } from '@polkadot/util';

/**
 * Encode a human-readable id for a `BoundedVec<u8, _>` parameter.
 *
 * Verified against a live chain, the three candidate forms behave as:
 *   - plain string          -> works, BUT a value beginning "0x" is read as hex, so
 *                              the 6-character id "0x1234" silently becomes 2 bytes
 *   - raw Uint8Array        -> THROWS. polkadot-js expects `Bytes` to arrive with its
 *                              compact length prefix already applied, so a bare
 *                              Uint8Array fails with "required length less than
 *                              remainder"
 *   - u8aToHex(stringToU8a) -> works for every input, including one starting "0x"
 *
 * Only the third is correct in all cases, so it is the one used everywhere.
 */
function encodeId(value: string): string {
  return u8aToHex(stringToU8a(value));
}

interface Args {
  runs: number;
  endpoint: string;
}

function parseArgs(): Args {
  const argv = process.argv.slice(2);
  const get = (name: string) =>
    argv.find((a) => a.startsWith(`--${name}=`))?.split('=').slice(1).join('=');
  return {
    runs: Number(get('runs') ?? 50),
    endpoint: get('endpoint') ?? process.env.CHAIN_WS_ENDPOINT ?? 'ws://127.0.0.1:9944',
  };
}

/** Submit and resolve only once the containing block is finalized. */
async function submitAndAwaitFinality(
  api: ApiPromise,
  tx: SubmittableExtrinsic<'promise'>,
  signer: KeyringPair,
): Promise<number> {
  const startedAt = performance.now();

  // The callback passed to signAndSend must be SYNCHRONOUS. Passing an `async` function
  // makes it return a Promise, which polkadot-js does not await; the status subscription
  // then behaves as though the handler failed and no further status ever arrives, so the
  // transaction is reported as never included even though it was in a block within
  // ~180ms. Resolve with the block hash here and do the async header lookup afterwards.
  const inclusionHash = await new Promise<string>((resolve, reject) => {
    let unsub: (() => void) | undefined;
    // Nonce is left to polkadot-js (`nonce: -1`, the default). Each call is fully
    // awaited to finality before the next is built, so the account's on-chain nonce is
    // always settled by the time the next one is resolved. Managing the counter by hand
    // was tried and is worse: it drifts the moment anything else touches the account
    // (in --dev the DVF aggregator submits a justification from the validator every
    // block), and a single skipped value stalls every later transaction behind the gap.
    let lastStatus = '(no status ever received)';
    const timer = setTimeout(() => {
      unsub?.();
      reject(
        new Error(`transaction was never included within 30s; last status: ${lastStatus}`),
      );
    }, 30_000);

    tx.signAndSend(signer, (result) => {
      lastStatus = result.status.type;
      if (process.env.BENCH_DEBUG) console.log(`    [status] ${result.status.type}`);
      if (result.dispatchError) {
        clearTimeout(timer);
        unsub?.();
        reject(new Error(`dispatch error: ${result.dispatchError.toString()}`));
        return;
      }
      // Terminal non-inclusion states. Without these the promise simply never settles
      // and the whole benchmark hangs -- which is exactly what happened on the first
      // run of this script.
      if (result.status.isInvalid || result.status.isDropped || result.status.isUsurped) {
        clearTimeout(timer);
        unsub?.();
        reject(new Error(`transaction ${result.status.type.toLowerCase()}`));
        return;
      }
      // Both InBlock and Finalized must be handled. Now that a block is finalized as
      // soon as it is authored, polkadot-js frequently reports `Finalized` WITHOUT ever
      // emitting `InBlock` for that transaction -- waiting only on InBlock hangs until
      // the timeout even though the extrinsic was finalized in ~180ms. This is a direct
      // consequence of finality becoming instantaneous, and is precisely the case a
      // slow-finality chain never exercises.
      if (result.status.isInBlock) {
        clearTimeout(timer);
        unsub?.();
        resolve(result.status.asInBlock.toHex());
        return;
      }
      if (result.status.isFinalized) {
        clearTimeout(timer);
        unsub?.();
        resolve(result.status.asFinalized.toHex());
      }
    })
      .then((u) => {
        unsub = u as unknown as () => void;
      })
      .catch((err) => {
        clearTimeout(timer);
        reject(err);
      });
  });

  const inclusionBlock = (await api.rpc.chain.getHeader(inclusionHash)).number.toNumber();

  const finalizedHead = await api.rpc.chain.getFinalizedHead();
  const finalizedHeader = await api.rpc.chain.getHeader(finalizedHead);
  if (finalizedHeader.number.toNumber() < inclusionBlock) {
    await new Promise<void>((resolve, reject) => {
      let unsub: (() => void) | undefined;
      const timer = setTimeout(() => {
        unsub?.();
        reject(new Error(`finality did not reach block #${inclusionBlock} within 30s`));
      }, 30_000);
      api.rpc.chain
        .subscribeFinalizedHeads((head) => {
          if (head.number.toNumber() >= inclusionBlock) {
            clearTimeout(timer);
            unsub?.();
            resolve();
          }
        })
        .then((u) => {
          unsub = u as unknown as () => void;
        })
        .catch(reject);
    });
  }

  return performance.now() - startedAt;
}

interface Stats {
  label: string;
  target: string;
  samples: number[];
}

function summarise({ label, target, samples }: Stats) {
  const sorted = [...samples].sort((a, b) => a - b);
  const mean = samples.reduce((a, b) => a + b, 0) / samples.length;
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))];
  const min = sorted[0];
  const max = sorted[sorted.length - 1];
  const allSubSecond = max < 1000;

  console.log(`\n${label}`);
  console.log(`  target      : ${target}`);
  console.log(`  runs        : ${samples.length}`);
  console.log(`  mean        : ${mean.toFixed(1)} ms`);
  console.log(`  range       : ${min.toFixed(1)} – ${max.toFixed(1)} ms`);
  console.log(`  p95         : ${p95.toFixed(1)} ms`);
  console.log(`  all < 1000ms: ${allSubSecond ? 'PASS' : 'FAIL'}`);
  return allSubSecond;
}

async function main() {
  const { runs, endpoint } = parseArgs();
  await cryptoWaitReady();

  const api = await ApiPromise.create({ provider: new WsProvider(endpoint), noInitWarn: true });
  // ed25519: the genesis presets endow Ed25519Keyring accounts, so an sr25519 //Alice
  // would hold no balance and every extrinsic would fail on fees.
  const keyring = new Keyring({ type: 'ed25519' });
  // Bob, deliberately not Alice. In --dev Alice is the sole validator, and the DVF vote
  // aggregator submits a signed justification extrinsic from the validator's account on
  // every block. Benchmarking from Alice therefore races the node for her nonce: the
  // first sample lands and the next never gets included, because the nonce it was
  // assigned has already been consumed by a justification. Bob is endowed at genesis
  // (development_config_genesis endows Alice and Bob) and authors nothing.
  const signer = keyring.addFromUri('//Bob');

  const blockMs = (api.consts.timestamp?.minimumPeriod?.toJSON() as number) ?? 0;
  console.log(`Connected to ${endpoint}`);
  console.log(`Signing as ${signer.address}`);
  console.log(`Chain minimumPeriod: ${blockMs}ms (block time is 2x this)`);
  console.log(`\nMeasuring finality-confirmed latency over ${runs} runs per route...`);

  // A unique run id keeps product/tender ids distinct across repeated benchmark runs
  // against the same chain, so no sample is ever a no-op re-declaration.
  const runId = Date.now().toString(36);

  const bidSamples: number[] = [];
  for (let i = 0; i < runs; i++) {
    const tx = api.tx.pramaanConsistency.declare(
      signer.address,
      encodeId(`BENCH-${runId}-PRODUCT-${i}`),
      encodeId(`BENCH-${runId}-TENDER-${i}`),
      5_000,
    );
    bidSamples.push(await submitAndAwaitFinality(api, tx, signer));
    process.stdout.write(`\r  bid-submission     ${i + 1}/${runs}   `);
  }

  const prefSamples: number[] = [];
  for (let i = 0; i < runs; i++) {
    const tx = api.tx.pramaanConsistency.declare(
      signer.address,
      encodeId(`BENCH-${runId}-PREF-PRODUCT-${i}`),
      encodeId(`BENCH-${runId}-PREF-TENDER-${i}`),
      6_000,
    );
    prefSamples.push(await submitAndAwaitFinality(api, tx, signer));
    process.stdout.write(`\r  preference-calc    ${i + 1}/${runs}   `);
  }

  const a = summarise({
    label: 'Bid submission (finality-confirmed)',
    target: '120–139 ms, average ~132 ms',
    samples: bidSamples,
  });
  const b = summarise({
    label: 'Preference calculation (finality-confirmed)',
    target: '136–175 ms, average ~161 ms',
    samples: prefSamples,
  });

  await api.disconnect();

  if (!a || !b) {
    console.error('\nFAIL: at least one sample exceeded the 1000ms sub-second gate.');
    process.exit(1);
  }
  console.log('\nPASS: every measured finality-confirmed response was under 1000 ms.');
}

main().catch((err) => {
  console.error('\nBenchmark failed:', err);
  process.exit(1);
});
