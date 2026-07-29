'use client';

import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { ArrowRight, CircleCheckBig, Play, RotateCw } from 'lucide-react';
import Link from 'next/link';
import { useCallback, useEffect, useState } from 'react';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardFooter,
  CardHeader,
  ComplianceResult,
  ErrorState,
  FinalityPending,
  buttonClasses,
  type ComplianceStatus,
} from '@/components';
import { newScenario, WALKTHROUGH, type Scenario, type TriggerId } from './steps';

interface TriggerResponse {
  result?: ComplianceStatus;
  reason?: string;
  txRef?: string | null;
  blockNumber?: number | null;
  latencyMs?: number;
  error?: string;
  [key: string]: unknown;
}

interface RunOutcome {
  response: TriggerResponse;
  /** Wall-clock time the browser measured, for comparison with the chain's own figure. */
  roundTripMs: number;
}

type RunState =
  | { status: 'idle' }
  | { status: 'running' }
  | { status: 'done'; outcome: RunOutcome }
  | { status: 'failed'; message: string };

/**
 * Judge walkthrough mode (spec Part 9.7).
 *
 * One button advances the demo. Each step runs a real trigger point against the live
 * chain, waits for finality, shows the verdict the chain returned, and then explains in
 * plain language what just happened and which claim in the submission it evidences.
 *
 * Nothing is scripted: if the chain returns amber where the caption expects amber, that is
 * because the chain returned amber. A failed step shows the failure rather than skipping
 * to the next slide, because a walkthrough that cannot fail is not evidence of anything.
 */
