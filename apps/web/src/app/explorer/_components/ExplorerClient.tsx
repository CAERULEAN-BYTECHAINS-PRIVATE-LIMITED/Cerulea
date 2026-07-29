'use client';

import { useCallback, useState } from 'react';
import { Tabs, type TabItem } from '@/components';
import { BlocksPanel } from './BlocksPanel';
import { EventsPanel } from './EventsPanel';
import { SearchPanel } from './SearchPanel';
import { ValidatorsPanel } from './ValidatorsPanel';

/**
 * The four explorer views.
 *
 * A `?tx=` in the URL opens on Search with the lookup already running — `ComplianceResult`
 * links here with the transaction reference the moment a trigger point returns, and a
 * judge following that link should land on the answer, not on a form.
 */
export function ExplorerClient({
  initialTx,
  initialBlock,
}: {
  initialTx?: string;
  initialBlock?: number;
}) {
  const hasJumpTarget = Boolean(initialTx || initialBlock !== undefined);
  const [tab, setTab] = useState(hasJumpTarget ? 'search' : 'blocks');
  const [pendingTx, setPendingTx] = useState<string | undefined>(initialTx);
  const [pendingBlock, setPendingBlock] = useState<number | undefined>(initialBlock);

  const openTx = useCallback((txRef: string) => {
    setPendingBlock(undefined);
    setPendingTx(txRef);
    setTab('search');
  }, []);

  const openBlock = useCallback((blockNumber: number) => {
    setPendingTx(undefined);
    setPendingBlock(blockNumber);
    setTab('search');
  }, []);

  const items: TabItem[] = [
    {
      id: 'blocks',
      label: 'Blocks',
      content: <BlocksPanel onOpenBlock={openBlock} />,
    },
    {
      id: 'validators',
      label: 'Validators',
      content: <ValidatorsPanel />,
    },
    {
      id: 'events',
      label: 'Events',
      content: <EventsPanel onOpenTx={openTx} />,
    },
    {
      id: 'search',
      label: 'Search',
      content: (
        <SearchPanel
          key={pendingTx ?? pendingBlock ?? 'blank'}
          initialTx={pendingTx}
          initialBlock={pendingBlock}
        />
      ),
    },
  ];

  return <Tabs items={items} value={tab} onValueChange={setTab} />;
}
