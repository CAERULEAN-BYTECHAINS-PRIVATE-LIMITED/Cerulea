//! The optional unit of account.
//!
//! A deployment that records amounts declares this section. A deployment
//! that records no amounts leaves the section out.
//!
//! The unit of account is the currency or the measure the deployment
//! records, such as the Indian rupee at scale 2. Every amount in the
//! deployment carries that scale, so arithmetic stays exact.
//!
//! Cerulea carries no token. Validators settle in fiat through the
//! subscription arrangement. This section records the amounts a deployment
//! tracks for its own work.

use crate::{
    actions::ActionRegistry,
    collections::CollectionRegistry,
    identities::Identities,
    types::{
        ActionId, CollectionId, Decimal, FieldPath, RoleId, TypeExpr, TypeRegistry,
        MAX_DECIMAL_SCALE,
    },
    Diagnostic, DiagnosticCode, Path, Report,
};
use alloc::{
    collections::BTreeSet,
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The currency or the measure a deployment records.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct UnitOfAccount {
    /// The short code a person reads, such as `INR`.
    pub code: String,
    /// The name a person reads, such as `Indian rupee`.
    pub name: String,
    /// The count of digits after the decimal point.
    pub scale: u8,
}

impl UnitOfAccount {
    /// Declare a unit of account.
    pub fn new(code: &str, name: &str, scale: u8) -> Self {
        UnitOfAccount {
            code: code.to_string(),
            name: name.to_string(),
            scale,
        }
    }

    /// The type every amount in this deployment carries.
    pub fn amount_type(&self) -> TypeExpr {
        TypeExpr::Decimal { scale: self.scale }
    }

    /// The zero amount at the declared scale.
    pub fn zero(&self) -> Decimal {
        Decimal::zero(self.scale)
    }
}

/// A collection holding balances.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct BalanceSheet {
    /// The collection holding the balances.
    pub collection: CollectionId,
    /// The field carrying the amount.
    pub amount_field: FieldPath,
    /// The field carrying the party that holds the amount.
    pub holder_field: FieldPath,
    /// One plain sentence stating what the sheet records.
    pub summary: String,
}

/// A standard amount one action records.
///
/// A municipal registration charge is an example. The deployment records
/// the amount. Cerulea charges nothing.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct StandardAmount {
    /// The action the amount belongs to.
    pub action: ActionId,
    /// The amount, at the scale of the unit of account.
    pub amount: Decimal,
    /// One plain sentence stating what the amount covers.
    pub summary: String,
}

/// A ceiling on the amount one role moves in one action.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct AmountLimit {
    /// The role the ceiling covers.
    pub role: RoleId,
    /// The largest amount the role moves in one action.
    pub maximum: Decimal,
    /// One plain sentence stating why the ceiling holds.
    pub summary: String,
}

/// The economics section.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Encode,
    Decode,
    TypeInfo,
    Serialize,
    Deserialize,
)]
pub struct Economics {
    /// The currency or the measure the deployment records.
    pub unit: UnitOfAccount,
    /// The collections holding balances.
    pub sheets: Vec<BalanceSheet>,
    /// The standard amounts.
    pub amounts: Vec<StandardAmount>,
    /// The ceilings per role.
    pub limits: Vec<AmountLimit>,
}

impl Economics {
    /// Declare a section over one unit of account.
    pub fn new(unit: UnitOfAccount) -> Self {
        Economics {
            unit,
            sheets: Vec::new(),
            amounts: Vec::new(),
            limits: Vec::new(),
        }
    }

    /// Check the whole economics section.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        actions: &ActionRegistry,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();

