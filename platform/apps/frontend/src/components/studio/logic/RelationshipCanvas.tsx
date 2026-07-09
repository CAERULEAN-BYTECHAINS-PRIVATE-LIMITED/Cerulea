"use client";

import React, { useCallback, useEffect, useMemo, useState } from "react";
import {
  Box,
  Divider,
  IconButton,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
  FormControlLabel,
  Switch,
} from "@mui/material";
import AddIcon from "@mui/icons-material/Add";
import DeleteOutlineIcon from "@mui/icons-material/DeleteOutline";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  addEdge,
  useEdgesState,
  useNodesState,
  type Connection,
  type Edge,
  type Node,
  type OnConnect,
  MarkerType,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";

export type FieldType =
  | "uuid"
  | "string"
  | "number"
  | "boolean"
  | "datetime"
  | "json"
  | "text"
  | "email"
  | "phone";

export type EntityField = {
  id: string;
  name: string;
  type: FieldType;
  required?: boolean;
  unique?: boolean;
  indexed?: boolean;
  apiExposed?: boolean;
  pii?: boolean;
  encrypted?: boolean;
  onChain?: boolean;
  immutable?: boolean;
  description?: string;
  defaultValue?: string;
};

export type EntityDef = {
  id: string;
  name: string;
  fields: EntityField[];
};

export type RelationshipKind = "one-to-one" | "one-to-many" | "many-to-many";

export type RelationshipDef = {
  id: string;
  fromEntityId: string;
  toEntityId: string;
  kind: RelationshipKind;
  name?: string;
  cascadeDelete?: boolean;
};

type Props = {
  entities: EntityDef[];
  relationships: RelationshipDef[];

  // Parent might be using one of these. We support both.
  onChange?: (relationships: RelationshipDef[]) => void;
  setRelationships?: (relationships: RelationshipDef[]) => void;
};

