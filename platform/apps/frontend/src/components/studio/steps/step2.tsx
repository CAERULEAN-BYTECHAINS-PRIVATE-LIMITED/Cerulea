'use client';

import React, { useEffect, useMemo, useState } from "react";
import {
  Box, Stack, Paper, Typography, TextField, Button, IconButton,
  Divider, Select, MenuItem, Tooltip, Fade, Chip, Switch,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Dialog, DialogTitle, DialogContent, DialogActions, InputAdornment,
  Accordion, AccordionSummary, AccordionDetails,
} from "@mui/material";
import { useTheme, styled, alpha } from "@mui/material/styles";
import { useStudio } from "@/context/StudioContext";
import { useRouter } from "next/navigation";

// Icons
import ArrowBackIcon from "@mui/icons-material/KeyboardArrowLeft";
import ArrowForwardIcon from "@mui/icons-material/KeyboardArrowRight";
import AddIcon from "@mui/icons-material/Add";
import DeleteOutlineIcon from "@mui/icons-material/DeleteOutline";
import StorageIcon from "@mui/icons-material/Storage";
import SecurityIcon from "@mui/icons-material/Security";
import BoltIcon from "@mui/icons-material/Bolt";
import PublicIcon from "@mui/icons-material/Public";
import CodeIcon from "@mui/icons-material/Code";
import AccountTreeIcon from "@mui/icons-material/AccountTree";
import KeyIcon from '@mui/icons-material/Key';
import CloseIcon from '@mui/icons-material/Close';
import QuestionMarkIcon from '@mui/icons-material/QuestionMark';
import AutoFixHighIcon from '@mui/icons-material/AutoFixHigh';
import SearchIcon from "@mui/icons-material/Search";
import HexagonOutlinedIcon from "@mui/icons-material/HexagonOutlined";
import ExpandMoreIcon from "@mui/icons-material/ExpandMore";
import InfoOutlinedIcon from "@mui/icons-material/InfoOutlined";

// Data
import ENTITY_PRESETS_RAW from "@/data/module-entity-presets.json";
import MODULES_SEED_RAW from "@/data/modules.seed.json";

// Internal Components
import RelationshipCanvas, { RelationshipDef as Relationship } from "../logic/RelationshipCanvas";
import LogicCanvas from "../logic/LogicCanvas";
import CustomScriptPanel from "../custom/CustomScriptPanel";

/* ------------------------------------------------------------------ */
/* Types                                                               */
/* ------------------------------------------------------------------ */
type Phase = "data" | "governance" | "behavior" | "exposure";

type DataType =
  | "uuid" | "string" | "text" | "boolean" | "int" | "float"
  | "datetime" | "json"
  | "address" | "uint256" | "bytes32" | "ipfs-hash";

type StorageStrategy = "database" | "on-chain" | "ipfs";

type Field = {
  id: string;
  name: string;
  type: DataType;
  storage: StorageStrategy;
  required: boolean;
  unique: boolean;
  indexed: boolean;
  encrypted: boolean;
  description?: string;
  defaultValue?: string;
};

type Entity = {
  id: string;
  name: string;
  description?: string;
  fields: Field[];
  isCore?: boolean;
};

type ModuleInfo = { id: string; label: string; category?: string };

function uid() { return Math.random().toString(36).slice(2, 10); }

/* ------------------------------------------------------------------ */
/* Helpers: map JSON preset field → Field type                        */
/* ------------------------------------------------------------------ */
function mapFieldType(f: any): DataType {
  if (f.format === "eth-address") return "address";
  if (f.type === "uuid") return "uuid";
  if (f.type === "string") return "string";
  if (f.type === "text") return "text";
  if (f.type === "boolean") return "boolean";
  if (f.type === "integer" || f.type === "int") return "int";
  if (f.type === "float" || f.type === "number") return "float";
  if (f.type === "datetime") return "datetime";
  if (f.type === "json") return "json";
  if (f.type === "address") return "address";
  if (f.type === "uint256") return "uint256";
  if (f.type === "bytes32") return "bytes32";
  if (f.type === "ipfs-hash") return "ipfs-hash";
  return "string";
}

function mapStorage(f: any): StorageStrategy {
  if (f.type === "ipfs-hash") return "ipfs";
  if (f.format === "eth-address" || f.type === "address" || f.type === "uint256" || f.type === "bytes32") return "on-chain";
  return "database";
}

function presetToEntity(preset: any): Entity {
  return {
    id: uid(),
    name: preset.name,
    description: preset.description || "",
    isCore: true,
    fields: (preset.fields || []).map((f: any): Field => ({
      id: uid(),
      name: f.name,
      type: mapFieldType(f),
      storage: mapStorage(f),
      required: f.nullable === false || f.name === "id",
      unique: f.name === "id" || !!f.unique,
      indexed: f.name === "id" || (f.name || "").endsWith("Id") || f.format === "eth-address",
      encrypted: f.name === "email" || f.name === "password" || f.name === "privateKey",
      defaultValue: (f.name === "createdAt" || f.name === "updatedAt") ? "now()" : undefined,
      description: f.description,
    })),
  };
}

/* ------------------------------------------------------------------ */
/* Catalog helpers                                                     */
/* ------------------------------------------------------------------ */
const PRESETS = (ENTITY_PRESETS_RAW as any).presets as Record<string, any[]>;

const MODULE_LABELS: Record<string, string> = {};
((MODULES_SEED_RAW as any).modules || []).forEach((m: any) => {
  MODULE_LABELS[m.moduleId] = m.title;
});

const MODULE_CATEGORIES: Record<string, string> = {};
((MODULES_SEED_RAW as any).modules || []).forEach((m: any) => {
  MODULE_CATEGORIES[m.moduleId] = m.category || "other";
});

/* ------------------------------------------------------------------ */
/* Styled Components                                                   */
/* ------------------------------------------------------------------ */
const FloatingIsland = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.95)' : 'rgba(20,20,23,0.95)',
  backdropFilter: 'blur(16px)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 20px 40px -8px rgba(0,0,0,0.3)',
  borderRadius: 100,
  padding: '8px 24px',
  display: 'flex',
  alignItems: 'center',
  gap: 16,
  zIndex: 1000,
  pointerEvents: 'auto',
}));

const StepPill = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.9)' : 'rgba(20,20,23,0.9)',
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

const PhaseSidebar = styled(Box)(({ theme }) => ({
  width: 220,
  height: '100%',
  borderRight: `1px solid ${theme.palette.divider}`,
  display: 'flex',
  flexDirection: 'column',
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.5)' : 'rgba(10,10,12,0.5)',
  backdropFilter: 'blur(20px)',
  paddingTop: 80,
  overflowY: 'auto',
}));

const PhaseItem = styled(Box, { shouldForwardProp: (p) => p !== 'active' })<{ active?: boolean }>(({ theme, active }) => ({
  padding: '12px 20px',
  cursor: 'pointer',
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  borderLeft: `3px solid ${active ? theme.palette.primary.main : 'transparent'}`,
  background: active ? alpha(theme.palette.primary.main, 0.05) : 'transparent',
  color: active ? theme.palette.primary.main : theme.palette.text.secondary,
  transition: 'all 0.2s ease',
  '&:hover': {
    background: active ? alpha(theme.palette.primary.main, 0.08) : alpha(theme.palette.action.hover, 0.5),
    color: active ? theme.palette.primary.main : theme.palette.text.primary,
  },
}));

const Workspace = styled(Box)(() => ({
  flex: 1,
  height: '100%',
  position: 'relative',
  overflow: 'hidden',
  display: 'flex',
  flexDirection: 'column',
  paddingTop: 80,
}));

const OPAQUE_MENU_PROPS = {
  PaperProps: {
    sx: {
      backgroundImage: 'none',
      backgroundColor: (t: any) => t.palette.mode === 'light' ? '#ffffff' : '#1e1e20',
      border: '1px solid',
      borderColor: 'divider',
      boxShadow: '0 8px 32px rgba(0,0,0,0.3)',
    },
  },
};

