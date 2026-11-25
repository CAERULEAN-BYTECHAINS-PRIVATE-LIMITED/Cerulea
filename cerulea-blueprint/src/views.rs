//! Declarative queries.
//!
//! A view names one source collection, the joins it follows, the fields it
//! projects, the filter it applies, the groups it forms, the aggregates it
//! computes and the order it returns.
//!
//! A view reads state. A view changes nothing. The source record binds to
//! the name `record`. Each join binds under the alias the join declares.
//!
//! A mean is written as a sum divided by a count inside a projection. Every
//! aggregate stays exact.

use crate::{
    collections::CollectionRegistry,
    expression::{Expr, TypeContext},
    identities::Identities,
    permissions::ROW_BINDING,
    types::{
        CollectionId, FieldName, FieldPath, ParamName, PurposeTag, RoleId, TypeExpr, TypeRegistry,
        ViewId,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The shape of a join.
#[derive(
    Clone,
    Copy,
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
pub enum JoinKind {
    /// The view returns the rows where both sides hold a record.
    #[default]
    Matched,
    /// The view returns every source row. The joined side is optional.
    Optional,
}

/// One join a view follows.
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
pub struct Join {
    /// The collection the join reads.
    pub collection: CollectionId,
    /// The name the joined record binds to.
    pub alias: ParamName,
    /// The field on the source side.
    pub left: FieldPath,
    /// The field on the joined side.
    pub right: FieldPath,
    /// The shape of the join.
    pub kind: JoinKind,
}

/// One projected field.
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
pub struct Projection {
    /// The name the field takes in the result.
    pub name: FieldName,
    /// The value the field carries.
    pub value: Expr,
}

impl Projection {
    /// Project one field of the source record under its own name.
    pub fn field(name: &str) -> Self {
        Projection {
            name: FieldName::new(name),
            value: Expr::field(Expr::bound(ROW_BINDING), name),
        }
    }

    /// Project a computed value under a name.
    pub fn computed(name: &str, value: Expr) -> Self {
        Projection {
            name: FieldName::new(name),
            value,
        }
    }
}

/// The direction a sort runs.
#[derive(
    Clone,
    Copy,
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
pub enum SortDirection {
    /// Smallest first.
    #[default]
    Ascending,
    /// Largest first.
    Descending,
}

/// One sort key.
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
pub struct SortKey {
    /// The result field the sort reads.
    pub name: FieldName,
    /// The direction the sort runs.
    pub direction: SortDirection,
}

impl SortKey {
    /// Sort smallest first.
    pub fn ascending(name: &str) -> Self {
        SortKey {
            name: FieldName::new(name),
            direction: SortDirection::Ascending,
        }
    }

    /// Sort largest first.
    pub fn descending(name: &str) -> Self {
        SortKey {
            name: FieldName::new(name),
            direction: SortDirection::Descending,
        }
    }
}

/// The operation an aggregate runs over a group.
#[derive(
    Clone,
    Copy,
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
pub enum AggregateFunction {
    /// The count of rows in the group.
    #[default]
    Count,
    /// The exact total of the values in the group.
    Total,
    /// The smallest value in the group.
    Smallest,
    /// The largest value in the group.
    Largest,
}

impl AggregateFunction {
    /// A word naming the operation for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            AggregateFunction::Count => "count",
            AggregateFunction::Total => "total",
            AggregateFunction::Smallest => "smallest",
            AggregateFunction::Largest => "largest",
        }
    }
}

/// One aggregate a view computes.
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
pub struct Aggregate {
    /// The name the aggregate takes in the result.
    pub name: FieldName,
    /// The operation the aggregate runs.
    pub function: AggregateFunction,
    /// The value the operation reads. A count reads nothing.
    pub value: Option<Expr>,
}

impl Aggregate {
    /// Count the rows in each group.
    pub fn count(name: &str) -> Self {
        Aggregate {
            name: FieldName::new(name),
            function: AggregateFunction::Count,
            value: None,
        }
    }

