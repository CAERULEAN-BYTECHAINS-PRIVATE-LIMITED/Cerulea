/**
 * `GET /api/chain/rules` — the ministry rule registry, read from the chain.
 *
 * Every consumer that displays "the rules in force" must read them from here rather than
 * from a bundled constant. A QA pass found `/dpiit` rendering a hardcoded table as if it
 * were registry state: five ministries already showed an HSN code the chain contradicted,
 * and after any `rule-update` the table kept showing the superseded values — reachable in
 * two clicks from the scripted walkthrough itself (update MeitY's threshold at step 6,
 * open the national console, see the old number).
 *
 * `effectiveInForce` is not cosmetic. `pallet-pramaan-rule-registry::get_effective_rule`
 * treats a rule whose `effective_from` is still in the future as not yet commenced and
 * falls back to the DPIIT default until it is, so a ministry can simultaneously have a
 * notified rule and be judged under the default. Both are reported, because showing only
 * the notified rule would misstate what a bid submitted right now is measured against.
 */

import { getApi } from '@/lib/chain';
import { u8aToString, hexToU8a } from '@cerulea/api';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

interface HsnThreshold {
  hsnCode: string;
  classOneBps: number;
  classTwoBps: number;
}

interface ChainRule {
  hsnThresholds: HsnThreshold[];
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: string;
  preferenceMarginBps: number;
  certificationThresholdPaise: string;
  exemptionFloorPaise: string;
  divisibility: string;
  effectiveFrom: number;
}

/** Ids are `BoundedVec<u8, 64>`; the chain hands them back as hex. */
function decodeId(value: unknown): string {
  const raw = String(value);
  if (!raw.startsWith('0x')) return raw;
  try {
    return u8aToString(hexToU8a(raw));
  } catch {
    return raw;
  }
}

function toNumber(value: unknown): number {
  const n = Number(String(value).replace(/,/g, ''));
  return Number.isFinite(n) ? n : 0;
}

/** u128 arrives as a number, a decimal string, or 0x-hex depending on magnitude. */
function toBigIntString(value: unknown): string {
  const raw = String(value).replace(/,/g, '');
  try {
    return BigInt(raw).toString();
  } catch {
    return '0';
  }
}

/** A fieldless SCALE enum decodes as either a bare string or a single-key object. */
function decodeEnum(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value && typeof value === 'object') {
    const keys = Object.keys(value as Record<string, unknown>);
    if (keys.length === 1) {
      const k = keys[0];
      return k.charAt(0).toUpperCase() + k.slice(1);
    }
  }
  return String(value);
}

function decodeRule(json: Record<string, unknown>): ChainRule {
  const thresholds = Array.isArray(json.hsnThresholds) ? json.hsnThresholds : [];
  return {
    hsnThresholds: thresholds.map((t) => {
      const row = t as Record<string, unknown>;
      return {
        hsnCode: decodeId(row.hsnCode),
        classOneBps: toNumber(row.classOneBps),
        classTwoBps: toNumber(row.classTwoBps),
      };
    }),
    para3aApplicable: Boolean(json.para3aApplicable),
    pliLinked: Boolean(json.pliLinked),
    calculationMethod: decodeEnum(json.calculationMethod),
    preferenceMarginBps: toNumber(json.preferenceMarginBps),
    certificationThresholdPaise: toBigIntString(json.certificationThreshold),
    exemptionFloorPaise: toBigIntString(json.exemptionFloor),
    divisibility: decodeEnum(json.divisibility),
    effectiveFrom: toNumber(json.effectiveFrom),
  };
}

export async function GET(): Promise<Response> {
  try {
    const api = await getApi();
    const currentBlock = (await api.rpc.chain.getHeader()).number.toNumber();

    const [entries, defaultRaw] = await Promise.all([
      api.query.pramaanRuleRegistry.rules.entries(),
      api.query.pramaanRuleRegistry.defaultRule(),
    ]);

    const defaultRule =
      defaultRaw && !(defaultRaw as unknown as { isNone: boolean }).isNone
        ? decodeRule(
            (defaultRaw as unknown as { unwrap(): { toJSON(): unknown } })
              .unwrap()
              .toJSON() as Record<string, unknown>,
          )
        : null;

    const ministries = await Promise.all(
      entries.map(async ([key, value]) => {
        const ministryId = decodeId(key.args[0].toHex());
        const version = await api.query.pramaanRuleRegistry.ruleVersion(key.args[0]);
        const rule = decodeRule(
          (value as unknown as { unwrap(): { toJSON(): unknown } })
            .unwrap()
            .toJSON() as Record<string, unknown>,
        );
        const commenced = rule.effectiveFrom <= currentBlock;
        return {
          ministryId,
          ruleVersion: toNumber(version.toString()),
          rule,
          /** Has this ministry's own rule commenced, or is the default still governing? */
          commenced,
          effectiveInForce: commenced ? rule : defaultRule,
        };
      }),
    );

    ministries.sort((a, b) => a.ministryId.localeCompare(b.ministryId));

    return Response.json({
      currentBlock,
      ministriesOnboarded: ministries.length,
      defaultRule,
      ministries,
      source: {
        ministries: 'pramaanRuleRegistry.rules',
        versions: 'pramaanRuleRegistry.ruleVersion',
        default: 'pramaanRuleRegistry.defaultRule',
      },
    });
  } catch (error) {
    return Response.json(
      { error: error instanceof Error ? error.message : 'Unknown error' },
      { status: 500 },
    );
  }
}
