'use client';

import * as React from 'react';
import StepGuidance from '@/components/studio/StepGuidance';
import { useTheme, alpha, styled } from '@mui/material/styles';
import {
  Box, Stack, Paper, Typography, TextField, Chip, Button,
  Divider, CircularProgress, Alert, MenuItem, Select, InputLabel, FormControl,
  IconButton, Tooltip, Collapse, Fade, Dialog, DialogContent
} from '@mui/material';

// Icons
import KeyboardArrowLeftIcon from '@mui/icons-material/KeyboardArrowLeft';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';
import SearchIcon from '@mui/icons-material/Search';
import LayersIcon from '@mui/icons-material/Layers';
import AddIcon from '@mui/icons-material/Add';
import CloseIcon from '@mui/icons-material/Close';
import BoltIcon from '@mui/icons-material/Bolt';
import AutoFixHighIcon from '@mui/icons-material/AutoFixHigh';
import QuestionMarkIcon from '@mui/icons-material/QuestionMark';

import ReactFlow, {
  ReactFlowProvider, useReactFlow,
  Background, BackgroundVariant,
  MiniMap, Controls,
  useNodesState, useEdgesState,
  Node, Edge, addEdge,
  Connection, Position, Handle,
  BaseEdge, EdgeLabelRenderer, getSmoothStepPath, EdgeProps, MarkerType
} from 'reactflow';
import 'reactflow/dist/style.css';

import { useStudio } from '@/context/StudioContext';

/* ------------------ Types ------------------ */
type ProjectType = 'dapp' | 'blockchain';

type Module = {
  moduleId: string;
  title: string;
  projectType: ProjectType;
  category?: string;
  tags: string[];
  blurb?: string;
  longDescription?: string[];
  dependsOn?: string[];
  recommends?: string[];
  reasonByDepId?: Record<string, string>;
  configSchema?: any;
  __custom?: boolean;
};

type EdgeRel = 'reads' | 'writes' | 'triggers' | 'feeds' | 'calls' | 'pays' | 'custom';
type EdgeData = { rel: EdgeRel };

/* ---------------- Styled Components ---------------- */

// 1. The Floating Dock
const FloatingIsland = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' 
    ? 'rgba(255, 255, 255, 0.9)' 
    : 'rgba(20, 20, 23, 0.8)',
  backdropFilter: 'blur(20px) saturate(180%)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 20px 40px -8px rgba(0, 0, 0, 0.2)',
  borderRadius: 24,
  overflow: 'hidden',
  transition: 'all 0.3s ease',
}));

// 2. The Step Indicator Pill (New)
const StepPill = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255, 255, 255, 0.9)' : 'rgba(20, 20, 23, 0.9)',
  backdropFilter: 'blur(10px)',
  border: `1px solid ${theme.palette.divider}`,
  borderRadius: 100,
  padding: '8px 20px',
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  boxShadow: '0 4px 12px rgba(0,0,0,0.1)',
  pointerEvents: 'auto',
}));

// 3. The Spotlight (Add Module)
const SpotlightDialog = styled(Dialog)(({ theme }) => ({
  '& .MuiDialog-paper': {
    background: theme.palette.mode === 'light' ? '#fff' : '#18181b', 
    borderRadius: 20,
    border: `1px solid ${theme.palette.divider}`,
    boxShadow: '0 24px 64px rgba(0,0,0,0.3)',
    width: '100%',
    maxWidth: 600,
    margin: 16,
    position: 'absolute',
    top: 100,
  },
}));

// 4. The Help Dialog
const HelpDialog = styled(Dialog)(({ theme }) => ({
  '& .MuiDialog-paper': {
    background: theme.palette.mode === 'light' ? '#ffffff' : '#121212',
    backgroundImage: 'none',
    borderRadius: 16,
    border: `1px solid ${theme.palette.divider}`,
    boxShadow: '0 24px 48px rgba(0,0,0,0.5)',
  },
}));

const ScrollBox = styled(Box)(({ theme }) => ({
  overflowY: 'auto',
  '&::-webkit-scrollbar': { width: '6px' },
  '&::-webkit-scrollbar-track': { background: 'transparent' },
  '&::-webkit-scrollbar-thumb': {
    backgroundColor: theme.palette.mode === 'light' ? 'rgba(0,0,0,0.1)' : 'rgba(255,255,255,0.1)',
    borderRadius: '3px',
  },
}));

/* ---------------- Helpers ---------------- */
async function safeJson<T = any>(r: Response): Promise<T> {
  const txt = await r.text();
  if (!txt) throw new Error(`Empty response (status ${r.status})`);
  try { return JSON.parse(txt) as T; } catch { throw new Error(`Non-JSON response (status ${r.status})`); }
}
function blurbFromJson(m: Module): string {
  if (m.blurb && m.blurb.trim()) return m.blurb.trim();
  const firstPara = (m.longDescription?.[0] || '').trim();
  if (firstPara) return firstPara.length > 140 ? firstPara.slice(0, 137) + '…' : firstPara;
  return m.title;
}

const EDGE_LABEL: Record<EdgeRel, string> = {
  reads: 'READS',
  writes: 'WRITES',
  triggers: 'TRIGGERS',
  feeds: 'FEEDS',
  calls: 'CALLS',
  pays: 'PAYS',
  custom: 'LINKS',
};

