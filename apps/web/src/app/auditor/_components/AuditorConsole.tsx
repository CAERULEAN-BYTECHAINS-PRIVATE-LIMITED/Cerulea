'use client';

import { FileSignature } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  Awaiting,
  Button,
  Check,
  Chip,
  DataList,
  DataRow,
  Dialog,
  Empty,
  Field,
  Figure,
  FigureRow,
  Hash,
  Input,
  Nil,
  Notice,
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  PathwayChip,
  Select,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  Tooltip,
  Verdict,
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

const OBLIGATION_RULE = [
  {
    band: 'Below ₹10 crore',
    obligation: 'Vendor may self-certify',
    effect: 'The declaration is recorded on chain in the vendor’s own name and the contract proceeds.',
  },
  {
    band: 'At or above ₹10 crore',
    obligation: 'Accountant’s certificate mandatory',
    effect: 'The runtime refuses a certification submitted without one.',
  },
  {
    band: 'When it falls due',
    obligation: 'At execution, not at bidding',
    effect: 'An above-threshold contract with no certificate yet is a pending obligation — review required, never a violation.',
  },
];

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
    <div className="space-y-4">
      <FigureRow>
        <Figure
          label="Certification threshold"
          value="₹10 cr"
          note="DPIIT default; each ministry may notify its own."
        />
        <Figure
          label="Awaiting a statutory certificate"
          value={mandatoryCount}
          note="Contracts at or above their ministry's threshold."
        />
        <Figure
          label="Certificates signed here"
          value={certificates.length}
          note="Every one bound to this auditor's account on chain."
        />
        <Figure
          label="Certificates now flagged"
          value={flaggedCount}
          note="A certified vendor later contradicted or debarred."
        />
      </FigureRow>

      <Panel>
        <PanelHead
          title="When an accountant's certificate is mandatory"
          meta={<PathwayChip pathway="P11" />}
        />
        <Table>
          <THead>
            <TR>
              <TH className="w-48">Contract value</TH>
              <TH className="w-64">Obligation</TH>
              <TH>Effect</TH>
            </TR>
          </THead>
          <TBody>
            {OBLIGATION_RULE.map((rule) => (
              <TR key={rule.band}>
                <TD className="font-medium text-ink">{rule.band}</TD>
                <TD>{rule.obligation}</TD>
                <TD className="text-ink-muted">{rule.effect}</TD>
              </TR>
            ))}
          </TBody>
        </Table>
        <PanelNote>
          The 19.07.2024 amendment places the certification obligation at execution rather than
          at bidding.
        </PanelNote>
      </Panel>

      {/* ---- The queue --------------------------------------------------------- */}
      <Panel>
        <PanelHead title="Certifications pending" meta={<Chip>{queue.length} contracts</Chip>} />
        <Table>
          <THead>
            <TR>
              <TH className="w-52">Contract</TH>
              <TH>Supplier</TH>
              <TH className="w-40">Nodal ministry</TH>
              <TH numeric className="w-36">Contract value</TH>
              <TH numeric className="w-32">Threshold</TH>
              <TH className="w-56">Obligation</TH>
              <TH className="w-24">
                <span className="sr-only">Certify</span>
              </TH>
            </TR>
          </THead>
          <TBody>
            {queue.map((row) => (
              <TR key={row.tender.id} className="hover:bg-shell">
                <TD>
                  <span className="font-mono text-2xs text-ink">{row.tender.id}</span>
                  <span className="mt-0.5 block text-2xs text-ink-muted">
                    {row.tender.itemCategory}
                  </span>
                </TD>
                <TD>{vendorName(row.vendor)}</TD>
                <TD>{ministryShort(row.tender.ministryId)}</TD>
                <TD numeric>{formatPaise(row.tender.valuePaise)}</TD>
                <TD numeric>{formatPaise(row.thresholdPaise)}</TD>
                <TD>
                  {row.mandatory ? (
                    <Chip tone="yellow">Auditor certificate mandatory</Chip>
                  ) : (
                    <Chip>Vendor may self-certify</Chip>
                  )}
                </TD>
                <TD className="text-right">
                  <Button
                    size="xs"
                    variant={row.mandatory ? 'primary' : 'default'}
                    onClick={() => setActive(row)}
                    icon={<FileSignature className="size-3.5" aria-hidden="true" />}
                  >
                    Certify
                  </Button>
                </TD>
              </TR>
            ))}
          </TBody>
        </Table>
        <PanelNote>
          The obligation column is read from each ministry&apos;s own certification threshold in the
          rule registry, not from a fixed figure in this page.
        </PanelNote>
      </Panel>

      {/* ---- Accountability ledger --------------------------------------------- */}
      <Panel>
        <PanelHead
          title="Auditor accountability ledger"
          meta={<Chip>{certificates.length} certificates</Chip>}
        />
        {certificates.length === 0 ? (
          <PanelBody>
            <Empty
              title="No certificates signed from this console yet"
              source="Issue one from the queue above. Each is bound to this auditor's account on chain and appears here with its transaction reference."
            />
          </PanelBody>
        ) : (
          <Table>
            <THead>
              <TR>
                <TH className="w-56">Certificate</TH>
                <TH>Supplier</TH>
                <TH className="w-52">Contract</TH>
                <TH className="w-44">Verdict</TH>
                <TH className="w-48">Flag</TH>
                <TH className="w-40">Record</TH>
              </TR>
            </THead>
            <TBody>
              {certificates.map((entry) => {
                const contradicted = Boolean(entry.vendor && flaggedVendors.has(entry.vendor));
                const debarred = Boolean(entry.vendor && debarredVendors.has(entry.vendor));
                return (
                  <TR key={entry.id}>
                    <TD mono>{entry.facts?.['Certificate id'] ?? <Nil />}</TD>
                    <TD>{entry.vendor ? vendorName(entry.vendor) : <Nil />}</TD>
                    <TD>
                      <span className="font-mono text-2xs">{entry.tender ?? '—'}</span>
                      <span className="mt-0.5 block text-2xs text-ink-muted">
                        {entry.facts?.['Certified value'] ?? ''}
                      </span>
                    </TD>
                    <TD>
                      <Tooltip content={entry.reason}>
                        <span tabIndex={0} className="inline-flex rounded-sm">
                          <Chip tone={entry.result === 'GREEN' ? 'green' : 'yellow'}>
                            {entry.result === 'GREEN' ? 'Certified' : 'Awaiting certificate'}
                          </Chip>
                        </span>
                      </Tooltip>
                    </TD>
                    <TD>
                      {debarred ? (
                        <Chip tone="red">Vendor debarred</Chip>
                      ) : contradicted ? (
                        <Chip tone="yellow">Declaration contradicted</Chip>
                      ) : (
                        <Nil label="None recorded" />
                      )}
                    </TD>
                    <TD>
                      {entry.txRef ? (
                        <Hash value={entry.txRef} />
                      ) : (
                        <span className="text-2xs text-ink-muted">Obligation pending</span>
                      )}
                    </TD>
                  </TR>
                );
              })}
            </TBody>
          </Table>
        )}
        <PanelNote>
          Rows are written from trigger-point responses this console received. The chain, read
          through the explorer, is the audit source.
        </PanelNote>
      </Panel>

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
      subtitle={`${row.tender.id} — ${row.tender.itemCategory}`}
      footer={
        <>
          <p className="text-2xs text-ink-muted">
            {aboveThreshold
              ? "At or above this ministry's threshold, an accountant's certificate is mandatory."
              : 'Below the threshold, the vendor may self-certify.'}
          </p>
          <div className="flex gap-2">
            <Button onClick={onClose}>{verdict ? 'Close' : 'Cancel'}</Button>
            <Button
              variant="primary"
              onClick={submit}
              loading={pending}
              loadingLabel="Recording the certificate"
              disabled={valuePaise === null}
              icon={<FileSignature className="size-3.5" aria-hidden="true" />}
            >
              {verdict ? 'Issue another' : 'Issue certificate'}
            </Button>
          </div>
        </>
      }
    >
      <div className="space-y-4">
        {pending && <Awaiting label="Recording the certificate" steps={false} />}

        {failure && (
          <Notice
            kind={failure.kind}
            detail={failure.message}
            technicalDetail={failure.technicalDetail}
            onRetry={submit}
          />
        )}

        {verdict && (
          <Verdict
            status={verdict.result}
            reason={verdict.reason}
            trigger={`Certification · ${certificateId}`}
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

            <Check
              checked={signAsAuditor}
              onChange={setSignAsAuditor}
              label="Sign as the statutory auditor"
              hint="Unticked, this is submitted as the vendor's own self-certification — above the threshold that leaves the obligation pending rather than satisfied."
            />

            <DataList>
              <DataRow label="Certificate id" value={certificateId} mono />
              <DataRow label="Contract" value={row.tender.id} mono />
              <DataRow label="Item category" value={row.tender.itemCategory} />
              <DataRow label="Signing accountant" value="S. Raghavan & Associates, Cost Accountants" />
            </DataList>
          </>
        )}
      </div>
    </Dialog>
  );
}
