//! Standalone ETL utility for converting legacy SQL/PostgreSQL exports
//! into canonical `cerulea-blueprint` records for the three seed products:
//! - Registry
//! - Supply Chain
//! - FinTech

use anyhow::{bail, Context, Result};
use cerulea_blueprint::{
    collections::{Collection, LawfulBasis, LegitimateUseCategory, PurposeDeclaration},
    types::{
        Decimal, FieldDef, FieldName, Hash32, RecordDef, RecordValue, RuleId, Timestamp, TypeDef,
        TypeExpr, TypeId, Value,
    },
    Blueprint, BlueprintMeta, SubstrateTarget,
};
use clap::{Parser, ValueEnum};
use codec::Encode;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::PathBuf,
};

/// Seed product target for legacy SQL/PostgreSQL data conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SeedProduct {
    Registry,
    SupplyChain,
    Fintech,
}

#[derive(Parser, Debug)]
#[command(name = "seed-etl", about = "Convert legacy SQL/PostgreSQL exports into canonical Blueprint records")]
struct EtlArgs {
    /// Target seed product (`registry`, `supply-chain`, or `fintech`).
    #[arg(long, value_enum)]
    product: SeedProduct,
    /// Input JSON array or CSV file exported from legacy SQL/PostgreSQL.
    #[arg(long)]
    input: PathBuf,
    /// Output JSON file containing canonical Blueprint metadata and SCALE-encoded records.
    #[arg(long)]
    output: PathBuf,
}

/// One converted canonical record ready for on-chain migration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanonicalRecordOutput {
    pub primary_key: String,
    pub canonical_hex: String,
    pub scale_hex: String,
    pub checksum_hex: String,
}

/// Complete ETL batch output with Blueprint schema and canonical records.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationBatch {
    pub product: SeedProduct,
    pub blueprint_id: String,
    pub collection: String,
    pub record_count: usize,
    pub records: Vec<CanonicalRecordOutput>,
}

