//! The tests of the three migration passes.
//!
//! Every pass is pure, so the suite drives each pass over a plain parcel
//! register. Each problem the planning pass reports carries its location
//! and its fix, and the suite checks the fix sentence a person reads.

use crate::migrate::{
    check_plan, inverse_plan, migrate_record, plan_collection, plan_record_type, preview,
    revert_available, revert_target, with_replaced_record, MigrationPlan, MigrationStep,
    OLD_RECORD_BINDING, OLD_VALUE_BINDING,
};
use cerulea_blueprint::{
    collections::{
        Collection, Index, PurposeDeclaration, Retention, RetentionAction, RetentionPeriod,
    },
    expression::{BinaryOp, Env, Expr, Meter},
    types::{
        FieldDef, FieldName, FieldPath, Identity, IndexName, RecordDef, Timestamp, TypeDef,
        TypeExpr, TypeId, TypeRegistry, Value,
    },
    Path, Report,
};

fn parcel_record_def() -> RecordDef {
    RecordDef {
        id: TypeId::new("parcel"),
        fields: vec![
            FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
            FieldDef::new("holder", TypeExpr::Text, "The present holder."),
            FieldDef::new("area", TypeExpr::Integer, "The area in square metres."),
        ],
        summary: "One land parcel in the register.".to_string(),
    }
}

fn defs() -> Vec<TypeDef> {
    vec![TypeDef::Record(parcel_record_def())]
}

fn by_holder() -> Index {
    Index {
        name: IndexName::new("by_holder"),
        fields: vec![FieldPath::parse("holder")],
        unique: false,
    }
}

fn parcel_collection() -> Collection {
    let mut held = Collection::new("parcel", "parcel", "survey_number", "The register.");
    held.purpose = PurposeDeclaration::new("The register records holders.", &["registration"]);
    held.indexes.push(by_holder());
    held
}

fn parcel(survey: &str, holder: &str, area: i128) -> Value {
    let mut value = Value::record(
        TypeId::new("parcel"),
        vec![
            (
                FieldName::new("survey_number"),
                Value::Text(survey.to_string()),
            ),
            (FieldName::new("holder"), Value::Text(holder.to_string())),
            (FieldName::new("area"), Value::Integer(area)),
        ],
    );
    value.canonicalise();
    value
}

fn plan() -> MigrationPlan {
    MigrationPlan::new("parcel", 1, "The register reshapes the parcel record.")
}

fn root() -> Path {
    Path::root().field("migration")
}

/// Run the planning pass over the parcel register at version one.
fn checked(plan: &MigrationPlan) -> Report {
    let held = defs();
    let (types, _) = TypeRegistry::build(&held, &Path::root().field("types"));
    check_plan(plan, &parcel_collection(), 1, &types, &root())
}

fn fixes(report: &Report) -> Vec<String> {
    report
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.fix.clone())
        .collect()
}

fn env() -> Env<'static> {
    Env::new(Identity([0u8; 32]), Timestamp::from_millis(0))
}

fn add_field(name: &str, ty: TypeExpr, default: Value, summary: &str) -> MigrationStep {
    MigrationStep::AddField {
        field: FieldName::new(name),
        ty,
        default,
        summary: summary.to_string(),
    }
}

// The step labels and the cost facts.

