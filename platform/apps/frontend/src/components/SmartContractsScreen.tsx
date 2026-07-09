'use client';

import React, { useMemo, useState, useEffect } from 'react';
import {
  Box, Typography, Stack, Paper, Chip, IconButton, Switch, Tooltip,
  Divider, Button, Dialog, DialogTitle, DialogContent, DialogActions,
  Accordion, AccordionSummary, AccordionDetails, Alert, TextField, Fade,
  Table, TableBody, TableCell, TableHead, TableRow, TableContainer,
} from '@mui/material';
import { alpha, useTheme, styled } from '@mui/material/styles';
import CloseIcon from '@mui/icons-material/Close';
import ExpandMoreIcon from '@mui/icons-material/ExpandMore';
import CodeIcon from '@mui/icons-material/Code';
import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import InfoOutlinedIcon from '@mui/icons-material/InfoOutlined';
import ArticleIcon from '@mui/icons-material/Article';
import AccountTreeIcon from '@mui/icons-material/AccountTree';
import { useSession } from 'next-auth/react';
import { useStudio } from '@/context/StudioContext';
import { deriveSmartContracts, getDemoContracts, SmartContract, ContractType } from '@/lib/smartContracts';

// ─── Styled Components ─────────────────────────────────────────────
const ContractItem = styled(Box, { shouldForwardProp: (p) => p !== 'selected' })<{ selected?: boolean }>(({ theme, selected }) => ({
  padding: '14px 20px',
  cursor: 'pointer',
  borderLeft: `3px solid ${selected ? theme.palette.primary.main : 'transparent'}`,
  background: selected ? alpha(theme.palette.primary.main, 0.05) : 'transparent',
  display: 'flex',
  alignItems: 'center',
  gap: 12,
  transition: 'all 0.15s ease',
  '&:hover': {
    background: selected ? alpha(theme.palette.primary.main, 0.08) : alpha(theme.palette.action.hover, 0.5),
  },
}));

const CONTRACT_TYPE_COLOR: Record<ContractType, string> = {
  ERC20: '#3b82f6',
  ERC721: '#8b5cf6',
  ERC1155: '#6366f1',
  Governance: '#10b981',
  Staking: '#f59e0b',
  Vault: '#06b6d4',
  Bridge: '#ec4899',
  Oracle: '#f97316',
  AccessControl: '#14b8a6',
  MultiSig: '#84cc16',
  Marketplace: '#ef4444',
  Custom: '#6b7280',
};

const SOLIDITY_STUBS: Record<string, string> = {
  ERC20: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC20/ERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract CeruleaToken is ERC20, Ownable {
    constructor(
        string memory name_,
        string memory symbol_,
        uint256 initialSupply
    ) ERC20(name_, symbol_) Ownable(msg.sender) {
        _mint(msg.sender, initialSupply);
    }

    function mint(address to, uint256 amount) external onlyOwner {
        _mint(to, amount);
    }
}`,
  ERC721: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/extensions/ERC721URIStorage.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract CeruleaNFT is ERC721URIStorage, Ownable {
    uint256 private _tokenIds;
    string public baseTokenURI;

    constructor(
        string memory name_,
        string memory symbol_,
        string memory baseURI_
    ) ERC721(name_, symbol_) Ownable(msg.sender) {
        baseTokenURI = baseURI_;
    }

    function mint(address to, string memory tokenURI_) external onlyOwner returns (uint256) {
        _tokenIds++;
        _mint(to, _tokenIds);
        _setTokenURI(_tokenIds, tokenURI_);
        return _tokenIds;
    }
}`,
  Governance: `// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

import "@openzeppelin/contracts/governance/Governor.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorSettings.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorCountingSimple.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorVotes.sol";
import "@openzeppelin/contracts/governance/extensions/GovernorVotesQuorumFraction.sol";

contract CeruleaGovernor is Governor, GovernorSettings, GovernorCountingSimple, GovernorVotes, GovernorVotesQuorumFraction {
    constructor(IVotes _token, uint48 _votingDelay, uint32 _votingPeriod, uint256 _quorumNumerator)
        Governor("Cerulea Governor")
        GovernorSettings(_votingDelay, _votingPeriod, 0)
        GovernorVotes(_token)
        GovernorVotesQuorumFraction(_quorumNumerator)
    {}
}`,
};

