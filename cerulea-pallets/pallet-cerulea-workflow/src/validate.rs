//! The checks one machine definition passes before it is installed.
//!
//! The Studio checks a whole Blueprint before deployment. The Blockchain
//! checks the machine again against the registered types and the
//! registered collections. A definition failing any check leaves the
//! registry untouched.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec::Vec,
};
use cerulea_blueprint::{
    expression::TypeContext,
    types::{ParamName, TypeExpr},
    workflows::{CyclePolicy, Workflow},
    Path,
};
use frame_support::ensure;
use pallet_cerulea_schema::{primitives::DeploymentId, registry::SchemaSnapshot};
use sp_runtime::DispatchError;

use crate::{Config, Error, RECORD_BINDING};

/// Check one machine definition against the registered definitions.
///
/// The check covers the collection, the states, the transitions, the
/// guards, the timers, the deadlines and the loop policy. A clean result
/// means the machine installs.
pub fn check_machine<T: Config>(
    deployment: &DeploymentId,
    machine: &Workflow,
) -> Result<(), DispatchError> {
    ensure!(!machine.states.is_empty(), Error::<T>::MachineInvalid);
    ensure!(
        machine.has_state(&machine.initial),
        Error::<T>::MachineInvalid
    );

    let snapshot = SchemaSnapshot::load::<T::Schema>(deployment);
    let collection = snapshot
        .collection(&machine.collection)
        .ok_or(Error::<T>::CollectionUnknown)?
        .clone();
    let registry = snapshot.type_registry();
    let record_type = TypeExpr::Record(collection.record_type.clone());

    // The state field points at an enumeration holding every state name.
    match machine.state_field.resolve(&record_type, &registry) {
        Some(TypeExpr::Enumeration(type_id)) => {
            let definition = registry
                .enumeration(&type_id)
                .ok_or(Error::<T>::MachineInvalid)?;
            for state in machine.states.iter() {
                let held = definition
                    .variants
                    .iter()
                    .any(|variant| variant.as_str() == state.id.as_str());
                ensure!(held, Error::<T>::MachineInvalid);
            }
        }
        _ => return Err(Error::<T>::StateFieldInvalid.into()),
    }

    let mut names: BTreeSet<String> = BTreeSet::new();
    for state in machine.states.iter() {
        ensure!(
            names.insert(state.id.as_str().to_string()),
            Error::<T>::MachineInvalid
        );
        ensure!(!state.title.is_empty(), Error::<T>::MachineInvalid);
    }

    let context = TypeContext::new(&registry).with(&ParamName::new(RECORD_BINDING), record_type);
    let path = Path::root().field("workflows").key(machine.id.as_str());

    let mut edges: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut transitions: BTreeSet<String> = BTreeSet::new();
    for transition in machine.transitions.iter() {
        ensure!(
            transitions.insert(transition.id.as_str().to_string()),
            Error::<T>::MachineInvalid
        );
        ensure!(
            machine.has_state(&transition.from),
            Error::<T>::MachineInvalid
        );
        ensure!(
            machine.has_state(&transition.to),
            Error::<T>::MachineInvalid
        );
        if let Some(guard) = &transition.guard {
            let carried = guard
                .check(&context, &path.field("guard"))
                .map_err(|_| Error::<T>::MachineInvalid)?;
            ensure!(carried == TypeExpr::Bool, Error::<T>::MachineInvalid);
        }
        edges
            .entry(transition.from.as_str().to_string())
            .or_default()
            .push(transition.to.as_str().to_string());
    }

    let mut timers: BTreeSet<String> = BTreeSet::new();
    for timer in machine.timers.iter() {
        ensure!(
            timers.insert(timer.id.as_str().to_string()),
            Error::<T>::MachineInvalid
        );
        ensure!(machine.has_state(&timer.state), Error::<T>::MachineInvalid);
        ensure!(timer.after_millis > 0, Error::<T>::MachineInvalid);
        check_action::<T>(machine, &timer.on_expiry)?;
    }

    for deadline in machine.deadlines.iter() {
        ensure!(
            timers.insert(deadline.id.as_str().to_string()),
            Error::<T>::MachineInvalid
        );
        ensure!(
            machine.has_state(&deadline.state),
            Error::<T>::MachineInvalid
        );
        let carried = deadline
            .due
            .check(&context, &path.field("due"))
            .map_err(|_| Error::<T>::MachineInvalid)?;
        ensure!(carried == TypeExpr::Timestamp, Error::<T>::DeadlineInvalid);
        check_action::<T>(machine, &deadline.on_miss)?;
    }

    if machine.cycles == CyclePolicy::Forbid {
        ensure!(
            cerulea_blueprint::find_cycle(&edges).is_none(),
            Error::<T>::MachineInvalid
        );
    }

    Ok(())
}

/// Check that one timer step names a state the machine declares.
fn check_action<T: Config>(
    machine: &Workflow,
    action: &cerulea_blueprint::workflows::TimerAction,
) -> Result<(), DispatchError> {
    if let cerulea_blueprint::workflows::TimerAction::MoveTo(state) = action {
        ensure!(machine.has_state(state), Error::<T>::MachineInvalid);
    }
    Ok(())
}
