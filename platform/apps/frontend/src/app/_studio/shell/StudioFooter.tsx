'use client';

import React from 'react';
import { Box, Button } from '@mui/material';
import ArrowBackIosNewIcon from '@mui/icons-material/ArrowBackIosNew';
import ArrowForwardIosIcon from '@mui/icons-material/ArrowForwardIos';
import { TOTAL_STEPS } from './StepRegistry';

type Props = {
  step: number;
  onPrev: () => void;
  onNext: () => void;
};

export default function StudioFooter({ step, onPrev, onNext }: Props) {
  return (
    <Box
      component="footer"
      sx={{
        position: 'sticky',
        bottom: 0,
        borderTop: (t) => `1px solid ${t.palette.divider}`,
        backdropFilter: 'blur(14px)',
        backgroundColor: (t) => t.palette.background.paper + 'CC',
        px: 2,
        py: 1,
        display: 'flex',
        gap: 1,
        justifyContent: 'space-between',
      }}
    >
      <Button
        onClick={onPrev}
        disabled={step <= 1}
        startIcon={<ArrowBackIosNewIcon />}
        variant="outlined"
      >
        Back
      </Button>
      <Button
        onClick={onNext}
        disabled={step >= TOTAL_STEPS}
        endIcon={<ArrowForwardIosIcon />}
        variant="contained"
      >
        Next
      </Button>
    </Box>
  );
}
