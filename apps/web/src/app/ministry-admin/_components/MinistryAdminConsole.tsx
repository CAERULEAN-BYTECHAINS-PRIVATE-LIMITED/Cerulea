'use client';

import { Tabs } from '@/components';
import { DebarmentPanel } from './DebarmentPanel';
import { RuleEditor } from './RuleEditor';

/**
 * Two separate powers, kept visibly separate: amending the rule set, and debarring a
 * vendor. They belong to the same official but they are different acts with different
 * consequences, so they never share a form.
 */
export function MinistryAdminConsole({ defaultMinistryId }: { defaultMinistryId: string }) {
  return (
    <Tabs
      items={[
        {
          id: 'rule',
          label: 'Rule set',
          content: <RuleEditor defaultMinistryId={defaultMinistryId} />,
        },
        {
          id: 'debarment',
          label: 'Debarment',
          content: <DebarmentPanel defaultMinistryId={defaultMinistryId} />,
        },
      ]}
    />
  );
}
