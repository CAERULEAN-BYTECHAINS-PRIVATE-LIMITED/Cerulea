'use client';

import { useEffect, useState } from 'react';
import {
  BlockRef,
  Chip,
  Figure,
  FigureRow,
  Hash,
  Nil,
  Notice,
  Panel,
  PanelHead,
  PanelNote,
  SkeletonRows,
  StaleBanner,
  Table,
  TBody,
  TD,
  TH,
  THead,
  TR,
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

  if (loading && !data) {
    return (
      <Panel>
        <PanelHead title="Recent blocks" />
        <SkeletonRows rows={8} label="Reading the chain head." />
      </Panel>
    );
  }

  if (!data) {
    return (
      <Notice
        kind="chain-unreachable"
        detail="The explorer could not read the chain head. Blocks reappear as soon as a validator responds."
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
    <div className="space-y-4">
      {error && <StaleBanner message={`The most recent refresh did not complete: ${error}`} />}

      <FigureRow>
        <Figure
          label="Best block"
          mono
          value={
            data.bestBlock
              ? formatBlockNumber(data.bestBlock)
              : head
                ? formatBlockNumber(head.number)
                : '—'
          }
        />
        <Figure
          label="Finalized block"
          mono
          value={formatBlockNumber(data.finalizedBlock)}
          note="Decisions are only returned at or below this height."
        />
        <Figure
          label="Average block time"
          value={averageBlockMs === null ? '—' : `${averageBlockMs} ms`}
          note={`Measured across the last ${blocks.length} blocks.`}
        />
        <Figure
          label="Blocks indexed"
          value={data.window.blocksIndexed.toLocaleString('en-IN')}
          note={`${formatBlockNumber(data.window.from)} to ${formatBlockNumber(data.window.to)}`}
        />
      </FigureRow>

      <Panel>
        <PanelHead title="Recent blocks" meta={<Chip>Refreshed every 2 s</Chip>} />
        <Table>
          <THead>
            <TR>
              <TH className="w-32">Block</TH>
              <TH className="w-20">Age</TH>
              <TH className="w-40">Author</TH>
              <TH numeric className="w-24">Extrinsics</TH>
              <TH numeric className="w-20">Events</TH>
              <TH>Contents</TH>
              <TH className="w-28">Finality</TH>
            </TR>
          </THead>
          <TBody>
            {blocks.map((block) => {
              const pramaanCalls = block.extrinsics.filter((extrinsic) =>
                extrinsic.section.startsWith(PRAMAAN_PREFIX),
              );
              return (
                <TR key={block.hash} className="hover:bg-shell">
                  <TD>
                    <button
                      type="button"
                      onClick={() => onOpenBlock(block.number)}
                      className="rounded-sm font-mono text-2xs font-semibold text-accent transition-colors duration-150 hover:text-accent-dark hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                    >
                      {formatBlockNumber(block.number)}
                    </button>
                    <span className="mt-0.5 block text-2xs text-ink-subtle">
                      {formatClock(block.timestamp)}
                    </span>
                  </TD>
                  <TD className="whitespace-nowrap text-ink-muted">
                    {formatAge(block.timestamp, now)}
                  </TD>
                  <TD>
                    {block.author ? (
                      <Hash value={block.author} label="Validator address" />
                    ) : (
                      <Nil label="Not in digest" />
                    )}
                  </TD>
                  <TD numeric>{block.extrinsicCount}</TD>
                  <TD numeric>
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
                      <span className="flex flex-wrap gap-1">
                        {pramaanCalls.map((call) => (
                          <Chip key={call.hash} tone="accent">
                            {call.section.replace(PRAMAAN_PREFIX, '')}.{call.method}
                          </Chip>
                        ))}
                      </span>
                    ) : (
                      <span className="text-2xs text-ink-subtle">Consensus only</span>
                    )}
                  </TD>
                  <TD>
                    {block.finalized ? (
                      <Chip tone="accent">Finalized</Chip>
                    ) : (
                      <Chip>Included</Chip>
                    )}
                  </TD>
                </TR>
              );
            })}
          </TBody>
        </Table>
        <PanelNote>
          A block is marked finalized only once the node reports a finalized head at or past it.
          The head is currently <BlockRef value={data.finalizedBlock} />.
        </PanelNote>
      </Panel>
    </div>
  );
}
