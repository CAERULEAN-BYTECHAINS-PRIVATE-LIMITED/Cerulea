//! Actions and the Access Manifest.
//!
//! An action carries parameters, preconditions, an effect sequence, the
//! events it emits, its transaction class and the roles that run it.
//!
//! The effect sequence is what the Execution Engine runs. The collections
//! and the key expressions inside the effects are what Access Manifest
//! derivation reads. Every Cerulea transaction declares the state it reads
//! and the state it writes before execution begins. Cerulea derives that
//! declaration from the Blueprint. A person writes none of it.

use crate::{
    collections::{CollectionRegistry, KeyRange, KeySelector},
    expression::{Env, Expr, Meter, TypeContext},
    fold::FoldSource,
    identities::Identities,
    rules::{RuleRef, Rules},
    types::{
        ActionId, BlockId, CollectionId, EventId, FieldPath, Identity, ParamName, RoleId, StateId,
        Timestamp, TypeExpr, TypeRegistry, Value, WorkflowId,
    },
    Diagnostic, DiagnosticCode, Location, Path, Report,
};
use alloc::{
    boxed::Box,
    collections::{BTreeMap, BTreeSet},
    format,
    string::{String, ToString},
    vec::Vec,
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use serde::{Deserialize, Serialize};

/// The deepest run of nested blocks manifest derivation walks.
pub const MAX_BLOCK_DEPTH: u32 = 16;

/// The class of a transaction.
///
/// The class sets the agreement threshold the Assurance Engine applies.
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
pub enum TransactionClass {
    /// Ordinary work. The threshold is 51 percent.
    #[default]
    Routine,
    /// Work moving value. The threshold is 67 percent.
    Financial,
    /// Work changing the shape of the deployment. The threshold is 80
    /// percent.
    Structural,
    /// Work changing the rules of the deployment. The threshold is 90
    /// percent and a delay applies.
    Constitutional,
}

impl TransactionClass {
    /// The share of the validator set that agrees before the work lands.
    pub fn threshold_percent(&self) -> u8 {
        match self {
            TransactionClass::Routine => 51,
            TransactionClass::Financial => 67,
            TransactionClass::Structural => 80,
            TransactionClass::Constitutional => 90,
        }
    }

    /// Report whether a mandatory delay applies before the work lands.
    pub fn holds_delay(&self) -> bool {
        matches!(self, TransactionClass::Constitutional)
    }

    /// A word naming the class for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            TransactionClass::Routine => "routine",
            TransactionClass::Financial => "financial",
            TransactionClass::Structural => "structural",
            TransactionClass::Constitutional => "constitutional",
        }
    }
}

/// One parameter of an action or a reusable block.
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
pub struct Param {
    /// The name the parameter binds.
    pub name: ParamName,
    /// The type the parameter carries.
    pub ty: TypeExpr,
    /// One plain sentence stating what the parameter holds.
    pub summary: String,
}

impl Param {
    /// Declare a parameter.
    pub fn new(name: &str, ty: TypeExpr, summary: &str) -> Self {
        Param {
            name: ParamName::new(name),
            ty,
            summary: summary.to_string(),
        }
    }
}

/// The direction of a state access.
#[derive(
    Clone,
    Copy,
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
pub enum AccessMode {
    /// The access reads records.
    Read,
    /// The access changes records.
    Write,
}

/// One step of an effect sequence.
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
pub enum Effect {
    /// Add one record.
    Insert {
        /// The collection the record lands in.
        collection: CollectionId,
        /// The key the record takes.
        key: KeySelector,
        /// The record.
        record: Expr,
    },
    /// Change one field of the records the key reaches.
    Update {
        /// The collection holding the records.
        collection: CollectionId,
        /// The keys the change reaches.
        key: KeySelector,
        /// The field the change writes.
        field: FieldPath,
        /// The value the field takes.
        value: Expr,
    },
    /// Remove the records the key reaches.
    Delete {
        /// The collection holding the records.
        collection: CollectionId,
        /// The keys the removal reaches.
        key: KeySelector,
    },
    /// Read one record and bind it to a name for the later effects.
    Read {
        /// The collection holding the record.
        collection: CollectionId,
        /// The key of the record.
        key: KeySelector,
        /// The name the record binds to.
        binding: ParamName,
        /// The record type the collection holds.
        element: TypeExpr,
    },
    /// Write one event to the audit log.
    Emit {
        /// The event.
        event: EventId,
        /// The payload the event carries.
        payload: Expr,
    },
    /// Move one record to another workflow state.
    Transition {
        /// The workflow.
        workflow: WorkflowId,
        /// The collection holding the record.
        collection: CollectionId,
        /// The key of the record.
        key: KeySelector,
        /// The state the record takes.
        to: StateId,
    },
    /// Run a reusable block.
    RunBlock {
        /// The block.
        block: BlockId,
        /// The values the block inputs take.
        arguments: Vec<(ParamName, Expr)>,
    },
}

