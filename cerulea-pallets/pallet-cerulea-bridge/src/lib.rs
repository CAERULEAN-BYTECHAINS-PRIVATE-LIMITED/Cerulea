//! The Cerulea bridge module.
//!
//! The bridge module carries messages between one Cerulea deployment and a
//! named network outside it. Every message arrives with a proof. The
//! module holds one verifier per target network and declines a message
//! whose proof fails. Seven target networks ship together. Networks
//! running Ethereum Virtual Machine bytecode, Cosmos, Solana, Polkadot,
//! Avalanche, Bitcoin and Layer 2 rollups.
//!
//! # The four pieces of state
//!
//! 1. **The endpoint registry.** One record per endpoint. The record holds
//!    the network kind, the verifier parameters of that kind, the message
//!    format and the action an arriving message runs.
//! 2. **The accepted headers.** One record per far network height the
//!    endpoint accepted. A proof names a height and verifies against the
//!    roots of that height.
//! 3. **The message registers.** The inbound register and the outbound
//!    register hold every message with its status.
//! 4. **The replay registry.** One entry per endpoint and message
//!    identifier. A message lands once. A second attempt is declined.
//!
//! # The five operations
//!
//! Register an endpoint. Submit far network headers. Deliver an inbound
//! message with its proof. Produce an outbound message with its proof of
//! inclusion in Cerulea. Cancel a message that passed its expiry.
//!
//! # Replay protection in plain words
//!
//! Every message carries an identifier covering the deployment, the
//! endpoint, the two network names, the sending party, the position of the
//! message in the stream of that sender and the payload. The module writes
//! that identifier into the replay registry the moment it accepts the
//! message. A later attempt carrying the same identifier finds the entry
//! and stops. A message therefore runs once.
//!
//! # Expiry in plain words
//!
//! Every message carries a moment it stops being acceptable. The module
//! places the message in a bucket of that moment. A bounded sweep at the
//! start of each block walks the buckets that passed and cancels what it
//! finds. The sweep handles a fixed count of messages per block, so the
//! work of a block stays inside its budget.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod anchor;
pub mod endpoint;
pub mod envelope;
pub mod inbound;
pub mod merkle;
pub mod mpt;
pub mod networks;
pub mod outbound;
pub mod rlp;
pub mod sweep;
pub mod verify;
pub mod weights;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use alloc::{vec, vec::Vec};
use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use endpoint::{EndpointKey, EndpointRecord, HeaderRecord, HeaderSource, VerifierParameters};
use envelope::{
    BridgeFault, MessageEnvelope, MessageFormat, MessageId, NetworkKind, Travel, TrustAssumption,
};
use frame_support::{pallet_prelude::*, traits::UnixTime};
use frame_system::pallet_prelude::*;
use pallet_cerulea_audit::contract::{AuditRequest, AuditSink, WorkClass};
use pallet_cerulea_governance::{ApprovalGate, ProposalKind};
use pallet_cerulea_schema::primitives::{BoundedName, DeploymentId, RecordKey};
use pallet_cerulea_state::RecordStore;
use scale_info::TypeInfo;

/// The name the module records in the audit log.
pub const MODULE_NAME: &str = "bridge";

/// The proposal kind that authorises an endpoint change.
///
/// An endpoint change alters the shape of the deployment, so the
/// electorate agrees at the structural class threshold before the change
/// lands.
pub const ENDPOINT_PROPOSAL: ProposalKind = ProposalKind::SchemaChange;

/// The span of one expiry bucket, in milliseconds.
pub const BUCKET_MILLIS: u64 = 60_000;

/// Where one message stands.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Encode,
    Decode,
    DecodeWithMemTracking,
    TypeInfo,
    MaxEncodedLen,
)]
pub enum MessageStatus {
    /// The message waits for its next step.
    #[default]
    Pending,
    /// The far network took delivery of the message.
    Delivered,
    /// The deployment ran the action the message targets.
    Executed,
    /// The message passed its expiry and stopped.
    Cancelled,
}

impl MessageStatus {
    /// A word naming the status for the interface layer.
    pub fn label(&self) -> &'static str {
        match self {
            MessageStatus::Pending => "pending",
            MessageStatus::Delivered => "delivered",
            MessageStatus::Executed => "executed",
            MessageStatus::Cancelled => "cancelled",
        }
    }

    /// Report whether the status still admits a change.
    pub fn is_open(&self) -> bool {
        matches!(self, MessageStatus::Pending)
    }
}

