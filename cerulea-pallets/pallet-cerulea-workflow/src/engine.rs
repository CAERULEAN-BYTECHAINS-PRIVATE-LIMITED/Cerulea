//! The evaluator seam and the action queue of the workflow module.
//!
//! A guard is a Blueprint expression carrying a truth value. A deadline is
//! a Blueprint expression carrying a timestamp. Both run through the same
//! metered evaluator the logic module uses, reached here through the
//! [`GuardEngine`] trait. The trait keeps the workflow module free of any
//! concrete evaluator, so a runtime supplies the evaluator it carries.
//!
//! A timer that runs an action hands the request to the [`ActionRunner`]
//! trait. The queue implementation in this module records the request in
//! workflow storage. The logic module drains the queue and runs the action
//! against the record.

use alloc::vec::Vec;
use cerulea_blueprint::{
    expression::{Env, Expr, Meter},
    types::{ActionId, CollectionId, Value},
    Diagnostic, Path,
};
use core::marker::PhantomData;

use pallet_cerulea_schema::primitives::{name_of, DeploymentId, RecordKey};
use sp_runtime::DispatchError;

use crate::{Config, DueAction, DueActions, Error, NextDueAction, Pallet};

/// The evaluator a guard and a deadline both run through.
///
/// The evaluator charges one step for every node it visits and stops at
/// the step limit. A run that passes the limit answers with a bound
/// diagnostic naming the limit and the fix.
pub trait GuardEngine {
    /// Evaluate one expression over one environment.
    fn evaluate(
        expr: &Expr,
        env: &Env,
        steps: u64,
        path: &Path,
    ) -> Result<Value, alloc::boxed::Box<Diagnostic>>;
}

/// The metered evaluator of the Blueprint.
///
/// Every node costs one step. The step limit comes from the runtime, so
/// two nodes charge the same work for the same guard.
pub struct MeteredGuards;

impl GuardEngine for MeteredGuards {
    fn evaluate(
        expr: &Expr,
        env: &Env,
        steps: u64,
        path: &Path,
    ) -> Result<Value, alloc::boxed::Box<Diagnostic>> {
        let mut meter = Meter::new(steps);
        expr.evaluate(env, &mut meter, path)
            .map_err(alloc::boxed::Box::new)
    }
}

/// One request to run a Blueprint action against a record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActionRequest<'a, AccountId> {
    /// The deployment the record belongs to.
    pub deployment: DeploymentId,
    /// The collection the record belongs to.
    pub collection: &'a CollectionId,
    /// The record key.
    pub record: &'a RecordKey,
    /// The action the workflow asks for.
    pub action: &'a ActionId,
    /// The account the Blockchain records as the actor.
    pub actor: &'a AccountId,
}

/// One queued action request with the identifier the queue gave it.
pub type QueuedRequest<T> = (
    u64,
    DueAction<<T as frame_system::Config>::AccountId, BlockNumberFor<T>>,
);

/// The runner a timer hands its action to.
///
/// The logic module runs Blueprint actions. The workflow module holds the
/// state machine. The trait keeps the two apart.
pub trait ActionRunner {
    /// The account type the runtime carries.
    type AccountId;

    /// Take one action request and answer once the request is held or run.
    fn run(request: ActionRequest<'_, Self::AccountId>) -> Result<u64, DispatchError>;
}

/// The runner that records every request in the workflow queue.
///
/// The queue is ordinary Blockchain storage. Cerulea Explorer reads the
/// queue. The logic module drains the queue through
/// [`Pallet::take_due_actions`] and runs each action against the record.
pub struct QueuedActions<T>(PhantomData<T>);

impl<T: Config> ActionRunner for QueuedActions<T> {
    type AccountId = T::AccountId;

    fn run(request: ActionRequest<'_, Self::AccountId>) -> Result<u64, DispatchError> {
        let collection =
            name_of(request.collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let action = name_of(request.action.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        let id = NextDueAction::<T>::get();
        let next = id.checked_add(1).ok_or(Error::<T>::QueueFull)?;
        DueActions::<T>::insert(
            id,
            DueAction {
                deployment: request.deployment,
                collection,
                record: request.record.clone(),
                action,
                actor: request.actor.clone(),
                raised_at: frame_system::Pallet::<T>::block_number(),
            },
        );
        NextDueAction::<T>::put(next);
        Ok(id)
    }
}

impl<T: Config> Pallet<T> {
    /// Take the oldest queued action requests and remove them from the
    /// queue.
    ///
    /// The logic module calls this, runs each action and keeps the outcome.
    /// The call returns at most `limit` requests.
    pub fn take_due_actions(limit: u32) -> Vec<QueuedRequest<T>> {
        let mut taken = Vec::new();
        for (id, held) in DueActions::<T>::iter().take(limit as usize) {
            taken.push((id, held));
        }
        taken.sort_by_key(|(id, _)| *id);
        for (id, _) in taken.iter() {
            DueActions::<T>::remove(id);
        }
        taken
    }

    /// The number of action requests waiting in the queue.
    pub fn due_action_count() -> u32 {
        DueActions::<T>::iter().count() as u32
    }
}

use frame_system::pallet_prelude::BlockNumberFor;
