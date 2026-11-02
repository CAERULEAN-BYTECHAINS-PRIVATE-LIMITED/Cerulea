/**
 * Fund the six demo persona accounts.
 *
 * The frontend maps each persona to a well-known development account (see
 * apps/web/src/lib/chain.ts PERSONA_ACCOUNTS): DPIIT=//Alice, MinistryAdmin=//Bob,
 * ProcuringEntity=//Charlie, Vendor=//Dave, Auditor=//Eve, CVC=//Ferdie.
 *
 * `--dev` builds genesis from `development_config_genesis`, which endows only Alice and
 * Bob. Charlie, Dave, Eve and Ferdie therefore start with a zero balance, and because
 * the runtime wires `pallet_transaction_payment` with `IdentityFee`, every extrinsic
 * they sign is rejected before any pallet logic runs:
 *
 *   1010: Invalid Transaction: Inability to pay some fees, e.g. account balance too low
 *
 * That surfaces as an opaque 500 from a trigger route, so it is fixed here once rather
 * than rediscovered per persona. Run this after starting a fresh `--dev` chain and
 * before seeding or driving the simulator.
 *
 * Idempotent: an account already above the target balance is skipped.
 *
 * Usage:
 *   npx tsx scripts/bootstrap/fund-personas.ts
 */

import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import { cryptoWaitReady } from '@polkadot/util-crypto';

const WS_ENDPOINT = process.env.CHAIN_WS_ENDPOINT ?? 'ws://127.0.0.1:9944';

/** Must mirror PERSONA_ACCOUNTS in apps/web/src/lib/chain.ts. */
const PERSONA_DEFAULTS: Record<string, string> = {
  MINISTRYADMIN: '//Bob',
  PROCURINGENTITY: '//Charlie',
  VENDOR: '//Dave',
  AUDITOR: '//Eve',
  CVC: '//Ferdie',
};
/** PERSONA_SURI_<ROLE> overrides, exactly as apps/web/src/lib/chain.ts reads them. */
const PERSONAS = Object.entries(PERSONA_DEFAULTS).map(
  ([role, fallback]) => process.env[`PERSONA_SURI_${role}`]?.trim() || fallback,
);

/** Generous enough that no demo run can exhaust it; far below Alice's genesis balance. */
const TARGET_BALANCE = 1_000_000_000_000_000n;

async function main() {
  await cryptoWaitReady();
  const api = await ApiPromise.create({
    provider: new WsProvider(WS_ENDPOINT),
    noInitWarn: true,
  });

  // ed25519 throughout: genesis is built from sp_keyring::Ed25519Keyring, so an sr25519
  // //Alice is a different AccountId32 that holds nothing.
  const keyring = new Keyring({ type: 'ed25519' });
  const alice = keyring.addFromUri('//Alice');

  console.log(`Connected to ${WS_ENDPOINT}`);
  console.log(`Funding from //Alice (${alice.address})\n`);

  for (const uri of PERSONAS) {
    const account = keyring.addFromUri(uri);
    const { data } = await api.query.system.account(account.address);
    const free = data.free.toBigInt();

    if (free >= TARGET_BALANCE) {
      console.log(`  ${uri.padEnd(10)} already funded (${free}), skipping`);
      continue;
    }

    const topUp = TARGET_BALANCE - free;
    await new Promise<void>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error(`transfer to ${uri} was not finalized within 60s`)),
        60_000,
      );
      // Callback must be synchronous: polkadot-js does not await a returned Promise,
      // and an async callback silently stalls the status subscription.
      api.tx.balances
        .transferKeepAlive(account.address, topUp)
        .signAndSend(alice, (result) => {
          if (result.dispatchError) {
            clearTimeout(timer);
            reject(new Error(`transfer to ${uri} failed: ${result.dispatchError.toString()}`));
            return;
          }
          if (result.status.isInvalid || result.status.isDropped || result.status.isUsurped) {
            clearTimeout(timer);
            reject(new Error(`transfer to ${uri} was ${result.status.type.toLowerCase()}`));
            return;
          }
          // Finality is instantaneous on this chain, so polkadot-js often reports
          // Finalized without ever emitting InBlock. Accept either.
          if (result.status.isInBlock || result.status.isFinalized) {
            clearTimeout(timer);
            resolve();
          }
        })
        .catch((err) => {
          clearTimeout(timer);
          reject(err);
        });
    });

    console.log(`  ${uri.padEnd(10)} funded with ${topUp}`);
  }

  console.log('\nVerifying every persona can pay fees...');
  let unfunded = 0;
  for (const uri of ['//Alice', ...PERSONAS]) {
    const account = keyring.addFromUri(uri);
    const { data } = await api.query.system.account(account.address);
    const free = data.free.toBigInt();
    const ok = free > 0n;
    if (!ok) unfunded++;
    console.log(`  ${ok ? 'OK  ' : 'FAIL'} ${uri.padEnd(10)} ${account.address} free=${free}`);
  }

  await api.disconnect();

  if (unfunded > 0) {
    console.error(`\n${unfunded} persona account(s) still hold nothing. Failing loudly.`);
    process.exit(1);
  }
  console.log('\nAll persona accounts funded.');
}

main().catch((err) => {
  console.error('Funding failed:', err);
  process.exit(1);
});