#[test]
fn every_step_carries_its_label_and_its_cost_facts() {
    let steps = vec![
        (
            add_field(
                "note",
                TypeExpr::Text,
                Value::Text(String::new()),
                "A note.",
            ),
            "add field",
            true,
            false,
        ),
        (
            MigrationStep::RemoveField {
                field: FieldName::new("area"),
            },
            "remove field",
            true,
            false,
        ),
        (
            MigrationStep::RenameField {
                from: FieldName::new("holder"),
                to: FieldName::new("owner"),
            },
            "rename field",
            true,
            false,
        ),
        (
            MigrationStep::RetypeField {
                field: FieldName::new("area"),
                ty: TypeExpr::Integer,
                convert: Expr::bound(OLD_VALUE_BINDING),
            },
            "retype field",
            true,
            false,
        ),
        (
            MigrationStep::AddIndex { index: by_holder() },
            "add index",
            false,
            true,
        ),
        (
            MigrationStep::RemoveIndex {
                name: IndexName::new("by_holder"),
            },
            "remove index",
            false,
            true,
        ),
        (
            MigrationStep::SetRetention {
                retention: Retention::default(),
            },
            "set retention",
            false,
            false,
        ),
        (
            MigrationStep::SetPurpose {
                purpose: PurposeDeclaration::default(),
            },
            "set purpose",
            false,
            false,
        ),
    ];
    for (step, label, records, indexes) in steps {
        assert_eq!(step.label(), label);
        assert_eq!(step.touches_records(), records, "{label} and the records");
        assert_eq!(step.touches_indexes(), indexes, "{label} and the indexes");
        let single = plan().with(step);
        assert_eq!(single.touches_records(), records);
        assert_eq!(single.touches_indexes(), indexes);
    }
}

#[test]
fn a_new_plan_raises_the_version_by_one_and_keeps_its_window() {
    let held = plan().within(40);
    assert_eq!(held.from_version, 1);
    assert_eq!(held.to_version, 2);
    assert_eq!(held.rollback_window, 40);
    assert!(held.steps.is_empty());
    assert_eq!(revert_target(&held), 1);
    let top = MigrationPlan::new("parcel", u32::MAX, "The last version.");
    assert_eq!(top.to_version, u32::MAX);
}

// The planning pass.

#[test]
fn a_plan_naming_another_collection_stops_at_the_first_problem() {
    let held = MigrationPlan::new("deed", 1, "The register reshapes deeds.").with(
        MigrationStep::RemoveField {
            field: FieldName::new("absent"),
        },
    );
    let report = checked(&held);
    assert_eq!(
        fixes(&report),
        vec!["Point the plan at the collection the registry holds.".to_string()]
    );
    assert_eq!(
        report.diagnostics[0].location.as_str(),
        "migration.collection"
    );
}

#[test]
fn a_plan_with_wrong_versions_an_empty_summary_and_no_steps_names_each_fix() {
    let mut held = MigrationPlan::new("parcel", 3, "");
    held.to_version = 2;
    let report = checked(&held);
    assert_eq!(
        fixes(&report),
        vec![
            "Rewrite the plan to start from version 1.".to_string(),
            "Give the plan a target version above the version it starts from.".to_string(),
            "Write one plain sentence stating what the plan changes.".to_string(),
            "Add at least one step, or withdraw the plan.".to_string(),
        ]
    );
}

#[test]
fn a_plan_over_a_record_type_the_registry_lacks_names_the_registration_fix() {
    let held = plan().with(add_field(
        "note",
        TypeExpr::Text,
        Value::Text(String::new()),
        "A note.",
    ));
    let empty: Vec<TypeDef> = Vec::new();
    let (types, _) = TypeRegistry::build(&empty, &Path::root().field("types"));
    let report = check_plan(&held, &parcel_collection(), 1, &types, &root());
    assert_eq!(
        fixes(&report),
        vec!["Register the record type before planning a migration.".to_string()]
    );
}

#[test]
fn a_clean_plan_over_every_step_kind_reports_nothing() {
    let held = plan()
        .with(add_field(
            "district",
            TypeExpr::Text,
            Value::Text("Rangareddy".to_string()),
            "The district of the parcel.",
        ))
        .with(MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .with(MigrationStep::RetypeField {
            field: FieldName::new("area"),
            ty: TypeExpr::Integer,
            convert: Expr::binary(
                BinaryOp::Add,
                Expr::bound(OLD_VALUE_BINDING),
                Expr::integer(1),
            ),
        })
        .with(MigrationStep::RemoveIndex {
            name: IndexName::new("by_holder"),
        })
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_district"),
                fields: vec![FieldPath::parse("district")],
                unique: false,
            },
        })
        .with(MigrationStep::SetRetention {
            retention: Retention {
                period: RetentionPeriod::Milliseconds(60_000),
                on_expiry: RetentionAction::Review,
            },
        })
        .with(MigrationStep::SetPurpose {
            purpose: PurposeDeclaration::new("The register records districts.", &["audit"]),
        })
        .with(MigrationStep::RemoveField {
            field: FieldName::new("district"),
        });
    let report = checked(&held);
    assert!(report.is_clean(), "{:?}", report.diagnostics);
}

