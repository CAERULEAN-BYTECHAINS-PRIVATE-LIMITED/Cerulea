/**
 * Judge walkthrough mode (spec Part 9.7).
 *
 * A presenter drives the whole demo from one button without remembering a click sequence,
 * and a judge reading the screen alone gets the same account. Every step runs a real
 * trigger point against the live chain — there is no recorded path and no mocked response.
 */

import type { Metadata } from 'next';
import { AppShell, FactGrid, Panel, PanelBody, PanelHead } from '@/components';
import { WalkthroughClient } from './_components/WalkthroughClient';

export const metadata: Metadata = {
  title: 'Guided walkthrough — CBC-PRAMAAN',
  description:
    'The six PRAMAAN trigger points, run in order against the live chain, with an explanation of each verdict.',
};

export const dynamic = 'force-dynamic';

export default function DemoPage() {
  return (
    <AppShell breadcrumb="Judge walkthrough" title="One procurement, six trigger points" width="wide">
      <div className="space-y-4">
        <Panel>
          <PanelHead title="How to read this" />
          <PanelBody>
            <FactGrid
              facts={[
                {
                  label: 'Not a recording',
                  value:
                    'Every step signs and submits a real extrinsic. If the network is down, the step fails and says so.',
                },
                {
                  label: 'The verdicts are the chain’s',
                  value:
                    'GREEN, YELLOW and RED are what the pallets returned, with their own reason sentence.',
                },
                {
                  label: 'Every step is traceable',
                  value:
                    'Each verdict carries its transaction reference and finalized block number.',
                },
              ]}
            />
          </PanelBody>
        </Panel>

        <WalkthroughClient />
      </div>
    </AppShell>
  );
}
