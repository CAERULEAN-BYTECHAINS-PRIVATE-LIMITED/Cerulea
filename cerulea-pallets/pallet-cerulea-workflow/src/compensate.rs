//! Compensation for a failed multi-step process.
//!
//! A process failing part way runs its compensating steps in reverse
//! order. The last completed step is undone first. The first completed
//! step is undone last.
//!
//! Compensation runs a bounded count of steps per block. The process
//! carries a cursor naming the steps still to undo, and the queue carries
//! the processes still compensating. Nothing is locked across blocks. Every
//! record stays readable throughout, and a reader sees the state the
//! compensation has reached.

use alloc::vec::Vec;
use cerulea_blueprint::{types::StateId, workflows::Workflow};
use codec::Encode;
use frame_support::{pallet_prelude::*, weights::Weight};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_text, BoundedName, DeploymentId};

use crate::{
    ActionRequest, ActionRunner, CompensationQueue, CompensationTarget, Config, Error, Event,
    Instance, InstanceStatus, Instances, Pallet, WeightInfo,
};

impl<T: Config> Pallet<T> {
    /// Start the compensating steps of one process.
    pub(crate) fn begin_compensation(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        actor: &T::AccountId,
        reason: &BoundedVec<u8, ConstU32<256>>,
    ) -> DispatchResult {
        let machine = Self::machine(deployment, workflow)?;
        let mut instance =
            Self::instance(deployment, workflow, id).ok_or(Error::<T>::InstanceUnknown)?;
        ensure!(
            matches!(
                instance.status,
                InstanceStatus::Running | InstanceStatus::Settled
            ),
            Error::<T>::InstanceClosed
        );
        let now = Self::now_millis();
        Self::ensure_write_admitted(deployment, &machine.collection, actor, now)?;

        Self::cancel_schedule(deployment, workflow, id, &mut instance);
        instance.status = InstanceStatus::Compensating;
        instance.cursor = instance.history.len() as u32;
        instance.undone = BoundedVec::default();
        let steps = instance.cursor;
        Instances::<T>::insert((*deployment, workflow.clone(), id), &instance);

        Self::record(
            deployment,
            "compensation_started",
            WorkClass::Routine,
            actor,
            "The process failed part way and started its compensating steps.",
            (workflow.clone(), id, reason.to_vec()).encode(),
        )?;
        Self::deposit_event(Event::CompensationStarted {
            deployment: *deployment,
            workflow: workflow.clone(),
            instance: id,
            steps,
        });

        if steps == 0 {
            Self::finish_compensation(deployment, workflow, id, &mut instance)?;
            Instances::<T>::insert((*deployment, workflow.clone(), id), &instance);
            return Ok(());
        }

        CompensationQueue::<T>::try_mutate(|queue| -> DispatchResult {
            queue
                .try_push(CompensationTarget {
                    deployment: *deployment,
                    workflow: workflow.clone(),
                    instance: id,
                })
                .map_err(|_| Error::<T>::CompensationQueueFull)?;
            Ok(())
        })
    }

    /// Run the compensating steps of this block.
    ///
    /// The block runs at most the declared count of steps. A process with
    /// steps left stays in the queue and continues in the next block.
    pub(crate) fn run_compensation_steps() -> Weight {
        let budget = T::MaxCompensationStepsPerBlock::get();
        if budget == 0 {
            return Weight::zero();
        }
        let queue = CompensationQueue::<T>::get();
        if queue.is_empty() {
            return Weight::zero();
        }

        let mut spent: u32 = 0;
        let mut kept: Vec<CompensationTarget> = Vec::new();
        for target in queue.into_iter() {
            if spent >= budget {
                kept.push(target);
                continue;
            }
            match Self::advance(&target, budget.saturating_sub(spent)) {
                Ok((ran, done)) => {
                    spent = spent.saturating_add(ran);
                    if !done {
                        kept.push(target);
                    }
                }
                Err(_) => {
                    // The process left its compensating state, so the queue
                    // drops it. The stored history keeps the record of the
                    // steps already undone.
                }
            }
        }
        CompensationQueue::<T>::put(BoundedVec::truncate_from(kept));
        T::WeightInfo::compensate_step().saturating_mul(spent as u64)
    }

