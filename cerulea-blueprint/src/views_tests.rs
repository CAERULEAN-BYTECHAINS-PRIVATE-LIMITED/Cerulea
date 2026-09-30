//! Behavioural tests for the declarative queries.
//!
//! Each test starts from a clean view, breaks one rule, runs the real
//! validator and asserts the diagnostic class, the location and the fix.

use crate::{
    collections::{Collection, CollectionRegistry, PurposeDeclaration},
    expression::{BinaryOp, Expr},
    identities::{Identities, RoleDef},
    permissions::ROW_BINDING,
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, ParamName, PurposeTag, RecordDef, RoleId,
        TypeDef, TypeExpr, TypeId, TypeRegistry, ViewId,
    },
    views::{
        validate_views, Aggregate, AggregateFunction, Join, JoinKind, Projection, SortDirection,
        SortKey, View, ViewRegistry,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{string::ToString, vec, vec::Vec};

fn types() -> Vec<TypeDef> {
    vec![
        TypeDef::Record(RecordDef {
            id: TypeId::new("parcel"),
            fields: vec![
                FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                FieldDef::new("district", TypeExpr::Text, "The district."),
                FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
                FieldDef::new("recorded_at", TypeExpr::Timestamp, "The recording time."),
                FieldDef::new("owner_id", TypeExpr::Text, "The owner."),
            ],
            summary: "One land parcel.".to_string(),
        }),
        TypeDef::Record(RecordDef {
            id: TypeId::new("owner"),
            fields: vec![
                FieldDef::new("id", TypeExpr::Text, "The owner number."),
                FieldDef::new("age", TypeExpr::Integer, "The age in years."),
            ],
            summary: "One owner.".to_string(),
        }),
    ]
}

fn collections() -> Vec<Collection> {
    let mut parcel = Collection::new("parcel", "parcel", "survey_number", "The parcels.");
    parcel.purpose =
        PurposeDeclaration::new("The register records land.", &["registration", "audit"]);
    let mut owner = Collection::new("owner", "owner", "id", "The owners.");
    owner.purpose = PurposeDeclaration::new("The register records owners.", &["registration"]);
    vec![parcel, owner]
}

fn identities() -> Identities {
    Identities {
        roles: vec![RoleDef::new(
            "registrar",
            None,
            "Registrar",
            "The registrar reads.",
        )],
        ..Identities::default()
    }
}

fn row(field: &str) -> Expr {
    Expr::field(Expr::bound(ROW_BINDING), field)
}

fn clean() -> View {
    let mut view = View::new(
        "parcels_by_district",
        "parcel",
        "The view counts parcels in each district.",
    );
    view.joins = vec![Join {
        collection: CollectionId::new("owner"),
        alias: ParamName::new("holder"),
        left: FieldPath::parse("owner_id"),
        right: FieldPath::parse("id"),
        kind: JoinKind::Matched,
    }];
    view.projection = vec![Projection::field("district")];
    view.filter = Some(Expr::binary(
        BinaryOp::Greater,
        row("area"),
        Expr::decimal(0, 4),
    ));
    view.group_by = vec![FieldName::new("district")];
    view.aggregates = vec![
        Aggregate::count("parcels"),
        Aggregate::total("area_total", row("area")),
        Aggregate {
            name: FieldName::new("latest"),
            function: AggregateFunction::Largest,
            value: Some(row("recorded_at")),
        },
        Aggregate {
            name: FieldName::new("oldest_owner"),
            function: AggregateFunction::Smallest,
            value: Some(Expr::field(Expr::bound("holder"), "age")),
        },
    ];
    view.sort = vec![
        SortKey::ascending("district"),
        SortKey::descending("parcels"),
    ];
    view.limit = Some(10);
    view.purposes = vec![PurposeTag::new("registration")];
    view.roles = vec![RoleId::new("registrar")];
    view
}

fn report(view: &View) -> Report {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root().field("collections"));
    view.validate(
        &registry,
        &collections,
        &identities(),
        &Path::root().field("views").index(0),
    )
}

