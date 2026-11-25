//! The tests of the rules module.
//!
//! The land parcel register carries the rules. One invariant keeps every
//! stored parcel at an area above zero. One precondition gates the
//! transfer of a parcel to a new holder.

use crate::{
    explain::RuleKind, mock::*, CollectionRules, Error, Invariants, Pallet, Preconditions,
    RuleFailure, RuleGate,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    rules::{Invariant, Precondition, RuleInput},
    types::{
        CollectionId, FieldDef, FieldName, ParamName, RecordDef, RuleId, TypeDef, TypeExpr, TypeId,
        Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId, RecordKey};

extern crate alloc;
use alloc::{string::ToString, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [9u8; 32];
const STEWARD: u64 = 1;
const CLERK: u64 = 2;

fn parcel_type() -> TypeDef {
    TypeDef::Record(RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new(
                "survey_number",
                TypeExpr::Text,
                "The survey number of the parcel.",
            ),
            FieldDef::new("holder", TypeExpr::Text, "The name of the present holder."),
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
        ],
        summary: "One land parcel in the register.".to_string(),
    })
}

fn parcel_collection() -> Collection {
    let mut collection = Collection::new(
        "parcel",
        "parcel",
        "survey_number",
        "The register of land parcels.",
    );
    collection.purpose = PurposeDeclaration::new(
        "The register records who holds each land parcel.",
        &["registration"],
    );
    collection
}

fn parcel(survey: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text("Anitha".to_string())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

fn area_invariant() -> Invariant {
    Invariant {
        id: RuleId::new("area_above_zero"),
        collection: CollectionId::new("parcel"),
        holds: Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound("record"), "area"),
            Expr::integer(0),
        ),
        problem: "The parcel carries an area of zero square metres.".to_string(),
        fix: "Enter the measured area from the survey sheet.".to_string(),
    }
}

fn transfer_precondition() -> Precondition {
    Precondition {
        id: RuleId::new("transfer_needs_a_new_holder"),
        inputs: vec![
            RuleInput::new("present_holder", TypeExpr::Text),
            RuleInput::new("new_holder", TypeExpr::Text),
        ],
        holds: Expr::binary(
            BinaryOp::NotEqual,
            Expr::bound("present_holder"),
            Expr::bound("new_holder"),
        ),
        problem: "The transfer names the present holder as the new holder.".to_string(),
        fix: "Name a different person as the new holder.".to_string(),
    }
}

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn collection_name() -> BoundedName {
    name_of("parcel").expect("the name fits")
}

fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.to_string());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

fn inputs(present: &str, new: &str) -> Vec<(ParamName, Value)> {
    vec![
        (
            ParamName::new("present_holder"),
            Value::Text(present.to_string()),
        ),
        (ParamName::new("new_holder"), Value::Text(new.to_string())),
    ]
}

fn install() {
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_type().encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_collection().encode())
    ));
    assert_ok!(Rules::register_invariant(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(area_invariant().encode())
    ));
    assert_ok!(Rules::register_precondition(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(transfer_precondition().encode())
    ));
}

fn store(survey: &str, area: i128) {
    assert_ok!(State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        collection_name(),
        bounded(parcel(survey, area).canonical_bytes())
    ));
}

#[test]
fn a_registered_invariant_reads_back() {
    new_test_ext().execute_with(|| {
        install();
        let name = name_of("area_above_zero").expect("the name fits");
        assert!(Invariants::<Test>::contains_key(DEPLOYMENT, &name));
        let held = Pallet::<Test>::invariant(&DEPLOYMENT, &name).expect("the rule decodes");
        assert_eq!(held.collection.as_str(), "parcel");
        assert_eq!(
            CollectionRules::<Test>::get(DEPLOYMENT, collection_name()).len(),
            1
        );
    });
}

#[test]
fn a_registered_precondition_reads_back() {
    new_test_ext().execute_with(|| {
        install();
        let name = name_of("transfer_needs_a_new_holder").expect("the name fits");
        assert!(Preconditions::<Test>::contains_key(DEPLOYMENT, &name));
        let held = Pallet::<Test>::precondition(&DEPLOYMENT, &name).expect("the rule decodes");
        assert_eq!(held.inputs.len(), 2);
    });
}

#[test]
fn a_rule_carrying_no_sentence_declines() {
    new_test_ext().execute_with(|| {
        let mut silent = area_invariant();
        silent.problem = "".to_string();
        assert_noop!(
            Rules::register_invariant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(silent.encode())
            ),
            Error::<Test>::SentenceMissing
        );
    });
}

#[test]
fn a_rule_registered_twice_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Rules::register_invariant(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(area_invariant().encode())
            ),
            Error::<Test>::RulePresent
        );
    });
}

#[test]
fn a_holding_precondition_admits_the_action() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("transfer_needs_a_new_holder"),
            &inputs("Anitha", "Benny"),
            &CLERK
        ));
    });
}

#[test]
fn a_failed_precondition_returns_the_plain_sentence_the_user_wrote() {
    new_test_ext().execute_with(|| {
        install();
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("transfer_needs_a_new_holder"),
            &inputs("Anitha", "Anitha"),
            &CLERK,
        )
        .expect_err("the rule declines the transfer");

        assert_eq!(failure.kind, RuleKind::Precondition);
        assert_eq!(
            failure.problem_text(),
            "The transfer names the present holder as the new holder."
        );
        assert_eq!(
            failure.fix_text(),
            "Name a different person as the new holder."
        );
        assert_eq!(
            failure.line(),
            "The transfer names the present holder as the new holder. Name a different person as the new holder."
        );
    });
}

