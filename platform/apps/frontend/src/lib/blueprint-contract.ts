/**
 * Canonical Blueprint schema contract aligned with `cerulea-blueprint` (Rust).
 *
 * Enforces:
 * - BLP-019: 4 MiB serialized Blueprint limit & 10,000 section item limit
 * - BLP-031: Explicit `substrate` target (`native` | `wasm` | `evm`)
 * - BLP-006: DPDP Act 2023 personal-data, lawful basis, consent tracking, & erasure rule checks
 * - BLP-011: Workflow non-terminal dead-end state detection
 * - BLP-015/GOV-009: Multi-organisation governance threshold & member validation
 */

export const MAX_BLUEPRINT_BYTES = 4 * 1024 * 1024;
export const MAX_BLUEPRINT_COLLECTION_ITEMS = 10_000;

export type SubstrateTarget = 'native' | 'wasm' | 'evm';

export interface BlueprintDiagnostic {
  code:
    | 'BoundExceeded'
    | 'MissingValue'
    | 'ValueOutOfRange'
    | 'SectionEmpty'
    | 'DuplicateIdentifier'
    | 'InvalidSubstrate';
  location: string;
  problem: string;
  fix: string;
}

export function validateBlueprintContract(raw: unknown): BlueprintDiagnostic[] {
  const diagnostics: BlueprintDiagnostic[] = [];
  if (!raw || typeof raw !== 'object') {
    diagnostics.push({
      code: 'MissingValue',
      location: 'blueprint',
      problem: 'Blueprint payload must be a JSON object.',
      fix: 'Supply a valid Blueprint JSON document.',
    });
    return diagnostics;
  }

  const serializedBytes = Buffer.byteLength(JSON.stringify(raw), 'utf8');
  if (serializedBytes > MAX_BLUEPRINT_BYTES) {
    diagnostics.push({
      code: 'BoundExceeded',
      location: 'blueprint',
      problem: `Serialized Blueprint is ${serializedBytes} bytes, past the ${MAX_BLUEPRINT_BYTES} byte limit.`,
      fix: `Keep the serialized Blueprint at or below ${MAX_BLUEPRINT_BYTES} bytes.`,
    });
    return diagnostics;
  }

  const bp = raw as Record<string, any>;

  // BLP-031: Validate explicit substrate target when provided.
  const substrate = bp.meta?.substrate;
  if (substrate !== undefined) {
    const normalized = typeof substrate === 'string' ? substrate.toLowerCase() : '';
    if (!['native', 'wasm', 'evm'].includes(normalized)) {
      diagnostics.push({
        code: 'InvalidSubstrate',
        location: 'meta.substrate',
        problem: `Unsupported substrate target "${String(substrate)}".`,
        fix: 'Set meta.substrate to "native", "wasm", or "evm".',
      });
    }
  }

  // BLP-019: Section collection bounds.
  for (const section of ['types', 'collections', 'actions', 'workflows', 'events', 'views', 'blocks']) {
    const arr = bp[section];
    if (Array.isArray(arr) && arr.length > MAX_BLUEPRINT_COLLECTION_ITEMS) {
      diagnostics.push({
        code: 'BoundExceeded',
        location: section,
        problem: `Section "${section}" holds ${arr.length} entries, past the ${MAX_BLUEPRINT_COLLECTION_ITEMS} limit.`,
        fix: `Keep "${section}" at or below ${MAX_BLUEPRINT_COLLECTION_ITEMS} entries.`,
      });
    }
  }

  // BLP-006: Personal-data, lawful basis, consent tracking, and erasure rule checks.
  if (Array.isArray(bp.collections)) {
    bp.collections.forEach((col: Record<string, any>, idx: number) => {
      const loc = `collections[${idx}]`;
      const isPersonal = Boolean(col?.personal_data);
      const hasBasis = col?.lawful_basis !== undefined && col?.lawful_basis !== null;
      const hasConsent = Boolean(col?.consent_tracking);
      const hasErasure = Boolean(col?.erasure_rule);

      if (!isPersonal && (hasBasis || hasConsent || hasErasure)) {
        diagnostics.push({
          code: 'ValueOutOfRange',
          location: `${loc}.personal_data`,
          problem: `Collection "${col?.id ?? idx}" sets personal_data=false while declaring compliance fields.`,
          fix: 'Set personal_data to true or remove lawful_basis, consent_tracking, and erasure_rule.',
        });
      } else if (isPersonal) {
        if (!hasBasis) {
          diagnostics.push({
            code: 'MissingValue',
            location: `${loc}.lawful_basis`,
            problem: `Personal-data collection "${col?.id ?? idx}" declares no lawful basis.`,
            fix: 'Declare Consent or LegitimateUse.',
          });
        } else if (
          (col.lawful_basis === 'Consent' || col.lawful_basis?.Consent !== undefined) &&
          !hasConsent
        ) {
          diagnostics.push({
            code: 'MissingValue',
            location: `${loc}.consent_tracking`,
            problem: `Consent-based collection "${col?.id ?? idx}" names no consent tracking rule.`,
            fix: 'Name a rule governing consent recording and withdrawal.',
          });
        }
        if (!hasErasure) {
          diagnostics.push({
            code: 'MissingValue',
            location: `${loc}.erasure_rule`,
            problem: `Personal-data collection "${col?.id ?? idx}" names no erasure rule.`,
            fix: 'Name an executable erasure rule.',
          });
        }
      }
    });
  }

  // BLP-011: Workflow dead-end non-terminal state checks.
  if (Array.isArray(bp.workflows)) {
    bp.workflows.forEach((wf: Record<string, any>, wfIdx: number) => {
      if (!Array.isArray(wf?.states)) return;
      const transitions = Array.isArray(wf.transitions) ? wf.transitions : [];
      const timers = Array.isArray(wf.timers) ? wf.timers : [];
      const deadlines = Array.isArray(wf.deadlines) ? wf.deadlines : [];

      wf.states.forEach((st: Record<string, any>, stIdx: number) => {
        if (st && st.final_state === false) {
          const id = st.id;
          const hasTxExit = transitions.some((t: any) => t?.from === id && t?.to !== id);
          const hasTimerExit = timers.some(
            (t: any) => t?.state === id && t?.on_expiry?.MoveTo && t.on_expiry.MoveTo !== id
          );
          const hasDeadlineExit = deadlines.some(
            (d: any) => d?.state === id && d?.on_miss?.MoveTo && d.on_miss.MoveTo !== id
          );
          if (!hasTxExit && !hasTimerExit && !hasDeadlineExit) {
            diagnostics.push({
              code: 'MissingValue',
              location: `workflows[${wfIdx}].states[${stIdx}].final_state`,
              problem: `Non-terminal state "${String(id)}" has no outgoing transition.`,
              fix: `Add an outgoing transition from "${String(id)}", or mark it as a final state.`,
            });
          }
        }
      });
    });
  }

  // BLP-015 / GOV-009: Multi-organisation governance validation.
  const multiOrg = bp.governance?.authority?.multi_org;
  if (multiOrg && typeof multiOrg === 'object') {
    const orgs = Array.isArray(multiOrg.organisations) ? multiOrg.organisations : [];
    const reqOrgs = Number(multiOrg.required_organisations ?? 0);
    if (orgs.length < 2) {
      diagnostics.push({
        code: 'ValueOutOfRange',
        location: 'governance.authority.multi_org.organisations',
        problem: `Multi-organisation governance declares ${orgs.length} organisation(s), below 2.`,
        fix: 'Declare at least two independent organisations.',
      });
    }
    if (reqOrgs < 2 || reqOrgs > orgs.length) {
      diagnostics.push({
        code: 'ValueOutOfRange',
        location: 'governance.authority.multi_org.required_organisations',
        problem: `required_organisations (${reqOrgs}) must be between 2 and ${orgs.length}.`,
        fix: 'Set required_organisations between 2 and the number of declared organisations.',
      });
    }
    const seen = new Set<string>();
    orgs.forEach((org: Record<string, any>, idx: number) => {
      const id = String(org?.id ?? '').trim();
      if (!id) {
        diagnostics.push({
          code: 'MissingValue',
          location: `governance.authority.multi_org.organisations[${idx}].id`,
          problem: 'Organisation identifier is empty.',
          fix: 'Provide a unique organisation identifier.',
        });
      } else if (seen.has(id)) {
        diagnostics.push({
          code: 'DuplicateIdentifier',
          location: `governance.authority.multi_org.organisations[${idx}].id`,
          problem: `Duplicate organisation identifier "${id}".`,
          fix: `Rename one of the "${id}" organisations.`,
        });
      } else {
        seen.add(id);
      }
      if (!Array.isArray(org?.roles) || org.roles.length === 0) {
        diagnostics.push({
          code: 'SectionEmpty',
          location: `governance.authority.multi_org.organisations[${idx}].roles`,
          problem: `Organisation "${id}" names zero roles.`,
          fix: 'Name at least one authorised role.',
        });
      }
    });
  }

  return diagnostics;
}
