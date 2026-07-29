/**
 * The shared UI foundation for the CBC-PRAMAAN console.
 *
 * Persona pages should import from here rather than reaching into individual files, so a
 * component can be reshaped in one place without a sweep across six consoles.
 */

export { AppShell, SiteFooter } from './AppShell';
export { ChainStatus, type ChainStatusData } from './ChainStatus';
export {
  ComplianceResult,
  type ComplianceBlocker,
  type ComplianceResultProps,
  type ComplianceStatus,
} from './ComplianceResult';
export { ErrorState, type ErrorKind } from './ErrorState';
export { FinalityPending } from './FinalityPending';
export { MainNav } from './MainNav';
export {
  PathwayBadge,
  PATHWAYS,
  PATHWAY_IDS,
  type PathwayDefinition,
  type PathwayId,
} from './PathwayBadge';
export { PersonaCard } from './PersonaCard';
export { PersonaSwitcher } from './PersonaSwitcher';
export { TxRef } from './TxRef';
export { Mark, Wordmark } from './Wordmark';

export * from './ui';
