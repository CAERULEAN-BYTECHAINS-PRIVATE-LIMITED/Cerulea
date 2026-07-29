/**
 * Anomaly detection over finalized chain records.
 *
 * ## What this is, stated plainly
 *
 * There are two different kinds of signal in here and the distinction matters enough that
 * the API carries it as a field on every finding (`origin`) rather than leaving it to a
 * footnote:
 *
 * 1. **`on-chain`** — the network's own consensus rules raised it. Exactly one signal
 *    qualifies today: `pallet-pramaan-consistency` emits `InconsistencyFlagged` whenever a
 *    new declaration differs from a prior declaration for the same `(vendor, product)` pair
 *    by more than the runtime's `ToleranceBps`. Every validator ran that comparison and
 *    agreed on the result. This module recomputes it from `pramaanConsistency.declarations`
 *    — byte for byte the pallet's own `abs_diff(new, prior) > ToleranceBps` test — rather
 *    than from the event stream, because the event index only holds the last few hundred
 *    blocks while storage holds every declaration ever made.
 *
 * 2. **`heuristic`** — a review prompt computed here, in the reviewer's console, over
 *    records the chain has already finalized. The chain did not raise these and does not
 *    endorse them. They are patterns worth a human's attention, nothing more, and the UI
 *    is required to say so.
 *
 * ## What this is NOT
 *
 * It is not a statistical model. The technical specification (Section 13.1) places
 * Isolation Forest anomaly detection in Phase 2, explicitly out of scope for this build,
 * to be layered on top of the tolerance check in Part 5.6. Nothing here is trained,
 * scored, or probabilistic — every finding is a deterministic rule with its inputs
 * printed next to it, and `MODEL_STATUS` below is returned to the client so the UI can
 * label the model as roadmap rather than shipped.
 *
 * ## Honesty constraints carried over from the rest of the reader
 *
 * Nothing is invented. Every field on every finding is a storage read or a decoded event.
 * A signal with no hits returns no findings and the UI renders an empty state naming what
 * would fill it. There is no sample-data path and no hardcoded list of suspect vendors.
 */

import { getApi } from '@/lib/chain';
import { coerceUnsignedInteger, decodeByteVec } from '@/lib/pramaan';
import { formatBps, formatPaise } from '@/lib/units';
import { indexWindow, recentEvents, type IndexWindow } from './reader';

// ---------------------------------------------------------------------------------
// Shapes
// ---------------------------------------------------------------------------------

/**
 * Review priority. Deliberately NOT the `GREEN` / `YELLOW` / `RED` tri-state: those three
 * are reserved by the build contract for a compliance verdict under the PPP-MII Order,
 * and an anomaly is not a verdict. A finding here says "look at this", never "this is
 * non-compliant".
 */
export type Severity = 'critical' | 'elevated' | 'watch';

/** Whether consensus raised this, or whether this console computed it after the fact. */
export type FindingOrigin = 'on-chain' | 'heuristic';

export type SignalId =
  | 'declaration-contradiction'
  | 'threshold-gaming'
  | 'repeat-non-local'
  | 'post-debarment-activity'
  | 'certification-threshold-clustering';

export interface Finding {
  id: string;
  signal: SignalId;
  signalLabel: string;
  origin: FindingOrigin;
  severity: Severity;
  /** SS58 address. Name resolution is the client's job — the chain holds accounts. */
  vendor: string;
  headline: string;
  /** One sentence a reviewer can act on without reading the code. */
  evidence: string;
  /** The block where the behaviour is verifiable, when a record carries one. */
  blockNumber: number | null;
  /** Populated only when the emitting extrinsic is still inside the event index window. */
  txRef: string | null;
  facts: { label: string; value: string }[];
}

export interface SignalSummary {
  id: SignalId;
  label: string;
  origin: FindingOrigin;
  /** How the signal is computed, in one sentence, for the UI to print verbatim. */
  method: string;
  count: number;
  /** Findings withheld from `findings` because the signal hit its display cap. */
  truncated: number;
}

