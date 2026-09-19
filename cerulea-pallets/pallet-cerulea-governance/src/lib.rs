//! The Cerulea governance module.
//!
//! The governance module holds the change process of a deployed system.
//! Five operations drive a change. Propose. Vote. Tally. Execute. Cancel.
//!
//! The scope covers five kinds of change. A schema change. A permission
//! change. A policy amendment. A validator set change. An upgrade
//! approval.
//!
//! Each kind carries a class of work, and each class carries a threshold.
//! Routine work needs 51 percent. Financial work needs 67 percent.
//! Structural work needs 80 percent. Constitutional work needs 90 percent
//! and a mandatory delay between approval and effect.
//!
//! The Governance section of the Blueprint raises a threshold above the
//! floor of its class. A deployment that declares no rule for a class
//! carries the floor of that class.
//!
//! An approved proposal takes effect once a module executes it. The module
//! that acts asks the approval gate first, so the module never acts on a
//! change the electorate declined.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod roll;
pub mod weights;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use roll::{RegisteredElectorate, VoterRoll};
pub use weights::WeightInfo;

use alloc::vec::Vec;
use cerulea_blueprint::{actions::TransactionClass, governance::Governance};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_schema::primitives::{name_of, BoundedName, DeploymentId};

/// The name the module carries in the audit log.
pub const MODULE_NAME: &str = "governance";

/// What one proposal changes.
///
/// The five kinds are the whole scope of the change process. A change
/// outside the five kinds passes through no proposal.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum ProposalKind {
    /// A change to the registered types and collections.
    SchemaChange,
    /// A change to the capability matrix.
    PermissionChange,
    /// A change to the rules the deployment runs under.
    PolicyAmendment,
    /// A change to the validator set.
    ValidatorSetChange,
    /// Approval of a platform version.
    UpgradeApproval,
}

impl ProposalKind {
    /// The class of work the kind carries.
    ///
    /// A schema change, a permission change and a validator set change all
    /// change the shape of the deployment, so each carries the structural
    /// class. A policy amendment and an upgrade approval change the rules
    /// of the deployment, so each carries the constitutional class.
    pub fn class(&self) -> WorkClass {
        match self {
            ProposalKind::SchemaChange => WorkClass::Structural,
            ProposalKind::PermissionChange => WorkClass::Structural,
            ProposalKind::ValidatorSetChange => WorkClass::Structural,
            ProposalKind::PolicyAmendment => WorkClass::Constitutional,
            ProposalKind::UpgradeApproval => WorkClass::Constitutional,
        }
    }

    /// A word naming the kind for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            ProposalKind::SchemaChange => "schema change",
            ProposalKind::PermissionChange => "permission change",
            ProposalKind::PolicyAmendment => "policy amendment",
            ProposalKind::ValidatorSetChange => "validator set change",
            ProposalKind::UpgradeApproval => "upgrade approval",
        }
    }
}

/// Where one proposal stands.
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
pub enum ProposalStatus {
    /// The proposal takes votes.
    Open,
    /// The electorate agreed and the proposal waits for its execution.
    Approved,
    /// The electorate declined the proposal.
    Rejected,
    /// A module carried the change out.
    Executed,
    /// The proposer withdrew the proposal.
    Cancelled,
}

/// The agreement one class of change requires.
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
pub struct QuorumSetting {
    /// The share of the electorate that agrees, in percent.
    pub threshold_percent: u8,
    /// The count of voters that take part.
    pub minimum_voters: u32,
    /// The span in milliseconds between approval and effect.
    pub delay_millis: u64,
}

impl QuorumSetting {
    /// The setting one class carries by default.
    pub fn floor(class: WorkClass) -> Self {
        QuorumSetting {
            threshold_percent: class.threshold_percent(),
            minimum_voters: 1,
            delay_millis: if class.holds_delay() { 86_400_000 } else { 0 },
        }
    }
}

