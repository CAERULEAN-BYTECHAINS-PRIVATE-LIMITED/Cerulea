'use client';

import { ArrowLeft, BadgeCheck, KeyRound, ScrollText, Send } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ComplianceResult,
  DataRow,
  ErrorState,
  Field,
  FinalityPending,
  Input,
  Select,
  type ComplianceBlocker,
} from '@/components';
import {
  MINISTRIES,
  TENDERS,
  classificationPathway,
  getMinistry,
  ministryName,
  recordInconsistency,
  recordLedgerEntry,
  submitBid,
  thresholdsFor,
  vendorName,
  type ApiFailure,
  type BidSubmissionResponse,
  type TenderRef,
} from '@/lib/api-client';
import { formatBps, formatPaise, percentToBps } from '@/lib/units';

/**
 * The vendor's bid screen.
 *
 * The PoC's core claim is zero vendor disruption: this is a GeM bid form, in GeM's own
 * field vocabulary, and the only thing the vendor does differently is read a verdict that
 * arrives before the bid closes. There is no key, no wallet, no gas and no mention of a
 * chain anywhere on the input side — the transaction reference appears only afterwards,
 * folded away behind "See the on-chain record".
 */

type Stage = 'form' | 'pending' | 'result' | 'failed';

const SIGNED_IN_VENDOR = 'vendor';

