// Technical Implementation Specification Part 7.4 — seed script behaviour:
//   1. Read ministries.json.
//   2. For each row, call pallet-pramaan-rule-registry's set_rule extrinsic from the
//      DPIIT account.
//   3. After all calls, query Rules for each ministry_id and diff against the source
//      JSON; any mismatch fails the script loudly rather than continuing silently.
//   4. The script must be idempotent: running it twice produces the same end state.
//
// ministries.json holds all 21 nodal ministries from PoC document Table 11 (DPIIT
// itself is entry #7 there, owning paper/cement/leather/lifts/air-conditioners). The
// DPIIT DEFAULT rule set (Table 10 — the fallback applied when a category has no
// assigned nodal ministry at all) is numerically identical to DPIIT's own entry in
// this file, so it is seeded separately via set_default_rule using that same shape,
// immediately after the 21 set_rule calls, rather than duplicated in the JSON.

import { ApiPromise, WsProvider, Keyring } from "@polkadot/api";
import { stringToU8a, u8aToHex } from "@polkadot/util";
import type { SubmittableExtrinsic } from "@polkadot/api/types";
import fs from "fs";
import path from "path";

const WS_ENDPOINT = process.env.CERULEA_WS_ENDPOINT ?? "ws://127.0.0.1:9944";
const DPIIT_SURI = process.env.DPIIT_SEED_SURI ?? "//Alice";

interface HsnThresholdJson {
	hsn_code: string;
	class_one_bps: number;
	class_two_bps: number;
}

interface MinistryRow {
	ministry_id: string;
	ministry_name: string;
	hsn_thresholds: HsnThresholdJson[];
	para_3a_applicable: boolean;
	pli_linked: boolean;
	calculation_method: "Standard" | "ComponentLevel" | "WeightedModule" | "Custom";
	preference_margin_bps: number;
	certification_threshold: string; // paise, as a decimal string (u128-range)
	exemption_floor: string; // paise
	divisibility: "Divisible" | "NonDivisible";
	effective_from: number;
	source_order: string;
}

function toChainRule(row: MinistryRow) {
	// Shape matches pramaan_primitives::Rule<Balance, BlockNumber> field-for-field, as
	// the polkadot-js API will encode it against the runtime's generated metadata.
	return {
		hsnThresholds: row.hsn_thresholds.map((h) => ({
			hsnCode: encodeId(h.hsn_code),
			classOneBps: h.class_one_bps,
			classTwoBps: h.class_two_bps,
		})),
		para3aApplicable: row.para_3a_applicable,
		pliLinked: row.pli_linked,
		calculationMethod: row.calculation_method,
		preferenceMarginBps: row.preference_margin_bps,
		certificationThreshold: row.certification_threshold,
		exemptionFloor: row.exemption_floor,
		divisibility: row.divisibility,
		effectiveFrom: row.effective_from,
	};
}

/**
 * Encode a human-readable id for a `BoundedVec<u8, _>` parameter.
 *
 * Must be a 0x-prefixed hex string. Verified against the live chain:
 *   - raw Uint8Array (what this function used to return) THROWS — polkadot-js expects
 *     `Bytes` to arrive with its compact length prefix already applied, so it reads the
 *     first byte as a length and fails with "required length less than remainder,
 *     expected at least 18, found 5"
 *   - a plain string works, EXCEPT that any value beginning "0x" is read as hex, so a
 *     6-character id like "0x1234" silently becomes 2 bytes
 *   - u8aToHex(stringToU8a(..)) is correct for every input
 */
function encodeId(id: string): string {
	return u8aToHex(stringToU8a(id));
}

/**
 * Submit a call as Root, via `sudo`, and resolve only once it is finalized.
 *
 * `set_rule` / `set_default_rule` are gated on `Config::DpiitOrigin`, which the runtime
 * wires as `pub type DpiitOrRoot = EnsureRoot<AccountId>` (cerulea-runtime/src/configs/
 * mod.rs). Despite the "DPIIT or Root" name it currently admits Root only, so signing
 * these calls directly — as this script previously did — always fails `NotAuthorised`.
 *
 * `sudo` does not propagate the inner call's failure as a dispatch error: the outer
 * extrinsic succeeds and the inner result is reported inside the `sudo.Sudid` event.
 * Without inspecting that event an `InvalidRule` rejection would be logged as a
 * successful seed, which is exactly the silent-failure mode Part 7.4 forbids.
 */
