//! The Cerulea workflow module.
//!
//! The workflow module drives the long-running processes of a deployment.
//! A machine definition comes from the Workflows section of the Blueprint.
//! The definition names the states a record passes through, the guarded
//! transitions between them, the timers that fire after a span in a state
//! and the deadlines that fall due at a computed time.
//!
//! Five operations drive a process. Start instance. Attempt transition.
//! Fire timer. Escalate deadline. Compensate.
//!
//! The state of a record lives in one field of the record. The field
//! carries an enumeration, so every state name sits in the type system. A
//! transition writes the new state into the record through the state
//! module, so one store holds the record and its state together.
//!
//! A guard is a Blueprint expression carrying a truth value. Every guard
//! runs through the metered expression evaluator. A guard answering false
//! leaves the record untouched and records the reason.
//!
//! A multi-step process failing part way runs its compensating steps in
//! reverse order. Compensation runs a bounded number of steps per block,
//! so nothing stays locked across blocks.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod compensate;
pub mod engine;
pub mod timer;
pub mod validate;
pub mod weights;

#[cfg(test)]
mod machine_tests;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use engine::{ActionRequest, ActionRunner, GuardEngine, MeteredGuards, QueuedActions};
pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{string::String, vec::Vec};
use cerulea_blueprint::{
    expression::Env,
    types::{
        CollectionId, FieldPath, Identity as ActorIdentity, StateId, Timestamp, Value, VariantName,
        WorkflowId,
    },
    workflows::{Timer, TimerAction, Workflow},
    Path,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_identity::IdentityRegistry;
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest, PermissionGate},
    primitives::{name_of, name_text, BoundedName, DeploymentId, RecordKey},
    registry::SchemaRegistry,
};
use pallet_cerulea_state::RecordStore;

/// The name a guard and a deadline bind the record to.
pub const RECORD_BINDING: &str = cerulea_blueprint::permissions::ROW_BINDING;

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "workflow";

/// Where one process stands.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum InstanceStatus {
    /// The process runs and takes further transitions.
    Running,
    /// The record reached a final state.
    Settled,
    /// The process failed and runs its compensating steps.
    Compensating,
    /// Every compensating step ran.
    Compensated,
}

/// One completed step of a process.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct Step<AccountId> {
    /// The transition the step took, or the timer that moved the record.
    pub transition: BoundedName,
    /// The state the record left.
    pub from: BoundedName,
    /// The state the record entered.
    pub to: BoundedName,
    /// The wall clock time of the step, in milliseconds.
    pub at_millis: u64,
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
}

/// One running process.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxSteps, MaxTimers))]
pub struct Instance<AccountId, BlockNumber, MaxSteps, MaxTimers>
where
    MaxSteps: Get<u32> + 'static,
    MaxTimers: Get<u32> + 'static,
{
    /// The record the process drives.
    pub record: RecordKey,
    /// The state the record holds now.
    pub state: BoundedName,
    /// Where the process stands.
    pub status: InstanceStatus,
    /// The wall clock time the record entered its present state.
    pub entered_at: u64,
    /// The wall clock time the process started.
    pub started_at: u64,
    /// The account that started the process.
    pub started_by: AccountId,
    /// The block the process started in.
    pub started_block: BlockNumber,
    /// Every completed step, in the order the steps ran.
    pub history: BoundedVec<Step<AccountId>, MaxSteps>,
    /// Every step already undone, in the order compensation ran them.
    pub undone: BoundedVec<BoundedName, MaxSteps>,
    /// The timers and deadlines scheduled for the present state, with the
    /// wall clock time each falls due at.
    pub scheduled: BoundedVec<(BoundedName, u64), MaxTimers>,
    /// The number of steps compensation still has to undo.
    pub cursor: u32,
}

/// Which kind of scheduled step falls due.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum ScheduleKind {
    /// A timer firing after a span inside one state.
    Timer,
    /// A deadline falling due at a computed time.
    Deadline,
}

/// The key of one scheduled step.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct ScheduleKey {
    /// The deployment.
    pub deployment: DeploymentId,
    /// The workflow.
    pub workflow: BoundedName,
    /// The process.
    pub instance: u64,
    /// The timer or the deadline.
    pub timer: BoundedName,
}

