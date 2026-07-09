'use client';

import * as React from 'react';
import { useStudio } from '@/context/StudioContext';
import {
  Box, Grid, Typography, Stack, Paper, Button, Chip, TextField, Select, MenuItem,
  InputLabel, FormControl, IconButton, Tooltip, Divider, CircularProgress,
  Dialog, DialogTitle, DialogContent, DialogActions, FormHelperText, List, ListItem,
  ListItemText, ListItemSecondaryAction
} from '@mui/material';
import { useTheme } from '@mui/material/styles';
import AutoAwesomeMosaicIcon from '@mui/icons-material/AutoAwesomeMosaic';
import LanIcon from '@mui/icons-material/Lan';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import ArrowBackIcon from '@mui/icons-material/ArrowBack';
import AddIcon from '@mui/icons-material/Add';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';

/* ---------- Types ---------- */
type Workspace = { id: string; name: string; slug: string; createdAt: string };
type ProjectType = 'dapp' | 'blockchain';

type Template = {
  id: string;
  projectType: ProjectType;
  title: string;
  description: string;
  category: string;
  icon?: string;
  tags: string[];
  preinstalledModules: string[];
};

type Step0Phase = 'choose-type' | 'gallery' | 'details';

/* ---------- Sizing ---------- */
const CARD_W = 448;
const CARD_H = 268;
const CHOICE_MAX_W = 760;
const CHOICE_H = 286;

/* ---------- Utils ---------- */
function slugify(raw: string) {
  return raw
    .toLowerCase()
    .normalize('NFKD')
    .replace(/[^\w\s-]+/g, '')
    .trim()
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .slice(0, 60);
}