#[test]
fn an_added_field_already_present_with_a_wrong_default_and_no_summary_names_each_fix() {
    let held = plan().with(add_field(
        "holder",
        TypeExpr::Integer,
        Value::Text("wrong".to_string()),
        "",
    ));
    let report = checked(&held);
    let found = fixes(&report);
    assert_eq!(found[0], "Retype `holder` rather than adding it again.");
    assert_eq!(
        found.last().expect("the report carries the summary fix"),
        "Write one plain sentence stating what the field holds."
    );
    assert_eq!(found.len(), 3, "{found:?}");
    assert_eq!(
        report.diagnostics[1].location.as_str(),
        "migration.steps[0].default"
    );
}

#[test]
fn a_removed_field_the_record_lacks_or_the_primary_key_names_each_fix() {
    let held = plan()
        .with(MigrationStep::RemoveField {
            field: FieldName::new("district"),
        })
        .with(MigrationStep::RemoveField {
            field: FieldName::new("survey_number"),
        });
    assert_eq!(
        fixes(&checked(&held)),
        vec![
            "Name a field the record carries.".to_string(),
            "Keep the primary key field. Create a new collection for a new key.".to_string(),
        ]
    );
}

#[test]
fn a_rename_from_a_missing_field_onto_a_present_field_names_each_fix() {
    let held = plan().with(MigrationStep::RenameField {
        from: FieldName::new("district"),
        to: FieldName::new("area"),
    });
    assert_eq!(
        fixes(&checked(&held)),
        vec![
            "Name a field the record carries.".to_string(),
            "Choose a name the record lacks, or remove `area` first.".to_string(),
        ]
    );
}

#[test]
fn a_retype_over_a_missing_field_names_the_fix() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("district"),
        ty: TypeExpr::Text,
        convert: Expr::text("none"),
    });
    assert_eq!(
        fixes(&checked(&held)),
        vec!["Name a field the record carries.".to_string()]
    );
}

#[test]
fn a_conversion_producing_another_type_names_the_type_it_must_produce() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("area"),
        ty: TypeExpr::Integer,
        convert: Expr::text("large"),
    });
    let report = checked(&held);
    assert_eq!(report.len(), 1);
    assert!(report.diagnostics[0]
        .fix
        .starts_with("Rewrite the conversion to produce "));
    assert_eq!(
        report.diagnostics[0].location.as_str(),
        "migration.steps[0].convert"
    );
}

#[test]
fn a_conversion_reading_an_unbound_name_reports_the_type_check_problem() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("area"),
        ty: TypeExpr::Integer,
        convert: Expr::bound("unbound_name"),
    });
    let report = checked(&held);
    assert_eq!(report.len(), 1);
    assert!(report.diagnostics[0]
        .location
        .as_str()
        .starts_with("migration.steps[0].convert"));
}

#[test]
fn a_conversion_reads_the_whole_old_record() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("holder"),
        ty: TypeExpr::Text,
        convert: Expr::field(Expr::bound(OLD_RECORD_BINDING), "survey_number"),
    });
    assert!(checked(&held).is_clean());
}

#[test]
fn an_added_index_repeating_a_name_covering_nothing_or_a_missing_field_names_each_fix() {
    let held = plan()
        .with(MigrationStep::AddIndex { index: by_holder() })
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_nothing"),
                fields: Vec::new(),
                unique: false,
            },
        })
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_district"),
                fields: vec![FieldPath::parse("district")],
                unique: true,
            },
        });
    assert_eq!(
        fixes(&checked(&held)),
        vec![
            "Remove the present index before adding a new one under that name.".to_string(),
            "Name at least one field for the index to cover.".to_string(),
            "Name a field the record carries.".to_string(),
        ]
    );
}

