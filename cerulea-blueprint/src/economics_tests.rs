//! Behavioural tests for the unit of account section.
//!
//! Each test starts from a clean section, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    actions::{Action, ActionRegistry},
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    economics::{AmountLimit, BalanceSheet, Economics, StandardAmount, UnitOfAccount},
    identities::{Identities, RoleDef},
    types::{
        ActionId, CollectionId, Decimal, FieldDef, FieldPath, RecordDef, RoleId, TypeDef, TypeExpr,
        TypeId, TypeRegistry,
    },
    Diagnostic, DiagnosticCode, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![TypeDef::Record(RecordDef {
        id: TypeId::new("account"),
        fields: vec![
            FieldDef::new("number", TypeExpr::Text, "The account number."),
            FieldDef::new("balance", TypeExpr::Decimal { scale: 2 }, "The balance."),
            FieldDef::new("points", TypeExpr::Integer, "The loyalty points."),
            FieldDef::new("owner", TypeExpr::Identity, "The owner."),
        ],
        summary: "One account.".to_string(),
    })]
}

fn collections() -> Vec<Collection> {
    let purpose = PurposeDeclaration::new("The ledger records balances.", &["accounting"]);
    let mut account = Collection::new("account", "account", "number", "The accounts.");
    account.purpose = purpose.clone();
    let mut orphan = Collection::new("orphan", "ghost", "number", "Over no declared type.");
    orphan.purpose = purpose;
    vec![account, orphan]
}

fn identities() -> Identities {
    Identities {
        roles: vec![RoleDef::new(
            "treasurer",
            None,
            "Treasurer",
            "The treasurer posts amounts.",
        )],
        ..Identities::default()
    }
}

fn actions() -> Vec<Action> {
    vec![Action::new("post_fee", "The action posts one fee.")]
}

fn sheet(collection: &str, amount: &str, holder: &str) -> BalanceSheet {
    BalanceSheet {
        collection: CollectionId::new(collection),
        amount_field: FieldPath::parse(amount),
        holder_field: FieldPath::parse(holder),
        summary: "The sheet records balances.".to_string(),
    }
}

fn clean() -> Economics {
    let mut economics = Economics::new(UnitOfAccount::new("INR", "Indian rupee", 2));
    economics.sheets = vec![sheet("account", "balance", "owner")];
    economics.amounts = vec![StandardAmount {
        action: ActionId::new("post_fee"),
        amount: Decimal::new(5_000, 2),
        summary: "The filing fee.".to_string(),
    }];
    economics.limits = vec![AmountLimit {
        role: RoleId::new("treasurer"),
        maximum: Decimal::new(1_000_000, 2),
        summary: "The daily ceiling.".to_string(),
    }];
    economics
}

