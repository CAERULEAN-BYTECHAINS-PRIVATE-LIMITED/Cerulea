//! The Cerulea identity module.
//!
//! The identity module answers one question for every other module. Who
//! is acting, and what does that actor carry. Five registries hold the
//! answer.
//!
//! 1. **Accounts.** Every account that acts in a deployment registers
//!    once and carries one identity value.
//! 2. **Organisational units.** A unit sits under a parent unit, so a
//!    deployment carries a tree rather than a flat list.
//! 3. **Roles.** A role belongs to a unit and names the credentials a
//!    holder needs.
//! 4. **Credentials.** An attestation names the issuer and the expiry. A
//!    credential past its expiry counts as absent.
//! 5. **Delegation.** A delegation passes a named scope from one account
//!    to another for a declared span. A delegation past its expiry
//!    resolves to denied.
//!
//! Every expiry reads the runtime clock in milliseconds. Two nodes read
//! the same clock inside one block, so a resolution is deterministic.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::vec::Vec;
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId};

/// One registered account.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(BlockNumber))]
pub struct AccountRecord<BlockNumber> {
    /// The identity value the Blueprint expressions read.
    pub identity: [u8; 32],
    /// The block the account registered at.
    pub registered_at: BlockNumber,
    /// An account that stopped acting keeps its records and stops
    /// resolving to any role.
    pub active: bool,
}

/// One organisational unit.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(Text))]
pub struct UnitRecord<Text> {
    /// The unit above this one. A top unit names none.
    pub parent: Option<BoundedName>,
    /// The title a person reads.
    pub title: Text,
    /// One sentence stating what the unit does.
    pub summary: Text,
}

/// One role definition.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(Text, MaxCredentials))]
pub struct RoleRecord<Text, MaxCredentials: Get<u32>> {
    /// The unit the role belongs to. A deployment wide role names none.
    pub unit: Option<BoundedName>,
    /// The credentials a holder needs.
    pub requires: BoundedVec<BoundedName, MaxCredentials>,
    /// The title a person reads.
    pub title: Text,
}

/// One role assignment.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
#[scale_info(skip_type_params(AccountId))]
pub struct Assignment<AccountId> {
    /// The account that granted the assignment.
    pub granter: AccountId,
    /// The moment the assignment landed, in milliseconds.
    pub granted_at: u64,
    /// The moment the assignment lapses, in milliseconds. An assignment
    /// that lasts names none.
    pub expires_at: Option<u64>,
}

impl<AccountId> Assignment<AccountId> {
    /// Report whether the assignment holds at one moment.
    pub fn holds_at(&self, now: u64) -> bool {
        match self.expires_at {
            Some(expiry) => now < expiry,
            None => true,
        }
    }
}

/// The read a dependant module performs over the identity registries.
pub trait IdentityRegistry {
    /// The account type the runtime carries.
    type AccountId;

    /// Report whether an account registered and stays active.
    fn is_registered(who: &Self::AccountId) -> bool;

    /// The identity value one account carries.
    fn identity_of(who: &Self::AccountId) -> Option<[u8; 32]>;

    /// The roles one account holds at one moment, in name order.
    ///
    /// A role whose assignment lapsed stays out. A role whose credential
    /// requirement fails stays out.
    fn roles_of(deployment: &DeploymentId, who: &Self::AccountId, now: u64) -> Vec<BoundedName>;

    /// Report whether an account holds one role at one moment.
    fn has_role(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        role: &BoundedName,
        now: u64,
    ) -> bool;

    /// The unit one role belongs to.
    fn unit_of(deployment: &DeploymentId, role: &BoundedName) -> Option<BoundedName>;

    /// The unit chain from one unit to the top of the tree.
    fn unit_chain(deployment: &DeploymentId, unit: &BoundedName) -> Vec<BoundedName>;

    /// The clock the module reads, in milliseconds.
    fn now_millis() -> u64;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// A bounded piece of text the interface layer shows.
    pub type Text<T> = BoundedVec<u8, <T as Config>::MaxTextBytes>;

    /// The settings the runtime supplies to the identity module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The clock every expiry reads.
        type Time: UnixTime;

        /// The largest number of units one deployment holds.
        #[pallet::constant]
        type MaxUnitsPerDeployment: Get<u32>;

        /// The largest number of roles one deployment holds.
        #[pallet::constant]
        type MaxRolesPerDeployment: Get<u32>;

