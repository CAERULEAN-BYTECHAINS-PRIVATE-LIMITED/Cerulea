//! Timers, deadlines and escalation.
//!
//! A timer fires after a span inside one state. A deadline falls due at a
//! time the machine computes from the record. Both enter one schedule held
//! in due order, so every block takes the oldest work first.
//!
//! One block runs at most the declared count of scheduled steps. Work
//! above that count waits for the next block. The weight of the block
//! charges for the steps the block ran.

use alloc::vec::Vec;
use cerulea_blueprint::{
    types::{StateId, Value},
    workflows::{TimerAction, Workflow},
    Path,
};
use codec::Encode;
use frame_support::{pallet_prelude::*, weights::Weight};
use frame_system::pallet_prelude::BlockNumberFor;
use pallet_cerulea_audit::WorkClass;
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName, DeploymentId};
use sp_runtime::DispatchError;

use crate::{
    ActionRequest, ActionRunner, Config, Error, Event, GuardEngine, Instance, InstanceStatus,
    Instances, Pallet, Schedule, ScheduleKey, ScheduleKind, ScheduledStep, Step, WeightInfo,
};

/// The bucket key one due time takes.
///
/// The key holds the due time in big endian bytes. The schedule stores the
/// key under the identity hasher, so a walk over the schedule returns the
/// steps in due order.
pub fn bucket_of(due_millis: u64) -> [u8; 8] {
    due_millis.to_be_bytes()
}

