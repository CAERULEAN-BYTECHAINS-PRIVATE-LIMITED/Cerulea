'use client';

import { ArrowRight, FileSearch, Scale, ShieldAlert } from 'lucide-react';
import Link from 'next/link';
import { useEffect, useMemo, useState } from 'react';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  DataRow,
  EmptyState,
  Select,
  Stat,
  Table,
  TBody,
  TCaption,
  TD,
  TH,
  THead,
  TR,
  TxRef,
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
 * a hundred and twenty declarations and four contradictions. Worse, one of the hints
 * claimed national scope ("across every tender and ministry") for a number that only ever
 * counted what that one tab had watched happen. Everything in this block is now a chain
 * read, and the register further down, which is still session-scoped, says so on its face.
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
  const readNote = countersError
    ? ` The chain could not be read: ${countersError}`
    : '';

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
    <div className="space-y-8">
      {/* Reads the chain on every load, as do the four figures directly below it. */}
      <AnomalyPanel />

      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="Inconsistency flags"
          value={chainValue((c) => c.inconsistenciesFlagged)}
          hint={`Raised by the chain's own consistency rule, across every tender and ministry.${readNote}`}
          icon={<ShieldAlert className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Vendor–product histories flagged"
          value={chainValue((c) => c.vendorProductPairsFlagged)}
          hint="Distinct vendor-and-product records on chain holding at least one contradiction."
        />
        <Stat
          label="Declarations on chain"
          value={chainValue((c) => c.totalDeclarations)}
          hint="Every local-content declaration recorded in pramaanConsistency.declarations."
        />
        <Stat
          label="Tolerance"
          value={formatBps(counters?.toleranceBps ?? TOLERANCE_BPS)}
          hint={
            counters
              ? "The runtime's ToleranceBps, read from the chain's own constants. Beyond it, a declaration is flagged."
              : "The runtime's ToleranceBps. Beyond it, a declaration is flagged."
          }
          icon={<Scale className="size-3.5" aria-hidden="true" />}
        />
      </section>

      {/* ---- The register ------------------------------------------------------ */}
      <section aria-labelledby="register-heading" className="space-y-4">
        <div>
          <h2 id="register-heading" className="text-xl font-semibold tracking-tight text-ink">
            Cross-tender inconsistency register — this session
          </h2>
          <p className="mt-1.5 max-w-3xl text-sm text-ink-muted">
            A declaration is checked against the vendor&apos;s own history for the same product
            regardless of which of the twelve pathways it travelled. Each entry below is a flag
            the chain raised <strong className="font-semibold text-ink">while this browser was
            watching</strong>, with both declarations set against each other. The national count
            is the &ldquo;Inconsistency flags&rdquo; figure above, and every contradiction on
            chain — including those raised before this tab was opened — is listed in the anomaly
            panel at the top of the page.
          </p>
        </div>

        {flags.length === 0 ? (
          <EmptyState
            icon={<FileSearch className="size-5" aria-hidden="true" />}
            title="No inconsistency flagged yet in this session"
            description="A flag is raised when a vendor declares a materially different local content figure for the same product on a second tender. Submit a declaration from the vendor console carrying a product identifier, then submit another for the same product with a very different figure."
            action={
              <Link href="/vendor" className={buttonClasses({ variant: 'secondary', size: 'sm' })}>
                Open the vendor console
                <ArrowRight className="size-4" aria-hidden="true" />
              </Link>
            }
          />
        ) : (
          <ul className="space-y-4">
            {flags.map((flag) => (
              <li key={flag.id}>
                <FlagCard flag={flag} />
              </li>
            ))}
          </ul>
        )}
      </section>

      {/* ---- The canonical case, labelled for what it is ----------------------- */}
      <section aria-labelledby="worked-heading" className="space-y-4">
        <div className="flex flex-wrap items-end justify-between gap-3">
          <div>
            <h2 id="worked-heading" className="text-xl font-semibold tracking-tight text-ink">
              The case the check exists for
            </h2>
            <p className="mt-1.5 max-w-3xl text-sm text-ink-muted">
              The worked example from the PoC document, reproduced here so the register&apos;s
              format is readable before the first live flag arrives.
            </p>
          </div>
          <Badge tone="neutral">Worked example — not a chain record</Badge>
        </div>

        <Card>
          <CardHeader
            title="Same product, same vendor, two irreconcilable declarations"
            description="Optical Fibre Cable (24F, Armoured), HSN 8544, declared to two different buyers within one quarter."
          />
          <CardBody>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <DeclarationPanel
                heading="Declaration on the earlier tender"
                tender="GEM/2025/B/6712330"
                ministry="DOT"
                bps={8_600}
                emphasis
              />
              <DeclarationPanel
                heading="Declaration on the later tender"
                tender="GEM/2025/B/6390218"
                ministry="MOHUA"
                bps={3_000}
                emphasis
              />
            </div>
            <p className="mt-4 text-sm leading-relaxed text-ink-muted">
              The gap is 56 percentage points against a tolerance of {formatBps(TOLERANCE_BPS)}.
              Under today&apos;s arrangements these two declarations sit in two ministries&apos; files
              and never meet. Here the second one is contradicted by the first at the moment it is
              made, and both are recorded against the same product in one history.
            </p>
          </CardBody>
        </Card>
      </section>

      {/* ---- Vendor drill-down -------------------------------------------------- */}
      <section aria-labelledby="history-heading" className="space-y-4">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 id="history-heading" className="text-xl font-semibold tracking-tight text-ink">
              Vendor declaration history
            </h2>
            <p className="mt-1.5 max-w-3xl text-sm text-ink-muted">
              Every declaration this console has seen decided, with the block it was sealed in.
            </p>
          </div>
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
        </div>

        <Card>
          {history.length === 0 ? (
            <CardBody>
              <EmptyState
                icon={<FileSearch className="size-5" aria-hidden="true" />}
                title="No declaration history to trace yet"
                description="Once a bid has been classified or evaluated anywhere in the console, its decision appears here with the rule version, the tender and the finalized block it was recorded in."
              />
            </CardBody>
          ) : (
            <Table>
              <THead>
                <TR>
                  <TH>Recorded</TH>
                  <TH>Vendor</TH>
                  <TH>Tender</TH>
                  <TH>Ministry</TH>
                  <TH>Declared</TH>
                  <TH>Outcome</TH>
                  <TH>Block</TH>
                  <TH>Transaction</TH>
                </TR>
              </THead>
              <TBody>
                {history.map((entry) => (
                  <TR key={entry.id}>
                    <TD className="whitespace-nowrap text-xs text-ink-muted">
                      {new Date(entry.at).toLocaleString('en-IN', {
                        dateStyle: 'medium',
                        timeStyle: 'short',
                      })}
                    </TD>
                    <TD>{entry.vendor ? vendorName(entry.vendor) : '—'}</TD>
                    <TD mono>{entry.tender ?? '—'}</TD>
                    <TD>{entry.ministry ? ministryShort(entry.ministry) : '—'}</TD>
                    <TD className="tabular-nums">
                      {entry.facts?.['Declared local content'] ?? '—'}
                    </TD>
                    <TD>
                      <span className="block text-sm text-ink">{entry.headline}</span>
                      <Badge
                        tone={
                          entry.result === 'GREEN'
                            ? 'green'
                            : entry.result === 'YELLOW'
                              ? 'yellow'
                              : 'red'
                        }
                        size="sm"
                        className="mt-1"
                      >
                        {entry.result}
                      </Badge>
                    </TD>
                    <TD mono>
                      {entry.blockNumber === null || entry.blockNumber === undefined
                        ? 'Ledger read'
                        : `#${entry.blockNumber.toLocaleString('en-IN')}`}
                    </TD>
                    <TD>
                      {entry.txRef ? (
                        <TxRef value={entry.txRef} head={8} tail={6} />
                      ) : (
                        <span className="text-xs text-ink-muted">No transaction submitted</span>
                      )}
                    </TD>
                  </TR>
                ))}
              </TBody>
              <TCaption>
                Trace any row further in the explorer, which reads the chain rather than this
                console&apos;s record of it.
              </TCaption>
            </Table>
          )}
        </Card>
      </section>
    </div>
  );
}

