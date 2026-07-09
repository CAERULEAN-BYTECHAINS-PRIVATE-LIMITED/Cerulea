// apps/frontend/src/components/studio/logic/LogicCanvas.tsx
"use client";

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
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
} from "@mui/material";
import DeleteOutlineIcon from "@mui/icons-material/DeleteOutline";
import SettingsBackupRestoreIcon from "@mui/icons-material/SettingsBackupRestore";
import RedoIcon from "@mui/icons-material/Redo";
import ZoomInIcon from "@mui/icons-material/ZoomIn";
import ZoomOutIcon from "@mui/icons-material/ZoomOut";
import MapIcon from "@mui/icons-material/Map";
import FitScreenIcon from "@mui/icons-material/FitScreen";
import ContentCopyIcon from "@mui/icons-material/ContentCopy";

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

export type BlockPort = { id: string; label: string; type?: string };
export type BlockDef = {
  id: string;                  // stable id of block kind (e.g., "http.get")
  label: string;               // display name
  group: string;               // palette group (Triggers, Data, Control, Chain…)
  in?: BlockPort[];            // input ports
  out?: BlockPort[];           // output ports
  params?: Record<string, any>;// default params
  track?: "dapp" | "blockchain" | "both";
};

type FlowNodeData = {
  kind: string;                // block kind id
  label: string;               // node label
  params: Record<string, any>; // editable params
};

export type LogicValue = { nodes: Node<FlowNodeData>[]; edges: Edge[] };

export type LogicCanvasProps = {
  track?: "dapp" | "blockchain";
  palette?: BlockDef[];                 // if omitted, a tiny built-in set is used
  initial?: Partial<LogicValue>;
  onChange?: (v: LogicValue) => void;
};

/* ------------------------------------------------------------------ */
/* Helpers                                                            */
/* ------------------------------------------------------------------ */

const H = 240; // canvas height calc helper base
const CANVAS_STYLE = { height: `calc(100vh - ${H}px)`, borderRadius: 16, overflow: "hidden" };
const PANEL_BG = "rgba(255,255,255,0.04)";

const id = (() => {
  let n = 1;
  return () => String(n++);
})();

function centerDropPosition(rf: ReturnType<typeof useReactFlow>) {
  const viewport = (rf as any).getViewport?.() || { x: 0, y: 0, zoom: 1 };
  const x = (window.innerWidth * 0.45 - viewport.x) / viewport.zoom;
  const y = (window.innerHeight * 0.45 - viewport.y) / viewport.zoom;
  return { x, y };
}

/* ------------------------------------------------------------------ */
/* Block Node (custom node with ports)                                */
/* ------------------------------------------------------------------ */

function Port({ type, id, label }: { type: "in" | "out"; id: string; label: string }) {
  return (
    <Handle
      type={type === "in" ? "target" : "source"}
      id={id}
      position={type === "in" ? Position.Left : Position.Right}
      style={{
        width: 10,
        height: 10,
        background: "#90caf9",
        border: "1px solid rgba(255,255,255,0.4)",
      }}
      title={label}
    />
  );
}

function BlockNode({ data }: { data: FlowNodeData }) {
  const meta = data as any;
  const portsIn: BlockPort[] = meta.__in || [];
  const portsOut: BlockPort[] = meta.__out || [];

  return (
    <Box
      sx={{
        bgcolor: "rgba(255,255,255,0.06)",
        border: "1px solid rgba(255,255,255,0.22)",
        px: 1.25,
        py: 0.75,
        borderRadius: 1.5,
        minWidth: 160,
      }}
    >
      <Typography variant="subtitle2" sx={{ fontWeight: 700, lineHeight: 1.2 }}>
        {data.label}
      </Typography>
      <Typography variant="caption" sx={{ opacity: 0.7 }}>
        {data.kind}
      </Typography>

      {/* Ports */}
      {portsIn.map((p) => (
        <Port key={`in-${p.id}`} type="in" id={p.id} label={p.label} />
      ))}
      {portsOut.map((p) => (
        <Port key={`out-${p.id}`} type="out" id={p.id} label={p.label} />
      ))}
    </Box>
  );
}

const nodeTypes = { block: BlockNode };

/* ------------------------------------------------------------------ */
/* Right Properties Drawer                                            */
/* ------------------------------------------------------------------ */

