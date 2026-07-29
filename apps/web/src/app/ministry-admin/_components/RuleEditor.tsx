'use client';

import { Plus, Save, Trash2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import {
  Awaiting,
  Button,
  Check,
  Chip,
  DataList,
  DataRow,
  Field,
  Input,
  Notice,
  Panel,
  PanelBody,
  PanelFoot,
  PanelHead,
  PanelNote,
  Select,
  Verdict,
} from '@/components';
import {
  CALCULATION_METHODS,
  DIVISIBILITIES,
  MINISTRIES,
  getMinistry,
  ministryName,
  recordLedgerEntry,
  updateRule,
  type ApiFailure,
  type CalculationMethod,
  type Divisibility,
  type MinistryRef,
  type RuleUpdateResponse,
} from '@/lib/api-client';
import { formatBps, formatPaise, paiseToRupees, percentToBps, rupeesToPaise } from '@/lib/units';

/**
 * The nine rule parameters of `pramaanRuleRegistry::Rule`, edited for one ministry.
 *
 * The validation below is not decorative: it is `pallet-pramaan-rule-registry`'s own
 * `validate_rule` restated in the browser, so an administrator sees why a rule is invalid
 * before a block is spent rejecting it — margin at most 100%, and within every HSN row a
 * Class-I threshold at or above its Class-II threshold, both at most 100%.
 */

const BPS_MAX = 10_000;

interface HsnRow {
  hsnCode: string;
  classOnePercent: string;
  classTwoPercent: string;
}

interface FormState {
  hsn: HsnRow[];
  para3aApplicable: boolean;
  pliLinked: boolean;
  calculationMethod: CalculationMethod;
  marginPercent: string;
  certificationThresholdRupees: string;
  exemptionFloorRupees: string;
  divisibility: Divisibility;
  effectiveFrom: string;
}

function formFor(ministry: MinistryRef): FormState {
  return {
    hsn: ministry.hsnThresholds.map((row) => ({
      hsnCode: row.hsnCode,
      classOnePercent: String(row.classOneBps / 100),
      classTwoPercent: String(row.classTwoBps / 100),
    })),
    para3aApplicable: ministry.para3aApplicable,
    pliLinked: ministry.pliLinked,
    calculationMethod: ministry.calculationMethod,
    marginPercent: String(ministry.preferenceMarginBps / 100),
    certificationThresholdRupees: String(paiseToRupees(ministry.certificationThresholdPaise)),
    exemptionFloorRupees: String(paiseToRupees(ministry.exemptionFloorPaise)),
    divisibility: ministry.divisibility,
    effectiveFrom: String(ministry.effectiveFrom),
  };
}

interface Errors {
  margin?: string;
  certificationThreshold?: string;
  exemptionFloor?: string;
  effectiveFrom?: string;
  hsn: Record<number, string>;
}

function validate(form: FormState): Errors {
  const errors: Errors = { hsn: {} };

  const margin = Number(form.marginPercent);
  if (!Number.isFinite(margin) || margin < 0) {
    errors.margin = 'The preference margin must be a percentage between 0 and 100.';
  } else if (percentToBpsSafe(margin) > BPS_MAX) {
    errors.margin = `InvalidRule: the preference margin cannot exceed 100% (${BPS_MAX} basis points).`;
  }

  form.hsn.forEach((row, index) => {
    if (row.hsnCode.trim() === '') {
      errors.hsn[index] = 'An HSN code is required. Use * to apply the row to every code.';
      return;
    }
    const one = Number(row.classOnePercent);
    const two = Number(row.classTwoPercent);
    if (!Number.isFinite(one) || !Number.isFinite(two) || one < 0 || two < 0) {
      errors.hsn[index] = 'Both thresholds must be percentages between 0 and 100.';
      return;
    }
    if (percentToBpsSafe(one) > BPS_MAX || percentToBpsSafe(two) > BPS_MAX) {
      errors.hsn[index] = `InvalidRule: a threshold cannot exceed 100% (${BPS_MAX} basis points).`;
      return;
    }
    if (percentToBpsSafe(one) < percentToBpsSafe(two)) {
      errors.hsn[index] =
        'InvalidRule: the Class-I threshold cannot be below the Class-II threshold.';
    }
  });

  const threshold = Number(form.certificationThresholdRupees);
  if (!Number.isFinite(threshold) || threshold < 0) {
    errors.certificationThreshold = 'The certification threshold must be an amount in rupees.';
  }
  const floor = Number(form.exemptionFloorRupees);
  if (!Number.isFinite(floor) || floor < 0) {
    errors.exemptionFloor = 'The exemption floor must be an amount in rupees.';
  }
  const block = Number(form.effectiveFrom);
  if (!Number.isInteger(block) || block < 0) {
    errors.effectiveFrom = 'The effective block must be a whole number, zero or greater.';
  }

  return errors;
}

function percentToBpsSafe(percent: number): number {
  return Math.round(percent * 100);
}

function hasErrors(errors: Errors): boolean {
  return (
    Object.keys(errors.hsn).length > 0 ||
    Boolean(errors.margin || errors.certificationThreshold || errors.exemptionFloor || errors.effectiveFrom)
  );
}

export function RuleEditor({ defaultMinistryId }: { defaultMinistryId: string }) {
  const [ministryId, setMinistryId] = useState(defaultMinistryId);
  const seed = getMinistry(defaultMinistryId) ?? MINISTRIES[0];
  const [form, setForm] = useState<FormState>(() => formFor(seed));
  const [touched, setTouched] = useState(false);
  const [pending, setPending] = useState(false);
  const [verdict, setVerdict] = useState<RuleUpdateResponse | null>(null);
  const [failure, setFailure] = useState<ApiFailure | null>(null);

  const errors = useMemo(() => validate(form), [form]);
  const invalid = hasErrors(errors);
  const ministry = getMinistry(ministryId);

  function selectMinistry(id: string) {
    setMinistryId(id);
    const next = getMinistry(id);
    if (next) setForm(formFor(next));
    setVerdict(null);
    setFailure(null);
    setTouched(false);
  }

  function patch(update: Partial<FormState>) {
    setForm((current) => ({ ...current, ...update }));
  }

  function patchHsn(index: number, update: Partial<HsnRow>) {
    setForm((current) => ({
      ...current,
      hsn: current.hsn.map((row, i) => (i === index ? { ...row, ...update } : row)),
    }));
  }

  async function onSubmit() {
    setTouched(true);
    if (invalid) return;

    setPending(true);
    setFailure(null);
    setVerdict(null);

    const outcome = await updateRule({
      ministry: ministryId,
      rule: {
        hsnThresholds: form.hsn.map((row) => ({
          hsnCode: row.hsnCode.trim(),
          classOneBps: percentToBps(Number(row.classOnePercent)),
          classTwoBps: percentToBps(Number(row.classTwoPercent)),
        })),
        para3aApplicable: form.para3aApplicable,
        pliLinked: form.pliLinked,
        calculationMethod: form.calculationMethod,
        preferenceMarginBps: percentToBps(Number(form.marginPercent)),
        certificationThreshold: rupeesToPaise(
          Number(form.certificationThresholdRupees),
        ).toString(),
        exemptionFloor: rupeesToPaise(Number(form.exemptionFloorRupees)).toString(),
        divisibility: form.divisibility,
        effectiveFrom: Number(form.effectiveFrom),
      },
    });
    setPending(false);

    if (!outcome.ok) {
      setFailure(outcome);
      return;
    }

    setVerdict(outcome.data);
    recordLedgerEntry({
      kind: 'rule-update',
      persona: 'Nodal Ministry Administrator',
      result: outcome.data.result,
      headline: `${ministryName(ministryId)} rule set — version ${outcome.data.newVersion ?? '?'}`,
      reason: outcome.data.reason,
      ministry: ministryId,
      txRef: outcome.data.txRef,
      blockNumber: outcome.data.blockNumber,
      facts: {
        'Calculation method': form.calculationMethod,
        'Preference margin': `${form.marginPercent}%`,
        'Effective from block': form.effectiveFrom,
      },
    });
  }

  return (
    <div className="grid grid-cols-1 gap-4 xl:grid-cols-3">
      <Panel className="xl:col-span-2">
        <PanelHead
          title="Rule set"
          meta={<Chip tone="accent">{ministry?.calculationMethod ?? 'Standard'}</Chip>}
        />
        <PanelBody className="space-y-5">
          <Field label="Ministry" required hint="The rule set applies to every tender this ministry buys under.">
            {(props) => (
              <Select
                {...props}
                value={ministryId}
                onChange={(event) => selectMinistry(event.target.value)}
              >
                {MINISTRIES.map((row) => (
                  <option key={row.id} value={row.id}>
                    {row.name}
                  </option>
                ))}
              </Select>
            )}
          </Field>

          {/* ---- 1. HSN thresholds ------------------------------------------- */}
          <fieldset className="space-y-2">
            <legend className="text-xs font-semibold tracking-wide text-ink-2 uppercase">
              Local content thresholds, by HSN code
            </legend>
            <p className="text-2xs text-ink-muted">
              The first row is what the classification pallet applies. Use{' '}
              <span className="font-mono">*</span> to cover every code the ministry buys.
            </p>

            {form.hsn.map((row, index) => (
              <div
                key={index}
                className="rounded-md border border-line bg-shell px-3 py-2.5"
              >
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-[1fr_1fr_1fr_auto] sm:items-end">
                  <Field label="HSN code">
                    {(props) => (
                      <Input
                        {...props}
                        value={row.hsnCode}
                        onChange={(event) => patchHsn(index, { hsnCode: event.target.value })}
                        className="font-mono"
                      />
                    )}
                  </Field>
                  <Field label="Class-I threshold (%)">
                    {(props) => (
                      <Input
                        {...props}
                        type="number"
                        min={0}
                        max={100}
                        step={0.01}
                        value={row.classOnePercent}
                        onChange={(event) =>
                          patchHsn(index, { classOnePercent: event.target.value })
                        }
                      />
                    )}
                  </Field>
                  <Field label="Class-II threshold (%)">
                    {(props) => (
                      <Input
                        {...props}
                        type="number"
                        min={0}
                        max={100}
                        step={0.01}
                        value={row.classTwoPercent}
                        onChange={(event) =>
                          patchHsn(index, { classTwoPercent: event.target.value })
                        }
                      />
                    )}
                  </Field>
                  <Button
                    variant="quiet"
                    aria-label={`Remove the row for HSN ${row.hsnCode || index + 1}`}
                    disabled={form.hsn.length === 1}
                    onClick={() => patch({ hsn: form.hsn.filter((_, i) => i !== index) })}
                    icon={<Trash2 className="size-3.5" aria-hidden="true" />}
                  >
                    Remove
                  </Button>
                </div>
                {/* `form-error`, not `status-red`: a malformed threshold row is a typo, not
                    a compliance verdict. See the token note in globals.css. */}
                {touched && errors.hsn[index] && (
                  <p className="mt-2 text-2xs font-medium text-form-error">
                    <span className="sr-only">Error: </span>
                    {errors.hsn[index]}
                  </p>
                )}
              </div>
            ))}

            <Button
              disabled={form.hsn.length >= 64}
              onClick={() =>
                patch({
                  hsn: [
                    ...form.hsn,
                    { hsnCode: '', classOnePercent: '50', classTwoPercent: '20' },
                  ],
                })
              }
              icon={<Plus className="size-3.5" aria-hidden="true" />}
            >
              Add an HSN row
            </Button>
          </fieldset>

          {/* ---- 2-4. Flags and method ---------------------------------------- */}
          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Check
              checked={form.para3aApplicable}
              onChange={(checked) => patch({ para3aApplicable: checked })}
              label="Para 3A applies"
              hint="Restricts sourcing to Class-I suppliers for items this ministry has notified as having sufficient local capacity."
            />
            <Check
              checked={form.pliLinked}
              onChange={(checked) => patch({ pliLinked: checked })}
              label="PLI-linked"
              hint="Enables the deeming rule that treats a PLI beneficiary as Class-II for the notified period."
            />
          </div>

          <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
            <Field label="Calculation method" required>
              {(props) => (
                <Select
                  {...props}
                  value={form.calculationMethod}
                  onChange={(event) =>
                    patch({ calculationMethod: event.target.value as CalculationMethod })
                  }
                >
                  {CALCULATION_METHODS.map((method) => (
                    <option key={method} value={method}>
                      {method}
                    </option>
                  ))}
                </Select>
              )}
            </Field>

            <Field label="Divisibility" required>
              {(props) => (
                <Select
                  {...props}
                  value={form.divisibility}
                  onChange={(event) =>
                    patch({ divisibility: event.target.value as Divisibility })
                  }
                >
                  {DIVISIBILITIES.map((value) => (
                    <option key={value} value={value}>
                      {value}
                    </option>
                  ))}
                </Select>
              )}
            </Field>

            <Field
              label="Purchase preference margin (%)"
              required
              error={touched ? errors.margin : undefined}
              hint="The band above L1 within which a Class-I supplier may be offered a price match."
            >
              {(props) => (
                <Input
                  {...props}
                  type="number"
                  min={0}
                  max={100}
                  step={0.01}
                  value={form.marginPercent}
                  onChange={(event) => patch({ marginPercent: event.target.value })}
                />
              )}
            </Field>

            <Field
              label="Effective from block"
              required
              error={touched ? errors.effectiveFrom : undefined}
              hint="Every bid evaluated at or after this block is judged against these parameters."
            >
              {(props) => (
                <Input
                  {...props}
                  type="number"
                  min={0}
                  step={1}
                  value={form.effectiveFrom}
                  onChange={(event) => patch({ effectiveFrom: event.target.value })}
                  className="font-mono"
                />
              )}
            </Field>

            <Field
              label="Certification threshold (₹)"
              required
              error={touched ? errors.certificationThreshold : undefined}
              hint={`At or above this value a chartered accountant's certificate is mandatory. Currently ${safeFormatRupees(form.certificationThresholdRupees)}.`}
            >
              {(props) => (
                <Input
                  {...props}
                  type="number"
                  min={0}
                  step={1}
                  value={form.certificationThresholdRupees}
                  onChange={(event) =>
                    patch({ certificationThresholdRupees: event.target.value })
                  }
                  className="font-mono"
                />
              )}
            </Field>

            <Field
              label="Exemption floor (₹)"
              required
              error={touched ? errors.exemptionFloor : undefined}
              hint={`Procurements below this value fall outside the order. Currently ${safeFormatRupees(form.exemptionFloorRupees)}.`}
            >
              {(props) => (
                <Input
                  {...props}
                  type="number"
                  min={0}
                  step={1}
                  value={form.exemptionFloorRupees}
                  onChange={(event) => patch({ exemptionFloorRupees: event.target.value })}
                  className="font-mono"
                />
              )}
            </Field>
          </div>
        </PanelBody>

        <PanelFoot>
          <p className="text-2xs text-ink-muted">
            {invalid && touched
              ? 'The registry would reject this rule set. Fix the errors above.'
              : 'The amended rule is written to the registry and carries a new version number.'}
          </p>
          <Button
            variant="primary"
            onClick={onSubmit}
            loading={pending}
            loadingLabel="Recording the rule change"
            disabled={touched && invalid}
            icon={<Save className="size-3.5" aria-hidden="true" />}
          >
            Record rule change
          </Button>
        </PanelFoot>
      </Panel>

      <div className="space-y-4">
        {pending && <Awaiting label="Recording the rule change" steps={false} />}

        {failure && (
          <Notice
            kind={failure.kind}
            detail={failure.message}
            technicalDetail={failure.technicalDetail}
            onRetry={onSubmit}
          />
        )}

        {verdict && (
          <Verdict
            status={verdict.result}
            reason={verdict.reason}
            trigger={`Rule update · ${ministryName(ministryId)}`}
            txRef={verdict.txRef ?? undefined}
            blockNumber={verdict.blockNumber ?? undefined}
            latencyMs={verdict.latencyMs}
            records={[
              { label: 'Rule version', value: String(verdict.newVersion ?? '—'), mono: true },
              { label: 'Effective from block', value: form.effectiveFrom, mono: true },
              { label: 'Calculation method', value: form.calculationMethod },
              { label: 'Divisibility', value: form.divisibility },
            ]}
          />
        )}

        <Panel>
          <PanelHead title="In force now" />
          <PanelBody>
            {ministry ? (
              <DataList>
                <DataRow label="Ministry id" value={ministry.id} mono />
                <DataRow
                  label="Class-I / Class-II"
                  value={`${formatBps(ministry.hsnThresholds[0].classOneBps)} / ${formatBps(ministry.hsnThresholds[0].classTwoBps)}`}
                />
                <DataRow label="HSN scope" value={ministry.hsnThresholds[0].hsnCode} mono />
                <DataRow label="Calculation method" value={ministry.calculationMethod} />
                <DataRow
                  label="Preference margin"
                  value={formatBps(ministry.preferenceMarginBps)}
                />
                <DataRow
                  label="Certification threshold"
                  value={formatPaise(ministry.certificationThresholdPaise)}
                />
                <DataRow
                  label="Exemption floor"
                  value={formatPaise(ministry.exemptionFloorPaise)}
                />
                <DataRow label="Divisibility" value={ministry.divisibility} />
                <DataRow label="Para 3A" value={ministry.para3aApplicable ? 'Applies' : 'Does not apply'} />
                <DataRow label="PLI-linked" value={ministry.pliLinked ? 'Yes' : 'No'} />
              </DataList>
            ) : null}
          </PanelBody>
          <PanelNote>The rule set seeded on chain for this ministry.</PanelNote>
        </Panel>
      </div>
    </div>
  );
}

function safeFormatRupees(rupees: string): string {
  const value = Number(rupees);
  if (!Number.isFinite(value) || value < 0) return 'not a valid amount';
  return formatPaise(rupeesToPaise(value));
}
