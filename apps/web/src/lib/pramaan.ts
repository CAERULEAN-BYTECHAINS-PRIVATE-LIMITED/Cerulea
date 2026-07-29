/**
 * Typed helpers shared by the six trigger-point routes (Technical Implementation
 * Specification Part 8.2, docs/PRAMAAN_BUILD_CONTRACT.md section 3).
 *
 * Three things live here and nowhere else:
 *
 *   1. Request validation, so a malformed body is a 400 with a sentence a human can act
 *      on rather than a 500 from deep inside SCALE encoding.
 *   2. The GREEN / YELLOW / RED decision logic. It is centralised here, not duplicated
 *      per route, because the tri-state is the PoC's headline claim and there must be
 *      exactly one place where its meaning is defined.
 *   3. Error mapping. A pallet *rejecting* a bid is a legitimate compliance answer, not
 *      a server fault, so `ExtrinsicFailedError` becomes an HTTP 200 RED quoting the
 *      pallet's own error name. Only genuine infrastructure faults are 5xx.
 *
 * Everything that touches the chain goes through `src/lib/chain.ts`; every currency or
 * percentage conversion goes through `src/lib/units.ts`. Neither is re-implemented here.
 */

import type { ApiPromise } from '@polkadot/api';
import type { SubmittableExtrinsic } from '@polkadot/api/types';
import type { ISubmittableResult } from '@polkadot/types/types';
import { hexToU8a, stringToU8a, u8aToHex, u8aToString } from '@polkadot/util';
import { decodeAddress } from '@polkadot/util-crypto';

import {
  addressOf,
  decodeDispatchError,
  ExtrinsicFailedError,
  FinalityTimeoutError,
  findEvent,
  getApi,
  PERSONA_ACCOUNTS,
  type FinalizedResult,
  type Persona,
} from './chain';
import { BPS_DENOMINATOR, formatBps, formatPaise } from './units';

// ---------------------------------------------------------------------------------
// Domain vocabulary
// ---------------------------------------------------------------------------------

/**
 * The tri-state every trigger point answers with (build contract section 3):
 *   GREEN  — compliant, proceeds.
 *   YELLOW — proceeds with a caveat, or needs a human.
 *   RED    — blocked. The `reason` must name the rule or debarment that blocked it.
 */
export type TriState = 'GREEN' | 'YELLOW' | 'RED';

/** `pallet_pramaan_classification::ClassResult`, verbatim from its source. */
export type ClassResult = 'ClassOne' | 'ClassTwo' | 'NonLocal' | 'ManualReviewRequired';

/** `pramaan_primitives::CalculationMethod`. */
export const CALCULATION_METHODS = [
  'Standard',
  'ComponentLevel',
  'WeightedModule',
  'Custom',
] as const;
export type CalculationMethod = (typeof CALCULATION_METHODS)[number];

/** `pramaan_primitives::Divisibility`. */
export const DIVISIBILITIES = ['Divisible', 'NonDivisible'] as const;
export type Divisibility = (typeof DIVISIBILITIES)[number];

/** `pallet_pramaan_preference::ClassResultLike` — the three-variant bid class. */
export const BID_CLASSES = ['ClassOne', 'ClassTwo', 'NonLocal'] as const;
export type BidClass = (typeof BID_CLASSES)[number];

/**
 * `pramaan_primitives::IdBound` is 64, so every MinistryId / TenderId / ProductId /
 * CertificateId / HsnCode is a `BoundedVec<u8, 64>`. Exceeding it fails at SCALE
 * encoding time with an opaque error, so it is checked at the API boundary instead.
 */
export const ID_MAX_BYTES = 64;

/** `pallet_pramaan_debarment::ReasonBound` is 256 bytes. */
export const DEBARMENT_REASON_MAX_BYTES = 256;

/** `pramaan_primitives::MaxHsn` — at most 64 per-HSN threshold rows in one rule. */
export const MAX_HSN_THRESHOLDS = 64;

/** Runtime config: `pallet_pramaan_preference::Config::MaxBids = ConstU32<128>`. */
export const MAX_BIDS = 128;

/** Runtime config: `pallet_pramaan_classification::Config::MaxComponents = ConstU32<64>`. */
export const MAX_COMPONENTS = 64;

// ---------------------------------------------------------------------------------
// Request validation
// ---------------------------------------------------------------------------------

/** Thrown by every validator below; `handleTrigger` turns it into an HTTP 400. */
export class BadRequestError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BadRequestError';
  }
}

/** Parse the request body and insist it is a JSON object, not an array or a scalar. */
export async function readJsonObject(request: Request): Promise<Record<string, unknown>> {
  let parsed: unknown;
  try {
    parsed = await request.json();
  } catch {
    throw new BadRequestError('Request body must be valid JSON.');
  }
  if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
    throw new BadRequestError('Request body must be a JSON object.');
  }
  return parsed as Record<string, unknown>;
}

/**
 * Read a field under its canonical name, falling back to legacy aliases. The build
 * contract's route table (section 3) names some money fields without the `Paise`
 * suffix that section 1 mandates ("label the field `...Paise` in any JSON that carries
 * one"); accepting both keeps the documented shape working while the canonical,
 * unit-explicit name is what the routes actually document and return.
 */
function pick(body: Record<string, unknown>, name: string, ...aliases: string[]): unknown {
  for (const key of [name, ...aliases]) {
    if (body[key] !== undefined && body[key] !== null) return body[key];
  }
  return undefined;
}

