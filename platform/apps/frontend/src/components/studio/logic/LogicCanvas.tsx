// FILE: src/components/studio/logic/LogicCanvas.tsx
"use client";

import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Box,
  Button,
  Chip,
  Divider,
  IconButton,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import AddIcon from "@mui/icons-material/Add";
import DeleteIcon from "@mui/icons-material/Delete";
import CloseIcon from "@mui/icons-material/Close";
import {
  Background,
  Connection,
  Controls,
  Edge,
  MiniMap,
  Node,
  ReactFlow,
  ReactFlowProvider,
  addEdge,
  Handle,
  Position,
  useEdgesState,
  useNodesState,
} from "reactflow";
import "reactflow/dist/style.css";

type BlockCategory =
  | "AUTH"
  | "AI"
  | "CONTROL"
  | "DATA"
  | "HTTP"
  | "WEB3"
  | "UTIL";

type Port = {
  id: string;
  name: string;
  kind: "in" | "out";
};

type BlockType =
  | "auth.signIn"
  | "auth.signUp"
  | "auth.requireRole"
  | "ai.llmCall"
  | "ai.classify"
  | "control.if"
  | "control.delay"
  | "data.query"
  | "data.insert"
  | "http.request"
  | "web3.read"
  | "web3.write"
  | "util.log";

type BlockDef = {
  type: BlockType;
  category: BlockCategory;
  label: string;
  description: string;
  ports: Port[];
  defaultProps: Record<string, any>;
};

type BlockNodeData = {
  blockType: BlockType;
  label: string;
  props: Record<string, any>;
  ports: Port[];
};

function uid(prefix = "id") {
  return `${prefix}_${Math.random().toString(36).slice(2, 10)}${Date.now().toString(36)}`;
}

const CANVAS_STYLE: React.CSSProperties = {
  width: "100%",
  height: "calc(100vh - 290px)",
  borderRadius: 28,
  overflow: "hidden",
  border: "1px solid rgba(255,255,255,0.12)",
  background: "rgba(10,12,18,0.35)",
  backdropFilter: "blur(16px)",
};

const BLOCKS: BlockDef[] = [
  {
    type: "auth.signIn",
    category: "AUTH",
    label: "Auth Sign-In",
    description: "Authenticate a user with credentials.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "ok", name: "ok", kind: "out" },
      { id: "fail", name: "fail", kind: "out" },
    ],
    defaultProps: { provider: "email", redirectTo: "/app" },
  },
  {
    type: "auth.signUp",
    category: "AUTH",
    label: "Auth Sign-Up",
    description: "Create a new user account.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "ok", name: "ok", kind: "out" },
      { id: "fail", name: "fail", kind: "out" },
    ],
    defaultProps: { provider: "email", sendVerification: true },
  },
  {
    type: "auth.requireRole",
    category: "AUTH",
    label: "Require Role",
    description: "Gate execution by role.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "pass", name: "pass", kind: "out" },
      { id: "deny", name: "deny", kind: "out" },
    ],
    defaultProps: { role: "admin" },
  },
  {
    type: "ai.llmCall",
    category: "AI",
    label: "LLM Call",
    description: "Call an LLM model with a prompt.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "out", name: "out", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: {
      model: "gpt-4o-mini",
      temperature: 0.2,
      maxTokens: 512,
      prompt: "Write a concise summary of {{input}}",
      system: "You are a helpful assistant.",
    },
  },
  {
    type: "ai.classify",
    category: "AI",
    label: "Classifier",
    description: "Classify input into categories.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "out", name: "out", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: { labels: ["safe", "unsafe"], threshold: 0.5 },
  },
  {
    type: "control.if",
    category: "CONTROL",
    label: "If",
    description: "Conditional branch based on expression.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "true", name: "true", kind: "out" },
      { id: "false", name: "false", kind: "out" },
    ],
    defaultProps: { expression: "ctx.ok === true" },
  },
  {
    type: "control.delay",
    category: "CONTROL",
    label: "Delay",
    description: "Wait for N milliseconds.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "out", name: "out", kind: "out" },
    ],
    defaultProps: { ms: 1000 },
  },
  {
    type: "data.query",
    category: "DATA",
    label: "DB Query",
    description: "Read from DB using a query.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "rows", name: "rows", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: { entity: "User", where: "id = {{ctx.userId}}", limit: 1 },
  },
  {
    type: "data.insert",
    category: "DATA",
    label: "DB Insert",
    description: "Insert a record into DB.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "ok", name: "ok", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: { entity: "User", values: { name: "{{ctx.name}}" } },
  },
  {
    type: "http.request",
    category: "HTTP",
    label: "HTTP Request",
    description: "Call an HTTP endpoint.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "ok", name: "ok", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: {
      method: "GET",
      url: "https://api.example.com",
      headers: {},
      body: "",
      timeoutMs: 10000,
    },
  },
  {
    type: "web3.read",
    category: "WEB3",
    label: "Web3 Read",
    description: "Read from chain (RPC call).",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "out", name: "out", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: { chain: "cerulea", method: "getBalance", params: ["{{ctx.address}}"] },
  },
  {
    type: "web3.write",
    category: "WEB3",
    label: "Web3 Write",
    description: "Submit tx to chain.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "ok", name: "ok", kind: "out" },
      { id: "error", name: "error", kind: "out" },
    ],
    defaultProps: { chain: "cerulea", method: "transfer", params: ["{{ctx.to}}", "{{ctx.amount}}"] },
  },
  {
    type: "util.log",
    category: "UTIL",
    label: "Log",
    description: "Log a message.",
    ports: [
      { id: "in", name: "in", kind: "in" },
      { id: "out", name: "out", kind: "out" },
    ],
    defaultProps: { message: "ctx={{ctx}}" },
  },
];

