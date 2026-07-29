/**
 * The DCF validator set, with its PoS and PoI inputs — the explorer's Validators tab.
 *
 * Read straight out of `pallet-cerulea-pos` and `pallet-cerulea-dcf`:
 *
 *   - `pos.validators`            — the registered set
 *   - `pos.validatorScores`       — the Proof-of-Stake score, 0–100
 *   - `pos.stake`                 — stake backing each validator
 *   - `poi.validatorInferenceCount` — Proof-of-Inference submissions counted this epoch
 *   - `dcf.validatorTrustScores`  — the combined DCF trust score
 *   - `dcf.posWeight` / `poiWeight` — the basis-point split the combined score uses
 *
 * `blocksAuthored` is counted over the indexed window only, and the response says so, so
 * the UI can label it "in the last N blocks" rather than presenting it as a lifetime total
 * it has not measured.
 */

import { getApi } from '@/lib/chain';
import { indexWindow, recentBlocks, syncIndex } from '../_lib/reader';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

function toNumber(value: { toString(): string }): number {
  return Number(value.toString().replace(/,/g, ''));
}

export async function GET(): Promise<Response> {
  try {
    await syncIndex();
    const api = await getApi();

    const [registered, posScores, trustScores, inferenceCounts, posWeight, poiWeight, epoch] =
      await Promise.all([
        api.query.pos.validators.entries(),
        api.query.pos.validatorScores.entries(),
        api.query.dcf.validatorTrustScores.entries(),
        api.query.poi.validatorInferenceCount.entries(),
        api.query.dcf.posWeight(),
        api.query.dcf.poiWeight(),
        api.query.dcf.currentEpoch(),
      ]);

    const posScoreByAddress = new Map(
      posScores.map(([key, value]) => [key.args[0].toString(), toNumber(value)]),
    );
    const trustByAddress = new Map(
      trustScores.map(([key, value]) => [key.args[0].toString(), toNumber(value)]),
    );
    const inferenceByAddress = new Map(
      inferenceCounts.map(([key, value]) => [key.args[0].toString(), toNumber(value)]),
    );

    const window = indexWindow();
    const authored = new Map<string, number>();
    for (const block of recentBlocks(window.blocksIndexed)) {
      if (!block.author) continue;
      authored.set(block.author, (authored.get(block.author) ?? 0) + 1);
    }

    const addresses = new Set<string>([
      ...registered.map(([key]) => key.args[0].toString()),
      ...trustByAddress.keys(),
      ...authored.keys(),
    ]);

    const stakes = await Promise.all(
      [...addresses].map(async (address) => {
        try {
          const stake = await api.query.pos.stake(address);
          return [address, stake.toString()] as const;
        } catch {
          return [address, null] as const;
        }
      }),
    );
    const stakeByAddress = new Map(stakes);

    const validators = [...addresses]
      .map((address) => ({
        address,
        registered: registered.some(([key]) => key.args[0].toString() === address),
        posScore: posScoreByAddress.get(address) ?? null,
        trustScore: trustByAddress.get(address) ?? null,
        inferenceCount: inferenceByAddress.get(address) ?? 0,
        stake: stakeByAddress.get(address) ?? null,
        blocksAuthoredInWindow: authored.get(address) ?? 0,
      }))
      .sort((a, b) => (b.trustScore ?? 0) - (a.trustScore ?? 0));

    return Response.json({
      validators,
      consensus: {
        epoch: toNumber(epoch),
        posWeightBps: toNumber(posWeight),
        poiWeightBps: toNumber(poiWeight),
      },
      window,
    });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 503 },
    );
  }
}
