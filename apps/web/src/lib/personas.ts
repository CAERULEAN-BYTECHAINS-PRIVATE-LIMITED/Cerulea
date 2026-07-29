/**
 * The six roles the PoC document Section 8 requires — "each of the six roles (vendor,
 * procuring entity, nodal ministry administrator, cost or chartered accountant, CVC or
 * audit reviewer, and DPIIT) sees only what its role permits".
 *
 * This file is the single source of truth for their routes, labels and iconography, so a
 * persona can be added to the nav, the landing page and the switcher from one place.
 *
 * The `devAccount` values mirror `PERSONA_ACCOUNTS` in `src/lib/chain.ts`. They are
 * repeated here as plain strings on purpose: importing `chain.ts` would pull
 * `@polkadot/api` into the client bundle for what is only ever a caption.
 */

import {
  Building2,
  FileCheck2,
  Landmark,
  ScrollText,
  ShieldCheck,
  Store,
  type LucideIcon,
} from 'lucide-react';

export type PersonaId =
  | 'vendor'
  | 'procuring-entity'
  | 'ministry-admin'
  | 'auditor'
  | 'cvc'
  | 'dpiit';

export interface Persona {
  id: PersonaId;
  /** Route this persona's console lives at. */
  route: `/${string}`;
  /** Full institutional title, used as a page heading. */
  label: string;
  /** Compact title for the nav and the persona switcher. */
  shortLabel: string;
  /** One line a judge can read aloud: what this role does in the procurement flow. */
  summary: string;
  /** The concrete actions this console exposes. Real capabilities, not marketing. */
  capabilities: string[];
  /** The demo organisation this persona is signed in as. */
  actingAs: string;
  /** Development account this persona signs with. See `chain.ts`. */
  devAccount: string;
  icon: LucideIcon;
}

export const PERSONAS: readonly Persona[] = [
  {
    id: 'vendor',
    route: '/vendor',
    label: 'Vendor',
    shortLabel: 'Vendor',
    summary:
      'Declares local content against a live GeM bid and sees the classification verdict before the bid closes.',
    capabilities: [
      'Submit a local content declaration for a bid',
      'See the Class I / Class II / Non-local verdict and the rule behind it',
      'Track declaration history across tenders',
    ],
    actingAs: 'Bharat Precision Instruments Pvt Ltd',
    devAccount: '//Dave',
    icon: Store,
  },
  {
    id: 'procuring-entity',
    route: '/procuring-entity',
    label: 'Procuring Entity',
    shortLabel: 'Procuring Entity',
    summary:
      'Evaluates the bids received on a tender and applies the purchase preference the order requires.',
    capabilities: [
      'Run bid evaluation, including the cross-ministry debarment check',
      'Calculate purchase preference and the L1 price-match band',
      'Record the award decision and the pathway it took',
    ],
    actingAs: 'Central Procurement Cell, Northern Railway',
    devAccount: '//Charlie',
    icon: Building2,
  },
  {
    id: 'ministry-admin',
    route: '/ministry-admin',
    label: 'Nodal Ministry Administrator',
    shortLabel: 'Ministry Admin',
    summary:
      'Maintains the ministry rule set — thresholds, Para 3A, PLI linkage — and issues debarments.',
    capabilities: [
      'Amend the ministry rule and set its effective block',
      'Debar or reinstate a vendor with a recorded reason',
      'Review declarations flagged as inconsistent',
    ],
    actingAs: 'Ministry of Electronics and Information Technology',
    devAccount: '//Bob',
    icon: Landmark,
  },
  {
    id: 'auditor',
    route: '/auditor',
    label: 'Cost or Chartered Accountant',
    shortLabel: 'CA / Auditor',
    summary:
      'Certifies local content for contracts at or above the Rs 10 crore certification threshold.',
    capabilities: [
      'Issue a local content certificate against a contract',
      'See which contracts require a statutory auditor and which may self-certify',
      'Review the certificates already on record',
    ],
    actingAs: 'S. Raghavan & Associates, Cost Accountants',
    devAccount: '//Eve',
    icon: FileCheck2,
  },
  {
    id: 'cvc',
    route: '/cvc',
    label: 'CVC / Audit Reviewer',
    shortLabel: 'CVC',
    summary:
      'Reads the finalized record after the fact — every decision, its inputs, and the block it was sealed in.',
    capabilities: [
      'Trace any decision back to the rule version in force at that block',
      'Review inconsistent declarations across tenders and ministries',
      'Export an audit trail for a tender or a vendor',
    ],
    actingAs: 'Central Vigilance Commission',
    devAccount: '//Ferdie',
    icon: ShieldCheck,
  },
  {
    id: 'dpiit',
    route: '/dpiit',
    label: 'DPIIT',
    shortLabel: 'DPIIT',
    summary:
      'Owns the default rule that every ministry inherits, and sees compliance across all of them.',
    capabilities: [
      'Set the national default rule and its effective block',
      'Compare ministry rule sets against the default',
      'Monitor classification outcomes across all 21 ministries',
    ],
    actingAs: 'Department for Promotion of Industry and Internal Trade',
    devAccount: '//Alice',
    icon: ScrollText,
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

/** Routes that are not persona consoles but appear in the primary navigation. */
export const GLOBAL_ROUTES = [
  { href: '/dashboard', label: 'Dashboard' },
  { href: '/explorer', label: 'Explorer' },
  { href: '/demo', label: 'Walkthrough' },
] as const;