function PropertiesDrawer({
  open,
  selectedNode,
  selectedEdge,
  onUpdateNode,
  onClose,
}: {
  open: boolean;
  selectedNode: Node<FlowNodeData> | null;
  selectedEdge: Edge | null;
  onUpdateNode: (updater: (d: FlowNodeData) => FlowNodeData) => void;
  onClose: () => void;
}) {
  const [label, setLabel] = useState("");
  const [paramsText, setParamsText] = useState("{}");
  const [edgeLabel, setEdgeLabel] = useState("");

  useEffect(() => {
    if (selectedNode) {
      setLabel(selectedNode.data.label || "");
      setParamsText(JSON.stringify(selectedNode.data.params ?? {}, null, 2));
    }
  }, [selectedNode?.id]);

  useEffect(() => {
    if (selectedEdge) setEdgeLabel(String(selectedEdge.label ?? ""));
  }, [selectedEdge?.id]);

  return (
    <Drawer
      open={open}
      anchor="right"
      onClose={onClose}
      PaperProps={{ sx: { width: 360, p: 2 } }}
    >
      <Stack spacing={2}>
        <Typography variant="subtitle1" sx={{ fontWeight: 700 }}>
          {selectedNode ? "Block properties" : "Edge properties"}
        </Typography>

        {selectedNode && (
          <>
            <TextField
              label="Label"
              size="small"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              onBlur={() =>
                onUpdateNode((d) => ({ ...d, label }))
              }
            />

            <TextField
              label="Params (JSON)"
              size="small"
              value={paramsText}
              onChange={(e) => setParamsText(e.target.value)}
              multiline
              minRows={10}
              onBlur={() => {
                try {
                  const parsed = JSON.parse(paramsText || "{}");
                  onUpdateNode((d) => ({ ...d, params: parsed }));
                } catch {
                  // keep text, user can fix JSON
                }
              }}
            />

            <Typography variant="caption" sx={{ opacity: 0.7 }}>
              Tip: params are merged into the block at runtime.
            </Typography>
          </>
        )}

        {selectedEdge && (
          <>
            <TextField
              label="Edge label"
              size="small"
              value={edgeLabel}
              onChange={(e) => setEdgeLabel(e.target.value)}
              onBlur={() => {
                selectedEdge.label = edgeLabel;
              }}
            />
            <Typography variant="caption" sx={{ opacity: 0.7 }}>
              Use labels like “success”, “error”, “true/false”, or any path name.
            </Typography>
          </>
        )}
      </Stack>
    </Drawer>
  );
}

/* ------------------------------------------------------------------ */
/* Inner canvas (needs ReactFlow context)                              */
/* ------------------------------------------------------------------ */