/// One proposal with its tally and its execution record.
#[derive(Clone, Debug, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen)]
#[scale_info(skip_type_params(MaxPayload))]
pub struct Proposal<AccountId, BlockNumber, MaxPayload>
where
    MaxPayload: Get<u32> + 'static,
{
    /// What the proposal changes.
    pub kind: ProposalKind,
    /// The class of work the change carries.
    pub class: WorkClass,
    /// The canonical encoding of the change.
    pub payload: BoundedVec<u8, MaxPayload>,
    /// The content hash of the payload.
    pub payload_hash: [u8; 32],
    /// The account that opened the proposal.
    pub proposer: AccountId,
    /// The wall clock time the proposal opened at.
    pub opened_at: u64,
    /// The wall clock time the vote closes at.
    pub deadline: u64,
    /// Where the proposal stands.
    pub status: ProposalStatus,
    /// The weight that agrees.
    pub approve_weight: u64,
    /// The weight that declines.
    pub reject_weight: u64,
    /// The count of voters that took part.
    pub voters: u32,
    /// The weight of the whole electorate at the moment of the decision.
    pub roll_weight: u64,
    /// The wall clock time the decision landed at.
    pub decided_at: Option<u64>,
    /// The wall clock time the change takes effect from.
    pub effective_at: Option<u64>,
    /// The account that carried the change out.
    pub executed_by: Option<AccountId>,
    /// The block the execution landed in.
    pub executed_block: Option<BlockNumber>,
}

/// One vote with its weight.
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
pub struct Vote {
    /// The voter agreed.
    pub approve: bool,
    /// The weight the voter carried.
    pub weight: u64,
    /// The wall clock time the vote landed at.
    pub at_millis: u64,
}

/// The read a dependant module performs over the approved changes.
///
/// The evolution module, the operator module and every other module that
/// carries a change out asks this gate first.
pub trait ApprovalGate {
    /// Report whether the electorate approved one change and a module
    /// carried it out.
    fn is_executed(deployment: &DeploymentId, kind: ProposalKind, payload: &[u8]) -> bool;

    /// Report whether the electorate approved one change.
    ///
    /// The answer holds for an approved proposal and for an executed
    /// proposal.
    fn is_approved(deployment: &DeploymentId, kind: ProposalKind, payload: &[u8]) -> bool;

    /// The share of the electorate one class of change requires.
    fn threshold_for(deployment: &DeploymentId, class: WorkClass) -> u8;
}

