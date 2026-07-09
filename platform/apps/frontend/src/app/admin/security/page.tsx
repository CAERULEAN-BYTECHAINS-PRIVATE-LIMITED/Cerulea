'use client';

import {
  Box, Typography, Paper, Stack, Chip, Divider, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  IconButton, Tooltip,
} from '@mui/material';
import Grid from '@mui/material/GridLegacy';
import { alpha } from '@mui/material/styles';
import ShieldIcon from '@mui/icons-material/Shield';
import BlockIcon from '@mui/icons-material/Block';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import ErrorOutlineIcon from '@mui/icons-material/ErrorOutline';
import GppMaybeIcon from '@mui/icons-material/GppMaybe';
import LockIcon from '@mui/icons-material/Lock';

type FailedLogin = {
  id: string;
  email: string;
  ip: string;
  country: string;
  attempts: number;
  lastAttempt: string;
  status: 'blocked' | 'monitoring' | 'cleared';
};

type SuspiciousEvent = {
  id: string;
  type: string;
  actor: string;
  detail: string;
  severity: 'high' | 'medium' | 'low';
  timestamp: string;
};

type BlockedIp = {
  id: string;
  ip: string;
  reason: string;
  country: string;
  blockedAt: string;
  expiresAt: string;
};

type SecurityAlert = {
  id: string;
  title: string;
  message: string;
  severity: 'critical' | 'high' | 'medium';
  timestamp: string;
  acknowledged: boolean;
};

const FAILED_LOGINS: FailedLogin[] = [
  { id: 'fl-001', email: 'admin@cerulea.app', ip: '45.152.66.14', country: 'RU', attempts: 38, lastAttempt: '2026-04-17T09:00:00Z', status: 'blocked' },
  { id: 'fl-002', email: 'test@cerulea.app', ip: '194.165.16.77', country: 'CN', attempts: 12, lastAttempt: '2026-04-17T08:30:00Z', status: 'blocked' },
  { id: 'fl-003', email: 'priya.k@blocktech.io', ip: '77.88.55.99', country: 'TR', attempts: 5, lastAttempt: '2026-04-16T22:15:00Z', status: 'monitoring' },
  { id: 'fl-004', email: 'marcus.webb@nullspace.io', ip: '185.220.101.8', country: 'DE', attempts: 3, lastAttempt: '2026-04-16T18:00:00Z', status: 'cleared' },
  { id: 'fl-005', email: 'dev@0xshield.tech', ip: '103.42.168.10', country: 'IN', attempts: 7, lastAttempt: '2026-04-16T14:20:00Z', status: 'monitoring' },
];

const SUSPICIOUS_EVENTS: SuspiciousEvent[] = [
  {
    id: 'se-001',
    type: 'GEO_ANOMALY',
    actor: 'dev@0xshield.tech',
    detail: 'Login from 3 different countries in under 2 hours (US, SG, IN)',
    severity: 'high',
    timestamp: '2026-04-17T08:44:00Z',
  },
  {
    id: 'se-002',
    type: 'API_ABUSE',
    actor: 'jordan@defi-stack.com',
    detail: 'API key used 94,000 times in 1 hour - 47x above normal',
    severity: 'high',
    timestamp: '2026-04-17T07:30:00Z',
  },
  {
    id: 'se-003',
    type: 'PRIVILEGE_ESCALATION',
    actor: 'marcus.webb@nullspace.io',
    detail: 'Attempted to access admin API endpoints 12 times',
    severity: 'high',
    timestamp: '2026-04-16T22:10:00Z',
  },
  {
    id: 'se-004',
    type: 'DATA_EXFIL',
    actor: 'tobias@web3labs.de',
    detail: 'Unusual bulk project export - 2.4GB of project data downloaded',
    severity: 'medium',
    timestamp: '2026-04-16T18:30:00Z',
  },
  {
    id: 'se-005',
    type: 'TOKEN_REUSE',
    actor: 'unknown',
    detail: 'Revoked API token presented 3 times from IP 204.16.200.x',
    severity: 'medium',
    timestamp: '2026-04-16T15:00:00Z',
  },
  {
    id: 'se-006',
    type: 'RATE_LIMIT_BYPASS',
    actor: 'sofia@nomadprotocol.xyz',
    detail: 'Possible rate limit bypass detected via rotating proxies',
    severity: 'low',
    timestamp: '2026-04-16T12:00:00Z',
  },
];

