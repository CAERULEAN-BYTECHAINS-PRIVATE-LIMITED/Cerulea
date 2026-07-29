/**
 * SS58 address encoding, implemented locally so the simulator has zero runtime
 * dependencies.
 *
 * Why the simulator needs this at all: every pramaan pallet takes the vendor as an
 * `AccountId`, and a vendor never signs anything — `classify`, `certify`, `debar` and
 * `calculate_preference` are all submitted by the procuring entity / ministry admin and
 * merely *name* the vendor. So a vendor identity only has to be a well-formed 32-byte
 * account id; it does not need a private key. That lets the simulator mint as many
 * distinct, deterministic vendor identities as a realistic tender needs, instead of
 * recycling the six `//Alice`-style dev accounts — which matters because debarment is
 * enforced vendor-wide and cross-ministry, so two scenarios sharing one address would
 * contaminate each other.
 *
 * Format (Substrate SS58): `base58( prefix_byte(s) || public_key || checksum[0..2] )`
 * where `checksum = blake2b-512("SS58PRE" || prefix_bytes || public_key)`.
 * Prefix 42 is the generic Substrate prefix this chain's dev chainspec uses.
 */

import { createHash } from 'node:crypto';

const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
const SS58_PREFIX_SALT = Buffer.from('SS58PRE', 'utf8');

/** The generic Substrate address prefix, which the dev chainspec in this repo uses. */
export const GENERIC_SUBSTRATE_PREFIX = 42;

function base58Encode(bytes: Uint8Array): string {
  if (bytes.length === 0) return '';

  // Standard big-integer base conversion over the byte array.
  const digits: number[] = [0];
  for (const byte of bytes) {
    let carry = byte;
    for (let i = 0; i < digits.length; i++) {
      const value = (digits[i] as number) * 256 + carry;
      digits[i] = value % 58;
      carry = (value / 58) | 0;
    }
    while (carry > 0) {
      digits.push(carry % 58);
      carry = (carry / 58) | 0;
    }
  }

  let out = '';
  // Leading zero bytes map to leading '1' characters.
  for (const byte of bytes) {
    if (byte !== 0) break;
    out += BASE58_ALPHABET.charAt(0);
  }
  for (let i = digits.length - 1; i >= 0; i--) {
    out += BASE58_ALPHABET.charAt(digits[i] as number);
  }
  return out;
}

function blake2b512(input: Buffer): Buffer {
  try {
    return createHash('blake2b512').update(input).digest();
  } catch {
    throw new Error(
      'This Node build does not expose the blake2b512 digest, which SS58 address ' +
        'encoding requires. Node 18+ built against OpenSSL 1.1.1 or later provides it; ' +
        'check `node -e "require(\'crypto\').createHash(\'blake2b512\')"`.',
    );
  }
}

/** Encode a 32-byte public key as an SS58 address. */
export function encodeSs58(publicKey: Uint8Array, prefix: number = GENERIC_SUBSTRATE_PREFIX): string {
  if (publicKey.length !== 32) {
    throw new RangeError(`encodeSs58: expected a 32-byte public key, got ${publicKey.length}`);
  }
  if (prefix > 63) {
    // Two-byte prefixes exist in the wider SS58 registry; this chain does not use one,
    // so refuse rather than silently emitting a malformed address.
    throw new RangeError(`encodeSs58: only single-byte prefixes (0-63) are supported, got ${prefix}`);
  }

  const prefixBytes = Buffer.from([prefix]);
  const payload = Buffer.concat([prefixBytes, Buffer.from(publicKey)]);
  const checksum = blake2b512(Buffer.concat([SS58_PREFIX_SALT, payload])).subarray(0, 2);
  return base58Encode(Buffer.concat([payload, checksum]));
}

/**
 * A stable 32-byte account id for a named participant.
 *
 * Derived from `blake2b-512(seed || ':' || label)` truncated to 32 bytes, so:
 *   - the same `(seed, label)` always yields the same address (reproducible demos), and
 *   - changing `--seed` re-mints every identity (so two people demoing on one shared
 *     chain do not collide).
 */
export function accountIdFor(seed: number, label: string): string {
  const digest = blake2b512(Buffer.from(`cbc-pramaan:${seed >>> 0}:${label}`, 'utf8'));
  return encodeSs58(digest.subarray(0, 32));
}
