'use client';

import * as React from 'react';
import StepGuidance from '@/components/studio/StepGuidance';
import { useStudio } from '@/context/StudioContext';
import { useSession } from 'next-auth/react';
import AuthModal from '@/components/auth/AuthModal';
import {
  Box, Grid, Typography, Stack, Paper, Button, Chip, TextField, Select, MenuItem,
  InputLabel, FormControl, IconButton, Tooltip, Divider,
  Dialog, DialogTitle, DialogContent, DialogActions, Tabs, Tab, Fade
} from '@mui/material';
import { useTheme, styled, alpha } from '@mui/material/styles';

// Icons
import AutoAwesomeMosaicIcon from '@mui/icons-material/AutoAwesomeMosaic';
import LanIcon from '@mui/icons-material/Lan';
import ArrowBackIcon from '@mui/icons-material/KeyboardArrowLeft';
import ArrowForwardIcon from '@mui/icons-material/KeyboardArrowRight';
import AddIcon from '@mui/icons-material/Add';
import SearchIcon from '@mui/icons-material/Search';
import DomainIcon from '@mui/icons-material/Domain';
import DnsIcon from '@mui/icons-material/Dns';
import BoltIcon from '@mui/icons-material/Bolt';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import RadioButtonUncheckedIcon from '@mui/icons-material/RadioButtonUnchecked';

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

type Step0Phase = 'choose-type' | 'legacy-question' | 'gallery' | 'details';
type LegacyMode = 'none' | 'connect' | 'port';

/* ---------- Utils ---------- */
function slugify(raw: string) {
  return raw.toLowerCase().normalize('NFKD').replace(/[^\w\s-]+/g, '').trim().replace(/\s+/g, '-').replace(/-+/g, '-').slice(0, 60);
}

/* ---------- Styled Components ---------- */

// 1. The Floating Dock (Now behaves as a static footer item in flex)
const FloatingIsland = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' 
    ? 'rgba(255, 255, 255, 0.95)' 
    : 'rgba(20, 20, 23, 0.95)',
  backdropFilter: 'blur(16px) saturate(180%)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 20px 40px -8px rgba(0, 0, 0, 0.3)',
  borderRadius: 100,
  padding: '8px 24px',
  display: 'flex',
  alignItems: 'center',
  gap: 16,
  transition: 'all 0.3s ease',
}));

// 2. Phase 1: Portal Card
const PortalCard = styled(Paper, { shouldForwardProp: (p) => p !== 'selected' })<{ selected?: boolean }>(({ theme, selected }) => ({
  height: 360,
  width: '100%',
  maxWidth: 420,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  cursor: 'pointer',
  position: 'relative',
  borderRadius: 32,
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.8)' : 'rgba(30,30,35,0.6)',
  backdropFilter: 'blur(12px)',
  border: `2px solid ${selected ? theme.palette.primary.main : 'transparent'}`,
  boxShadow: selected 
    ? `0 0 0 4px ${alpha(theme.palette.primary.main, 0.2)}` 
    : '0 24px 48px -12px rgba(0,0,0,0.1)',
  transition: 'all 0.3s ease',
  '&:hover': {
    transform: 'translateY(-8px)',
    boxShadow: '0 32px 64px -12px rgba(0,0,0,0.15)',
    borderColor: theme.palette.divider,
  }
}));

// 3. Phase 2: Stack Row (Wide, Vertical List)
const StackRow = styled(Paper, { shouldForwardProp: (p) => p !== 'selected' })<{ selected?: boolean }>(({ theme, selected }) => ({
  position: 'relative',
  padding: '20px 24px',
  borderRadius: 16,
  cursor: 'pointer',
  background: selected 
    ? alpha(theme.palette.primary.main, 0.04)
    : (theme.palette.mode === 'light' ? '#fff' : '#18181b'),
  border: `1px solid ${selected ? theme.palette.primary.main : theme.palette.divider}`,
  boxShadow: selected 
    ? `0 0 0 2px ${theme.palette.primary.main}` 
    : '0 2px 4px rgba(0,0,0,0.02)',
  transition: 'all 0.2s cubic-bezier(0.4, 0, 0.2, 1)',
  display: 'flex',
  alignItems: 'center',
  gap: 24,
  '&:hover': {
    borderColor: theme.palette.primary.main,
    transform: 'translateX(4px)',
    boxShadow: '0 4px 12px rgba(0,0,0,0.05)',
  }
}));

