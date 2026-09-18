//! Workflows.
//!
//! A workflow holds the states one record passes through, the guarded
//! transitions between them, the timers that fire after a span in a state
//! and the deadlines that fall due at a computed time.
//!
//! The state of a record lives in one field of the record. That field
//! carries an enumeration, so every state name sits in the type system.
//!
//! A workflow declares whether a loop between states is lawful. A workflow
//! that forbids loops is checked for one.

use crate::{
    actions::ActionRegistry,
    collections::CollectionRegistry,
    describe_cycle,
    expression::{Expr, TypeContext},
    find_cycle,
    identities::Identities,
    permissions::ROW_BINDING,
    types::{
        ActionId, CollectionId, EventId, FieldPath, ParamName, RoleId, StateId, TimerId,
        TransitionId, TypeExpr, TypeRegistry, WorkflowId,
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

/// Whether a loop between states is lawful in this workflow.
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
pub enum CyclePolicy {
    /// The states run one way. A loop raises a diagnostic.
    #[default]
    Forbid,
    /// A record returns to an earlier state. A review cycle uses this.
    Allow,
}

/// One state of a workflow.
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
pub struct WorkflowState {
    /// The name of the state.
    pub id: StateId,
    /// A record in a final state stays there.
    pub final_state: bool,
    /// The name a person reads on screen.
    pub title: String,
    /// One plain sentence stating what the state means.
    pub summary: String,
}

impl WorkflowState {
    /// Declare a state a record passes through.
    pub fn passing(id: &str, title: &str, summary: &str) -> Self {
        WorkflowState {
            id: StateId::new(id),
            final_state: false,
            title: title.to_string(),
            summary: summary.to_string(),
        }
    }

    /// Declare a state a record rests in.
    pub fn resting(id: &str, title: &str, summary: &str) -> Self {
        WorkflowState {
            id: StateId::new(id),
            final_state: true,
            title: title.to_string(),
            summary: summary.to_string(),
        }
    }
}

/// One guarded transition between two states.
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
pub struct Transition {
    /// The name of the transition.
    pub id: TransitionId,
    /// The state the record leaves.
    pub from: StateId,
    /// The state the record takes.
    pub to: StateId,
    /// The guard over the record. The record binds to the name `record`.
    pub guard: Option<Expr>,
    /// The action that carries the transition.
    pub action: Option<ActionId>,
    /// The roles that run the transition.
    pub roles: Vec<RoleId>,
}

impl Transition {
    /// Declare a transition with no guard.
    pub fn new(id: &str, from: &str, to: &str) -> Self {
        Transition {
            id: TransitionId::new(id),
            from: StateId::new(from),
            to: StateId::new(to),
            guard: None,
            action: None,
            roles: Vec::new(),
        }
    }

    /// Guard the transition with a predicate over the record.
    pub fn guarded_by(mut self, guard: Expr) -> Self {
        self.guard = Some(guard);
        self
    }

    /// Name the action that carries the transition.
    pub fn carried_by(mut self, action: &str) -> Self {
        self.action = Some(ActionId::new(action));
        self
    }

    /// Name the roles that run the transition.
    pub fn run_by(mut self, roles: &[&str]) -> Self {
        self.roles = roles.iter().map(|role| RoleId::new(role)).collect();
        self
    }
}

/// The step a timer or a deadline runs.
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
pub enum TimerAction {
    /// Move the record to another state.
    MoveTo(StateId),
    /// Run an action against the record.
    Run(ActionId),
    /// Write an event to the audit log.
    Emit(EventId),
}

/// A timer firing after a span inside one state.
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
pub struct Timer {
    /// The name of the timer.
    pub id: TimerId,
    /// The state the timer watches.
    pub state: StateId,
    /// The span in milliseconds from entry to firing.
    pub after_millis: u64,
    /// The step the timer runs.
    pub on_expiry: TimerAction,
}

/// A deadline falling due at a computed time.
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
pub struct Deadline {
    /// The name of the deadline.
    pub id: TimerId,
    /// The state the deadline watches.
    pub state: StateId,
    /// The time the deadline falls due. The record binds to the name
    /// `record`.
    pub due: Expr,
    /// The step the deadline runs once the time passes.
    pub on_miss: TimerAction,
}

/// A workflow.
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
pub struct Workflow {
    /// The name of the workflow.
    pub id: WorkflowId,
    /// The collection the workflow drives.
    pub collection: CollectionId,
    /// The field holding the state of a record.
    pub state_field: FieldPath,
    /// The states.
    pub states: Vec<WorkflowState>,
    /// The state a new record takes.
    pub initial: StateId,
    /// The guarded transitions.
    pub transitions: Vec<Transition>,
    /// The timers.
    pub timers: Vec<Timer>,
    /// The deadlines.
    pub deadlines: Vec<Deadline>,
    /// Whether a loop between states is lawful.
    pub cycles: CyclePolicy,
    /// One plain sentence stating what the workflow drives.
    pub summary: String,
}

impl Workflow {
    /// Report whether the workflow holds the named state.
    pub fn has_state(&self, id: &StateId) -> bool {
        self.states.iter().any(|state| &state.id == id)
    }

    /// The state names, in declaration order.
    pub fn state_names(&self) -> Vec<&str> {
        self.states.iter().map(|state| state.id.as_str()).collect()
    }

    /// Raise a dangling reference diagnostic for a state the workflow lacks.
    pub fn missing_state(&self, id: &StateId, location: Location) -> Diagnostic {
        let listed = self.state_names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!(
                "The entry names the state `{id}`, which `{}` lacks.",
                self.id
            ),
            if listed.is_empty() {
                format!(
                    "Declare a state named `{id}` on the workflow `{}`.",
                    self.id
                )
            } else {
                format!("Point the entry at one of these states: {listed}.")
            },
        )
    }

    /// The states a record reaches from the initial state.
    pub fn reachable(&self) -> BTreeSet<String> {
        let mut found: BTreeSet<String> = BTreeSet::new();
        let mut queue: Vec<&StateId> = Vec::new();
        if self.has_state(&self.initial) {
            found.insert(self.initial.as_str().to_string());
            queue.push(&self.initial);
        }
        while let Some(state) = queue.pop() {
            for transition in self.transitions.iter().filter(|held| &held.from == state) {
                if found.insert(transition.to.as_str().to_string()) {
                    queue.push(&transition.to);
                }
            }
            for timer in self.timers.iter().filter(|held| &held.state == state) {
                if let TimerAction::MoveTo(target) = &timer.on_expiry {
                    if found.insert(target.as_str().to_string()) {
                        queue.push(target);
                    }
                }
            }
            for deadline in self.deadlines.iter().filter(|held| &held.state == state) {
                if let TimerAction::MoveTo(target) = &deadline.on_miss {
                    if found.insert(target.as_str().to_string()) {
                        queue.push(target);
                    }
                }
            }
        }
        found
    }

    /// The actions this workflow names, each with its location.
    pub fn action_refs(&self, path: &Path) -> Vec<(ActionId, Location)> {
        let mut found = Vec::new();
        for (position, transition) in self.transitions.iter().enumerate() {
            if let Some(action) = &transition.action {
                found.push((
                    action.clone(),
                    path.field("transitions")
                        .index(position)
                        .field("action")
                        .location(),
                ));
            }
        }
        for (position, timer) in self.timers.iter().enumerate() {
            if let TimerAction::Run(action) = &timer.on_expiry {
                found.push((
                    action.clone(),
                    path.field("timers")
                        .index(position)
                        .field("on_expiry")
                        .location(),
                ));
            }
        }
        for (position, deadline) in self.deadlines.iter().enumerate() {
            if let TimerAction::Run(action) = &deadline.on_miss {
                found.push((
                    action.clone(),
                    path.field("deadlines")
                        .index(position)
                        .field("on_miss")
                        .location(),
                ));
            }
        }
        found
    }

    /// The events this workflow names, each with its location.
    pub fn event_refs(&self, path: &Path) -> Vec<(EventId, Location)> {
        let mut found = Vec::new();
        for (position, timer) in self.timers.iter().enumerate() {
            if let TimerAction::Emit(event) = &timer.on_expiry {
                found.push((
                    event.clone(),
                    path.field("timers")
                        .index(position)
                        .field("on_expiry")
                        .location(),
                ));
            }
        }
        for (position, deadline) in self.deadlines.iter().enumerate() {
            if let TimerAction::Emit(event) = &deadline.on_miss {
                found.push((
                    event.clone(),
                    path.field("deadlines")
                        .index(position)
                        .field("on_miss")
                        .location(),
                ));
            }
        }
        found
    }

    /// Check one workflow.
    pub fn validate(
        &self,
        types: &TypeRegistry,
        collections: &CollectionRegistry,
        identities: &Identities,
        actions: &ActionRegistry,
        path: &Path,
    ) -> Report {
        let mut report = Report::new();
        report.absorb(self.id.validate(&path.field("id")));

        let collection = collections.get(&self.collection);
        if collection.is_none() {
            report.push(collections.missing(&self.collection, path.field("collection").location()));
        }
        let record_type = collection.map(|found| TypeExpr::Record(found.record_type.clone()));

        report.absorb(self.state_field.validate(&path.field("state_field")));
        if let Some(record_type) = &record_type {
            match self.state_field.resolve(record_type, types) {
                Some(TypeExpr::Enumeration(type_id)) => {
                    if let Some(def) = types.enumeration(&type_id) {
                        for (position, state) in self.states.iter().enumerate() {
                            let held = def
                                .variants
                                .iter()
                                .any(|variant| variant.as_str() == state.id.as_str());
                            if !held {
                                report.push(Diagnostic::at(
                                    DiagnosticCode::DanglingReference,
                                    path.field("states").index(position).field("id").location(),
                                    format!(
                                        "The state `{}` sits outside the enumeration `{type_id}`.",
                                        state.id
                                    ),
                                    format!(
                                        "Add `{}` to the enumeration `{type_id}`, or rename the state.",
                                        state.id
                                    ),
                                ));
                            }
                        }
                    }
                }
                Some(other) => report.push(Diagnostic::at(
                    DiagnosticCode::TypeMismatch,
                    path.field("state_field").location(),
                    format!("The state field carries {}.", other.describe()),
                    "Point the state field at a field carrying an enumeration.".to_string(),
                )),
                None => {
                    if collection.is_some() {
                        report.push(Diagnostic::at(
                            DiagnosticCode::DanglingReference,
                            path.field("state_field").location(),
                            format!(
                                "The state field `{}` resolves to no field on `{}`.",
                                self.state_field, self.collection
                            ),
                            format!(
                                "Name a field declared on the record type of `{}`.",
                                self.collection
                            ),
                        ));
                    }
                }
            }
        }

        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for (position, state) in self.states.iter().enumerate() {
            let here = path.field("states").index(position);
            report.absorb(state.id.validate(&here.field("id")));
            if !seen.insert(state.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!(
                        "The workflow `{}` declares the state `{}` twice.",
                        self.id, state.id
                    ),
                    format!("Rename one of the two `{}` states.", state.id),
                ));
            }
            if state.title.is_empty() {
                report.push(Diagnostic::at(
                    DiagnosticCode::MissingValue,
                    here.field("title").location(),
                    format!("The state `{}` carries an empty title.", state.id),
                    "Give the state the name a person reads on screen.".to_string(),
                ));
            }
        }
        if self.states.is_empty() {
            report.push(Diagnostic::at(
                DiagnosticCode::SectionEmpty,
                path.field("states").location(),
                format!("The workflow `{}` declares zero states.", self.id),
                "Declare at least one state and name it as the initial state.".to_string(),
            ));
        }
        if !self.has_state(&self.initial) {
            report.push(self.missing_state(&self.initial, path.field("initial").location()));
        }

        let mut transition_names: BTreeSet<&str> = BTreeSet::new();
        let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (position, transition) in self.transitions.iter().enumerate() {
            let here = path.field("transitions").index(position);
            report.absorb(transition.id.validate(&here.field("id")));
            if !transition_names.insert(transition.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!(
                        "The workflow `{}` declares the transition `{}` twice.",
                        self.id, transition.id
                    ),
                    format!("Rename one of the two `{}` transitions.", transition.id),
                ));
            }
            if !self.has_state(&transition.from) {
                report.push(self.missing_state(&transition.from, here.field("from").location()));
            }
            if !self.has_state(&transition.to) {
                report.push(self.missing_state(&transition.to, here.field("to").location()));
            }
            if let Some(state) = self.states.iter().find(|held| held.id == transition.from) {
                if state.final_state {
                    report.push(Diagnostic::at(
                        DiagnosticCode::CycleDetected,
                        here.field("from").location(),
                        format!(
                            "The transition `{}` leaves the final state `{}`.",
                            transition.id, transition.from
                        ),
                        format!(
                            "Clear the final mark on `{}`, or remove the transition.",
                            transition.from
                        ),
                    ));
                }
            }
            if let (Some(guard), Some(record_type)) = (&transition.guard, &record_type) {
                let ctx =
                    TypeContext::new(types).with(&ParamName::new(ROW_BINDING), record_type.clone());
                match guard.check(&ctx, &here.field("guard")) {
                    Ok(TypeExpr::Bool) => {}
                    Ok(other) => report.push(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        here.field("guard").location(),
                        format!("The guard carries {}.", other.describe()),
                        "Give the guard a truth value.".to_string(),
                    )),
                    Err(diagnostic) => report.push(diagnostic),
                }
            }
            if let Some(action) = &transition.action {
                if !actions.contains(action) {
                    report.push(actions.missing(action, here.field("action").location()));
                }
            }
            for (index, role) in transition.roles.iter().enumerate() {
                let role_path = here.field("roles").index(index);
                report.absorb(role.validate(&role_path));
                if !identities.has_role(role) {
                    report.push(identities.missing_role(role, role_path.location()));
                }
            }
            edges
                .entry(transition.from.as_str().to_string())
                .or_default()
                .push(transition.to.as_str().to_string());
        }

        if self.cycles == CyclePolicy::Forbid {
            if let Some(cycle) = find_cycle(&edges) {
                let start = cycle.first().cloned().unwrap_or_default();
                let position = self
                    .transitions
                    .iter()
                    .position(|transition| transition.from.as_str() == start)
                    .unwrap_or(0);
                report.push(Diagnostic::at(
                    DiagnosticCode::CycleDetected,
                    path.field("transitions")
                        .index(position)
                        .field("to")
                        .location(),
                    format!(
                        "The workflow `{}` closes a loop: {}.",
                        self.id,
                        describe_cycle(&cycle)
                    ),
                    "Remove one transition on the loop, or set the workflow to allow a loop."
                        .to_string(),
                ));
            }
        }

        let reachable = self.reachable();
        for (position, state) in self.states.iter().enumerate() {
            if !reachable.contains(state.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DanglingReference,
                    path.field("states").index(position).field("id").location(),
                    format!(
                        "A record reaches the state `{}` through no transition.",
                        state.id
                    ),
                    format!("Add a transition into `{}`, or remove the state.", state.id),
                ));
            }
        }

        let mut timer_names: BTreeSet<&str> = BTreeSet::new();
        for (position, timer) in self.timers.iter().enumerate() {
            let here = path.field("timers").index(position);
            report.absorb(timer.id.validate(&here.field("id")));
            if !timer_names.insert(timer.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!(
                        "The workflow `{}` declares the timer `{}` twice.",
                        self.id, timer.id
                    ),
                    format!("Rename one of the two `{}` timers.", timer.id),
                ));
            }
            if !self.has_state(&timer.state) {
                report.push(self.missing_state(&timer.state, here.field("state").location()));
            }
            if timer.after_millis == 0 {
                report.push(Diagnostic::at(
                    DiagnosticCode::ValueOutOfRange,
                    here.field("after_millis").location(),
                    format!("The timer `{}` fires after zero milliseconds.", timer.id),
                    "Set a span above zero.".to_string(),
                ));
            }
            self.check_timer_action(
                &timer.on_expiry,
                actions,
                &here.field("on_expiry"),
                &mut report,
            );
        }

        for (position, deadline) in self.deadlines.iter().enumerate() {
            let here = path.field("deadlines").index(position);
            report.absorb(deadline.id.validate(&here.field("id")));
            if !timer_names.insert(deadline.id.as_str()) {
                report.push(Diagnostic::at(
                    DiagnosticCode::DuplicateIdentifier,
                    here.field("id").location(),
                    format!(
                        "The workflow `{}` declares the deadline `{}` twice.",
                        self.id, deadline.id
                    ),
                    format!("Rename one of the two `{}` deadlines.", deadline.id),
                ));
            }
            if !self.has_state(&deadline.state) {
                report.push(self.missing_state(&deadline.state, here.field("state").location()));
            }
            if let Some(record_type) = &record_type {
                let ctx =
                    TypeContext::new(types).with(&ParamName::new(ROW_BINDING), record_type.clone());
                match deadline.due.check(&ctx, &here.field("due")) {
                    Ok(TypeExpr::Timestamp) => {}
                    Ok(other) => report.push(Diagnostic::at(
                        DiagnosticCode::TypeMismatch,
                        here.field("due").location(),
                        format!("The deadline carries {}.", other.describe()),
                        "Give the deadline a timestamp.".to_string(),
                    )),
                    Err(diagnostic) => report.push(diagnostic),
                }
            }
            self.check_timer_action(
                &deadline.on_miss,
                actions,
                &here.field("on_miss"),
                &mut report,
            );
        }

        report
    }

    fn check_timer_action(
        &self,
        action: &TimerAction,
        actions: &ActionRegistry,
        path: &Path,
        report: &mut Report,
    ) {
        match action {
            TimerAction::MoveTo(state) => {
                if !self.has_state(state) {
                    report.push(self.missing_state(state, path.location()));
                }
            }
            TimerAction::Run(id) => {
                if !actions.contains(id) {
                    report.push(actions.missing(id, path.location()));
                }
            }
            TimerAction::Emit(_) => {}
        }
    }
}

