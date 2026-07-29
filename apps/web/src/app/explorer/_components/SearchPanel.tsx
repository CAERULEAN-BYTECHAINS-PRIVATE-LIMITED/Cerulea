'use client';

import { CircleCheckBig, CircleDashed, Search } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  DataRow,
  EmptyState,
  ErrorState,
  Field,
  Input,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  TxRef,
} from '@/components';
import { eventFields, formatBlockNumber, formatClock } from './format';
import type { ExplorerEvent, SearchResponse } from './types';

type State =
  | { status: 'idle' }
  | { status: 'searching'; term: string }
  | { status: 'found'; result: SearchResponse }
  | { status: 'missing'; term: string; message: string }
  | { status: 'error'; message: string };

export function SearchPanel({
  initialTx,
  initialBlock,
  onTermConsumed,
}: {
  initialTx?: string;
  initialBlock?: number;
  onTermConsumed?: () => void;
}) {
  const [term, setTerm] = useState(initialTx ?? (initialBlock ? String(initialBlock) : ''));
  const [state, setState] = useState<State>({ status: 'idle' });

  const run = useCallback(
    async (raw: string) => {
      const value = raw.trim();
      if (!value) {
        setState({ status: 'idle' });
        return;
      }
      setState({ status: 'searching', term: value });

      const params = new URLSearchParams();
      if (value.startsWith('0x')) params.set('tx', value.toLowerCase());
      else params.set('block', value.replace(/[^0-9]/g, ''));

      try {
        const response = await fetch(`/api/chain/search?${params.toString()}`, {
          cache: 'no-store',
        });
        const payload = (await response.json()) as SearchResponse & { error?: string };
        if (response.ok && payload.found) {
          setState({ status: 'found', result: payload });
        } else if (response.status === 404) {
          setState({
            status: 'missing',
            term: value,
            message: payload.error ?? 'Nothing matched that reference.',
          });
        } else {
          setState({ status: 'error', message: payload.error ?? `Search failed (${response.status})` });
        }
      } catch (error) {
        setState({
          status: 'error',
          message: error instanceof Error ? error.message : 'Search failed',
        });
      } finally {
        onTermConsumed?.();
      }
    },
    [onTermConsumed],
  );

  // A `?tx=` in the URL is a jump target: `ComplianceResult` links straight here after a
  // trigger point returns, so the search must run without the judge pressing anything.
  useEffect(() => {
    const jump = initialTx ?? (initialBlock !== undefined ? String(initialBlock) : undefined);
    if (!jump) return;
    setTerm(jump);
    void run(jump);
  }, [initialTx, initialBlock, run]);

  return (
    <div className="space-y-5">
      <Card>
        <CardHeader
          title="Find a decision"
          description="Paste the transaction reference from any compliance result, or a block number, to see the block it was sealed in and the events it emitted."
        />
        <CardBody>
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              void run(term);
            }}
          >
            <Field
              label="Transaction reference or block number"
              hint="A transaction reference is 0x followed by 64 hexadecimal characters."
              className="min-w-0 flex-1"
            >
              {(props) => (
                <Input
                  {...props}
                  value={term}
                  onChange={(event) => setTerm(event.target.value)}
                  placeholder="0x… or 5421"
                  spellCheck={false}
                  className="font-mono"
                />
              )}
            </Field>
            <Button
              type="submit"
              loading={state.status === 'searching'}
              loadingLabel="Searching the chain"
              leadingIcon={<Search className="size-4" aria-hidden="true" />}
            >
              Search
            </Button>
          </form>
        </CardBody>
      </Card>

      {state.status === 'idle' && (
        <EmptyState
          icon={<Search className="size-5" aria-hidden="true" />}
          title="Nothing searched yet"
          description="Every compliance result in this console carries a transaction reference. Open the on-chain record on any verdict and follow the explorer link, or paste a reference above."
        />
      )}

      {state.status === 'error' && (
        <ErrorState
          kind="chain-unreachable"
          title="The search could not be completed"
          detail="The explorer could not reach the node to resolve that reference. Nothing about the transaction has changed."
          technicalDetail={state.message}
          onRetry={() => void run(term)}
        />
      )}

      {state.status === 'missing' && (
        <EmptyState
          icon={<Search className="size-5" aria-hidden="true" />}
          title="No match in the explorer's search window"
          description={state.message}
          action={
            <Button size="sm" variant="secondary" onClick={() => void run(state.term)}>
              Search again
            </Button>
          }
        />
      )}

      {state.status === 'found' && <SearchResult result={state.result} />}
    </div>
  );
}

