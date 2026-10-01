//! Action execution with the effect sequence.
//!
//! An action runs in six stages, in order.
//!
//! 1. **The arguments.** Every declared parameter takes a value. A missing
//!    value aborts before anything runs.
//! 2. **The roles.** An action that names roles admits a caller holding
//!    one of them. A delegation carrying the action admits the caller as
//!    well.
//! 3. **The manifest.** The action derives its Access Manifest from the
//!    arguments. Every later effect must sit inside that manifest, so the
//!    state an action touches is declared before the action runs.
//! 4. **The preconditions.** Every named precondition evaluates against
//!    the arguments. A precondition that declines returns the plain
//!    sentence the author wrote beside the rule.
//! 5. **The effects.** The effects run in order through the state module,
//!    so every write resolves permission, validates against the schema,
//!    writes the record and maintains the indexes.
//! 6. **The invariants.** Every invariant over a touched collection
//!    evaluates against the records the action wrote.
//!
//! A failure at stage five or stage six rolls the effects back. An action
//! that declares compensating steps then runs those steps in reverse
//! order and the Blockchain keeps the compensation. An action that
//! declares none aborts the whole transaction.

use alloc::{string::String, vec::Vec};
use cerulea_blueprint::{
    actions::{AccessManifest, Action, Effect},
    collections::{KeyRange, KeySelector},
    expression::{Env, Expr, Meter},
    types::{CollectionId, EventId, FieldName, FieldPath, ParamName, Value},
    Blueprint, Path,
};
use frame_support::{
    ensure, pallet_prelude::DispatchResult, storage::with_storage_layer, traits::Get,
};
use pallet_cerulea_identity::{DelegationOutcome, IdentityRegistry};
use pallet_cerulea_schema::primitives::{name_of, DeploymentId, RecordKey};
use pallet_cerulea_state::RecordStore;
use sp_runtime::DispatchError;

use crate::{Bindings, Config, Error, Event, Pallet, StateView};

/// The place an action sends a workflow transition.
///
/// The workflow module implements this trait. A runtime that leaves the
/// workflow module out supplies the unit type, which declines every
/// transition and names the fix.
pub trait TransitionSink {
    /// The account type the runtime carries.
    type AccountId;

    /// Move one record to another workflow state.
    fn transition(
        deployment: &DeploymentId,
        workflow: &cerulea_blueprint::types::WorkflowId,
        collection: &CollectionId,
        key: &RecordKey,
        to: &cerulea_blueprint::types::StateId,
        actor: &Self::AccountId,
    ) -> DispatchResult;
}

/// The sink of a runtime carrying no workflow module.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoWorkflows<AccountId>(core::marker::PhantomData<AccountId>);

impl<AccountId> TransitionSink for NoWorkflows<AccountId> {
    type AccountId = AccountId;

    fn transition(
        _deployment: &DeploymentId,
        _workflow: &cerulea_blueprint::types::WorkflowId,
        _collection: &CollectionId,
        _key: &RecordKey,
        _to: &cerulea_blueprint::types::StateId,
        _actor: &Self::AccountId,
    ) -> DispatchResult {
        Err(DispatchError::Other(
            "Add the workflow module to the runtime to run a transition.",
        ))
    }
}

/// What one action run produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionOutcome {
    /// The number of effects the run performed.
    pub effects: u32,
    /// The number of expression steps the run spent.
    pub steps: u64,
    /// The events the run emitted, in order.
    pub events: Vec<(EventId, Value)>,
    /// The records the run wrote.
    pub touched: Vec<(CollectionId, RecordKey)>,
    /// The plain sentence a failed run carries.
    pub failure: Option<String>,
}

impl ExecutionOutcome {
    /// Start an empty outcome.
    pub fn new() -> Self {
        ExecutionOutcome {
            effects: 0,
            steps: 0,
            events: Vec::new(),
            touched: Vec::new(),
            failure: None,
        }
    }

