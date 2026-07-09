'use client';

import { useState, useMemo } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  TextField, InputAdornment, MenuItem, Select, FormControl, InputLabel,
} from '@mui/material';
import { alpha } from '@mui/material/styles';
import SearchIcon from '@mui/icons-material/Search';
import FilterListIcon from '@mui/icons-material/FilterList';
import DownloadIcon from '@mui/icons-material/Download';
import AssignmentIcon from '@mui/icons-material/Assignment';

type AuditLog = {
  id: string;
  timestamp: string;
  actor: string;
  actorType: 'user' | 'system' | 'admin' | 'ci';
  action: string;
  resource: string;
  resourceType: string;
  status: 'success' | 'failure' | 'warning';
  ipAddress: string;
};

const AUDIT_LOGS: AuditLog[] = [
  { id: 'adm-001', timestamp: '2026-04-17T09:14:32Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'ADMIN_LOGIN', resource: 'admin.cerulea.app', resourceType: 'auth', status: 'success', ipAddress: '203.0.113.5' },
  { id: 'adm-002', timestamp: '2026-04-17T09:01:04Z', actor: 'system', actorType: 'system', action: 'IP_BLOCKED', resource: '45.152.66.14', resourceType: 'security', status: 'success', ipAddress: 'internal' },
  { id: 'adm-003', timestamp: '2026-04-17T08:52:11Z', actor: 'system', actorType: 'system', action: 'AUTO_SNAPSHOT', resource: 'CeruleaChain / #4182034', resourceType: 'blockchain', status: 'success', ipAddress: 'internal' },
  { id: 'adm-004', timestamp: '2026-04-17T08:30:00Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'USER_SUSPENDED', resource: 'marcus.webb@nullspace.io', resourceType: 'user', status: 'success', ipAddress: '203.0.113.5' },
  { id: 'adm-005', timestamp: '2026-04-17T07:44:00Z', actor: 'jordan@defi-stack.com', actorType: 'user', action: 'API_KEY_CREATED', resource: 'Production DeFi Key', resourceType: 'api_key', status: 'success', ipAddress: '104.18.22.14' },
  { id: 'adm-006', timestamp: '2026-04-17T07:30:00Z', actor: 'priya.k@blocktech.io', actorType: 'user', action: 'PROJECT_DEPLOYED', resource: 'SupplyChain Staging', resourceType: 'project', status: 'failure', ipAddress: '52.14.88.200' },
  { id: 'adm-007', timestamp: '2026-04-17T06:00:00Z', actor: 'system', actorType: 'system', action: 'QUOTA_WARNING', resource: 'Anthropic API', resourceType: 'billing', status: 'warning', ipAddress: 'internal' },
  { id: 'adm-008', timestamp: '2026-04-16T22:18:44Z', actor: 'ci-pipeline', actorType: 'ci', action: 'DEPLOY_FAILED', resource: 'SupplyChain Staging v0.3.1', resourceType: 'project', status: 'failure', ipAddress: '54.220.14.8' },
  { id: 'adm-009', timestamp: '2026-04-16T22:10:00Z', actor: 'marcus.webb@nullspace.io', actorType: 'user', action: 'UNAUTHORIZED_ACCESS', resource: '/api/admin/*', resourceType: 'security', status: 'failure', ipAddress: '185.220.101.8' },
  { id: 'adm-010', timestamp: '2026-04-16T20:00:00Z', actor: 'carlos@latamchain.org', actorType: 'user', action: 'VALIDATOR_ADDED', resource: 'LATAMChain / val-12', resourceType: 'blockchain', status: 'success', ipAddress: '190.216.33.14' },
  { id: 'adm-011', timestamp: '2026-04-16T18:05:00Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'PROPOSAL_CREATED', resource: 'Governance / prop-003', resourceType: 'governance', status: 'success', ipAddress: '203.0.113.5' },
  { id: 'adm-012', timestamp: '2026-04-16T15:43:21Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'NODE_KEY_ROTATED', resource: 'node-val-02', resourceType: 'blockchain', status: 'success', ipAddress: '203.0.113.5' },
  { id: 'adm-013', timestamp: '2026-04-16T14:22:00Z', actor: 'system', actorType: 'system', action: 'IP_BLOCKED', resource: '103.42.168.10', resourceType: 'security', status: 'success', ipAddress: 'internal' },
  { id: 'adm-014', timestamp: '2026-04-16T12:00:00Z', actor: 'system', actorType: 'system', action: 'STRIPE_WEBHOOK', resource: 'sub_1QXyzABC / upgraded', resourceType: 'billing', status: 'success', ipAddress: 'internal' },
  { id: 'adm-015', timestamp: '2026-04-16T11:22:08Z', actor: 'system', actorType: 'system', action: 'LOGIN_ATTEMPT_BLOCKED', resource: 'auth / 45.152.66.14', resourceType: 'auth', status: 'failure', ipAddress: '45.152.66.14' },
  { id: 'adm-016', timestamp: '2026-04-16T10:00:00Z', actor: 'dev@0xshield.tech', actorType: 'user', action: 'PROJECT_DEPLOYED', resource: 'Shield Security dApp', resourceType: 'project', status: 'success', ipAddress: '52.86.120.10' },
  { id: 'adm-017', timestamp: '2026-04-15T20:00:00Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'INTEGRATION_UPDATED', resource: 'Alchemy / mainnet', resourceType: 'integration', status: 'success', ipAddress: '203.0.113.5' },
  { id: 'adm-018', timestamp: '2026-04-15T14:33:50Z', actor: 'test@cerulea.app', actorType: 'admin', action: 'API_KEY_REVOKED', resource: 'Old Test Key', resourceType: 'api_key', status: 'warning', ipAddress: '203.0.113.5' },
];

const RESOURCE_TYPES = ['all', 'auth', 'security', 'blockchain', 'project', 'user', 'api_key', 'billing', 'governance', 'integration'];
const STATUS_OPTIONS = ['all', 'success', 'failure', 'warning'];

const STATUS_COLOR: Record<string, string> = {
  success: '#4caf50',
  failure: '#f44336',
  warning: '#ff9800',
};

const ACTOR_TYPE_COLOR: Record<string, { bg: string; text: string }> = {
  user: { bg: alpha('#448aff', 0.1), text: '#448aff' },
  admin: { bg: alpha('#f44336', 0.1), text: '#f44336' },
  system: { bg: alpha('#ce93d8', 0.1), text: '#ce93d8' },
  ci: { bg: alpha('#ffd54f', 0.1), text: '#ffd54f' },
};

function exportCSV(logs: AuditLog[]) {
  const header = 'Timestamp,Actor,Actor Type,Action,Resource,Resource Type,Status,IP Address\n';
  const rows = logs
    .map((l) => `"${l.timestamp}","${l.actor}","${l.actorType}","${l.action}","${l.resource}","${l.resourceType}","${l.status}","${l.ipAddress}"`)
    .join('\n');
  const blob = new Blob([header + rows], { type: 'text/csv' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `cerulea-admin-audit-${new Date().toISOString().slice(0, 10)}.csv`;
  a.click();
  URL.revokeObjectURL(url);
}

export default function AdminAuditPage() {
  const [search, setSearch] = useState('');
  const [resourceType, setResourceType] = useState('all');
  const [status, setStatus] = useState('all');
  const [timeRange, setTimeRange] = useState('7d');

  const filtered = useMemo(() =>
    AUDIT_LOGS.filter((l) => {
      const q = search.toLowerCase();
      const matchSearch = !q ||
        l.actor.toLowerCase().includes(q) ||
        l.action.toLowerCase().includes(q) ||
        l.resource.toLowerCase().includes(q);
      const matchType = resourceType === 'all' || l.resourceType === resourceType;
      const matchStatus = status === 'all' || l.status === status;
      return matchSearch && matchType && matchStatus;
    }),
  [search, resourceType, status]);

  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
            <AssignmentIcon sx={{ color: '#80deea', fontSize: 22 }} />
            <Typography variant="h4" fontWeight={900}>Platform Audit Logs</Typography>
          </Stack>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            Complete immutable audit trail of all platform-level events and admin actions
          </Typography>
        </Box>
        <Button
          variant="outlined"
          startIcon={<DownloadIcon />}
          onClick={() => exportCSV(filtered)}
          sx={{ borderRadius: 999, fontWeight: 700, flexShrink: 0 }}
        >
          Export CSV
        </Button>
      </Stack>

      {/* Resource Type Chips */}
      <Stack direction="row" spacing={0.75} flexWrap="wrap" mb={3} useFlexGap>
        {RESOURCE_TYPES.map((rt) => (
          <Chip
            key={rt}
            label={rt === 'all' ? 'All Types' : rt.replace('_', ' ')}
            onClick={() => setResourceType(rt)}
            variant={resourceType === rt ? 'filled' : 'outlined'}
            size="small"
            sx={{
              fontWeight: 700,
              fontSize: '0.67rem',
              textTransform: 'capitalize',
              cursor: 'pointer',
              height: 22,
              ...(resourceType === rt
                ? { bgcolor: alpha('#448aff', 0.15), color: '#448aff', border: `1px solid ${alpha('#448aff', 0.3)}` }
                : { borderColor: 'rgba(255,255,255,0.1)', color: 'rgba(255,255,255,0.45)' }),
            }}
          />
        ))}
      </Stack>

      {/* Filters */}
      <Paper
        variant="outlined"
        sx={{ p: 2, mb: 3, borderRadius: 3, display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center', borderColor: 'rgba(255,255,255,0.07)' }}
      >
        <FilterListIcon sx={{ color: 'rgba(255,255,255,0.3)' }} fontSize="small" />
        <TextField
          size="small"
          placeholder="Search actor, action, resource..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          InputProps={{
            startAdornment: <InputAdornment position="start"><SearchIcon sx={{ fontSize: 16 }} /></InputAdornment>,
            sx: { fontSize: '0.82rem' },
          }}
          sx={{ flex: 1, minWidth: 240 }}
        />
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Status</InputLabel>
          <Select value={status} label="Status" onChange={(e) => setStatus(e.target.value)}>
            {STATUS_OPTIONS.map((s) => (
              <MenuItem key={s} value={s}>{s === 'all' ? 'All' : s.charAt(0).toUpperCase() + s.slice(1)}</MenuItem>
            ))}
          </Select>
        </FormControl>
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Time Range</InputLabel>
          <Select value={timeRange} label="Time Range" onChange={(e) => setTimeRange(e.target.value)}>
            <MenuItem value="1d">Last 24 hours</MenuItem>
            <MenuItem value="7d">Last 7 days</MenuItem>
            <MenuItem value="30d">Last 30 days</MenuItem>
            <MenuItem value="90d">Last 90 days</MenuItem>
          </Select>
        </FormControl>
        <Typography variant="caption" sx={{ ml: 'auto', color: 'rgba(255,255,255,0.35)' }}>
          {filtered.length} entries
        </Typography>
      </Paper>

      {/* Table */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(255,255,255,0.02)' }}>
                {['Timestamp', 'Actor', 'Type', 'Action', 'Resource', 'IP', 'Status'].map((h) => (
                  <TableCell
                    key={h}
                    sx={{ fontWeight: 700, color: 'rgba(255,255,255,0.45)', fontSize: '0.65rem', letterSpacing: 0.7, py: 1.5 }}
                  >
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
                    '&:hover': { bgcolor: 'rgba(255,255,255,0.02)' },
                    ...(log.status === 'failure' && { bgcolor: alpha('#f44336', 0.025) }),
                  }}
                >
                  <TableCell>
                    <Typography variant="caption" sx={{ fontFamily: 'monospace', color: 'rgba(255,255,255,0.4)', fontSize: '0.72rem' }}>
                      {new Date(log.timestamp).toLocaleString('en-GB', {
                        day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit', second: '2-digit',
                      })}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.78rem', fontWeight: 600 }} noWrap>
                      {log.actor}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={log.actorType}
                      size="small"
                      sx={{
                        height: 18,
                        fontSize: '0.6rem',
                        fontWeight: 700,
                        bgcolor: ACTOR_TYPE_COLOR[log.actorType].bg,
                        color: ACTOR_TYPE_COLOR[log.actorType].text,
                        border: 'none',
                        textTransform: 'capitalize',
                      }}
                    />
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.76rem', fontWeight: 700 }}>
                      {log.action}
                    </Typography>
                  </TableCell>
                  <TableCell sx={{ maxWidth: 220 }}>
                    <Typography variant="body2" sx={{ fontSize: '0.76rem', color: 'rgba(255,255,255,0.55)' }} noWrap>
                      {log.resource}
                    </Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.63rem', textTransform: 'capitalize' }}>
                      {log.resourceType.replace('_', ' ')}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="caption" sx={{ fontFamily: 'monospace', color: 'rgba(255,255,255,0.35)', fontSize: '0.7rem' }}>
                      {log.ipAddress}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={0.75}>
                      <Box sx={{ width: 6, height: 6, borderRadius: '50%', bgcolor: STATUS_COLOR[log.status] }} />
                      <Typography variant="caption" sx={{ fontSize: '0.7rem', color: STATUS_COLOR[log.status], fontWeight: 700, textTransform: 'capitalize' }}>
                        {log.status}
                      </Typography>
                    </Stack>
                  </TableCell>
                </TableRow>
              ))}
              {filtered.length === 0 && (
                <TableRow>
                  <TableCell colSpan={7} align="center" sx={{ py: 6 }}>
                    <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.35)' }}>
                      No log entries match your filters.
                    </Typography>
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
