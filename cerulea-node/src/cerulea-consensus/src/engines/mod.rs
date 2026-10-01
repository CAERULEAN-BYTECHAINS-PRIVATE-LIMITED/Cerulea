//! The engines of the Dynamic Consensus Framework.
//!
//! The Dynamic Consensus Framework is the agreement mechanism of the Cerulea
//! Blockchain. The framework separates three decisions that other designs
//! bundle together. Who may author a block. Who authors the next block. When a
//! block becomes irreversible. Eight engines carry the work. One shared
//! measure, the Trust Score, joins them.
//!
//! | Engine | Responsibility |
//! |---|---|
//! | Commit Engine | Reaches agreement and makes blocks irreversible |
//! | Authoring Engine | Selects the slot committee and the block author |
//! | Execution Engine | Groups transactions into domains and runs them in parallel |
//! | Assurance Engine | Applies the threshold appropriate to each transaction class |
//! | Custody Engine | Issues history challenges and scores the responses |
//! | Standing Engine | Computes and maintains the Trust Score |
//! | Admission Engine | Governs entry to and exit from the validator set |
//! | Amendment Engine | Governs changes to the seven engines above |
//!
//! This module holds the types the node-side engines share. The Commit,
//! Authoring and Execution Engines live in this crate. The remaining five live
//! in the pallet set.
//!
//! # The limits that stay fixed
//!
//! Two limits hold for every deployment profile. Neither limit is exposed and
//! neither limit is crossable.
//!
//! The first limit is the fault bound. Byzantine fault tolerance means the
//! network stays correct while some participants fail or act against the
//! network. Safety holds while the total participant count exceeds three times
//! the faulty count. [`participants_exceed_three_times_faulty`] states the
//! limit in code. Every candidate deployment profile passes that check before
//! the Amendment Engine presents it.
//!
//! The second limit is the timing assumption. No deterministic protocol
//! guarantees termination in a fully asynchronous network holding even one
//! faulty participant. Cerulea therefore assumes a bound on message delay that
//! eventually holds. [`ASSUMED_MESSAGE_DELAY_BOUND_MS`] carries the assumed
//! bound. Safety holds at every moment. Progress resumes once message delay
//! settles under that bound.

pub mod manifest;
pub mod manifest_derive;
pub mod manifest_guard;

pub mod authoring;
pub mod bounds;
pub mod checkpoint;
pub mod commit;
pub mod execution;
pub mod peers;

pub use authoring::{
    derive_slot_seed, draw, draw_from_keystore, draw_value, epoch_draw, epoch_draw_from_keystore,
    verify_draw, verify_epoch_draw, verify_result, weighted_draw, CommitteeDraw, DrawInput,
    DrawProof, DrawResult, SlotCommittee,
};

pub use checkpoint::{
    accepts_block, accepts_reorganisation, checkpoint_due, checkpoint_index, follows,
    highest_anchored_block, Checkpoint, CheckpointFault, CheckpointStore, SyncGate,
    CHECKPOINT_FILE_NAME, DEFAULT_CHECKPOINT_INTERVAL, MAX_HELD_CHECKPOINTS,
};

pub use commit::{
    cast_vote, check_vote, vote_payload, AggregateSignature, CommitVote, SignatureCollector,
    SignerBitmap, VoteAdmission,
};

pub use execution::{
    domain_root, empty_root, leaf_hash, merkle_root, node_hash, order_domains, order_members,
    DisjointSet, DomainOutcome, DomainPartitioner, DomainRunner, DomainStore, ExecutionReport,
    OrderKey, ResourceLimits, ResourceMeter, ResourceUse, StateRoot, StateRoots,
    TransactionOutcome, TransactionRunner,
};

pub use bounds::{admit_and_decode, admit_bytes, price, Admitted, BoundFault, MessagePath};

pub use peers::{
    AddressPrefix, DiversityAssessment, DiversityPolicy, EclipseReason, PeerAddress, PeerRecord,
    PeerSet, DEFAULT_MIN_PEERS, DEFAULT_MIN_PREFIXES, IPV4_PREFIX_BYTES, IPV6_PREFIX_BYTES,
};

pub use manifest_guard::{
    run_under_manifest, AccessTally, ManifestGuard, MemoryStateStore, StateAccess, StoredRecord,
};

pub use manifest_derive::{
    derive_manifest, derive_manifest_from, AccessSpec, ActionDescription, DescribesAccess,
    KeyExpression, KeyPart, ParameterValue, TransactionParameters,
};

pub use manifest::{
    AccessManifest, CollectionAccess, DeclaresAccess, MaxAccess, PendingTransaction, MAX_ACCESS,
    MAX_KEY_BYTES,
};

use codec::{Decode, Encode};
use scale_info::TypeInfo;
use thiserror::Error;

/// The number of validators drawn into the committee for one slot.
pub const COMMITTEE_SIZE: usize = cerulea_primitives::COMMITTEE_SIZE;

/// The multiple that sets the fault bound.
///
/// Safety holds while the total participant count exceeds this multiple of the
/// faulty count.
pub const FAULT_MULTIPLE: u32 = 3;

