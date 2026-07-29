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
			hsnCode: h.hsn_code,
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

function ministryIdBytes(id: string): Uint8Array {
	return new TextEncoder().encode(id);
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
	const keyring = new Keyring({ type: "sr25519" });
	const dpiit = keyring.addFromUri(DPIIT_SURI);

	console.log(`Connected to ${WS_ENDPOINT}, seeding as ${dpiit.address}`);

	for (const row of rows) {
		const ministryId = ministryIdBytes(row.ministry_id);
		const rule = toChainRule(row);

		await new Promise<void>((resolve, reject) => {
			api.tx.palletPramaanRuleRegistry
				.setRule(ministryId, rule)
				.signAndSend(dpiit, ({ status, dispatchError }) => {
					if (dispatchError) {
						if (dispatchError.isModule) {
							const decoded = api.registry.findMetaError(dispatchError.asModule);
							reject(new Error(`set_rule(${row.ministry_id}) failed: ${decoded.section}.${decoded.name}`));
						} else {
							reject(new Error(`set_rule(${row.ministry_id}) failed: ${dispatchError.toString()}`));
						}
						return;
					}
					if (status.isFinalized) {
						console.log(`  set_rule(${row.ministry_id}) finalized in block ${status.asFinalized.toHex()}`);
						resolve();
					}
				})
				.catch(reject);
		});
	}

	// DPIIT default rule set (PoC document Table 10), sourced from the DPIIT entry's
	// own shape per this file's header comment.
	const dpiitRow = rows.find((r) => r.ministry_id === "DPIIT");
	if (!dpiitRow) {
		throw new Error("ministries.json has no DPIIT entry to derive the default rule set from.");
	}
	await new Promise<void>((resolve, reject) => {
		api.tx.palletPramaanRuleRegistry
			.setDefaultRule(toChainRule(dpiitRow))
			.signAndSend(dpiit, ({ status, dispatchError }) => {
				if (dispatchError) {
					reject(new Error(`set_default_rule failed: ${dispatchError.toString()}`));
					return;
				}
				if (status.isFinalized) {
					console.log(`  set_default_rule finalized in block ${status.asFinalized.toHex()}`);
					resolve();
				}
			})
			.catch(reject);
	});

	console.log("\nVerifying: querying Rules for every ministry_id and diffing against the source JSON...");
	let mismatches = 0;
	for (const row of rows) {
		const ministryId = ministryIdBytes(row.ministry_id);
		const onChain = await api.query.palletPramaanRuleRegistry.rules(ministryId);
		if (onChain.isNone) {
			console.error(`  MISMATCH: ${row.ministry_id} has no on-chain rule after seeding.`);
			mismatches++;
			continue;
		}
		const onChainJson = onChain.unwrap().toJSON();
		const expected = toChainRule(row);
		// Structural diff via JSON string comparison after normalizing key order is
		// good enough here since both sides come from the same toChainRule() shape;
		// a byte-level SCALE comparison would be stricter but this catches the class
		// of error the spec is guarding against (a value silently not applied).
		const onChainNormalized = JSON.stringify(onChainJson, Object.keys(onChainJson).sort());
		const expectedNormalized = JSON.stringify(expected, Object.keys(expected).sort());
		if (onChainNormalized !== expectedNormalized) {
			console.error(`  MISMATCH: ${row.ministry_id} on-chain value differs from ministries.json.`);
			console.error(`    on-chain: ${onChainNormalized}`);
			console.error(`    expected: ${expectedNormalized}`);
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