    /// Report whether the run landed every effect.
    pub fn succeeded(&self) -> bool {
        self.failure.is_none()
    }
}

impl Default for ExecutionOutcome {
    fn default() -> Self {
        ExecutionOutcome::new()
    }
}

/// Write one value at a field path inside a record.
///
/// A path naming a field the record lacks answers with nothing, so the
/// caller reports the problem rather than writing a new shape.
pub fn set_at(record: &Value, path: &FieldPath, value: Value) -> Option<Value> {
    set_step(record, path.0.as_slice(), value)
}

fn set_step(record: &Value, path: &[FieldName], value: Value) -> Option<Value> {
    let head = path.first()?;
    match record {
        Value::Record(held) => {
            let mut fields: Vec<(FieldName, Value)> = held.fields.clone();
            let slot = fields.iter().position(|(name, _)| name == head)?;
            let produced = if path.len() == 1 {
                value
            } else {
                set_step(&fields[slot].1, &path[1..], value)?
            };
            fields[slot].1 = produced;
            Some(Value::record(held.type_id.clone(), fields))
        }
        _ => None,
    }
}

impl<T: Config> Pallet<T> {
    /// Run one action end to end.
    ///
    /// The result reports the effects the run landed, the events it
    /// emitted and the steps it spent. A run that failed and compensated
    /// carries the plain sentence a person reads.
    pub fn run_action(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        action_name: &str,
        arguments: &Bindings,
    ) -> Result<ExecutionOutcome, DispatchError> {
        let blueprint = Self::installed(deployment)?;
        let action = blueprint
            .actions
            .iter()
            .find(|held| held.id.as_str() == action_name)
            .cloned()
            .ok_or(Error::<T>::ActionUnknown)?;

        // Stage one. Every declared parameter takes a value.
        let mut bound: Bindings = Vec::new();
        for param in action.params.iter() {
            let value = arguments
                .iter()
                .find(|(name, _)| name == &param.name)
                .map(|(_, value)| value.clone())
                .ok_or(Error::<T>::ParameterMissing)?;
            bound.push((param.name.clone(), value));
        }

        // Stage two. The roles the action names.
        Self::check_roles(deployment, actor, &action)?;

        // Stage three. The Access Manifest of this call.
        let manifest = Self::derive_manifest(deployment, actor, &action, &bound, &blueprint)?;

        // Stage four. The preconditions.
        let mut meter = Self::meter();
        Self::check_preconditions(deployment, actor, &action, &bound, &blueprint, &mut meter)?;

        // Stage five and stage six inside one storage layer.
        let attempt = with_storage_layer(|| {
            let mut outcome = ExecutionOutcome::new();
            let mut held = Self::meter();
            Self::run_effects(
                deployment,
                actor,
                &action.effects,
                &bound,
                &blueprint,
                &manifest,
                &mut held,
                &mut outcome,
            )?;
            Self::check_invariants(deployment, actor, &blueprint, &outcome, &mut held)?;
            outcome.steps = held.used();
            Ok(outcome)
        });

        match attempt {
            Ok(mut outcome) => {
                outcome.steps = outcome.steps.saturating_add(meter.used());
                for (event, payload) in outcome.events.iter() {
                    Self::deposit_event(Event::EventEmitted {
                        deployment: *deployment,
                        event: Self::bounded(event.as_str())?,
                        actor: actor.clone(),
                        digest: Self::digest_of(payload),
                    });
                }
                crate::ActionRuns::<T>::mutate(deployment, |count| {
                    *count = count.saturating_add(1)
                });
                Self::deposit_event(Event::ActionExecuted {
                    deployment: *deployment,
                    action: Self::bounded(action.id.as_str())?,
                    actor: actor.clone(),
                    effects: outcome.effects,
                    steps: outcome.steps,
                });
                Ok(outcome)
            }
            Err(failure) => {
                if action.compensation.is_empty() {
                    return Err(failure);
                }
                Self::compensate(deployment, actor, &action, &bound, &blueprint, failure)
            }
        }
    }

