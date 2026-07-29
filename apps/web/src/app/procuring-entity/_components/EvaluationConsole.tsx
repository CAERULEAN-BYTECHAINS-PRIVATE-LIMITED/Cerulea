'use client';

import { Calculator, ClipboardCheck, ListChecks } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Awaiting,
  Button,
  Chip,
  DataList,
  DataRow,
  Empty,
  Hash,
  Nil,
  Notice,
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  PathwayChip,
  SectionHead,
  StatusToken,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  Tooltip,
  Verdict,
  asPathwayId,
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
    <div className="space-y-5">
      {/* ---- Tender list ------------------------------------------------------- */}
      <Panel>
        <PanelHead
          title="Tenders open for evaluation"
          meta={<Chip tone="neutral">Sample tenders — not chain records</Chip>}
        />
        <Table>
          <THead>
            <TR>
              <TH className="w-48">Bid number</TH>
              <TH>Item category</TH>
              <TH className="w-40">Nodal ministry</TH>
              <TH numeric className="w-36">Estimated value</TH>
              <TH numeric className="w-16">Bids</TH>
              <TH className="w-56">Status</TH>
              <TH className="w-24">
                <span className="sr-only">Open</span>
              </TH>
            </TR>
          </THead>
          <TBody>
            {TENDERS.map((row) => {
              const selected = row.id === tender.id;
              return (
                <TR key={row.id} className={selected ? 'bg-accent-tint' : 'hover:bg-shell'}>
                  <TD mono>{row.id}</TD>
                  <TD>
                    <span className="font-medium text-ink">{row.itemCategory}</span>
                    <span className="mt-0.5 block text-2xs text-ink-muted">
                      HSN {row.hsnCode} · {row.bidType}
                    </span>
                  </TD>
                  <TD>{ministryShort(row.ministryId)}</TD>
                  <TD numeric>{formatPaise(row.valuePaise)}</TD>
                  <TD numeric>{row.bids.length}</TD>
                  <TD>
                    <StatusCell
                      bids={row.bids}
                      evaluations={evaluations[row.id] ?? {}}
                      awarded={Boolean(preference[row.id])}
                    />
                  </TD>
                  <TD className="text-right">
                    <Button
                      size="xs"
                      variant={selected ? 'quiet' : 'default'}
                      onClick={() => setTenderId(row.id)}
                    >
                      {selected ? 'Open' : 'Evaluate'}
                    </Button>
                  </TD>
                </TR>
              );
            })}
          </TBody>
        </Table>
        <PanelNote>
          Sample tenders and sample bids bundled with this console as the inputs an evaluation
          runs against. The verdicts are real: evaluating submits a signed extrinsic.
        </PanelNote>
      </Panel>

      {/* ---- Tender detail ----------------------------------------------------- */}
      <section aria-labelledby="detail-heading" className="space-y-4">
        <SectionHead
          id="detail-heading"
          title={tender.itemCategory}
          meta={
            <>
              <span className="font-mono text-2xs text-ink-muted">{tender.id}</span>
              <Button
                onClick={runEvaluation}
                loading={evaluating !== null}
                loadingLabel="Evaluating bids"
                icon={<ListChecks className="size-3.5" aria-hidden="true" />}
              >
                {allEvaluated ? 'Re-run bid evaluation' : 'Evaluate all bids'}
              </Button>
              <Button
                variant="primary"
                onClick={runPreference}
                disabled={rankable.length === 0 || evaluating !== null}
                loading={preferenceRunning}
                loadingLabel="Calculating preference"
                icon={<Calculator className="size-3.5" aria-hidden="true" />}
              >
                Calculate purchase preference
              </Button>
            </>
          }
        />

        {failure && (
          <Notice
            kind={failure.kind}
            detail={failure.message}
            technicalDetail={failure.technicalDetail}
            onRetry={() => setFailure(null)}
            retryLabel="Dismiss and retry"
          />
        )}

        {evaluating && (
          <Awaiting
            label={`Evaluating ${vendorName(evaluating)}`}
            steps={false}
            className="mx-auto max-w-xl"
          />
        )}

        <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
          <Panel className="xl:col-span-2">
            <PanelHead
              title="Bids on this tender"
              meta={<Chip tone="neutral">Sample bids</Chip>}
            />
            <Table>
              <THead>
                <TR>
                  <TH>Bidder</TH>
                  <TH numeric className="w-28">Declared LC</TH>
                  <TH numeric className="w-36">Bid price</TH>
                  <TH className="w-28">Class</TH>
                  <TH className="w-24">Verdict</TH>
                  <TH className="w-40">Record</TH>
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
                        {vendor && (
                          <span className="mt-0.5 flex flex-wrap items-center gap-1.5 text-2xs text-ink-muted">
                            <span>
                              {vendor.city}, {vendor.state}
                            </span>
                            {vendor.isMse && <Chip>MSE · {vendor.msme}</Chip>}
                          </span>
                        )}
                      </TD>
                      <TD numeric>{formatBps(bid.declaredLocalContentBps)}</TD>
                      <TD numeric>
                        {formatPaise(bid.pricePaise)}
                        {isL1 && (
                          <span className="mt-0.5 block text-2xs font-medium text-accent">L1</span>
                        )}
                      </TD>
                      <TD>
                        {evaluation ? (
                          (evaluation.response.class ?? <Nil label="Not classified" />)
                        ) : (
                          <Nil label="Not evaluated" />
                        )}
                      </TD>
                      <TD>
                        {evaluation ? (
                          <Tooltip content={evaluation.response.reason}>
                            <span tabIndex={0} className="inline-flex rounded-sm">
                              <StatusToken status={evaluation.response.result} />
                            </span>
                          </Tooltip>
                        ) : (
                          <Nil label="Awaiting evaluation" />
                        )}
                      </TD>
                      <TD>
                        {evaluation?.response.txRef ? (
                          <Hash value={evaluation.response.txRef} />
                        ) : evaluation ? (
                          <span className="text-2xs text-ink-muted">Ledger read</span>
                        ) : (
                          <Nil />
                        )}
                      </TD>
                    </TR>
                  );
                })}
              </TBody>
            </Table>
            <PanelNote>
              Class-I at {formatBps(thresholds.classOneBps)} and above, Class-II at{' '}
              {formatBps(thresholds.classTwoBps)} and above, under{' '}
              {ministryShort(tender.ministryId)}&apos;s rule for HSN {tender.hsnCode}. Every bid is
              checked against the shared national debarment ledger before it is classified.
            </PanelNote>
          </Panel>

          <Panel>
            <PanelHead title="Tender file" />
            <PanelBody>
              <DataList>
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
                  value={rule ? formatBps(rule.preferenceMarginBps) : <Nil />}
                />
                <DataRow label="Divisibility" value={rule?.divisibility ?? <Nil />} />
                <DataRow label="Para 3A applicable" value={rule?.para3aApplicable ? 'Yes' : 'No'} />
                <DataRow
                  label="Global tender enquiry"
                  value={tender.isTenderGte ? 'Approved under GFR Rule 161(iv)' : 'Not approved'}
                />
                <DataRow
                  label="Lowest classified bid"
                  value={l1Paise === null ? 'Pending evaluation' : formatPaise(l1Paise)}
                />
              </DataList>
            </PanelBody>
          </Panel>
        </div>

        {/* ---- Preference outcome --------------------------------------------- */}
        {tenderPreference ? (
          <div className="space-y-4">
            <Verdict
              status={tenderPreference.result}
              reason={tenderPreference.reason}
              trigger={`Purchase preference · ${tender.id}`}
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
                { label: 'Tender divisibility', value: rule?.divisibility ?? '—' },
              ]}
            />

            <Panel>
              <PanelHead title="Award split, per vendor" />
              <Table>
                <THead>
                  <TR>
                    <TH>Vendor</TH>
                    <TH className="w-24">Class</TH>
                    <TH numeric className="w-36">Bid price</TH>
                    <TH numeric className="w-36">Matched price</TH>
                    <TH numeric className="w-28">Award share</TH>
                    <TH className="w-32">Qualifies</TH>
                    <TH className="w-56">Pathway</TH>
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
                          <span className="mt-0.5 block">
                            <Hash value={outcome.vendor} label="Account" copyable={false} />
                          </span>
                        </TD>
                        <TD>
                          {bid ? (tenderEvaluations[bid.vendor]?.response.class ?? <Nil />) : <Nil />}
                        </TD>
                        <TD numeric>{bid ? formatPaise(bid.pricePaise) : <Nil />}</TD>
                        <TD numeric>
                          {outcome.matchedPricePaise ? formatPaise(outcome.matchedPricePaise) : <Nil />}
                        </TD>
                        <TD numeric>{formatBps(outcome.awardedPercentBps)}</TD>
                        <TD>
                          <Chip tone={outcome.qualifies ? 'green' : 'neutral'}>
                            {outcome.qualifies ? 'Qualifies' : 'Does not qualify'}
                          </Chip>
                        </TD>
                        <TD>
                          {pathway ? <PathwayChip pathway={pathway} /> : <Nil />}
                        </TD>
                      </TR>
                    );
                  })}
                </TBody>
              </Table>
              <PanelNote>
                Read from <span className="font-mono">pramaanPreference.preferenceResults</span> at
                the finalized block. A {formatBps(rule?.preferenceMarginBps ?? 2_000)} margin over
                L1 defines the band within which a Class-I supplier may match L1&apos;s price; on a{' '}
                {rule?.divisibility === 'Divisible' ? 'divisible' : 'non-divisible'} tender the
                award{' '}
                {rule?.divisibility === 'Divisible'
                  ? 'is split between the matching supplier and L1'
                  : 'goes to the matching supplier in full'}
                .
              </PanelNote>
            </Panel>
          </div>
        ) : (
          <Empty
            title="No preference calculation on this tender yet"
            source={
              rankable.length === 0
                ? 'Preference can only rank bids that carry a classification recorded on chain.'
                : `${rankable.length} bid${rankable.length === 1 ? '' : 's'} carry a classification and can be ranked.`
            }
            action={
              rankable.length === 0 ? (
                <Button
                  onClick={runEvaluation}
                  icon={<ClipboardCheck className="size-3.5" aria-hidden="true" />}
                >
                  Evaluate all bids
                </Button>
              ) : (
                <Button
                  variant="primary"
                  onClick={runPreference}
                  loading={preferenceRunning}
                  icon={<Calculator className="size-3.5" aria-hidden="true" />}
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

  if (results.length === 0) return <Nil label="Not evaluated" />;

  const counts = {
    GREEN: results.filter((r) => r === 'GREEN').length,
    YELLOW: results.filter((r) => r === 'YELLOW').length,
    RED: results.filter((r) => r === 'RED').length,
  };

  return (
    <span className="flex flex-wrap items-center gap-1">
      {counts.GREEN > 0 && <Chip tone="green">{counts.GREEN} compliant</Chip>}
      {counts.YELLOW > 0 && <Chip tone="yellow">{counts.YELLOW} review</Chip>}
      {counts.RED > 0 && <Chip tone="red">{counts.RED} blocked</Chip>}
      {awarded && <Chip tone="accent">Preference calculated</Chip>}
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
