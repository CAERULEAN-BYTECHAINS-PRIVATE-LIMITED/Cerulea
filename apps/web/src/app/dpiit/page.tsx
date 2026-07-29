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
      breadcrumb="Role portals"
      title="National compliance rollup"
      actingAs={PERSONA.actingAs}
    >
      <NationalRollup />
    </AppShell>
  );
}