/* ====================================================================== */
export default function Step0({ goNext }: { goNext: () => void }) {
  const theme = useTheme();
  const isLight = theme.palette.mode === 'light';

  const { projectType, templateId, appMetadata, workspaceId, setStudioState } = useStudio();

  const [phase, setPhase] = React.useState<Step0Phase>(projectType ? 'gallery' : 'choose-type');
  const [dType, setDType] = React.useState<ProjectType | null>(projectType);
  const [templates, setTemplates] = React.useState<Template[]>([]);
  const [loadingTemplates, setLoadingTemplates] = React.useState(false);

  const [workspaces, setWorkspaces] = React.useState<Workspace[]>([]);
  const [loadingWs, setLoadingWs] = React.useState(false);

  const [selectedTemplate, setSelectedTemplate] = React.useState<string | null>(templateId ?? null);
  const [search, setSearch] = React.useState('');

  const [name, setName] = React.useState(appMetadata?.appName ?? '');
  const [slug, setSlug] = React.useState(slugify(appMetadata?.appName ?? ''));
  const [slugDirty, setSlugDirty] = React.useState(false);
  const [description, setDescription] = React.useState(appMetadata?.appDescription ?? '');
  const [wsId, setWsId] = React.useState(workspaceId ?? '');

  const [wsDialogOpen, setWsDialogOpen] = React.useState(false);
  const [wsManageOpen, setWsManageOpen] = React.useState(false);
  const [wsNewName, setWsNewName] = React.useState('');
  const [wsDeleting, setWsDeleting] = React.useState<string | null>(null);

  const [dappDetails, setDappDetails] = React.useState({
    network: 'cerulea-testnet',
    tokenFocus: [] as string[],
    royalties: 5,
    monetization: [] as string[],
    emailSender: '',
  });
  const [chainDetails, setChainDetails] = React.useState({
    consensus: 'PoA',
    region: 'apac-south',
    initialValidators: 2,
    nativeToken: { symbol: 'CER', decimals: 18 },
    feeModel: { baseGas: 1, burnPct: 0.2, validatorSharePct: 0.8 },
  });

  /* ---- Workspaces ---- */
  React.useEffect(() => {
    let ignore = false;
    (async () => {
      setLoadingWs(true);
      try {
        const r = await fetch('/api/workspaces');
        const data = (await r.json()) as Workspace[];
        if (!ignore) {
          setWorkspaces(data);
          if (!wsId && data.length) {
            setWsId(data[0].id);
            setStudioState({ workspaceId: data[0].id });
          }
        }
      } finally {
        if (!ignore) setLoadingWs(false);
      }
    })();
    return () => { ignore = true; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /* ---- Templates ---- */
  const loadTemplates = React.useCallback(async (ptype: ProjectType) => {
    setLoadingTemplates(true);
    try {
      const r = await fetch(`/api/templates?projectType=${ptype}`);
      const data = (await r.json()) as Template[];
      setTemplates(data);
    } finally {
      setLoadingTemplates(false);
    }
  }, []);

  React.useEffect(() => {
    if (dType && phase === 'gallery' && templates.length === 0) {
      loadTemplates(dType);
    }
  }, [dType, phase, templates.length, loadTemplates]);

  /* ---- Slug live-update ---- */
  React.useEffect(() => {
    if (!slugDirty) setSlug(slugify(name));
  }, [name, slugDirty]);

  /* ---- Search filter ---- */
  const filteredTemplates = React.useMemo(() => {
    if (!search.trim()) return templates;
    const q = search.toLowerCase();
    return templates.filter(t =>
      `${t.title} ${t.description} ${(t.tags || []).join(' ')}`.toLowerCase().includes(q)
    );
  }, [templates, search]);

  /* ---- Handlers ---- */
  const chooseType = (ptype: ProjectType) => {
    setDType(ptype);
    setStudioState({ projectType: ptype, templateId: null });
    setSelectedTemplate(null);
    setSearch('');
    if (typeof window !== 'undefined') {
      window.localStorage.removeItem('cerulea.templateModules');
      window.localStorage.setItem('cerulea.projectType', ptype);
      window.localStorage.removeItem('cerulea.templateId');
    }
    setPhase('gallery');
    loadTemplates(ptype);
  };

  const chooseTemplate = (tpl: Template | null) => {
    const id = tpl?.id ?? null;
    setSelectedTemplate(id);
    setStudioState({ templateId: id });

    const nextName = tpl?.title ?? 'New Project';
    setName(nextName);
    setSlug(slugify(nextName));
    setSlugDirty(false);
    setDescription(tpl?.description ?? '');

    // Set seed for Step 1 immediately so going back/forward never leaves stale values
    if (typeof window !== 'undefined') {
      const mods = tpl?.preinstalledModules ?? [];
      window.localStorage.setItem('cerulea.templateModules', JSON.stringify(mods));
      window.localStorage.setItem('cerulea.templateId', id ?? '');
      if (dType) window.localStorage.setItem('cerulea.projectType', dType);
    }

    setPhase('details');
  };

  const submitDisabled = !dType || !name || name.trim().length < 3 || !slug;

  const onSave = async () => {
    if (submitDisabled) return;

    // Finalize Step-1 seeding right before redirect (handles any back/forward juggling)
    if (typeof window !== 'undefined') {
      if (selectedTemplate) {
        try {
          const r = await fetch(`/api/templates?ids=${encodeURIComponent(selectedTemplate)}`);
          const arr = (await r.json()) as Template[];
          const mods = (arr[0]?.preinstalledModules ?? []).filter(Boolean);
          window.localStorage.setItem('cerulea.templateModules', JSON.stringify([...new Set(mods)]));
        } catch {
          // fall back to what we already stored on selection
        }
      } else {
        // Start from scratch
        window.localStorage.setItem('cerulea.templateModules', JSON.stringify([]));
      }
      if (dType) window.localStorage.setItem('cerulea.projectType', dType);
      window.localStorage.setItem('cerulea.templateId', selectedTemplate ?? '');
    }

    const payload: any = {
      name,
      slug,
      description,
      projectType: dType,
      templateId: selectedTemplate ?? null,
      workspaceId: wsId || null, // allow individuals to skip
      details: dType === 'dapp' ? { dapp: dappDetails } : { blockchain: chainDetails },
    };

    const res = await fetch('/api/projects', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(payload),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({}));
      alert(err?.error || 'Failed to create project');
      return;
    }

    const proj = await res.json();
    setStudioState({
      projectId: proj.id,
      slug: proj.slug,
      appMetadata: { appName: name, appDescription: description },
    });

    goNext();
  };

  const openNewWs = () => { setWsDialogOpen(true); setWsNewName(''); };
  const openManage = () => setWsManageOpen(true);

  const createWorkspace = async () => {
    const nm = wsNewName.trim();
    if (!nm) return;

    try {
      const res = await fetch('/api/workspaces', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ name: nm }),
      });
      if (!res.ok) throw new Error('Create failed');
      const ws: Workspace = await res.json();
      setWorkspaces((w) => [ws, ...w]);
      setWsId(ws.id);
      setStudioState({ workspaceId: ws.id });
    } catch (e) {
      alert('Could not create workspace. Check /api/workspaces.');
    } finally {
      setWsDialogOpen(false);
    }
  };

  const deleteWorkspace = async (id: string) => {
    if (!confirm('Delete this workspace? Projects that reference it should be updated.')) return;
    setWsDeleting(id);
    try {
      const res = await fetch(`/api/workspaces?id=${encodeURIComponent(id)}`, { method: 'DELETE' });
      if (!res.ok) throw new Error('Delete failed');
      setWorkspaces((w) => w.filter(x => x.id !== id));
      if (wsId === id) {
        setWsId('');
        setStudioState({ workspaceId: '' as any });
      }
    } catch {
      alert('Failed to delete workspace.');
    } finally {
      setWsDeleting(null);
    }
  };

  /* ----------------------------- VIEW ----------------------------- */
  return (
    <Stack spacing={3} alignItems="center" sx={{ position: 'relative' }}>
      {/* Page-only glassy background */}
      <Box
        aria-hidden
        sx={(t) => ({
          position: 'fixed',
          inset: 0,
          zIndex: -1,
          background: t.palette.mode === 'light'
            ? `
              radial-gradient(1200px 600px at 20% 0%, rgba(99,102,241,0.10), transparent 60%),
              radial-gradient(1000px 500px at 80% 20%, rgba(16,185,129,0.10), transparent 60%),
              linear-gradient(to bottom, rgba(255,255,255,0.72), rgba(255,255,255,0.66))
            `
            : `
              radial-gradient(1200px 600px at 20% 0%, rgba(99,102,241,0.18), transparent 60%),
              radial-gradient(1000px 500px at 80% 20%, rgba(16,185,129,0.16), transparent 60%),
              linear-gradient(to bottom, rgba(13,15,19,0.70), rgba(13,15,19,0.65))
            `,
          backdropFilter: 'blur(14px) saturate(130%)',
          pointerEvents: 'none',
        })}
      />

      {/* Hero */}
      <Stack spacing={1.25} alignItems="center" textAlign="center" sx={{ maxWidth: 980, mt: 0.5 }}>
        <Typography
          variant="h4"
          fontWeight={900}
          sx={{
            letterSpacing: 0.2,
            background: 'linear-gradient(90deg, #8ab4ff, #7c4dff)',
            WebkitBackgroundClip: 'text',
            WebkitTextFillColor: 'transparent',
          }}
        >
          What will you build today?
        </Typography>
        <Typography variant="body1" sx={{ opacity: 0.9 }}>
          Start by choosing your foundational architecture.
        </Typography>
        <Box sx={{ height: 3, width: 120, borderRadius: 999, background: 'linear-gradient(90deg,#7c4dff,#22d3ee)', mt: 0.5 }} />
      </Stack>

      {/* Choose project type */}
      {phase === 'choose-type' && (
        <Grid container spacing={3} justifyContent="center" sx={{ width: '100%', maxWidth: 1280 }}>
          <Grid item xs={12} md={6} lg={5} display="flex" justifyContent="center">
            <SelectablePanel
              title="Build a Decentralized App (dApp)"
              description="Create on the shared, public Cerulea blockchain. Ideal for community, DeFi, NFTs."
              icon={<AutoAwesomeMosaicIcon fontSize="large" />}
              selected={dType === 'dapp'}
              onClick={() => chooseType('dapp')}
            />
          </Grid>
          <Grid item xs={12} md={6} lg={5} display="flex" justifyContent="center">
            <SelectablePanel
              title="Launch a Private Blockchain"
              description="Deploy a sovereign, high-performance network with privacy and governance controls."
              icon={<LanIcon fontSize="large" />}
              selected={dType === 'blockchain'}
              onClick={() => chooseType('blockchain')}
            />
          </Grid>
        </Grid>
      )}

      {/* Template gallery */}
      {phase === 'gallery' && dType && (
        <Stack spacing={2} sx={{ width: '100%', maxWidth: { xs: '100%', xl: 1850 } }}>
          <TopBackBar onBack={() => setPhase('choose-type')} />

          <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ px: 0.5 }}>
            <Typography variant="h5" fontWeight={800}>
              Select a Starting Point
            </Typography>
            <TextField
              placeholder="Search templates"
              size="small"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              sx={{ minWidth: 280 }}
            />
          </Stack>

          {loadingTemplates ? (
            <Box display="flex" alignItems="center" justifyContent="center" p={6}><CircularProgress /></Box>
          ) : (
            <Box
              sx={{
                display: 'grid',
                gridTemplateColumns: {
                  xs: `repeat(1, ${CARD_W}px)`,
                  md: `repeat(2, ${CARD_W}px)`,
                  lg: `repeat(3, ${CARD_W}px)`,
                },
                gap: 4,
                justifyContent: 'center',
              }}
            >
              {/* Scratch */}
              <TemplateCard
                key="scratch"
                template={{
                  id: 'scratch',
                  projectType: dType,
                  title: 'Start from Scratch',
                  description: 'Begin with a clean slate and add modules later.',
                  category: 'General',
                  tags: ['Starter'],
                  preinstalledModules: [],
                }}
                onSelect={chooseTemplate}
                selected={selectedTemplate === null}
              />

              {filteredTemplates.map((t) => (
                <TemplateCard
                  key={t.id}
                  template={t}
                  onSelect={chooseTemplate}
                  selected={selectedTemplate === t.id}
                />
              ))}
            </Box>
          )}
        </Stack>
      )}

      {/* Project details */}
      {phase === 'details' && dType && (
        <Stack spacing={3} sx={{ width: '100%', maxWidth: 980 }}>
          <TopBackBar onBack={() => setPhase('gallery')} />
          <Typography variant="h5" fontWeight={800}>Project Details</Typography>

          <Stack spacing={2}>
            <TextField
              label="Project Name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              fullWidth
              required
              inputProps={{ maxLength: 80 }}
            />

            <TextField
              label="Slug"
              value={slug}
              onChange={(e) => { setSlug(e.target.value); setSlugDirty(true); }}
              onBlur={() => setSlug((s) => slugify(s))}
              helperText="Used in URLs. Lowercase letters, numbers, and hyphens only."
              fullWidth
              inputProps={{ maxLength: 60 }}
            />

            <FormControl fullWidth>
              <InputLabel id="ws-label">Workspace (optional)</InputLabel>
              <Select
                labelId="ws-label"
                label="Workspace (optional)"
                value={wsId}
                onChange={(e) => { setWsId(e.target.value as string); setStudioState({ workspaceId: e.target.value as string }); }}
                MenuProps={{
                  PaperProps: {
                    elevation: 8,
                    sx: {
                      bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
                      backgroundImage: 'none',
                      backdropFilter: 'none',
                      border: '1px solid',
                      borderColor: 'divider',
                    },
                  },
                }}
                slotProps={{
                  paper: {
                    elevation: 8,
                    sx: {
                      bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
                      backgroundImage: 'none',
                      backdropFilter: 'none',
                      border: '1px solid',
                      borderColor: 'divider',
                    },
                  } as any,
                }}
              >
                {loadingWs && <MenuItem value=""><em>Loading…</em></MenuItem>}
                {!loadingWs && workspaces.map((w) => (
                  <MenuItem key={w.id} value={w.id}>{w.name}</MenuItem>
                ))}
                {!loadingWs && workspaces.length === 0 && <MenuItem value=""><em>No workspaces</em></MenuItem>}
              </Select>
              <FormHelperText>
                Organize projects by team or client. Individuals can leave this blank.
              </FormHelperText>
            </FormControl>

            <Stack direction="row" spacing={1}>
              <Button onClick={openNewWs} startIcon={<AddIcon />} variant="outlined">Create workspace</Button>
              <Button onClick={openManage} variant="text">Manage workspaces</Button>
            </Stack>

            <TextField
              label="Description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              fullWidth
              multiline
              minRows={3}
            />
          </Stack>

          <Divider />

          {dType === 'dapp' ? (
            <DappDetails value={dappDetails} onChange={setDappDetails} />
          ) : (
            <ChainDetails value={chainDetails} onChange={setChainDetails} />
          )}

          <Stack direction="row" justifyContent="flex-end" spacing={2}>
            <Button variant="contained" disabled={submitDisabled} onClick={onSave}>
              Save and Continue
            </Button>
          </Stack>
        </Stack>
      )}

      {/* Create workspace dialog — OPAQUE */}
      <Dialog
        open={wsDialogOpen}
        onClose={() => setWsDialogOpen(false)}
        fullWidth
        maxWidth="xs"
        PaperProps={{
          elevation: 10,
          sx: {
            borderRadius: 3,
            bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
            color: 'inherit',
            backgroundImage: 'none',
            backdropFilter: 'none',
            border: '1px solid',
            borderColor: 'divider',
          },
        }}
        BackdropProps={{ sx: { backgroundColor: 'rgba(0,0,0,0.35)' } }}
      >
        <DialogTitle sx={{ pb: 0.5 }}>Create workspace</DialogTitle>
        <DialogContent sx={{ pt: 1.5 }}>
          <TextField
            autoFocus
            margin="dense"
            label="Workspace name"
            fullWidth
            value={wsNewName}
            onChange={(e) => setWsNewName(e.target.value)}
          />
        </DialogContent>
        <DialogActions sx={{ px: 2, pb: 2, pt: 1, gap: 1, justifyContent: 'flex-end', flexWrap: 'wrap' }}>
          <Button onClick={() => setWsDialogOpen(false)}>Cancel</Button>
          <Button onClick={createWorkspace} variant="contained">Create</Button>
        </DialogActions>
      </Dialog>

      {/* Manage workspaces (Delete) — OPAQUE */}
      <Dialog
        open={wsManageOpen}
        onClose={() => setWsManageOpen(false)}
        fullWidth
        maxWidth="sm"
        PaperProps={{
          elevation: 10,
          sx: {
            borderRadius: 3,
            bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
            backgroundImage: 'none',
            backdropFilter: 'none',
            border: '1px solid',
            borderColor: 'divider',
          },
        }}
        BackdropProps={{ sx: { backgroundColor: 'rgba(0,0,0,0.35)' } }}
      >
        <DialogTitle>Workspaces</DialogTitle>
        <DialogContent dividers>
          {loadingWs ? (
            <Box py={3} display="flex" justifyContent="center"><CircularProgress size={20} /></Box>
          ) : (
            <List dense>
              {workspaces.map((w) => (
                <ListItem key={w.id} divider>
                  <ListItemText
                    primary={w.name}
                    secondary={w.slug}
                    primaryTypographyProps={{ fontWeight: 600 }}
                  />
                  <ListItemSecondaryAction>
                    <IconButton
                      edge="end"
                      aria-label="delete"
                      onClick={() => deleteWorkspace(w.id)}
                      disabled={wsDeleting === w.id}
                    >
                      <DeleteOutlineIcon />
                    </IconButton>
                  </ListItemSecondaryAction>
                </ListItem>
              ))}
              {workspaces.length === 0 && (
                <Box py={2} textAlign="center" sx={{ opacity: 0.7 }}>No workspaces</Box>
              )}
            </List>
          )}
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setWsManageOpen(false)}>Close</Button>
        </DialogActions>
      </Dialog>
    </Stack>
  );
}

