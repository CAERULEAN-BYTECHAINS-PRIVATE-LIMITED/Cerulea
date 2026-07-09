"use client";

import * as React from "react";
import {
  Box,
  Divider,
  FormControl,
  FormControlLabel,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  Switch,
  TextField,
  Typography,
  Accordion,
  AccordionSummary,
  AccordionDetails,
  Chip,
  Button,
} from "@mui/material";

import ExpandMoreIcon from "@mui/icons-material/ExpandMore";

import type { Edge, Node } from "reactflow";

type Port = { id: string; label: string };

type BlockNodeData = {
  kind: "block";
  blockId: string;
  label: string;
  group: string;
  description: string;
  params: Record<string, any>;
  ports: { in: Port[]; out: Port[] };
};

type EdgeData = {
  kind: "flow";
  label: string;

  condition: {
    type: "always" | "expression" | "success" | "failure";
    expression?: string;
  };

  // Execution semantics for the transition
  transition?: {
    mode: "sync" | "async";
    priority: number; // higher wins when multiple outgoing edges are eligible
    debounceMs: number; // prevent rapid re-fire
    throttlePerSec: number; // cap rate
  };

  // Reliability behavior on this edge (can override node-level behavior)
  retry?: {
    enabled: boolean;
    maxAttempts: number;
    backoffMs: number;
    jitter: boolean;
    retryOn: Array<"timeout" | "network" | "5xx" | "4xx" | "exception">;
  };

  timeoutMs?: number;

  // Data passing / mapping between nodes
  dataFlow?: {
    passContext: boolean;
    inputPath: string;
    outputPath: string;
    mappingJson: string; // JSON string for deterministic editing
  };

  // Observability
  telemetry?: {
    enabled: boolean;
    spanName: string;
    logLevel: "debug" | "info" | "warn" | "error";
    tagsCsv: string;
  };

  metadata?: Record<string, any>;
};

function safeJsonStringify(v: any, fallback = "{}") {
  try {
    return JSON.stringify(v ?? {}, null, 2);
  } catch {
    return fallback;
  }
}

function safeJsonParse(s: string, fallback: any = {}) {
  try {
    return JSON.parse(s);
  } catch {
    return fallback;
  }
}

function ensureEdgeDefaults(d?: EdgeData): EdgeData {
  const base: EdgeData = {
    kind: "flow",
    label: "",
    condition: { type: "always" },

    transition: {
      mode: "sync",
      priority: 0,
      debounceMs: 0,
      throttlePerSec: 0,
    },

    retry: {
      enabled: false,
      maxAttempts: 0,
      backoffMs: 200,
      jitter: true,
      retryOn: ["timeout", "network", "5xx", "exception"],
    },

    timeoutMs: 15000,

    dataFlow: {
      passContext: true,
      inputPath: "",
      outputPath: "",
      mappingJson: "{}",
    },

    telemetry: {
      enabled: true,
      spanName: "",
      logLevel: "info",
      tagsCsv: "",
    },

    metadata: {},
  };

  const merged = { ...base, ...(d || {}) } as EdgeData;

  merged.condition = { ...base.condition, ...(d?.condition || {}) };

  merged.transition = { ...base.transition, ...(d?.transition || {}) } as typeof base.transition;

  merged.retry = { ...base.retry, ...(d?.retry || {}) } as typeof base.retry;
  merged.retry!.retryOn = Array.isArray(d?.retry?.retryOn) ? d!.retry!.retryOn : base.retry!.retryOn;

  merged.dataFlow = { ...base.dataFlow, ...(d?.dataFlow || {}) } as typeof base.dataFlow;
  merged.telemetry = { ...base.telemetry, ...(d?.telemetry || {}) } as typeof base.telemetry;

  merged.timeoutMs = typeof d?.timeoutMs === "number" ? d!.timeoutMs : base.timeoutMs;

  merged.metadata = (d?.metadata && typeof d.metadata === "object") ? d.metadata : base.metadata;

  // ensure mappingJson is a string
  if (typeof merged.dataFlow!.mappingJson !== "string") {
    merged.dataFlow!.mappingJson = safeJsonStringify(merged.dataFlow!.mappingJson);
  }

  return merged;
}