/// One entry of the replay registry.
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
pub struct SeenRecord<BlockNumber> {
    /// The block the module accepted the message at.
    pub at: BlockNumber,
    /// The far network height the proof reached.
    pub height: u64,
    /// The direction the message travelled.
    pub travel: Travel,
}

/// One inbound message the module accepted.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct InboundRecord<BlockNumber> {
    /// The envelope the proof covered.
    pub envelope: MessageEnvelope,
    /// The far network height the proof reached.
    pub height: u64,
    /// The far network block hash the proof reached.
    pub block_hash: [u8; 32],
    /// The trust the acceptance rests on.
    pub trust: TrustAssumption,
    /// Where the message stands.
    pub status: MessageStatus,
    /// The block the module accepted the message at.
    pub accepted_at: BlockNumber,
    /// The count of effects the action ran.
    pub effects: u32,
}

/// One outbound message the module produced.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct OutboundRecord<BlockNumber> {
    /// The envelope the far network receives.
    pub envelope: MessageEnvelope,
    /// The digest the far network commits to.
    pub commitment: [u8; 32],
    /// Where the message stands.
    pub status: MessageStatus,
    /// The block the module produced the message at.
    pub created_at: BlockNumber,
    /// The reference the far network answered with.
    pub reference: [u8; 32],
}

/// The body one outbound message carries.
///
/// The body names the record the message covers, carries the stored bytes
/// and carries the proof that the record sits in the collection. A reader
/// on the far network folds the proof and compares the result with the
/// collection root, so the far network checks the Cerulea state without
/// trusting the relayer.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct OutboundBody {
    /// The collection the record sits in.
    pub collection: BoundedName,
    /// The key of the record.
    pub key: RecordKey,
    /// The stored bytes of the record.
    pub value: Vec<u8>,
    /// The inclusion root of the collection.
    pub root: [u8; 32],
    /// The proof that the record sits under the root.
    pub proof: pallet_cerulea_state::proof::InclusionProof,
}

/// One batch of far network headers.
///
/// A network settling through signatures over its own blocks carries the
/// settlement inside the message proof, so that network needs no batch.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo)]
pub enum HeaderBatch {
    /// Headers of a network running Ethereum Virtual Machine bytecode.
    EthereumVirtualMachine(networks::evm::EvmHeaderBatch),
    /// Attested slot headers of a Solana network.
    Solana(networks::solana::SolanaHeaderBatch),
    /// Headers of the Bitcoin network.
    Bitcoin(networks::bitcoin::BitcoinHeaderBatch),
    /// Headers of the settlement layer of a Layer 2 rollup.
    Rollup(networks::evm::EvmHeaderBatch),
}

impl HeaderBatch {
    /// The network kind the batch belongs to.
    pub fn kind(&self) -> NetworkKind {
        match self {
            HeaderBatch::EthereumVirtualMachine(_) => NetworkKind::EthereumVirtualMachine,
            HeaderBatch::Solana(_) => NetworkKind::Solana,
            HeaderBatch::Bitcoin(_) => NetworkKind::Bitcoin,
            HeaderBatch::Rollup(_) => NetworkKind::Rollup,
        }
    }
}

/// One endpoint as the caller declares it.
///
/// The declaration travels as one call argument. The Blueprint carries the
/// same shape in its integrations section, so a deployment reads its
/// endpoints from the Blueprint and registers them without translation.
#[derive(
    Clone, Debug, PartialEq, Eq, Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen,
)]
pub struct EndpointDeclaration<AccountId> {
    /// The name the operator gave the network on the far side.
    pub network: BoundedName,
    /// The direction messages travel through the endpoint.
    pub direction: Travel,
    /// The endpoint takes messages in both directions.
    pub both_ways: bool,
    /// The collection the endpoint reads and writes.
    pub collection: BoundedName,
    /// The action an arriving message runs.
    pub action: BoundedName,
    /// The format of the payload bytes.
    pub format: MessageFormat,
    /// The transaction class an arriving message runs under.
    pub class: WorkClass,
    /// The lifetime an outbound message takes, in milliseconds.
    pub lifetime_millis: u64,
    /// The verifier parameters of the network kind.
    pub params: VerifierParameters,
    /// The account an arriving message runs as.
    pub executor: AccountId,
}

/// The action an arriving message runs.
///
/// The logic module implements this contract through the adapter the
/// module ships. A runtime therefore reaches the Blueprint interpreter
/// without the bridge module reaching into the storage of another module.
pub trait ActionRunner {
    /// The account type the runtime carries.
    type AccountId;