export function WalkthroughClient() {
  // Generated after mount: the tender number is random per run, and generating it during
  // render would give the server and the client two different bid numbers.
  const [scenario, setScenario] = useState<Scenario | null>(null);
  useEffect(() => setScenario(newScenario()), []);

  const [current, setCurrent] = useState(0);
  const [runs, setRuns] = useState<Record<number, RunState>>({});
  const [followUps, setFollowUps] = useState<Record<number, RunState>>({});
  const reduceMotion = useReducedMotion();

  const setRun = useCallback((index: number, state: RunState) => {
    setRuns((previous) => ({ ...previous, [index]: state }));
  }, []);
  const setFollowUp = useCallback((index: number, state: RunState) => {
    setFollowUps((previous) => ({ ...previous, [index]: state }));
  }, []);

  const call = useCallback(
    async (
      endpoint: string,
      body: Record<string, unknown>,
      trigger: TriggerId,
    ): Promise<RunOutcome> => {
      const startedAt = performance.now();
      const response = await fetch(endpoint, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const payload = (await response.json()) as TriggerResponse;
      const roundTripMs = Math.round(performance.now() - startedAt);
      if (!response.ok) {
        throw new Error(payload.error ?? `The trigger point returned ${response.status}.`);
      }

      // Record the chain's own measured submission-to-finality time so the analytics
      // dashboard plots figures that were measured rather than assumed.
      if (typeof payload.latencyMs === 'number') {
        void fetch('/api/chain/latency', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            trigger,
            latencyMs: payload.latencyMs,
            blockNumber: payload.blockNumber ?? null,
            txRef: payload.txRef ?? null,
          }),
        }).catch(() => {
          // The measurement is a nice-to-have for the dashboard; losing it must never
          // interrupt the walkthrough in front of an audience.
        });
      }

      return { response: payload, roundTripMs };
    },
    [],
  );

  const runStep = useCallback(
    async (index: number) => {
      if (!scenario) return;
      const step = WALKTHROUGH[index];
      setRun(index, { status: 'running' });
      try {
        const body = await step.body(scenario);
        setRun(index, { status: 'done', outcome: await call(step.endpoint, body, step.id) });
      } catch (error) {
        setRun(index, {
          status: 'failed',
          message: error instanceof Error ? error.message : 'The request did not complete.',
        });
      }
    },
    [call, scenario, setRun],
  );

  const runFollowUp = useCallback(
    async (index: number) => {
      if (!scenario) return;
      const followUp = WALKTHROUGH[index].followUp;
      if (!followUp) return;
      setFollowUp(index, { status: 'running' });
      try {
        setFollowUp(index, {
          status: 'done',
          outcome: await call(followUp.endpoint, followUp.body(scenario), followUp.trigger),
        });
      } catch (error) {
        setFollowUp(index, {
          status: 'failed',
          message: error instanceof Error ? error.message : 'The request did not complete.',
        });
      }
    },
    [call, scenario, setFollowUp],
  );

  function restart() {
    setScenario(newScenario());
    setRuns({});
    setFollowUps({});
    setCurrent(0);
    if (typeof window !== 'undefined') window.scrollTo({ top: 0, behavior: 'smooth' });
  }

  const completed = WALKTHROUGH.filter((_, index) => runs[index]?.status === 'done').length;
  const finished = completed === WALKTHROUGH.length;

  if (!scenario) {
    return (
      <Card>
        <CardBody className="py-12 text-center text-sm text-ink-muted" role="status">
          Preparing the walkthrough scenario.
        </CardBody>
      </Card>
    );
  }

  return (
    <div className="space-y-6">
      <ProgressRail current={current} runs={runs} onSelect={setCurrent} />

      <div className="space-y-6">
        {WALKTHROUGH.map((step, index) => {
          const state = runs[index] ?? { status: 'idle' };
          const isCurrent = index === current;
          const isDone = state.status === 'done';
          if (!isCurrent && !isDone) return null;

          const followUpState = followUps[index] ?? { status: 'idle' };

          return (
            <Card key={step.id} className={isCurrent ? 'ring-1 ring-cerulea/30' : undefined}>
              <CardHeader
                title={
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="inline-flex size-6 shrink-0 items-center justify-center rounded-full bg-cerulea text-xs font-bold text-white">
                      {step.point}
                    </span>
                    {step.title}
                  </span>
                }
                description={step.intent}
                actions={
                  isDone ? (
                    <Badge
                      tone="teal"
                      icon={<CircleCheckBig className="size-3" aria-hidden="true" />}
                    >
                      Run
                    </Badge>
                  ) : (
                    <Badge tone="neutral">Trigger point {step.point} of 6</Badge>
                  )
                }
              />

              <CardBody className="space-y-5">
                <dl className="grid gap-x-6 gap-y-3 rounded-lg bg-surface-sunken px-4 py-3 sm:grid-cols-2 lg:grid-cols-3">
                  {step.facts(scenario).map((fact) => (
                    <div key={fact.label} className="min-w-0">
                      <dt className="text-xs tracking-wide text-ink-subtle uppercase">
                        {fact.label}
                      </dt>
                      <dd className="mt-0.5 text-sm font-medium break-words text-ink">
                        {fact.value}
                      </dd>
                    </div>
                  ))}
                </dl>

                {state.status === 'running' && (
                  <FinalityPending label={`Running trigger point ${step.point}`} />
                )}

                {state.status === 'failed' && (
                  <ErrorState
                    kind="unknown"
                    title="This step did not produce a verdict"
                    detail="The trigger point returned an error rather than a compliance decision. Nothing has been recorded on chain for this step, so it can be run again safely."
                    technicalDetail={state.message}
                    onRetry={() => void runStep(index)}
                    retryLabel="Run this step again"
                  />
                )}

                <AnimatePresence initial={false}>
                  {state.status === 'done' && (
                    <motion.div
                      initial={reduceMotion ? { opacity: 0 } : { opacity: 0, y: 8 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ duration: 0.2, ease: 'easeOut' }}
                      className="space-y-5"
                    >
                      <VerdictBlock step={step.title} outcome={state.outcome} />
                      <Caption
                        heading="What just happened"
                        body={step.caption}
                        claim={step.claim}
                      />

                      {step.followUp && (
                        <div className="rounded-lg border border-cerulea/25 bg-cerulea-light/40 px-4 py-4">
                          <p className="text-xs font-semibold tracking-wide text-cerulea-dark uppercase">
                            Then what
                          </p>
                          <p className="mt-1.5 text-sm leading-relaxed text-ink">
                            {step.followUp.premise}
                          </p>

                          {followUpState.status === 'idle' && (
                            <Button
                              className="mt-3"
                              size="sm"
                              variant="secondary"
                              onClick={() => void runFollowUp(index)}
                              leadingIcon={<Play className="size-4" aria-hidden="true" />}
                            >
                              {step.followUp.action}
                            </Button>
                          )}
                          {followUpState.status === 'running' && (
                            <FinalityPending
                              className="mt-3"
                              showSteps={false}
                              label={step.followUp.action}
                            />
                          )}
                          {followUpState.status === 'failed' && (
                            <ErrorState
                              className="mt-3"
                              kind="unknown"
                              technicalDetail={followUpState.message}
                              onRetry={() => void runFollowUp(index)}
                            />
                          )}
                          {followUpState.status === 'done' && (
                            <div className="mt-4 space-y-4">
                              <VerdictBlock
                                step={step.followUp.action}
                                outcome={followUpState.outcome}
                              />
                              <Caption heading="What that shows" body={step.followUp.caption} />
                            </div>
                          )}
                        </div>
                      )}
                    </motion.div>
                  )}
                </AnimatePresence>
              </CardBody>

              <CardFooter>
                <p className="text-xs text-ink-muted">
                  {isDone
                    ? `Trigger point ${step.point} complete.`
                    : `POST ${step.endpoint} — a real extrinsic, signed and finalized.`}
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  {state.status === 'idle' && (
                    <Button
                      onClick={() => void runStep(index)}
                      leadingIcon={<Play className="size-4" aria-hidden="true" />}
                    >
                      Run step {step.point}
                    </Button>
                  )}
                  {isDone && index < WALKTHROUGH.length - 1 && (
                    <Button
                      onClick={() => setCurrent(index + 1)}
                      variant={isCurrent ? 'primary' : 'secondary'}
                      trailingIcon={<ArrowRight className="size-4" aria-hidden="true" />}
                    >
                      Next step
                    </Button>
                  )}
                </div>
              </CardFooter>
            </Card>
          );
        })}
      </div>

      {finished && (
        <Card>
          <CardHeader
            title="All six trigger points have run on the live chain"
            description="Every verdict above came back only after its block was finalized, and every one of them is now a permanent, queryable record. Two places to check that claim rather than take it on trust."
          />
          <CardBody className="flex flex-wrap gap-3">
            <Link href="/dashboard" className={buttonClasses({ variant: 'primary' })}>
              See the measured latency for these six calls
            </Link>
            <Link href="/explorer" className={buttonClasses({ variant: 'secondary' })}>
              Find any of these decisions in the explorer
            </Link>
            <Button
              variant="ghost"
              onClick={restart}
              leadingIcon={<RotateCw className="size-4" aria-hidden="true" />}
            >
              Start again with a fresh bid number
            </Button>
          </CardBody>
        </Card>
      )}
    </div>
  );
}

