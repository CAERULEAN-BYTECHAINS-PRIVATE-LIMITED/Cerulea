'use client';

import { useState, useMemo } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  TextField, InputAdornment, MenuItem, Select, FormControl, InputLabel,
  Tooltip, IconButton,
} from '@mui/material';
import { alpha } from '@mui/material/styles';
import SearchIcon from '@mui/icons-material/Search';
import FilterListIcon from '@mui/icons-material/FilterList';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import PauseCircleIcon from '@mui/icons-material/PauseCircle';
import DeleteIcon from '@mui/icons-material/Delete';
import NoteAddIcon from '@mui/icons-material/NoteAdd';
import FolderIcon from '@mui/icons-material/Folder';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import AppsIcon from '@mui/icons-material/Apps';
import FlagIcon from '@mui/icons-material/Flag';

type Project = {
  id: string;
  name: string;
  owner: string;
  ownerEmail: string;
  type: 'dApp' | 'blockchain';
  status: 'active' | 'draft' | 'deploying' | 'failed' | 'suspended';
  created: string;
  lastModified: string;
  flags: string[];
};

const PROJECTS: Project[] = [
  {
    id: 'proj-001',
    name: 'VoteDAO',
    owner: 'Aswin Thotapalli',
    ownerEmail: 'test@cerulea.app',
    type: 'dApp',
    status: 'active',
    created: '2026-01-14',
    lastModified: '2026-04-17',
    flags: [],
  },
  {
    id: 'proj-002',
    name: 'CeruleaChain Mainnet',
    owner: 'Aswin Thotapalli',
    ownerEmail: 'test@cerulea.app',
    type: 'blockchain',
    status: 'active',
    created: '2025-11-01',
    lastModified: '2026-04-17',
    flags: [],
  },
  {
    id: 'proj-003',
    name: 'QuantumLedger L2',
    owner: 'Mei Zhang',
    ownerEmail: 'mei.zhang@quantumledger.net',
    type: 'blockchain',
    status: 'active',
    created: '2026-02-10',
    lastModified: '2026-04-15',
    flags: [],
  },
  {
    id: 'proj-004',
    name: 'SupplyChain Tracker',
    owner: 'Priya Kapoor',
    ownerEmail: 'priya.k@blocktech.io',
    type: 'dApp',
    status: 'failed',
    created: '2026-03-05',
    lastModified: '2026-04-16',
    flags: ['deploy_failure'],
  },
  {
    id: 'proj-005',
    name: 'DeFi Aggregator',
    owner: 'Jordan Ellis',
    ownerEmail: 'jordan@defi-stack.com',
    type: 'dApp',
    status: 'deploying',
    created: '2026-04-10',
    lastModified: '2026-04-17',
    flags: [],
  },
  {
    id: 'proj-006',
    name: 'OpenChain Testnet',
    owner: 'Lena Hoffman',
    ownerEmail: 'lena.h@openchain.eu',
    type: 'blockchain',
    status: 'draft',
    created: '2026-04-13',
    lastModified: '2026-04-13',
    flags: [],
  },
  {
    id: 'proj-007',
    name: 'NFT Marketplace',
    owner: 'Ryo Tanaka',
    ownerEmail: 'ryo@decentral.jp',
    type: 'dApp',
    status: 'active',
    created: '2026-01-28',
    lastModified: '2026-04-14',
    flags: [],
  },
  {
    id: 'proj-008',
    name: 'Shield Security dApp',
    owner: 'Dev Patel',
    ownerEmail: 'dev@0xshield.tech',
    type: 'dApp',
    status: 'active',
    created: '2026-02-22',
    lastModified: '2026-04-12',
    flags: ['high_api_usage'],
  },
  {
    id: 'proj-009',
    name: 'LATAMChain',
    owner: 'Carlos Mendez',
    ownerEmail: 'carlos@latamchain.org',
    type: 'blockchain',
    status: 'active',
    created: '2026-03-18',
    lastModified: '2026-04-16',
    flags: [],
  },
  {
    id: 'proj-010',
    name: 'Nomad Protocol',
    owner: 'Sofia Reyes',
    ownerEmail: 'sofia@nomadprotocol.xyz',
    type: 'dApp',
    status: 'draft',
    created: '2026-04-01',
    lastModified: '2026-04-17',
    flags: [],
  },
  {
    id: 'proj-011',
    name: 'Web3Labs Dashboard',
    owner: 'Tobias Muller',
    ownerEmail: 'tobias@web3labs.de',
    type: 'dApp',
    status: 'suspended',
    created: '2025-12-10',
    lastModified: '2026-04-10',
    flags: ['tos_violation', 'unusual_activity'],
  },
  {
    id: 'proj-012',
    name: 'ChainBridge Africa',
    owner: 'Amara Diallo',
    ownerEmail: 'amara.d@chainbridge.africa',
    type: 'blockchain',
    status: 'draft',
    created: '2026-04-15',
    lastModified: '2026-04-15',
    flags: [],
  },
];