    /// Total the values in each group.
    pub fn total(name: &str, value: Expr) -> Self {
        Aggregate {
            name: FieldName::new(name),
            function: AggregateFunction::Total,
            value: Some(value),
        }
    }
}

/// A view.
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
pub struct View {
    /// The name of the view.
    pub id: ViewId,
    /// The collection the view reads.
    pub source: CollectionId,
    /// The joins the view follows.
    pub joins: Vec<Join>,
    /// The fields the view projects.
    pub projection: Vec<Projection>,
    /// The filter the view applies to each row.
    pub filter: Option<Expr>,
    /// The result fields the view groups by.
    pub group_by: Vec<FieldName>,
    /// The aggregates the view computes over each group.
    pub aggregates: Vec<Aggregate>,
    /// The order the view returns.
    pub sort: Vec<SortKey>,
    /// The largest count of rows the view returns.
    pub limit: Option<u32>,
    /// The purposes a read of this view declares.
    pub purposes: Vec<PurposeTag>,
    /// The roles that read the view.
    pub roles: Vec<RoleId>,
    /// One plain sentence stating what the view answers.
    pub summary: String,
}

impl View {
    /// Declare a view over one collection.
    pub fn new(id: &str, source: &str, summary: &str) -> Self {
        View {
            id: ViewId::new(id),
            source: CollectionId::new(source),
            joins: Vec::new(),
            projection: Vec::new(),
            filter: None,
            group_by: Vec::new(),
            aggregates: Vec::new(),
            sort: Vec::new(),
            limit: None,
            purposes: Vec::new(),
            roles: Vec::new(),
            summary: summary.to_string(),
        }
    }

    /// The names the result carries, in order.
    pub fn result_names(&self) -> Vec<&FieldName> {
        self.projection
            .iter()
            .map(|field| &field.name)
            .chain(self.aggregates.iter().map(|aggregate| &aggregate.name))
            .collect()
    }

