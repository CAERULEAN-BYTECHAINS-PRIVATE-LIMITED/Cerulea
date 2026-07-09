'use client';

import React, { useState } from 'react';
import { AppBar, Toolbar, Box, Typography, IconButton, LinearProgress, Drawer, Tooltip, Button } from '@mui/material';
import ArrowBackIosNewIcon from '@mui/icons-material/ArrowBackIosNew';
import SmartToyOutlinedIcon from '@mui/icons-material/SmartToyOutlined';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import { getStepLabel, TOTAL_STEPS } from './StepRegistry';
import SmartContractsScreen from '@/components/SmartContractsScreen';

type Props = {
  step: number;
  onPrev: () => void;
  onNext: () => void;
  projectId?: string | null;
  autosaveLabel?: string;
  busy?: boolean;
};

export default function StudioHeader({
  step,
  onPrev,
  onNext,
  projectId,
  autosaveLabel,
  busy = false,
}: Props) {
  const pct = (Math.min(Math.max(step, 1), TOTAL_STEPS) / TOTAL_STEPS) * 100;
  const [contractsOpen, setContractsOpen] = useState(false);

  return (
    <>
      <AppBar
        position="sticky"
        elevation={0}
        sx={{
          top: 0,
          backdropFilter: 'blur(14px)',
          backgroundColor: (t) => t.palette.background.paper + 'CC',
          borderBottom: (t) => `1px solid ${t.palette.divider}`,
        }}
      >
        <Toolbar sx={{ minHeight: 56, display: 'flex', gap: 1 }}>
          <IconButton color="inherit" onClick={onPrev} size="small" aria-label="Previous step">
            <ArrowBackIosNewIcon fontSize="inherit" />
          </IconButton>

          <Box sx={{ flex: 1, minWidth: 0 }}>
            <Typography noWrap fontWeight={600}>
              Step {step} of {TOTAL_STEPS}: {getStepLabel(step)}
            </Typography>
            <Typography variant="caption" sx={{ opacity: 0.7 }}>
              {projectId ? `Project: ${projectId}` : 'No project selected'}
              {autosaveLabel ? ` • ${autosaveLabel}` : ''}
            </Typography>
            <LinearProgress
              variant="determinate"
              value={pct}
              sx={{ height: 3, borderRadius: 1, mt: 0.5 }}
            />
          </Box>

          <Tooltip title="Smart Contracts: view and manage contracts derived from your blueprint">
            <Button
              variant="outlined"
              size="small"
              startIcon={<HexagonOutlinedIcon />}
              onClick={() => setContractsOpen(true)}
              sx={{ borderRadius: 999, fontSize: '0.7rem', fontWeight: 700, px: 1.5, py: 0.5 }}
            >
              Contracts
            </Button>
          </Tooltip>

          <IconButton color="inherit" aria-label="Open AI">
            <SmartToyOutlinedIcon />
          </IconButton>
        </Toolbar>
        {busy && <LinearProgress />}
      </AppBar>

      <Drawer
        anchor="right"
        open={contractsOpen}
        onClose={() => setContractsOpen(false)}
        PaperProps={{ sx: { width: { xs: '100%', md: 800 }, maxWidth: '100vw' } }}
      >
        <SmartContractsScreen onClose={() => setContractsOpen(false)} />
      </Drawer>
    </>
  );
}