#[test]
fn a_removed_index_the_collection_lacks_names_the_fix() {
    let held = plan().with(MigrationStep::RemoveIndex {
        name: IndexName::new("by_district"),
    });
    assert_eq!(
        fixes(&checked(&held)),
        vec!["Name an index the collection carries.".to_string()]
    );
}

#[test]
fn an_empty_purpose_declaration_names_each_fix() {
    let held = plan().with(MigrationStep::SetPurpose {
        purpose: PurposeDeclaration::default(),
    });
    assert_eq!(
        fixes(&checked(&held)),
        vec![
            "Write one plain sentence stating why the collection holds records.".to_string(),
            "Name at least one purpose the collection serves.".to_string(),
        ]
    );
}

// The definition pass.

#[test]
fn the_definition_pass_produces_the_record_type_the_plan_yields() {
    let held = plan()
        .with(add_field(
            "district",
            TypeExpr::Text,
            Value::Text(String::new()),
            "The district.",
        ))
        .with(MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .with(MigrationStep::RetypeField {
            field: FieldName::new("area"),
            ty: TypeExpr::Decimal { scale: 2 },
            convert: Expr::bound(OLD_VALUE_BINDING),
        })
        .with(MigrationStep::RemoveField {
            field: FieldName::new("district"),
        })
        .with(MigrationStep::AddIndex { index: by_holder() });
    let produced =
        plan_record_type(&held, &parcel_record_def(), &root()).expect("the plan applies");
    let names: Vec<&str> = produced
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    assert_eq!(names, vec!["survey_number", "owner", "area"]);
    assert_eq!(
        produced
            .field(&FieldName::new("area"))
            .map(|field| &field.ty),
        Some(&TypeExpr::Decimal { scale: 2 })
    );
}

#[test]
fn the_definition_pass_declines_a_step_naming_a_missing_field() {
    let cases = vec![
        (
            MigrationStep::RemoveField {
                field: FieldName::new("district"),
            },
            "migration.steps[0].field",
        ),
        (
            MigrationStep::RenameField {
                from: FieldName::new("district"),
                to: FieldName::new("zone"),
            },
            "migration.steps[0].from",
        ),
        (
            MigrationStep::RetypeField {
                field: FieldName::new("district"),
                ty: TypeExpr::Text,
                convert: Expr::bound(OLD_VALUE_BINDING),
            },
            "migration.steps[0].field",
        ),
    ];
    for (step, location) in cases {
        let diagnostic = plan_record_type(&plan().with(step), &parcel_record_def(), &root())
            .expect_err("the record lacks the field");
        assert_eq!(diagnostic.fix, "Name a field the record carries.");
        assert_eq!(diagnostic.location.as_str(), location);
    }
}

#[test]
fn the_collection_pass_follows_renames_retention_purpose_and_indexes() {
    let retention = Retention {
        period: RetentionPeriod::Milliseconds(86_400_000),
        on_expiry: RetentionAction::Erase,
    };
    let purpose = PurposeDeclaration::new("The register serves audit.", &["audit"]);
    let held = plan()
        .with(MigrationStep::RenameField {
            from: FieldName::new("survey_number"),
            to: FieldName::new("survey"),
        })
        .with(MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_area"),
                fields: vec![FieldPath::parse("area")],
                unique: false,
            },
        })
        .with(MigrationStep::SetRetention {
            retention: retention.clone(),
        })
        .with(MigrationStep::SetPurpose {
            purpose: purpose.clone(),
        })
        .with(add_field(
            "district",
            TypeExpr::Text,
            Value::Text(String::new()),
            "The district.",
        ));
    let produced = plan_collection(&held, &parcel_collection(), &root()).expect("the plan applies");
    assert_eq!(produced.primary_key, FieldPath::parse("survey"));
    assert_eq!(produced.indexes[0].fields, vec![FieldPath::parse("owner")]);
    assert_eq!(produced.indexes[1].name, IndexName::new("by_area"));
    assert_eq!(produced.retention, retention);
    assert_eq!(produced.purpose, purpose);

    let removed = plan_collection(
        &plan().with(MigrationStep::RemoveIndex {
            name: IndexName::new("by_holder"),
        }),
        &parcel_collection(),
        &root(),
    )
    .expect("the index is present");
    assert!(removed.indexes.is_empty());
}

