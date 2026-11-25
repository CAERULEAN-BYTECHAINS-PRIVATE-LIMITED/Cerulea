//! The registry and evaluation tests of the rules module.
//!
//! The land parcel register carries the rules again. These tests walk every
//! refusal the registry raises, every sentence an evaluation returns and
//! every record the audit log keeps.

use crate::{
    explain::{RuleKind, MAX_SENTENCE_BYTES},
    mock::*,
    CollectionRules, Error, Event, Invariants, Pallet, Preconditions, RuleFailure, RuleGate,
    RuleNames, StoredRule,
};
use cerulea_blueprint::{
    collections::{Collection, PurposeDeclaration},
    expression::{BinaryOp, Builtin, Expr},
    rules::{Invariant, Precondition, RuleInput},
    types::{
        CollectionId, FieldDef, FieldName, ParamName, RecordDef, RuleId, Timestamp, TypeDef,
        TypeExpr, TypeId, Value,
    },
};
use codec::Encode;
use frame_support::{assert_noop, assert_ok, traits::ConstU32, BoundedVec};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId, RecordKey};
use sp_runtime::DispatchError;

extern crate alloc;
use alloc::{format, string::String, string::ToString, vec, vec::Vec};

const DEPLOYMENT: DeploymentId = [7u8; 32];
const STEWARD: u64 = 1;
const CLERK: u64 = 2;

fn bounded<M: frame_support::traits::Get<u32>>(bytes: Vec<u8>) -> BoundedVec<u8, M> {
    BoundedVec::try_from(bytes).expect("the payload sits inside the bound")
}

fn name(text: &str) -> BoundedName {
    name_of(text).expect("the name fits")
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

fn area_invariant(id: &str) -> Invariant {
    Invariant {
        id: RuleId::new(id),
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

fn transfer_precondition(id: &str) -> Precondition {
    Precondition {
        id: RuleId::new(id),
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

fn transfer_inputs(present: &str, new: &str) -> Vec<(ParamName, Value)> {
    vec![
        (
            ParamName::new("present_holder"),
            Value::Text(present.to_string()),
        ),
        (ParamName::new("new_holder"), Value::Text(new.to_string())),
    ]
}

fn put_invariant(rule: &Invariant) -> frame_support::dispatch::DispatchResult {
    Rules::register_invariant(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(rule.encode()),
    )
}

fn put_precondition(rule: &Precondition) -> frame_support::dispatch::DispatchResult {
    Rules::register_precondition(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(rule.encode()),
    )
}

fn install_parcel_register() {
    let parcel_type = TypeDef::Record(RecordDef {
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
    });
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
    assert_ok!(Schema::register_type(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(parcel_type.encode())
    ));
    assert_ok!(Schema::register_collection(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        bounded(collection.encode())
    ));
}

fn store(survey: &str, area: i128) {
    assert_ok!(State::insert(
        RuntimeOrigin::signed(STEWARD),
        DEPLOYMENT,
        name("parcel"),
        bounded(parcel(survey, area).canonical_bytes())
    ));
}

fn record_key(survey: &str) -> RecordKey {
    let mut value = Value::Text(survey.to_string());
    value.canonicalise();
    BoundedVec::try_from(value.canonical_bytes()).expect("the key fits")
}

fn long_text() -> String {
    "r".repeat(129)
}

#[test]
fn an_unsigned_origin_registers_nothing() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Rules::register_invariant(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                bounded(area_invariant("area_above_zero").encode())
            ),
            DispatchError::BadOrigin
        );
        assert_noop!(
            Rules::register_precondition(
                RuntimeOrigin::none(),
                DEPLOYMENT,
                bounded(transfer_precondition("transfer_check").encode())
            ),
            DispatchError::BadOrigin
        );
    });
}

#[test]
fn an_invariant_registration_stores_its_registrar_block_and_collection() {
    new_test_ext().execute_with(|| {
        System::set_block_number(5);
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));

        let held = Invariants::<Test>::get(DEPLOYMENT, name("area_above_zero"))
            .expect("the registry holds the rule");
        assert_eq!(held.registrar, STEWARD);
        assert_eq!(held.registered_at, 5);
        assert_eq!(held.collection, Some(name("parcel")));
        assert_eq!(
            held.definition.to_vec(),
            area_invariant("area_above_zero").encode()
        );
        assert_eq!(
            RuleNames::<Test>::get(DEPLOYMENT).into_inner(),
            vec![name("area_above_zero")]
        );
        System::assert_last_event(RuntimeEvent::Rules(Event::InvariantRegistered {
            deployment: DEPLOYMENT,
            rule: name("area_above_zero"),
            collection: name("parcel"),
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "invariant_registered", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].reason,
            "The steward registered one invariant over a collection."
        );
        assert_eq!(
            entries[0].payload,
            (name("area_above_zero"), name("parcel")).encode()
        );
    });
}

