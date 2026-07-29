'use client';

import { ShieldCheck } from 'lucide-react';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  DataRow,
  EmptyState,
  ErrorState,
  Stat,
  Table,
  TBody,
  TCaption,
  TD,
  TH,
  THead,
  TR,
  TxRef,
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
      <Card>
        <CardHeader title="Validators" description="Reading the DCF validator set." />
        <div className="space-y-2 px-5 py-4" role="status" aria-live="polite">
          <span className="sr-only">Reading validator trust scores.</span>
          {[0, 1, 2].map((key) => (
            <div key={key} className="h-10 rounded bg-surface-sunken" />
          ))}
        </div>
      </Card>
    );
  }

  if (!data) {
    return (
      <ErrorState
        kind="chain-unreachable"
        detail="The validator set could not be read from pallet-cerulea-pos and pallet-cerulea-dcf."
        technicalDetail={error ?? undefined}
        onRetry={refresh}
      />
    );
  }

  const { validators, consensus, window } = data;

  return (
    <div className="space-y-5">
      <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <Stat label="Validators" value={validators.length} hint="Registered in pallet-cerulea-pos" />
        <Stat label="Current epoch" value={consensus.epoch.toLocaleString('en-IN')} mono />
        <Stat
          label="PoS weight"
          value={formatBps(consensus.posWeightBps)}
          hint="Share of the combined trust score from stake"
        />
        <Stat
          label="PoI weight"
          value={formatBps(consensus.poiWeightBps)}
          hint="Share from Proof-of-Inference"
        />
      </div>

      <Card>
        <CardHeader
          title="DCF validator set"
          description="Trust scores are read live from pallet-cerulea-dcf; the PoS score and stake come from pallet-cerulea-pos, and the inference count from pallet-cerulea-poi."
        />
        {validators.length === 0 ? (
          <CardBody>
            <EmptyState
              icon={<ShieldCheck className="size-5" aria-hidden="true" />}
              title="No validators registered"
              description="This view lists every validator in pallet-cerulea-pos with its DCF trust score. It fills in as soon as a validator registers on the network this console is connected to."
            />
          </CardBody>
        ) : (
          <Table containerClassName="rounded-b-card">
            <THead>
              <TR>
                <TH>Validator</TH>
                <TH>DCF trust score</TH>
                <TH>PoS score</TH>
                <TH className="text-right">PoI submissions</TH>
                <TH className="text-right">Blocks authored</TH>
                <TH>Status</TH>
              </TR>
            </THead>
            <TBody>
              {validators.map((validator) => (
                <TR key={validator.address} className="hover:bg-surface-sunken">
                  <TD>
                    <TxRef value={validator.address} label="Validator address" head={10} tail={8} />
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
                  <TD className="text-right font-mono text-[0.8125rem] text-ink tabular-nums">
                    {validator.inferenceCount.toLocaleString('en-IN')}
                  </TD>
                  <TD className="text-right font-mono text-[0.8125rem] text-ink tabular-nums">
                    {validator.blocksAuthoredInWindow.toLocaleString('en-IN')}
                  </TD>
                  <TD>
                    <Badge tone={validator.registered ? 'teal' : 'neutral'} size="sm">
                      {validator.registered ? 'Registered' : 'Authoring only'}
                    </Badge>
                  </TD>
                </TR>
              ))}
            </TBody>
            <TCaption>
              Blocks authored is counted over the {window.blocksIndexed.toLocaleString('en-IN')}{' '}
              blocks this explorer currently holds ({formatBlockNumber(window.from)} to{' '}
              {formatBlockNumber(window.to)}), not over the life of the chain.
            </TCaption>
          </Table>
        )}
      </Card>

      <Card>
        <CardHeader
          title="How the trust score is composed"
          description="The Dynamic Consensus Framework blends two independent signals rather than ranking validators on stake alone."
        />
        <CardBody>
          <dl>
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
          </dl>
        </CardBody>
      </Card>
    </div>
  );
}

/**
 * A score as a number plus a proportional bar. The bar is brand blue, never a status
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
    <div className="min-w-[9rem]">
      <span className="font-mono text-[0.8125rem] text-ink tabular-nums">{display}</span>
      <div
        className="mt-1.5 h-1.5 w-full overflow-hidden rounded-full bg-cerulea-light"
        role="img"
        aria-label={`${display} of a possible ${max.toLocaleString('en-IN')}`}
      >
        <div
          className="h-full rounded-full bg-cerulea transition-[width] duration-200 ease-out"
          style={{ width: `${fraction * 100}%` }}
        />
      </div>
    </div>
  );
}
