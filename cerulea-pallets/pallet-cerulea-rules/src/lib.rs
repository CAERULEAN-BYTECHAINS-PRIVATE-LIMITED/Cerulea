//! The Cerulea rules module.
//!
//! The rules module evaluates the two kinds of condition a Blueprint
//! declares. A precondition gates one action. An invariant holds at all
//! times over one collection.
//!
//! The timing is fixed. A precondition evaluates before an action runs. An
//! invariant evaluates after every state change inside the transaction. A
//! rule answering false aborts the whole transaction, so the store never
//! holds a record that breaks a rule.
//!
//! Three operations drive the module. Register rule. Evaluate. Explain
//! failure.
//!
//! A failed rule answers with the two plain sentences the person wrote
//! beside the rule in the Studio. One sentence states the problem. One
//! sentence states the fix. The formal expression stays inside the module.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod explain;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod registry_tests;
#[cfg(test)]
mod tests;

pub use explain::{RuleFailure, RuleKind};
pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use cerulea_blueprint::{
    expression::{Env, Meter},
    rules::{Invariant, Precondition},
    types::{CollectionId, Identity as ActorIdentity, ParamName, RuleId, Timestamp, Value},
    Path,
};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_schema::primitives::{name_of, name_text, BoundedName, DeploymentId, RecordKey};
use pallet_cerulea_state::RecordStore;

use crate::explain::RuleKind as Kind;

/// The name an invariant binds the record to.
pub const RECORD_BINDING: &str = cerulea_blueprint::permissions::ROW_BINDING;

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "rules";

/// One registered rule with its stored definition.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxBytes))]
pub struct StoredRule<AccountId, BlockNumber, MaxBytes>
where
    MaxBytes: Get<u32> + 'static,
{
    /// The canonical encoding of the rule definition.
    pub definition: BoundedVec<u8, MaxBytes>,
    /// The collection the rule covers. A precondition carries no
    /// collection.
    pub collection: Option<BoundedName>,
    /// The account that registered the rule.
    pub registrar: AccountId,
    /// The block the rule landed at.
    pub registered_at: BlockNumber,
}

/// The evaluation a dependant module asks the rules module for.
///
/// The state module, the logic module and the workflow module all reach
/// the rules through this trait. A caller never reads rule storage
/// directly.
pub trait RuleGate {
    /// The account type the runtime carries.
    type AccountId;

    /// Evaluate one precondition over the inputs an action supplies.
    ///
    /// The call runs before the action. A false answer carries the two
    /// plain sentences the person wrote beside the rule.
    fn check_precondition(
        deployment: &DeploymentId,
        rule: &RuleId,
        inputs: &[(ParamName, Value)],
        actor: &Self::AccountId,
    ) -> Result<(), RuleFailure>;

    /// Evaluate every invariant of one collection over one record.
    ///
    /// The call runs after a state change inside the transaction. The
    /// first rule that declines stops the pass and carries its sentences.
    fn check_invariants(
        deployment: &DeploymentId,
        collection: &CollectionId,
        record: &Value,
        actor: &Self::AccountId,
    ) -> Result<(), RuleFailure>;

    /// The two plain sentences one registered rule carries.
    fn explain(deployment: &DeploymentId, rule: &RuleId) -> Option<RuleFailure>;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the rules module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The store holding the records the invariants cover.
        type Records: RecordStore<AccountId = Self::AccountId>;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every evaluation reads.
        type Time: UnixTime;

        /// The largest rule definition, in bytes.
        #[pallet::constant]
        type MaxRuleBytes: Get<u32>;

        /// The largest number of rules one deployment holds.
        #[pallet::constant]
        type MaxRulesPerDeployment: Get<u32>;

        /// The largest number of invariants one collection carries.
        #[pallet::constant]
        type MaxRulesPerCollection: Get<u32>;

        /// The step budget of one rule evaluation.
        #[pallet::constant]
        type RuleStepLimit: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every registered invariant.
    #[pallet::storage]
    pub type Invariants<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        StoredRule<T::AccountId, BlockNumberFor<T>, T::MaxRuleBytes>,
        OptionQuery,
    >;

    /// Every registered precondition.
    #[pallet::storage]
    pub type Preconditions<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        StoredRule<T::AccountId, BlockNumberFor<T>, T::MaxRuleBytes>,
        OptionQuery,
    >;

    /// The invariants of every collection, in registration order.
    #[pallet::storage]
    pub type CollectionRules<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        BoundedVec<BoundedName, T::MaxRulesPerCollection>,
        ValueQuery,
    >;