impl Effect {
    /// The collection and the key span this effect touches.
    pub fn access(&self) -> Option<(&CollectionId, &KeySelector, AccessMode)> {
        match self {
            Effect::Insert {
                collection, key, ..
            } => Some((collection, key, AccessMode::Write)),
            Effect::Update {
                collection, key, ..
            } => Some((collection, key, AccessMode::Write)),
            Effect::Delete { collection, key } => Some((collection, key, AccessMode::Write)),
            Effect::Read {
                collection, key, ..
            } => Some((collection, key, AccessMode::Read)),
            Effect::Transition {
                collection, key, ..
            } => Some((collection, key, AccessMode::Write)),
            Effect::Emit { .. } | Effect::RunBlock { .. } => None,
        }
    }

    /// A word naming the step for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            Effect::Insert { .. } => "insert",
            Effect::Update { .. } => "update",
            Effect::Delete { .. } => "delete",
            Effect::Read { .. } => "read",
            Effect::Emit { .. } => "emit",
            Effect::Transition { .. } => "transition",
            Effect::RunBlock { .. } => "run block",
        }
    }

    /// Visit every expression inside this effect.
    pub fn walk_expressions(&self, visit: &mut dyn FnMut(&Expr)) {
        if let Some((_, key, _)) = self.access() {
            if let Some(expr) = key.expression() {
                expr.walk(visit);
            }
        }
        match self {
            Effect::Insert { record, .. } => record.walk(visit),
            Effect::Update { value, .. } => value.walk(visit),
            Effect::Emit { payload, .. } => payload.walk(visit),
            Effect::RunBlock { arguments, .. } => {
                for (_, value) in arguments {
                    value.walk(visit);
                }
            }
            Effect::Delete { .. } | Effect::Read { .. } | Effect::Transition { .. } => {}
        }
    }

    /// Replace every read of a name with a supplied expression.
    pub fn substitute(&self, replacements: &BTreeMap<ParamName, Expr>) -> Effect {
        let key_of = |key: &KeySelector| match key {
            KeySelector::Exact(expr) => KeySelector::Exact(expr.substitute(replacements)),
            KeySelector::Prefix(expr) => KeySelector::Prefix(expr.substitute(replacements)),
            KeySelector::Full => KeySelector::Full,
        };
        match self {
            Effect::Insert {
                collection,
                key,
                record,
            } => Effect::Insert {
                collection: collection.clone(),
                key: key_of(key),
                record: record.substitute(replacements),
            },
            Effect::Update {
                collection,
                key,
                field,
                value,
            } => Effect::Update {
                collection: collection.clone(),
                key: key_of(key),
                field: field.clone(),
                value: value.substitute(replacements),
            },
            Effect::Delete { collection, key } => Effect::Delete {
                collection: collection.clone(),
                key: key_of(key),
            },
            Effect::Read {
                collection,
                key,
                binding,
                element,
            } => Effect::Read {
                collection: collection.clone(),
                key: key_of(key),
                binding: binding.clone(),
                element: element.clone(),
            },
            Effect::Emit { event, payload } => Effect::Emit {
                event: event.clone(),
                payload: payload.substitute(replacements),
            },
            Effect::Transition {
                workflow,
                collection,
                key,
                to,
            } => Effect::Transition {
                workflow: workflow.clone(),
                collection: collection.clone(),
                key: key_of(key),
                to: to.clone(),
            },
            Effect::RunBlock { block, arguments } => Effect::RunBlock {
                block: block.clone(),
                arguments: arguments
                    .iter()
                    .map(|(name, value)| (name.clone(), value.substitute(replacements)))
                    .collect(),
            },
        }
    }
}

/// The body of a reusable block, as manifest derivation reads it.
#[derive(Clone, Copy, Debug)]
pub struct BlockBody<'a> {
    /// The typed inputs the block takes.
    pub inputs: &'a [Param],
    /// The effect sequence the block runs.
    pub effects: &'a [Effect],
}

/// A library of reusable blocks.
///
/// Manifest derivation walks into a block the same way it walks an action.
pub trait BlockSource {
    /// Look up a block.
    fn block(&self, id: &BlockId) -> Option<BlockBody<'_>>;
}

/// A library holding zero blocks.
///
/// An action that runs no block derives its manifest against this library.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoBlocks;

impl BlockSource for NoBlocks {
    fn block(&self, _id: &BlockId) -> Option<BlockBody<'_>> {
        None
    }
}

/// One collection and one key span inside an Access Manifest.
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
pub struct AccessEntry {
    /// The collection.
    pub collection: CollectionId,
    /// The key span inside the collection.
    pub range: KeyRange,
}

/// The state one transaction declares before execution begins.
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
pub struct AccessManifest {
    /// The action the manifest belongs to.
    pub action: ActionId,
    /// The state the transaction reads.
    pub reads: Vec<AccessEntry>,
    /// The state the transaction changes.
    pub writes: Vec<AccessEntry>,
    /// The class of the transaction.
    pub class: TransactionClass,
}

impl AccessManifest {
    /// Report whether the manifest admits a read of the given span.
    pub fn admits_read(&self, collection: &CollectionId, range: &KeyRange) -> bool {
        self.reads
            .iter()
            .chain(self.writes.iter())
            .any(|entry| &entry.collection == collection && entry.range.covers(range))
    }

    /// Report whether the manifest admits a change to the given span.
    pub fn admits_write(&self, collection: &CollectionId, range: &KeyRange) -> bool {
        self.writes
            .iter()
            .any(|entry| &entry.collection == collection && entry.range.covers(range))
    }