/**
 * A `BoundedVec<u8, 64>` identifier, returned as a 0x-prefixed HEX STRING.
 *
 * Hex — not a plain string, not a Uint8Array. All three were tried against the live
 * chain and only hex is correct for every input:
 *
 *   - plain string  works, EXCEPT `@polkadot/api` reads any value beginning "0x" as
 *                   hex, so a tender id like "0x1234" silently encodes as 2 bytes, not 6
 *   - Uint8Array    THROWS. polkadot-js expects a `Bytes` argument to already carry its
 *                   compact length prefix, so a bare byte array is misread as one.
 *                   Observed live as "Compact input is > Number.MAX_SAFE_INTEGER"
 *   - hex string    unambiguous, and round-trips to exactly these bytes
 *
 * The length bound is still checked against the UTF-8 encoding, since that is what the
 * pallet's `BoundedVec` limit actually applies to.
 */
export function requireId(body: Record<string, unknown>, field: string): string {
  const value = body[field];
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new BadRequestError(`"${field}" is required and must be a non-empty string.`);
  }
  const bytes = stringToU8a(value.trim());
  if (bytes.length > ID_MAX_BYTES) {
    throw new BadRequestError(
      `"${field}" must be at most ${ID_MAX_BYTES} bytes when UTF-8 encoded; got ${bytes.length}.`,
    );
  }
  return u8aToHex(bytes);
}

/** The same value as `requireId`, but as the original string for use in a `reason`. */
export function requireIdText(body: Record<string, unknown>, field: string): string {
  requireId(body, field);
  return (body[field] as string).trim();
}

/** Basis points: an integer in [0, 10000]. See `units.ts` for the conversion helpers. */
export function requireBps(body: Record<string, unknown>, field: string): number {
  const value = body[field];
  if (typeof value !== 'number' || !Number.isInteger(value)) {
    throw new BadRequestError(
      `"${field}" is required and must be an integer number of basis points (10000 = 100%).`,
    );
  }
  if (value < 0 || value > BPS_DENOMINATOR) {
    throw new BadRequestError(
      `"${field}" must be between 0 and ${BPS_DENOMINATOR} basis points; got ${value}.`,
    );
  }
  return value;
}

export function optionalBoolean(
  body: Record<string, unknown>,
  field: string,
  fallback: boolean,
): boolean {
  const value = body[field];
  if (value === undefined || value === null) return fallback;
  if (typeof value !== 'boolean') {
    throw new BadRequestError(`"${field}", when supplied, must be a boolean.`);
  }
  return value;
}

/**
 * A currency amount in paise (build contract section 1). Accepts a JS number or a
 * decimal string; a string is the safe form for anything above 2^53 paise (Rs 90,071
 * crore), which is well inside the u128 range the pallets use.
 */
export function requirePaise(
  body: Record<string, unknown>,
  field: string,
  ...aliases: string[]
): bigint {
  const value = pick(body, field, ...aliases);
  const parsed = coerceUnsignedInteger(value);
  if (parsed === null) {
    throw new BadRequestError(
      `"${field}" is required and must be a non-negative whole number of paise ` +
        `(as a number or a decimal string). Never send rupees to this API.`,
    );
  }
  return parsed;
}

/** A block number: `BlockNumber` is `u32` in this runtime. */
export function requireBlockNumber(body: Record<string, unknown>, field: string): number {
  const value = body[field];
  if (typeof value !== 'number' || !Number.isInteger(value) || value < 0 || value > 0xffff_ffff) {
    throw new BadRequestError(`"${field}" is required and must be a block number (a u32 integer).`);
  }
  return value;
}

export function optionalBlockNumber(
  body: Record<string, unknown>,
  field: string,
): number | undefined {
  if (body[field] === undefined || body[field] === null) return undefined;
  return requireBlockNumber(body, field);
}

/** A string field constrained to a closed set of variants (an on-chain enum). */
export function requireEnum<T extends string>(
  body: Record<string, unknown>,
  field: string,
  variants: readonly T[],
): T {
  const value = body[field];
  if (typeof value !== 'string' || !(variants as readonly string[]).includes(value)) {
    throw new BadRequestError(`"${field}" must be one of: ${variants.join(', ')}.`);
  }
  return value as T;
}

/**
 * Resolve a caller-supplied account reference to an SS58 address.
 *
 * Accepts a demo persona name (`vendor`, `auditor`, ...), a raw derivation URI
 * (`//Dave`), or an SS58 address. The SS58 branch deliberately does NOT go through
 * `addressOf`: that helper treats an unknown string as a derivation URI, so passing an
 * SS58 address to it would silently derive a brand new, different account.
 */
export async function resolveAccount(
  body: Record<string, unknown>,
  field: string,
): Promise<string> {
  const value = body[field];
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new BadRequestError(
      `"${field}" is required and must be an SS58 address, a demo persona ` +
        `(${Object.keys(PERSONA_ACCOUNTS).join(', ')}), or a //Derivation URI.`,
    );
  }
  return resolveAccountValue(value.trim(), field);
}

/** Same as `resolveAccount` but for a value already pulled out of a nested object. */
export async function resolveAccountValue(value: string, label: string): Promise<string> {
  if (value in PERSONA_ACCOUNTS) return addressOf(value as Persona);
  if (value.startsWith('//')) return addressOf(value);
  try {
    decodeAddress(value);
  } catch {
    throw new BadRequestError(
      `"${label}" is not a valid SS58 address, demo persona, or //Derivation URI: ${value}`,
    );
  }
  return value;
}

/** Optional account field; returns `null` (SCALE `None`) when absent. */
export async function optionalAccount(
  body: Record<string, unknown>,
  field: string,
): Promise<string | null> {
  if (body[field] === undefined || body[field] === null) return null;
  return resolveAccount(body, field);
}

