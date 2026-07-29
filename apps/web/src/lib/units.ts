/**
 * Unit conversion between what a human reads and what the chain stores.
 *
 * Two conventions run through this entire build (see docs/PRAMAAN_BUILD_CONTRACT.md):
 *   - money  : paise, the smallest currency unit, as an integer
 *   - percent: basis points, u16, where BPS_DENOMINATOR = 10_000
 *
 * Every value crossing the API boundary must be converted here rather than inline, so
 * there is exactly one place a rupee-vs-paise mistake can be made -- the class of bug
 * that was already caught once between two independently-written pallets.
 */

export const BPS_DENOMINATOR = 10_000;
export const PAISE_PER_RUPEE = 100n;

/** Rs 10 crore, the DPIIT default certification threshold, in paise. */
export const RS_10_CRORE_PAISE = 100_000_000_00n;
/** Rs 5 lakh, the DPIIT default exemption floor, in paise. */
export const RS_5_LAKH_PAISE = 500_000_00n;
/** Rs 200 crore, the domestic-tender gate in PathwayId::P5, in paise. */
export const RS_200_CRORE_PAISE = 200_000_000_000n;

/** Rupees (may be fractional) -> paise. Rounds to the nearest paisa. */
export function rupeesToPaise(rupees: number): bigint {
  if (!Number.isFinite(rupees) || rupees < 0) {
    throw new RangeError(`rupeesToPaise: expected a non-negative finite number, got ${rupees}`);
  }
  return BigInt(Math.round(rupees * 100));
}

/** Paise -> rupees as a number. Display only; never send the result to a pallet. */
export function paiseToRupees(paise: bigint | string | number): number {
  return Number(BigInt(paise)) / 100;
}

/**
 * Paise formatted the way an Indian procurement officer expects to read it:
 * crore / lakh, with the Indian digit grouping.
 */
export function formatPaise(paise: bigint | string | number): string {
  const rupees = BigInt(paise) / PAISE_PER_RUPEE;
  const CRORE = 10_000_000n;
  const LAKH = 100_000n;

  if (rupees >= CRORE) {
    return `₹${trimZeros(Number(rupees) / 1e7)} crore`;
  }
  if (rupees >= LAKH) {
    return `₹${trimZeros(Number(rupees) / 1e5)} lakh`;
  }
  return `₹${rupees.toLocaleString('en-IN')}`;
}

function trimZeros(n: number): string {
  return n.toFixed(2).replace(/\.00$/, '').replace(/(\.\d)0$/, '$1');
}

/** Percent (0-100, may be fractional) -> basis points. 19.99% -> 1999. */
export function percentToBps(percent: number): number {
  if (!Number.isFinite(percent) || percent < 0 || percent > 100) {
    throw new RangeError(`percentToBps: expected 0-100, got ${percent}`);
  }
  return Math.round(percent * 100);
}

/** Basis points -> percent. 1999 -> 19.99. */
export function bpsToPercent(bps: number): number {
  return bps / 100;
}

/** Basis points as a display string. 5000 -> "50%", 1999 -> "19.99%". */
export function formatBps(bps: number): string {
  return `${trimZeros(bpsToPercent(bps))}%`;
}