/// An index over the declared workflows.
#[derive(Clone, Debug, Default)]
pub struct WorkflowRegistry<'a> {
    entries: BTreeMap<&'a str, &'a Workflow>,
}

impl<'a> WorkflowRegistry<'a> {
    /// Build a registry over a slice of workflows.
    pub fn build(workflows: &'a [Workflow]) -> Self {
        let mut entries: BTreeMap<&'a str, &'a Workflow> = BTreeMap::new();
        for workflow in workflows {
            entries.entry(workflow.id.as_str()).or_insert(workflow);
        }
        WorkflowRegistry { entries }
    }

    /// Look up a workflow.
    pub fn get(&self, id: &WorkflowId) -> Option<&'a Workflow> {
        self.entries.get(id.as_str()).copied()
    }

    /// Report whether a workflow exists.
    pub fn contains(&self, id: &WorkflowId) -> bool {
        self.entries.contains_key(id.as_str())
    }

    /// The workflow names, in sorted order.
    pub fn names(&self) -> Vec<&str> {
        self.entries.keys().copied().collect()
    }

    /// Count the workflows.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Report whether the registry holds zero workflows.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Raise a dangling reference diagnostic for a workflow the Blueprint
    /// lacks.
    pub fn missing(&self, id: &WorkflowId, location: Location) -> Diagnostic {
        let listed = self.names().join(", ");
        Diagnostic::at(
            DiagnosticCode::DanglingReference,
            location,
            format!("The entry names the workflow `{id}`, which the Blueprint lacks."),
            if listed.is_empty() {
                format!("Declare a workflow named `{id}` in the workflows section.")
            } else {
                format!("Point the entry at one of these workflows: {listed}.")
            },
        )
    }
}