function makeId(prefix: string) {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}`;
}

const PANEL_SX = {
  bgcolor: "rgba(10, 16, 28, 0.40)",
  border: "1px solid rgba(255,255,255,0.10)",
  backdropFilter: "blur(14px)",
  borderRadius: 6,
  overflow: "hidden",
};

export default function RelationshipCanvas({
  entities,
  relationships,
  onChange,
  setRelationships,
}: Props) {
  // FIX #1: prevent "onChange is not a function"
  const emitChange = useCallback(
    (next: RelationshipDef[]) => {
      if (typeof onChange === "function") return onChange(next);
      if (typeof setRelationships === "function") return setRelationships(next);
      // If neither exists, do nothing (prevents runtime crash)
    },
    [onChange, setRelationships]
  );

  const initialNodes = useMemo<Node[]>(
    () =>
      entities.map((e, idx) => ({
        id: e.id,
        type: "default",
        data: { label: e.name },
        position: { x: 120 + (idx % 3) * 260, y: 120 + Math.floor(idx / 3) * 160 },
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [entities.map((e) => e.id).join("|"), entities.map((e) => e.name).join("|")]
  );

  const initialEdges = useMemo<Edge[]>(
    () =>
      relationships.map((r) => ({
        id: r.id,
        source: r.fromEntityId,
        target: r.toEntityId,
        markerEnd: { type: MarkerType.ArrowClosed },
        data: { kind: r.kind, name: r.name ?? "", cascadeDelete: !!r.cascadeDelete },
        label: r.name?.trim() ? r.name : r.kind,
        style: { strokeWidth: 2 },
      })),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      relationships
        .map(
          (r) =>
            `${r.id}:${r.fromEntityId}:${r.toEntityId}:${r.kind}:${r.name ?? ""}:${!!r.cascadeDelete}`
        )
        .join("|"),
    ]
  );

  const [nodes, setNodes, onNodesChange] = useNodesState(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  useEffect(() => {
    setNodes((prev) => {
      const byId = new Map(prev.map((n) => [n.id, n]));
      const next: Node[] = [];
      for (const e of entities) {
        const existing = byId.get(e.id);
        if (existing) {
          next.push({
            ...existing,
            data: { ...(existing.data as any), label: e.name },
          });
        } else {
          next.push({
            id: e.id,
            type: "default",
            data: { label: e.name },
            position: { x: 160 + Math.random() * 220, y: 140 + Math.random() * 220 },
          });
        }
      }
      return next;
    });

    setEdges((prev) =>
      prev.filter(
        (ed) =>
          entities.some((e) => e.id === ed.source) && entities.some((e) => e.id === ed.target)
      )
    );
  }, [entities, setNodes, setEdges]);

  useEffect(() => {
    setEdges(initialEdges);
  }, [initialEdges, setEdges]);

  const [filter, setFilter] = useState("");
  const [selectedEntityId, setSelectedEntityId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  const selectedEntity = useMemo(
    () => (selectedEntityId ? entities.find((e) => e.id === selectedEntityId) ?? null : null),
    [entities, selectedEntityId]
  );

  const selectedRelationship = useMemo(
    () => (selectedEdgeId ? relationships.find((r) => r.id === selectedEdgeId) ?? null : null),
    [relationships, selectedEdgeId]
  );

  const filteredEntities = useMemo(() => {
    const q = filter.trim().toLowerCase();
    if (!q) return entities;
    return entities.filter((e) => e.name.toLowerCase().includes(q));
  }, [entities, filter]);

  const commitRelationshipsFromEdges = useCallback(
    (nextEdges: Edge[]) => {
      const next: RelationshipDef[] = nextEdges.map((ed) => {
        const data: any = ed.data ?? {};
        const kind: RelationshipKind = (data.kind ?? "one-to-many") as RelationshipKind;
        return {
          id: ed.id,
          fromEntityId: String(ed.source),
          toEntityId: String(ed.target),
          kind,
          name: (data.name ?? ed.label ?? "").toString(),
          cascadeDelete: !!data.cascadeDelete,
        };
      });
      emitChange(next); // FIX #1 applied here
    },
    [emitChange]
  );

  const onConnect: OnConnect = useCallback(
    (params: Connection) => {
      const newEdge: Edge = {
        id: makeId("rel"),
        source: params.source ?? "",
        target: params.target ?? "",
        markerEnd: { type: MarkerType.ArrowClosed },
        data: { kind: "one-to-many" as RelationshipKind, name: "", cascadeDelete: false },
        label: "one-to-many",
        style: { strokeWidth: 2 },
      };
      setEdges((eds) => {
        const next = addEdge(newEdge, eds);
        commitRelationshipsFromEdges(next);
        return next;
      });
      setSelectedEntityId(null);
      setSelectedEdgeId(newEdge.id);
    },
    [commitRelationshipsFromEdges, setEdges]
  );

  const onPaneClick = useCallback(() => {
    setSelectedEntityId(null);
    setSelectedEdgeId(null);
  }, []);

  const onNodeClick = useCallback((_evt: any, node: Node) => {
    setSelectedEdgeId(null);
    setSelectedEntityId(node.id);
  }, []);

  const onEdgeClick = useCallback((_evt: any, edge: Edge) => {
    setSelectedEntityId(null);
    setSelectedEdgeId(edge.id);
  }, []);

  const addEntityToCanvas = useCallback(
    (entityId: string) => {
      setSelectedEdgeId(null);
      setSelectedEntityId(entityId);
      setNodes((prev) => {
        if (prev.some((n) => n.id === entityId)) return prev;
        const ent = entities.find((e) => e.id === entityId);
        if (!ent) return prev;
        return [
          ...prev,
          {
            id: entityId,
            type: "default",
            data: { label: ent.name },
            position: { x: 220 + Math.random() * 260, y: 160 + Math.random() * 240 },
          },
        ];
      });
    },
    [entities, setNodes]
  );

  const deleteRelationship = useCallback(
    (relId: string) => {
      setEdges((prev) => {
        const next = prev.filter((e) => e.id !== relId);
        commitRelationshipsFromEdges(next);
        return next;
      });
      setSelectedEdgeId(null);
    },
    [commitRelationshipsFromEdges, setEdges]
  );

  const updateRelationship = useCallback(
    (rel: RelationshipDef) => {
      const next = relationships.map((r) => (r.id === rel.id ? rel : r));
      emitChange(next); // FIX #1 applied here

      setEdges((prev) =>
        prev.map((e) =>
          e.id === rel.id
            ? {
                ...e,
                source: rel.fromEntityId,
                target: rel.toEntityId,
                data: { kind: rel.kind, name: rel.name ?? "", cascadeDelete: !!rel.cascadeDelete },
                label: rel.name?.trim() ? rel.name : rel.kind,
              }
            : e
        )
      );
    },
    [relationships, emitChange, setEdges]
  );

  return (
    <Box
      sx={{
        display: "grid",
        gridTemplateColumns: { xs: "1fr", lg: "320px 1fr 360px" },
        gap: 2,
        height: "100%",
        minHeight: 520,
        alignItems: "stretch",
      }}
    >
      {/* Left: Entities */}
      <Paper
        variant="outlined"
        sx={{
          ...PANEL_SX,
          display: "flex",
          flexDirection: "column",
          minHeight: 0,
        }}
      >
        {/* FIX #2: more padding so text doesn’t get clipped by rounded corners */}
        <Stack
          direction="row"
          alignItems="center"
          justifyContent="space-between"
          sx={{ px: 3, pt: 3, pb: 1.5 }}
        >
          <Typography variant="h6" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
            Entities
          </Typography>
          <IconButton size="small" disabled>
            <AddIcon fontSize="small" />
          </IconButton>
        </Stack>

        <Box sx={{ px: 3, pb: 1.5 }}>
          <TextField
            fullWidth
            size="small"
            placeholder="Search entities…"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            InputProps={{ sx: { bgcolor: "rgba(255,255,255,0.05)" } }}
          />
        </Box>

        <Divider sx={{ borderColor: "rgba(255,255,255,0.08)" }} />

        <Box sx={{ p: 2.5, overflowY: "auto", minHeight: 0 }}>
          <Stack spacing={1}>
            {filteredEntities.map((e) => {
              const isSelected = e.id === selectedEntityId;
              return (
                <Paper
                  key={e.id}
                  variant="outlined"
                  onClick={() => addEntityToCanvas(e.id)}
                  sx={{
                    cursor: "pointer",
                    p: 1.25,
                    borderRadius: 4,
                    bgcolor: isSelected ? "rgba(56, 132, 255, 0.18)" : "rgba(255,255,255,0.04)",
                    borderColor: isSelected ? "rgba(56, 132, 255, 0.55)" : "rgba(255,255,255,0.10)",
                  }}
                >
                  <Stack direction="row" alignItems="center" justifyContent="space-between" gap={1}>
                    <Box sx={{ minWidth: 0 }}>
                      <Typography sx={{ fontWeight: 700 }} noWrap>
                        {e.name}
                      </Typography>
                      <Typography variant="caption" sx={{ opacity: 0.75 }}>
                        {e.fields?.length ?? 0} fields
                      </Typography>
                    </Box>
                    <Box sx={{ flex: "0 0 auto" }}>
                      <IconButton size="small" disabled sx={{ opacity: 0.4 }}>
                        <DeleteOutlineIcon fontSize="small" />
                      </IconButton>
                    </Box>
                  </Stack>
                </Paper>
              );
            })}

            {filteredEntities.length === 0 && (
              <Typography variant="body2" sx={{ opacity: 0.7 }}>
                No entities match your search.
              </Typography>
            )}
          </Stack>
        </Box>
      </Paper>

      {/* Center: Canvas */}
      <Paper
        variant="outlined"
        sx={{
          ...PANEL_SX,
          p: 0,
          minHeight: 520,
          display: "flex",
          flexDirection: "column",
          minWidth: 0,
        }}
      >
        {/* FIX #2: more padding so text doesn’t get clipped by rounded corners */}
        <Box sx={{ px: 3, pt: 3, pb: 1.5 }}>
          <Typography variant="h6" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
            Canvas
          </Typography>
          <Typography variant="body2" sx={{ opacity: 0.75, mt: 0.25 }}>
            Drag from one entity to another to create a relationship.
          </Typography>
        </Box>

        <Divider sx={{ borderColor: "rgba(255,255,255,0.08)" }} />

        <Box sx={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
          <ReactFlow
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onPaneClick={onPaneClick}
            onNodeClick={onNodeClick}
            onEdgeClick={onEdgeClick}
            fitView
            proOptions={{ hideAttribution: true }}
            defaultEdgeOptions={{
              markerEnd: { type: MarkerType.ArrowClosed },
              style: { strokeWidth: 2 },
            }}
          >
            <Background />
            <MiniMap pannable zoomable />
            <Controls />
          </ReactFlow>
        </Box>
      </Paper>

      {/* Right: Properties */}
      <Paper
        variant="outlined"
        sx={{
          ...PANEL_SX,
          display: "flex",
          flexDirection: "column",
          minHeight: 0,
        }}
      >
        {/* FIX #2: more padding so text doesn’t get clipped by rounded corners */}
        <Stack
          direction="row"
          alignItems="center"
          justifyContent="space-between"
          sx={{ px: 3, pt: 3, pb: 1.5 }}
        >
          <Typography variant="h6" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
            Properties
          </Typography>
          <IconButton
            size="small"
            onClick={() => {
              setSelectedEntityId(null);
              setSelectedEdgeId(null);
            }}
            sx={{ opacity: selectedEntityId || selectedEdgeId ? 1 : 0.4 }}
            disabled={!selectedEntityId && !selectedEdgeId}
          >
            ✕
          </IconButton>
        </Stack>

        <Divider sx={{ borderColor: "rgba(255,255,255,0.08)" }} />

        <Box sx={{ p: 2.5, overflowY: "auto", minHeight: 0 }}>
          {!selectedEntity && !selectedRelationship && (
            <Typography variant="body2" sx={{ opacity: 0.75 }}>
              Select an entity or an edge to edit its properties.
            </Typography>
          )}

          {selectedEntity && (
            <Stack spacing={2}>
              <Typography variant="subtitle2" sx={{ opacity: 0.8 }}>
                Entity
              </Typography>
              <TextField
                label="Entity name"
                size="small"
                value={selectedEntity.name}
                disabled
                helperText="Entity names are edited in the Fields tab."
              />
            </Stack>
          )}

          {selectedRelationship && (
            <Stack spacing={2}>
              <Typography variant="subtitle2" sx={{ opacity: 0.8 }}>
                Relationship
              </Typography>

              <TextField
                label="Name"
                size="small"
                value={selectedRelationship.name ?? ""}
                onChange={(e) => updateRelationship({ ...selectedRelationship, name: e.target.value })}
                placeholder="Optional (e.g., owns, createdBy)"
              />

              <TextField
                select
                label="Type"
                size="small"
                value={selectedRelationship.kind}
                onChange={(e) =>
                  updateRelationship({ ...selectedRelationship, kind: e.target.value as RelationshipKind })
                }
              >
                <MenuItem value="one-to-one">one-to-one</MenuItem>
                <MenuItem value="one-to-many">one-to-many</MenuItem>
                <MenuItem value="many-to-many">many-to-many</MenuItem>
              </TextField>

              <TextField
                select
                label="From"
                size="small"
                value={selectedRelationship.fromEntityId}
                onChange={(e) => updateRelationship({ ...selectedRelationship, fromEntityId: e.target.value })}
              >
                {entities.map((e) => (
                  <MenuItem key={e.id} value={e.id}>
                    {e.name}
                  </MenuItem>
                ))}
              </TextField>

              <TextField
                select
                label="To"
                size="small"
                value={selectedRelationship.toEntityId}
                onChange={(e) => updateRelationship({ ...selectedRelationship, toEntityId: e.target.value })}
              >
                {entities.map((e) => (
                  <MenuItem key={e.id} value={e.id}>
                    {e.name}
                  </MenuItem>
                ))}
              </TextField>

              <FormControlLabel
                control={
                  <Switch
                    checked={!!selectedRelationship.cascadeDelete}
                    onChange={(_, checked) =>
                      updateRelationship({ ...selectedRelationship, cascadeDelete: checked })
                    }
                  />
                }
                label="Cascade delete"
              />

              <Divider sx={{ borderColor: "rgba(255,255,255,0.08)" }} />

              <Stack direction="row" justifyContent="space-between" alignItems="center">
                <Typography variant="body2" sx={{ opacity: 0.8 }}>
                  Delete relationship
                </Typography>
                <IconButton color="error" onClick={() => deleteRelationship(selectedRelationship.id)}>
                  <DeleteOutlineIcon />
                </IconButton>
              </Stack>
            </Stack>
          )}
        </Box>
      </Paper>
    </Box>
  );
}