#[test]
fn a_precondition_registration_stores_no_collection() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));

        let held = Preconditions::<Test>::get(DEPLOYMENT, name("transfer_check"))
            .expect("the registry holds the rule");
        assert_eq!(held.registrar, STEWARD);
        assert_eq!(held.collection, None);
        System::assert_last_event(RuntimeEvent::Rules(Event::PreconditionRegistered {
            deployment: DEPLOYMENT,
            rule: name("transfer_check"),
        }));

        let entries = Audit::query_by_event(&DEPLOYMENT, "precondition_registered", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].actor, STEWARD);
        assert_eq!(
            entries[0].reason,
            "The steward registered one precondition over an action."
        );
        assert_eq!(entries[0].payload, b"transfer_check".to_vec());
    });
}

#[test]
fn an_unreadable_definition_declines_for_both_kinds() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            Rules::register_invariant(RuntimeOrigin::signed(STEWARD), DEPLOYMENT, bounded(vec![])),
            Error::<Test>::RuleUnreadable
        );
        assert_noop!(
            Rules::register_precondition(
                RuntimeOrigin::signed(STEWARD),
                DEPLOYMENT,
                bounded(vec![])
            ),
            Error::<Test>::RuleUnreadable
        );
    });
}

#[test]
fn a_rule_missing_a_sentence_declines_for_both_kinds() {
    new_test_ext().execute_with(|| {
        let mut invariant = area_invariant("area_above_zero");
        invariant.fix = "".to_string();
        assert_noop!(put_invariant(&invariant), Error::<Test>::SentenceMissing);

        let mut precondition = transfer_precondition("transfer_check");
        precondition.fix = "".to_string();
        assert_noop!(
            put_precondition(&precondition),
            Error::<Test>::SentenceMissing
        );

        let mut precondition = transfer_precondition("transfer_check");
        precondition.problem = "".to_string();
        assert_noop!(
            put_precondition(&precondition),
            Error::<Test>::SentenceMissing
        );
    });
}

#[test]
fn a_rule_name_past_128_bytes_declines() {
    new_test_ext().execute_with(|| {
        assert_noop!(
            put_invariant(&area_invariant(&long_text())),
            Error::<Test>::NameTooLong
        );
        assert_noop!(
            put_precondition(&transfer_precondition(&long_text())),
            Error::<Test>::NameTooLong
        );
    });
}

#[test]
fn a_collection_name_past_128_bytes_declines() {
    new_test_ext().execute_with(|| {
        let mut invariant = area_invariant("area_above_zero");
        invariant.collection = CollectionId::new(&long_text());
        assert_noop!(put_invariant(&invariant), Error::<Test>::NameTooLong);
    });
}

#[test]
fn a_precondition_registered_twice_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        assert_noop!(
            put_precondition(&transfer_precondition("transfer_check")),
            Error::<Test>::RulePresent
        );
    });
}

#[test]
fn a_deployment_holding_its_full_count_of_rules_declines_one_more() {
    new_test_ext().execute_with(|| {
        for index in 0..64u32 {
            assert_ok!(put_precondition(&transfer_precondition(&format!(
                "transfer_check_{index}"
            ))));
        }
        assert_eq!(RuleNames::<Test>::get(DEPLOYMENT).len(), 64);
        assert_noop!(
            put_precondition(&transfer_precondition("transfer_check_64")),
            Error::<Test>::RuleLimitReached
        );
        assert_noop!(
            put_invariant(&area_invariant("area_above_zero")),
            Error::<Test>::RuleLimitReached
        );
    });
}

#[test]
fn a_collection_carrying_its_full_count_of_invariants_declines_one_more() {
    new_test_ext().execute_with(|| {
        for index in 0..16u32 {
            assert_ok!(put_invariant(&area_invariant(&format!(
                "area_above_zero_{index}"
            ))));
        }
        assert_eq!(
            CollectionRules::<Test>::get(DEPLOYMENT, name("parcel")).len(),
            16
        );
        assert_noop!(
            put_invariant(&area_invariant("area_above_zero_16")),
            Error::<Test>::CollectionRulesFull
        );
    });
}