// ─── Component ─────────────────────────────────────────────────────

interface SmartContractsScreenProps {
  onClose?: () => void;
  fullPage?: boolean;
  onGoToBlueprint?: () => void;
}

export default function SmartContractsScreen({ onClose, fullPage, onGoToBlueprint }: SmartContractsScreenProps) {
  const theme = useTheme();
  const { selectedModules, projectType } = useStudio();
  const { data: session } = useSession();
  const isTestAccount = (session?.user as any)?.isTestAccount === true;

  const [contracts, setContracts] = useState<SmartContract[]>(() =>
    deriveSmartContracts(selectedModules, projectType)
  );
  const [selectedId, setSelectedId] = useState<string | null>(contracts[0]?.id ?? null);

  // When session loads and user is the test account, upgrade to the full demo set if no real contracts exist.
  useEffect(() => {
    if (!isTestAccount) return;
    const derived = deriveSmartContracts(selectedModules, projectType);
    if (derived.length < 3) {
      const demo = getDemoContracts();
      setContracts(demo);
      setSelectedId((id) => id ?? demo[0]?.id ?? null);
    }
  }, [isTestAccount, selectedModules, projectType]);
  const [disableDialogId, setDisableDialogId] = useState<string | null>(null);
  const [showSource, setShowSource] = useState(false);
  const [paramEdits, setParamEdits] = useState<Record<string, string>>({});

  const selectedContract = useMemo(
    () => contracts.find((c) => c.id === selectedId) ?? null,
    [contracts, selectedId]
  );

  const toggleContract = (id: string, enabled: boolean) => {
    if (!enabled) {
      setDisableDialogId(id);
    } else {
      setContracts((prev) => prev.map((c) => (c.id === id ? { ...c, enabled: true } : c)));
    }
  };

  const confirmDisable = (id: string) => {
    setContracts((prev) => prev.map((c) => (c.id === id ? { ...c, enabled: false } : c)));
    setDisableDialogId(null);
  };

  const disableTarget = contracts.find((c) => c.id === disableDialogId);

  return (
    <Box
      sx={{
        width: '100%',
        height: fullPage ? '100vh' : '100%',
        display: 'flex',
        flexDirection: 'column',
        bgcolor: 'background.default',
      }}
    >
      {/* Header */}
      <Box
        sx={{
          px: 3,
          py: 2,
          borderBottom: `1px solid ${theme.palette.divider}`,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          background: theme.palette.mode === 'dark' ? 'rgba(20,20,23,0.95)' : 'rgba(255,255,255,0.95)',
          backdropFilter: 'blur(12px)',
        }}
      >
        <Box>
          <Typography variant="h6" fontWeight={900}>
            Smart Contracts
          </Typography>
          <Typography variant="caption" color="text.secondary">
            {contracts.length} contract{contracts.length !== 1 ? 's' : ''} derived from your project configuration ·{' '}
            {contracts.filter((c) => c.enabled).length} enabled
          </Typography>
        </Box>
        <Stack direction="row" spacing={1} alignItems="center">
          {onGoToBlueprint && (
            <Button
              variant="contained"
              size="small"
              onClick={onGoToBlueprint}
              startIcon={<ArticleIcon sx={{ fontSize: 15 }} />}
              sx={{ borderRadius: 999, fontWeight: 700, fontSize: '0.72rem', px: 2 }}
            >
              Add via Blueprint
            </Button>
          )}
          <Tooltip title="Contracts are automatically generated from your Blueprint modules and Economics settings. Disabling a contract removes it from deployment.">
            <InfoOutlinedIcon sx={{ fontSize: 18, color: 'text.disabled', cursor: 'help' }} />
          </Tooltip>
          {onClose && (
            <IconButton onClick={onClose} size="small">
              <CloseIcon />
            </IconButton>
          )}
        </Stack>
      </Box>

      {contracts.length === 0 ? (
        <Box sx={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', flexDirection: 'column', gap: 2, opacity: 0.6 }}>
          <CodeIcon sx={{ fontSize: 48 }} />
          <Typography variant="body1" fontWeight={600}>No smart contracts yet</Typography>
          <Typography variant="body2" color="text.secondary" align="center" sx={{ maxWidth: 300 }}>
            Add modules to your Blueprint (Step 2) to automatically generate smart contracts here.
          </Typography>
        </Box>
      ) : (
        <Box sx={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          {/* Left: Contract list */}
          <Box
            sx={{
              width: 280,
              borderRight: `1px solid ${theme.palette.divider}`,
              overflowY: 'auto',
              flexShrink: 0,
            }}
          >
            <Box sx={{ p: 2 }}>
              <Typography variant="overline" fontWeight={800} color="text.disabled" fontSize="0.6rem">
                CONTRACTS
              </Typography>
            </Box>
            {contracts.map((c) => (
              <ContractItem
                key={c.id}
                selected={selectedId === c.id}
                onClick={() => setSelectedId(c.id)}
              >
                <Box
                  sx={{
                    width: 8,
                    height: 8,
                    borderRadius: '50%',
                    bgcolor: c.enabled
                      ? CONTRACT_TYPE_COLOR[c.contractType] ?? theme.palette.primary.main
                      : theme.palette.action.disabled,
                    flexShrink: 0,
                  }}
                />
                <Box sx={{ flex: 1, minWidth: 0 }}>
                  <Typography variant="body2" fontWeight={700} noWrap>
                    {c.name}
                  </Typography>
                  <Chip
                    label={c.contractType}
                    size="small"
                    sx={{
                      height: 16,
                      fontSize: '0.6rem',
                      fontWeight: 700,
                      bgcolor: alpha(CONTRACT_TYPE_COLOR[c.contractType] ?? theme.palette.primary.main, 0.12),
                      color: CONTRACT_TYPE_COLOR[c.contractType] ?? theme.palette.primary.main,
                    }}
                  />
                </Box>
                {c.enabled ? (
                  <CheckCircleIcon sx={{ fontSize: 14, color: 'success.main', flexShrink: 0 }} />
                ) : (
                  <WarningAmberIcon sx={{ fontSize: 14, color: 'warning.main', flexShrink: 0 }} />
                )}
              </ContractItem>
            ))}
          </Box>

          {/* Right: Contract detail */}
          {selectedContract && (
            <Fade in key={selectedContract.id}>
              <Box sx={{ flex: 1, overflowY: 'auto', p: 4 }}>
                <Stack spacing={3} sx={{ maxWidth: 800 }}>
                  {/* Contract header */}
                  <Stack direction="row" alignItems="flex-start" justifyContent="space-between">
                    <Box>
                      <Stack direction="row" alignItems="center" spacing={1.5} mb={0.5}>
                        <Typography variant="h5" fontWeight={900}>
                          {selectedContract.name}
                        </Typography>
                        <Chip
                          label={selectedContract.contractType}
                          size="small"
                          sx={{
                            fontWeight: 700,
                            bgcolor: alpha(CONTRACT_TYPE_COLOR[selectedContract.contractType] ?? theme.palette.primary.main, 0.12),
                            color: CONTRACT_TYPE_COLOR[selectedContract.contractType] ?? theme.palette.primary.main,
                          }}
                        />
                        <Chip
                          label={`From: ${selectedContract.source}`}
                          size="small"
                          variant="outlined"
                          sx={{ fontWeight: 600, opacity: 0.7 }}
                        />
                      </Stack>
                      <Typography variant="body2" color="text.secondary">
                        {selectedContract.description}
                      </Typography>
                    </Box>
                    <Switch
                      checked={selectedContract.enabled}
                      onChange={(e) => toggleContract(selectedContract.id, e.target.checked)}
                      color="success"
                    />
                  </Stack>

                  {!selectedContract.enabled && (
                    <Alert severity="warning" icon={<WarningAmberIcon />}>
                      <Typography variant="body2" fontWeight={700}>This contract is disabled</Typography>
                      <Typography variant="body2">{selectedContract.ifDisabled}</Typography>
                    </Alert>
                  )}

                  <Divider />

                  {/* Why it exists */}
                  <Paper
                    variant="outlined"
                    sx={{ p: 2.5, borderRadius: 3, bgcolor: alpha(theme.palette.info.main, 0.04) }}
                  >
                    <Stack direction="row" spacing={1.5} alignItems="flex-start">
                      <InfoOutlinedIcon sx={{ color: 'info.main', mt: 0.3, flexShrink: 0 }} />
                      <Box>
                        <Typography variant="subtitle2" fontWeight={700} gutterBottom>
                          Why this contract exists
                        </Typography>
                        <Typography variant="body2" color="text.secondary">
                          {selectedContract.whyItExists}
                        </Typography>
                      </Box>
                    </Stack>
                  </Paper>

                  {/* Dependencies */}
                  {(selectedContract.dependentModules.length > 0 || selectedContract.dependentEntities.length > 0) && (
                    <Paper variant="outlined" sx={{ p: 2.5, borderRadius: 3 }}>
                      <Stack direction="row" spacing={1.5} alignItems="flex-start">
                        <AccountTreeIcon sx={{ color: 'text.secondary', mt: 0.3, flexShrink: 0 }} />
                        <Box sx={{ width: '100%' }}>
                          <Typography variant="subtitle2" fontWeight={700} gutterBottom>
                            What depends on this
                          </Typography>
                          {selectedContract.dependentModules.length > 0 && (
                            <Box mb={1}>
                              <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.5 }}>MODULES</Typography>
                              <Stack direction="row" flexWrap="wrap" gap={0.5}>
                                {selectedContract.dependentModules.map((m) => (
                                  <Chip key={m} label={m} size="small" variant="outlined" />
                                ))}
                              </Stack>
                            </Box>
                          )}
                          {selectedContract.dependentEntities.length > 0 && (
                            <Box>
                              <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 0.5 }}>ENTITIES (on-chain fields)</Typography>
                              <Stack direction="row" flexWrap="wrap" gap={0.5}>
                                {selectedContract.dependentEntities.map((e) => (
                                  <Chip key={e} label={e} size="small" variant="outlined" />
                                ))}
                              </Stack>
                            </Box>
                          )}
                        </Box>
                      </Stack>
                    </Paper>
                  )}

                  {/* Impact if disabled */}
                  <Paper
                    variant="outlined"
                    sx={{ p: 2.5, borderRadius: 3, bgcolor: alpha(theme.palette.warning.main, 0.04) }}
                  >
                    <Stack direction="row" spacing={1.5} alignItems="flex-start">
                      <WarningAmberIcon sx={{ color: 'warning.main', mt: 0.3, flexShrink: 0 }} />
                      <Box>
                        <Typography variant="subtitle2" fontWeight={700} gutterBottom>
                          What happens if you disable this
                        </Typography>
                        <Typography variant="body2" color="text.secondary">
                          {selectedContract.ifDisabled}
                        </Typography>
                      </Box>
                    </Stack>
                  </Paper>

                  {/* Constructor params */}
                  {selectedContract.constructorParams && selectedContract.constructorParams.length > 0 && (
                    <Box>
                      <Typography variant="subtitle1" fontWeight={700} gutterBottom>
                        Constructor Parameters
                      </Typography>
                      <Stack spacing={2}>
                        {selectedContract.constructorParams.map((p) => (
                          <Box key={p.name}>
                            <Stack direction="row" alignItems="center" spacing={1} mb={0.5}>
                              <Typography variant="body2" fontWeight={700} fontFamily="monospace">
                                {p.name}
                              </Typography>
                              <Chip label={p.type} size="small" variant="outlined" sx={{ fontFamily: 'monospace', height: 18, fontSize: '0.65rem' }} />
                            </Stack>
                            <Typography variant="caption" color="text.secondary" sx={{ display: 'block', mb: 1 }}>
                              {p.description}
                            </Typography>
                            <TextField
                              fullWidth
                              size="small"
                              placeholder={p.defaultValue || `Enter ${p.name}`}
                              value={paramEdits[`${selectedContract.id}.${p.name}`] ?? p.defaultValue ?? ''}
                              onChange={(e) =>
                                setParamEdits((prev) => ({
                                  ...prev,
                                  [`${selectedContract.id}.${p.name}`]: e.target.value,
                                }))
                              }
                            />
                          </Box>
                        ))}
                      </Stack>
                    </Box>
                  )}

                  {/* View Source */}
                  <Accordion
                    expanded={showSource}
                    onChange={(_, v) => setShowSource(v)}
                    sx={{ borderRadius: '12px !important', '&:before': { display: 'none' } }}
                    variant="outlined"
                  >
                    <AccordionSummary expandIcon={<ExpandMoreIcon />}>
                      <Stack direction="row" alignItems="center" spacing={1}>
                        <CodeIcon fontSize="small" />
                        <Typography variant="subtitle2" fontWeight={700}>
                          View Solidity Source (Stub)
                        </Typography>
                      </Stack>
                    </AccordionSummary>
                    <AccordionDetails sx={{ p: 0 }}>
                      <Box
                        component="pre"
                        sx={{
                          m: 0,
                          p: 2,
                          fontSize: '0.78rem',
                          fontFamily: 'monospace',
                          overflowX: 'auto',
                          bgcolor: theme.palette.mode === 'dark' ? '#0a0a0c' : '#f5f5f5',
                          borderRadius: '0 0 12px 12px',
                          lineHeight: 1.6,
                          color: theme.palette.mode === 'dark' ? '#e2e8f0' : '#1a1a1a',
                          whiteSpace: 'pre-wrap',
                          wordBreak: 'break-word',
                        }}
                      >
                        {SOLIDITY_STUBS[selectedContract.contractType] ||
                          `// ${selectedContract.name}.sol\n// Auto-generated stub; full source generated at deployment time.\n\npragma solidity ^0.8.20;\n\ncontract ${selectedContract.name} {\n    // Implementation generated by Cerulea at deploy time\n}`}
                      </Box>
                    </AccordionDetails>
                  </Accordion>
                </Stack>
              </Box>
            </Fade>
          )}
        </Box>
      )}

      {/* Disable Confirmation Dialog */}
      <Dialog
        open={!!disableDialogId}
        onClose={() => setDisableDialogId(null)}
        maxWidth="sm"
        fullWidth
        PaperProps={{ sx: { borderRadius: 4 } }}
      >
        <DialogTitle sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
          <WarningAmberIcon color="warning" />
          <Typography variant="h6" fontWeight={800}>Disable {disableTarget?.name}?</Typography>
        </DialogTitle>
        <DialogContent>
          <Alert severity="warning" sx={{ mb: 2 }}>
            {disableTarget?.ifDisabled}
          </Alert>
          <Typography variant="body2" color="text.secondary">
            This contract will not be included in the deployment. You can re-enable it any time before deploying.
          </Typography>
        </DialogContent>
        <DialogActions sx={{ p: 2.5 }}>
          <Button variant="outlined" onClick={() => setDisableDialogId(null)}>Cancel</Button>
          <Button variant="contained" color="warning" onClick={() => confirmDisable(disableDialogId!)}>
            Disable Contract
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