/// The assumed bound on message delay, in milliseconds.
///
/// No deterministic protocol guarantees termination in a fully asynchronous
/// network holding even one faulty participant. Cerulea assumes a bound on
/// message delay that eventually holds, and states the assumption here. Safety
/// holds at every moment. Progress resumes once delay settles under the bound.
pub const ASSUMED_MESSAGE_DELAY_BOUND_MS: u64 = 2_000;

/// The share of aggregated weight a routine transaction requires, in percent.
pub const ROUTINE_THRESHOLD_PERCENT: u8 = 51;

/// The share of aggregated weight a financial transaction requires, in percent.
pub const FINANCIAL_THRESHOLD_PERCENT: u8 = 67;

/// The share of aggregated weight a structural transaction requires, in percent.
pub const STRUCTURAL_THRESHOLD_PERCENT: u8 = 80;

/// The share of aggregated weight a constitutional transaction requires, in
/// percent.
pub const CONSTITUTIONAL_THRESHOLD_PERCENT: u8 = 90;

/// The default threshold profile.
///
/// A deployment profile may raise or lower each entry. The Amendment Engine
/// governs the change. This table is the profile a deployment starts from.
pub const DEFAULT_THRESHOLD_TABLE: [(TransactionClass, u8); 4] = [
    (TransactionClass::Routine, ROUTINE_THRESHOLD_PERCENT),
    (TransactionClass::Financial, FINANCIAL_THRESHOLD_PERCENT),
    (TransactionClass::Structural, STRUCTURAL_THRESHOLD_PERCENT),
    (
        TransactionClass::Constitutional,
        CONSTITUTIONAL_THRESHOLD_PERCENT,
    ),
];

/// Report whether a participant count respects the fault bound.
///
/// Safety holds while the total participant count exceeds three times the
/// faulty count. Every deployment profile passes this check before the
/// Amendment Engine presents it.
pub const fn participants_exceed_three_times_faulty(total: u32, faulty: u32) -> bool {
    (total as u64) > (FAULT_MULTIPLE as u64) * (faulty as u64)
}

/// Report the highest faulty count a participant count tolerates.
///
/// A set of `total` participants tolerates this many faulty members while
/// safety holds.
pub const fn max_faulty(total: u32) -> u32 {
    if total == 0 {
        0
    } else {
        (total - 1) / FAULT_MULTIPLE
    }
}

/// The class of a transaction.
///
/// The class sets the share of aggregated Trust Score weight the transaction
/// requires before commitment. The Blueprint action definition carries the
/// class. Higher variants demand a higher share.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub enum TransactionClass {
    /// Scans, readings and log entries.
    #[default]
    Routine,
    /// Transfers, settlements and payments.
    Financial,
    /// Schema, permission and validator set changes.
    Structural,
    /// Policy amendment and upgrade authority.
    Constitutional,
}

impl TransactionClass {
    /// The default share of aggregated weight this class requires, in percent.
    pub const fn default_threshold_percent(&self) -> u8 {
        match self {
            TransactionClass::Routine => ROUTINE_THRESHOLD_PERCENT,
            TransactionClass::Financial => FINANCIAL_THRESHOLD_PERCENT,
            TransactionClass::Structural => STRUCTURAL_THRESHOLD_PERCENT,
            TransactionClass::Constitutional => CONSTITUTIONAL_THRESHOLD_PERCENT,
        }
    }

    /// Report whether this class waits a mandatory delay window.
    ///
    /// A constitutional change waits the full window before it takes effect.
    /// Every other class takes effect on commitment.
    pub const fn waits_delay_window(&self) -> bool {
        matches!(self, TransactionClass::Constitutional)
    }

    /// A short label for logs and interface copy.
    pub const fn label(&self) -> &'static str {
        match self {
            TransactionClass::Routine => "routine",
            TransactionClass::Financial => "financial",
            TransactionClass::Structural => "structural",
            TransactionClass::Constitutional => "constitutional",
        }
    }
}

/// The identifier of a collection.
///
/// A collection is one named store of records inside a Blueprint. The
/// identifier is the first sixteen bytes of the hash of the collection name
/// under a Cerulea domain separator. Sixteen bytes keep two distinct names
/// apart for every deployment size Cerulea supports.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct CollectionId(pub [u8; 16]);

/// The domain separator mixed into every collection identifier.
pub const COLLECTION_DOMAIN: &[u8] = b"cerulea.collection.v1";

impl CollectionId {
    /// Derive the identifier of a collection from its name.
    pub fn from_name(name: &str) -> Self {
        let mut preimage = Vec::with_capacity(COLLECTION_DOMAIN.len() + name.len());
        preimage.extend_from_slice(COLLECTION_DOMAIN);
        preimage.extend_from_slice(name.as_bytes());
        let digest = sp_core::hashing::blake2_256(&preimage);
        let mut raw = [0u8; 16];
        raw.copy_from_slice(&digest[..16]);
        CollectionId(raw)
    }

    /// Read the identifier as bytes.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }
}

/// The span of keys one access touches inside one collection.
///
/// A key is the primary identifier of one record inside a collection.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default)]
pub enum KeyRange {
    /// One key alone.
    Exact(Vec<u8>),
    /// Every key starting with the given bytes.
    Prefix(Vec<u8>),
    /// Every key in the collection.
    #[default]
    Full,
}

