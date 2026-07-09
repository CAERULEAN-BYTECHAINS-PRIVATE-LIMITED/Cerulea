'use client';

import { useState, useMemo } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  TextField, InputAdornment, MenuItem, Select, FormControl, InputLabel,
  Avatar, Tooltip, IconButton,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import SearchIcon from '@mui/icons-material/Search';
import FilterListIcon from '@mui/icons-material/FilterList';
import VisibilityIcon from '@mui/icons-material/Visibility';
import BlockIcon from '@mui/icons-material/Block';
import LockResetIcon from '@mui/icons-material/LockReset';
import ManageAccountsIcon from '@mui/icons-material/ManageAccounts';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import AdminPanelSettingsIcon from '@mui/icons-material/AdminPanelSettings';
import PeopleIcon from '@mui/icons-material/People';

type User = {
  id: string;
  name: string;
  email: string;
  plan: 'Developer' | 'Pro' | 'Enterprise';
  status: 'active' | 'suspended' | 'pending';
  lastLogin: string;
  projects: number;
  riskFlags: string[];
  isAdmin?: boolean;
};

const USERS: User[] = [
  {
    id: 'u-001',
    name: 'Aswin Thotapalli',
    email: 'test@cerulea.app',
    plan: 'Pro',
    status: 'active',
    lastLogin: '2026-04-17T09:14:00Z',
    projects: 12,
    riskFlags: [],
    isAdmin: true,
  },
  {
    id: 'u-002',
    name: 'Priya Kapoor',
    email: 'priya.k@blocktech.io',
    plan: 'Enterprise',
    status: 'active',
    lastLogin: '2026-04-17T08:00:00Z',
    projects: 34,
    riskFlags: [],
  },
  {
    id: 'u-003',
    name: 'Jordan Ellis',
    email: 'jordan@defi-stack.com',
    plan: 'Developer',
    status: 'active',
    lastLogin: '2026-04-16T22:00:00Z',
    projects: 4,
    riskFlags: ['high_api_usage'],
  },
  {
    id: 'u-004',
    name: 'Mei Zhang',
    email: 'mei.zhang@quantumledger.net',
    plan: 'Pro',
    status: 'active',
    lastLogin: '2026-04-16T18:44:00Z',
    projects: 9,
    riskFlags: [],
  },
  {
    id: 'u-005',
    name: 'Tobias Muller',
    email: 'tobias@web3labs.de',
    plan: 'Enterprise',
    status: 'active',
    lastLogin: '2026-04-16T14:00:00Z',
    projects: 58,
    riskFlags: ['unusual_geo'],
  },
  {
    id: 'u-006',
    name: 'Amara Diallo',
    email: 'amara.d@chainbridge.africa',
    plan: 'Developer',
    status: 'pending',
    lastLogin: '2026-04-15T10:00:00Z',
    projects: 1,
    riskFlags: [],
  },
  {
    id: 'u-007',
    name: 'Ryo Tanaka',
    email: 'ryo@decentral.jp',
    plan: 'Pro',
    status: 'active',
    lastLogin: '2026-04-14T08:00:00Z',
    projects: 17,
    riskFlags: [],
  },
  {
    id: 'u-008',
    name: 'Marcus Webb',
    email: 'marcus.webb@nullspace.io',
    plan: 'Developer',
    status: 'suspended',
    lastLogin: '2026-04-10T06:00:00Z',
    projects: 2,
    riskFlags: ['failed_payments', 'tos_violation'],
  },
  {
    id: 'u-009',
    name: 'Sofia Reyes',
    email: 'sofia@nomadprotocol.xyz',
    plan: 'Pro',
    status: 'active',
    lastLogin: '2026-04-17T07:30:00Z',
    projects: 6,
    riskFlags: [],
  },
  {
    id: 'u-010',
    name: 'Dev Patel',
    email: 'dev@0xshield.tech',
    plan: 'Enterprise',
    status: 'active',
    lastLogin: '2026-04-17T05:00:00Z',
    projects: 44,
    riskFlags: ['unusual_geo', 'high_api_usage'],
  },
  {
    id: 'u-011',
    name: 'Lena Hoffman',
    email: 'lena.h@openchain.eu',
    plan: 'Developer',
    status: 'pending',
    lastLogin: '2026-04-13T14:00:00Z',
    projects: 0,
    riskFlags: [],
  },
  {
    id: 'u-012',
    name: 'Carlos Mendez',
    email: 'carlos@latamchain.org',
    plan: 'Pro',
    status: 'active',
    lastLogin: '2026-04-16T20:00:00Z',
    projects: 8,
    riskFlags: [],
  },
];

