'use client';

import { Paper, styled, alpha } from '@mui/material';

export const GlassPanel = styled(Paper)(({ theme }) => ({
  // The "Real Glass" Effect (High Transparency, Low Blur)
  backgroundColor: alpha(
    theme.palette.mode === 'dark' ? '#1D273A' : '#FFFFFF',
    theme.palette.mode === 'dark' ? 0.6 : 0.5
  ),
  backdropFilter: 'blur(10px)',
  WebkitBackdropFilter: 'blur(10px)',
  border: `1px solid ${alpha(
    theme.palette.mode === 'dark' ? '#FFFFFF' : '#000000',
    0.15
  )}`,
  boxShadow: `0 8px 32px 0 ${alpha(
    theme.palette.mode === 'dark' ? '#000000' : '#172b4d',
    0.1
  )}`,
  backgroundImage: 'none',
}));