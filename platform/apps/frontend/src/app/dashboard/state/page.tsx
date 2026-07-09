'use client';

import { useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  MenuItem, Select, FormControl, InputLabel,
  Dialog, DialogTitle, DialogContent, DialogActions,
  TextField, Alert, Divider,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import AddIcon from '@mui/icons-material/Add';
import RestoreIcon from '@mui/icons-material/Restore';
import ScheduleIcon from '@mui/icons-material/Schedule';
import StorageIcon from '@mui/icons-material/Storage';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import PendingIcon from '@mui/icons-material/Pending';
import SaveIcon from '@mui/icons-material/Save';

type Snapshot = {
  id: string;
  network: string;
  blockHeight: number;
  timestamp: string;
  size: string;
  status: 'ready' | 'creating' | 'failed';
};

const STUB_SNAPSHOTS: Snapshot[] = [
  { id: 'snap-0041', network: 'CeruleaChain Mainnet', blockHeight: 4_182_034, timestamp: '2026-04-17T06:00:00Z', size: '2.4 GB', status: 'ready' },
  { id: 'snap-0040', network: 'CeruleaChain Mainnet', blockHeight: 4_170_000, timestamp: '2026-04-16T06:00:00Z', size: '2.3 GB', status: 'ready' },
  { id: 'snap-0039', network: 'CeruleaChain Mainnet', blockHeight: 4_158_100, timestamp: '2026-04-15T06:00:00Z', size: '2.3 GB', status: 'ready' },
  { id: 'snap-0012', network: 'VoteApp Devnet',       blockHeight: 98_100,    timestamp: '2026-04-17T06:00:00Z', size: '180 MB', status: 'ready' },
  { id: 'snap-0011', network: 'VoteApp Devnet',       blockHeight: 95_200,    timestamp: '2026-04-16T06:00:00Z', size: '174 MB', status: 'ready' },
  { id: 'snap-live', network: 'CeruleaChain Mainnet', blockHeight: 4_182_401, timestamp: 'N/A',                    size: 'N/A',      status: 'creating' },
];

const STATUS_COLOR: Record<string, 'success' | 'warning' | 'error'> = {
  ready: 'success',
  creating: 'warning',
  failed: 'error',
};

const STATUS_ICON: Record<string, React.ReactNode> = {
  ready: <CheckCircleIcon sx={{ fontSize: 14 }} />,
  creating: <PendingIcon sx={{ fontSize: 14 }} />,
};

export default function StatePage() {
  const theme = useTheme();
  const [snapshots, setSnapshots] = useState<Snapshot[]>(STUB_SNAPSHOTS);
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const [restoreId, setRestoreId] = useState<string | null>(null);
  const [interval, setInterval] = useState('24h');
  const [retention, setRetention] = useState('7');
  const [selectedNetwork, setSelectedNetwork] = useState('CeruleaChain Mainnet');

  const handleCreate = () => {
    const latest = snapshots.filter((s) => s.network === selectedNetwork && s.status === 'ready');
    const latestHeight = latest.length > 0
      ? Math.max(...latest.map((s) => s.blockHeight)) + Math.floor(Math.random() * 1000 + 100)
      : 1000;
    const newSnap: Snapshot = {
      id: `snap-${Date.now()}`,
      network: selectedNetwork,
      blockHeight: latestHeight,
      timestamp: new Date().toISOString(),
      size: 'N/A',
      status: 'creating',
    };
    setSnapshots((prev) => [newSnap, ...prev]);
    setCreateDialogOpen(false);
  };

  return (
    <Box sx={{ p: 4, maxWidth: 1100 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>
            State Snapshots
          </Typography>
          <Typography variant="body1" color="text.secondary">
            Create, manage, and restore blockchain state snapshots for disaster recovery.
          </Typography>
        </Box>
        <Button
          variant="contained"
          startIcon={<AddIcon />}
          onClick={() => setCreateDialogOpen(true)}
          sx={{ borderRadius: 999, fontWeight: 700 }}
        >
          Create Snapshot
        </Button>
      </Stack>

      {/* Auto-snapshot Schedule */}
      <Paper
        variant="outlined"
        sx={{
          p: 3, mb: 4, borderRadius: 3,
          background: alpha(theme.palette.primary.main, 0.03),
          borderColor: alpha(theme.palette.primary.main, 0.15),
        }}
      >
        <Stack direction="row" alignItems="center" gap={1.5} mb={2}>
          <ScheduleIcon sx={{ color: 'primary.main' }} fontSize="small" />
          <Typography variant="h6" fontWeight={800}>Auto-Snapshot Schedule</Typography>
        </Stack>
        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2.5} alignItems={{ sm: 'flex-end' }}>
          <FormControl size="small" sx={{ minWidth: 180 }}>
            <InputLabel>Snapshot Interval</InputLabel>
            <Select value={interval} label="Snapshot Interval" onChange={(e) => setInterval(e.target.value)}>
              <MenuItem value="6h">Every 6 hours</MenuItem>
              <MenuItem value="12h">Every 12 hours</MenuItem>
              <MenuItem value="24h">Every 24 hours</MenuItem>
              <MenuItem value="7d">Weekly</MenuItem>
            </Select>
          </FormControl>
          <FormControl size="small" sx={{ minWidth: 180 }}>
            <InputLabel>Retention Count</InputLabel>
            <Select value={retention} label="Retention Count" onChange={(e) => setRetention(e.target.value)}>
              <MenuItem value="3">Keep last 3</MenuItem>
              <MenuItem value="7">Keep last 7</MenuItem>
              <MenuItem value="14">Keep last 14</MenuItem>
              <MenuItem value="30">Keep last 30</MenuItem>
            </Select>
          </FormControl>
          <Button variant="outlined" startIcon={<SaveIcon />} sx={{ borderRadius: 999, fontWeight: 700 }}>
            Save Schedule
          </Button>
          <Box sx={{ flex: 1 }} />
          <Stack direction="row" spacing={1} alignItems="center">
            <Box sx={{ width: 8, height: 8, borderRadius: '50%', bgcolor: 'success.main' }} />
            <Typography variant="caption" color="text.secondary">
              Next snapshot in <strong>4h 12m</strong>
            </Typography>
          </Stack>
        </Stack>
      </Paper>

      {/* Snapshots Table */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
        <Stack
          direction="row"
          alignItems="center"
          gap={1.5}
          sx={{ px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}` }}
        >
          <StorageIcon fontSize="small" sx={{ color: 'text.disabled' }} />
          <Typography variant="h6" fontWeight={800}>Snapshots</Typography>
          <Chip
            label={snapshots.filter((s) => s.status === 'ready').length}
            size="small"
            color="success"
            variant="outlined"
            sx={{ fontWeight: 700 }}
          />
        </Stack>
        <TableContainer>
          <Table>
            <TableHead>
              <TableRow>
                {['Snapshot ID', 'Network', 'Block Height', 'Timestamp', 'Size', 'Status', 'Actions'].map((h) => (
                  <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                    {h.toUpperCase()}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {snapshots.map((snap) => (
                <TableRow key={snap.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: 'monospace', fontWeight: 700 }}>{snap.id}</Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary">{snap.network}</Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" fontWeight={600}>
                      {snap.blockHeight > 0 ? `#${snap.blockHeight.toLocaleString()}` : 'N/A'}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" sx={{ fontSize: '0.8rem' }}>
                      {snap.timestamp !== 'N/A'
                        ? new Date(snap.timestamp).toLocaleString('en-GB', {
                          day: '2-digit', month: 'short', year: 'numeric', hour: '2-digit', minute: '2-digit',
                        })
                        : 'N/A'}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2">{snap.size}</Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={snap.status}
                      size="small"
                      color={STATUS_COLOR[snap.status]}
                      icon={STATUS_ICON[snap.status] as any}
                      sx={{ fontWeight: 700, fontSize: '0.7rem', textTransform: 'capitalize' }}
                    />
                  </TableCell>
                  <TableCell>
                    <Button
                      size="small"
                      variant="outlined"
                      startIcon={<RestoreIcon sx={{ fontSize: 14 }} />}
                      disabled={snap.status !== 'ready'}
                      onClick={() => setRestoreId(snap.id)}
                      sx={{ borderRadius: 999, fontSize: '0.7rem' }}
                    >
                      Restore
                    </Button>
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>

      {/* Create Snapshot Dialog */}
      <Dialog open={createDialogOpen} onClose={() => setCreateDialogOpen(false)} maxWidth="sm" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800 }}>Create Snapshot</DialogTitle>
        <DialogContent>
          <Stack spacing={2.5} pt={1}>
            <FormControl fullWidth size="small">
              <InputLabel>Network</InputLabel>
              <Select value={selectedNetwork} label="Network" onChange={(e) => setSelectedNetwork(e.target.value)}>
                <MenuItem value="CeruleaChain Mainnet">CeruleaChain Mainnet</MenuItem>
                <MenuItem value="VoteApp Devnet">VoteApp Devnet</MenuItem>
              </Select>
            </FormControl>
            <Typography variant="caption" color="text.secondary">
              A snapshot will be taken at the current block height. This may take a few minutes to complete.
            </Typography>
          </Stack>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setCreateDialogOpen(false)} sx={{ borderRadius: 999 }}>Cancel</Button>
          <Button variant="contained" onClick={handleCreate} sx={{ borderRadius: 999, fontWeight: 700 }}>
            Create
          </Button>
        </DialogActions>
      </Dialog>

      {/* Restore Confirm Dialog */}
      <Dialog open={!!restoreId} onClose={() => setRestoreId(null)} maxWidth="xs" fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}>
        <DialogTitle sx={{ fontWeight: 800 }}>Restore Snapshot?</DialogTitle>
        <DialogContent>
          <Alert severity="warning" sx={{ borderRadius: 2, mb: 2 }}>
            Restoring will replace the current chain state. This action cannot be undone.
          </Alert>
          <Typography variant="body2" color="text.secondary">
            Are you sure you want to restore snapshot <strong>{restoreId}</strong>?
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setRestoreId(null)} sx={{ borderRadius: 999 }}>Cancel</Button>
          <Button variant="contained" color="warning" onClick={() => setRestoreId(null)} sx={{ borderRadius: 999, fontWeight: 700 }}>
            Restore
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