function VerdictBlock({ step, outcome }: { step: string; outcome: RunOutcome }) {
  const { response, roundTripMs } = outcome;
  const status: ComplianceStatus =
    response.result === 'GREEN' || response.result === 'YELLOW' || response.result === 'RED'
      ? response.result
      : 'YELLOW';

  const records: { label: string; value: string; mono?: boolean }[] = [];
  if (typeof response.class === 'string') {
    records.push({ label: 'Classification', value: response.class, mono: true });
  }
  if (typeof response.newVersion === 'number') {
    records.push({ label: 'New rule version', value: `v${response.newVersion}`, mono: true });
  }
  if (typeof response.certificateId === 'string') {
    records.push({ label: 'Certificate id', value: response.certificateId, mono: true });
  }
  if (typeof response.matchedPrice === 'string' && response.matchedPrice) {
    records.push({ label: 'Matched price (paise)', value: response.matchedPrice, mono: true });
  }
  if (typeof response.status === 'string') {
    records.push({ label: 'Recorded state', value: response.status, mono: true });
  }
  records.push({ label: 'Browser round trip', value: `${roundTripMs} ms`, mono: true });

  return (
    <ComplianceResult
      status={status}
      trigger={step}
      reason={response.reason ?? 'The chain returned a verdict without a stated reason.'}
      txRef={response.txRef ?? undefined}
      blockNumber={response.blockNumber ?? undefined}
      latencyMs={response.latencyMs}
      records={records}
    />
  );
}

function Caption({
  heading,
  body,
  claim,
}: {
  heading: string;
  body: string;
  claim?: string;
}) {
  return (
    <div className="border-l-2 border-cerulea pl-4">
      <p className="text-xs font-semibold tracking-wide text-cerulea uppercase">{heading}</p>
      <p className="mt-1.5 max-w-3xl text-[0.9375rem] leading-relaxed text-ink">{body}</p>
      {claim && (
        <p className="mt-2.5 max-w-3xl text-sm text-ink-muted">
          <span className="font-medium text-ink">Demonstrates: </span>
          {claim}
        </p>
      )}
    </div>
  );
}

/** The six steps as a rail, so a judge always knows where in the sequence they are. */
function ProgressRail({
  current,
  runs,
  onSelect,
}: {
  current: number;
  runs: Record<number, RunState>;
  onSelect: (index: number) => void;
}) {
  return (
    <nav aria-label="Walkthrough steps">
      <ol className="grid gap-2 sm:grid-cols-3 lg:grid-cols-6">
        {WALKTHROUGH.map((step, index) => {
          const state = runs[index]?.status ?? 'idle';
          const done = state === 'done';
          const isCurrent = index === current;
          const reachable = done || index <= current;

          return (
            <li key={step.id}>
              <button
                type="button"
                onClick={() => reachable && onSelect(index)}
                disabled={!reachable}
                aria-current={isCurrent ? 'step' : undefined}
                className={
                  'w-full rounded-lg border px-3 py-2.5 text-left transition-colors duration-150 ease-out ' +
                  'focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea ' +
                  'disabled:cursor-not-allowed disabled:opacity-55 ' +
                  (isCurrent
                    ? 'border-cerulea bg-cerulea-light'
                    : done
                      ? 'border-border bg-surface hover:border-cerulea/40'
                      : 'border-dashed border-border bg-surface')
                }
              >
                <span className="flex items-center gap-2">
                  <span
                    className={
                      'inline-flex size-5 shrink-0 items-center justify-center rounded-full text-[0.625rem] font-bold ' +
                      (done
                        ? 'bg-teal text-white'
                        : isCurrent
                          ? 'bg-cerulea text-white'
                          : 'bg-surface-sunken text-ink-subtle')
                    }
                  >
                    {done ? <CircleCheckBig className="size-3" aria-hidden="true" /> : step.point}
                  </span>
                  <span className="truncate text-xs font-medium text-ink">
                    {STEP_SHORT_LABELS[step.id]}
                  </span>
                </span>
              </button>
            </li>
          );
        })}
      </ol>
    </nav>
  );
}

const STEP_SHORT_LABELS: Record<TriggerId, string> = {
  'bid-submission': 'Bid submission',
  'bid-evaluation': 'Bid evaluation',
  'preference-calculation': 'Preference',
  'ca-certification': 'Certification',
  debarment: 'Debarment',
  'rule-update': 'Rule update',
};
