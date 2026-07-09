'use client';

import { useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  TextField, IconButton, Tooltip, Divider,
  Dialog, DialogTitle, DialogContent, DialogActions, Alert,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import CheckIcon from '@mui/icons-material/Check';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import WebhookIcon from '@mui/icons-material/Webhook';
import HubIcon from '@mui/icons-material/Hub';
import SettingsIcon from '@mui/icons-material/Settings';

type Integration = {
  id: string;
  name: string;
  description: string;
  category: string;
  enabled: boolean;
  endpoint?: string;
  logoChar: string;
  logoColor: string;
};

const STUB_INTEGRATIONS: Integration[] = [
  {
    id: 'stripe',
    name: 'Stripe',
    description: 'Accept payments and manage subscriptions within your dApp.',
    category: 'Payments',
    enabled: true,
    endpoint: 'https://api.cerulea.app/hooks/stripe/****...****a3f2',
    logoChar: 'S',
    logoColor: '#6772e5',
  },
  {
    id: 'sumsub',
    name: 'Sumsub',
    description: 'KYC/AML identity verification for compliant dApps.',
    category: 'KYC / Identity',
    enabled: true,
    endpoint: 'https://api.cerulea.app/hooks/sumsub/****...****b9d1',
    logoChar: 'K',
    logoColor: '#00b16a',
  },
  {
    id: 'alchemy',
    name: 'Alchemy',
    description: 'Enhanced node infrastructure and RPC access via Alchemy.',
    category: 'Infrastructure',
    enabled: false,
    endpoint: undefined,
    logoChar: 'A',
    logoColor: '#363ff9',
  },
  {
    id: 'ipfs',
    name: 'IPFS / Filecoin',
    description: 'Decentralised file storage for on-chain metadata and assets.',
    category: 'Storage',
    enabled: false,
    endpoint: undefined,
    logoChar: 'F',
    logoColor: '#0090ff',
  },
];

const STUB_WEBHOOK = {
  url: 'https://api.cerulea.app/webhooks/****...****c7e8',
  secret: 'whsec_****...****d4f9',
  events: ['project.deployed', 'snapshot.created', 'node.status_changed', 'proposal.finalized'],
};

const REST_ENDPOINT = 'https://rpc.cerulea.app/v1/****...****e1b2/mainnet';

function CopyButton({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  const handleCopy = () => {
    navigator.clipboard.writeText(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };
  return (
    <Tooltip title={copied ? 'Copied!' : 'Copy'} arrow>
      <IconButton size="small" onClick={handleCopy}>
        {copied
          ? <CheckIcon sx={{ fontSize: 14, color: 'success.main' }} />
          : <ContentCopyIcon sx={{ fontSize: 14 }} />}
      </IconButton>
    </Tooltip>
  );
}

export default function IntegrationsPage() {
  const theme = useTheme();
  const [integrations, setIntegrations] = useState(STUB_INTEGRATIONS);
  const [reConfigId, setReConfigId] = useState<string | null>(null);

  const toggle = (id: string) => {
    setIntegrations((prev) =>
      prev.map((i) => (i.id === id ? { ...i, enabled: !i.enabled } : i))
    );
  };

  const current = integrations.find((i) => i.id === reConfigId);

  return (
    <Box sx={{ p: 4, maxWidth: 1100 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>
            Integrations
          </Typography>
          <Typography variant="body1" color="text.secondary">
            Connect your dApp to third-party services, webhooks, and APIs.
          </Typography>
        </Box>
        <Button
          variant="outlined"
          startIcon={<HubIcon />}
          endIcon={<OpenInNewIcon sx={{ fontSize: 14 }} />}
          sx={{ borderRadius: 999, fontWeight: 700 }}
          onClick={() => {
            const isLocal = window.location.hostname.includes('localhost');
            window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
          }}
        >
          Configure in Studio
        </Button>
      </Stack>

      {/* Integration Cards */}
      <Grid container spacing={2.5} mb={4}>
        {integrations.map((intg) => (
          <Grid xs={12} sm={6} key={intg.id}>
            <Paper
              variant="outlined"
              sx={{
                p: 3, borderRadius: 3, height: '100%',
                transition: 'border-color 0.15s',
                ...(intg.enabled && {
                  borderColor: alpha(theme.palette.success.main, 0.3),
                  background: alpha(theme.palette.success.main, 0.02),
                }),
              }}
            >
              <Stack direction="row" alignItems="flex-start" spacing={2}>
                {/* Logo */}
                <Box
                  sx={{
                    width: 44, height: 44, borderRadius: 2, flexShrink: 0,
                    bgcolor: alpha(intg.logoColor, 0.12),
                    display: 'flex', alignItems: 'center', justifyContent: 'center',
                    fontWeight: 900, fontSize: 18, color: intg.logoColor,
                  }}
                >
                  {intg.logoChar}
                </Box>

                <Box sx={{ flex: 1 }}>
                  <Stack direction="row" alignItems="center" spacing={1} mb={0.5}>
                    <Typography variant="body1" fontWeight={800}>{intg.name}</Typography>
                    <Chip
                      label={intg.category}
                      size="small"
                      variant="outlined"
                      sx={{ fontWeight: 700, fontSize: '0.65rem', height: 18 }}
                    />
                    <Chip
                      label={intg.enabled ? 'Enabled' : 'Disabled'}
                      size="small"
                      color={intg.enabled ? 'success' : 'default'}
                      sx={{ fontWeight: 700, fontSize: '0.65rem', height: 18, ml: 'auto !important' }}
                    />
                  </Stack>
                  <Typography variant="body2" color="text.secondary" mb={1.5}>
                    {intg.description}
                  </Typography>

                  {intg.enabled && intg.endpoint && (
                    <Stack
                      direction="row"
                      alignItems="center"
                      spacing={0.5}
                      sx={{
                        mb: 1.5, px: 1.5, py: 0.75, borderRadius: 2,
                        bgcolor: alpha(theme.palette.background.paper, 0.5),
                        border: `1px solid ${theme.palette.divider}`,
                      }}
                    >
                      <Typography variant="caption" sx={{ fontFamily: 'monospace', flex: 1, color: 'text.secondary' }} noWrap>
                        {intg.endpoint}
                      </Typography>
                      <CopyButton value={intg.endpoint} />
                    </Stack>
                  )}

                  <Stack direction="row" spacing={1}>
                    <Button
                      size="small"
                      variant={intg.enabled ? 'outlined' : 'contained'}
                      color={intg.enabled ? 'error' : 'primary'}
                      onClick={() => toggle(intg.id)}
                      sx={{ borderRadius: 999, fontSize: '0.7rem', fontWeight: 700 }}
                    >
                      {intg.enabled ? 'Disable' : 'Enable'}
                    </Button>
                    <Button
                      size="small"
                      variant="outlined"
                      startIcon={<SettingsIcon sx={{ fontSize: 12 }} />}
                      onClick={() => setReConfigId(intg.id)}
                      sx={{ borderRadius: 999, fontSize: '0.7rem', fontWeight: 700 }}
                    >
                      Re-configure
                    </Button>
                  </Stack>
                </Box>
              </Stack>
            </Paper>
          </Grid>
        ))}
      </Grid>

      {/* REST Endpoint + Webhook */}
      <Grid container spacing={2.5}>
        <Grid xs={12} md={6}>
          <Paper variant="outlined" sx={{ p: 3, borderRadius: 3, height: '100%' }}>
            <Typography variant="h6" fontWeight={800} mb={0.5}>REST API Endpoint</Typography>
            <Typography variant="body2" color="text.secondary" mb={2}>
              Use this URL to connect external services to your deployed network's RPC interface.
            </Typography>
            <Stack
              direction="row"
              alignItems="center"
              spacing={0.5}
              sx={{
                px: 1.5, py: 1, borderRadius: 2,
                bgcolor: alpha(theme.palette.primary.main, 0.04),
                border: `1px solid ${alpha(theme.palette.primary.main, 0.15)}`,
              }}
            >
              <Typography variant="caption" sx={{ fontFamily: 'monospace', flex: 1, color: 'text.secondary' }} noWrap>
                {REST_ENDPOINT}
              </Typography>
              <CopyButton value={REST_ENDPOINT} />
            </Stack>
          </Paper>
        </Grid>

        <Grid xs={12} md={6}>
          <Paper variant="outlined" sx={{ p: 3, borderRadius: 3, height: '100%' }}>
            <Stack direction="row" alignItems="center" gap={1} mb={0.5}>
              <WebhookIcon fontSize="small" sx={{ color: 'secondary.main' }} />
              <Typography variant="h6" fontWeight={800}>Webhook Config</Typography>
            </Stack>
            <Typography variant="body2" color="text.secondary" mb={2}>
              Receive real-time event notifications at your endpoint.
            </Typography>
            <Stack spacing={1.5}>
              <Box>
                <Typography variant="caption" color="text.secondary" fontWeight={700}>WEBHOOK URL</Typography>
                <Stack direction="row" alignItems="center" spacing={0.5}
                  sx={{ mt: 0.5, px: 1.5, py: 0.75, borderRadius: 2, border: `1px solid ${theme.palette.divider}` }}>
                  <Typography variant="caption" sx={{ fontFamily: 'monospace', flex: 1 }} noWrap>{STUB_WEBHOOK.url}</Typography>
                  <CopyButton value={STUB_WEBHOOK.url} />
                </Stack>
              </Box>
              <Box>
                <Typography variant="caption" color="text.secondary" fontWeight={700}>SIGNING SECRET</Typography>
                <Stack direction="row" alignItems="center" spacing={0.5}
                  sx={{ mt: 0.5, px: 1.5, py: 0.75, borderRadius: 2, border: `1px solid ${theme.palette.divider}` }}>
                  <Typography variant="caption" sx={{ fontFamily: 'monospace', flex: 1 }} noWrap>{STUB_WEBHOOK.secret}</Typography>
                  <CopyButton value={STUB_WEBHOOK.secret} />
                </Stack>
              </Box>
              <Box>
                <Typography variant="caption" color="text.secondary" fontWeight={700}>SUBSCRIBED EVENTS</Typography>
                <Stack direction="row" spacing={0.5} flexWrap="wrap" mt={0.5}>
                  {STUB_WEBHOOK.events.map((e) => (
                    <Chip key={e} label={e} size="small" variant="outlined" sx={{ fontWeight: 600, fontSize: '0.65rem', height: 20, my: 0.25 }} />
                  ))}
                </Stack>
              </Box>
            </Stack>
          </Paper>
        </Grid>
      </Grid>

      {/* Re-configure Dialog */}
      <Dialog open={!!reConfigId} onClose={() => setReConfigId(null)} maxWidth="sm" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800 }}>Re-configure {current?.name}</DialogTitle>
        <DialogContent>
          <Alert severity="info" sx={{ borderRadius: 2, mb: 2 }}>
            Full integration configuration is available in the Studio builder (Step 5: Integrations).
          </Alert>
          <Typography variant="body2" color="text.secondary">
            You will be redirected to the Studio where you can update API keys, scopes, and event mappings for <strong>{current?.name}</strong>.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setReConfigId(null)} sx={{ borderRadius: 999 }}>Cancel</Button>
            <Button
              variant="contained"
              endIcon={<OpenInNewIcon sx={{ fontSize: 14 }} />}
              sx={{ borderRadius: 999, fontWeight: 700 }}
              onClick={() => {
                const isLocal = window.location.hostname.includes('localhost');
                window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
              }}
            >
              Open Studio
            </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