// -------------------------------------------------------------------------------------

function FlagCard({ flag }: { flag: InconsistencyEntry }) {
  const gap =
    flag.priorBps === null ? null : Math.abs(flag.declaredBps - flag.priorBps);
  const tender = getTender(flag.tender);

  return (
    <Card className="border-status-yellow/30">
      <CardHeader
        title={`${vendorName(flag.vendor)} — ${flag.product}`}
        description={
          tender
            ? `${tender.itemCategory} · HSN ${tender.hsnCode}`
            : 'Declared against a product already in this vendor’s history'
        }
        actions={<Badge tone="yellow">Inconsistency flagged</Badge>}
      />
      <CardBody>
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <DeclarationPanel
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
          <DeclarationPanel
            heading="Declared on this bid"
            tender={flag.tender}
            ministry={flag.ministry}
            bps={flag.declaredBps}
          />
        </div>

        <dl className="mt-4">
          <DataRow
            label="Gap between the two declarations"
            value={gap === null ? 'Materially different' : `${formatBps(gap)} against a tolerance of ${formatBps(TOLERANCE_BPS)}`}
          />
          <DataRow
            label="Finalized block"
            value={
              flag.blockNumber === null || flag.blockNumber === undefined
                ? 'Not recorded'
                : `#${flag.blockNumber.toLocaleString('en-IN')}`
            }
            mono
          />
          <DataRow
            label="Transaction"
            value={flag.txRef ? <TxRef value={flag.txRef} /> : 'Not recorded'}
          />
        </dl>
      </CardBody>
    </Card>
  );
}

function DeclarationPanel({
  heading,
  tender,
  ministry,
  bps,
  emphasis = false,
}: {
  heading: string;
  tender: string;
  ministry: string;
  bps: number | null;
  emphasis?: boolean;
}) {
  return (
    <div className="rounded-lg border border-border bg-surface-sunken px-4 py-4">
      <p className="text-xs font-semibold tracking-wide text-ink-muted uppercase">{heading}</p>
      <p
        className={`mt-2 font-semibold tabular-nums text-ink ${emphasis ? 'text-3xl' : 'text-2xl'}`}
      >
        {bps === null ? 'Not readable' : formatBps(bps)}
      </p>
      <p className="mt-2 font-mono text-[0.8125rem] text-ink-muted">{tender}</p>
      <p className="mt-0.5 text-xs text-ink-muted">{ministryShort(ministry)}</p>
    </div>
  );
}
