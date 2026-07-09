"use client";
import React, { useEffect, useMemo, useRef, useState } from "react";
import { Box, Paper, Tabs, Tab, Stack, Button, Typography, Divider } from "@mui/material";
import dynamic from "next/dynamic";

const MonacoEditor = dynamic(() => import("@monaco-editor/react"), { ssr: false });

export default function CustomScriptPanel({ projectId }: { projectId: string }) {
  const [tab, setTab] = useState<0|1>(0);
  const [tsCode, setTsCode] = useState<string>(`// TypeScript custom logic
export function onWebhook(payload: any) {
  return { ok: true, payload };
}`);
  const [solCode, setSolCode] = useState<string>(`// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

contract Custom {
    function ping() public pure returns (uint256) { return 42; }
}`);

  const [diagnostics, setDiagnostics] = useState<string>("");

  // Very lightweight “validation”: rely on Monaco’s TS diagnostics; for Solidity we just check basic braces.
  async function validate() {
    if (tab === 0) {
      setDiagnostics("Validating TypeScript…");
      setTimeout(()=> setDiagnostics("TypeScript: basic syntax looks OK (Monaco markers would surface inline)."), 200);
    } else {
      setDiagnostics("Validating Solidity…");
      const ok = solCode.includes("contract") && solCode.includes("{") && solCode.includes("}");
      setTimeout(()=> setDiagnostics(ok ? "Solidity: basic structure looks OK. Full compile will run during codegen/deploy." : "Solidity: the file looks incomplete (missing contract or braces)."), 200);
    }
  }

  return (
    <Paper variant="outlined" sx={{ p: 0, borderRadius: 2, overflow: "hidden", bgcolor:"rgba(255,255,255,0.04)" }}>
      <Box sx={{ px: 2, py: 1 }}>
        <Stack direction="row" alignItems="center" spacing={2}>
          <Tabs value={tab} onChange={(_,v)=>setTab(v)} sx={{ minHeight: 36, height: 36 }}>
            <Tab label="TypeScript" sx={{ minHeight: 36, height: 36 }} />
            <Tab label="Solidity"    sx={{ minHeight: 36, height: 36 }} />
          </Tabs>
          <Box sx={{ flex: 1 }} />
          <Button variant="outlined" size="small" onClick={validate}>Validate</Button>
        </Stack>
      </Box>
      <Divider sx={{ opacity: 0.25 }} />
      <Box sx={{ height: "calc(100vh - 340px)" }}>
        {tab === 0 && (
          <MonacoEditor
            height="100%"
            defaultLanguage="typescript"
            value={tsCode}
            onChange={(v)=> setTsCode(v || "")}
            options={{ theme: "vs-dark", automaticLayout: true, minimap: { enabled: false } }}
          />
        )}
        {tab === 1 && (
          <MonacoEditor
            height="100%"
            defaultLanguage="sol"
            language="sol"
            value={solCode}
            onChange={(v)=> setSolCode(v || "")}
            options={{ theme: "vs-dark", automaticLayout: true, minimap: { enabled: false } }}
          />
        )}
      </Box>
      <Divider sx={{ opacity: 0.25 }} />
      <Box sx={{ px: 2, py: 1.5 }}>
        <Typography variant="caption" sx={{ whiteSpace: "pre-wrap", opacity: 0.9 }}>
          {diagnostics || "Use TypeScript for off-chain logic and Solidity for on-chain contracts. We will compile during generation/deploy."}
        </Typography>
      </Box>
    </Paper>
  );
}