        /// The largest number of roles one account holds.
        #[pallet::constant]
        type MaxRolesPerAccount: Get<u32>;

        /// The largest number of credentials one role requires.
        #[pallet::constant]
        type MaxCredentialsPerRole: Get<u32>;

        /// The largest depth of the organisation tree.
        #[pallet::constant]
        type MaxUnitDepth: Get<u32>;

        /// The largest piece of text one record carries, in bytes.
        #[pallet::constant]
        type MaxTextBytes: Get<u32>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every registered account.
    #[pallet::storage]
    pub type Accounts<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        AccountRecord<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The organisation tree of every deployment.
    #[pallet::storage]
    pub type Units<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        UnitRecord<Text<T>>,
        OptionQuery,
    >;

    /// The unit names of every deployment, in name order.
    #[pallet::storage]
    pub type UnitNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxUnitsPerDeployment>,
        ValueQuery,
    >;

    /// The role definitions of every deployment.
    #[pallet::storage]
    pub type Roles<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        BoundedName,
        RoleRecord<Text<T>, T::MaxCredentialsPerRole>,
        OptionQuery,
    >;

    /// The role names of every deployment, in name order.
    #[pallet::storage]
    pub type RoleNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxRolesPerDeployment>,
        ValueQuery,
    >;

    /// The role assignments of every account.
    #[pallet::storage]
    pub type Assignments<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Blake2_128Concat, T::AccountId>,
            NMapKey<Blake2_128Concat, BoundedName>,
        ),
        Assignment<T::AccountId>,
        OptionQuery,
    >;

    /// The role names every account holds, in name order.
    #[pallet::storage]
    pub type HeldRoles<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        BoundedVec<BoundedName, T::MaxRolesPerAccount>,
        ValueQuery,
    >;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An account entered the registry.
        AccountRegistered {
            /// The account.
            who: T::AccountId,
            /// The identity value the account carries.
            identity: [u8; 32],
        },
        /// An account stopped acting.
        AccountSuspended {
            /// The account.
            who: T::AccountId,
        },
        /// An organisational unit entered the tree.
        UnitCreated {
            /// The deployment.
            deployment: DeploymentId,
            /// The unit.
            unit: BoundedName,
            /// The unit above this one.
            parent: Option<BoundedName>,
        },
        /// A role definition entered the registry.
        RoleDefined {
            /// The deployment.
            deployment: DeploymentId,
            /// The role.
            role: BoundedName,
            /// The unit the role belongs to.
            unit: Option<BoundedName>,
        },
        /// An account gained a role.
        RoleAssigned {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            who: T::AccountId,
            /// The role.
            role: BoundedName,
            /// The moment the assignment lapses.
            expires_at: Option<u64>,
        },
        /// An account lost a role.
        RoleRevoked {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            who: T::AccountId,
            /// The role.
            role: BoundedName,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account already registered. Register each account once.
        AccountPresent,
        /// The registry lacks that account. Register the account before
        /// naming it.
        AccountUnknown,
        /// The account stopped acting. Restore the account before granting
        /// it a role.
        AccountSuspended,
        /// The deployment already holds a unit under that name. Choose a
        /// name the tree lacks.
        UnitPresent,
        /// The tree lacks that unit. Create the unit before naming it.
        UnitUnknown,
        /// The unit tree holds its full depth. Place the unit higher in
        /// the tree, or raise the depth in the runtime.
        UnitDepthReached,
        /// The unit names itself as an ancestor. Choose a parent outside
        /// the branch of the unit.
        UnitCycle,
        /// The deployment holds its full count of units. Raise the limit
        /// in the runtime.
        UnitLimitReached,
        /// The deployment already holds a role under that name. Choose a
        /// name the registry lacks.
        RolePresent,
        /// The registry lacks that role. Define the role before assigning
        /// it.
        RoleUnknown,
        /// The deployment holds its full count of roles. Raise the limit
        /// in the runtime.
        RoleLimitReached,
        /// The account holds its full count of roles. Revoke a role, or
        /// raise the limit in the runtime.
        AccountRoleLimitReached,
        /// The account already holds that role. Revoke the assignment
        /// before granting it again.
        RoleAlreadyHeld,
        /// The account holds no such assignment. Assign the role before
        /// revoking it.
        RoleNotHeld,
        /// The expiry sits at or before the present moment. Give the grant
        /// an expiry in the future.
        ExpiryInThePast,
        /// The text runs past the text limit. Shorten the title or the
        /// summary.
        TextTooLong,
        /// The role requires more credentials than the runtime admits.
        /// Remove a requirement, or raise the limit in the runtime.
        CredentialLimitReached,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one account.
        ///
        /// The identity value is the value a Blueprint expression compares
        /// against the caller.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_account())]
        pub fn register_account(
            origin: OriginFor<T>,
            who: T::AccountId,
            identity: [u8; 32],
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                Accounts::<T>::get(&who).is_none(),
                Error::<T>::AccountPresent
            );
            Accounts::<T>::insert(
                &who,
                AccountRecord {
                    identity,
                    registered_at: frame_system::Pallet::<T>::block_number(),
                    active: true,
                },
            );
            Self::deposit_event(Event::AccountRegistered { who, identity });
            Ok(())
        }

        /// Stop one account from acting.
        ///
        /// Every record the account wrote stays readable. The account
        /// resolves to no role from this call onward.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::suspend_account())]
        pub fn suspend_account(origin: OriginFor<T>, who: T::AccountId) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            Accounts::<T>::try_mutate(&who, |slot| -> DispatchResult {
                let held = slot.as_mut().ok_or(Error::<T>::AccountUnknown)?;
                held.active = false;
                Ok(())
            })?;
            Self::deposit_event(Event::AccountSuspended { who });
            Ok(())
        }

        /// Create one organisational unit.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::create_unit())]
        pub fn create_unit(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            unit: BoundedName,
            parent: Option<BoundedName>,
            title: Text<T>,
            summary: Text<T>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                Units::<T>::get(deployment, &unit).is_none(),
                Error::<T>::UnitPresent
            );
            if let Some(above) = parent.as_ref() {
                ensure!(
                    Units::<T>::get(deployment, above).is_some(),
                    Error::<T>::UnitUnknown
                );
                ensure!(above != &unit, Error::<T>::UnitCycle);
                let depth = Self::chain_of(&deployment, above).len();
                ensure!(
                    (depth as u32) < T::MaxUnitDepth::get(),
                    Error::<T>::UnitDepthReached
                );
            }

            UnitNames::<T>::try_mutate(deployment, |names| match names.binary_search(&unit) {
                Ok(_) => Ok(()),
                Err(slot) => names
                    .try_insert(slot, unit.clone())
                    .map_err(|_| Error::<T>::UnitLimitReached),
            })?;
            Units::<T>::insert(
                deployment,
                &unit,
                UnitRecord {
                    parent: parent.clone(),
                    title,
                    summary,
                },
            );
            Self::deposit_event(Event::UnitCreated {
                deployment,
                unit,
                parent,
            });
            Ok(())
        }

        /// Define one role.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::define_role())]
        pub fn define_role(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            role: BoundedName,
            unit: Option<BoundedName>,
            requires: BoundedVec<BoundedName, T::MaxCredentialsPerRole>,
            title: Text<T>,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                Roles::<T>::get(deployment, &role).is_none(),
                Error::<T>::RolePresent
            );
            if let Some(held) = unit.as_ref() {
                ensure!(
                    Units::<T>::get(deployment, held).is_some(),
                    Error::<T>::UnitUnknown
                );
            }
            RoleNames::<T>::try_mutate(deployment, |names| match names.binary_search(&role) {
                Ok(_) => Ok(()),
                Err(slot) => names
                    .try_insert(slot, role.clone())
                    .map_err(|_| Error::<T>::RoleLimitReached),
            })?;
            Roles::<T>::insert(
                deployment,
                &role,
                RoleRecord {
                    unit: unit.clone(),
                    requires,
                    title,
                },
            );
            Self::deposit_event(Event::RoleDefined {
                deployment,
                role,
                unit,
            });
            Ok(())
        }

        /// Give one account one role.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::assign_role())]
        pub fn assign_role(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            who: T::AccountId,
            role: BoundedName,
            expires_at: Option<u64>,
        ) -> DispatchResult {
            let granter = ensure_signed(origin)?;
            let account = Accounts::<T>::get(&who).ok_or(Error::<T>::AccountUnknown)?;
            ensure!(account.active, Error::<T>::AccountSuspended);
            ensure!(
                Roles::<T>::get(deployment, &role).is_some(),
                Error::<T>::RoleUnknown
            );
            ensure!(
                Assignments::<T>::get((deployment, who.clone(), role.clone())).is_none(),
                Error::<T>::RoleAlreadyHeld
            );
            let now = Self::now();
            if let Some(expiry) = expires_at {
                ensure!(expiry > now, Error::<T>::ExpiryInThePast);
            }

            HeldRoles::<T>::try_mutate(deployment, &who, |held| match held.binary_search(&role) {
                Ok(_) => Ok(()),
                Err(slot) => held
                    .try_insert(slot, role.clone())
                    .map_err(|_| Error::<T>::AccountRoleLimitReached),
            })?;
            Assignments::<T>::insert(
                (deployment, who.clone(), role.clone()),
                Assignment {
                    granter,
                    granted_at: now,
                    expires_at,
                },
            );
            Self::deposit_event(Event::RoleAssigned {
                deployment,
                who,
                role,
                expires_at,
            });
            Ok(())
        }

        /// Take one role away from one account.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::revoke_role())]
        pub fn revoke_role(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            who: T::AccountId,
            role: BoundedName,
        ) -> DispatchResult {
            let _ = ensure_signed(origin)?;
            ensure!(
                Assignments::<T>::get((deployment, who.clone(), role.clone())).is_some(),
                Error::<T>::RoleNotHeld
            );
            Assignments::<T>::remove((deployment, who.clone(), role.clone()));
            HeldRoles::<T>::mutate(deployment, &who, |held| {
                held.retain(|present| present != &role);
            });
            Self::deposit_event(Event::RoleRevoked {
                deployment,
                who,
                role,
            });
            Ok(())
        }
    }
}