    /// Run one action with one payload and answer with the count of
    /// effects the run performed.
    fn run(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
        action: &str,
        payload: &[u8],
        format: MessageFormat,
    ) -> Result<u32, DispatchError>;
}

/// Turn one fault into a plain sentence naming the fix.
pub fn fault_sentence(fault: BridgeFault) -> &'static str {
    fault.fix()
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    /// The settings the runtime supplies to the bridge module.
    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The event type of the runtime.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The weight figures of the calls.
        type WeightInfo: WeightInfo;

        /// The store an outbound message reads its record and its proof
        /// from.
        type Records: RecordStore<AccountId = Self::AccountId>;

        /// The log every bridge event lands in.
        type Audit: AuditSink<AccountId = Self::AccountId>;

        /// The gate that answers whether the electorate approved an
        /// endpoint change.
        type Approvals: ApprovalGate;

        /// The interpreter an arriving message runs its action through.
        type Actions: ActionRunner<AccountId = Self::AccountId>;

        /// The clock every expiry reads.
        type Time: UnixTime;

        /// The largest number of endpoints one deployment registers.
        #[pallet::constant]
        type MaxEndpointsPerDeployment: Get<u32>;

        /// The largest number of far network headers one endpoint keeps.
        #[pallet::constant]
        type MaxHeadersPerEndpoint: Get<u32>;

        /// The largest number of messages one expiry bucket holds.
        #[pallet::constant]
        type MaxPendingPerBucket: Get<u32>;

        /// The largest number of messages one block sweep cancels.
        #[pallet::constant]
        type MaxSweepPerBlock: Get<u32>;

        /// The largest number of expiry buckets one block sweep walks.
        #[pallet::constant]
        type MaxBucketsPerBlock: Get<u32>;

        /// The longest lifetime an outbound message takes, in
        /// milliseconds.
        #[pallet::constant]
        type MaxLifetimeMillis: Get<u64>;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    /// The endpoints of every deployment.
    #[pallet::storage]
    pub type Endpoints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        EndpointRecord<T::AccountId, BlockNumberFor<T>>,
    >;

    /// The endpoint names of one deployment, in registration order.
    #[pallet::storage]
    pub type EndpointNames<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        DeploymentId,
        BoundedVec<BoundedName, T::MaxEndpointsPerDeployment>,
        ValueQuery,
    >;

    /// The far network headers one endpoint accepted.
    #[pallet::storage]
    pub type Headers<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EndpointKey, Twox64Concat, u64, HeaderRecord>;

    /// The highest far network height one endpoint accepted.
    #[pallet::storage]
    pub type Tips<T: Config> = StorageMap<_, Blake2_128Concat, EndpointKey, u64, ValueQuery>;

    /// The lowest far network height one endpoint still keeps.
    #[pallet::storage]
    pub type Floors<T: Config> = StorageMap<_, Blake2_128Concat, EndpointKey, u64, ValueQuery>;

    /// The replay registry.
    ///
    /// The entry lands the moment the module accepts a message. A second
    /// attempt carrying the same identifier finds the entry and stops.
    #[pallet::storage]
    pub type Seen<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        Blake2_128Concat,
        MessageId,
        SeenRecord<BlockNumberFor<T>>,
    >;

    /// The inbound messages one endpoint accepted.
    #[pallet::storage]
    pub type Inbound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        Blake2_128Concat,
        MessageId,
        InboundRecord<BlockNumberFor<T>>,
    >;

    /// The outbound messages one endpoint produced.
    #[pallet::storage]
    pub type Outbound<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        Blake2_128Concat,
        MessageId,
        OutboundRecord<BlockNumberFor<T>>,
    >;

    /// The body of one outbound message.
    #[pallet::storage]
    pub type OutboundBodies<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        Blake2_128Concat,
        MessageId,
        BoundedVec<u8, ConstU32<{ envelope::MAX_PAYLOAD_BYTES }>>,
    >;

    /// The position of the next outbound message of one endpoint.
    #[pallet::storage]
    pub type OutboundNonce<T: Config> =
        StorageMap<_, Blake2_128Concat, EndpointKey, u64, ValueQuery>;

    /// The messages waiting in one expiry bucket.
    #[pallet::storage]
    pub type ExpirySchedule<T: Config> = StorageMap<
        _,
        Twox64Concat,
        u64,
        BoundedVec<(EndpointKey, MessageId, Travel), T::MaxPendingPerBucket>,
        ValueQuery,
    >;

    /// The expiry bucket the sweep reads next.
    #[pallet::storage]
    pub type SweepCursor<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// The state root anchors of one endpoint.
    #[pallet::storage]
    pub type Anchors<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EndpointKey,
        Twox64Concat,
        u64,
        anchor::AnchorRecord<BlockNumberFor<T>>,
    >;

    /// The position of the next anchor of one endpoint.
    #[pallet::storage]
    pub type AnchorNonce<T: Config> = StorageMap<_, Blake2_128Concat, EndpointKey, u64, ValueQuery>;

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// One endpoint joined the deployment.
        EndpointRegistered {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The kind of network on the far side.
            kind: NetworkKind,
        },
        /// One endpoint stopped taking messages.
        EndpointRetired {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
        },
        /// One endpoint accepted far network headers.
        HeadersAccepted {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The highest height the batch reached.
            tip: u64,
            /// The count of headers the batch carried.
            count: u32,
        },
        /// One inbound message passed its proof and ran its action.
        MessageExecuted {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The identifier of the message.
            id: MessageId,
            /// The far network height the proof reached.
            height: u64,
            /// The count of effects the action ran.
            effects: u32,
        },
        /// One outbound message is ready for the driver.
        MessageReady {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The identifier of the message.
            id: MessageId,
            /// The digest the far network commits to.
            commitment: [u8; 32],
            /// The moment the message stops being acceptable.
            expiry_millis: u64,
        },
        /// The far network took delivery of one outbound message.
        MessageDelivered {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The identifier of the message.
            id: MessageId,
            /// The reference the far network answered with.
            reference: [u8; 32],
        },
        /// One message passed its expiry and stopped.
        MessageCancelled {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The identifier of the message.
            id: MessageId,
            /// The direction the message travelled.
            travel: Travel,
        },
        /// One state root anchor is ready for the driver.
        AnchorRequested {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The position of the anchor.
            anchor: u64,
            /// The state root the anchor commits.
            root: [u8; 32],
        },
        /// The far network holds one state root anchor.
        AnchorConfirmed {
            /// The deployment the endpoint belongs to.
            deployment: DeploymentId,
            /// The endpoint name.
            endpoint: BoundedName,
            /// The position of the anchor.
            anchor: u64,
            /// The far network height the anchor sits at.
            height: u64,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The deployment holds no endpoint under that name. Register the
        /// endpoint before sending a message through it.
        EndpointUnknown,
        /// The deployment already holds an endpoint under that name. Pick
        /// a name the deployment lacks.
        EndpointRepeated,
        /// The deployment reached its endpoint limit. Retire an endpoint
        /// before registering another.
        EndpointLimit,
        /// The endpoint stopped taking messages. Register the endpoint
        /// again to resume.
        EndpointRetired,
        /// The endpoint takes messages the other way. Register an endpoint
        /// that takes messages this way.
        DirectionDeclined,
        /// The verifier parameters name a network kind the endpoint lacks.
        /// Send the parameters of the network kind the endpoint names.
        KindMismatch,
        /// The electorate approved no such endpoint change. Raise a
        /// structural proposal carrying these endpoint bytes and carry it
        /// out.
        ApprovalMissing,
        /// The endpoint accepted this message already. Send a message
        /// carrying the next position of the sender.
        MessageReplayed,
        /// The endpoint holds no message under that identifier. Send the
        /// identifier the acceptance answered with.
        MessageUnknown,
        /// The message passed its expiry. Send a message carrying a later
        /// expiry.
        MessageExpired,
        /// The message stands past the step this call performs. Read the
        /// message status before calling again.
        MessageSettled,
        /// The proof failed. Read the fault the event carries and send the
        /// proof the far network produced.
        ProofDeclined,
        /// The collection holds no record under that key. Write the record
        /// before sending it across.
        RecordAbsent,
        /// The collection carries no inclusion root yet. Write one record
        /// into the collection before anchoring it.
        RootAbsent,
        /// The message runs past the size the module accepts. Send a
        /// smaller record across.
        PayloadTooLarge,
        /// The expiry bucket reached its limit. Wait for the sweep to
        /// clear the bucket and send the message again.
        BucketFull,
        /// The lifetime runs past the limit the runtime holds. Shorten the
        /// lifetime of the endpoint.
        LifetimeTooLong,
        /// The endpoint holds no anchor at that position. Request the
        /// anchor before confirming it.
        AnchorUnknown,
        /// The anchor carries a state root the far network lacks. Write
        /// the anchor transaction the request named.
        AnchorMismatch,
        /// The header batch names a network kind the endpoint lacks. Send
        /// the header shape the endpoint network kind names.
        BatchMismatch,
        /// The header batch failed its checks. Send headers linking to the
        /// trusted checkpoint of the endpoint.
        HeadersDeclined,
        /// The endpoint name runs past the limit. Shorten the name to one
        /// hundred and twenty eight bytes.
        NameTooLong,
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_now: BlockNumberFor<T>) -> Weight {
            sweep::run::<T>()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Register one endpoint.
        ///
        /// A root origin registers an endpoint directly. Any other origin
        /// registers an endpoint the electorate approved at the structural
        /// class threshold.
        #[pallet::call_index(0)]
        #[pallet::weight(T::WeightInfo::register_endpoint())]
        pub fn register_endpoint(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            declaration: EndpointDeclaration<T::AccountId>,
        ) -> DispatchResult {
            let who = Self::authorise(
                origin,
                &deployment,
                &(name.clone(), declaration.clone()).encode(),
            )?;
            ensure!(
                declaration.lifetime_millis > 0
                    && declaration.lifetime_millis <= T::MaxLifetimeMillis::get(),
                Error::<T>::LifetimeTooLong
            );
            let key = EndpointKey::new(deployment, name.clone());
            ensure!(
                Endpoints::<T>::get(&key).is_none(),
                Error::<T>::EndpointRepeated
            );
            EndpointNames::<T>::try_mutate(deployment, |held| -> DispatchResult {
                held.try_push(name.clone())
                    .map_err(|_| Error::<T>::EndpointLimit)?;
                Ok(())
            })?;

            let kind = declaration.params.kind();
            if let Some(checkpoint) = declaration.params.checkpoint() {
                if checkpoint.height > 0 {
                    Tips::<T>::insert(&key, checkpoint.height);
                    Floors::<T>::insert(&key, checkpoint.height);
                }
            }
            let record = EndpointRecord {
                kind,
                network: declaration.network,
                direction: declaration.direction,
                both_ways: declaration.both_ways,
                collection: declaration.collection,
                action: declaration.action,
                format: declaration.format,
                class: declaration.class,
                lifetime_millis: declaration.lifetime_millis,
                params: declaration.params,
                registrar: who.clone(),
                executor: declaration.executor,
                registered_at: frame_system::Pallet::<T>::block_number(),
                live: true,
            };
            Endpoints::<T>::insert(&key, record);

            Self::log(
                &deployment,
                "endpoint.registered",
                WorkClass::Structural,
                &who,
                "The deployment registered a bridge endpoint.",
                name.encode(),
            )?;
            Self::deposit_event(Event::EndpointRegistered {
                deployment,
                endpoint: name,
                kind,
            });
            Ok(())
        }

        /// Stop one endpoint taking messages.
        #[pallet::call_index(1)]
        #[pallet::weight(T::WeightInfo::retire_endpoint())]
        pub fn retire_endpoint(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
        ) -> DispatchResult {
            let who = Self::authorise(origin, &deployment, &name.encode())?;
            let key = EndpointKey::new(deployment, name.clone());
            Endpoints::<T>::try_mutate(&key, |slot| -> DispatchResult {
                let record = slot.as_mut().ok_or(Error::<T>::EndpointUnknown)?;
                record.live = false;
                Ok(())
            })?;
            Self::log(
                &deployment,
                "endpoint.retired",
                WorkClass::Structural,
                &who,
                "The deployment retired a bridge endpoint.",
                name.encode(),
            )?;
            Self::deposit_event(Event::EndpointRetired {
                deployment,
                endpoint: name,
            });
            Ok(())
        }

        /// Submit one batch of far network headers.
        #[pallet::call_index(2)]
        #[pallet::weight(T::WeightInfo::submit_headers())]
        pub fn submit_headers(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            batch: HeaderBatch,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            let key = EndpointKey::new(deployment, name.clone());
            let record = Endpoints::<T>::get(&key).ok_or(Error::<T>::EndpointUnknown)?;
            ensure!(record.live, Error::<T>::EndpointRetired);
            ensure!(batch.kind() == record.kind, Error::<T>::BatchMismatch);

            let now = Self::now_millis();
            let accepted = Self::read_batch(&record.params, &batch, now)
                .map_err(|_| Error::<T>::HeadersDeclined)?;
            let count = accepted.len() as u32;
            let tip = Self::record_headers(&key, accepted)?;

            Endpoints::<T>::try_mutate(&key, |slot| -> DispatchResult {
                let held = slot.as_mut().ok_or(Error::<T>::EndpointUnknown)?;
                if let Some(top) = Headers::<T>::get(&key, tip) {
                    held.params.advance(endpoint::Checkpoint {
                        height: top.height,
                        hash: top.hash,
                    });
                }
                Ok(())
            })?;

            Self::log(
                &deployment,
                "headers.accepted",
                WorkClass::Routine,
                &who,
                "The endpoint accepted far network headers.",
                (tip, count).encode(),
            )?;
            Self::deposit_event(Event::HeadersAccepted {
                deployment,
                endpoint: name,
                tip,
                count,
            });
            Ok(())
        }

        /// Deliver one inbound message with its proof.
        ///
        /// The call checks the proof, writes the replay entry and runs the
        /// action the endpoint names. The three steps run inside one
        /// dispatch, so the Blockchain commits all three or none.
        #[pallet::call_index(3)]
        #[pallet::weight(T::WeightInfo::deliver_message())]
        pub fn deliver_message(
            origin: OriginFor<T>,
            envelope: MessageEnvelope,
            proof: verify::Proof,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            inbound::deliver::<T>(&who, &envelope, &proof)
        }

        /// Produce one outbound message covering one record.
        #[pallet::call_index(4)]
        #[pallet::weight(T::WeightInfo::dispatch_outbound())]
        pub fn dispatch_outbound(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            key: RecordKey,
            party: envelope::NetworkAddress,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            outbound::produce::<T>(&who, deployment, name, key, party)
        }

        /// Record the delivery of one outbound message.
        #[pallet::call_index(5)]
        #[pallet::weight(T::WeightInfo::confirm_outbound())]
        pub fn confirm_outbound(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            id: MessageId,
            reference: [u8; 32],
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            outbound::confirm::<T>(&who, deployment, name, id, reference)
        }

        /// Request one state root anchor on the far network.
        #[pallet::call_index(6)]
        #[pallet::weight(T::WeightInfo::request_anchor())]
        pub fn request_anchor(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            collection: BoundedName,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            anchor::request::<T>(&who, deployment, name, collection)
        }

        /// Confirm that the far network holds one state root anchor.
        #[pallet::call_index(7)]
        #[pallet::weight(T::WeightInfo::confirm_anchor())]
        pub fn confirm_anchor(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            position: u64,
            evidence: anchor::AnchorEvidence,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            anchor::confirm::<T>(&who, deployment, name, position, &evidence)
        }

        /// Request one Cerulea checkpoint anchor on the far network.
        ///
        /// A checkpoint is one committed block the chain names as settled. The
        /// anchored set the framework pallet holds is the set this path
        /// commits externally. The driver writes the thirty two byte
        /// commitment into a data carrying output of the far network.
        #[pallet::call_index(9)]
        #[pallet::weight(T::WeightInfo::request_anchor())]
        pub fn request_checkpoint_anchor(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            checkpoint: cerulea_primitives::checkpoint::Checkpoint,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            anchor::request_checkpoint::<T>(&who, deployment, name, checkpoint)
        }

        /// Cancel one message that passed its expiry.
        #[pallet::call_index(8)]
        #[pallet::weight(T::WeightInfo::cancel_message())]
        pub fn cancel_message(
            origin: OriginFor<T>,
            deployment: DeploymentId,
            name: BoundedName,
            id: MessageId,
            travel: Travel,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            sweep::cancel_expired::<T>(&who, deployment, name, id, travel)
        }
    }
}

