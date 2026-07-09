'use client';

import {
  Box, Typography, Paper, Stack, Chip, LinearProgress, Divider,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha, useTheme } from '@mui/material/styles';

import PeopleIcon from '@mui/icons-material/People';
import CorporateFareIcon from '@mui/icons-material/CorporateFare';
import FolderIcon from '@mui/icons-material/Folder';
import RocketLaunchIcon from '@mui/icons-material/RocketLaunch';
import ErrorIcon from '@mui/icons-material/Error';
import ApiIcon from '@mui/icons-material/Api';
import SmartToyIcon from '@mui/icons-material/SmartToy';
import AttachMoneyIcon from '@mui/icons-material/AttachMoney';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import RadioButtonCheckedIcon from '@mui/icons-material/RadioButtonChecked';

type KpiProps = {
  label: string;
  value: string;
  sub: string;
  delta?: string;
  deltaUp?: boolean;
  color: string;
  icon: React.ReactNode;
};

function KpiCard({ label, value, sub, delta, deltaUp, color, icon }: KpiProps) {
  return (
    <Paper
      variant="outlined"
      sx={{
        p: 2.5,
        borderRadius: 3,
        height: '100%',
        background: alpha(color, 0.05),
        borderColor: alpha(color, 0.15),
        transition: 'box-shadow 0.15s',
        '&:hover': { boxShadow: `0 0 0 1px ${alpha(color, 0.3)}` },
      }}
    >
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={1}>
        <Typography
          variant="caption"
          sx={{ color: 'rgba(255,255,255,0.45)', fontWeight: 700, letterSpacing: 0.7, fontSize: '0.67rem' }}
        >
          {label.toUpperCase()}
        </Typography>
        <Box sx={{ color: color, opacity: 0.75 }}>{icon}</Box>
      </Stack>
      <Typography variant="h3" fontWeight={900} sx={{ color, lineHeight: 1.1, fontSize: '1.8rem' }}>
        {value}
      </Typography>
      <Stack direction="row" alignItems="center" spacing={1} mt={0.75}>
        <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.4)' }}>{sub}</Typography>
        {delta && (
          <Chip
            label={delta}
            size="small"
            sx={{
              height: 17,
              fontSize: '0.58rem',
              fontWeight: 800,
              bgcolor: deltaUp ? alpha('#4caf50', 0.12) : alpha('#f44336', 0.12),
              color: deltaUp ? '#4caf50' : '#f44336',
              border: 'none',
            }}
          />
        )}
      </Stack>
    </Paper>
  );
}

const KPI_DATA: KpiProps[] = [
  {
    label: 'Total Users',
    value: '2,841',
    sub: '+18 this week',
    delta: '+0.6%',
    deltaUp: true,
    color: '#448aff',
    icon: <PeopleIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'Active Orgs',
    value: '312',
    sub: '28 enterprise',
    delta: '+4',
    deltaUp: true,
    color: '#ce93d8',
    icon: <CorporateFareIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'Total Projects',
    value: '4,190',
    sub: '1,247 dApp / 2,943 Chain',
    delta: '+31',
    deltaUp: true,
    color: '#80deea',
    icon: <FolderIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'Active Deployments',
    value: '817',
    sub: 'Across all networks',
    delta: '+5',
    deltaUp: true,
    color: '#4caf50',
    icon: <RocketLaunchIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'Failed Deployments',
    value: '14',
    sub: 'Last 24 hours',
    delta: '-3',
    deltaUp: true,
    color: '#f44336',
    icon: <ErrorIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'API Requests Today',
    value: '9.2M',
    sub: 'Avg 106/s',
    delta: '+12%',
    deltaUp: true,
    color: '#ff9800',
    icon: <ApiIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'AI Requests Today',
    value: '142K',
    sub: 'CeruleAI completions',
    delta: '+7%',
    deltaUp: true,
    color: '#ab47bc',
    icon: <SmartToyIcon sx={{ fontSize: 24 }} />,
  },
  {
    label: 'Revenue Today',
    value: '$3,840',
    sub: 'Stripe + subscriptions',
    delta: '+$210',
    deltaUp: true,
    color: '#26a69a',
    icon: <AttachMoneyIcon sx={{ fontSize: 24 }} />,
  },
];

type ServiceStatus = {
  name: string;
  status: 'operational' | 'degraded' | 'down';
  latency?: string;
  uptime?: string;
};