    fn check_roles(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        action: &Action,
    ) -> DispatchResult {
        if action.roles.is_empty() {
            return Ok(());
        }
        let now = T::Identity::now_millis();
        let held = T::Identity::roles_of(deployment, actor, now);
        for role in action.roles.iter() {
            if let Ok(named) = name_of(role.as_str()) {
                if held.contains(&named) {
                    return Ok(());
                }
            }
        }
        if let Ok(named) = name_of(action.id.as_str()) {
            let outcome = T::Identity::delegation_for(deployment, actor, &named, now);
            if let DelegationOutcome::Allowed { role, .. } = outcome {
                for declared in action.roles.iter() {
                    if let Ok(named_role) = name_of(declared.as_str()) {
                        if named_role == role {
                            return Ok(());
                        }
                    }
                }
            }
        }
        Err(Error::<T>::PermissionDenied.into())
    }

    fn derive_manifest(
        _deployment: &DeploymentId,
        actor: &T::AccountId,
        action: &Action,
        bound: &Bindings,
        blueprint: &Blueprint,
    ) -> Result<AccessManifest, DispatchError> {
        let mut map: alloc::collections::BTreeMap<ParamName, Value> =
            alloc::collections::BTreeMap::new();
        for (name, value) in bound.iter() {
            map.insert(name.clone(), value.clone());
        }
        let library = cerulea_blueprint::compose::BlockLibrary::build(&blueprint.blocks);
        action
            .derive_manifest(
                &map,
                Self::identity_of(actor),
                Self::now(),
                &library,
                &Path::root().field("action"),
            )
            .map_err(|_| Error::<T>::ManifestBreach.into())
    }

