import { CircleCheckBig, CircleX, LayoutDashboard, PlayCircle, TriangleAlert } from 'lucide-react';
import Link from 'next/link';
import { AppShell, PersonaCard, buttonClasses } from '@/components';
import { PERSONAS } from '@/lib/personas';

/**
 * The landing page: a role chooser, presented as a real product front door.
 *
 * Everything on it is a fact about this build — the twelve pathways come from
 * `pramaan-primitives`, the twenty-one ministries from `scripts/seed-ministries`, and the
 * finality claim from the Part 8.3 wait every trigger-point route performs.
 */

const FACTS = [
  {
    figure: '12',
    label: 'Decision pathways',
    detail: 'P1 to P12 of the Order, each implemented in a pallet and each named on the result.',
  },
  {
    figure: '21',
    label: 'Nodal ministries',
    detail: 'Every ministry carries its own thresholds, Para 3A list and calculation method.',
  },
  {
    figure: '6',
    label: 'Trigger points',
    detail: 'Bid submission, evaluation, preference, certification, debarment and rule update.',
  },
  {
    figure: 'Finality',
    label: 'Before any answer',
    detail: 'No verdict is returned until the block carrying it is finalized by the network.',
  },
];

const LEGEND = [
  {
    Icon: CircleCheckBig,
    word: 'Compliant',
    token: 'GREEN',
    meaning: 'Compliant and proceeds — a Class I classification or a qualifying preference outcome.',
    dot: 'text-status-green',
    chip: 'bg-status-green text-white',
  },
  {
    Icon: TriangleAlert,
    word: 'Review required',
    token: 'YELLOW',
    meaning:
      'Proceeds with a caveat or needs a human — Class II, manual review, or certification still awaiting its auditor.',
    dot: 'text-[#8a6100]',
    chip: 'bg-status-yellow text-ink',
  },
  {
    Icon: CircleX,
    word: 'Blocked',
    token: 'RED',
    meaning:
      'Blocked — Non-local, an active debarment, or a failed eligibility gate. The result names the rule that caused it.',
    dot: 'text-status-red',
    chip: 'bg-status-red text-white',
  },
];

export default function Home() {
  return (
    <AppShell>
      {/* ---- Hero -------------------------------------------------------------- */}
      <section className="pt-4 pb-12 sm:pt-10">
        <p className="text-xs font-semibold tracking-wide text-cerulea uppercase">
          Proof of concept · Public Procurement (Preference to Make in India) Order, 2017 as
          amended
        </p>

        <h1 className="mt-3 max-w-4xl text-3xl leading-[1.15] font-semibold tracking-tight text-balance text-ink sm:text-[2.75rem]">
          Make in India compliance, decided on chain and provable long after the tender closes.
        </h1>

        <p className="mt-5 max-w-2xl text-base leading-relaxed text-ink-muted">
          CBC-PRAMAAN turns the local content rules into something a procuring entity can act on
          in the moment and an auditor can verify years later. Each classification, preference
          calculation, certification and debarment is a finalized transaction on the Cerulea
          network, carrying the rule version that was in force when it was made.
        </p>

        <div className="mt-8 flex flex-wrap items-center gap-3">
          <Link
            href="/demo"
            className={buttonClasses({ variant: 'primary', size: 'lg' })}
          >
            <PlayCircle className="size-5" aria-hidden="true" />
            Start the guided walkthrough
          </Link>
          <Link
            href="/dashboard"
            className={buttonClasses({ variant: 'secondary', size: 'lg' })}
          >
            <LayoutDashboard className="size-5" aria-hidden="true" />
            Open the dashboard
          </Link>
        </div>

        <dl className="mt-12 grid grid-cols-1 gap-px overflow-hidden rounded-card border border-border bg-border sm:grid-cols-2 lg:grid-cols-4">
          {FACTS.map((fact) => (
            <div key={fact.label} className="bg-surface px-5 py-5">
              <dt className="text-xs font-semibold tracking-wide text-ink-muted uppercase">
                {fact.label}
              </dt>
              <dd>
                <p className="mt-2 text-2xl font-semibold tracking-tight text-ink">
                  {fact.figure}
                </p>
                <p className="mt-1.5 text-sm leading-relaxed text-ink-muted">{fact.detail}</p>
              </dd>
            </div>
          ))}
        </dl>
      </section>

      {/* ---- Role chooser ------------------------------------------------------ */}
      <section aria-labelledby="roles-heading" className="border-t border-border py-12">
        <div className="flex flex-wrap items-end justify-between gap-4">
          <div>
            <h2
              id="roles-heading"
              className="text-xl font-semibold tracking-tight text-ink sm:text-2xl"
            >
              Choose a role
            </h2>
            <p className="mt-1.5 max-w-2xl text-sm text-ink-muted">
              Each of the six roles sees only what its role permits. Pick one to open its
              console.
            </p>
          </div>
          <p className="rounded-full border border-border bg-surface px-3 py-1.5 text-xs text-ink-muted">
            Demo mode — role selection, not sign-in
          </p>
        </div>

        <ul className="mt-6 grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {PERSONAS.map((persona) => (
            <li key={persona.id} className="flex">
              <PersonaCard persona={persona} className="w-full" />
            </li>
          ))}
        </ul>
      </section>

      {/* ---- Verdict legend ---------------------------------------------------- */}
      <section aria-labelledby="legend-heading" className="border-t border-border py-12">
        <h2
          id="legend-heading"
          className="text-xl font-semibold tracking-tight text-ink sm:text-2xl"
        >
          What a result means
        </h2>
        <p className="mt-1.5 max-w-2xl text-sm text-ink-muted">
          Every trigger point returns one of three verdicts, with one plain sentence of reason
          and the transaction it was recorded in. The three colours are reserved for exactly
          this and are never used for anything else in the product.
        </p>

        <ul className="mt-6 grid grid-cols-1 gap-4 md:grid-cols-3">
          {LEGEND.map((entry) => (
            <li
              key={entry.token}
              className="rounded-card border border-border bg-surface px-5 py-4"
            >
              <div className="flex items-center gap-2.5">
                <entry.Icon className={`size-5 ${entry.dot}`} aria-hidden="true" />
                <span className={`font-semibold ${entry.dot}`}>{entry.word}</span>
                <span
                  className={`rounded px-1.5 py-0.5 font-mono text-[0.6875rem] font-bold tracking-widest ${entry.chip}`}
                >
                  {entry.token}
                </span>
              </div>
              <p className="mt-2.5 text-sm leading-relaxed text-ink-muted">{entry.meaning}</p>
            </li>
          ))}
        </ul>

        <p className="mt-6 max-w-3xl text-sm text-ink-muted">
          A verdict is never carried by colour alone: each one pairs a distinct icon, the status
          word and the reserved colour, so it survives a washed-out projector, a colour-blind
          reader and a screen reader equally.
        </p>
      </section>
    </AppShell>
  );
}
