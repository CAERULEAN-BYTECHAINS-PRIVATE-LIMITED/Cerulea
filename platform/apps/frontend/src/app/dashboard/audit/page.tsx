'use client';

import { useState, useMemo } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  TextField, InputAdornment, MenuItem, Select, FormControl, InputLabel,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import SearchIcon from '@mui/icons-material/Search';
import DownloadIcon from '@mui/icons-material/Download';
import FilterListIcon from '@mui/icons-material/FilterList';

type LogEntry = {
  id: string;
  timestamp: string;
  actor: string;
  action: string;
  resource: string;
  category: string;
  status: 'success' | 'failure' | 'warning';
};

const STUB_LOGS: LogEntry[] = [
  { id: 'log-001', timestamp: '2026-04-17T09:14:32Z', actor: 'aswin@cerulea.app', action: 'API_KEY_CREATED', resource: 'Production Backend', category: 'keys', status: 'success' },
  { id: 'log-002', timestamp: '2026-04-17T08:52:11Z', actor: 'system', action: 'SNAPSHOT_CREATED', resource: 'CeruleaChain Mainnet / #4182034', category: 'snapshot', status: 'success' },
  { id: 'log-003', timestamp: '2026-04-17T07:30:00Z', actor: 'aswin@cerulea.app', action: 'PROJECT_DEPLOYED', resource: 'VoteApp Devnet', category: 'deployment', status: 'success' },
  { id: 'log-004', timestamp: '2026-04-16T22:18:44Z', actor: 'ci-pipeline', action: 'DEPLOY_FAILED', resource: 'SupplyChain Staging', category: 'deployment', status: 'failure' },
  { id: 'log-005', timestamp: '2026-04-16T18:05:00Z', actor: 'aswin@cerulea.app', action: 'PROPOSAL_VOTED', resource: 'prop-002', category: 'governance', status: 'success' },
  { id: 'log-006', timestamp: '2026-04-16T15:43:21Z', actor: 'aswin@cerulea.app', action: 'NODE_KEY_ROTATED', resource: 'node-val-02', category: 'nodes', status: 'success' },
  { id: 'log-007', timestamp: '2026-04-16T11:22:08Z', actor: 'system', action: 'LOGIN_ATTEMPT_BLOCKED', resource: 'auth', category: 'auth', status: 'failure' },
  { id: 'log-008', timestamp: '2026-04-15T20:00:00Z', actor: 'aswin@cerulea.app', action: 'INTEGRATION_UPDATED', resource: 'Alchemy / mainnet', category: 'integrations', status: 'success' },
  { id: 'log-009', timestamp: '2026-04-15T14:33:50Z', actor: 'aswin@cerulea.app', action: 'API_KEY_REVOKED', resource: 'Old Test Key', category: 'keys', status: 'warning' },
  { id: 'log-010', timestamp: '2026-04-15T10:10:10Z', actor: 'system', action: 'AUTO_SNAPSHOT_SCHEDULED', resource: 'CeruleaChain Mainnet', category: 'snapshot', status: 'success' },
];

const CATEGORIES = ['all', 'keys', 'snapshot', 'deployment', 'governance', 'nodes', 'auth', 'integrations'];

const STATUS_COLOR: Record<string, 'success' | 'error' | 'warning'> = {
  success: 'success',
  failure: 'error',
  warning: 'warning',
};

const CATEGORY_COLOR: Record<string, string> = {
  keys: 'primary',
  snapshot: 'info',
  deployment: 'success',
  governance: 'secondary',
  nodes: 'warning',
  auth: 'error',
  integrations: 'default',
};

