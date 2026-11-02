'use client';

import {
  Chip,
  DataList,
  DataRow,
  Empty,
  Figure,
  FigureRow,
  Hash,
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
} from '@/components';
import { formatBps } from '@/lib/units';
import { formatBlockNumber } from './format';
import type { ValidatorsResponse } from './types';
import { usePolledResource } from './usePolledResource';

/** `pallet-cerulea-dcf` stores the combined trust score on a 0–10,000 scale. */
const TRUST_SCORE_MAX = 10_000;
/** `pallet-cerulea-pos` stores its own score on a 0–100 scale. */
const POS_SCORE_MAX = 100;

export function ValidatorsPanel() {
  const { data, loading, error, refresh } = usePolledResource<ValidatorsResponse>(
    '/api/chain/validators',
    5_000,
  );

  if (loading && !data) {
    return (
      <Panel>
        <PanelHead title="DCF validator set" />
        <SkeletonRows rows={3} label="Reading validator trust scores." />
      </Panel>
    );
  }

  if (!data) {
    return (
      <Notice
        kind="chain-unreachable"
        detail="The validator set could not be read from pallet-cerulea-pos and pallet-cerulea-dcf."
        technicalDetail={error ?? undefined}
        onRetry={refresh}
      />
    );
  }

  const { validators, consensus, window } = data;

  return (
    <div className="space-y-4">
      <FigureRow>
        <Figure
          label="Validators"
          value={validators.length}
          note="Registered in pallet-cerulea-pos."
        />
        <Figure label="Current epoch" value={consensus.epoch.toLocaleString('en-IN')} mono />
        <Figure
          label="PoS weight"
          value={formatBps(consensus.posWeightBps)}
          note="Share of the combined trust score from stake."
        />
        <Figure
          label="PoI weight"
          value={formatBps(consensus.poiWeightBps)}
          note="Share from Proof-of-Inference."
        />
      </FigureRow>

      <Panel>
        <PanelHead title="DCF validator set" meta={<Chip>{validators.length} registered</Chip>} />
        {validators.length === 0 ? (
          <PanelBody>
            <Empty
              title="No validators registered"
              source="This view lists every validator in pallet-cerulea-pos with its DCF trust score. It fills in as soon as a validator registers on the network this console is connected to."
            />
          </PanelBody>
        ) : (
          <Table>
            <THead>
              <TR>
                <TH>Validator</TH>
                <TH className="w-52">DCF trust score</TH>
                <TH className="w-52">PoS score</TH>
                <TH numeric className="w-32">PoI submissions</TH>
                <TH numeric className="w-32">Blocks authored</TH>
                <TH className="w-32">Status</TH>
              </TR>
            </THead>
            <TBody>
              {validators.map((validator) => (
                <TR key={validator.address} className="hover:bg-shell">
                  <TD>
                    <Hash value={validator.address} label="Validator address" head={10} tail={8} />
                  </TD>
                  <TD>
                    <ScoreMeter
                      value={validator.trustScore}
                      max={TRUST_SCORE_MAX}
                      display={
                        validator.trustScore === null
                          ? '—'
                          : validator.trustScore.toLocaleString('en-IN')
                      }
                    />
                  </TD>
                  <TD>
                    <ScoreMeter
                      value={validator.posScore}
                      max={POS_SCORE_MAX}
                      display={validator.posScore === null ? '—' : `${validator.posScore} / 100`}
                    />
                  </TD>
                  <TD numeric>{validator.inferenceCount.toLocaleString('en-IN')}</TD>
                  <TD numeric>{validator.blocksAuthoredInWindow.toLocaleString('en-IN')}</TD>
                  <TD>
                    <Chip tone={validator.registered ? 'accent' : 'neutral'}>
                      {validator.registered ? 'Registered' : 'Authoring only'}
                    </Chip>
                  </TD>
                </TR>
              ))}
            </TBody>
          </Table>
        )}
        <PanelNote>
          Trust scores are read live from pallet-cerulea-dcf; the PoS score and stake from
          pallet-cerulea-pos, and the inference count from pallet-cerulea-poi. Blocks authored is
          counted over the {window.blocksIndexed.toLocaleString('en-IN')} blocks this explorer
          currently holds ({formatBlockNumber(window.from)} to {formatBlockNumber(window.to)}), not
          over the life of the chain.
        </PanelNote>
      </Panel>

      <Panel>
        <PanelHead title="How the trust score is composed" />
        <PanelBody>
          <DataList columns={2}>
            <DataRow
              label="Proof of Stake contribution"
              value={`${formatBps(consensus.posWeightBps)} of the combined score`}
            />
            <DataRow
              label="Proof of Inference contribution"
              value={`${formatBps(consensus.poiWeightBps)} of the combined score`}
            />
            <DataRow
              label="Trust score scale"
              value={`0 to ${TRUST_SCORE_MAX.toLocaleString('en-IN')}`}
              mono
            />
            <DataRow label="Epoch" value={consensus.epoch.toLocaleString('en-IN')} mono />
          </DataList>
        </PanelBody>
        <PanelNote>
          The Dynamic Consensus Framework blends two independent signals rather than ranking
          validators on stake alone.
        </PanelNote>
      </Panel>
    </div>
  );
}

/**
 * A score as a number plus a proportional bar. The bar is accent blue, never a status
 * colour: a validator's trust score is not a compliance verdict, and a green bar here
 * would teach a judge the wrong association.
 */
function ScoreMeter({
  value,
  max,
  display,
}: {
  value: number | null;
  max: number;
  display: string;
}) {
  const fraction = value === null ? 0 : Math.max(0, Math.min(1, value / max));
  return (
    <div className="min-w-[8rem]">
      <span className="font-mono text-2xs text-ink tabular-nums">{display}</span>
      <div
        className="mt-1 h-1 w-full overflow-hidden rounded-sm bg-shell-2"
        role="img"
        aria-label={`${display} of a possible ${max.toLocaleString('en-IN')}`}
      >
        <div
          className="h-full bg-accent transition-[width] duration-200 ease-out"
          style={{ width: `${fraction * 100}%` }}
        />
      </div>
    </div>
  );
}