function BlockNode({ data }: { data: BlockNodeData }) {
  const inPorts = data.ports.filter((p) => p.kind === "in");
  const outPorts = data.ports.filter((p) => p.kind === "out");

  return (
    <Box
      sx={{
        minWidth: 240,
        borderRadius: 999,
        border: "1px solid rgba(255,255,255,0.16)",
        bgcolor: "rgba(10,12,18,0.55)",
        backdropFilter: "blur(14px)",
        boxShadow: "0 18px 55px rgba(0,0,0,0.35)",
        px: 2.2,
        py: 1.6,
        position: "relative",
      }}
    >
      {inPorts.map((p, idx) => (
        <Handle
          key={p.id}
          type="target"
          id={p.id}
          position={Position.Left}
          style={{
            top: 38 + idx * 18,
            width: 10,
            height: 10,
            borderRadius: 999,
            border: "2px solid rgba(255,255,255,0.55)",
          }}
        />
      ))}

      {outPorts.map((p, idx) => (
        <Handle
          key={p.id}
          type="source"
          id={p.id}
          position={Position.Right}
          style={{
            top: 38 + idx * 18,
            width: 10,
            height: 10,
            borderRadius: 999,
            border: "2px solid rgba(59,130,246,0.75)",
          }}
        />
      ))}

      <Typography sx={{ fontWeight: 900, fontSize: 14, lineHeight: 1.1 }}>
        {data.label}
      </Typography>
      <Typography sx={{ opacity: 0.75, fontSize: 12, mt: 0.4 }}>
        {data.blockType}
      </Typography>
    </Box>
  );
}

const nodeTypes = { block: BlockNode };

type PropertiesDrawerProps = {
  open: boolean;
  onClose: () => void;
  selectedNode: Node<BlockNodeData> | null;
  selectedEdge: Edge | null;
  onUpdateEdge: (edgeId: string, patch: Partial<Edge>) => void;
};