    /// The collections the manifest names, in sorted order.
    pub fn collections(&self) -> Vec<CollectionId> {
        let mut found: Vec<CollectionId> = self
            .reads
            .iter()
            .chain(self.writes.iter())
            .map(|entry| entry.collection.clone())
            .collect();
        found.sort();
        found.dedup();
        found
    }
}

/// An action.
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
pub struct Action {
    /// The name of the action.
    pub id: ActionId,
    /// The parameters a caller supplies.
    pub params: Vec<Param>,
    /// The preconditions gating the action.
    pub preconditions: Vec<RuleRef>,
    /// The effect sequence the Execution Engine runs.
    pub effects: Vec<Effect>,
    /// The effect sequence that runs when an effect fails.
    pub compensation: Vec<Effect>,
    /// The events the action emits.
    pub emits: Vec<EventId>,
    /// The class of the transaction.
    pub class: TransactionClass,
    /// The roles that run the action.
    pub roles: Vec<RoleId>,
    /// One plain sentence stating what the action does.
    pub summary: String,
}

impl Action {
    /// Declare a routine action.
    pub fn new(id: &str, summary: &str) -> Self {
        Action {
            id: ActionId::new(id),
            params: Vec::new(),
            preconditions: Vec::new(),
            effects: Vec::new(),
            compensation: Vec::new(),
            emits: Vec::new(),
            class: TransactionClass::Routine,
            roles: Vec::new(),
            summary: summary.to_string(),
        }
    }

    /// Look up one parameter.
    pub fn param(&self, name: &ParamName) -> Option<&Param> {
        self.params.iter().find(|param| &param.name == name)
    }

    /// The events this action names, each with its location.
    pub fn event_refs(&self, path: &Path) -> Vec<(EventId, Location)> {
        let mut found = Vec::new();
        for (position, event) in self.emits.iter().enumerate() {
            found.push((
                event.clone(),
                path.field("emits").index(position).location(),
            ));
        }
        for (position, effect) in self.effects.iter().enumerate() {
            if let Effect::Emit { event, .. } = effect {
                found.push((
                    event.clone(),
                    path.field("effects")
                        .index(position)
                        .field("event")
                        .location(),
                ));
            }
        }
        found
    }

    /// The workflow states this action moves records into, each with its
    /// location.
    pub fn transition_refs(&self, path: &Path) -> Vec<(WorkflowId, StateId, Location)> {
        let mut found = Vec::new();
        for (position, effect) in self.effects.iter().enumerate() {
            if let Effect::Transition { workflow, to, .. } = effect {
                found.push((
                    workflow.clone(),
                    to.clone(),
                    path.field("effects")
                        .index(position)
                        .field("workflow")
                        .location(),
                ));
            }
        }
        found
    }

    /// The blocks this action runs, each with its location.
    pub fn block_refs(&self, path: &Path) -> Vec<(BlockId, Location)> {
        let mut found = Vec::new();
        for (position, effect) in self
            .effects
            .iter()
            .chain(self.compensation.iter())
            .enumerate()
        {
            if let Effect::RunBlock { block, .. } = effect {
                found.push((
                    block.clone(),
                    path.field("effects")
                        .index(position)
                        .field("block")
                        .location(),
                ));
            }
        }
        found
    }

    /// Derive the Access Manifest for one call of this action.
    ///
    /// The derivation evaluates every key expression against the supplied
    /// parameters. The result names the collections read, the collections
    /// changed, the key span inside each one and the transaction class.
    pub fn derive_manifest(
        &self,
        arguments: &BTreeMap<ParamName, Value>,
        caller: Identity,
        now: Timestamp,
        blocks: &dyn BlockSource,
        path: &Path,
    ) -> Result<AccessManifest, Diagnostic> {
        let mut env = Env::new(caller, now);
        for param in &self.params {
            let value = arguments.get(&param.name).ok_or_else(|| {
                Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    path.field("params").location(),
                    format!(
                        "The call to `{}` leaves out the parameter `{}`.",
                        self.id, param.name
                    ),
                    format!("Supply a value for `{}`.", param.name),
                )
            })?;
            env.bind(&param.name, value.clone());
        }

        let mut meter = Meter::default();
        let mut reads: Vec<AccessEntry> = Vec::new();
        let mut writes: Vec<AccessEntry> = Vec::new();

        collect_access(
            &self.effects,
            &env,
            &mut meter,
            blocks,
            0,
            &path.field("effects"),
            &mut reads,
            &mut writes,
        )?;
        collect_access(
            &self.compensation,
            &env,
            &mut meter,
            blocks,
            0,
            &path.field("compensation"),
            &mut reads,
            &mut writes,
        )?;

        reads.sort();
        reads.dedup();
        writes.sort();
        writes.dedup();

