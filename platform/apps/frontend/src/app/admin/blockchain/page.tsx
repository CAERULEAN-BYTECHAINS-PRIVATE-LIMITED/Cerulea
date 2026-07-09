'use client';

import {
  Box, Typography, Paper, Stack, Chip, Divider, LinearProgress, Tooltip,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha } from '@mui/material/styles';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import SpeedIcon from '@mui/icons-material/Speed';
import StorageIcon from '@mui/icons-material/Storage';
import VerifiedIcon from '@mui/icons-material/Verified';
import AccessTimeIcon from '@mui/icons-material/AccessTime';

type Chain = {
  id: string;
  name: string;
  layer: 'L1' | 'L2' | 'dApp';
  networkType: 'mainnet' | 'testnet' | 'devnet';
  health: 'healthy' | 'degraded' | 'critical';
  blockHeight: number;
  tps: number;
  targetTps: number;
  consensusHealth: number;
  validators: number;
  activeValidators: number;
  uptimePct: number;
  gasPrice: string;
  latestBlock: string;
  chainId: number;
};

const CHAINS: Chain[] = [
  {
    id: 'chain-001',
    name: 'CeruleaChain Mainnet',
    layer: 'L1',
    networkType: 'mainnet',
    health: 'healthy',
    blockHeight: 4_182_034,
    tps: 1_240,
    targetTps: 2_000,
    consensusHealth: 98,
    validators: 121,
    activeValidators: 118,
    uptimePct: 99.98,
    gasPrice: '0.0001 CRL',
    latestBlock: '2026-04-17T09:14:40Z',
    chainId: 1337,
  },
  {
    id: 'chain-002',
    name: 'QuantumLedger L2',
    layer: 'L2',
    networkType: 'mainnet',
    health: 'healthy',
    blockHeight: 1_029_405,
    tps: 4_800,
    targetTps: 6_000,
    consensusHealth: 96,
    validators: 30,
    activeValidators: 30,
    uptimePct: 99.95,
    gasPrice: '0.000005 ETH',
    latestBlock: '2026-04-17T09:14:38Z',
    chainId: 42161,
  },
  {
    id: 'chain-003',
    name: 'LATAMChain',
    layer: 'L1',
    networkType: 'mainnet',
    health: 'degraded',
    blockHeight: 834_210,
    tps: 182,
    targetTps: 500,
    consensusHealth: 74,
    validators: 15,
    activeValidators: 11,
    uptimePct: 97.40,
    gasPrice: '0.002 LATM',
    latestBlock: '2026-04-17T09:12:01Z',
    chainId: 9900,
  },
  {
    id: 'chain-004',
    name: 'OpenChain Testnet',
    layer: 'L1',
    networkType: 'testnet',
    health: 'healthy',
    blockHeight: 221_340,
    tps: 88,
    targetTps: 200,
    consensusHealth: 100,
    validators: 5,
    activeValidators: 5,
    uptimePct: 100,
    gasPrice: '0 (faucet)',
    latestBlock: '2026-04-17T09:14:35Z',
    chainId: 31337,
  },
  {
    id: 'chain-005',
    name: 'ChainBridge Devnet',
    layer: 'L2',
    networkType: 'devnet',
    health: 'healthy',
    blockHeight: 12_448,
    tps: 22,
    targetTps: 100,
    consensusHealth: 100,
    validators: 3,
    activeValidators: 3,
    uptimePct: 100,
    gasPrice: '0 (dev)',
    latestBlock: '2026-04-17T09:14:10Z',
    chainId: 99999,
  },
  {
    id: 'chain-006',
    name: 'VoteDAO dApp Chain',
    layer: 'dApp',
    networkType: 'mainnet',
    health: 'healthy',
    blockHeight: 98_210,
    tps: 14,
    targetTps: 50,
    consensusHealth: 99,
    validators: 7,
    activeValidators: 7,
    uptimePct: 99.90,
    gasPrice: '0.0003 VOTE',
    latestBlock: '2026-04-17T09:14:31Z',
    chainId: 8888,
  },
];

