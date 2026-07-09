"use client";

import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  Box,
  Paper,
  Stack,
  Typography,
  Divider,
  IconButton,
  Tooltip,
  Button,
  TextField,
  List,
  ListItemButton,
  ListItemText,
  Drawer,
  MenuItem,
  Select,
  FormControl,
  InputLabel,
  Checkbox,
  FormControlLabel,
} from "@mui/material";

import DeleteOutlineIcon from "@mui/icons-material/DeleteOutline";
import SettingsBackupRestoreIcon from "@mui/icons-material/SettingsBackupRestore";
import RedoIcon from "@mui/icons-material/Redo";
import ZoomInIcon from "@mui/icons-material/ZoomIn";
import ZoomOutIcon from "@mui/icons-material/ZoomOut";
import MapIcon from "@mui/icons-material/Map";
import FitScreenIcon from "@mui/icons-material/FitScreen";
import ClearAllIcon from "@mui/icons-material/ClearAll";

import ReactFlow, {
  Background,
  Controls,
  MiniMap,
  addEdge,
  useEdgesState,
  useNodesState,
  ReactFlowProvider,
  useReactFlow,
  Position,
  Handle,
  Connection,
  Edge,
  Node,
} from "reactflow";
import "reactflow/dist/style.css";

/* ------------------------------------------------------------------ */
/* Types                                                              */
/* ------------------------------------------------------------------ */

export type Field = { name: string; type: string };
export type Entity = { id: string; name: string; fields?: Field[] };

export type RelEdgeData = {
  kind: "relationship";
  name?: string;
  notes?: string;
  cardinality: "1-1" | "1-N" | "N-1" | "N-M";
  cascadeDelete?: boolean;
};

export type RelationshipCanvasValue = {
  nodes: Node[];
  edges: Edge<RelEdgeData>[];
};

export type RelationshipCanvasProps = {
  entities: Entity[]; // full set from Fields tab
  initial?: Partial<RelationshipCanvasValue>;
  onChange?: (v: RelationshipCanvasValue) => void;
  onRequestEntityDelete?: (entityId: string) => void; // if user deletes an entity here
};

/* ------------------------------------------------------------------ */
/* Layout helpers                                                      */
/* ------------------------------------------------------------------ */

const H = 240; // header height padding baseline
const PANEL_BG = "rgba(255,255,255,0.04)";
const CANVAS_STYLE = {
  height: `calc(100vh - ${H}px)`,
  borderRadius: 16,
  overflow: "hidden",
};

const genId = (() => {
  let n = 1;
  return () => String(n++);
})();

function gridPosition(idx: number) {
  const x = (idx % 4) * 260 + 80;
  const y = Math.floor(idx / 4) * 180 + 80;
  return { x, y };
}

/* ------------------------------------------------------------------ */
/* Entity Node renderer                                               */
/* ------------------------------------------------------------------ */

function EntityNode({ data }: { data: { name: string; preview: string[] } }) {
  const { name, preview } = data;
  return (
    <Box
      sx={{
        bgcolor: "rgba(255,255,255,0.06)",
        border: "1px solid rgba(255,255,255,0.22)",
        px: 1.25,
        py: 0.75,
        borderRadius: 1.5,
        minWidth: 200,
      }}
    >
      <Typography variant="subtitle2" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
        {name}
      </Typography>
      <Divider sx={{ my: 0.5, opacity: 0.25 }} />
      <Stack spacing={0.25}>
        {preview.slice(0, 4).map((f) => (
          <Typography key={f} variant="caption" sx={{ opacity: 0.8 }}>
            • {f}
          </Typography>
        ))}
        {preview.length > 4 && (
          <Typography variant="caption" sx={{ opacity: 0.6 }}>
            +{preview.length - 4} more…
          </Typography>
        )}
      </Stack>

      {/* Ports */}
      <Handle
        type="source"
        id="out"
        position={Position.Right}
        style={{ width: 10, height: 10, background: "#90caf9", border: "1px solid rgba(255,255,255,0.4)" }}
      />
      <Handle
        type="target"
        id="in"
        position={Position.Left}
        style={{ width: 10, height: 10, background: "#90caf9", border: "1px solid rgba(255,255,255,0.4)" }}
      />
    </Box>
  );
}