    /// Build the type context a projection expression reads.
    pub fn context<'a>(
        &self,
        types: &'a TypeRegistry<'a>,
        collections: &CollectionRegistry,
    ) -> TypeContext<'a> {
        let mut ctx = TypeContext::new(types);
        if let Some(collection) = collections.get(&self.source) {
            ctx.bind(
                &ParamName::new(ROW_BINDING),
                TypeExpr::Record(collection.record_type.clone()),
            );
        }
        for join in &self.joins {
            if let Some(collection) = collections.get(&join.collection) {
                let record = TypeExpr::Record(collection.record_type.clone());
                let bound = match join.kind {
                    JoinKind::Matched => record,
                    JoinKind::Optional => TypeExpr::optional(record),
                };
                ctx.bind(&join.alias, bound);
            }
        }
        ctx
    }

    /// The result field types, resolved against the declared types.
    pub fn result_types(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
    ) -> Vec<(FieldName, TypeExpr)> {
        let ctx = self.context(types, collections);
        let path = Path::root().field("views");
        let mut found = Vec::new();
        for field in &self.projection {
            if let Ok(ty) = field.value.check(&ctx, &path) {
                found.push((field.name.clone(), ty));
            }
        }
        for aggregate in &self.aggregates {
            let ty = match (&aggregate.function, &aggregate.value) {
                (AggregateFunction::Count, _) => Some(TypeExpr::Integer),
                (_, Some(value)) => value.check(&ctx, &path).ok(),
                (_, None) => None,
            };
            if let Some(ty) = ty {
                found.push((aggregate.name.clone(), ty));
            }
        }
        found
    }

    /// Check one view.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        report.absorb(self.id.validate(&path.field("id")));

        let source = collections.get(&self.source);
        if source.is_none() {
            report.push(collections.missing(&self.source, path.field("source").location()));
        }
        if self.summary.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                path.field("summary").location(),
                format!("The view `{}` states nothing.", self.id),
                "Write one plain sentence stating what the view answers.".to_string(),
            ));
        }

        let mut aliases: BTreeSet<&str> = BTreeSet::new();
        for (position, join) in self.joins.iter().enumerate() {
            let here = path.field("joins").index(position);
            report.absorb(join.alias.validate(&here.field("alias")));
            if join.alias.as_str() == ROW_BINDING {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("alias").location(),
                    format!("The join takes the name `{ROW_BINDING}`, which holds the source row."),
                    "Give the join another alias.".to_string(),
                ));
            }
            if !aliases.insert(join.alias.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("alias").location(),
                    format!(
                        "The view `{}` uses the alias `{}` twice.",
                        self.id, join.alias
                    ),
                    format!("Rename one of the two `{}` joins.", join.alias),
                ));
            }
            let joined = collections.get(&join.collection);
            if joined.is_none() {
                report.push(
                    collections.missing(&join.collection, here.field("collection").location()),
                );
            }
            report.absorb(join.left.validate(&here.field("left")));
            report.absorb(join.right.validate(&here.field("right")));
            if let (Some(source), Some(joined)) = (source, joined) {
                let left_type = join
                    .left
                    .resolve(&TypeExpr::Record(source.record_type.clone()), types);
                let right_type = join
                    .right
                    .resolve(&TypeExpr::Record(joined.record_type.clone()), types);
                match (left_type, right_type) {
                    (Some(left), Some(right)) => {
                        if left != right {
                            report.push(Diagnostic::at(
                                DiagnosticCode::TypeMismatch,
                                here.location(),
                                format!(
                                    "The join holds {} on the left and {} on the right.",
                                    left.describe(),
                                    right.describe()
                                ),
                                "Join two fields sharing one type.".to_string(),
                            ));
                        }
                    }
                    (None, _) => report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.field("left").location(),
                        format!(
                            "The join names the field `{}`, which `{}` lacks.",
                            join.left, self.source
                        ),
                        format!(
                            "Name a field declared on the record type of `{}`.",
                            self.source
                        ),
                    )),
                    (_, None) => report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.field("right").location(),
                        format!(
                            "The join names the field `{}`, which `{}` lacks.",
                            join.right, join.collection
                        ),
                        format!(
                            "Name a field declared on the record type of `{}`.",
                            join.collection
                        ),
                    )),
                }
            }
        }

        let ctx = self.context(types, collections);
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for (position, field) in self.projection.iter().enumerate() {
            let here = path.field("projection").index(position);
            report.absorb(field.name.validate(&here.field("name")));
            if !names.insert(field.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("name").location(),
                    format!("The view `{}` projects `{}` twice.", self.id, field.name),
                    format!("Rename one of the two `{}` fields.", field.name),
                ));
            }
            if let Err(diagnostic) = field.value.check(&ctx, &here.field("value")) {
                report.push(diagnostic);
            }
        }
        if self.projection.is_empty() && self.aggregates.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                path.field("projection").location(),
                format!("The view `{}` returns zero fields.", self.id),
                "Project at least one field, or compute at least one aggregate.".to_string(),
            ));
        }

        if let Some(filter) = &self.filter {
            let filter_path = path.field("filter");
            match filter.check(&ctx, &filter_path) {
                Ok(TypeExpr::Bool) => {}
                Ok(other) => report.push(Diagnostic::at(
                    DiagnosticCode::TypeMismatch,
                    filter_path.location(),
                    format!("The filter carries {}.", other.describe()),
                    "Give the filter a truth value.".to_string(),
                )),
                Err(diagnostic) => report.push(diagnostic),
            }
        }

        for (position, aggregate) in self.aggregates.iter().enumerate() {
            let here = path.field("aggregates").index(position);
            report.absorb(aggregate.name.validate(&here.field("name")));
            if !names.insert(aggregate.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("name").location(),
                    format!("The view `{}` returns `{}` twice.", self.id, aggregate.name),
                    format!("Rename one of the two `{}` fields.", aggregate.name),
                ));
            }
            match (&aggregate.function, &aggregate.value) {
                (AggregateFunction::Count, None) => {}
                (AggregateFunction::Count, Some(_)) => report.push(Diagnostic::at(
                    DiagnosticCode::TypeMismatch,
                    here.field("value").location(),
                    format!("The count `{}` reads a value.", aggregate.name),
                    "Remove the value from the count.".to_string(),
                )),
                (function, Some(value)) => match value.check(&ctx, &here.field("value")) {
                    Ok(TypeExpr::Integer) | Ok(TypeExpr::Decimal { .. }) => {}
                    Ok(TypeExpr::Timestamp)
                        if matches!(
                            function,
                            AggregateFunction::Smallest | AggregateFunction::Largest
                        ) => {}
                    Ok(other) => report.push(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        here.field("value").location(),
                        format!("The {} reads {}.", function.label(), other.describe()),
                        "Read a whole number or a decimal here.".to_string(),
                    )),
                    Err(diagnostic) => report.push(diagnostic),
                },
                (function, None) => report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("value").location(),
                    format!(
                        "The {} `{}` reads nothing.",
                        function.label(),
                        aggregate.name
                    ),
                    "Name the value the operation reads.".to_string(),
                )),
            }
        }

        for (position, group) in self.group_by.iter().enumerate() {
            let here = path.field("group_by").index(position);
            report.absorb(group.validate(&here));
            if !self.projection.iter().any(|field| &field.name == group) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.location(),
                    format!("The group names `{group}`, which the projection leaves out."),
                    format!("Project a field named `{group}`, or group by a projected field."),
                ));
            }
        }
        if !self.aggregates.is_empty() && self.group_by.is_empty() {
            if let Some(field) = self.projection.first() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    path.field("group_by").location(),
                    format!(
                        "The view `{}` computes an aggregate beside the projected field `{}` and groups by nothing.",
                        self.id, field.name
                    ),
                    format!("Group by `{}`, or remove the projection.", field.name),
                ));
            }
        }

        for (position, key) in self.sort.iter().enumerate() {
            let here = path.field("sort").index(position);
            report.absorb(key.name.validate(&here.field("name")));
            if !self.result_names().iter().any(|name| *name == &key.name) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.field("name").location(),
                    format!(
                        "The sort names `{}`, which the result leaves out.",
                        key.name
                    ),
                    format!("Sort by a projected field, or project `{}`.", key.name),
                ));
            }
        }

        if let Some(0) = self.limit {
            report.push(Diagnostic::at(
                DiagnosticCode::ValueOutOfRange,
                path.field("limit").location(),
                format!("The view `{}` returns zero rows.", self.id),
                "Set a limit above zero, or clear the limit.".to_string(),
            ));
        }

        for (position, role) in self.roles.iter().enumerate() {
            let here = path.field("roles").index(position);
            report.absorb(role.validate(&here));
            if !identities.has_role(role) {
                report.push(identities.missing_role(role, here.location()));
            }
        }

        if let Some(collection) = source {
            if self.purposes.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::SectionEmpty,
                    path.field("purposes").location(),
                    format!("The view `{}` declares zero purposes.", self.id),
                    "Declare the purpose the read serves so the purpose check passes.".to_string(),
                ));
            }
            for (position, purpose) in self.purposes.iter().enumerate() {
                let here = path.field("purposes").index(position);
                report.absorb(purpose.validate(&here));
                if !collection.purpose.admits(purpose) {
                    report.push(Diagnostic::at(
                        DiagnosticCode::DanglingReference,
                        here.location(),
                        format!(
                            "The view declares the purpose `{purpose}`, which `{}` leaves out.",
                            self.source
                        ),
                        format!(
                            "Declare `{purpose}` on the collection `{}`, or name a purpose the collection already declares.",
                            self.source
                        ),
                    ));
                }
            }
        }

        report
    }
}