/* ---------- Top back bar ---------- */
function TopBackBar({ onBack }: { onBack: () => void }) {
  return (
    <Stack direction="row" alignItems="center" spacing={1}>
      <IconButton onClick={onBack} aria-label="Back"><ArrowBackIcon /></IconButton>
      <Typography variant="subtitle2">Back</Typography>
    </Stack>
  );
}

/* ---------- Choice panels ---------- */
function SelectablePanel({
  title, description, icon, selected, onClick,
}: {
  title: string; description: string; icon: React.ReactNode; selected: boolean; onClick: () => void;
}) {
  const theme = useTheme();
  const isLight = theme.palette.mode === 'light';

  return (
    <Paper
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') onClick(); }}
      sx={{
        width: '100%',
        maxWidth: CHOICE_MAX_W,
        height: { xs: CHOICE_H - 16, md: CHOICE_H },
        p: 4,
        borderRadius: 24,
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        cursor: 'pointer',
        transition: 'transform 180ms ease, box-shadow 180ms ease',
        '&:hover': { transform: 'translateY(-2px)' },
        background: isLight
          ? 'linear-gradient( to bottom right, rgba(255,255,255,0.60), rgba(255,255,255,0.30) )'
          : 'linear-gradient( to bottom right, rgba(255,255,255,0.08), rgba(255,255,255,0.04) )',
        backdropFilter: 'blur(10px) saturate(120%)',
        border: '1px solid rgba(255,255,255,0.22)',
        boxShadow: `${selected ? '0 0 0 2px rgba(99,102,241,0.95)' : '0 8px 28px rgba(0,0,0,0.48)'}`,
        overflow: 'hidden',
        color: isLight ? theme.palette.text.primary : 'rgba(255,255,255,0.95)',
      }}
    >
      <Stack spacing={1.4} alignItems="center" textAlign="center" sx={{ width: '100%' }}>
        <Box>{icon}</Box>
        <Typography variant="h6" fontWeight={800} sx={{ px: 1 }}>{title}</Typography>
        <Typography variant="body2" sx={{ opacity: 0.9, px: 1, maxWidth: 560, mx: 'auto' }}>
          {description}
        </Typography>
      </Stack>
    </Paper>
  );
}