const SERVICES: ServiceStatus[] = [
  { name: 'Auth Service', status: 'operational', latency: '48ms', uptime: '99.98%' },
  { name: 'Studio Frontend', status: 'operational', latency: '120ms', uptime: '99.95%' },
  { name: 'Project API', status: 'operational', latency: '85ms', uptime: '99.97%' },
  { name: 'CeruleAI (Claude)', status: 'operational', latency: '1.2s', uptime: '99.90%' },
  { name: 'Deployment Engine', status: 'degraded', latency: '4.1s', uptime: '98.40%' },
  { name: 'RPC Gateway', status: 'operational', latency: '31ms', uptime: '99.99%' },
  { name: 'Stripe Webhooks', status: 'operational', latency: '210ms', uptime: '100%' },
  { name: 'SQLite Persistence', status: 'operational', latency: '12ms', uptime: '100%' },
  { name: 'Email Service', status: 'operational', latency: '340ms', uptime: '99.80%' },
];

const STATUS_DOT: Record<string, string> = {
  operational: '#4caf50',
  degraded: '#ff9800',
  down: '#f44336',
};

type Alert = {
  id: string;
  severity: 'critical' | 'warning' | 'info';
  message: string;
  time: string;
};

const ALERTS: Alert[] = [
  { id: 'a1', severity: 'critical', message: 'Deployment Engine P95 latency above 4s threshold (currently 4.1s)', time: '12 min ago' },
  { id: 'a2', severity: 'critical', message: '14 failed deployments in the last 24h - up from 3 yesterday', time: '1 hr ago' },
  { id: 'a3', severity: 'critical', message: 'Suspicious login spike from IP 45.152.66.x - 38 attempts', time: '3 hr ago' },
  { id: 'a4', severity: 'warning', message: 'AI request quota at 87% for the month on Anthropic account', time: '6 hr ago' },
  { id: 'a5', severity: 'warning', message: 'User plan downgrade rate increased by 18% this week', time: '1 day ago' },
];

const SEVERITY_COLOR: Record<string, string> = {
  critical: '#f44336',
  warning: '#ff9800',
  info: '#448aff',
};

// Trend proxy using LinearProgress bars
const TREND_BARS = [
  { label: 'Mon', value: 62 },
  { label: 'Tue', value: 74 },
  { label: 'Wed', value: 68 },
  { label: 'Thu', value: 88 },
  { label: 'Fri', value: 95 },
  { label: 'Sat', value: 51 },
  { label: 'Sun', value: 43 },
];

