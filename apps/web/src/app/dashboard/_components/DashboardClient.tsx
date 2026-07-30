'use client';

import Link from 'next/link';
import {
  Chip,
  Figure,
  FigureRow,
  Notice,
  Panel,
  PanelHead,
  PanelNote,
  SkeletonRows,
  StaleBanner,
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
      <Notice
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
    <div className="space-y-4">
      {metrics.error && (
        <StaleBanner message={`The most recent refresh did not complete: ${metrics.error}`} />
      )}

      {/* --- Counter row (spec Part 9.6) ------------------------------------------------ */}
      <FigureRow>
        <Figure
          label="Ministries onboarded"
          value={counters.ministriesOnboarded.toLocaleString('en-IN')}
          note="Rule sets registered in pramaanRuleRegistry."
        />
        <Figure
          label="Declarations recorded"
          value={counters.totalDeclarations.toLocaleString('en-IN')}
          note={`Across ${counters.declarationPairs.toLocaleString('en-IN')} vendor–product histories.`}
        />
        <Figure
          label="Inconsistencies flagged"
          value={counters.inconsistenciesFlagged.toLocaleString('en-IN')}
          note={`Declarations more than ${formatBps(counters.toleranceBps)} apart for the same product.`}
        />
        <Figure
          label="Certificates issued"
          value={counters.certificatesIssued.toLocaleString('en-IN')}
          note={`${counters.preferenceDecisions.toLocaleString('en-IN')} preference decisions on record.`}
        />
      </FigureRow>

      {/* --- Charts --------------------------------------------------------------------- */}
      <div className="grid gap-4 xl:grid-cols-2">
        <VerdictChart
          entries={verdicts.entries}
          sessionStartedAt={verdicts.sessionStartedAt}
          totals={verdicts.totals}
          chainTotals={verdicts.chainTotals}
          now={metrics.updatedAt ?? verdicts.sessionStartedAt}
        />
        <DebarmentChart byMinistry={debarmentsByMinistry} />
      </div>

      {latency.data ? (
        <LatencyChart byTrigger={latency.data.byTrigger} />
      ) : (
        <Notice
          kind="unknown"
          title="Latency measurements are unavailable"
          detail="The session's measured trigger-point latencies could not be read. No figure is shown rather than a placeholder one."
          technicalDetail={latency.error ?? undefined}
          onRetry={latency.refresh}
        />
      )}

      {/* --- Ministry rule sets ---------------------------------------------------------- */}
      <Panel>
        <PanelHead
          title="Ministry rule sets on chain"
          meta={
            <Link href="/explorer" className={buttonClasses({ variant: 'default' })}>
              Open the explorer
            </Link>
          }
        />
        <Table>
          <THead>
            <TR>
              <TH className="w-48">Ministry</TH>
              <TH numeric className="w-32">Rule version</TH>
              <TH>Amended since genesis</TH>
            </TR>
          </THead>
          <TBody>
            {ministries.map((ministry) => (
              <TR key={ministry.ministryId} className="hover:bg-shell">
                <TD mono>{ministry.ministryId}</TD>
                <TD numeric>v{ministry.ruleVersion}</TD>
                <TD>
                  {ministry.ruleVersion > 1 ? (
                    <Chip tone="accent">
                      {ministry.ruleVersion - 1} amendment
                      {ministry.ruleVersion - 1 === 1 ? '' : 's'}
                    </Chip>
                  ) : (
                    <span className="text-2xs text-ink-subtle">Genesis rule unchanged</span>
                  )}
                </TD>
              </TR>
            ))}
          </TBody>
        </Table>
        <PanelNote>
          Each ministry owns its own thresholds, calculation method and Para 3A position. The
          version number rises every time a nodal administrator amends the rule, and every past
          version stays queryable.
        </PanelNote>
      </Panel>

      <Panel>
        <PanelNote className="flex flex-wrap items-center justify-between gap-x-6 gap-y-1 border-t-0">
          <span>
            Every figure on this page is a live storage read or an event decoded from a finalized
            block. Session counts start when this console connects to the network; chain-wide
            counts are the totals the chain holds right now.
          </span>
          <span className="font-mono whitespace-nowrap">
            head #{chain.currentBlock.toLocaleString('en-IN')} · finalized #
            {chain.finalizedBlock.toLocaleString('en-IN')}
          </span>
        </PanelNote>
      </Panel>
    </div>
  );
}

function DashboardSkeleton() {
  return (
    <div className="space-y-4">
      <Panel>
        <SkeletonRows rows={4} label="Reading compliance analytics from the chain." />
      </Panel>
      <div className="grid gap-4 xl:grid-cols-2">
        {[0, 1].map((key) => (
          <Panel key={key}>
            <PanelHead title="Reading the chain" />
            <SkeletonRows rows={8} label="Reading chart data." />
          </Panel>
        ))}
      </div>
    </div>
  );
}
