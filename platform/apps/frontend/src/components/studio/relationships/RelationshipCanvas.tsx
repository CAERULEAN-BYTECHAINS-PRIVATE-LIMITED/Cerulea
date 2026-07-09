'use client';

import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  Box,
  Divider,
  Drawer,
  IconButton,
  Stack,
  Typography,
  Button,
  Tooltip,
  TextField,
  MenuItem,
} from '@mui/material';
import CloseIcon from '@mui/icons-material/Close';
import {
  Background,
  Controls,
  MiniMap,
  ReactFlow,
  addEdge,
  applyEdgeChanges,
  applyNodeChanges,
  Connection,
  Edge,
  Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

type Track = 'dapp' | 'blockchain';

type Entity = { id: string; name: string; fields: any[] };

type Relationship = {
  id: string;
  from: string;
  to: string;
  type: 'relatedTo' | 'oneToMany' | 'manyToMany' | 'oneToOne';
};

type Props = {
  track: Track;
  entities: Entity[];
  relationships: Relationship[];
  onChange: (next: { entities?: Entity[]; relationships?: Relationship[] }) => void;
};

function uid() {
  return typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : Math.random().toString(36).slice(2);
}

export default function RelationshipCanvas({ entities, relationships, onChange }: Props) {
  const wrapperRef = useRef<HTMLDivElement | null>(null);

  // Nodes are entities
  const nodes: Node[] = useMemo(() => {
    // stable layout if you don’t store positions yet
    return entities.map((e, idx) => ({
      id: e.id,
      position: { x: 120 + (idx % 2) * 260, y: 80 + Math.floor(idx / 2) * 140 },
      data: {
        label: e.name,
        entityId: e.id,
      },
      style: {
        borderRadius: 16,
        border: '1px solid rgba(255,255,255,0.14)',
        background: 'rgba(15,18,28,0.72)',
        padding: 10,
        color: 'white',
        minWidth: 160,
      },
    }));
  }, [entities]);

  const edges: Edge[] = useMemo(() => {
    return relationships.map((r) => ({
      id: r.id,
      source: r.from,
      target: r.to,
      label: r.type,
      data: {
        relType: r.type,
        cardinalityFrom: r.type === 'oneToMany' ? '1' : r.type === 'manyToMany' ? '*' : '1',
        cardinalityTo: r.type === 'oneToMany' ? '*' : r.type === 'manyToMany' ? '*' : '1',
        cascade: 'none',
        required: false,
        unique: false,
        indexed: true,
        onDelete: 'restrict',
        onUpdate: 'restrict',
        description: '',
      },
      style: { strokeWidth: 2 },
    }));
  }, [relationships]);

  const [rfNodes, setRfNodes] = useState<Node[]>(nodes);
  const [rfEdges, setRfEdges] = useState<Edge[]>(edges);

  // Sync props -> local (guarded by JSON compare to avoid loops)
  useEffect(() => {
    const a = JSON.stringify(nodes.map((n) => ({ id: n.id, data: n.data })));
    const b = JSON.stringify(rfNodes.map((n) => ({ id: n.id, data: n.data })));
    if (a !== b) setRfNodes(nodes);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [nodes]);

  useEffect(() => {
    const a = JSON.stringify(edges.map((e) => ({ id: e.id, source: e.source, target: e.target, data: e.data, label: e.label })));
    const b = JSON.stringify(rfEdges.map((e) => ({ id: e.id, source: e.source, target: e.target, data: e.data, label: e.label })));
    if (a !== b) setRfEdges(edges);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [edges]);

  const [selectedNode, setSelectedNode] = useState<Node | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<Edge | null>(null);

  const onNodesChange = (changes: any) => setRfNodes((nds) => applyNodeChanges(changes, nds));
  const onEdgesChange = (changes: any) => setRfEdges((eds) => applyEdgeChanges(changes, eds));

  const onConnect = (connection: Connection) => {
    if (!connection.source || !connection.target) return;
    const id = uid();
    const rel: Relationship = {
      id,
      from: connection.source,
      to: connection.target,
      type: 'relatedTo',
    };
    onChange({ relationships: [...relationships, rel] });
  };

  // Persist relationship edits back to parent
  const saveEdgeToRelationships = (edgeId: string, patch: Partial<Relationship>, edgeDataPatch?: any) => {
    const next = relationships.map((r) =>
      r.id === edgeId ? { ...r, ...patch } : r
    );
    onChange({ relationships: next });

    // keep local edge data in sync too
    setRfEdges((eds) =>
      eds.map((e) =>
        e.id === edgeId
          ? {
              ...e,
              label: patch.type ?? e.label,
              data: { ...(e.data || {}), ...(edgeDataPatch || {}) },
            }
          : e
      )
    );
  };

  const deleteRelationship = (edgeId: string) => {
    onChange({ relationships: relationships.filter((r) => r.id !== edgeId) });
    setSelectedEdge(null);
  };

  return (
    <Box sx={{ display: 'flex', gap: 2, minHeight: 560 }}>
      {/* Entities list */}
      <Box
        sx={{
          width: 340,
          borderRadius: 6,
          border: '1px solid rgba(255,255,255,0.08)',
          overflow: 'hidden',
        }}
      >
        <Box sx={{ p: 2 }}>
          <Typography sx={{ fontWeight: 700 }}>Entities</Typography>
          <Typography sx={{ opacity: 0.7, fontSize: 12 }}>
            Drag connections on canvas to create relationships
          </Typography>
        </Box>

        <Divider sx={{ borderColor: 'rgba(255,255,255,0.08)' }} />

        <Box sx={{ px: 2, py: 2, maxHeight: 500, overflowY: 'auto' }}>
          {entities.map((e) => (
            <Box
              key={e.id}
              sx={{
                p: 2,
                mb: 1,
                borderRadius: 4,
                border: '1px solid rgba(255,255,255,0.08)',
                background: 'rgba(255,255,255,0.03)',
              }}
            >
              <Typography sx={{ fontWeight: 700 }}>{e.name}</Typography>
              <Typography sx={{ opacity: 0.7, fontSize: 12 }}>
                {e.fields?.length ?? 0} fields
              </Typography>
            </Box>
          ))}
        </Box>
      </Box>

      {/* Canvas */}
      <Box
        ref={wrapperRef}
        sx={{
          flex: 1,
          borderRadius: 6,
          border: '1px solid rgba(255,255,255,0.08)',
          overflow: 'hidden', // fixes corner cutoff
          position: 'relative',
          minWidth: 0,
        }}
      >
        <ReactFlow
          nodes={rfNodes}
          edges={rfEdges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onConnect={onConnect}
          fitView
          fitViewOptions={{ padding: 0.2 }}
          onNodeClick={(_, n) => {
            setSelectedEdge(null);
            setSelectedNode(n);
          }}
          onEdgeClick={(_, e) => {
            setSelectedNode(null);
            setSelectedEdge(e);
          }}
          onPaneClick={() => {
            setSelectedNode(null);
            setSelectedEdge(null);
          }}
        >
          <Background />
          <Controls />
          <MiniMap />
        </ReactFlow>
      </Box>

      {/* Properties Drawer (opaque) */}
      <Drawer
        anchor="right"
        open={!!selectedNode || !!selectedEdge}
        onClose={() => {
          setSelectedNode(null);
          setSelectedEdge(null);
        }}
        PaperProps={{
          sx: {
            width: 380,
            background: 'rgba(15,18,28,0.92)',
            borderLeft: '1px solid rgba(255,255,255,0.08)',
            backdropFilter: 'blur(10px)',
          },
        }}
      >
        <Box sx={{ p: 2 }}>
          <Stack direction="row" alignItems="center" spacing={1}>
            <Typography sx={{ fontWeight: 800 }}>
              {selectedNode ? 'Entity' : 'Relationship'} Properties
            </Typography>
            <Box sx={{ flex: 1 }} />
            <IconButton
              onClick={() => {
                setSelectedNode(null);
                setSelectedEdge(null);
              }}
            >
              <CloseIcon />
            </IconButton>
          </Stack>

          <Divider sx={{ my: 2, borderColor: 'rgba(255,255,255,0.08)' }} />

          {selectedNode && (
            <>
              <Typography sx={{ opacity: 0.75, mb: 1 }}>
                Entity name is edited in Fields tab.
              </Typography>
              <TextField
                fullWidth
                label="Entity ID"
                value={String(selectedNode.id)}
                disabled
              />
            </>
          )}

          {selectedEdge && (
            <>
              <TextField
                fullWidth
                label="Relationship Type"
                select
                value={String((selectedEdge.data as any)?.relType || 'relatedTo')}
                onChange={(e) => {
                  const type = e.target.value as Relationship['type'];
                  saveEdgeToRelationships(
                    selectedEdge.id,
                    { type },
                    { relType: type }
                  );
                }}
                sx={{ mb: 2 }}
              >
                {['relatedTo', 'oneToOne', 'oneToMany', 'manyToMany'].map((t) => (
                  <MenuItem key={t} value={t}>
                    {t}
                  </MenuItem>
                ))}
              </TextField>

              <TextField
                fullWidth
                label="Label"
                value={String(selectedEdge.label || '')}
                onChange={(e) => {
                  const label = e.target.value;
                  setRfEdges((eds) =>
                    eds.map((ed) => (ed.id === selectedEdge.id ? { ...ed, label } : ed))
                  );
                }}
                sx={{ mb: 2 }}
              />

              <TextField
                fullWidth
                label="Description"
                value={String((selectedEdge.data as any)?.description || '')}
                onChange={(e) =>
                  saveEdgeToRelationships(selectedEdge.id, {}, { description: e.target.value })
                }
                sx={{ mb: 2 }}
              />

              <TextField
                fullWidth
                label="On Delete"
                select
                value={String((selectedEdge.data as any)?.onDelete || 'restrict')}
                onChange={(e) =>
                  saveEdgeToRelationships(selectedEdge.id, {}, { onDelete: e.target.value })
                }
                sx={{ mb: 2 }}
              >
                {['restrict', 'cascade', 'setNull', 'noAction'].map((t) => (
                  <MenuItem key={t} value={t}>
                    {t}
                  </MenuItem>
                ))}
              </TextField>

              <TextField
                fullWidth
                label="On Update"
                select
                value={String((selectedEdge.data as any)?.onUpdate || 'restrict')}
                onChange={(e) =>
                  saveEdgeToRelationships(selectedEdge.id, {}, { onUpdate: e.target.value })
                }
                sx={{ mb: 2 }}
              >
                {['restrict', 'cascade', 'noAction'].map((t) => (
                  <MenuItem key={t} value={t}>
                    {t}
                  </MenuItem>
                ))}
              </TextField>

              <Stack direction="row" spacing={1}>
                <Button
                  color="error"
                  variant="outlined"
                  onClick={() => deleteRelationship(selectedEdge.id)}
                >
                  Delete
                </Button>
                <Box sx={{ flex: 1 }} />
                <Tooltip title="Edges auto-save.">
                  <Typography sx={{ opacity: 0.6, fontSize: 12, alignSelf: 'center' }}>
                    Autosaved
                  </Typography>
                </Tooltip>
              </Stack>
            </>
          )}
        </Box>
      </Drawer>
    </Box>
  );
}