fn report(economics: &Economics) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let action_defs = actions();
    let actions = ActionRegistry::build(&action_defs);
    economics.validate(
        &registry,
        &collections,
        &identities(),
        &actions,
        &Path::root().field("economics"),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_section_passes_and_names_its_amount_type() {
    let economics = clean();
    assert!(report(&economics).is_clean());
    assert_eq!(economics.unit.amount_type(), TypeExpr::Decimal { scale: 2 });
    assert_eq!(economics.unit.zero(), Decimal::new(0, 2));
}

#[test]
fn unit_faults_each_raise_a_diagnostic() {
    let mut economics = clean();
    economics.unit = UnitOfAccount::new("", "", 31);
    economics.sheets.clear();
    economics.amounts.clear();
    economics.limits.clear();
    let found = report(&economics);
    assert_eq!(found.len(), 3);
    assert_eq!(
        at(&found, "economics.unit.code").fix,
        "Give the unit a short code such as `INR`."
    );
    assert_eq!(
        at(&found, "economics.unit.name").fix,
        "Give the unit the name a person reads, such as `Indian rupee`."
    );
    let scale = at(&found, "economics.unit.scale");
    assert_eq!(
        scale.problem,
        "The unit of account carries scale 31, above the maximum of 30."
    );
    assert_eq!(scale.fix, "Choose a scale of 30 or below.");
}

#[test]
fn balance_sheet_faults_each_raise_a_diagnostic() {
    let mut economics = clean();
    economics.sheets = vec![
        sheet("account", "balance", "owner"),
        sheet("account", "points", "number"),
        sheet("account", "missing", "absent"),
        sheet("ghost", "balance", "owner"),
        sheet("orphan", "balance", "owner"),
        sheet("account", "", "owner"),
    ];
    economics.sheets[3].summary.clear();
    let found = report(&economics);
    assert_eq!(
        at(&found, "economics.sheets[1].collection").fix,
        "Merge the two sheets into one."
    );
    let amount = at(&found, "economics.sheets[1].amount_field");
    assert_eq!(
        amount.problem,
        "The amount field carries a whole number and the unit of account carries a decimal at scale 2."
    );
    assert_eq!(amount.fix, "Declare the field at scale 2.");
    let holder = at(&found, "economics.sheets[1].holder_field");
    assert_eq!(holder.problem, "The holder field carries text.");
    assert_eq!(
        holder.fix,
        "Point the holder field at a field carrying an identity."
    );
    assert_eq!(
        at(&found, "economics.sheets[2].amount_field").fix,
        "Name a field declared on the record type of `account`."
    );
    assert_eq!(
        at(&found, "economics.sheets[2].holder_field").problem,
        "The entry names the field `absent`, which `account` lacks."
    );
    assert_eq!(
        at(&found, "economics.sheets[3].collection").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "economics.sheets[3].summary").fix,
        "Write one plain sentence stating what the sheet records."
    );
    assert!(found.find("economics.sheets[4].amount_field").is_none());
    assert!(found.find("economics.sheets[4].holder_field").is_none());
    assert_eq!(
        at(&found, "economics.sheets[5].amount_field").code,
        DiagnosticCode::MissingValue
    );
}

#[test]
fn standard_amount_faults_each_raise_a_diagnostic() {
    let mut economics = clean();
    economics.amounts.push(StandardAmount {
        action: ActionId::new("post_fee"),
        amount: Decimal::new(-5, 1),
        summary: "A repeated amount.".to_string(),
    });
    economics.amounts.push(StandardAmount {
        action: ActionId::new("ghost_action"),
        amount: Decimal::new(100, 2),
        summary: "An amount for an absent action.".to_string(),
    });
    let found = report(&economics);
    assert_eq!(
        at(&found, "economics.amounts[1].action").fix,
        "Merge the two amounts into one."
    );
    let amount: Vec<&Diagnostic> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "economics.amounts[1].amount")
        .collect();
    assert_eq!(amount.len(), 2);
    assert_eq!(
        amount[0].problem,
        "The amount carries scale 1 and the unit of account carries scale 2."
    );
    assert_eq!(amount[0].fix, "Enter the amount at scale 2.");
    assert_eq!(amount[1].fix, "Enter an amount at zero or above.");
    assert_eq!(
        at(&found, "economics.amounts[2].action").fix,
        "Point the entry at one of these actions: post_fee."
    );
}

#[test]
fn ceiling_faults_each_raise_a_diagnostic() {
    let mut economics = clean();
    economics.limits.push(AmountLimit {
        role: RoleId::new("treasurer"),
        maximum: Decimal::new(0, 2),
        summary: "A repeated ceiling.".to_string(),
    });
    economics.limits.push(AmountLimit {
        role: RoleId::new("Clerk"),
        maximum: Decimal::new(10, 3),
        summary: "A ceiling for a malformed role.".to_string(),
    });
    let found = report(&economics);
    assert_eq!(
        at(&found, "economics.limits[1].role").fix,
        "Merge the two ceilings into one."
    );
    assert_eq!(
        at(&found, "economics.limits[1].maximum").fix,
        "Enter a ceiling above zero."
    );
    let malformed: Vec<DiagnosticCode> = found
        .diagnostics
        .iter()
        .filter(|d| d.location.as_str() == "economics.limits[2].role")
        .map(|d| d.code)
        .collect();
    assert_eq!(
        malformed,
        [
            DiagnosticCode::InvalidIdentifier,
            DiagnosticCode::DanglingReference
        ]
    );
    assert_eq!(
        at(&found, "economics.limits[2].maximum").code,
        DiagnosticCode::TypeMismatch
    );
}
