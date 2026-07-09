import type { Dispatch, SetStateAction } from 'react';

export type StepComponentProps = {
  stepIndex: number;
  stepId: number;
  projectId?: string;
  setProjectId?: Dispatch<SetStateAction<string | null>>;
  goPrev: () => void;
  goNext: () => void;
  onSoftSave?: () => void; // called by step when it “saves” something lightweight
};
