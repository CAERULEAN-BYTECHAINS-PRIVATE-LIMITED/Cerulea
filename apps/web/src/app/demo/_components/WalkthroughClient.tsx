'use client';

import { AnimatePresence, motion, useReducedMotion } from 'framer-motion';
import { ArrowRight, Check, Play, RotateCw } from 'lucide-react';
import Link from 'next/link';
import { useCallback, useState, useSyncExternalStore } from 'react';
import {
  Awaiting,
  Button,
  Chip,
  FactGrid,
  Notice,
  Panel,
  PanelBody,
  PanelFoot,
  PanelHead,
  Verdict,
  buttonClasses,
  cn,
  type ComplianceStatus,
} from '@/components';
import { formatPaise } from '@/lib/units';
import { scenarioStore, WALKTHROUGH, type Scenario, type StepId, type TriggerId } from './steps';

/** Trigger point 1 — the classification whose block the provenance step re-reads. */
const BID_SUBMISSION_INDEX = WALKTHROUGH.findIndex((step) => step.id === 'bid-submission');

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
  // The tender number is random per run, so it is generated on the client only — see the
  // note on `scenarioStore`. Null on the server render, a scenario from first paint on.
  const scenario: Scenario | null = useSyncExternalStore(
    scenarioStore.subscribe,
    scenarioStore.getSnapshot,
    scenarioStore.getServerSnapshot,
  );

  const [current, setCurrent] = useState(0);
  const [runs, setRuns] = useState<Record<number, RunState>>({});
  const [followUps, setFollowUps] = useState<Record<number, RunState>>({});
  /** True while the reset is lifting the debarment step 5 wrote — see `restart` below. */
  const [resetting, setResetting] = useState(false);
  /** Set only when that lift failed, so the presenter is told the chain is still dirty. */
  const [resetNote, setResetNote] = useState<string | null>(null);
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
      //
      // Only calls that actually reached a block are recorded. Two of the trigger points
      // can answer from current state without writing anything — an evaluation blocked by
      // an existing debarment, a certification that needs an auditor — and those return in
      // single-digit milliseconds. Real, but not submission-to-finality times, and folding
      // them into a chart that claims to measure finality would flatter the number.
      if (typeof payload.latencyMs === 'number' && typeof payload.blockNumber === 'number') {
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
        // The provenance panel writes nothing: it re-reads the classification recorded at
        // trigger point 1 from that decision's own block, so it needs step 1's block
        // number and calls its read endpoint directly rather than through `call`, which is
        // built for trigger points that submit an extrinsic and post a latency sample.
        if (step.kind === 'provenance') {
          const source = runs[BID_SUBMISSION_INDEX];
          const blockNumber =
            source?.status === 'done' ? source.outcome.response.blockNumber ?? null : null;
          if (blockNumber == null) {
            throw new Error(
              'Run trigger point 1 first — its finalized block is the one this step re-reads.',
            );
          }
          const startedAt = performance.now();
          const response = await fetch(step.endpoint, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ ...(await step.body(scenario)), blockNumber }),
          });
          const payload = (await response.json()) as TriggerResponse;
          const roundTripMs = Math.round(performance.now() - startedAt);
          if (!response.ok) {
            throw new Error(payload.error ?? `The verification returned ${response.status}.`);
          }
          setRun(index, { status: 'done', outcome: { response: payload, roundTripMs } });
          return;
        }

        const body = await step.body(scenario);
        setRun(index, {
          status: 'done',
          outcome: await call(step.endpoint, body, step.id as TriggerId),
        });
      } catch (error) {
        setRun(index, {
          status: 'failed',
          message: error instanceof Error ? error.message : 'The request did not complete.',
        });
      }
    },
    [call, scenario, setRun, runs],
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

  /**
   * Restart the walkthrough — and put the chain back the way this run found it.
   *
   * Resetting used to mint a new bid number and nothing else, which made the demo
   * cumulative rather than repeatable: step 5 writes a real debarment, and nothing ever
   * lifted it. Rehearsing five times before a judging session left five active debarments
   * standing, and the analytics dashboard — which correctly reads the chain — reported
   * every one of them. Six rehearsals, six debarred vendors, none of them part of the story
   * being told.
   *
   * So the reset lifts the debarment it raised before handing out a fresh scenario. Three
   * details make that work:
   *
   *   - the lift is attempted only if step 5 actually completed, since there is nothing to
   *     lift otherwise and `lift_debarment` would fail with `NoSuchDebarment`;
   *   - the scenario is captured BEFORE `scenarioStore.reset()`, because the debarred
   *     account is derived per run and the new scenario cannot name who was debarred;
   *   - a failed lift never blocks the restart. The presenter still gets a clean
   *     walkthrough, and the note below says plainly that the chain was not fully restored
   *     rather than leaving them to discover it on the dashboard mid-demo.
   */
  const restart = useCallback(async () => {
    if (resetting) return;
    const debarmentIndex = WALKTHROUGH.findIndex((step) => step.id === 'debarment');
    const raisedDebarment = debarmentIndex >= 0 && runs[debarmentIndex]?.status === 'done';
    const finishing = scenario;

    setResetNote(null);
    if (raisedDebarment && finishing) {
      setResetting(true);
      try {
        const response = await fetch('/api/trigger/debarment', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({
            vendor: finishing.rival,
            ministry: finishing.otherMinistry,
            action: 'lift',
          }),
        });
        const payload = (await response.json()) as TriggerResponse;
        if (!response.ok) {
          throw new Error(payload.error ?? `The trigger point returned ${response.status}.`);
        }
      } catch (error) {
        setResetNote(
          `The debarment step 5 recorded against ${finishing.rivalName} could not be lifted, so it ` +
            `is still in force and the dashboard will keep counting it as active. ` +
            `${error instanceof Error ? error.message : 'The request did not complete.'} ` +
            `Lift it from the ministry administrator console before the next run.`,
        );
      } finally {
        setResetting(false);
      }
    }

    scenarioStore.reset();
    setRuns({});
    setFollowUps({});
    setCurrent(0);
    if (typeof window !== 'undefined') window.scrollTo({ top: 0, behavior: 'smooth' });
  }, [resetting, runs, scenario]);

  const completed = WALKTHROUGH.filter((_, index) => runs[index]?.status === 'done').length;
  const finished = completed === WALKTHROUGH.length;

  if (!scenario) {
    return (
      <Panel>
        <PanelBody className="py-8 text-center text-sm text-ink-muted" role="status">
          Preparing the walkthrough scenario.
        </PanelBody>
      </Panel>
    );
  }

  return (
    <div className="space-y-4">
      {resetNote && (
        <Notice
          kind="unknown"
          title="The chain was not fully restored"
          detail={resetNote}
        />
      )}

      <ProgressRail current={current} runs={runs} onSelect={setCurrent} />

      <div className="space-y-4">
        {WALKTHROUGH.map((step, index) => {
          const state = runs[index] ?? { status: 'idle' };
          const isCurrent = index === current;
          const isDone = state.status === 'done';
          if (!isCurrent && !isDone) return null;

          const followUpState = followUps[index] ?? { status: 'idle' };

          return (
            <Panel key={step.id} className={isCurrent ? 'border-accent-line ring-1 ring-accent-line' : undefined}>
              <PanelHead
                title={
                  <span className="flex flex-wrap items-center gap-2">
                    <span className="inline-flex size-4 shrink-0 items-center justify-center rounded-sm bg-accent font-mono text-2xs font-bold text-white">
                      {step.point}
                    </span>
                    {step.title}
                  </span>
                }
                meta={
                  step.kind === 'provenance' ? (
                    <Chip tone={isDone ? 'accent' : undefined}>
                      {isDone ? 'Verified' : 'Verification'}
                    </Chip>
                  ) : isDone ? (
                    <Chip tone="accent">Run</Chip>
                  ) : (
                    <Chip>Trigger point {step.point} of 6</Chip>
                  )
                }
              />

              <PanelBody className="space-y-4">
                <FactGrid
                  className="rounded-md bg-shell px-3 py-2.5"
                  facts={step.facts(scenario)}
                />

                {state.status === 'running' && (
                  <Awaiting
                    label={
                      step.kind === 'provenance'
                        ? 'Re-deriving the step 1 verdict from its finalized block'
                        : `Running trigger point ${step.point}`
                    }
                  />
                )}

                {state.status === 'failed' && (
                  <Notice
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
                      className="space-y-4"
                    >
                      {step.kind === 'provenance' ? (
                        <ProvenanceBlock outcome={state.outcome} />
                      ) : (
                        <VerdictBlock step={step.title} outcome={state.outcome} />
                      )}
                      <Caption
                        heading={step.kind === 'provenance' ? 'What this proves' : 'What just happened'}
                        body={step.caption}
                        claim={step.claim}
                      />

                      {step.followUp && (
                        <div className="rounded-md border border-accent-line bg-accent-tint px-3 py-3">
                          <p className="text-2xs font-semibold tracking-wide text-accent-dark uppercase">
                            Then what
                          </p>
                          <p className="mt-1 max-w-[75ch] text-sm text-ink">
                            {step.followUp.premise}
                          </p>

                          {followUpState.status === 'idle' && (
                            <Button
                              className="mt-2.5"
                              onClick={() => void runFollowUp(index)}
                              icon={<Play className="size-3.5" aria-hidden="true" />}
                            >
                              {step.followUp.action}
                            </Button>
                          )}
                          {followUpState.status === 'running' && (
                            <Awaiting
                              className="mt-2.5"
                              steps={false}
                              label={step.followUp.action}
                            />
                          )}
                          {followUpState.status === 'failed' && (
                            <Notice
                              className="mt-2.5"
                              kind="unknown"
                              technicalDetail={followUpState.message}
                              onRetry={() => void runFollowUp(index)}
                            />
                          )}
                          {followUpState.status === 'done' && (
                            <div className="mt-3 space-y-3">
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
              </PanelBody>

              <PanelFoot>
                <p className="font-mono text-2xs text-ink-muted">
                  {step.kind === 'provenance'
                    ? isDone
                      ? 'Verification complete — historical state re-read, nothing written.'
                      : `POST ${step.endpoint} — a read of historical state; nothing is written.`
                    : isDone
                      ? `Trigger point ${step.point} complete.`
                      : `POST ${step.endpoint} — a real extrinsic, signed and finalized.`}
                </p>
                <div className="flex flex-wrap items-center gap-2">
                  {state.status === 'idle' && (
                    <Button
                      variant="primary"
                      onClick={() => void runStep(index)}
                      icon={<Play className="size-3.5" aria-hidden="true" />}
                    >
                      {step.kind === 'provenance'
                        ? 'Re-derive the step 1 verdict'
                        : `Run step ${step.point}`}
                    </Button>
                  )}
                  {/* Only the panel the presenter is on offers the advance. Completed panels
                      stay on screen as the record of what was shown, but a second live
                      "Next step" further up the page is an invitation to lose your place. */}
                  {isDone && isCurrent && index < WALKTHROUGH.length - 1 && (
                    <Button
                      variant="primary"
                      onClick={() => setCurrent(index + 1)}
                      iconAfter={<ArrowRight className="size-3.5" aria-hidden="true" />}
                    >
                      Next step — {WALKTHROUGH[index + 1].title.toLowerCase()}
                    </Button>
                  )}
                </div>
              </PanelFoot>
            </Panel>
          );
        })}
      </div>

      {finished && (
        <Panel>
          <PanelHead title="All six trigger points have run, and the step 1 verdict has been re-derived from its block" />
          <PanelBody className="flex flex-wrap gap-2">
            <Link href="/dashboard" className={buttonClasses({ variant: 'primary' })}>
              See the measured latency for these six calls
            </Link>
            <Link href="/explorer" className={buttonClasses({ variant: 'default' })}>
              Find any of these decisions in the explorer
            </Link>
            <Button
              variant="quiet"
              onClick={() => void restart()}
              loading={resetting}
              loadingLabel="Lifting the debarment this walkthrough recorded"
              icon={<RotateCw className="size-3.5" aria-hidden="true" />}
            >
              {resetting
                ? 'Lifting the debarment and starting again'
                : 'Start again — lifts the debarment first'}
            </Button>
          </PanelBody>
        </Panel>
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
    records.push({
      label: 'Matched price',
      value: formatPaise(response.matchedPrice),
      mono: true,
    });
  }
  if (typeof response.debarredBy === 'string' && response.debarredBy) {
    records.push({ label: 'Debarred by', value: response.debarredBy, mono: true });
  }
  if (typeof response.status === 'string') {
    records.push({ label: 'Recorded state', value: response.status, mono: true });
  }
  // The per-bid decision path is the reasoning behind a preference outcome, so it belongs
  // in the record rather than only in the prose that describes it.
  if (Array.isArray(response.outcomes)) {
    (response.outcomes as { vendor?: string; decisionPath?: string; qualifies?: boolean }[]).forEach(
      (outcome, index) => {
        records.push({
          label: `Bid ${index + 1} pathway`,
          value: `${outcome.decisionPath ?? '—'} · ${outcome.qualifies ? 'eligible' : 'excluded'}`,
          mono: true,
        });
      },
    );
  }
  records.push({ label: 'Browser round trip', value: `${roundTripMs} ms`, mono: true });

  return (
    <Verdict
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

/**
 * The provenance panel's result: the step 1 verdict read back from its own block, set
 * beside current state. A match is the evidence; a mismatch would be a genuine alarm, so
 * it is shown as one rather than hidden.
 */
function ProvenanceBlock({ outcome }: { outcome: RunOutcome }) {
  const { response } = outcome;
  const classAtBlock = typeof response.classAtBlock === 'string' ? response.classAtBlock : null;
  const classAtHead = typeof response.classAtHead === 'string' ? response.classAtHead : null;
  const match = response.match === true;
  const recordedBlock =
    typeof response.recordedBlock === 'number' ? response.recordedBlock : null;
  const finalizedHead =
    typeof response.finalizedHead === 'number' ? response.finalizedHead : null;
  const confirmations =
    typeof response.confirmations === 'number' ? response.confirmations : null;
  const blockHash = typeof response.blockHash === 'string' ? response.blockHash : null;

  const status: ComplianceStatus = match ? 'GREEN' : 'RED';
  const tri: Record<string, string> = { GREEN: 'ClassOne (GREEN)', YELLOW: 'ClassTwo / review (YELLOW)', RED: 'NonLocal (RED)' };

  const records: { label: string; value: string; mono?: boolean }[] = [];
  if (recordedBlock !== null) {
    records.push({ label: 'Verdict recorded at block', value: `#${recordedBlock.toLocaleString('en-IN')}`, mono: true });
  }
  if (blockHash) {
    records.push({ label: 'That block’s hash', value: `${blockHash.slice(0, 14)}…${blockHash.slice(-6)}`, mono: true });
  }
  records.push({
    label: 'Read from state at that block',
    value: classAtBlock ? (tri[classAtBlock] ?? classAtBlock) : 'not found',
    mono: true,
  });
  records.push({
    label: 'Read from current state, now',
    value: classAtHead ? (tri[classAtHead] ?? classAtHead) : 'not found',
    mono: true,
  });
  if (confirmations !== null) {
    records.push({
      label: 'Finalized blocks stacked on top',
      value: confirmations.toLocaleString('en-IN'),
      mono: true,
    });
  }
  if (finalizedHead !== null) {
    records.push({ label: 'Current finalized head', value: `#${finalizedHead.toLocaleString('en-IN')}`, mono: true });
  }
  records.push({ label: 'Match', value: match ? 'identical — record intact' : 'MISMATCH', mono: true });

  const reason = match
    ? `The classification read from block #${recordedBlock?.toLocaleString('en-IN') ?? '—'} is identical to the one in current state, after the rule was amended at step 6. The record was re-derived from a finalized block, not recalled by the application.`
    : 'The verdict read from the recorded block does not match current state. On this chain that should be impossible without a re-finalization — treat it as an alarm, not a demo artefact.';

  return (
    <Verdict
      status={status}
      trigger="Step 1 verdict, re-derived from its block"
      reason={reason}
      blockNumber={recordedBlock ?? undefined}
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
    <div className="border-l-2 border-accent pl-3">
      <p className="text-2xs font-semibold tracking-wide text-accent uppercase">{heading}</p>
      <p className="mt-1 max-w-[75ch] text-sm text-ink">{body}</p>
      {claim && (
        <p className="mt-1.5 max-w-[75ch] text-2xs text-ink-muted">
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
      <ol className="grid gap-1.5 sm:grid-cols-3 lg:grid-cols-7">
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
                className={cn(
                  'w-full rounded-md border px-2.5 py-2 text-left transition-colors duration-150',
                  'focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent',
                  'disabled:cursor-not-allowed disabled:opacity-55',
                  isCurrent
                    ? 'border-accent bg-accent-tint'
                    : done
                      ? 'border-line bg-paper hover:bg-shell'
                      : 'border-dashed border-line bg-paper',
                )}
              >
                <span className="flex items-center gap-2">
                  <span
                    className={cn(
                      'inline-flex size-4 shrink-0 items-center justify-center rounded-sm font-mono text-2xs font-bold',
                      done || isCurrent ? 'bg-accent text-white' : 'bg-shell-2 text-ink-subtle',
                    )}
                  >
                    {done ? <Check className="size-2.5" aria-hidden="true" /> : step.point}
                  </span>
                  <span className="truncate text-2xs font-medium text-ink">
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

const STEP_SHORT_LABELS: Record<StepId, string> = {
  'bid-submission': 'Bid submission',
  'bid-evaluation': 'Bid evaluation',
  'preference-calculation': 'Preference',
  'ca-certification': 'Certification',
  debarment: 'Debarment',
  'rule-update': 'Rule update',
  provenance: 'Provenance',
};
