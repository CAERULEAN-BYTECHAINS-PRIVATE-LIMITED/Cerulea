'use client';

import { useEffect, useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Tooltip, LinearProgress, CircularProgress,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import { useSession } from 'next-auth/react';
import AddIcon from '@mui/icons-material/Add';
import VpnKeyIcon from '@mui/icons-material/VpnKey';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import ErrorIcon from '@mui/icons-material/Error';
import ShieldIcon from '@mui/icons-material/Shield';
import HubIcon from '@mui/icons-material/Hub';
import DeviceHubIcon from '@mui/icons-material/DeviceHub';
import DnsIcon from '@mui/icons-material/Dns';

type Node = {
  id: string;
  role: 'Validator' | 'RPC' | 'Archival';
  status: 'Active' | 'Down' | 'Syncing';
  region: string;
  uptime: number;
  network: string;
  cpuPct: number;
  memPct: number;
};

const TEST_NODES: Node[] = [
  { id: 'node-val-01', role: 'Validator', status: 'Active',  region: 'us-east-1',      uptime: 99.94, network: 'CeruleaChain Mainnet',  cpuPct: 34, memPct: 51 },
  { id: 'node-val-02', role: 'Validator', status: 'Active',  region: 'eu-west-1',      uptime: 99.81, network: 'CeruleaChain Mainnet',  cpuPct: 29, memPct: 48 },
  { id: 'node-val-03', role: 'Validator', status: 'Down',    region: 'ap-southeast-1', uptime: 71.2,  network: 'CeruleaChain Mainnet',  cpuPct: 0,  memPct: 0 },
  { id: 'node-val-04', role: 'Validator', status: 'Active',  region: 'eu-central-1',   uptime: 100,   network: 'TradeFi Private L1',    cpuPct: 22, memPct: 44 },
  { id: 'node-val-05', role: 'Validator', status: 'Active',  region: 'us-west-2',      uptime: 99.97, network: 'TradeFi Private L1',    cpuPct: 27, memPct: 46 },
  { id: 'node-rpc-01', role: 'RPC',       status: 'Active',  region: 'us-east-1',      uptime: 100,   network: 'VoteApp Devnet',         cpuPct: 18, memPct: 38 },
  { id: 'node-rpc-02', role: 'RPC',       status: 'Active',  region: 'eu-west-1',      uptime: 99.99, network: 'VoteApp Devnet',         cpuPct: 21, memPct: 40 },
  { id: 'node-rpc-03', role: 'RPC',       status: 'Syncing', region: 'ap-northeast-1', uptime: 98.1,  network: 'NFT Marketplace Chain', cpuPct: 55, memPct: 67 },
  { id: 'node-arc-01', role: 'Archival',  status: 'Active',  region: 'us-west-2',      uptime: 99.55, network: 'CeruleaChain Mainnet',  cpuPct: 12, memPct: 72 },
  { id: 'node-arc-02', role: 'Archival',  status: 'Active',  region: 'eu-west-2',      uptime: 99.8,  network: 'TradeFi Private L1',    cpuPct: 9,  memPct: 68 },
];

const ROLE_COLOR: Record<string, string> = { Validator: 'secondary', RPC: 'primary', Archival: 'info' };
const STATUS_COLORS: Record<string, string> = { Active: 'success.main', Down: 'error.main', Syncing: 'warning.main' };

function FaultCard({ label, value, sub, color, icon }: {
  label: string; value: string | number; sub?: string; color: string; icon: React.ReactNode;
}) {
  const theme = useTheme();
  return (
    <Paper variant="outlined" sx={{ p: 2.5, borderRadius: 3, background: alpha(color, 0.05), borderColor: alpha(color, 0.2) }}>
      <Stack direction="row" alignItems="center" justifyContent="space-between">
        <Box>
          <Typography variant="caption" color="text.secondary" fontWeight={700} sx={{ letterSpacing: 0.5 }}>{label.toUpperCase()}</Typography>
          <Typography variant="h4" fontWeight={900} sx={{ color, mt: 0.5 }}>{value}</Typography>
          {sub && <Typography variant="caption" color="text.secondary">{sub}</Typography>}
        </Box>
        <Box sx={{ color, opacity: 0.6 }}>{icon}</Box>
      </Stack>
    </Paper>
  );
}

export default function NodesPage() {
  const theme = useTheme();
  const { data: session } = useSession();
  const [nodes, setNodes] = useState<Node[]>([]);
  const [loading, setLoading] = useState(true);

  const isTestAccount = session?.user?.email === 'test@cerulea.app';

  useEffect(() => {
    if (!session) return;
    if (isTestAccount) {
      setNodes(TEST_NODES);
      setLoading(false);
      return;
    }
    // For regular users: derive nodes from their deployed projects
    (async () => {
      try {
        const res = await fetch('/api/projects');
        if (!res.ok) { setLoading(false); return; }
        const j = await res.json();
        const userProjects = (j.projects || []).filter((p: any) => p.status === 'active');
        const derived: Node[] = [];
        userProjects.forEach((p: any, i: number) => {
          derived.push(
            { id: `node-val-${p.id.slice(0, 6)}`, role: 'Validator', status: 'Active', region: 'us-east-1', uptime: 99.9, network: p.name, cpuPct: 25, memPct: 45 },
            { id: `node-rpc-${p.id.slice(0, 6)}`, role: 'RPC', status: 'Active', region: 'eu-west-1', uptime: 100, network: p.name, cpuPct: 18, memPct: 38 },
          );
        });
        setNodes(derived);
      } catch {
        // silent
      } finally {
        setLoading(false);
      }
    })();
  }, [session, isTestAccount]);

  const activeNodes = nodes.filter((n) => n.status === 'Active');
  const downNodes = nodes.filter((n) => n.status === 'Down');
  const validators = nodes.filter((n) => n.role === 'Validator');
  const activeValidators = validators.filter((n) => n.status === 'Active');
  const faultTolerance = validators.length ? Math.floor((activeValidators.length / validators.length) * 100) : 0;

  return (
    <Box sx={{ p: 4, maxWidth: 1200 }}>
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>Nodes</Typography>
          <Typography variant="body1" color="text.secondary">
            Manage your validator, RPC, and archival nodes across all networks.
          </Typography>
        </Box>
        <Stack direction="row" spacing={1.5}>
          <Tooltip title="Available after deployment" arrow>
            <span>
              <Button variant="contained" startIcon={<AddIcon />} disabled sx={{ borderRadius: 999, fontWeight: 700 }}>
                Provision Node
              </Button>
            </span>
          </Tooltip>
          <Button variant="outlined" startIcon={<VpnKeyIcon />} sx={{ borderRadius: 999, fontWeight: 700 }}>
            Rotate Keys
          </Button>
        </Stack>
      </Stack>

      {loading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}><CircularProgress /></Box>
      ) : nodes.length === 0 ? (
        <Paper variant="outlined" sx={{ borderRadius: 3, py: 10, textAlign: 'center', borderStyle: 'dashed' }}>
          <DnsIcon sx={{ fontSize: 48, color: 'text.disabled', mb: 1 }} />
          <Typography variant="body1" color="text.secondary" gutterBottom>No nodes provisioned yet.</Typography>
          <Typography variant="caption" color="text.secondary">Nodes are automatically provisioned when you deploy a network.</Typography>
        </Paper>
      ) : (
        <>
          <Grid container spacing={2.5} mb={4}>
            <Grid xs={12} sm={6} md={3}>
              <FaultCard label="Total Nodes" value={nodes.length} sub="Across all networks" color={theme.palette.primary.main} icon={<HubIcon sx={{ fontSize: 32 }} />} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <FaultCard label="Active" value={activeNodes.length} sub="Healthy and reachable" color={theme.palette.success.main} icon={<CheckCircleIcon sx={{ fontSize: 32 }} />} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <FaultCard label="Down / Degraded" value={downNodes.length} sub="Require attention" color={theme.palette.error.main} icon={<ErrorIcon sx={{ fontSize: 32 }} />} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <Paper variant="outlined" sx={{ p: 2.5, borderRadius: 3, background: alpha(theme.palette.warning.main, 0.05), borderColor: alpha(theme.palette.warning.main, 0.2) }}>
                <Typography variant="caption" color="text.secondary" fontWeight={700} sx={{ letterSpacing: 0.5 }}>FAULT TOLERANCE</Typography>
                <Typography variant="h4" fontWeight={900} sx={{ color: theme.palette.warning.main, mt: 0.5 }}>{faultTolerance}%</Typography>
                <LinearProgress variant="determinate" value={faultTolerance} color={faultTolerance >= 90 ? 'success' : faultTolerance >= 66 ? 'warning' : 'error'} sx={{ mt: 1, borderRadius: 999, height: 6 }} />
                <Stack direction="row" alignItems="center" gap={0.5} mt={0.5}>
                  <ShieldIcon sx={{ fontSize: 12, color: 'text.secondary' }} />
                  <Typography variant="caption" color="text.secondary">{activeValidators.length}/{validators.length} validators active</Typography>
                </Stack>
              </Paper>
            </Grid>
          </Grid>

          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
            <Box sx={{ px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}` }}>
              <Stack direction="row" alignItems="center" justifyContent="space-between">
                <Typography variant="h6" fontWeight={800}>Node Roster</Typography>
                <DeviceHubIcon sx={{ color: 'text.disabled' }} fontSize="small" />
              </Stack>
            </Box>
            <TableContainer>
              <Table>
                <TableHead>
                  <TableRow>
                    {['Node ID', 'Network', 'Role', 'Status', 'Region', 'CPU', 'Memory', 'Uptime', 'Actions'].map((h) => (
                      <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>{h.toUpperCase()}</TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {nodes.map((node) => (
                    <TableRow key={node.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) }, ...(node.status === 'Down' && { bgcolor: alpha(theme.palette.error.main, 0.03) }) }}>
                      <TableCell>
                        <Typography variant="body2" fontWeight={700} sx={{ fontFamily: 'monospace' }}>{node.id}</Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" color="text.secondary">{node.network}</Typography>
                      </TableCell>
                      <TableCell>
                        <Chip label={node.role} size="small" color={ROLE_COLOR[node.role] as any} variant="outlined" sx={{ fontWeight: 700, fontSize: '0.7rem' }} />
                      </TableCell>
                      <TableCell>
                        <Stack direction="row" alignItems="center" spacing={0.75}>
                          <Box sx={{ width: 8, height: 8, borderRadius: '50%', bgcolor: STATUS_COLORS[node.status] }} />
                          <Typography variant="body2" fontWeight={600} sx={{ color: STATUS_COLORS[node.status] }}>{node.status}</Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" color="text.secondary">{node.region}</Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" fontWeight={700} color={node.cpuPct > 80 ? 'error.main' : node.cpuPct > 60 ? 'warning.main' : 'text.secondary'}>
                          {node.status === 'Down' ? 'N/A' : `${node.cpuPct}%`}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" fontWeight={700} color={node.memPct > 85 ? 'error.main' : node.memPct > 70 ? 'warning.main' : 'text.secondary'}>
                          {node.status === 'Down' ? 'N/A' : `${node.memPct}%`}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Box sx={{ minWidth: 90 }}>
                          <Typography variant="caption" fontWeight={700} color={node.uptime >= 99 ? 'success.main' : node.uptime >= 90 ? 'warning.main' : 'error.main'}>
                            {node.uptime.toFixed(2)}%
                          </Typography>
                          <LinearProgress variant="determinate" value={node.uptime} color={node.uptime >= 99 ? 'success' : node.uptime >= 90 ? 'warning' : 'error'} sx={{ mt: 0.5, borderRadius: 999, height: 4 }} />
                        </Box>
                      </TableCell>
                      <TableCell>
                        <Stack direction="row" spacing={1}>
                          <Tooltip title="Available after deployment" arrow>
                            <span><Button size="small" variant="outlined" disabled sx={{ borderRadius: 999, fontSize: '0.7rem' }}>SSH</Button></span>
                          </Tooltip>
                          <Button size="small" variant="outlined" startIcon={<VpnKeyIcon sx={{ fontSize: 12 }} />} sx={{ borderRadius: 999, fontSize: '0.7rem' }}>Rotate</Button>
                        </Stack>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </Paper>
        </>
      )}
    </Box>
  );
}
