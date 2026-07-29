/**
 * Judge walkthrough mode (spec Part 9.7).
 *
 * A presenter drives the whole demo from one button without remembering a click sequence,
 * and a judge reading the screen alone gets the same account. Every step runs a real
 * trigger point against the live chain — there is no recorded path and no mocked response.
 */

import type { Metadata } from 'next';
import { AppShell, Card, CardBody } from '@/components';
import { WalkthroughClient } from './_components/WalkthroughClient';

export const metadata: Metadata = {
  title: 'Guided walkthrough — CBC-PRAMAAN',
  description:
    'The six PRAMAAN trigger points, run in order against the live chain, with an explanation of each verdict.',
};

export const dynamic = 'force-dynamic';

export default function DemoPage() {
  return (
    <AppShell
      eyebrow="Judge walkthrough"
      title="One procurement, six trigger points"
      subtitle="A single GeM tender followed from the vendor's declaration through to a ministry amending the rule that judged it. Each step runs a real transaction on the live Cerulea network and returns only once the block carrying it has reached finality."
      width="wide"
    >
      <div className="space-y-6">
        <Card>
          <CardBody className="grid gap-x-8 gap-y-4 text-sm leading-relaxed text-ink-muted md:grid-cols-3">
            <p>
              <span className="font-medium text-ink">Nothing here is a recording. </span>
              Every step signs and submits an extrinsic to the chain this console is
              connected to. If the network is down, the step fails and says so.
            </p>
            <p>
              <span className="font-medium text-ink">The verdicts are the chain&rsquo;s. </span>
              Green, amber and red are what the pallets returned, along with the sentence
              they returned explaining themselves. The captions explain the answer; they do
              not supply it.
            </p>
            <p>
              <span className="font-medium text-ink">Every step is traceable. </span>
              Each verdict carries the transaction reference and the finalized block number.
              Open the on-chain record on any of them to follow it into the explorer.
            </p>
          </CardBody>
        </Card>

        <WalkthroughClient />
      </div>
    </AppShell>
  );
}