const HEALTH_COLOR: Record<string, string> = {
  healthy: '#4caf50',
  degraded: '#ff9800',
  critical: '#f44336',
};

const NET_TYPE_COLOR: Record<string, { bg: string; text: string }> = {
  mainnet: { bg: alpha('#4caf50', 0.1), text: '#4caf50' },
  testnet: { bg: alpha('#ff9800', 0.1), text: '#ff9800' },
  devnet: { bg: alpha('#448aff', 0.1), text: '#448aff' },
};

const LAYER_COLOR: Record<string, { bg: string; text: string }> = {
  L1: { bg: alpha('#ce93d8', 0.1), text: '#ce93d8' },
  L2: { bg: alpha('#80deea', 0.1), text: '#80deea' },
  dApp: { bg: alpha('#ffd54f', 0.1), text: '#ffd54f' },
};

function HealthRow({ label, value, max, unit, color }: { label: string; value: number; max: number; unit?: string; color: string }) {
  const pct = Math.min((value / max) * 100, 100);
  return (
    <Stack direction="row" alignItems="center" spacing={1} sx={{ mb: 0.75 }}>
      <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.4)', minWidth: 90, fontSize: '0.72rem' }}>
        {label}
      </Typography>
      <LinearProgress
        variant="determinate"
        value={pct}
        sx={{
          flex: 1,
          height: 6,
          borderRadius: 3,
          bgcolor: 'rgba(255,255,255,0.06)',
          '& .MuiLinearProgress-bar': { bgcolor: color, borderRadius: 3 },
        }}
      />
      <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.55)', minWidth: 54, textAlign: 'right', fontSize: '0.72rem', fontFamily: 'monospace' }}>
        {value.toLocaleString()}{unit || ''}
      </Typography>
    </Stack>
  );
}

