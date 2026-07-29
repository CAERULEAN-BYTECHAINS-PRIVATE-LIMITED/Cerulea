import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { getPersona } from '@/lib/personas';
import { BidSubmissionConsole } from './_components/BidSubmissionConsole';

/**
 * `/vendor` — tech spec Part 9.3, the vendor console.
 *
 * A Server Component that renders the frame and hands the interactive bid form to a
 * Client Component, so the page's headings and institutional copy stream immediately and
 * only the form itself waits on JavaScript.
 */

const PERSONA = getPersona('vendor');

export const metadata: Metadata = {
  title: 'Vendor — CBC-PRAMAAN',
  description:
    'Declare local content against a live GeM bid and see the Class I / Class II / Non-local verdict before the bid closes.',
};

export default function VendorPage() {
  return (
    <AppShell
      eyebrow={`Vendor console · ${PERSONA.actingAs}`}
      title="Submit a bid on GeM"
      subtitle="The bid form a seller already fills, with one addition: the local content declaration is classified against the buying ministry's own rule set and answered before the bid closes, instead of being disputed after the award."
    >
      <BidSubmissionConsole />
    </AppShell>
  );
}