type Props =
  | {
      mode: "block";
      node: Node<BlockNodeData>;
      onChangeParams: (params: Record<string, any>) => void;
    }
  | {
      mode: "edge";
      edge: Edge<EdgeData>;
      onChangeEdge: (data: EdgeData) => void;
    };

export default function PropertiesDrawer(props: Props) {
  if (props.mode === "block") {
    return <BlockProperties node={props.node} onChangeParams={props.onChangeParams} />;
  }
  return <EdgeProperties edge={props.edge} onChangeEdge={props.onChangeEdge} />;
}

/* ----------------------------- BLOCK PROPS ----------------------------- */

function BlockProperties({
  node,
  onChangeParams,
}: {
  node: Node<BlockNodeData>;
  onChangeParams: (params: Record<string, any>) => void;
}) {
  // We keep node's fixed "label/group/description" informational here,
  // and expose a large, consistent config model inside params.
  const params = React.useMemo(() => {
    const p = node.data.params || {};
    // Seed consistent structure if missing
    return {
      enabled: p.enabled ?? true,

      // How/when this block runs
      run: {
        when: p.run?.when ?? "always", // always | expression | onSuccess | onFailure
        expression: p.run?.expression ?? "",
      },

      // Error handling at node level
      error: {
        strategy: p.error?.strategy ?? "fail", // fail | continue | route
        routeLabel: p.error?.routeLabel ?? "", // if strategy=route
        includeErrorInContext: p.error?.includeErrorInContext ?? true,
      },

      // Retry/timeout at node level (some edges may override)
      retry: {
        enabled: p.retry?.enabled ?? false,
        maxAttempts: p.retry?.maxAttempts ?? 0,
        backoffMs: p.retry?.backoffMs ?? 200,
        jitter: p.retry?.jitter ?? true,
      },
      timeoutMs: p.timeoutMs ?? 15000,

      // Data IO
      io: {
        inputPath: p.io?.inputPath ?? "",
        outputPath: p.io?.outputPath ?? "",
        // optional deterministic mapping JSON
        mappingJson: typeof p.io?.mappingJson === "string" ? p.io.mappingJson : safeJsonStringify(p.io?.mappingJson ?? {}),
      },

      // Observability
      telemetry: {
        enabled: p.telemetry?.enabled ?? true,
        logLevel: p.telemetry?.logLevel ?? "info",
        tagsCsv: p.telemetry?.tagsCsv ?? "",
        note: p.telemetry?.note ?? "",
      },

      // Security / permissions (generic; used by codegen later)
      security: {
        requireAuth: p.security?.requireAuth ?? false,
        rolesCsv: p.security?.rolesCsv ?? "",
        permissionsCsv: p.security?.permissionsCsv ?? "",
        rateLimitPerMin: p.security?.rateLimitPerMin ?? 0,
      },

      // Environment / execution context
      runtime: {
        env: p.runtime?.env ?? "dev", // dev|staging|prod
        worker: p.runtime?.worker ?? "default",
        concurrency: p.runtime?.concurrency ?? 1,
      },

      // Freeform metadata
      metaJson: typeof p.metaJson === "string" ? p.metaJson : safeJsonStringify(p.metaJson ?? {}),
      ...p, // keep any module-specific fields that were already there
    };
  }, [node.data.params]);

  const [draft, setDraft] = React.useState(params);

  React.useEffect(() => {
    setDraft(params);
  }, [params]);

  const push = (next: any) => {
    setDraft(next);
    // ensure mappingJson/metaJson remain strings
    const normalized = {
      ...next,
      io: { ...next.io, mappingJson: next.io?.mappingJson ?? "{}" },
      metaJson: next.metaJson ?? "{}",
    };
    onChangeParams(normalized);
  };

  return (
    <Box>
      {/* Header info */}
      <Stack spacing={0.6} sx={{ mb: 2 }}>
        <Typography sx={{ fontWeight: 900 }}>{node.data.label}</Typography>
        <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap" }}>
          <Chip size="small" label={node.data.group} sx={{ opacity: 0.9 }} />
          <Chip size="small" label={node.data.blockId} sx={{ opacity: 0.7 }} />
        </Stack>
        <Typography sx={{ fontSize: 13, color: "rgba(255,255,255,0.75)" }}>
          {node.data.description}
        </Typography>
      </Stack>

      <Divider sx={{ borderColor: "rgba(255,255,255,0.10)", mb: 2 }} />

      {/* Main sections */}
      <Accordion defaultExpanded>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Basics</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.4}>
            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.enabled}
                  onChange={(e) => push({ ...draft, enabled: e.target.checked })}
                />
              }
              label="Enabled"
            />

            <FormControl fullWidth size="small">
              <InputLabel>Run when</InputLabel>
              <Select
                label="Run when"
                value={draft.run?.when ?? "always"}
                onChange={(e) => push({ ...draft, run: { ...draft.run, when: String(e.target.value) } })}
              >
                <MenuItem value="always">Always</MenuItem>
                <MenuItem value="expression">When expression is true</MenuItem>
                <MenuItem value="onSuccess">Only if previous step succeeded</MenuItem>
                <MenuItem value="onFailure">Only if previous step failed</MenuItem>
              </Select>
            </FormControl>

            <TextField
              size="small"
              label="Run expression (if applicable)"
              placeholder="Example: context.user.isPremium === true"
              value={draft.run?.expression ?? ""}
              onChange={(e) => push({ ...draft, run: { ...draft.run, expression: e.target.value } })}
              helperText="Used only when Run when = expression."
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Error handling</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.4}>
            <FormControl fullWidth size="small">
              <InputLabel>Strategy</InputLabel>
              <Select
                label="Strategy"
                value={draft.error?.strategy ?? "fail"}
                onChange={(e) => push({ ...draft, error: { ...draft.error, strategy: String(e.target.value) } })}
              >
                <MenuItem value="fail">Fail the flow</MenuItem>
                <MenuItem value="continue">Continue anyway</MenuItem>
                <MenuItem value="route">Route to a specific edge label</MenuItem>
              </Select>
            </FormControl>

            <TextField
              size="small"
              label="Route edge label (if strategy = route)"
              value={draft.error?.routeLabel ?? ""}
              onChange={(e) => push({ ...draft, error: { ...draft.error, routeLabel: e.target.value } })}
              helperText="If set, the engine will look for an outgoing edge with this label when an error occurs."
            />

            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.error?.includeErrorInContext}
                  onChange={(e) =>
                    push({ ...draft, error: { ...draft.error, includeErrorInContext: e.target.checked } })
                  }
                />
              }
              label="Attach error object to context"
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Reliability</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.4}>
            <TextField
              size="small"
              label="Timeout (ms)"
              type="number"
              value={draft.timeoutMs ?? 15000}
              onChange={(e) => push({ ...draft, timeoutMs: Number(e.target.value || 0) })}
              helperText="Max time this block is allowed to run before it is considered timed out."
            />

            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.retry?.enabled}
                  onChange={(e) => push({ ...draft, retry: { ...draft.retry, enabled: e.target.checked } })}
                />
              }
              label="Enable retries"
            />

            <Stack direction="row" spacing={1}>
              <TextField
                size="small"
                label="Max attempts"
                type="number"
                value={draft.retry?.maxAttempts ?? 0}
                onChange={(e) =>
                  push({ ...draft, retry: { ...draft.retry, maxAttempts: Number(e.target.value || 0) } })
                }
                sx={{ flex: 1 }}
              />
              <TextField
                size="small"
                label="Backoff (ms)"
                type="number"
                value={draft.retry?.backoffMs ?? 200}
                onChange={(e) =>
                  push({ ...draft, retry: { ...draft.retry, backoffMs: Number(e.target.value || 0) } })
                }
                sx={{ flex: 1 }}
              />
            </Stack>

            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.retry?.jitter}
                  onChange={(e) => push({ ...draft, retry: { ...draft.retry, jitter: e.target.checked } })}
                />
              }
              label="Jitter backoff"
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Data input/output</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <TextField
              size="small"
              label="Input path (context)"
              placeholder="Example: request.body"
              value={draft.io?.inputPath ?? ""}
              onChange={(e) => push({ ...draft, io: { ...draft.io, inputPath: e.target.value } })}
              helperText="Where this block should read inputs from the context."
            />
            <TextField
              size="small"
              label="Output path (context)"
              placeholder="Example: order.created"
              value={draft.io?.outputPath ?? ""}
              onChange={(e) => push({ ...draft, io: { ...draft.io, outputPath: e.target.value } })}
              helperText="Where this block should write results in the context."
            />

            <TextField
              size="small"
              label="Mapping JSON"
              multiline
              minRows={6}
              value={draft.io?.mappingJson ?? "{}"}
              onChange={(e) => push({ ...draft, io: { ...draft.io, mappingJson: e.target.value } })}
              helperText="Optional deterministic transform mapping. Stored as JSON text for stability."
              sx={{
                "& .MuiOutlinedInput-root": {
                  fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
                  fontSize: 12.5,
                },
              }}
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Telemetry</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.telemetry?.enabled}
                  onChange={(e) => push({ ...draft, telemetry: { ...draft.telemetry, enabled: e.target.checked } })}
                />
              }
              label="Telemetry enabled"
            />
            <FormControl fullWidth size="small">
              <InputLabel>Log level</InputLabel>
              <Select
                label="Log level"
                value={draft.telemetry?.logLevel ?? "info"}
                onChange={(e) =>
                  push({ ...draft, telemetry: { ...draft.telemetry, logLevel: String(e.target.value) } })
                }
              >
                <MenuItem value="debug">Debug</MenuItem>
                <MenuItem value="info">Info</MenuItem>
                <MenuItem value="warn">Warn</MenuItem>
                <MenuItem value="error">Error</MenuItem>
              </Select>
            </FormControl>
            <TextField
              size="small"
              label="Tags (CSV)"
              placeholder="billing,critical,external"
              value={draft.telemetry?.tagsCsv ?? ""}
              onChange={(e) => push({ ...draft, telemetry: { ...draft.telemetry, tagsCsv: e.target.value } })}
            />
            <TextField
              size="small"
              label="Note"
              value={draft.telemetry?.note ?? ""}
              onChange={(e) => push({ ...draft, telemetry: { ...draft.telemetry, note: e.target.value } })}
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Security & Limits</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.security?.requireAuth}
                  onChange={(e) =>
                    push({ ...draft, security: { ...draft.security, requireAuth: e.target.checked } })
                  }
                />
              }
              label="Require authentication"
            />
            <TextField
              size="small"
              label="Roles (CSV)"
              placeholder="admin,ops"
              value={draft.security?.rolesCsv ?? ""}
              onChange={(e) => push({ ...draft, security: { ...draft.security, rolesCsv: e.target.value } })}
            />
            <TextField
              size="small"
              label="Permissions (CSV)"
              placeholder="orders:write,payments:refund"
              value={draft.security?.permissionsCsv ?? ""}
              onChange={(e) =>
                push({ ...draft, security: { ...draft.security, permissionsCsv: e.target.value } })
              }
            />
            <TextField
              size="small"
              type="number"
              label="Rate limit (per minute)"
              value={draft.security?.rateLimitPerMin ?? 0}
              onChange={(e) =>
                push({ ...draft, security: { ...draft.security, rateLimitPerMin: Number(e.target.value || 0) } })
              }
              helperText="Generic limiter (useful during deploy/runtime). 0 = unlimited."
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Runtime</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControl fullWidth size="small">
              <InputLabel>Environment</InputLabel>
              <Select
                label="Environment"
                value={draft.runtime?.env ?? "dev"}
                onChange={(e) => push({ ...draft, runtime: { ...draft.runtime, env: String(e.target.value) } })}
              >
                <MenuItem value="dev">Dev</MenuItem>
                <MenuItem value="staging">Staging</MenuItem>
                <MenuItem value="prod">Prod</MenuItem>
              </Select>
            </FormControl>
            <TextField
              size="small"
              label="Worker queue"
              placeholder="default"
              value={draft.runtime?.worker ?? "default"}
              onChange={(e) => push({ ...draft, runtime: { ...draft.runtime, worker: e.target.value } })}
            />
            <TextField
              size="small"
              label="Concurrency"
              type="number"
              value={draft.runtime?.concurrency ?? 1}
              onChange={(e) =>
                push({ ...draft, runtime: { ...draft.runtime, concurrency: Number(e.target.value || 1) } })
              }
              helperText="How many instances of this block can run concurrently."
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Metadata (JSON)</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <TextField
              size="small"
              label="Meta JSON"
              multiline
              minRows={6}
              value={draft.metaJson ?? "{}"}
              onChange={(e) => push({ ...draft, metaJson: e.target.value })}
              helperText="Freeform metadata for future exporters/codegen."
              sx={{
                "& .MuiOutlinedInput-root": {
                  fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
                  fontSize: 12.5,
                },
              }}
            />
          </Stack>
        </AccordionDetails>
      </Accordion>
    </Box>
  );
}

