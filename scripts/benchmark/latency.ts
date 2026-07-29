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
import { stringToU8a } from '@polkadot/util';

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

  const inclusionBlock = await new Promise<number>((resolve, reject) => {
    let unsub: (() => void) | undefined;
    tx.signAndSend(signer, async (result) => {
      if (result.dispatchError) {
        unsub?.();
        reject(new Error(`dispatch error: ${result.dispatchError.toString()}`));
        return;
      }
      if (result.status.isInBlock) {
        unsub?.();
        const header = await api.rpc.chain.getHeader(result.status.asInBlock);
        resolve(header.number.toNumber());
      }
    })
      .then((u) => {
        unsub = u as unknown as () => void;
      })
      .catch(reject);
  });

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
  const alice = keyring.addFromUri('//Alice');

  const blockMs = (api.consts.timestamp?.minimumPeriod?.toJSON() as number) ?? 0;
  console.log(`Connected to ${endpoint}`);
  console.log(`Signing as ${alice.address}`);
  console.log(`Chain minimumPeriod: ${blockMs}ms (block time is 2x this)`);
  console.log(`\nMeasuring finality-confirmed latency over ${runs} runs per route...`);

  const bidSamples: number[] = [];
  for (let i = 0; i < runs; i++) {
    const tx = api.tx.pramaanConsistency.declare(
      alice.address,
      stringToU8a(`BENCH-PRODUCT-${i}`),
      stringToU8a(`BENCH-TENDER-${i}`),
      5_000,
    );
    bidSamples.push(await submitAndAwaitFinality(api, tx, alice));
    process.stdout.write(`\r  bid-submission     ${i + 1}/${runs}`);
  }

  const prefSamples: number[] = [];
  for (let i = 0; i < runs; i++) {
    const tx = api.tx.pramaanConsistency.declare(
      alice.address,
      stringToU8a(`BENCH-PREF-PRODUCT-${i}`),
      stringToU8a(`BENCH-PREF-TENDER-${i}`),
      6_000,
    );
    prefSamples.push(await submitAndAwaitFinality(api, tx, alice));
    process.stdout.write(`\r  preference-calc    ${i + 1}/${runs}`);
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