export interface AnomalyReport {
  chain: { currentBlock: number; finalizedBlock: number };
  counters: {
    total: number;
    critical: number;
    elevated: number;
    watch: number;
    onChain: number;
    heuristic: number;
    vendorsFlagged: number;
    recordsScanned: number;
  };
  model: { name: string; status: string; note: string };
  signals: SignalSummary[];
  findings: Finding[];
  sources: Record<string, string>;
  window: IndexWindow;
}

/** Returned to the client so the roadmap claim is made by the API, not by the markup. */
const MODEL_STATUS = {
  name: 'Isolation Forest anomaly detection',
  status: 'Phase 2 — roadmap, not shipped',
  note:
    'The technical specification places the statistical model in Phase 2, on top of the ' +
    'tolerance check built in Part 5.6. Nothing on this page is a trained model: every ' +
    'finding below is a deterministic rule with its own inputs shown alongside it.',
};

/** At most this many findings per signal, so one noisy rule cannot bury the others. */
const MAX_PER_SIGNAL = 12;

/** Threshold gaming: a declaration this many bps at or above a boundary counts as "at" it. */
const BOUNDARY_TOUCH_BPS = 2;
/** Certification clustering: within this fraction below a threshold counts as "just under". */
const JUST_UNDER_PERCENT = 2;
/** A heuristic pattern needs at least this many occurrences by one vendor to be raised. */
const MIN_REPEATS = 2;

const SEVERITY_RANK: Record<Severity, number> = { critical: 0, elevated: 1, watch: 2 };

// ---------------------------------------------------------------------------------
// Decoding helpers
// ---------------------------------------------------------------------------------

/**
 * The two halves of a tuple storage key.
 *
 * `Declarations` is keyed by `(VendorId, ProductId)` and `Classifications` by
 * `(AccountId, TenderId)` — one `Blake2_128Concat` hasher over the whole tuple, so
 * `key.args[0]` is the tuple codec itself and its members are indexable.
 */
function tupleKeyParts(key: { args: readonly unknown[] }): [unknown, unknown] {
  const tuple = key.args[0] as unknown as Record<number, unknown>;
  return [tuple[0], tuple[1]];
}

/** SS58 for an `AccountId` codec. `toString()` is the SS58 form for this type. */
function account(codec: unknown): string {
  return String(codec ?? '');
}

/** A `BoundedVec<u8, IdBound>` (tender / product / ministry id) as readable ASCII. */
function idText(codec: unknown): string {
  if (codec === null || codec === undefined) return '';
  const withHex = codec as { toHex?: () => string };
  if (typeof withHex.toHex === 'function') return decodeByteVec(withHex.toHex());
  return decodeByteVec(codec);
}

function toNumber(value: unknown): number {
  const parsed = Number(String(value ?? '0').replace(/,/g, ''));
  return Number.isFinite(parsed) ? parsed : 0;
}

/** Both spellings, because `toJSON()` camel-cases and `toHuman()` sometimes does not. */
function field<T>(record: Record<string, unknown>, camel: string, snake: string): T | undefined {
  return (record[camel] ?? record[snake]) as T | undefined;
}

function plural(count: number, one: string, many: string): string {
  return count === 1 ? one : many;
}

/**
 * A shortfall below a threshold, which is routinely smaller than one rupee — the whole
 * point of the signal is a value trimmed to sit *just* under. `formatPaise` rounds those
 * to "₹0", which reads as though there were no gap at all, so sub-rupee gaps are stated
 * in paise instead.
 */
function formatShortfall(paise: bigint): string {
  if (paise < BigInt(100)) return `${paise.toString()} ${plural(Number(paise), 'paisa', 'paise')}`;
  return formatPaise(paise);
}

// ---------------------------------------------------------------------------------
// The report
// ---------------------------------------------------------------------------------

