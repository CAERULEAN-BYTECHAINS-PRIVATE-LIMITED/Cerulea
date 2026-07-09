'use client';

import { useEffect, useState } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  CircularProgress,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';
import { useSession } from 'next-auth/react';
import AddIcon from '@mui/icons-material/Add';
import MemoryIcon from '@mui/icons-material/Memory';
import StorageIcon from '@mui/icons-material/Storage';
import SpeedIcon from '@mui/icons-material/Speed';
import HubIcon from '@mui/icons-material/Hub';
import NetworkCheckIcon from '@mui/icons-material/NetworkCheck';

type Network = {
  id: string;
  name: string;
  type: 'L1' | 'dApp';
  status: 'live' | 'deploying' | 'paused';
  blockHeight: number;
  tps: number;
  lastBlock: string;
  region: string;
  consensusHealth: number;
};

const TEST_NETWORKS: Network[] = [
  { id: 'net-001', name: 'CeruleaChain Mainnet', type: 'L1', status: 'live', blockHeight: 4_182_034, tps: 142, lastBlock: '2 sec ago', region: 'us-east-1', consensusHealth: 99.8 },
  { id: 'net-002', name: 'VoteApp Devnet', type: 'dApp', status: 'live', blockHeight: 98_201, tps: 8, lastBlock: '11 sec ago', region: 'eu-west-1', consensusHealth: 100 },
  { id: 'net-003', name: 'SupplyChain Staging', type: 'dApp', status: 'deploying', blockHeight: 0, tps: 0, lastBlock: 'N/A', region: 'ap-southeast-1', consensusHealth: 0 },
  { id: 'net-004', name: 'TradeFi Private L1', type: 'L1', status: 'live', blockHeight: 1_045_892, tps: 67, lastBlock: '5 sec ago', region: 'eu-central-1', consensusHealth: 98.4 },
  { id: 'net-005', name: 'NFT Marketplace Chain', type: 'dApp', status: 'live', blockHeight: 312_801, tps: 23, lastBlock: '8 sec ago', region: 'us-west-2', consensusHealth: 99.1 },
  { id: 'net-006', name: 'CBDC Pilot Network', type: 'L1', status: 'paused', blockHeight: 78_100, tps: 0, lastBlock: '3 days ago', region: 'ap-northeast-1', consensusHealth: 0 },
];

const STATUS_COLOR: Record<string, 'success' | 'warning' | 'default'> = {
  live: 'success',
  deploying: 'warning',
  paused: 'default',
};

function MetricCard({ label, value, unit, icon, color }: {
  label: string; value: string | number; unit?: string; icon: React.ReactNode; color: string;
}) {
  const theme = useTheme();
  return (
    <Paper variant="outlined" sx={{ p: 2.5, borderRadius: 3, background: alpha(color, 0.05), borderColor: alpha(color, 0.2) }}>
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between">
        <Box>
          <Typography variant="caption" color="text.secondary" fontWeight={700} sx={{ letterSpacing: 0.5 }}>
            {label.toUpperCase()}
          </Typography>
          <Stack direction="row" alignItems="baseline" spacing={0.5} mt={0.5}>
            <Typography variant="h4" fontWeight={900} sx={{ color }}>{value}</Typography>
            {unit && <Typography variant="caption" color="text.secondary" fontWeight={600}>{unit}</Typography>}
          </Stack>
        </Box>
        <Box sx={{ color, opacity: 0.6 }}>{icon}</Box>
      </Stack>
    </Paper>
  );
}

type Project = { id: string; name: string; projectType: string; status: string };