/* ------------------------------ EDGE PROPS ------------------------------ */

function EdgeProperties({
  edge,
  onChangeEdge,
}: {
  edge: Edge<EdgeData>;
  onChangeEdge: (data: EdgeData) => void;
}) {
  const data = ensureEdgeDefaults(edge.data);

  const [draft, setDraft] = React.useState<EdgeData>(() => data);

  React.useEffect(() => {
    setDraft(data);
  }, [edge.id]); // reload when selecting a different edge

  const push = (next: EdgeData) => {
    setDraft(next);
    // normalize mappingJson to string
    if (next.dataFlow && typeof next.dataFlow.mappingJson !== "string") {
      next = {
        ...next,
        dataFlow: { ...next.dataFlow, mappingJson: safeJsonStringify(next.dataFlow.mappingJson) },
      };
    }
    onChangeEdge(next);
  };

  const retryOn = draft.retry?.retryOn ?? [];
  const toggleRetryOn = (k: any) => {
    const next = retryOn.includes(k) ? retryOn.filter((x) => x !== k) : [...retryOn, k];
    push({ ...draft, retry: { ...draft.retry!, retryOn: next } });
  };

  return (
    <Box>
      <Stack spacing={0.6} sx={{ mb: 2 }}>
        <Typography sx={{ fontWeight: 900 }}>Edge</Typography>
        <Typography sx={{ fontSize: 13, color: "rgba(255,255,255,0.75)" }}>
          Configure when this transition fires and how it behaves (retries, timeouts, data passing).
        </Typography>
      </Stack>

      <Divider sx={{ borderColor: "rgba(255,255,255,0.10)", mb: 2 }} />

      <Accordion defaultExpanded>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Basics</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <TextField
              size="small"
              label="Edge label"
              placeholder="Example: success / failure / approved"
              value={draft.label ?? ""}
              onChange={(e) => push({ ...draft, label: e.target.value })}
              helperText="Used by the runtime for routing + by humans for readability."
            />

            <FormControl fullWidth size="small">
              <InputLabel>Condition type</InputLabel>
              <Select
                label="Condition type"
                value={draft.condition?.type ?? "always"}
                onChange={(e) =>
                  push({ ...draft, condition: { ...draft.condition, type: e.target.value as any } })
                }
              >
                <MenuItem value="always">Always</MenuItem>
                <MenuItem value="expression">Expression</MenuItem>
                <MenuItem value="success">Only on success</MenuItem>
                <MenuItem value="failure">Only on failure</MenuItem>
              </Select>
            </FormControl>

            <TextField
              size="small"
              label="Condition expression"
              placeholder="Example: context.order.total > 1000"
              value={draft.condition?.expression ?? ""}
              onChange={(e) =>
                push({ ...draft, condition: { ...draft.condition, expression: e.target.value } })
              }
              helperText="Used only when Condition type = Expression."
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Transition behavior</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControl fullWidth size="small">
              <InputLabel>Mode</InputLabel>
              <Select
                label="Mode"
                value={draft.transition?.mode ?? "sync"}
                onChange={(e) =>
                  push({ ...draft, transition: { ...draft.transition!, mode: e.target.value as any } })
                }
              >
                <MenuItem value="sync">Sync</MenuItem>
                <MenuItem value="async">Async</MenuItem>
              </Select>
            </FormControl>

            <TextField
              size="small"
              type="number"
              label="Priority"
              value={draft.transition?.priority ?? 0}
              onChange={(e) =>
                push({
                  ...draft,
                  transition: { ...draft.transition!, priority: Number(e.target.value || 0) },
                })
              }
              helperText="When multiple edges are eligible, higher priority wins."
            />

            <Stack direction="row" spacing={1}>
              <TextField
                size="small"
                type="number"
                label="Debounce (ms)"
                value={draft.transition?.debounceMs ?? 0}
                onChange={(e) =>
                  push({
                    ...draft,
                    transition: { ...draft.transition!, debounceMs: Number(e.target.value || 0) },
                  })
                }
                sx={{ flex: 1 }}
                helperText="Prevent rapid re-fire."
              />
              <TextField
                size="small"
                type="number"
                label="Throttle (per sec)"
                value={draft.transition?.throttlePerSec ?? 0}
                onChange={(e) =>
                  push({
                    ...draft,
                    transition: { ...draft.transition!, throttlePerSec: Number(e.target.value || 0) },
                  })
                }
                sx={{ flex: 1 }}
                helperText="0 = unlimited."
              />
            </Stack>
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Timeout & Retries</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <TextField
              size="small"
              type="number"
              label="Timeout override (ms)"
              value={draft.timeoutMs ?? 15000}
              onChange={(e) => push({ ...draft, timeoutMs: Number(e.target.value || 0) })}
              helperText="Overrides node default for this edge transition if your runtime supports it."
            />

            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.retry?.enabled}
                  onChange={(e) => push({ ...draft, retry: { ...draft.retry!, enabled: e.target.checked } })}
                />
              }
              label="Enable retries on this edge"
            />

            <Stack direction="row" spacing={1}>
              <TextField
                size="small"
                type="number"
                label="Max attempts"
                value={draft.retry?.maxAttempts ?? 0}
                onChange={(e) =>
                  push({ ...draft, retry: { ...draft.retry!, maxAttempts: Number(e.target.value || 0) } })
                }
                sx={{ flex: 1 }}
              />
              <TextField
                size="small"
                type="number"
                label="Backoff (ms)"
                value={draft.retry?.backoffMs ?? 200}
                onChange={(e) =>
                  push({ ...draft, retry: { ...draft.retry!, backoffMs: Number(e.target.value || 0) } })
                }
                sx={{ flex: 1 }}
              />
            </Stack>

            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.retry?.jitter}
                  onChange={(e) => push({ ...draft, retry: { ...draft.retry!, jitter: e.target.checked } })}
                />
              }
              label="Jitter backoff"
            />

            <Typography sx={{ fontSize: 13, fontWeight: 700, opacity: 0.9, mt: 1 }}>
              Retry on
            </Typography>

            <Stack direction="row" spacing={1} sx={{ flexWrap: "wrap" }}>
              {(["timeout", "network", "5xx", "4xx", "exception"] as const).map((k) => (
                <Chip
                  key={k}
                  clickable
                  label={k}
                  color={retryOn.includes(k) ? "primary" : "default"}
                  onClick={() => toggleRetryOn(k)}
                  sx={{ mb: 0.6 }}
                />
              ))}
            </Stack>
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Data passing</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.dataFlow?.passContext}
                  onChange={(e) =>
                    push({ ...draft, dataFlow: { ...draft.dataFlow!, passContext: e.target.checked } })
                  }
                />
              }
              label="Pass context through this edge"
            />

            <TextField
              size="small"
              label="Input path (context)"
              value={draft.dataFlow?.inputPath ?? ""}
              onChange={(e) =>
                push({ ...draft, dataFlow: { ...draft.dataFlow!, inputPath: e.target.value } })
              }
              helperText="Optional: where to read from context for the next node."
            />
            <TextField
              size="small"
              label="Output path (context)"
              value={draft.dataFlow?.outputPath ?? ""}
              onChange={(e) =>
                push({ ...draft, dataFlow: { ...draft.dataFlow!, outputPath: e.target.value } })
              }
              helperText="Optional: where to write results for the next node."
            />

            <TextField
              size="small"
              label="Mapping JSON"
              multiline
              minRows={6}
              value={draft.dataFlow?.mappingJson ?? "{}"}
              onChange={(e) =>
                push({ ...draft, dataFlow: { ...draft.dataFlow!, mappingJson: e.target.value } })
              }
              helperText="Deterministic mapping for what this edge passes forward."
              sx={{
                "& .MuiOutlinedInput-root": {
                  fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
                  fontSize: 12.5,
                },
              }}
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Telemetry</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <FormControlLabel
              control={
                <Switch
                  checked={!!draft.telemetry?.enabled}
                  onChange={(e) =>
                    push({ ...draft, telemetry: { ...draft.telemetry!, enabled: e.target.checked } })
                  }
                />
              }
              label="Telemetry enabled"
            />

            <TextField
              size="small"
              label="Span name"
              value={draft.telemetry?.spanName ?? ""}
              onChange={(e) =>
                push({ ...draft, telemetry: { ...draft.telemetry!, spanName: e.target.value } })
              }
              helperText="Helpful when debugging complex flows."
            />

            <FormControl fullWidth size="small">
              <InputLabel>Log level</InputLabel>
              <Select
                label="Log level"
                value={draft.telemetry?.logLevel ?? "info"}
                onChange={(e) =>
                  push({ ...draft, telemetry: { ...draft.telemetry!, logLevel: e.target.value as any } })
                }
              >
                <MenuItem value="debug">Debug</MenuItem>
                <MenuItem value="info">Info</MenuItem>
                <MenuItem value="warn">Warn</MenuItem>
                <MenuItem value="error">Error</MenuItem>
              </Select>
            </FormControl>

            <TextField
              size="small"
              label="Tags (CSV)"
              value={draft.telemetry?.tagsCsv ?? ""}
              onChange={(e) =>
                push({ ...draft, telemetry: { ...draft.telemetry!, tagsCsv: e.target.value } })
              }
            />
          </Stack>
        </AccordionDetails>
      </Accordion>

      <Accordion>
        <AccordionSummary expandIcon={<ExpandMoreIcon />}>
          <Typography sx={{ fontWeight: 800 }}>Metadata</Typography>
        </AccordionSummary>
        <AccordionDetails>
          <Stack spacing={1.2}>
            <TextField
              size="small"
              label="Metadata JSON"
              multiline
              minRows={6}
              value={safeJsonStringify(draft.metadata ?? {})}
              onChange={(e) => push({ ...draft, metadata: safeJsonParse(e.target.value, {}) })}
              helperText="Freeform metadata for exporters/runtime."
              sx={{
                "& .MuiOutlinedInput-root": {
                  fontFamily: "ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace",
                  fontSize: 12.5,
                },
              }}
            />

            <Button
              variant="outlined"
              onClick={() =>
                push({
                  ...draft,
                  metadata: {},
                })
              }
            >
              Clear metadata
            </Button>
          </Stack>
        </AccordionDetails>
      </Accordion>
    </Box>
  );
}