impl KeyRange {
    /// Report whether this range and another range share at least one key.
    ///
    /// The partitioner joins two transactions into one conflict domain where
    /// their written ranges overlap inside one collection.
    pub fn overlaps(&self, other: &KeyRange) -> bool {
        match (self, other) {
            (KeyRange::Full, _) | (_, KeyRange::Full) => true,
            (KeyRange::Exact(left), KeyRange::Exact(right)) => left == right,
            (KeyRange::Prefix(prefix), KeyRange::Exact(key))
            | (KeyRange::Exact(key), KeyRange::Prefix(prefix)) => key.starts_with(prefix),
            (KeyRange::Prefix(left), KeyRange::Prefix(right)) => {
                left.starts_with(right) || right.starts_with(left)
            }
        }
    }

    /// Report whether this range holds the given key.
    ///
    /// The manifest guard calls this on every state access during execution.
    pub fn covers(&self, key: &[u8]) -> bool {
        match self {
            KeyRange::Full => true,
            KeyRange::Exact(exact) => exact.as_slice() == key,
            KeyRange::Prefix(prefix) => key.starts_with(prefix),
        }
    }

    /// The byte length this range carries.
    pub fn byte_len(&self) -> usize {
        match self {
            KeyRange::Full => 0,
            KeyRange::Exact(bytes) | KeyRange::Prefix(bytes) => bytes.len(),
        }
    }
}

/// The Trust Score of one validator, in parts per million.
///
/// The Trust Score is the single composite measure that drives the Authoring,
/// Commit, Custody and Admission Engines. A score of zero carries no weight. A
/// score of [`TrustScore::FULL`] carries the highest weight the framework
/// grants.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct TrustScore(pub u32);

impl TrustScore {
    /// The highest Trust Score, in parts per million.
    pub const FULL: u32 = 1_000_000;

    /// Build a Trust Score from parts per million, holding it inside range.
    pub const fn from_parts_per_million(parts: u32) -> Self {
        if parts > Self::FULL {
            TrustScore(Self::FULL)
        } else {
            TrustScore(parts)
        }
    }

    /// Build a Trust Score from a percentage.
    pub const fn from_percent(percent: u8) -> Self {
        let parts = (percent as u32) * (Self::FULL / 100);
        Self::from_parts_per_million(parts)
    }

    /// Read the score as parts per million.
    pub const fn parts_per_million(&self) -> u32 {
        self.0
    }

    /// Read the score as a weight for tallying.
    pub const fn weight(&self) -> u128 {
        self.0 as u128
    }
}

/// The number of one slot.
///
/// A slot is one fixed span of time. Each slot draws its own committee.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct Slot(pub u64);

/// The seed of one slot.
///
/// Every node derives the same seed for a slot from chain state. The seed
/// enters the verifiable random draw beside the validator key.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct SlotSeed(pub [u8; 32]);

impl SlotSeed {
    /// Read the seed as bytes.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The position a validator holds inside one slot committee.
///
/// Rank one authors the block. Ranks two through five stand by in order.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo, Default,
)]
pub struct Rank(pub u8);

impl Rank {
    /// The rank that authors first.
    pub const FIRST: Rank = Rank(1);

    /// The rank that follows this one.
    pub const fn next(&self) -> Rank {
        Rank(self.0.saturating_add(1))
    }

    /// Read the rank as a zero based index into the committee.
    pub const fn index(&self) -> usize {
        (self.0.saturating_sub(1)) as usize
    }

    /// Build a rank from a zero based index into the committee.
    pub const fn from_index(index: usize) -> Rank {
        Rank((index as u8).saturating_add(1))
    }
}

/// One member of a slot committee.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct CommitteeMember<ValidatorId> {
    /// The validator drawn into the committee.
    pub validator: ValidatorId,
    /// The position the validator holds.
    pub rank: Rank,
    /// The draw output after scaling by standing.
    ///
    /// A lower value wins a lower rank. The Authoring Engine sorts on this
    /// value.
    pub weighted_draw: u128,
}

/// The committee drawn for one slot.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
pub struct Committee<ValidatorId> {
    /// The slot this committee serves.
    pub slot: Slot,
    /// The members in rank order, starting at rank one.
    pub members: Vec<CommitteeMember<ValidatorId>>,
}

impl<ValidatorId: PartialEq> Committee<ValidatorId> {
    /// Build an empty committee for a slot.
    pub fn empty(slot: Slot) -> Self {
        Committee {
            slot,
            members: Vec::new(),
        }
    }

    /// The validator holding the given rank.
    pub fn at_rank(&self, rank: Rank) -> Option<&ValidatorId> {
        self.members
            .iter()
            .find(|member| member.rank == rank)
            .map(|member| &member.validator)
    }

    /// The rank the given validator holds.
    pub fn rank_of(&self, validator: &ValidatorId) -> Option<Rank> {
        self.members
            .iter()
            .find(|member| &member.validator == validator)
            .map(|member| member.rank)
    }

    /// Report whether the given validator sits in this committee.
    pub fn holds(&self, validator: &ValidatorId) -> bool {
        self.rank_of(validator).is_some()
    }