        Ok(AccessManifest {
            action: self.id.clone(),
            reads,
            writes,
            class: self.class,
        })
    }

    /// Check one action.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        rules: &Rules,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        report.absorb(self.id.validate(&path.field("id")));

        if self.summary.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::MissingValue,
                path.field("summary").location(),
                format!("The action `{}` states nothing.", self.id),
                "Write one plain sentence stating what the action does.".to_string(),
            ));
        }

        let mut ctx = TypeContext::new(types);
        let mut names: BTreeSet<&str> = BTreeSet::new();
        for (position, param) in self.params.iter().enumerate() {
            let here = path.field("params").index(position);
            report.absorb(param.name.validate(&here.field("name")));
            report.absorb(param.ty.validate(types, &here.field("ty")));
            if !names.insert(param.name.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateName,
                    here.field("name").location(),
                    format!(
                        "The action `{}` declares the parameter `{}` twice.",
                        self.id, param.name
                    ),
                    format!("Rename one of the two `{}` parameters.", param.name),
                ));
            }
            ctx.bind(&param.name, param.ty.clone());
        }

        if self.roles.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                path.field("roles").location(),
                format!("The action `{}` names zero roles.", self.id),
                "Name at least one role that runs the action.".to_string(),
            ));
        }
        for (position, role) in self.roles.iter().enumerate() {
            let here = path.field("roles").index(position);
            report.absorb(role.validate(&here));
            if !identities.has_role(role) {
                report.push(identities.missing_role(role, here.location()));
            }
        }

        for (position, rule) in self.preconditions.iter().enumerate() {
            let here = path.field("preconditions").index(position);
            match rules.precondition(rule) {
                Some(found) => {
                    for input in &found.inputs {
                        match self.param(&input.name) {
                            Some(param) =>
                                if param.ty != input.ty {
                                    report.push(Diagnostic::at(
                                        DiagnosticCode::TypeMismatch,
                                        here.location(),
                                        format!(
                                            "The rule `{rule}` reads `{}` as {} and the action declares {}.",
                                            input.name,
                                            input.ty.describe(),
                                            param.ty.describe()
                                        ),
                                        format!(
                                            "Declare the parameter `{}` as {}.",
                                            input.name,
                                            input.ty.describe()
                                        ),
                                    ));
                                },
                            None => report.push(Diagnostic::at(
                                DiagnosticCode::MissingValue,
                                here.location(),
                                format!(
                                    "The rule `{rule}` reads `{}` and the action declares no such parameter.",
                                    input.name
                                ),
                                format!(
                                    "Declare a parameter named `{}` carrying {}.",
                                    input.name,
                                    input.ty.describe()
                                ),
                            )),
                        }
                    }
                }
                None => report.push(rules.missing_precondition(rule, here.location())),
            }
        }

        let mut effect_ctx = ctx.clone();
        for (position, effect) in self.effects.iter().enumerate() {
            check_effect(
                effect,
                &mut effect_ctx,
                types,
                collections,
                &path.field("effects").index(position),
                &mut report,
            );
        }
        let mut compensation_ctx = ctx;
        for (position, effect) in self.compensation.iter().enumerate() {
            check_effect(
                effect,
                &mut compensation_ctx,
                types,
                collections,
                &path.field("compensation").index(position),
                &mut report,
            );
        }

        report
    }
}

#[allow(clippy::too_many_arguments)]
fn collect_access(
    effects: &[Effect],
    env: &Env,
    meter: &mut Meter,
    blocks: &dyn BlockSource,
    depth: u32,
    path: &Path,
    reads: &mut Vec<AccessEntry>,
    writes: &mut Vec<AccessEntry>,
) -> Result<(), Diagnostic> {
    if depth > MAX_BLOCK_DEPTH {
        return Err(Diagnostic::at(
            DiagnosticCode::BoundExceeded,
            path.location(),
            format!("The blocks nest deeper than {MAX_BLOCK_DEPTH} levels."),
            "Flatten one of the blocks so the nesting stays inside the limit.".to_string(),
        ));
    }

    for (position, effect) in effects.iter().enumerate() {
        let here = path.index(position);

        if let Some((collection, key, mode)) = effect.access() {
            let range = key.resolve(env, meter, &here.field("key"))?;
            let entry = AccessEntry {
                collection: collection.clone(),
                range,
            };
            match mode {
                AccessMode::Read => reads.push(entry),
                AccessMode::Write => writes.push(entry),
            }
        }

        let mut folds: Vec<(CollectionId, KeySelector)> = Vec::new();
        effect.walk_expressions(&mut |expr| {
            if let Expr::Fold(fold) = expr {
                if let FoldSource::Collection {
                    collection, keys, ..
                } = &fold.source
                {
                    folds.push((collection.clone(), keys.clone()));
                }
            }
        });
        for (collection, keys) in folds {
            let range = keys.resolve(env, meter, &here.field("fold"))?;
            reads.push(AccessEntry { collection, range });
        }

        if let Effect::RunBlock { block, arguments } = effect {
            let body = blocks.block(block).ok_or_else(|| {
                Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    here.field("block").location(),
                    format!("The effect runs the block `{block}`, which the Blueprint lacks."),
                    format!("Declare a block named `{block}` in the composition library."),
                )
            })?;
            let mut replacements: BTreeMap<ParamName, Expr> = BTreeMap::new();
            for input in body.inputs {
                let supplied = arguments
                    .iter()
                    .find(|(name, _)| name == &input.name)
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| {
                        Diagnostic::at(
                            DiagnosticCode::MissingValue,
                            here.field("arguments").location(),
                            format!(
                                "The call to `{block}` leaves out the input `{}`.",
                                input.name
                            ),
                            format!("Supply a value for `{}`.", input.name),
                        )
                    })?;
                replacements.insert(input.name.clone(), supplied);
            }
            let inlined: Vec<Effect> = body
                .effects
                .iter()
                .map(|inner| inner.substitute(&replacements))
                .collect();
            collect_access(
                &inlined,
                env,
                meter,
                blocks,
                depth + 1,
                &here.field("block"),
                reads,
                writes,
            )?;
        }
    }
    Ok(())
}