/// One scheduled step.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct ScheduledStep {
    /// Which kind of step falls due.
    pub kind: ScheduleKind,
    /// The state the step watches.
    pub state: BoundedName,
    /// The wall clock time the step falls due at, in milliseconds.
    pub due_millis: u64,
}

/// One process waiting for its compensating steps.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct CompensationTarget {
    /// The deployment.
    pub deployment: DeploymentId,
    /// The workflow.
    pub workflow: BoundedName,
    /// The process.
    pub instance: u64,
}

/// One action request a timer raised.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct DueAction<AccountId, BlockNumber> {
    /// The deployment.
    pub deployment: DeploymentId,
    /// The collection holding the record.
    pub collection: BoundedName,
    /// The record key.
    pub record: RecordKey,
    /// The action the workflow asks for.
    pub action: BoundedName,
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
    /// The block the request landed in.
    pub raised_at: BlockNumber,
}

/// The plain statement of one refused transition.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct Refusal {
    /// The workflow.
    pub workflow: BoundedName,
    /// The process.
    pub instance: u64,
    /// The transition the caller asked for.
    pub transition: BoundedName,
    /// One sentence stating the problem.
    pub problem: BoundedVec<u8, ConstU32<256>>,
    /// One sentence stating the fix.
    pub fix: BoundedVec<u8, ConstU32<256>>,
}

impl Refusal {
    /// The problem sentence as text.
    pub fn problem_text(&self) -> String {
        String::from_utf8_lossy(self.problem.as_slice()).into_owned()
    }

    /// The fix sentence as text.
    pub fn fix_text(&self) -> String {
        String::from_utf8_lossy(self.fix.as_slice()).into_owned()
    }
}

/// One running process, carrying the bounds the runtime declares.
pub type ProcessOf<T> = Instance<
    <T as frame_system::Config>::AccountId,
    BlockNumberFor<T>,
    <T as Config>::MaxStepsPerInstance,
    <T as Config>::MaxTimersPerState,
>;

/// The read a dependant module performs over the running processes.
pub trait ProcessView {
    /// The state one process holds now.
    fn instance_state(
        deployment: &DeploymentId,
        workflow: &WorkflowId,
        instance: u64,
    ) -> Option<StateId>;

    /// Report whether one process reached a final state.
    fn is_settled(deployment: &DeploymentId, workflow: &WorkflowId, instance: u64) -> bool;

    /// The number of completed steps one process holds.
    fn step_count(deployment: &DeploymentId, workflow: &WorkflowId, instance: u64) -> u32;
}

/// Read one field of a record value.
///
/// The path walks nested records. A path reaching past a value that holds
/// no fields answers with nothing.
pub fn value_at(record: &Value, path: &FieldPath) -> Option<Value> {
    let mut current = record.clone();
    for step in path.0.iter() {
        current = match current {
            Value::Record(held) => held.field(step)?.clone(),
            Value::Optional(held) => match held.item {
                Some(inner) => match *inner {
                    Value::Record(nested) => nested.field(step)?.clone(),
                    _ => return None,
                },
                None => return None,
            },
            _ => return None,
        };
    }
    Some(current)
}

/// Write one field of a record value.
///
/// The path walks nested records. The call answers true once the field
/// took the new value. A path reaching past a value that holds no fields
/// answers false and leaves the record untouched.
pub fn set_value_at(record: &mut Value, path: &FieldPath, new: Value) -> bool {
    let mut steps = path.0.iter();
    let head = match steps.next() {
        Some(found) => found,
        None => return false,
    };
    let rest = FieldPath(steps.cloned().collect());
    let holder = match record {
        Value::Record(held) => held,
        Value::Optional(held) => match held.item.as_mut() {
            Some(inner) => match inner.as_mut() {
                Value::Record(nested) => nested,
                _ => return false,
            },
            None => return false,
        },
        _ => return false,
    };
    for (name, slot) in holder.fields.iter_mut() {
        if name == head {
            if rest.is_empty() {
                *slot = new;
                return true;
            }
            return set_value_at(slot, &rest, new);
        }
    }
    false
}

