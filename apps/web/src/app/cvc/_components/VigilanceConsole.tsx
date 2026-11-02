'use client';

import { ArrowRight } from 'lucide-react';
import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
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
  Select,
  StatusToken,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  buttonClasses,
} from '@/components';
import {
  VENDORS,
  getTender,
  ministryShort,
  readInconsistencies,
  readLedger,
  subscribeConsoleRecord,
  vendorName,
  type InconsistencyEntry,
  type LedgerEntry,
} from '@/lib/api-client';
import { formatBps } from '@/lib/units';
import { AnomalyPanel } from './AnomalyPanel';

/**
 * The CVC / audit reviewer's console: the record after the fact.
 *
 * This console never submits anything. It reads — which is exactly the role, and also why
 * the register below is empty until a declaration has actually been made somewhere else.
 * An empty register says so and says what produces an entry, rather than showing rows a
 * reviewer might mistake for findings.
 *
 * The tolerance is the runtime's own: `ToleranceBps = 1000`, ten percentage points. A
 * declaration that differs from the vendor's earlier one for the same product by more than
 * that is flagged, whichever pathway either declaration travelled.
 */

const TOLERANCE_BPS = 1_000;

/**
 * The four figures at the top of this console, read from `GET /api/chain/metrics`.
 *
 * They used to be counted out of localStorage, which meant a reviewer opening `/cvc` in a
 * fresh browser was shown four zeroes while `/dashboard` — reading the same chain — showed
 * a hundred and twenty declarations and four contradictions. Everything in this block is
 * now a chain read, and the register further down, which is still session-scoped, says so
 * on its face.
 */
interface ChainCounters {
  totalDeclarations: number;
  inconsistenciesFlagged: number;
  vendorProductPairsFlagged: number;
  toleranceBps: number;
}

