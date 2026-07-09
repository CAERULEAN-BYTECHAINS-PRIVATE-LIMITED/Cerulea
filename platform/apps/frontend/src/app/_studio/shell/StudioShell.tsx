'use client';

import React, { Component, ErrorInfo, useEffect, useMemo, useState, useCallback, Suspense } from 'react';
import { Box, Container, Stack, Typography, Paper, Button, Fab, Tooltip } from '@mui/material';
import HexagonOutlinedIcon from '@mui/icons-material/HexagonOutlined';
import { alpha } from '@mui/material/styles';
import { ALL_STEPS, StepMeta } from './StepRegistry';
import { useStudio } from '@/context/StudioContext';
import { useAutoSave } from '@/lib/useAutoSave';
import dynamic from 'next/dynamic';

const SmartContractsScreen = dynamic(() => import('@/components/SmartContractsScreen'), { ssr: false });
const ProjectTrackBar = dynamic(() => import('@/components/studio/ProjectTrackBar'), { ssr: false });

const STEP_SECTION_KEYS = ['type', 'blueprint', 'data', 'economics', 'integrations', 'ui', 'deploy'];

/* ---- Error Boundary so a broken step shows an error, not a blank page ---- */
class StepErrorBoundary extends Component<
  { children: React.ReactNode; stepLabel: string },
  { hasError: boolean; error?: Error }
> {
  constructor(props: any) {
    super(props);
    this.state = { hasError: false };
  }
  static getDerivedStateFromError(error: Error) {
    return { hasError: true, error };
  }
  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('[StudioShell] Step render error:', error, info);
  }
  render() {
    if (this.state.hasError) {
      return (
        <Box sx={{ p: 4, textAlign: 'center' }}>
          <Typography variant="h6" color="error" gutterBottom>
            Something went wrong loading "{this.props.stepLabel}"
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2, fontFamily: 'monospace', whiteSpace: 'pre-wrap' }}>
            {this.state.error?.message}
          </Typography>
          <Button variant="outlined" onClick={() => this.setState({ hasError: false })}>
            Retry
          </Button>
        </Box>
      );
    }
    return this.props.children;
  }
}

// Props passed to each step component
type StepProps = {
  goNext: () => void;
  goPrev: () => void;
  projectId: string | null;
};

// Utility to detect a React component export
const isComponentType = (x: any): x is React.ComponentType<any> =>
  typeof x === 'function' ||
  (x &&
    typeof x === 'object' &&
    (x.$$typeof === (Symbol as any).for('react.memo') ||
      x.$$typeof === (Symbol as any).for('react.forward_ref')));

function BrokenStep({ meta, mod }: { meta: StepMeta; mod: any }) {
  return (
    <Paper sx={{ p: 2, border: '1px solid', borderColor: 'error.main', background: (t) => t.palette.error.light + '22' }}>
      <Typography variant="h6" color="error" gutterBottom>
        Step “{meta.label}” isn’t exporting a React component
      </Typography>
      <Typography sx={{ mb: 1 }}>
        File loaded for this step, but no component export was found.
        <br />
        Add <code>export default function YourStep() {'{'} return (&lt;.../&gt;); {'}'}</code>
      </Typography>
      <Typography variant="body2" sx={{ whiteSpace: 'pre-wrap', fontFamily: 'ui-monospace, Menlo, monospace' }}>
        Module export keys: {JSON.stringify(Object.keys(mod || {}), null, 2)}
      </Typography>
    </Paper>
  );
}

// Lazy wrapper that uses StepMeta.loader()
function makeLazy(meta: StepMeta): React.LazyExoticComponent<React.ComponentType<StepProps>> {
  return React.lazy(async (): Promise<{ default: React.ComponentType<StepProps> }> => {
    try {
      const mod = await meta.loader();
      const candidates: any[] = [
        mod?.default,
        mod?.Page,
        mod?.Component,
        mod?.Step,
        ...Object.values(mod ?? {}),
      ];
      let picked = candidates.find(isComponentType);
      if (!picked && React.isValidElement(mod?.default)) {
        const node = mod.default as React.ReactElement;
        picked = function WrappedNode() {
          return node;
        };
      }
      if (!picked) return { default: () => <BrokenStep meta={meta} mod={mod} /> };
      return { default: picked as React.ComponentType<any> };
    } catch {
      return {
        default: () => (
          <Paper sx={{ p: 2, border: '1px solid', borderColor: 'error.main' }}>
            <Typography variant="h6" color="error" gutterBottom>
              Failed to load step “{meta.label}”
            </Typography>
            <Typography variant="body2">Check the import path in StepRegistry for this step.</Typography>
          </Paper>
        ),
      };
    }
  });
}