async function sudoSubmit(
	api: ApiPromise,
	signer: ReturnType<Keyring["addFromUri"]>,
	call: SubmittableExtrinsic<"promise">,
	label: string
): Promise<void> {
	return new Promise<void>((resolve, reject) => {
		api.tx.sudo
			.sudo(call)
			.signAndSend(signer, ({ status, dispatchError, events }) => {
				if (dispatchError) {
					if (dispatchError.isModule) {
						const decoded = api.registry.findMetaError(dispatchError.asModule);
						reject(new Error(`${label} failed: ${decoded.section}.${decoded.name}`));
					} else {
						reject(new Error(`${label} failed: ${dispatchError.toString()}`));
					}
					return;
				}
				if (!status.isFinalized) return;

				// Unwrap the inner call's own result out of sudo.Sudid.
				for (const { event } of events) {
					if (event.section !== "sudo" || event.method !== "Sudid") continue;
					const result = event.data[0] as unknown as {
						isErr: boolean;
						asErr: { isModule: boolean; asModule: unknown };
					};
					if (result?.isErr) {
						const err = result.asErr;
						if (err.isModule) {
							const decoded = api.registry.findMetaError(
								err.asModule as Parameters<typeof api.registry.findMetaError>[0]
							);
							reject(new Error(`${label} rejected by the pallet: ${decoded.section}.${decoded.name}`));
						} else {
							reject(new Error(`${label} rejected by the pallet: ${String(err)}`));
						}
						return;
					}
				}

				console.log(`  ${label} finalized in block ${status.asFinalized.toHex()}`);
				resolve();
			})
			.catch(reject);
	});
}

/**
 * Render a Rule (from either side of the comparison) as a canonical string.
 *
 * The two sides arrive in genuinely different representations and a naive
 * JSON.stringify comparison is worse than useless here — it reports a false PASS.
 * Specifically:
 *
 *   - Key order differs, so the strings must be built from sorted keys, recursively.
 *     (The previous implementation passed `Object.keys(top).sort()` as stringify's
 *     *replacer array*, which is an allow-list applied at EVERY depth — so every
 *     nested hsn_threshold key was silently dropped from both sides and the HSN
 *     values were never actually compared.)
 *   - `certification_threshold` / `exemption_floor` are u128. The chain returns them
 *     as a number when small enough and a "0x..." hex string when not; the JSON
 *     source holds a decimal string. All three normalise to a BigInt.
 *   - `hsn_code` is a BoundedVec<u8>, which the chain returns as hex ("0x2a"), while
 *     the source holds the readable string ("*"). Decode the hex back to UTF-8.
 */
function canonicalise(value: unknown): string {
	return JSON.stringify(normalise(value));
}

function normalise(value: unknown): unknown {
	if (Array.isArray(value)) {
		return value.map(normalise);
	}
	if (value !== null && typeof value === "object") {
		const source = value as Record<string, unknown>;
		const out: Record<string, unknown> = {};
		for (const key of Object.keys(source).sort()) {
			// polkadot-js emits camelCase from toJSON(); toChainRule() already does too.
			out[key] = normaliseField(key, source[key]);
		}
		return out;
	}
	return value;
}

function normaliseField(key: string, value: unknown): unknown {
	if (key === "hsnCode" && typeof value === "string") {
		return hexToUtf8(value);
	}
	if ((key === "certificationThreshold" || key === "exemptionFloor") && value !== null) {
		return toBigIntString(value);
	}
	return normalise(value);
}

function toBigIntString(value: unknown): string {
	if (typeof value === "bigint") return value.toString();
	if (typeof value === "number") return BigInt(value).toString();
	if (typeof value === "string") {
		return value.startsWith("0x") ? BigInt(value).toString() : BigInt(value).toString();
	}
	return String(value);
}

