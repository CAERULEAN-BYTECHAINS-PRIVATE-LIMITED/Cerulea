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
  type MinistryRef,
  type TriState,
} from '@/lib/api-client';
import { formatBps, formatPaise } from '@/lib/units';

/**
 * DPIIT's national view.
 *
 * The ministry count is read from the same list the seed script writes to the chain, so
 * "21 of 21" cannot drift from what is actually onboarded. The distribution below is
 * this session's decisions and says so — a national compliance rate is a claim only the
 * chain can make, and the explorer is where that claim is checked.
 */

const DEFAULT_RULE = MINISTRIES[0];

export function NationalRollup() {
  const [ledger, setLedger] = useState<LedgerEntry[]>([]);

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
  const variances = MINISTRIES.filter((row) => deviations(row).length > 0).length;

  return (
    <div className="space-y-8">
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="Ministries onboarded"
          value={`${MINISTRY_COUNT} of ${MINISTRY_COUNT}`}
          hint="Each carrying its own rule set in the registry."
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
            {MINISTRIES.map((row) => {
              const differences = deviations(row);
              const isDefault = row.id === DEFAULT_RULE.id;
              return (
                <TR key={row.id} className={isDefault ? 'bg-cerulea-light/40' : undefined}>
                  <TD>
                    <span className="font-medium text-ink">{row.short}</span>
                    <span className="mt-0.5 block text-xs text-ink-muted">{row.name}</span>
                  </TD>
                  <TD mono>{row.id}</TD>
                  <TD className="tabular-nums">
                    {formatBps(row.hsnThresholds[0].classOneBps)} /{' '}
                    {formatBps(row.hsnThresholds[0].classTwoBps)}
                    <span className="mt-0.5 block font-mono text-[0.6875rem] text-ink-subtle">
                      HSN {row.hsnThresholds[0].hsnCode}
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
            {MINISTRY_COUNT} rule sets, seeded from scripts/seed-ministries/ministries.json — the
            same file the seed script writes to the chain. Adding the twenty-second is a row in
            that file, not a release.
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
function deviations(row: MinistryRef): string[] {
  const differences: string[] = [];
  const defaultHsn = DEFAULT_RULE.hsnThresholds[0];
  const hsn = row.hsnThresholds[0];

  if (hsn.hsnCode !== defaultHsn.hsnCode) differences.push(`HSN ${hsn.hsnCode}`);
  if (hsn.classOneBps !== defaultHsn.classOneBps) {
    differences.push(`Class-I ${formatBps(hsn.classOneBps)}`);
  }
  if (hsn.classTwoBps !== defaultHsn.classTwoBps) {
    differences.push(`Class-II ${formatBps(hsn.classTwoBps)}`);
  }
  if (row.calculationMethod !== DEFAULT_RULE.calculationMethod) {
    differences.push(row.calculationMethod);
  }
  if (row.divisibility !== DEFAULT_RULE.divisibility) differences.push(row.divisibility);
  if (row.para3aApplicable !== DEFAULT_RULE.para3aApplicable) {
    differences.push(row.para3aApplicable ? 'Para 3A applies' : 'Para 3A does not apply');
  }
  if (row.pliLinked !== DEFAULT_RULE.pliLinked) {
    differences.push(row.pliLinked ? 'PLI-linked' : 'Not PLI-linked');
  }
  if (row.preferenceMarginBps !== DEFAULT_RULE.preferenceMarginBps) {
    differences.push(`Margin ${formatBps(row.preferenceMarginBps)}`);
  }
  if (row.certificationThresholdPaise !== DEFAULT_RULE.certificationThresholdPaise) {
    differences.push(`Threshold ${formatPaise(row.certificationThresholdPaise)}`);
  }
  return differences;
}