/// The headers of one endpoint, read from storage.
pub struct StoredHeaders<T: Config> {
    /// The endpoint the reader answers for.
    pub key: EndpointKey,
    /// The runtime the reader belongs to.
    pub marker: core::marker::PhantomData<T>,
}

impl<T: Config> StoredHeaders<T> {
    /// Build a reader over one endpoint.
    pub fn new(key: EndpointKey) -> Self {
        StoredHeaders {
            key,
            marker: core::marker::PhantomData,
        }
    }
}

impl<T: Config> HeaderSource for StoredHeaders<T> {
    fn header(&self, height: u64) -> Option<HeaderRecord> {
        Headers::<T>::get(&self.key, height)
    }

    fn tip_height(&self) -> u64 {
        Tips::<T>::get(&self.key)
    }
}

impl<T: Config> Pallet<T> {
    /// The moment the clock of the runtime reads, in milliseconds.
    pub fn now_millis() -> u64 {
        let held = T::Time::now();
        u64::try_from(held.as_millis()).unwrap_or(u64::MAX)
    }

    /// The bucket one moment falls in.
    pub fn bucket_of(millis: u64) -> u64 {
        millis / BUCKET_MILLIS
    }

    /// The endpoint record under one key.
    pub fn endpoint(
        deployment: &DeploymentId,
        name: &BoundedName,
    ) -> Option<EndpointRecord<T::AccountId, BlockNumberFor<T>>> {
        Endpoints::<T>::get(EndpointKey::new(*deployment, name.clone()))
    }

