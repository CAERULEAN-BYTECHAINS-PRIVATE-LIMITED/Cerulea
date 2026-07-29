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
      breadcrumb="Role portals"
      title="Rule set and debarment"
      actingAs={PERSONA.actingAs}
    >
      <MinistryAdminConsole defaultMinistryId={DEFAULT_MINISTRY_ID} />
    </AppShell>
  );
}