function PropertiesDrawer({ open, onClose, selectedNode, selectedEdge, onUpdateEdge }: PropertiesDrawerProps) {
  const [edgeLabel, setEdgeLabel] = useState<string>("");
  const [edgeCondition, setEdgeCondition] = useState<string>("");
  const [edgeRetries, setEdgeRetries] = useState<number>(0);
  const [edgeTimeoutMs, setEdgeTimeoutMs] = useState<number>(0);
  const [edgeOnError, setEdgeOnError] = useState<string>("halt");
  const [edgeNotes, setEdgeNotes] = useState<string>("");

  const [nodeLabel, setNodeLabel] = useState<string>("");
  const [nodePropsJson, setNodePropsJson] = useState<string>("{}");

  useEffect(() => {
    if (selectedEdge) {
      setEdgeLabel((selectedEdge.label as string) || "");
      const d: any = (selectedEdge as any).data || {};
      setEdgeCondition(d.condition || "");
      setEdgeRetries(typeof d.retries === "number" ? d.retries : 0);
      setEdgeTimeoutMs(typeof d.timeoutMs === "number" ? d.timeoutMs : 0);
      setEdgeOnError(d.onError || "halt");
      setEdgeNotes(d.notes || "");
    }
  }, [selectedEdge]);

  useEffect(() => {
    if (selectedNode) {
      setNodeLabel(selectedNode.data?.label || "");
      setNodePropsJson(JSON.stringify(selectedNode.data?.props || {}, null, 2));
    }
  }, [selectedNode]);

  if (!open) return null;

  return (
    <Box
      sx={{
        position: "fixed",
        right: 16,
        top: 96,
        width: 380,
        maxHeight: "calc(100vh - 120px)",
        overflowY: "auto",
        borderRadius: 5,
        border: "1px solid rgba(255,255,255,0.12)",
        bgcolor: "rgba(10,12,18,0.55)",
        backdropFilter: "blur(18px)",
        boxShadow: "0 30px 85px rgba(0,0,0,0.55)",
        p: 2,
        zIndex: 1300,
      }}
    >
      <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 1 }}>
        <Typography sx={{ fontWeight: 900 }}>Properties</Typography>
        <IconButton
          onClick={onClose}
          sx={{ border: "1px solid rgba(255,255,255,0.10)", bgcolor: "rgba(255,255,255,0.04)" }}
        >
          <CloseIcon fontSize="small" />
        </IconButton>
      </Stack>

      <Divider sx={{ opacity: 0.12, mb: 2 }} />

      {selectedEdge && (
        <Stack spacing={1.25} sx={{ mb: 2 }}>
          <Typography sx={{ fontWeight: 800 }}>Edge</Typography>

          <TextField
            size="small"
            label="Edge label"
            value={edgeLabel}
            onChange={(e) => setEdgeLabel(e.target.value)}
            onBlur={() => {
              if (!selectedEdge) return;
              onUpdateEdge(selectedEdge.id, {
                label: edgeLabel,
                data: {
                  ...(selectedEdge as any).data,
                  condition: edgeCondition,
                  retries: edgeRetries,
                  timeoutMs: edgeTimeoutMs,
                  onError: edgeOnError,
                  notes: edgeNotes,
                },
              });
            }}
            sx={{ "& .MuiInputBase-root": { borderRadius: 999 } }}
          />

          <TextField
            size="small"
            label="Condition (optional)"
            placeholder="e.g. ctx.ok === true"
            value={edgeCondition}
            onChange={(e) => setEdgeCondition(e.target.value)}
            onBlur={() => {
              if (!selectedEdge) return;
              onUpdateEdge(selectedEdge.id, {
                data: {
                  ...(selectedEdge as any).data,
                  condition: edgeCondition,
                  retries: edgeRetries,
                  timeoutMs: edgeTimeoutMs,
                  onError: edgeOnError,
                  notes: edgeNotes,
                },
              });
            }}
            sx={{ "& .MuiInputBase-root": { borderRadius: 999 } }}
          />

          <Stack direction="row" spacing={1}>
            <TextField
              size="small"
              type="number"
              label="Retries"
              value={edgeRetries}
              onChange={(e) => setEdgeRetries(Number(e.target.value || 0))}
              onBlur={() => {
                if (!selectedEdge) return;
                onUpdateEdge(selectedEdge.id, {
                  data: {
                    ...(selectedEdge as any).data,
                    condition: edgeCondition,
                    retries: edgeRetries,
                    timeoutMs: edgeTimeoutMs,
                    onError: edgeOnError,
                    notes: edgeNotes,
                  },
                });
              }}
              sx={{ flex: 1, "& .MuiInputBase-root": { borderRadius: 999 } }}
            />
            <TextField
              size="small"
              type="number"
              label="Timeout (ms)"
              value={edgeTimeoutMs}
              onChange={(e) => setEdgeTimeoutMs(Number(e.target.value || 0))}
              onBlur={() => {
                if (!selectedEdge) return;
                onUpdateEdge(selectedEdge.id, {
                  data: {
                    ...(selectedEdge as any).data,
                    condition: edgeCondition,
                    retries: edgeRetries,
                    timeoutMs: edgeTimeoutMs,
                    onError: edgeOnError,
                    notes: edgeNotes,
                  },
                });
              }}
              sx={{ flex: 1, "& .MuiInputBase-root": { borderRadius: 999 } }}
            />
          </Stack>

          <TextField
            size="small"
            label="On error"
            value={edgeOnError}
            onChange={(e) => setEdgeOnError(e.target.value)}
            onBlur={() => {
              if (!selectedEdge) return;
              onUpdateEdge(selectedEdge.id, {
                data: {
                  ...(selectedEdge as any).data,
                  condition: edgeCondition,
                  retries: edgeRetries,
                  timeoutMs: edgeTimeoutMs,
                  onError: edgeOnError,
                  notes: edgeNotes,
                },
              });
            }}
            select
            SelectProps={{ native: true }}
            sx={{ "& .MuiInputBase-root": { borderRadius: 999 } }}
          >
            <option value="halt">halt</option>
            <option value="retry">retry</option>
            <option value="fallback">fallback</option>
          </TextField>

          <TextField
            size="small"
            label="Notes"
            value={edgeNotes}
            onChange={(e) => setEdgeNotes(e.target.value)}
            onBlur={() => {
              if (!selectedEdge) return;
              onUpdateEdge(selectedEdge.id, {
                data: {
                  ...(selectedEdge as any).data,
                  condition: edgeCondition,
                  retries: edgeRetries,
                  timeoutMs: edgeTimeoutMs,
                  onError: edgeOnError,
                  notes: edgeNotes,
                },
              });
            }}
            multiline
            minRows={2}
            sx={{ "& .MuiInputBase-root": { borderRadius: 18 } }}
          />
        </Stack>
      )}

      {selectedNode && (
        <Stack spacing={1.25}>
          <Typography sx={{ fontWeight: 800 }}>Block</Typography>

          <TextField
            size="small"
            label="Block label"
            value={nodeLabel}
            onChange={(e) => setNodeLabel(e.target.value)}
            onBlur={() => {
              if (!selectedNode) return;
              (selectedNode.data as any).label = nodeLabel;
            }}
            sx={{ "& .MuiInputBase-root": { borderRadius: 999 } }}
          />

          <TextField
            size="small"
            label="Block properties (JSON)"
            value={nodePropsJson}
            onChange={(e) => setNodePropsJson(e.target.value)}
            onBlur={() => {
              if (!selectedNode) return;
              try {
                const parsed = JSON.parse(nodePropsJson || "{}");
                (selectedNode.data as any).props = parsed;
              } catch {
                // ignore invalid JSON (user can fix)
              }
            }}
            multiline
            minRows={8}
            sx={{ "& .MuiInputBase-root": { borderRadius: 18 } }}
          />
        </Stack>
      )}

      {!selectedNode && !selectedEdge && (
        <Typography sx={{ opacity: 0.75 }}>Click a block or edge to edit properties.</Typography>
      )}
    </Box>
  );
}