// 4. Phase 3: Split Glass Panel
const SplitGlassPanel = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' 
    ? 'rgba(255, 255, 255, 0.95)' 
    : 'rgba(20, 20, 23, 0.95)',
  backdropFilter: 'blur(20px) saturate(180%)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 32px 64px -16px rgba(0, 0, 0, 0.4)',
  borderRadius: 24,
  overflow: 'hidden',
  display: 'flex',
  flexDirection: 'column',
  width: '100%',
  maxWidth: 1100,
  // FLEXIBLE HEIGHT: It will fill the available space in the flex container
  flex: 1, 
  minHeight: 0, 
  [theme.breakpoints.up('md')]: {
    flexDirection: 'row',
  },
  [theme.breakpoints.down('md')]: {
    height: 'auto',
    flex: 'none',
  }
}));

const ConfigSection = styled(Box)(({ theme }) => ({
  flex: 1,
  padding: 40,
  overflowY: 'auto',
  height: '100%',
  '&::-webkit-scrollbar': { width: '6px' },
  '&::-webkit-scrollbar-track': { background: 'transparent' },
  '&::-webkit-scrollbar-thumb': {
    backgroundColor: theme.palette.mode === 'light' ? 'rgba(0,0,0,0.1)' : 'rgba(255,255,255,0.1)',
    borderRadius: '3px',
  },
}));

// Opaque Menu
const OPAQUE_MENU_PROPS = {
  PaperProps: {
    sx: {
      backgroundImage: 'none',
      backgroundColor: (t: any) => t.palette.mode === 'light' ? '#ffffff' : '#1e1e20',
      border: '1px solid',
      borderColor: 'divider',
      boxShadow: '0 8px 32px rgba(0,0,0,0.3)',
    }
  }
};