/// The content hash of one proposal payload.
pub fn payload_hash(kind: ProposalKind, payload: &[u8]) -> [u8; 32] {
    sp_crypto_hashing::blake2_256(&(b"cerulea.governance.v1", kind, payload).encode())
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the governance module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The electorate the tally reads.
        type Roll: VoterRoll<AccountId = Self::AccountId>;

        /// The log the module records its work in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The clock every deadline and every delay reads.
        type Time: UnixTime;

        /// The largest proposal payload, in bytes.
        #[pallet::constant]
        type MaxPayloadBytes: Get<u32>;

        /// The largest number of roles the upgrade authority names.
        #[pallet::constant]
        type MaxAuthorityRoles: Get<u32>;

        /// The largest span between the opening of a vote and its
        /// deadline, in milliseconds.
        #[pallet::constant]
        type MaxVoteSpan: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The agreement each class of change requires, per deployment.
    #[pallet::storage]
    pub type Quorum<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        WorkClass,
        QuorumSetting,
        OptionQuery,
    >;

    /// The roles the upgrade authority names, per deployment.
    #[pallet::storage]
    pub type Authority<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxAuthorityRoles>,
        ValueQuery,
    >;

    /// Every proposal.
    #[pallet::storage]
    pub type Proposals<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Twox64Concat,
        u64,
        Proposal<T::AccountId, BlockNumberFor<T>, T::MaxPayloadBytes>,
        OptionQuery,
    >;

    /// The identifier the next proposal takes.
    #[pallet::storage]
    pub type NextProposal<T: Config> =
        StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    /// Every vote, keyed by deployment, proposal and voter.
    #[pallet::storage]
    pub type Votes<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Twox64Concat, u64>,
            NMapKey<Blake2_128Concat, T::AccountId>,
        ),
        Vote,
        OptionQuery,
    >;

    /// The proposal that carried out each change, keyed by the kind and
    /// the content hash of the payload.
    #[pallet::storage]
    pub type ExecutedChanges<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Twox64Concat, ProposalKind>,
            NMapKey<Blake2_128Concat, [u8; 32]>,
        ),
        u64,
        OptionQuery,
    >;

    /// The proposal that holds approval for each change.
    #[pallet::storage]
    pub type ApprovedChanges<T: Config> = StorageNMap<
        _,
        (
            NMapKey<Blake2_128Concat, DeploymentId>,
            NMapKey<Twox64Concat, ProposalKind>,
            NMapKey<Blake2_128Concat, [u8; 32]>,
        ),
        u64,
        OptionQuery,
    >;

    /// The weight each member of the electorate carries.
    #[pallet::storage]
    pub type Electorate<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        Blake2_128Concat,
        T::AccountId,
        u64,
        OptionQuery,
    >;

    /// The weight of the whole electorate, per deployment.
    #[pallet::storage]
    pub type RollWeight<T: Config> = StorageMap<_, Blake2_128Concat, DeploymentId, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// The Governance section of the Blueprint entered the module.
        GovernanceInstalled {
            /// The deployment.
            deployment: DeploymentId,
            /// The number of quorum rules the section declares.
            rules: u32,
        },
        /// One member of the electorate took a weight.
        VoterWeighted {
            /// The deployment.
            deployment: DeploymentId,
            /// The voter.
            voter: T::AccountId,
            /// The weight the voter carries now.
            weight: u64,
        },
        /// A proposal opened.
        ProposalOpened {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
            /// What the proposal changes.
            kind: ProposalKind,
            /// The account that opened the proposal.
            proposer: T::AccountId,
        },
        /// A vote landed.
        VoteCast {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
            /// The voter.
            voter: T::AccountId,
            /// The voter agreed.
            approve: bool,
            /// The weight the voter carried.
            weight: u64,
        },
        /// The electorate agreed.
        ProposalApproved {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
            /// The wall clock time the change takes effect from.
            effective_at: u64,
        },
        /// The electorate declined.
        ProposalRejected {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
        },
        /// A module carried the change out.
        ProposalExecuted {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
            /// The account that carried the change out.
            executor: T::AccountId,
        },
        /// The proposer withdrew the proposal.
        ProposalCancelled {
            /// The deployment.
            deployment: DeploymentId,
            /// The proposal.
            proposal: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The name runs past 128 bytes. Shorten the role name.
        NameTooLong,
        /// The section failed to decode. Submit the canonical encoding the
        /// Studio produces.
        SectionUnreadable,
        /// The section names more roles than the runtime admits. Name
        /// fewer roles, or raise the limit in the runtime.
        AuthorityTooWide,
        /// The section declares a threshold below the floor of its class.
        /// Raise the threshold to the floor of the class or above it.
        ThresholdBelowFloor,
        /// The module holds no proposal under that identifier. Open a
        /// proposal before voting on it.
        ProposalUnknown,
        /// The proposal takes no further vote. Open a new proposal.
        ProposalClosed,
        /// The vote closed at its deadline. Open a new proposal.
        DeadlinePassed,
        /// The deadline sits at or before the present moment. Set a
        /// deadline in the future.
        DeadlineTooEarly,
        /// The deadline sits past the largest vote span. Set a nearer
        /// deadline, or raise the span in the runtime.
        DeadlineTooLate,
        /// The account carries no weight in the electorate. Ask the
        /// steward to give the account its weight.
        VoterUnknown,
        /// The account already voted on this proposal. Cancel the vote
        /// before voting again.
        VoteAlreadyCast,
        /// The electorate carries no weight. Give at least one account its
        /// weight before opening a vote.
        RollEmpty,
        /// The proposal waits for its decision. Call the tally before
        /// executing the change.
        ProposalUndecided,
        /// The mandatory delay has yet to pass. Wait for the delay of the
        /// class, then execute the change.
        DelayPending,
        /// The caller opened no proposal under that identifier. Ask the
        /// proposer to withdraw the proposal.
        NotTheProposer,
        /// The deployment reached its last proposal identifier. Start a
        /// new deployment.
        RegisterFull,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Install the Governance section of the Blueprint.
        ///
        /// The section raises a threshold above the floor of its class. A
        /// class the section leaves out carries the floor of that class.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::install_governance())]
        pub fn install_governance(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            section: BoundedVec<u8, T::MaxPayloadBytes>,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let parsed = Governance::decode(&mut section.as_slice())
                .map_err(|_| Error::<T>::SectionUnreadable)?;

            let mut roles: BoundedVec<BoundedName, T::MaxAuthorityRoles> = BoundedVec::default();
            for role in parsed.authority.roles.iter() {
                let name = name_of(role.as_str()).map_err(|_| Error::<T>::NameTooLong)?;
                roles
                    .try_push(name)
                    .map_err(|_| Error::<T>::AuthorityTooWide)?;
            }

            let mut rules = 0u32;
            for rule in parsed.quorum.iter() {
                let class: WorkClass = rule.class.into();
                ensure!(
                    rule.threshold_percent >= class.threshold_percent(),
                    Error::<T>::ThresholdBelowFloor
                );
                Quorum::<T>::insert(
                    deployment,
                    class,
                    QuorumSetting {
                        threshold_percent: rule.threshold_percent,
                        minimum_voters: rule.minimum_voters,
                        delay_millis: rule
                            .delay_millis
                            .max(QuorumSetting::floor(class).delay_millis),
                    },
                );
                rules = rules.saturating_add(1);
            }
            Authority::<T>::insert(deployment, roles);

            Self::record(
                &deployment,
                "governance_installed",
                WorkClass::Constitutional,
                &who,
                "The steward installed the governance section of the Blueprint.",
                rules.encode(),
            )?;
            Self::deposit_event(Event::GovernanceInstalled { deployment, rules });
            Ok(())
        }

        /// Give one account its weight in the electorate.
        ///
        /// A weight of zero takes the account out of the electorate.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::set_voter_weight())]
        pub fn set_voter_weight(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            voter: T::AccountId,
            weight: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let held = Electorate::<T>::get(deployment, &voter).unwrap_or(0);
            let total = RollWeight::<T>::get(deployment)
                .saturating_sub(held)
                .saturating_add(weight);
            if weight == 0 {
                Electorate::<T>::remove(deployment, &voter);
            } else {
                Electorate::<T>::insert(deployment, &voter, weight);
            }
            RollWeight::<T>::insert(deployment, total);

            Self::record(
                &deployment,
                "voter_weighted",
                WorkClass::Structural,
                &who,
                "The steward set the weight one account carries in the electorate.",
                (voter.clone(), weight).encode(),
            )?;
            Self::deposit_event(Event::VoterWeighted {
                deployment,
                voter,
                weight,
            });
            Ok(())
        }

        /// Open one proposal.
        ///
        /// The kind of change carries the class of work, and the class
        /// carries the threshold the proposal reaches before it lands.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::propose())]
        pub fn propose(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            kind: ProposalKind,
            payload: BoundedVec<u8, T::MaxPayloadBytes>,
            deadline: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now = Self::now_millis();
            ensure!(deadline > now, Error::<T>::DeadlineTooEarly);
            ensure!(
                deadline.saturating_sub(now) <= T::MaxVoteSpan::get(),
                Error::<T>::DeadlineTooLate
            );
            ensure!(
                T::Roll::total_weight(&deployment) > 0,
                Error::<T>::RollEmpty
            );

            let id = NextProposal::<T>::get(deployment);
            let next = id.checked_add(1).ok_or(Error::<T>::RegisterFull)?;
            let class = kind.class();
            let hash = payload_hash(kind, payload.as_slice());

            Proposals::<T>::insert(
                deployment,
                id,
                Proposal {
                    kind,
                    class,
                    payload,
                    payload_hash: hash,
                    proposer: who.clone(),
                    opened_at: now,
                    deadline,
                    status: ProposalStatus::Open,
                    approve_weight: 0,
                    reject_weight: 0,
                    voters: 0,
                    roll_weight: T::Roll::total_weight(&deployment),
                    decided_at: None,
                    effective_at: None,
                    executed_by: None,
                    executed_block: None,
                },
            );
            NextProposal::<T>::insert(deployment, next);

            Self::record(
                &deployment,
                "proposal_opened",
                class,
                &who,
                "A member of the electorate opened one proposal.",
                (id, kind, hash).encode(),
            )?;
            Self::deposit_event(Event::ProposalOpened {
                deployment,
                proposal: id,
                kind,
                proposer: who,
            });
            Ok(())
        }

        /// Cast one weighted vote.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::vote())]
        pub fn vote(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            proposal: u64,
            approve: bool,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now = Self::now_millis();
            let weight = T::Roll::weight_of(&deployment, &who);
            ensure!(weight > 0, Error::<T>::VoterUnknown);
            ensure!(
                Votes::<T>::get((deployment, proposal, who.clone())).is_none(),
                Error::<T>::VoteAlreadyCast
            );

            Proposals::<T>::try_mutate(deployment, proposal, |slot| -> DispatchResult {
                let held = slot.as_mut().ok_or(Error::<T>::ProposalUnknown)?;
                ensure!(
                    held.status == ProposalStatus::Open,
                    Error::<T>::ProposalClosed
                );
                ensure!(now <= held.deadline, Error::<T>::DeadlinePassed);
                if approve {
                    held.approve_weight = held.approve_weight.saturating_add(weight);
                } else {
                    held.reject_weight = held.reject_weight.saturating_add(weight);
                }
                held.voters = held.voters.saturating_add(1);
                Ok(())
            })?;
            Votes::<T>::insert(
                (deployment, proposal, who.clone()),
                Vote {
                    approve,
                    weight,
                    at_millis: now,
                },
            );

            Self::record(
                &deployment,
                "vote_cast",
                WorkClass::Routine,
                &who,
                "A member of the electorate cast one weighted vote.",
                (proposal, approve, weight).encode(),
            )?;
            Self::deposit_event(Event::VoteCast {
                deployment,
                proposal,
                voter: who,
                approve,
                weight,
            });
            Ok(())
        }

        /// Settle one proposal against the quorum of its class.
        ///
        /// The call approves a proposal that reached its threshold. The
        /// call declines a proposal that passed its deadline without
        /// reaching the threshold. The call also declines a proposal whose
        /// remaining weight can no longer reach the threshold.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::tally())]
        pub fn tally(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            proposal: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            Self::settle(&deployment, proposal, &who)
        }

        /// Record that a module carried one approved change out.
        ///
        /// The call runs once the mandatory delay of the class has passed.
        /// The module that acts asks the approval gate before it acts.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::execute())]
        pub fn execute(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            proposal: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let now = Self::now_millis();
            let (kind, hash, class) = Proposals::<T>::try_mutate(
                deployment,
                proposal,
                |slot| -> Result<_, DispatchError> {
                    let held = slot.as_mut().ok_or(Error::<T>::ProposalUnknown)?;
                    ensure!(
                        held.status == ProposalStatus::Approved,
                        Error::<T>::ProposalUndecided
                    );
                    let effective = held.effective_at.unwrap_or(now);
                    ensure!(now >= effective, Error::<T>::DelayPending);
                    held.status = ProposalStatus::Executed;
                    held.executed_by = Some(who.clone());
                    held.executed_block = Some(frame_system::Pallet::<T>::block_number());
                    Ok((held.kind, held.payload_hash, held.class))
                },
            )?;

            ExecutedChanges::<T>::insert((deployment, kind, hash), proposal);
            Self::record(
                &deployment,
                "proposal_executed",
                class,
                &who,
                "A module carried out one approved change.",
                (proposal, kind, hash).encode(),
            )?;
            Self::deposit_event(Event::ProposalExecuted {
                deployment,
                proposal,
                executor: who,
            });
            Ok(())
        }

        /// Withdraw one open proposal.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::cancel())]
        pub fn cancel(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            proposal: u64,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let (kind, hash) = Proposals::<T>::try_mutate(
                deployment,
                proposal,
                |slot| -> Result<_, DispatchError> {
                    let held = slot.as_mut().ok_or(Error::<T>::ProposalUnknown)?;
                    ensure!(
                        matches!(held.status, ProposalStatus::Open | ProposalStatus::Approved),
                        Error::<T>::ProposalClosed
                    );
                    ensure!(held.proposer == who, Error::<T>::NotTheProposer);
                    held.status = ProposalStatus::Cancelled;
                    held.decided_at = Some(Self::now_millis());
                    Ok((held.kind, held.payload_hash))
                },
            )?;
            ApprovedChanges::<T>::remove((deployment, kind, hash));

            Self::record(
                &deployment,
                "proposal_cancelled",
                WorkClass::Routine,
                &who,
                "The proposer withdrew one proposal.",
                proposal.encode(),
            )?;
            Self::deposit_event(Event::ProposalCancelled {
                deployment,
                proposal,
            });
            Ok(())
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

    /// The agreement one class of change requires in one deployment.
    pub fn quorum_for(deployment: &DeploymentId, class: WorkClass) -> QuorumSetting {
        Quorum::<T>::get(deployment, class).unwrap_or_else(|| QuorumSetting::floor(class))
    }

    /// One stored proposal.
    pub fn proposal(
        deployment: &DeploymentId,
        id: u64,
    ) -> Option<Proposal<T::AccountId, BlockNumberFor<T>, T::MaxPayloadBytes>> {
        Proposals::<T>::get(deployment, id)
    }

    /// One stored vote.
    pub fn vote_of(deployment: &DeploymentId, id: u64, voter: &T::AccountId) -> Option<Vote> {
        Votes::<T>::get((*deployment, id, voter.clone()))
    }

    /// Settle one proposal against the quorum of its class.
    fn settle(deployment: &DeploymentId, id: u64, actor: &T::AccountId) -> DispatchResult {
        let now = Self::now_millis();
        let outcome =
            Proposals::<T>::try_mutate(deployment, id, |slot| -> Result<_, DispatchError> {
                let held = slot.as_mut().ok_or(Error::<T>::ProposalUnknown)?;
                ensure!(
                    held.status == ProposalStatus::Open,
                    Error::<T>::ProposalClosed
                );
                let rule = Self::quorum_for(deployment, held.class);
                let total = T::Roll::total_weight(deployment).max(held.roll_weight);
                held.roll_weight = total;

                let share = u128::from(held.approve_weight).saturating_mul(100);
                let needed = u128::from(total).saturating_mul(u128::from(rule.threshold_percent));
                let reached = share >= needed && held.voters >= rule.minimum_voters;

                let outstanding = total
                    .saturating_sub(held.approve_weight)
                    .saturating_sub(held.reject_weight);
                let best =
                    u128::from(held.approve_weight.saturating_add(outstanding)).saturating_mul(100);
                let unreachable = best < needed;

                if reached {
                    held.status = ProposalStatus::Approved;
                    held.decided_at = Some(now);
                    held.effective_at = Some(now.saturating_add(rule.delay_millis));
                    return Ok(Some(true));
                }
                if unreachable || now > held.deadline {
                    held.status = ProposalStatus::Rejected;
                    held.decided_at = Some(now);
                    return Ok(Some(false));
                }
                Ok(None)
            })?;

        match outcome {
            Some(true) => {
                let held =
                    Proposals::<T>::get(deployment, id).ok_or(Error::<T>::ProposalUnknown)?;
                ApprovedChanges::<T>::insert((*deployment, held.kind, held.payload_hash), id);
                Self::record(
                    deployment,
                    "proposal_approved",
                    held.class,
                    actor,
                    "The electorate agreed to one proposal.",
                    (id, held.kind, held.payload_hash).encode(),
                )?;
                Self::deposit_event(Event::ProposalApproved {
                    deployment: *deployment,
                    proposal: id,
                    effective_at: held.effective_at.unwrap_or(now),
                });
            }
            Some(false) => {
                Self::record(
                    deployment,
                    "proposal_rejected",
                    WorkClass::Routine,
                    actor,
                    "The electorate declined one proposal.",
                    id.encode(),
                )?;
                Self::deposit_event(Event::ProposalRejected {
                    deployment: *deployment,
                    proposal: id,
                });
            }
            None => {}
        }
        Ok(())
    }
}

impl<T: Config> ApprovalGate for Pallet<T> {
    fn is_executed(deployment: &DeploymentId, kind: ProposalKind, payload: &[u8]) -> bool {
        ExecutedChanges::<T>::contains_key((*deployment, kind, payload_hash(kind, payload)))
    }

    fn is_approved(deployment: &DeploymentId, kind: ProposalKind, payload: &[u8]) -> bool {
        let hash = payload_hash(kind, payload);
        ApprovedChanges::<T>::contains_key((*deployment, kind, hash))
            || ExecutedChanges::<T>::contains_key((*deployment, kind, hash))
    }

    fn threshold_for(deployment: &DeploymentId, class: WorkClass) -> u8 {
        Self::quorum_for(deployment, class).threshold_percent
    }
}

/// The class of work one Blueprint transaction class carries.
pub fn class_of(class: TransactionClass) -> WorkClass {
    class.into()
}
