'use client';

import { Activity, AlertTriangle, Radar, ScanSearch, ShieldCheck } from 'lucide-react';
import { useEffect, useState } from 'react';
import {
  Badge,
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  DataRow,
  EmptyState,
  Stat,
  TxRef,
} from '@/components';
import { vendorName } from '@/lib/api-client';

/**
 * The anomaly detection surface on the CVC console.
 *
 * Every number and every row here comes from `GET /api/chain/anomalies`, which reads chain
 * storage. Nothing on this panel is read from localStorage, and there is no sample data
 * path: a signal with no hits renders an empty state naming what would fill it.
 *
 * The one thing this component must get right, and the reason the copy below is as long as
 * it is, is the distinction between the signal the network's own consensus rules raise and
 * the four review heuristics computed after the fact. The API carries that on every finding
 * as `origin`; the UI restates it on every row rather than only in the introduction.
 *
 * Severity uses `brand` / `teal` / `neutral` tones deliberately. `green` / `yellow` / `red`
 * are reserved by the build contract for compliance verdicts under the PPP-MII Order, and
 * an anomaly is a prompt to look, never a verdict.
 */

interface Fact {
  label: string;
  value: string;
}

type Severity = 'critical' | 'elevated' | 'watch';
type Origin = 'on-chain' | 'heuristic';

interface Finding {
  id: string;
  signal: string;
  signalLabel: string;
  origin: Origin;
  severity: Severity;
  vendor: string;
  headline: string;
  evidence: string;
  blockNumber: number | null;
  txRef: string | null;
  facts: Fact[];
}

interface SignalSummary {
  id: string;
  label: string;
  origin: Origin;
  method: string;
  count: number;
  truncated: number;
}

interface AnomalyReport {
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
}

const SEVERITY_LABEL: Record<Severity, string> = {
  critical: 'Critical',
  elevated: 'Elevated',
  watch: 'Watch',
};

/** Reserved verdict colours are not available here; see the note at the top of the file. */
const SEVERITY_TONE: Record<Severity, 'brand' | 'teal' | 'neutral'> = {
  critical: 'brand',
  elevated: 'teal',
  watch: 'neutral',
};

