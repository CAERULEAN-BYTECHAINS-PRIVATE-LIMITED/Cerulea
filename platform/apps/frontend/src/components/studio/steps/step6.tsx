'use client';

import React, { useEffect, useRef, useState } from "react";
import {
  Box, Button, Divider, LinearProgress, Paper, Stack, Typography,
  Chip, Fade, IconButton, Tooltip, Alert, CircularProgress
} from "@mui/material";
import Grid from '@mui/material/GridLegacy';
import { useTheme, styled, alpha } from "@mui/material/styles";
import { useStudio } from "@/context/StudioContext";

// Icons
import ArrowBackIcon from "@mui/icons-material/KeyboardArrowLeft";
import RocketLaunchIcon from "@mui/icons-material/RocketLaunch";
import CheckCircleIcon from "@mui/icons-material/CheckCircle";
import VerifiedUserIcon from "@mui/icons-material/VerifiedUser";
import CodeIcon from "@mui/icons-material/Code";
import CloudQueueIcon from "@mui/icons-material/CloudQueue";
import DnsIcon from "@mui/icons-material/Dns";
import FactCheckIcon from "@mui/icons-material/FactCheck";
import MemoryIcon from "@mui/icons-material/Memory";
import TerminalIcon from '@mui/icons-material/Terminal';
import LockIcon from '@mui/icons-material/Lock';
import SpeedIcon from '@mui/icons-material/Speed';
import NetworkCheckIcon from '@mui/icons-material/NetworkCheck';
import StorageIcon from '@mui/icons-material/Storage';
import OpenInNewIcon from "@mui/icons-material/OpenInNew";

/* ------------------ Types ------------------ */
type LogPhase =
  | "idle"
  | "validation"
  | "code_generation"
  | "infra_provisioning"
  | "service_deployment"
  | "post_deploy_checks"
  | "background_finalization";

type DeployMeta = {
  deployId: string;
  region: string;
  startTime: number;
};

/* ------------------ Styled Components ------------------ */

const FloatingIsland = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255, 255, 255, 0.95)' : 'rgba(20, 20, 23, 0.95)',
  backdropFilter: 'blur(16px)',
  border: `1px solid ${theme.palette.divider}`,
  boxShadow: '0 20px 40px -8px rgba(0, 0, 0, 0.3)',
  borderRadius: 100,
  padding: '8px 24px',
  display: 'flex',
  alignItems: 'center',
  gap: 16,
  zIndex: 1000,
  pointerEvents: 'auto',
}));

const StepPill = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? 'rgba(255, 255, 255, 0.9)' : 'rgba(20, 20, 23, 0.9)',
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

const TerminalWindow = styled(Paper)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? '#f5f5f5' : '#0a0a0c',
  border: `1px solid ${alpha(theme.palette.divider, 0.1)}`,
  borderRadius: 16,
  overflow: 'hidden',
  display: 'flex',
  flexDirection: 'column',
  flex: 1, 
  boxShadow: theme.palette.mode === 'light' 
    ? '0 12px 24px -8px rgba(0,0,0,0.1)' 
    : '0 24px 48px -12px rgba(0,0,0,0.5)',
  fontFamily: '"Fira Code", "Roboto Mono", monospace',
}));

const TerminalHeader = styled(Box)(({ theme }) => ({
  background: theme.palette.mode === 'light' ? '#e0e0e0' : '#141416',
  padding: '12px 16px',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  borderBottom: `1px solid ${theme.palette.divider}`,
}));

const TerminalDot = styled(Box)<{ color: string }>(({ color }) => ({
  width: 10,
  height: 10,
  borderRadius: '50%',
  backgroundColor: color,
}));

