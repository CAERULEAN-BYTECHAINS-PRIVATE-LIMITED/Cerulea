import { ArrowRight } from 'lucide-react';
import Link from 'next/link';
import {
  AppShell,
  Figure,
  FigureRow,
  Panel,
  PanelHead,
  PanelNote,
  StatusToken,
  TBody,
  TD,
  TH,
  THead,
  TR,
  Table,
} from '@/components';
import { MINISTRY_COUNT } from '@/lib/api-client';
import { NETWORK_ROUTES, PERSONAS } from '@/lib/personas';

/**
 * The front door: a directory, not a landing page.
 *
 * Six role portals and two network views, listed the way a government portal lists its
 * services — one row each, one line of remit, one destination. There is no hero, no
 * proposition and no call to action, because this is signed-in software and the reader
 * already knows why they are here.
 *
 * `/demo` is deliberately not on this page and is not linked from anywhere else.
 */

const NETWORK_REMIT: Record<string, string> = {
  '/dashboard': 'Live compliance outcomes, debarments in force, and measured decision latency.',
  '/explorer': 'Blocks, validators, runtime events, and lookup by transaction or block.',
};

const LEGEND = [
  {
    token: 'GREEN' as const,
    word: 'Compliant',
    meaning: 'Proceeds. A Class-I classification, or a qualifying preference outcome.',
  },
  {
    token: 'YELLOW' as const,
    word: 'Review required',
    meaning: 'Proceeds with a caveat, or needs a human. Class-II, manual review, certificate pending.',
  },
  {
    token: 'RED' as const,
    word: 'Blocked',
    meaning: 'Non-local, an active debarment, or a failed eligibility gate. The rule is named.',
  },
];

export default function Home() {
  return (
    <AppShell breadcrumb="CBC-PRAMAAN" title="Compliance verification portal">
      <div className="space-y-4">
        <FigureRow>
          <Figure
            label="Nodal ministries"
            value={MINISTRY_COUNT}
            note="Each with its own thresholds, Para 3A position and calculation method."
          />
          <Figure
            label="Decision pathways"
            value="12"
            note="P1 to P12 of the Order, each implemented in a pallet and named on the result."
          />
          <Figure
            label="Trigger points"
            value="6"
            note="Submission, evaluation, preference, certification, debarment, rule update."
          />
          <Figure
            label="Validators"
            value="3"
            note="No verdict is returned until its block is finalized by the network."
          />
        </FigureRow>

        <Panel>
          <PanelHead title="Role portals" meta={<span className="text-2xs text-ink-muted">Demo mode — role selection, not sign-in</span>} />
          <Table>
            <THead>
              <TR>
                <TH className="w-52">Portal</TH>
                <TH>Remit</TH>
                <TH className="w-72">Signed in as</TH>
                <TH className="w-24">
                  <span className="sr-only">Open</span>
                </TH>
              </TR>
            </THead>
            <TBody>
              {PERSONAS.map((persona) => (
                <TR key={persona.id} className="hover:bg-shell">
                  <TD>
                    <Link
                      href={persona.route}
                      className="rounded-sm font-medium text-accent hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                    >
                      {persona.label}
                    </Link>
                  </TD>
                  <TD className="text-ink-muted">{persona.remit}</TD>
                  <TD className="text-ink-muted">{persona.actingAs}</TD>
                  <TD className="text-right">
                    <Link
                      href={persona.route}
                      aria-label={`Open the ${persona.label} portal`}
                      className="inline-flex items-center gap-1 rounded-sm text-2xs font-medium text-accent hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                    >
                      Open
                      <ArrowRight className="size-3" aria-hidden="true" />
                    </Link>
                  </TD>
                </TR>
              ))}
            </TBody>
          </Table>
          <PanelNote>
            Each of the six roles sees only what its role permits. Selecting a portal does not
            authenticate anyone — this proof of concept has no sign-in.
          </PanelNote>
        </Panel>

        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <Panel>
            <PanelHead title="Network views" />
            <Table>
              <THead>
                <TR>
                  <TH className="w-32">View</TH>
                  <TH>Contents</TH>
                </TR>
              </THead>
              <TBody>
                {NETWORK_ROUTES.map((route) => (
                  <TR key={route.href} className="hover:bg-shell">
                    <TD>
                      <Link
                        href={route.href}
                        className="rounded-sm font-medium text-accent hover:underline focus-visible:outline-2 focus-visible:outline-offset-1 focus-visible:outline-accent"
                      >
                        {route.label}
                      </Link>
                    </TD>
                    <TD className="text-ink-muted">{NETWORK_REMIT[route.href]}</TD>
                  </TR>
                ))}
              </TBody>
            </Table>
          </Panel>

          <Panel>
            <PanelHead title="What a result means" />
            <Table>
              <THead>
                <TR>
                  <TH className="w-20">Token</TH>
                  <TH className="w-36">Verdict</TH>
                  <TH>Applies when</TH>
                </TR>
              </THead>
              <TBody>
                {LEGEND.map((entry) => (
                  <TR key={entry.token}>
                    <TD>
                      <StatusToken status={entry.token} />
                    </TD>
                    <TD className="font-medium">{entry.word}</TD>
                    <TD className="text-ink-muted">{entry.meaning}</TD>
                  </TR>
                ))}
              </TBody>
            </Table>
            <PanelNote>
              A verdict is never carried by colour alone: each pairs a distinct icon, the status
              word and the reserved colour, so it survives a washed-out projector, a colour-blind
              reader and a screen reader equally. These three colours are used for nothing else.
            </PanelNote>
          </Panel>
        </div>
      </div>
    </AppShell>
  );
}