/**
 * UTF-8 encode free text with an explicit byte bound (e.g. a debarment reason), as a
 * 0x-prefixed hex string. Same encoding rule as `requireId` — see its doc comment for
 * why a bare Uint8Array cannot be passed to a `Bytes` argument.
 */
export function encodeBoundedText(value: string, maxBytes: number, field: string): string {
  const bytes = stringToU8a(value);
  if (bytes.length > maxBytes) {
    throw new BadRequestError(
      `"${field}" must be at most ${maxBytes} bytes when UTF-8 encoded; got ${bytes.length}.`,
    );
  }
  return u8aToHex(bytes);
}

// ---------------------------------------------------------------------------------
// Reading values back out of chain data
// ---------------------------------------------------------------------------------

/**
 * Coerce a value that may arrive as a JS number, a decimal string, a comma-grouped
 * string (which is what `Codec.toHuman()` produces for integers, e.g. `"1,000,000"`),
 * or a hex string (which is what `toJSON()` produces for u128 values above 2^53).
 * Returns `null` when the value is not an unsigned integer in any of those forms.
 */
export function coerceUnsignedInteger(value: unknown): bigint | null {
  // `BigInt(0)` rather than the `0n` literal: this project's tsconfig targets ES2017,
  // where BigInt literals are a compile error even though the BigInt runtime is present.
  if (typeof value === 'bigint') return value >= BigInt(0) ? value : null;
  if (typeof value === 'number') {
    if (!Number.isInteger(value) || value < 0) return null;
    return BigInt(value);
  }
  if (typeof value === 'string') {
    const trimmed = value.trim().replace(/,/g, '');
    if (/^0x[0-9a-fA-F]+$/.test(trimmed)) return BigInt(trimmed);
    if (/^\d+$/.test(trimmed)) return BigInt(trimmed);
  }
  return null;
}

/**
 * Decode a `BoundedVec<u8, _>` as it appears in `toJSON()` output. polkadot-js renders
 * a byte vector either as a `0x`-prefixed hex string or as an array of byte values
 * depending on how the type was registered from metadata, so both are handled.
 */
export function decodeByteVec(value: unknown): string {
  if (typeof value === 'string') {
    return value.startsWith('0x') ? u8aToString(hexToU8a(value)) : value;
  }
  if (Array.isArray(value) && value.every((byte) => typeof byte === 'number')) {
    return u8aToString(Uint8Array.from(value as number[]));
  }
  return '';
}

/**
 * Read one field out of an event emitted by the just-finalized extrinsic.
 *
 * Results are read from events rather than by re-querying storage so the value
 * provably came from the finalized block rather than from whatever the chain state
 * happens to be by the time the response is written.
 */
export function requireEventField(
  result: FinalizedResult,
  section: string,
  method: string,
  field: string,
): unknown {
  const event = findEvent(result, section, method);
  if (!event) {
    throw new Error(
      `Extrinsic ${result.txRef} finalized in block #${result.blockNumber} but emitted no ` +
        `${section}.${method} event, so its result cannot be read back.`,
    );
  }
  return event[field];
}

/**
 * Every event of one kind emitted by the finalized extrinsic, not just the first.
 *
 * `findEvent` returns a single match, which is enough for events a pallet emits at most
 * once. `pallet_pramaan_consistency` deliberately emits one `InconsistencyFlagged` per
 * contradicting prior declaration — "one event per concrete pair of inconsistent
 * tenders", per its own module doc — so reading only the first would under-report how
 * many earlier declarations a new one contradicts.
 */
export function findAllEvents(
  result: FinalizedResult,
  section: string,
  method: string,
): Record<string, unknown>[] {
  return result.events
    .filter(({ event }) => event.section === section && event.method === method)
    .map(({ event }) => event.data.toHuman() as Record<string, unknown>);
}

/** Assert an event was emitted, without reading a particular field out of it. */
export function requireEvent(
  result: FinalizedResult,
  section: string,
  method: string,
): Record<string, unknown> {
  const event = findEvent(result, section, method);
  if (!event) {
    throw new Error(
      `Extrinsic ${result.txRef} finalized in block #${result.blockNumber} but emitted no ` +
        `${section}.${method} event, so its result cannot be read back.`,
    );
  }
  return event;
}

/**
 * `pallet_sudo` swallows the inner call's dispatch error: the outer extrinsic succeeds
 * and the failure is reported as `Err` inside the `sudo.Sudid` event. Without this
 * check a rejected rule update would be reported as a success. Called by every route
 * that dispatches through sudo.
 */
export function assertSudoInnerCallSucceeded(api: ApiPromise, result: FinalizedResult): void {
  const record = result.events.find(
    ({ event }) => event.section === 'sudo' && event.method === 'Sudid',
  );
  if (!record) return;

  // `Sudid` carries a single field, `sudoResult: Result<(), DispatchError>`. Reading it
  // off the raw codec rather than `toHuman()` is what makes `decodeDispatchError` able
  // to name the pallet error (`InvalidRule`) instead of an opaque module/index pair.
  const sudoResult = record.event.data[0] as unknown as SudoDispatchResult | undefined;
  if (sudoResult?.isErr) {
    throw new ExtrinsicFailedError(result.txRef, decodeDispatchError(api, sudoResult.asErr));
  }
}

type ChainDispatchError = NonNullable<ISubmittableResult['dispatchError']>;

interface SudoDispatchResult {
  readonly isErr: boolean;
  readonly asErr: ChainDispatchError;
}