const PipelineStep = styled(Box, { shouldForwardProp: (p) => p !== 'active' && p !== 'completed' })<{ active?: boolean; completed?: boolean }>(({ theme, active, completed }) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 16,
  padding: '16px 20px',
  borderRadius: 12,
  transition: 'all 0.3s ease',
  backgroundColor: active ? alpha(theme.palette.primary.main, 0.08) : completed ? alpha(theme.palette.success.main, 0.05) : 'transparent',
  border: `1px solid ${active ? alpha(theme.palette.primary.main, 0.3) : completed ? alpha(theme.palette.success.main, 0.2) : 'transparent'}`,
  opacity: active || completed ? 1 : 0.4,
  marginBottom: 8,
}));

const SummaryCard = styled(Paper)(({ theme }) => ({
  padding: 24,
  borderRadius: 20,
  border: `1px solid ${theme.palette.divider}`,
  background: theme.palette.mode === 'light' ? 'rgba(255,255,255,0.6)' : 'rgba(20,20,24,0.6)',
  backdropFilter: 'blur(12px)',
}));

const MetricCard = styled(Paper)(({ theme }) => ({
  padding: 16,
  borderRadius: 16,
  background: theme.palette.mode === 'light' ? '#fff' : '#13151C',
  border: `1px solid ${theme.palette.divider}`,
  display: 'flex',
  flexDirection: 'column',
  gap: 4,
}));

/* ------------------ Constants ------------------ */

const SCRIPTED_LOGS = [
  "Reading cerulea.config.ts...",
  "Resolving module dependencies...",
  "Validating entity schema 'User'...",
  "Validating entity schema 'Project'...",
  "Checking integration keys for 'Stripe'...",
  "Verifying wallet signature...",
  "Audit: No critical vulnerabilities found in config.",
  "Initializing code generator v2.4.0...",
  "Scaffolding Next.js 14 frontend...",
  "Generating Solidity contracts...",
  "Compiling contracts with Hardhat...",
  "Generating Typechain bindings...",
  "Building API routes for 'Auth' module...",
  "Optimizing static assets...",
  "Tree-shaking unused dependencies...",
  "Build complete: .next/ (45MB)",
  "Build complete: artifacts/ (12MB)",
];

const INFRA_LOG_POOL = [
  "aws_vpc.main: Creating...",
  "aws_vpc.main: Creation complete after 12s [id=vpc-0a8b...]",
  "aws_subnet.public_a: Creating...",
  "aws_subnet.public_a: Creation complete after 4s",
  "aws_internet_gateway.gw: Creating...",
  "aws_security_group.allow_tls: Creating...",
  "aws_db_instance.postgres: Creating... (this may take a while)",
  "aws_db_instance.postgres: Still creating... [10s elapsed]",
  "aws_db_instance.postgres: Still creating... [20s elapsed]",
  "aws_ecs_cluster.main: Creating...",
  "aws_iam_role.ecs_task_execution_role: Creating...",
  "aws_lb.front_end: Creating...",
  "aws_lb_target_group.front_end: Creating...",
  "Provisioning complete. Applying state...",
  "Initializing Kubernetes control plane...",
  "Waiting for nodes to join cluster...",
  "Node ip-10-0-1-45.ec2.internal joined.",
  "Node ip-10-0-1-120.ec2.internal joined.",
  "Pulling image: cerulea/core:latest...",
  "Pulling image: cerulea/worker:latest...",
  "Deploying deployment.apps/web-server...",
  "Deploying service/web-loadbalancer...",
  "Waiting for load balancer to become healthy...",
];

const BLOCKCHAIN_LOG_POOL = [
  "Bootnode: Started P2P networking on 0.0.0.0:30333",
  "Genesis: Initializing chain spec...",
  "Consensus: Aura (Authorities: 0x4a...e1)",
  "Grandpa: Voters initialized.",
  "Sync: 0 peers connected.",
  "Sync: 4 peers connected. Downloading headers...",
  "Imported #1 (0x4a...b2) - 1.2MB",
  "Imported #2 (0x9c...f1) - 0.8MB",
  "Telemetry: Connecting to telemetry.polkadot.io...",
  "RPC: HTTP server started on 127.0.0.1:9933",
  "RPC: WebSocket server started on 127.0.0.1:9944",
  "TxPool: 0 ready, 0 pending",
  "Mining: Prepared block for proposing at 6000ms",
  "Grandpa: Finalizing block #1...",
  "State: Caching trie nodes...",
  "Database: Compacting RocksDB...",
];

