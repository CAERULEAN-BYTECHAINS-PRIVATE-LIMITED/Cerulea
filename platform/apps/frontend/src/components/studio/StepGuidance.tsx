'use client';

import { useState, useEffect } from 'react';
import {
  Dialog, DialogContent, DialogActions, Button, Typography, Box, Stack, Chip,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import ArrowForwardIcon from '@mui/icons-material/ArrowForward';
import LightbulbOutlinedIcon from '@mui/icons-material/LightbulbOutlined';

export interface GuidanceStep {
  first: string;
  next: string;
}

interface StepGuidanceProps {
  stepKey: string;
  title: string;
  subtitle: string;
  description: string;
  steps: GuidanceStep[];
  tip?: string;
}

export default function StepGuidance({ stepKey, title, subtitle, description, steps, tip }: StepGuidanceProps) {
  const theme = useTheme();
  const [open, setOpen] = useState(false);

  useEffect(() => {
    try {
      const dismissed = localStorage.getItem(`guidance:dismissed:${stepKey}`);
      if (!dismissed) setOpen(true);
    } catch {
      // localStorage not available
    }
  }, [stepKey]);

  const handleDismiss = () => {
    try {
      localStorage.setItem(`guidance:dismissed:${stepKey}`, '1');
    } catch {}
    setOpen(false);
  };

  if (!open) return null;

  return (
    <Dialog
      open={open}
      onClose={handleDismiss}
      maxWidth="sm"
      fullWidth
      PaperProps={{
        sx: {
          borderRadius: 3,
          background: theme.palette.mode === 'dark'
            ? 'rgba(16,16,20,0.97)'
            : 'rgba(255,255,255,0.97)',
          backdropFilter: 'blur(20px)',
          border: `1px solid ${alpha(theme.palette.primary.main, 0.2)}`,
          boxShadow: `0 24px 64px ${alpha(theme.palette.common.black, 0.4)}`,
          overflow: 'hidden',
        },
      }}
    >
      {/* Gradient header bar */}
      <Box
        sx={{
          px: 3, pt: 3, pb: 2,
          background: `linear-gradient(135deg, ${alpha(theme.palette.primary.main, 0.12)}, ${alpha(theme.palette.secondary.main, 0.06)})`,
          borderBottom: `1px solid ${alpha(theme.palette.divider, 0.5)}`,
        }}
      >
        <Chip
          label={subtitle}
          size="small"
          sx={{
            mb: 1.5,
            bgcolor: alpha(theme.palette.primary.main, 0.12),
            color: 'primary.main',
            fontWeight: 700,
            fontSize: '0.65rem',
            letterSpacing: 1,
          }}
        />
        <Typography variant="h6" fontWeight={800} sx={{ lineHeight: 1.2 }}>
          {title}
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mt: 0.75, lineHeight: 1.6 }}>
          {description}
        </Typography>
      </Box>

      <DialogContent sx={{ pt: 2.5, pb: 1 }}>
        <Typography variant="overline" fontWeight={800} color="text.disabled" sx={{ fontSize: '0.6rem', letterSpacing: 1.5 }}>
          HOW TO USE THIS STEP
        </Typography>

        <Stack spacing={1.5} sx={{ mt: 1.5 }}>
          {steps.map((s, i) => (
            <Box
              key={i}
              sx={{
                display: 'flex',
                gap: 1.5,
                p: 1.5,
                borderRadius: 2,
                bgcolor: theme.palette.mode === 'dark' ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.04)',
                border: `1px solid ${alpha(theme.palette.divider, 0.7)}`,
              }}
            >
              <Box
                sx={{
                  width: 22, height: 22, borderRadius: '50%', flexShrink: 0, mt: 0.1,
                  bgcolor: alpha(theme.palette.primary.main, 0.15),
                  color: 'primary.main', fontWeight: 800, fontSize: '0.7rem',
                  display: 'flex', alignItems: 'center', justifyContent: 'center',
                }}
              >
                {i + 1}
              </Box>
              <Box>
                <Stack direction="row" alignItems="center" spacing={0.75} flexWrap="wrap">
                  <Typography variant="body2" fontWeight={700}>{s.first}</Typography>
                  <ArrowForwardIcon sx={{ fontSize: 12, color: 'text.disabled' }} />
                  <Typography variant="body2" color="text.secondary">{s.next}</Typography>
                </Stack>
              </Box>
            </Box>
          ))}
        </Stack>

        {tip && (
          <Box
            sx={{
              mt: 2, p: 1.5, borderRadius: 2,
              bgcolor: alpha(theme.palette.warning.main, 0.06),
              border: `1px solid ${alpha(theme.palette.warning.main, 0.2)}`,
              display: 'flex', gap: 1.25, alignItems: 'flex-start',
            }}
          >
            <LightbulbOutlinedIcon sx={{ fontSize: 16, color: 'warning.main', mt: 0.1, flexShrink: 0 }} />
            <Typography variant="caption" color="text.secondary" sx={{ lineHeight: 1.5 }}>
              <strong>Tip:</strong> {tip}
            </Typography>
          </Box>
        )}
      </DialogContent>

      <DialogActions sx={{ px: 3, pb: 2.5, pt: 1.5 }}>
        <Button
          variant="contained"
          onClick={handleDismiss}
          endIcon={<CheckCircleOutlineIcon />}
          sx={{ borderRadius: 999, fontWeight: 700, px: 3 }}
        >
          Got it, let's go
        </Button>
      </DialogActions>
    </Dialog>
  );
}