    /// The endpoint names of one deployment.
    pub fn endpoint_names(deployment: &DeploymentId) -> Vec<BoundedName> {
        EndpointNames::<T>::get(deployment).into_inner()
    }

    /// The inbound record under one identifier.
    pub fn inbound_message(
        deployment: &DeploymentId,
        name: &BoundedName,
        id: &MessageId,
    ) -> Option<InboundRecord<BlockNumberFor<T>>> {
        Inbound::<T>::get(EndpointKey::new(*deployment, name.clone()), id)
    }

    /// The outbound record under one identifier.
    pub fn outbound_message(
        deployment: &DeploymentId,
        name: &BoundedName,
        id: &MessageId,
    ) -> Option<OutboundRecord<BlockNumberFor<T>>> {
        Outbound::<T>::get(EndpointKey::new(*deployment, name.clone()), id)
    }

    /// The body of one outbound message.
    pub fn outbound_body(
        deployment: &DeploymentId,
        name: &BoundedName,
        id: &MessageId,
    ) -> Option<Vec<u8>> {
        OutboundBodies::<T>::get(EndpointKey::new(*deployment, name.clone()), id)
            .map(|held| held.into_inner())
    }

    /// Report whether one endpoint saw one message already.
    pub fn has_seen(deployment: &DeploymentId, name: &BoundedName, id: &MessageId) -> bool {
        Seen::<T>::contains_key(EndpointKey::new(*deployment, name.clone()), id)
    }