const DEP_REASON: Record<string, string> = {
  'nft-mint->erc721': 'Minting NFTs requires the ERC-721 contract.',
  'royalties->erc721': 'EIP-2981 metadata attaches to ERC-721 tokens.',
  'erc4626->erc20': 'Vault shares track deposits of an ERC-20 asset.',
};

/* ---------------- Custom Node ---------------- */
type ModuleNodeData = {
  moduleId: string;
  title: string;
  group?: string;
  blurb?: string;
  category?: string;
  tags?: string[];
  config?: Record<string, any>;
};

function ModuleNode({ data, selected }: { data: ModuleNodeData; selected: boolean }) {
  const theme = useTheme();
  const isLight = theme.palette.mode === 'light';

  const accentColor = React.useMemo(() => {
    const colors = ['#6366f1', '#ec4899', '#14b8a6', '#f59e0b', '#8b5cf6'];
    let hash = 0;
    for (let i = 0; i < data.moduleId.length; i++) hash += data.moduleId.charCodeAt(i);
    return colors[hash % colors.length];
  }, [data.moduleId]);

  const tooltipContent = (
    <Box sx={{ maxWidth: 280, p: 0.5 }}>
      <Stack direction="row" alignItems="center" spacing={1} sx={{ mb: 1 }}>
        <Box sx={{ width: 28, height: 28, borderRadius: 1.5, bgcolor: alpha(accentColor, 0.15), display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
          <BoltIcon sx={{ fontSize: 16, color: accentColor }} />
        </Box>
        <Box>
          <Typography variant="subtitle2" fontWeight={800} sx={{ lineHeight: 1.2 }}>{data.title}</Typography>
          {data.category && (
            <Typography variant="caption" sx={{ color: accentColor, fontWeight: 700, fontSize: '0.62rem', textTransform: 'uppercase', letterSpacing: 0.3 }}>
              {data.category}
            </Typography>
          )}
        </Box>
      </Stack>
      {data.blurb && (
        <Typography variant="body2" sx={{ lineHeight: 1.55, mb: 1, color: 'text.primary', fontSize: '0.8rem' }}>
          {data.blurb}
        </Typography>
      )}
      {data.tags && data.tags.length > 0 && (
        <Box sx={{ mb: 1 }}>
          <Typography variant="caption" fontWeight={800} sx={{ opacity: 0.5, letterSpacing: 0.5, display: 'block', mb: 0.5 }}>GENERATES</Typography>
          <Stack direction="row" flexWrap="wrap" gap={0.5}>
            {data.tags.slice(0, 5).map((t) => (
              <Chip key={t} label={t} size="small"
                sx={{ height: 17, fontSize: '0.58rem', fontWeight: 700, bgcolor: alpha(accentColor, 0.1), color: accentColor, border: 'none' }} />
            ))}
          </Stack>
        </Box>
      )}
      <Box sx={{ mt: 1, pt: 1, borderTop: '1px solid', borderColor: 'divider' }}>
        <Typography variant="caption" sx={{ opacity: 0.55, fontStyle: 'italic' }}>
          Click the node to configure parameters
        </Typography>
      </Box>
    </Box>
  );

  return (
    <Box sx={{ position: 'relative' }}>
      <Handle type="target" position={Position.Left} style={{ width: 10, height: 10, left: -5, border: '2px solid white', background: accentColor }} />
      <Tooltip title={tooltipContent} placement="right" arrow enterDelay={400} enterNextDelay={200}
        componentsProps={{
          tooltip: {
            sx: {
              bgcolor: isLight ? 'rgba(255,255,255,0.97)' : 'rgba(20,20,24,0.97)',
              color: 'text.primary',
              border: '1px solid',
              borderColor: 'divider',
              backdropFilter: 'blur(12px)',
              boxShadow: '0 8px 32px rgba(0,0,0,0.25)',
              borderRadius: 2,
              p: 1.5,
            },
          },
          arrow: { sx: { color: isLight ? 'rgba(255,255,255,0.97)' : 'rgba(20,20,24,0.97)' } },
        }}
      >
        <Paper
          elevation={selected ? 12 : 2}
          sx={{
            minWidth: 260,
            borderRadius: 3,
            overflow: 'hidden',
            border: '1px solid',
            borderColor: selected ? accentColor : (isLight ? 'rgba(0,0,0,0.06)' : 'rgba(255,255,255,0.08)'),
            transition: 'all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1)',
            background: isLight ? 'rgba(255,255,255,0.95)' : 'rgba(24,24,27,0.95)',
            backdropFilter: 'blur(10px)',
            boxShadow: selected ? `0 12px 30px ${alpha(accentColor, 0.3)}` : '0 4px 6px rgba(0,0,0,0.04)',
            transform: selected ? 'scale(1.02)' : 'scale(1)',
          }}
        >
          <Stack direction="row" alignItems="center" spacing={1.5} sx={{ p: 2, borderBottom: '1px solid', borderColor: 'divider' }}>
            <Box sx={{
              width: 32, height: 32, borderRadius: 2,
              bgcolor: alpha(accentColor, 0.1), color: accentColor,
              display: 'flex', alignItems: 'center', justifyContent: 'center'
            }}>
              <BoltIcon fontSize="small" />
            </Box>
            <Box sx={{ overflow: 'hidden' }}>
              <Typography variant="subtitle2" fontWeight={800} noWrap sx={{ fontSize: '0.9rem' }}>{data.title}</Typography>
              <Typography variant="caption" sx={{ fontFamily: 'monospace', opacity: 0.5, fontSize: '0.7rem' }}>{data.moduleId}</Typography>
            </Box>
          </Stack>
          {data.group && (
            <Box sx={{ px: 2, py: 1, bgcolor: alpha(accentColor, 0.04) }}>
              <Chip
                label={data.group} size="small"
                sx={{ height: 20, fontSize: '0.65rem', fontWeight: 700, borderRadius: 1, bgcolor: alpha(accentColor, 0.1), color: accentColor }}
              />
            </Box>
          )}
        </Paper>
      </Tooltip>
      <Handle type="source" position={Position.Right} style={{ width: 10, height: 10, right: -5, border: '2px solid white', background: accentColor }} />
    </Box>
  );
}
const nodeTypes = { module: ModuleNode };

/* -------------- Custom Edge -------------- */
function RelationEdge(props: EdgeProps<EdgeData>) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, data, selected } = props;
  const [edgePath, labelX, labelY] = getSmoothStepPath({ sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, borderRadius: 16 });
  const rel = (data?.rel ?? 'custom') as EdgeRel;

  return (
    <>
      <BaseEdge 
        id={id} path={edgePath} markerEnd={markerEnd} 
        style={{ strokeWidth: selected ? 3 : 2, stroke: selected ? '#6366f1' : 'rgba(120,120,120, 0.3)', transition: 'all 0.3s' }} 
      />
      <EdgeLabelRenderer>
        <div style={{ position: 'absolute', transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY}px)`, pointerEvents: 'all', zIndex: 10 }}>
          <Box sx={{ 
             bgcolor: selected ? '#6366f1' : (t => t.palette.background.paper), 
             color: selected ? 'white' : 'text.secondary',
             border: '1px solid', borderColor: selected ? 'transparent' : 'divider',
             borderRadius: 20, px: 1, py: 0.2, fontSize: '0.65rem', fontWeight: 800, letterSpacing: 0.5,
             boxShadow: '0 2px 8px rgba(0,0,0,0.08)'
          }}>
             {EDGE_LABEL[rel]}
          </Box>
        </div>
      </EdgeLabelRenderer>
    </>
  );
}
const edgeTypes = { relation: RelationEdge };

import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';

/* ---- Config Form ---- */
function FieldHelp({ description, example }: { description?: string; example?: string }) {
  if (!description && !example) return null;
  return (
    <Tooltip
      title={
        <Box sx={{ maxWidth: 220 }}>
          {description && <Typography variant="caption" sx={{ display: 'block', mb: example ? 0.5 : 0 }}>{description}</Typography>}
          {example && <Typography variant="caption" sx={{ opacity: 0.7 }}>Example: {example}</Typography>}
        </Box>
      }
      placement="top"
      arrow
    >
      <InfoOutlinedIcon sx={{ fontSize: 14, opacity: 0.5, cursor: 'help', flexShrink: 0, '&:hover': { opacity: 1 } }} />
    </Tooltip>
  );
}

function SchemaForm({ schema, value, onChange }: { schema: any; value: Record<string, any>; onChange: (patch: Record<string, any>) => void; }) {
  const props = schema?.properties || {};
  const entries = Object.entries<any>(props);
  if (!entries.length) return (
    <Box sx={{ textAlign: 'center', py: 2 }}>
      <Typography variant="caption" sx={{ opacity: 0.5, display: 'block' }}>No configuration needed</Typography>
      <Typography variant="caption" sx={{ opacity: 0.35, display: 'block', mt: 0.5 }}>This module works with its default settings.</Typography>
    </Box>
  );

  return (
    <Stack spacing={2}>
      {entries.map(([key, def]) => {
        const label = def.title || key;
        const description = def.description;
        const example = def.examples?.[0] ?? def.default;
        const v = value?.[key];

        if (def.enum) {
          return (
            <Box key={key}>
              <Stack direction="row" alignItems="center" spacing={0.5} sx={{ mb: 0.5 }}>
                <Typography variant="caption" fontWeight={700} sx={{ opacity: 0.7 }}>{label}</Typography>
                <FieldHelp description={description} example={String(example ?? '')} />
              </Stack>
              <FormControl fullWidth size="small">
                <Select value={v ?? ''} displayEmpty onChange={(e) => onChange({ [key]: e.target.value })}>
                  <MenuItem value="" disabled><em>Select {label}</em></MenuItem>
                  {def.enum.map((opt: string) => <MenuItem key={opt} value={opt}>{opt}</MenuItem>)}
                </Select>
              </FormControl>
            </Box>
          );
        }
        if (def.type === 'boolean') {
          return (
            <Stack key={key} direction="row" alignItems="center" justifyContent="space-between" sx={{ p: 1.5, borderRadius: 1.5, border: '1px solid', borderColor: 'divider' }}>
              <Stack direction="row" alignItems="center" spacing={0.5}>
                <Typography variant="body2">{label}</Typography>
                <FieldHelp description={description} />
              </Stack>
              <Button size="small" variant={v ? 'contained' : 'outlined'} onClick={() => onChange({ [key]: !v })}>{v ? 'ON' : 'OFF'}</Button>
            </Stack>
          );
        }
        return (
          <Box key={key}>
            <Stack direction="row" alignItems="center" spacing={0.5} sx={{ mb: 0.5 }}>
              <Typography variant="caption" fontWeight={700} sx={{ opacity: 0.7 }}>{label}</Typography>
              <FieldHelp description={description} example={String(example ?? '')} />
            </Stack>
            <TextField
              fullWidth size="small"
              type={def.type === 'integer' ? 'number' : 'text'}
              value={v ?? ''}
              placeholder={def.default !== undefined ? `Default: ${def.default}` : undefined}
              onChange={(e) => onChange({ [key]: def.type === 'integer' ? Number(e.target.value) : e.target.value })}
            />
          </Box>
        );
      })}
    </Stack>
  );
}

/* =======================
   Step 1 Logic
======================= */
const GRID_COLS = 4;
const GRID_GAP_X = 280;
const GRID_GAP_Y = 160;
const GRID_START = { x: 50, y: 50 };

function gridPos(index: number) {
  const col = index % GRID_COLS;
  const row = Math.floor(index / GRID_COLS);
  return { x: GRID_START.x + col * GRID_GAP_X, y: GRID_START.y + row * GRID_GAP_Y };
}
const uniq = <T,>(arr: T[]) => Array.from(new Set(arr));

function Step1Inner({ goPrev, goNext }: { goPrev?: () => void; goNext?: () => void; }) {
  const theme = useTheme();
  const { projectType: ctxProjectType, setStudioState } = useStudio();
  const projectType = (ctxProjectType || (typeof window !== 'undefined' ? (localStorage.getItem('cerulea.projectType') as ProjectType | null) : null) || 'dapp') as ProjectType;

  // Data
  const [library, setLibrary] = React.useState<Module[]>([]);
  const [nodes, setNodes, onNodesChange] = useNodesState<ModuleNodeData>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<EdgeData>([]);
  
  // Selection
  const [selectedNodeId, setSelectedNodeId] = React.useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = React.useState<string | null>(null);

  // UI State
  const [isSpotlightOpen, setIsSpotlightOpen] = React.useState(false);
  const [isHelpOpen, setIsHelpOpen] = React.useState(false);
  const [q, setQ] = React.useState('');
  
  const rf = useReactFlow();

  // Load Library
  React.useEffect(() => {
    (async () => {
      try {
        const r = await fetch(`/api/modules?projectType=${projectType}`);
        const data = await safeJson<Module[]>(r);
        setLibrary(data);
      } catch (e) { console.warn(e); }
    })();
  }, [projectType]);

  const libById = React.useMemo(() => new Map(library.map((m) => [m.moduleId, m] as const)), [library]);

  /* ------ Persistence ------ */
  async function persistBlueprintNow() {
    if (typeof window === 'undefined') return;
    const projectId = localStorage.getItem('cerulea.projectId');
    if (!projectId) return;
    const mods = nodes.map((n) => ({
      moduleId: (n.data as any)?.moduleId,
      label: (n.data as any)?.label,
      group: (n.data as any)?.group,
      config: (n.data as any)?.config,
    }));
    try {
      await fetch(`/api/projects/${projectId}/blueprint`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ modules: mods, graph: { nodes, edges } }),
      });
    } catch (err) { console.warn(err); }
  }
  
  React.useEffect(() => {
    if(nodes.length > 0) {
       localStorage.setItem('cerulea.step1.graph', JSON.stringify({ nodes, edges }));
       // Keep StudioContext.selectedModules in sync so downstream steps (step6 deploy) can read them
       const moduleIds = nodes.map(n => n.data?.moduleId).filter(Boolean) as string[];
       setStudioState({ selectedModules: moduleIds });
    }
    const timer = setTimeout(persistBlueprintNow, 2000);
    return () => clearTimeout(timer);
  }, [nodes, edges]);

  // Seed canvas & Check if help needed
  const seededRef = React.useRef(false);
  React.useEffect(() => {
    if (seededRef.current) return;
    if (!library.length) return;

    (async () => {
       // 1. Try restore graph
       try {
         const raw = localStorage.getItem('cerulea.step1.graph');
         const saved = raw ? JSON.parse(raw) : null;
         if (saved?.nodes?.length) { 
            setNodes(saved.nodes); 
            setEdges(saved.edges || []); 
            seededRef.current = true;
            return; 
         }
       } catch {}

       // 2. Load from template (Fresh Start)
       let ids: string[] = [];
       try {
         const rawIds = localStorage.getItem('cerulea.templateModules');
         ids = rawIds ? JSON.parse(rawIds) : [];
       } catch {}

       if (ids.length > 0) {
          const toPlace = uniq(ids).map(id => libById.get(id)).filter(Boolean) as Module[];
          const newNodes = toPlace.map((m, i) => ({
             id: `n_${m.moduleId}`,
             type: 'module',
             position: gridPos(i),
             data: { moduleId: m.moduleId, title: m.title }
          }));
          setNodes(newNodes);
          seededRef.current = true;
       } else {
          // If totally empty, show help
          setIsHelpOpen(true);
       }
    })();
  }, [library]);

  /* ------ Graph Logic ------ */
  function nodeIdFor(modId: string) { return `n_${modId}`; }
  function modIdFromNodeId(nodeId: string) { return nodeId.startsWith('n_') ? nodeId.slice(2) : nodeId; }
  
  function addModule(moduleId: string) {
    const mod = libById.get(moduleId);
    if (!mod) return;
    const id = nodeIdFor(moduleId);
    
    // If module already exists, just select it and pan to it
    if (nodes.find(n => n.id === id)) {
       setSelectedNodeId(id);
       setIsSpotlightOpen(false);
       rf.fitView({ nodes: [{ id }], duration: 800, padding: 2 });
       return;
    }

    // Determine position: Center of view + small random offset
    const center = rf.project({ 
      x: (window.innerWidth / 2) - 130, // half node width roughly
      y: (window.innerHeight / 2) - 50 
    });

    const newNode: Node = {
      id, type: 'module',
      position: center.x ? center : { x: 100, y: 100 },
      data: {
        moduleId: mod.moduleId,
        title: mod.title,
        blurb: blurbFromJson(mod),
        category: mod.category,
        tags: mod.tags,
      },
    };

    setNodes(nds => [...nds, newNode]);
    setIsSpotlightOpen(false);
    setSelectedNodeId(id);
  }

  const onConnect = (c: Connection) => {
    setEdges((eds) => addEdge({ ...c, type: 'relation', markerEnd: { type: MarkerType.ArrowClosed }, data: { rel: 'custom' } }, eds) as any);
  };
  const onNodeClick = (_: any, n: Node) => { setSelectedEdgeId(null); setSelectedNodeId(n.id); };
  const onEdgeClick = (_: any, e: Edge) => { setSelectedNodeId(null); setSelectedEdgeId(e.id); };
  const onPaneClick = () => { setSelectedNodeId(null); setSelectedEdgeId(null); };

  /* ------ Selectors ------ */
  const selectedNode = React.useMemo(() => nodes.find(n => n.id === selectedNodeId) || null, [nodes, selectedNodeId]);
  const selectedModule = React.useMemo(() => selectedNode ? libById.get(modIdFromNodeId(selectedNode.id)) : null, [selectedNode, libById]);
  const selectedEdge = React.useMemo(() => edges.find(e => e.id === selectedEdgeId) || null, [edges, selectedEdgeId]);

  function updateNodeData(patch: any) {
    if(!selectedNodeId) return;
    setNodes(nds => nds.map(n => n.id === selectedNodeId ? { ...n, data: { ...n.data, ...patch } } : n));
  }

  function addMissing(depId: string) {
    addModule(depId);
    setTimeout(() => {
       const targetId = nodeIdFor(depId);
       if(selectedNodeId && nodes.some(n => n.id === targetId)) {
          setEdges(eds => [...eds, { id: `e_${selectedNodeId}_${targetId}_${Date.now()}`, source: selectedNodeId, target: targetId, type: 'relation', markerEnd: { type: MarkerType.ArrowClosed }, data: { rel: 'reads' } } as any]);
       }
    }, 200);
  }

  const handleNext = async () => { await persistBlueprintNow(); if (goNext) goNext(); };

  // Filter Spotlight
  const filteredLib = React.useMemo(() => {
    if (!q) return library;
    return library.filter(m => (m.title + m.moduleId).toLowerCase().includes(q.toLowerCase()));
  }, [library, q]);

  return (
    <Box sx={{ width: '100%', height: '100%', position: 'absolute', inset: 0, overflow: 'hidden', bgcolor: 'background.default' }}>
      <StepGuidance
        stepKey="step1"
        title="Blueprint Builder"
        subtitle="STEP 2 OF 7"
        description="Design the architecture of your application by placing and connecting modules on the canvas. Each module represents a feature or capability."
        steps={[
          { first: 'Add modules', next: 'Click "Add Extra Modules" or use the search (Ctrl+K) to find and place modules on the canvas.' },
          { first: 'Connect modules', next: 'Drag from a module\'s right handle to another module to define how they interact.' },
          { first: 'Configure each module', next: 'Click a module node to open the configuration panel on the right and set its parameters.' },
        ]}
        tip="Start with the core modules your use case requires. You can always add more later. Connected modules automatically share relevant data entities."
      />

      {/* 1. Infinite Canvas */}
      <ReactFlow
        nodes={nodes} edges={edges}
        onNodesChange={onNodesChange} onEdgesChange={onEdgesChange}
        onConnect={onConnect}
        onNodeClick={onNodeClick} onEdgeClick={onEdgeClick}
        onPaneClick={onPaneClick}
        nodeTypes={nodeTypes} edgeTypes={edgeTypes}
        fitView snapToGrid snapGrid={[20, 20]}
        minZoom={0.2} maxZoom={2}
      >
        <Background variant={BackgroundVariant.Dots} gap={24} size={1.5} color={theme.palette.mode === 'light' ? '#e4e4e7' : '#27272a'} />
        
        {/* Adjusted controls margin to avoid top-bar overlap */}
        <Controls position="top-left" style={{ marginTop: 80, marginLeft: 20 }} />
        <MiniMap position="top-right" style={{ marginTop: 80, marginRight: 20, borderRadius: 12, overflow:'hidden' }} zoomable pannable />
      </ReactFlow>

      {/* 2. Step Indicator (Top Center) */}
      <Box sx={{ position: 'absolute', top: 100, left: '50%', transform: 'translateX(-50%)', zIndex: 10, pointerEvents: 'none' }}>
        <StepPill>
          <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1, lineHeight: 1 }}>STEP 2 OF 6</Typography>
          <Divider orientation="vertical" flexItem sx={{ height: 14, my: 'auto', opacity: 0.5 }} />
          <Typography variant="subtitle2" fontWeight={700}>Blueprint Builder</Typography>
        </StepPill>
      </Box>

      {/* Empty-state guidance banner */}
      {nodes.length === 0 && (
        <Box sx={{
          position: 'absolute', top: '50%', left: '50%',
          transform: 'translate(-50%, -50%)',
          zIndex: 5, pointerEvents: 'none', textAlign: 'center',
        }}>
          <Box sx={{
            p: 3, borderRadius: 3,
            background: (t) => t.palette.mode === 'dark' ? 'rgba(20,20,24,0.7)' : 'rgba(255,255,255,0.7)',
            backdropFilter: 'blur(12px)',
            border: (t) => `1px dashed ${alpha(t.palette.primary.main, 0.3)}`,
            maxWidth: 400,
          }}>
            <Typography variant="h6" fontWeight={700} sx={{ mb: 1 }}>Build your application blueprint</Typography>
            <Typography variant="body2" sx={{ opacity: 0.7, lineHeight: 1.6 }}>
              Click <strong>"Add Extra Modules"</strong> below to choose the features
              your app needs: like wallets, tokens, governance, or payments.
              Each module becomes a building block on this canvas.
            </Typography>
            <Typography variant="caption" sx={{ display: 'block', mt: 1.5, opacity: 0.5 }}>
              Drag modules to reposition them. Draw lines between modules to define how they interact.
            </Typography>
          </Box>
        </Box>
      )}

      {/* 3. Floating Dock (Bottom) */}
      <Box sx={{ position: 'absolute', bottom: 32, left: '50%', transform: 'translateX(-50%)', zIndex: 50 }}>
        <FloatingIsland sx={{ px: 2, py: 1.5, display: 'flex', alignItems: 'center', gap: 2, borderRadius: 100 }}>
           
           <Tooltip title="Back">
             <IconButton onClick={goPrev ? goPrev : () => window.history.back()} size="small" sx={{border: '1px solid', borderColor:'divider'}}>
               <KeyboardArrowLeftIcon />
             </IconButton>
           </Tooltip>

           <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />

           <Button
             variant="contained"
             startIcon={<AddIcon />}
             onClick={() => setIsSpotlightOpen(true)}
             sx={{ borderRadius: 100, px: 3, py: 1, fontWeight: 700, boxShadow: '0 4px 12px rgba(99, 102, 241, 0.4)' }}
           >
             Add Extra Modules
           </Button>

           <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />

           <Tooltip title="Clear Canvas">
             <IconButton size="small" onClick={() => {setNodes([]); setEdges([]);}} color="error">
                <DeleteOutlineIcon />
             </IconButton>
           </Tooltip>

           <Tooltip title="Help & Guide">
             <IconButton size="small" onClick={() => setIsHelpOpen(true)} color="primary">
                <QuestionMarkIcon fontSize="small" />
             </IconButton>
           </Tooltip>

           <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />

           <Button 
             variant="text" 
             endIcon={<KeyboardArrowRightIcon />} 
             onClick={handleNext}
             sx={{ fontWeight: 700, color: 'text.primary' }}
           >
             Next
           </Button>

        </FloatingIsland>
      </Box>

      {/* 4. The "Spotlight" Library Modal */}
      <SpotlightDialog 
        open={isSpotlightOpen} 
        onClose={() => setIsSpotlightOpen(false)}
        transitionDuration={200}
      >
        <Box sx={{ p: 2 }}>
           <TextField
             autoFocus
             fullWidth
             placeholder="Search for modules..."
             value={q} onChange={e => setQ(e.target.value)}
             InputProps={{
               startAdornment: <SearchIcon sx={{ mr: 2, color: 'text.secondary' }} />,
               sx: { fontSize: '1.2rem', '& fieldset': { border: 'none' } }
             }}
           />
        </Box>
        <Divider />
        <DialogContent sx={{ p: 0, height: 400 }}>
           <ScrollBox sx={{ height: '100%' }}>
             {filteredLib.map(m => (
               <Box 
                 key={m.moduleId} 
                 onClick={() => addModule(m.moduleId)}
                 sx={{ 
                   p: 2, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 2,
                   '&:hover': { bgcolor: alpha(theme.palette.primary.main, 0.08) },
                   borderBottom: '1px solid', borderColor: 'divider'
                 }}
               >
                  <Box sx={{ p: 1, borderRadius: 1, bgcolor: alpha(theme.palette.primary.main, 0.1), color: 'primary.main' }}>
                     <LayersIcon />
                  </Box>
                  <Box>
                     <Typography variant="subtitle1" fontWeight={700}>{m.title}</Typography>
                     <Typography variant="caption" color="text.secondary">{blurbFromJson(m)}</Typography>
                  </Box>
                  <Box sx={{ ml: 'auto' }}>
                    <Chip label="Add" size="small" clickable onClick={() => addModule(m.moduleId)} />
                  </Box>
               </Box>
             ))}
           </ScrollBox>
        </DialogContent>
      </SpotlightDialog>

      {/* 5. Help / Info Dialog */}
      <HelpDialog 
         open={isHelpOpen} 
         onClose={() => setIsHelpOpen(false)}
         PaperProps={{ sx: { borderRadius: 4, maxWidth: 500 } }}
      >
         <Box sx={{ p: 3 }}>
            <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ mb: 2 }}>
              <Typography variant="h6" fontWeight={800}>Blueprint Studio Guide</Typography>
              <IconButton onClick={() => setIsHelpOpen(false)} size="small"><CloseIcon /></IconButton>
            </Stack>
            
            <Stack spacing={2} sx={{ color: 'text.secondary' }}>
               <Box>
                  <Typography variant="subtitle2" fontWeight={700} color="text.primary">1. Module Library</Typography>
                  <Typography variant="body2">Click <b>"Add Extra Modules"</b> to search the catalog. A module can be a smart contract, a backend service, or a UI feature.</Typography>
               </Box>
               <Divider />
               <Box>
                  <Typography variant="subtitle2" fontWeight={700} color="text.primary">2. Configuration</Typography>
                  <Typography variant="body2">Select any module on the canvas to open the <b>Inspector</b>. Here you can configure parameters (e.g., token name) and see dependencies.</Typography>
               </Box>
               <Divider />
               <Box>
                  <Typography variant="subtitle2" fontWeight={700} color="text.primary">3. Connections</Typography>
                  <Typography variant="body2">Drag from one module&apos;s handle to another to link them. Click the line to define the relationship (e.g., <i>Reads</i>, <i>Pays</i>).</Typography>
               </Box>
            </Stack>
            
            <Button fullWidth variant="contained" size="large" onClick={() => setIsHelpOpen(false)} sx={{ mt: 3, borderRadius: 2 }}>
               Got it
            </Button>
         </Box>
      </HelpDialog>

      {/* 6. Contextual Inspector (Right "Floating Island") */}
      <Fade in={!!selectedNodeId || !!selectedEdgeId} mountOnEnter unmountOnExit>
         {/* FIX: Increased top to 90 to clear header */}
         <Box sx={{ position: 'absolute', top: 90, right: 20, bottom: 20, width: 340, zIndex: 40, pointerEvents: 'none' }}>
           <FloatingIsland sx={{ height: '100%', display: 'flex', flexDirection: 'column', pointerEvents: 'auto' }}>
              
              <Stack direction="row" alignItems="center" justifyContent="space-between" sx={{ p: 2, borderBottom: '1px solid', borderColor: 'divider' }}>
                 <Typography variant="subtitle1" fontWeight={800}>
                    {selectedNodeId ? 'Configuration' : 'Relationship'}
                 </Typography>
                 <IconButton size="small" onClick={onPaneClick}>
                    <CloseIcon fontSize="small" />
                 </IconButton>
              </Stack>

              <ScrollBox sx={{ flex: 1, p: 3 }}>
                 {selectedNode && selectedModule && (
                    <Stack spacing={2.5}>
                       {/* Module header */}
                       <Box>
                          <Stack direction="row" spacing={1.5} alignItems="flex-start" sx={{ mb: 1 }}>
                            <Box sx={{ width: 38, height: 38, borderRadius: 2, bgcolor: alpha(theme.palette.primary.main, 0.1), display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, mt: 0.25 }}>
                              <BoltIcon sx={{ color: 'primary.main', fontSize: 20 }} />
                            </Box>
                            <Box>
                              <Typography variant="h6" fontWeight={800} sx={{ lineHeight: 1.2 }}>{selectedModule.title}</Typography>
                              {selectedModule.category && (
                                <Typography variant="caption" fontWeight={700} color="primary.main" sx={{ textTransform: 'uppercase', letterSpacing: 0.4, fontSize: '0.65rem' }}>
                                  {selectedModule.category}
                                </Typography>
                              )}
                            </Box>
                          </Stack>
                          <Typography variant="body2" color="text.secondary" sx={{ lineHeight: 1.65 }}>
                            {selectedModule.blurb || selectedModule.longDescription?.[0] || ''}
                          </Typography>
                          {selectedModule.tags && selectedModule.tags.length > 0 && (
                            <Box sx={{ mt: 1.5 }}>
                              <Typography variant="caption" fontWeight={700} color="text.disabled" sx={{ letterSpacing: 0.5, mb: 0.75, display: 'block' }}>
                                WHAT THIS GENERATES
                              </Typography>
                              <Stack direction="row" flexWrap="wrap" gap={0.5}>
                                {selectedModule.tags.slice(0, 6).map((t) => (
                                  <Chip key={t} label={t} size="small"
                                    sx={{ height: 20, fontSize: '0.62rem', fontWeight: 600,
                                      bgcolor: alpha(theme.palette.primary.main, 0.08),
                                      color: 'primary.main', border: 'none' }} />
                                ))}
                              </Stack>
                            </Box>
                          )}
                       </Box>

                       <Divider />

                       <Box>
                         <Stack direction="row" alignItems="center" spacing={0.5} sx={{ mb: 0.75 }}>
                           <Typography variant="caption" fontWeight={800} color="text.secondary" sx={{ letterSpacing: 0.5 }}>LOGICAL GROUP</Typography>
                           <Tooltip title="Optional label to group related modules (e.g. 'Payments', 'Identity'). Shown as a badge on the node. Just for your own organisation." placement="top" arrow>
                             <InfoOutlinedIcon sx={{ fontSize: 13, opacity: 0.5, cursor: 'help' }} />
                           </Tooltip>
                         </Stack>
                         <TextField
                            fullWidth size="small"
                            placeholder="e.g. Payments, Identity, Governance"
                            value={selectedNode.data.group || ''}
                            onChange={e => updateNodeData({ group: e.target.value })}
                         />
                       </Box>

                       <Box>
                          <Stack direction="row" alignItems="center" spacing={0.5} sx={{ mb: 0.5 }}>
                            <Typography variant="caption" fontWeight={800} color="text.secondary" sx={{ letterSpacing: 0.5 }}>PARAMETERS</Typography>
                            <Tooltip title="These settings customise this module's behavior in your deployment. All fields have sensible defaults — you only need to change values specific to your project." placement="top" arrow>
                              <InfoOutlinedIcon sx={{ fontSize: 13, opacity: 0.5, cursor: 'help' }} />
                            </Tooltip>
                          </Stack>
                          <Typography variant="caption" color="text.disabled" sx={{ display: 'block', mb: 1.5 }}>
                            Leave defaults or customise for your project.
                          </Typography>
                          <SchemaForm
                             schema={selectedModule.configSchema}
                             value={selectedNode.data.config || {}}
                             onChange={patch => updateNodeData({ config: { ...selectedNode.data.config || {}, ...patch } })}
                          />
                       </Box>

                       {!!selectedModule.dependsOn?.length && (
                         <Box sx={{ bgcolor: alpha(theme.palette.info.main, 0.05), p: 2, borderRadius: 2 }}>
                            <Typography variant="caption" fontWeight={700} color="info.main" sx={{ mb: 1, display: 'block' }}>SUGGESTIONS</Typography>
                            <Stack spacing={1}>
                               {selectedModule.dependsOn.map(dep => {
                                  const key1 = `${selectedModule.moduleId}->${dep}`;
                                  const key2 = `${dep}->${selectedModule.moduleId}`;
                                  const reason = selectedModule.reasonByDepId?.[dep] || DEP_REASON[key1] || DEP_REASON[key2] || 'Recommended dependency';
                                  
                                  return (
                                     <Box key={dep} sx={{p:1, border:'1px dashed', borderColor:'divider', borderRadius:1}}>
                                        <Typography variant="subtitle2" fontWeight={700}>{dep}</Typography>
                                        <Typography variant="caption" display="block" color="text.secondary" sx={{mb:1}}>{reason}</Typography>
                                        <Button
                                          size="small"
                                          startIcon={<AutoFixHighIcon />}
                                          onClick={() => addMissing(dep)}
                                          variant="outlined" color="info" fullWidth
                                        >
                                          Add to Canvas
                                        </Button>
                                     </Box>
                                  );
                               })}
                            </Stack>
                         </Box>
                       )}
                       
                       <Button color="error" variant="outlined" startIcon={<DeleteOutlineIcon />} onClick={() => { setNodes(n => n.filter(x => x.id !== selectedNodeId)); setSelectedNodeId(null); }}>
                          Remove Module
                       </Button>
                    </Stack>
                 )}

                 {selectedEdge && (
                    <Stack spacing={3}>
                       <Box>
                         <Typography variant="subtitle1" fontWeight={800} sx={{ mb: 0.5 }}>Connection Type</Typography>
                         <Typography variant="body2" color="text.secondary">
                           How does one module interact with the other? This defines the data or control flow between them.
                         </Typography>
                       </Box>
                       <Box>
                         <FormControl fullWidth>
                           <InputLabel>Relationship</InputLabel>
                           <Select
                             label="Relationship"
                             value={selectedEdge.data?.rel || 'custom'}
                             onChange={e => setEdges(eds => eds.map(ed => ed.id === selectedEdgeId ? { ...ed, data: { ...ed.data, rel: e.target.value as EdgeRel } } : ed))}
                           >
                             <MenuItem value="reads"><strong>READS</strong>: reads data from</MenuItem>
                             <MenuItem value="writes"><strong>WRITES</strong>: writes/saves data to</MenuItem>
                             <MenuItem value="triggers"><strong>TRIGGERS</strong>: activates or starts</MenuItem>
                             <MenuItem value="feeds"><strong>FEEDS</strong>: passes data to</MenuItem>
                             <MenuItem value="calls"><strong>CALLS</strong>: invokes a function on</MenuItem>
                             <MenuItem value="pays"><strong>PAYS</strong>: sends a payment to</MenuItem>
                             <MenuItem value="custom"><strong>LINKS</strong>: general connection</MenuItem>
                           </Select>
                         </FormControl>
                       </Box>
                       <Button color="error" variant="outlined" onClick={() => { setEdges(e => e.filter(x => x.id !== selectedEdgeId)); setSelectedEdgeId(null); }}>
                          Delete Connection
                       </Button>
                    </Stack>
                 )}
              </ScrollBox>
           </FloatingIsland>
         </Box>
      </Fade>

    </Box>
  );
}

export default function Step1(props: { goPrev?: () => void; goNext?: () => void; }) {
  return (
    <ReactFlowProvider>
      <Step1Inner {...props} />
    </ReactFlowProvider>
  );
}