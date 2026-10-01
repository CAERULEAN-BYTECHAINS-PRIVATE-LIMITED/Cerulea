//! The Cerulea operator module.
//!
//! Every customer deployment carries a Cerulea operator role. The role is
//! mandatory and strictly limited.
//!
//! The role does four things. Platform version upgrade. Platform version
//! downgrade. Licence verification, together with suspension of block
//! production on a lapse. Push of platform updates.
//!
//! The role never reads a customer record, never writes customer data,
//! holds no permission over customer governance and alters no Blueprint.
//!
//! The limits are enforced by the permission module rather than by policy.
//! Every request names the collection it reaches. A permitted request
//! names the reserved platform scope. Any request naming a customer
//! collection resolves through the permission module, which refuses the
//! operator role. The requested action then aborts.
//!
//! Every operator action appears in the customer audit log with the actor,
//! the timestamp and the reason.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{string::String, vec::Vec};
use cerulea_blueprint::types::{ActionId, CollectionId, Timestamp};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_evolution::PlatformVersions;
use pallet_cerulea_schema::{
    access::{AccessAction, AccessRequest, PermissionGate},
    primitives::{name_text, BoundedName, DeploymentId},
};
use pallet_cerulea_tier::LicenceControl;

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "operator";

/// The collection the Cerulea operator role reaches.
///
/// The scope holds the platform version and the licence state. The scope
/// holds no customer record.
pub const PLATFORM_SCOPE: &str = "cerulea_platform";

/// What the Cerulea operator role asks for.
///
/// The first five kinds sit inside the scope of the role. The last four
/// kinds sit outside it. The module holds both so a request outside the
/// scope aborts and lands in the audit log with its reason.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub enum OperatorRequest {
    /// Move the deployment to a later platform version.
    UpgradePlatform {
        /// The version the deployment takes.
        version: u32,
    },
    /// Move the deployment to an earlier platform version.
    DowngradePlatform {
        /// The version the deployment takes.
        version: u32,
    },
    /// Check the licence of the deployment.
    VerifyLicence,
    /// Stop block production when the licence lapsed.
    SuspendOnLapse,
    /// Push one platform update to the deployment.
    PushUpdate {
        /// The version the update carries.
        version: u32,
        /// The content hash of the update.
        digest: [u8; 32],
    },
    /// Read the records of one customer collection.
    ReadRecords {
        /// The collection the request names.
        collection: BoundedName,
    },
    /// Change one record of a customer collection.
    WriteRecord {
        /// The collection the request names.
        collection: BoundedName,
    },
    /// Grant one capability inside customer governance.
    GrantPermission {
        /// The role the request names.
        role: BoundedName,
    },
    /// Change the Blueprint of the customer.
    ChangeBlueprint {
        /// The content address the request names.
        blueprint: [u8; 32],
    },
}

impl OperatorRequest {
    /// Report whether the request sits inside the scope of the role.
    pub fn in_scope(&self) -> bool {
        matches!(
            self,
            OperatorRequest::UpgradePlatform { .. }
                | OperatorRequest::DowngradePlatform { .. }
                | OperatorRequest::VerifyLicence
                | OperatorRequest::SuspendOnLapse
                | OperatorRequest::PushUpdate { .. }
        )
    }

    /// The collection the request reaches.
    ///
    /// A permitted request reaches the reserved platform scope. Any other
    /// request reaches a customer collection, which the permission module
    /// refuses to the operator role.
    pub fn scope(&self) -> CollectionId {
        match self {
            OperatorRequest::ReadRecords { collection }
            | OperatorRequest::WriteRecord { collection } => {
                CollectionId::new(&name_text(collection))
            }
            OperatorRequest::GrantPermission { role } => CollectionId::new(&name_text(role)),
            OperatorRequest::ChangeBlueprint { .. } => CollectionId::new("cerulea_blueprint"),
            _ => CollectionId::new(PLATFORM_SCOPE),
        }
    }

    /// The action the request performs over that collection.
    pub fn access(&self) -> AccessAction {
        match self {
            OperatorRequest::ReadRecords { .. } => AccessAction::Read,
            OperatorRequest::WriteRecord { .. } => AccessAction::Update,
            OperatorRequest::GrantPermission { .. } => AccessAction::Update,
            OperatorRequest::ChangeBlueprint { .. } => AccessAction::Update,
            _ => AccessAction::Invoke(ActionId::new("platform_maintenance")),
        }
    }