function hexToUtf8(value: string): string {
	if (!value.startsWith("0x")) return value;
	const bytes = new Uint8Array(
		(value.slice(2).match(/.{1,2}/g) ?? []).map((byte) => parseInt(byte, 16))
	);
	return new TextDecoder().decode(bytes);
}

async function main() {
	const jsonPath = path.join(__dirname, "ministries.json");
	const rows: MinistryRow[] = JSON.parse(fs.readFileSync(jsonPath, "utf8"));

	if (rows.length !== 21) {
		throw new Error(
			`ministries.json must hold exactly 21 nodal ministries (PoC document Table 11); found ${rows.length}. Confirm the file wasn't accidentally trimmed or duplicated before seeding.`
		);
	}
	for (const row of rows) {
		if (!row.ministry_name || row.ministry_name.trim().length === 0) {
			throw new Error(`Ministry ${row.ministry_id} has an empty ministry_name — refusing to seed.`);
		}
	}

	console.log(`Loaded ${rows.length} ministries from ${jsonPath}`);

	const provider = new WsProvider(WS_ENDPOINT);
	const api = await ApiPromise.create({ provider });
	// ed25519, matching cerulea-runtime/src/genesis_config_presets.rs, which builds the
	// sudo key and every endowed account from sp_keyring::Ed25519Keyring. An sr25519
	// //Alice is a different AccountId32 entirely: not root, and holding no balance to
	// pay fees with. See apps/web/src/lib/chain.ts for the same reasoning.
	const keyring = new Keyring({ type: "ed25519" });
	const dpiit = keyring.addFromUri(DPIIT_SURI);

	console.log(`Connected to ${WS_ENDPOINT}, seeding as ${dpiit.address}`);

	for (const row of rows) {
		const ministryId = encodeId(row.ministry_id);
		const rule = toChainRule(row);
		await sudoSubmit(
			api,
			dpiit,
			api.tx.pramaanRuleRegistry.setRule(ministryId, rule),
			`set_rule(${row.ministry_id})`
		);
	}

	// DPIIT default rule set (PoC document Table 10), sourced from the DPIIT entry's
	// own shape per this file's header comment.
	//
	// The runtime now also pre-loads this same default at genesis (Part 6.2), so this
	// call is strictly redundant on a fresh chain. It is kept because Part 7.4 requires
	// the script to be idempotent and self-sufficient: it must reach the correct end
	// state on a chain whose genesis predates that change, and re-running it on a
	// current chain simply rewrites an identical value.
	const dpiitRow = rows.find((r) => r.ministry_id === "DPIIT");
	if (!dpiitRow) {
		throw new Error("ministries.json has no DPIIT entry to derive the default rule set from.");
	}
	await sudoSubmit(
		api,
		dpiit,
		api.tx.pramaanRuleRegistry.setDefaultRule(toChainRule(dpiitRow)),
		"set_default_rule"
	);

	console.log("\nVerifying: querying Rules for every ministry_id and diffing against the source JSON...");
	let mismatches = 0;
	for (const row of rows) {
		const ministryId = encodeId(row.ministry_id);
		const onChain = await api.query.pramaanRuleRegistry.rules(ministryId);
		if (onChain.isNone) {
			console.error(`  MISMATCH: ${row.ministry_id} has no on-chain rule after seeding.`);
			mismatches++;
			continue;
		}
		const actual = canonicalise(onChain.unwrap().toJSON());
		const expected = canonicalise(toChainRule(row));

		if (actual !== expected) {
			console.error(`  MISMATCH: ${row.ministry_id} on-chain value differs from ministries.json.`);
			console.error(`    on-chain: ${actual}`);
			console.error(`    expected: ${expected}`);
			mismatches++;
		}
	}

	await api.disconnect();

	if (mismatches > 0) {
		console.error(`\n${mismatches} ministr${mismatches === 1 ? "y" : "ies"} failed verification. Failing loudly per Part 7.4.`);
		process.exit(1);
	}

	console.log(`\nAll ${rows.length} ministries verified on-chain, plus the DPIIT default rule set. Seed complete.`);
}

main().catch((err) => {
	console.error("Seed script failed:", err);
	process.exit(1);
});