        let unit_path = path.field("unit");
        if self.unit.code.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                unit_path.field("code").location(),
                "The unit of account carries an empty code.".to_string(),
                "Give the unit a short code such as `INR`.".to_string(),
            ));
        }
        if self.unit.name.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                unit_path.field("name").location(),
                "The unit of account carries an empty name.".to_string(),
                "Give the unit the name a person reads, such as `Indian rupee`.".to_string(),
            ));
        }
        if self.unit.scale > MAX_DECIMAL_SCALE {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                unit_path.field("scale").location(),
                format!(
                    "The unit of account carries scale {}, above the maximum of {}.",
                    self.unit.scale, MAX_DECIMAL_SCALE
                ),
                format!("Choose a scale of {MAX_DECIMAL_SCALE} or below."),
            ));
        }
        let amount_type = self.unit.amount_type();

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (position, sheet) in self.sheets.iter().enumerate() {
            let here = path.field("sheets").index(position);
            if !seen.insert(sheet.collection.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("collection").location(),
                    format!(
                        "Two balance sheets name the collection `{}`.",
                        sheet.collection
                    ),
                    "Merge the two sheets into one.".to_string(),
                ));
            }
            match collections.get(&sheet.collection) {
                Some(collection) => {
                    let record = TypeExpr::Record(collection.record_type.clone());
                    report.absorb(sheet.amount_field.validate(&here.field("amount_field")));
                    match sheet.amount_field.resolve(&record, types) {
                        Some(found) => {
                            if found != amount_type {
                                report.push(Diagnostic::at(
                                    DiagnosticCode::TypeMismatch,
                                    here.field("amount_field").location(),
                                    format!(
                                        "The amount field carries {} and the unit of account carries {}.",
                                        found.describe(),
                                        amount_type.describe()
                                    ),
                                    format!(
                                        "Declare the field at scale {}.",
                                        self.unit.scale
                                    ),
                                ));
                            }
                        }
                        None => {
                            if types.record(&collection.record_type).is_some() {
                                report.push(missing_field(
                                    &sheet.amount_field,
                                    &sheet.collection,
                                    here.field("amount_field"),
                                ));
                            }
                        }
                    }
                    report.absorb(sheet.holder_field.validate(&here.field("holder_field")));
                    match sheet.holder_field.resolve(&record, types) {
                        Some(TypeExpr::Identity) => {}
                        Some(found) => report.push(Diagnostic::at(
                            DiagnosticCode::TypeMismatch,
                            here.field("holder_field").location(),
                            format!("The holder field carries {}.", found.describe()),
                            "Point the holder field at a field carrying an identity.".to_string(),
                        )),
                        None => {
                            if types.record(&collection.record_type).is_some() {
                                report.push(missing_field(
                                    &sheet.holder_field,
                                    &sheet.collection,
                                    here.field("holder_field"),
                                ));
                            }
                        }
                    }
                }
                None => report.push(
                    collections.missing(&sheet.collection, here.field("collection").location()),
                ),
            }
            if sheet.summary.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("summary").location(),
                    format!(
                        "The balance sheet over `{}` states nothing.",
                        sheet.collection
                    ),
                    "Write one plain sentence stating what the sheet records.".to_string(),
                ));
            }
        }

        let mut amount_actions: BTreeSet<&str> = BTreeSet::new();
        for (position, amount) in self.amounts.iter().enumerate() {
            let here = path.field("amounts").index(position);
            if !actions.contains(&amount.action) {
                report.push(actions.missing(&amount.action, here.field("action").location()));
            }
            if !amount_actions.insert(amount.action.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("action").location(),
                    format!("Two standard amounts name the action `{}`.", amount.action),
                    "Merge the two amounts into one.".to_string(),
                ));
            }
            report.merge(scale_matches(
                amount.amount,
                self.unit.scale,
                &here.field("amount"),
            ));
            if amount.amount.units < 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("amount").location(),
                    format!(
                        "The standard amount for `{}` sits below zero.",
                        amount.action
                    ),
                    "Enter an amount at zero or above.".to_string(),
                ));
            }
        }

        let mut limit_roles: BTreeSet<&str> = BTreeSet::new();
        for (position, limit) in self.limits.iter().enumerate() {
            let here = path.field("limits").index(position);
            report.absorb(limit.role.validate(&here.field("role")));
            if !identities.has_role(&limit.role) {
                report.push(identities.missing_role(&limit.role, here.field("role").location()));
            }
            if !limit_roles.insert(limit.role.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("role").location(),
                    format!("Two ceilings name the role `{}`.", limit.role),
                    "Merge the two ceilings into one.".to_string(),
                ));
            }
            report.merge(scale_matches(
                limit.maximum,
                self.unit.scale,
                &here.field("maximum"),
            ));
            if limit.maximum.units <= 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("maximum").location(),
                    format!("The ceiling for `{}` sits at zero or below.", limit.role),
                    "Enter a ceiling above zero.".to_string(),
                ));
            }
        }

        report
    }
}

fn missing_field(field: &FieldPath, collection: &CollectionId, path: Path) -> Diagnostic {
    Diagnostic::at(
        DiagnosticCode::DanglingReference,
        path.location(),
        format!("The entry names the field `{field}`, which `{collection}` lacks."),
        format!("Name a field declared on the record type of `{collection}`."),
    )
}

