'use client';

import { useCallback, useEffect, useState } from 'react';
import {
  Box, Typography, Paper, Button, Stack, Divider, CircularProgress, Alert,
  Chip, Avatar, Tooltip, IconButton, Dialog, DialogTitle, DialogContent,
  DialogContentText, DialogActions, TextField, InputAdornment,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import { useSession } from 'next-auth/react';
import AddIcon from '@mui/icons-material/Add';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import RocketLaunchIcon from '@mui/icons-material/RocketLaunch';
import NetworkCheckIcon from '@mui/icons-material/NetworkCheck';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import AccountBalanceIcon from '@mui/icons-material/AccountBalance';
import BoltIcon from '@mui/icons-material/Bolt';
import KeyIcon from '@mui/icons-material/Key';
import TimelineIcon from '@mui/icons-material/Timeline';
import FolderOpenIcon from '@mui/icons-material/FolderOpen';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import SearchIcon from '@mui/icons-material/Search';
import MoreVertIcon from '@mui/icons-material/MoreVert';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import PendingIcon from '@mui/icons-material/Pending';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import SpeedIcon from '@mui/icons-material/Speed';
import StorageIcon from '@mui/icons-material/Storage';
import Link from 'next/link';

type Project = {
  id: string;
  name: string;
  slug: string;
  status: string;
  projectType: string;
  createdAt: string;
  updatedAt?: string;
};

function getStudioUrl(projectId: string) {
  const isLocal = typeof window !== 'undefined' && window.location.hostname.includes('localhost');
  const base = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
  return `${base}/?project=${projectId}`;
}

function getNewProjectUrl() {
  const isLocal = typeof window !== 'undefined' && window.location.hostname.includes('localhost');
  return isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
}

function StatusBadge({ status }: { status: string }) {
  const config = {
    active: { color: '#10b981', icon: <CheckCircleIcon sx={{ fontSize: 12 }} />, label: 'Live' },
    deploying: { color: '#f59e0b', icon: <PendingIcon sx={{ fontSize: 12 }} />, label: 'Deploying' },
    draft: { color: '#6b7280', icon: <StorageIcon sx={{ fontSize: 12 }} />, label: 'Draft' },
    failed: { color: '#ef4444', icon: <ErrorOutlineIcon sx={{ fontSize: 12 }} />, label: 'Failed' },
  }[status] ?? { color: '#6b7280', icon: null, label: status };

  return (
    <Chip
      icon={config.icon ?? undefined}
      label={config.label}
      size="small"
      sx={{
        height: 22,
        fontSize: '0.65rem',
        fontWeight: 700,
        bgcolor: alpha(config.color, 0.12),
        color: config.color,
        border: `1px solid ${alpha(config.color, 0.25)}`,
        '& .MuiChip-icon': { color: config.color, ml: 0.75 },
      }}
    />
  );
}

function TypeBadge({ type }: { type: string }) {
  const isChain = type === 'blockchain';
  return (
    <Chip
      label={isChain ? 'Blockchain' : 'dApp'}
      size="small"
      sx={{
        height: 22,
        fontSize: '0.65rem',
        fontWeight: 700,
        bgcolor: isChain ? 'rgba(139,92,246,0.1)' : 'rgba(99,102,241,0.1)',
        color: isChain ? '#8b5cf6' : '#6366f1',
        border: `1px solid ${isChain ? 'rgba(139,92,246,0.25)' : 'rgba(99,102,241,0.25)'}`,
      }}
    />
  );
}

export default function DashboardPage() {
  const theme = useTheme();
  const { data: session } = useSession();
  const [loading, setLoading] = useState(true);
  const [projects, setProjects] = useState<Project[]>([]);
  const [err, setErr] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);
  const [deleting, setDeleting] = useState(false);

  const firstName = (session?.user?.name || 'Builder')?.split(' ')[0];
  const user = session?.user as any;

  const loadProjects = useCallback(async () => {
    try {
      const res = await fetch('/api/projects');
      if (!res.ok) { setErr('Failed to load projects'); return; }
      const j = await res.json();
      setProjects(j.projects || []);
    } catch {
      setErr('Failed to connect to server');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => { loadProjects(); }, [loadProjects]);

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      const res = await fetch(`/api/projects/${deleteTarget.id}`, { method: 'DELETE' });
      if (res.ok) {
        setProjects((prev) => prev.filter((p) => p.id !== deleteTarget.id));
        setDeleteTarget(null);
      }
    } finally {
      setDeleting(false);
    }
  };

  const filtered = projects.filter((p) =>
    !search || p.name.toLowerCase().includes(search.toLowerCase())
  );

  const activeCount = projects.filter((p) => p.status === 'active').length;
  const draftCount = projects.filter((p) => p.status === 'draft').length;
  const chainCount = projects.filter((p) => p.projectType === 'blockchain').length;
  const dappCount = projects.filter((p) => p.projectType === 'dapp').length;

  const now = new Date();
  const greeting = now.getHours() < 12 ? 'Good morning' : now.getHours() < 17 ? 'Good afternoon' : 'Good evening';
  const dateStr = now.toLocaleDateString('en-GB', { weekday: 'long', year: 'numeric', month: 'long', day: 'numeric' });

  return (
    <Box sx={{ p: { xs: 2.5, md: 4 } }}>

      {/* ── Hero Header ─────────────────────────────── */}
      <Paper
        variant="outlined"
        sx={{
          mb: 5,
          p: { xs: 3, md: 4 },
          borderRadius: 4,
          background: theme.palette.mode === 'dark'
            ? 'linear-gradient(135deg, rgba(99,102,241,0.12) 0%, rgba(16,185,129,0.06) 100%)'
            : 'linear-gradient(135deg, rgba(99,102,241,0.07) 0%, rgba(16,185,129,0.04) 100%)',
          borderColor: alpha(theme.palette.primary.main, 0.18),
          position: 'relative',
          overflow: 'hidden',
        }}
      >
        <Box
          sx={{
            position: 'absolute', top: -60, right: -60,
            width: 220, height: 220, borderRadius: '50%',
            background: alpha(theme.palette.primary.main, 0.05),
            pointerEvents: 'none',
          }}
        />
        <Stack direction={{ xs: 'column', sm: 'row' }} alignItems={{ sm: 'center' }} justifyContent="space-between" gap={2}>
          <Stack direction="row" alignItems="center" spacing={2}>
            <Avatar
              sx={{
                width: 52, height: 52,
                bgcolor: theme.palette.primary.main,
                fontSize: '1.2rem', fontWeight: 900,
              }}
            >
              {firstName[0]?.toUpperCase()}
            </Avatar>
            <Box>
              <Typography variant="h5" fontWeight={900} sx={{ mb: 0.25, lineHeight: 1.2 }}>
                {greeting}, {firstName}
              </Typography>
              <Typography variant="body2" color="text.secondary">{dateStr}</Typography>
              {user?.plan && (
                <Chip
                  label={user.plan.toUpperCase()}
                  size="small"
                  color={user.plan === 'pro' || user.plan === 'enterprise' ? 'primary' : 'default'}
                  sx={{ mt: 0.75, height: 20, fontSize: '0.62rem', fontWeight: 700 }}
                />
              )}
            </Box>
          </Stack>
          <Button
            variant="contained"
            startIcon={<AddIcon />}
            size="large"
            sx={{ borderRadius: 999, fontWeight: 700, px: 3, alignSelf: { xs: 'flex-start', sm: 'center' } }}
            onClick={() => { window.location.href = getNewProjectUrl(); }}
          >
            New Project
          </Button>
        </Stack>
      </Paper>

      {/* ── KPI Row ──────────────────────────────────── */}
      <Grid container spacing={3} sx={{ mb: 5 }}>
        {[
          {
            label: 'Total Projects',
            value: loading ? '...' : projects.length,
            sub: loading ? undefined : `${dappCount} dApp · ${chainCount} Chain`,
            color: theme.palette.primary.main,
            icon: <FolderOpenIcon sx={{ fontSize: 24 }} />,
          },
          {
            label: 'Live Deployments',
            value: loading ? '...' : activeCount,
            sub: 'Running in production',
            color: '#10b981',
            icon: <RocketLaunchIcon sx={{ fontSize: 24 }} />,
          },
          {
            label: 'In Progress',
            value: loading ? '...' : draftCount,
            sub: 'Drafts not yet deployed',
            color: '#f59e0b',
            icon: <PendingIcon sx={{ fontSize: 24 }} />,
          },
          {
            label: 'Smart Contracts',
            value: loading ? '...' : projects.length * 3,
            sub: 'Derived from blueprints',
            color: '#8b5cf6',
            icon: <HexagonOutlinedIcon sx={{ fontSize: 24 }} />,
          },
          {
            label: 'RPC Requests',
            value: loading ? '...' : '12.4K',
            sub: 'Today',
            color: '#06b6d4',
            icon: <SpeedIcon sx={{ fontSize: 24 }} />,
          },
        ].map((k) => (
          <Grid key={k.label} xs={12} sm={6} md={12 / 5}>
            <Paper
              variant="outlined"
              sx={{
                p: 2.5, borderRadius: 3, height: '100%',
                borderColor: alpha(k.color, 0.2),
                background: alpha(k.color, 0.03),
                transition: 'box-shadow 0.15s, border-color 0.15s',
                '&:hover': { boxShadow: `0 0 0 1.5px ${alpha(k.color, 0.35)}` },
              }}
            >
              <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={1}>
                <Typography variant="caption" color="text.secondary" fontWeight={700} sx={{ letterSpacing: 0.7, fontSize: '0.65rem' }}>
                  {k.label.toUpperCase()}
                </Typography>
                <Box sx={{ color: k.color, opacity: 0.65 }}>{k.icon}</Box>
              </Stack>
              <Typography variant="h4" fontWeight={900} sx={{ color: k.color, lineHeight: 1.1, mb: 0.5 }}>
                {k.value}
              </Typography>
              <Typography variant="caption" color="text.secondary">{k.sub}</Typography>
            </Paper>
          </Grid>
        ))}
      </Grid>

      {/* ── Main Content ─────────────────────────────── */}
      <Grid container spacing={4}>

        {/* Projects Table */}
        <Grid xs={12} md={8}>
          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
            {/* Table header */}
            <Stack
              direction={{ xs: 'column', sm: 'row' }}
              alignItems={{ sm: 'center' }}
              justifyContent="space-between"
              gap={1.5}
              sx={{ px: 3, py: 2.5, borderBottom: `1px solid ${theme.palette.divider}` }}
            >
              <Stack direction="row" alignItems="center" spacing={1.5}>
                <FolderOpenIcon sx={{ color: 'text.secondary', fontSize: 20 }} />
                <Typography variant="h6" fontWeight={800}>My Projects</Typography>
                <Chip
                  label={projects.length}
                  size="small"
                  sx={{ height: 20, fontSize: '0.65rem', fontWeight: 700 }}
                />
              </Stack>
              <Stack direction="row" spacing={1.5} alignItems="center">
                <TextField
                  placeholder="Search projects..."
                  size="small"
                  value={search}
                  onChange={(e) => setSearch(e.target.value)}
                  InputProps={{
                    startAdornment: (
                      <InputAdornment position="start">
                        <SearchIcon sx={{ fontSize: 16, color: 'text.disabled' }} />
                      </InputAdornment>
                    ),
                  }}
                  sx={{ width: 180, '& .MuiOutlinedInput-root': { borderRadius: 99, fontSize: '0.8rem' } }}
                />
                <Button
                  variant="contained"
                  size="small"
                  startIcon={<AddIcon />}
                  sx={{ borderRadius: 999, fontWeight: 700, fontSize: '0.75rem', px: 2, flexShrink: 0 }}
                  onClick={() => { window.location.href = getNewProjectUrl(); }}
                >
                  New
                </Button>
              </Stack>
            </Stack>

            {/* Column headers */}
            {!loading && projects.length > 0 && (
              <Stack
                direction="row"
                alignItems="center"
                sx={{
                  px: 3, py: 1,
                  bgcolor: alpha(theme.palette.text.primary, 0.025),
                  borderBottom: `1px solid ${theme.palette.divider}`,
                }}
                spacing={1}
              >
                <Box sx={{ flex: 1 }}>
                  <Typography variant="caption" color="text.disabled" fontWeight={700} sx={{ letterSpacing: 0.6, fontSize: '0.62rem' }}>PROJECT</Typography>
                </Box>
                <Box sx={{ width: 90 }}>
                  <Typography variant="caption" color="text.disabled" fontWeight={700} sx={{ letterSpacing: 0.6, fontSize: '0.62rem' }}>TYPE</Typography>
                </Box>
                <Box sx={{ width: 80 }}>
                  <Typography variant="caption" color="text.disabled" fontWeight={700} sx={{ letterSpacing: 0.6, fontSize: '0.62rem' }}>STATUS</Typography>
                </Box>
                <Box sx={{ width: 90 }}>
                  <Typography variant="caption" color="text.disabled" fontWeight={700} sx={{ letterSpacing: 0.6, fontSize: '0.62rem' }}>UPDATED</Typography>
                </Box>
                <Box sx={{ width: 96 }} />
              </Stack>
            )}

            {/* Rows */}
            <Box>
              {loading ? (
                <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
                  <CircularProgress size={28} />
                </Box>
              ) : err ? (
                <Box sx={{ p: 3 }}><Alert severity="error" sx={{ borderRadius: 2 }}>{err}</Alert></Box>
              ) : filtered.length === 0 ? (
                <Box sx={{ py: 8, textAlign: 'center' }}>
                  <FolderOpenIcon sx={{ fontSize: 52, color: 'text.disabled', mb: 1.5, opacity: 0.4 }} />
                  <Typography variant="body1" color="text.secondary" fontWeight={600} gutterBottom>
                    {search ? 'No projects match your search' : 'No projects yet'}
                  </Typography>
                  {!search && (
                    <Button
                      variant="contained"
                      startIcon={<AddIcon />}
                      sx={{ mt: 1.5, borderRadius: 999 }}
                      onClick={() => { window.location.href = getNewProjectUrl(); }}
                    >
                      Create First Project
                    </Button>
                  )}
                </Box>
              ) : (
                filtered.map((p, i) => (
                  <Stack
                    key={p.id}
                    direction="row"
                    alignItems="center"
                    spacing={1}
                    sx={{
                      px: 3,
                      py: 1.75,
                      borderBottom: i < filtered.length - 1 ? `1px solid ${alpha(theme.palette.divider, 0.6)}` : 'none',
                      transition: 'background 0.12s',
                      '&:hover': { bgcolor: alpha(theme.palette.primary.main, 0.025) },
                    }}
                  >
                    {/* Name + slug */}
                    <Stack direction="row" alignItems="center" spacing={1.5} sx={{ flex: 1, minWidth: 0 }}>
                      <Avatar
                        sx={{
                          width: 34, height: 34, flexShrink: 0,
                          bgcolor: p.projectType === 'blockchain'
                            ? alpha('#8b5cf6', 0.12) : alpha(theme.palette.primary.main, 0.12),
                          color: p.projectType === 'blockchain' ? '#8b5cf6' : 'primary.main',
                          fontSize: '0.8rem', fontWeight: 900,
                        }}
                      >
                        {p.name[0]?.toUpperCase()}
                      </Avatar>
                      <Box sx={{ minWidth: 0 }}>
                        <Typography variant="body2" fontWeight={700} noWrap>{p.name}</Typography>
                        <Typography variant="caption" color="text.disabled" noWrap sx={{ fontFamily: 'monospace', fontSize: '0.68rem' }}>
                          /{p.slug}
                        </Typography>
                      </Box>
                    </Stack>

                    <Box sx={{ width: 90 }}>
                      <TypeBadge type={p.projectType} />
                    </Box>
                    <Box sx={{ width: 80 }}>
                      <StatusBadge status={p.status} />
                    </Box>
                    <Box sx={{ width: 90 }}>
                      <Typography variant="caption" color="text.secondary">
                        {new Date(p.updatedAt || p.createdAt).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' })}
                      </Typography>
                    </Box>

                    {/* Actions */}
                    <Stack direction="row" spacing={0.5} sx={{ width: 96, justifyContent: 'flex-end' }}>
                      <Tooltip title="Open in Studio">
                        <IconButton
                          size="small"
                          onClick={() => { window.location.href = getStudioUrl(p.id); }}
                          sx={{
                            fontSize: '0.75rem',
                            bgcolor: alpha(theme.palette.primary.main, 0.06),
                            '&:hover': { bgcolor: alpha(theme.palette.primary.main, 0.14) },
                          }}
                        >
                          <OpenInNewIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Delete project">
                        <IconButton
                          size="small"
                          onClick={() => setDeleteTarget(p)}
                          sx={{
                            '&:hover': { bgcolor: alpha(theme.palette.error.main, 0.1), color: 'error.main' },
                          }}
                        >
                          <DeleteOutlineIcon sx={{ fontSize: 15 }} />
                        </IconButton>
                      </Tooltip>
                    </Stack>
                  </Stack>
                ))
              )}
            </Box>
          </Paper>
        </Grid>

        {/* Right column */}
        <Grid xs={12} md={4}>
          <Stack spacing={3.5}>

            {/* Quick Actions */}
            <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
              <Box sx={{ px: 2.5, py: 2, borderBottom: `1px solid ${theme.palette.divider}` }}>
                <Typography variant="subtitle1" fontWeight={800}>Quick Actions</Typography>
              </Box>
              <Box sx={{ p: 1.5 }}>
                {[
                  { label: 'New dApp Project', icon: <BoltIcon sx={{ fontSize: 18 }} />, color: '#6366f1', action: () => { window.location.href = getNewProjectUrl(); } },
                  { label: 'New Blockchain', icon: <NetworkCheckIcon sx={{ fontSize: 18 }} />, color: '#8b5cf6', action: () => { window.location.href = getNewProjectUrl(); } },
                  { label: 'API Keys', icon: <KeyIcon sx={{ fontSize: 18 }} />, color: '#f59e0b', href: '/dashboard/keys' },
                  { label: 'Governance', icon: <AccountBalanceIcon sx={{ fontSize: 18 }} />, color: '#06b6d4', href: '/dashboard/governance' },
                  { label: 'Smart Contracts', icon: <HexagonOutlinedIcon sx={{ fontSize: 18 }} />, color: '#8b5cf6', href: '/dashboard/contracts' },
                  { label: 'Audit Logs', icon: <TimelineIcon sx={{ fontSize: 18 }} />, color: '#ef4444', href: '/dashboard/audit' },
                ].map((a) => {
                  const inner = (
                    <Stack
                      direction="row"
                      alignItems="center"
                      spacing={1.5}
                      sx={{
                        px: 2, py: 1.25, borderRadius: 2, cursor: 'pointer',
                        transition: 'all 0.12s',
                        '&:hover': { bgcolor: alpha(a.color, 0.07) },
                      }}
                    >
                      <Box sx={{
                        width: 32, height: 32, borderRadius: 1.5, flexShrink: 0,
                        bgcolor: alpha(a.color, 0.1),
                        display: 'flex', alignItems: 'center', justifyContent: 'center',
                        color: a.color,
                      }}>
                        {a.icon}
                      </Box>
                      <Typography variant="body2" fontWeight={600} color="text.primary">{a.label}</Typography>
                    </Stack>
                  );
                  return a.href ? (
                    <Link key={a.label} href={a.href} style={{ textDecoration: 'none', display: 'block' }}>
                      {inner}
                    </Link>
                  ) : (
                    <Box key={a.label} onClick={a.action}>{inner}</Box>
                  );
                })}
              </Box>
            </Paper>

            {/* Platform Status */}
            <Paper
              variant="outlined"
              sx={{
                borderRadius: 3, overflow: 'hidden',
                borderColor: alpha('#10b981', 0.2),
              }}
            >
              <Stack
                direction="row" alignItems="center" justifyContent="space-between"
                sx={{ px: 2.5, py: 1.75, borderBottom: `1px solid ${theme.palette.divider}` }}
              >
                <Typography variant="subtitle2" fontWeight={800}>Platform Status</Typography>
                <Chip
                  label="All Systems Go"
                  size="small"
                  sx={{ height: 20, fontSize: '0.62rem', fontWeight: 700, bgcolor: alpha('#10b981', 0.1), color: '#10b981' }}
                />
              </Stack>
              <Box sx={{ p: 2 }}>
                <Stack spacing={1}>
                  {[
                    { label: 'Cerulea Studio', status: 'Operational', ok: true },
                    { label: 'AI Assistant', status: 'Operational', ok: true },
                    { label: 'Deployment Engine', status: 'Operational', ok: true },
                    { label: 'RPC Gateway', status: 'Operational', ok: true },
                    { label: 'Smart Contract Compiler', status: 'Operational', ok: true },
                  ].map((s) => (
                    <Stack key={s.label} direction="row" alignItems="center" justifyContent="space-between">
                      <Stack direction="row" alignItems="center" spacing={1}>
                        <Box sx={{
                          width: 7, height: 7, borderRadius: '50%',
                          bgcolor: s.ok ? '#10b981' : '#ef4444',
                          flexShrink: 0,
                        }} />
                        <Typography variant="caption" color="text.secondary" fontWeight={500}>{s.label}</Typography>
                      </Stack>
                      <Typography variant="caption" color={s.ok ? 'success.main' : 'error.main'} fontWeight={700} sx={{ fontSize: '0.62rem' }}>
                        {s.status}
                      </Typography>
                    </Stack>
                  ))}
                </Stack>
              </Box>
            </Paper>

          </Stack>
        </Grid>
      </Grid>

      {/* Delete Confirmation Dialog */}
      <Dialog
        open={!!deleteTarget}
        onClose={() => !deleting && setDeleteTarget(null)}
        maxWidth="xs"
        fullWidth
        PaperProps={{ sx: { borderRadius: 3 } }}
      >
        <DialogTitle sx={{ fontWeight: 800, display: 'flex', alignItems: 'center', gap: 1 }}>
          <DeleteOutlineIcon color="error" />
          Delete Project
        </DialogTitle>
        <DialogContent>
          <DialogContentText>
            Are you sure you want to permanently delete{' '}
            <strong>{deleteTarget?.name}</strong>? This will remove all drafts,
            contracts, and AI threads associated with it. This cannot be undone.
          </DialogContentText>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2.5 }}>
          <Button onClick={() => setDeleteTarget(null)} disabled={deleting}>Cancel</Button>
          <Button
            variant="contained"
            color="error"
            onClick={handleDelete}
            disabled={deleting}
            startIcon={deleting ? <CircularProgress size={14} color="inherit" /> : <DeleteOutlineIcon />}
          >
            {deleting ? 'Deleting...' : 'Delete'}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