export async function buildAnomalyReport(): Promise<AnomalyReport> {
  const api = await getApi();

  const [head, finalizedHash] = await Promise.all([
    api.rpc.chain.getHeader(),
    api.rpc.chain.getFinalizedHead(),
  ]);
  const currentBlock = head.number.toNumber();
  const finalizedBlock = (await api.rpc.chain.getHeader(finalizedHash)).number.toNumber();

  const [ruleEntries, defaultRule, declarationEntries, classificationEntries, debarmentEntries, certificateEntries] =
    await Promise.all([
      api.query.pramaanRuleRegistry.rules.entries(),
      api.query.pramaanRuleRegistry.defaultRule(),
      api.query.pramaanConsistency.declarations.entries(),
      api.query.pramaanClassification.classifications.entries(),
      api.query.pramaanDebarment.debarments.entries(),
      api.query.pramaanCertification.certificates.entries(),
    ]);

  const toleranceBps = toNumber(api.consts.pramaanConsistency?.toleranceBps?.toString() ?? '0');

  // --- Rule registry: the boundaries the two heuristics below measure against ---------
  const classBoundaries = new Map<number, { label: string; ministries: Set<string> }>();
  const certThresholds = new Map<string, Set<string>>(); // paise (decimal string) -> ministries

  const collectRule = (ministry: string, json: unknown): void => {
    if (typeof json !== 'object' || json === null || Array.isArray(json)) return;
    const rule = json as Record<string, unknown>;

    const thresholds =
      (field<Record<string, unknown>[]>(rule, 'hsnThresholds', 'hsn_thresholds') ?? []);
    for (const threshold of thresholds) {
      const classOne = toNumber(field(threshold, 'classOneBps', 'class_one_bps'));
      const classTwo = toNumber(field(threshold, 'classTwoBps', 'class_two_bps'));
      const hsn = decodeByteVec(field(threshold, 'hsnCode', 'hsn_code'));
      for (const [bps, label] of [
        [classOne, `Class-I boundary${hsn ? ` (HSN ${hsn})` : ''}`],
        [classTwo, `Class-II boundary${hsn ? ` (HSN ${hsn})` : ''}`],
      ] as const) {
        if (bps <= 0) continue;
        const bucket = classBoundaries.get(bps) ?? { label, ministries: new Set<string>() };
        bucket.ministries.add(ministry);
        classBoundaries.set(bps, bucket);
      }
    }

    const certificationThreshold = coerceUnsignedInteger(
      field(rule, 'certificationThreshold', 'certification_threshold'),
    );
    if (certificationThreshold !== null && certificationThreshold > BigInt(0)) {
      const key = certificationThreshold.toString();
      const bucket = certThresholds.get(key) ?? new Set<string>();
      bucket.add(ministry);
      certThresholds.set(key, bucket);
    }
  };

  for (const [key, value] of ruleEntries) {
    const ministry = idText(key.args[0]);
    collectRule(ministry, (value as unknown as { toJSON(): unknown }).toJSON());
  }
  collectRule('DPIIT default', (defaultRule as unknown as { toJSON(): unknown }).toJSON());

  // --- Event index, for a transaction hash where one is still in the window -----------
  const flagEvents = recentEvents({ limit: 6_000, sections: ['pramaanConsistency'] }).filter(
    (event) => event.method === 'InconsistencyFlagged',
  );

  const findings: Finding[] = [];
  const signals: SignalSummary[] = [];
  let recordsScanned = 0;

  const push = (
    summary: Omit<SignalSummary, 'count' | 'truncated'>,
    produced: Finding[],
  ): void => {
    produced.sort((a, b) => SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity]);
    const shown = produced.slice(0, MAX_PER_SIGNAL);
    signals.push({ ...summary, count: produced.length, truncated: produced.length - shown.length });
    findings.push(...shown);
  };

  // ===================================================================================
  // 1. Declaration contradiction — ON CHAIN
  // ===================================================================================
  // Recomputes the pallet's own comparison over the full `(vendor, product)` history:
  // every new declaration against every prior one, one finding per contradicting prior,
  // which is exactly one `InconsistencyFlagged` event each. Storage rather than events,
  // so a contradiction from a thousand blocks ago is still reported.
  {
    const produced: Finding[] = [];

    for (const [key, value] of declarationEntries) {
      const [vendorCodec, productCodec] = tupleKeyParts(key);
      const vendor = account(vendorCodec);
      const product = idText(productCodec);
      const history =
        ((value as unknown as { toJSON(): unknown }).toJSON() as Record<string, unknown>[]) ?? [];
      recordsScanned += history.length;

      for (let i = 1; i < history.length; i += 1) {
        const laterBps = toNumber(field(history[i], 'localContentBps', 'local_content_bps'));
        const laterTender = decodeByteVec(history[i].tender);
        const laterBlock = toNumber(field(history[i], 'blockNumber', 'block_number'));

        for (let j = 0; j < i; j += 1) {
          const priorBps = toNumber(field(history[j], 'localContentBps', 'local_content_bps'));
          const gap = Math.abs(laterBps - priorBps);
          if (gap <= toleranceBps) continue;

          const priorTender = decodeByteVec(history[j].tender);
          const priorBlock = toNumber(field(history[j], 'blockNumber', 'block_number'));

          const match = flagEvents.find((event) => {
            const data = event.data;
            return (
              event.blockNumber === laterBlock &&
              String(data.vendor ?? '') === vendor &&
              toNumber(field(data, 'newValue', 'new_value')) === laterBps &&
              toNumber(field(data, 'priorValue', 'prior_value')) === priorBps
            );
          });

          produced.push({
            id: `contradiction-${vendor.slice(0, 8)}-${product}-${priorTender}-${laterTender}`,
            signal: 'declaration-contradiction',
            signalLabel: 'Declaration contradiction',
            origin: 'on-chain',
            severity: gap > toleranceBps * 3 ? 'critical' : 'elevated',
            vendor,
            headline: `${formatBps(priorBps)} then ${formatBps(laterBps)} for the same product`,
            evidence:
              `The same vendor declared ${formatBps(priorBps)} local content for ${product} on ` +
              `tender ${priorTender}, then ${formatBps(laterBps)} for that same product on ` +
              `tender ${laterTender} — a gap of ${formatBps(gap)} against a runtime tolerance of ` +
              `${formatBps(toleranceBps)}. The chain rejected the pair as inconsistent when the ` +
              `second declaration was recorded in block #${laterBlock.toLocaleString('en-IN')}.`,
            blockNumber: laterBlock || null,
            txRef: match?.txRef ?? null,
            facts: [
              { label: 'Product', value: product },
              { label: 'Earlier declaration', value: `${formatBps(priorBps)} on ${priorTender}` },
              { label: 'Later declaration', value: `${formatBps(laterBps)} on ${laterTender}` },
              { label: 'Gap', value: `${formatBps(gap)} (tolerance ${formatBps(toleranceBps)})` },
              {
                label: 'Blocks',
                value: `#${priorBlock.toLocaleString('en-IN')} then #${laterBlock.toLocaleString('en-IN')}`,
              },
            ],
          });
        }
      }
    }

    push(
      {
        id: 'declaration-contradiction',
        label: 'Declaration contradiction',
        origin: 'on-chain',
        method:
          'The consensus rule itself. pallet-pramaan-consistency compares every new ' +
          `declaration against the vendor's full history for that product and flags any pair ` +
          `differing by more than ToleranceBps (${formatBps(toleranceBps)}). Recomputed here from ` +
          'pramaanConsistency.declarations so contradictions older than the event index are not lost.',
      },
      produced,
    );
  }

  // ===================================================================================
  // 2. Threshold gaming — HEURISTIC
  // ===================================================================================
  // A declaration landing exactly on, or one to two basis points above, a classification
  // boundary is legitimate on its own: the boundaries are inclusive at the lower end, so
  // 50.00% really is Class-I. Doing it repeatedly, across tenders, is the pattern worth a
  // reviewer's time. Two or more such declarations by one vendor raises a finding.
  {
    type Touch = { boundary: number; label: string; bps: number; product: string; tender: string; block: number };
    const byVendor = new Map<string, Touch[]>();

    for (const [key, value] of declarationEntries) {
      const [vendorCodec, productCodec] = tupleKeyParts(key);
      const vendor = account(vendorCodec);
      const product = idText(productCodec);
      const history =
        ((value as unknown as { toJSON(): unknown }).toJSON() as Record<string, unknown>[]) ?? [];

      for (const record of history) {
        const bps = toNumber(field(record, 'localContentBps', 'local_content_bps'));
        for (const [boundary, meta] of classBoundaries) {
          const over = bps - boundary;
          if (over < 0 || over > BOUNDARY_TOUCH_BPS) continue;
          const touches = byVendor.get(vendor) ?? [];
          touches.push({
            boundary,
            label: meta.label,
            bps,
            product,
            tender: decodeByteVec(record.tender),
            block: toNumber(field(record, 'blockNumber', 'block_number')),
          });
          byVendor.set(vendor, touches);
          break;
        }
      }
    }

    const produced: Finding[] = [];
    for (const [vendor, touches] of byVendor) {
      if (touches.length < MIN_REPEATS) continue;
      const latest = touches.reduce((a, b) => (b.block > a.block ? b : a));
      produced.push({
        id: `gaming-${vendor.slice(0, 8)}`,
        signal: 'threshold-gaming',
        signalLabel: 'Threshold gaming',
        origin: 'heuristic',
        severity: touches.length >= 3 ? 'elevated' : 'watch',
        vendor,
        headline: `${touches.length} declarations sitting on a classification boundary`,
        evidence:
          `This vendor made ${touches.length} declarations that land exactly on, or within ` +
          `${BOUNDARY_TOUCH_BPS} basis points above, a ministry's Class-I or Class-II boundary — ` +
          touches
            .slice(0, 4)
            .map((t) => `${formatBps(t.bps)} against a ${formatBps(t.boundary)} boundary on ${t.tender}`)
            .join('; ') +
          `. Each one is permissible in isolation, since the boundaries are inclusive at the lower ` +
          `end. The repetition is what a reviewer may want to test against the underlying cost sheets.`,
        blockNumber: latest.block || null,
        txRef: null,
        facts: [
          { label: 'Boundary touches', value: String(touches.length) },
          {
            label: 'Products',
            value: [...new Set(touches.map((t) => t.product))].slice(0, 4).join(', '),
          },
          {
            label: 'Tenders',
            value: [...new Set(touches.map((t) => t.tender))].slice(0, 4).join(', '),
          },
          {
            label: 'Boundaries touched',
            value: [...new Set(touches.map((t) => `${formatBps(t.boundary)} ${t.label}`))]
              .slice(0, 3)
              .join(', '),
          },
        ],
      });
    }

    push(
      {
        id: 'threshold-gaming',
        label: 'Threshold gaming',
        origin: 'heuristic',
        method:
          `Declarations landing at, or up to ${BOUNDARY_TOUCH_BPS} bps above, any Class-I or ` +
          'Class-II boundary held in pramaanRuleRegistry, counted per vendor across tenders. ' +
          `Raised at ${MIN_REPEATS} or more. A single boundary declaration is not an anomaly.`,
      },
      produced,
    );
  }

  // ===================================================================================
  // 3. Repeat non-local — HEURISTIC
  // ===================================================================================
  // A single NonLocal classification is an ordinary outcome, not a finding. A vendor
  // collecting several across different tenders is a supply-chain question.
  {
    const byVendor = new Map<string, string[]>();
    for (const [key, value] of classificationEntries) {
      const [vendorCodec, tenderCodec] = tupleKeyParts(key);
      const outcome = String((value as unknown as { toJSON(): unknown }).toJSON() ?? '');
      recordsScanned += 1;
      if (outcome !== 'NonLocal') continue;
      const vendor = account(vendorCodec);
      const tenders = byVendor.get(vendor) ?? [];
      tenders.push(idText(tenderCodec));
      byVendor.set(vendor, tenders);
    }

    const produced: Finding[] = [];
    for (const [vendor, tenders] of byVendor) {
      if (tenders.length < MIN_REPEATS) continue;
      produced.push({
        id: `non-local-${vendor.slice(0, 8)}`,
        signal: 'repeat-non-local',
        signalLabel: 'Repeat non-local',
        origin: 'heuristic',
        severity: tenders.length >= 3 ? 'elevated' : 'watch',
        vendor,
        headline: `Classified Non-local on ${tenders.length} tenders`,
        evidence:
          `This vendor fell below the Class-II threshold on ${tenders.length} separate ` +
          `${plural(tenders.length, 'tender', 'tenders')} (${tenders.slice(0, 4).join(', ')}). ` +
          'Each classification is a finalized chain record and correct on its own terms; the ' +
          'pattern is surfaced so a reviewer can ask whether the vendor is bidding into ' +
          'categories it cannot supply locally.',
        blockNumber: null,
        txRef: null,
        facts: [
          { label: 'Non-local classifications', value: String(tenders.length) },
          { label: 'Tenders', value: tenders.slice(0, 6).join(', ') },
          { label: 'Verifiable in', value: 'pramaanClassification.classifications' },
        ],
      });
    }

    push(
      {
        id: 'repeat-non-local',
        label: 'Repeat non-local',
        origin: 'heuristic',
        method:
          'Non-local outcomes counted per vendor across every tender in ' +
          `pramaanClassification.classifications. Raised at ${MIN_REPEATS} or more. One Non-local ` +
          'classification is an ordinary result and is not reported.',
      },
      produced,
    );
  }

  // ===================================================================================
  // 4. Post-debarment activity — HEURISTIC (over an on-chain enforcement gap)
  // ===================================================================================
  // Debarment is cross-ministry: pallet-pramaan-debarment's DebarmentCheck ignores the
  // ministry argument entirely, so classification refuses a debarred vendor outright.
  // Certification and declaration recording do not consult it, so activity inside a
  // debarment window is possible and is exactly what this looks for.
  {
    interface Window { ministry: string; from: number; to: number | null; }
    const windows = new Map<string, Window[]>();

    for (const [key, value] of debarmentEntries) {
      const vendor = account(key.args[0]);
      const records =
        ((value as unknown as { toJSON(): unknown }).toJSON() as Record<string, unknown>[]) ?? [];
      const list: Window[] = records.map((record) => ({
        ministry: decodeByteVec(record.ministry),
        from: toNumber(field(record, 'effectiveFrom', 'effective_from')),
        to: (() => {
          const raw = field(record, 'effectiveTo', 'effective_to');
          return raw === null || raw === undefined ? null : toNumber(raw);
        })(),
      }));
      if (list.length > 0) windows.set(vendor, list);
    }

    const covering = (vendor: string, block: number): Window | undefined =>
      windows.get(vendor)?.find((w) => block >= w.from && (w.to === null || block <= w.to));

    const produced: Finding[] = [];

    for (const [key, value] of certificateEntries) {
      const certificate = (value as unknown as { toJSON(): unknown }).toJSON() as Record<
        string,
        unknown
      > | null;
      if (!certificate) continue;
      const vendor = account(certificate.vendor);
      const block = toNumber(field(certificate, 'blockNumber', 'block_number'));
      const window = covering(vendor, block);
      if (!window) continue;
      produced.push({
        id: `post-debarment-cert-${idText(key.args[0])}`,
        signal: 'post-debarment-activity',
        signalLabel: 'Post-debarment activity',
        origin: 'heuristic',
        severity: 'critical',
        vendor,
        headline: `Certificate issued while debarred by ${window.ministry}`,
        evidence:
          `Certificate ${idText(key.args[0])} was recorded for tender ` +
          `${decodeByteVec(certificate.tender)} in block #${block.toLocaleString('en-IN')}, inside ` +
          `an active ${window.ministry} debarment running from block ` +
          `#${window.from.toLocaleString('en-IN')}` +
          `${window.to === null ? ' with no end block set' : ` to #${window.to.toLocaleString('en-IN')}`}. ` +
          'Classification refuses a debarred vendor outright; the certification pallet does not ' +
          'consult the debarment ledger, so this is a gap a reviewer should see rather than a ' +
          'rule the chain broke.',
        blockNumber: block || null,
        txRef: null,
        facts: [
          { label: 'Debarring ministry', value: window.ministry },
          {
            label: 'Debarment window',
            value: `#${window.from.toLocaleString('en-IN')} to ${window.to === null ? 'open-ended' : `#${window.to.toLocaleString('en-IN')}`}`,
          },
          { label: 'Tender', value: decodeByteVec(certificate.tender) },
          {
            label: 'Contract value',
            value: (() => {
              const paise = coerceUnsignedInteger(certificate.value);
              return paise === null ? 'Not readable' : formatPaise(paise);
            })(),
          },
        ],
      });
    }

    for (const [key, value] of declarationEntries) {
      const [vendorCodec, productCodec] = tupleKeyParts(key);
      const vendor = account(vendorCodec);
      if (!windows.has(vendor)) continue;
      const history =
        ((value as unknown as { toJSON(): unknown }).toJSON() as Record<string, unknown>[]) ?? [];
      for (const record of history) {
        const block = toNumber(field(record, 'blockNumber', 'block_number'));
        const window = covering(vendor, block);
        if (!window) continue;
        const tender = decodeByteVec(record.tender);
        produced.push({
          id: `post-debarment-decl-${vendor.slice(0, 8)}-${tender}-${block}`,
          signal: 'post-debarment-activity',
          signalLabel: 'Post-debarment activity',
          origin: 'heuristic',
          severity: 'critical',
          vendor,
          headline: `Declaration recorded while debarred by ${window.ministry}`,
          evidence:
            `A local-content declaration of ` +
            `${formatBps(toNumber(field(record, 'localContentBps', 'local_content_bps')))} for ` +
            `${idText(productCodec)} on tender ${tender} was recorded in block ` +
            `#${block.toLocaleString('en-IN')}, inside an active ${window.ministry} debarment. ` +
            'Debarment on this chain is cross-ministry, so the ministry that debarred the vendor ' +
            'does not limit where the restriction bites.',
          blockNumber: block || null,
          txRef: null,
          facts: [
            { label: 'Debarring ministry', value: window.ministry },
            {
              label: 'Debarment window',
              value: `#${window.from.toLocaleString('en-IN')} to ${window.to === null ? 'open-ended' : `#${window.to.toLocaleString('en-IN')}`}`,
            },
            { label: 'Product', value: idText(productCodec) },
            { label: 'Tender', value: tender },
          ],
        });
      }
    }

    push(
      {
        id: 'post-debarment-activity',
        label: 'Post-debarment activity',
        origin: 'heuristic',
        method:
          'Certificates and declarations whose recorded block falls inside an active window in ' +
          'pramaanDebarment.debarments for the same vendor. Debarment is enforced cross-ministry, ' +
          'so the debarring ministry does not narrow the window.',
      },
      produced,
    );
  }

  // ===================================================================================
  // 5. Certification threshold clustering — HEURISTIC
  // ===================================================================================
  // A contract value landing just below a ministry's certification threshold avoids the
  // mandatory auditor certificate. One such value proves nothing. Several by one vendor
  // is the shape of a contract deliberately split or trimmed.
  {
    const thresholds = [...certThresholds.entries()].map(([paise, ministries]) => ({
      value: BigInt(paise),
      ministries: [...ministries].sort(),
    }));

    interface Near { certificateId: string; value: bigint; threshold: bigint; ministries: string[]; tender: string; block: number; auditor: boolean; }
    const byVendor = new Map<string, Near[]>();

    for (const [key, value] of certificateEntries) {
      const certificate = (value as unknown as { toJSON(): unknown }).toJSON() as Record<
        string,
        unknown
      > | null;
      if (!certificate) continue;
      recordsScanned += 1;
      const amount = coerceUnsignedInteger(certificate.value);
      if (amount === null) continue;

      for (const threshold of thresholds) {
        if (amount >= threshold.value) continue;
        const floor =
          threshold.value - (threshold.value * BigInt(JUST_UNDER_PERCENT)) / BigInt(100);
        if (amount < floor) continue;
        const vendor = account(certificate.vendor);
        const list = byVendor.get(vendor) ?? [];
        list.push({
          certificateId: idText(key.args[0]),
          value: amount,
          threshold: threshold.value,
          ministries: threshold.ministries,
          tender: decodeByteVec(certificate.tender),
          block: toNumber(field(certificate, 'blockNumber', 'block_number')),
          auditor: certificate.auditor !== null && certificate.auditor !== undefined,
        });
        byVendor.set(vendor, list);
        break;
      }
    }

    const produced: Finding[] = [];
    for (const [vendor, near] of byVendor) {
      const latest = near.reduce((a, b) => (b.block > a.block ? b : a));
      const shortfall = latest.threshold - latest.value;
      produced.push({
        id: `cert-cluster-${vendor.slice(0, 8)}`,
        signal: 'certification-threshold-clustering',
        signalLabel: 'Certification threshold clustering',
        origin: 'heuristic',
        severity: near.length >= MIN_REPEATS ? 'elevated' : 'watch',
        vendor,
        headline:
          near.length === 1
            ? `One contract certified at ${formatPaise(latest.value)}, ${formatShortfall(shortfall)} under the threshold`
            : `${near.length} contracts priced just under the certification threshold`,
        evidence:
          `${near.length} ${plural(near.length, 'contract', 'contracts')} certified for this vendor ` +
          `${plural(near.length, 'lands', 'land')} within ${JUST_UNDER_PERCENT}% below a ministry's ` +
          `certification threshold — ` +
          near
            .slice(0, 3)
            .map(
              (n) =>
                `${formatPaise(n.value)} against a ${formatPaise(n.threshold)} threshold on ${n.tender}`,
            )
            .join('; ') +
          `. Below the threshold a mandatory auditor certificate is not required, so a value ` +
          'trimmed to sit just under it is worth a look at how the contract was scoped.',
        blockNumber: latest.block || null,
        txRef: null,
        facts: [
          { label: 'Contracts just under', value: String(near.length) },
          {
            label: 'Threshold',
            value: `${formatPaise(latest.threshold)} (${latest.ministries.slice(0, 3).join(', ')})`,
          },
          { label: 'Closest shortfall', value: formatShortfall(shortfall) },
          {
            label: 'Auditor signature',
            value: near.some((n) => n.auditor)
              ? 'At least one carries a voluntary auditor signature'
              : 'None carry an auditor signature',
          },
          { label: 'Certificates', value: near.slice(0, 4).map((n) => n.certificateId).join(', ') },
        ],
      });
    }

    push(
      {
        id: 'certification-threshold-clustering',
        label: 'Certification threshold clustering',
        origin: 'heuristic',
        method:
          `Certificate values within ${JUST_UNDER_PERCENT}% below any certificationThreshold held ` +
          'in pramaanRuleRegistry, grouped per vendor. Below that threshold the mandatory auditor ' +
          'certificate does not apply.',
      },
      produced,
    );
  }

  // --- Totals --------------------------------------------------------------------------
  findings.sort(
    (a, b) =>
      SEVERITY_RANK[a.severity] - SEVERITY_RANK[b.severity] ||
      (a.origin === b.origin ? 0 : a.origin === 'on-chain' ? -1 : 1) ||
      (b.blockNumber ?? 0) - (a.blockNumber ?? 0),
  );

  const counters = {
    total: signals.reduce((sum, signal) => sum + signal.count, 0),
    critical: findings.filter((f) => f.severity === 'critical').length,
    elevated: findings.filter((f) => f.severity === 'elevated').length,
    watch: findings.filter((f) => f.severity === 'watch').length,
    onChain: signals.filter((s) => s.origin === 'on-chain').reduce((sum, s) => sum + s.count, 0),
    heuristic: signals.filter((s) => s.origin === 'heuristic').reduce((sum, s) => sum + s.count, 0),
    vendorsFlagged: new Set(findings.map((f) => f.vendor)).size,
    recordsScanned,
  };

  return {
    chain: { currentBlock, finalizedBlock },
    counters,
    model: MODEL_STATUS,
    signals,
    findings,
    sources: {
      'declaration-contradiction': 'pramaanConsistency.declarations (+ InconsistencyFlagged events)',
      'threshold-gaming': 'pramaanConsistency.declarations x pramaanRuleRegistry.rules',
      'repeat-non-local': 'pramaanClassification.classifications',
      'post-debarment-activity':
        'pramaanDebarment.debarments x pramaanCertification.certificates / pramaanConsistency.declarations',
      'certification-threshold-clustering':
        'pramaanCertification.certificates x pramaanRuleRegistry.rules',
    },
    window: indexWindow(),
  };
}