/* ---------- Template card ---------- */
function TemplateCard({
  template, onSelect, selected,
}: {
  template: Template;
  onSelect: (tpl: Template | null) => void;
  selected: boolean;
}) {
  const theme = useTheme();
  const isLight = theme.palette.mode === 'light';

  return (
    <Paper
      role="button"
      tabIndex={0}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') onSelect(template); }}
      onClick={() => onSelect(template)}
      sx={{
        width: CARD_W,
        height: CARD_H,
        p: 3.5,
        borderRadius: 24,
        position: 'relative',
        transition: 'transform 180ms ease, box-shadow 180ms ease, backdrop-filter 180ms ease, background 180ms ease',
        '&:hover': {
          transform: 'translateY(-2px)',
          backdropFilter: 'blur(22px) saturate(170%)',
          background: isLight
            ? 'linear-gradient(to bottom right, rgba(255,255,255,0.66), rgba(255,255,255,0.30))'
            : 'linear-gradient(to bottom right, rgba(255,255,255,0.10), rgba(255,255,255,0.06))',
        },
        boxShadow: `${selected ? '0 0 0 2px rgba(99,102,241,0.95)' : '0 10px 30px rgba(0,0,0,0.50)'}`,
        background: isLight
          ? 'linear-gradient(to bottom right, rgba(255,255,255,0.52), rgba(255,255,255,0.26))'
          : 'linear-gradient(to bottom right, rgba(255,255,255,0.06), rgba(255,255,255,0.03))',
        backdropFilter: 'blur(10px) saturate(120%)',
        border: '1px solid rgba(255,255,255,0.22)',
        overflow: 'hidden',
        color: isLight ? theme.palette.text.primary : 'rgba(255,255,255,0.95)',
        '&:hover .overlay': { opacity: 1 },
        '&:hover .base': { opacity: 0 },
      }}
    >
      {/* BASE */}
      <Box
        className="base"
        sx={{
          position: 'relative',
          zIndex: 1,
          height: '100%',
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          textAlign: 'center',
          minWidth: 0,
          px: 3,
          pb: 7, // reserve space so tags never overlap the text
          justifyContent: 'center',
          transition: 'opacity 160ms ease',
        }}
      >
        <Stack spacing={1} sx={{ width: '100%', minWidth: 0 }}>
          <Stack direction="row" alignItems="center" spacing={1} sx={{ justifyContent: 'center' }}>
            <Typography
              variant="h6"
              fontWeight={900}
              sx={{ wordBreak: 'break-word', px: 2, letterSpacing: 0.2 }}
            >
              {template.title}
            </Typography>
            <Tooltip title="This template will pre-install a set of modules.">
              <InfoOutlinedIcon fontSize="small" />
            </Tooltip>
          </Stack>

          <Typography
            variant="body1"
            sx={{
              opacity: 0.95,
              wordBreak: 'break-word',
              display: '-webkit-box',
              WebkitLineClamp: 3,
              WebkitBoxOrient: 'vertical',
              overflow: 'hidden',
              px: 1.5,
              lineHeight: 1.35,
            }}
          >
            {template.description}
          </Typography>
        </Stack>

        {/* Tags pinned bottom-center */}
        <Box
          sx={{
            position: 'absolute',
            bottom: 10,
            left: '50%',
            transform: 'translateX(-50%)',
            maxWidth: '90%',
            display: 'flex',
            flexWrap: 'wrap',
            rowGap: 0.5,
            columnGap: 0.5,
            justifyContent: 'center',
          }}
        >
          {template.tags.slice(0, 3).map(tag => (
            <Chip key={tag} size="small" label={tag} />
          ))}
        </Box>
      </Box>

      {/* HOVER OVERLAY — centered and scroll-safe */}
      <Box
        className="overlay"
        sx={{
          position: 'absolute',
          inset: 0,
          zIndex: 2,
          borderRadius: 24,
          p: 2.5,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          bgcolor: isLight ? '#fff' : 'rgba(18,18,20,1)',
          color: isLight ? theme.palette.text.primary : '#fff',
          opacity: 0,
          transition: 'opacity 160ms ease',
          pointerEvents: 'auto',
          overflow: 'hidden',
          textAlign: 'center',
        }}
      >
        <Stack spacing={1.25} sx={{ width: '92%', maxWidth: '92%' }} alignItems="center">
          <Typography variant="subtitle2" sx={{ fontWeight: 900 }}>
            Pre-installed modules
          </Typography>

          <Box
            sx={{
              width: '100%',
              maxHeight: 160,
              overflowY: 'auto',
              display: 'flex',
              flexWrap: 'wrap',
              rowGap: 0.5,
              columnGap: 0.5,
              justifyContent: 'center',
              px: 0.5,
            }}
          >
            {template.preinstalledModules.length === 0
              ? <Chip size="small" label="None" />
              : template.preinstalledModules.map(m => <Chip key={m} size="small" label={m} />)}
          </Box>
        </Stack>
      </Box>
    </Paper>
  );
}

