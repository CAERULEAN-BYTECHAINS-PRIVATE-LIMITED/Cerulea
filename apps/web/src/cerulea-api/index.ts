/**
 * `@cerulea/api` — the client library for the Cerulea network.
 *
 * Everything in the CBC-PRAMAAN frontend that touches the chain goes through this
 * module. It is a thin, deliberate layer over `@polkadot/api`, which remains the
 * transport and SCALE codec underneath (and is declared as a dependency — this package
 * does not reimplement either). What it adds is the part that is specific to Cerulea and
 * that cost real debugging to get right:
 *
 *   1. `encodeId` — the one correct way to pass a `BoundedVec<u8, N>` to a Cerulea
 *      pallet. Getting this wrong fails at SCALE-encoding time with an opaque error, and
 *      two of the three plausible spellings are wrong in different ways.
 *   2. `CERULEA_SS58_FORMAT` / `CERULEA_CRYPTO_TYPE` — the chain's actual address format
 *      and signature scheme. The scheme is ed25519, not the sr25519 that most Substrate
 *      tooling defaults to, because Cerulea's genesis is built from an ed25519 keyring.
 *   3. `PRAMAAN_PALLETS` — the runtime's pallet accessor names, so a typo becomes a type
 *      error rather than an `undefined is not a function` at request time.
 *   4. `isFinalizedStatus` — the finality-status predicate. Cerulea finalizes a block as
 *      soon as it is authored, and in that regime polkadot-js frequently reports
 *      `Finalized` WITHOUT ever emitting `InBlock`. Code that waits only on `InBlock`
 *      hangs until its timeout on a transaction that finalized in ~200ms.
 *
 * Re-exporting the polkadot primitives from here means call sites import from
 * `@cerulea/api` rather than reaching past it, so this stays the single seam where the
 * underlying client could be swapped or extended.
 */

import { ApiPromise, WsProvider, Keyring } from '@polkadot/api';
import type { ISubmittableResult } from '@polkadot/types/types';
import { stringToU8a, u8aToHex } from '@polkadot/util';
import { cryptoWaitReady } from '@polkadot/util-crypto';

// --- Re-exported transport primitives ------------------------------------------------
// Call sites import these from '@cerulea/api', never from '@polkadot/*' directly.
export { ApiPromise, WsProvider, Keyring };
export { cryptoWaitReady, decodeAddress, encodeAddress } from '@polkadot/util-crypto';
export { hexToU8a, stringToU8a, u8aToHex, u8aToString } from '@polkadot/util';
export type { SubmittableExtrinsic } from '@polkadot/api/types';
export type { KeyringPair } from '@polkadot/keyring/types';
export type { ISubmittableResult } from '@polkadot/types/types';

/** Cerulea's SS58 registry prefix (`ss58Format` in the chain properties). */
export const CERULEA_SS58_FORMAT = 42;

/**
 * The signature scheme Cerulea accounts use.
 *
 * ed25519, not sr25519. `cerulea-runtime`'s genesis presets are built from
 * `sp_keyring::Ed25519Keyring` — the validators, the sudo key, and every endowed
 * account. An `AccountId32` is derived from a specific public key, so an sr25519
 * `//Alice` is a completely different account that is neither root nor funded, and every
 * extrinsic it signs is rejected for want of fees before any pallet logic runs.
 */
export const CERULEA_CRYPTO_TYPE = 'ed25519' as const;

/** Default endpoint; overridden by `CHAIN_WS_ENDPOINT` in any deployed environment. */
export const DEFAULT_ENDPOINT = 'ws://127.0.0.1:9944';

/**
 * The CBC-PRAMAAN pallet accessor names as they appear on `api.tx` / `api.query`.
 *
 * The runtime aliases each pallet in its `#[frame_support::runtime]` block (e.g.
 * `pub type PramaanRuleRegistry = pallet_pramaan_rule_registry;`), and polkadot-js
 * camel-cases the alias — NOT the crate name. Spelling it `palletPramaanRuleRegistry`
 * yields `undefined` at call time, which is how it was first got wrong.
 */
export const PRAMAAN_PALLETS = {
  ruleRegistry: 'pramaanRuleRegistry',
  classification: 'pramaanClassification',
  preference: 'pramaanPreference',
  certification: 'pramaanCertification',
  debarment: 'pramaanDebarment',
  consistency: 'pramaanConsistency',
} as const;

export type PramaanPallet = (typeof PRAMAAN_PALLETS)[keyof typeof PRAMAAN_PALLETS];

/**
 * Encode a human-readable identifier for a `BoundedVec<u8, N>` argument.
 *
 * Returns a 0x-prefixed hex string, which is the ONLY spelling correct for every input.
 * Verified against a running node:
 *
 *   - a plain string works, EXCEPT that polkadot-js reads any value beginning `0x` as
 *     hex, so the six-character tender id `"0x1234"` silently encodes as two bytes
 *   - a raw `Uint8Array` THROWS: polkadot-js expects a `Bytes` argument to already carry
 *     its compact length prefix, so it reads the first byte as a length. Observed as
 *     "Compact input is > Number.MAX_SAFE_INTEGER" and "required length less than
 *     remainder, expected at least 18, found 5"
 *   - hex round-trips to exactly these bytes, whatever the input looks like
 *
 * Callers that need to enforce the pallet's `BoundedVec` limit should measure
 * `stringToU8a(value).length`, since the bound applies to the UTF-8 byte length.
 */
export function encodeId(value: string): string {
  return u8aToHex(stringToU8a(value));
}

/** Byte length an id will occupy on chain, for checking against a `BoundedVec` bound. */
export function idByteLength(value: string): number {
  return stringToU8a(value).length;
}

/**
 * Has this transaction reached a block?
 *
 * Accepts `Finalized` as well as `InBlock`. On Cerulea a block is finalized as soon as
 * it is authored, and polkadot-js then commonly reports `Finalized` without ever having
 * emitted `InBlock` for the extrinsic — so a predicate that checks only `InBlock` will
 * wait out its entire timeout on a transaction that succeeded in ~200ms. This is a
 * failure mode a slow-finality chain never exhibits, which is exactly why it belongs in
 * the client library rather than in each caller.
 */
export function isFinalizedStatus(result: ISubmittableResult): boolean {
  return result.status.isInBlock || result.status.isFinalized;
}

/** The block hash a transaction landed in, whichever status reported it. */
export function blockHashOf(result: ISubmittableResult): string | undefined {
  if (result.status.isInBlock) return result.status.asInBlock.toHex();
  if (result.status.isFinalized) return result.status.asFinalized.toHex();
  return undefined;
}

/** Terminal states in which a transaction will never be included. */
export function isRejectedStatus(result: ISubmittableResult): boolean {
  return result.status.isInvalid || result.status.isDropped || result.status.isUsurped;
}

/**
 * Connect to a Cerulea node.
 *
 * `noInitWarn` is set because this runtime intentionally exposes a narrower RPC surface
 * than polkadot-js's defaults expect, and the resulting warning is noise rather than
 * signal.
 */
export async function connect(endpoint: string = DEFAULT_ENDPOINT): Promise<ApiPromise> {
  const api = await ApiPromise.create({
    provider: new WsProvider(endpoint),
    noInitWarn: true,
  });
  await api.isReady;
  return api;
}

/** A keyring using Cerulea's signature scheme and address format. */
export async function createKeyring(): Promise<Keyring> {
  await cryptoWaitReady();
  return new Keyring({ type: CERULEA_CRYPTO_TYPE, ss58Format: CERULEA_SS58_FORMAT });
}