export default function StudioShell({
  initialStep = 0,
  initialProjectId = null,
}: {
  initialStep?: number;
  initialProjectId?: string | null;
}) {
  const studio = useStudio();
  const { projectType } = studio;

  // Build the workflow:
  // If projectType is not chosen yet, show only 'common' steps
  const currentWorkflow = useMemo(() => {
    const commons = ALL_STEPS.filter((s) => s.path === 'common');
    if (!projectType) return commons.length ? commons : [ALL_STEPS[0]];
    return ALL_STEPS.filter((s) => s.path === projectType || s.path === 'common');
  }, [projectType]);

  const [stepIndex, setStepIndex] = useState(() =>
    Math.max(0, Math.min(initialStep, currentWorkflow.length - 1))
  );
  const [projectId] = useState<string | null>(initialProjectId);
  const [contractsOpen, setContractsOpen] = useState(false);

  // Clamp index if workflow changes
  useEffect(() => {
    if (stepIndex > currentWorkflow.length - 1) {
      setStepIndex(Math.max(0, currentWorkflow.length - 1));
    }
  }, [currentWorkflow.length, stepIndex]);

  const TOTAL_STEPS = currentWorkflow.length;
  const step = currentWorkflow[stepIndex];
  const StepView = useMemo(() => makeLazy(step), [step]);

  // Navigation (no shell buttons; kept for page-level use + keyboard)
  const goPrev = useCallback(() => setStepIndex((i) => Math.max(0, i - 1)), []);
  const goNext = useCallback(
    () => setStepIndex((i) => Math.min(TOTAL_STEPS - 1, i + 1)),
    [TOTAL_STEPS]
  );

  // Keyboard nav remains (hint is shown in footer)
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowLeft') goPrev();
      if (e.key === 'ArrowRight') goNext();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [goPrev, goNext]);

  // Autosave snapshot (unchanged)
  const projectSnapshot = useMemo(
    () => ({
      ...studio,
      selectedModules: (studio as any).selectedModules || [],
      logicFlow: (studio as any).logicFlow || {},
      customDataSchemas: (studio as any).customDataSchemas || [],
      accessControls: (studio as any).accessControls || {},
      tokenomics: (studio as any).tokenomics || {},
      aiConfigs: (studio as any).aiConfigs || [],
      uiBuilder: (studio as any).uiBuilder || {},
      __v: 1,
    }),
    [studio]
  );

  const { status } = useAutoSave({
    projectId: projectId ?? 'local',
    stepCode: 'project',
    data: projectSnapshot,
  });

  const statusText =
    status === 'saving'
      ? 'Saving…'
      : status === 'saved'
      ? 'Saved'
      : status === 'error'
      ? 'Save failed'
      : 'Idle';

  return (
    <Box sx={{ minHeight: 'calc(100vh - 64px)', display: 'flex', flexDirection: 'column' }}>
      {/* Smart Contracts Overlay */}
      {contractsOpen && (
        <Box
          sx={{
            position: 'fixed',
            inset: 0,
            top: 64,
            zIndex: 1400,
            bgcolor: 'background.default',
            overflow: 'auto',
          }}
        >
          <SmartContractsScreen
            onClose={() => setContractsOpen(false)}
            onGoToBlueprint={() => { setContractsOpen(false); setStepIndex(1); }}
          />
        </Box>
      )}

      {/* Project Progress TrackBar — floats in the NavBar strip (above all step overlays) */}
      {!contractsOpen && (
        <Box sx={{ position: 'fixed', top: 14, left: '50%', transform: 'translateX(-50%)', zIndex: 1400, pointerEvents: 'none' }}>
          <ProjectTrackBar currentStepKey={STEP_SECTION_KEYS[stepIndex] || 'type'} />
        </Box>
      )}

      {/* Floating Smart Contracts Button — left side to avoid AI chatbot overlap */}
      <Fab
        variant="extended"
        size="small"
        onClick={() => setContractsOpen((o) => !o)}
        sx={{
          position: 'fixed',
          bottom: 32,
          left: 24,
          zIndex: 1302,
          bgcolor: contractsOpen ? 'primary.main' : 'background.paper',
          color: contractsOpen ? 'white' : 'primary.main',
          border: (t) => `1px solid ${alpha(t.palette.primary.main, 0.5)}`,
          boxShadow: (t) => `0 4px 24px ${alpha(t.palette.primary.main, 0.3)}`,
          fontWeight: 700,
          fontSize: '0.72rem',
          letterSpacing: 0.4,
          gap: 0.75,
          px: 2,
          '&:hover': { bgcolor: 'primary.main', color: 'white' },
        }}
      >
        <HexagonOutlinedIcon sx={{ fontSize: 16 }} />
        Smart Contracts
      </Fab>
      <Container maxWidth="xl" sx={{ flex: 1, py: 3, display: 'flex', flexDirection: 'column' }}>
        {/* Keep a lightweight step header; pages own Back/Next inside themselves */}
        <Stack sx={{ mb: 2 }}>
          <Typography variant="overline" color="text.secondary">
            STEP {stepIndex + 1} OF {TOTAL_STEPS}
          </Typography>
          <Typography variant="h5" sx={{ fontWeight: 600 }}>
            {step.label}
          </Typography>
        </Stack>

        <Box sx={{ flex: 1, display: 'flex', flexDirection: 'column', width: '100%' }}>
          <StepErrorBoundary stepLabel={step.label || `Step ${stepIndex + 1}`}>
            <Suspense fallback={<Typography>Loading…</Typography>}>
              <StepView goNext={goNext} goPrev={goPrev} projectId={projectId} />
            </Suspense>
          </StepErrorBoundary>
        </Box>
      </Container>

      <Box component="footer" sx={{ borderTop: (t) => `1px solid ${t.palette.divider}`, py: 1.5, px: 2, mt: 'auto' }}>
        <Container maxWidth="xl" sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <Typography variant="caption" color="text.secondary">
            {statusText}
          </Typography>
          <Typography variant="caption" color="text.secondary">
            Tip: use ← / → to navigate steps
          </Typography>
        </Container>
      </Box>
    </Box>
  );
}
