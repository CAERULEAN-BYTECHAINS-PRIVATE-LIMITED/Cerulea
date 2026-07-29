/**
 * The Compliance Analytics Dashboard (spec Part 9.6).
 *
 * Four counters, three charts, and the ministry rule table underneath. Everything is read
 * live: the counters are storage reads, the verdict chart counts events decoded out of
 * finalized blocks, and the latency chart plots only measurements this session actually
 * took. Where the chain holds nothing, the chart says which storage item was empty rather
 * than drawing a plausible shape.
 */

import type { Metadata } from 'next';
import { AppShell } from '@/components';
import { DashboardClient } from './_components/DashboardClient';

export const metadata: Metadata = {
  title: 'Compliance analytics — CBC-PRAMAAN',
  description:
    'Live compliance outcomes, debarments and measured decision latency across the PRAMAAN pallets.',
};

export const dynamic = 'force-dynamic';

export default function DashboardPage() {
  return (
    <AppShell
      width="wide"
      eyebrow="Cross-ministry view"
      title="Compliance analytics"
      subtitle="What the chain currently holds across all twenty-one ministry rule sets: the verdicts returned in this session, the debarments in force, and how long each trigger point actually took to reach a finalized answer."
    >
      <DashboardClient />
    </AppShell>
  );
}
