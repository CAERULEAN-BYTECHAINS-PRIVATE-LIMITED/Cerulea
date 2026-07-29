'use client';

import { FileCheck2, FileSignature, Scale, TriangleAlert } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ComplianceResult,
  DataRow,
  Dialog,
  EmptyState,
  ErrorState,
  Field,
  FinalityPending,
  Input,
  PathwayBadge,
  Select,
  Stat,
  Table,
  TBody,
  TCaption,
  TD,
  TH,
  THead,
  TR,
  Tooltip,
  TxRef,
} from '@/components';
import {
  MINISTRIES,
  TENDERS,
  certify,
  getMinistry,
  ministryShort,
  readInconsistencies,
  readLedger,
  recordLedgerEntry,
  requiresAuditorCertificate,
  subscribeConsoleRecord,
  vendorName,
  type ApiFailure,
  type CertificationResponse,
  type InconsistencyEntry,
  type LedgerEntry,
  type TenderRef,
} from '@/lib/api-client';
import { formatPaise, paiseToRupees, rupeesToPaise } from '@/lib/units';

/**
 * The cost or chartered accountant's console (PathwayId P11).
 *
 * Two halves, and the second is the point. Anyone can issue a certificate; what the
 * accountability ledger adds is that every certificate stays bound to the account that
 * signed it, so a certificate issued today is still attributable when the vendor's
 * declaration is contradicted a year later. The flag column is not decorative — it lights
 * from records this console has actually seen the chain produce.
 */

const AUDITOR_ACCOUNT = 'auditor';

interface QueueRow {
  tender: TenderRef;
  vendor: string;
  mandatory: boolean;
  thresholdPaise: string;
}

function buildQueue(): QueueRow[] {
  return TENDERS.flatMap((tender) => {
    const threshold = getMinistry(tender.ministryId)?.certificationThresholdPaise ?? '0';
    // The obligation attaches to the supplier that will execute the contract; on an
    // unawarded tender that is every bidder still in the running, so the lowest bid is
    // the one an accountant would be briefed on first.
    const leadBidder = [...tender.bids].sort((a, b) =>
      BigInt(a.pricePaise) < BigInt(b.pricePaise) ? -1 : 1,
    )[0];
    return [
      {
        tender,
        vendor: leadBidder.vendor,
        mandatory: requiresAuditorCertificate(tender),
        thresholdPaise: threshold,
      },
    ];
  }).sort((a, b) => Number(b.mandatory) - Number(a.mandatory));
}