export default function AdminBlockchainPage() {
  const healthyCount = CHAINS.filter((c) => c.health === 'healthy').length;
  const degradedCount = CHAINS.filter((c) => c.health === 'degraded').length;

  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
            <HexagonOutlinedIcon sx={{ color: '#ce93d8', fontSize: 22 }} />
            <Typography variant="h4" fontWeight={900}>Blockchain Operations</Typography>
          </Stack>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            Real-time health and metrics for all {CHAINS.length} managed networks
          </Typography>
        </Box>
        <Stack direction="row" spacing={1.5}>
          <Chip label={`${healthyCount} healthy`} size="small" sx={{ bgcolor: alpha('#4caf50', 0.1), color: '#4caf50', fontWeight: 700, height: 24 }} />
          {degradedCount > 0 && (
            <Chip label={`${degradedCount} degraded`} size="small" sx={{ bgcolor: alpha('#ff9800', 0.1), color: '#ff9800', fontWeight: 700, height: 24 }} />
          )}
        </Stack>
      </Stack>

      {/* Chain cards */}
      <Grid container spacing={2.5}>
        {CHAINS.map((chain) => (
          <Grid key={chain.id} xs={12} md={6}>
            <Paper
              variant="outlined"
              sx={{
                borderRadius: 3,
                overflow: 'hidden',
                borderColor: chain.health === 'healthy'
                  ? 'rgba(255,255,255,0.07)'
                  : alpha(HEALTH_COLOR[chain.health], 0.3),
                height: '100%',
              }}
            >
              {/* Chain header */}
              <Stack
                direction="row"
                alignItems="center"
                spacing={1.5}
                sx={{
                  px: 3,
                  py: 2,
                  borderBottom: '1px solid rgba(255,255,255,0.07)',
                  bgcolor: chain.health !== 'healthy' ? alpha(HEALTH_COLOR[chain.health], 0.05) : 'transparent',
                }}
              >
                <Box
                  sx={{
                    width: 10,
                    height: 10,
                    borderRadius: '50%',
                    bgcolor: HEALTH_COLOR[chain.health],
                    flexShrink: 0,
                    boxShadow: `0 0 8px ${HEALTH_COLOR[chain.health]}`,
                  }}
                />
                <Box sx={{ flex: 1, minWidth: 0 }}>
                  <Typography variant="subtitle2" fontWeight={800} sx={{ lineHeight: 1.2 }}>
                    {chain.name}
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.35)', fontFamily: 'monospace', fontSize: '0.67rem' }}>
                    Chain ID {chain.chainId}
                  </Typography>
                </Box>
                <Chip
                  label={chain.layer}
                  size="small"
                  sx={{
                    height: 19,
                    fontSize: '0.62rem',
                    fontWeight: 700,
                    bgcolor: LAYER_COLOR[chain.layer].bg,
                    color: LAYER_COLOR[chain.layer].text,
                    border: 'none',
                  }}
                />
                <Chip
                  label={chain.networkType}
                  size="small"
                  sx={{
                    height: 19,
                    fontSize: '0.62rem',
                    fontWeight: 700,
                    bgcolor: NET_TYPE_COLOR[chain.networkType].bg,
                    color: NET_TYPE_COLOR[chain.networkType].text,
                    border: 'none',
                    textTransform: 'capitalize',
                  }}
                />
              </Stack>

              {/* Stats grid */}
              <Box sx={{ px: 3, py: 2 }}>
                <Grid container spacing={1.5} mb={2}>
                  {[
                    { icon: <StorageIcon sx={{ fontSize: 14 }} />, label: 'Block Height', value: chain.blockHeight.toLocaleString() },
                    { icon: <SpeedIcon sx={{ fontSize: 14 }} />, label: 'Current TPS', value: chain.tps.toLocaleString() },
                    { icon: <VerifiedIcon sx={{ fontSize: 14 }} />, label: 'Validators', value: `${chain.activeValidators} / ${chain.validators}` },
                    { icon: <AccessTimeIcon sx={{ fontSize: 14 }} />, label: 'Uptime', value: `${chain.uptimePct}%` },
                  ].map((stat) => (
                    <Grid key={stat.label} xs={6}>
                      <Paper
                        variant="outlined"
                        sx={{
                          px: 1.5, py: 1.25, borderRadius: 2,
                          borderColor: 'rgba(255,255,255,0.06)',
                          bgcolor: 'rgba(255,255,255,0.02)',
                        }}
                      >
                        <Stack direction="row" alignItems="center" spacing={0.75} mb={0.25}>
                          <Box sx={{ color: 'rgba(255,255,255,0.35)' }}>{stat.icon}</Box>
                          <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.35)', fontSize: '0.65rem', fontWeight: 600 }}>
                            {stat.label}
                          </Typography>
                        </Stack>
                        <Typography variant="body2" fontWeight={800} sx={{ fontFamily: 'monospace', fontSize: '0.85rem' }}>
                          {stat.value}
                        </Typography>
                      </Paper>
                    </Grid>
                  ))}
                </Grid>

                {/* Progress bars */}
                <HealthRow
                  label="TPS Load"
                  value={chain.tps}
                  max={chain.targetTps}
                  color={chain.tps / chain.targetTps > 0.9 ? '#f44336' : chain.tps / chain.targetTps > 0.7 ? '#ff9800' : '#4caf50'}
                />
                <HealthRow
                  label="Consensus"
                  value={chain.consensusHealth}
                  max={100}
                  unit="%"
                  color={chain.consensusHealth < 80 ? '#f44336' : chain.consensusHealth < 90 ? '#ff9800' : '#4caf50'}
                />
                <HealthRow
                  label="Validators"
                  value={chain.activeValidators}
                  max={chain.validators}
                  color={chain.activeValidators / chain.validators < 0.8 ? '#f44336' : '#4caf50'}
                />

                <Divider sx={{ my: 1.5, borderColor: 'rgba(255,255,255,0.05)' }} />

                <Stack direction="row" justifyContent="space-between" alignItems="center">
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.68rem' }}>
                    Gas: {chain.gasPrice}
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.68rem', fontFamily: 'monospace' }}>
                    Last block:{' '}
                    {new Date(chain.latestBlock).toLocaleTimeString('en-GB', { hour: '2-digit', minute: '2-digit', second: '2-digit' })}
                  </Typography>
                </Stack>
              </Box>
            </Paper>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
