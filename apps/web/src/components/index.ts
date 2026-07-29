/**
 * The shared UI foundation for the CBC-PRAMAAN console.
 *
 * Pages import from here rather than reaching into individual files, so a component can be
 * reshaped in one place without a sweep across nine routes.
 */

export { AppShell } from './AppShell';
export { Awaiting } from './Awaiting';
export { ChainStatus, type ChainStatusData } from './ChainStatus';
export { BlockRef, Hash } from './Hash';
export { Notice, StaleBanner, type ErrorKind } from './Notice';
export {
  asPathwayId,
  PathwayChip,
  PATHWAYS,
  PATHWAY_IDS,
  type PathwayDefinition,
  type PathwayId,
} from './Pathway';
export { SiteNav } from './SiteNav';
export {
  Verdict,
  type ComplianceBlocker,
  type ComplianceStatus,
  type VerdictProps,
} from './Verdict';
export { Mark, Wordmark } from './Wordmark';

export * from './ui';
