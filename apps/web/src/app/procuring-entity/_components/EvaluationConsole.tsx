'use client';

import { Calculator, ClipboardCheck, Gavel, Inbox, ListChecks } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ComplianceResult,
  DataRow,
  EmptyState,
  ErrorState,
  FinalityPending,
  PathwayBadge,
  Table,
  TBody,
  TCaption,
  TD,
  TH,
  THead,
  TR,
  Tooltip,
  TxRef,
  type PathwayId,
} from '@/components';
import {
  TENDERS,
  calculatePreference,
  evaluateBid,
  getMinistry,
  getVendor,
  ministryShort,
  recordLedgerEntry,
  thresholdsFor,
  vendorName,
  type ApiFailure,
  type BidClass,
  type BidEvaluationResponse,
  type PreferenceBidInput,
  type PreferenceResponse,
  type TenderBid,
  type TriState,
} from '@/lib/api-client';
import { formatBps, formatPaise } from '@/lib/units';

/**
 * The procuring entity's console: the tender list, then one tender's bids in detail.
 *
 * Two trigger points run from here. Bid evaluation classifies each bid and checks the
 * shared debarment ledger first, which is what makes a cross-ministry debarment visible
 * as a RED on a tender belonging to an entirely different ministry. Preference
 * calculation then ranks what survived, and its per-vendor rows carry the award share and
 * the pathway — the only thing that tells a divisible award (P8) from a non-divisible
 * one (P9), since both offer a match at L1's price.
 */

interface Evaluation {
  response: BidEvaluationResponse;
  at: number;
}

type EvaluationMap = Record<string, Evaluation>;