/* ----- dApp Details ----- */
function DappDetails({
  value, onChange,
}: {
  value: { network: string; tokenFocus: string[]; royalties: number; monetization: string[]; emailSender: string };
  onChange: (v: any) => void;
}) {
  const theme = useTheme();
  const menuSX = {
    bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
    backgroundImage: 'none',
    backdropFilter: 'none',
    border: '1px solid',
    borderColor: 'divider',
  } as const;

  const tokenOptions = [
    'erc20','erc721','erc1155','erc4626','erc777','erc1363','erc4907','erc3525','erc721a','mixed','custom'
  ];

  return (
    <Stack spacing={2}>
      <Typography variant="h6" fontWeight={800}>dApp Settings</Typography>

      <FormControl fullWidth>
        <InputLabel id="network-label">Primary Network</InputLabel>
        <Select
          labelId="network-label"
          label="Primary Network"
          value={value.network}
          onChange={(e) => onChange({ ...value, network: e.target.value })}
          MenuProps={{ PaperProps: { sx: menuSX } }}
          slotProps={{ paper: { sx: menuSX } as any }}
        >
          <MenuItem value="cerulea-testnet">Cerulea Testnet</MenuItem>
          <MenuItem value="cerulea-mainnet">Cerulea Mainnet</MenuItem>
        </Select>
      </FormControl>

      <FormControl fullWidth>
        <InputLabel id="token-label">Token Standard Focus</InputLabel>
        <Select
          labelId="token-label"
          label="Token Standard Focus"
          multiple
          value={value.tokenFocus}
          onChange={(e) => onChange({ ...value, tokenFocus: e.target.value as string[] })}
          MenuProps={{ PaperProps: { sx: menuSX } }}
          slotProps={{ paper: { sx: menuSX } as any }}
          renderValue={(sel) => (sel as string[]).join(', ').toUpperCase()}
        >
          {tokenOptions.map(t => <MenuItem key={t} value={t}>{t.toUpperCase()}</MenuItem>)}
        </Select>
      </FormControl>

      <TextField
        type="number"
        label="Default Royalties (%)"
        value={value.royalties}
        onChange={(e) => onChange({ ...value, royalties: Number(e.target.value) })}
        fullWidth
        inputProps={{ min: 0, max: 25 }}
      />

      <FormControl fullWidth>
        <InputLabel id="monetization-label">Monetization</InputLabel>
        <Select
          labelId="monetization-label"
          label="Monetization"
          multiple
          value={value.monetization}
          onChange={(e) => onChange({ ...value, monetization: e.target.value as string[] })}
          MenuProps={{ PaperProps: { sx: menuSX } }}
          slotProps={{ paper: { sx: menuSX } as any }}
        >
          <MenuItem value="subscriptions">Subscriptions</MenuItem>
          <MenuItem value="tips">Tips</MenuItem>
          <MenuItem value="marketplace-fees">Marketplace Fees</MenuItem>
        </Select>
      </FormControl>

      <TextField
        label="Email Sender (optional)"
        value={value.emailSender}
        onChange={(e) => onChange({ ...value, emailSender: e.target.value })}
        fullWidth
      />
    </Stack>
  );
}