    /// The event name the audit log records the request under.
    pub fn event(&self) -> &'static str {
        match self {
            OperatorRequest::UpgradePlatform { .. } => "platform_upgraded",
            OperatorRequest::DowngradePlatform { .. } => "platform_downgraded",
            OperatorRequest::VerifyLicence => "licence_checked",
            OperatorRequest::SuspendOnLapse => "suspension_on_lapse",
            OperatorRequest::PushUpdate { .. } => "platform_update_pushed",
            OperatorRequest::ReadRecords { .. } => "customer_record_read_refused",
            OperatorRequest::WriteRecord { .. } => "customer_record_write_refused",
            OperatorRequest::GrantPermission { .. } => "customer_permission_refused",
            OperatorRequest::ChangeBlueprint { .. } => "blueprint_change_refused",
        }
    }

    /// A short phrase naming the request for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            OperatorRequest::UpgradePlatform { .. } => "platform version upgrade",
            OperatorRequest::DowngradePlatform { .. } => "platform version downgrade",
            OperatorRequest::VerifyLicence => "licence verification",
            OperatorRequest::SuspendOnLapse => "suspension on licence lapse",
            OperatorRequest::PushUpdate { .. } => "platform update",
            OperatorRequest::ReadRecords { .. } => "read of customer records",
            OperatorRequest::WriteRecord { .. } => "write of customer data",
            OperatorRequest::GrantPermission { .. } => "change to customer governance",
            OperatorRequest::ChangeBlueprint { .. } => "change to the Blueprint",
        }
    }
}

/// What the module did with one request.
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
pub enum Outcome {
    /// The request ran.
    Carried,
    /// The request sat outside the scope of the role and aborted.
    OutsideScope,
    /// The permission module refused the request and it aborted.
    PermissionDenied,
}

impl Outcome {
    /// A word naming the outcome for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            Outcome::Carried => "carried",
            Outcome::OutsideScope => "outside scope",
            Outcome::PermissionDenied => "permission denied",
        }
    }
}

/// One appointed operator.
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
pub struct OperatorRecord<BlockNumber> {
    /// The wall clock time the appointment landed at.
    pub appointed_at: u64,
    /// The block the appointment landed in.
    pub appointed_block: BlockNumber,
    /// An operator left inactive reaches nothing.
    pub active: bool,
}

/// One request the module handled.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct RequestRecord<AccountId, BlockNumber> {
    /// The account the Blockchain records as the actor.
    pub actor: AccountId,
    /// What the operator asked for.
    pub request: OperatorRequest,
    /// What the module did with the request.
    pub outcome: Outcome,
    /// One plain sentence stating why the operator acted.
    pub reason: BoundedVec<u8, ConstU32<256>>,
    /// The wall clock time the request landed at.
    pub at_millis: u64,
    /// The block the request landed in.
    pub block: BlockNumber,
}

impl<AccountId, BlockNumber> RequestRecord<AccountId, BlockNumber> {
    /// The reason sentence as text.
    pub fn reason_text(&self) -> String {
        String::from_utf8_lossy(self.reason.as_slice()).into_owned()
    }
}

/// The read a dependant module performs over the operator role.
pub trait OperatorRole {
    /// The account type the runtime carries.
    type AccountId;

    /// Report whether one account holds the operator role of a deployment.
    fn is_operator(deployment: &DeploymentId, who: &Self::AccountId) -> bool;

    /// Report whether one request sits inside the scope of the role.
    fn permits(request: &OperatorRequest) -> bool;
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the operator module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The resolver that enforces the limits of the role.
        type Permissions: PermissionGate<AccountId = Self::AccountId>;

        /// The platform version registry the role moves.
        type Versions: PlatformVersions<AccountId = Self::AccountId>;

        /// The licence control the role reaches.
        type Licences: LicenceControl<AccountId = Self::AccountId>;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every record stamps itself with.
        type Time: UnixTime;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// Every appointed operator.
    #[pallet::storage]
    pub type Operators<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        OperatorRecord<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// Every request the module handled.
    #[pallet::storage]
    pub type Requests<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        RequestRecord<T::AccountId, BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The identifier the next request takes.
    #[pallet::storage]
    pub type NextRequest<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    /// The operators a chain specification appoints at genesis.
    ///
    /// Every customer deployment carries a Cerulea operator role from the
    /// first block. A chain specification therefore names the account and
    /// the deployment the role covers.
    #[pallet::genesis_config]
    #[derive(frame_support::DefaultNoBound)]
    pub struct GenesisConfig<T: Config> {
        /// The deployment and account pairs the chain appoints.
        pub operators: alloc::vec::Vec<(DeploymentId, T::AccountId)>,
    }

