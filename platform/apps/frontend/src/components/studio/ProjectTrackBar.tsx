'use client';

import { useEffect, useState } from 'react';
import { Box, Stack, Typography, Tooltip } from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import CheckCircleIcon from '@mui/icons-material/CheckCircle';
import RadioButtonUncheckedIcon from '@mui/icons-material/RadioButtonUnchecked';
import FiberManualRecordIcon from '@mui/icons-material/FiberManualRecord';

interface Section {
  label: string;
  key: string;
  shortLabel: string;
}

const SECTIONS: Section[] = [
  { key: 'type',        label: 'Project Type',       shortLabel: 'Type' },
  { key: 'blueprint',   label: 'Blueprint',           shortLabel: 'Blueprint' },
  { key: 'data',        label: 'Data & Logic',        shortLabel: 'Data' },
  { key: 'economics',   label: 'Token Economics',     shortLabel: 'Economics' },
  { key: 'integrations',label: 'Integrations',        shortLabel: 'Integrations' },
  { key: 'ui',          label: 'UI Builder',          shortLabel: 'UI' },
  { key: 'deploy',      label: 'Review & Deploy',     shortLabel: 'Deploy' },
];

type SectionStatus = 'done' | 'partial' | 'empty';

function computeProgress(): Record<string, SectionStatus> {
  const result: Record<string, SectionStatus> = {};

  try {
    // Type: projectType + templateId
    const pType = localStorage.getItem('cerulea.projectType');
    const tId   = localStorage.getItem('cerulea.templateId') || localStorage.getItem('cerulea.templateModules');

    // Don't show any progress if the user hasn't selected a project type yet in this session
    if (!pType) {
      SECTIONS.forEach(s => { result[s.key] = 'empty'; });
      return result;
    }

    result.type = pType && tId ? 'done' : pType ? 'partial' : 'empty';

    // Blueprint: nodes on canvas
    const graphRaw = localStorage.getItem('cerulea.step1.graph');
    const graph = graphRaw ? JSON.parse(graphRaw) : null;
    const nodeCount = graph?.nodes?.length ?? 0;
    result.blueprint = nodeCount >= 3 ? 'done' : nodeCount > 0 ? 'partial' : 'empty';

    // Data: moduleEntities
    const draftRaw = localStorage.getItem('draft:local:3');
    const draft = draftRaw ? JSON.parse(draftRaw) : null;
    const modCount = draft?.data?.moduleEntities
      ? Object.keys(draft.data.moduleEntities).length
      : 0;
    result.data = modCount >= 2 ? 'done' : modCount > 0 ? 'partial' : 'empty';

    // Economics: check if any tokenomics key saved
    const econ = localStorage.getItem('cerulea.step3.economics') || localStorage.getItem('draft:local:4');
    result.economics = econ ? 'done' : 'empty';

    // Integrations: any integration configured
    const intg = localStorage.getItem('cerulea.step4.integrations') || localStorage.getItem('draft:local:5');
    result.integrations = intg ? 'done' : 'empty';

    // UI: any UI config saved
    const ui = localStorage.getItem('cerulea.step5.ui') || localStorage.getItem('draft:local:6');
    result.ui = ui ? 'done' : 'empty';

    // Deploy: deployed flag
    const deployed = localStorage.getItem('cerulea.deployed');
    result.deploy = deployed ? 'done' : 'empty';
  } catch {
    // localStorage not available (SSR)
    SECTIONS.forEach(s => { result[s.key] = 'empty'; });
  }

  return result;
}

function totalPercent(statuses: Record<string, SectionStatus>): number {
  const vals = Object.values(statuses);
  const score = vals.reduce((acc, s) => acc + (s === 'done' ? 1 : s === 'partial' ? 0.5 : 0), 0);
  return Math.round((score / vals.length) * 100);
}

interface ProjectTrackBarProps {
  currentStepKey?: string;
}

export default function ProjectTrackBar({ currentStepKey }: ProjectTrackBarProps) {
  const theme = useTheme();
  const [statuses, setStatuses] = useState<Record<string, SectionStatus>>({});

  useEffect(() => {
    setStatuses(computeProgress());
    const id = setInterval(() => setStatuses(computeProgress()), 3000);
    return () => clearInterval(id);
  }, []);

  const percent = totalPercent(statuses);
  const isDark = theme.palette.mode === 'dark';

  return (
    <Box
      sx={{
        px: 2,
        py: 0.75,
        borderRadius: 999,
        width: 'max-content',
        maxWidth: '90vw',
        bgcolor: isDark ? 'rgba(16,16,20,0.92)' : 'rgba(255,255,255,0.93)',
        backdropFilter: 'blur(12px)',
        border: `1px solid ${alpha(theme.palette.divider, 0.5)}`,
        boxShadow: `0 4px 20px ${alpha(theme.palette.common.black, 0.15)}`,
        display: 'flex',
        alignItems: 'center',
        gap: 0.5,
      }}
    >
        {/* Overall percent */}
        <Typography
          variant="caption"
          fontWeight={800}
          color="primary.main"
          sx={{ fontSize: '0.65rem', mr: 1, letterSpacing: 0.5 }}
        >
          {percent}%
        </Typography>

        {SECTIONS.map((section, i) => {
          const status = statuses[section.key] || 'empty';
          const isCurrent = section.key === currentStepKey;
          const color = status === 'done'
            ? theme.palette.success.main
            : status === 'partial'
              ? theme.palette.warning.main
              : isCurrent
                ? theme.palette.primary.main
                : theme.palette.text.disabled;

          return (
            <Tooltip
              key={section.key}
              title={`${section.label}: ${status === 'done' ? 'Complete' : status === 'partial' ? 'In Progress' : 'Not started'}`}
              placement="bottom"
              arrow
            >
              <Stack
                direction="row"
                alignItems="center"
                spacing={0.4}
                sx={{ pointerEvents: 'auto', cursor: 'default' }}
              >
                {status === 'done' ? (
                  <CheckCircleIcon sx={{ fontSize: 13, color }} />
                ) : status === 'partial' ? (
                  <FiberManualRecordIcon sx={{ fontSize: 10, color }} />
                ) : (
                  <RadioButtonUncheckedIcon sx={{ fontSize: 13, color, opacity: isCurrent ? 1 : 0.4 }} />
                )}
                <Typography
                  variant="caption"
                  sx={{
                    fontSize: '0.62rem',
                    fontWeight: isCurrent ? 800 : 500,
                    color: isCurrent ? 'primary.main' : color,
                    opacity: status === 'empty' && !isCurrent ? 0.5 : 1,
                    letterSpacing: 0.3,
                    whiteSpace: 'nowrap',
                  }}
                >
                  {section.shortLabel}
                </Typography>
                {i < SECTIONS.length - 1 && (
                  <Box sx={{ width: 12, height: 1, bgcolor: alpha(theme.palette.divider, 0.6), mx: 0.25 }} />
                )}
              </Stack>
            </Tooltip>
          );
        })}
    </Box>
  );
}