/* ====================================================================== */
export default function Step0({ goNext }: { goNext: () => void }) {
  const theme = useTheme();
  const { projectType, templateId, appMetadata, workspaceId, setStudioState } = useStudio();
  const { data: session } = useSession();

  // State
  const [phase, setPhase] = React.useState<Step0Phase>(projectType ? 'gallery' : 'choose-type');
  const [dType, setDType] = React.useState<ProjectType | null>(projectType);
  const [authModalOpen, setAuthModalOpen] = React.useState(false);
  const [pendingType, setPendingType] = React.useState<ProjectType | null>(null);
  const [legacyMode, setLegacyMode] = React.useState<LegacyMode>('none');
  const [legacyStep, setLegacyStep] = React.useState<'has-legacy' | 'how-to-proceed'>('has-legacy');
  const [templates, setTemplates] = React.useState<Template[]>([]);
  const [loadingTemplates, setLoadingTemplates] = React.useState(false);
  const [workspaces, setWorkspaces] = React.useState<Workspace[]>([]);
  const [selectedTemplate, setSelectedTemplate] = React.useState<string | null>(templateId ?? null);
  const [search, setSearch] = React.useState('');
  const [categoryFilter, setCategoryFilter] = React.useState('All');
  
  // Form State
  const [name, setName] = React.useState(appMetadata?.appName ?? '');
  const [slug, setSlug] = React.useState(slugify(appMetadata?.appName ?? ''));
  const [slugDirty, setSlugDirty] = React.useState(false);
  const [description, setDescription] = React.useState(appMetadata?.appDescription ?? '');
  const [wsId, setWsId] = React.useState(workspaceId ?? '');

  // Dialogs
  const [wsDialogOpen, setWsDialogOpen] = React.useState(false);
  const [wsNewName, setWsNewName] = React.useState('');

  // Deep Config
  const [dappDetails, setDappDetails] = React.useState({
    network: 'cerulea-testnet', tokenFocus: [] as string[], royalties: 5, monetization: [] as string[], emailSender: '',
  });
  const [chainDetails, setChainDetails] = React.useState({
    consensus: 'PoA', region: 'apac-south', initialValidators: 2, nativeToken: { symbol: 'CER', decimals: 18 }, feeModel: { baseGas: 1, burnPct: 0.2, validatorSharePct: 0.8 },
  });

  /* ---- Effects ---- */
  React.useEffect(() => {
    (async () => {
      try {
        const r = await fetch('/api/workspaces');
        const data = await r.json();
        setWorkspaces(data);
        if (!wsId && data.length) { setWsId(data[0].id); setStudioState({ workspaceId: data[0].id }); }
      } catch {}
    })();
  }, []); // eslint-disable-line

  const loadTemplates = React.useCallback(async (ptype: ProjectType) => {
    setLoadingTemplates(true);
    try {
      const r = await fetch(`/api/templates?projectType=${ptype}`);
      setTemplates(await r.json());
    } finally { setLoadingTemplates(false); }
  }, []);

  React.useEffect(() => {
    if (dType && phase === 'gallery' && templates.length === 0) loadTemplates(dType);
  }, [dType, phase]);

  React.useEffect(() => { if (!slugDirty) setSlug(slugify(name)); }, [name, slugDirty]);

  /* ---- Handlers ---- */
  const proceedToGallery = (ptype: ProjectType, lmode: LegacyMode = 'none') => {
    setDType(ptype);
    setStudioState({ projectType: ptype, templateId: null, legacyMode: lmode } as any);
    setSelectedTemplate(null);
    setSearch('');
    if (typeof window !== 'undefined') {
      localStorage.removeItem('cerulea.templateModules');
      localStorage.setItem('cerulea.projectType', ptype);
      localStorage.removeItem('cerulea.templateId');
    }
    setPhase('gallery');
    loadTemplates(ptype);
  };

  const chooseType = (ptype: ProjectType) => {
    // Gate: require login before Blueprint selection
    if (!session) {
      setPendingType(ptype);
      setAuthModalOpen(true);
      return;
    }
    if (ptype === 'blockchain') {
      setDType(ptype);
      setLegacyStep('has-legacy');
      setPhase('legacy-question');
    } else {
      proceedToGallery(ptype);
    }
  };

  const handleLegacyChoice = (hasLegacy: boolean) => {
    if (!hasLegacy) {
      setLegacyMode('none');
      proceedToGallery('blockchain', 'none');
    } else {
      setLegacyStep('how-to-proceed');
    }
  };

  const handleLegacyProceed = (mode: 'connect' | 'port') => {
    setLegacyMode(mode);
    proceedToGallery('blockchain', mode);
  };

  const selectTemplate = (tpl: Template | null) => {
    const id = tpl?.id ?? null;
    setSelectedTemplate(id);
    
    if (tpl) {
       setName(prev => prev || tpl.title);
       setSlug(prev => prev || slugify(tpl.title));
       setDescription(prev => prev || tpl.description);
    }
  };

  const onConfirmTemplate = () => {
    if (!selectedTemplate) return;
    setStudioState({ templateId: selectedTemplate });
    setPhase('details');
  };

  const onInitialize = async () => {
    if (!dType || !name || name.trim().length < 3 || !slug) return;

    const payload: any = {
      name, slug, description, projectType: dType, templateId: selectedTemplate ?? null, workspaceId: wsId || null,
      details: dType === 'dapp' ? { dapp: dappDetails } : { blockchain: chainDetails },
    };

    try {
      const res = await fetch('/api/projects', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(payload) });
      if (!res.ok) { alert('Failed to create project'); return; }
      const proj = await res.json();
      setStudioState({ projectId: proj.id, slug: proj.slug, appMetadata: { appName: name, appDescription: description } });

      // Seed Step 1
      if (typeof window !== 'undefined') {
        localStorage.removeItem('cerulea.step1.graph');
        let modulesToLoad: string[] = [];
        if (selectedTemplate && selectedTemplate !== 'scratch') {
           const tpl = templates.find(t => t.id === selectedTemplate);
           if (tpl) modulesToLoad = tpl.preinstalledModules || [];
        }
        localStorage.setItem('cerulea.templateModules', JSON.stringify(modulesToLoad));
        localStorage.setItem('cerulea.projectType', dType);
      }

      goNext();
    } catch (e) {
      console.error(e);
    }
  };

  const createWorkspace = async () => {
    if (!wsNewName.trim()) return;
    try {
      const res = await fetch('/api/workspaces', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name: wsNewName.trim() }) });
      if (!res.ok) throw new Error();
      const ws = await res.json();
      setWorkspaces((w) => [ws, ...w]);
      setWsId(ws.id);
    } catch { alert('Create failed'); } finally { setWsDialogOpen(false); }
  };

  const categories = React.useMemo(() => {
    const cats = new Set(templates.map(t => t.category));
    return ['All', ...Array.from(cats)];
  }, [templates]);

  const filteredTemplates = React.useMemo(() => {
    let arr = templates;
    if (categoryFilter !== 'All') {
       arr = arr.filter(t => t.category === categoryFilter);
    }
    if (search.trim()) {
      arr = arr.filter(t => `${t.title} ${t.description} ${t.tags.join(' ')}`.toLowerCase().includes(search.toLowerCase()));
    }
    return arr;
  }, [templates, search, categoryFilter]);

  return (
    // MAIN LAYOUT: Flex Column
    <Box sx={{
      width: '100%',
      position: 'fixed', inset: 0, top: 64,
      overflow: 'hidden',
      bgcolor: 'background.default',
      display: 'flex',
      flexDirection: 'column'
    }}>

      <StepGuidance
        stepKey="step0"
        title="Choose Your Project Type"
        subtitle="STEP 1 OF 7"
        description="This is where your project begins. Select whether you're building a decentralized application (dApp) or deploying a Private Blockchain network."
        steps={[
          { first: 'Pick a project type', next: 'Select "dApp" for public blockchain applications or "Private Blockchain" for enterprise deployments.' },
          { first: 'Browse templates', next: 'Choose a starting template that matches your use case (DEX, NFT Marketplace, Governance, etc.).' },
          { first: 'Configure basics', next: 'Enter your project name and set up foundational parameters like consensus and token details.' },
        ]}
        tip="Not sure which to pick? dApp is for consumer-facing products. Private Blockchain is for internal enterprise systems with custom governance."
      />

      {/* Auth gate modal appears after choose-type if user is not logged in */}
      <AuthModal
        open={authModalOpen}
        title="Sign in to continue"
        subtitle="Create a free account or sign in to start building your project."
        onSuccess={() => {
          setAuthModalOpen(false);
          if (pendingType === 'blockchain') {
            setDType('blockchain');
            setLegacyStep('has-legacy');
            setPhase('legacy-question');
          } else if (pendingType) {
            proceedToGallery(pendingType);
          }
        }}
        onClose={() => setAuthModalOpen(false)}
      />
      
      {/* Background Ambience */}
      <Box sx={{ position: 'absolute', inset: 0, opacity: 0.4, zIndex: -1, 
         backgroundImage: theme.palette.mode === 'light' 
           ? 'radial-gradient(#ccc 1px, transparent 1px)' 
           : 'radial-gradient(#333 1px, transparent 1px)', 
         backgroundSize: '32px 32px',
         maskImage: 'radial-gradient(ellipse at center, black 40%, transparent 100%)'
      }} />

      {/* CONTENT AREA: Takes available space, scrolls if needed */}
      <Box sx={{ 
        flex: 1, 
        width: '100%', 
        overflowY: 'auto', 
        overflowX: 'hidden',
        display: 'flex', 
        flexDirection: 'column', 
        alignItems: 'center',
        pt: 4, 
        pb: 2 
      }}>
        
        {/* PHASE 1: CHOOSE TYPE */}
        {phase === 'choose-type' && (
           <Fade in mountOnEnter unmountOnExit timeout={400}>
             <Stack spacing={6} alignItems="center" justifyContent="center" sx={{ width: '100%', maxWidth: 1000, px: 4, my: 'auto' }}>
                <Stack spacing={1} textAlign="center">
                   <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1 }}>STEP 1 OF 6: PROJECT FOUNDATION</Typography>
                   <Typography variant="h3" fontWeight={900} sx={{ letterSpacing: -1 }}>What are you building?</Typography>
                   <Typography variant="h6" color="text.secondary" fontWeight={400}>This determines the modules, templates, and infrastructure available to you.</Typography>
                </Stack>

                <Stack direction={{ xs: 'column', md: 'row' }} spacing={4} width="100%" justifyContent="center">
                   <PortalCard onClick={() => chooseType('dapp')}>
                      <Box sx={{ p: 3, mb: 3, borderRadius: '50%', bgcolor: alpha(theme.palette.primary.main, 0.1), color: 'primary.main' }}><AutoAwesomeMosaicIcon sx={{ fontSize: 64 }} /></Box>
                      <Typography variant="h4" fontWeight={800} gutterBottom>dApp</Typography>
                      <Typography variant="body1" color="text.secondary" align="center" sx={{ px: 4 }}>
                        A decentralised application that runs on a shared public blockchain. Users own their data and assets via wallets.
                      </Typography>
                      <Typography variant="caption" color="text.secondary" sx={{ mt: 2, opacity: 0.7 }}>e.g. NFT marketplace, DeFi platform, DAO voting app</Typography>
                   </PortalCard>

                   <PortalCard onClick={() => chooseType('blockchain')}>
                      <Box sx={{ p: 3, mb: 3, borderRadius: '50%', bgcolor: alpha(theme.palette.secondary.main, 0.1), color: 'secondary.main' }}><LanIcon sx={{ fontSize: 64 }} /></Box>
                      <Typography variant="h4" fontWeight={800} gutterBottom>Private Blockchain</Typography>
                      <Typography variant="body1" color="text.secondary" align="center" sx={{ px: 4 }}>
                        A fully owned, sovereign blockchain network with your own consensus, validators, and token. You control who participates.
                      </Typography>
                      <Typography variant="caption" color="text.secondary" sx={{ mt: 2, opacity: 0.7 }}>e.g. Supply chain ledger, CBDC, enterprise settlement network</Typography>
                   </PortalCard>
                </Stack>
             </Stack>
           </Fade>
        )}

        {/* PHASE 1.5: LEGACY QUESTION (only for Private Blockchain) */}
        {phase === 'legacy-question' && (
           <Fade in mountOnEnter unmountOnExit timeout={400}>
             <Stack spacing={6} alignItems="center" justifyContent="center" sx={{ width: '100%', maxWidth: 760, px: 4, my: 'auto' }}>
                <Stack spacing={1} textAlign="center">
                   <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1 }}>STEP 1 OF 6: PRIVATE BLOCKCHAIN</Typography>
                   {legacyStep === 'has-legacy' ? (
                     <>
                       <Typography variant="h3" fontWeight={900} sx={{ letterSpacing: -1 }}>Existing Systems?</Typography>
                       <Typography variant="h6" color="text.secondary" fontWeight={400}>
                         Do you have an existing legacy system you want to integrate or migrate?
                       </Typography>
                     </>
                   ) : (
                     <>
                       <Typography variant="h3" fontWeight={900} sx={{ letterSpacing: -1 }}>Integration Strategy</Typography>
                       <Typography variant="h6" color="text.secondary" fontWeight={400}>
                         How would you like to proceed with your legacy system?
                       </Typography>
                     </>
                   )}
                </Stack>

                {legacyStep === 'has-legacy' ? (
                  <Stack direction={{ xs: 'column', md: 'row' }} spacing={3} width="100%" justifyContent="center">
                     <PortalCard onClick={() => handleLegacyChoice(true)} sx={{ height: 280, maxWidth: 340 }}>
                        <Box sx={{ p: 3, mb: 2, borderRadius: '50%', bgcolor: alpha(theme.palette.warning.main, 0.1), color: 'warning.main' }}>
                           <DnsIcon sx={{ fontSize: 48 }} />
                        </Box>
                        <Typography variant="h5" fontWeight={800} gutterBottom>Yes, I have one</Typography>
                        <Typography variant="body2" color="text.secondary" align="center" sx={{ px: 3 }}>I want to integrate with or migrate from an existing system (ERP, CRM, database, etc.)</Typography>
                     </PortalCard>

                     <PortalCard onClick={() => handleLegacyChoice(false)} sx={{ height: 280, maxWidth: 340 }}>
                        <Box sx={{ p: 3, mb: 2, borderRadius: '50%', bgcolor: alpha(theme.palette.success.main, 0.1), color: 'success.main' }}>
                           <BoltIcon sx={{ fontSize: 48 }} />
                        </Box>
                        <Typography variant="h5" fontWeight={800} gutterBottom>No, starting fresh</Typography>
                        <Typography variant="body2" color="text.secondary" align="center" sx={{ px: 3 }}>Building everything on blockchain from scratch, no legacy systems to deal with.</Typography>
                     </PortalCard>
                  </Stack>
                ) : (
                  <Stack direction={{ xs: 'column', md: 'row' }} spacing={3} width="100%" justifyContent="center">
                     <PortalCard onClick={() => handleLegacyProceed('connect')} sx={{ height: 320, maxWidth: 340 }}>
                        <Box sx={{ p: 3, mb: 2, borderRadius: '50%', bgcolor: alpha(theme.palette.primary.main, 0.1), color: 'primary.main' }}>
                           <DnsIcon sx={{ fontSize: 48 }} />
                        </Box>
                        <Typography variant="h5" fontWeight={800} gutterBottom>Connect (Hybrid)</Typography>
                        <Typography variant="body2" color="text.secondary" align="center" sx={{ px: 3 }}>
                           Keep your existing system running. Add a blockchain layer alongside it for auditability, tokenisation, or governance.
                        </Typography>
                        <Typography variant="caption" color="text.secondary" sx={{ mt: 1, opacity: 0.7 }}>Least disruption, recommended for most enterprises</Typography>
                     </PortalCard>

                     <PortalCard onClick={() => handleLegacyProceed('port')} sx={{ height: 320, maxWidth: 340 }}>
                        <Box sx={{ p: 3, mb: 2, borderRadius: '50%', bgcolor: alpha(theme.palette.secondary.main, 0.1), color: 'secondary.main' }}>
                           <LanIcon sx={{ fontSize: 48 }} />
                        </Box>
                        <Typography variant="h5" fontWeight={800} gutterBottom>Full Port (Migrate)</Typography>
                        <Typography variant="body2" color="text.secondary" align="center" sx={{ px: 3 }}>
                           Migrate all your data and business logic from the legacy system onto the new blockchain. Clean slate, fully decentralised.
                        </Typography>
                        <Typography variant="caption" color="text.secondary" sx={{ mt: 1, opacity: 0.7 }}>Maximum transformation, for greenfield replacements</Typography>
                     </PortalCard>
                  </Stack>
                )}

                {legacyStep === 'how-to-proceed' && (
                  <Button variant="text" onClick={() => setLegacyStep('has-legacy')} startIcon={<ArrowBackIcon />}>
                    Back
                  </Button>
                )}
             </Stack>
           </Fade>
        )}

        {/* PHASE 2: GALLERY (Vertical Stack of Wide Cards) */}
        {phase === 'gallery' && (
           <Fade in mountOnEnter unmountOnExit timeout={400}>
             <Box sx={{ width: '100%', maxWidth: 900, px: 4 }}>
                {/* Header */}
                <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ mb: 2 }}>
                   <Box>
                      <Typography variant="overline" fontWeight={800} color="primary">
                        STEP 1 OF 6: {dType === 'blockchain' ? 'PRIVATE BLOCKCHAIN' : 'DAPP'}{legacyMode !== 'none' ? ` · LEGACY ${legacyMode.toUpperCase()}` : ''}
                      </Typography>
                      <Typography variant="h4" fontWeight={900}>Choose a Starting Template</Typography>
                      <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                        Templates pre-install relevant modules so you don't start from zero. You can add or remove modules in the next step.
                      </Typography>
                   </Box>
                   <Stack direction="row" alignItems="center" spacing={3}>
                      <Tabs value={categoryFilter} onChange={(_, v) => setCategoryFilter(v)} sx={{ minHeight: 0 }}>
                         {categories.map(c => <Tab key={c} value={c} label={c} sx={{ minHeight: 0, fontWeight: 600 }} />)}
                      </Tabs>
                      <TextField 
                         placeholder="Search..." size="small" value={search} onChange={e => setSearch(e.target.value)}
                         InputProps={{ startAdornment: <SearchIcon sx={{ mr: 1, color: 'text.secondary' }} /> }}
                         sx={{ width: 220 }}
                      />
                   </Stack>
                </Stack>

                {/* Vertical Stack List */}
                <Stack spacing={2} sx={{ pb: 4 }}>
                   {/* Scratch */}
                   <StackRow 
                      selected={selectedTemplate === 'scratch'}
                      onClick={() => selectTemplate({ id: 'scratch', title: 'Blank Canvas', description: 'Start from scratch with an empty workspace.', tags:['Custom'], preinstalledModules: [] } as any)}
                   >
                      <Box sx={{ width: 48, height: 48, borderRadius: 3, bgcolor: alpha(theme.palette.text.secondary, 0.1), display:'flex', alignItems:'center', justifyContent:'center', color:'text.secondary' }}>
                         <AddIcon />
                      </Box>
                      <Box sx={{ flex: 1 }}>
                         <Typography variant="h6" fontWeight={700}>Blank Canvas</Typography>
                         <Typography variant="body2" color="text.secondary">Build from zero</Typography>
                      </Box>
                      {selectedTemplate === 'scratch' ? <CheckCircleIcon color="primary" /> : <RadioButtonUncheckedIcon color="disabled" />}
                   </StackRow>

                   {filteredTemplates.map(t => (
                      <StackRow 
                         key={t.id}
                         selected={selectedTemplate === t.id}
                         onClick={() => selectTemplate(t)}
                      >
                         <Box sx={{ width: 48, height: 48, borderRadius: 3, bgcolor: alpha(theme.palette.primary.main, 0.1), display:'flex', alignItems:'center', justifyContent:'center', color:'primary.main' }}>
                            <BoltIcon />
                         </Box>
                         <Box sx={{ flex: 1 }}>
                            <Stack direction="row" alignItems="center" spacing={2} mb={0.5}>
                               <Typography variant="h6" fontWeight={700}>{t.title}</Typography>
                               <Chip label={t.category} size="small" sx={{ height: 20, fontSize: '0.65rem', fontWeight: 700 }} />
                            </Stack>
                            <Typography variant="body2" color="text.secondary">{t.description}</Typography>
                            <Stack direction="row" spacing={1} mt={1}>
                               {t.tags.slice(0,3).map(tag => (
                                  <Typography key={tag} variant="caption" sx={{ opacity: 0.6, fontFamily: 'monospace' }}>#{tag}</Typography>
                               ))}
                            </Stack>
                         </Box>
                         {selectedTemplate === t.id ? <CheckCircleIcon color="primary" /> : <RadioButtonUncheckedIcon color="disabled" />}
                      </StackRow>
                   ))}
                </Stack>
             </Box>
           </Fade>
        )}

        {/* PHASE 3: CONFIGURATION (Split Glass Panel - Fits in content area) */}
        {phase === 'details' && (
           <Fade in mountOnEnter unmountOnExit timeout={400}>
             <Box sx={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', alignItems: 'center', px: 4 }}>
                <Box sx={{ width: '100%', maxWidth: 1100, mb: 2 }}>
                   <Typography variant="overline" fontWeight={800} color="primary">STEP 1 OF 6: PROJECT CONFIGURATION</Typography>
                   <Typography variant="h4" fontWeight={900}>Configure Your Project</Typography>
                   <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5 }}>
                     Give your project a name and configure the core parameters. These settings define how your {dType === 'blockchain' ? 'blockchain network' : 'dApp'} will be deployed.
                   </Typography>
                </Box>

                <SplitGlassPanel elevation={0}>
                   {/* Left: Identity */}
                   <ConfigSection sx={{ borderRight: { md: `1px solid ${theme.palette.divider}` }, bgcolor: alpha(theme.palette.background.default, 0.4) }}>
                      <Stack direction="row" alignItems="center" spacing={2} mb={4}>
                         <DomainIcon color="primary" fontSize="large" />
                         <Box>
                            <Typography variant="h5" fontWeight={800}>Identity</Typography>
                            <Typography variant="caption" color="text.secondary">Naming & Workspace</Typography>
                         </Box>
                      </Stack>
                      
                      <Stack spacing={4}>
                         <TextField label="Project Name" fullWidth value={name} onChange={e => setName(e.target.value)} variant="outlined" />
                         <TextField label="Slug" fullWidth value={slug} onChange={e => {setSlug(e.target.value); setSlugDirty(true);}} helperText={`cerulea.studio/${slug}`} />
                         <Box>
                            <Stack direction="row" alignItems="center" justifyContent="space-between" mb={1}>
                               <InputLabel>Workspace</InputLabel>
                               <Button size="small" onClick={() => {setWsDialogOpen(true); setWsNewName('');}} startIcon={<AddIcon />}>New</Button>
                            </Stack>
                            <Select fullWidth value={wsId} onChange={e => {setWsId(e.target.value); setStudioState({workspaceId: e.target.value});}} displayEmpty MenuProps={OPAQUE_MENU_PROPS as any}>
                               <MenuItem value="">Personal Project</MenuItem>
                               {workspaces.map(w => <MenuItem key={w.id} value={w.id}>{w.name}</MenuItem>)}
                            </Select>
                         </Box>
                         <TextField label="Description" multiline rows={3} fullWidth value={description} onChange={e => setDescription(e.target.value)} placeholder="Brief description..." />
                      </Stack>
                   </ConfigSection>

                   {/* Right: Technical */}
                   <ConfigSection>
                      <Stack direction="row" alignItems="center" spacing={2} mb={4}>
                         {dType === 'dapp' ? <DnsIcon color="secondary" fontSize="large" /> : <BoltIcon color="secondary" fontSize="large" />}
                         <Box>
                            <Typography variant="h5" fontWeight={800}>{dType === 'dapp' ? 'Network Specs' : 'Genesis Params'}</Typography>
                            <Typography variant="caption" color="text.secondary">Technical Configuration</Typography>
                         </Box>
                      </Stack>

                      {dType === 'dapp' 
                         ? <DappDetails value={dappDetails} onChange={setDappDetails} />
                         : <ChainDetails value={chainDetails} onChange={setChainDetails} />
                      }
                   </ConfigSection>
                </SplitGlassPanel>
             </Box>
           </Fade>
        )}

      </Box>

      {/* FOOTER DOCK (Separate Flex Item - Cannot Overlap) */}
      {phase !== 'choose-type' && (
         <Box sx={{ height: 100, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
           <FloatingIsland elevation={4}>
              <Tooltip title="Back">
                <span>
                   <IconButton 
                      onClick={() => setPhase(prev => prev === 'details' ? 'gallery' : 'choose-type')}
                      sx={{ border: '1px solid', borderColor: 'divider' }}
                   >
                      <ArrowBackIcon />
                   </IconButton>
                </span>
              </Tooltip>

              <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />

              {phase === 'gallery' && (
                 <Button variant="contained" disabled={!selectedTemplate} onClick={onConfirmTemplate} endIcon={<ArrowForwardIcon />} sx={{ borderRadius: 100, px: 3, fontWeight: 700 }}>
                    Configure
                 </Button>
              )}

              {phase === 'details' && (
                 <Button variant="contained" disabled={!name || !slug} onClick={onInitialize} color="primary" sx={{ borderRadius: 100, px: 4, fontWeight: 700 }}>
                    Initialize Project
                 </Button>
              )}
           </FloatingIsland>
         </Box>
      )}

      {/* --- Dialogs --- */}
      <Dialog open={wsDialogOpen} onClose={() => setWsDialogOpen(false)} PaperProps={{ sx: { borderRadius: 3, p: 1 } }}>
         <DialogTitle>New Workspace</DialogTitle>
         <DialogContent>
            <TextField autoFocus margin="dense" label="Name" fullWidth value={wsNewName} onChange={e => setWsNewName(e.target.value)} />
         </DialogContent>
         <DialogActions>
            <Button onClick={() => setWsDialogOpen(false)}>Cancel</Button>
            <Button variant="contained" onClick={createWorkspace}>Create</Button>
         </DialogActions>
      </Dialog>
    </Box>
  );
}