#[test]
fn a_failed_invariant_returns_the_plain_sentence_the_user_wrote() {
    new_test_ext().execute_with(|| {
        install();
        let failure = Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 0),
            &CLERK,
        )
        .expect_err("the rule declines the record");

        assert_eq!(failure.kind, RuleKind::Invariant);
        assert_eq!(
            failure.problem_text(),
            "The parcel carries an area of zero square metres."
        );
        assert_eq!(
            failure.fix_text(),
            "Enter the measured area from the survey sheet."
        );
    });
}

#[test]
fn a_holding_invariant_admits_the_record() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 400),
            &CLERK
        ));
    });
}

#[test]
fn a_collection_carrying_no_invariant_admits_every_record() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("holder"),
            &parcel("12/3", 0),
            &CLERK
        ));
    });
}

#[test]
fn the_explain_entry_point_states_the_sentences_of_a_rule() {
    new_test_ext().execute_with(|| {
        install();
        let held = Pallet::<Test>::explain(&DEPLOYMENT, &RuleId::new("area_above_zero"))
            .expect("the registry holds the rule");
        assert_eq!(held.kind, RuleKind::Invariant);
        assert_eq!(
            held.problem_text(),
            "The parcel carries an area of zero square metres."
        );

        let gate =
            Pallet::<Test>::explain(&DEPLOYMENT, &RuleId::new("transfer_needs_a_new_holder"))
                .expect("the registry holds the rule");
        assert_eq!(gate.kind, RuleKind::Precondition);
    });
}

#[test]
fn an_unknown_rule_carries_its_own_statement() {
    new_test_ext().execute_with(|| {
        install();
        assert!(Pallet::<Test>::explain(&DEPLOYMENT, &RuleId::new("absent_rule")).is_none());
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("absent_rule"),
            &[],
            &CLERK,
        )
        .expect_err("the registry lacks the rule");
        assert_eq!(failure.problem_text(), "The registry lacks that rule.");
        assert_eq!(
            failure.fix_text(),
            "Register the rule before evaluating it."
        );
    });
}

#[test]
fn a_precondition_call_over_holding_inputs_passes() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Rules::evaluate_precondition(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            name_of("transfer_needs_a_new_holder").expect("the name fits"),
            bounded(inputs("Anitha", "Benny").encode())
        ));
    });
}

#[test]
fn a_precondition_call_over_failing_inputs_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Rules::evaluate_precondition(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                name_of("transfer_needs_a_new_holder").expect("the name fits"),
                bounded(inputs("Anitha", "Anitha").encode())
            ),
            Error::<Test>::RuleDeclinedWork
        );
    });
}

#[test]
fn an_invariant_call_over_a_stored_record_passes() {
    new_test_ext().execute_with(|| {
        install();
        store("12/3", 400);
        assert_ok!(Rules::evaluate_invariants(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            collection_name(),
            record_key("12/3")
        ));
    });
}

#[test]
fn an_invariant_call_over_an_absent_record_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Rules::evaluate_invariants(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                collection_name(),
                record_key("99/9")
            ),
            Error::<Test>::RecordAbsent
        );
    });
}

#[test]
fn a_retired_rule_evaluates_no_longer() {
    new_test_ext().execute_with(|| {
        install();
        assert_ok!(Rules::retire_rule(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name_of("area_above_zero").expect("the name fits")
        ));
        assert_ok!(Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 0),
            &CLERK
        ));
        assert_eq!(
            CollectionRules::<Test>::get(DEPLOYMENT, collection_name()).len(),
            0
        );
    });
}

#[test]
fn retiring_an_unknown_rule_declines() {
    new_test_ext().execute_with(|| {
        install();
        assert_noop!(
            Rules::retire_rule(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                name_of("absent_rule").expect("the name fits")
            ),
            Error::<Test>::RuleUnknown
        );
    });
}

#[test]
fn every_rule_record_appears_in_the_audit_log_with_its_actor() {
    new_test_ext().execute_with(|| {
        install();
        let registered = Audit::query_by_event(&DEPLOYMENT, "invariant_registered", 0, 64);
        assert_eq!(registered.len(), 1);
        assert_eq!(registered[0].actor, STEWARD);
        assert_eq!(registered[0].module, "rules");

        let _ = Rules::evaluate_precondition(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            name_of("transfer_needs_a_new_holder").expect("the name fits"),
            bounded(inputs("Anitha", "Anitha").encode()),
        );
    });
}

#[test]
fn a_declined_precondition_records_the_sentences_in_the_audit_log() {
    new_test_ext().execute_with(|| {
        install();
        // The call runs through the module rather than through a dispatch,
        // so the audit record stays after the refusal.
        let failure = Pallet::<Test>::run_precondition(
            &DEPLOYMENT,
            &name_of("transfer_needs_a_new_holder").expect("the name fits"),
            &inputs("Anitha", "Anitha"),
            &CLERK,
        )
        .expect_err("the rule declines the transfer");
        assert_eq!(
            alloc::string::String::from_utf8(failure.bytes()).expect("the bytes read back"),
            failure.line()
        );
    });
}

#[test]
fn a_failure_of_an_unknown_kind_names_the_rule() {
    let failure = RuleFailure::new(
        name_of("area_above_zero").expect("the name fits"),
        RuleKind::Invariant,
        "The parcel carries an area of zero square metres.",
        "Enter the measured area from the survey sheet.",
    );
    assert_eq!(failure.kind.label(), "invariant");
    assert_eq!(RuleKind::Precondition.label(), "precondition");
}