export function VigilanceConsole() {
  const [flags, setFlags] = useState<InconsistencyEntry[]>([]);
  const [ledger, setLedger] = useState<LedgerEntry[]>([]);
  const [vendorFilter, setVendorFilter] = useState<string>('all');
  const [counters, setCounters] = useState<ChainCounters | null>(null);
  const [countersError, setCountersError] = useState<string | null>(null);

  useEffect(() => {
    const sync = () => {
      setFlags(readInconsistencies());
      setLedger(readLedger());
    };
    sync();
    return subscribeConsoleRecord(sync);
  }, []);

  useEffect(() => {
    let live = true;
    const load = async () => {
      try {
        const response = await fetch('/api/chain/metrics', { cache: 'no-store' });
        const body = (await response.json()) as {
          counters?: ChainCounters;
          error?: string;
        };
        if (!live) return;
        if (!response.ok || body.error || !body.counters) {
          setCountersError(body.error ?? `The metrics route answered ${response.status}.`);
          setCounters(null);
          return;
        }
        setCounters(body.counters);
        setCountersError(null);
      } catch (cause) {
        if (live) {
          setCountersError(cause instanceof Error ? cause.message : 'Unable to reach the node.');
          setCounters(null);
        }
      }
    };
    void load();
    const timer = setInterval(load, 15_000);
    return () => {
      live = false;
      clearInterval(timer);
    };
  }, []);

  /** Never a zero standing in for "not read yet" — an em dash says the read has not landed. */
  const chainValue = (pick: (counters: ChainCounters) => number): string =>
    counters ? pick(counters).toLocaleString('en-IN') : '—';
  const readNote = countersError ? ` Chain read failed: ${countersError}` : '';

  const declarations = ledger.filter(
    (entry) => entry.kind === 'classification' || entry.kind === 'evaluation',
  );

  const history = useMemo(() => {
    if (vendorFilter === 'all') return declarations;
    return declarations.filter((entry) => entry.vendor === vendorFilter);
  }, [declarations, vendorFilter]);

  const vendorsSeen = useMemo(() => {
    const accounts = new Set(declarations.map((entry) => entry.vendor).filter(Boolean));
    return VENDORS.filter((vendor) => accounts.has(vendor.account));
  }, [declarations]);

  return (
    <div className="space-y-5">
      {/* Reads the chain on every load, as do the four figures directly below it. */}
      <AnomalyPanel />

      <FigureRow>
        <Figure
          label="Inconsistency flags"
          value={chainValue((c) => c.inconsistenciesFlagged)}
          note={`Raised by the chain's own consistency rule, across every tender and ministry.${readNote}`}
        />
        <Figure
          label="Vendor–product histories flagged"
          value={chainValue((c) => c.vendorProductPairsFlagged)}
          note="Distinct vendor-and-product records on chain holding at least one contradiction."
        />
        <Figure
          label="Declarations on chain"
          value={chainValue((c) => c.totalDeclarations)}
          note="Every local-content declaration in pramaanConsistency.declarations."
        />
        <Figure
          label="Tolerance"
          value={formatBps(counters?.toleranceBps ?? TOLERANCE_BPS)}
          note={
            counters
              ? "The runtime's ToleranceBps, read from the chain's own constants."
              : "The runtime's ToleranceBps. Beyond it, a declaration is flagged."
          }
        />
      </FigureRow>

      {/* ---- The register ------------------------------------------------------ */}
      <section aria-labelledby="register-heading" className="space-y-3">
        <SectionHead
          id="register-heading"
          title="Cross-tender inconsistency register"
          meta={<Chip>This session only</Chip>}
        />

        {flags.length === 0 ? (
          <Empty
            title="No inconsistency flagged yet in this session"
            source="A flag is raised when a vendor declares a materially different local content figure for the same product on a second tender. The national count is the figure above; every contradiction on chain is listed in the anomaly panel."
            action={
              <Link href="/vendor" className={buttonClasses({ variant: 'default' })}>
                Open the vendor console
                <ArrowRight className="size-3.5" aria-hidden="true" />
              </Link>
            }
          />
        ) : (
          <ul className="space-y-3">
            {flags.map((flag) => (
              <li key={flag.id}>
                <FlagPanel flag={flag} />
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* ---- The canonical case, labelled for what it is ----------------------- */}
      <section aria-labelledby="worked-heading" className="space-y-3">
        <SectionHead
          id="worked-heading"
          title="The case the check exists for"
          meta={<Chip tone="outline">Worked example — not a chain record</Chip>}
        />

        <Panel>
          <PanelHead title="Same product, same vendor, two irreconcilable declarations" />
          <PanelBody>
            <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
              <Declaration
                heading="Declaration on the earlier tender"
                tender="GEM/2025/B/6712330"
                ministry="DOT"
                bps={8_600}
              />
              <Declaration
                heading="Declaration on the later tender"
                tender="GEM/2025/B/6390218"
                ministry="MOHUA"
                bps={3_000}
              />
            </div>
          </PanelBody>
          <PanelNote>
            Optical Fibre Cable (24F, Armoured), HSN 8544, declared to two different buyers within
            one quarter. The gap is 56 percentage points against a tolerance of{' '}
            {formatBps(TOLERANCE_BPS)}. Under today&apos;s arrangements these two declarations sit
            in two ministries&apos; files and never meet.
          </PanelNote>
        </Panel>
      </section>

      {/* ---- Vendor drill-down -------------------------------------------------- */}
      <section aria-labelledby="history-heading" className="space-y-3">
        <SectionHead
          id="history-heading"
          title="Vendor declaration history"
          meta={
            <div className="w-full max-w-xs">
              <label htmlFor="vendor-filter" className="sr-only">
                Filter by vendor
              </label>
              <Select
                id="vendor-filter"
                value={vendorFilter}
                onChange={(event) => setVendorFilter(event.target.value)}
              >
                <option value="all">All vendors</option>
                {vendorsSeen.map((vendor) => (
                  <option key={vendor.account} value={vendor.account}>
                    {vendor.name}
                  </option>
                ))}
              </Select>
            </div>
          }
        />

        <Panel>
          {history.length === 0 ? (
            <PanelBody>
              <Empty
                title="No declaration history to trace yet"
                source="Once a bid has been classified or evaluated anywhere in the console, its decision appears here with the tender and the finalized block it was recorded in."
              />
            </PanelBody>
          ) : (
            <Table>
              <THead>
                <TR>
                  <TH className="w-40">Recorded</TH>
                  <TH>Vendor</TH>
                  <TH className="w-48">Tender</TH>
                  <TH className="w-32">Ministry</TH>
                  <TH numeric className="w-24">Declared</TH>
                  <TH>Outcome</TH>
                  <TH numeric className="w-28">Block</TH>
                  <TH className="w-40">Transaction</TH>
                </TR>
              </THead>
              <TBody>
                {history.map((entry) => (
                  <TR key={entry.id}>
                    <TD className="text-2xs whitespace-nowrap text-ink-muted">
                      {new Date(entry.at).toLocaleString('en-IN', {
                        dateStyle: 'medium',
                        timeStyle: 'short',
                      })}
                    </TD>
                    <TD>{entry.vendor ? vendorName(entry.vendor) : <Nil />}</TD>
                    <TD mono>{entry.tender ?? <Nil />}</TD>
                    <TD>{entry.ministry ? ministryShort(entry.ministry) : <Nil />}</TD>
                    <TD numeric>{entry.facts?.['Declared local content'] ?? <Nil />}</TD>
                    <TD>
                      <span className="flex flex-wrap items-center gap-1.5">
                        <StatusToken status={entry.result} />
                        <span className="text-ink">{entry.headline}</span>
                      </span>
                    </TD>
                    <TD numeric>
                      {entry.blockNumber === null || entry.blockNumber === undefined ? (
                        <span className="font-sans text-2xs text-ink-muted">Ledger read</span>
                      ) : (
                        `#${entry.blockNumber.toLocaleString('en-IN')}`
                      )}
                    </TD>
                    <TD>
                      {entry.txRef ? <Hash value={entry.txRef} /> : <Nil label="No transaction" />}
                    </TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          )}
          <PanelNote>
            Every declaration this console has seen decided, with the block it was sealed in. Trace
            any row further in the explorer, which reads the chain rather than this console&apos;s
            record of it.
          </PanelNote>
        </Panel>
      </section>
    </div>
  );
}

// -------------------------------------------------------------------------------------

function FlagPanel({ flag }: { flag: InconsistencyEntry }) {
  const gap = flag.priorBps === null ? null : Math.abs(flag.declaredBps - flag.priorBps);
  const tender = getTender(flag.tender);

  return (
    <Panel>
      <PanelHead
        title={`${vendorName(flag.vendor)} — ${flag.product}`}
        meta={<Chip tone="yellow">Inconsistency flagged</Chip>}
      />
      <PanelBody>
        <div className="grid grid-cols-1 gap-2 md:grid-cols-2">
          <Declaration
            heading="Declared earlier"
            tender={flag.priorTender ?? 'Recorded on chain'}
            // The earlier declaration may have been made to an entirely different
            // ministry — which is the whole point of the check — so the buyer is resolved
            // from that tender rather than reusing this one's.
            ministry={
              (flag.priorTender ? getTender(flag.priorTender)?.ministryId : undefined) ??
              flag.ministry
            }
            bps={flag.priorBps}
          />
          <Declaration
            heading="Declared on this bid"
            tender={flag.tender}
            ministry={flag.ministry}
            bps={flag.declaredBps}
          />
        </div>

        <DataList className="mt-3">
          <DataRow
            label="Gap between the two declarations"
            value={
              gap === null
                ? 'Materially different'
                : `${formatBps(gap)} against a tolerance of ${formatBps(TOLERANCE_BPS)}`
            }
          />
          <DataRow
            label="Finalized block"
            value={
              flag.blockNumber === null || flag.blockNumber === undefined ? (
                <Nil />
              ) : (
                `#${flag.blockNumber.toLocaleString('en-IN')}`
              )
            }
            mono
          />
          <DataRow
            label="Transaction"
            value={flag.txRef ? <Hash value={flag.txRef} /> : <Nil />}
          />
        </DataList>
      </PanelBody>
      <PanelNote>
        {tender
          ? `${tender.itemCategory} · HSN ${tender.hsnCode}`
          : 'Declared against a product already in this vendor’s history.'}
      </PanelNote>
    </Panel>
  );
}

function Declaration({
  heading,
  tender,
  ministry,
  bps,
}: {
  heading: string;
  tender: string;
  ministry: string;
  bps: number | null;
}) {
  return (
    <Figure
      label={heading}
      value={bps === null ? 'Not readable' : formatBps(bps)}
      note={
        <>
          <span className="font-mono">{tender}</span> · {ministryShort(ministry)}
        </>
      }
    />
  );
}