fn scale_matches(amount: Decimal, scale: u8, path: &Path) -> Report {
    let mut report = Report::new();
    if amount.scale != scale {
        report.push(Diagnostic::at(
            DiagnosticCode::TypeMismatch,
            path.location(),
            format!(
                "The amount carries scale {} and the unit of account carries scale {scale}.",
                amount.scale
            ),
            format!("Enter the amount at scale {scale}."),
        ));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{Action, ActionRegistry},
        collections::{Collection, PurposeDeclaration},
        identities::RoleDef,
        types::{FieldDef, RecordDef, TypeDef, TypeId},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![TypeDef::Record(RecordDef {
            id: TypeId::new("ledger_entry"),
            fields: vec![
                FieldDef::new("reference", TypeExpr::Text, "The entry reference."),
                FieldDef::new(
                    "holder",
                    TypeExpr::Identity,
                    "The party that holds the amount.",
                ),
                FieldDef::new("amount", TypeExpr::Decimal { scale: 2 }, "The amount held."),
            ],
            summary: "One ledger entry.".to_string(),
        })]
    }

    fn collections() -> Vec<Collection> {
        let mut ledger = Collection::new(
            "ledger",
            "ledger_entry",
            "reference",
            "The deployment ledger.",
        );
        ledger.purpose = PurposeDeclaration::new("The ledger records amounts.", &["accounting"]);
        vec![ledger]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![RoleDef::new(
                "treasurer",
                None,
                "Treasurer",
                "The treasurer records amounts.",
            )],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn actions() -> Vec<Action> {
        let mut action = Action::new("record_receipt", "The action records one receipt.");
        action.roles = vec![RoleId::new("treasurer")];
        vec![action]
    }

    fn sample() -> Economics {
        let mut economics = Economics::new(UnitOfAccount::new("INR", "Indian rupee", 2));
        economics.sheets = vec![BalanceSheet {
            collection: CollectionId::new("ledger"),
            amount_field: FieldPath::parse("amount"),
            holder_field: FieldPath::parse("holder"),
            summary: "The sheet records the amount each party holds.".to_string(),
        }];
        economics.amounts = vec![StandardAmount {
            action: ActionId::new("record_receipt"),
            amount: Decimal::new(5000, 2),
            summary: "The registration charge set by the district.".to_string(),
        }];
        economics.limits = vec![AmountLimit {
            role: RoleId::new("treasurer"),
            maximum: Decimal::new(10_000_000, 2),
            summary: "The treasurer moves up to one hundred thousand rupees at once.".to_string(),
        }];
        economics
    }

    fn run(economics: &Economics) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let declared = actions();
        let action_registry = ActionRegistry::build(&declared);
        economics.validate(
            &type_registry,
            &collection_registry,
            &identities(),
            &action_registry,
            &Path::root().field("economics"),
        )
    }

    #[test]
    fn a_complete_economics_section_validates() {
        let report = run(&sample());
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_sheet_naming_an_absent_collection_reports_its_location() {
        let mut economics = sample();
        economics.sheets[0].collection = CollectionId::new("absent_collection");
        let report = run(&economics);
        let found = report
            .find("economics.sheets[0].collection")
            .expect("the diagnostic sits on the collection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn an_amount_field_at_another_scale_is_caught() {
        let mut economics = sample();
        economics.unit.scale = 4;
        let report = run(&economics);
        let found = report
            .find("economics.sheets[0].amount_field")
            .expect("the diagnostic sits on the amount field");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
        assert!(found.fix.contains("scale 4"));
    }

    #[test]
    fn a_holder_field_outside_an_identity_is_caught() {
        let mut economics = sample();
        economics.sheets[0].holder_field = FieldPath::parse("reference");
        let report = run(&economics);
        let found = report
            .find("economics.sheets[0].holder_field")
            .expect("the diagnostic sits on the holder field");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_standard_amount_naming_an_absent_action_is_caught() {
        let mut economics = sample();
        economics.amounts[0].action = ActionId::new("absent_action");
        let report = run(&economics);
        let found = report
            .find("economics.amounts[0].action")
            .expect("the diagnostic sits on the action");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("record_receipt"));
    }

    #[test]
    fn an_amount_at_another_scale_is_caught() {
        let mut economics = sample();
        economics.amounts[0].amount = Decimal::new(5000, 3);
        let report = run(&economics);
        let found = report
            .find("economics.amounts[0].amount")
            .expect("the diagnostic sits on the amount");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_ceiling_at_zero_is_caught() {
        let mut economics = sample();
        economics.limits[0].maximum = Decimal::new(0, 2);
        let report = run(&economics);
        let found = report
            .find("economics.limits[0].maximum")
            .expect("the diagnostic sits on the ceiling");
        assert_eq!(found.code, DiagnosticCode::ValueOutOfRange);
    }

    #[test]
    fn a_ceiling_naming_an_absent_role_is_caught() {
        let mut economics = sample();
        economics.limits[0].role = RoleId::new("absent_role");
        let report = run(&economics);
        assert!(report.find("economics.limits[0].role").is_some());
    }

    #[test]
    fn an_empty_unit_code_is_caught() {
        let mut economics = sample();
        economics.unit.code = String::new();
        let report = run(&economics);
        let found = report
            .find("economics.unit.code")
            .expect("the diagnostic sits on the unit code");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
    }

    #[test]
    fn the_unit_names_the_amount_type_and_the_zero_amount() {
        let unit = UnitOfAccount::new("INR", "Indian rupee", 2);
        assert_eq!(unit.amount_type(), TypeExpr::Decimal { scale: 2 });
        assert_eq!(unit.zero(), Decimal::new(0, 2));
        assert_eq!(unit.zero().to_string(), "0.00");
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let economics = sample();
        let bytes = economics.encode();
        let back = Economics::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(economics, back);
        let text = serde_json::to_string(&economics).expect("the section serialises");
        let parsed: Economics = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(economics, parsed);
    }
}