#[test]
fn a_shared_name_lists_once_and_retires_both_rules() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_invariant(&area_invariant("parcel_check")));
        assert_ok!(put_precondition(&transfer_precondition("parcel_check")));
        assert_eq!(
            RuleNames::<Test>::get(DEPLOYMENT).into_inner(),
            vec![name("parcel_check")]
        );

        assert_ok!(Rules::retire_rule(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("parcel_check")
        ));
        assert!(!Invariants::<Test>::contains_key(
            DEPLOYMENT,
            name("parcel_check")
        ));
        assert!(!Preconditions::<Test>::contains_key(
            DEPLOYMENT,
            name("parcel_check")
        ));
        assert!(RuleNames::<Test>::get(DEPLOYMENT).is_empty());
        assert!(CollectionRules::<Test>::get(DEPLOYMENT, name("parcel")).is_empty());
    });
}

#[test]
fn retiring_a_precondition_clears_it_and_records_the_steward() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        assert_ok!(put_precondition(&transfer_precondition("second_check")));
        assert_ok!(Rules::retire_rule(
            RuntimeOrigin::signed(STEWARD),
            DEPLOYMENT,
            name("transfer_check")
        ));

        assert!(Pallet::<Test>::precondition(&DEPLOYMENT, &name("transfer_check")).is_none());
        assert_eq!(
            RuleNames::<Test>::get(DEPLOYMENT).into_inner(),
            vec![name("second_check")]
        );
        System::assert_last_event(RuntimeEvent::Rules(Event::RuleRetired {
            deployment: DEPLOYMENT,
            rule: name("transfer_check"),
        }));
        let entries = Audit::query_by_event(&DEPLOYMENT, "rule_retired", 0, 64);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].actor, STEWARD);
        assert_eq!(entries[0].reason, "The steward retired one rule.");
        assert_eq!(entries[0].payload, b"transfer_check".to_vec());

        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("transfer_check"),
            &transfer_inputs("Anitha", "Benny"),
            &CLERK,
        )
        .expect_err("the registry lacks the retired rule");
        assert_eq!(failure.problem_text(), "The registry lacks that rule.");
        assert_eq!(
            failure.fix_text(),
            "Register the rule before evaluating it."
        );
    });
}

#[test]
fn a_holding_precondition_call_records_the_held_event() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        assert_ok!(Rules::evaluate_precondition(
            RuntimeOrigin::signed(CLERK),
            DEPLOYMENT,
            name("transfer_check"),
            bounded(transfer_inputs("Anitha", "Benny").encode())
        ));
        System::assert_last_event(RuntimeEvent::Rules(Event::RuleHeld {
            deployment: DEPLOYMENT,
            rule: name("transfer_check"),
            actor: CLERK,
        }));
    });
}

#[test]
fn a_precondition_call_carrying_unreadable_inputs_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        assert_noop!(
            Rules::evaluate_precondition(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                name("transfer_check"),
                bounded(vec![])
            ),
            Error::<Test>::RuleUnreadable
        );
    });
}

#[test]
fn a_call_supplying_more_inputs_than_the_rule_reads_declines() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        let mut supplied = transfer_inputs("Anitha", "Benny");
        supplied.push((ParamName::new("witness"), Value::Text("Chitra".to_string())));

        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("transfer_check"),
            &supplied,
            &CLERK,
        )
        .expect_err("the rule reads two inputs");
        assert_eq!(failure.kind, RuleKind::Precondition);
        assert_eq!(failure.rule, name("transfer_check"));
        assert_eq!(
            failure.problem_text(),
            "The call supplies more values than the rule reads."
        );
        assert_eq!(
            failure.fix_text(),
            "Supply one value for each declared input."
        );

        assert_noop!(
            Rules::evaluate_precondition(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                name("transfer_check"),
                bounded(supplied.encode())
            ),
            Error::<Test>::RuleDeclinedWork
        );
    });
}

#[test]
fn an_input_left_unsupplied_returns_the_names_in_scope() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("transfer_check"),
            &[(
                ParamName::new("present_holder"),
                Value::Text("Anitha".to_string()),
            )],
            &CLERK,
        )
        .expect_err("the rule reads an input nobody supplied");
        assert_eq!(failure.kind, RuleKind::Precondition);
        assert_eq!(
            failure.problem_text(),
            "The expression reads `new_holder`, which is out of scope."
        );
        assert_eq!(
            failure.fix_text(),
            "Read one of the names in scope here: present_holder."
        );
    });
}

