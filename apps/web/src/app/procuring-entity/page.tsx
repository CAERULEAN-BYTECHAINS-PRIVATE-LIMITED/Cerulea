import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { EvaluationConsole } from './_components/EvaluationConsole';

/** `/procuring-entity` — tech spec Part 9.3, the buyer's evaluation console. */

const PERSONA = getPersona('procuring-entity');

export const metadata: Metadata = {
  title: 'Procuring Entity — CBC-PRAMAAN',
  description:
    'Evaluate the bids received on a tender, check the shared debarment ledger, and apply the purchase preference the Make in India order requires.',
};

export default function ProcuringEntityPage() {
  return (
    <AppShell
      width="wide"
      breadcrumb="Role portals"
      title="Bid evaluation and purchase preference"
      actingAs={PERSONA.actingAs}
    >
      <EvaluationConsole />
    </AppShell>
  );
}