export function BidSubmissionConsole() {
  const [tenderId, setTenderId] = useState<string>(TENDERS[0].id);
  const [ministryId, setMinistryId] = useState<string>(TENDERS[0].ministryId);
  const [productId, setProductId] = useState<string>(TENDERS[0].productId);
  const [localContent, setLocalContent] = useState<string>('58');
  const [pliClaimed, setPliClaimed] = useState(false);
  const [touched, setTouched] = useState(false);

  const [stage, setStage] = useState<Stage>('form');
  const [verdict, setVerdict] = useState<BidSubmissionResponse | null>(null);
  const [failure, setFailure] = useState<ApiFailure | null>(null);
  const [submittedBps, setSubmittedBps] = useState(0);

  const tender = useMemo(
    () => TENDERS.find((row) => row.id === tenderId) ?? TENDERS[0],
    [tenderId],
  );
  const thresholds = thresholdsFor(ministryId);
  const rule = getMinistry(ministryId);

  const localContentError = validateLocalContent(localContent);
  const canSubmit = localContentError === undefined;

  function selectTender(id: string) {
    const next = TENDERS.find((row) => row.id === id);
    if (!next) return;
    setTenderId(id);
    // The buyer's own ministry is the one whose rule set applies. It stays editable
    // because a tender routed through a different nodal ministry is a real case.
    setMinistryId(next.ministryId);
    setProductId(next.productId);
  }

  async function onSubmit() {
    setTouched(true);
    if (!canSubmit) return;

    const declaredLocalContentBps = percentToBps(Number(localContent));
    setSubmittedBps(declaredLocalContentBps);
    setStage('pending');
    setFailure(null);

    const outcome = await submitBid({
      vendor: SIGNED_IN_VENDOR,
      tender: tender.id,
      ministry: ministryId,
      declaredLocalContentBps,
      product: productId.trim() || undefined,
      isPliManufacturer: pliClaimed,
    });

    if (!outcome.ok) {
      setFailure(outcome);
      setStage('failed');
      return;
    }

    const data = outcome.data;
    setVerdict(data);
    setStage('result');

    recordLedgerEntry({
      kind: 'classification',
      persona: 'Vendor',
      result: data.result,
      headline: `${data.class ?? 'Classification'} — ${tender.itemCategory}`,
      reason: data.reason,
      tender: tender.id,
      ministry: ministryId,
      vendor: SIGNED_IN_VENDOR,
      txRef: data.txRef,
      blockNumber: data.blockNumber,
      pathway: classificationPathway(ministryId, data.class, pliClaimed) ?? null,
      facts: {
        'Declared local content': formatBps(declaredLocalContentBps),
        'Item category': tender.itemCategory,
      },
    });

    if (data.consistencyFlagged) {
      recordInconsistency({
        vendor: SIGNED_IN_VENDOR,
        product: productId.trim(),
        tender: tender.id,
        declaredBps: declaredLocalContentBps,
        priorBps: data.priorDeclaredBps ?? null,
        priorTender: data.priorTender ?? null,
        ministry: ministryId,
        txRef: data.consistencyTxRef ?? data.txRef,
        blockNumber: data.consistencyBlockNumber ?? data.blockNumber,
      });
    }
  }

  function backToForm() {
    setStage('form');
    setVerdict(null);
    setFailure(null);
  }

  // ---- Pending -------------------------------------------------------------------
  if (stage === 'pending') {
    return (
      <div className="mx-auto max-w-2xl py-6">
        <FinalityPending label={`Submitting bid ${tender.id}`} />
        <p className="mt-4 text-center text-sm text-ink-muted">
          The bid is being classified against {ministryName(ministryId)}&apos;s rule set. The
          verdict appears once the block carrying it is finalized.
        </p>
      </div>
    );
  }

  // ---- Operational failure (never a RED) -----------------------------------------
  if (stage === 'failed' && failure) {
    return (
      <div className="mx-auto max-w-3xl py-2">
        <ErrorState
          kind={failure.kind}
          detail={failure.message}
          technicalDetail={failure.technicalDetail}
          onRetry={onSubmit}
          retryLabel="Resubmit the bid"
          action={
            <Button variant="secondary" size="sm" onClick={backToForm}>
              Change the declaration
            </Button>
          }
        />
        <BidSummary tender={tender} ministryId={ministryId} className="mt-6" />
      </div>
    );
  }

  // ---- Verdict --------------------------------------------------------------------
  if (stage === 'result' && verdict) {
    return (
      <div className="space-y-6">
        <ComplianceResult
          status={verdict.result}
          reason={verdict.reason}
          trigger={`Bid submission — ${tender.id}`}
          txRef={verdict.txRef ?? undefined}
          blockNumber={verdict.blockNumber ?? undefined}
          latencyMs={verdict.latencyMs}
          pathway={classificationPathway(ministryId, verdict.class, pliClaimed)}
          blockedBy={blockerFor(verdict, ministryId, tender, submittedBps)}
          records={[
            { label: 'Bid number', value: tender.id, mono: true },
            { label: 'Buyer organisation', value: tender.buyerOrganisation },
            { label: 'Item category', value: `${tender.itemCategory} · HSN ${tender.hsnCode}` },
            { label: 'Declared local content', value: formatBps(submittedBps) },
            {
              label: 'Class-I / Class-II threshold applied',
              value: `${formatBps(thresholds.classOneBps)} / ${formatBps(thresholds.classTwoBps)}`,
            },
            { label: 'Classification recorded', value: verdict.class ?? 'Not recorded' },
            ...(productId.trim()
              ? [{ label: 'Product id', value: productId.trim(), mono: true }]
              : []),
          ]}
        />

        {verdict.consistencyFlagged && (
          <Card className="border-status-yellow/30">
            <CardHeader
              title="Flagged as inconsistent with this vendor's own history"
              description="The cross-tender consistency check runs across all twelve pathways, because a declaration is checked against the vendor's history regardless of which route it takes."
            />
            <CardBody>
              <dl className="grid grid-cols-1 gap-x-8 sm:grid-cols-2">
                <DataRow label="Declared on this bid" value={formatBps(submittedBps)} />
                <DataRow
                  label="Declared earlier for the same product"
                  value={
                    verdict.priorDeclaredBps === null || verdict.priorDeclaredBps === undefined
                      ? 'A materially different figure'
                      : formatBps(verdict.priorDeclaredBps)
                  }
                />
                <DataRow label="Product" value={productId.trim()} mono />
                <DataRow
                  label="Earlier tender"
                  value={verdict.priorTender ?? 'Recorded on chain'}
                  mono
                />
              </dl>
              <p className="mt-4 text-sm text-ink-muted">
                The flag does not change this bid&apos;s classification. It is evidence recorded
                alongside it, and it is what the CVC console reads across tenders.
              </p>
            </CardBody>
          </Card>
        )}

        {verdict.consistencyError && (
          <Card>
            <CardBody>
              <p className="text-sm text-ink">
                The classification reached finality, but the declaration could not be added to
                the consistency history: {verdict.consistencyError}. The verdict above stands.
              </p>
            </CardBody>
          </Card>
        )}

        <BidSummary tender={tender} ministryId={ministryId} />

        <div className="flex flex-wrap gap-3">
          <Button
            variant="secondary"
            onClick={backToForm}
            leadingIcon={<ArrowLeft className="size-4" aria-hidden="true" />}
          >
            Submit another bid
          </Button>
        </div>
      </div>
    );
  }

  // ---- The bid form ----------------------------------------------------------------
  return (
    <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
      <Card className="lg:col-span-2">
        <CardHeader
          title="Bid participation form"
          description="The same fields a seller fills on GeM today. Nothing extra is asked of the vendor."
        />
        <CardBody className="space-y-5">
          <Field
            label="Bid number"
            required
            hint={`${tender.bidType} · ${tender.packetType} · closes ${tender.bidEndDate}`}
          >
            {(props) => (
              <Select
                {...props}
                value={tenderId}
                onChange={(event) => selectTender(event.target.value)}
              >
                {TENDERS.map((row) => (
                  <option key={row.id} value={row.id}>
                    {row.id} — {row.itemCategory}
                  </option>
                ))}
              </Select>
            )}
          </Field>

          <Field
            label="Buyer organisation (nodal ministry rule set applied)"
            required
            hint={
              rule
                ? `${rule.calculationMethod} calculation · preference margin ${formatBps(rule.preferenceMarginBps)} · ${rule.divisibility}`
                : undefined
            }
          >
            {(props) => (
              <Select
                {...props}
                value={ministryId}
                onChange={(event) => setMinistryId(event.target.value)}
              >
                {MINISTRIES.map((row) => (
                  <option key={row.id} value={row.id}>
                    {row.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>

          <Field
            label="Local content percentage"
            required
            error={touched ? localContentError : undefined}
            hint={`Class-I at ${formatBps(thresholds.classOneBps)} and above; Class-II at ${formatBps(thresholds.classTwoBps)} and above, for HSN ${tender.hsnCode} under this ministry's rule.`}
          >
            {(props) => (
              <div className="relative">
                <Input
                  {...props}
                  type="number"
                  inputMode="decimal"
                  min={0}
                  max={100}
                  step={0.01}
                  value={localContent}
                  onBlur={() => setTouched(true)}
                  onChange={(event) => setLocalContent(event.target.value)}
                  className="pr-9"
                />
                <span
                  className="pointer-events-none absolute inset-y-0 right-3 flex items-center text-sm text-ink-muted"
                  aria-hidden="true"
                >
                  %
                </span>
              </div>
            )}
          </Field>

          <Field
            label="Product identifier (optional)"
            hint="Supplying it records this declaration against the vendor's history for the product, so a contradiction across tenders is caught."
          >
            {(props) => (
              <Input
                {...props}
                value={productId}
                onChange={(event) => setProductId(event.target.value)}
                placeholder={tender.productId}
              />
            )}
          </Field>

          <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-border bg-surface-sunken px-4 py-3">
            <input
              type="checkbox"
              checked={pliClaimed}
              onChange={(event) => setPliClaimed(event.target.checked)}
              className="mt-0.5 size-4 accent-cerulea"
            />
            <span className="text-sm text-ink">
              I have received a Production Linked Incentive for this category
              <span className="mt-0.5 block text-xs text-ink-muted">
                Recognised only where the nodal ministry&apos;s rule is PLI-linked. It deems the
                manufacturer Class-II for the notified period.
                {rule && !rule.pliLinked && ' This ministry’s rule is not PLI-linked.'}
              </span>
            </span>
          </label>
        </CardBody>

        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-border px-5 py-4">
          <p className="text-xs text-ink-muted">
            The verdict is returned only after the block carrying it is finalized.
          </p>
          <Button
            onClick={onSubmit}
            disabled={touched && !canSubmit}
            leadingIcon={<Send className="size-4" aria-hidden="true" />}
          >
            Submit bid
          </Button>
        </div>
      </Card>

      <div className="space-y-6">
        <BidSummary tender={tender} ministryId={ministryId} />

        <Card>
          <CardHeader title="What the vendor never sees" />
          <CardBody className="space-y-3">
            {[
              {
                Icon: KeyRound,
                text: 'No key, no wallet, no seed phrase. The bid is signed by the procuring entity that records the classification.',
              },
              {
                Icon: BadgeCheck,
                text: 'No gas, no token, no network fee. The vendor pays nothing to be classified.',
              },
              {
                Icon: ScrollText,
                text: 'No new portal. This is the GeM bid form, with a verdict added before the bid closes instead of a dispute after it.',
              },
            ].map(({ Icon, text }) => (
              <div key={text} className="flex gap-3">
                <Icon className="mt-0.5 size-4 shrink-0 text-cerulea" aria-hidden="true" />
                <p className="text-sm leading-relaxed text-ink-muted">{text}</p>
              </div>
            ))}
          </CardBody>
        </Card>
      </div>
    </div>
  );
}

// -------------------------------------------------------------------------------------

function BidSummary({
  tender,
  ministryId,
  className,
}: {
  tender: TenderRef;
  ministryId: string;
  className?: string;
}) {
  return (
    <Card className={className}>
      <CardHeader title="Bid details" description={tender.buyerOrganisation}>
        <Badge tone="brand">{tender.bidType}</Badge>
      </CardHeader>
      <CardBody>
        <dl>
          <DataRow label="Bid number" value={tender.id} mono />
          <DataRow label="Bid end date" value={tender.bidEndDate} />
          <DataRow label="Item category" value={tender.itemCategory} />
          <DataRow label="HSN code" value={tender.hsnCode} mono />
          <DataRow
            label="Total quantity"
            value={`${tender.totalQuantity.toLocaleString('en-IN')} ${tender.unit}`}
          />
          <DataRow label="Estimated bid value" value={formatPaise(tender.valuePaise)} />
          <DataRow label="EMD" value={formatPaise(tender.emdPaise)} />
          <DataRow label="ePBG" value={`${tender.epbgPercent}% of order value`} />
          <DataRow label="Evaluation method" value={tender.evaluationMethod} />
          <DataRow label="MSE purchase preference" value="Applicable" />
          <DataRow label="Make in India (MII)" value={ministryName(ministryId)} />
          <DataRow label="Bidding as" value={vendorName('vendor')} />
        </dl>
      </CardBody>
    </Card>
  );
}

function validateLocalContent(value: string): string | undefined {
  if (value.trim() === '') return 'Enter the local content percentage declared for this bid.';
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return 'Local content must be a number between 0 and 100.';
  if (parsed < 0 || parsed > 100) return 'Local content must be between 0 and 100 percent.';
  return undefined;
}

/** A RED must name the rule or debarment that caused the block. */
function blockerFor(
  verdict: BidSubmissionResponse,
  ministryId: string,
  tender: TenderRef,
  declaredBps: number,
): ComplianceBlocker | undefined {
  if (verdict.result !== 'RED') return undefined;

  if (verdict.palletError === 'VendorDebarred') {
    return {
      kind: 'debarment',
      label: 'An active debarment on the shared national ledger',
      detail:
        'A debarment recorded by any nodal ministry blocks this vendor under every ministry, not only the one that recorded it.',
      citation: 'GFR Rule 151(iii); PoC document Table 8 — one shared ledger, enforced before bidding.',
    };
  }

  const thresholds = thresholdsFor(ministryId);
  return {
    kind: 'rule',
    label: `${ministryName(ministryId)} local content threshold, HSN ${tender.hsnCode}`,
    detail: `Declared ${formatBps(declaredBps)} against a Class-II threshold of ${formatBps(thresholds.classTwoBps)} and a Class-I threshold of ${formatBps(thresholds.classOneBps)}.`,
    citation:
      'Public Procurement (Preference to Make in India) Order, 2017 — order P-45021/2/2017-PP(BE-II), as amended.',
  };
}