/// An index over the declared views.
#[derive(Clone, Debug, Default)]
pub struct ViewRegistry<'a> {
    entries: BTreeMap<&'a str, &'a View>,
}

impl<'a> ViewRegistry<'a> {
    /// Build a registry over a slice of views.
    pub fn build(views: &'a [View]) -> Self {
        let mut entries: BTreeMap<&'a str, &'a View> = BTreeMap::new();
        for view in views {
            entries.entry(view.id.as_str()).or_insert(view);
        }
        ViewRegistry { entries }
    }

    /// Look up a view.
    pub fn get(&self, id: &ViewId) -> Option<&'a View> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether a view exists.
    pub fn contains(&self, id: &ViewId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The view names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the views.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero views.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for a view the Blueprint lacks.
    pub fn missing(&self, id: &ViewId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the view `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a view named `{id}` in the views section.")
            } else {
                format!("Point the entry at one of these views: {listed}.")
            },
        )
    }
}

/// Check a whole views section.
pub fn validate_views(
    views: &[View],
    types: &TypeRegistry,
    collections: &CollectionRegistry,
    identities: &Identities,
    path: &Path,
) -> Report {
    let mut report = Report::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (position, view) in views.iter().enumerate() {
        let here = path.index(position);
        if !seen.insert(view.id.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                here.field("id").location(),
                format!("Two views share the name `{}`.", view.id),
                format!("Rename one of the two views away from `{}`.", view.id),
            ));
        }
        report.merge(view.validate(types, collections, identities, &here));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collections::{Collection, PurposeDeclaration},
        expression::BinaryOp,
        identities::RoleDef,
        types::{FieldDef, RecordDef, TypeDef, TypeId},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![
            TypeDef::Record(RecordDef {
                id: TypeId::new("parcel"),
                fields: vec![
                    FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                    FieldDef::new("district", TypeExpr::Text, "The district."),
                    FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
                ],
                summary: "One land parcel.".to_string(),
            }),
            TypeDef::Record(RecordDef {
                id: TypeId::new("valuation"),
                fields: vec![
                    FieldDef::new("survey_number", TypeExpr::Text, "The parcel valued."),
                    FieldDef::new("amount", TypeExpr::Decimal { scale: 2 }, "The valuation."),
                ],
                summary: "One valuation.".to_string(),
            }),
        ]
    }

    fn collections() -> Vec<Collection> {
        let mut parcel =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        parcel.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        let mut valuation =
            Collection::new("valuation", "valuation", "survey_number", "The valuations.");
        valuation.purpose =
            PurposeDeclaration::new("The record carries valuations.", &["registration"]);
        vec![parcel, valuation]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![RoleDef::new(
                "registrar",
                None,
                "Registrar",
                "The registrar reads.",
            )],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn sample() -> View {
        let mut view = View::new(
            "parcels_by_district",
            "parcel",
            "The view totals area by district.",
        );
        view.joins = vec![Join {
            collection: CollectionId::new("valuation"),
            alias: ParamName::new("valued"),
            left: FieldPath::parse("survey_number"),
            right: FieldPath::parse("survey_number"),
            kind: JoinKind::Optional,
        }];
        view.projection = vec![Projection::field("district")];
        view.filter = Some(Expr::binary(
            BinaryOp::Greater,
            Expr::field(Expr::bound(ROW_BINDING), "area"),
            Expr::decimal(0, 4),
        ));
        view.group_by = vec![FieldName::new("district")];
        view.aggregates = vec![
            Aggregate::count("parcel_count"),
            Aggregate::total("total_area", Expr::field(Expr::bound(ROW_BINDING), "area")),
        ];
        view.sort = vec![SortKey::descending("total_area")];
        view.limit = Some(50);
        view.purposes = vec![PurposeTag::new("registration")];
        view.roles = vec![RoleId::new("registrar")];
        view
    }

    fn run(views: &[View]) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        validate_views(
            views,
            &type_registry,
            &collection_registry,
            &identities(),
            &Path::root().field("views"),
        )
    }

    #[test]
    fn a_complete_view_validates() {
        let report = run(&[sample()]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_view_naming_an_absent_source_reports_its_location() {
        let mut view = sample();
        view.source = CollectionId::new("absent_collection");
        let report = run(&[view]);
        let found = report
            .find("views[0].source")
            .expect("the diagnostic sits on the source");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_projection_naming_an_absent_field_reports_its_location() {
        let mut view = sample();
        view.projection = vec![Projection::field("absent_field")];
        view.group_by = vec![FieldName::new("absent_field")];
        view.sort = Vec::new();
        let report = run(&[view]);
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic
                    .location
                    .as_str()
                    .starts_with("views[0].projection[0].value")
            })
            .expect("the diagnostic sits on the projection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_filter_carrying_a_value_other_than_a_truth_value_is_caught() {
        let mut view = sample();
        view.filter = Some(Expr::field(Expr::bound(ROW_BINDING), "area"));
        let report = run(&[view]);
        let found = report
            .find("views[0].filter")
            .expect("the diagnostic sits on the filter");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_join_over_two_different_types_is_caught() {
        let mut view = sample();
        view.joins[0].right = FieldPath::parse("amount");
        let report = run(&[view]);
        let found = report
            .find("views[0].joins[0]")
            .expect("the diagnostic sits on the join");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_sort_naming_a_field_outside_the_result_is_caught() {
        let mut view = sample();
        view.sort = vec![SortKey::ascending("absent_field")];
        let report = run(&[view]);
        let found = report
            .find("views[0].sort[0].name")
            .expect("the diagnostic sits on the sort key");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn a_group_naming_a_field_outside_the_projection_is_caught() {
        let mut view = sample();
        view.group_by = vec![FieldName::new("area")];
        let report = run(&[view]);
        assert!(report.find("views[0].group_by[0]").is_some());
    }

    #[test]
    fn a_count_reading_a_value_is_caught() {
        let mut view = sample();
        view.aggregates[0].value = Some(Expr::field(Expr::bound(ROW_BINDING), "area"));
        let report = run(&[view]);
        let found = report
            .find("views[0].aggregates[0].value")
            .expect("the diagnostic sits on the count");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_total_over_text_is_caught() {
        let mut view = sample();
        view.aggregates[1].value = Some(Expr::field(Expr::bound(ROW_BINDING), "district"));
        let report = run(&[view]);
        let found = report
            .find("views[0].aggregates[1].value")
            .expect("the diagnostic sits on the total");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn the_join_alias_reaches_the_projection() {
        let mut view = sample();
        view.projection.push(Projection::computed(
            "valued_present",
            Expr::IsPresent(alloc::boxed::Box::new(Expr::bound("valued"))),
        ));
        view.group_by.push(FieldName::new("valued_present"));
        let report = run(&[view]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn the_result_types_name_every_field() {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let view = sample();
        let found = view.result_types(&type_registry, &collection_registry);
        assert_eq!(found.len(), 3);
        assert_eq!(found[0], (FieldName::new("district"), TypeExpr::Text));
        assert_eq!(
            found[1],
            (FieldName::new("parcel_count"), TypeExpr::Integer)
        );
        assert_eq!(
            found[2],
            (FieldName::new("total_area"), TypeExpr::Decimal { scale: 4 })
        );
    }

    #[test]
    fn a_purpose_outside_the_collection_declaration_is_caught() {
        let mut view = sample();
        view.purposes = vec![PurposeTag::new("marketing_list")];
        let report = run(&[view]);
        assert!(report.find("views[0].purposes[0]").is_some());
    }

    #[test]
    fn the_registry_reports_a_missing_view_with_the_fix() {
        let views = [sample()];
        let registry = ViewRegistry::build(&views);
        assert!(registry.contains(&ViewId::new("parcels_by_district")));
        let diagnostic = registry.missing(
            &ViewId::new("absent_view"),
            Path::root()
                .field("events")
                .index(0)
                .field("subscribers")
                .index(1)
                .location(),
        );
        assert!(diagnostic.fix.contains("parcels_by_district"));
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let view = sample();
        let bytes = view.encode();
        let back = View::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(view, back);
        let text = serde_json::to_string(&view).expect("the view serialises");
        let parsed: View = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(view, parsed);
    }
}