/* ----- Chain Details ----- */
function ChainDetails({
  value, onChange,
}: {
  value: {
    consensus: string;
    region: string;
    initialValidators: number;
    nativeToken: { symbol: string; decimals: number };
    feeModel: { baseGas: number; burnPct: number; validatorSharePct: number };
  };
  onChange: (v: any) => void;
}) {
  const theme = useTheme();
  const menuSX = {
    bgcolor: theme.palette.mode === 'light' ? '#fff' : '#0E1117',
    backgroundImage: 'none',
    backdropFilter: 'none',
    border: '1px solid',
    borderColor: 'divider',
  } as const;

  return (
    <Stack spacing={2}>
      <Typography variant="h6" fontWeight={800}>Private Chain Settings</Typography>

      <FormControl fullWidth>
        <InputLabel id="consensus-label">Consensus</InputLabel>
        <Select
          labelId="consensus-label"
          label="Consensus"
          value={value.consensus}
          onChange={(e) => onChange({ ...value, consensus: e.target.value })}
          MenuProps={{ PaperProps: { sx: menuSX } }}
          slotProps={{ paper: { sx: menuSX } as any }}
        >
          <MenuItem value="PoA">PoA</MenuItem>
          <MenuItem value="PoS">PoS</MenuItem>
        </Select>
      </FormControl>

      <FormControl fullWidth>
        <InputLabel id="region-label">Region</InputLabel>
        <Select
          labelId="region-label"
          label="Region"
          value={value.region}
          onChange={(e) => onChange({ ...value, region: e.target.value })}
          MenuProps={{ PaperProps: { sx: menuSX } }}
          slotProps={{ paper: { sx: menuSX } as any }}
        >
          <MenuItem value="apac-south">APAC South</MenuItem>
          <MenuItem value="us-east">US East</MenuItem>
          <MenuItem value="eu-central">EU Central</MenuItem>
        </Select>
      </FormControl>

      <TextField
        type="number"
        label="Initial Validators"
        value={value.initialValidators}
        onChange={(e) => onChange({ ...value, initialValidators: Number(e.target.value) })}
        fullWidth
        inputProps={{ min: 1, max: 10 }}
      />

      <TextField
        label="Native Token Symbol"
        value={value.nativeToken.symbol}
        onChange={(e) => onChange({ ...value, nativeToken: { ...value.nativeToken, symbol: e.target.value } })}
        fullWidth
        inputProps={{ maxLength: 8 }}
      />

      <TextField
        type="number"
        label="Native Token Decimals"
        value={value.nativeToken.decimals}
        onChange={(e) => onChange({ ...value, nativeToken: { ...value.nativeToken, decimals: Number(e.target.value) } })}
        fullWidth
        inputProps={{ min: 0, max: 30 }}
      />

      <TextField
        type="number"
        label="Base Gas"
        value={value.feeModel.baseGas}
        onChange={(e) => onChange({ ...value, feeModel: { ...value.feeModel, baseGas: Number(e.target.value) } })}
        fullWidth
      />

      <TextField
        type="number"
        label="Burn %"
        value={value.feeModel.burnPct}
        onChange={(e) => onChange({ ...value, feeModel: { ...value.feeModel, burnPct: Number(e.target.value) } })}
        fullWidth
      />

      <TextField
        type="number"
        label="Validator Share %"
        value={value.feeModel.validatorSharePct}
        onChange={(e) => onChange({ ...value, feeModel: { ...value.feeModel, validatorSharePct: Number(e.target.value) } })}
        fullWidth
      />
    </Stack>
  );
}
