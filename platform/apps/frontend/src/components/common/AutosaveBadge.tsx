// apps/frontend/src/components/common/AutosaveBadge.tsx
'use client';

import { useEffect, useState } from 'react';
import { Box, Chip, Tooltip } from '@mui/material';

type Props = { projectId: string };

export default function AutosaveBadge({ projectId }: Props) {
  const storageKey = `studio:${projectId}:autosaveStatus`;
  const [state, setState] = useState<{ status: 'saved'|'saving'|'error'|'idle'; ts?: number; bucket?: string }>({ status: 'idle' });

  useEffect(() => {
    const read = () => {
      try {
        const raw = localStorage.getItem(storageKey);
        if (raw) {
          const parsed = JSON.parse(raw);
          setState(parsed);
        }
      } catch { /* ignore */ }
    };
    read();

    const onStorage = (e: StorageEvent) => {
      if (e.key === storageKey) read();
    };
    window.addEventListener('storage', onStorage);

    // small poll to catch same-tab updates
    const id = setInterval(read, 1000);
    return () => {
      window.removeEventListener('storage', onStorage);
      clearInterval(id);
    };
  }, [storageKey]);

  const label =
    state.status === 'saving'
      ? 'Saving…'
      : state.status === 'saved'
      ? `Saved${state.bucket ? ` (${state.bucket})` : ''}`
      : state.status === 'error'
      ? 'Save failed'
      : 'Idle';

  const color:
    | 'default'
    | 'primary'
    | 'success'
    | 'error'
    | 'warning'
    | 'info' =
    state.status === 'saved' ? 'success' :
    state.status === 'saving' ? 'info' :
    state.status === 'error' ? 'error' : 'default';

  return (
    <Tooltip
      title={
        state.ts
          ? new Date(state.ts).toLocaleString()
          : 'Autosave status'
      }
      arrow
    >
      <Box>
        <Chip size="small" color={color} variant="outlined" label={label} />
      </Box>
    </Tooltip>
  );
}