#[test]
fn a_precondition_producing_a_number_returns_the_truth_value_fix() {
    new_test_ext().execute_with(|| {
        let fee = Precondition {
            id: RuleId::new("fee_in_rupees"),
            inputs: vec![RuleInput::new("fee", TypeExpr::Integer)],
            holds: Expr::bound("fee"),
            problem: "The fee falls below the stamp duty.".to_string(),
            fix: "Pay the stamp duty in full.".to_string(),
        };
        assert_ok!(put_precondition(&fee));
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("fee_in_rupees"),
            &[(ParamName::new("fee"), Value::Integer(500))],
            &CLERK,
        )
        .expect_err("the rule produces a whole number");
        assert_eq!(
            failure.problem_text(),
            "The precondition produced a whole number."
        );
        assert_eq!(failure.fix_text(), "Give the precondition a truth value.");
    });
}

#[test]
fn a_precondition_reads_the_clock_of_the_blockchain() {
    new_test_ext().execute_with(|| {
        let window = Precondition {
            id: RuleId::new("objection_window_open"),
            inputs: vec![RuleInput::new("closes_at", TypeExpr::Timestamp)],
            holds: Expr::binary(
                BinaryOp::Less,
                Expr::Call {
                    function: Builtin::Now,
                    args: vec![],
                },
                Expr::bound("closes_at"),
            ),
            problem: "The objection window for the parcel has closed.".to_string(),
            fix: "Raise the objection at the revenue office.".to_string(),
        };
        assert_ok!(put_precondition(&window));
        let closes = [(
            ParamName::new("closes_at"),
            Value::Timestamp(Timestamp::from_millis(START + 1_000)),
        )];

        assert_eq!(Pallet::<Test>::now_millis(), START);
        assert_ok!(Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("objection_window_open"),
            &closes,
            &CLERK
        ));

        set_clock(START + 2_000);
        assert_eq!(Pallet::<Test>::now_millis(), START + 2_000);
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new("objection_window_open"),
            &closes,
            &CLERK,
        )
        .expect_err("the window closed");
        assert_eq!(
            failure.line(),
            "The objection window for the parcel has closed. Raise the objection at the revenue office."
        );
    });
}

#[test]
fn a_rule_name_past_128_bytes_returns_the_shorten_fix() {
    new_test_ext().execute_with(|| {
        let failure = Pallet::<Test>::check_precondition(
            &DEPLOYMENT,
            &RuleId::new(&long_text()),
            &[],
            &CLERK,
        )
        .expect_err("the name runs past the bound");
        assert!(failure.rule.is_empty());
        assert_eq!(failure.kind, RuleKind::Precondition);
        assert_eq!(failure.problem_text(), "The rule name runs past 128 bytes.");
        assert_eq!(failure.fix_text(), "Shorten the rule name in the Studio.");
        assert!(Pallet::<Test>::explain(&DEPLOYMENT, &RuleId::new(&long_text())).is_none());
    });
}

#[test]
fn an_invariant_reading_an_absent_field_returns_the_field_fix() {
    new_test_ext().execute_with(|| {
        let depth = Invariant {
            id: RuleId::new("depth_above_zero"),
            collection: CollectionId::new("parcel"),
            holds: Expr::binary(
                BinaryOp::Greater,
                Expr::field(Expr::bound("record"), "depth"),
                Expr::integer(0),
            ),
            problem: "The well on the parcel carries no depth.".to_string(),
            fix: "Enter the depth from the borewell report.".to_string(),
        };
        assert_ok!(put_invariant(&depth));
        let failure = Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 400),
            &CLERK,
        )
        .expect_err("the record lacks the field");
        assert_eq!(failure.kind, RuleKind::Invariant);
        assert_eq!(failure.rule, name("depth_above_zero"));
        assert_eq!(
            failure.problem_text(),
            "The record value lacks the field `depth`."
        );
        assert_eq!(
            failure.fix_text(),
            "Give the field `depth` a value before the read."
        );
    });
}

#[test]
fn an_invariant_producing_a_number_returns_the_truth_value_fix() {
    new_test_ext().execute_with(|| {
        let mut area = area_invariant("area_as_number");
        area.holds = Expr::field(Expr::bound("record"), "area");
        assert_ok!(put_invariant(&area));
        let failure = Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 400),
            &CLERK,
        )
        .expect_err("the rule produces a whole number");
        assert_eq!(
            failure.problem_text(),
            "The invariant produced a whole number."
        );
        assert_eq!(failure.fix_text(), "Give the invariant a truth value.");
    });
}

