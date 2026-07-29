'use client';

import { Search } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import {
  Button,
  Chip,
  DataList,
  DataRow,
  Empty,
  FactGrid,
  Field,
  Hash,
  Input,
  Nil,
  Notice,
  Panel,
  PanelBody,
  PanelHead,
  PanelNote,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  cn,
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
  const jumpTarget = initialTx ?? (initialBlock !== undefined ? String(initialBlock) : undefined);
  const [term, setTerm] = useState(jumpTarget ?? '');
  // A `?tx=` in the URL means the lookup is already under way as far as the reader is
  // concerned, so the panel starts in the searching state rather than flipping into it
  // from an effect. `ExplorerClient` remounts this component per jump target, so the
  // initial state is always the right one for the target it was mounted for.
  const [state, setState] = useState<State>(
    jumpTarget ? { status: 'searching', term: jumpTarget } : { status: 'idle' },
  );

  const execute = useCallback(
    async (raw: string) => {
      const value = raw.trim();
      if (!value) {
        setState({ status: 'idle' });
        return;
      }

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

  /** Set the searching state, then fetch. Used by the form; the jump path skips the first
   *  half because the component already mounted in the searching state. */
  const run = useCallback(
    (raw: string) => {
      const value = raw.trim();
      if (value) setState({ status: 'searching', term: value });
      void execute(value);
    },
    [execute],
  );

  // A `?tx=` in the URL is a jump target: `Verdict` links straight here after a trigger
  // point returns, so the search must run without the judge pressing anything. The effect
  // only performs the request — no state is set synchronously here, because the searching
  // state was already the component's initial state.
  useEffect(() => {
    if (!jumpTarget) return;
    // `execute` writes state only after awaiting the fetch, which is the pattern the rule's
    // own guidance recommends. The rule follows the call graph and cannot see that every
    // write is post-await, so it is silenced here rather than the effect restructured.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    void execute(jumpTarget);
  }, [jumpTarget, execute]);

  return (
    <div className="space-y-4">
      <Panel>
        <PanelHead title="Find a decision" />
        <PanelBody>
          <form
            className="flex flex-wrap items-end gap-3"
            onSubmit={(event) => {
              event.preventDefault();
              run(term);
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
              variant="primary"
              type="submit"
              loading={state.status === 'searching'}
              loadingLabel="Searching the chain"
              icon={<Search className="size-3.5" aria-hidden="true" />}
            >
              Search
            </Button>
          </form>
        </PanelBody>
        <PanelNote>
          Every compliance result carries a transaction reference. Paste one here to see the block
          it was sealed in and the events it emitted.
        </PanelNote>
      </Panel>

      {state.status === 'idle' && (
        <Empty
          title="Nothing searched yet"
          source="Open the on-chain record on any verdict and follow the explorer link, or paste a reference above."
        />
      )}

      {state.status === 'error' && (
        <Notice
          kind="chain-unreachable"
          title="The search could not be completed"
          detail="The explorer could not reach the node to resolve that reference. Nothing about the transaction has changed."
          technicalDetail={state.message}
          onRetry={() => run(term)}
        />
      )}

      {state.status === 'missing' && (
        <Empty
          title="No match in the explorer's search window"
          source={state.message}
          action={<Button onClick={() => run(state.term)}>Search again</Button>}
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
    <div className="space-y-4">
      <Panel>
        <PanelHead
          title={
            result.kind === 'transaction'
              ? 'Transaction found'
              : `Block ${formatBlockNumber(block.number)}`
          }
          meta={
            block.finalized ? (
              <Chip tone="accent">Finalized</Chip>
            ) : (
              <Chip>Included, awaiting finality</Chip>
            )
          }
        />
        <PanelBody>
          <DataList columns={2}>
            {result.txRef && (
              <DataRow label="Transaction reference" value={<Hash value={result.txRef} />} />
            )}
            {result.extrinsic && (
              <DataRow
                label="Call"
                value={`${result.extrinsic.section}.${result.extrinsic.method}`}
                mono
              />
            )}
            <DataRow label="Block number" value={formatBlockNumber(block.number)} mono />
            <DataRow label="Block hash" value={<Hash value={block.hash} label="Block hash" />} />
            <DataRow label="Sealed at" value={formatClock(block.timestamp)} mono />
            <DataRow
              label="Authored by"
              value={
                block.author ? (
                  <Hash value={block.author} label="Validator address" />
                ) : (
                  <Nil label="Not recorded in the digest" />
                )
              }
            />
            <DataRow
              label="Finalized head at read time"
              value={formatBlockNumber(result.finalizedBlock)}
              mono
            />
          </DataList>
        </PanelBody>
      </Panel>

      <Panel>
        <PanelHead
          title={
            result.kind === 'transaction'
              ? 'Events emitted by this transaction'
              : 'Events emitted in this block'
          }
          meta={<Chip>{events.length}</Chip>}
        />
        {events.length === 0 ? (
          <PanelBody>
            <Empty
              title="No events recorded against this reference"
              source="The extrinsic was included but emitted no runtime events. That normally means it was an inherent, such as the block timestamp."
            />
          </PanelBody>
        ) : (
          <PanelBody className="space-y-2">
            {[...pramaanEvents, ...events.filter((event) => !event.section.startsWith('pramaan'))].map(
              (event) => (
                <EventDetail key={event.id} event={event} />
              ),
            )}
          </PanelBody>
        )}
        <PanelNote>
          {pramaanEvents.length > 0
            ? 'The compliance events are listed first; the consensus events beneath them are what makes the record final.'
            : 'No compliance event was emitted here — this block carried consensus traffic only.'}
        </PanelNote>
      </Panel>

      {result.kind === 'block' && block.extrinsics.length > 0 && (
        <Panel>
          <PanelHead title="Extrinsics in this block" />
          <Table>
            <THead>
              <TR>
                <TH className="w-16">Index</TH>
                <TH>Call</TH>
                <TH className="w-44">Transaction reference</TH>
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
                    <Hash value={extrinsic.hash} />
                  </TD>
                </TR>
              ))}
            </TBody>
          </Table>
        </Panel>
      )}
    </div>
  );
}

function EventDetail({ event }: { event: ExplorerEvent }) {
  const fields = eventFields(event.data);
  const isPramaan = event.section.startsWith('pramaan');

  return (
    <div
      className={cn(
        'rounded-md border px-3 py-2.5',
        isPramaan ? 'border-accent-line bg-accent-tint' : 'border-line bg-shell',
      )}
    >
      <div className="flex flex-wrap items-center gap-2">
        <Chip tone={isPramaan ? 'accent' : 'neutral'}>{event.section}</Chip>
        <span className="font-mono text-2xs font-semibold text-ink">{event.method}</span>
      </div>
      {fields.length > 0 && <FactGrid className="mt-2" facts={fields.map((f) => ({ ...f, mono: true }))} />}
    </div>
  );
}