const BLOCKED_IPS: BlockedIp[] = [
  { id: 'ip-001', ip: '45.152.66.14', reason: 'Brute force login (38 attempts)', country: 'RU', blockedAt: '2026-04-17T09:01:00Z', expiresAt: '2026-04-24T09:01:00Z' },
  { id: 'ip-002', ip: '194.165.16.77', reason: 'Brute force login (12 attempts)', country: 'CN', blockedAt: '2026-04-17T08:32:00Z', expiresAt: '2026-04-24T08:32:00Z' },
  { id: 'ip-003', ip: '185.220.101.8', reason: 'Known Tor exit node', country: 'DE', blockedAt: '2026-04-15T00:00:00Z', expiresAt: 'Permanent' },
  { id: 'ip-004', ip: '91.108.4.x/24', reason: 'Telegram data center - spam range', country: 'NL', blockedAt: '2026-04-14T10:00:00Z', expiresAt: 'Permanent' },
  { id: 'ip-005', ip: '103.42.168.10', reason: 'Suspicious auth pattern (7 failed)', country: 'IN', blockedAt: '2026-04-16T14:22:00Z', expiresAt: '2026-04-23T14:22:00Z' },
];

const ALERTS: SecurityAlert[] = [
  {
    id: 'sa-001',
    title: 'Brute Force Attack Detected',
    message: '38 failed login attempts from 45.152.66.14 (RU) targeting admin account. IP blocked automatically.',
    severity: 'critical',
    timestamp: '2026-04-17T09:01:00Z',
    acknowledged: false,
  },
  {
    id: 'sa-002',
    title: 'Admin API Access Attempted',
    message: 'Suspended user marcus.webb@nullspace.io attempted to call /api/admin/* endpoints 12 times.',
    severity: 'high',
    timestamp: '2026-04-16T22:10:00Z',
    acknowledged: false,
  },
  {
    id: 'sa-003',
    title: 'AI Quota Near Limit',
    message: 'Anthropic API usage at 87% of monthly quota. Risk of service degradation if not addressed.',
    severity: 'high',
    timestamp: '2026-04-16T06:00:00Z',
    acknowledged: true,
  },
];

const SEVERITY_COLOR: Record<string, string> = {
  critical: '#f44336',
  high: '#ff9800',
  medium: '#ffd54f',
  low: 'rgba(255,255,255,0.45)',
};

const STATUS_COLOR: Record<string, string> = {
  blocked: '#f44336',
  monitoring: '#ff9800',
  cleared: '#4caf50',
};

