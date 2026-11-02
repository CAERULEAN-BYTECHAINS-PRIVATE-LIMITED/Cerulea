'use client';

import { Ban, RotateCcw, Search } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import {
  Awaiting,
  Button,
  Chip,
  DataList,
  DataRow,
  Empty,
  Field,
  Input,
  Notice,
  Panel,
  PanelBody,
  PanelFoot,
  PanelHead,
  PanelNote,
  Select,
  Textarea,
  Verdict,
} from '@/components';
import {
  MINISTRIES,
  VENDORS,
  ministryName,
  recordLedgerEntry,
  setDebarment,
  vendorName,
  type ApiFailure,
  type DebarmentResponse,
} from '@/lib/api-client';

/**
 * Debarment, recorded by a nodal ministry onto the shared national ledger.
 *
 * The window is stated in blocks because this runtime has no `pallet_timestamp` — time is
 * block height, and a debarment order that says "block 1,428,000" is a fact the chain can
 * check, where a wall-clock date would be a claim it cannot. The panel therefore shows the
 * block window *and* what it works out to at this network's 200 ms block time, so an
 * administrator can set "two years" without doing the arithmetic.
 */

/** Cerulea produces a block every 200 ms (see the consensus configuration). */
const BLOCKS_PER_SECOND = 5;
const BLOCKS_PER_DAY = BLOCKS_PER_SECOND * 60 * 60 * 24;

const DURATIONS = [
  { label: '6 months', days: 182 },
  { label: '1 year', days: 365 },
  { label: '2 years (GFR Rule 151(iii) maximum)', days: 730 },
] as const;

const REASON_MAX_BYTES = 256;