    /// The number of members drawn.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Report whether the committee drew zero members.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// One conflict domain.
///
/// A conflict domain is a maximal set of pending transactions whose written
/// state never overlaps the written state of another domain. Domains execute in
/// parallel and commit on their own.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct ConflictDomain {
    /// The positions of the member transactions inside the pending list.
    ///
    /// The order is the execution order the Execution Engine settled.
    pub members: Vec<usize>,
    /// The highest transaction class present in this domain.
    ///
    /// The domain commits at the threshold this class requires.
    pub highest_class: TransactionClass,
}

impl ConflictDomain {
    /// The number of transactions in this domain.
    pub fn len(&self) -> usize {
        self.members.len()
    }

    /// Report whether the domain holds zero transactions.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// One of the eight engines of the Dynamic Consensus Framework.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Encode, Decode, TypeInfo)]
pub enum EngineKind {
    /// Reaches agreement and makes blocks irreversible.
    Commit,
    /// Selects the slot committee and the block author.
    Authoring,
    /// Groups transactions into domains and runs them in parallel.
    Execution,
    /// Applies the threshold appropriate to each transaction class.
    Assurance,
    /// Issues history challenges and scores the responses.
    Custody,
    /// Computes and maintains the Trust Score.
    Standing,
    /// Governs entry to and exit from the validator set.
    Admission,
    /// Governs changes to the seven engines above.
    Amendment,
}

impl EngineKind {
    /// The name the logs carry for this engine.
    pub const fn name(&self) -> &'static str {
        match self {
            EngineKind::Commit => "Commit Engine",
            EngineKind::Authoring => "Authoring Engine",
            EngineKind::Execution => "Execution Engine",
            EngineKind::Assurance => "Assurance Engine",
            EngineKind::Custody => "Custody Engine",
            EngineKind::Standing => "Standing Engine",
            EngineKind::Admission => "Admission Engine",
            EngineKind::Amendment => "Amendment Engine",
        }
    }

    /// Every engine of the framework, in the order the specification lists them.
    pub const ALL: [EngineKind; 8] = [
        EngineKind::Commit,
        EngineKind::Authoring,
        EngineKind::Execution,
        EngineKind::Assurance,
        EngineKind::Custody,
        EngineKind::Standing,
        EngineKind::Admission,
        EngineKind::Amendment,
    ];
}

/// One engine of the Dynamic Consensus Framework.
pub trait Engine {
    /// The engine this type carries.
    const KIND: EngineKind;

    /// The name the logs carry for this engine.
    fn engine_name() -> &'static str {
        Self::KIND.name()
    }
}

/// The engine that groups transactions into domains and runs them.
pub trait ExecutionEngine: Engine {
    /// Group the pending transactions into conflict domains.
    ///
    /// Two transactions join one domain where a write of one meets a write or a
    /// read of the other. The pass runs in time proportional to the number of
    /// declared accesses plus the number of true conflicts.
    fn partition<T: DeclaresAccess>(&self, pending: &[T]) -> Vec<ConflictDomain>;

    /// Settle the execution order inside one domain.
    ///
    /// The order runs class descending, then arrival sequence ascending, then
    /// transaction hash ascending. Every validator reproduces the same order.
    fn order<T: DeclaresAccess>(&self, pending: &[T], members: &[usize]) -> Vec<usize>;
}

/// The engine that selects the slot committee and the block author.
pub trait AuthoringEngine: Engine {
    /// The identifier of a validator.
    type ValidatorId;

    /// Draw the committee for one slot.
    ///
    /// Every eligible validator evaluates a verifiable random function over the
    /// slot number and the slot seed, and publishes the result with its proof.
    /// The five lowest outputs after scaling by standing form the committee in
    /// rank order.
    fn committee(
        &self,
        input: &authoring::DrawInput,
        entries: &[(authoring::DrawResult, TrustScore)],
    ) -> Result<Committee<Self::ValidatorId>, EngineError>;

    /// The validator scheduled to author at the given rank.
    fn scheduled_author<'a>(
        &self,
        committee: &'a Committee<Self::ValidatorId>,
        rank: Rank,
    ) -> Option<&'a Self::ValidatorId>;
}

/// The engine that reaches agreement and makes blocks irreversible.
pub trait CommitEngine: Engine {
    /// The identifier of a validator.
    type ValidatorId;

    /// Add the weight of one signer to the tally.
    ///
    /// A validator counts once. A second signature from the same validator
    /// leaves the tally unchanged.
    fn add_signer(
        &mut self,
        validator: Self::ValidatorId,
        weight: TrustScore,
    ) -> Result<bool, EngineError>;

    /// Report whether the tally crossed the threshold the class requires.
    fn crossed(&self, class: TransactionClass) -> bool;
}

