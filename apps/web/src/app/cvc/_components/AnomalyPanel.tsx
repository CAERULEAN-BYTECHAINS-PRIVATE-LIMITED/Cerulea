'use client';

import { useEffect, useState } from 'react';
import {
  Chip,
  DataList,
  DataRow,
  Empty,
  Figure,
  FigureRow,
  Hash,
  Nil,
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  SectionHead,
  SkeletonRows,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
} from '@/components';
import { vendorName } from '@/lib/api-client';

/**
 * The anomaly detection surface on the CVC console.
 *
 * Every number and every row here comes from `GET /api/chain/anomalies`, which reads chain
 * storage. Nothing on this panel is read from localStorage, and there is no sample data
 * path: a signal with no hits renders an empty state naming what would fill it.
 *
 * The one thing this component must get right is the distinction between the signal the
 * network's own consensus rules raise and the four review heuristics computed after the
 * fact. The API carries that on every finding as `origin`; the UI restates it on every row
 * rather than only in the introduction.
 *
 * Severity uses `strong` / `accent` / `neutral` tones deliberately. `green` / `yellow` /
 * `red` are reserved for compliance verdicts under the PPP-MII Order, and an anomaly is a
 * prompt to look, never a verdict.
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
const SEVERITY_TONE: Record<Severity, 'strong' | 'accent' | 'neutral'> = {
  critical: 'strong',
  elevated: 'accent',
  watch: 'neutral',
};

function OriginChip({ origin }: { origin: Origin }) {
  return (
    <Chip tone={origin === 'on-chain' ? 'accent' : 'neutral'}>
      {origin === 'on-chain' ? 'Consensus finding' : 'Review heuristic'}
    </Chip>
  );
}

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
    <section aria-labelledby="anomaly-heading" className="space-y-3">
      <SectionHead
        id="anomaly-heading"
        title="Anomaly detection"
        meta={<Chip tone="accent">Rule-based, computed live</Chip>}
      />

      {loading && !report && !error && (
        <Panel>
          <SkeletonRows rows={4} label="Reading declarations, classifications, debarments and certificates from the chain" />
        </Panel>
      )}

      {error && (
        <Empty
          title="The anomaly surface could not read the chain"
          source={`No findings are shown rather than stale ones. The node reported: ${error}`}
        />
      )}

      {report && (
        <>
          <FigureRow>
            <Figure
              label="Findings"
              value={report.counters.total}
              note={`${report.counters.vendorsFlagged} distinct ${report.counters.vendorsFlagged === 1 ? 'vendor' : 'vendors'} named.`}
            />
            <Figure
              label="Raised by consensus"
              value={report.counters.onChain}
              note="Flagged by the runtime's own rules, not by this console."
            />
            <Figure
              label="Review heuristics"
              value={report.counters.heuristic}
              note="Patterns computed here over finalized records. Not verdicts."
            />
            <Figure
              label="Records scanned"
              value={report.counters.recordsScanned.toLocaleString('en-IN')}
              note={`Chain head #${report.chain.currentBlock.toLocaleString('en-IN')}, finalized #${report.chain.finalizedBlock.toLocaleString('en-IN')}.`}
            />
          </FigureRow>

          {/* ---- What each signal is, and how it is computed --------------------- */}
          <Panel>
            <PanelHead title="The five signals" />
            <Table>
              <THead>
                <TR>
                  <TH className="w-64">Signal</TH>
                  <TH className="w-44">Origin</TH>
                  <TH numeric className="w-24">Findings</TH>
                  <TH>Rule applied, and the storage it reads</TH>
                </TR>
              </THead>
              <TBody>
                {report.signals.map((signal) => (
                  <TR key={signal.id}>
                    <TD className="font-medium text-ink">{signal.label}</TD>
                    <TD>
                      <OriginChip origin={signal.origin} />
                    </TD>
                    <TD numeric>
                      {signal.count}
                      {signal.truncated > 0 && (
                        <span className="block font-sans text-2xs text-ink-muted">
                          {signal.truncated} not shown
                        </span>
                      )}
                    </TD>
                    <TD className="text-ink-muted">
                      {signal.method}
                      {report.sources[signal.id] && (
                        <span className="mt-0.5 block font-mono text-2xs">
                          {report.sources[signal.id]}
                        </span>
                      )}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
            <PanelNote>
              A <span className="font-medium text-ink">consensus finding</span> is one the network
              itself raised: every validator ran the same rule over the same declaration and
              agreed. A <span className="font-medium text-ink">review heuristic</span> is computed
              in this console over records the chain has already finalized — a pattern worth a
              reviewer&apos;s attention, not a ruling. {report.model.name} — {report.model.status}.{' '}
              {report.model.note}
            </PanelNote>
          </Panel>

          {/* ---- The findings ---------------------------------------------------- */}
          {report.findings.length === 0 ? (
            <Empty
              title="No anomaly raised across the records currently on chain"
              source="All five signals ran and none matched. A finding appears when a vendor contradicts its own declaration for a product, sits repeatedly on a classification boundary, collects several Non-local outcomes, transacts inside an active debarment window, or certifies contracts just below a ministry's certification threshold."
            />
          ) : (
            <ul className="space-y-3">
              {report.findings.map((finding) => (
                <li key={finding.id}>
                  <FindingPanel finding={finding} />
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
 * chain were never in the roster. The full address is printed unaltered in the panel body,
 * so nothing is hidden; this only stops a 48-character key from becoming the headline.
 */
function vendorLabel(account: string): string {
  const resolved = vendorName(account);
  if (resolved !== account) return resolved;
  return `${account.slice(0, 6)}…${account.slice(-5)}`;
}

function FindingPanel({ finding }: { finding: Finding }) {
  return (
    <Panel>
      <PanelHead
        title={`${vendorLabel(finding.vendor)} — ${finding.headline}`}
        meta={
          <>
            <OriginChip origin={finding.origin} />
            <Chip tone={SEVERITY_TONE[finding.severity]}>{SEVERITY_LABEL[finding.severity]}</Chip>
          </>
        }
      />
      <PanelBody>
        <p className="max-w-[75ch] text-sm text-ink">{finding.evidence}</p>
        <DataList className="mt-3" columns={2}>
          {finding.facts.map((fact) => (
            <DataRow key={fact.label} label={fact.label} value={fact.value || <Nil />} />
          ))}
          <DataRow
            label="Verifiable at"
            value={
              finding.blockNumber === null
                ? 'Chain storage — no block number'
                : `#${finding.blockNumber.toLocaleString('en-IN')}`
            }
            mono={finding.blockNumber !== null}
          />
          <DataRow
            label="Transaction"
            value={
              finding.txRef ? (
                <Hash value={finding.txRef} />
              ) : (
                <Nil label="Outside the indexed window — trace by block" />
              )
            }
          />
        </DataList>
      </PanelBody>
      <PanelNote>
        {finding.signalLabel} · vendor account <span className="font-mono">{finding.vendor}</span>
      </PanelNote>
    </Panel>
  );
}
