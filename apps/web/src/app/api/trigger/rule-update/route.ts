/**
 * Trigger point 6 — rule update.
 *
 * DPIIT changes a ministry's nine rule parameters. This is the mechanism behind the
 * claim that adding a ministry or amending a threshold is a configuration change rather
 * than an engineering one: the new rule is a storage write, it carries a version number
 * and an effective block, and every bid evaluated after that block is judged against it
 * without anything being redeployed. Calls
 * `pallet_pramaan_rule_registry::set_rule`.
 *
 * Request  { ministry, rule: { ...nine parameters } }
 * Response { result, status, newVersion, reason, txRef, blockNumber, latencyMs }
 */

import { getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  assertSudoInnerCallSucceeded,
  buildChainRule,
  coerceUnsignedInteger,
  handleTrigger,
  requireEventField,
  requireId,
  requireIdText,
  setRuleSudoTx,
  type TriState,
} from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function POST(request: Request): Promise<Response> {
  return handleTrigger(request, async (body) => {
    const ministry = requireId(body, 'ministry');
    const ministryText = requireIdText(body, 'ministry');
    const rule = buildChainRule(body.rule);

    const api = await getApi();
    // Rule changes are DPIIT's alone. See `setRuleSudoTx` for why the call is dispatched
    // through sudo: the runtime wires `DpiitOrigin = EnsureRoot`, so DPIIT authority is
    // exercised as Root through pallet_sudo in this build, and a plain signed `set_rule`
    // would be rejected with `NotAuthorised`.
    const signer = await getSigner('dpiit');

    const result = await submitAndFinalize(setRuleSudoTx(api, { ministry, rule }), signer);

    // sudo reports the inner call's failure inside its own `Sudid` event instead of as a
    // dispatch error on the extrinsic, so without this an `InvalidRule` rejection would
    // be reported as a successful rule change.
    assertSudoInnerCallSucceeded(api, result);

    const newVersionRaw = requireEventField(
      result,
      'pramaanRuleRegistry',
      'RuleUpdated',
      'newVersion',
    );
    const newVersion = coerceUnsignedInteger(newVersionRaw);
    if (newVersion === null) {
      throw new Error(
        `pramaanRuleRegistry.RuleUpdated carried an unreadable newVersion: ${String(newVersionRaw)}`,
      );
    }

    return {
      result: 'GREEN' satisfies TriState,
      status: 'RuleUpdated',
      newVersion: Number(newVersion),
      reason:
        `Recorded: ${ministryText}'s rule set is now at version ${newVersion} and takes effect from ` +
        `block ${rule.effectiveFrom}, so every bid evaluated after that block is judged against ` +
        `these parameters without any software being redeployed.`,
      txRef: result.txRef,
      blockNumber: result.blockNumber,
      latencyMs: result.latencyMs,
    };
  });
}