const nodeTypes = { entity: EntityNode };

/* ------------------------------------------------------------------ */
/* Right Properties Drawer for edges                                  */
/* ------------------------------------------------------------------ */

function RelPropertiesDrawer({
  open,
  edge,
  onUpdate,
  onDelete,
  onClose,
}: {
  open: boolean;
  edge: Edge<RelEdgeData> | null;
  onUpdate: (data: RelEdgeData) => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const [name, setName] = useState("");
  const [notes, setNotes] = useState("");
  const [card, setCard] = useState<RelEdgeData["cardinality"]>("1-1");
  const [cascade, setCascade] = useState(false);

  useEffect(() => {
    if (!edge) return;
    setName(edge.data?.name || "");
    setNotes(edge.data?.notes || "");
    setCard(edge.data?.cardinality || "1-1");
    setCascade(Boolean(edge.data?.cascadeDelete));
  }, [edge?.id]);

  const push = () =>
    onUpdate({
      kind: "relationship",
      name,
      notes,
      cardinality: card,
      cascadeDelete: cascade,
    });

  return (
    <Drawer
      open={open}
      anchor="right"
      onClose={onClose}
      PaperProps={{ sx: { width: 360, p: 2 } }}
    >
      <Stack spacing={2}>
        <Typography variant="subtitle1" sx={{ fontWeight: 700 }}>
          Relationship properties
        </Typography>

        <TextField
          label="Relationship name"
          size="small"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onBlur={push}
        />

        <FormControl size="small">
          <InputLabel id="card-lbl">Cardinality</InputLabel>
          <Select
            labelId="card-lbl"
            label="Cardinality"
            value={card}
            onChange={(e) =>
              setCard(e.target.value as RelEdgeData["cardinality"])
            }
            onBlur={push}
          >
            <MenuItem value="1-1">1 — 1</MenuItem>
            <MenuItem value="1-N">1 — N</MenuItem>
            <MenuItem value="N-1">N — 1</MenuItem>
            <MenuItem value="N-M">N — M</MenuItem>
          </Select>
        </FormControl>

        <FormControlLabel
          control={
            <Checkbox
              checked={cascade}
              onChange={(e) => setCascade(e.target.checked)}
              onBlur={push}
            />
          }
          label="Cascade delete"
        />

        <TextField
          label="Notes"
          size="small"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
          onBlur={push}
          multiline
          minRows={5}
        />

        <Stack direction="row" spacing={1}>
          <Button
            color="error"
            startIcon={<DeleteOutlineIcon />}
            onClick={onDelete}
            variant="outlined"
          >
            Delete relationship
          </Button>
          <Box sx={{ flex: 1 }} />
        </Stack>
      </Stack>
    </Drawer>
  );
}

/* ------------------------------------------------------------------ */
/* Inner canvas                                                        */
/* ------------------------------------------------------------------ */

function InnerCanvas({
  entities,
  initial,
  onChange,
  onRequestEntityDelete,
}: RelationshipCanvasProps) {
  const rf = useReactFlow();

  // Build nodes from entities if not present
  const bootstrapNodes: Node[] =
    initial?.nodes && initial.nodes.length
      ? initial.nodes
      : entities.map((e, i) => ({
          id: e.id,
          type: "entity",
          position: gridPosition(i),
          data: {
            name: e.name,
            preview: (e.fields || []).map((f) => `${f.name}: ${f.type}`),
          },
        }));

  const [nodes, setNodes, onNodesChange] = useNodesState(bootstrapNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge<RelEdgeData>>(
    (initial?.edges as Edge<RelEdgeData>[]) || []
  );

  const [showMiniMap, setShowMiniMap] = useState(false);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  // Undo/redo stacks
  const undo = useRef<RelationshipCanvasValue[]>([]);
  const redo = useRef<RelationshipCanvasValue[]>([]);
  const snapshot = useCallback(() => {
    undo.current.push({
      nodes: structuredClone(nodes),
      edges: structuredClone(edges),
    });
    if (undo.current.length > 50) undo.current.shift();
    redo.current = [];
  }, [nodes, edges]);

  const commit = useCallback(
    (n: Node[], e: Edge<RelEdgeData>[]) => {
      setNodes(n);
      setEdges(e);
      onChange?.({ nodes: n, edges: e });
    },
    [setNodes, setEdges, onChange]
  );

  // Keep nodes aligned with incoming entities (add any new ones, keep positions of existing)
  useEffect(() => {
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const next = entities.map((e, i) => {
      const keep = byId.get(e.id);
      return (
        keep || {
          id: e.id,
          type: "entity",
          position: gridPosition(i),
          data: {
            name: e.name,
            preview: (e.fields || []).map((f) => `${f.name}: ${f.type}`),
          },
        }
      );
    });
    // remove nodes that no longer exist as entities
    if (next.length !== nodes.length || next.some((n, i) => n.id !== nodes[i]?.id)) {
      commit(next, edges.filter((ed) => next.find((n) => n.id === ed.source) && next.find((n) => n.id === ed.target)));
    }
  }, [entities]); // eslint-disable-line react-hooks/exhaustive-deps

  // Selection handlers
  const onNodeClick = useCallback((_, n: Node) => {
    setSelectedNodeId(n.id);
    setSelectedEdgeId(null);
  }, []);
  const onEdgeClick = useCallback((_, e: Edge<RelEdgeData>) => {
    setSelectedNodeId(null);
    setSelectedEdgeId(e.id);
  }, []);

  // Connect entities -> relationship edge
  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target) return;
      snapshot();
      const e: Edge<RelEdgeData> = addEdge<RelEdgeData>(
        {
          ...c,
          data: { kind: "relationship", cardinality: "1-1" },
          label: "1 — 1",
        },
        edges
      ) as any;
      commit(nodes, e as any);
    },
    [edges, nodes, commit, snapshot]
  );

  // Keyboard shortcuts
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;

      // Delete selected nodes/edges
      if (e.key === "Delete" || e.key === "Backspace") {
        const selNodes = rf.getNodes().filter((n) => n.selected);
        const selEdges = rf.getEdges().filter((ed) => ed.selected);

        if (!selNodes.length && !selEdges.length) return;

        snapshot();
        // If deleting entities here, notify parent to drop them from Fields tab too
        selNodes.forEach((n) => onRequestEntityDelete?.(n.id));

        const keepNodeIds = new Set(selNodes.map((n) => n.id));
        const n = nodes.filter((nn) => !keepNodeIds.has(nn.id));
        const e2 = edges.filter(
          (ed) => !keepNodeIds.has(ed.source) && !keepNodeIds.has(ed.target) && !ed.selected
        );
        commit(n, e2);
      }

      // Duplicate nodes
      if (mod && e.key.toLowerCase() === "d") {
        const sel = rf.getNodes().filter((n) => n.selected);
        if (!sel.length) return;
        snapshot();
        const dup = sel.map((n) => ({
          ...n,
          id: genId(),
          position: { x: n.position.x + 30, y: n.position.y + 30 },
          selected: false,
        }));
        commit([...nodes, ...dup], edges);
      }

      // Select all
      if (mod && e.key.toLowerCase() === "a") {
        e.preventDefault();
        rf.setNodes(nodes.map((n) => ({ ...n, selected: true })));
      }

      // Esc clears selection
      if (e.key === "Escape") {
        rf.setNodes(nodes.map((n) => ({ ...n, selected: false })));
        rf.setEdges(edges.map((ed) => ({ ...ed, selected: false })));
        setSelectedNodeId(null);
        setSelectedEdgeId(null);
      }

      // Undo / Redo
      if (mod && e.key.toLowerCase() === "z") {
        const snap = undo.current.pop();
        if (!snap) return;
        redo.current.push({ nodes: structuredClone(nodes), edges: structuredClone(edges) });
        commit(structuredClone(snap.nodes), structuredClone(snap.edges));
      }
      if ((mod && e.key.toLowerCase() === "y") || (mod && e.shiftKey && e.key.toLowerCase() === "z")) {
        const snap = redo.current.pop();
        if (!snap) return;
        undo.current.push({ nodes: structuredClone(nodes), edges: structuredClone(edges) });
        commit(structuredClone(snap.nodes), structuredClone(snap.edges));
      }
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, [nodes, edges, rf, commit, snapshot, onRequestEntityDelete]);

  // Toolbar actions
  const doFit = () => rf.fitView({ padding: 0.2 });
  const zoomIn = () => rf.zoomIn();
  const zoomOut = () => rf.zoomOut();

  // Left panel selection sync
  const selectEntity = (entityId: string) => {
    rf.setNodes(nodes.map((n) => ({ ...n, selected: n.id === entityId })));
    setSelectedNodeId(entityId);
    setSelectedEdgeId(null);
    const n = nodes.find((x) => x.id === entityId);
    if (n) rf.setCenter(n.position.x, n.position.y, { zoom: 1.1, duration: 300 });
  };

  // Edge properties drawer handlers
  const selectedEdge =
    edges.find((e) => e.id === selectedEdgeId) || null;

  const updateEdgeData = (data: RelEdgeData) => {
    if (!selectedEdge) return;
    snapshot();
    const next = edges.map((e) =>
      e.id === selectedEdge.id ? { ...e, data, label: prettyCard(data.cardinality) } : e
    );
    commit(nodes, next);
  };

  const deleteSelectedEdge = () => {
    if (!selectedEdge) return;
    snapshot();
    commit(
      nodes,
      edges.filter((e) => e.id !== selectedEdge.id)
    );
    setSelectedEdgeId(null);
  };

  return (
    <Box sx={{ px: 3, display: "grid", gridTemplateColumns: "320px 1fr", gap: 2 }}>
      {/* Left: Entities panel */}
      <Paper
        variant="outlined"
        sx={{
          p: 0,
          borderRadius: 2,
          overflow: "hidden",
          bgcolor: PANEL_BG,
        }}
      >
        <Box sx={{ p: 1.5 }}>
          <Stack direction="row" alignItems="center" spacing={1}>
            <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
              Entities
            </Typography>
            <Box sx={{ flex: 1 }} />
            <Tooltip title="Delete all from canvas (entities remain in Fields unless you delete them there)">
              <span>
                <IconButton
                  size="small"
                  onClick={() => {
                    snapshot();
                    commit([], []);
                  }}
                >
                  <ClearAllIcon fontSize="small" />
                </IconButton>
              </span>
            </Tooltip>
          </Stack>
        </Box>
        <Divider sx={{ opacity: 0.25 }} />
        <Box
          sx={{
            height: `calc(100vh - ${H + 60}px)`,
            overflow: "auto",
            p: 1.25,
          }}
        >
          <List dense sx={{ pt: 0 }}>
            {entities.map((e) => (
              <ListItemButton
                key={e.id}
                selected={selectedNodeId === e.id}
                onClick={() => selectEntity(e.id)}
                sx={{ borderRadius: 1 }}
              >
                <ListItemText primary={e.name} secondary={`${e.fields?.length ?? 0} fields`} />
                <Tooltip title="Delete entity (also removes its relationships)">
                  <IconButton
                    edge="end"
                    size="small"
                    onClick={(evt) => {
                      evt.stopPropagation();
                      onRequestEntityDelete?.(e.id);
                      snapshot();
                      const n = nodes.filter((nn) => nn.id !== e.id);
                      const es = edges.filter(
                        (ed) => ed.source !== e.id && ed.target !== e.id
                      );
                      commit(n, es);
                    }}
                  >
                    <DeleteOutlineIcon fontSize="small" />
                  </IconButton>
                </Tooltip>
              </ListItemButton>
            ))}
          </List>
        </Box>
      </Paper>

      {/* Center: Canvas */}
      <Paper variant="outlined" sx={{ borderRadius: 2, p: 0, overflow: "hidden", bgcolor: PANEL_BG }}>
        <Box sx={{ display: "flex", alignItems: "center", px: 1.25, py: 0.75 }}>
          <Tooltip title="Undo">
            <IconButton size="small" onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "z", ctrlKey: true }))}>
              <SettingsBackupRestoreIcon fontSize="small" />
            </IconButton>
          </Tooltip>
          <Tooltip title="Redo">
            <IconButton size="small" onClick={() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "y", ctrlKey: true }))}>
              <RedoIcon fontSize="small" />
            </IconButton>
          </Tooltip>
          <Divider orientation="vertical" flexItem sx={{ mx: 1, opacity: 0.25 }} />
          <Tooltip title="Fit view">
            <IconButton size="small" onClick={doFit}><FitScreenIcon fontSize="small" /></IconButton>
          </Tooltip>
          <Tooltip title="Zoom in"><IconButton size="small" onClick={zoomIn}><ZoomInIcon fontSize="small" /></IconButton></Tooltip>
          <Tooltip title="Zoom out"><IconButton size="small" onClick={zoomOut}><ZoomOutIcon fontSize="small" /></IconButton></Tooltip>
          <Tooltip title={showMiniMap ? "Hide minimap" : "Show minimap"}>
            <IconButton size="small" onClick={() => setShowMiniMap((v) => !v)}><MapIcon fontSize="small" /></IconButton>
          </Tooltip>
          <Box sx={{ flex: 1 }} />
          <Typography variant="caption" sx={{ opacity: 0.7 }}>
            Drag from a node’s edge to another node to create a relationship
          </Typography>
        </Box>
        <Divider sx={{ opacity: 0.25 }} />
        <Box sx={CANVAS_STYLE}>
          <ReactFlow
            nodeTypes={nodeTypes}
            nodes={nodes}
            edges={edges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
            onConnect={onConnect}
            onNodeClick={onNodeClick}
            onEdgeClick={onEdgeClick}
            fitView
          >
            <Background />
            {showMiniMap && <MiniMap pannable zoomable nodeStrokeWidth={2} />}
            <Controls />
          </ReactFlow>
        </Box>
      </Paper>

      {/* Right: relationship properties */}
      <RelPropertiesDrawer
        open={Boolean(selectedEdge)}
        edge={selectedEdge}
        onUpdate={updateEdgeData}
        onDelete={deleteSelectedEdge}
        onClose={() => setSelectedEdgeId(null)}
      />
    </Box>
  );
}

/* ------------------------------------------------------------------ */
/* Utils                                                               */
/* ------------------------------------------------------------------ */

function prettyCard(c: RelEdgeData["cardinality"]) {
  switch (c) {
    case "1-1":
      return "1 — 1";
    case "1-N":
      return "1 — N";
    case "N-1":
      return "N — 1";
    case "N-M":
      return "N — M";
  }
}

/* ------------------------------------------------------------------ */
/* Export (provider wrapper)                                           */
/* ------------------------------------------------------------------ */

export default function RelationshipCanvas(props: RelationshipCanvasProps) {
  return (
    <ReactFlowProvider>
      <InnerCanvas {...props} />
    </ReactFlowProvider>
  );
}
