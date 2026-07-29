'use client';

import { Building2, FileWarning, Landmark, ScrollText } from 'lucide-react';
import Link from 'next/link';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  ErrorState,
  Stat,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  buttonClasses,
} from '@/components';
import { formatBps } from '@/lib/units';
import { usePolledResource } from '../../explorer/_components/usePolledResource';
import { DebarmentChart } from './DebarmentChart';
import { LatencyChart } from './LatencyChart';
import type { LatencyResponse, MetricsResponse } from './types';
import { VerdictChart } from './VerdictChart';

export function DashboardClient() {
  const metrics = usePolledResource<MetricsResponse>('/api/chain/metrics', 5_000);
  const latency = usePolledResource<LatencyResponse>('/api/chain/latency', 5_000);

  if (metrics.loading && !metrics.data) return <DashboardSkeleton />;

  if (!metrics.data) {
    return (
      <ErrorState
        kind="chain-unreachable"
        title="The dashboard could not read the chain"
        detail="None of the figures on this page are cached or precomputed — they are storage reads and decoded events — so with the network unreachable there is nothing honest to show."
        technicalDetail={metrics.error ?? undefined}
        onRetry={metrics.refresh}
      />
    );
  }

  const { counters, verdicts, debarmentsByMinistry, ministries, chain } = metrics.data;

  return (
    <div className="space-y-6">
      {metrics.error && (
        <p className="rounded-card border border-border bg-surface-sunken px-4 py-2.5 text-sm text-ink-muted">
          Showing the last figures read successfully. The most recent refresh did not
          complete: {metrics.error}
        </p>
      )}

      {/* --- Counter row (spec Part 9.6) ------------------------------------------------ */}
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="Ministries onboarded"
          value={counters.ministriesOnboarded.toLocaleString('en-IN')}
          hint="Rule sets registered in pramaanRuleRegistry"
          icon={<Landmark className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Declarations recorded"
          value={counters.totalDeclarations.toLocaleString('en-IN')}
          hint={`Across ${counters.declarationPairs.toLocaleString('en-IN')} vendor–product histories`}
          icon={<ScrollText className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Inconsistencies flagged"
          value={counters.inconsistenciesFlagged.toLocaleString('en-IN')}
          hint={`Declarations more than ${formatBps(counters.toleranceBps)} apart for the same product`}
          icon={<FileWarning className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Certificates issued"
          value={counters.certificatesIssued.toLocaleString('en-IN')}
          hint={`${counters.preferenceDecisions.toLocaleString('en-IN')} preference decisions on record`}
          icon={<Building2 className="size-3.5" aria-hidden="true" />}
        />
      </div>

      {/* --- Charts --------------------------------------------------------------------- */}
      <div className="grid gap-6 xl:grid-cols-2">
        <VerdictChart
          entries={verdicts.entries}
          sessionStartedAt={verdicts.sessionStartedAt}
          totals={verdicts.totals}
        />
        <DebarmentChart byMinistry={debarmentsByMinistry} />
      </div>

      {latency.data ? (
        <LatencyChart byTrigger={latency.data.byTrigger} />
      ) : (
        <ErrorState
          kind="unknown"
          title="Latency measurements are unavailable"
          detail="The session's measured trigger-point latencies could not be read. No figure is shown rather than a placeholder one."
          technicalDetail={latency.error ?? undefined}
          onRetry={latency.refresh}
        />
      )}

      {/* --- Ministry rule sets ---------------------------------------------------------- */}
      <Card>
        <CardHeader
          title="Ministry rule sets on chain"
          description="Each ministry owns its own thresholds, calculation method and Para 3A position. The version number rises every time a nodal administrator amends the rule, and every past version stays queryable."
          actions={
            <Link href="/explorer" className={buttonClasses({ variant: 'secondary', size: 'sm' })}>
              Open the explorer
            </Link>
          }
        />
        <Table containerClassName="rounded-b-card">
          <THead>
            <TR>
              <TH>Ministry</TH>
              <TH className="text-right">Rule version</TH>
              <TH>Amended since genesis</TH>
            </TR>
          </THead>
          <TBody>
            {ministries.map((ministry) => (
              <TR key={ministry.ministryId} className="hover:bg-surface-sunken">
                <TD mono>{ministry.ministryId}</TD>
                <TD className="text-right font-mono tabular-nums">v{ministry.ruleVersion}</TD>
                <TD>
                  {ministry.ruleVersion > 1 ? (
                    <Badge tone="brand" size="sm">
                      {ministry.ruleVersion - 1} amendment
                      {ministry.ruleVersion - 1 === 1 ? '' : 's'}
                    </Badge>
                  ) : (
                    <span className="text-sm text-ink-subtle">Genesis rule unchanged</span>
                  )}
                </TD>
              </TR>
            ))}
          </TBody>
        </Table>
      </Card>

      <Card>
        <CardBody className="flex flex-wrap items-center justify-between gap-x-8 gap-y-2 text-xs text-ink-muted">
          <p>
            Every figure on this page is a live storage read or an event decoded from a
            finalized block. Session counts start when this console connects to the network;
            chain-wide counts are the totals the chain holds right now.
          </p>
          <p className="font-mono whitespace-nowrap">
            head #{chain.currentBlock.toLocaleString('en-IN')} · finalized #
            {chain.finalizedBlock.toLocaleString('en-IN')}
          </p>
        </CardBody>
      </Card>
    </div>
  );
}

function DashboardSkeleton() {
  return (
    <div className="space-y-6" role="status" aria-live="polite">
      <span className="sr-only">Reading compliance analytics from the chain.</span>
      <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-4">
        {[0, 1, 2, 3].map((key) => (
          <div key={key} className="rounded-card border border-border bg-surface px-5 py-4">
            <div className="h-3 w-28 rounded bg-surface-sunken" />
            <div className="mt-3 h-7 w-16 rounded bg-surface-sunken" />
            <div className="mt-2 h-3 w-36 rounded bg-surface-sunken" />
          </div>
        ))}
      </div>
      <div className="grid gap-6 xl:grid-cols-2">
        {[0, 1].map((key) => (
          <div key={key} className="rounded-card border border-border bg-surface">
            <div className="border-b border-border px-5 py-4">
              <div className="h-4 w-52 rounded bg-surface-sunken" />
            </div>
            <div className="px-5 py-4">
              <div className="h-72 rounded bg-surface-sunken" />
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}