// ---------------------------------------------------------------------------------
// Chain reads that inform a decision but are not themselves the decision
// ---------------------------------------------------------------------------------

/** One active debarment record, decoded for use in a RED sentence. */
export interface ActiveDebarment {
  ministry: string;
  reason: string;
  effectiveFrom: number;
  effectiveTo: number | null;
}

interface DebarmentRecordJson {
  ministry?: unknown;
  effectiveFrom?: unknown;
  effectiveTo?: unknown;
  reason?: unknown;
}

/**
 * Read `pramaanDebarment.debarments(vendor)` and return the first record that is
 * active right now, or `null`.
 *
 * "Active" is the pallet's own definition (pallet-pramaan-debarment `is_active`):
 * `effective_from <= now && (effective_to.is_none() || effective_to > now)`.
 *
 * Enforcement is deliberately NOT scoped to a ministry: `DebarmentCheck::is_debarred`
 * in the pallet ignores its `ministry` argument and returns true for any active record
 * from any ministry (PoC document Table 8, "one shared ledger"). This read reproduces
 * that exactly, so the pre-check and the on-chain check can never disagree.
 *
 * Both the storage read and the "now" it is compared against are taken at the same
 * block hash, so the answer is a single consistent snapshot rather than two reads that
 * could straddle a block boundary.
 */
export async function readActiveDebarment(
  api: ApiPromise,
  vendorAddress: string,
): Promise<ActiveDebarment | null> {
  const header = await api.rpc.chain.getHeader();
  const now = header.number.toNumber();
  const at = await api.at(header.hash);

  const raw = await at.query.pramaanDebarment.debarments(vendorAddress);
  const records = raw.toJSON();
  if (!Array.isArray(records)) return null;

  for (const entry of records as DebarmentRecordJson[]) {
    if (typeof entry !== 'object' || entry === null) continue;

    const effectiveFrom = coerceUnsignedInteger(entry.effectiveFrom);
    if (effectiveFrom === null) continue;
    const effectiveTo =
      entry.effectiveTo === null || entry.effectiveTo === undefined
        ? null
        : coerceUnsignedInteger(entry.effectiveTo);

    const nowBig = BigInt(now);
    const started = effectiveFrom <= nowBig;
    const notYetEnded = effectiveTo === null || effectiveTo > nowBig;
    if (!started || !notYetEnded) continue;

    return {
      ministry: decodeByteVec(entry.ministry),
      reason: decodeByteVec(entry.reason),
      effectiveFrom: Number(effectiveFrom),
      effectiveTo: effectiveTo === null ? null : Number(effectiveTo),
    };
  }
  return null;
}

/** The nine rule parameters as read back off the chain, in the units the chain uses. */
export interface EffectiveRule {
  certificationThresholdPaise: bigint;
  exemptionFloorPaise: bigint;
  preferenceMarginBps: number;
  calculationMethod: string;
  divisibility: string;
  para3aApplicable: boolean;
  pliLinked: boolean;
}

/**
 * `Rules[ministry]`, falling back to `DefaultRule` — the same resolution order as the
 * pallet's own `get_effective_rule`. Used only to decide things the emitted events do
 * not carry (whether a certification is above its ministry's auditor threshold), never
 * to report an outcome the chain already stated.
 */
export async function readEffectiveRule(
  api: ApiPromise,
  ministry: string,
): Promise<EffectiveRule | null> {
  const specific = await api.query.pramaanRuleRegistry.rules(ministry);
  let json = specific.toJSON();
  if (json === null || json === undefined) {
    json = (await api.query.pramaanRuleRegistry.defaultRule()).toJSON();
  }
  if (typeof json !== 'object' || json === null || Array.isArray(json)) return null;

  const rule = json as Record<string, unknown>;
  const certificationThreshold = coerceUnsignedInteger(rule.certificationThreshold);
  const exemptionFloor = coerceUnsignedInteger(rule.exemptionFloor);
  const preferenceMargin = coerceUnsignedInteger(rule.preferenceMarginBps);
  if (certificationThreshold === null || exemptionFloor === null || preferenceMargin === null) {
    return null;
  }

  return {
    certificationThresholdPaise: certificationThreshold,
    exemptionFloorPaise: exemptionFloor,
    preferenceMarginBps: Number(preferenceMargin),
    calculationMethod: String(rule.calculationMethod ?? 'Standard'),
    divisibility: String(rule.divisibility ?? 'Divisible'),
    para3aApplicable: rule.para3aApplicable === true,
    pliLinked: rule.pliLinked === true,
  };
}

// ---------------------------------------------------------------------------------
// The tri-state decision logic
// ---------------------------------------------------------------------------------

/**
 * Classification outcome -> tri-state, per build contract section 3:
 *   ClassOne              GREEN  — compliant and proceeds.
 *   ClassTwo              YELLOW — proceeds, but without Class-I purchase preference.
 *   ManualReviewRequired  YELLOW — needs a human (PathwayId::P3).
 *   NonLocal              RED    — blocked by the Class-II threshold.
 */
export function classToTriState(classResult: ClassResult): TriState {
  switch (classResult) {
    case 'ClassOne':
      return 'GREEN';
    case 'ClassTwo':
    case 'ManualReviewRequired':
      return 'YELLOW';
    case 'NonLocal':
      return 'RED';
  }
}

