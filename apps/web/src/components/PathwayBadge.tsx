'use client';

import { GitBranch } from 'lucide-react';
import { Badge } from './ui/Badge';
import { cn } from './ui/cn';
import { Tooltip } from './ui/Tooltip';

export type PathwayId =
  | 'P1'
  | 'P2'
  | 'P3'
  | 'P4'
  | 'P5'
  | 'P6'
  | 'P7'
  | 'P8'
  | 'P9'
  | 'P10'
  | 'P11'
  | 'P12';

export interface PathwayDefinition {
  id: PathwayId;
  /** Four or five words, for the badge face. */
  title: string;
  /** The PoC document's own wording, transcribed from `pramaan-primitives/src/lib.rs`. */
  description: string;
  /** The pallet that owns this pathway. */
  pallet: string;
}

/**
 * The twelve decision pathways (tech spec Part 4.2 / PoC document Section 4.2, Figure 7).
 *
 * `description` is transcribed from the doc comments on `PathwayId` in
 * `pramaan-primitives/src/lib.rs`, which are themselves verbatim from the submission. When
 * a judge asks "what is P8", the answer on screen is the answer in the document.
 */
export const PATHWAYS: Record<PathwayId, PathwayDefinition> = {
  P1: {
    id: 'P1',
    title: 'Standard formula',
    description: 'P1 applies the standard formula against the applicable threshold.',
    pallet: 'pramaan-classification',
  },
  P2: {
    id: 'P2',
    title: 'Component-level',
    description:
      'P2 handles component-level and weighted-module methods, validating each component against its own condition before aggregating.',
    pallet: 'pramaan-classification',
  },
  P3: {
    id: 'P3',
    title: 'Recorded human decision',
    description:
      'P3 covers categories where no automatable formula exists and routes them to a recorded human decision rather than computing one.',
    pallet: 'pramaan-classification',
  },
  P4: {
    id: 'P4',
    title: 'PLI deeming rule',
    description:
      'P4 applies the PLI deeming rule, which treats a manufacturer as Class-II only where the incentive has been received and only for the period the PLI ministry notified.',
    pallet: 'pramaan-classification',
  },
  P5: {
    id: 'P5',
    title: 'Sub Rs 200 crore restriction',
    description: 'P5 enforces the sub Rupees 200 crore domestic restriction.',
    pallet: 'pramaan-preference',
  },
  P6: {
    id: 'P6',
    title: 'Global tender enquiry',
    description:
      'P6 admits Non-local suppliers only where a global tender enquiry has been approved under GFR Rule 161(iv).',
    pallet: 'pramaan-preference',
  },
  P7: {
    id: 'P7',
    title: 'Para 3A restriction',
    description:
      'P7 enforces Para 3A, restricting sourcing to Class-I suppliers for items a nodal ministry has notified as having sufficient local capacity, in system integration, EPC, turnkey, and service tenders.',
    pallet: 'pramaan-preference',
  },
  P8: {
    id: 'P8',
    title: 'Divisible award',
    description:
      'P8 and P9 split on divisibility. A Class-I bid priced at or below L1 multiplied by 1.20 is offered the chance to match L1 price. P8 is the divisible award.',
    pallet: 'pramaan-preference',
  },
  P9: {
    id: 'P9',
    title: 'Non-divisible award',
    description:
      'As P8, for a non-divisible tender: the lowest Class-I within the band is offered a price match for the full contract.',
    pallet: 'pramaan-preference',
  },
  P10: {
    id: 'P10',
    title: 'MSE preference overlay',
    description:
      'P10 overlays the MSE preference where both preferences are concurrently active. The Department of Expenditure memorandum of 18.05.2023 governs how the two apply together.',
    pallet: 'pramaan-preference',
  },
  P11: {
    id: 'P11',
    title: 'Certification obligation',
    description:
      'P11 carries the certification obligation, which the 19.07.2024 amendment places at execution rather than at bidding.',
    pallet: 'pramaan-certification',
  },
  P12: {
    id: 'P12',
    title: 'False declaration',
    description:
      'P12 handles the consequence of a false declaration: a downgrade in class triggers a penalty of up to 10 percent of contract value, and debarment of up to two years follows under GFR Rule 151(iii).',
    pallet: 'pramaan-debarment',
  },
};

export const PATHWAY_IDS = Object.keys(PATHWAYS) as PathwayId[];

/**
 * Shows which of P1–P12 a decision took.
 *
 * Deliberately brand-toned, never a status colour: the pathway is *how* the decision was
 * reached, not whether it passed. Hovering or focusing the badge gives the PoC document's
 * own sentence for that pathway.
 */
export function PathwayBadge({
  pathway,
  showTitle = true,
  className,
}: {
  pathway: PathwayId;
  showTitle?: boolean;
  className?: string;
}) {
  const definition = PATHWAYS[pathway];

  return (
    <Tooltip
      content={
        <span>
          <span className="font-semibold">
            Pathway {definition.id} — {definition.title}
          </span>
          <br />
          {definition.description}
        </span>
      }
    >
      <button
        type="button"
        aria-label={`Decision pathway ${definition.id}, ${definition.title}`}
        className={cn(
          'rounded-full focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-cerulea',
          className,
        )}
      >
        <Badge tone="brand" icon={<GitBranch className="size-3" aria-hidden="true" />}>
          <span className="font-mono font-semibold">{definition.id}</span>
          {showTitle && (
            <span className="font-normal text-cerulea-dark/80">{definition.title}</span>
          )}
        </Badge>
      </button>
    </Tooltip>
  );
}
