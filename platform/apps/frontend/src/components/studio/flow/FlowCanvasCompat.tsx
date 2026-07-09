"use client";

import React, { useEffect, useMemo, useRef, useState } from "react";

/**
 * FlowCanvasCompat
 * - Loads the "reactflow" package at runtime (client-only).
 * - Works with v10 (default export) and v11 (named ReactFlow export).
 * - Never renders undefined components (avoids "Element type is invalid" crash).
 */
export type FlowNode = {
  id: string;
  position: { x: number; y: number };
  data?: any;
  type?: string;
};
export type FlowEdge = {
  id: string;
  source: string;
  target: string;
  label?: string;
};

type Props = {
  nodes: FlowNode[];
  edges: FlowEdge[];
  onConnect?: (p: any) => void;
  onNodeDragStop?: (e: any, n: any) => void;
  onNodeDoubleClick?: (e: any, n: any) => void;
  style?: React.CSSProperties;
  fitView?: boolean;
};

function isRenderable(fn: any) {
  return typeof fn === "function";
}

export default function FlowCanvasCompat({
  nodes,
  edges,
  onConnect,
  onNodeDragStop,
  onNodeDoubleClick,
  style,
  fitView,
}: Props) {
  const [mod, setMod] = useState<any>(null);

  useEffect(() => {
    let mounted = true;
    (async () => {
      try {
        const m: any = await import("reactflow"); // single package only
        if (!mounted) return;
        setMod(m);
      } catch (_) {
        if (!mounted) return;
        setMod({}); // remain non-crashing; we’ll show a hint instead
      }
    })();
    return () => {
      mounted = false;
    };
  }, []);

  const ReactFlow = useMemo(() => (mod && (mod.ReactFlow || mod.default)) || null, [mod]);
  const Background = useMemo(() => (mod && mod.Background) || null, [mod]);
  const MiniMap = useMemo(() => (mod && mod.MiniMap) || null, [mod]);
  const Controls = useMemo(() => (mod && mod.Controls) || null, [mod]);

  if (!ReactFlow) {
    return (
      <div
        style={{
          ...style,
          padding: 12,
          color: "#cbd5e1",
          border: "1px solid rgba(148,163,184,.2)",
          borderRadius: 8,
          background: "rgba(15,23,42,.35)",
        }}
      >
        Loading canvas… Ensure only <code>reactflow</code> is installed (remove all <code>@reactflow/*</code> subpackages).
      </div>
    );
  }

  // Import CSS only once we know "reactflow" resolved (prevents bad export paths).
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  try { require("reactflow/dist/style.css"); } catch {}

  const children: React.ReactNode[] = [];
  if (isRenderable(Background)) children.push(React.createElement(Background, { key: "bg" }));
  if (isRenderable(MiniMap)) children.push(React.createElement(MiniMap, { key: "mm" }));
  if (isRenderable(Controls)) children.push(React.createElement(Controls, { key: "ctl" }));

  return React.createElement(ReactFlow, {
    nodes,
    edges,
    onConnect,
    onNodeDragStop,
    onNodeDoubleClick,
    fitView,
    children,
    style,
  });
}
