//! Schema migration planning and application.
//!
//! A deployment changes shape over time. A user adds a field to a record.
//! A user renames a field. A user adds a secondary index. Cerulea carries
//! each change as a migration plan rather than a code upgrade.
//!
//! A plan names the collection it changes, the version it starts from and
//! the version it produces. The plan holds an ordered list of steps. Three
//! passes run over a plan.
//!
//! 1. The planning pass checks the steps against the registered definition
//!    and reports every problem with the exact location and the fix.
//! 2. The definition pass produces the new record type and the new
//!    collection definition.
//! 3. The record pass rewrites one stored record into the new shape.
//!
//! Every pass is pure. The pallet calls the passes inside one transaction,
//! so a failure at any step leaves the registry untouched.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    collections::{Collection, Index, PurposeDeclaration, Retention},
    expression::{Env, Expr, Meter, TypeContext},
    types::{
        CollectionId, FieldDef, FieldName, FieldPath, IndexName, ParamName, RecordDef, TypeDef,
        TypeExpr, TypeRegistry, Value,
    },
    Diagnostic, DiagnosticCode, Path, Report,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

/// The name the conversion expression of a retype step binds the old field
/// value under.
pub const OLD_VALUE_BINDING: &str = "old_value";

/// The name every conversion expression binds the whole old record under.
pub const OLD_RECORD_BINDING: &str = "old_record";

/// One change inside a migration plan.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo)]
pub enum MigrationStep {
    /// Add one field and give every stored record the declared value.
    AddField {
        /// The new field name.
        field: FieldName,
        /// The type the new field carries.
        ty: TypeExpr,
        /// The value every stored record takes for the new field.
        default: Value,
        /// One sentence stating what the field holds.
        summary: String,
    },
    /// Remove one field from the record type and from every stored record.
    RemoveField {
        /// The field the plan removes.
        field: FieldName,
    },
    /// Rename one field, keeping the value.
    RenameField {
        /// The present name.
        from: FieldName,
        /// The new name.
        to: FieldName,
    },
    /// Change the type of one field and convert every stored value.
    RetypeField {
        /// The field the plan retypes.
        field: FieldName,
        /// The new type.
        ty: TypeExpr,
        /// The conversion. The old field value binds under `old_value` and
        /// the whole old record binds under `old_record`.
        convert: Expr,
    },
    /// Add one secondary index.
    AddIndex {
        /// The index definition.
        index: Index,
    },
    /// Remove one secondary index.
    RemoveIndex {
        /// The index name.
        name: IndexName,
    },
    /// Replace the retention rule of the collection.
    SetRetention {
        /// The new retention rule.
        retention: Retention,
    },
    /// Replace the purpose declaration of the collection.
    SetPurpose {
        /// The new purpose declaration.
        purpose: PurposeDeclaration,
    },
}

impl MigrationStep {
    /// A short label for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            MigrationStep::AddField { .. } => "add field",
            MigrationStep::RemoveField { .. } => "remove field",
            MigrationStep::RenameField { .. } => "rename field",
            MigrationStep::RetypeField { .. } => "retype field",
            MigrationStep::AddIndex { .. } => "add index",
            MigrationStep::RemoveIndex { .. } => "remove index",
            MigrationStep::SetRetention { .. } => "set retention",
            MigrationStep::SetPurpose { .. } => "set purpose",
        }
    }

    /// Report whether the step rewrites stored records.
    pub fn touches_records(&self) -> bool {
        matches!(
            self,
            MigrationStep::AddField { .. }
                | MigrationStep::RemoveField { .. }
                | MigrationStep::RenameField { .. }
                | MigrationStep::RetypeField { .. }
        )
    }

    /// Report whether the step changes the declared indexes.
    pub fn touches_indexes(&self) -> bool {
        matches!(
            self,
            MigrationStep::AddIndex { .. } | MigrationStep::RemoveIndex { .. }
        )
    }
}

/// One migration plan.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo)]
pub struct MigrationPlan {
    /// The collection the plan changes.
    pub collection: CollectionId,
    /// The version the plan starts from.
    pub from_version: u32,
    /// The version the plan produces.
    pub to_version: u32,
    /// The ordered steps.
    pub steps: Vec<MigrationStep>,
    /// The number of blocks during which a revert stays available.
    pub rollback_window: u32,
    /// One sentence stating what the plan changes.
    pub summary: String,
}

