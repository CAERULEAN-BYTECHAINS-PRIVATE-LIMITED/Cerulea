import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { AuditorConsole } from './_components/AuditorConsole';

/** `/auditor` — tech spec Part 9.3, the cost or chartered accountant's console. */

const PERSONA = getPersona('auditor');

export const metadata: Metadata = {
  title: 'Cost or Chartered Accountant — CBC-PRAMAAN',
  description:
    'Certify local content for contracts at or above the ₹10 crore threshold, and see every certificate this auditor has signed.',
};

export default function AuditorPage() {
  return (
    <AppShell
      width="wide"
      breadcrumb="Role portals"
      title="Local content certification"
      actingAs={PERSONA.actingAs}
    >
      <AuditorConsole />
    </AppShell>
  );
}