    fn check_preconditions(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        action: &Action,
        bound: &Bindings,
        blueprint: &Blueprint,
        meter: &mut Meter,
    ) -> DispatchResult {
        let view = StateView::<T>::new(*deployment, T::MaxRowsPerRead::get());
        let mut env = Env::with_view(Self::identity_of(actor), Self::now(), &view);
        for (name, value) in bound.iter() {
            env.bind(name, value.clone());
        }
        for rule in action.preconditions.iter() {
            let held = blueprint
                .rules
                .precondition(rule)
                .ok_or(Error::<T>::PreconditionFailed)?;
            let passed = held
                .check_inputs(&env, meter, &Path::root().field("precondition"))
                .map_err(|_| Error::<T>::BudgetSpent)?;
            ensure!(passed, Error::<T>::PreconditionFailed);
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_effects(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        effects: &[Effect],
        bound: &Bindings,
        blueprint: &Blueprint,
        manifest: &AccessManifest,
        meter: &mut Meter,
        outcome: &mut ExecutionOutcome,
    ) -> DispatchResult {
        let view = StateView::<T>::new(*deployment, T::MaxRowsPerRead::get());
        let mut env = Env::with_view(Self::identity_of(actor), Self::now(), &view);
        for (name, value) in bound.iter() {
            env.bind(name, value.clone());
        }
        Self::walk_effects(
            deployment, actor, effects, &mut env, blueprint, manifest, meter, outcome,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn walk_effects(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        effects: &[Effect],
        env: &mut Env,
        blueprint: &Blueprint,
        manifest: &AccessManifest,
        meter: &mut Meter,
        outcome: &mut ExecutionOutcome,
    ) -> DispatchResult {
        for effect in effects.iter() {
            ensure!(
                outcome.effects < T::MaxEffectsPerAction::get(),
                Error::<T>::EffectLimitReached
            );
            outcome.effects = outcome.effects.saturating_add(1);
            Self::run_effect(
                deployment, actor, effect, env, blueprint, manifest, meter, outcome,
            )?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn run_effect(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        effect: &Effect,
        env: &mut Env,
        blueprint: &Blueprint,
        manifest: &AccessManifest,
        meter: &mut Meter,
        outcome: &mut ExecutionOutcome,
    ) -> DispatchResult {
        let path = Path::root().field("effect");
        match effect {
            Effect::Insert {
                collection,
                key,
                record,
            } => {
                let range = Self::resolve_range(key, env, meter, &path)?;
                ensure!(
                    manifest.admits_write(collection, &range),
                    Error::<T>::ManifestBreach
                );
                let value = Self::run_in(record, env, meter, &path)?;
                let written = T::Records::insert(deployment, collection, actor, &value)?;
                outcome.touched.push((collection.clone(), written));
            }
            Effect::Update {
                collection,
                key,
                field,
                value,
            } => {
                let range = Self::resolve_range(key, env, meter, &path)?;
                ensure!(
                    manifest.admits_write(collection, &range),
                    Error::<T>::ManifestBreach
                );
                let produced = Self::run_in(value, env, meter, &path)?;
                let held =
                    T::Records::range(deployment, collection, &range, T::MaxRowsPerRead::get());
                for (record_key, record) in held.into_iter() {
                    let changed = set_at(&record, field, produced.clone())
                        .ok_or(Error::<T>::RecordInvalid)?;
                    T::Records::update(deployment, collection, actor, &record_key, &changed)?;
                    outcome.touched.push((collection.clone(), record_key));
                }
            }
            Effect::Delete { collection, key } => {
                let range = Self::resolve_range(key, env, meter, &path)?;
                ensure!(
                    manifest.admits_write(collection, &range),
                    Error::<T>::ManifestBreach
                );
                let held = T::Records::keys(deployment, collection, T::MaxRowsPerRead::get());
                for record_key in held.into_iter() {
                    if Self::in_range(&range, record_key.as_slice()) {
                        T::Records::delete(deployment, collection, actor, &record_key)?;
                    }
                }
            }
            Effect::Read {
                collection,
                key,
                binding,
                ..
            } => {
                let range = Self::resolve_range(key, env, meter, &path)?;
                ensure!(
                    manifest.admits_read(collection, &range),
                    Error::<T>::ManifestBreach
                );
                let held =
                    T::Records::range(deployment, collection, &range, T::MaxRowsPerRead::get());
                let record = held
                    .into_iter()
                    .next()
                    .map(|(_, value)| value)
                    .ok_or(Error::<T>::RecordInvalid)?;
                env.bind(binding, record);
            }
            Effect::Emit { event, payload } => {
                let declared = blueprint
                    .events
                    .iter()
                    .find(|held| &held.id == event)
                    .ok_or(Error::<T>::EventUnknown)?;
                let value = Self::run_in(payload, env, meter, &path)?;
                let _ = declared;
                outcome.events.push((event.clone(), value));
            }
            Effect::Transition {
                workflow,
                collection,
                key,
                to,
            } => {
                let range = Self::resolve_range(key, env, meter, &path)?;
                ensure!(
                    manifest.admits_write(collection, &range),
                    Error::<T>::ManifestBreach
                );
                let held = T::Records::keys(deployment, collection, T::MaxRowsPerRead::get());
                let record_key = held
                    .into_iter()
                    .find(|found| Self::in_range(&range, found.as_slice()))
                    .ok_or(Error::<T>::RecordInvalid)?;
                T::Workflows::transition(deployment, workflow, collection, &record_key, to, actor)?;
                outcome.touched.push((collection.clone(), record_key));
            }
            Effect::RunBlock { block, arguments } => {
                let declared = blueprint
                    .blocks
                    .iter()
                    .find(|held| &held.id == block)
                    .ok_or(Error::<T>::BlockUnknown)?;
                let mut replacements: alloc::collections::BTreeMap<ParamName, Expr> =
                    alloc::collections::BTreeMap::new();
                for (name, value) in arguments.iter() {
                    replacements.insert(name.clone(), value.clone());
                }
                let steps: Vec<Effect> = declared
                    .effects
                    .iter()
                    .map(|held| held.substitute(&replacements))
                    .collect();
                Self::walk_effects(
                    deployment, actor, &steps, env, blueprint, manifest, meter, outcome,
                )?;
            }
        }
        Ok(())
    }

    fn resolve_range(
        key: &KeySelector,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<KeyRange, DispatchError> {
        key.resolve(env, meter, path)
            .map_err(|_| Error::<T>::BudgetSpent.into())
    }

    fn in_range(range: &KeyRange, key: &[u8]) -> bool {
        match range {
            KeyRange::Full => true,
            KeyRange::Exact(held) => held.as_slice() == key,
            KeyRange::Prefix(held) => key.starts_with(held.as_slice()),
        }
    }

    fn check_invariants(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        blueprint: &Blueprint,
        outcome: &ExecutionOutcome,
        meter: &mut Meter,
    ) -> DispatchResult {
        let view = StateView::<T>::new(*deployment, T::MaxRowsPerRead::get());
        let env = Env::with_view(Self::identity_of(actor), Self::now(), &view);
        for (collection, key) in outcome.touched.iter() {
            let invariants = blueprint.rules.invariants_over(collection);
            if invariants.is_empty() {
                continue;
            }
            let record = match T::Records::read(deployment, collection, key) {
                Some(held) => held,
                None => continue,
            };
            for invariant in invariants.iter() {
                let held = invariant
                    .check_record(&record, &env, meter, &Path::root().field("invariant"))
                    .map_err(|_| Error::<T>::BudgetSpent)?;
                ensure!(held, Error::<T>::PreconditionFailed);
            }
        }
        Ok(())
    }

    fn compensate(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        action: &Action,
        bound: &Bindings,
        blueprint: &Blueprint,
        failure: DispatchError,
    ) -> Result<ExecutionOutcome, DispatchError> {
        let reversed: Vec<Effect> = action.compensation.iter().rev().cloned().collect();
        let manifest = Self::derive_manifest(deployment, actor, action, bound, blueprint)?;
        let mut outcome = ExecutionOutcome::new();
        let mut meter = Self::meter();
        Self::run_effects(
            deployment,
            actor,
            &reversed,
            bound,
            blueprint,
            &manifest,
            &mut meter,
            &mut outcome,
        )?;
        outcome.steps = meter.used();
        outcome.failure = Some(Self::failure_sentence(&failure));

        for (event, payload) in outcome.events.iter() {
            Self::deposit_event(Event::EventEmitted {
                deployment: *deployment,
                event: Self::bounded(event.as_str())?,
                actor: actor.clone(),
                digest: Self::digest_of(payload),
            });
        }
        let reason = outcome.failure.clone().unwrap_or_default();
        Self::deposit_event(Event::ActionCompensated {
            deployment: *deployment,
            action: Self::bounded(action.id.as_str())?,
            actor: actor.clone(),
            reason: frame_support::BoundedVec::truncate_from(reason.into_bytes()),
        });
        Ok(outcome)
    }

    /// The plain sentence a failed run carries.
    pub fn failure_sentence(failure: &DispatchError) -> String {
        match failure {
            DispatchError::Module(held) => match held.message {
                Some(message) => alloc::format!(
                    "The action stopped at the step reporting {message}. The compensating steps ran in reverse."
                ),
                None => String::from(
                    "The action stopped at one of its steps. The compensating steps ran in reverse.",
                ),
            },
            // A message that already closes with a full stop keeps one full
            // stop in the sentence.
            DispatchError::Other(message) => alloc::format!(
                "The action stopped at the step reporting {}. The compensating steps ran in reverse.",
                message.trim_end_matches('.')
            ),
            _ => String::from(
                "The action stopped at one of its steps. The compensating steps ran in reverse.",
            ),
        }
    }
}