impl<T: Config> Pallet<T> {
    /// Schedule every timer and every deadline of the present state.
    pub(crate) fn schedule_state(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        machine: &Workflow,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
        now: u64,
    ) -> DispatchResult {
        let state = name_text(&instance.state);
        let record = Self::read_record(deployment, &machine.collection, &instance.record)?;

        for timer in machine
            .timers
            .iter()
            .filter(|held| held.state.as_str() == state)
        {
            let due = now.saturating_add(timer.after_millis);
            let name = name_of(timer.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            Self::place(
                deployment,
                workflow,
                id,
                instance,
                &name,
                ScheduleKind::Timer,
                due,
            )?;
        }

        for deadline in machine
            .deadlines
            .iter()
            .filter(|held| held.state.as_str() == state)
        {
            let env = Self::environment(&instance.started_by, now, &record);
            let path = Path::root().field("deadlines").key(deadline.id.as_str());
            let answer = T::Guards::evaluate(&deadline.due, &env, T::GuardStepLimit::get(), &path)
                .map_err(|_| Error::<T>::GuardFaulted)?;
            let due = match answer {
                Value::Timestamp(held) => held.millis(),
                _ => return Err(Error::<T>::DeadlineInvalid.into()),
            };
            let name = name_of(deadline.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            Self::place(
                deployment,
                workflow,
                id,
                instance,
                &name,
                ScheduleKind::Deadline,
                due,
            )?;
        }
        Ok(())
    }

    /// Put one step into the schedule and note it on the process.
    fn place(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
        timer: &BoundedName,
        kind: ScheduleKind,
        due: u64,
    ) -> DispatchResult {
        Schedule::<T>::insert(
            bucket_of(due),
            ScheduleKey {
                deployment: *deployment,
                workflow: workflow.clone(),
                instance: id,
                timer: timer.clone(),
            },
            ScheduledStep {
                kind,
                state: instance.state.clone(),
                due_millis: due,
            },
        );
        instance
            .scheduled
            .try_push((timer.clone(), due))
            .map_err(|_| Error::<T>::TimerLimitReached)?;
        Ok(())
    }

    /// Remove every scheduled step of one process.
    ///
    /// A record leaving a state drops the timers and the deadlines of that
    /// state. The schedule therefore holds live work alone.
    pub(crate) fn cancel_schedule(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
    ) {
        for (timer, due) in instance.scheduled.iter() {
            Schedule::<T>::remove(
                bucket_of(*due),
                ScheduleKey {
                    deployment: *deployment,
                    workflow: workflow.clone(),
                    instance: id,
                    timer: timer.clone(),
                },
            );
        }
        instance.scheduled = BoundedVec::default();
    }

    /// Run the scheduled steps that fall due in this block.
    pub(crate) fn run_due_steps() -> Weight {
        let now = Self::now_millis();
        let limit = T::MaxScheduledPerBlock::get();
        let mut due: Vec<([u8; 8], ScheduleKey, ScheduledStep)> = Vec::new();
        for (bucket, key, step) in Schedule::<T>::iter() {
            if u64::from_be_bytes(bucket) > now {
                break;
            }
            if due.len() as u32 >= limit {
                break;
            }
            due.push((bucket, key, step));
        }

        let count = due.len() as u64;
        for (bucket, key, step) in due.into_iter() {
            Schedule::<T>::remove(bucket, &key);
            if Self::run_scheduled(&key, &step).is_err() {
                Self::deposit_event(Event::ScheduledStepFailed {
                    deployment: key.deployment,
                    workflow: key.workflow.clone(),
                    instance: key.instance,
                    timer: key.timer.clone(),
                });
            }
        }
        T::WeightInfo::run_scheduled_step().saturating_mul(count)
    }

    /// Fire one scheduled step at once.
    ///
    /// The step runs once its due time has passed. A step due later stays
    /// in the schedule.
    pub(crate) fn force_scheduled(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        timer: &BoundedName,
        kind: ScheduleKind,
    ) -> DispatchResult {
        let instance =
            Self::instance(deployment, workflow, id).ok_or(Error::<T>::InstanceUnknown)?;
        let due = instance
            .scheduled
            .iter()
            .find(|(name, _)| name == timer)
            .map(|(_, due)| *due)
            .ok_or(Error::<T>::TimerUnknown)?;
        ensure!(due <= Self::now_millis(), Error::<T>::TimerPending);

        let key = ScheduleKey {
            deployment: *deployment,
            workflow: workflow.clone(),
            instance: id,
            timer: timer.clone(),
        };
        let step = Schedule::<T>::get(bucket_of(due), &key).ok_or(Error::<T>::TimerUnknown)?;
        ensure!(step.kind == kind, Error::<T>::TimerUnknown);
        Schedule::<T>::remove(bucket_of(due), &key);
        Self::run_scheduled(&key, &step)
    }

    /// Run one scheduled step.
    pub(crate) fn run_scheduled(key: &ScheduleKey, step: &ScheduledStep) -> DispatchResult {
        let machine = Self::machine(&key.deployment, &key.workflow)?;
        let mut instance = Self::instance(&key.deployment, &key.workflow, key.instance)
            .ok_or(Error::<T>::InstanceUnknown)?;
        ensure!(
            instance.status == InstanceStatus::Running,
            Error::<T>::InstanceClosed
        );
        ensure!(instance.state == step.state, Error::<T>::TimerUnknown);

        let wanted = name_text(&key.timer);
        let action = match step.kind {
            ScheduleKind::Timer => machine
                .timers
                .iter()
                .find(|held| held.id.as_str() == wanted)
                .map(|held| held.on_expiry.clone())
                .ok_or(Error::<T>::TimerUnknown)?,
            ScheduleKind::Deadline => machine
                .deadlines
                .iter()
                .find(|held| held.id.as_str() == wanted)
                .map(|held| held.on_miss.clone())
                .ok_or(Error::<T>::TimerUnknown)?,
        };

        instance.scheduled.retain(|(name, _)| name != &key.timer);
        let actor = instance.started_by.clone();
        let now = Self::now_millis();

        match &action {
            TimerAction::MoveTo(target) => {
                Self::move_by_schedule(key, &machine, &mut instance, target, now)?;
            }
            TimerAction::Run(named) => {
                T::Actions::run(ActionRequest {
                    deployment: key.deployment,
                    collection: &machine.collection,
                    record: &instance.record,
                    action: named,
                    actor: &actor,
                })?;
                Instances::<T>::insert(
                    (key.deployment, key.workflow.clone(), key.instance),
                    &instance,
                );
            }
            TimerAction::Emit(named) => {
                Self::record(
                    &key.deployment,
                    named.as_str(),
                    WorkClass::Routine,
                    &actor,
                    "A workflow timer recorded one event.",
                    (key.workflow.clone(), key.instance).encode(),
                )?;
                Instances::<T>::insert(
                    (key.deployment, key.workflow.clone(), key.instance),
                    &instance,
                );
            }
        }

        match step.kind {
            ScheduleKind::Timer => {
                Self::record(
                    &key.deployment,
                    "timer_fired",
                    WorkClass::Routine,
                    &actor,
                    "A workflow timer fired after its span in the state.",
                    (key.workflow.clone(), key.instance, key.timer.clone()).encode(),
                )?;
                Self::deposit_event(Event::TimerFired {
                    deployment: key.deployment,
                    workflow: key.workflow.clone(),
                    instance: key.instance,
                    timer: key.timer.clone(),
                });
            }
            ScheduleKind::Deadline => {
                Self::record(
                    &key.deployment,
                    "deadline_escalated",
                    WorkClass::Routine,
                    &actor,
                    "A workflow deadline passed and the escalation ran.",
                    (key.workflow.clone(), key.instance, key.timer.clone()).encode(),
                )?;
                Self::deposit_event(Event::DeadlineEscalated {
                    deployment: key.deployment,
                    workflow: key.workflow.clone(),
                    instance: key.instance,
                    deadline: key.timer.clone(),
                });
            }
        }
        Ok(())
    }

    /// Move one record because a timer or a deadline asked for the move.
    ///
    /// The timer carries the authority of the machine, so the move runs
    /// without a guard. The step joins the history, so compensation undoes
    /// the move in its turn.
    fn move_by_schedule(
        key: &ScheduleKey,
        machine: &Workflow,
        instance: &mut Instance<
            T::AccountId,
            BlockNumberFor<T>,
            T::MaxStepsPerInstance,
            T::MaxTimersPerState,
        >,
        target: &StateId,
        now: u64,
    ) -> DispatchResult {
        let actor = instance.started_by.clone();
        Self::cancel_schedule(&key.deployment, &key.workflow, key.instance, instance);
        Self::write_state(&key.deployment, machine, &actor, &instance.record, target)?;

        let landed = name_of(target.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        instance
            .history
            .try_push(Step {
                transition: key.timer.clone(),
                from: instance.state.clone(),
                to: landed.clone(),
                at_millis: now,
                actor: actor.clone(),
            })
            .map_err(|_| Error::<T>::HistoryFull)?;
        instance.state = landed.clone();
        instance.entered_at = now;

        if Self::is_final(machine, target) {
            instance.status = InstanceStatus::Settled;
            Self::deposit_event(Event::InstanceSettled {
                deployment: key.deployment,
                workflow: key.workflow.clone(),
                instance: key.instance,
                state: landed,
            });
        } else {
            Self::schedule_state(
                &key.deployment,
                &key.workflow,
                key.instance,
                machine,
                instance,
                now,
            )?;
        }
        Instances::<T>::insert(
            (key.deployment, key.workflow.clone(), key.instance),
            &*instance,
        );
        Ok(())
    }

    /// The number of steps the schedule holds.
    pub fn scheduled_count() -> u32 {
        Schedule::<T>::iter().count() as u32
    }

    /// The due time of one scheduled step.
    pub fn scheduled_due(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        instance: u64,
        timer: &BoundedName,
    ) -> Option<u64> {
        Self::instance(deployment, workflow, instance).and_then(|held| {
            held.scheduled
                .iter()
                .find(|(name, _)| name == timer)
                .map(|(_, due)| *due)
        })
    }
}

/// The error a scheduled step reports when it names no known machine.
pub fn missing_machine<T: Config>() -> DispatchError {
    Error::<T>::MachineUnknown.into()
}