    #[pallet::genesis_build]
    impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
        fn build(&self) {
            for (deployment, operator) in self.operators.iter() {
                Operators::<T>::insert(
                    deployment,
                    operator,
                    OperatorRecord {
                        appointed_at: 0,
                        appointed_block: frame_system::Pallet::<T>::block_number(),
                        active: true,
                    },
                );
                Pallet::<T>::deposit_event(Event::OperatorAppointed {
                    deployment: *deployment,
                    operator: operator.clone(),
                });
            }
        }
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// An account took the Cerulea operator role.
        OperatorAppointed {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            operator: T::AccountId,
        },
        /// An account left the Cerulea operator role.
        OperatorRetired {
            /// The deployment.
            deployment: DeploymentId,
            /// The account.
            operator: T::AccountId,
        },
        /// An operator request ran.
        RequestCarried {
            /// The deployment.
            deployment: DeploymentId,
            /// The request.
            request: u64,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
        },
        /// An operator request aborted.
        RequestRefused {
            /// The deployment.
            deployment: DeploymentId,
            /// The request.
            request: u64,
            /// The account the Blockchain records as the actor.
            actor: T::AccountId,
            /// What the module did with the request.
            outcome: Outcome,
        },
        /// The platform version of a deployment changed.
        PlatformVersionChanged {
            /// The deployment.
            deployment: DeploymentId,
            /// The version the deployment runs now.
            version: u32,
        },
        /// Block production stopped because the licence lapsed.
        SuspendedOnLapse {
            /// The deployment.
            deployment: DeploymentId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The account holds no operator role for that deployment. Appoint
        /// the operator before it acts.
        NotAnOperator,
        /// The registry already holds that operator. Retire the operator
        /// before appointing it again.
        OperatorPresent,
        /// The reason sentence is empty. State why the operator acts.
        ReasonMissing,
        /// The deployment holds no licence. Install the licence the
        /// control plane issued.
        LicenceMissing,
        /// The deployment reached its last request identifier. Start a new
        /// deployment.
        RegisterFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Appoint one account to the Cerulea operator role.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::appoint_operator())]
        pub fn appoint_operator(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            operator: T::AccountId,
        ) -> DispatchResult {
            let who = Self::ensure_appointing(origin, &deployment, &operator)?;
            ensure!(
                Operators::<T>::get(deployment, &operator).is_none(),
                Error::<T>::OperatorPresent
            );
            Operators::<T>::insert(
                deployment,
                &operator,
                OperatorRecord {
                    appointed_at: Self::now_millis(),
                    appointed_block: frame_system::Pallet::<T>::block_number(),
                    active: true,
                },
            );
            Self::record(
                &deployment,
                "operator_appointed",
                WorkClass::Constitutional,
                &who,
                "The deployment appointed one account to the Cerulea operator role.",
                operator.encode(),
            )?;
            Self::deposit_event(Event::OperatorAppointed {
                deployment,
                operator,
            });
            Ok(())
        }

        /// Take one account out of the Cerulea operator role.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::retire_operator())]
        pub fn retire_operator(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            operator: T::AccountId,
        ) -> DispatchResult {
            let who = Self::ensure_appointing(origin, &deployment, &operator)?;
            Operators::<T>::try_mutate(deployment, &operator, |slot| -> DispatchResult {
                let held = slot.as_mut().ok_or(Error::<T>::NotAnOperator)?;
                held.active = false;
                Ok(())
            })?;
            Self::record(
                &deployment,
                "operator_retired",
                WorkClass::Constitutional,
                &who,
                "The deployment took one account out of the Cerulea operator role.",
                operator.encode(),
            )?;
            Self::deposit_event(Event::OperatorRetired {
                deployment,
                operator,
            });
            Ok(())
        }

        /// Handle one operator request.
        ///
        /// A request inside the scope of the role runs. A request outside
        /// the scope aborts and lands in the audit log with the actor, the
        /// timestamp and the reason. The call itself answers with success
        /// in both cases, so the record of the refusal stays.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::request())]
        pub fn request(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            request: OperatorRequest,
            reason: BoundedVec<u8, ConstU32<256>>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            ensure!(!reason.is_empty(), Error::<T>::ReasonMissing);
            let held = Operators::<T>::get(deployment, &who).ok_or(Error::<T>::NotAnOperator)?;
            ensure!(held.active, Error::<T>::NotAnOperator);
            Self::handle(&deployment, &who, request, reason)
        }
    }
}