/// Check a whole workflows section.
pub fn validate_workflows(
    workflows: &[Workflow],
    types: &TypeRegistry,
    collections: &CollectionRegistry,
    identities: &Identities,
    actions: &ActionRegistry,
    path: &Path,
) -> Report {
    let mut report = Report::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for (position, workflow) in workflows.iter().enumerate() {
        let here = path.index(position);
        if !seen.insert(workflow.id.as_str()) {
            report.push(Diagnostic::at(
                DiagnosticCode::DuplicateIdentifier,
                here.field("id").location(),
                format!("Two workflows share the name `{}`.", workflow.id),
                format!(
                    "Rename one of the two workflows away from `{}`.",
                    workflow.id
                ),
            ));
        }
        report.merge(workflow.validate(types, collections, identities, actions, &here));
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        actions::{Action, ActionRegistry},
        collections::{Collection, PurposeDeclaration},
        expression::Builtin,
        identities::RoleDef,
        types::{EnumerationDef, FieldDef, RecordDef, TypeDef, TypeId, VariantName},
    };
    use alloc::vec;

    fn types() -> Vec<TypeDef> {
        vec![
            TypeDef::Enumeration(EnumerationDef {
                id: TypeId::new("claim_state"),
                variants: vec![
                    VariantName::new("lodged"),
                    VariantName::new("under_review"),
                    VariantName::new("settled"),
                ],
                summary: "The state of a claim.".to_string(),
            }),
            TypeDef::Record(RecordDef {
                id: TypeId::new("claim"),
                fields: vec![
                    FieldDef::new("reference", TypeExpr::Text, "The claim reference."),
                    FieldDef::new(
                        "state",
                        TypeExpr::Enumeration(TypeId::new("claim_state")),
                        "The state of the claim.",
                    ),
                    FieldDef::new("lodged_at", TypeExpr::Timestamp, "The time of lodging."),
                    FieldDef::new("checked", TypeExpr::Bool, "The review outcome."),
                ],
                summary: "One claim.".to_string(),
            }),
        ]
    }

    fn collections() -> Vec<Collection> {
        let mut claim = Collection::new("claim", "claim", "reference", "The claim register.");
        claim.purpose = PurposeDeclaration::new("The register records claims.", &["settlement"]);
        vec![claim]
    }

    fn identities() -> Identities {
        Identities {
            units: Vec::new(),
            roles: vec![RoleDef::new(
                "assessor",
                None,
                "Assessor",
                "The assessor reviews claims.",
            )],
            credentials: Vec::new(),
            delegations: Vec::new(),
        }
    }

    fn actions() -> Vec<Action> {
        let mut action = Action::new("settle_claim", "The action settles one claim.");
        action.roles = vec![RoleId::new("assessor")];
        vec![action]
    }

    fn sample() -> Workflow {
        Workflow {
            id: WorkflowId::new("claim_flow"),
            collection: CollectionId::new("claim"),
            state_field: FieldPath::parse("state"),
            states: vec![
                WorkflowState::passing("lodged", "Lodged", "The claim arrived."),
                WorkflowState::passing("under_review", "Under review", "An assessor reads it."),
                WorkflowState::resting("settled", "Settled", "The claim closed."),
            ],
            initial: StateId::new("lodged"),
            transitions: vec![
                Transition::new("start_review", "lodged", "under_review").run_by(&["assessor"]),
                Transition::new("settle", "under_review", "settled")
                    .guarded_by(Expr::field(Expr::bound(ROW_BINDING), "checked"))
                    .carried_by("settle_claim")
                    .run_by(&["assessor"]),
            ],
            timers: vec![Timer {
                id: TimerId::new("review_reminder"),
                state: StateId::new("under_review"),
                after_millis: 604_800_000,
                on_expiry: TimerAction::Emit(EventId::new("review_overdue")),
            }],
            deadlines: vec![Deadline {
                id: TimerId::new("settlement_due"),
                state: StateId::new("under_review"),
                due: Expr::Call {
                    function: Builtin::TimestampPlus,
                    args: vec![
                        Expr::field(Expr::bound(ROW_BINDING), "lodged_at"),
                        Expr::integer(2_592_000_000),
                    ],
                },
                on_miss: TimerAction::MoveTo(StateId::new("settled")),
            }],
            cycles: CyclePolicy::Forbid,
            summary: "The workflow drives a claim from lodging to settlement.".to_string(),
        }
    }

    fn run(workflows: &[Workflow]) -> Report {
        let defs = types();
        let (type_registry, _) = TypeRegistry::build(&defs, &Path::root().field("types"));
        let held = collections();
        let (collection_registry, _) =
            CollectionRegistry::build(&held, &Path::root().field("collections"));
        let declared = actions();
        let action_registry = ActionRegistry::build(&declared);
        validate_workflows(
            workflows,
            &type_registry,
            &collection_registry,
            &identities(),
            &action_registry,
            &Path::root().field("workflows"),
        )
    }

    #[test]
    fn a_complete_workflow_validates() {
        let report = run(&[sample()]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_transition_naming_an_absent_state_reports_its_location() {
        let mut workflow = sample();
        workflow.transitions[0].to = StateId::new("absent_state");
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].transitions[0].to")
            .expect("the diagnostic sits on the target state");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("under_review"));
    }

    #[test]
    fn a_loop_in_a_workflow_forbidding_loops_is_detected() {
        let mut workflow = sample();
        workflow.states[2].final_state = false;
        workflow
            .transitions
            .push(Transition::new("reopen", "settled", "lodged"));
        let report = run(&[workflow]);
        let found = report
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::CycleDetected)
            .expect("the loop is reported");
        assert!(found.problem.contains("->"));
        assert!(found.fix.contains("allow a loop"));
    }

    #[test]
    fn a_loop_in_a_workflow_allowing_loops_is_lawful() {
        let mut workflow = sample();
        workflow.states[2].final_state = false;
        workflow.cycles = CyclePolicy::Allow;
        workflow
            .transitions
            .push(Transition::new("reopen", "settled", "lodged"));
        let report = run(&[workflow]);
        assert!(report.is_clean(), "{report}");
    }

    #[test]
    fn a_state_outside_the_enumeration_reports_the_fix() {
        let mut workflow = sample();
        workflow.states.push(WorkflowState::passing(
            "withdrawn",
            "Withdrawn",
            "The claim left.",
        ));
        workflow
            .transitions
            .push(Transition::new("withdraw", "lodged", "withdrawn"));
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].states[3].id")
            .expect("the diagnostic sits on the new state");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("claim_state"));
    }

    #[test]
    fn an_unreachable_state_reports_its_location() {
        let mut workflow = sample();
        workflow.transitions.remove(0);
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].states[1].id")
            .expect("the diagnostic sits on the unreachable state");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("Add a transition"));
    }

    #[test]
    fn a_guard_carrying_a_value_other_than_a_truth_value_is_caught() {
        let mut workflow = sample();
        workflow.transitions[1].guard = Some(Expr::field(Expr::bound(ROW_BINDING), "reference"));
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].transitions[1].guard")
            .expect("the diagnostic sits on the guard");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
    }

    #[test]
    fn a_deadline_carrying_a_value_other_than_a_timestamp_is_caught() {
        let mut workflow = sample();
        workflow.deadlines[0].due = Expr::field(Expr::bound(ROW_BINDING), "reference");
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].deadlines[0].due")
            .expect("the diagnostic sits on the deadline");
        assert_eq!(found.code, DiagnosticCode::TypeMismatch);
        assert!(found.fix.contains("timestamp"));
    }

    #[test]
    fn a_transition_leaving_a_final_state_is_caught() {
        let mut workflow = sample();
        workflow.cycles = CyclePolicy::Allow;
        workflow
            .transitions
            .push(Transition::new("reopen", "settled", "lodged"));
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].transitions[2].from")
            .expect("the diagnostic sits on the source state");
        assert_eq!(found.code, DiagnosticCode::CycleDetected);
    }

    #[test]
    fn a_transition_naming_an_absent_action_is_caught() {
        let mut workflow = sample();
        workflow.transitions[1].action = Some(ActionId::new("absent_action"));
        let report = run(&[workflow]);
        let found = report
            .find("workflows[0].transitions[1].action")
            .expect("the diagnostic sits on the action");
        assert_eq!(found.code, DiagnosticCode::DanglingReference);
        assert!(found.fix.contains("settle_claim"));
    }

    #[test]
    fn a_timer_firing_at_zero_is_caught() {
        let mut workflow = sample();
        workflow.timers[0].after_millis = 0;
        let report = run(&[workflow]);
        assert!(report.find("workflows[0].timers[0].after_millis").is_some());
    }

    #[test]
    fn the_workflow_reports_the_events_and_the_actions_it_names() {
        let workflow = sample();
        let events = workflow.event_refs(&Path::root().field("workflows").index(0));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].1.as_str(), "workflows[0].timers[0].on_expiry");
        let actions = workflow.action_refs(&Path::root().field("workflows").index(0));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, ActionId::new("settle_claim"));
    }

    #[test]
    fn a_guarded_transition_reads_the_record() {
        let workflow = sample();
        let guard = workflow.transitions[1]
            .guard
            .clone()
            .expect("the guard is present");
        assert_eq!(guard, Expr::field(Expr::bound(ROW_BINDING), "checked"));
    }

    #[test]
    fn the_section_round_trips_through_scale_and_json() {
        let workflow = sample();
        let bytes = workflow.encode();
        let back = Workflow::decode(&mut &bytes[..]).expect("the encoding decodes");
        assert_eq!(workflow, back);
        let text = serde_json::to_string(&workflow).expect("the workflow serialises");
        let parsed: Workflow = serde_json::from_str(&text).expect("the text parses");
        assert_eq!(workflow, parsed);
    }
}
