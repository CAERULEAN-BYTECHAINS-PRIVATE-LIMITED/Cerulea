'use client';

import { useState, useEffect } from 'react';
import {
  Box, Typography, Paper, Stack, Chip, Button,
  Table, TableBody, TableCell, TableContainer, TableHead, TableRow,
  Tabs, Tab, LinearProgress, Divider, CircularProgress,
} from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import HowToVoteIcon from '@mui/icons-material/HowToVote';
import LockIcon from '@mui/icons-material/Lock';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import CancelIcon from '@mui/icons-material/Cancel';
import PendingIcon from '@mui/icons-material/Pending';
import AccountBalanceIcon from '@mui/icons-material/AccountBalance';
import RocketLaunchIcon from '@mui/icons-material/RocketLaunch';
import Link from 'next/link';

type Proposal = {
  id: string;
  title: string;
  description: string;
  votesFor: number;
  votesAgainst: number;
  deadline: string;
  status: 'active' | 'passed' | 'rejected' | 'pending';
};

type MultiSigTx = {
  id: string;
  to: string;
  value: string;
  description: string;
  threshold: number;
  approvals: number;
  status: 'pending' | 'executed' | 'cancelled';
};

type HistoryEntry = {
  id: string;
  proposal: string;
  vote: 'For' | 'Against' | 'Abstain';
  date: string;
  result: string;
};

const STUB_PROPOSALS: Proposal[] = [
  { id: 'prop-001', title: 'Increase block gas limit to 30M', description: 'Proposal to raise the gas limit per block to improve throughput on high-load periods.', votesFor: 7812, votesAgainst: 1204, deadline: '2026-04-25', status: 'active' },
  { id: 'prop-002', title: 'Add validator node in ap-northeast-1', description: 'Deploy an additional validator node in Tokyo region to reduce latency for Asian users.', votesFor: 5400, votesAgainst: 3200, deadline: '2026-04-30', status: 'active' },
  { id: 'prop-003', title: 'Reduce epoch duration from 6400 to 3200', description: 'Faster epoch rotation for improved finality in testnets.', votesFor: 2100, votesAgainst: 6800, deadline: '2026-05-05', status: 'active' },
];

const STUB_MULTISIG: MultiSigTx[] = [
  { id: 'msig-001', to: '0xDeF1...3A7c', value: '50,000 CRL', description: 'Treasury allocation for Q2 grants', threshold: 3, approvals: 2, status: 'pending' },
  { id: 'msig-002', to: '0x9bC3...fF10', value: '12,500 CRL', description: 'Infrastructure vendor payment', threshold: 2, approvals: 2, status: 'executed' },
  { id: 'msig-003', to: '0x4aE8...2D9b', value: '200 CRL', description: 'Bug bounty payout', threshold: 2, approvals: 1, status: 'pending' },
];

const STUB_HISTORY: HistoryEntry[] = [
  { id: 'h-1', proposal: 'Enable EVM compatibility layer', vote: 'For', date: '2026-02-14', result: 'Passed' },
  { id: 'h-2', proposal: 'Reduce validator stake to 1000 CRL', vote: 'Against', date: '2026-01-28', result: 'Rejected' },
  { id: 'h-3', proposal: 'Deploy fee-burning mechanism', vote: 'For', date: '2025-12-10', result: 'Passed' },
];

const STATUS_ICON: Record<string, React.ReactNode> = {
  active: <PendingIcon fontSize="small" />,
  passed: <CheckCircleIcon fontSize="small" />,
  rejected: <CancelIcon fontSize="small" />,
  pending: <PendingIcon fontSize="small" />,
};

const STATUS_COLOR: Record<string, 'success' | 'error' | 'warning' | 'default'> = {
  active: 'warning',
  passed: 'success',
  rejected: 'error',
  pending: 'warning',
  executed: 'success',
  cancelled: 'error',
};

