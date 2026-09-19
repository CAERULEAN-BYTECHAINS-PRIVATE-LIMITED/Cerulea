//! The Cerulea logic module.
//!
//! The logic module is the Blueprint interpreter. Four operations drive
//! it. Install a Blueprint. Execute an action. Evaluate an expression.
//! Evaluate a view.
//!
//! Three properties hold for every operation.
//!
//! 1. **Metered.** Every step charges against a budget the runtime sets.
//!    An evaluation that spends the budget aborts.
//! 2. **Deterministic.** The interpreter reads the Blockchain clock and
//!    the caller identity alone. Two nodes therefore reach the same
//!    result.
//! 3. **Bounded.** Every loop is a fold with a declared element count. The
//!    count is known before the fold runs, so termination is proven
//!    rather than assumed.
//!
//! Installation derives the deployment identifier from the content
//! address of the Blueprint. A change to any part of a Blueprint
//! therefore produces a new deployment identifier, and the Blueprint a
//! deployment runs is always the Blueprint its identifier names.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_support;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{
    collections::KeyRange,
    expression::{Env, Expr, Meter},
    fold::CollectionView,
    types::{CollectionId, Identity, ParamName, Timestamp, Value},
    Blueprint, Path,
};
use codec::Decode;
use core::marker::PhantomData;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use pallet_cerulea_identity::IdentityRegistry;
use pallet_cerulea_permission::PermissionMatrix;
use pallet_cerulea_schema::{
    access::PermissionGate,
    primitives::{name_of, BoundedName, DeploymentId},
    registry::SchemaRegistrar,
};
use pallet_cerulea_state::RecordStore;

/// A read over the record store, shaped for the expression evaluator.
///
/// A fold over a collection range asks this view for the records the
/// range reaches. The view answers in canonical key order, so two nodes
/// fold in the same order.
pub struct StateView<T: Config> {
    deployment: DeploymentId,
    limit: u32,
    marker: PhantomData<T>,
}

impl<T: Config> StateView<T> {
    /// Build a view over one deployment.
    pub fn new(deployment: DeploymentId, limit: u32) -> Self {
        StateView {
            deployment,
            limit,
            marker: PhantomData,
        }
    }
}

impl<T: Config> core::fmt::Debug for StateView<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("StateView")
    }
}

impl<T: Config> CollectionView for StateView<T> {
    fn records(&self, collection: &CollectionId, keys: &KeyRange) -> Vec<Value> {
        T::Records::range(&self.deployment, collection, keys, self.limit)
            .into_iter()
            .map(|(_, record)| record)
            .collect()
    }
}

/// The bindings one evaluation starts with.
pub type Bindings = Vec<(ParamName, Value)>;

/// The interpreter a dependant module drives.
pub trait LogicInterpreter {
    /// The account type the runtime carries.
    type AccountId;

    /// Report whether a deployment holds an installed Blueprint.
    fn is_installed(deployment: &DeploymentId) -> bool;

    /// The Blueprint one deployment runs.
    fn blueprint(deployment: &DeploymentId) -> Option<Blueprint>;

    /// Evaluate one expression under the given bindings.
    fn evaluate(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
        expression: &Expr,
        bindings: &Bindings,
    ) -> Result<Value, DispatchError>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the logic module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The registry installation registers types and collections in.
        type Schema: SchemaRegistrar;

        /// The store the effects read and write.
        type Records: RecordStore<AccountId = Self::AccountId>;

        /// The resolver the interpreter asks before it runs an action.
        type Permissions: PermissionGate<AccountId = Self::AccountId>
            + PermissionMatrix<AccountId = Self::AccountId>;

        /// The registry that answers which roles an actor holds.
        type Identity: IdentityRegistry<AccountId = Self::AccountId>;

        /// The largest encoded Blueprint the module accepts, in bytes.
        #[pallet::constant]
        type MaxBlueprintBytes: Get<u32>;

        /// The largest encoded payload one call carries, in bytes.
        #[pallet::constant]
        type MaxPayloadBytes: Get<u32>;

        /// The expression step budget one evaluation runs inside.
        #[pallet::constant]
        type StepLimit: Get<u64>;

        /// The largest number of records one view or one fold reads.
        #[pallet::constant]
        type MaxRowsPerRead: Get<u32>;

        /// The largest number of effects one action runs, counting the
        /// steps a block call expands into.
        #[pallet::constant]
        type MaxEffectsPerAction: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The Blueprint every deployment runs.
    #[pallet::storage]
    #[pallet::unbounded]
    pub type Installed<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, Blueprint, OptionQuery>;

    /// The block every deployment installed at.
    #[pallet::storage]
    pub type InstalledAt<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, BlockNumberFor<T>, OptionQuery>;