impl<T: Config> Pallet<T> {
    /// The clock the module reads, in milliseconds.
    pub fn now() -> u64 {
        T::Time::now().as_millis() as u64
    }

    /// The unit chain from one unit to the top of the tree.
    ///
    /// The result starts at the named unit. A tree that folds back on
    /// itself stops at the declared depth.
    pub fn chain_of(deployment: &DeploymentId, unit: &BoundedName) -> Vec<BoundedName> {
        let mut found: Vec<BoundedName> = Vec::new();
        let mut current = Some(unit.clone());
        let ceiling = T::MaxUnitDepth::get() as usize;
        while let Some(held) = current {
            if found.contains(&held) || found.len() >= ceiling {
                break;
            }
            found.push(held.clone());
            current = Units::<T>::get(deployment, &held).and_then(|record| record.parent);
        }
        found
    }

    /// The roles one account holds at one moment, in name order.
    pub fn active_roles(
        deployment: &DeploymentId,
        who: &T::AccountId,
        now: u64,
    ) -> Vec<BoundedName> {
        let account = match Accounts::<T>::get(who) {
            Some(held) => held,
            None => return Vec::new(),
        };
        if !account.active {
            return Vec::new();
        }
        HeldRoles::<T>::get(deployment, who)
            .into_iter()
            .filter(|role| Self::assignment_holds(deployment, who, role, now))
            .collect()
    }