function exportCSV(logs: LogEntry[]) {
  const header = 'Timestamp,Actor,Action,Resource,Category,Status\n';
  const rows = logs
    .map((l) => `"${l.timestamp}","${l.actor}","${l.action}","${l.resource}","${l.category}","${l.status}"`)
    .join('\n');
  const blob = new Blob([header + rows], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `cerulea-audit-${new Date().toISOString().slice(0, 10)}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

export default function AuditPage() {
  const theme = useTheme();
  const [search, setSearch] = useState('');
  const [category, setCategory] = useState('all');
  const [status, setStatus] = useState('all');

  const filtered = useMemo(() =>
    STUB_LOGS.filter((l) => {
      const matchSearch =
        l.actor.toLowerCase().includes(search.toLowerCase()) ||
        l.action.toLowerCase().includes(search.toLowerCase()) ||
        l.resource.toLowerCase().includes(search.toLowerCase());
      const matchCat = category === 'all' || l.category === category;
      const matchStatus = status === 'all' || l.status === status;
      return matchSearch && matchCat && matchStatus;
    }),
  [search, category, status]);

  return (
    <Box sx={{ p: 4, maxWidth: 1200 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>
            Audit Logs
          </Typography>
          <Typography variant="body1" color="text.secondary">
            Track all actions, deployments, and system events across your account.
          </Typography>
        </Box>
        <Button
          variant="outlined"
          startIcon={<DownloadIcon />}
          onClick={() => exportCSV(filtered)}
          sx={{ borderRadius: 999, fontWeight: 700 }}
        >
          Export CSV
        </Button>
      </Stack>

      {/* Category Chips */}
      <Stack direction="row" spacing={1} flexWrap="wrap" mb={3}>
        {CATEGORIES.map((c) => (
          <Chip
            key={c}
            label={c === 'all' ? 'All Categories' : c}
            onClick={() => setCategory(c)}
            color={category === c ? (CATEGORY_COLOR[c] as any) || 'primary' : 'default'}
            variant={category === c ? 'filled' : 'outlined'}
            size="small"
            sx={{ fontWeight: 700, fontSize: '0.7rem', textTransform: 'capitalize', cursor: 'pointer' }}
          />
        ))}
      </Stack>

      {/* Filters */}
      <Paper
        variant="outlined"
        sx={{ p: 2, mb: 3, borderRadius: 3, display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}
      >
        <FilterListIcon sx={{ color: 'text.disabled' }} fontSize="small" />
        <TextField
          size="small"
          placeholder="Search actor, action, resource…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon fontSize="small" /></InputAdornment> }}
          sx={{ flex: 1, minWidth: 240 }}
        />
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Status</InputLabel>
          <Select value={status} label="Status" onChange={(e) => setStatus(e.target.value)}>
            <MenuItem value="all">All</MenuItem>
            <MenuItem value="success">Success</MenuItem>
            <MenuItem value="failure">Failure</MenuItem>
            <MenuItem value="warning">Warning</MenuItem>
          </Select>
        </FormControl>
        <Typography variant="caption" color="text.secondary" sx={{ ml: 'auto' }}>
          {filtered.length} entries
        </Typography>
      </Paper>

      {/* Log Table */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow>
                {['Timestamp', 'Actor', 'Action', 'Resource', 'Category', 'Status'].map((h) => (
                  <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                    {h.toUpperCase()}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {filtered.map((log) => (
                <TableRow
                  key={log.id}
                  sx={{
                    '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) },
                    ...(log.status === 'failure' && { bgcolor: alpha(theme.palette.error.main, 0.03) }),
                  }}
                >
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: 'monospace', color: 'text.secondary', fontSize: '0.75rem' }}>
                      {new Date(log.timestamp).toLocaleString('en-GB', {
                        day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit',
                      })}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" fontWeight={600} noWrap sx={{ maxWidth: 160 }}>
                      {log.actor}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: 'monospace', fontWeight: 700 }}>
                      {log.action}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" color="text.secondary" noWrap sx={{ maxWidth: 200 }}>
                      {log.resource}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={log.category}
                      size="small"
                      color={(CATEGORY_COLOR[log.category] as any) || 'default'}
                      variant="outlined"
                      sx={{ fontWeight: 700, fontSize: '0.65rem', height: 20, textTransform: 'capitalize' }}
                    />
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={log.status}
                      size="small"
                      color={STATUS_COLOR[log.status]}
                      sx={{ fontWeight: 700, fontSize: '0.65rem', height: 20, textTransform: 'capitalize' }}
                    />
                  </TableCell>
                </TableRow>
              ))}
              {filtered.length === 0 && (
                <TableRow>
                  <TableCell colSpan={6} align="center" sx={{ py: 6 }}>
                    <Typography variant="body2" color="text.secondary">No log entries match your filters.</Typography>
                  </TableCell>
                </TableRow>
              )}
            </TableBody>
          </Table>
        </TableContainer>
      </Paper>
    </Box>
  );
}