export function DebarmentPanel({ defaultMinistryId }: { defaultMinistryId: string }) {
  const [query, setQuery] = useState('');
  const [vendorAccount, setVendorAccount] = useState(VENDORS[2].account);
  const [ministryId, setMinistryId] = useState(defaultMinistryId);
  const [action, setAction] = useState<'debar' | 'lift'>('debar');
  const [effectiveFrom, setEffectiveFrom] = useState('');
  const [effectiveTo, setEffectiveTo] = useState('');
  const [reason, setReason] = useState(
    'Local content declared at 86% on GEM/2025/B/6712330 contradicted by a 30% declaration for the same product; verification under GFR Rule 151(iii).',
  );

  const [chainHead, setChainHead] = useState<number | null>(null);
  const [pending, setPending] = useState(false);
  const [verdict, setVerdict] = useState<DebarmentResponse | null>(null);
  const [failure, setFailure] = useState<ApiFailure | null>(null);

  // The status endpoint belongs to the API layer and may not be present. If it answers,
  // the effective-from default becomes the live chain head; if not, the field simply
  // stays blank and the route defaults it to the current block itself.
  useEffect(() => {
    let cancelled = false;
    fetch('/api/chain/status')
      .then((response) => (response.ok ? response.json() : null))
      .then((payload: unknown) => {
        if (cancelled || typeof payload !== 'object' || payload === null) return;
        const value = (payload as Record<string, unknown>).finalizedBlock;
        const block = typeof value === 'number' ? value : Number(value);
        if (Number.isFinite(block)) setChainHead(block);
      })
      .catch(() => {
        // A missing status endpoint is not an error worth showing anyone.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const matches = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (needle === '') return VENDORS;
    return VENDORS.filter(
      (vendor) =>
        vendor.name.toLowerCase().includes(needle) ||
        vendor.city.toLowerCase().includes(needle) ||
        vendor.state.toLowerCase().includes(needle) ||
        vendor.udyam.toLowerCase().includes(needle),
    );
  }, [query]);

  const selectedVendor = VENDORS.find((vendor) => vendor.account === vendorAccount);
  const fromBlock = effectiveFrom.trim() === '' ? chainHead : Number(effectiveFrom);
  const toBlock = effectiveTo.trim() === '' ? null : Number(effectiveTo);

  const windowError =
    toBlock !== null && fromBlock !== null && toBlock <= fromBlock
      ? 'A debarment that ends before it starts would never be enforced. The end block must be later than the start block.'
      : undefined;
  const reasonBytes = new TextEncoder().encode(reason).length;
  const reasonError =
    reasonBytes > REASON_MAX_BYTES
      ? `The recorded reason must be at most ${REASON_MAX_BYTES} bytes; this is ${reasonBytes}.`
      : undefined;

  function applyDuration(days: number) {
    const start = fromBlock ?? 0;
    setEffectiveFrom(String(start));
    setEffectiveTo(String(start + days * BLOCKS_PER_DAY));
  }

  async function submit() {
    if (windowError || reasonError) return;
    setPending(true);
    setFailure(null);
    setVerdict(null);

    const outcome = await setDebarment({
      vendor: vendorAccount,
      ministry: ministryId,
      action,
      ...(action === 'debar'
        ? {
            ...(effectiveFrom.trim() === '' ? {} : { effectiveFrom: Number(effectiveFrom) }),
            ...(toBlock === null ? {} : { effectiveTo: toBlock }),
            reason: reason.trim() || undefined,
          }
        : {}),
    });
    setPending(false);

    if (!outcome.ok) {
      setFailure(outcome);
      return;
    }

    setVerdict(outcome.data);
    recordLedgerEntry({
      kind: 'debarment',
      persona: 'Nodal Ministry Administrator',
      result: outcome.data.result,
      headline: `${outcome.data.status === 'DebarmentLifted' ? 'Debarment lifted' : 'Debarred'} — ${vendorName(vendorAccount)}`,
      reason: outcome.data.reason,
      ministry: ministryId,
      vendor: vendorAccount,
      txRef: outcome.data.txRef,
      blockNumber: outcome.data.blockNumber,
      pathway: 'P12',
      facts: {
        Action: action === 'debar' ? 'Debar' : 'Lift',
        'Effective from block': String(outcome.data.effectiveFrom ?? '—'),
        'Effective to block':
          outcome.data.effectiveTo === null || outcome.data.effectiveTo === undefined
            ? 'No end date recorded'
            : String(outcome.data.effectiveTo),
      },
    });
  }

  return (
    <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
      <Panel className="xl:col-span-2">
        <PanelHead title="Debarment register" />
        <PanelBody className="space-y-4">
          <Field label="Find a vendor" hint="Search by name, city, state or Udyam registration.">
            {(props) => (
              <div className="relative">
                <Search
                  className="pointer-events-none absolute inset-y-0 left-3 my-auto size-4 text-ink-subtle"
                  aria-hidden="true"
                />
                <Input
                  {...props}
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="Chambal, Bhiwadi, UDYAM-RJ…"
                  className="pl-8"
                />
              </div>
            )}
          </Field>

          {matches.length === 0 ? (
            <Empty
              title="No vendor matches that search"
              source="Search the registered supplier list by company name, city, state or Udyam number."
            />
          ) : (
            <ul className="divide-y divide-line overflow-hidden rounded-md border border-line">
              {matches.map((vendor) => {
                const selected = vendor.account === vendorAccount;
                return (
                  <li key={vendor.account}>
                    <button
                      type="button"
                      onClick={() => setVendorAccount(vendor.account)}
                      aria-pressed={selected}
                      className={`flex w-full flex-wrap items-center justify-between gap-3 px-3 py-2 text-left transition-colors duration-150 ${
                        selected ? 'bg-accent-tint' : 'bg-paper hover:bg-shell'
                      }`}
                    >
                      <span className="min-w-0">
                        <span className="block text-sm font-medium text-ink">{vendor.name}</span>
                        <span className="mt-0.5 block text-2xs text-ink-muted">
                          {vendor.city}, {vendor.state} · {vendor.udyam}
                        </span>
                      </span>
                      <Chip tone={selected ? 'accent' : 'neutral'}>{vendor.msme}</Chip>
                    </button>
                  </li>
                );
              })}
            </ul>
          )}

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="Recording ministry" required>
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

            <Field label="Order" required>
              {(props) => (
                <Select
                  {...props}
                  value={action}
                  onChange={(event) => setAction(event.target.value as 'debar' | 'lift')}
                >
                  <option value="debar">Debar the vendor</option>
                  <option value="lift">Lift an existing debarment</option>
                </Select>
              )}
            </Field>
          </div>

          {action === 'debar' && (
            <>
              <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                <Field
                  label="Effective from block"
                  hint={
                    chainHead === null
                      ? 'Leave blank to start at the current block.'
                      : `Leave blank to start at the current block (#${chainHead.toLocaleString('en-IN')}).`
                  }
                >
                  {(props) => (
                    <Input
                      {...props}
                      type="number"
                      min={0}
                      step={1}
                      value={effectiveFrom}
                      onChange={(event) => setEffectiveFrom(event.target.value)}
                      placeholder={chainHead === null ? 'Current block' : String(chainHead)}
                      className="font-mono"
                    />
                  )}
                </Field>

                <Field
                  label="Effective to block (optional)"
                  error={windowError}
                  hint="Leave blank for a debarment with no end date recorded."
                >
                  {(props) => (
                    <Input
                      {...props}
                      type="number"
                      min={0}
                      step={1}
                      value={effectiveTo}
                      onChange={(event) => setEffectiveTo(event.target.value)}
                      placeholder="No end date"
                      className="font-mono"
                    />
                  )}
                </Field>
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <span className="text-2xs font-medium tracking-wide text-ink-muted uppercase">
                  Set a period
                </span>
                {DURATIONS.map((duration) => (
                  <Button key={duration.label} size="xs" onClick={() => applyDuration(duration.days)}>
                    {duration.label}
                  </Button>
                ))}
                {(effectiveFrom || effectiveTo) && (
                  <Button
                    variant="quiet"
                    size="xs"
                    onClick={() => {
                      setEffectiveFrom('');
                      setEffectiveTo('');
                    }}
                  >
                    Clear
                  </Button>
                )}
              </div>

              <Field
                label="Reason recorded on the order"
                required
                error={reasonError}
                hint={`${reasonBytes} of ${REASON_MAX_BYTES} bytes. This text is written to the chain and is what a blocked bid quotes back.`}
              >
                {(props) => (
                  <Textarea
                    {...props}
                    value={reason}
                    onChange={(event) => setReason(event.target.value)}
                  />
                )}
              </Field>
            </>
          )}
        </PanelBody>

        <PanelFoot>
          <p className="text-2xs text-ink-muted">
            A debarment recorded here blocks the vendor under every ministry, not only this one.
          </p>
          <Button
            variant="primary"
            onClick={submit}
            loading={pending}
            loadingLabel={action === 'debar' ? 'Recording the debarment' : 'Lifting the debarment'}
            disabled={Boolean(windowError || reasonError)}
            icon={
              action === 'debar' ? (
                <Ban className="size-3.5" aria-hidden="true" />
              ) : (
                <RotateCcw className="size-3.5" aria-hidden="true" />
              )
            }
          >
            {action === 'debar' ? 'Record debarment order' : 'Lift debarment'}
          </Button>
        </PanelFoot>
      </Panel>

      <div className="space-y-4">
        {pending && <Awaiting label="Recording the order" steps={false} />}

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
            trigger={`Debarment · ${vendorName(vendorAccount)}`}
            txRef={verdict.txRef ?? undefined}
            blockNumber={verdict.blockNumber ?? undefined}
            latencyMs={verdict.latencyMs}
            pathway="P12"
            blockedBy={
              verdict.result === 'RED'
                ? {
                    kind: 'debarment',
                    label: `Debarred by ${ministryName(ministryId)}`,
                    detail: describeWindow(verdict.effectiveFrom, verdict.effectiveTo),
                    citation: 'GFR Rule 151(iii); enforced across every ministry from the shared ledger.',
                  }
                : undefined
            }
            records={[
              { label: 'Vendor', value: vendorName(vendorAccount) },
              { label: 'Recording ministry', value: ministryName(ministryId) },
              { label: 'Order', value: verdict.status ?? '—' },
            ]}
          />
        )}

        <Panel>
          <PanelHead title="Effective window" />
          <PanelBody>
            <DataList>
              <DataRow label="Vendor" value={selectedVendor?.name ?? vendorAccount} />
              <DataRow
                label="From block"
                value={fromBlock === null ? 'Current block at submission' : `#${fromBlock.toLocaleString('en-IN')}`}
                mono
              />
              <DataRow
                label="To block"
                value={toBlock === null ? 'No end date recorded' : `#${toBlock.toLocaleString('en-IN')}`}
                mono
              />
              <DataRow
                label="Duration"
                value={
                  toBlock === null || fromBlock === null
                    ? 'Open-ended until lifted'
                    : approximateDuration(toBlock - fromBlock)
                }
              />
            </DataList>
          </PanelBody>
          <PanelNote>
            Stated in blocks, because this runtime keeps time as block height. Cerulea produces a
            block every 200 ms, so a day is {BLOCKS_PER_DAY.toLocaleString('en-IN')} blocks. The
            duration above is that arithmetic, not a separate record.
          </PanelNote>
        </Panel>
      </div>
    </div>
  );
}

function describeWindow(from: number | undefined, to: number | null | undefined): string {
  const start = from === undefined ? 'the current block' : `block ${from.toLocaleString('en-IN')}`;
  const end =
    to === null || to === undefined
      ? 'with no end date recorded'
      : `until block ${to.toLocaleString('en-IN')}`;
  return `In force from ${start} ${end}.`;
}

function approximateDuration(blocks: number): string {
  const days = blocks / BLOCKS_PER_DAY;
  if (days >= 365) return `≈ ${(days / 365).toFixed(1)} years (${blocks.toLocaleString('en-IN')} blocks)`;
  if (days >= 1) return `≈ ${Math.round(days)} days (${blocks.toLocaleString('en-IN')} blocks)`;
  return `${blocks.toLocaleString('en-IN')} blocks`;
}
