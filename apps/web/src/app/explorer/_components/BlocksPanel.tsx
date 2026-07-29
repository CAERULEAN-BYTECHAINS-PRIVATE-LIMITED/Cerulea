'use client';

import { Boxes, CircleCheckBig, CircleDashed } from 'lucide-react';
import { useEffect, useState } from 'react';
import {
  Badge,
  Card,
  CardHeader,
  ErrorState,
  Stat,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
  TxRef,
} from '@/components';
import { formatAge, formatBlockNumber, formatClock } from './format';
import type { BlocksResponse } from './types';
import { usePolledResource } from './usePolledResource';

const PRAMAAN_PREFIX = 'pramaan';

export function BlocksPanel({ onOpenBlock }: { onOpenBlock: (blockNumber: number) => void }) {
  const { data, loading, error, refresh } = usePolledResource<BlocksResponse>(
    '/api/chain/blocks?limit=25',
    2_000,
  );

  // A relative age has to re-render on its own clock, not only when a poll lands.
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setNow(Date.now()), 1_000);
    return () => clearInterval(timer);
  }, []);

  if (loading && !data) return <BlocksSkeleton />;

  if (!data) {
    return (
      <ErrorState
        kind="chain-unreachable"
        detail="The explorer could not read the chain head. Blocks will reappear as soon as a validator responds."
        technicalDetail={error ?? undefined}
        onRetry={refresh}
      />
    );
  }

  const blocks = data.blocks;
  const head = blocks[0];
  const spanSeconds =
    blocks.length > 1 ? (blocks[0].timestamp - blocks[blocks.length - 1].timestamp) / 1000 : 0;
  const averageBlockMs =
    blocks.length > 1 ? Math.round((spanSeconds * 1000) / (blocks.length - 1)) : null;

  return (
    <div className="space-y-5">
      {error && (
        <p className="rounded-card border border-border bg-surface-sunken px-4 py-2.5 text-sm text-ink-muted">
          Showing the last blocks read successfully. The most recent refresh did not
          complete: {error}
        </p>
      )}

      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Stat
          label="Best block"
          value={
            data.bestBlock
              ? formatBlockNumber(data.bestBlock)
              : head
                ? formatBlockNumber(head.number)
                : '—'
          }
          mono
        />
        <Stat
          label="Finalized block"
          value={formatBlockNumber(data.finalizedBlock)}
          hint="Decisions are only returned at or below this height"
          mono
        />
        <Stat
          label="Average block time"
          value={averageBlockMs === null ? '—' : `${averageBlockMs} ms`}
          hint={`Measured across the last ${blocks.length} blocks`}
        />
        <Stat
          label="Blocks indexed"
          value={data.window.blocksIndexed.toLocaleString('en-IN')}
          hint={`${formatBlockNumber(data.window.from)} to ${formatBlockNumber(data.window.to)}`}
        />
      </div>

      <Card>
        <CardHeader
          title="Recent blocks"
          description="Live, refreshed every two seconds. A block is marked finalized only once the node reports a finalized head at or past it."
        />
        <Table containerClassName="rounded-b-card">
          <THead>
            <TR>
              <TH>Block</TH>
              <TH>Age</TH>
              <TH>Author</TH>
              <TH className="text-right">Extrinsics</TH>
              <TH className="text-right">Events</TH>
              <TH>Contents</TH>
              <TH>Finality</TH>
            </TR>
          </THead>
          <TBody>
            {blocks.map((block) => {
              const pramaanCalls = block.extrinsics.filter((extrinsic) =>
                extrinsic.section.startsWith(PRAMAAN_PREFIX),
              );
              return (
                <TR key={block.hash} className="hover:bg-surface-sunken">
                  <TD>
                    <button
                      type="button"
                      onClick={() => onOpenBlock(block.number)}
                      className="rounded font-mono text-[0.8125rem] font-semibold text-cerulea transition-colors duration-150 hover:text-cerulea-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea"
                    >
                      {formatBlockNumber(block.number)}
                    </button>
                    <span className="mt-0.5 block text-xs text-ink-subtle">
                      {formatClock(block.timestamp)}
                    </span>
                  </TD>
                  <TD className="text-sm text-ink-muted whitespace-nowrap">
                    {formatAge(block.timestamp, now)}
                  </TD>
                  <TD>
                    {block.author ? (
                      <TxRef value={block.author} label="Validator address" head={8} tail={6} />
                    ) : (
                      <span className="text-sm text-ink-subtle">Not in digest</span>
                    )}
                  </TD>
                  <TD className="text-right font-mono text-[0.8125rem] text-ink tabular-nums">
                    {block.extrinsicCount}
                  </TD>
                  <TD className="text-right font-mono text-[0.8125rem] text-ink tabular-nums">
                    {block.eventsAvailable === false ? (
                      <span
                        className="text-ink-subtle"
                        title="The node had already pruned this block's state, so its events could not be decoded. The block itself is unchanged."
                      >
                        pruned
                      </span>
                    ) : (
                      block.eventCount
                    )}
                  </TD>
                  <TD>
                    {pramaanCalls.length > 0 ? (
                      <div className="flex flex-wrap gap-1.5">
                        {pramaanCalls.map((call) => (
                          <Badge key={call.hash} tone="brand" size="sm">
                            {call.section.replace(PRAMAAN_PREFIX, '')}.{call.method}
                          </Badge>
                        ))}
                      </div>
                    ) : (
                      <span className="text-sm text-ink-subtle">Consensus only</span>
                    )}
                  </TD>
                  <TD>
                    {block.finalized ? (
                      <Badge
                        tone="teal"
                        size="sm"
                        icon={<CircleCheckBig className="size-3" aria-hidden="true" />}
                      >
                        Finalized
                      </Badge>
                    ) : (
                      <Badge
                        tone="neutral"
                        size="sm"
                        icon={<CircleDashed className="size-3" aria-hidden="true" />}
                      >
                        Included
                      </Badge>
                    )}
                  </TD>
                </TR>
              );
            })}
          </TBody>
        </Table>
      </Card>
    </div>
  );
}

function BlocksSkeleton() {
  return (
    <div className="space-y-5" role="status" aria-live="polite">
      <span className="sr-only">Reading the chain head.</span>
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        {[0, 1, 2, 3].map((key) => (
          <div key={key} className="rounded-card border border-border bg-surface px-5 py-4">
            <div className="h-3 w-24 rounded bg-surface-sunken" />
            <div className="mt-3 h-6 w-20 rounded bg-surface-sunken" />
          </div>
        ))}
      </div>
      <Card>
        <CardHeader
          title="Recent blocks"
          description="Reading the last blocks from the node."
          actions={<Boxes className="size-4 text-ink-subtle" aria-hidden="true" />}
        />
        <div className="space-y-2 px-5 py-4">
          {Array.from({ length: 8 }, (_, index) => (
            <div key={index} className="h-9 rounded bg-surface-sunken" />
          ))}
        </div>
      </Card>
    </div>
  );
}