export default function GovernancePage() {
  const theme = useTheme();
  const [tab, setTab] = useState(0);
  const [hasBlockchain, setHasBlockchain] = useState<boolean | null>(null);

  useEffect(() => {
    fetch('/api/projects')
      .then((r) => r.json())
      .then((j) => {
        const projects: any[] = j.projects || [];
        setHasBlockchain(projects.some((p) => p.projectType === 'blockchain'));
      })
      .catch(() => setHasBlockchain(false));
  }, []);

  if (hasBlockchain === null) {
    return (
      <Box sx={{ p: 4, display: 'flex', justifyContent: 'center', pt: 10 }}>
        <CircularProgress size={32} />
      </Box>
    );
  }

  // dApp-only users: show contextual upgrade prompt
  if (!hasBlockchain) {
    return (
      <Box sx={{ p: 4, maxWidth: 680 }}>
        <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
          <Box>
            <Typography variant="h4" fontWeight={900} gutterBottom>Governance</Typography>
            <Typography variant="body1" color="text.secondary">
              On-chain governance for your network.
            </Typography>
          </Box>
        </Stack>

        <Paper
          variant="outlined"
          sx={{
            p: 4, borderRadius: 3, textAlign: 'center',
            borderColor: alpha(theme.palette.primary.main, 0.2),
            background: alpha(theme.palette.primary.main, 0.03),
          }}
        >
          <Box sx={{
            width: 64, height: 64, borderRadius: '50%', mx: 'auto', mb: 2,
            bgcolor: alpha(theme.palette.primary.main, 0.1),
            display: 'flex', alignItems: 'center', justifyContent: 'center',
          }}>
            <AccountBalanceIcon sx={{ fontSize: 32, color: 'primary.main' }} />
          </Box>
          <Typography variant="h6" fontWeight={800} gutterBottom>
            Governance is for Blockchain Projects
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 3, maxWidth: 460, mx: 'auto' }}>
            Governance lets validator node operators and token holders vote on proposals, manage
            multi-signature treasury transactions, and control protocol parameters. This requires
            a Private Blockchain project with a native token and governance module.
          </Typography>

          <Stack direction="row" spacing={2} justifyContent="center">
            <Button
              variant="contained"
              startIcon={<RocketLaunchIcon />}
              sx={{ borderRadius: 999, fontWeight: 700 }}
              onClick={() => {
                const isLocal = typeof window !== 'undefined' && window.location.hostname.includes('localhost');
                window.location.href = isLocal ? 'http://studio.localhost:3000' : 'https://studio.cerulea.app';
              }}
            >
              Create a Blockchain Project
            </Button>
            <Button
              variant="outlined"
              sx={{ borderRadius: 999, fontWeight: 700 }}
              component={Link}
              href="/dashboard"
            >
              Back to Overview
            </Button>
          </Stack>
        </Paper>

        <Paper variant="outlined" sx={{ p: 3, borderRadius: 3, mt: 3 }}>
          <Typography variant="subtitle2" fontWeight={800} gutterBottom>
            What governance unlocks:
          </Typography>
          <Stack spacing={1.5} mt={1}>
            {[
              'Submit and vote on protocol upgrade proposals',
              'Multi-signature treasury management with configurable thresholds',
              'Validator node rotation and stake management',
              'Token-weighted voting with quorum requirements',
              'Audit trail of all governance decisions on-chain',
            ].map((item) => (
              <Stack key={item} direction="row" spacing={1.5} alignItems="center">
                <CheckCircleIcon sx={{ fontSize: 16, color: 'success.main', flexShrink: 0 }} />
                <Typography variant="body2" color="text.secondary">{item}</Typography>
              </Stack>
            ))}
          </Stack>
        </Paper>
      </Box>
    );
  }

  return (
    <Box sx={{ p: 4, maxWidth: 1100 }}>
      {/* Header */}
      <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={4}>
        <Box>
          <Typography variant="h4" fontWeight={900} gutterBottom>
            Governance
          </Typography>
          <Typography variant="body1" color="text.secondary">
            Vote on proposals and manage multi-signature transactions for your network.
          </Typography>
        </Box>
        <Button variant="outlined" startIcon={<HowToVoteIcon />} sx={{ borderRadius: 999, fontWeight: 700 }}>
          Create Proposal
        </Button>
      </Stack>

      {/* Tabs */}
      <Tabs
        value={tab}
        onChange={(_, v) => setTab(v)}
        sx={{ mb: 3, borderBottom: `1px solid ${theme.palette.divider}` }}
      >
        <Tab label="Active Proposals" sx={{ fontWeight: 700, textTransform: 'none' }} />
        <Tab label="Multi-sig Transactions" sx={{ fontWeight: 700, textTransform: 'none' }} />
      </Tabs>

      {/* Tab 0: Proposals */}
      {tab === 0 && (
        <Stack spacing={3}>
          {STUB_PROPOSALS.map((p) => {
            const total = p.votesFor + p.votesAgainst;
            const forPct = total > 0 ? (p.votesFor / total) * 100 : 0;
            return (
              <Paper key={p.id} variant="outlined" sx={{ p: 3, borderRadius: 3 }}>
                <Stack direction="row" alignItems="flex-start" justifyContent="space-between" mb={1.5}>
                  <Box sx={{ flex: 1, mr: 2 }}>
                    <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
                      <Typography variant="body1" fontWeight={800}>{p.title}</Typography>
                      <Chip
                        label={p.status}
                        size="small"
                        color={STATUS_COLOR[p.status]}
                        icon={STATUS_ICON[p.status] as any}
                        sx={{ fontWeight: 700, fontSize: '0.7rem', textTransform: 'capitalize' }}
                      />
                    </Stack>
                    <Typography variant="body2" color="text.secondary">{p.description}</Typography>
                  </Box>
                  <Button
                    variant="contained"
                    size="small"
                    startIcon={<HowToVoteIcon />}
                    disabled={p.status !== 'active'}
                    sx={{ borderRadius: 999, fontWeight: 700, whiteSpace: 'nowrap' }}
                  >
                    Vote
                  </Button>
                </Stack>

                <Stack spacing={1}>
                  <Stack direction="row" justifyContent="space-between" alignItems="center">
                    <Stack direction="row" spacing={0.5} alignItems="center">
                      <Box sx={{ width: 8, height: 8, borderRadius: '50%', bgcolor: 'success.main' }} />
                      <Typography variant="caption" fontWeight={700} color="success.main">
                        For: {p.votesFor.toLocaleString()}
                      </Typography>
                    </Stack>
                    <Stack direction="row" spacing={0.5} alignItems="center">
                      <Typography variant="caption" fontWeight={700} color="error.main">
                        Against: {p.votesAgainst.toLocaleString()}
                      </Typography>
                      <Box sx={{ width: 8, height: 8, borderRadius: '50%', bgcolor: 'error.main' }} />
                    </Stack>
                  </Stack>
                  <Box sx={{ position: 'relative', height: 8, borderRadius: 999, overflow: 'hidden', bgcolor: alpha(theme.palette.error.main, 0.15) }}>
                    <Box
                      sx={{
                        position: 'absolute', left: 0, top: 0, bottom: 0,
                        width: `${forPct}%`,
                        bgcolor: 'success.main',
                        borderRadius: 999,
                        transition: 'width 0.4s ease',
                      }}
                    />
                  </Box>
                  <Stack direction="row" justifyContent="space-between">
                    <Typography variant="caption" color="text.secondary">
                      {forPct.toFixed(1)}% in favor
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      Deadline: {new Date(p.deadline).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })}
                    </Typography>
                  </Stack>
                </Stack>
              </Paper>
            );
          })}

          {/* Voting History */}
          <Box>
            <Typography variant="h6" fontWeight={800} mb={2} mt={1}>Voting History</Typography>
            <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
              <TableContainer>
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      {['Proposal', 'Your Vote', 'Date', 'Result'].map((h) => (
                        <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                          {h.toUpperCase()}
                        </TableCell>
                      ))}
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    {STUB_HISTORY.map((h) => (
                      <TableRow key={h.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                        <TableCell>
                          <Typography variant="body2" fontWeight={600}>{h.proposal}</Typography>
                        </TableCell>
                        <TableCell>
                          <Chip
                            label={h.vote}
                            size="small"
                            color={h.vote === 'For' ? 'success' : h.vote === 'Against' ? 'error' : 'default'}
                            variant="outlined"
                            sx={{ fontWeight: 700, fontSize: '0.7rem' }}
                          />
                        </TableCell>
                        <TableCell>
                          <Typography variant="body2" color="text.secondary">{h.date}</Typography>
                        </TableCell>
                        <TableCell>
                          <Chip
                            label={h.result}
                            size="small"
                            color={h.result === 'Passed' ? 'success' : 'error'}
                            sx={{ fontWeight: 700, fontSize: '0.7rem' }}
                          />
                        </TableCell>
                      </TableRow>
                    ))}
                  </TableBody>
                </Table>
              </TableContainer>
            </Paper>
          </Box>
        </Stack>
      )}

      {/* Tab 1: Multi-sig */}
      {tab === 1 && (
        <Paper variant="outlined" sx={{ borderRadius: 3, overflow: 'hidden' }}>
          <Stack
            direction="row"
            alignItems="center"
            gap={1.5}
            sx={{ px: 3, py: 2, borderBottom: `1px solid ${theme.palette.divider}` }}
          >
            <LockIcon fontSize="small" sx={{ color: 'secondary.main' }} />
            <Typography variant="h6" fontWeight={800}>Multi-sig Transactions</Typography>
          </Stack>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  {['ID', 'To', 'Value', 'Description', 'Approvals', 'Status', ''].map((h) => (
                    <TableCell key={h} sx={{ fontWeight: 700, color: 'text.secondary', fontSize: '0.75rem', letterSpacing: 0.5 }}>
                      {h.toUpperCase()}
                    </TableCell>
                  ))}
                </TableRow>
              </TableHead>
              <TableBody>
                {STUB_MULTISIG.map((tx) => (
                  <TableRow key={tx.id} sx={{ '&:hover': { bgcolor: alpha(theme.palette.action.hover, 0.5) } }}>
                    <TableCell>
                      <Typography variant="body2" sx={{ fontFamily: 'monospace', color: 'text.secondary' }}>
                        {tx.id}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" sx={{ fontFamily: 'monospace', fontWeight: 600 }}>
                        {tx.to}
                      </Typography>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" fontWeight={700}>{tx.value}</Typography>
                    </TableCell>
                    <TableCell>
                      <Typography variant="body2" color="text.secondary">{tx.description}</Typography>
                    </TableCell>
                    <TableCell>
                      <Stack direction="row" alignItems="center" spacing={1}>
                        <Typography variant="body2" fontWeight={700}>
                          {tx.approvals}/{tx.threshold}
                        </Typography>
                        <LinearProgress
                          variant="determinate"
                          value={(tx.approvals / tx.threshold) * 100}
                          sx={{ width: 48, borderRadius: 999, height: 6 }}
                          color={tx.approvals >= tx.threshold ? 'success' : 'warning'}
                        />
                      </Stack>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={tx.status}
                        size="small"
                        color={STATUS_COLOR[tx.status]}
                        sx={{ fontWeight: 700, fontSize: '0.7rem', textTransform: 'capitalize' }}
                      />
                    </TableCell>
                    <TableCell align="right">
                      <Button
                        size="small"
                        variant={tx.status === 'pending' ? 'contained' : 'outlined'}
                        disabled={tx.status !== 'pending'}
                        sx={{ borderRadius: 999, fontWeight: 700, fontSize: '0.7rem' }}
                      >
                        Sign
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </Paper>
      )}
    </Box>
  );
}
