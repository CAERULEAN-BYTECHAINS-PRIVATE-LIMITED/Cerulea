'use client';

import { ArrowRight, Landmark, LayoutDashboard, Radar } from 'lucide-react';
import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  Stat,
  Table,
  TBody,
  TCaption,
  TD,
  TH,
  THead,
  TR,
  buttonClasses,
} from '@/components';
import {
  MINISTRIES,
  MINISTRY_COUNT,
  readLedger,
  subscribeConsoleRecord,
  type LedgerEntry,
  type TriState,
} from '@/lib/api-client';
import { formatBps, formatPaise } from '@/lib/units';

/**
 * DPIIT's national view.
 *
 * **The rule table is read from the chain, not from a bundled constant.** It previously
 * rendered `MINISTRIES` — a client-side literal — as though it were registry state, with
 * a comment claiming "21 of 21 cannot drift". It could and did: a QA pass found five
 * ministries showing an HSN code the chain contradicted, and any `rule-update` left the
 * table showing superseded values while the chain held the new ones. That is reachable
 * in two clicks from the scripted walkthrough (amend a rule at step 6, open this page).
 *
 * `MINISTRIES` is still used, but only for the human-readable ministry NAMES, which are
 * reference metadata and are not stored on chain. Every rule parameter — thresholds,
 * method, margin, certification threshold, divisibility — comes from
 * `/api/chain/rules`, which reads `pramaanRuleRegistry`.
 *
 * The verdict distribution below is this session's decisions and says so: a national
 * compliance rate is a claim only the chain can make, and the explorer is where it is
 * checked.
 */

/** Rule parameters exactly as `/api/chain/rules` reports them. */
interface ChainRuleRow {
  ministryId: string;
  ruleVersion: number;
  commenced: boolean;
  rule: {
    hsnThresholds: { hsnCode: string; classOneBps: number; classTwoBps: number }[];
    para3aApplicable: boolean;
    pliLinked: boolean;
    calculationMethod: string;
    preferenceMarginBps: number;
    certificationThresholdPaise: string;
    exemptionFloorPaise: string;
    divisibility: string;
    effectiveFrom: number;
  };
}

interface ChainRulesResponse {
  currentBlock: number;
  ministriesOnboarded: number;
  defaultRule: ChainRuleRow['rule'] | null;
  ministries: ChainRuleRow[];
}

/** Display metadata only. Never a source of rule values. */
const MINISTRY_NAMES = new Map(MINISTRIES.map((m) => [m.id, { short: m.short, name: m.name }]));

