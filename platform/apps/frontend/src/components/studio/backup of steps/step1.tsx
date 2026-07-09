'use client';

import * as React from 'react';
import { useTheme } from '@mui/material/styles';
import {
  Box, Stack, Paper, Typography, TextField, Chip, Button,
  Divider, CircularProgress, Alert, MenuItem, Select, InputLabel, FormControl
} from '@mui/material';
import KeyboardArrowLeftIcon from '@mui/icons-material/KeyboardArrowLeft';
import KeyboardArrowRightIcon from '@mui/icons-material/KeyboardArrowRight';
import DeleteOutlineIcon from '@mui/icons-material/DeleteOutline';

import ReactFlow, {
  ReactFlowProvider, useReactFlow,
  Background, BackgroundVariant,
  MiniMap, Controls,
  useNodesState, useEdgesState,
  Node, Edge, addEdge, applyNodeChanges, applyEdgeChanges,
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

type TemplateLite = {
  templateId: string;
  preinstalledModules?: string[];
};

type EdgeRel = 'reads' | 'writes' | 'triggers' | 'feeds' | 'calls' | 'pays' | 'custom';
type EdgeData = { rel: EdgeRel };

type Blueprint = {
  modules: { moduleId: string; label?: string; group?: string; config?: Record<string, any> }[];
  graph: { nodes: Node[]; edges: Edge<EdgeData>[] };
};

const GRID_COLS = 4;
const GRID_GAP_X = 220;
const GRID_GAP_Y = 140;
const GRID_START = { x: 160, y: 100 };

/* ---------------- helpers ---------------- */
async function safeJson<T = any>(r: Response): Promise<T> {
  const txt = await r.text();
  if (!txt) throw new Error(`Empty response (status ${r.status})`);
  try { return JSON.parse(txt) as T; } catch { throw new Error(`Non-JSON response (status ${r.status})`); }
}
const uniq = <T,>(arr: T[]) => Array.from(new Set(arr));
function blurbFromJson(m: Module): string {
  if (m.blurb && m.blurb.trim()) return m.blurb.trim();
  const firstPara = (m.longDescription?.[0] || '').trim();
  if (firstPara) return firstPara.length > 140 ? firstPara.slice(0, 137) + '…' : firstPara;
  return m.title;
}
function gridPos(index: number) {
  const col = index % GRID_COLS;
  const row = Math.floor(index / GRID_COLS);
  return { x: GRID_START.x + col * GRID_GAP_X, y: GRID_START.y + row * GRID_GAP_Y };
}

/* ------------------ Edge labels ------------------ */
const EDGE_LABEL: Record<EdgeRel, string> = {
  reads: 'reads',
  writes: 'writes',
  triggers: 'triggers',
  feeds: 'feeds',
  calls: 'calls',
  pays: 'pays',
  custom: 'relates to',
};

/* --------- Reasons for dependency pairs (fallback text) --------- */
const DEP_REASON: Record<string, string> = {
  'nft-mint->erc721': 'Minting NFTs requires the ERC-721 contract.',
  'royalties->erc721': 'EIP-2981 metadata attaches to ERC-721 tokens.',
  'erc4626->erc20': 'Vault shares track deposits of an ERC-20 asset.',
  'voting->erc20': 'Token-weighted voting reads ERC-20 balances.',
  'proposals->voting': 'Proposals execute only after successful vote.',
  'proposals->treasury': 'Budgeted proposals pay out via treasury.',
  'subscriptions->erc20': 'Recurring charges denominate in an ERC-20.',
  'staking->erc20': 'Stake/unstake flows operate on ERC-20 balances.',
  'marketplace->erc721': 'Listings reference ERC-721 token IDs.',
  'chainlink->oracles': 'Chainlink is a provider used by Oracles.',
  'sendgrid->emails': 'SendGrid delivers the transactional emails.',
  'stripe->fiat-onramp': 'Stripe is a supported on-ramp provider.',
  'kyc->wallet-auth': 'KYC checks are applied to authenticated wallets.',
  'evm-config->consensus': 'Consensus parameters inform EVM limits.',
  'genesis->evm-config': 'Genesis allocs depend on final EVM params.',
  'stf->evm-config': 'State transition constrained by EVM rules.',
  'tokenomics->consensus': 'Rewards/issuance align with consensus epochs.',
  'validators->consensus': 'Validator rules reference consensus epochs.',
  'validators->tokenomics': 'Rewards/slashing logic uses tokenomics policy.',
  'fees->evm-config': 'Base-fee tuning needs gas targets/limits.',
  'rpc->evm-config': 'Exposed RPC depends on EVM features enabled.',
};

/* --------- Tag Categories → Tags (compact dropdown lists) --------- */
const TAG_CATEGORIES: Record<string, string[]> = {
  'Identity & Access': ['auth', 'wallets', 'profiles', 'rbac', 'kyc'],
  'Tokens & NFTs': ['erc20', 'erc721', 'erc1155', 'erc4626', 'nft', 'royalties', 'mint', 'burn', 'staking'],
  'Commerce & Payments': ['payments', 'subscriptions', 'invoices', 'onramp', 'offramp', 'upi', 'stripe', 'razorpay'],
  'Data & Oracles': ['oracles', 'price', 'data', 'ipfs', 'storage', 'event'],
  'Governance & Treasury': ['governance', 'votes', 'proposals', 'treasury'],
  'UI & Content': ['ui', 'cms', 'i18n', 'branding'],
  'Community & Comms': ['notifications', 'emails', 'chat', 'social'],
  'Compliance & Safety': ['aml', 'fraud', 'rate-limit', 'audit'],
  'Analytics & Observability': ['analytics', 'metrics', 'logs', 'alerts'],
  'Core Protocol': ['consensus', 'evm', 'stf', 'genesis', 'p2p', 'upgrade'],
  'Networking & APIs': ['rpc', 'graphql', 'websocket', 'faucet'],
  'Economics & Staking': ['fees', 'tokenomics', 'validators'],
};

/* ---------------- custom node ---------------- */
function ModuleNode({ data, selected }: { data: { moduleId: string; title: string; group?: string }; selected: boolean }) {
  const theme = useTheme();
  const isLight = theme.palette.mode === 'light';
  return (
    <Paper
      sx={{
        p: 1.5, px: 2, minWidth: 220, minHeight: 64,
        borderRadius: 2,
        border: '1px solid rgba(255,255,255,0.22)',
        background: isLight
          ? 'linear-gradient( to bottom right, rgba(255,255,255,0.65), rgba(255,255,255,0.35) )'
          : 'linear-gradient( to bottom right, rgba(255,255,255,0.12), rgba(255,255,255,0.07) )',
        backdropFilter: 'blur(10px) saturate(130%)',
        boxShadow: selected ? '0 0 0 2px rgba(99,102,241,0.9)' : '0 6px 18px rgba(0,0,0,0.35)',
        display: 'flex', alignItems: 'center', justifyContent: 'center', textAlign: 'center',
      }}
    >
      <Handle type="target" position={Position.Left} style={{ opacity: 0.8 }} />
      <Stack spacing={0.25} sx={{ maxWidth: 260 }}>
        <Typography variant="subtitle1" fontWeight={800} sx={{ lineHeight: 1.2, textAlign: 'center' }}>
          {data.title}
        </Typography>
        {data.group ? (
          <Typography variant="caption" sx={{ opacity: 0.8, textAlign: 'center' }}>{data.group}</Typography>
        ) : null}
      </Stack>
      <Handle type="source" position={Position.Right} style={{ opacity: 0.8 }} />
    </Paper>
  );
}
const nodeTypes = { module: ModuleNode };

/* -------------- custom edge to avoid overlapping labels -------------- */
function hashOffsetPx(id: string): number {
  let h = 0;
  for (let i = 0; i < id.length; i++) h = (h * 31 + id.charCodeAt(i)) | 0;
  const table = [-24, -12, 0, 12, 24, 36, -36];
  return table[Math.abs(h) % table.length];
}
function RelationEdge(props: EdgeProps<EdgeData>) {
  const { id, sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, markerEnd, data, selected } = props;
  const [edgePath, labelX, labelY] = getSmoothStepPath({
    sourceX, sourceY, targetX, targetY, sourcePosition, targetPosition, borderRadius: 8,
  });
  const offset = hashOffsetPx(id);
  const rel = (data?.rel ?? 'custom') as EdgeRel;
  return (
    <>
      <BaseEdge id={id} path={edgePath} markerEnd={markerEnd} interactionWidth={24} />
      <EdgeLabelRenderer>
        <div
          style={{
            position: 'absolute',
            transform: `translate(-50%, -50%) translate(${labelX}px, ${labelY + offset}px)`,
            pointerEvents: 'all',
            zIndex: 10,
          }}
        >
          <Chip
            size="small"
            label={EDGE_LABEL[rel]}
            color={selected ? 'primary' : 'default'}
            sx={{
              fontWeight: 700,
              backgroundImage: 'none',
              backdropFilter: 'none',
              backgroundColor: (t) => (t.palette.mode === 'light' ? 'rgba(255,255,255,0.98)' : 'rgba(15,18,24,0.98)'),
              border: '1px solid',
              borderColor: (t) => t.palette.divider,
            }}
          />
        </div>
      </EdgeLabelRenderer>
    </>
  );
}
const edgeTypes = { relation: RelationEdge };

/* ---- Generic JSON Schema → small form ---- */
function SchemaForm({
  schema,
  value,
  onChange,
}: {
  schema: any;
  value: Record<string, any>;
  onChange: (patch: Record<string, any>) => void;
}) {
  const props = schema?.properties || {};
  const entries = Object.entries<any>(props);
  if (!entries.length) {
    return (
      <Typography variant="body2" sx={{ opacity: 0.75 }}>
        No configurable fields.
      </Typography>
    );
  }
  return (
    <Stack spacing={1}>
      {entries.map(([key, def]) => {
        const t = def.type || 'string';
        const label = def.title || key;
        const help = def.description || '';
        const v = value?.[key];
        if (def.enum) {
          return (
            <FormControl key={key} fullWidth size="small">
              <InputLabel id={`${key}-lbl`}>{label}</InputLabel>
              <Select
                labelId={`${key}-lbl`}
                label={label}
                value={v ?? ''}
                onChange={(e) => onChange({ [key]: e.target.value })}
                MenuProps={OPAQUE_MENU_PROPS as any}
              >
                {def.enum.map((opt: string) => (
                  <MenuItem key={opt} value={opt}>
                    {opt}
                  </MenuItem>
                ))}
              </Select>
              {!!help && (
                <Typography variant="caption" sx={{ mt: 0.25, opacity: 0.75 }}>
                  {help}
                </Typography>
              )}
            </FormControl>
          );
        }
        if (t === 'boolean') {
          return (
            <Stack key={key} direction="row" alignItems="center" spacing={1}>
              <Chip size="small" label={label} />
              <Button
                size="small"
                variant={v ? 'contained' : 'outlined'}
                onClick={() => onChange({ [key]: !v })}
              >
                {v ? 'On' : 'Off'}
              </Button>
            </Stack>
          );
        }
        // number/integer/string
        return (
          <Box key={key}>
            <TextField
              fullWidth
              size="small"
              type={t === 'number' || t === 'integer' ? 'number' : 'text'}
              label={label}
              value={v ?? ''}
              onChange={(e) =>
                onChange({
                  [key]:
                    t === 'number' || t === 'integer'
                      ? Number(e.target.value)
                      : e.target.value,
                })
              }
            />
            {!!help && (
              <Typography variant="caption" sx={{ mt: 0.25, opacity: 0.75 }}>
                {help}
              </Typography>
            )}
          </Box>
        );
      })}
    </Stack>
  );
}

/* OPAQUE menu for all Selects in this page */
const OPAQUE_MENU_PROPS = {
  PaperProps: {
    sx: {
      backdropFilter: 'none',
      backgroundImage: 'none',
      backgroundColor: (t: any) => (t.palette.mode === 'light' ? 'rgba(255,255,255,1)' : 'rgba(17,25,40,0.98)'),
      border: '1px solid',
      borderColor: (t: any) => t.palette.divider,
    },
  },
};
/* =======================
   Step 1 Inner Component
======================= */
function Step1Inner({
  goPrev,
  goNext,
}: {
  goPrev?: () => void;
  goNext?: () => void;
}) {
  const theme = useTheme();
  const { projectType: ctxProjectType } = useStudio();

  const projectType =
    (ctxProjectType ||
      (typeof window !== 'undefined'
        ? (localStorage.getItem('cerulea.projectType') as ProjectType | null)
        : null) ||
      'dapp') as ProjectType;

  /* ------ Module Library ------ */
  const [library, setLibrary] = React.useState<Module[]>([]);
  const [libLoading, setLibLoading] = React.useState(false);
  const [libError, setLibError] = React.useState<string | null>(null);

  const libById = React.useMemo(
    () => new Map(library.map((m) => [m.moduleId, m] as const)),
    [library]
  );

  React.useEffect(() => {
    let ignore = false;
    (async () => {
      setLibLoading(true);
      setLibError(null);
      try {
        const r = await fetch(`/api/modules?projectType=${projectType}`);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const data = await safeJson<Module[]>(r);
        if (!ignore) setLibrary(data);
      } catch (e: any) {
        if (!ignore) setLibError(e?.message ?? 'Failed to load modules');
      } finally {
        if (!ignore) setLibLoading(false);
      }
    })();
    return () => {
      ignore = true;
    };
  }, [projectType]);

  /* ------ Canvas state ------ */
  const [nodes, setNodes, onNodesChange] = useNodesState<Node[]>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge<EdgeData>[]>([]);
  const [selectedNodeId, setSelectedNodeId] = React.useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = React.useState<string | null>(null);

  const rf = useReactFlow();

  /* ------ Persist: local & server (debounced) ------ */
  function saveLocal(n = nodes, e = edges) {
    if (typeof window === 'undefined') return;
    localStorage.setItem('cerulea.step1.graph', JSON.stringify({ nodes: n, edges: e }));
  }
  const DEBOUNCE_MS = 1200;
  const persistTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null);
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
    const body = { modules: mods, graph: { nodes, edges } };
    try {
      await fetch(`/api/projects/${projectId}/blueprint`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
    } catch (err) {
      console.warn('Persist blueprint failed', err);
    }
  }
  function persistBlueprintDebounced() {
    if (persistTimer.current) clearTimeout(persistTimer.current);
    persistTimer.current = setTimeout(() => {
      persistBlueprintNow();
    }, DEBOUNCE_MS);
  }
  React.useEffect(() => {
    saveLocal(nodes, edges);
    persistBlueprintDebounced();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes, edges]);

  /* ------ Helpers ------ */
  function nodeIdFor(modId: string) {
    return `n_${modId}`;
  }
  function modIdFromNodeId(nodeId: string) {
    return nodeId.startsWith('n_') ? nodeId.slice(2) : nodeId;
  }

  function ensureNodeForModule(mod: Module, position?: { x: number; y: number }): Node {
    const id = nodeIdFor(mod.moduleId);
    const existing = nodes.find((n) => n.id === id);
    if (existing) return existing;
    const newNode: Node = {
      id,
      type: 'module',
      position: position ?? gridPos(nodes.length),
      data: { moduleId: mod.moduleId, title: mod.title },
    };
    setNodes((nds) => [...nds, newNode]);
    return newNode;
  }

  function addModuleById(moduleId: string, position?: { x: number; y: number }) {
    const mod = libById.get(moduleId);
    if (!mod) return;
    ensureNodeForModule(mod, position);
  }

  /* ------ Seed canvas once library is ready ------ */
  const seededRef = React.useRef(false);
  React.useEffect(() => {
    if (seededRef.current) return;
    if (!library.length) return;

    (async () => {
      // 1) restore saved
      const saved = (() => {
        try {
          const raw = localStorage.getItem('cerulea.step1.graph');
          return raw ? (JSON.parse(raw) as { nodes: Node[]; edges: Edge[] }) : null;
        } catch {
          return null;
        }
      })();

      if (saved?.nodes?.length) {
        setNodes(saved.nodes as any);
        setEdges((saved.edges as any) || []);
        seededRef.current = true;
        return;
      }

      // 2) seed from templateModules
      const rawIds = localStorage.getItem('cerulea.templateModules');
      let ids: string[] = [];
      try {
        ids = rawIds ? (JSON.parse(rawIds) as string[]) : [];
      } catch {
        ids = [];
      }

      // 3) If empty, try selected templates → fetch to union preinstalledModules
      if (!ids.length) {
        const rawTpl = localStorage.getItem('cerulea.selectedTemplateIds');
        let tplIds: string[] = [];
        try {
          tplIds = rawTpl ? (JSON.parse(rawTpl) as string[]) : [];
        } catch {
          tplIds = [];
        }

        if (tplIds.length) {
          try {
            const r = await fetch(`/api/templates?ids=${tplIds.join(',')}`);
            const tplData = await safeJson<TemplateLite[]>(r);
            const union = new Set<string>();
            tplData.forEach((t) => (t.preinstalledModules || []).forEach((m) => union.add(m)));
            ids = Array.from(union);
          } catch {
            /* ignore; fall through with empty ids */
          }
        }
      }

      const uniqIds = uniq(ids);
      const toPlace = uniqIds.map((id) => libById.get(id)).filter(Boolean) as Module[];
      const newNodes: Node[] = toPlace.map((m, i) => ({
        id: nodeIdFor(m.moduleId),
        type: 'module',
        position: gridPos(i),
        data: { moduleId: m.moduleId, title: m.title },
      }));
      if (newNodes.length) {
        setNodes(newNodes);
        setEdges([]);
      }
      seededRef.current = true;
    })();
  }, [library]); // eslint-disable-line react-hooks/exhaustive-deps

  /* ------ Drag from library → canvas ------ */
  const onDragStart = React.useCallback((event: React.DragEvent, modId: string) => {
    event.dataTransfer.setData('application/reactflow', modId);
    event.dataTransfer.effectAllowed = 'move';
  }, []);
  const onDrop = React.useCallback(
    (event: React.DragEvent) => {
      event.preventDefault();
      const modId = event.dataTransfer.getData('application/reactflow');
      if (!modId) return;
      const pos = rf.screenToFlowPosition({ x: event.clientX, y: event.clientY });
      addModuleById(modId, pos);
    },
    [rf, library, nodes] // eslint-disable-line react-hooks/exhaustive-deps
  );
  const onDragOver = React.useCallback((event: React.DragEvent) => {
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
  }, []);

  /* ------ Connect, click, delete edges/nodes ------ */
  const onConnect = React.useCallback(
    (connection: Connection) => {
      const e: Edge<EdgeData> = addEdge(
        {
          ...connection,
          type: 'relation',
          markerEnd: { type: MarkerType.ArrowClosed },
          data: { rel: 'custom' },
        },
        edges
      ) as Edge<EdgeData>;
      setEdges(e as any);
    },
    [edges]
  );

  const onNodeClick = React.useCallback((_: any, node: Node) => {
    setSelectedEdgeId(null);
    setSelectedNodeId(node.id);
  }, []);

  const onEdgeClick = React.useCallback((_: any, edge: Edge) => {
    setSelectedNodeId(null);
    setSelectedEdgeId(edge.id);
  }, []);

  const deleteSelectedEdge = React.useCallback(() => {
    if (!selectedEdgeId) return;
    setEdges((eds) => eds.filter((e) => e.id !== selectedEdgeId));
    setSelectedEdgeId(null);
  }, [selectedEdgeId]);

  const clearCanvas = React.useCallback(() => {
    setNodes([]);
    setEdges([]);
    setSelectedEdgeId(null);
    setSelectedNodeId(null);
  }, []);

  /* ------ Library filters (compact) ------ */
  const [q, setQ] = React.useState('');
  const [tagCat, setTagCat] = React.useState<string>('');
  const [activeTags, setActiveTags] = React.useState<Set<string>>(new Set());

  const visibleTags = React.useMemo(() => (tagCat ? TAG_CATEGORIES[tagCat] || [] : []), [tagCat]);

  const filteredLib = React.useMemo(() => {
    let arr = library;
    if (q.trim()) {
      const needle = q.trim().toLowerCase();
      arr = arr.filter((m) => {
        const hay = [
          m.title,
          m.blurb ?? '',
          (m.tags ?? []).join(' '),
          ...(m.longDescription ?? []),
          m.category ?? '',
          m.moduleId,
        ]
          .join(' ')
          .toLowerCase();
        return hay.includes(needle);
      });
    }
    if (activeTags.size) {
      arr = arr.filter((m) => (m.tags || []).some((t) => activeTags.has(t)));
    }
    return arr;
  }, [library, q, activeTags]);

  function toggleTag(tag: string) {
    setActiveTags((prev) => {
      const next = new Set(prev);
      if (next.has(tag)) next.delete(tag);
      else next.add(tag);
      return next;
    });
  }

  /* ------ Selection derived data ------ */
  const selectedNode = React.useMemo(() => nodes.find((n) => n.id === selectedNodeId) || null, [nodes, selectedNodeId]);
  const selectedModule: Module | null = React.useMemo(() => {
    if (!selectedNode) return null;
    const mid = modIdFromNodeId(selectedNode.id);
    return libById.get(mid) || null;
  }, [selectedNode, libById]);
  const selectedEdge = React.useMemo(() => edges.find((e) => e.id === selectedEdgeId) || null, [edges, selectedEdgeId]);

  /* ------ Update node data helpers ------ */
  function updateNodeData(nodeId: string, patch: Record<string, any>) {
    setNodes((nds) => nds.map((n) => (n.id === nodeId ? { ...n, data: { ...(n.data as any), ...patch } } : n)));
  }

  /* ------ Add dependency quickly ------ */
  function addMissing(depId: string) {
    const mod = libById.get(depId);
    if (!mod) return;
    const newNode = ensureNodeForModule(mod);
    if (selectedNode) {
      const e: Edge<EdgeData> = {
        id: `e_${selectedNode.id}_${newNode.id}_${Date.now()}`,
        source: selectedNode.id,
        target: newNode.id,
        type: 'relation',
        markerEnd: { type: MarkerType.ArrowClosed },
        data: { rel: 'reads' },
      };
      setEdges((eds) => [...eds, e]);
    }
  }

  const isLightBg = theme.palette.mode === 'light';

  /* ------ Navigation (prefer shell callbacks over URL) ------ */
  const goToStep = React.useCallback((n: number) => {
    if (typeof window === 'undefined') return;
    const path = window.location.pathname;
    const parts = path.split('/');
    const ix = parts.findIndex((p) => /^step-\d+$/.test(p));
    if (ix >= 0) parts[ix] = `step-${n}`;
    else parts.push(`step-${n}`);
    window.location.href = parts.join('/');
  }, []);

  const handleBack = React.useCallback(() => {
    if (goPrev) return goPrev(); // render step0.tsx via shell
    goToStep(0); // fallback
  }, [goPrev, goToStep]);

  const handleNext = React.useCallback(async () => {
    await persistBlueprintNow();
    if (goNext) return goNext(); // render step2.tsx via shell
    goToStep(2); // fallback
  }, [goNext, goToStep]);

  /* ------ Render ------ */
  return (
    <Box sx={{ height: 'calc(100vh - 140px)' }}>
      <Stack spacing={1.25} sx={{ height: '100%', px: 2, pb: 2 }}>
        {/* Minimal top toolbar (like Step 0) */}
        <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ px: 0.5 }}>
          <Button
            variant="text"
            size="small"
            onClick={handleBack}
            startIcon={<KeyboardArrowLeftIcon />}
            sx={{ minWidth: 0, px: 0.5, fontWeight: 700, textTransform: 'none' }}
          >
            Back
          </Button>
          <Button
            variant="text"
            size="small"
            onClick={handleNext}
            endIcon={<KeyboardArrowRightIcon />}
            sx={{ minWidth: 0, px: 0.5, fontWeight: 700, textTransform: 'none' }}
          >
            Next
          </Button>
        </Stack>

        {/* Main content row: Library | Canvas | Module Console */}
        <Stack direction="row" spacing={2} sx={{ flex: 1, minHeight: 0 }}>
          {/* Left: Module Library */}
          <Paper
            variant="outlined"
            sx={{
              width: 360,
              minWidth: 320,
              maxWidth: 420,
              alignSelf: 'stretch',
              p: 2,
              borderRadius: 3,
              display: 'flex',
              flexDirection: 'column',
              background: isLightBg
                ? 'linear-gradient( to bottom right, rgba(255,255,255,0.9), rgba(255,255,255,0.7) )'
                : 'linear-gradient( to bottom right, rgba(255,255,255,0.08), rgba(255,255,255,0.04) )',
              backdropFilter: 'blur(8px) saturate(120%)',
            }}
          >
            <Typography variant="h6" fontWeight={800} gutterBottom>
              Module Library
            </Typography>

            <Typography variant="body2" sx={{ mb: 1.5, opacity: 0.8 }}>
              Module Library is a curated catalog of building blocks for your project. A module can be an on-chain
              contract (e.g., ERC-20), a backend capability (e.g., email delivery), or an application feature (e.g., subscriptions).
            </Typography>

            <TextField
              fullWidth
              size="small"
              placeholder="Search modules"
              value={q}
              onChange={(e) => setQ(e.target.value)}
              sx={{ mb: 1.25 }}
            />

            <FormControl fullWidth size="small" sx={{ mb: 1 }}>
              <InputLabel id="tagcat">Filter by category</InputLabel>
              <Select
                labelId="tagcat"
                label="Filter by category"
                value={tagCat}
                onChange={(e) => {
                  setTagCat(e.target.value);
                  setActiveTags(new Set());
                }}
                MenuProps={OPAQUE_MENU_PROPS as any}
              >
                <MenuItem value="">(none)</MenuItem>
                {Object.keys(TAG_CATEGORIES).map((c) => (
                  <MenuItem key={c} value={c}>
                    {c}
                  </MenuItem>
                ))}
              </Select>
            </FormControl>

            {!!(tagCat && visibleTags.length) && (
              <Stack direction="row" spacing={0.5} useFlexGap flexWrap="wrap" sx={{ mb: 1 }}>
                {visibleTags.map((t) => (
                  <Chip
                    key={t}
                    size="small"
                    label={t}
                    onClick={() => toggleTag(t)}
                    color={activeTags.has(t) ? 'primary' : 'default'}
                  />
                ))}
              </Stack>
            )}

            <Divider sx={{ my: 1 }} />

            {/* Scrollable list area */}
            <Box sx={{ flex: 1, minHeight: 0, overflowY: 'auto', pr: 1 }}>
              {libLoading ? (
                <Stack alignItems="center" justifyContent="center" sx={{ py: 4 }}>
                  <CircularProgress size={22} />
                </Stack>
              ) : libError ? (
                <Alert severity="error" sx={{ my: 1 }}>
                  {libError}
                </Alert>
              ) : (
                <Stack spacing={1.25}>
                  {filteredLib.map((m) => {
                    const inCanvas = nodes.some((n) => n.id === nodeIdFor(m.moduleId));
                    return (
                      <Paper
                        key={m.moduleId}
                        variant="outlined"
                        draggable
                        onDragStart={(e) => onDragStart(e, m.moduleId)}
                        onClick={() => addModuleById(m.moduleId)}
                        sx={{
                          width: '100%',
                          boxSizing: 'border-box',
                          p: 1.25,
                          borderRadius: 2,
                          cursor: 'grab',
                          '&:active': { cursor: 'grabbing' },
                          background: isLightBg
                            ? 'linear-gradient( to bottom right, rgba(255,255,255,0.95), rgba(255,255,255,0.8) )'
                            : 'linear-gradient( to bottom right, rgba(255,255,255,0.1), rgba(255,255,255,0.06) )',
                          backdropFilter: 'blur(6px)',
                          borderColor: inCanvas ? 'primary.main' : undefined,
                          overflow: 'hidden',
                        }}
                      >
                        <Typography
                          variant="subtitle2"
                          fontWeight={800}
                          sx={{ mb: 0.25, textAlign: 'center', wordBreak: 'break-word', overflowWrap: 'anywhere' }}
                        >
                          {m.title}
                        </Typography>
                        <Typography
                          variant="caption"
                          sx={{
                            display: 'block',
                            mb: 0.75,
                            textAlign: 'center',
                            opacity: 0.8,
                            wordBreak: 'break-word',
                            overflowWrap: 'anywhere',
                          }}
                        >
                          {blurbFromJson(m)}
                        </Typography>
                        <Stack
                          direction="row"
                          spacing={0.5}
                          useFlexGap
                          flexWrap="wrap"
                          justifyContent="center"
                          sx={{ maxWidth: '100%' }}
                        >
                          {(m.tags || []).slice(0, 6).map((t) => (
                            <Chip key={t} size="small" label={t} />
                          ))}
                        </Stack>
                      </Paper>
                    );
                  })}
                </Stack>
              )}
            </Box>
          </Paper>

          {/* Middle: Canvas */}
          <Paper
            variant="outlined"
            sx={{
              flex: 1,
              minWidth: 520,
              alignSelf: 'stretch',
              p: 0,
              borderRadius: 3,
              position: 'relative',
              overflow: 'hidden',
              background: isLightBg
                ? 'linear-gradient( to bottom right, rgba(255,255,255,0.7), rgba(255,255,255,0.5) )'
                : 'linear-gradient( to bottom right, rgba(255,255,255,0.06), rgba(255,255,255,0.03) )',
              backdropFilter: 'blur(8px)',
            }}
          >
            <Typography
              variant="h6"
              fontWeight={800}
              sx={{ position: 'absolute', left: '50%', top: 10, transform: 'translateX(-50%)', zIndex: 5 }}
            >
              Canvas
            </Typography>

            <Button
              size="small"
              startIcon={<DeleteOutlineIcon />}
              onClick={clearCanvas}
              sx={{ position: 'absolute', right: 12, top: 12, zIndex: 5 }}
            >
              Clear canvas
            </Button>

            <Box sx={{ height: '100%', width: '100%' }} onDrop={onDrop} onDragOver={onDragOver}>
              <ReactFlow
                nodes={nodes}
                edges={edges}
                onNodesChange={onNodesChange}
                onEdgesChange={onEdgesChange}
                onConnect={onConnect}
                onNodeClick={onNodeClick}
                onEdgeClick={onEdgeClick}
                nodeTypes={nodeTypes as any}
                edgeTypes={edgeTypes as any}
                fitView
              >
                <Background variant={BackgroundVariant.Dots} gap={18} size={1} />
                <Controls />
                <MiniMap />
              </ReactFlow>
            </Box>
          </Paper>

          {/* Right: Module Console */}
          <Paper
            variant="outlined"
            sx={{
              width: 380,
              minWidth: 340,
              maxWidth: 440,
              alignSelf: 'stretch',
              p: 2,
              borderRadius: 3,
              background: isLightBg
                ? 'linear-gradient( to bottom right, rgba(255,255,255,0.92), rgba(255,255,255,0.76) )'
                : 'linear-gradient( to bottom right, rgba(255,255,255,0.1), rgba(255,255,255,0.06) )',
              backdropFilter: 'blur(8px)',
            }}
          >
            <Typography variant="h6" fontWeight={800} align="center" gutterBottom>
              Module Console
            </Typography>

            {!selectedNodeId && !selectedEdgeId && (
              <Stack spacing={1.25} sx={{ opacity: 0.9 }}>
                <Typography variant="body2">• Click or drag a module from the library to add it to the canvas.</Typography>
                <Typography variant="body2">
                  • Select a module to see guidance and configure its behavior. Your choices are saved in the blueprint and carried into code generation and deployment.
                </Typography>
                <Typography variant="body2">
                  • Draw connections between modules to label relationships: <b>reads</b>, <b>writes</b>, <b>triggers</b>, <b>feeds</b>, <b>calls</b>, <b>pays</b>.
                </Typography>
              </Stack>
            )}

            {!!selectedNodeId && selectedModule && (
              <NodeDetailsPanel
                nodeId={selectedNodeId}
                mod={selectedModule}
                onChangeGroup={(group) => updateNodeData(selectedNodeId, { group })}
                onChangeConfig={(patch) =>
                  updateNodeData(selectedNodeId, {
                    config: {
                      ...(((nodes.find((n) => n.id === selectedNodeId)?.data as any)?.config) || {}),
                      ...patch,
                    },
                  })
                }
                onAddMissing={addMissing}
              />
            )}

            {!!selectedEdgeId && selectedEdge && (
              <EdgeDetailsPanel
                edge={selectedEdge}
                onChangeRel={(rel) =>
                  setEdges((eds) =>
                    eds.map((e) => (e.id === selectedEdgeId ? { ...e, data: { ...(e.data || {}), rel } } : e))
                  )
                }
                onDelete={deleteSelectedEdge}
              />
            )}
          </Paper>
        </Stack>
      </Stack>
    </Box>
  );
}
/* =======================
   Right Panel Sections
======================= */