/** One plain sentence explaining a classification, readable aloud by a non-technical judge. */
export function describeClass(classResult: ClassResult, declaredBps: number): string {
  const declared = formatBps(declaredBps);
  switch (classResult) {
    case 'ClassOne':
      return `Compliant: the declared local content of ${declared} meets this ministry's Class-I threshold, so the vendor bids as a Class-I local supplier.`;
    case 'ClassTwo':
      return `Proceeds with a caveat: the declared local content of ${declared} meets only the Class-II threshold, so the vendor may bid but does not receive Class-I purchase preference.`;
    case 'ManualReviewRequired':
      return `Needs a human: this ministry's rule uses a Custom calculation method for which no automatable formula exists, so the classification has been routed to a recorded human decision.`;
    case 'NonLocal':
      return `Blocked: the declared local content of ${declared} falls below this ministry's Class-II threshold under the Make in India order, so the vendor is Non-local and cannot bid on this tender.`;
  }
}

/**
 * One plain sentence naming an active debarment, for the bid-evaluation RED. Names the
 * debarring ministry and quotes the reason recorded on chain, because a RED must say
 * which rule or debarment blocked the bid.
 */
export function describeDebarment(debarment: ActiveDebarment): string {
  const ministry = debarment.ministry || 'a nodal ministry';
  const reason = debarment.reason ? ` The recorded reason is: ${debarment.reason}.` : '';
  const until =
    debarment.effectiveTo === null
      ? 'with no end date recorded'
      : `until block ${debarment.effectiveTo}`;
  return (
    `Blocked: this vendor is under an active debarment recorded by ${ministry} from block ` +
    `${debarment.effectiveFrom} ${until}, and the shared national ledger enforces it across ` +
    `every ministry.${reason}`
  );
}

/**
 * Pallet error name -> one plain sentence. `ExtrinsicFailedError.palletError` carries
 * the pallet's own error name (`decodeDispatchError` resolves it through the chain
 * metadata), so a RED quotes the chain's answer rather than an answer we invented.
 */
const PALLET_ERROR_SENTENCES: Record<string, string> = {
  // pallet-pramaan-classification
  VendorDebarred:
    'Blocked: this vendor has an active debarment on the shared national ledger, which applies across every ministry (VendorDebarred).',
  NoRuleForMinistry:
    'Blocked: no rule set is configured for this ministry and no DPIIT default rule is in place, so no threshold could be applied (NoRuleForMinistry).',
  WeightsDoNotSumToOneHundredPercent:
    'Blocked: the component weights in this declaration do not add up to 100 percent, so no weighted local-content figure could be computed (WeightsDoNotSumToOneHundredPercent).',
  NotComponentLevelMethod:
    "Blocked: a component-level declaration was submitted but this ministry's rule does not use a component-level or weighted-module calculation method (NotComponentLevelMethod).",

  // pallet-pramaan-preference
  EmptyBidList: 'Blocked: no bids were supplied, so there is nothing to rank (EmptyBidList).',
  ItemCountExceeded: `Blocked: more than ${MAX_BIDS} bids were supplied in a single evaluation (ItemCountExceeded).`,
  TenderValueExceedsDomesticLimit:
    'Blocked: this tender exceeds the Rs 200 crore limit for the domestic preference route and no global tender enquiry has been approved for it (TenderValueExceedsDomesticLimit).',
  NonLocalNotPermittedOnDomesticTender:
    'Blocked: every bid on this tender is from a Non-local supplier, and Non-local suppliers are admitted only where a global tender enquiry has been approved under GFR Rule 161(iv) (NonLocalNotPermittedOnDomesticTender).',
  Para3ARequiresClassOne:
    'Blocked: Para 3A applies to this ministry, restricting sourcing to Class-I suppliers, and no Class-I bid was received (Para3ARequiresClassOne).',

  // pallet-pramaan-certification
  AuditorRequired:
    "Blocked: this contract is at or above the ministry's certification threshold, so a chartered accountant's certificate is mandatory and none was supplied (AuditorRequired).",
  AuditorRoleMissing:
    'Blocked: the account named as the certifying auditor does not hold the auditor role (AuditorRoleMissing).',
  CertificateIdAlreadyUsed:
    'Blocked: a certificate with this identifier has already been issued, and certificate identifiers cannot be reused (CertificateIdAlreadyUsed).',
  AuditorLedgerFull:
    "Blocked: this auditor's accountability ledger is full and cannot record another certificate (AuditorLedgerFull).",

  // pallet-pramaan-debarment
  NoSuchDebarment:
    'Blocked: no debarment record exists for this vendor under this ministry, so there was nothing to lift (NoSuchDebarment).',
  DebarmentRecordsFull:
    "Blocked: this vendor's debarment record list is full and cannot take another entry (DebarmentRecordsFull).",

  // pallet-pramaan-rule-registry
  NotAuthorised:
    'Blocked: the submitting account is not authorised to make this change; rule changes require DPIIT or Root authority (NotAuthorised).',
  InvalidRule:
    'Blocked: the submitted rule set failed the registry’s own validation, so no rule was changed (InvalidRule).',

  // pallet-pramaan-consistency
  HistoryFull:
    "Blocked: this vendor's declaration history for the product is full and cannot take another entry (HistoryFull).",
};

/** The plain sentence for a pallet error name, with a truthful fallback for unknowns. */
export function describePalletError(palletError: string): string {
  return (
    PALLET_ERROR_SENTENCES[palletError] ??
    `Blocked: the chain rejected this submission under its own rule "${palletError}".`
  );
}

// ---------------------------------------------------------------------------------
// The shared route wrapper
// ---------------------------------------------------------------------------------

/** Whatever a route handler returns is merged into the JSON response body. */
export type TriggerPayload = Record<string, unknown>;