/// A problem raised by one of the engines.
///
/// Every message states the problem and names the fix.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EngineError {
    /// The manifest carries more entries than the limit allows.
    #[error(
        "The access manifest carries {count} entries and the limit is {limit}. \
         Fix: split the Blueprint action into smaller actions."
    )]
    ManifestTooLarge {
        /// The number of entries the manifest carried.
        count: usize,
        /// The highest number of entries a manifest carries.
        limit: usize,
    },

    /// A key range carries more bytes than the limit allows.
    #[error(
        "A key range carries {length} bytes and the limit is {limit}. \
         Fix: shorten the key expression in the Blueprint action."
    )]
    KeyRangeTooLong {
        /// The byte length the range carried.
        length: usize,
        /// The highest byte length a range carries.
        limit: usize,
    },

    /// A key expression named a parameter the transaction leaves unbound.
    #[error(
        "The action {action} names the parameter {parameter} and the transaction leaves it unbound.          Fix: supply a value for {parameter} when submitting the transaction."
    )]
    ParameterAbsent {
        /// The parameter the key expression named.
        parameter: String,
        /// The action carrying the key expression.
        action: String,
    },

    /// A key expression carried no parts.
    #[error(
        "The action {action} carries an empty key expression for the collection {collection}.          Fix: name a literal key or a parameter in that key expression."
    )]
    KeyExpressionEmpty {
        /// The action carrying the empty expression.
        action: String,
        /// The collection the expression names.
        collection: String,
    },

    /// Execution reached a record the manifest leaves out of its read side.
    #[error(
        "The transaction read the key {key} of collection {collection} and the Access Manifest          leaves that key out. Fix: declare the key in the Blueprint action read list."
    )]
    ReadOutsideManifest {
        /// The collection the access named.
        collection: String,
        /// The key the access named.
        key: String,
    },

    /// Execution reached a record the manifest leaves out of its write side.
    #[error(
        "The transaction wrote the key {key} of collection {collection} and the Access Manifest          leaves that key out. Fix: declare the key in the Blueprint action write list."
    )]
    WriteOutsideManifest {
        /// The collection the access named.
        collection: String,
        /// The key the access named.
        key: String,
    },

    /// A transaction spent more computation than its limit allows.
    #[error(
        "The transaction spent {used} computation units and the limit is {limit}. \
         Fix: split the Blueprint action into smaller actions."
    )]
    ComputationLimitCrossed {
        /// The units the transaction spent.
        used: u64,
        /// The highest number of units one transaction spends.
        limit: u64,
    },

    /// A transaction held more memory than its limit allows.
    #[error(
        "The transaction held {used} bytes of memory and the limit is {limit}. \
         Fix: read fewer records in one Blueprint action."
    )]
    MemoryLimitCrossed {
        /// The bytes the transaction held at its highest moment.
        used: u64,
        /// The highest number of bytes one transaction holds.
        limit: u64,
    },

    /// A transaction wrote more bytes than its limit allows.
    #[error(
        "The transaction wrote {used} bytes and the limit is {limit}. \
         Fix: write fewer records in one Blueprint action."
    )]
    StorageLimitCrossed {
        /// The bytes the transaction wrote.
        used: u64,
        /// The highest number of bytes one transaction writes.
        limit: u64,
    },

    /// The node keystore refused the signing request.
    #[error(
        "The node keystore refused the signing request. Detail: {detail}. \
         Fix: check that the keystore path stays readable by the node process."
    )]
    KeystoreRefused {
        /// The detail the keystore reported.
        detail: String,
    },

    /// The node keystore holds no draw key for this validator.
    #[error(
        "The node keystore holds no slot draw key for this validator. \
         Fix: insert the validator sr25519 key under the key type cerk."
    )]
    DrawKeyAbsent,

    /// A draw proof failed the check.
    #[error(
        "The slot draw proof for slot {slot} failed the check. \
         Fix: send the draw proof produced by the validator key named in the claim."
    )]
    DrawProofRejected {
        /// The slot the draw served.
        slot: u64,
    },

    /// A claimed draw output differs from the proof.
    #[error(
        "The claimed draw output for slot {slot} differs from the proof. \
         Fix: send the output the proof produces."
    )]
    DrawOutputMismatch {
        /// The slot the draw served.
        slot: u64,
    },

    /// A draw output failed to read back.
    #[error(
        "The slot draw output failed to read back. Detail: {detail}. \
         Fix: send a draw proof built by the sr25519 draw of this release."
    )]
    DrawOutputUnreadable {
        /// The detail the reader reported.
        detail: String,
    },

    /// The node keystore holds no seal key for this author.
    #[error(
        "The node keystore holds no block seal key for this author.          Fix: insert the validator ed25519 key under the key type cerk."
    )]
    SealKeyAbsent,

    /// A block seal failed the signature check.
    #[error(
        "The block seal for slot {slot} failed the signature check.          Fix: seal the block with the validator key named as the author."
    )]
    SealSignatureRejected {
        /// The slot the block served.
        slot: u64,
    },

    /// A block header carried no Cerulea seal.
    #[error(
        "The block header carries no Cerulea seal.          Fix: produce the block through a node that holds the author key."
    )]
    SealAbsent,

    /// A seal named a slot other than the one under check.
    #[error(
        "The block seal names slot {claimed} and the block sits in slot {expected}.          Fix: seal the block with the slot it belongs to."
    )]
    SealSlotMismatch {
        /// The slot the seal named.
        claimed: u64,
        /// The slot the block sits in.
        expected: u64,
    },

    /// The named author sits outside the committee for that slot.
    #[error(
        "The named author sits outside the committee drawn for slot {slot}.          Fix: produce the block on a node holding a committee seat for that slot."
    )]
    AuthorOutsideCommittee {
        /// The slot the block served.
        slot: u64,
    },

    /// The claimed rank differs from the committee position.
    #[error(
        "The block claims rank {claimed} and the committee holds that author at rank {held}.          Fix: claim the rank the committee gives the author."
    )]
    RankMismatch {
        /// The rank the block claimed.
        claimed: u8,
        /// The rank the committee holds.
        held: u8,
    },

    /// The claimed rank held no window at that moment.
    #[error(
        "Rank {rank} holds no authoring window at {moment_ms} milliseconds.          Fix: author inside the window the rank timeout gives that rank."
    )]
    RankTimingRejected {
        /// The rank the block claimed.
        rank: u8,
        /// The moment the block carried, in milliseconds.
        moment_ms: u64,
    },

    /// The block claimed a moment the local clock has yet to reach.
    #[error(
        "Rank {rank} claims the slot at {claimed_ms} milliseconds and the block arrived at {arrival_ms} milliseconds. Fix: publish the block once the window of the claimed rank opens."
    )]
    SealMomentAhead {
        /// The rank the block claimed.
        rank: u8,
        /// The moment the seal claimed, in milliseconds.
        claimed_ms: u64,
        /// The moment the block arrived, in milliseconds.
        arrival_ms: u64,
    },

    /// The validator key pair sits outside the registry.
    #[error(
        "The validator key pair sits outside the registry.          Fix: admit the validator through the Admission Engine before it authors."
    )]
    ValidatorKeysUnrecorded,

    /// The seal key named differs from the recorded key of that validator.
    #[error(
        "The seal key named differs from the key recorded for that validator.          Fix: seal the block with the ed25519 key recorded against the draw key."
    )]
    ValidatorKeysMismatch,

    /// A commit vote failed the signature check.
    #[error(
        "The commit vote failed the signature check.          Fix: sign the vote with the validator key recorded for that seat."
    )]
    VoteSignatureRejected,

    /// A commit vote came from a validator outside the set.
    #[error(
        "The commit vote came from a validator outside the active set.          Fix: admit the validator through the Admission Engine before it votes."
    )]
    VoterOutsideSet,

    /// A commit vote named a slot other than the one under collection.
    #[error(
        "The commit vote names slot {claimed} and the collection serves slot {expected}.          Fix: vote on the slot the block belongs to."
    )]
    VoteSlotMismatch {
        /// The slot the vote named.
        claimed: u64,
        /// The slot the collection serves.
        expected: u64,
    },

    /// A commit vote named a block other than the one under collection.
    #[error(
        "The commit vote for slot {slot} names another block.          Fix: vote on the block hash the author published for that slot."
    )]
    VoteBlockMismatch {
        /// The slot the collection serves.
        slot: u64,
    },

    /// A commit vote arrived outside the collection window.
    #[error(
        "The commit vote for slot {slot} arrived at {moment_ms} milliseconds and the collection          window is closed. Fix: send the vote inside the slot window."
    )]
    CollectionWindowClosed {
        /// The slot the collection serves.
        slot: u64,
        /// The moment the vote arrived, in milliseconds.
        moment_ms: u64,
    },

    /// A bitmap position sits outside the validator set.
    #[error(
        "Position {position} sits outside a validator set of {members} members.          Fix: build the bitmap from the validator set of that slot."
    )]
    BitmapPositionOutside {
        /// The position the caller named.
        position: usize,
        /// The number of members the set holds.
        members: usize,
    },

    /// An aggregate envelope carried an unknown scheme number.
    #[error(
        "The aggregate signature carries scheme {scheme} and this release reads scheme one.          Fix: upgrade the node to a release that reads that scheme."
    )]
    AggregateSchemeUnknown {
        /// The scheme number the envelope carried.
        scheme: u8,
    },

    /// An aggregate bitmap failed the shape check.
    #[error(
        "The aggregate signer bitmap fails the shape check.          Fix: build the bitmap from the validator set of that slot."
    )]
    AggregateBitmapMalformed,

    /// An aggregate named a different number of signers than it carried.
    #[error(
        "The aggregate names {named} signers and carries {carried} signatures.          Fix: place one signature for each position the bitmap names."
    )]
    AggregateCountMismatch {
        /// The number of positions the bitmap named.
        named: usize,
        /// The number of signatures the envelope carried.
        carried: usize,
    },

    /// One signature inside an aggregate failed the check.
    #[error(
        "The signature at position {position} of the aggregate failed the check.          Fix: place the signature of the validator holding that position."
    )]
    AggregateSignatureRejected {
        /// The position the signature sat at.
        position: usize,
    },

    /// A commit submission carried more bytes than the limit allows.
    #[error(
        "The commit submission carries {bytes} bytes and the limit is {limit}.          Fix: send one aggregate for one block rather than a joined message."
    )]
    SubmissionTooLarge {
        /// The bytes the submission carried.
        bytes: usize,
        /// The highest number of bytes one submission carries.
        limit: usize,
    },

    /// A commit submission named more signatures than the limit allows.
    #[error(
        "The commit submission names {count} signatures and the limit is {limit}.          Fix: send one aggregate built from the validator set of that slot."
    )]
    SubmissionCountAboveLimit {
        /// The number of signatures the submission named.
        count: usize,
        /// The highest number of signatures one submission carries.
        limit: usize,
    },

    /// The eligible validator set held no members.
    #[error(
        "The eligible validator set holds no members. \
         Fix: admit at least one validator through the Admission Engine."
    )]
    EmptyValidatorSet,

    /// A message crossed the size bound of its path.
    #[error("{detail}")]
    MessageAboveBound {
        /// The sentence naming the path, the size, the limit and the fix.
        detail: String,
    },

    /// A message inside its bound failed to read back.
    #[error(
        "The {path} failed to read back. Detail: {detail}. \
         Fix: send a message built by the release this chain runs."
    )]
    MessageUnreadable {
        /// The path the message arrived on.
        path: &'static str,
        /// The detail the reader reported.
        detail: String,
    },

    /// A peer sits outside the validator allow-list of a private chain.
    #[error(
        "The peer {peer} sits outside the validator allow-list of this chain. \
         Fix: admit the validator through the Admission Engine before it peers."
    )]
    PeerOutsideAllowList {
        /// The identifier the propagation layer gives that peer.
        peer: String,
    },

    /// A chain failed the anchored checkpoint check.
    #[error("{detail}")]
    CheckpointRejected {
        /// The sentence naming the problem and the fix.
        detail: String,
    },

    /// The local checkpoint store failed to read or to write.
    #[error(
        "The local checkpoint store failed. Detail: {detail}. \
         Fix: check that the node base path stays readable and writable."
    )]
    CheckpointStoreUnreadable {
        /// The detail the store reported.
        detail: String,
    },

    /// A seal claim or a header hash ran to an unexpected length.
    #[error(
        "The value carries {bytes} bytes and the Authoring Engine expects {expected}. \
         Fix: run a node release that builds the seal claim of this chain."
    )]
    SealClaimUnexpectedLength {
        /// The bytes the value carried.
        bytes: usize,
        /// The bytes the Authoring Engine expects.
        expected: usize,
    },

    /// An equivocation report carried more bytes than the limit allows.
    #[error(
        "The equivocation report carries {bytes} bytes and the limit is {limit}. \
         Fix: report one pair of sealed headers in one message."
    )]
    EquivocationReportTooLarge {
        /// The bytes the report carried.
        bytes: usize,
        /// The highest number of bytes one report carries.
        limit: usize,
    },

    /// An equivocation report failed to read back.
    #[error(
        "The equivocation report failed to read back. Detail: {detail}. \
         Fix: send a report built by the Authoring Engine of this release."
    )]
    EquivocationReportUnreadable {
        /// The detail the reader reported.
        detail: String,
    },

    /// An equivocation proof failed the evidence check.
    #[error("{detail}")]
    EquivocationProofRejected {
        /// The sentence naming the problem and the fix.
        detail: String,
    },

    /// A participant count would cross the fault bound.
    #[error(
        "A participant count of {total} fails to exceed three times the faulty count of {faulty}. \
         Fix: raise the participant count above {needed}."
    )]
    FaultBoundCrossed {
        /// The total participant count.
        total: u32,
        /// The faulty participant count the profile assumed.
        faulty: u32,
        /// The lowest total participant count the fault bound accepts.
        needed: u32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_threshold_table_carries_the_four_classes() {
        assert_eq!(DEFAULT_THRESHOLD_TABLE.len(), 4);
        assert_eq!(DEFAULT_THRESHOLD_TABLE[0], (TransactionClass::Routine, 51));
        assert_eq!(
            DEFAULT_THRESHOLD_TABLE[1],
            (TransactionClass::Financial, 67)
        );
        assert_eq!(
            DEFAULT_THRESHOLD_TABLE[2],
            (TransactionClass::Structural, 80)
        );
        assert_eq!(
            DEFAULT_THRESHOLD_TABLE[3],
            (TransactionClass::Constitutional, 90)
        );
        for (class, percent) in DEFAULT_THRESHOLD_TABLE {
            assert_eq!(class.default_threshold_percent(), percent);
        }
    }

    #[test]
    fn a_constitutional_change_waits_the_delay_window() {
        assert!(TransactionClass::Constitutional.waits_delay_window());
        assert!(!TransactionClass::Routine.waits_delay_window());
        assert!(!TransactionClass::Financial.waits_delay_window());
        assert!(!TransactionClass::Structural.waits_delay_window());
    }

    #[test]
    fn the_class_order_runs_routine_through_constitutional() {
        assert!(TransactionClass::Routine < TransactionClass::Financial);
        assert!(TransactionClass::Financial < TransactionClass::Structural);
        assert!(TransactionClass::Structural < TransactionClass::Constitutional);
    }

    #[test]
    fn the_fault_bound_rejects_a_set_that_is_too_small() {
        assert!(participants_exceed_three_times_faulty(4, 1));
        assert!(!participants_exceed_three_times_faulty(3, 1));
        assert!(participants_exceed_three_times_faulty(100, 33));
        assert!(!participants_exceed_three_times_faulty(99, 33));
    }

    #[test]
    fn the_tolerated_faulty_count_respects_the_fault_bound() {
        assert_eq!(max_faulty(0), 0);
        assert_eq!(max_faulty(1), 0);
        assert_eq!(max_faulty(4), 1);
        assert_eq!(max_faulty(7), 2);
        assert_eq!(max_faulty(100), 33);
        for total in 1u32..500 {
            assert!(participants_exceed_three_times_faulty(
                total,
                max_faulty(total)
            ));
        }
    }

    #[test]
    fn two_collection_names_take_two_identifiers() {
        let parcel = CollectionId::from_name("parcel");
        let holder = CollectionId::from_name("holder");
        assert_ne!(parcel, holder);
        assert_eq!(parcel, CollectionId::from_name("parcel"));
        assert_eq!(parcel.as_bytes().len(), 16);
    }

    #[test]
    fn a_full_range_overlaps_every_range() {
        let full = KeyRange::Full;
        assert!(full.overlaps(&KeyRange::Full));
        assert!(full.overlaps(&KeyRange::Exact(b"one".to_vec())));
        assert!(full.overlaps(&KeyRange::Prefix(b"one".to_vec())));
        assert!(KeyRange::Exact(b"one".to_vec()).overlaps(&full));
    }

    #[test]
    fn two_exact_keys_overlap_only_where_they_match() {
        let left = KeyRange::Exact(b"parcel-1".to_vec());
        let right = KeyRange::Exact(b"parcel-1".to_vec());
        let other = KeyRange::Exact(b"parcel-2".to_vec());
        assert!(left.overlaps(&right));
        assert!(!left.overlaps(&other));
    }

    #[test]
    fn a_prefix_overlaps_the_keys_beneath_it() {
        let prefix = KeyRange::Prefix(b"parcel-".to_vec());
        assert!(prefix.overlaps(&KeyRange::Exact(b"parcel-1".to_vec())));
        assert!(!prefix.overlaps(&KeyRange::Exact(b"holder-1".to_vec())));
        assert!(prefix.overlaps(&KeyRange::Prefix(b"parcel-north".to_vec())));
        assert!(!prefix.overlaps(&KeyRange::Prefix(b"holder".to_vec())));
    }

    #[test]
    fn a_range_covers_the_keys_it_names() {
        assert!(KeyRange::Full.covers(b"anything"));
        assert!(KeyRange::Exact(b"parcel-1".to_vec()).covers(b"parcel-1"));
        assert!(!KeyRange::Exact(b"parcel-1".to_vec()).covers(b"parcel-2"));
        assert!(KeyRange::Prefix(b"parcel-".to_vec()).covers(b"parcel-9"));
        assert!(!KeyRange::Prefix(b"parcel-".to_vec()).covers(b"holder-9"));
    }

    #[test]
    fn a_trust_score_holds_inside_range() {
        assert_eq!(TrustScore::from_parts_per_million(2_000_000).0, 1_000_000);
        assert_eq!(TrustScore::from_percent(50).0, 500_000);
        assert_eq!(TrustScore::from_percent(100).0, 1_000_000);
        assert_eq!(TrustScore::from_percent(0).weight(), 0);
    }

    #[test]
    fn a_rank_maps_to_a_committee_position() {
        assert_eq!(Rank::FIRST.index(), 0);
        assert_eq!(Rank::FIRST.next(), Rank(2));
        assert_eq!(Rank::from_index(0), Rank::FIRST);
        assert_eq!(Rank::from_index(4), Rank(5));
    }

    #[test]
    fn a_committee_reads_back_by_rank_and_by_validator() {
        let committee = Committee {
            slot: Slot(9),
            members: vec![
                CommitteeMember {
                    validator: 11u32,
                    rank: Rank(1),
                    weighted_draw: 5,
                },
                CommitteeMember {
                    validator: 22u32,
                    rank: Rank(2),
                    weighted_draw: 9,
                },
            ],
        };
        assert_eq!(committee.at_rank(Rank(1)), Some(&11));
        assert_eq!(committee.at_rank(Rank(2)), Some(&22));
        assert_eq!(committee.at_rank(Rank(3)), None);
        assert_eq!(committee.rank_of(&22), Some(Rank(2)));
        assert!(committee.holds(&11));
        assert!(!committee.holds(&33));
        assert_eq!(committee.len(), 2);
        assert!(!committee.is_empty());
        assert!(Committee::<u32>::empty(Slot(1)).is_empty());
    }

    #[test]
    fn every_engine_carries_a_name() {
        assert_eq!(EngineKind::ALL.len(), 8);
        for kind in EngineKind::ALL {
            assert!(kind.name().ends_with("Engine"));
        }
        assert_eq!(EngineKind::Commit.name(), "Commit Engine");
    }

    #[test]
    fn every_engine_error_names_a_fix() {
        let errors = [
            EngineError::ManifestTooLarge {
                count: 70,
                limit: 64,
            },
            EngineError::KeyRangeTooLong {
                length: 300,
                limit: 128,
            },
            EngineError::EmptyValidatorSet,
            EngineError::FaultBoundCrossed {
                total: 3,
                faulty: 1,
                needed: 4,
            },
        ];
        for error in errors {
            let text = error.to_string();
            assert!(text.contains("Fix:"), "the message names a fix: {}", text);
        }
    }

    #[test]
    fn the_committee_size_and_delay_bound_hold_their_stated_values() {
        assert_eq!(COMMITTEE_SIZE, 5);
        assert_eq!(FAULT_MULTIPLE, 3);
        assert!(ASSUMED_MESSAGE_DELAY_BOUND_MS > 0);
    }
}
