// apps/frontend/src/components/common/InfoTooltip.tsx
'use client';
import React from 'react';
import { Tooltip, IconButton } from '@mui/material';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';

type Props = { title: React.ReactNode };

export default function InfoTooltip({ title }: Props) {
  return (
    <Tooltip
      title={title}
      arrow
      placement="bottom-start"
      slotProps={{ popper: { sx: { maxWidth: 520 } } }}
    >
      <IconButton size="small" aria-label="Info">
        <InfoOutlinedIcon fontSize="small" />
      </IconButton>
    </Tooltip>
  );
}