export default function AdminSecurityPage() {
  return (
    <Box sx={{ maxWidth: 1400 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
            <ShieldIcon sx={{ color: '#f44336', fontSize: 22 }} />
            <Typography variant="h4" fontWeight={900}>Security Center</Typography>
          </Stack>
          <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.4)' }}>
            Platform-wide security monitoring, threat detection, and access controls
          </Typography>
        </Box>
        <Stack direction="row" spacing={1.5}>
          <Chip
            label={`${ALERTS.filter((a) => !a.acknowledged).length} unacknowledged`}
            size="small"
            sx={{ bgcolor: alpha('#f44336', 0.1), color: '#f44336', fontWeight: 700, height: 24 }}
          />
        </Stack>
      </Stack>

      <Grid container spacing={3}>
        {/* Alerts */}
        <Grid xs={12}>
          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: alpha('#f44336', 0.2) }}>
            <Stack
              direction="row"
              alignItems="center"
              spacing={1}
              sx={{ px: 3, py: 2, borderBottom: `1px solid ${alpha('#f44336', 0.15)}` }}
            >
              <GppMaybeIcon sx={{ fontSize: 16, color: '#f44336' }} />
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem', color: '#f44336' }}>
                Active Security Alerts
              </Typography>
            </Stack>
            <Stack divider={<Divider sx={{ borderColor: 'rgba(255,255,255,0.05)' }} />}>
              {ALERTS.map((alert) => (
                <Stack key={alert.id} direction="row" alignItems="flex-start" sx={{ px: 3, py: 2 }} spacing={2}>
                  <Box
                    sx={{
                      width: 8,
                      height: 8,
                      borderRadius: '50%',
                      bgcolor: SEVERITY_COLOR[alert.severity],
                      mt: 0.7,
                      flexShrink: 0,
                      ...(alert.acknowledged ? {} : { boxShadow: `0 0 6px ${SEVERITY_COLOR[alert.severity]}` }),
                    }}
                  />
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Stack direction="row" alignItems="center" spacing={1} mb={0.25}>
                      <Typography variant="body2" fontWeight={700} sx={{ fontSize: '0.85rem' }}>
                        {alert.title}
                      </Typography>
                      <Chip
                        label={alert.severity.toUpperCase()}
                        size="small"
                        sx={{
                          height: 16,
                          fontSize: '0.55rem',
                          fontWeight: 800,
                          bgcolor: alpha(SEVERITY_COLOR[alert.severity], 0.12),
                          color: SEVERITY_COLOR[alert.severity],
                          border: 'none',
                        }}
                      />
                      {alert.acknowledged && (
                        <Chip label="ACK" size="small" sx={{ height: 16, fontSize: '0.55rem', fontWeight: 800, bgcolor: 'rgba(255,255,255,0.06)', color: 'rgba(255,255,255,0.35)', border: 'none' }} />
                      )}
                    </Stack>
                    <Typography variant="body2" sx={{ color: 'rgba(255,255,255,0.55)', fontSize: '0.8rem' }}>
                      {alert.message}
                    </Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.67rem' }}>
                      {new Date(alert.timestamp).toLocaleString('en-GB', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })}
                    </Typography>
                  </Box>
                  <Button size="small" variant="outlined" sx={{ borderRadius: 999, fontSize: '0.7rem', flexShrink: 0 }}>
                    {alert.acknowledged ? 'Dismiss' : 'Acknowledge'}
                  </Button>
                </Stack>
              ))}
            </Stack>
          </Paper>
        </Grid>

        {/* Failed Logins */}
        <Grid xs={12} md={7}>
          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
            <Stack
              direction="row"
              alignItems="center"
              spacing={1}
              sx={{ px: 3, py: 2, borderBottom: '1px solid rgba(255,255,255,0.07)' }}
            >
              <LockIcon sx={{ fontSize: 15, color: 'rgba(255,255,255,0.4)' }} />
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem' }}>
                Recent Failed Logins
              </Typography>
            </Stack>
            <TableContainer>
              <Table size="small">
                <TableHead>
                  <TableRow sx={{ bgcolor: 'rgba(255,255,255,0.02)' }}>
                    {['Email', 'IP Address', 'Country', 'Attempts', 'Last Attempt', 'Status'].map((h) => (
                      <TableCell key={h} sx={{ fontWeight: 700, color: 'rgba(255,255,255,0.45)', fontSize: '0.65rem', letterSpacing: 0.7 }}>
                        {h.toUpperCase()}
                      </TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {FAILED_LOGINS.map((fl) => (
                    <TableRow key={fl.id} sx={{ '&:hover': { bgcolor: 'rgba(255,255,255,0.02)' } }}>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontSize: '0.78rem' }}>{fl.email}</Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.78rem' }}>{fl.ip}</Typography>
                      </TableCell>
                      <TableCell>
                        <Chip
                          label={fl.country}
                          size="small"
                          sx={{ height: 18, fontSize: '0.62rem', fontWeight: 700, bgcolor: 'rgba(255,255,255,0.05)', color: 'rgba(255,255,255,0.55)', border: 'none' }}
                        />
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" fontWeight={700} sx={{ fontSize: '0.8rem', color: fl.attempts >= 10 ? '#f44336' : '#ff9800' }}>
                          {fl.attempts}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" sx={{ fontFamily: 'monospace', color: 'rgba(255,255,255,0.4)', fontSize: '0.7rem' }}>
                          {new Date(fl.lastAttempt).toLocaleString('en-GB', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Stack direction="row" alignItems="center" spacing={0.75}>
                          <Box sx={{ width: 6, height: 6, borderRadius: '50%', bgcolor: STATUS_COLOR[fl.status] }} />
                          <Typography variant="caption" sx={{ fontSize: '0.72rem', color: STATUS_COLOR[fl.status], fontWeight: 600, textTransform: 'capitalize' }}>
                            {fl.status}
                          </Typography>
                        </Stack>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </Paper>
        </Grid>

        {/* Blocked IPs */}
        <Grid xs={12} md={5}>
          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
            <Stack
              direction="row"
              alignItems="center"
              spacing={1}
              sx={{ px: 3, py: 2, borderBottom: '1px solid rgba(255,255,255,0.07)' }}
            >
              <BlockIcon sx={{ fontSize: 15, color: '#f44336' }} />
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem' }}>
                Blocked IPs
              </Typography>
              <Box sx={{ flex: 1 }} />
              <Chip label={`${BLOCKED_IPS.length} blocked`} size="small" sx={{ bgcolor: alpha('#f44336', 0.1), color: '#f44336', fontWeight: 700, height: 18, fontSize: '0.62rem' }} />
            </Stack>
            <Stack divider={<Divider sx={{ borderColor: 'rgba(255,255,255,0.05)' }} />}>
              {BLOCKED_IPS.map((ip) => (
                <Stack key={ip.id} direction="row" alignItems="flex-start" sx={{ px: 3, py: 1.5 }} spacing={1.5}>
                  <Box sx={{ flex: 1, minWidth: 0 }}>
                    <Stack direction="row" alignItems="center" spacing={1} mb={0.25}>
                      <Typography variant="body2" fontWeight={700} sx={{ fontFamily: 'monospace', fontSize: '0.8rem' }}>
                        {ip.ip}
                      </Typography>
                      <Chip label={ip.country} size="small" sx={{ height: 16, fontSize: '0.58rem', fontWeight: 700, bgcolor: 'rgba(255,255,255,0.05)', color: 'rgba(255,255,255,0.45)', border: 'none' }} />
                    </Stack>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.45)', fontSize: '0.72rem' }}>
                      {ip.reason}
                    </Typography>
                    <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.3)', fontSize: '0.67rem', display: 'block' }}>
                      Expires: {ip.expiresAt === 'Permanent' ? 'Permanent' : new Date(ip.expiresAt).toLocaleDateString('en-GB', { day: '2-digit', month: 'short', year: 'numeric' })}
                    </Typography>
                  </Box>
                  <Tooltip title="Unblock IP">
                    <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.3)', '&:hover': { color: '#4caf50', bgcolor: alpha('#4caf50', 0.1) } }}>
                      <BlockIcon sx={{ fontSize: 14 }} />
                    </IconButton>
                  </Tooltip>
                </Stack>
              ))}
            </Stack>
          </Paper>
        </Grid>

        {/* Suspicious Activity Feed */}
        <Grid xs={12}>
          <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden', borderColor: 'rgba(255,255,255,0.07)' }}>
            <Stack
              direction="row"
              alignItems="center"
              spacing={1}
              sx={{ px: 3, py: 2, borderBottom: '1px solid rgba(255,255,255,0.07)' }}
            >
              <WarningAmberIcon sx={{ fontSize: 15, color: '#ff9800' }} />
              <Typography variant="h6" fontWeight={800} sx={{ fontSize: '0.95rem' }}>
                Suspicious Activity Feed
              </Typography>
            </Stack>
            <TableContainer>
              <Table size="small">
                <TableHead>
                  <TableRow sx={{ bgcolor: 'rgba(255,255,255,0.02)' }}>
                    {['Severity', 'Type', 'Actor', 'Detail', 'Timestamp'].map((h) => (
                      <TableCell key={h} sx={{ fontWeight: 700, color: 'rgba(255,255,255,0.45)', fontSize: '0.65rem', letterSpacing: 0.7 }}>
                        {h.toUpperCase()}
                      </TableCell>
                    ))}
                  </TableRow>
                </TableHead>
                <TableBody>
                  {SUSPICIOUS_EVENTS.map((ev) => (
                    <TableRow key={ev.id} sx={{ '&:hover': { bgcolor: 'rgba(255,255,255,0.02)' } }}>
                      <TableCell>
                        <Stack direction="row" alignItems="center" spacing={0.75}>
                          <Box sx={{ width: 7, height: 7, borderRadius: '50%', bgcolor: SEVERITY_COLOR[ev.severity] }} />
                          <Typography variant="caption" sx={{ fontSize: '0.7rem', color: SEVERITY_COLOR[ev.severity], fontWeight: 700, textTransform: 'capitalize' }}>
                            {ev.severity}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontFamily: 'monospace', fontSize: '0.75rem', fontWeight: 700 }}>
                          {ev.type}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" sx={{ fontSize: '0.78rem' }}>{ev.actor}</Typography>
                      </TableCell>
                      <TableCell sx={{ maxWidth: 420 }}>
                        <Typography variant="body2" sx={{ fontSize: '0.78rem', color: 'rgba(255,255,255,0.6)' }}>
                          {ev.detail}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" sx={{ fontFamily: 'monospace', color: 'rgba(255,255,255,0.4)', fontSize: '0.7rem' }}>
                          {new Date(ev.timestamp).toLocaleString('en-GB', { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' })}
                        </Typography>
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </TableContainer>
          </Paper>
        </Grid>
      </Grid>
    </Box>
  );
}