const STATUS_COLOR: Record<string, string> = {
  active: '#4caf50',
  draft: 'rgba(255,255,255,0.4)',
  deploying: '#448aff',
  failed: '#f44336',
  suspended: '#ff9800',
};

const FLAG_LABEL: Record<string, string> = {
  deploy_failure: 'Deploy Fail',
  high_api_usage: 'High API',
  tos_violation: 'TOS',
  unusual_activity: 'Unusual',
};

export default function AdminProjectsPage() {
  const [search, setSearch] = useState('');
  const [typeFilter, setTypeFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState('all');

  const filtered = useMemo(() =>
    PROJECTS.filter((p) => {
      const q = search.toLowerCase();
      const matchSearch = !q ||
        p.name.toLowerCase().includes(q) ||
        p.owner.toLowerCase().includes(q) ||
        p.ownerEmail.toLowerCase().includes(q);
      const matchType = typeFilter === 'all' || p.type === typeFilter;
      const matchStatus = statusFilter === 'all' || p.status === statusFilter;
      return matchSearch && matchType && matchStatus;
    }),
  [search, typeFilter, statusFilter]);

  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
            <FolderIcon sx={{ color: '#80deea', fontSize: 22 }} />
            <Typography variant="h4" fontWeight={900}>Project Management</Typography>
          </Stack>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            All {PROJECTS.length} projects across all users on the platform
          </Typography>
        </Box>
        <Stack direction="row" spacing={1.5}>
          <Chip label={`${PROJECTS.filter((p) => p.type === 'dApp').length} dApps`} size="small" icon={<AppsIcon sx={{ fontSize: 12 }} />} sx={{ bgcolor: alpha('#448aff', 0.1), color: '#448aff', fontWeight: 700, height: 24 }} />
          <Chip label={`${PROJECTS.filter((p) => p.type === 'blockchain').length} Chains`} size="small" icon={<HexagonOutlinedIcon sx={{ fontSize: 12 }} />} sx={{ bgcolor: alpha('#ce93d8', 0.1), color: '#ce93d8', fontWeight: 700, height: 24 }} />
          <Chip label={`${PROJECTS.filter((p) => p.flags.length > 0).length} flagged`} size="small" icon={<FlagIcon sx={{ fontSize: 12 }} />} sx={{ bgcolor: alpha('#f44336', 0.1), color: '#f44336', fontWeight: 700, height: 24 }} />
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
          placeholder="Search project name or owner..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          InputProps={{
            startAdornment: <InputAdornment position="start"><SearchIcon sx={{ fontSize: 16 }} /></InputAdornment>,
            sx: { fontSize: '0.82rem' },
          }}
          sx={{ flex: 1, minWidth: 240 }}
        />
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Type</InputLabel>
          <Select value={typeFilter} label="Type" onChange={(e) => setTypeFilter(e.target.value)}>
            <MenuItem value="all">All Types</MenuItem>
            <MenuItem value="dApp">dApp</MenuItem>
            <MenuItem value="blockchain">Blockchain</MenuItem>
          </Select>
        </FormControl>
        <FormControl size="small" sx={{ minWidth: 150 }}>
          <InputLabel>Status</InputLabel>
          <Select value={statusFilter} label="Status" onChange={(e) => setStatusFilter(e.target.value)}>
            <MenuItem value="all">All</MenuItem>
            <MenuItem value="active">Active</MenuItem>
            <MenuItem value="draft">Draft</MenuItem>
            <MenuItem value="deploying">Deploying</MenuItem>
            <MenuItem value="failed">Failed</MenuItem>
            <MenuItem value="suspended">Suspended</MenuItem>
          </Select>
        </FormControl>
        <Typography variant="caption" sx={{ ml: 'auto', color: 'rgba(255,255,255,0.35)' }}>
          {filtered.length} of {PROJECTS.length} projects
        </Typography>
      </Paper>

      {/* Table */}
      <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
        <TableContainer>
          <Table size="small">
            <TableHead>
              <TableRow sx={{ bgcolor: 'rgba(255,255,255,0.02)' }}>
                {['Name', 'Owner', 'Type', 'Status', 'Created', 'Last Modified', 'Flags', 'Actions'].map((h) => (
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
              {filtered.map((proj) => (
                <TableRow
                  key={proj.id}
                  sx={{
                    '&:hover': { bgcolor: 'rgba(255,255,255,0.02)' },
                    ...(proj.status === 'failed' && { bgcolor: alpha('#f44336', 0.025) }),
                    ...(proj.status === 'suspended' && { bgcolor: alpha('#ff9800', 0.025) }),
                  }}
                >
                  <TableCell>
                    <Typography variant="body2" fontWeight={700} sx={{ fontSize: '0.82rem' }}>
                      {proj.name}
                    </Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontFamily: 'monospace', fontSize: '0.67rem' }}>
                      {proj.id}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.8rem' }}>{proj.owner}</Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.68rem' }}>{proj.ownerEmail}</Typography>
                  </TableCell>
                  <TableCell>
                    <Chip
                      label={proj.type}
                      size="small"
                      icon={proj.type === 'blockchain' ? <HexagonOutlinedIcon sx={{ fontSize: '0.65rem !important' }} /> : <AppsIcon sx={{ fontSize: '0.65rem !important' }} />}
                      sx={{
                        height: 20,
                        fontSize: '0.63rem',
                        fontWeight: 700,
                        bgcolor: proj.type === 'blockchain' ? alpha('#ce93d8', 0.1) : alpha('#448aff', 0.1),
                        color: proj.type === 'blockchain' ? '#ce93d8' : '#448aff',
                        border: 'none',
                      }}
                    />
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" alignItems="center" spacing={0.75}>
                      <Box sx={{ width: 7, height: 7, borderRadius: '50%', bgcolor: STATUS_COLOR[proj.status] }} />
                      <Typography variant="body2" sx={{ fontSize: '0.78rem', color: STATUS_COLOR[proj.status], fontWeight: 600, textTransform: 'capitalize' }}>
                        {proj.status}
                      </Typography>
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.75rem', color: 'rgba(255,255,255,0.45)', fontFamily: 'monospace' }}>
                      {proj.created}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Typography variant="body2" sx={{ fontSize: '0.75rem', color: 'rgba(255,255,255,0.45)', fontFamily: 'monospace' }}>
                      {proj.lastModified}
                    </Typography>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" spacing={0.5} flexWrap="wrap">
                      {proj.flags.length === 0 ? (
                        <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.2)', fontSize: '0.68rem' }}>None</Typography>
                      ) : (
                        proj.flags.map((flag) => (
                          <Chip
                            key={flag}
                            label={FLAG_LABEL[flag] || flag}
                            size="small"
                            sx={{
                              height: 17,
                              fontSize: '0.58rem',
                              fontWeight: 700,
                              bgcolor: alpha('#f44336', 0.1),
                              color: '#f44336',
                              border: 'none',
                            }}
                          />
                        ))
                      )}
                    </Stack>
                  </TableCell>
                  <TableCell>
                    <Stack direction="row" spacing={0.25}>
                      <Tooltip title="Open in Studio">
                        <IconButton size="small" sx={{ color: '#448aff', '&:hover': { bgcolor: alpha('#448aff', 0.1) } }}>
                          <OpenInNewIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Suspend Project">
                        <IconButton size="small" sx={{ color: '#ff9800', '&:hover': { bgcolor: alpha('#ff9800', 0.1) } }}>
                          <PauseCircleIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Delete Project">
                        <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.35)', '&:hover': { color: '#f44336', bgcolor: alpha('#f44336', 0.1) } }}>
                          <DeleteIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Add Note">
                        <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.35)', '&:hover': { bgcolor: 'rgba(255,255,255,0.06)' } }}>
                          <NoteAddIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                    </Stack>
                  </TableCell>
                </TableRow>
              ))}
              {filtered.length === 0 && (
                <TableRow>
                  <TableCell colSpan={8} align="center" sx={{ py: 6 }}>
                    <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.35)' }}>
                      No projects match your filters.
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