const DEPLOYMENT_PHASES: Array<{ id: LogPhase; label: string; icon: React.ReactNode }> = [
  { id: 'validation', label: 'Validation & Security', icon: <VerifiedUserIcon fontSize="small" /> },
  { id: 'code_generation', label: 'Code Generation', icon: <CodeIcon fontSize="small" /> },
  { id: 'infra_provisioning', label: 'Infra Provisioning', icon: <CloudQueueIcon fontSize="small" /> },
  { id: 'service_deployment', label: 'Service Deployment', icon: <DnsIcon fontSize="small" /> },
  { id: 'post_deploy_checks', label: 'Health Checks', icon: <FactCheckIcon fontSize="small" /> },
  { id: 'background_finalization', label: 'Finalization', icon: <MemoryIcon fontSize="small" /> },
];

function generateRandomHex(len: number) {
  const chars = "0123456789ABCDEF";
  let res = "";
  for (let i = 0; i < len; i++) res += chars[Math.floor(Math.random() * 16)];
  return res;
}

function generateLogLine(phase: LogPhase, projectType: 'dapp' | 'blockchain' | null): string {
  const source = projectType === 'blockchain' ? BLOCKCHAIN_LOG_POOL : INFRA_LOG_POOL;
  let pool: string[] = [];
  
  if (phase === 'infra_provisioning') pool = source;
  else if (phase === 'service_deployment') pool = source;
  else if (phase === 'post_deploy_checks') pool = ["Health check: 200 OK", "Latency check: 45ms", "Consistency check: PASSED", "Uptime monitor: ACTIVE"];
  else pool = ["Syncing state...", "Processing background jobs...", "Indexing blocks...", "Optimizing storage..."];

  if (pool.length === 0) pool = ["Processing..."];
  
  const base = pool[Math.floor(Math.random() * pool.length)];
  const detail = Math.random() > 0.7 ? ` [${Math.floor(Math.random() * 500)}ms]` : '';
  return `> ${base}${detail}`;
}

// Random Walk helper for smoother stats
function walkValue(current: number, min: number, max: number, volatility: number) {
  const change = (Math.random() - 0.5) * volatility;
  let next = current + change;
  if (next < min) next = min + Math.random() * (volatility / 2);
  if (next > max) next = max - Math.random() * (volatility / 2);
  return Math.round(next * 10) / 10;
}

