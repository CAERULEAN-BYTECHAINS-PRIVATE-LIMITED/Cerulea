import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { VigilanceConsole } from './_components/VigilanceConsole';

/** `/cvc` — tech spec Part 9.3, the vigilance and audit reviewer's console. */

const PERSONA = getPersona('cvc');

export const metadata: Metadata = {
  title: 'CVC / Audit Reviewer — CBC-PRAMAAN',
  description:
    'Read the finalized record across tenders and ministries: every inconsistency flag, both declarations side by side, and a vendor’s full declaration history.',
};

export default function CvcPage() {
  return (
    <AppShell
      width="wide"
      eyebrow={`Vigilance console · ${PERSONA.actingAs}`}
      title="Cross-tender review"
      subtitle="A declaration made to one ministry is checked against the same vendor's declarations to every other one. This console reads that record after the fact and submits nothing itself."
    >
      <VigilanceConsole />
    </AppShell>
  );
}