function InnerCanvas({
  track = "dapp",
  palette = [],
  initial,
  onChange,
}: LogicCanvasProps) {
  const rf = useReactFlow();

  // nodes / edges
  const [nodes, setNodes, onNodesChange] = useNodesState<FlowNodeData>(initial?.nodes || []);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initial?.edges || []);

  // UI: palette / minimap / selection
  const [q, setQ] = useState("");
  const [showMiniMap, setShowMiniMap] = useState(false);
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedEdgeId, setSelectedEdgeId] = useState<string | null>(null);

  // Undo/redo
  const undo = useRef<LogicValue[]>([]);
  const redo = useRef<LogicValue[]>([]);
  const snapshot = useCallback(() => {
    undo.current.push({ nodes: structuredClone(nodes), edges: structuredClone(edges) });
    if (undo.current.length > 50) undo.current.shift();
    redo.current = [];
  }, [nodes, edges]);

  const commit = useCallback(
    (n: Node<FlowNodeData>[], e: Edge[]) => {
      setNodes(n);
      setEdges(e);
      onChange?.({ nodes: n, edges: e });
    },
    [onChange, setNodes, setEdges]
  );

  // Palette (filtered per track)
  const blocks = useMemo(
    () =>
      (palette.length ? palette : DEFAULT_BLOCKS).filter(
        (b) => !b.track || b.track === "both" || b.track === track
      ),
    [palette, track]
  );
  const groups = useMemo(() => {
    const g = new Map<string, BlockDef[]>();
    for (const b of blocks) {
      const arr = g.get(b.group) || [];
      arr.push(b);
      g.set(b.group, arr);
    }
    return Array.from(g.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([name, list]) => ({
        name,
        list: list
          .filter((x) =>
            (x.label + " " + x.id).toLowerCase().includes(q.trim().toLowerCase())
          )
          .sort((a, b) => a.label.localeCompare(b.label)),
      }));
  }, [blocks, q]);

  // Add node
  const addBlock = useCallback(
    (b: BlockDef) => {
      snapshot();
      const pos = centerDropPosition(rf);
      const newNode: Node<FlowNodeData> = {
        id: id(),
        type: "block",
        position: pos,
        data: {
          kind: b.id,
          label: b.label,
          params: structuredClone(b.params || {}),
          // store ports on data for the node renderer
          __in: b.in || [],
          __out: b.out || [],
        } as any,
      };
      const n = [...nodes, newNode];
      commit(n, edges);
      // ensure it is in view
      setTimeout(() => rf.fitView({ padding: 0.2 }), 0);
    },
    [rf, nodes, edges, snapshot, commit]
  );

  // Connect
  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target) return;
      snapshot();
      const next = addEdge(
        {
          ...c,
          animated: false,
          label: "next",
        },
        edges
      );
      commit(nodes, next);
    },
    [edges, nodes, commit, snapshot]
  );

  // Select handlers
  const onNodeClick = useCallback((_: any, n: Node<FlowNodeData>) => {
    setSelectedEdgeId(null);
    setSelectedNodeId(n.id);
  }, []);
  const onEdgeClick = useCallback((_: any, e: Edge) => {
    setSelectedNodeId(null);
    setSelectedEdgeId(e.id);
  }, []);

  // Update node data from drawer
  const updateNode = useCallback(
    (updater: (d: FlowNodeData) => FlowNodeData) => {
      if (!selectedNodeId) return;
      snapshot();
      const n = nodes.map((nn) =>
        nn.id === selectedNodeId ? { ...nn, data: updater(nn.data) } : nn
      );
      commit(n, edges);
    },
    [selectedNodeId, nodes, edges, commit, snapshot]
  );

  // Keyboard
  useEffect(() => {
    const h = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;

      // Delete
      if (e.key === "Delete" || e.key === "Backspace") {
        const selNodes = rf.getNodes().filter((n) => n.selected);
        const selEdges = rf.getEdges().filter((ed) => ed.selected);
        if (!selNodes.length && !selEdges.length) return;

        snapshot();
        const keepNodeIds = new Set(selNodes.map((n) => n.id));
        const n = nodes.filter((nn) => !keepNodeIds.has(nn.id));
        const e2 = edges.filter(
          (ed) => !keepNodeIds.has(ed.source) && !keepNodeIds.has(ed.target) && !ed.selected
        );
        commit(n, e2);
      }

      // Duplicate
      if (mod && e.key.toLowerCase() === "d") {
        const sel = rf.getNodes().filter((n) => n.selected);
        if (!sel.length) return;
        snapshot();
        const dup = sel.map((n) => ({
          ...n,
          id: id(),
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
  }, [nodes, edges, rf, commit, snapshot]);

  // Toolbar actions
  const doFit = () => rf.fitView({ padding: 0.2 });
  const zoomIn = () => rf.zoomIn();
  const zoomOut = () => rf.zoomOut();

  // Keep parent in sync
  useEffect(() => {
    onChange?.({ nodes, edges });
  }, [nodes, edges]); // eslint-disable-line react-hooks/exhaustive-deps

  const selectedNode = nodes.find((n) => n.id === selectedNodeId) || null;
  const selectedEdge = edges.find((e) => e.id === selectedEdgeId) || null;

  return (
    <Box sx={{ px: 3, display: "grid", gridTemplateColumns: "320px 1fr", gap: 2 }}>
      {/* Left: Palette --------------------------------------------------- */}
      <Paper variant="outlined" sx={{ p: 0, borderRadius: 2, overflow: "hidden", bgcolor: PANEL_BG }}>
        <Box sx={{ p: 1.5 }}>
          <Stack direction="row" alignItems="center" spacing={1}>
            <Typography variant="subtitle1" sx={{ fontWeight: 600 }}>
              Blocks
            </Typography>
            <Box sx={{ flex: 1 }} />
            <Tooltip title="Duplicate (Ctrl/Cmd+D), Delete (Del), Select all (Ctrl/Cmd+A)">
              <IconButton size="small">
                <ContentCopyIcon fontSize="small" />
              </IconButton>
            </Tooltip>
          </Stack>
          <TextField
            fullWidth
            size="small"
            placeholder="Search blocks…"
            value={q}
            onChange={(e) => setQ(e.target.value)}
            sx={{ mt: 1 }}
          />
        </Box>
        <Divider sx={{ opacity: 0.25 }} />
        <Box sx={{ height: `calc(100vh - ${H + 60}px)`, overflow: "auto", p: 1.25 }}>
          {groups.map(({ name, list }) =>
            list.length ? (
              <Box key={name} sx={{ mb: 1.25 }}>
                <Typography variant="overline" sx={{ opacity: 0.7 }}>
                  {name}
                </Typography>
                <List dense sx={{ pt: 0 }}>
                  {list.map((b) => (
                    <ListItemButton key={b.id} onClick={() => addBlock(b)} sx={{ borderRadius: 1 }}>
                      <ListItemText primary={b.label} secondary={b.id} />
                    </ListItemButton>
                  ))}
                </List>
              </Box>
            ) : null
          )}
        </Box>
      </Paper>

      {/* Center: Canvas -------------------------------------------------- */}
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
            Click a block or edge to edit on the right
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

      {/* Right: Properties Drawer (slides over canvas) ------------------- */}
      <PropertiesDrawer
        open={Boolean(selectedNode || selectedEdge)}
        selectedNode={selectedNode}
        selectedEdge={selectedEdge}
        onUpdateNode={updateNode}
        onClose={() => {
          setSelectedNodeId(null);
          setSelectedEdgeId(null);
          // also clear selection in the canvas
          rf.setNodes(nodes.map((n) => ({ ...n, selected: false })));
          rf.setEdges(edges.map((e) => ({ ...e, selected: false })));
        }}
      />
    </Box>
  );
}

/* ------------------------------------------------------------------ */
/* Default palette (tiny, to keep file self-contained)                 */
/* You can pass a bigger list from Step-2; this is only a fallback.    */
/* ------------------------------------------------------------------ */

const DEFAULT_BLOCKS: BlockDef[] = [
  // Triggers
  { id: "trigger.userAction", label: "User Action", group: "Triggers", out: [{ id: "next", label: "next" }], track: "both" },
  { id: "trigger.webhook", label: "Webhook", group: "Triggers", out: [{ id: "next", label: "next" }], track: "both" },

  // Data
  { id: "db.read", label: "DB Read", group: "Data", in: [{ id: "in", label: "in" }], out: [{ id: "out", label: "out" }], params: { entity: "", where: {} }, track: "both" },
  { id: "db.create", label: "DB Create", group: "Data", in: [{ id: "in", label: "in" }], out: [{ id: "out", label: "out" }], params: { entity: "", values: {} }, track: "both" },

  // Control
  { id: "control.if", label: "If", group: "Control", in: [{ id: "in", label: "in" }], out: [{ id: "true", label: "true" }, { id: "false", label: "false" }], params: { expr: "" }, track: "both" },
  { id: "control.foreach", label: "For Each", group: "Control", in: [{ id: "in", label: "in" }], out: [{ id: "each", label: "each" }, { id: "done", label: "done" }], params: { list: "" }, track: "both" },

  // HTTP
  { id: "http.request", label: "HTTP Request", group: "Integrations", in: [{ id: "in", label: "in" }], out: [{ id: "out", label: "out" }], params: { method: "GET", url: "", headers: {}, body: {} }, track: "both" },

  // Chain
  { id: "chain.transfer", label: "Chain Transfer", group: "Blockchain", in: [{ id: "in", label: "in" }], out: [{ id: "sent", label: "sent" }], params: { to: "", amount: "", token: "CER" }, track: "blockchain" },
];

/* ------------------------------------------------------------------ */
/* Export (wrapped with provider)                                      */
/* ------------------------------------------------------------------ */

export default function LogicCanvas(props: LogicCanvasProps) {
  return (
    <ReactFlowProvider>
      <InnerCanvas {...props} />
    </ReactFlowProvider>
  );
}