export function NationalRollup() {
  const [ledger, setLedger] = useState<LedgerEntry[]>([]);
  const [chain, setChain] = useState<ChainRulesResponse | null>(null);
  const [chainError, setChainError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const load = async () => {
      try {
        const response = await fetch('/api/chain/rules', { cache: 'no-store' });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const body: ChainRulesResponse = await response.json();
        if (!cancelled) {
          setChain(body);
          setChainError(null);
        }
      } catch (error) {
        if (!cancelled) setChainError(error instanceof Error ? error.message : 'unavailable');
      }
    };
    void load();
    const timer = setInterval(() => void load(), 10_000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    const sync = () => setLedger(readLedger());
    sync();
    return subscribeConsoleRecord(sync);
  }, []);

  const distribution = useMemo(() => {
    const counts = { GREEN: 0, YELLOW: 0, RED: 0 } as Record<TriState, number>;
    for (const entry of ledger) counts[entry.result] += 1;
    return counts;
  }, [ledger]);

  const total = distribution.GREEN + distribution.YELLOW + distribution.RED;
  const ministriesTouched = new Set(ledger.map((entry) => entry.ministry).filter(Boolean)).size;

  const chainRows = chain?.ministries ?? [];
  const chainDefault = chain?.defaultRule ?? null;
  const onboarded = chain?.ministriesOnboarded ?? null;
  const variances = chainRows.filter((row) => deviations(row.rule, chainDefault).length > 0).length;

  return (
    <div className="space-y-8">
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="Ministries onboarded"
          value={onboarded === null ? '—' : `${onboarded} of ${MINISTRY_COUNT}`}
          hint="Counted from pramaanRuleRegistry on chain, not from a bundled list."
          icon={<Landmark className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Rule sets differing from the default"
          value={String(variances)}
          hint="Ministries that have notified their own parameters."
        />
        <Stat
          label="Decisions this session"
          value={String(total)}
          hint="Trigger-point answers returned after finality."
        />
        <Stat
          label="Ministries exercised this session"
          value={String(ministriesTouched)}
          hint="Distinct rule sets a decision has been taken against."
          icon={<Radar className="size-3.5" aria-hidden="true" />}
        />
      </section>

      {/* ---- Distribution ------------------------------------------------------ */}
      <Card>
        <CardHeader
          title="Compliance outcome distribution"
          description="Every verdict this console has seen returned, across all six trigger points."
          actions={
            <Link
              href="/dashboard"
              className={buttonClasses({ variant: 'primary', size: 'sm' })}
            >
              <LayoutDashboard className="size-4" aria-hidden="true" />
              Open the national dashboard
            </Link>
          }
        />
        <CardBody>
          {total === 0 ? (
            <EmptyState
              icon={<Radar className="size-5" aria-hidden="true" />}
              title="No decisions recorded in this session yet"
              description="The distribution fills as bids are classified, preferences calculated, certificates issued and debarments recorded. The dashboard reads the chain directly and does not depend on this session."
              action={
                <Link
                  href="/vendor"
                  className={buttonClasses({ variant: 'secondary', size: 'sm' })}
                >
                  Start with a bid submission
                  <ArrowRight className="size-4" aria-hidden="true" />
                </Link>
              }
            />
          ) : (
            <div className="space-y-5">
              <div
                className="flex h-3 w-full overflow-hidden rounded-full bg-surface-sunken"
                role="img"
                aria-label={`${distribution.GREEN} compliant, ${distribution.YELLOW} review required, ${distribution.RED} blocked, out of ${total} decisions.`}
              >
                <span
                  className="bg-status-green transition-[width] duration-200 ease-out"
                  style={{ width: `${(distribution.GREEN / total) * 100}%` }}
                />
                <span
                  className="bg-status-yellow transition-[width] duration-200 ease-out"
                  style={{ width: `${(distribution.YELLOW / total) * 100}%` }}
                />
                <span
                  className="bg-status-red transition-[width] duration-200 ease-out"
                  style={{ width: `${(distribution.RED / total) * 100}%` }}
                />
              </div>

              <dl className="grid grid-cols-1 gap-4 sm:grid-cols-3">
                <DistributionCell
                  token="GREEN"
                  word="Compliant"
                  count={distribution.GREEN}
                  total={total}
                />
                <DistributionCell
                  token="YELLOW"
                  word="Review required"
                  count={distribution.YELLOW}
                  total={total}
                />
                <DistributionCell
                  token="RED"
                  word="Blocked"
                  count={distribution.RED}
                  total={total}
                />
              </dl>

              <p className="text-xs text-ink-muted">
                Counted from decisions returned to this console. The chain is the record; the
                explorer reads it.
              </p>
            </div>
          )}
        </CardBody>
      </Card>

      {/* ---- The registry ------------------------------------------------------- */}
      <Card>
        <CardHeader
          title="Ministry rule sets against the national default"
          description="DPIIT owns the default every ministry inherits. A ministry that has notified its own parameters shows what it changed."
        />
        <Table>
          <THead>
            <TR>
              <TH>Ministry</TH>
              <TH>Id</TH>
              <TH>Class-I / Class-II</TH>
              <TH>Method</TH>
              <TH className="text-right">Margin</TH>
              <TH className="text-right">Certification threshold</TH>
              <TH>Divisibility</TH>
              <TH>Differs from the default</TH>
            </TR>
          </THead>
          <TBody>
            {chainRows.map((entry) => {
              const row = entry.rule;
              const meta = MINISTRY_NAMES.get(entry.ministryId);
              const differences = deviations(row, chainDefault);
              const isDefault = entry.ministryId === 'DPIIT';
              const hsn = row.hsnThresholds[0];
              return (
                <TR key={entry.ministryId} className={isDefault ? 'bg-cerulea-light/40' : undefined}>
                  <TD>
                    <span className="font-medium text-ink">{meta?.short ?? entry.ministryId}</span>
                    <span className="mt-0.5 block text-xs text-ink-muted">
                      {meta?.name ?? 'Name not held on chain'}
                    </span>
                  </TD>
                  <TD mono>
                    {entry.ministryId}
                    <span className="mt-0.5 block font-mono text-[0.6875rem] text-ink-subtle">
                      v{entry.ruleVersion}
                      {!entry.commenced ? ' · not yet in force' : ''}
                    </span>
                  </TD>
                  <TD className="tabular-nums">
                    {hsn ? `${formatBps(hsn.classOneBps)} / ${formatBps(hsn.classTwoBps)}` : '—'}
                    <span className="mt-0.5 block font-mono text-[0.6875rem] text-ink-subtle">
                      HSN {hsn?.hsnCode ?? '—'}
                    </span>
                  </TD>
                  <TD>{row.calculationMethod}</TD>
                  <TD className="text-right tabular-nums">{formatBps(row.preferenceMarginBps)}</TD>
                  <TD className="text-right tabular-nums">
                    {formatPaise(row.certificationThresholdPaise)}
                  </TD>
                  <TD>{row.divisibility}</TD>
                  <TD>
                    {isDefault ? (
                      <Badge tone="brand">The national default</Badge>
                    ) : differences.length === 0 ? (
                      <span className="text-sm text-ink-muted">Inherits the default</span>
                    ) : (
                      <span className="flex flex-wrap gap-1">
                        {differences.map((difference) => (
                          <Badge key={difference} tone="neutral" size="sm">
                            {difference}
                          </Badge>
                        ))}
                      </span>
                    )}
                  </TD>
                </TR>
              );
            })}
          </TBody>
          <TCaption>
            {chainError
              ? `Rule parameters could not be read from the chain (${chainError}). Nothing is shown rather than showing values that may be stale.`
              : `Read live from pramaanRuleRegistry at block ${chain?.currentBlock ?? '—'}, refreshed every ten seconds. Ministry names are local reference data; every rule parameter above is on-chain state. Adding the twenty-second ministry is a row in scripts/seed-ministries/ministries.json, not a release.`}
          </TCaption>
        </Table>
      </Card>
    </div>
  );
}

// -------------------------------------------------------------------------------------

function DistributionCell({
  token,
  word,
  count,
  total,
}: {
  token: TriState;
  word: string;
  count: number;
  total: number;
}) {
  const chip =
    token === 'GREEN'
      ? 'bg-status-green text-white'
      : token === 'YELLOW'
        ? 'bg-status-yellow text-ink'
        : 'bg-status-red text-white';

  return (
    <div className="rounded-lg border border-border px-4 py-3">
      <dt className="flex items-center gap-2">
        <span
          className={`rounded px-1.5 py-0.5 font-mono text-[0.6875rem] font-bold tracking-widest ${chip}`}
        >
          {token}
        </span>
        <span className="text-sm font-medium text-ink">{word}</span>
      </dt>
      <dd className="mt-2 text-2xl font-semibold tabular-nums text-ink">
        {count}
        <span className="ml-2 text-sm font-normal text-ink-muted">
          {total === 0 ? '' : `${Math.round((count / total) * 100)}%`}
        </span>
      </dd>
    </div>
  );
}

/** What a ministry has notified that the national default does not say. */
type RuleShape = ChainRuleRow['rule'];

/** Which parameters this ministry has notified differently from the national default. */
function deviations(row: RuleShape, defaultRule: RuleShape | null): string[] {
  if (!defaultRule) return [];
  const differences: string[] = [];
  const defaultHsn = defaultRule.hsnThresholds[0];
  const hsn = row.hsnThresholds[0];
  if (!hsn || !defaultHsn) return differences;

  if (hsn.hsnCode !== defaultHsn.hsnCode) differences.push(`HSN ${hsn.hsnCode}`);
  if (hsn.classOneBps !== defaultHsn.classOneBps) {
    differences.push(`Class-I ${formatBps(hsn.classOneBps)}`);
  }
  if (hsn.classTwoBps !== defaultHsn.classTwoBps) {
    differences.push(`Class-II ${formatBps(hsn.classTwoBps)}`);
  }
  if (row.calculationMethod !== defaultRule.calculationMethod) {
    differences.push(row.calculationMethod);
  }
  if (row.divisibility !== defaultRule.divisibility) differences.push(row.divisibility);
  if (row.para3aApplicable !== defaultRule.para3aApplicable) {
    differences.push(row.para3aApplicable ? 'Para 3A applies' : 'Para 3A does not apply');
  }
  if (row.pliLinked !== defaultRule.pliLinked) {
    differences.push(row.pliLinked ? 'PLI-linked' : 'Not PLI-linked');
  }
  if (row.preferenceMarginBps !== defaultRule.preferenceMarginBps) {
    differences.push(`Margin ${formatBps(row.preferenceMarginBps)}`);
  }
  if (row.certificationThresholdPaise !== defaultRule.certificationThresholdPaise) {
    differences.push(`Threshold ${formatPaise(row.certificationThresholdPaise)}`);
  }
  return differences;
}
