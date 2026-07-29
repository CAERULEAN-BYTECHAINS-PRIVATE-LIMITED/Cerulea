'use client';

import { useMemo, useState } from 'react';
import {
  Button,
  Chip,
  Empty,
  Field,
  Hash,
  Input,
  Nil,
  Notice,
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  SkeletonRows,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  cn,
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
      <Panel>
        <PanelHead title="Runtime events" />
        <SkeletonRows rows={6} label="Reading runtime events." />
      </Panel>
    );
  }

  if (!data) {
    return (
      <Notice
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
    <div className="space-y-4">
      <Panel>
        <PanelHead title="Runtime events" meta={<Chip>{events.length} shown</Chip>} />
        <PanelBody className="space-y-3">
          <div className="scroll-x flex flex-wrap items-center gap-1.5">
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
        </PanelBody>
        <PanelNote>
          Decoded from the {data.window.blocksIndexed.toLocaleString('en-IN')} blocks this explorer
          holds, {formatBlockNumber(data.window.from)} to {formatBlockNumber(data.window.to)}.
          {data.window.blocksWithoutEvents > 0
            ? ` ${data.window.blocksWithoutEvents.toLocaleString('en-IN')} of them were read after the node pruned their state, so their events are not available here.`
            : ''}
        </PanelNote>
      </Panel>

      {events.length === 0 ? (
        <Empty
          title={
            query
              ? 'No event in this window matches that filter'
              : 'No events from these pallets in the current window'
          }
          source={
            query
              ? 'The filter matches an event name, a decoded field value, or a transaction reference.'
              : 'Every classification, preference calculation, certification, debarment and rule change appears here within a second of being finalized.'
          }
          action={query ? <Button onClick={() => setQuery('')}>Clear filter</Button> : undefined}
        />
      ) : (
        <Panel>
          <Table>
            <THead>
              <TR>
                <TH className="w-28">Block</TH>
                <TH className="w-24">Time</TH>
                <TH className="w-36">Pallet</TH>
                <TH className="w-52">Event</TH>
                <TH>Decoded fields</TH>
                <TH className="w-44">Transaction</TH>
              </TR>
            </THead>
            <TBody>
              {events.map((event) => (
                <EventRow key={event.id} event={event} onOpenTx={onOpenTx} />
              ))}
            </TBody>
          </Table>
        </Panel>
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
      className={cn(
        'inline-flex shrink-0 items-center gap-1.5 rounded-sm border px-2 py-0.5 text-2xs font-medium',
        'transition-colors duration-150',
        'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
        active
          ? 'border-accent bg-accent text-white'
          : 'border-line bg-paper text-ink-muted hover:bg-shell hover:text-ink',
      )}
    >
      {children}
      <span className={cn('font-mono', active ? 'text-white/75' : 'text-ink-subtle')}>
        {count.toLocaleString('en-IN')}
      </span>
    </button>
  );
}

function EventRow({
  event,
  onOpenTx,
}: {
  event: ExplorerEvent;
  onOpenTx: (txRef: string) => void;
}) {
  const fields = eventFields(event.data);
  const isPramaan = event.section.startsWith('pramaan');

  return (
    <TR className="hover:bg-shell">
      <TD mono>{formatBlockNumber(event.blockNumber)}</TD>
      <TD className="whitespace-nowrap text-2xs text-ink-muted">{formatClock(event.timestamp)}</TD>
      <TD>
        <Chip tone={isPramaan ? 'accent' : 'neutral'}>
          {SECTION_LABELS[event.section] ?? event.section}
        </Chip>
      </TD>
      <TD className="font-mono text-2xs font-semibold text-ink">{event.method}</TD>
      <TD>
        {fields.length === 0 ? (
          <Nil label="No decoded fields" />
        ) : (
          <dl className="flex flex-wrap gap-x-4 gap-y-0.5">
            {fields.map((field) => (
              <div key={field.label} className="min-w-0">
                <dt className="text-2xs tracking-wide text-ink-subtle uppercase">{field.label}</dt>
                <dd
                  className="max-w-56 truncate font-mono text-2xs text-ink"
                  title={field.value}
                >
                  {field.value}
                </dd>
              </div>
            ))}
          </dl>
        )}
      </TD>
      <TD>
        {event.txRef ? (
          <span className="flex flex-col items-start gap-0.5">
            <Hash value={event.txRef} />
            <button
              type="button"
              onClick={() => onOpenTx(event.txRef as string)}
              className="rounded-sm text-2xs font-medium text-accent transition-colors duration-150 hover:text-accent-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
            >
              Trace this transaction
            </button>
          </span>
        ) : (
          <Nil label="No transaction" />
        )}
      </TD>
    </TR>
  );
}