function SearchResult({ result }: { result: SearchResponse }) {
  const { block, events } = result;
  const pramaanEvents = events.filter((event) => event.section.startsWith('pramaan'));

  return (
    <div className="space-y-5">
      <Card>
        <CardHeader
          title={
            result.kind === 'transaction'
              ? 'Transaction found'
              : `Block ${formatBlockNumber(block.number)}`
          }
          description={
            result.kind === 'transaction'
              ? `Included in block ${formatBlockNumber(block.number)} and ${block.finalized ? 'finalized' : 'not yet finalized'}.`
              : 'The block, its author, and everything it emitted.'
          }
          actions={
            block.finalized ? (
              <Badge tone="teal" icon={<CircleCheckBig className="size-3" aria-hidden="true" />}>
                Finalized
              </Badge>
            ) : (
              <Badge tone="neutral" icon={<CircleDashed className="size-3" aria-hidden="true" />}>
                Included, awaiting finality
              </Badge>
            )
          }
        />
        <CardBody>
          <dl>
            {result.txRef && (
              <DataRow label="Transaction reference" value={<TxRef value={result.txRef} />} />
            )}
            {result.extrinsic && (
              <DataRow
                label="Call"
                value={`${result.extrinsic.section}.${result.extrinsic.method}`}
                mono
              />
            )}
            <DataRow label="Block number" value={formatBlockNumber(block.number)} mono />
            <DataRow label="Block hash" value={<TxRef value={block.hash} label="Block hash" />} />
            <DataRow label="Sealed at" value={formatClock(block.timestamp)} mono />
            <DataRow
              label="Authored by"
              value={
                block.author ? (
                  <TxRef value={block.author} label="Validator address" />
                ) : (
                  'Not recorded in the digest'
                )
              }
            />
            <DataRow
              label="Finalized head at read time"
              value={formatBlockNumber(result.finalizedBlock)}
              mono
            />
          </dl>
        </CardBody>
      </Card>

      <Card>
        <CardHeader
          title={
            result.kind === 'transaction'
              ? 'Events emitted by this transaction'
              : 'Events emitted in this block'
          }
          description={
            pramaanEvents.length > 0
              ? 'The compliance events are listed first; the consensus events beneath them are what makes the record final.'
              : 'No compliance event was emitted here — this block carried consensus traffic only.'
          }
        />
        {events.length === 0 ? (
          <CardBody>
            <EmptyState
              title="No events recorded against this reference"
              description="The extrinsic was included but emitted no runtime events. That normally means it was an inherent, such as the block timestamp."
            />
          </CardBody>
        ) : (
          <CardBody className="space-y-4">
            {[...pramaanEvents, ...events.filter((event) => !event.section.startsWith('pramaan'))].map(
              (event) => (
                <EventDetail key={event.id} event={event} />
              ),
            )}
          </CardBody>
        )}
      </Card>

      {result.kind === 'block' && block.extrinsics.length > 0 && (
        <Card>
          <CardHeader title="Extrinsics in this block" />
          <Table containerClassName="rounded-b-card">
            <THead>
              <TR>
                <TH className="w-16">Index</TH>
                <TH>Call</TH>
                <TH>Transaction reference</TH>
              </TR>
            </THead>
            <TBody>
              {block.extrinsics.map((extrinsic) => (
                <TR key={extrinsic.hash}>
                  <TD mono>{extrinsic.index}</TD>
                  <TD mono>
                    {extrinsic.section}.{extrinsic.method}
                  </TD>
                  <TD>
                    <TxRef value={extrinsic.hash} />
                  </TD>
                </TR>
              ))}
            </TBody>
          </Table>
        </Card>
      )}
    </div>
  );
}

function EventDetail({ event }: { event: ExplorerEvent }) {
  const fields = eventFields(event.data);
  const isPramaan = event.section.startsWith('pramaan');

  return (
    <div
      className={
        'rounded-lg border px-4 py-3 ' +
        (isPramaan ? 'border-cerulea/20 bg-cerulea-light/40' : 'border-border bg-surface-sunken')
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        <Badge tone={isPramaan ? 'brand' : 'neutral'} size="sm">
          {event.section}
        </Badge>
        <span className="font-mono text-sm font-semibold text-ink">{event.method}</span>
      </div>
      {fields.length > 0 && (
        <dl className="mt-2.5 grid gap-x-6 gap-y-2 sm:grid-cols-2 lg:grid-cols-3">
          {fields.map((field) => (
            <div key={field.label} className="min-w-0">
              <dt className="text-xs tracking-wide text-ink-subtle uppercase">{field.label}</dt>
              <dd className="truncate font-mono text-[0.8125rem] text-ink" title={field.value}>
                {field.value}
              </dd>
            </div>
          ))}
        </dl>
      )}
    </div>
  );
}