fn check_effect(
    effect: &Effect,
    ctx: &mut TypeContext,
    types: &TypeRegistry,
    collections: &CollectionRegistry,
    path: &Path,
    report: &mut Report,
) {
    if let Some((collection_id, key, _)) = effect.access() {
        match collections.get(collection_id) {
            Some(collection) => {
                if let Some(expr) = key.expression() {
                    let key_path = path.field("key");
                    match expr.check(ctx, &key_path) {
                        Ok(found) => match key {
                            KeySelector::Exact(_) => {
                                if let Some(declared) = collection.key_type(types) {
                                    if found != declared {
                                        report.push(Diagnostic::at(
                                            DiagnosticCode::TypeMismatch,
                                            key_path.location(),
                                            format!(
                                                "The key carries {} and `{collection_id}` keys records by {}.",
                                                found.describe(),
                                                declared.describe()
                                            ),
                                            format!("Supply {} as the key.", declared.describe()),
                                        ));
                                    }
                                }
                            }
                            _ => {
                                if !found.is_key_safe() {
                                    report.push(Diagnostic::at(
                                        DiagnosticCode::TypeMismatch,
                                        key_path.location(),
                                        format!(
                                            "The key prefix carries {}, which lacks a total order.",
                                            found.describe()
                                        ),
                                        "Supply a key prefix carrying text, a whole number, a timestamp, an identity or a hash."
                                            .to_string(),
                                    ));
                                }
                            }
                        },
                        Err(diagnostic) => report.push(diagnostic),
                    }
                }
            }
            None => {
                report.push(collections.missing(collection_id, path.field("collection").location()))
            }
        }
    }

    match effect {
        Effect::Insert {
            collection, record, ..
        } => {
            if let Some(found) = collections.get(collection) {
                let declared = TypeExpr::Record(found.record_type.clone());
                match record.check(ctx, &path.field("record")) {
                    Ok(carried) => {
                        if carried != declared {
                            report.push(Diagnostic::at(
                                DiagnosticCode::TypeMismatch,
                                path.field("record").location(),
                                format!(
                                    "The insert carries {} and `{collection}` holds {}.",
                                    carried.describe(),
                                    declared.describe()
                                ),
                                format!("Build {} for the insert.", declared.describe()),
                            ));
                        }
                    }
                    Err(diagnostic) => report.push(diagnostic),
                }
            }
        }
        Effect::Update {
            collection,
            field,
            value,
            ..
        } => {
            if let Some(found) = collections.get(collection) {
                let record_type = TypeExpr::Record(found.record_type.clone());
                report.absorb(field.validate(&path.field("field")));
                match field.resolve(&record_type, types) {
                    Some(declared) => match value.check(ctx, &path.field("value")) {
                        Ok(carried) => {
                            if carried != declared {
                                report.push(Diagnostic::at(
                                    DiagnosticCode::TypeMismatch,
                                    path.field("value").location(),
                                    format!(
                                        "The change carries {} and the field `{field}` holds {}.",
                                        carried.describe(),
                                        declared.describe()
                                    ),
                                    format!("Supply {} here.", declared.describe()),
                                ));
                            }
                        }
                        Err(diagnostic) => report.push(diagnostic),
                    },
                    None => {
                        if types.record(&found.record_type).is_some() {
                            report.push(Diagnostic::at(
                                DiagnosticCode::DanglingReference,
                                path.field("field").location(),
                                format!(
                                    "The change names the field `{field}`, which `{collection}` lacks."
                                ),
                                format!(
                                    "Name a field declared on the record type of `{collection}`."
                                ),
                            ));
                        }
                    }
                }
            }
        }
        Effect::Read {
            collection,
            binding,
            element,
            ..
        } => {
            report.absorb(binding.validate(&path.field("binding")));
            report.absorb(element.validate(types, &path.field("element")));
            if let Some(found) = collections.get(collection) {
                let declared = TypeExpr::Record(found.record_type.clone());
                if element != &declared {
                    report.push(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        path.field("element").location(),
                        format!(
                            "The read declares {} and `{collection}` holds {}.",
                            element.describe(),
                            declared.describe()
                        ),
                        format!("Declare {} as the element type.", declared.describe()),
                    ));
                }
            }
            ctx.bind(binding, TypeExpr::optional(element.clone()));
        }
        Effect::Emit { payload, .. } => {
            if let Err(diagnostic) = payload.check(ctx, &path.field("payload")) {
                report.push(diagnostic);
            }
        }
        Effect::RunBlock { arguments, .. } => {
            for (position, (name, value)) in arguments.iter().enumerate() {
                let here = path.field("arguments").index(position);
                report.absorb(name.validate(&here.field("name")));
                if let Err(diagnostic) = value.check(ctx, &here.field("value")) {
                    report.push(diagnostic);
                }
            }
        }
        Effect::Delete { .. } | Effect::Transition { .. } => {}
    }
}