    /// Check the origin of an endpoint change.
    ///
    /// A root origin passes. Any other origin passes where the electorate
    /// approved the change and a module carried it out.
    pub(crate) fn authorise(
        origin: OriginFor<T>,
        deployment: &DeploymentId,
        payload: &[u8],
    ) -> Result<T::AccountId, DispatchError> {
        match ensure_signed_or_root(origin)? {
            Some(who) => {
                ensure!(
                    T::Approvals::is_executed(deployment, ENDPOINT_PROPOSAL, payload),
                    Error::<T>::ApprovalMissing
                );
                Ok(who)
            }
            None => Ok(Self::platform_account()),
        }
    }

    /// The account the module records for a change a root origin made.
    pub fn platform_account() -> T::AccountId {
        let seed = sp_crypto_hashing::blake2_256(b"cerulea.bridge.platform.v1");
        T::AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::new(&seed))
            .expect("an account decodes from a thirty two byte digest")
    }

    /// The account the module records for one far network sender.
    ///
    /// The far network sender holds no account on this chain. The module
    /// derives a stable account from the network name and the sending
    /// address, so the audit log names the same actor context for every
    /// message of that sender.
    pub fn source_account(source: &BoundedName, party: &[u8]) -> T::AccountId {
        let seed = sp_crypto_hashing::blake2_256(
            &(b"cerulea.bridge.actor.v1", source.as_slice(), party).encode(),
        );
        T::AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::new(&seed))
            .expect("an account decodes from a thirty two byte digest")
    }

    /// Read one header batch against the parameters of one endpoint.
    pub(crate) fn read_batch(
        params: &VerifierParameters,
        batch: &HeaderBatch,
        now_millis: u64,
    ) -> Result<Vec<HeaderRecord>, BridgeFault> {
        match (params, batch) {
            (
                VerifierParameters::EthereumVirtualMachine(held),
                HeaderBatch::EthereumVirtualMachine(carried),
            ) => networks::evm::accept_headers(held, carried, now_millis),
            (VerifierParameters::Solana(held), HeaderBatch::Solana(carried)) => {
                networks::solana::accept_headers(held, carried, now_millis)
            }
            (VerifierParameters::Bitcoin(held), HeaderBatch::Bitcoin(carried)) => {
                networks::bitcoin::accept_headers(held, carried, now_millis)
            }
            (VerifierParameters::Rollup(held), HeaderBatch::Rollup(carried)) => {
                networks::evm::accept_headers(&held.settlement, carried, now_millis)
            }
            _ => Err(BridgeFault::KindMismatch),
        }
    }

    /// Write one list of accepted headers and answer with the new tip.
    ///
    /// The endpoint keeps a bounded window of headers. A header falling
    /// below the window is removed, so the storage of one endpoint stays
    /// inside a fixed size.
    pub(crate) fn record_headers(
        key: &EndpointKey,
        accepted: Vec<HeaderRecord>,
    ) -> Result<u64, DispatchError> {
        let mut tip = Tips::<T>::get(key);
        for held in accepted.iter() {
            Headers::<T>::insert(key, held.height, held);
            tip = tip.max(held.height);
        }
        Tips::<T>::insert(key, tip);

        let window = u64::from(T::MaxHeadersPerEndpoint::get().max(1));
        let mut floor = Floors::<T>::get(key);
        let lowest = tip.saturating_sub(window).saturating_add(1);
        let mut removed = 0u32;
        while floor < lowest && removed < T::MaxHeadersPerEndpoint::get() {
            Headers::<T>::remove(key, floor);
            floor = floor.saturating_add(1);
            removed = removed.saturating_add(1);
        }
        Floors::<T>::insert(key, floor);
        Ok(tip)
    }

    /// Place one message in the bucket of its expiry.
    pub(crate) fn schedule(
        key: &EndpointKey,
        id: MessageId,
        travel: Travel,
        expiry_millis: u64,
    ) -> DispatchResult {
        let bucket = Self::bucket_of(expiry_millis);
        ExpirySchedule::<T>::try_mutate(bucket, |held| -> DispatchResult {
            held.try_push((key.clone(), id, travel))
                .map_err(|_| Error::<T>::BucketFull)?;
            Ok(())
        })?;
        let cursor = SweepCursor::<T>::get();
        if cursor == 0 || bucket < cursor {
            SweepCursor::<T>::put(bucket);
        }
        Ok(())
    }

    /// Append one record to the audit log.
    pub(crate) fn log(
        deployment: &DeploymentId,
        event: &str,
        class: WorkClass,
        actor: &T::AccountId,
        reason: &str,
        payload: Vec<u8>,
    ) -> DispatchResult {
        T::Audit::record(
            AuditRequest::new(*deployment, MODULE_NAME, event, actor, reason)
                .classed(class)
                .carrying(payload),
        )?;
        Ok(())
    }
}

