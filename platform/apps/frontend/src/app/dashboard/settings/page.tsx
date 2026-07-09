'use client';

import { useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  TextField, Divider, LinearProgress,
  Dialog, DialogTitle, DialogContent, DialogActions, Alert,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import { useSession } from 'next-auth/react';
import SaveIcon from '@mui/icons-material/Save';
import UpgradeIcon from '@mui/icons-material/Upgrade';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import DeleteForeverIcon from '@mui/icons-material/DeleteForever';
import RefreshIcon from '@mui/icons-material/Refresh';
import StarIcon from '@mui/icons-material/Star';

const PLAN_COLOR: Record<string, string> = {
  Developer: '#448aff',
  Pro: '#9c27b0',
  Enterprise: '#ff6d00',
};

const USAGE = {
  projects: { used: 3, limit: 5 },
  deployments: { used: 12, limit: 20 },
  apiCalls: { used: 48_200, limit: 100_000 },
  storage: { used: 4.8, limit: 10 },
};

function UsageMeter({ label, used, limit, unit }: { label: string; used: number; limit: number; unit?: string }) {
  const theme = useTheme();
  const pct = Math.min((used / limit) * 100, 100);
  const color = pct >= 90 ? theme.palette.error.main : pct >= 70 ? theme.palette.warning.main : theme.palette.success.main;
  return (
    <Box>
      <Stack direction="row" justifyContent="space-between" alignItems="baseline" mb={0.5}>
        <Typography variant="body2" fontWeight={600}>{label}</Typography>
        <Typography variant="caption" color="text.secondary">
          {typeof used === 'number' && used > 1000 ? used.toLocaleString() : used}
          {unit ? ` ${unit}` : ''} / {typeof limit === 'number' && limit > 1000 ? limit.toLocaleString() : limit}{unit ? ` ${unit}` : ''}
        </Typography>
      </Stack>
      <LinearProgress
        variant="determinate"
        value={pct}
        sx={{ borderRadius: 999, height: 7, bgcolor: alpha(color, 0.12), '& .MuiLinearProgress-bar': { bgcolor: color } }}
      />
    </Box>
  );
}

export default function SettingsPage() {
  const theme = useTheme();
  const { data: session } = useSession();

  const [name, setName] = useState(session?.user?.name || '');
  const [email, setEmail] = useState(session?.user?.email || '');
  const [saved, setSaved] = useState(false);

  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [deleteConfirm, setDeleteConfirm] = useState('');

  const [resetLoading, setResetLoading] = useState(false);
  const [resetDone, setResetDone] = useState(false);

  const isTestAccount = session?.user?.email === 'test@cerulea.app';
  const currentPlan = 'Developer';

  const handleSave = () => {
    setSaved(true);
    setTimeout(() => setSaved(false), 2000);
  };

  const handleResetTestAccount = async () => {
    setResetLoading(true);
    try {
      await fetch('/api/test-account/reset', { method: 'POST' });

      // Clear ALL studio localStorage keys — full factory reset
      const STUDIO_KEYS = [
        'cerulea.projectType', 'cerulea.templateId', 'cerulea.templateModules',
        'cerulea.step1.graph', 'cerulea.step3.economics', 'cerulea.step4.integrations',
        'cerulea.step5.ui', 'cerulea.deployed',
        'draft:local:1', 'draft:local:2', 'draft:local:3', 'draft:local:4',
        'draft:local:5', 'draft:local:6', 'draft:local:7',
      ];
      STUDIO_KEYS.forEach((k) => localStorage.removeItem(k));

      // Clear guidance dismissal flags so all step guidance shows again
      Object.keys(localStorage)
        .filter((k) => k.startsWith('guidance:dismissed:'))
        .forEach((k) => localStorage.removeItem(k));

      setResetDone(true);
      // Give 1.5s for confirmation to show, then redirect to studio
      setTimeout(() => {
        const isLocal = window.location.hostname.includes('localhost');
        window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
      }, 1500);
    } catch {
      // silent
    } finally {
      setResetLoading(false);
    }
  };

  return (
    <Box sx={{ p: 4, maxWidth: 860 }}>
      {/* Header */}
      <Box mb={4}>
        <Typography variant="h4" fontWeight={900} gutterBottom>
          Settings & Billing
        </Typography>
        <Typography variant="body1" color="text.secondary">
          Manage your profile, subscription, and account configuration.
        </Typography>
      </Box>

      {/* ── Section 1: Profile ── */}
      <Paper variant="outlined" sx={{ p: 3, borderRadius: 3, mb: 3 }}>
        <Typography variant="h6" fontWeight={800} mb={2.5}>Profile</Typography>
        <Grid container spacing={2.5}>
          <Grid xs={12} sm={6}>
            <TextField
              label="Full Name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              fullWidth
              size="small"
            />
          </Grid>
          <Grid xs={12} sm={6}>
            <TextField
              label="Email Address"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              fullWidth
              size="small"
              type="email"
            />
          </Grid>
          <Grid xs={12} sm={6}>
            <TextField
              label="Current Password"
              type="password"
              placeholder="Leave blank to keep unchanged"
              fullWidth
              size="small"
            />
          </Grid>
          <Grid xs={12} sm={6}>
            <TextField
              label="New Password"
              type="password"
              placeholder="Minimum 8 characters"
              fullWidth
              size="small"
            />
          </Grid>
        </Grid>
        <Box mt={2.5}>
          <Button
            variant="contained"
            startIcon={saved ? undefined : <SaveIcon />}
            onClick={handleSave}
            color={saved ? 'success' : 'primary'}
            sx={{ borderRadius: 999, fontWeight: 700 }}
          >
            {saved ? 'Saved!' : 'Save Changes'}
          </Button>
        </Box>
      </Paper>

      {/* ── Section 2: Subscription ── */}
      <Paper
        variant="outlined"
        sx={{
          p: 3, borderRadius: 3, mb: 3,
          background: alpha(PLAN_COLOR[currentPlan] || theme.palette.primary.main, 0.03),
          borderColor: alpha(PLAN_COLOR[currentPlan] || theme.palette.primary.main, 0.18),
        }}
      >
        <Stack direction="row" alignItems="center" justifyContent="space-between" mb={2.5}>
          <Stack direction="row" alignItems="center" spacing={1.5}>
            <StarIcon sx={{ color: PLAN_COLOR[currentPlan] || 'primary.main' }} />
            <Typography variant="h6" fontWeight={800}>Subscription</Typography>
          </Stack>
          <Chip
            label={currentPlan}
            sx={{
              fontWeight: 900,
              bgcolor: alpha(PLAN_COLOR[currentPlan] || theme.palette.primary.main, 0.12),
              color: PLAN_COLOR[currentPlan] || 'primary.main',
              border: `1px solid ${alpha(PLAN_COLOR[currentPlan] || theme.palette.primary.main, 0.3)}`,
              fontSize: '0.8rem',
            }}
          />
        </Stack>

        <Stack spacing={2} mb={2.5}>
          <UsageMeter label="Projects" used={USAGE.projects.used} limit={USAGE.projects.limit} />
          <UsageMeter label="Deployments this month" used={USAGE.deployments.used} limit={USAGE.deployments.limit} />
          <UsageMeter label="API Calls" used={USAGE.apiCalls.used} limit={USAGE.apiCalls.limit} unit="calls" />
          <UsageMeter label="Storage" used={USAGE.storage.used} limit={USAGE.storage.limit} unit="GB" />
        </Stack>

        <Divider sx={{ mb: 2.5 }} />

        <Stack direction="row" alignItems="center" justifyContent="space-between">
          <Box>
            <Typography variant="body2" color="text.secondary">
              Next billing: <strong>May 1, 2026</strong> · $0 (Developer plan is free)
            </Typography>
          </Box>
          <Button
            variant="contained"
            startIcon={<UpgradeIcon />}
            sx={{
              borderRadius: 999, fontWeight: 700,
              bgcolor: PLAN_COLOR['Pro'],
              '&:hover': { bgcolor: alpha(PLAN_COLOR['Pro'], 0.85) },
            }}
          >
            Upgrade to Pro
          </Button>
        </Stack>
      </Paper>

      {/* ── Section 3: Danger Zone ── */}
      <Paper
        variant="outlined"
        sx={{
          p: 3, borderRadius: 3,
          borderColor: alpha(theme.palette.error.main, 0.25),
          background: alpha(theme.palette.error.main, 0.02),
        }}
      >
        <Stack direction="row" alignItems="center" spacing={1.5} mb={2.5}>
          <WarningAmberIcon sx={{ color: 'error.main' }} fontSize="small" />
          <Typography variant="h6" fontWeight={800} color="error.main">Danger Zone</Typography>
        </Stack>

        <Stack spacing={2}>
          {/* Test account reset */}
          {isTestAccount && (
            <Stack
              direction="row"
              alignItems="center"
              justifyContent="space-between"
              sx={{
                p: 2, borderRadius: 2,
                border: `1px solid ${alpha(theme.palette.warning.main, 0.2)}`,
                bgcolor: alpha(theme.palette.warning.main, 0.04),
              }}
            >
              <Box>
                <Typography variant="body2" fontWeight={700}>Factory Reset Test Account</Typography>
                <Typography variant="caption" color="text.secondary">
                  Deletes ALL projects, drafts, AI threads, and clears all studio selections. Returns to a completely blank state as if new. You will be redirected to Studio.
                </Typography>
              </Box>
              <Button
                variant="outlined"
                color="warning"
                startIcon={<RefreshIcon />}
                onClick={handleResetTestAccount}
                disabled={resetLoading}
                sx={{ borderRadius: 999, fontWeight: 700, whiteSpace: 'nowrap', ml: 2 }}
              >
                {resetDone ? 'Reset!' : resetLoading ? 'Resetting…' : 'Reset Account'}
              </Button>
            </Stack>
          )}

          {/* Delete account */}
          <Stack
            direction="row"
            alignItems="center"
            justifyContent="space-between"
            sx={{
              p: 2, borderRadius: 2,
              border: `1px solid ${alpha(theme.palette.error.main, 0.2)}`,
            }}
          >
            <Box>
              <Typography variant="body2" fontWeight={700}>Delete Account</Typography>
              <Typography variant="caption" color="text.secondary">
                Permanently remove your account, all projects, and deployed networks. This cannot be undone.
              </Typography>
            </Box>
            <Button
              variant="outlined"
              color="error"
              startIcon={<DeleteForeverIcon />}
              onClick={() => setDeleteDialogOpen(true)}
              sx={{ borderRadius: 999, fontWeight: 700, whiteSpace: 'nowrap', ml: 2 }}
            >
              Delete Account
            </Button>
          </Stack>
        </Stack>

        {resetDone && (
          <Alert severity="success" sx={{ mt: 2, borderRadius: 2 }}>
            Account factory reset complete. Redirecting to Studio...
          </Alert>
        )}
      </Paper>

      {/* Delete Confirm Dialog */}
      <Dialog open={deleteDialogOpen} onClose={() => { setDeleteDialogOpen(false); setDeleteConfirm(''); }} maxWidth="sm" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800, color: 'error.main' }}>
          <Stack direction="row" alignItems="center" spacing={1}>
            <DeleteForeverIcon />
            <span>Delete Account</span>
          </Stack>
        </DialogTitle>
        <DialogContent>
          <Alert severity="error" sx={{ borderRadius: 2, mb: 2.5 }}>
            This will permanently delete your account, all projects, deployed networks, snapshots, and API keys. There is no recovery.
          </Alert>
          <Typography variant="body2" color="text.secondary" mb={1.5}>
            Type <strong>DELETE</strong> to confirm:
          </Typography>
          <TextField
            fullWidth
            size="small"
            placeholder="DELETE"
            value={deleteConfirm}
            onChange={(e) => setDeleteConfirm(e.target.value)}
            error={deleteConfirm.length > 0 && deleteConfirm !== 'DELETE'}
          />
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => { setDeleteDialogOpen(false); setDeleteConfirm(''); }} sx={{ borderRadius: 999 }}>
            Cancel
          </Button>
          <Button
            variant="contained"
            color="error"
            disabled={deleteConfirm !== 'DELETE'}
            startIcon={<DeleteForeverIcon />}
            sx={{ borderRadius: 999, fontWeight: 700 }}
          >
            Delete My Account
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