export function AuditorConsole() {
  // Inline arrow, not a bare reference: `react-hooks/use-memo` rejects the latter, and
  // the React Compiler cannot see through a passed-by-name factory either.
  const queue = useMemo(() => buildQueue(), []);
  const [active, setActive] = useState<QueueRow | null>(null);

  const [ledger, setLedger] = useState<LedgerEntry[]>([]);
  const [flags, setFlags] = useState<InconsistencyEntry[]>([]);

  useEffect(() => {
    const sync = () => {
      setLedger(readLedger());
      setFlags(readInconsistencies());
    };
    sync();
    return subscribeConsoleRecord(sync);
  }, []);

  const certificates = ledger.filter((entry) => entry.kind === 'certification');
  const flaggedVendors = new Set(flags.map((flag) => flag.vendor));
  const debarredVendors = new Set(
    ledger.filter((entry) => entry.kind === 'debarment' && entry.result === 'RED').map((e) => e.vendor),
  );
  const flaggedCount = certificates.filter(
    (entry) =>
      (entry.vendor && flaggedVendors.has(entry.vendor)) ||
      (entry.vendor && debarredVendors.has(entry.vendor)),
  ).length;

  const mandatoryCount = queue.filter((row) => row.mandatory).length;

  return (
    <div className="space-y-8">
      {/* ---- The threshold rule, stated plainly ------------------------------- */}
      <section className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
        <Stat
          label="Certification threshold"
          value="₹10 crore"
          hint="DPIIT default; each ministry may notify its own."
          icon={<Scale className="size-3.5" aria-hidden="true" />}
        />
        <Stat
          label="Awaiting a statutory certificate"
          value={String(mandatoryCount)}
          hint="Contracts at or above their ministry's threshold."
        />
        <Stat
          label="Certificates signed here"
          value={String(certificates.length)}
          hint="Every one bound to this auditor's account on chain."
        />
        <Stat
          label="Certificates now flagged"
          value={String(flaggedCount)}
          hint="A certified vendor later contradicted or debarred."
        />
      </section>

      <Card>
        <CardHeader
          title="When an accountant's certificate is mandatory"
          actions={<PathwayBadge pathway="P11" />}
        />
        <CardBody className="grid grid-cols-1 gap-6 md:grid-cols-3">
          <div>
            <p className="text-sm font-semibold text-ink">Below ₹10 crore</p>
            <p className="mt-1 text-sm leading-relaxed text-ink-muted">
              The vendor self-certifies its local content. The declaration is recorded on chain
              in the vendor&apos;s own name and is enough for the contract to proceed.
            </p>
          </div>
          <div>
            <p className="text-sm font-semibold text-ink">At or above ₹10 crore</p>
            <p className="mt-1 text-sm leading-relaxed text-ink-muted">
              A cost or chartered accountant&apos;s certificate is mandatory. The runtime refuses a
              certification submitted without one.
            </p>
          </div>
          <div>
            <p className="text-sm font-semibold text-ink">When it falls due</p>
            <p className="mt-1 text-sm leading-relaxed text-ink-muted">
              The 19.07.2024 amendment places the obligation at execution, not at bidding — so an
              above-threshold contract with no certificate yet is a pending obligation, shown as
              review required, never as a violation.
            </p>
          </div>
        </CardBody>
      </Card>

      {/* ---- The queue --------------------------------------------------------- */}
      <Card>
        <CardHeader
          title="Certifications pending"
          description="Contracts in this accountant's book of work, ranked with the statutory ones first."
        />
        <Table containerClassName="rounded-b-card">
          <THead>
            <TR>
              <TH>Contract</TH>
              <TH>Supplier</TH>
              <TH>Nodal ministry</TH>
              <TH className="text-right">Contract value</TH>
              <TH className="text-right">Threshold</TH>
              <TH>Obligation</TH>
              <TH><span className="sr-only">Certify</span></TH>
            </TR>
          </THead>
          <TBody>
            {queue.map((row) => (
              <TR key={row.tender.id} className="hover:bg-surface-sunken">
                <TD>
                  <span className="font-mono text-[0.8125rem] text-ink">{row.tender.id}</span>
                  <span className="mt-0.5 block text-xs text-ink-muted">
                    {row.tender.itemCategory}
                  </span>
                </TD>
                <TD>{vendorName(row.vendor)}</TD>
                <TD>{ministryShort(row.tender.ministryId)}</TD>
                <TD className="text-right tabular-nums">{formatPaise(row.tender.valuePaise)}</TD>
                <TD className="text-right tabular-nums">{formatPaise(row.thresholdPaise)}</TD>
                <TD>
                  {row.mandatory ? (
                    <Badge tone="yellow">Auditor certificate mandatory</Badge>
                  ) : (
                    <Badge tone="neutral">Vendor may self-certify</Badge>
                  )}
                </TD>
                <TD className="text-right">
                  <Button
                    size="sm"
                    variant={row.mandatory ? 'primary' : 'secondary'}
                    onClick={() => setActive(row)}
                    leadingIcon={<FileSignature className="size-4" aria-hidden="true" />}
                  >
                    Certify
                  </Button>
                </TD>
              </TR>
            ))}
          </TBody>
          <TCaption>
            The obligation column is read from each ministry&apos;s own certification threshold in
            the rule registry, not from a fixed figure in this page.
          </TCaption>
        </Table>
      </Card>

      {/* ---- Accountability ledger --------------------------------------------- */}
      <Card>
        <CardHeader
          title="Auditor Accountability Ledger"
          description="Every certificate this auditor has signed, with the transaction it was recorded in. A flag appears when a certified vendor is later contradicted or debarred."
        />
        {certificates.length === 0 ? (
          <CardBody>
            <EmptyState
              icon={<FileCheck2 className="size-5" aria-hidden="true" />}
              title="No certificates signed from this console yet"
              description="Issue a certificate from the queue above. Each one is bound to this auditor's account on chain and appears here with its transaction reference and finalized block."
            />
          </CardBody>
        ) : (
          <Table>
            <THead>
              <TR>
                <TH>Certificate</TH>
                <TH>Supplier</TH>
                <TH>Contract</TH>
                <TH>Verdict</TH>
                <TH>Flag</TH>
                <TH>Record</TH>
              </TR>
            </THead>
            <TBody>
              {certificates.map((entry) => {
                const contradicted = Boolean(entry.vendor && flaggedVendors.has(entry.vendor));
                const debarred = Boolean(entry.vendor && debarredVendors.has(entry.vendor));
                return (
                  <TR key={entry.id}>
                    <TD mono>{entry.facts?.['Certificate id'] ?? '—'}</TD>
                    <TD>{entry.vendor ? vendorName(entry.vendor) : '—'}</TD>
                    <TD>
                      <span className="font-mono text-[0.8125rem]">{entry.tender ?? '—'}</span>
                      <span className="mt-0.5 block text-xs text-ink-muted">
                        {entry.facts?.['Certified value'] ?? ''}
                      </span>
                    </TD>
                    <TD>
                      <Tooltip content={entry.reason}>
                        <span tabIndex={0} className="inline-flex rounded-full">
                          <Badge tone={entry.result === 'GREEN' ? 'green' : 'yellow'}>
                            {entry.result === 'GREEN' ? 'Certified' : 'Awaiting certificate'}
                          </Badge>
                        </span>
                      </Tooltip>
                    </TD>
                    <TD>
                      {contradicted || debarred ? (
                        <span className="inline-flex items-center gap-1.5 text-sm font-medium text-status-red">
                          <TriangleAlert className="size-4" aria-hidden="true" />
                          {debarred ? 'Vendor debarred' : 'Declaration contradicted'}
                        </span>
                      ) : (
                        <span className="text-sm text-ink-muted">None recorded</span>
                      )}
                    </TD>
                    <TD>
                      {entry.txRef ? (
                        <TxRef value={entry.txRef} head={8} tail={6} />
                      ) : (
                        <span className="text-xs text-ink-muted">
                          No transaction — obligation pending
                        </span>
                      )}
                    </TD>
                  </TR>
                );
              })}
            </TBody>
            <TCaption>
              Rows are written from trigger-point responses this console received. The chain, read
              through the explorer, is the audit source.
            </TCaption>
          </Table>
        )}
      </Card>

      {active && (
        <CertifyDialog
          row={active}
          onClose={() => setActive(null)}
          onCertified={(response, context) => {
            recordLedgerEntry({
              kind: 'certification',
              persona: 'Cost or Chartered Accountant',
              result: response.result,
              headline: `${context.signedByAuditor ? 'Auditor certificate' : 'Self-certification'} — ${vendorName(context.vendor)}`,
              reason: response.reason,
              tender: context.tender,
              ministry: context.ministry,
              vendor: context.vendor,
              txRef: response.txRef,
              blockNumber: response.blockNumber,
              pathway: 'P11',
              facts: {
                'Certificate id': response.certificateId ?? context.certificateId,
                'Certified value': formatPaise(context.valuePaise),
                'Auditor required': response.requiresAuditor ? 'Yes' : 'No',
              },
            });
          }}
        />
      )}
    </div>
  );
}