/// Build the canonical Blueprint definition for a seed product.
pub fn seed_blueprint(product: SeedProduct) -> Blueprint {
    match product {
        SeedProduct::Registry => {
            let mut bp = Blueprint::new(
                BlueprintMeta::new(
                    "seed_registry",
                    "Canonical Registry seed product schema.",
                    "Cerulea Registry",
                )
                .with_substrate(SubstrateTarget::Native),
            );
            bp.types = vec![TypeDef::Record(RecordDef {
                id: TypeId::new("registry_entry"),
                fields: vec![
                    FieldDef::new("entry_id", TypeExpr::Text, "Primary registry identifier."),
                    FieldDef::new("holder_name", TypeExpr::Text, "Registered holder name."),
                    FieldDef::new("area_units", TypeExpr::Integer, "Measured units."),
                    FieldDef::new("registered_at", TypeExpr::Timestamp, "Registration timestamp."),
                    FieldDef::new("verified", TypeExpr::Bool, "Verification status."),
                ],
                summary: "A canonical registry record.".to_string(),
            })];
            let mut col = Collection::new(
                "registry_entries",
                "registry_entry",
                "entry_id",
                "Registry records migrated from legacy SQL.",
            );
            col.purpose = PurposeDeclaration::new(
                "Maintain official registry records.",
                &["registration", "verification"],
            );
            col.personal_data = true;
            col.lawful_basis = Some(LawfulBasis::LegitimateUse(
                LegitimateUseCategory::LegalFunction,
            ));
            col.erasure_rule = Some(RuleId::new("registry_erasure"));
            bp.rules.invariants.push(cerulea_blueprint::rules::Invariant {
                id: RuleId::new("registry_erasure"),
                collection: cerulea_blueprint::types::CollectionId::new("registry_entries"),
                holds: cerulea_blueprint::expression::Expr::boolean(true),
                problem: "Registry erasure rule failed.".to_string(),
                fix: "Run registry erasure.".to_string(),
            });
            bp.collections = vec![col];
            bp
        }
        SeedProduct::SupplyChain => {
            let mut bp = Blueprint::new(
                BlueprintMeta::new(
                    "seed_supply_chain",
                    "Canonical Supply Chain seed product schema.",
                    "Cerulea Supply Chain",
                )
                .with_substrate(SubstrateTarget::Native),
            );
            bp.types = vec![TypeDef::Record(RecordDef {
                id: TypeId::new("shipment"),
                fields: vec![
                    FieldDef::new("shipment_id", TypeExpr::Text, "Unique shipment identifier."),
                    FieldDef::new("origin", TypeExpr::Text, "Origin facility code."),
                    FieldDef::new("destination", TypeExpr::Text, "Destination facility code."),
                    FieldDef::new("weight_grams", TypeExpr::Integer, "Gross shipment weight."),
                    FieldDef::new("dispatched_at", TypeExpr::Timestamp, "Dispatch timestamp."),
                ],
                summary: "A canonical supply chain shipment record.".to_string(),
            })];
            let mut col = Collection::new(
                "shipments",
                "shipment",
                "shipment_id",
                "Supply chain shipments migrated from legacy SQL.",
            );
            col.purpose = PurposeDeclaration::new(
                "Track provenance and custody across supply chain checkpoints.",
                &["tracking", "provenance"],
            );
            bp.collections = vec![col];
            bp
        }
        SeedProduct::Fintech => {
            let mut bp = Blueprint::new(
                BlueprintMeta::new(
                    "seed_fintech",
                    "Canonical FinTech seed product schema.",
                    "Cerulea FinTech",
                )
                .with_substrate(SubstrateTarget::Native),
            );
            bp.types = vec![TypeDef::Record(RecordDef {
                id: TypeId::new("ledger_account"),
                fields: vec![
                    FieldDef::new("account_ref", TypeExpr::Text, "Unique account reference."),
                    FieldDef::new("currency", TypeExpr::Text, "ISO currency code."),
                    FieldDef::new(
                        "balance",
                        TypeExpr::Decimal { scale: 2 },
                        "Account balance with 2 decimal places.",
                    ),
                    FieldDef::new("kyc_verified", TypeExpr::Bool, "KYC verification state."),
                    FieldDef::new("updated_at", TypeExpr::Timestamp, "Last ledger update."),
                ],
                summary: "A canonical FinTech ledger account record.".to_string(),
            })];
            let mut col = Collection::new(
                "ledger_accounts",
                "ledger_account",
                "account_ref",
                "FinTech accounts migrated from legacy SQL.",
            );
            col.purpose = PurposeDeclaration::new(
                "Maintain settled FinTech account balances.",
                &["settlement", "audit"],
            );
            col.personal_data = true;
            col.lawful_basis = Some(LawfulBasis::Consent);
            col.consent_tracking = Some(RuleId::new("fintech_consent"));
            col.erasure_rule = Some(RuleId::new("fintech_erasure"));
            bp.rules.invariants.push(cerulea_blueprint::rules::Invariant {
                id: RuleId::new("fintech_consent"),
                collection: cerulea_blueprint::types::CollectionId::new("ledger_accounts"),
                holds: cerulea_blueprint::expression::Expr::boolean(true),
                problem: "Consent missing or withdrawn.".to_string(),
                fix: "Obtain active principal consent.".to_string(),
            });
            bp.rules.invariants.push(cerulea_blueprint::rules::Invariant {
                id: RuleId::new("fintech_erasure"),
                collection: cerulea_blueprint::types::CollectionId::new("ledger_accounts"),
                holds: cerulea_blueprint::expression::Expr::boolean(true),
                problem: "Erasure rule check failed.".to_string(),
                fix: "Execute FinTech erasure workflow.".to_string(),
            });
            bp.collections = vec![col];
            bp
        }
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn parse_decimal_scale_2(raw: &serde_json::Value) -> Result<i128> {
    if let Some(n) = raw.as_i64() {
        return Ok(i128::from(n) * 100);
    }
    let text = raw
        .as_str()
        .map(str::trim)
        .context("expected decimal string or integer")?;
    let negative = text.starts_with('-');
    let unsigned = text.trim_start_matches(['-', '+']);
    let mut parts = unsigned.split('.');
    let whole: i128 = parts
        .next()
        .unwrap_or("0")
        .parse()
        .context("invalid decimal whole part")?;
    let frac_str = parts.next().unwrap_or("");
    if parts.next().is_some() || frac_str.len() > 2 {
        bail!("invalid scale-2 decimal value `{text}`");
    }
    let padded = format!("{frac_str:0<2}");
    let frac: i128 = padded.parse().context("invalid decimal fraction")?;
    let magnitude = whole * 100 + frac;
    Ok(if negative { -magnitude } else { magnitude })
}

fn require_str<'a>(row: &'a BTreeMap<String, serde_json::Value>, key: &str) -> Result<&'a str> {
    row.get(key)
        .and_then(|v| v.as_str())
        .with_context(|| format!("missing string column `{key}`"))
}

fn require_i128(row: &BTreeMap<String, serde_json::Value>, key: &str) -> Result<i128> {
    let val = row
        .get(key)
        .with_context(|| format!("missing integer column `{key}`"))?;
    if let Some(i) = val.as_i64() {
        return Ok(i128::from(i));
    }
    if let Some(s) = val.as_str() {
        return s
            .trim()
            .parse::<i128>()
            .with_context(|| format!("invalid integer in `{key}`"));
    }
    bail!("column `{key}` is not an integer")
}

