'use client';

import { createContext, useContext, useState, ReactNode } from 'react';

// --- CORE TYPES ---

export interface AppMetadata {
  appName: string;
  appDescription: string;
  appLogoUrl?: string;
  faviconUrl?: string;
  socialImageUrl?: string;
  primaryColor?: string;
}

export interface AppGoal {
  dappCategory: string;
  monetizationGoal: string;
  primaryObjective: string;
  keyInteractions: string[];
  targetAudience: string;
  monetizationModel: string[];
  dataStorageModel: string;
  dataConsistency: string;
  externalIntegrations: string[];
  crossChainInteroperability: string[];
  primaryUserInteraction: string;
  authenticationMethod: string;
  expectedDailyUsers: number;
  transactionVolume: string;
  dataVolume: string;
  userProfileType: string;
  notificationMethods: string[];
  governanceModel: string;
  upgradeMechanism: string;
}

export interface NetworkConfig {
  consensusMechanism?: 'PoA' | 'PoS';
  deploymentRegion?: 'us-east' | 'eu-central' | 'apac-south';
}

// --- MAIN STATE & CONTEXT TYPE ---

export interface StudioState {
  // NEW: set when project is created
  projectId?: string;
  slug?: string;

  // Step 0
  projectType: 'dapp' | 'blockchain' | null;
  templateId: string | null;
  workspaceId: string;

  // Step-specific state
  appMetadata: AppMetadata;
  networkConfig: NetworkConfig;
  appGoal: AppGoal;

  // Step 1/2
  selectedModules: string[];

  // Legacy sub-path (Private Blockchain track only)
  legacyMode?: 'none' | 'connect' | 'port';
}

export interface StudioContextType extends StudioState {
  setStudioState: (newState: Partial<StudioState>) => void;
}

// --- INITIAL STATE ---

const initialState: StudioState = {
  projectType: null,
  templateId: null,
  workspaceId: 'personal',
  appMetadata: { appName: '', appDescription: '' },
  networkConfig: {},
  appGoal: {
    dappCategory: '',
    monetizationGoal: '',
    primaryObjective: '',
    keyInteractions: [],
    targetAudience: '',
    monetizationModel: [],
    dataStorageModel: '',
    dataConsistency: '',
    externalIntegrations: [],
    crossChainInteroperability: [],
    primaryUserInteraction: '',
    authenticationMethod: '',
    expectedDailyUsers: 0,
    transactionVolume: 'low',
    dataVolume: 'small',
    userProfileType: '',
    notificationMethods: [],
    governanceModel: '',
    upgradeMechanism: '',
  },
  selectedModules: [],
};

// --- CONTEXT & PROVIDER ---

export const StudioContext = createContext<StudioContextType | undefined>(undefined);

export const StudioProvider = ({ children }: { children: ReactNode }) => {
  const [state, setStateValue] = useState<StudioState>(initialState);

  const setStudioState = (newState: Partial<StudioState>) => {
    setStateValue((prev) => {
      const updated = { ...prev, ...newState };

      if (newState.appMetadata) {
        updated.appMetadata = { ...prev.appMetadata, ...newState.appMetadata };
      }
      if (newState.networkConfig) {
        updated.networkConfig = { ...prev.networkConfig, ...newState.networkConfig };
      }
      if (newState.appGoal) {
        updated.appGoal = { ...prev.appGoal, ...newState.appGoal };
      }

      return updated;
    });
  };

  return <StudioContext.Provider value={{ ...state, setStudioState }}>{children}</StudioContext.Provider>;
};

export const useStudio = () => {
  const context = useContext(StudioContext);
  if (!context) throw new Error('useStudio must be used within a StudioProvider');
  return context;
};