#[test]
fn the_collection_pass_declines_a_repeated_or_missing_index() {
    let repeated = plan_collection(
        &plan().with(MigrationStep::AddIndex { index: by_holder() }),
        &parcel_collection(),
        &root(),
    )
    .expect_err("the index is present");
    assert_eq!(repeated.fix, "Remove the present index first.");

    let missing = plan_collection(
        &plan().with(MigrationStep::RemoveIndex {
            name: IndexName::new("by_area"),
        }),
        &parcel_collection(),
        &root(),
    )
    .expect_err("the index is absent");
    assert_eq!(missing.fix, "Name an index the collection carries.");
}

#[test]
fn a_rename_elsewhere_leaves_the_primary_key_alone() {
    let produced = plan_collection(
        &plan().with(MigrationStep::RenameField {
            from: FieldName::new("area"),
            to: FieldName::new("size"),
        }),
        &parcel_collection(),
        &root(),
    )
    .expect("the plan applies");
    assert_eq!(produced.primary_key, FieldPath::parse("survey_number"));
    assert_eq!(produced.indexes[0].fields, vec![FieldPath::parse("holder")]);
}

// The record pass.

#[test]
fn the_record_pass_removes_a_field_and_keeps_the_rest() {
    let held = plan().with(MigrationStep::RemoveField {
        field: FieldName::new("area"),
    });
    let mut meter = Meter::new(10_000);
    let produced = migrate_record(
        &held,
        &parcel("12/3", "Anitha", 400),
        &env(),
        &mut meter,
        &root(),
    )
    .expect("the rewrite runs");
    match produced {
        Value::Record(record) => {
            assert_eq!(record.field(&FieldName::new("area")), None);
            assert_eq!(
                record.field(&FieldName::new("holder")),
                Some(&Value::Text("Anitha".to_string()))
            );
        }
        other => panic!("the rewrite yields a record, and this reads {other:?}"),
    }
}

#[test]
fn the_record_pass_reads_the_old_record_inside_a_conversion() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("holder"),
        ty: TypeExpr::Text,
        convert: Expr::field(Expr::bound(OLD_RECORD_BINDING), "survey_number"),
    });
    let mut meter = Meter::new(10_000);
    let produced = migrate_record(
        &held,
        &parcel("12/3", "Anitha", 400),
        &env(),
        &mut meter,
        &root(),
    )
    .expect("the rewrite runs");
    match produced {
        Value::Record(record) => assert_eq!(
            record.field(&FieldName::new("holder")),
            Some(&Value::Text("12/3".to_string()))
        ),
        other => panic!("the rewrite yields a record, and this reads {other:?}"),
    }
}

#[test]
fn the_record_pass_declines_a_value_other_than_a_record() {
    let mut meter = Meter::new(10_000);
    let diagnostic = migrate_record(&plan(), &Value::Integer(4), &env(), &mut meter, &root())
        .expect_err("a collection stores record values");
    assert_eq!(
        diagnostic.fix,
        "Migrate a record value. A collection stores record values."
    );
}