export default function AdminOverviewPage() {
  const theme = useTheme();
  const today = new Date().toLocaleDateString('en-GB', {
    weekday: 'long', year: 'numeric', month: 'long', day: 'numeric',
  });

  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} sx={{ mb: 0.25 }}>
            Platform Overview
          </Typography>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            {today} - Super Admin Command Center
          </Typography>
        </Box>
        <Chip
          label="LIVE"
          size="small"
          icon={<RadioButtonCheckedIcon sx={{ fontSize: '0.65rem !important', animation: 'pulse 2s infinite' }} />}
          sx={{
            bgcolor: alpha('#4caf50', 0.12),
            color: '#4caf50',
            border: `1px solid ${alpha('#4caf50', 0.25)}`,
            fontWeight: 800,
            fontSize: '0.65rem',
            height: 24,
            '& .MuiChip-icon': { color: '#4caf50 !important' },
          }}
        />
      </Stack>

      {/* KPI Cards */}
      <Grid container spacing={2} mb={4}>
        {KPI_DATA.map((kpi) => (
          <Grid key={kpi.label} xs={12} sm={6} md={3}>
            <KpiCard {...kpi} />
          </Grid>
        ))}
      </Grid>

      <Grid container spacing={3}>
        {/* Platform Health */}
        <Grid xs={12} md={5}>
          <Paper
            variant="outlined"
            sx={{ borderRadius: 3, overflow: 'hidden', height: '100%', borderColor: 'rgba(255,255,255,0.07)' }}
          >
            <Stack
              direction="row"
              alignItems="center"
              justifyContent="space-between"
              sx={{ px: 3, py: 2, borderBottom: '1px solid rgba(255,255,255,0.07)' }}
            >
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem' }}>
                Platform Health
              </Typography>
              <Chip
                label="8 / 9 Operational"
                size="small"
                sx={{
                  bgcolor: alpha('#4caf50', 0.1),
                  color: '#4caf50',
                  fontWeight: 700,
                  fontSize: '0.65rem',
                  height: 20,
                  border: 'none',
                }}
              />
            </Stack>
            <Stack divider={<Divider sx={{ borderColor: 'rgba(255,255,255,0.05)' }} />}>
              {SERVICES.map((svc) => (
                <Stack
                  key={svc.name}
                  direction="row"
                  alignItems="center"
                  sx={{ px: 3, py: 1.25 }}
                  spacing={1.5}
                >
                  <Box
                    sx={{
                      width: 8,
                      height: 8,
                      borderRadius: '50%',
                      bgcolor: STATUS_DOT[svc.status],
                      flexShrink: 0,
                      boxShadow: `0 0 6px ${STATUS_DOT[svc.status]}`,
                    }}
                  />
                  <Typography variant="body2" sx={{ flex: 1, fontSize: '0.82rem', fontWeight: 500 }}>
                    {svc.name}
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.7rem' }}>
                    {svc.latency}
                  </Typography>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.7rem', minWidth: 44, textAlign: 'right' }}>
                    {svc.uptime}
                  </Typography>
                  {svc.status !== 'operational' && (
                    <Chip
                      label={svc.status.toUpperCase()}
                      size="small"
                      sx={{
                        height: 16,
                        fontSize: '0.55rem',
                        fontWeight: 800,
                        bgcolor: alpha(STATUS_DOT[svc.status], 0.12),
                        color: STATUS_DOT[svc.status],
                        border: 'none',
                      }}
                    />
                  )}
                </Stack>
              ))}
            </Stack>
          </Paper>
        </Grid>

        {/* Critical Alerts */}
        <Grid xs={12} md={4}>
          <Paper
            variant="outlined"
            sx={{ borderRadius: 3, overflow: 'hidden', borderColor: alpha('#f44336', 0.2) }}
          >
            <Stack
              direction="row"
              alignItems="center"
              spacing={1}
              sx={{ px: 3, py: 2, borderBottom: `1px solid ${alpha('#f44336', 0.15)}` }}
            >
              <WarningAmberIcon sx={{ fontSize: 16, color: '#f44336' }} />
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem', color: '#f44336' }}>
                Critical Alerts
              </Typography>
              <Box sx={{ flex: 1 }} />
              <Chip
                label={`${ALERTS.filter((a) => a.severity === 'critical').length} critical`}
                size="small"
                sx={{
                  bgcolor: alpha('#f44336', 0.1),
                  color: '#f44336',
                  fontWeight: 800,
                  fontSize: '0.6rem',
                  height: 18,
                  border: 'none',
                }}
              />
            </Stack>
            <Stack divider={<Divider sx={{ borderColor: 'rgba(255,255,255,0.05)' }} />}>
              {ALERTS.map((alert) => (
                <Stack key={alert.id} direction="row" alignItems="flex-start" spacing={1.5} sx={{ px: 3, py: 1.5 }}>
                  <Box
                    sx={{
                      width: 7,
                      height: 7,
                      borderRadius: '50%',
                      bgcolor: SEVERITY_COLOR[alert.severity],
                      mt: 0.65,
                      flexShrink: 0,
                    }}
                  />
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Typography variant="body2" sx={{ fontSize: '0.79rem', lineHeight: 1.4, color: 'rgba(255,255,255,0.8)' }}>
                      {alert.message}
                    </Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.67rem' }}>
                      {alert.time}
                    </Typography>
                  </Box>
                </Stack>
              ))}
            </Stack>
          </Paper>
        </Grid>

        {/* Trend proxy */}
        <Grid xs={12} md={3}>
          <Paper
            variant="outlined"
            sx={{ borderRadius: 3, overflow: 'hidden', height: '100%', borderColor: 'rgba(255,255,255,0.07)' }}
          >
            <Stack sx={{ px: 3, py: 2, borderBottom: '1px solid rgba(255,255,255,0.07)' }}>
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem' }}>
                API Requests (7d)
              </Typography>
              <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.35)' }}>
                Relative volume by day
              </Typography>
            </Stack>
            <Stack spacing={1.5} sx={{ px: 3, py: 2.5 }}>
              {TREND_BARS.map((bar) => (
                <Stack key={bar.label} direction="row" alignItems="center" spacing={1.5}>
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.4)', minWidth: 28, fontSize: '0.7rem' }}>
                    {bar.label}
                  </Typography>
                  <LinearProgress
                    variant="determinate"
                    value={bar.value}
                    sx={{
                      flex: 1,
                      height: 8,
                      borderRadius: 4,
                      bgcolor: 'rgba(255,255,255,0.06)',
                      '& .MuiLinearProgress-bar': {
                        bgcolor: bar.value >= 80 ? '#4caf50' : bar.value >= 60 ? '#448aff' : 'rgba(255,255,255,0.25)',
                        borderRadius: 4,
                      },
                    }}
                  />
                  <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.4)', minWidth: 28, textAlign: 'right', fontSize: '0.7rem' }}>
                    {bar.value}%
                  </Typography>
                </Stack>
              ))}
            </Stack>
          </Paper>
        </Grid>
      </Grid>
    </Box>
  );
}
