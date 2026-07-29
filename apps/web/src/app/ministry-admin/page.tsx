import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { MinistryAdminConsole } from './_components/MinistryAdminConsole';

/** `/ministry-admin` — tech spec Part 9.3, the nodal ministry administrator's console. */

const PERSONA = getPersona('ministry-admin');

/** The persona signs in as MeitY; every other ministry stays selectable. */
const DEFAULT_MINISTRY_ID = 'MEITY';

export const metadata: Metadata = {
  title: 'Nodal Ministry Administrator — CBC-PRAMAAN',
  description:
    'Amend a ministry rule set — thresholds, Para 3A, PLI linkage, certification threshold — and record or lift a debarment on the shared national ledger.',
};

export default function MinistryAdminPage() {
  return (
    <AppShell
      width="wide"
      eyebrow={`Nodal ministry console · ${PERSONA.actingAs}`}
      title="Rule set and debarment"
      subtitle="Adding a ministry or amending a threshold is a configuration change, not an engineering one: the amended rule is a storage write carrying a version number and an effective block, and every bid evaluated after that block is judged against it without anything being redeployed."
    >
      <MinistryAdminConsole defaultMinistryId={DEFAULT_MINISTRY_ID} />
    </AppShell>
  );
}