/// The state name one record carries, with the enumeration it belongs to.
pub fn state_of(
    record: &Value,
    field: &FieldPath,
) -> Option<(cerulea_blueprint::types::TypeId, VariantName)> {
    match value_at(record, field)? {
        Value::Enumeration(held) => Some((held.type_id, held.variant)),
        _ => None,
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the workflow module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry the module reads collection definitions from.
        type Schema: SchemaRegistry;

        /// The store holding the records the processes drive.
        type Records: RecordStore<AccountId = Self::AccountId>;

        /// The registry the module reads roles and identities from.
        type Identities: IdentityRegistry<AccountId = Self::AccountId>;

        /// The resolver the module asks before it changes a record.
        type Permissions: PermissionGate<AccountId = Self::AccountId>;

        /// The evaluator every guard and every deadline runs through.
        type Guards: GuardEngine;

        /// The runner every timer action reaches.
        type Actions: ActionRunner<AccountId = Self::AccountId>;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every timer and every deadline reads.
        type Time: UnixTime;

        /// The largest machine definition, in bytes.
        #[pallet::constant]
        type MaxMachineBytes: Get<u32>;

        /// The largest number of machines one deployment holds.
        #[pallet::constant]
        type MaxMachinesPerDeployment: Get<u32>;

        /// The largest number of completed steps one process holds.
        #[pallet::constant]
        type MaxStepsPerInstance: Get<u32>;

        /// The largest number of timers and deadlines one state carries.
        #[pallet::constant]
        type MaxTimersPerState: Get<u32>;

        /// The largest number of scheduled steps one block runs.
        #[pallet::constant]
        type MaxScheduledPerBlock: Get<u32>;

        /// The largest number of compensating steps one block runs.
        #[pallet::constant]
        type MaxCompensationStepsPerBlock: Get<u32>;

        /// The largest number of processes waiting for compensation.
        #[pallet::constant]
        type MaxCompensatingInstances: Get<u32>;

        /// The step budget of one guard evaluation.
        #[pallet::constant]
        type GuardStepLimit: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every installed machine definition.
    #[pallet::storage]
    pub type Machines<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        BoundedVec<u8, T::MaxMachineBytes>,
        OptionQuery,
    >;

    /// The installed machine names of every deployment.
    #[pallet::storage]
    pub type MachineNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxMachinesPerDeployment>,
        ValueQuery,
    >;

    /// Every running process.
    #[pallet::storage]
    pub type Instances<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, BoundedName>,
            NMapKey<Twox64Concat, u64>,
        ),
        Instance<T::AccountId, BlockNumberFor<T>, T::MaxStepsPerInstance, T::MaxTimersPerState>,
        OptionQuery,
    >;

    /// The identifier the next process of every machine takes.
    #[pallet::storage]
    pub type NextInstance<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        u64,
        ValueQuery,
    >;

    /// Every scheduled timer and every scheduled deadline.
    ///
    /// The first key is the wall clock due time in big endian bytes, held
    /// under the identity hasher. A walk over the map therefore returns the
    /// steps in due order, so one block takes the oldest work first.
    #[pallet::storage]
    pub type Schedule<T: Config> = StorageDoubleMap<
        _,
        Identity,
        [u8; 8],
        Blake2_128Concat,
        ScheduleKey,
        ScheduledStep,
        OptionQuery,
    >;

    /// The processes waiting for their compensating steps.
    #[pallet::storage]
    pub type CompensationQueue<T: Config> =
        StorageValue<_, BoundedVec<CompensationTarget, T::MaxCompensatingInstances>, ValueQuery>;

    /// The action requests the timers raised.
    #[pallet::storage]
    pub type DueActions<T: Config> =
        StorageMap<_, Twox64Concat, u64, DueAction<T::AccountId, BlockNumberFor<T>>, OptionQuery>;

    /// The identifier the next action request takes.
    #[pallet::storage]
    pub type NextDueAction<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A machine definition entered the registry.
        MachineInstalled {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The number of states the machine declares.
            states: u32,
        },
        /// A machine definition left the registry.
        MachineRetired {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
        },
        /// A process started.
        InstanceStarted {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The state the record takes.
            state: BoundedName,
        },
        /// A record moved from one state to another.
        TransitionTaken {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The transition.
            transition: BoundedName,
            /// The state the record entered.
            state: BoundedName,
        },
        /// A guard answered false and the record stayed where it was.
        TransitionRefused {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The transition.
            transition: BoundedName,
        },
        /// A process reached a final state.
        InstanceSettled {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The final state.
            state: BoundedName,
        },
        /// A timer fired.
        TimerFired {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The timer.
            timer: BoundedName,
        },
        /// A deadline passed and the escalation ran.
        DeadlineEscalated {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The deadline.
            deadline: BoundedName,
        },
        /// A scheduled step failed and left the schedule.
        ScheduledStepFailed {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The timer or the deadline.
            timer: BoundedName,
        },
        /// A process started its compensating steps.
        CompensationStarted {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The number of steps compensation undoes.
            steps: u32,
        },
        /// One compensating step ran.
        StepCompensated {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
            /// The step the compensation undid.
            step: BoundedName,
            /// The state the record returned to.
            state: BoundedName,
        },
        /// Every compensating step ran.
        CompensationCompleted {
            /// The deployment.
            deployment: DeploymentId,
            /// The workflow.
            workflow: BoundedName,
            /// The process.
            instance: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the workflow name.
        NameTooLong,
        /// The definition failed to decode. Submit the canonical encoding
        /// the Studio produces.
        MachineUnreadable,
        /// The definition carries a problem. Read the machine report and
        /// correct the definition in the Studio.
        MachineInvalid,
        /// The registry lacks that machine. Install the machine before
        /// starting a process.
        MachineUnknown,
        /// The deployment holds its full count of machines. Retire a
        /// machine, or raise the limit in the runtime.
        MachineLimitReached,
        /// The registry already holds a machine under that name. Retire the
        /// machine before installing another under the same name.
        MachinePresent,
        /// The registry lacks the collection the machine names. Register
        /// the collection before installing the machine.
        CollectionUnknown,
        /// The store holds no record under that key. Insert the record
        /// before starting a process over it.
        RecordAbsent,
        /// The record carries no enumeration at the declared state field.
        /// Point the state field at a field carrying an enumeration.
        StateFieldInvalid,
        /// The module holds no process under that identifier. Start a
        /// process before driving it.
        InstanceUnknown,
        /// The process left its running state. Start a new process over the
        /// record.
        InstanceClosed,
        /// The machine declares no transition of that name out of the
        /// present state. Name a transition that leaves the present state.
        TransitionUnavailable,
        /// The caller holds none of the roles the transition names. Ask a
        /// holder of one of those roles to take the transition.
        RoleMissing,
        /// The guard answered false. Call the explain entry point for the
        /// problem and the fix in plain language.
        GuardRefused,
        /// The guard failed to evaluate. Correct the guard expression in
        /// the Studio.
        GuardFaulted,
        /// The permission module refused the change. Grant the role the
        /// capability over the collection the machine drives.
        PermissionDenied,
        /// The process holds its full count of steps. Raise the step limit
        /// in the runtime.
        HistoryFull,
        /// The state carries more timers and deadlines than the runtime
        /// admits. Raise the timer limit in the runtime.
        TimerLimitReached,
        /// The machine declares no timer of that name. Name a timer the
        /// machine declares.
        TimerUnknown,
        /// The timer falls due later. Wait for the due time, or let the
        /// block schedule fire the timer.
        TimerPending,
        /// The queue of waiting processes is full. Wait for the running
        /// compensations to finish.
        CompensationQueueFull,
        /// The action queue reached its last identifier. Drain the queue
        /// through the logic module.
        QueueFull,
        /// The deadline expression answered with a value other than a
        /// timestamp. Give the deadline a timestamp.
        DeadlineInvalid,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        /// Run the scheduled work of one block.
        ///
        /// The block runs at most the declared count of scheduled steps and
        /// at most the declared count of compensating steps. Work above
        /// those counts waits for the next block, so one block never runs
        /// without bound.
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            let mut spent = T::WeightInfo::on_initialize_idle();
            spent = spent.saturating_add(Self::run_due_steps());
            spent = spent.saturating_add(Self::run_compensation_steps());
            spent
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Install one machine definition.
        ///
        /// The definition comes from the Workflows section of the
        /// Blueprint. The call checks the definition against the registered
        /// types and the registered collections before storing it.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::install_machine())]
        pub fn install_machine(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            definition: BoundedVec<u8, T::MaxMachineBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let machine = Workflow::decode(&mut definition.as_slice())
                .map_err(|_| Error::<T>::MachineUnreadable)?;
            let name = name_of(machine.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            ensure!(
                Machines::<T>::get(deployment, &name).is_none(),
                Error::<T>::MachinePresent
            );
            validate::check_machine::<T>(&deployment, &machine)?;

            MachineNames::<T>::try_mutate(deployment, |names| -> DispatchResult {
                names
                    .try_push(name.clone())
                    .map_err(|_| Error::<T>::MachineLimitReached)?;
                Ok(())
            })?;
            Machines::<T>::insert(deployment, &name, &definition);

            Self::record(
                &deployment,
                "machine_installed",
                WorkClass::Structural,
                &who,
                "The steward installed one workflow machine.",
                name.to_vec(),
            )?;
            Self::deposit_event(Event::MachineInstalled {
                deployment,
                workflow: name,
                states: machine.states.len() as u32,
            });
            Ok(())
        }

        /// Remove one machine definition.
        ///
        /// Every running process keeps its stored history. A retired
        /// machine takes no further process.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::retire_machine())]
        pub fn retire_machine(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(
                Machines::<T>::get(deployment, &workflow).is_some(),
                Error::<T>::MachineUnknown
            );
            Machines::<T>::remove(deployment, &workflow);
            MachineNames::<T>::mutate(deployment, |names| {
                names.retain(|held| held != &workflow);
            });
            Self::record(
                &deployment,
                "machine_retired",
                WorkClass::Structural,
                &who,
                "The steward retired one workflow machine.",
                workflow.to_vec(),
            )?;
            Self::deposit_event(Event::MachineRetired {
                deployment,
                workflow,
            });
            Ok(())
        }

        /// Start one process over one record.
        ///
        /// The record takes the initial state the machine declares. Every
        /// timer and every deadline of that state enters the schedule.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::start_instance())]
        pub fn start_instance(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
            record: RecordKey,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let machine = Self::machine(&deployment, &workflow)?;
            let now = Self::now_millis();
            Self::ensure_write_admitted(&deployment, &machine.collection, &who, now)?;

            let initial = name_of(machine.initial.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            Self::write_state(&deployment, &machine, &who, &record, &machine.initial)?;

            let id = NextInstance::<T>::get(deployment, &workflow);
            let mut instance = Instance {
                record: record.clone(),
                state: initial.clone(),
                status: InstanceStatus::Running,
                entered_at: now,
                started_at: now,
                started_by: who.clone(),
                started_block: frame_system::Pallet::<T>::block_number(),
                history: BoundedVec::default(),
                undone: BoundedVec::default(),
                scheduled: BoundedVec::default(),
                cursor: 0,
            };
            Self::schedule_state(&deployment, &workflow, id, &machine, &mut instance, now)?;
            if Self::is_final(&machine, &machine.initial) {
                instance.status = InstanceStatus::Settled;
            }
            Instances::<T>::insert((deployment, workflow.clone(), id), &instance);
            NextInstance::<T>::insert(deployment, &workflow, id.saturating_add(1));

            Self::record(
                &deployment,
                "instance_started",
                WorkClass::Routine,
                &who,
                "The caller started one workflow process.",
                (workflow.clone(), id, initial.clone()).encode(),
            )?;
            Self::deposit_event(Event::InstanceStarted {
                deployment,
                workflow,
                instance: id,
                state: initial,
            });
            Ok(())
        }

        /// Attempt one transition of one process.
        ///
        /// The call checks the role the transition names, resolves
        /// permission and evaluates the guard. A guard answering false
        /// leaves the record untouched and records the reason.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::attempt_transition())]
        pub fn attempt_transition(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
            instance: u64,
            transition: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::take_transition(&deployment, &workflow, instance, &transition, &who)
        }

        /// Fire one timer that already falls due.
        ///
        /// The block schedule fires every due timer. This call fires one
        /// due timer at once.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::fire_timer())]
        pub fn fire_timer(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
            instance: u64,
            timer: BoundedName,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Self::force_scheduled(
                &deployment,
                &workflow,
                instance,
                &timer,
                ScheduleKind::Timer,
            )
        }

        /// Escalate one deadline that already passed.
        ///
        /// The block schedule escalates every passed deadline. This call
        /// escalates one passed deadline at once.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::escalate_deadline())]
        pub fn escalate_deadline(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
            instance: u64,
            deadline: BoundedName,
        ) -> DispatchResult {
            ensure_signed(origin)?;
            Self::force_scheduled(
                &deployment,
                &workflow,
                instance,
                &deadline,
                ScheduleKind::Deadline,
            )
        }

        /// Start the compensating steps of one failed process.
        ///
        /// The process stops taking transitions. Every completed step runs
        /// its compensating step in reverse order, a bounded count of steps
        /// per block.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::compensate())]
        pub fn compensate(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            workflow: BoundedName,
            instance: u64,
            reason: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::begin_compensation(&deployment, &workflow, instance, &who, &reason)
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The wall clock time of the Blockchain, in milliseconds.
    pub fn now_millis() -> u64 {
        T::Time::now().as_millis() as u64
    }

    /// One installed machine definition.
    pub fn machine(
        deployment: &DeploymentId,
        workflow: &BoundedName,
    ) -> Result<Workflow, DispatchError> {
        let bytes = Machines::<T>::get(deployment, workflow).ok_or(Error::<T>::MachineUnknown)?;
        Workflow::decode(&mut bytes.as_slice()).map_err(|_| Error::<T>::MachineUnreadable.into())
    }

    /// One running process.
    pub fn instance(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
    ) -> Option<ProcessOf<T>> {
        Instances::<T>::get((*deployment, workflow.clone(), id))
    }

    /// Report whether one state is a final state of the machine.
    pub fn is_final(machine: &Workflow, state: &StateId) -> bool {
        machine
            .states
            .iter()
            .any(|held| &held.id == state && held.final_state)
    }

    /// Append one record to the audit log.
    pub fn record(
        deployment: &DeploymentId,
        event: &str,
        class: WorkClass,
        actor: &T::AccountId,
        reason: &str,
        payload: Vec<u8>,
    ) -> Result<u64, DispatchError> {
        T::Audit::record(
            AuditRequest::new(*deployment, MODULE_NAME, event, actor, reason)
                .classed(class)
                .carrying(payload),
        )
    }

    /// Ask the permission module whether the caller changes the records of
    /// one collection.
    pub fn ensure_write_admitted(
        deployment: &DeploymentId,
        collection: &CollectionId,
        actor: &T::AccountId,
        now: u64,
    ) -> DispatchResult {
        let request = AccessRequest::new(
            *deployment,
            actor,
            collection.clone(),
            AccessAction::Update,
            Timestamp::from_millis(now),
        );
        T::Permissions::resolve(&request).map_err(|_| Error::<T>::PermissionDenied)?;
        Ok(())
    }

    /// Read the record one process drives.
    pub fn read_record(
        deployment: &DeploymentId,
        collection: &CollectionId,
        key: &RecordKey,
    ) -> Result<Value, DispatchError> {
        T::Records::read(deployment, collection, key).ok_or_else(|| Error::<T>::RecordAbsent.into())
    }

    /// Write one state name into the state field of one record.
    pub fn write_state(
        deployment: &DeploymentId,
        machine: &Workflow,
        actor: &T::AccountId,
        key: &RecordKey,
        state: &StateId,
    ) -> DispatchResult {
        let mut record = Self::read_record(deployment, &machine.collection, key)?;
        let (type_id, _) =
            state_of(&record, &machine.state_field).ok_or(Error::<T>::StateFieldInvalid)?;
        let placed = set_value_at(
            &mut record,
            &machine.state_field,
            Value::enumeration(type_id, VariantName::new(state.as_str())),
        );
        ensure!(placed, Error::<T>::StateFieldInvalid);
        record.canonicalise();
        T::Records::update(deployment, &machine.collection, actor, key, &record)
    }

    /// Build the evaluation environment of one guard or one deadline.
    pub fn environment(actor: &T::AccountId, now: u64, record: &Value) -> Env<'static> {
        let identity = T::Identities::identity_of(actor)
            .map(ActorIdentity)
            .unwrap_or_default();
        let mut env = Env::new(identity, Timestamp::from_millis(now));
        env.bind(
            &cerulea_blueprint::types::ParamName::new(RECORD_BINDING),
            record.clone(),
        );
        env
    }

    /// Take one transition of one process.
    pub fn take_transition(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        transition: &BoundedName,
        actor: &T::AccountId,
    ) -> DispatchResult {
        let machine = Self::machine(deployment, workflow)?;
        let mut instance =
            Self::instance(deployment, workflow, id).ok_or(Error::<T>::InstanceUnknown)?;
        ensure!(
            instance.status == InstanceStatus::Running,
            Error::<T>::InstanceClosed
        );

        let wanted = name_text(transition);
        let from = name_text(&instance.state);
        let step = machine
            .transitions
            .iter()
            .find(|held| held.id.as_str() == wanted && held.from.as_str() == from)
            .cloned()
            .ok_or(Error::<T>::TransitionUnavailable)?;

        let now = Self::now_millis();
        if !step.roles.is_empty() {
            let held = step.roles.iter().any(|role| {
                name_of(role.as_str())
                    .map(|name| T::Identities::has_role(deployment, actor, &name, now))
                    .unwrap_or(false)
            });
            ensure!(held, Error::<T>::RoleMissing);
        }
        Self::ensure_write_admitted(deployment, &machine.collection, actor, now)?;

        let record = Self::read_record(deployment, &machine.collection, &instance.record)?;
        if let Some(guard) = &step.guard {
            let env = Self::environment(actor, now, &record);
            let path = Path::root().field("workflows").key(machine.id.as_str());
            let answer = T::Guards::evaluate(guard, &env, T::GuardStepLimit::get(), &path);
            match answer {
                Ok(Value::Bool(true)) => {}
                Ok(Value::Bool(false)) => {
                    Self::deposit_event(Event::TransitionRefused {
                        deployment: *deployment,
                        workflow: workflow.clone(),
                        instance: id,
                        transition: transition.clone(),
                    });
                    return Err(Error::<T>::GuardRefused.into());
                }
                Ok(_) => return Err(Error::<T>::GuardFaulted.into()),
                Err(_) => return Err(Error::<T>::GuardFaulted.into()),
            }
        }

        Self::cancel_schedule(deployment, workflow, id, &mut instance);
        Self::write_state(deployment, &machine, actor, &instance.record, &step.to)?;

        let target = name_of(step.to.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
        instance
            .history
            .try_push(Step {
                transition: transition.clone(),
                from: instance.state.clone(),
                to: target.clone(),
                at_millis: now,
                actor: actor.clone(),
            })
            .map_err(|_| Error::<T>::HistoryFull)?;
        instance.state = target.clone();
        instance.entered_at = now;

        if let Some(action) = &step.action {
            T::Actions::run(ActionRequest {
                deployment: *deployment,
                collection: &machine.collection,
                record: &instance.record,
                action,
                actor,
            })?;
        }

        let settled = Self::is_final(&machine, &step.to);
        if settled {
            instance.status = InstanceStatus::Settled;
        } else {
            Self::schedule_state(deployment, workflow, id, &machine, &mut instance, now)?;
        }
        Instances::<T>::insert((*deployment, workflow.clone(), id), &instance);

        Self::record(
            deployment,
            "transition_taken",
            WorkClass::Routine,
            actor,
            "The caller moved one record to its next state.",
            (workflow.clone(), id, transition.clone(), target.clone()).encode(),
        )?;
        Self::deposit_event(Event::TransitionTaken {
            deployment: *deployment,
            workflow: workflow.clone(),
            instance: id,
            transition: transition.clone(),
            state: target.clone(),
        });
        if settled {
            Self::deposit_event(Event::InstanceSettled {
                deployment: *deployment,
                workflow: workflow.clone(),
                instance: id,
                state: target,
            });
        }
        Ok(())
    }

    /// State in plain language why one transition would be refused.
    ///
    /// The call changes nothing. The interface layer runs it after a
    /// refusal and shows the problem and the fix. A transition that would
    /// run answers with nothing.
    pub fn explain_refusal(
        deployment: &DeploymentId,
        workflow: &BoundedName,
        id: u64,
        transition: &BoundedName,
        actor: &T::AccountId,
    ) -> Option<Refusal> {
        let machine = Self::machine(deployment, workflow).ok()?;
        let instance = Self::instance(deployment, workflow, id)?;
        let statement = |problem: &str, fix: &str| Refusal {
            workflow: workflow.clone(),
            instance: id,
            transition: transition.clone(),
            problem: BoundedVec::truncate_from(problem.as_bytes().to_vec()),
            fix: BoundedVec::truncate_from(fix.as_bytes().to_vec()),
        };

        if instance.status != InstanceStatus::Running {
            return Some(statement(
                "The process left its running state.",
                "Start a new process over the record.",
            ));
        }

        let wanted = name_text(transition);
        let from = name_text(&instance.state);
        let step = match machine
            .transitions
            .iter()
            .find(|held| held.id.as_str() == wanted && held.from.as_str() == from)
        {
            Some(found) => found,
            None => {
                return Some(statement(
                    "The present state offers no transition of that name.",
                    "Name a transition that leaves the present state.",
                ))
            }
        };

        let now = Self::now_millis();
        if !step.roles.is_empty() {
            let carried = step.roles.iter().any(|role| {
                name_of(role.as_str())
                    .map(|name| T::Identities::has_role(deployment, actor, &name, now))
                    .unwrap_or(false)
            });
            if !carried {
                return Some(statement(
                    "The caller holds none of the roles the transition names.",
                    "Ask a holder of one of those roles to take the transition.",
                ));
            }
        }

        let record = Self::read_record(deployment, &machine.collection, &instance.record).ok()?;
        let guard = step.guard.as_ref()?;
        let env = Self::environment(actor, now, &record);
        let path = Path::root().field("workflows").key(machine.id.as_str());
        match T::Guards::evaluate(guard, &env, T::GuardStepLimit::get(), &path) {
            Ok(Value::Bool(true)) => None,
            Ok(Value::Bool(false)) => Some(statement(
                "The guard of the transition answered false.",
                "Meet the condition the transition names, then take the transition again.",
            )),
            Ok(_) => Some(statement(
                "The guard of the transition answered with a value other than a truth value.",
                "Give the guard a truth value in the Studio.",
            )),
            Err(diagnostic) => Some(statement(
                diagnostic.problem.as_str(),
                diagnostic.fix.as_str(),
            )),
        }
    }
}

impl<T: Config> ProcessView for Pallet<T> {
    fn instance_state(
        deployment: &DeploymentId,
        workflow: &WorkflowId,
        instance: u64,
    ) -> Option<StateId> {
        let name = name_of(workflow.as_str()).ok()?;
        Self::instance(deployment, &name, instance)
            .map(|held| StateId::new(&name_text(&held.state)))
    }

    fn is_settled(deployment: &DeploymentId, workflow: &WorkflowId, instance: u64) -> bool {
        name_of(workflow.as_str())
            .ok()
            .and_then(|name| Self::instance(deployment, &name, instance))
            .map(|held| held.status == InstanceStatus::Settled)
            .unwrap_or(false)
    }

    fn step_count(deployment: &DeploymentId, workflow: &WorkflowId, instance: u64) -> u32 {
        name_of(workflow.as_str())
            .ok()
            .and_then(|name| Self::instance(deployment, &name, instance))
            .map(|held| held.history.len() as u32)
            .unwrap_or(0)
    }
}

/// The timers and deadlines one state carries.
pub fn timers_of<'a>(machine: &'a Workflow, state: &str) -> Vec<&'a Timer> {
    machine
        .timers
        .iter()
        .filter(|timer| timer.state.as_str() == state)
        .collect()
}

/// The step one timer action names, ready for a diagnostic.
pub fn timer_action_label(action: &TimerAction) -> &'static str {
    match action {
        TimerAction::MoveTo(_) => "move the record",
        TimerAction::Run(_) => "run the action",
        TimerAction::Emit(_) => "record the event",
    }
}
