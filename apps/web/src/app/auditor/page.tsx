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
      eyebrow={`Certification console · ${PERSONA.actingAs}`}
      title="Local content certification"
      subtitle="Below ₹10 crore a vendor self-certifies. At or above it a cost or chartered accountant's certificate is mandatory — and every certificate stays bound to the account that signed it, which is what makes the accountability ledger below possible."
    >
      <AuditorConsole />
    </AppShell>
  );
}
