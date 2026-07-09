'use client';

import { useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Dialog, DialogTitle, DialogContent, DialogActions,
  TextField, IconButton, Tooltip, Alert, Divider,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import AddIcon from '@mui/icons-material/Add';
import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import KeyIcon from '@mui/icons-material/Key';
import LockIcon from '@mui/icons-material/Lock';
import CheckIcon from '@mui/icons-material/Check';

type ApiKey = {
  id: string;
  name: string;
  scopes: string[];
  created: string;
  lastUsed: string;
  preview: string;
};

type ValidatorKey = {
  id: string;
  address: string;
  type: string;
  network: string;
  added: string;
};

const STUB_API_KEYS: ApiKey[] = [
  { id: 'ak-1', name: 'Production Backend', scopes: ['read', 'write'], created: '2025-11-01', lastUsed: '2 hours ago', preview: 'ck_live_****...****a3f2' },
  { id: 'ak-2', name: 'Analytics Service', scopes: ['read'], created: '2025-12-15', lastUsed: 'Yesterday', preview: 'ck_live_****...****9b1d' },
  { id: 'ak-3', name: 'CI/CD Pipeline', scopes: ['deploy', 'read'], created: '2026-01-08', lastUsed: '3 days ago', preview: 'ck_live_****...****c7e8' },
];

const STUB_VALIDATOR_KEYS: ValidatorKey[] = [
  { id: 'vk-1', address: '0x3d5a****...****fe29', type: 'BLS-12381', network: 'CeruleaChain Mainnet', added: '2025-11-01' },
  { id: 'vk-2', address: '0x8f2b****...****aa41', type: 'BLS-12381', network: 'CeruleaChain Mainnet', added: '2025-11-01' },
  { id: 'vk-3', address: '0x1c9e****...****3d80', type: 'Ed25519', network: 'VoteApp Devnet', added: '2025-12-20' },
];

const SCOPE_COLOR: Record<string, 'primary' | 'success' | 'warning' | 'error'> = {
  read: 'primary',
  write: 'warning',
  deploy: 'success',
  admin: 'error',
};

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
        {copied ? <CheckIcon sx={{ fontSize: 14, color: 'success.main' }} /> : <ContentCopyIcon sx={{ fontSize: 14 }} />}
      </IconButton>
    </Tooltip>
  );
}