fn at<'a>(report: &'a Report, location: &str) -> &'a Diagnostic {
    report
        .find(location)
        .unwrap_or_else(|| panic!("no diagnostic sits at {location}: {report}"))
}

#[test]
fn a_clean_view_passes_and_lists_its_result() {
    let view = clean();
    let found = report(&view);
    assert!(found.is_clean(), "{found}");
    let names: Vec<&str> = view.result_names().iter().map(|n| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "district",
            "parcels",
            "area_total",
            "latest",
            "oldest_owner"
        ]
    );
    assert_eq!(AggregateFunction::Count.label(), "count");
    assert_eq!(AggregateFunction::Total.label(), "total");
    assert_eq!(AggregateFunction::Smallest.label(), "smallest");
    assert_eq!(AggregateFunction::Largest.label(), "largest");
    assert_eq!(view.sort[1].direction, SortDirection::Descending);
}

#[test]
fn the_result_types_follow_the_projection_and_the_aggregates() {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let mut view = clean();
    view.projection
        .push(Projection::computed("broken", Expr::bound("absent")));
    view.aggregates.push(Aggregate {
        name: FieldName::new("unread"),
        function: AggregateFunction::Total,
        value: None,
    });
    let result = view.result_types(&registry, &collections);
    let typed: Vec<(&str, TypeExpr)> = result
        .iter()
        .map(|(name, ty)| (name.as_str(), ty.clone()))
        .collect();
    assert_eq!(
        typed,
        [
            ("district", TypeExpr::Text),
            ("parcels", TypeExpr::Integer),
            ("area_total", TypeExpr::Decimal { scale: 4 }),
            ("latest", TypeExpr::Timestamp),
            ("oldest_owner", TypeExpr::Integer),
        ]
    );
}

#[test]
fn an_optional_join_binds_an_optional_record() {
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let mut view = clean();
    view.joins[0].kind = JoinKind::Optional;
    let ctx = view.context(&registry, &collections);
    assert_eq!(
        ctx.lookup(&ParamName::new("holder")),
        Some(&TypeExpr::optional(TypeExpr::Record(TypeId::new("owner"))))
    );
    assert_eq!(
        ctx.lookup(&ParamName::new(ROW_BINDING)),
        Some(&TypeExpr::Record(TypeId::new("parcel")))
    );
    let found = report(&view);
    let reading = at(&found, "views[0].aggregates[3].value");
    assert_eq!(reading.code, DiagnosticCode::TypeMismatch);
    assert_eq!(reading.fix, "Read a field out of a record value.");
}

#[test]
fn header_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.id = ViewId::new("Bad");
    view.source = CollectionId::new("ghost");
    view.summary.clear();
    let found = report(&view);
    assert_eq!(
        at(&found, "views[0].id").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "views[0].source").fix,
        "Point the reference at one of these collections: owner, parcel."
    );
    assert_eq!(
        at(&found, "views[0].summary").fix,
        "Write one plain sentence stating what the view answers."
    );
}