fn require_u64(row: &BTreeMap<String, serde_json::Value>, key: &str) -> Result<u64> {
    let val = row
        .get(key)
        .with_context(|| format!("missing timestamp column `{key}`"))?;
    if let Some(u) = val.as_u64() {
        return Ok(u);
    }
    if let Some(s) = val.as_str() {
        return s
            .trim()
            .parse::<u64>()
            .with_context(|| format!("invalid u64 timestamp in `{key}`"));
    }
    bail!("column `{key}` is not a timestamp")
}

fn require_bool(row: &BTreeMap<String, serde_json::Value>, key: &str) -> Result<bool> {
    let val = row
        .get(key)
        .with_context(|| format!("missing boolean column `{key}`"))?;
    if let Some(b) = val.as_bool() {
        return Ok(b);
    }
    if let Some(s) = val.as_str() {
        return match s.trim().to_ascii_lowercase().as_str() {
            "true" | "t" | "1" => Ok(true),
            "false" | "f" | "0" => Ok(false),
            other => bail!("invalid boolean `{other}` in `{key}`"),
        };
    }
    bail!("column `{key}` is not a boolean")
}

/// Convert legacy SQL/PostgreSQL rows into a validated canonical `MigrationBatch`.
pub fn convert_rows(
    product: SeedProduct,
    rows: &[BTreeMap<String, serde_json::Value>],
) -> Result<MigrationBatch> {
    let blueprint = seed_blueprint(product);
    if let Err(report) = blueprint.validate() {
        bail!("seed blueprint failed validation: {report}");
    }
    let collection_name = blueprint.collections[0].id.as_str().to_string();
    let type_id = blueprint.collections[0].record_type.clone();
    let mut seen_keys = BTreeSet::new();
    let mut records = Vec::with_capacity(rows.len());

    for (idx, row) in rows.iter().enumerate() {
        let (pk, fields): (String, Vec<(FieldName, Value)>) = match product {
            SeedProduct::Registry => {
                let entry_id = require_str(row, "entry_id")?;
                let holder_name = require_str(row, "holder_name")?;
                let area_units = require_i128(row, "area_units")?;
                let registered_at = require_u64(row, "registered_at")?;
                let verified = require_bool(row, "verified")?;
                (
                    entry_id.to_string(),
                    vec![
                        (FieldName::new("entry_id"), Value::Text(entry_id.to_string())),
                        (
                            FieldName::new("holder_name"),
                            Value::Text(holder_name.to_string()),
                        ),
                        (FieldName::new("area_units"), Value::Integer(area_units)),
                        (
                            FieldName::new("registered_at"),
                            Value::Timestamp(Timestamp(registered_at)),
                        ),
                        (FieldName::new("verified"), Value::Bool(verified)),
                    ],
                )
            }
            SeedProduct::SupplyChain => {
                let shipment_id = require_str(row, "shipment_id")?;
                let origin = require_str(row, "origin")?;
                let destination = require_str(row, "destination")?;
                let weight_grams = require_i128(row, "weight_grams")?;
                let dispatched_at = require_u64(row, "dispatched_at")?;
                (
                    shipment_id.to_string(),
                    vec![
                        (
                            FieldName::new("shipment_id"),
                            Value::Text(shipment_id.to_string()),
                        ),
                        (FieldName::new("origin"), Value::Text(origin.to_string())),
                        (
                            FieldName::new("destination"),
                            Value::Text(destination.to_string()),
                        ),
                        (FieldName::new("weight_grams"), Value::Integer(weight_grams)),
                        (
                            FieldName::new("dispatched_at"),
                            Value::Timestamp(Timestamp(dispatched_at)),
                        ),
                    ],
                )
            }
            SeedProduct::Fintech => {
                let account_ref = require_str(row, "account_ref")?;
                let currency = require_str(row, "currency")?;
                let balance_val = row
                    .get("balance")
                    .context("missing decimal column `balance`")?;
                let mantissa = parse_decimal_scale_2(balance_val)?;
                let kyc_verified = require_bool(row, "kyc_verified")?;
                let updated_at = require_u64(row, "updated_at")?;
                (
                    account_ref.to_string(),
                    vec![
                        (
                            FieldName::new("account_ref"),
                            Value::Text(account_ref.to_string()),
                        ),
                        (FieldName::new("currency"), Value::Text(currency.to_string())),
                        (
                            FieldName::new("balance"),
                            Value::Decimal(Decimal::new(mantissa, 2)),
                        ),
                        (FieldName::new("kyc_verified"), Value::Bool(kyc_verified)),
                        (
                            FieldName::new("updated_at"),
                            Value::Timestamp(Timestamp(updated_at)),
                        ),
                    ],
                )
            }
        };

        if !seen_keys.insert(pk.clone()) {
            bail!("row {idx}: duplicate primary key `{pk}`");
        }

        let record = Value::Record(RecordValue {
            type_id: type_id.clone(),
            fields,
        });
        let canonical = record.canonical_bytes();
        let scale = record.encode();
        let checksum = Hash32::of(&canonical);
        records.push(CanonicalRecordOutput {
            primary_key: pk,
            canonical_hex: to_hex(&canonical),
            scale_hex: to_hex(&scale),
            checksum_hex: to_hex(&checksum.0),
        });
    }

    Ok(MigrationBatch {
        product,
        blueprint_id: blueprint.id().to_hex(),
        collection: collection_name,
        record_count: records.len(),
        records,
    })
}