    /// The registered rule names of every deployment.
    #[pallet::storage]
    pub type RuleNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxRulesPerDeployment>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An invariant entered the registry.
        InvariantRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The rule.
            rule: BoundedName,
            /// The collection the rule covers.
            collection: BoundedName,
        },
        /// A precondition entered the registry.
        PreconditionRegistered {
            /// The deployment.
            deployment: DeploymentId,
            /// The rule.
            rule: BoundedName,
        },
        /// A rule left the registry.
        RuleRetired {
            /// The deployment.
            deployment: DeploymentId,
            /// The rule.
            rule: BoundedName,
        },
        /// A rule held over the work it covers.
        RuleHeld {
            /// The deployment.
            deployment: DeploymentId,
            /// The rule.
            rule: BoundedName,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
        },
        /// A rule declined the work it covers.
        RuleDeclined {
            /// The deployment.
            deployment: DeploymentId,
            /// The rule.
            rule: BoundedName,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the rule name.
        NameTooLong,
        /// The definition failed to decode. Submit the canonical encoding
        /// the Studio produces.
        RuleUnreadable,
        /// The rule carries an empty problem sentence or an empty fix
        /// sentence. Write both sentences beside the rule in the Studio.
        SentenceMissing,
        /// The registry already holds a rule under that name. Retire the
        /// rule before registering another under the same name.
        RulePresent,
        /// The registry lacks that rule. Register the rule before
        /// evaluating it.
        RuleUnknown,
        /// The deployment holds its full count of rules. Retire a rule, or
        /// raise the limit in the runtime.
        RuleLimitReached,
        /// The collection carries its full count of invariants. Retire an
        /// invariant, or raise the limit in the runtime.
        CollectionRulesFull,
        /// The store holds no record under that key. Insert the record
        /// before evaluating an invariant over it.
        RecordAbsent,
        /// The rule declined the work. Call the explain entry point for the
        /// problem and the fix in plain language.
        RuleDeclinedWork,
        /// The rule expression faulted. Correct the rule in the Studio.
        RuleFaulted,
        /// The call supplies more inputs than the rule reads. Supply one
        /// value for each declared input.
        InputsTooMany,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one invariant.
        ///
        /// The rule covers one collection and holds at all times. The rule
        /// carries one sentence stating the problem and one sentence
        /// stating the fix.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_invariant())]
        pub fn register_invariant(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            definition: BoundedVec<u8, T::MaxRuleBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let rule = Invariant::decode(&mut definition.as_slice())
                .map_err(|_| Error::<T>::RuleUnreadable)?;
            ensure!(
                !rule.problem.is_empty() && !rule.fix.is_empty(),
                Error::<T>::SentenceMissing
            );
            let name = name_of(rule.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            let collection =
                name_of(rule.collection.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            ensure!(
                Invariants::<T>::get(deployment, &name).is_none(),
                Error::<T>::RulePresent
            );

            Self::note_name(&deployment, &name)?;
            CollectionRules::<T>::try_mutate(deployment, &collection, |names| -> DispatchResult {
                names
                    .try_push(name.clone())
                    .map_err(|_| Error::<T>::CollectionRulesFull)?;
                Ok(())
            })?;
            Invariants::<T>::insert(
                deployment,
                &name,
                StoredRule {
                    definition,
                    collection: Some(collection.clone()),
                    registrar: who.clone(),
                    registered_at: frame_system::Pallet::<T>::block_number(),
                },
            );

            Self::record(
                &deployment,
                "invariant_registered",
                WorkClass::Structural,
                &who,
                "The steward registered one invariant over a collection.",
                (name.clone(), collection.clone()).encode(),
            )?;
            Self::deposit_event(Event::InvariantRegistered {
                deployment,
                rule: name,
                collection,
            });
            Ok(())
        }

        /// Register one precondition.
        ///
        /// The rule gates one action. The rule carries one sentence
        /// stating the problem and one sentence stating the fix.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::register_precondition())]
        pub fn register_precondition(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            definition: BoundedVec<u8, T::MaxRuleBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let rule = Precondition::decode(&mut definition.as_slice())
                .map_err(|_| Error::<T>::RuleUnreadable)?;
            ensure!(
                !rule.problem.is_empty() && !rule.fix.is_empty(),
                Error::<T>::SentenceMissing
            );
            let name = name_of(rule.id.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
            ensure!(
                Preconditions::<T>::get(deployment, &name).is_none(),
                Error::<T>::RulePresent
            );

            Self::note_name(&deployment, &name)?;
            Preconditions::<T>::insert(
                deployment,
                &name,
                StoredRule {
                    definition,
                    collection: None,
                    registrar: who.clone(),
                    registered_at: frame_system::Pallet::<T>::block_number(),
                },
            );

            Self::record(
                &deployment,
                "precondition_registered",
                WorkClass::Structural,
                &who,
                "The steward registered one precondition over an action.",
                name.to_vec(),
            )?;
            Self::deposit_event(Event::PreconditionRegistered {
                deployment,
                rule: name,
            });
            Ok(())
        }

        /// Remove one registered rule.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::retire_rule())]
        pub fn retire_rule(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            rule: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let held = Invariants::<T>::get(deployment, &rule)
                .or_else(|| Preconditions::<T>::get(deployment, &rule))
                .ok_or(Error::<T>::RuleUnknown)?;
            if let Some(collection) = held.collection.as_ref() {
                CollectionRules::<T>::mutate(deployment, collection, |names| {
                    names.retain(|name| name != &rule);
                });
            }
            Invariants::<T>::remove(deployment, &rule);
            Preconditions::<T>::remove(deployment, &rule);
            RuleNames::<T>::mutate(deployment, |names| names.retain(|name| name != &rule));

            Self::record(
                &deployment,
                "rule_retired",
                WorkClass::Structural,
                &who,
                "The steward retired one rule.",
                rule.to_vec(),
            )?;
            Self::deposit_event(Event::RuleRetired { deployment, rule });
            Ok(())
        }

        /// Evaluate one precondition over the inputs a caller supplies.
        ///
        /// The call answers with success while the rule holds. A rule
        /// declining the work aborts the call.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::evaluate_precondition())]
        pub fn evaluate_precondition(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            rule: BoundedName,
            inputs: BoundedVec<u8, T::MaxRuleBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let supplied: Vec<(ParamName, Value)> =
                Decode::decode(&mut inputs.as_slice()).map_err(|_| Error::<T>::RuleUnreadable)?;
            match Self::run_precondition(&deployment, &rule, &supplied, &who) {
                Ok(()) => {
                    Self::deposit_event(Event::RuleHeld {
                        deployment,
                        rule,
                        actor: who,
                    });
                    Ok(())
                }
                Err(failure) => {
                    Self::record(
                        &deployment,
                        "precondition_declined",
                        WorkClass::Routine,
                        &who,
                        "One precondition declined the work a caller asked for.",
                        failure.bytes(),
                    )?;
                    Self::deposit_event(Event::RuleDeclined {
                        deployment,
                        rule,
                        actor: who,
                    });
                    Err(Error::<T>::RuleDeclinedWork.into())
                }
            }
        }

        /// Evaluate every invariant of one collection over one record.
        ///
        /// The call answers with success while every rule holds. The first
        /// rule declining the work aborts the call.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::evaluate_invariants())]
        pub fn evaluate_invariants(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            collection: BoundedName,
            key: RecordKey,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let id = CollectionId::new(&name_text(&collection));
            let record =
                T::Records::read(&deployment, &id, &key).ok_or(Error::<T>::RecordAbsent)?;
            match Self::run_invariants(&deployment, &id, &record, &who) {
                Ok(()) => Ok(()),
                Err(failure) => {
                    Self::record(
                        &deployment,
                        "invariant_declined",
                        WorkClass::Routine,
                        &who,
                        "One invariant declined the change a caller asked for.",
                        failure.bytes(),
                    )?;
                    Self::deposit_event(Event::RuleDeclined {
                        deployment,
                        rule: failure.rule,
                        actor: who,
                    });
                    Err(Error::<T>::RuleDeclinedWork.into())
                }
            }
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The wall clock time of the Blockchain, in milliseconds.
    pub fn now_millis() -> u64 {
        T::Time::now().as_millis() as u64
    }

    /// Append one record to the audit log.
    fn record(
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

    /// Note one rule name in the registry of the deployment.
    fn note_name(deployment: &DeploymentId, name: &BoundedName) -> DispatchResult {
        RuleNames::<T>::try_mutate(deployment, |names| -> DispatchResult {
            if names.contains(name) {
                return Ok(());
            }
            names
                .try_push(name.clone())
                .map_err(|_| Error::<T>::RuleLimitReached)?;
            Ok(())
        })
    }

    /// One registered invariant.
    pub fn invariant(deployment: &DeploymentId, rule: &BoundedName) -> Option<Invariant> {
        let held = Invariants::<T>::get(deployment, rule)?;
        Invariant::decode(&mut held.definition.as_slice()).ok()
    }

    /// One registered precondition.
    pub fn precondition(deployment: &DeploymentId, rule: &BoundedName) -> Option<Precondition> {
        let held = Preconditions::<T>::get(deployment, rule)?;
        Precondition::decode(&mut held.definition.as_slice()).ok()
    }

    /// The invariants one collection carries, in registration order.
    pub fn invariants_of(deployment: &DeploymentId, collection: &CollectionId) -> Vec<BoundedName> {
        match name_of(collection.as_str()) {
            Ok(name) => CollectionRules::<T>::get(deployment, name).into_inner(),
            Err(_) => Vec::new(),
        }
    }

    /// Build the evaluation environment of one rule.
    fn environment(now: u64) -> Env<'static> {
        Env::new(ActorIdentity::default(), Timestamp::from_millis(now))
    }

    /// Evaluate one precondition over bound inputs.
    pub fn run_precondition(
        deployment: &DeploymentId,
        rule: &BoundedName,
        inputs: &[(ParamName, Value)],
        _actor: &T::AccountId,
    ) -> Result<(), RuleFailure> {
        let held = Self::precondition(deployment, rule).ok_or_else(|| {
            RuleFailure::new(
                rule.clone(),
                Kind::Precondition,
                "The registry lacks that rule.",
                "Register the rule before evaluating it.",
            )
        })?;
        if inputs.len() > held.inputs.len() {
            return Err(RuleFailure::new(
                rule.clone(),
                Kind::Precondition,
                "The call supplies more values than the rule reads.",
                "Supply one value for each declared input.",
            ));
        }

        let mut env = Self::environment(Self::now_millis());
        for (name, value) in inputs.iter() {
            env.bind(name, value.clone());
        }
        let mut meter = Meter::new(T::RuleStepLimit::get());
        let path = Path::root().field("rules").key(held.id.as_str());
        match held.check_inputs(&env, &mut meter, &path) {
            Ok(true) => Ok(()),
            Ok(false) => Err(RuleFailure::of_precondition(rule.clone(), &held)),
            Err(diagnostic) => Err(RuleFailure::of_diagnostic(
                rule.clone(),
                Kind::Precondition,
                &diagnostic,
            )),
        }
    }

    /// Evaluate every invariant of one collection over one record.
    pub fn run_invariants(
        deployment: &DeploymentId,
        collection: &CollectionId,
        record: &Value,
        _actor: &T::AccountId,
    ) -> Result<(), RuleFailure> {
        let now = Self::now_millis();
        let env = Self::environment(now);
        for name in Self::invariants_of(deployment, collection).into_iter() {
            let held = match Self::invariant(deployment, &name) {
                Some(found) => found,
                None => continue,
            };
            let mut meter = Meter::new(T::RuleStepLimit::get());
            let path = Path::root().field("rules").key(held.id.as_str());
            match held.check_record(record, &env, &mut meter, &path) {
                Ok(true) => {}
                Ok(false) => return Err(RuleFailure::of_invariant(name, &held)),
                Err(diagnostic) => {
                    return Err(RuleFailure::of_diagnostic(
                        name,
                        Kind::Invariant,
                        &diagnostic,
                    ))
                }
            }
        }
        Ok(())
    }

    /// The two plain sentences one registered rule carries.
    pub fn sentences(deployment: &DeploymentId, rule: &BoundedName) -> Option<RuleFailure> {
        if let Some(held) = Self::invariant(deployment, rule) {
            return Some(RuleFailure::of_invariant(rule.clone(), &held));
        }
        Self::precondition(deployment, rule)
            .map(|held| RuleFailure::of_precondition(rule.clone(), &held))
    }
}

impl<T: Config> RuleGate for Pallet<T> {
    type AccountId = T::AccountId;

    fn check_precondition(
        deployment: &DeploymentId,
        rule: &RuleId,
        inputs: &[(ParamName, Value)],
        actor: &Self::AccountId,
    ) -> Result<(), RuleFailure> {
        let name = name_of(rule.as_str()).map_err(|_| {
            RuleFailure::new(
                BoundedVec::truncate_from(vec![]),
                Kind::Precondition,
                "The rule name runs past 128 bytes.",
                "Shorten the rule name in the Studio.",
            )
        })?;
        Self::run_precondition(deployment, &name, inputs, actor)
    }

    fn check_invariants(
        deployment: &DeploymentId,
        collection: &CollectionId,
        record: &Value,
        actor: &Self::AccountId,
    ) -> Result<(), RuleFailure> {
        Self::run_invariants(deployment, collection, record, actor)
    }

    fn explain(deployment: &DeploymentId, rule: &RuleId) -> Option<RuleFailure> {
        let name = name_of(rule.as_str()).ok()?;
        Self::sentences(deployment, &name)
    }
}