function LogicCanvasInner() {
  const [track] = useState<"dapp" | "blockchain">("dapp");
  const [search, setSearch] = useState("");

  const palette = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return BLOCKS;
    return BLOCKS.filter(
      (b) =>
        b.label.toLowerCase().includes(q) ||
        b.type.toLowerCase().includes(q) ||
        b.category.toLowerCase().includes(q)
    );
  }, [search]);

  const initialNodes = useMemo<Node<BlockNodeData>[]>(() => [], []);
  const initialEdges = useMemo<Edge[]>(() => [], []);

  const [nodes, setNodes, onNodesChange] = useNodesState<BlockNodeData>(initialNodes);
  const [edges, setEdges, onEdgesChange] = useEdgesState(initialEdges);

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [selectedNode, setSelectedNode] = useState<Node<BlockNodeData> | null>(null);
  const [selectedEdge, setSelectedEdge] = useState<Edge | null>(null);

  const hydratedRef = useRef(false);
  useEffect(() => {
    if (hydratedRef.current) return;
    hydratedRef.current = true;
  }, []);

  const addBlock = useCallback(
    (def: BlockDef) => {
      const n: Node<BlockNodeData> = {
        id: uid("blk"),
        type: "block",
        position: { x: 300 + Math.random() * 300, y: 140 + Math.random() * 260 },
        data: {
          blockType: def.type,
          label: def.label,
          props: { ...def.defaultProps },
          ports: def.ports,
        },
      };
      setNodes((prev) => [...prev, n]);
    },
    [setNodes]
  );

  const clearAll = useCallback(() => {
    setNodes([]);
    setEdges([]);
    setSelectedEdge(null);
    setSelectedNode(null);
    setDrawerOpen(false);
  }, [setNodes, setEdges]);

  const onConnect = useCallback(
    (c: Connection) => {
      if (!c.source || !c.target) return;

      const next = addEdge(
        {
          ...c,
          animated: false,
          type: "smoothstep",
          label: "next",
          data: {
            condition: "",
            mapping: {},
            retries: 0,
            timeoutMs: 0,
            onError: "halt",
            notes: "",
          },
        },
        edges
      );
      setEdges(next);
    },
    [edges, setEdges]
  );

  const onSelectionChange = useCallback(
    (sel: { nodes: Node[]; edges: Edge[] }) => {
      const n = (sel.nodes?.[0] as Node<BlockNodeData> | undefined) ?? null;
      const e = (sel.edges?.[0] as Edge | undefined) ?? null;

      setSelectedNode(n);
      setSelectedEdge(e);
      setDrawerOpen(Boolean(n || e));
    },
    []
  );

  return (
    <Stack spacing={2}>
      <Stack direction="row" justifyContent="space-between" alignItems="center">
        <Typography sx={{ fontWeight: 900 }}>Logic & Actions</Typography>
        <Chip
          label={track.toUpperCase()}
          size="small"
          sx={{
            fontWeight: 900,
            bgcolor: "rgba(59,130,246,0.18)",
            border: "1px solid rgba(59,130,246,0.28)",
          }}
        />
      </Stack>

      <Stack direction="row" spacing={2} alignItems="flex-start">
        {/* Palette */}
        <Box
          sx={{
            width: 360,
            borderRadius: 5,
            overflow: "hidden",
            border: "1px solid rgba(255,255,255,0.12)",
            bgcolor: "rgba(10,12,18,0.38)",
            backdropFilter: "blur(18px)",
          }}
        >
          <Stack sx={{ px: 2.5, py: 2 }} spacing={1.2}>
            <Typography sx={{ fontWeight: 900, pl: 0.5 }}>Blocks</Typography>
            <TextField
              size="small"
              placeholder="Search blocks..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              sx={{ "& .MuiInputBase-root": { borderRadius: 999 } }}
            />
            <Stack direction="row" spacing={1}>
              <Button
                size="small"
                onClick={clearAll}
                startIcon={<DeleteIcon />}
                sx={{
                  borderRadius: 999,
                  border: "1px solid rgba(255,255,255,0.12)",
                  bgcolor: "rgba(255,255,255,0.04)",
                  fontWeight: 900,
                }}
              >
                Clear All
              </Button>
            </Stack>
          </Stack>

          <Divider sx={{ opacity: 0.12 }} />

          <Box sx={{ maxHeight: "calc(100vh - 360px)", overflowY: "auto", px: 2, py: 2 }}>
            <Stack spacing={1.25}>
              {palette.map((b) => (
                <Box
                  key={b.type}
                  onClick={() => addBlock(b)}
                  sx={{
                    cursor: "pointer",
                    borderRadius: 4,
                    border: "1px solid rgba(255,255,255,0.12)",
                    bgcolor: "rgba(255,255,255,0.04)",
                    px: 2,
                    py: 1.4,
                  }}
                >
                  <Stack direction="row" justifyContent="space-between" alignItems="center">
                    <Typography sx={{ fontWeight: 900 }}>{b.label}</Typography>
                    <IconButton
                      size="small"
                      sx={{
                        border: "1px solid rgba(59,130,246,0.28)",
                        bgcolor: "rgba(59,130,246,0.12)",
                      }}
                    >
                      <AddIcon fontSize="small" />
                    </IconButton>
                  </Stack>
                  <Typography sx={{ opacity: 0.7, fontSize: 12 }}>{b.category}</Typography>
                </Box>
              ))}
            </Stack>
          </Box>
        </Box>

        {/* Canvas */}
        <Box sx={{ flex: 1 }}>
          <Box sx={CANVAS_STYLE}>
            <ReactFlow
              nodes={nodes}
              edges={edges}
              nodeTypes={nodeTypes}
              onNodesChange={onNodesChange}
              onEdgesChange={onEdgesChange}
              onConnect={onConnect}
              onSelectionChange={onSelectionChange}
              fitView
              fitViewOptions={{ padding: 0.2 }}
            >
              <Background gap={20} size={1} />
              <MiniMap />
              <Controls />
            </ReactFlow>
          </Box>

          <PropertiesDrawer
            open={drawerOpen}
            onClose={() => setDrawerOpen(false)}
            selectedNode={selectedNode}
            selectedEdge={selectedEdge}
            onUpdateEdge={(edgeId, patch) =>
              setEdges((eds) => eds.map((e) => (e.id === edgeId ? { ...e, ...patch } : e)))
            }
          />
        </Box>
      </Stack>
    </Stack>
  );
}

export default function LogicCanvas() {
  return (
    <ReactFlowProvider>
      <LogicCanvasInner />
    </ReactFlowProvider>
  );
}
