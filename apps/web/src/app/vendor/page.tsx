import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { BidSubmissionConsole } from './_components/BidSubmissionConsole';

/** `/vendor` — tech spec Part 9.3, the vendor console. */

const PERSONA = getPersona('vendor');

export const metadata: Metadata = {
  title: 'Vendor',
  description:
    'Declare local content against a live GeM bid and see the Class I / Class II / Non-local verdict before the bid closes.',
};

export default function VendorPage() {
  return (
    <AppShell
      breadcrumb="Role portals"
      title="Bid participation"
      actingAs={PERSONA.actingAs}
      width="wide"
    >
      <BidSubmissionConsole />
    </AppShell>
  );
}