impl<T: Config> Pallet<T> {
    /// Check the origin of an appointment or a retirement.
    ///
    /// The root origin appoints and retires, which is the path the chain
    /// specification and the Amendment Engine take. An active operator of the
    /// deployment appoints and retires beside it. Every other account meets
    /// `NotAnOperator`. The answer names the account the audit record carries.
    fn ensure_appointing(
        origin: OriginFor<T>,
        deployment: &DeploymentId,
        operator: &T::AccountId,
    ) -> Result<T::AccountId, DispatchError> {
        match ensure_signed_or_root(origin)? {
            Some(who) => {
                ensure!(
                    <Self as OperatorRole>::is_operator(deployment, &who),
                    Error::<T>::NotAnOperator
                );
                Ok(who)
            }
            None => Ok(operator.clone()),
        }
    }

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

    /// Handle one request from an appointed operator.
    fn handle(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        request: OperatorRequest,
        reason: BoundedVec<u8, ConstU32<256>>,
    ) -> DispatchResult {
        let now = Self::now_millis();
        let sentence = String::from_utf8_lossy(reason.as_slice()).into_owned();

        // The permission module enforces the limits of the role. A request
        // naming a customer collection resolves to a denial.
        let scope = request.scope();
        let probe = AccessRequest::new(
            *deployment,
            actor,
            scope,
            request.access(),
            Timestamp::from_millis(now),
        );
        let admitted = T::Permissions::resolve(&probe).is_ok();

        let outcome = if !admitted {
            Outcome::PermissionDenied
        } else if !request.in_scope() {
            Outcome::OutsideScope
        } else {
            Self::carry(deployment, actor, &request, &sentence)?;
            Outcome::Carried
        };

        let id = NextRequest::<T>::get(deployment);
        let next = id.checked_add(1).ok_or(Error::<T>::RegisterFull)?;
        Requests::<T>::insert(
            deployment,
            id,
            RequestRecord {
                actor: actor.clone(),
                request: request.clone(),
                outcome,
                reason: reason.clone(),
                at_millis: now,
                block: frame_system::Pallet::<T>::block_number(),
            },
        );
        NextRequest::<T>::insert(deployment, next);

        Self::record(
            deployment,
            request.event(),
            WorkClass::Constitutional,
            actor,
            &sentence,
            (request.label(), outcome.label(), now).encode(),
        )?;

        if outcome == Outcome::Carried {
            Self::deposit_event(Event::RequestCarried {
                deployment: *deployment,
                request: id,
                actor: actor.clone(),
            });
        } else {
            Self::deposit_event(Event::RequestRefused {
                deployment: *deployment,
                request: id,
                actor: actor.clone(),
                outcome,
            });
        }
        Ok(())
    }

    /// Run one request that sits inside the scope of the role.
    fn carry(
        deployment: &DeploymentId,
        actor: &T::AccountId,
        request: &OperatorRequest,
        reason: &str,
    ) -> DispatchResult {
        match request {
            OperatorRequest::UpgradePlatform { version }
            | OperatorRequest::DowngradePlatform { version }
            | OperatorRequest::PushUpdate { version, .. } => {
                T::Versions::set_platform_version(deployment, *version, actor, reason)?;
                Self::deposit_event(Event::PlatformVersionChanged {
                    deployment: *deployment,
                    version: *version,
                });
            }
            OperatorRequest::VerifyLicence => {
                let held = T::Licences::verify(deployment, actor)?;
                if !held {
                    T::Licences::suspend_on_lapse(deployment, actor)?;
                    Self::deposit_event(Event::SuspendedOnLapse {
                        deployment: *deployment,
                    });
                }
            }
            OperatorRequest::SuspendOnLapse => {
                if T::Licences::suspend_on_lapse(deployment, actor)? {
                    Self::deposit_event(Event::SuspendedOnLapse {
                        deployment: *deployment,
                    });
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// One stored request.
    pub fn request_record(
        deployment: &DeploymentId,
        id: u64,
    ) -> Option<RequestRecord<T::AccountId, BlockNumberFor<T>>> {
        Requests::<T>::get(deployment, id)
    }

    /// The number of requests the module handled.
    pub fn request_count(deployment: &DeploymentId) -> u64 {
        NextRequest::<T>::get(deployment)
    }
}

impl<T: Config> OperatorRole for Pallet<T> {
    type AccountId = T::AccountId;

    fn is_operator(deployment: &DeploymentId, who: &Self::AccountId) -> bool {
        Operators::<T>::get(deployment, who)
            .map(|held| held.active)
            .unwrap_or(false)
    }

    fn permits(request: &OperatorRequest) -> bool {
        request.in_scope()
    }
}