function NodeDetailsPanel({
  nodeId,
  mod,
  onChangeGroup,
  onChangeConfig,
  onAddMissing,
}: {
  nodeId: string;
  mod: Module;
  onChangeGroup: (group?: string) => void;
  onChangeConfig: (patch: Record<string, any>) => void;
  onAddMissing: (depId: string) => void;
}) {
  const [group, setGroup] = React.useState<string>('');
  const [cfg, setCfg] = React.useState<Record<string, any>>({});

  React.useEffect(() => {
    // Reset local form state when node/module changes
    setGroup('');
    setCfg({});
  }, [nodeId, mod.moduleId]);

  function depReason(depId: string): string {
    const key1 = `${mod.moduleId}->${depId}`;
    const key2 = `${depId}->${mod.moduleId}`;
    return (
      mod.reasonByDepId?.[depId] ||
      DEP_REASON[key1] ||
      DEP_REASON[key2] ||
      'This module is required.'
    );
  }

  return (
    <Stack spacing={1.25}>
      <Typography variant="subtitle1" fontWeight={800} align="center">
        {mod.title}
      </Typography>

      {!!mod.blurb && (
        <Typography variant="body2" sx={{ opacity: 0.9 }}>
          {mod.blurb}
        </Typography>
      )}

      {(mod.longDescription || []).slice(0, 2).map((p, i) => (
        <Typography key={i} variant="body2" sx={{ opacity: 0.85 }}>
          {p}
        </Typography>
      ))}

      {/* Grouping */}
      <Divider sx={{ my: 1 }} />
      <Typography variant="overline" sx={{ opacity: 0.8 }}>
        Group
      </Typography>
      <TextField
        fullWidth
        size="small"
        placeholder="e.g., Auth, NFTs, Payments"
        value={group}
        onChange={(e) => setGroup(e.target.value)}
        onBlur={() => onChangeGroup(group || undefined)}
      />

      {/* Dependencies */}
      {!!(mod.dependsOn && mod.dependsOn.length) && (
        <>
          <Divider sx={{ my: 1 }} />
          <Typography variant="overline" sx={{ opacity: 0.8 }}>
            Dependencies
          </Typography>
          <Stack spacing={0.75}>
            {mod.dependsOn!.map((depId) => (
              <Paper key={depId} variant="outlined" sx={{ p: 1, borderRadius: 2, backgroundImage: 'none' }}>
                <Stack direction="row" alignItems="center" spacing={1} justifyContent="space-between">
                  <Stack spacing={0.25} sx={{ pr: 1 }}>
                    <Typography variant="body2" fontWeight={700}>
                      {depId}
                    </Typography>
                    <Typography variant="caption" sx={{ opacity: 0.8 }}>
                      {depReason(depId)}
                    </Typography>
                  </Stack>
                  <Button size="small" variant="outlined" onClick={() => onAddMissing(depId)}>
                    Add
                  </Button>
                </Stack>
              </Paper>
            ))}
          </Stack>
        </>
      )}

      {/* Configuration */}
      <Divider sx={{ my: 1 }} />
      <Typography variant="overline" sx={{ opacity: 0.8 }}>
        Configuration
      </Typography>
      <SchemaForm
        schema={mod.configSchema || { type: 'object', properties: {} }}
        value={cfg}
        onChange={(patch) => {
          const next = { ...cfg, ...patch };
          setCfg(next);
          onChangeConfig(patch);
        }}
      />
    </Stack>
  );
}