export default function KeysPage() {
  const theme = useTheme();
  const [apiKeys, setApiKeys] = useState<ApiKey[]>(STUB_API_KEYS);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [newKeyName, setNewKeyName] = useState('');
  const [generatedKey, setGeneratedKey] = useState<string | null>(null);
  const [revokeId, setRevokeId] = useState<string | null>(null);

  const handleGenerate = () => {
    const key = `ck_live_${Math.random().toString(36).slice(2, 10)}...${Math.random().toString(36).slice(2, 6)}`;
    const newKey: ApiKey = {
      id: `ak-${Date.now()}`,
      name: newKeyName || 'New API Key',
      scopes: ['read'],
      created: new Date().toISOString().slice(0, 10),
      lastUsed: 'Never',
      preview: `ck_live_****...****${Math.random().toString(36).slice(2, 6)}`,
    };
    setApiKeys((prev) => [...prev, newKey]);
    setGeneratedKey(key);
    setNewKeyName('');
  };

  const handleRevoke = (id: string) => {
    setApiKeys((prev) => prev.filter((k) => k.id !== id));
    setRevokeId(null);
  };

  return (
    <Box sx={{ p: 4, maxWidth: 1100 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>
            Keys & Access
          </Typography>
          <Typography variant="body1" color="text.secondary">
            Manage API keys, validator signing keys, and access credentials.
          </Typography>
        </Box>
        <Button
          variant="contained"
          startIcon={<AddIcon />}
          onClick={() => { setGeneratedKey(null); setDialogOpen(true); }}
          sx={{ borderRadius: 999, fontWeight: 700 }}
        >
          Generate API Key
        </Button>
      </Stack>

      {/* API Keys */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', mb: 4 }}>
        <Box sx={{ px: 3, pt: 2.5, pb: 2, borderBottom: `1px solid ${theme.palette.divider}` }}>
          <Stack direction="row" alignItems="center" gap={1} mb={0.5}>
            <KeyIcon fontSize="small" sx={{ color: 'primary.main' }} />
            <Typography variant="h6" fontWeight={800}>API Keys</Typography>
            <Chip label={apiKeys.length} size="small" sx={{ fontWeight: 700, ml: 0.5 }} />
          </Stack>
          <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 640 }}>
            API keys authenticate your backend services, CI/CD pipelines, and third-party integrations with the Cerulea platform. Each key can be scoped to specific permissions (read, write, deploy). Treat API keys like passwords: store them in environment variables, never commit them to source control, and rotate them regularly.
          </Typography>
        </Box>
        <TableContainer>
          <Table>
            <TableHead>
              <TableRow>
                {['Name', 'Key', 'Scopes', 'Created', 'Last Used', ''].map((h) => (
                  <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                    {h.toUpperCase()}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {apiKeys.map((key) => (
                <TableRow key={key.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                  <TableCell>
                    <Typography variant="body2" fontWeight={700}>{key.name}</Typography>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={0.5}>
                      <Typography variant="body2" sx={{ fontFamily: 'monospace', color: 'text.secondary' }}>
                        {key.preview}
                      </Typography>
                      <CopyButton value={key.preview} />
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" spacing={0.5} flexWrap="wrap">
                      {key.scopes.map((s) => (
                        <Chip
                          key={s}
                          label={s}
                          size="small"
                          color={SCOPE_COLOR[s] || 'default'}
                          variant="outlined"
                          sx={{ fontWeight: 700, fontSize: '0.65rem', height: 18 }}
                        />
                      ))}
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">{key.created}</Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">{key.lastUsed}</Typography>
                  </TableCell>
                  <TableCell align="right">
                    <Tooltip title="Revoke key" arrow>
                      <IconButton
                        size="small"
                        color="error"
                        onClick={() => setRevokeId(key.id)}
                      >
                        <DeleteOutlineIcon fontSize="small" />
                      </IconButton>
                    </Tooltip>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>

      {/* Validator Keys */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
        <Box sx={{ px: 3, pt: 2.5, pb: 2, borderBottom: `1px solid ${theme.palette.divider}` }}>
          <Stack direction="row" alignItems="center" gap={1} mb={0.5}>
            <LockIcon fontSize="small" sx={{ color: 'secondary.main' }} />
            <Typography variant="h6" fontWeight={800}>Validator Keys</Typography>
            <Chip label={STUB_VALIDATOR_KEYS.length} size="small" sx={{ fontWeight: 700, ml: 0.5 }} />
          </Stack>
          <Typography variant="body2" color="text.secondary" sx={{ maxWidth: 640 }}>
            Validator keys are cryptographic signing keys assigned to nodes that participate in your network's consensus process. These keys sign blocks and attestations to prove the node's identity and vote weight. BLS-12381 keys are standard for PoS validators; Ed25519 keys are used in faster finality chains. Never share private validator keys; compromise leads to slashing penalties or network forks.
          </Typography>
        </Box>
        <TableContainer>
          <Table>
            <TableHead>
              <TableRow>
                {['Address', 'Type', 'Network', 'Added', ''].map((h) => (
                  <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                    {h.toUpperCase()}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {STUB_VALIDATOR_KEYS.map((vk) => (
                <TableRow key={vk.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={0.5}>
                      <Typography variant="body2" sx={{ fontFamily: 'monospace', fontWeight: 600 }}>
                        {vk.address}
                      </Typography>
                      <CopyButton value={vk.address} />
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={vk.type}
                      size="small"
                      variant="outlined"
                      color="secondary"
                      sx={{ fontWeight: 700, fontSize: '0.7rem' }}
                    />
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">{vk.network}</Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">{vk.added}</Typography>
                  </TableCell>
                  <TableCell align="right">
                    <Button size="small" variant="outlined" sx={{ borderRadius: 999, fontSize: '0.7rem' }}>
                      Export
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>

      {/* Generate Key Dialog */}
      <Dialog open={dialogOpen} onClose={() => setDialogOpen(false)} maxWidth="sm" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800 }}>Generate New API Key</DialogTitle>
        <DialogContent>
          {!generatedKey ? (
            <Stack spacing={2.5} pt={1}>
              <TextField
                label="Key Name"
                placeholder="e.g. Production Backend"
                value={newKeyName}
                onChange={(e) => setNewKeyName(e.target.value)}
                fullWidth
                size="small"
              />
              <Typography variant="caption" color="text.secondary">
                The key will be shown only once. Store it securely after generation.
              </Typography>
            </Stack>
          ) : (
            <Stack spacing={2} pt={1}>
              <Alert severity="warning" sx={{ borderRadius: 2 }}>
                Copy this key now; it will not be shown again.
              </Alert>
              <Paper
                variant="outlined"
                sx={{
                  p: 2, borderRadius: 2,
                  bgcolor: alpha(theme.palette.success.main, 0.05),
                  borderColor: alpha(theme.palette.success.main, 0.3),
                }}
              >
                <Stack direction="row" alignItems="center" justifyContent="space-between">
                  <Typography variant="body2" sx={{ fontFamily: 'monospace', wordBreak: 'break-all' }}>
                    {generatedKey}
                  </Typography>
                  <CopyButton value={generatedKey} />
                </Stack>
              </Paper>
            </Stack>
          )}
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setDialogOpen(false)} sx={{ borderRadius: 999 }}>
            {generatedKey ? 'Done' : 'Cancel'}
          </Button>
          {!generatedKey && (
            <Button
              variant="contained"
              onClick={handleGenerate}
              disabled={!newKeyName.trim()}
              sx={{ borderRadius: 999, fontWeight: 700 }}
            >
              Generate
            </Button>
          )}
        </DialogActions>
      </Dialog>

      {/* Revoke Confirm Dialog */}
      <Dialog open={!!revokeId} onClose={() => setRevokeId(null)} maxWidth="xs" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800 }}>Revoke API Key?</DialogTitle>
        <DialogContent>
          <Typography variant="body2" color="text.secondary">
            This action is irreversible. Any service using this key will lose access immediately.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setRevokeId(null)} sx={{ borderRadius: 999 }}>Cancel</Button>
          <Button
            variant="contained"
            color="error"
            onClick={() => revokeId && handleRevoke(revokeId)}
            sx={{ borderRadius: 999, fontWeight: 700 }}
          >
            Revoke
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