#[test]
fn the_record_pass_declines_a_record_lacking_the_retyped_field() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("district"),
        ty: TypeExpr::Text,
        convert: Expr::bound(OLD_VALUE_BINDING),
    });
    let mut meter = Meter::new(10_000);
    let diagnostic = migrate_record(
        &held,
        &parcel("12/3", "Anitha", 400),
        &env(),
        &mut meter,
        &root(),
    )
    .expect_err("the record lacks the field");
    assert_eq!(
        diagnostic.fix,
        "Apply the plan to records carrying the declared shape."
    );
    assert_eq!(diagnostic.location.as_str(), "migration.steps[0].field");
}

#[test]
fn the_record_pass_stops_when_the_conversion_runs_past_its_meter() {
    let held = plan().with(MigrationStep::RetypeField {
        field: FieldName::new("area"),
        ty: TypeExpr::Integer,
        convert: Expr::binary(
            BinaryOp::Add,
            Expr::bound(OLD_VALUE_BINDING),
            Expr::integer(1),
        ),
    });
    let mut meter = Meter::new(0);
    assert!(migrate_record(
        &held,
        &parcel("12/3", "Anitha", 400),
        &env(),
        &mut meter,
        &root()
    )
    .is_err());
}

#[test]
fn the_record_pass_leaves_records_alone_for_steps_over_the_collection() {
    let held = plan()
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_area"),
                fields: vec![FieldPath::parse("area")],
                unique: false,
            },
        })
        .with(MigrationStep::RemoveIndex {
            name: IndexName::new("by_holder"),
        })
        .with(MigrationStep::SetRetention {
            retention: Retention::default(),
        })
        .with(MigrationStep::SetPurpose {
            purpose: PurposeDeclaration::new("The register serves audit.", &["audit"]),
        });
    let record = parcel("12/3", "Anitha", 400);
    let mut meter = Meter::new(10_000);
    assert_eq!(
        migrate_record(&held, &record, &env(), &mut meter, &root()).expect("the rewrite runs"),
        record
    );
}

// The reverse.

#[test]
fn the_reverse_walks_the_steps_backwards_and_inverts_each_one() {
    let retention = Retention {
        period: RetentionPeriod::Milliseconds(5_000),
        on_expiry: RetentionAction::Erase,
    };
    let held = plan()
        .within(12)
        .with(add_field(
            "district",
            TypeExpr::Text,
            Value::Text(String::new()),
            "The district.",
        ))
        .with(MigrationStep::RenameField {
            from: FieldName::new("holder"),
            to: FieldName::new("owner"),
        })
        .with(MigrationStep::AddIndex {
            index: Index {
                name: IndexName::new("by_area"),
                fields: vec![FieldPath::parse("area")],
                unique: false,
            },
        })
        .with(MigrationStep::RemoveIndex {
            name: IndexName::new("by_holder"),
        })
        .with(MigrationStep::SetRetention { retention })
        .with(MigrationStep::SetPurpose {
            purpose: PurposeDeclaration::new("Another purpose.", &["audit"]),
        });
    let previous = parcel_collection();
    let inverse = inverse_plan(&held, &previous, &root()).expect("every step inverts");
    assert_eq!(inverse.from_version, 2);
    assert_eq!(inverse.to_version, 1);
    assert_eq!(inverse.rollback_window, 12);
    assert_eq!(inverse.summary, "The revert restores version 1.");
    assert_eq!(
        inverse.steps,
        vec![
            MigrationStep::SetPurpose {
                purpose: previous.purpose.clone(),
            },
            MigrationStep::SetRetention {
                retention: previous.retention.clone(),
            },
            MigrationStep::AddIndex { index: by_holder() },
            MigrationStep::RemoveIndex {
                name: IndexName::new("by_area"),
            },
            MigrationStep::RenameField {
                from: FieldName::new("owner"),
                to: FieldName::new("holder"),
            },
            MigrationStep::RemoveField {
                field: FieldName::new("district"),
            },
        ]
    );
}

#[test]
fn a_removed_index_the_earlier_collection_lacks_carries_no_reverse() {
    let held = plan().with(MigrationStep::RemoveIndex {
        name: IndexName::new("by_area"),
    });
    let diagnostic =
        inverse_plan(&held, &parcel_collection(), &root()).expect_err("the index is absent");
    assert_eq!(
        diagnostic.fix,
        "Propose a forward plan adding the index back."
    );
}