function EdgeDetailsPanel({
  edge,
  onChangeRel,
  onDelete,
}: {
  edge: Edge<EdgeData>;
  onChangeRel: (rel: EdgeRel) => void;
  onDelete: () => void;
}) {
  const relVal = (edge.data?.rel || 'custom') as EdgeRel;
  return (
    <Stack spacing={1.25}>
      <Typography variant="subtitle1" fontWeight={800} align="center">
        Connection
      </Typography>
      <Typography variant="body2" sx={{ opacity: 0.85 }}>
        Label this connection to describe how data or actions flow between modules. These hints improve defaults in later steps.
      </Typography>

      <FormControl fullWidth size="small">
        <InputLabel id="rel-type">Relationship</InputLabel>
        <Select
          labelId="rel-type"
          label="Relationship"
          value={relVal}
          onChange={(e) => onChangeRel(e.target.value as EdgeRel)}
          MenuProps={OPAQUE_MENU_PROPS as any}
        >
          <MenuItem value="reads">reads</MenuItem>
          <MenuItem value="writes">writes</MenuItem>
          <MenuItem value="triggers">triggers</MenuItem>
          <MenuItem value="feeds">feeds</MenuItem>
          <MenuItem value="calls">calls</MenuItem>
          <MenuItem value="pays">pays</MenuItem>
          <MenuItem value="custom">relates to</MenuItem>
        </Select>
      </FormControl>

      <Divider />

      <Button color="error" variant="outlined" onClick={onDelete}>
        Delete connection
      </Button>
    </Stack>
  );
}

/* =======================
   Page Wrapper / Export
======================= */
export default function Step1({
  goPrev,
  goNext,
}: {
  goPrev?: () => void;
  goNext?: () => void;
}) {
  return (
    <ReactFlowProvider>
      <Box sx={{ p: 2 }}>
        <Step1Inner goPrev={goPrev} goNext={goNext} />
      </Box>
    </ReactFlowProvider>
  );
}