impl MigrationPlan {
    /// Start an empty plan raising the version by one.
    pub fn new(collection: &str, from_version: u32, summary: &str) -> Self {
        MigrationPlan {
            collection: CollectionId::new(collection),
            from_version,
            to_version: from_version.saturating_add(1),
            steps: Vec::new(),
            rollback_window: 0,
            summary: summary.to_string(),
        }
    }

    /// Add one step.
    pub fn with(mut self, step: MigrationStep) -> Self {
        self.steps.push(step);
        self
    }

    /// Set the number of blocks a revert stays available for.
    pub fn within(mut self, blocks: u32) -> Self {
        self.rollback_window = blocks;
        self
    }

    /// Report whether any step rewrites stored records.
    pub fn touches_records(&self) -> bool {
        self.steps.iter().any(MigrationStep::touches_records)
    }

    /// Report whether any step changes the declared indexes.
    pub fn touches_indexes(&self) -> bool {
        self.steps.iter().any(MigrationStep::touches_indexes)
    }
}

/// The status of one migration plan.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum MigrationState {
    /// The plan is stored and waits for an apply call.
    Proposed,
    /// The plan changed the registry and the records carry the new shape.
    Applied,
    /// The plan was undone inside its rollback window.
    Reverted,
}

fn problem(code: DiagnosticCode, path: &Path, statement: String, fix: String) -> Diagnostic {
    Diagnostic::at(code, path.location(), statement, fix)
}