    /// The number of actions every deployment has run.
    #[pallet::storage]
    pub type ActionRuns<T: Config> = StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// A Blueprint entered a deployment.
        BlueprintInstalled {
            /// The deployment, which is the content address of the
            /// Blueprint.
            deployment: DeploymentId,
            /// The number of collections the Blueprint declares.
            collections: u32,
            /// The number of actions the Blueprint declares.
            actions: u32,
            /// The account that installed the Blueprint.
            installer: T::AccountId,
        },
        /// An expression produced a result.
        ExpressionEvaluated {
            /// The deployment.
            deployment: DeploymentId,
            /// The hash of the canonical encoding of the result.
            digest: [u8; 32],
            /// The number of steps the evaluation spent.
            steps: u64,
        },
        /// An action ran and every effect landed.
        ActionExecuted {
            /// The deployment.
            deployment: DeploymentId,
            /// The action.
            action: BoundedName,
            /// The account the chain records as the actor.
            actor: T::AccountId,
            /// The number of effects the action ran.
            effects: u32,
            /// The number of steps the action spent.
            steps: u64,
        },
        /// An action failed and its compensating steps ran in reverse.
        ActionCompensated {
            /// The deployment.
            deployment: DeploymentId,
            /// The action.
            action: BoundedName,
            /// The account the chain records as the actor.
            actor: T::AccountId,
            /// The plain sentence a person reads.
            reason: BoundedVec<u8, ConstU32<512>>,
        },
        /// An action emitted one declared event.
        EventEmitted {
            /// The deployment.
            deployment: DeploymentId,
            /// The event.
            event: BoundedName,
            /// The account the chain records as the actor.
            actor: T::AccountId,
            /// The hash of the canonical encoding of the payload.
            digest: [u8; 32],
        },
        /// A view produced rows.
        ViewEvaluated {
            /// The deployment.
            deployment: DeploymentId,
            /// The view.
            view: BoundedName,
            /// The number of rows the view produced.
            rows: u32,
            /// The hash of the canonical encoding of the rows.
            digest: [u8; 32],
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the name in the
        /// Blueprint.
        NameTooLong,
        /// The payload failed to decode. Submit the canonical encoding
        /// the Studio produces.
        PayloadUnreadable,
        /// The Blueprint failed validation. Run the Studio check to read
        /// every problem with its location and its fix.
        BlueprintInvalid,
        /// The deployment already runs that Blueprint. Publish a change
        /// to produce a new deployment identifier.
        BlueprintPresent,
        /// The deployment runs no Blueprint. Install a Blueprint before
        /// running an action.
        BlueprintAbsent,
        /// The Blueprint declares no action under that name. Name an
        /// action the Blueprint declares.
        ActionUnknown,
        /// The Blueprint declares no view under that name. Name a view
        /// the Blueprint declares.
        ViewUnknown,
        /// The Blueprint declares no event under that name. Name an event
        /// the Blueprint declares.
        EventUnknown,
        /// The Blueprint declares no reusable block under that name. Name
        /// a block the Blueprint declares.
        BlockUnknown,
        /// The call leaves out a parameter the action declares. Supply a
        /// value for every declared parameter.
        ParameterMissing,
        /// The evaluation spent its step budget. Simplify the expression,
        /// or raise the budget in the runtime.
        BudgetSpent,
        /// The expression produced a value of another shape. Match the
        /// expression to the type the Blueprint declares.
        ResultMismatch,
        /// A precondition of the action declined the call. Read the
        /// sentence the author wrote beside the rule.
        PreconditionFailed,
        /// The permission module refused the call. Call the explain entry
        /// point to read the problem and the fix in plain language.
        PermissionDenied,
        /// An effect reaches a collection the Access Manifest leaves out.
        /// Declare the collection in the action, or narrow the effect.
        ManifestBreach,
        /// The action runs more effects than the runtime admits. Split
        /// the action, or raise the limit in the runtime.
        EffectLimitReached,
        /// The effect reaches a workflow state. The workflow module joins
        /// the runtime at a later task, so run the transition through the
        /// workflow module once it lands.
        TransitionUnsupported,
        /// A record the effect writes carries another shape. Match the
        /// record to the registered type.
        RecordInvalid,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Install one Blueprint.
        ///
        /// The deployment identifier is the content address of the
        /// Blueprint. Installation validates the whole Blueprint and then
        /// registers every type and every collection it declares.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::install_blueprint())]
        pub fn install_blueprint(
            origin: OriginFor<T>,
            blueprint: BoundedVec<u8, T::MaxBlueprintBytes>,
        ) -> DispatchResult {
            let installer = ensure_signed(origin)?;
            let parsed = Blueprint::decode(&mut blueprint.as_slice())
                .map_err(|_| Error::<T>::PayloadUnreadable)?;
            parsed
                .validate()
                .map_err(|_| Error::<T>::BlueprintInvalid)?;

            let deployment = parsed.id().0;
            ensure!(
                Installed::<T>::get(deployment).is_none(),
                Error::<T>::BlueprintPresent
            );
            T::Schema::register_blueprint(&deployment, &parsed)?;

            let collections = u32::try_from(parsed.collections.len()).unwrap_or(u32::MAX);
            let actions = u32::try_from(parsed.actions.len()).unwrap_or(u32::MAX);
            Installed::<T>::insert(deployment, parsed);
            InstalledAt::<T>::insert(deployment, frame_system::Pallet::<T>::block_number());
            Self::deposit_event(Event::BlueprintInstalled {
                deployment,
                collections,
                actions,
                installer,
            });
            Ok(())
        }

        /// Evaluate one expression and write its result digest.
        ///
        /// The Studio reads the digest to confirm that a formula produces
        /// the same result on the Blockchain as it produced in the
        /// preview. The call changes no record.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::evaluate_expression())]
        pub fn evaluate_expression(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            expression: BoundedVec<u8, T::MaxPayloadBytes>,
            bindings: BoundedVec<u8, T::MaxPayloadBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let parsed = Expr::decode(&mut expression.as_slice())
                .map_err(|_| Error::<T>::PayloadUnreadable)?;
            let bound = Bindings::decode(&mut bindings.as_slice())
                .map_err(|_| Error::<T>::PayloadUnreadable)?;
            let (result, steps) = Self::run_expression(&deployment, &who, &parsed, &bound)?;
            Self::deposit_event(Event::ExpressionEvaluated {
                deployment,
                digest: sp_crypto_hashing::blake2_256(&result.canonical_bytes()),
                steps,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The moment the interpreter reads.
    pub fn now() -> Timestamp {
        Timestamp::from_millis(T::Identity::now_millis())
    }

    /// The identity value one account carries.
    pub fn identity_of(who: &T::AccountId) -> Identity {
        Identity(T::Identity::identity_of(who).unwrap_or([0u8; 32]))
    }

    /// A fresh meter carrying the runtime step budget.
    pub fn meter() -> Meter {
        Meter::new(T::StepLimit::get())
    }

    /// Turn a name into its bounded form.
    pub fn bounded(name: &str) -> Result<BoundedName, DispatchError> {
        name_of(name).map_err(|_| Error::<T>::NameTooLong.into())
    }

    /// The Blueprint one deployment runs.
    pub fn installed(deployment: &DeploymentId) -> Result<Blueprint, DispatchError> {
        Installed::<T>::get(deployment).ok_or_else(|| Error::<T>::BlueprintAbsent.into())
    }

    /// Evaluate one expression and report the steps it spent.
    pub fn run_expression(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        expression: &Expr,
        bindings: &Bindings,
    ) -> Result<(Value, u64), DispatchError> {
        let view = StateView::<T>::new(*deployment, T::MaxRowsPerRead::get());
        let mut env = Env::with_view(Self::identity_of(actor), Self::now(), &view);
        for (name, value) in bindings.iter() {
            env.bind(name, value.clone());
        }
        let mut meter = Self::meter();
        let produced = expression
            .evaluate(&env, &mut meter, &Path::root().field("expression"))
            .map_err(|_| Error::<T>::BudgetSpent)?;
        Ok((produced, meter.used()))
    }

    /// Evaluate one expression under a prepared setting.
    pub fn run_in(
        expression: &Expr,
        env: &Env,
        meter: &mut Meter,
        path: &Path,
    ) -> Result<Value, DispatchError> {
        expression
            .evaluate(env, meter, path)
            .map_err(|_| Error::<T>::BudgetSpent.into())
    }

    /// The digest of one value.
    pub fn digest_of(value: &Value) -> [u8; 32] {
        sp_crypto_hashing::blake2_256(&value.canonical_bytes())
    }

    /// The digest of a list of rows.
    pub fn digest_of_rows(rows: &[Vec<(cerulea_blueprint::types::FieldName, Value)>]) -> [u8; 32] {
        let mut held: Vec<u8> = vec![];
        for row in rows.iter() {
            for (name, value) in row.iter() {
                held.extend_from_slice(name.as_str().as_bytes());
                held.extend_from_slice(&value.canonical_bytes());
            }
        }
        sp_crypto_hashing::blake2_256(&held)
    }
}

impl<T: Config> LogicInterpreter for Pallet<T> {
    type AccountId = T::AccountId;

    fn is_installed(deployment: &DeploymentId) -> bool {
        Installed::<T>::contains_key(deployment)
    }

    fn blueprint(deployment: &DeploymentId) -> Option<Blueprint> {
        Installed::<T>::get(deployment)
    }

    fn evaluate(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
        expression: &Expr,
        bindings: &Bindings,
    ) -> Result<Value, DispatchError> {
        Pallet::<T>::run_expression(deployment, actor, expression, bindings).map(|(value, _)| value)
    }
}
