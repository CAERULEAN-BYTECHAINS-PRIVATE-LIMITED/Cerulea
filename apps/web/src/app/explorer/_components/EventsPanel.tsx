'use client';

import { Radio } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  ErrorState,
  Field,
  Input,
  TxRef,
} from '@/components';
import { eventFields, formatBlockNumber, formatClock } from './format';
import type { EventsResponse, ExplorerEvent } from './types';
import { usePolledResource } from './usePolledResource';

/** The six pallets the PoC document is about, plus the consensus pallets underneath. */
const PRAMAAN_SECTIONS = [
  'pramaanRuleRegistry',
  'pramaanClassification',
  'pramaanPreference',
  'pramaanCertification',
  'pramaanDebarment',
  'pramaanConsistency',
];

const SECTION_LABELS: Record<string, string> = {
  pramaanRuleRegistry: 'Rule registry',
  pramaanClassification: 'Classification',
  pramaanPreference: 'Preference',
  pramaanCertification: 'Certification',
  pramaanDebarment: 'Debarment',
  pramaanConsistency: 'Consistency',
};

type Scope = 'pramaan' | 'all' | string;

export function EventsPanel({ onOpenTx }: { onOpenTx: (txRef: string) => void }) {
  const [scope, setScope] = useState<Scope>('pramaan');
  const [query, setQuery] = useState('');

  const url = useMemo(() => {
    const params = new URLSearchParams({ limit: '80' });
    if (scope === 'pramaan') PRAMAAN_SECTIONS.forEach((section) => params.append('pallet', section));
    else if (scope !== 'all') params.append('pallet', scope);
    return `/api/chain/events?${params.toString()}`;
  }, [scope]);

  const { data, loading, error, refresh } = usePolledResource<EventsResponse>(url, 2_500);

  const events = useMemo(() => {
    if (!data) return [];
    const needle = query.trim().toLowerCase();
    if (!needle) return data.events;
    return data.events.filter(
      (event) =>
        `${event.section}.${event.method}`.toLowerCase().includes(needle) ||
        JSON.stringify(event.data).toLowerCase().includes(needle) ||
        (event.txRef ?? '').toLowerCase().includes(needle),
    );
  }, [data, query]);

  if (loading && !data) {
    return (
      <Card>
        <CardHeader title="Runtime events" description="Reading events from recent blocks." />
        <div className="space-y-2 px-5 py-4" role="status" aria-live="polite">
          <span className="sr-only">Reading runtime events.</span>
          {[0, 1, 2, 3, 4].map((key) => (
            <div key={key} className="h-16 rounded bg-surface-sunken" />
          ))}
        </div>
      </Card>
    );
  }

  if (!data) {
    return (
      <ErrorState
        kind="chain-unreachable"
        detail="Runtime events could not be decoded from recent blocks."
        technicalDetail={error ?? undefined}
        onRetry={refresh}
      />
    );
  }

  const otherSections = data.sections.filter(
    (section) => !PRAMAAN_SECTIONS.includes(section.section),
  );
  const pramaanCount = data.sections
    .filter((section) => PRAMAAN_SECTIONS.includes(section.section))
    .reduce((total, section) => total + section.count, 0);

  return (
    <div className="space-y-5">
      <Card>
        <CardHeader
          title="Runtime events"
          description={
            `Decoded from the ${data.window.blocksIndexed.toLocaleString('en-IN')} blocks this explorer holds, ` +
            `${formatBlockNumber(data.window.from)} to ${formatBlockNumber(data.window.to)}.` +
            (data.window.blocksWithoutEvents > 0
              ? ` ${data.window.blocksWithoutEvents.toLocaleString('en-IN')} of them were read after the node pruned their state, so their events are not available here.`
              : '')
          }
        />
        <CardBody className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <FilterChip
              active={scope === 'pramaan'}
              onClick={() => setScope('pramaan')}
              count={pramaanCount}
            >
              PRAMAAN pallets
            </FilterChip>
            <FilterChip
              active={scope === 'all'}
              onClick={() => setScope('all')}
              count={data.sections.reduce((total, section) => total + section.count, 0)}
            >
              All pallets
            </FilterChip>
            {data.sections
              .filter((section) => PRAMAAN_SECTIONS.includes(section.section))
              .map((section) => (
                <FilterChip
                  key={section.section}
                  active={scope === section.section}
                  onClick={() => setScope(section.section)}
                  count={section.count}
                >
                  {SECTION_LABELS[section.section] ?? section.section}
                </FilterChip>
              ))}
            {otherSections.map((section) => (
              <FilterChip
                key={section.section}
                active={scope === section.section}
                onClick={() => setScope(section.section)}
                count={section.count}
              >
                {section.section}
              </FilterChip>
            ))}
          </div>

          <Field
            label="Filter within these events"
            hint="Matches an event name, any decoded field value, or a transaction reference."
          >
            {(props) => (
              <Input
                {...props}
                type="search"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                placeholder="Vendor address, tender id, or event name"
              />
            )}
          </Field>
        </CardBody>
      </Card>

      {events.length === 0 ? (
        <EmptyState
          icon={<Radio className="size-5" aria-hidden="true" />}
          title={
            query
              ? 'No event in this window matches that filter'
              : 'No events from these pallets in the current window'
          }
          description={
            query
              ? 'The filter matches an event name, a decoded field value, or a transaction reference. Clear it to see everything in the window.'
              : 'Every classification, preference calculation, certification, debarment and rule change appears here within a second of being finalized. Run a trigger point from a persona console or the guided walkthrough and it will show up.'
          }
          action={
            query ? (
              <Button size="sm" variant="secondary" onClick={() => setQuery('')}>
                Clear filter
              </Button>
            ) : undefined
          }
        />
      ) : (
        <ul className="space-y-3">
          {events.map((event) => (
            <EventCard key={event.id} event={event} onOpenTx={onOpenTx} />
          ))}
        </ul>
      )}
    </div>
  );
}