/// Check a plan against the registered definitions.
///
/// The report names every problem with the exact step position and the
/// fix. A clean report means the plan applies.
pub fn check_plan(
    plan: &MigrationPlan,
    collection: &Collection,
    version: u32,
    types: &TypeRegistry,
    path: &Path,
) -> Report {
    let mut report = Report::new();

    if plan.collection != collection.id {
        report.push(problem(
            DiagnosticCode::DanglingReference,
            &path.field("collection"),
            format!(
                "The plan names the collection `{}` and the registry holds `{}`.",
                plan.collection, collection.id
            ),
            "Point the plan at the collection the registry holds.".to_string(),
        ));
        return report;
    }

    if plan.from_version != version {
        report.push(problem(
            DiagnosticCode::ValueOutOfRange,
            &path.field("from_version"),
            format!(
                "The plan starts from version {} and the collection sits at version {}.",
                plan.from_version, version
            ),
            format!("Rewrite the plan to start from version {version}."),
        ));
    }

    if plan.to_version <= plan.from_version {
        report.push(problem(
            DiagnosticCode::ValueOutOfRange,
            &path.field("to_version"),
            format!(
                "The plan produces version {} from version {}.",
                plan.to_version, plan.from_version
            ),
            "Give the plan a target version above the version it starts from.".to_string(),
        ));
    }

    if plan.summary.is_empty() {
        report.push(problem(
            DiagnosticCode::MissingValue,
            &path.field("summary"),
            "The plan carries an empty summary.".to_string(),
            "Write one plain sentence stating what the plan changes.".to_string(),
        ));
    }

    if plan.steps.is_empty() {
        report.push(problem(
            DiagnosticCode::SectionEmpty,
            &path.field("steps"),
            "The plan holds zero steps.".to_string(),
            "Add at least one step, or withdraw the plan.".to_string(),
        ));
    }

    let record = match types.record(&collection.record_type) {
        Some(found) => found,
        None => {
            report.push(problem(
                DiagnosticCode::DanglingReference,
                &path.field("collection"),
                format!(
                    "The registry lacks the record type `{}`.",
                    collection.record_type
                ),
                "Register the record type before planning a migration.".to_string(),
            ));
            return report;
        }
    };

    let mut fields: Vec<FieldName> = record.fields.iter().map(|held| held.name.clone()).collect();
    let mut indexes: Vec<IndexName> = collection
        .indexes
        .iter()
        .map(|held| held.name.clone())
        .collect();
    let steps_path = path.field("steps");

    for (position, step) in plan.steps.iter().enumerate() {
        let here = steps_path.index(position);
        match step {
            MigrationStep::AddField {
                field,
                ty,
                default,
                summary,
            } => {
                report.absorb(field.validate(&here.field("field")));
                if fields.contains(field) {
                    report.push(problem(
                        DiagnosticCode::DuplicateName,
                        &here.field("field"),
                        format!("The record already carries the field `{field}`."),
                        format!("Retype `{field}` rather than adding it again."),
                    ));
                }
                report.absorb(ty.validate(types, &here.field("ty")));
                report.absorb(default.check(ty, types, &here.field("default")));
                if summary.is_empty() {
                    report.push(problem(
                        DiagnosticCode::MissingValue,
                        &here.field("summary"),
                        format!("The field `{field}` carries an empty summary."),
                        "Write one plain sentence stating what the field holds.".to_string(),
                    ));
                }
                fields.push(field.clone());
            }
            MigrationStep::RemoveField { field } => {
                if !fields.contains(field) {
                    report.push(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("field"),
                        format!("The record lacks the field `{field}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
                if collection.primary_key.0.first() == Some(field) {
                    report.push(problem(
                        DiagnosticCode::ValueOutOfRange,
                        &here.field("field"),
                        format!("The field `{field}` carries the primary key."),
                        "Keep the primary key field. Create a new collection for a new key."
                            .to_string(),
                    ));
                }
                fields.retain(|held| held != field);
            }
            MigrationStep::RenameField { from, to } => {
                report.absorb(to.validate(&here.field("to")));
                if !fields.contains(from) {
                    report.push(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("from"),
                        format!("The record lacks the field `{from}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
                if fields.contains(to) {
                    report.push(problem(
                        DiagnosticCode::DuplicateName,
                        &here.field("to"),
                        format!("The record already carries the field `{to}`."),
                        format!("Choose a name the record lacks, or remove `{to}` first."),
                    ));
                }
                for held in fields.iter_mut() {
                    if held == from {
                        *held = to.clone();
                    }
                }
            }
            MigrationStep::RetypeField { field, ty, convert } => {
                if !fields.contains(field) {
                    report.push(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("field"),
                        format!("The record lacks the field `{field}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
                report.absorb(ty.validate(types, &here.field("ty")));
                let mut ctx = TypeContext::new(types);
                if let Some(present) = record.field(field) {
                    ctx.bind(&ParamName::new(OLD_VALUE_BINDING), present.ty.clone());
                }
                ctx.bind(
                    &ParamName::new(OLD_RECORD_BINDING),
                    TypeExpr::Record(record.id.clone()),
                );
                match convert.check(&ctx, &here.field("convert")) {
                    Ok(produced) => {
                        if &produced != ty {
                            report.push(problem(
                                DiagnosticCode::TypeMismatch,
                                &here.field("convert"),
                                format!(
                                    "The conversion produces {} and the step declares {}.",
                                    produced.describe(),
                                    ty.describe()
                                ),
                                format!("Rewrite the conversion to produce {}.", ty.describe()),
                            ));
                        }
                    }
                    Err(diagnostic) => report.push(diagnostic),
                }
            }
            MigrationStep::AddIndex { index } => {
                report.absorb(index.name.validate(&here.field("index").field("name")));
                if indexes.contains(&index.name) {
                    report.push(problem(
                        DiagnosticCode::DuplicateName,
                        &here.field("index").field("name"),
                        format!("The collection already carries the index `{}`.", index.name),
                        "Remove the present index before adding a new one under that name."
                            .to_string(),
                    ));
                }
                if index.fields.is_empty() {
                    report.push(problem(
                        DiagnosticCode::SectionEmpty,
                        &here.field("index").field("fields"),
                        format!("The index `{}` names zero fields.", index.name),
                        "Name at least one field for the index to cover.".to_string(),
                    ));
                }
                for (slot, field) in index.fields.iter().enumerate() {
                    let target = here.field("index").field("fields").index(slot);
                    match field.0.first() {
                        Some(head) if fields.contains(head) => {}
                        _ => report.push(problem(
                            DiagnosticCode::DanglingReference,
                            &target,
                            format!("The index names the field `{field}` and the record lacks it."),
                            "Name a field the record carries.".to_string(),
                        )),
                    }
                }
                indexes.push(index.name.clone());
            }
            MigrationStep::RemoveIndex { name } => {
                if !indexes.contains(name) {
                    report.push(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("name"),
                        format!("The collection lacks the index `{name}`."),
                        "Name an index the collection carries.".to_string(),
                    ));
                }
                indexes.retain(|held| held != name);
            }
            MigrationStep::SetRetention { .. } => {}
            MigrationStep::SetPurpose { purpose } => {
                if purpose.statement.is_empty() {
                    report.push(problem(
                        DiagnosticCode::MissingValue,
                        &here.field("purpose").field("statement"),
                        "The purpose declaration carries an empty statement.".to_string(),
                        "Write one plain sentence stating why the collection holds records."
                            .to_string(),
                    ));
                }
                if purpose.purposes.is_empty() {
                    report.push(problem(
                        DiagnosticCode::SectionEmpty,
                        &here.field("purpose").field("purposes"),
                        "The purpose declaration names zero purposes.".to_string(),
                        "Name at least one purpose the collection serves.".to_string(),
                    ));
                }
            }
        }
    }

    report
}

/// Produce the record type the plan yields.
pub fn plan_record_type(
    plan: &MigrationPlan,
    record: &RecordDef,
    path: &Path,
) -> Result<RecordDef, Diagnostic> {
    let mut produced = record.clone();
    let steps_path = path.field("steps");
    for (position, step) in plan.steps.iter().enumerate() {
        let here = steps_path.index(position);
        match step {
            MigrationStep::AddField {
                field, ty, summary, ..
            } => {
                produced.fields.push(FieldDef {
                    name: field.clone(),
                    ty: ty.clone(),
                    summary: summary.clone(),
                });
            }
            MigrationStep::RemoveField { field } => {
                let before = produced.fields.len();
                produced.fields.retain(|held| &held.name != field);
                if produced.fields.len() == before {
                    return Err(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("field"),
                        format!("The record lacks the field `{field}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
            }
            MigrationStep::RenameField { from, to } => {
                let mut found = false;
                for held in produced.fields.iter_mut() {
                    if &held.name == from {
                        held.name = to.clone();
                        found = true;
                    }
                }
                if !found {
                    return Err(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("from"),
                        format!("The record lacks the field `{from}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
            }
            MigrationStep::RetypeField { field, ty, .. } => {
                let mut found = false;
                for held in produced.fields.iter_mut() {
                    if &held.name == field {
                        held.ty = ty.clone();
                        found = true;
                    }
                }
                if !found {
                    return Err(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("field"),
                        format!("The record lacks the field `{field}`."),
                        "Name a field the record carries.".to_string(),
                    ));
                }
            }
            MigrationStep::AddIndex { .. }
            | MigrationStep::RemoveIndex { .. }
            | MigrationStep::SetRetention { .. }
            | MigrationStep::SetPurpose { .. } => {}
        }
    }
    Ok(produced)
}

/// Produce the collection definition the plan yields.
pub fn plan_collection(
    plan: &MigrationPlan,
    collection: &Collection,
    path: &Path,
) -> Result<Collection, Diagnostic> {
    let mut produced = collection.clone();
    let steps_path = path.field("steps");
    for (position, step) in plan.steps.iter().enumerate() {
        let here = steps_path.index(position);
        match step {
            MigrationStep::AddIndex { index } => {
                if produced.indexes.iter().any(|held| held.name == index.name) {
                    return Err(problem(
                        DiagnosticCode::DuplicateName,
                        &here.field("index").field("name"),
                        format!("The collection already carries the index `{}`.", index.name),
                        "Remove the present index first.".to_string(),
                    ));
                }
                produced.indexes.push(index.clone());
            }
            MigrationStep::RemoveIndex { name } => {
                let before = produced.indexes.len();
                produced.indexes.retain(|held| &held.name != name);
                if produced.indexes.len() == before {
                    return Err(problem(
                        DiagnosticCode::DanglingReference,
                        &here.field("name"),
                        format!("The collection lacks the index `{name}`."),
                        "Name an index the collection carries.".to_string(),
                    ));
                }
            }
            MigrationStep::SetRetention { retention } => {
                produced.retention = retention.clone();
            }
            MigrationStep::SetPurpose { purpose } => {
                produced.purpose = purpose.clone();
            }
            MigrationStep::RenameField { from, to } => {
                rename_in_path(&mut produced.primary_key, from, to);
                for index in produced.indexes.iter_mut() {
                    for field in index.fields.iter_mut() {
                        rename_in_path(field, from, to);
                    }
                }
            }
            MigrationStep::AddField { .. }
            | MigrationStep::RemoveField { .. }
            | MigrationStep::RetypeField { .. } => {}
        }
    }
    Ok(produced)
}

fn rename_in_path(path: &mut FieldPath, from: &FieldName, to: &FieldName) {
    if path.0.first() == Some(from) {
        path.0[0] = to.clone();
    }
}

/// Rewrite one stored record into the shape the plan yields.
///
/// The record keeps every field the plan leaves alone. A conversion
/// expression reads the old field value under `old_value` and the whole
/// old record under `old_record`.
pub fn migrate_record(
    plan: &MigrationPlan,
    record: &Value,
    env: &Env,
    meter: &mut Meter,
    path: &Path,
) -> Result<Value, Diagnostic> {
    let mut held = match record {
        Value::Record(inner) => inner.clone(),
        other => {
            return Err(problem(
                DiagnosticCode::TypeMismatch,
                path,
                format!(
                    "The stored value carries {}.",
                    other.infer_type().describe()
                ),
                "Migrate a record value. A collection stores record values.".to_string(),
            ))
        }
    };
    let original = Value::Record(held.clone());
    let record_binding = ParamName::new(OLD_RECORD_BINDING);
    let value_binding = ParamName::new(OLD_VALUE_BINDING);
    let steps_path = path.field("steps");

    for (position, step) in plan.steps.iter().enumerate() {
        let here = steps_path.index(position);
        match step {
            MigrationStep::AddField { field, default, .. } => {
                held.fields.push((field.clone(), default.clone()));
            }
            MigrationStep::RemoveField { field } => {
                held.fields.retain(|(name, _)| name != field);
            }
            MigrationStep::RenameField { from, to } => {
                for (name, _) in held.fields.iter_mut() {
                    if name == from {
                        *name = to.clone();
                    }
                }
            }
            MigrationStep::RetypeField { field, convert, .. } => {
                let present = held
                    .fields
                    .iter()
                    .find(|(name, _)| name == field)
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| {
                        problem(
                            DiagnosticCode::DanglingReference,
                            &here.field("field"),
                            format!("The stored record lacks the field `{field}`."),
                            "Apply the plan to records carrying the declared shape.".to_string(),
                        )
                    })?;
                let bound = env
                    .with(&record_binding, original.clone())
                    .with(&value_binding, present);
                let produced = convert.evaluate(&bound, meter, &here.field("convert"))?;
                for (name, value) in held.fields.iter_mut() {
                    if name == field {
                        *value = produced.clone();
                    }
                }
            }
            MigrationStep::AddIndex { .. }
            | MigrationStep::RemoveIndex { .. }
            | MigrationStep::SetRetention { .. }
            | MigrationStep::SetPurpose { .. } => {}
        }
    }

    let mut produced = Value::Record(held);
    produced.canonicalise();
    Ok(produced)
}

/// The version a revert restores.
pub fn revert_target(plan: &MigrationPlan) -> u32 {
    plan.from_version
}

/// Produce the plan that undoes a plan.
///
/// A revert walks the steps in reverse order and inverts each one. Four
/// steps invert on their own. A rename inverts by swapping the names. An
/// added field inverts by removing the field. An added index inverts by
/// removing the index. A retention change and a purpose change invert
/// from the stored earlier collection.
///
/// A removed field and a retyped field lose the earlier values, so a plan
/// carrying either step reports that it carries no reverse. The fix names
/// a forward plan or an export.
pub fn inverse_plan(
    plan: &MigrationPlan,
    previous: &Collection,
    path: &Path,
) -> Result<MigrationPlan, Diagnostic> {
    let mut inverse = MigrationPlan {
        collection: plan.collection.clone(),
        from_version: plan.to_version,
        to_version: plan.from_version,
        steps: Vec::new(),
        rollback_window: plan.rollback_window,
        summary: format!("The revert restores version {}.", plan.from_version),
    };
    let steps_path = path.field("steps");

    for (position, step) in plan.steps.iter().enumerate().rev() {
        let here = steps_path.index(position);
        match step {
            MigrationStep::AddField { field, .. } => {
                inverse.steps.push(MigrationStep::RemoveField {
                    field: field.clone(),
                });
            }
            MigrationStep::RenameField { from, to } => {
                inverse.steps.push(MigrationStep::RenameField {
                    from: to.clone(),
                    to: from.clone(),
                });
            }
            MigrationStep::AddIndex { index } => {
                inverse.steps.push(MigrationStep::RemoveIndex {
                    name: index.name.clone(),
                });
            }
            MigrationStep::RemoveIndex { name } => {
                let restored = previous
                    .indexes
                    .iter()
                    .find(|held| &held.name == name)
                    .cloned()
                    .ok_or_else(|| {
                        problem(
                            DiagnosticCode::DanglingReference,
                            &here.field("name"),
                            format!("The earlier collection lacks the index `{name}`."),
                            "Propose a forward plan adding the index back.".to_string(),
                        )
                    })?;
                inverse
                    .steps
                    .push(MigrationStep::AddIndex { index: restored });
            }
            MigrationStep::SetRetention { .. } => {
                inverse.steps.push(MigrationStep::SetRetention {
                    retention: previous.retention.clone(),
                });
            }
            MigrationStep::SetPurpose { .. } => {
                inverse.steps.push(MigrationStep::SetPurpose {
                    purpose: previous.purpose.clone(),
                });
            }
            MigrationStep::RemoveField { field } => {
                return Err(problem(
                    DiagnosticCode::ValueOutOfRange,
                    &here.field("field"),
                    format!("The plan removed the field `{field}` and the values went with it."),
                    "Propose a forward plan adding the field back, or restore from an export."
                        .to_string(),
                ))
            }
            MigrationStep::RetypeField { field, .. } => {
                return Err(problem(
                    DiagnosticCode::ValueOutOfRange,
                    &here.field("field"),
                    format!(
                        "The plan retyped the field `{field}` and the earlier values went with it."
                    ),
                    "Propose a forward plan converting the field back, or restore from an export."
                        .to_string(),
                ))
            }
        }
    }

    Ok(inverse)
}

/// Report whether a revert is still available.
///
/// The window counts blocks from the apply call. A window of zero keeps a
/// revert available for the life of the deployment.
pub fn revert_available(applied_at: u32, now: u32, window: u32) -> bool {
    if window == 0 {
        return true;
    }
    now.saturating_sub(applied_at) <= window
}

/// The type definitions of a deployment with one record type replaced.
///
/// The caller holds the result alive while it builds a registry over it.
pub fn with_replaced_record(defs: &[TypeDef], replacement: RecordDef) -> Vec<TypeDef> {
    let mut produced: Vec<TypeDef> = defs
        .iter()
        .filter(|held| held.id() != &replacement.id)
        .cloned()
        .collect();
    produced.push(TypeDef::Record(replacement));
    produced.sort_by(|left, right| left.id().cmp(right.id()));
    produced
}

/// The outcome of a planning pass, ready for the interface layer.
///
/// The Studio shows a user the definitions a plan produces before the
/// user commits the plan. The preview carries those definitions together
/// with the two facts that drive the cost estimate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MigrationPreview {
    /// The record type the plan produces.
    pub produced_type: RecordDef,
    /// The collection definition the plan produces.
    pub produced_collection: Collection,
    /// The plan rewrites every stored record.
    pub rewrites_records: bool,
    /// The plan rebuilds the secondary indexes.
    pub rebuilds_indexes: bool,
    /// The plan carries a reverse.
    pub reversible: bool,
}

/// Run the planning pass and produce the preview.
///
/// A clean plan yields the definitions it produces. A plan carrying a
/// problem yields the report instead, with every diagnostic naming its
/// location and its fix.
pub fn preview(
    plan: &MigrationPlan,
    collection: &Collection,
    version: u32,
    types: &TypeRegistry,
    path: &Path,
) -> Result<MigrationPreview, Report> {
    let report = check_plan(plan, collection, version, types, path);
    if !report.is_clean() {
        return Err(report);
    }

    let present = match types.record(&collection.record_type) {
        Some(found) => found,
        None => {
            let mut missing = Report::new();
            missing.push(problem(
                DiagnosticCode::DanglingReference,
                &path.field("collection"),
                format!(
                    "The registry lacks the record type `{}`.",
                    collection.record_type
                ),
                "Register the record type before planning a migration.".to_string(),
            ));
            return Err(missing);
        }
    };

    let produced_type = plan_record_type(plan, present, path).map_err(|diagnostic| {
        let mut held = Report::new();
        held.push(diagnostic);
        held
    })?;
    let produced_collection = plan_collection(plan, collection, path).map_err(|diagnostic| {
        let mut held = Report::new();
        held.push(diagnostic);
        held
    })?;

    Ok(MigrationPreview {
        produced_type,
        produced_collection,
        rewrites_records: plan.touches_records(),
        rebuilds_indexes: plan.touches_indexes() || plan.touches_records(),
        reversible: inverse_plan(plan, collection, path).is_ok(),
    })
}