export function EvaluationConsole() {
  const [tenderId, setTenderId] = useState(TENDERS[0].id);
  const tender = useMemo(() => TENDERS.find((t) => t.id === tenderId) ?? TENDERS[0], [tenderId]);

  const [evaluations, setEvaluations] = useState<Record<string, EvaluationMap>>({});
  const [evaluating, setEvaluating] = useState<string | null>(null);
  const [preference, setPreference] = useState<Record<string, PreferenceResponse>>({});
  const [preferenceRunning, setPreferenceRunning] = useState(false);
  const [failure, setFailure] = useState<ApiFailure | null>(null);

  const tenderEvaluations = evaluations[tender.id] ?? {};
  const tenderPreference = preference[tender.id];

  const rankable = tender.bids.filter((bid) => {
    const outcome = tenderEvaluations[bid.vendor]?.response;
    return outcome ? isBidClass(outcome.class) : false;
  });
  const allEvaluated = tender.bids.every((bid) => tenderEvaluations[bid.vendor]);

  // ---- Actions ---------------------------------------------------------------------

  async function runEvaluation() {
    setFailure(null);
    const collected: EvaluationMap = { ...tenderEvaluations };

    for (const bid of tender.bids) {
      setEvaluating(bid.vendor);
      const outcome = await evaluateBid({
        vendor: bid.vendor,
        tender: tender.id,
        ministry: tender.ministryId,
        declaredLocalContentBps: bid.declaredLocalContentBps,
        isPliManufacturer: bid.isPliManufacturer ?? false,
      });

      if (!outcome.ok) {
        setEvaluating(null);
        setFailure(outcome);
        setEvaluations((current) => ({ ...current, [tender.id]: collected }));
        return;
      }

      collected[bid.vendor] = { response: outcome.data, at: Date.now() };
      setEvaluations((current) => ({ ...current, [tender.id]: { ...collected } }));

      recordLedgerEntry({
        kind: 'evaluation',
        persona: 'Procuring Entity',
        result: outcome.data.result,
        headline: `${vendorName(bid.vendor)} — ${outcome.data.class ?? 'blocked before classification'}`,
        reason: outcome.data.reason,
        tender: tender.id,
        ministry: tender.ministryId,
        vendor: bid.vendor,
        txRef: outcome.data.txRef,
        blockNumber: outcome.data.blockNumber,
        facts: {
          'Declared local content': formatBps(bid.declaredLocalContentBps),
          'Bid price': formatPaise(bid.pricePaise),
        },
      });
    }
    setEvaluating(null);
  }

  async function runPreference() {
    setFailure(null);
    setPreferenceRunning(true);

    const bids: PreferenceBidInput[] = rankable.map((bid) => ({
      vendor: bid.vendor,
      class: tenderEvaluations[bid.vendor].response.class as BidClass,
      pricePaise: bid.pricePaise,
      isMse: getVendor(bid.vendor)?.isMse ?? false,
      isGte: false,
    }));

    const outcome = await calculatePreference({
      tender: tender.id,
      ministry: tender.ministryId,
      tenderValuePaise: tender.valuePaise,
      isTenderGte: tender.isTenderGte,
      bids,
    });
    setPreferenceRunning(false);

    if (!outcome.ok) {
      setFailure(outcome);
      return;
    }

    setPreference((current) => ({ ...current, [tender.id]: outcome.data }));
    recordLedgerEntry({
      kind: 'preference',
      persona: 'Procuring Entity',
      result: outcome.data.result,
      headline: `Purchase preference — ${tender.itemCategory}`,
      reason: outcome.data.reason,
      tender: tender.id,
      ministry: tender.ministryId,
      txRef: outcome.data.txRef,
      blockNumber: outcome.data.blockNumber,
      pathway: outcome.data.outcomes?.find((row) => row.qualifies)?.decisionPath ?? null,
      facts: {
        'Bids ranked': String(bids.length),
        'Matched price': outcome.data.matchedPricePaise
          ? formatPaise(outcome.data.matchedPricePaise)
          : 'No price match required',
      },
    });
  }

  // ---- Derived figures --------------------------------------------------------------

  const l1Paise = useMemo(() => lowestPrice(rankable), [rankable]);
  const thresholds = thresholdsFor(tender.ministryId);
  const rule = getMinistry(tender.ministryId);

  return (
    <div className="space-y-8">
      {/* ---- Tender list ------------------------------------------------------- */}
      <Card>
        <CardHeader
          title="Tenders open for evaluation"
          description="Sample tenders and sample bids, bundled with this console as the inputs an evaluation is run against — they are not read from the chain and no bid below has been received from anyone. What IS real is the verdict: evaluating one submits a signed extrinsic, and a status appears against a tender only once its bids have been decided on chain."
        />
        <Table containerClassName="rounded-b-card">
          <THead>
            <TR>
              <TH>Bid number</TH>
              <TH>Item category</TH>
              <TH>Nodal ministry</TH>
              <TH className="text-right">Estimated value</TH>
              <TH className="text-right">Bids</TH>
              <TH>Status</TH>
              <TH><span className="sr-only">Open</span></TH>
            </TR>
          </THead>
          <TBody>
            {TENDERS.map((row) => {
              const selected = row.id === tender.id;
              return (
                <TR
                  key={row.id}
                  className={selected ? 'bg-cerulea-light/50' : 'hover:bg-surface-sunken'}
                >
                  <TD mono>{row.id}</TD>
                  <TD>
                    <span className="font-medium text-ink">{row.itemCategory}</span>
                    <span className="mt-0.5 block text-xs text-ink-muted">
                      HSN {row.hsnCode} · {row.bidType}
                    </span>
                  </TD>
                  <TD>{ministryShort(row.ministryId)}</TD>
                  <TD className="text-right tabular-nums">{formatPaise(row.valuePaise)}</TD>
                  <TD className="text-right tabular-nums">{row.bids.length}</TD>
                  <TD>
                    <StatusCell
                      bids={row.bids}
                      evaluations={evaluations[row.id] ?? {}}
                      awarded={Boolean(preference[row.id])}
                    />
                  </TD>
                  <TD className="text-right">
                    <Button
                      size="sm"
                      variant={selected ? 'subtle' : 'secondary'}
                      onClick={() => setTenderId(row.id)}
                    >
                      {selected ? 'Open' : 'Evaluate'}
                    </Button>
                  </TD>
                </TR>
              );
            })}
          </TBody>
          <TCaption>
            Six live tenders across six nodal ministries. The status column carries a compliance
            verdict and nothing else.
          </TCaption>
        </Table>
      </Card>

      {/* ---- Tender detail ----------------------------------------------------- */}
      <section aria-labelledby="detail-heading" className="space-y-6">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2 id="detail-heading" className="text-xl font-semibold tracking-tight text-ink">
              {tender.itemCategory}
            </h2>
            <p className="mt-1 text-sm text-ink-muted">
              {tender.id} · {tender.buyerOrganisation}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            <Button
              variant="secondary"
              onClick={runEvaluation}
              loading={evaluating !== null}
              loadingLabel="Evaluating bids"
              leadingIcon={<ListChecks className="size-4" aria-hidden="true" />}
            >
              {allEvaluated ? 'Re-run bid evaluation' : 'Evaluate all bids'}
            </Button>
            <Button
              onClick={runPreference}
              disabled={rankable.length === 0 || evaluating !== null}
              loading={preferenceRunning}
              loadingLabel="Calculating preference"
              leadingIcon={<Calculator className="size-4" aria-hidden="true" />}
            >
              Calculate purchase preference
            </Button>
          </div>
        </div>

        {failure && (
          <ErrorState
            kind={failure.kind}
            detail={failure.message}
            technicalDetail={failure.technicalDetail}
            onRetry={() => setFailure(null)}
            retryLabel="Dismiss and retry"
          />
        )}

        {evaluating && (
          <FinalityPending
            label={`Evaluating ${vendorName(evaluating)}`}
            showSteps={false}
            className="mx-auto max-w-xl"
          />
        )}

        <div className="grid grid-cols-1 gap-6 xl:grid-cols-3">
          <Card className="xl:col-span-2">
            <CardHeader
              title="Sample bids on this tender"
              description="Bundled sample inputs, not bids received from suppliers. Each one is nonetheless evaluated for real: every bid is checked against the shared national debarment ledger on chain before it is classified."
            />
            <Table containerClassName="rounded-b-card">
              <THead>
                <TR>
                  <TH>Bidder</TH>
                  <TH className="text-right">Declared LC</TH>
                  <TH className="text-right">Bid price</TH>
                  <TH>Classification</TH>
                  <TH>Verdict</TH>
                  <TH>Record</TH>
                </TR>
              </THead>
              <TBody>
                {tender.bids.map((bid) => {
                  const evaluation = tenderEvaluations[bid.vendor];
                  const vendor = getVendor(bid.vendor);
                  const isL1 = l1Paise !== null && BigInt(bid.pricePaise) === l1Paise;
                  return (
                    <TR key={bid.vendor}>
                      <TD>
                        <span className="font-medium text-ink">{vendorName(bid.vendor)}</span>
                        <span className="mt-0.5 flex flex-wrap items-center gap-1.5 text-xs text-ink-muted">
                          {vendor && (
                            <>
                              <span>
                                {vendor.city}, {vendor.state}
                              </span>
                              {vendor.isMse && (
                                <Badge tone="neutral" size="sm">
                                  MSE · {vendor.msme}
                                </Badge>
                              )}
                            </>
                          )}
                        </span>
                      </TD>
                      <TD className="text-right tabular-nums">
                        {formatBps(bid.declaredLocalContentBps)}
                      </TD>
                      <TD className="text-right tabular-nums">
                        {formatPaise(bid.pricePaise)}
                        {isL1 && (
                          <span className="mt-0.5 block text-xs font-medium text-cerulea">
                            Lowest bid
                          </span>
                        )}
                      </TD>
                      <TD>
                        {evaluation ? (
                          <span className="text-sm text-ink">
                            {evaluation.response.class ?? 'Not classified'}
                          </span>
                        ) : (
                          <span className="text-sm text-ink-subtle">Not evaluated</span>
                        )}
                      </TD>
                      <TD>
                        {evaluation ? (
                          <Tooltip content={evaluation.response.reason}>
                            <span tabIndex={0} className="inline-flex rounded-full">
                              <VerdictBadge status={evaluation.response.result} />
                            </span>
                          </Tooltip>
                        ) : (
                          <Badge tone="neutral">Awaiting evaluation</Badge>
                        )}
                      </TD>
                      <TD>
                        {evaluation?.response.txRef ? (
                          <TxRef value={evaluation.response.txRef} head={8} tail={6} />
                        ) : evaluation ? (
                          <span className="text-xs text-ink-muted">
                            Ledger read — no transaction
                          </span>
                        ) : (
                          <span className="text-xs text-ink-subtle">—</span>
                        )}
                      </TD>
                    </TR>
                  );
                })}
              </TBody>
              <TCaption>
                Class-I at {formatBps(thresholds.classOneBps)} and above, Class-II at{' '}
                {formatBps(thresholds.classTwoBps)} and above, under{' '}
                {ministryShort(tender.ministryId)}&apos;s rule for HSN {tender.hsnCode}.
              </TCaption>
            </Table>
          </Card>

          <Card>
            <CardHeader title="Tender file" />
            <CardBody>
              <dl>
                <DataRow label="Bid number" value={tender.id} mono />
                <DataRow label="Bid end date" value={tender.bidEndDate} />
                <DataRow label="Bid type" value={tender.bidType} />
                <DataRow label="Evaluation method" value={tender.evaluationMethod} />
                <DataRow
                  label="Total quantity"
                  value={`${tender.totalQuantity.toLocaleString('en-IN')} ${tender.unit}`}
                />
                <DataRow label="Estimated value" value={formatPaise(tender.valuePaise)} />
                <DataRow
                  label="Preference margin"
                  value={rule ? formatBps(rule.preferenceMarginBps) : '—'}
                />
                <DataRow label="Divisibility" value={rule?.divisibility ?? '—'} />
                <DataRow
                  label="Para 3A applicable"
                  value={rule?.para3aApplicable ? 'Yes' : 'No'}
                />
                <DataRow
                  label="Global tender enquiry"
                  value={tender.isTenderGte ? 'Approved under GFR Rule 161(iv)' : 'Not approved'}
                />
                <DataRow
                  label="Lowest classified bid"
                  value={l1Paise === null ? 'Pending evaluation' : formatPaise(l1Paise)}
                />
              </dl>
            </CardBody>
          </Card>
        </div>

        {/* ---- Preference outcome --------------------------------------------- */}
        {tenderPreference ? (
          <div className="space-y-4">
            <ComplianceResult
              status={tenderPreference.result}
              reason={tenderPreference.reason}
              trigger={`Purchase preference — ${tender.id}`}
              txRef={tenderPreference.txRef ?? undefined}
              blockNumber={tenderPreference.blockNumber ?? undefined}
              latencyMs={tenderPreference.latencyMs}
              blockedBy={
                tenderPreference.result === 'RED'
                  ? {
                      kind: 'eligibility',
                      label: `Eligibility gate — ${tenderPreference.palletError ?? 'rejected by the preference pallet'}`,
                      detail:
                        'The ranking was refused before any award could be made. No bid on this tender qualifies under the gate named above.',
                      citation:
                        'Public Procurement (Preference to Make in India) Order, 2017 — Para 3A and GFR Rule 161(iv).',
                    }
                  : undefined
              }
              records={[
                { label: 'Bids ranked', value: String(rankable.length), mono: true },
                {
                  label: 'Lowest classified bid',
                  value: l1Paise === null ? '—' : formatPaise(l1Paise),
                },
                {
                  label: 'Matched price offered',
                  value: tenderPreference.matchedPricePaise
                    ? formatPaise(tenderPreference.matchedPricePaise)
                    : 'None — the lowest eligible bid stands',
                },
                {
                  label: 'Tender divisibility',
                  value: rule?.divisibility ?? '—',
                },
              ]}
            />

            <Card>
              <CardHeader
                title="Award split, per vendor"
                description="Read from pramaanPreference.preferenceResults at the finalized block. The pathway is what distinguishes a divisible award from a non-divisible one."
              />
              <Table>
                <THead>
                  <TR>
                    <TH>Vendor</TH>
                    <TH>Class</TH>
                    <TH className="text-right">Bid price</TH>
                    <TH className="text-right">Matched price</TH>
                    <TH className="text-right">Award share</TH>
                    <TH>Qualifies</TH>
                    <TH>Pathway</TH>
                  </TR>
                </THead>
                <TBody>
                  {(tenderPreference.outcomes ?? []).map((outcome, index) => {
                    const bid = rankable[index];
                    const pathway = asPathwayId(outcome.decisionPath);
                    return (
                      <TR key={`${outcome.vendor}-${index}`}>
                        <TD>
                          <span className="font-medium text-ink">
                            {vendorName(bid?.vendor ?? outcome.vendor)}
                          </span>
                          <span className="mt-0.5 block font-mono text-[0.6875rem] text-ink-subtle">
                            {outcome.vendor.slice(0, 10)}…{outcome.vendor.slice(-6)}
                          </span>
                        </TD>
                        <TD>
                          {bid
                            ? (tenderEvaluations[bid.vendor]?.response.class ?? '—')
                            : '—'}
                        </TD>
                        <TD className="text-right tabular-nums">
                          {bid ? formatPaise(bid.pricePaise) : '—'}
                        </TD>
                        <TD className="text-right tabular-nums">
                          {outcome.matchedPricePaise
                            ? formatPaise(outcome.matchedPricePaise)
                            : '—'}
                        </TD>
                        <TD className="text-right tabular-nums">
                          {formatBps(outcome.awardedPercentBps)}
                        </TD>
                        <TD>
                          <Badge tone={outcome.qualifies ? 'green' : 'neutral'}>
                            {outcome.qualifies ? 'Qualifies' : 'Does not qualify'}
                          </Badge>
                        </TD>
                        <TD>
                          {pathway ? (
                            <PathwayBadge pathway={pathway} />
                          ) : (
                            <span className="text-xs text-ink-subtle">Not recorded</span>
                          )}
                        </TD>
                      </TR>
                    );
                  })}
                </TBody>
                <TCaption>
                  A {formatBps(rule?.preferenceMarginBps ?? 2_000)} margin over L1 defines the
                  band within which a Class-I supplier may be offered the chance to match L1&apos;s
                  price. On a {rule?.divisibility === 'Divisible' ? 'divisible' : 'non-divisible'}{' '}
                  tender the award{' '}
                  {rule?.divisibility === 'Divisible'
                    ? 'is split between the matching supplier and L1'
                    : 'goes to the matching supplier in full'}
                  .
                </TCaption>
              </Table>
            </Card>
          </div>
        ) : (
          <EmptyState
            icon={<Gavel className="size-5" aria-hidden="true" />}
            title="No preference calculation on this tender yet"
            description={
              rankable.length === 0
                ? 'Evaluate the bids first. Preference can only rank bids that carry a classification recorded on chain.'
                : `${rankable.length} bid${rankable.length === 1 ? '' : 's'} carry a classification and can be ranked. Run the calculation to see the award split and the pathway it took.`
            }
            action={
              rankable.length === 0 ? (
                <Button
                  variant="secondary"
                  onClick={runEvaluation}
                  leadingIcon={<ClipboardCheck className="size-4" aria-hidden="true" />}
                >
                  Evaluate all bids
                </Button>
              ) : (
                <Button
                  onClick={runPreference}
                  loading={preferenceRunning}
                  leadingIcon={<Calculator className="size-4" aria-hidden="true" />}
                >
                  Calculate purchase preference
                </Button>
              )
            }
          />
        )}
      </section>
    </div>
  );
}