/* ====================================================================== */
export default function Step6({ goPrev }: { goPrev?: () => void }) {
  const theme = useTheme();
  const { appMetadata, selectedModules, projectType } = useStudio(); 

  // State
  const [deploying, setDeploying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [logs, setLogs] = useState<string[]>([]);
  const [activePhase, setActivePhase] = useState<LogPhase>("idle");
  const [completedPhases, setCompletedPhases] = useState<Set<LogPhase>>(new Set());
  const [deployMeta, setDeployMeta] = useState<DeployMeta | null>(null);
  
  // Metrics State
  const [metricsActive, setMetricsActive] = useState(false);
  const [metrics, setMetrics] = useState({ cpu: 0, ram: 0, net: 0, storage: 200 }); // Storage in MB

  // Refs
  const scrollRef = useRef<HTMLDivElement>(null);
  const timerRef = useRef<any>(null);
  const metricsTimerRef = useRef<any>(null);
  const logIndexRef = useRef(0);

  // Constants
  const peerLabel = 'STORAGE'; // Changed from Peers/Instances to Storage

  // Auto-scroll logs
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [logs]);

  // Start Deployment Logic
  const startDeployment = () => {
    if (deploying) return;

    const meta = {
      deployId: `dep-${generateRandomHex(8)}`, 
      region: "us-east-1",
      startTime: Date.now()
    };
    setDeployMeta(meta);
    setDeploying(true);
    setMetricsActive(false); // Reset metrics
    setMetrics({ cpu: 0, ram: 0, net: 0, storage: 200 }); // Base OS size 200MB
    setLogs([`INITIALIZING DEPLOYMENT: ${meta.deployId}`, `TARGET REGION: ${meta.region}`, `Loading blueprint configuration...`]);
    setProgress(0);
    setActivePhase("validation");
    setCompletedPhases(new Set());
    logIndexRef.current = 0;

    // 1. Metrics Boot Delay (10-12 Seconds)
    setTimeout(() => {
      setMetricsActive(true);
      // Initialize starting values when they "boot up"
      setMetrics({ cpu: 15, ram: 24, net: 0.5, storage: 210 });
    }, 12000);

    // 2. Main Progression Loop
    timerRef.current = setInterval(() => {
      setProgress((prev) => {
        let currentPhase: LogPhase = "validation";
        let speed = 0.5;

        // --- REALISTIC TIMING LOGIC ---
        // Validation (0-10%): Takes ~3 mins (180s). Tick is 500ms. 
        if (prev < 10) { 
           currentPhase = "validation"; 
           speed = 0.02 + Math.random() * 0.02; // Very slow start
        } 
        // Code Gen (10-25%): Slows down further
        else if (prev < 25) { 
           currentPhase = "code_generation"; 
           speed = 0.015 + Math.random() * 0.02;
        } 
        // Infra (25-60%): The "Hours" slog
        else if (prev < 60) { 
           currentPhase = "infra_provisioning"; 
           speed = 0.0005; // Effectively frozen (Hours)
        } 
        // Service Deploy (60-90%): Still slow
        else if (prev < 90) { 
           currentPhase = "service_deployment"; 
           speed = 0.001; 
        } 
        // Finalizing (90+): Crawl
        else { 
           currentPhase = "background_finalization"; 
           speed = 0.0001; 
        }

        setActivePhase((p) => {
           if (p !== currentPhase && p !== 'idle') {
              setCompletedPhases((s) => {
                 const newSet = new Set(s);
                 newSet.add(p);
                 return newSet;
              });
           }
           return currentPhase;
        });

        // Log Logic
        setLogs((prevLogs) => {
           let newLog: string | null = null;
           const isFastPhase = currentPhase === 'validation' || currentPhase === 'code_generation';
           
           if (isFastPhase) {
              // In validation phase, output scripted logs slowly
              if (Math.random() > 0.9 && logIndexRef.current < SCRIPTED_LOGS.length) {
                 newLog = `> ${SCRIPTED_LOGS[logIndexRef.current]}`;
                 logIndexRef.current++;
              }
           } else {
              // In heavy phases, logs are VERY rare
              if (Math.random() > 0.99) { 
                 const newRaw = generateLogLine(currentPhase, projectType);
                 const ts = new Date().toISOString().split('T')[1].split('.')[0];
                 newLog = `${ts} ${newRaw}`;
              }
           }

           if (newLog) {
              const updated = [...prevLogs, newLog];
              return updated.length > 1000 ? updated.slice(updated.length - 1000) : updated;
           }
           return prevLogs;
        });

        const next = prev + speed;
        return next > 99.9 ? 99.9 : next;
      });
    }, 500);

    // 3. Metrics Random Walk Loop (Smoother)
    metricsTimerRef.current = setInterval(() => {
      setMetrics((prev) => {
        // "Step & Creep" Storage Logic
        // 95% chance of slow creep (logs), 5% chance of burst (install)
        const isBurst = Math.random() > 0.95;
        const growth = isBurst 
           ? (10 + Math.random() * 40) // Burst 10-50MB
           : (0.1 + Math.random() * 0.5); // Creep 0.1-0.6MB

        // Bursty Network Logic
        // 90% chance of lull, 10% chance of high activity
        const isNetBurst = Math.random() > 0.90;
        const netTarget = isNetBurst 
           ? (40 + Math.random() * 60) // 40-100 MB/s
           : (0.1 + Math.random() * 2); // 0.1-2 MB/s
        
        // Smoothly approach the network target instead of jumping instantly
        const netDiff = netTarget - prev.net;
        const newNet = prev.net + (netDiff * 0.2); // Move 20% towards target per tick

        return {
          cpu: walkValue(prev.cpu, 10, 80, 5),      // CPU 10-80%
          ram: walkValue(prev.ram, 20, 60, 2),      // RAM 20-60%
          net: Math.round(newNet * 10) / 10,        // Smoothed Net
          storage: prev.storage + growth            // Accumulating storage
        };
      });
    }, 1000);
  };

  // Cleanup
  useEffect(() => {
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
      if (metricsTimerRef.current) clearInterval(metricsTimerRef.current);
    };
  }, []);

  return (
    <>
      <Box sx={{ width: '100%', position: 'fixed', inset: 0, top: 64, bgcolor: 'background.default', display: 'flex', flexDirection: 'column' }}>

         <Box sx={{ position: 'absolute', inset: 0, opacity: 0.3, zIndex: -1,
            backgroundImage: theme.palette.mode === 'light' ? 'radial-gradient(#ccc 1px, transparent 1px)' : 'radial-gradient(#333 1px, transparent 1px)',
            backgroundSize: '24px 24px'
         }} />

         <Box sx={{ position: 'absolute', top: 24, left: '50%', transform: 'translateX(-50%)', zIndex: 10, pointerEvents: 'none' }}>
            <StepPill>
               <Typography variant="overline" fontWeight={800} color="primary" sx={{ letterSpacing: 1, lineHeight: 1 }}>STEP 6 OF 6</Typography>
               <Divider orientation="vertical" flexItem sx={{ height: 14, my: 'auto', opacity: 0.5 }} />
               <Typography variant="subtitle2" fontWeight={700}>Review & Deploy</Typography>
            </StepPill>
         </Box>

         <Box sx={{ flex: 1, display: 'flex', overflow: 'hidden', pt: 10, px: 4, pb: 14 }}>
            <Grid container spacing={4} sx={{ height: '100%' }}>
               
               <Grid xs={12} md={4} sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 3 }}>
                  <SummaryCard>
                     <Stack direction="row" alignItems="center" spacing={2} mb={2}>
                        <Box sx={{ p: 1.5, borderRadius: 2, bgcolor: 'primary.main', color: 'white' }}>
                           <RocketLaunchIcon />
                        </Box>
                        <Box>
                           <Typography variant="h6" fontWeight={900}>{appMetadata?.appName || "New Project"}</Typography>
                           <Typography variant="body2" fontWeight={600} color="text.primary" sx={{ opacity: 0.8 }}>{projectType === 'blockchain' ? 'Layer 1 Network' : 'Full Stack dApp'}</Typography>
                        </Box>
                     </Stack>

                     {!deploying && (
                        <Box sx={{ p: 2, borderRadius: 2, bgcolor: (t) => t.palette.mode === 'light' ? 'rgba(0,0,0,0.04)' : 'rgba(255,255,255,0.06)', mb: 2, border: (t) => `1px solid ${t.palette.divider}` }}>
                           <Typography variant="body2" fontWeight={700} sx={{ mb: 0.75 }}>What happens when you deploy</Typography>
                           <Stack spacing={0.5}>
                             {[
                               'Cerulea validates your configuration and checks for conflicts',
                               'Smart contracts are compiled and generated from your modules',
                               'Infrastructure is provisioned in your selected region',
                               'Your app goes live with an RPC endpoint and dashboard URL',
                             ].map((line, i) => (
                               <Stack key={i} direction="row" spacing={1} alignItems="flex-start">
                                 <Typography variant="caption" fontWeight={800} color="primary.main" sx={{ mt: 0.1, flexShrink: 0 }}>{i + 1}.</Typography>
                                 <Typography variant="body2" color="text.primary" sx={{ lineHeight: 1.5 }}>{line}</Typography>
                               </Stack>
                             ))}
                           </Stack>
                        </Box>
                     )}

                     {deploying && (
                        <Alert severity="info" variant="outlined" sx={{ mt: 2, bgcolor: alpha(theme.palette.info.main, 0.1) }}>
                           <Typography variant="caption" fontWeight={700}>
                              Deployment in progress. This process will take hours. Do not close this window.
                           </Typography>
                        </Alert>
                     )}
                  </SummaryCard>

                  <Box sx={{ flex: 1, overflowY: 'auto' }}>
                     <Typography variant="overline" fontWeight={800} color="text.secondary" sx={{ pl: 1, mb: 1, display: 'block' }}>DEPLOYMENT SEQUENCE</Typography>
                     {DEPLOYMENT_PHASES.map((p) => {
                        const isActive = activePhase === p.id;
                        const isCompleted = completedPhases.has(p.id as LogPhase);
                        
                        return (
                           <PipelineStep key={p.id} active={isActive} completed={isCompleted}>
                              <Box sx={{ color: isCompleted ? 'success.main' : isActive ? 'primary.main' : 'text.disabled', display: 'flex' }}>
                                 {isCompleted ? <CheckCircleIcon /> : isActive ? <CircularProgress size={24} color="inherit" /> : p.icon}
                              </Box>
                              <Box sx={{ flex: 1 }}>
                                 <Typography variant="subtitle2" fontWeight={700} color={isActive || isCompleted ? 'text.primary' : 'text.disabled'}>
                                    {p.label}
                                 </Typography>
                                 {isActive && <Typography variant="caption" color="primary">Processing...</Typography>}
                              </Box>
                           </PipelineStep>
                        );
                     })}
                  </Box>
               </Grid>

               <Grid xs={12} md={8} sx={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 2 }}>
                  <Box display="grid" gridTemplateColumns="repeat(4, 1fr)" gap={2}>
                     <MetricCard>
                        <Stack direction="row" alignItems="center" gap={1} color="text.secondary">
                           <SpeedIcon fontSize="small" /> <Typography variant="caption" fontWeight={700}>CPU LOAD</Typography>
                        </Stack>
                        <Typography variant="h5" fontWeight={800} color="primary.main" fontFamily="monospace">
                           {metricsActive ? `${metrics.cpu}%` : '--'}
                        </Typography>
                     </MetricCard>
                     <MetricCard>
                        <Stack direction="row" alignItems="center" gap={1} color="text.secondary">
                           <MemoryIcon fontSize="small" /> <Typography variant="caption" fontWeight={700}>MEMORY</Typography>
                        </Stack>
                        <Typography variant="h5" fontWeight={800} color="secondary.main" fontFamily="monospace">
                           {metricsActive ? `${metrics.ram}%` : '--'}
                        </Typography>
                     </MetricCard>
                     <MetricCard>
                        <Stack direction="row" alignItems="center" gap={1} color="text.secondary">
                           <NetworkCheckIcon fontSize="small" /> <Typography variant="caption" fontWeight={700}>NET I/O</Typography>
                        </Stack>
                        <Typography variant="h5" fontWeight={800} color="warning.main" fontFamily="monospace">
                           {metricsActive ? `${metrics.net} MB/s` : '--'}
                        </Typography>
                     </MetricCard>
                     <MetricCard>
                        <Stack direction="row" alignItems="center" gap={1} color="text.secondary">
                           <StorageIcon fontSize="small" /> <Typography variant="caption" fontWeight={700}>STORAGE</Typography>
                        </Stack>
                        <Typography variant="h5" fontWeight={800} color="success.main" fontFamily="monospace">
                           {metricsActive ? `${Math.floor(metrics.storage)} MB` : '--'}
                        </Typography>
                     </MetricCard>
                  </Box>

                  <TerminalWindow elevation={10}>
                     <TerminalHeader>
                        <Stack direction="row" alignItems="center" spacing={1}>
                           <TerminalDot color="#FF5F56" />
                           <TerminalDot color="#FFBD2E" />
                           <TerminalDot color="#27C93F" />
                        </Stack>
                        {deployMeta && (
                           <Typography variant="caption" fontFamily="monospace" color="text.secondary">
                              ID: {deployMeta.deployId} | REGION: {deployMeta.region}
                           </Typography>
                        )}
                     </TerminalHeader>
                     
                     <Box ref={scrollRef} sx={{ flex: 1, p: 2, overflowY: 'auto', lineHeight: 1.6 }}>
                        {!deploying && logs.length === 0 ? (
                           <Box sx={{ height: '100%', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', opacity: 0.3 }}>
                              <TerminalIcon sx={{ fontSize: 64, mb: 2 }} />
                              <Typography>System Ready.</Typography>
                              <Typography>Waiting for initialization...</Typography>
                           </Box>
                        ) : (
                           <>
                              {logs.map((log, i) => (
                                 <div key={i} style={{ marginBottom: 2, wordBreak: 'break-all', color: theme.palette.mode === 'light' ? '#333' : '#e0e0e0', fontSize: 13 }}>
                                    {log.includes('>') ? (
                                       <span style={{ color: theme.palette.success.main, fontWeight: 'bold' }}>{log}</span>
                                    ) : (
                                       <span>{log}</span>
                                    )}
                                 </div>
                              ))}
                              {deploying && (
                                 <div className="animate-pulse" style={{ marginTop: 8, color: theme.palette.success.main, fontWeight: 'bold' }}>_</div>
                              )}
                           </>
                        )}
                     </Box>

                     {deploying && (
                        <Box sx={{ p: 2, bgcolor: theme.palette.mode === 'light' ? '#eee' : '#111', borderTop: `1px solid ${theme.palette.divider}` }}>
                           <Stack direction="row" justifyContent="space-between" mb={1}>
                              <Typography variant="caption" color="text.secondary" fontFamily="monospace">STATUS: {activePhase.toUpperCase().replace('_', ' ')}</Typography>
                              <Typography variant="caption" color="primary" fontFamily="monospace">{progress.toFixed(3)}%</Typography>
                           </Stack>
                           <LinearProgress variant="determinate" value={progress} sx={{ height: 4, borderRadius: 2 }} />
                        </Box>
                     )}
                  </TerminalWindow>
               </Grid>

            </Grid>
         </Box>

         <Box sx={{ position: 'absolute', bottom: 32, left: '50%', transform: 'translateX(-50%)', zIndex: 100 }}>
            <FloatingIsland elevation={6}>
               <Tooltip title="Back">
                 <IconButton onClick={goPrev} size="small" sx={{border: '1px solid', borderColor:'divider'}}>
                    <ArrowBackIcon />
                 </IconButton>
               </Tooltip>
               <Divider orientation="vertical" flexItem sx={{ height: 20, my: 'auto' }} />
               
               {!deploying ? (
                  <Button 
                     variant="contained" 
                     size="large"
                     onClick={startDeployment}
                     startIcon={<RocketLaunchIcon />}
                     sx={{ borderRadius: 100, px: 4, fontWeight: 800, background: 'linear-gradient(45deg, #2563eb, #7c3aed)' }}
                  >
                     Initialize Deployment
                  </Button>
               ) : (
                  <Tooltip title="Deployment is in progress. Please wait.">
                     <span>
                        <Button 
                           variant="outlined" 
                           size="large"
                           disabled
                           startIcon={<LockIcon />}
                           sx={{ borderRadius: 100, px: 4, fontWeight: 700 }}
                        >
                           Open Dashboard
                        </Button>
                     </span>
                  </Tooltip>
               )}
            </FloatingIsland>
         </Box>

      </Box>
    </>
  );
}