const PLAN_COLOR: Record<string, { bg: string; text: string }> = {
  Developer: { bg: alpha('#448aff', 0.12), text: '#448aff' },
  Pro: { bg: alpha('#ce93d8', 0.12), text: '#ce93d8' },
  Enterprise: { bg: alpha('#ffd54f', 0.12), text: '#ffd54f' },
};

const STATUS_COLOR: Record<string, string> = {
  active: '#4caf50',
  suspended: '#f44336',
  pending: '#ff9800',
};

const RISK_LABEL: Record<string, string> = {
  high_api_usage: 'High API',
  unusual_geo: 'Unusual Geo',
  failed_payments: 'Failed Pay.',
  tos_violation: 'TOS',
};

export default function AdminUsersPage() {
  const theme = useTheme();
  const [search, setSearch] = useState('');
  const [planFilter, setPlanFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState('all');

  const filtered = useMemo(() =>
    USERS.filter((u) => {
      const q = search.toLowerCase();
      const matchSearch = !q ||
        u.name.toLowerCase().includes(q) ||
        u.email.toLowerCase().includes(q);
      const matchPlan = planFilter === 'all' || u.plan === planFilter;
      const matchStatus = statusFilter === 'all' || u.status === statusFilter;
      return matchSearch && matchPlan && matchStatus;
    }),
  [search, planFilter, statusFilter]);

  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
            <PeopleIcon sx={{ color: '#448aff', fontSize: 22 }} />
            <Typography variant="h4" fontWeight={900}>User Management</Typography>
          </Stack>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            All {USERS.length} registered users across the Cerulea platform
          </Typography>
        </Box>
        <Stack direction="row" spacing={1.5}>
          <Chip label={`${USERS.filter((u) => u.status === 'active').length} active`} size="small" sx={{ bgcolor: alpha('#4caf50', 0.1), color: '#4caf50', fontWeight: 700, height: 24 }} />
          <Chip label={`${USERS.filter((u) => u.status === 'suspended').length} suspended`} size="small" sx={{ bgcolor: alpha('#f44336', 0.1), color: '#f44336', fontWeight: 700, height: 24 }} />
          <Chip label={`${USERS.filter((u) => u.status === 'pending').length} pending`} size="small" sx={{ bgcolor: alpha('#ff9800', 0.1), color: '#ff9800', fontWeight: 700, height: 24 }} />
        </Stack>
      </Stack>

      {/* Filters */}
      <Paper
        variant="outlined"
        sx={{ p: 2, mb: 3, borderRadius: 3, display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center', borderColor: 'rgba(255,255,255,0.07)' }}
      >
        <FilterListIcon sx={{ color: 'rgba(255,255,255,0.3)' }} fontSize="small" />
        <TextField
          size="small"
          placeholder="Search name or email..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          InputProps={{
            startAdornment: <InputAdornment position="start"><SearchIcon sx={{ fontSize: 16 }} /></InputAdornment>,
            sx: { fontSize: '0.82rem' },
          }}
          sx={{ flex: 1, minWidth: 240 }}
        />
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Plan</InputLabel>
          <Select value={planFilter} label="Plan" onChange={(e) => setPlanFilter(e.target.value)}>
            <MenuItem value="all">All Plans</MenuItem>
            <MenuItem value="Developer">Developer</MenuItem>
            <MenuItem value="Pro">Pro</MenuItem>
            <MenuItem value="Enterprise">Enterprise</MenuItem>
          </Select>
        </FormControl>
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Status</InputLabel>
          <Select value={statusFilter} label="Status" onChange={(e) => setStatusFilter(e.target.value)}>
            <MenuItem value="all">All</MenuItem>
            <MenuItem value="active">Active</MenuItem>
            <MenuItem value="suspended">Suspended</MenuItem>
            <MenuItem value="pending">Pending</MenuItem>
          </Select>
        </FormControl>
        <Typography variant="caption" sx={{ ml: 'auto', color: 'rgba(255,255,255,0.35)' }}>
          {filtered.length} of {USERS.length} users
        </Typography>
      </Paper>

      {/* Table */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(255,255,255,0.02)' }}>
                {['User', 'Plan', 'Status', 'Projects', 'Last Login', 'Risk Flags', 'Actions'].map((h) => (
                  <TableCell
                    key={h}
                    sx={{ fontWeight: 700, color: 'rgba(255,255,255,0.45)', fontSize: '0.67rem', letterSpacing: 0.7, py: 1.5 }}
                  >
                    {h.toUpperCase()}
                  </TableCell>
                ))}
              </TableRow>
            </TableHead>
            <TableBody>
              {filtered.map((user) => (
                <TableRow
                  key={user.id}
                  sx={{
                    '&:hover': { bgcolor: 'rgba(255,255,255,0.02)' },
                    ...(user.status === 'suspended' && { bgcolor: alpha('#f44336', 0.025) }),
                  }}
                >
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={1.5}>
                      <Avatar
                        sx={{
                          width: 30,
                          height: 30,
                          bgcolor: alpha('#448aff', 0.15),
                          color: '#448aff',
                          fontSize: '0.7rem',
                          fontWeight: 900,
                        }}
                      >
                        {user.name.split(' ').map((n) => n[0]).join('').slice(0, 2).toUpperCase()}
                      </Avatar>
                      <Box>
                        <Stack direction="row" alignItems="center" spacing={0.5}>
                          <Typography variant="body2" fontWeight={700} sx={{ fontSize: '0.82rem' }}>
                            {user.name}
                          </Typography>
                          {user.isAdmin && (
                            <Tooltip title="Super Admin">
                              <AdminPanelSettingsIcon sx={{ fontSize: 13, color: '#f44336' }} />
                            </Tooltip>
                          )}
                        </Stack>
                        <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.35)', fontSize: '0.7rem' }}>
                          {user.email}
                        </Typography>
                      </Box>
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={user.plan}
                      size="small"
                      sx={{
                        height: 20,
                        fontSize: '0.65rem',
                        fontWeight: 700,
                        bgcolor: PLAN_COLOR[user.plan].bg,
                        color: PLAN_COLOR[user.plan].text,
                        border: 'none',
                      }}
                    />
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={0.75}>
                      <Box sx={{ width: 7, height: 7, borderRadius: '50%', bgcolor: STATUS_COLOR[user.status] }} />
                      <Typography variant="body2" sx={{ fontSize: '0.78rem', color: STATUS_COLOR[user.status], fontWeight: 600, textTransform: 'capitalize' }}>
                        {user.status}
                      </Typography>
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.8rem', fontWeight: 700 }}>
                      {user.projects}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.75rem', color: 'rgba(255,255,255,0.45)', fontFamily: 'monospace' }}>
                      {new Date(user.lastLogin).toLocaleString('en-GB', {
                        day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit',
                      })}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" spacing={0.5} flexWrap="wrap">
                      {user.riskFlags.length === 0 ? (
                        <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.25)', fontSize: '0.68rem' }}>None</Typography>
                      ) : (
                        user.riskFlags.map((flag) => (
                          <Chip
                            key={flag}
                            icon={<WarningAmberIcon sx={{ fontSize: '0.6rem !important' }} />}
                            label={RISK_LABEL[flag] || flag}
                            size="small"
                            sx={{
                              height: 17,
                              fontSize: '0.58rem',
                              fontWeight: 700,
                              bgcolor: alpha('#ff9800', 0.12),
                              color: '#ff9800',
                              border: 'none',
                              '& .MuiChip-icon': { color: '#ff9800 !important' },
                            }}
                          />
                        ))
                      )}
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" spacing={0.5}>
                      <Tooltip title="View Profile">
                        <IconButton size="small" sx={{ color: '#448aff', '&:hover': { bgcolor: alpha('#448aff', 0.1) } }}>
                          <VisibilityIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title={user.status === 'suspended' ? 'Unsuspend' : 'Suspend'}>
                        <IconButton
                          size="small"
                          sx={{
                            color: user.status === 'suspended' ? '#4caf50' : '#ff9800',
                            '&:hover': { bgcolor: alpha(user.status === 'suspended' ? '#4caf50' : '#ff9800', 0.1) },
                          }}
                        >
                          <BlockIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Reset Password">
                        <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.4)', '&:hover': { bgcolor: 'rgba(255,255,255,0.06)' } }}>
                          <LockResetIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Impersonate">
                        <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.4)', '&:hover': { bgcolor: 'rgba(255,255,255,0.06)', color: '#ce93d8' } }}>
                          <ManageAccountsIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                    </Stack>
                  </TableCell>
                </TableRow>
              ))}
              {filtered.length === 0 && (
                <TableRow>
                  <TableCell colSpan={7} align="center" sx={{ py: 6 }}>
                    <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.35)' }}>
                      No users match your filters.
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