#[test]
fn join_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.joins.push(Join {
        collection: CollectionId::new("ghost"),
        alias: ParamName::new(ROW_BINDING),
        left: FieldPath::default(),
        right: FieldPath::parse("id"),
        kind: JoinKind::Matched,
    });
    view.joins.push(Join {
        collection: CollectionId::new("owner"),
        alias: ParamName::new("holder"),
        left: FieldPath::parse("district"),
        right: FieldPath::parse("age"),
        kind: JoinKind::Matched,
    });
    view.joins.push(Join {
        collection: CollectionId::new("owner"),
        alias: ParamName::new("second"),
        left: FieldPath::parse("missing"),
        right: FieldPath::parse("id"),
        kind: JoinKind::Matched,
    });
    view.joins.push(Join {
        collection: CollectionId::new("owner"),
        alias: ParamName::new("third"),
        left: FieldPath::parse("owner_id"),
        right: FieldPath::parse("missing"),
        kind: JoinKind::Matched,
    });
    let found = report(&view);

    let reserved = at(&found, "views[0].joins[1].alias");
    assert_eq!(reserved.code, DiagnosticCode::DuplicateName);
    assert_eq!(reserved.fix, "Give the join another alias.");
    assert_eq!(
        at(&found, "views[0].joins[1].collection").code,
        DiagnosticCode::DanglingReference
    );
    assert_eq!(
        at(&found, "views[0].joins[1].left").fix,
        "Name at least one field such as `holder`."
    );
    let repeated = at(&found, "views[0].joins[2].alias");
    assert_eq!(repeated.fix, "Rename one of the two `holder` joins.");
    let mismatch = at(&found, "views[0].joins[2]");
    assert_eq!(mismatch.code, DiagnosticCode::TypeMismatch);
    assert_eq!(
        mismatch.problem,
        "The join holds text on the left and a whole number on the right."
    );
    assert_eq!(mismatch.fix, "Join two fields sharing one type.");
    assert_eq!(
        at(&found, "views[0].joins[3].left").fix,
        "Name a field declared on the record type of `parcel`."
    );
    assert_eq!(
        at(&found, "views[0].joins[4].right").fix,
        "Name a field declared on the record type of `owner`."
    );
}

#[test]
fn projection_and_filter_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.projection.push(Projection::field("district"));
    view.projection
        .push(Projection::computed("9lives", Expr::bound("absent")));
    view.filter = Some(row("area"));
    let found = report(&view);
    assert_eq!(
        at(&found, "views[0].projection[1].name").fix,
        "Rename one of the two `district` fields."
    );
    assert_eq!(
        at(&found, "views[0].projection[2].name").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "views[0].projection[2].value").code,
        DiagnosticCode::DanglingReference
    );
    let filter = at(&found, "views[0].filter");
    assert_eq!(filter.code, DiagnosticCode::TypeMismatch);
    assert_eq!(filter.problem, "The filter carries a decimal at scale 4.");
    assert_eq!(filter.fix, "Give the filter a truth value.");

    let mut broken = clean();
    broken.filter = Some(Expr::bound("absent"));
    let found = report(&broken);
    assert_eq!(
        at(&found, "views[0].filter").code,
        DiagnosticCode::DanglingReference
    );
}

#[test]
fn a_view_returning_nothing_is_an_empty_section() {
    let mut view = clean();
    view.projection.clear();
    view.aggregates.clear();
    view.group_by.clear();
    view.sort.clear();
    let found = report(&view);
    let empty = at(&found, "views[0].projection");
    assert_eq!(empty.code, DiagnosticCode::SectionEmpty);
    assert_eq!(
        empty.fix,
        "Project at least one field, or compute at least one aggregate."
    );
}

#[test]
fn aggregate_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.aggregates.push(Aggregate {
        name: FieldName::new("parcels"),
        function: AggregateFunction::Count,
        value: Some(row("area")),
    });
    view.aggregates
        .push(Aggregate::total("district_total", row("district")));
    view.aggregates
        .push(Aggregate::total("time_total", row("recorded_at")));
    view.aggregates
        .push(Aggregate::total("broken", Expr::bound("absent")));
    view.aggregates.push(Aggregate {
        name: FieldName::new("nothing"),
        function: AggregateFunction::Smallest,
        value: None,
    });
    let found = report(&view);
    assert_eq!(
        at(&found, "views[0].aggregates[4].name").fix,
        "Rename one of the two `parcels` fields."
    );
    assert_eq!(
        at(&found, "views[0].aggregates[4].value").fix,
        "Remove the value from the count."
    );
    let text = at(&found, "views[0].aggregates[5].value");
    assert_eq!(text.problem, "The total reads text.");
    assert_eq!(text.fix, "Read a whole number or a decimal here.");
    assert_eq!(
        at(&found, "views[0].aggregates[6].value").problem,
        "The total reads a timestamp."
    );
    assert_eq!(
        at(&found, "views[0].aggregates[7].value").code,
        DiagnosticCode::DanglingReference
    );
    let nothing = at(&found, "views[0].aggregates[8].value");
    assert_eq!(nothing.code, DiagnosticCode::MissingValue);
    assert_eq!(nothing.problem, "The smallest `nothing` reads nothing.");
    assert_eq!(nothing.fix, "Name the value the operation reads.");
}

