/**
 * Deterministic pseudo-random number generation.
 *
 * Every generated GeM record — vendor names, GSTINs, bid numbers, quantities, EMD
 * figures, dates — comes out of one of these generators, and nothing in this simulator
 * ever calls `Math.random()` or `Date.now()` for record content. That is what makes
 * `--seed=<n>` a real guarantee: the same seed produces byte-identical records, so a
 * demo rehearsed on Monday shows the same bid numbers on Friday.
 *
 * Each scenario gets its own generator derived from `(seed, pathwayId)` rather than
 * drawing from one shared stream, so `--scenario=P7` produces exactly the records that
 * `--all` produces for P7. A shared stream would make a single scenario's records depend
 * on how many scenarios ran before it.
 */

/** mulberry32 — small, fast, well-distributed 32-bit PRNG. */
export class Rng {
  private state: number;

  constructor(seed: number) {
    // Normalise into uint32 space; seed 0 would otherwise produce a degenerate stream.
    this.state = (seed >>> 0) || 0x9e3779b9;
  }

  /** Uniform float in [0, 1). */
  next(): number {
    this.state = (this.state + 0x6d2b79f5) >>> 0;
    let t = this.state;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  }

  /** Uniform integer in [min, max] inclusive. */
  int(min: number, max: number): number {
    if (max < min) throw new RangeError(`Rng.int: max (${max}) < min (${min})`);
    return min + Math.floor(this.next() * (max - min + 1));
  }

  /** One element of a non-empty array. */
  pick<T>(items: readonly T[]): T {
    if (items.length === 0) throw new RangeError('Rng.pick: empty array');
    const chosen = items[this.int(0, items.length - 1)];
    // `noUncheckedIndexedAccess` cannot see that the index is in range.
    return chosen as T;
  }

  /** `count` distinct elements of `items`, in a stable shuffled order. */
  sample<T>(items: readonly T[], count: number): T[] {
    if (count > items.length) {
      throw new RangeError(`Rng.sample: asked for ${count} of ${items.length} items`);
    }
    return this.shuffle(items).slice(0, count);
  }

  /** Fisher-Yates, non-mutating. */
  shuffle<T>(items: readonly T[]): T[] {
    const copy = items.slice();
    for (let i = copy.length - 1; i > 0; i--) {
      const j = this.int(0, i);
      const a = copy[i] as T;
      const b = copy[j] as T;
      copy[i] = b;
      copy[j] = a;
    }
    return copy;
  }

  /** True with probability `p`. */
  chance(p: number): boolean {
    return this.next() < p;
  }

  /** A zero-padded decimal string of exactly `digits` characters. */
  digits(count: number): string {
    let out = '';
    for (let i = 0; i < count; i++) out += String(this.int(0, 9));
    return out;
  }

  /** `count` uppercase A-Z letters. */
  letters(count: number): string {
    const alphabet = 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
    let out = '';
    for (let i = 0; i < count; i++) out += alphabet.charAt(this.int(0, 25));
    return out;
  }

  /** `count` raw bytes. */
  bytes(count: number): Uint8Array {
    const out = new Uint8Array(count);
    for (let i = 0; i < count; i++) out[i] = this.int(0, 255);
    return out;
  }
}

/**
 * FNV-1a over a string, used to fold a scenario label into the CLI seed so each
 * scenario's stream is independent but still fully determined by `--seed`.
 */
export function hashLabel(label: string): number {
  let hash = 0x811c9dc5;
  for (let i = 0; i < label.length; i++) {
    hash ^= label.charCodeAt(i);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

/** The generator for one scenario: a pure function of the CLI seed and the pathway id. */
export function rngFor(seed: number, label: string): Rng {
  return new Rng(((seed >>> 0) ^ hashLabel(label)) >>> 0);
}