export interface TriggerOptions {
  /**
   * Extra fields merged into the RED response produced when the pallet rejects the
   * extrinsic — and, where a route needs it, an override of `result`/`reason`. Used by
   * ca-certification, where `AuditorRequired` is a pending obligation (YELLOW) rather
   * than a compliance failure.
   */
  onPalletRejection?: (error: ExtrinsicFailedError) => TriggerPayload;
}

/**
 * Runs a trigger-point handler and maps every outcome onto the right HTTP status.
 *
 *   BadRequestError       -> 400, with the sentence the validator produced.
 *   ExtrinsicFailedError  -> 200 with `result: 'RED'`. The pallet refusing a bid is the
 *                            product working, not the server failing.
 *   FinalityTimeoutError  -> 504. The transaction may still finalize; the caller is
 *                            told exactly that rather than being handed a wrong answer.
 *   anything else         -> 500.
 */
export async function handleTrigger(
  request: Request,
  handler: (body: Record<string, unknown>) => Promise<TriggerPayload>,
  options: TriggerOptions = {},
): Promise<Response> {
  const startedAt = Date.now();
  try {
    const body = await readJsonObject(request);
    const payload = await handler(body);
    // The handler's own `latencyMs` (submission -> confirmed finality, measured by
    // `submitAndFinalize`) wins where there is one; the wall-clock fallback covers the
    // paths that reach a decision from a storage read without submitting anything.
    return Response.json({ latencyMs: Date.now() - startedAt, ...payload });
  } catch (error) {
    const latencyMs = Date.now() - startedAt;

    if (error instanceof BadRequestError) {
      return Response.json({ error: error.message }, { status: 400 });
    }

    if (error instanceof ExtrinsicFailedError) {
      // A rejection is an on-chain fact and must be auditable: the extrinsic WAS
      // included, and the block that carried the rejection is what a reviewer looks up
      // when asked "why was this blocked". `ExtrinsicFailedError` now carries that block
      // hash, so resolve it to a number rather than reporting null. If the hash is
      // genuinely unavailable (the transaction failed before inclusion) null is still
      // returned — honest, rather than invented.
      let blockNumber: number | null = null;
      let blockHash: string | null = error.blockHash ?? null;
      if (error.blockHash) {
        try {
          const api = await getApi();
          blockNumber = (await api.rpc.chain.getHeader(error.blockHash)).number.toNumber();
        } catch {
          blockHash = error.blockHash;
        }
      }
      return Response.json({
        result: 'RED' satisfies TriState,
        reason: describePalletError(error.palletError),
        palletError: error.palletError,
        txRef: error.txRef,
        blockNumber,
        blockHash,
        latencyMs,
        ...(options.onPalletRejection?.(error) ?? {}),
      });
    }

    if (error instanceof FinalityTimeoutError) {
      return Response.json(
        {
          error:
            `The transaction was included in block #${error.inclusionBlock} but that block did ` +
            `not reach finality within ${error.waitedMs} ms, so no compliance answer can be ` +
            `returned yet. The transaction may still finalize; re-check by transaction reference.`,
          txRef: error.txRef,
          inclusionBlockNumber: error.inclusionBlock,
          latencyMs,
        },
        { status: 504 },
      );
    }

    const detail = error instanceof Error ? error.message : String(error);
    return Response.json(
      { error: 'The compliance service could not complete this request.', detail, latencyMs },
      { status: 500 },
    );
  }
}

// ---------------------------------------------------------------------------------
// Extrinsic builders
// ---------------------------------------------------------------------------------

/** `pallet_pramaan_classification::classify` — PathwayId P1 / P3 / P4. */
export function classifyTx(
  api: ApiPromise,
  args: {
    vendor: string;
    tender: string;
    ministry: string;
    declaredLocalContentBps: number;
    isPliManufacturer: boolean;
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanClassification.classify(
    args.vendor,
    args.tender,
    args.ministry,
    args.declaredLocalContentBps,
    args.isPliManufacturer,
  );
}

/** `pallet_pramaan_classification::ComponentDeclaration`, ready to encode. */
export interface ChainComponentDeclaration {
  name: string;
  declaredBps: number;
  weightBps: number;
}

/**
 * Validate a `components` array. Its presence on a bid-submission body selects the
 * component-level path (PathwayId P2) instead of the single-percentage one.
 *
 * The weights are deliberately NOT checked here for summing to 10000. The pallet
 * refuses a short weight set with `WeightsDoNotSumToOneHundredPercent` rather than
 * silently normalising it, and that refusal is the behaviour worth demonstrating — so
 * the chain, not this file, gets to be the one that says no.
 */
export function requireComponents(value: unknown): ChainComponentDeclaration[] {
  if (!Array.isArray(value) || value.length === 0) {
    throw new BadRequestError('"components", when supplied, must be a non-empty array.');
  }
  if (value.length > MAX_COMPONENTS) {
    throw new BadRequestError(
      `"components" may hold at most ${MAX_COMPONENTS} entries (the runtime's MaxComponents ` +
        `bound); got ${value.length}.`,
    );
  }
  return value.map((entry, index) => {
    if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) {
      throw new BadRequestError(`"components[${index}]" must be an object.`);
    }
    const component = entry as Record<string, unknown>;
    return {
      name: requireId(component, 'name'),
      declaredBps: requireBps(component, 'declaredBps'),
      weightBps: requireBps(component, 'weightBps'),
    };
  });
}

/**
 * The weighted average the component-level path produces, computed exactly as
 * `pallet_pramaan_classification::weighted_average_bps` computes it: sum of
 * `declared_bps * weight_bps`, divided by 10000 with integer truncation.
 *
 * This is recomputed here rather than read back because the `Classified` event carries
 * only the resulting class, not the aggregate that produced it — and the consistency
 * declaration needs a single percentage to record against the vendor's history. Any
 * divergence from the pallet's arithmetic would put a different number in the history
 * than the one the classification used, so the formula is mirrored digit for digit.
 */