/* ------------------------------------------------------------------ */
/* Component                                                           */
/* ------------------------------------------------------------------ */
/* ------------------------------------------------------------------ */
/* Fallback entity generator for modules without preset data          */
/* ------------------------------------------------------------------ */
function generateFallbackEntities(mod: ModuleInfo): Entity[] {
  const label = mod.label || mod.id;

  const makeFields = (...extra: Omit<Field, 'id'>[]): Field[] => [
    { id: uid(), name: 'id', type: 'uuid', storage: 'database', required: true, unique: true, indexed: true, encrypted: false },
    { id: uid(), name: 'createdAt', type: 'datetime', storage: 'database', required: false, unique: false, indexed: true, encrypted: false, defaultValue: 'now()' },
    { id: uid(), name: 'updatedAt', type: 'datetime', storage: 'database', required: false, unique: false, indexed: false, encrypted: false, defaultValue: 'now()' },
    ...extra.map(f => ({ ...f, id: uid() })),
  ];

  return [
    {
      id: uid(),
      name: label,
      description: `Core data model for ${label}`,
      isCore: true,
      fields: makeFields(
        { name: 'name', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'status', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false, defaultValue: 'active' },
        { name: 'ownerId', type: 'uuid', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'metadata', type: 'json', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
      ),
    },
    {
      id: uid(),
      name: `${label}Event`,
      description: `Blockchain events emitted by ${label}`,
      isCore: true,
      fields: makeFields(
        { name: 'eventType', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'actor', type: 'address', storage: 'on-chain', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'payload', type: 'json', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
        { name: 'blockNumber', type: 'int', storage: 'on-chain', required: false, unique: false, indexed: true, encrypted: false },
      ),
    },
    {
      id: uid(),
      name: `${label}Config`,
      description: `Configuration settings for ${label}`,
      isCore: true,
      fields: makeFields(
        { name: 'key', type: 'string', storage: 'database', required: true, unique: true, indexed: true, encrypted: false },
        { name: 'value', type: 'text', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
        { name: 'isActive', type: 'boolean', storage: 'database', required: true, unique: false, indexed: false, encrypted: false, defaultValue: 'true' },
        { name: 'expiresAt', type: 'datetime', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
      ),
    },
    {
      id: uid(),
      name: `${label}Permission`,
      description: `Role-based access control for ${label}`,
      isCore: false,
      fields: makeFields(
        { name: 'role', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'walletAddress', type: 'address', storage: 'on-chain', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'canRead', type: 'boolean', storage: 'database', required: true, unique: false, indexed: false, encrypted: false, defaultValue: 'true' },
        { name: 'canWrite', type: 'boolean', storage: 'database', required: true, unique: false, indexed: false, encrypted: false, defaultValue: 'false' },
        { name: 'grantedAt', type: 'datetime', storage: 'database', required: false, unique: false, indexed: false, encrypted: false, defaultValue: 'now()' },
      ),
    },
    {
      id: uid(),
      name: `${label}AuditLog`,
      description: `Immutable audit trail for ${label} operations`,
      isCore: false,
      fields: makeFields(
        { name: 'action', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'actor', type: 'string', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'resourceId', type: 'uuid', storage: 'database', required: true, unique: false, indexed: true, encrypted: false },
        { name: 'changes', type: 'json', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
        { name: 'ipAddress', type: 'string', storage: 'database', required: false, unique: false, indexed: false, encrypted: false },
      ),
    },
  ];
}

export default function Step2({ goPrev, goNext }: { goPrev?: () => void; goNext?: () => void }) {
  const theme = useTheme();
  const { setStudioState, selectedModules: ctxModules } = useStudio() as any;
  const router = useRouter();

  /* ---------- phase navigation ---------- */
  const [phase, setPhase] = useState<Phase>("data");

  /* ---------- entity state ---------- */
  // Per-module entity storage: Record<moduleId, Entity[]>
  const [moduleEntities, setModuleEntities] = useState<Record<string, Entity[]>>({});
  // Modules list from Blueprint (step1)
  const [blueprintModules, setBlueprintModules] = useState<ModuleInfo[]>([]);
  const [selectedModuleId, setSelectedModuleId] = useState<string | null>(null);
  const [selectedEntityId, setSelectedEntityId] = useState<string | null>(null);
  const [addEntityOpen, setAddEntityOpen] = useState(false);
  const [entitySearch, setEntitySearch] = useState('');
  const [relationships, setRelationships] = useState<Relationship[]>([]);
  const [logicMode, setLogicMode] = useState<'visual' | 'code'>('visual');
  const [isHelpOpen, setIsHelpOpen] = useState(false);

  /* ---------- derived ---------- */
  const allEntities = useMemo(
    () => Object.values(moduleEntities).flat(),
    [moduleEntities]
  );
  const currentModuleEntities = useMemo(
    () => (selectedModuleId ? (moduleEntities[selectedModuleId] || []) : []),
    [moduleEntities, selectedModuleId]
  );
  const selectedEntity = useMemo(
    () => allEntities.find((e) => e.id === selectedEntityId) || null,
    [allEntities, selectedEntityId]
  );

  // Which module owns the selected entity
  const selectedEntityModuleId = useMemo(() => {
    if (!selectedEntityId) return null;
    return Object.entries(moduleEntities).find(([, ents]) =>
      ents.some((e) => e.id === selectedEntityId)
    )?.[0] || null;
  }, [selectedEntityId, moduleEntities]);

  /* ---------- seeding ---------- */
  useEffect(() => {
    try {
      let mods: ModuleInfo[] = [];

      // Priority 1: StudioContext selectedModules — always wins if present
      if (Array.isArray(ctxModules) && ctxModules.length > 0) {
        mods = ctxModules.map((id: string) => ({
          id,
          label: MODULE_LABELS[id] || id,
          category: MODULE_CATEGORIES[id],
        }));
      }

      // Priority 2: localStorage step1 graph
      if (mods.length === 0) {
        const step1Raw = localStorage.getItem('cerulea.step1.graph');
        const step1Graph = step1Raw ? JSON.parse(step1Raw) : null;
        if (step1Graph?.nodes) {
          const seen = new Set<string>();
          step1Graph.nodes.forEach((n: any) => {
            const mid = n.data?.moduleId;
            if (mid && mid !== '_custom' && !seen.has(mid)) {
              seen.add(mid);
              mods.push({
                id: mid,
                label: n.data?.label || MODULE_LABELS[mid] || mid,
                category: MODULE_CATEGORIES[mid],
              });
            }
          });
        }
      }

      // Priority 3: template modules
      if (mods.length === 0) {
        const tplRaw = localStorage.getItem('cerulea.templateModules');
        if (tplRaw) {
          const ids: string[] = JSON.parse(tplRaw);
          mods = ids.filter((id) => id !== '_custom').map((id) => ({
            id,
            label: MODULE_LABELS[id] || id,
            category: MODULE_CATEGORIES[id],
          }));
        }
      }

      // Priority 4: saved draft — only if it has real module IDs (not _custom stale data)
      if (mods.length === 0) {
        try {
          const raw = localStorage.getItem('draft:local:3');
          if (raw) {
            const parsed = JSON.parse(raw);
            if (parsed.data?.moduleEntities) {
              const draftMods = Object.keys(parsed.data.moduleEntities).filter((id) => id !== '_custom');
              if (draftMods.length > 0) {
                setModuleEntities(parsed.data.moduleEntities);
                setRelationships(parsed.data.relationships || []);
                setBlueprintModules(
                  draftMods.map((id) => ({
                    id,
                    label: MODULE_LABELS[id] || id,
                    category: MODULE_CATEGORIES[id],
                  }))
                );
                setSelectedModuleId(draftMods[0]);
                const firstEnt = parsed.data.moduleEntities[draftMods[0]]?.[0];
                if (firstEnt) setSelectedEntityId(firstEnt.id);
                return;
              }
            }
          }
        } catch { /* ignore */ }
      }

      // Priority 5: minimum fallback
      if (mods.length === 0) {
        mods = [{ id: 'user-auth', label: 'User Authentication', category: 'identity' }];
      }

      setBlueprintModules(mods);
      setSelectedModuleId(mods[0].id);

      // Try to merge saved per-module entity edits from draft (only for matching module IDs)
      let savedEntities: Record<string, Entity[]> = {};
      try {
        const raw = localStorage.getItem('draft:local:3');
        if (raw) {
          const parsed = JSON.parse(raw);
          if (parsed.data?.moduleEntities) savedEntities = parsed.data.moduleEntities;
          if (parsed.data?.relationships) setRelationships(parsed.data.relationships);
        }
      } catch { /* ignore */ }

      // Seed entities: use saved edits if present for this module, else presets, else fallback
      const seeded: Record<string, Entity[]> = {};
      mods.forEach((mod) => {
        if (savedEntities[mod.id]?.length > 0) {
          seeded[mod.id] = savedEntities[mod.id];
        } else {
          const preset = PRESETS[mod.id] || [];
          seeded[mod.id] = preset.length > 0
            ? preset.map(presetToEntity)
            : generateFallbackEntities(mod);
        }
      });

      setModuleEntities(seeded);
      const firstEnt = seeded[mods[0].id]?.[0];
      if (firstEnt) setSelectedEntityId(firstEnt.id);
    } catch (e) {
      console.warn('Step2 seeding error:', e);
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  /* ---------- save ---------- */
  const handleSave = () => {
    const snapshot = { moduleEntities, relationships };
    localStorage.setItem('draft:local:3', JSON.stringify({ data: snapshot, t: Date.now() }));
    setStudioState({ schemaJson: snapshot } as any);
    if (goNext) goNext();
  };

  /* ---------- entity actions ---------- */
  const addBlankEntity = () => {
    if (!selectedModuleId) return;
    const id = uid();
    const newEnt: Entity = {
      id, name: 'NewEntity', description: '', isCore: false,
      fields: [{ id: uid(), name: 'id', type: 'uuid', storage: 'database', required: true, unique: true, indexed: true, encrypted: false }],
    };
    setModuleEntities((prev) => ({
      ...prev,
      [selectedModuleId]: [...(prev[selectedModuleId] || []), newEnt],
    }));
    setSelectedEntityId(id);
  };

  const deleteEntity = (modId: string, entId: string) => {
    setModuleEntities((prev) => ({
      ...prev,
      [modId]: (prev[modId] || []).filter((e) => e.id !== entId),
    }));
    if (selectedEntityId === entId) setSelectedEntityId(null);
  };

  const updateEntity = (modId: string, entId: string, patch: Partial<Entity>) => {
    setModuleEntities((prev) => ({
      ...prev,
      [modId]: (prev[modId] || []).map((e) => (e.id === entId ? { ...e, ...patch } : e)),
    }));
  };

  const addField = () => {
    if (!selectedEntity || !selectedEntityModuleId) return;
    const newField: Field = {
      id: uid(), name: 'newField', type: 'string', storage: 'database',
      required: false, unique: false, indexed: false, encrypted: false,
    };
    updateEntity(selectedEntityModuleId, selectedEntity.id, {
      fields: [...selectedEntity.fields, newField],
    });
  };

  const updateField = (fieldId: string, patch: Partial<Field>) => {
    if (!selectedEntity || !selectedEntityModuleId) return;
    updateEntity(selectedEntityModuleId, selectedEntity.id, {
      fields: selectedEntity.fields.map((f) => (f.id === fieldId ? { ...f, ...patch } : f)),
    });
  };

  const deleteField = (fieldId: string) => {
    if (!selectedEntity || !selectedEntityModuleId) return;
    updateEntity(selectedEntityModuleId, selectedEntity.id, {
      fields: selectedEntity.fields.filter((f) => f.id !== fieldId),
    });
  };

  const addEntityFromCatalog = (preset: any) => {
    if (!selectedModuleId) return;
    const ent = presetToEntity({ ...preset, isCore: false });
    ent.isCore = false;
    setModuleEntities((prev) => ({
      ...prev,
      [selectedModuleId]: [...(prev[selectedModuleId] || []), ent],
    }));
    setSelectedEntityId(ent.id);
    setAddEntityOpen(false);
  };

  /* ---------- catalog for "Add More Entities" dialog ---------- */
  const catalogEntries = useMemo(() => {
    const currentNames = new Set(currentModuleEntities.map((e) => e.name.toLowerCase()));
    const entries: Array<{ moduleId: string; moduleLabel: string; entity: any }> = [];
    Object.entries(PRESETS).forEach(([moduleId, ents]) => {
      ents.forEach((ent) => {
        if (!currentNames.has(ent.name.toLowerCase())) {
          entries.push({
            moduleId,
            moduleLabel: MODULE_LABELS[moduleId] || moduleId,
            entity: ent,
          });
        }
      });
    });
    return entries;
  }, [currentModuleEntities]);

  const filteredCatalog = useMemo(() => {
    if (!entitySearch) return catalogEntries;
    const q = entitySearch.toLowerCase();
    return catalogEntries.filter(
      (e) =>
        e.entity.name.toLowerCase().includes(q) ||
        e.moduleLabel.toLowerCase().includes(q)
    );
  }, [catalogEntries, entitySearch]);

  // Group by module label for display
  const catalogGroups = useMemo(() => {
    const groups: Record<string, typeof filteredCatalog> = {};
    filteredCatalog.forEach((e) => {
      const key = e.moduleLabel;
      if (!groups[key]) groups[key] = [];
      groups[key].push(e);
    });
    return groups;
  }, [filteredCatalog]);

  /* ---------------------------------------------------------------- */
  /* Render: DATA LAYER (3-column)                                    */
  /* ---------------------------------------------------------------- */
  const renderDataLayer = () => (
    <Box sx={{ height: '100%', display: 'flex', overflow: 'hidden' }}>

      {/* COLUMN 1: Blueprint Modules */}
      <Box
        sx={{
          width: 200, flexShrink: 0,
          borderRight: `1px solid ${theme.palette.divider}`,
          bgcolor: alpha(theme.palette.background.default, 0.4),
          display: 'flex', flexDirection: 'column', overflowY: 'auto',
        }}
      >
        <Box sx={{ p: 2, pb: 1, borderBottom: `1px solid ${theme.palette.divider}` }}>
          <Stack direction="row" alignItems="center" spacing={0.5} mb={0.25}>
            <HexagonOutlinedIcon sx={{ fontSize: 14, color: 'text.secondary' }} />
            <Typography variant="overline" fontWeight={800} fontSize="0.6rem" color="text.secondary">
              BLUEPRINT MODULES
            </Typography>
          </Stack>
          <Typography variant="caption" color="text.secondary" sx={{ lineHeight: 1.3 }}>
            Entities pre-filled from your Blueprint selections.
          </Typography>
        </Box>
        <Box sx={{ flex: 1, py: 1 }}>
          {blueprintModules.length === 0 ? (
            <Box sx={{ p: 2 }}>
              <Typography variant="caption" color="text.disabled">
                Add modules in Blueprint Builder (Step 2) first.
              </Typography>
            </Box>
          ) : (
            blueprintModules.map((mod) => {
              const count = moduleEntities[mod.id]?.length || 0;
              const isActive = selectedModuleId === mod.id;
              return (
                <Box
                  key={mod.id}
                  onClick={() => {
                    setSelectedModuleId(mod.id);
                    const first = moduleEntities[mod.id]?.[0];
                    if (first) setSelectedEntityId(first.id);
                  }}
                  sx={{
                    mx: 1, mb: 0.5, px: 1.5, py: 1.25, borderRadius: 2, cursor: 'pointer',
                    bgcolor: isActive ? alpha(theme.palette.primary.main, 0.1) : 'transparent',
                    border: `1px solid ${isActive ? alpha(theme.palette.primary.main, 0.3) : 'transparent'}`,
                    '&:hover': { bgcolor: isActive ? alpha(theme.palette.primary.main, 0.12) : alpha(theme.palette.action.hover, 0.5) },
                  }}
                >
                  <Typography
                    variant="body2"
                    fontWeight={700}
                    color={isActive ? 'primary.main' : 'text.primary'}
                    sx={{ lineHeight: 1.3 }}
                  >
                    {mod.label}
                  </Typography>
                  <Stack direction="row" alignItems="center" spacing={0.5} mt={0.25}>
                    <Typography variant="caption" color="text.secondary">
                      {count} {count === 1 ? 'entity' : 'entities'}
                    </Typography>
                    {mod.category && (
                      <Chip
                        label={mod.category.replace(/-/g, ' ')}
                        size="small"
                        sx={{ height: 14, fontSize: '0.55rem', fontWeight: 700, opacity: 0.7 }}
                      />
                    )}
                  </Stack>
                </Box>
              );
            })
          )}
        </Box>
      </Box>

      {/* COLUMN 2: Entity List for selected module */}
      <Box
        sx={{
          width: 260, flexShrink: 0,
          borderRight: `1px solid ${theme.palette.divider}`,
          bgcolor: alpha(theme.palette.background.default, 0.2),
          display: 'flex', flexDirection: 'column', overflowY: 'auto',
        }}
      >
        <Box sx={{ p: 2, pb: 1, borderBottom: `1px solid ${theme.palette.divider}` }}>
          <Typography variant="overline" fontWeight={800} fontSize="0.6rem" color="text.secondary">
            ENTITIES
          </Typography>
          <Tooltip
            title="An entity is a entity representing a core object in your app (like User, Token, or Order). Each entity becomes a database table or smart contract struct."
            placement="right"
            arrow
          >
            <InfoOutlinedIcon sx={{ fontSize: 12, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
          </Tooltip>
          {selectedModuleId && blueprintModules.find((m) => m.id === selectedModuleId) && (
            <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mt: 0.25 }}>
              {blueprintModules.find((m) => m.id === selectedModuleId)?.label}
            </Typography>
          )}
        </Box>

        <Box sx={{ flex: 1, py: 1 }}>
          {currentModuleEntities.length === 0 && selectedModuleId ? (
            <Box sx={{ p: 2 }}>
              <Typography variant="caption" color="text.disabled">
                No entities yet. Add one below.
              </Typography>
            </Box>
          ) : (
            currentModuleEntities.map((ent) => {
              const isActive = selectedEntityId === ent.id;
              return (
                <Box
                  key={ent.id}
                  onClick={() => setSelectedEntityId(ent.id)}
                  sx={{
                    mx: 1, mb: 0.5, px: 1.5, py: 1, borderRadius: 2, cursor: 'pointer',
                    bgcolor: isActive ? alpha(theme.palette.primary.main, 0.1) : 'transparent',
                    border: `1px solid ${isActive ? alpha(theme.palette.primary.main, 0.3) : 'transparent'}`,
                    '&:hover': { bgcolor: isActive ? alpha(theme.palette.primary.main, 0.12) : alpha(theme.palette.action.hover, 0.4) },
                  }}
                >
                  <Stack direction="row" alignItems="center" justifyContent="space-between">
                    <Stack direction="row" alignItems="center" spacing={1}>
                      <StorageIcon sx={{ fontSize: 14, color: isActive ? 'primary.main' : 'text.secondary' }} />
                      <Box>
                        <Typography
                          variant="body2"
                          fontWeight={700}
                          color={isActive ? 'primary.main' : 'text.primary'}
                        >
                          {ent.name}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          {ent.fields.length} fields
                          {ent.isCore && (
                            <Chip
                              label="Core"
                              size="small"
                              sx={{ ml: 0.5, height: 14, fontSize: '0.55rem', fontWeight: 700, bgcolor: alpha(theme.palette.success.main, 0.12), color: 'success.main' }}
                            />
                          )}
                        </Typography>
                      </Box>
                    </Stack>
                    {!ent.isCore && (
                      <IconButton
                        size="small"
                        color="error"
                        onClick={(ev) => { ev.stopPropagation(); deleteEntity(selectedModuleId!, ent.id); }}
                        sx={{ opacity: 0, '.MuiBox-root:hover > * > &': { opacity: 1 } }}
                      >
                        <DeleteOutlineIcon sx={{ fontSize: 14 }} />
                      </IconButton>
                    )}
                  </Stack>
                </Box>
              );
            })
          )}
        </Box>

        {selectedModuleId && (
          <Box sx={{ p: 1.5, borderTop: `1px solid ${theme.palette.divider}` }}>
            <Button
              startIcon={<AddIcon />}
              variant="outlined"
              size="small"
              fullWidth
              onClick={() => setAddEntityOpen(true)}
              sx={{ borderRadius: 2, mb: 0.75, fontWeight: 700 }}
            >
              Add More Entities
            </Button>
            <Button
              startIcon={<AddIcon />}
              size="small"
              fullWidth
              onClick={addBlankEntity}
              sx={{ borderRadius: 2, color: 'text.secondary' }}
            >
              Add Custom Entity
            </Button>
          </Box>
        )}
      </Box>

      {/* COLUMN 3: Field Editor */}
      {selectedEntity ? (
        <Box sx={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          {/* Entity Header */}
          <Box
            sx={{
              px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}`,
              bgcolor: alpha(theme.palette.background.paper, 0.6), flexShrink: 0,
            }}
          >
            <Stack direction="row" alignItems="center" spacing={2}>
              <TextField
                variant="standard"
                value={selectedEntity.name}
                onChange={(e) => updateEntity(selectedEntityModuleId!, selectedEntity.id, { name: e.target.value })}
                InputProps={{
                  disableUnderline: true,
                  style: { fontSize: '1.3rem', fontWeight: 800 },
                }}
              />
              <Chip
                label={selectedEntity.isCore ? 'Core Entity' : 'Custom Entity'}
                size="small"
                color={selectedEntity.isCore ? 'primary' : 'default'}
                variant="outlined"
              />
            </Stack>
            <TextField
              variant="standard"
              placeholder="Add a description..."
              value={selectedEntity.description || ''}
              onChange={(e) => updateEntity(selectedEntityModuleId!, selectedEntity.id, { description: e.target.value })}
              InputProps={{ disableUnderline: true, style: { fontSize: '0.82rem' } }}
              fullWidth
              sx={{ mt: 0.5 }}
            />
          </Box>

          {/* Fields Table */}
          <Box sx={{ flex: 1, overflow: 'auto', p: 2.5 }}>
            <Stack direction="row" alignItems="center" justifyContent="space-between" mb={1.5}>
              <Typography variant="subtitle2" fontWeight={800}>
                Fields
                <Tooltip title="A field is a single piece of data on this entity (like a name, email address, or token balance). Each field has a type, storage location, and constraints." arrow>
                  <InfoOutlinedIcon sx={{ fontSize: 13, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                </Tooltip>
              </Typography>
              <Button startIcon={<AddIcon />} size="small" onClick={addField} sx={{ borderRadius: 2 }}>
                Add Field
              </Button>
            </Stack>
            <TableContainer component={Paper} variant="outlined" sx={{ borderRadius: 2 }}>
              <Table size="small">
                <TableHead sx={{ bgcolor: alpha(theme.palette.action.hover, 0.05) }}>
                  <TableRow>
                    <TableCell width="22%">
                      Field Name
                    </TableCell>
                    <TableCell width="16%">
                      Type
                      <Tooltip title="The data type: UUID (unique ID), String (text), Int (number), Address (crypto wallet), Uint256 (large number for token amounts), DateTime, Boolean (true/false), JSON (structured data)." arrow>
                        <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                      </Tooltip>
                    </TableCell>
                    <TableCell width="16%">
                      Storage
                      <Tooltip title="Database: stored off-chain in your app's database (free, fast). On-Chain: stored on the blockchain (costs gas, immutable, auditable). IPFS: stored on a decentralized file system (for files and metadata)." arrow>
                        <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                      </Tooltip>
                    </TableCell>
                    <TableCell width="28%">
                      Constraints
                      <Tooltip title="Req = Required (can't be empty). Unq = Unique (no two records can have the same value). Priv = Private (data is encrypted at rest)." arrow>
                        <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                      </Tooltip>
                    </TableCell>
                    <TableCell width="13%">Default</TableCell>
                    <TableCell width="5%"></TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {selectedEntity.fields.map((f) => (
                    <TableRow key={f.id} hover>
                      <TableCell>
                        <TextField
                          size="small" fullWidth value={f.name} variant="standard"
                          onChange={(e) => updateField(f.id, { name: e.target.value })}
                          InputProps={{
                            disableUnderline: true,
                            startAdornment: f.name === 'id'
                              ? <KeyIcon sx={{ fontSize: 14, color: 'warning.main', mr: 0.5 }} />
                              : null,
                            style: { fontWeight: 600 },
                          }}
                        />
                      </TableCell>
                      <TableCell>
                        <Select
                          size="small" fullWidth value={f.type} variant="standard" disableUnderline
                          onChange={(e) => updateField(f.id, { type: e.target.value as DataType })}
                          MenuProps={OPAQUE_MENU_PROPS as any}
                        >
                          <MenuItem value="uuid">UUID</MenuItem>
                          <MenuItem value="string">String</MenuItem>
                          <MenuItem value="text">Text</MenuItem>
                          <MenuItem value="int">Integer</MenuItem>
                          <MenuItem value="float">Float</MenuItem>
                          <MenuItem value="boolean">Boolean</MenuItem>
                          <MenuItem value="datetime">DateTime</MenuItem>
                          <MenuItem value="json">JSON</MenuItem>
                          <Divider />
                          <MenuItem value="address">Address</MenuItem>
                          <MenuItem value="uint256">Uint256</MenuItem>
                          <MenuItem value="bytes32">Bytes32</MenuItem>
                          <MenuItem value="ipfs-hash">IPFS Hash</MenuItem>
                        </Select>
                      </TableCell>
                      <TableCell>
                        <Select
                          size="small" fullWidth value={f.storage} variant="standard" disableUnderline
                          onChange={(e) => updateField(f.id, { storage: e.target.value as StorageStrategy })}
                          MenuProps={OPAQUE_MENU_PROPS as any}
                          sx={{ color: f.storage === 'on-chain' ? 'warning.main' : f.storage === 'ipfs' ? 'info.main' : 'text.primary' }}
                        >
                          <MenuItem value="database">Database</MenuItem>
                          <MenuItem value="on-chain">On-Chain</MenuItem>
                          <MenuItem value="ipfs">IPFS</MenuItem>
                        </Select>
                      </TableCell>
                      <TableCell>
                        <Stack direction="row" spacing={0.5}>
                          <Chip
                            label="Req" size="small" clickable
                            onClick={() => updateField(f.id, { required: !f.required })}
                            color={f.required ? "primary" : "default"}
                            variant={f.required ? "filled" : "outlined"}
                          />
                          <Chip
                            label="Unq" size="small" clickable
                            onClick={() => updateField(f.id, { unique: !f.unique })}
                            color={f.unique ? "secondary" : "default"}
                            variant={f.unique ? "filled" : "outlined"}
                          />
                          <Chip
                            label="Priv" size="small" clickable
                            onClick={() => updateField(f.id, { encrypted: !f.encrypted })}
                            color={f.encrypted ? "success" : "default"}
                            variant={f.encrypted ? "filled" : "outlined"}
                          />
                        </Stack>
                      </TableCell>
                      <TableCell>
                        <TextField
                          size="small" fullWidth placeholder="-"
                          value={f.defaultValue || ''}
                          onChange={(e) => updateField(f.id, { defaultValue: e.target.value })}
                        />
                      </TableCell>
                      <TableCell>
                        <IconButton size="small" color="error" onClick={() => deleteField(f.id)}>
                          <DeleteOutlineIcon sx={{ fontSize: 14 }} />
                        </IconButton>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </Box>
        </Box>
      ) : (
        <Box sx={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 1, opacity: 0.5 }}>
          <StorageIcon sx={{ fontSize: 40 }} />
          <Typography variant="body2">Select an entity to edit its fields</Typography>
        </Box>
      )}
    </Box>
  );

  /* ---------------------------------------------------------------- */
  /* Render: GOVERNANCE                                               */
  /* ---------------------------------------------------------------- */
  const renderGovernance = () => (
    <Box sx={{ p: 4, height: '100%', overflowY: 'auto', display: 'flex', justifyContent: 'center' }}>
      <Box sx={{ width: '100%', maxWidth: 1000 }}>
        <Typography variant="h5" fontWeight={800}>Access Control Rules</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3, mt: 0.5 }}>
          Define who can perform each action on your data. "Public" means anyone. "Owner" means only the record's creator. "Admin" means only privileged users.
        </Typography>

        {blueprintModules.length === 0 ? (
          <Box sx={{ textAlign: 'center', py: 10, opacity: 0.5 }}>
            <SecurityIcon sx={{ fontSize: 40, mb: 1 }} />
            <Typography>No entities yet. Add modules in the Blueprint Builder first.</Typography>
          </Box>
        ) : (
          blueprintModules.map((mod) => {
            const modEnts = moduleEntities[mod.id] || [];
            if (!modEnts.length) return null;
            return (
              <Accordion
                key={mod.id}
                defaultExpanded
                variant="outlined"
                sx={{ mb: 1.5, borderRadius: '12px !important', overflow: 'hidden', '&:before': { display: 'none' } }}
              >
                <AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ bgcolor: alpha(theme.palette.primary.main, 0.04) }}>
                  <Stack direction="row" alignItems="center" spacing={1.5}>
                    <HexagonOutlinedIcon sx={{ fontSize: 16, color: 'primary.main' }} />
                    <Typography variant="subtitle2" fontWeight={800}>{mod.label}</Typography>
                    {mod.category && (
                      <Chip label={mod.category} size="small" variant="outlined" sx={{ fontSize: '0.6rem', height: 16 }} />
                    )}
                    <Typography variant="caption" color="text.secondary">{modEnts.length} {modEnts.length === 1 ? 'entity' : 'entities'}</Typography>
                  </Stack>
                </AccordionSummary>
                <AccordionDetails sx={{ p: 0 }}>
                  <TableContainer>
                    <Table>
                      <TableHead sx={{ bgcolor: alpha(theme.palette.action.hover, 0.05) }}>
                        <TableRow>
                          <TableCell width="25%">ENTITY</TableCell>
                          <TableCell width="20%">CREATE</TableCell>
                          <TableCell width="20%">READ</TableCell>
                          <TableCell width="20%">UPDATE</TableCell>
                          <TableCell width="15%">DELETE</TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {modEnts.map((ent) => (
                          <TableRow key={ent.id}>
                            <TableCell sx={{ fontWeight: 700 }}>{ent.name}</TableCell>
                            {['Create', 'Read', 'Update', 'Delete'].map((action, i) => (
                              <TableCell key={action}>
                                <Select size="small" fullWidth defaultValue={i === 1 ? 'public' : 'owner'} MenuProps={OPAQUE_MENU_PROPS as any} sx={{ borderRadius: 2 }}>
                                  <MenuItem value="public">Public</MenuItem>
                                  <MenuItem value="auth">Auth User</MenuItem>
                                  <MenuItem value="owner">Owner</MenuItem>
                                  <MenuItem value="admin">Admin</MenuItem>
                                </Select>
                              </TableCell>
                            ))}
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </TableContainer>
                </AccordionDetails>
              </Accordion>
            );
          })
        )}
      </Box>
    </Box>
  );

  /* ---------------------------------------------------------------- */
  /* Render: BEHAVIOR / LOGIC                                        */
  /* ---------------------------------------------------------------- */

  // Pre-seeded trigger templates per module type
  const TRIGGER_SEEDS: Record<string, Array<{ event: string; action: string; description: string; color: string }>> = {
    'user-auth': [
      { event: 'User.Created', action: 'Send Welcome Email', description: 'Fire when a new user account is created.', color: '#3b82f6' },
      { event: 'User.LoginFailed (3x)', action: 'Lock Account + Alert', description: 'Prevent brute-force by locking after 3 failed attempts.', color: '#ef4444' },
    ],
    'token-erc20': [
      { event: 'Transfer.Completed', action: 'Update Balance Cache', description: 'Keep off-chain balance cache in sync after every transfer.', color: '#8b5cf6' },
      { event: 'Token.Minted', action: 'Emit Notification', description: 'Notify the recipient wallet when tokens are minted to them.', color: '#10b981' },
    ],
    'governance': [
      { event: 'Proposal.Created', action: 'Notify Voters', description: 'Alert all eligible voters when a new proposal is submitted.', color: '#f59e0b' },
      { event: 'Vote.Deadline.Reached', action: 'Finalize Proposal', description: 'Auto-execute the winning outcome when voting ends.', color: '#06b6d4' },
    ],
    '_default': [
      { event: 'Record.Created', action: 'Index for Search', description: 'Update the search index when a new record is added.', color: '#3b82f6' },
      { event: 'Record.Updated', action: 'Write Audit Log', description: 'Track all changes in the audit log for compliance.', color: '#10b981' },
    ],
  };

  const [triggerModeMap, setTriggerModeMap] = useState<Record<string, 'cards' | 'visual' | 'code'>>({});
  const getTriggerMode = (modId: string) => triggerModeMap[modId] || 'cards';
  const setTriggerMode = (modId: string, mode: 'cards' | 'visual' | 'code') =>
    setTriggerModeMap((prev) => ({ ...prev, [modId]: mode }));

  const renderBehavior = () => (
    <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', overflowY: 'auto' }}>
      {/* Header */}
      <Box sx={{
        px: 4, py: 2.5, borderBottom: `1px solid ${theme.palette.divider}`,
        bgcolor: 'background.paper', flexShrink: 0,
      }}>
        <Stack direction="row" alignItems="flex-start" justifyContent="space-between">
          <Box>
            <Stack direction="row" alignItems="center" spacing={1} mb={0.5}>
              <BoltIcon sx={{ color: 'warning.main', fontSize: 20 }} />
              <Typography variant="h6" fontWeight={800}>Logic &amp; Triggers</Typography>
            </Stack>
            <Typography variant="body2" color="text.secondary">
              Define what happens automatically when events occur in your app. Each trigger connects an event (e.g., "User created") to one or more actions (e.g., "Send email"). Pre-filled triggers are recommended best-practices for your modules.
            </Typography>
          </Box>
          <Tooltip
            title="Triggers fire automatically when entity events occur. Example: when a Payment is Created, run SendEmail and MintNFT. No backend code needed."
            placement="left" arrow
            componentsProps={{ tooltip: { sx: { bgcolor: 'background.paper', color: 'text.primary', border: '1px solid', borderColor: 'divider', boxShadow: '0 8px 32px rgba(0,0,0,0.25)', borderRadius: 2, p: 1.5, maxWidth: 300, fontSize: '0.8rem' } } }}
          >
            <Chip
              label="How triggers work"
              size="small" variant="outlined"
              icon={<AutoFixHighIcon sx={{ fontSize: '0.9rem !important' }} />}
              sx={{ fontWeight: 600, fontSize: '0.72rem', cursor: 'help', flexShrink: 0, mt: 0.5 }}
            />
          </Tooltip>
        </Stack>
      </Box>

      {/* Per-module trigger cards */}
      <Box sx={{ flex: 1, p: 3, overflowY: 'auto' }}>
        {blueprintModules.length === 0 ? (
          <Box sx={{ textAlign: 'center', py: 12, opacity: 0.5 }}>
            <BoltIcon sx={{ fontSize: 48, mb: 1 }} />
            <Typography>No modules yet. Add modules in the Blueprint Builder first.</Typography>
          </Box>
        ) : (
          <Stack spacing={2.5}>
            {blueprintModules.map((mod) => {
              const mode = getTriggerMode(mod.id);
              const seeds = TRIGGER_SEEDS[mod.id] || TRIGGER_SEEDS['_default'];
              return (
                <Paper
                  key={mod.id}
                  variant="outlined"
                  sx={{ borderRadius: 3, overflow: 'hidden' }}
                >
                  {/* Module header */}
                  <Box sx={{
                    px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}`,
                    bgcolor: alpha(theme.palette.primary.main, 0.03),
                    display: 'flex', alignItems: 'center', justifyContent: 'space-between',
                  }}>
                    <Stack direction="row" alignItems="center" spacing={1.5}>
                      <HexagonOutlinedIcon sx={{ fontSize: 16, color: 'primary.main' }} />
                      <Typography variant="subtitle2" fontWeight={800}>{mod.label}</Typography>
                      {mod.category && (
                        <Chip label={mod.category} size="small" variant="outlined" sx={{ fontSize: '0.6rem', height: 16 }} />
                      )}
                    </Stack>
                    <Stack direction="row" spacing={0.75}>
                      {(['cards', 'visual', 'code'] as const).map((m) => (
                        <Paper
                          key={m}
                          variant="outlined"
                          onClick={() => setTriggerMode(mod.id, m)}
                          sx={{
                            px: 1.5, py: 0.5, borderRadius: 999, cursor: 'pointer',
                            fontWeight: 700, fontSize: '0.7rem',
                            bgcolor: mode === m ? 'primary.main' : 'background.paper',
                            color: mode === m ? 'white' : 'text.secondary',
                            borderColor: mode === m ? 'primary.main' : 'divider',
                            display: 'flex', alignItems: 'center', gap: 0.5,
                          }}
                          elevation={0}
                        >
                          {m === 'cards' && <BoltIcon sx={{ fontSize: 12 }} />}
                          {m === 'visual' && <AccountTreeIcon sx={{ fontSize: 12 }} />}
                          {m === 'code' && <CodeIcon sx={{ fontSize: 12 }} />}
                          {m === 'cards' ? 'Rules' : m === 'visual' ? 'Visual' : 'Script'}
                        </Paper>
                      ))}
                    </Stack>
                  </Box>

                  {/* Content area */}
                  <Box sx={{ p: mode === 'cards' ? 2.5 : 0 }}>
                    {mode === 'cards' && (
                      <Stack spacing={1.5}>
                        <Typography variant="caption" color="text.secondary" fontWeight={700} sx={{ letterSpacing: 0.5 }}>
                          RECOMMENDED TRIGGERS FOR {mod.label.toUpperCase()}
                        </Typography>
                        <Stack spacing={1}>
                          {seeds.map((t, i) => (
                            <Paper
                              key={i}
                              variant="outlined"
                              sx={{
                                p: 2, borderRadius: 2,
                                borderColor: alpha(t.color, 0.3),
                                bgcolor: alpha(t.color, 0.03),
                              }}
                            >
                              <Stack direction="row" alignItems="flex-start" spacing={2}>
                                {/* Event */}
                                <Box sx={{ flex: 1 }}>
                                  <Typography variant="caption" fontWeight={800} color="text.secondary" sx={{ letterSpacing: 0.5 }}>WHEN</Typography>
                                  <Paper
                                    variant="outlined"
                                    sx={{
                                      mt: 0.5, px: 1.5, py: 0.75, borderRadius: 1.5,
                                      display: 'inline-flex', alignItems: 'center', gap: 0.75,
                                      bgcolor: alpha(t.color, 0.08), borderColor: alpha(t.color, 0.4),
                                    }}
                                    elevation={0}
                                  >
                                    <BoltIcon sx={{ fontSize: 13, color: t.color }} />
                                    <Typography variant="body2" fontWeight={700} sx={{ color: t.color, fontFamily: 'monospace', fontSize: '0.78rem' }}>
                                      {t.event}
                                    </Typography>
                                  </Paper>
                                </Box>

                                {/* Arrow */}
                                <Box sx={{ mt: 2.5, color: 'text.disabled', fontWeight: 900, fontSize: '1.2rem', flexShrink: 0 }}>→</Box>

                                {/* Action */}
                                <Box sx={{ flex: 1 }}>
                                  <Typography variant="caption" fontWeight={800} color="text.secondary" sx={{ letterSpacing: 0.5 }}>THEN</Typography>
                                  <Paper
                                    variant="outlined"
                                    sx={{
                                      mt: 0.5, px: 1.5, py: 0.75, borderRadius: 1.5,
                                      display: 'inline-flex', alignItems: 'center', gap: 0.75,
                                      bgcolor: alpha(theme.palette.success.main, 0.08), borderColor: alpha(theme.palette.success.main, 0.3),
                                    }}
                                    elevation={0}
                                  >
                                    <AutoFixHighIcon sx={{ fontSize: 13, color: 'success.main' }} />
                                    <Typography variant="body2" fontWeight={700} sx={{ color: 'success.main', fontFamily: 'monospace', fontSize: '0.78rem' }}>
                                      {t.action}
                                    </Typography>
                                  </Paper>
                                </Box>

                                {/* Description */}
                                <Box sx={{ flex: 1.5 }}>
                                  <Typography variant="caption" fontWeight={800} color="text.secondary" sx={{ letterSpacing: 0.5 }}>WHY</Typography>
                                  <Typography variant="body2" color="text.secondary" sx={{ mt: 0.5, lineHeight: 1.4 }}>{t.description}</Typography>
                                </Box>
                              </Stack>
                            </Paper>
                          ))}
                        </Stack>
                        <Button
                          startIcon={<AddIcon />}
                          variant="outlined"
                          size="small"
                          sx={{ borderRadius: 2, alignSelf: 'flex-start', mt: 0.5, fontWeight: 700 }}
                        >
                          Add Custom Trigger
                        </Button>
                      </Stack>
                    )}
                    {mode === 'visual' && (
                      <Box sx={{ height: 380 }}>
                        <LogicCanvas />
                      </Box>
                    )}
                    {mode === 'code' && (
                      <Box sx={{ height: 380, bgcolor: theme.palette.mode === 'dark' ? '#0d0d0f' : '#1e1e1e' }}>
                        <CustomScriptPanel projectId="" />
                      </Box>
                    )}
                  </Box>
                </Paper>
              );
            })}
          </Stack>
        )}
      </Box>
    </Box>
  );

  /* ---------------------------------------------------------------- */
  /* Render: EXPOSURE                                                 */
  /* ---------------------------------------------------------------- */
  const renderExposure = () => (
    <Box sx={{ p: 4, height: '100%', overflowY: 'auto', display: 'flex', justifyContent: 'center' }}>
      <Box sx={{ width: '100%', maxWidth: 1000 }}>
        <Typography variant="h5" fontWeight={800}>API &amp; Visibility</Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3, mt: 0.5 }}>
          On-chain means data lives on the blockchain (auditable, immutable, costs gas). API Public means Cerulea generates REST/GraphQL endpoints for this entity. Encryption adds at-rest encryption for sensitive fields.
        </Typography>

        {blueprintModules.length === 0 ? (
          <Box sx={{ textAlign: 'center', py: 10, opacity: 0.5 }}>
            <PublicIcon sx={{ fontSize: 40, mb: 1 }} />
            <Typography>No entities yet. Add modules in the Blueprint Builder first.</Typography>
          </Box>
        ) : (
          blueprintModules.map((mod) => {
            const modEnts = moduleEntities[mod.id] || [];
            if (!modEnts.length) return null;
            return (
              <Accordion
                key={mod.id}
                defaultExpanded
                variant="outlined"
                sx={{ mb: 1.5, borderRadius: '12px !important', overflow: 'hidden', '&:before': { display: 'none' } }}
              >
                <AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ bgcolor: alpha(theme.palette.primary.main, 0.04) }}>
                  <Stack direction="row" alignItems="center" spacing={1.5}>
                    <HexagonOutlinedIcon sx={{ fontSize: 16, color: 'primary.main' }} />
                    <Typography variant="subtitle2" fontWeight={800}>{mod.label}</Typography>
                    {mod.category && (
                      <Chip label={mod.category} size="small" variant="outlined" sx={{ fontSize: '0.6rem', height: 16 }} />
                    )}
                    <Typography variant="caption" color="text.secondary">{modEnts.length} {modEnts.length === 1 ? 'entity' : 'entities'}</Typography>
                  </Stack>
                </AccordionSummary>
                <AccordionDetails sx={{ p: 0 }}>
                  <TableContainer>
                    <Table>
                      <TableHead sx={{ bgcolor: alpha(theme.palette.action.hover, 0.05) }}>
                        <TableRow>
                          <TableCell width="30%">ENTITY</TableCell>
                          <TableCell width="20%" align="center">
                            ON-CHAIN
                            <Tooltip title="Data stored on the blockchain: immutable, auditable, and visible to all validators. Incurs gas costs on write." arrow>
                              <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                            </Tooltip>
                          </TableCell>
                          <TableCell width="20%" align="center">
                            API PUBLIC
                            <Tooltip title="Cerulea auto-generates REST and GraphQL endpoints for this entity. Turn off to keep the entity internal-only." arrow>
                              <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                            </Tooltip>
                          </TableCell>
                          <TableCell width="30%" align="center">
                            ENCRYPTION
                            <Tooltip title="At-rest encryption: sensitive field values are encrypted in the database. Transparent to your app, adds a layer of security." arrow>
                              <InfoOutlinedIcon sx={{ fontSize: 11, color: 'text.secondary', ml: 0.5, cursor: 'help', verticalAlign: 'middle' }} />
                            </Tooltip>
                          </TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {modEnts.map((ent) => (
                          <TableRow key={ent.id}>
                            <TableCell sx={{ fontWeight: 700 }}>{ent.name}</TableCell>
                            <TableCell align="center"><Switch size="small" /></TableCell>
                            <TableCell align="center"><Switch size="small" defaultChecked /></TableCell>
                            <TableCell align="center"><Chip label="At Rest" size="small" variant="outlined" /></TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </TableContainer>
                </AccordionDetails>
              </Accordion>
            );
          })
        )}
      </Box>
    </Box>
  );

  /* ---------------------------------------------------------------- */
  /* Main render                                                      */
  /* ---------------------------------------------------------------- */
  return (
    <Box sx={{ width: '100%', position: 'fixed', inset: 0, top: 64, bgcolor: 'background.default', display: 'flex', flexDirection: 'column' }}>

      {/* Background dot grid */}
      <Box sx={{
        position: 'absolute', inset: 0, opacity: 0.3, zIndex: -1,
        backgroundImage: theme.palette.mode === 'light' ? 'radial-gradient(#ccc 1px, transparent 1px)' : 'radial-gradient(#333 1px, transparent 1px)',
        backgroundSize: '24px 24px',
      }} />

      {/* Step indicator */}
      <Box sx={{ position: 'absolute', top: 24, left: '50%', transform: 'translateX(-50%)', zIndex: 10, pointerEvents: 'none' }}>
        <StepPill>
          <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1, lineHeight: 1 }}>STEP 3 OF 6</Typography>
          <Divider orientation="vertical" flexItem sx={{ height: 14, my: 'auto', opacity: 0.5 }} />
          <Typography variant="subtitle2" fontWeight={700}>Data &amp; Logic</Typography>
        </StepPill>
      </Box>

      {/* Layout: sidebar + workspace */}
      <Box sx={{ flex: 1, display: 'flex', overflow: 'hidden' }}>

        {/* LEFT NAVIGATION SIDEBAR */}
        <PhaseSidebar>
          <Box sx={{ px: 2.5, pb: 2 }}>
            <Typography variant="overline" fontWeight={800} color="text.disabled" fontSize="0.6rem">STEP 3 OF 6</Typography>
            <Typography variant="subtitle1" fontWeight={800}>Data &amp; Logic</Typography>
            <Typography variant="caption" color="text.secondary">Define what your app stores and how it behaves.</Typography>
          </Box>
          <Stack spacing={0.5} sx={{ px: 1.5 }}>
            <PhaseItem active={phase === 'data'} onClick={() => setPhase('data')}>
              <StorageIcon fontSize="small" />
              <Box>
                <Typography variant="subtitle2" fontWeight={700}>Entities</Typography>
                <Typography variant="caption" color="text.secondary" sx={{ display: 'block', lineHeight: 1.2 }}>
                  {allEntities.length} entities across {blueprintModules.length} modules
                </Typography>
              </Box>
            </PhaseItem>
            <PhaseItem active={phase === 'governance'} onClick={() => setPhase('governance')}>
              <SecurityIcon fontSize="small" />
              <Box>
                <Typography variant="subtitle2" fontWeight={700}>Access Rules</Typography>
                <Typography variant="caption" color="text.secondary" sx={{ display: 'block', lineHeight: 1.2 }}>Who can read / write data</Typography>
              </Box>
            </PhaseItem>
            <PhaseItem active={phase === 'behavior'} onClick={() => setPhase('behavior')}>
              <BoltIcon fontSize="small" />
              <Box>
                <Typography variant="subtitle2" fontWeight={700}>Logic &amp; Triggers</Typography>
                <Typography variant="caption" color="text.secondary" sx={{ display: 'block', lineHeight: 1.2 }}>Automate actions on events</Typography>
              </Box>
            </PhaseItem>
            <PhaseItem active={phase === 'exposure'} onClick={() => setPhase('exposure')}>
              <PublicIcon fontSize="small" />
              <Box>
                <Typography variant="subtitle2" fontWeight={700}>API &amp; Visibility</Typography>
                <Typography variant="caption" color="text.secondary" sx={{ display: 'block', lineHeight: 1.2 }}>On-chain vs database exposure</Typography>
              </Box>
            </PhaseItem>
          </Stack>
        </PhaseSidebar>

        {/* MAIN WORKSPACE */}
        <Workspace>
          <Fade in={phase === 'data'} mountOnEnter unmountOnExit><Box height="100%">{renderDataLayer()}</Box></Fade>
          <Fade in={phase === 'governance'} mountOnEnter unmountOnExit><Box height="100%">{renderGovernance()}</Box></Fade>
          <Fade in={phase === 'behavior'} mountOnEnter unmountOnExit><Box height="100%">{renderBehavior()}</Box></Fade>
          <Fade in={phase === 'exposure'} mountOnEnter unmountOnExit><Box height="100%">{renderExposure()}</Box></Fade>
        </Workspace>
      </Box>

      {/* FLOATING DOCK */}
      <Box sx={{ position: 'absolute', bottom: 32, left: '50%', transform: 'translateX(-50%)', zIndex: 100 }}>
        <FloatingIsland elevation={6}>
          <Tooltip title="Back">
            <IconButton onClick={goPrev || (() => router.back())} size="small" sx={{ border: '1px solid', borderColor: 'divider' }}>
              <ArrowBackIcon />
            </IconButton>
          </Tooltip>
          <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />
          <Tooltip title="Help & Guide">
            <IconButton size="small" color="primary" onClick={() => setIsHelpOpen(true)}>
              <QuestionMarkIcon fontSize="small" />
            </IconButton>
          </Tooltip>
          <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />
          <Button variant="contained" onClick={handleSave} endIcon={<ArrowForwardIcon />} sx={{ borderRadius: 100, px: 3, fontWeight: 700 }}>
            Save &amp; Next
          </Button>
        </FloatingIsland>
      </Box>

      {/* ADD MORE ENTITIES DIALOG */}
      <Dialog
        open={addEntityOpen}
        onClose={() => { setAddEntityOpen(false); setEntitySearch(''); }}
        maxWidth="md"
        fullWidth
        PaperProps={{ sx: { borderRadius: 3, height: '75vh' } }}
      >
        <DialogTitle sx={{ pb: 1, display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <Box>
            <Typography variant="h6" fontWeight={800}>Add More Entities</Typography>
            <Typography variant="caption" color="text.secondary">
              Browse entities from all modules. Selected entities will be added to{' '}
              <strong>{blueprintModules.find((m) => m.id === selectedModuleId)?.label || 'the current module'}</strong>.
            </Typography>
          </Box>
          <IconButton onClick={() => { setAddEntityOpen(false); setEntitySearch(''); }}>
            <CloseIcon />
          </IconButton>
        </DialogTitle>
        <DialogContent dividers sx={{ display: 'flex', flexDirection: 'column', gap: 2, p: 2 }}>
          <TextField
            placeholder="Search entities by name or module..."
            size="small"
            value={entitySearch}
            onChange={(e) => setEntitySearch(e.target.value)}
            InputProps={{
              startAdornment: <InputAdornment position="start"><SearchIcon sx={{ fontSize: 16 }} /></InputAdornment>,
            }}
            autoFocus
          />
          <Box sx={{ flex: 1, overflowY: 'auto' }}>
            {Object.keys(catalogGroups).length === 0 ? (
              <Box sx={{ textAlign: 'center', py: 4 }}>
                <Typography color="text.secondary">
                  {entitySearch ? 'No matching entities found.' : 'All available entities are already added.'}
                </Typography>
              </Box>
            ) : (
              Object.entries(catalogGroups).map(([groupLabel, entries]) => (
                <Accordion key={groupLabel} defaultExpanded={Object.keys(catalogGroups).length <= 3} disableGutters elevation={0} sx={{ border: `1px solid ${theme.palette.divider}`, borderRadius: '8px !important', mb: 1, '&:before': { display: 'none' } }}>
                  <AccordionSummary expandIcon={<ExpandMoreIcon />} sx={{ minHeight: 44, py: 0 }}>
                    <Typography variant="subtitle2" fontWeight={700}>{groupLabel}</Typography>
                    <Chip label={entries.length} size="small" sx={{ ml: 1, height: 18, fontSize: '0.65rem' }} />
                  </AccordionSummary>
                  <AccordionDetails sx={{ pt: 0, pb: 1 }}>
                    <Stack direction="row" flexWrap="wrap" gap={1}>
                      {entries.map((e) => (
                        <Chip
                          key={`${e.moduleId}-${e.entity.name}`}
                          label={e.entity.name}
                          onClick={() => addEntityFromCatalog(e.entity)}
                          variant="outlined"
                          clickable
                          icon={<AddIcon sx={{ fontSize: '0.9rem !important' }} />}
                          sx={{ fontWeight: 600 }}
                        />
                      ))}
                    </Stack>
                  </AccordionDetails>
                </Accordion>
              ))
            )}
          </Box>
        </DialogContent>
        <DialogActions sx={{ px: 3, pb: 2 }}>
          <Button onClick={() => { setAddEntityOpen(false); setEntitySearch(''); }} sx={{ borderRadius: 999 }}>
            Close
          </Button>
        </DialogActions>
      </Dialog>

      {/* HELP DIALOG */}
      <Dialog open={isHelpOpen} onClose={() => setIsHelpOpen(false)} maxWidth="sm" fullWidth PaperProps={{ sx: { borderRadius: 4 } }}>
        <DialogTitle sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', p: 3 }}>
          <Typography variant="h6" fontWeight={800}>
            {phase === 'data' && 'Entities Guide'}
            {phase === 'governance' && 'Access Control Guide'}
            {phase === 'behavior' && 'Logic & Triggers Guide'}
            {phase === 'exposure' && 'API & Visibility Guide'}
          </Typography>
          <IconButton onClick={() => setIsHelpOpen(false)}><CloseIcon /></IconButton>
        </DialogTitle>
        <DialogContent dividers sx={{ p: 4 }}>
          <Typography variant="body1" color="text.secondary" paragraph>
            {phase === 'data' && "Entities are your app's core objects. Each module from your Blueprint has 5+ pre-filled entities with all recommended fields. Click any entity to see and edit its fields. Use 'Add More Entities' to browse the full catalog."}
            {phase === 'governance' && "Set the rules for who can interact with your data. Access Control Rules (ACR) determine who can Create, Read, Update, or Delete records for each entity."}
            {phase === 'behavior' && "Design the logic of your application. Triggers allow you to automate workflows, like sending an email when a user signs up or minting an NFT when a payment is received."}
            {phase === 'exposure' && "Control the interface of your application. Toggle which APIs are generated (GraphQL/REST) and ensure sensitive data is encrypted."}
          </Typography>
          <Box bgcolor={alpha(theme.palette.info.main, 0.1)} p={2} borderRadius={2}>
            <Typography variant="subtitle2" fontWeight={700} gutterBottom>Pro Tip</Typography>
            <Typography variant="body2" fontSize={13}>
              {phase === 'data' && "Core entities (green badge) come from your module presets and are pre-configured. Custom entities (no badge) are entities you created manually."}
              {phase === 'governance' && "Start with 'Owner Only' for critical data to prevent unauthorized access."}
              {phase === 'behavior' && "Visual flows are great for high-level logic. Switch to 'Script' for complex calculations."}
              {phase === 'exposure' && "Always encrypt PII (Personally Identifiable Information) before storing it."}
            </Typography>
          </Box>
        </DialogContent>
      </Dialog>
    </Box>
  );
}