    /// Undo up to `budget` steps of one process.
    ///
    /// The result carries the number of steps the call undid and whether
    /// the process finished.
    fn advance(target: &CompensationTarget, budget: u32) -> Result<(u32, bool), DispatchError> {
        let machine = Self::machine(&target.deployment, &target.workflow)?;
        let mut instance = Self::instance(&target.deployment, &target.workflow, target.instance)
            .ok_or(Error::<T>::InstanceUnknown)?;
        ensure!(
            instance.status == InstanceStatus::Compensating,
            Error::<T>::InstanceClosed
        );

        let mut ran = 0u32;
        while ran < budget && instance.cursor > 0 {
            Self::undo_one(target, &machine, &mut instance)?;
            ran = ran.saturating_add(1);
        }
        let done = instance.cursor == 0;
        if done {
            Self::finish_compensation(
                &target.deployment,
                &target.workflow,
                target.instance,
                &mut instance,
            )?;
        }
        Instances::<T>::insert(
            (target.deployment, target.workflow.clone(), target.instance),
            &instance,
        );
        Ok((ran, done))
    }

    /// Undo the last step the process has left.
    fn undo_one(
        target: &CompensationTarget,
        machine: &Workflow,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
    ) -> DispatchResult {
        let position = instance.cursor.saturating_sub(1) as usize;
        let step = instance
            .history
            .get(position)
            .cloned()
            .ok_or(Error::<T>::InstanceClosed)?;

        let back = StateId::new(&name_text(&step.from));
        let actor = instance.started_by.clone();
        Self::write_state(&target.deployment, machine, &actor, &instance.record, &back)?;

        // A machine that declares the way back runs the action of that
        // transition as the compensating step.
        let reverse = machine.transitions.iter().find(|held| {
            held.from.as_str() == name_text(&step.to) && held.to.as_str() == name_text(&step.from)
        });
        if let Some(found) = reverse {
            if let Some(action) = &found.action {
                T::Actions::run(ActionRequest {
                    deployment: target.deployment,
                    collection: &machine.collection,
                    record: &instance.record,
                    action,
                    actor: &actor,
                })?;
            }
        }

        instance.state = step.from.clone();
        instance.entered_at = Self::now_millis();
        instance.cursor = instance.cursor.saturating_sub(1);
        instance
            .undone
            .try_push(step.transition.clone())
            .map_err(|_| Error::<T>::HistoryFull)?;

        Self::record(
            &target.deployment,
            "step_compensated",
            WorkClass::Routine,
            &actor,
            "The process undid one completed step.",
            (
                target.workflow.clone(),
                target.instance,
                step.transition.clone(),
                step.from.clone(),
            )
                .encode(),
        )?;
        Self::deposit_event(Event::StepCompensated {
            deployment: target.deployment,
            workflow: target.workflow.clone(),
            instance: target.instance,
            step: step.transition.clone(),
            state: step.from.clone(),
        });
        Ok(())
    }

    /// Close the compensation of one process.
    fn finish_compensation(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
    ) -> DispatchResult {
        instance.status = InstanceStatus::Compensated;
        let actor = instance.started_by.clone();
        Self::record(
            deployment,
            "compensation_completed",
            WorkClass::Routine,
            &actor,
            "Every compensating step of the process ran.",
            (workflow.clone(), id).encode(),
        )?;
        Self::deposit_event(Event::CompensationCompleted {
            deployment: *deployment,
            workflow: workflow.clone(),
            instance: id,
        });
        Ok(())
    }

    /// The steps one process has already undone, in compensation order.
    pub fn undone_steps(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
    ) -> Vec<BoundedName> {
        Self::instance(deployment, workflow, id)
            .map(|held| held.undone.into_inner())
            .unwrap_or_default()
    }

    /// The number of processes waiting for their compensating steps.
    pub fn compensating_count() -> u32 {
        CompensationQueue::<T>::get().len() as u32
    }
}