    fn assignment_holds(
        deployment: &DeploymentId,
        who: &T::AccountId,
        role: &BoundedName,
        now: u64,
    ) -> bool {
        match Assignments::<T>::get((*deployment, who.clone(), role.clone())) {
            Some(held) => held.holds_at(now),
            None => false,
        }
    }
}

impl<T: Config> IdentityRegistry for Pallet<T> {
    type AccountId = T::AccountId;

    fn is_registered(who: &Self::AccountId) -> bool {
        Accounts::<T>::get(who)
            .map(|held| held.active)
            .unwrap_or(false)
    }

    fn identity_of(who: &Self::AccountId) -> Option<[u8; 32]> {
        Accounts::<T>::get(who).map(|held| held.identity)
    }

    fn roles_of(deployment: &DeploymentId, who: &Self::AccountId, now: u64) -> Vec<BoundedName> {
        Pallet::<T>::active_roles(deployment, who, now)
    }

    fn has_role(
        deployment: &DeploymentId,
        who: &Self::AccountId,
        role: &BoundedName,
        now: u64,
    ) -> bool {
        Pallet::<T>::active_roles(deployment, who, now).contains(role)
    }

    fn unit_of(deployment: &DeploymentId, role: &BoundedName) -> Option<BoundedName> {
        Roles::<T>::get(deployment, role).and_then(|held| held.unit)
    }

    fn unit_chain(deployment: &DeploymentId, unit: &BoundedName) -> Vec<BoundedName> {
        Pallet::<T>::chain_of(deployment, unit)
    }

    fn now_millis() -> u64 {
        Pallet::<T>::now()
    }
}