#[test]
fn the_first_declining_invariant_stops_the_pass() {
    new_test_ext().execute_with(|| {
        let mut holder = area_invariant("holder_named");
        holder.holds = Expr::binary(
            BinaryOp::NotEqual,
            Expr::field(Expr::bound("record"), "holder"),
            Expr::text("Anitha"),
        );
        holder.problem = "The parcel names Anitha as the holder.".to_string();
        holder.fix = "Record the holder from the sale deed.".to_string();
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));
        assert_ok!(put_invariant(&holder));
        assert_eq!(
            Pallet::<Test>::invariants_of(&DEPLOYMENT, &CollectionId::new("parcel")),
            vec![name("area_above_zero"), name("holder_named")]
        );

        let failure = Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 400),
            &CLERK,
        )
        .expect_err("the second rule declines");
        assert_eq!(failure.rule, name("holder_named"));
        assert_eq!(
            failure.line(),
            "The parcel names Anitha as the holder. Record the holder from the sale deed."
        );
    });
}

#[test]
fn a_listed_invariant_missing_from_the_registry_is_skipped() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));
        let garbled: BoundedVec<u8, ConstU32<16_384>> = bounded(vec![1, 2, 3]);
        Invariants::<Test>::insert(
            DEPLOYMENT,
            name("garbled_rule"),
            StoredRule {
                definition: garbled,
                collection: Some(name("parcel")),
                registrar: STEWARD,
                registered_at: 1u64,
            },
        );
        CollectionRules::<Test>::mutate(DEPLOYMENT, name("parcel"), |names| {
            assert!(names.try_insert(0, name("ghost_rule")).is_ok());
            assert!(names.try_insert(1, name("garbled_rule")).is_ok());
        });

        assert!(Pallet::<Test>::invariant(&DEPLOYMENT, &name("ghost_rule")).is_none());
        assert!(Pallet::<Test>::invariant(&DEPLOYMENT, &name("garbled_rule")).is_none());
        assert!(Pallet::<Test>::explain(&DEPLOYMENT, &RuleId::new("garbled_rule")).is_none());
        assert_ok!(Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 400),
            &CLERK
        ));
        let failure = Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &CollectionId::new("parcel"),
            &parcel("12/3", 0),
            &CLERK,
        )
        .expect_err("the registered rule still declines");
        assert_eq!(failure.rule, name("area_above_zero"));
    });
}

#[test]
fn a_collection_name_past_128_bytes_carries_no_invariant() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));
        let long = CollectionId::new(&long_text());
        assert!(Pallet::<Test>::invariants_of(&DEPLOYMENT, &long).is_empty());
        assert_ok!(Pallet::<Test>::check_invariants(
            &DEPLOYMENT,
            &long,
            &parcel("12/3", 0),
            &CLERK
        ));
    });
}

#[test]
fn an_invariant_call_over_a_stored_breaking_record_declines() {
    new_test_ext().execute_with(|| {
        install_parcel_register();
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));
        store("44/1", 0);
        assert_noop!(
            Rules::evaluate_invariants(
                RuntimeOrigin::signed(CLERK),
                DEPLOYMENT,
                name("parcel"),
                record_key("44/1")
            ),
            Error::<Test>::RuleDeclinedWork
        );
    });
}

#[test]
fn the_sentences_of_each_kind_read_back_through_the_module() {
    new_test_ext().execute_with(|| {
        assert_ok!(put_invariant(&area_invariant("area_above_zero")));
        assert_ok!(put_precondition(&transfer_precondition("transfer_check")));

        let invariant = Pallet::<Test>::sentences(&DEPLOYMENT, &name("area_above_zero"))
            .expect("the registry holds the invariant");
        assert_eq!(invariant.kind, RuleKind::Invariant);
        assert_eq!(
            invariant.fix_text(),
            "Enter the measured area from the survey sheet."
        );

        let precondition = Pallet::<Test>::sentences(&DEPLOYMENT, &name("transfer_check"))
            .expect("the registry holds the precondition");
        assert_eq!(precondition.kind, RuleKind::Precondition);
        assert_eq!(
            precondition.line(),
            "The transfer names the present holder as the new holder. Name a different person as the new holder."
        );
        assert!(Pallet::<Test>::sentences(&DEPLOYMENT, &name("absent_rule")).is_none());
    });
}

#[test]
fn a_failure_keeps_at_most_the_sentence_bound() {
    let long = "a".repeat(MAX_SENTENCE_BYTES as usize + 40);
    let failure = RuleFailure::new(
        name("area_above_zero"),
        RuleKind::Invariant,
        &long,
        "Enter the measured area from the survey sheet.",
    );
    assert_eq!(failure.problem.len(), MAX_SENTENCE_BYTES as usize);
    assert_eq!(
        failure.fix_text(),
        "Enter the measured area from the survey sheet."
    );
    assert_eq!(
        failure.bytes().len(),
        MAX_SENTENCE_BYTES as usize + 1 + failure.fix.len()
    );
}
