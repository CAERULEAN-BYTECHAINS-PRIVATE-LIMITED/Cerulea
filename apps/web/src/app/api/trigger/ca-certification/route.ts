/**
 * Trigger point 4 — chartered-accountant certification (PathwayId P11).
 *
 * Below the ministry's certification threshold a vendor self-certifies. At or above it
 * a chartered accountant's certificate is mandatory, and the 19.07.2024 amendment
 * places that obligation at execution rather than at bidding — so an above-threshold
 * contract with no certificate yet is a pending obligation (YELLOW), not a violation.
 * Calls `pallet_pramaan_certification::certify`.
 *
 * Request  { vendor, tender, ministry, valuePaise, auditor?, certificateId? }
 * Response { result, certificateId, requiresAuditor, reason, txRef, blockNumber, latencyMs }
 */

import { getApi, getSigner, submitAndFinalize } from '@/lib/chain';
import {
  handleTrigger,
  certifyTx,
  optionalAccount,
  readEffectiveRule,
  requireEvent,
  requireId,
  requirePaise,
  resolveAccount,
  rupees,
  type TriState,
} from '@/lib/pramaan';

export const dynamic = 'force-dynamic';
export const runtime = 'nodejs';

export async function POST(request: Request): Promise<Response> {
  // Captured by both the handler and the rejection hook below, so a RED produced by a
  // pallet rejection still reports which certificate id was attempted.
  let certificateIdText = '';
  let requiresAuditor = false;

  return handleTrigger(
    request,
    async (body) => {
      const startedAt = Date.now();

      const vendor = await resolveAccount(body, 'vendor');
      const tender = requireId(body, 'tender');
      // FLAGGED — spec vs. source. The build contract's route table omits `ministry`,
      // but `certify` requires it and the pallet's own doc comment says why: there is no
      // other way to read `Rules[ministry].certification_threshold`, and without the
      // threshold there is no way to know whether an auditor is mandatory.
      const ministry = requireId(body, 'ministry');
      const valuePaise = requirePaise(body, 'valuePaise', 'value');
      const auditor = await optionalAccount(body, 'auditor');

      // The pallet requires a caller-supplied, unique certificate id; the route table
      // shows `certificateId` only on the response. Both readings are honoured: a
      // caller may supply one, and one is minted when they do not.
      certificateIdText =
        typeof body.certificateId === 'string' && body.certificateId.trim().length > 0
          ? body.certificateId.trim()
          : `CERT-${crypto.randomUUID()}`;
      const certificateId = requireId({ certificateId: certificateIdText }, 'certificateId');

      const api = await getApi();

      // The threshold is a rule parameter, not a result, so reading it from the registry
      // is not a substitute for reading the outcome from the finalized event — it is
      // what decides whether an extrinsic should be submitted at all.
      const rule = await readEffectiveRule(api, ministry);
      requiresAuditor = rule !== null && valuePaise >= rule.certificationThresholdPaise;

      if (requiresAuditor && auditor === null) {
        // Deliberately no extrinsic. The pallet would reject this with `AuditorRequired`,
        // but a missing certificate on an above-threshold contract is not a failed
        // compliance test — it is an obligation that falls due at execution. Build
        // contract section 3 puts exactly this case in YELLOW: "an above-threshold
        // certification still awaiting its auditor certificate".
        const threshold = rule ? rupees(rule.certificationThresholdPaise) : 'the applicable threshold';
        return {
          result: 'YELLOW' satisfies TriState,
          certificateId: certificateIdText,
          requiresAuditor: true,
          reason:
            `Needs a human: at ${rupees(valuePaise)} this contract is at or above the ` +
            `${threshold} certification threshold for this ministry, so a chartered accountant's ` +
            `certificate is mandatory before execution and none has been supplied yet.`,
          txRef: null,
          blockNumber: null,
          latencyMs: Date.now() - startedAt,
        };
      }

      // Who signs: an auditor-backed certificate is lodged by the certifying auditor,
      // whose account the Auditor Accountability Ledger then binds every one of their
      // certificates to. A self-certification below threshold is the vendor's own
      // declaration, so the vendor signs it. (Both are `ensure_signed` on chain; the
      // pallets defer role enforcement to this layer by design.)
      const signer = await getSigner(auditor === null ? 'vendor' : 'auditor');

      const result = await submitAndFinalize(
        certifyTx(api, { certificateId, ministry, vendor, tender, valuePaise, auditor }),
        signer,
      );

      requireEvent(result, 'pramaanCertification', 'Certified');

      const reason =
        auditor === null
          ? `Compliant: at ${rupees(valuePaise)} this contract is below this ministry's auditor-certificate threshold, so the vendor's self-certification is accepted and recorded on chain.`
          : `Compliant: the certificate is recorded and permanently bound to the certifying auditor in the Auditor Accountability Ledger.`;

      return {
        result: 'GREEN' satisfies TriState,
        certificateId: certificateIdText,
        requiresAuditor,
        reason,
        txRef: result.txRef,
        blockNumber: result.blockNumber,
        latencyMs: result.latencyMs,
      };
    },
    {
      onPalletRejection: (error) => ({
        certificateId: certificateIdText,
        requiresAuditor,
        // `AuditorRequired` is the one pallet rejection on this route that is not a
        // compliance failure. It can only be reached by a race — the pre-flight check
        // above normally catches it — but if the rule's threshold changed between the
        // read and the block, the answer is still "awaiting the certificate", not
        // "blocked". Everything else keeps the shared RED mapping.
        ...(error.palletError === 'AuditorRequired'
          ? {
              result: 'YELLOW' satisfies TriState,
              reason:
                "Needs a human: this contract is at or above its ministry's certification " +
                "threshold, so a chartered accountant's certificate is mandatory before " +
                'execution and none has been supplied yet.',
            }
          : {}),
      }),
    },
  );
}