/* ---------------- Sub-Forms ---------------- */
function DappDetails({ value, onChange }: { value: any, onChange: (v: any) => void }) {
   return (
      <Stack spacing={4}>
         <FormControl fullWidth>
            <InputLabel>Target Network</InputLabel>
            <Select value={value.network} label="Target Network" onChange={e => onChange({...value, network: e.target.value})} MenuProps={OPAQUE_MENU_PROPS as any}>
               <MenuItem value="cerulea-testnet">Cerulea Testnet</MenuItem>
               <MenuItem value="cerulea-mainnet">Cerulea Mainnet</MenuItem>
               <MenuItem value="ethereum">Ethereum Mainnet</MenuItem>
            </Select>
         </FormControl>
         <FormControl fullWidth>
            <InputLabel>Token Standards</InputLabel>
            <Select multiple value={value.tokenFocus} label="Token Standards" onChange={e => onChange({...value, tokenFocus: e.target.value})} MenuProps={OPAQUE_MENU_PROPS as any}>
               {['erc20','erc721','erc1155'].map(t => <MenuItem key={t} value={t}>{t.toUpperCase()}</MenuItem>)}
            </Select>
         </FormControl>
         <TextField type="number" label="Royalties (%)" value={value.royalties} onChange={e => onChange({...value, royalties: Number(e.target.value)})} />
      </Stack>
   );
}