// -------------------------------------------------------------------------------------

function VerdictBadge({ status }: { status: TriState }) {
  if (status === 'GREEN') return <Badge tone="green">Compliant</Badge>;
  if (status === 'YELLOW') return <Badge tone="yellow">Review required</Badge>;
  return <Badge tone="red">Blocked</Badge>;
}

function StatusCell({
  bids,
  evaluations,
  awarded,
}: {
  bids: readonly TenderBid[];
  evaluations: EvaluationMap;
  awarded: boolean;
}) {
  const results = bids
    .map((bid) => evaluations[bid.vendor]?.response.result)
    .filter((value): value is TriState => value !== undefined);

  if (results.length === 0) {
    return (
      <span className="inline-flex items-center gap-1.5 text-sm text-ink-subtle">
        <Inbox className="size-3.5" aria-hidden="true" />
        Not evaluated
      </span>
    );
  }

  const counts = {
    GREEN: results.filter((r) => r === 'GREEN').length,
    YELLOW: results.filter((r) => r === 'YELLOW').length,
    RED: results.filter((r) => r === 'RED').length,
  };

  return (
    <span className="flex flex-wrap items-center gap-1">
      {counts.GREEN > 0 && <Badge tone="green" size="sm">{counts.GREEN} compliant</Badge>}
      {counts.YELLOW > 0 && <Badge tone="yellow" size="sm">{counts.YELLOW} review</Badge>}
      {counts.RED > 0 && <Badge tone="red" size="sm">{counts.RED} blocked</Badge>}
      {awarded && <Badge tone="brand" size="sm">Preference calculated</Badge>}
    </span>
  );
}

function isBidClass(value: string | null | undefined): value is BidClass {
  return value === 'ClassOne' || value === 'ClassTwo' || value === 'NonLocal';
}

function lowestPrice(bids: TenderBid[]): bigint | null {
  if (bids.length === 0) return null;
  return bids.reduce<bigint>((lowest, bid) => {
    const price = BigInt(bid.pricePaise);
    return price < lowest ? price : lowest;
  }, BigInt(bids[0].pricePaise));
}

const PATHWAY_IDS = new Set([
  'P1', 'P2', 'P3', 'P4', 'P5', 'P6', 'P7', 'P8', 'P9', 'P10', 'P11', 'P12',
]);

/** Only render a pathway the chain actually recorded. */
function asPathwayId(value: string | null | undefined): PathwayId | undefined {
  if (!value) return undefined;
  const upper = value.toUpperCase();
  return PATHWAY_IDS.has(upper) ? (upper as PathwayId) : undefined;
}
