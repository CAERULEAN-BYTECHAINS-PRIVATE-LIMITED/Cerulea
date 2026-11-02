/**
 * The six roles the PoC document Section 8 requires — "each of the six roles (vendor,
 * procuring entity, nodal ministry administrator, cost or chartered accountant, CVC or
 * audit reviewer, and DPIIT) sees only what its role permits" — plus the two
 * cross-cutting network views.
 *
 * This file is the single source of truth for the navigation. Every label here is short
 * enough to sit in a nav strip and precise enough to be read out to a jury.
 *
 * `devAccount` mirrors `PERSONA_ACCOUNTS` in `src/lib/chain.ts`. The values are repeated
 * here as plain strings on purpose: importing `chain.ts` would pull `@polkadot/api` into
 * the client bundle for what is only ever a caption.
 */

export type PersonaId =
  | 'vendor'
  | 'procuring-entity'
  | 'ministry-admin'
  | 'auditor'
  | 'cvc'
  | 'dpiit';

export interface Persona {
  id: PersonaId;
  route: `/${string}`;
  /** Full institutional title. Used as the page heading. */
  label: string;
  /** What fits in the navigation strip. */
  navLabel: string;
  /** The organisation this console is signed in as. */
  actingAs: string;
  /** The role's remit, in one clause. Read on the portal directory only. */
  remit: string;
  /** Development account this persona signs with. See `chain.ts`. */
  devAccount: string;
}

export const PERSONAS: readonly Persona[] = [
  {
    id: 'vendor',
    route: '/vendor',
    label: 'Vendor',
    navLabel: 'Vendor',
    actingAs: 'Bharat Precision Instruments Pvt Ltd',
    remit: 'Declare local content on a GeM bid and receive the classification before close.',
    devAccount: '//Dave',
  },
  {
    id: 'procuring-entity',
    route: '/procuring-entity',
    label: 'Procuring Entity',
    navLabel: 'Procuring entity',
    actingAs: 'Central Procurement Cell, Northern Railway',
    remit: 'Evaluate bids, check the national debarment ledger, apply purchase preference.',
    devAccount: '//Charlie',
  },
  {
    id: 'ministry-admin',
    route: '/ministry-admin',
    label: 'Nodal Ministry Administrator',
    navLabel: 'Ministry',
    actingAs: 'Ministry of Electronics and Information Technology',
    remit: 'Maintain the ministry rule set and issue debarment orders.',
    devAccount: '//Bob',
  },
  {
    id: 'auditor',
    route: '/auditor',
    label: 'Cost or Chartered Accountant',
    navLabel: 'Auditor',
    actingAs: 'S. Raghavan & Associates, Cost Accountants',
    remit: 'Certify local content on contracts at or above the certification threshold.',
    devAccount: '//Eve',
  },
  {
    id: 'cvc',
    route: '/cvc',
    label: 'Central Vigilance Commission',
    navLabel: 'Vigilance',
    actingAs: 'Central Vigilance Commission',
    remit: 'Read the finalized record: anomalies, contradictions, and the block each was sealed in.',
    devAccount: '//Ferdie',
  },
  {
    id: 'dpiit',
    route: '/dpiit',
    label: 'DPIIT',
    navLabel: 'DPIIT',
    actingAs: 'Department for Promotion of Industry and Internal Trade',
    remit: 'Own the national default rule and monitor every ministry against it.',
    devAccount: '//Alice',
  },
] as const;

const BY_ID = new Map<PersonaId, Persona>(PERSONAS.map((persona) => [persona.id, persona]));

export function getPersona(id: PersonaId): Persona {
  const persona = BY_ID.get(id);
  if (!persona) throw new Error(`Unknown persona: ${id}`);
  return persona;
}

/** Resolve a persona from a pathname such as `/vendor` or `/vendor/bids/GEM-2025-B-6798497`. */
export function personaFromPathname(pathname: string): Persona | undefined {
  return PERSONAS.find(
    (persona) => pathname === persona.route || pathname.startsWith(`${persona.route}/`),
  );
}

/**
 * The two network views, which belong to no single role.
 *
 * `/demo` is deliberately absent. The guided walkthrough is reachable by typing its URL
 * and by nothing else: it is a presenter's tool, not a place a judge should stumble into
 * while looking at the compliance record.
 */
export const NETWORK_ROUTES = [
  { href: '/dashboard', label: 'Analytics' },
  { href: '/explorer', label: 'Explorer' },
] as const;