/// Check a whole actions section.
pub fn validate_actions(
    actions: &[Action],
    types: &TypeRegistry,
    collections: &CollectionRegistry,
    identities: &Identities,
    rules: &Rules,
    path: &Path,
) -> Report {
    let mut report = Report::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (position, action) in actions.iter().enumerate() {
        let here = path.index(position);
        if !seen.insert(action.id.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                here.field("id").location(),
                format!("Two actions share the name `{}`.", action.id),
                format!("Rename one of the two actions away from `{}`.", action.id),
            ));
        }
        report.merge(action.validate(types, collections, identities, rules, &here));
    }
    report
}

/// An index over the declared actions.
#[derive(Clone, Debug, Default)]
pub struct ActionRegistry<'a> {
    entries: BTreeMap<&'a str, &'a Action>,
}

impl<'a> ActionRegistry<'a> {
    /// Build a registry over a slice of actions.
    pub fn build(actions: &'a [Action]) -> Self {
        let mut entries: BTreeMap<&'a str, &'a Action> = BTreeMap::new();
        for action in actions {
            entries.entry(action.id.as_str()).or_insert(action);
        }
        ActionRegistry { entries }
    }

    /// Look up an action.
    pub fn get(&self, id: &ActionId) -> Option<&'a Action> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether an action exists.
    pub fn contains(&self, id: &ActionId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The action names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the actions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero actions.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for an action the Blueprint
    /// lacks.
    pub fn missing(&self, id: &ActionId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the action `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare an action named `{id}` in the actions section.")
            } else {
                format!("Point the entry at one of these actions: {listed}.")
            },
        )
    }
}

/// Build a boxed library holding zero blocks.
///
/// The Studio uses this call when an action runs no block.
pub fn no_blocks() -> Box<dyn BlockSource> {
    Box::new(NoBlocks)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        collections::{Collection, PurposeDeclaration},
        expression::{BinaryOp, Builtin},
        identities::RoleDef,
        rules::{Precondition, RuleInput},
        types::{Decimal, FieldDef, FieldName, RecordDef, RuleId, TypeDef, TypeId},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![
            TypeDef::Record(RecordDef {
                id: TypeId::new("parcel"),
                fields: vec![
                    FieldDef::new("survey_number", TypeExpr::Text, "The survey number."),
                    FieldDef::new("holder", TypeExpr::Identity, "The registered holder."),
                    FieldDef::new("area", TypeExpr::Decimal { scale: 4 }, "The area."),
                ],
                summary: "One land parcel.".to_string(),
            }),
            TypeDef::Record(RecordDef {
                id: TypeId::new("transfer"),
                fields: vec![
                    FieldDef::new("reference", TypeExpr::Text, "The transfer reference."),
                    FieldDef::new(
                        "moved_area",
                        TypeExpr::Decimal { scale: 4 },
                        "The area moved.",
                    ),
                ],
                summary: "One transfer.".to_string(),
            }),
        ]
    }

    fn collections() -> Vec<Collection> {
        let mut parcel =
            Collection::new("parcel", "parcel", "survey_number", "The parcel register.");
        parcel.purpose =
            PurposeDeclaration::new("The register records land holdings.", &["registration"]);
        let mut transfer =
            Collection::new("transfer", "transfer", "reference", "The transfer log.");
        transfer.purpose = PurposeDeclaration::new("The log records transfers.", &["registration"]);
        vec![parcel, transfer]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![RoleDef::new(
                "registrar",
                None,
                "Registrar",
                "The registrar records transfers.",
            )],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn rules() -> Rules {
        Rules {
            invariants: Vec::new(),
            preconditions: vec![Precondition {
                id: RuleId::new("area_fits"),
                inputs: vec![RuleInput::new("moved_area", TypeExpr::Decimal { scale: 4 })],
                holds: Expr::binary(
                    BinaryOp::Greater,
                    Expr::bound("moved_area"),
                    Expr::decimal(0, 4),
                ),
                problem: "The transfer moves zero land.".to_string(),
                fix: "Enter an area above zero hectares.".to_string(),
            }],
        }
    }

    fn record_transfer() -> Action {
        let mut action = Action::new("record_transfer", "The action records one land transfer.");
        action.params = vec![
            Param::new("survey_number", TypeExpr::Text, "The parcel moved."),
            Param::new("reference", TypeExpr::Text, "The transfer reference."),
            Param::new(
                "moved_area",
                TypeExpr::Decimal { scale: 4 },
                "The area moved.",
            ),
            Param::new(
                "new_holder",
                TypeExpr::Identity,
                "The party receiving the land.",
            ),
        ];
        action.preconditions = vec![RuleId::new("area_fits")];
        action.roles = vec![RoleId::new("registrar")];
        action.class = TransactionClass::Structural;
        action.effects = vec![
            Effect::Read {
                collection: CollectionId::new("parcel"),
                key: KeySelector::Exact(Expr::bound("survey_number")),
                binding: ParamName::new("held"),
                element: TypeExpr::Record(TypeId::new("parcel")),
            },
            Effect::Update {
                collection: CollectionId::new("parcel"),
                key: KeySelector::Exact(Expr::bound("survey_number")),
                field: FieldPath::parse("holder"),
                value: Expr::bound("new_holder"),
            },
            Effect::Insert {
                collection: CollectionId::new("transfer"),
                key: KeySelector::Exact(Expr::bound("reference")),
                record: Expr::MakeRecord {
                    type_id: TypeId::new("transfer"),
                    fields: vec![
                        (FieldName::new("reference"), Expr::bound("reference")),
                        (FieldName::new("moved_area"), Expr::bound("moved_area")),
                    ],
                },
            },
        ];
        action
    }

    fn run(actions: &[Action]) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        validate_actions(
            actions,
            &type_registry,
            &collection_registry,
            &identities(),
            &rules(),
            &Path::root().field("actions"),
        )
    }

    fn arguments() -> BTreeMap<ParamName, Value> {
        let mut bound = BTreeMap::new();
        bound.insert(
            ParamName::new("survey_number"),
            Value::Text("KL-07-119".to_string()),
        );
        bound.insert(
            ParamName::new("reference"),
            Value::Text("TR-2026-0001".to_string()),
        );
        bound.insert(
            ParamName::new("moved_area"),
            Value::Decimal(Decimal::new(5_000, 4)),
        );
        bound.insert(
            ParamName::new("new_holder"),
            Value::Identity(Identity([8u8; 32])),
        );
        bound
    }

    #[test]
    fn a_complete_action_validates() {
        let report = run(&[record_transfer()]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_dangling_collection_reference_reports_its_exact_location() {
        let mut actions = vec![
            Action::new("a", "One."),
            Action::new("b", "Two."),
            Action::new("c", "Three."),
        ];
        for action in actions.iter_mut() {
            action.roles = vec![RoleId::new("registrar")];
        }
        let mut fourth = record_transfer();
        fourth.effects[1] = Effect::Update {
            collection: CollectionId::new("absent_collection"),
            key: KeySelector::Exact(Expr::bound("survey_number")),
            field: FieldPath::parse("holder"),
            value: Expr::bound("new_holder"),
        };
        actions.push(fourth);
        let report = run(&actions);
        let found = report
            .find("actions[3].effects[1].collection")
            .expect("the diagnostic sits on the collection");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("parcel"));
    }

    #[test]
    fn an_effect_writing_the_wrong_type_is_caught() {
        let mut action = record_transfer();
        action.effects[1] = Effect::Update {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Exact(Expr::bound("survey_number")),
            field: FieldPath::parse("holder"),
            value: Expr::bound("reference"),
        };
        let report = run(&[action]);
        let found = report
            .find("actions[0].effects[1].value")
            .expect("the diagnostic sits on the value");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_key_of_the_wrong_type_is_caught() {
        let mut action = record_transfer();
        action.effects[0] = Effect::Read {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Exact(Expr::bound("moved_area")),
            binding: ParamName::new("held"),
            element: TypeExpr::Record(TypeId::new("parcel")),
        };
        let report = run(&[action]);
        let found = report
            .find("actions[0].effects[0].key")
            .expect("the diagnostic sits on the key");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_precondition_the_action_leaves_unsupplied_is_caught() {
        let mut action = record_transfer();
        action
            .params
            .retain(|param| param.name.as_str() != "moved_area");
        action.effects.pop();
        let report = run(&[action]);
        let found = report
            .find("actions[0].preconditions[0]")
            .expect("the diagnostic sits on the precondition");
        assert_eq!(found.code, DiagnosticCode::MissingValue);
        assert!(found.fix.contains("moved_area"));
    }

    #[test]
    fn a_precondition_the_blueprint_lacks_is_caught() {
        let mut action = record_transfer();
        action.preconditions = vec![RuleId::new("absent_rule")];
        let report = run(&[action]);
        let found = report
            .find("actions[0].preconditions[0]")
            .expect("the diagnostic sits on the precondition");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("area_fits"));
    }

    #[test]
    fn the_manifest_names_the_reads_and_the_writes() {
        let action = record_transfer();
        let manifest = action
            .derive_manifest(
                &arguments(),
                Identity([1u8; 32]),
                Timestamp::from_millis(1),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect("the derivation runs");
        assert_eq!(manifest.class, TransactionClass::Structural);
        assert_eq!(manifest.class.threshold_percent(), 80);
        assert_eq!(manifest.reads.len(), 1);
        assert_eq!(manifest.reads[0].collection, CollectionId::new("parcel"));
        assert_eq!(manifest.writes.len(), 2);
        assert_eq!(manifest.collections().len(), 2);

        let parcel_key = Value::Text("KL-07-119".to_string()).canonical_bytes();
        assert!(manifest.admits_write(
            &CollectionId::new("parcel"),
            &KeyRange::Exact(parcel_key.clone())
        ));
        assert!(manifest.admits_read(&CollectionId::new("parcel"), &KeyRange::Exact(parcel_key)));
        assert!(!manifest.admits_write(
            &CollectionId::new("parcel"),
            &KeyRange::Exact(Value::Text("KL-07-120".to_string()).canonical_bytes())
        ));
    }

    #[test]
    fn the_manifest_stays_the_same_across_two_derivations() {
        let action = record_transfer();
        let first = action
            .derive_manifest(
                &arguments(),
                Identity([1u8; 32]),
                Timestamp::from_millis(1),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect("the derivation runs");
        let second = action
            .derive_manifest(
                &arguments(),
                Identity([1u8; 32]),
                Timestamp::from_millis(9),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect("the derivation runs");
        assert_eq!(first, second);
        assert_eq!(first.encode(), second.encode());
    }

    #[test]
    fn a_call_leaving_out_a_parameter_names_the_fix() {
        let action = record_transfer();
        let mut short = arguments();
        short.remove(&ParamName::new("reference"));
        let diagnostic = action
            .derive_manifest(
                &short,
                Identity([1u8; 32]),
                Timestamp::from_millis(1),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect_err("an absent parameter stops the derivation");
        assert_eq!(diagnostic.code, DiagnosticCode::MissingValue);
        assert!(diagnostic.fix.contains("reference"));
    }

    #[test]
    fn a_prefix_key_widens_the_manifest_entry() {
        let mut action = Action::new("clear_district", "The action clears one district.");
        action.roles = vec![RoleId::new("registrar")];
        action.params = vec![Param::new(
            "district",
            TypeExpr::Text,
            "The district prefix.",
        )];
        action.effects = vec![Effect::Delete {
            collection: CollectionId::new("parcel"),
            key: KeySelector::Prefix(Expr::bound("district")),
        }];
        let report = run(&[action.clone()]);
        assert!(report.is_clean(), "{report}");

        let mut bound = BTreeMap::new();
        bound.insert(ParamName::new("district"), Value::Text("KL".to_string()));
        let manifest = action
            .derive_manifest(
                &bound,
                Identity([1u8; 32]),
                Timestamp::from_millis(1),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect("the derivation runs");
        assert_eq!(manifest.writes.len(), 1);
        assert_eq!(manifest.writes[0].range.label(), "prefix");
    }

    #[test]
    fn a_block_the_blueprint_lacks_stops_the_derivation() {
        let mut action = Action::new("run_missing", "The action runs a block.");
        action.roles = vec![RoleId::new("registrar")];
        action.effects = vec![Effect::RunBlock {
            block: BlockId::new("absent_block"),
            arguments: Vec::new(),
        }];
        let diagnostic = action
            .derive_manifest(
                &BTreeMap::new(),
                Identity([1u8; 32]),
                Timestamp::from_millis(1),
                &NoBlocks,
                &Path::root().field("actions").index(0),
            )
            .expect_err("an absent block stops the derivation");
        assert_eq!(diagnostic.code, DiagnosticCode::DanglingReference);
        assert_eq!(diagnostic.location.as_str(), "actions[0].effects[0].block");
    }

    #[test]
    fn the_transaction_classes_carry_their_thresholds() {
        assert_eq!(TransactionClass::Routine.threshold_percent(), 51);
        assert_eq!(TransactionClass::Financial.threshold_percent(), 67);
        assert_eq!(TransactionClass::Structural.threshold_percent(), 80);
        assert_eq!(TransactionClass::Constitutional.threshold_percent(), 90);
        assert!(TransactionClass::Constitutional.holds_delay());
        assert!(!TransactionClass::Routine.holds_delay());
    }

    #[test]
    fn an_action_naming_an_absent_role_is_caught() {
        let mut action = record_transfer();
        action.roles = vec![RoleId::new("absent_role")];
        let report = run(&[action]);
        let found = report
            .find("actions[0].roles[0]")
            .expect("the diagnostic sits on the role");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
    }

    #[test]
    fn the_read_binding_reaches_the_later_effects() {
        let mut action = record_transfer();
        action.effects.push(Effect::Emit {
            event: EventId::new("transfer_recorded"),
            payload: Expr::IsPresent(Box::new(Expr::bound("held"))),
        });
        let report = run(&[action]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn the_action_reports_the_events_it_names() {
        let mut action = record_transfer();
        action.emits = vec![EventId::new("transfer_recorded")];
        action.effects.push(Effect::Emit {
            event: EventId::new("holder_changed"),
            payload: Expr::Call {
                function: Builtin::Caller,
                args: Vec::new(),
            },
        });
        let refs = action.event_refs(&Path::root().field("actions").index(0));
        assert_eq!(refs.len(), 2);
        assert_eq!(refs[0].1.as_str(), "actions[0].emits[0]");
        assert_eq!(refs[1].1.as_str(), "actions[0].effects[3].event");
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let action = record_transfer();
        let bytes = action.encode();
        let back = Action::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(action, back);
        let text = serde_json::to_string(&action).expect("the action serialises");
        let parsed: Action = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(action, parsed);
    }

    #[test]
    fn the_registry_reports_a_missing_action_with_the_fix() {
        let actions = [record_transfer()];
        let registry = ActionRegistry::build(&actions);
        assert!(registry.contains(&ActionId::new("record_transfer")));
        let diagnostic = registry.missing(
            &ActionId::new("absent_action"),
            Path::root().field("workflows").index(0).location(),
        );
        assert!(diagnostic.fix.contains("record_transfer"));
        assert!(no_blocks().block(&BlockId::new("any")).is_none());
    }
}