function ChainDetails({ value, onChange }: { value: any, onChange: (v: any) => void }) {
   return (
      <Stack spacing={4}>
         <FormControl fullWidth>
            <InputLabel>Consensus Mechanism</InputLabel>
            <Select value={value.consensus} label="Consensus Mechanism" onChange={e => onChange({...value, consensus: e.target.value})} MenuProps={OPAQUE_MENU_PROPS as any}>
               <MenuItem value="PoA">Proof of Authority (Dev/Test)</MenuItem>
               <MenuItem value="PoS">Proof of Stake (Production)</MenuItem>
            </Select>
         </FormControl>
         <Stack direction="row" spacing={2}>
            <TextField label="Native Token" fullWidth value={value.nativeToken.symbol} onChange={e => onChange({...value, nativeToken: {...value.nativeToken, symbol: e.target.value}})} />
            <TextField type="number" label="Decimals" fullWidth value={value.nativeToken.decimals} onChange={e => onChange({...value, nativeToken: {...value.nativeToken, decimals: Number(e.target.value)}})} />
         </Stack>
         <Stack direction="row" spacing={2}>
            <TextField type="number" label="Validators" fullWidth value={value.initialValidators} onChange={e => onChange({...value, initialValidators: Number(e.target.value)})} />
            <TextField type="number" label="Base Gas" fullWidth value={value.feeModel.baseGas} onChange={e => onChange({...value, feeModel: {...value.feeModel, baseGas: Number(e.target.value)}})} />
         </Stack>
      </Stack>
   );
}