export function AnomalyPanel() {
  const [report, setReport] = useState<AnomalyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let live = true;
    const load = async () => {
      try {
        const response = await fetch('/api/chain/anomalies', { cache: 'no-store' });
        const body = (await response.json()) as AnomalyReport & { error?: string };
        if (!live) return;
        if (!response.ok || body.error) {
          setError(body.error ?? `The anomaly route answered ${response.status}.`);
          setReport(null);
        } else {
          setReport(body);
          setError(null);
        }
      } catch (cause) {
        if (live) setError(cause instanceof Error ? cause.message : 'Unable to reach the node.');
      } finally {
        if (live) setLoading(false);
      }
    };
    void load();
    const timer = setInterval(load, 15_000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, []);

  return (
    <section aria-labelledby="anomaly-heading" className="space-y-4">
      <div className="flex flex-wrap items-end justify-between gap-3">
        <div>
          <h2 id="anomaly-heading" className="text-xl font-semibold tracking-tight text-ink">
            Anomaly detection
          </h2>
          <p className="mt-1.5 max-w-3xl text-sm leading-relaxed text-ink-muted">
            Two different things are shown below and the difference matters. A{' '}
            <strong className="font-semibold text-ink">consensus finding</strong> is one the
            network itself raised: every validator ran the same rule over the same declaration
            and agreed, which is why a contradicted declaration cannot be quietly withdrawn. A{' '}
            <strong className="font-semibold text-ink">review heuristic</strong> is computed
            here, in this console, over records the chain has already finalized — a pattern
            worth a reviewer&apos;s attention, not a ruling, and not something the chain
            endorses. Everything on this panel is read from chain storage each time the page
            loads; nothing is scored, trained or estimated.
          </p>
        </div>
        <Badge tone="brand" icon={<Radar className="size-3.5" aria-hidden="true" />}>
          Rule-based, computed live
        </Badge>
      </div>

      {loading && !report && !error && (
        <Card>
          <CardBody>
            <p className="text-sm text-ink-muted">Reading declarations, classifications, debarments and certificates from the chain…</p>
          </CardBody>
        </Card>
      )}

      {error && (
        <EmptyState
          icon={<AlertTriangle className="size-5" aria-hidden="true" />}
          title="The anomaly surface could not read the chain"
          description={`No findings are shown rather than stale ones. The node reported: ${error}`}
        />
      )}

      {report && (
        <>
          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
            <Stat
              label="Findings"
              value={String(report.counters.total)}
              hint={`${report.counters.vendorsFlagged} distinct ${report.counters.vendorsFlagged === 1 ? 'vendor' : 'vendors'} named.`}
              icon={<ScanSearch className="size-3.5" aria-hidden="true" />}
            />
            <Stat
              label="Raised by consensus"
              value={String(report.counters.onChain)}
              hint="Flagged by the runtime's own rules, not by this console."
              icon={<ShieldCheck className="size-3.5" aria-hidden="true" />}
            />
            <Stat
              label="Review heuristics"
              value={String(report.counters.heuristic)}
              hint="Patterns computed here over finalized records. Not verdicts."
            />
            <Stat
              label="Records scanned"
              value={report.counters.recordsScanned.toLocaleString('en-IN')}
              hint={`Chain head #${report.chain.currentBlock.toLocaleString('en-IN')}, finalized #${report.chain.finalizedBlock.toLocaleString('en-IN')}.`}
              icon={<Activity className="size-3.5" aria-hidden="true" />}
            />
          </div>

          {/* ---- What each signal is, and how it is computed --------------------- */}
          <Card>
            <CardHeader
              title="The five signals"
              description="Each one names the storage it reads and the rule it applies, so a finding can be checked rather than believed."
            />
            <CardBody className="space-y-3">
              {report.signals.map((signal) => (
                <div
                  key={signal.id}
                  className="rounded-lg border border-border bg-surface-sunken px-4 py-3"
                >
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="text-sm font-semibold text-ink">{signal.label}</span>
                    <Badge tone={signal.origin === 'on-chain' ? 'brand' : 'neutral'} size="sm">
                      {signal.origin === 'on-chain' ? 'Consensus finding' : 'Review heuristic'}
                    </Badge>
                    <span className="ml-auto text-sm font-semibold text-ink tabular-nums">
                      {signal.count} {signal.count === 1 ? 'finding' : 'findings'}
                      {signal.truncated > 0 && (
                        <span className="ml-1 text-xs font-normal text-ink-muted">
                          ({signal.truncated} not shown)
                        </span>
                      )}
                    </span>
                  </div>
                  <p className="mt-1.5 text-sm leading-relaxed text-ink-muted">{signal.method}</p>
                  {report.sources[signal.id] && (
                    <p className="mt-1 font-mono text-[0.75rem] text-ink-muted">
                      {report.sources[signal.id]}
                    </p>
                  )}
                </div>
              ))}
            </CardBody>
            <CardFooter>
              <p className="text-xs leading-relaxed text-ink-muted">
                <strong className="font-semibold text-ink">{report.model.name}</strong> —{' '}
                {report.model.status}. {report.model.note}
              </p>
            </CardFooter>
          </Card>

          {/* ---- The findings ---------------------------------------------------- */}
          {report.findings.length === 0 ? (
            <EmptyState
              icon={<ScanSearch className="size-5" aria-hidden="true" />}
              title="No anomaly raised across the records currently on chain"
              description="All five signals ran and none matched. A finding appears when a vendor contradicts its own declaration for a product, sits repeatedly on a classification boundary, collects several Non-local outcomes, transacts inside an active debarment window, or certifies contracts just below a ministry's certification threshold."
            />
          ) : (
            <ul className="space-y-4">
              {report.findings.map((finding) => (
                <li key={finding.id}>
                  <FindingCard finding={finding} />
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}

// -------------------------------------------------------------------------------------

/**
 * A readable name where the demo roster knows the account, and a shortened address where
 * it does not — the chain holds accounts, not names, and most of the vendors on a seeded
 * chain were never in the roster. The full address is printed unaltered in the card body,
 * so nothing is hidden; this only stops a 48-character key from becoming the headline.
 */
function vendorLabel(account: string): string {
  const resolved = vendorName(account);
  if (resolved !== account) return resolved;
  return `${account.slice(0, 6)}…${account.slice(-5)}`;
}

function FindingCard({ finding }: { finding: Finding }) {
  return (
    <Card>
      <CardHeader
        title={`${vendorLabel(finding.vendor)} — ${finding.headline}`}
        description={finding.signalLabel}
        actions={
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={finding.origin === 'on-chain' ? 'brand' : 'neutral'} size="sm">
              {finding.origin === 'on-chain' ? 'Consensus finding' : 'Review heuristic'}
            </Badge>
            <Badge tone={SEVERITY_TONE[finding.severity]}>
              {SEVERITY_LABEL[finding.severity]}
            </Badge>
          </div>
        }
      />
      <CardBody>
        <p className="text-sm leading-relaxed text-ink">{finding.evidence}</p>
        <dl className="mt-4">
          {finding.facts.map((fact) => (
            <DataRow key={fact.label} label={fact.label} value={fact.value || '—'} />
          ))}
          <DataRow
            label="Verifiable at"
            value={
              finding.blockNumber === null
                ? 'Chain storage — this record carries no block number'
                : `Block #${finding.blockNumber.toLocaleString('en-IN')}`
            }
            mono={finding.blockNumber !== null}
          />
          <DataRow
            label="Transaction"
            value={
              finding.txRef ? (
                <TxRef value={finding.txRef} head={8} tail={6} />
              ) : (
                'Outside the explorer’s indexed window — trace by block'
              )
            }
          />
          <DataRow label="Vendor account" value={finding.vendor} mono />
        </dl>
      </CardBody>
    </Card>
  );
}