fn load_rows(path: &PathBuf) -> Result<Vec<BTreeMap<String, serde_json::Value>>> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read input file {}", path.display()))?;
    if path.extension().and_then(|s| s.to_str()) == Some("csv") {
        let mut rdr = csv::Reader::from_reader(raw.as_bytes());
        let headers = rdr.headers()?.clone();
        let mut out = Vec::new();
        for rec in rdr.records() {
            let rec = rec?;
            let mut map = BTreeMap::new();
            for (h, v) in headers.iter().zip(rec.iter()) {
                map.insert(h.to_string(), serde_json::Value::String(v.to_string()));
            }
            out.push(map);
        }
        return Ok(out);
    }
    serde_json::from_str(&raw).context("expected JSON array of SQL row objects")
}

fn main() -> Result<()> {
    let args = EtlArgs::parse();
    let rows = load_rows(&args.input)?;
    let batch = convert_rows(args.product, &rows)?;
    let json = serde_json::to_string_pretty(&batch)?;
    fs::write(&args.output, json)
        .with_context(|| format!("failed to write output file {}", args.output.display()))?;
    println!(
        "Converted {} {:?} records (blueprint {})",
        batch.record_count, batch.product, batch.blueprint_id
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_registry_supply_chain_and_fintech_rows() {
        let reg_rows = vec![BTreeMap::from([
            ("entry_id".into(), serde_json::json!("REG-001")),
            ("holder_name".into(), serde_json::json!("Asha")),
            ("area_units".into(), serde_json::json!(450)),
            ("registered_at".into(), serde_json::json!(1_700_000_000_000_u64)),
            ("verified".into(), serde_json::json!(true)),
        ])];
        let reg_batch = convert_rows(SeedProduct::Registry, &reg_rows).expect("registry ETL");
        assert_eq!(reg_batch.record_count, 1);
        assert_eq!(reg_batch.collection, "registry_entries");

        let sc_rows = vec![BTreeMap::from([
            ("shipment_id".into(), serde_json::json!("SHP-100")),
            ("origin".into(), serde_json::json!("BLR")),
            ("destination".into(), serde_json::json!("COK")),
            ("weight_grams".into(), serde_json::json!(12_500)),
            ("dispatched_at".into(), serde_json::json!(1_700_000_100_000_u64)),
        ])];
        let sc_batch = convert_rows(SeedProduct::SupplyChain, &sc_rows).expect("supply chain ETL");
        assert_eq!(sc_batch.record_count, 1);
        assert_eq!(sc_batch.collection, "shipments");

        let ft_rows = vec![BTreeMap::from([
            ("account_ref".into(), serde_json::json!("ACC-9")),
            ("currency".into(), serde_json::json!("INR")),
            ("balance".into(), serde_json::json!("1250.75")),
            ("kyc_verified".into(), serde_json::json!("true")),
            ("updated_at".into(), serde_json::json!("1700000200000")),
        ])];
        let ft_batch = convert_rows(SeedProduct::Fintech, &ft_rows).expect("fintech ETL");
        assert_eq!(ft_batch.record_count, 1);
        assert_eq!(ft_batch.collection, "ledger_accounts");
    }

    #[test]
    fn rejects_duplicate_primary_keys_and_missing_columns() {
        let dup_rows = vec![
            BTreeMap::from([
                ("shipment_id".into(), serde_json::json!("SHP-1")),
                ("origin".into(), serde_json::json!("A")),
                ("destination".into(), serde_json::json!("B")),
                ("weight_grams".into(), serde_json::json!(10)),
                ("dispatched_at".into(), serde_json::json!(100)),
            ]),
            BTreeMap::from([
                ("shipment_id".into(), serde_json::json!("SHP-1")),
                ("origin".into(), serde_json::json!("A")),
                ("destination".into(), serde_json::json!("C")),
                ("weight_grams".into(), serde_json::json!(20)),
                ("dispatched_at".into(), serde_json::json!(200)),
            ]),
        ];
        assert!(convert_rows(SeedProduct::SupplyChain, &dup_rows).is_err());

        let missing = vec![BTreeMap::from([(
            "entry_id".into(),
            serde_json::json!("REG-2"),
        )])];
        assert!(convert_rows(SeedProduct::Registry, &missing).is_err());
    }
}