#[test]
fn a_removed_or_retyped_field_carries_no_reverse_and_names_the_way_forward() {
    let removed = inverse_plan(
        &plan().with(MigrationStep::RemoveField {
            field: FieldName::new("area"),
        }),
        &parcel_collection(),
        &root(),
    )
    .expect_err("the values left with the field");
    assert_eq!(
        removed.fix,
        "Propose a forward plan adding the field back, or restore from an export."
    );

    let retyped = inverse_plan(
        &plan().with(MigrationStep::RetypeField {
            field: FieldName::new("area"),
            ty: TypeExpr::Text,
            convert: Expr::text("large"),
        }),
        &parcel_collection(),
        &root(),
    )
    .expect_err("the earlier values left with the retype");
    assert_eq!(
        retyped.fix,
        "Propose a forward plan converting the field back, or restore from an export."
    );
}

#[test]
fn a_revert_stays_available_inside_its_window() {
    assert!(revert_available(10, 1_000_000, 0));
    assert!(revert_available(10, 10, 5));
    assert!(revert_available(10, 15, 5));
    assert!(!revert_available(10, 16, 5));
    assert!(revert_available(20, 10, 5));
}

#[test]
fn a_replaced_record_type_takes_the_place_of_the_earlier_one_in_name_order() {
    let other = TypeDef::Record(RecordDef {
        id: TypeId::new("deed"),
        fields: vec![FieldDef::new("number", TypeExpr::Text, "The deed number.")],
        summary: "One deed.".to_string(),
    });
    let mut replacement = parcel_record_def();
    replacement
        .fields
        .push(FieldDef::new("district", TypeExpr::Text, "The district."));
    let produced = with_replaced_record(
        &[TypeDef::Record(parcel_record_def()), other],
        replacement.clone(),
    );
    assert_eq!(produced.len(), 2);
    assert_eq!(produced[0].id(), &TypeId::new("deed"));
    assert_eq!(produced[1], TypeDef::Record(replacement));
}

// The preview.

#[test]
fn the_preview_of_a_clean_plan_carries_the_definitions_and_the_cost_facts() {
    let held = defs();
    let (types, _) = TypeRegistry::build(&held, &Path::root().field("types"));
    let reversible = plan().with(add_field(
        "district",
        TypeExpr::Text,
        Value::Text(String::new()),
        "The district.",
    ));
    let shown =
        preview(&reversible, &parcel_collection(), 1, &types, &root()).expect("the plan is clean");
    assert!(shown.rewrites_records);
    assert!(shown.rebuilds_indexes);
    assert!(shown.reversible);
    assert_eq!(shown.produced_type.fields.len(), 4);
    assert_eq!(shown.produced_collection, parcel_collection());

    let one_way = plan().with(MigrationStep::RemoveField {
        field: FieldName::new("area"),
    });
    let shown =
        preview(&one_way, &parcel_collection(), 1, &types, &root()).expect("the plan is clean");
    assert!(!shown.reversible);

    let index_only = plan().with(MigrationStep::RemoveIndex {
        name: IndexName::new("by_holder"),
    });
    let shown =
        preview(&index_only, &parcel_collection(), 1, &types, &root()).expect("the plan is clean");
    assert!(!shown.rewrites_records);
    assert!(shown.rebuilds_indexes);
    assert!(shown.produced_collection.indexes.is_empty());
}

#[test]
fn the_preview_of_a_plan_carrying_a_problem_yields_the_report() {
    let held = defs();
    let (types, _) = TypeRegistry::build(&held, &Path::root().field("types"));
    let report = preview(&plan(), &parcel_collection(), 1, &types, &root())
        .expect_err("the plan holds zero steps");
    assert_eq!(
        fixes(&report),
        vec!["Add at least one step, or withdraw the plan.".to_string()]
    );
}
