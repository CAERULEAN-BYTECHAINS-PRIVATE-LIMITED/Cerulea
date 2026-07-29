import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { NationalRollup } from './_components/NationalRollup';

/** `/dpiit` — tech spec Part 9.3, the national view. */

const PERSONA = getPersona('dpiit');

export const metadata: Metadata = {
  title: 'DPIIT — CBC-PRAMAAN',
  description:
    'Ministries onboarded, the compliance outcome distribution, and every ministry rule set compared against the national default.',
};

export default function DpiitPage() {
  return (
    <AppShell
      width="wide"
      eyebrow={`National console · ${PERSONA.actingAs}`}
      title="Make in India compliance, nationally"
      subtitle="DPIIT owns the default rule every ministry inherits and sees the outcome of every ministry that has departed from it. Onboarding the next ministry is a row of configuration, not a release."
    >
      <NationalRollup />
    </AppShell>
  );
}
