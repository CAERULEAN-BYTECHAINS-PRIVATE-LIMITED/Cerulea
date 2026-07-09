'use client';

import { useEffect, useState } from 'react';
import {
  Box, Typography, Paper, Button, Stack, Chip,
  TextField, MenuItem, Select, FormControl, InputLabel,
  CircularProgress, Alert, Avatar, InputAdornment,
  Dialog, DialogTitle, DialogContent, DialogActions, IconButton, Tooltip,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import SearchIcon from '@mui/icons-material/Search';
import AddIcon from '@mui/icons-material/Add';
import FolderOpenIcon from '@mui/icons-material/FolderOpen';
import OpenInNewIcon from '@mui/icons-material/OpenInNew';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';

type Project = {
  id: string;
  name: string;
  slug: string;
  status: string;
  projectType: string;
  createdAt: string;
  updatedAt?: string;
};

const STATUS_COLOR: Record<string, 'success' | 'warning' | 'default' | 'error' | 'info'> = {
  active: 'success',
  deploying: 'warning',
  draft: 'default',
  failed: 'error',
};

const TYPE_LABEL: Record<string, string> = {
  blockchain: 'Blockchain',
  dapp: 'dApp',
};

function getStudioUrl(projectId: string) {
  const isLocal = typeof window !== 'undefined' && window.location.hostname.includes('localhost');
  const base = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
  return `${base}/?project=${projectId}`;
}

export default function ProjectsPage() {
  const theme = useTheme();
  const [projects, setProjects] = useState<Project[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [typeFilter, setTypeFilter] = useState('all');
  const [statusFilter, setStatusFilter] = useState('all');
  const [deleteTarget, setDeleteTarget] = useState<Project | null>(null);
  const [deleting, setDeleting] = useState(false);

  const loadProjects = async () => {
    try {
      const res = await fetch('/api/projects');
      if (!res.ok) { setError('Failed to load projects'); return; }
      const j = await res.json();
      setProjects(j.projects || []);
    } catch {
      setError('Failed to connect to server');
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { loadProjects(); }, []);

  const handleDelete = async () => {
    if (!deleteTarget) return;
    setDeleting(true);
    try {
      const res = await fetch(`/api/projects/${deleteTarget.id}`, { method: 'DELETE' });
      if (res.ok) {
        setProjects((prev) => prev.filter((p) => p.id !== deleteTarget.id));
        setDeleteTarget(null);
      } else {
        setError('Failed to delete project');
      }
    } catch {
      setError('Failed to delete project');
    } finally {
      setDeleting(false);
    }
  };

  const filtered = projects.filter((p) => {
    const matchSearch = p.name.toLowerCase().includes(search.toLowerCase()) ||
      p.slug.toLowerCase().includes(search.toLowerCase());
    const matchType = typeFilter === 'all' || p.projectType === typeFilter;
    const matchStatus = statusFilter === 'all' || p.status === statusFilter;
    return matchSearch && matchType && matchStatus;
  });

  const typeColor = (t: string) =>
    t === 'blockchain'
      ? alpha(theme.palette.secondary.main, 0.15)
      : alpha(theme.palette.primary.main, 0.15);

  return (
    <Box sx={{ p: 4, maxWidth: 1200 }}>
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>Projects</Typography>
          <Typography variant="body1" color="text.secondary">
            All your blockchain networks and dApps in one place.
          </Typography>
        </Box>
        <Button
          variant="contained"
          startIcon={<AddIcon />}
          sx={{ borderRadius: 999, fontWeight: 700 }}
          onClick={() => { window.location.href = getStudioUrl('new'); }}
        >
          New Project
        </Button>
      </Stack>

      <Paper variant="outlined" sx={{ p: 2, mb: 3, borderRadius: 3, display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
        <TextField
          size="small"
          placeholder="Search projects"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          InputProps={{ startAdornment: <InputAdornment position="start"><SearchIcon fontSize="small" /></InputAdornment> }}
          sx={{ flex: 1, minWidth: 220 }}
        />
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Type</InputLabel>
          <Select value={typeFilter} label="Type" onChange={(e) => setTypeFilter(e.target.value)}>
            <MenuItem value="all">All Types</MenuItem>
            <MenuItem value="blockchain">Blockchain</MenuItem>
            <MenuItem value="dapp">dApp</MenuItem>
          </Select>
        </FormControl>
        <FormControl size="small" sx={{ minWidth: 140 }}>
          <InputLabel>Status</InputLabel>
          <Select value={statusFilter} label="Status" onChange={(e) => setStatusFilter(e.target.value)}>
            <MenuItem value="all">All Statuses</MenuItem>
            <MenuItem value="active">Active</MenuItem>
            <MenuItem value="draft">Draft</MenuItem>
            <MenuItem value="deploying">Deploying</MenuItem>
            <MenuItem value="failed">Failed</MenuItem>
          </Select>
        </FormControl>
        <Typography variant="caption" color="text.secondary" sx={{ ml: 'auto' }}>
          {filtered.length} of {projects.length} projects
        </Typography>
      </Paper>

      {loading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}><CircularProgress /></Box>
      ) : error ? (
        <Alert severity="error">{error}</Alert>
      ) : filtered.length === 0 ? (
        <Paper variant="outlined" sx={{ borderRadius: 3, py: 10, textAlign: 'center', borderStyle: 'dashed' }}>
          <FolderOpenIcon sx={{ fontSize: 48, color: 'text.disabled', mb: 1 }} />
          <Typography variant="body1" color="text.secondary" gutterBottom>
            {projects.length === 0 ? 'No projects yet.' : 'No projects match your filters.'}
          </Typography>
          {projects.length === 0 && (
            <Button
              variant="contained"
              startIcon={<AddIcon />}
              sx={{ mt: 1, borderRadius: 999 }}
              onClick={() => { window.location.href = getStudioUrl('new'); }}
            >
              Create First Project
            </Button>
          )}
        </Paper>
      ) : (
        <Grid container spacing={2.5}>
          {filtered.map((p) => (
            <Grid xs={12} sm={6} md={4} key={p.id}>
              <Paper
                variant="outlined"
                sx={{
                  p: 2.5, borderRadius: 3, height: '100%',
                  display: 'flex', flexDirection: 'column', gap: 1.5,
                  transition: 'border-color 0.15s, box-shadow 0.15s',
                  '&:hover': {
                    borderColor: theme.palette.primary.main,
                    boxShadow: `0 0 0 1px ${alpha(theme.palette.primary.main, 0.15)}`,
                  },
                }}
              >
                <Stack direction="row" alignItems="center" gap={1.5}>
                  <Avatar sx={{ width: 40, height: 40, bgcolor: typeColor(p.projectType), color: p.projectType === 'blockchain' ? 'secondary.main' : 'primary.main', fontWeight: 900, fontSize: 16 }}>
                    {p.name[0]?.toUpperCase()}
                  </Avatar>
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Typography variant="body1" fontWeight={800} noWrap>{p.name}</Typography>
                    <Typography variant="caption" color="text.secondary" noWrap>/{p.slug}</Typography>
                  </Box>
                  <Tooltip title="Delete project">
                    <IconButton
                      size="small"
                      onClick={() => setDeleteTarget(p)}
                      sx={{ color: 'text.disabled', '&:hover': { color: 'error.main' } }}
                    >
                      <DeleteOutlineIcon fontSize="small" />
                    </IconButton>
                  </Tooltip>
                </Stack>

                <Stack direction="row" spacing={1} flexWrap="wrap">
                  <Chip
                    label={TYPE_LABEL[p.projectType] || p.projectType}
                    size="small" variant="outlined"
                    sx={{ fontWeight: 700, fontSize: '0.7rem', borderColor: p.projectType === 'blockchain' ? alpha(theme.palette.secondary.main, 0.4) : alpha(theme.palette.primary.main, 0.4), color: p.projectType === 'blockchain' ? 'secondary.main' : 'primary.main' }}
                  />
                  <Chip label={p.status || 'draft'} size="small" color={STATUS_COLOR[p.status] || 'default'} variant="filled" sx={{ fontWeight: 700, fontSize: '0.7rem' }} />
                </Stack>

                <Typography variant="caption" color="text.secondary">
                  Last modified:{' '}
                  {new Date(p.updatedAt || p.createdAt).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })}
                </Typography>

                <Box sx={{ mt: 'auto', pt: 1 }}>
                  <Button
                    fullWidth variant="outlined" size="small"
                    endIcon={<OpenInNewIcon fontSize="small" />}
                    sx={{ borderRadius: 999, fontWeight: 700 }}
                    onClick={() => { window.location.href = getStudioUrl(p.id); }}
                  >
                    Open in Studio
                  </Button>
                </Box>
              </Paper>
            </Grid>
          ))}
        </Grid>
      )}

      {/* Delete confirmation dialog */}
      <Dialog open={Boolean(deleteTarget)} onClose={() => setDeleteTarget(null)} maxWidth="xs" fullWidth>
        <DialogTitle sx={{ fontWeight: 700 }}>Delete Project</DialogTitle>
        <DialogContent>
          <Typography>
            Are you sure you want to delete <strong>{deleteTarget?.name}</strong>? This cannot be undone.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => setDeleteTarget(null)} disabled={deleting}>Cancel</Button>
          <Button
            variant="contained" color="error"
            onClick={handleDelete}
            disabled={deleting}
            startIcon={deleting ? <CircularProgress size={14} /> : <DeleteOutlineIcon />}
          >
            Delete
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