/// The adapter that reaches the Blueprint interpreter of the logic module.
///
/// The runtime names this type as the action runner of the bridge module.
/// The adapter decodes the payload into a Cerulea value and binds the
/// value to the parameters the action declares.
pub struct LogicActions<T>(core::marker::PhantomData<T>);

impl<T: pallet_cerulea_logic::Config> ActionRunner for LogicActions<T> {
    type AccountId = T::AccountId;

    fn run(
        deployment: &DeploymentId,
        actor: &Self::AccountId,
        action: &str,
        payload: &[u8],
        format: MessageFormat,
    ) -> Result<u32, DispatchError> {
        use cerulea_blueprint::types::Value;
        use pallet_cerulea_logic::LogicInterpreter;

        let blueprint = pallet_cerulea_logic::Pallet::<T>::blueprint(deployment).ok_or(
            DispatchError::Other("Install the Blueprint before a bridge message runs an action."),
        )?;
        let declared = blueprint
            .actions
            .iter()
            .find(|held| held.id.as_str() == action)
            .ok_or(DispatchError::Other(
                "Declare the action the bridge endpoint names in the Blueprint.",
            ))?;

        let carried = match format {
            MessageFormat::CanonicalValue => Value::decode(&mut &payload[..]).map_err(|_| {
                DispatchError::Other(
                    "Send the canonical encoding of a Cerulea value as the payload.",
                )
            })?,
            MessageFormat::RawBytes | MessageFormat::WordAligned => Value::Bytes(payload.to_vec()),
        };

        let mut bindings: pallet_cerulea_logic::Bindings = vec![];
        match (&carried, declared.params.len()) {
            (Value::Record(held), count) if count > 1 => {
                for param in declared.params.iter() {
                    let value = held
                        .field(&cerulea_blueprint::types::FieldName::new(
                            param.name.as_str(),
                        ))
                        .cloned()
                        .ok_or(DispatchError::Other(
                            "Carry one field per action parameter in the message payload.",
                        ))?;
                    bindings.push((param.name.clone(), value));
                }
            }
            _ => {
                let param = declared.params.first().ok_or(DispatchError::Other(
                    "Declare one parameter on the action the bridge endpoint names.",
                ))?;
                bindings.push((param.name.clone(), carried));
            }
        }

        let outcome =
            pallet_cerulea_logic::Pallet::<T>::run_action(deployment, actor, action, &bindings)?;
        Ok(outcome.effects)
    }
}