// -------------------------------------------------------------------------------------

interface CertifyContext {
  vendor: string;
  tender: string;
  ministry: string;
  valuePaise: string;
  certificateId: string;
  signedByAuditor: boolean;
}

function CertifyDialog({
  row,
  onClose,
  onCertified,
}: {
  row: QueueRow;
  onClose: () => void;
  onCertified: (response: CertificationResponse, context: CertifyContext) => void;
}) {
  const [vendor, setVendor] = useState(row.vendor);
  const [ministryId, setMinistryId] = useState(row.tender.ministryId);
  const [valueRupees, setValueRupees] = useState(String(paiseToRupees(row.tender.valuePaise)));
  const [signAsAuditor, setSignAsAuditor] = useState(true);
  const [certificateId] = useState(
    () => `CERT/${new Date().getFullYear()}/${row.tender.ministryId}/${Date.now().toString().slice(-6)}`,
  );

  const [pending, setPending] = useState(false);
  const [verdict, setVerdict] = useState<CertificationResponse | null>(null);
  const [failure, setFailure] = useState<ApiFailure | null>(null);

  const valuePaise = useMemo(() => {
    const value = Number(valueRupees);
    return Number.isFinite(value) && value >= 0 ? rupeesToPaise(value).toString() : null;
  }, [valueRupees]);

  const threshold = getMinistry(ministryId)?.certificationThresholdPaise ?? '0';
  const aboveThreshold = valuePaise !== null && BigInt(valuePaise) >= BigInt(threshold);

  async function submit() {
    if (valuePaise === null) return;
    setPending(true);
    setFailure(null);
    setVerdict(null);

    const outcome = await certify({
      vendor,
      tender: row.tender.id,
      ministry: ministryId,
      valuePaise,
      certificateId,
      ...(signAsAuditor ? { auditor: AUDITOR_ACCOUNT } : {}),
    });
    setPending(false);

    if (!outcome.ok) {
      setFailure(outcome);
      return;
    }
    setVerdict(outcome.data);
    onCertified(outcome.data, {
      vendor,
      tender: row.tender.id,
      ministry: ministryId,
      valuePaise,
      certificateId,
      signedByAuditor: signAsAuditor,
    });
  }

  return (
    <Dialog
      open
      onClose={onClose}
      size="lg"
      title="Issue a local content certificate"
      description={`${row.tender.id} — ${row.tender.itemCategory}`}
      footer={
        <div className="flex w-full flex-wrap items-center justify-between gap-3">
          <p className="text-xs text-ink-muted">
            {aboveThreshold
              ? "At or above this ministry's threshold, an accountant's certificate is mandatory."
              : 'Below the threshold, the vendor may self-certify.'}
          </p>
          <div className="flex gap-2">
            <Button variant="secondary" onClick={onClose}>
              {verdict ? 'Close' : 'Cancel'}
            </Button>
            <Button
              onClick={submit}
              loading={pending}
              loadingLabel="Recording the certificate"
              disabled={valuePaise === null}
              leadingIcon={<FileSignature className="size-4" aria-hidden="true" />}
            >
              {verdict ? 'Issue another' : 'Issue certificate'}
            </Button>
          </div>
        </div>
      }
    >
      <div className="space-y-5">
        {pending && <FinalityPending label="Recording the certificate" showSteps={false} />}

        {failure && (
          <ErrorState
            kind={failure.kind}
            detail={failure.message}
            technicalDetail={failure.technicalDetail}
            onRetry={submit}
          />
        )}

        {verdict && (
          <ComplianceResult
            status={verdict.result}
            reason={verdict.reason}
            trigger={`Certification — ${certificateId}`}
            txRef={verdict.txRef ?? undefined}
            blockNumber={verdict.blockNumber ?? undefined}
            latencyMs={verdict.latencyMs}
            pathway="P11"
            records={[
              { label: 'Certificate id', value: verdict.certificateId ?? certificateId, mono: true },
              { label: 'Supplier', value: vendorName(vendor) },
              { label: 'Certified value', value: valuePaise ? formatPaise(valuePaise) : '—' },
              {
                label: 'Auditor required',
                value: verdict.requiresAuditor ? 'Yes — at or above the threshold' : 'No — below the threshold',
              },
            ]}
          />
        )}

        {!verdict && (
          <>
            <Field label="Supplier" required>
              {(props) => (
                <Select {...props} value={vendor} onChange={(event) => setVendor(event.target.value)}>
                  {row.tender.bids.map((bid) => (
                    <option key={bid.vendor} value={bid.vendor}>
                      {vendorName(bid.vendor)}
                    </option>
                  ))}
                </Select>
              )}
            </Field>

            <Field label="Nodal ministry" required hint="Its rule supplies the certification threshold.">
              {(props) => (
                <Select
                  {...props}
                  value={ministryId}
                  onChange={(event) => setMinistryId(event.target.value)}
                >
                  {MINISTRIES.map((row_) => (
                    <option key={row_.id} value={row_.id}>
                      {row_.name}
                    </option>
                  ))}
                </Select>
              )}
            </Field>

            <Field
              label="Contract value (₹)"
              required
              hint={`Threshold for this ministry: ${formatPaise(threshold)}. This contract is ${aboveThreshold ? 'at or above' : 'below'} it.`}
            >
              {(props) => (
                <Input
                  {...props}
                  type="number"
                  min={0}
                  step={1}
                  value={valueRupees}
                  onChange={(event) => setValueRupees(event.target.value)}
                  className="font-mono"
                />
              )}
            </Field>

            <label className="flex cursor-pointer items-start gap-3 rounded-lg border border-border bg-surface-sunken px-4 py-3">
              <input
                type="checkbox"
                checked={signAsAuditor}
                onChange={(event) => setSignAsAuditor(event.target.checked)}
                className="mt-0.5 size-4 accent-cerulea"
              />
              <span className="text-sm text-ink">
                Sign as the statutory auditor
                <span className="mt-0.5 block text-xs text-ink-muted">
                  Unticked, this is submitted as the vendor&apos;s own self-certification. Above the
                  threshold that leaves the obligation pending rather than satisfied.
                </span>
              </span>
            </label>

            <dl className="rounded-lg border border-border px-4">
              <DataRow label="Certificate id" value={certificateId} mono />
              <DataRow label="Contract" value={row.tender.id} mono />
              <DataRow label="Item category" value={row.tender.itemCategory} />
              <DataRow label="Signing accountant" value="S. Raghavan & Associates, Cost Accountants" />
            </dl>
          </>
        )}
      </div>
    </Dialog>
  );
}
