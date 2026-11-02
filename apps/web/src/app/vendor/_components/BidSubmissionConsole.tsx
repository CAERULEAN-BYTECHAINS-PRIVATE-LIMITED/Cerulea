'use client';

import { ArrowLeft, Send } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Awaiting,
  Button,
  Check,
  Chip,
  DataList,
  DataRow,
  Field,
  Input,
  Notice,
  Panel,
  PanelBody,
  PanelFoot,
  PanelHead,
  PanelNote,
  Select,
  Verdict,
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
 * folded away behind "On-chain record".
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
      <div className="mx-auto max-w-2xl py-4">
        <Awaiting label={`Submitting bid ${tender.id}`} />
      </div>
    );
  }

  // ---- Operational failure (never a RED) -----------------------------------------
  if (stage === 'failed' && failure) {
    return (
      <div className="mx-auto max-w-3xl space-y-4 py-1">
        <Notice
          kind={failure.kind}
          detail={failure.message}
          technicalDetail={failure.technicalDetail}
          onRetry={onSubmit}
          retryLabel="Resubmit the bid"
          action={
            <Button onClick={backToForm}>Change the declaration</Button>
          }
        />
        <BidSummary tender={tender} ministryId={ministryId} />
      </div>
    );
  }

  // ---- Verdict --------------------------------------------------------------------
  if (stage === 'result' && verdict) {
    return (
      <div className="space-y-4">
        <Verdict
          status={verdict.result}
          reason={verdict.reason}
          trigger={`Bid submission · ${tender.id}`}
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
          <Panel>
            <PanelHead
              title="Inconsistent with this vendor's own history"
              meta={<Chip tone="yellow">Flagged</Chip>}
            />
            <PanelBody>
              <DataList columns={2}>
                <DataRow label="Declared on this bid" value={formatBps(submittedBps)} />
                <DataRow
                  label="Declared earlier, same product"
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
              </DataList>
            </PanelBody>
            <PanelNote>
              The flag does not change this classification. It is evidence recorded alongside
              it, and it is what the vigilance console reads across tenders.
            </PanelNote>
          </Panel>
        )}

        {verdict.consistencyError && (
          <Panel>
            <PanelBody>
              <p className="text-sm text-ink">
                The classification reached finality, but the declaration could not be added to
                the consistency history: {verdict.consistencyError}. The verdict above stands.
              </p>
            </PanelBody>
          </Panel>
        )}

        <BidSummary tender={tender} ministryId={ministryId} />

        <Button onClick={backToForm} icon={<ArrowLeft className="size-3.5" aria-hidden="true" />}>
          Submit another bid
        </Button>
      </div>
    );
  }

  // ---- The bid form ----------------------------------------------------------------
  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-3">
      <Panel className="lg:col-span-2">
        <PanelHead title="Bid participation form" />
        <PanelBody className="space-y-4">
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
            hint={`Class-I at ${formatBps(thresholds.classOneBps)} and above; Class-II at ${formatBps(thresholds.classTwoBps)} and above, HSN ${tender.hsnCode}.`}
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
                  className="pr-8"
                />
                <span
                  className="pointer-events-none absolute inset-y-0 right-2.5 flex items-center text-sm text-ink-muted"
                  aria-hidden="true"
                >
                  %
                </span>
              </div>
            )}
          </Field>

          <Field
            label="Product identifier (optional)"
            hint="Records this declaration against the vendor's history for the product, so a contradiction across tenders is caught."
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

          <Check
            checked={pliClaimed}
            onChange={setPliClaimed}
            label="Production Linked Incentive received for this category"
            hint={`Recognised only where the ministry's rule is PLI-linked; it deems the manufacturer Class-II for the notified period.${
              rule && !rule.pliLinked ? ' This ministry’s rule is not PLI-linked.' : ''
            }`}
          />
        </PanelBody>

        <PanelFoot>
          <p className="text-2xs text-ink-muted">
            The verdict is returned only after the block carrying it is finalized.
          </p>
          <Button
            variant="primary"
            onClick={onSubmit}
            disabled={touched && !canSubmit}
            icon={<Send className="size-3.5" aria-hidden="true" />}
          >
            Submit bid
          </Button>
        </PanelFoot>
      </Panel>

      <div className="space-y-4">
        <BidSummary tender={tender} ministryId={ministryId} />

        <Panel>
          <PanelHead title="What the vendor never sees" />
          <PanelBody>
            <DataList>
              <DataRow label="Key, wallet or seed phrase" value="None" />
              <DataRow label="Gas, token or network fee" value="None" />
              <DataRow label="New portal to learn" value="None" />
            </DataList>
          </PanelBody>
          <PanelNote>
            The bid is signed by the procuring entity that records the classification. This is
            the GeM bid form, with a verdict added before the bid closes.
          </PanelNote>
        </Panel>
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
    <Panel className={className}>
      <PanelHead title="Bid details" meta={<Chip tone="accent">{tender.bidType}</Chip>} />
      <PanelBody>
        <DataList>
          <DataRow label="Bid number" value={tender.id} mono />
          <DataRow label="Buyer organisation" value={tender.buyerOrganisation} />
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
        </DataList>
      </PanelBody>
    </Panel>
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
