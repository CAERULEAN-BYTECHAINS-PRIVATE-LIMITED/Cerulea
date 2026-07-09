// StepRegistry.tsx
// Registry of steps for the Studio shell.
// Each entry provides a lazy loader so the shell can dynamically import it.

export type StepMeta = {
  id: number;
  label: string;
  path: 'dapp' | 'blockchain' | 'common';
  // Must return a dynamic import promise for the step module
  loader: () => Promise<any>;
};

// NOTE: Step 1 is 'common' so BOTH paths include it.
export const STEP_REGISTRY: StepMeta[] = [
  {
    id: 0,
    label: 'Project Foundation',
    path: 'common',
    loader: () => import('@/components/studio/steps/step0'),
  },
  {
    id: 1,
    label: 'Application Blueprint',
    path: 'common',
    loader: () => import('@/components/studio/steps/step1'),
  },
  {
    id: 2,
    label: '',
    path: 'common',
    loader: () => import('@/components/studio/steps/step2'),
  },
  {
    id: 3,
    label: '',
    path: 'common',
    loader: () => import('@/components/studio/steps/step3'),
  },
  {
    id: 4,
    label: '',
    path: 'common',
    loader: () => import('@/components/studio/steps/step4'),
  },
  /*{
    id: 5,
    label: '',
    path: 'common',
    loader: () => import('@/components/studio/steps/step5'),
  }, */
  {
    id: 5,
    label: '',
    path: 'common',
    loader: () => import('@/components/studio/steps/step6'),
  },

  // Add more steps here as you build them; prefer 'common' unless truly path-specific.
];

// Back-compat alias for code that still imports ALL_STEPS
export const ALL_STEPS = STEP_REGISTRY;

// Helpers used by StudioHeader and StudioFooter
export const TOTAL_STEPS = STEP_REGISTRY.length;
export function getStepLabel(step: number): string {
  return STEP_REGISTRY[step - 1]?.label || `Step ${step}`;
}