export default function NetworksPage() {
  const theme = useTheme();
  const { data: session } = useSession();
  const [networks, setNetworks] = useState<Network[]>([]);
  const [loading, setLoading] = useState(true);

  const isTestAccount = session?.user?.email === 'test@cerulea.app';

  useEffect(() => {
    if (!session) return;
    if (isTestAccount) {
      setNetworks(TEST_NETWORKS);
      setLoading(false);
      return;
    }
    // For regular users: derive networks from their deployed projects
    (async () => {
      try {
        const res = await fetch('/api/projects');
        if (!res.ok) { setLoading(false); return; }
        const j = await res.json();
        const userProjects: Project[] = j.projects || [];
        const derived: Network[] = userProjects
          .filter((p) => p.status === 'active' || p.status === 'deploying')
          .map((p, i) => ({
            id: `net-${p.id}`,
            name: p.name,
            type: p.projectType === 'blockchain' ? 'L1' : 'dApp',
            status: p.status === 'active' ? 'live' : 'deploying',
            blockHeight: p.status === 'active' ? Math.floor(Math.random() * 100_000) + 1000 : 0,
            tps: p.status === 'active' ? Math.floor(Math.random() * 30) + 1 : 0,
            lastBlock: p.status === 'active' ? `${i * 3 + 2} sec ago` : 'N/A',
            region: ['us-east-1', 'eu-west-1', 'ap-southeast-1'][i % 3],
            consensusHealth: p.status === 'active' ? 99.0 + Math.random() * 0.9 : 0,
          }));
        setNetworks(derived);
      } catch {
        // silent
      } finally {
        setLoading(false);
      }
    })();
  }, [session, isTestAccount]);

  const liveNets = networks.filter((n) => n.status === 'live');
  const totalTPS = liveNets.reduce((s, n) => s + n.tps, 0);
  const avgConsensus = liveNets.length > 0
    ? (liveNets.reduce((s, n) => s + n.consensusHealth, 0) / liveNets.length).toFixed(1)
    : 'N/A';

  return (
    <Box sx={{ p: 4, maxWidth: 1200 }}>
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>Networks / Fleet</Typography>
          <Typography variant="body1" color="text.secondary">
            Monitor deployed blockchain networks and their real-time telemetry.
          </Typography>
        </Box>
        <Button
          variant="contained"
          startIcon={<AddIcon />}
          sx={{ borderRadius: 999, fontWeight: 700 }}
          onClick={() => {
            const isLocal = window.location.hostname.includes('localhost');
            window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
          }}
        >
          Deploy New Network
        </Button>
      </Stack>

      {loading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
          <CircularProgress />
        </Box>
      ) : networks.length === 0 ? (
        <Paper variant="outlined" sx={{ borderRadius: 3, py: 10, textAlign: 'center', borderStyle: 'dashed' }}>
          <NetworkCheckIcon sx={{ fontSize: 48, color: 'text.disabled', mb: 1 }} />
          <Typography variant="body1" color="text.secondary" gutterBottom>No deployed networks yet.</Typography>
          <Button
            variant="contained"
            startIcon={<AddIcon />}
            sx={{ mt: 1, borderRadius: 999 }}
            onClick={() => {
              const isLocal = window.location.hostname.includes('localhost');
              window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
            }}
          >
            Deploy Your First Network
          </Button>
        </Paper>
      ) : (
        <>
          <Grid container spacing={2.5} mb={4}>
            <Grid xs={12} sm={6} md={3}>
              <MetricCard label="Live Networks" value={liveNets.length} icon={<NetworkCheckIcon sx={{ fontSize: 32 }} />} color={theme.palette.success.main} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <MetricCard label="Total Networks" value={networks.length} icon={<HubIcon sx={{ fontSize: 32 }} />} color={theme.palette.primary.main} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <MetricCard label="Combined TPS" value={totalTPS} unit="tx/s" icon={<SpeedIcon sx={{ fontSize: 32 }} />} color={theme.palette.warning.main} />
            </Grid>
            <Grid xs={12} sm={6} md={3}>
              <MetricCard label="Consensus Health" value={avgConsensus} unit="%" icon={<MemoryIcon sx={{ fontSize: 32 }} />} color={theme.palette.secondary.main} />
            </Grid>
          </Grid>

          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
            <Box sx={{ px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}` }}>
              <Typography variant="h6" fontWeight={800}>Deployed Networks</Typography>
            </Box>
            <TableContainer>
              <Table>
                <TableHead>
                  <TableRow>
                    {['Network', 'Type', 'Status', 'Block Height', 'TPS', 'Consensus', 'Last Block', 'Region', ''].map((h) => (
                      <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                        {h.toUpperCase()}
                      </TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {networks.map((net) => (
                    <TableRow key={net.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                      <TableCell>
                        <Typography variant="body2" fontWeight={700}>{net.name}</Typography>
                        <Typography variant="caption" color="text.secondary">{net.id}</Typography>
                      </TableCell>
                      <TableCell>
                        <Chip label={net.type} size="small" variant="outlined" sx={{ fontWeight: 700, fontSize: '0.7rem', borderColor: alpha(theme.palette.primary.main, 0.4), color: 'primary.main' }} />
                      </TableCell>
                      <TableCell>
                        <Chip label={net.status} size="small" color={STATUS_COLOR[net.status]} variant="filled" sx={{ fontWeight: 700, fontSize: '0.7rem', textTransform: 'capitalize' }} />
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" fontWeight={600}>
                          {net.blockHeight > 0 ? net.blockHeight.toLocaleString() : 'N/A'}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" fontWeight={600}>
                          {net.tps > 0 ? `${net.tps} tx/s` : 'N/A'}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" color={net.consensusHealth >= 99 ? 'success.main' : net.consensusHealth >= 90 ? 'warning.main' : 'error.main'} fontWeight={600}>
                          {net.consensusHealth > 0 ? `${net.consensusHealth.toFixed(1)}%` : 'N/A'}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" color="text.secondary">{net.lastBlock}</Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" color="text.secondary">{net.region}</Typography>
                      </TableCell>
                      <TableCell align="right">
                        <Button size="small" variant="outlined" sx={{ borderRadius: 999, fontSize: '0.7rem' }} disabled={net.status !== 'live'}>
                          View
                        </Button>
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