function FilterChip({
  active,
  count,
  onClick,
  children,
}: {
  active: boolean;
  count: number;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-pressed={active}
      className={
        'inline-flex items-center gap-1.5 rounded-full border px-3 py-1.5 text-xs font-medium ' +
        'transition-colors duration-150 ease-out ' +
        'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea ' +
        (active
          ? 'border-cerulea bg-cerulea text-white'
          : 'border-border bg-surface text-ink-muted hover:border-cerulea/40 hover:text-ink')
      }
    >
      {children}
      <span className={active ? 'font-mono text-white/80' : 'font-mono text-ink-subtle'}>
        {count.toLocaleString('en-IN')}
      </span>
    </button>
  );
}

function EventCard({
  event,
  onOpenTx,
}: {
  event: ExplorerEvent;
  onOpenTx: (txRef: string) => void;
}) {
  const fields = eventFields(event.data);
  const isPramaan = event.section.startsWith('pramaan');

  return (
    <li>
      <Card>
        <CardBody className="space-y-3">
          <div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone={isPramaan ? 'brand' : 'neutral'} size="sm">
                {SECTION_LABELS[event.section] ?? event.section}
              </Badge>
              <span className="font-mono text-sm font-semibold text-ink">{event.method}</span>
            </div>
            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-ink-muted">
              <span className="font-mono">{formatBlockNumber(event.blockNumber)}</span>
              <span>{formatClock(event.timestamp)}</span>
              {event.txRef && (
                <button
                  type="button"
                  onClick={() => onOpenTx(event.txRef as string)}
                  className="rounded font-medium text-cerulea transition-colors duration-150 hover:text-cerulea-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
                >
                  Trace this transaction
                </button>
              )}
            </div>
          </div>

          {fields.length > 0 && (
            <dl className="grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
              {fields.map((field) => (
                <div key={field.label} className="min-w-0">
                  <dt className="text-xs tracking-wide text-ink-subtle uppercase">
                    {field.label}
                  </dt>
                  <dd className="truncate font-mono text-[0.8125rem] text-ink" title={field.value}>
                    {field.value}
                  </dd>
                </div>
              ))}
            </dl>
          )}

          {event.txRef && (
            <p className="flex flex-wrap items-center gap-2 border-t border-border pt-3 text-xs text-ink-muted">
              <span className="tracking-wide uppercase">Txn</span>
              <TxRef value={event.txRef} />
            </p>
          )}
        </CardBody>
      </Card>
    </li>
  );
}