export function weightedAverageBps(components: ChainComponentDeclaration[]): number {
  let weightedSum = 0;
  for (const component of components) {
    weightedSum += component.declaredBps * component.weightBps;
  }
  return Math.trunc(weightedSum / BPS_DENOMINATOR);
}

/** `pallet_pramaan_classification::classify_component_level` — PathwayId P2. */
export function classifyComponentLevelTx(
  api: ApiPromise,
  args: {
    vendor: string;
    tender: string;
    ministry: string;
    components: ChainComponentDeclaration[];
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanClassification.classifyComponentLevel(
    args.vendor,
    args.tender,
    args.ministry,
    args.components,
  );
}

/**
 * `pallet_pramaan_consistency::declare` — the cross-tender consistency check.
 *
 * Not a thirteenth pathway: the PoC document is explicit that it "runs across all
 * twelve, because a declaration is checked against the vendor's history regardless of
 * which route it takes". That is why bid submission calls it alongside classification
 * rather than instead of anything.
 */
export function declareTx(
  api: ApiPromise,
  args: {
    vendor: string;
    product: string;
    tender: string;
    localContentBps: number;
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanConsistency.declare(
    args.vendor,
    args.product,
    args.tender,
    args.localContentBps,
  );
}

/** One vendor's row of the preference outcome, as the pallet stores it. */
export interface PreferenceOutcomeRow {
  vendor: string;
  qualifies: boolean;
  awardedPercentBps: number;
  matchedPricePaise: string | null;
  decisionPath: string | null;
}

/**
 * Per-vendor preference outcomes, read at the hash of the block that was just finalized.
 *
 * This is the one place a result is read from storage rather than from an event, and
 * the reason is structural: `PreferenceCalculated` is a tender-level event carrying only
 * `qualifies` and `matchedPrice`, while the pallet writes the per-vendor detail —
 * `awarded_percent_bps` and `decision_path` — to `PreferenceResults`. Without that
 * detail P8 and P9 are indistinguishable from the outside: both offer a match at L1's
 * price, and only the award share (50% vs 100%) and the recorded pathway tell them
 * apart, so the "12 of 12 pathways" claim would be unfalsifiable.
 *
 * The provenance guarantee is preserved by reading `api.at(blockHash)` — the state as
 * of the finalized block itself, not whatever the chain looks like by the time the
 * response is written.
 *
 * Rows are looked up per submitted vendor rather than by enumerating storage keys, so
 * each row's `vendor` is byte-for-byte the address the caller sent.
 */
export async function readPreferenceOutcomes(
  api: ApiPromise,
  blockHash: string,
  tender: string,
  vendors: string[],
): Promise<PreferenceOutcomeRow[]> {
  const at = await api.at(blockHash);
  const rows: PreferenceOutcomeRow[] = [];

  for (const vendor of vendors) {
    const raw = await at.query.pramaanPreference.preferenceResults(vendor, tender);
    const json = raw.toJSON();
    if (typeof json !== 'object' || json === null || Array.isArray(json)) continue;

    const outcome = json as Record<string, unknown>;
    const matched = coerceUnsignedInteger(outcome.matchedPrice);
    const awarded = coerceUnsignedInteger(outcome.awardedPercentBps);

    rows.push({
      vendor,
      qualifies: outcome.qualifies === true,
      awardedPercentBps: awarded === null ? 0 : Number(awarded),
      matchedPricePaise: matched === null ? null : matched.toString(),
      decisionPath: decodePathway(outcome.decisionPath),
    });
  }
  return rows;
}

/**
 * `pramaan_primitives::PathwayId` as it appears in `toJSON()`. A fieldless enum variant
 * renders either as a bare string or as a single-key object depending on the codec
 * path, so both are accepted and normalised to the document's own spelling (`P8`).
 */
function decodePathway(value: unknown): string | null {
  if (typeof value === 'string') return value.toUpperCase();
  if (typeof value === 'object' && value !== null && !Array.isArray(value)) {
    const [key] = Object.keys(value as Record<string, unknown>);
    return key ? key.toUpperCase() : null;
  }
  return null;
}

/** One bid as `pallet_pramaan_preference::BidItem` expects it. */
export interface ChainBidItem {
  vendor: string;
  class: BidClass;
  price: string;
  isMse: boolean;
  isGte: boolean;
}

/** `pallet_pramaan_preference::calculate_preference` — PathwayId P5 through P10. */
export function calculatePreferenceTx(
  api: ApiPromise,
  args: {
    tender: string;
    ministry: string;
    bids: ChainBidItem[];
    tenderValuePaise: bigint;
    isTenderGte: boolean;
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanPreference.calculatePreference(
    args.tender,
    args.ministry,
    args.bids,
    args.tenderValuePaise.toString(),
    args.isTenderGte,
  );
}

/** `pallet_pramaan_certification::certify` — PathwayId P11. */
export function certifyTx(
  api: ApiPromise,
  args: {
    certificateId: string;
    ministry: string;
    vendor: string;
    tender: string;
    valuePaise: bigint;
    auditor: string | null;
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanCertification.certify(
    args.certificateId,
    args.ministry,
    args.vendor,
    args.tender,
    args.valuePaise.toString(),
    args.auditor,
  );
}

/** `pallet_pramaan_debarment::debar` — PathwayId P12. */
export function debarTx(
  api: ApiPromise,
  args: {
    vendor: string;
    ministry: string;
    effectiveFrom: number;
    effectiveTo: number | null;
    reason: string;
  },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanDebarment.debar(
    args.vendor,
    args.ministry,
    args.effectiveFrom,
    args.effectiveTo,
    args.reason,
  );
}

/** `pallet_pramaan_debarment::lift_debarment`. */
export function liftDebarmentTx(
  api: ApiPromise,
  args: { vendor: string; ministry: string },
): SubmittableExtrinsic<'promise'> {
  return api.tx.pramaanDebarment.liftDebarment(args.vendor, args.ministry);
}

/** The nine rule parameters, keyed exactly as the runtime metadata names them. */
export interface ChainRule {
  hsnThresholds: { hsnCode: string; classOneBps: number; classTwoBps: number }[];
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: CalculationMethod;
  preferenceMarginBps: number;
  certificationThreshold: string;
  exemptionFloor: string;
  divisibility: Divisibility;
  effectiveFrom: number;
}

/**
 * `pallet_pramaan_rule_registry::set_rule`, wrapped in `sudo.sudo`.
 *
 * FLAGGED — spec vs. source. The build contract (section 2) says `set_rule`'s origin is
 * "DPIIT or Root", which reads as though a DPIIT-held signed account could submit it
 * directly. The runtime disagrees: `cerulea-runtime/src/configs/mod.rs` wires
 * `type DpiitOrigin = DpiitOrRoot` where `pub type DpiitOrRoot = EnsureRoot<AccountId>`,
 * with its own comment explaining that this build has no standalone DPIIT account, so
 * DPIIT authority "is represented here as Root, reachable through the existing
 * pallet_sudo". A plain signed `set_rule` therefore always fails with `NotAuthorised`.
 * The source wins, so the call is dispatched through sudo, signed by the DPIIT persona.
 */
export function setRuleSudoTx(
  api: ApiPromise,
  args: { ministry: string; rule: ChainRule },
): SubmittableExtrinsic<'promise'> {
  return api.tx.sudo.sudo(api.tx.pramaanRuleRegistry.setRule(args.ministry, args.rule));
}

/**
 * Validate and shape the nine rule parameters from a request body.
 *
 * The three checks the registry's own `validate_rule` performs are mirrored here so a
 * malformed rule is a 400 with a specific message rather than an opaque on-chain
 * `InvalidRule`. The on-chain check remains authoritative and is still mapped to RED if
 * it ever rejects something this passes.
 */
export function buildChainRule(raw: unknown): ChainRule {
  if (typeof raw !== 'object' || raw === null || Array.isArray(raw)) {
    throw new BadRequestError('"rule" is required and must be an object with the nine rule parameters.');
  }
  const rule = raw as Record<string, unknown>;

  const thresholdsRaw = rule.hsnThresholds;
  if (!Array.isArray(thresholdsRaw)) {
    throw new BadRequestError('"rule.hsnThresholds" is required and must be an array.');
  }
  if (thresholdsRaw.length > MAX_HSN_THRESHOLDS) {
    throw new BadRequestError(
      `"rule.hsnThresholds" may hold at most ${MAX_HSN_THRESHOLDS} entries; got ${thresholdsRaw.length}.`,
    );
  }

  const hsnThresholds = thresholdsRaw.map((entry, index) => {
    if (typeof entry !== 'object' || entry === null || Array.isArray(entry)) {
      throw new BadRequestError(`"rule.hsnThresholds[${index}]" must be an object.`);
    }
    const row = entry as Record<string, unknown>;
    const hsnCode = requireId(row, 'hsnCode');
    const classOneBps = requireBps(row, 'classOneBps');
    const classTwoBps = requireBps(row, 'classTwoBps');
    // Mirrors pallet-pramaan-rule-registry::validate_rule.
    if (classOneBps < classTwoBps) {
      throw new BadRequestError(
        `"rule.hsnThresholds[${index}]": the Class-I threshold (${formatBps(classOneBps)}) cannot be ` +
          `below the Class-II threshold (${formatBps(classTwoBps)}).`,
      );
    }
    return { hsnCode, classOneBps, classTwoBps };
  });

  return {
    hsnThresholds,
    para3aApplicable: optionalBoolean(rule, 'para3aApplicable', false),
    pliLinked: optionalBoolean(rule, 'pliLinked', false),
    calculationMethod: requireEnum(rule, 'calculationMethod', CALCULATION_METHODS),
    preferenceMarginBps: requireBps(rule, 'preferenceMarginBps'),
    // Named as the on-chain struct names them, since a caller building a Rule mirrors
    // the chain's shape; the `...Paise` aliases are accepted too, because section 1 of
    // the build contract asks every currency-carrying JSON field to be labelled that way.
    certificationThreshold: requirePaise(
      rule,
      'certificationThreshold',
      'certificationThresholdPaise',
    ).toString(),
    exemptionFloor: requirePaise(rule, 'exemptionFloor', 'exemptionFloorPaise').toString(),
    divisibility: requireEnum(rule, 'divisibility', DIVISIBILITIES),
    effectiveFrom: requireBlockNumber(rule, 'effectiveFrom'),
  };
}

// ---------------------------------------------------------------------------------
// Formatting used in `reason` sentences
// ---------------------------------------------------------------------------------

/**
 * Paise as a figure an Indian procurement officer reads without conversion
 * ("₹10 crore", "₹5 lakh"). Wraps `units.formatPaise` so the routes never touch a raw
 * paise figure in prose meant to be read aloud.
 */
export function rupees(paise: bigint): string {
  return formatPaise(paise);
}