#[test]
fn grouping_sorting_and_limit_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.group_by = vec![FieldName::new("area"), FieldName::new("Bad")];
    view.sort.push(SortKey::ascending("area"));
    view.limit = Some(0);
    let found = report(&view);
    let group = at(&found, "views[0].group_by[0]");
    assert_eq!(group.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        group.fix,
        "Project a field named `area`, or group by a projected field."
    );
    assert_eq!(
        at(&found, "views[0].group_by[1]").code,
        DiagnosticCode::InvalidIdentifier
    );
    assert_eq!(
        at(&found, "views[0].sort[2].name").fix,
        "Sort by a projected field, or project `area`."
    );
    assert_eq!(
        at(&found, "views[0].limit").fix,
        "Set a limit above zero, or clear the limit."
    );

    let mut ungrouped = clean();
    ungrouped.group_by.clear();
    let found = report(&ungrouped);
    let missing = at(&found, "views[0].group_by");
    assert_eq!(missing.code, DiagnosticCode::MissingValue);
    assert_eq!(
        missing.fix,
        "Group by `district`, or remove the projection."
    );
}

#[test]
fn role_and_purpose_faults_each_raise_a_diagnostic() {
    let mut view = clean();
    view.roles = vec![RoleId::new("auditor")];
    view.purposes = vec![PurposeTag::new("marketing")];
    let found = report(&view);
    assert_eq!(
        at(&found, "views[0].roles[0]").fix,
        "Point the entry at one of these roles: registrar."
    );
    let purpose = at(&found, "views[0].purposes[0]");
    assert_eq!(purpose.code, DiagnosticCode::DanglingReference);
    assert_eq!(
        purpose.fix,
        "Declare `marketing` on the collection `parcel`, or name a purpose the collection already declares."
    );

    let mut silent = clean();
    silent.purposes.clear();
    let found = report(&silent);
    assert_eq!(
        at(&found, "views[0].purposes").fix,
        "Declare the purpose the read serves so the purpose check passes."
    );
}

#[test]
fn the_registry_answers_lookups_and_names_the_views_on_offer() {
    let views = vec![clean(), View::new("owners", "owner", "The owners.")];
    let registry = ViewRegistry::build(&views);
    assert_eq!(registry.len(), 2);
    assert!(!registry.is_empty());
    assert!(registry.contains(&ViewId::new("owners")));
    assert_eq!(
        registry
            .get(&ViewId::new("owners"))
            .map(|view| view.source.as_str()),
        Some("owner")
    );
    assert_eq!(registry.names(), ["owners", "parcels_by_district"]);
    let listed = registry.missing(&ViewId::new("ghost"), Location("here".to_string()));
    assert_eq!(
        listed.fix,
        "Point the entry at one of these views: owners, parcels_by_district."
    );
    let empty_views: Vec<View> = Vec::new();
    let empty = ViewRegistry::build(&empty_views);
    assert!(empty.is_empty());
    assert_eq!(
        empty
            .missing(&ViewId::new("ghost"), Location::default())
            .fix,
        "Declare a view named `ghost` in the views section."
    );
}

#[test]
fn a_repeated_view_name_is_reported_by_the_section_pass() {
    let views = vec![clean(), clean()];
    let defs = types();
    let (registry, _) = TypeRegistry::build(&defs, &Path::root());
    let held = collections();
    let (collections, _) = CollectionRegistry::build(&held, &Path::root());
    let found = validate_views(
        &views,
        &registry,
        &collections,
        &identities(),
        &Path::root().field("views"),
    );
    assert_eq!(found.len(), 1);
    let repeated = at(&found, "views[1].id");
    assert_eq!(repeated.code, DiagnosticCode::DuplicateIdentifier);
    assert_eq!(
        repeated.fix,
        "Rename one of the two views away from `parcels_by_district`."
    );
}
